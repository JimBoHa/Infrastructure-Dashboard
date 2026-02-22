use crate::config::Config;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rumqttc::{AsyncClient, Event, MqttOptions, QoS};
use serde_json::json;
use sqlx::PgPool;
use sqlx::Row;
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug)]
pub enum AckCommand {
    Committed {
        node_mqtt_id: String,
        stream_id: Uuid,
        seqs: Vec<u64>,
    },
    LossRange {
        node_mqtt_id: String,
        stream_id: Uuid,
        start_seq: u64,
        end_seq: u64,
        dropped_at: Option<DateTime<Utc>>,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone)]
struct LossRange {
    start_seq: u64,
    end_seq: u64,
}

#[derive(Debug)]
struct NodeAckState {
    stream_id: Uuid,
    acked_seq: u64,
    pending: BTreeSet<u64>,
    loss_ranges: Vec<LossRange>,
    dirty: bool,
    last_published_acked_seq: u64,
}

pub fn channel() -> (mpsc::UnboundedSender<AckCommand>, mpsc::UnboundedReceiver<AckCommand>) {
    mpsc::unbounded_channel()
}

pub async fn run_ack_manager(
    config: Config,
    pool: PgPool,
    mut rx: mpsc::UnboundedReceiver<AckCommand>,
) -> Result<()> {
    let mut state = load_state(&pool).await.unwrap_or_default();

    loop {
        let mut mqttoptions = MqttOptions::new(
            format!("{}-ack", config.mqtt_client_id),
            config.mqtt_host.clone(),
            config.mqtt_port,
        );
        mqttoptions.set_keep_alive(config.mqtt_keepalive());
        if let Some(username) = &config.mqtt_username {
            mqttoptions.set_credentials(
                username.clone(),
                config.mqtt_password.clone().unwrap_or_default(),
            );
        }

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 64);
        tracing::info!("ack publisher connected to MQTT");

        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    publish_acks(&client, &config, &pool, &mut state).await;
                }
                cmd = rx.recv() => {
                    let Some(cmd) = cmd else { return Ok(()); };
                    if let Err(err) = apply_command(&pool, &mut state, cmd).await {
                        tracing::warn!(error=%err, "failed to apply ack command");
                    }
                }
                ev = eventloop.poll() => {
                    match ev {
                        Ok(Event::Incoming(_)) => {}
                        Ok(_) => {}
                        Err(err) => {
                            tracing::warn!(error=%err, "ack MQTT connection dropped; reconnecting");
                            break;
                        }
                    }
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

async fn load_state(pool: &PgPool) -> Result<HashMap<String, NodeAckState>> {
    let mut out: HashMap<String, NodeAckState> = HashMap::new();

    let rows = sqlx::query(
        r#"
        SELECT node_mqtt_id, stream_id, acked_seq
        FROM node_forwarder_ack_state
        "#
    )
    .fetch_all(pool)
    .await
    .context("load node_forwarder_ack_state")?;

    for row in rows {
        let node_mqtt_id: String = row.try_get("node_mqtt_id")?;
        let stream_id: Uuid = row.try_get("stream_id")?;
        let acked_seq: i64 = row.try_get("acked_seq")?;
        out.insert(
            node_mqtt_id,
            NodeAckState {
                stream_id,
                acked_seq: acked_seq.max(0) as u64,
                pending: BTreeSet::new(),
                loss_ranges: Vec::new(),
                dirty: false,
                last_published_acked_seq: 0,
            },
        );
    }

    let loss_rows = sqlx::query(
        r#"
        SELECT node_mqtt_id, stream_id, start_seq, end_seq
        FROM node_forwarder_loss_ranges
        ORDER BY node_mqtt_id, start_seq
        "#
    )
    .fetch_all(pool)
    .await
    .context("load node_forwarder_loss_ranges")?;

    for row in loss_rows {
        let node_mqtt_id: String = row.try_get("node_mqtt_id")?;
        let stream_id: Uuid = row.try_get("stream_id")?;
        let start_seq: i64 = row.try_get("start_seq")?;
        let end_seq: i64 = row.try_get("end_seq")?;

        if let Some(state) = out.get_mut(&node_mqtt_id) {
            if state.stream_id == stream_id {
                state.loss_ranges.push(LossRange {
                    start_seq: start_seq.max(0) as u64,
                    end_seq: end_seq.max(0) as u64,
                });
            }
        }
    }

    Ok(out)
}

async fn apply_command(pool: &PgPool, state: &mut HashMap<String, NodeAckState>, cmd: AckCommand) -> Result<()> {
    match cmd {
        AckCommand::Committed { node_mqtt_id, stream_id, seqs } => {
            if node_mqtt_id.trim().is_empty() || seqs.is_empty() {
                return Ok(());
            }
            let entry = state.entry(node_mqtt_id.clone()).or_insert_with(|| NodeAckState {
                stream_id,
                acked_seq: 0,
                pending: BTreeSet::new(),
                loss_ranges: Vec::new(),
                dirty: false,
                last_published_acked_seq: 0,
            });

            if entry.stream_id != stream_id {
                reset_node_state(pool, &node_mqtt_id, stream_id).await?;
                entry.stream_id = stream_id;
                entry.acked_seq = 0;
                entry.pending.clear();
                entry.loss_ranges.clear();
                entry.dirty = true;
            }

            for seq in seqs {
                if seq > entry.acked_seq {
                    entry.pending.insert(seq);
                }
            }

            if advance_acked_seq(entry) {
                persist_ack_state(pool, &node_mqtt_id, entry.stream_id, entry.acked_seq).await?;
                entry.dirty = true;
            }
        }
        AckCommand::LossRange {
            node_mqtt_id,
            stream_id,
            start_seq,
            end_seq,
            dropped_at,
            reason,
        } => {
            if node_mqtt_id.trim().is_empty() || start_seq == 0 || end_seq < start_seq {
                return Ok(());
            }
            let entry = state.entry(node_mqtt_id.clone()).or_insert_with(|| NodeAckState {
                stream_id,
                acked_seq: 0,
                pending: BTreeSet::new(),
                loss_ranges: Vec::new(),
                dirty: false,
                last_published_acked_seq: 0,
            });
            if entry.stream_id != stream_id {
                reset_node_state(pool, &node_mqtt_id, stream_id).await?;
                entry.stream_id = stream_id;
                entry.acked_seq = 0;
                entry.pending.clear();
                entry.loss_ranges.clear();
                entry.dirty = true;
            }

            persist_loss_range(pool, &node_mqtt_id, stream_id, start_seq, end_seq, dropped_at, reason).await?;
            entry.loss_ranges.push(LossRange { start_seq, end_seq });
            normalize_loss_ranges(&mut entry.loss_ranges);

            if advance_acked_seq(entry) {
                persist_ack_state(pool, &node_mqtt_id, entry.stream_id, entry.acked_seq).await?;
                entry.dirty = true;
            }
        }
    }
    Ok(())
}

fn normalize_loss_ranges(ranges: &mut Vec<LossRange>) {
    if ranges.len() <= 1 {
        return;
    }
    ranges.sort_by_key(|r| r.start_seq);
    let mut merged: Vec<LossRange> = Vec::with_capacity(ranges.len());
    for range in ranges.drain(..) {
        if let Some(last) = merged.last_mut() {
            if range.start_seq <= last.end_seq.saturating_add(1) {
                last.end_seq = last.end_seq.max(range.end_seq);
                continue;
            }
        }
        merged.push(range);
    }
    *ranges = merged;
}

fn advance_acked_seq(entry: &mut NodeAckState) -> bool {
    let mut advanced = false;
    loop {
        let next = entry.acked_seq.saturating_add(1);
        if entry.pending.remove(&next) {
            entry.acked_seq = next;
            advanced = true;
            continue;
        }
        if let Some(range_end) = entry
            .loss_ranges
            .iter()
            .find(|range| range.start_seq <= next && next <= range.end_seq)
            .map(|range| range.end_seq)
        {
            if range_end > entry.acked_seq {
                entry.acked_seq = range_end;
                advanced = true;
                // Drop any pending <= acked_seq.
                while entry.pending.first().copied().unwrap_or(u64::MAX) <= entry.acked_seq {
                    let first = *entry.pending.first().unwrap();
                    entry.pending.remove(&first);
                }
                continue;
            }
        }
        break;
    }
    advanced
}

async fn publish_acks(
    client: &AsyncClient,
    config: &Config,
    pool: &PgPool,
    state: &mut HashMap<String, NodeAckState>,
) {
    // Best-effort: coalesce DB writes to the command path; here we only publish.
    for (node_mqtt_id, entry) in state.iter_mut() {
        if !entry.dirty && entry.acked_seq == entry.last_published_acked_seq {
            continue;
        }
        let topic = format!("{}/{}/ack", config.mqtt_topic_prefix, node_mqtt_id);
        let payload = json!({
            "stream_id": entry.stream_id.to_string(),
            "acked_seq": entry.acked_seq,
        });
        match client
            .publish(topic, QoS::AtLeastOnce, false, serde_json::to_vec(&payload).unwrap_or_default())
            .await
        {
            Ok(_) => {
                entry.last_published_acked_seq = entry.acked_seq;
                entry.dirty = false;
                // Best-effort: prune old loss ranges once ACK advances.
                let _ = prune_loss_ranges(pool, node_mqtt_id, entry.stream_id, entry.acked_seq).await;
            }
            Err(err) => {
                tracing::debug!(error=%err, node=%node_mqtt_id, "failed to publish ACK");
                break;
            }
        }
    }
}

async fn persist_ack_state(pool: &PgPool, node_mqtt_id: &str, stream_id: Uuid, acked_seq: u64) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO node_forwarder_ack_state (node_mqtt_id, stream_id, acked_seq, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (node_mqtt_id)
        DO UPDATE SET
            stream_id = EXCLUDED.stream_id,
            acked_seq = GREATEST(node_forwarder_ack_state.acked_seq, EXCLUDED.acked_seq),
            updated_at = NOW()
        "#
    )
    .bind(node_mqtt_id)
    .bind(stream_id)
    .bind(acked_seq as i64)
    .execute(pool)
    .await
    .context("persist ack state")?;
    Ok(())
}

async fn reset_node_state(pool: &PgPool, node_mqtt_id: &str, stream_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO node_forwarder_ack_state (node_mqtt_id, stream_id, acked_seq, updated_at)
        VALUES ($1, $2, 0, NOW())
        ON CONFLICT (node_mqtt_id)
        DO UPDATE SET
            stream_id = EXCLUDED.stream_id,
            acked_seq = 0,
            updated_at = NOW()
        "#
    )
    .bind(node_mqtt_id)
    .bind(stream_id)
    .execute(pool)
    .await
    .context("reset ack state")?;

    sqlx::query(
        r#"
        DELETE FROM node_forwarder_loss_ranges
        WHERE node_mqtt_id = $1
          AND stream_id != $2
        "#
    )
    .bind(node_mqtt_id)
    .bind(stream_id)
    .execute(pool)
    .await
    .ok();
    Ok(())
}

async fn persist_loss_range(
    pool: &PgPool,
    node_mqtt_id: &str,
    stream_id: Uuid,
    start_seq: u64,
    end_seq: u64,
    dropped_at: Option<DateTime<Utc>>,
    reason: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO node_forwarder_loss_ranges (node_mqtt_id, stream_id, start_seq, end_seq, dropped_at, reason)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT DO NOTHING
        "#
    )
    .bind(node_mqtt_id)
    .bind(stream_id)
    .bind(start_seq as i64)
    .bind(end_seq as i64)
    .bind(dropped_at)
    .bind(reason)
    .execute(pool)
    .await
    .context("persist loss range")?;
    Ok(())
}

async fn prune_loss_ranges(pool: &PgPool, node_mqtt_id: &str, stream_id: Uuid, acked_seq: u64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM node_forwarder_loss_ranges
        WHERE node_mqtt_id = $1
          AND stream_id = $2
          AND end_seq <= $3
        "#
    )
    .bind(node_mqtt_id)
    .bind(stream_id)
    .bind(acked_seq as i64)
    .execute(pool)
    .await
    .context("prune loss ranges")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_acked_seq_with_pending_and_losses() {
        let stream_id = Uuid::new_v4();
        let mut state = NodeAckState {
            stream_id,
            acked_seq: 0,
            pending: BTreeSet::new(),
            loss_ranges: vec![LossRange { start_seq: 3, end_seq: 5 }],
            dirty: false,
            last_published_acked_seq: 0,
        };
        state.pending.extend([1, 2, 6, 7]);
        assert!(advance_acked_seq(&mut state));
        assert_eq!(state.acked_seq, 7);
    }

    #[test]
    fn normalizes_loss_ranges() {
        let mut ranges = vec![
            LossRange { start_seq: 10, end_seq: 12 },
            LossRange { start_seq: 1, end_seq: 2 },
            LossRange { start_seq: 3, end_seq: 5 },
            LossRange { start_seq: 5, end_seq: 8 },
        ];
        normalize_loss_ranges(&mut ranges);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start_seq, 1);
        assert_eq!(ranges[0].end_seq, 8);
        assert_eq!(ranges[1].start_seq, 10);
        assert_eq!(ranges[1].end_seq, 12);
    }
}
