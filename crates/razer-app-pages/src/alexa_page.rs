//! Alexa's standalone frontend (Bp / Yp / yE / OE), with isolated preview scenes.
//! Installation, authentication and device state are never inferred from a preview.
use gpui_kit::base::motion::{Easing, Presence, Transition};
use gpui_kit::base::{Button, Checkbox, CheckboxState, Collapsible, Link};
use gpui_kit::component::{
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_pages::features::Choice;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use razer_widgets::surface::css;
use razer_widgets::theme::AlexaColors;
use std::time::Duration;

mod controls;
mod dialogs;
mod installer;
mod sections;
use controls::{SourceButtonKind, source_button, source_switch, text_button};
use dialogs::{Modal, ModalKind};
use installer::InstallState;

#[cfg(test)]
mod tests;

fn text(key: &str) -> String {
    i18n::t(&format!("ALEXA_SOURCE.{key}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Home,
    Skills,
    Settings,
    Help,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Account {
    Unknown,
    Guest,
    Razer,
    Code,
    InvalidCode,
    Unauthorized,
    Error,
    Ready,
}

const SCENES: &[(&str, &str)] = &[
    ("unknown", "尚未读取安装与账户状态"),
    ("guest", "本地预览：未登录 Razer"),
    ("razer", "本地预览：等待 Amazon 登录"),
    ("requesting", "本地预览：登录请求中"),
    ("code", "本地预览：验证码（示例）"),
    ("expired", "本地预览：验证码过期"),
    ("unauthorized", "本地预览：授权被拒绝"),
    ("error", "本地预览：登录错误"),
    ("ready", "本地预览：已登录（示例）"),
    ("detecting", "本地预览：正在检测模块"),
    ("available", "本地预览：可安装"),
    ("waiting", "本地预览：等待安装"),
    ("downloading", "本地预览：下载中"),
    ("saving", "本地预览：保存中"),
    ("installing", "本地预览：安装中"),
    ("install-error", "本地预览：安装错误"),
    ("notes-loading", "本地预览：更新说明加载中"),
    ("notes", "本地预览：更新说明（示例）"),
    ("notes-empty", "本地预览：更新说明为空"),
    ("notes-waiting", "本地预览：更新等待中"),
    ("notes-downloading", "本地预览：更新下载中"),
    ("notes-saving", "本地预览：更新保存中"),
    ("notes-installing", "本地预览：更新安装中"),
];
fn scene_choices() -> Vec<Choice> {
    SCENES
        .iter()
        .map(|(id, label)| Choice::new(*id, *label))
        .collect()
}
fn input_choices() -> Vec<Choice> {
    vec![
        Choice::new("default", text("DEFAULT")),
        Choice::new("sample-mic", "示例麦克风（未检测设备）"),
    ]
}
fn language_choices() -> Vec<Choice> {
    [
        ("de-DE", "Deutsch"),
        ("en-AU", "English - Australia"),
        ("en-CA", "English - Canada"),
        ("en-IN", "English - India"),
        ("en-GB", "English - United Kingdom"),
        ("en-US", "English - United States"),
        ("es-MX", "Español - Mexico"),
        ("es-ES", "Español - Spain"),
        ("es-US", "Español - United States"),
        ("fr-CA", "Français - Canada"),
        ("fr-FR", "Français - France"),
        ("ja-JP", "日本語"),
    ]
    .into_iter()
    .map(|(id, label)| Choice::new(id, label))
    .collect()
}

struct Skill {
    id: &'static str,
    icon: &'static str,
    title: &'static str,
    description: &'static str,
    content: &'static [&'static str],
}
const SKILLS: &[Skill] = &[
    Skill {
        id: "basic-lighting",
        icon: "synapse/alexa-skill-lighting.svg",
        title: "BASIC_LIGHTING_CONTROLS",
        description: "BASIC_LIGHTING_DESC",
        content: &["BASIC_LIGHTING_DESC", "BASIC_LIGHTING_CONTENT_2"],
    },
    Skill {
        id: "chroma-lighting",
        icon: "synapse/alexa-skill-chroma.svg",
        title: "CHROMA_LIGHTING_CONTROLS",
        description: "CHROMA_LIGHTING_DESC",
        content: &[
            "CHROMA_LIGHTING_CONTENT_1",
            "CHROMA_LIGHTING_CONTENT_2",
            "CHROMA_LIGHTING_CONTENT_3",
            "CHROMA_LIGHTING_CONTENT_4",
            "CHROMA_LIGHTING_CONTENT_5",
        ],
    },
    Skill {
        id: "launch-application",
        icon: "synapse/alexa-skill-launch.svg",
        title: "LAUNCH_APPLICATION",
        description: "LAUNCH_APPLICATION_DESC",
        content: &["LAUNCH_APPLICATION_CONTENT"],
    },
    Skill {
        id: "multimedia",
        icon: "synapse/alexa-skill-media.svg",
        title: "MULTI_MEDIA_CONTROLS",
        description: "MULTI_MEDIA_CONTENT_1",
        content: &["MULTI_MEDIA_CONTENT_1", "MULTI_MEDIA_CONTENT_2"],
    },
    Skill {
        id: "power",
        icon: "synapse/alexa-skill-power.svg",
        title: "POWERS_CONTROLS",
        description: "POWER_CONTROLS_DESC",
        content: &[],
    },
];

pub struct AlexaPage {
    preview: bool,
    focus: FocusHandle,
    shortcut_focus: FocusHandle,
    page: Page,
    history: Vec<Page>,
    history_index: usize,
    account: Account,
    pending: bool,
    scene: String,
    scenes: Entity<SelectState<Vec<Choice>>>,
    inputs: Entity<SelectState<Vec<Choice>>>,
    languages: Entity<SelectState<Vec<Choice>>>,
    input: String,
    language: String,
    synapse_skills: bool,
    expanded: Option<&'static str>,
    enabled: bool,
    sounds: bool,
    cards: bool,
    wake_word: bool,
    shortcut_enabled: bool,
    shortcut: String,
    recording: bool,
    installer: Option<InstallState>,
    modal: Option<Modal>,
    modal_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    lifecycle_task: Option<Task<()>>,
    patch_height: Pixels,
    notice: String,
    _subscriptions: Vec<Subscription>,
}

impl AlexaPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::create(false, window, cx)
    }
    pub fn new_preview(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::create(true, window, cx)
    }
    fn create(preview: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scenes =
            cx.new(|cx| SelectState::new(scene_choices(), Some(IndexPath::new(0)), window, cx));
        let inputs =
            cx.new(|cx| SelectState::new(input_choices(), Some(IndexPath::new(0)), window, cx));
        let languages =
            cx.new(|cx| SelectState::new(language_choices(), Some(IndexPath::new(5)), window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&scenes, window, |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(scene)) = event {
                    if !this.preview {
                        return;
                    }
                    this.choose_scene(scene, window, cx);
                }
            }),
            cx.subscribe_in(&inputs, window, |this: &mut Self, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if this.enabled && this.account == Account::Ready {
                        this.input = value.clone();
                        cx.notify();
                    }
                }
            }),
            cx.subscribe_in(&languages, window, |this: &mut Self, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if this.enabled && this.account == Account::Ready {
                        this.language = value.clone();
                        cx.notify();
                    }
                }
            }),
        ];
        Self {
            preview,
            focus: cx.focus_handle(),
            shortcut_focus: cx.focus_handle(),
            page: Page::Home,
            history: vec![Page::Home],
            history_index: 0,
            account: Account::Unknown,
            pending: false,
            scene: "unknown".into(),
            scenes,
            inputs,
            languages,
            input: "default".into(),
            language: "en-US".into(),
            synapse_skills: true,
            expanded: None,
            enabled: true,
            sounds: true,
            cards: true,
            wake_word: true,
            shortcut_enabled: true,
            shortcut: "Ctrl + Shift + A".into(),
            recording: false,
            // The native page is already available; this is an installer view,
            // not an observation that the external Alexa module is installed.
            installer: preview.then_some(InstallState::Unknown),
            modal: None,
            modal_focus: cx.focus_handle(),
            return_focus: None,
            lifecycle_task: None,
            patch_height: px(0.),
            notice: String::new(),
            _subscriptions: subscriptions,
        }
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if self.modal.is_some() {
            self.modal_focus.focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
    }
    pub fn has_previous_page(&self) -> bool {
        self.history_index > 0 && self.modal.is_none()
    }
    pub fn history_blocked(&self) -> bool {
        self.modal.is_some()
    }
    pub fn has_next_page(&self) -> bool {
        self.history_index + 1 < self.history.len() && self.modal.is_none()
    }
    pub fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_previous_page() {
            self.history_index -= 1;
            self.page = self.history[self.history_index];
            self.recording = false;
            self.focus(window, cx);
            cx.notify();
        }
    }
    pub fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_next_page() {
            self.history_index += 1;
            self.page = self.history[self.history_index];
            self.recording = false;
            self.focus(window, cx);
            cx.notify();
        }
    }
    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scene = self.scene.clone();
        self.choose_scene(&scene, window, cx);
    }
    fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_some()
            || self.installer.is_some()
            || (matches!(page, Page::Skills | Page::Settings) && self.account != Account::Ready)
        {
            return;
        }
        if self.page != page {
            self.history.truncate(self.history_index + 1);
            self.history.push(page);
            self.history_index += 1;
            self.page = page;
        }
        self.recording = false;
        self.focus(window, cx);
        cx.notify();
    }
    fn choose_scene(&mut self, scene: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preview && scene != "unknown" {
            return;
        }
        self.lifecycle_task = None;
        self.modal = None;
        self.return_focus = None;
        self.page = Page::Home;
        self.history = vec![Page::Home];
        self.history_index = 0;
        self.recording = false;
        self.expanded = None;
        self.notice.clear();
        self.scene = scene.into();
        self.pending = scene == "requesting";
        self.account = match scene {
            "guest" => Account::Guest,
            "razer" | "requesting" => Account::Razer,
            "code" => Account::Code,
            "expired" => Account::InvalidCode,
            "unauthorized" => Account::Unauthorized,
            "error" => Account::Error,
            "ready" => Account::Ready,
            scene if scene.starts_with("notes") => Account::Ready,
            _ => Account::Unknown,
        };
        self.installer = match scene {
            "unknown" if self.preview => Some(InstallState::Unknown),
            "detecting" => Some(InstallState::Detecting),
            "available" => Some(InstallState::Available),
            "waiting" => Some(InstallState::Waiting),
            "downloading" => Some(InstallState::Downloading),
            "saving" => Some(InstallState::Saving),
            "installing" => Some(InstallState::Installing),
            "install-error" => Some(InstallState::Error),
            _ => None,
        };
        self.scenes.update(cx, |state, cx| {
            state.set_selected_value(&scene.to_owned(), window, cx)
        });
        self.focus(window, cx);
        if scene.starts_with("notes") {
            self.open_patch_scene(scene, window, cx);
        }
        cx.notify();
    }
    fn auth_scene(&mut self, scene: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.account != Account::Unknown {
            self.choose_scene(scene, window, cx);
        }
    }
    fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .h(css(48.))
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .border_b_2()
            .border_color(cx.theme().title_bar)
            .child(div().flex_1())
            .child(
                h_flex().gap(css(20.)).children(
                    [
                        (Page::Home, "HOME"),
                        (Page::Skills, "SKILLS"),
                        (Page::Settings, "SETTINGS"),
                    ]
                    .into_iter()
                    .filter(|(page, _)| *page == Page::Home || self.account == Account::Ready)
                    .map(|(page, label)| {
                        surface::navigation_button(
                            match page {
                                Page::Home => "alexa-home",
                                Page::Skills => "alexa-skills",
                                Page::Settings => "alexa-settings",
                                Page::Help => "alexa-help-tab",
                            },
                            label,
                            self.page == page,
                            cx,
                        )
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.navigate(page, window, cx)),
                        )
                    }),
                ),
            )
            .child(
                h_flex().flex_1().justify_end().pr(css(10.)).child(
                    surface::asset_button(
                        "alexa-help",
                        if self.page == Page::Help {
                            "synapse/help-active.svg"
                        } else {
                            "synapse/help-default.svg"
                        },
                        "Alexa 帮助",
                        cx,
                    )
                    .size(css(24.))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.navigate(Page::Help, window, cx)),
                    ),
                ),
            )
            .into_any_element()
    }
    fn home(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let description = |width| {
            v_flex()
                .w_full()
                .max_w(css(width))
                .mx_auto()
                .mt(css(20.))
                .text_center()
        };
        match self.account {
            Account::Unknown => div()
                .child("尚未读取 Razer ID 与 Amazon 账户状态")
                .into_any_element(),
            Account::Guest => v_flex()
                .w_full()
                .max_w(css(480.))
                .mx_auto()
                .py(css(30.))
                .px(css(40.))
                .rounded(css(5.))
                .bg(cx.theme().group_box)
                .child(
                    div()
                        .mb(css(20.))
                        .font_family("RazerF5")
                        .text_size(css(16.))
                        .text_color(cx.theme().primary)
                        .child(text("ALEXA_LOGIN_TITLE").to_uppercase()),
                )
                .child(text("ALEXA_LOGIN_DESC"))
                .child(
                    h_flex().justify_center().mt(css(20.)).child(
                        source_button(
                            "alexa-razer-login",
                            text("TEXT_LOG_IN"),
                            SourceButtonKind::Green,
                            false,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.notice =
                                "Razer ID 登录服务未连接；可在上方选择示例账户状态。".into();
                            cx.notify();
                        })),
                    ),
                )
                .into_any_element(),
            Account::Razer => v_flex()
                .child(description(550.).child(text("ALEXA_PAGE_DESC_1")))
                .child(description(550.).child(text("ALEXA_PAGE_DESC_2")))
                .child(
                    h_flex().justify_center().mt(css(20.)).child(
                        source_button(
                            "alexa-amazon-login",
                            text("LOGIN_AMAZON"),
                            SourceButtonKind::Amazon,
                            self.pending,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.auth_scene("requesting", window, cx)
                        })),
                    ),
                )
                .child(self.regions())
                .into_any_element(),
            Account::Code => v_flex()
                .child(description(560.).child(format!(
                    "{} [verificationURL · 示例占位，非登录地址] {}",
                    text("LOGIN_AMAZON_DESC_1"),
                    text("LOGIN_AMAZON_DESC_2")
                )))
                .child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .w(css(220.))
                        .h(css(115.))
                        .mt(css(20.))
                        .mx_auto()
                        .bg(cx.theme().group_box)
                        .child(div().text_size(css(32.)).child("SAMPLE"))
                        .child(
                            source_button(
                                "alexa-copy",
                                text("COPY"),
                                SourceButtonKind::Green,
                                false,
                                window,
                                cx,
                            )
                            .mt(css(10.))
                            .text_color(cx.theme().group_box)
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    "SAMPLE — 本地界面示例，不能用于登录".into(),
                                ));
                                this.notice = "已复制示例文字（不能用于登录）".into();
                                cx.notify();
                            })),
                        ),
                )
                .child(
                    h_flex().justify_center().mt(css(10.)).child(
                        text_button("alexa-code-cancel", text("TEXT_CANCEL"), cx)
                            .text_size(css(16.))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.auth_scene("razer", window, cx)
                            })),
                    ),
                )
                .into_any_element(),
            Account::InvalidCode | Account::Unauthorized | Account::Error => v_flex()
                .child(description(550.).child(text(match self.account {
                    Account::InvalidCode => "TEXT_INVALID_CODE_PAIR",
                    Account::Unauthorized => "TEXT_UNAUTHORIZED_CLIENT",
                    _ => "TEXT_ERROR_DEFAULT",
                })))
                .child(
                    h_flex()
                        .justify_center()
                        .gap(css(10.))
                        .mt(css(20.))
                        .child(
                            source_button(
                                "alexa-auth-retry",
                                text("TEXT_RETRY"),
                                SourceButtonKind::Green,
                                self.pending,
                                window,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pending = true;
                                this.notice =
                                    "本地登录请求预览；可在场景列表选择验证码或错误结果。".into();
                                cx.notify();
                            })),
                        )
                        .child(
                            source_button(
                                "alexa-auth-cancel",
                                text("TEXT_CANCEL"),
                                SourceButtonKind::Gray,
                                false,
                                window,
                                cx,
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| this.auth_scene("razer", window, cx),
                            )),
                        ),
                )
                .into_any_element(),
            Account::Ready => v_flex()
                .items_center()
                // Source 80vh is relative to the hosted WebContents below TabUI.
                .min_h(
                    (window.viewport_size().height - window.rem_size() * (42. / 16.)).max(px(0.))
                        * 0.8,
                )
                .child(
                    description(400.)
                        .child(format!(
                            "{}{}",
                            text("PAGE_DESC_ALEXA_READY_1"),
                            text("PAGE_DESC_ALEXA_READY_2")
                        ))
                        .child(div().mt(css(19.52)).child(text("PAGE_DESC_ALEXA_READY_3"))),
                )
                .child(
                    h_flex()
                        .w_full()
                        .max_w(css(980.))
                        .mx_auto()
                        .px(css(10.))
                        .justify_center()
                        .flex_wrap()
                        .children((1..=6).map(|number| {
                            let color = if number <= 3 {
                                AlexaColors::bubble_blue()
                            } else {
                                cx.theme().primary
                            };
                            v_flex()
                                .relative()
                                .w(css(300.))
                                .max_h(css(120.))
                                .p(css(20.))
                                .mx(css(10.))
                                .mt(css(10.))
                                .mb(css(40.))
                                .rounded(css(5.))
                                .text_center()
                                .text_size(css(20.))
                                .bg(color)
                                .text_color(if number <= 3 {
                                    razer_widgets::theme::PaletteColors.white()
                                } else {
                                    AlexaColors::bubble_text()
                                })
                                .child(
                                    div()
                                        .line_clamp(3)
                                        .child(text(&format!("ALEXA_TRIAL_{number}"))),
                                )
                                .child(
                                    canvas(
                                        |_, _, _| (),
                                        move |bounds, _, window, _| {
                                            let mut path = PathBuilder::fill();
                                            path.move_to(bounds.origin);
                                            path.line_to(bounds.top_right());
                                            path.line_to(bounds.bottom_left());
                                            path.close();
                                            if let Ok(path) = path.build() {
                                                window.paint_path(path, color);
                                            }
                                        },
                                    )
                                    .absolute()
                                    .top_full()
                                    .left(css(20.))
                                    .size(css(20.)),
                                )
                        })),
                )
                .into_any_element(),
        }
    }
    fn regions(&self) -> AnyElement {
        v_flex()
            .mt(css(30.))
            .text_center()
            .child(text("ALEXA_SUPPORT_REGIONS"))
            .child(
                h_flex()
                    .justify_center()
                    .gap(css(50.))
                    .mt(css(10.))
                    .mb(css(20.))
                    .child(
                        v_flex()
                            .text_left()
                            .child("English (US)")
                            .child("English (AU)")
                            .child("English (GB)")
                            .child("English (IN)")
                            .child("Deutsch (DE)"),
                    )
                    .child(
                        v_flex()
                            .text_left()
                            .child("Français (CA)")
                            .child("Español (US)")
                            .child("Español (ES)")
                            .child("Español (MX)")
                            .child("日本語 (JP)"),
                    ),
            )
            .child(text("ALEXA_SUPPORT_NOTE"))
            .into_any_element()
    }
}

impl Render for AlexaPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if let Some(state) = self.installer {
            self.install_page(state, window, cx)
        } else {
            match self.page {
                Page::Home => self.home(window, cx),
                Page::Skills => self.skills(window, cx),
                Page::Settings => self.settings(window, cx),
                Page::Help => self.help(cx),
            }
        };
        v_flex()
            .id("alexa-page")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family("Roboto")
            // The later body/html rule overrides the early 14px body reset.
            .text_size(css(16.))
            .line_height(relative(1.22))
            .when(self.preview, |view| {
                view.child(
                    h_flex()
                        .w_full()
                        .flex_shrink_0()
                        .px(css(20.))
                        .py(css(7.))
                        .gap(css(15.))
                        .flex_wrap()
                        .child(
                            surface::select(&self.scenes)
                                .id("alexa-preview-scene")
                                .items(scene_choices())
                                .w(css(285.))
                                .accessibility_label("Alexa 本地预览场景"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(css(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    if self.account == Account::Unknown
                                        && self.installer == Some(InstallState::Unknown)
                                    {
                                        "安装、账户及服务状态未知；登录与安装未连接。"
                                    } else {
                                        "本地界面预览：账户、验证码、设备及安装进度均为示例。"
                                    },
                                ),
                        ),
                )
            })
            .when(self.installer.is_none(), |view| {
                view.child(self.navigation(cx))
            })
            .child(
                div()
                    .id("alexa-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .scrollable_both()
                    .child(
                        v_flex()
                            .min_w(css(600.))
                            .w_full()
                            .pt(css(10.))
                            .px(css(20.))
                            .pb(css(20.))
                            .when(!self.notice.is_empty(), |view| {
                                view.child(
                                    div()
                                        .mb(css(10.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(self.notice.clone()),
                                )
                            })
                            .child(body),
                    ),
            )
            .when(self.modal.is_some(), |view| {
                view.child(self.modal_view(window, cx))
            })
    }
}

/// A separate page entity reuses the production body without changing its account state.
pub fn open_preview(window: &mut Window, cx: &mut App) {
    let page = cx.new(|cx| AlexaPage::new_preview(window, cx));
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("Alexa · 界面预览")
            .w((window.rem_size() * 80.)
                .min((window.viewport_size().width - window.rem_size() * 2.).max(px(0.))))
            .child(
                div()
                    .w_full()
                    .h((window.viewport_size().height - window.rem_size() * 8.).max(px(0.)))
                    .child(page.clone()),
            )
    });
}
