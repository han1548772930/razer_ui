//! Current Profiles 43/Ia. Selection belongs to this modal; submitting retains
//! an explicit local request. Native drafts are never labelled vendor exports.
use super::*;
use razer_model::model::Profile;
use razer_widgets::surface::css;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

#[path = "transfer_codec.rs"]
mod codec;
#[path = "transfer_view.rs"]
mod view;

#[derive(Deserialize)]
struct SourceData {
    labels: BTreeMap<String, String>,
    targets: BTreeMap<u32, TransferTarget>,
}
#[derive(Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TransferTarget {
    product_id: u32,
    dongle_id: Option<u32>,
    ble_id: Option<u32>,
    category: TransferCategory,
    #[serde(default, rename = "is8kAnalogDevice")]
    is_eight_k_analog_device: bool,
}
#[derive(Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum TransferCategory {
    Accessory,
    Audio,
    Broadcaster,
    Case,
    Chair,
    Egpu,
    Gamepad,
    Hue,
    Iot,
    Keyboard,
    Keypad,
    Monitor,
    Mouse,
    Mousemat,
    Mouseplusmat,
    System,
}
impl TransferCategory {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Accessory => "ACCESSORY",
            Self::Audio => "AUDIO",
            Self::Broadcaster => "BROADCASTER",
            Self::Case => "CASE",
            Self::Chair => "CHAIR",
            Self::Egpu => "EGPU",
            Self::Gamepad => "GAMEPAD",
            Self::Hue => "HUE",
            Self::Iot => "IOT",
            Self::Keyboard => "KEYBOARD",
            Self::Keypad => "KEYPAD",
            Self::Monitor => "MONITOR",
            Self::Mouse => "MOUSE",
            Self::Mousemat => "MOUSEMAT",
            Self::Mouseplusmat => "MOUSEPLUSMAT",
            Self::System => "SYSTEM",
        }
    }
}
impl TransferTarget {
    fn matches_category(&self, category: Option<&str>) -> bool {
        category == Some(self.category.as_str())
    }

    fn matches_product(&self, product: Option<f64>) -> bool {
        product.is_some_and(|product| {
            [Some(self.product_id), self.dongle_id, self.ble_id]
                .into_iter()
                .flatten()
                .any(|identity| f64::from(identity) == product)
        })
    }
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
        macros: Vec<Value>,
    },
    Export {
        profiles: Vec<String>,
    },
}
impl TransferIntent {
    pub fn validate(&self, available: &[Profile]) -> Result<(), String> {
        if let Self::Export { profiles } = self {
            if profiles
                .iter()
                .any(|id| !available.iter().any(|p| &p.id == id))
            {
                return Err(local(
                    "所选配置已移除。请重新选择要导出的配置。",
                    "A selected profile was removed. Select the profiles to export again.",
                ));
            }
        }
        Ok(())
    }
    pub fn description(&self) -> String {
        match self {
            Self::Import { profiles, macros } => local(
                &format!(
                    "已保留 {} 项配置和 {} 项宏的本地导入请求；尚未应用到配置或设备。",
                    profiles.len(),
                    macros.len()
                ),
                &format!(
                    "Local import request retained: {} profiles, {} macros. Not applied to profiles or devices.",
                    profiles.len(),
                    macros.len()
                ),
            ),
            Self::Export { profiles } => local(
                &format!(
                    "已保留 {} 项配置的导出选择；官方文件尚未生成。",
                    profiles.len()
                ),
                &format!(
                    "Export selection retained: {} profiles. A vendor file has not been generated.",
                    profiles.len()
                ),
            ),
        }
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
    macros: Vec<Value>,
    cloud: bool,
    select_all: bool,
    file_name: String,
    warning: bool,
    error: Option<String>,
    busy: bool,
    generation: u64,
    task: Option<Task<()>>,
    focus: FocusHandle,
    cloud_devices: Entity<SelectState<Vec<Choice>>>,
    closed: bool,
}
impl ProfileTransfer {
    pub fn new(
        mode: Mode,
        pid: u32,
        name: String,
        profiles: Vec<Profile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rows = if mode == Mode::Export {
            profiles
                .into_iter()
                .filter(|p| p.guid != "af06d371-861f-4b98-8d78-bfaef5cfdebf")
                .map(|p| Row {
                    key: p.id,
                    name: p.name,
                    selected: true,
                    profile: Value::Null,
                    macros: Vec::new(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let cloud_devices = cx.new(|cx| {
            SelectState::new(
                vec![Choice::new("unavailable", " ")],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        Self {
            mode,
            pid,
            device_name: name,
            rows,
            macros: Vec::new(),
            cloud: false,
            select_all: false,
            file_name: String::new(),
            warning: false,
            error: None,
            busy: false,
            generation: 0,
            task: None,
            focus,
            cloud_devices,
            closed: false,
        }
    }
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        cx.emit(Closed(None));
        cx.notify();
    }
    fn enabled(&self) -> bool {
        !self.closed && !self.busy && !self.cloud && self.rows.iter().any(|row| row.selected)
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
                    .map(|r| r.key.clone())
                    .collect(),
            }
        } else {
            let selected: BTreeSet<_> = self
                .rows
                .iter()
                .filter(|r| r.selected)
                .flat_map(|r| {
                    r.macros
                        .iter()
                        .filter(|m| m.selected)
                        .map(|m| m.guid.as_str())
                })
                .collect();
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
                macros: self
                    .macros
                    .iter()
                    .filter(|m| m["guid"].as_str().is_some_and(|id| selected.contains(id)))
                    .cloned()
                    .collect(),
            }
        };
        self.closed = true;
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
        cx.emit(Closed(Some(intent)));
        cx.notify();
    }
    fn toggle_cloud(&mut self, cx: &mut Context<Self>) {
        self.cloud = !self.cloud;
        self.generation = self.generation.wrapping_add(1);
        self.rows.clear();
        self.macros.clear();
        self.file_name.clear();
        self.error = None;
        self.warning = false;
        // Keep the native picker task owned until it returns; its generation
        // cannot replace the newly selected Local/Cloud state.
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
        if self.closed || self.busy || self.cloud || self.mode != Mode::Import {
            return;
        }
        let Some(target) = data().targets.get(&self.pid).cloned() else {
            self.error = Some(local(
                "此产品的当前导入兼容性尚未核实。",
                "Current import compatibility for this product is not verified.",
            ));
            cx.notify();
            return;
        };
        self.busy = true;
        self.error = None;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
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
                        .background_spawn(async move { codec::read(&path, &target) })
                        .await;
                    Some((Some(name), result))
                }
                Ok(None) => None,
                Err(error) => Some((None, Err(error))),
            };
            let _ = owner.update(cx, |this, cx| {
                this.busy = false;
                this.task = None;
                if this.closed || this.cloud || this.generation != generation {
                    cx.notify();
                    return;
                }
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
                        this.macros = file.macros;
                        if this.rows.is_empty() {
                            this.error = Some(text("v3x"));
                        }
                    }
                    Some((_, Err(error))) => {
                        this.rows.clear();
                        this.macros.clear();
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
                targets: r.child(decode_3)?,
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
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<u32, TransferTarget>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<TransferTarget> {
        let value = match r.tag()? {
            0 => TransferTarget {
                product_id: r.child(decode_4)?,
                dongle_id: None,
                ble_id: None,
                category: r.child(decode_6)?,
                is_eight_k_analog_device: Default::default(),
            },
            1 => TransferTarget {
                product_id: r.child(decode_4)?,
                dongle_id: r.child(decode_7)?,
                ble_id: None,
                category: r.child(decode_6)?,
                is_eight_k_analog_device: Default::default(),
            },
            2 => TransferTarget {
                product_id: r.child(decode_4)?,
                dongle_id: r.child(decode_7)?,
                ble_id: r.child(decode_7)?,
                category: r.child(decode_6)?,
                is_eight_k_analog_device: Default::default(),
            },
            3 => TransferTarget {
                product_id: r.child(decode_4)?,
                dongle_id: None,
                ble_id: r.child(decode_7)?,
                category: r.child(decode_6)?,
                is_eight_k_analog_device: Default::default(),
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<TransferCategory> {
        let value = match r.tag()? {
            0 => TransferCategory::Mouse,
            1 => TransferCategory::Mouseplusmat,
            2 => TransferCategory::Accessory,
            3 => TransferCategory::Mousemat,
            4 => TransferCategory::Keyboard,
            5 => TransferCategory::Keypad,
            6 => TransferCategory::System,
            7 => TransferCategory::Hue,
            8 => TransferCategory::Audio,
            9 => TransferCategory::Iot,
            10 => TransferCategory::Broadcaster,
            11 => TransferCategory::Gamepad,
            12 => TransferCategory::Monitor,
            13 => TransferCategory::Case,
            14 => TransferCategory::Egpu,
            15 => TransferCategory::Chair,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u32>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_4)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_TRANSFER_DATA,
        0,
        "110f1e1dc077a622575577306fdcf7bc66194e18a2be5a75418f623a74cf4beb",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_transfer_data_decodes() {
    let _ = read_transfer_catalog();
}
// End generated catalog reader: razer-app-pages/transfer_data.
