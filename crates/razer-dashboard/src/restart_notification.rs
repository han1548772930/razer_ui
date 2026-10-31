//! Current notification-manager 2724 / Root.useV / 4411.useP.
//! The caller list is process-local; dismissing the window does not clear it.
use gpui_kit::component::*;
use gpui_kit::*;
use razer_i18n as i18n;
use razer_widgets::keyboard_navigation::Button;
use razer_widgets::{surface, theme::RestartNotificationColors as Colors};
use serde_json::Value;
use std::time::{Duration, Instant};

pub const STORAGE_KEY: &str = "notification_manager_unique_noti_restart";
pub const WINDOW_NAME: &str = "notification-restart";

#[derive(Default)]
pub struct RestartCallers {
    callers: Vec<String>,
}

impl RestartCallers {
    /// Consume the real host event payload. Some(bool) requests the source
    /// useV effect, including repeated true events from an existing caller.
    pub fn consume(&mut self, event: &Value) -> Result<Option<bool>, String> {
        if event["key"] != STORAGE_KEY
            || !matches!(event["keyState"].as_str(), Some("created" | "changed"))
        {
            return Ok(None);
        }
        let value = match event.get("newValue") {
            Some(Value::String(encoded)) => {
                serde_json::from_str::<Value>(encoded).map_err(|error| error.to_string())?
            }
            Some(value @ (Value::Bool(_) | Value::Number(_) | Value::Null)) => value.clone(),
            _ => return Err("restart notification value is not JSON text".into()),
        };
        let requested = match &value {
            Value::Null => false,
            Value::Bool(value) => *value,
            Value::Number(value) => value.as_f64() != Some(0.),
            Value::String(value) => !value.is_empty(),
            Value::Array(_) | Value::Object(_) => true,
        };
        if requested {
            let Some(source) = event["srcURL"].as_str().filter(|source| !source.is_empty()) else {
                return Ok(None);
            };
            if !self.callers.iter().any(|caller| caller == source) {
                self.callers.push(source.to_owned());
            }
        } else {
            // Original remove-unique-notification clears all callers, even
            // when the false signal is sent by just one application.
            self.callers.clear();
        }
        Ok(Some(!self.callers.is_empty()))
    }
}

pub enum RestartNotificationEvent {
    RestartRequested,
    SizeChanged(gpui_kit::Size<Pixels>),
}

/// One native notification window owns focus, measurement and its close timer.
pub struct RestartNotification {
    focus: FocusHandle,
    closing: Option<Instant>,
    close_task: Option<Task<()>>,
    measured: Option<(u32, u32)>,
}

impl EventEmitter<RestartNotificationEvent> for RestartNotification {}

impl RestartNotification {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            focus,
            closing: None,
            close_task: None,
            measured: None,
        }
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing.is_some() {
            return;
        }
        self.closing = Some(Instant::now());
        self.close_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let _ = this.update_in(cx, |_, window, _| window.remove_window());
        }));
        cx.notify();
    }

    fn action(id: &'static str, label: &str, primary: bool) -> Button {
        Button::new(id)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .min_w(surface::css(90.))
            .px(surface::css(16.))
            .pt(surface::css(7.))
            .pb(surface::css(6.))
            .border_1()
            .border_color(Colors::button_border())
            .rounded(surface::css(3.))
            .font_family("Roboto")
            .text_size(surface::css(12.))
            .line_height(relative(1.22))
            .bg(if primary { Colors::accent() } else { Colors::later() })
            .text_color(if primary {
                Colors::button_border()
            } else {
                Colors::text()
            })
            .hover(|style| style.opacity(0.9))
            .focus_visible(|style| style.border_color(Colors::text()))
            .accessibility_label(i18n::t(label))
            .child(i18n::t(label))
    }
}

// 4411's ResizeObserver uses ceil, plus one further pixel for fractional sizes.
pub fn source_window_dimension(value: f32) -> u32 {
    let rounded = value.ceil();
    (if value == rounded { rounded } else { rounded + 1. }).max(0.) as u32
}

fn close_opacity(elapsed: Duration) -> f32 {
    (1. - elapsed.as_secs_f32() / 0.2).clamp(0., 1.)
}

impl Render for RestartNotification {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opacity = self
            .closing
            .map(|start| close_opacity(start.elapsed()))
            .unwrap_or(1.);
        if self.closing.is_some() && opacity > 0. {
            window.request_animation_frame();
        }
        let owner = cx.entity().downgrade();
        let panel = div()
            .id("restart-notification")
            .track_focus(&self.focus)
            .absolute()
            .left_0()
            .top_0()
            .flex()
            .flex_col()
            .items_center()
            // The source fixed-position shrink-to-fit box contains a 440px
            // description, two 20px insets and two 1px borders (border-box).
            .w(surface::css(482.))
            .min_w(surface::css(480.))
            .min_h(surface::css(162.))
            .p(surface::css(20.))
            .gap(surface::css(20.))
            .border_1()
            .border_color(Colors::accent())
            .rounded(surface::css(5.))
            .bg(Colors::surface())
            .text_color(Colors::text())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .opacity(opacity)
            .child(
                Button::new("restart-close")
                    .absolute()
                    .top(surface::css(8.))
                    .right(surface::css(8.))
                    .size(surface::css(20.))
                    .p_0()
                    .accessibility_label("Close")
                    .rounded(surface::css(2.))
                    .hover(|style| style.bg(Colors::close_hover()))
                    .focus_visible(|style| style.border_1().border_color(Colors::accent()))
                    .child(img("synapse/notification-restart-close.svg").size_full())
                    .on_click(cx.listener(|this, _, window, cx| this.dismiss(window, cx))),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .text_color(Colors::accent())
                    .child(i18n::t("SYSTEM_RESTART_REQUIRED").to_uppercase()),
            )
            .child(
                div()
                    .w(surface::css(440.))
                    .flex_shrink_0()
                    .whitespace_normal()
                    .text_center()
                    .child(i18n::t("SYSTEM_RESTART_REQUIRED_NOTE")),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap(surface::css(10.))
                    // `.disabled` is a CSS class only. LATER remains active.
                    .child(
                        Self::action("restart-later", "LATER", false)
                            .on_click(cx.listener(|this, _, window, cx| this.dismiss(window, cx))),
                    )
                    .child(
                        Self::action("restart-now", "RESTART_NOW", true).on_click(cx.listener(
                            |_, _, _, cx| cx.emit(RestartNotificationEvent::RestartRequested),
                        )),
                    ),
            );
        let content = div()
            .size_full()
            // ResizeObserver reads the panel's border box. An inset canvas
            // would measure its padding box and lose both source borders.
            .on_children_prepainted(move |bounds, window, cx| {
                let Some(bounds) = bounds.first() else { return };
                let size = (
                    source_window_dimension(f32::from(bounds.size.width)),
                    source_window_dimension(f32::from(bounds.size.height)),
                );
                let owner = owner.clone();
                window.defer(cx, move |_, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.measured != Some(size) {
                            this.measured = Some(size);
                            cx.emit(RestartNotificationEvent::SizeChanged(gpui_kit::size(
                                px(size.0 as f32),
                                px(size.1 as f32),
                            )));
                        }
                    });
                });
            })
            .child(panel);
        image_cache(razer_assets::resource_image_cache())
            .size_full()
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::{RestartCallers, STORAGE_KEY, close_opacity, source_window_dimension};
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn repeated_caller_reopens_a_dismissed_window_without_duplicate_callers() {
        let mut callers = RestartCallers::default();
        let event = json!({"key": STORAGE_KEY, "keyState":"created", "srcURL":"dashboard", "newValue":"true"});
        assert_eq!(callers.consume(&event).unwrap(), Some(true));
        assert_eq!(callers.consume(&event).unwrap(), Some(true));
        assert_eq!(callers.callers, ["dashboard"]);
    }

    #[test]
    fn false_from_one_caller_clears_every_source() {
        let mut callers = RestartCallers::default();
        for source in ["dashboard", "device"] {
            callers.consume(&json!({"key":STORAGE_KEY, "keyState":"changed", "srcURL":source, "newValue":"true"})).unwrap();
        }
        assert_eq!(callers.consume(&json!({"key":STORAGE_KEY, "keyState":"changed", "srcURL":"device", "newValue":"false"})).unwrap(), Some(false));
        assert!(callers.callers.is_empty());
    }

    #[test]
    fn ignored_and_invalid_events_never_create_a_restart_request() {
        let mut callers = RestartCallers::default();
        for state in ["removed", "cleared"] {
            assert_eq!(callers.consume(&json!({"key":STORAGE_KEY, "keyState":state, "srcURL":"dashboard", "newValue":"true"})).unwrap(), None);
        }
        assert!(callers.consume(&json!({"key":STORAGE_KEY, "keyState":"changed", "srcURL":"dashboard", "newValue":"{"})).is_err());
        assert!(callers.callers.is_empty());
    }

    #[test]
    fn fractional_measurements_and_close_timing_match_the_source() {
        assert_eq!(source_window_dimension(162.), 162);
        assert_eq!(source_window_dimension(162.25), 164);
        assert_eq!(close_opacity(Duration::from_millis(100)), 0.5);
        assert_eq!(close_opacity(Duration::from_millis(200)), 0.);
        assert_eq!(close_opacity(Duration::from_millis(250)), 0.);
    }
}
