//! DJI-Topics (Doku v1.11.3, `00.topic-definition.md`). Seriennummern werden geprüft, bevor sie
//! in Topics, Pfade oder Dateinamen wandern.

#[derive(Debug, PartialEq, Eq)]
pub enum DjiTopic<'a> {
    Osd { sn: &'a str },
    State { sn: &'a str },
    Status { gateway: &'a str },
    ServicesReply { gateway: &'a str },
    Events { gateway: &'a str },
    Requests { gateway: &'a str },
    Other,
}

/// Was der Server abonniert. `services` und `*_reply` in Gegenrichtung abonniert er nicht.
pub const SUBSCRIPTIONS: &[&str] = &[
    "thing/product/+/osd",
    "thing/product/+/state",
    "sys/product/+/status",
    "thing/product/+/services_reply",
    "thing/product/+/events",
    "thing/product/+/requests",
];

pub fn is_valid_sn(sn: &str) -> bool {
    (1..=64).contains(&sn.len()) && sn.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub fn parse(topic: &str) -> DjiTopic<'_> {
    let parts: Vec<&str> = topic.split('/').collect();
    let [root, "product", sn, leaf] = parts.as_slice() else {
        return DjiTopic::Other;
    };
    if !is_valid_sn(sn) {
        return DjiTopic::Other;
    }
    match (*root, *leaf) {
        ("thing", "osd") => DjiTopic::Osd { sn },
        ("thing", "state") => DjiTopic::State { sn },
        ("sys", "status") => DjiTopic::Status { gateway: sn },
        ("thing", "services_reply") => DjiTopic::ServicesReply { gateway: sn },
        ("thing", "events") => DjiTopic::Events { gateway: sn },
        ("thing", "requests") => DjiTopic::Requests { gateway: sn },
        _ => DjiTopic::Other,
    }
}

pub fn services(gateway: &str) -> String {
    format!("thing/product/{gateway}/services")
}

pub fn status_reply(gateway: &str) -> String {
    format!("sys/product/{gateway}/status_reply")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_and_gateway_topics() {
        assert_eq!(parse("thing/product/AC1/osd"), DjiTopic::Osd { sn: "AC1" });
        assert_eq!(
            parse("thing/product/RC1/state"),
            DjiTopic::State { sn: "RC1" }
        );
        assert_eq!(
            parse("sys/product/RC1/status"),
            DjiTopic::Status { gateway: "RC1" }
        );
        assert_eq!(
            parse("thing/product/RC1/services_reply"),
            DjiTopic::ServicesReply { gateway: "RC1" }
        );
    }

    #[test]
    fn rejects_suspicious_serial_numbers() {
        assert_eq!(parse("thing/product/../osd"), DjiTopic::Other);
        assert_eq!(parse("thing/product/a+b/osd"), DjiTopic::Other);
        assert_eq!(
            parse("sys/product/RC1/status_reply"),
            DjiTopic::Other,
            "eigene Antworten ignorieren"
        );
        assert_eq!(parse("thing/product/RC1/osd/extra"), DjiTopic::Other);
    }
}
