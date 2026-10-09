//! `lighting_driver.dll` 绑定 —— Chroma 灯光的最底层写出。
//!
//! 函数签名完全来自实测的 JS 包装层
//! `electron/modules/lighting/ffiLightingDriver.js`：
//!
//! ```js
//! { Startup:["void",[]], Shutdown:["void",[]],
//!   Configure:["char*",["string"]], FreeString:["void",["pointer"]],
//!   HookLightingCallback:["bool",["string","string","pointer"]],
//!   SetWriteFFICallback:["void",["pointer"]], GetDllVersion:["char*",[]] }
//! ```
//!
//! `Configure` 收一个 **JSON 字符串**、吐一个 **JSON 字符串**，
//! 是整套引擎里最干净、最容易复用的接口。
//!
//! # ⚠️ 调用时机
//!
//! 雷云此刻正在运行并持有灯光设备。本模块**不会自动调用** `Startup`/`Configure`；
//! 需要显式调用 [`LightingDriver::startup`] 与 [`LightingDriver::configure`]。
//! 与运行中的雷云同时使用会有设备争用，请先退出雷云再试。
//!
//! 注意：`pub fn` 标记 `#[allow(dead_code)]` 是因为 UI 尚未接入这些调用，
//! 它们正是方案 2 后续要用的入口。

#![allow(dead_code)]

use std::ffi::{CStr, CString, c_char};

use super::dll::EngineLibrary;
use razer_platform::native_paths::EnginePaths;

/// `void Startup()`
type FnStartup = unsafe extern "C" fn();
/// `void Shutdown()`
type FnShutdown = unsafe extern "C" fn();
/// `char *Configure(const char *json)`
type FnConfigure = unsafe extern "C" fn(*const c_char) -> *mut c_char;
/// `void FreeString(char *ptr)`
type FnFreeString = unsafe extern "C" fn(*mut c_char);
/// `char *GetDllVersion()`
type FnGetDllVersion = unsafe extern "C" fn() -> *mut c_char;

/// 已加载的灯光驱动。
pub struct LightingDriver {
    lib: EngineLibrary,
    started: bool,
}

impl LightingDriver {
    /// 加载 `lighting_driver*.dll`。**不调用 `Startup`**。
    pub fn load(paths: &EnginePaths) -> anyhow::Result<Self> {
        let path = paths
            .resolve("lighting_driver")
            .ok_or_else(|| anyhow::anyhow!("未找到 lighting_driver.dll"))?;
        Ok(Self {
            lib: EngineLibrary::load(&path)?,
            started: false,
        })
    }

    pub fn path(&self) -> &std::path::Path {
        self.lib.path()
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    /// 读取 DLL 版本号。只读、无副作用，可以安全调用。
    pub fn version(&self) -> Option<String> {
        // SAFETY: 签名取自实测的 JS 绑定表；GetDllVersion 无参数、只返回字符串。
        let get_version: FnGetDllVersion = unsafe { self.lib.func("GetDllVersion")? };
        let ptr = unsafe { get_version() };
        if ptr.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        // 该 DLL 返回的字符串必须由它自己的 FreeString 释放。
        if let Some(free_string) = unsafe { self.lib.func::<FnFreeString>("FreeString") } {
            unsafe { free_string(ptr) };
        }
        Some(text)
    }

    /// 调用 `Startup()` 初始化驱动。
    pub fn startup(&mut self) -> anyhow::Result<()> {
        let startup: FnStartup = unsafe { self.lib.func("Startup") }
            .ok_or_else(|| anyhow::anyhow!("lighting_driver 缺少 Startup 导出"))?;
        unsafe { startup() };
        self.started = true;
        Ok(())
    }

    /// 调用 `Shutdown()`。
    pub fn shutdown(&mut self) -> anyhow::Result<()> {
        let shutdown: FnShutdown = unsafe { self.lib.func("Shutdown") }
            .ok_or_else(|| anyhow::anyhow!("lighting_driver 缺少 Shutdown 导出"))?;
        unsafe { shutdown() };
        self.started = false;
        Ok(())
    }

    /// 发送一条 JSON 命令并解析返回的 JSON。
    ///
    /// 这是驱动对外的**主接口**：雷云前端的所有灯光操作最终都变成
    /// 一条 `Configure(json)` 调用。
    pub fn configure(&self, command: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let configure: FnConfigure = unsafe { self.lib.func("Configure") }
            .ok_or_else(|| anyhow::anyhow!("lighting_driver 缺少 Configure 导出"))?;
        let free_string: FnFreeString = unsafe { self.lib.func("FreeString") }
            .ok_or_else(|| anyhow::anyhow!("lighting_driver 缺少 FreeString 导出"))?;

        let json = CString::new(command.to_string())
            .map_err(|e| anyhow::anyhow!("命令包含内嵌 NUL：{e}"))?;

        // SAFETY: 传入以 NUL 结尾的 C 字符串；返回指针由本 DLL 分配，
        // 按契约必须用同一个 DLL 的 FreeString 释放。
        let out = unsafe { configure(json.as_ptr()) };
        if out.is_null() {
            anyhow::bail!("Configure 返回空指针");
        }
        let text = unsafe { CStr::from_ptr(out) }
            .to_string_lossy()
            .into_owned();
        unsafe { free_string(out) };

        serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Configure 返回的不是合法 JSON：{e}；原文：{text}"))
    }

    /// 跑一遍**注册 → 恢复 → 注销**，验证整条调用链通不通。
    ///
    /// # 为什么是这三步
    ///
    /// 这三步正好覆盖 [`CommandType`] 的三个取值，而且**都不下发灯效**，
    /// 因此不会覆盖设备上正在生效的效果——这是能在真实设备上安全跑的
    /// 最小验证序列。
    ///
    /// 字段名逐字取自实测的 JS 包装层
    /// (`electron/modules/lighting/ffiLightingDriver.js`)：
    ///
    /// ```js
    /// case "AddDevice":    return this.configure({type:"device.register",   ...payload});
    /// case "RemoveDevice": return this.configure({type:"device.unregister", ...payload});
    /// case "Resume":       return this.configure({type:"mode.set", pause:false});
    /// ```
    ///
    /// 而 `payload` 里的键是 **snake_case** 的 `device_handle` /
    /// `device_identifier`（同一文件里 `i?.payload?.device_handle` 可证），
    /// 不是驼峰。
    ///
    /// # 与运行中的雷云争用
    ///
    /// 雷云若正在运行会同时持有设备，建议先退出。调用方（`--lighting handshake`）
    /// 会先打印这句提示。
    pub fn handshake(&mut self, handle: u64, identifier: &str) -> anyhow::Result<Vec<String>> {
        let mut steps = Vec::new();

        // ① 注册
        let register = serde_json::json!({
            "type": CommandType::DeviceRegister.wire(),
            "device_handle": handle,
            "device_identifier": identifier,
        });
        self.configure(&register)
            .map_err(|e| anyhow::anyhow!("device.register 失败：{e}"))?;
        steps.push(format!(
            "device.register   device_handle={handle} device_identifier={identifier}"
        ));

        // ② 恢复（`mode.set` + `pause:false`）。与 `Pause` 相反，
        //    表示解除暂停——雷云在设备注册后用它把驱动从暂停态唤醒。
        let resume = serde_json::json!({
            "type": CommandType::ModeSet.wire(),
            "pause": false,
        });
        self.configure(&resume)
            .map_err(|e| anyhow::anyhow!("mode.set(pause=false) 失败：{e}"))?;
        steps.push("mode.set          pause=false（恢复）".to_string());

        // ③ 注销。即使这一步失败也要如实报出来，不能静默吞掉——
        //    设备句柄留在驱动里会影响雷云自己后续的注册。
        let unregister = serde_json::json!({
            "type": CommandType::DeviceUnregister.wire(),
            "device_handle": handle,
            "device_identifier": identifier,
        });
        self.configure(&unregister)
            .map_err(|e| anyhow::anyhow!("device.unregister 失败：{e}"))?;
        steps.push("device.unregister 已注销".to_string());

        Ok(steps)
    }
}

/// `lighting_driver` 的命令类型。
///
/// 三种取值**直接来自实测的 JS 包装层**
/// `electron/modules/lighting/ffiLightingDriver.js` 的 `callDLL`：
///
/// ```js
/// case "AddDevice":    return this.configure({type:"device.register",   ...payload});
/// case "RemoveDevice": return this.configure({type:"device.unregister", ...payload});
/// case "Pause":        return this.configure({type:"mode.set", pause:true});
/// case "Resume":       return this.configure({type:"mode.set", pause:false});
/// case "Configure":    return this.configure(payload);
/// ```
///
/// 即：设备注册用 `device.register`，注销用 `device.unregister`，
/// 暂停/恢复统一是 `mode.set` + `pause` 布尔。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    /// `{type:"device.register"}`
    DeviceRegister,
    /// `{type:"device.unregister"}`
    DeviceUnregister,
    /// `{type:"mode.set"}`
    ModeSet,
}

impl CommandType {
    /// 线上协议里的字面量。
    pub fn wire(self) -> &'static str {
        match self {
            Self::DeviceRegister => "device.register",
            Self::DeviceUnregister => "device.unregister",
            Self::ModeSet => "mode.set",
        }
    }
}
