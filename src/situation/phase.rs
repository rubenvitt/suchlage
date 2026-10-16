//! Start- und Landeerkennung mit Entprellung (Referenz Kap. 4):
//! - Ein Wechsel gilt erst nach 2 aufeinanderfolgenden Proben im neuen Zustand.
//! - Start braucht zusätzlich Plausibilität (Höhe über Start > 1 m oder |Steigrate| > 0,5 m/s),
//!   sonst würden laufende Motoren am Boden schon als Flug zählen.
//! - Landung braucht keine Plausibilität: niedriges Schweben darf keinen Flug beenden, deshalb
//!   entscheidet in der Luft allein der Modus.
//! - `Unknown`-Proben (Update, nicht verbunden) ändern nichts und setzen nichts zurück.

use crate::contract::FlightPhase;

use super::observation::ModeClass;

const CONFIRM_SAMPLES: u8 = 2;
const TAKEOFF_MIN_ELEVATION_M: f64 = 1.0;
const TAKEOFF_MIN_VERTICAL_SPEED_MPS: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub at_ms: i64,
    pub class: ModeClass,
    pub elevation_m: Option<f64>,
    pub vertical_speed_mps: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    TookOff { at_ms: i64 },
    Landed { at_ms: i64 },
}

#[derive(Debug, Clone)]
pub struct PhaseTracker {
    stable: FlightPhase,
    candidate: Option<(FlightPhase, u8, i64)>,
}

impl PhaseTracker {
    pub fn new(initial: FlightPhase) -> Self {
        Self {
            stable: initial,
            candidate: None,
        }
    }

    pub fn phase(&self) -> FlightPhase {
        self.stable
    }

    /// Startet der Tracker mit `FlightPhase::Unknown`, gilt das wie `Ground`: Ein Start wird erkannt,
    /// eine Landung nicht.
    pub fn observe(&mut self, s: &Sample) -> Option<Transition> {
        let target = match (self.stable, s.class) {
            (_, ModeClass::Unknown) => return None,
            (FlightPhase::Airborne, ModeClass::Ground) => FlightPhase::Ground,
            (FlightPhase::Airborne, ModeClass::Air) => {
                self.candidate = None;
                return None;
            }
            (_, ModeClass::Air) if plausible_takeoff(s) => FlightPhase::Airborne,
            _ => {
                self.candidate = None;
                return None;
            }
        };
        let (count, first_at) = match self.candidate {
            Some((phase, count, first_at)) if phase == target => (count + 1, first_at),
            _ => (1, s.at_ms),
        };
        if count < CONFIRM_SAMPLES {
            self.candidate = Some((target, count, first_at));
            return None;
        }
        self.candidate = None;
        self.stable = target;
        Some(match target {
            FlightPhase::Airborne => Transition::TookOff { at_ms: first_at },
            _ => Transition::Landed { at_ms: first_at },
        })
    }
}

fn plausible_takeoff(s: &Sample) -> bool {
    s.elevation_m.is_some_and(|e| e > TAKEOFF_MIN_ELEVATION_M)
        || s.vertical_speed_mps
            .is_some_and(|v| v.abs() > TAKEOFF_MIN_VERTICAL_SPEED_MPS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(at_ms: i64, class: ModeClass, elevation: f64) -> Sample {
        Sample {
            at_ms,
            class,
            elevation_m: Some(elevation),
            vertical_speed_mps: Some(0.0),
        }
    }

    #[test]
    fn takeoff_needs_two_plausible_air_samples_and_dates_back_to_the_first() {
        let mut t = PhaseTracker::new(FlightPhase::Ground);
        assert_eq!(t.observe(&s(0, ModeClass::Ground, 0.0)), None);
        assert_eq!(t.observe(&s(2_000, ModeClass::Air, 3.0)), None);
        assert_eq!(
            t.observe(&s(4_000, ModeClass::Air, 8.0)),
            Some(Transition::TookOff { at_ms: 2_000 })
        );
        assert_eq!(t.phase(), FlightPhase::Airborne);
    }

    #[test]
    fn single_air_outlier_is_ignored() {
        let mut t = PhaseTracker::new(FlightPhase::Ground);
        t.observe(&s(0, ModeClass::Air, 3.0));
        assert_eq!(t.observe(&s(2_000, ModeClass::Ground, 0.0)), None);
        assert_eq!(t.observe(&s(4_000, ModeClass::Air, 3.0)), None);
        assert_eq!(t.phase(), FlightPhase::Ground);
    }

    #[test]
    fn spinning_motors_on_the_ground_are_no_takeoff() {
        let mut t = PhaseTracker::new(FlightPhase::Ground);
        for i in 0..5 {
            assert_eq!(t.observe(&s(i * 2_000, ModeClass::Air, 0.1)), None);
        }
        assert_eq!(t.observe(&s(10_000, ModeClass::Air, 1.5)), None);
        assert_eq!(
            t.observe(&s(12_000, ModeClass::Air, 2.5)),
            Some(Transition::TookOff { at_ms: 10_000 })
        );
    }

    #[test]
    fn low_hover_does_not_land_but_two_ground_samples_do() {
        let mut t = PhaseTracker::new(FlightPhase::Airborne);
        assert_eq!(t.observe(&s(0, ModeClass::Air, 0.4)), None);
        assert_eq!(t.observe(&s(2_000, ModeClass::Ground, 0.0)), None);
        assert_eq!(
            t.observe(&s(4_000, ModeClass::Ground, 0.0)),
            Some(Transition::Landed { at_ms: 2_000 })
        );
    }

    #[test]
    fn unknown_samples_neither_change_nor_reset() {
        let mut t = PhaseTracker::new(FlightPhase::Airborne);
        t.observe(&s(0, ModeClass::Ground, 0.0));
        assert_eq!(t.observe(&s(2_000, ModeClass::Unknown, 0.0)), None);
        assert_eq!(
            t.observe(&s(4_000, ModeClass::Ground, 0.0)),
            Some(Transition::Landed { at_ms: 0 })
        );
    }

    #[test]
    fn vertical_speed_alone_makes_a_takeoff_plausible() {
        let mut t = PhaseTracker::new(FlightPhase::Ground);
        let climb = |at| Sample {
            at_ms: at,
            class: ModeClass::Air,
            elevation_m: None,
            vertical_speed_mps: Some(2.0),
        };
        t.observe(&climb(0));
        assert_eq!(
            t.observe(&climb(2_000)),
            Some(Transition::TookOff { at_ms: 0 })
        );
    }

    #[test]
    fn unknown_start_phase_detects_takeoff_but_no_landing() {
        let mut t = PhaseTracker::new(FlightPhase::Unknown);
        assert_eq!(t.observe(&s(0, ModeClass::Ground, 0.0)), None);
        assert_eq!(t.observe(&s(2_000, ModeClass::Ground, 0.0)), None);
        assert_eq!(t.phase(), FlightPhase::Unknown);
        t.observe(&s(4_000, ModeClass::Air, 3.0));
        assert_eq!(
            t.observe(&s(6_000, ModeClass::Air, 6.0)),
            Some(Transition::TookOff { at_ms: 4_000 })
        );
    }
}
