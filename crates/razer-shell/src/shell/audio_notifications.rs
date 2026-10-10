//! Current mounted Audio_StreamMixer AudioEnumerator owner and worker lifetime.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::audio_notification::AudioNotification;
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

fn source_product(product_id: u32) -> bool {
    // Only these current startup roots register audio_streamMixer. The same
    // class occurs in other bundles without being activated by their roots.
    // See audio-util-active-consumers-current.json, independently of DLL version.
    matches!(product_id, 1422 | 1446)
}

pub(super) struct Session {
    stop: Sender<()>,
    worker: Option<JoinHandle<()>>,
    _status: Task<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        // Wake the owned thread immediately. Never block the GPUI thread on IPC.
        let _ = self.stop.send(());
    }
}

impl Session {
    fn retire(mut self) -> Option<JoinHandle<()>> {
        let _ = self.stop.send(());
        self.worker.take()
    }

    fn finish(mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn enabled(client: &mut ServiceClient, enable: bool) -> anyhow::Result<()> {
    let reply = client.request(ServiceRequest::AudioNotificationsEnable { enable })?;
    ensure!(
        reply["response"]["enabled"].as_i64() == Some(i64::from(enable)),
        "AudioEnumerator EnableNotification 响应与请求不匹配"
    );
    Ok(())
}

fn report(status: &Sender<String>, message: String) {
    // An unloaded owner no longer has a UI consumer, so cleanup diagnostics
    // must still survive a disconnected status receiver.
    eprintln!("[audio-enumerator] {message}");
    let _ = status.send(message);
}

fn run(stop: Receiver<()>, status: Sender<String>, product_id: u32) -> anyhow::Result<()> {
    let mut client = ServiceClient::spawn()?;
    let result = (|| {
        // Each original product middleware has its own listener owner. Keep an
        // independent worker rather than inventing shared duplicate enable calls.
        enabled(&mut client, true)?;
        loop {
            match stop.recv_timeout(Duration::from_millis(250)) {
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            let events: Vec<AudioNotification> =
                serde_json::from_value(client.request(ServiceRequest::AudioNotificationsDrain)?)
                    .context("AudioEnumerator 通知响应格式无效")?;
            for event in events {
                ensure!(
                    event.event == "RzAudioUtilEvent"
                        && event.event_type == "AudioEnumerator_DeviceChange"
                        && !event.endpoint_id.is_empty(),
                    "AudioEnumerator 原事件格式不匹配"
                );
                // Preserve the emitted spelling. Current product listeners use
                // AudioEnumerator_deviceChange (lowercase d) and only log.
                // This is diagnostic receipt, never an invented cache refresh.
                eprintln!(
                    "[audio-enumerator:{product_id}] {}",
                    serde_json::to_string(&event)?
                );
            }
        }
        Ok::<_, anyhow::Error>(())
    })();
    // Cleanup also runs after a drain/enable failure; an error can follow a
    // successful registration, so it cannot prove no listener was retained.
    if let Err(error) = enabled(&mut client, false) {
        report(&status, format!("AudioEnumerator 注销失败：{error:#}"));
    }
    if let Err(error) = client.request(ServiceRequest::Shutdown) {
        report(
            &status,
            format!("AudioEnumerator 通信进程退出失败：{error:#}"),
        );
    }
    result
}

impl AppShell {
    pub(super) fn install_audio_notification_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let sessions = std::mem::take(&mut this.audio_notifications);
            let retired = std::mem::take(&mut this.audio_notification_cleanup);
            // All retained workers begin teardown concurrently; sequential
            // joining must not defer the next product's disable request.
            for session in sessions.values() {
                let _ = session.stop.send(());
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (_, session) in sessions {
                    session.finish();
                }
                for worker in retired {
                    let _ = worker.join();
                }
            });
            async move { cleanup.await }
        }));
    }

    pub(super) fn sync_audio_notifications(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let desired = self.devices.iter().filter_map(|workspace| {
            let workspace = workspace.read(cx);
            let device = workspace.device(cx);
            if !source_product(device.product_id) { return None; }
            let mut routes = self.device_observations.iter().filter(|route| route.matches(device));
            let route = routes.next()?;
            if routes.next().is_some() { return None; }
            // A receiver-presence observation alone does not initialize a peer product.
            if matches!(route.connection(), razer_model::model::DeviceConnectionObservation::ReceiverPeer(status) if status != 1) {
                return None;
            }
            Some((workspace.identity(cx), device.product_id))
        }).collect::<BTreeMap<_, _>>();
        let removed = self.audio_notifications.keys()
            .filter(|key| !desired.contains_key(*key)).cloned().collect::<Vec<_>>();
        for key in removed {
            if let Some(worker) = self.audio_notifications.remove(&key).and_then(Session::retire) {
                self.audio_notification_cleanup.push(worker);
            }
        }
        // Joining only finished threads keeps teardown ownership without
        // blocking input while COM unregister or an IPC shutdown is pending.
        let mut pending = Vec::new();
        for worker in std::mem::take(&mut self.audio_notification_cleanup) {
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                pending.push(worker);
            }
        }
        self.audio_notification_cleanup = pending;
        for (identity, product_id) in desired {
            if self.audio_notifications.contains_key(&identity) {
                continue;
            }
            let (stop, stop_rx) = mpsc::channel();
            let (status_tx, status_rx) = mpsc::channel();
            let worker = match thread::Builder::new()
                .name(format!("razer-audio-{product_id}"))
                .spawn(move || {
                    if let Err(error) = run(stop_rx, status_tx.clone(), product_id) {
                        report(
                            &status_tx,
                            format!("AudioEnumerator 初始化或通知读取失败：{error:#}"),
                        );
                    }
                }) {
                Ok(worker) => worker,
                Err(error) => {
                    window.push_notification(
                        format!("AudioEnumerator 工作线程启动失败：{error}"),
                        cx,
                    );
                    continue;
                }
            };
            let owner = identity.clone();
            let status = cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(250))
                        .await;
                    let mut messages = Vec::new();
                    let disconnected = loop {
                        match status_rx.try_recv() {
                            Ok(message) => messages.push(message),
                            Err(mpsc::TryRecvError::Empty) => break false,
                            Err(mpsc::TryRecvError::Disconnected) => break true,
                        }
                    };
                    let current = this.update_in(cx, |this, window, cx| {
                        if !this.audio_notifications.contains_key(&owner) {
                            return false;
                        }
                        for message in messages {
                            this.status = message.clone();
                            window.push_notification(message, cx);
                            cx.notify();
                        }
                        true
                    });
                    if disconnected || !matches!(current, Ok(true)) {
                        break;
                    }
                }
            });
            self.audio_notifications.insert(
                identity,
                Session {
                    stop,
                    worker: Some(worker),
                    _status: status,
                },
            );
        }
    }
}
