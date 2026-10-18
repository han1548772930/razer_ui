//! Read-only filesystem lookup. Never loads a library or validates a live device.
use razer_catalog::native_library::{self, NativeLibrary, NativeResource};
use std::path::{Path, PathBuf};

/// 引擎 DLL 的两处安装位置。
#[derive(Debug, Clone, Default)]
pub struct EnginePaths {
    /// `C:\Program Files\Razer\RazerAppEngine\app-<ver>\CommonDLL`
    /// 文件名稳定、无版本号，优先使用。
    pub common_dll: Option<PathBuf>,
    /// `%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Apps\Common`
    /// 文件名带 `_vX.Y.Z.W` 版本号，且有按 productId 分的子目录。
    pub apps_common: Option<PathBuf>,
}

impl EnginePaths {
    /// 定位两处安装位置。**只读文件系统，不加载任何 DLL**，任何线程可安全调用。
    ///
    /// 两处都可能不存在（没装雷云），所以两个字段都是 `Option`，
    /// 而不是「找不到就报错」——没装雷云时应用仍应能起来。
    pub fn discover() -> Self {
        let program_files = std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
        let engine_root = program_files.join("Razer").join("RazerAppEngine");

        // Compare numeric components: app-4.0.1000 is newer than app-4.0.999.
        // Directory discovery is not proof of the loaded host or a live service.
        let common_dll = std::fs::read_dir(&engine_root)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                let version = numeric_version(name.to_str()?.strip_prefix("app-")?)?;
                let dir = entry.path().join("CommonDLL");
                dir.is_dir().then_some((version, dir))
            })
            .max_by_key(|(name, _)| name.clone())
            .map(|(_, dir)| dir);

        let apps_common = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|root| {
                root.join("Razer")
                    .join("RazerAppEngine")
                    .join("User Data")
                    .join("Apps")
                    .join("Common")
            })
            .filter(|dir| dir.is_dir());

        Self {
            common_dll,
            apps_common,
        }
    }

    /// 两处位置是否都不可用（用于界面显示「雷云是否安装」）。
    pub fn is_empty(&self) -> bool {
        self.common_dll.is_none() && self.apps_common.is_none()
    }

    /// 找某个引擎的 DLL 文件。**只读文件系统**，不加载。
    ///
    /// 查找顺序：`CommonDLL` → `Apps\Common`（含一层 productId 子目录）。
    ///
    /// 文件名匹配规则：DLL 可能是 `lighting_driver.dll`，也可能是带版本号的
    /// `lighting_driver_v4.0.0.0.dll`，所以接受 `<stem>.dll` 与 `<stem>_v*.dll`；
    /// 用 `_v` 作分隔而不是 `starts_with(stem)`，避免 `lighting_driver`
    /// 误匹配到 `lighting_driver_helper`。
    pub fn resolve(&self, stem: &str) -> Option<PathBuf> {
        for dir in [self.common_dll.as_ref(), self.apps_common.as_ref()]
            .into_iter()
            .flatten()
        {
            let mut candidates = Vec::new();
            collect_dlls(dir, stem, 0, &mut candidates);
            candidates.sort_by_key(|path| dll_rank(path, stem));
            if let Some(path) = candidates.pop() {
                return Some(path);
            }
        }
        None
    }

    /// Current packaged host main.js loads these core services by their exact
    /// CommonDLL filenames. Apps/Common is only used by its explicit debug mode.
    /// Do not silently initialize a product-scoped or downloaded substitute.
    pub fn resolve_host_service(&self, stem: &str) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            matches!(stem, "mapping_engine" | "simple_service"),
            "未知宿主服务 {stem}"
        );
        let directory = self
            .common_dll
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("未找到已安装宿主的 CommonDLL 目录"))?;
        let path = directory.join(format!("{stem}.dll"));
        anyhow::ensure!(path.is_file(), "宿主服务文件不存在：{}", path.display());
        Ok(path)
    }
}

fn numeric_version(value: &str) -> Option<Vec<u32>> {
    let parts = value
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()
        .ok()?;
    (parts.len() >= 2).then_some(parts)
}

fn dll_rank(path: &Path, stem: &str) -> (bool, Vec<u32>, PathBuf) {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let stem = stem.to_ascii_lowercase();
    let exact = name == format!("{stem}.dll");
    let version = name
        .strip_prefix(&format!("{stem}_v"))
        .and_then(|name| name.strip_suffix(".dll"))
        .and_then(numeric_version)
        .unwrap_or_default();
    (exact, version, path.to_owned())
}

/// 在 `dir` 下收集匹配 `stem` 的 DLL。
///
/// `depth` 限制递归层数：`Apps\Common` 下有一层 productId 目录，再深就不找了。
fn collect_dlls(dir: &Path, stem: &str, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let stem = stem.to_ascii_lowercase();
    let exact = format!("{stem}.dll");
    let versioned = format!("{stem}_v");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 1 {
                collect_dlls(&path, &stem, depth + 1, out);
            }
            continue;
        }
        let Some(name) = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
        else {
            continue;
        };
        if name == exact
            || name
                .strip_prefix(&versioned)
                .and_then(|name| name.strip_suffix(".dll"))
                .and_then(numeric_version)
                .is_some()
        {
            out.push(path);
        }
    }
}

/// Exact paths produced by the current host's manifest resource rule. No name
/// prefix search, version substitution, installation or DLL load occurs here.
pub fn resource_candidates(
    library: &NativeLibrary,
    product_id: Option<u32>,
) -> Vec<(&NativeResource, PathBuf)> {
    let installed = EnginePaths::discover();
    let user_data = host_user_data_root().ok().map(|root| root.join("Apps"));
    let mut candidates = Vec::new();
    for resource in &library.resources {
        for binding in &resource.bindings {
            if binding.product_id != product_id {
                continue;
            }
            let Some(name) = binding.static_relative_path() else {
                continue;
            };
            let relative = PathBuf::from(name);
            if relative.is_absolute()
                || relative.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir | std::path::Component::Prefix(_)
                    )
                })
            {
                continue;
            }
            let path = if let Some(file) = name.strip_prefix("CommonDLL/") {
                installed
                    .common_dll
                    .as_ref()
                    .map(|directory| directory.join(file))
            } else {
                user_data.as_ref().map(|directory| directory.join(relative))
            };
            if let Some(path) = path {
                candidates.push((resource, path));
            }
        }
    }
    candidates
}

pub fn native_library_candidates(
    id: &str,
    product_id: Option<u32>,
) -> Option<Vec<(String, PathBuf)>> {
    let library = native_library::find(id)?;
    let installed = EnginePaths::discover();
    let programs_root = installed
        .common_dll
        .as_ref()
        .and_then(|directory| directory.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Razer\RazerAppEngine"));
    let user_data_root = installed
        .apps_common
        .as_ref()
        .and_then(|directory| directory.parent())
        .and_then(|directory| directory.parent())
        .map(PathBuf::from)
        .or_else(|| host_user_data_root().ok())?;
    Some(
        native_library::candidate_paths(library, product_id, &programs_root, &user_data_root)
            .into_iter()
            .map(|(rule, path)| (format!("{rule:?}"), path))
            .collect(),
    )
}

/// The current monitor's client, server and server-side dependency are one
/// installed resource group. These paths never fall back to analysis inputs.
#[derive(Debug, Clone)]
pub struct SystemMonitorPaths {
    pub client_dll: PathBuf,
    pub server_dll: PathBuf,
    pub server_executable: PathBuf,
    pub helper_folder: String,
    pub helper_name: String,
}

pub fn system_monitor_paths(product_id: u32) -> anyhow::Result<SystemMonitorPaths> {
    fn installed_resource(id: &str, product_id: u32) -> anyhow::Result<PathBuf> {
        let library = native_library::find(id).ok_or_else(|| {
            anyhow::anyhow!("System monitor resource is absent from the catalog: {id}")
        })?;
        resource_candidates(library, Some(product_id))
            .into_iter()
            .chain(resource_candidates(library, None))
            .map(|(_, path)| path)
            .find(|path| path.is_file())
            .ok_or_else(|| anyhow::anyhow!("System monitor installed resource is missing: {id}"))
    }

    let client_dll = installed_resource("RzSystemCommon", product_id)?;
    let server_dll = installed_resource("RzSystemService", product_id)?;
    let server_executable = installed_resource("RzDLLService", product_id)?;
    let server_dependency = installed_resource("cpuidsdk64Common", product_id)?;
    let directory = server_executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("System monitor helper has no parent directory"))?;
    anyhow::ensure!(
        [&client_dll, &server_dll, &server_dependency]
            .into_iter()
            .all(|path| path.parent() == Some(directory)),
        "System monitor installed resources do not share their manifest directory"
    );
    let applications = host_user_data_root()?.join("Apps");
    let helper_folder = directory
        .strip_prefix(applications)
        .map_err(|_| anyhow::anyhow!("System monitor helper is outside installed Apps resources"))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("System monitor helper folder is not valid Unicode"))?
        .replace('/', "\\");
    let helper_name = server_executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("System monitor helper filename is not valid Unicode"))?
        .to_owned();
    Ok(SystemMonitorPaths {
        client_dll,
        server_dll,
        server_executable,
        helper_folder,
        helper_name,
    })
}

/// Current host constants.js userDataDir. Installation observations and native
/// resource discovery share this source-confirmed platform path.
pub fn host_user_data_root() -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|root| root.join("Razer/RazerAppEngine/User Data"))
            .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is unavailable"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|root| root.join("Library/Application Support/Razer/RazerAppEngine/User Data"))
            .ok_or_else(|| anyhow::anyhow!("The user home directory is unavailable"));
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    anyhow::bail!("The current Razer host has no installation path for this platform")
}

pub fn resolve_path(stem: &str) -> Option<PathBuf> {
    EnginePaths::discover().resolve(stem)
}
