//! Laufzeit-Hülle um den reinen Lage-Kern: Aktor, Speicher, Verteilung.

pub mod service;

use std::sync::Arc;

pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(crate::time::now_ms)
}
