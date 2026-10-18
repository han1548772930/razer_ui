//! 雷云原生引擎 DLL 的定位与加载。
//!
//! 找到文件不代表 DLL 的 ABI 或功能已验证。剩余适配器及替代边界
//! 记录在 `docs/re/architecture.md`，实际比较由独立维护入口管理。
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// 一个已加载的引擎 DLL。
///
/// # 安全边界
///
/// [`EngineLibrary::load`] 会执行第三方 `DllMain`，可能阻塞。
/// 生产调用必须遵守独立 worker 边界；分析比较遵守维护入口的作用域和清理契约。
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
    /// 调用方还必须保证库覆盖函数指针、借用缓冲区和回调的完整生命周期。
    /// 当前替代范围与验证边界见 `docs/re/architecture.md`。
    pub unsafe fn func<T>(&self, name: &str) -> Option<T> {
        // SAFETY: 由调用方保证 `T` 的签名正确（见上）。
        let symbol = unsafe { self.lib.get::<T>(name.as_bytes()) }.ok()?;
        // 把函数指针复制出来：`Symbol` 的生命周期绑定在 `self.lib` 上，
        // 而调用方要的是一个可自由保存的值。
        Some(unsafe { std::ptr::read(&*symbol) })
    }
}
