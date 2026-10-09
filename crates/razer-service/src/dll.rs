//! 雷云原生引擎 DLL 的定位与加载。
//!
//! 保留现有目录发现与加载实现；找到文件不代表当前 DLL 的 ABI 已验证。
//! 当前官方封装、查询链与未核实边界见 `docs/re/dll-readonly-inventory.md`。
//! 开发验证只静态读取文件，不加载或执行 DLL。
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use razer_platform::native_paths::EnginePaths;

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
