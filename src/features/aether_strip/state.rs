//! Device observations are never imported from a profile or invented by a request.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct Bend {
    pub(super) id: u32,
    pub(super) value: u32,
}

#[derive(Clone, Default)]
pub(super) struct Observation {
    pub(super) online: Option<bool>,
    pub(super) locked: Option<bool>,
    pub(super) power_on: Option<bool>,
    pub(super) synapse_override: Option<bool>,
    pub(super) detected: Option<u32>,
    pub(super) refreshing: bool,
}
impl Observation {
    pub(super) fn enabled(&self) -> bool {
        self.online == Some(true)
            && self.locked == Some(false)
            && self.power_on == Some(true)
            && self.synapse_override == Some(true)
    }
    pub(super) fn external_control(&self) -> bool {
        self.locked == Some(true) || self.synapse_override == Some(false)
    }
    pub(super) fn unavailable(&self) -> bool {
        self.online.is_none() || self.locked.is_none() || self.power_on.is_none()
    }
}

pub(super) fn configured(bends: &[Bend]) -> u32 {
    bends
        .iter()
        .fold(0u32, |sum, bend| sum.saturating_add(bend.value))
}

/// Current Js/$s: distribute the full detected count, rounding each share up.
pub(super) fn distribute(total: u32, sides: u32) -> Vec<Bend> {
    if !(1..=4).contains(&sides) || total < sides {
        return vec![];
    }
    let mut remaining = total;
    (0..sides)
        .map(|id| {
            let value = remaining.div_ceil(sides - id);
            remaining -= value;
            Bend { id, value }
        })
        .collect()
}

pub(super) fn restored(value: Option<&Value>, detected: Option<u32>) -> Vec<Bend> {
    let Some(array) = value
        .and_then(|v| v.get("bendData"))
        .and_then(Value::as_array)
    else {
        return vec![];
    };
    if array.is_empty() || array.len() > 4 {
        return vec![];
    }
    let mut bends = Vec::new();
    let mut total = 0u32;
    for (id, value) in array.iter().enumerate() {
        let Some(value) = value
            .get("value")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
        else {
            return vec![];
        };
        if value == 0 {
            return vec![];
        }
        let Some(next) = total.checked_add(value) else {
            return vec![];
        };
        total = next;
        bends.push(Bend {
            id: id as u32,
            value,
        });
    }
    if detected.is_some_and(|count| total > count) {
        return vec![];
    }
    bends
}

pub(super) fn identify_payload(bends: &[Bend], id: u32, detected: u32) -> Option<Value> {
    let bend = bends.iter().find(|b| b.id == id)?;
    if bend.value == 0 || configured(bends) > detected {
        return None;
    }
    let start: u32 = bends
        .iter()
        .take_while(|b| b.id != id)
        .map(|b| b.value)
        .sum();
    Some(
        serde_json::json!({"stripeLedNumber":detected,"colStart":start,
        "colEnd":start + bend.value - 1,"pollTime":3,"sleepInterval":500}),
    )
}
