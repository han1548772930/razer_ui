//! WN / Qd / rv: product support is a device page, reached from the Help icon.
use super::workspace::DeviceWorkspace;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_widgets::surface;
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

fn help_button(id: &'static str, label: String, disabled: bool, cx: &App) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .child(label)
        .disabled(disabled)
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.theme().button)
        .text_color(cx.theme().button_foreground)
        .styles(|s| s.disabled(|s| s.opacity(0.3)))
        .h(surface::css(27.))
        .min_w(surface::css(100.))
        .border_1()
        .border_color(cx.theme().title_bar)
        .rounded(cx.theme().font_size * (3. / 16.))
        .px(surface::css(12.))
        .py_0()
        .text_size(surface::css(12.))
        .when(!disabled, |s| s.hover(|s| s.opacity(0.8)))
        .focus_visible(|s| s.border_color(cx.theme().primary))
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
        if !matches!(self.pid(), 182 | 653 | 777)
            && razer_catalog::audited_mouse_mat(self.pid()).is_none()
        {
            return div().into_any_element();
        }
        let serial = self.device().serial_number.clone();
        let firmware = self.device().current_firmware_version().to_owned();
        let reset_panel = surface::panel(i18n::t("FACTORY_RESET"), cx).gap_0().child(
            v_flex()
                .mt(surface::css(20.))
                .items_start()
                .child(i18n::t("FACTORY_RESET_PROFILES"))
                .child(
                    help_button("help-factory-reset", i18n::t("RESET"), true, cx)
                        .mt(surface::css(20.))
                        .tooltip(|window, cx| {
                            tooltip::Tooltip::new("设备服务未连接，无法恢复设备出厂设置")
                                .build(window, cx)
                        }),
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
                        serial.is_empty() || self.help.copied_serial,
                        cx,
                    )
                    .mt(surface::css(20.))
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
                            razer_assets::resource_svg()
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
        let left = v_flex().gap(surface::css(20.)).child(reset_panel);
        let right = v_flex()
            .gap(surface::css(20.))
            .child(serial_panel)
            .when(!firmware.is_empty(), |this| this.child(firmware_panel));
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }
}
