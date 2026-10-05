//! Current HomePage route trees: 22534 / 44442 / 19388 / 94608.
//! Service-dependent groups remain absent until their real data is available.
use super::{AppShell, Location};
use crate::{
    i18n,
    ui::{surface, theme::MainPageColors},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};

mod dashboard_cards;
mod dashboard_device;
mod dashboard_grid;
mod dashboard_group;
mod dashboard_tutorial;
use dashboard_grid::{DashboardCard, DashboardGrid};
pub(super) use dashboard_grid::{DashboardChanged, DashboardState};
use dashboard_group::{CollapseIcon, DashboardGroupContent};
pub(super) use dashboard_tutorial::{DashboardTutorial, DashboardTutorialEvent};

/// 55 CSS's 1279/600 breakpoints and current 22534/Pi.getMaxColumns.
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
    // Current 22534 ee keeps both useful links in the empty device card.
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
                .child(i18n::t("NO_DEVICE_FOUND").to_uppercase()),
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
        // Pi.render adds .reflow when more than four devices are present. Its
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
        // Keep the actual CSS container width independent of whole card slots.
        // Pi's inline width:unset overrides the <=1260 width:min-content rule.
        let minimum = if width <= 600. { 290. } else { 620. };
        let available = (width - gutter * 2.).min(maximum).max(minimum);
        Self {
            gutter,
            body_max_width: if reflow { 2460. + gutter * 2. } else { 1260. },
            dashboard_width: available,
            narrow,
        }
    }
}

/// Exact current Pi.getMaxColumns arithmetic, including its extra-column
/// condition. The source reads the actual .dashboard rectangle, not card width.
fn dashboard_columns(width: f32) -> usize {
    let measured = width.ceil() - 20.;
    let mut count = (measured / 310.).floor();
    if measured - 290. * count >= 310. {
        count += 1.;
    }
    count.max(1.) as usize
}

impl AppShell {
    pub(super) fn modules_page(&self, _: &mut Context<Self>) -> AnyElement {
        self.module_catalog.clone().into_any_element()
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
        assert_eq!(compact.dashboard_width, 620.);
        assert_eq!((below.gutter, below.dashboard_width), (30., 910.));
        assert_eq!((above.gutter, above.dashboard_width), (20., 1220.));
        assert_eq!(MainLayout::new(2560., 16., 4).dashboard_width, 1220.);
        assert_eq!(MainLayout::new(2560., 16., 5).dashboard_width, 2460.);
        assert_eq!(MainLayout::new(1920., 16., 5).dashboard_width, 1880.);
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
