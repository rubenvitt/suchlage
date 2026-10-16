fn main() {
    // Ohne diese Zeile bettet sqlx::migrate! neue Migrationen erst nach der nächsten Rust-Änderung ein.
    println!("cargo:rerun-if-changed=migrations");
}
