//! The field signature: HMAC-SHA256 over concatenated strings, with the amount
//! rendered the way JavaScript's `String(number)` would render it.

use serde_json::Value;

/// `String(n)` in JavaScript for the common range: integers without a decimal
/// point, otherwise the shortest round-trip decimal form. `-0` becomes `0`.
pub fn js_number_string(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    if v.fract() == 0.0 && v.abs() < 1e21 {
        return format!("{}", v as i128);
    }
    // Rust's Display for f64 is the shortest representation that round-trips,
    // which matches JavaScript for 1e-6 <= |v| < 1e21.
    v.to_string()
}

pub fn js_value_string(v: &Value) -> String {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                js_number_string(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Produce a signature the way BigBang does. Used by tests and the chaos simulator.
pub fn sign_balance_change(
    api_key: &str,
    username: &str,
    amount: f64,
    game: &str,
    game_category: &str,
    transaction_id: &str,
) -> String {
    let message = format!(
        "{username}{}{game}{game_category}{transaction_id}",
        js_number_string(amount)
    );
    domain::sig::hmac_sha256_hex(api_key.as_bytes(), message.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::BalanceChange;

    #[test]
    fn js_number_forms() {
        assert_eq!(js_number_string(-2.5), "-2.5");
        assert_eq!(js_number_string(2.0), "2");
        assert_eq!(js_number_string(0.0), "0");
        assert_eq!(js_number_string(-0.0), "0");
        assert_eq!(js_number_string(0.1), "0.1");
        assert_eq!(js_number_string(1247.5), "1247.5");
        assert_eq!(js_number_string(100000.0), "100000");
    }

    #[test]
    fn json_number_forms() {
        assert_eq!(js_value_string(&serde_json::json!(-2.5)), "-2.5");
        assert_eq!(js_value_string(&serde_json::json!(2)), "2");
        assert_eq!(js_value_string(&serde_json::json!(0)), "0");
        assert_eq!(js_value_string(&serde_json::json!("1.50")), "1.50");
    }

    #[test]
    fn signature_round_trips_through_the_body() {
        let key = "ek_test_abc";
        let sig = sign_balance_change(
            key,
            "player_42",
            -2.5,
            "SGHotHotFruit",
            "Habanero",
            "66a3f1b2c",
        );
        let body = serde_json::json!({
            "username": "player_42", "amount": -2.5, "game": "SGHotHotFruit",
            "game_category": "Habanero", "transaction_id": "66a3f1b2c", "signature": sig,
            "round_id": "66a3f1b2c", "type": "round", "round_end": true, "sandbox": true
        });
        let cb: BalanceChange = serde_json::from_value(body).unwrap();
        assert!(domain::sig::verify_hex(
            key.as_bytes(),
            cb.signature_message().as_bytes(),
            &cb.signature
        ));
        assert_eq!(cb.amount_money().unwrap().minor(), -250);
    }

    #[test]
    fn zero_amount_is_accepted() {
        let body = serde_json::json!({
            "username": "p", "amount": 0, "game": "g", "game_category": "c",
            "transaction_id": "t", "signature": "", "round_end": true
        });
        let cb: BalanceChange = serde_json::from_value(body).unwrap();
        assert_eq!(cb.amount_money().unwrap().minor(), 0);
        assert_eq!(cb.signature_message(), "p0gct");
    }
}
