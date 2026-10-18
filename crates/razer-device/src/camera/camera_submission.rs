//! Current camera profile/task submissions, separate from local draft storage.
use crate::camera::{
    CameraControlProperty as C, CameraSetting, StandardCameraProperty as P, StandardCameraRead,
    StandardCameraSetting, StandardCameraWrite, VideoProcAmpProperty as V,
};
use crate::camera_exposure_family::{
    CameraFamilyExposureProgress, CameraFamilyExposureRequest as X,
    validate_camera_family_exposure_progress,
};
use crate::camera_extensions::{
    CAMERA_EXTENSION_NODE, CameraExtensionExchange, CameraExtensionPhase, CameraExtensionProgress,
    CameraExtensionRequest as E, CameraExtensionTransport, CameraKsCallOutcome,
    camera_extension_header, execute_camera_extension,
};
use crate::camera_focus_family::{
    CameraFamilyFocusProgress, CameraFamilyFocusRequest as F, validate_camera_family_focus_progress,
};
use crate::camera_processing_family::{
    CameraFamilyProcessingProgress, CameraFamilyProcessingRequest as Q,
    validate_camera_family_processing_progress,
};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraScope {
    pub product_id: u32,
    pub container_id: String,
}
impl CameraScope {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.product_id, 3594..=3596),
            "camera product has no current audit"
        );
        let b = self.container_id.as_bytes();
        ensure!(
            b.len() == 38 && b[0] == b'{' && b[37] == b'}',
            "camera requires a complete OS ContainerId"
        );
        for (i, c) in b.iter().enumerate().take(37).skip(1) {
            ensure!(
                if [9, 14, 19, 24].contains(&i) {
                    *c == b'-'
                } else {
                    c.is_ascii_hexdigit()
                },
                "invalid camera ContainerId"
            );
        }
        ensure!(
            b.iter().any(|c| c.is_ascii_hexdigit() && *c != b'0'),
            "camera ContainerId is nil"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraCommand {
    /// A local modal DirectShow property page; closing it is not a write ACK.
    ShowProperties,
    StandardRead(P),
    StandardWrite(StandardCameraSetting),
    Extension(E),
    FamilyFocus(F),
    FamilyExposure(X),
    FamilyProcessing(Q),
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraSubmissionPlan {
    pub commands: Vec<CameraCommand>,
}
impl CameraSubmissionPlan {
    pub fn validate(&self, product_id: u32) -> Result<()> {
        ensure!(
            matches!(product_id, 3594..=3596),
            "camera product has no current audit"
        );
        ensure!(
            !self.commands.is_empty() && self.commands.len() <= 64,
            "invalid camera command count"
        );
        ensure!(
            product_id == 3594
                || !self
                    .commands
                    .iter()
                    .any(|c| matches!(c, CameraCommand::Extension(_))),
            "camera private protocol has no independent audit for this model"
        );
        for command in &self.commands {
            if matches!(command, CameraCommand::ShowProperties) {
                ensure!(
                    self.commands.len() == 1,
                    "camera property dialog must own its operation"
                );
            }
            if let CameraCommand::FamilyFocus(request) = command {
                request.command(product_id)?;
            }
            if let CameraCommand::FamilyExposure(request) = command {
                request.command(product_id)?;
            }
            if let CameraCommand::FamilyProcessing(request) = command {
                request.initial_command(product_id)?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraCommandOutcome {
    PropertiesClosed,
    Read(StandardCameraRead),
    Write(StandardCameraWrite),
    /// An actual failed Set HRESULT; never an acknowledgement or observation.
    RejectedWrite {
        requested: StandardCameraSetting,
        native_hresult: i32,
        #[serde(default)]
        phase: crate::camera::CameraSetPhase,
    },
    Extension(CameraExtensionProgress),
    FamilyFocus(CameraFamilyFocusProgress),
    FamilyExposure(CameraFamilyExposureProgress),
    FamilyProcessing(CameraFamilyProcessingProgress),
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraSubmissionProgress {
    pub outcomes: Vec<CameraCommandOutcome>,
    /// Untrusted transport data is retained for review and never published as
    /// an acknowledged command or device observation.
    #[serde(default)]
    pub rejected_outcome: Option<CameraCommandOutcome>,
    pub unconfirmed: Option<CameraCommand>,
    pub completed: bool,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraOperationStatus {
    Running,
    Finished(CameraSubmissionProgress),
}
pub trait CameraCommandTransport {
    fn execute(
        &mut self,
        command: &CameraCommand,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<CameraCommandOutcome>;
}
pub fn execute_camera_submission(
    transport: &mut impl CameraCommandTransport,
    product_id: u32,
    plan: &CameraSubmissionPlan,
    validate: &impl Fn() -> Result<()>,
) -> CameraSubmissionProgress {
    let mut progress = CameraSubmissionProgress::default();
    if let Err(e) = plan.validate(product_id) {
        progress.error = Some(e.to_string());
        return progress;
    }
    for (index, command) in plan.commands.iter().enumerate() {
        if let Err(e) = validate() {
            progress.error = Some(e.to_string());
            return progress;
        }
        match transport.execute(command, validate) {
            Err(e) => {
                progress.unconfirmed = Some(command.clone());
                progress.error = Some(format!("{e:#}"));
                return progress;
            }
            Ok(outcome) => {
                let validity = validate_command_outcome(product_id, command, &outcome);
                if let Err(e) = validity {
                    // Retain the raw receipt for review, but do not publish it
                    // as a confirmed read/write of a different command.
                    progress.rejected_outcome = Some(outcome);
                    progress.unconfirmed = Some(command.clone());
                    progress.error = Some(format!("invalid camera transport receipt: {e:#}"));
                    return progress;
                }
                progress.outcomes.push(outcome);
                if let Some(CameraCommandOutcome::RejectedWrite {
                    native_hresult,
                    phase,
                    ..
                }) = progress.outcomes.last()
                {
                    progress.error = Some(format!(
                        "camera {phase:?} failed: 0x{:08x}",
                        *native_hresult as u32
                    ));
                    if source_compound_ignores_false(plan) {
                        continue;
                    }
                    return progress;
                }
                if let Some(CameraCommandOutcome::Extension(p)) = progress.outcomes.last()
                    && !p.completed
                {
                    progress.error = p.failure.as_ref().map(|e| e.message.clone());
                    // Current Bn calls SetHDRBrightness even if SetHDR returned
                    // false. A failed native call is distinct from cancellation
                    // or a transport exception; retain its failed receipt.
                    let continue_hdr = matches!(
                        command,
                        CameraCommand::Extension(E::SetHdr { enabled: true })
                    ) && matches!(
                        plan.commands.get(index + 1),
                        Some(CameraCommand::Extension(E::SetHdrBrightness { .. }))
                    ) && p.failure.as_ref().is_some_and(|failure| {
                        matches!(
                            failure.phase,
                            CameraExtensionPhase::OpenControl
                                | CameraExtensionPhase::Set
                                | CameraExtensionPhase::Get
                                | CameraExtensionPhase::Decode
                        )
                    });
                    let continue_framing = matches!(
                        command,
                        CameraCommand::Extension(E::SetAutoframing { mode: 0 })
                    ) && is_view_write(plan.commands.get(index + 1))
                        && p.failure.as_ref().is_some_and(|failure| {
                            matches!(
                                failure.phase,
                                CameraExtensionPhase::OpenControl
                                    | CameraExtensionPhase::Set
                                    | CameraExtensionPhase::Get
                                    | CameraExtensionPhase::Decode
                            )
                        });
                    let continue_source_compound = source_compound_ignores_false(plan)
                        && p.failure.as_ref().is_some_and(|failure| {
                            matches!(
                                failure.phase,
                                CameraExtensionPhase::OpenControl
                                    | CameraExtensionPhase::Set
                                    | CameraExtensionPhase::Get
                                    | CameraExtensionPhase::Decode
                            )
                        });
                    if !continue_hdr && !continue_framing && !continue_source_compound {
                        return progress;
                    }
                }
                if let Some(CameraCommandOutcome::FamilyFocus(p)) = progress.outcomes.last()
                    && !p.completed
                {
                    progress.error = p.failure.as_ref().map(|e| e.message.clone());
                    let continue_framing =
                        matches!(
                            command,
                            CameraCommand::FamilyFocus(F::SetAutoframing { mode: 0 })
                        ) && is_view_write(plan.commands.get(index + 1))
                            && p.failure.as_ref().is_some_and(|failure| {
                                matches!(failure.phase,
                            crate::camera_focus_family::CameraFamilyFocusPhase::Set
                            | crate::camera_focus_family::CameraFamilyFocusPhase::Get
                            | crate::camera_focus_family::CameraFamilyFocusPhase::Decode
                            | crate::camera_focus_family::CameraFamilyFocusPhase::Exhausted)
                            });
                    let continue_source_compound =
                        source_compound_ignores_false(plan)
                            && p.failure.as_ref().is_some_and(|failure| {
                                matches!(failure.phase,
                            crate::camera_focus_family::CameraFamilyFocusPhase::Set
                            | crate::camera_focus_family::CameraFamilyFocusPhase::Get
                            | crate::camera_focus_family::CameraFamilyFocusPhase::Decode
                            | crate::camera_focus_family::CameraFamilyFocusPhase::Exhausted)
                            });
                    if !continue_framing && !continue_source_compound {
                        return progress;
                    }
                }
                if let Some(CameraCommandOutcome::FamilyExposure(p)) = progress.outcomes.last()
                    && !p.completed
                {
                    progress.error = p.failure.as_ref().map(|e| e.message.clone());
                    return progress;
                }
                if let Some(CameraCommandOutcome::FamilyProcessing(p)) = progress.outcomes.last()
                    && !p.completed
                {
                    progress.error = p.failure.as_ref().map(|e| e.message.clone());
                    // Current Wn / Zn / jn continue after native false, but
                    // cancellation, target validation and exceptions stop.
                    let source_compound = matches!(
                        plan.commands.first(),
                        Some(CameraCommand::FamilyProcessing(
                            Q::SetHdr { .. }
                                | Q::SetDynamicNoiseReduction { .. }
                                | Q::SetAdaptiveMjpegQuality { .. }
                        ))
                    );
                    use crate::camera_focus_family::CameraFamilyFocusPhase as Phase;
                    let last_attempt = p.exchanges.last().and_then(|e| e.attempts.last());
                    let native_false =
                        p.failure
                            .as_ref()
                            .is_some_and(|failure| match failure.phase {
                                Phase::Topology | Phase::Decode | Phase::Exhausted => true,
                                Phase::Set => last_attempt.is_some_and(|a| a.set_outcome.is_some()),
                                Phase::Get => last_attempt.is_some_and(|a| a.get_outcome.is_some()),
                                _ => false,
                            });
                    if !source_compound || !native_false {
                        return progress;
                    }
                }
            }
        }
    }
    // Cancellation after the last acknowledgement cannot erase that receipt.
    if let Err(e) = validate() {
        progress.error = Some(e.to_string());
        return progress;
    }
    progress.completed = progress.error.is_none();
    progress
}
fn is_view_write(command: Option<&CameraCommand>) -> bool {
    matches!(command, Some(CameraCommand::StandardWrite(setting)) if setting.property == P::CameraControl(C::Zoom))
}
fn source_compound_ignores_false(plan: &CameraSubmissionPlan) -> bool {
    match plan.commands.first() {
        Some(CameraCommand::StandardWrite(setting)) if setting.property == P::CameraControl(C::Focus) =>
            plan.commands.len() > 1,
        Some(CameraCommand::StandardWrite(setting)) if matches!(setting.property, P::VideoProcAmp(_)) =>
            plan.commands.iter().all(|command| matches!(command, CameraCommand::StandardWrite(setting) if matches!(setting.property, P::VideoProcAmp(V::Brightness | V::Contrast | V::Saturation | V::WhiteBalance | V::Sharpness | V::Gain)))),
        Some(CameraCommand::Extension(E::SetAutoframing { mode: 0 } | E::SetDynamicNoiseReduction { .. }))
        | Some(CameraCommand::FamilyFocus(F::SetAutoframing { mode: 0 })) => true,
        _ => false,
    }
}

fn validate_command_outcome(
    product_id: u32,
    command: &CameraCommand,
    outcome: &CameraCommandOutcome,
) -> Result<()> {
    match (command, outcome) {
        (CameraCommand::ShowProperties, CameraCommandOutcome::PropertiesClosed) => {}
        (CameraCommand::StandardRead(property), CameraCommandOutcome::Read(read)) => {
            ensure!(
                read.property == *property,
                "read property does not match submitted command"
            );
            ensure!(
                read.observation.native_hresult >= 0,
                "camera Get returned a failed HRESULT"
            );
        }
        (CameraCommand::StandardWrite(setting), CameraCommandOutcome::Write(write)) => {
            ensure!(
                write.property == setting.property,
                "write property does not match submitted command"
            );
            ensure!(
                write.requested == setting.setting,
                "write value/auto mode does not match submitted command"
            );
            ensure!(
                write.acknowledgement.accepted,
                "camera Set receipt is not accepted"
            );
            ensure!(
                write.acknowledgement.native_hresult >= 0,
                "camera Set returned a failed HRESULT"
            );
        }
        (
            CameraCommand::StandardWrite(setting),
            CameraCommandOutcome::RejectedWrite {
                requested,
                native_hresult,
                ..
            },
        ) => {
            ensure!(
                *setting == *requested && *native_hresult < 0,
                "invalid failed camera Set receipt"
            );
        }
        (CameraCommand::Extension(request), CameraCommandOutcome::Extension(progress)) => {
            validate_extension_receipt(*request, progress)?;
        }
        (CameraCommand::FamilyFocus(request), CameraCommandOutcome::FamilyFocus(progress)) => {
            validate_camera_family_focus_progress(product_id, *request, progress)?;
        }
        (
            CameraCommand::FamilyExposure(request),
            CameraCommandOutcome::FamilyExposure(progress),
        ) => {
            validate_camera_family_exposure_progress(product_id, *request, progress)?;
        }
        (
            CameraCommand::FamilyProcessing(request),
            CameraCommandOutcome::FamilyProcessing(progress),
        ) => {
            validate_camera_family_processing_progress(product_id, *request, progress)?;
        }
        _ => bail!("receipt kind does not match submitted command"),
    }
    Ok(())
}

fn set_succeeded(exchange: &CameraExtensionExchange) -> bool {
    exchange.set_attempted && exchange.set_outcome.is_some_and(|o| o.hresult >= 0)
}
fn get_succeeded(exchange: &CameraExtensionExchange) -> bool {
    exchange.get_attempted
        && exchange
            .get_outcome
            .is_some_and(|o| o.hresult == 0 && o.bytes_returned == 8)
        && exchange.response.is_some()
}
fn validate_extension_receipt(request: E, progress: &CameraExtensionProgress) -> Result<()> {
    ensure!(
        progress.request == request,
        "private request does not match submitted command"
    );
    let expected = request.commands();
    ensure!(
        progress.exchanges.len() <= expected.len(),
        "private receipt contains extra exchanges"
    );
    for (index, exchange) in progress.exchanges.iter().enumerate() {
        ensure!(
            exchange.command == expected[index],
            "private command bytes/metadata do not match request"
        );
        ensure!(
            exchange.set_outcome.is_none() || exchange.set_attempted,
            "SET result without an attempted SET"
        );
        ensure!(
            !exchange.get_attempted || set_succeeded(exchange),
            "GET before a successful SET"
        );
        ensure!(
            exchange.get_outcome.is_none() || exchange.get_attempted,
            "GET result without an attempted GET"
        );
        ensure!(
            exchange.response.is_none() || exchange.get_attempted,
            "response without an attempted GET"
        );
        ensure!(
            exchange.get_outcome.is_none() || exchange.response.is_some(),
            "GET result has no raw response"
        );
        if exchange.acknowledged {
            ensure!(
                set_succeeded(exchange)
                    && get_succeeded(exchange)
                    && exchange.response.is_some_and(|r| r[0] == 0),
                "private ACK has a failed or incomplete KS exchange"
            );
        } else {
            ensure!(
                !progress.completed && index + 1 == progress.exchanges.len(),
                "unacknowledged exchange precedes later results"
            );
        }
    }
    if progress.completed {
        ensure!(
            progress.failure.is_none(),
            "completed private receipt also reports failure"
        );
        ensure!(
            progress.exchanges.len() == expected.len(),
            "completed private receipt omits exchanges"
        );
        ensure!(
            progress.exchanges.iter().all(|e| e.acknowledged),
            "completed private receipt lacks acknowledgements"
        );
        // Re-decode only the supplied bytes with a memory-only transport. This
        // shares the audited response offsets without calling an OS adapter,
        // sleeping, or treating the receipt as independent device read-back.
        let mut replay = ExtensionReceiptReplay { progress, call: 0 };
        let decoded = execute_camera_extension(&mut replay, 3594, request, &|| Ok(()));
        ensure!(
            decoded == *progress,
            "private decoded value is inconsistent with raw exchanges"
        );
        return Ok(());
    }
    ensure!(
        progress.value.is_none(),
        "failed private operation publishes a decoded value"
    );
    let failure = progress
        .failure
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("incomplete private receipt has no failure phase"))?;
    ensure!(
        !failure.message.is_empty(),
        "private failure has no message"
    );
    ensure!(
        failure.command_index < expected.len(),
        "private failure command index is out of range"
    );
    let count = progress.exchanges.len();
    ensure!(
        count == failure.command_index || count == failure.command_index + 1,
        "private failure does not follow its acknowledged prefix"
    );
    use CameraExtensionPhase::*;
    if count == failure.command_index {
        ensure!(
            progress.exchanges.iter().all(|e| e.acknowledged),
            "private failure follows an unacknowledged command"
        );
        ensure!(
            matches!(failure.phase, Validation)
                || (failure.phase == Target && count == 0)
                || (failure.phase == Wait && expected[failure.command_index].wait_before_ms != 0),
            "private failure phase requires a missing exchange"
        );
    } else {
        let exchange = &progress.exchanges[failure.command_index];
        ensure!(
            progress.exchanges[..failure.command_index]
                .iter()
                .all(|e| e.acknowledged),
            "private failure follows an unacknowledged command"
        );
        let phase_matches = match failure.phase {
            Target => false,
            OpenControl => !exchange.set_attempted && !exchange.get_attempted,
            Validation => {
                (!exchange.set_attempted && !exchange.get_attempted)
                    || (set_succeeded(exchange) && !exchange.get_attempted)
                    || exchange.acknowledged
            }
            Set => {
                exchange.set_attempted
                    && !exchange.get_attempted
                    && !exchange.acknowledged
                    && exchange.set_outcome.is_none_or(|o| o.hresult < 0)
            }
            Wait => set_succeeded(exchange) && !exchange.get_attempted && !exchange.acknowledged,
            Get => {
                set_succeeded(exchange)
                    && exchange.get_attempted
                    && exchange.response.is_some()
                    && !exchange.acknowledged
                    && exchange
                        .get_outcome
                        .is_none_or(|o| o.hresult != 0 || o.bytes_returned != 8)
            }
            Decode => {
                set_succeeded(exchange)
                    && get_succeeded(exchange)
                    && !exchange.acknowledged
                    && exchange.response.is_some_and(|r| r[0] != 0)
            }
        };
        ensure!(
            phase_matches,
            "private failure phase is inconsistent with recorded calls"
        );
    }
    Ok(())
}

struct ExtensionReceiptReplay<'a> {
    progress: &'a CameraExtensionProgress,
    call: usize,
}
impl CameraExtensionTransport for ExtensionReceiptReplay<'_> {
    fn begin_node(&mut self, node_id: u32) -> Result<()> {
        ensure!(node_id == CAMERA_EXTENSION_NODE, "invalid private node");
        Ok(())
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        let set = self.call % 2 == 0;
        ensure!(
            *header == camera_extension_header(set),
            "invalid private header"
        );
        let exchange = self
            .progress
            .exchanges
            .get(self.call / 2)
            .ok_or_else(|| anyhow::anyhow!("missing private exchange"))?;
        let result = if set {
            ensure!(
                *data == exchange.command.bytes,
                "invalid private wire command"
            );
            exchange.set_outcome
        } else {
            *data = exchange
                .response
                .ok_or_else(|| anyhow::anyhow!("missing private response"))?;
            exchange.get_outcome
        };
        self.call += 1;
        result.ok_or_else(|| anyhow::anyhow!("missing private result"))
    }
    fn end_node(&mut self) {}
    fn wait_ms(&mut self, _: u32) -> Result<()> {
        Ok(())
    }
}
fn integer(profile: &Value, path: &str) -> Result<i32> {
    let n = profile
        .pointer(path)
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow::anyhow!("missing camera field {path}"))?;
    ensure!(
        n.is_finite() && n.fract() == 0. && n >= i32::MIN as f64 && n <= i32::MAX as f64,
        "invalid camera LONG {path}"
    );
    Ok(n as i32)
}
fn boolean(profile: &Value, path: &str) -> Result<bool> {
    profile
        .pointer(path)
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow::anyhow!("missing camera bool {path}"))
}
fn write(p: P, value: i32, is_auto: bool) -> CameraCommand {
    CameraCommand::StandardWrite(StandardCameraSetting {
        property: p,
        setting: CameraSetting { value, is_auto },
    })
}
fn view(profile: &Value) -> Result<&Value> {
    let mode = profile
        .pointer("/camera/viewPresets/viewMode")
        .ok_or_else(|| anyhow::anyhow!("missing view mode"))?;
    let entries = profile
        .pointer("/camera/viewPresets/data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("missing view presets"))?;
    let found: Vec<_> = entries
        .iter()
        .filter(|p| p.get("mode") == Some(mode))
        .collect();
    ensure!(found.len() == 1, "camera view preset is not unique");
    Ok(found[0])
}
fn image(profile: &Value) -> Result<&Value> {
    let mode = profile
        .get("imageMode")
        .or_else(|| profile.get("imageSettingMode"))
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("missing image mode"))?;
    profile
        .get("image")
        .and_then(Value::as_array)
        .and_then(|p| p.get(mode as usize))
        .and_then(|p| p.get("dataSet"))
        .ok_or_else(|| anyhow::anyhow!("missing selected image preset"))
}
/// The native setter ACK is not a getter, verified persistence or a local save.
pub fn compile_camera_edit(
    product_id: u32,
    key: &str,
    profile: &Value,
) -> Result<CameraSubmissionPlan> {
    let name = key.split_once(':').map_or(key, |(_, n)| n);
    let mut commands = Vec::new();
    match name {
        "save-camera" => {
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SaveUvcExtCmdToFlash)
            } else {
                CameraCommand::FamilyProcessing(Q::SaveUvcExtCmdToFlash)
            });
        }
        "restore-camera" => {
            ensure!(
                product_id == 3594,
                "camera firmware restore has no independent audit for this model"
            );
            commands.push(CameraCommand::Extension(E::RestoreDefaults));
        }
        "image-preset" | "brightness" | "contrast" | "saturation" | "sharpness" | "gain"
        | "white-balance" | "auto-white-balance" => {
            let p = image(profile)?;
            let properties: Vec<_> = match name {
                "brightness" => vec![(V::Brightness, "/brightness")],
                "contrast" => vec![(V::Contrast, "/contrast")],
                "saturation" => vec![(V::Saturation, "/saturation")],
                "sharpness" => vec![(V::Sharpness, "/sharpness")],
                "gain" => vec![(V::Gain, "/gain")],
                "white-balance" | "auto-white-balance" => {
                    vec![(V::WhiteBalance, "/whiteBalance/value")]
                }
                _ => vec![
                    (V::Brightness, "/brightness"),
                    (V::Contrast, "/contrast"),
                    (V::Saturation, "/saturation"),
                    (V::WhiteBalance, "/whiteBalance/value"),
                    (V::Sharpness, "/sharpness"),
                    (V::Gain, "/gain"),
                ],
            };
            for (property, path) in properties {
                if name == "image-preset"
                    && matches!(property, V::Sharpness | V::Gain)
                    && p.pointer(path).is_none_or(Value::is_null)
                {
                    continue;
                }
                commands.push(write(
                    P::VideoProcAmp(property),
                    integer(p, path)?,
                    property == V::WhiteBalance && boolean(p, "/whiteBalance/isAuto")?,
                ));
            }
        }
        "anti-flicker" => commands.push(write(
            P::VideoProcAmp(V::AntiFlicker),
            integer(profile, "/camera/antiFlicker")?,
            false,
        )),
        "zoom" | "pan" | "tilt" | "pan-tilt" | "view-preset" => {
            let p = view(profile)?;
            if matches!(name, "zoom" | "view-preset") {
                let zoom = p
                    .get("zoom")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| anyhow::anyhow!("missing zoom"))?;
                ensure!(
                    zoom.is_finite() && (1.0..=4.0).contains(&zoom),
                    "invalid zoom"
                );
                commands.push(write(
                    P::CameraControl(C::Zoom),
                    (zoom * 100.).round() as i32,
                    false,
                ));
            }
            for (n, property) in [("pan", C::Pan), ("tilt", C::Tilt)] {
                if name == n || matches!(name, "pan-tilt" | "view-preset") {
                    let value = p
                        .get(n)
                        .and_then(Value::as_f64)
                        .ok_or_else(|| anyhow::anyhow!("missing camera axis"))?;
                    ensure!(
                        value.is_finite() && value >= i32::MIN as f64 && value <= i32::MAX as f64,
                        "invalid camera axis LONG"
                    );
                    commands.push(write(
                        P::CameraControl(property),
                        value.trunc() as i32,
                        false,
                    ));
                }
            }
        }
        "lowLightCompensation" => commands.push(write(
            P::CameraControl(C::LowLightCompensation),
            i32::from(boolean(profile, "/camera/lowLightCompensation")?),
            false,
        )),
        "autofocus" | "focus" | "focus-tracking" | "focus-mode" | "focus-lighting" => {
            let auto = boolean(profile, "/camera/autoFocus/isEnabled")?;
            commands.push(write(
                P::CameraControl(C::Focus),
                if auto {
                    0
                } else {
                    integer(profile, "/camera/autoFocus/manualFocus")?
                },
                auto,
            ));
            let mode = if auto {
                integer(profile, "/camera/autoFocus/mode")?
            } else {
                1
            };
            let light_condition = integer(profile, "/camera/autoFocus/lightingType")?;
            // Family commands are independently recovered from the 1.0.35.0
            // base/X vtables, not inherited from the fixed-node Pro protocol.
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetFdafMode {
                    mode,
                    light_condition,
                })
            } else {
                CameraCommand::FamilyFocus(F::SetFdafMode {
                    mode,
                    light_condition,
                })
            });
            if auto {
                let value = integer(profile, "/camera/autoFocus/tracking")?;
                commands.push(if product_id == 3594 {
                    CameraCommand::Extension(E::SetAutoFocusLowSensitivity { value })
                } else {
                    CameraCommand::FamilyFocus(F::SetAutoFocusLowSensitivity { value })
                });
            }
        }
        "auto-exposure" | "reset-exposure" => {
            let automatic = boolean(profile, "/camera/autoExposure/isEnabled")?;
            if name == "reset-exposure" {
                ensure!(
                    matches!(product_id, 3594 | 3595) && automatic,
                    "current camera exposure reset requires its mounted automatic default"
                );
            }
            commands.push(write(P::CameraControl(C::Exposure), 0, automatic));
            // Current Z0i sends the entire reducer autoExposure object. The MW
            // task maker selects companions using the resulting auto mode.
            let tasks = if automatic {
                vec!["metering", "compensation"]
            } else if product_id == 3596 {
                // 0E0C's native ISO slot is a no-op, with no device receipt.
                vec!["manual-exposure", "shutter"]
            } else {
                vec!["iso", "shutter"]
            };
            for task in tasks {
                commands.extend(compile_camera_edit(product_id, task, profile)?.commands);
            }
        }
        "manual-exposure" => {
            // The current taskMaker only enqueues N0J for the non-extended
            // 3596 profile while auto exposure is disabled. 3595's entry
            // explicitly enables hasExtendedUVCExposure instead.
            ensure!(
                product_id == 3596,
                "camera model does not use the current standard manual-exposure lane"
            );
            ensure!(
                !boolean(profile, "/camera/autoExposure/isEnabled")?,
                "manual exposure is unavailable while auto exposure is enabled"
            );
            commands.push(write(
                P::CameraControl(C::Exposure),
                integer(profile, "/camera/autoExposure/exposureValue")?,
                false,
            ));
        }
        "mirror" => {
            let mode = i32::from(boolean(profile, "/camera/mirrorVideo")?);
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetMirrorVideo { mode })
            } else {
                CameraCommand::FamilyProcessing(Q::SetMirrorVideo { mode })
            });
        }
        "auto-framing" => {
            // Current generic CAMERA_AUTOFRAMING handler, including its
            // explicit return to the stored view when disabling the feature.
            let enabled = boolean(profile, "/camera/autoFraming/isEnabled")?;
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetAutoframing {
                    mode: i32::from(enabled),
                })
            } else {
                CameraCommand::FamilyFocus(F::SetAutoframing {
                    mode: i32::from(enabled),
                })
            });
            if enabled && product_id != 3596 {
                let framing = &profile["camera"]["autoFraming"];
                let speed = match framing["trackingSpeed"].as_str() {
                    Some("Quick") => 2,
                    Some("Gradual") => 1,
                    _ => 0,
                };
                let size = match (product_id, framing["view"].as_str()) {
                    (3594, Some("Close")) | (3595, Some("Wide")) => 1,
                    (3594, Some("Wide")) | (3595, Some("Close")) => 2,
                    _ => 0,
                };
                commands.push(if product_id == 3594 {
                    CameraCommand::Extension(E::SetAutoframingSpeedAndSize { speed, size })
                } else {
                    CameraCommand::FamilyFocus(F::SetAutoframingSpeedAndSize { speed, size })
                });
            } else if !enabled {
                commands.extend(compile_camera_edit(product_id, "view-preset", profile)?.commands);
            }
        }
        "iso" => {
            let code = integer(profile, "/camera/autoExposure/isoSensitivity")?;
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetIsoSensitivity { code })
            } else {
                CameraCommand::FamilyExposure(X::SetIsoSensitivity { code })
            });
        }
        "shutter" => {
            let value = integer(profile, "/camera/autoExposure/shutterSpeed")?;
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetShutterSpeed { value })
            } else {
                CameraCommand::FamilyExposure(X::SetShutterSpeed { value })
            });
        }
        "shutter-guard" => {
            // Current 3594 MW, 6259.9bbe7cd31292227b53ee.js:
            // cameraShutterLimitControl.setShutterLimitControl (64306-64310)
            // passes Number(value === "normal") to rzCamKimberly's DLL call.
            // Native Pro vtable RVA 0x52710 writes A0 1C 01 <mode>.
            let mode = profile
                .pointer("/camera/autoExposure/shutterLimitControl")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("missing camera shutter limit control"))?;
            commands.push(CameraCommand::Extension(E::SetShutterLimitControl {
                mode: i32::from(mode == "normal"),
            }));
        }
        "metering" => {
            let mode = integer(profile, "/camera/autoExposure/metering")?;
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetAutoExposureMetering { mode })
            } else {
                CameraCommand::FamilyExposure(X::SetAutoExposureMetering { mode })
            });
        }
        "compensation" => {
            let ev = profile
                .pointer("/camera/autoExposure/compensation")
                .and_then(Value::as_f64)
                .ok_or_else(|| anyhow::anyhow!("missing EV compensation"))?;
            ensure!(
                ev.is_finite() && (-3.0..=3.0).contains(&ev),
                "invalid EV compensation"
            );
            let value = (10. * ev + 30.).trunc() as i32;
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetEvCompensation { value })
            } else {
                CameraCommand::FamilyExposure(X::SetEvCompensation { value })
            });
        }
        "state2D" | "state3D" => {
            for (channel, path) in [
                (2, "/camera/dynamicNoiseReduction/state2D"),
                (1, "/camera/dynamicNoiseReduction/state3D"),
            ] {
                let enabled = boolean(profile, path)?;
                commands.push(if product_id == 3594 {
                    CameraCommand::Extension(E::SetDynamicNoiseReduction { channel, enabled })
                } else {
                    CameraCommand::FamilyProcessing(Q::SetDynamicNoiseReduction {
                        channel,
                        enabled,
                    })
                });
            }
        }
        "hdr" | "hdr-mode" => {
            // Current MW Bn: acquire the UI capture-stop lock, SetHDR, then
            // SetHDRBrightness only when enabled. The page owns that lock;
            // commands retain only the OS-independent device protocol.
            let enabled = boolean(profile, "/camera/hdr/isEnabled")?;
            commands.push(if product_id == 3594 {
                CameraCommand::Extension(E::SetHdr { enabled })
            } else {
                CameraCommand::FamilyProcessing(Q::SetHdr { enabled })
            });
            if enabled {
                let brightness = integer(profile, "/camera/hdr/mode")?;
                commands.push(if product_id == 3594 {
                    CameraCommand::Extension(E::SetHdrBrightness { brightness })
                } else {
                    CameraCommand::FamilyProcessing(Q::SetHdrBrightness { brightness })
                });
            }
        }
        "auto-quality" | "quality-mode" | "manual-quality" | "mjpeg-quality" => {
            ensure!(
                matches!(product_id, 3595 | 3596),
                "camera MJPEG model has no current submission audit"
            );
            let enabled = boolean(profile, "/camera/mjpegQuality/autoQuality")?;
            commands.push(CameraCommand::FamilyProcessing(
                Q::SetAdaptiveMjpegQuality {
                    enabled,
                    quality: if enabled {
                        integer(profile, "/camera/mjpegQuality/mode")?
                    } else {
                        0
                    },
                },
            ));
            if !enabled {
                commands.push(CameraCommand::FamilyProcessing(Q::SetFixedMjpegQuality {
                    quality: integer(profile, "/camera/mjpegQuality/manualQuality")?,
                }));
            }
        }
        "ldc" => {
            // Current base default omits ldc: JS Number(undefined) narrows to
            // false in the native bool argument. Keep absence in the profile.
            let enabled = profile
                .pointer("/camera/ldc")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            commands.push(CameraCommand::FamilyProcessing(Q::SetLdc {
                enabled,
                mode: 0,
            }));
        }
        "watermark" | "watermark-position" => {
            let position = match profile
                .pointer("/camera/watermark/position")
                .and_then(Value::as_str)
            {
                Some("left-top") => 1,
                Some("right-top") => 2,
                Some("left-bottom") => 3,
                Some("center-top") => 4,
                Some("center-bottom") => 5,
                _ => 0,
            };
            commands.push(CameraCommand::FamilyProcessing(Q::SetWatermark {
                enabled: boolean(profile, "/camera/watermark/isEnabled")?,
                position,
            }));
        }
        "apply-microphone" => {
            ensure!(
                product_id == 3595,
                "camera microphone page is not mounted for this product"
            );
            commands.push(CameraCommand::FamilyProcessing(Q::SetAudioFunction {
                adaptive_noise_reduction: boolean(profile, "/deviceMic/adaptiveNoiseReduction")?,
                crystal_clear_voice: boolean(profile, "/deviceMic/crystalClearVoice")?,
            }));
        }
        "shortcut-key" | "resolution" => {
            bail!("camera action {name} belongs to its local OS session")
        }
        _ => bail!("camera action {name} has no audited submission binding"),
    }
    let plan = CameraSubmissionPlan { commands };
    plan.validate(product_id)?;
    Ok(plan)
}

#[cfg(test)]
#[path = "camera_submission_tests.rs"]
mod tests;
