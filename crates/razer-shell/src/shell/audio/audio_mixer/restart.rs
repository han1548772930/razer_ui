//! Scoped stream restart receipts and the owned communication process.
use super::{MIXER_PAGE_QUEUE, discovery, route};
use anyhow::{Context as _, ensure};
use razer_device::{audio_mixer::StreamsResetResult, backend::HidNode};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde::Deserialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[cfg(test)]
#[path = "restart_tests.rs"]
mod tests;

#[derive(Deserialize)]
struct RestartReply {
    node: HidNode,
    product_id: u32,
    device_container_id: String,
    result: RestartResult,
}

#[derive(Deserialize)]
struct RestartResult {
    source_property: String,
    reset: StreamsResetResult,
}

fn accept_reply(
    response: serde_json::Value,
    node: &HidNode,
    container: &str,
) -> anyhow::Result<StreamsResetResult> {
    let reply: RestartReply =
        serde_json::from_value(response).context("Audio restart 回执缺少有效的身份或结果字段")?;
    ensure!(
        reply.node == *node
            && reply.product_id == 1342
            && reply.device_container_id.eq_ignore_ascii_case(container)
            && reply.result.source_property == razer_device::audio_mixer::STREAM_RESET_PROPERTY,
        "Audio restart 回执作用域不匹配"
    );
    let reset = reply.result.reset;
    ensure!(
        reset.source_sequence_completed
            && reset.return_codes.iter().map(|(index, _)| *index).eq(
                razer_device::audio_mixer::driver_spec()
                    .stream_indices
                    .iter()
                    .copied()
            ),
        "Audio restart 原流索引序列缺少回执"
    );
    ensure!(
        reset.completed_indices.iter().copied().eq(reset
            .return_codes
            .iter()
            .filter_map(|(index, code)| (*code == 0).then_some(*index))),
        "Audio restart 成功索引与原生返回值不一致"
    );
    // The original caller completes the sequence even when a native return
    // code is nonzero. A sequence receipt does not certify recovered audio.
    Ok(reset)
}

pub(in crate::shell) fn restart_audio_streams(
    observation: &discovery::ObservedDevice,
    canceled: &Arc<AtomicBool>,
) -> anyhow::Result<StreamsResetResult> {
    let _queue = MIXER_PAGE_QUEUE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow::anyhow!("Mixer 请求队列不可用"))?;
    ensure!(!canceled.load(Ordering::Acquire), "Audio restart 已取消");
    let mut client = ServiceClient::spawn()?;
    client.set_cancellation_guard(canceled.clone());
    let result = (|| {
        // ResetStream only needs its driver GUID and stream index. DSP
        // descriptors and firmware queries are not part of this command.
        let node = route::resolve_identity(&mut client, observation)?;
        let response = client.request(ServiceRequest::HidNodeMixerRestartStreams {
            node: node.clone(),
            product_id: 1342,
        })?;
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Audio restart 已取消；未采用在途回执，已提交的驱动操作可能已经生效"
        );
        accept_reply(response, &node, observation.container())
    })();
    let cleanup = if client.is_stopped() {
        client.finish_termination()
    } else {
        client.request(ServiceRequest::Shutdown).map(|_| ())
    };
    match (result, cleanup) {
        (Ok(result), Ok(())) => Ok(result),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error.context("Audio restart 原序列已完成，但通信进程结束失败")),
        (Err(error), Err(cleanup)) => {
            Err(anyhow::anyhow!("{error:#}；通信进程结束失败：{cleanup:#}"))
        }
    }
}
