//! Shared current binding presentation; no platform I/O or remote catalogs.
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};

/// Current192 he() skips every presentation read for an empty filtered status
/// list. Te catches rejection of this whole chain, including local JSON parse
/// and USB enumeration, before taking its null-result restoration branch.
pub fn source192_binding_info(
    container: &str,
    rows: &[(u16, u8)],
    connected_raw: impl FnOnce() -> anyhow::Result<Option<String>>,
    enumerate: impl FnOnce() -> anyhow::Result<Value>,
    runtime: &[Value],
) -> anyhow::Result<Value> {
    let rows: Vec<_> = rows
        .iter()
        .filter(|(pid, _)| *pid != u16::MAX && *pid != 193)
        .map(|(pid, status)| (u32::from(*pid), *status))
        .collect();
    if rows.is_empty() {
        return Ok(json!([]));
    }
    let available = super::pairing_catalog()["available"]
        .as_array()
        .context("Current192 available-device presentation is absent")?;
    let connected: Value = serde_json::from_str(&connected_raw()?.unwrap_or_else(|| "[]".into()))
        .context("Current192 connectedDeviceInfo JSON parse failed")?;
    let physical = enumerate()?;
    let truthy = match &connected {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        _ => true,
    };
    let is_array = connected.is_array();
    let empty = Vec::new();
    let connected = if truthy {
        connected.as_array().unwrap_or(&empty)
    } else {
        &empty
    };
    for (raw, _) in &rows {
        let Some(product) = available.iter().find(|entry| entry["dongleId"] == *raw) else {
            continue;
        };
        // he calls .find only after a catalog match. A truthy non-array or a
        // null visited before the first matching record throws in that owner.
        ensure!(
            !truthy || is_array,
            "Current192 connectedDeviceInfo has no source find method"
        );
        for info in connected {
            ensure!(
                !info.is_null(),
                "Current192 connectedDeviceInfo.find visited null"
            );
            if info["productId"] == product["productId"] {
                break;
            }
        }
    }
    project_binding_rows_with_runtime(192, container, &rows, connected, &physical, runtime)
}

pub fn project_binding_rows_with_runtime(
    source_product_id: u32,
    container: &str,
    rows: &[(u32, u8)],
    connected: &[Value],
    physical: &Value,
    runtime: &[Value],
) -> anyhow::Result<Value> {
    ensure!(
        matches!(source_product_id, 164 | 179 | 192 | 241 | 756),
        "缺少当前绑定 caller"
    );
    ensure!(
        physical["complete"] == true,
        "物理接口枚举未完成，无法判断绑定角色"
    );
    let own_dongle = match source_product_id {
        756 => 757,
        192 => 193,
        _ => source_product_id,
    };
    let opposite_category = if source_product_id == 192 {
        "KEYBOARD"
    } else {
        "MOUSE"
    };
    let devices = super::pairing_catalog()["devices"]
        .as_array()
        .context("缺少本地兼容产品目录")?;
    let available = super::pairing_catalog()["available"]
        .as_array()
        .context("缺少本地可用产品目录")?;
    let interfaces = physical["interfaces"]
        .as_array()
        .context("缺少物理接口枚举")?;
    let mut projected = Vec::new();
    for &(raw, status) in rows {
        if raw == u16::MAX as u32 || raw == own_dongle {
            continue;
        }
        let role = if interfaces.iter().any(|row| {
            row["product_id"] == raw
                && row["device_container_id"].as_str().is_some_and(|id| {
                    if source_product_id == 192 {
                        id == container
                    } else {
                        id.eq_ignore_ascii_case(container)
                    }
                })
        }) {
            "master"
        } else {
            "slave"
        };
        let mut row = json!({"role":role,"name":{"en":format!("unknown-{raw}")},"productName":{"en":format!("unknown{raw}")},"editionName":{"en":format!("unknown{raw}")},"category":opposite_category,"editionId":0,"layoutId":0,"productId":raw,"status":status});
        let registered = available.iter().find(|entry| {
            entry["dongleId"] == raw || (source_product_id == 756 && entry["productId"] == raw)
        });
        if let Some(registered) = registered {
            let product_id = registered["productId"]
                .as_u64()
                .context("目录 productId 缺失")?;
            if let Some(info) = connected
                .iter()
                .find(|info| info["productId"] == product_id)
                .and_then(Value::as_object)
            {
                row = Value::Object(info.clone());
                // Current192 spreads {role}, then the connected JSON record,
                // then the raw response. A role field in that record wins.
                if source_product_id != 192 || !row.as_object().unwrap().contains_key("role") {
                    row["role"] = json!(role);
                }
                row["status"] = json!(status);
                if source_product_id == 192 {
                    let truthy = |value: &&Value| match value {
                        Value::Null => false,
                        Value::Bool(value) => *value,
                        Value::Number(value) => value.as_f64().is_some_and(|v| v != 0.),
                        Value::String(value) => !value.is_empty(),
                        _ => true,
                    };
                    // Current192 V() is false in the verified startup. `info`
                    // is JSON.parse(connectedDeviceInfo), so _e's conditional
                    // getHWModule function branch cannot be present on it.
                    let observed = (info.get("category") == Some(&json!("MOUSE")))
                        .then(|| {
                            [product_id, u64::from(raw)]
                                .into_iter()
                                .find_map(|product| {
                                    // 20236.ct reduces all WindowStorage values and
                                    // overwrites on every strict match: last wins.
                                    runtime
                                        .iter()
                                        .rfind(|row| {
                                            row["productId"] == product
                                                && row["deviceContainerId"] == container
                                        })
                                        .and_then(|row| row.get("sidepadLayout"))
                                        .filter(truthy)
                                        .cloned()
                                })
                        })
                        .flatten();
                    row["layoutId"] = observed
                        .or_else(|| info.get("layoutId").filter(truthy).cloned())
                        .unwrap_or(json!(0));
                }
            } else if let Some(device) = devices
                .iter()
                .find(|entry| entry["DeviceDonglePid"] == registered["dongleId"])
            {
                let names = device["ProductInfo"].as_array().and_then(|editions| {
                    editions
                        .iter()
                        .find(|e| e["EID"] == 0)
                        .or_else(|| editions.first())
                });
                if let Some(names) = names {
                    let name = json!({"en":names["Name"],"zh-cn":names["CHSName"]});
                    for key in ["name", "productName", "editionName"] {
                        row[key] = name.clone();
                    }
                    if source_product_id != 756 {
                        if device["DeviceType"]
                            .as_str()
                            .is_some_and(|value| !value.is_empty())
                        {
                            row["category"] = device["DeviceType"].clone();
                        }
                    }
                }
            }
            row["productId"] = json!(product_id);
            row["dongleId"] = json!(raw);
        }
        projected.push(row);
    }
    Ok(Value::Array(projected))
}

#[cfg(test)]
mod tests {
    use super::*;
    const OWNER: &str = "{11111111-2222-3333-4444-555555555555}";
    fn physical() -> anyhow::Result<Value> {
        Ok(json!({"complete":true,"interfaces":[{"product_id":183,"device_container_id":OWNER}]}))
    }

    #[test]
    fn current192_empty_status_skips_presentation_and_raw_unknown_ids_remain_nonempty() {
        let empty = source192_binding_info(
            OWNER,
            &[(193, 1), (65535, 1)],
            || panic!("empty getter must not load presentation"),
            || panic!("empty getter must not enumerate roles"),
            &[],
        )
        .unwrap();
        assert_eq!(empty, json!([]));
        let unknown =
            source192_binding_info(OWNER, &[(0, 0)], || Ok(Some("{}".into())), physical, &[])
                .unwrap();
        assert_eq!(unknown[0]["productId"], 0);
        assert_eq!(unknown[0]["status"], 0);
    }

    #[test]
    fn current192_he_rejects_presentation_after_a_successful_nonempty_getter() {
        let malformed = source192_binding_info(
            OWNER,
            &[(183, 1)],
            || Ok(Some("invalid-json".into())),
            || panic!("JSON.parse precedes role enumeration"),
            &[],
        )
        .unwrap_err();
        assert!(format!("{malformed:#}").contains("JSON parse"));
        let physical_error = source192_binding_info(
            OWNER,
            &[(183, 1)],
            || Ok(None),
            || anyhow::bail!("simulated role enumeration failure"),
            &[],
        )
        .unwrap_err();
        assert!(format!("{physical_error:#}").contains("simulated role enumeration failure"));
        for invalid in ["{}", "true", "1", "[null]"] {
            assert!(
                source192_binding_info(
                    OWNER,
                    &[(183, 1)],
                    || Ok(Some(invalid.into())),
                    physical,
                    &[]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn current192_he_uses_source_truthiness_and_first_connected_match_before_later_null() {
        for empty in ["null", "false", "0", "\"\"", "[]"] {
            let result = source192_binding_info(
                OWNER,
                &[(183, 7)],
                || Ok(Some(empty.into())),
                physical,
                &[],
            )
            .unwrap();
            assert_eq!(result[0]["productId"], 182);
            assert_eq!(result[0]["dongleId"], 183);
            assert_eq!(result[0]["status"], 7);
        }
        let raw = json!([{"productId":182,"category":"MOUSE","layoutId":4},null]).to_string();
        let result = source192_binding_info(
            OWNER,
            &[(183, 0), (183, 7)],
            || Ok(Some(raw)),
            physical,
            &[],
        )
        .unwrap();
        assert_eq!(result[0]["layoutId"], 4);
        assert_eq!(result[0]["role"], "master");
        assert_eq!(result[0]["status"], 0);
        assert_eq!(result[1]["status"], 7);
    }
}
