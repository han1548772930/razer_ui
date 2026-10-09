//! Current per-product ImportExportModal. Vendor profile transfer remains
//! unavailable; selecting a path or profile never reports an import/export.
use super::*;
use gpui_kit::base::{Button as BaseButton, Dialog, DialogPopup};
use gpui_kit::component::checkbox::Checkbox;
use razer_i18n as i18n;
use serde::Deserialize;
use std::{path::PathBuf, sync::OnceLock};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProfileTransferMode {
    Import,
    Export,
}

#[derive(Deserialize)]
struct TransferSpec {
    width: f32,
    height: f32,
    top: f32,
    locale_keys: Vec<String>,
}

fn spec(pid: u32) -> &'static TransferSpec {
    #[derive(Deserialize)]
    struct ProductSpec {
        product_id: u32,
        import_export: TransferSpec,
    }
    #[derive(Deserialize)]
    struct Data {
        products: Vec<ProductSpec>,
    }
    static DATA: OnceLock<Data> = OnceLock::new();
    &DATA
        .get_or_init(|| {
            serde_json::from_str(include_str!("../source_profile_menu_data.json"))
                .expect("audited current profile transfer geometry")
        })
        .products
        .iter()
        .find(|product| product.product_id == pid)
        .expect("profile transfer is mounted only for audited products")
        .import_export
}

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

struct ProfileRow {
    profile: Profile,
    selected: bool,
}

pub(super) struct SourceProfileTransfer {
    pid: u32,
    mode: ProfileTransferMode,
    profiles: Vec<ProfileRow>,
    cloud: bool,
    select_all: bool,
    path: Option<PathBuf>,
    picker_pending: bool,
    picker_generation: u64,
    picker_task: Option<Task<()>>,
    picker_error: Option<String>,
    focus: FocusHandle,
    closed: bool,
}

pub(super) struct SourceProfileTransferClosed;
impl EventEmitter<SourceProfileTransferClosed> for SourceProfileTransfer {}

impl SourceProfileTransfer {
    pub(super) fn new(
        pid: u32,
        mode: ProfileTransferMode,
        profiles: Vec<Profile>,
        active: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let profiles = if mode == ProfileTransferMode::Export {
            profiles
                .into_iter()
                // The native model has no specsProfiles preset identity.
                // Never infer a factory preset from a user-editable name.
                .map(|profile| ProfileRow {
                    // Current 3886 selects every profile initially; the other
                    // seven select only selectedProfileGuid.
                    selected: pid == 3886 || profile.id == active,
                    profile,
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            pid,
            mode,
            profiles,
            cloud: false,
            // Source deselectAll starts false, regardless of selection count.
            select_all: false,
            path: None,
            picker_pending: false,
            picker_generation: 0,
            picker_task: None,
            picker_error: None,
            focus: cx.focus_handle(),
            closed: false,
        }
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.picker_generation += 1;
        self.picker_task = None;
        cx.emit(SourceProfileTransferClosed);
    }

    fn toggle_cloud(&mut self, cx: &mut Context<Self>) {
        self.cloud = !self.cloud;
        self.path = None;
        self.picker_error = None;
        self.profiles.clear();
        self.picker_generation += 1;
        cx.notify();
    }

    fn browse(&mut self, cx: &mut Context<Self>) {
        if self.mode != ProfileTransferMode::Import || self.cloud || self.picker_pending {
            return;
        }
        self.picker_pending = true;
        self.picker_error = None;
        let generation = self.picker_generation;
        // GPUI exposes no extension filter. Seven current products request
        // .synapse4; 3886 has no accept restriction. Contents stay unread.
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(if self.pid == 3886 {
                text("BROWSE_FOR_PROFILES").into()
            } else {
                format!("{} (.synapse4)", text("BROWSE_FOR_PROFILES")).into()
            }),
        });
        self.picker_task = Some(cx.spawn(async move |owner, cx| {
            let result = picker.await;
            let _ = owner.update(cx, |this, cx| {
                this.picker_pending = false;
                this.picker_task = None;
                if !this.closed && !this.cloud && this.picker_generation == generation {
                    match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.first() {
                                if this.pid == 3886 || path.extension().and_then(|part| part.to_str())
                                    .is_some_and(|extension| extension.eq_ignore_ascii_case("synapse4"))
                                {
                                    this.path = Some(path.clone());
                                } else {
                                    this.picker_error = Some(local_text(
                                        "请选择 .synapse4 文件。原路径已保留。",
                                        "Choose a .synapse4 file. The previous path is retained.",
                                    ));
                                }
                            }
                        }
                        Ok(Ok(None)) => {} // Native Cancel preserves the selected file.
                        _ => this.picker_error = Some(local_text(
                            "无法打开文件选择器。原路径已保留，请重试。",
                            "Could not open the file picker. The previous path is retained; try again.",
                        )),
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn unavailable(&self) -> String {
        match self.mode {
            ProfileTransferMode::Import => local_text(
                "此本地版本暂不支持导入 .synapse4 配置。",
                "Importing .synapse4 profiles is unavailable in this local version.",
            ),
            ProfileTransferMode::Export => local_text(
                "此本地版本暂不支持导出 .synapse4 配置。",
                "Exporting .synapse4 profiles is unavailable in this local version.",
            ),
        }
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .relative()
            .w_full()
            .h(surface::css(36.))
            .flex_shrink_0()
            .justify_center()
            .bg(rgb(0x222222))
            .border_b_1()
            .border_color(rgb(0x515151))
            .rounded_t(surface::css(5.))
            .text_color(rgb(0x999999))
            .child(
                text(if self.mode == ProfileTransferMode::Import {
                    "TEXT_IMPORT_PROFILES"
                } else {
                    "TEXT_EXPORT_PROFILES"
                })
                .to_uppercase(),
            )
            .child(
                BaseButton::new("source-transfer-close")
                    .absolute()
                    .right(surface::css(5.))
                    .size(surface::css(20.))
                    .accessibility_label(text("CLOSE"))
                    .focus_visible(|style| style.border_1().border_color(rgb(0x44d62c)))
                    .child(img("synapse/profiles-close.svg").size_full())
                    .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
            )
    }

    fn local_cloud(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .h(surface::css(49.))
            .flex_shrink_0()
            .items_start()
            .justify_center()
            .pt(surface::css(10.))
            .bg(rgb(0x222222))
            .child(
                BaseButton::new("source-transfer-local-cloud")
                    .accessibility_label(format!("{} / {}", text("TEXT_LOCAL"), text("TEXT_CLOUD")))
                    .h(surface::css(36.))
                    .p(surface::css(5.))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .rounded(surface::css(18.))
                    .bg(rgb(0x111111))
                    .flex()
                    .hover(|style| style.border_color(rgb(0x44d62c)))
                    .active(|style| style.bg(rgb(0x292929)))
                    .focus_visible(|style| style.border_color(rgb(0x44d62c)))
                    .children([false, true].map(|cloud| {
                        div()
                            .h(surface::css(24.))
                            .px(surface::css(10.))
                            .flex()
                            .items_center()
                            .rounded(surface::css(12.))
                            .when(!cloud, |view| view.mr(surface::css(5.)))
                            .when(cloud == self.cloud, |view| {
                                view.bg(rgb(0x44d62c)).text_color(rgb(0x212121))
                            })
                            .child(text(if cloud { "TEXT_CLOUD" } else { "TEXT_LOCAL" }))
                    }))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_cloud(cx))),
            )
    }

    fn browse_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let filename = self
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path_control = if self.pid == 3886 && !self.cloud {
            // Current 3886 attaches the browse click to the folder image only.
            h_flex()
                .ml(surface::css(10.))
                .w(surface::css(250.))
                .h(surface::css(27.))
                .flex_shrink_0()
                .pl(surface::css(6.))
                .pr(surface::css(4.))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .bg(rgb(0x111111))
                .hover(|style| style.border_color(rgb(0x44c62d)))
                .child(div().flex_1().min_w_0().truncate().child(filename))
                .child(
                    BaseButton::new("source-transfer-browse-icon")
                        .size(surface::css(20.))
                        .disabled(self.picker_pending)
                        .accessibility_label(text("BROWSE_FOR_PROFILES"))
                        .focus_visible(|style| style.border_1().border_color(rgb(0x44c62d)))
                        .child(img("synapse/source-transfer-folder.svg").size_full())
                        .on_click(cx.listener(|this, _, _, cx| this.browse(cx))),
                )
                .into_any_element()
        } else {
            BaseButton::new("source-transfer-browse")
                .ml(surface::css(10.))
                .w(surface::css(250.))
                .h(surface::css(27.))
                .flex_shrink_0()
                .pl(surface::css(6.))
                .pr(surface::css(4.))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .bg(rgb(0x111111))
                .flex()
                .items_center()
                .disabled(self.cloud || self.picker_pending)
                .when(self.cloud, |view| view.opacity(0.3))
                .accessibility_label(text(if self.cloud {
                    "COMPATIBLE_CLOUD_PROFILES"
                } else {
                    "BROWSE_FOR_PROFILES"
                }))
                .hover(|style| style.border_color(rgb(0x44c62d)))
                .focus_visible(|style| style.border_color(rgb(0x44c62d)))
                .child(div().flex_1().min_w_0().truncate().child(filename))
                .child(if self.cloud {
                    Icon::new(IconName::ChevronDown)
                        .size(surface::css(20.))
                        .into_any_element()
                } else {
                    img("synapse/source-transfer-folder.svg")
                        .size(surface::css(20.))
                        .into_any_element()
                })
                .on_click(cx.listener(|this, _, _, cx| this.browse(cx)))
                .into_any_element()
        };
        h_flex()
            .w_full()
            .h(surface::css(47.))
            .flex_shrink_0()
            .bg(rgb(0x222222))
            .pl(surface::css(if self.cloud { 89. } else { 100. }))
            .child(text(if self.cloud {
                "COMPATIBLE_CLOUD_PROFILES"
            } else {
                "BROWSE_FOR_PROFILES"
            }))
            .child(path_control)
    }

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.cloud
            && spec(self.pid)
                .locale_keys
                .iter()
                .any(|key| key == "DISABLED_FEATURE_DESC")
        {
            return h_flex()
                .flex_1()
                .min_h_0()
                .justify_center()
                .child(
                    h_flex()
                        .h(surface::css(80.))
                        .px(surface::css(20.))
                        .border_1()
                        .border_color(rgb(0xfd8611))
                        .rounded(surface::css(3.))
                        .child(
                            img("synapse/source-transfer-cone.svg")
                                .size(surface::css(40.))
                                .mr(surface::css(10.)),
                        )
                        .child(text("DISABLED_FEATURE_DESC")),
                )
                .into_any_element();
        }
        if self.mode == ProfileTransferMode::Export && !self.profiles.is_empty() {
            return v_flex()
                .flex_1()
                .min_h_0()
                .child(
                    BaseButton::new("source-transfer-select-all")
                        .h(surface::css(40.))
                        .w_full()
                        .flex_shrink_0()
                        .pl(surface::css(20.))
                        .flex()
                        .items_center()
                        .text_color(rgb(0x44d62c))
                        .child(text(if self.select_all {
                            "TEXT_SELECT_ALL_PROFILES"
                        } else {
                            "TEXT_DESELECT_ALL_PROFILES"
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            for row in &mut this.profiles {
                                row.selected = this.select_all;
                            }
                            this.select_all = !this.select_all;
                            cx.notify();
                        })),
                )
                .child(
                    v_flex()
                        .id("source-transfer-profiles")
                        .flex_1()
                        .min_h_0()
                        .pl(surface::css(20.))
                        .pr(surface::css(27.))
                        .scrollable_y()
                        .child(
                            div().mb(surface::css(6.)).child(
                                product::registered(self.pid)
                                    .map(|product| product.name())
                                    .unwrap_or(""),
                            ),
                        )
                        .children(self.profiles.iter().enumerate().map(|(index, row)| {
                            Checkbox::new(("source-transfer-profile", index))
                                .h(surface::css(20.))
                                .mb(surface::css(10.))
                                .flex_shrink_0()
                                .label(row.profile.name.clone())
                                .checked(row.selected)
                                .on_click(cx.listener(move |this, checked, _, cx| {
                                    if let Some(row) = this.profiles.get_mut(index) {
                                        row.selected = *checked;
                                    }
                                    // Individual changes do not alter source deselectAll.
                                    cx.notify();
                                }))
                        })),
                )
                .into_any_element();
        }
        let status = self.picker_error.clone().unwrap_or_else(|| {
            if self.cloud {
                // 3886's current Cloud is enabled in source. Its connected
                // compatible devices are unavailable in this local host.
                local_text(
                    "当前没有可用的兼容云端配置。",
                    "No compatible cloud profiles are available.",
                )
            } else {
                self.unavailable()
            }
        });
        div()
            .id("source-transfer-status")
            .test_support()
            .role(Role::Status)
            .flex_1()
            .min_h_0()
            .px(surface::css(50.))
            .pt(surface::css(20.))
            .text_center()
            .when(self.picker_error.is_some(), |view| {
                view.text_color(rgb(0xfd4949))
            })
            .child(status)
            .into_any_element()
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .flex_shrink_0()
            .items_center()
            .py(surface::css(16.))
            .border_t_1()
            .border_color(rgb(0x515151))
            .rounded_b(surface::css(5.))
            .bg(rgb(0x222222))
            .when(self.mode == ProfileTransferMode::Export, |view| {
                view.child(
                    div()
                        .px(surface::css(10.))
                        .mb(surface::css(10.))
                        .text_center()
                        .line_height(surface::css(16.))
                        .child(text("IMPORT_EXPORT_MODAL_WILL_NOT_IMPORT")),
                )
                .child(
                    div()
                        .px(surface::css(10.))
                        .mb(surface::css(10.))
                        .text_size(surface::css(12.))
                        .text_center()
                        .child(self.unavailable()),
                )
            })
            .child(
                h_flex()
                    .h(surface::css(27.))
                    .gap(surface::css(10.))
                    .child(
                        Self::footer_button("source-transfer-cancel", "CANCEL", false)
                            .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
                    )
                    .child(Self::footer_button(
                        "source-transfer-submit",
                        if self.mode == ProfileTransferMode::Import {
                            "IMPORT"
                        } else {
                            "EXPORT"
                        },
                        true,
                    )),
            )
    }

    fn footer_button(id: &'static str, key: &str, primary: bool) -> BaseButton {
        BaseButton::new(id)
            .h_full()
            .px(surface::css(24.))
            .border_1()
            .border_color(rgba(0x0000004d))
            .rounded(surface::css(3.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(surface::css(12.))
            .line_height(surface::css(14.))
            .bg(rgb(if primary { 0x44d62c } else { 0x707070 }))
            .text_color(rgb(if primary { 0x000000 } else { 0xffffff }))
            // No vendor encoder/decoder or service: there is no submit callback.
            .disabled(primary)
            .when(primary, |view| view.opacity(0.3))
            .when(!primary, |view| {
                view.hover(|style| style.opacity(0.8))
                    .active(|style| style.opacity(0.6))
            })
            .focus_visible(|style| style.border_color(rgb(0xcccccc)))
            .child(text(key).to_uppercase())
    }
}

impl Render for SourceProfileTransfer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = spec(self.pid);
        let left =
            window.viewport_size().width / 2. - surface::css(300.).to_pixels(window.rem_size());
        let panel = v_flex()
            .id("source-profile-transfer")
            .test_support()
            .w(surface::css(geometry.width))
            .h(surface::css(geometry.height))
            .border_1()
            .border_color(rgb(0x515151))
            .rounded(surface::css(5.))
            .bg(rgb(0x111111))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc))
            .child(self.header(cx))
            .when(self.mode == ProfileTransferMode::Import, |view| {
                view.child(self.local_cloud(cx)).child(self.browse_row(cx))
            })
            .child(self.body(cx))
            .child(self.footer(cx));
        Dialog::new(cx)
            .layer(3, true)
            .focus_handle(self.focus.clone())
            // Current components attach no Escape/Enter/backdrop dismissal.
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .close_on_backdrop_press(false)
            .backdrop(div().absolute().inset_0().bg(rgba(0x000000b3)).occlude())
            .popup(
                DialogPopup::new()
                    .absolute()
                    .left(left)
                    .top(surface::css(geometry.top))
                    .child(panel),
            )
    }
}
