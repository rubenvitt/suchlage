//! Livebild-Befehle an Pilot 2 mit Domänenargumenten. Außerhalb von `dji/` sieht niemand
//! Methodennamen oder Ergebniscodes.

use std::time::{Duration, Instant};

use crate::contract::{DeviceLogLevel, Lens};
use crate::devlog::DeviceLog;
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
    devlog: DeviceLog,
}

impl LiveControl {
    pub fn new(link: MqttLink, devlog: DeviceLog) -> Self {
        Self { link, devlog }
    }

    async fn call(
        &self,
        gateway_sn: &str,
        method: &str,
        data: serde_json::Value,
    ) -> Result<(), ControlError> {
        let (tid, payload) = command::envelope(method, data, now_ms());
        let started = Instant::now();
        let reply = self
            .link
            .request(
                topic::services(gateway_sn),
                payload,
                Correlation::Tid(tid.clone()),
                TIMEOUT,
            )
            .await;
        let ms = started.elapsed().as_millis() as u64;
        let result = match reply {
            Ok(reply) => match reply["result"].as_i64() {
                Some(0) => Ok(()),
                Some(code) => {
                    tracing::warn!(
                        gateway = gateway_sn,
                        method,
                        tid,
                        code,
                        ms,
                        "Befehl abgelehnt"
                    );
                    Err(ControlError::Refused(live_error_text(code)))
                }
                None => Err(ControlError::Refused(
                    "Pilot 2 antwortet ohne Ergebnis.".into(),
                )),
            },
            Err(LinkError::Timeout) => {
                tracing::warn!(gateway = gateway_sn, method, tid, ms, "Befehl ohne Antwort");
                self.devlog.system(
                    DeviceLogLevel::Error,
                    Some(gateway_sn),
                    format!(
                        "{method}: keine Antwort in {} s · tid {}",
                        TIMEOUT.as_secs(),
                        short(&tid)
                    ),
                );
                Err(ControlError::Refused(format!(
                    "Pilot 2 antwortet nicht (keine Antwort in {} s). Ist die Flugansicht offen? \
                     Läuft dort schon ein manuell gestartetes Livebild?",
                    TIMEOUT.as_secs()
                )))
            }
            Err(e @ (LinkError::NotConnected | LinkError::Closed)) => {
                tracing::warn!(gateway = gateway_sn, method, error = %e, "Befehl nicht gesendet");
                self.devlog.system(
                    DeviceLogLevel::Error,
                    Some(gateway_sn),
                    format!("{method} nicht gesendet: {e}"),
                );
                Err(ControlError::Unreachable(format!(
                    "Befehl nicht gesendet: {e}."
                )))
            }
        };
        if result.is_ok() {
            tracing::info!(gateway = gateway_sn, method, tid, ms, "Befehl bestätigt");
        }
        result
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

fn short(tid: &str) -> String {
    tid.chars().take(8).collect()
}
