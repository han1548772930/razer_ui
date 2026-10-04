//! Audited root props and their consumers, rather than treating
//! `isEnableProfileBar` as visibility. See the current accessory audits.
use super::*;

impl SourceProductWorkspace {
    pub(super) fn profile_bar_visible(&self) -> bool {
        match self.device.product_id {
            179 | 769 => false,
            164 | 241 | 778 | 784 | 3871 | 3884 | 3886 | 3946 => true,
            _ => !self
                .current_page()
                .is_some_and(|p| p.role() == ProductPageRole::Help),
        }
    }

    fn profile_switch_enabled(&self) -> bool {
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
            778 | 3871 | 3884 | 3886 => key == "TAB_LIGHTING",
            784 => !matches!(key, "CUSTOMIZED" | "HELP"),
            3946 => !matches!(key, "TAB_CALIBRATION" | "HELP"),
            _ => key != "HELP",
        }
    }

    pub(super) fn profile_bar(&self, cx: &App) -> AnyElement {
        if !self.profile_bar_visible() {
            return div().into_any_element();
        }
        h_flex()
            .id("source-profile-bar")
            .h(surface::css(26.))
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
            .child(
                div().w(surface::css(250.)).px(surface::css(10.)).child(
                    surface::select(&self.profile)
                        .id("source-profile-select")
                        .accessibility_label(crate::i18n::t("PROFILE"))
                        .items(
                            self.device
                                .profiles
                                .iter()
                                .map(|p| Choice::new(&p.id, &p.name))
                                .collect(),
                        )
                        .disabled(!self.profile_switch_enabled())
                        .opacity(1.)
                        .w_full(),
                ),
            )
            .text_color(cx.theme().foreground)
            .into_any_element()
    }
}
