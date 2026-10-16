//! Serialized source local Mic actions, owned preview restoration and cleanup.
use anyhow::{Context as _, ensure};
use razer_device::{
    audio_mixer_local::{Command, Receipt, State},
    backend::HidNode,
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::AudioMixerRequest;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

struct Owner {
    node: HidNode,
    lifetime: Arc<AtomicBool>,
    state: State,
    preview_restore: Option<Value>,
    lighting_sources: Option<[u8; 4]>,
    lighting_error: Option<String>,
}
static SOURCE_STATES: OnceLock<Mutex<BTreeMap<Vec<u8>, Owner>>> = OnceLock::new();
static WORKERS: AtomicUsize = AtomicUsize::new(0);
pub(super) fn active_workers() -> usize {
    WORKERS.load(Ordering::Acquire)
}
struct Worker;
impl Drop for Worker {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn submit(
    client: &mut ServiceClient,
    node: &HidNode,
    state: &mut State,
    commands: Vec<Command>,
    canceled: &AtomicBool,
) -> anyhow::Result<Vec<Receipt>> {
    let mut receipts = Vec::new();
    for command in commands {
        let result = (|| -> anyhow::Result<Receipt> {
            ensure!(
                !canceled.load(Ordering::Acquire),
                "Local Mixer action canceled after {} completed reports",
                receipts.len()
            );
            if let Command::ChannelMute {
                channel, enabled, ..
            } = &command
            {
                if (1..=4).contains(channel) {
                    state.channel_mutes[usize::from(*channel) - 1] = Some(*enabled);
                }
            }
            let response = client.request(ServiceRequest::HidNodeMixerLocalWrite {
                node: node.clone(),
                product_id: 1342,
                command: command.clone(),
            })?;
            ensure!(
                response["node"] == serde_json::to_value(node)? && response["product_id"] == 1342,
                "Local Mixer response identity mismatch"
            );
            let receipt: Receipt = serde_json::from_value(response["receipt"].clone())?;
            ensure!(
                receipt.transport_completed
                    && serde_json::to_value(&receipt.command)? == serde_json::to_value(&command)?,
                "Local Mixer response command mismatch"
            );
            Ok(receipt)
        })();
        match result {
            Ok(receipt) => receipts.push(receipt),
            Err(error) => {
                receipts.push(Receipt {
                    command,
                    transport_completed: false,
                    response: serde_json::json!({"error":format!("{error:#}")}),
                    profile_updates: Vec::new(),
                });
                break;
            }
        }
    }
    Ok(receipts)
}

fn submit_source(
    client: &mut ServiceClient,
    node: &HidNode,
    state: &mut State,
    commands: Vec<Command>,
    canceled: &AtomicBool,
    path: &str,
    preview: bool,
) -> anyhow::Result<Vec<Receipt>> {
    if path != "/device/micPreview" {
        return submit(client, node, state, commands, canceled);
    }
    // Pi/Ui catch their complete gain chain. Li continues to routing even
    // after that failure. Restore $i also catches before Mi; preview's direct
    // route call does not, so an unsuccessful route skips its monitor setter.
    let boundary = commands
        .iter()
        .position(|c| matches!(c, Command::InputMicGain { .. }))
        .context("Preview gain boundary absent")?
        + 1;
    let mut head = commands;
    let mut tail = head.split_off(boundary);
    let mut receipts = submit(client, node, state, head, canceled)?;
    if canceled.load(Ordering::Acquire) {
        return Ok(receipts);
    }
    if preview {
        receipts.extend(submit(client, node, state, tail, canceled)?);
    } else {
        let monitor = tail.pop().context("Preview monitor restore absent")?;
        receipts.extend(submit(client, node, state, tail, canceled)?);
        if !canceled.load(Ordering::Acquire) {
            receipts.extend(submit(client, node, state, vec![monitor], canceled)?);
        }
    }
    Ok(receipts)
}

fn watch(key: Vec<u8>, node: HidNode, lifetime: Arc<AtomicBool>) -> anyhow::Result<()> {
    WORKERS.fetch_add(1, Ordering::AcqRel);
    let result = std::thread::Builder::new()
        .name("mixer-preview-cleanup".into())
        .spawn(move || {
            let _worker = Worker;
            let mut lighting_client: Option<ServiceClient> = None;
            while !lifetime.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(80));
                let result = (|| -> anyhow::Result<()> {
                    let _queue = super::MIXER_PAGE_QUEUE
                        .get_or_init(|| Mutex::new(()))
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Mixer lighting queue poisoned"))?;
                    if lifetime.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let mut states = SOURCE_STATES
                        .get_or_init(|| Mutex::new(BTreeMap::new()))
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Mixer lighting state poisoned"))?;
                    let Some(owner) = states.get_mut(&key) else {
                        return Ok(());
                    };
                    let Some(sources) = owner.lighting_sources else {
                        return Ok(());
                    };
                    if lighting_client.is_none() {
                        lighting_client = Some(ServiceClient::spawn()?);
                    }
                    let result = submit(
                        lighting_client
                            .as_mut()
                            .context("Mixer lighting worker absent")?,
                        &node,
                        &mut owner.state,
                        vec![Command::LightingSample { sources }],
                        &lifetime,
                    )?;
                    let error = result
                        .iter()
                        .find(|r| !r.transport_completed)
                        .map(|r| r.response.to_string());
                    if error != owner.lighting_error {
                        if let Some(error) = &error {
                            eprintln!("Audio Mixer lighting sample: {error}");
                        }
                        owner.lighting_error = error;
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    eprintln!("Audio Mixer lighting worker: {error:#}");
                }
            }
            if let Some(mut client) = lighting_client {
                if let Err(error) = client.request(ServiceRequest::Shutdown) {
                    eprintln!("Audio Mixer lighting cleanup: {error:#}");
                }
            }
            let result = (|| -> anyhow::Result<()> {
                let _queue = super::MIXER_PAGE_QUEUE
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mixer cleanup queue poisoned"))?;
                let owner = SOURCE_STATES
                    .get_or_init(|| Mutex::new(BTreeMap::new()))
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mixer cleanup state poisoned"))?
                    .remove(&key);
                let Some(mut owner) = owner else {
                    return Ok(());
                };
                let Some(mut profile) = owner.preview_restore.take() else {
                    return Ok(());
                };
                profile["device"]["micPreview"] = Value::Bool(false);
                let commands = owner
                    .state
                    .clone()
                    .compile("/device/micPreview", &profile)?;
                let mut client = ServiceClient::spawn()?;
                let result = submit_source(
                    &mut client,
                    &node,
                    &mut owner.state,
                    commands,
                    &AtomicBool::new(false),
                    "/device/micPreview",
                    false,
                );
                let result = result.and_then(|receipts| {
                    ensure!(
                        receipts.iter().all(|r| r.transport_completed),
                        "Preview restore incomplete: {}",
                        serde_json::to_string(&receipts)?
                    );
                    Ok(receipts)
                });
                let cleanup = client.request(ServiceRequest::Shutdown);
                match (result, cleanup) {
                    (Ok(_), Ok(_)) => Ok(()),
                    (Err(e), Ok(_)) => Err(e),
                    (Ok(_), Err(e)) => {
                        Err(e.context("Preview restored but worker shutdown unconfirmed"))
                    }
                    (Err(e), Err(c)) => {
                        Err(anyhow::anyhow!("Preview restore: {e:#}; shutdown: {c:#}"))
                    }
                }
            })();
            if let Err(error) = result {
                eprintln!("Audio Mixer preview scope cleanup failed: {error:#}");
            }
        });
    if let Err(error) = result {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn write(
    client: &mut ServiceClient,
    node: &HidNode,
    request: &AudioMixerRequest,
    canceled: &AtomicBool,
) -> anyhow::Result<Vec<Receipt>> {
    let profile = request
        .local_profile
        .as_ref()
        .context("Local Mixer request lacks complete source profile")?;
    let lifetime = request
        .local_lifetime
        .as_ref()
        .context("Local Mixer request lacks retained scope lifetime")?;
    ensure!(
        !lifetime.load(Ordering::Acquire),
        "Local Mixer owner already canceled"
    );
    let mut key = serde_json::to_vec(node)?;
    key.extend_from_slice(&(Arc::as_ptr(lifetime) as usize).to_le_bytes());
    let mut states = SOURCE_STATES
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .map_err(|_| anyhow::anyhow!("Mixer source cache poisoned"))?;
    // This function runs under MIXER_PAGE_QUEUE. Restore canceled scopes
    // before the next profile's first write, even if their watcher is waiting.
    let stale = states
        .iter()
        .filter(|(_, owner)| owner.node == *node && owner.lifetime.load(Ordering::Acquire))
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for old_key in stale {
        let mut old = states
            .remove(&old_key)
            .context("Canceled Mixer owner disappeared")?;
        if let Some(mut saved) = old.preview_restore.take() {
            saved["device"]["micPreview"] = Value::Bool(false);
            let commands = old.state.clone().compile("/device/micPreview", &saved)?;
            let restored = submit_source(
                client,
                node,
                &mut old.state,
                commands,
                &AtomicBool::new(false),
                "/device/micPreview",
                false,
            )?;
            ensure!(
                restored.iter().all(|r| r.transport_completed),
                "Previous Mixer preview restore incomplete: {}",
                serde_json::to_string(&restored)?
            );
        }
    }
    if !states.contains_key(&key) {
        let response =
            client.request(ServiceRequest::AudioMixerHardwareInfo { node: node.clone() })?;
        ensure!(
            response["node"] == serde_json::to_value(node)?,
            "Mixer initial hardware identity mismatch"
        );
        let snapshot: razer_device::audio_mixer_hardware::Snapshot =
            serde_json::from_value(response["initial"].clone())?;
        watch(key.clone(), node.clone(), lifetime.clone())?;
        states.insert(
            key.clone(),
            Owner {
                node: node.clone(),
                lifetime: lifetime.clone(),
                state: State {
                    channel_mutes: snapshot.channel_mutes.map(Some),
                },
                preview_restore: None,
                lighting_sources: None,
                lighting_error: None,
            },
        );
    }
    let owner = states
        .get_mut(&key)
        .context("Mixer source owner disappeared")?;
    let commands = owner.state.clone().compile(&request.path, profile)?;
    if request.path.starts_with("/device/fader/")
        || request.path.starts_with("/device/mixerBind/inputSource/")
    {
        owner.lighting_sources = if profile["device"]["fader"]["useMode"] == 1 {
            let mut sources = [0; 4];
            for (index, source) in sources.iter_mut().enumerate() {
                let name = profile["device"]["mixerBind"]["inputSource"]
                    [format!("channel{}", index + 1)]
                .as_str()
                .context("Fader source binding absent")?;
                *source = match name {
                    "mic" => 0,
                    "chat" => 1,
                    "music" => 2,
                    "game" => 3,
                    "optical" => 4,
                    "lineIn" => 5,
                    "system" => 6,
                    _ => anyhow::bail!("Unknown fader source binding {name}"),
                };
            }
            Some(sources)
        } else {
            None
        };
    }
    if profile["device"]["micPreview"] == true || owner.preview_restore.is_some() {
        owner.preview_restore = Some(profile.clone());
    }
    let result = submit_source(
        client,
        node,
        &mut owner.state,
        commands,
        canceled,
        &request.path,
        profile["device"]["micPreview"] == true,
    );
    if let (Ok(receipts), Some(saved)) = (&result, owner.preview_restore.as_mut()) {
        for receipt in receipts {
            for (path, value) in &receipt.profile_updates {
                if let Some(slot) = saved.pointer_mut(path) {
                    *slot = value.clone();
                }
            }
        }
    }
    if result
        .as_ref()
        .is_ok_and(|receipts| receipts.iter().all(|r| r.transport_completed))
        && request.path == "/device/micPreview"
        && profile["device"]["micPreview"] == false
    {
        owner.preview_restore = None;
    }
    result
}
