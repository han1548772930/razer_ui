//! Current mapping-to-firmware requirements, independent of UI and transport.
//!
//! The ordering follows current mouse ET and keyboard equivalents. The macro
//! predicate follows current module 3466.Xq, including its appEngine gate.
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::sync::OnceLock;

mod mouse_capabilities;

pub fn current_mouse_mapping_capabilities(product_id: u32) -> Option<&'static MappingCapabilities> {
    static CAPABILITIES: OnceLock<Vec<(u32, MappingCapabilities)>> = OnceLock::new();
    CAPABILITIES
        .get_or_init(mouse_capabilities::current_mouse_capabilities)
        .iter()
        .find(|(product, _)| *product == product_id)
        .map(|(_, capabilities)| capabilities)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MappingCategory {
    Mouse,
    MousePlusMat,
    Keyboard,
    System,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum MappingSupport {
    Enabled(bool),
    Assignments(Vec<String>),
}

impl MappingSupport {
    fn source_truthy(&self) -> bool {
        match self {
            Self::Enabled(enabled) => *enabled,
            Self::Assignments(_) => true,
        }
    }

    fn contains(&self, assignment: Option<&str>) -> bool {
        matches!(self, Self::Assignments(ids) if assignment.is_some_and(|id| ids.iter().any(|candidate| candidate == id)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
struct FirmwareMappingSupport {
    #[serde(rename = "controllerGroup")]
    controller: Option<MappingSupport>,
    #[serde(rename = "macroGroup")]
    macros: Option<MappingSupport>,
    #[serde(rename = "mouseGroup")]
    mouse: Option<MappingSupport>,
    #[serde(rename = "scrollingGroup")]
    scrolling: Option<MappingSupport>,
    #[serde(rename = "sensitivityGroup")]
    sensitivity: Option<MappingSupport>,
    #[serde(rename = "profileNavigationGroup")]
    profile_navigation: Option<MappingSupport>,
}

/// Fixed capabilities observed in the current product configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingCapabilities {
    category: MappingCategory,
    is_obm_device: Option<bool>,
    is_xena_t2_series: Option<bool>,
    no_firmware_input_ids: Option<Vec<String>>,
    supported: FirmwareMappingSupport,
    ble_macros_available: Option<bool>,
    has_controller_assignments_without_synapse: bool,
    required_mouse: Option<Vec<String>>,
    required_turbo_mouse: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SourceConfig {
    #[serde(rename = "DeviceInfo")]
    device_info: SourceDeviceInfo,
    #[serde(rename = "MOUSE_FUNC_REQUIRED_SYNAPSE_VALUES")]
    required_mouse: Option<Vec<String>>,
    #[serde(rename = "MOUSE_TURBO_FUNC_REQUIRED_SYNAPSE_VALUES")]
    required_turbo_mouse: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SourceDeviceInfo {
    category: String,
    #[serde(rename = "isOBMDevice")]
    is_obm_device: Option<bool>,
    #[serde(rename = "isXenaT2Series")]
    is_xena_t2_series: Option<bool>,
    #[serde(rename = "OBMSpecs")]
    obm_specs: Option<SourceObmSpecs>,
    ble: Option<SourceBle>,
    #[serde(rename = "controllerAssignmentsWithoutSynapse")]
    controller_assignments_without_synapse: Option<Value>,
}

#[derive(Deserialize)]
struct SourceObmSpecs {
    #[serde(rename = "noFirmwareInputId")]
    no_firmware_input_ids: Option<Vec<String>>,
    #[serde(rename = "supportedMappings")]
    supported: Option<FirmwareMappingSupport>,
}

#[derive(Deserialize)]
struct SourceBle {
    #[serde(rename = "obmMacrosAvailable")]
    macros_available: Option<bool>,
}

impl MappingCapabilities {
    /// Boundary for product configurations that have not yet been emitted as
    /// ordinary typed Rust tables. Unknown extension fields remain outside this
    /// decision; invalid types in its consumed fields are errors.
    pub fn from_source_config(config: &Value) -> Result<Self> {
        let source: SourceConfig =
            serde_json::from_value(config.clone()).context("Invalid mapping capabilities")?;
        let info = source.device_info;
        let category = match info.category.as_str() {
            "MOUSE" => MappingCategory::Mouse,
            "MOUSEPLUSMAT" => MappingCategory::MousePlusMat,
            "KEYBOARD" => MappingCategory::Keyboard,
            "SYSTEM" => MappingCategory::System,
            _ => MappingCategory::Other(info.category),
        };
        let (no_firmware_input_ids, supported) = match info.obm_specs {
            Some(specs) => (
                specs.no_firmware_input_ids,
                specs.supported.unwrap_or_default(),
            ),
            None => (None, FirmwareMappingSupport::default()),
        };
        Ok(Self {
            category,
            is_obm_device: info.is_obm_device,
            is_xena_t2_series: info.is_xena_t2_series,
            no_firmware_input_ids,
            supported,
            ble_macros_available: info.ble.and_then(|ble| ble.macros_available),
            has_controller_assignments_without_synapse: info
                .controller_assignments_without_synapse
                .is_some(),
            required_mouse: source.required_mouse,
            required_turbo_mouse: source.required_turbo_mouse,
        })
    }

    pub fn requires_synapse(
        &self,
        mapping: &MappingRequirement,
        observations: &MappingObservations<'_>,
        secondary: bool,
    ) -> bool {
        if secondary {
            return true;
        }
        if self.is_xena_t2_series == Some(true) && mapping.input_id == "DKM_SB_247" {
            return true;
        }
        if self.is_obm_device != Some(true) {
            return matches!(
                self.category,
                MappingCategory::Keyboard | MappingCategory::System
            );
        }
        if matches!(
            mapping.output,
            MappingOutput::HyperShift | MappingOutput::Disabled
        ) {
            return false;
        }
        if self
            .no_firmware_input_ids
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|input| input == &mapping.input_id))
        {
            return true;
        }
        if self
            .supported
            .controller
            .as_ref()
            .is_some_and(MappingSupport::source_truthy)
        {
            return false;
        }
        if let MappingOutput::Macro(payload) = &mapping.output {
            if !self.firmware_macros()
                || observations.bluetooth == Some(true) && self.ble_macros_available != Some(true)
                || payload
                    .as_ref()
                    .is_some_and(|payload| payload.playback.as_deref() == Some("Queue"))
            {
                return true;
            }
        }
        if matches!(mapping.output, MappingOutput::Controller(_))
            && self.has_controller_assignments_without_synapse
        {
            return false;
        }
        match &mapping.output {
            MappingOutput::Keyboard(present) => !present,
            MappingOutput::Mouse(Some(payload)) => self.mouse_requires_synapse(payload),
            MappingOutput::Scrolling(Some(payload)) => !self
                .supported
                .scrolling
                .as_ref()
                .is_some_and(|support| support.contains(payload.assignment.as_deref())),
            MappingOutput::Sensitivity(Some(payload)) => !self
                .supported
                .sensitivity
                .as_ref()
                .is_some_and(|support| support.contains(payload.assignment.as_deref())),
            MappingOutput::ProfileNavigation(Some(payload)) => !self
                .supported
                .profile_navigation
                .as_ref()
                .is_some_and(|support| support.contains(payload.assignment.as_deref())),
            MappingOutput::Macro(Some(payload)) => {
                self.macro_requires_synapse(payload.guid.as_deref(), observations.macros)
            }
            MappingOutput::Multimedia(Some(payload)) => matches!(
                payload.assignment.as_deref(),
                Some("MicVolumeUp" | "MicVolumeDown" | "MuteAll" | "MuteMic")
            ),
            _ => true,
        }
    }

    fn firmware_macros(&self) -> bool {
        self.supported
            .macros
            .as_ref()
            .is_some_and(MappingSupport::source_truthy)
    }

    pub fn macro_requires_synapse(&self, guid: Option<&str>, macros: &[MacroRequirement]) -> bool {
        if !self.firmware_macros() {
            return true;
        }
        macros
            .iter()
            .find(|entry| Some(entry.guid.as_str()) == guid)
            .is_some_and(MacroRequirement::requires_synapse)
    }

    fn mouse_requires_synapse(&self, payload: &MouseRequirement) -> bool {
        let turbo = payload.turbo_mode.as_ref().and_then(|turbo| turbo.enabled);
        if payload.assignment.as_deref() == Some("DoubleClick") && turbo == Some(true) {
            return true;
        }
        let configured = if turbo == Some(true) {
            &self.required_turbo_mouse
        } else {
            &self.required_mouse
        };
        if let Some(ids) = configured.as_ref().filter(|ids| !ids.is_empty()) {
            return payload
                .assignment
                .as_ref()
                .is_some_and(|id| ids.contains(id));
        }
        const SOFTWARE_MOUSE_ASSIGNMENTS: &[&str] = &[
            "ScrollLeft",
            "ScrollRight",
            "RepeatScrollUp",
            "RepeatScrollDown",
            "RepeatScrollLeft",
            "RepeatScrollRight",
            "BossKey",
            "ScrollUp",
            "ScrollDown",
        ];
        payload.assignment.as_deref().is_some_and(|assignment| {
            SOFTWARE_MOUSE_ASSIGNMENTS.contains(&assignment)
                && !self
                    .supported
                    .mouse
                    .as_ref()
                    .is_some_and(|support| support.contains(Some(assignment)))
        })
    }
}

/// Runtime facts consumed by the source predicate, without owning I/O.
pub struct MappingObservations<'a> {
    bluetooth: Option<bool>,
    macros: &'a [MacroRequirement],
}

impl<'a> MappingObservations<'a> {
    pub fn new(bluetooth: Option<bool>, macros: &'a [MacroRequirement]) -> Self {
        Self { bluetooth, macros }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingRequirement {
    input_id: String,
    output: MappingOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MappingOutput {
    Keyboard(bool),
    Mouse(Option<MouseRequirement>),
    Scrolling(Option<AssignmentRequirement>),
    Sensitivity(Option<AssignmentRequirement>),
    ProfileNavigation(Option<AssignmentRequirement>),
    Macro(Option<MacroMappingRequirement>),
    Multimedia(Option<AssignmentRequirement>),
    Controller(bool),
    HyperShift,
    Disabled,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssignmentRequirement {
    assignment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct MouseRequirement {
    #[serde(rename = "mouseAssignment")]
    assignment: Option<String>,
    #[serde(rename = "turboMode")]
    turbo_mode: Option<TurboRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TurboRequirement {
    #[serde(rename = "isTurbo")]
    enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct MacroMappingRequirement {
    guid: Option<String>,
    #[serde(rename = "macroPlaybackOption")]
    playback: Option<String>,
}

impl MappingRequirement {
    /// `input_id` belongs to the enclosing mapping when the source stores a
    /// multi-assignment array; inner rows do not have to repeat it.
    pub fn from_source_mapping(mapping: &Value, input_id: &str) -> Result<Self> {
        let object = mapping.as_object().context("Mapping must be an object")?;
        let output = object
            .get("outputType")
            .and_then(Value::as_str)
            .context("Mapping outputType must be a string")?;
        let payload = object.get(output).filter(|value| !value.is_null());
        fn assignment(
            payload: Option<&Value>,
            field: &str,
        ) -> Result<Option<AssignmentRequirement>> {
            payload
                .map(|payload| {
                    let object = payload
                        .as_object()
                        .context("Mapping payload must be an object")?;
                    let assignment = object
                        .get(field)
                        .filter(|value| !value.is_null())
                        .map(|value| {
                            value
                                .as_str()
                                .map(str::to_owned)
                                .context("Mapping assignment must be a string")
                        })
                        .transpose()?;
                    Ok(AssignmentRequirement { assignment })
                })
                .transpose()
        }
        let output = match output {
            "keyboardGroup" => MappingOutput::Keyboard(payload.is_some()),
            "mouseGroup" => MappingOutput::Mouse(
                payload
                    .map(|payload| {
                        let source: MouseRequirement = serde_json::from_value(payload.clone())
                            .context("Invalid mouse mapping requirement")?;
                        Ok::<_, anyhow::Error>(source)
                    })
                    .transpose()?,
            ),
            "scrollingGroup" => {
                MappingOutput::Scrolling(assignment(payload, "scrollingAssignment")?)
            }
            "sensitivityGroup" => {
                MappingOutput::Sensitivity(assignment(payload, "sensitivityAssignment")?)
            }
            "profileNavigationGroup" => MappingOutput::ProfileNavigation(assignment(
                payload,
                "profileNavigationAssignment",
            )?),
            "macroGroup" => MappingOutput::Macro(
                payload
                    .map(|payload| {
                        serde_json::from_value(payload.clone())
                            .context("Invalid macro mapping requirement")
                    })
                    .transpose()?,
            ),
            "multimediaGroup" => {
                MappingOutput::Multimedia(assignment(payload, "multimediaAssignment")?)
            }
            "controllerGroup" => MappingOutput::Controller(payload.is_some()),
            "hyperShiftGroup" => MappingOutput::HyperShift,
            "disableGroup" => MappingOutput::Disabled,
            output => MappingOutput::Other(output.to_owned()),
        };
        Ok(Self {
            input_id: input_id.into(),
            output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn capabilities(info: Value, overrides: Value) -> MappingCapabilities {
        let mut config = overrides;
        config["DeviceInfo"] = info;
        MappingCapabilities::from_source_config(&config).unwrap()
    }

    fn obm(supported: Value) -> MappingCapabilities {
        capabilities(
            json!({
                "category":"MOUSE", "isOBMDevice":true,
                "OBMSpecs":{"noFirmwareInputId":[],"supportedMappings":supported}
            }),
            json!({}),
        )
    }

    fn requirement(output: &str, payload: Value) -> MappingRequirement {
        MappingRequirement::from_source_mapping(
            &json!({"outputType":output, output:payload}),
            "RightClick",
        )
        .unwrap()
    }

    fn evaluate(caps: &MappingCapabilities, output: &str, payload: Value) -> bool {
        caps.requires_synapse(
            &requirement(output, payload),
            &MappingObservations::new(None, &[]),
            false,
        )
    }

    #[test]
    fn source_order_preserves_non_obm_exemptions_and_secondary_assignment() {
        let mouse = capabilities(json!({"category":"MOUSE"}), json!({}));
        let keyboard = capabilities(json!({"category":"KEYBOARD"}), json!({}));
        let disabled = requirement("disableGroup", Value::Null);
        let observations = MappingObservations::new(None, &[]);
        assert!(!mouse.requires_synapse(&disabled, &observations, false));
        assert!(keyboard.requires_synapse(&disabled, &observations, false));
        assert!(mouse.requires_synapse(&disabled, &observations, true));
        let mut device = obm(json!({"controllerGroup":true}));
        device.no_firmware_input_ids = Some(vec!["RightClick".into()]);
        assert!(evaluate(&device, "keyboardGroup", json!({})));
        assert!(!device.requires_synapse(&disabled, &observations, false));
        device.is_xena_t2_series = Some(true);
        let xena = MappingRequirement::from_source_mapping(
            &json!({"outputType":"disableGroup"}),
            "DKM_SB_247",
        )
        .unwrap();
        assert!(device.requires_synapse(&xena, &observations, false));
    }

    #[test]
    fn controller_support_precedes_software_groups_and_absent_payloads() {
        let device = obm(json!({"controllerGroup":true}));
        assert!(!evaluate(
            &device,
            "macroGroup",
            json!({"macroPlaybackOption":"Queue"})
        ));
        assert!(!evaluate(&device, "unknownGroup", Value::Null));
        let mut device = obm(json!({}));
        assert!(evaluate(&device, "controllerGroup", Value::Null));
        device.has_controller_assignments_without_synapse = true;
        assert!(!evaluate(&device, "controllerGroup", Value::Null));
        assert!(!evaluate(&device, "keyboardGroup", json!({})));
        assert!(evaluate(&device, "keyboardGroup", Value::Null));
    }

    #[test]
    fn mouse_membership_and_turbo_overrides_follow_current_zr() {
        let mut device = obm(json!({"mouseGroup":["ScrollUp"]}));
        assert!(!evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"ScrollUp"})
        ));
        assert!(evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"ScrollDown"})
        ));
        assert!(!evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"Click"})
        ));
        device.required_mouse = Some(vec!["Click".into()]);
        device.required_turbo_mouse = Some(vec!["Menu".into()]);
        assert!(evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"Click"})
        ));
        assert!(!evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"Click","turboMode":{"isTurbo":true}})
        ));
        assert!(evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"Menu","turboMode":{"isTurbo":true}})
        ));
        assert!(evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"DoubleClick","turboMode":{"isTurbo":true}})
        ));
        device.required_mouse = Some(vec![]);
        assert!(evaluate(
            &device,
            "mouseGroup",
            json!({"mouseAssignment":"ScrollDown"})
        ));
    }

    #[test]
    fn firmware_assignments_and_multimedia_mic_values_match_source_predicates() {
        let device = obm(json!({
            "sensitivityGroup":["DPI_Up"], "scrollingGroup":["ScrollModeSwitch"],
            "profileNavigationGroup":["Next"]
        }));
        assert!(!evaluate(
            &device,
            "sensitivityGroup",
            json!({"sensitivityAssignment":"DPI_Up"})
        ));
        assert!(evaluate(
            &device,
            "sensitivityGroup",
            json!({"sensitivityAssignment":"DPI_CycleUp"})
        ));
        assert!(!evaluate(
            &device,
            "scrollingGroup",
            json!({"scrollingAssignment":"ScrollModeSwitch"})
        ));
        assert!(!evaluate(
            &device,
            "profileNavigationGroup",
            json!({"profileNavigationAssignment":"Next"})
        ));
        for id in ["MicVolumeUp", "MicVolumeDown", "MuteAll", "MuteMic"] {
            assert!(evaluate(
                &device,
                "multimediaGroup",
                json!({"multimediaAssignment":id})
            ));
        }
        assert!(!evaluate(
            &device,
            "multimediaGroup",
            json!({"multimediaAssignment":"VolumeUp"})
        ));
    }

    #[test]
    fn macros_retain_engine_gate_event_values_bluetooth_and_queue_order() {
        let macros = MacroRequirement::from_source_catalog(&[
            json!({"guid":"no-engine","macroEvents":[{"Type":6}]}),
            json!({"guid":"relative","appEngine":{"mouseMoveType":"relative"},"macroEvents":[{"Type":1}]}),
            json!({"guid":"absolute","appEngine":{"mouseMoveType":"absolute"}}),
            json!({"guid":"event","appEngine":{},"macroEvents":[{"Type":10}]}),
            json!({"guid":"button","appEngine":{},"macroEvents":[{"MouseEvent":{"MouseButton":9}}]}),
        ]).unwrap();
        let device = obm(json!({"macroGroup":true}));
        let observations = MappingObservations::new(None, &macros);
        for (guid, expected) in [
            ("missing", false),
            ("no-engine", false),
            ("relative", false),
            ("absolute", true),
            ("event", true),
            ("button", true),
        ] {
            let mapping = requirement(
                "macroGroup",
                json!({"guid":guid,"macroPlaybackOption":"Once"}),
            );
            assert_eq!(
                device.requires_synapse(&mapping, &observations, false),
                expected,
                "{guid}"
            );
        }
        let mapping = requirement(
            "macroGroup",
            json!({"guid":"relative","macroPlaybackOption":"Once"}),
        );
        assert!(device.requires_synapse(
            &mapping,
            &MappingObservations::new(Some(true), &macros),
            false
        ));
        let queued = requirement(
            "macroGroup",
            json!({"guid":"relative","macroPlaybackOption":"Queue"}),
        );
        assert!(device.requires_synapse(&queued, &observations, false));
        assert!(obm(json!({})).requires_synapse(&mapping, &observations, false));
    }

    #[test]
    fn invalid_consumed_fields_are_errors_and_missing_runtime_values_stay_optional() {
        assert!(
            MappingCapabilities::from_source_config(
                &json!({"DeviceInfo":{"category":"MOUSE","isOBMDevice":"true"}})
            )
            .is_err()
        );
        assert!(
            MappingRequirement::from_source_mapping(
                &json!({"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":12}}),
                "RightClick"
            )
            .is_err()
        );
        assert!(
            MacroRequirement::from_source_catalog(&[
                json!({"guid":"bad","appEngine":{},"macroEvents":[{"Type":"6"}]})
            ])
            .is_err()
        );
        assert!(MacroRequirement::from_source_catalog(&[json!({"guid":"unobserved"})]).is_ok());
    }

    #[test]
    fn current_mouse_capabilities_cover_registry_and_keep_firmware_membership_branches() {
        let current = mouse_capabilities::current_mouse_capabilities();
        assert_eq!(current.len(), 75);
        for pair in current.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "Duplicate or unordered product identity"
            );
        }
        for (pid, _) in current {
            assert!(
                razer_catalog::registered(pid).is_some(),
                "Unregistered product {pid}"
            );
        }
        assert!(current_mouse_mapping_capabilities(162).is_none());
        let viper = current_mouse_mapping_capabilities(229).unwrap();
        assert!(!evaluate(
            viper,
            "sensitivityGroup",
            json!({"sensitivityAssignment":"DPI_CycleUp"})
        ));
        assert!(evaluate(
            viper,
            "mouseGroup",
            json!({"mouseAssignment":"ScrollUp"})
        ));
        let naga = current_mouse_mapping_capabilities(235).unwrap();
        assert!(!evaluate(
            naga,
            "mouseGroup",
            json!({"mouseAssignment":"ScrollUp"})
        ));
        assert!(evaluate(
            naga,
            "mouseGroup",
            json!({"mouseAssignment":"ScrollLeft"})
        ));
        assert!(!evaluate(
            naga,
            "profileNavigationGroup",
            json!({"profileNavigationAssignment":"CycleUp"})
        ));
    }
}

/// The subset of an observed macro that participates in current 3466.Xq.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MacroRequirement {
    guid: String,
    #[serde(rename = "appEngine")]
    app_engine: Option<MacroEngineRequirement>,
    #[serde(rename = "macroEvents")]
    events: Option<Vec<MacroEventRequirement>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct MacroEngineRequirement {
    #[serde(rename = "mouseMoveType")]
    mouse_move: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct MacroEventRequirement {
    #[serde(rename = "Type")]
    event_type: Option<u32>,
    #[serde(rename = "MouseEvent")]
    mouse: Option<MacroMouseRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct MacroMouseRequirement {
    #[serde(rename = "MouseButton")]
    button: Option<u32>,
}

impl MacroRequirement {
    pub fn from_source_catalog(catalog: &[Value]) -> Result<Vec<Self>> {
        catalog
            .iter()
            .map(|entry| {
                serde_json::from_value(entry.clone())
                    .context("Invalid macro requirement observation")
            })
            .collect()
    }

    fn requires_synapse(&self) -> bool {
        let Some(engine) = &self.app_engine else {
            return false;
        };
        if matches!(
            engine.mouse_move.as_deref(),
            Some("absolute" | "foreground")
        ) {
            return true;
        }
        self.events.as_ref().is_some_and(|events| {
            events.iter().any(|event| {
                matches!(event.event_type, Some(6 | 5 | 4 | 10))
                    || event
                        .mouse
                        .as_ref()
                        .is_some_and(|mouse| matches!(mouse.button, Some(8 | 9)))
            })
        })
    }
}
