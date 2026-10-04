//! Current Armory 29770 root / 20540 navigation / 77989 feature hook.
//! See docs/re/armory-default-source.json for scoped source and CSS receipts.
use crate::{
    i18n,
    ui::{scroll::SourceScrollable as _, surface},
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::*;
use gpui_kit::*;

fn tr(key: &str) -> String {
    i18n::t(&format!("ARMORY_SOURCE.{key}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArmoryTab {
    Spotlight,
    Browse,
    MyDownloads,
    MyUploads,
}
impl ArmoryTab {
    const ALL: [Self; 4] = [
        Self::Spotlight,
        Self::Browse,
        Self::MyDownloads,
        Self::MyUploads,
    ];
    fn key(self) -> &'static str {
        match self {
            Self::Spotlight => "SPOTLIGHT_HEADER",
            Self::Browse => "BROWSE_HEADER",
            Self::MyDownloads => "MY_DOWNLOADS_HEADER",
            Self::MyUploads => "MY_UPLOADS_HEADER",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Spotlight => "armory-spotlight",
            Self::Browse => "armory-browse",
            Self::MyDownloads => "armory-downloads",
            Self::MyUploads => "armory-uploads",
        }
    }
}

pub(super) struct ArmoryPage {
    tab: ArmoryTab,
    focus: FocusHandle,
    history: Vec<ArmoryTab>,
    history_index: usize,
    phase1: bool,
    guest: bool,
    banner_open: bool,
}
impl ArmoryPage {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            // 29770 settles on Browse after 77989 finishes without feature data.
            tab: ArmoryTab::Browse,
            history: vec![ArmoryTab::Browse],
            history_index: 0,
            phase1: false,
            guest: true,
            banner_open: true,
            focus: cx.focus_handle(),
        }
    }
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn visible(&self, tab: ArmoryTab) -> bool {
        (!self.guest || tab != ArmoryTab::MyUploads)
            && (self.phase1 || !matches!(tab, ArmoryTab::MyUploads | ArmoryTab::Spotlight))
    }
    pub(super) fn has_previous_page(&self) -> bool {
        self.history_index > 0
    }
    pub(super) fn has_next_page(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }
    pub(super) fn step_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if forward && self.has_next_page() {
            self.history_index += 1;
        } else if !forward && self.has_previous_page() {
            self.history_index -= 1;
        } else {
            return;
        }
        self.tab = self.history[self.history_index];
        self.focus(window, cx);
    }
    fn set_tab(&mut self, tab: ArmoryTab, cx: &mut Context<Self>) {
        if self.tab == tab || !self.visible(tab) {
            return;
        }
        self.tab = tab;
        self.history.truncate(self.history_index + 1);
        self.history.push(tab);
        self.history_index = self.history.len() - 1;
        cx.notify();
    }
    fn banner(&self, cx: &mut Context<Self>) -> AnyElement {
        // 458 CSS: image cover/center, 20px 30px 0 margin, min-width 1220.
        div()
            .relative()
            .mt(surface::css(20.))
            .mx(surface::css(30.))
            .min_w(surface::css(1220.))
            .max_h(surface::css(150.))
            .rounded(surface::css(5.))
            .overflow_hidden()
            .child(
                img("synapse/armory-introduction.png")
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                v_flex()
                    .relative()
                    .items_center()
                    .justify_center()
                    .p(surface::css(20.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(24.))
                            .line_height(surface::css(30.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0x44d62c))
                            .text_center()
                            .mb(surface::css(15.))
                            // All source capabilities false -> isExchangeEnabled true.
                            .child(tr("EXCHANGE_GET_STARTED").to_uppercase()),
                    )
                    .children(
                        ["AI_MACRO_DESC", "MACRO_USAGE_DESC", "ASSIGN_MACRO_DESC"].map(|key| {
                            div()
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .child(tr(key))
                        }),
                    ),
            )
            .child(
                BaseButton::new("armory-banner-close")
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(10.))
                    .p_0()
                    .size(surface::css(24.))
                    .accessibility_label(tr("CLOSE"))
                    .child(img("synapse/armory-banner-close.svg").size_full())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.banner_open = false;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
impl Render for ArmoryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().w_full();
        if self.tab == ArmoryTab::Browse && self.banner_open {
            body = body.child(self.banner(cx));
        }
        // 86024 returns null for an empty service dataset. The former paragraphs
        // containing source keys and implementation notes were not product UI.
        v_flex()
            .id("armory-window")
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .track_focus(&self.focus)
            .child(
                h_flex()
                    .w_full()
                    .h(surface::css(48.))
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(rgb(0))
                    .child(surface::nav_left())
                    .child(
                        h_flex()
                            .flex_grow(1.)
                            .flex_shrink_0()
                            .justify_center()
                            .gap(surface::css(20.))
                            .children(
                                ArmoryTab::ALL
                                    .into_iter()
                                    .filter(|tab| self.visible(*tab))
                                    .map(|tab| {
                                        surface::navigation_button(
                                            tab.id(),
                                            tr(tab.key()),
                                            self.tab == tab,
                                            cx,
                                        )
                                        .role(Role::Tab)
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.set_tab(tab, cx)
                                            }),
                                        )
                                    }),
                            ),
                    )
                    .child(surface::nav_right()),
            )
            .child(
                div()
                    .id("armory-body")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(body),
            )
    }
}
