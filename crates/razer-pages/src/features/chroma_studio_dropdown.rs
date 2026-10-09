//! Current 1592 dropdown lifecycle, composed with Base Popover focus/dismissal.
use super::*;
use gpui_kit::base::Popover;
use std::time::Duration;

pub(super) struct DropdownState {
    open: bool,
    shown: bool,
    closing: bool,
    task: Option<Task<()>>,
    width: Pixels,
}
impl DropdownState {
    pub(super) fn new() -> Self {
        Self {
            open: false,
            shown: false,
            closing: false,
            task: None,
            width: px(0.),
        }
    }
    pub(super) fn is_open(&self) -> bool {
        self.open && !self.closing
    }
    pub(super) fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if open == self.is_open() {
            return;
        }
        self.open = true;
        self.shown = false;
        self.closing = !open;
        self.task = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = owner.update(cx, |this, cx| {
                this.open = open;
                this.shown = open;
                this.closing = false;
                cx.notify();
            });
        }));
        cx.notify();
    }
}

#[derive(IntoElement)]
pub(super) struct StudioDropdown {
    id: ElementId,
    state: Entity<DropdownState>,
    enabled: bool,
    trigger: BaseButton,
    body: AnyElement,
    width: f32,
    shift: f32,
    block: bool,
}
impl StudioDropdown {
    pub(super) fn new(
        id: impl Into<ElementId>,
        state: &Entity<DropdownState>,
        enabled: bool,
        trigger: BaseButton,
        body: AnyElement,
        width: f32,
        shift: f32,
    ) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            enabled,
            trigger,
            body,
            width,
            shift,
            block: false,
        }
    }
}
impl StudioDropdown {
    pub(super) fn block(mut self) -> Self {
        self.block = true;
        self
    }
}
impl RenderOnce for StudioDropdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let open = state.open;
        let shown = state.shown;
        let owner = self.state.clone();
        let enabled = self.enabled;
        let measured = self.state.clone();
        let content_state = self.state.clone();
        let trigger = self.trigger.disabled(!enabled).when(self.block, |trigger| {
            trigger.child(
                canvas(
                    move |bounds, _, cx| {
                        if measured.read(cx).width != bounds.size.width {
                            let measured = measured.clone();
                            cx.defer(move |cx| {
                                measured.update(cx, |state, cx| {
                                    state.width = bounds.size.width;
                                    cx.notify();
                                })
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
        });
        Popover::new(self.id)
            .when(self.block, |view| view.w_full())
            .open(open)
            .anchor(Anchor::TopLeft)
            .offset(surface::css(2.).to_pixels(window.rem_size()))
            .trigger(trigger)
            .on_open_change(move |open, _, cx| {
                owner.update(cx, |state, cx| state.set_open(enabled && *open, cx));
            })
            .content(move |_, _, cx| {
                div()
                    .w(surface::css(self.width))
                    .when(self.block, |view| {
                        view.w(content_state.read(cx).width + px(2.))
                    })
                    .ml(surface::css(-self.shift))
                    .bg(Colors::black())
                    .border_color(Colors::input_border())
                    .border_x_1()
                    .when(shown, |view| view.border_y_1())
                    .when(!shown, |view| view.h_0().overflow_hidden())
                    .child(self.body)
            })
    }
}

pub(super) fn arrow(
    open: bool,
    id: ElementId,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    use gpui_kit::base::motion::{self, Easing, Transition};
    svg()
        .path("synapse/chroma-studio-brightness-pointer.svg")
        .w(surface::css(10.))
        .h(surface::css(5.))
        .text_color(Colors::dropdown_arrow())
        .with_transformation(Transformation::rotate(radians(motion::transition(
            id,
            if open { std::f32::consts::PI } else { 0. },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        ))))
}
