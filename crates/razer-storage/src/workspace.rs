//! Local, versioned storage. This format does not claim compatibility with Synapse profiles.
//! Preserve legacy DeviceFeatures and migrate old top-level Vec<Device> files on save.
use razer_model::model::Device;
use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
const VERSION: u32 = 2;
mod retired_shortcuts;
pub use retired_shortcuts::RetiredShortcuts;
mod device_save;
pub use device_save::DeviceSaveDraft;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFile {
    version: u32,
    pub devices: Vec<Device>,
    #[serde(default)]
    pub tracking_intro_seen: bool,
    #[serde(default)]
    pub shortcuts: Vec<razer_model::shortcuts::Shortcut>,
    #[serde(default, skip_serializing_if = "RetiredShortcuts::is_empty")]
    retired_shortcuts: RetiredShortcuts,
    #[serde(default)]
    pub macros: razer_model::macro_library::MacroLibraryFile,
    #[serde(default)]
    pub preferences: razer_model::preferences::AppPreferences,
    #[serde(default)]
    pub custom_colors: razer_model::preferences::CustomColorSlots,
    #[serde(default)]
    pub host_tab_order: Vec<String>,
    #[serde(default)]
    pub dashboard: razer_model::preferences::DashboardPreferences,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_services: Option<razer_model::module_service::ModuleServiceSnapshot>,
}
impl WorkspaceFile {
    pub fn new(devices: Vec<Device>, tracking_intro_seen: bool) -> Self {
        Self {
            version: VERSION,
            devices,
            tracking_intro_seen,
            shortcuts: vec![],
            retired_shortcuts: RetiredShortcuts::default(),
            macros: Default::default(),
            preferences: razer_model::preferences::AppPreferences::default(),
            custom_colors: [None; 16],
            host_tab_order: vec![],
            dashboard: Default::default(),
            module_services: None,
        }
    }
    pub fn with_shortcuts(mut self, shortcuts: Vec<razer_model::shortcuts::Shortcut>) -> Self {
        self.shortcuts = shortcuts;
        self
    }
    pub fn retired_shortcuts(&self) -> &RetiredShortcuts {
        &self.retired_shortcuts
    }
    pub fn with_retired_shortcuts(mut self, shortcuts: RetiredShortcuts) -> Self {
        self.retired_shortcuts = shortcuts;
        self
    }
    pub fn with_preferences(
        mut self,
        preferences: razer_model::preferences::AppPreferences,
    ) -> Self {
        self.preferences = preferences;
        self
    }
    pub fn with_macros(mut self, macros: razer_model::macro_library::MacroLibraryFile) -> Self {
        self.macros = macros;
        self
    }
    pub fn with_custom_colors(
        mut self,
        colors: razer_model::preferences::CustomColorSlots,
    ) -> Self {
        self.custom_colors = colors;
        self
    }
    pub fn with_host_tab_order(mut self, order: Vec<String>) -> Self {
        self.host_tab_order = order;
        self
    }
    pub fn with_dashboard(
        mut self,
        dashboard: razer_model::preferences::DashboardPreferences,
    ) -> Self {
        self.dashboard = dashboard;
        self
    }
    pub fn with_module_services(
        mut self,
        snapshot: Option<razer_model::module_service::ModuleServiceSnapshot>,
    ) -> Self {
        self.module_services = snapshot;
        self
    }
}
pub fn read_workspace(path: &Path) -> anyhow::Result<Option<WorkspaceFile>> {
    crate::read_document(path, decode_workspace)
}

fn decode_known<T: serde::de::DeserializeOwned>(text: &str) -> anyhow::Result<T> {
    let mut unknown = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value =
        serde_ignored::deserialize(&mut deserializer, |path| unknown.push(path.to_string()))?;
    deserializer.end()?;
    anyhow::ensure!(
        unknown.is_empty(),
        "配置含当前版本无法保留的字段：{}",
        unknown.join(", ")
    );
    Ok(value)
}

fn decode_workspace(text: &str) -> anyhow::Result<WorkspaceFile> {
    let text = text.trim_start_matches('\u{feff}');
    let mut file = if text.trim_start().starts_with('[') {
        WorkspaceFile::new(decode_known(text)?, false)
    } else {
        let file: WorkspaceFile = decode_known(text)?;
        anyhow::ensure!(
            file.version == VERSION,
            "Unsupported profile version {}",
            file.version
        );
        file
    };
    file.retired_shortcuts.migrate(&mut file.shortcuts)?;
    razer_model::shortcuts::validate_stored_shortcuts(&file.shortcuts)
        .map_err(anyhow::Error::msg)?;
    file.preferences.validate().map_err(anyhow::Error::msg)?;
    file.macros.validate().map_err(anyhow::Error::msg)?;
    file.devices.retain(|device| {
        !(device.product_id == 9001
            && device.serial_number == "DEMO-KEYBOARD-0001"
            && device.device_container_id == "{DEMO-0000-0000-0000-000000000001}")
    });
    Ok(file)
}
pub fn write_workspace(path: &Path, file: &WorkspaceFile) -> anyhow::Result<()> {
    file.preferences.validate().map_err(anyhow::Error::msg)?;
    file.macros.validate().map_err(anyhow::Error::msg)?;
    razer_model::shortcuts::validate_stored_shortcuts(&file.shortcuts)
        .map_err(anyhow::Error::msg)?;
    file.retired_shortcuts.validate(&file.shortcuts)?;
    crate::write_document(path, file, |text| decode_workspace(text).map(|_| ()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_online_shortcuts_are_retained_across_saves_outside_local_actions() {
        let path = test_path("retired-shortcuts");
        let document = serde_json::json!({
            "version": 2,
            "devices": [],
            "shortcuts": [
                {
                    "id": "online", "input": "KEY_C", "modifiers": ["GUI"],
                    "hypershift": false,
                    "output": {"kind": "windows", "action": "MSCopilot"}
                },
                {
                    "id": "local", "input": "KEY_K", "modifiers": ["CTRL"],
                    "hypershift": false,
                    "output": {"kind": "windows", "action": "Calculator"}
                }
            ]
        });
        let original = document.to_string();
        std::fs::write(&path, &original).unwrap();
        let loaded = read_workspace(&path).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        assert_eq!(loaded.shortcuts.len(), 1);
        assert_eq!(loaded.shortcuts[0].id, "local");
        razer_model::shortcuts::encode_shortcuts(&loaded.shortcuts).unwrap();
        let retained = loaded.retired_shortcuts().clone();
        let active = loaded.shortcuts;
        for _ in 0..2 {
            let next = WorkspaceFile::new(vec![], true)
                .with_shortcuts(active.clone())
                .with_retired_shortcuts(retained.clone());
            write_workspace(&path, &next).unwrap();
            let saved = read_workspace(&path).unwrap().unwrap();
            assert_eq!(saved.shortcuts, active);
            let value = serde_json::to_value(saved).unwrap();
            assert_eq!(
                value["retired_shortcuts"],
                serde_json::json!([document["shortcuts"][0]])
            );
        }
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
    }

    #[test]
    fn retained_online_records_cannot_hide_local_actions_or_invalid_input() {
        let archived = serde_json::json!({
            "id": "archived", "input": "KEY_C", "modifiers": ["GUI"],
            "hypershift": false,
            "output": {"kind": "windows", "action": "MSCopilot"}
        });
        let mut document = serde_json::json!({
            "version": 2, "devices": [], "retired_shortcuts": [archived.clone()]
        });
        assert!(
            decode_workspace(&document.to_string())
                .unwrap()
                .shortcuts
                .is_empty()
        );
        document["retired_shortcuts"][0]["input"] = serde_json::json!("NOT_A_KEY");
        assert!(decode_workspace(&document.to_string()).is_err());
        document["retired_shortcuts"][0] = archived.clone();
        document["retired_shortcuts"][0]["output"]["action"] = serde_json::json!("Calculator");
        assert!(decode_workspace(&document.to_string()).is_err());
        document["retired_shortcuts"][0] = archived.clone();
        document["shortcuts"] = serde_json::json!([archived]);
        assert!(decode_workspace(&document.to_string()).is_err());
    }

    #[test]
    fn legacy_synthetic_keyboard_is_removed_without_relabeling_or_losing_other_devices() {
        let preview = razer_model::product_preview::registered_preview(653).unwrap();
        let mut legacy = preview.clone();
        legacy.product_id = 9001;
        legacy.real_product_id = 9001;
        legacy.serial_number = "DEMO-KEYBOARD-0001".into();
        legacy.device_container_id = "{DEMO-0000-0000-0000-000000000001}".into();
        let mut unrelated = legacy.clone();
        unrelated.serial_number = "USER-DEVICE".into();
        let retained = serde_json::to_value([&preview, &unrelated]).unwrap();
        let devices = vec![preview, legacy, unrelated];
        let file = WorkspaceFile::new(devices.clone(), true);
        for document in [
            serde_json::to_string(&devices).unwrap(),
            serde_json::to_string(&file).unwrap(),
        ] {
            let restored = decode_workspace(&document).unwrap();
            assert_eq!(serde_json::to_value(&restored.devices).unwrap(), retained);
        }
    }
    #[test]
    fn dashboard_layout_is_optional_and_leaves_device_data_unchanged() {
        let original = WorkspaceFile::new(
            vec![
                razer_model::product_preview::registered_preview(182).unwrap(),
                razer_model::product_preview::registered_preview(179).unwrap(),
            ],
            false,
        );
        let mut old = serde_json::to_value(&original).unwrap();
        old.as_object_mut().unwrap().remove("dashboard");
        let loaded = decode_workspace(&old.to_string()).unwrap();
        assert_eq!(loaded.dashboard, Default::default());
        let devices = serde_json::to_value(&loaded.devices).unwrap();
        let mut dashboard = razer_model::preferences::DashboardPreferences::default();
        dashboard
            .items_order
            .insert("devices".into(), vec!["second".into(), "first".into()]);
        dashboard.groups_collapsed.insert("module".into(), true);
        let saved = loaded.with_dashboard(dashboard.clone());
        let restored = decode_workspace(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(restored.dashboard, dashboard);
        assert_eq!(serde_json::to_value(restored.devices).unwrap(), devices);
        assert_eq!(restored.preferences, original.preferences);
    }
    #[test]
    fn host_order_is_optional_in_older_files_and_round_trips_with_device_data() {
        let original = WorkspaceFile::new(
            vec![
                razer_model::product_preview::registered_preview(182).unwrap(),
                razer_model::product_preview::registered_preview(179).unwrap(),
            ],
            true,
        );
        let mut old = serde_json::to_value(&original).unwrap();
        old.as_object_mut().unwrap().remove("host_tab_order");
        let mut loaded = decode_workspace(&old.to_string()).unwrap();
        assert!(loaded.host_tab_order.is_empty());
        let device_data = serde_json::to_value(&loaded.devices).unwrap();
        loaded = loaded.with_host_tab_order(vec!["host-second".into(), "host-first".into()]);
        let restored = decode_workspace(&serde_json::to_string(&loaded).unwrap()).unwrap();
        assert_eq!(restored.host_tab_order, ["host-second", "host-first"]);
        assert_eq!(serde_json::to_value(restored.devices).unwrap(), device_data);
        assert!(restored.tracking_intro_seen);
    }
    fn test_path(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-data");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{name}-{}.json", std::process::id()))
    }
    #[test]
    fn legacy_migration_preserves_device_data_and_rejects_future_versions() {
        let path = test_path("migrate");
        let devices = vec![
            razer_model::product_preview::registered_preview(182).unwrap(),
            razer_model::product_preview::registered_preview(179).unwrap(),
        ];
        std::fs::write(
            &path,
            format!("\u{feff}{}", serde_json::to_string(&devices).unwrap()),
        )
        .unwrap();
        let mut file = read_workspace(&path).unwrap().unwrap();
        assert_eq!(file.custom_colors, [None; 16]);
        file.custom_colors[0] = Some([68, 214, 44]);
        file.custom_colors[15] = Some([17, 17, 17]);
        file.tracking_intro_seen = true;
        assert_eq!(file.devices[0].serial_number, devices[0].serial_number);
        write_workspace(&path, &file).unwrap();
        let saved = read_workspace(&path).unwrap().unwrap();
        assert!(saved.tracking_intro_seen);
        assert_eq!(saved.custom_colors, file.custom_colors);
        assert_eq!(
            serde_json::to_value(&saved.devices).unwrap(),
            serde_json::to_value(&devices).unwrap()
        );
        std::fs::write(&path, r#"{"version":999,"devices":[]}"#).unwrap();
        assert!(read_workspace(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
    }
    #[test]
    fn corrupt_storage_is_reported_instead_of_becoming_an_empty_device_list() {
        let path = test_path("corrupt");
        std::fs::write(&path, "{").unwrap();
        assert!(read_workspace(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn unknown_fields_cannot_be_lost_when_loading_or_saving() {
        let file = WorkspaceFile::new(
            vec![
                razer_model::product_preview::registered_preview(182).unwrap(),
                razer_model::product_preview::registered_preview(179).unwrap(),
            ],
            false,
        );
        let mut value = serde_json::to_value(&file).unwrap();
        value["future_options"] = serde_json::json!({"enabled": true});
        assert!(decode_workspace(&value.to_string()).is_err());
        value.as_object_mut().unwrap().remove("future_options");
        value["devices"][0]["features"]["future_options"] = serde_json::json!(true);
        assert!(decode_workspace(&value.to_string()).is_err());

        let path = test_path("preserve-future");
        let original = r#"{"version":999,"devices":[]}"#;
        std::fs::write(&path, original).unwrap();
        assert!(write_workspace(&path, &file).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn aliases_and_new_shortcuts_survive_strict_legacy_loading() {
        let mut value = serde_json::to_value(WorkspaceFile::new(
            vec![
                razer_model::product_preview::registered_preview(182).unwrap(),
                razer_model::product_preview::registered_preview(179).unwrap(),
            ],
            false,
        ))
        .unwrap();
        let device = value["devices"][0].as_object_mut().unwrap();
        let edition = device.remove("edition_id").unwrap();
        device.insert("editionId".into(), edition);
        let layout = device.remove("layout_id").unwrap();
        device.insert("layoutId".into(), layout);
        value["shortcuts"] = serde_json::json!([{
            "id":"example", "input":"KEY_K", "modifiers":["CTRL"], "hypershift":false,
            "output":{"kind":"text","text":"你好\n保存"}
        }]);
        let restored = decode_workspace(&value.to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(&restored.shortcuts).unwrap(),
            value["shortcuts"]
        );
        value.as_object_mut().unwrap().remove("shortcuts");
        assert!(
            decode_workspace(&value.to_string())
                .unwrap()
                .shortcuts
                .is_empty()
        );
    }
}
