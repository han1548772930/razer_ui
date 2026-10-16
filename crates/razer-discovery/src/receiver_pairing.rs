//! Current 164/241/756 DUALLINK_SCAN_DEVICE projection. Radio PID is a dongleId,
//! not automatically a productId or an online workspace identity.
use anyhow::{Context as _, ensure};
use razer_device::receiver_pairing::ScanCandidate;
use serde_json::{Value, json};

/// Source runtime overrides need separately retained owner/serial scope and
/// real records. This wrapper has no such observations to merge. The source
/// scan helper does not copy raw keyboardLayout into UI layoutId.
pub fn project_scan_candidates(
    receiver_pid: u16,
    scanned: &[ScanCandidate],
) -> anyhow::Result<Value> {
    project_scan_candidates_for_source(receiver_pid, None, scanned, &[])
}

pub fn project_scan_candidates_for_source(
    receiver_pid: u16,
    source_product_id: Option<u16>,
    scanned: &[ScanCandidate],
    connected: &[Value],
) -> anyhow::Result<Value> {
    let source_pid = if source_product_id == Some(756) || receiver_pid == 757 {
        757
    } else {
        receiver_pid
    };
    ensure!(
        matches!(source_pid, 164 | 179 | 241 | 757),
        "当前产品尚未核验扫描候选转换"
    );
    ensure!(scanned.len() <= 3, "扫描候选超出当前 Protocol25 记录数");
    let receiver = razer_device::receiver_pairing::catalog_device(source_pid)
        .context("当前兼容目录没有接收器")?;
    let buddies = receiver.pairing_buddies();
    let mut result = Vec::new();
    for candidate in scanned {
        let raw = candidate.dongle_id;
        let device = razer_device::receiver_pairing::catalog_device(raw);
        // Current756 handleScanDevices projects every actual event record;
        // Linker callers reject an absent catalog device before their buddies
        // filter. Neither branch turns a missing catalog row into a read.
        if source_pid != 757 && (device.is_none() || !buddies.contains(&raw)) {
            continue;
        }
        let editions = device.map_or(&[][..], |device| device.editions());
        let selected = editions
            .iter()
            .find(|edition| edition.edition_id() == u16::from(candidate.edition_id))
            .or_else(|| editions.iter().find(|edition| edition.edition_id() == 0));
        let default_en = format!("unknown-{raw}");
        let default_cn = format!("未知设备-{raw}");
        let name = json!({
            "en":selected.map(|edition|edition.name()).filter(|name|!name.is_empty()).unwrap_or(&default_en),
            "zh-cn":selected.map(|edition|edition.chinese_name()).filter(|name|!name.is_empty()).unwrap_or(&default_cn),
        });
        let product_id = device
            .and_then(|device| device.siblings())
            .and_then(|rows| rows.first())
            .map(|product| u64::from(*product))
            .unwrap_or(0);
        let category = device.map(|device| device.category()).unwrap_or("");
        let mut row = json!({"productId":product_id,"dongleId":raw,"name":name,"productName":name,"editionName":name,"category":category,"editionId":candidate.edition_id});
        let Some(registered) =
            razer_device::receiver_pairing::registered_source756_pair_product(raw)
        else {
            // Current756 spreads the actual candidate AFTER its presentation
            // defaults. Linker callers do the reverse. The distinction keeps
            // an unknown756 scan's real edition and empty category intact.
            let mut defaults = json!({"productId":product_id,"dongleId":raw,"name":{"en":default_en},"productName":{"en":default_en},"editionName":{"en":default_en},"category":"MOUSE","editionId":0,"layoutId":0});
            if source_pid == 757 {
                defaults
                    .as_object_mut()
                    .unwrap()
                    .extend(row.as_object().unwrap().clone());
            }
            result.push(defaults);
            continue;
        };
        let product_id = u64::from(registered);
        row["productId"] = json!(product_id);
        if let Some(observed) = connected
            .iter()
            .find(|row| row["productId"] == product_id)
            .and_then(Value::as_object)
        {
            row.as_object_mut().unwrap().extend(observed.clone());
        }
        result.push(row);
    }
    Ok(Value::Array(result))
}

/// Current756 event54 response reconstructs the candidate from the current
/// AvailableDevices/DualDongle tables. Names use the scanned edition; fields
/// use the post-pair getEdition reading or its explicit source failure defaults.
/// The caller must separately validate the matching hardware success event.
pub fn project_source756_pair_device(
    dongle_id: u16,
    scanned_edition: u8,
    edition_id: u8,
    layout_id: u8,
    connected: &[Value],
) -> anyhow::Result<Value> {
    let device = razer_device::receiver_pairing::source756_pair_device(dongle_id)?;
    let editions = device.editions();
    let selected = editions
        .iter()
        .find(|edition| edition.edition_id() == u16::from(scanned_edition))
        .or_else(|| editions.iter().find(|edition| edition.edition_id() == 0));
    let fallback_en = format!("unknown-{dongle_id}");
    let fallback_cn = format!("未知设备-{dongle_id}");
    let names = json!({"en":selected.map(|edition| edition.name()).filter(|name| !name.is_empty()).unwrap_or(&fallback_en),"zh-cn":selected.map(|edition| edition.chinese_name()).filter(|name| !name.is_empty()).unwrap_or(&fallback_cn)});
    let registered = razer_device::receiver_pairing::registered_source756_pair_product(dongle_id);
    let product_id = registered.unwrap_or(0);
    // The mounted Protocol25 pair parser supplies no typeDesc. Source falls
    // back to the opposite-category metadata factory's actual MOUSE category.
    let mut projected = json!({"productId":product_id,"dongleId":dongle_id,"category":"MOUSE","name":names,"productName":names,"editionName":names,"editionId":edition_id,"layoutId":layout_id});
    if registered.is_some() {
        if let Some(info) = connected
            .iter()
            .find(|row| row["productId"] == product_id)
            .and_then(Value::as_object)
        {
            projected = Value::Object(info.clone());
            projected["dongleId"] = json!(dongle_id);
        }
    }
    Ok(projected)
}

/// Current he/_e/te uses source connectedDeviceInfo only as a presentation
/// override. The raw status and dongle ID always come from this query. These
/// descriptive edition/layout defaults never become DeviceReadValues.
pub fn project_binding_info(
    source_product_id: u32,
    container: &str,
    query: &Value,
    connected: &[Value],
    physical: &Value,
) -> anyhow::Result<Value> {
    project_binding_info_with_runtime(
        source_product_id,
        container,
        query,
        connected,
        physical,
        &[],
    )
}

/// Runtime rows must come from the current device owners. They supply source
/// presentation only and never turn a local layout default into a device read.
pub fn project_binding_info_with_runtime(
    source_product_id: u32,
    container: &str,
    query: &Value,
    connected: &[Value],
    physical: &Value,
    runtime: &[Value],
) -> anyhow::Result<Value> {
    let rows = crate::discovery::receiver_peers(query)?;
    project_binding_rows_with_runtime(
        source_product_id,
        container,
        &rows,
        connected,
        physical,
        runtime,
    )
}

/// Current he() accepts raw zero/duplicate IDs; discovery's unique-peer
/// projection must not rewrite that independent source query's records.
pub fn project_binding_rows_with_runtime(
    source_product_id: u32,
    container: &str,
    rows: &[(u32, u8)],
    connected: &[Value],
    physical: &Value,
    runtime: &[Value],
) -> anyhow::Result<Value> {
    razer_device::receiver_pairing::project_binding_rows_with_runtime(
        source_product_id,
        container,
        rows,
        connected,
        physical,
        runtime,
    )
}

/// Current756 ge consults the logical slave cache first, then the live te
/// result. Failed te is caught by the caller and leaves the original master.
pub fn project_device_info(
    source_product_id: u32,
    records: &Value,
    bindings: Option<&Value>,
) -> anyhow::Result<Value> {
    let source_default = razer_device::receiver_pairing::catalog_master(source_product_id)
        .context("caller 主设备缺失")?;
    let default = json!({"productId": source_default.product_id(), "dongleId": source_default.dongle_id(), "bleId": source_default.ble_id()});
    let rows: Vec<_> = match records {
        Value::Object(rows) => rows.values().collect(),
        Value::Array(rows) => rows.iter().collect(),
        _ => Vec::new(),
    };
    let cached = rows
        .into_iter()
        .find(|row| row["productId"] == source_product_id);
    let master = cached.map(|row| &row["master"]).unwrap_or(&default);
    let slaves = if let Some(row) = cached {
        vec![json!({"productId":row["productId"],"dongleId":row["dongleId"]})]
    } else {
        bindings.and_then(Value::as_array).into_iter().flatten().map(|row| json!({"productId":row.get("productId").unwrap_or(&Value::Null),"dongleId":row.get("dongleId").unwrap_or(&Value::Null)})).collect()
    };
    Ok(
        json!({"master":{"productId":master["productId"],"dongleId":master["dongleId"],"bleId":master["bleId"]},"slaves":slaves}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const OWNER: &str = "{11111111-2222-3333-4444-555555555555}";

    #[test]
    fn source192_binding_projection_preserves_raw_zero_duplicate_status_and_order() {
        let physical =
            json!({"complete":true,"interfaces":[{"product_id":183,"device_container_id":OWNER}]});
        let rows = project_binding_rows_with_runtime(
            192,
            OWNER,
            &[(193, 1), (0, 0), (183, 7), (183, 0), (65535, 1)],
            &[],
            &physical,
            &[],
        )
        .unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 3);
        assert_eq!(rows[0]["productId"], 0);
        assert_eq!(rows[0]["status"], 0);
        assert_eq!(rows[1]["dongleId"], 183);
        assert_eq!(rows[1]["status"], 7);
        assert_eq!(rows[2]["dongleId"], 183);
        assert_eq!(rows[2]["status"], 0);
        assert_eq!(rows[1]["role"], "master");
        let other_container = OWNER.to_uppercase().replace("11111111", "AAAAAAAA");
        assert_eq!(
            project_binding_rows_with_runtime(
                192,
                &other_container,
                &[(183, 1)],
                &[],
                &physical,
                &[]
            )
            .unwrap()[0]["role"],
            "slave"
        );
    }

    #[test]
    fn source192_layout_uses_last_exact_runtime_then_radio_then_plain_record() {
        let query =
            json!({"devices":[{"product_id":193,"status":1},{"product_id":183,"status":0}]});
        let connected = [json!({"productId":182,"category":"MOUSE","layoutId":7})];
        let physical =
            json!({"complete":true,"interfaces":[{"product_id":183,"device_container_id":OWNER}]});
        let runtime = [
            json!({"productId":182,"deviceContainerId":OWNER,"sidepadLayout":2}),
            json!({"productId":182,"deviceContainerId":OWNER,"sidepadLayout":4}),
            json!({"productId":183,"deviceContainerId":OWNER,"sidepadLayout":6}),
        ];
        let projected =
            project_binding_info_with_runtime(192, OWNER, &query, &connected, &physical, &runtime)
                .unwrap();
        assert_eq!(projected[0]["layoutId"], 4);
        assert_eq!(projected[0]["status"], 0);
        assert_eq!(projected[0]["role"], "master");
        let mut runtime = runtime;
        runtime[1]["sidepadLayout"] = json!(0);
        assert_eq!(
            project_binding_info_with_runtime(192, OWNER, &query, &connected, &physical, &runtime)
                .unwrap()[0]["layoutId"],
            6
        );
        runtime[2]["deviceContainerId"] = json!(OWNER.to_lowercase().replace('1', "A"));
        assert_eq!(
            project_binding_info_with_runtime(192, OWNER, &query, &connected, &physical, &runtime)
                .unwrap()[0]["layoutId"],
            7
        );
        assert_eq!(
            project_binding_info(192, OWNER, &query, &connected, &physical).unwrap()[0]["layoutId"],
            7
        );
        let keyboard = [json!({"productId":182,"category":"KEYBOARD","layoutId":3})];
        assert_eq!(
            project_binding_info_with_runtime(192, OWNER, &query, &keyboard, &physical, &runtime)
                .unwrap()[0]["layoutId"],
            3
        );
    }

    #[test]
    fn source756_pair_reconstructs_unregistered_ui_without_reusing_scan_identity() {
        let projected = project_source756_pair_device(
            180,
            128,
            5,
            6,
            &[json!({"productId":181,"editionId":77,"productName":{"en":"Local override"}})],
        )
        .unwrap();
        assert_eq!(projected["productId"], 0);
        assert_eq!(projected["dongleId"], 180);
        assert_eq!(projected["category"], "MOUSE");
        assert_eq!(projected["editionId"], 5);
        assert_eq!(projected["layoutId"], 6);
        assert_eq!(projected["productName"]["en"], "Razer Naga V2 Hyperspeed");
        let no_sibling = project_source756_pair_device(241, 0, 0, 0, &[]).unwrap();
        assert_eq!(no_sibling["productId"], 0);
        assert_eq!(no_sibling["category"], "MOUSE");
        assert!(project_source756_pair_device(60000, 129, 0, 0, &[]).is_err());
    }

    #[test]
    fn source756_pair_names_use_scanned_edition_and_connected_override_keeps_radio() {
        let projected = project_source756_pair_device(183, 128, 0, 0, &[]).unwrap();
        assert_eq!(projected["productId"], 182);
        assert_eq!(projected["editionId"], 0);
        assert_eq!(projected["layoutId"], 0);
        assert_eq!(
            projected["productName"]["en"],
            "RAZER DEATHADDER V3 PRO WHITE EDITION"
        );
        let connected = json!({"productId":182,"dongleId":999,"editionId":9,"layoutId":3,"category":"MOUSE","productName":{"en":"Observed presentation"},"sourceField":"retained"});
        let merged = project_source756_pair_device(183, 128, 0, 0, &[connected.clone()]).unwrap();
        let mut expected = connected;
        expected["dongleId"] = json!(183);
        assert_eq!(merged, expected);
    }

    #[test]
    fn source756_binding_override_keeps_raw_status_and_actual_master_role() {
        let query = json!({"devices":[{"product_id":757,"status":1},{"product_id":183,"status":0},{"product_id":65535,"status":0}]});
        let connected = [
            json!({"productId":182,"category":"MOUSE","editionId":128,"layoutId":7,"status":1,"dongleId":999,"productName":{"en":"Local edition"}}),
        ];
        let physical =
            json!({"complete":true,"interfaces":[{"product_id":183,"device_container_id":OWNER}]});
        let rows = project_binding_info(756, OWNER, &query, &connected, &physical).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["editionId"], 128);
        assert_eq!(rows[0]["layoutId"], 7);
        assert_eq!(rows[0]["status"], 0);
        assert_eq!(rows[0]["dongleId"], 183);
        assert_eq!(rows[0]["role"], "master");
        assert!(
            project_binding_info(756, OWNER, &query, &connected, &json!({"complete":false}))
                .is_err()
        );
    }

    #[test]
    fn source756_device_info_falls_back_and_cached_slave_retains_original_master() {
        let defaults = project_device_info(756, &json!({}), None).unwrap();
        assert_eq!(
            defaults,
            json!({"master":{"productId":756,"dongleId":757,"bleId":758},"slaves":[]})
        );
        let records = json!({"physical-key":{"productId":756,"dongleId":757,"master":{"productId":241,"dongleId":241,"bleId":0}}});
        let value = project_device_info(
            756,
            &records,
            Some(&json!([{"productId":182,"dongleId":183}])),
        )
        .unwrap();
        assert_eq!(value["master"]["productId"], 241);
        assert_eq!(value["slaves"], json!([{"productId":756,"dongleId":757}]));
        assert!(value["master"].get("claimInterface").is_none());
    }

    #[test]
    fn source756_scan_uses_its_source_projection_on_a_cached_physical_master() {
        let result = project_scan_candidates_for_source(
            241,
            Some(756),
            &[ScanCandidate {
                dongle_id: 183,
                edition_id: 128,
                keyboard_layout: 9,
            }],
            &[json!({"productId":182,"layoutId":6})],
        )
        .unwrap();
        assert_eq!(result[0]["productId"], 182);
        assert_eq!(result[0]["layoutId"], 6);
        assert_eq!(result[0]["editionId"], 128);
    }

    #[test]
    fn source756_scan_preserves_unknown_and_unregistered_event_records() {
        let rows = project_scan_candidates_for_source(
            757,
            Some(756),
            &[
                ScanCandidate {
                    dongle_id: 60000,
                    edition_id: 129,
                    keyboard_layout: 7,
                },
                ScanCandidate {
                    dongle_id: 180,
                    edition_id: 128,
                    keyboard_layout: 6,
                },
            ],
            &[],
        )
        .unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert_eq!(
            rows[0],
            json!({"productId":0,"dongleId":60000,"name":{"en":"unknown-60000","zh-cn":"未知设备-60000"},"productName":{"en":"unknown-60000","zh-cn":"未知设备-60000"},"editionName":{"en":"unknown-60000","zh-cn":"未知设备-60000"},"category":"","editionId":129,"layoutId":0})
        );
        assert_eq!(rows[1]["productId"], 181);
        assert_eq!(rows[1]["editionId"], 128);
        assert_eq!(rows[1]["category"], "MOUSE");
        assert_eq!(rows[1]["productName"]["en"], "Razer Naga V2 Hyperspeed");
        assert_eq!(rows[1]["layoutId"], 0);
        assert_eq!(
            project_scan_candidates(
                241,
                &[ScanCandidate {
                    dongle_id: 60000,
                    edition_id: 129,
                    keyboard_layout: 7
                }]
            )
            .unwrap(),
            json!([])
        );
    }
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
