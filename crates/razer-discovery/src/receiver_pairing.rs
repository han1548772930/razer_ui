//! Current 164/241 DUALLINK_SCAN_DEVICE projection. Radio PID is a dongleId,
//! not automatically a productId or an online workspace identity.
use anyhow::{Context as _, ensure};
use razer_device::receiver_pairing::ScanCandidate;
use serde_json::{Value, json};
use std::sync::OnceLock;

fn catalog() -> &'static Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: Value = serde_json::from_str(include_str!(
            "../../../assets/data/receiver-pairing-catalog.json"
        ))
        .expect("source-generated pairing catalog");
        assert_eq!(catalog["schema_version"], 1);
        catalog
    })
}

/// Historical connectedDeviceInfo is not merged. Source runtime overrides need
/// separately retained owner/serial scope and real records. The source scan
/// helper does not copy raw keyboardLayout into UI layoutId; preserve absence.
pub fn project_scan_candidates(
    receiver_pid: u16,
    scanned: &[ScanCandidate],
) -> anyhow::Result<Value> {
    ensure!(
        matches!(receiver_pid, 164 | 241),
        "当前产品尚未核验扫描候选转换"
    );
    ensure!(scanned.len() <= 3, "扫描候选超出当前 Protocol25 记录数");
    let source = catalog();
    let devices = source["devices"]
        .as_array()
        .context("缺少当前兼容设备目录")?;
    let available = source["available"]
        .as_array()
        .context("缺少当前可用产品目录")?;
    let receiver = devices
        .iter()
        .find(|row| row["DeviceDonglePid"] == receiver_pid)
        .context("当前兼容目录没有接收器")?;
    let buddies = receiver["DevicePairingBuddies"]
        .as_array()
        .context("接收器缺少原始配对兼容列表")?;
    let mut result = Vec::new();
    for candidate in scanned {
        let raw = candidate.dongle_id;
        let Some(device) = devices.iter().find(|row| row["DeviceDonglePid"] == raw) else {
            continue;
        };
        if !buddies.iter().any(|pid| *pid == raw) {
            continue;
        }
        let editions = device["ProductInfo"]
            .as_array()
            .context("兼容产品没有版本名称列表")?;
        let selected = editions
            .iter()
            .find(|edition| edition["EID"] == candidate.edition_id)
            .or_else(|| editions.iter().find(|edition| edition["EID"] == 0));
        let default_en = format!("unknown-{raw}");
        let default_cn = format!("未知设备-{raw}");
        let name = json!({
            "en":selected.and_then(|row|row["Name"].as_str()).filter(|name|!name.is_empty()).unwrap_or(&default_en),
            "zh-cn":selected.and_then(|row|row["CHSName"].as_str()).filter(|name|!name.is_empty()).unwrap_or(&default_cn),
        });
        let Some(registered) = available.iter().find(|row| row["dongleId"] == raw) else {
            // Source unmatched branch defaults; zeros are not device reads.
            let product_id = device["DeviceSiblings"]
                .as_array()
                .and_then(|rows| rows.first())
                .and_then(Value::as_u64)
                .unwrap_or(0);
            result.push(json!({"productId":product_id,"dongleId":raw,"name":{"en":default_en},"productName":{"en":default_en},"editionName":{"en":default_en},"category":"MOUSE","editionId":0,"layoutId":0}));
            continue;
        };
        let product_id = registered["productId"]
            .as_u64()
            .filter(|pid| *pid > 0)
            .context("可用产品条目缺少真实 productId")?;
        let category = device["DeviceType"].as_str().context("兼容产品类别缺失")?;
        result.push(json!({"productId":product_id,"dongleId":raw,"name":name,"productName":name,"editionName":name,"category":category,"editionId":candidate.edition_id}));
    }
    Ok(Value::Array(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Compile fixtures only: current AGENTS prohibits executing tests.
    #[test]
    fn radio_pid_maps_to_actual_product_and_does_not_invent_layout() {
        let rows = project_scan_candidates(
            241,
            &[ScanCandidate {
                dongle_id: 713,
                edition_id: 128,
                keyboard_layout: 7,
            }],
        )
        .unwrap();
        assert_eq!(rows[0]["productId"], 716);
        assert_eq!(rows[0]["dongleId"], 713);
        assert_eq!(rows[0]["category"], "KEYBOARD");
        assert_eq!(rows[0]["editionId"], 128);
        assert!(rows[0].get("layoutId").is_none());
    }
    #[test]
    fn original_buddies_filter_precedes_candidate_exposure() {
        assert_eq!(
            project_scan_candidates(
                164,
                &[ScanCandidate {
                    dongle_id: 713,
                    edition_id: 0,
                    keyboard_layout: 0
                }]
            )
            .unwrap(),
            json!([])
        );
        assert_eq!(
            project_scan_candidates(
                241,
                &[ScanCandidate {
                    dongle_id: 183,
                    edition_id: 0,
                    keyboard_layout: 0
                }]
            )
            .unwrap()[0]["productId"],
            182
        );
    }
}
