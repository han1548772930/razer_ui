//! UI7405 setup snapshot/defaults, measurement and the exact Cancel restore.
use super::*;

pub(super) struct State {
    pub(super) snapshot: Value,
    pub(super) intro: bool,
    pub(super) step: usize,
    pub(super) ai: Option<usize>,
    pub(super) eq: usize,
    pub(super) distance: usize,
    pub(super) ai_result: u8,
    pub(super) eq_result: usize,
    pub(super) bass_result: i8,
    pub(super) exciter_result: i8,
    pub(super) speech_done: bool,
    pub(super) environment_done: bool,
    pub(super) gain: Option<f64>,
    pub(super) gain_percent: Option<f64>,
    pub(super) noise: Option<f64>,
    pub(super) fader: Option<Entity<SliderState>>,
    fader_subscription: Option<Subscription>,
    pub(super) levels: [f32; 2],
    pub(super) peaks: [f32; 2],
    peak_task: Option<Task<()>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: Value::Null,
            intro: false,
            step: 0,
            ai: None,
            eq: 0,
            distance: 0,
            ai_result: 0,
            eq_result: 0,
            bass_result: 0,
            exciter_result: 2,
            speech_done: false,
            environment_done: false,
            gain: None,
            gain_percent: None,
            noise: None,
            fader: None,
            fader_subscription: None,
            levels: [0.; 2],
            peaks: [0.; 2],
            peak_task: None,
        }
    }
}
impl AudioProductWorkspace {
    pub(super) fn open_seiren_microphone_wizard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.seiren_mic.wizard.is_some() {
            return;
        }
        let mut snapshot = self.draft.clone();
        snapshot["profile"]["micVolume"] = self.seiren_eq.live.endpoint(razer_device::seiren_mixer::MICROPHONE)
            .and_then(|reading| reading.value())
            .map(|value| json!({"value":value.level,"isEnabled":!value.muted})).unwrap_or(Value::Null);
        self.seiren_mic.wizard = Some(State {
            snapshot,
            intro: true,
            ..Default::default()
        });
        let value = self.seiren_microphone_volume_preview()["value"]
            .as_f64()
            .unwrap_or(0.) as f32;
        let fader = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(value)
        });
        let subscription =
            cx.subscribe_in(&fader, window, |this, _, event: &SliderEvent, _, cx| {
                let SliderEvent::Change(value) = event else {
                    return;
                };
                let mut volume = this.seiren_microphone_volume_preview().clone();
                if !this.seiren_endpoint_available(0) || volume["isEnabled"] != true {
                    return;
                }
                let value = f64::from(value.start());
                if (value - volume["value"].as_f64().unwrap_or(0.)).abs() > 0.01 {
                    volume["value"] = json!(value);
                    this.seiren_microphone_set_volume(volume, cx);
                }
            });
        let wizard = self.seiren_mic.wizard.as_mut().unwrap();
        wizard.fader = Some(fader);
        wizard.fader_subscription = Some(subscription);
        wizard.peak_task = Some(cx.spawn(async move |owner, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let alive = owner.update(cx, |this, cx| {
                    let levels = if this
                        .seiren_mic
                        .measurement
                        .as_ref()
                        .is_some_and(|capture| capture.started.is_some())
                    {
                        this.seiren_mic
                            .wizard
                            .as_ref()
                            .map(|wizard| wizard.levels)
                            .unwrap_or_default()
                    } else {
                        this.seiren_microphone_native_levels()
                    };
                    let Some(wizard) = &mut this.seiren_mic.wizard else {
                        return false;
                    };
                    for (peak, level) in wizard.peaks.iter_mut().zip(levels) {
                        *peak = if level > *peak {
                            level
                        } else {
                            (*peak - 0.5).max(0.)
                        };
                    }
                    cx.notify();
                    true
                });
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        }));
        if let Some(focus) = &self.seiren_mic.focus {
            focus.focus(window, cx);
        }
        self.seiren_mic.wizard_defaults = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = owner.update(cx, |this, cx| {
                if this.seiren_mic.wizard.is_some() {
                    this.seiren_wizard_defaults(cx);
                }
            });
        }));
        cx.notify();
    }
    fn seiren_wizard_defaults(&mut self, cx: &mut Context<Self>) {
        self.seiren_microphone_set_volume(
            json!({"isEnabled":true,"value":mic::db_to_percent(3.)}),
            cx,
        );
        for (field, module, payload) in [
            (
                "noise_gate",
                2,
                json!({"isEnabled":false,"thresholdDB":-60,"atTime":10,"rlTime":150,"holdTime":50,"decayDB":-6}),
            ),
            (
                "ai_noise_suppression",
                3,
                json!({"isEnabled":false,"value":0}),
            ),
            ("mic_compressor", 4, json!({"isEnabled":false})),
            ("mic_limiter", 5, json!({"isEnabled":true})),
            ("mic_reverb", 6, json!({"isEnabled":false,"value":0})),
        ] {
            self.seiren_wizard_dsp(field, module, payload, cx);
        }
        for (field, operation) in [
            ("vocal_bass", "ON_SET_VOCAL_BASS"),
            ("vocal_exciter", "ON_SET_VOCAL_EXCITER"),
        ] {
            self.seiren_wizard_effect(
                field,
                operation,
                json!({"isEnabled":false,"value":50,"min":0,"max":100,"step":1}),
                cx,
            );
        }
    }
    fn seiren_wizard_dsp(
        &mut self,
        field: &str,
        module: u8,
        payload: Value,
        cx: &mut Context<Self>,
    ) {
        self.draft["device"][field] = payload.clone();
        self.submit_seiren_eq_change(
            razer_device::seiren_eq::Change::MicrophoneDsp { module, payload },
            cx,
        );
    }
    fn seiren_wizard_effect(
        &mut self,
        field: &str,
        operation: &str,
        payload: Value,
        cx: &mut Context<Self>,
    ) {
        self.draft["device"]["eqEffects"][field] = payload.clone();
        self.seiren_eq.effects_committed = self.draft["device"]["eqEffects"].clone();
        self.submit_seiren_eq_payload(operation, payload, cx);
    }
    pub(super) fn start_seiren_wizard_measurement(&mut self, cx: &mut Context<Self>) {
        if self.seiren_mic.measurement.is_some() {
            return;
        }
        let Some(step) = self.seiren_mic.wizard.as_ref().map(|wizard| wizard.step) else {
            return;
        };
        let purpose = if step == 0 {
            CapturePurpose::Speech
        } else if step == 1 {
            CapturePurpose::Environment
        } else {
            return;
        };
        let request = SeirenAudioCaptureRequested::new(
            cx.entity_id(),
            self.spec.product_id,
            CaptureOptions::new(purpose, Some(48_000)).expect("source measurement rate"),
        );
        self.seiren_mic.recording_error = None;
        let mut measurement = CaptureOwner::new(request.clone());
        measurement.dispatched = false;
        self.seiren_mic.measurement = Some(measurement);
        let delay = measurement_cooldown(self.seiren_mic.measurement_last_stop, Instant::now());
        self.seiren_mic.measurement_start_delay = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor().timer(delay).await;
            let _ = owner.update(cx, |this, cx| {
                let Some(measurement) = &mut this.seiren_mic.measurement else {
                    return;
                };
                if measurement.request.same_owner(&request) && !request.is_canceled() {
                    measurement.dispatched = true;
                    cx.emit(request);
                }
            });
        }));
        cx.notify();
    }
    pub(super) fn start_seiren_wizard_measurement_timer(
        &mut self,
        request: SeirenAudioCaptureRequested,
        cx: &mut Context<Self>,
    ) {
        // The shell validates that an observed frame follows successful audio
        // initialization. Source sa arms its duration timer after getUserMedia
        // and AudioContext.resume, rather than on the activating click.
        self.seiren_mic.measurement_timer = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(10))
                .await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(measurement) = &mut this.seiren_mic.measurement {
                    if measurement.request.same_owner(&request) {
                        this.stop_seiren_wizard_measurement(true);
                        cx.notify();
                    }
                }
            });
        }));
    }
    pub(super) fn stop_seiren_wizard_measurement(&mut self, accept_final: bool) {
        self.seiren_mic.measurement_start_delay = None;
        self.seiren_mic.measurement_timer = None;
        if let Some(measurement) = &mut self.seiren_mic.measurement {
            measurement.stop(accept_final);
            self.seiren_mic.measurement_last_stop = measurement.stopped;
            // No worker owns a session during the source 750ms cooldown.
            // Releasing this reservation cannot wait for a nonexistent reply.
            if !measurement.dispatched {
                self.seiren_mic.measurement = None;
            }
        }
    }
    pub(super) fn finish_seiren_microphone_measurement(
        &mut self,
        purpose: CapturePurpose,
        clip: &PcmClip,
        cx: &mut Context<Self>,
    ) {
        self.seiren_mic.measurement_timer = None;
        if self.seiren_mic.wizard.is_none() {
            return;
        }
        match purpose {
            CapturePurpose::Speech => {
                let result = mic::speech_measurement(clip.samples(), clip.sample_rate());
                // Current caller marks the test completed after the analysis
                // callback even when it rejects the metric; retain that quirk.
                self.seiren_mic.wizard.as_mut().unwrap().speech_done = true;
                if let Some(result) = result
                    .filter(|result| result.metric_db.is_finite() && result.metric_db >= -100.)
                {
                    let Some(volume) = self.seiren_eq.live.endpoint(razer_device::seiren_mixer::MICROPHONE).and_then(|reading| reading.value()) else { return; };
                    let current = f64::from(volume.level);
                    let gain = mic::percent_to_db(current) + result.recommended_db;
                    let percent = (mic::db_to_percent(gain).clamp(0., 100.) * 100.).round() / 100.;
                    let wizard = self.seiren_mic.wizard.as_mut().unwrap();
                    wizard.gain = Some(gain.clamp(0., 36.));
                    wizard.gain_percent = Some(percent);
                    self.seiren_microphone_set_volume(
                        json!({"isEnabled":true,"value":percent}),
                        cx,
                    );
                }
            }
            CapturePurpose::Environment => {
                let wizard = self.seiren_mic.wizard.as_mut().unwrap();
                wizard.environment_done = true;
                wizard.noise = mic::environment_measurement(clip.samples(), clip.sample_rate());
            }
            _ => {}
        }
    }
    pub(super) fn seiren_wizard_select(
        &mut self,
        step: usize,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(wizard) = &mut self.seiren_mic.wizard else {
            return;
        };
        match step {
            2 if index < 3 => {
                wizard.ai = Some(index);
                let values = [0, 50, 75];
                wizard.ai_result = values[index];
                self.seiren_wizard_dsp(
                    "ai_noise_suppression",
                    3,
                    json!({"isEnabled":true,"value":values[index]}),
                    cx,
                );
            }
            3 if index < 5 => {
                wizard.eq = index;
                wizard.eq_result = index;
                self.seiren_select_preset(
                    ["podcast", "arena", "fpscallout", "studio", "custom"][index],
                    window,
                    cx,
                );
            }
            4 if index < 2 => {
                wizard.distance = index;
                wizard.bass_result = if index == 0 { 0 } else { 3 };
                wizard.exciter_result = 2;
                let effects = &self.draft["device"]["eqEffects"];
                let payload = |field: &str, enabled: bool, value: u8| {
                    let mut payload =
                        json!({"isEnabled":true,"value":50,"min":0,"max":100,"step":1});
                    if let Some(current) = effects[field].as_object() {
                        payload.as_object_mut().unwrap().extend(current.clone());
                    }
                    payload["isEnabled"] = json!(enabled);
                    payload["value"] = json!(value);
                    payload
                };
                let bass = payload("vocal_bass", index == 1, if index == 0 { 50 } else { 80 });
                let exciter = payload("vocal_exciter", true, 70);
                self.seiren_wizard_effect("vocal_bass", "ON_SET_VOCAL_BASS", bass, cx);
                self.seiren_wizard_effect("vocal_exciter", "ON_SET_VOCAL_EXCITER", exciter, cx);
            }
            _ => {}
        }
        cx.notify();
    }
    pub(super) fn seiren_wizard_next(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.seiren_mic.wizard else {
            return;
        };
        if wizard.intro {
            wizard.intro = false;
            cx.notify();
            return;
        }
        if (wizard.step == 0 && !wizard.speech_done)
            || (wizard.step == 1 && !wizard.environment_done)
        {
            return;
        }
        let noise = (wizard.step == 1).then_some(wizard.noise).flatten();
        wizard.step = (wizard.step + 1).min(5);
        if let Some(noise) = noise {
            self.seiren_wizard_noise(noise, cx);
        }
        cx.notify();
    }
    pub(super) fn seiren_wizard_back(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.seiren_mic.wizard {
            if wizard.step == 0 {
                wizard.intro = true;
            } else {
                wizard.step -= 1;
            }
            cx.notify();
        }
    }
    fn seiren_wizard_noise(&mut self, noise: f64, cx: &mut Context<Self>) {
        self.seiren_wizard_dsp("noise_gate",2,json!({"isEnabled":true,"thresholdDB":noise,"atTime":4,"holdTime":40,"rlTime":400,"decayDB":-120}),cx);
    }
    pub(super) fn restart_seiren_wizard(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.seiren_mic.wizard {
            let snapshot = wizard.snapshot.clone();
            *wizard = State {
                snapshot,
                ai: wizard.ai,
                eq: wizard.eq,
                distance: wizard.distance,
                // Source B resets results, but retains the selection object x.
                // Its literal "Podcast" falls through the usage map to Custom.
                eq_result: 4,
                exciter_result: 0,
                fader: wizard.fader.take(),
                fader_subscription: wizard.fader_subscription.take(),
                peak_task: wizard.peak_task.take(),
                ..Default::default()
            };
        }
        self.seiren_wizard_defaults(cx);
        cx.notify();
    }
    pub(super) fn close_seiren_microphone_wizard(
        &mut self,
        apply: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(wizard) = self.seiren_mic.wizard.take() else {
            return;
        };
        self.seiren_mic.wizard_defaults = None;
        self.stop_seiren_wizard_measurement(false);
        if apply {
            if let Some(percent) = wizard.gain_percent {
                self.seiren_microphone_set_volume(json!({"isEnabled":true,"value":percent}), cx);
            }
            if let Some(noise) = wizard.noise {
                self.seiren_wizard_noise(noise, cx);
            }
            // Source Apply replays every selected side effect.
            self.seiren_mic.wizard = Some(wizard);
            for step in 2..=4 {
                let wizard = self.seiren_mic.wizard.as_ref().unwrap();
                let index = match step {
                    2 => wizard.ai,
                    3 => Some(wizard.eq),
                    _ => Some(wizard.distance),
                };
                if let Some(index) = index {
                    self.seiren_wizard_select(step, index, window, cx);
                }
            }
            self.seiren_mic.wizard = None;
        } else {
            let before = wizard.snapshot;
            if let Some(value) = before.pointer("/profile/micVolume").filter(|value| !value.is_null()) {
                self.seiren_microphone_set_volume(value.clone(), cx);
            }
            for (field, module) in [
                ("noise_gate", 2),
                ("ai_noise_suppression", 3),
                ("mic_compressor", 4),
                ("mic_limiter", 5),
                ("mic_reverb", 6),
            ] {
                if let Some(value) = before.pointer(&format!("/device/{field}")) {
                    self.seiren_wizard_dsp(field, module, value.clone(), cx);
                }
            }
            if let Some(preset) = before
                .pointer("/device/parametric/selectedPreset")
                .and_then(Value::as_str)
            {
                self.seiren_select_preset(preset, window, cx);
            }
            for (field, operation) in [
                ("vocal_bass", "ON_SET_VOCAL_BASS"),
                ("vocal_exciter", "ON_SET_VOCAL_EXCITER"),
            ] {
                if let Some(value) = before.pointer(&format!("/device/eqEffects/{field}")) {
                    self.seiren_wizard_effect(field, operation, value.clone(), cx);
                }
            }
        }
        self.seiren_mic.setup_seen = Some(true);
        cx.emit(SeirenMicrophoneSetupSeen(true));
        if let Some(focus) = &self.seiren_mic.trigger_focus {
            focus.focus(window, cx);
        }
        cx.notify();
    }
}

fn measurement_cooldown(last_stop: Option<Instant>, now: Instant) -> Duration {
    last_stop.map_or(Duration::ZERO, |last_stop| {
        Duration::from_millis(750).saturating_sub(now.saturating_duration_since(last_stop))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn microphone_measurement_cooldown_uses_the_previous_stop_instant() {
        let stopped = Instant::now();
        assert_eq!(measurement_cooldown(None, stopped), Duration::ZERO);
        assert_eq!(
            measurement_cooldown(Some(stopped), stopped),
            Duration::from_millis(750)
        );
        assert_eq!(
            measurement_cooldown(Some(stopped), stopped + Duration::from_millis(200)),
            Duration::from_millis(550)
        );
        assert_eq!(
            measurement_cooldown(Some(stopped), stopped + Duration::from_secs(1)),
            Duration::ZERO
        );
    }
}
