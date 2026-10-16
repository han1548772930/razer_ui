//! Current Armory 24988 / 13784 PROFILE form, opened with an actual local
//! device snapshot. No contribution UUID, author, download or success is made up.
use gpui_kit::base::{Button, Dialog, DialogPopup};
use gpui_kit::component::{
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_model::model::DeviceCategory;
use razer_model::model::Profile;
use razer_pages::features::Choice;
use razer_widgets::surface;
use std::path::PathBuf;

const TITLE_LIMIT: usize = 32;
const DESCRIPTION_LIMIT: usize = 500;
const GAME_LIMIT: usize = 10;

fn text(key: &str) -> String {
    i18n::t(key)
}

fn local_text(zh: &str, en: &str) -> String {
    if i18n::locale().to_ascii_lowercase().starts_with("zh") {
        zh.into()
    } else {
        en.into()
    }
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

fn has_title(value: &str) -> bool {
    // The source uses JavaScript trim(), whose whitespace differs from Rust.
    value.chars().any(|ch| !matches!(ch, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
}

fn clip_utf16(value: &str, limit: usize) -> String {
    let mut used = 0;
    value
        .chars()
        .take_while(|ch| {
            used += ch.len_utf16();
            used <= limit
        })
        .collect()
}

#[derive(Clone)]
struct SupportedGame {
    path: PathBuf,
    name: String,
}

pub struct ShareProfileDialog {
    device: Device,
    device_select: Entity<SelectState<Vec<Choice>>>,
    profile_select: Entity<SelectState<Vec<Choice>>>,
    title: Entity<InputState>,
    description: Entity<TextareaState>,
    games: Vec<SupportedGame>,
    focus: FocusHandle,
    unavailable: bool,
    picker_pending: bool,
    picker_task: Option<Task<()>>,
    picker_error: Option<String>,
    subscriptions: Vec<Subscription>,
}

pub struct ShareClosed;
impl EventEmitter<ShareClosed> for ShareProfileDialog {}

impl ShareProfileDialog {
    pub fn new(device: Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let eligible = !device.is_single_profile;
        let devices = if eligible {
            vec![Choice::new(
                device.device_container_id.clone(),
                device.display_name(),
            )]
        } else {
            Vec::new()
        };
        let profiles = if eligible {
            device
                .profiles
                .iter()
                .map(|profile| Choice::new(&profile.id, &profile.name))
                .collect()
        } else {
            Vec::new()
        };
        let selected_profile = if eligible {
            device
                .profiles
                .iter()
                .position(|profile| profile.id == device.active_profile)
                .map(IndexPath::new)
        } else {
            None
        };
        let device_select = cx
            .new(|cx| SelectState::new(devices, eligible.then_some(IndexPath::new(0)), window, cx));
        let profile_select = cx.new(|cx| SelectState::new(profiles, selected_profile, window, cx));
        let title =
            cx.new(|cx| InputState::new(window, cx).placeholder(text("AWESOME_CONTENT_TITLE")));
        let description =
            cx.new(|cx| TextareaState::new(window, cx).placeholder(text("ADD_DETAILS")));
        let mut this = Self {
            device,
            device_select,
            profile_select,
            title,
            description,
            games: Vec::new(),
            focus: cx.focus_handle(),
            unavailable: false,
            picker_pending: false,
            picker_task: None,
            picker_error: None,
            subscriptions: Vec::new(),
        };
        this.subscriptions.push(cx.subscribe_in(
            &this.title,
            window,
            |_, input, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value();
                    if utf16_len(&value) > TITLE_LIMIT {
                        input.update(cx, |state, cx| {
                            state.set_value(clip_utf16(&value, TITLE_LIMIT), window, cx)
                        });
                    }
                }
                // The parent draws the focus border, so focus changes must
                // invalidate it as well as value changes.
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.description,
            window,
            |_, input, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value();
                    if utf16_len(&value) > DESCRIPTION_LIMIT {
                        input.update(cx, |state, cx| {
                            state.set_value(clip_utf16(&value, DESCRIPTION_LIMIT), window, cx)
                        });
                    }
                }
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.profile_select,
            window,
            |this, _, event, _, cx| {
                if matches!(event, SelectEvent::Confirm(_)) {
                    this.unavailable = false;
                    cx.notify();
                }
            },
        ));
        this
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    fn selected_profile(&self, cx: &App) -> Option<&Profile> {
        let id = self.profile_select.read(cx).selected_value()?;
        self.device
            .profiles
            .iter()
            .find(|profile| &profile.id == id)
    }

    fn can_submit(&self, cx: &App) -> bool {
        // 24988 M: PROFILE requires a selected device/profile and a nonblank
        // title. Online toxicity checks are unavailable, never marked passed.
        self.device_select.read(cx).selected_value().is_some()
            && self.selected_profile(cx).is_some()
            && has_title(&self.title.read(cx).value())
            && !self.picker_pending
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.can_submit(cx) {
            // No Armory account, moderation call, contribution POST or success.
            self.unavailable = true;
            cx.notify();
        }
    }

    fn browse_games(&mut self, cx: &mut Context<Self>) {
        if self.picker_pending
            || self.games.len() >= GAME_LIMIT
            || self.selected_profile(cx).is_none()
        {
            return;
        }
        self.picker_pending = true;
        self.picker_error = None;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(text("ADD_GAME").into()),
        });
        self.picker_task = Some(cx.spawn(async move |this, cx| {
            let result = picker.await;
            let _ = this.update(cx, |this, cx| {
                this.picker_pending = false;
                this.picker_task = None;
                match result {
                    Ok(Ok(Some(paths))) => {
                        for path in paths {
                            if this.games.len() >= GAME_LIMIT {
                                break;
                            }
                            let extension = path
                                .extension()
                                .and_then(|value| value.to_str())
                                .unwrap_or("");
                            if !extension.eq_ignore_ascii_case("exe")
                                && !extension.eq_ignore_ascii_case("url")
                            {
                                this.picker_error = Some(local_text(
                                    "请选择 .exe 或 .url 文件。",
                                    "Choose an .exe or .url file.",
                                ));
                                continue;
                            }
                            if this.games.iter().any(|game| {
                                game.path
                                    .as_os_str()
                                    .to_string_lossy()
                                    .eq_ignore_ascii_case(&path.as_os_str().to_string_lossy())
                            }) {
                                continue;
                            }
                            let name = path
                                .file_stem()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            if !name.is_empty() {
                                this.games.push(SupportedGame { path, name });
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        this.picker_error = Some(local_text(
                            "无法打开文件选择器，请重试。",
                            "Could not open the file picker. Try again.",
                        ))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn label(key: &str, cx: &App) -> AnyElement {
        div()
            .text_size(surface::css(12.))
            .text_color(cx.theme().muted_foreground)
            .child(text(key))
            .into_any_element()
    }

    fn profile_preview(&self, cx: &App) -> AnyElement {
        let Some(profile) = self.selected_profile(cx) else {
            return div().into_any_element();
        };
        // 13784 mounts 90516 with hideProfileInfo once a device is selected.
        // Preview only parameters present in this snapshot; never copy 93771's
        // example contribution identity, author, counts or hardcoded 3KB size.
        let mut dpi: Vec<(u32, u32)> = profile
            .dpi_stages
            .as_ref()
            .map(|stages| {
                stages
                    .stages
                    .iter()
                    .map(|stage| (stage.x, stage.y))
                    .collect()
            })
            .unwrap_or_default();
        if self.device.category == DeviceCategory::Mouse {
            if let Some(settings) = profile
                .settings
                .as_ref()
                .and_then(|settings| serde_json::to_value(settings).ok())
            {
                if let Some(stages) = settings
                    .pointer("/sensitivity/stages")
                    .and_then(|stages| stages.as_array())
                {
                    dpi = stages
                        .iter()
                        .filter_map(|stage| {
                            Some((
                                u32::try_from(stage.get(0)?.as_u64()?).ok()?,
                                u32::try_from(stage.get(1)?.as_u64()?).ok()?,
                            ))
                        })
                        .collect();
                }
            }
        }
        v_flex()
            .id("armory-local-profile-preview")
            .w_full()
            .gap(surface::css(10.))
            .mt(surface::css(20.))
            .p(surface::css(20.))
            .border_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .child(text("PROFILE_DETAILS")),
            )
            .child(div().child(format!("{} · {}", self.device.display_name(), profile.name)))
            .when(!dpi.is_empty(), |view| {
                view.child(
                    v_flex()
                        .gap(surface::css(8.))
                        .child(Self::label("SENSITIVITY", cx))
                        .children(dpi.into_iter().enumerate().map(|(index, (x, y))| {
                            h_flex()
                                .id(("armory-preview-dpi", index))
                                .gap(surface::css(15.))
                                .child(format!("{}", index + 1))
                                .child(if x == y {
                                    format!("{x} DPI")
                                } else {
                                    format!("X {x}  Y {y}")
                                })
                        })),
                )
            })
            .into_any_element()
    }
}

impl Render for ShareProfileDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = (f32::from(window.viewport_size().width) * 0.8).clamp(800., 1020.);
        let available_height = (f32::from(window.viewport_size().height) - 128.).max(200.);
        let profile_choices = self
            .device
            .profiles
            .iter()
            .map(|profile| Choice::new(&profile.id, &profile.name))
            .collect();
        let title = Input::new(&self.title)
            .appearance(false)
            .aria_label(text("TITLE"))
            .h(surface::css(23.))
            .w_full()
            .px(surface::css(5.))
            .border_1()
            .border_color(if self.title.focus_handle(cx).is_focused(window) {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .bg(cx.theme().input);
        let description = Textarea::new(&self.description)
            .appearance(false)
            .aria_label(text("DESCRIPTION_OPTIONAL"))
            .h(surface::css(91.))
            .w_full()
            .px(surface::css(5.))
            .border_1()
            .border_color(if self.description.focus_handle(cx).is_focused(window) {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .bg(cx.theme().input);
        let form = v_flex()
            .id("armory-share-form")
            .w(surface::css(800.))
            .max_w_full()
            .p(surface::css(20.))
            .gap(surface::css(15.))
            .bg(cx.theme().input)
            .rounded(surface::css(5.))
            .child(
                h_flex()
                    .w_full()
                    .gap(surface::css(15.))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(surface::css(5.))
                            .child(Self::label("FALLBACK_DEVICE_PROMPT", cx))
                            .child(
                                surface::select(&self.device_select)
                                    .id("armory-share-device")
                                    .items(if self.device.is_single_profile {
                                        Vec::new()
                                    } else {
                                        vec![Choice::new(
                                            self.device.device_container_id.clone(),
                                            self.device.display_name(),
                                        )]
                                    })
                                    .placeholder(text("DEVICE_PLACEHOLDER"))
                                    .accessibility_label(text("FALLBACK_DEVICE_PROMPT"))
                                    .w_full(),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(surface::css(5.))
                            .child(Self::label("SELECT_PROFILE", cx))
                            .child(
                                surface::select(&self.profile_select)
                                    .id("armory-share-profile")
                                    .items(profile_choices)
                                    .placeholder(text("PROFILE_PLACEHOLDER"))
                                    .accessibility_label(text("SELECT_PROFILE"))
                                    .disabled(
                                        self.device_select.read(cx).selected_value().is_none(),
                                    )
                                    .w_full(),
                            ),
                    ),
            )
            .child(div().h(px(1.)).bg(cx.theme().border))
            .child(
                v_flex()
                    .gap(surface::css(5.))
                    .child(Self::label("TITLE", cx))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(text("HELPER_TEXT")),
                    )
                    .child(title)
                    .child(
                        div()
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{}/32", utf16_len(&self.title.read(cx).value()))),
                    ),
            )
            .child(
                v_flex()
                    .gap(surface::css(5.))
                    .child(Self::label("DESCRIPTION_OPTIONAL", cx))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(text("DESCRIPTION_INPUT_PLACEHOLDER")),
                    )
                    .child(description)
                    .child(
                        div()
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{}/500",
                                utf16_len(&self.description.read(cx).value())
                            )),
                    ),
            )
            .child(
                v_flex()
                    .gap(surface::css(5.))
                    .mt(surface::css(30.))
                    .child(Self::label("GAMES_SUPPORTED_OPTIONAL", cx))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(surface::css(8.))
                            .children(self.games.iter().map(|game| {
                                let path = game.path.clone();
                                Button::new(SharedString::from(format!(
                                    "armory-game-{}",
                                    game.path.display()
                                )))
                                .accessibility_label(format!("{} {}", text("REMOVE"), game.name))
                                .px(surface::css(8.))
                                .h(surface::css(32.))
                                .border_1()
                                .border_color(cx.theme().border)
                                .focus_visible(|style| style.border_color(cx.theme().primary))
                                .child(format!("{} ×", game.name))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.games.retain(|game| game.path != path);
                                        cx.notify();
                                    },
                                ))
                            }))
                            .child(
                                Button::new("armory-add-supported-game")
                                    .accessibility_label(text("ADD_GAME"))
                                    .size(surface::css(32.))
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .focus_visible(|style| style.border_color(cx.theme().primary))
                                    .disabled(
                                        self.selected_profile(cx).is_none()
                                            || self.games.len() >= GAME_LIMIT
                                            || self.picker_pending,
                                    )
                                    .styles(|styles| styles.disabled(|style| style.opacity(0.3)))
                                    .child("+")
                                    .on_click(cx.listener(|this, _, _, cx| this.browse_games(cx))),
                            ),
                    ),
            )
            .when_some(self.picker_error.clone(), |view, error| {
                view.child(div().text_color(cx.theme().danger).child(error))
            });
        let size = self
            .selected_profile(cx)
            .and_then(|profile| serde_json::to_vec(profile).ok())
            .map(|bytes| format!("{} B", bytes.len()))
            .unwrap_or_else(|| "—".into());
        let panel = v_flex().id("armory-share-popup").w(px(width)).max_w_full().max_h(px(available_height))
            .bg(cx.theme().background).text_color(cx.theme().foreground).text_size(surface::css(14.))
            .child(h_flex().relative().w_full().h(surface::css(36.)).flex_shrink_0().justify_center().items_center()
                .border_b_1().border_color(cx.theme().border).font_family("RazerF5").text_color(cx.theme().muted_foreground)
                .child(text("SHARE_NEW"))
                .child(Button::new("armory-share-close").absolute().right_0().top_0().size(surface::css(36.))
                    .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
                    .accessibility_label(text("CLOSE")).child("×").on_click(cx.listener(|_, _, _, cx| cx.emit(ShareClosed)))))
            .child(v_flex().id("armory-share-body").w_full().min_h_0().flex_1().overflow_y_scroll()
                .items_center().p(surface::css(20.)).child(form).child(self.profile_preview(cx))
                .when(self.unavailable, |view| view.child(div().id("armory-share-unavailable")
                    .w_full().mt(surface::css(15.)).text_color(cx.theme().warning)
                    .child(local_text("分享服务未连接。草稿已保留，尚未提交。", "Sharing is unavailable. Your draft is retained and has not been submitted.")))))
            .child(h_flex().w_full().flex_shrink_0().gap(surface::css(15.)).justify_center().items_center()
                .py(surface::css(9.)).border_t_1().border_color(cx.theme().border)
                .child(Button::new("armory-share-submit").accessibility_label(text("SUBMIT"))
                    .focus_visible(|style| style.border_1().border_color(cx.theme().foreground))
                    .w(surface::css(100.)).h(surface::css(27.)).bg(cx.theme().primary).text_color(cx.theme().primary_foreground)
                    .disabled(!self.can_submit(cx)).styles(|styles| styles.disabled(|style| style.opacity(0.3)))
                    .child(text("SUBMIT")).on_click(cx.listener(|this, _, _, cx| this.submit(cx))))
                .child(div().text_color(cx.theme().muted_foreground).child(format!("{} · {}: {size}", local_text("本地草稿", "Local draft"), text("FILE_SIZE")))));
        let share_view = cx.entity().downgrade();
        Dialog::new(cx)
            .layer(3, true)
            .focus_handle(self.focus.clone())
            .items_start()
            .pt(surface::css(108.))
            .close_on_backdrop_press(false)
            .on_ok(move |_, _, cx| {
                let _ = share_view.update(cx, |this, cx| this.submit(cx));
                false
            })
            .on_close(cx.listener(|_, _, _, cx| cx.emit(ShareClosed)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(cx.theme().background.opacity(0.8)),
            )
            .popup(DialogPopup::new().max_w_full().child(panel))
    }
}
