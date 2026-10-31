//! Current PID1342 useLU/usePU: two number/range rows and CU rotary controls.
//! UI main.2623357c.js (7135bf1ec237047bc3c0ca25710aa39b02c6fa9516b7aa1425f743e0c57010e9),
//! CSS main.07a2c4ed.css (23582a2533f63335b3a2b40f0849f860e1f728bab81f87cc1546ed9c56703752).
//! Each mounted card owns one 500ms N/C timer; number inputs dispatch immediately.
use super::audio_mixer_knob::MixerKnob;
use super::audio_mixer_theme::Colors as C;
use super::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use razer_widgets::source_slider::SourceSlider;

#[derive(Default)]
pub(super) struct State {
    pub pending: BTreeMap<String, Task<()>>,
    pub dispatching: bool,
    inputs: BTreeMap<String, Entity<InputState>>,
    fallback: BTreeMap<String, f32>,
}

fn base(path: &str) -> Option<&'static str> {
    ["/device/noiseGate", "/device/compressor"]
        .into_iter()
        .find(|base| path.starts_with(&format!("{base}/")))
}

fn input_value(text: &str, min: f32, max: f32, integer_only: bool) -> Option<Value> {
    if integer_only && text.contains('.') {
        return None;
    }
    // An HTML number control exposes invalid intermediate input as empty.
    if text.is_empty() || matches!(text, "-" | "+") {
        return Some(json!(""));
    }
    let number = text.parse::<f64>().ok()?;
    (number.is_finite() && number >= f64::from(min) && number <= f64::from(max))
        .then(|| json!(number))
}

impl AudioProductWorkspace {
    pub(super) fn initialize_mixer_dynamics(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1342 {
            return;
        }
        for path in [
            "/device/noiseGate/threshold/value",
            "/device/noiseGate/targetGain/value",
            "/device/compressor/threshold/value",
            "/device/compressor/softKneeWidth/value",
        ] {
            let control = self.control(path).expect("source dynamics number control");
            let (min, max) = (control.min, control.max);
            let input = cx.new(|cx| InputState::new(window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, input, event, window, cx| {
                    if this.syncing || !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let text = input.read(cx).value().to_string();
                    if let Some(value) = input_value(&text, min, max, path.contains("noiseGate")) {
                        if let Some(number) = value.as_f64() {
                            // useLU's targetGain fallback intentionally accepts only >=0.
                            let retain = if path.contains("/threshold/") {
                                number <= 0.
                            } else {
                                number >= 0.
                            };
                            if retain {
                                this.mixer_dynamics
                                    .fallback
                                    .insert(path.into(), number as f32);
                            }
                        }
                        this.edit(path, value, window, cx);
                    } else {
                        this.syncing = true;
                        this.sync_mixer_dynamics(window, cx);
                        this.syncing = false;
                    }
                },
            ));
            self.mixer_dynamics.inputs.insert(path.into(), input);
        }
    }

    pub(super) fn sync_mixer_dynamics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (path, input) in &self.mixer_dynamics.inputs {
            let value = self.draft.pointer(path);
            let text = value
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string())
                })
                .unwrap_or_default();
            if input.read(cx).value().to_string() != text {
                input.update(cx, |input, cx| input.set_value(text, window, cx));
            }
            if value.is_some_and(|v| v.as_str() == Some("")) {
                if let Some(slider) = self.sliders.get(path) {
                    let fallback = self
                        .mixer_dynamics
                        .fallback
                        .get(path)
                        .copied()
                        .unwrap_or(0.);
                    slider.update(cx, |slider, cx| slider.set_value(fallback, window, cx));
                }
            }
        }
    }

    pub(super) fn preserves_mixer_dynamics(&self, path: &str) -> bool {
        base(path).is_some_and(|base| self.mixer_dynamics.pending.contains_key(base))
    }

    pub(super) fn mixer_dynamics_slider(
        &mut self,
        path: &str,
        event: &SliderEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 1342 {
            return false;
        }
        let Some(base) = base(path) else {
            return false;
        };
        if let SliderEvent::Change(value) = event {
            let path = path.to_owned();
            let value = value.start();
            let epoch = self.mixer_io.epoch;
            let timer = cx.spawn_in(window, async move |owner, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let _ = owner.update_in(cx, |this, window, cx| {
                    this.mixer_dynamics.pending.remove(base);
                    if this.mixer_io.epoch == epoch {
                        this.mixer_dynamics.dispatching = true;
                        this.edit(&path, json!(value), window, cx);
                        this.mixer_dynamics.dispatching = false;
                    }
                });
            });
            self.mixer_dynamics.pending.insert(base.into(), timer);
            cx.notify();
        }
        true
    }

    pub(super) fn mixer_dynamics_panel(
        &self,
        section: &AudioSection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let noise = section.title == "NOISE_GATE";
        let base = if noise {
            "/device/noiseGate"
        } else {
            "/device/compressor"
        };
        let toggle_path = format!("{base}/isEnabled");
        let enabled = self
            .draft
            .pointer(&toggle_path)
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mode_path = format!("{base}/useMode");
        let advanced = self.draft.pointer(&mode_path).and_then(Value::as_u64) == Some(1);
        let help_keys: &[&str] = if noise {
            &[
                "NOISE_GATE_TOOLTIP",
                "THRESHOLD",
                "NOISE_GATE_THRESHOLD_TOOLTIP",
                "REDUCTION",
                "NOISE_GATE_REDUCTION_TOOLTIP",
                "ATTACK",
                "NOISE_GATE_ATTACK_TOOLTIP",
                "RELEASE",
                "NOISE_GATE_RELEASE_TOOLTIP",
            ]
        } else {
            &[
                "COMPRESSOR_TOOLTIP",
                "THRESHOLD",
                "COMPRESSOR_THRESHOLD_TOOLTIP",
                "SOFT_KNEE_WIDTH",
                "COMPRESSOR_SOFT_KNEE_WIDTH_TOOLTIP",
                "RATIO",
                "COMPRESSOR_RATIO_TOOLTIP",
                "GAIN",
                "COMPRESSOR_GAIN_TOOLTIP",
                "ATTACK",
                "COMPRESSOR_ATTACK_TOOLTIP",
                "RELEASE",
                "COMPRESSOR_RELEASE_TOOLTIP",
            ]
        };
        let panel = surface::panel_with_title_switch(
            t(&section.title),
            surface::SynapseSwitch::new(SharedString::from(format!("mixer-dynamics-{base}")))
                .accessibility_label(t(&section.title))
                .checked(enabled)
                .on_change(cx.listener(move |this, next: &bool, window, cx| {
                    this.edit(&toggle_path, json!(*next), window, cx)
                })),
            surface::help_control(
                SharedString::from(format!("mixer-dynamics-help-{base}")),
                help_keys
                    .iter()
                    .map(|key| t(key).to_string())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            ),
            cx,
        );
        let mut body = v_flex().gap_0().when(!enabled, |body| body.opacity(0.3));
        let controls = section.controls.iter().filter(|control| {
            control.kind == "slider"
                && super::super::audio_mixer::accepts_mode(&control.path, &self.draft)
        });
        let mut knobs = h_flex()
            .items_start()
            .when(!noise, |row| row.justify_between());
        for control in controls {
            let Some(slider) = self.sliders.get(&control.path) else {
                continue;
            };
            if control.path.contains("/attackTime/")
                || control.path.contains("/releaseTime/")
                || control.path.contains("/ratio/")
                || control.path.contains("/makeUpGain/")
            {
                knobs = knobs.child(
                    div()
                        .mt(surface::css(25.))
                        .when(noise, |knob| knob.mr(surface::css(60.)))
                        // usePU also labels ratio and makeup gain "ms" in the original.
                        .child(MixerKnob::new(slider, t(&control.label), "ms", enabled)),
                );
                continue;
            }
            let value = slider.read(cx).value().start();
            let number = self.mixer_dynamics.inputs.get(&control.path);
            let mut row = v_flex().gap_0();
            if let Some(input) = number {
                let empty = self
                    .draft
                    .pointer(&control.path)
                    .is_some_and(|v| v.as_str() == Some(""));
                let label = if control.path.contains("/targetGain/") {
                    "REDUCTION"
                } else {
                    &control.label
                };
                row = row.mb(surface::css(20.)).child(
                    h_flex()
                        .mb(surface::css(20.))
                        .child(t(label))
                        .child(
                            Input::new(input)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .disabled(!enabled)
                                .w(surface::css(50.))
                                .h(surface::css(27.))
                                .mx(surface::css(10.))
                                .px(surface::css(10.))
                                .py(surface::css(5.))
                                .border_1()
                                .border_color(if empty {
                                    rgb(0xff0000).into()
                                } else {
                                    C::border()
                                })
                                .rounded_none()
                                .text_color(C::value())
                                .text_size(surface::css(14.)),
                        )
                        .child("dB"),
                );
            }
            row = row.child(
                SourceSlider::new(slider, (value - control.min) / (control.max - control.min))
                    .enabled(true)
                    .interaction_enabled(enabled)
                    .tip(number.is_none().then(|| format!("{value:.0}")))
                    .when(number.is_some(), |slider| {
                        slider.range_labels(
                            format!("{} dB", control.min),
                            format!("{} dB", control.max),
                            C::muted(),
                        )
                    }),
            );
            body = body.child(row);
        }
        if advanced {
            body = body.child(knobs);
        }
        panel
            .child(body)
            .child(
                razer_widgets::keyboard_navigation::Button::new(SharedString::from(format!(
                    "mixer-dynamics-mode-{base}"
                )))
                .mt(surface::css(20.))
                .text_size(surface::css(14.))
                .text_color(C::text())
                .underline()
                .child(t(if advanced { "SHOW_LESS" } else { "SHOW_MORE" }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(&mode_path, json!(u8::from(!advanced)), window, cx)
                })),
            )
            .into_any_element()
    }
}
