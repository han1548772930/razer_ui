//! Current-source keyboard gaming mapping preparation for twelve products.
//! `getKeyMappingForGameMode` and `applyGamingModeSettings` transform copies
//! of active/default engines. These values are not device, driver, service or
//! profile-persistence acknowledgements.
use crate::mapping_engine_hash::source_hash;
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

include!("keyboard_gaming_mapping_data_generated.rs");

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct GamingModeSettings {
    pub state: u8,
    pub is_windows_key_disabled: bool,
    pub is_alt_tab_disabled: bool,
    pub is_alt_f4_disabled: bool,
    pub is_alt_space_disabled: bool,
    pub is_alt_enter_disabled: bool,
    pub is_fn_sleep_enabled: bool,
    pub scancode: Option<u16>,
}

impl GamingModeSettings {
    /// Current `getGameModeBit`. The Fn-sleep option is not a bit in this byte.
    pub fn game_mode_bit(self) -> u8 {
        self.state
            | if self.is_windows_key_disabled { 4 } else { 0 }
            | if self.is_alt_tab_disabled { 8 } else { 0 }
            | if self.is_alt_f4_disabled { 16 } else { 0 }
            | if self.is_alt_space_disabled { 32 } else { 0 }
            | if self.is_alt_enter_disabled { 64 } else { 0 }
    }
}

/// Caller observations used by the source wrapper. `running_games` is a count
/// from the actual `gms-running-games` observation; unavailable/error is None
/// and follows the original off fallback. It is never invented as success.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GamingMappingContext {
    pub game_mode_enabled: Option<bool>,
    pub running_games: Option<usize>,
    pub compose_f4: bool,
    pub mod_tap: bool,
    /// A separate current-source pass for firmware analog devices; it is not
    /// enabled by any of this batch's twelve DeviceInfo registrations.
    pub firmware_analog_mapping: bool,
}

impl GamingMappingContext {
    pub fn effective_mode(self, settings: GamingModeSettings) -> u8 {
        match self.game_mode_enabled {
            Some(false) => 0,
            Some(true) => {
                if settings.state == 2 {
                    2
                } else {
                    1
                }
            }
            None => settings.state,
        }
    }

    fn active(self, settings: GamingModeSettings) -> bool {
        let mode = self.effective_mode(settings);
        mode > 0 && (mode == 1 || self.running_games.is_some_and(|count| count > 0))
    }
}

/// Current MW global/legacy caller -> setGamingMode -> LED state. The 679
/// global caller suppresses the factory preset's indicator even when global
/// mapping mode is enabled (6737 en/applyGlobalGameModeModel).
pub fn indicator_enabled(
    document: &Value,
    product_id: u32,
    running_games: Option<usize>,
) -> Result<bool> {
    ensure!(
        matches!(product_id, 515 | 679),
        "Gaming indicator caller was not audited for this product"
    );
    let profile = document["profiles"]
        .as_array()
        .and_then(|profiles| {
            profiles
                .iter()
                .find(|profile| profile["guid"] == document["activeProfile"])
        })
        .context("Gaming indicator active profile is missing")?;
    let state = profile["gamingMode"]["state"]
        .as_u64()
        .context("Gaming indicator state is missing")?;
    ensure!(state <= 2, "Gaming indicator state is invalid");
    let enabled = document
        .get("gameModeEnabled")
        .map(|value| value.as_bool().context("Global gaming mode is not boolean"))
        .transpose()?;
    if product_id == 679
        && enabled.is_some()
        && profile["guid"] == "af06d371-861f-4b98-8d78-bfaef5cfdebf"
    {
        return Ok(false);
    }
    Ok(GamingMappingContext {
        game_mode_enabled: enabled,
        running_games,
        ..Default::default()
    }
    .active(GamingModeSettings {
        state: state as u8,
        ..Default::default()
    }))
}

/// Immutable local preparation. Originals support exact discard/restore;
/// active/default outputs still require the separate publication chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardGamingMappingPlan {
    original_engine: Value,
    original_default_engine: Value,
    active_engine: Value,
    default_engine: Value,
    active: bool,
    settings: GamingModeSettings,
}

impl KeyboardGamingMappingPlan {
    /// No fallback substitutes the active profile for a missing default
    /// observation. Pass both observed engines, even for the common path:
    /// Windows/menu/Fn controls are appended to the default engine.
    pub fn prepare(
        product_id: u32,
        app_engine: &Value,
        default_engine: &Value,
        settings: GamingModeSettings,
        context: GamingMappingContext,
        validate: impl Fn() -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            supports_product(product_id),
            "Gaming mapping product was not source audited"
        );
        ensure!(
            settings.state <= 2,
            "Unknown gaming-mode state {}",
            settings.state
        );
        ensure!(
            !context.firmware_analog_mapping,
            "Firmware dual-pass analog mapping is not implemented for this batch"
        );
        validate()?;
        let mut plan = Self {
            original_engine: app_engine.clone(),
            original_default_engine: default_engine.clone(),
            active_engine: app_engine.clone(),
            default_engine: default_engine.clone(),
            active: context.active(settings),
            settings,
        };
        // The original wrapper returns the original data when inactive. It
        // does not leave a cloned `gamemode:false` field or stale overrides.
        if !plan.active {
            return Ok(plan);
        }
        let mut mappings = observed_mappings(app_engine, "active")?;
        let mut defaults = observed_mappings(default_engine, "default")?;
        let analog = defaults.iter().any(|m| m["input"]["type"] == "analogKey");
        let kind = if analog { "analogKey" } else { "keyboard" };

        for hypershift in [None, Some(true)] {
            for flag in [2, 3] {
                push(
                    &mut defaults,
                    input(kind, 93, flag, hypershift, true),
                    disabled(),
                );
            }
        }
        for key in [210, 246] {
            for hypershift in [true, false] {
                for flag in [0, 1] {
                    defaults.push(json!({"input":{"type":"razerKey","key":key,"flag":flag,"hypershift":hypershift,"gamemode":true},"output":disabled()}));
                }
            }
        }
        if settings.is_windows_key_disabled {
            for flag in [2, 3] {
                push(&mut defaults, input(kind, 91, flag, None, true), disabled());
            }
        }
        let any_alt = settings.is_alt_tab_disabled
            || settings.is_alt_f4_disabled
            || settings.is_alt_space_disabled
            || settings.is_alt_enter_disabled;
        if any_alt {
            for flag in [0, 1, 2, 3] {
                push(
                    &mut mappings,
                    input(kind, 56, flag, None, true),
                    cascade(vec![entry(None, keyboard(56, flag), true)]),
                );
            }
        }
        if context.mod_tap {
            for flag in [2, 3] {
                push(
                    &mut mappings,
                    input("keyboard", 56, flag, None, true),
                    cascade(vec![entry(None, keyboard(56, flag), false)]),
                );
            }
        }
        validate()?;
        if settings.is_alt_tab_disabled {
            apply_alt_key(&mut mappings, kind, 15, analog, context.mod_tap)?;
        }
        if settings.is_alt_f4_disabled {
            apply_f4(&mut mappings, &defaults, app_engine, kind, analog, context)?;
        }
        if settings.is_alt_space_disabled {
            apply_alt_key(&mut mappings, kind, 57, analog, context.mod_tap)?;
        }
        if settings.is_alt_enter_disabled {
            apply_alt_key(&mut mappings, kind, 28, analog, context.mod_tap)?;
        }
        if settings.is_fn_sleep_enabled && context.compose_f4 {
            let scancode = settings.scancode.unwrap_or(44);
            let tail = keyboard(scancode, 0);
            let inner = cascade(vec![entry(None, tail.clone(), true)]);
            push(
                &mut mappings,
                input(kind, scancode, 0, Some(true), true),
                cascade(vec![
                    entry(Some(razer_modifier()), inner, true),
                    entry(None, tail, true),
                ]),
            );
            push(
                &mut mappings,
                input(kind, scancode, 1, Some(true), true),
                cascade(vec![entry(None, keyboard(scancode, 1), true)]),
            );
        }
        validate()?;
        plan.active_engine["mappings"] = json!(mappings);
        plan.active_engine["gamemode"] = json!(true);
        plan.default_engine["mappings"] = json!(defaults);
        plan.active_engine["hash"] = json!(source_hash(&plan.active_engine)?);
        plan.default_engine["hash"] = json!(source_hash(&plan.default_engine)?);
        validate()?;
        Ok(plan)
    }

    pub fn active_engine(&self) -> &Value {
        &self.active_engine
    }
    pub fn default_engine(&self) -> &Value {
        &self.default_engine
    }
    pub fn restore_engine(&self) -> &Value {
        &self.original_engine
    }
    pub fn restore_default_engine(&self) -> &Value {
        &self.original_default_engine
    }
    pub fn settings(&self) -> GamingModeSettings {
        self.settings
    }
    pub fn game_mode_bit(&self) -> u8 {
        self.settings.game_mode_bit()
    }
    pub fn is_temporary(&self) -> bool {
        self.active
    }
}

fn observed_mappings(engine: &Value, label: &str) -> Result<Vec<Value>> {
    let mappings = engine
        .get("mappings")
        .and_then(Value::as_array)
        .with_context(|| format!("{label} appEngine mappings were not observed"))?;
    for record in mappings {
        ensure!(
            record.get("input").is_some_and(Value::is_object)
                && record.get("output").is_some_and(Value::is_object),
            "{label} appEngine contains an unsupported mapping record"
        );
        ensure!(
            record["input"]["gamemode"] != true,
            "{label} appEngine already contains temporary gaming mappings; restore its original first"
        );
    }
    Ok(mappings.clone())
}

fn input(kind: &str, scancode: u16, flag: u8, hypershift: Option<bool>, gamemode: bool) -> Value {
    let mut value = json!({"type":kind,"scancode":scancode,"flag":flag});
    if let Some(hypershift) = hypershift {
        value["hypershift"] = json!(hypershift);
    }
    if gamemode {
        value["gamemode"] = json!(true);
    }
    value
}
fn keyboard(scancode: u16, flag: u8) -> Value {
    input("keyboard", scancode, flag, None, false)
}
fn razer_modifier() -> Value {
    json!({"type":"razerKey","key":1,"flag":0})
}
fn disabled() -> Value {
    json!({"type":"disabled"})
}
fn cascade(entries: Vec<Value>) -> Value {
    json!({"type":"cascade","cascade":entries})
}
fn entry(input: Option<Value>, output: Value, track: bool) -> Value {
    let mut value = json!({"output":output});
    if let Some(input) = input {
        value["input"] = input;
    }
    if track {
        value["track"] = json!(true);
    }
    value
}
fn push(mappings: &mut Vec<Value>, input: Value, output: Value) {
    mappings.push(json!({"input":input,"output":output}));
}
fn guards(analog: bool, mod_tap: bool, track: bool, hypershift: Option<bool>) -> Vec<Value> {
    let kind = if analog { "analogKey" } else { "keyboard" };
    let mut result = vec![
        entry(
            Some(input(kind, 56, 0, hypershift, false)),
            disabled(),
            track,
        ),
        entry(
            Some(input(kind, 56, 2, hypershift, false)),
            disabled(),
            track,
        ),
    ];
    if mod_tap {
        result.push(entry(
            Some(input("keyboard", 56, 2, hypershift, false)),
            disabled(),
            track,
        ));
    }
    result
}

// The current source matches scancode/flag, deliberately without type or
// hypershift discrimination. It removes the LAST ordinary mapping per flag.
fn matches(record: &Value, scancode: u16, flag: u8, ordinary: bool) -> bool {
    record["input"]["scancode"] == scancode
        && record["input"]["flag"] == flag
        && (!ordinary || record["input"]["gamemode"] != true)
}
fn take_pair(mappings: &mut Vec<Value>, scancode: u16) -> Result<Option<[Value; 2]>> {
    let mut outputs = [None, None];
    for flag in [0, 1] {
        if let Some(index) = mappings
            .iter()
            .rposition(|m| matches(m, scancode, flag, true))
        {
            outputs[usize::from(flag)] = mappings.remove(index).get("output").cloned();
        }
    }
    observed_pair(outputs, scancode)
}
fn first_pair(mappings: &[Value], scancode: u16) -> Result<Option<[Value; 2]>> {
    let outputs = [0, 1].map(|flag| {
        mappings
            .iter()
            .find(|m| matches(m, scancode, flag, false))
            .and_then(|m| m.get("output").cloned())
    });
    observed_pair(outputs, scancode)
}
fn observed_pair(outputs: [Option<Value>; 2], scancode: u16) -> Result<Option<[Value; 2]>> {
    match outputs {
        [None, None] => Ok(None),
        [Some(down), Some(up)] => Ok(Some([down, up])),
        _ => anyhow::bail!(
            "Unpaired observed mapping for scancode {scancode}; source would omit an undefined output"
        ),
    }
}
fn apply_alt_key(
    mappings: &mut Vec<Value>,
    kind: &str,
    scancode: u16,
    analog: bool,
    mod_tap: bool,
) -> Result<()> {
    let outputs = take_pair(mappings, scancode)?;
    for flag in [0, 1] {
        let mut entries = if flag == 0 || analog {
            guards(analog, mod_tap, false, None)
        } else {
            Vec::new()
        };
        entries.push(entry(
            None,
            outputs
                .as_ref()
                .map(|p| p[usize::from(flag)].clone())
                .unwrap_or_else(|| keyboard(scancode, flag)),
            true,
        ));
        push(
            mappings,
            input(kind, scancode, flag, None, true),
            cascade(entries),
        );
    }
    Ok(())
}
fn apply_f4(
    mappings: &mut Vec<Value>,
    defaults: &[Value],
    engine: &Value,
    kind: &str,
    analog: bool,
    context: GamingMappingContext,
) -> Result<()> {
    let outputs = if engine["functionKeyPrimary"] == "multi" {
        first_pair(defaults, 62)?
    } else {
        take_pair(mappings, 62)?
    };
    for hypershift in [None, Some(true)] {
        for flag in [0, 1] {
            let mut entries = if flag == 0 || analog {
                // F4's ordinary branch does not use the extra modTap guard.
                let mut entries = guards(analog, false, true, None);
                if hypershift == Some(true) {
                    entries.push(entry(Some(razer_modifier()), disabled(), true));
                }
                entries
            } else {
                Vec::new()
            };
            entries.push(entry(
                None,
                outputs
                    .as_ref()
                    .map(|p| p[usize::from(flag)].clone())
                    .unwrap_or_else(|| keyboard(62, flag)),
                true,
            ));
            push(
                mappings,
                input(kind, 62, flag, hypershift, true),
                cascade(entries),
            );
        }
    }
    if context.compose_f4 {
        for flag in [0, 1] {
            let entries = if analog {
                let mut entries = guards(true, context.mod_tap, false, Some(true));
                entries.push(entry(None, keyboard(62, flag), false));
                entries
            } else if flag == 0 {
                let mut inner = guards(false, false, true, None);
                inner.push(entry(None, keyboard(5, flag), true));
                vec![
                    entry(Some(razer_modifier()), cascade(inner), true),
                    entry(None, keyboard(5, flag), true),
                ]
            } else {
                vec![entry(None, keyboard(5, flag), true)]
            };
            push(
                mappings,
                input(kind, 5, flag, Some(true), true),
                cascade(entries),
            );
        }
    }
    Ok(())
}

/// This logical-product gate is separate from the hardware gaming LED gate.
pub fn supports_product(product_id: u32) -> bool {
    // Current 515 middleware 2592.ca84d3908676d84b1779.js module 72937;
    // same full ordinary-keyboard transform, no analog/compose-F4 branches.
    product_id == 515 || AUDITED_GAMING_MAPPING_PRODUCTS.contains(&product_id)
}

#[cfg(test)]
#[path = "keyboard_gaming_mapping_tests.rs"]
mod tests;
