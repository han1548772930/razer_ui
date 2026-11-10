//! Current keyboard OBM macro worker conversion and firmware item codec.
//! Editor source JSON is decoded once; storage receives only validated bytes.
use crate::onboard_macro::MacroWriteRequest;
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProfileMacroReferences {
    pub guid: String,
    pub macro_guids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct MacroSynchronization {
    pub profiles: Vec<ProfileMacroReferences>,
    pub macros: Vec<MacroWriteRequest>,
    /// OS entropy is allocated by the shell, then masked and checked against
    /// actual firmware IDs by the storage owner when it executes the request.
    pub random_candidates: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct CompiledMacro {
    request: MacroWriteRequest,
    comparison_items: Vec<crate::onboard_macro::decode::Item>,
}

impl CompiledMacro {
    pub fn request(&self) -> &MacroWriteRequest {
        &self.request
    }
    pub fn comparison_items(&self) -> &[crate::onboard_macro::decode::Item] {
        &self.comparison_items
    }
    pub fn into_request(self) -> MacroWriteRequest {
        self.request
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MacroDocument {
    guid: String,
    name: String,
    app_engine: Engine,
    #[serde(default)]
    macro_events: Vec<EditorEvent>,
}
#[derive(Deserialize)]
struct EditorEvent {
    #[serde(rename = "Type")]
    kind: Value,
}

#[derive(Deserialize)]
struct SourceProfile {
    guid: String,
    #[serde(default)]
    mappings: Vec<SourceMapping>,
}

#[derive(Deserialize)]
enum SourceInputKind {
    AnalogInput,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct SourceMapping {
    #[serde(rename = "inputType")]
    input_type: SourceInputKind,
    #[serde(default)]
    mapping: Vec<Value>,
    #[serde(flatten)]
    output: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(tag = "outputType")]
enum SourceOutput {
    #[serde(rename = "macroGroup")]
    Macro {
        #[serde(rename = "macroGroup")]
        reference: SourceMacroReference,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct SourceMacroReference {
    guid: String,
}

fn profile_macro_references(value: &Value) -> anyhow::Result<ProfileMacroReferences> {
    let profile: SourceProfile = serde_json::from_value(value.clone())?;
    let mut macro_guids = Vec::new();
    for row in profile.mappings {
        let output = match row.input_type {
            SourceInputKind::AnalogInput => row
                .mapping
                .into_iter()
                .next()
                .context("Analog macro mapping primary stage is absent")?,
            SourceInputKind::Other => Value::Object(row.output),
        };
        // An unreadable primary stage must not become an empty reference set:
        // the device reconciler uses that set to decide which macros to delete.
        if let SourceOutput::Macro { reference } = serde_json::from_value(output)? {
            if !macro_guids.contains(&reference.guid) {
                macro_guids.push(reference.guid);
            }
        }
    }
    Ok(ProfileMacroReferences {
        guid: profile.guid,
        macro_guids,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Engine {
    mouse_move_type: String,
    events: Vec<Value>,
}
#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Event {
    Keyboard {
        #[serde(default)]
        scancode: Option<u32>,
        #[serde(rename = "HID")]
        hid: Option<u16>,
        flag: u32,
    },
    Mouse {
        id: u32,
        #[serde(default)]
        data: Option<i64>,
    },
    Delay {
        ms: f64,
    },
    MouseMove {
        x: f64,
        y: f64,
    },
    #[serde(other)]
    Unsupported,
}

pub fn prepare_synchronization(
    product: u32,
    profiles: &[Value],
    documents: &[Value],
    random_candidates: Vec<u16>,
) -> anyhow::Result<MacroSynchronization> {
    ensure!(
        matches!(product, 740 | 746),
        "Onboard macro editor caller product differs"
    );
    let profiles = profiles
        .iter()
        .map(profile_macro_references)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut macros = Vec::new();
    for document in documents {
        if let Some(compiled) = compile_document(product, document)? {
            macros.push(compiled.into_request());
        }
    }
    Ok(MacroSynchronization {
        profiles,
        macros,
        random_candidates,
    })
}

/// Decodes the source document at the interchange boundary. The current
/// worker and key-table identities qualify every registered OBM keyboard.
pub fn compile_document(product: u32, document: &Value) -> anyhow::Result<Option<CompiledMacro>> {
    ensure!(
        crate::keyboard_profile_inventory::supports(product),
        "Macro compiler product has no current keyboard OBM source"
    );
    let document: MacroDocument = serde_json::from_value(document.clone())?;
    if matches!(
        document.app_engine.mouse_move_type.as_str(),
        "absolute" | "foreground"
    ) || document.macro_events.iter().any(|event| {
        event
            .kind
            .as_f64()
            .is_some_and(|kind| [6.0, 5.0, 4.0, 10.0].contains(&kind))
    }) {
        return Ok(None);
    }
    compile(product, document).map(Some)
}

/// The three current OBM worker key-table variants have identical ordered
/// scancode/HID/flag/name/key-code projections. Their differences are virtual-key input
/// conversion fields, which are not consumed by this firmware compiler.
pub(crate) fn keyboard_keys(
    product: u32,
) -> anyhow::Result<&'static [crate::mapping_engine::data::KeyDefinition]> {
    ensure!(
        crate::keyboard_profile_inventory::supports(product),
        "Macro key table product has no current keyboard OBM source"
    );
    crate::mapping_engine::source_keys(679).context("Current shared OBM macro key table is absent")
}

fn wrapped_u16(value: f64) -> anyhow::Result<u16> {
    ensure!(value.is_finite(), "Macro coordinate is not finite");
    Ok(value.trunc().rem_euclid(65536.) as u16)
}

fn compile(product: u32, document: MacroDocument) -> anyhow::Result<CompiledMacro> {
    use crate::onboard_macro::decode::Item;
    let keys = keyboard_keys(product)?;
    ensure!(
        matches!(
            document.app_engine.mouse_move_type.as_str(),
            "none" | "relative"
        ),
        "Macro movement mode has no onboard worker conversion"
    );
    let relative = document.app_engine.mouse_move_type == "relative";
    let events = document
        .app_engine
        .events
        .iter()
        .map(|event| serde_json::from_value::<Event>(event.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut position = events.iter().find_map(|event| match event {
        Event::MouseMove { x, y } => Some((*x, *y)),
        _ => None,
    });
    let mut data = Vec::new();
    let mut worker_items = Vec::new();
    let mut comparison_items = Vec::new();
    let mut num_items = 0u32;
    let mut mouse_state = 0u8;
    for (event, original) in events.into_iter().zip(document.app_engine.events) {
        let mut worker_item = original;
        match event {
            Event::Keyboard {
                scancode,
                hid,
                flag,
            } => {
                let flag_base = flag - flag % 2;
                let hid = if let Some(scancode) = scancode.filter(|code| *code != 0) {
                    keys.iter()
                        .find(|key| {
                            key.scancode == scancode
                                && key
                                    .output_flag
                                    .filter(|flag| *flag != 0)
                                    .unwrap_or(key.flag)
                                    == flag_base
                        })
                        .and_then(|key| key.hid)
                        .context("Macro keyboard scancode/flag has no source HID")?
                        .parse::<u16>()?
                } else {
                    hid.context("Macro keyboard event has no physical scancode or HID")?
                };
                let worker_scancode = crate::keyboard_magnetic_mapping::macro_scancode(hid);
                if scancode.is_some_and(|scan| scan != 0) {
                    worker_item["HID"] = json!(hid);
                }
                if let Some(scancode) = worker_scancode {
                    worker_item["scancode"] = json!(scancode);
                }
                worker_item["flag"] = json!(flag % 2);
                comparison_items.push(Item::Keyboard {
                    hid,
                    scancode: worker_scancode.or(scancode),
                    key_name: None,
                    flag: (flag % 2) as u8,
                });
                let hid =
                    source_fw_hid(hid).context("Macro HID has no current firmware encoding")?;
                data.extend_from_slice(&[if flag % 2 == 0 { 1 } else { 2 }, hid]);
            }
            Event::Mouse { id, data: wheel } => {
                comparison_items.push(Item::Mouse {
                    id: Some(id),
                    data: wheel,
                });
                for (mask, bit) in [(1, 1), (4, 2), (16, 4), (64, 8), (256, 16)] {
                    if id & mask != 0 {
                        mouse_state |= bit;
                    }
                }
                // The retained source tests bit16 in both branches. Preserve
                // that actual codec behavior, including the middle-button row.
                for (mask, bit) in [(2, 1), (8, 2), (16, 4), (128, 8), (512, 16)] {
                    if id & mask != 0 {
                        mouse_state &= !bit;
                    }
                }
                if id == 1024 {
                    data.extend_from_slice(&[10, if wheel == Some(120) { 1 } else { 255 }]);
                } else {
                    data.extend_from_slice(&[8, mouse_state]);
                }
            }
            Event::Delay { ms } => {
                ensure!(ms.is_finite(), "Macro delay is invalid");
                if ms == 0. {
                    continue;
                }
                comparison_items.push(Item::Delay { milliseconds: ms });
                let count = if ms < 256. {
                    1
                } else if ms < 65536. {
                    2
                } else if ms < 16777216. {
                    3
                } else {
                    4
                };
                let ms = ms.trunc().rem_euclid(4294967296.) as u32;
                data.push(16 + count as u8);
                data.extend_from_slice(&ms.to_be_bytes()[4 - count..]);
            }
            Event::MouseMove { x, y } => {
                let (x, y) = if relative {
                    let previous = position.context("Relative macro origin is absent")?;
                    position = Some((x, y));
                    let relative_value = |before, after| {
                        let delta = after - before;
                        if delta < 0. { 65536. + delta } else { delta }
                    };
                    (relative_value(previous.0, x), relative_value(previous.1, y))
                } else {
                    (x, y)
                };
                // The worker removes zero moves after getRelativeValue, before
                // the firmware encoder truncates each coordinate to 16 bits.
                if x == 0. && y == 0. {
                    continue;
                }
                if relative {
                    worker_item = json!({"type":"mouseMove","x":x,"y":y});
                }
                comparison_items.push(Item::Cursor { x, y });
                data.push(21);
                data.extend_from_slice(&wrapped_u16(x)?.to_be_bytes());
                data.extend_from_slice(&wrapped_u16(y)?.to_be_bytes());
            }
            Event::Unsupported => anyhow::bail!("Macro contains an event without a firmware codec"),
        }
        worker_items.push(worker_item);
        num_items = num_items
            .checked_add(1)
            .context("Macro item count overflow")?;
    }
    Ok(CompiledMacro {
        request: MacroWriteRequest {
            guid: document.guid,
            name: document.name,
            num_items,
            timestamp: String::new(),
            hash: crate::mapping_engine_hash::source_hash(&json!(worker_items))?,
            data,
        },
        comparison_items,
    })
}

/// Current worker HID→scancode followed by macroItemToFWData scancode→HID.
/// Most values compose to identity; these source exceptions do not.
fn source_fw_hid(hid: u16) -> Option<u8> {
    match hid {
        50 => Some(49),
        148 => Some(115),
        4..=49 | 51..=115 | 133 | 135..=140 | 144..=147 | 224..=231 => Some(hid as u8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_current_obm_products_keep_worker_values_before_firmware_wrapping() {
        let products = [
            550, 551, 552, 577, 579, 590, 599, 600, 602, 614, 617, 619, 620, 642, 647, 653, 658,
            661, 664, 677, 678, 679, 688, 691, 697, 708, 716, 717, 719, 720, 721, 724, 727, 728,
            739, 740, 741, 742, 746, 747, 752, 756,
        ];
        for product in products {
            let document = json!({"guid":"11223344-5566-7788-99aa-bbccddeeff12", "name":"wire boundaries",
            "macroEvents":[], "appEngine":{"mouseMoveType":"none", "events":[
                {"type":"keyboard","scancode":30,"flag":0},
                {"type":"delay","ms":-1.5},
                {"type":"mouseMove","x":65536,"y":1.5},
                {"type":"keyboard","scancode":30,"flag":1}
            ]}});
            let result = compile_document(product, &document).unwrap().unwrap();
            assert_eq!(
                result.request().data,
                [1, 4, 17, 255, 21, 0, 0, 0, 1, 2, 4],
                "PID{product}"
            );
            assert!(matches!(
                result.comparison_items()[1],
                crate::onboard_macro::decode::Item::Delay { milliseconds: -1.5 }
            ));
            assert!(matches!(
                result.comparison_items()[2],
                crate::onboard_macro::decode::Item::Cursor { x: 65536.0, y: 1.5 }
            ));
            let decoded =
                crate::onboard_macro::decode::keyboard_items(product, &result.request().data, 11)
                    .unwrap();
            assert!(crate::onboard_macro::decode::differs(
                result.comparison_items(),
                &decoded.items
            ));
        }
    }

    #[test]
    fn source_eligibility_compares_numeric_types_without_coercing_strings() {
        let mut document = json!({"guid":"one","name":"one","macroEvents":[{"Type":6.0}],
            "appEngine":{"mouseMoveType":"none","events":[]}});
        assert!(compile_document(739, &document).unwrap().is_none());
        document["macroEvents"][0]["Type"] = json!("6");
        assert!(compile_document(739, &document).unwrap().is_some());
    }

    #[test]
    fn profile_references_keep_source_order_and_only_the_primary_analog_stage() {
        for product in [740, 746] {
            let profiles = [json!({"guid":"profile", "mappings":[
                {"inputType":"KeyboardInput", "outputType":"macroGroup", "macroGroup":{"guid":"first"}},
                {"inputType":"AnalogInput", "mapping":[
                    {"outputType":"macroGroup", "macroGroup":{"guid":"second"}},
                    {"outputType":"macroGroup", "macroGroup":{"guid":"secondary-stage"}}
                ]},
                {"inputType":"KeyboardInput", "outputType":"macroGroup", "macroGroup":{"guid":"first"}},
                {"inputType":"KeyboardInput", "outputType":"disableGroup"}
            ]})];
            let result = prepare_synchronization(product, &profiles, &[], vec![]).unwrap();
            assert_eq!(
                result.profiles,
                [ProfileMacroReferences {
                    guid: "profile".into(),
                    macro_guids: vec!["first".into(), "second".into()],
                }]
            );
        }
    }

    #[test]
    fn malformed_mapping_cannot_become_an_empty_macro_reference_set() {
        for product in [740, 746] {
            for row in [
                Value::Null,
                json!({"inputType":"AnalogInput"}),
                json!({"inputType":"AnalogInput", "mapping":[]}),
                json!({"inputType":"AnalogInput", "mapping":[null]}),
                json!({"inputType":"AnalogInput", "mapping":[{}]}),
                json!({"inputType":"KeyboardInput"}),
                json!({"outputType":"macroGroup", "macroGroup":{"guid":"used"}}),
                json!({"inputType":"KeyboardInput", "outputType":"macroGroup", "macroGroup":{"guid":5}}),
            ] {
                let profile = json!({"guid":"profile", "mappings":[row]});
                assert!(
                    prepare_synchronization(product, &[profile.clone()], &[], vec![]).is_err(),
                    "PID{product}: {profile}"
                );
            }
        }
    }

    #[test]
    fn worker_relative_motion_delay_widths_and_source_mouse_state_are_preserved() {
        let context = prepare_synchronization(740, &[], &[json!({"guid":"11223344-5566-7788-99aa-bbccddeeff12","name":"sample",
            "macroEvents":[],"appEngine":{"mouseMoveType":"relative","events":[
                {"type":"mouseMove","x":100,"y":50},
                {"type":"mouseMove","x":98,"y":55},
                {"type":"delay","ms":0},{"type":"delay","ms":255},{"type":"delay","ms":256},
                {"type":"delay","ms":65536},{"type":"delay","ms":16777216},
                {"type":"mouse","id":16},{"type":"mouse","id":1024,"data":120},
                {"type":"keyboard","scancode":30,"flag":0},{"type":"keyboard","scancode":30,"flag":1}
            ]}})], vec![7]).unwrap();
        let request = &context.macros[0];
        assert_eq!(request.num_items, 9);
        assert_eq!(
            request.data,
            [
                21, 255, 254, 0, 5, 17, 255, 18, 1, 0, 19, 1, 0, 0, 20, 1, 0, 0, 0, 8, 0, 10, 1, 1,
                4, 2, 4,
            ]
        );
    }

    #[test]
    fn worker_rejects_unknown_event_and_filters_source_unsupported_macros() {
        let document = json!({"guid":"one","name":"one","appEngine":{"mouseMoveType":"none","events":[{"type":"randomDelay","minMs":1,"maxMs":2}]}});
        assert!(prepare_synchronization(746, &[], &[document], vec![]).is_err());
        let context = prepare_synchronization(746, &[], &[json!({"guid":"text","name":"text","macroEvents":[{"Type":4}],"appEngine":{"mouseMoveType":"none","events":[{"type":"clipboard"}]}})], vec![]).unwrap();
        assert!(context.macros.is_empty());
    }

    #[test]
    fn worker_delay_width_uses_the_source_number_before_wire_truncation() {
        for product in [740, 746] {
            let document = json!({
                "guid":"11223344-5566-7788-99aa-bbccddeeff12", "name":"delay boundaries",
                "macroEvents":[], "appEngine":{"mouseMoveType":"none","events":[
                    {"type":"delay","ms":255.5},
                    {"type":"delay","ms":256.0},
                    {"type":"delay","ms":16777215.5},
                    {"type":"delay","ms":4294967295.0},
                    {"type":"delay","ms":4294967296.0},
                    {"type":"delay","ms":4294967552.0}
                ]}
            });
            let result = prepare_synchronization(product, &[], &[document], vec![7]).unwrap();
            assert_eq!(result.macros[0].num_items, 6);
            assert_eq!(
                result.macros[0].data,
                [
                    17, 255, 18, 1, 0, 19, 255, 255, 255, 20, 255, 255, 255, 255, 20, 0, 0, 0, 0,
                    20, 0, 0, 1, 0
                ],
                "PID{product}"
            );
        }
    }

    #[test]
    fn worker_filters_relative_moves_after_source_conversion_and_keeps_origin() {
        for product in [740, 746] {
            let document = json!({
                "guid":"11223344-5566-7788-99aa-bbccddeeff12", "name":"relative boundaries",
                "macroEvents":[], "appEngine":{"mouseMoveType":"relative","events":[
                    {"type":"mouseMove","x":65536,"y":0},
                    {"type":"mouseMove","x":0,"y":0},
                    {"type":"mouseMove","x":-65536,"y":0},
                    {"type":"mouseMove","x":-65537,"y":1},
                    {"type":"mouseMove","x":-1,"y":1}
                ]}
            });
            let result = prepare_synchronization(product, &[], &[document], vec![]).unwrap();
            let request = &result.macros[0];
            assert_eq!(request.num_items, 2, "PID{product}");
            assert_eq!(request.data, [21, 255, 255, 0, 1, 21, 0, 0, 0, 0]);
            // Positive 65536 survives the source zero filter even though its
            // later wire representation is zero. The worker hash keeps it.
            let expected_worker_items = json!([
                {"type":"mouseMove","x":65535,"y":1},
                {"type":"mouseMove","x":65536,"y":0}
            ]);
            assert_eq!(
                request.hash,
                crate::mapping_engine_hash::source_hash(&expected_worker_items).unwrap()
            );
        }
    }
}
