//! Mounted1422/1446 Audio_StreamMixer owns AudioMarie and AudioUtil together.
use super::*;
use razer_device::seiren_eq::{Change, NativeOperation, NativeReply, Report};
use razer_discovery::{
    direct::{self, DirectRoute},
    discovery::ObservedDevice,
};
use razer_pages::features::SeirenEqRequested;
use razer_pages::features::{
    SeirenStreamMixerCaptureChanged, SeirenStreamMixerObservation, SeirenStreamMixerRequested,
};
#[path = "audio_notifications_mixer.rs"]
mod mixer;

pub(super) enum Command {
    Stop,
    Eq {
        request: SeirenEqRequested,
        reply: Sender<anyhow::Result<Report>>,
    },
    Mixer {
        request: SeirenStreamMixerRequested,
        reply: Sender<anyhow::Result<MixerReceipt>>,
    },
    Capture(SeirenStreamMixerCaptureChanged),
    HostEvents(Vec<Value>),
    GlobalShortcutsEnabled(bool),
}
pub(super) enum MixerReceipt {
    LocalSaved,
    DeviceSubmitted,
}
pub(super) enum Update {
    Diagnostic(String),
    Native(Value),
    Ready(bool),
    Host(
        razer_storage::host::HostStorageCall,
        Sender<Result<razer_storage::host::HostStorageReply, String>>,
    ),
    Mixer(SeirenStreamMixerObservation),
    CaptureMode(SeirenStreamMixerCaptureChanged, bool),
    DeviceVersion(SeirenStreamMixerRequested, Sender<Result<u64, String>>),
    ProfileNotification {
        sender_id: u64,
        product_id: u32,
        container_id: String,
        preset_name: String,
        timer_tick: Value,
    },
}
fn save_device_version(
    status: &Sender<Update>,
    request: &SeirenStreamMixerRequested,
) -> anyhow::Result<()> {
    if !request.action().updates_device_document() {
        return Ok(());
    }
    let (reply, receiver) = mpsc::channel();
    status
        .send(Update::DeviceVersion(request.clone(), reply))
        .context("Seiren device-document owner closed")?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        ensure!(
            request.is_valid(),
            "Seiren mixer owner canceled while awaiting local persistence"
        );
        match receiver.recv_timeout(Duration::from_millis(40)) {
            Ok(result) => {
                result.map_err(anyhow::Error::msg)?;
                ensure!(
                    request.is_valid(),
                    "Seiren mixer owner canceled after local persistence"
                );
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) if std::time::Instant::now() < deadline => {}
            Err(error) => {
                return Err(error)
                    .context("Seiren device-document local persistence receipt unavailable");
            }
        }
    }
}
pub(super) fn report(status: &Sender<Update>, message: String) {
    eprintln!("[Audio_StreamMixer] {message}");
    let _ = status.send(Update::Diagnostic(message));
}
fn native_request(
    route: &DirectRoute,
    operation: NativeOperation,
) -> anyhow::Result<ServiceRequest> {
    match route {
        #[cfg(windows)]
        DirectRoute::Container(target) => Ok(ServiceRequest::SeirenNativeCall {
            target: target.clone(),
            native_operation: operation,
        }),
        _ => {
            let _ = operation;
            anyhow::bail!("Marie native audio requires its Windows container adapter")
        }
    }
}
fn native_call(
    client: &mut ServiceClient,
    route: &DirectRoute,
    operation: NativeOperation,
) -> anyhow::Result<NativeReply> {
    let response = client.request(native_request(route, operation.clone())?)?;
    ensure!(
        response["operation"] == serde_json::to_value(operation)?,
        "Marie native response operation differs"
    );
    serde_json::from_value(response["reply"].clone()).context("Marie native reply type is invalid")
}
fn routing(client: &mut ServiceClient, enabled: bool) -> anyhow::Result<()> {
    // Source awaits dispatch but never gates on the returned bool. Transport
    // exceptions still enter the caller's catch and stop its remaining chain.
    client.request(ServiceRequest::AudioRoutingEnable {
        enable: i32::from(enabled),
    })?;
    Ok(())
}
fn initialize(client: &mut ServiceClient, route: &DirectRoute) -> anyhow::Result<bool> {
    #[cfg(windows)]
    if let DirectRoute::Container(target) = route {
        let reply = client.request(ServiceRequest::SeirenNativeOpen {
            target: target.clone(),
        })?;
        ensure!(
            reply["target"] == serde_json::to_value(target)?,
            "Marie initialization route differs"
        );
        if reply["initialized"] != serde_json::to_value(NativeReply::Boolean(true))? {
            eprintln!(
                "[AudioMarie] unchecked DeviceInitialize result: {}",
                reply["initialized"]
            );
        }
        return Ok(true);
    }
    let _ = (client, route);
    anyhow::bail!("Marie native audio requires its Windows container adapter")
}
fn native_events(client: &mut ServiceClient, route: &DirectRoute) -> anyhow::Result<Vec<Value>> {
    #[cfg(windows)]
    if let DirectRoute::Container(target) = route {
        let reply = client.request(ServiceRequest::SeirenNativeEvents {
            target: target.clone(),
        })?;
        ensure!(
            reply["target"] == serde_json::to_value(target)?,
            "Marie native event route differs"
        );
        return serde_json::from_value(reply["events"].clone()).map_err(Into::into);
    }
    let _ = (client, route);
    anyhow::bail!("Marie native audio requires its Windows container adapter")
}
fn close(client: &mut ServiceClient, route: &DirectRoute) -> anyhow::Result<()> {
    #[cfg(windows)]
    if let DirectRoute::Container(target) = route {
        client.request(ServiceRequest::SeirenNativeClose {
            target: target.clone(),
        })?;
    }
    let _ = (client, route);
    Ok(())
}
fn local_snapshot(
    product_id: u32,
    action: Option<&razer_device::seiren_stream_mixer::Action>,
) -> anyhow::Result<Value> {
    use razer_device::seiren_stream_mixer as mixer;
    let guid = uuid::Uuid::new_v4().to_string();
    let mut snapshot = None;
    razer_storage::source_local::update(
        &razer_storage::source_local::path(),
        mixer::STORAGE_KEY,
        |old| {
            let (mut cache, created) = mixer::prepare_cache(old, product_id, &guid)?;
            let changed = action
                .map(|action| mixer::apply_cache(&mut cache, product_id, action, &guid))
                .transpose()?
                .unwrap_or(false);
            snapshot = Some(cache.clone());
            Ok((created || changed).then_some(cache))
        },
    )?;
    mixer::ui_payload(
        &snapshot.context("Local mixer snapshot is absent")?,
        product_id,
        "",
    )
}
fn run_local(
    commands: Receiver<Command>,
    status: Sender<Update>,
    product_id: u32,
    live: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    // One retained FIFO owns all local preview mutations. No ServiceClient is
    // created here and LocalSaved never claims a device/service write-back.
    match local_snapshot(product_id, None) {
        Ok(cache) => {
            let _ = status.send(Update::Mixer(SeirenStreamMixerObservation::Cache(cache)));
        }
        Err(error) => report(&status, format!("Mixer local cache: {error:#}")),
    }
    while let Ok(command) = commands.recv() {
        if !live.load(Ordering::Acquire) {
            break;
        }
        match command {
            Command::Stop => break,
            Command::Mixer { request, reply } => {
                let result = (|| {
                    ensure!(
                        request.is_valid() && request.product_id() == product_id,
                        "Local mixer request owner changed"
                    );
                    let cache = local_snapshot(product_id, Some(request.action()))?;
                    if request.is_valid() {
                        let _ =
                            status.send(Update::Mixer(SeirenStreamMixerObservation::Cache(cache)));
                    }
                    save_device_version(&status, &request)?;
                    Ok(MixerReceipt::LocalSaved)
                })();
                let _ = reply.send(result);
            }
            Command::Eq { reply, .. } => {
                let _ = reply.send(Err(anyhow::anyhow!("麦克风未连接，未提交设备操作")));
            }
            Command::Capture(request) => {
                if request.capturing() && request.is_valid() {
                    report(&status, "混音器快捷键捕获需要本地音频服务连接".into());
                }
            }
            Command::HostEvents(_) => {}
            Command::GlobalShortcutsEnabled(_) => {}
        }
    }
    Ok(())
}
pub(super) fn run(
    commands: Receiver<Command>,
    status: Sender<Update>,
    observed: Option<ObservedDevice>,
    product_id: u32,
    initial_rate: u32,
    view_id: u64,
    view_name: String,
    live: Arc<AtomicBool>,
    shortcuts_enabled: bool,
) -> anyhow::Result<()> {
    let Some(observed) = observed else {
        return run_local(commands, status, product_id, live);
    };
    let mut client = ServiceClient::spawn()?;
    client.set_request_guard(live.clone());
    let mut native = None;
    let mut stream_enabled = false;
    let mut capturing = false;
    let mut capture_request: Option<SeirenStreamMixerCaptureChanged> = None;
    let mut mixer = mixer::MixerOwner::new(
        observed.product_id(),
        observed.container().to_owned(),
        razer_device::seiren_stream_mixer::device_name(observed.product_id())?.to_owned(),
        view_id,
        view_name,
        status.clone(),
    );
    let result = (|| {
        if !shortcuts_enabled {
            client.request(ServiceRequest::EnableGlobalShortcuts { enable: false })?;
        }
        let route = direct::seiren_eq::resolve(&mut client, &observed)?;
        match initialize(&mut client, &route) {
            Ok(_) => {
                native = Some(route.clone());
            }
            Err(error) => report(&status, format!("AudioMarie init: {error:#}")),
        }
        enabled(&mut client, true)?;
        mixer.initialize_host()?;
        if native.is_some() {
            match native_call(&mut client, &route, NativeOperation::GetMode) {
                Ok(NativeReply::Long(mode)) if mode == 0 || mode == 1 => stream_enabled = mode == 1,
                result => report(&status, format!("AudioMarie GetMode: {result:?}")),
            }
        }
        if mixer.other_active() && stream_enabled {
            let _ = mixer::set_mode(&mut client, &route, false, &status)?;
            stream_enabled = false;
        }
        if stream_enabled {
            let _ = mixer::set_mode(&mut client, &route, true, &status)?;
            mixer.claim(&mut client, &route)?;
            mixer.register_presets(&mut client)?;
        }
        mixer.publish(&mut client)?;
        if native.is_some() {
            let _ = native_call(
                &mut client,
                &route,
                NativeOperation::SetLevelMeterEnable { enabled: true },
            )?;
            let _ = status.send(Update::Ready(false));
            routing(&mut client, false)?;
            let restored = route.seiren_eq(&mut client, Change::SamplingRate(initial_rate))?;
            if stream_enabled {
                routing(&mut client, true)?;
            }
            let _ = status.send(Update::Ready(restored.source_result()));
        }
        loop {
            if !live.load(Ordering::Acquire) {
                break;
            }
            if capturing
                && capture_request
                    .as_ref()
                    .is_some_and(|request| !request.is_valid())
            {
                // Blur/unmount invalidates the old request before its Stop can
                // reach this FIFO. Teardown must not use that invalid guard.
                client.request(ServiceRequest::StopShortcutCapture)?;
                capturing = false;
                capture_request = None;
            }
            match commands.recv_timeout(Duration::from_millis(40)) {
                Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Ok(Command::GlobalShortcutsEnabled(enable)) => {
                    if let Err(error) =
                        client.request(ServiceRequest::EnableGlobalShortcuts { enable })
                    {
                        report(
                            &status,
                            format!("Mixer shortcut recording suspension: {error:#}"),
                        );
                    }
                }
                Ok(Command::HostEvents(events)) => {
                    for event in events {
                        if let Err(error) = mixer.event(&mut client, &route, event) {
                            report(&status, format!("Mixer active owner: {error:#}"));
                        }
                    }
                }
                Ok(Command::Capture(request)) => {
                    let enabled = request.capturing();
                    if request.product_id() != product_id
                        || enabled && !request.is_valid()
                        || capture_request
                            .as_ref()
                            .is_some_and(|current| current.generation() > request.generation())
                    {
                        continue;
                    }
                    match client.request(if enabled {
                        ServiceRequest::StartShortcutCapture
                    } else {
                        ServiceRequest::StopShortcutCapture
                    }) {
                        Ok(_) => {
                            capturing = enabled;
                            capture_request = enabled.then_some(request);
                        }
                        Err(error) => report(&status, format!("Mixer shortcut capture: {error:#}")),
                    }
                }
                Ok(Command::Mixer { request, reply }) => {
                    let result = (|| {
                        ensure!(
                            request.is_valid(),
                            "Seiren mixer operation canceled before local save"
                        );
                        ensure!(
                            request.product_id() == observed.product_id(),
                            "Seiren mixer product changed"
                        );
                        client.set_request_guard(request.guard());
                        mixer.execute(&mut client, &route, &request, &mut stream_enabled)?;
                        ensure!(
                            request.is_valid(),
                            "Seiren mixer operation canceled after submission"
                        );
                        Ok(MixerReceipt::DeviceSubmitted)
                    })();
                    client.set_request_guard(live.clone());
                    let _ = reply.send(result);
                    if client.is_stopped() {
                        anyhow::bail!("Audio_StreamMixer service stopped");
                    }
                }
                Ok(Command::Eq { request, reply }) => {
                    let guard = request.guard();
                    let sampling = matches!(request.change(), Some(Change::SamplingRate(_)));
                    let outcome = (|| {
                        ensure!(
                            request.is_valid(),
                            "Seiren task canceled before service submission"
                        );
                        client.set_request_guard(guard);
                        let change = request
                            .change()
                            .cloned()
                            .context("Seiren request has no device operation")?;
                        ensure!(
                            !change.requires_native() || native.is_some(),
                            "AudioMarie initialization failed; native operation was not submitted"
                        );
                        if sampling {
                            let _ = status.send(Update::Ready(false));
                            routing(&mut client, false)?;
                        }
                        let report = route.seiren_eq(&mut client, change)?;
                        if sampling {
                            // Even a false native setter is followed by the
                            // source's routing restore, then ready=false.
                            if stream_enabled {
                                routing(&mut client, true)?;
                            }
                            let _ = status.send(Update::Ready(report.source_result()));
                        }
                        ensure!(
                            request.is_valid(),
                            "Seiren task canceled after device submission"
                        );
                        Ok(report)
                    })();
                    client.set_request_guard(live.clone());
                    let _ = reply.send(outcome);
                    if client.is_stopped() {
                        anyhow::bail!("Audio_StreamMixer service stopped");
                    }
                }
            }
            let events: Vec<AudioNotification> =
                serde_json::from_value(client.request(ServiceRequest::AudioNotificationsDrain)?)?;
            for event in events {
                ensure!(
                    event.event == "RzAudioUtilEvent"
                        && event.event_type == "AudioEnumerator_DeviceChange"
                        && !event.endpoint_id.is_empty(),
                    "AudioEnumerator original event differs"
                );
                // Current product listens to lowercase deviceChange and logs.
                eprintln!("[AudioEnumerator] {}", serde_json::to_string(&event)?);
            }
            if native.is_some() {
                for event in native_events(&mut client, &route)? {
                    if event["containerId"] == observed.container() {
                        if event["eventType"] == "deviceMute" {
                            let enabled = event["mute"].as_i64() == Some(0);
                            // Same original listener applies this even when the
                            // changed native endpoint is not the microphone.
                            if let Err(error) = native_call(
                                &mut client,
                                &route,
                                NativeOperation::SetLevelMeterEnable { enabled },
                            ) {
                                report(&status, format!("Marie mute meter state: {error:#}"));
                            }
                        }
                        let _ = status.send(Update::Native(event));
                    }
                }
            }
            // Routing status also remains source diagnostics, never a getter
            // replacing an optimistic page setting.
            let events = client.request(ServiceRequest::AudioRouterEvents)?;
            if events["events"]
                .as_array()
                .is_some_and(|events| !events.is_empty())
            {
                eprintln!("[AudioRouter_StatusChange] {}", events["events"]);
            }
            if capturing {
                let events = client.request(ServiceRequest::ShortcutCaptureEvents)?;
                for event in events["events"]
                    .as_array()
                    .context("Mixer shortcut mode event array is absent")?
                {
                    let event: Value = serde_json::from_str(
                        event["event"]
                            .as_str()
                            .context("Mixer shortcut mode event JSON is absent")?,
                    )?;
                    if let (Some(enabled), Some(request)) = (
                        event["isEnabled"].as_bool(),
                        capture_request
                            .as_ref()
                            .filter(|request| request.is_valid()),
                    ) {
                        let _ = status.send(Update::CaptureMode(request.clone(), enabled));
                    }
                }
            }
            if let Err(error) = mixer.poll_shortcuts(&mut client, &route, stream_enabled, capturing)
            {
                report(&status, format!("Mixer shortcut event: {error:#}"));
            }
        }
        Ok::<_, anyhow::Error>(())
    })();
    // Teardown must remain possible after the owner invalidates foreground
    // requests. No subsequent product mutation is submitted under this guard.
    client.set_request_guard(Arc::new(AtomicBool::new(true)));
    if capturing {
        if let Err(error) = client.request(ServiceRequest::StopShortcutCapture) {
            report(
                &status,
                format!("Mixer shortcut capture cleanup: {error:#}"),
            );
        }
    }
    mixer.cleanup_shortcuts(&mut client);
    if let Err(error) = enabled(&mut client, false) {
        report(&status, format!("AudioEnumerator cleanup: {error:#}"));
    }
    if let Some(route) = native {
        if let Err(error) = close(&mut client, &route) {
            report(&status, format!("AudioMarie cleanup: {error:#}"));
        }
    }
    if !client.is_stopped() {
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            report(&status, format!("Audio_StreamMixer shutdown: {error:#}"));
        }
    }
    result
}
