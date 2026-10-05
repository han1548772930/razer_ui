//! 58190.va / MacroItem_control_action: source SVGs, slots and CSS tooltips.
use super::*;

impl MacroPage {
    pub(super) fn row_controls(
        &self,
        index: usize,
        hovered: bool,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let alpha = motion::transition(
            (ElementId::from(("macro-row-controls", index)), "opacity"),
            if hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        h_flex()
            .w(relative(0.3))
            .min_w_0()
            .justify_end()
            .child(
                h_flex()
                    .opacity(alpha)
                    .child(
                        row_button(
                            ("macro-row-duplicate", index),
                            "duplicate",
                            "TEXT_TOOLTIP_DUPLICATE",
                            disabled,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.finish_pending_edits(window, cx);
                                this.duplicate_action(index, cx);
                            },
                        )),
                    )
                    .child(
                        row_button(
                            ("macro-row-delete", index),
                            "delete",
                            "TEXT_TOOLTIP_DELETE",
                            disabled,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.finish_pending_edits(window, cx);
                                this.delete_action(index, cx);
                            },
                        )),
                    )
                    // control_action span wins over draggable's 8x18 by
                    // specificity: the replaced content occupies 20x20.
                    .child(
                        div().size(css(20.)).flex_shrink_0().child(
                            img("synapse/macro/drag.svg")
                                .size(css(20.))
                                .object_fit(ObjectFit::Fill),
                        ),
                    ),
            )
            .into_any_element()
    }
}

fn row_button(
    id: impl Into<ElementId>,
    icon: &'static str,
    key: &'static str,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let id = id.into();
    let hover = window.use_keyed_state((id.clone(), "hover"), cx, |_, _| false);
    let hovered = *hover.read(cx) && !disabled;
    let alpha = motion::transition(
        (id.clone(), "tooltip"),
        if hovered { 1. } else { 0. },
        Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
        window,
        cx,
    );
    BaseButton::new(id)
        .relative()
        .size(css(20.))
        .flex_shrink_0()
        .p_0()
        .mr(css(16.))
        .disabled(disabled)
        .active(|s| s.opacity(0.3))
        .child(
            img(SharedString::from(format!(
                "synapse/macro/{icon}{}.svg",
                if icon == "delete" && hovered {
                    "-hover"
                } else {
                    ""
                }
            )))
            .size_full(),
        )
        .on_hover(move |value, _, cx| {
            hover.update(cx, |state, cx| {
                *state = *value;
                cx.notify();
            })
        })
        .when(hovered, |button| {
            button.child(
                div()
                    .absolute()
                    .right_0()
                    .top(css(35.))
                    .w_auto()
                    .h_auto()
                    .px(css(10.))
                    .py(css(8.))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .bg(rgb(0))
                    .text_color(rgb(0xcccccc))
                    .font_family("Roboto")
                    .text_size(css(14.))
                    .line_height(css(16.))
                    .whitespace_nowrap()
                    .text_left()
                    .opacity(alpha)
                    .child(tr(key)),
            )
        })
}
