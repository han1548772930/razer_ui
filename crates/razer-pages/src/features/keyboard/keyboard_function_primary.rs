//! PID717 Fp and $P: debounced primary-key choice and the system Fn layer.
use super::{KeyboardProductChanged, KeyboardProductWorkspace};
use gpui_kit::{
    AnyElement, App, Context, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled,
    Task, Window, base::Radio, component::v_flex, div, prelude::FluentBuilder as _,
};
use razer_device::keyboard_function_swap::FunctionKeyPrimary;
use razer_i18n::t;
use razer_widgets::{surface, theme::KeyboardFunctionPrimaryColors as Colors};
use serde_json::Value;
use std::time::Duration;

#[derive(Default)]
pub(super) struct State {
    generation: u64,
    timer: Option<Task<()>>,
    recorder_held: bool,
    closing: bool,
}

impl State {
    pub(super) fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.timer = None;
    }
    pub(super) fn stop(&mut self) {
        self.cancel();
        self.closing = true;
    }
    pub(super) fn reconnect(&mut self) {
        self.cancel();
        self.closing = false;
    }
}

fn counter_key(input: &str) -> bool {
    matches!(
        input,
        "KEY_F1"
            | "KEY_F2"
            | "KEY_F3"
            | "KEY_F4"
            | "KEY_F5"
            | "KEY_F6"
            | "KEY_F7"
            | "KEY_F8"
            | "KEY_F9"
            | "KEY_F10"
            | "KEY_F11"
            | "KEY_F12"
            | "KEY_WINDOWS"
    )
}

#[derive(IntoElement)]
struct Indicator {
    id: &'static str,
    checked: bool,
}
impl RenderOnce for Indicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::base::motion::{self, Easing, Transition};
        let amount = motion::transition(
            (self.id, "indicator"),
            if self.checked { 1. } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        div()
            .absolute()
            .left(surface::css(10. - 5. * amount))
            .top(surface::css(10. - 5. * amount))
            .size(surface::css(10. * amount))
            .rounded_full()
            .bg(Colors::selected())
            .opacity(amount)
    }
}

impl KeyboardProductWorkspace {
    pub fn set_function_primary_recorder_held(&mut self, held: bool, cx: &mut Context<Self>) {
        if self.function_primary.recorder_held != held {
            self.function_primary.recorder_held = held;
            if held {
                self.function_primary.cancel();
            }
            cx.notify();
        }
    }

    pub(super) fn function_primary(&self) -> Option<FunctionKeyPrimary> {
        match self.draft.get("functionKeyPrimary").and_then(Value::as_str) {
            Some("func") => Some(FunctionKeyPrimary::Function),
            Some("multi") => Some(FunctionKeyPrimary::Multimedia),
            _ => None,
        }
    }

    pub(super) fn function_system_layer(&self, input: &str) -> bool {
        self.spec.product_id == 717
            && counter_key(input)
            && self
                .function_primary()
                .is_some_and(|primary| self.hypershift == (primary == FunctionKeyPrimary::Function))
    }

    pub(super) fn function_hypershift_mapping_allowed(&self, input: &str) -> bool {
        self.spec.product_id == 717
            && counter_key(input)
            && self.function_primary() == Some(FunctionKeyPrimary::Multimedia)
    }

    fn queue_function_primary(&mut self, primary: FunctionKeyPrimary, cx: &mut Context<Self>) {
        if self.spec.product_id != 717
            || self.function_primary.recorder_held
            || self.function_primary.closing
        {
            return;
        }
        self.function_primary.cancel();
        let generation = self.function_primary.generation;
        self.function_primary.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.function_primary.generation != generation
                    || this.function_primary.recorder_held
                {
                    return;
                }
                this.function_primary.timer = None;
                this.clear_analog_mapping(cx);
                this.selected_key = None;
                this.draft["functionKeyPrimary"] = Value::from(match primary {
                    FunctionKeyPrimary::Function => "func",
                    FunctionKeyPrimary::Multimedia => "multi",
                });
                // The page value is optimistic; the workspace's SET owner keeps
                // it out of persisted profiles until the command ACK is valid.
                cx.emit(KeyboardProductChanged);
                cx.notify();
            });
        }));
    }

    pub(super) fn function_primary_panel(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if self.spec.product_id != 717 {
            return None;
        }
        let selected = self.function_primary();
        let held = self.function_primary.recorder_held;
        let rows = [
            (
                "keyboard-function-primary-func",
                "FUNCTION_KEYS",
                FunctionKeyPrimary::Function,
            ),
            (
                "keyboard-function-primary-multi",
                "MULTIMEDIA_KEYS",
                FunctionKeyPrimary::Multimedia,
            ),
        ]
        .into_iter()
        .map(|(id, key, primary)| {
            let owner = cx.entity().downgrade();
            Radio::new(id)
                .checked(selected == Some(primary))
                .disabled(held)
                .accessibility_label(t(key))
                .relative()
                .mb(surface::css(10.))
                .pl(surface::css(30.))
                .text_size(surface::css(14.))
                .line_height(surface::css(20.))
                .text_color(Colors::text())
                .focus_visible(|style| style.text_color(Colors::selected()))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .size(surface::css(20.))
                        .rounded_full()
                        .border_1()
                        .border_color(Colors::border()),
                )
                .child(Indicator {
                    id,
                    checked: selected == Some(primary),
                })
                .child(t(key))
                .on_change(move |_, _, _, cx| {
                    let _ = owner.update(cx, |this, cx| this.queue_function_primary(primary, cx));
                })
        });
        Some(
            surface::panel_with_help(
                t("FUNCTION_KEY_PRIMARY_HEADER"),
                surface::help_control(
                    "keyboard-function-primary-help",
                    t("FUNCTION_KEY_PRIMARY_TOOLTIP"),
                ),
                cx,
            )
            .when(held, |panel| panel.opacity(0.3))
            .child(v_flex().children(rows))
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyboardProductChanged, KeyboardProductWorkspace};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};
    use std::time::Duration;

    #[gpui_kit::test]
    fn function_primary_radio_debounces_native_input_and_cancels_with_its_owner(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let mut page = None;
        let window = cx.open_window(size(px(1300.), px(1800.)), |window, cx| {
            let view = cx.new(|cx| KeyboardProductWorkspace::new(717, false, window, cx));
            view.update(cx, |view, cx| {
                let mut saved = view.snapshot();
                saved["functionKeyPrimary"] = "multi".into();
                view.restore(Some(&saved), window, cx);
            });
            page = Some(view.clone());
            Root::new(view, window, cx)
        });
        let page = page.unwrap();
        let events = cx.new(|_| 0usize);
        let _subscription = events.update(cx, |_, cx| {
            cx.subscribe(&page, |events, _, _: &KeyboardProductChanged, _| {
                *events += 1
            })
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("keyboard-function-primary-multi").checked(),
                Some(true)
            );
            window.click("keyboard-function-primary-func", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(99));
        cx.run_until_parked();
        events.read_with(cx, |events, _| assert_eq!(*events, 0));
        page.read_with(cx, |page, _| assert!(page.function_system_layer("KEY_F5")));
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        events.read_with(cx, |events, _| assert_eq!(*events, 1));
        page.read_with(cx, |page, _| {
            assert_eq!(page.snapshot()["functionKeyPrimary"], "func");
            assert!(!page.function_system_layer("KEY_F5"));
            assert!(page.selected_key.is_none());
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("keyboard-function-primary-func").checked(),
                Some(true)
            );
            window.press("tab", cx);
            assert_eq!(
                window.find("keyboard-function-primary-multi").focused(),
                Some(true)
            );
            window.press("space", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        events.read_with(cx, |events, _| assert_eq!(*events, 2));
        page.read_with(cx, |page, _| {
            assert_eq!(page.snapshot()["functionKeyPrimary"], "multi")
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.click("keyboard-button-drawer-toggle", cx);
            assert!(window.try_find("analog-drawer-row-KEY_F5").is_none());
            window.click("keyboard-hypershift", cx);
            assert!(window.try_find("analog-drawer-row-KEY_F5").is_some());
            window.click("keyboard-button-drawer-toggle", cx);
            window.click("keyboard-hypershift", cx);
        })
        .unwrap();

        // A real recorder ownership notification cancels the captured click.
        cx.update_window(window.into(), |_, window, cx| {
            window.click("keyboard-function-primary-func", cx);
            page.update(cx, |page, cx| {
                page.set_function_primary_recorder_held(true, cx)
            });
            window.render_frame(cx);
            assert_eq!(
                window.find("keyboard-function-primary-func").disabled(),
                Some(true)
            );
            window.click("keyboard-function-primary-func", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        events.read_with(cx, |events, _| assert_eq!(*events, 2));
        cx.update_window(window.into(), |_, window, cx| {
            page.update(cx, |page, cx| {
                page.set_function_primary_recorder_held(false, cx)
            });
            window.click("keyboard-function-primary-func", cx);
            page.update(cx, |page, cx| {
                let restored = page.snapshot();
                page.restore(Some(&restored), window, cx);
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        events.read_with(cx, |events, _| assert_eq!(*events, 2));
        for disconnect in [false, true] {
            cx.update_window(window.into(), |_, window, cx| {
                window.click("keyboard-function-primary-func", cx);
                page.update(cx, |page, _| {
                    if disconnect {
                        page.set_legacy_mapping_connection(None);
                    } else {
                        page.set_mapping_callback_generation(71);
                    }
                });
            })
            .unwrap();
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(100));
            cx.run_until_parked();
            events.read_with(cx, |events, _| assert_eq!(*events, 2));
        }
    }
}
