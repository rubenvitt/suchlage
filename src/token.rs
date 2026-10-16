//! Zufallstokens (256 Bit) und ihr Hash. In der DB steht nur der Hash (Lifeline-Muster).

use sha2::{Digest, Sha256};

pub fn generate() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("Betriebssystem liefert Zufall");
    hex::encode(bytes)
}

pub fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn tokens_are_long_unique_and_hash_stably() {
        let (a, b) = (super::generate(), super::generate());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert_eq!(super::hash("x"), super::hash("x"));
        assert_ne!(super::hash(&a), a);
    }
}
