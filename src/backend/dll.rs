//! 雷云原生引擎 DLL 的定位与加载。
//!
//! 保留现有目录发现与加载实现；找到文件不代表当前 DLL 的 ABI 已验证。
//! 当前官方封装、查询链与未核实边界见 `docs/re/dll-readonly-inventory.md`。
//! 开发验证只静态读取文件，不加载或执行 DLL。
#![allow(dead_code)]

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
    pub(crate) fn resolve_host_service(&self, stem: &str) -> anyhow::Result<PathBuf> {
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

/// 一个已加载的引擎 DLL。
///
/// # 安全边界
///
/// [`EngineLibrary::load`] 会执行第三方 `DllMain`，可能阻塞。
/// 加载不属于当前静态开发验证；运行时调用必须遵守独立 worker 边界。
pub struct EngineLibrary {
    lib: libloading::Library,
    path: PathBuf,
}

impl EngineLibrary {
    /// 加载指定路径的 DLL。
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        // SAFETY: 调用方负责保证不在 UI 线程上调用——第三方 DllMain 可能阻塞。
        let lib = unsafe { libloading::Library::new(path) }
            .map_err(|err| anyhow::anyhow!("加载 {} 失败：{err}", path.display()))?;
        Ok(Self {
            lib,
            path: path.to_path_buf(),
        })
    }

    /// 按引擎名加载（先 [`EnginePaths::resolve`]，再 [`Self::load`]）。
    pub fn discover(paths: &EnginePaths, stem: &str) -> anyhow::Result<Self> {
        let path = paths
            .resolve(stem)
            .ok_or_else(|| anyhow::anyhow!("未找到 {stem}.dll"))?;
        Self::load(&path)
    }

    /// 已加载的文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 取一个导出函数。
    ///
    /// `None` 表示该 DLL 没有这个导出——**不是**「加载失败」。
    /// 不同版本的雷云导出集合不同，调用方据此降级。
    ///
    /// # Safety
    ///
    /// 调用方必须保证 `T` 与该 DLL 的真实导出签名一致：签名不符会损坏栈。
    /// Signatures require the current wrapper and callback contract; a PE export
    /// name alone does not prove ABI. See docs/re/dll-readonly-inventory.md.
    pub unsafe fn func<T>(&self, name: &str) -> Option<T> {
        // SAFETY: 由调用方保证 `T` 的签名正确（见上）。
        let symbol = unsafe { self.lib.get::<T>(name.as_bytes()) }.ok()?;
        // 把函数指针复制出来：`Symbol` 的生命周期绑定在 `self.lib` 上，
        // 而调用方要的是一个可自由保存的值。
        Some(unsafe { std::ptr::read(&*symbol) })
    }
}
