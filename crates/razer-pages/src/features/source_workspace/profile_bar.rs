//! Audited root props and their consumers, rather than treating
//! `isEnableProfileBar` as visibility. See the current accessory audits.
use super::*;

impl SourceProductWorkspace {
    pub(super) fn profile_bar_visible(&self) -> bool {
        match self.device.product_id {
            179 | 769 => false,
            // 190 Bm and 679 JU always mount the bar. Their
            // displayProfileBar prop controls the sync icon, not visibility.
            164 | 190 | 241 | 679 | 691 | 778 | 784 | 3871 | 3884 | 3886 | 3946 => true,
            _ => !self
                .current_page()
                .is_some_and(|p| p.role() == ProductPageRole::Help),
        }
    }

    pub(super) fn profile_switch_enabled(&self) -> bool {
        let key = self.current_page().map(|p| p.kind().key()).unwrap_or("");
        match self.device.product_id {
            // These roots explicitly dispatch setProfileDropdownState.
            784 => key != "CUSTOMIZED",
            3884 => key == "TAB_LIGHTING",
            _ => true,
        }
    }

    fn profile_sync_enabled(&self) -> bool {
        let key = self.current_page().map(|p| p.kind().key()).unwrap_or("");
        match self.device.product_id {
            // 190 vm = [TAB_POWER, TAB_CALIBRATION, HELP].
            190 => !matches!(key, "TAB_POWER" | "TAB_CALIBRATION" | "HELP"),
            // 691 Ca -> Kt: only the sync glyph changes on these pages.
            691 => !matches!(key, "OLED" | "TAB_POWER" | "HELP"),
            778 | 3871 | 3884 | 3886 => key == "TAB_LIGHTING",
            784 => !matches!(key, "CUSTOMIZED" | "HELP"),
            3946 => !matches!(key, "TAB_CALIBRATION" | "HELP"),
            _ => key != "HELP",
        }
    }

    pub(super) fn profile_bar_width(&self) -> f32 {
        if !self.profile_bar_visible() {
            0.
        } else if profile_menu::spec(self.device.product_id).is_some() {
            surface::PROFILE_BAR_WIDTH
        } else {
            286.
        }
    }

    pub(super) fn profile_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if !self.profile_bar_visible() {
            return div().into_any_element();
        }
        h_flex()
            .id("source-profile-bar")
            .h(surface::css(26.))
            .w(surface::css(self.profile_bar_width()))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .when(!self.profile_switch_enabled(), |v| v.opacity(0.3))
            .child(
                div()
                    .size(surface::css(26.))
                    .ml(surface::css(10.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img(if self.profile_sync_enabled() {
                            "synapse/profile.svg"
                        } else {
                            "synapse/profile-unsupported.svg"
                        })
                        .size(surface::css(20.)),
                    ),
            )
            .child(div().w(surface::css(250.)).px(surface::css(10.)).child(
                if self.profile_rename.is_some() {
                    div()
                        .id("source-profile-name-editor")
                        .w_full()
                        .on_action(cx.listener(
                            |this, _: &gpui_kit::component::input::Escape, window, cx| {
                                this.profile_rename = None;
                                this.profile.update(cx, |state, cx| state.focus(window, cx));
                                cx.notify();
                            },
                        ))
                        .child(
                            gpui_kit::component::input::Input::new(&self.profile_name)
                                .id("source-profile-name")
                                .aria_label(razer_i18n::t("PROFILE"))
                                .h(surface::css(27.))
                                .w_full()
                                .rounded_none()
                                .focus_bordered(false)
                                .border_1()
                                .border_color(cx.theme().primary)
                                .bg(cx.theme().group_box)
                                .px(surface::css(5.))
                                .py(surface::css(5.))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.)),
                        )
                        .into_any_element()
                } else {
                    surface::select(&self.profile)
                        .id("source-profile-select")
                        .accessibility_label(razer_i18n::t("PROFILE"))
                        .items(
                            self.device
                                .profiles
                                .iter()
                                .map(|p| Choice::new(&p.id, &p.name))
                                .collect(),
                        )
                        .disabled(!self.profile_switch_enabled())
                        .opacity(1.)
                        .w_full()
                        .into_any_element()
                },
            ))
            .child(self.profile_more(window, cx))
            .text_color(cx.theme().foreground)
            .into_any_element()
    }
}
