//! Current systrayv2 launcher presentation (554.2573b048).
//!
//! The list is deliberately data-in/data-out.  A caller must provide the
//! launcher observations produced by the host/renderer path; this module does
//! not manufacture a Synapse row or infer installed applications.
use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::*;
use razer_widgets::surface;
use razer_widgets::theme::TrayColors;
use std::time::Duration;

/// One renderer `app.launchers` item after its title and logo are resolved.
#[derive(Clone, Debug, Default)]
pub struct TrayLauncher {
    pub name: String,
    pub title: String,
    pub logo: SharedString,
}

/// Render `.systray>.apps` using the current source dimensions and palette.
/// The click callback is the host-owned activation boundary.
pub fn launcher_list(
    launchers: &[TrayLauncher],
    on_click: impl Fn(&TrayLauncher, &mut Window, &mut App) + Clone + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let one = launchers.len() == 1;
    let mut list = div()
        .id("tray-launchers")
        .w_full()
        .h(surface::css(60.))
        .flex_shrink_0()
        .bg(TrayColors::launcher())
        .border_t_1()
        .border_color(TrayColors::surface());
    if !one {
        list = list.flex().flex_wrap().justify_around();
    }
    for launcher in launchers {
        let item = launcher.clone();
        let title = launcher.title.to_uppercase();
        let id = format!("tray-launcher-{}", launcher.name);
        let state = window.use_keyed_state((ElementId::from(id.clone()), "states"), cx, |_, _| {
            LauncherState::default()
        });
        let hovered = state.read(cx).hovered;
        let pressed = state.read(cx).pressed;
        let bg = motion::transition(
            (id.clone(), "background"),
            if hovered {
                TrayColors::border()
            } else {
                TrayColors::launcher()
            },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        let fg = motion::transition(
            (id.clone(), "title"),
            if hovered {
                TrayColors::hover_text()
            } else {
                TrayColors::muted()
            },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        let opacity = motion::transition(
            (id.clone(), "icon"),
            if pressed { 0.3_f32 } else { 1. },
            Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
            window,
            cx,
        );
        let mut row = BaseButton::new(format!("tray-launcher-{}", launcher.name))
            .relative()
            .h(surface::css(59.))
            .px(surface::css(10.))
            .bg(bg)
            .line_height(relative(1.22))
            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                state.hovered = *hovered;
                if !hovered {
                    state.pressed = false;
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_click({
                let callback = on_click.clone();
                move |_, window, cx| callback(&item, window, cx)
            })
            .child(
                img(launcher.logo.clone())
                    .size(surface::css(32.))
                    .flex_shrink_0()
                    .opacity(opacity),
            );
        if one {
            row = row.w_full().flex().items_center().justify_center().child(
                div()
                    .min_w_0()
                    .ml(surface::css(10.))
                    .text_size(surface::css(12.))
                    .text_color(fg)
                    .truncate()
                    .child(title),
            );
        } else {
            row = row.flex_1().flex().items_center().justify_center();
        }
        list = list.child(row);
    }
    list.into_any_element()
}

#[derive(Default)]
struct LauncherState {
    hovered: bool,
    pressed: bool,
}

/// The source `.app-version` badge is fixed to the viewport's bottom-right.
/// The version is supplied by the audited source bundle rather than Cargo's
/// package version, which is a different product identifier.
pub fn version_badge(version: impl Into<SharedString>) -> AnyElement {
    deferred(
        div()
            .id("tray-app-version")
            .absolute()
            .right_0()
            .bottom_0()
            .px(surface::css(5.))
            .py(surface::css(2.))
            .bg(rgba(0x00000033))
            .text_color(rgb(0x888888))
            .text_size(surface::css(12.))
            .child(format!("v{}", version.into())),
    )
    .with_priority(1030)
    .into_any_element()
}
