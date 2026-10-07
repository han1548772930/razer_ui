//! 雷云后端。
//!
//! 查询通过独立 worker 隔离 DLL 生命周期，UI 线程只接收观察结果。
//! `catalog` / `resolve_path` 只读文件系统；`probe_engine` 会加载 DLL，
//! 不属于当前允许的开发验证方式。源码、ABI 与消费者状态见
//! `docs/re/dll-readonly-inventory.md`。设备与服务写回单独集成。
#![allow(dead_code)]
pub(crate) mod device_changes;
pub(crate) mod device_identity;
pub(crate) mod device_reads;
pub(crate) mod discovery;
pub(crate) mod receiver_capabilities;
mod receiver_catalog;
pub(crate) mod receiver_protocol;
pub(crate) mod runtime;
pub(crate) mod system;

pub mod dll;
pub mod lighting;
pub mod protocol;

use std::path::PathBuf;

use dll::{EngineLibrary, EnginePaths};

/// 一个引擎 DLL 的规格。
///
/// 符号用于定位导出；当前声明与静态证据见 DLL 接入契约，名称本身不证明 ABI。
pub struct EngineSpec {
    /// 中文名
    pub label: &'static str,
    /// DLL 文件名（不含扩展名与版本号）
    pub stem: &'static str,
    /// 它负责什么
    pub purpose: &'static str,
    /// 用来验证可用性的关键符号。
    ///
    /// Current mapping/simple wrappers use undecorated aliases, also present in
    /// the statically inspected DLLs. Export presence alone does not prove ABI.
    pub symbols: &'static [&'static str],
}

/// 引擎清单。顺序同时决定 `--probe` 的输出顺序。
const ENGINES: &[EngineSpec] = &[
    EngineSpec {
        label: "灯光写出",
        stem: "lighting_driver",
        purpose: "Chroma 灯光的最低层写出，JSON 命令接口",
        symbols: &[
            "Startup",
            "Shutdown",
            "Configure",
            "ConfigureInMem",
            "FreeString",
            "GetDllVersion",
            "HookLightingCallback",
            "SetWriteFFICallback",
        ],
    },
    EngineSpec {
        label: "灯光引擎",
        stem: "RzLightingEngineApi",
        purpose: "Chroma 效果引擎：设备/引擎生命周期与事件轮询",
        symbols: &[
            "CreateLightingEngine",
            "DestroyLightingEngine",
            "CreateLightingDevice",
            "DestroyLightingDevice",
            "RzLightingApi",
            "RzLightingApiNoReturn",
            "PollEvents",
            "GetFeatureSet",
            "SetEventInterface",
            "SetAllocator",
            "UsePollingMode",
            "SetOperatingMode",
        ],
    },
    EngineSpec {
        label: "按键映射与宏",
        stem: "mapping_engine",
        purpose: "按键重映射、Hypershift、Snap Tap、宏录制、全局快捷键",
        symbols: &[
            "mappingEngineInitialize",
            "mappingEngineShutdown",
            "getGlobalMode",
            "getDeviceMode",
        ],
    },
    EngineSpec {
        label: "音频与进程",
        stem: "simple_service",
        purpose: "默认音频设备、音量、侧音，以及进程启动",
        symbols: &[
            "simpleGetVersionInfo",
            "simpleEnumerateAudioDevices",
            "simpleGetDefaultSpeaker",
            "simpleGetDefaultMicrophone",
            "simpleGetSpeakerVolume",
            "simpleGetMicrophoneVolume",
            "simpleGetSidetoneVolume",
        ],
    },
    EngineSpec {
        label: "系统工具",
        stem: "SysUtilsNative",
        purpose: "机器 ID、驱动信息、前台窗口监控、应用图标",
        symbols: &[
            "GetDLLVersion",
            "GetMachineId",
            "GetDriverInfo",
            "BringProcessWindowToFront",
            "isWindowsDynamicLightingEnabled",
            "GetInstalledRazerAppEngineProduct",
        ],
    },
];

/// 保留的加载限制。当前尚未核实这些引擎的完整 ABI 与加载生命周期；
/// 不能把目录或导出存在当作可执行查询的依据。
const BLOCKING_ENGINES: &[&str] = &["SysUtilsNative"];

/// 该引擎是否受到加载限制。
pub fn blocks_load(stem: &str) -> bool {
    BLOCKING_ENGINES.contains(&stem)
}

/// 引擎目录。**只读静态数据，不碰 DLL**，任何线程都可安全调用。
pub fn catalog() -> &'static [EngineSpec] {
    ENGINES
}

/// 按 DLL 名（stem）找规格。
pub fn spec_for(stem: &str) -> Option<&'static EngineSpec> {
    ENGINES.iter().find(|spec| spec.stem == stem)
}

/// 解析某个引擎的 DLL 路径。**只读文件系统，不加载 DLL**，线程安全。
pub fn resolve_path(stem: &str) -> Option<PathBuf> {
    EnginePaths::discover().resolve(stem)
}

/// 引擎 DLL 的两处安装位置。**只读文件系统**，线程安全。
pub fn engine_paths() -> EnginePaths {
    EnginePaths::discover()
}

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
    let Some(path) = resolve_path(spec.stem) else {
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
