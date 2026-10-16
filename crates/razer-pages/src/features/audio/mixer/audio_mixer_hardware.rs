//! MW5736 B/x/G/L/w/U and source taskMaker hardware actions.
use super::*;
use razer_device::audio_mixer_hardware::Snapshot;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

#[derive(Default)]
pub(super) struct State {
    cancel: Option<Arc<AtomicBool>>,
    generation: u64,
    previous: Option<Snapshot>,
    channel_mutes: [Option<bool>; 4],
    last_fader: BTreeMap<String, Instant>,
    pending: BTreeMap<String, Task<()>>,
    pub error: Option<String>,
}
impl State {
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Release);
        }
        self.generation = self.generation.wrapping_add(1);
        self.previous = None;
        self.pending.clear();
        self.last_fader.clear();
    }
    pub fn receipts(&mut self, receipts: &[razer_device::audio_mixer_local::Receipt]) {
        for receipt in receipts {
            if let razer_device::audio_mixer_local::Command::ChannelMute {
                channel, enabled, ..
            } = &receipt.command
            {
                if (1..=4).contains(channel) {
                    self.channel_mutes[*channel as usize - 1] = Some(*enabled);
                }
            }
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl AudioProductWorkspace {
    pub(super) fn sync_mixer_hardware_activity(&mut self, cx: &mut Context<Self>) {
        let active = self.spec.product_id == 1342 && self.mixer_io.active;
        if active == self.mixer_hardware.cancel.is_some() {
            return;
        }
        self.mixer_hardware.cancel();
        if !active {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.mixer_hardware.cancel = Some(cancel.clone());
        cx.emit(AudioMixerRequest {
            generation: self.mixer_hardware.generation,
            operation: AudioMixerOperation::HardwareObserve,
            path: "/device/hardwareEvents".into(),
            target: crate::features::audio_mixer::AudioMixerPath::MicMonitorVolume.target(),
            value: None,
            eq_bands: None,
            local_profile: None,
            local_lifetime: Some(cancel),
            epoch: self.mixer_io.epoch,
            edit_revision: 0,
        });
    }
    pub fn mixer_hardware_current(&self, request: &AudioMixerRequest) -> bool {
        self.spec.product_id == 1342
            && self.mixer_io.active
            && request.epoch == self.mixer_io.epoch
            && request.generation == self.mixer_hardware.generation
            && self
                .mixer_hardware
                .cancel
                .as_ref()
                .is_some_and(|c| !c.load(Ordering::Acquire))
    }
    fn hardware_patch(
        &mut self,
        path: &str,
        value: Value,
        write: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mixer_profile_routing.get_or_insert_with(|| {
            audio_mixer_local_state::ProfileRouting::from_draft(&self.draft)
        });
        let Some(target) = self.draft.pointer_mut(path) else {
            self.mixer_hardware.error = Some(format!("Hardware source path absent: {path}"));
            return;
        };
        *target = value;
        if let Some(routing) = &mut self.mixer_profile_routing {
            routing.retain_edit(path, &self.draft);
        }
        self.retain_active_effect(path);
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        if write {
            self.request_mixer_write(path, None, cx);
        }
        cx.notify();
    }
    fn hardware_routes(
        &mut self,
        input: &str,
        binding: &str,
        muted: Option<bool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (name, bus) in [
            ("playbackMix", "playbackMixReducer"),
            ("streamMix", "streamMixReducer"),
            ("lineOut", "lineOutReducer"),
            ("voiceChat", "voiceChatReducer"),
        ] {
            if binding != "muteAll" && binding != name {
                continue;
            }
            if name == "voiceChat" && input == "chat" {
                continue;
            }
            let path = format!("/device/{bus}/{input}/isMuted");
            let saved = self.snapshot();
            let value = muted
                .unwrap_or_else(|| saved.pointer(&path).and_then(Value::as_bool) != Some(true));
            self.hardware_patch(&path, json!(value), true, window, cx);
        }
    }
    fn hardware_binding(&mut self, binding: &str, window: &mut Window, cx: &mut Context<Self>) {
        match binding {
            "muteAll" => {
                for input in [
                    "mic", "chat", "music", "game", "optical", "lineIn", "system",
                ] {
                    self.hardware_routes(input, "muteAll", None, window, cx);
                }
            }
            "mic" | "chat" | "music" | "game" | "optical" | "lineIn" | "system" => {
                self.hardware_routes(binding, "muteAll", None, window, cx)
            }
            "playbackMix" | "streamMix" | "lineOut" | "voiceChat" => {
                let path = format!("/device/{binding}Reducer/volume/isEnabled");
                let value = self.snapshot().pointer(&path).and_then(Value::as_bool) != Some(true);
                self.hardware_patch(&path, json!(value), true, window, cx);
            }
            "voiceChanger" | "echoReverb" | "keyShifter" | "vocalFading" => {
                let path = format!("/device/{binding}/isEnabled");
                let value = self.snapshot().pointer(&path).and_then(Value::as_bool) != Some(true);
                self.hardware_patch(&path, json!(value), true, window, cx);
            }
            "bleep" => {}
            id => self.select_effect_preset(id, window, cx),
        }
    }
    fn hardware_fader(
        &mut self,
        mut key: String,
        percent: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        use anyhow::{Context as _, ensure};
        let mut percent = f64::from(percent);
        if key == "mic" {
            key = if self.draft["device"]["micSetup"]["useMode"] == 0 {
                "xlr"
            } else {
                "mic"
            }
            .into();
            let setup = &self.draft["device"]["micSetup"][&key];
            let min = setup["min"].as_f64().context("Mic fader minimum missing")?;
            let max = setup["max"].as_f64().context("Mic fader maximum missing")?;
            let gain = setup["value"].as_f64().context("Mic fader gain missing")?;
            ensure!(max > min && max != 0., "Mic fader range empty");
            let step = setup["step"].as_f64().context("Mic fader step missing")?;
            let factor = 10_f64.powi(
                step.to_string()
                    .split_once('.')
                    .map_or(0, |(_, tail)| tail.len()) as i32,
            );
            let mapped = (((gain - min) * percent / 100. + min) * factor).round() / factor;
            let limit = if percent == 100. {
                mapped / max * 100.
            } else {
                ((mapped.clamp(min, max) - min) / (max - min) * 100. * factor).round() / factor
            };
            if percent > limit {
                percent = limit;
            }
        }
        let path = format!("/device/outputMixerReducer/{key}");
        let mut value = self
            .draft
            .pointer(&path)
            .cloned()
            .context("Mixer fader source absent")?;
        let min = value["min"]
            .as_f64()
            .context("Mixer fader minimum absent")?;
        let max = value["max"]
            .as_f64()
            .context("Mixer fader maximum absent")?;
        let step = value["step"].as_f64().context("Mixer fader step absent")?;
        let factor = 10_f64.powi(
            step.to_string()
                .split_once('.')
                .map_or(0, |(_, tail)| tail.len()) as i32,
        );
        value["value"] = json!(((min + (max - min) * percent / 100.) * factor).round() / factor);
        if let Some(object) = value.as_object_mut() {
            object.remove("isPercentage");
        }
        self.mixer_profile_routing.get_or_insert_with(|| {
            audio_mixer_local_state::ProfileRouting::from_draft(&self.draft)
        });
        // Redux handles the percent payload immediately, while taskMaker only
        // persists its converted raw value when the throttled event executes.
        self.draft["device"]["outputMixerReducer"][&key] = value.clone();
        self.mixer_channels.observe_output();
        self.sync(window, cx);
        cx.notify();
        self.mixer_hardware.pending.remove(&key);
        let now = Instant::now();
        let immediate = self
            .mixer_hardware
            .last_fader
            .get(&key)
            .is_none_or(|last| now.duration_since(*last).as_millis() > 100);
        if immediate {
            self.mixer_hardware.last_fader.insert(key, now);
            self.commit_hardware_fader(&path, value, cx);
        } else {
            let generation = self.mixer_hardware.generation;
            let task = cx.spawn(async move |owner, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let _ = owner.update(cx, |this, cx| {
                    if this.mixer_hardware.generation == generation {
                        this.commit_hardware_fader(&path, value, cx);
                    }
                });
            });
            self.mixer_hardware.pending.insert(key, task);
        }
        Ok(())
    }
    fn commit_hardware_fader(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        if let Some(routing) = &mut self.mixer_profile_routing {
            routing.retain_service_updates(&[(path.into(), value)]);
        }
        cx.emit(AudioProductChanged);
        self.request_mixer_write(&format!("{path}/value"), None, cx);
    }
    pub fn observe_mixer_hardware(
        &mut self,
        request: &AudioMixerRequest,
        result: Result<Value, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.mixer_hardware_current(request) {
            return;
        }
        let response = match result {
            Ok(v) => v,
            Err(e) => {
                self.mixer_hardware.error = Some(e);
                return;
            }
        };
        if let Some(initial) = response.get("initial") {
            match serde_json::from_value::<Snapshot>(initial.clone()) {
                Ok(snapshot) => {
                    self.mixer_hardware.channel_mutes = snapshot.channel_mutes.map(Some);
                    self.mixer_hardware.previous = Some(snapshot);
                }
                Err(e) => self.mixer_hardware.error = Some(e.to_string()),
            }
            return;
        }
        let Some(events) = response["events"].as_array() else {
            self.mixer_hardware.error = Some("Mixer hardware events absent".into());
            return;
        };
        for event in events {
            if let Some(error) = event["native_error"].as_str() {
                self.mixer_hardware.error = Some(error.into());
            }
            let snapshot = match serde_json::from_value::<Snapshot>(event["snapshot"].clone()) {
                Ok(v) => v,
                Err(e) => {
                    self.mixer_hardware.error = Some(e.to_string());
                    continue;
                }
            };
            let Some(previous) = self.mixer_hardware.previous.replace(snapshot.clone()) else {
                continue;
            };
            for index in 0..4 {
                let channel = format!("channel{}", index + 1);
                let input = self.draft["device"]["mixerBind"]["inputSource"][&channel]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                if snapshot.keys[index] && !previous.keys[index] {
                    let binding = self.draft["device"]["mixerBind"]["muteButton"][&channel]
                        .as_str()
                        .unwrap_or("")
                        .to_owned();
                    let muted = self.mixer_hardware.channel_mutes[index] != Some(true);
                    self.hardware_routes(&input, &binding, Some(muted), window, cx);
                }
                if snapshot.sliders[index] != previous.sliders[index] {
                    if let Err(error) =
                        self.hardware_fader(input, snapshot.sliders[index], window, cx)
                    {
                        self.mixer_hardware.error = Some(format!("{error:#}"));
                    }
                }
            }
            if snapshot.keys[4] && !previous.keys[4] {
                let binding = self.draft["device"]["mixerBind"]["bleepButton"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                self.hardware_binding(&binding, window, cx);
            }
            if snapshot.mic_long_press && !previous.mic_long_press {
                let binding = self.draft["device"]["mixerBind"]["longPressMicMuteButton"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                self.hardware_binding(&binding, window, cx);
            }
            if snapshot.phantom_power != previous.phantom_power {
                // ON_SET_MIC_SETUP_FROM_DEVICE stores source mic state but
                // deliberately does not enqueue another device setup write.
                self.hardware_patch(
                    "/device/micSetup/xlr/isEnabled",
                    json!(snapshot.phantom_power),
                    false,
                    window,
                    cx,
                );
            }
        }
    }
}
