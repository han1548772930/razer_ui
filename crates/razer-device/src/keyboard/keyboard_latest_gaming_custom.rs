//! Module1101 c/A/u: selected custom mapping sets and custom key combinations.
use super::*;

fn selected(settings: &Value) -> Vec<Value> {
    let Some(sets) = settings["customMappingSets"]
        .as_array()
        .filter(|sets| !sets.is_empty())
    else {
        return Vec::new();
    };
    let index = number(field(settings, "customMappingSet", &Value::from(1))) - 1.0;
    let index = if index.is_nan() {
        None
    } else {
        let index = index.clamp(0.0, 2.0);
        (index.fract() == 0.0).then_some(index as usize)
    };
    let set = index
        .and_then(|index| sets.get(index))
        .filter(|value| !value.is_null())
        .unwrap_or(&sets[0]);
    set.as_array()
        .or_else(|| set["mappings"].as_array())
        .cloned()
        .unwrap_or_default()
}
fn combos(settings: &Value, engine: &MappingEngine) -> Vec<Value> {
    let values = [
        &settings["customGameMode"]["payload"]["combos"],
        &settings["customGameMode"]["combos"],
        &settings["payload"]["combos"],
        &settings["combos"],
    ];
    let Some(rows) = values
        .into_iter()
        .find(|value| !value.is_null())
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for row in rows {
        if !truthy(&row["enabled"]) {
            continue;
        }
        let Some(key) = row["key1"].as_str().filter(|key| !key.is_empty()) else {
            continue;
        };
        if engine.source_key_metadata(key).is_err() {
            continue;
        }
        let target = row["key2"]
            .as_str()
            .filter(|key| !key.is_empty())
            .filter(|key| engine.source_key_metadata(key).is_ok());
        let mut record =
            json!({"inputID":key,"inputType":"KeyInput","isHyperShift":false,"customCombo":true});
        if let Some(target) = target {
            record["outputType"] = json!("keyboardGroup");
            record["keyboardGroup"] = json!({"key":target,"modifiers":[]});
        } else {
            record["disableOnly"] = json!(true);
        }
        result.push(record);
    }
    result
}
fn key_number(value: &Value) -> Option<u32> {
    value
        .get("key")
        .unwrap_or(&value["scancode"])
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}
fn combo(
    rows: &mut Vec<Row>,
    record: &Value,
    engine: &MappingEngine,
    seen: &mut BTreeSet<(String, u32, u32)>,
    context: &mut GameContext,
) {
    let Some(key) = record["inputID"]
        .as_str()
        .and_then(|key| engine.source_key_metadata(key).ok())
    else {
        return;
    };
    let Some(scan) = key_number(&key) else {
        return;
    };
    let target = if truthy(&record["disableOnly"]) {
        None
    } else {
        record["keyboardGroup"]["key"]
            .as_str()
            .and_then(|key| engine.source_key_metadata(key).ok())
    };
    if !truthy(&record["disableOnly"]) && target.is_none() {
        return;
    }
    let kind = if record["inputType"] == "AnalogInput" {
        "analogKey"
    } else {
        "keyboard"
    };
    let keys = crate::mapping_engine::source_keys(756).expect("audited source keys");
    let normal = keys.iter().any(|key| key.scancode == scan && key.flag == 0);
    let extended = keys.iter().any(|key| key.scancode == scan && key.flag == 2);
    let flag = key["flag"].as_u64().unwrap_or(0) as u32;
    let mut variants = Vec::new();
    if flag & !1 == 0 && normal {
        variants.push(0);
    }
    if flag & !1 == 2 && extended {
        variants.push(2);
    }
    if normal && !variants.contains(&0) {
        variants.push(0);
    }
    if extended && !variants.contains(&2) {
        variants.push(2);
    }
    for flag in variants.iter().flat_map(|flag| [*flag, *flag | 1]) {
        if !seen.insert((kind.into(), scan, flag)) {
            continue;
        }
        if truthy(&record["disableOnly"]) {
            if let Some(index) = rows.iter().position(|row| {
                row.value["input"]["type"] == kind
                    && row.value["input"]["scancode"] == scan
                    && row.value["input"]["flag"] == flag
                    && row.value["input"]["hypershift"] != true
                    && row.value["input"]["gamemode"] == true
            }) {
                rows.remove(index);
            }
            rows.push(Row::new(
                json!({"input":input(kind,scan,flag,None,true),"output":disabled()}),
            ));
            continue;
        }
        let identity = native(&input(kind, scan, flag, None, false));
        let gaming = rows.iter().any(|row| {
            row.value["input"]["hypershift"] != true
                && row.value["input"]["gamemode"] == true
                && native(&row.value["input"]) == identity
        });
        // Original compares output.type (string) to g (object), so this
        // selection also includes ordinary disabled mappings.
        let ordinary = rows.iter().position(|row| {
            row.value["input"]["hypershift"] != true
                && row.value["input"]["gamemode"] != true
                && native(&row.value["input"]) == identity
                && number(&row.value["input"]["modifiers"]) == 0.0
        });
        let previous = ordinary.map(|index| {
            let row = rows.remove(index);
            remember(context, &row);
            row.value
        });
        if !gaming {
            rows.push(Row::new(json!({"input":input(kind,scan,flag,None,true),"output":previous.and_then(|row|row.get("output").cloned()).filter(|value|!value.is_null()).unwrap_or_else(||keyboard(scan,flag))})));
        }
    }
    if truthy(&record["disableOnly"]) {
        return;
    }
    let Some(target) = target else {
        return;
    };
    let Some(scan_target) = key_number(&target) else {
        return;
    };
    let target_flag = target["flag"].as_u64().unwrap_or(0) as u32;
    let guard_flags = [
        if normal { Some(0) } else { None },
        if extended { Some(2) } else { None },
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    for flag in [target_flag, target_flag | 1] {
        let identity = native(&input(kind, scan_target, flag, None, false));
        let existing = rows.iter().position(|row| {
            row.value["input"]["hypershift"] != true && native(&row.value["input"]) == identity
        });
        let mut guards = guard_flags
            .iter()
            .map(|flag| {
                entry(
                    Some(input(kind, scan, *flag, None, false)),
                    disabled(),
                    false,
                )
            })
            .collect::<Vec<_>>();
        if let Some(index) = existing {
            if rows[index].value["input"]["gamemode"] == true {
                let output = &rows[index].value["output"];
                let previous = if output["type"] == "cascade" {
                    output["cascade"].as_array().cloned().unwrap_or_default()
                } else {
                    vec![entry(None, output.clone(), false)]
                };
                let mut known = previous.iter().map(stable).collect::<BTreeSet<_>>();
                guards.retain(|row| known.insert(stable(row)));
                guards.extend(previous);
                rows[index].value["output"] = cascade(guards);
            }
            continue;
        }
        guards.push(entry(None, keyboard(scan_target, flag), false));
        push(
            rows,
            input(kind, scan_target, flag, None, true),
            cascade(guards),
        );
    }
}

pub(super) fn apply(
    rows: &mut Vec<Row>,
    settings: &Value,
    engine: &MappingEngine,
    engine_context: &mut EngineContext,
    context: &mut GameContext,
) {
    let mut records = selected(settings);
    records.extend(combos(settings, engine));
    let mut seen = BTreeSet::new();
    // Firmware analog branch calls u(...,"keyboard",...). Custom mappings
    // are deliberately applied once, after protection and conditional restore.
    for record in records {
        if truthy(&record["customCombo"]) {
            combo(rows, &record, engine, &mut seen, context);
            continue;
        }
        // Original catches each custom mapping failure independently. Retain
        // all ordinary/device state; this is only engine preparation.
        if let Ok(generated) = engine.record(&record, engine_context) {
            for mut generated in generated {
                if !truthy(&generated["input"]) {
                    continue;
                }
                generated["input"]["gamemode"] = json!(true);
                let mask = number(&generated["input"]["modifiers"]);
                if mask != 0.0 && cascade::choices(mask).is_some() {
                    cascade::apply_conditional(rows, generated, context);
                } else {
                    cascade::apply_direct(rows, generated);
                }
            }
        }
    }
}
