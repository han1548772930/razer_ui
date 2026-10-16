//! GenericCamera 1.0.35.0 base (0E0B) and X (0E0C) processing/save methods.
//! Vtable RVAs and complete bodies are retained by the current camera audit.
//! X DNR reads the other channel from the device before writing both bytes;
//! a profile value cannot substitute for that query. Each ISP call owns its
//! own native mutex, matching the two separate calls in the original method.
use crate::camera_focus_family::{
    CameraFamilyFocusAttempt, CameraFamilyFocusFailure, CameraFamilyFocusPhase,
    CameraFamilyFocusTransport, execute_camera_family_isp, validate_camera_family_isp_progress,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraFamilyProcessingRequest {
    SaveUvcExtCmdToFlash,
    SetHdr {
        enabled: bool,
    },
    SetHdrBrightness {
        brightness: i32,
    },
    SetLdc {
        enabled: bool,
        mode: i32,
    },
    SetDynamicNoiseReduction {
        channel: i32,
        enabled: bool,
    },
    SetMirrorVideo {
        mode: i32,
    },
    SetAdaptiveMjpegQuality {
        enabled: bool,
        quality: i32,
    },
    SetFixedMjpegQuality {
        quality: i32,
    },
    SetWatermark {
        enabled: bool,
        position: i32,
    },
    GetAudioFunction,
    /// Current 3595 SetAudioFunction writes both bytes, saves flash, then reboots.
    SetAudioFunction {
        adaptive_noise_reduction: bool,
        crystal_clear_voice: bool,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyProcessingCommand {
    pub bytes: [u8; 8],
    pub native_rva: u32,
    pub mutates_state: bool,
}
impl CameraFamilyProcessingRequest {
    pub fn initial_command(self, product_id: u32) -> Result<CameraFamilyProcessingCommand> {
        use CameraFamilyProcessingRequest::*;
        ensure!(
            matches!(product_id, 3595 | 3596),
            "camera processing product has no current private audit"
        );
        ensure!(
            product_id == 3595 || !matches!(self, GetAudioFunction | SetAudioFunction { .. }),
            "camera microphone function is not mounted for this product"
        );
        if let SetDynamicNoiseReduction { channel, .. } = self {
            ensure!(matches!(channel as u8, 1 | 2), "invalid camera DNR channel");
        }
        let (bytes, native_rva, mutates_state) = match self {
            GetAudioFunction => ([0xc0, 0x0e, 0xa1, 0, 0, 0, 0, 0], 0x4e500, false),
            SetAudioFunction {
                adaptive_noise_reduction,
                crystal_clear_voice,
            } => (
                [
                    0xc0,
                    0x0e,
                    0xa0,
                    adaptive_noise_reduction.into(),
                    crystal_clear_voice.into(),
                    0,
                    0,
                    0,
                ],
                0x4e450,
                true,
            ),
            SaveUvcExtCmdToFlash if product_id == 3595 => {
                ([0xc0, 3, 0xa8, 0, 0, 0, 0, 0], 0x4d720, true)
            }
            SaveUvcExtCmdToFlash => ([0xfb, 0, 2, 0, 0, 0, 0, 0], 0x53b30, true),
            SetHdr { enabled } => ([0xff, 2, enabled.into(), 0, 0, 0, 0, 0], 0x4d530, true),
            SetHdrBrightness { brightness } => {
                ([0xff, 7, brightness as u8, 0, 0, 0, 0, 0], 0x4d660, true)
            }
            SetLdc { enabled, mode } => (
                [0xff, 1, enabled.into(), 3, mode as u8, 0, 0, 0],
                0x4d590,
                true,
            ),
            SetDynamicNoiseReduction { .. } if product_id == 3596 => {
                ([8, 0, 0, 0, 0, 0, 0, 0], 0x540c0, false)
            }
            SetDynamicNoiseReduction { channel, enabled } => (
                [0xc0, 0x0e, channel as u8, enabled.into(), 0, 0, 0, 0],
                0x4db80,
                true,
            ),
            SetMirrorVideo { mode } if product_id == 3596 => {
                ([0x29, 0, mode as u8, 0, 0, 0, 0, 0], 0x54240, true)
            }
            SetMirrorVideo { mode } => ([0xc0, 0x0e, 3, mode as u8, 0, 0, 0, 0], 0x4dca0, true),
            // X ignores the native enabled argument. Preserve that behavior,
            // including the subsequent fixed-quality call in a manual task.
            SetAdaptiveMjpegQuality { quality, .. } if product_id == 3596 => {
                ([0x2a, 0, quality as u8, 0, 0, 0, 0, 0], 0x54e60, true)
            }
            SetAdaptiveMjpegQuality { enabled, quality } => (
                [0xc0, 0x0e, 9, enabled.into(), 0, 0, quality as u8, 0],
                0x4e190,
                true,
            ),
            SetFixedMjpegQuality { quality } => {
                ([0xc0, 0x0e, 0x0b, quality as u8, 0, 0, 0, 0], 0x4e290, true)
            }
            SetWatermark { enabled, position } => (
                [0xc0, 8, 1, enabled.into(), position as u8, 0, 0, 0],
                0x4d810,
                true,
            ),
        };
        Ok(CameraFamilyProcessingCommand {
            bytes,
            native_rva,
            mutates_state,
        })
    }
    fn followup(
        self,
        product_id: u32,
        exchange: usize,
        response: [u8; 8],
    ) -> Option<CameraFamilyProcessingCommand> {
        if matches!(self, Self::SetAudioFunction { .. }) && product_id == 3595 {
            return match exchange {
                0 => Some(CameraFamilyProcessingCommand {
                    bytes: [0xc0, 3, 0xa8, 0, 0, 0, 0, 0],
                    native_rva: 0x4d720,
                    mutates_state: true,
                }),
                1 => Some(CameraFamilyProcessingCommand {
                    bytes: [0xc0, 0x0e, 7, 2, 0, 0, 0, 0],
                    native_rva: 0x4e650,
                    mutates_state: true,
                }),
                _ => None,
            };
        }
        if exchange != 0 {
            return None;
        }
        let Self::SetDynamicNoiseReduction { channel, enabled } = self else {
            return None;
        };
        (product_id == 3596).then(|| CameraFamilyProcessingCommand {
            bytes: if channel as u8 == 1 {
                [0x28, 0, response[2], enabled.into(), 0, 0, 0, 0]
            } else {
                [0x28, 0, enabled.into(), response[3], 0, 0, 0, 0]
            },
            native_rva: 0x53ed0,
            mutates_state: true,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraFamilyProcessingValue {
    /// Successful status-0 exchange for a method with no native out argument.
    Acknowledged,
    NativeValue {
        value: u8,
    },
    Watermark {
        state: u8,
        position: u8,
    },
    AudioFunction {
        adaptive_noise_reduction: u8,
        crystal_clear_voice: u8,
    },
}
/// Native Sleep(10) between complete ISP calls, after their COM cleanup.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub struct CameraFamilyProcessingDelay {
    pub after_exchange: usize,
    pub milliseconds: u32,
    pub completed: bool,
    pub failure: Option<CameraFamilyFocusFailure>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyProcessingExchange {
    pub command: CameraFamilyProcessingCommand,
    pub topology_nodes: Option<u32>,
    pub attempts: Vec<CameraFamilyFocusAttempt>,
    pub completed: bool,
    pub failure: Option<CameraFamilyFocusFailure>,
}
impl CameraFamilyProcessingExchange {
    fn response(&self) -> Option<[u8; 8]> {
        self.attempts
            .last()
            .filter(|a| a.acknowledged)
            .and_then(|a| a.response)
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyProcessingProgress {
    pub product_id: u32,
    pub request: CameraFamilyProcessingRequest,
    pub exchanges: Vec<CameraFamilyProcessingExchange>,
    #[serde(default)]
    pub delays: Vec<CameraFamilyProcessingDelay>,
    pub value: Option<CameraFamilyProcessingValue>,
    pub completed: bool,
    pub failure: Option<CameraFamilyFocusFailure>,
}
impl CameraFamilyProcessingProgress {
    pub fn failed(
        product_id: u32,
        request: CameraFamilyProcessingRequest,
        phase: CameraFamilyFocusPhase,
        message: String,
    ) -> Self {
        Self {
            product_id,
            request,
            exchanges: vec![],
            delays: vec![],
            value: None,
            completed: false,
            failure: Some(CameraFamilyFocusFailure {
                phase,
                node_id: None,
                message,
            }),
        }
    }
}
fn decode(
    product_id: u32,
    request: CameraFamilyProcessingRequest,
    response: [u8; 8],
) -> CameraFamilyProcessingValue {
    use CameraFamilyProcessingRequest::*;
    use CameraFamilyProcessingValue as V;
    let value = match request {
        GetAudioFunction => {
            return V::AudioFunction {
                adaptive_noise_reduction: response[3],
                crystal_clear_voice: response[4],
            };
        }
        SetDynamicNoiseReduction { enabled, .. } if product_id == 3596 => enabled.into(),
        SetMirrorVideo { mode } if product_id == 3596 => mode as u8,
        SetAdaptiveMjpegQuality { quality, .. } if product_id == 3596 => quality as u8,
        SetDynamicNoiseReduction { .. }
        | SetMirrorVideo { .. }
        | SetAdaptiveMjpegQuality { .. }
        | SetFixedMjpegQuality { .. } => response[3],
        SetWatermark { .. } => {
            return V::Watermark {
                state: response[2],
                position: response[3],
            };
        }
        _ => return V::Acknowledged,
    };
    V::NativeValue { value }
}
pub fn execute_camera_family_processing(
    transport: &mut impl CameraFamilyFocusTransport,
    product_id: u32,
    request: CameraFamilyProcessingRequest,
    validate: &impl Fn() -> Result<()>,
) -> CameraFamilyProcessingProgress {
    let initial = match request.initial_command(product_id) {
        Ok(c) => c,
        Err(e) => {
            return CameraFamilyProcessingProgress::failed(
                product_id,
                request,
                CameraFamilyFocusPhase::Target,
                e.to_string(),
            );
        }
    };
    let mut progress = CameraFamilyProcessingProgress {
        product_id,
        request,
        exchanges: vec![],
        delays: vec![],
        value: None,
        completed: false,
        failure: None,
    };
    let mut command = initial;
    loop {
        let isp = execute_camera_family_isp(transport, product_id, command.bytes, validate);
        let exchange = CameraFamilyProcessingExchange {
            command,
            topology_nodes: isp.topology_nodes,
            attempts: isp.attempts,
            completed: isp.completed,
            failure: isp.failure,
        };
        let response = exchange.response();
        let next = response.and_then(|r| request.followup(product_id, progress.exchanges.len(), r));
        progress.failure = exchange.failure.clone();
        let completed = exchange.completed;
        progress.exchanges.push(exchange);
        // Preserve an acknowledged final write when cancellation arrives after
        // GET. A successful read alone never acknowledges X's pending write.
        if next.is_none()
            && let Some(response) = response
        {
            progress.value = Some(decode(product_id, request, response));
        }
        if !completed {
            return progress;
        }
        if let Some(next) = next {
            if matches!(
                request,
                CameraFamilyProcessingRequest::SetAudioFunction { .. }
            ) {
                let result = validate()
                    .map_err(|e| (CameraFamilyFocusPhase::Validation, e))
                    .and_then(|_| {
                        transport
                            .wait_ms(10)
                            .map_err(|e| (CameraFamilyFocusPhase::Wait, e))
                    })
                    .and_then(|_| validate().map_err(|e| (CameraFamilyFocusPhase::Validation, e)));
                let failure = result.err().map(|(phase, e)| CameraFamilyFocusFailure {
                    phase,
                    node_id: None,
                    message: e.to_string(),
                });
                progress.delays.push(CameraFamilyProcessingDelay {
                    after_exchange: progress.exchanges.len() - 1,
                    milliseconds: 10,
                    completed: failure.is_none(),
                    failure: failure.clone(),
                });
                if let Some(failure) = failure {
                    progress.failure = Some(failure);
                    return progress;
                }
            }
            command = next;
        } else {
            progress.completed = true;
            return progress;
        }
    }
}
pub fn validate_camera_family_processing_progress(
    product_id: u32,
    request: CameraFamilyProcessingRequest,
    progress: &CameraFamilyProcessingProgress,
) -> Result<()> {
    use CameraFamilyFocusPhase as Phase;
    ensure!(
        progress.product_id == product_id && progress.request == request,
        "family processing receipt identity mismatch"
    );
    let initial = request.initial_command(product_id);
    if progress.exchanges.is_empty() {
        ensure!(
            !progress.completed
                && progress.delays.is_empty()
                && progress.value.is_none()
                && progress
                    .failure
                    .as_ref()
                    .is_some_and(|f| !f.message.is_empty()
                        && f.node_id.is_none()
                        && (f.phase == Phase::Target
                            || (initial.is_ok() && f.phase == Phase::Validation))),
            "family processing receipt omits its exchange"
        );
        return Ok(());
    }
    let initial = initial?;
    ensure!(
        progress.exchanges.len() <= 3 && progress.exchanges[0].command == initial,
        "family processing initial command/native metadata mismatch"
    );
    let audio = matches!(
        request,
        CameraFamilyProcessingRequest::SetAudioFunction { .. }
    );
    let mut delays = progress.delays.iter();
    let mut delay_failure = None;
    for (ix, exchange) in progress.exchanges.iter().enumerate() {
        validate_camera_family_isp_progress(
            exchange.topology_nodes,
            &exchange.attempts,
            exchange.completed,
            exchange.failure.as_ref(),
        )?;
        let followup = exchange
            .response()
            .and_then(|r| request.followup(product_id, ix, r));
        let next = progress.exchanges.get(ix + 1);
        if let Some(next) = next {
            ensure!(
                exchange.completed && followup == Some(next.command),
                "family processing command does not follow its actual prior response"
            );
        }
        if exchange.completed && followup.is_some() {
            if audio {
                let delay = delays.next().ok_or_else(|| {
                    anyhow::anyhow!("camera audio omitted its native inter-call wait")
                })?;
                ensure!(
                    delay.after_exchange == ix && delay.milliseconds == 10,
                    "camera audio inter-call wait identity mismatch"
                );
                ensure!(
                    delay.completed == delay.failure.is_none(),
                    "camera audio wait completion mismatch"
                );
                if let Some(failure) = &delay.failure {
                    ensure!(
                        next.is_none()
                            && failure.node_id.is_none()
                            && !failure.message.is_empty()
                            && matches!(failure.phase, Phase::Wait | Phase::Validation),
                        "camera audio proceeded after a failed inter-call wait"
                    );
                    delay_failure = Some(failure.clone());
                } else {
                    ensure!(
                        next.is_some(),
                        "camera audio omitted its save/reboot after a successful wait"
                    );
                }
            } else {
                ensure!(
                    next.is_some(),
                    "family processing successful read omitted its required write"
                );
            }
        } else {
            ensure!(
                next.is_none(),
                "family processing continued after its final or failed exchange"
            );
        }
    }
    ensure!(
        delays.next().is_none(),
        "family processing receipt has an unexecuted delay"
    );
    let last = progress.exchanges.last().unwrap();
    let is_final = last
        .response()
        .and_then(|r| request.followup(product_id, progress.exchanges.len() - 1, r))
        .is_none();
    let expected = if is_final {
        last.response().map(|r| decode(product_id, request, r))
    } else {
        None
    };
    ensure!(
        progress.value == expected
            && progress.completed == (is_final && last.completed)
            && progress.failure == delay_failure.or_else(|| last.failure.clone()),
        "family processing value/completion is not its actual final exchange"
    );
    Ok(())
}

#[cfg(test)]
#[path = "camera_processing_family_tests.rs"]
mod tests;
