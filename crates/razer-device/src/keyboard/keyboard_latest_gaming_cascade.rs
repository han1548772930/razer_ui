//! Module1101 O/V..le/ce: modifier expansion and protected cascades.
use super::*;

fn bits(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    value.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
}
pub(super) fn choices(mask: f64) -> Option<Vec<Vec<(u32, u32)>>> {
    if !mask.is_finite() || mask <= 0.0 || bits(mask) & !4095 != 0 {
        return None;
    }
    let table: [(i32, &[(u32, u32)]); 12] = [
        (1, &[(56, 0), (56, 2)]),
        (2, &[(29, 0), (29, 2)]),
        (4, &[(42, 0), (54, 0)]),
        (8, &[(91, 2), (92, 2)]),
        (16, &[(56, 2)]),
        (32, &[(29, 2)]),
        (64, &[(54, 0)]),
        (128, &[(92, 2)]),
        (256, &[(56, 0)]),
        (512, &[(29, 0)]),
        (1024, &[(42, 0)]),
        (2048, &[(91, 2)]),
    ];
    let result = table
        .into_iter()
        .filter(|(bit, _)| bits(mask) & bit != 0)
        .map(|(_, keys)| keys.to_vec())
        .collect::<Vec<_>>();
    (!result.is_empty()).then_some(result)
}
// O: condition identity ignores make/release but preserves side/extended flag.
fn condition(value: &Value) -> String {
    if !value.is_object() {
        return String::new();
    }
    let mut out = json!({});
    copy(&mut out, value, "type");
    match value["type"].as_str() {
        Some("keyboard" | "analogKey") => {
            if value.get("scancode").is_some() {
                copy(&mut out, value, "scancode");
            } else {
                copy(&mut out, value, "page");
                copy(&mut out, value, "id");
            }
            out["flag"] = json!(bits(number(&value["flag"])) & !1);
            if value["type"] == "keyboard" {
                copy(&mut out, value, "unitId");
            }
        }
        Some("razerKey") => copy(&mut out, value, "key"),
        _ => {
            for key in ["scancode", "key", "page", "id", "data", "button", "axis"] {
                copy(&mut out, value, key);
            }
            if value.get("flag").is_some() {
                out["flag"] = json!(bits(number(&value["flag"])) & !1);
            }
        }
    }
    stable(&out)
}
fn entries(output: &Value) -> Option<&Vec<Value>> {
    if output["type"] == "cascade" {
        output["cascade"].as_array()
    } else {
        None
    }
}
fn has_conditions(output: &Value) -> bool {
    entries(output).is_some_and(|rows| rows.iter().any(|row| truthy(&row["input"])))
}
fn fallback(output: &Value) -> Option<usize> {
    entries(output)?
        .iter()
        .position(|row| !truthy(&row["input"]) && row.get("output").is_some())
}
pub(super) fn default_output(input: &Value) -> Value {
    let mut out = json!({"type":"keyboard","flag":field(input,"flag",&Value::from(0))});
    if input.get("scancode").is_some() {
        copy(&mut out, input, "scancode");
    } else {
        copy(&mut out, input, "page");
        copy(&mut out, input, "id");
    }
    if input["type"] == "keyboard" {
        copy(&mut out, input, "unitId");
    }
    out
}
pub(super) fn modifier_entries(
    mask: f64,
    output: &Value,
    default: &Value,
    kind: &str,
) -> Vec<Value> {
    fn expand(
        groups: &[Vec<(u32, u32)>],
        index: usize,
        output: &Value,
        default: &Value,
        kind: &str,
    ) -> Vec<Value> {
        let mut rows = groups[index]
            .iter()
            .map(|(key, flag)| {
                entry(
                    Some(input(kind, *key, *flag, None, false)),
                    if index == groups.len() - 1 {
                        output.clone()
                    } else {
                        let mut inner = expand(groups, index + 1, output, default, kind);
                        // j adds this fallback in addition to the recursive fallback.
                        inner.push(entry(None, default.clone(), true));
                        cascade(inner)
                    },
                    false,
                )
            })
            .collect::<Vec<_>>();
        rows.push(entry(None, default.clone(), true));
        rows
    }
    choices(mask)
        .map(|groups| expand(&groups, 0, output, default, kind))
        .unwrap_or_default()
}
fn leaf_paths(rows: &[Value], prefix: &[String]) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for row in rows {
        if !truthy(&row["input"]) {
            continue;
        }
        let mut path = prefix.to_vec();
        path.push(condition(&row["input"]));
        if let Some(children) = entries(&row["output"]) {
            result.extend(leaf_paths(children, &path));
        } else {
            result.insert(stable(&json!(path)));
        }
    }
    result
}
fn prefix_match(path: &[String], protected: &BTreeSet<String>) -> bool {
    protected.iter().any(|value| {
        serde_json::from_str::<Vec<String>>(value).is_ok_and(|prefix| path.starts_with(&prefix))
    })
}
// ee drops protected path prefixes. ne drops exact replacement leaves and
// preserves unrelated condition branches. Both retain fallback-only leaves.
fn prune(
    rows: &[Value],
    paths: &BTreeSet<String>,
    prefix: &[String],
    prefixes: bool,
) -> Vec<Value> {
    let mut result = Vec::new();
    for row in rows {
        if !truthy(&row["input"]) {
            result.push(row.clone());
            continue;
        }
        let mut path = prefix.to_vec();
        path.push(condition(&row["input"]));
        if prefixes && prefix_match(&path, paths) {
            continue;
        }
        let Some(children) = entries(&row["output"]) else {
            if prefixes || !paths.contains(&stable(&json!(path))) {
                result.push(row.clone());
            }
            continue;
        };
        let conditional = children.iter().any(|row| truthy(&row["input"]));
        if !prefixes && !conditional {
            if !paths.contains(&stable(&json!(path))) {
                result.push(row.clone());
            }
            continue;
        }
        let filtered = prune(children, paths, &path, prefixes);
        if conditional && !filtered.iter().any(|row| truthy(&row["input"])) {
            continue;
        }
        let mut updated = row.clone();
        updated["output"]["cascade"] = json!(filtered);
        result.push(updated);
    }
    result
}
fn disabled_paths(output: &Value, prefix: &[String], result: &mut BTreeSet<String>) {
    if let Some(rows) = entries(output) {
        for row in rows {
            if !truthy(&row["input"]) {
                continue;
            }
            let mut path = prefix.to_vec();
            path.push(condition(&row["input"]));
            if row["output"]["type"] == "disabled" {
                result.insert(stable(&json!(path)));
            } else {
                disabled_paths(&row["output"], &path, result);
            }
        }
    }
}
pub(super) fn protect(rows: &[Row], context: &mut GameContext) {
    for row in rows
        .iter()
        .filter(|row| row.value["input"]["gamemode"] == true)
    {
        let mut paths = BTreeSet::new();
        disabled_paths(&row.value["output"], &[], &mut paths);
        if !paths.is_empty() {
            context
                .protected
                .entry(native(&row.value["input"]))
                .or_default()
                .extend(paths);
        }
    }
}
struct Merge<'a> {
    mask: f64,
    output: &'a Value,
    kind: &'a str,
    protected: &'a BTreeSet<String>,
    fallback: Option<&'a Value>,
}
impl Merge<'_> {
    fn additions(&self, default: &Value) -> Vec<Value> {
        let rows = modifier_entries(self.mask, self.output, default, self.kind)
            .into_iter()
            .filter(|row| truthy(&row["input"]))
            .collect::<Vec<_>>();
        prune(&rows, self.protected, &[], true)
    }
    fn wrap(&self, default: &Value) -> Value {
        let mut rows = self.additions(default);
        if rows.is_empty() {
            return default.clone();
        }
        rows.push(entry(None, default.clone(), true));
        cascade(rows)
    }
    fn edit(&self, output: &Value) -> Value {
        let Some(index) = fallback(output) else {
            let Some(default) = self.fallback else {
                return output.clone();
            };
            let mut result = output.clone();
            result["cascade"]
                .as_array_mut()
                .unwrap()
                .push(entry(None, self.wrap(default), true));
            return result;
        };
        let rows = entries(output).unwrap();
        let mut added = self.additions(&rows[index]["output"]);
        if added.is_empty() {
            return output.clone();
        }
        let paths = leaf_paths(&added, &[]);
        let conditions = rows
            .iter()
            .filter(|row| truthy(&row["input"]))
            .cloned()
            .collect::<Vec<_>>();
        added.extend(prune(&conditions, &paths, &[], false));
        added.push(rows[index].clone());
        let mut result = output.clone();
        result["cascade"] = json!(added);
        result
    }
    fn apply(&self, output: &Value) -> Value {
        let Some(rows) = entries(output) else {
            return self.wrap(output);
        };
        if leaf_paths(rows, &[])
            .iter()
            .any(|path| self.protected.contains(path))
        {
            let Some(index) = fallback(output) else {
                return self.edit(output);
            };
            let mut result = output.clone();
            result["cascade"][index]["output"] = self.apply(&rows[index]["output"]);
            result
        } else {
            self.edit(output)
        }
    }
}
fn merge(
    output: &Value,
    mask: f64,
    replacement: &Value,
    kind: &str,
    protected: &BTreeSet<String>,
    fallback: Option<&Value>,
) -> Value {
    Merge {
        mask,
        output: replacement,
        kind,
        protected,
        fallback,
    }
    .apply(output)
}
fn replace_fallback(output: &Value, replacement: &Value) -> Value {
    if !has_conditions(output) {
        return replacement.clone();
    }
    let Some(index) = fallback(output) else {
        return output.clone();
    };
    let mut result = output.clone();
    result["cascade"][index]["output"] =
        replace_fallback(&output["cascade"][index]["output"], replacement);
    result
}
pub(super) fn apply_direct(rows: &mut Vec<Row>, record: Value) {
    if !truthy(&record["input"]) {
        return;
    }
    let Some(index) = rows.iter().position(|row| {
        row.value["input"]["gamemode"] == true
            && native(&row.value["input"]) == native(&record["input"])
    }) else {
        append(rows, record);
        return;
    };
    let output = &rows[index].value["output"];
    if output["type"] != "disabled" {
        rows[index].value["output"] = if has_conditions(output) {
            replace_fallback(output, &record["output"])
        } else {
            record["output"].clone()
        };
    }
}
pub(super) fn apply_conditional(rows: &mut Vec<Row>, record: Value, context: &mut GameContext) {
    let mask = number(&record["input"]["modifiers"]);
    if !truthy(&record["input"]) || choices(mask).is_none() {
        return;
    }
    let mut value = record["input"].clone();
    value.as_object_mut().unwrap().remove("modifiers");
    if let Some(index) = rows.iter().position(|row| {
        row.value["input"]["gamemode"] == true && native(&row.value["input"]) == native(&value)
    }) {
        if rows[index].value["output"]["type"] != "disabled" {
            let paths = context
                .protected
                .get(&native(&rows[index].value["input"]))
                .cloned()
                .unwrap_or_default();
            rows[index].value["output"] = merge(
                &rows[index].value["output"],
                mask,
                &record["output"],
                value["type"].as_str().unwrap_or(""),
                &paths,
                Some(&default_output(&value)),
            );
        }
    } else {
        let kind = value["type"].as_str().unwrap_or("");
        // Preserve le's D(c) check in BOTH lookups. Its opposite-type retry
        // does not silently acquire a different native identity.
        let find = |kind: &str| {
            rows.iter().position(|row| {
                row.value["input"]["gamemode"] != true
                    && row.value["input"]["hypershift"] != true
                    && number(&row.value["input"]["modifiers"]) == 0.0
                    && row.value["input"]["type"] == kind
                    && native(&row.value["input"]) == native(&value)
            })
        };
        let index = find(kind).or_else(|| find(opposite(kind)));
        let default = index
            .map(|index| consume(rows, index, context))
            .and_then(|record| record.get("output").cloned())
            .filter(|output| !output.is_null())
            .unwrap_or_else(|| default_output(&value));
        rows.push(Row::new(json!({"input":value,"output":cascade(modifier_entries(mask,&record["output"],&default,kind))})));
    }
}
pub(super) fn restore_conditions(rows: &mut [Row], context: &GameContext) {
    for row in rows {
        let value = row.value["input"].clone();
        if value["gamemode"] != true
            || !matches!(value["type"].as_str(), Some("keyboard" | "analogKey"))
            || value["hypershift"] == true
            || row.value["output"]["type"] != "cascade"
        {
            continue;
        }
        let kind = value["type"].as_str().unwrap();
        let same = context
            .conditional
            .get(&source_key(&value, kind))
            .filter(|list| !list.is_empty());
        let mut conditions = same
            .or_else(|| context.conditional.get(&source_key(&value, opposite(kind))))
            .cloned()
            .unwrap_or_default();
        conditions.sort_by(|a, b| {
            number(&a["input"]["modifiers"]).total_cmp(&number(&b["input"]["modifiers"]))
        });
        let paths = context
            .protected
            .get(&native(&value))
            .cloned()
            .unwrap_or_default();
        let fallback = default_output(&value);
        for record in conditions {
            row.value["output"] = merge(
                &row.value["output"],
                number(&record["input"]["modifiers"]),
                &record["output"],
                kind,
                &paths,
                Some(&fallback),
            );
        }
    }
}
