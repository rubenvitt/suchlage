//! Seriennummern von Fernsteuerung und Fluggerät. Geprüft wird, bevor eine SN in Topics, Pfade
//! oder Dateinamen wandert. Liegt außerhalb von `dji/`, weil auch Medien und Web sie prüfen.

pub fn is_valid_sn(sn: &str) -> bool {
    (1..=64).contains(&sn.len()) && sn.bytes().all(|b| b.is_ascii_alphanumeric())
}
