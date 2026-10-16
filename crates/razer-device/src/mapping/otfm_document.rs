//! Ro -> utilities rr/er/or/La. Source-shaped local documents, with the
//! original empty playback option and numeric string delay fields retained.
use super::*;
use crate::{
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
};
use std::collections::{BTreeMap, BTreeSet};

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
        .unwrap_or(0.)
}
fn makecode(vk: u64) -> u64 {
    match vk {
        162 => 17,
        163 => 285,
        160 => 16,
        161 => 310,
        164 => 18,
        165 => 312,
        _ => vk,
    }
}
fn key_event(item: &Value, down: bool, id: u64) -> Result<Value> {
    let keys =
        crate::mapping_engine::source_keys(70).context("Current OTFM key table is absent")?;
    let vk = item["vkCode"].as_u64();
    let flags = item["flags"].as_u64();
    let flag = match flags {
        Some(33 | 1 | 129) => 2,
        _ => 0,
    };
    let key = keys
        .iter()
        .find(|key| {
            let same = key.virtual_key.parse::<u64>().ok() == vk;
            if matches!(flags, Some(0 | 128)) && item["scanCode"] == 69 && vk == Some(19) {
                same
            } else {
                same && item["scanCode"] == key.scancode
                    && key.output_flag.filter(|v| *v != 0).unwrap_or(key.flag) == flag
            }
        })
        .or_else(|| {
            keys.iter()
                .find(|key| key.virtual_key.parse::<u64>().ok() == vk)
        });
    let flag = key
        .map(|key| key.output_flag.filter(|v| *v != 0).unwrap_or(key.flag))
        .unwrap_or(0);
    Ok(
        json!({"Type":1,"Id":id,"KeyEvent":{"Makecode":key.and_then(|key|key.virtual_key.parse::<u64>().ok()).map(makecode),
        "State":flag+u32::from(!down)},"flag":null,"selected":false,"isPairing":false}),
    )
}
fn delay(item: &Value) -> Value {
    json!({"Type":0,"Number":format!("{:.3}",number(&item["time"])/1_000_000.),"selected":false})
}
fn mouse(button: u64, state: Option<u64>, id: u64) -> Value {
    json!({"Type":2,"Id":id,"MouseEvent":{"MouseButton":button,"State":state},"flag":null,"selected":false,"isPairing":false})
}
fn raw_event(item: &Value, previous: &[Value], id: &mut u64, now: u64) -> Result<Vec<Value>> {
    let action = item["action"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| item["action"].as_u64().map(|v| v.to_string()))
        .unwrap_or_default();
    let mut next_id = || {
        *id = now.max((*id).saturating_add(1));
        *id
    };
    let event = match action.as_str() {
        "keydown" => key_event(item, true, next_id())?,
        "keyup" => {
            let code = item["vkCode"].as_u64().map(makecode);
            if let Some(previous) = previous.iter().rev().find(|event| {
                event["KeyEvent"]["Makecode"].as_u64() == code && event["KeyEvent"].is_object()
            }) {
                let mut event = previous.clone();
                event["KeyEvent"]["State"] =
                    json!(event["KeyEvent"]["State"].as_u64().unwrap_or(0) + 1);
                event
            } else {
                key_event(item, false, *id)?
            }
        }
        "lbuttondown" | "rbuttondown" | "mbuttondown" | "523" => {
            let button = match action.as_str() {
                "lbuttondown" => 0,
                "rbuttondown" => 1,
                "mbuttondown" => 2,
                _ => {
                    if item["data"] == 131072 {
                        4
                    } else {
                        3
                    }
                }
            };
            let mut event = mouse(button, Some(0), next_id());
            if action == "mbuttondown" {
                event["flag"] = json!(0);
            }
            event
        }
        "lbuttonup" | "rbuttonup" | "mbuttonup" | "524" => {
            let button = match action.as_str() {
                "lbuttonup" => 0,
                "rbuttonup" => 1,
                "mbuttonup" => 2,
                _ => {
                    if item["data"] == 131072 {
                        4
                    } else {
                        3
                    }
                }
            };
            let mut event = previous
                .iter()
                .rev()
                .find(|event| event["MouseEvent"]["MouseButton"] == button)
                .cloned()
                .unwrap_or_else(|| mouse(button, Some(1), *id));
            event["MouseEvent"]["State"] = json!(1);
            if action == "mbuttonup" {
                event["flag"] = json!(1);
            }
            event
        }
        "mousewheel" | "mousehwheel" => {
            let positive = number(&item["data"]) > 0.;
            let button = if action == "mousewheel" {
                if positive { 8 } else { 9 }
            } else if positive {
                7
            } else {
                6
            };
            let mut event = mouse(button, None, *id);
            event["Id"] = Value::Null;
            event
        }
        "mousemove" => {
            json!({"Type":3,"Id":null,"Number":0,"MouseEvent":{"Buffer":[]},"selected":false})
        }
        _ => return Ok(vec![]),
    };
    Ok(vec![delay(item), event])
}
fn clean_events(events: Vec<Value>) -> Vec<Value> {
    let mut held = BTreeSet::new();
    let mut output: Vec<Value> = vec![];
    for event in events {
        let key = event["KeyEvent"]["Makecode"].to_string();
        if event["Type"] == 1 {
            let down = event["KeyEvent"]["State"]
                .as_u64()
                .is_some_and(|state| state % 2 == 0);
            if down && !held.insert(key.clone()) {
                continue;
            }
            if !down {
                held.remove(&key);
            }
        }
        if event["Type"] == 0 {
            if let Some(previous) = output.last_mut().filter(|previous| previous["Type"] == 0) {
                previous["Number"] = json!(format!(
                    "{:.3}",
                    number(&previous["Number"]) + number(&event["Number"])
                ));
                continue;
            }
        }
        output.push(event);
    }
    output
}
pub fn macro_document(
    machine: &Machine,
    existing: &[Value],
    guid: &str,
    now: u64,
) -> Result<Value> {
    ensure!(
        !machine.geometry_changed(),
        "Monitor setting has been changed"
    );
    let mut items = machine
        .items
        .iter()
        .filter(|item| !(item["scanCode"] == 0 && item["vkCode"] == 255))
        .cloned()
        .collect::<Vec<_>>();
    for item in &mut items {
        if matches!(item["flags"].as_u64(), Some(0 | 128)) {
            normalize_key(item);
        }
    }
    if !items.is_empty() {
        if items[0]["action"] == "keyup" {
            items.remove(0);
        }
        if let Some(first) = items.first_mut() {
            first["time"] = json!(0);
        }
        // Mt.macroItemsVerification uses flags exactly 0/128, including its
        // undefined-key entry for non-key inputs. Omitted JS fields stay absent.
        let mut held = BTreeMap::<String, (Option<Value>, Option<Value>)>::new();
        for item in &items {
            let key = item.get("vkCode").map(Value::to_string).unwrap_or_default();
            if item["flags"] == 0 {
                held.entry(key).or_insert_with(|| {
                    (item.get("vkCode").cloned(), item.get("scanCode").cloned())
                });
            } else if item["flags"] == 128 {
                held.remove(&key);
            }
        }
        for (vk, scan) in held.into_values() {
            let mut release = json!({"action":"keyup","flags":128,"time":0,"timerTick":now});
            if let Some(vk) = vk {
                release["vkCode"] = vk;
            }
            if let Some(scan) = scan {
                release["scanCode"] = scan;
            }
            items.push(release);
        }
    }
    let mut events = Vec::new();
    let mut movement = Vec::new();
    let mut previous_nonmove = false;
    let mut identity = now;
    for (index, item) in items.iter().enumerate() {
        if machine.mouse_move_type != "none" && item["action"] == "mousemove" {
            if previous_nonmove || index == 0 {
                previous_nonmove = false;
                events.extend(raw_event(item, &events, &mut identity, now)?);
            }
            movement.push(json!({"x":item["x"],"y":item["y"],"time":format!("{:.3}",number(&item["time"])/1000.).parse::<f64>()?}));
        } else if item["action"] != "mousemove" {
            if !movement.is_empty() {
                if let Some(last) = events.iter_mut().rev().find(|event| event["Type"] == 3) {
                    last["Number"] = json!(format!(
                        "{:.3}",
                        movement
                            .iter()
                            .map(|point| number(&point["time"]))
                            .sum::<f64>()
                            / 1000.
                    ));
                    last["MouseEvent"]["Buffer"] = json!(std::mem::take(&mut movement));
                } else {
                    movement.clear();
                }
            }
            previous_nonmove = true;
            events.extend(raw_event(item, &events, &mut identity, now)?);
        }
    }
    // rr does not flush a final movement-only tail after forEach.
    let mut events = clean_events(events);
    let geometry = json!({"monitorInforBefore":machine.monitor_after,"virtualScreenInforBefore":machine.screen_after});
    let mode = match machine.mouse_move_type {
        "absolute" => 1,
        "foreground" => 2,
        "relative" => 3,
        _ => 0,
    };
    let mut engine_events = Vec::new();
    for event in &events {
        engine_events.extend(crate::macro_engine::compile_event(
            event,
            mode,
            Some(&geometry),
        )?);
    }
    let mut engine = json!({"events":engine_events,"mouseMoveType":machine.mouse_move_type});
    engine["hash"] = json!(source_hash(&engine)?);
    events.insert(
        0,
        json!({"Type":"actionBar","recordProfile":{},"selected":false}),
    );
    let names = existing
        .iter()
        .filter_map(|doc| doc["name"].as_str())
        .collect::<BTreeSet<_>>();
    let mut name = "Macro 1".to_owned();
    for index in 0..names.len() {
        if !names.contains(name.as_str()) {
            break;
        }
        name = format!("Macro {}", index + 2);
    }
    Ok(
        json!({"name":name,"guid":guid,"appEngine":engine,"macroEvents":events,
        "actionLoad":{"previousAction":[],"nextAction":[]},"macroPlaybackOption":"","delaySetting":0,
        "macroItems":items,"deviceList":[],"monitorInfo":machine.monitor_after,
        "virtualScreenInfo":machine.screen_after,"mouseMoveType":machine.mouse_move_type}),
    )
}

pub fn assign_mapping(
    assigned: &[Value],
    macro_doc: &Value,
    analog: &Value,
    dkm: &[Value],
) -> Result<Option<Value>> {
    let last = assigned.last().context("OTFM has no assigned key")?;
    let kind = last["type"].as_str().unwrap_or("");
    let flag = last["flag"].as_u64();
    let keys =
        crate::mapping_engine::source_keys(70).context("Current OTFM key table is absent")?;
    let key = keys
        .iter()
        .find(|key| {
            Some(u64::from(key.output_flag.unwrap_or(key.flag))) == flag
                && last["scanCode"] == key.scancode
                && last[if kind == "analogKey" {
                    "virtualKey"
                } else {
                    "vkCode"
                }] == key
                    .virtual_key
                    .parse::<u32>()
                    .ok()
                    .map(|v| json!(v))
                    .unwrap_or(Value::Null)
        })
        .or_else(|| {
            keys.iter().find(|key| {
                Some(u64::from(key.output_flag.unwrap_or(key.flag))) == flag
                    && last["scanCode"] == key.scancode
            })
        });
    let input = if kind == "razerKey" {
        dkm.iter()
            .find(|key| key["key"] == last["key"])
            .and_then(|key| key["inputID"].as_str())
    } else {
        key.and_then(|key| key.input_id)
    };
    let Some(input) = input else {
        return Ok(None);
    };
    let group = json!({"name":macro_doc["name"],"guid":macro_doc["guid"],"macroPlaybackOption":"Once","repeatCount":2});
    Ok(Some(if kind == "analogKey" {
        let make = analog
            .get("defaultMakeActuation")
            .cloned()
            .unwrap_or_else(|| json!(1.5));
        let break_point = analog
            .get("defaultBreakActuation")
            .cloned()
            .unwrap_or_else(|| json!(1.5));
        let point = json!({"0":make,"1":break_point});
        json!({"inputType":"AnalogInput","isHyperShift":false,"inputID":input,"mapping":[
            {"actuationPoint":point,"outputType":"macroGroup","macroGroup":group},
            {"actuationPoint":point,"isDefault":false,"outputType":"defaultGroup","isHyperShift":false}]})
    } else {
        let mut mapping = json!({"isDefault":false,"isActiveMapping":true,"isHyperShift":false,"inputID":input,
            "outputType":"macroGroup","inputType":if kind=="razerKey" {"DKMInput"}else{"KeyInput"},"inputModifiers":[],"macroGroup":group});
        if kind == "razerKey" {
            mapping["dkmInput"] = json!(input);
        }
        mapping
    }))
}

pub fn update_profile(
    document: &Value,
    mapping: Option<Value>,
    movement: &str,
    context: &mut EngineContext,
) -> Result<Value> {
    let active = document["activeProfile"]
        .as_str()
        .context("OTFM active device profile is absent")?;
    let product = document["productId"]
        .as_u64()
        .context("OTFM device product is absent")? as u32;
    let mut result = document.clone();
    let profile = result["profiles"]
        .as_array_mut()
        .context("OTFM device profiles are absent")?
        .iter_mut()
        .find(|p| p["guid"] == active)
        .context("OTFM active profile was not found")?;
    if !profile["mappings"].is_array() {
        profile["mappings"] = json!([]);
    }
    let rows = profile["mappings"].as_array_mut().unwrap();
    if let Some(mut mapping) = mapping {
        if let Some(previous) = rows.iter_mut().find(|row| {
            row["inputID"] == mapping["inputID"] && row["isHyperShift"] == mapping["isHyperShift"]
        }) {
            if previous["inputType"] == "AnalogInput" {
                let rapid = previous["mapping"][0]
                    .get("rapidTrigger")
                    .or_else(|| previous.get("rapidTrigger"))
                    .cloned();
                let make = document["analogData"]
                    .get("defaultMakeActuation")
                    .cloned()
                    .unwrap_or_else(|| json!(19656));
                let break_point = document["analogData"]
                    .get("defaultBreakActuation")
                    .cloned()
                    .unwrap_or_else(|| json!(18018));
                let point = json!({"0":previous["mapping"][0]["actuationPoint"].get("0").cloned().unwrap_or(make),
                    "1":previous["mapping"][0]["actuationPoint"].get("1").cloned().unwrap_or(break_point)});
                if mapping["inputType"] != "AnalogInput" {
                    mapping = json!({"inputType":"AnalogInput","isHyperShift":mapping["isHyperShift"],"inputID":mapping["inputID"],"mapping":[
                        {"outputType":mapping["outputType"],"macroGroup":mapping["macroGroup"],"actuationPoint":point},
                        {"isDefault":false,"outputType":"defaultGroup","isHyperShift":mapping["isHyperShift"],"actuationPoint":point}]});
                } else {
                    for stage in mapping["mapping"].as_array_mut().unwrap() {
                        stage["actuationPoint"] = point.clone();
                    }
                }
                if let Some(rapid) = mapping["mapping"][0]
                    .get("rapidTrigger")
                    .or_else(|| mapping.get("rapidTrigger"))
                    .cloned()
                    .or(rapid)
                {
                    mapping["mapping"][0]["rapidTrigger"] = rapid;
                }
            }
            *previous = mapping;
        } else {
            rows.push(mapping);
        }
    }
    let (analog, ordinary): (Vec<_>, Vec<_>) = rows
        .iter()
        .filter(|row| row["outputType"] != "modTapGroup")
        .cloned()
        .partition(|row| row["inputType"] == "AnalogInput");
    let factory = MappingEngine::new(product)?;
    let mut draft = context.clone();
    let mut mappings = factory.generate(&ordinary, &mut draft)?.app_engine["mappings"]
        .as_array()
        .context("OTFM ordinary mappings are absent")?
        .clone();
    if !profile["appEngine"].is_object() {
        profile["appEngine"] = json!({});
    }
    if !analog.is_empty() {
        let generated = factory.generate_analog(&analog, &mut draft)?.app_engine;
        for group in generated["mappings"]
            .as_array()
            .context("OTFM analog mappings are absent")?
        {
            mappings.extend(
                group
                    .as_array()
                    .context("OTFM analog mapping group is invalid")?
                    .iter()
                    .cloned(),
            );
        }
        for field in ["actuations", "rapidTriggers"] {
            let value = &generated[field];
            if value.as_array().is_some_and(|v| !v.is_empty())
                || value.as_object().is_some_and(|v| !v.is_empty())
            {
                profile["appEngine"][field] = value.clone();
            } else {
                profile["appEngine"].as_object_mut().unwrap().remove(field);
            }
        }
    }
    profile["appEngine"]["mappings"] = json!(mappings);
    profile["appEngine"]["hash"] = json!(source_hash(&profile["appEngine"])?);
    if !profile["macro"].is_object() {
        profile["macro"] = json!({});
    }
    profile["macro"]["mouseMoveType"] = json!(movement);
    result["version"] = match &document["version"] {
        Value::Number(value) => value
            .as_u64()
            .and_then(|v| v.checked_add(1))
            .map(|v| json!(v))
            .unwrap_or_else(|| json!(value.as_f64().unwrap_or(0.) + 1.)),
        _ => json!(0),
    };
    *context = draft;
    Ok(result)
}
