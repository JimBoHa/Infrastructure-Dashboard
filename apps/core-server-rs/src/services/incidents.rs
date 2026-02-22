use chrono::{DateTime, Duration, Utc};
use sqlx::{FromRow, Postgres, Transaction};

const INCIDENT_GAP_SECONDS: i64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentStatus {
    Open,
    Snoozed,
    Closed,
}

impl IncidentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Snoozed => "snoozed",
            Self::Closed => "closed",
        }
    }
}

fn severity_rank(value: &str) -> i32 {
    match value.trim().to_lowercase().as_str() {
        "critical" => 0,
        "warning" => 1,
        "info" => 2,
        _ => 99,
    }
}

fn is_resolved_transition(transition: &str) -> bool {
    let trimmed = transition.trim();
    trimmed.eq_ignore_ascii_case("resolved") || trimmed.eq_ignore_ascii_case("ok")
}

fn should_rollover_incident(last_event_at: DateTime<Utc>, now: DateTime<Utc>, transition: &str) -> bool {
    if is_resolved_transition(transition) {
        return false;
    }
    let gap_cutoff = now - Duration::seconds(INCIDENT_GAP_SECONDS.max(60));
    last_event_at < gap_cutoff
}

#[derive(Debug, Clone)]
pub struct IncidentKey {
    pub rule_id: Option<i64>,
    pub target_key: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct IncidentRow {
    id: i64,
    status: String,
    snoozed_until: Option<DateTime<Utc>>,
    severity: String,
    title: String,
    last_event_at: DateTime<Utc>,
}

pub async fn get_or_create_incident(
    tx: &mut Transaction<'_, Postgres>,
    now: DateTime<Utc>,
    key: &IncidentKey,
    severity: &str,
    title: &str,
    transition: &str,
) -> Result<i64, sqlx::Error> {
    let row: Option<IncidentRow> = sqlx::query_as(
        r#"
        SELECT id, status, snoozed_until, severity, title, last_event_at
        FROM incidents
        WHERE rule_id IS NOT DISTINCT FROM $1
          AND target_key IS NOT DISTINCT FROM $2
          AND status <> 'closed'
        ORDER BY last_event_at DESC
        LIMIT 1
        "#,
    )
    .bind(key.rule_id)
    .bind(&key.target_key)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(existing) = row {
        if should_rollover_incident(existing.last_event_at, now, transition) {
            sqlx::query(
                r#"
                UPDATE incidents
                SET
                    status = 'closed',
                    closed_at = $2,
                    updated_at = $2,
                    snoozed_until = NULL
                WHERE id = $1
                "#,
            )
            .bind(existing.id)
            .bind(now)
            .execute(&mut **tx)
            .await?;
        } else {
            let existing_status = existing.status.trim().to_lowercase();
            let keep_snoozed = existing_status == "snoozed"
                && existing
                    .snoozed_until
                    .map(|until| until > now)
                    .unwrap_or(false);
            let next_status = if keep_snoozed {
                IncidentStatus::Snoozed
            } else {
                IncidentStatus::Open
            };
            let next_snoozed_until = if keep_snoozed {
                existing.snoozed_until
            } else {
                None
            };

            let next_severity = if severity_rank(severity) < severity_rank(&existing.severity) {
                severity.trim()
            } else {
                existing.severity.trim()
            };
            let next_title = if existing.title.trim().is_empty() {
                title.trim()
            } else {
                existing.title.trim()
            };

            sqlx::query(
                r#"
                UPDATE incidents
                SET
                    last_event_at = $2,
                    updated_at = $2,
                    status = $3,
                    snoozed_until = $4,
                    severity = $5,
                    title = $6
                WHERE id = $1
                "#,
            )
            .bind(existing.id)
            .bind(now)
            .bind(next_status.as_str())
            .bind(next_snoozed_until)
            .bind(next_severity)
            .bind(next_title)
            .execute(&mut **tx)
            .await?;

            return Ok(existing.id);
        }
    }

    let inserted: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO incidents (
            rule_id,
            target_key,
            severity,
            status,
            title,
            first_event_at,
            last_event_at,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, 'open', $4, $5, $5, $5, $5)
        RETURNING id
        "#,
    )
    .bind(key.rule_id)
    .bind(&key.target_key)
    .bind(severity.trim())
    .bind(title.trim())
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;

    Ok(inserted.0)
}

pub fn parse_incident_status(value: &str) -> Option<IncidentStatus> {
    match value.trim().to_lowercase().as_str() {
        "open" => Some(IncidentStatus::Open),
        "snoozed" => Some(IncidentStatus::Snoozed),
        "closed" => Some(IncidentStatus::Closed),
        _ => None,
    }
}

pub fn parse_severity(value: &str) -> Option<&'static str> {
    match value.trim().to_lowercase().as_str() {
        "critical" => Some("critical"),
        "warning" => Some("warning"),
        "info" => Some("info"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn should_rollover_incident_rolls_after_gap_for_non_resolved_transitions() {
        let now = Utc.with_ymd_and_hms(2026, 2, 11, 12, 0, 0).unwrap();

        let recent = now - Duration::minutes(5);
        assert!(!should_rollover_incident(recent, now, "fired"));

        let stale = now - Duration::hours(2);
        assert!(should_rollover_incident(stale, now, "fired"));

        assert!(!should_rollover_incident(stale, now, "resolved"));
        assert!(!should_rollover_incident(stale, now, "OK"));
    }
}
