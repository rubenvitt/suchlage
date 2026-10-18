//! Kartenelemente (M2): ein Ort, der prüft, speichert und jede Änderung an Browser (SSE) und Pilots
//! (WebSocket) verteilt. Das DJI-Format kennt nur `dji::map`.

pub mod rules;

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, broadcast, watch};

use crate::contract::{MapElement, MapElementKind, MapElementOrigin, MapPoint};
use crate::error::AppError;
use crate::store::map_elements as store;

/// Puffer je Empfänger. Wer mehr verpasst, bekommt die ganze Liste neu.
const CHANGES_BUFFERED: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum MapChange {
    Created(MapElement),
    Updated(MapElement),
    Deleted { id: String },
}

/// Neues Element. Die ID kommt aus Pilot 2; ohne ID vergibt der Server eine.
#[derive(Debug, Clone, PartialEq)]
pub struct NewElement {
    pub id: Option<String>,
    pub name: String,
    pub kind: MapElementKind,
    pub points: Vec<MapPoint>,
    /// Schon normalisiert (`rules::normalize_color` oder `rules::palette_color`).
    pub color: String,
    pub clamp_to_ground: bool,
    pub author: String,
    pub origin: MapElementOrigin,
}

/// Änderung. Was `None` ist, bleibt.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ElementPatch {
    pub name: Option<String>,
    pub color: Option<String>,
    pub shape: Option<(MapElementKind, Vec<MapPoint>)>,
    pub clamp_to_ground: Option<bool>,
}

#[derive(Clone)]
pub struct MapHub {
    pool: SqlitePool,
    changes: broadcast::Sender<MapChange>,
    stop: watch::Receiver<bool>,
    /// Schreiben ist eine Reihe: Lesen, Schreiben und Verteilen einer Änderung laufen ohne eine
    /// zweite dazwischen. Sonst überschriebe eine Umbenennung im Browser eine gleichzeitige
    /// Verschiebung aus Pilot 2, und Pilots und Browser sähen Änderungen in anderer Reihenfolge.
    writes: Arc<Mutex<()>>,
}

impl MapHub {
    /// `stop` beendet die WebSocket-Verbindungen der Pilots mit dem Server.
    pub fn new(pool: SqlitePool, stop: watch::Receiver<bool>) -> Self {
        let (changes, _) = broadcast::channel(CHANGES_BUFFERED);
        Self {
            pool,
            changes,
            stop,
            writes: Arc::new(Mutex::new(())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MapChange> {
        self.changes.subscribe()
    }

    pub fn stop_signal(&self) -> watch::Receiver<bool> {
        self.stop.clone()
    }

    pub async fn list(&self) -> Result<Vec<MapElement>, AppError> {
        Ok(store::list(&self.pool).await?)
    }

    pub async fn get(&self, id: &str) -> Result<Option<MapElement>, AppError> {
        Ok(store::get(&self.pool, id).await?)
    }

    /// Legt an. Gibt es die ID schon (Pilot 2 wiederholt eine Anfrage), wird der Stand überschrieben.
    pub async fn create(&self, new: NewElement, now_ms: i64) -> Result<MapElement, AppError> {
        let _serial = self.writes.lock().await;
        let id = match new.id {
            Some(id) => valid_id(id)?,
            None => uuid::Uuid::new_v4().to_string(),
        };
        let element = MapElement {
            id,
            name: rules::name(&new.name).map_err(AppError::Validation)?,
            kind: new.kind,
            points: rules::shape(new.kind, new.points).map_err(AppError::Validation)?,
            color: new.color,
            clamp_to_ground: new.clamp_to_ground,
            author: new.author,
            origin: new.origin,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            mission_id: None,
        };
        if store::insert(&self.pool, &element).await? {
            self.publish(MapChange::Created(element.clone()));
            return Ok(element);
        }
        let patch = ElementPatch {
            name: Some(element.name),
            color: Some(element.color),
            shape: Some((element.kind, element.points)),
            clamp_to_ground: Some(element.clamp_to_ground),
        };
        self.apply(&element.id, patch, now_ms).await
    }

    pub async fn update(
        &self,
        id: &str,
        patch: ElementPatch,
        now_ms: i64,
    ) -> Result<MapElement, AppError> {
        let _serial = self.writes.lock().await;
        self.apply(id, patch, now_ms).await
    }

    /// Nur mit gehaltener Schreibsperre.
    async fn apply(
        &self,
        id: &str,
        patch: ElementPatch,
        now_ms: i64,
    ) -> Result<MapElement, AppError> {
        let mut element = store::get(&self.pool, id)
            .await?
            .ok_or(AppError::NotFound)?;
        if let Some(name) = patch.name {
            element.name = rules::name(&name).map_err(AppError::Validation)?;
        }
        if let Some(color) = patch.color {
            element.color = color;
        }
        if let Some((kind, points)) = patch.shape {
            element.points = rules::shape(kind, points).map_err(AppError::Validation)?;
            element.kind = kind;
        }
        if let Some(clamp) = patch.clamp_to_ground {
            element.clamp_to_ground = clamp;
        }
        // Steigt immer: Der Browser verwirft einen Stand, der älter ist als der bekannte.
        element.updated_at_ms = now_ms.max(element.updated_at_ms + 1);
        if !store::update(&self.pool, &element).await? {
            return Err(AppError::NotFound);
        }
        self.publish(MapChange::Updated(element.clone()));
        Ok(element)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let _serial = self.writes.lock().await;
        if !store::delete(&self.pool, id).await? {
            return Err(AppError::NotFound);
        }
        self.publish(MapChange::Deleted { id: id.to_string() });
        Ok(())
    }

    fn publish(&self, change: MapChange) {
        // Ohne Empfänger (kein Browser, kein Pilot) geht die Änderung nur in die Datenbank.
        let _ = self.changes.send(change);
    }
}

/// IDs aus Pilot 2 sind UUIDs; erlaubt ist, was ohne Escaping in Pfad und JSON passt.
fn valid_id(id: String) -> Result<String, AppError> {
    let ok = (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(id)
        .ok_or_else(|| AppError::Validation("Element-ID ungültig".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::examples;

    async fn hub() -> MapHub {
        let (_stop, rx) = watch::channel(false);
        MapHub::new(crate::db::test_pool().await, rx)
    }

    fn pin(id: Option<&str>) -> NewElement {
        let e = examples::map_elements().remove(1);
        NewElement {
            id: id.map(Into::into),
            name: e.name,
            kind: e.kind,
            points: e.points,
            color: e.color,
            clamp_to_ground: e.clamp_to_ground,
            author: e.author,
            origin: e.origin,
        }
    }

    #[tokio::test]
    async fn changes_are_stored_and_published_in_order() {
        let hub = hub().await;
        let mut rx = hub.subscribe();
        let created = hub.create(pin(None), 10).await.unwrap();
        assert_eq!(created.id.len(), 36, "Server vergibt eine UUID");
        let renamed = hub
            .update(
                &created.id,
                ElementPatch {
                    name: Some(" Fundstelle ".into()),
                    ..ElementPatch::default()
                },
                20,
            )
            .await
            .unwrap();
        assert_eq!(
            (renamed.name.as_str(), renamed.updated_at_ms),
            ("Fundstelle", 20)
        );
        hub.delete(&created.id).await.unwrap();

        assert_eq!(
            rx.recv().await.unwrap(),
            MapChange::Created(created.clone())
        );
        assert_eq!(rx.recv().await.unwrap(), MapChange::Updated(renamed));
        assert_eq!(
            rx.recv().await.unwrap(),
            MapChange::Deleted { id: created.id }
        );
        assert!(hub.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_repeated_create_from_pilot_updates_instead_of_failing() {
        let hub = hub().await;
        let id = "6d5da7b4-ac39-42bf-9580-4c8a94c11989";
        let first = hub.create(pin(Some(id)), 10).await.unwrap();
        let mut rx = hub.subscribe();
        let again = hub
            .create(
                NewElement {
                    name: "Pilot 1 (neu)".into(),
                    ..pin(Some(id))
                },
                20,
            )
            .await
            .unwrap();
        assert_eq!(
            (again.created_at_ms, again.name.as_str()),
            (first.created_at_ms, "Pilot 1 (neu)")
        );
        assert!(matches!(rx.recv().await.unwrap(), MapChange::Updated(_)));
        assert_eq!(hub.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn concurrent_changes_neither_get_lost_nor_go_back_in_time() {
        let hub = hub().await;
        let created = hub.create(pin(None), 100).await.unwrap();
        let moved = vec![MapPoint {
            lat: 52.5,
            lon: 10.5,
            alt_m: None,
        }];
        // Beide mit derselben Uhrzeit, wie zwei Anfragen in derselben Millisekunde.
        let rename = hub.update(
            &created.id,
            ElementPatch {
                name: Some("Fundstelle".into()),
                ..ElementPatch::default()
            },
            100,
        );
        let shift = hub.update(
            &created.id,
            ElementPatch {
                shape: Some((MapElementKind::Point, moved.clone())),
                ..ElementPatch::default()
            },
            100,
        );
        let (a, b) = tokio::join!(rename, shift);
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_ne!(a.updated_at_ms, b.updated_at_ms);
        let stored = hub.get(&created.id).await.unwrap().unwrap();
        assert_eq!(
            (stored.name.as_str(), stored.points.clone()),
            ("Fundstelle", moved),
            "keine Änderung geht verloren"
        );
        assert_eq!(
            stored.updated_at_ms,
            a.updated_at_ms.max(b.updated_at_ms),
            "der zuletzt geschriebene Stand ist auch der neueste"
        );
    }

    #[tokio::test]
    async fn invalid_input_changes_nothing() {
        let hub = hub().await;
        let mut rx = hub.subscribe();
        let empty_name = NewElement {
            name: " ".into(),
            ..pin(None)
        };
        let no_points = NewElement {
            points: vec![],
            ..pin(None)
        };
        let bad_id = pin(Some("../etc"));
        for bad in [empty_name, no_points, bad_id] {
            let err = hub.create(bad, 10).await.unwrap_err();
            assert!(matches!(err, AppError::Validation(_)), "{err:?}");
        }
        let created = hub.create(pin(None), 10).await.unwrap();
        let err = hub
            .update(
                &created.id,
                ElementPatch {
                    shape: Some((MapElementKind::Polygon, created.points.clone())),
                    ..ElementPatch::default()
                },
                20,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{err:?}");
        assert!(matches!(
            hub.update("fehlt", ElementPatch::default(), 20).await,
            Err(AppError::NotFound)
        ));
        assert!(matches!(hub.delete("fehlt").await, Err(AppError::NotFound)));
        assert!(matches!(rx.try_recv().unwrap(), MapChange::Created(_)));
        assert!(
            rx.try_recv().is_err(),
            "nur das gültige Anlegen wurde verteilt"
        );
    }
}
