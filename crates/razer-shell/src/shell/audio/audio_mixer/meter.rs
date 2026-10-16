//! Retained native Mic capture producer; no synthetic peak samples.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

static ACTIVE: AtomicUsize = AtomicUsize::new(0);
pub(super) fn active_workers() -> usize {
    ACTIVE.load(Ordering::Acquire)
}
struct Active;
impl Drop for Active {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}

fn run(
    observation: discovery::ObservedDevice,
    cancel: Arc<AtomicBool>,
    mixer: bool,
    sender: mpsc::Sender<Result<[[f32; 2]; 5], String>>,
) -> anyhow::Result<()> {
    use anyhow::{Context as _, ensure};
    let mut client = ServiceClient::spawn()?;
    let result = (|| {
        let node = route::resolve_identity(&mut client, &observation)?;
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        let started = client.request(ServiceRequest::AudioMixerMeterStart {
            node: node.clone(),
            mixer,
        })?;
        ensure!(
            started["node"] == serde_json::to_value(&node)? && started["capture_started"] == true,
            "Mic capture start did not acknowledge the selected collection"
        );
        while !cancel.load(Ordering::Acquire) {
            let began = Instant::now();
            let sample = {
                let _queue = MIXER_PAGE_QUEUE
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mixer meter queue poisoned"))?;
                client.request(ServiceRequest::AudioMixerMeterRead { node: node.clone() })?
            };
            ensure!(
                sample["node"] == serde_json::to_value(&node)?,
                "Mic capture collection changed"
            );
            let mut values = [[0.; 2]; 5];
            for (index, channel) in ["mic", "playBackMix", "streamMix", "lineOut", "voiceChat"]
                .into_iter()
                .enumerate()
            {
                for (side, name) in ["left", "right"].into_iter().enumerate() {
                    values[index][side] = sample[channel][name]
                        .as_f64()
                        .with_context(|| format!("Audio meter {channel}.{name} sample missing"))?
                        as f32;
                    ensure!(
                        values[index][side].is_finite(),
                        "Audio meter sample is not finite"
                    );
                }
            }
            if sender.send(Ok(values)).is_err() {
                break;
            }
            let remaining = Duration::from_millis(80).saturating_sub(began.elapsed());
            // Short sleeps let page/device cancellation release capture promptly.
            let until = Instant::now() + remaining;
            while !cancel.load(Ordering::Acquire) && Instant::now() < until {
                std::thread::sleep(
                    Duration::from_millis(5).min(until.saturating_duration_since(Instant::now())),
                );
            }
        }
        Ok::<_, anyhow::Error>(())
    })();
    // Stop is required after partial start too. Shutdown/Drop is a second
    // cleanup boundary if the IPC reply itself failed.
    let stop = client.request(ServiceRequest::AudioMixerMeterStop);
    let shutdown = client.request(ServiceRequest::Shutdown);
    result?;
    stop?;
    shutdown?;
    Ok(())
}

impl AppShell {
    pub(super) fn request_audio_mixer_meter(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: AudioMixerRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(body) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        if !body.read(cx).mixer_meter_current(&request) {
            return;
        }
        let device = workspace.read(cx).device(cx);
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
            body.update(cx, |body, cx| {
                body.observe_mixer_meter(
                    &request,
                    Err("Mic meter requires one observed wired device".into()),
                    cx,
                )
            });
            return;
        };
        let route = (**route).clone();
        let Some(cancel) = request.local_lifetime.clone() else {
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        let (sender, receiver) = mpsc::channel();
        let worker_route = route.clone();
        let worker_cancel = cancel.clone();
        let mixer = request.path == "/device/audioMeter/mixer";
        ACTIVE.fetch_add(1, Ordering::AcqRel);
        let spawned = std::thread::Builder::new()
            .name("razer-audio-mic-meter".into())
            .spawn(move || {
                let _active = Active;
                if let Err(error) = run(worker_route, worker_cancel, mixer, sender.clone()) {
                    let _ = sender.send(Err(format!("{error:#}")));
                }
            });
        if let Err(error) = spawned {
            ACTIVE.fetch_sub(1, Ordering::AcqRel);
            body.update(cx, |body, cx| {
                body.observe_mixer_meter(&request, Err(error.to_string()), cx)
            });
            return;
        }
        let body = body.downgrade();
        cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
                let mut samples = Vec::new();
                let disconnected = loop {
                    match receiver.try_recv() {
                        Ok(sample) => samples.push(sample),
                        Err(mpsc::TryRecvError::Empty) => break false,
                        Err(mpsc::TryRecvError::Disconnected) => break true,
                    }
                };
                let current = shell
                    .update_in(cx, |shell, _, cx| {
                        let scope_current = shell.devices.contains(&workspace)
                            && workspace.read(cx).identity(cx) == identity
                            && shell.device_observations.iter().any(|candidate| {
                                candidate.matches(workspace.read(cx).device(cx))
                                    && candidate
                                        .container()
                                        .eq_ignore_ascii_case(route.container())
                                    && candidate.hid_node() == route.hid_node()
                            });
                        if !scope_current {
                            return false;
                        }
                        body.update(cx, |body, cx| {
                            if !body.mixer_meter_current(&request) {
                                return false;
                            }
                            for sample in samples {
                                body.observe_mixer_meter(&request, sample, cx);
                            }
                            true
                        })
                        .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if !current {
                    cancel.store(true, Ordering::Release);
                }
                if disconnected || !current {
                    break;
                }
            }
            cancel.store(true, Ordering::Release);
        })
        .detach();
    }
}
