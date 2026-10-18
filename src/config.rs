//! Bootstrap-Konfiguration aus der Umgebung. Nur das, was vor der Datenbank feststehen muss –
//! alle fachlichen Einstellungen leben in der App (ADR 0002).

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BootConfig {
    pub data_dir: PathBuf,
    pub bind: SocketAddr,
    /// Öffentliche Adresse, z. B. `https://suchlage.example`. `url::Url` hängt an eine Adresse
    /// ohne Pfad selbst den Schrägstrich an (`https://suchlage.example/`).
    pub public_url: url::Url,
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{var} ist ungültig: {reason}")]
pub struct BootError {
    pub var: &'static str,
    pub reason: String,
}

impl BootConfig {
    pub fn from_env() -> Result<Self, BootError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, BootError> {
        let data_dir = PathBuf::from(get("SUCHLAGE_DATA_DIR").unwrap_or_else(|| "./data".into()));
        let bind_raw = get("SUCHLAGE_BIND").unwrap_or_else(|| "127.0.0.1:8080".into());
        let bind = bind_raw
            .parse()
            .map_err(|e: std::net::AddrParseError| BootError {
                var: "SUCHLAGE_BIND",
                reason: e.to_string(),
            })?;
        let url_raw = get("SUCHLAGE_PUBLIC_URL").unwrap_or_else(|| "http://127.0.0.1:8080".into());
        let public_url = url::Url::parse(&url_raw).map_err(|e| BootError {
            var: "SUCHLAGE_PUBLIC_URL",
            reason: e.to_string(),
        })?;
        if !matches!(public_url.scheme(), "http" | "https") {
            return Err(BootError {
                var: "SUCHLAGE_PUBLIC_URL",
                reason: "nur http oder https".into(),
            });
        }
        Ok(Self {
            data_dir,
            bind,
            public_url,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("suchlage.db")
    }

    pub fn records_dir(&self) -> PathBuf {
        self.data_dir.join("records")
    }

    /// Vorschaubilder der Fotos (M3); jederzeit aus dem Fotospeicher neu erzeugbar.
    pub fn thumbs_dir(&self) -> PathBuf {
        self.data_dir.join("thumbs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn defaults_bind_to_loopback() {
        let cfg = BootConfig::from_lookup(lookup(&[])).unwrap();
        assert_eq!(cfg.bind.to_string(), "127.0.0.1:8080");
        assert_eq!(cfg.data_dir, PathBuf::from("./data"));
        assert_eq!(cfg.public_url.as_str(), "http://127.0.0.1:8080/");
        assert_eq!(cfg.db_path(), PathBuf::from("./data/suchlage.db"));
    }

    #[test]
    fn invalid_bind_names_the_variable() {
        let err = BootConfig::from_lookup(lookup(&[("SUCHLAGE_BIND", "nirgends")])).unwrap_err();
        assert_eq!(err.var, "SUCHLAGE_BIND");
    }

    #[test]
    fn public_url_must_be_http_or_https() {
        let err =
            BootConfig::from_lookup(lookup(&[("SUCHLAGE_PUBLIC_URL", "ftp://x")])).unwrap_err();
        assert_eq!(err.var, "SUCHLAGE_PUBLIC_URL");
    }
}
