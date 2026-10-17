//! Posteingang der MQTT-Sitzung: übersetzt, beantwortet update_topo, ordnet Antworten
//! (services_reply per `tid`, dynsec per `correlationData`) wartenden Anfragen zu und sagt, welcher
//! Gerätestand zu speichern ist. Rein bis auf die oneshot-Kanäle – testbar ohne Broker.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::topic::{self, DjiTopic};
use super::translate::{Outgoing, Translator};
use crate::mqtt::{Correlation, DYNSEC_RESPONSE_TOPIC, LinkError, Reply};
use crate::situation::observation::Observed;
use crate::store::device_state::StoredDevice;

#[derive(Debug, Default)]
pub struct InboxOutput {
    pub observations: Vec<Observed>,
    pub publish: Vec<Outgoing>,
    /// (Geräte-SN, Gateway-SN, gemergter Stand) nach einer state-Nachricht.
    pub persist: Option<(String, String, Value)>,
}

#[derive(Default)]
pub struct Inbox {
    translator: Translator,
    pending: HashMap<Correlation, Reply>,
}

impl Inbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spielt gespeicherte Gerätestände ein (einmal beim Start). Liefert die Observations dazu.
    pub fn restore(&mut self, devices: &[StoredDevice]) -> Vec<Observed> {
        let mut out = Vec::new();
        for d in devices {
            let Some(data) = d.data.as_object() else {
                continue;
            };
            out.extend(
                self.translator
                    .restore(d.updated_at_ms, &d.sn, &d.gateway_sn, data.clone())
                    .observations,
            );
        }
        out
    }

    /// Merkt eine wartende Anfrage vor. Aufgegebene Anfragen (Frist abgelaufen) fallen dabei weg.
    pub fn register(&mut self, correlation: Correlation, reply: Reply) {
        self.pending.retain(|_, r| !r.is_closed());
        self.pending.insert(correlation, reply);
    }

    pub fn on_publish(&mut self, at_ms: i64, topic: &str, payload: &[u8]) -> InboxOutput {
        let mut out = InboxOutput::default();
        if topic == DYNSEC_RESPONSE_TOPIC {
            self.on_dynsec_response(payload);
            return out;
        }
        let translated = self.translator.handle(at_ms, topic, payload);
        if let Some(cmd) = translated.command_reply
            && let Some(reply) = self.pending.remove(&Correlation::Tid(cmd.tid))
        {
            let _ = reply.send(Ok(json!({ "method": cmd.method, "result": cmd.result })));
        }
        if let DjiTopic::State { sn } = topic::parse(topic)
            && let Some((gateway_sn, data)) = self.translator.persisted(sn)
        {
            out.persist = Some((sn.to_string(), gateway_sn, Value::Object(data)));
        }
        out.observations = translated.observations;
        out.publish.extend(translated.reply);
        out
    }

    /// Ein dynsec-Antwortobjekt kann mehrere Kommandos beantworten; jedes trägt seine `correlationData`.
    fn on_dynsec_response(&mut self, payload: &[u8]) {
        let Ok(body) = serde_json::from_slice::<Value>(payload) else {
            tracing::warn!("dynsec-Antwort nicht lesbar");
            return;
        };
        for response in body["responses"].as_array().into_iter().flatten() {
            if let Some(id) = response["correlationData"].as_str()
                && let Some(reply) = self.pending.remove(&Correlation::DynSec(id.to_string()))
            {
                let _ = reply.send(Ok(response.clone()));
            }
        }
    }

    pub fn fail_all(&mut self, error: LinkError) {
        for (_, reply) in self.pending.drain() {
            let _ = reply.send(Err(error.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::situation::observation::Observation;

    use tokio::sync::oneshot;

    #[test]
    fn services_reply_resolves_the_waiting_request() {
        let mut inbox = Inbox::new();
        let (tx, mut rx) = oneshot::channel();
        inbox.register(Correlation::Tid("t1".into()), tx);
        inbox.on_publish(
            0,
            "thing/product/RC1/services_reply",
            json!({"tid": "t1", "bid": "b", "method": "live_start_push", "data": {"result": 0}})
                .to_string()
                .as_bytes(),
        );
        assert_eq!(
            rx.try_recv().unwrap().unwrap(),
            json!({"method": "live_start_push", "result": 0})
        );
    }

    #[test]
    fn reply_for_someone_else_leaves_the_request_waiting() {
        let mut inbox = Inbox::new();
        let (tx, mut rx) = oneshot::channel();
        inbox.register(Correlation::Tid("t1".into()), tx);
        let other =
            json!({"tid": "t2", "bid": "b", "method": "live_start_push", "data": {"result": 0}});
        inbox.on_publish(
            0,
            "thing/product/RC1/services_reply",
            other.to_string().as_bytes(),
        );
        let dynsec_with_same_id =
            json!({"responses": [{"command": "createClient", "correlationData": "t1"}]});
        inbox.on_publish(
            0,
            DYNSEC_RESPONSE_TOPIC,
            dynsec_with_same_id.to_string().as_bytes(),
        );
        assert_eq!(rx.try_recv(), Err(oneshot::error::TryRecvError::Empty));
    }

    #[test]
    fn dynsec_response_resolves_by_correlation_data() {
        let mut inbox = Inbox::new();
        let (tx, mut rx) = oneshot::channel();
        inbox.register(Correlation::DynSec("c7".into()), tx);
        let body = json!({"responses": [{"command": "createClient", "correlationData": "c7"}]});
        let out = inbox.on_publish(0, DYNSEC_RESPONSE_TOPIC, body.to_string().as_bytes());
        assert!(out.observations.is_empty());
        assert_eq!(rx.try_recv().unwrap().unwrap()["command"], "createClient");
    }

    #[test]
    fn dynsec_error_is_passed_on_as_value() {
        let mut inbox = Inbox::new();
        let (tx, mut rx) = oneshot::channel();
        inbox.register(Correlation::DynSec("c8".into()), tx);
        let body = json!({"responses": [{"command": "createRole", "error": "Role already exists", "correlationData": "c8"}]});
        inbox.on_publish(0, DYNSEC_RESPONSE_TOPIC, body.to_string().as_bytes());
        assert_eq!(
            rx.try_recv().unwrap().unwrap()["error"],
            "Role already exists"
        );
    }

    #[test]
    fn topology_is_answered_and_observed() {
        let mut inbox = Inbox::new();
        let out = inbox.on_publish(
            0,
            "sys/product/RC1/status",
            json!({"tid": "t", "bid": "b", "method": "update_topo", "data": {"sub_devices": []}})
                .to_string()
                .as_bytes(),
        );
        assert_eq!(out.publish[0].topic, "sys/product/RC1/status_reply");
        assert_eq!(out.observations.len(), 1);
        assert!(out.persist.is_none(), "nur state wird gespeichert");
    }

    #[test]
    fn osd_is_observed_but_not_persisted() {
        let mut inbox = Inbox::new();
        let osd = json!({"gateway": "RC1", "data": {"latitude": 52.0, "longitude": 10.0}});
        let out = inbox.on_publish(1, "thing/product/AC1/osd", osd.to_string().as_bytes());
        assert!(!out.observations.is_empty());
        assert!(
            out.persist.is_none(),
            "osd kommt periodisch und wird nicht gespeichert"
        );
    }

    #[test]
    fn disconnect_fails_everything_waiting() {
        let mut inbox = Inbox::new();
        let (tx, mut rx) = oneshot::channel();
        inbox.register(Correlation::Tid("t".into()), tx);
        inbox.fail_all(LinkError::NotConnected);
        assert_eq!(rx.try_recv().unwrap(), Err(LinkError::NotConnected));
    }

    /// Review Focus 3: nach einem Neustart sind Videoquellen und Startpunkt wieder da.
    #[test]
    fn restored_inbox_knows_video_sources_and_home() {
        let mut first = Inbox::new();
        let cap = json!({"gateway": "RC1", "data": {"live_capacity": {"device_list": [{"sn": "AC1", "camera_list": [{"camera_index": "61-0-0", "video_list": [{"video_index": "normal-0", "video_type": "wide"}]}]}]}}});
        let home =
            json!({"gateway": "RC1", "data": {"home_latitude": 52.0, "home_longitude": 10.0}});
        let a = first
            .on_publish(1, "thing/product/RC1/state", cap.to_string().as_bytes())
            .persist
            .unwrap();
        let b = first
            .on_publish(2, "thing/product/AC1/state", home.to_string().as_bytes())
            .persist
            .unwrap();
        let stored: Vec<StoredDevice> = [(a, 1), (b, 2)]
            .into_iter()
            .map(|((sn, gateway_sn, data), at)| StoredDevice {
                sn,
                gateway_sn,
                data,
                updated_at_ms: at,
            })
            .collect();

        let observations = Inbox::new().restore(&stored);
        assert!(observations.iter().any(|o| matches!(&o.observation, Observation::VideoSources { gateway_sn, sources } if gateway_sn == "RC1" && sources.len() == 1)));
        assert!(observations.iter().any(|o| matches!(&o.observation, Observation::AircraftTelemetry { telemetry, periodic: false, .. } if telemetry.home.is_some())));
        assert!(
            observations.iter().all(|o| o.at_ms <= 2),
            "mit der alten Empfangszeit: die Frische zeigt das wahre Alter"
        );
    }
}
