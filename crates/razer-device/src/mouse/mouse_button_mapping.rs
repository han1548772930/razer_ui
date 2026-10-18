//! Current firmware mouse button assignments for the four audited DualLink
//! mouse products (170, 204, 226 and 45066).
//!
//! The web engine receives a firmware button ID from its UI mapping table. It
//! does not use the shared `getButtonIDList` method for this mouse engine, so
//! this API deliberately requires the caller to provide that observed ID. The
//! encoder is independent of the UI and of local JSON drafts.
pub use crate::onboard_assignment::MacroPlayback;
use crate::{
    backend::FeatureTransport, device_reads::DeviceReadCapability,
    feature_exchange::exchange_with_report_data_size_observing_send,
};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseMappingDescriptor {
    pub product_id: u32,
    pub function_ids: &'static [u8],
    pub consumer_codes: &'static [u16],
    pub razer_codes: &'static [u8],
    pub windows_codes: &'static [u8],
    pub power_codes: &'static [u8],
}

include!("mouse_button_mapping_data_generated.rs");

const GET_ASSIGNMENT: [u8; 3] = [80, 2, 140];
const SET_ASSIGNMENT: [u8; 3] = [80, 2, 12];
const REPORT_DATA_SIZE: u8 = 10;

const FN_OFF: u8 = 0;
const FN_BUTTON: u8 = 1;
const FN_DPI: u8 = 6;
const FN_PROFILE: u8 = 7;
const FN_POWER: u8 = 9;
const FN_CONSUMER: u8 = 10;
const FN_DOUBLE_CLICK: u8 = 11;
const FN_HYPERSHIFT: u8 = 12;
const FN_TURBO_BUTTON: u8 = 14;
const FN_RAZER: u8 = 17;
const FN_SCROLL_MODE: u8 = 18;
const FN_WINDOWS: u8 = 21;
const FN_PERFECT_180: u8 = 22;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum AssignmentMode {
    Normal,
    Hypershift,
}

impl AssignmentMode {
    pub fn raw(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Hypershift => 1,
        }
    }

    fn from_raw(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Hypershift),
            _ => anyhow::bail!("Unknown mouse button assignment mode {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

impl MouseButton {
    fn raw(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Middle => 3,
            Self::Button4 => 4,
            Self::Button5 => 5,
            Self::ScrollUp => 9,
            Self::ScrollDown => 10,
            Self::ScrollLeft => 104,
            Self::ScrollRight => 105,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MouseAction {
    Button(MouseButton),
    TurboButton {
        button: MouseButton,
        /// Source uses floor(1000 / keysPerSecond). A missing value is the
        /// source default of seven activations per second.
        keys_per_second: Option<u16>,
    },
    RepeatScroll(MouseButton),
    DoubleClick,
    /// The source checks the literal string "true" for this legacy branch.
    /// An ordinary boolean `true` still encodes DoubleClick.
    LegacyStringTurboDoubleClick,
    ScrollModeSwitch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct KeyboardAction {
    pub hid: u8,
    /// Source modifier encoding: Ctrl=1, Shift=2, Alt=4, GUI=8 and the
    /// corresponding right-hand values 16, 32, 64 and 128.
    pub modifiers: u8,
    pub turbo: bool,
    pub keys_per_second: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum DpiAction {
    Up,
    Down,
    Clutch { x: u16, y: u16 },
    CycleUp,
    CycleDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MacroAction {
    /// ID observed in the device's firmware macro inventory. A local draft or
    /// GUID is not a firmware ID. Zero is a valid source-generated ID; the
    /// caller must supply an ID from real inventory, not infer it from a draft.
    pub macro_id: u16,
    pub playback: MacroPlayback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProfileAction {
    Up,
    Down,
    Specific(u8),
    CycleUp,
    CycleDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ScrollModeAction {
    Up,
    Down,
    CycleUp,
    CycleDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Perfect180Direction {
    Right,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonAction {
    Disabled,
    Mouse(MouseAction),
    Keyboard(KeyboardAction),
    Dpi(DpiAction),
    Macro(MacroAction),
    Profile(ProfileAction),
    HyperShift,
    Consumer(ConsumerAction),
    Razer(RazerKey),
    Power(PowerAction),
    ScrollMode(ScrollModeAction),
    Windows(WindowsShortcut),
    Perfect180 {
        direction: Perfect180Direction,
        report_count: u16,
        enabled: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ButtonTarget {
    pub profile_id: u8,
    /// This is the real firmware ID obtained from the caller's observed UI
    /// mapping list, not an array index or a locally invented button number.
    pub button_id: u8,
    pub mode: AssignmentMode,
}

impl ButtonTarget {
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.profile_id > 0,
            "Mouse mapping profile ID cannot be zero"
        );
        ensure!(
            self.button_id > 0,
            "Mouse mapping firmware button ID cannot be zero"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ButtonAssignment {
    pub target: ButtonTarget,
    pub function_id: u8,
    pub data_size: u8,
    pub data: [u8; 5],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ObservedButtonAssignment {
    pub assignment: ButtonAssignment,
    /// The fixed five source bytes are retained even when `data_size` is less.
    pub raw_data: [u8; 5],
    /// Complete returned payload, including unknown fields and trailing bytes.
    pub raw_payload: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ButtonWriteStep {
    pub target: ButtonTarget,
    pub requested: ButtonAssignment,
    pub attempted: bool,
    pub acknowledgment_received: bool,
    pub acknowledgment_data: Vec<u8>,
    pub observed: Option<ObservedButtonAssignment>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ButtonWriteReport {
    pub steps: Vec<ButtonWriteStep>,
    pub source_completed: bool,
    verified: bool,
}

impl ButtonWriteReport {
    pub fn verified(&self) -> bool {
        self.verified
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ButtonFailurePhase {
    Prepare,
    Write,
    Readback,
    Cancelled,
}

#[derive(Debug)]
pub struct ButtonWriteFailure {
    pub phase: ButtonFailurePhase,
    pub progress: ButtonWriteReport,
    pub failed_step: Option<usize>,
    error: anyhow::Error,
}

impl ButtonWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.progress.steps.iter().any(|step| step.attempted)
    }
}

impl std::fmt::Display for ButtonWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse button mapping {:?} failed: {:#}",
            self.phase, self.error
        )
    }
}

impl std::error::Error for ButtonWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

fn descriptor(product_id: u32) -> Result<&'static MouseMappingDescriptor> {
    AUDITED_MAPPING_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.product_id == product_id)
        .context("Mouse button mapping has no current product descriptor")
}

/// Return the compile-time typed lookup descriptor for a source-audited mouse.
pub fn capability(product_id: u32) -> Option<&'static MouseMappingDescriptor> {
    AUDITED_MAPPING_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.product_id == product_id)
}

fn ensure_function(descriptor: &MouseMappingDescriptor, function_id: u8) -> Result<()> {
    ensure!(
        descriptor.function_ids.contains(&function_id),
        "Function {function_id} is not source-gated for this mouse product"
    );
    Ok(())
}

fn period_ms(keys_per_second: Option<u16>) -> Result<u16> {
    let rate = keys_per_second.unwrap_or(7);
    ensure!(rate > 0, "Turbo keys-per-second must be greater than zero");
    Ok(1000 / rate)
}

fn be_u16(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

/// Encode the exact five-byte firmware data field and meaningful data size.
pub fn encode(product_id: u32, action: ButtonAction) -> Result<(u8, u8, [u8; 5])> {
    let descriptor = descriptor(product_id)?;
    let mut data = [0; 5];
    let (function_id, data_size) = match action {
        ButtonAction::Disabled => (FN_OFF, 0),
        ButtonAction::Mouse(action) => match action {
            MouseAction::Button(button) => {
                data[0] = button.raw();
                (FN_BUTTON, 1)
            }
            MouseAction::TurboButton {
                button,
                keys_per_second,
            } => {
                let period = be_u16(period_ms(keys_per_second)?);
                data[0] = button.raw();
                data[1..3].copy_from_slice(&period);
                (FN_TURBO_BUTTON, 3)
            }
            MouseAction::RepeatScroll(button) => {
                ensure!(
                    matches!(
                        button,
                        MouseButton::ScrollUp
                            | MouseButton::ScrollDown
                            | MouseButton::ScrollLeft
                            | MouseButton::ScrollRight
                    ),
                    "RepeatScroll requires a scroll direction"
                );
                data[0] = button.raw();
                let period = be_u16(period_ms(None)?);
                data[1..3].copy_from_slice(&period);
                (FN_TURBO_BUTTON, 3)
            }
            MouseAction::DoubleClick => {
                data[0] = 1;
                (FN_DOUBLE_CLICK, 1)
            }
            MouseAction::LegacyStringTurboDoubleClick => (FN_OFF, 0),
            MouseAction::ScrollModeSwitch => {
                data[0] = 1;
                (FN_SCROLL_MODE, 1)
            }
        },
        ButtonAction::Keyboard(action) => {
            let (function_id, size, encoded) = crate::onboard_assignment::keyboard_function(
                action.hid,
                action.modifiers,
                action.turbo,
                action.keys_per_second,
                false,
            )?;
            data = encoded;
            (function_id, size)
        }
        ButtonAction::Dpi(action) => {
            data[0] = match action {
                DpiAction::Up => 1,
                DpiAction::Down => 2,
                DpiAction::Clutch { x, y } => {
                    data[1..3].copy_from_slice(&be_u16(x));
                    data[3..5].copy_from_slice(&be_u16(y));
                    5
                }
                DpiAction::CycleUp => 6,
                DpiAction::CycleDown => 7,
            };
            (
                FN_DPI,
                if matches!(action, DpiAction::Clutch { .. }) {
                    5
                } else {
                    1
                },
            )
        }
        ButtonAction::Macro(action) => {
            if let MacroPlayback::NTimes(count) = action.playback {
                ensure!(count > 0, "Macro repeat count must be greater than zero");
            }
            let (function, size, encoded) =
                crate::onboard_assignment::macro_function(action.macro_id, action.playback);
            data = encoded;
            (function, size)
        }
        ButtonAction::Profile(action) => {
            let size = match action {
                ProfileAction::Up => {
                    data[0] = 1;
                    1
                }
                ProfileAction::Down => {
                    data[0] = 2;
                    1
                }
                ProfileAction::Specific(id) => {
                    ensure!(id > 0, "Specific profile target cannot be zero");
                    data[0] = 3;
                    data[1] = id;
                    2
                }
                ProfileAction::CycleUp => {
                    data[0] = 4;
                    1
                }
                ProfileAction::CycleDown => {
                    data[0] = 5;
                    1
                }
            };
            (FN_PROFILE, size)
        }
        ButtonAction::HyperShift => {
            data[0] = 1;
            (FN_HYPERSHIFT, 1)
        }
        ButtonAction::Consumer(action) => {
            ensure!(
                descriptor.consumer_codes.contains(&(action as u16)),
                "Consumer action is not source-gated"
            );
            data[..2].copy_from_slice(&be_u16(action as u16));
            (FN_CONSUMER, 2)
        }
        ButtonAction::Razer(action) => {
            ensure!(
                descriptor.razer_codes.contains(&(action as u8)),
                "Razer key is not source-gated"
            );
            data[0] = action as u8;
            (FN_RAZER, 1)
        }
        ButtonAction::Power(action) => {
            ensure!(
                descriptor.power_codes.contains(&(action as u8)),
                "Power key is not source-gated"
            );
            data[0] = action as u8;
            (FN_POWER, 1)
        }
        ButtonAction::ScrollMode(action) => {
            data[0] = match action {
                ScrollModeAction::Up => 2,
                ScrollModeAction::Down => 3,
                ScrollModeAction::CycleUp => 4,
                ScrollModeAction::CycleDown => 5,
            };
            (FN_SCROLL_MODE, 1)
        }
        ButtonAction::Windows(action) => {
            ensure!(
                descriptor.windows_codes.contains(&(action as u8)),
                "Windows shortcut is not source-gated"
            );
            data[0] = action as u8;
            (FN_WINDOWS, 1)
        }
        ButtonAction::Perfect180 {
            direction,
            report_count,
            enabled,
        } => {
            data[0] = match direction {
                Perfect180Direction::Right => 0,
                Perfect180Direction::Left => 1,
            };
            data[1..3].copy_from_slice(&be_u16(report_count));
            data[3] = u8::from(enabled);
            (FN_PERFECT_180, 4)
        }
    };
    ensure_function(descriptor, function_id)?;
    Ok((function_id, data_size, data))
}

pub fn assignment(
    product_id: u32,
    target: ButtonTarget,
    action: ButtonAction,
) -> Result<ButtonAssignment> {
    target.validate()?;
    let (function_id, data_size, data) = encode(product_id, action)?;
    Ok(ButtonAssignment {
        target,
        function_id,
        data_size,
        data,
    })
}

fn parse_assignment(
    product_id: u32,
    expected: ButtonTarget,
    data: &[u8],
) -> Result<ObservedButtonAssignment> {
    descriptor(product_id)?;
    expected.validate()?;
    ensure!(
        data.len() >= 10,
        "Mouse button assignment response is truncated"
    );
    let returned = ButtonTarget {
        profile_id: data[0],
        button_id: data[1],
        mode: AssignmentMode::from_raw(data[2])?,
    };
    ensure!(
        returned == expected,
        "Mouse button assignment identity differs"
    );
    let mut raw_data = [0; 5];
    raw_data.copy_from_slice(&data[5..10]);
    Ok(ObservedButtonAssignment {
        assignment: ButtonAssignment {
            target: returned,
            function_id: data[3],
            data_size: data[4],
            data: raw_data,
        },
        raw_data,
        raw_payload: data.to_vec(),
    })
}

pub struct MouseButtonMapping<'a> {
    device: &'a dyn FeatureTransport,
    transport: &'a DeviceReadCapability,
}

impl<'a> MouseButtonMapping<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        transport: &'a DeviceReadCapability,
    ) -> Result<Self> {
        descriptor(transport.product_id)?;
        ensure!(
            transport.source_class == "rzDevice25DualLinkMouse"
                && transport.vendor_id == 0x1532
                && transport.report_id == 0
                && transport.report_bytes == 91
                && transport.transaction_prefix == 0
                && transport.transaction_modulus == 31
                && transport.claim_interface == if transport.product_id == 226 { 3 } else { 0 }
                && transport.claim_interfaces_by_pid.is_empty(),
            "Mouse button mapping requires the audited DualLink Feature transport"
        );
        Ok(Self { device, transport })
    }

    pub fn read(
        &self,
        target: ButtonTarget,
        next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<ObservedButtonAssignment> {
        self.read_observing_send(target, next_transaction, &validate, &mut || {})
    }

    pub(crate) fn read_observing_send(
        &self,
        target: ButtonTarget,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        observe_send: &mut impl FnMut(),
    ) -> Result<ObservedButtonAssignment> {
        target.validate()?;
        validate()?;
        let transaction = next_transaction()?;
        let data = exchange_with_report_data_size_observing_send(
            self.device,
            self.transport,
            GET_ASSIGNMENT,
            &[target.profile_id, target.button_id, target.mode.raw()],
            transaction,
            REPORT_DATA_SIZE,
            validate,
            observe_send,
        )?;
        parse_assignment(self.transport.product_id, target, &data)
    }

    pub fn apply(
        &self,
        target: ButtonTarget,
        action: ButtonAction,
        confirm_with_readback: bool,
        next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ButtonWriteReport, ButtonWriteFailure> {
        let requested = match assignment(self.transport.product_id, target, action) {
            Ok(value) => value,
            Err(error) => {
                return Err(ButtonWriteFailure {
                    phase: ButtonFailurePhase::Prepare,
                    progress: ButtonWriteReport::default(),
                    failed_step: None,
                    error,
                });
            }
        };
        self.apply_assignment(requested, confirm_with_readback, next_transaction, validate)
    }

    /// Restore an independently observed raw firmware assignment to its original
    /// target. This preserves unknown function IDs without claiming a decoder.
    /// Oversized/contradictory observations are rejected before device I/O.
    pub fn restore_assignment(
        &self,
        observed: &ObservedButtonAssignment,
        confirm_with_readback: bool,
        next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ButtonWriteReport, ButtonWriteFailure> {
        let prepared = (|| {
            let assignment = observed.assignment;
            assignment.target.validate()?;
            ensure!(
                assignment.data_size <= 5,
                "Observed mouse mapping exceeds the fixed five-byte field"
            );
            ensure!(
                observed.raw_data == assignment.data,
                "Observed raw assignment data disagrees"
            );
            let expected = [
                assignment.target.profile_id,
                assignment.target.button_id,
                assignment.target.mode.raw(),
                assignment.function_id,
                assignment.data_size,
                assignment.data[0],
                assignment.data[1],
                assignment.data[2],
                assignment.data[3],
                assignment.data[4],
            ];
            ensure!(
                observed.raw_payload.starts_with(&expected),
                "Observed raw assignment payload disagrees"
            );
            let inventory = crate::mouse_button_inventory::capability(self.transport.product_id)
                .context("No current mouse button inventory for raw restore")?;
            ensure!(
                inventory.inputs.iter().any(|input| input.firmware_id
                    == Some(assignment.target.button_id)
                    && input.mode == assignment.target.mode),
                "Observed mouse mapping target is absent from current product inventory"
            );
            Ok(assignment)
        })();
        let requested = prepared.map_err(|error| ButtonWriteFailure {
            phase: ButtonFailurePhase::Prepare,
            progress: ButtonWriteReport::default(),
            failed_step: None,
            error,
        })?;
        self.apply_assignment(requested, confirm_with_readback, next_transaction, validate)
    }

    fn apply_assignment(
        &self,
        requested: ButtonAssignment,
        confirm_with_readback: bool,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ButtonWriteReport, ButtonWriteFailure> {
        let target = requested.target;
        let mut report = ButtonWriteReport {
            steps: vec![ButtonWriteStep {
                target,
                requested,
                attempted: false,
                acknowledgment_received: false,
                acknowledgment_data: Vec::new(),
                observed: None,
            }],
            ..ButtonWriteReport::default()
        };
        if let Err(error) = validate() {
            return Err(ButtonWriteFailure {
                phase: ButtonFailurePhase::Cancelled,
                progress: report,
                failed_step: Some(0),
                error,
            });
        }
        let transaction = match next_transaction() {
            Ok(value) => value,
            Err(error) => {
                return Err(ButtonWriteFailure {
                    phase: ButtonFailurePhase::Write,
                    progress: report,
                    failed_step: Some(0),
                    error,
                });
            }
        };
        let payload = {
            let data = requested.data;
            vec![
                target.profile_id,
                target.button_id,
                target.mode.raw(),
                requested.function_id,
                requested.data_size,
                data[0],
                data[1],
                data[2],
                data[3],
                data[4],
            ]
        };
        let attempted = std::cell::Cell::new(false);
        let cancelled = std::cell::Cell::new(false);
        let guarded = || validate().inspect_err(|_| cancelled.set(true));
        let response = exchange_with_report_data_size_observing_send(
            self.device,
            self.transport,
            SET_ASSIGNMENT,
            &payload,
            transaction,
            REPORT_DATA_SIZE,
            &guarded,
            &mut || attempted.set(true),
        );
        report.steps[0].attempted = attempted.get();
        let response = response.map_err(|error| ButtonWriteFailure {
            phase: if cancelled.get() {
                ButtonFailurePhase::Cancelled
            } else {
                ButtonFailurePhase::Write
            },
            progress: report.clone(),
            failed_step: Some(0),
            error,
        })?;
        report.steps[0].acknowledgment_received = true;
        report.steps[0].acknowledgment_data = response;
        if let Err(error) = validate() {
            return Err(ButtonWriteFailure {
                phase: ButtonFailurePhase::Cancelled,
                progress: report,
                failed_step: Some(0),
                error,
            });
        }
        report.source_completed = true;
        if !confirm_with_readback {
            return Ok(report);
        }
        let observed = match self.read(target, &mut next_transaction, &validate) {
            Ok(value) => value,
            Err(error) => {
                return Err(ButtonWriteFailure {
                    phase: ButtonFailurePhase::Readback,
                    progress: report,
                    failed_step: Some(0),
                    error,
                });
            }
        };
        let matches = same_meaningful_assignment(&requested, &observed.assignment);
        report.steps[0].observed = Some(observed);
        report.verified = matches;
        if !matches {
            return Err(ButtonWriteFailure {
                phase: ButtonFailurePhase::Readback,
                progress: report,
                failed_step: Some(0),
                error: anyhow::anyhow!(
                    "Independent button assignment GET differs from the requested SET"
                ),
            });
        }
        Ok(report)
    }
}

fn same_meaningful_assignment(requested: &ButtonAssignment, observed: &ButtonAssignment) -> bool {
    let requested_len = usize::from(requested.data_size).min(requested.data.len());
    let observed_len = usize::from(observed.data_size).min(observed.data.len());
    requested.target == observed.target
        && requested.function_id == observed.function_id
        && requested.data_size == observed.data_size
        && requested.data[..requested_len] == observed.data[..observed_len]
}

pub fn read(
    device: &dyn FeatureTransport,
    transport: &DeviceReadCapability,
    target: ButtonTarget,
    next_transaction: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> Result<ObservedButtonAssignment> {
    MouseButtonMapping::new(device, transport)?.read(target, next_transaction, validate)
}

pub fn apply(
    device: &dyn FeatureTransport,
    transport: &DeviceReadCapability,
    target: ButtonTarget,
    action: ButtonAction,
    confirm_with_readback: bool,
    next_transaction: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> std::result::Result<ButtonWriteReport, ButtonWriteFailure> {
    match MouseButtonMapping::new(device, transport) {
        Ok(mapping) => mapping.apply(
            target,
            action,
            confirm_with_readback,
            next_transaction,
            validate,
        ),
        Err(error) => Err(ButtonWriteFailure {
            phase: ButtonFailurePhase::Prepare,
            progress: ButtonWriteReport::default(),
            failed_step: None,
            error,
        }),
    }
}

#[cfg(test)]
#[path = "mouse_button_mapping_tests.rs"]
mod tests;
