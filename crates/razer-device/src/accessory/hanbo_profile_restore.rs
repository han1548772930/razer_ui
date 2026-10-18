//! Current-profile versus previous-profile selection from MW 30525/89750 and
//! the runtime-ordered port dispatch in MW 90012 `Mn`.
use crate::{
    hanbo_cooler::{CoolerCommand, FAN_PORT_ID, PUMP_PORT_ID},
    hanbo_settings,
};
use anyhow::{Context as _, Result, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// One source profile-change restore pass. The source intentionally computes
/// `diff(current, previous)` and dispatches current port values.
pub fn commands(previous: Option<&Value>, current: Option<&Value>) -> Result<Vec<CoolerCommand>> {
    let empty = Value::Object(Default::default());
    let previous = previous.unwrap_or(&empty);
    let current_profile = current.unwrap_or(&empty);
    ensure!(
        previous.is_object(),
        "Previous Hanbo profile must be an object"
    );
    ensure!(
        current_profile.is_object(),
        "Current Hanbo profile must be an object"
    );
    let available_commands = hanbo_settings::restore_commands(current)?;
    let difference = difference(current_profile, previous)?;
    let port_difference = match difference.result.as_ref() {
        Some(Difference::Object(fields)) => fields
            .iter()
            .find_map(|(key, value)| (key == "ports").then_some(value)),
        None => None,
        _ => anyhow::bail!("Hanbo profile difference must be an object"),
    };
    let Some(port_difference) = port_difference else {
        return Ok(available_commands);
    };
    let Difference::Array(rows) = port_difference else {
        anyhow::bail!("Hanbo port difference does not have the source array format");
    };
    let mut selected = BTreeSet::new();
    // The source indexes its diff by runtime position, after merging saved
    // ports by identity. It does not index by the saved array's order.
    for (index, id) in [
        PUMP_PORT_ID,
        FAN_PORT_ID,
        4,
        2_147_483_649,
        2_147_483_650,
        2_147_483_651,
    ]
    .into_iter()
    .enumerate()
    {
        if current_profile
            .get("ports")
            .and_then(Value::as_array)
            .and_then(|ports| {
                ports
                    .iter()
                    .find(|port| port.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
            })
            .and_then(|port| port.get("isActive"))
            .is_some_and(|value| value == &Value::Bool(false))
        {
            continue;
        }
        let row = rows
            .get(index)
            .context("Hanbo port difference omits a runtime row")?;
        let fields = match row {
            ArrayDifference::Equal | ArrayDifference::Removed => continue,
            ArrayDifference::Changed(fields) => fields,
            ArrayDifference::Added(value) => {
                if let Some(object) = value.as_object() {
                    if object.get("isActive").is_some_and(truthy) || object.contains_key("thermal")
                    {
                        selected.insert(id);
                    }
                    continue;
                }
                anyhow::bail!("Hanbo inserted port is not an object");
            }
        };
        let Difference::Object(fields) = fields.as_ref() else {
            anyhow::bail!("Hanbo changed port is not an object");
        };
        if fields
            .iter()
            .any(|(key, value)| key == "isActive" && value.truthy())
            || fields.iter().any(|(key, _)| key == "thermal")
        {
            selected.insert(id);
        }
    }
    Ok(available_commands
        .into_iter()
        .filter(|command| {
            let id = match command {
                CoolerCommand::PumpParameter { .. } | CoolerCommand::PumpCurve { .. } => {
                    PUMP_PORT_ID
                }
                CoolerCommand::FanParameter { .. } | CoolerCommand::FanCurve { .. } => FAN_PORT_ID,
                CoolerCommand::CpuGpuTemperature { .. } => return false,
            };
            selected.contains(&id)
        })
        .collect())
}

#[derive(Debug)]
struct ScoredDifference<'a> {
    score: f64,
    result: Option<Difference<'a>>,
}

#[derive(Debug)]
enum Difference<'a> {
    Object(Vec<(String, Difference<'a>)>),
    Array(Vec<ArrayDifference<'a>>),
    /// Scalar replacement is the source object `{__old, __new}`.
    Scalar,
    Value(&'a Value),
}

impl Difference<'_> {
    fn truthy(&self) -> bool {
        match self {
            Self::Value(value) => truthy(value),
            Self::Object(_) | Self::Array(_) | Self::Scalar => true,
        }
    }
}

#[derive(Debug)]
enum ArrayDifference<'a> {
    Equal,
    Changed(Box<Difference<'a>>),
    Added(&'a Value),
    Removed,
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn same_scalar(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        _ => left == right,
    }
}

fn same_type(left: &Value, right: &Value) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn difference<'a>(left: &'a Value, right: &'a Value) -> Result<ScoredDifference<'a>> {
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            let mut fields = Vec::new();
            let mut score: f64 = 0.;
            for (key, value) in left {
                if !right.contains_key(key) {
                    fields.push((format!("{key}__deleted"), Difference::Value(value)));
                    score -= 30.;
                }
            }
            for (key, value) in right {
                if !left.contains_key(key) {
                    fields.push((format!("{key}__added"), Difference::Value(value)));
                    score -= 30.;
                }
            }
            for (key, value) in left {
                let Some(other) = right.get(key) else {
                    continue;
                };
                score += 20.;
                let child = difference(value, other)?;
                score += (child.score / 5.).clamp(-10., 20.);
                if let Some(child) = child.result {
                    fields.push((key.clone(), child));
                }
            }
            if fields.is_empty() {
                Ok(ScoredDifference {
                    score: 100. * (left.len() as f64).max(0.5),
                    result: None,
                })
            } else {
                Ok(ScoredDifference {
                    score: score.max(0.),
                    result: Some(Difference::Object(fields)),
                })
            }
        }
        (Value::Array(left), Value::Array(right)) => array_difference(left, right),
        _ if same_type(left, right) && same_scalar(left, right) => Ok(ScoredDifference {
            score: 100.,
            result: None,
        }),
        _ => Ok(ScoredDifference {
            score: 0.,
            result: Some(Difference::Scalar),
        }),
    }
}

#[derive(Clone, Debug)]
enum Token<'a> {
    Scalar(&'a Value),
    Object(String),
}

impl Token<'_> {
    fn key(&self) -> String {
        match self {
            Self::Object(key) => key.clone(),
            Self::Scalar(Value::String(value)) => value.clone(),
            Self::Scalar(Value::Number(value)) => {
                crate::ecmascript_number::to_string(value.as_f64().expect("JSON number"))
                    .expect("finite JSON number")
            }
            Self::Scalar(value) => value.to_string(),
        }
    }

    fn equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Scalar(left), Self::Scalar(right)) => {
                same_type(left, right) && same_scalar(left, right)
            }
            (Self::Object(left), Self::Object(right)) => left == right,
            (Self::Object(left), Self::Scalar(Value::String(right)))
            | (Self::Scalar(Value::String(right)), Self::Object(left)) => left == right,
            _ => false,
        }
    }
}

struct Original<'a> {
    key: String,
    value: &'a Value,
    index: usize,
}

struct Scalarized<'a> {
    tokens: Vec<Token<'a>>,
    originals: Vec<Original<'a>>,
    next: usize,
}

fn scalarize<'a>(values: &'a [Value], previous: Option<&Scalarized<'a>>) -> Result<Scalarized<'a>> {
    let mut matches: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    if let Some(previous) = previous {
        for (index, value) in values
            .iter()
            .enumerate()
            .filter(|(_, value)| value.is_object() || value.is_array())
        {
            let mut best: Option<(&Original<'_>, f64, usize)> = None;
            for original in &previous.originals {
                if same_type(value, original.value) {
                    let score = difference(value, original.value)?.score;
                    let distance = original.index.abs_diff(index);
                    if best.as_ref().is_none_or(|(_, best_score, best_distance)| {
                        score > *best_score || score == *best_score && distance < *best_distance
                    }) {
                        best = Some((original, score, distance));
                    }
                }
            }
            if let Some((original, score, _)) = best {
                if matches
                    .get(&original.key)
                    .is_none_or(|(prior, _)| score > *prior)
                {
                    matches.insert(original.key.clone(), (score, index));
                }
            }
        }
    }
    let mut result = Scalarized {
        tokens: Vec::new(),
        originals: Vec::new(),
        next: previous.map_or(1, |previous| previous.next),
    };
    for (index, value) in values.iter().enumerate() {
        if value.is_object() || value.is_array() {
            let key = match matches.iter().find(|(_, (_, matched))| *matched == index) {
                Some((key, _)) => key.clone(),
                None => {
                    let key = format!("__$!SCALAR{}", result.next);
                    result.next += 1;
                    key
                }
            };
            result.tokens.push(Token::Object(key.clone()));
            result.originals.push(Original { key, value, index });
        } else {
            result.tokens.push(Token::Scalar(value));
        }
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpcodeKind {
    Equal,
    Delete,
    Insert,
    Replace,
}

struct Opcode {
    kind: OpcodeKind,
    left: std::ops::Range<usize>,
    right: std::ops::Range<usize>,
}

fn matching_blocks(left: &[Token<'_>], right: &[Token<'_>]) -> Vec<(usize, usize, usize)> {
    let mut positions: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, token) in right.iter().enumerate() {
        positions.entry(token.key()).or_default().push(index);
    }
    if right.len() >= 200 {
        let threshold = right.len() / 100 + 1;
        positions.retain(|_, indices| indices.len() <= threshold);
    }
    let mut pending = vec![(0, left.len(), 0, right.len())];
    let mut blocks = Vec::new();
    while let Some((left_start, left_end, right_start, right_end)) = pending.pop() {
        let (mut best_left, mut best_right, mut best_size) = (left_start, right_start, 0);
        let mut previous: BTreeMap<usize, usize> = BTreeMap::new();
        for (index, token) in left.iter().enumerate().take(left_end).skip(left_start) {
            let mut current = BTreeMap::new();
            if let Some(indices) = positions.get(&token.key()) {
                for &other in indices
                    .iter()
                    .filter(|&&other| other >= right_start && other < right_end)
                {
                    let size = other
                        .checked_sub(1)
                        .and_then(|prior| previous.get(&prior))
                        .copied()
                        .unwrap_or(0)
                        + 1;
                    current.insert(other, size);
                    if size > best_size {
                        (best_left, best_right, best_size) =
                            (index + 1 - size, other + 1 - size, size);
                    }
                }
            }
            previous = current;
        }
        // The source matcher has no junk callback; popular entries excluded
        // from the seed map still extend an already matching block.
        while best_left > left_start
            && best_right > right_start
            && left[best_left - 1].equals(&right[best_right - 1])
        {
            best_left -= 1;
            best_right -= 1;
            best_size += 1;
        }
        while best_left + best_size < left_end
            && best_right + best_size < right_end
            && left[best_left + best_size].equals(&right[best_right + best_size])
        {
            best_size += 1;
        }
        if best_size != 0 {
            blocks.push((best_left, best_right, best_size));
            if left_start < best_left && right_start < best_right {
                pending.push((left_start, best_left, right_start, best_right));
            }
            if best_left + best_size < left_end && best_right + best_size < right_end {
                pending.push((
                    best_left + best_size,
                    left_end,
                    best_right + best_size,
                    right_end,
                ));
            }
        }
    }
    blocks.sort_unstable();
    let mut merged: Vec<(usize, usize, usize)> = Vec::new();
    for (left, right, size) in blocks {
        if let Some(prior) = merged
            .last_mut()
            .filter(|prior| prior.0 + prior.2 == left && prior.1 + prior.2 == right)
        {
            prior.2 += size;
        } else {
            merged.push((left, right, size));
        }
    }
    merged.push((left.len(), right.len(), 0));
    merged
}

fn opcodes(left: &[Token<'_>], right: &[Token<'_>]) -> Vec<Opcode> {
    let (mut left_index, mut right_index) = (0, 0);
    let mut result = Vec::new();
    for (left_start, right_start, size) in matching_blocks(left, right) {
        let kind = match (left_index < left_start, right_index < right_start) {
            (true, true) => Some(OpcodeKind::Replace),
            (true, false) => Some(OpcodeKind::Delete),
            (false, true) => Some(OpcodeKind::Insert),
            (false, false) => None,
        };
        if let Some(kind) = kind {
            result.push(Opcode {
                kind,
                left: left_index..left_start,
                right: right_index..right_start,
            });
        }
        left_index = left_start + size;
        right_index = right_start + size;
        if size != 0 {
            result.push(Opcode {
                kind: OpcodeKind::Equal,
                left: left_start..left_index,
                right: right_start..right_index,
            });
        }
    }
    result
}

fn array_difference<'a>(left: &'a [Value], right: &'a [Value]) -> Result<ScoredDifference<'a>> {
    let left_tokens = scalarize(left, None)?;
    let right_tokens = scalarize(right, Some(&left_tokens))?;
    let mut rows = Vec::new();
    let mut score: f64 = 0.;
    let mut equal = true;
    for opcode in opcodes(&left_tokens.tokens, &right_tokens.tokens) {
        if opcode.kind != OpcodeKind::Equal {
            equal = false;
        }
        match opcode.kind {
            OpcodeKind::Equal => {
                for index in opcode.left {
                    let key = left_tokens.tokens[index].key();
                    if let Some(original) = left_tokens
                        .originals
                        .iter()
                        .find(|original| original.key == key)
                    {
                        let other = right_tokens
                            .originals
                            .iter()
                            .find(|original| original.key == key)
                            .context(
                                "Source scalarized object is absent from the previous profile",
                            )?;
                        match difference(original.value, other.value)?.result {
                            Some(changed) => {
                                rows.push(ArrayDifference::Changed(Box::new(changed)));
                                equal = false;
                            }
                            None => rows.push(ArrayDifference::Equal),
                        }
                    } else {
                        rows.push(ArrayDifference::Equal);
                    }
                    score += 10.;
                }
            }
            OpcodeKind::Delete => {
                for _ in opcode.left {
                    rows.push(ArrayDifference::Removed);
                    score -= 5.;
                }
            }
            OpcodeKind::Insert => {
                for index in opcode.right {
                    rows.push(ArrayDifference::Added(&right[index]));
                    score -= 5.;
                }
            }
            OpcodeKind::Replace => {
                for _ in opcode.left {
                    rows.push(ArrayDifference::Removed);
                    score -= 5.;
                }
                for index in opcode.right {
                    rows.push(ArrayDifference::Added(&right[index]));
                    score -= 5.;
                }
            }
        }
    }
    if equal {
        Ok(ScoredDifference {
            score: 100.,
            result: None,
        })
    } else {
        Ok(ScoredDifference {
            score: score.max(0.),
            result: Some(Difference::Array(rows)),
        })
    }
}

#[cfg(test)]
#[path = "hanbo_profile_restore_tests.rs"]
mod tests;
