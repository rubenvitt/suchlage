//! Broker-Verwaltung über das dynsec-Plugin. Reine Befehls-JSONs in `dynsec`, Versand über `MqttLink`.

pub mod dynsec;

use std::time::Duration;

use serde_json::Value;

use crate::error::AppError;
use crate::mqtt::{Correlation, DYNSEC_TOPIC, MqttLink};

/// Ein Befehl je Anfrage: der Posteingang (B12) ordnet je `correlationData` genau eine Antwort zu,
/// so zählt jeder Fehler.
async fn request(link: &MqttLink, command: Value) -> Result<Value, AppError> {
    let name = command["command"].as_str().unwrap_or_default().to_string();
    let (ids, payload) = dynsec::payload(vec![command]);
    let response = link
        .request(
            DYNSEC_TOPIC.into(),
            payload,
            Correlation::DynSec(ids[0].clone()),
            Duration::from_secs(5),
        )
        .await
        .map_err(|e| AppError::Unavailable(format!("Broker nicht erreichbar: {e}")))?;
    match response["error"].as_str() {
        Some(error) if !dynsec::is_benign(&name, error) => Err(AppError::Unavailable(format!(
            "Broker lehnt {name} ab: {error}"
        ))),
        _ => Ok(response),
    }
}

async fn run(link: &MqttLink, commands: Vec<Value>) -> Result<(), AppError> {
    for command in commands {
        request(link, command).await?;
    }
    Ok(())
}

/// Idempotent: legt an, was fehlt, und weist nur fehlende Rollen zu.
pub async fn ensure_rc(link: &MqttLink, sn: &str, password: &str) -> Result<(), AppError> {
    run(link, dynsec::ensure_rc_commands(sn, password)).await?;
    let client = request(link, dynsec::get_client_command(&dynsec::rc_username(sn))).await?;
    run(link, dynsec::missing_rc_roles(sn, &client)).await
}

pub async fn revoke_rc(link: &MqttLink, sn: &str) -> Result<(), AppError> {
    run(link, dynsec::revoke_rc_commands(sn)).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Beantwortet jede Anfrage mit `answer(befehl)` und liefert die Befehlsnamen in Reihenfolge.
    fn broker(answer: fn(&str) -> Value) -> (MqttLink, tokio::task::JoinHandle<Vec<String>>) {
        let (link, mut rx) = MqttLink::channel();
        let task = tokio::spawn(async move {
            let mut names = vec![];
            while let Some(req) = rx.recv().await {
                let v: Value = serde_json::from_slice(&req.payload).unwrap();
                let name = v["commands"][0]["command"].as_str().unwrap().to_string();
                let _ = req.reply.send(Ok(answer(&name)));
                names.push(name);
            }
            names
        });
        (link, task)
    }

    #[tokio::test]
    async fn repeated_pairing_passes_over_existing_entries_and_assigns_no_role_twice() {
        let (link, task) = broker(|name| match name {
            "createRole" => json!({"error": "Role already exists"}),
            "createClient" => json!({"error": "Client already exists"}),
            "addRoleACL" => json!({"error": "ACL with this topic already exists"}),
            "getClient" => json!({"data": {"client": {"roles": [
                {"rolename": "rc-RC1", "priority": 3},
                {"rolename": dynsec::BLOCK_ROLE, "priority": 2},
                {"rolename": dynsec::DEVICES_ROLE, "priority": 1},
            ]}}}),
            _ => json!({}),
        });
        ensure_rc(&link, "RC1", "pw").await.unwrap();
        drop(link);
        let names = task.await.unwrap();
        assert_eq!(
            names.len(),
            dynsec::ensure_rc_commands("RC1", "pw").len() + 1
        );
        assert_eq!(names.last().map(String::as_str), Some("getClient"));
        assert!(!names.iter().any(|n| n == "addClientRole"), "{names:?}");
        assert!(
            !names.iter().any(|n| n == "setClientPassword"),
            "trennt die RC: {names:?}"
        );
    }

    #[tokio::test]
    async fn real_refusal_stops_and_names_the_command() {
        let (link, task) = broker(|name| match name {
            "addClientRole" => json!({"error": "Role not found"}),
            _ => json!({}),
        });
        let err = ensure_rc(&link, "RC1", "pw").await.unwrap_err();
        assert!(
            matches!(&err, AppError::Unavailable(m) if m == "Broker lehnt addClientRole ab: Role not found"),
            "{err:?}"
        );
        drop(link);
        let names = task.await.unwrap();
        assert_eq!(
            names.iter().filter(|n| *n == "addClientRole").count(),
            1,
            "danach nichts mehr"
        );
    }
}
