//! Current 2676 immediate radio intent and independent device/write presentation.
use super::*;
use gpui_kit::base::{
    Radio,
    motion::{self, Easing, Transition},
};
use razer_device::{
    controller_initialization::{Read, SocdMode, SocdObservation},
    controller_socd::{SocdOperation, SocdResult},
};
use razer_widgets::theme::GamepadTriggerColors as Colors;

#[derive(Clone)]
pub struct ControllerSocdEdited(pub SocdMode);

#[derive(Clone, Default)]
pub(crate) struct SocdPresentation {
    pub draft: Option<SocdMode>,
    pub persisted: Option<SocdMode>,
    pub result: Option<(SocdOperation, SocdResult)>,
    pub error: Option<String>,
    pub saving: bool,
    pub writing: bool,
    pub enabled: bool,
}
impl SocdPresentation {
    fn selected(&self, observation: &Read<SocdObservation>) -> Option<SocdMode> {
        self.draft
            .or_else(|| observation.value().and_then(|value| value.mode()))
    }
}
#[derive(IntoElement)]
struct SocdRadio {
    mode: SocdMode,
    label: String,
    selected: bool,
    enabled: bool,
    owner: WeakEntity<GamepadProductWorkspace>,
}
impl RenderOnce for SocdRadio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = SharedString::from(format!("controller-socd-{:?}", self.mode));
        let progress = motion::transition(
            id.clone(),
            if self.selected { 1_f32 } else { 0. },
            Transition::new(std::time::Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        Radio::new(id)
            .checked(self.selected)
            .disabled(!self.enabled)
            .accessibility_label(self.label.clone())
            .flex()
            .items_center()
            .mx(surface::css(20.))
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .text_color(Colors::text())
            .child(
                div()
                    .size(surface::css(20.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(Colors::radio_border())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(surface::css(10. * progress))
                            .rounded_full()
                            .bg(Colors::accent())
                            .opacity(progress),
                    ),
            )
            .child(div().ml(surface::css(10.)).child(self.label))
            .on_change(move |_, _, _, cx| {
                let _ = self.owner.update(cx, |this, cx| {
                    if !this.socd.enabled {
                        return;
                    }
                    if let Err(error) = project_controller_socd(&mut this.draft, self.mode) {
                        this.socd.error = Some(format!("{error:#}"));
                    } else {
                        this.socd.draft = Some(self.mode);
                        cx.emit(ControllerSocdEdited(self.mode));
                    }
                    cx.notify();
                });
            })
    }
}
impl EventEmitter<ControllerSocdEdited> for GamepadProductWorkspace {}
impl GamepadProductWorkspace {
    pub(crate) fn present_controller_socd(
        &mut self,
        presentation: SocdPresentation,
        cx: &mut Context<Self>,
    ) {
        self.socd = presentation;
        cx.notify();
    }
    pub fn controller_socd_write_result(&self) -> Option<&(SocdOperation, SocdResult)> {
        self.socd.result.as_ref()
    }
    pub fn controller_socd_persisted(&self) -> Option<SocdMode> {
        self.socd.persisted
    }
    pub(super) fn socd_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut panel = surface::panel_with_help(
            t("SOCD_SETTINGS"),
            surface::help_control("controller-socd-help", t("DPAD_SOCD_TOOLTIP")),
            cx,
        )
        .child(surface::note(t("DPAD_SOCD_SETTINGS_DESC"), cx));
        for (mode, label, description) in [
            (SocdMode::Neutral, "NEUTRAL_SOCD", "NEUTRAL_SOCD_DESC"),
            (SocdMode::First, "FIRST_INPUT_SOCD", "FIRST_INPUT_SOCD_DESC"),
            (SocdMode::Last, "LAST_INPUT_SOCD", "LAST_INPUT_SOCD_DESC"),
        ] {
            panel = panel.child(
                v_flex()
                    .mt(surface::css(10.))
                    .ml(surface::css(-20.))
                    .child(SocdRadio {
                        mode,
                        label: t(label),
                        selected: self.socd.selected(&self.controller_socd_observation)
                            == Some(mode),
                        enabled: self.socd.enabled,
                        owner: cx.entity().downgrade(),
                    })
                    .child(
                        div()
                            .ml(surface::css(50.))
                            .text_size(surface::css(12.))
                            .opacity(0.7)
                            .child(t(description)),
                    ),
            );
        }
        let failed = self.socd.error.is_some()
            || (self.socd.draft.is_none()
                && (matches!(&self.controller_socd_observation, Read::Failed { .. })
                    || self
                        .controller_socd_observation
                        .value()
                        .is_some_and(|value| value.mode().is_none())));
        if failed {
            panel = panel.child(surface::note(t("ERROR"), cx));
        } else if self.socd.saving || self.socd.writing {
            panel = panel.child(surface::note(t("LOADING"), cx));
        } else if matches!(self.controller_socd_observation, Read::Unobserved)
            && self.socd.draft.is_none()
        {
            panel = panel.child(surface::note(format!("{}: —", t("SOCD_SETTINGS")), cx));
        }
        panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;
    #[test]
    fn controller_socd_ui_never_selects_cached_defaults_for_absent_failed_or_unknown_reads() {
        let mut state = SocdPresentation {
            persisted: Some(SocdMode::Neutral),
            ..Default::default()
        };
        for observed in [
            Read::Unobserved,
            Read::Failed {
                error: "real GET failure".into(),
            },
            Read::Observed {
                value: SocdObservation { raw_option: 255 },
            },
        ] {
            assert_eq!(state.selected(&observed), None);
        }
        let observed = Read::Observed {
            value: SocdObservation { raw_option: 2 },
        };
        assert_eq!(state.selected(&observed), Some(SocdMode::First));
        state.draft = Some(SocdMode::Last);
        assert_eq!(state.selected(&observed), Some(SocdMode::Last));
        assert_eq!(state.persisted, Some(SocdMode::Neutral));
        assert_eq!(observed.value().unwrap().raw_option, 2);
    }
}
