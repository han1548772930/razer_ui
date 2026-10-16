//! Current 164 modules 6776/useR and 1398/Hh/Xi cache projection.
use super::*;

pub(super) fn device_info(spec: &Spec, device: &Device) -> serde_json::Value {
    let mut info = serde_json::Map::new();
    for key in [
        "productId",
        "vendorId",
        "category",
        "claimInterface",
        "dongleId",
        "bleId",
        "canPairTwoDevices",
        "isProductivity",
        "isMultiPairingSupported",
    ] {
        if let Some(value) = spec.config.get(key) {
            info.insert(key.into(), value.clone());
        }
    }
    info.insert(
        "deviceContainerId".into(),
        device.device_container_id.clone().into(),
    );
    info.insert("serialNumber".into(), device.serial_number.clone().into());
    info.insert("deviceName".into(), device.display_name().into());
    serde_json::Value::Object(info)
}

pub(super) fn is_dongle(device: &Device) -> bool {
    // MW connectControllerRzDevice branches on the product's dongleId first.
    // Wireless accessories without it use the actual extended-firmware query;
    // USB presence and a physical PID cannot establish this source state.
    match spec(device.product_id).config["dongleId"].as_u64() {
        Some(id) => u64::from(device.real_product_id) == id,
        None => device.dashboard.readonly_pairing_is_dongle == Some(true),
    }
}

fn number(value: Option<&serde_json::Value>) -> Option<u32> {
    let value = value?;
    match value {
        serde_json::Value::Number(value) => value.as_u64().and_then(|value| value.try_into().ok()),
        serde_json::Value::String(value) => {
            let parsed = value.trim().parse::<f64>().ok()?;
            (parsed.is_finite()
                && parsed.fract() == 0.
                && parsed >= 0.
                && parsed <= u32::MAX as f64)
                .then_some(parsed as u32)
        }
        serde_json::Value::Null => Some(0),
        serde_json::Value::Bool(value) => Some(u32::from(*value)),
        _ => None,
    }
}

pub(super) fn cached_peers(
    records: &serde_json::Value,
    info: &serde_json::Value,
    context: &context::DeviceContext,
) -> Vec<Peer> {
    let ids: Vec<_> = [number(info.get("productId")), number(info.get("dongleId"))]
        .into_iter()
        .flatten()
        .collect();
    let rows: Vec<_> = match records {
        serde_json::Value::Array(rows) => rows.iter().collect(),
        serde_json::Value::Object(rows) => rows.values().collect(),
        _ => Vec::new(),
    };
    rows.into_iter()
        .filter_map(|row| {
            // useR deliberately matches master/owner product/radio IDs and does
            // not require the cache record's container to equal the open window.
            let owner = row
                .get("master")
                .filter(|value| !value.is_null())
                .or_else(|| row.get("owner"))?;
            if ![
                number(owner.get("productId")),
                number(owner.get("dongleId")),
            ]
            .into_iter()
            .flatten()
            .any(|id| ids.contains(&id))
            {
                return None;
            }
            let product_id =
                number(row.get("productId")).or_else(|| number(row.get("dongleId")))?;
            let dongle_id = number(row.get("dongleId"));
            let locale = i18n::locale().to_lowercase();
            let label = row
                .get("productName")
                .filter(|value| !value.is_null())
                .or_else(|| row.get("name"));
            let name = label
                .and_then(|value| match value {
                    serde_json::Value::String(value) if !value.trim().is_empty() => {
                        Some(value.clone())
                    }
                    serde_json::Value::Object(names) => names
                        .get(&locale)
                        .or_else(|| names.get("en"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    _ => None,
                })
                .or_else(|| context.device_name(product_id, dongle_id))
                .unwrap_or_default();
            Some(Peer {
                id: format!(
                    "cache:{product_id}:{}:{}",
                    dongle_id.unwrap_or(0),
                    row["deviceContainerId"].as_str().unwrap_or("")
                ),
                name,
                product_id,
                dongle_id,
                edition: number(row.get("editionId")),
                layout: number(row.get("layoutId")),
                lane: if row["category"] == "KEYBOARD" {
                    Lane::Keyboard
                } else {
                    Lane::Mouse
                },
                payload: Some(row.clone()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[::core::prelude::v1::test]
    fn current_cache_supports_object_array_and_owner_alias_without_hardware_claim() {
        let info = json!({"productId":164});
        let row = json!({"productId":170,"dongleId":171,"name":"Stored label","owner":{"productId":"164"},"deviceContainerId":"another source window"});
        let context = context::DeviceContext::default();
        let array = cached_peers(&json!([row.clone()]), &info, &context);
        let object = cached_peers(&json!({"physical-key":row}), &info, &context);
        assert_eq!(array[0].name, "Stored label");
        assert_eq!(array[0].id, object[0].id);
        assert!(!context.connected(&array[0], 164));
        assert!(
            cached_peers(
                &json!([{"productId":170,"master":{"productId":241}}]),
                &info,
                &context
            )
            .is_empty()
        );
    }
}
