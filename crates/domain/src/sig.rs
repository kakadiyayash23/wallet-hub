//! HMAC-SHA256 helpers shared by every provider. Comparison is constant time.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(message);
    hex::encode(mac.finalize().into_bytes())
}

/// True when `expected_hex` equals HMAC(key, message). Case-insensitive on the hex.
pub fn verify_hex(key: &[u8], message: &[u8], expected_hex: &str) -> bool {
    let Ok(expected) = hex::decode(expected_hex.trim()) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(message);
    let actual = mac.finalize().into_bytes();
    if actual.len() != expected.len() {
        return false;
    }
    actual.as_slice().ct_eq(expected.as_slice()).into()
}

/// Constant-time equality for secrets of any length.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let sig = hmac_sha256_hex(b"key", b"message");
        assert!(verify_hex(b"key", b"message", &sig));
        assert!(verify_hex(b"key", b"message", &sig.to_uppercase()));
        assert!(!verify_hex(b"key", b"message!", &sig));
        assert!(!verify_hex(b"other", b"message", &sig));
        assert!(!verify_hex(b"key", b"message", "zz"));
    }
}
