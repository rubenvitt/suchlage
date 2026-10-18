//! Einsätze: ein Ort, der Übergänge prüft, speichert und jede Änderung an die Browser meldet.

pub mod rules;

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, MutexGuard, broadcast};

use crate::contract::{
    Mission, MissionAssign, MissionCreate, MissionState, MissionUpdate, SectionCreate,
};
use crate::error::AppError;
use crate::map::MapHub;
use crate::store::{flights, missions as store};

/// Puffer je Empfänger; ein verpasstes Signal ersetzt das nächste, der Browser lädt die Liste neu.
const CHANGES_BUFFERED: usize = 16;

#[derive(Clone)]
pub struct MissionHub {
    pool: SqlitePool,
    map: MapHub,
    changes: broadcast::Sender<()>,
    /// Ändern ist eine Reihe: Prüfen und Schreiben laufen ohne eine zweite Änderung dazwischen.
    writes: Arc<Mutex<()>>,
}

impl MissionHub {
    /// Übernimmt den laufenden Einsatz als Sichtbereich der Pilots.
    pub async fn start_up(pool: SqlitePool, map: MapHub) -> Result<Self, AppError> {
        map.set_pilot_scope(store::running_id(&pool).await?);
        let (changes, _) = broadcast::channel(CHANGES_BUFFERED);
        Ok(Self {
            pool,
            map,
            changes,
            writes: Arc::new(Mutex::new(())),
        })
    }

    /// Die Schreibsperre der Einsätze, für Löschungen, die länger dauern (Dateien zuerst). Wer sie
    /// hält, darf keine Methode dieses Hubs aufrufen, die selbst sperrt (nur `get`, `changed`).
    pub(crate) async fn serial(&self) -> MutexGuard<'_, ()> {
        self.writes.lock().await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.changes.subscribe()
    }

    pub async fn list(&self) -> Result<Vec<Mission>, AppError> {
        Ok(store::list(&self.pool).await?)
    }

    pub async fn get(&self, id: i64) -> Result<Mission, AppError> {
        store::get(&self.pool, id).await?.ok_or(AppError::NotFound)
    }

    pub async fn create(
        &self,
        input: MissionCreate,
        created_by: &str,
        now_ms: i64,
    ) -> Result<Mission, AppError> {
        let input = MissionCreate {
            name: rules::name(&input.name, "Name").map_err(AppError::Validation)?,
            location: input.location,
        };
        let _serial = self.writes.lock().await;
        let mission = store::create(&self.pool, &input, created_by, now_ms).await?;
        self.changed();
        Ok(mission)
    }

    pub async fn start(&self, id: i64, now_ms: i64) -> Result<Mission, AppError> {
        self.change_state(id, MissionState::Running, now_ms).await
    }

    pub async fn close(&self, id: i64, now_ms: i64) -> Result<Mission, AppError> {
        self.change_state(id, MissionState::Closed, now_ms).await
    }

    async fn change_state(
        &self,
        id: i64,
        to: MissionState,
        now_ms: i64,
    ) -> Result<Mission, AppError> {
        let _serial = self.writes.lock().await;
        let current = self.get(id).await?;
        rules::transition(current.state, to).map_err(AppError::Unprocessable)?;
        if to == MissionState::Closed {
            // Den Zustand ändert unter dem Schloss niemand sonst: Bleibt die Zeile unverändert,
            // fliegt im Einsatz noch ein Fluggerät (ein Flugstart läuft nicht über dieses Schloss).
            if !store::close(&self.pool, id, now_ms).await? {
                return Err(AppError::Unprocessable(
                    "Im Einsatz fliegt noch ein Fluggerät".into(),
                ));
            }
        } else {
            // Ein zweiter laufender Einsatz scheitert am Teilindex (409).
            store::set_state(&self.pool, id, to, now_ms).await?;
        }
        self.map
            .set_pilot_scope(store::running_id(&self.pool).await?);
        self.changed();
        self.get(id).await
    }

    /// Name und Ort ändern, auch im laufenden Einsatz; abgeschlossen ist nur lesbar.
    pub async fn update(&self, id: i64, input: MissionUpdate) -> Result<Mission, AppError> {
        let input = MissionUpdate {
            name: rules::name(&input.name, "Name").map_err(AppError::Validation)?,
            location: input.location,
        };
        let _serial = self.writes.lock().await;
        if self.get(id).await?.state == MissionState::Closed {
            return Err(AppError::Unprocessable(
                "Der Einsatz ist abgeschlossen".into(),
            ));
        }
        store::update(&self.pool, id, &input).await?;
        self.changed();
        self.get(id).await
    }

    pub async fn add_section(
        &self,
        id: i64,
        input: SectionCreate,
        now_ms: i64,
    ) -> Result<Mission, AppError> {
        let input = SectionCreate {
            label: rules::name(&input.label, "Bezeichnung").map_err(AppError::Validation)?,
            location: input.location,
        };
        let _serial = self.writes.lock().await;
        if self.get(id).await?.state == MissionState::Closed {
            return Err(AppError::Unprocessable(
                "Der Einsatz ist abgeschlossen".into(),
            ));
        }
        store::add_section(&self.pool, id, &input, now_ms).await?;
        self.changed();
        self.get(id).await
    }

    /// Ordnet einen Flug „ohne Einsatz“ nachträglich zu, auch in einen abgeschlossenen Einsatz.
    pub async fn assign_flight(&self, flight_id: i64, to: MissionAssign) -> Result<(), AppError> {
        let _serial = self.writes.lock().await;
        flights::get(&self.pool, flight_id)
            .await?
            .ok_or(AppError::NotFound)?;
        self.get(to.mission_id).await?;
        if !store::assign_flight(&self.pool, flight_id, to.mission_id).await? {
            return Err(AppError::Unprocessable(
                "Der Flug gehört schon zu einem Einsatz".into(),
            ));
        }
        self.changed();
        Ok(())
    }

    /// Meldet den Browsern „neu laden“ (`missions_changed`), auch nach Löschungen.
    pub fn changed(&self) {
        // Ohne Empfänger schlägt `send` fehl – dann hört niemand zu.
        let _ = self.changes.send(());
    }
}
