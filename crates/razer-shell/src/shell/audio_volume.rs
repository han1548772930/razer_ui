//! Source 1352 speaker endpoint selection and typed read/write completion.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::simple_audio_volume::{AudioVolumeReadResult, AudioVolumeWriteResult};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{
    AudioVolumeCompletion, AudioVolumeOperation, AudioVolumeReply, AudioVolumeRequest,
};
use serde::Deserialize;
use std::{
    sync::{Mutex, OnceLock, atomic::Ordering},
    time::Duration,
};

// Serialize this application's page workers before endpoint resolution and
// submission. The service additionally locks the real endpoint across processes.
static PAGE_AUDIO_QUEUE: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Deserialize, PartialEq, Eq)]
struct Endpoint {
    id: String,
    name: String,
    #[serde(rename = "containerId")]
    container: String,
    #[serde(rename = "type")]
    kind: String,
}
fn resolve(client: &mut ServiceClient, container: &str) -> anyhow::Result<Endpoint> {
    ensure!(
        !container.is_empty() && !container.starts_with("hid-collection:"),
        "扬声器查询缺少真实设备容器；HID collection 不能替代 ContainerId"
    );
    let endpoints: Vec<Endpoint> =
        serde_json::from_value(client.request(ServiceRequest::AudioDevices)?)
            .context("simple_service AudioDevices 响应格式无效")?;
    // Current getAudioDeviceId filters type first, then takes the first exact
    // container match in the source-provided order. 1352 has no virtual flag.
    // Original name.includes(rzDevice.device.productName) fallback needs a live
    // native productName record which this model does not retain. Catalog/UI
    // names cannot stand in for that observation, so unmatched routes fail.
    endpoints
        .into_iter()
        .find(|endpoint| endpoint.kind == "speaker" && endpoint.container == container)
        .filter(|endpoint| !endpoint.id.is_empty())
        .context("没有匹配真实容器的扬声器；原始产品名回退尚缺真实 productName 观察")
}
fn check_read(reading: &AudioVolumeReadResult, endpoint: &Endpoint) -> anyhow::Result<()> {
    ensure!(
        reading.device_id == endpoint.id,
        "音量响应的系统 endpoint ID 不匹配"
    );
    ensure!(
        !reading.result || reading.volume <= 100,
        "扬声器音量响应超出当前页面 0..100 范围"
    );
    Ok(())
}
impl AppShell {
    pub(super) fn request_audio_volume(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: AudioVolumeRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace.read(cx).audio_volume_request_matches(request, cx) {
            return;
        }
        let device = workspace.read(cx).device(cx);
        let container = device.device_container_id.clone();
        let identity = workspace.read(cx).identity(cx);
        let profile = device.active_profile.clone();
        let revision = self.discovery_revision;
        let route_current = device.product_id == 1352
            && self.devices.contains(&workspace)
            && self
                .device_observations
                .iter()
                .any(|observed| observed.matches(device) && observed.hid_node().is_none());
        if !route_current || !workspace.read(cx).audio_volume_request_current(request, cx) {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_audio_volume(
                    request,
                    Err("当前页面没有真实的 1352 容器观察".into()),
                    false,
                    window,
                    cx,
                )
            });
            return;
        }
        let Some(cancellation) = workspace.read(cx).audio_volume_cancellation(request, cx) else {
            return;
        };
        cx.spawn_in(window,async move |this,cx| {
            let selected_container=container.clone();
            let result=cx.background_spawn(async move {
                let check_cancel=||->anyhow::Result<()> {
                    ensure!(!cancellation.load(Ordering::Acquire),"音量请求已取消；没有发送新的设置");
                    Ok(())
                };
                let _serial=PAGE_AUDIO_QUEUE.get_or_init(||Mutex::new(())).lock().map_err(|_|anyhow::anyhow!("音频请求队列不可用"))?;
                check_cancel()?;
                let mut client=ServiceClient::spawn()?;
                let result=(|| {
                    let endpoint=resolve(&mut client,&selected_container)?;
                    check_cancel()?;
                    let reply=match request.operation {
                        AudioVolumeOperation::Read=> {
                            // Current getSpeakerVolume defaults to maxTry=8,
                            // delayInMs=2000 and accepts only successful records.
                            // Rust transport/schema failures propagate explicitly.
                            let mut observed=None;
                            for attempt in 0..=8 {
                                check_cancel()?;
                                let reading:AudioVolumeReadResult=serde_json::from_value(client.request(ServiceRequest::AudioVolumeRead {device_id:endpoint.id.clone()})?)?;
                                check_read(&reading,&endpoint)?;
                                let received=reading.result;
                                observed=Some(reading);
                                if received {break;}
                                if attempt<8 {
                                    for _ in 0..20 {check_cancel()?;std::thread::sleep(Duration::from_millis(100));}
                                }
                            }
                            let reading=observed.context("扬声器未返回音量观察")?;
                            AudioVolumeReply::Read(reading)
                        }
                        AudioVolumeOperation::Write {mute,volume}=> {
                            ensure!(volume<=100,"请求音量超出源页面范围");
                            // Re-enumerate just before submission. A ContainerId
                            // must never be passed in place of the endpoint .id.
                            ensure!(resolve(&mut client,&selected_container)?==endpoint,"设置前扬声器 endpoint 已变化");
                            check_cancel()?;
                            let write:AudioVolumeWriteResult=serde_json::from_value(client.request(ServiceRequest::AudioVolumeWrite {device_id:endpoint.id.clone(),mute,volume})
                                .context("音量设置请求已开始但未收到回执；设备可能已更新，当前状态未确认")?)?;
                            check_read(&write.previous,&endpoint)?;
                            ensure!(write.previous.result,"设置前音量状态读取未成功");
                            ensure!(write.source_response.result==(write.hresult>=0),"音量设置的原始结果与 HRESULT 不匹配");
                            ensure!(write.volume_submitted==(write.previous.volume!=volume),"音量设置回执与原请求不匹配");
                            ensure!(!write.mute_submitted||write.previous.muted!=mute,"静音设置回执与原请求不匹配");
                            if let Some(reading)=&write.observation {check_read(reading,&endpoint)?;}
                            AudioVolumeReply::Write(write)
                        }
                    };
                    // Preserve a received mutation result if post-operation
                    // enumeration or shutdown fails; it may already have changed
                    // volume even when the current endpoint cannot be confirmed.
                    let post=resolve(&mut client,&selected_container);
                    let endpoint_current=post.as_ref().is_ok_and(|current|*current==endpoint)
                        &&!cancellation.load(Ordering::Acquire);
                    let warning=post.err().map(|error|format!("操作后音频观察失败：{error:#}"));
                    Ok::<_,anyhow::Error>(AudioVolumeCompletion {reply,warning,endpoint_current})
                })();
                let shutdown=client.request(ServiceRequest::Shutdown);
                result.map(|mut completion| {
                    if let Err(error)=shutdown {
                        let warning=format!("音频通信进程未正常结束：{error:#}");
                        completion.warning=Some(match completion.warning {Some(previous)=>format!("{previous}；{warning}"),None=>warning});
                    }
                    completion
                })
            }).await.map_err(|error|format!("{error:#}"));
            let _=this.update_in(cx,|this,window,cx| {
                if !workspace.read(cx).audio_volume_request_matches(request,cx) {return;}
                let device=workspace.read(cx).device(cx);
                let scope_current=this.devices.contains(&workspace)&&this.discovery_revision==revision
                    &&workspace.read(cx).identity(cx)==identity&&device.active_profile==profile&&device.device_container_id==container
                    &&this.device_observations.iter().any(|observed|observed.matches(device)&&observed.hid_node().is_none());
                workspace.update(cx,|workspace,cx|workspace.finish_audio_volume(request,result,scope_current,window,cx));
            });
        }).detach();
    }
}
