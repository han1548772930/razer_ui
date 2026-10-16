//! Current render shutdown is a separate lifecycle operation. The original
//! method uses the native wrappers directly, so it does not enter the spatial
//! before-hook, update source caches/local drafts, or read any device property.
use super::*;

#[cfg(test)]
#[path = "thx_v4_shutdown_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThxRenderShutdownStatus {
    Completed,
    TransportFailed,
    Canceled,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThxRenderShutdownOutcome {
    pub status: ThxRenderShutdownStatus,
    pub transport_kind: ThxTransportKind,
    /// Only means that the source shutdown control flow reached its end.
    /// False setter/removal replies do not stop it. This is not an assertion
    /// that all settings were accepted, read back, or saved to the device.
    pub shutdown_completed: bool,
    pub receipts: Vec<ThxLifecycleReceipt>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

impl<T: ThxV4Transport> ThxV4Controller<T> {
    fn shutdown_exchange(
        &mut self,
        operation: ThxOperation,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut ThxRenderShutdownOutcome,
    ) -> Result<ThxReply> {
        if let Err(error) = validate() {
            outcome.status = ThxRenderShutdownStatus::Canceled;
            return Err(error);
        }
        let result = self.transport.exchange(&self.state.device, &operation);
        let reply = result.as_ref().ok().cloned().or_else(|| {
            result
                .as_ref()
                .err()
                .and_then(|error| error.downcast_ref::<ThxExchangeInterrupted>())
                .map(|interrupted| interrupted.reply.clone())
        });
        // A received removal result invalidates Rust binding ownership even
        // when a later guard cancels this job. No source UI/cache is changed.
        if operation == ThxOperation::RemoveDevice
            && matches!(
                reply.as_ref(),
                Some(ThxReply::NativeBool(_) | ThxReply::LocalBinding(_))
            )
        {
            self.state.ready = false;
        }
        outcome.receipts.push(ThxLifecycleReceipt {
            operation,
            reply,
            error: result.as_ref().err().map(|error| format!("{error:#}")),
        });
        if let Err(error) = validate() {
            outcome.status = ThxRenderShutdownStatus::Canceled;
            let guard_error = format!("{error:#}");
            let receipt = outcome.receipts.last_mut().expect("Just recorded exchange");
            receipt.error = Some(match receipt.error.take() {
                Some(transport_error) => format!("{transport_error}; guard: {guard_error}"),
                None => guard_error,
            });
            return Err(error);
        }
        result
    }

    fn run_render_shutdown(
        &mut self,
        validate: &mut impl FnMut() -> Result<()>,
        outcome: &mut ThxRenderShutdownOutcome,
    ) -> Result<()> {
        if let Err(error) = self.require_transport() {
            outcome.status = ThxRenderShutdownStatus::Unavailable;
            return Err(error);
        }
        // Source shutdown does not add/check a binding or require a successful
        // prior setup. It awaits each wrapper, logs false, and advances.
        for id in ["eq_enabled", "spatial_enabled"] {
            let operation = ThxOperation::SetProperty(ThxPropertyRequest::boolean(id, false));
            match self.shutdown_exchange(operation, validate, outcome)? {
                ThxReply::NativeBool(true) | ThxReply::RpcAcknowledged => {}
                ThxReply::NativeBool(false) => outcome
                    .warnings
                    .push(format!("Source continued after shutdown SetProperty({id}) returned false")),
                ThxReply::RpcRejected { code, message } => outcome.warnings.push(format!(
                    "Source false-result continuation after shutdown SetProperty({id}) RPC status {code}: {message:?}"
                )),
                _ => bail!("THX render shutdown SetProperty({id}) returned the wrong response type"),
            }
        }
        match self.shutdown_exchange(ThxOperation::RemoveDevice, validate, outcome)? {
            ThxReply::NativeBool(true) | ThxReply::LocalBinding(true) => {}
            ThxReply::NativeBool(false) | ThxReply::LocalBinding(false) => outcome
                .warnings
                .push("Source continued after shutdown RemoveDevice returned false".into()),
            _ => bail!("THX render shutdown RemoveDevice returned the wrong response type"),
        }
        // No extra remove(): transport/resource release is owned by the caller.
        outcome.shutdown_completed = true;
        outcome.status = ThxRenderShutdownStatus::Completed;
        Ok(())
    }

    /// Runs the current source render shutdown sequence independently from
    /// worker/transport release. Guards between exchanges are Rust policy;
    /// cancellation cannot undo an acknowledged write or removal. Shutdown
    /// does not change the source spatial cache, UI state or local draft.
    pub fn shutdown_render(
        &mut self,
        mut validate: impl FnMut() -> Result<()>,
    ) -> ThxRenderShutdownOutcome {
        let mut outcome = ThxRenderShutdownOutcome {
            status: ThxRenderShutdownStatus::TransportFailed,
            transport_kind: self.transport.kind(),
            shutdown_completed: false,
            receipts: vec![],
            warnings: vec![],
            error: None,
        };
        if let Err(error) = self.run_render_shutdown(&mut validate, &mut outcome) {
            outcome.error = Some(format!("{error:#}"));
        }
        outcome
    }
}
