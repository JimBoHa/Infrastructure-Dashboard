use anyhow::Result;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::Value as JsonValue;
use std::time::Duration;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct MqttPublisher {
    client: AsyncClient,
}

impl MqttPublisher {
    pub fn new(
        client_id: &str,
        host: &str,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<(Self, JoinHandle<()>)> {
        let mut options = MqttOptions::new(client_id, host, port);
        options.set_keep_alive(Duration::from_secs(10));
        if let (Some(username), Some(password)) = (username, password) {
            options.set_credentials(username, password);
        }
        let (client, mut eventloop) = AsyncClient::new(options, 10);
        let handle = tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::warn!(error = %err, "mqtt event loop error");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });
        Ok((Self { client }, handle))
    }

    pub async fn publish_json(&self, topic: &str, payload: &JsonValue) -> Result<()> {
        let bytes = serde_json::to_vec(payload)?;
        self.client
            .publish(topic, QoS::AtLeastOnce, false, bytes)
            .await?;
        Ok(())
    }
}
