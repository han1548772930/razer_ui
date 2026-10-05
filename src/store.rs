//! Local, versioned storage. This format does not claim compatibility with Synapse profiles.
//! Preserve legacy DeviceFeatures and migrate old top-level Vec<Device> files on save.
use crate::model::Device;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
const VERSION: u32 = 2;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFile {
    version: u32,
    pub(crate) devices: Vec<Device>,
    #[serde(default)]
    pub(crate) tracking_intro_seen: bool,
    #[serde(default)]
    pub(crate) shortcuts: Vec<crate::features::shortcuts::Shortcut>,
    #[serde(default)]
    pub(crate) macros: crate::features::macro_library::MacroLibraryFile,
    #[serde(default)]
    pub(crate) preferences: crate::preferences::AppPreferences,
    #[serde(default)]
    pub(crate) custom_colors: crate::preferences::CustomColorSlots,
    #[serde(default)]
    pub(crate) host_tab_order: Vec<String>,
    #[serde(default)]
    pub(crate) dashboard: crate::preferences::DashboardPreferences,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) module_services: Option<crate::features::module_service::ModuleServiceSnapshot>,
}
impl WorkspaceFile {
    pub(crate) fn new(devices: Vec<Device>, tracking_intro_seen: bool) -> Self {
        Self {
            version: VERSION,
            devices,
            tracking_intro_seen,
            shortcuts: vec![],
            macros: Default::default(),
            preferences: crate::preferences::AppPreferences::default(),
            custom_colors: [None; 16],
            host_tab_order: vec![],
            dashboard: Default::default(),
            module_services: None,
        }
    }
    pub(crate) fn with_shortcuts(
        mut self,
        shortcuts: Vec<crate::features::shortcuts::Shortcut>,
    ) -> Self {
        self.shortcuts = shortcuts;
        self
    }
    pub(crate) fn with_preferences(
        mut self,
        preferences: crate::preferences::AppPreferences,
    ) -> Self {
        self.preferences = preferences;
        self
    }
    pub(crate) fn with_macros(
        mut self,
        macros: crate::features::macro_library::MacroLibraryFile,
    ) -> Self {
        self.macros = macros;
        self
    }
    pub(crate) fn with_custom_colors(
        mut self,
        colors: crate::preferences::CustomColorSlots,
    ) -> Self {
        self.custom_colors = colors;
        self
    }
    pub(crate) fn with_host_tab_order(mut self, order: Vec<String>) -> Self {
        self.host_tab_order = order;
        self
    }
    pub(crate) fn with_dashboard(
        mut self,
        dashboard: crate::preferences::DashboardPreferences,
    ) -> Self {
        self.dashboard = dashboard;
        self
    }
    pub(crate) fn with_module_services(
        mut self,
        snapshot: Option<crate::features::module_service::ModuleServiceSnapshot>,
    ) -> Self {
        self.module_services = snapshot;
        self
    }
}
pub fn store_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("razer_ui")
        .join("profiles.json")
}
pub(crate) fn read_workspace(path: &Path) -> anyhow::Result<Option<WorkspaceFile>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    decode_workspace(&text).map(Some)
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
    let file = if text.trim_start().starts_with('[') {
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
    crate::features::shortcuts::validate_stored_shortcuts(&file.shortcuts)
        .map_err(anyhow::Error::msg)?;
    file.preferences.validate().map_err(anyhow::Error::msg)?;
    file.macros.validate().map_err(anyhow::Error::msg)?;
    Ok(file)
}
pub(crate) fn write_workspace(path: &Path, file: &WorkspaceFile) -> anyhow::Result<()> {
    file.preferences.validate().map_err(anyhow::Error::msg)?;
    file.macros.validate().map_err(anyhow::Error::msg)?;
    // Recheck the current disk file, not just the version loaded at startup.
    // A newer app or external edit must not be silently replaced by this one.
    let previous = match std::fs::read(path) {
        Ok(bytes) => {
            decode_workspace(std::str::from_utf8(&bytes)?)?;
            Some(bytes)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    crate::features::shortcuts::validate_stored_shortcuts(&file.shortcuts)
        .map_err(anyhow::Error::msg)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_vec_pretty(file)?;
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    use std::io::Write;
    let mut output = std::fs::File::create(&tmp)?;
    output.write_all(&text)?;
    output.sync_all()?;
    drop(output);
    let current = match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        current == previous,
        "配置文件在保存期间被其他程序修改；本次更改未写入，请重新读取后再保存。"
    );
    if let Some(previous) = previous {
        std::fs::write(path.with_extension("json.bak"), previous)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
// Kept for the existing explicit CLI self-test.
pub fn load_from(path: &Path) -> Option<Vec<Device>> {
    read_workspace(path).ok().flatten().map(|s| s.devices)
}
pub fn save_to(path: &Path, devices: &[Device]) -> anyhow::Result<()> {
    write_workspace(path, &WorkspaceFile::new(devices.to_vec(), false))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dashboard_layout_is_optional_and_leaves_device_data_unchanged() {
        let original = WorkspaceFile::new(crate::model::measured_devices(), false);
        let mut old = serde_json::to_value(&original).unwrap();
        old.as_object_mut().unwrap().remove("dashboard");
        let loaded = decode_workspace(&old.to_string()).unwrap();
        assert_eq!(loaded.dashboard, Default::default());
        let devices = serde_json::to_value(&loaded.devices).unwrap();
        let mut dashboard = crate::preferences::DashboardPreferences::default();
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
        let original = WorkspaceFile::new(crate::model::measured_devices(), true);
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
        let devices = crate::model::measured_devices();
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
        let file = WorkspaceFile::new(crate::model::measured_devices(), false);
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
        let mut value =
            serde_json::to_value(WorkspaceFile::new(crate::model::measured_devices(), false))
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
