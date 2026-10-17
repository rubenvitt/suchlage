//! Livebild-Befehle an Pilot 2 mit Domänenargumenten. Außerhalb von `dji/` sieht niemand
//! Methodennamen oder Ergebniscodes.

use std::time::Duration;

use crate::contract::Lens;
use crate::mqtt::{Correlation, LinkError, MqttLink};
use crate::time::now_ms;

use super::command;
use super::topic;
use super::translate::live_error_text;

const TIMEOUT: Duration = Duration::from_secs(10);

/// Warum ein Befehl nicht durchging; der Text ist ein deutscher Satz für die Oberfläche.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ControlError {
    /// Pilot 2 hat abgelehnt oder nicht geantwortet.
    #[error("{0}")]
    Refused(String),
    /// Der Befehl hat den Broker nicht erreicht.
    #[error("{0}")]
    Unreachable(String),
}

#[derive(Clone)]
pub struct LiveControl {
    link: MqttLink,
}

impl LiveControl {
    pub fn new(link: MqttLink) -> Self {
        Self { link }
    }

    async fn call(
        &self,
        gateway_sn: &str,
        method: &str,
        data: serde_json::Value,
    ) -> Result<(), ControlError> {
        let (tid, payload) = command::envelope(method, data, now_ms());
        let reply = self
            .link
            .request(
                topic::services(gateway_sn),
                payload,
                Correlation::Tid(tid),
                TIMEOUT,
            )
            .await;
        match reply {
            Ok(reply) => match reply["result"].as_i64() {
                Some(0) => Ok(()),
                Some(code) => Err(ControlError::Refused(live_error_text(code))),
                None => Err(ControlError::Refused(
                    "Pilot 2 antwortet ohne Ergebnis.".into(),
                )),
            },
            Err(LinkError::Timeout) => Err(ControlError::Refused(
                "Pilot 2 antwortet nicht (keine Antwort in 10 s).".into(),
            )),
            Err(e @ (LinkError::NotConnected | LinkError::Closed)) => Err(
                ControlError::Unreachable(format!("Befehl nicht gesendet: {e}.")),
            ),
        }
    }

    pub async fn start(
        &self,
        gateway_sn: &str,
        video_id: &str,
        rtmp_url: &str,
    ) -> Result<(), ControlError> {
        self.call(
            gateway_sn,
            "live_start_push",
            command::live_start_push(video_id, rtmp_url),
        )
        .await
    }

    pub async fn stop(&self, gateway_sn: &str, video_id: &str) -> Result<(), ControlError> {
        self.call(
            gateway_sn,
            "live_stop_push",
            command::live_stop_push(video_id),
        )
        .await
    }

    pub async fn change_lens(
        &self,
        gateway_sn: &str,
        video_id: &str,
        lens: Lens,
    ) -> Result<(), ControlError> {
        self.call(
            gateway_sn,
            "live_lens_change",
            command::live_lens_change(video_id, lens),
        )
        .await
    }
}
