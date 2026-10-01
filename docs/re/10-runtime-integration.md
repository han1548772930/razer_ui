# 运行时接入：静态签名与真实服务边界

2026-10-01 审计只读取 `.ref`、已安装 PE 文件及依赖源码，未加载 DLL、未运行 worker、未写设备。以下“已证实”指调用声明和本机导出相互吻合，不代表真实设备执行已验证。

## 1. 当前安装与原宿主

原宿主来自 [.ref/synapse-asar](../../.ref/synapse-asar)，版本 4.0.563；本机最新安装为 `C:\Program Files\Razer\RazerAppEngine\app-4.0.821`，另保留 4.0.662。版本不能混为同一份二进制。

本轮用 Python `struct` 只读解析 PE 导出表：

| 本机文件 | 导出数量 | 关键发现 |
|---|---:|---|
| `app-4.0.821/CommonDLL/mapping_engine.dll` | 267 | 同时存在 `mappingEngineInitialize`、`registerGlobalShortcut`、`getGlobalMode` 等普通名称与 C++ 修饰名 |
| `app-4.0.821/CommonDLL/simple_service.dll` | 89 | 同时存在 `simpleServiceInitialize`、`simpleGetVersionInfo`、`simpleEnumerateAudioDevices` 等普通名称与修饰名 |
| `app-4.0.821/CommonDLL/SysUtilsNative.dll` | 89 | 普通名称；`FreeMalloc`、`SetNodeFFIEvent` 存在 |
| `%LOCALAPPDATA%/Razer/RazerAppEngine/User Data/Apps/Common/lighting_driver_v1.9.14.0.dll` | 10 | `Startup`、`Shutdown`、`Configure`、`FreeString`、`SetWriteFFICallback` 存在 |
| 同目录 `RzLightingEngineApi_v4.0.55.0.dll` | 15 | `RzLightingApi`、`FreeMalloc`、`PollEvents` 等存在；导出名本身不能推出完整调用参数 |

因此 [backend/mod.rs](../../src/backend/mod.rs) 中“mapping/simple 只能按修饰名查找”、[backend/dll.rs](../../src/backend/dll.rs) 中“导出表证明签名”的旧注释均不成立。普通别名已存在；签名应来自 JS `apiObj`、`Callback` 和调用点，而非仅凭导出名。

## 2. 可建立 Rust 适配的声明

下面 `cb0 = void()`、`cb2 = void(bool, const char *reason)`、`cb3 = void(bool, const char *reason, const char *value)`。这些是原 `ffi-napi-rz` 声明，不是 Win32 `BOOL`。字符串应在回调内复制；不能把借用指针送到 UI 线程。

| 边界 | 已证实声明 | 来源与限制 |
|---|---|---|
| 映射生命周期 | `void mappingEngineInitialize(cb0)` / `void mappingEngineShutdown(cb0)` | [mapping_engine/win/index.js](../../.ref/synapse-asar/electron/modules/mapping_engine/win/index.js) 的 `initDll` 和同名 action；没有宿主 nonce 参数 |
| 映射查询 | `void getGlobalMode(cb3)` / `void getGlobalShortcuts(cb3)` | 同文件对应方法；返回 JSON/字符串需再解析，失败保留 `result/reason` |
| 按键通知注册 | `void registerGlobalShortcut(uint32 vkeyCode, uint32 modifiers, const char *argument, cb2)`；注销前两项加 `cb2` | 同文件 `registerGlobalShortcut`；事件回调为 `void(int, const char *, uint64)`，通过 `setGlobalShortcutEventCallback(event_cb, cb2)` 安装 |
| 原全局映射提交 | `void localStorageSetItem(const char *key, const char *value, cb2)` | 同文件同名方法；页面使用键 `synapseGlobalShortcuts`，值包含 `{appEngine: {mappings, hash}}` |
| 简单服务生命周期 | `void simpleServiceInitialize(cb0)` / `void simpleServiceShutdown(cb0)` | [simple_service/win/index.js](../../.ref/synapse-asar/electron/modules/simple_service/win/index.js) |
| 版本、系统音频枚举 | `void simpleGetVersionInfo(cb3)` / `void simpleEnumerateAudioDevices(cb3)` | 同文件同名方法，结果分别叫 `versionInfo` / `deviceList`；不等于 Razer USB 设备枚举 |
| 进程启动 | `void simpleLaunchUserAppProcess(const char *folderName, const char *filePath, const char *params, void(bool,const char *,int))` | 同文件；`NoWait` 变体回调为 `cb2`，不能混用 |
| 灯光 driver | `void Startup()` / `void Shutdown()` / `char *Configure(const char *)` / `void FreeString(void *)` | [ffiLightingDriver.js](../../.ref/synapse-asar/electron/modules/lighting/ffiLightingDriver.js)；现有 [lighting.rs](../../src/backend/lighting.rs) 仅覆盖部分 driver 调用 |
| 系统工具 | `bool Initialize()`、`void Terminate()`、`void *GetDLLVersion()`、`void FreeMalloc(void *)`、`bool SetNodeFFIEvent(void *)` | [sysutil/win/index.js](../../.ref/synapse-asar/electron/modules/sysutil/win/index.js) 与 [ffiMain.js](../../.ref/synapse-asar/electron/modules/ffi/ffiMain.js)；已有旧探测记录显示加载阻塞，本轮不重新探测 |

[7282 的 GlobalShortcutsContainer](../../.ref/frontend/static/js/7282.873c10ab.chunk.js) 用 `generateAppEngineMappings` 生成原引擎映射，再计算 hash。`registerGlobalShortcut(..., argument)` 是独立的通知 API；把任意 UI JSON 塞入 argument 不能证明原输出已经配置。提交接口可以接收已经生成的 `appEngine`，但不得编造 mapping/hash 协议。

### 2.1 全局快捷键现有配置不能通过已验证接口读回

`getGlobalShortcuts` 的 wrapper 仅保证第三个回调参数是字符串，Electron 返回 `{result, reason, shortcuts: string}`；前端包中只存在包装方法，没有解析该字符串的消费点，因此内部 JSON schema 尚未证实。它与 `registerGlobalShortcut` 的通知注册接口位于同一组，不能将结果视为 `synapseGlobalShortcuts.appEngine`，也不能将空结果作为“用户没有原配置”的证据。

原配置读取位于 7282 chunk 的 `loadShortcuts`（字符偏移约 123293）：`jt.A.get(ce.A.localStorageName, true)` 读取浏览器存储，然后使用 `JSON.parse(...).mappings`。55 chunk 的 module 69782（约 192228）读取同一键，验证 `mappings` 和 `appEngine` 后，只将 `{appEngine}` 写入映射引擎。浏览器存储中的完整对象是 `{mappings: UI映射列表, appEngine: {mappings: 引擎映射列表, hash}}`，两层 mappings 不能混用。

本机 PE 有普通导出 `localStorageGetItem`，但没有相应修饰名；4.0.563 的 `.ref` Windows/macOS wrapper 和只读解析的本机 4.0.821 `app.asar` Windows wrapper 均没有该函数的 `apiObj`、回调声明或调用点。不能仅凭名字假定 `void(key, cb3)`。因此本轮没有新增 `ReadGlobalShortcutMappings` 的未证实 FFI；UI 不提供向原引擎提交的启用按钮。日后须先取得可信读取签名、读取并展示现有配置，再实现经用户审阅的合并提交，不能以 `{}` 或空列表代替读取失败。

## 3. 仍需实现的设备传输

原 `UsbRzDeviceAction.js` 使用 `node-rz-hid` / `rz-usb-detect`；BLE、串口另有 addon。`.ref` 和当前安装的 `app.asar.unpacked/node_modules` 中确有 `HID.node`、`detection.node` 等二进制，不能说“文件缺失”。然而它们是 Node/N-API 对象入口；所存包缺少核心 HID/检测 C++ 实现和可直接链接的完整公开 C 头文件，不能把 `hid.sendFeatureReport` 这个 JS 方法当作 DLL 的 C 导出调用。

可独立使用 Windows SetupAPI + HID API 枚举接口、读取 VID/PID/usage/path；得到接口不等于拿到原前端完整 DeviceInfo、container/profile/firmware 状态。DPI、polling、Smart Tracking、配对、777 EQ 的写入仍需逐项复原产品包的报告编码、读写顺序、互斥、确认和回读。`electron/Protocol/protocol25*.js` 提供部分报告编码证据；不能仅用页面 setter 名或 driver 的 `Configure` 代替这些设备协议。

灯光 driver 在宿主中通过 `SetWriteFFICallback` 回到 HID / IoT / LampArray 路由；仅注册设备并调用 `Configure` 不代表帧已写入硬件。`WssAction.js` 的 5426 `/synapse` 是插件服务，白名单四种 clientName，不是可直接代替全部 renderer IPC 的设备 RPC。

## 4. 独立 Windows 全局快捷键

`Cargo.lock` 原已有间接依赖 `windows-sys 0.61.2`（另有旧版本）；本轮已增加 Windows 专属显式依赖，GPUI UI 层继续只使用 `gpui-kit`。本地 [0.61.2 KeyboardAndMouse 源码](C:/Users/han/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/windows-sys-0.61.2/src/Windows/Win32/UI/Input/KeyboardAndMouse/mod.rs) 已核实 `RegisterHotKey`、`UnregisterHotKey`、`SendInput`、`MOD_NOREPEAT`、`KEYEVENTF_UNICODE`。

建议用单独线程建立消息队列，在同一线程注册/注销并接收 `WM_HOTKEY`；UI 不必接管 GPUI 窗口过程。记录注册失败/冲突，修改时保留旧映射直到新注册成功，退出注销。本地注册范围是 Ctrl/Alt/Shift/Win 加单个虚拟键；不能冒充原引擎的左右修饰键区别、Hypershift、设备专属按键、鼠标组合、宏/Chroma 全能力。系统保留组合可能注册失败。

| 本地输出 | 可实现边界 |
|---|---|
| Program | `std::process::Command` 或 `ShellExecuteW` 打开选定程序；参数作为参数传递，不拼 shell 命令 |
| Website | 系统 URL 打开 API / `ShellExecuteW`；校验协议并把错误反馈给该快捷键 |
| Media | `SendInput` 发送明确媒体虚拟键的按下/释放；不能将“已注入”称为目标播放器已响应 |
| Windows | 逐项映射到官方 API、设置 URI 或明确组合键；不支持的原动作保持不可用 |
| Text | `SendInput` + `KEYEVENTF_UNICODE` 注入 UTF-16；目标是触发时的前台窗口，受完整性级别限制，不能保证所有应用接受 |

所需特性至少 `Win32_Foundation`、`Win32_UI_Input_KeyboardAndMouse`、`Win32_UI_WindowsAndMessaging`；Shell 打开增加 `Win32_UI_Shell`。DLL 路线应置于本项目独立 worker 子进程，隐藏控制台、按请求限时，超时只结束该 worker；初始化与回调都不能阻塞 UI，也不能在回调仍可能发生时卸载 DLL。

## 5. 本轮已实现的 worker 接缝

[runtime.rs](../../src/backend/runtime.rs) 的 `ServiceClient::spawn()` 启动当前程序 `--service-worker`，Windows 使用 `CREATE_NO_WINDOW`。`request(ServiceRequest)` 为阻塞 API，调用者必须放在后台任务，保留 client 才能保留原生注册状态。主入口应在 GPUI 初始化前调用 `run_worker()` 并以其状态码退出。

[runtime_job.rs](../../src/backend/runtime_job.rs) 按 `windows-sys 0.61.2` 的官方 Win32 声明创建 Job Object，设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。父进程通过不可继承的 `OwnedHandle` 持有唯一 Job 句柄；worker 在启动请求线程前加入 Job，加入失败则清理 child 并返回错误，不降级到无保护模式。只有加入成功后才可能送出加载 vendor DLL 的请求。父程序即使通过 `cx.quit` 直接退出、没有执行 Rust Drop，操作系统仍会关闭句柄并终止该 worker；不依赖卡死的 DLL 返回或 worker 再次读取 EOF。新增特性仅为 `Win32_System_JobObjects`，版本仍用现有锁定依赖。

请求包括 `SimpleVersion`、`AudioDevices`、`HidDevices`、`GlobalMode`、`GlobalShortcuts`、`RegisterShortcut`、`UnregisterShortcut`、`SubmitGlobalShortcutMappings`、`ShortcutEvents` 和 `Shutdown`。提交只有收到原生成功回调才返回 `accepted`；这不是设备回读。原引擎映射的生成器位于 [2280 chunk](../../.ref/frontend/static/js/2280.d4f2f9d4.chunk.js) 的 `generateAppEngineMappings → getAppEngineMapping`，worker 不编造它的输出。

父子通信使用带请求序号的 JSON 行，单帧上限 4 MiB。仅解析带 `RAZER_UI_SERVICE ` 前缀的响应；无关原生日志使用非 UTF-8 Windows 编码时也不会误判成协议帧。独立 writer 避免在 pipe 写入时挂死；完整请求限时 15 秒，单个原生回调限时 4 秒。即使 DllMain 或原生函数本身在返回前阻塞，父进程的完整请求期限仍然生效。回调超时后 worker 标为不可复用并退出，父进程结束自己持有的 child；终止后的 OS `wait` 放在独立回收线程，不会再次阻塞请求线程。终止失败返回明确错误，不称为已成功结束；关闭 Job 句柄同时提供终止保障。`is_stopped()` 表示该客户端连接已关闭，不能用它推断每项设备服务已经初始化。

DLL 缺失、加载失败或缺少导出只使当前请求失败，HID 和另一 DLL 的请求仍可独立尝试；原生回调超时则终止整个 worker，避免迟到回调串入另一请求。已加载库被保留，不会因缺导出后的重试而重复加载；回调函数静态存活，DLL 不在回调可能存活时卸载，超时的字符串实参也留至进程退出。关闭请求会分别尝试已经初始化的引擎，除非先前回调超时导致 worker 不可复用。错误响应保留完整上下文和底层原因。原生事件队列最多 512 项，满时丢弃后续事件，不允许回调阻塞 DLL 线程。

[runtime_hid.rs](../../src/backend/runtime_hid.rs) 通过官方 SetupAPI/HID 接口，只读枚举 VID `0x1532/0x068e` 的 interface metadata，返回 `{interfaces, failures, complete}`，包含路径、VID/PID、产品字符串、usage 和报告长度。单接口路径/打开/属性失败会记录后继续；caps 或可选字符串失败保留已知接口并将不可读字段设为 `null`，同时记录具体操作和错误。枚举本身中途失败时保留已有结果、记录原因并令 `complete=false`；只有正常遇到 `ERROR_NO_MORE_ITEMS` 才为 true。HID 请求不依赖两个 vendor DLL，其请求失败不会污染其他服务。句柄、设备集合与 preparsed data 均由 RAII 释放。未实现物理设备去重、container/profile/firmware 读取或设备写报告。本轮仅编译审查，未启动 worker 或执行真实 DLL/HID API。
