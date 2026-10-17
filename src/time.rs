/// Empfangszeit in ms seit Epoch (UTC). Die einzige Uhr des Servers; der Lage-Kern bekommt
/// die Zeit als Argument und liest sie nie selbst.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
