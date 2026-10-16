//! Lage-Kern: reiner Zustandsautomat. Kennt weder DJI noch tokio, sqlx, axum, rumqttc, reqwest
//! (geprüft in scripts/check.sh). Eingang: `Observed`, Ausgang: `Effect` (Task B5).

pub mod observation;
pub mod phase;
pub mod state;
pub mod stream;
pub mod track;
