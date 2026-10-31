//! Current macro main.6a2e8a10: source events and ordinary/phased publication.
//! Local editor identities are resolved to persistent UUIDs before compilation.
use super::*;
use serde_json::{Value, json};

impl MacroLibraryFile {
    /// Restore both native consumers from the local library. The source UI's qs
    /// writes only the consumer selected by the edited type; PhasedMacroData
    /// (1080b2) requires three lanes, while MacroData (f7106) accepts the ordinary
    /// lane retained on every source document.
    pub fn publication_catalogs(
        &self,
    ) -> Result<razer_device::macro_engine::PublicationCatalogs, String> {
        let source = self.source_macro_documents()?;
        let macros = razer_device::macro_engine::publication_documents(&source)
            .map_err(|error| error.to_string())?;
        let mut phased_macros = Vec::new();
        for (entry, document) in self
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Macro)
            .zip(&macros)
        {
            if entry.macro_type != MacroType::Phased {
                continue;
            }
            let engine = document.get("appEngine").ok_or("Macro engine is missing")?;
            for lane in ["pressEvents", "holdEvents", "releaseEvents"] {
                if engine.get(lane).and_then(Value::as_array).is_none() {
                    return Err(format!("Phased macro {lane} is missing"));
                }
            }
            phased_macros.push(document.clone());
        }
        Ok(razer_device::macro_engine::PublicationCatalogs {
            macros,
            phased_macros,
        })
    }

    /// Assignment menus inspect source event kinds and retained identities;
    /// they do not require every editor row to be ready for publication.
    pub fn mapping_catalog(&self) -> Vec<Value> {
        self.entries.iter().filter(|entry| entry.kind == EntryKind::Macro)
            .filter_map(|entry| {
                if let Ok(Some(document))=entry.retained_source_document(){return Some(document);}
                let guid = entry.xml_guid.as_ref()?;
                let mode = entry.execution.as_ref().filter(|snapshot|snapshot.matches(&entry.actions)).map(|snapshot|snapshot.engine().movement.setting()).or_else(||entry.actions.iter().find_map(|item| item.mouse_movement.as_ref().map(|m|m.mode))).unwrap_or(entry.xml_mouse_mode);
                let events:Vec<_> = entry.actions.iter().map(|item| self.source_action(item).unwrap_or_else(|_| {
                    let kind=match item.kind {ActionKind::Delay=>0,ActionKind::Keyboard=>1,ActionKind::Mouse if item.mouse_movement.is_some()=>3,ActionKind::Mouse=>2,ActionKind::Text=>4,ActionKind::Command=>5,ActionKind::Loop=>6,ActionKind::Macro=>7,ActionKind::Launch=>10};
                    json!({"Type":kind,"MouseEvent":{"MouseButton":item.mouse.as_ref().and_then(|mouse|mouse.button)}})
                })).collect();
                Some(json!({"guid":guid,"name":entry.name,"macroPlaybackOption":match entry.macro_type {MacroType::Standard=>"standard",MacroType::Sequence=>"sequence",MacroType::Phased=>"phased"},"macroEvents":events,
                    "appEngine":{"mouseMoveType":match mode {1=>"absolute",2=>"foreground",3=>"relative",_=>"none"}}}))
            }).collect()
    }
    pub fn ensure_guids(&mut self) {
        for entry in &mut self.entries {
            if entry.kind == EntryKind::Macro && entry.xml_guid.is_none() {
                entry.xml_guid = Some(uuid::Uuid::new_v4().to_string());
            }
        }
    }

    /// Source-shaped UI documents. Incomplete legacy physical input rows stay
    /// explicit errors; local persistence does not depend on this compilation.
    pub fn source_macro_documents(&self) -> Result<Vec<Value>, String> {
        self.validate()?;
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Macro)
            .map(|entry| self.source_macro_document(entry))
            .collect()
    }

    fn source_macro_document(&self, entry: &Entry) -> Result<Value, String> {
        if let Some(document) = entry.retained_source_document()? {
            return Ok(document);
        }
        let guid = entry
            .xml_guid
            .as_deref()
            .ok_or("Macro has no persistent UUID")?;
        let mode = entry
            .execution
            .as_ref()
            .map(|snapshot| snapshot.engine().movement.setting())
            .or_else(|| {
                entry
                    .actions
                    .iter()
                    .find_map(|item| item.mouse_movement.as_ref().map(|movement| movement.mode))
            })
            .unwrap_or(entry.xml_mouse_mode);
        let execution = self.execution_for_actions(entry, &entry.actions, None, mode)?;
        let kind = match entry.macro_type {
            MacroType::Standard => 0,
            MacroType::Sequence => 1,
            MacroType::Phased => 2,
        };
        let name = match entry.macro_type {
            MacroType::Standard => "standard",
            MacroType::Sequence => "sequence",
            MacroType::Phased => "phased",
        };
        let mut source = vec![
            json!({"Type":"actionBar","recordProfile":{"mmtSetting":mode,"macroType":kind},"selected":false}),
        ];
        let original = entry
            .source_document
            .as_ref()
            .map(|document| import::source_entry(document, entry.id))
            .transpose()?;
        let original_events = entry
            .source_document
            .as_ref()
            .and_then(|document| document.get("macroEvents"))
            .and_then(Value::as_array);
        if let Some(action_bar) = original_events.and_then(|events| {
            events
                .iter()
                .find(|event| event.get("Type").and_then(Value::as_str) == Some("actionBar"))
        }) {
            source[0] = action_bar.clone();
        }
        let original_rows: Vec<_> = original_events
            .into_iter()
            .flatten()
            .filter(|event| event.get("Type").and_then(Value::as_str) != Some("actionBar"))
            .collect();
        for (index, item) in entry.actions.iter().enumerate() {
            if original.as_ref().and_then(|entry| entry.actions.get(index)) == Some(item)
                && let Some(event) = original_rows.get(index)
            {
                source.push((*event).clone());
            } else {
                source.push(self.source_action(item)?);
            }
        }
        let engine = serde_json::to_value(execution.engine()).map_err(|error| error.to_string())?;
        let compiled = json!({"guid":guid,"name":entry.name,"macroType":kind,"macroPlaybackOption":name,
            "delaySetting":entry.record_delay,"activePhase":entry.active_phase,
            "macroEvents":source,"appEngine":engine});
        let mut document = entry.source_document.clone().unwrap_or_else(|| json!({}));
        document
            .as_object_mut()
            .ok_or("Retained macro source is not an object")?
            .extend(compiled.as_object().unwrap().clone());
        Ok(document)
    }

    pub(super) fn source_action(&self, item: &ActionItem) -> Result<Value, String> {
        fn number(text: &str) -> Result<f64, String> {
            if text.trim().is_empty() {
                return Ok(0.);
            }
            text.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("Invalid macro numeric value: {text}"))
        }
        let mut event = match item.kind {
            ActionKind::Delay if item.state == "randomized" => {
                json!({"Type":0,"Number":{"min":number(&item.number_min)?,"max":number(&item.number_max)?}})
            }
            ActionKind::Delay => json!({"Type":0,"Number":number(&item.value)?}),
            ActionKind::Keyboard => {
                let key = item.keyboard.as_ref();
                if key.is_none() && !item.value.is_empty() {
                    return Err("Legacy key row has no physical key metadata".into());
                }
                let mut event = json!({"Type":1,"Id":key.and_then(|k|k.pair_id),"KeyEvent":{"Makecode":key.and_then(|k|k.makecode),"State":key.and_then(|k|k.state)},"flag":key.and_then(|k|k.flag)});
                if let Some(kind) = key.and_then(|k| k.key_type.as_ref()) {
                    event["KeyEvent"]["Type"] = kind.clone().into();
                }
                if let Some(scan) = item.recorded_input.as_ref().and_then(|v| {
                    v.get("scanCode")
                        .or_else(|| v.get("event")?.as_array()?.last()?.get("scanCode"))
                }) {
                    event["ScanCode"] = scan.clone();
                }
                event
            }
            ActionKind::Mouse if item.mouse_movement.is_some() => {
                json!({"Type":3,"MouseEvent":{"Buffer":item.mouse_movement.as_ref().unwrap().buffer}})
            }
            ActionKind::Mouse => {
                let mouse = item.mouse.as_ref();
                if mouse.is_none() && !item.value.is_empty() {
                    return Err("Legacy mouse row has no physical button metadata".into());
                }
                json!({"Type":2,"Id":mouse.and_then(|m|m.pair_id),"MouseEvent":{"MouseButton":mouse.and_then(|m|m.button),"State":mouse.and_then(|m|m.state)}})
            }
            ActionKind::Text => json!({"Type":4,"Text":item.value}),
            ActionKind::Command => json!({"Type":5,"TextCommand":item.value}),
            ActionKind::Loop => {
                json!({"Type":6,"Id":item.loop_pair_id,"Number":number(&item.value)?,"LoopEvent":{"State":u8::from(item.state=="end")}})
            }
            ActionKind::Macro => {
                let target = item.macro_id.and_then(|id| {
                    self.entries
                        .iter()
                        .find(|e| e.id == id && e.kind == EntryKind::Macro)
                });
                let guid = target
                    .and_then(|e| e.xml_guid.as_deref())
                    .or(item.xml_macro_guid.as_deref());
                if item.macro_id.is_some() && target.is_none() {
                    return Err("Nested macro target was removed".into());
                }
                if let Some(guid) = guid {
                    if !self
                        .entries
                        .iter()
                        .any(|e| e.xml_guid.as_deref() == Some(guid))
                    {
                        return Err("Nested macro UUID is unresolved locally".into());
                    }
                }
                json!({"Type":7,"guid":guid})
            }
            ActionKind::Launch => {
                json!({"Type":10,"RadioIndex":match item.state.as_str(){"program"=>Some(0),"website"=>Some(1),_=>None},"Content0":item.value,"Content1":item.secondary_value})
            }
        };
        if let Some(phase) = item.phase {
            event["phase"] = phase.into();
        }
        if let Some(key) = &item.translation_key {
            event["translationKey"] = key.clone().into();
        }
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(id: u64, kind: MacroType, actions: Vec<ActionItem>) -> Entry {
        Entry {
            id,
            name: format!("Macro {id}"),
            kind: EntryKind::Macro,
            parent: None,
            open: false,
            actions,
            macro_type: kind,
            active_phase: None,
            record_delay: 0,
            xml_guid: None,
            xml_mouse_mode: 0,
            source_document: None,
            execution: None,
        }
    }
    fn library(entries: Vec<Entry>) -> MacroLibraryFile {
        let mut file = MacroLibraryFile {
            next_id: entries.iter().map(|e| e.id).max().unwrap_or(0) + 1,
            entries,
            ..Default::default()
        };
        file.ensure_guids();
        file
    }
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn nested_macro_compiles_to_target_uuid_and_never_local_id() {
        let mut nested = ActionItem::new(ActionKind::Macro);
        nested.macro_id = Some(2);
        let file = library(vec![
            entry(1, MacroType::Standard, vec![nested]),
            entry(2, MacroType::Sequence, vec![]),
        ]);
        let docs = file.source_macro_documents().unwrap();
        assert_eq!(
            docs[0]["appEngine"]["events"][0]["guid"],
            file.entries[1].xml_guid.as_deref().unwrap()
        );
        assert_eq!(docs[1]["macroPlaybackOption"], "sequence");
        assert_ne!(docs[0]["appEngine"]["events"][0]["guid"], "2");
    }
    #[test]
    fn phased_source_hash_uses_three_lanes_publication_hash_uses_whole_engine() {
        let actions = (0..3)
            .map(|phase| {
                let mut item = ActionItem::new(ActionKind::Text);
                item.phase = Some(phase);
                item.value = format!("phase {phase}");
                item
            })
            .collect();
        let file = library(vec![entry(1, MacroType::Phased, actions)]);
        let docs = file.source_macro_documents().unwrap();
        for (phase, lane) in ["pressEvents", "holdEvents", "releaseEvents"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                docs[0]["appEngine"][lane][0]["text"],
                format!("phase {phase}")
            );
            assert_eq!(docs[0]["appEngine"][lane].as_array().unwrap().len(), 2);
        }
        let published = razer_device::macro_engine::publication_documents(&docs).unwrap();
        assert_ne!(
            docs[0]["appEngine"]["hash"],
            published[0]["appEngine"]["hash"]
        );
        assert_eq!(
            published[0]["appEngine"]["pressEvents"],
            docs[0]["appEngine"]["pressEvents"]
        );
        assert!(
            published[0]["appEngine"]["events"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn invalid_legacy_row_preserves_assignment_catalog_but_cannot_publish() {
        let mut item = ActionItem::new(ActionKind::Keyboard);
        item.value = "old display-only label".into();
        let file = library(vec![entry(1, MacroType::Standard, vec![item])]);
        assert!(file.source_macro_documents().is_err());
        assert_eq!(file.mapping_catalog().len(), 1);
        assert_eq!(file.mapping_catalog()[0]["macroEvents"][0]["Type"], 1);
        assert!(file.validate().is_ok());
    }
}
