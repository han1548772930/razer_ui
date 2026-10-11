//! Source 32807 CONFIRM/delete plans and Ko storage semantics. `data` is the
//! actual synapse_<productId> record; selectedSensitivityMatcher is profile-local.
use anyhow::{ensure, Context as _};
use serde_json::{json, Value};

pub struct Mutation {
    pub data: Value,
    /// Separate ON_STAGES_CHANGE chain, executed before matcher persistence.
    pub dpi_stages: Option<Value>,
    pub response: Value,
}
fn active_index(data: &Value) -> anyhow::Result<usize> {
    let active = data["activeProfile"]
        .as_str()
        .context("no observed active profile")?;
    data["profiles"]
        .as_array()
        .context("no observed profiles")?
        .iter()
        .position(|p| p["guid"].as_str() == Some(active))
        .context("active profile not present")
}
fn profiles(data: &Value) -> anyhow::Result<Vec<Value>> {
    if data["sensitivityMatcher"].is_null() {
        return Ok(Vec::new());
    }
    Ok(data["sensitivityMatcher"]["profiles"]
        .as_array()
        .context("invalid matcher profiles")?
        .clone())
}
fn finish(
    mut data: Value,
    index: usize,
    matcher: Value,
    selected: Option<&str>,
    dpi_stages: Option<Value>,
) -> anyhow::Result<Mutation> {
    data["sensitivityMatcher"] = matcher.clone();
    let active = data["profiles"][index]
        .as_object_mut()
        .context("invalid active profile")?;
    if let Some(selected) = selected {
        active.insert("selectedSensitivityMatcher".into(), json!(selected));
    } else {
        active.remove("selectedSensitivityMatcher");
    }
    if let Some(stages) = &dpi_stages {
        active.insert("dpiStages".into(), stages.clone());
    }
    let previous = if data.get("version").is_none() {
        None
    } else {
        Some(
            data["version"]
                .as_u64()
                .context("invalid observed version")?,
        )
    };
    data["version"] = json!(crate::mapping_submission::next_source_version(previous)?);
    let mut payload = json!({"sensitivityMatcher":matcher});
    if let Some(selected) = selected {
        payload["selectedSensitivityMatcher"] = json!(selected);
    }
    Ok(Mutation {
        data,
        dpi_stages,
        response: json!({"type":"MW_SET_SENSITIVITY_PROFILE_TO_UI","payload":payload}),
    })
}
fn replace_stage(profile: &Value, dpi: &Value) -> anyhow::Result<Option<Value>> {
    let Some(stages) = profile.get("dpiStages") else {
        return Ok(None);
    };
    let mut result = stages.clone();
    let active = result["active"]
        .as_u64()
        .context("invalid DPI active stage")?;
    let list = result["stages"]
        .as_array_mut()
        .context("invalid DPI stages")?;
    if let Some(stage) = active.checked_sub(1).and_then(|i| list.get_mut(i as usize)) {
        *stage = dpi.clone();
    }
    Ok(Some(result))
}
/// UI SET_SELECTED action emits stages first, then ON_UPDATE_SENSITIVITY_PROFILE.
/// It is not a command in the DPI_MATCHER_UI_COMMAND namespace.
pub fn select(data: &Value, guid: &str) -> anyhow::Result<Mutation> {
    let index = active_index(data)?;
    let mut rows = profiles(data)?;
    rows.retain(|row| row["name"] != "None");
    let profile = rows
        .iter()
        .find(|p| p["guid"].as_str() == Some(guid))
        .context("selected matcher profile absent")?;
    let dpi = profile.get("dpi").context("selected matcher has no DPI")?;
    ensure!(!dpi.is_null(), "selected matcher DPI is unavailable");
    let stages = replace_stage(&data["profiles"][index], dpi)?;
    finish(
        data.clone(),
        index,
        json!({"profiles":rows}),
        Some(guid),
        stages,
    )
}
/// Native computer/device name and UUID must be actual caller observations.
/// Source W$ returns the locale object unchanged because strict equality with
/// distinct object names cannot match. Do not invent a flattened display name.
pub fn confirm(data: &Value, dpi: f64, guid: &str, name: Value) -> anyhow::Result<Mutation> {
    ensure!(dpi.is_finite() && dpi > 0., "invalid measured DPI");
    ensure!(!guid.is_empty(), "missing generated UUID");
    ensure!(
        name.is_string() || name.is_object(),
        "invalid observed profile name"
    );
    let index = active_index(data)?;
    let mut rows = profiles(data)?;
    ensure!(
        !rows.iter().any(|p| p["guid"] == guid),
        "UUID already exists"
    );
    let dpi = json!({"x":dpi,"y":dpi,"independent":false,"visible":true});
    let stages = replace_stage(&data["profiles"][index], &dpi)?;
    rows.push(json!({"guid":guid,"dpi":dpi,"name":name}));
    finish(
        data.clone(),
        index,
        json!({"profiles":rows}),
        Some(guid),
        stages,
    )
}
pub fn delete(data: &Value, guid: &str) -> anyhow::Result<Mutation> {
    let index = active_index(data)?;
    let mut rows = profiles(data)?;
    rows.retain(|profile| profile["guid"].as_str() != Some(guid));
    // H selects first remaining profile even if the removed item was inactive.
    let selected = rows.first().and_then(|p| p["guid"].as_str()).map(str::to_owned);
    let stages = if data["profiles"][index]["selectedSensitivityMatcher"].as_str() == Some(guid)
        && !rows.is_empty()
    {
        replace_stage(&data["profiles"][index], &rows[0]["dpi"])?
    } else {
        None
    };
    finish(
        data.clone(),
        index,
        json!({"profiles":rows}),
        selected.as_deref(),
        stages,
    )
}
pub fn delete_all(data: &Value) -> anyhow::Result<Mutation> {
    finish(
        data.clone(),
        active_index(data)?,
        json!({"profiles":[]}),
        Some(""),
        None,
    )
}
/// Actual reducer ON_UPDATE_SENSITIVITY_PROFILE payload, including rename/add.
pub fn update(data: &Value, payload: &Value) -> anyhow::Result<Mutation> {
    let matcher = payload
        .get("sensitivityMatcher")
        .context("missing sensitivityMatcher")?
        .clone();
    matcher["profiles"]
        .as_array()
        .context("invalid sensitivityMatcher profiles")?;
    let selected = payload
        .get("selectedSensitivityMatcher")
        .map(|v| v.as_str().context("invalid selectedSensitivityMatcher"))
        .transpose()?;
    finish(data.clone(), active_index(data)?, matcher, selected, None)
}
