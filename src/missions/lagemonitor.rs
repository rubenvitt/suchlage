//! Laufender Einsatz für den Lagemonitor im FüKw: Stammdaten und Einsatzkarte, nur lesen.

use crate::contract::{FmFeatureCollection, LmMission};
use crate::error::AppError;
use crate::map::geojson::{self, Track};
use crate::store::{flights, missions as store};

use super::MissionHub;

impl MissionHub {
    /// Der laufende Einsatz mit Kartenelementen und allen Flugspuren, sonst `None`.
    pub async fn lagemonitor(&self) -> Result<Option<(LmMission, FmFeatureCollection)>, AppError> {
        let Some(id) = store::running_id(&self.pool).await? else {
            return Ok(None);
        };
        let mission = self.get(id).await?;
        let elements = self.map.list_in(Some(id)).await?;
        let mut tracks = Vec::new();
        for (flight, _) in flights::of_mission_with_section(&self.pool, id).await? {
            tracks.push(Track {
                flugmappe_section_id: None,
                segments: flights::segments(&self.pool, flight.id).await?,
                flight,
            });
        }
        let view = LmMission {
            id: mission.id,
            name: mission.name,
            location: mission.location,
            started_at_ms: mission.started_at_ms,
            section: mission.sections.last().map(|s| s.label.clone()),
        };
        Ok(Some((
            view,
            geojson::feature_collection(&elements, &tracks),
        )))
    }
}
