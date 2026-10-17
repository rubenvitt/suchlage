//! Befehle für das dynamic-security-Plugin von Mosquitto 2.0.22 (ADR 0003). `%u`/`%c` werden in
//! dieser Version nicht ersetzt (geprüft 16.10.2026) – deshalb eine Rolle je Fernsteuerung.

use serde_json::{Value, json};

pub fn rc_username(sn: &str) -> String {
    format!("rc-{sn}")
}

/// Erlaubt Fluggeräte-Telemetrie mit vorher unbekannter SN (Priorität 1).
pub const DEVICES_ROLE: &str = "suchlage-rc-geraete";
/// Verbietet die Topics aller gekoppelten RCs (Priorität 2), damit `+/osd|state` keine fremde RC trifft.
pub const BLOCK_ROLE: &str = "suchlage-rc-sperre";
/// Rolle des Admin-Clients: Die `dynsec init`-Rolle `admin` erlaubt kein Senden auf beliebige Topics.
const SERVER_ROLE: &str = "suchlage-server";

fn acl(role: &str, acltype: &str, topic: String, allow: bool) -> Value {
    json!({ "command": "addRoleACL", "rolename": role, "acltype": acltype, "topic": topic, "allow": allow, "priority": 0 })
}

fn client_role(username: &str, role: &str, priority: i32) -> Value {
    json!({ "command": "addClientRole", "username": username, "rolename": role, "priority": priority })
}

/// Rollen der Fernsteuerung mit Priorität: eigene vor Sperre vor Platzhalter.
fn rc_roles(sn: &str) -> [(String, i32); 3] {
    [
        (rc_username(sn), 3),
        (BLOCK_ROLE.to_string(), 2),
        (DEVICES_ROLE.to_string(), 1),
    ]
}

/// Rollen, ACLs und Client. Die Zuordnung der Rollen folgt erst nach `getClient`
/// (`missing_rc_roles`): 2.0.22 beantwortet ein doppeltes `addClientRole` mit „Internal error“.
pub fn ensure_rc_commands(sn: &str, password: &str) -> Vec<Value> {
    let name = rc_username(sn);
    let own =
        |acltype: &str, root: &str| acl(&name, acltype, format!("{root}/product/{sn}/#"), true);
    vec![
        json!({ "command": "createRole", "rolename": name }),
        own("publishClientSend", "sys"),
        own("publishClientSend", "thing"),
        own("subscribePattern", "sys"),
        own("subscribePattern", "thing"),
        json!({ "command": "createRole", "rolename": DEVICES_ROLE }),
        acl(
            DEVICES_ROLE,
            "publishClientSend",
            "thing/product/+/osd".into(),
            true,
        ),
        acl(
            DEVICES_ROLE,
            "publishClientSend",
            "thing/product/+/state".into(),
            true,
        ),
        json!({ "command": "createRole", "rolename": BLOCK_ROLE }),
        acl(
            BLOCK_ROLE,
            "publishClientSend",
            format!("thing/product/{sn}/#"),
            false,
        ),
        json!({ "command": "createClient", "username": name, "password": password }),
        json!({ "command": "setClientPassword", "username": name, "password": password }),
    ]
}

pub fn get_client_command(username: &str) -> Value {
    json!({ "command": "getClient", "username": username })
}

/// `addClientRole` nur für die Rollen, die laut `getClient`-Antwort fehlen. So trennt ein
/// wiederholtes Koppeln die Fernsteuerung nicht: Jedes erfolgreiche `addClientRole` wirft den
/// verbundenen Client in 2.0.22 hinaus, und Pilot 2 hält seine MQTT-Verbindung auch ohne die Seite.
pub fn missing_rc_roles(sn: &str, get_client: &Value) -> Vec<Value> {
    let assigned: Vec<&str> = get_client["data"]["client"]["roles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["rolename"].as_str())
        .collect();
    let name = rc_username(sn);
    rc_roles(sn)
        .into_iter()
        .filter(|(role, _)| !assigned.contains(&role.as_str()))
        .map(|(role, priority)| client_role(&name, &role, priority))
        .collect()
}

pub fn revoke_rc_commands(sn: &str) -> Vec<Value> {
    let name = rc_username(sn);
    vec![
        json!({ "command": "deleteClient", "username": name }),
        json!({ "command": "deleteRole", "rolename": name }),
        json!({ "command": "removeRoleACL", "rolename": BLOCK_ROLE, "acltype": "publishClientSend", "topic": format!("thing/product/{sn}/#") }),
    ]
}

/// Bei jeder Verbindung ohne Blick auf die Antworten gesendet. Nur das allererste `addClientRole`
/// wirkt (und trennt den Server einmal); jedes weitere scheitert folgenlos am Doppel.
pub fn server_role_commands(admin_username: &str) -> Vec<Value> {
    vec![
        json!({ "command": "createRole", "rolename": SERVER_ROLE }),
        acl(SERVER_ROLE, "publishClientSend", "#".into(), true),
        acl(SERVER_ROLE, "subscribePattern", "#".into(), true),
        client_role(admin_username, SERVER_ROLE, 0),
    ]
}

/// Hängt jedem Befehl eine `correlationData` an und baut die Nutzlast.
pub fn payload(mut commands: Vec<Value>) -> (Vec<String>, Vec<u8>) {
    let ids: Vec<String> = commands
        .iter_mut()
        .map(|c| {
            let id = uuid::Uuid::new_v4().to_string();
            c["correlationData"] = json!(id);
            id
        })
        .collect();
    (
        ids,
        json!({ "commands": commands }).to_string().into_bytes(),
    )
}

/// Fehler, die bei wiederholtem, idempotentem Anlegen oder Abbauen erwartbar sind: Anlegen trifft
/// auf Bestehendes, Abbauen auf schon Entferntes. Ein „not found“ beim Anlegen bleibt ein Fehler.
pub fn is_benign(command: &str, error: &str) -> bool {
    let error = error.to_lowercase();
    let creates = command.starts_with("create") || command.starts_with("add");
    let removes = command.starts_with("delete") || command.starts_with("remove");
    (creates && error.contains("already exists")) || (removes && error.contains("not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acls(cmds: &[Value]) -> Vec<(&str, &str, &str)> {
        cmds.iter()
            .filter(|c| c["command"] == "addRoleACL")
            .map(|c| {
                (
                    c["rolename"].as_str().unwrap(),
                    c["acltype"].as_str().unwrap(),
                    c["topic"].as_str().unwrap(),
                )
            })
            .collect()
    }

    fn get_client(roles: &[&str]) -> Value {
        let roles: Vec<Value> = roles.iter().map(|r| json!({ "rolename": r })).collect();
        json!({ "command": "getClient", "data": { "client": { "username": "rc-RCA", "roles": roles } } })
    }

    #[test]
    fn rc_role_is_bound_to_its_own_serial_number() {
        let cmds = ensure_rc_commands("RCA", "pw");
        let topics = acls(&cmds);
        assert!(topics.contains(&("rc-RCA", "subscribePattern", "thing/product/RCA/#")));
        assert!(topics.contains(&("rc-RCA", "publishClientSend", "sys/product/RCA/#")));
        assert!(
            !topics
                .iter()
                .any(|(_, t, topic)| *t == "subscribePattern" && topic.contains('+')),
            "kein Abo fremder Geräte"
        );
        assert!(
            !topics.iter().any(|(_, _, topic)| topic.contains("%u")),
            "%u wird in 2.0.22 nicht ersetzt"
        );
        let client = cmds
            .iter()
            .find(|c| c["command"] == "createClient")
            .unwrap();
        assert_eq!(
            (client["username"].as_str(), client["password"].as_str()),
            (Some("rc-RCA"), Some("pw"))
        );
    }

    #[test]
    fn own_role_beats_the_block_which_beats_the_wildcard_devices() {
        let cmds = ensure_rc_commands("RCA", "pw");
        let roles = missing_rc_roles("RCA", &get_client(&[]));
        let priority = |role: &str| {
            roles.iter().find(|c| c["rolename"] == role).unwrap()["priority"]
                .as_i64()
                .unwrap()
        };
        assert!(
            priority("rc-RCA") > priority(BLOCK_ROLE)
                && priority(BLOCK_ROLE) > priority(DEVICES_ROLE)
        );
        let block = cmds
            .iter()
            .find(|c| c["rolename"] == BLOCK_ROLE && c["command"] == "addRoleACL")
            .unwrap();
        assert_eq!(
            (block["topic"].as_str(), block["allow"].as_bool()),
            (Some("thing/product/RCA/#"), Some(false))
        );
        assert!(
            acls(&cmds)
                .iter()
                .filter(|(_, _, topic)| topic.contains('+'))
                .all(|(role, _, _)| *role == DEVICES_ROLE),
            "Platzhalter nur in der Rolle mit der niedrigsten Priorität"
        );
    }

    #[test]
    fn only_missing_roles_are_assigned_to_the_client() {
        let all = missing_rc_roles("RCA", &json!({"command": "getClient"}));
        assert_eq!(all.len(), 3, "ohne Rollenliste fehlen alle");
        assert!(
            all.iter()
                .all(|c| c["command"] == "addClientRole" && c["username"] == "rc-RCA")
        );
        assert_eq!(
            missing_rc_roles("RCA", &get_client(&["rc-RCA", BLOCK_ROLE, DEVICES_ROLE])),
            Vec::<Value>::new(),
            "wiederholtes Koppeln trennt die Fernsteuerung nicht"
        );
        let missing = missing_rc_roles("RCA", &get_client(&["rc-RCA", "fremd"]));
        let names: Vec<_> = missing
            .iter()
            .map(|c| c["rolename"].as_str().unwrap())
            .collect();
        assert_eq!(names, [BLOCK_ROLE, DEVICES_ROLE]);
    }

    #[test]
    fn server_role_may_send_and_subscribe_everywhere() {
        let cmds = server_role_commands("suchlage-admin");
        assert_eq!(
            cmds[0],
            json!({ "command": "createRole", "rolename": "suchlage-server" })
        );
        assert_eq!(
            acls(&cmds),
            [
                ("suchlage-server", "publishClientSend", "#"),
                ("suchlage-server", "subscribePattern", "#")
            ]
        );
        let bound = cmds
            .iter()
            .find(|c| c["command"] == "addClientRole")
            .unwrap();
        assert_eq!(
            (bound["username"].as_str(), bound["rolename"].as_str()),
            (Some("suchlage-admin"), Some("suchlage-server"))
        );
    }

    #[test]
    fn payload_tags_every_command_for_correlation() {
        let (ids, bytes) = payload(revoke_rc_commands("RCA"));
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(v["commands"][2]["correlationData"], ids[2].as_str());
        assert_eq!(v["commands"][2]["command"], "removeRoleACL");
    }

    /// Meldungen wörtlich aus `plugins/dynamic-security/{clients,roles}.c` von 2.0.22.
    #[test]
    fn repeated_setup_errors_are_benign_only_for_their_direction() {
        assert!(is_benign("createRole", "Role already exists"));
        assert!(is_benign("createClient", "Client already exists"));
        assert!(is_benign(
            "addRoleACL",
            "ACL with this topic already exists"
        ));
        assert!(is_benign("deleteClient", "Client not found"));
        assert!(is_benign("deleteRole", "Role not found"));
        assert!(is_benign("removeRoleACL", "ACL not found"));
        assert!(
            !is_benign("addClientRole", "Role not found"),
            "halb angelegte ACL"
        );
        assert!(!is_benign("setClientPassword", "Client not found"));
        assert!(!is_benign("getClient", "Client not found"));
        assert!(!is_benign("addClientRole", "Internal error"));
        assert!(!is_benign("createRole", "Permission denied"));
    }
}
