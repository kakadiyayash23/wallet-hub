//! Money is an integer number of minor units (cents). Never a float.
//! Providers send decimals; the provider crate converts at the edge.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MoneyError {
    #[error("not a decimal amount: {0:?}")]
    Parse(String),
    #[error("more than two decimal places: {0:?}")]
    Precision(String),
    #[error("amount overflow")]
    Overflow,
}

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_minor(minor: i64) -> Self {
        Money(minor)
    }

    pub const fn minor(self) -> i64 {
        self.0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn abs(self) -> Money {
        Money(self.0.abs())
    }

    pub fn checked_add(self, other: Money) -> Result<Money, MoneyError> {
        self.0
            .checked_add(other.0)
            .map(Money)
            .ok_or(MoneyError::Overflow)
    }

    pub fn checked_sub(self, other: Money) -> Result<Money, MoneyError> {
        self.0
            .checked_sub(other.0)
            .map(Money)
            .ok_or(MoneyError::Overflow)
    }

    pub fn checked_neg(self) -> Result<Money, MoneyError> {
        self.0.checked_neg().map(Money).ok_or(MoneyError::Overflow)
    }

    /// Multiply by odds given in thousandths (2.10 -> 2100). Rounds down.
    pub fn mul_milli(self, milli: i64) -> Result<Money, MoneyError> {
        let v = (self.0 as i128) * (milli as i128) / 1000;
        i64::try_from(v)
            .map(Money)
            .map_err(|_| MoneyError::Overflow)
    }

    /// Parse "12.5", "-2.50", "100", "0". Rejects more than two decimals.
    pub fn from_decimal_str(s: &str) -> Result<Money, MoneyError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(MoneyError::Parse(s.to_string()));
        }
        let (neg, digits) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s.strip_prefix('+').unwrap_or(s)),
        };
        let (int_part, frac_part) = match digits.split_once('.') {
            Some((i, f)) => (i, f),
            None => (digits, ""),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return Err(MoneyError::Parse(s.to_string()));
        }
        if !int_part.chars().all(|c| c.is_ascii_digit())
            || !frac_part.chars().all(|c| c.is_ascii_digit())
        {
            return Err(MoneyError::Parse(s.to_string()));
        }
        if frac_part.len() > 2 && frac_part[2..].chars().any(|c| c != '0') {
            return Err(MoneyError::Precision(s.to_string()));
        }
        let int_val: i64 = if int_part.is_empty() {
            0
        } else {
            int_part.parse().map_err(|_| MoneyError::Overflow)?
        };
        let mut frac = frac_part.to_string();
        frac.truncate(2);
        while frac.len() < 2 {
            frac.push('0');
        }
        let frac_val: i64 = frac.parse().unwrap_or(0);
        let minor = int_val
            .checked_mul(100)
            .and_then(|v| v.checked_add(frac_val))
            .ok_or(MoneyError::Overflow)?;
        Ok(Money(if neg { -minor } else { minor }))
    }

    /// From a JSON float that a provider sent. Uses the shortest round-trip
    /// string form, so 2.5 -> 250 and 0.1 -> 10 with no binary noise.
    pub fn from_f64(v: f64) -> Result<Money, MoneyError> {
        if !v.is_finite() {
            return Err(MoneyError::Parse(v.to_string()));
        }
        Money::from_decimal_str(&v.to_string())
    }

    /// "-2.50", "0.00", "1247.50"
    pub fn to_decimal_string(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        format!("{sign}{}.{:02}", abs / 100, abs % 100)
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / 100.0
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_decimal_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn parses_common_forms() {
        assert_eq!(Money::from_decimal_str("2.5").unwrap(), Money(250));
        assert_eq!(Money::from_decimal_str("-2.50").unwrap(), Money(-250));
        assert_eq!(Money::from_decimal_str("100").unwrap(), Money(10000));
        assert_eq!(Money::from_decimal_str("0").unwrap(), Money(0));
        assert_eq!(Money::from_decimal_str(".5").unwrap(), Money(50));
        assert_eq!(
            Money::from_decimal_str("1.005").unwrap_err(),
            MoneyError::Precision("1.005".into())
        );
        assert!(Money::from_decimal_str("abc").is_err());
    }

    #[test]
    fn from_f64_has_no_binary_noise() {
        assert_eq!(Money::from_f64(0.1).unwrap(), Money(10));
        assert_eq!(Money::from_f64(-2.5).unwrap(), Money(-250));
        assert_eq!(Money::from_f64(1247.5).unwrap(), Money(124750));
        assert_eq!(Money::from_f64(0.0).unwrap(), Money(0));
    }

    #[test]
    fn formats_two_decimals() {
        assert_eq!(Money(-250).to_string(), "-2.50");
        assert_eq!(Money(5).to_string(), "0.05");
        assert_eq!(Money(124750).to_string(), "1247.50");
    }

    proptest! {
        #[test]
        fn decimal_string_round_trips(minor in -1_000_000_000_000i64..1_000_000_000_000i64) {
            let m = Money(minor);
            prop_assert_eq!(Money::from_decimal_str(&m.to_decimal_string()).unwrap(), m);
        }

        #[test]
        fn odds_never_overflow_for_sane_inputs(stake in 0i64..10_000_000, odds in 1000i64..100_000) {
            prop_assert!(Money(stake).mul_milli(odds).is_ok());
        }
    }
}
