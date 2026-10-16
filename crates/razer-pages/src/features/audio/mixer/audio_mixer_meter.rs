//! Current MW5736 te/ee: alpha .8, capture 80 ms, display 40 ms,
//! and a 500 ms last-raw fallback. Only native producer observations enter here.
use super::{AudioMixerOperation, AudioMixerRequest, AudioProductWorkspace};
use gpui_kit::{Context, Task};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(super) struct State {
    cancellation: Option<Arc<AtomicBool>>,
    generation: u64,
    smoothing: [Smoothing; 5],
    page: String,
    pub values: [[f32; 2]; 5],
    delayed: Vec<(Task<()>, Arc<AtomicBool>)>,
    fallback: Option<Task<()>>,
    error: Option<String>,
}

impl State {
    pub fn cancel(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.store(true, Ordering::Release);
        }
        self.generation = self.generation.wrapping_add(1);
        self.delayed.clear();
        self.fallback = None;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Default)]
struct Smoothing {
    ema: Option<[f32; 2]>,
    previous: Option<[f32; 2]>,
}
impl Smoothing {
    fn observe(&mut self, raw: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let previous_ema = self.ema.unwrap_or(raw);
        let ema = std::array::from_fn(|i| 0.8 * raw[i] + 0.2 * previous_ema[i]);
        let target = ema.map(|value| value.round());
        let midpoint = std::array::from_fn(|i| {
            let previous = self.previous.map_or(0., |values| values[i]);
            if previous != 0. {
                ((target[i] + previous) / 2.).floor()
            } else {
                target[i]
            }
        });
        self.ema = Some(ema);
        self.previous = Some(target);
        (midpoint, target)
    }
}

impl AudioProductWorkspace {
    pub(super) fn sync_mixer_meter_activity(&mut self, cx: &mut Context<Self>) {
        let active = self.spec.product_id == 1342
            && self.mixer_io.active
            && matches!(self.page.as_str(), "TAB_MIC" | "TAB_MIXER");
        if !active && self.mixer_meter.cancellation.is_none() {
            return;
        }
        if active && self.mixer_meter.cancellation.is_some() && self.mixer_meter.page == self.page {
            return;
        }
        self.mixer_meter.cancel();
        if !active {
            return;
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        self.mixer_meter.cancellation = Some(cancellation.clone());
        self.mixer_meter.error = None;
        self.mixer_meter.page = self.page.clone();
        cx.emit(AudioMixerRequest {
            generation: self.mixer_meter.generation,
            operation: AudioMixerOperation::MeterObserve,
            path: if self.page == "TAB_MIXER" {
                "/device/audioMeter/mixer"
            } else {
                "/device/audioMeter/mic"
            }
            .into(),
            target: razer_device::audio_mixer::MixerTarget {
                control: razer_device::audio_mixer::MixerControl::MicrophonePeak,
                band: None,
                channel: razer_device::audio_mixer::MixerChannel::Both,
            },
            value: None,
            eq_bands: None,
            local_profile: None,
            local_lifetime: Some(cancellation),
            epoch: self.mixer_io.epoch,
            edit_revision: 0,
        });
    }

    pub fn mixer_meter_current(&self, request: &AudioMixerRequest) -> bool {
        self.spec.product_id == 1342
            && self.mixer_io.active
            && matches!(self.page.as_str(), "TAB_MIC" | "TAB_MIXER")
            && request.epoch == self.mixer_io.epoch
            && request.generation == self.mixer_meter.generation
            && self
                .mixer_meter
                .cancellation
                .as_ref()
                .is_some_and(|cancel| !cancel.load(Ordering::Acquire))
    }

    pub fn observe_mixer_meter(
        &mut self,
        request: &AudioMixerRequest,
        result: Result<[[f32; 2]; 5], String>,
        cx: &mut Context<Self>,
    ) {
        if !self.mixer_meter_current(request) {
            return;
        }
        let raw = match result {
            Ok(raw) if raw.iter().flatten().all(|value| value.is_finite()) => raw,
            Ok(_) => {
                self.mixer_meter.error = Some("Audio meter returned nonfinite values".into());
                return;
            }
            Err(error) => {
                eprintln!("Audio Mixer meter: {error}");
                self.mixer_meter.error = Some(error);
                return;
            }
        };
        self.mixer_meter.error = None;
        let smoothed: [([f32; 2], [f32; 2]); 5] =
            std::array::from_fn(|index| self.mixer_meter.smoothing[index].observe(raw[index]));
        self.mixer_meter.values = smoothed.map(|(midpoint, _)| midpoint);
        let target = smoothed.map(|(_, target)| target);
        if let Some(mic) = &mut self.mixer_mic {
            mic.meter = Some(self.mixer_meter.values[0]);
        }
        let generation = self.mixer_meter.generation;
        // MW5736 te/ee leaves every 40 ms interpolation pending on new input.
        // Reap only completed tasks; a burst must not cancel earlier samples.
        self.mixer_meter
            .delayed
            .retain(|(_, complete)| !complete.load(Ordering::Acquire));
        let complete = Arc::new(AtomicBool::new(false));
        let finished = complete.clone();
        let task = cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(40))
                .await;
            let _ = owner.update(cx, |this, cx| {
                if this.mixer_meter.generation == generation {
                    this.mixer_meter.values = target;
                    if let Some(mic) = &mut this.mixer_mic {
                        mic.meter = Some(target[0]);
                    }
                    cx.notify();
                }
            });
            finished.store(true, Ordering::Release);
        });
        self.mixer_meter.delayed.push((task, complete));
        self.mixer_meter.fallback = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            let _ = owner.update(cx, |this, cx| {
                if this.mixer_meter.generation == generation {
                    this.mixer_meter.values = raw;
                    if let Some(mic) = &mut this.mixer_mic {
                        mic.meter = Some(raw[0]);
                    }
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::Smoothing;
    #[test]
    fn source_ema_keeps_unrounded_accumulator_and_zero_previous_skips_midpoint() {
        let mut smoothing = Smoothing::default();
        assert_eq!(smoothing.observe([0., 100.]), ([0., 100.], [0., 100.]));
        assert_eq!(smoothing.observe([10., 0.]), ([8., 60.], [8., 20.]));
        assert_eq!(smoothing.observe([9., 1.]), ([8., 12.], [9., 5.]));
    }
}
