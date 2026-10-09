//! Presentation and commit policy for the original DU Audio Meter stepper.
use super::{
    settings::{Effect, normalize_color_boost},
    workspace::DeviceWorkspace,
};
use gpui_kit::base::{NumberInput, StepAction, step_value};
use gpui_kit::component::{
    input::{Input, InputState},
    *,
};
use gpui_kit::*;
use razer_widgets::surface;
use razer_widgets::theme;

// Stepper 44230 permits partial/negative numeric drafts; maxLength excludes the
// optional minus sign. Range and quarter-step normalization happen on commit.
pub(super) fn valid_color_boost_draft(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    if unsigned.len() > 4 {
        return false;
    }
    let mut parts = unsigned.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    (unsigned.is_empty() || !integer.is_empty())
        && integer.bytes().all(|c| c.is_ascii_digit())
        && fraction.is_none_or(|s| s.len() <= 3 && s.bytes().all(|c| c.is_ascii_digit()))
        && parts.next().is_none()
}

impl DeviceWorkspace {
    pub(super) fn commit_color_boost(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings().lighting.effect != Effect::AudioMeter {
            return;
        }
        let value = self
            .controls
            .color_boost
            .read(cx)
            .value()
            .parse()
            .unwrap_or(0.);
        self.set_color_boost(value, window, cx);
    }

    fn set_color_boost(&mut self, value: f32, window: &mut Window, cx: &mut Context<Self>) {
        let value = normalize_color_boost(value);
        if self.settings().lighting.params().color_boost != value {
            self.edit(window, cx, |s| s.lighting.params_mut().color_boost = value);
        }
        // Also canonicalize a draft whose normalized domain value did not change.
        if self.controls.color_boost.read(cx).value().as_str() != value.to_string() {
            self.controls.color_boost.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx);
            });
        }
    }

    fn step_color_boost(
        &mut self,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = self.controls.color_boost.read(cx).value();
        let value = step_value(&draft, action, 0.25, Some(0.25), Some(4.))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.25);
        self.set_color_boost(value, window, cx);
        self.controls
            .color_boost
            .update(cx, |input, cx| input.focus(window, cx));
    }

    pub(super) fn color_boost_editor(&self, cx: &Context<Self>) -> AnyElement {
        v_flex()
            .mt(surface::css(20.))
            .items_start()
            .child("色彩增强")
            .child(ColorBoostInput {
                input: self.controls.color_boost.clone(),
                owner: cx.entity().downgrade(),
            })
            .into_any_element()
    }
}

#[derive(IntoElement)]
struct ColorBoostInput {
    input: Entity<InputState>,
    owner: WeakEntity<DeviceWorkspace>,
}

impl RenderOnce for ColorBoostInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self.input.focus_handle(cx);
        let focused = focus.is_focused(window);
        let value = self.input.read(cx).value().parse::<f32>().unwrap_or(0.);
        let hover = theme::DropdownColors::new().hover();
        let active = cx.theme().button_primary_foreground.opacity(0.1);
        let arrow = move |button: gpui_kit::base::Button, asset, at_limit| {
            let opacity = if at_limit { 0.3 } else { 1. };
            button
                .w(surface::css(18.))
                .h(surface::css(12.))
                .p_0()
                .flex()
                .items_center()
                .justify_center()
                .opacity(if focused { opacity } else { 0. })
                .group_hover("color-boost", move |s| s.opacity(opacity))
                .hover(move |s| s.bg(hover))
                .active(move |s| s.bg(active))
                .child(img(asset).size(surface::css(8.)))
        };
        let owner = self.owner;
        div()
            .id("color-boost")
            .test_support()
            .group("color-boost")
            .track_focus(&focus)
            .mt(surface::css(10.))
            .mb(surface::css(10.))
            .w(surface::css(60.))
            .h(surface::css(27.))
            .border_1()
            .border_color(if focused {
                cx.theme().primary
            } else {
                theme::stepper_border()
            })
            .hover(|s| s.border_color(cx.theme().primary))
            .bg(cx.theme().group_box)
            .on_key_down(|event, window, cx| {
                // Original Stepper commits both Enter and Escape by blurring.
                if matches!(event.keystroke.key.as_str(), "escape" | "enter") {
                    window.blur(cx);
                }
            })
            .child(
                NumberInput::new(&self.input)
                    .size_full()
                    .controls_right()
                    .input(
                        Input::new(&self.input)
                            .id("color-boost-input")
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .h(surface::css(25.))
                            .w_full()
                            .p_0()
                            .pl(surface::css(5.))
                            .text_size(surface::css(14.)),
                    )
                    .increment_button(move |b| arrow(b, "synapse/stepper-up.svg", value >= 4.))
                    .decrement_button(move |b| arrow(b, "synapse/stepper-down.svg", value <= 0.25))
                    .on_step(move |action, window, cx| {
                        let _ = owner
                            .update(cx, |owner, cx| owner.step_color_boost(action, window, cx));
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceWorkspace, Effect, normalize_color_boost, valid_color_boost_draft};
    use gpui_kit::component::Root;
    use gpui_kit::{AppContext, TestAppContext, px, size};

    #[test]
    fn source_stepper_accepts_partial_drafts_without_premature_clamping() {
        for draft in ["", "-", "0", "0.", "0.3", "0.25", "3.99", "-3.99"] {
            assert!(valid_color_boost_draft(draft), "{draft}");
        }
        for draft in [".", "1.234", "1..2", "NaN", "1e2", "+1", "1 2", "１２"] {
            assert!(!valid_color_boost_draft(draft), "{draft}");
        }
    }

    #[test]
    fn source_stepper_clamps_and_rounds_up_to_quarters() {
        for (input, expected) in [
            (0., 0.25),
            (-1., 0.25),
            (0.26, 0.5),
            (0.5, 0.5),
            (1.01, 1.25),
            (3.99, 4.),
            (9., 4.),
            (f32::NAN, 0.25),
        ] {
            assert_eq!(normalize_color_boost(input), expected);
        }
    }

    #[gpui_kit::test]
    fn audio_meter_commits_typed_drafts_and_steps_without_replacing_input(cx: &mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        cx.update(gpui_kit::init);
        let mut workspace = None;
        let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
            let view = cx.new(|cx| {
                DeviceWorkspace::new(razer_model::demo::demo_keyboard(), true, window, cx)
            });
            view.update(cx, |view, cx| {
                view.set_page(crate::nav::Tab::Lighting, window, cx);
                view.edit(window, cx, |s| s.lighting.effect = Effect::AudioMeter);
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        let input_id = cx.update(|cx| view.read(cx).controls.color_boost.entity_id());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("color-boost").bounds().size,
                size(px(60.), px(27.))
            );
            window.click("color-boost-input", cx);
            window.press("secondary-a", cx);
            window.input("0.26", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).settings().lighting.params().color_boost, 1.);
            assert_eq!(
                view.read(cx).controls.color_boost.read(cx).value().as_str(),
                "0.26"
            );
        });
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(view.read(cx).settings().lighting.params().color_boost, 0.5));
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("color-boost-input", cx);
            window.press("up", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(view.read(cx).settings().lighting.params().color_boost, 0.75));
        cx.update_window(handle.into(), |_, window, cx| {
            window.press("secondary-a", cx);
            window.input("3.99", cx);
            window.press("tab", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let owner = view.read(cx);
            assert_eq!(owner.settings().lighting.params().color_boost, 4.);
            assert_eq!(owner.controls.color_boost.read(cx).value().as_str(), "4");
            assert_eq!(owner.controls.color_boost.entity_id(), input_id);
        });
    }
}
