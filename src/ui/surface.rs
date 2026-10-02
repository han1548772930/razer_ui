use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

pub(crate) use super::synapse_select::select;

pub(crate) const BODY_MIN_WIDTH: f32 = 600.;
pub(crate) const BODY_MAX_WIDTH: f32 = 1240.;
pub(crate) const WIDGET_WIDTH: f32 = 600.;
pub(crate) const WIDGET_GAP: f32 = 20.;
pub(crate) const COLUMN_STACK_MAX_WIDTH: f32 = 1279.;
pub(crate) const COMPACT_COLUMN_MARGIN: f32 = 30.;
pub(crate) const WIDGET_PADDING_X: f32 = 40.;
pub(crate) const WIDGET_PADDING_Y: f32 = 30.;
pub(crate) const WIDGET_RADIUS: f32 = 5.;
pub(crate) const CONFIG_WRAPPER_MIN_WIDTH: f32 = 770.;
pub(crate) const CONFIG_WRAPPER_MAX_WIDTH: f32 = 1220.;

// Reference CSS uses a 16px root. Keep its ratios through GPUI's rem scale.
pub(crate) fn css(value: f32) -> Rems {
    rems(value / 16.)
}

/// Source media queries use viewport CSS pixels; normalize our rem-scaled UI
/// back to the source's 16px root before applying the 1279px breakpoint.
pub(crate) fn stacked_device_columns(viewport_width: f32, root_font_size: f32) -> bool {
    viewport_width * 16. / root_font_size.max(1.) <= COLUMN_STACK_MAX_WIDTH
}

pub(crate) fn navigation_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Button {
    Button::new(id)
        .xsmall()
        .label(label)
        .selected(selected)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().transparent
                })
                .foreground(if selected {
                    cx.theme().primary_foreground
                } else {
                    cx.theme().muted_foreground
                })
                .hover(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().secondary_hover
                })
                .active(cx.theme().primary),
        )
        .h(css(28.))
        .px(css(10.))
        .py_0()
        .text_size(css(12.))
        .rounded(cx.theme().font_size * (14. / 16.))
        .border_0()
}

pub(crate) fn asset_button(
    id: &'static str,
    asset: &'static str,
    label: &'static str,
    cx: &App,
) -> Button {
    let states = match asset {
        "synapse/eq-reset.svg" => {
            Some(("synapse/eq-reset-hover.svg", "synapse/eq-reset-active.svg"))
        }
        "synapse/help-default.svg" => Some(("synapse/help-hover.svg", "synapse/help-active.svg")),
        _ => None,
    };
    Button::new(id)
        .ghost()
        .p_0()
        .border_0()
        .rounded(cx.theme().radius)
        .size(css(28.))
        .accessibility_label(label)
        .tooltip(label)
        .custom(
            ButtonCustomVariant::new(cx)
                .hover(cx.theme().secondary_hover)
                .active(cx.theme().group_box),
        )
        .child(
            div()
                .id("source-icon")
                .group("source-icon")
                .relative()
                .size(css(20.))
                .child(img(asset).size_full().object_fit(ObjectFit::Contain))
                .when_some(states, |this, (hover, active)| {
                    this.child(
                        img(hover)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_hover("source-icon", |s| s.opacity(1.)),
                    )
                    .child(
                        img(active)
                            .id("icon-pressed")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_active("source-icon", |s| s.opacity(1.)),
                    )
                }),
        )
}

/// Source .switch: 32x18 track, 14px handle, round in both states.
/// Base owns controlled activation, keyboard focus and accessibility.
#[derive(IntoElement)]
pub(crate) struct SynapseSwitch {
    id: ElementId,
    base: gpui_kit::base::Switch,
    checked: bool,
    disabled: bool,
    label: Option<SharedString>,
}
impl SynapseSwitch {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: gpui_kit::base::Switch::new(id.clone()),
            id,
            checked: false,
            disabled: false,
            label: None,
        }
    }
    pub(crate) fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self.base = self.base.checked(checked);
        self
    }
    pub(crate) fn label(mut self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.base = self.base.accessibility_label(label.clone());
        self.label = Some(label);
        self
    }
    pub(crate) fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.base = self.base.accessibility_label(label);
        self
    }
    pub(crate) fn on_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.base = self
            .base
            .on_change(move |value, _, window, cx| handler(&value, window, cx));
        self
    }
}
impl Disableable for SynapseSwitch {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.base = self.base.disabled(disabled);
        self
    }
}
impl RenderOnce for SynapseSwitch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.id, cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        self.base
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_2()
            .text_size(css(14.))
            .text_color(cx.theme().foreground)
            .when(self.disabled, |s| s.opacity(0.3))
            .when(!self.disabled, |s| s.hover(|s| s.opacity(0.7)))
            .child(
                div()
                    .id("switch-track")
                    .test_support()
                    .w(css(32.))
                    .h(css(18.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .p(css(1.))
                    .border_1()
                    .border_color(if focused {
                        cx.theme().ring
                    } else {
                        cx.theme().title_bar.opacity(0.3)
                    })
                    .rounded(css(16.))
                    .bg(if self.checked {
                        cx.theme().primary
                    } else {
                        cx.theme().switch
                    })
                    .when(self.checked, |s| s.justify_end())
                    .child(
                        div()
                            .size(css(14.))
                            .rounded_full()
                            .bg(cx.theme().switch_thumb),
                    ),
            )
            .when_some(self.label, |s, label| s.child(div().min_w_0().child(label)))
    }
}

/// .dot-bg's 22px grid and .dim-corner's radial fade, drawn behind content.
pub(crate) fn dot_background(cx: &App) -> AnyElement {
    let color = cx.theme().border;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let scale = window.rem_size() / 16.;
            let step = scale * 22.;
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let step = f32::from(step);
            let start_x = (width / 2. - step / 2.).rem_euclid(step);
            let start_y = (height / 2. - step / 2.).rem_euclid(step);
            for row in 0..=(height / step) as usize {
                for col in 0..=(width / step) as usize {
                    let x = start_x + col as f32 * step;
                    let y = start_y + row as f32 * step;
                    // CSS radial-gradient defaults to an ellipse reaching the corners.
                    let radius = (((x - width / 2.) / (width / 2.)).powi(2)
                        + ((y - height / 2.) / (height / 2.)).powi(2))
                    .sqrt()
                        / 2_f32.sqrt();
                    window.paint_quad(fill(
                        Bounds::new(
                            bounds.origin + point(px(x), px(y)),
                            size(scale * 2., scale * 2.),
                        ),
                        color.opacity((1. - radius).clamp(0., 1.)),
                    ));
                }
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

pub(crate) fn page_columns() -> DeviceColumns {
    DeviceColumns {
        children: Vec::new(),
        style: StyleRefinement::default(),
    }
}

/// `.widget-col` gains 30px side margins below the source's 1280px boundary.
/// That rule forces the two fixed 600px cards onto separate rows even at 1279px,
/// where ordinary flex wrapping of 600 + 20 + 600 would still fit.
#[derive(IntoElement)]
pub(crate) struct DeviceColumns {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}
impl ParentElement for DeviceColumns {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}
impl Styled for DeviceColumns {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for DeviceColumns {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let stacked = stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        h_flex()
            .id("page-columns")
            .test_support()
            .w_full()
            .max_w(css(BODY_MAX_WIDTH))
            .mx_auto()
            .gap(css(WIDGET_GAP))
            .flex_wrap()
            .items_start()
            .justify_center()
            .when(stacked, |this| {
                this.flex_col()
                    .items_center()
                    .px(css(COMPACT_COLUMN_MARGIN))
                    .min_w(css(WIDGET_WIDTH + COMPACT_COLUMN_MARGIN * 2.))
            })
            .refine_style(&self.style)
            .children(self.children)
    }
}

pub(crate) fn page_column(content: impl IntoElement) -> AnyElement {
    v_flex()
        .flex_grow(0.)
        .flex_shrink_0()
        .w(css(WIDGET_WIDTH))
        .min_w(css(WIDGET_WIDTH))
        .max_w(css(WIDGET_WIDTH))
        .child(content)
        .into_any_element()
}

pub(crate) fn panel(title: impl Into<SharedString>, cx: &App) -> Div {
    panel_with_control(title, div(), cx)
}
pub(crate) fn panel_with_control(
    title: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    // Original .widget .titleRow .title applies text-transform: uppercase.
    let title = title.into().to_uppercase();
    v_flex()
        .w_full()
        .gap_4()
        .py(css(WIDGET_PADDING_Y))
        .px(css(WIDGET_PADDING_X))
        .bg(cx.theme().group_box)
        .rounded(css(WIDGET_RADIUS))
        .text_size(css(14.))
        .child(
            h_flex()
                .gap(css(20.))
                .child(
                    div()
                        .font_family("RazerF5")
                        .text_size(css(16.))
                        .text_color(cx.theme().primary)
                        .child(title),
                )
                .child(control),
        )
}
pub(crate) fn note(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
pub(crate) fn external(
    id: &'static str,
    label: impl Into<SharedString>,
    url: &'static str,
) -> Button {
    Button::new(id)
        .label(label)
        .outline()
        .icon(gpui_kit::assets::IconName::ExternalLink)
        .on_click(move |_, _, cx| cx.open_url(url))
}
pub(crate) fn product_image(pid: u32, edition_id: u32, layout_id: u32) -> AnyElement {
    div()
        .size_full()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .when_some(
            crate::resources::device_image(
                pid,
                edition_id,
                layout_id,
                crate::resources::DeviceImage::Product,
            ),
            |this, path| this.child(img(path).object_fit(ObjectFit::Contain).size_full()),
        )
        .into_any_element()
}

pub(crate) fn product_banner(pid: u32, edition_id: u32, layout_id: u32, cx: &App) -> AnyElement {
    div()
        .relative()
        .flex()
        .justify_center()
        .items_center()
        .w_full()
        .h(css(250.))
        .max_w(css(1220.))
        .min_w(css(1024.))
        .mx_auto()
        .my(css(10.))
        .child(dot_background(cx))
        .child(
            div()
                .relative()
                .w(css(325.))
                .h_full()
                .child(product_image(pid, edition_id, layout_id)),
        )
        .into_any_element()
}

pub(crate) fn config_wrapper() -> Div {
    div()
        .relative()
        .w_full()
        .min_w(css(CONFIG_WRAPPER_MIN_WIDTH))
        .max_w(css(CONFIG_WRAPPER_MAX_WIDTH))
        .mx_auto()
}
