//! Prüfregeln für Einsätze. Rein: kein Speicher, keine Laufzeit.

use crate::contract::MissionState;

pub const MAX_NAME_CHARS: usize = 120;

/// Name eines Einsatzes oder Abschnitts: getrimmt, 1 bis 120 Zeichen (wie bei Kartenelementen).
pub fn name(raw: &str, field: &str) -> Result<String, String> {
    let name = raw.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > MAX_NAME_CHARS {
        return Err(format!("{field}: 1 bis {MAX_NAME_CHARS} Zeichen"));
    }
    Ok(name.to_string())
}

/// Nur vorwärts: vorbereitet → laufend → abgeschlossen; ein vorbereiteter Einsatz darf entfallen.
pub fn transition(from: MissionState, to: MissionState) -> Result<(), String> {
    use MissionState::*;
    match (from, to) {
        (Prepared, Running) | (Prepared, Closed) | (Running, Closed) => Ok(()),
        _ => Err(format!(
            "Einsatz kann nicht von {from:?} nach {to:?} wechseln"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MissionState::*;

    #[test]
    fn only_forward_transitions() {
        assert!(transition(Prepared, Running).is_ok());
        assert!(transition(Prepared, Closed).is_ok());
        assert!(transition(Running, Closed).is_ok());
        assert!(transition(Closed, Running).is_err());
        assert!(transition(Running, Prepared).is_err());
        assert!(transition(Closed, Closed).is_err());
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(name("  Wald ", "Name").unwrap(), "Wald");
        assert!(name("  ", "Name").is_err());
        assert!(name(&"x".repeat(121), "Name").is_err());
    }
}
