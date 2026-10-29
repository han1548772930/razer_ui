//! Teardown drains old replies without publishing them as current observations.
use crate::ResponseEnvelope;
use anyhow::{Context as _, ensure};
use serde::Deserialize;
#[cfg(test)]
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

/// A matching successful service teardown reply. Process exit and pipe joins
/// are separate facts and never construct this receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShutdownConfirmation {
    request_id: u64,
    source: ConfirmationSource,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmationSource {
    ServiceResponse,
    BrokerSession,
}

impl ShutdownConfirmation {
    pub fn source(self) -> ConfirmationSource {
        self.source
    }

    pub fn request_id(self) -> u64 {
        self.request_id
    }
}

pub(super) fn confirm(
    response: &ResponseEnvelope,
    shutdown_id: u64,
) -> anyhow::Result<ShutdownConfirmation> {
    ensure!(response.id == shutdown_id, "设备服务清理回执序号不匹配");
    response.validate_outcome()?;
    if let Some(error) = &response.error {
        return Err(crate::failure_diagnostics::retain(
            anyhow::anyhow!("设备服务恢复失败：{error}"),
            response.diagnostics.clone(),
        ));
    }
    #[derive(Deserialize)]
    struct Receipt {
        stopped: bool,
    }
    let receipt: Receipt = serde_json::from_value(
        response
            .data
            .as_ref()
            .context("设备服务清理回执为空")?
            .clone(),
    )
    .context("设备服务清理回执格式无效")?;
    ensure!(receipt.stopped, "设备服务未确认清理完成");
    Ok(ShutdownConfirmation {
        request_id: shutdown_id,
        source: ConfirmationSource::ServiceResponse,
    })
}

pub(super) fn confirm_session_close(
    receipt: &crate::hid_broker::CloseSessionReceipt,
    local_close_id: u64,
) -> anyhow::Result<ShutdownConfirmation> {
    receipt.validate()?;
    Ok(ShutdownConfirmation {
        request_id: local_close_id,
        source: ConfirmationSource::BrokerSession,
    })
}

#[cfg(test)]
pub(super) fn drain(
    responses: &Receiver<anyhow::Result<ResponseEnvelope>>,
    shutdown_id: u64,
    timeout: Duration,
) -> anyhow::Result<ShutdownConfirmation> {
    let deadline = Instant::now() + timeout;
    let mut late_response = false;
    loop {
        let response = responses
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("设备服务未返回清理回执；恢复结果未确认")??;
        if response.id != shutdown_id {
            // ServiceClient serializes requests. Cancellation can leave exactly
            // one immediately preceding request awaiting its response.
            ensure!(
                !late_response && response.id == shutdown_id.wrapping_sub(1),
                "设备服务清理回执序号越界或重复"
            );
            late_response = true;
            continue;
        }
        return confirm(&response, shutdown_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    fn response(
        id: u64,
        data: Option<serde_json::Value>,
        error: Option<&str>,
    ) -> anyhow::Result<ResponseEnvelope> {
        Ok(ResponseEnvelope {
            diagnostics: None,
            id,
            data,
            error: error.map(str::to_owned),
            fatal: false,
        })
    }
    #[test]
    fn late_device_receipt_is_drained_before_matching_cleanup_receipt() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(response(
                41,
                Some(serde_json::json!({"written":true})),
                None,
            ))
            .unwrap();
        sender
            .send(response(
                42,
                Some(serde_json::json!({"stopped":true})),
                None,
            ))
            .unwrap();
        assert_eq!(
            drain(&receiver, 42, Duration::ZERO).unwrap().request_id(),
            42
        );
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn old_success_does_not_prove_restoration_or_hide_cleanup_failure() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(response(
                41,
                Some(serde_json::json!({"stopped":true})),
                None,
            ))
            .unwrap();
        sender
            .send(response(
                42,
                None,
                Some("mouse acceleration restore failed"),
            ))
            .unwrap();
        let error = drain(&receiver, 42, Duration::ZERO).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mouse acceleration restore failed")
        );
    }
    #[test]
    fn absent_invalid_and_future_cleanup_receipts_are_failures() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"stopped":false}),
            serde_json::json!({"stopped":"true"}),
        ] {
            let (sender, receiver) = mpsc::channel();
            sender.send(response(42, Some(value), None)).unwrap();
            assert!(drain(&receiver, 42, Duration::ZERO).is_err());
        }
        let (sender, receiver) = mpsc::channel();
        sender
            .send(response(
                43,
                Some(serde_json::json!({"stopped":true})),
                None,
            ))
            .unwrap();
        assert!(drain(&receiver, 42, Duration::ZERO).is_err());
        drop(sender);
        assert!(drain(&receiver, 42, Duration::ZERO).is_err());
    }
    #[test]
    fn unrelated_or_duplicate_old_responses_cannot_extend_teardown() {
        for ids in [vec![40], vec![41, 41]] {
            let (sender, receiver) = mpsc::channel();
            for id in ids {
                sender
                    .send(response(
                        id,
                        Some(serde_json::json!({"written":true})),
                        None,
                    ))
                    .unwrap();
            }
            sender
                .send(response(
                    42,
                    Some(serde_json::json!({"stopped":true})),
                    None,
                ))
                .unwrap();
            assert!(drain(&receiver, 42, Duration::ZERO).is_err());
        }
    }

    #[test]
    fn shutdown_confirmation_requires_the_exact_successful_service_reply() {
        let success = response(
            42,
            Some(serde_json::json!({"stopped":true,"extension":7})),
            None,
        )
        .unwrap();
        assert_eq!(confirm(&success, 42).unwrap().request_id(), 42);
        assert!(confirm(&success, 41).is_err());
        for (data, error) in [
            (Some(serde_json::json!({"stopped":false})), None),
            (Some(serde_json::json!({"stopped":"true"})), None),
            (Some(serde_json::json!({})), None),
            (None, None),
            (None, Some("actual owner teardown failed")),
            (
                Some(serde_json::json!({"stopped":true})),
                Some("contradictory"),
            ),
        ] {
            assert!(confirm(&response(42, data, error).unwrap(), 42).is_err());
        }
    }

    #[test]
    fn broker_confirmation_requires_release_and_successful_restoration() {
        use crate::hid_broker::CloseSessionReceipt;
        for receipt in [
            CloseSessionReceipt::pending("native owner remains".to_owned()),
            CloseSessionReceipt::restoration_failed("restore failed".to_owned()),
        ] {
            assert!(confirm_session_close(&receipt, 42).is_err());
        }
        let confirmation = confirm_session_close(&CloseSessionReceipt::confirmed(), 42).unwrap();
        assert_eq!(confirmation.request_id(), 42);
        assert_eq!(confirmation.source(), ConfirmationSource::BrokerSession);
    }
}
