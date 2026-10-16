//! Actual mounted FirmwareFactoryReset component: button/spinner and popup.
use super::*;

impl SourceHelp {
    pub(super) fn firmware_reset_panel(&self, cx: &Context<Self>) -> AnyElement {
        let Some((_, page)) = self.metadata() else {
            return div().into_any_element();
        };
        help_panel("FACTORY_RESET", true, cx)
            .id("source-help-firmware-reset-panel")
            .relative()
            .child(i18n::t(&page.reset_title))
            .child(
                h_flex()
                    .items_end()
                    .child(
                        help_button(
                            "source-help-firmware-reset",
                            i18n::t("RESET"),
                            self.reset_cooldown,
                            cx,
                        )
                        .mt(surface::css(20.))
                        .w(surface::css(100.))
                        .px_0()
                        .line_height(surface::css(14.))
                        .on_click(cx.listener(|this, _, window, cx| this.show_reset(window, cx))),
                    )
                    .when(self.reset_cooldown, |row| row.child(Spinner::new().small())),
            )
            .when(self.inline_reset, |panel| panel.child(self.reset_popup(cx)))
            .into_any_element()
    }
}
