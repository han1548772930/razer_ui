//! Explicitly local JSON, not Synapse's base64/hash envelope or service export.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const FORMAT: &str = "razer-ui-profile";
const VERSION: u32 = 1;
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileDocument {
    format: String,
    version: u32,
    product_id: u32,
    layout_id: u32,
    profile: ProfileContent,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileContent {
    name: String,
    settings: Value,
}

fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.trim() == name
        && name.encode_utf16().count() <= 32
        && !name.chars().any(char::is_control)
}

fn import_name(profiles: &[Profile], original: &str) -> String {
    if !profiles
        .iter()
        .any(|profile| profile.name.eq_ignore_ascii_case(original))
    {
        return original.into();
    }
    (1..)
        .map(|index| {
            let suffix = format!(" ({index})");
            let mut base = original.to_string();
            while base.encode_utf16().count() + suffix.len() > 32 {
                base.pop();
            }
            format!("{}{suffix}", base.trim_end())
        })
        .find(|name| {
            !profiles
                .iter()
                .any(|profile| profile.name.eq_ignore_ascii_case(name))
        })
        .expect("an unused import name")
}

fn validate_settings(settings: &ProfileSettings, pid: u32) -> Result<(), String> {
    let mut normalized = settings.clone();
    normalized.normalize(pid);
    if normalized != *settings {
        return Err("配置包含超出范围或不完整的设置。".into());
    }
    if ![125, 250, 500, 1000, 2000, 4000, 8000].contains(&settings.polling) {
        return Err("配置包含无效的轮询率。".into());
    }
    for bindings in [&settings.bindings, &settings.hypershift_bindings] {
        if bindings.len() > 512
            || bindings.iter().any(|(key, value)| {
                key.is_empty()
                    || key.len() > 128
                    || key.chars().any(char::is_control)
                    || value.len() > 65536
            })
        {
            return Err("配置中的按键映射超出限制。".into());
        }
    }
    let mut paths = std::collections::BTreeSet::new();
    if settings.linked_games.len() > 128
        || settings.linked_games.iter().any(|game| {
            !linked_games::valid_game(game)
                || !paths.insert(linked_games::executable_key(&game.executable))
        })
    {
        return Err("配置包含无效或重复的关联程序。".into());
    }
    if [&settings.audio, &settings.mic]
        .iter()
        .any(|eq| eq.bands.len() > 64 || eq.custom.len() > 64)
    {
        return Err("配置中的均衡器频段数量超出限制。".into());
    }
    let keyboard = &settings.keyboard;
    let pairs = keyboard.snap_key_pairs();
    let mut keys = std::collections::BTreeSet::new();
    if pairs.is_empty()
        || pairs.len() > 4
        || keyboard.snap_keys != pairs[0]
        || pairs.iter().flatten().any(|key| {
            crate::features::settings::canonical_snap_key(key).as_ref() != Some(key)
                || !keys.insert(key)
        })
    {
        return Err("配置包含无效或重复的 Snap Tap 按键。".into());
    }
    let defaults = crate::features::settings::Keyboard::default();
    let mut modes = std::collections::BTreeSet::new();
    if keyboard.dial_modes.len() > defaults.dial_modes.len() + 100
        || !keyboard.dial_selection_valid()
        || defaults.dial_modes.iter().any(|default| {
            !keyboard
                .dial_modes
                .iter()
                .any(|mode| !mode.is_custom && mode.uid == default.uid)
        })
        || keyboard.dial_modes.iter().any(|mode| {
            mode.uid.is_empty()
                || mode.uid.len() > 128
                || !modes.insert(&mode.uid)
                || mode.uid.chars().any(char::is_control)
                || !mode.is_custom
                    && !defaults
                        .dial_modes
                        .iter()
                        .any(|default| default.uid == mode.uid)
                || mode.is_custom
                    && (mode.name.trim().is_empty()
                        || mode.name.encode_utf16().count() > 40
                        || mode.name.chars().any(char::is_control))
                || mode.mappings.iter().any(|(input, value)| {
                    !["ScrollLeft", "ScrollRight"].contains(&input.as_str()) || value.len() > 65536
                })
        })
    {
        return Err("配置包含无效的拨轮模式或映射。".into());
    }
    Ok(())
}

// Reject duplicate JSON keys at every depth instead of silently accepting the
// last occurrence. The ordinary serde_json recursion limit also remains active.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Value;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
                serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = seq.next_element()? {
                    values.push(value);
                }
                Ok(Value::Array(values))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some((key, UniqueValue(value))) =
                    map.next_entry::<String, UniqueValue>()?
                {
                    if values.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom("duplicate JSON key"));
                    }
                }
                Ok(Value::Object(values))
            }
        }
        deserializer.deserialize_any(Visitor).map(Self)
    }
}

fn decode_profile(bytes: &[u8], pid: u32, layout: u32) -> Result<Profile, String> {
    if bytes.len() > MAX_BYTES {
        return Err("配置文件不得大于 2 MiB。".into());
    }
    let UniqueValue(value) =
        serde_json::from_slice(bytes).map_err(|error| format!("无法读取配置 JSON：{error}"))?;
    let document: ProfileDocument = serde_json::from_value(value)
        .map_err(|_| "请选择本项目导出的 .razer-ui-profile.json 文件。".to_string())?;
    if document.format != FORMAT {
        return Err("此文件不是本项目的配置格式。".into());
    }
    if document.version != VERSION {
        return Err(format!("不支持配置格式版本 {}。", document.version));
    }
    if document.product_id != pid || document.layout_id != layout {
        return Err("此配置的设备型号或键盘布局与当前设备不符。".into());
    }
    if !valid_name(&document.profile.name) {
        return Err("配置名称须为 1–32 个字符，且不能包含控制字符。".into());
    }
    let settings: ProfileSettings = serde_json::from_value(document.profile.settings.clone())
        .map_err(|error| format!("配置设置无效：{error}"))?;
    // The store accepts older local settings with defaults. The transfer format
    // is stricter: compare to the current serializer to reject unknown/missing
    // fields recursively without maintaining a second schema or dropping data.
    if serde_json::to_value(&settings).map_err(|error| error.to_string())?
        != document.profile.settings
    {
        return Err("配置设置包含未知、缺失或不兼容的字段。".into());
    }
    validate_settings(&settings, pid)?;
    Ok(Profile {
        id: String::new(),
        guid: String::new(),
        name: document.profile.name,
        dpi_stages: None,
        settings: Some(settings),
    })
}

fn encode_profile(device: &Device, profile: &Profile) -> Result<Vec<u8>, String> {
    if !valid_name(&profile.name) {
        return Err("请先将配置文件重命名为有效名称。".into());
    }
    let settings = profile.settings.as_ref().ok_or("配置设置尚未加载。")?;
    validate_settings(settings, device.product_id)?;
    let document = ProfileDocument {
        format: FORMAT.into(),
        version: VERSION,
        product_id: device.product_id,
        layout_id: device.layout_id,
        profile: ProfileContent {
            name: profile.name.clone(),
            settings: serde_json::to_value(settings).map_err(|error| error.to_string())?,
        },
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("配置内容超出 2 MiB 导出限制。".into());
    }
    Ok(bytes)
}

fn read_profile(path: &Path, pid: u32, layout: u32) -> Result<Profile, String> {
    let file = std::fs::File::open(path).map_err(|error| format!("无法打开配置文件：{error}"))?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
        return Err("请选择不大于 2 MiB 的配置文件。".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    decode_profile(&bytes, pid, layout)
}

fn write_profile(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "文件已存在，请选择新的文件名。".into()
            } else {
                format!("无法创建导出文件：{error}")
            }
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path); // Only the newly created incomplete file.
        return Err(format!("导出失败：{error}"));
    }
    Ok(())
}

struct ImportDialog {
    target: ProfileTarget,
    pid: u32,
    layout: u32,
    loaded: Option<Profile>,
    name: Entity<InputState>,
    file: Option<String>,
    error: Option<String>,
    busy: bool,
}
impl ImportDialog {
    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择本地配置 JSON".into()),
        });
        let (pid, layout) = (self.pid, self.layout);
        cx.spawn_in(window, async move |view, cx| {
            let result = match picker.await {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => {
                        let label = path.to_string_lossy().into_owned();
                        Some((
                            label,
                            cx.background_executor()
                                .spawn(async move { read_profile(&path, pid, layout) })
                                .await,
                        ))
                    }
                    None => None,
                },
                Ok(Ok(None)) => None,
                _ => Some((String::new(), Err("无法打开文件选择窗口。".into()))),
            };
            _ = view.update_in(cx, |this, window, cx| {
                this.busy = false;
                let Some(workspace) = this.target.current(cx) else {
                    this.loaded = None;
                    this.error = Some("当前配置文件已切换，请重新打开导入。".into());
                    cx.notify();
                    return;
                };
                if let Some((file, result)) = result {
                    this.file = Some(file);
                    match result {
                        Ok(profile) => {
                            let name =
                                import_name(&workspace.read(cx).device.profiles, &profile.name);
                            this.name
                                .update(cx, |input, cx| input.set_value(name, window, cx));
                            this.loaded = Some(profile);
                            this.error = None;
                        }
                        Err(error) => {
                            this.loaded = None;
                            this.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(mut profile) = self.loaded.clone() else {
            return;
        };
        let Some(workspace) = self.target.current(cx) else {
            self.error = Some("当前配置文件已切换，请重新打开导入。".into());
            cx.notify();
            return;
        };
        let name = self.name.read(cx).value().trim().to_string();
        if !valid_name(&name)
            || workspace
                .read(cx)
                .device
                .profiles
                .iter()
                .any(|profile| profile.name.eq_ignore_ascii_case(&name))
        {
            self.error = Some("请输入 1–32 个字符且未被使用的配置名称。".into());
            cx.notify();
            return;
        }
        profile.name = name;
        window.close_dialog(cx);
        workspace.update(cx, |workspace, cx| {
            workspace.continue_with(Continue::ImportProfile(profile), window, cx)
        });
    }
}
impl Render for ImportDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("profile-import-dialog")
            .test_support()
            .gap_3()
            .child(surface::note(
                "仅支持本项目导出的 .razer-ui-profile.json。导入会新增本地配置，不覆盖已有配置。",
                cx,
            ))
            .child(
                Button::new("profile-import-browse")
                    .label(if self.busy {
                        "正在读取…"
                    } else {
                        "选择文件…"
                    })
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, window, cx| this.browse(window, cx))),
            )
            .when_some(self.file.clone(), |body, file| {
                body.child(div().text_xs().child(file))
            })
            .when(self.loaded.is_some(), |body| {
                body.child(div().child("新配置名称"))
                    .child(
                        Input::new(&self.name)
                            .id("profile-import-name")
                            .aria_label("新配置名称"),
                    )
                    .child(surface::note(
                        "同名配置已自动添加编号；导入后仍需保存设备配置。",
                        cx,
                    ))
            })
            .when_some(self.error.clone(), |body, error| {
                body.child(
                    div()
                        .id("profile-import-error")
                        .test_support()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap_3()
                    .child(
                        Button::new("profile-import-cancel")
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("profile-import-confirm")
                            .label("导入到本地草稿")
                            .primary()
                            .disabled(self.loaded.is_none() || self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.import(window, cx))),
                    ),
            )
    }
}

struct ExportDialog {
    target: ProfileTarget,
    bytes: Vec<u8>,
    busy: bool,
    result: Option<Result<String, String>>,
}
impl ExportDialog {
    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.result = None;
        let directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let picker = cx.prompt_for_new_path(&directory, Some("profile.razer-ui-profile.json"));
        cx.spawn_in(window, async move |view, cx| {
            let path = match picker.await {
                Ok(Ok(path)) => Ok(path),
                _ => Err("无法打开保存文件窗口。".to_string()),
            };
            let prepared = view.update_in(cx, |this, _, cx| {
                if this.target.current(cx).is_none() {
                    Err("当前配置文件已切换，请重新打开导出。".to_string())
                } else {
                    path.map(|path| path.map(|path| (path, this.bytes.clone())))
                }
            });
            let result = match prepared {
                Ok(Ok(Some((path, bytes)))) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            write_profile(&path, &bytes)
                                .map(|_| path.to_string_lossy().into_owned())
                        })
                        .await,
                ),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => Some(Err(error)),
                Err(_) => return,
            };
            _ = view.update_in(cx, |this, _, cx| {
                this.busy = false;
                this.result = result;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
impl Render for ExportDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().id("profile-export-dialog").test_support().gap_3()
            .child(surface::note("导出当前本地配置快照，包含按键映射和关联程序路径。此 JSON 供本项目导入，不是 Synapse 配置包。", cx));
        if let Some(result) = &self.result {
            body = body.child(
                div()
                    .id("profile-export-result")
                    .test_support()
                    .text_color(if result.is_ok() {
                        cx.theme().foreground
                    } else {
                        cx.theme().danger
                    })
                    .child(match result {
                        Ok(path) => format!("已导出：{path}"),
                        Err(error) => error.clone(),
                    }),
            );
        }
        body.child(
            h_flex()
                .justify_end()
                .gap_3()
                .child(
                    Button::new("profile-export-close")
                        .label("关闭")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .child(
                    Button::new("profile-export-save")
                        .label(if self.busy {
                            "正在导出…"
                        } else {
                            "选择导出位置…"
                        })
                        .primary()
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.export(window, cx))),
                ),
        )
    }
}

impl DeviceWorkspace {
    pub(super) fn open_profile_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = ProfileTarget::new(self, cx);
        let (pid, layout) = (self.device.product_id, self.device.layout_id);
        let name = cx.new(|cx| InputState::new(window, cx));
        let view = cx.new(|_| ImportDialog {
            target,
            pid,
            layout,
            loaded: None,
            name,
            file: None,
            error: None,
            busy: false,
        });
        window.open_dialog(cx, move |dialog, window, _| {
            dialog
                .title("导入本地配置")
                .w((window.rem_size() * (540. / 16.))
                    .min((window.viewport_size().width - px(40.)).max(px(240.))))
                .child(view.clone())
        });
    }
    pub(in crate::features::workspace) fn import_local_profile(
        &mut self,
        mut profile: Profile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        profile.name = import_name(&self.device.profiles, &profile.name);
        let id = next_profile_id(&self.device, &self.saved);
        profile.id = id.clone();
        profile.guid = id.clone();
        self.device.profiles.push(profile);
        self.device.active_profile = id;
        self.refresh_profile_choices(window, cx);
    }
    pub(in crate::features::workspace) fn export_local_profile(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.device.active_profile_obj() else {
            return;
        };
        let bytes = match encode_profile(&self.device, profile) {
            Ok(bytes) => bytes,
            Err(error) => {
                window.push_notification(error, cx);
                return;
            }
        };
        let name = profile.name.clone();
        let target = ProfileTarget::new(self, cx);
        let view = cx.new(|_| ExportDialog {
            target,
            bytes,
            busy: false,
            result: None,
        });
        window.open_dialog(cx, move |dialog, window, _| {
            dialog
                .title(format!("导出本地配置 — {name}"))
                .w((window.rem_size() * (540. / 16.))
                    .min((window.viewport_size().width - px(40.)).max(px(240.))))
                .child(view.clone())
        });
    }
}

#[cfg(test)]
#[path = "profile_transfer_tests.rs"]
mod tests;
