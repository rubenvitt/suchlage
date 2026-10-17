//! MQTT-Sitzung des Servers (rumqttc, ohne TLS). Verbindet mit den Broker-Einstellungen,
//! verbindet nach Fehlern selbst neu und baut die Verbindung bei geänderten Einstellungen neu auf.
//! Kennt `runtime` nicht: Observations gehen über einen einfachen Kanal hinaus.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use sqlx::SqlitePool;
use tokio::sync::{mpsc, watch};

use super::link::Inbox;
use super::record::RecordLine;
use super::recorder::Recorder;
use super::topic::SUBSCRIPTIONS;
use crate::broker::dynsec;
use crate::contract::BrokerStatus;
use crate::mqtt::{DYNSEC_RESPONSE_TOPIC, DYNSEC_TOPIC, LinkError, LinkRequest};
use crate::settings::{BrokerSettings, Settings};
use crate::situation::observation::{Observation, Observed};
use crate::store;
use crate::time::now_ms;

const CLIENT_ID: &str = "suchlage-server";
const REQUEST_QUEUE: usize = 256;
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

async fn report(observer: &mpsc::Sender<Observed>, observation: Observation) {
    let _ = observer
        .send(Observed {
            at_ms: now_ms(),
            observation,
        })
        .await;
}

/// Öffnet oder schließt den Rohmitschnitt nach `Settings.general.record_raw_mqtt`.
async fn set_recording(
    slot: &mut Option<Recorder>,
    enabled: bool,
    dir: &Path,
    observer: &mpsc::Sender<Observed>,
) {
    match (enabled, slot.is_some()) {
        (true, false) => match Recorder::open(dir, now_ms()) {
            Ok(r) => {
                report(
                    observer,
                    Observation::RawRecording(Some(r.file_name().to_string())),
                )
                .await;
                *slot = Some(r);
            }
            Err(e) => tracing::error!(error = %e, "Rohmitschnitt nicht möglich"),
        },
        (false, true) => {
            *slot = None;
            report(observer, Observation::RawRecording(None)).await;
        }
        _ => {}
    }
}

/// Posteingang, aus `device_state` wiederhergestellt; die Observations gehen an den Kern. Nur beim Start.
async fn restored_inbox(pool: &SqlitePool, observer: &mpsc::Sender<Observed>) -> Inbox {
    let mut inbox = Inbox::new();
    match store::device_state::all(pool).await {
        Ok(devices) => {
            for o in inbox.restore(&devices) {
                let _ = observer.send(o).await;
            }
        }
        Err(e) => tracing::warn!(error = %e, "Gerätestand nicht lesbar – startet leer"),
    }
    inbox
}

/// Was die Verbindung bestimmt. `rc_url` geht nur an die Fernsteuerungen und erzwingt keinen
/// Neuaufbau, der wartende Anfragen abbräche.
fn connection(broker: &Option<BrokerSettings>) -> Option<(&str, u16, &str, &str)> {
    broker.as_ref().map(|b| {
        (
            b.host.as_str(),
            b.port,
            b.admin_username.as_str(),
            b.admin_password.as_str(),
        )
    })
}

fn options(broker: &BrokerSettings) -> MqttOptions {
    let mut options = MqttOptions::new(CLIENT_ID, broker.host.clone(), broker.port);
    options.set_credentials(broker.admin_username.clone(), broker.admin_password.clone());
    options.set_keep_alive(Duration::from_secs(30));
    options.set_max_packet_size(1 << 20, 1 << 20);
    options
}

/// Ohne Broker: beantwortet Anfragen sofort mit `NotConnected`, bis sich die Einstellungen ändern
/// (`true`) oder die Sitzung endet (`false`).
async fn wait_for_settings(
    settings: &mut watch::Receiver<Settings>,
    requests: &mut mpsc::Receiver<LinkRequest>,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    loop {
        tokio::select! {
            changed = settings.changed() => return changed.is_ok(),
            Some(req) = requests.recv() => { let _ = req.reply.send(Err(LinkError::NotConnected)); }
            _ = shutdown.changed() => return false,
        }
    }
}

pub async fn run(
    mut settings: watch::Receiver<Settings>,
    observer: mpsc::Sender<Observed>,
    pool: SqlitePool,
    records_dir: PathBuf,
    mut requests: mpsc::Receiver<LinkRequest>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut recorder: Option<Recorder> = None;
    // Einmal je Prozess: über Neuverbindungen hinweg bleibt der Merge-Stand des Übersetzers erhalten.
    let mut inbox = restored_inbox(&pool, &observer).await;
    loop {
        let current = settings.borrow_and_update().clone();
        set_recording(
            &mut recorder,
            current.general.record_raw_mqtt,
            &records_dir,
            &observer,
        )
        .await;
        let Some(broker) = current.broker.clone() else {
            report(&observer, Observation::Broker(BrokerStatus::NotConfigured)).await;
            if wait_for_settings(&mut settings, &mut requests, &mut shutdown).await {
                continue;
            }
            return;
        };

        report(&observer, Observation::Broker(BrokerStatus::Connecting)).await;
        let (client, mut eventloop) = AsyncClient::new(options(&broker), REQUEST_QUEUE);
        let mut connected = false;

        loop {
            tokio::select! {
                event = eventloop.poll() => match event {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        connected = true;
                        // try_*: in diesem Zweig nie auf die eigene Warteschlange warten (rumqttc-Falle).
                        for t in SUBSCRIPTIONS.iter().copied().chain([DYNSEC_RESPONSE_TOPIC]) {
                            if let Err(e) = client.try_subscribe(t, QoS::AtLeastOnce) {
                                tracing::error!(error = %e, topic = t, "Abonnieren fehlgeschlagen");
                            }
                        }
                        // Ohne diese Rolle darf der Admin-Client weder `status_reply` noch `services` senden.
                        let (_, payload) = dynsec::payload(dynsec::server_role_commands(&broker.admin_username));
                        if let Err(e) = client.try_publish(DYNSEC_TOPIC, QoS::AtLeastOnce, false, payload) {
                            tracing::error!(error = %e, "Serverrolle nicht gesetzt");
                        }
                        report(&observer, Observation::Broker(BrokerStatus::Connected)).await;
                    }
                    Ok(Event::Incoming(Packet::Publish(p))) => {
                        let at = now_ms();
                        if let Some(r) = recorder.as_mut()
                            && let Err(e) = r.write(&RecordLine::new(at, &p.topic, &p.payload))
                        {
                            tracing::warn!(error = %e, "Rohmitschnitt: Schreiben fehlgeschlagen");
                        }
                        let out = inbox.on_publish(at, &p.topic, &p.payload);
                        for o in out.observations {
                            let _ = observer.send(o).await;
                        }
                        for msg in out.publish {
                            if let Err(e) = client.try_publish(msg.topic, QoS::AtLeastOnce, false, msg.payload) {
                                tracing::warn!(error = %e, "Antwort nicht gesendet");
                            }
                        }
                        // Wartet kurz auf SQLite, nicht auf rumqttc; state kommt nur bei Änderung.
                        if let Some((sn, gateway_sn, data)) = out.persist
                            && let Err(e) = store::device_state::put(&pool, &sn, &gateway_sn, &data, at).await
                        {
                            tracing::warn!(error = %e, sn, "Gerätestand nicht gespeichert");
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        if connected {
                            tracing::warn!(error = %e, "MQTT-Verbindung verloren");
                        }
                        connected = false;
                        inbox.fail_all(LinkError::NotConnected);
                        report(&observer, Observation::Broker(BrokerStatus::Disconnected)).await;
                        // Das nächste poll() verbindet neu.
                        tokio::time::sleep(RECONNECT_DELAY).await;
                    }
                },
                Some(req) = requests.recv() => {
                    if !connected {
                        let _ = req.reply.send(Err(LinkError::NotConnected));
                        continue;
                    }
                    match client.try_publish(req.topic, QoS::AtLeastOnce, false, req.payload) {
                        // Die Antwort kann erst im nächsten poll() kommen, also nach dem Eintragen.
                        Ok(()) => inbox.register(req.correlation, req.reply),
                        Err(e) => {
                            tracing::warn!(error = %e, "Befehl nicht gesendet");
                            let _ = req.reply.send(Err(LinkError::NotConnected));
                        }
                    }
                }
                changed = settings.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    let next = settings.borrow_and_update().clone();
                    set_recording(&mut recorder, next.general.record_raw_mqtt, &records_dir, &observer).await;
                    if connection(&next.broker) != connection(&current.broker) {
                        let _ = client.try_disconnect();
                        inbox.fail_all(LinkError::NotConnected);
                        break;
                    }
                }
                _ = shutdown.changed() => {
                    let _ = client.try_disconnect();
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::GeneralSettings;
    use crate::mqtt::{Correlation, MqttLink};
    use crate::settings::SettingsHandle;
    use serde_json::json;

    async fn next(rx: &mut mpsc::Receiver<Observed>) -> Observed {
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Observation kommt")
            .expect("Kanal offen")
    }

    /// Ohne Broker: Gerätestand und Mitschnitt kommen trotzdem, Anfragen scheitern sofort,
    /// und das Abschalten des Mitschnitts sowie das Herunterfahren wirken.
    #[tokio::test]
    async fn without_broker_restores_records_and_fails_requests() {
        let pool = crate::db::test_pool().await;
        let cap = json!({"live_capacity": {"device_list": [{"sn": "AC1", "camera_list": [{"camera_index": "61-0-0", "video_list": [{"video_index": "normal-0", "video_type": "wide"}]}]}]}});
        store::device_state::put(&pool, "RC1", "RC1", &cap, 5)
            .await
            .unwrap();
        let settings = SettingsHandle::load(pool.clone()).await.unwrap();
        let (observer, mut observed) = mpsc::channel(16);
        let (link, requests) = MqttLink::channel();
        let (stop, stop_rx) = watch::channel(false);
        let dir = tempfile::tempdir().unwrap();
        let task = tokio::spawn(run(
            settings.subscribe(),
            observer,
            pool,
            dir.path().to_path_buf(),
            requests,
            stop_rx,
        ));

        let mut restored = Vec::new();
        let name = loop {
            match next(&mut observed).await {
                Observed {
                    observation: Observation::RawRecording(Some(name)),
                    ..
                } => break name,
                o => restored.push(o),
            }
        };
        assert!(restored.iter().any(|o| matches!(&o.observation, Observation::VideoSources { gateway_sn, .. } if gateway_sn == "RC1")));
        assert!(
            restored.iter().all(|o| o.at_ms == 5),
            "mit der gespeicherten Empfangszeit"
        );
        assert!(dir.path().join(&name).exists());
        assert_eq!(
            next(&mut observed).await.observation,
            Observation::Broker(BrokerStatus::NotConfigured)
        );

        let reply = link
            .request(
                "t".into(),
                vec![],
                Correlation::Tid("x".into()),
                Duration::from_secs(1),
            )
            .await;
        assert_eq!(reply, Err(LinkError::NotConnected));

        settings
            .update_general(GeneralSettings {
                auto_live_on_takeoff: true,
                record_raw_mqtt: false,
            })
            .await
            .unwrap();
        assert_eq!(
            next(&mut observed).await.observation,
            Observation::RawRecording(None)
        );

        stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("Sitzung endet mit dem Shutdown-Signal")
            .unwrap();
    }
}
