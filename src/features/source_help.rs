//! Retained Help UI from each product's mounted current source component.
use crate::{i18n, model::Device, ui::surface};
use gpui_kit::base::{Button as BaseButton, Link};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use std::{sync::OnceLock, time::Duration};

#[derive(Deserialize)]
struct Help {
    product_id: u32,
    support: Option<String>,
    guide: Option<String>,
    #[serde(default)]
    pages: Vec<HelpPage>,
}
#[derive(Deserialize)]
struct HelpPage {
    offset: usize,
    firmware: bool,
    view_more: bool,
    reset: bool,
    oled_reset: bool,
    reset_title: String,
    obm: bool,
    firmware_reset: Option<serde_json::Value>,
    system_info: bool,
    tutorial: bool,
    thx_instructions: bool,
    camo: bool,
}
fn records() -> &'static [Help] {
    static RECORDS: OnceLock<Vec<Help>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(include_str!("source_help_data.json"))
            .expect("validated product Help metadata")
    })
}

pub(super) struct SourceHelp {
    device: Device,
    page_offset: Option<usize>,
    copied_serial: bool,
    view_more: bool,
    copy_task: Option<Task<()>>,
}
impl SourceHelp {
    pub(super) fn new(device: Device, _: &mut Context<Self>) -> Self {
        Self {
            device,
            page_offset: None,
            copied_serial: false,
            view_more: false,
            copy_task: None,
        }
    }
    pub(super) fn set_device(&mut self, device: &Device, cx: &mut Context<Self>) {
        if self.device.product_id != device.product_id
            || self.device.serial_number != device.serial_number
        {
            self.copy_task = None;
            self.copied_serial = false;
            self.view_more = false;
        }
        if self.device.product_id != device.product_id {
            self.page_offset = None;
        }
        self.device = device.clone();
        cx.notify();
    }
    pub(super) fn set_page(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.page_offset != Some(offset) {
            self.page_offset = Some(offset);
            self.view_more = false;
            cx.notify();
        }
    }
    fn metadata(&self) -> Option<(&'static Help, &'static HelpPage)> {
        let help = records()
            .iter()
            .find(|h| h.product_id == self.device.product_id)?;
        let page = if let Some(offset) = self.page_offset {
            help.pages.iter().find(|p| p.offset == offset)?
        } else {
            help.pages.first()?
        };
        Some((help, page))
    }
    fn copy_serial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.copied_serial || self.device.serial_number.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(self.device.serial_number.clone()));
        self.copied_serial = true;
        self.copy_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = view.update(cx, |view, cx| {
                view.copied_serial = false;
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn show_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, page)) = self.metadata() else {
            return;
        };
        if !page.reset && !(page.oled_reset && !self.device.use_ble) {
            return;
        }
        let title = self.device.display_name();
        let message = if page.oled_reset && !self.device.use_ble {
            i18n::t(&page.reset_title)
        } else if let Some(reset) = &page.firmware_reset {
            i18n::t(
                reset
                    .get("modalMsg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("FACTORY_RESET_MSG"),
            )
        } else {
            i18n::t(if page.obm {
                "FACTORY_RESET_MSG"
            } else {
                "FACTORY_RESET_MSG_NO_OBM_DEVICE"
            })
        };
        window.open_dialog(cx, move |dialog, _, cx| {
            dialog
                .title(title.clone())
                .child(message.clone())
                .child(surface::note("设备服务未连接，无法恢复设备出厂设置。", cx))
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_3()
                        .child(
                            Button::new("source-help-reset-cancel")
                                .label(i18n::t("CANCEL"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("source-help-reset-confirm")
                                .label(i18n::t("RESET"))
                                .disabled(true),
                        ),
                )
        });
    }
}

fn help_link(id: &'static str, key: &str, url: impl Into<SharedString>, cx: &App) -> Link {
    let label = i18n::t(key);
    Link::new(id)
        .href(url)
        .accessibility_label(label.clone())
        .open_with(|url, _, _, cx| cx.open_url(url))
        .self_start()
        .flex()
        .items_center()
        .gap(surface::css(5.))
        .text_size(surface::css(14.))
        .text_color(cx.theme().foreground)
        .hover(|style| style.text_color(cx.theme().primary))
        .child(div().underline().child(label))
        .child(
            svg()
                .path("synapse/external-link.svg")
                .size(surface::css(20.)),
        )
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
        .styles(|style| style.disabled(|style| style.opacity(0.3)))
        .h(surface::css(27.))
        .min_w(surface::css(100.))
        .border_1()
        .border_color(cx.theme().title_bar)
        .rounded(surface::css(3.))
        .px(surface::css(12.))
        .py_0()
        .text_size(surface::css(12.))
        .when(!disabled, |button| button.hover(|style| style.opacity(0.8)))
        .focus_visible(|style| style.border_color(cx.theme().primary))
}
fn unavailable_panel(
    title: &str,
    description: &str,
    action: &str,
    id: &'static str,
    cx: &App,
) -> Div {
    surface::panel(i18n::t(title), cx)
        .child(i18n::t(description))
        .child(
            help_button(id, i18n::t(action), true, cx)
                .tooltip(|window, cx| tooltip::Tooltip::new("设备服务未连接").build(window, cx)),
        )
}
impl Render for SourceHelp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((help, page)) = self.metadata() else {
            return surface::note("此产品的帮助页证据尚未解析。", cx).into_any_element();
        };
        let mut support = surface::panel(i18n::t("SUPPORT"), cx).gap(surface::css(10.));
        if let Some(url) = &help.support {
            support = support.child(help_link(
                "source-help-device",
                "VISIT_DEVICE_SUPPORT",
                url.clone(),
                cx,
            ));
        }
        if let Some(prefix) = &help.guide {
            // App locale tags use zh-CN; current vendor language assets and
            // guide filenames use lower-case tags. Empty locale defaults to en.
            let locale = i18n::locale().to_ascii_lowercase();
            let locale = if locale.is_empty() { "en" } else { &locale };
            support = support.child(help_link(
                "source-help-guide",
                "VISIT_MASTER_PAGE",
                format!("{prefix}{locale}.pdf"),
                cx,
            ));
        }
        support = support.child(help_link(
            "source-help-synapse",
            "VISIT_SYNAPSE_SUPPORT",
            "https://support.razer.com",
            cx,
        ));
        let mut left = v_flex().gap(surface::css(20.));
        if page.system_info {
            left = left.child(
                surface::panel(i18n::t("CUSTOMIZE_SYSTEM_INFO_TITLE"), cx)
                    .child(surface::note("尚未获取此设备的系统信息。", cx)),
            );
        }
        if page.tutorial {
            left = left.child(unavailable_panel(
                "TUTORIAL",
                "TUTORIAL_DESC",
                "VIEW_AGAIN",
                "source-help-tutorial",
                cx,
            ));
        }
        if page.thx_instructions {
            left = left.child(unavailable_panel(
                "THX_SPATIAL_AUDIO",
                "THX_SPATIAL_AUDIO_NOTICE",
                "SHOW_ME_HOW",
                "source-help-thx",
                cx,
            ));
        }
        if page.camo {
            left = left.child(unavailable_panel(
                "CAMO_STUDIO_PRO_LICENSE_CODE",
                "CAMO_STUDIO_PRO_LICENSE_CODE_DESCRIPTION",
                "INSTALL_CAMO_STUDIO",
                "source-help-camo",
                cx,
            ));
        }
        left = left.child(support);
        if page.reset || page.oled_reset && !self.device.use_ble {
            left = left.child(
                surface::panel(i18n::t("FACTORY_RESET"), cx)
                    .child(i18n::t(&page.reset_title))
                    .child(
                        help_button("source-help-reset", i18n::t("RESET"), false, cx)
                            .self_start()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_reset(window, cx)),
                            ),
                    ),
            );
        }
        let serial = self.device.serial_number.clone();
        let mut right = v_flex().gap(surface::css(20.)).child(
            surface::panel(i18n::t("SERIAL_NUM"), cx)
                .child(format!("{} {serial}", i18n::t("SERIAL")))
                .child(
                    help_button(
                        "source-help-copy",
                        i18n::t(if self.copied_serial {
                            "COPIED_SERIAL"
                        } else {
                            "COPY_SERIAL"
                        }),
                        serial.is_empty() || self.copied_serial,
                        cx,
                    )
                    .self_start()
                    .on_click(cx.listener(|this, _, window, cx| this.copy_serial(window, cx))),
                ),
        );
        let firmware = &self.device.firmware_info.current_fw_version;
        if page.firmware && !firmware.is_empty() {
            let mut panel = surface::panel(i18n::t("DEVICE_HEADER"), cx)
                .child(format!("{}: {firmware}", i18n::t("FIRMWARE_VERSION")));
            if page.view_more {
                let label = i18n::t(if self.view_more {
                    "VIEW_LESS"
                } else {
                    "VIEW_MORE"
                });
                // UI/MW/Synapse runtime versions are absent from Device. Source
                // suppresses empty fields even in the expanded state.
                panel = panel.child(
                    BaseButton::new("source-help-versions")
                        .accessibility_label(label.clone())
                        .self_start()
                        .flex()
                        .items_center()
                        .gap(surface::css(4.))
                        .px_0()
                        .py(surface::css(15.))
                        .hover(|style| style.text_color(cx.theme().primary))
                        .child(div().underline().child(label))
                        .child(
                            svg()
                                .path(if self.view_more {
                                    "synapse/help-less.svg"
                                } else {
                                    "synapse/help-more.svg"
                                })
                                .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.view_more = !this.view_more;
                            cx.notify();
                        })),
                );
            }
            right = right.child(panel);
        }
        right = right.child(
            surface::panel(i18n::t("PRODUCT_REGISTRATION"), cx).child(help_link(
                "source-help-register",
                "REGISTER_ONLINE",
                "https://www.razer.com/product-registration",
                cx,
            )),
        );
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }
}
