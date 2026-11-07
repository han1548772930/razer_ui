//! Gesture drafts and the source's four tap intervals share the earbud job queue.
use super::*;
use razer_device::{
    earbuds_auto_pause::{Operation, Report},
    earbuds_gesture::{self as protocol, TapObservation, TapSpeed},
    earbuds_protocol40::{ToggleObservation, WriteOutcome},
};
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as Colors};

#[path = "earbuds_gesture_modal.rs"]
mod modal;

#[derive(Default)]
pub(super) struct State {
    pub(super) device: ToggleObservation,
    pub(super) persisted: Option<bool>,
    pub(super) draft: bool,
    pub(super) error: Option<String>,
    pub(super) profile_error: Option<String>,
    pub(super) last_write: Option<WriteOutcome>,
    pub(super) tap_device: TapObservation,
    pub(super) tap_persisted: Option<TapSpeed>,
    pub(super) tap_draft: TapSpeed,
    pub(super) tap_error: Option<String>,
    pub(super) tap_profile_error: Option<String>,
    pub(super) tap_last_write: Option<WriteOutcome>,
    pub(super) popup: Option<modal::Popup>,
}

impl State {
    pub(super) fn clear_observations(&mut self) {
        self.device = ToggleObservation::Unobserved;
        self.tap_device = TapObservation::Unobserved;
    }

    pub(super) fn load_profile(
        &mut self,
        device: &razer_model::model::Device,
        initial: bool,
        continuing: bool,
    ) {
        match if initial {
            Ok(None)
        } else {
            razer_storage::earbuds_profiles::read_gesture(device, &device.active_profile)
        } {
            Ok(value) => {
                self.persisted = value;
                self.profile_error = None;
            }
            Err(error) => {
                self.persisted = None;
                self.profile_error = Some(format!("{error:#}"));
            }
        }
        match if initial {
            Ok(None)
        } else {
            razer_storage::earbuds_profiles::read_tap_speed(device, &device.active_profile)
        } {
            Ok(value) => {
                self.tap_persisted = value;
                self.tap_profile_error = None;
            }
            Err(error) => {
                self.tap_persisted = None;
                self.tap_profile_error = Some(format!("{error:#}"));
            }
        }
        if continuing {
            if let Some(value) = self.persisted {
                self.draft = value;
            }
        } else {
            self.draft = false;
            self.tap_draft = TapSpeed::Normal;
        }
        self.error = None;
        self.tap_error = None;
    }
}

impl AudioProductWorkspace {
    pub(super) fn observe_gesture_report(&mut self, report: &Report, operation: Operation) {
        let pending_gesture = self
            .earbuds
            .edits
            .iter()
            .map(|edit| &edit.operation)
            .any(|op| matches!(op, Operation::Gesture { .. }));
        let pending_tap = self
            .earbuds
            .edits
            .iter()
            .map(|edit| &edit.operation)
            .any(|op| matches!(op, Operation::TapSpeed { .. }));
        let state = &mut self.earbuds.gesture;
        if let Some(observed) = &report.gesture {
            if let ToggleObservation::Observed(enabled) = observed {
                if !pending_gesture {
                    state.draft = match operation {
                        Operation::RestoreGesture { enabled } => enabled.unwrap_or(true),
                        _ => *enabled,
                    };
                }
            }
            state.error = observed.diagnostic();
            state.device = observed.clone();
        }
        if let Some(observed) = &report.tap_speed {
            if let TapObservation::Observed(ticks) = observed {
                if !pending_tap {
                    if let Some(speed) = match operation {
                        Operation::RestoreTapSpeed { speed } => {
                            Some(speed.unwrap_or(TapSpeed::Normal))
                        }
                        _ => TapSpeed::from_ticks(*ticks),
                    } {
                        state.tap_draft = speed;
                    }
                }
            }
            state.tap_error = match observed {
                TapObservation::ReadFailed(error) => Some(error.clone()),
                _ => None,
            };
            state.tap_device = observed.clone();
        }
        if matches!(
            operation,
            Operation::Gesture { .. } | Operation::RestoreGesture { .. }
        ) {
            if let WriteOutcome::Failed(error) = &report.write {
                state.error = Some(error.clone());
            }
            state.last_write = Some(report.write.clone());
        }
        if matches!(
            operation,
            Operation::TapSpeed { .. } | Operation::RestoreTapSpeed { .. }
        ) {
            if let WriteOutcome::Failed(error) = &report.write {
                state.tap_error = Some(error.clone());
            }
            state.tap_last_write = Some(report.write.clone());
        }
    }

    pub(super) fn fail_gesture_request(&mut self, operation: Operation, error: String) {
        let state = &mut self.earbuds.gesture;
        match operation {
            Operation::Gesture { .. } | Operation::RestoreGesture { .. } => {
                if matches!(operation, Operation::RestoreGesture { .. }) {
                    state.device = ToggleObservation::ReadFailed(error.clone());
                }
                state.error = Some(error.clone());
                state.last_write = Some(WriteOutcome::Failed(error));
            }
            Operation::TapSpeed { .. } | Operation::RestoreTapSpeed { .. } => {
                if matches!(operation, Operation::RestoreTapSpeed { .. }) {
                    state.tap_device = TapObservation::ReadFailed(error.clone());
                }
                state.tap_error = Some(error.clone());
                state.tap_last_write = Some(WriteOutcome::Failed(error));
            }
            _ => {}
        }
    }

    fn gesture_connected(&self) -> bool {
        self.earbuds.active
            && self.earbuds.initialized
            && self
                .earbuds
                .controls
                .as_ref()
                .is_some_and(|control| !control.earbud_disconnected)
    }

    fn toggle_gesture(&mut self, cx: &mut Context<Self>) {
        if !self.gesture_connected() || self.earbuds.gesture.profile_error.is_some() {
            return;
        }
        self.earbuds.gesture.draft = !self.earbuds.gesture.draft;
        self.earbuds.queue_edit(Operation::Gesture {
            enabled: self.earbuds.gesture.draft,
        });
        self.start_earbuds_request(cx);
        cx.notify();
    }

    fn choose_tap_speed(&mut self, speed: TapSpeed, cx: &mut Context<Self>) {
        if !self.gesture_connected()
            || !self.earbuds.gesture.draft
            || self.earbuds.gesture.tap_profile_error.is_some()
            || self.earbuds.gesture.tap_draft == speed
            || !self.tap_speed_visible()
        {
            return;
        }
        self.earbuds.gesture.tap_draft = speed;
        self.earbuds.queue_edit(Operation::TapSpeed { speed });
        self.start_earbuds_request(cx);
        cx.notify();
    }

    fn tap_speed_visible(&self) -> bool {
        protocol::tap_supported(
            self.spec.product_id,
            self.earbuds
                .controls
                .as_ref()
                .and_then(|control| control.firmware_version.as_deref()),
        )
    }

    pub(super) fn earbuds_gesture_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.earbuds.gesture;
        let connected = self.gesture_connected();
        let enabled = connected && state.profile_error.is_none();
        let tap_enabled = connected && state.draft && state.tap_profile_error.is_none();
        surface::panel_with_title_switch(
            t("GESTURES"),
            surface::SynapseSwitch::new("earbuds-gestures")
                .checked(state.draft)
                .disabled(!enabled)
                .accessibility_label(t("GESTURES"))
                .on_change(cx.listener(|this, _, _, cx| this.toggle_gesture(cx))),
            div(),
            cx,
        )
        .opacity(if connected { 1. } else { 0.5 })
        .child(
            h_flex()
                .flex_wrap()
                .gap(surface::css(4.))
                .text_size(surface::css(14.))
                .child(t("GESTURES_DESCRIPTION"))
                .child(
                    Button::new("earbuds-view-gestures")
                        .disabled(!connected)
                        .accessibility_label(t("VIEW_GESTURES"))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(surface::css(4.))
                        .py(surface::css(2.))
                        .rounded(surface::css(2.))
                        .text_color(Colors::text())
                        .hover(|style| style.text_color(Colors::selected()))
                        .child(div().underline().child(t("VIEW_GESTURES")))
                        .child(
                            razer_assets::resource_svg()
                                .path("synapse/earbuds-gesture-arrow.svg")
                                .size(surface::css(16.)),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_earbuds_gestures(window, cx)
                        })),
                ),
        )
        .when(self.tap_speed_visible(), |panel| {
            panel
                .child(
                    div()
                        .mt(surface::css(22.))
                        .mb(surface::css(10.))
                        .child(t("TAP_SPEED").to_uppercase()),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(surface::css(10.))
                        .opacity(if state.draft { 1. } else { 0.3 })
                        .children(TapSpeed::ALL.into_iter().map(|speed| {
                            let selected = state.tap_draft == speed;
                            Button::new(("earbuds-tap-speed", speed.ticks() as usize))
                                .disabled(!tap_enabled)
                                .accessibility_label(t(&speed.key().to_uppercase()))
                                .cursor_pointer()
                                .min_w(surface::css(90.))
                                .h(surface::css(27.))
                                .px(surface::css(16.))
                                .py(surface::css(7.))
                                .rounded(surface::css(3.))
                                .border_1()
                                .border_color(if selected {
                                    Colors::selected()
                                } else {
                                    Colors::border()
                                })
                                .bg(if selected {
                                    Colors::tab_active()
                                } else {
                                    Colors::background()
                                })
                                .text_color(Colors::text())
                                .text_size(surface::css(12.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(tap_enabled, |button| {
                                    button.hover(|style| {
                                        style
                                            .bg(Colors::tab_active())
                                            .border_color(Colors::selected())
                                    })
                                })
                                .child(t(&speed.key().to_uppercase()).to_uppercase())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.choose_tap_speed(speed, cx)
                                }))
                        })),
                )
        })
        .into_any_element()
    }
}
