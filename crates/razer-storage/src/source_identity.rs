use anyhow::{Context as _, ensure};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Copy)]
pub enum CachePolicy {
    ReplaceProduct,
    ReplaceSerial,
}

fn merge(
    previous: Option<Value>,
    incoming: &Value,
    policy: CachePolicy,
) -> anyhow::Result<Option<Value>> {
    let record = incoming
        .as_object()
        .context("Connected identity is not an object")?;
    let product = incoming["productId"]
        .as_u64()
        .context("Connected identity product is absent")?;
    let serial = incoming["serialNumber"]
        .as_str()
        .context("Connected identity serial is absent")?;
    ensure!(
        !serial.is_empty(),
        "Empty queried serial cannot identify a cache record"
    );
    let edition = incoming["editionId"]
        .as_u64()
        .context("Connected identity edition is absent")?;
    let layout = incoming["layoutId"]
        .as_u64()
        .context("Connected identity layout is absent")?;
    let mut rows = previous
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let index = rows
        .iter()
        .position(|row| row["productId"] == product && row["serialNumber"] == serial);
    let mut merged = index
        .and_then(|index| rows[index].as_object().cloned())
        .unwrap_or_default();
    if index.is_some()
        && matches!(policy, CachePolicy::ReplaceProduct)
        && merged.get("editionId").and_then(Value::as_u64) == Some(0)
        && edition == 0
        && merged.get("layoutId").and_then(Value::as_u64) == Some(layout)
    {
        return Ok(None);
    }
    merged.extend(record.clone());
    let merged = Value::Object(merged);
    match (index, policy) {
        (Some(_), CachePolicy::ReplaceProduct) => {
            for row in &mut rows {
                if row["productId"] == product {
                    *row = merged.clone();
                }
            }
        }
        (Some(index), CachePolicy::ReplaceSerial) => rows[index] = merged,
        (None, _) => rows.push(merged),
    }
    Ok(Some(Value::Array(rows)))
}

pub fn publish(path: &Path, incoming: &Value, policy: CachePolicy) -> anyhow::Result<bool> {
    crate::source_local::update(path, "connectedDeviceInfo", |previous| {
        merge(previous, incoming, policy)
    })
}

#[cfg(test)]
mod tests {
    use super::{CachePolicy, merge};
    use serde_json::json;

    #[test]
    fn current_keyboard_cache_preserves_zero_edition_early_return_and_product_replacement() {
        let rows = json!([
            {"productId":691,"serialNumber":"first","editionId":0,"layoutId":3,"retained":true},
            {"productId":691,"serialNumber":"second","editionId":128,"layoutId":1},
            {"productId":756,"serialNumber":"other","editionId":0,"layoutId":1}
        ]);
        let same = json!({"productId":691,"serialNumber":"first","editionId":0,"layoutId":3,"name":{"en":"updated"}});
        assert!(
            merge(Some(rows.clone()), &same, CachePolicy::ReplaceProduct)
                .unwrap()
                .is_none()
        );
        let next = json!({"productId":691,"serialNumber":"first","editionId":128,"layoutId":4});
        let changed = merge(Some(rows.clone()), &next, CachePolicy::ReplaceProduct)
            .unwrap()
            .unwrap();
        assert_eq!(changed[0], changed[1]);
        assert_eq!(changed[0]["retained"], true);
        assert_eq!(changed[2], rows[2]);
    }

    #[test]
    fn newer_cache_replaces_only_matching_serial_and_never_adds_readiness() {
        let rows = json!([
            {"productId":182,"serialNumber":"first","editionId":0,"layoutId":0},
            {"productId":182,"serialNumber":"second","editionId":0,"layoutId":0}
        ]);
        let incoming = json!({"productId":182,"serialNumber":"first","editionId":128,"layoutId":0});
        let changed = merge(Some(rows.clone()), &incoming, CachePolicy::ReplaceSerial)
            .unwrap()
            .unwrap();
        assert_eq!(changed[0], incoming);
        assert_eq!(changed[1], rows[1]);
        assert!(changed[0].get("setupStatus").is_none());
        assert!(
            merge(
                None,
                &json!({"productId":691,"serialNumber":"","editionId":0,"layoutId":0}),
                CachePolicy::ReplaceProduct
            )
            .is_err()
        );
    }
}
