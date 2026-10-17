//! Anfrage/Antwort-Kanal zur einen MQTT-Sitzung des Servers. Kennt weder DJI noch dynsec-Befehle;
//! die Zuordnung der Antworten macht der Posteingang (`dji::link`).

use std::time::Duration;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

pub const DYNSEC_TOPIC: &str = "$CONTROL/dynamic-security/v1";
pub const DYNSEC_RESPONSE_TOPIC: &str = "$CONTROL/dynamic-security/v1/response";

/// Woran der Posteingang die Antwort erkennt: `tid` bei `services_reply`, `correlationData` bei dynsec.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Correlation {
    Tid(String),
    DynSec(String),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LinkError {
    #[error("Broker nicht verbunden")]
    NotConnected,
    #[error("keine Antwort innerhalb der Frist")]
    Timeout,
    #[error("MQTT-Sitzung beendet")]
    Closed,
}

pub type Reply = oneshot::Sender<Result<Value, LinkError>>;

pub struct LinkRequest {
    pub topic: String,
    pub payload: Vec<u8>,
    pub correlation: Correlation,
    pub reply: Reply,
}

#[derive(Clone)]
pub struct MqttLink {
    tx: mpsc::Sender<LinkRequest>,
}

impl MqttLink {
    pub fn channel() -> (MqttLink, mpsc::Receiver<LinkRequest>) {
        let (tx, rx) = mpsc::channel(64);
        (MqttLink { tx }, rx)
    }

    /// Veröffentlicht `payload` und wartet auf die Antwort mit derselben Korrelation.
    pub async fn request(
        &self,
        topic: String,
        payload: Vec<u8>,
        correlation: Correlation,
        timeout: Duration,
    ) -> Result<Value, LinkError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LinkRequest {
                topic,
                payload,
                correlation,
                reply,
            })
            .await
            .map_err(|_| LinkError::Closed)?;
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LinkError::Closed),
            Err(_) => Err(LinkError::Timeout),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn request_times_out_without_answer() {
        let (link, mut rx) = MqttLink::channel();
        tokio::spawn(async move {
            let _keep = rx.recv().await; // angenommen, nie beantwortet
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let r = link
            .request(
                "t".into(),
                vec![],
                Correlation::Tid("x".into()),
                Duration::from_millis(50),
            )
            .await;
        assert_eq!(r, Err(LinkError::Timeout));
    }

    #[tokio::test]
    async fn request_without_session_is_closed() {
        let (link, rx) = MqttLink::channel();
        drop(rx);
        let r = link
            .request(
                "t".into(),
                vec![],
                Correlation::Tid("x".into()),
                Duration::from_secs(1),
            )
            .await;
        assert_eq!(r, Err(LinkError::Closed));
    }

    #[tokio::test]
    async fn answer_reaches_the_caller() {
        let (link, mut rx) = MqttLink::channel();
        tokio::spawn(async move {
            let req = rx.recv().await.unwrap();
            assert_eq!(
                (req.topic.as_str(), req.correlation),
                ("t", Correlation::DynSec("c1".into()))
            );
            let _ = req.reply.send(Ok(serde_json::json!({"ok": true})));
        });
        let r = link
            .request(
                "t".into(),
                b"{}".to_vec(),
                Correlation::DynSec("c1".into()),
                Duration::from_secs(1),
            )
            .await;
        assert_eq!(r, Ok(serde_json::json!({"ok": true})));
    }
}
