//! 雷云后端。
//!
//! # 方案 2：复用雷云现有后端引擎
//!
//! 逆向确认（`docs/re/dll-readonly-inventory.md` §21）：雷云的行为几乎全在
//! 与 Electron 无关的原生 DLL 里，因此 Rust 可以**绕过整个 Electron 层**
//! 直接复用它们。
//!
//! ```text
//! 本项目 UI (gpui-kit)
//!        │  直接 FFI
//!        ▼
//! 雷云原生引擎 DLL ──► 雷云驱动 / HID ──► 硬件
//! ```
//!
//! # ⚠️ 安全边界（实测教训）
//!
//! **绝对不要在 UI 线程上加载这些 DLL。** 实测把探测放进 `render()` 会让
//! 整个界面卡死；逐个验证后确认是 `SysUtilsNative.dll` 的 `DllMain` 会阻塞
//! （见 `--probe <stem>` 命令行用法）。
//!
//! 因此本模块的设计约束是：
//! 1. [`catalog`] / [`resolve_path`] 只读文件系统，**任何线程都可安全调用**；
//! 2. [`probe_engine`] 会真正 `LoadLibrary`，**只能在能容忍阻塞的地方调用**
//!    （命令行 `--probe`，将来是后台线程或独立子进程）。
// 该模块的 API 面是**故意完整**的：逐条对应逆向雷云得到的功能层/模型定义，
// 即使界面暂未调用每个成员也保留，使模型与逆向结果一一对应。
// 这只用于领域模型模块；`src/pages/**` 里不存在这个豁免。
#![allow(dead_code)]
pub(crate) mod runtime;
pub(crate) mod system;

pub mod dll;
pub mod lighting;
pub mod protocol;

use std::path::PathBuf;

use dll::{EngineLibrary, EnginePaths};

/// 一个引擎 DLL 的规格。
///
/// 符号清单来自 `dumpbin /exports`（原始输出见 `.ref/notes/exports_*.txt`）。
pub struct EngineSpec {
    /// 中文名
    pub label: &'static str,
    /// DLL 文件名（不含扩展名与版本号）
    pub stem: &'static str,
    /// 它负责什么
    pub purpose: &'static str,
    /// 用来验证可用性的关键符号。
    ///
    /// `simple_service` 与 `mapping_engine` 的导出是 **C++ 修饰名**，
    /// 必须按修饰名逐字查找（`GetProcAddress` 匹配完整名字）；
    /// 其余是 extern "C" 的普通名字。
    pub symbols: &'static [&'static str],
}

/// 引擎清单。顺序会影响 `--probe` 的输出顺序，因此把**已知会阻塞的放最后**。
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
            "?mappingEngineInitialize@@YAXP6AXXZ@Z",
            "?mappingEngineShutdown@@YAXP6AXXZ@Z",
            "?getGlobalMode@@YAXP6AX_NPEBD1@Z@Z",
            "?getDeviceMode@@YAXPEBDP6AX_N00@Z@Z",
        ],
    },
    EngineSpec {
        label: "音频与进程",
        stem: "simple_service",
        purpose: "默认音频设备、音量、侧音，以及进程启动",
        symbols: &[
            "?simpleGetVersionInfo@@YAXP6AX_NPEBD1@Z@Z",
            "?simpleEnumerateAudioDevices@@YAXP6AX_NPEBD1@Z@Z",
            "?simpleGetDefaultSpeaker@@YAXP6AX_NPEBD110E@Z@Z",
            "?simpleGetDefaultMicrophone@@YAXP6AX_NPEBD110E@Z@Z",
            "?simpleGetSpeakerVolume@@YAXPEBDP6AX_N001E@Z@Z",
            "?simpleGetMicrophoneVolume@@YAXPEBDP6AX_N001E@Z@Z",
            "?simpleGetSidetoneVolume@@YAXPEBDP6AX_N001E@Z@Z",
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

/// 实测会在 `DllMain` 里**永久阻塞**的引擎，禁止加载。
///
/// 证据：`razer_ui --probe SysUtilsNative` 等待 20 秒后仍无任何输出，
/// 进程只能强杀。因此本模块直接拒绝加载它。
const BLOCKING_ENGINES: &[&str] = &["SysUtilsNative"];

/// 该引擎是否已知会在加载时阻塞。
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
/// # ⚠️ 只能在能容忍阻塞的地方调用
///
/// 这会执行第三方 `DllMain`；实测 `SysUtilsNative.dll` 的 `DllMain` 会**永久阻塞**。
/// 所以：
///
/// - **绝不要**在 UI 线程 / `render()` 里调用；
/// - 命令行 `--probe` 会先 `print!` 再调用，这样即使卡住也能从输出看出卡在哪一个。
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
    // 不会按任何签名去调用，因此没有副作用。
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
