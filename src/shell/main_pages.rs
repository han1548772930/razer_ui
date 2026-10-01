//! HomePage's four routes have different render trees (4130 / 6505 / 9388 / 7282).
//! Service-dependent groups remain absent until their real data is available.
use super::{AppShell, Location};
use crate::{nav::Tab, ui::surface};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

/// 55 CSS's 1279/600 breakpoints plus Li.getMaxColumns' 290px cards / 20px gap.
/// Normalize by our rem scale so larger text also causes the grid to reflow.
pub(super) struct MainLayout {
    pub(super) gutter: f32,
    pub(super) body_max_width: f32,
    dashboard_width: f32,
    narrow: bool,
}
impl MainLayout {
    pub(super) fn new(viewport_width: f32, rem_size: f32, device_count: usize) -> Self {
        let width = viewport_width * 16. / rem_size.max(1.);
        let narrow = width < 1280.;
        let gutter = if narrow { 30. } else { 20. };
        // Li.render adds .reflow when more than four devices are present. Its
        // higher-specificity max-width overrides the narrow .dashboard rule.
        let reflow = device_count > 4;
        let maximum: f32 = if reflow {
            2460.
        } else if width <= 600. {
            290.
        } else if narrow {
            910.
        } else {
            1220.
        };
        let available = (width - gutter * 2.).min(maximum);
        let columns = ((available + 20.) / 310.).floor().max(1.);
        Self {
            gutter,
            body_max_width: if reflow { 2460. + gutter * 2. } else { 1260. },
            dashboard_width: columns * 310. - 20.,
            narrow,
        }
    }
}

impl AppShell {
    pub(super) fn dashboard(&self, layout: &MainLayout, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("dashboard")
            .w(surface::css(layout.dashboard_width))
            .flex_shrink_0()
            .when(!layout.narrow, |this| this.mx_auto())
            .my(surface::css(10.))
            .child(
                Button::new("dashboard-devices-toggle")
                    .ghost()
                    .h(surface::css(18.))
                    .p_0()
                    .justify_start()
                    .text_size(surface::css(14.))
                    .accessibility_label(if self.dashboard_collapsed {
                        "展开设备"
                    } else {
                        "折叠设备"
                    })
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                Icon::default()
                                    .path("synapse/expand.svg")
                                    .size(surface::css(10.))
                                    .transform(Transformation::rotate(radians(
                                        if self.dashboard_collapsed {
                                            -std::f32::consts::FRAC_PI_2
                                        } else {
                                            0.
                                        },
                                    ))),
                            )
                            .child("设备"),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dashboard_collapsed = !this.dashboard_collapsed;
                        cx.notify();
                    })),
            )
            .when(!self.dashboard_collapsed, |view| {
                view.child(
                    h_flex()
                        .mt(surface::css(10.))
                        .gap(surface::css(20.))
                        .flex_wrap()
                        .children(self.devices.iter().map(|entity| {
                            let workspace = entity.read(cx);
                            let device = workspace.device();
                            let key = workspace.identity();
                            let supported = !Tab::for_product(device.product_id).is_empty();
                            Button::new(SharedString::from(format!("open-{key}")))
                                .accessibility_label(format!("打开 {}", device.display_name()))
                                .w(surface::css(290.))
                                .h_auto()
                                .p_0()
                                .border_0()
                                .rounded(cx.theme().font_size * (5. / 16.))
                                .custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().group_box)
                                        .hover(cx.theme().group_box)
                                        .active(cx.theme().group_box),
                                )
                                .disabled(!supported)
                                .child(
                                    v_flex()
                                        .w_full()
                                        .pt(surface::css(10.))
                                        .px(surface::css(20.))
                                        .pb(surface::css(9.))
                                        .gap(surface::css(8.))
                                        .child(dashboard_image(
                                            device.product_id,
                                            device.edition_id,
                                            250.,
                                            140.,
                                        ))
                                        .child(
                                            div()
                                                .w_full()
                                                .min_h(surface::css(17.))
                                                .text_size(surface::css(14.))
                                                .line_height(surface::css(16.))
                                                .text_center()
                                                .whitespace_normal()
                                                .child(device.display_name()),
                                        )
                                        .child(
                                            div()
                                                .text_size(surface::css(12.))
                                                .text_color(cx.theme().muted_foreground)
                                                .text_center()
                                                .child(
                                                    if device.serial_number.starts_with("PREVIEW-")
                                                    {
                                                        "预览设备"
                                                    } else {
                                                        "本地快照"
                                                    },
                                                ),
                                        ),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.navigate(Location::Device(key.clone()), window, cx);
                                }))
                        }))
                        .when(self.devices.is_empty(), |view| {
                            view.child(surface::note("没有可显示的设备。", cx))
                        }),
                )
            })
            .into_any_element()
    }

    pub(super) fn modules_page(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("devices-modules")
            .w(surface::css(1220.))
            .min_w(surface::css(1220.))
            .flex_shrink_0()
            .pb(surface::css(40.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(24.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(10.))
                    .child("设备"),
            )
            .children(self.devices.iter().map(|entity| {
                let workspace = entity.read(cx);
                let device = workspace.device();
                let key = workspace.identity();
                let supported = !Tab::for_product(device.product_id).is_empty();
                let detail_device = device.clone();
                h_flex()
                    .id(SharedString::from(format!("module-device-{key}")))
                    .w_full()
                    .h(surface::css(80.))
                    .mb(surface::css(1.))
                    .pl(surface::css(20.))
                    .pr(surface::css(30.))
                    .bg(cx.theme().group_box)
                    .child(dashboard_image(
                        device.product_id,
                        device.edition_id,
                        40.,
                        40.,
                    ))
                    .child(
                        div()
                            .ml(surface::css(10.))
                            .flex_basis(surface::css(500.))
                            .flex_shrink_1()
                            .min_w_0()
                            .text_size(surface::css(16.))
                            .text_ellipsis()
                            .child(device.display_name()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(surface::css(14.))
                            .text_color(cx.theme().muted_foreground)
                            .child("本地快照"),
                    )
                    .child(
                        Button::new(SharedString::from(format!("module-info-{key}")))
                            .ghost()
                            .label("详情")
                            .h(surface::css(27.))
                            .on_click(move |_, window, cx| {
                                super::service_pages::open_device_details(detail_device.clone(), window, cx);
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("module-open-{key}")))
                            .label("打开")
                            .h(surface::css(27.))
                            .min_w(surface::css(90.))
                            .ml(surface::css(30.))
                            .text_size(surface::css(12.))
                            .disabled(!supported)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.navigate(Location::Device(key.clone()), window, cx);
                            })),
                    )
            }))
            .child(div().mt(surface::css(20.)).child(surface::note(
                "尚未连接设备与模块服务，无法读取安装和固件更新状态。",
                cx,
            )))
            .child(surface::external(
                "module-information",
                "Razer 应用",
                "https://www.razer.com/software",
            ))
            .child(self.module_catalog.clone())
            .into_any_element()
    }

    pub(super) fn gamer_room_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.gamer_room.clone().into_any_element()
    }

    pub(super) fn shortcuts_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.shortcuts.clone().into_any_element()
    }

    pub(super) fn preview_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .flex_wrap()
            .gap(surface::css(12.))
            .child(
                Button::new("preview-keyboard")
                    .label("预览 653 键盘")
                    .outline()
                    .on_click(cx.listener(|this, _, w, cx| this.add_preview(653, w, cx))),
            )
            .child(
                Button::new("preview-headset")
                    .label("预览 777 耳机")
                    .outline()
                    .on_click(cx.listener(|this, _, w, cx| this.add_preview(777, w, cx))),
            )
            .into_any_element()
    }
}

fn dashboard_image(pid: u32, edition: u32, width: f32, height: f32) -> AnyElement {
    // PluginImages uses separate artwork. Missing downloads use a logo placeholder;
    // Customize's prd artwork must not be silently substituted for Dashboard.
    div()
        .w(surface::css(width))
        .h(surface::css(height))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(if pid == 182 && edition == 0 {
            img("synapse/dashboard-182.png")
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        } else {
            img("synapse/synapse.svg")
                .size(surface::css(height.min(100.)))
                .opacity(0.3)
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::MainLayout;

    #[test]
    fn dashboard_reflows_at_source_breakpoints_without_shrinking_cards() {
        let small = MainLayout::new(600., 16., 4);
        let compact = MainLayout::new(680., 16., 4);
        let below = MainLayout::new(1279., 16., 4);
        let above = MainLayout::new(1280., 16., 4);
        assert_eq!(small.dashboard_width, 290.);
        assert_eq!(compact.dashboard_width, 600.);
        assert_eq!((below.gutter, below.dashboard_width), (30., 910.));
        assert_eq!((above.gutter, above.dashboard_width), (20., 1220.));
        assert_eq!(MainLayout::new(2560., 16., 4).dashboard_width, 1220.);
        assert_eq!(MainLayout::new(2560., 16., 5).dashboard_width, 2460.);
        assert_eq!(MainLayout::new(1920., 16., 5).dashboard_width, 1840.);
    }

    #[test]
    fn larger_text_reflows_dashboard_instead_of_overlapping_cards() {
        let normal = MainLayout::new(1280., 16., 4);
        let larger = MainLayout::new(1280., 20., 4);
        assert_eq!(normal.dashboard_width, 1220.);
        assert_eq!(larger.dashboard_width, 910.);
        // Even a very narrow viewport keeps a whole card reachable by horizontal scroll.
        assert_eq!(MainLayout::new(280., 20., 4).dashboard_width, 290.);
    }
}
