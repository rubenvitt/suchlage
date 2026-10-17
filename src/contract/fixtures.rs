//! Schreibt die Beispielwerte als typgeprüftes TypeScript ins Frontend. `tsc` prüft damit, dass
//! Rusts Serialisierung zu den erzeugten TS-Typen passt – der eigentliche Vertragstest.

use std::collections::BTreeSet;
use std::path::Path;

use super::examples;

#[test]
fn examples_roundtrip_through_json() {
    examples::assert_roundtrips();
}

#[test]
fn write_typescript_fixtures() {
    let fixtures = examples::all();
    let mut types = BTreeSet::new();
    let mut body = String::new();
    for f in &fixtures {
        types.insert(f.ts_type.trim_end_matches("[]"));
        body.push_str(&format!(
            "\nexport const {}: {} = {};\n",
            f.name,
            f.ts_type,
            serde_json::to_string_pretty(&f.json).unwrap()
        ));
    }
    let mut out = String::from(
        "// Erzeugt von `cargo test contract` (src/contract/fixtures.rs) – nicht von Hand ändern.\n",
    );
    for t in types {
        out.push_str(&format!("import type {{ {t} }} from './generated/{t}';\n"));
    }
    out.push_str(&body);
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend/src/contract/fixtures.generated.ts");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(out.as_str()) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, out).unwrap();
    }
}
