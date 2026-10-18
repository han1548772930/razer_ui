//! Numeric text and byte coercion shared by current JavaScript wire callers.
use anyhow::{Result, bail, ensure};

/// Shortest decimal with ECMAScript's fixed/exponent thresholds, independent
/// of host locale and without fractional `.0` or negative zero.
pub(crate) fn to_string(value: f64) -> Result<String> {
    ensure!(value.is_finite(), "Non-finite source number");
    if value == 0.0 {
        return Ok("0".into());
    }
    let raw = serde_json::to_string(&value)?;
    let (sign, raw) = if let Some(rest) = raw.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", raw.as_str())
    };
    let (mantissa, exponent) = match raw.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i32>()?),
        None => (raw, 0),
    };
    let dot = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let mut digits = mantissa.replace('.', "");
    let mut point = dot + exponent;
    while digits.starts_with('0') && digits.len() > 1 {
        digits.remove(0);
        point -= 1;
    }
    while digits.ends_with('0') && digits.len() > 1 {
        digits.pop();
    }
    if digits.is_empty() {
        bail!("Empty JS decimal");
    }
    if value.abs() >= 1e-6 && value.abs() < 1e21 {
        if point <= 0 {
            Ok(format!("{sign}0.{}{digits}", "0".repeat((-point) as usize)))
        } else if point as usize >= digits.len() {
            Ok(format!(
                "{sign}{digits}{}",
                "0".repeat(point as usize - digits.len())
            ))
        } else {
            let (a, b) = digits.split_at(point as usize);
            Ok(format!("{sign}{a}.{b}"))
        }
    } else {
        let exponent = point - 1;
        let exponent = if exponent >= 0 {
            format!("+{exponent}")
        } else {
            exponent.to_string()
        };
        let (first, rest) = digits.split_at(1);
        Ok(if rest.is_empty() {
            format!("{sign}{first}e{exponent}")
        } else {
            format!("{sign}{first}.{rest}e{exponent}")
        })
    }
}

pub(crate) fn to_uint8(value: f64) -> u8 {
    if value.is_finite() {
        value.trunc().rem_euclid(256.) as u8
    } else {
        0
    }
}
