//! Existing source-described engine declarations and loading restrictions. No library execution.
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
    BLOCKING_ENGINES
        .iter()
        .any(|name| name.eq_ignore_ascii_case(stem))
}

/// 引擎目录。**只读静态数据，不碰 DLL**，任何线程都可安全调用。
pub fn catalog() -> &'static [EngineSpec] {
    ENGINES
}

/// 按 DLL 名（stem）找规格。
pub fn spec_for(stem: &str) -> Option<&'static EngineSpec> {
    ENGINES.iter().find(|spec| spec.stem == stem)
}
