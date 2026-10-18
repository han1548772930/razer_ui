//! Current Profiles 43/Ia. Selection belongs to this modal; the retained product
//! owner applies the submitted command and owns persistence or file output.
use super::*;
use razer_model::model::Profile;
use razer_model::profile_transfer::SourceExportSelection;
use razer_widgets::surface::css;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[path = "transfer_view.rs"]
mod view;

#[derive(Deserialize)]
struct SourceData {
    labels: BTreeMap<String, String>,
}
fn data() -> &'static SourceData {
    static DATA: OnceLock<SourceData> = OnceLock::new();
    DATA.get_or_init(|| read_transfer_catalog())
}
fn text(symbol: &str) -> String {
    i18n::t(&data().labels[symbol])
}
fn local(zh: &str, en: &str) -> String {
    if i18n::locale().starts_with("zh") {
        zh.into()
    } else {
        en.into()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Import,
    Export,
}

#[derive(Clone)]
pub enum TransferIntent {
    Import {
        // Current Ua carries selectedProfileData rows, including per-profile
        // macro choices. A union alone loses A-selected/B-unselected intent.
        profiles: Vec<Value>,
    },
    Export {
        profiles: Vec<SourceExportSelection>,
    },
}
impl TransferIntent {
    pub fn validate(&self, available: &[Profile]) -> Result<(), String> {
        if let Self::Export { profiles } = self {
            if profiles
                .iter()
                .any(|row| !available.iter().any(|p| p.id == row.profile_id))
            {
                return Err(local(
                    "所选配置已移除。请重新选择要导出的配置。",
                    "A selected profile was removed. Select the profiles to export again.",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Closed(pub Option<TransferIntent>);
impl EventEmitter<Closed> for ProfileTransfer {}

struct MacroRow {
    guid: String,
    name: String,
    selected: bool,
}
struct Row {
    key: String,
    name: String,
    selected: bool,
    profile: Value,
    macros: Vec<MacroRow>,
}
pub struct ProfileTransfer {
    mode: Mode,
    pid: u32,
    device_name: String,
    rows: Vec<Row>,
    default_profile: Option<Value>,
    select_all: bool,
    file_name: String,
    warning: bool,
    error: Option<String>,
    busy: bool,
    generation: u64,
    task: Option<Task<()>>,
    request: razer_storage::file_transfer::Transfer,
    focus: FocusHandle,
    closed: bool,
}
impl ProfileTransfer {
    pub fn new(
        mode: Mode,
        pid: u32,
        name: String,
        profiles: Vec<Profile>,
        macros: Vec<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let default_profile = profiles
            .first()
            .and_then(|profile| profile.source_settings.clone());
        let rows = if mode == Mode::Export {
            profiles
                .into_iter()
                .filter(|p| p.guid != "af06d371-861f-4b98-8d78-bfaef5cfdebf")
                .map(|p| {
                    let macro_rows = p
                        .source_settings
                        .as_ref()
                        .map(|source| macro_rows(source, &macros))
                        .unwrap_or_default();
                    Row {
                        key: p.id,
                        name: p.name,
                        selected: true,
                        profile: Value::Null,
                        macros: macro_rows,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            mode,
            pid,
            device_name: name,
            rows,
            default_profile,
            select_all: false,
            file_name: String::new(),
            warning: false,
            error: None,
            busy: false,
            generation: 0,
            task: None,
            request: razer_storage::file_transfer::Transfer::new(),
            focus,
            closed: false,
        }
    }
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.request.cancel();
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        cx.emit(Closed(None));
        cx.notify();
    }
    fn enabled(&self) -> bool {
        !self.closed && !self.busy && self.rows.iter().any(|row| row.selected)
    }
    fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.enabled() {
            return;
        }
        let intent = if self.mode == Mode::Export {
            TransferIntent::Export {
                profiles: self
                    .rows
                    .iter()
                    .filter(|r| r.selected)
                    .map(|r| {
                        SourceExportSelection::new(
                            r.key.clone(),
                            r.macros
                                .iter()
                                .filter(|row| row.selected)
                                .map(|row| row.guid.clone())
                                .collect(),
                        )
                    })
                    .collect(),
            }
        } else {
            TransferIntent::Import {
                profiles: self
                    .rows
                    .iter()
                    .filter(|r| r.selected)
                    .map(|r| {
                        serde_json::json!({
                            "name": r.name,
                            "profile": r.profile,
                            "isProfileSelected": true,
                            "hasWarning": self.warning,
                            "fileType": "synapse4",
                            "listItems": r.macros.iter().map(|m| serde_json::json!({
                                "guid": m.guid,
                                "text": m.name,
                                "isMacroSelected": m.selected,
                            })).collect::<Vec<_>>(),
                        })
                    })
                    .collect(),
            }
        };
        self.closed = true;
        self.request.cancel();
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        cx.emit(Closed(Some(intent)));
        cx.notify();
    }
    fn select(&mut self, key: &str, selected: bool, cx: &mut Context<Self>) {
        if self.closed || self.busy {
            return;
        }
        if let Some(row) = self.rows.iter_mut().find(|r| r.key == key) {
            row.selected = selected;
            if selected {
                for m in &mut row.macros {
                    m.selected = true;
                }
            }
        }
        cx.notify();
    }
    fn browse(&mut self, cx: &mut Context<Self>) {
        if self.closed || self.busy || self.mode != Mode::Import {
            return;
        }
        let Some(target) =
            razer_device::profile_transfer::product_target(self.pid).map(|product| product.target)
        else {
            self.error = Some(local(
                "此产品的当前导入兼容性尚未核实。",
                "Current import compatibility for this product is not verified.",
            ));
            cx.notify();
            return;
        };
        self.request.cancel();
        self.request = razer_storage::file_transfer::Transfer::new();
        let current = self.request.clone();
        let operation = match self.request.accept() {
            Ok(operation) => operation,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.busy = true;
        self.error = None;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let default_profile = self.default_profile.clone();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(format!("{} (.synapse4)", text("M88")).into()),
        });
        self.task = Some(cx.spawn(async move |owner, cx| {
            let path = match prompt.await {
                Ok(Ok(Some(paths))) => Ok(paths.into_iter().next()),
                Ok(Ok(None)) => Ok(None),
                _ => Err(local(
                    "无法打开文件选择器。",
                    "Could not open the file picker.",
                )),
            };
            let result = match path {
                Ok(Some(path)) => {
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let result = cx
                        .background_spawn(async move {
                            razer_storage::profile_transfer::read_global(
                                &operation,
                                &path,
                                &target,
                                default_profile.as_ref(),
                            )
                        })
                        .await;
                    Some((Some(name), result))
                }
                Ok(None) => None,
                Err(error) => Some((None, Err(error))),
            };
            let _ = owner.update(cx, |this, cx| {
                if this.closed || this.generation != generation || !this.request.is_same(&current) {
                    return;
                }
                this.busy = false;
                this.task = None;
                if let Some((Some(name), _)) = &result {
                    // Da records the selected filename before decoding, also
                    // when the file is incompatible or corrupt.
                    this.file_name = name.clone();
                }
                match result {
                    Some((_, Ok(file))) => {
                        this.warning = file.warning;
                        // Ia prefers the target product's localized name.
                        if this.device_name.is_empty() {
                            this.device_name = file.device_name;
                        }
                        this.rows = file
                            .profiles
                            .into_iter()
                            .enumerate()
                            .map(|(ix, profile)| {
                                let macros = macro_rows(&profile, &file.macros);
                                Row {
                                    key: format!("import-{generation}-{ix}"),
                                    name: profile["name"].as_str().unwrap_or("").into(),
                                    selected: true,
                                    profile,
                                    macros,
                                }
                            })
                            .collect();
                        if this.rows.is_empty() {
                            this.error = Some(text("v3x"));
                        }
                    }
                    Some((_, Err(error))) => {
                        this.rows.clear();
                        this.warning = false;
                        this.error = Some(error);
                    }
                    None => (), // Cancel retains the previous successfully read selection.
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
impl Drop for ProfileTransfer {
    fn drop(&mut self) {
        self.request.cancel();
    }
}
fn macro_rows(profile: &Value, macros: &[Value]) -> Vec<MacroRow> {
    let mut ids = Vec::new();
    let mut scan = |mapping: &Value| {
        if let Some(id) = mapping["macroGroup"]["guid"].as_str() {
            if !ids.contains(&id.to_string()) {
                ids.push(id.to_string());
            }
        }
    };
    for mapping in profile["mappings"].as_array().into_iter().flatten() {
        scan(mapping);
        for child in mapping["mapping"].as_array().into_iter().flatten() {
            scan(child);
        }
    }
    for keymap in profile["keymaps"].as_array().into_iter().flatten() {
        for mapping in keymap["mappings"].as_array().into_iter().flatten() {
            scan(mapping);
        }
    }
    if let Some(panels) = profile["sidePanelMappings"].as_object() {
        for mappings in panels.values() {
            for mapping in mappings.as_array().into_iter().flatten() {
                scan(mapping);
            }
        }
    }
    ids.into_iter()
        .filter_map(|guid| {
            let m = macros.iter().find(|m| m["guid"] == guid)?;
            Some(MacroRow {
                guid,
                name: m["name"].as_str().unwrap_or("").into(),
                selected: true,
            })
        })
        .collect()
}

// Generated catalog reader: razer-app-pages/transfer_data.
fn read_transfer_catalog() -> SourceData {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceData> {
        let value = match r.tag()? {
            0 => SourceData {
                labels: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_TRANSFER_DATA,
        0,
        "46acbfb7d1168b4f6f4136a1cf475096721c157460e6b5a226e559553231f150",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod transfer_data_catalog_tests {
    #[test]
    fn packaged_catalog_transfer_data_decodes() {
        let _ = super::read_transfer_catalog();
    }
}
// End generated catalog reader: razer-app-pages/transfer_data.
