//! Pure current-source app-engine generation for products 190/678/679/688.
//! No persistence, native publication or device acknowledgment occurs here.
//! Unknown/unimplemented branches fail explicitly; they are never successful saves.
use crate::mapping_engine_hash::source_hash;
use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::OnceLock,
};

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
    pub analog_generation: Option<String>,
    pub analog_binary_range: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedEngine {
    pub app_engine: Value,
    pub synapse_turbos: Vec<Value>,
}

pub struct MappingEngine {
    product_id: u32,
}

fn decoded_data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../assets/data/mapping-engine-current-data.json"
        ))
        .expect("validated source literal data")
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
    pub fn new(product_id: u32) -> Result<Self> {
        ensure!(
            [190, 678, 679, 688].contains(&product_id),
            "Active factory not audited for product {product_id}"
        );
        Ok(Self { product_id })
    }

    fn data(&self) -> &Value {
        &decoded_data()[self.product_id.to_string()]
    }

    fn key(&self, id: &Value) -> Result<&Value> {
        let id = id
            .as_str()
            .or_else(|| id.get("inputID").and_then(Value::as_str))
            .context("Missing source key identifier")?;
        self.data()["keys"]
            .as_array()
            .context("Invalid key table")?
            .iter()
            .find(|k| k["inputID"] == id)
            .with_context(|| format!("Unresolved source key {id}"))
    }

    fn key_event(&self, id: &Value, up: bool) -> Result<Value> {
        let key = self.key(id)?;
        let flag = key
            .get("outputFlag")
            .or_else(|| key.get("flag"))
            .and_then(Value::as_u64)
            .context("Missing source key flag")?;
        Ok(json!({"type":"keyboard","scancode":key["scancode"],"flag":flag+u64::from(up)}))
    }

    fn is_single(&self, id: &str) -> bool {
        self.data()["constants"]["SINGLE_STATE_INPUT_ID"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == id))
    }

    fn is_scroll(&self, id: &str) -> bool {
        ["ScrollUp", "ScrollDown", "ScrollLeft", "ScrollRight"].contains(&id)
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

    /// Actual default event data, with caller-provided UUIDs. No storage write.
    pub fn generate_default_turbos(&self, context: &mut EngineContext) -> Result<Vec<Value>> {
        let mut draft = context.clone();
        let mut turbos = Vec::new();
        for id in self.data()["default_turbo_ids"]
            .as_array()
            .context("Invalid default turbo list")?
        {
            let name = id.as_str().context("Invalid default turbo identifier")?;
            let events = self.data()["turbo_events"]
                .get(name)
                .context("Unresolved default turbo event")?;
            let mut app_engine = json!({"events":events});
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
                mask |= match m.as_str().unwrap_or("") {
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
                    _ => 0,
                };
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
                            if key["type"] == "razerKey" {
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
        let (down_output, up_output) = self.outputs(record, context, 0)?;
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
            names.push(string(self.key(modifier)?, "name")?.to_owned());
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
        lookup_names.push(string(self.key(key)?, "name")?.to_owned());
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
                    events.push(delay(30));
                }
                events.push(key_up);
                mods_up.reverse();
                events.extend(mods_up);
                // ae mutates the temporary key list via reverse before naming.
                if !mods.is_empty() {
                    names.reverse();
                }
                names.push(string(self.key(key)?, "name")?.to_owned());
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
            let rate = group["turboMode"]["keysPerSecond"]
                .as_f64()
                .context("Missing mouse turbo rate")?;
            ensure!(rate > 0.0 && rate.is_finite(), "Invalid mouse turbo rate");
            Some(((1000.0 / rate + 0.5).floor() - 30.0).max(0.0))
        } else {
            None
        };
        if let Some(name) = m.turbo {
            let guid = self
                .guid_by_name(name, context, true)?
                .context("Default turbo not observed")?;
            let mut down = json!({"type":"turbo","guid":guid,"flag":0});
            if let Some(ms) = turbo_delay {
                down["delay"] = json!(ms);
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
                down["delay"] = json!(ms);
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

    fn outputs(&self, r: &Value, c: &mut EngineContext, depth: u32) -> Result<(Value, Value)> {
        ensure!(depth < 8, "Recursive output mapping");
        let output = string(r, "outputType")?;
        match output {
            "keyboardGroup" => self.keyboard_outputs(r, c),
            "mouseGroup" => self.mouse_outputs(r, c),
            "multimediaGroup" => self.media_outputs(r, c),
            "disableGroup" => Ok((disabled(), disabled())),
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
                let exe = path
                    .rsplit('.')
                    .next()
                    .is_some_and(|v| v.eq_ignore_ascii_case("exe"));
                let mut down = json!({"type":"launch","path":if path.is_empty(){format!("cmd /c start \"link\" {}",string(g,"url")?)}else if exe{path.to_owned()}else{format!("cmd /c \"{path}\"")},"startHidden":!exe});
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

    fn joystick_outputs(
        &self,
        r: &Value,
        c: &EngineContext,
        analog: bool,
    ) -> Result<(Value, Value)> {
        let g = r.get("joystickGroup").context("Missing joystickGroup")?;
        let mode = string(g, "joystickMode")?;
        if mode == "button" {
            let button = js_numeric(
                g.get("joystickButtonAssignment")
                    .context("Missing joystick button")?,
            )?;
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
            xyz.push(
                axes.get(axis)
                    .map(js_numeric)
                    .transpose()?
                    .unwrap_or(0.0)
                    .clamp(-128.0, 127.0),
            );
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
            .or_else(|| self.key(&r["inputID"]).ok()?.get("analogKeyID").cloned())
            .context("Missing analog key id")?;
        let wrapped = r.get("mapping").and_then(Value::as_array);
        let first = wrapped.and_then(|v| v.first()).unwrap_or(r);
        let points = first
            .get("actuationPoint")
            .and_then(Value::as_array)
            .context("Missing observed actuation points")?;
        ensure!(points.len() == 2, "Invalid actuation points");
        let clamp = wrapped.is_some() && r["analogGeneration"] == "analogV1";
        let mut make = points[0].clone();
        let mut release = points[1].clone();
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
            .or_else(|| self.key(&r["inputID"]).ok()?.get("analogKeyID").cloned())
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

    /// Source analog orchestrator branches currently established independently.
    /// Complex dual-key/DKS/double-mapping branches fail instead of flattening.
    pub fn generate_analog(
        &self,
        records: &[Value],
        context: &mut EngineContext,
    ) -> Result<GeneratedEngine> {
        let mut c = context.clone();
        let generation = c
            .analog_generation
            .clone()
            .context("Observed analog generation required")?;
        let mut mappings = Vec::new();
        let mut actuations = Vec::new();
        let mut rapid = json!({});
        for record in records {
            let mut r = record.clone();
            r["isAnalogGen2"] = json!(c.is_analog_gen2);
            r["analogGeneration"] = json!(generation);
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
            if empty_keyboard && second["outputType"] == "defaultGroup" {
                actuations.push(self.actuation(&r)?);
                if truthy(first.get("rapidTrigger")) {
                    let rt = self.rapid_trigger(&r, &c)?;
                    rapid
                        .as_object_mut()
                        .unwrap()
                        .extend(rt.as_object().unwrap().clone());
                }
                continue;
            }
            ensure!(
                first["outputType"] != "keyboardGroup" && second["outputType"] == "defaultGroup",
                "Analog dual-key/double-mapping/DKS branch not yet implemented"
            );
            let mut input = first;
            for key in [
                "inputID",
                "inputType",
                "isHyperShift",
                "analogKeyID",
                "interrupts",
            ] {
                copy_field(&mut input, key, &r, key);
            }
            if c.is_analog_gen2 && input["outputType"] == "joystickGroup" {
                input["outputType"] = json!("joystickAnalogGroup");
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
            // kt yields mappingData as an array; generateAnalogMappings pushes it.
            mappings.push(json!(self.record(&input, &mut c)?));
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

#[cfg(test)]
#[path = "mapping_engine_tests.rs"]
mod tests;
