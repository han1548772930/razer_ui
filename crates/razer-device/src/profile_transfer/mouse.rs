//! Current PID70 MW25947.Xt and PID182 MW30525.Xt profile import processing.
use crate::{
    mapping_document::MappingProduct,
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
    mapping_submission::next_source_version,
    mouse_legacy_mapping,
};
use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The selected cache belongs to the caller's retained device session.
#[non_exhaustive]
pub struct MouseImportContext<'a> {
    pub product_id: u32,
    pub existing_profiles: &'a [Value],
    pub existing_macros: &'a [Value],
    pub device_settings: Option<&'a Value>,
    pub selected_profile: Option<&'a Value>,
}

impl<'a> MouseImportContext<'a> {
    pub fn new(
        product_id: u32,
        existing_profiles: &'a [Value],
        existing_macros: &'a [Value],
        device_settings: Option<&'a Value>,
    ) -> Self {
        Self {
            product_id,
            existing_profiles,
            existing_macros,
            device_settings,
            selected_profile: None,
        }
    }

    pub fn with_selected_profile(mut self, selected_profile: Option<&'a Value>) -> Self {
        self.selected_profile = selected_profile;
        self
    }
}

#[non_exhaustive]
/// A local collection candidate. Persisting it does not publish a native
/// mapping document or acknowledge a device write.
pub struct PreparedMouseImport {
    pub profiles: Vec<Value>,
    pub device_settings: Value,
    pub imported_macros: Vec<Value>,
}

#[derive(Deserialize)]
struct ProfileIdentity {
    guid: String,
    name: String,
}

#[derive(Deserialize)]
struct ImportedProfile {
    name: String,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

#[derive(Deserialize)]
struct SelectedRow {
    profile: ImportedProfile,
    #[serde(rename = "listItems")]
    list_items: Vec<MacroSelection>,
}

#[derive(Deserialize)]
struct MacroSelection {
    guid: String,
    #[serde(rename = "isMacroSelected", default)]
    selected: Value,
}

#[derive(Deserialize)]
struct DeviceVersion {
    #[serde(default, deserialize_with = "present_version")]
    version: Option<u64>,
}

fn present_version<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error> {
    u64::deserialize(deserializer).map(Some)
}

fn macro_store(values: &[Value]) -> Result<BTreeMap<String, String>> {
    let mut macros = BTreeMap::new();
    for value in values {
        let identity: ProfileIdentity = serde_json::from_value(value.clone())
            .context("Current mouse macro identity is invalid")?;
        ensure!(
            !identity.guid.is_empty(),
            "Current mouse macro GUID is empty"
        );
        ensure!(
            macros.insert(identity.guid, identity.name).is_none(),
            "Current mouse macro GUID is repeated"
        );
    }
    Ok(macros)
}

fn group_guid(value: &Value) -> Result<&str> {
    value
        .as_object()
        .and_then(|group| group.get("guid"))
        .and_then(Value::as_str)
        .context("Imported mouse macro group GUID is absent")
}

fn repair_mapping(
    value: &mut Value,
    macros: &BTreeMap<String, String>,
    remapped: &BTreeMap<String, (String, String)>,
) -> Result<()> {
    let mapping = value
        .as_object_mut()
        .context("Imported mouse mapping is not an object")?;
    let direct = mapping.get("macroGroup").filter(|value| !value.is_null());
    let group = direct.or_else(|| {
        mapping
            .get("mapping")
            .and_then(Value::as_array)
            .and_then(|stages| stages.first())
            .and_then(|stage| stage.get("macroGroup"))
    });
    let Some(group) = group.filter(|group| super::truthy(group)) else {
        return Ok(());
    };
    let guid = group_guid(group)?.to_owned();
    let is_direct = direct.is_some();
    if let Some((guid, name)) = remapped.get(&guid) {
        let group = if is_direct {
            mapping.get_mut("macroGroup")
        } else {
            mapping
                .get_mut("mapping")
                .and_then(Value::as_array_mut)
                .and_then(|stages| stages.first_mut())
                .and_then(|stage| stage.get_mut("macroGroup"))
        }
        .and_then(Value::as_object_mut)
        .expect("validated macro group");
        group.insert("guid".into(), guid.clone().into());
        group.insert("name".into(), name.clone().into());
        // Xt returns immediately after remapping the selected first group;
        // the later nested-stage repair is a different branch.
        return Ok(());
    }
    let current_macro = macros.get(&guid).map(|name| (guid, name.clone()));
    if let Some((_, name)) = &current_macro {
        if is_direct {
            mapping
                .get_mut("macroGroup")
                .and_then(Value::as_object_mut)
                .expect("validated macro group")
                .insert("name".into(), name.clone().into());
        } else {
            mapping
                .get_mut("mapping")
                .and_then(Value::as_array_mut)
                .and_then(|stages| stages.first_mut())
                .and_then(|stage| stage.get_mut("macroGroup"))
                .and_then(Value::as_object_mut)
                .expect("validated nested macro group")
                .insert("name".into(), name.clone().into());
        }
    }
    if let Some(stages) = mapping
        .get_mut("mapping")
        .filter(|value| super::truthy(value))
    {
        let stages = stages
            .as_array_mut()
            .context("Imported mouse mapping stages are not an array")?;
        for (index, stage) in stages.iter_mut().enumerate() {
            let stage = stage
                .as_object_mut()
                .context("Imported mouse mapping stage is not an object")?;
            if stage.get("macroGroup").is_some_and(super::truthy)
                && let Some((guid, name)) = &current_macro
            {
                let group = stage
                    .get_mut("macroGroup")
                    .and_then(Value::as_object_mut)
                    .context("Imported nested mouse macro group is invalid")?;
                // Xt uses the macro selected by the first stage for every
                // macro stage, rather than resolving each stage separately.
                group.insert("guid".into(), guid.clone().into());
                group.insert("name".into(), name.clone().into());
            } else {
                stage.remove("macroGroup");
                if index == 0 {
                    stage.insert("keyboardGroup".into(), json!({}));
                    stage.insert("outputType".into(), "keyboardGroup".into());
                } else {
                    stage.insert("outputType".into(), "defaultGroup".into());
                    stage.insert("isDefault".into(), false.into());
                }
            }
        }
    }
    Ok(())
}

fn turbo_ids(engine: Option<&Value>) -> BTreeSet<String> {
    engine
        .and_then(|engine| engine.get("mappings"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mapping| mapping.get("output"))
        .filter(|output| output.get("type").and_then(Value::as_str) == Some("turbo"))
        .filter_map(|output| output.get("guid").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn stored_turbos(settings: &Map<String, Value>, name: &str) -> Result<Option<Vec<Value>>> {
    settings
        .get(name)
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_array()
                .cloned()
                .with_context(|| format!("Current mouse {name} is not an array"))
        })
        .transpose()
}

fn valid_guid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|index| bytes[*index] == b'-')
        && bytes[14] == b'4'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
}

/// Global Profiles supplies no macro rows. Existing local macro references
/// still participate in repair and compilation. Preparing a candidate performs
/// no file, service or device writes and never activates an imported profile.
pub fn prepare_global_mouse_import(
    rows: &[Value],
    context: MouseImportContext<'_>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> Result<PreparedMouseImport> {
    prepare_mouse_import_inner(rows, &[], context, fresh_guids)
}

/// Product UI qS is a no-op for these button lists, and neither product exports
/// an optional sanitizer or analogSpecs. This caller stage selects macros by
/// listItems before the shared middleware processor handles them.
pub fn prepare_mouse_import(
    rows: &[Value],
    macro_data: &[Value],
    context: MouseImportContext<'_>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> Result<PreparedMouseImport> {
    let selected: BTreeSet<_> = rows
        .iter()
        .map(|row| {
            serde_json::from_value::<SelectedRow>(row.clone())
                .context("Imported mouse profile row is invalid")
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flat_map(|row| row.list_items)
        .filter(|selection| super::truthy(&selection.selected))
        .map(|selection| selection.guid)
        .collect();
    let macros = macro_data
        .iter()
        .filter(|value| {
            value
                .get("guid")
                .and_then(Value::as_str)
                .is_some_and(|guid| selected.contains(guid))
        })
        .cloned()
        .collect::<Vec<_>>();
    prepare_mouse_import_inner(rows, &macros, context, fresh_guids)
}

fn prepare_mouse_import_inner(
    rows: &[Value],
    selected_macros: &[Value],
    context: MouseImportContext<'_>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> Result<PreparedMouseImport> {
    ensure!(
        !rows.is_empty(),
        "No mouse profiles were selected for import"
    );
    let defaults = match context.product_id {
        70 => mouse_legacy_mapping::source_profile_defaults()?,
        182 => super::mouse_defaults::default_profile_182(),
        _ => anyhow::bail!("Current mouse import processor product is invalid"),
    };
    let mut settings = match context.device_settings {
        Some(settings) => settings
            .as_object()
            .context("Current mouse device settings are not an object")?
            .clone(),
        None => Map::new(),
    };
    let storage_field =
        MappingProduct::for_product_id(context.product_id).map(MappingProduct::storage_field);
    let stored_document = storage_field.and_then(|field| settings.get(field)).cloned();
    ensure!(
        stored_document.as_ref().is_none_or(Value::is_object),
        "Current mouse mapping document is not an object"
    );
    let empty_settings = Map::new();
    let turbo_settings = if storage_field.is_some() {
        stored_document
            .as_ref()
            .and_then(Value::as_object)
            .unwrap_or(&empty_settings)
    } else {
        &settings
    };
    let original_macros = macro_store(context.existing_macros)?;
    let mut macros = original_macros.clone();
    let mut identities = BTreeSet::new();
    for profile in context.existing_profiles {
        let identity: ProfileIdentity = serde_json::from_value(profile.clone())
            .context("Current mouse profile identity is invalid")?;
        ensure!(
            !identity.guid.is_empty() && identities.insert(identity.guid),
            "Current mouse profile identity is empty or repeated"
        );
    }
    let rows = rows
        .iter()
        .map(|row| {
            serde_json::from_value::<SelectedRow>(row.clone())
                .context("Imported mouse profile row is invalid")
        })
        .collect::<Result<Vec<_>>>()?;
    let engine = MappingEngine::new(context.product_id)?;
    let mut engine_context = EngineContext {
        default_turbos: stored_turbos(turbo_settings, "defaultTurbos")?
            .or(stored_turbos(turbo_settings, "synapseDefaultTurbos")?),
        synapse_turbos: Some(stored_turbos(turbo_settings, "synapseTurbos")?.unwrap_or_default()),
        dkm_keys: match context.product_id {
            70 => mouse_legacy_mapping::source_dkm_keys()?,
            182 => super::mouse_defaults::source_dkm_keys_182(),
            _ => unreachable!(),
        },
        fresh_guids: fresh_guids.into_iter().collect::<VecDeque<_>>(),
        ..Default::default()
    };
    let mut all_macros = context.existing_macros.to_vec();
    let mut remapped = BTreeMap::new();
    let mut imported_macros = Vec::new();
    for value in selected_macros {
        let identity: ProfileIdentity = serde_json::from_value(value.clone())
            .context("Imported mouse macro identity is invalid")?;
        ensure!(
            !identity.guid.is_empty(),
            "Imported mouse macro GUID is empty"
        );
        let name = super::imported_name(&all_macros, &identity.name);
        let mut imported = value
            .as_object()
            .expect("parsed mouse macro object")
            .clone();
        imported.insert("name".into(), name.clone().into());
        let guid = if macros.contains_key(&identity.guid) {
            let guid = engine_context
                .fresh_guids
                .pop_front()
                .context("Fresh imported mouse macro UUID is required")?;
            ensure!(valid_guid(&guid), "Imported mouse macro UUID v4 is invalid");
            ensure!(
                !macros.contains_key(&guid),
                "Imported mouse macro UUID is repeated"
            );
            remapped.insert(identity.guid, (guid.clone(), name.clone()));
            imported.insert("guid".into(), guid.clone().into());
            guid
        } else {
            identity.guid
        };
        let imported = super::prepare_macro(&Value::Object(imported))?;
        macros.insert(guid, name);
        all_macros.push(imported.clone());
        imported_macros.push(imported);
    }
    let mut compiled_any = false;
    let mut profiles = context.existing_profiles.to_vec();
    for row in rows {
        let mut profile = defaults
            .as_object()
            .expect("typed mouse default profile")
            .clone();
        profile.extend(row.profile.fields);
        profile.insert("name".into(), row.profile.name.into());
        let unavailable: BTreeSet<_> = row
            .list_items
            .into_iter()
            .filter(|selection| !super::truthy(&selection.selected))
            .filter(|selection| !original_macros.contains_key(&selection.guid))
            .map(|selection| selection.guid)
            .collect();
        let mut mappings = profile
            .get("mappings")
            .filter(|value| super::truthy(value))
            .map(|value| {
                value
                    .as_array()
                    .cloned()
                    .context("Imported mouse mappings are not an array")
            })
            .transpose()?;
        if let Some(mappings) = &mut mappings {
            mappings.retain(|mapping| {
                mapping
                    .get("macroGroup")
                    .and_then(|group| group.get("guid"))
                    .and_then(Value::as_str)
                    .is_none_or(|guid| !unavailable.contains(guid))
            });
        }
        if let Some(panels) = profile
            .get_mut("sidePanelMappings")
            .filter(|value| super::truthy(value))
        {
            for mappings in panels
                .as_object_mut()
                .context("Imported mouse side-panel mappings are invalid")?
                .values_mut()
            {
                for mapping in mappings
                    .as_array_mut()
                    .context("Imported mouse side-panel mapping list is invalid")?
                {
                    repair_mapping(mapping, &macros, &remapped)?;
                }
            }
        }
        if let Some(mappings) = &mut mappings {
            for mapping in mappings.iter_mut() {
                repair_mapping(mapping, &macros, &remapped)?;
            }
            if engine_context
                .default_turbos
                .as_ref()
                .is_none_or(Vec::is_empty)
            {
                engine.generate_default_turbos(&mut engine_context)?;
            }
            // Xt passes the profile array as je/$e's third argument. Even an
            // empty JS array is truthy, so this branch uses multi mappings.
            // The source factory dispatches only outputType. Imported analog
            // wrappers without one produce no ordinary engine records; their
            // stages remain in the saved profile after macro repair.
            let ordinary = mappings
                .iter()
                .filter(|mapping| mapping.get("outputType").and_then(Value::as_str).is_some())
                .map(|mapping| {
                    let mut mapping = mapping.clone();
                    mapping
                        .as_object_mut()
                        .expect("validated mapping")
                        .remove("mapping");
                    mapping
                })
                .collect::<Vec<_>>();
            let mut app_engine = engine
                .generate_multi(&ordinary, &mut engine_context)?
                .app_engine;
            let generated = app_engine
                .get_mut("mappings")
                .and_then(Value::as_array_mut)
                .expect("generated mouse engine mappings");
            generated.retain(|mapping| {
                !mapping
                    .get("input")
                    .and_then(|input| input.get("otfs"))
                    .is_some_and(super::truthy)
            });
            if mappings.iter().any(|mapping| {
                mapping
                    .get("sensitivityGroup")
                    .and_then(|group| group.get("sensitivityAssignment"))
                    .and_then(Value::as_str)
                    == Some("DPI_OnTheFly")
            }) {
                let scroll = ["ScrollUp", "ScrollDown"].map(|input| {
                    json!({"outputType":"sensitivityGroup","inputType":"MouseInput","inputID":input,
                        "sensitivityGroup":{"sensitivityAssignment":"OTFS_Scroll"}})
                });
                let scroll = engine.generate(&scroll, &mut engine_context)?;
                generated.extend(
                    scroll.app_engine["mappings"]
                        .as_array()
                        .expect("generated OTFS mappings")
                        .iter()
                        .cloned(),
                );
            }
            // The fourth prune argument is absent in Xt. Its existing profile
            // array is the third (multi-mode) argument, not a keep-alive list.
            let previous_turbos = turbo_ids(profile.get("appEngine"));
            let new_turbos = turbo_ids(Some(&app_engine));
            if !previous_turbos.is_empty() {
                engine_context
                    .synapse_turbos
                    .as_mut()
                    .expect("observed turbo list")
                    .retain(|turbo| {
                        turbo
                            .get("guid")
                            .and_then(Value::as_str)
                            .is_none_or(|guid| {
                                !previous_turbos.contains(guid) || new_turbos.contains(guid)
                            })
                    });
            }
            app_engine["hash"] = source_hash(&app_engine)?.into();
            profile.insert("mappings".into(), Value::Array(mappings.clone()));
            profile.insert("appEngine".into(), app_engine);
            compiled_any = true;
        }
        let name = super::imported_name(
            &profiles,
            profile
                .get("name")
                .and_then(Value::as_str)
                .expect("imported name"),
        );
        let guid = engine_context
            .fresh_guids
            .pop_front()
            .context("Fresh mouse import UUID is required")?;
        ensure!(
            valid_guid(&guid),
            "Imported mouse profile UUID v4 is invalid"
        );
        ensure!(
            identities.insert(guid.clone()),
            "Imported mouse profile UUID is repeated"
        );
        profile.insert("name".into(), name.into());
        profile.insert("guid".into(), guid.into());
        if !profile.get("equalizer").is_some_and(super::truthy)
            && let Some(equalizer) = context
                .selected_profile
                .and_then(|profile| profile.get("equalizer"))
                .filter(|value| super::truthy(value))
        {
            profile.insert("equalizer".into(), equalizer.clone());
        }
        for previous in &profiles {
            if let Some(camera) = profile.get("camera").filter(|value| super::truthy(value))
                && let Some(previous) = previous.get("camera").filter(|value| super::truthy(value))
            {
                let mut merged = previous
                    .as_object()
                    .context("Current mouse camera extensions are not an object")?
                    .clone();
                merged.extend(
                    camera
                        .as_object()
                        .context("Imported mouse camera extensions are not an object")?
                        .clone(),
                );
                profile.insert("camera".into(), Value::Object(merged));
            }
        }
        // The complete 70/182 singleton defaults and bootstrap useFeature
        // registrations enable neither supportDpiMatcher nor serial storage.
        // Xt therefore preserves selectedSensitivityMatcher without adding it.
        profiles.push(Value::Object(profile));
    }
    if let Some(storage_field) = storage_field {
        let selected_guid = context
            .selected_profile
            .map(|profile| {
                profile
                    .get("guid")
                    .and_then(Value::as_str)
                    .context("Selected mouse profile GUID is absent")
            })
            .transpose()?;
        let document = mouse_legacy_mapping::append_imported_profiles(
            context.existing_profiles,
            &profiles[context.existing_profiles.len()..],
            selected_guid,
            stored_document.as_ref(),
            &mut engine_context,
        )?;
        settings.insert(storage_field.into(), document);
    } else {
        if compiled_any {
            settings.insert(
                "defaultTurbos".into(),
                engine_context
                    .default_turbos
                    .expect("generated default turbos")
                    .into(),
            );
            settings.insert(
                "synapseTurbos".into(),
                engine_context
                    .synapse_turbos
                    .expect("observed turbo list")
                    .into(),
            );
        }
        let version: DeviceVersion = serde_json::from_value(Value::Object(settings.clone()))
            .context("Current mouse source version is invalid")?;
        settings.insert(
            "version".into(),
            next_source_version(version.version)?.into(),
        );
    }
    Ok(PreparedMouseImport {
        profiles,
        device_settings: Value::Object(settings),
        imported_macros,
    })
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
