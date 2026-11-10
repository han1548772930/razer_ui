//! The current OBM worker's local macro import projection. The original device
//! bytes and decoder stop/repair facts stay with the caller's MacroReading.
use super::{MacroReading, decode::Item};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    app_engine: Engine,
    delay_setting: u8,
    guid: String,
    macro_events: Vec<EditorEvent>,
    macro_playback_option: String,
    name: String,
}

impl Document {
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn engine_events(&self) -> &[EngineEvent] {
        &self.app_engine.events
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Engine {
    hash: String,
    events: Vec<EngineEvent>,
    mouse_move_type: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EngineEvent {
    Keyboard {
        scancode: Option<u32>,
        flag: u32,
    },
    Mouse {
        id: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<i64>,
    },
    Delay {
        ms: i64,
    },
}

#[derive(Clone, Debug, Serialize)]
struct Empty {}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
enum EditorEvent {
    ActionBar {
        #[serde(rename = "Type")]
        kind: &'static str,
        #[serde(rename = "recordProfile")]
        record_profile: Empty,
        selected: bool,
    },
    Delay {
        #[serde(rename = "Type")]
        kind: u8,
        #[serde(rename = "Number")]
        number: String,
        selected: bool,
    },
    Keyboard {
        #[serde(rename = "Type")]
        kind: u8,
        #[serde(rename = "KeyEvent")]
        event: KeyEvent,
        flag: u32,
        #[serde(rename = "isPairing")]
        is_pairing: bool,
        selected: bool,
        #[serde(rename = "Id")]
        id: u32,
    },
    Mouse {
        #[serde(rename = "MouseEvent")]
        event: MouseEvent,
        #[serde(rename = "Type")]
        kind: u8,
        flag: u8,
        #[serde(rename = "isPairing")]
        is_pairing: bool,
        selected: bool,
    },
}

#[derive(Clone, Debug, Serialize)]
struct KeyEvent {
    #[serde(rename = "Makecode")]
    make_code: u32,
    #[serde(rename = "State")]
    state: u32,
}

#[derive(Clone, Debug, Serialize)]
struct MouseEvent {
    #[serde(rename = "MouseButton")]
    button: u8,
    #[serde(rename = "State")]
    state: Option<u8>,
}

fn source_mouse(item: &Item) -> Option<MouseEvent> {
    let Item::Mouse { id: Some(id), data } = item else {
        return None;
    };
    let (button, state) = match (*id, data) {
        (1, None) => (0, Some(0)),
        (2, None) => (0, Some(1)),
        (4, None) => (1, Some(0)),
        (8, None) => (1, Some(1)),
        (16, None) => (2, Some(0)),
        (32, None) => (2, Some(1)),
        (64, None) => (3, Some(0)),
        (128, None) => (3, Some(1)),
        (256, None) => (4, Some(0)),
        (512, None) => (4, Some(1)),
        (0, None) => (5, None),
        (1024, Some(120)) => (6, None),
        (1024, Some(65416)) => (7, None),
        (2048, Some(120 | 65416)) => (8, None),
        _ => return None,
    };
    Some(MouseEvent { button, state })
}

fn mouse_engine(product: u32, event: &MouseEvent) -> Vec<EngineEvent> {
    let mouse = |id, data| EngineEvent::Mouse { id, data };
    match event.button {
        0..=4 => {
            let up = event.state.is_some_and(|state| state != 0);
            let (down, release) = match event.button {
                0 => (1, 2),
                1 => (4, 8),
                2 if matches!(product, 739 | 752) => (32, 16),
                2 => (16, 32),
                3 => (64, 128),
                4 => (256, 512),
                _ => unreachable!(),
            };
            vec![mouse(if up { release } else { down }, None)]
        }
        5 if event.state == Some(0) => vec![
            mouse(1, None),
            mouse(2, None),
            EngineEvent::Delay { ms: 10 },
            mouse(1, None),
            mouse(2, None),
        ],
        6 => vec![mouse(2048, Some(65416))],
        7 => vec![mouse(2048, Some(120))],
        8 => vec![mouse(1024, Some(120))],
        _ => Vec::new(),
    }
}

/// Imports only the source-supported projection of actual decoded bytes. This
/// does not change the catalog, local library or physical device by itself.
pub fn from_reading(product: u32, reading: &MacroReading) -> Result<Document> {
    reading.validate_keyboard_content(product)?;
    let decoded = reading.keyboard_projection(product)?;
    document(
        product,
        &reading.metadata.guid,
        &reading.metadata.name,
        &decoded.items,
    )
}

fn document(product: u32, guid: &str, name: &str, items: &[Item]) -> Result<Document> {
    let keys = crate::onboard_macro_codec::keyboard_keys(product)?;
    let mut events = Vec::new();
    let mut engine_events = Vec::new();
    let mut next_id = 1u32;
    for item in items {
        match item {
            Item::Delay { milliseconds } => {
                ensure!(
                    milliseconds.is_finite() && milliseconds.fract() == 0.,
                    "Decoded macro delay is not a firmware integer"
                );
                events.push(EditorEvent::Delay {
                    kind: 0,
                    number: (milliseconds / 1000.).to_string(),
                    selected: false,
                });
                engine_events.push(EngineEvent::Delay {
                    ms: *milliseconds as i64,
                });
            }
            Item::Keyboard { key_name, flag, .. } => {
                // The source uses `flag - flat % 2`; `flat` is absent on every
                // decoded item. Its scancode alternative therefore never matches.
                let Some(key) = key_name
                    .as_deref()
                    .and_then(|name| keys.iter().find(|key| key.input_id == Some(name)))
                else {
                    continue;
                };
                let make_code = if key.key_code.is_empty() {
                    0
                } else {
                    key.key_code
                        .parse::<u32>()
                        .context("Source macro key code is invalid")?
                };
                let base = key.output_flag.unwrap_or(key.flag);
                let state = base + u32::from(flag % 2 != 0);
                let id = if state % 2 == 0 {
                    let id = next_id;
                    next_id = next_id
                        .checked_add(1)
                        .context("Imported macro pairing ID overflow")?;
                    id
                } else {
                    events
                        .iter()
                        .rev()
                        .find_map(|event| match event {
                            EditorEvent::Keyboard { event, id, .. }
                                if event.make_code == make_code =>
                            {
                                Some(*id)
                            }
                            _ => None,
                        })
                        .unwrap_or(0)
                };
                events.push(EditorEvent::Keyboard {
                    kind: 1,
                    event: KeyEvent { make_code, state },
                    flag: state,
                    is_pairing: false,
                    selected: false,
                    id,
                });
                let mut converted = keys
                    .iter()
                    .find(|key| key.key_code.parse::<u32>().ok() == Some(make_code));
                if !matches!(product, 739 | 752) {
                    if let Some(name) = match make_code {
                        285 => Some("KEY_RIGHT_CTRL"),
                        310 => Some("KEY_RIGHT_SHIFT"),
                        312 => Some("KEY_RIGHT_ALT"),
                        _ => None,
                    } {
                        converted = keys.iter().find(|key| key.input_id == Some(name));
                    }
                } else if make_code == 16 && matches!(state, 2 | 3) && converted.is_some() {
                    engine_events.push(EngineEvent::Keyboard {
                        scancode: Some(54),
                        flag: state - 2,
                    });
                    continue;
                }
                if (256..=258).contains(&make_code) {
                    converted = keys
                        .iter()
                        .find(|key| key.key_code.parse::<u32>().ok() == Some(make_code - 240));
                }
                engine_events.push(EngineEvent::Keyboard {
                    scancode: converted.map(|key| key.scancode),
                    flag: state,
                });
            }
            Item::Mouse { .. } => {
                let Some(event) = source_mouse(item) else { continue };
                engine_events.extend(mouse_engine(product, &event));
                // The source's pairing checks use MouseEvent.button/flag while
                // the constructed fields are MouseButton/State, so no Id is set.
                events.push(EditorEvent::Mouse {
                    flag: event.state.unwrap_or(0),
                    event,
                    kind: 2,
                    is_pairing: false,
                    selected: false,
                });
            }
            // The current import worker has no cursor case. Raw cursor records
            // remain in the observation rather than invented local editor data.
            Item::Cursor { .. } => {}
        }
    }
    events.insert(
        0,
        EditorEvent::ActionBar {
            kind: "actionBar",
            record_profile: Empty {},
            selected: false,
        },
    );
    let mut app_engine = Engine {
        hash: String::new(),
        events: engine_events,
        mouse_move_type: "none",
    };
    app_engine.hash = crate::mapping_engine_hash::source_hash(&serde_json::to_value(&app_engine)?)?;
    Ok(Document {
        app_engine,
        delay_setting: 0,
        guid: guid.into(),
        macro_events: events,
        macro_playback_option: String::new(),
        name: name.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn import_boundary_rejects_unaccounted_bytes_instead_of_using_local_macro_data() {
        let mut reading = MacroReading {
            metadata: super::super::decode_metadata(7, vec![0; 250]).unwrap(),
            data_size: 4,
            data: vec![1, 4, 2, 4],
            data_blocks: Vec::new(),
        };
        let imported = from_reading(747, &reading).unwrap();
        assert_eq!(imported.guid(), reading.metadata.guid);
        assert_eq!(imported.engine_events().len(), 2);
        reading.data_size = 6;
        assert!(from_reading(747, &reading).is_err());
        assert!(from_reading(515, &reading).is_err());
    }

    #[test]
    fn source_import_keeps_key_pairing_and_its_wheel_cursor_projection() {
        let decoded = super::super::decode::keyboard_items(
            679,
            &[1, 4, 17, 125, 2, 4, 10, 1, 21, 0, 3, 0, 4, 10, 255],
            15,
        )
        .unwrap();
        let result = document(679, "device-guid", "device-name", &decoded.items).unwrap();
        assert_eq!(
            result.engine_events(),
            [
                EngineEvent::Keyboard {
                    scancode: Some(30),
                    flag: 0
                },
                EngineEvent::Delay { ms: 125 },
                EngineEvent::Keyboard {
                    scancode: Some(30),
                    flag: 1
                },
                EngineEvent::Mouse {
                    id: 2048,
                    data: Some(65416)
                },
                EngineEvent::Mouse {
                    id: 2048,
                    data: Some(120)
                },
            ]
        );
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            json["macroEvents"][0],
            json!({"Type":"actionBar","recordProfile":{},"selected":false})
        );
        assert_eq!(json["macroEvents"][1]["Id"], 1);
        assert_eq!(json["macroEvents"][2]["Number"], "0.125");
        assert_eq!(json["macroEvents"][3]["Id"], 1);
        assert_eq!(
            json["macroEvents"][4]["MouseEvent"],
            json!({"MouseButton":6,"State":null})
        );
        assert!(json["macroEvents"][4].get("Id").is_none());
        assert_eq!(json["macroEvents"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn older_worker_middle_button_conversion_remains_a_real_product_difference() {
        for product in [679, 739, 747, 752, 756] {
            let result = document(
                product,
                "g",
                "n",
                &[
                    Item::Mouse {
                        id: Some(16),
                        data: None,
                    },
                    Item::Mouse {
                        id: Some(32),
                        data: None,
                    },
                ],
            )
            .unwrap();
            let ids: Vec<_> = result
                .engine_events()
                .iter()
                .map(|event| match event {
                    EngineEvent::Mouse { id, .. } => *id,
                    _ => unreachable!(),
                })
                .collect();
            assert_eq!(
                ids,
                if matches!(product, 739 | 752) {
                    vec![32, 16]
                } else {
                    vec![16, 32]
                }
            );
        }
    }

    #[test]
    fn source_name_match_skips_unmapped_keys_and_retains_shift_conversion() {
        let decoded =
            super::super::decode::keyboard_items(739, &[1, 231, 2, 231, 1, 229, 2, 229], 8)
                .unwrap();
        let result = document(739, "g", "n", &decoded.items).unwrap();
        assert_eq!(
            result.engine_events(),
            [
                EngineEvent::Keyboard {
                    scancode: Some(42),
                    flag: 0
                },
                EngineEvent::Keyboard {
                    scancode: Some(42),
                    flag: 1
                }
            ]
        );
        assert_eq!(
            serde_json::to_value(&result).unwrap()["macroEvents"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}
