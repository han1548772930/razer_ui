//! WN / Qd / rv: product support is a device page, reached from the Help icon.
use super::workspace::DeviceWorkspace;
use crate::{i18n, ui::surface};
use gpui_kit::base::{Button as BaseButton, Link};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct HelpState {
    copied_serial: bool,
    view_more: bool,
    copy_task: Option<Task<()>>,
}

/// DeviceInfo.masterGuideURL/supportPage in each original product bundle.
fn support_links(pid: u32, locale: &str) -> Option<(&'static str, String)> {
    let (support, guide) = match pid {
        182 => (
            "https://mysupport.razer.com/app/answers/detail/a_id/6125",
            "https://dl.razerzone.com/master-guides/RazerSynapse3/DEATHADDERV3PRO-00000182-",
        ),
        653 => (
            "https://mysupport.razer.com/app/answers/detail/a_id/9703/",
            "https://dl.razerzone.com/master-guides/RazerSynapse3/BLACKWIDOWV4PRO-00000653-",
        ),
        777 => (
            "https://mysupport.razer.com/app/answers/detail/a_id/3851",
            "https://dl.razerzone.com/master-guides/RazerSynapse3/KRAKENBTSANRIOLIMITEDEDITION-00000777-",
        ),
        _ => {
            let product = crate::product::audited_mouse_mat(pid)?;
            (product.support_url(), product.guide_prefix())
        }
    };
    let language = if locale.is_empty() {
        "en".to_string()
    } else {
        locale.to_ascii_lowercase()
    };
    Some((support, format!("{guide}{language}.pdf")))
}

fn help_link(id: &'static str, label: String, url: impl Into<SharedString>, cx: &App) -> Link {
    Link::new(id)
        .href(url)
        .accessibility_label(label.clone())
        .open_with(|href, _, _, cx| cx.open_url(href))
        .mt(surface::css(10.))
        .self_start()
        .flex()
        .items_center()
        .text_size(surface::css(14.))
        .text_color(cx.theme().foreground)
        .hover(|s| s.text_color(cx.theme().primary))
        .child(div().underline().child(label))
        .child(
            svg()
                .path("synapse/external-link.svg")
                .size(surface::css(20.))
                .ml(surface::css(5.)),
        )
}

fn help_button(id: &'static str, label: String, cx: &App) -> Button {
    Button::new(id)
        .label(label)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(cx.theme().button)
                .foreground(cx.theme().button_foreground)
                .hover(cx.theme().button)
                .active(cx.theme().button),
        )
        .h(surface::css(27.))
        .min_w(surface::css(100.))
        .border_1()
        .border_color(cx.theme().title_bar)
        .rounded(cx.theme().font_size * (3. / 16.))
        .px(surface::css(12.))
        .py_0()
        .text_size(surface::css(12.))
        .hover(|s| s.opacity(0.8))
}

impl DeviceWorkspace {
    fn copy_help_serial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.help.copied_serial || self.device().serial_number.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.device().serial_number.clone(),
        ));
        self.help.copied_serial = true;
        self.help.copy_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = view.update(cx, |view, cx| {
                view.help.copied_serial = false;
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn help_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some((support, guide)) = support_links(self.pid(), &i18n::locale()) else {
            return div().into_any_element();
        };
        let serial = self.device().serial_number.clone();
        let firmware = self.device().firmware_info.current_fw_version.clone();
        let support_panel = surface::panel(i18n::t("SUPPORT"), cx).gap_0().child(
            v_flex()
                .mt(surface::css(20.))
                .child(help_link(
                    "help-device-support",
                    i18n::t("VISIT_DEVICE_SUPPORT"),
                    support,
                    cx,
                ))
                .child(help_link(
                    "help-master-guide",
                    i18n::t("VISIT_MASTER_PAGE"),
                    guide,
                    cx,
                ))
                .child(help_link(
                    "help-synapse-support",
                    i18n::t("VISIT_SYNAPSE_SUPPORT"),
                    "https://support.razer.com",
                    cx,
                )),
        );
        let reset_panel = surface::panel(i18n::t("FACTORY_RESET"), cx).gap_0().child(
            v_flex()
                .mt(surface::css(20.))
                .items_start()
                .child(i18n::t("FACTORY_RESET_PROFILES"))
                .child(
                    help_button("help-factory-reset", i18n::t("RESET"), cx)
                        .mt(surface::css(20.))
                        .disabled(true)
                        .tooltip("设备服务未连接，无法恢复设备出厂设置"),
                ),
        );
        let serial_panel = surface::panel(i18n::t("SERIAL_NUM"), cx).gap_0().child(
            v_flex()
                .mt(surface::css(20.))
                .items_start()
                .child(
                    div()
                        .id("help-serial")
                        .test_support()
                        .aria_label(serial.clone())
                        .child(format!("{} {serial}", i18n::t("SERIAL"))),
                )
                .child(
                    help_button(
                        "help-copy-serial",
                        i18n::t(if self.help.copied_serial {
                            "COPIED_SERIAL"
                        } else {
                            "COPY_SERIAL"
                        }),
                        cx,
                    )
                    .mt(surface::css(20.))
                    .disabled(serial.is_empty() || self.help.copied_serial)
                    .on_click(cx.listener(|this, _, window, cx| this.copy_help_serial(window, cx))),
                ),
        );
        let view_more_label = i18n::t(if self.help.view_more {
            "VIEW_LESS"
        } else {
            "VIEW_MORE"
        });
        let firmware_panel = surface::panel(i18n::t("DEVICE_HEADER"), cx).gap_0().child(
            v_flex()
                .mt(surface::css(20.))
                .items_start()
                .child(format!("{}: {firmware}", i18n::t("FIRMWARE_VERSION")))
                // Original runtime UI/MW/Synapse versions are absent in the
                // local snapshot. Never substitute this application's version.
                .child(
                    BaseButton::new("help-more-versions")
                        .accessibility_label(view_more_label.clone())
                        .h(surface::css(50.))
                        .px_0()
                        .py(surface::css(15.))
                        .justify_start()
                        .gap(surface::css(4.))
                        .text_size(surface::css(14.))
                        .hover(|s| s.text_color(cx.theme().primary))
                        .child(div().underline().child(view_more_label))
                        .child(
                            svg()
                                .path(if self.help.view_more {
                                    "synapse/help-less.svg"
                                } else {
                                    "synapse/help-more.svg"
                                })
                                .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.help.view_more = !this.help.view_more;
                            cx.notify();
                        })),
                ),
        );
        let registration_panel = surface::panel(i18n::t("PRODUCT_REGISTRATION"), cx)
            .gap_0()
            .child(div().mt(surface::css(20.)).child(help_link(
                "help-register-product",
                i18n::t("REGISTER_ONLINE"),
                "https://www.razer.com/product-registration",
                cx,
            )));
        let left = v_flex()
            .gap(surface::css(20.))
            .child(support_panel)
            .child(reset_panel);
        let right = v_flex()
            .gap(surface::css(20.))
            .child(serial_panel)
            .when(!firmware.is_empty(), |this| this.child(firmware_panel))
            .child(registration_panel);
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }
}
