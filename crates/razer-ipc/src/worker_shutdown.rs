//! Teardown drains old replies without publishing them as current observations.
use crate::ResponseEnvelope;
use anyhow::{Context as _, ensure};
use serde::Deserialize;
use std::{sync::mpsc::Receiver, time::{Duration, Instant}};

pub(super) fn drain(
    responses: &Receiver<anyhow::Result<ResponseEnvelope>>,
    shutdown_id: u64,
    timeout: Duration,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;
    let mut late_response = false;
    loop {
        let response = responses.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("设备服务未返回清理回执；恢复结果未确认")??;
        if response.id != shutdown_id {
            // ServiceClient serializes requests. Cancellation can leave exactly
            // one immediately preceding request awaiting its response.
            ensure!(!late_response && response.id == shutdown_id.wrapping_sub(1), "设备服务清理回执序号越界或重复");
            late_response = true;
            continue;
        }
        ensure!(response.data.is_some() != response.error.is_some(), "设备服务清理回执状态无效");
        if let Some(error) = response.error { anyhow::bail!("设备服务恢复失败：{error}"); }
        #[derive(Deserialize)]
        struct Receipt { stopped: bool }
        let receipt: Receipt = serde_json::from_value(response.data.context("设备服务清理回执为空")?)
            .context("设备服务清理回执格式无效")?;
        ensure!(receipt.stopped, "设备服务未确认清理完成");
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    fn response(id: u64, data: Option<serde_json::Value>, error: Option<&str>) -> anyhow::Result<ResponseEnvelope> {
        Ok(ResponseEnvelope { id, data, error: error.map(str::to_owned), fatal: false })
    }
    #[test]
    fn late_device_receipt_is_drained_before_matching_cleanup_receipt() {
        let (sender, receiver) = mpsc::channel();
        sender.send(response(41, Some(serde_json::json!({"written":true})), None)).unwrap();
        sender.send(response(42, Some(serde_json::json!({"stopped":true})), None)).unwrap();
        drain(&receiver, 42, Duration::ZERO).unwrap();
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn old_success_does_not_prove_restoration_or_hide_cleanup_failure() {
        let (sender, receiver) = mpsc::channel();
        sender.send(response(41, Some(serde_json::json!({"stopped":true})), None)).unwrap();
        sender.send(response(42, None, Some("mouse acceleration restore failed"))).unwrap();
        let error = drain(&receiver, 42, Duration::ZERO).unwrap_err();
        assert!(error.to_string().contains("mouse acceleration restore failed"));
    }
    #[test]
    fn absent_invalid_and_future_cleanup_receipts_are_failures() {
        for value in [serde_json::json!({}), serde_json::json!({"stopped":false}), serde_json::json!({"stopped":"true"})] {
            let (sender, receiver) = mpsc::channel();
            sender.send(response(42, Some(value), None)).unwrap();
            assert!(drain(&receiver, 42, Duration::ZERO).is_err());
        }
        let (sender, receiver) = mpsc::channel();
        sender.send(response(43, Some(serde_json::json!({"stopped":true})), None)).unwrap();
        assert!(drain(&receiver, 42, Duration::ZERO).is_err());
        drop(sender);
        assert!(drain(&receiver, 42, Duration::ZERO).is_err());
    }
    #[test]
    fn unrelated_or_duplicate_old_responses_cannot_extend_teardown() {
        for ids in [vec![40], vec![41, 41]] {
            let (sender, receiver) = mpsc::channel();
            for id in ids { sender.send(response(id, Some(serde_json::json!({"written":true})), None)).unwrap(); }
            sender.send(response(42, Some(serde_json::json!({"stopped":true})), None)).unwrap();
            assert!(drain(&receiver, 42, Duration::ZERO).is_err());
        }
    }
}
