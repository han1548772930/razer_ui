//! Current /chroma-app/settings/ 1743: Zo -> mi/vi; source receipts are generated.
//! This entity owns local Chroma intent, not host autostart or device ownership.
use crate::shell::{
    release_notes,
    settings_page::{SettingsEvent, SettingsPage},
};
use crate::{
    features::Choice,
    i18n,
    ui::{scroll::SourceScrollable as _, surface::css},
};
use gpui_kit::base::{Button, Checkbox, CheckboxState, Link};
use gpui_kit::component::{
    select::{SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};
#[path = "settings_button.rs"]
mod source_button;
use crate::ui::theme::ChromaSettingsColors as Colors;

#[derive(Deserialize)]
struct Language {
    name: String,
    code: String,
}
#[derive(Deserialize)]
struct Social {
    name: String,
    link: String,
    label: String,
}
#[derive(Deserialize)]
struct Source {
    translations: BTreeMap<String, BTreeMap<String, String>>,
    languages: Vec<Language>,
    #[serde(rename = "tutorialKeys")]
    tutorial_keys: Vec<String>,
    version: String,
    socials: Vec<Social>,
}
fn source() -> &'static Source {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    SOURCE.get_or_init(|| {
        serde_json::from_str(include_str!("chroma_settings_data.json"))
            .expect("audited Chroma Settings")
    })
}
pub(super) fn text(key: &str) -> String {
    source()
        .translations
        .get(&i18n::locale().to_lowercase())
        .and_then(|t| t.get(key))
        .or_else(|| source().translations["en"].get(key))
        .cloned()
        .unwrap_or_else(|| key.into())
}
fn local(zh: &str, en: &str) -> String {
    if i18n::locale().starts_with("zh") {
        zh.into()
    } else {
        en.into()
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LocalDraft {
    auto_start: Option<bool>,
    start_minimized: Option<bool>,
    notifications: Option<bool>,
    dynamic_lighting: Option<bool>,
    tutorials: BTreeMap<String, bool>,
}
impl LocalDraft {
    fn path() -> PathBuf {
        crate::store::store_path().with_file_name("chroma-settings-local.json")
    }
    fn load() -> Result<Self, String> {
        match std::fs::read(Self::path()) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save(&self) -> Result<(), String> {
        let p = Self::path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(p, bytes).map_err(|e| e.to_string())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Chroma,
    General,
}
impl Page {
    fn key(self) -> &'static str {
        match self {
            Self::Chroma => "CHROMA_APP",
            Self::General => "GENERAL",
        }
    }
}
pub(super) enum SettingsAction {
    ResetTutorials,
    OpenMigration,
}
pub(super) struct ChromaSettings {
    page: Page,
    history: Vec<Page>,
    history_ix: usize,
    focus: FocusHandle,
    window: AnyWindowHandle,
    locale_owner: Entity<SettingsPage>,
    dashboard: Entity<crate::shell::chroma_page::ChromaPage>,
    language: Entity<SelectState<Vec<Choice>>>,
    draft: LocalDraft,
    saved_draft: LocalDraft,
    error: Option<String>,
    dynamic_supported: bool,
    dynamic_switching: bool,
    dualsense_supported: bool,
    dualsense_present: bool,
    notes: Option<Entity<release_notes::ReleaseNotes>>,
    notes_subscription: Option<Subscription>,
    unsaved: bool,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<SettingsAction> for ChromaSettings {}
impl ChromaSettings {
    pub(super) fn new(
        locale_owner: Entity<SettingsPage>,
        dashboard: Entity<crate::shell::chroma_page::ChromaPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let choices = source()
            .languages
            .iter()
            .map(|l| Choice::new(&l.code, l.name.clone()))
            .collect::<Vec<_>>();
        let language = cx.new(|cx| SelectState::new(choices, None, window, cx));
        language.update(cx, |s, cx| {
            s.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
        });
        let (draft, error) = match LocalDraft::load() {
            Ok(d) => (d, None),
            Err(e) => (LocalDraft::default(), Some(e)),
        };
        let subscriptions = vec![
            cx.observe(&dashboard, |_, _, cx| cx.notify()),
            cx.subscribe(&locale_owner, |this: &mut Self, _, event, cx| {
                if matches!(event, SettingsEvent::Language) {
                    let handle = this.window;
                    let owner = cx.entity().downgrade();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.language.update(cx, |s, cx| {
                                    s.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
                                });
                                cx.notify();
                            });
                        });
                    });
                }
            }),
            cx.subscribe(&language, |this: &mut Self, _, event, cx| {
                if let SelectEvent::Confirm(Some(code)) = event {
                    let handle = this.window;
                    let locale = this.locale_owner.clone();
                    let code = code.clone();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            locale.update(cx, |s, cx| s.set_language(&code, window, cx))
                        });
                    });
                }
            }),
        ];
        Self {
            page: Page::Chroma,
            history: vec![Page::Chroma],
            history_ix: 0,
            focus: cx.focus_handle(),
            window: window.window_handle(),
            locale_owner,
            dashboard,
            language,
            saved_draft: draft.clone(),
            draft,
            error,
            dynamic_supported: crate::backend::system::supports_dynamic_lighting(),
            dynamic_switching: false,
            dualsense_supported: false,
            dualsense_present: false,
            notes: None,
            notes_subscription: None,
            unsaved: false,
            _subscriptions: subscriptions,
        }
    }
    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        if self
            .notes
            .as_ref()
            .is_some_and(|notes| notes.update(cx, |notes, cx| notes.focus_if_open(window, cx)))
        {
            return;
        }
        self.focus.focus(window, cx);
    }
    pub(super) fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.unsaved {
            self.error = Some(local(
                "本地更改尚未保存。可重试保存，或放弃未保存更改后关闭。",
                "Local changes are unsaved. Retry saving or discard unsaved changes before closing.",
            ));
            cx.notify();
        }
        !self.unsaved
    }
    pub(super) fn bind_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window = window.window_handle();
        self.notes = None;
        self.notes_subscription = None;
        self.language.update(cx, |s, cx| {
            s.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
        });
    }
    fn toggle(
        cx: &Context<Self>,
        action: fn(&mut Self, bool, &mut Context<Self>),
    ) -> impl Fn(CheckboxState, &ClickEvent, &mut Window, &mut App) + 'static {
        let owner = cx.entity().downgrade();
        move |state, _, _, cx| {
            let _ = owner.update(cx, |this, cx| {
                action(this, state == CheckboxState::Checked, cx)
            });
        }
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        self.error = self.draft.save().err();
        self.unsaved = self.error.is_some();
        if !self.unsaved {
            self.saved_draft = self.draft.clone();
        }
        cx.notify();
    }
    fn discard(&mut self, cx: &mut Context<Self>) {
        self.draft = self.saved_draft.clone();
        self.unsaved = false;
        self.error = None;
        cx.notify();
    }
    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        if page == self.page {
            return;
        }
        self.history.truncate(self.history_ix + 1);
        self.history.push(page);
        self.history_ix += 1;
        self.page = page;
        cx.notify();
    }
    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let next = if forward {
            self.history_ix.checked_add(1)
        } else {
            self.history_ix.checked_sub(1)
        };
        if let Some(ix) = next.filter(|ix| *ix < self.history.len()) {
            self.history_ix = ix;
            self.page = self.history[ix];
            cx.notify();
        }
    }
    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.unsaved {
            self.error = Some(local(
                "本地更改尚未保存，请先重试保存",
                "Local changes are unsaved. Retry saving before refreshing.",
            ));
            cx.notify();
            return;
        }
        match LocalDraft::load() {
            Ok(d) => {
                self.saved_draft = d.clone();
                self.draft = d;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
        cx.notify();
    }
    /// Accept only the current host's WDL/storage and Sensa feature observations.
    /// Local checkbox drafts never supply these service facts.
    #[allow(dead_code)]
    pub(super) fn observe(
        &mut self,
        switching: bool,
        dualsense_supported: bool,
        dualsense_present: bool,
        cx: &mut Context<Self>,
    ) {
        self.dynamic_switching = switching;
        self.dualsense_supported = dualsense_supported;
        self.dualsense_present = dualsense_present;
        cx.notify();
    }
    fn reset_tutorials(&mut self, cx: &mut Context<Self>) {
        if self.tutorials_reset(cx) {
            return;
        }
        for key in &source().tutorial_keys {
            self.draft.tutorials.insert(key.clone(), true);
        }
        self.save(cx);
        cx.emit(SettingsAction::ResetTutorials);
    }
    fn tutorials_reset(&self, cx: &App) -> bool {
        source().tutorial_keys.iter().take(3).all(|key| {
            if key == "isShowIntroductionBanner" {
                self.dashboard.read(cx).introduction_enabled()
            } else {
                self.draft.tutorials.get(key) == Some(&true)
            }
        })
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let navigation = h_flex().w(relative(0.2)).children(
            [
                ("chroma-settings-back", "BACK", "back", self.history_ix == 0),
                (
                    "chroma-settings-forward",
                    "FORWARD",
                    "forward",
                    self.history_ix + 1 >= self.history.len(),
                ),
                ("chroma-settings-refresh", "REFRESH", "refresh", false),
            ]
            .map(|(id, label, asset, disabled)| {
                Button::new(id)
                    .accessibility_label(text(label))
                    .disabled(disabled)
                    .p_0()
                    .w(css(40.))
                    .h(css(38.))
                    .when(disabled, |b| b.opacity(0.3))
                    .when(!disabled, |b| b.hover(|s| s.bg(Colors::hover())))
                    .focus_visible(|s| s.border_1().border_color(Colors::selected()))
                    .child(
                        img(SharedString::from(format!(
                            "synapse/chroma-settings-{asset}.svg"
                        )))
                        .size(css(20.)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| match asset {
                        "back" => this.step(false, cx),
                        "forward" => this.step(true, cx),
                        _ => this.reload(cx),
                    }))
            }),
        );
        h_flex()
            .h(css(40.))
            .flex_shrink_0()
            .bg(Colors::background())
            .child(navigation)
            .child(
                div()
                    .flex_1()
                    .text_center()
                    .text_size(css(14.))
                    .text_color(Colors::secondary())
                    .child(text("SETTINGS_HEADER").to_uppercase()),
            )
            .child(div().w(relative(0.2)))
            .into_any_element()
    }
    fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(css(48.))
            .border_b(css(2.))
            .border_color(Colors::tooltip())
            .flex_shrink_0()
            .justify_center()
            .gap(css(20.))
            .children([Page::Chroma, Page::General].map(|page| {
                let selected = page == self.page;
                Button::new(SharedString::from(format!(
                    "chroma-settings-nav-{}",
                    page.key()
                )))
                .accessibility_label(text(page.key()))
                .selected(selected)
                .px(css(10.))
                .py(css(7.))
                .rounded(css(14.))
                .text_size(css(12.))
                .line_height(css(14.))
                .when(selected, |b| {
                    b.bg(Colors::selected()).text_color(Colors::panel())
                })
                .when(!selected, |b| {
                    b.text_color(Colors::secondary())
                        .hover(|s| s.bg(Colors::hover()).text_color(Colors::text()))
                })
                .focus_visible(|s| s.border_1().border_color(Colors::selected()))
                .child(text(page.key()).to_uppercase())
                .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
            }))
            .into_any_element()
    }
    fn startup(&self, cx: &mut Context<Self>) -> AnyElement {
        let auto = self.draft.auto_start.unwrap_or(true);
        let minimized = self.draft.start_minimized.unwrap_or(true);
        panel("AUTO_LAUNCH")
            .child(
                checkbox(
                    "chroma-settings-autostart",
                    text("START_CHROMA_APP"),
                    auto,
                    false,
                )
                .on_change(Self::toggle(cx, |this, value, cx| {
                    this.draft.auto_start = Some(value);
                    this.save(cx);
                })),
            )
            .child(
                v_flex()
                    .relative()
                    .pl(css(30.))
                    .child(
                        div()
                            .absolute()
                            .left(css(9.))
                            .top_0()
                            .w(css(1.))
                            .h(css(37.))
                            .bg(Colors::note()),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(css(10.))
                            .top(css(36.))
                            .w(css(13.))
                            .h(css(1.))
                            .bg(Colors::note()),
                    )
                    .child(
                        div()
                            .mb(css(10.))
                            .text_color(Colors::note())
                            .child(text("NOTE_DISABLE_CHROMA_APP")),
                    )
                    .child(
                        checkbox(
                            "chroma-settings-minimized",
                            text("MINIMIZE_SYSTRAY"),
                            minimized,
                            !auto,
                        )
                        .on_change(Self::toggle(cx, |this, value, cx| {
                            if this.draft.auto_start.unwrap_or(true) {
                                this.draft.start_minimized = Some(value);
                                this.save(cx);
                            }
                        })),
                    ),
            )
            .into_any_element()
    }
    fn notifications(&self, cx: &mut Context<Self>) -> AnyElement {
        let tip = format!(
            "{}\n• {}\n• {}\n• {}",
            text("NOTIFICATIONS_TOOLTIP"),
            text("NOTIFICATIONS_TOOLTIP_DESC1"),
            text("NOTIFICATIONS_TOOLTIP_DESC2"),
            text("NOTIFICATIONS_TOOLTIP_DESC3")
        );
        panel("NOTIFICATIONS")
            .child(
                h_flex()
                    .gap(css(10.))
                    .child(
                        checkbox(
                            "chroma-settings-notifications",
                            text("DISPLAY_NOTIFICATIONS"),
                            self.draft.notifications.unwrap_or(false),
                            false,
                        )
                        .on_change(Self::toggle(cx, |this, value, cx| {
                            this.draft.notifications = Some(value);
                            this.save(cx);
                        })),
                    )
                    .child(help("chroma-settings-notification-help", tip)),
            )
            .into_any_element()
    }
    fn lighting(&self, cx: &mut Context<Self>) -> AnyElement {
        let dynamic = self.draft.dynamic_lighting.unwrap_or(false);
        panel("DEVICE_LIGHTING")
            .relative()
            .child(
                help(
                    "chroma-settings-lighting-help",
                    text("DEVICE_LIGHTING_TIPS"),
                )
                .absolute()
                .right(css(10.))
                .top(css(10.)),
            )
            .child(
                h_flex()
                    .gap(css(14.))
                    .child(
                        h_flex()
                            .h(css(36.))
                            .p(css(5.))
                            .border_1()
                            .border_color(Colors::border())
                            .rounded(css(18.))
                            .children([(false, "CHROMA_RGB"), (true, "DYNAMIC_LIGHTING")].map(
                                |(value, label)| {
                                    Button::new(SharedString::from(format!(
                                        "chroma-settings-lighting-mode-{label}"
                                    )))
                                    .accessibility_label(text(label))
                                    .selected(dynamic == value)
                                    .disabled(self.dynamic_switching)
                                    .h(css(26.))
                                    .px(css(10.))
                                    .py_0()
                                    .rounded(css(13.))
                                    .text_size(css(12.))
                                    .when(dynamic == value, |b| {
                                        b.bg(Colors::selected()).text_color(Colors::panel())
                                    })
                                    .when(self.dynamic_switching, |b| b.opacity(0.3))
                                    .focus_visible(|s| {
                                        s.border_1().border_color(Colors::selected())
                                    })
                                    .child(text(label))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.dynamic_switching {
                                            this.draft.dynamic_lighting = Some(value);
                                            this.save(cx);
                                        }
                                    }))
                                },
                            )),
                    )
                    .when(self.dynamic_switching, |r| {
                        r.child(spinner::Spinner::new().small())
                    }),
            )
            .child(div().pt(css(20.)).child(text(if dynamic {
                "DYNAMIC_LIGHTING_TITLE"
            } else {
                "CHROMA_RGB_TITLE"
            })))
            .child(
                h_flex()
                    .items_start()
                    .mt(css(10.))
                    .gap(css(10.))
                    .child(
                        img("synapse/chroma-settings-warning.svg")
                            .size(css(14.))
                            .mt(css(3.))
                            .flex_shrink_0(),
                    )
                    .child(text(if dynamic {
                        "DYNAMIC_LIGHTING_MSG"
                    } else {
                        "CHROMA_RGB_MSG"
                    })),
            )
            .child(
                h_flex()
                    .mt(css(20.))
                    .gap(css(20.))
                    .child(
                        img("synapse/chroma-settings-wdl.svg")
                            .size(css(44.))
                            .flex_shrink_0(),
                    )
                    .child(policy(
                        "chroma-settings-open-wdl",
                        text("OPEN_WINDOWS_DYNAMIC_LIGHTING"),
                        "ms-settings:personalization-lighting",
                    )),
            )
            .into_any_element()
    }
    fn chroma(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let reset = self.tutorials_reset(cx);
        let left = v_flex()
            .child(self.startup(cx))
            .child(self.notifications(cx))
            .into_any_element();
        let right = v_flex()
            .child(
                panel("TUTORIAL_RESET").child(
                    h_flex()
                        .gap(css(20.))
                        .child(
                            source_button::settings_button(
                                "chroma-settings-reset",
                                text("RESET"),
                                reset,
                                window,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.reset_tutorials(cx))),
                        )
                        .child(div().flex_1().child(text("CHROMA_APP_TUTORIAL_RESET_MSG"))),
                ),
            )
            .child(
                panel("PROFILE_MIGRATION").child(
                    h_flex()
                        .gap(css(20.))
                        .child(
                            source_button::settings_button(
                                "chroma-settings-migration",
                                text("LAUNCH"),
                                false,
                                window,
                                cx,
                            )
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(SettingsAction::OpenMigration)),
                            ),
                        )
                        .child(div().flex_1().child(text("PROFILE_MIGRATION_DESC"))),
                ),
            )
            .when(self.dynamic_supported, |v| v.child(self.lighting(cx)))
            .into_any_element();
        columns(window, left, right)
    }
    fn open_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let notes = release_notes::open_for(release_notes::NotesApp::Chroma, window, cx);
        self.notes_subscription = Some(cx.observe(&notes, |_, _, cx| cx.notify()));
        self.notes = Some(notes);
        cx.notify();
    }
    fn general(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        columns(
            window,
            v_flex()
                .child(panel("LANGUAGE").child(crate::ui::surface::select(&self.language).w_full()))
                .child(
                    panel("THX_WHAT_NEWS").child(
                        Button::new("chroma-settings-release-notes")
                            .accessibility_label(
                                text("RELEASE_PATCH_NOTE_WHATS_NEW")
                                    .replace("{{releasePatchNote}}", "Release Notes"),
                            )
                            .p_0()
                            .text_size(css(14.))
                            .text_color(Colors::text())
                            .text_decoration_1()
                            .hover(|s| s.text_color(Colors::selected()))
                            .focus_visible(|s| s.border_1().border_color(Colors::selected()))
                            .child(
                                text("RELEASE_PATCH_NOTE_WHATS_NEW")
                                    .replace("{{releasePatchNote}}", "Release Notes"),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_notes(window, cx)),
                            ),
                    ),
                )
                .into_any_element(),
            self.about(cx),
        )
    }
    fn about(&self, _cx: &mut Context<Self>) -> AnyElement {
        panel("ABOUT")
            .child(div().flex().justify_center().mb(css(20.)).child(img("synapse/chroma-settings-logo.svg").w(css(400.)).h(css(400.*70./392.)).object_fit(ObjectFit::Contain)))
            .child(div().text_center().mb(css(20.)).text_color(Colors::secondary()).child(text("VERSION").replace("{{number}}",&source().version)))
            .child(div().text_center().text_color(Colors::secondary()).child(text("COPYRIGHT").replace("{{year}}",&chrono::Local::now().format("%Y").to_string())))
            .when(self.dualsense_supported&&self.dualsense_present,|p|p.child(div().mt(css(20.)).text_center().text_color(Colors::secondary()).child(text("PS_TRADEMARK"))))
            .child(div().mt(css(20.)).mb(css(20.)).text_center().text_color(Colors::secondary()).child(text("TRADEMARK")))
            .child(v_flex().items_center().mb(css(30.)).child(
                h_flex().flex_wrap().justify_center().children([
                    ("eula","LICENSE_TERMS_EULA","https://assets.razerzone.com/downloads/software/RazerSynapse3EndUserLicenseAgreement.pdf"),
                    ("oss","OPEN_SOURCE_SOFTWARE_NOTICE","https://support.razer.com/razer-synapse-3-open-source-software-notices"),
                    ("terms","TERMS_OF_NOTICE","https://www.razer.com/legal/terms-of-service"),
                ].map(|(id,key,url)|h_flex().child(policy(id,text(key),url)).child(div().px(css(5.)).text_color(Colors::secondary()).child("|")))))
                .child(policy("privacy",text("PRIVACY_POLICY"),"https://www.razer.com/legal/privacy-policy")))
            .child(div().mb(css(20.)).text_center().text_color(Colors::secondary()).child(text("CONNECT_WITH_US").to_uppercase()))
            .child(h_flex().justify_center().mb(css(10.)).child(social(source().socials.iter().find(|s|s.name=="insider").unwrap())))
            .child(h_flex().justify_center().gap(css(24.)).mb(css(10.)).children(source().socials.iter().filter(|s|s.name!="insider").map(social)))
            .into_any_element()
    }
}
fn panel(title: &str) -> Div {
    v_flex()
        .w(css(600.))
        .my(css(10.))
        .px(css(40.))
        .py(css(30.))
        .rounded(css(5.))
        .bg(Colors::panel())
        .text_size(css(14.))
        .line_height(css(17.))
        .text_color(Colors::text())
        .child(
            div()
                .mb(css(20.))
                .font_family("RazerF5")
                .text_size(css(18.))
                .text_color(Colors::selected())
                .child(text(title).to_uppercase()),
        )
}
fn columns(window: &Window, left: AnyElement, right: AnyElement) -> AnyElement {
    let narrow = window.viewport_size().width / window.rem_size() < 1279. / 16.;
    h_flex()
        .items_start()
        .flex_wrap()
        .justify_center()
        .max_w(css(1240.))
        .mx_auto()
        .child(
            div()
                .w(css(600.))
                .when(narrow, |v| v.mx(css(30.)))
                .child(left),
        )
        .child(
            div()
                .w(css(600.))
                .when(narrow, |v| v.mx(css(30.)))
                .child(right),
        )
        .into_any_element()
}
fn checkbox(id: &'static str, label: String, checked: bool, disabled: bool) -> Checkbox {
    Checkbox::new(id)
        .accessibility_label(label.clone())
        .checked(checked)
        .disabled(disabled)
        .group(id)
        .flex()
        .items_start()
        .min_h(css(20.))
        .gap(css(10.))
        .when(disabled, |c| c.opacity(0.3))
        .focus_visible(|s| s.bg(Colors::hover()))
        .child(
            div()
                .size(css(20.))
                .relative()
                .flex_shrink_0()
                .border_1()
                .rounded(css(2.4))
                .border_color(if checked {
                    Colors::selected()
                } else {
                    Colors::checkbox_border()
                })
                .when(checked, |v| {
                    v.bg(Colors::selected()).child(
                        img("synapse/chroma-settings-check.svg")
                            .absolute()
                            .top_0()
                            .left_0()
                            .size(css(20.)),
                    )
                })
                .when(!disabled, |v| {
                    v.group_hover(id, |s| s.border_color(Colors::selected()))
                }),
        )
        .child(
            div()
                .pt(css(2.))
                .text_size(css(14.))
                .line_height(css(17.))
                .child(label),
        )
}
fn help(id: &'static str, tip: String) -> Button {
    Button::new(id)
        .accessibility_label(tip.clone())
        .p_0()
        .size(css(14.))
        .rounded(css(7.))
        .bg(Colors::help())
        .hover(|s| s.bg(Colors::help_hover()))
        .focus_visible(|s| s.border_1().border_color(Colors::selected()))
        .child(img("synapse/chroma-settings-help.svg").size_full())
        .tooltip(move |window, cx| {
            Tooltip::new(tip.clone())
                .max_w(css(300.))
                .text_size(css(14.))
                .line_height(css(16.))
                .bg(Colors::tooltip())
                .border_color(Colors::border())
                .rounded_none()
                .shadow_none()
                .px(css(10.))
                .py(css(8.))
                .build(window, cx)
        })
}
fn policy(id: &'static str, label: String, url: &'static str) -> Link {
    Link::new(SharedString::from(format!("chroma-settings-policy-{id}")))
        .href(url)
        .open_with(|url, _, _, cx| cx.open_url(url))
        .accessibility_label(label.clone())
        .text_size(css(14.))
        .text_color(Colors::secondary())
        .text_decoration_1()
        .cursor_pointer()
        .hover(|s| s.text_color(Colors::selected()))
        .focus_visible(|s| s.border_1().border_color(Colors::selected()))
        .child(label)
}
fn social(spec: &Social) -> Link {
    let name = spec.name.clone();
    let label = text(&spec.label);
    let group = SharedString::from(format!("chroma-settings-social-{name}"));
    Link::new(group.clone())
        .href(spec.link.clone())
        .open_with(|url, _, _, cx| cx.open_url(url))
        .accessibility_label(label.clone())
        .group(group.clone())
        .relative()
        .w(css(if name == "insider" { 270. } else { 28. }))
        .h(css(if name == "insider" { 50. } else { 28. }))
        .flex_shrink_0()
        .child(
            img(SharedString::from(format!(
                "synapse/chroma-settings-{name}.svg"
            )))
            .size_full(),
        )
        .child(
            img(SharedString::from(format!(
                "synapse/chroma-settings-{name}-hover.svg"
            )))
            .size_full()
            .absolute()
            .inset_0()
            .opacity(0.)
            .group_hover(group, |s| s.opacity(1.)),
        )
        .focus_visible(|s| s.border_1().border_color(Colors::selected()))
        .tooltip(move |window, cx| {
            Tooltip::new(label.clone())
                .bg(Colors::tooltip())
                .border_color(Colors::border())
                .rounded_none()
                .shadow_none()
                .px(css(10.))
                .py(css(8.))
                .build(window, cx)
        })
}
impl Render for ChromaSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::Chroma => self.chroma(window, cx),
            Page::General => self.general(window, cx),
        };
        let content=v_flex().pt(css(10.)).px(css(20.)).pb(css(20.)).child(content)
            .child(div().id("chroma-settings-local-status").text_size(css(12.)).text_color(Colors::note()).text_center().child(local(
                "本地 Chroma 设置草稿；系统启动与设备灯光状态未读取，尚未应用到宿主。关于版本取自当前界面来源。",
                "Local Chroma settings draft. Startup and device lighting state have not been read or applied to the host. About shows the reference UI version.")))
            .when_some(self.error.clone(),|v,error|v.child(h_flex().gap(css(10.)).justify_center()
                .child(div().id("chroma-settings-storage-error").text_size(css(12.)).child(error))
                .child(Button::new("chroma-settings-retry").accessibility_label(local("重试保存","Retry save")).child(local("重试保存","Retry save"))
                    .on_click(cx.listener(|this,_,_,cx|this.save(cx))))));
        let content = content.when(self.unsaved, |v| {
            v.child(
                Button::new("chroma-settings-discard")
                    .accessibility_label(local(
                        "放弃未保存的本地更改",
                        "Discard unsaved local changes",
                    ))
                    .self_center()
                    .child(local(
                        "放弃未保存的本地更改",
                        "Discard unsaved local changes",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
            )
        });
        v_flex()
            .id("chroma-settings")
            .test_support()
            .aria_label(text("SETTINGS_HEADER"))
            .size_full()
            .min_w(css(600.))
            .font_family("Roboto")
            .bg(Colors::background())
            .text_color(Colors::text())
            .track_focus(&self.focus)
            .child(self.toolbar(cx))
            .child(self.navigation(cx))
            .child(
                div()
                    .id("chroma-settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(content),
            )
            .when_some(self.notes.clone(), |v, notes| v.child(notes))
    }
}
