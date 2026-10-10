//! Retained Help UI from each product's mounted current source component.
use gpui_kit::base::{Button as BaseButton, Link};
use gpui_kit::component::button::Button;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_widgets::surface;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
    time::Duration,
};

#[derive(Deserialize)]
struct Help {
    product_id: u32,
    support: Option<String>,
    guide: Option<String>,
    pages: Vec<HelpPage>,
}
#[derive(Deserialize)]
struct HelpPage {
    offset: usize,
    serial: bool,
    registration: bool,
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
    confirmation_generation: u64,
    reset_generation: u64,
    pending_reset: bool,
    reset_cooldown: bool,
    reset_cooldown_task: Option<Task<()>>,
    reset_error: Option<String>,
    audio_restart: bool,
    audio_reset_generations: BTreeSet<u64>,
    audio_reset_tasks: BTreeMap<u64, Task<()>>,
}
#[derive(Clone, Copy, Debug)]
pub struct HelpResetRequest {
    generation: u64,
    audio_streams: bool,
}
/// Actual query identity accompanies the raw source document separately.
/// Transport fields must never be injected into the original storage document.
pub struct HelpResetOutcome {
    pub source_document: serde_json::Value,
    pub serial_number: String,
}
impl HelpResetRequest {
    pub fn generation(self) -> u64 {
        self.generation
    }
    pub fn is_audio_streams(self) -> bool {
        self.audio_streams
    }
}
pub(super) enum HelpResetEvent {
    Requested(HelpResetRequest),
    Canceled(HelpResetRequest),
}
impl EventEmitter<HelpResetEvent> for SourceHelp {}
impl SourceHelp {
    pub(super) fn new(device: Device, _: &mut Context<Self>) -> Self {
        Self {
            device,
            page_offset: None,
            copied_serial: false,
            view_more: false,
            copy_task: None,
            confirmation_generation: 0,
            reset_generation: 0,
            pending_reset: false,
            reset_cooldown: false,
            reset_cooldown_task: None,
            reset_error: None,
            audio_restart: false,
            audio_reset_generations: BTreeSet::new(),
            audio_reset_tasks: BTreeMap::new(),
        }
    }
    pub(super) fn set_device(&mut self, device: &Device, cx: &mut Context<Self>) {
        if self.device.product_id != device.product_id
            || self.device.serial_number != device.serial_number
            || self.device.device_container_id != device.device_container_id
        {
            if self.pending_reset {
                cx.emit(HelpResetEvent::Canceled(HelpResetRequest {
                    generation: self.reset_generation,
                    audio_streams: false,
                }));
            }
            self.copy_task = None;
            self.copied_serial = false;
            self.view_more = false;
            self.reset_generation = self.reset_generation.wrapping_add(1);
            self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
            self.pending_reset = false;
            self.reset_cooldown = false;
            self.reset_cooldown_task = None;
            self.reset_error = None;
            self.cancel_audio_resets(cx);
        }
        if self.device.product_id != device.product_id {
            self.page_offset = None;
        }
        self.device = device.clone();
        cx.notify();
    }
    pub(super) fn set_page(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.page_offset != Some(offset) {
            if self.pending_reset {
                cx.emit(HelpResetEvent::Canceled(HelpResetRequest {
                    generation: self.reset_generation,
                    audio_streams: false,
                }));
            }
            self.page_offset = Some(offset);
            self.view_more = false;
            self.reset_generation = self.reset_generation.wrapping_add(1);
            self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
            self.pending_reset = false;
            self.reset_cooldown = false;
            self.reset_cooldown_task = None;
            self.reset_error = None;
            self.cancel_audio_resets(cx);
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
        if self.reset_cooldown {
            return;
        }
        // These current Help classes' confirmDel emits ON_RESET_DEVICE.
        // Other reset variants (OBM/OLED/firmware) remain separately audited.
        let local_reset = matches!(self.device.product_id, 164 | 241);
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
        self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
        let generation = self.confirmation_generation;
        let owner = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _cx| {
            dialog
                .title(title.clone())
                .child(message.clone())
                .on_close({
                    let owner = owner.clone();
                    move |_, _, cx| {
                        let _ = owner.update(cx, |view, _| {
                            if view.confirmation_generation == generation {
                                view.confirmation_generation =
                                    view.confirmation_generation.wrapping_add(1);
                            }
                        });
                    }
                })
                .footer(
                    h_flex()
                        .justify_end()
                        .gap_3()
                        .child(
                            Button::new("source-help-reset-cancel")
                                .label(i18n::t("CANCEL"))
                                .on_click({
                                    let owner = owner.clone();
                                    move |_, window, cx| {
                                        let _ = owner.update(cx, |view, _| {
                                            if view.confirmation_generation == generation {
                                                view.confirmation_generation =
                                                    view.confirmation_generation.wrapping_add(1);
                                            }
                                        });
                                        window.close_dialog(cx);
                                    }
                                }),
                        )
                        .child(
                            Button::new("source-help-reset-confirm")
                                .label(i18n::t("RESET"))
                                .disabled(!local_reset)
                                .on_click({
                                    let owner = owner.clone();
                                    move |_, window, cx| {
                                        let accepted = owner
                                            .update(cx, |view, cx| {
                                                if !local_reset
                                                    || view.confirmation_generation != generation
                                                {
                                                    return false;
                                                }
                                                view.pending_reset = true;
                                                view.reset_error = None;
                                                view.reset_generation =
                                                    view.reset_generation.wrapping_add(1);
                                                let request = HelpResetRequest {
                                                    generation: view.reset_generation,
                                                    audio_streams: false,
                                                };
                                                view.reset_cooldown = true;
                                                // Original Help resetDevice re-enables after two
                                                // seconds independently of middleware completion.
                                                view.reset_cooldown_task = Some(cx.spawn_in(
                                                    window,
                                                    async move |view, cx| {
                                                        cx.background_executor()
                                                            .timer(Duration::from_secs(2))
                                                            .await;
                                                        let _ = view.update(cx, |view, cx| {
                                                            if view.reset_generation
                                                                == request.generation
                                                            {
                                                                view.reset_cooldown = false;
                                                                cx.notify();
                                                            }
                                                        });
                                                    },
                                                ));
                                                cx.emit(HelpResetEvent::Requested(request));
                                                cx.notify();
                                                true
                                            })
                                            .unwrap_or(false);
                                        if accepted {
                                            window.close_dialog(cx);
                                        }
                                    }
                                }),
                        ),
                )
        });
    }
    pub(super) fn reset_matches(&self, request: HelpResetRequest) -> bool {
        if request.audio_streams {
            return self.audio_reset_generations.contains(&request.generation);
        }
        self.pending_reset && self.reset_generation == request.generation
    }
    fn cancel_audio_resets(&mut self, cx: &mut Context<Self>) {
        for generation in std::mem::take(&mut self.audio_reset_generations) {
            cx.emit(HelpResetEvent::Canceled(HelpResetRequest {
                generation,
                audio_streams: true,
            }));
        }
        self.audio_reset_tasks.clear();
        self.audio_restart = false;
    }
    fn restart_audio(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.device.product_id != 1342 {
            return;
        }
        // Current ug.resetAudio sets the spinner, delays 1000 ms, invokes its
        // actual resetAudio prop and removes the spinner without awaiting IO.
        // Repeated clicks each retain their own original timer/request.
        self.audio_restart = true;
        self.reset_generation = self.reset_generation.wrapping_add(1);
        let request = HelpResetRequest {
            generation: self.reset_generation,
            audio_streams: true,
        };
        self.audio_reset_generations.insert(request.generation);
        let task = cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let _ = view.update(cx, |view, cx| {
                if !view.reset_matches(request) {
                    return;
                }
                view.audio_restart = false;
                if let Some(task) = view.audio_reset_tasks.remove(&request.generation) {
                    task.detach();
                }
                cx.emit(HelpResetEvent::Requested(request));
                cx.notify();
            });
        });
        self.audio_reset_tasks.insert(request.generation, task);
        cx.notify();
    }
    pub(super) fn accept_source_serial(&mut self, serial: &str) {
        if self.device.serial_number != serial {
            self.device.serial_number = serial.to_owned();
            self.copy_task = None;
            self.copied_serial = false;
        }
    }
    pub(super) fn finish_reset(
        &mut self,
        request: HelpResetRequest,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self.reset_matches(request) {
            return;
        }
        if request.audio_streams {
            self.audio_reset_generations.remove(&request.generation);
            // This action does not replace profiles or claim recovered audio.
            cx.notify();
            return;
        }
        self.pending_reset = false;
        self.reset_error = error;
        cx.notify();
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
fn audio_setup_action(id: &'static str, key: &str, uri: &'static str, cx: &App) -> BaseButton {
    let label = format!("1. {}", i18n::t(key));
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .disabled(!cfg!(windows))
        .self_start()
        .p_0()
        .mb(surface::css(10.))
        .text_color(cx.theme().foreground)
        .hover(|style| style.text_color(cx.theme().primary))
        .child(label)
        .on_click(move |_, _, cx| cx.open_url(uri))
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
        let audio_support = self.device.product_id == 1342;
        if audio_support {
            // ug mounts Audio Troubleshooting in the left column, with
            // Support in the right. No factory-reset confirmation is involved.
            left = v_flex()
                .gap(surface::css(20.))
                .child(
                    surface::panel(i18n::t("SETTING_IT_UP"), cx)
                        .child(i18n::t("SETTING_IT_UP_DES"))
                        .child(
                            div()
                                .mt(surface::css(20.))
                                .child(i18n::t("SETTING_IT_UP_BASIC")),
                        )
                        .child(
                            v_flex()
                                .child(audio_setup_action(
                                    "source-help-sound-settings",
                                    "SETTING_IT_UP_BASIC_1",
                                    "ms-settings:sound",
                                    cx,
                                ))
                                .child(
                                    div()
                                        .mb(surface::css(10.))
                                        .child(format!("2. {}", i18n::t("SETTING_IT_UP_BASIC_2"))),
                                )
                                .child(
                                    div()
                                        .mb(surface::css(10.))
                                        .child(format!("3. {}", i18n::t("SETTING_IT_UP_BASIC_3"))),
                                ),
                        )
                        .child(
                            div()
                                .mt(surface::css(20.))
                                .child(i18n::t("SETTING_IT_UP_ADVANCED")),
                        )
                        .child(
                            v_flex()
                                .child(audio_setup_action(
                                    "source-help-volume-settings",
                                    "SETTING_IT_UP_ADVANCED_1",
                                    "ms-settings:apps-volume",
                                    cx,
                                ))
                                .child(
                                    div().mb(surface::css(10.)).child(format!(
                                        "2. {}",
                                        i18n::t("SETTING_IT_UP_ADVANCED_2")
                                    )),
                                )
                                .child(
                                    div().mb(surface::css(10.)).child(format!(
                                        "3. {}",
                                        i18n::t("SETTING_IT_UP_ADVANCED_3")
                                    )),
                                )
                                .child(
                                    div().mb(surface::css(10.)).child(format!(
                                        "4. {}",
                                        i18n::t("SETTING_IT_UP_ADVANCED_4")
                                    )),
                                ),
                        ),
                )
                .child(
                    surface::panel(i18n::t("AUDIO_TROUBLESHOOTING"), cx)
                        .child(i18n::t("AUDIO_TROUBLESHOOTING_DES"))
                        .child(
                            h_flex()
                                .child(
                                    help_button(
                                        "source-help-restart-audio",
                                        i18n::t("AUDIO_TROUBLESHOOTING_BUTTON"),
                                        false,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.restart_audio(window, cx),
                                    )),
                                )
                                .when(self.audio_restart, |row| row.child(Spinner::new().small())),
                        ),
                );
        }
        if !audio_support && (page.reset || page.oled_reset && !self.device.use_ble) {
            left = left.child(
                surface::panel(i18n::t("FACTORY_RESET"), cx)
                    .child(i18n::t(&page.reset_title))
                    .child(
                        help_button(
                            "source-help-reset",
                            i18n::t("RESET"),
                            self.reset_cooldown,
                            cx,
                        )
                        .self_start()
                        .on_click(cx.listener(|this, _, window, cx| this.show_reset(window, cx))),
                    )
                    .when_some(self.reset_error.clone(), |panel, error| {
                        panel.child(surface::note(error, cx))
                    }),
            );
        }
        let serial = self.device.serial_number.clone();
        let mut right = v_flex().gap(surface::css(20.));
        if audio_support {
            let mut panel = surface::panel(i18n::t("SUPPORT"), cx).gap(surface::css(10.));
            if let Some(url) = &help.support {
                panel = panel.child(help_link(
                    "source-help-audio-device",
                    "VISIT_DEVICE_SUPPORT",
                    url.clone(),
                    cx,
                ));
            }
            if let Some(prefix) = &help.guide {
                let locale = i18n::locale().to_ascii_lowercase();
                let locale = if locale.is_empty() { "en" } else { &locale };
                panel = panel.child(help_link(
                    "source-help-audio-guide",
                    "VISIT_MASTER_PAGE",
                    format!("{prefix}{locale}.pdf"),
                    cx,
                ));
            }
            right = right.child(panel.child(help_link(
                "source-help-audio-synapse",
                "VISIT_SYNAPSE_SUPPORT",
                "https://support.razer.com",
                cx,
            )));
            right = right.child(
                surface::panel(i18n::t("FACTORY_RESET"), cx)
                    .child(i18n::t(&page.reset_title))
                    .child(
                        help_button(
                            "source-help-reset",
                            i18n::t("RESET"),
                            self.reset_cooldown,
                            cx,
                        )
                        .self_start()
                        .on_click(cx.listener(|this, _, window, cx| this.show_reset(window, cx))),
                    ),
            );
        }
        if page.serial {
            right = right.child(
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
        }
        let firmware = self.device.current_firmware_version();
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
        if page.registration {
            right = right.child(surface::panel(i18n::t("PRODUCT_REGISTRATION"), cx).child(
                help_link(
                    "source-help-register",
                    "REGISTER_ONLINE",
                    "https://www.razer.com/product-registration",
                    cx,
                ),
            ));
        }
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }
}
