//! Pure current-source app-engine generation for products 70/190/515/678/679/688.
//! No persistence, native publication or device acknowledgment occurs here.
//! Unknown/unimplemented branches fail explicitly; they are never successful saves.
use crate::mapping_engine_hash::source_hash;
use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};

#[path = "mapping_engine_data.rs"]
pub(crate) mod data;
use data::{EngineData, KeyDefinition};

/// Shared source keys for pure macro compilation; engine defaults remain private.
pub(crate) fn source_keys(product_id: u32) -> Option<&'static [KeyDefinition]> {
    data::for_product(product_id).map(|data| data.keys)
}

#[derive(Clone, Debug, Default)]
pub struct EngineContext {
    /// Actual observed storage lists; absence differs from a valid empty list.
    pub default_turbos: Option<Vec<Value>>,
    pub synapse_turbos: Option<Vec<Value>>,
    /// Runtime updateDKMKeys data. Do not synthesize physical key identifiers.
    pub dkm_keys: BTreeMap<String, Value>,
    /// Fresh UUID v4 identifiers allocated by the caller. Consumed only on success.
    pub fresh_guids: VecDeque<String>,
    pub is_analog_gen2: bool,
    /// Current factory's setIsGlobalShortcuts state; device mappings default false.
    pub is_global_shortcuts: bool,
    pub analog_generation: Option<String>,
    pub analog_binary_range: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedEngine {
    pub app_engine: Value,
    pub synapse_turbos: Vec<Value>,
}

pub struct MappingEngine {
    data: &'static EngineData,
}

/// Native input edges encoded by the selected source factory.
pub struct EncodedShortcutInput {
    down: Value,
    up: Value,
}

impl EncodedShortcutInput {
    pub fn into_parts(self) -> (Value, Value) {
        (self.down, self.up)
    }
}

fn input_modifier_bit(id: &str) -> Option<u32> {
    Some(match id {
        "ALT" => 1,
        "CTRL" => 2,
        "SHIFT" => 4,
        "GUI" => 8,
        "KEY_LEFT_ALT" => 256,
        "KEY_LEFT_CTRL" => 512,
        "KEY_LEFT_SHIFT" => 1024,
        "KEY_LEFT_GUI" => 2048,
        "KEY_RIGHT_ALT" => 16,
        "KEY_RIGHT_CTRL" => 32,
        "KEY_RIGHT_SHIFT" => 64,
        "KEY_RIGHT_GUI" => 128,
        _ => return None,
    })
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(v)) => *v,
        Some(Value::Number(v)) => v.as_f64().is_some_and(|v| v != 0.0),
        Some(Value::String(v)) => !v.is_empty(),
        Some(_) => true,
    }
}

fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing source {key}"))
}

fn copy_field(to: &mut Value, target: &str, from: &Value, source: &str) {
    if let Some(v) = from.get(source) {
        to[target] = v.clone();
    }
}

// Current MW indexes points with [0]/[1]. Its local profile defaults use
// objects with numeric keys, while imported profiles may contain arrays.
fn actuation_point(record: &Value, index: usize) -> Result<Value> {
    let points = record
        .get("actuationPoint")
        .context("Missing observed actuation points")?;
    let point = match points {
        Value::Array(points) => points.get(index),
        Value::Object(points) => points.get(&index.to_string()),
        _ => None,
    }
    .context("Missing observed actuation point")?;
    ensure!(point.is_number(), "Actuation point is not numeric");
    Ok(point.clone())
}

fn multi(outputs: Vec<Value>) -> Value {
    json!({"type":"multi","outputs":outputs})
}
fn delay(ms: u32) -> Value {
    json!({"type":"delay","ms":ms})
}
fn disabled() -> Value {
    json!({"type":"disabled"})
}

#[derive(Clone, Copy)]
struct MouseKey {
    down: u32,
    up: u32,
    data: Option<u32>,
    id: Option<u32>,
    turbo: Option<&'static str>,
    repeat: Option<u32>,
    delay: Option<u32>,
}

fn mouse_key(id: &str) -> Result<MouseKey> {
    let (down, up, data, turbo, repeat, delay, special_id) = match id {
        "LeftClick" | "Click" => (1, 2, None, None, None, None, None),
        "RightClick" | "Menu" => (4, 8, None, None, None, None, None),
        "ScrollButton" => (16, 32, None, None, None, None, None),
        "Button4" | "Previous" => (64, 128, None, None, None, None, None),
        "Button5" | "Next" => (256, 512, None, None, None, None, None),
        "ScrollUp" => (1024, 0, Some(120), None, None, None, None),
        "ScrollDown" => (1024, 0, Some(65416), None, None, None, None),
        "ScrollLeft" => (2048, 0, Some(65416), None, None, None, None),
        "ScrollRight" => (2048, 0, Some(120), None, None, None, None),
        "DoubleClick" => (0, 0, None, Some("DoubleClick"), Some(1), None, None),
        "RepeatScrollLeft" => (
            1,
            2,
            None,
            Some("RepeatScrollLeft"),
            Some(0),
            Some(30),
            Some(0),
        ),
        "RepeatScrollRight" => (
            4,
            8,
            None,
            Some("RepeatScrollRight"),
            Some(0),
            Some(30),
            Some(0),
        ),
        "RepeatScrollUp" => (
            1,
            2,
            None,
            Some("RepeatScrollUp"),
            Some(0),
            Some(30),
            Some(0),
        ),
        "RepeatScrollDown" => (
            4,
            8,
            None,
            Some("RepeatScrollDown"),
            Some(0),
            Some(30),
            Some(0),
        ),
        _ => bail!("Unresolved source mouse assignment {id}"),
    };
    Ok(MouseKey {
        down,
        up,
        data,
        id: special_id,
        turbo,
        repeat,
        delay,
    })
}

fn mouse_event(key: MouseKey, up: bool) -> Value {
    let mut out = json!({"type":"mouse","id":if up {key.up} else {key.down}});
    if let Some(data) = key.data {
        out["data"] = json!(data);
    }
    out
}

impl MappingEngine {
    pub(crate) fn source_keys(&self) -> &'static [KeyDefinition] {
        self.data.keys
    }

    pub fn new(product_id: u32) -> Result<Self> {
        let data = data::for_product(product_id)
            .with_context(|| format!("Active factory not audited for product {product_id}"))?;
        Ok(Self { data })
    }

    /// The current Dashboard factory is independent of a connected product.
    pub fn global_shortcuts() -> Self {
        Self {
            data: data::for_global_shortcuts(),
        }
    }

    /// Encodes a validated global shortcut through the ordinary source input
    /// rules, including physical modifier sides and extended release flags.
    pub fn encode_shortcut_input(
        &self,
        input_id: &str,
        modifiers: &[String],
        hypershift: bool,
    ) -> Result<EncodedShortcutInput> {
        ensure!(
            modifiers.iter().all(|id| input_modifier_bit(id).is_some()),
            "Unknown native shortcut modifier"
        );
        let input_type = match input_id {
            "RightClick" | "ScrollButton" | "Button4" | "Button5" => "MouseInput",
            _ => {
                ensure!(
                    self.key(&json!(input_id))?.scancode != 0,
                    "Shortcut has no native scan code"
                );
                "KeyInput"
            }
        };
        let record = json!({
            "inputID": input_id,
            "inputType": input_type,
            "inputModifiers": modifiers,
            "isHyperShift": hypershift,
        });
        let (mut down, up) = self.inputs(&record, &EngineContext::default())?;
        ensure!(down.len() == 1, "Shortcut must encode one press edge");
        Ok(EncodedShortcutInput {
            down: down.pop().expect("Exactly one encoded shortcut input"),
            up: up.context("Shortcut must encode a release edge")?,
        })
    }

    /// Original `getKeyByInputID` metadata for source transforms such as Snap Tap.
    pub fn source_key_metadata(&self, input_id: &str) -> Result<Value> {
        serde_json::to_value(self.key(&json!(input_id))?).map_err(Into::into)
    }

    fn data(&self) -> &'static EngineData {
        self.data
    }

    fn key(&self, id: &Value) -> Result<&'static KeyDefinition> {
        let id = id
            .as_str()
            .or_else(|| id.get("inputID").and_then(Value::as_str))
            .context("Missing source key identifier")?;
        self.data()
            .keys
            .iter()
            .find(|k| k.input_id == Some(id))
            .with_context(|| format!("Unresolved source key {id}"))
    }

    fn key_event(&self, id: &Value, up: bool) -> Result<Value> {
        let key = self.key(id)?;
        let flag = key.output_flag.unwrap_or(key.flag);
        Ok(json!({"type":"keyboard","scancode":key.scancode,"flag":u64::from(flag)+u64::from(up)}))
    }

    fn is_single(&self, id: &str) -> bool {
        self.data().single_state_input_ids.contains(&id)
    }

    fn is_scroll(&self, id: &str) -> bool {
        self.data().scroll_input_ids.contains(&id)
    }

    fn guid_by_name(
        &self,
        name: &str,
        context: &EngineContext,
        defaults: bool,
    ) -> Result<Option<String>> {
        let turbos = if defaults {
            context.default_turbos.as_ref()
        } else {
            context.synapse_turbos.as_ref()
        }
        .context("Turbo storage not observed")?;
        Ok(turbos
            .iter()
            .find(|t| t["name"] == name)
            .and_then(|t| t["guid"].as_str())
            .map(str::to_owned))
    }

    fn next_guid(context: &mut EngineContext) -> Result<String> {
        let guid = context
            .fresh_guids
            .pop_front()
            .context("Fresh turbo UUID is required")?;
        let bytes = guid.as_bytes();
        ensure!(
            bytes.len() == 36
                && [8, 13, 18, 23].iter().all(|i| bytes[*i] == b'-')
                && bytes[14] == b'4'
                && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
                && bytes
                    .iter()
                    .enumerate()
                    .all(|(i, b)| [8, 13, 18, 23].contains(&i) || b.is_ascii_hexdigit()),
            "Invalid turbo UUID v4"
        );
        Ok(guid)
    }

    fn add_turbo(
        &self,
        name: String,
        events: Vec<Value>,
        context: &mut EngineContext,
    ) -> Result<String> {
        ensure!(
            context.synapse_turbos.is_some(),
            "Synapse turbo storage not observed"
        );
        let guid = Self::next_guid(context)?;
        let mut app_engine = json!({"events":events});
        app_engine["hash"] = json!(source_hash(&app_engine)?);
        context
            .synapse_turbos
            .as_mut()
            .unwrap()
            .push(json!({"name":name,"guid":guid,"appEngine":app_engine}));
        Ok(guid)
    }

    /// Number of fresh UUIDs consumed by the selected source factory.
    pub fn default_turbo_count(&self) -> usize {
        self.data().default_turbos.len()
    }

    /// Actual default event data, with caller-provided UUIDs. No storage write.
    pub fn generate_default_turbos(&self, context: &mut EngineContext) -> Result<Vec<Value>> {
        let mut draft = context.clone();
        let mut turbos = Vec::new();
        for turbo in self.data().default_turbos {
            let name = turbo.id;
            let mut app_engine = json!({"events":turbo.events});
            app_engine["hash"] = json!(source_hash(&app_engine)?);
            turbos.push(json!({"name":name,"id":name,"guid":Self::next_guid(&mut draft)?,"appEngine":app_engine}));
        }
        draft.default_turbos = Some(turbos.clone());
        *context = draft;
        Ok(turbos)
    }

    fn inputs(
        &self,
        record: &Value,
        context: &EngineContext,
    ) -> Result<(Vec<Value>, Option<Value>)> {
        let id = string(record, "inputID")?;
        let modifiers = if let Some(m) = record.get("inputModifiers").filter(|m| truthy(Some(m))) {
            let mut mask = 0u32;
            for m in m.as_array().context("Invalid inputModifiers")? {
                mask |= m.as_str().and_then(input_modifier_bit).unwrap_or(0);
            }
            Some(mask)
        } else {
            None
        };
        let decorate = |v: &mut Value| {
            copy_field(v, "hypershift", record, "isHyperShift");
            if let Some(m) = modifiers {
                v["modifiers"] = json!(m);
            }
        };
        match string(record, "inputType")? {
            "KeyInput" | "AnalogInput" => {
                let analog = record["inputType"] == "AnalogInput";
                let mut down = self.key_event(&json!(id), false)?;
                let mut up = self.key_event(&json!(id), true)?;
                if analog {
                    down["type"] = json!("analogKey");
                    up["type"] = json!("analogKey");
                }
                decorate(&mut down);
                decorate(&mut up);
                if let Some(interrupts) = record
                    .get("interrupts")
                    .filter(|v| truthy(Some(v)))
                    .filter(|_| {
                        !analog
                            || truthy(record.get("macroGroup"))
                            || truthy(record.get("dynamicKeyStrokeGroup"))
                    })
                {
                    let mut decoded = Vec::new();
                    for interrupt in interrupts.as_array().context("Invalid interrupts")? {
                        if let Ok(key) = self.key(interrupt) {
                            let mut event = self.key_event(interrupt, false)?;
                            if key.kind == "razerKey" {
                                event["type"] = json!("razerKey");
                            }
                            decoded.push(event);
                        }
                    }
                    down["interrupts"] = json!(decoded);
                }
                Ok((vec![down], Some(up)))
            }
            "DKMInput" => {
                let key = context
                    .dkm_keys
                    .get(id)
                    .context("Actual updateDKMKeys state required")?;
                let key = key.get("key").context("Missing DKM engine key")?;
                let mut down = json!({"type":"razerKey","key":key,"flag":0});
                let mut up = json!({"type":"razerKey","key":key,"flag":1});
                decorate(&mut down);
                decorate(&mut up);
                Ok((vec![down], Some(up)))
            }
            "MouseInput" => {
                let key = mouse_key(id)?;
                let event = |up: bool| {
                    let mut v = if key.id == Some(0) {
                        json!({"type":"mouse","id":0,"data":if up{key.up}else{key.down}})
                    } else {
                        mouse_event(key, up)
                    };
                    decorate(&mut v);
                    if record["sensitivityGroup"]["sensitivityAssignment"] == "OTFS_Scroll" {
                        v["otfs"] = json!(true);
                    }
                    v
                };
                let down = event(false);
                let downs = if id == "ScrollUp" || id == "ScrollDown" {
                    (0..10)
                        .map(|n| {
                            let mut v = down.clone();
                            v["data"] = json!(if id == "ScrollUp" {
                                120 + 120 * n
                            } else {
                                65416 - 120 * n
                            });
                            v
                        })
                        .collect()
                } else {
                    vec![down]
                };
                Ok((downs, (key.up != 0).then(|| event(true))))
            }
            "ControlPodInput" => {
                let value = match id {
                    "ControlPodClick" => json!({"id":"topButton","data":1}),
                    "ControlPodDoubleClick" => json!({"id":"topButton","data":2}),
                    "ControlPodTripleClick" => json!({"id":"topButton","data":3}),
                    "ControlPodPressHold" => json!({"id":"topButton","data":4}),
                    "ControlPodSourceClick" => json!({"id":"sourceButton","data":1}),
                    "ControlPodSourceDoubleClick" => json!({"id":"sourceButton","data":2}),
                    "ControlPodClockwise" => {
                        json!({"id":"dial","rotation":"clockwise","repeat":true})
                    }
                    "ControlPodAnticlockwise" => {
                        json!({"id":"dial","rotation":"anticlockwise","repeat":true})
                    }
                    _ => bail!("Unresolved Control Pod input {id}"),
                };
                let mut down = json!({"type":"controlPod"});
                if let Some(object) = value.as_object() {
                    down.as_object_mut()
                        .expect("controlPod output is an object")
                        .extend(object.clone());
                }
                decorate(&mut down);
                Ok((vec![down], None))
            }
            "CoolingPadInput" => {
                let (id, byte1, byte2) = match id {
                    "CycleChromaLighting" => (70, 2, 3),
                    "ChromaDisabled" => (70, 2, 2),
                    "ChromaEnabled" => (70, 2, 1),
                    "CycleFanSpeedMode" => (70, 1, 3),
                    "TurnOffFan" => (70, 1, 2),
                    "TurnOnFan" => (70, 1, 1),
                    "PowerOffFan" => (9, 2, 8),
                    "PowerOnFan" => (9, 3, 8),
                    "ExternalPowerOn" => (12, 1, 0),
                    "ExternalPowerOff" => (12, 0, 0),
                    _ => bail!("Unresolved Cooling Pad input {id}"),
                };
                let mut down = json!({"type":"coolingPad","id":id,"byte1":byte1,"byte2":byte2});
                decorate(&mut down);
                Ok((vec![down], None))
            }
            other => bail!("Input generator not yet implemented: {other}"),
        }
    }

    /// Generates the real ordinary factory data atomically. Does not publish it.
    pub fn generate(
        &self,
        records: &[Value],
        context: &mut EngineContext,
    ) -> Result<GeneratedEngine> {
        let mut draft = context.clone();
        let mut mappings = Vec::new();
        for (index, record) in records.iter().enumerate() {
            mappings.extend(
                self.record(record, &mut draft)
                    .with_context(|| format!("Mapping {index}"))?,
            );
        }
        let mut engine = json!({"mappings":mappings});
        engine["hash"] = json!(source_hash(&engine)?);
        let result = GeneratedEngine {
            app_engine: engine,
            synapse_turbos: draft.synapse_turbos.clone().unwrap_or_default(),
        };
        *context = draft;
        Ok(result)
    }

    pub fn record(&self, record: &Value, context: &mut EngineContext) -> Result<Vec<Value>> {
        ensure!(
            record.get("mapping").is_none(),
            "Analog wrapper requires generate_analog; ordinary pairing would lose actuation branches"
        );
        if record["outputType"] == "defaultGroup" {
            return Ok(Vec::new());
        }
        let (downs, up) = self.inputs(record, context)?;
        let (mut down_output, up_output) = self.outputs(record, context, 0)?;
        // Current source treats Control Pod and Cooling Pad inputs as
        // one-shot inputs. When the selected output is a keyboard/mouse or a
        // profile-navigation action, it explicitly combines the make and
        // release outputs into the single hardware event.
        if matches!(
            record["inputType"].as_str(),
            Some("ControlPodInput" | "CoolingPadInput")
        ) {
            let release_type = up_output.get("type").and_then(Value::as_str);
            if matches!(
                down_output.get("type").and_then(Value::as_str),
                Some("keyboard" | "mouse")
            ) || matches!(release_type, Some("switchProfile" | "navigateProfile"))
            {
                down_output = multi(vec![down_output, up_output.clone()]);
            }
            return Ok(vec![
                json!({"input":downs.into_iter().next().context("Missing one-shot input")?,"output":down_output}),
            ]);
        }
        let mut result = downs
            .into_iter()
            .map(|input| json!({"input":input,"output":down_output}))
            .collect::<Vec<_>>();
        if let Some(input) = up {
            result.push(json!({"input":input,"output":up_output}));
        }
        Ok(result)
    }

    fn keyboard_outputs(
        &self,
        record: &Value,
        context: &mut EngineContext,
    ) -> Result<(Value, Value)> {
        let group = record
            .get("keyboardGroup")
            .context("Missing keyboardGroup")?;
        let input_id = string(record, "inputID")?;
        let key = group.get("key").context("Missing keyboard output key")?;
        let mods = group
            .get("modifiers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut mods_down = Vec::new();
        let mut mods_up = Vec::new();
        let mut names = Vec::new();
        for modifier in &mods {
            mods_down.push(self.key_event(modifier, false)?);
            mods_up.push(self.key_event(modifier, true)?);
            names.push(self.key(modifier)?.name.to_owned());
        }
        let key_down = self.key_event(key, false)?;
        let key_up = self.key_event(key, true)?;
        if let Some(mouse) = group.get("mouseGroup") {
            ensure!(
                group.get("turboMode").is_none(),
                "Keyboard + mouse + turbo compound branch unresolved"
            );
            let assignment = string(mouse, "mouseAssignment")?;
            let m = mouse_key(assignment)?;
            let mut down = mods_down.clone();
            down.push(key_down.clone());
            down.push(delay(30));
            let mut up = vec![key_up.clone()];
            match assignment {
                "ScrollUp" | "ScrollDown" | "ScrollLeft" | "ScrollRight" => {
                    down.push(mouse_event(m, false));
                    up.extend(mods_up);
                }
                "DoubleClick" => {
                    down.extend([
                        json!({"type":"mouse","id":1,"data":0}),
                        json!({"type":"mouse","id":2,"data":0}),
                        delay(10),
                        json!({"type":"mouse","id":1,"data":0}),
                        json!({"type":"mouse","id":2,"data":0}),
                    ]);
                    up.extend(mods_up);
                }
                "RepeatScrollUp" | "RepeatScrollDown" => {
                    let guid = if assignment == "RepeatScrollUp" {
                        "8d147e18-f4f7-4c1f-a648-3dd92aab64ab"
                    } else {
                        "07130a0a-4df3-4d58-8d68-6aeb14df7f2b"
                    };
                    if !mods.is_empty() {
                        down[mods.len()]["flag"] = json!(0);
                        down.extend(mods_up.clone());
                    }
                    down.push(key_up.clone());
                    down.push(delay(30));
                    down.push(json!({"type":"turbo","guid":guid,"flag":0,"repeat":0,"delay":143}));
                    if !mods.is_empty() {
                        let mut key_up = key_up;
                        key_up["flag"] = json!(1);
                        let turbo_index = down.len() - 3;
                        down[turbo_index]["flag"] = json!(1);
                        up = mods_up;
                        up.push(key_up);
                    }
                    up.push(delay(30));
                    up.push(json!({"type":"mouse","id":m.up}));
                    up.push(json!({"type":"turbo","guid":guid,"flag":1}));
                }
                _ => {
                    let mut mouse_down = mouse_event(m, false);
                    if !mods.is_empty() {
                        mouse_down.as_object_mut().unwrap().remove("data");
                    }
                    down.push(mouse_down);
                    let lock = matches!(
                        key.as_str().unwrap_or(""),
                        "KEY_SCROLL_LOCK"
                            | "KEY_CAPS_LOCK"
                            | "KEY_NUMPAD_NUM_LOCK"
                            | "KEY_LEFT_ALT"
                            | "KEY_LEFT_CTRL"
                            | "KEY_LEFT_SHIFT"
                            | "KEY_LEFT_GUI"
                            | "KEY_RIGHT_ALT"
                            | "KEY_RIGHT_CTRL"
                            | "KEY_RIGHT_SHIFT"
                            | "KEY_RIGHT_GUI"
                    );
                    if !mods.is_empty() || lock {
                        up = vec![json!({"type":"mouse","id":m.up}), delay(30), key_up];
                        up.extend(mods_up);
                    } else {
                        up.push(delay(30));
                        up.push(json!({"type":"mouse","id":m.up}));
                    }
                }
            }
            return Ok((multi(down), multi(up)));
        }
        let turbo = group.get("turboMode");
        let single = self.is_single(input_id);
        if !single && !truthy(turbo) {
            return if mods.is_empty() {
                Ok((key_down, key_up))
            } else {
                mods_down.push(key_down);
                let mut up = vec![key_up];
                up.extend(mods_up);
                Ok((multi(mods_down), multi(up)))
            };
        }
        let mut lookup_names = names.clone();
        lookup_names.push(self.key(key)?.name.to_owned());
        ensure!(
            group.get("modifiers").and_then(Value::as_array).is_some(),
            "Source turbo generator requires observed modifiers array"
        );
        let name = lookup_names.join(" + ");
        let guid = match self.guid_by_name(&name, context, false)? {
            Some(guid) => guid,
            None => {
                let mut events = mods_down;
                events.push(key_down);
                if mods.is_empty() && self.is_scroll(input_id) {
                    events.push(delay(self.data().scroll_turbo_delay_ms));
                }
                events.push(key_up);
                mods_up.reverse();
                events.extend(mods_up);
                // ae mutates the temporary key list via reverse before naming.
                if !mods.is_empty() {
                    names.reverse();
                }
                names.push(self.key(key)?.name.to_owned());
                self.add_turbo(names.join(" + "), events, context)?
            }
        };
        let mut down = json!({"type":"turbo","guid":guid,"flag":0});
        if single {
            down["repeat"] = json!(1);
        } else {
            let rate = turbo
                .and_then(|t| t.get("keysPerSecond"))
                .and_then(Value::as_f64)
                .context("Missing turbo keysPerSecond")?;
            ensure!(rate > 0.0 && rate.is_finite(), "Invalid turbo rate");
            down["delay"] = json!((1000.0 / rate + 0.5).floor());
        }
        let up = json!({"type":"turbo","guid":guid,"flag":1});
        let alt_tab = !single
            && key == "KEY_TAB"
            && mods.iter().any(|v| {
                v == "KEY_LEFT_ALT"
                    || v == "KEY_RIGHT_ALT"
                    || v.get("inputID")
                        .is_some_and(|v| v == "KEY_LEFT_ALT" || v == "KEY_RIGHT_ALT")
            });
        Ok((down, if alt_tab { multi(vec![up]) } else { up }))
    }

    fn mouse_outputs(&self, record: &Value, context: &mut EngineContext) -> Result<(Value, Value)> {
        let group = record.get("mouseGroup").context("Missing mouseGroup")?;
        let assignment = string(group, "mouseAssignment")?;
        if assignment == "ScrollModeSwitch" {
            return Ok((disabled(), json!({"type":"scrollModeSwitch","flag":1})));
        }
        let m = mouse_key(assignment)?;
        let input = string(record, "inputID")?;
        let has_turbo = truthy(group.get("turboMode"));
        let turbo_delay = if has_turbo {
            // Current Mouse_default calculates this for every truthy turboMode,
            // including source defaults containing only {isTurbo:true}. Missing
            // keysPerSecond is undefined, so Math.max receives NaN and the native
            // JSON boundary receives an explicit null, rather than no delay.
            let rate = group["turboMode"]
                .get("keysPerSecond")
                .map(|value| js_numeric(value).unwrap_or(f64::NAN))
                .unwrap_or(f64::NAN);
            let delay = (1000.0 / rate + 0.5).floor() - 30.0;
            Some(js_number_value(if delay.is_nan() {
                delay
            } else {
                delay.max(0.0)
            }))
        } else {
            None
        };
        if let Some(name) = m.turbo {
            let guid = self
                .guid_by_name(name, context, true)?
                .context("Default turbo not observed")?;
            let mut down = json!({"type":"turbo","guid":guid,"flag":0});
            if let Some(ms) = turbo_delay {
                down["delay"] = ms;
            } else {
                if let Some(repeat) = m.repeat {
                    down["repeat"] = json!(repeat);
                }
                if let Some(ms) = m.delay {
                    down["delay"] = json!(ms);
                }
            }
            let up = if record["inputType"] == "DKMInput"
                && ["DKM_KBMK_01", "DKM_KBMK_02", "DKM_KBMK_03", "DKM_KBMK_04"].contains(&input)
                && name == "DoubleClick"
            {
                disabled()
            } else {
                json!({"type":"turbo","guid":guid,"flag":1})
            };
            return Ok((down, up));
        }
        let single_mouse =
            record["inputType"] == "MouseInput" && mouse_key(input)?.up == 0 && m.up != 0;
        if has_turbo || single_mouse {
            let guid = match self.guid_by_name(assignment, context, false)? {
                Some(guid) => guid,
                None => {
                    let mut events = vec![mouse_event(m, false)];
                    if m.up != 0 {
                        events.push(delay(30));
                        events.push(mouse_event(m, true));
                    }
                    self.add_turbo(assignment.to_owned(), events, context)?
                }
            };
            let mut down = json!({"type":"turbo","guid":guid,"flag":0});
            if !has_turbo {
                down["repeat"] = json!(1);
            }
            if let Some(ms) = turbo_delay {
                down["delay"] = ms;
            }
            return Ok((down, json!({"type":"turbo","guid":guid,"flag":1})));
        }
        Ok((
            mouse_event(m, false),
            if m.up == 0 {
                disabled()
            } else {
                mouse_event(m, true)
            },
        ))
    }

    fn media_outputs(&self, record: &Value, context: &EngineContext) -> Result<(Value, Value)> {
        let group = record
            .get("multimediaGroup")
            .context("Missing multimediaGroup")?;
        let assignment = string(group, "multimediaAssignment")?;
        let id = string(record, "inputID")?;
        let down = match assignment {
            "MuteMic" => json!({"type":"audio","id":"mic","mute":2}),
            "MuteAll" => json!({"type":"audio","id":"all","mute":2}),
            "MicVolumeDown" | "MicVolumeUp" => {
                json!({"type":"audio","id":if assignment=="MicVolumeDown"{"micVolumeDown"}else{"micVolumeUp"}})
            }
            "VolumeDown" | "VolumeUp" => {
                let scan = if assignment == "VolumeDown" { 46 } else { 48 };
                let down = json!({"type":"keyboard","scancode":scan,"flag":2});
                if self.is_scroll(id) {
                    multi(vec![
                        down,
                        delay(30),
                        json!({"type":"keyboard","scancode":scan,"flag":3}),
                    ])
                } else {
                    down
                }
            }
            "PrevTrack" | "NextTrack" => {
                json!({"type":"keyboard","scancode":if assignment=="PrevTrack"{16}else{25},"flag":2})
            }
            "TrackJoggingForward" | "TrackJoggingBackward" => {
                let scan = if assignment == "TrackJoggingForward" {
                    77
                } else {
                    75
                };
                multi(vec![
                    json!({"type":"keyboard","scancode":42,"flag":0}),
                    json!({"type":"keyboard","scancode":scan,"flag":2}),
                    json!({"type":"keyboard","scancode":scan,"flag":3}),
                    json!({"type":"keyboard","scancode":42,"flag":1}),
                ])
            }
            "Play" | "MuteVolume" => {
                let scan = if assignment == "Play" { 34 } else { 32 };
                multi(vec![
                    json!({"type":"keyboard","scancode":scan,"flag":2}),
                    delay(10),
                    json!({"type":"keyboard","scancode":scan,"flag":3}),
                ])
            }
            _ => bail!("Unresolved multimedia assignment {assignment}"),
        };
        let use_turbo = group
            .get("useTurbo")
            .is_none_or(|v| v.is_null() || truthy(Some(v)));
        if self.is_single(id) && use_turbo {
            if let Some(guid) = self.guid_by_name(assignment, context, true)? {
                return Ok((
                    json!({"type":"turbo","guid":guid,"flag":0,"repeat":1}),
                    json!({}),
                ));
            }
        }
        let mut down = down;
        if down["type"] == "audio" {
            down["repeat"] = json!(1);
        }
        let up = if down["type"] == "keyboard" {
            json!({"type":"keyboard","scancode":down["scancode"],"flag":3})
        } else {
            disabled()
        };
        Ok((down, up))
    }

    fn windows_shortcut_outputs(
        &self,
        record: &Value,
        context: &EngineContext,
    ) -> Result<(Value, Value)> {
        // Current 679 MW2291 ge/at/dt, also present in current 515/190
        // MW2520 and 678/688 MW2291. DKM mouse buttons trigger LockComputer
        // on release; wheel/pod inputs use timed modifier/brightness pulses.
        let assignment = string(&record["win8ShortcutsGroup"], "windowsShortcutAssignment")?;
        ensure!(
            assignment != "MSCopilot",
            "Internet Copilot shortcuts are excluded from local mapping publication"
        );
        let input = string(record, "inputID")?;
        let single = self.is_single(input)
            || matches!(
                record["inputType"].as_str(),
                Some("ControlPodInput" | "CoolingPadInput")
            );
        let lock_on_release = ["DKM_M_01", "DKM_M_02", "DKM_SB_03"].contains(&input);
        let guid = self.guid_by_name(assignment, context, true)?;
        let key =
            |scancode: u32, flag: u32| json!({"type":"keyboard","scancode":scancode,"flag":flag});
        let lock = |ms| {
            multi(vec![
                key(91, 2),
                key(38, 0),
                delay(ms),
                key(38, 1),
                key(91, 3),
            ])
        };
        // JSON.stringify omits an undefined guid in the source global shortcut
        // path. Keep that absence, rather than manufacturing a turbo identity.
        let turbo = || {
            let mut value = json!({"type":"turbo","flag":0,"repeat":1});
            if let Some(guid) = &guid {
                value["guid"] = json!(guid);
            }
            value
        };
        let down = match assignment {
            "SwitchApps" if single => {
                let mut alt = key(56, 0);
                alt["releaseAfterMs"] = json!(1000);
                multi(vec![alt, key(15, 0), key(15, 1)])
            }
            "SwitchApps" => multi(vec![key(56, 0), delay(10), key(15, 0)]),
            "OfficeZoomReset" => multi(vec![key(29, 0), delay(10), key(11, 0)]),
            "DisplayBrightnessUp" | "DisplayBrightnessDown" => {
                let value = json!({"type":"display","id":if assignment=="DisplayBrightnessUp" {"driverBrightnessUp"} else {"driverBrightnessDown"}});
                if single {
                    multi(vec![
                        value,
                        delay(10),
                        json!({"type":"display","id":"driverBrightnessStop"}),
                    ])
                } else {
                    value
                }
            }
            "Mail" | "ThisPC" | "Refresh" => {
                let scancode = match assignment {
                    "Mail" => 108,
                    "ThisPC" => 107,
                    _ => 103,
                };
                multi(vec![key(scancode, 2), delay(10), key(scancode, 3)])
            }
            "SwitchAppRight" | "SwitchAppLeft" => {
                let mut alt = key(56, 0);
                alt["releaseAfterMs"] = json!(750);
                let mut events = vec![alt];
                if assignment == "SwitchAppLeft" {
                    let mut shift = key(42, 0);
                    shift["releaseAfterMs"] = json!(750);
                    events.push(shift);
                }
                events.extend([key(15, 0), key(15, 1)]);
                if assignment == "SwitchAppLeft" {
                    events.push(key(42, 1));
                }
                multi(events)
            }
            "LockComputer" if context.is_global_shortcuts => turbo(),
            "LockComputer" if lock_on_release => disabled(),
            "LockComputer" => lock(10),
            _ if guid.is_some() => turbo(),
            _ => {
                let path = match assignment {
                    "Calculator" => "calc",
                    "MSPaint" => "mspaint",
                    "Notepad" => "notepad",
                    "Snipping_Tool" => "snippingtool",
                    "LaunchTaskManager" => "cmd /c start taskmgr",
                    "User_Directory" => "cmd /c start explorer %USERPROFILE%",
                    "File_Explorer" => "explorer",
                    "PowerUserMenu" => "cmd /c start ms-settings:",
                    _ => "",
                };
                let mut value = json!({"type":"launch","path":path});
                if path.starts_with("cmd") {
                    value["startHidden"] = json!(true);
                }
                value
            }
        };
        let up = match assignment {
            "SwitchApps" => multi(vec![key(15, 1), delay(10), key(56, 1)]),
            "OfficeZoomReset" => multi(vec![key(11, 1), delay(10), key(29, 1)]),
            "LockComputer" if lock_on_release => lock(50),
            "DisplayBrightnessUp" | "DisplayBrightnessDown" => {
                json!({"type":"display","id":"driverBrightnessStop"})
            }
            _ => disabled(),
        };
        Ok((down, up))
    }

    fn outputs(&self, r: &Value, c: &mut EngineContext, depth: u32) -> Result<(Value, Value)> {
        ensure!(depth < 8, "Recursive output mapping");
        let output = string(r, "outputType")?;
        match output {
            "keyboardGroup" => self.keyboard_outputs(r, c),
            "mouseGroup" => self.mouse_outputs(r, c),
            "multimediaGroup" => self.media_outputs(r, c),
            "win8ShortcutsGroup" => self.windows_shortcut_outputs(r, c),
            "disableGroup" => Ok((disabled(), disabled())),
            "systemGroup" => {
                // Current 515 MW2520 module 7476 System_default: only release
                // performs the local OS command; unknown assignments keep type.
                let group = r.get(output).context("Missing systemGroup")?;
                let mut up = json!({"type":"system"});
                match group["systemAssignment"].as_str() {
                    Some("Sleep") => up["id"] = json!("sleep"),
                    Some("Hibernate") => up["id"] = json!("hibernate"),
                    _ => {}
                }
                Ok((disabled(), up))
            }
            "modTapGroup" => {
                let group = r.get("modTapGroup").context("Missing modTapGroup")?;
                let tap = group.get("tapKeyId").context("Missing tap key")?;
                let hold = if let Some(output) = group.get("holdOutputType").and_then(Value::as_str)
                {
                    let mut inner = r.clone();
                    inner["outputType"] = json!(output);
                    self.outputs(&inner, c, depth + 1)?
                } else {
                    let mut inner = r.clone();
                    inner["inputType"] = json!("KeyInput");
                    let (d, u) = self.inputs(&inner, c)?;
                    (
                        d.into_iter().next().context("Missing hold input")?,
                        u.context("Missing hold release")?,
                    )
                };
                Ok((
                    json!({"type":"modtap","ms":50,"tap":self.key_event(tap,false)?,"hold":hold.0}),
                    json!({"type":"modtap","ms":50,"tap":self.key_event(tap,true)?,"hold":hold.1}),
                ))
            }
            "macroGroup" => {
                let group = r.get("macroGroup").context("Missing macroGroup")?;
                let mode = string(group, "macroPlaybackOption")?;
                let guid = group.get("guid").context("Missing macro guid")?;
                let flag = match mode {
                    "ContinuousToggle" => 2,
                    "Sequence" => 3,
                    "Queue" => 4,
                    "PhasedMacro" => 5,
                    _ => 0,
                };
                let phased = mode == "Phased";
                let mut down = json!({"type":if phased{"phasedMacro"}else{"macro"},"guid":guid,"flag":if phased{0}else{flag},"toggle":if phased{0}else{flag}});
                if mode == "Once" {
                    down["repeat"] = json!(1);
                } else if mode == "NTimes" {
                    copy_field(&mut down, "repeat", group, "repeatCount");
                }
                let up = if phased {
                    json!({"type":"phasedMacro","guid":guid,"flag":1})
                } else if mode == "ContinuousHeld" {
                    json!({"type":"macro","guid":guid,"flag":1})
                } else {
                    disabled()
                };
                Ok((down, up))
            }
            "lightPacGroup" => {
                let group = r.get(output).context("Missing lightPacGroup")?;
                let mut down = json!({"type":"lightpac","id":group.get("ID").context("Missing Chroma profile ID")?});
                if truthy(group.get("chromaEffectAssignment")) {
                    copy_field(
                        &mut down,
                        "chromaEffectAssignment",
                        group,
                        "chromaEffectAssignment",
                    );
                }
                Ok((down, disabled()))
            }
            "interDeviceGroup" => {
                // Current70 MW2520 factory91614, both source function bodies.
                let group = r.get(output).context("Missing interDeviceGroup")?;
                let kind = string(group, "interDeviceType")?;
                let value = match kind {
                    "profileNavigationGroup" => {
                        let navigation = &group[kind];
                        if navigation["profileNavigationAssignment"] == "Specific" {
                            let mut value =
                                json!({"type":"switchProfile","guid":navigation["guid"]});
                            copy_field(&mut value, "deviceContainerId", group, "deviceContainerId");
                            if truthy(navigation.get("lightpacId")) {
                                copy_field(&mut value, "lightpacId", navigation, "lightpacId");
                            }
                            value
                        } else {
                            let mut value = json!({"type":"navigateProfile","name":navigation["profileNavigationAssignment"]});
                            copy_field(&mut value, "deviceContainerId", group, "deviceContainerId");
                            value
                        }
                    }
                    "keymapGroup" => json!({"type":"switchKeymap","id":group[kind]["type"]}),
                    "sensitivityGroup" => {
                        json!({"type":"sensitivity","sensitivityGroup":group[kind],"flag":0})
                    }
                    "fanSpeedModeGroup" => json!({"type":"switchFanSpeedMode","flag":0}),
                    _ => bail!("Unknown inter-device assignment {kind}"),
                };
                let down = json!({"type":"interdevice","pid":group["interPID"],"value":value});
                let up = if kind == "sensitivityGroup"
                    && matches!(
                        group[kind]["sensitivityAssignment"].as_str(),
                        Some("DPI_Clutch" | "DPI_OnTheFly")
                    ) {
                    json!({"type":"interdevice","pid":group["interPID"],"value":{"type":"sensitivity","sensitivityGroup":group[kind],"flag":1}})
                } else {
                    disabled()
                };
                Ok((down, up))
            }
            "profileNavigationGroup" => {
                let g = r.get(output).context("Missing profile navigation group")?;
                let assignment = string(g, "profileNavigationAssignment")?;
                let up = if assignment == "Specific" {
                    let mut up = json!({"type":"switchProfile"});
                    copy_field(&mut up, "guid", g, "guid");
                    if truthy(g.get("lightpacId")) {
                        copy_field(&mut up, "lightpacId", g, "lightpacId");
                    }
                    up
                } else {
                    json!({"type":"navigateProfile","name":assignment})
                };
                Ok((json!({"type":"disable"}), up))
            }
            "keymapGroup" => {
                let g = r.get(output).context("Missing keymap group")?;
                let kind = string(g, "type")?;
                let mut down = json!({"type":"switchKeymap","id":kind});
                if kind == "specificKeymap" {
                    copy_field(&mut down, "guid", g, "guid");
                }
                let up = if truthy(g.get("isClutch")) {
                    let mut up = json!({"type":"switchKeymap"});
                    copy_field(&mut up, "guid", g, "previousKeymapGuid");
                    up
                } else {
                    disabled()
                };
                Ok((down, up))
            }
            "launchGroup" => {
                let g = r.get(output).context("Missing launchGroup")?;
                let path = g.get("path").and_then(Value::as_str).unwrap_or("");
                ensure!(
                    !path.is_empty(),
                    "Internet website launch assignments are excluded from local mapping publication"
                );
                let exe = path
                    .rsplit('.')
                    .next()
                    .is_some_and(|v| v.eq_ignore_ascii_case("exe"));
                let mut down = json!({"type":"launch","path":if exe{path.to_owned()}else{format!("cmd /c \"{path}\"")},"startHidden":!exe});
                if !path.is_empty() {
                    down["pathToCheck"] = json!(path);
                }
                Ok((down, disabled()))
            }
            "textBlockGroup" => {
                let g = r.get(output).context("Missing text group")?;
                let mut down = json!({"type":"clipboard","id":"text"});
                copy_field(&mut down, "text", g, "text");
                Ok((down, disabled()))
            }
            "displayGroup" => {
                let assignment = string(&r[output], "displayAssignment")?;
                let guid = self
                    .guid_by_name(assignment, c, true)?
                    .context("Display default turbo not observed")?;
                Ok((
                    json!({"type":"turbo","guid":guid,"flag":0,"delay":100}),
                    json!({"type":"turbo","guid":guid,"flag":1}),
                ))
            }
            "sensitivityGroup" => {
                let g = r.get(output).context("Missing sensitivity group")?;
                let assignment = string(g, "sensitivityAssignment")?;
                if assignment == "DPI_OnTheFly" {
                    return Ok((
                        json!({"type":"otfs","flag":0}),
                        json!({"type":"otfs","flag":1}),
                    ));
                }
                if assignment == "OTFS_Scroll" {
                    return Ok((
                        json!({"type":"sensitivity","sensitivityGroup":{"sensitivityAssignment":if r["inputID"]=="ScrollUp"{"DPI_Value_Up"}else{"DPI_Value_Down"}},"flag":0}),
                        json!({}),
                    ));
                }
                Ok((
                    json!({"type":"sensitivity","sensitivityGroup":g,"flag":0}),
                    if assignment == "DPI_Clutch" {
                        json!({"type":"sensitivity","sensitivityGroup":g,"flag":1})
                    } else {
                        disabled()
                    },
                ))
            }
            "joystickGroup" | "joystickAnalogGroup" => {
                self.joystick_outputs(r, c, output == "joystickAnalogGroup")
            }
            "scrollingGroup" => {
                let group = r.get("scrollingGroup").context("Missing scrolling group")?;
                Ok((
                    json!({"type":"scrolling","scrollingGroup":group,"flag":0}),
                    json!({"type":"scrolling","scrollingGroup":group,"flag":1}),
                ))
            }
            "doubleMappingGroup" => self.double_mapping_outputs(r, c, depth),
            "controllerGroup" => self.controller_outputs(r, c),
            "aiLauncherGroup" => {
                let g = r.get(output).context("Missing AI launcher group")?;
                let mut ai = json!({});
                copy_field(&mut ai, "assignment", g, "assignment");
                copy_field(&mut ai, "param", g, "param");
                Ok((
                    json!({"type":"copyText","data":{"type":"aiLauncherGroup","aiLauncher":ai},"copiedText":""}),
                    disabled(),
                ))
            }
            _ => self.simple_outputs(r, output),
        }
    }

    fn simple_outputs(&self, r: &Value, output: &str) -> Result<(Value, Value)> {
        let once = match output {
            "specificQuickEffect" => Some("specificQuickEffect"),
            "offLightingGroup" => Some("offLightingGroup"),
            "onLightingGroup" => Some("onLightingGroup"),
            "otfmGroup" => Some("otfm"),
            "outputDeviceGroup" => Some("outputDeviceGroup"),
            "inputDeviceGroup" => Some("inputDeviceGroup"),
            "layoutSwitchGroup" => Some("layoutSwitch"),
            "functionKeyPrimaryGroup" => Some("functionKeyPrimary"),
            "gameModeGroup" => Some("gameMode"),
            "switchCommandDial" => Some("commandDial"),
            _ => None,
        };
        if let Some(kind) = once {
            return Ok((json!({"type":kind}), disabled()));
        }
        let paired = match output {
            "hyperShiftGroup" => Some(("hypershift", None)),
            "switchQuickEffect" => Some(("switchQuickEffect", None)),
            "switchFanSpeedMode" => Some(("switchFanSpeedMode", None)),
            "switchPowerState" => Some(("switchPowerState", Some("powerState"))),
            "switchFanState" => Some(("switchFanState", Some("fanState"))),
            "switchExternalPowerState" => {
                Some(("switchExternalPowerState", Some("externalPowerState")))
            }
            "switchChromaState" => Some(("switchChromaState", Some("chromaState"))),
            _ => None,
        };
        if let Some((kind, state)) = paired {
            let mut d = json!({"type":kind,"flag":0});
            let mut u = json!({"type":kind,"flag":1});
            if let Some(state) = state {
                copy_field(&mut d, "state", r, state);
                copy_field(&mut u, "state", r, state);
            }
            return Ok((d, u));
        }
        if ["scrollingGroup", "brightnessGlobalGroup"].contains(&output) {
            let g = r.get(output).context("Missing copied group")?;
            let kind = if output == "scrollingGroup" {
                "scrolling"
            } else {
                "brightnessGlobal"
            };
            let mut d = json!({"type":kind,"flag":0});
            let mut u = json!({"type":kind,"flag":1});
            d[output] = g.clone();
            u[output] = g.clone();
            return Ok((d, u));
        }
        let named = match output {
            "backlightGroup" => Some(("backlight", "backlightAssignment")),
            "oledLightGroup" => Some(("oledLight", "oledLightAssignment")),
            _ => None,
        };
        if let Some((kind, key)) = named {
            let mut d = json!({"type":kind,"flag":0});
            let mut u = json!({"type":kind,"flag":1});
            copy_field(&mut d, "name", &r[output], key);
            copy_field(&mut u, "name", &r[output], key);
            return Ok((d, u));
        }
        let on_release = match output {
            "screenRefreshGroup" => Some(("screenRefresh", output, "screenRefreshAssignment")),
            "snapTapGroup" => Some(("snapTap", output, "snapTapAssignment")),
            "bladePerformanceGroup" => {
                Some(("bladePerformance", output, "performanceModeAssignment"))
            }
            "bladeTrackpadGroup" => Some(("bladeTrackpad", "trackpadGroup", "trackpadAssignment")),
            "bladeVCLedStateGroup" => {
                Some(("bladeVCLedState", output, "bladeVCLedStateAssignment"))
            }
            "bladeBatteryGroup" => Some(("bladeBattery", output, "assignment")),
            _ => None,
        };
        if let Some((kind, group, key)) = on_release {
            let mut up = json!({"type":kind});
            copy_field(&mut up, "id", &r[group], key);
            return Ok((json!({"type":"disable"}), up));
        }
        bail!("Output generator not yet implemented: {output}")
    }

    fn double_mapping_outputs(
        &self,
        record: &Value,
        context: &mut EngineContext,
        depth: u32,
    ) -> Result<(Value, Value)> {
        let triggers = record
            .get("doubleMappingGroup")
            .and_then(|v| v.get("doubleMappingGroup"))
            .and_then(|v| v.get("triggers"))
            .and_then(Value::as_array)
            .context("Missing double-mapping triggers")?;
        ensure!(!triggers.is_empty(), "Double-mapping trigger list is empty");

        // The current factory has a fast path for exactly two digital
        // triggers. It produces a compact triggers object keyed by 1 and 2.
        let digital_only = triggers.len() == 2
            && triggers.iter().all(|trigger| {
                matches!(
                    trigger["outputType"].as_str(),
                    Some("keyboardGroup" | "mouseGroup")
                ) || (trigger["outputType"] == "controllerGroup"
                    && !truthy(
                        trigger["controllerGroup"]
                            .get("controllerMode")
                            .and_then(|v| v.get("isAnalog")),
                    ))
                    || (trigger["outputType"] == "joystickGroup"
                        && trigger["joystickGroup"]["joystickMode"] == "button")
            });
        if digital_only {
            let mut down = serde_json::Map::new();
            let mut up = serde_json::Map::new();
            for (index, trigger) in triggers.iter().enumerate() {
                let mut input = trigger.clone();
                for key in ["inputID", "inputType", "isHyperShift", "analogKeyID"] {
                    copy_field(&mut input, key, record, key);
                }
                let (down_output, up_output) = self.outputs(&input, context, depth + 1)?;
                let actuation = actuation_point(trigger, 0)?;
                let key = (index + 1).to_string();
                down.insert(
                    key.clone(),
                    json!({"actuation":actuation,"mapping":down_output}),
                );
                up.insert(key, json!({"actuation":actuation,"mapping":up_output}));
            }
            return Ok((
                json!({"type":"triggers","outputs":down}),
                json!({"type":"triggers","outputs":up}),
            ));
        }

        let mut down_digital = serde_json::Map::new();
        let mut up_digital = serde_json::Map::new();
        let mut analog_down = Value::Object(serde_json::Map::new());
        let mut analog_up = Value::Object(serde_json::Map::new());
        let mut trigger_index = 1u32;
        for trigger in triggers {
            let mut input = trigger.clone();
            for key in ["inputID", "inputType", "isHyperShift", "analogKeyID"] {
                copy_field(&mut input, key, record, key);
            }
            let (down_output, up_output) = self.outputs(&input, context, depth + 1)?;
            let output_type = trigger["outputType"].as_str().unwrap_or_default();
            let analog = (output_type == "controllerGroup"
                && truthy(
                    trigger["controllerGroup"]
                        .get("controllerMode")
                        .and_then(|v| v.get("isAnalog")),
                ))
                || (output_type == "joystickGroup"
                    && matches!(
                        trigger["joystickGroup"]["joystickMode"].as_str(),
                        Some("axes" | "raxes")
                    ));
            if analog {
                analog_down = down_output;
                analog_up = up_output;
            } else {
                let actuation = actuation_point(trigger, 0)?;
                let key = trigger_index.to_string();
                down_digital.insert(
                    key.clone(),
                    json!({"actuation":actuation,"mapping":down_output}),
                );
                up_digital.insert(key, json!({"actuation":actuation,"mapping":up_output}));
                trigger_index += 1;
            }
        }
        let down = json!({"type":"doubleMapping","analog":analog_down,
            "digital":{"type":"triggers","outputs":down_digital}});
        let up = json!({"type":"doubleMapping","analog":analog_up,
            "digital":{"type":"triggers","outputs":up_digital}});
        Ok((down, up))
    }

    fn joystick_outputs(
        &self,
        r: &Value,
        c: &EngineContext,
        analog: bool,
    ) -> Result<(Value, Value)> {
        let g = r.get("joystickGroup").context("Missing joystickGroup")?;
        let mode = string(g, "joystickMode")?;
        if mode == "button" {
            let button = js_number_value(js_numeric(
                g.get("joystickButtonAssignment")
                    .context("Missing joystick button")?,
            )?);
            return Ok((
                json!({"type":"joystick","id":mode,"button":button,"flag":0}),
                json!({"type":"joystick","id":mode,"button":button,"flag":1}),
            ));
        }
        ensure!(
            ["axes", "raxes"].contains(&mode),
            "Unresolved joystick mode"
        );
        let axes = g
            .get("joystickAxisAssignment")
            .context("Missing joystick axes")?;
        let mut xyz = Vec::new();
        for axis in ["X", "Y", "Z"] {
            xyz.push(js_number_value(
                axes.get(axis)
                    .map(js_numeric)
                    .transpose()?
                    .unwrap_or(0.0)
                    .clamp(-128.0, 127.0),
            ));
        }
        if analog {
            let mut a = json!({"0":[0,0,0]});
            let end = if c.is_analog_gen2 {
                c.analog_binary_range
                    .context("Observed analog binaryRange required")?
            } else {
                255
            };
            a[end.to_string()] = json!(xyz);
            let o = json!({"type":if c.is_analog_gen2{"analogJoystickV2"}else{"analogJoystick"},"id":mode,"actuations":a});
            Ok((o.clone(), o))
        } else {
            Ok((
                json!({"type":"joystick","id":mode,"xyz":xyz}),
                json!({"type":"joystick","id":mode,"xyz":[0,0,0]}),
            ))
        }
    }

    fn controller_outputs(&self, r: &Value, c: &EngineContext) -> Result<(Value, Value)> {
        let g = r
            .get("controllerGroup")
            .context("Missing controllerGroup")?;
        let assignment = string(g, "controllerAssignment")?;
        if truthy(g.get("controllerMode").and_then(|v| v.get("isAnalog"))) {
            let id = match assignment {
                "LEFTTRIGGER" => "leftTrigger",
                "RIGHTTRIGGER" => "rightTrigger",
                "LEFT_JS_LEFT" | "LEFT_JS_RIGHT" => "leftThumbX",
                "LEFT_JS_UP" | "LEFT_JS_DOWN" => "leftThumbY",
                "RIGHT_JS_LEFT" | "RIGHT_JS_RIGHT" => "rightThumbX",
                "RIGHT_JS_UP" | "RIGHT_JS_DOWN" => "rightThumbY",
                _ => bail!("Unresolved analog controller assignment"),
            };
            let mut actuations = json!({});
            let list = g["analogSensitivityList"]["analogSensitivityAssignment"]
                .as_array()
                .context("Missing observed controller curve")?;
            for p in list {
                let x = p["x"].as_f64().context("Invalid curve x")?;
                let y = p["y"].as_f64().context("Invalid curve y")?;
                let key = if c.is_analog_gen2 {
                    (1638.0 * x / 0.1).floor()
                } else {
                    (12.14 * (x - 1.5) / 0.1 + 0.5).floor()
                };
                let output = if assignment.contains("JS") {
                    (y * if assignment.ends_with("RIGHT") || assignment.ends_with("UP") {
                        32767.0
                    } else {
                        -32768.0
                    } / 255.0)
                        .floor()
                } else {
                    y
                };
                actuations[format!(
                    "{}",
                    (key + if c.is_analog_gen2 { 0.5 } else { 0.0 }).floor()
                )] = json!(output);
            }
            let output = json!({"type":if c.is_analog_gen2{"analogXboxGamepadV2"}else{"analogXboxGamepad"},"id":id,"actuations":actuations});
            return Ok((output.clone(), output));
        }
        let value = match assignment {
            "DPADUP" => 1,
            "DPADDOWN" => 2,
            "DPADLEFT" => 4,
            "DPADRIGHT" => 8,
            "MENUBUTTON" => 16,
            "VIEWBUTTON" => 32,
            "LEFT_JS_PRESS" => 64,
            "RIGHT_JS_PRESS" => 128,
            "LEFTBUMPER" => 256,
            "RIGHTBUMPER" => 512,
            "A_BUTTON" => 4096,
            "B_BUTTON" => 8192,
            "X_BUTTON" => 16384,
            "Y_BUTTON" => 32768,
            _ => bail!("Unresolved digital controller assignment"),
        };
        Ok((
            json!({"type":"xboxGamepad","id":"button","value":value,"flag":0}),
            json!({"type":"xboxGamepad","id":"button","value":value,"flag":1}),
        ))
    }

    /// Exact source MapActuation helper; wrapped v1 clamps only the first stage.
    pub fn actuation(&self, r: &Value) -> Result<Value> {
        let key = r
            .get("analogKeyID")
            .cloned()
            .or_else(|| {
                self.key(&r["inputID"])
                    .ok()
                    .map(|key| json!(key.analog_key_id))
            })
            .context("Missing analog key id")?;
        let wrapped = r.get("mapping").and_then(Value::as_array);
        let first = wrapped.and_then(|v| v.first()).unwrap_or(r);
        let clamp = wrapped.is_some() && r["analogGeneration"] == "analogV1";
        let mut make = actuation_point(first, 0)?;
        let mut release = actuation_point(first, 1)?;
        if clamp {
            make = json!(make.as_f64().context("Invalid make")?.max(16.0));
            release = json!(release.as_f64().context("Invalid break")?.max(16.0));
        }
        let mut act = json!({"keyId":key,"make":make,"break":release});
        if truthy(r.get("isHyperShift"))
            && wrapped.is_none_or(|v| v.get(1).is_none_or(|v| !truthy(v.get("keyboardGroup"))))
        {
            copy_field(&mut act, "hypershift", r, "isHyperShift");
        }
        Ok(act)
    }

    pub fn rapid_trigger(&self, r: &Value, c: &EngineContext) -> Result<Value> {
        let key = r
            .get("analogKeyID")
            .cloned()
            .or_else(|| {
                self.key(&r["inputID"])
                    .ok()
                    .map(|key| json!(key.analog_key_id))
            })
            .context("Missing rapid-trigger key id")?;
        let rt = r["mapping"]
            .get(0)
            .and_then(|v| v.get("rapidTrigger"))
            .context("Missing rapid trigger")?;
        let factor = if c.is_analog_gen2 { 1628.0 } else { 12.0 };
        let make = rt["makeSensitivity"]
            .as_f64()
            .context("Missing rapid trigger make")?
            * factor;
        let release = if truthy(rt.get("isEnableUpStroke")) {
            rt["breakSensitivity"]
                .as_f64()
                .context("Missing rapid trigger break")?
                * factor
        } else {
            make
        };
        let mut result = json!({});
        result[key.as_u64().context("Invalid analog key id")?.to_string()] =
            json!({"makeSensitivity":make,"breakSensitivity":release});
        Ok(result)
    }

    // Current MW2291 De: the dual-key factory is also used for a keyboard
    // first stage followed by multimedia/macro/system actions. It differs
    // from the doubleMapping factory's digital/analog grouping.
    fn dual_key_outputs(
        &self,
        record: &Value,
        context: &mut EngineContext,
    ) -> Result<(Value, Value)> {
        let stages = record["mapping"]
            .as_array()
            .context("Missing dual-key stages")?;
        let mut first = stages[0].clone();
        first["inputID"] = record["inputID"].clone();
        if first["keyboardGroup"]
            .as_object()
            .is_none_or(|group| group.is_empty())
        {
            first["keyboardGroup"] = json!({"key":record["inputID"],"keyGroup":0,"modifiers":[]});
        }
        let (down, up) = self.outputs(&first, context, 0)?;
        let mut second = stages[1].clone();
        if second["outputType"] == "defaultGroup" {
            return Ok((down, up));
        }
        second["inputID"] = record["inputID"].clone();
        let (second_down, second_up) = if let Some(modifiers) = second["keyboardGroup"]["modifiers"]
            .as_array()
            .filter(|modifiers| !modifiers.is_empty())
        {
            let mut make = modifiers
                .iter()
                .map(|key| self.key_event(key, false))
                .collect::<Result<Vec<_>>>()?;
            make.push(self.key_event(&second["keyboardGroup"]["key"], false)?);
            let mut release = vec![self.key_event(&second["keyboardGroup"]["key"], true)?];
            release.extend(
                modifiers
                    .iter()
                    .rev()
                    .map(|key| self.key_event(key, true))
                    .collect::<Result<Vec<_>>>()?,
            );
            (multi(make), multi(release))
        } else {
            self.outputs(&second, context, 0)?
        };
        let second_pair = if down["type"] == "multi" && second_down["type"] != "multi" {
            (multi(vec![second_down]), multi(vec![second_up]))
        } else {
            (second_down, second_up)
        };
        let mut make = json!({"type":"triggers","outputs":{
            "1":{"actuation":actuation_point(&first,0)?,"mapping":down},
            "2":{"actuation":actuation_point(&second,0)?,"mapping":second_pair.0}}});
        let release = json!({"type":"triggers","outputs":{
            "1":{"actuation":actuation_point(&first,0)?,"mapping":up},
            "2":{"actuation":actuation_point(&second,0)?,"mapping":second_pair.1}}});
        if truthy(down.get("delayInMs")) {
            make["delayInMs"] = down["delayInMs"].clone();
        }
        Ok((make, release))
    }

    /// Current MW2291 generateAnalogMappings / kt / bt / De. Retains each
    /// mapping array, actuation-only branch, dual-key trigger and rapid trigger.
    pub fn generate_analog(
        &self,
        records: &[Value],
        context: &mut EngineContext,
    ) -> Result<GeneratedEngine> {
        let mut c = context.clone();
        let generation = c.analog_generation.clone();
        let mut mappings = Vec::new();
        let mut actuations = Vec::new();
        let mut rapid = json!({});
        for record in records {
            let mut r = record.clone();
            r["isAnalogGen2"] = json!(c.is_analog_gen2);
            if let Some(generation) = &generation {
                r["analogGeneration"] = json!(generation);
            } else {
                r.as_object_mut()
                    .context("Analog mapping is not an object")?
                    .remove("analogGeneration");
            }
            let stages = r
                .get("mapping")
                .and_then(Value::as_array)
                .context("Missing analog stages")?;
            ensure!(stages.len() == 2, "Expected two source analog stages");
            let first = stages[0].clone();
            let second = stages[1].clone();
            let empty_keyboard = first["outputType"] == "keyboardGroup"
                && first["keyboardGroup"]
                    .as_object()
                    .is_some_and(|v| v.is_empty());
            let second_is_default = second["outputType"] == "defaultGroup";
            let second_analog = (second["outputType"] == "controllerGroup"
                && second["controllerGroup"]["controllerMode"]["isAnalog"] == true)
                || (second["outputType"] == "joystickGroup"
                    && matches!(
                        second["joystickGroup"]["joystickMode"].as_str(),
                        Some("axes" | "raxes")
                    ));
            let first_keyboard = first["outputType"] == "keyboardGroup";
            let double = if first_keyboard {
                matches!(
                    second["outputType"].as_str(),
                    Some("keyboardGroup" | "mouseGroup")
                ) || second_analog
            } else {
                !second_is_default
            };
            if first_keyboard && !double {
                if !second
                    .get("keyboardGroup")
                    .is_some_and(|group| truthy(Some(group)))
                {
                    actuations.push(self.actuation(&r)?);
                }
                if truthy(first.get("rapidTrigger")) {
                    let rt = self.rapid_trigger(&r, &c)?;
                    rapid
                        .as_object_mut()
                        .unwrap()
                        .extend(rt.as_object().unwrap().clone());
                }
                if !empty_keyboard {
                    let (downs, up) = self.inputs(&r, &c)?;
                    let (down_output, up_output) = self.dual_key_outputs(&r, &mut c)?;
                    let mut records = downs
                        .into_iter()
                        .map(|input| json!({"input":input,"output":down_output}))
                        .collect::<Vec<_>>();
                    if let Some(input) = up {
                        records.push(json!({"input":input,"output":up_output}));
                    }
                    mappings.push(json!(records));
                }
                continue;
            }
            // kt receives the first stage for ordinary/double non-keyboard
            // mappings, the wrapper for two keyboard stages, or the second
            // analog stage when its first keyboard stage is empty.
            let direct_second = first_keyboard && second_analog && empty_keyboard;
            let mut input = if direct_second {
                second.clone()
            } else if first_keyboard {
                r.clone()
            } else {
                first.clone()
            };
            for key in [
                "inputID",
                "inputType",
                "isHyperShift",
                "analogKeyID",
                "interrupts",
            ] {
                copy_field(&mut input, key, &r, key);
            }
            let no_actuation = truthy(
                input
                    .get("controllerGroup")
                    .and_then(|v| v.get("controllerMode"))
                    .and_then(|v| v.get("isAnalog")),
            ) || truthy(
                input
                    .get("joystickGroup")
                    .and_then(|v| v.get("isJoystickMovement")),
            );
            if input.get("actuationPoint").is_some() && !no_actuation {
                actuations.push(self.actuation(&input)?);
            }
            if truthy(input.get("rapidTrigger")) {
                let rt = self.rapid_trigger(&r, &c)?;
                rapid
                    .as_object_mut()
                    .unwrap()
                    .extend(rt.as_object().unwrap().clone());
            }
            let (downs, up) = self.inputs(&input, &c)?;
            let output = if double && !direct_second {
                let mut wrapper = r.clone();
                wrapper["outputType"] = json!("doubleMappingGroup");
                wrapper["doubleMappingGroup"] =
                    json!({"doubleMappingGroup":{"triggers":[first, second]}});
                wrapper
            } else {
                if c.is_analog_gen2 && input["outputType"] == "joystickGroup" {
                    input["outputType"] = json!("joystickAnalogGroup");
                }
                input
            };
            let (down_output, up_output) = self.outputs(&output, &mut c, 0)?;
            let mut records = downs
                .into_iter()
                .map(|input| json!({"input":input,"output":down_output}))
                .collect::<Vec<_>>();
            if let Some(input) = up {
                records.push(json!({"input":input,"output":up_output}));
            }
            mappings.push(json!(records));
        }
        let engine = json!({"mappings":mappings,"actuations":actuations,"rapidTriggers":rapid});
        let result = GeneratedEngine {
            app_engine: engine,
            synapse_turbos: c.synapse_turbos.clone().unwrap_or_default(),
        };
        *context = c;
        Ok(result)
    }

    /// Source multi mapping groups only entries with input.key, numeric keys first.
    pub fn generate_multi(
        &self,
        records: &[Value],
        context: &mut EngineContext,
    ) -> Result<GeneratedEngine> {
        let mut c = context.clone();
        let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
        for r in records {
            for entry in self.record(r, &mut c)? {
                let Some(key) = entry["input"].get("key") else {
                    continue;
                };
                let key = key
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| key.to_string());
                let children = if entry["output"]["type"] == "multi" {
                    entry["output"]["outputs"]
                        .as_array()
                        .context("Invalid multi output")?
                        .clone()
                } else {
                    vec![entry["output"].clone()]
                };
                let index = groups
                    .iter()
                    .position(|(k, _)| k == &key)
                    .unwrap_or_else(|| {
                        groups.push((key.clone(), Vec::new()));
                        groups.len() - 1
                    });
                for output in children {
                    groups[index]
                        .1
                        .push(json!({"input":entry["input"],"output":output}));
                }
            }
        }
        groups.sort_by(|(a, _), (b, _)| match (array_index(a), array_index(b)) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        });
        let mut mappings = Vec::new();
        for (_, entries) in groups {
            for flag in [0, 1] {
                let filtered = entries
                    .iter()
                    .filter(|e| {
                        e["input"]["flag"] == flag
                            && (flag != 0 || e["output"]["type"] != "disabled")
                    })
                    .collect::<Vec<_>>();
                if let Some(first) = filtered.first() {
                    mappings.push(json!({"input":first["input"],"output":multi(filtered.iter().map(|e|e["output"].clone()).collect())}));
                }
            }
        }
        let mut engine = json!({"mappings":mappings});
        engine["hash"] = json!(source_hash(&engine)?);
        let result = GeneratedEngine {
            app_engine: engine,
            synapse_turbos: c.synapse_turbos.clone().unwrap_or_default(),
        };
        *context = c;
        Ok(result)
    }
}

fn array_index(s: &str) -> Option<u32> {
    let n = s.parse::<u32>().ok()?;
    (n != u32::MAX && n.to_string() == s).then_some(n)
}
fn js_numeric(v: &Value) -> Result<f64> {
    let n = match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            if s.trim().is_empty() {
                Some(0.0)
            } else {
                s.parse().ok()
            }
        }
        Value::Null => Some(0.0),
        Value::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
        _ => None,
    }
    .context("Unsupported JS numeric input")?;
    ensure!(n.is_finite(), "Non-finite numeric input");
    Ok(n)
}

/// JS JSON.stringify emits exact whole numbers without a decimal point.
/// The native joystick parser requires INTEGER rather than DOUBLE values.
/// Preserve fractional values so the native boundary can reject them; do not
/// silently truncate source data to make an unsupported record appear valid.
fn js_number_value(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

#[cfg(test)]
#[path = "mapping_engine_tests.rs"]
mod tests;
