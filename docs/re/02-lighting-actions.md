# 灯光协议：引擎 action、effect 和 driver 分层

## 1. 来源与适用范围

数字常量来自 [frontend 2973.2f1d1e6a.chunk.js](../../.ref/frontend/static/js/2973.2f1d1e6a.chunk.js)，搜索 `LightingEngine_ActionId`、`LightingEngine_EffectId`、`LightingDevice_FlushFrame` 可定位导出和赋值。

宿主 driver 绑定来自 [ffiLightingDriver.js](../../.ref/synapse-asar/electron/modules/lighting/ffiLightingDriver.js)。产品实际灯效来自 653 模块 78193、777 模块 7816，见 [灯光页面](../screens/07-lighting.md)。

本表是静态协议定位，不表示当前 Rust UI 已调用这些动作，也不提供未经验证的通用“发一个数字即可改灯”方案。

## 2. 必须分清的命名空间

| 层 | 示例 | 语义 |
|---|---|---|
| 产品 UI 灯效 | Ambient_Effect、Breathing_Effect、Tidal_Effect | 产品支持的选项和参数，受连接/edition 影响 |
| LightingEngine action | 33 = LightingEngine_AddEffect | 引擎要执行的操作 |
| LightingEngine effect ID | 1 = Ripple，6 = Static | 引擎效果类型 |
| LightingEngine event | 1 = CustomMode，2 = RenderFrame | 回调事件 |
| 宿主 lightingDriver action | AddDevice、Configure、Pause | Electron wrapper 分发名 |
| driver Configure JSON | type=device.register、mode.set | DLL JSON 协议 |

这些数字/字符串不能互换。action 1、effect 1、event 1 同时存在且含义不同。UI 的 Tidal/Wheel 不能因为模型缺枚举就映射到 Wave；更不能直接将上述数字当成 HID 报文。

## 3. 引擎 action 反查表

下面为原包 `LightingEngine_ActionId` 对象中的条目，保留其实际集合。

| ID | 函数 |
|---:|---|
| 1 | `LightingEngine_CreateDevice` |
| 2 | `LightingEngine_DestroyDevice` |
| 3 | `LightingEngine_CreateEngine` |
| 4 | `LightingEngine_DestroyEngine` |
| 17 | `LightingEngine_AddDevice` |
| 18 | `LightingEngine_RemoveDevice` |
| 19 | `LightingEngine_SetDevicePosition` |
| 20 | `LightingEngine_GetNumRows` |
| 21 | `LightingEngine_GetNumCols` |
| 33 | `LightingEngine_AddEffect` |
| 34 | `LightingEngine_RemoveEffect` |
| 35 | `LightingEngine_SetMask` |
| 36 | `LightingEngine_GetMask` |
| 49 | `LightingEngine_EnableEngine` |
| 50 | `LightingEngine_SetPreviewMode` |
| 51 | `LightingEngine_SyncEffect` |
| 52 | `LightingEngine_GetCurrentFrame` |
| 53 | `LightingEngine_GetCurrentFrameMultipleDevices` |
| 57344 | `LightingEngine_InitChromaAiColorGen` |
| 61440 | `LightingDevice_UpdateDeviceConfig` |
| 61441 | `LightingDevice_GetNumRows` |
| 61442 | `LightingDevice_GetNumCols` |
| 61443 | `LightingDevice_AddLinkedDevice` |
| 61444 | `LightingDevice_RemoveLinkedDevice` |
| 61445 | `LightingDevice_InputEvent` |
| 61446 | `LightingDevice_MapInput` |
| 61447 | `LightingDevice_SetBrightnessState` |
| 61448 | `LightingDevice_GetBrightnessState` |
| 61449 | `LightingDevice_SetBrightnessLevel` |
| 61450 | `LightingDevice_GetBrightnessLevel` |
| 61451 | `LightingDevice_Identify` |
| 61697 | `LightingDevice_GetNumRegions` |
| 61698 | `LightingDevice_EnumRegion` |
| 61699 | `LightingDevice_GetRegionInfo` |
| 61700 | `LightingDevice_UpdateRegionInfo` |
| 65520 | `LightingEngine_Terminate` |

原包还导出 `LightingDevice_FlushFrame = 61452`，但 **LightingEngine_ActionId 反查对象没有 61452 这一项**。这是原包的导出/反查不一致，应单独记录，不能把“反查中没有”解释成“常量不存在”。

## 4. Effect ID 与事件 ID

| Effect ID | 导出名 |
|---|---|
| 1 | LightingEngine_EffectId_Ripple |
| 2 | LightingEngine_EffectId_Wave |
| 3 | LightingEngine_EffectId_Reactive |
| 4 | LightingEngine_EffectId_Spectrum |
| 5 | LightingEngine_EffectId_Breath |
| 6 | LightingEngine_EffectId_Static |
| 7 | LightingEngine_EffectId_Starlight |
| 8 | LightingEngine_EffectId_Fire |
| 11 | LightingEngine_EffectId_Ambience |
| 12 | LightingEngine_EffectId_AudioVU |
| 13 | LightingEngine_EffectId_ColorWheel |
| 15 | LightingEngine_EffectId_SpecialEdition |

| Event ID | 导出名 |
|---|---|
| 1 | LightingEngine_Event_CustomMode |
| 2 | LightingEngine_Event_RenderFrame |
| 3 | LightingEngine_Event_InputNotify |
| 256 | LightingEngine_Event_PreviewFrame |

不能把 effect 与 event 合并成一个按 ID 唯一索引的 map，否则 1/2/3 会冲突。产品 quick 列表不是这张 effect 表的完整拷贝；具体映射和 settings schema 必须沿 UI 更新函数继续追踪。

## 5. 宿主 lightingDriver wrapper

`callDLL(event, action)` 的分发包括：

| action | 实际行为 |
|---|---|
| InitDLL | 加载绑定，初始化 Startup 和写回调 |
| GetDllVersion | 读取版本字符串 |
| AddDevice | 调用 onAddDevice、维护 deviceMap；Configure({type:"device.register", ...payload}) |
| RemoveDevice | 调用 onRemoveDevice、移除 deviceMap；Configure({type:"device.unregister", ...payload}) |
| Configure | 将 payload 交给 configure |
| Pause | Configure({type:"mode.set", pause:true}) |
| Resume | Configure({type:"mode.set", pause:false}) |
| HookLightingCallback | 使用 lightingDllName、callbackRegisterFunction、lightingChannel 建立回调 |

设备字段为 `device_handle` / `device_identifier`。write callback 从 JSON 的 device_handle 查映射后路由输出；初始化/注册本身不等于某个效果已经下发。

JS 声明的 FFI 形状：

```text
Startup: void()
Shutdown: void()
Configure: char*(string)
FreeString: void(pointer)
HookLightingCallback: bool(string, string, pointer)
SetWriteFFICallback: void(pointer)
GetDllVersion: char*()
```

这是 wrapper 声明，不是本轮已运行验证的 C ABI。字符串由对应 DLL 的 FreeString 释放；回调的上下文和生命周期必须保持有效。effect 设置、帧输出与注册/暂停协议不能混为一谈。

## 6. 当前 Rust 状态与实现要求

[backend/lighting.rs](../../src/backend/lighting.rs) 已有 Startup/Shutdown/Configure/GetDllVersion 等绑定和显式命令行路径；[features/lighting.rs](../../src/features/lighting.rs) 没有因此自动接通。其参数变化主要更新 AppShell 内存。

`[建议]` Rust 层为产品 effect、引擎 action、引擎 event、driver 命令分别定义类型；后端适配器负责参数校验、生命周期、请求/响应和设备路由，页面只表达领域意图。GPUI Kit 的 render 不调用 DLL。

验收需证明：选中真实产品 effect → 序列化正确设置 → 已初始化的目标设备/引擎收到请求 → 错误/确认被处理 → UI 与回读一致。当前文档没有声称这条链已经完成。
