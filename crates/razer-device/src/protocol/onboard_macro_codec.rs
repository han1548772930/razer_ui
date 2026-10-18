//! Current magnetic-keyboard macro worker conversion and firmware item codec.
//! Editor source JSON is decoded once; storage receives only validated bytes.
use crate::{mapping_engine, onboard_macro::MacroWriteRequest};
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
        .map(|profile| {
            let guid = profile["guid"]
                .as_str()
                .context("Macro profile GUID is absent")?;
            let mappings = match profile.get("mappings") {
                None => Vec::new(),
                Some(Value::Array(rows)) => rows.clone(),
                Some(_) => anyhow::bail!("Macro profile mappings are not a list"),
            };
            let mut macro_guids = Vec::new();
            for row in mappings {
                let stage = if row["inputType"] == "AnalogInput" {
                    &row["mapping"][0]
                } else {
                    &row
                };
                if stage["outputType"] != "macroGroup" {
                    continue;
                }
                let guid = stage["macroGroup"]["guid"]
                    .as_str()
                    .context("Macro mapping GUID is absent")?;
                if !macro_guids.iter().any(|saved| saved == guid) {
                    macro_guids.push(guid.to_owned());
                }
            }
            Ok(ProfileMacroReferences {
                guid: guid.into(),
                macro_guids,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut macros = Vec::new();
    for document in documents {
        let document: MacroDocument = serde_json::from_value(document.clone())?;
        if matches!(
            document.app_engine.mouse_move_type.as_str(),
            "absolute" | "foreground"
        ) || document.macro_events.iter().any(|event| {
            event
                .kind
                .as_u64()
                .is_some_and(|kind| [6, 5, 4, 10].contains(&kind))
        }) {
            continue;
        }
        macros.push(compile(product, document)?);
    }
    Ok(MacroSynchronization {
        profiles,
        macros,
        random_candidates,
    })
}

fn wrapped_u16(value: f64) -> anyhow::Result<u16> {
    ensure!(value.is_finite(), "Macro coordinate is not finite");
    Ok(value.trunc().rem_euclid(65536.) as u16)
}

fn compile(product: u32, document: MacroDocument) -> anyhow::Result<MacroWriteRequest> {
    let keys = mapping_engine::source_keys(product).context("Macro source keys are absent")?;
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
                let hid =
                    source_fw_hid(hid).context("Macro HID has no current firmware encoding")?;
                data.extend_from_slice(&[if flag % 2 == 0 { 1 } else { 2 }, hid]);
            }
            Event::Mouse { id, data: wheel } => {
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
                ensure!(ms.is_finite() && ms >= 0., "Macro delay is invalid");
                if ms == 0. {
                    continue;
                }
                let ms = ms.trunc().rem_euclid(4294967296.) as u32;
                let count = if ms < 256 {
                    1
                } else if ms < 65536 {
                    2
                } else if ms < 16777216 {
                    3
                } else {
                    4
                };
                data.push(16 + count as u8);
                data.extend_from_slice(&ms.to_be_bytes()[4 - count..]);
            }
            Event::MouseMove { x, y } => {
                let (x, y) = if relative {
                    let previous = position.context("Relative macro origin is absent")?;
                    position = Some((x, y));
                    (x - previous.0, y - previous.1)
                } else {
                    (x, y)
                };
                if x == 0. && y == 0. {
                    continue;
                }
                if relative {
                    worker_item = json!({"type":"mouseMove","x":if x<0. {65536.+x}else{x},"y":if y<0. {65536.+y}else{y}});
                }
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
    Ok(MacroWriteRequest {
        guid: document.guid,
        name: document.name,
        num_items,
        timestamp: String::new(),
        hash: crate::mapping_engine_hash::source_hash(&json!(worker_items))?,
        data,
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
}
