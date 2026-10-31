use super::{FocusBehavior, FocusRing, focus_target};
use gpui_kit::base::{Button as BaseButton, ButtonStyles, RoleOverride, Selectable};
use gpui_kit::{
    AnyElement, App, ClickEvent, Corners, Display, ElementId, FocusHandle, InteractiveElement,
    Interactivity, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Visibility, Window, px,
};

/// Source button presentation and navigation over Kit's activation semantics.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    base: BaseButton,
    provided_focus_handle: Option<FocusHandle>,
    disabled: bool,
    focusable: bool,
    tab_index: isize,
    tab_stop: bool,
    focus_ring: FocusRing,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: BaseButton::new(id.clone()),
            id,
            provided_focus_handle: None,
            disabled: false,
            focusable: true,
            tab_index: 0,
            tab_stop: true,
            focus_ring: FocusRing::Standard,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.base = self.base.disabled(disabled);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.base = self.base.selected(selected);
        self
    }

    pub fn styles(mut self, build: impl FnOnce(ButtonStyles) -> ButtonStyles) -> Self {
        self.base = self.base.styles(build);
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.base = self.base.accessibility_label(label);
        self
    }

    pub fn role(mut self, role: impl Into<RoleOverride>) -> Self {
        self.base = self.base.role(role);
        self
    }

    pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
        self.provided_focus_handle = Some(focus_handle.clone());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.base = self.base.on_click(handler);
        self
    }

    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self.base = self.base.tab_index(tab_index);
        self
    }

    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self.base = self.base.tab_stop(tab_stop);
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self.base = self.base.focusable(focusable);
        self
    }

    /// Set at the source control owner, where its CSS focus branch is known.
    pub fn focus_ring(mut self, ring: FocusRing) -> Self {
        self.focus_ring = ring;
        self
    }
}

impl Selectable for Button {
    fn selected(self, selected: bool) -> Self {
        Button::selected(self, selected)
    }

    fn is_selected(&self) -> bool {
        self.base.is_selected()
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl InteractiveElement for Button {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Button {}

impl RenderOnce for Button {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self
            .provided_focus_handle
            .unwrap_or_else(|| {
                window
                    .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
            })
            .tab_index(self.tab_index)
            .tab_stop(self.tab_stop && !self.disabled);
        let style = self.base.style();
        let visible =
            style.visibility != Some(Visibility::Hidden) && style.display != Some(Display::None);
        let resolve = |radius: Option<gpui_kit::AbsoluteLength>| {
            radius.map_or(px(0.), |radius| radius.to_pixels(window.rem_size()))
        };
        let radii = Corners {
            top_left: resolve(style.corner_radii.top_left),
            top_right: resolve(style.corner_radii.top_right),
            bottom_left: resolve(style.corner_radii.bottom_left),
            bottom_right: resolve(style.corner_radii.bottom_right),
        };
        focus_target(
            self.base.track_focus(&focus),
            &focus,
            FocusBehavior::Spatial,
            self.focusable && !self.disabled && visible,
        )
        .ring(self.focus_ring)
        .radii(radii)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::base::Root;
    use gpui_kit::component::Theme;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, Context, Render, TestAppContext, div, size};

    struct Buttons {
        first: FocusHandle,
        last: FocusHandle,
        keyed_disabled: bool,
        activations: Vec<&'static str>,
    }

    impl Render for Buttons {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let control = |id| Button::new(id).size(px(36.));
            div()
                .flex()
                .gap(px(12.))
                .p(px(20.))
                .child(control("first").track_focus(&self.first))
                .child(
                    control("pointer-only")
                        .focusable(false)
                        .on_click(cx.listener(|this, _, _, _| {
                            this.activations.push("pointer-only");
                        })),
                )
                .child(
                    control("disabled")
                        .disabled(true)
                        .on_click(cx.listener(|this, _, _, _| {
                            this.activations.push("disabled");
                        })),
                )
                .child(control("hidden").invisible())
                .child(control("display-none").hidden())
                .child(
                    control("keyed")
                        .disabled(self.keyed_disabled)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.activations.push("keyed");
                            cx.notify();
                        })),
                )
                .child(control("last").track_focus(&self.last))
        }
    }

    #[gpui_kit::test]
    fn button_navigation_preserves_focus_and_activation_requires_window(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            Theme::update(cx, |theme| theme.font_size = px(16.));
            super::super::init(cx);
        });
        let mut buttons = None;
        let handle = cx.open_window(size(px(400.), px(160.)), |window, cx| {
            let view = cx.new(|cx| Buttons {
                first: cx.focus_handle(),
                last: cx.focus_handle(),
                keyed_disabled: false,
                activations: Vec::new(),
            });
            view.read(cx).first.focus(window, cx);
            buttons = Some(view.clone());
            Root::new(view, window, cx)
        });
        let buttons = buttons.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press("right", cx);
            let keyed = window.focused(cx).expect("available button to the right");
            assert!(!buttons.read(cx).first.is_focused(window));
            assert!(!buttons.read(cx).last.is_focused(window));
            window.press("space", cx);
            window.render_frame(cx);
            assert_eq!(buttons.read(cx).activations, ["keyed"]);
            assert!(
                keyed.is_focused(window),
                "keyed focus survives a new render"
            );

            window.press("right", cx);
            assert!(buttons.read(cx).last.is_focused(window));
            window.press("left", cx);
            assert!(keyed.is_focused(window));
            window.click("pointer-only", cx);
            assert!(
                keyed.is_focused(window),
                "step controls preserve sibling focus"
            );
            window.click("disabled", cx);
            assert_eq!(buttons.read(cx).activations, ["keyed", "pointer-only"]);

            buttons.update(cx, |this, cx| {
                this.keyed_disabled = true;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(keyed.is_focused(window));
            window.press("space", cx);
            assert_eq!(buttons.read(cx).activations, ["keyed", "pointer-only"]);
            window.focus_next(cx);
            assert!(buttons.read(cx).last.is_focused(window));
            window.focus_prev(cx);
            assert!(buttons.read(cx).first.is_focused(window));
        })
        .unwrap();
    }
}
