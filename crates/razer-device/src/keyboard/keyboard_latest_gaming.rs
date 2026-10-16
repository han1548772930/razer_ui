//! Current 756 MW6737 module 1101 (`getKeyMappingForGameMode`).
//! The firmware analog caller runs analog and keyboard passes on the same
//! consumed-mapping context. Preparation is pure; no device save is implied.
//! Original module bytes 489720..524573, SHA-256
//! 85083d2f1709d541f88d44a397ef75fbdcf34d7725af96bcd854e3c0373870d0.
use crate::mapping_engine::{EngineContext, MappingEngine};
use crate::mapping_engine_hash::{canonical_hash_input, source_hash};
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[path = "keyboard_latest_gaming_cascade.rs"]
mod cascade;
#[path = "keyboard_latest_gaming_custom.rs"]
mod custom;

const CONSUMED: &str = "__gameModeConsumedMappings";

#[derive(Clone, Debug)]
struct Row {
    value: Value,
    // JS Map keys are object identities. Equal records can occupy different
    // positions, while a newly generated game-mode record has no position.
    position: Option<usize>,
}
impl Row {
    fn new(value: Value) -> Self {
        Self {
            value,
            position: None,
        }
    }
}

#[derive(Default)]
struct GameContext {
    snapshot: Vec<Value>,
    raw_inputs: BTreeSet<String>,
    source: BTreeMap<String, Vec<Value>>,
    conditional: BTreeMap<String, Vec<Value>>,
    protected: BTreeMap<String, BTreeSet<String>>,
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.0),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}
fn number(value: &Value) -> f64 {
    match value {
        Value::Null => 0.0,
        Value::Bool(v) => {
            if *v {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(v) => v.as_f64().unwrap_or(f64::NAN),
        Value::String(v) if v.trim().is_empty() => 0.0,
        Value::String(v) => {
            let v = v.trim();
            let radix = [
                ("0x", 16),
                ("0X", 16),
                ("0b", 2),
                ("0B", 2),
                ("0o", 8),
                ("0O", 8),
            ]
            .into_iter()
            .find_map(|(prefix, base)| v.strip_prefix(prefix).map(|digits| (digits, base)));
            if let Some((digits, base)) = radix {
                u64::from_str_radix(digits, base)
                    .map(|v| v as f64)
                    .unwrap_or(f64::NAN)
            } else {
                v.parse().unwrap_or(f64::NAN)
            }
        }
        Value::Array(v) if v.is_empty() => 0.0,
        Value::Array(v) if v.len() == 1 && !v[0].is_object() => {
            if v[0].is_null() {
                0.0
            } else {
                number(&v[0])
            }
        }
        _ => f64::NAN,
    }
}
fn field<'a>(value: &'a Value, key: &str, fallback: &'a Value) -> &'a Value {
    value.get(key).filter(|v| !v.is_null()).unwrap_or(fallback)
}
fn stable(value: &Value) -> String {
    // The document and generated records are checked before reaching here.
    // Escaping '<' changes only the private identity representation, equally
    // on both sides; these strings are never persisted in source snapshots.
    canonical_hash_input(value, &[]).expect("validated source identity")
}
fn copy(to: &mut Value, from: &Value, key: &str) {
    if let Some(value) = from.get(key) {
        to[key] = value.clone();
    }
}
fn decorate_identity(out: &mut Value, input: &Value) {
    for key in ["hypershift", "otfs"] {
        if input[key] == true {
            out[key] = json!(true);
        }
    }
    for (enabled, id) in [("dkp", "dkpId"), ("snaptap", "snaptapId")] {
        if input[enabled] == true || input.get(id).is_some() {
            out[enabled] = json!(true);
            copy(out, input, id);
            if enabled == "snaptap" && input["snaptapPrioritize"] == true {
                out["snaptapPrioritize"] = json!(true);
            }
        }
    }
}
// D: native input identity, deliberately ignoring modifiers/gamemode and
// treating absent hypershift like false. It preserves extended flags/IDs.
fn native(input: &Value) -> String {
    if !input.is_object() {
        return String::new();
    }
    let mut out = json!({"flag":field(input,"flag",&Value::from(0))});
    copy(&mut out, input, "type");
    decorate_identity(&mut out, input);
    match input["type"].as_str() {
        Some("keyboard" | "analogKey") => {
            if input.get("scancode").is_some() {
                copy(&mut out, input, "scancode");
            } else {
                copy(&mut out, input, "page");
                copy(&mut out, input, "id");
            }
            if input["type"] == "keyboard" {
                copy(&mut out, input, "unitId");
            }
        }
        Some("razerKey") => {
            copy(&mut out, input, "key");
            copy(&mut out, input, "reportId");
        }
        _ => {
            for key in ["key", "id", "data", "button", "axis", "unitId", "reportId"] {
                copy(&mut out, input, key);
            }
        }
    }
    stable(&out)
}
// T distinguishes conditional mappings without changing D's native identity.
fn full(input: &Value) -> String {
    let identity = native(input);
    if identity.is_empty() {
        return identity;
    }
    stable(&json!({"nativeIdentity":identity,"modifiers":field(input,"modifiers",&Value::from(0))}))
}
// L/N retain the original input type's unit/report rules even when looking
// up the opposite pass. Do not coerce an analog input into a keyboard first.
fn source_key(input: &Value, kind: &str) -> String {
    let mut out =
        json!({"flag":field(input,"flag",&Value::from(0)),"hypershift":input["hypershift"]==true});
    if input.get("scancode").is_some() {
        copy(&mut out, input, "scancode");
    } else {
        copy(&mut out, input, "page");
        copy(&mut out, input, "id");
    }
    if input["type"] == "keyboard" {
        copy(&mut out, input, "unitId");
    }
    if input["type"] == "razerKey" {
        copy(&mut out, input, "reportId");
    }
    decorate_identity(&mut out, input);
    format!("{kind}:{}", stable(&out))
}
fn opposite(kind: &str) -> &'static str {
    if kind == "analogKey" {
        "keyboard"
    } else {
        "analogKey"
    }
}
fn input(kind: &str, key: u32, flag: u32, hypershift: Option<bool>, gaming: bool) -> Value {
    let mut out = json!({"type":kind,"scancode":key,"flag":flag});
    if let Some(value) = hypershift {
        out["hypershift"] = json!(value);
    }
    if gaming {
        out["gamemode"] = json!(true);
    }
    out
}
fn keyboard(key: u32, flag: u32) -> Value {
    input("keyboard", key, flag, None, false)
}
fn disabled() -> Value {
    json!({"type":"disabled"})
}
fn fn_input() -> Value {
    json!({"type":"razerKey","key":1,"flag":0})
}
fn entry(condition: Option<Value>, output: Value, track: bool) -> Value {
    let mut out = json!({"output":output});
    if let Some(condition) = condition {
        out["input"] = condition;
    }
    if track {
        out["track"] = json!(true);
    }
    out
}
fn cascade(entries: Vec<Value>) -> Value {
    json!({"type":"cascade","cascade":entries})
}
fn append(rows: &mut Vec<Row>, record: Value) {
    if !truthy(&record["input"]) {
        return;
    }
    if record["input"]["gamemode"] == true
        && rows.iter().any(|row| {
            row.value["input"]["gamemode"] == true
                && native(&row.value["input"]) == native(&record["input"])
        })
    {
        return;
    }
    rows.push(Row::new(record));
}
fn push(rows: &mut Vec<Row>, input: Value, output: Value) {
    append(rows, json!({"input":input,"output":output}));
}
fn ordinary(record: &Value, key: u32, flag: u32, kind: &str, modifiers: f64) -> bool {
    let value = &record["input"];
    value["gamemode"] != true
        && value["hypershift"] != true
        && native(value) == native(&input(kind, key, flag, None, false))
        && number(&value["modifiers"]) == modifiers
}
fn remember(context: &mut GameContext, row: &Row) {
    if !truthy(&row.value["input"]) {
        return;
    }
    let mut snapshot = json!({"origin":"active","source":if context.raw_inputs.contains(&full(&row.value["input"])) {"raw"}else{"appEngine"},"mapping":row.value});
    if let Some(position) = row.position {
        snapshot["position"] = json!(position);
    }
    context.snapshot.push(snapshot);
}
fn consume(rows: &mut Vec<Row>, index: usize, context: &mut GameContext) -> Value {
    let row = rows.remove(index);
    remember(context, &row);
    let value = &row.value["input"];
    if matches!(value["type"].as_str(), Some("keyboard" | "analogKey"))
        && (value["modifiers"].is_null() || value["modifiers"].as_f64() == Some(0.0))
    {
        let list = context
            .source
            .entry(source_key(value, value["type"].as_str().unwrap()))
            .or_default();
        if !list.iter().any(|row| full(&row["input"]) == full(value)) {
            list.push(row.value.clone());
        }
    }
    row.value
}
fn take_source(
    rows: &mut Vec<Row>,
    key: u32,
    flag: u32,
    kind: &str,
    context: &mut GameContext,
) -> Option<Value> {
    for kind in [kind, opposite(kind)] {
        if let Some(index) = rows
            .iter()
            .rposition(|row| ordinary(&row.value, key, flag, kind, 0.0))
        {
            return Some(consume(rows, index, context));
        }
        if let Some(row) = context
            .source
            .get(&source_key(&input(kind, key, flag, None, false), kind))
            .and_then(|list| list.first())
        {
            return Some(row.clone());
        }
    }
    None
}
fn take_conditions(
    rows: &mut Vec<Row>,
    key: u32,
    flag: u32,
    kind: &str,
    context: &mut GameContext,
) {
    for kind in [kind, opposite(kind)] {
        let id = source_key(&input(kind, key, flag, None, false), kind);
        let mut list = context.conditional.get(&id).cloned().unwrap_or_default();
        for index in (0..rows.len()).rev() {
            let modifiers = number(&rows[index].value["input"]["modifiers"]);
            if modifiers != 0.0
                && cascade::choices(modifiers).is_some()
                && ordinary(&rows[index].value, key, flag, kind, modifiers)
            {
                let record = consume(rows, index, context);
                if !list
                    .iter()
                    .any(|old| full(&old["input"]) == full(&record["input"]))
                {
                    list.push(record);
                }
            }
        }
        let found = !list.is_empty();
        context.conditional.insert(id, list);
        if found {
            return;
        }
    }
}
fn take_pair(
    rows: &mut Vec<Row>,
    key: u32,
    kind: &str,
    context: &mut GameContext,
) -> [Option<Value>; 2] {
    let records = [
        take_source(rows, key, 0, kind, context),
        take_source(rows, key, 1, kind, context),
    ];
    take_conditions(rows, key, 0, kind, context);
    take_conditions(rows, key, 1, kind, context);
    records.map(|record| record.and_then(|record| record.get("output").cloned()))
}

fn flatten_generated(rows: &Value, result: &mut Vec<Value>) {
    if let Some(rows) = rows.as_array() {
        for row in rows {
            if row.is_array() {
                flatten_generated(row, result);
            } else if truthy(&row["input"]) && row["input"]["gamemode"] != true {
                result.push(row.clone());
            }
        }
    }
}
// b has per-record catch semantics. A rejected raw record contributes no
// identity; it never causes a fake device read or successful persistence.
fn raw_mappings(
    profile: &Value,
    engine: &MappingEngine,
    context: &mut EngineContext,
) -> Vec<Value> {
    let mut result = Vec::new();
    for row in profile["mappings"].as_array().into_iter().flatten() {
        let mut draft = context.clone();
        if row["inputType"] == "AnalogInput" {
            draft.analog_generation = Some(
                field(
                    row,
                    "analogGeneration",
                    field(profile, "analogGeneration", &Value::from("analogV1")),
                )
                .as_str()
                .unwrap_or("analogV1")
                .into(),
            );
            if let Ok(output) = engine.generate_analog(&[row.clone()], &mut draft) {
                flatten_generated(&output.app_engine["mappings"], &mut result);
                *context = draft;
            }
        } else if let Ok(output) = engine.record(row, &mut draft) {
            result.extend(
                output
                    .into_iter()
                    .filter(|row| truthy(&row["input"]) && row["input"]["gamemode"] != true),
            );
            *context = draft;
        }
    }
    result
}
fn restore(rows: Vec<Row>, profile: &Value, raw: &[Value]) -> Vec<Row> {
    fn insert(
        result: &mut Vec<Row>,
        seen: &mut BTreeSet<String>,
        value: &Value,
        position: Option<usize>,
    ) {
        let id = full(&value["input"]);
        if id.is_empty() || !seen.insert(id.clone()) {
            return;
        }
        if let Some(index) = result
            .iter()
            .position(|row| full(&row.value["input"]) == id)
        {
            result[index] = Row {
                value: value.clone(),
                position: Some(index),
            };
        } else {
            let index = position.unwrap_or(result.len()).min(result.len());
            result.insert(
                index,
                Row {
                    value: value.clone(),
                    position: Some(index),
                },
            );
        }
    }
    let snapshot = &profile["appEngine"][CONSUMED];
    let snapshot = if snapshot["version"].as_f64() == Some(1.0)
        && profile["guid"].is_string()
        && snapshot["profileGuid"] == profile["guid"]
    {
        snapshot["mappings"].as_array()
    } else {
        None
    };
    let gaming = rows
        .iter()
        .filter(|row| row.value["input"]["gamemode"] == true)
        .cloned()
        .collect::<Vec<_>>();
    let mut result = rows
        .into_iter()
        .filter(|row| row.value["input"]["gamemode"] != true)
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    if let Some(snapshot) = snapshot {
        let mut records = snapshot
            .iter()
            .filter(|record| {
                record["origin"] == "active"
                    && matches!(record["source"].as_str(), Some("raw" | "appEngine"))
                    && record["mapping"].get("input").is_some()
            })
            .collect::<Vec<_>>();
        records.sort_by(|a, b| {
            number(field(a, "position", &json!(9_007_199_254_740_991u64))).total_cmp(&number(
                field(b, "position", &json!(9_007_199_254_740_991u64)),
            ))
        });
        let mut used = BTreeSet::new();
        for record in records {
            let id = full(&record["mapping"]["input"]);
            let source = raw
                .iter()
                .enumerate()
                .find(|(index, row)| !used.contains(index) && full(&row["input"]) == id);
            let value = if let Some((index, row)) = source {
                used.insert(index);
                row
            } else if record["source"] == "raw" {
                continue;
            } else {
                &record["mapping"]
            };
            insert(
                &mut result,
                &mut seen,
                value,
                record["position"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok()),
            );
        }
    } else {
        let mut positions = BTreeMap::new();
        for row in gaming {
            let id = native(&row.value["input"]);
            if !id.is_empty() {
                positions
                    .entry(id)
                    .or_insert(row.position.unwrap_or(result.len()));
            }
        }
        for row in raw {
            if let Some(position) = positions.get(&native(&row["input"])) {
                insert(&mut result, &mut seen, row, Some(*position));
            }
        }
    }
    result
}

fn alt_guards(kind: &str, track: bool, hypershift: Option<bool>, mod_tap: bool) -> Vec<Value> {
    let mut entries = [0, 2]
        .into_iter()
        .map(|flag| {
            entry(
                Some(input(kind, 56, flag, hypershift, false)),
                disabled(),
                track,
            )
        })
        .collect::<Vec<_>>();
    if mod_tap {
        entries.push(entry(
            Some(input("keyboard", 56, 2, hypershift, false)),
            disabled(),
            track,
        ));
    }
    entries
}
fn apply_alt(rows: &mut Vec<Row>, key: u32, kind: &str, mod_tap: bool, context: &mut GameContext) {
    let outputs = take_pair(rows, key, kind, context);
    for flag in [0, 1] {
        let mut entries = if flag == 0 || kind == "analogKey" {
            alt_guards(kind, false, None, mod_tap)
        } else {
            Vec::new()
        };
        entries.push(entry(
            None,
            outputs[flag as usize]
                .clone()
                .unwrap_or_else(|| keyboard(key, flag)),
            true,
        ));
        push(rows, input(kind, key, flag, None, true), cascade(entries));
    }
}
fn apply_f4(
    rows: &mut Vec<Row>,
    defaults: &[Row],
    kind: &str,
    multi: bool,
    compose: bool,
    mod_tap: bool,
    context: &mut GameContext,
) {
    let outputs = if multi {
        [0, 1].map(|flag| {
            defaults
                .iter()
                .find(|row| ordinary(&row.value, 62, flag, kind, 0.0))
                .or_else(|| {
                    defaults
                        .iter()
                        .find(|row| ordinary(&row.value, 62, flag, opposite(kind), 0.0))
                })
                .and_then(|row| row.value.get("output").cloned())
        })
    } else {
        take_pair(rows, 62, kind, context)
    };
    for hypershift in [None, Some(true)] {
        for flag in [0, 1] {
            let mut entries = if flag == 0 || kind == "analogKey" {
                alt_guards(kind, true, None, false)
            } else {
                Vec::new()
            };
            if (flag == 0 || kind == "analogKey") && hypershift == Some(true) {
                entries.push(entry(Some(fn_input()), disabled(), true));
            }
            entries.push(entry(
                None,
                outputs[flag as usize]
                    .clone()
                    .unwrap_or_else(|| keyboard(62, flag)),
                true,
            ));
            push(
                rows,
                input(kind, 62, flag, hypershift, true),
                cascade(entries),
            );
        }
    }
    if compose {
        for flag in [0, 1] {
            let entries = if kind == "analogKey" {
                let mut entries = alt_guards(kind, false, Some(true), mod_tap);
                entries.push(entry(None, keyboard(62, flag), false));
                entries
            } else if flag == 0 {
                let mut inner = alt_guards(kind, true, None, false);
                inner.push(entry(None, keyboard(5, flag), true));
                vec![
                    entry(Some(fn_input()), cascade(inner), true),
                    entry(None, keyboard(5, flag), true),
                ]
            } else {
                vec![entry(None, keyboard(5, flag), true)]
            };
            push(
                rows,
                input(kind, 5, flag, Some(true), true),
                cascade(entries),
            );
        }
    }
}
fn pass(
    rows: &mut Vec<Row>,
    defaults: &mut Vec<Row>,
    settings: &Value,
    kind: &str,
    first: bool,
    compose: bool,
    mod_tap: bool,
    multi: bool,
    context: &mut GameContext,
) {
    for hypershift in [None, Some(true)] {
        for flag in [2, 3] {
            push(
                defaults,
                input(kind, 93, flag, hypershift, true),
                disabled(),
            );
        }
    }
    if first {
        for key in [210, 246] {
            for hypershift in [true, false] {
                for flag in [0, 1] {
                    push(
                        defaults,
                        json!({"type":"razerKey","key":key,"flag":flag,"hypershift":hypershift,"gamemode":true}),
                        disabled(),
                    );
                }
            }
        }
    }
    if settings["isWindowsKeyDisabled"] == true {
        for flag in [2, 3] {
            push(defaults, input(kind, 91, flag, None, true), disabled());
        }
    }
    if [
        "isAltTabDisabled",
        "isAltF4Disabled",
        "isAltSpaceDisabled",
        "isAltEnterDisabled",
    ]
    .iter()
    .any(|key| settings[*key] == true)
    {
        for flag in [0, 1, 2, 3] {
            push(
                rows,
                input(kind, 56, flag, None, true),
                cascade(vec![entry(None, keyboard(56, flag), true)]),
            );
        }
    }
    // Ie's extra modTap override is suppressed by the firmware dual-pass
    // caller's final argument; 756 must never use the legacy approximation.
    if settings["isAltTabDisabled"] == true {
        apply_alt(rows, 15, kind, mod_tap, context);
    }
    if settings["isAltF4Disabled"] == true {
        apply_f4(rows, defaults, kind, multi, compose, mod_tap, context);
    }
    if settings["isAltSpaceDisabled"] == true {
        apply_alt(rows, 57, kind, mod_tap, context);
    }
    if settings["isAltEnterDisabled"] == true {
        apply_alt(rows, 28, kind, mod_tap, context);
    }
    if settings["isFnSleepEnabled"] == true && compose {
        let key = settings.get("scancode").cloned().unwrap_or(json!(44));
        let mut make = input(kind, 0, 0, Some(true), true);
        make["scancode"] = key.clone();
        let mut release = make.clone();
        release["flag"] = json!(1);
        let down = json!({"type":"keyboard","scancode":key,"flag":0});
        push(
            rows,
            make,
            cascade(vec![
                entry(
                    Some(fn_input()),
                    cascade(vec![entry(None, down.clone(), true)]),
                    true,
                ),
                entry(None, down, true),
            ]),
        );
        push(
            rows,
            release,
            cascade(vec![entry(
                None,
                json!({"type":"keyboard","scancode":key,"flag":1}),
                true,
            )]),
        );
    }
}

/// Fixed firmware-analog branch of the current 756 source. Caller-supplied
/// game observations are counts; unavailable observations follow source off.
pub fn apply(
    document: &Value,
    running_games: Option<usize>,
    compose_f4: bool,
    context: &mut EngineContext,
) -> Result<Value> {
    ensure!(
        document["productId"].as_u64() == Some(756),
        "Latest gaming caller requires logical product 756"
    );
    canonical_hash_input(document, &[])?;
    let profiles = document["profiles"]
        .as_array()
        .context("Latest gaming profiles are absent")?;
    // getActiveProfile keeps the last matching GUID.
    let Some(index) = profiles
        .iter()
        .rposition(|profile| profile["guid"] == document["activeProfile"])
    else {
        return Ok(document.clone());
    };
    let mut draft_context = context.clone();
    let engine = MappingEngine::new(756)?;
    let mut profile = profiles[index].clone();
    if profile.get("appEngine").is_none() {
        profile["appEngine"] = json!({"hash":"","mappings":[]});
    }
    let raw = raw_mappings(&profile, &engine, &mut draft_context);
    let rows = profile["appEngine"]["mappings"]
        .as_array()
        .context("Latest gaming active engine mappings are absent")?
        .iter()
        .enumerate()
        .map(|(index, value)| Row {
            value: value.clone(),
            position: Some(index),
        })
        .collect();
    let mut rows = restore(rows, &profile, &raw);
    profile["appEngine"]
        .as_object_mut()
        .context("Latest gaming engine is invalid")?
        .remove(CONSUMED);
    profile
        .as_object_mut()
        .context("Latest gaming profile is invalid")?
        .remove(CONSUMED);
    profile["appEngine"]["gamemode"] = json!(false);
    let mut result = document.clone();
    let settings = profile["gamingMode"].clone();
    let mode = if document.get("gameModeEnabled").is_some() {
        if truthy(&document["gameModeEnabled"]) {
            if settings["state"].as_f64() == Some(2.0) {
                json!(2)
            } else {
                json!(1)
            }
        } else {
            json!(0)
        }
    } else {
        settings["state"].clone()
    };
    let active = number(&mode) > 0.0
        && (mode.as_f64() == Some(1.0) || running_games.is_some_and(|count| count > 0));
    let mut defaults = result["defaultMappings"]["appEngine"].clone();
    let mut default_rows = defaults["mappings"].as_array().map(|rows| {
        rows.iter()
            .filter(|row| row["input"]["gamemode"] != true)
            .cloned()
            .map(Row::new)
            .collect::<Vec<_>>()
    });
    if active {
        ensure!(
            defaults.is_object(),
            "Latest gaming default engine is absent"
        );
        let defaults = default_rows.get_or_insert_with(Vec::new);
        profile["appEngine"]["gamemode"] = json!(true);
        let mut game = GameContext {
            raw_inputs: raw
                .iter()
                .map(|row| full(&row["input"]))
                .filter(|id| !id.is_empty())
                .collect(),
            ..Default::default()
        };
        let mod_tap = truthy(&profile["modTap"]);
        let multi = profile["appEngine"]["functionKeyPrimary"] == "multi";
        pass(
            &mut rows,
            defaults,
            &settings,
            "analogKey",
            true,
            compose_f4,
            mod_tap,
            multi,
            &mut game,
        );
        pass(
            &mut rows, defaults, &settings, "keyboard", false, compose_f4, mod_tap, multi,
            &mut game,
        );
        cascade::protect(&rows, &mut game);
        cascade::restore_conditions(&mut rows, &game);
        custom::apply(&mut rows, &settings, &engine, &mut draft_context, &mut game);
        if !game.snapshot.is_empty()
            && profile["guid"]
                .as_str()
                .is_some_and(|guid| !guid.is_empty())
        {
            profile["appEngine"][CONSUMED] =
                json!({"version":1,"profileGuid":profile["guid"],"mappings":game.snapshot});
        }
    }
    profile["appEngine"]["mappings"] =
        Value::Array(rows.into_iter().map(|row| row.value).collect());
    profile["appEngine"]["hash"] = json!(source_hash(&profile["appEngine"])?);
    if let Some(rows) = default_rows {
        defaults["mappings"] = Value::Array(rows.into_iter().map(|row| row.value).collect());
    }
    if !defaults.is_null() {
        defaults["hash"] = json!(source_hash(&defaults)?);
        result["defaultMappings"]["appEngine"] = defaults;
    }
    let profiles = result["profiles"].as_array_mut().unwrap();
    if active {
        profiles.remove(index);
        profiles.push(profile);
    } else {
        profiles[index] = profile;
    }
    *context = draft_context;
    Ok(result)
}

#[cfg(test)]
#[path = "keyboard_latest_gaming_tests.rs"]
mod tests;
