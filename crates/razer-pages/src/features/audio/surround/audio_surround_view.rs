//! Source-mounted Speaker Mode, Volume Mixer, Time Alignment and EQ controls.
//! Geometry follows audio-surround-audit.json's current JS/CSS owners.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use razer_widgets::source_slider::SourceSlider;

impl AudioProductWorkspace {
    fn surround_product_name(&self) -> String {
        razer_catalog::registered(self.spec.product_id)
            .map(|product| t(product.name()))
            .unwrap_or_default()
    }
    fn surround_diagram(&self, cx: &App) -> AnyElement {
        let enabled = self.surround_mode() == Some(true)
            && (self.spec.product_id == 3942 || self.surround.controls.front().is_some());
        let asset = match (self.spec.product_id, enabled) {
            (1392, false) => "synapse/audio-surround-1392-primary.svg",
            (1392, true) => "synapse/audio-surround-1392-surround.svg",
            (1442, false) => "synapse/audio-surround-1442-primary.svg",
            (1442, true) => "synapse/audio-surround-1442-surround.svg",
            (3942, false) => "synapse/audio-surround-3942-primary.svg",
            (3942, true) => "synapse/audio-surround-3942-surround.svg",
            _ => unreachable!("Surround product was checked before mounting"),
        };
        let stereo = !enabled || self.spec.product_id == 1442;
        let mut diagram = div()
            .relative()
            .w(surface::css(311.))
            .h(surface::css(260.))
            .mt(surface::css(10.))
            .child(img(asset).w(surface::css(311.)).h(surface::css(260.)));
        for (key, x, y, width, visible) in [
            ("FRONT_LEFT", 35., 26., 43., enabled),
            ("CENTER", 116., -1., 80., enabled),
            ("FRONT_RIGHT", 233., 26., 43., enabled),
            ("REAR_LEFT", -12., 213., 43., enabled),
            ("REAR_RIGHT", 280., 213., 43., enabled),
            (
                "LEFT",
                if self.spec.product_id == 1392 { -6. } else { -25. },
                if self.spec.product_id == 1392 { 168. } else { 142. },
                43.,
                stereo,
            ),
            (
                "RIGHT",
                if self.spec.product_id == 1392 { 274. } else { 293. },
                if self.spec.product_id == 1392 { 168. } else { 142. },
                43.,
                stereo,
            ),
        ] {
            if visible {
                diagram = diagram.child(
                    div()
                        .absolute()
                        .left(surface::css(x))
                        .top(surface::css(y))
                        .w(surface::css(width))
                        .text_size(surface::css(12.))
                        .text_color(cx.theme().foreground)
                        .text_center()
                        .child(t(key)),
                );
            }
        }
        diagram
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(55.))
                    .w_full()
                    .text_size(surface::css(14.))
                    .text_center()
                    .child(self.surround_product_name()),
            )
            .into_any_element()
    }
    fn surround_mode(&self) -> Option<bool> {
        self.surround
            .pending
            .values()
            .rev()
            .find_map(|operation| match operation {
                SurroundOperation::Change(Change::Enabled(v)) => Some(*v),
                _ => None,
            })
            .or_else(|| self.surround.controls.mode().value().copied())
    }
    fn surround_enabled(&self) -> bool {
        self.surround.connected && self.surround_mode() == Some(true)
    }
    fn select_surround_mode(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.surround.connected {
            return;
        }
        if enabled && self.surround.controls.front().is_none() {
            self.surround.choosing_front = true;
            self.surround.selected = self.surround.endpoints.first().map(|e| e.id().to_owned());
            if let (Some(select), Some(id)) = (&self.surround.select, &self.surround.selected) {
                self.surround.syncing = true;
                select.update(cx, |select, cx| select.set_selected_value(id, window, cx));
                self.surround.syncing = false;
            }
            cx.notify();
        } else {
            self.surround.choosing_front = false;
            self.dispatch_surround(SurroundOperation::Change(Change::Enabled(enabled)), cx);
            if !enabled {
                self.leave_surround_page(cx);
            }
        }
    }
    fn surround_help(&self, id: &'static str, key: &'static str) -> AnyElement {
        surface::help_control(id, t(key)).into_any_element()
    }
    fn surround_horizontal(
        &self,
        control: Control,
        label: String,
        enabled: bool,
        cx: &App,
    ) -> AnyElement {
        let value = self.surround_value(control);
        let enabled = enabled && value.is_some() && self.surround.connected;
        let mut row = v_flex()
            .capture_key_down(|event, _, cx| {
                if matches!(
                    event.keystroke.key.as_str(),
                    "home" | "end" | "up" | "down" | "left" | "right"
                ) {
                    cx.stop_propagation();
                }
            })
            .gap(surface::css(8.))
            .mb(surface::css(20.))
            .child(
                h_flex()
                    .justify_between()
                    .child(label)
                    .child(value.map_or_else(|| "—".into(), |v| format!("{v:.0}"))),
            );
        if let Some(value) = value {
            row = row.child(
                SourceSlider::new(&self.surround.sliders[&control], value / 100.)
                    .tip(Some(format!("{value:.0}")))
                    .enabled(enabled),
            );
        } else {
            row = row.child(
                div()
                    .h(surface::css(64.))
                    .border_b_1()
                    .border_color(cx.theme().border),
            );
        }
        row.into_any_element()
    }
    fn surround_vertical(
        &self,
        control: Control,
        label: &str,
        enabled: bool,
        cx: &App,
    ) -> AnyElement {
        let value = self.surround_value(control);
        let state = &self.surround.sliders[&control];
        let (min, max) = control.limits();
        let enabled = enabled && value.is_some() && self.surround.connected;
        let mut body = v_flex()
            .capture_key_down(|_, _, cx| cx.stop_propagation())
            .items_center()
            .gap(surface::css(10.))
            .child(value.map_or_else(|| "—".into(), |v| format!("{v:+.0}")));
        if let Some(value) = value {
            let progress = (value - min) / (max - min);
            body = body.child(
                BaseSlider::new(state)
                    .vertical()
                    .disabled(!enabled)
                    .w(surface::css(24.))
                    .h(surface::css(284.))
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(10.))
                            .w(surface::css(4.))
                            .h_full()
                            .bg(cx.theme().border),
                    )
                    .child(
                        SliderTrack::new(state)
                            .axis(Axis::Vertical)
                            .disabled(!enabled)
                            .size_full()
                            .child(
                                SliderIndicator::new(state).size_full().relative().child(
                                    SliderThumb::new(state)
                                        .axis(Axis::Vertical)
                                        .disabled(!enabled)
                                        .absolute()
                                        .left(surface::css(3.))
                                        .bottom(relative(progress))
                                        .mb(surface::css(-7.))
                                        .w(surface::css(18.))
                                        .h(surface::css(14.))
                                        .bg(cx.theme().primary),
                                ),
                            ),
                    ),
            );
        } else {
            body = body.child(div().w(surface::css(24.)).h(surface::css(284.)));
        }
        body.child(t(label))
            .opacity(if enabled { 1. } else { 0.3 })
            .into_any_element()
    }
    fn surround_speaker_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.surround_mode();
        let mut column = v_flex().gap(surface::css(20.)).flex_1().min_w_0().child(
            h_flex()
                .gap(surface::css(1.))
                .child(
                    Button::new("surround-primary")
                        .label(t("PRIMARY"))
                        .selected(enabled == Some(false))
                        .disabled(!self.surround.connected)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.select_surround_mode(false, window, cx)
                        })),
                )
                .child(
                    Button::new("surround-mode")
                        .label(t("SURROUND"))
                        .selected(enabled == Some(true) || self.surround.choosing_front)
                        .disabled(!self.surround.connected || self.surround.endpoints.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.select_surround_mode(true, window, cx)
                        })),
                ),
        );
        let select = self.surround.select.as_ref().expect("Surround initialized");
        if self.surround.choosing_front {
            column = column.child(
                v_flex()
                    .gap(surface::css(20.))
                    .child(t("FRONT_SPEAKERS"))
                    .child(t("FRONT_SPEAKER_POPUP_DESC"))
                    .child(Select::new(select).placeholder(t("FRONT_SPEAKER_PLACEHOLDER")))
                    .child(
                        h_flex()
                            .gap(surface::css(12.))
                            .child(
                                Button::new("surround-front-cancel")
                                    .label(t("CANCEL"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.surround.choosing_front = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("surround-front-confirm")
                                    .label(t("CONFIRM"))
                                    .disabled(self.surround.selected.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.submit_surround_front(true, cx)
                                    })),
                            ),
                    ),
            );
        } else {
            column = column.child(t("SPEAKER_MODE_DECSCRIPTION"));
            if enabled == Some(true) {
                column = column
                    .child(
                        h_flex()
                            .gap(surface::css(8.))
                            .child(t("FRONT_SPEAKERS"))
                            .child(
                                self.surround_help("surround-front-help", "FRONT_SPEAKER_TOOLTIP"),
                            ),
                    )
                    .child(
                        Select::new(select)
                            .placeholder(t("FRONT_SPEAKER_PLACEHOLDER"))
                            .disabled(!self.surround.connected),
                    );
            }
            column = column.child(t(if enabled == Some(true) {
                "REAR_SPEAKERS"
            } else {
                "PRIMARY_SPEAKERS"
            }));
            column = column.child(self.surround_product_name());
            if self.surround.connected && self.surround.endpoints.is_empty() {
                column = column.child(t("SPEAKER_MODE_WARN"));
            }
            if enabled == Some(true) {
                let replicate = self.surround.controls.replicate().value().copied();
                column =
                    column.child(
                        h_flex()
                            .gap(surface::css(8.))
                            .child(
                                Checkbox::new("surround-replicate")
                                    .label(t("AUDIO_MONITORING_CHECK"))
                                    .checked(replicate == Some(true))
                                    .disabled(!self.surround.connected || replicate.is_none())
                                    .on_change(cx.listener(|this, next, _, cx| {
                                        this.dispatch_surround(
                                            SurroundOperation::Change(Change::Replicate(*next)),
                                            cx,
                                        )
                                    })),
                            )
                            .child(self.surround_help(
                                "surround-replicate-help",
                                "AUDIO_MIRRORING_TOOLTIP",
                            )),
                    );
            }
        }
        surface::panel_with_help(
            t("SPEAKER_MODE"),
            h_flex()
                .child(self.surround_help("surround-mode-help", "COMBO_MODE_TOOLTIP"))
                .child(self.surround_tutorial_button("surround-mode-tutorial", 2, cx)),
            cx,
        )
        .w_full()
        .max_w(surface::css(1220.))
        .min_w_0()
        .child(
            h_flex()
                .gap(surface::css(30.))
                .items_start()
                .child(column)
                .child(
                    v_flex()
                        .items_center()
                        .gap(surface::css(10.))
                        .child(self.surround_diagram(cx))
                        .child(
                            Button::new("surround-demo")
                                .label(t("PLAY_DEMO"))
                                .disabled(
                                    self.demo
                                        .as_ref()
                                        .is_none_or(|demo| demo.read(cx).is_forced_open()),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(demo) = &this.demo {
                                        demo.update(cx, |demo, cx| {
                                            demo.open_from_surround(window, cx)
                                        });
                                    }
                                    cx.notify();
                                })),
                        ),
                ),
        )
        .into_any_element()
    }
    fn surround_volume_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let master = self.surround.master.as_ref();
        let enabled = self.surround_enabled();
        let unmuted = master.is_some_and(|m| !m.muted);
        surface::panel_with_title_switch(
            t("OPEN_WINDOW_VOLUME_MIXER"),
            surface::SynapseSwitch::new("surround-master-switch")
                .checked(unmuted)
                .disabled(!enabled || master.is_none())
                .accessibility_label(t("OPEN_WINDOW_VOLUME_MIXER"))
                .on_change(cx.listener(|this, next: &bool, _, cx| {
                    if let Some(master) = &this.surround.master {
                        this.dispatch_surround(
                            SurroundOperation::Master {
                                mute: !*next,
                                volume: master.volume,
                            },
                            cx,
                        );
                    }
                })),
            h_flex()
                .child(self.surround_help("surround-volume-help", "VOlUME_MIXER_TOOLTIP"))
                .child(self.surround_tutorial_button("surround-volume-tutorial", 4, cx)),
            cx,
        )
        .w_full()
        .min_w_0()
        .child(self.surround_horizontal(
            Control::Master,
            format!("{} - {}", t("MASTER"), t("ALL_SPEAKERS")),
            enabled && unmuted,
            cx,
        ))
        .child(self.surround_horizontal(
            Control::FrontVolume,
            self.surround.controls.front().map_or_else(
                || t("FRONT_SPEAKERS"),
                |front| format!("{} - {}", t("FRONT_SPEAKERS"), front.name()),
            ),
            enabled && unmuted,
            cx,
        ))
        .child(self.surround_horizontal(
            Control::RearVolume,
            t("REAR_SPEAKERS"),
            enabled && unmuted,
            cx,
        ))
        .into_any_element()
    }
    fn surround_delay_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.surround_enabled() && self.surround.controls.front().is_some();
        surface::panel_with_help(
            t("TIME_ALIGNMENT"),
            h_flex()
                .child(self.surround_help("surround-delay-help", "TIME_ALIGNMENT_TOOLTIP"))
                .child(self.surround_tutorial_button("surround-delay-tutorial", 5, cx)),
            cx,
        )
        .w_full()
        .min_w_0()
        .child(
            h_flex()
                .items_start()
                .gap(surface::css(20.))
                .child(
                    v_flex()
                        .flex_1()
                        .gap(surface::css(20.))
                        .child(t("TIME_ALIGNMENT_DESC_1"))
                        .child(t("TIME_ALIGNMENT_DESC_2"))
                        .child(
                            Button::new("surround-listen")
                                .label(t(if self.surround.signal_requested {
                                    "STOP"
                                } else {
                                    "LISTEN"
                                }))
                                .disabled(!enabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.surround.signal_requested =
                                        !this.surround.signal_requested;
                                    this.dispatch_surround(
                                        SurroundOperation::Change(Change::TestSignal(
                                            this.surround.signal_requested,
                                        )),
                                        cx,
                                    );
                                })),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .gap(surface::css(10.))
                        .child(t("FRONT_SPEAKERS"))
                        .child(self.surround_vertical(Control::Delay, "", enabled, cx))
                        .child(t("REAR_SPEAKERS"))
                        .child(
                            Button::new("surround-delay-reset")
                                .label(t("RESET"))
                                .disabled(!enabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.dispatch_surround(
                                        SurroundOperation::Change(Change::Delay(Delay::ZERO)),
                                        cx,
                                    )
                                })),
                        ),
                ),
        )
        .into_any_element()
    }
    fn surround_eq_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.surround_enabled() && self.surround.controls.front().is_some();
        let mut columns = h_flex().items_start().gap(surface::css(20.));
        for (channel, title, controls) in [
            (
                Channel::Front,
                "FRONT_SPEAKERS",
                [Control::FrontLow, Control::FrontMid, Control::FrontHigh],
            ),
            (
                Channel::Rear,
                "REAR_SPEAKERS",
                [Control::RearLow, Control::RearMid, Control::RearHigh],
            ),
        ] {
            let mut sliders = h_flex().justify_between().gap(surface::css(12.));
            for (control, label) in
                controls
                    .into_iter()
                    .zip(["LOW_FREQUENCY", "MID_FREQUENCY", "HIGH_FREQUENCY"])
            {
                sliders = sliders.child(self.surround_vertical(control, label, enabled, cx));
            }
            columns = columns.child(
                v_flex()
                    .flex_1()
                    .gap(surface::css(20.))
                    .child(t(title))
                    .child(sliders)
                    .child(
                        Button::new(if channel == Channel::Front {
                            "surround-front-eq-reset"
                        } else {
                            "surround-rear-eq-reset"
                        })
                        .label(t("RESET"))
                        .disabled(!enabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dispatch_surround(
                                SurroundOperation::Change(Change::Equalizer {
                                    channel,
                                    equalizer: Equalizer::FLAT,
                                }),
                                cx,
                            )
                        })),
                    ),
            );
        }
        surface::panel_with_help(
            t("EQUALIZER_ADJUSTMENTS"),
            h_flex()
                .child(surface::help_control(
                    "surround-eq-help",
                    format!(
                        "{}\n{}",
                        t("EQUALIZER_ADJUSTMENTS_TOOLTIP_1"),
                        t("EQUALIZER_ADJUSTMENTS_TOOLTIP_2")
                    ),
                ))
                .child(self.surround_tutorial_button("surround-eq-tutorial", 3, cx)),
            cx,
        )
        .w_full()
        .min_w_0()
        .child(
            div()
                .mb(surface::css(20.))
                .child(t("EQUALIZER_ADJUSTMENTS_DESC")),
        )
        .child(columns)
        .into_any_element()
    }
    pub(in crate::features::audio_products) fn surround_page(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w_full()
            .max_w(surface::css(1220.))
            .gap(surface::css(20.))
            .child(self.surround_speaker_panel(cx))
            .children(
                self.surround
                    .failure
                    .as_ref()
                    .map(|message| div().text_color(cx.theme().danger).child(message.clone())),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap(surface::css(20.))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(surface::css(20.))
                            .child(self.surround_volume_panel(cx))
                            .child(self.surround_delay_panel(cx)),
                    )
                    .child(div().flex_1().min_w_0().child(self.surround_eq_panel(cx))),
            )
            .into_any_element()
    }
}
