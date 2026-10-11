//! Current mounted ap/Oc popup lifecycle and tp/Rc visual frame scheduling.
use super::*;
use calibration::state::CalibrationAction;
use gpui_kit::base::Dialog;
use razer_widgets::theme::GamepadDialogColors as Colors;
use std::time::Instant;

pub(super) struct TriggerDialog {
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    tab_focus: razer_model::host_window_focus::PopupTabFocus,
}

impl GamepadProductWorkspace {
    pub(super) fn set_calibration_selection_hover(
        &mut self,
        part: Option<u8>,
        cx: &mut Context<Self>,
    ) {
        self.calibration_selection_hover = part;
        cx.notify();
    }
    pub(super) fn open_trigger_calibration_popup(
        &mut self,
        part: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.has_popup_calibration() || self.trigger_calibration_popup.is_some() {
            return;
        }
        // Two popup views observe the same controllerCalibrationReducer.
        // Independent visual state must not reset its shared validity.
        self.trigger_calibration_state
            .set_reducer_valid(self.calibration_state.reducer_valid());
        let Some(intent) = self.trigger_calibration_state.open(part, Instant::now()) else {
            return;
        };
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.trigger_calibration_popup = Some(TriggerDialog {
            focus,
            return_focus,
            tab_focus: Default::default(),
        });
        cx.emit(intent);
        cx.notify();
    }
    pub(super) fn close_trigger_calibration_popup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.trigger_calibration_popup.take() else {
            return;
        };
        // useEffect cleanup stops selectedTrigger, even if the shared observed
        // reducer's part switched and Recalibrate used that observed part.
        let part = self.trigger_calibration_state.selected_part;
        if let Some(intent) =
            self.trigger_calibration_state
                .request(CalibrationAction::Stop, part, Instant::now())
        {
            cx.emit(intent);
        }
        self.trigger_calibration_state.stop_visuals();
        if let Some(focus) = dialog.return_focus {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    pub(super) fn restart_trigger_calibration(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.trigger_calibration_popup.is_none() {
            return;
        }
        let part = self.trigger_calibration_state.part();
        if let Some(intent) =
            self.trigger_calibration_state
                .request(CalibrationAction::Start, part, Instant::now())
        {
            cx.emit(intent);
            cx.notify();
        }
    }
    pub fn trigger_calibration_generation(&self) -> Option<u64> {
        self.has_popup_calibration()
            .then(|| self.trigger_calibration_state.generation())
    }
    pub fn trigger_calibration_focus_pending(&self) -> bool {
        self.trigger_calibration_popup
            .as_ref()
            .is_some_and(|dialog| dialog.tab_focus.pending())
    }
    pub fn trigger_calibration_intent(&self) -> Option<&TriggerCalibrationIntent> {
        self.trigger_calibration_state.intent()
    }
    pub fn trigger_calibration_submission_error(&self) -> Option<&str> {
        self.trigger_calibration_state.submission_error()
    }
    pub fn finish_trigger_calibration_submission(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let accepted = self
            .trigger_calibration_state
            .finish_submission(generation, result);
        if accepted {
            cx.notify();
        }
        accepted
    }
    pub fn observe_trigger_calibration(
        &mut self,
        observation: TriggerCalibrationObservation,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.trigger_calibration_popup.is_none()
            || !self
                .trigger_calibration_state
                .observe(observation, Instant::now())
        {
            return false;
        }
        self.calibration_state
            .set_reducer_valid(self.trigger_calibration_state.valid);
        cx.notify();
        true
    }
    /// Actual synapse/window-name focus observation from the source helper.
    /// Initially inactive is permitted; active then inactive closes the popup.
    pub fn observe_trigger_calibration_focus(
        &mut self,
        generation: u64,
        active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if generation != self.trigger_calibration_state.generation() {
            return false;
        }
        let Some(dialog) = self.trigger_calibration_popup.as_mut() else {
            return false;
        };
        if dialog.tab_focus.observe(active) {
            self.close_trigger_calibration_popup(window, cx);
        }
        true
    }
    pub(super) fn schedule_trigger_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.trigger_calibration_popup.is_none()
            || !self.trigger_calibration_state.needs_frame()
            || self.trigger_frame_pending
        {
            return;
        }
        self.trigger_frame_pending = true;
        let owner = cx.weak_entity();
        window.on_next_frame(move |_, cx| {
            let _ = owner.update(cx, |this, cx| {
                this.trigger_frame_pending = false;
                if this.trigger_calibration_popup.is_some() {
                    this.trigger_calibration_state.tick(Instant::now());
                    cx.notify();
                }
            });
        });
    }
    pub(super) fn render_trigger_calibration_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = &self.trigger_calibration_popup else {
            return div().into_any_element();
        };
        let state = &self.trigger_calibration_state;
        let pid = self.spec.product_id;
        let content = trigger_calibration_renderer::render(
            pid,
            self.edition_id,
            trigger_calibration_renderer::TriggerCalibrationView {
                part: state.part(),
                step: state.step,
                valid: state.valid,
                raw_trigger_percent: state.raw_trigger_percent,
                timer_progress: state.timer_progress,
                marker_progress: state.marker_progress,
                prompt_progress: state.prompt_progress,
            },
            cx,
        );
        let panel = v_flex()
            .occlude()
            .w(surface::css(850.))
            .h(window.viewport_size().height)
            .bg(Colors::detail())
            .text_color(Colors::text())
            .text_size(surface::css(14.))
            .child(
                h_flex()
                    .relative()
                    .h(surface::css(36.))
                    .px(surface::css(36.))
                    .justify_center()
                    .items_center()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .text_color(razer_widgets::theme::FeedbackColors::secondary())
                    .border_b_1()
                    .border_color(razer_widgets::theme::FeedbackColors::border())
                    // Outer ap/Oc title comes from selectedTrigger, not observed partId.
                    .child(t(if state.selected_part == 4 {
                        "RIGHT_TRIGGER_CALIBRATION"
                    } else {
                        "LEFT_TRIGGER_CALIBRATION"
                    }))
                    .child(
                        BaseButton::new("gamepad-trigger-calibration-close")
                            .absolute()
                            .right_0()
                            .top_0()
                            .size(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .accessibility_label(t("CLOSE"))
                            .child(
                                img(SharedString::from(format!(
                                    "synapse/gamepad-{pid}-calibration-close.svg"
                                )))
                                .size(surface::css(20.)),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_trigger_calibration_popup(window, cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .justify_center()
                    .pt(surface::css(20.))
                    .px(surface::css(25.))
                    .pb(surface::css(30.))
                    .text_center()
                    .child(content),
            );
        Dialog::new(cx)
            .focus_handle(dialog.focus.clone())
            .close_on_backdrop_press(false)
            .close_on_escape(false)
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .backdrop(div())
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}
