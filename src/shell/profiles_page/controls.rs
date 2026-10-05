//! Controls mounted by the current Profiles 43/7693/5529 components.
use super::*;
use crate::ui::surface::css;

pub(super) fn nav_tip(id: &'static str, label: &'static str, button: BaseButton) -> AnyElement {
    use crate::ui::source_tooltip::{SourceTooltip, SourceTooltipKind};
    SourceTooltip::new(id, i18n::t(label), 300.)
        .kind(SourceTooltipKind::ProfilesNav)
        .trigger(move |_, _, _| button.into_any_element())
        .into_any_element()
}

#[derive(Default)]
pub(super) struct Pointer {
    pub(super) hovered: bool,
    pub(super) pressed: bool,
}

pub(super) fn pointer(id: ElementId, window: &mut Window, cx: &mut App) -> Entity<Pointer> {
    window.use_keyed_state((id, "profiles-pointer"), cx, |_, _| Pointer::default())
}

pub(super) fn track(
    button: BaseButton,
    state: &Entity<Pointer>,
    window: &mut Window,
) -> BaseButton {
    button
        .on_hover(window.listener_for(state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            if !hovered {
                state.pressed = false;
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(state, |state, _, _, cx| {
                state.pressed = true;
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(state, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(state, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
}

pub(super) fn close_button(id: &'static str, window: &mut Window, cx: &mut App) -> BaseButton {
    let state = pointer(id.into(), window, cx);
    let pointer = state.read(cx);
    let background: Hsla = if pointer.pressed {
        rgba(0x0000001a).into()
    } else if pointer.hovered {
        rgba(0xffffff1a).into()
    } else {
        rgba(0).into()
    };
    let background = motion::transition(
        (id, "background"),
        background,
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    track(BaseButton::new(id), &state, window)
        .accessibility_label(i18n::t("CLOSE"))
        .absolute()
        .right_0()
        .top_0()
        .size(css(36.))
        .p_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(background)
        .child(img("synapse/profiles-close.svg").size(css(20.)))
}

pub(super) fn more_button(open: bool, window: &mut Window, cx: &mut App) -> BaseButton {
    let id = "profiles-device-more";
    let state = pointer(id.into(), window, cx);
    let pointer = state.read(cx);
    let target = if open || pointer.pressed {
        0x44d62c
    } else if pointer.hovered {
        0x5d5d5d
    } else {
        0x222222
    };
    let border = motion::transition(
        (id, "border"),
        Hsla::from(rgb(target)),
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    track(BaseButton::new(id), &state, window)
        .accessibility_label("profile actions")
        .size(css(26.))
        .mr(css(10.))
        .p_0()
        .border_1()
        .border_color(border)
        .flex()
        .items_center()
        .justify_center()
        .child(img("synapse/profile-more.svg").size(css(20.)))
}

pub(super) fn linked_check(
    id: ElementId,
    active: bool,
    busy: bool,
    hovered: bool,
    pressed: bool,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    // The tile's more-specific active/hover rules override shared .check-box.
    let fill = if pressed && (active || busy) {
        0x2f941e
    } else if pressed {
        0x44d62c
    } else if active && hovered {
        0x7ce26c
    } else if active {
        0x44d62c
    } else if busy {
        0x000000
    } else {
        0x111111
    };
    let border = if pressed || active {
        fill
    } else if hovered {
        0x44d62c
    } else {
        0x737373
    };
    let border: Hsla = motion::transition(
        (id.clone(), "check-border"),
        Hsla::from(rgb(border)),
        Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
        window,
        cx,
    );
    let top = Presence::new((id.clone(), "check-top"), active)
        .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    let bottom = Presence::new((id, "check-bottom"), active)
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    div()
        .absolute()
        .left(css(10.))
        .top(css(10.))
        .size(css(20.))
        .border_1()
        .border_color(border)
        .rounded(css(2.4))
        .bg(rgb(fill))
        .when(active, |check| {
            check.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let scale = f32::from(bounds.size.width) / 20.;
                        for (x, y, angle, height) in [
                            (8.6_f32, 16.4_f32, -145_f32, 15.4 * top),
                            (0.8, 10.2, -50., 9.6 * bottom),
                        ] {
                            let angle = angle.to_radians();
                            let start = bounds.origin + point(px(x * scale), px(y * scale));
                            let mut path = PathBuilder::stroke(px(3. * scale));
                            path.move_to(start);
                            path.line_to(
                                start
                                    + point(
                                        px(-angle.sin() * height * scale),
                                        px(angle.cos() * height * scale),
                                    ),
                            );
                            if let Ok(path) = path.build() {
                                window.paint_path(path, rgb(0x111111));
                            }
                        }
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
        })
        .when(busy, |check| {
            check.child(
                div()
                    .absolute()
                    .left(css(4.))
                    .top(css(8.))
                    .w(css(10.))
                    .h(css(2.))
                    .bg(rgb(0x44d62c)),
            )
        })
        .into_any_element()
}

pub(super) fn linked_add(window: &mut Window, cx: &mut App) -> BaseButton {
    let id = "profiles-device-add-game";
    let state = pointer(id.into(), window, cx);
    let hover = state.read(cx).hovered || state.read(cx).pressed;
    let border: Hsla = motion::transition(
        (id, "border"),
        Hsla::from(rgb(if hover { 0x44d62c } else { 0x5d5d5d })),
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    track(BaseButton::new(id), &state, window)
        .accessibility_label(i18n::t("ADD_GAME_TITLE"))
        .p_0()
        .w(css(240.))
        .h(css(190.))
        .mx(css(5.))
        .mt(css(10.))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .items_stretch()
        .justify_start()
        .border_2()
        .border_dashed()
        .border_color(border)
        .rounded(css(5.))
        .child(
            div()
                .h(css(120.))
                .w_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(img("synapse/dashboard-add.svg").size(css(40.))),
        )
        // Source footer is ordinary block text, not vertically centered or muted.
        .child(
            div()
                .h(css(70.))
                .w_full()
                .flex_shrink_0()
                .text_size(css(14.))
                .line_height(css(16.))
                .text_center()
                .text_color(rgb(0xcccccc))
                .child(format!(
                    "{} {}\n{}",
                    i18n::t("CLICK_TO_ADD"),
                    i18n::t("GAME_PROGRAM"),
                    i18n::t("DRAG_AND_DROP_HERE")
                )),
        )
}

pub(super) fn trim_name(name: &str) -> &str {
    // ECMAScript String.trim: Rust's Unicode whitespace differs at BOM/NEL.
    name.trim_matches(|c| {
        matches!(c,
        '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
        '\u{205f}' | '\u{3000}' | '\u{feff}')
    })
}
