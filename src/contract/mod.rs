//! Vertrag zwischen Server und Browser: reine Daten (serde + ts-rs), hängt von nichts im Crate ab.
//! `cargo test` erzeugt daraus `frontend/src/contract/generated/*.ts` und die Beispieldaten.

pub mod admin;
pub mod devicelog;
pub mod examples;
pub mod flights;
pub mod live;
pub mod map;
pub mod missions;
pub mod photos;

#[cfg(test)]
mod fixtures;

pub use admin::*;
pub use devicelog::*;
pub use flights::*;
pub use live::*;
pub use map::*;
pub use missions::*;
pub use photos::*;

/// Ab hier gilt ein Wert als verzögert (3 verpasste osd-Nachrichten bei 0,5 Hz).
pub const FRESHNESS_DELAYED_MS: i64 = 6_000;
/// Ab hier gilt die Verbindung als verloren.
pub const FRESHNESS_LOST_MS: i64 = 30_000;
/// Eine Lücke über diesem Wert beginnt ein neues Spursegment (Spec §5).
pub const SEGMENT_GAP_MS: i64 = FRESHNESS_DELAYED_MS;
/// So lange darf ein offener Flug ohne Daten bleiben, bevor er als „Verbindung verloren“ schließt.
pub const FLIGHT_CLOSE_AFTER_MS: i64 = 600_000;
