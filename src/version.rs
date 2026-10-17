//! Welcher Stand läuft. Die Release-Pipeline setzt beim Bauen `SUCHLAGE_VERSION` und
//! `SUCHLAGE_COMMIT` (Docker-Build-Args); das ist keine Laufzeit-Konfiguration, sondern steht fest
//! im Binary. Ohne sie gilt `<Cargo-Version>-dev`.

/// Release-Version ohne `v`, etwa `0.2.3`.
pub const VERSION: &str = match option_env!("SUCHLAGE_VERSION") {
    // Ein leeres Build-Arg (lokales `docker build`) zählt wie ein fehlendes.
    Some(v) if !v.is_empty() => v,
    _ => concat!(env!("CARGO_PKG_VERSION"), "-dev"),
};

/// Kurzer Commit des Builds.
pub const COMMIT: Option<&str> = match option_env!("SUCHLAGE_COMMIT") {
    Some(c) if !c.is_empty() => Some(c),
    _ => None,
};
