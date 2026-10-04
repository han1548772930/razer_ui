//! Independent application roots for the Dashboard module windows.
//!
//! The current Dashboard opens Macro, Armory and Profiles by named host
//! windows. These roots deliberately contain the already audited local pages;
//! they do not add installer or service state. The window registry and
//! same-name focus behavior live in `display_window`.
use super::{
    alexa_page::AlexaPage, armory_page::ArmoryPage, feedback_page::FeedbackPage,
    macro_page::MacroPage, profiles_page::ProfilesPage,
};
use crate::features::ProductWorkspace;
use crate::ui::surface;
use gpui_kit::component::{
    button::{ButtonCustomVariant, ButtonRounded, ButtonVariants},
    *,
};
use gpui_kit::*;

/// Alexa's current Dashboard module is a named `alexa` window.  The page
/// entity is the same audited local frontend used by the host preview, while
/// this root gives module clicks the source window identity and reuse policy.
pub(super) struct AlexaWindow {
    page: Entity<AlexaPage>,
    _page_subscription: Subscription,
}

impl AlexaWindow {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let page = cx.new(|cx| AlexaPage::new(window, cx));
        page.update(cx, |page, cx| page.focus(window, cx));
        let page_subscription = cx.observe(&page, |_, _, cx| cx.notify());
        Self {
            page,
            _page_subscription: page_subscription,
        }
    }
}

impl Render for AlexaWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page.read(cx);
        let has_previous = page.has_previous_page();
        let has_next = page.has_next_page();
        let history_blocked = page.history_blocked();
        // Alexa's root renders `.toolbar` before its 48px `.nav-tabs`.  The
        // page does not own this toolbar because host previews supply it.
        v_flex()
            .id("alexa-window")
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .id("alexa-window-toolbar")
                    .w_full()
                    .h(surface::css(38.))
                    .flex_shrink_0()
                    .child(
                        h_flex()
                            .flex_grow(0.)
                            .flex_shrink(0.)
                            .flex_basis(relative(0.2))
                            .h_full()
                            .child(
                                surface::history_button(
                                    "alexa-window-back",
                                    false,
                                    has_previous,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.page.update(cx, |page, cx| page.go_back(window, cx));
                                    },
                                )),
                            )
                            .child(
                                surface::history_button("alexa-window-forward", true, has_next, cx)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.page
                                            .update(cx, |page, cx| page.go_forward(window, cx));
                                    })),
                            )
                            .child(
                                surface::asset_button(
                                    "alexa-window-refresh",
                                    "synapse/alexa-refresh.svg",
                                    crate::i18n::t("REFRESH"),
                                    cx,
                                )
                                .w(surface::css(40.))
                                .h_full()
                                .disabled(history_blocked)
                                .rounded(ButtonRounded::None)
                                .custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().transparent)
                                        .hover(cx.theme().secondary_hover)
                                        .active(cx.theme().secondary_hover),
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.page.update(cx, |page, cx| page.refresh(window, cx));
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_grow(3.)
                            .flex_shrink(1.)
                            .flex_basis(surface::css(340.))
                            .min_w_0()
                            .h_full()
                            .items_center()
                            .justify_center()
                            .child(
                                img("synapse/alexa-header.svg")
                                    .h(surface::css(16.))
                                    .w(surface::css(205.382 * 16. / 30.))
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        div()
                            .flex_grow(0.)
                            .flex_shrink(0.)
                            .flex_basis(relative(0.2)),
                    ),
            )
            .child(div().flex_1().min_h_0().child(self.page.clone()))
    }
}

pub(super) struct MacroWindow {
    page: Entity<MacroPage>,
}

impl MacroWindow {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            page: cx.new(|cx| MacroPage::new(window, cx)),
        }
    }
}

impl Render for MacroWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.page.clone()
    }
}

pub(super) struct ArmoryWindow {
    page: Entity<ArmoryPage>,
}

impl ArmoryWindow {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            page: cx.new(|cx| ArmoryPage::new(window, cx)),
        }
    }
}

impl Render for ArmoryWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.page.clone()
    }
}

pub(super) struct ProfilesWindow {
    page: Entity<ProfilesPage>,
}

impl ProfilesWindow {
    pub(super) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        devices: Vec<Entity<ProductWorkspace>>,
    ) -> Self {
        let page = cx.new(|cx| ProfilesPage::new(window, cx));
        page.update(cx, |page, cx| page.set_devices(devices, cx));
        Self { page }
    }
}

impl Render for ProfilesWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.page.clone()
    }
}

pub(super) struct FeedbackWindow {
    page: Entity<FeedbackPage>,
}

impl FeedbackWindow {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            page: cx.new(|cx| FeedbackPage::new(window, cx)),
        }
    }
}

impl Render for FeedbackWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.page.clone()
    }
}
