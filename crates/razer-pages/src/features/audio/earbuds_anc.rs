//! PID2657 ANC owns the local triple, device confirmation and status-only events.
use super::*;
use razer_device::{
    earbuds_anc::{Observation, State as HardwareState},
    earbuds_auto_pause::{Operation, Report},
    earbuds_controls::EarbudsTask,
    earbuds_protocol40::{AncChange, AncLevel, AncMode, AncWrite},
};
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as Colors};

#[derive(Default)]
pub(super) struct State {
    pub(super) device: Observation,
    pub(super) profile_error: Option<String>,
    pub(super) error: Option<String>,
    pub(super) storage_error: Option<String>,
}

impl AudioProductWorkspace {
    pub(super) fn observe_anc_report(&mut self, report: Report, restored: bool) {
        let Some(result) = report.anc else {
            self.earbuds.anc.error = Some("ANC reply omits its result".into());
            return;
        };
        let controls = self
            .earbuds
            .controls
            .as_mut()
            .expect("earbud control owner");
        controls.anc_runtime = Some(result.runtime);
        controls.last_anc_write = Some(result.write.clone());
        self.earbuds.anc.error = match result.write {
            AncWrite::Failed(error) => Some(error),
            _ => None,
        };
        if let Some(setting) = result.confirmation {
            self.earbuds.anc.device = Observation::Observed {
                state: HardwareState::from_raw(setting.firmware_state()),
                level: Some(setting.level),
            };
        }
        if restored
            && !self
                .earbuds
                .edits
                .iter()
                .any(|edit| matches!(edit.operation, Operation::Anc { .. }))
        {
            if let Operation::Anc { setting, .. } = report.operation {
                controls.draft.anc = Some(setting);
            }
        }
    }

    pub(super) fn observe_anc_notification(
        &mut self,
        state: HardwareState,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 2657 {
            return;
        }
        self.earbuds.anc.device = Observation::Observed { state, level: None };
        let Some(controls) = &mut self.earbuds.controls else {
            return;
        };
        let Some(setting) = controls.anc_notification(controls.generation(), &[5, 33, state.raw()])
        else {
            return;
        };
        self.earbuds.queue_notification(Operation::Anc { setting });
        self.start_earbuds_request(cx);
    }

    fn change_earbuds_anc(&mut self, change: AncChange, cx: &mut Context<Self>) {
        if !self.earbuds.active
            || !self.earbuds.initialized
            || self.earbuds.anc.profile_error.is_some()
        {
            return;
        }
        let Some(controls) = &mut self.earbuds.controls else {
            return;
        };
        let Some(EarbudsTask::SetAnc { setting, .. }) = controls.change_anc(change) else {
            return;
        };
        self.earbuds.queue_edit(Operation::Anc { setting });
        self.start_earbuds_request(cx);
        cx.notify();
    }

    fn toggle_earbuds_anc(&mut self, cx: &mut Context<Self>) {
        let Some(setting) = self
            .earbuds
            .controls
            .as_ref()
            .and_then(|control| control.draft.anc)
        else {
            return;
        };
        self.change_earbuds_anc(
            AncChange {
                enabled: Some(!setting.enabled),
                ..Default::default()
            },
            cx,
        );
    }
    fn toggle_earbuds_anc_mode(&mut self, cx: &mut Context<Self>) {
        let Some(setting) = self
            .earbuds
            .controls
            .as_ref()
            .and_then(|control| control.draft.anc)
        else {
            return;
        };
        let mode = if setting.mode == AncMode::Anc {
            AncMode::Ambient
        } else {
            AncMode::Anc
        };
        self.change_earbuds_anc(
            AncChange {
                mode: Some(mode),
                ..Default::default()
            },
            cx,
        );
    }
    fn choose_earbuds_anc_mode(&mut self, mode: AncMode, cx: &mut Context<Self>) {
        let Some(previous) = self
            .earbuds
            .controls
            .as_ref()
            .and_then(|control| control.draft.anc)
        else {
            return;
        };
        // Sm emits the child's selected tab, then its parent's two-tab toggle.
        self.change_earbuds_anc(
            AncChange {
                mode: Some(mode),
                ..Default::default()
            },
            cx,
        );
        let toggled = if previous.mode == AncMode::Anc {
            AncMode::Ambient
        } else {
            AncMode::Anc
        };
        self.change_earbuds_anc(
            AncChange {
                mode: Some(toggled),
                ..Default::default()
            },
            cx,
        );
    }
    fn toggle_earbuds_reduce(&mut self, keyboard: bool, cx: &mut Context<Self>) {
        let Some(setting) = self
            .earbuds
            .controls
            .as_ref()
            .and_then(|control| control.draft.anc)
        else {
            return;
        };
        if !keyboard && setting.mode != AncMode::Anc {
            return;
        }
        self.change_earbuds_anc(
            AncChange {
                level: Some(if setting.level == AncLevel::Reduced {
                    AncLevel::Full
                } else {
                    AncLevel::Reduced
                }),
                ..Default::default()
            },
            cx,
        );
    }

    pub(super) fn earbuds_anc_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let controls = self.earbuds.controls.as_ref();
        let setting = controls
            .and_then(|control| control.draft.anc)
            .unwrap_or(razer_device::earbuds_protocol40::ANC_UI_INITIAL);
        let disconnected = controls.is_some_and(|control| control.earbud_disconnected);
        let enabled = self.earbuds.active
            && self.earbuds.initialized
            && !disconnected
            && self.earbuds.anc.profile_error.is_none();
        let modes_enabled = enabled && setting.enabled;
        let reduce_pointer = modes_enabled && setting.mode == AncMode::Anc;
        let pointer = surface::pointer_state("earbuds-anc-selector", window, cx);
        let hovered = pointer.read(cx).sample().0 && modes_enabled;
        let border = surface::fade_color(
            "earbuds-anc-selector-border",
            if hovered {
                Colors::selected()
            } else {
                Colors::border()
            },
            200,
            window,
            cx,
        );
        let tabs = h_flex()
            .id("earbuds-anc-selector")
            .test_support()
            .self_start()
            .h(surface::css(36.))
            .p(surface::css(4.))
            .border_1()
            .border_color(border)
            .rounded(surface::css(18.))
            .bg(Colors::background())
            .opacity(if setting.enabled { 1. } else { 0.5 })
            .on_click(cx.listener(|this, _, _, cx| this.toggle_earbuds_anc_mode(cx)))
            .children(
                [(AncMode::Anc, "ANC"), (AncMode::Ambient, "AMBIENT1")]
                    .into_iter()
                    .map(|(mode, key)| {
                        let selected = setting.mode == mode;
                        let background = surface::fade_color(
                            ("earbuds-anc-mode-bg", mode.ui_status() as usize),
                            if selected {
                                Colors::selected()
                            } else {
                                Colors::background()
                            },
                            200,
                            window,
                            cx,
                        );
                        let foreground = surface::fade_color(
                            ("earbuds-anc-mode-text", mode.ui_status() as usize),
                            if selected {
                                Colors::background()
                            } else {
                                Colors::text()
                            },
                            200,
                            window,
                            cx,
                        );
                        Button::new(("earbuds-anc-mode", mode.ui_status() as usize))
                            .disabled(!modes_enabled)
                            .accessibility_label(t(key))
                            .h_full()
                            .min_w(surface::css(36.))
                            .px(surface::css(10.))
                            .py(surface::css(5.))
                            .mr(surface::css(if mode == AncMode::Anc { 5. } else { 0. }))
                            .rounded(surface::css(13.))
                            .bg(background)
                            .text_color(foreground)
                            .text_size(surface::css(14.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(t(key))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.choose_earbuds_anc_mode(mode, cx);
                            }))
                    }),
            );
        let tabs = surface::track_pointer(tabs, &pointer, window);
        let mut panel = surface::panel_with_title_switch(
            t("ACTIVE_NOISE_CANCELLATION"),
            surface::SynapseSwitch::new("earbuds-anc")
                .checked(setting.enabled)
                .disabled(!enabled)
                .accessibility_label(t("ACTIVE_NOISE_CANCELLATION"))
                .on_change(cx.listener(|this, _, _, cx| this.toggle_earbuds_anc(cx))),
            div()
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .child(surface::help_control(
                    "earbuds-anc-help",
                    t("ACTIVE_NOISE_CANCELLATION_TIP_EARBUDS"),
                )),
            cx,
        )
        .relative()
        .opacity(if disconnected { 0.5 } else { 1. })
        .child(tabs);
        if controls.is_some_and(|control| control.reduce_supported()) {
            panel = panel.child(
                h_flex()
                    .relative()
                    .mt(surface::css(20.))
                    .opacity(if reduce_pointer { 1. } else { 0.3 })
                    .child(
                        surface::check_item(
                            "earbuds-reduce-anc",
                            "",
                            setting.level == AncLevel::Reduced,
                            !enabled,
                            window,
                            cx,
                        )
                        .w(surface::css(20.))
                        .mb_0()
                        .opacity(1.)
                        .role(Role::CheckBox)
                        .aria_toggled(if setting.level == AncLevel::Reduced {
                            Toggled::True
                        } else {
                            Toggled::False
                        })
                        .accessibility_label(t("REDUCE_ANC_BLUETOOTH_CALL"))
                        .on_click(cx.listener(|this, event, _, cx| {
                            this.toggle_earbuds_reduce(matches!(event, ClickEvent::Keyboard(_)), cx)
                        })),
                    )
                    .child(
                        Button::new("earbuds-reduce-anc-label")
                            .disabled(!reduce_pointer)
                            .relative()
                            .top(surface::css(1.))
                            .ml(surface::css(10.))
                            .child(t("REDUCE_ANC_BLUETOOTH_CALL"))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.toggle_earbuds_reduce(false, cx)),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .left(surface::css(5.))
                            .top(surface::css(2.))
                            .child(surface::help_control(
                                "earbuds-reduce-anc-help",
                                t("REDUCE_ANC_BLUETOOTH_CALL_TOOLTIP"),
                            )),
                    )
                    .when(!reduce_pointer, |row| {
                        row.child(
                            div()
                                .absolute()
                                .inset_0()
                                .occlude()
                                .capture_any_mouse_down(|_, window, cx| {
                                    window.prevent_default();
                                    cx.stop_propagation();
                                })
                                .capture_any_mouse_up(|_, _, cx| cx.stop_propagation()),
                        )
                    }),
            );
        }
        panel.into_any_element()
    }
}
