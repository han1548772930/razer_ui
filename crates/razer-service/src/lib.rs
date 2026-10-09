//! 雷云后端。
//!
//! 查询通过独立 worker 隔离 DLL 生命周期，UI 线程只接收观察结果。
//! 声明目录归 razer-catalog，文件定位归 razer-platform；`probe_engine` 会加载 DLL，
//! 不属于当前允许的开发验证方式。源码、ABI 与消费者状态见
//! `docs/re/dll-readonly-inventory.md`。设备与服务写回单独集成。
#![allow(dead_code)]
pub mod native_query;
pub mod native_read;
pub mod runtime;

pub mod dll;
pub mod lighting;

use std::path::PathBuf;

use dll::EngineLibrary;
use razer_catalog::engines::EngineSpec;

/// 一次引擎探测的结果。
///
/// 分清三种情况，命令行与界面据此给出不同结论：
///
/// | 情况 | `path` | `error` | 含义 |
/// |---|---|---|---|
/// | 找不到文件 | `None` | `Some` | 该版本的雷云没装这个引擎 |
/// | 加载失败 | `Some` | `Some` | 文件在，但 `LoadLibrary` 失败（位数不符 / 缺依赖） |
/// | 加载成功 | `Some` | `None` | 拿到句柄；`missing` 列出缺失的导出 |
#[derive(Debug, Clone)]
pub struct ProbeStatus {
    /// 引擎名（DLL 文件主干名）。
    pub stem: &'static str,
    /// 解析到的 DLL 路径。
    pub path: Option<PathBuf>,
    /// 已找到的导出数量。
    pub found: usize,
    /// 期望但**没有**找到的导出。
    ///
    /// 非空**不代表失败**：不同雷云版本的导出集合不同，
    /// 因此如实列出，由调用方判断够不够用。
    pub missing: Vec<&'static str>,
    /// 失败原因（找不到文件或加载失败）。
    pub error: Option<String>,
}

impl ProbeStatus {
    /// 是否拿到了 DLL 句柄。
    pub fn loaded(&self) -> bool {
        self.path.is_some() && self.error.is_none()
    }

    /// 一行结论。
    pub fn summary(&self) -> String {
        if let Some(err) = &self.error {
            return format!("失败：{err}");
        }
        if self.path.is_none() {
            return "未找到".to_string();
        }
        let total = self.found + self.missing.len();
        if self.missing.is_empty() {
            format!("已加载 · {total}/{total} 个导出齐全")
        } else {
            format!(
                "已加载 · {}/{} 个导出（缺：{}）",
                self.found,
                total,
                self.missing.join(", ")
            )
        }
    }
}

/// 真正加载一个引擎 DLL 并检查其导出。
///
/// 此入口执行第三方 `DllMain`，不是静态 PE 检查，不属于当前允许的开发验证。
/// 加载可能阻塞，不得在 UI 线程或 `render()` 中调用。
pub fn probe_engine(spec: &EngineSpec) -> ProbeStatus {
    let Some(path) = razer_platform::native_paths::EnginePaths::discover().resolve(spec.stem)
    else {
        return ProbeStatus {
            stem: spec.stem,
            path: None,
            found: 0,
            missing: spec.symbols.to_vec(),
            error: Some(format!("未找到 {}.dll", spec.stem)),
        };
    };

    let lib = match EngineLibrary::load(&path) {
        Ok(lib) => lib,
        Err(err) => {
            return ProbeStatus {
                stem: spec.stem,
                path: Some(path),
                found: 0,
                missing: spec.symbols.to_vec(),
                error: Some(err.to_string()),
            };
        }
    };

    // 逐个查导出。`*mut c_void` 只是个占位类型，这里只查符号是否存在、
    // 不会按任何签名调用这些导出；此前加载库仍已执行 DLL 入口。
    let mut missing = Vec::new();
    let mut found = 0;
    for symbol in spec.symbols {
        // SAFETY: 见上——只取地址，不调用。
        if unsafe { lib.func::<*mut std::ffi::c_void>(symbol) }.is_some() {
            found += 1;
        } else {
            missing.push(*symbol);
        }
    }

    ProbeStatus {
        stem: spec.stem,
        path: Some(path),
        found,
        missing,
        error: None,
    }
}
