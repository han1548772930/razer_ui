//! Current OBM mapping consumer. Source defaults are used for comparison;
//! every imported first stage comes from a successful device observation.
use super::{Mode, import_source as ui, mapping as wire};
use crate::{keyboard_analog::{ActuationPoint, RapidTriggerKey}, onboard_macro::MacroCatalog};
use anyhow::{Context as _, Result, ensure};

pub struct Context<'a> {
    pub product: u32,
    pub layout: u32,
    pub profile_guid: &'a str,
    pub macros: &'a MacroCatalog,
    /// The current converter's createDefaultActuation result, not the target
    /// profile's factory preset. The connection owner supplies this state.
    pub second_stage: [u16; 2],
}

pub struct Observation<'a> {
    pub assignment: &'a wire::Assignment,
    pub actuation: Option<&'a ActuationPoint>,
    pub rapid_trigger: Option<&'a RapidTriggerKey>,
}

fn output_kind(output: &wire::Output) -> Option<ui::OutputType> {
    use ui::OutputType as U;
    use wire::Output as W;
    Some(match output {
        W::Disabled => U::DisableGroup,
        W::Keyboard(_) => U::KeyboardGroup,
        W::Mouse(_) => U::MouseGroup,
        W::Macro(_) => U::MacroGroup,
        W::Dpi(_) => U::SensitivityGroup,
        W::Profile(_) => U::ProfileNavigationGroup,
        W::Power(_) => U::PowerKeyGroup,
        W::Media(_) => U::MultimediaGroup,
        W::Razer(_) => U::RazerKeyGroup,
        W::WindowsShortcut(_) => U::Win8ShortcutsGroup,
        W::Hypershift => U::HyperShiftGroup,
        W::Perfect180(_) | W::Unknown { .. } => return None,
    })
}

fn firmware_default(context: &Context<'_>, assignment: &wire::Assignment) -> bool {
    let Some(default) = wire::default_output_for(context.product, context.layout, &assignment.input_id, assignment.mode) else { return false };
    if matches!(assignment.output, wire::Output::Hypershift | wire::Output::Disabled) {
        return matches!((&assignment.output, default), (wire::Output::Disabled, wire::Output::Disabled));
    }
    let mut default = default.clone();
    let mut observed = assignment.output.clone();
    if let (wire::Output::Keyboard(left), wire::Output::Keyboard(right)) = (&mut default, &mut observed)
        && left.extended_key != right.extended_key
    {
        left.extended_key = None;
        right.extended_key = None;
        left.extended_modifier = false;
        right.extended_modifier = false;
    }
    if matches!(context.product, 721 | 739 | 752)
        && wire::default_use_turbo_for(context.product, context.layout, &assignment.input_id, assignment.mode).is_some()
    {
        // The decoded media group does not acquire the source default's
        // useTurbo property. These three converters retain that distinction.
        return false;
    }
    default == observed
}

fn analog_default(context: &Context<'_>, observed: &Observation<'_>) -> Result<bool> {
    let Some(defaults) = ui::analog(context.product) else { return Ok(true) };
    if observed.assignment.mode == Mode::Hypershift { return Ok(true) }
    let actuation = observed.actuation.context("OBM analog actuation has not been observed")?;
    let trigger = observed.rapid_trigger.context("OBM rapid trigger has not been observed")?;
    ensure!(trigger.enabled <= 1, "OBM rapid trigger has an unknown enable value");
    let actuation_defaults = defaults.default_preset_actuation.iter()
        .find(|(guid, _)| guid == context.profile_guid).map(|(_, values)| values)
        .unwrap_or(&defaults.default_actuation);
    let trigger_defaults = defaults.default_preset_rapid_trigger.iter()
        .find(|(guid, _)| guid == context.profile_guid).map(|(_, values)| values)
        .unwrap_or(&defaults.default_rapid_trigger);
    let actuation_default = actuation_defaults.specific.get(&observed.assignment.input_id).unwrap_or(&actuation_defaults.rest);
    let trigger_default = trigger_defaults.specific.get(&observed.assignment.input_id).unwrap_or(&trigger_defaults.rest);
    Ok(actuation.threshold_low == actuation_default.threshold_l
        && (trigger.enabled != 0) == trigger_default.enabled
        && (trigger.enabled == 0 || (trigger.down_sensitivity == trigger_default.down_sensitivity
        && trigger.up_sensitivity == trigger_default.up_sensitivity)))
}

fn action_name(value: impl serde::Serialize) -> Result<String> {
    // Source action enums are serialized at the existing profile-format
    // boundary; no untyped action reaches the conversion decisions.
    let value = serde_json::to_value(value)?;
    Ok(value.as_str().context("OBM action is not a source string")?.to_owned())
}

fn turbo(value: wire::Turbo) -> ui::Turbo {
    ui::Turbo { is_turbo: true, keys_per_second: value.keys_per_second }
}

fn converted_output(context: &Context<'_>, assignment: &wire::Assignment) -> Result<Option<ui::Output>> {
    use ui::OutputType as U;
    use wire::Output as W;
    let mut output = ui::Output { output_type: output_kind(&assignment.output), ..Default::default() };
    match &assignment.output {
        W::Disabled | W::Hypershift => {},
        W::Keyboard(keyboard) => {
            let keys = crate::onboard_macro_codec::keyboard_keys(context.product)?;
            let hid = keyboard.key.hid.to_string();
            let key = keys.iter().find(|key| key.hid == Some(hid.as_str()))
                .context("OBM keyboard output has no current UI key")?;
            let modifiers = keyboard.modifier_names().filter_map(|name| {
                let name = match name {
                    "Left_Shift" => "Shift", "Left_Alt" => "Alt", "Left_Ctrl" => "Ctrl",
                    "Right_Shift" => "Right Shift", "Right_Alt" => "Right Alt", "Right_Ctrl" => "Right Ctrl", "Left_GUI" => "Windows",
                    _ => return None,
                };
                keys.iter().find(|key| key.name == name).and_then(|key| key.input_id).map(str::to_owned)
            }).collect::<Vec<_>>();
            output.keyboard_group = Some(ui::Keyboard {
                key: key.input_id.map(|input| ui::Key::Named(input.to_owned())),
                key_group: Some(0), modifiers: Some(ui::Modifiers::Names(modifiers)),
                turbo_mode: keyboard.turbo.map(turbo), ..Default::default()
            });
        },
        W::Mouse(mouse) => output.mouse_group = Some(ui::Mouse { mouse_assignment: action_name(mouse.action)?, turbo_mode: mouse.turbo.map(turbo) }),
        W::Macro(value) => {
            let metadata = &context.macros.macros.iter().find(|row| row.metadata.macro_id == value.macro_id)
                .context("OBM macro assignment has no observed metadata")?.metadata;
            let (playback, count) = match value.playback {
                crate::onboard_assignment::MacroPlayback::Once => ("Once", Some(1)),
                crate::onboard_assignment::MacroPlayback::NTimes(count) => ("NTimes", Some(count)),
                crate::onboard_assignment::MacroPlayback::Held => ("ContinuousHeld", None),
                crate::onboard_assignment::MacroPlayback::Toggle => ("ContinuousToggle", None),
                crate::onboard_assignment::MacroPlayback::Sequence => anyhow::bail!("Current keyboard decoder has no sequence macro output"),
            };
            output.macro_group = Some(ui::Macro { guid: metadata.guid.clone(), name: metadata.name.clone(), macro_playback_option: playback.into(), repeat_count: count });
        },
        W::Dpi(dpi) => output.sensitivity_group = Some(ui::Sensitivity {
            sensitivity_assignment: match dpi.action {
                wire::DpiAction::Up => "DPI_Up", wire::DpiAction::Down => "DPI_Down", wire::DpiAction::Clutch => "DPI_Clutch", wire::DpiAction::CycleUp => "DPI_CycleUp", wire::DpiAction::CycleDown => "DPI_CycleDown",
            }.into(), independent: true, dpi_x: dpi.x, dpi_y: dpi.y,
        }),
        W::Profile(profile) => output.profile_navigation_group = Some(ui::ProfileNavigation {
            profile_navigation_assignment: match profile.action {
                wire::ProfileAction::Up => Some("NextProfile"), wire::ProfileAction::Down => Some("PreviousProfile"),
                wire::ProfileAction::CycleUp => Some("CycleUp"), wire::ProfileAction::CycleDown => Some("CycleDown"), wire::ProfileAction::Specific => None,
            }.map(str::to_owned), profile_id: None,
        }),
        W::Media(media) => output.multimedia_group = Some(ui::Media { multimedia_assignment: action_name(media)?, use_turbo: None }),
        W::WindowsShortcut(shortcut) => output.win8_shortcuts_group = Some(ui::Shortcut { windows_shortcut_assignment: action_name(shortcut)? }),
        W::Power(_) | W::Razer(_) | W::Perfect180(_) => return Ok(None),
        W::Unknown { reason, .. } => anyhow::bail!("OBM output is unresolved: {reason:?}"),
    }
    ensure!(output.output_type != Some(U::DefaultGroup), "Unexpected OBM output default");
    Ok(Some(output))
}

fn empty_mapping(assignment: &wire::Assignment) -> ui::Mapping {
    ui::Mapping {
        input_id: ui_input(&assignment.input_id),
        input_type: match assignment.input_kind { wire::InputKind::Key => ui::InputType::KeyInput, wire::InputKind::Dkm => ui::InputType::DKMInput, wire::InputKind::QuotedAnalog => ui::InputType::QuotedAnalogInput },
        is_hyper_shift: Some(ui::SourceFlag::Boolean(assignment.mode == Mode::Hypershift)),
        key_input: None, capital_key_input: None, analog_input: None, dkm_input: None, firmware_id: None,
        output: ui::Output::default(), mapping: None,
    }
}

fn ui_input(input: &str) -> String {
    match input {
        "KEY_NUM_LOCK" => "KEY_NUMPAD_NUM_LOCK", "Esc" => "KEY_ESC",
        "KEY_OPEN_BRACKET" => "KEY_OPEN_SQUARE_BRACKET", "KEY_CLOSE_BRACKET" => "KEY_CLOSE_SQUARE_BRACKET",
        "KEY_QUOTE" => "KEY_APOSTROPHE", "KEY_LEFT_CONTROL" => "KEY_LEFT_CTRL", "KEY_RIGHT_CONTROL" => "KEY_RIGHT_CTRL",
        "PRT_SC" => "KEY_PRINT_SCREEN", "SCR_LK" => "KEY_SCROLL_LOCK", "PAUSE" => "KEY_PAUSE",
        value => {
            if let Some(number) = value.strip_prefix('F').and_then(|value| value.parse::<u8>().ok())
                && (1..=24).contains(&number) && value == format!("F{number}")
            { return format!("KEY_F{number}") }
            value
        }
    }.to_owned()
}

pub fn convert(context: &Context<'_>, observed: Observation<'_>) -> Result<Option<ui::Mapping>> {
    let assignment = observed.assignment;
    if let wire::Output::Unknown { reason, .. } = &assignment.output {
        anyhow::bail!("OBM output is unresolved: {reason:?}");
    }
    let default_assignment = firmware_default(context, assignment);
    if default_assignment && analog_default(context, &observed)? { return Ok(None) }
    let dkm = ui::dkm_inputs(context.product).contains(&assignment.input_id.as_str());
    let mut row = empty_mapping(assignment);
    if dkm { row.dkm_input = Some(assignment.input_id.clone()); }
    if let Some(output) = converted_output(context, assignment)? {
        row.output = output;
    } else if let Some(default) = ui::mappings(context.product, context.layout)?.find(|row|
        row.input_id == assignment.input_id && row.is_hyper_shift.as_ref().is_some_and(ui::SourceFlag::enabled) == (assignment.mode == Mode::Hypershift))
    {
        row = default.clone();
    } else {
        row.output.output_type = Some(ui::OutputType::DefaultGroup);
    }
    if !dkm && let Some(actuation) = observed.actuation {
        let is_v3 = ui::analog(context.product).is_some();
        let trigger = if is_v3 { Some(observed.rapid_trigger.context("OBM analog rapid trigger is unobserved")?) } else { None };
        if let Some(trigger) = trigger { ensure!(trigger.enabled <= 1, "OBM rapid trigger has an unknown enable value"); }
        let factory_preset = ui::analog(context.product).is_some_and(|values|
            values.default_preset_actuation.iter().any(|(guid, _)| guid == context.profile_guid));
        let rapid_trigger = trigger.filter(|value| value.enabled != 0 && !factory_preset).map(|value| {
            let units = |raw: u16| {
                let raw = f64::from(raw) / 1638.;
                if matches!(context.product, 721 | 756) { raw } else { (raw * 10.).round() / 10. }
            };
            ui::UiRapidTrigger {
                is_enable_up_stroke: value.down_sensitivity != value.up_sensitivity && value.up_sensitivity != value.down_sensitivity.saturating_sub(1638).max(256),
                make_sensitivity: units(value.down_sensitivity), break_sensitivity: units(value.up_sensitivity), continuous: true,
            }
        });
        if default_assignment && is_v3 { row.output.keyboard_group = Some(ui::Keyboard::default()); }
        row.input_type = ui::InputType::AnalogInput;
        row.mapping = Some(vec![
            ui::Stage { output: std::mem::take(&mut row.output), actuation_point: [(0, actuation.threshold_low), (1, if is_v3 { 256 } else { actuation.threshold_high })].into(), rapid_trigger },
            ui::Stage { output: ui::Output { output_type: Some(ui::OutputType::DefaultGroup), ..Default::default() }, actuation_point: [(0, context.second_stage[0]), (1, context.second_stage[1])].into(), rapid_trigger: None },
        ]);
    }
    Ok(Some(row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{onboard_assignment::ButtonAssignment, onboard_macro::{MacroInventory, MacroStorageInfo}};

    fn catalog() -> MacroCatalog {
        MacroCatalog { inventory: MacroInventory { storage: MacroStorageInfo { max_macro_id: 1024, max_memory: 10000, available_memory: 10000, recyclable_memory: 0 }, count: 0, ids: vec![], pages: vec![] }, macros: vec![] }
    }
    fn assignment(product: u32, function_id: u8, data: [u8; 5]) -> wire::Assignment {
        wire::decode_for(product, 0, &ButtonAssignment { key_id: 31, mode: 0, function_id, data_size: 5, data }, &catalog()).unwrap()
    }
    fn context(product: u32, macros: &MacroCatalog) -> Context<'_> {
        Context { product, layout: 0, profile_guid: "local-guid", macros, second_stage: [65520, 63882] }
    }

    #[test]
    fn observed_default_is_omitted_but_unknown_is_not_replaced() {
        let macros = catalog();
        let context = context(550, &macros);
        let observed = assignment(550, 2, [0, 4, 0, 0, 0]);
        assert!(convert(&context, Observation { assignment: &observed, actuation: None, rapid_trigger: None }).unwrap().is_none());
        let unknown = assignment(550, 255, [0; 5]);
        assert!(convert(&context, Observation { assignment: &unknown, actuation: None, rapid_trigger: None }).is_err());
    }

    #[test]
    fn remapped_key_uses_current_ui_key_and_modifier_names() {
        let macros = catalog();
        let observed = assignment(550, 2, [3, 5, 0, 0, 0]);
        let row = convert(&context(550, &macros), Observation { assignment: &observed, actuation: None, rapid_trigger: None }).unwrap().unwrap();
        let group = row.output.keyboard_group.unwrap();
        assert_eq!(group.key, Some(ui::Key::Named("KEY_B".into())));
        assert_eq!(group.modifiers, Some(ui::Modifiers::Names(vec!["KEY_LEFT_CTRL".into(), "KEY_LEFT_SHIFT".into()])));
    }

    #[test]
    fn analog_observation_preserves_product_precision_and_second_stage() {
        let macros = catalog();
        for product in [679, 721, 756] {
            let observed = assignment(product, 2, [0, 4, 0, 0, 0]);
            let actuation = ActuationPoint { key_id: 31, threshold_low: 22000, threshold_high: 20000 };
            let rapid = RapidTriggerKey { key_id: 31, enabled: 1, down_sensitivity: 2000, up_sensitivity: 800 };
            let row = convert(&context(product, &macros), Observation { assignment: &observed, actuation: Some(&actuation), rapid_trigger: Some(&rapid) }).unwrap().unwrap();
            let stages = row.mapping.unwrap();
            assert_eq!(stages[0].actuation_point, [(0, 22000), (1, 256)].into());
            assert_eq!(stages[1].actuation_point, [(0, 65520), (1, 63882)].into());
            assert_eq!(stages[0].output.keyboard_group, Some(ui::Keyboard::default()));
            let trigger = stages[0].rapid_trigger.as_ref().unwrap();
            assert_eq!(trigger.make_sensitivity, if product == 679 { 1.2 } else { 2000. / 1638. });
            assert!(trigger.is_enable_up_stroke);
        }
    }

    #[test]
    fn failed_or_unknown_analog_read_does_not_supply_a_default() {
        let macros = catalog();
        let observed = assignment(679, 2, [0, 4, 0, 0, 0]);
        assert!(convert(&context(679, &macros), Observation { assignment: &observed, actuation: None, rapid_trigger: None }).is_err());
        let actuation = ActuationPoint { key_id: 31, threshold_low: 22000, threshold_high: 20000 };
        let rapid = RapidTriggerKey { key_id: 31, enabled: 2, down_sensitivity: 2000, up_sensitivity: 800 };
        assert!(convert(&context(679, &macros), Observation { assignment: &observed, actuation: Some(&actuation), rapid_trigger: Some(&rapid) }).is_err());
    }
}
