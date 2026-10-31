//! Current profile importer supplies already normalized GUIDs, names and
//! macroEvents/appEngine. The library owns editor projection and persistence;
//! it never repeats the importer's source collision or hash transformations.
use super::*;
use serde_json::{Value, json};

fn optional_u8(value: &Value, field: &str) -> Result<Option<u8>, String> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .and_then(|n| u8::try_from(n).ok())
        .map(Some)
        .ok_or_else(|| format!("Invalid imported macro {field}"))
}
fn text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_default()
}
fn number(value: &Value) -> Result<String, String> {
    if let Some(text) = value.as_str() {
        if !text.trim().is_empty() && text.parse::<f64>().is_ok_and(|number| number.is_finite()) {
            return Ok(text.to_owned());
        }
    }
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .map(|n| n.to_string())
        .ok_or_else(|| "Invalid imported macro number".to_owned())
}

pub(super) fn source_entry(document: &Value, id: u64) -> Result<Entry, String> {
    let guid = document["guid"]
        .as_str()
        .ok_or("Imported macro UUID is absent")?;
    uuid::Uuid::parse_str(guid).map_err(|_| "Invalid imported macro UUID")?;
    let name = document["name"]
        .as_str()
        .ok_or("Imported macro name is absent")?
        .to_owned();
    let events = document["macroEvents"]
        .as_array()
        .ok_or("Imported macroEvents is not an array")?;
    let profile = events
        .iter()
        .find(|event| event["Type"] == "actionBar")
        .map(|event| &event["recordProfile"]);
    let mode = profile
        .and_then(|profile| profile["mmtSetting"].as_u64())
        .map(|mode| u8::try_from(mode).map_err(|_| "Invalid imported mouse mode"))
        .transpose()?
        .unwrap_or_else(|| match document["appEngine"]["mouseMoveType"].as_str() {
            Some("absolute") => 1,
            Some("foreground") => 2,
            Some("relative") => 3,
            _ => 0,
        });
    let macro_type = match document["macroPlaybackOption"].as_str() {
        Some("" | "standard") => MacroType::Standard,
        Some("sequence") => MacroType::Sequence,
        Some("phased") => MacroType::Phased,
        None => match document["macroType"]
            .as_u64()
            .or_else(|| profile.and_then(|profile| profile["macroType"].as_u64()))
            .unwrap_or(0)
        {
            0 => MacroType::Standard,
            1 => MacroType::Sequence,
            2 => MacroType::Phased,
            _ => return Err("Unknown imported macro type".into()),
        },
        Some(_) => return Err("Unknown imported macro playback option".into()),
    };
    let mut identities = HashMap::<String, u64>::new();
    let mut actions = Vec::new();
    for event in events {
        if event["Type"] == "actionBar" {
            continue;
        }
        let kind = match event["Type"].as_u64() {
            Some(0) => ActionKind::Delay,
            Some(1) => ActionKind::Keyboard,
            Some(2 | 3) => ActionKind::Mouse,
            Some(4) => ActionKind::Text,
            Some(5) => ActionKind::Command,
            Some(6) => ActionKind::Loop,
            Some(7) => ActionKind::Macro,
            Some(10) => ActionKind::Launch,
            other => return Err(format!("Unknown imported macro event {other:?}")),
        };
        let mut action = ActionItem::new(kind);
        action.phase = optional_u8(&event["phase"], "phase")?;
        let pair = if event["Id"].is_null() {
            None
        } else {
            // The editor uses document-local integers. This projection does
            // not rewrite the retained source event identities.
            let key = event["Id"].to_string();
            let next = identities.len() as u64 + 1;
            Some(*identities.entry(key).or_insert(next))
        };
        match kind {
            ActionKind::Delay if event["Number"].is_object() => {
                action.state = "randomized".into();
                action.number_min = number(&event["Number"]["min"])?;
                action.number_max = number(&event["Number"]["max"])?;
            }
            ActionKind::Delay => action.value = number(&event["Number"])?,
            ActionKind::Keyboard => {
                let makecode = if event["KeyEvent"]["Makecode"].is_null() {
                    None
                } else {
                    Some(
                        event["KeyEvent"]["Makecode"]
                            .as_u64()
                            .and_then(|n| u16::try_from(n).ok())
                            .ok_or("Invalid imported key makecode")?,
                    )
                };
                action.keyboard = Some(KeyboardEvent {
                    pair_id: pair,
                    makecode,
                    state: optional_u8(&event["KeyEvent"]["State"], "key state")?,
                    flag: optional_u8(&event["flag"], "key flag")?,
                    key_type: event["KeyEvent"]["Type"].as_str().map(str::to_owned),
                });
                // source_action reads ScanCode from this retained field. It
                // remains local provenance and is never a new recorder event.
                action.recorded_input = event.get("ScanCode").map(|scan| json!({"scanCode":scan}));
            }
            ActionKind::Mouse if event["Type"] == 3 => {
                let buffer = event["MouseEvent"]["Buffer"]
                    .as_array()
                    .ok_or("Imported mouse movement buffer is absent")?
                    .clone();
                let geometry = [
                    "monitorInforBefore",
                    "monitorInforAfter",
                    "virtualScreenInforBefore",
                    "virtualScreenInforAfter",
                ]
                .iter()
                .all(|key| !event[*key].is_null())
                .then(|| RecordingGeometry {
                    monitors_before: event["monitorInforBefore"].clone(),
                    monitors_after: event["monitorInforAfter"].clone(),
                    screen_before: event["virtualScreenInforBefore"].clone(),
                    screen_after: event["virtualScreenInforAfter"].clone(),
                });
                action.value = (buffer
                    .iter()
                    .filter_map(|point| point["time"].as_f64())
                    .sum::<f64>()
                    / 1000.)
                    .to_string();
                action.mouse_movement = Some(MouseMovement {
                    imported_xml: geometry.is_none(),
                    mode,
                    buffer,
                    geometry,
                });
            }
            ActionKind::Mouse => {
                action.mouse = Some(MouseEvent {
                    pair_id: pair,
                    button: optional_u8(&event["MouseEvent"]["MouseButton"], "mouse button")?,
                    state: optional_u8(&event["MouseEvent"]["State"], "mouse state")?,
                })
            }
            ActionKind::Text => {
                action.value = text(&event["Text"]);
                action.translation_key = match event.get("translationKey") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(key)) if key.is_empty() => None,
                    Some(Value::String(key)) => Some(key.clone()),
                    Some(_) => return Err("Invalid imported text translation identity".into()),
                };
            }
            ActionKind::Command => action.value = text(&event["TextCommand"]),
            ActionKind::Loop => {
                action.loop_pair_id = pair;
                action.value = number(&event["Number"])?;
                action.state = if event["LoopEvent"]["State"] == 1 {
                    "end"
                } else {
                    "start"
                }
                .into();
            }
            ActionKind::Macro => action.xml_macro_guid = event["guid"].as_str().map(str::to_owned),
            ActionKind::Launch => {
                action.state = match event["RadioIndex"].as_u64() {
                    Some(0) => "program",
                    Some(1) => "website",
                    _ => "",
                }
                .into();
                action.value = text(&event["Content0"]);
                action.secondary_value = text(&event["Content1"]);
            }
        }
        actions.push(action);
    }
    Ok(Entry {
        id,
        name,
        kind: EntryKind::Macro,
        parent: None,
        open: false,
        actions,
        macro_type,
        active_phase: optional_u8(&document["activePhase"], "active phase")?,
        record_delay: optional_u8(&document["delaySetting"], "delay setting")?.unwrap_or(0),
        xml_guid: Some(guid.to_owned()),
        xml_mouse_mode: mode,
        source_document: Some(document.clone()),
        execution: None,
    })
}

impl Entry {
    pub(super) fn retained_source_document(&self) -> Result<Option<Value>, String> {
        if self.execution.is_some() {
            return Ok(None);
        }
        let Some(document) = &self.source_document else {
            return Ok(None);
        };
        let baseline = source_entry(document, self.id)?;
        if self.actions != baseline.actions
            || self.macro_type != baseline.macro_type
            || self.xml_mouse_mode != baseline.xml_mouse_mode
        {
            return Ok(None);
        }
        let mut document = document.clone();
        document["name"] = self.name.clone().into();
        document["guid"] = self
            .xml_guid
            .clone()
            .ok_or("Imported macro UUID was removed")?
            .into();
        if self.record_delay != baseline.record_delay {
            document["delaySetting"] = self.record_delay.into();
        }
        if self.active_phase != baseline.active_phase {
            document["activePhase"] = json!(self.active_phase);
        }
        Ok(Some(document))
    }
}

impl MacroLibraryFile {
    /// Atomic local append. The profile import owner already applied the
    /// original collision rules; a changed library is an explicit error.
    pub fn append_source_documents(&mut self, documents: &[Value]) -> Result<(), String> {
        let mut candidate = self.clone();
        for document in documents {
            if candidate.next_id >= u64::MAX - 1 {
                return Err("Macro local identity space exhausted".into());
            }
            let entry = source_entry(document, candidate.next_id)?;
            if candidate
                .entries
                .iter()
                .any(|old| old.xml_guid == entry.xml_guid)
            {
                return Err("Imported macro UUID now exists in the shared library".into());
            }
            candidate.next_id += 1;
            candidate.entries.push(entry);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn imported_profile_macro_roundtrips_engine_identity_and_source_fields() {
        let document = json!({"guid":"3067b9df-40a4-4bc3-a9ea-2380195fe948","name":"Imported","macroPlaybackOption":"phased","customLocalField":{"saved":true},"macroEvents":[
            {"Type":"actionBar","recordProfile":{"mmtSetting":0,"macroType":2}},
            {"Type":1,"Id":"source-key-uuid","KeyEvent":{"Makecode":65,"State":0},"ScanCode":30,"flag":0,"phase":0},
            {"Type":1,"Id":"source-key-uuid","KeyEvent":{"Makecode":65,"State":1},"ScanCode":30,"flag":1,"phase":2}],
            "appEngine":{"events":[],"pressEvents":[{"type":"keyboard","scancode":30,"flag":0}],"releaseEvents":[{"type":"keyboard","scancode":30,"flag":1}],"holdEvents":[],"hash":"source-exact","mouseMoveType":"none"}});
        let mut library = MacroLibraryFile::default();
        library
            .append_source_documents(&[document.clone()])
            .unwrap();
        let loaded: MacroLibraryFile =
            serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert_eq!(
            loaded.source_macro_documents().unwrap(),
            vec![document.clone()]
        );
        assert_eq!(
            loaded.mapping_catalog()[0]["appEngine"],
            document["appEngine"]
        );
        library.entries[0].name = "Renamed".into();
        let mut renamed = document;
        renamed["name"] = "Renamed".into();
        assert_eq!(library.source_macro_documents().unwrap(), vec![renamed]);
        library.entries[0].actions[0].phase = Some(1);
        assert_ne!(
            library.source_macro_documents().unwrap()[0]["appEngine"]["hash"],
            "source-exact"
        );
    }
    #[test]
    fn import_does_not_partially_append_on_duplicate_or_invalid_second_document() {
        let mut library = MacroLibraryFile::default();
        let before = library.clone();
        let document = json!({"guid":"3067b9df-40a4-4bc3-a9ea-2380195fe948","name":"Imported","macroEvents":[],"macroPlaybackOption":"standard","appEngine":{"events":[]}});
        assert!(
            library
                .append_source_documents(&[document.clone(), document])
                .is_err()
        );
        assert_eq!(library, before);
    }
}
