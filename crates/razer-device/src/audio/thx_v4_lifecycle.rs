//! Explicit render setup from current AudioTHXGenericV4 source. This is not a
//! ReadSpatial side effect, profile restore, capture setup or UI refresh.
use super::*;

#[cfg(test)]
#[path = "thx_v4_lifecycle_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThxRenderSetupTrigger {
    InitialDeviceSetup,
    DefaultPlaybackDeviceChanged,
    PostRestoreDeviceData,
}
impl ThxRenderSetupTrigger {
    pub fn force_rebind(self) -> bool {
        self == Self::DefaultPlaybackDeviceChanged
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThxRenderSetupStatus {
    Completed,
    SetupRejected,
    TransportFailed,
    Canceled,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThxLifecycleReceipt {
    pub operation: ThxOperation,
    pub reply: Option<ThxReply>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThxRenderSetupOutcome {
    pub trigger: ThxRenderSetupTrigger,
    pub status: ThxRenderSetupStatus,
    pub transport_kind: ThxTransportKind,
    /// Source core returns true even when one or more setters returned false.
    /// This flag does not imply all settings were accepted or read back.
    pub setup_succeeded: bool,
    pub binding_ready: bool,
    pub receipts: Vec<ThxLifecycleReceipt>,
    pub retry_waits_ms: Vec<u64>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

impl<T: ThxV4Transport> ThxV4Controller<T> {
    fn lifecycle_exchange(
        &mut self,
        operation: ThxOperation,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut ThxRenderSetupOutcome,
    ) -> Result<ThxReply> {
        if let Err(error) = validate() {
            outcome.status = ThxRenderSetupStatus::Canceled;
            return Err(error);
        }
        let result = self.transport.exchange(&self.state.device, &operation);
        // Preserve a received acknowledgement before checking cancellation.
        // Cancellation can prevent further writes, not undo an accepted one.
        outcome.receipts.push(ThxLifecycleReceipt {
            operation,
            reply: result.as_ref().ok().cloned().or_else(|| {
                result
                    .as_ref()
                    .err()
                    .and_then(|error| error.downcast_ref::<ThxExchangeInterrupted>())
                    .map(|interrupted| interrupted.reply.clone())
            }),
            error: result.as_ref().err().map(|error| format!("{error:#}")),
        });
        if let Err(error) = validate() {
            outcome.status = ThxRenderSetupStatus::Canceled;
            return Err(error);
        }
        result
    }

    fn run_render_setup(
        &mut self,
        validate: &mut impl FnMut() -> Result<()>,
        wait_retry: &mut impl FnMut(u64) -> Result<()>,
        outcome: &mut ThxRenderSetupOutcome,
    ) -> Result<()> {
        if let Err(error) = self.require_transport() {
            outcome.status = ThxRenderSetupStatus::Unavailable;
            return Err(error);
        }
        self.state.ready = false;
        if outcome.trigger.force_rebind() {
            match self.lifecycle_exchange(ThxOperation::RemoveDevice, validate, outcome)? {
                ThxReply::NativeBool(true) | ThxReply::LocalBinding(true) => {}
                ThxReply::NativeBool(false) | ThxReply::LocalBinding(false) => outcome
                    .warnings
                    .push("Source continued after force RemoveDevice returned false".into()),
                _ => bail!("THX force RemoveDevice returned the wrong response type"),
            }
        }
        match self.lifecycle_exchange(ThxOperation::AddDevice, validate, outcome)? {
            ThxReply::NativeBool(true) | ThxReply::LocalBinding(true) => {}
            ThxReply::NativeBool(false) | ThxReply::LocalBinding(false) => {
                outcome.status = ThxRenderSetupStatus::SetupRejected;
                bail!("THX render setup AddDevice returned false");
            }
            _ => bail!("THX render setup AddDevice returned the wrong response type"),
        }
        for _ in 0..3 {
            match self.lifecycle_exchange(ThxOperation::CheckStatus, validate, outcome)? {
                ThxReply::NativeBool(true) | ThxReply::LocalReady(true) => {
                    self.state.ready = true;
                    outcome.binding_ready = true;
                    break;
                }
                ThxReply::NativeBool(false) | ThxReply::LocalReady(false) => {
                    // The source waits after the third false response too.
                    outcome.retry_waits_ms.push(500);
                    if let Err(error) = wait_retry(500) {
                        if validate().is_err() {
                            outcome.status = ThxRenderSetupStatus::Canceled;
                        }
                        return Err(error);
                    }
                    if let Err(error) = validate() {
                        outcome.status = ThxRenderSetupStatus::Canceled;
                        return Err(error);
                    }
                }
                _ => bail!("THX render setup CheckStatus returned the wrong response type"),
            }
        }
        if !outcome.binding_ready {
            outcome.status = ThxRenderSetupStatus::SetupRejected;
            bail!("THX render setup did not become ready after three checks");
        }
        // Current source initThxSettings: fixed order; false logs and continues,
        // whereas a rejected Promise/exception stops the remaining calls.
        for id in [
            "upmix_enabled",
            "eq_enabled",
            "spatial_advanced_early_reflections_enabled",
            "spatial_advanced_late_reverb_enabled",
        ] {
            let operation = ThxOperation::SetProperty(ThxPropertyRequest::boolean(id, true));
            match self.lifecycle_exchange(operation, validate, outcome)? {
                ThxReply::NativeBool(true) | ThxReply::RpcAcknowledged => {}
                ThxReply::NativeBool(false) => outcome
                    .warnings
                    .push(format!("Source continued after SetProperty({id}) returned false")),
                ThxReply::RpcRejected { code, message } => outcome.warnings.push(format!(
                    "Source false-result continuation after SetProperty({id}) RPC status {code}: {message:?}"
                )),
                _ => bail!("THX render setup SetProperty({id}) returned the wrong response type"),
            }
        }
        // No spatial getter or runtimeCache/profile/UI assignment in source.
        outcome.setup_succeeded = true;
        outcome.status = ThxRenderSetupStatus::Completed;
        Ok(())
    }

    pub fn setup_render(
        &mut self,
        trigger: ThxRenderSetupTrigger,
        mut validate: impl FnMut() -> Result<()>,
        mut wait_retry: impl FnMut(u64) -> Result<()>,
    ) -> ThxRenderSetupOutcome {
        let mut outcome = ThxRenderSetupOutcome {
            trigger,
            status: ThxRenderSetupStatus::TransportFailed,
            transport_kind: self.transport.kind(),
            setup_succeeded: false,
            binding_ready: false,
            receipts: vec![],
            retry_waits_ms: vec![],
            warnings: vec![],
            error: None,
        };
        if let Err(error) = self.run_render_setup(&mut validate, &mut wait_retry, &mut outcome) {
            outcome.error = Some(format!("{error:#}"));
        }
        outcome
    }
}
