//! Reviewable source 6505/44442 `H → w/O/L` states. Every record in this surface
//! is explicitly a preview; production ModuleCatalog retains its unknown service state.
use super::{MODULES, Module, source_link};
use crate::{
    features::Choice,
    i18n,
    ui::{
        surface,
        theme::{MainPageColors, PaletteColors, ProfileAlertColors},
    },
};
use gpui_kit::base::{Popover, Progress, ProgressIndicator, ProgressTrack};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    checkbox::Checkbox,
    scroll::ScrollableElement as _,
    select::{SelectEvent, SelectState},
    spinner::Spinner,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::collections::BTreeSet;

/// The source installer phases. Percent and file counts are separate signals;
/// a completed download does not imply a completed installation.
#[derive(Clone, Copy, PartialEq)]
enum InstallerPhase {
    Unknown,
    Available,
    Waiting,
    Downloading { percent: f32, size_mb: u32 },
    Installing { installed: u32, total: u32 },
    Completed,
    Error,
    Canceled,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Connection {
    Usb,
    Dongle,
    Bluetooth,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    Installer,
    Installed,
    Removing,
    Firmware,
    Maintenance,
}

const SCENARIOS: &[(&str, &str)] = &[
    ("unknown", "服务状态未读取"),
    ("available", "可用模块"),
    ("new-device", "新设备"),
    ("waiting", "等待安装"),
    ("downloading", "正在下载"),
    ("saving", "下载完成，正在保存"),
    ("installing", "正在安装"),
    ("completed", "安装完成回调"),
    ("error", "安装失败与重试"),
    ("canceled", "已取消下载"),
    ("maintenance", "模块维护中"),
    ("installed", "已安装模块与断开设备"),
    ("removing", "正在移除"),
    ("firmware-usb", "USB 固件更新"),
    ("firmware-dongle", "无线连接更新受限"),
    ("firmware-bt", "蓝牙连接更新受限"),
    ("firmware-guide", "外部固件更新指南"),
];
fn choices() -> Vec<Choice> {
    SCENARIOS
        .iter()
        .map(|(key, label)| Choice::new(*key, *label))
        .collect()
}

pub(super) fn open(window: &mut Window, cx: &mut App) {
    let preview = cx.new(|cx| ModulePreview::new(window, cx));
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("设备与模块 · 界面预览")
            .width(
                (window.rem_size() * (1280. / 16.))
                    .min(window.viewport_size().width - window.rem_size() * 2.),
            )
            .child(preview.clone())
    });
}

struct ModulePreview {
    selected: Entity<SelectState<Vec<Choice>>>,
    scenario: String,
    scene: Scene,
    phase: InstallerPhase,
    online: bool,
    connection: Connection,
    sdk_update: bool,
    expanded: BTreeSet<&'static str>,
    removal: Option<&'static str>,
    clear_settings: bool,
    removed: BTreeSet<&'static str>,
    last_action: String,
    _subscriptions: Vec<Subscription>,
}
impl ModulePreview {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selected =
            cx.new(|cx| SelectState::new(choices(), Some(IndexPath::new(0)), window, cx));
        let subscription = cx.subscribe(&selected, |this: &mut Self, _, event, cx| {
            if let SelectEvent::Confirm(Some(key)) = event {
                this.choose(key);
                cx.notify();
            }
        });
        Self {
            selected,
            scenario: "unknown".into(),
            scene: Scene::Installer,
            phase: InstallerPhase::Unknown,
            online: true,
            connection: Connection::Usb,
            sdk_update: true,
            expanded: BTreeSet::new(),
            removal: None,
            clear_settings: false,
            removed: BTreeSet::new(),
            last_action: String::new(),
            _subscriptions: vec![subscription],
        }
    }
    fn choose(&mut self, key: &str) {
        self.scenario = key.into();
        self.scene = Scene::Installer;
        self.removal = None;
        self.removed.clear();
        self.clear_settings = false;
        self.last_action.clear();
        self.expanded.clear();
        self.phase = match key {
            "unknown" => InstallerPhase::Unknown,
            "waiting" => InstallerPhase::Waiting,
            "downloading" => InstallerPhase::Downloading {
                percent: 37.,
                size_mb: 64,
            },
            "saving" => InstallerPhase::Downloading {
                percent: 100.,
                size_mb: 64,
            },
            "installing" => InstallerPhase::Installing {
                installed: 17,
                total: 40,
            },
            "completed" => InstallerPhase::Completed,
            "error" => InstallerPhase::Error,
            "canceled" => InstallerPhase::Canceled,
            _ => InstallerPhase::Available,
        };
        match key {
            "installed" => self.scene = Scene::Installed,
            "removing" => self.scene = Scene::Removing,
            "maintenance" => self.scene = Scene::Maintenance,
            "firmware-usb" | "firmware-dongle" | "firmware-bt" | "firmware-guide" => {
                self.scene = Scene::Firmware;
                self.sdk_update = key != "firmware-guide";
                self.connection = match key {
                    "firmware-dongle" => Connection::Dongle,
                    "firmware-bt" => Connection::Bluetooth,
                    _ => Connection::Usb,
                };
                self.expanded.insert("firmware");
            }
            _ => {}
        }
    }
    fn toggle(&mut self, key: &'static str, cx: &mut Context<Self>) {
        if !self.expanded.remove(key) {
            self.expanded.insert(key);
        }
        cx.notify();
    }
    fn preview_install(&mut self, cx: &mut Context<Self>) {
        self.phase = InstallerPhase::Downloading {
            percent: 0.,
            size_mb: 64,
        };
        self.last_action = "已预览安装请求。进度由所选样例决定。".into();
        cx.notify();
    }
    fn preview_cancel(&mut self, cx: &mut Context<Self>) {
        self.phase = InstallerPhase::Canceled;
        self.last_action = "已预览取消下载。".into();
        cx.notify();
    }
    fn heading(&self, key: &str, cx: &App) -> Div {
        div()
            .font_family("RazerF5")
            .text_size(surface::css(24.))
            .text_color(cx.theme().primary)
            .mb(surface::css(10.))
            .child(i18n::t(key).to_uppercase())
    }
    fn action(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        primary: bool,
        cx: &App,
    ) -> Button {
        Button::new(id)
            .label(label)
            .min_w(surface::css(90.))
            .h(surface::css(27.))
            .px(surface::css(16.))
            .py_0()
            .rounded(cx.theme().font_size * (3. / 16.))
            .text_size(surface::css(12.))
            .line_height(surface::css(12.))
            .border_1()
            .border_color(PaletteColors.swatch_border())
            .ml(surface::css(30.))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(if primary {
                        cx.theme().primary
                    } else {
                        PaletteColors.secondary()
                    })
                    .foreground(if primary {
                        cx.theme().title_bar
                    } else {
                        PaletteColors.white()
                    })
                    .hover(if primary {
                        cx.theme().primary.opacity(0.8)
                    } else {
                        PaletteColors.secondary().opacity(0.8)
                    }),
            )
    }
    fn row_title(
        &self,
        icon: &'static str,
        title: impl Into<SharedString>,
        faded: bool,
        cx: &App,
    ) -> Div {
        h_flex()
            .child(
                img(icon)
                    .size(surface::css(40.))
                    .when(faded, |i| i.opacity(0.3)),
            )
            .child(
                div()
                    .ml(surface::css(10.))
                    .w(surface::css(500.))
                    .flex_shrink_0()
                    .text_size(surface::css(16.))
                    .text_color(cx.theme().foreground)
                    .when(faded, |d| d.opacity(0.3))
                    .text_ellipsis()
                    .child(title.into()),
            )
    }
    fn progress(&self, id: &'static str, percent: f32, text: String, cx: &App) -> AnyElement {
        v_flex()
            .w(surface::css(200.))
            .gap(surface::css(5.))
            .child(
                Progress::new(id)
                    .value(percent)
                    .accessibility_label(text.clone())
                    .child(
                        ProgressTrack::new()
                            .w_full()
                            .h(surface::css(8.))
                            .rounded(surface::css(4.))
                            .bg(cx.theme().primary.opacity(0.3))
                            .child(
                                ProgressIndicator::new()
                                    .h_full()
                                    .w(relative(percent.clamp(0., 100.) / 100.))
                                    .rounded(surface::css(4.))
                                    .bg(cx.theme().primary),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(surface::css(14.))
                    .text_color(cx.theme().muted_foreground)
                    .child(text),
            )
            .into_any_element()
    }
    fn install_row(&self, item: &'static Module, cx: &mut Context<Self>) -> AnyElement {
        let expanded = self.expanded.contains(item.id);
        let is_device = self.scenario == "new-device";
        let progress = match self.phase {
            InstallerPhase::Downloading { percent, size_mb } => self.progress(
                "preview-download",
                percent,
                if percent >= 100. {
                    format!("{}…", i18n::t("SAVING"))
                } else {
                    format!(
                        "{} {}/{size_mb} MB",
                        i18n::t("DOWNLOADING"),
                        (percent * size_mb as f32 / 100.).round() as u32
                    )
                },
                cx,
            ),
            InstallerPhase::Installing { installed, total } => self.progress(
                "preview-install",
                installed as f32 / total.max(1) as f32 * 100.,
                format!("{}…", i18n::t("INSTALLING")),
                cx,
            ),
            InstallerPhase::Error => div()
                .text_color(ProfileAlertColors::new().danger())
                .child(i18n::t_or("INSTALLATION_FAILED", "安装失败"))
                .into_any_element(),
            InstallerPhase::Unknown => div()
                .text_color(cx.theme().muted_foreground)
                .child("安装状态未读取")
                .into_any_element(),
            _ => div().into_any_element(),
        };
        let action = if self.scene == Scene::Maintenance {
            h_flex()
                .ml(surface::css(30.))
                .gap(surface::css(10.))
                .child(
                    Icon::new(IconName::TriangleAlert).text_color(MainPageColors.tutorial_accent()),
                )
                .child(i18n::t("DISABLED_FEATURE_DESC"))
                .into_any_element()
        } else {
            match self.phase {
                InstallerPhase::Downloading { percent, .. } => self
                    .action("cancel-download", i18n::t("CANCEL"), false, cx)
                    .disabled(percent >= 100.)
                    .on_click(cx.listener(|this, _, _, cx| this.preview_cancel(cx)))
                    .into_any_element(),
                InstallerPhase::Installing { .. } => self
                    .action("cancel-install", i18n::t("CANCEL"), false, cx)
                    .disabled(true)
                    .into_any_element(),
                _ => self
                    .action(
                        "start-install",
                        i18n::t(if self.phase == InstallerPhase::Error {
                            "RETRY"
                        } else {
                            "INSTALL"
                        }),
                        true,
                        cx,
                    )
                    .disabled(!self.online || self.phase == InstallerPhase::Unknown)
                    .when(
                        !self.online || self.phase == InstallerPhase::Unknown,
                        |button| {
                            button.tooltip(if self.phase == InstallerPhase::Unknown {
                                "尚未连接模块安装服务".into()
                            } else {
                                i18n::t("INTERNET_CONNECTION_REQUIRED")
                            })
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.preview_install(cx)))
                    .into_any_element(),
            }
        };
        v_flex()
            .w_full()
            .child(
                h_flex()
                    .h(surface::css(80.))
                    .pl(surface::css(20.))
                    .pr(surface::css(30.))
                    .mb(surface::css(1.))
                    .bg(cx.theme().group_box)
                    .child(self.row_title(
                        if is_device {
                            "synapse/category-keyboard.svg"
                        } else {
                            item.icon
                        },
                        if is_device {
                            "预览设备 A"
                        } else {
                            item.title
                        },
                        false,
                        cx,
                    ))
                    .child(div().w(surface::css(145.)).when(!is_device, |d| {
                        d.child(
                            Button::new("module-more-info")
                                .ghost()
                                .p_0()
                                .justify_start()
                                .w(surface::css(145.))
                                .label(i18n::t(if expanded {
                                    "CLOSE"
                                } else {
                                    "MORE_INFORMATION"
                                }))
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.toggle(item.id, cx)),
                                ),
                        )
                    }))
                    .child(h_flex().flex_1().justify_end().child(progress))
                    .child(action),
            )
            .when(expanded && !is_device, |d| {
                d.child(self.description(item, cx))
            })
            .into_any_element()
    }
    fn description(&self, item: &'static Module, cx: &App) -> AnyElement {
        h_flex()
            .items_start()
            .min_h(surface::css(202.))
            .p(surface::css(20.))
            .bg(MainPageColors.detail_surface())
            .when_some(item.image, |d, path| {
                d.child(
                    img(path)
                        .w(surface::css(288.))
                        .h(surface::css(162.))
                        .mr(surface::css(20.))
                        .object_fit(ObjectFit::Contain),
                )
            })
            .child(
                v_flex()
                    .w(surface::css(592.))
                    .text_size(surface::css(14.))
                    .text_color(cx.theme().foreground)
                    .child(
                        div()
                            .mb(surface::css(20.))
                            .whitespace_normal()
                            .child(item.description),
                    )
                    .child(
                        h_flex()
                            .when_some(item.url, |d, url| {
                                d.child(source_link(
                                    "preview-learn-more",
                                    i18n::t("LEARN_MORE"),
                                    url,
                                    cx,
                                ))
                            })
                            .child(
                                div()
                                    .ml_auto()
                                    .mr(surface::css(20.))
                                    .child(format!("{}: < 1 MB", i18n::t("SIZE"))),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn installed_row(
        &self,
        key: &'static str,
        title: &'static str,
        icon: &'static str,
        removable: bool,
        device: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let removing = self.scene == Scene::Removing || self.removed.contains(key);
        let disconnected = device && removable;
        let owner = cx.entity().downgrade();
        let remove = Button::new(SharedString::from(format!("remove-{key}")))
            .label(i18n::t("REMOVE"))
            .ghost()
            .h(surface::css(27.))
            .ml(surface::css(30.))
            .min_w(surface::css(45.))
            .underline()
            .px_0()
            .disabled(!removable)
            .custom(
                ButtonCustomVariant::new(cx)
                    .foreground(cx.theme().muted_foreground)
                    .hover(cx.theme().transparent),
            )
            .hover(|style| style.text_color(ProfileAlertColors::new().headphone_danger()));
        let panel = v_flex()
            .id(SharedString::from(format!("remove-confirm-{key}")))
            .w(surface::css(300.))
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(ProfileAlertColors::new().danger())
            .rounded(cx.theme().font_size * (3. / 16.))
            .text_size(surface::css(14.))
            .text_center()
            .child(
                div()
                    .text_size(surface::css(14.))
                    .text_color(ProfileAlertColors::new().danger())
                    .child(i18n::t(if device {
                        "REMOVE_DEVICE_TITLE"
                    } else {
                        "REMOVE_MODULE_TITLE"
                    })),
            )
            .child(if key == "macro" {
                v_flex()
                    .text_left()
                    .gap(surface::css(4.))
                    .child(i18n::t("REMOVE_MODULE_MACRO_MAIN_DESC"))
                    .child(div().pb(surface::css(20.)).child(format!(
                        "{}:",
                        i18n::t("REMOVE_MODULE_MACRO_DEVICE_ASSIGNMENTS_DESC")
                    )))
                    .child("• 预览设备 A")
                    .child("• 预览设备 B")
                    .child(
                        div()
                            .pt(surface::css(20.))
                            .child(i18n::t("ADD_MODULE_ANYTIME")),
                    )
                    .into_any_element()
            } else {
                div()
                    .whitespace_normal()
                    .child(
                        i18n::t(if device {
                            "REMOVE_DEVICE_MSG"
                        } else {
                            "REMOVE_MODULE_MSG"
                        })
                        .replace("{{title}}", title),
                    )
                    .into_any_element()
            })
            .when(!device && key != "macro", |d| {
                d.child(
                    Checkbox::new("clear-module-settings")
                        .mt(surface::css(12.))
                        .checked(self.clear_settings)
                        .label(i18n::t("REMOVE_ALEXA_SETTINGS"))
                        .on_click(cx.listener(|this, checked, _, cx| {
                            this.clear_settings = *checked;
                            cx.notify();
                        })),
                )
            })
            .child(
                self.action("confirm-module-remove", i18n::t("REMOVE"), false, cx)
                    .ml_0()
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(ProfileAlertColors::new().danger())
                            .foreground(cx.theme().title_bar)
                            .hover(ProfileAlertColors::new().danger().opacity(0.8)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.removal = None;
                        this.removed.insert(key);
                        this.last_action = format!(
                            "已预览移除“{title}”{}。",
                            if this.clear_settings {
                                "并清除模块设置"
                            } else {
                                ""
                            }
                        );
                        this.clear_settings = false;
                        cx.notify();
                    })),
            );
        h_flex()
            .relative()
            .h(surface::css(80.))
            .pl(surface::css(20.))
            .pr(surface::css(30.))
            .mb(surface::css(1.))
            .bg(cx.theme().group_box)
            .child(self.row_title(
                icon,
                if disconnected {
                    format!("{title} ({})", i18n::t("DISCONNECTED"))
                } else {
                    title.into()
                },
                disconnected,
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .text_size(surface::css(14.))
                    .text_color(cx.theme().muted_foreground)
                    .when(!removing, |d| {
                        d.child(format!("{}: 2026年 10月 01日", i18n::t("LAST_UPDATE")))
                    }),
            )
            .child(if removing {
                h_flex()
                    .gap(surface::css(5.))
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        Spinner::new().with_size(surface::css(20.).to_pixels(cx.theme().font_size)),
                    )
                    .child(i18n::t("TEXT_REMOVING"))
                    .into_any_element()
            } else {
                Popover::new(SharedString::from(format!("remove-popover-{key}")))
                    .anchor(Anchor::TopRight)
                    .offset(surface::css(-2.5).to_pixels(cx.theme().font_size))
                    .open(self.removal == Some(key))
                    .trigger(remove)
                    .on_open_change(move |open, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.removal = (*open && removable).then_some(key);
                            this.clear_settings = false;
                            cx.notify();
                        });
                    })
                    .content(move |_, _, _| panel)
                    .into_any_element()
            })
            .into_any_element()
    }
    fn firmware(&self, cx: &mut Context<Self>) -> AnyElement {
        let warning = if !self.sdk_update {
            None
        } else {
            match self.connection {
                Connection::Usb => None,
                Connection::Dongle => Some("FW_UPDATE_DISABLED"),
                Connection::Bluetooth => Some("FW_UPDATE_BT_DISABLED"),
            }
        };
        let expanded = self.expanded.contains("firmware");
        let title = "预览设备 A";
        v_flex()
            .w_full()
            .child(self.heading("FIRMWARE_UPDATES", cx))
            .child(
                h_flex()
                    .h(surface::css(80.))
                    .pl(surface::css(20.))
                    .pr(surface::css(30.))
                    .mb(surface::css(1.))
                    .bg(cx.theme().group_box)
                    .child(
                        h_flex()
                            .text_color(MainPageColors.tutorial_accent())
                            .child(
                                Icon::default()
                                    .path("synapse/category-keyboard.svg")
                                    .size(surface::css(40.)),
                            )
                            .child(
                                div()
                                    .ml(surface::css(10.))
                                    .w(surface::css(500.))
                                    .flex_shrink_0()
                                    .text_size(surface::css(16.))
                                    .text_ellipsis()
                                    .child(format!(
                                        "{title} ({})",
                                        i18n::t("FIRMWARE_UPDATE_REQUIRED")
                                    )),
                            ),
                    )
                    .child(
                        Button::new("firmware-info")
                            .ghost()
                            .p_0()
                            .justify_start()
                            .w(surface::css(145.))
                            .label(i18n::t(if expanded {
                                "CLOSE"
                            } else {
                                "MORE_INFORMATION"
                            }))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle("firmware", cx))),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .max_w(surface::css(314.))
                            .gap(surface::css(10.))
                            .when_some(warning, |d, key| {
                                d.child(
                                    Icon::new(IconName::TriangleAlert)
                                        .text_color(MainPageColors.tutorial_accent()),
                                )
                                .child(
                                    div()
                                        .text_size(surface::css(14.))
                                        .whitespace_normal()
                                        .child(i18n::t(key)),
                                )
                            }),
                    )
                    .child(
                        self.action(
                            "launch-firmware-preview",
                            i18n::t("LAUNCH_UPDATER"),
                            false,
                            cx,
                        )
                        .disabled(warning.is_some())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.last_action = if this.sdk_update {
                                "已预览启动设备固件更新程序。"
                            } else {
                                "已预览打开在线固件更新指南。"
                            }
                            .into();
                            cx.notify();
                        })),
                    ),
            )
            .when(expanded, |d| {
                d.child(
                    h_flex()
                        .items_start()
                        .p(surface::css(20.))
                        .bg(MainPageColors.detail_surface())
                        .text_size(surface::css(14.))
                        .child(
                            v_flex()
                                .w(surface::css(200.))
                                .flex_shrink_0()
                                .gap(surface::css(10.))
                                .child(
                                    i18n::t("VERSION_NUMBER")
                                        .replace("{{version}}", "1.02.00（预览）"),
                                )
                                .child(format!("{}: 2026年 10月 01日", i18n::t("RELEASED")))
                                .when(!self.sdk_update, |d| {
                                    d.child(format!("{}: 12 MB", i18n::t("SIZE")))
                                }),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .gap(surface::css(10.))
                                .child(
                                    Button::new("preview-firmware-guide")
                                        .ghost()
                                        .p_0()
                                        .justify_start()
                                        .label(
                                            i18n::t("FIRMWARE_UPDATE_GUIDE")
                                                .replace("{{deviceName}}", title)
                                                .replace("{{version}}", "1.02.00"),
                                        )
                                        .icon(IconName::ExternalLink)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.last_action = "已预览固件指南链接。".into();
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .pl(surface::css(15.))
                                        .child("• 预览发行说明：改善设备连接稳定性。"),
                                )
                                .child(
                                    div()
                                        .pl(surface::css(15.))
                                        .child("• 预览发行说明：更新设备兼容性。"),
                                ),
                        ),
                )
            })
            .into_any_element()
    }
}
impl Render for ModulePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.scene == Scene::Firmware {
            self.firmware(cx)
        } else if matches!(self.scene, Scene::Installed | Scene::Removing) {
            v_flex()
                .child(self.heading("UPDATED_RECENTLY", cx))
                .child(self.installed_row(
                    "connected",
                    "预览设备 A",
                    "synapse/category-keyboard.svg",
                    false,
                    true,
                    cx,
                ))
                .child(self.installed_row(
                    "disconnected",
                    "预览设备 B",
                    "synapse/category-mouse.svg",
                    true,
                    true,
                    cx,
                ))
                .child(self.installed_row(
                    "alexa",
                    "Alexa",
                    "synapse/module-alexa.svg",
                    true,
                    false,
                    cx,
                ))
                .child(self.installed_row(
                    "macro",
                    "宏",
                    "synapse/module-macro.svg",
                    true,
                    false,
                    cx,
                ))
                .child(self.installed_row(
                    "linkedGames",
                    "已关联的游戏",
                    "synapse/module-linked-games.svg",
                    false,
                    false,
                    cx,
                ))
                .into_any_element()
        } else {
            v_flex()
                .child(self.heading(
                    if self.scenario == "new-device" {
                        "NEW_DEVICES"
                    } else {
                        "AVAILABLE_MODULES"
                    },
                    cx,
                ))
                .child(self.install_row(&MODULES[0], cx))
                .into_any_element()
        };
        v_flex()
            .w_full()
            .gap(surface::css(16.))
            .child(
                div()
                    .text_size(surface::css(14.))
                    .whitespace_normal()
                    .child("界面预览：以下是演示数据。安装、移除和固件按钮仅预览交互。"),
            )
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .child(div().child("状态样例"))
                    .child(
                        surface::select(&self.selected)
                            .items(choices())
                            .accessibility_label("预览状态")
                            .w(surface::css(300.)),
                    )
                    .child(
                        Checkbox::new("preview-online")
                            .checked(self.online)
                            .label("网络已连接")
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.online = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("module-preview-scroll")
                    .w_full()
                    .max_h(
                        (window.viewport_size().height - window.rem_size() * 16.)
                            .max(window.rem_size() * 12.),
                    )
                    .overflow_scrollbar()
                    .child(
                        div()
                            .w(surface::css(1220.))
                            .min_w(surface::css(1220.))
                            .pb(surface::css(120.))
                            .child(body),
                    ),
            )
            .child(
                div()
                    .min_h(surface::css(20.))
                    .text_size(surface::css(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last_action.clone()),
            )
    }
}
