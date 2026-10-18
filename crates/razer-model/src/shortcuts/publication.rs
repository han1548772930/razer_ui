//! Validated documents for the local mapping engine's five storage consumers.
use super::{Shortcut, native};
use crate::macro_library::MacroLibraryFile;
use razer_device::{
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Vendor JSON stays at the native document boundary. Construction and IPC
/// deserialization validate the complete dependency set before publication.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "DocumentFields")]
pub struct CompiledDocument {
    default_turbos: Vec<Value>,
    synapse_turbos: Vec<Value>,
    macros: Vec<Value>,
    phased_macros: Vec<Value>,
    app_engine: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentFields {
    default_turbos: Vec<Value>,
    synapse_turbos: Vec<Value>,
    macros: Vec<Value>,
    phased_macros: Vec<Value>,
    app_engine: Value,
}

impl TryFrom<DocumentFields> for CompiledDocument {
    type Error = String;
    fn try_from(fields: DocumentFields) -> Result<Self, String> {
        let document = Self {
            default_turbos: fields.default_turbos,
            synapse_turbos: fields.synapse_turbos,
            macros: fields.macros,
            phased_macros: fields.phased_macros,
            app_engine: fields.app_engine,
        };
        document.validate()?;
        Ok(document)
    }
}

impl CompiledDocument {
    pub fn default_turbos(&self) -> &[Value] {
        &self.default_turbos
    }
    pub fn synapse_turbos(&self) -> &[Value] {
        &self.synapse_turbos
    }
    pub fn macros(&self) -> &[Value] {
        &self.macros
    }
    pub fn phased_macros(&self) -> &[Value] {
        &self.phased_macros
    }
    pub fn app_engine(&self) -> &Value {
        &self.app_engine
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut identities = BTreeMap::new();
        let mut turbos = BTreeSet::new();
        let mut macros = BTreeSet::new();
        let mut phased_macros = BTreeSet::new();
        validate_collection(&self.default_turbos, &mut turbos, &mut identities)?;
        validate_collection(&self.synapse_turbos, &mut turbos, &mut identities)?;
        validate_collection(&self.macros, &mut macros, &mut identities)?;
        validate_collection(&self.phased_macros, &mut phased_macros, &mut identities)?;
        validate_engine(&self.app_engine, &["mappings"])?;
        let mappings = self
            .app_engine
            .get("mappings")
            .and_then(Value::as_array)
            .ok_or("Shortcut engine mappings must be an array")?;
        let mut inputs = BTreeSet::new();
        for mapping in mappings {
            let input = mapping
                .get("input")
                .ok_or("Shortcut mapping has no input")?;
            serde_json::from_value::<NativeInput>(input.clone())
                .map_err(|error| format!("Invalid native shortcut input: {error}"))?
                .validate()?;
            let identity = razer_device::mapping_engine_hash::canonical_json(input)
                .map_err(|e| e.to_string())?;
            if !inputs.insert(identity) {
                return Err("Duplicate native shortcut input".into());
            }
            let output = mapping
                .get("output")
                .ok_or("Shortcut mapping has no output")?;
            validate_references(output, &turbos, &macros, &phased_macros, 0)?;
        }
        for document in self
            .default_turbos
            .iter()
            .chain(&self.synapse_turbos)
            .chain(&self.macros)
            .chain(&self.phased_macros)
        {
            for lane in ["events", "pressEvents", "holdEvents", "releaseEvents"] {
                if let Some(events) = document
                    .get("appEngine")
                    .and_then(|engine| engine.get(lane))
                {
                    for event in events
                        .as_array()
                        .ok_or("Native event lane must be an array")?
                    {
                        validate_references(event, &turbos, &macros, &phased_macros, 0)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_collection(
    documents: &[Value],
    target: &mut BTreeSet<String>,
    identities: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    for document in documents {
        let guid = document
            .get("guid")
            .and_then(Value::as_str)
            .ok_or("Native document has no UUID")?;
        let parsed = uuid::Uuid::parse_str(guid).map_err(|_| "Invalid native document UUID")?;
        if identities
            .get(&parsed.to_string())
            .is_some_and(|existing| existing != document)
        {
            return Err("Conflicting native document UUID".into());
        }
        identities.insert(parsed.to_string(), document.clone());
        if !target.insert(guid.to_owned()) {
            return Err("Duplicate native document UUID in catalog".into());
        }
        if document.get("name").and_then(Value::as_str).is_none() {
            return Err("Native document has no name".into());
        }
        let engine = document
            .get("appEngine")
            .ok_or("Native document has no engine")?;
        let lanes: &[&str] =
            if document.get("macroPlaybackOption").and_then(Value::as_str) == Some("phased") {
                &["events", "pressEvents", "holdEvents", "releaseEvents"]
            } else {
                &["events"]
            };
        validate_engine(engine, lanes)?;
    }
    Ok(())
}

fn validate_engine(engine: &Value, lanes: &[&str]) -> Result<(), String> {
    for lane in lanes {
        if engine.get(*lane).and_then(Value::as_array).is_none() {
            return Err(format!("Native engine has no {lane} lane"));
        }
    }
    let hash = engine
        .get("hash")
        .and_then(Value::as_str)
        .ok_or("Native engine has no hash")?;
    if source_hash(engine).map_err(|e| e.to_string())? != hash {
        return Err("Native engine hash does not match its contents".into());
    }
    Ok(())
}

fn validate_references(
    value: &Value,
    turbos: &BTreeSet<String>,
    macros: &BTreeSet<String>,
    phased: &BTreeSet<String>,
    depth: usize,
) -> Result<(), String> {
    let output: NativeOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("Invalid native shortcut output: {error}"))?;
    output.validate(turbos, macros, phased, depth)
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum NativeInput {
    #[serde(rename = "keyboard")]
    Keyboard {
        scancode: u16,
        flag: u8,
        modifiers: u16,
        hypershift: bool,
    },
    #[serde(rename = "mouse")]
    Mouse {
        id: u32,
        modifiers: u16,
        hypershift: bool,
    },
    #[serde(rename = "razerKey")]
    RazerKey {
        key: u16,
        flag: u8,
        modifiers: u16,
        hypershift: bool,
    },
}

impl NativeInput {
    fn validate(self) -> Result<(), String> {
        let (flag, modifiers) = match self {
            Self::Keyboard {
                scancode,
                flag,
                modifiers,
                hypershift,
            } => {
                let _ = hypershift;
                if scancode == 0 {
                    return Err("Shortcut keyboard input has no scan code".into());
                }
                (flag, modifiers)
            }
            Self::Mouse {
                id,
                modifiers,
                hypershift,
            } => {
                let _ = hypershift;
                if id == 0 {
                    return Err("Shortcut mouse input has no button".into());
                }
                (0, modifiers)
            }
            Self::RazerKey {
                key,
                flag,
                modifiers,
                hypershift,
            } => {
                let _ = (key, hypershift);
                (flag, modifiers)
            }
        };
        if flag > 5 || modifiers > 4095 {
            return Err("Shortcut input flags or modifiers are outside the native range".into());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum NativeOutput {
    Disabled,
    Keyboard {
        scancode: u16,
        flag: u8,
    },
    Mouse {
        id: u32,
        data: Option<u32>,
    },
    Delay {
        ms: f64,
    },
    RandomDelay {
        #[serde(rename = "minMs")]
        min_ms: f64,
        #[serde(rename = "maxMs")]
        max_ms: f64,
    },
    Audio {
        id: String,
    },
    Display {
        id: String,
    },
    Launch {
        path: String,
    },
    Clipboard {
        id: String,
        text: String,
    },
    MouseMove {
        x: f64,
        y: f64,
    },
    Loop {
        flag: u8,
        repeat: Option<f64>,
    },
    Turbo {
        guid: String,
        flag: u8,
    },
    Macro {
        guid: String,
        flag: u8,
    },
    PhasedMacro {
        guid: String,
        flag: u8,
    },
    Multi {
        outputs: Vec<NativeOutput>,
    },
}

impl NativeOutput {
    fn validate(
        &self,
        turbos: &BTreeSet<String>,
        macros: &BTreeSet<String>,
        phased: &BTreeSet<String>,
        depth: usize,
    ) -> Result<(), String> {
        if depth >= 32 {
            return Err("Native output nesting exceeds the supported boundary".into());
        }
        let duration = |ms: f64| ms.is_finite() && ms >= 0. && ms <= f64::from(u32::MAX);
        match self {
            Self::Turbo { guid, flag }
            | Self::Macro { guid, flag }
            | Self::PhasedMacro { guid, flag } => {
                let catalog = match self {
                    Self::Turbo { .. } => turbos,
                    Self::Macro { .. } => macros,
                    _ => phased,
                };
                if !catalog.contains(guid) {
                    return Err("Unresolved native output target UUID".into());
                }
                if *flag > 5 {
                    return Err("Native output flag is outside its supported range".into());
                }
            }
            Self::Multi { outputs } => {
                for output in outputs {
                    output.validate(turbos, macros, phased, depth + 1)?;
                }
            }
            Self::Keyboard { scancode, flag } if *scancode == 0 || *flag > 5 => {
                return Err("Native keyboard output has invalid scan code or flags".into());
            }
            Self::Delay { ms } if !duration(*ms) => {
                return Err("Native delay is outside the supported range".into());
            }
            Self::RandomDelay { min_ms, max_ms }
                if !duration(*min_ms) || !duration(*max_ms) || min_ms > max_ms =>
            {
                return Err("Native random delay bounds are invalid".into());
            }
            Self::Audio { id } | Self::Display { id } if id.is_empty() => {
                return Err("Native output has no action identity".into());
            }
            Self::Launch { path } if path.is_empty() || path.contains('\0') => {
                return Err("Native launch output has no valid command".into());
            }
            Self::Clipboard { id, text } if id != "text" || text.contains('\0') => {
                return Err("Native clipboard output is invalid".into());
            }
            Self::MouseMove { x, y } if !x.is_finite() || !y.is_finite() => {
                return Err("Native mouse movement is nonfinite".into());
            }
            Self::Loop { flag, repeat }
                if *flag > 1
                    || *flag == 0
                        && !repeat.is_some_and(|value| value.is_finite() && value >= 0.) =>
            {
                return Err("Native loop output is invalid".into());
            }
            Self::Mouse { id, data } => {
                let _ = (id, data);
            }
            _ => {}
        }
        Ok(())
    }
}

/// Owns generated default identities for one long-lived mapping session.
/// Failed compilations do not change the last usable context or any user data.
#[derive(Default)]
pub struct Compiler {
    context: EngineContext,
}

impl Compiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compile(
        &mut self,
        shortcuts: &[Shortcut],
        library: &MacroLibraryFile,
    ) -> Result<CompiledDocument, String> {
        super::validate_stored_shortcuts(shortcuts)?;
        let mut context = self.context.clone();
        if context.default_turbos.is_none() {
            let factory = MappingEngine::global_shortcuts();
            context.fresh_guids = (0..factory.default_turbo_count())
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect::<VecDeque<_>>();
            factory
                .generate_default_turbos(&mut context)
                .map_err(|e| e.to_string())?;
        }
        context.synapse_turbos = Some(Vec::new());
        context.is_global_shortcuts = true;
        let source = library.source_macro_documents()?;
        let published = razer_device::macro_engine::publication_documents(&source)
            .map_err(|e| e.to_string())?;
        // Js/qs sends the complete saved list to the consumer selected by the
        // edited macro type. Both consumers therefore require the same list.
        // Source output edits are saved before the independent input recorder.
        // A row without an input produces no native mappings and stays local.
        let recorded: Vec<_> = shortcuts
            .iter()
            .filter(|shortcut| !shortcut.input().is_empty())
            .cloned()
            .collect();
        let app_engine = native::encode_with_context(&recorded, &mut context, Some(library))?;
        let document = CompiledDocument::try_from(DocumentFields {
            default_turbos: context
                .default_turbos
                .clone()
                .ok_or("Default Turbo compilation did not produce a catalog")?,
            synapse_turbos: context
                .synapse_turbos
                .clone()
                .ok_or("Turbo compilation did not produce a catalog")?,
            macros: published.clone(),
            phased_macros: published,
            app_engine,
        })?;
        self.context = context;
        Ok(document)
    }
}

#[cfg(test)]
#[path = "publication_tests.rs"]
mod tests;
