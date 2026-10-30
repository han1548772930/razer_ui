//! Numeric text and byte coercion shared by current JavaScript wire callers.
use anyhow::{Result, bail, ensure};

/// StringNumericLiteral conversion used by source unary `+`, subtraction and
/// mixed Number/String comparisons. Invalid text produces NaN, not an error.
pub(crate) fn from_string(value: &str) -> f64 {
    let value = crate::source_string::trim(value);
    match value {
        "" => return 0.,
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefixes, bits) in [(["0x", "0X"], 4), (["0b", "0B"], 1), (["0o", "0O"], 3)] {
        if let Some(digits) = prefixes
            .iter()
            .find_map(|prefix| value.strip_prefix(*prefix))
        {
            return from_radix_digits(digits, bits);
        }
    }

    let bytes = value.as_bytes();
    let mut cursor = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let mut digits = 0;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
        digits += 1;
    }
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return f64::NAN;
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let exponent_start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if cursor == exponent_start {
            return f64::NAN;
        }
    }
    if cursor != bytes.len() {
        return f64::NAN;
    }
    value.parse().unwrap_or(f64::NAN)
}

fn from_radix_digits(digits: &str, bits_per_digit: u32) -> f64 {
    if digits.is_empty() {
        return f64::NAN;
    }
    let mut significand = 0u64;
    let mut bits = 0usize;
    let mut round = false;
    let mut sticky = false;
    for byte in digits.bytes() {
        let Some(digit) = char::from(byte).to_digit(1 << bits_per_digit) else {
            return f64::NAN;
        };
        for shift in (0..bits_per_digit).rev() {
            let bit = u64::from((digit >> shift) & 1);
            if bits == 0 && bit == 0 {
                continue;
            }
            bits = bits.saturating_add(1);
            match bits {
                1..=53 => significand = (significand << 1) | bit,
                54 => round = bit != 0,
                _ => sticky |= bit != 0,
            }
        }
    }
    if bits <= 53 {
        return significand as f64;
    }
    if bits > 1024 {
        return f64::INFINITY;
    }
    // Retain the round and sticky bits until the complete integer is known;
    // folding long radix strings through f64 would round intermediate values.
    if round && (sticky || significand & 1 != 0) {
        significand += 1;
    }
    significand as f64 * 2f64.powi((bits - 53) as i32)
}

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

#[cfg(test)]
mod tests {
    use super::from_string;

    fn assert_number(text: &str, expected: f64) {
        let actual = from_string(text);
        if expected.is_nan() {
            assert!(actual.is_nan(), "{text:?}: {actual:?}");
        } else {
            assert_eq!(actual.to_bits(), expected.to_bits(), "{text:?}");
        }
    }

    #[test]
    fn source_number_string_grammar_preserves_decimal_exponent_and_signed_zero() {
        for (text, expected) in [
            ("", 0.),
            (" \t\r\n\u{feff}\u{3000}", 0.),
            ("\u{feff}\u{00a0} +42 \u{2028}", 42.),
            ("00012", 12.),
            ("+1.", 1.),
            (".5", 0.5),
            ("-.5", -0.5),
            ("1.e2", 100.),
            ("+1.5E-2", 0.015),
            ("-0", -0.),
            ("-0e9999", -0.),
            ("-1e-9999", -0.),
            ("1e9999", f64::INFINITY),
            ("-1e9999", f64::NEG_INFINITY),
            ("Infinity", f64::INFINITY),
            ("+Infinity", f64::INFINITY),
            ("-Infinity", f64::NEG_INFINITY),
            ("9007199254740993", 9007199254740992.),
            ("9007199254740995", 9007199254740996.),
            ("5e-324", f64::from_bits(1)),
        ] {
            assert_number(text, expected);
        }
    }

    #[test]
    fn source_number_string_grammar_does_not_accept_rust_float_extensions() {
        for text in [
            "NaN",
            "nan",
            "inf",
            "+inf",
            "-inf",
            "infinity",
            "INFINITY",
            "Infinityx",
            "+",
            "-",
            ".",
            "+.",
            "1e",
            "1e+",
            "1e-",
            "1 e2",
            "1_000",
            "1f64",
            "0x",
            "0b",
            "0o",
            "0xG",
            "0b2",
            "0o8",
            "-0x10",
            "+0b10",
            "0o7.0",
            "0x1p1",
            "\u{0085}1",
            "1\u{0085}",
            "\u{180e}1",
            "\u{200b}1",
            "\0",
            "１２",
            "not-a-number",
        ] {
            assert_number(text, f64::NAN);
        }
    }

    #[test]
    fn source_number_radix_integers_round_once_to_nearest_even() {
        for (text, expected) in [
            ("0x0", 0.),
            ("0Xff", 255.),
            ("0b1010", 10.),
            ("0B1010", 10.),
            ("0o17", 15.),
            ("0O17", 15.),
            ("0x20000000000001", 9007199254740992.),
            ("0x20000000000003", 9007199254740996.),
            ("0x40000000000003", 18014398509481988.),
            (
                "0b100000000000000000000000000000000000000000000000000001",
                9007199254740992.,
            ),
            ("0o400000000000000001", 9007199254740992.),
        ] {
            assert_number(text, expected);
        }
        assert_number(&format!("0x{}20", "0".repeat(2048)), 32.);
        assert_number(&format!("0x1{}", "0".repeat(256)), f64::INFINITY);
        assert_number(&format!("0x{}", "f".repeat(256)), f64::INFINITY);
        assert_number(
            &format!("0x{}8{}", "f".repeat(13), "0".repeat(242)),
            f64::MAX,
        );
        assert_number(
            &format!("0x{}b{}", "f".repeat(13), "f".repeat(242)),
            f64::MAX,
        );
        assert_number(
            &format!("0x{}c{}", "f".repeat(13), "0".repeat(242)),
            f64::INFINITY,
        );
        assert_number(&format!("0x1{}g", "0".repeat(256)), f64::NAN);
    }
}
