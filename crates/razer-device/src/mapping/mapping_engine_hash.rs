//! Active factory hash: top-level exclusions, stable UTF-16 key ordering,
//! JSON number formatting, '<' escaping, then MD5 of UTF-8 bytes.
use anyhow::{Result, ensure};
use serde_json::Value;

/// Stable JSON used by the current profile-file reader. Its hash omits the
/// middleware's additional '<' escaping, so keep the two call chains distinct.
pub fn canonical_json(value: &Value) -> Result<String> {
    let mut result = String::new();
    stable_json(value, &mut result)?;
    Ok(result)
}

pub fn canonical_hash_input(value: &Value, exclusions: &[&str]) -> Result<String> {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        for key in exclusions {
            object.remove(*key);
        }
    }
    let mut result = String::new();
    stable_json(&value, &mut result)?;
    Ok(result.replace('<', "\\u003C"))
}

pub fn source_hash(value: &Value) -> Result<String> {
    source_hash_with_exclusions(value, &["hash", "gamemode"])
}

pub fn source_hash_with_exclusions(value: &Value, exclusions: &[&str]) -> Result<String> {
    Ok(format!(
        "{:x}",
        md5::compute(canonical_hash_input(value, exclusions)?)
    ))
}

fn stable_json(value: &Value, out: &mut String) -> Result<()> {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            // Array.prototype.sort uses UTF-16 code units, not Unicode scalars.
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key)?);
                out.push(':');
                stable_json(&object[key], out)?;
            }
            out.push('}');
        }
        Value::Array(array) => {
            out.push('[');
            for (index, value) in array.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                stable_json(value, out)?;
            }
            out.push(']');
        }
        Value::Number(number) => {
            let f = number
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("Non-JS number"))?;
            ensure!(f.is_finite(), "Non-finite hash input");
            if number.is_i64() || number.is_u64() {
                ensure!(
                    f.abs() <= 9_007_199_254_740_991.0,
                    "Integer exceeds exact JS range"
                );
            }
            out.push_str(&crate::ecmascript_number::to_string(f)?);
        }
        other => out.push_str(&serde_json::to_string(other)?),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_hash_excludes_only_top_level_and_escapes_less_than() {
        let v = json!({"z":"<tag>","hash":"old","gamemode":true,"a":{"hash":7,"gamemode":false}});
        assert_eq!(
            canonical_hash_input(&v, &["hash", "gamemode"]).unwrap(),
            "{\"a\":{\"gamemode\":false,\"hash\":7},\"z\":\"\\u003Ctag>\"}"
        );
        assert_eq!(
            source_hash(&json!({})).unwrap(),
            "99914b932bd37a50b983c5e7c90ae93b"
        );
        assert_eq!(
            source_hash(&json!({"events":[]})).unwrap(),
            "7c29ae49a296fae31b7a8f763361d24e"
        );
    }

    #[test]
    fn js_numbers_and_utf16_sort_are_preserved() {
        let v = json!({"\u{e000}":1.0,"\u{10000}":-0.0,"n":[1e-7,1e-6,1e20,1e21]});
        assert_eq!(
            canonical_hash_input(&v, &[]).unwrap(),
            "{\"n\":[1e-7,0.000001,100000000000000000000,1e+21],\"𐀀\":0,\"\":1}"
        );
        assert!(source_hash(&json!(9_007_199_254_740_992u64)).is_err());
    }
}
