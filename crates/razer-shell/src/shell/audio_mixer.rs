//! 1342 Audio Mixer DSP page requests over the source-derived HID protocol.
use super::*;
use razer_device::audio_mixer::{MixerValue, MixerWriteResult};
use razer_discovery::discovery;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{
    AudioMixerCompletion, AudioMixerOperation, AudioMixerReply, AudioMixerRequest,
};
use std::sync::{Mutex, OnceLock};

#[path = "audio_mixer/route.rs"]
mod route;

static MIXER_PAGE_QUEUE: OnceLock<Mutex<()>> = OnceLock::new();

fn check_response(
    response: &serde_json::Value,
    node: &razer_device::backend::HidNode,
    request: &AudioMixerRequest,
) -> anyhow::Result<()> {
    use anyhow::ensure;
    ensure!(
        response.get("node") == Some(&serde_json::to_value(node)?),
        "Mixer 响应 HID 身份不匹配"
    );
    ensure!(
        response
            .get("product_id")
            .and_then(serde_json::Value::as_u64)
            == Some(1342),
        "Mixer 响应产品不匹配"
    );
    ensure!(
        response.get("target") == Some(&serde_json::to_value(&request.target)?),
        "Mixer 响应属性不匹配"
    );
    ensure!(
        response
            .get("source_property")
            .and_then(serde_json::Value::as_str)
            == Some(razer_device::audio_mixer::source_property(&request.target)?),
        "Mixer 响应源属性不匹配"
    );
    Ok(())
}

impl AppShell {
    pub(super) fn request_audio_mixer(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: AudioMixerRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace.read(cx).mixer_request_matches(&request, cx) {
            return;
        }
        if !workspace.read(cx).mixer_request_current(&request, cx) {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mixer(
                    request.clone(),
                    Err("Mixer 请求作用域已变化".into()),
                    false,
                    window,
                    cx,
                )
            });
            return;
        }
        let device = workspace.read(cx).device(cx);
        if device.product_id != 1342 {
            return;
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(device)
                    && route.peer_product_id().is_none()
                    && matches!(route.transport(), Some(discovery::ObservedTransport::Wired))
            })
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mixer(
                    request.clone(),
                    Err("1342 DSP 没有唯一的有线 HID 观察".into()),
                    false,
                    window,
                    cx,
                )
            });
            return;
        };
        let route = (**route).clone();
        let identity = workspace.read(cx).identity(cx);
        let profile = device.active_profile.clone();
        let container = device.device_container_id.clone();
        let revision = self.discovery_revision;
        let Some(cancellation) = workspace.read(cx).mixer_cancellation(&request, cx) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let selected_request = request.clone();
            let selected_route = route.clone();
            let result = cx
                .background_spawn(async move {
                    let request = selected_request;
                    let route = selected_route;
                    use anyhow::{Context as _, ensure};
                    use std::sync::atomic::Ordering;
                    let _queue = MIXER_PAGE_QUEUE
                        .get_or_init(|| Mutex::new(()))
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Mixer 请求队列不可用"))?;
                    ensure!(!cancellation.load(Ordering::Acquire), "Mixer 请求已取消");
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        ensure!(!cancellation.load(Ordering::Acquire), "Mixer 请求已取消");
                        let node = route::resolve(&mut client, &route, &request.target)?;
                        ensure!(!cancellation.load(Ordering::Acquire), "Mixer 请求已取消");
                        let value = match request.operation {
                            AudioMixerOperation::MicEqWrite => {
                                let bands = request.eq_bands.context("Mic EQ请求缺少完整数组")?;
                                let expected = razer_device::audio_mixer::mic_eq_values(&bands)?;
                                route::revalidate(&mut client, &route, &node, &request.target)?;
                                ensure!(
                                    !cancellation.load(Ordering::Acquire),
                                    "Mic EQ请求已取消，未发送新的设置"
                                );
                                let response =
                                    client.request(ServiceRequest::HidNodeMixerEqWrite {
                                        node: node.clone(),
                                        product_id: 1342,
                                        bands,
                                    })?;
                                check_response(&response, &node, &request)?;
                                ensure!(
                                    response["bands"] == serde_json::to_value(bands)?,
                                    "Mic EQ回执数组与请求不匹配"
                                );
                                let results: Vec<MixerWriteResult> =
                                    serde_json::from_value(response["results"].clone())?;
                                ensure!(
                                    results.len() == 11
                                        && results.iter().all(|result| result.verified),
                                    "Mic EQ缺少完整设备回读确认"
                                );
                                ensure!(
                                    serde_json::to_value(&results[0].requested)?
                                        == serde_json::to_value(MixerValue::Boolean {
                                            enabled: true
                                        })?,
                                    "Mic EQ启用回执不匹配"
                                );
                                for (actual, expected) in results[1..].iter().zip(expected.iter()) {
                                    ensure!(
                                        serde_json::to_value(&actual.requested)?
                                            == serde_json::to_value(expected)?,
                                        "Mic EQ原参数或顺序回执不匹配"
                                    );
                                }
                                AudioMixerReply::MicEqWrite(results)
                            }
                            AudioMixerOperation::Read => {
                                let response =
                                    client.request(ServiceRequest::HidNodeMixerRead {
                                        node: node.clone(),
                                        product_id: 1342,
                                        target: request.target.clone(),
                                    })?;
                                check_response(&response, &node, &request)?;
                                let value = response
                                    .get("result")
                                    .cloned()
                                    .context("Mixer 读取响应缺少 result")?;
                                AudioMixerReply::Read(serde_json::from_value(value)?)
                            }
                            AudioMixerOperation::Write => {
                                let requested =
                                    request.value.clone().context("Mixer 写入请求缺少 value")?;
                                if matches!(
                                    request.path.as_str(),
                                    "/device/noiseGate/isEnabled"
                                        | "/device/compressor/isEnabled"
                                        | "/device/vocalFading/isEnabled"
                                        | "/device/voiceChanger/isEnabled"
                                ) {
                                    let response =
                                        client.request(ServiceRequest::HidNodeMixerRead {
                                            node: node.clone(),
                                            product_id: 1342,
                                            target: request.target.clone(),
                                        })?;
                                    check_response(&response, &node, &request)?;
                                    let previous: MixerValue = serde_json::from_value(
                                        response
                                            .get("result")
                                            .cloned()
                                            .context("DSP 开关读取缺少 result")?,
                                    )?;
                                    if let (
                                        MixerValue::Boolean { enabled: a },
                                        MixerValue::Boolean { enabled: b },
                                    ) = (&requested, &previous)
                                    {
                                        if a == b {
                                            // The source skips an unchanged switch. This is
                                            // a real read receipt, never a fabricated write ack.
                                            route::revalidate(
                                                &mut client,
                                                &route,
                                                &node,
                                                &request.target,
                                            )?;
                                            return Ok(AudioMixerCompletion {
                                                reply: AudioMixerReply::Read(previous),
                                                warning: None,
                                            });
                                        }
                                    } else {
                                        anyhow::bail!("DSP 开关读取类型不匹配")
                                    }
                                }
                                ensure!(
                                    !cancellation.load(Ordering::Acquire),
                                    "Mixer 请求已取消，未发送新的设置"
                                );
                                let sent_value = serde_json::to_value(&requested)?;
                                route::revalidate(&mut client, &route, &node, &request.target)?;
                                ensure!(
                                    !cancellation.load(Ordering::Acquire),
                                    "Mixer 请求已取消，未发送新的设置"
                                );
                                let response =
                                    client.request(ServiceRequest::HidNodeMixerWrite {
                                        node: node.clone(),
                                        product_id: 1342,
                                        target: request.target.clone(),
                                        value: requested,
                                    })?;
                                check_response(&response, &node, &request)?;
                                let value = response
                                    .get("result")
                                    .cloned()
                                    .context("Mixer 写入响应缺少 result")?;
                                let write: MixerWriteResult = serde_json::from_value(value)?;
                                ensure!(
                                    serde_json::to_value(&write.requested)? == sent_value,
                                    "Mixer 写入回执与请求不匹配"
                                );
                                ensure!(write.verified, "Mixer 写入缺少真实回读确认");
                                AudioMixerReply::Write(write)
                            }
                        };
                        route::revalidate(&mut client, &route, &node, &request.target)?;
                        Ok::<_, anyhow::Error>(AudioMixerCompletion {
                            reply: value,
                            warning: None,
                        })
                    })();
                    let shutdown_error = client
                        .request(ServiceRequest::Shutdown)
                        .err()
                        .map(|error| format!("Mixer 通信进程未正常结束：{error:#}"));
                    result.map(|mut completion| {
                        completion.warning = shutdown_error;
                        completion
                    })
                })
                .await
                .map_err(|error| format!("{error:#}"));
            let _ = this.update_in(cx, |this, window, cx| {
                if !workspace.read(cx).mixer_request_matches(&request, cx) {
                    return;
                }
                let current = this.devices.contains(&workspace)
                    && this.discovery_revision == revision
                    && workspace.read(cx).identity(cx) == identity
                    && workspace.read(cx).device(cx).active_profile == profile
                    && workspace.read(cx).device(cx).device_container_id == container
                    && this.device_observations.iter().any(|candidate| {
                        candidate.matches(workspace.read(cx).device(cx))
                            && candidate
                                .container()
                                .eq_ignore_ascii_case(route.container())
                            && candidate.physical_product_id() == route.physical_product_id()
                            && candidate.hid_node() == route.hid_node()
                            && candidate.peer_product_id().is_none()
                            && matches!(
                                candidate.transport(),
                                Some(discovery::ObservedTransport::Wired)
                            )
                    });
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_mixer(request, result, current, window, cx)
                });
            });
        })
        .detach();
    }
}
