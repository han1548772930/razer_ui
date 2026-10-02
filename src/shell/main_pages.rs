//! HomePage's four routes have different render trees (4130 / 6505 / 9388 / 7282).
//! Service-dependent groups remain absent until their real data is available.
use super::{AppShell, Location};
use crate::{
    i18n,
    nav::Tab,
    ui::{surface, theme::MainPageColors},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};

mod dashboard_cards;
mod dashboard_grid;
mod dashboard_group;
mod dashboard_tutorial;
use dashboard_grid::{DashboardCard, DashboardGrid};
pub(super) use dashboard_grid::{DashboardChanged, DashboardState};
use dashboard_group::{CollapseIcon, DashboardGroupContent};
pub(super) use dashboard_tutorial::{DashboardTutorial, DashboardTutorialEvent};

/// 55 CSS's 1279/600 breakpoints plus Li.getMaxColumns' 290px cards / 20px gap.
/// Normalize by our rem scale so larger text also causes the grid to reflow.
pub(super) struct MainLayout {
    pub(super) gutter: f32,
    pub(super) body_max_width: f32,
    dashboard_width: f32,
    narrow: bool,
}

fn dashboard_caption(label: &str, description: Option<&str>, cx: &App) -> impl IntoElement {
    v_flex()
        .w_full()
        .items_center()
        .text_center()
        .whitespace_normal()
        .child(
            div()
                .px(surface::css(10.))
                .min_h(surface::css(17.))
                .w_full()
                .text_size(surface::css(14.))
                .line_height(surface::css(16.))
                .text_color(cx.theme().foreground)
                .child(i18n::t(label).to_uppercase()),
        )
        .when_some(description, |caption, description| {
            caption.child(
                div()
                    .w_full()
                    .px(surface::css(10.))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .text_color(MainPageColors.card_caption())
                    .child(i18n::t(description).to_uppercase()),
            )
        })
}

fn dashboard_group_toggle(
    id: &'static str,
    label: &str,
    collapsed: bool,
    cx: &App,
) -> gpui_kit::base::Button {
    // A Base Button avoids the styled Button's centered, nowrap content slot.
    // The complete title row remains keyboard-operable without moving its text.
    gpui_kit::base::Button::new((ElementId::from(id), "toggle"))
        .group(id)
        .h(surface::css(18.))
        .p_0()
        .justify_start()
        .gap(surface::css(10.))
        .text_size(surface::css(14.))
        .line_height(surface::css(18.))
        .text_color(cx.theme().foreground)
        .cursor_default()
        .accessibility_label(i18n::t(label))
        .aria_expanded(!collapsed)
        .hover(|view| view.text_color(MainPageColors.banner_heading()))
        .focus_visible(|view| view.bg(cx.theme().secondary_hover))
        .child(CollapseIcon::new(id, collapsed))
        .child(i18n::t(label))
}

fn dashboard_empty_devices(cx: &App) -> impl IntoElement {
    // 4130's `ee` keeps both useful links in the empty device card. This is a
    // local snapshot list, so its title must not claim a hardware scan result.
    v_flex()
        .w(surface::css(290.))
        .h(surface::css(220.))
        .flex_shrink_0()
        .pt(surface::css(10.))
        .px(surface::css(20.))
        .pb(surface::css(9.))
        .border_2()
        .border_dashed()
        .border_color(cx.theme().border)
        .rounded(surface::css(5.))
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_center()
        .child(
            div()
                .mt(surface::css(57.))
                .mb(surface::css(67.))
                .child("没有可显示的设备"),
        )
        .child(
            v_flex().items_center().gap(surface::css(10.)).children([
                (
                    "compatible-devices",
                    "VIEW_COMPATIBLE_DEVICES",
                    "https://mysupport.razer.com/app/answers/detail/a_id/6120/~/razer-synapse-4-supported-devices",
                ),
                ("razer-store", "VISIT_RAZER_STORE", "https://www.razer.com/store/"),
            ].map(|(id, label, url)| {
                gpui_kit::base::Link::new((ElementId::from("dashboard-empty"), id))
                    .href(url)
                    .open_with(|url, _, _, cx| cx.open_url(url))
                    .accessibility_label(i18n::t(label))
                    .text_color(cx.theme().foreground)
                    .underline()
                    .cursor_pointer()
                    .hover(|view| view.text_color(cx.theme().primary))
                    .focus_visible(|view| view.text_color(cx.theme().primary))
                    .child(i18n::t(label).to_uppercase())
            })),
        )
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
                let catalog = self.module_catalog.clone();
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
                        device.layout_id,
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
                            .child(if device.serial_number.starts_with("PREVIEW-") {
                                "预览设备"
                            } else {
                                "本地快照"
                            }),
                    )
                    .child(
                        super::service_pages::module_detail_action(
                            SharedString::from(format!("module-info-{key}")),
                            "详情",
                            cx,
                        )
                        .on_click(move |_, window, cx| {
                            catalog.update(cx, |catalog, cx| {
                                catalog.open_device_details(detail_device.clone(), window, cx)
                            });
                        }),
                    )
                    .child(
                        super::service_pages::module_action(
                            SharedString::from(format!("module-open-{key}")),
                            "打开",
                            false,
                            !supported,
                            cx,
                        )
                        .ml(surface::css(30.))
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.navigate(Location::Device(key.clone()), window, cx);
                            },
                        )),
                    )
            }))
            .child(div().mt(surface::css(20.)).child(surface::note(
                "尚未连接设备与模块服务，无法读取安装和固件更新状态。",
                cx,
            )))
            .child(self.module_catalog.clone())
            .into_any_element()
    }

    pub(super) fn gamer_room_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.gamer_room.clone().into_any_element()
    }

    pub(super) fn shortcuts_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.shortcuts.clone().into_any_element()
    }
}

fn dashboard_image(pid: u32, edition: u32, layout: u32, width: f32, height: f32) -> AnyElement {
    // PluginImages uses separate artwork. Missing downloads use a logo placeholder;
    // Customize's prd artwork must not be silently substituted for Dashboard.
    div()
        .w(surface::css(width))
        .h(surface::css(height))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            if let Some(asset) = crate::resources::dashboard_image(pid, edition, layout) {
                img(asset)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else {
                img("synapse/synapse.svg")
                    .size(surface::css(height.min(100.)))
                    .opacity(0.3)
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            },
        )
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
