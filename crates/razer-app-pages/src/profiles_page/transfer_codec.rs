//! Profiles 43/Ea and 1867/D: base64 UTF-8, stable JSON, MD5, then outer name.
//! This reads a file; it neither executes migrations nor applies device settings.
use serde_json::Value;

pub struct DecodedFile {
    pub profiles: Vec<Value>,
    pub macros: Vec<Value>,
    pub device_name: String,
    pub warning: bool,
}

fn base64(input: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(input.len() * 3 / 4);
    let mut bits = 0u32;
    let mut available = 0;
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            // Current 7207/R normalizes URL-safe characters and y removes
            // every non-alphabet character, including existing padding.
            _ => continue,
        };
        bits = (bits << 6) | u32::from(value);
        available += 6;
        if available >= 8 {
            available -= 8;
            bytes.push((bits >> available) as u8);
        }
    }
    if available == 6 {
        return Err("Truncated base64 profile payload".into());
    }
    Ok(bytes)
}

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn same_number(left: &Value, right: &Value) -> bool {
    match (left.as_f64(), right.as_f64()) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

// JavaScript JSON.stringify's ordinary finite-number notation. The MD5 check
// is authoritative: an unsupported encoding is rejected, never accepted unhashed.
fn number(value: &serde_json::Number) -> Result<String, String> {
    let value = value
        .as_f64()
        .ok_or("Profile number is outside JavaScript range")?;
    if !value.is_finite() {
        return Err("Non-finite profile number".into());
    }
    if value == 0. {
        return Ok("0".into());
    }
    let negative = value < 0.;
    let raw = value.abs().to_string();
    let (mantissa, exp) = raw.split_once('e').map_or((raw.as_str(), 0), |(m, e)| {
        (m, e.parse::<i32>().unwrap_or(0))
    });
    let dot = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let mut digits = mantissa.replace('.', "");
    let leading = digits.bytes().take_while(|b| *b == b'0').count();
    digits.drain(..leading);
    let point = dot + exp - leading as i32;
    while digits.ends_with('0') {
        digits.pop();
    }
    let sign = if negative { "-" } else { "" };
    let result = if point > 0 && point <= 21 {
        if point as usize >= digits.len() {
            format!("{digits}{}", "0".repeat(point as usize - digits.len()))
        } else {
            format!(
                "{}.{}",
                &digits[..point as usize],
                &digits[point as usize..]
            )
        }
    } else if point <= 0 && point > -6 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else {
        let fraction = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        let exponent = point - 1;
        format!(
            "{}{fraction}e{}{exponent}",
            &digits[..1],
            if exponent >= 0 { "+" } else { "" }
        )
    };
    Ok(format!("{sign}{result}"))
}

fn stable(value: &Value) -> Result<String, String> {
    Ok(match value {
        Value::Object(values) => {
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            let fields = keys
                .into_iter()
                .map(|key| {
                    Ok(format!(
                        "{}:{}",
                        serde_json::to_string(key).map_err(|e| e.to_string())?,
                        stable(&values[key])?
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(stable)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        Value::Number(value) => number(value)?,
        _ => serde_json::to_string(value).map_err(|e| e.to_string())?,
    })
}

fn entry(encoded: &Value) -> Result<Value, String> {
    let bytes = base64(
        encoded["payload"]
            .as_str()
            .ok_or("Missing profile payload")?,
    )?;
    let mut value: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid profile JSON: {e}"))?;
    let mut hashed = value.clone();
    let object = hashed
        .as_object_mut()
        .ok_or("Profile payload is not an object")?;
    object.remove("hash");
    object.remove("gamemode");
    let digest = format!("{:x}", md5::compute(stable(&hashed)?.as_bytes()));
    if encoded["hash"].as_str() != Some(&digest) {
        return Err("Profile hash does not match".into());
    }
    value["name"] = encoded["name"]
        .as_str()
        .ok_or("Missing profile name")?
        .into();
    Ok(value)
}

pub fn read(path: &std::path::Path, target: &Value) -> Result<DecodedFile, String> {
    use std::io::Read as _;
    // Bound local parsing independently from vendor UI. No downloaded code runs.
    const MAX_BYTES: u64 = 16 * 1024 * 1024;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Profile file exceeds 16 MiB".into());
    }
    if bytes.starts_with(b"PK") {
        return Err("Synapse 3 migration is not connected".into());
    }
    // Current Ea reads the outer file with FileReader.readAsText(..., "utf-8"):
    // decode malformed sequences as replacement characters and consume its BOM.
    let text = String::from_utf8_lossy(&bytes);
    let document: Value = serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text))
        .map_err(|e| format!("Invalid .synapse4 document: {e}"))?;
    let rows = document["profiles"]
        .as_array()
        .ok_or("Missing profiles array")?;
    let matching_category =
        document["category"].is_string() && document["category"] == target["category"];
    let matching_pid = ["productId", "dongleId", "bleId"]
        .iter()
        .any(|key| same_number(&document["productId"], &target[*key]));
    if !matching_category && !matching_pid {
        return Err("No compatible profiles for this product".into());
    }
    let mut warning =
        matching_category && !same_number(&document["productId"], &target["productId"]);
    let mut decode = |rows: &[Value]| {
        rows.iter()
            .filter_map(|row| match entry(row) {
                Ok(value) => Some(value),
                Err(_) => {
                    warning = true;
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let mut profiles = decode(rows);
    let macros = decode(
        document["macros"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    );
    // Current 43/Da strips nested Dynamic Keystroke mappings for products
    // without is8kAnalogDevice, after validating the original payload hash.
    if !js_truthy(&target["is8kAnalogDevice"]) {
        for profile in &mut profiles {
            let mut mappings = profile["mappings"].as_array().cloned().unwrap_or_default();
            mappings.retain(|mapping| {
                !mapping["mapping"].as_array().is_some_and(|nested| {
                    nested
                        .iter()
                        .any(|mapping| js_truthy(&mapping["dynamicKeyStrokeGroup"]))
                })
            });
            profile["mappings"] = mappings.into();
        }
    }
    Ok(DecodedFile {
        profiles,
        macros,
        device_name: document["deviceName"].as_str().unwrap_or("").into(),
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::{base64, js_truthy, number, same_number, stable};
    use serde_json::json;

    #[test]
    fn source_truthiness_preserves_zero_and_empty_string_mappings() {
        for value in [json!(null), json!(false), json!(0), json!(-0.0), json!("")] {
            assert!(!js_truthy(&value));
        }
        for value in [json!(true), json!(1), json!("false"), json!([]), json!({})] {
            assert!(js_truthy(&value));
        }
        assert!(same_number(&json!(2636.0), &json!(2636)));
        assert!(!same_number(&json!("2636"), &json!(2636)));
        assert!(!same_number(&json!(null), &json!(null)));
    }

    #[test]
    fn decoder_uses_the_source_normalized_base64_alphabet() {
        assert_eq!(base64("e=y!J4I joxfQ==").unwrap(), br#"{"x":1}"#);
        assert_eq!(base64("-_8=").unwrap(), [251, 255]);
        assert!(base64("A").is_err());
    }

    #[test]
    fn stable_hash_text_uses_utf16_key_order_and_javascript_number_thresholds() {
        assert_eq!(
            stable(&json!({"\u{e000}":1,"\u{10000}":2})).unwrap(),
            "{\"\u{10000}\":2,\"\u{e000}\":1}"
        );
        for (value, expected) in [
            (json!(-0.0), "0"),
            (json!(1e-6), "0.000001"),
            (json!(1e-7), "1e-7"),
            (json!(1e20), "100000000000000000000"),
            (json!(1e21), "1e+21"),
        ] {
            assert_eq!(number(value.as_number().unwrap()).unwrap(), expected);
        }
    }
}
