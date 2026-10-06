# DLL 只读查询现状与当前源码凭据

复核日期：2026-10-06。路线：现在逐项完成界面（包括 UI 增删改、Apply、Save）及所需只读查询；通过 DLL 修改、写回和保存随后统一处理。

本轮仅阅读 Rust、当前 `.ref/host-4.0.827/` 和已标明历史性质的审计文档，并用维护工具目录中的 Acorn 对当前 wrapper 作静态解析。**未加载或执行任何 DLL，未运行 worker、应用、探测、构建、测试或设备命令。** 没有读取已停用的旧参考源码目录。后端代码未修改。

下文“当前声明已复核”仅表示 4.0.827 wrapper 的 `apiObj`、Callback 参数及调用点与 Rust 声明形状相符。它不等于实际被 `EnginePaths` 发现的 DLL 已确认属于当前版本，也不等于其导出、调用约定、回调线程模型及运行结果已经验证；**未核实部分不可标为 current ABI 已验证**。

## 现有 ServiceRequest 分类

枚举位于 [runtime.rs](../../src/backend/runtime.rs)，实际分派位于 [runtime_native.rs](../../src/backend/runtime_native.rs)。共 10 个变体。

| ServiceRequest | 实际边界 | 分类及所需前置 | 当前证据与限制 |
| --- | --- | --- | --- |
| `SimpleVersion` | `simpleGetVersionInfo(cb3)` | 查询意图；首次先加载 simple_service 并 Initialize | 当前声明已复核；回调字段 `versionInfo`，不能从结果推断其他 DLL 版本 |
| `AudioDevices` | `simpleEnumerateAudioDevices(cb3)` | 查询意图；同上 | 当前声明已复核；回调字段 `deviceList`，是系统音频设备枚举，不等同完整 Razer 设备状态 |
| `GlobalMode` | `getGlobalMode(cb3)` | 查询意图；首次先加载 mapping_engine 并 Initialize | 当前声明已复核；返回字符串内容/schema 未在本轮重新证明 |
| `GlobalShortcuts` | `getGlobalShortcuts(cb3)` | 查询意图；同上 | 当前声明已复核；回调字段 `shortcuts` 不足以证明它就是完整 `synapseGlobalShortcuts.appEngine` 或 UI 映射数据 |
| `HidDevices` | Windows SetupAPI/HID 元数据 API | 只读枚举；不加载 Razer DLL | [runtime_hid.rs](../../src/backend/runtime_hid.rs) 使用 `CreateFileW` access=0、共享读写及 `HidD_Get*`/`HidP_GetCaps`；未发送 feature/output report。证据属于 Win32 实现，不应冒称 Razer current ABI |
| `RegisterShortcut` | `setGlobalShortcutEventCallback`，随后 `registerGlobalShortcut` | **注册/改变运行状态**，不是只读查询 | 安装 native callback 并注册全局快捷键；当前声明已复核，留待后续写操作阶段 |
| `UnregisterShortcut` | `unregisterGlobalShortcut` | **注销/改变运行状态** | 当前声明已复核，留待后续阶段 |
| `SubmitGlobalShortcutMappings` | `localStorageSetItem("synapseGlobalShortcuts", JSON({appEngine}))` | **写入原引擎存储** | 当前 setter 的声明及调用点已复核；本轮未重审前端 `generateAppEngineMappings` 的 payload/schema，不把当前检查 `mappings`/`hash` 存在等同完整协议验证 |
| `ShortcutEvents` | `Receiver::try_iter().take(512)` | 本地事件队列消费；无新增 DLL 调用 | 会取走本进程队列项；不是无状态快照查询。事件安装仅在 RegisterShortcut 路径，不能靠此请求自动获得事件 |
| `Shutdown` | 已初始化的 `mappingEngineShutdown(cb0)` / `simpleServiceShutdown(cb0)`；随后结束自有 worker | 生命周期清理，不是查询，也不是设备配置写入 | 当前声明已复核；可以是查询会话的清理步骤，但禁止以“只读”名义在本轮运行它 |

`cb0 = void()`，`cb2 = void(bool, string)`，`cb3 = void(bool, string, string)` 是当前 JS 的声明记法。Rust 对应 `extern "C"` 回调，复制返回的 NUL 结尾字符串后才离开回调。此对应关系不代替实际 DLL/FFI 边界验证。

查询意图与“完全无副作用的执行”应区分：现有 vendor 查询仍触发 DLL 加载、DLL 初始化及服务会话管理；加载会执行 DllMain。因此即便选择四个查询请求，也不能绕过本轮禁止加载/执行 DLL 的要求。

## 当前 wrapper 精确位置

文件来自 [current-host-version-audit.md](current-host-version-audit.md) 所说明的当前包静态提取。以下为 0 起始、右端不包含的 UTF-16 偏移；这些文件为压缩源码，单一行号不足以定位。

**M：** [.ref/host-4.0.827/electron/modules/mapping_engine/win/index.js](../../.ref/host-4.0.827/electron/modules/mapping_engine/win/index.js)

SHA-256：`32b0dab3a1a83a970b2501608d53c6544cab0c9a2b47054d7bd9a660fe4133c0`

**S：** [.ref/host-4.0.827/electron/modules/simple_service/win/index.js](../../.ref/host-4.0.827/electron/modules/simple_service/win/index.js)

SHA-256：`7b16cb4d3a053004d0674de99390d3f3d552022406192898c3f4ff51ca889566`

| 文件/方法 | apiObj 属性范围 | 方法或 switch 分支范围 | 复核到的调用形状 |
| --- | --- | --- | --- |
| M / mappingEngineInitialize | 5268–5312 | 13461–13988 | `void(pointer)`；`Callback("void", [])` |
| M / mappingEngineShutdown | 5313–5355 | 13988–14458 | 同上 |
| M / getGlobalMode | 9094–9128 | 89034–89890 | `void(pointer)`；cb3 |
| M / getGlobalShortcuts | 8050–8089 | 101691–102527 | `void(pointer)`；cb3 |
| M / setGlobalShortcutEventCallback | 8156–8217 | 103502–104353 | `void(pointer, pointer)`；event callback + cb2 |
| M / registerGlobalShortcut | 8218–8288 | 104353–105416 | `void(uint32, uint32, string, pointer)`；cb2 |
| M / unregisterGlobalShortcut | 8289–8352 | 105416–106266 | `void(uint32, uint32, pointer)`；cb2 |
| M / localStorageSetItem | 6760–6818 | 63388–64155 | `void(string, string, pointer)`；cb2 |
| S / simpleServiceInitialize | 1103–1147 | 4754–5308 | `void(pointer)`；cb0 |
| S / simpleServiceShutdown | 1148–1190 | 5308–5813 | 同上 |
| S / simpleGetVersionInfo | 1385–1426 | 10855–11690 | `void(pointer)`；cb3，字段 versionInfo |
| S / simpleEnumerateAudioDevices | 2032–2080 | 22039–22875 | `void(pointer)`；cb3，字段 deviceList |

M 的 `createCbGlobalShortcutEvent` 位于 101137–101691，声明为 `void(int, string, ulonglong)`。当前文件顶部从 `ffi-napi-rz.CanUseAnneCallback()` 得到开关 `t`；事件 callback 有普通/带 `t` 的构造分支，`localStorageSetItem` 也传入该开关。本轮只确认回调类型与调用点，**没有把这个开关解释为某种 C 调用约定，也没有重审 ffi-napi-rz 的全部实现**。

## 现有界面接线

[runtime_page.rs](../../src/shell/runtime_page.rs) 的 `read_services` 依次请求 `HidDevices`、`SimpleVersion`、`AudioDevices`，分别保留失败信息；关闭路径请求 `Shutdown`。本轮对 `src` 的静态调用点搜索没有发现该界面调用 `GlobalMode`、`GlobalShortcuts` 或三个写/注册请求。

已有只读服务入口可以作为后续界面读取工作的基础，但不能把“已有请求枚举”写成所有设备页面均已具有只读查询。当前缺少产品专属 profile、firmware、OLED、Studio LED/设备区域等 typed query/observation 的统一落地清单。新增查询应继续逐项核对当前宿主或当前产品包的读取声明、参数、返回 schema、失败与空结果语义，不能由 setter 名字反推 getter ABI。

## ServiceRequest 之外的入口

| 模块/入口 | 分类 | 本轮结论 |
| --- | --- | --- |
| `EnginePaths::discover/resolve`，`backend::catalog/resolve_path/engine_paths` | 文件系统路径/静态目录查询 | 不加载 DLL。目录/文件名选取不是版本证明；当前实现使用路径字符串排序，不按生产 manifest 或 PE hash 固定版本 |
| `EngineLibrary::load/discover` | DLL 加载 | 会执行 DllMain；不属于允许本轮执行的静态查询 |
| `EngineLibrary::func` | 在已加载库中取符号 | 导出名不证明参数 ABI；不能依据旧注释或旧 exports 清单声称 current ABI |
| `probe_engine` / `--probe` | 加载库后探测符号 | 不是静态 PE 导出解析；本轮不执行 |
| `LightingDriver::version` | 查询意图，但前置已加载 DLL | 当前 wrapper 的 GetDllVersion/FreeString 契约已另行静态复核，见下文；未验证实际被发现的 native binary |
| `LightingDriver::startup/shutdown` | 引擎生命周期变更 | 不是只读属性读取 |
| `LightingDriver::configure/handshake` | 通用命令/注册/模式变更 | `handshake` 会 device.register → mode.set(pause=false) → device.unregister；“不下发具体灯效”不等于只读，留待后续写操作阶段 |
| `backend::protocol` | 纯协议值/数据模型 | 自身不执行设备 I/O；其中写命令类型不能被当作已实现只读调用 |
| `backend::system` 的年份/Windows 版本/WDL 支持判断 | Win32 时间/注册表只读查询 | 非 Razer DLL ABI；打开属性对话框/显示设置等函数是用户触发导航操作，不归入属性查询 |

当前 [ffiLightingDriver.js](../../.ref/host-4.0.827/electron/modules/lighting/ffiLightingDriver.js) SHA-256 为
`6c47b657aaaf44d899b936ac6cf0fde6c1b5f0ff50c824c887f84b783cccc64c`。
GetDllVersion 属性 673–699 为 `["char*",[]]`，FreeString 属性 540–571 为 `["void",["pointer"]`；`getDllVersion` 方法 2141–2417 先读取字符串，再调用同库 FreeString。Startup 468–487、Shutdown 488–508、Configure 509–539 分别声明 `void()`、`void()`、`char*(string)`。这里只确认这些具体声明，不把整个 lighting 命令集视为完成审计。

`backend/mod.rs` 中 RzLightingEngineApi、SysUtilsNative 等导出清单及 `dll.rs` 的旧说明，本轮没有完整 current ABI 复核。尤其“导出表证明签名”“mapping/simple 只有 C++ 修饰名”“version 无副作用可以安全调用”等旧表述不可替代以上当前证据及禁止执行边界。[10-runtime-integration.md](10-runtime-integration.md) 已标注历史结论限制；本文件只更新明确列出的当前读取声明，不使其余历史结论自动转为 current。

## 后续落地边界

1. 先按界面所需补齐只读查询及错误/空结果展示；每个新增接口记录当前声明和返回格式。
2. 对任何实际准备调用的 native binary，另做不加载的路径、版本、hash、架构、PE 导出核对，再与当前 wrapper 声明绑定。当前机器安装版本与当前提取源码版本不是天然相同。
3. 将只读请求、事件订阅/服务生命周期、写回请求分别列为能力；现有统一 ServiceRequest 分派本身没有只读强制门禁。
4. 通过 DLL 修改配置、写回、保存、注册快捷键及发送改变状态的引擎命令，统一留在后续 DLL 写操作阶段；UI 增删改、Apply、Save 不因此后置。本轮静态审计没有实施任何这些 DLL 调用。
