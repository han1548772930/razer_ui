//! 本地配置持久化。
//!
//! # 为什么必须自建
//!
//! 逆向确认（`docs/FEATURES.md` I6 / D4，证据 `docs/re/01-ipc-api-surface.md` §20.8）：
//! 雷云的 IPC 层**不提供任何配置持久化**——`memory_storage` / `window_storage` /
//! `keyStorage` 全是按窗口 URL 索引的**内存** Map，窗口销毁即清空；
//! 真正的落盘在远程前端与 C++ 引擎内部，我们无法复用。
//!
//! 因此替代 UI 必须自建配置存储。

use std::path::PathBuf;

use crate::model::Device;

/// 配置文件名。
const FILE_NAME: &str = "profiles.json";

/// 配置存储位置。
///
/// 优先 `%APPDATA%\razer_ui\profiles.json`；拿不到环境变量时退化到当前目录，
/// 保证在任何环境下都能落盘而不是静默失败。
pub fn store_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("razer_ui").join(FILE_NAME)
}

/// 读取已保存的设备配置。
///
/// 文件不存在或内容损坏时返回 `None`，由调用方回退到本机实测快照。
pub fn load() -> Option<Vec<Device>> {
    load_from(&store_path())
}

/// 从指定路径读取（便于自检与测试）。
pub fn load_from(path: &std::path::Path) -> Option<Vec<Device>> {
    let text = std::fs::read_to_string(path).ok()?;
    // 容忍 UTF-8 BOM（外部工具写出来的文件常带）。
    let text = text.trim_start_matches('\u{feff}');
    match serde_json::from_str(text) {
        Ok(devices) => Some(devices),
        Err(err) => {
            eprintln!("配置文件解析失败（{}）：{err}", path.display());
            None
        }
    }
}

/// 保存设备配置，返回实际写入的路径。
pub fn save(devices: &[Device]) -> anyhow::Result<PathBuf> {
    let path = store_path();
    save_to(&path, devices)?;
    Ok(path)
}

/// 写入指定路径（便于自检与测试）。
pub fn save_to(path: &std::path::Path, devices: &[Device]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(devices)?;
    std::fs::write(path, text)?;
    Ok(())
}
