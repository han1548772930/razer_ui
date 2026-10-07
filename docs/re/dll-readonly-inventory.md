# DLL 与设备只读接入当前契约

本文对应当前 Rust 请求、消费者和静态源码证据。当前官方 host 为静态提取的 4.0.827，身份见 [host 版本审计](current-host-version-audit.md)；产品读取参数来自当前 middleware。文件存在、导出存在、能力登记、查询代码存在和硬件读取成功不能互相替代。

开发验证仅静态读取源码、wrapper、PE 与资源，并允许格式化及 `cargo check --locked --all-targets`；不运行应用、worker、测试、安装器、下载的 JavaScript 或 DLL。以下实现状态不表示已执行 native 验收。UI 编辑、增删改、Apply/Save 和明确标注的本地草稿仍在当前范围；设备/服务写回与 DLL 持久化后置。

## 请求、返回与消费者

请求定义在 [runtime.rs](../../src/backend/runtime.rs)，分派在 [runtime_native.rs](../../src/backend/runtime_native.rs)。表中保留现有写操作，避免把整个 `ServiceRequest` 误称为只读接口；该枚举本身没有强制只读门禁。

| 请求 | 实际边界及返回 | 当前消费者与限制 |
| --- | --- | --- |
| `UsbDevices` | Windows SetupAPI 物理 USB 枚举 | 启动/热插拔发现；不加载 Node/NAN addon，也不把 HID collection 当物理 USB 列表 |
| `HidDevices` | SetupAPI、`CreateFileW(access=0)`、`HidD_Get*`、`HidP_GetCaps` 元数据 | 启动/热插拔发现；不加载 Razer DLL，不发送 feature/output report |
| `DeviceRead { target, kind }` | 源能力选择的 Firmware、Battery、Charging、Polling、Dpi 查询，返回 typed 值或错误 | 发现后的字段读取；必须有真实路径/容器/物理 PID，relay 还需真实 peer；产品默认值不是读数 |
| `ReceiverWirelessStatus { path, device_container_id }` | 源能力选择的 V2 无线连接查询，保留原始 PID/status | 发现链及接收器父卡/工具 Bindings 读取；空结果与失败分开，不执行 Scan/Pair/Unpair 写命令 |
| `SimpleVersion` | `simpleGetVersionInfo(cb3)`；非空 versionInfo，接受 JSON 或字符串 | 运行状态页显式服务刷新；首次加载并 Initialize simple_service，不推断其他 DLL 版本 |
| `AudioDevices` | `simpleEnumerateAudioDevices(cb3)`；必须为 JSON 数组，真实 `[]` 合法 | 运行状态页及 Control Pod 音频；空指针、空字符串、非法 JSON/非数组是错误 |
| `GlobalMode` | `getGlobalMode(cb3)`；非空值，接受 JSON 或字符串 | worker API 已登记，未发现普通 UI 查询调用；内部 schema 尚未完整证明 |
| `GlobalShortcuts` | `getGlobalShortcuts(cb3)`；非空值，接受 JSON 或字符串 | 不能视为完整 `synapseGlobalShortcuts.appEngine` 或 UI 映射读取 |
| `StartMacroRecording` | mapping Initialize、注册 recorder、安装 started/stopped/item callbacks、`startMacroRecording("kSoftware", cb2)` | Macro actor；请求接受不等于异步 started；改变录制会话状态，不提交设备宏 |
| `StopMacroRecording` | `stopMacroRecording`，保留回调至事件排空 | Macro actor；生命周期变更，停止错误不能冒充录制已结束 |
| `MacroRecordingEvents` | 消费本地录制事件队列 | 本身不安装 native callback，也不是无状态快照 |
| `SuspendMacroMappings` / `ResumeMacroMappings` | `disableMapping` / `enableMapping` | Macro 录制协调；临时改变服务映射状态，不是纯查询，也不等于设备宏持久化 |
| `RegisterShortcut` | 安装 `setGlobalShortcutEventCallback`，再 `registerGlobalShortcut` | 注册/改变运行状态；后置写操作，未作为普通只读 UI 入口 |
| `UnregisterShortcut` | `unregisterGlobalShortcut` | 注销/改变运行状态，同上 |
| `SubmitGlobalShortcutMappings` | `localStorageSetItem("synapseGlobalShortcuts", JSON({appEngine}))` | 原引擎存储写入；mappings/hash 形状检查不等于完整生成协议证明 |
| `ShortcutEvents` | 本地队列最多取 512 项 | 不新增 DLL 调用；事件订阅只在 RegisterShortcut 路径安装 |
| `Shutdown` | 停 recorder、恢复 mappings、注销回调，分别关闭已初始化的 mapping/simple | 生命周期清理；poisoned 时避免继续调用不可靠 native 会话，随后结束自有 worker |

录制源证据见 [macro-recording-current-evidence.json](macro-recording-current-evidence.json)，UI/异步状态见 [Macro 当前契约](macro-ui-current.md)。现存录制协调请求的分类不扩大当前设备/服务配置写回范围。

## 发现、字段读取与观察生命周期

[runtime_page.rs](../../src/shell/runtime_page.rs) 独立请求 USB 与 HID，再执行发现。一个枚举失败不丢弃另一个真实结果。接口身份先发布到 UI；UI 确认该轮连接/配置作用域后才读取设备字段，确认等待上限 30 秒。显式服务刷新随后独立读取 version/audio。启动发现及 `DeviceChangeMonitor` 热插拔刷新已接线，繁忙期间保留后续刷新请求。

字段实现位于 [device_reads.rs](../../src/backend/device_reads.rs) 和 [runtime_device_reads.rs](../../src/backend/runtime_device_reads.rs)。[生成能力](../../assets/data/device-read-capabilities.json)来自 [当前读取证据](mouse-read-capabilities-current-evidence.json)，仅覆盖实际追踪到的产品/命令。Firmware 返回版本字符串，Battery 返回百分比，Charging 保留实际状态，Polling 返回 Hz，Dpi 返回 X/Y。各字段独立保存观察及错误；充电状态不能推算 100% 电量，能力目录或本地配置不能变成读数。

`DeviceReadTarget` 包含逻辑 product_id、physical_product_id、可选 peer_product_id、真实 container/path。发送前、打开后及返回后校验 VID/PID/interface/report length/container；relay 查询前后重新确认真实在线 peer。无源能力、路径不唯一、身份改变或响应不匹配时失败，不执行原 middleware 整段初始化、模式 setter 或映射任务。

`begin_local_session` 使旧 runtime 观察/遥测失效，保留本地 profile 与设备草稿。启动空存储或读取错误不填充固定已测设备。发现结果、字段结果、工作区身份和连接代际必须一致；过期结果不得覆盖新会话或本地编辑。

父卡消费 `ReceiverDevicesObservation { connected, complete }`：partial 中已知在线设备仍可导航，partial 缺席是未知，complete 缺席才确认不在线。导航目标必须唯一且来自真实工作区，重复 PID/edition 不能去重后冒充唯一。具体 UI 见 [接收器当前契约](receiver-ui-current.md)。**接收器父卡后续单次 Bindings 查询向全局发现/新增产品工作区的增量发布仍未完成**；启动发现已有查询不能替代这条持续状态链，不能声称全部鼠标发现问题已解决。

## 当前 wrapper 声明

下列位置为当前 4.0.827 文件中 0 起始、右端不包含的 UTF-16 偏移。

- M：[mapping_engine/win/index.js](../../.ref/host-4.0.827/electron/modules/mapping_engine/win/index.js)，SHA-256 `32b0dab3a1a83a970b2501608d53c6544cab0c9a2b47054d7bd9a660fe4133c0`。
- S：[simple_service/win/index.js](../../.ref/host-4.0.827/electron/modules/simple_service/win/index.js)，SHA-256 `7b16cb4d3a053004d0674de99390d3f3d552022406192898c3f4ff51ca889566`。

| 文件/方法 | apiObj 范围 | 方法/分支范围 | 声明 |
| --- | --- | --- | --- |
| M / mappingEngineInitialize | 5268–5312 | 13461–13988 | `void(pointer)`；cb0 |
| M / mappingEngineShutdown | 5313–5355 | 13988–14458 | `void(pointer)`；cb0 |
| M / getGlobalMode | 9094–9128 | 89034–89890 | `void(pointer)`；cb3 |
| M / getGlobalShortcuts | 8050–8089 | 101691–102527 | `void(pointer)`；cb3 |
| M / setGlobalShortcutEventCallback | 8156–8217 | 103502–104353 | `void(pointer, pointer)`；event callback + cb2 |
| M / registerGlobalShortcut | 8218–8288 | 104353–105416 | `void(uint32, uint32, string, pointer)`；cb2 |
| M / unregisterGlobalShortcut | 8289–8352 | 105416–106266 | `void(uint32, uint32, pointer)`；cb2 |
| M / localStorageSetItem | 6760–6818 | 63388–64155 | `void(string, string, pointer)`；cb2 |
| S / simpleServiceInitialize | 1103–1147 | 4754–5308 | `void(pointer)`；cb0 |
| S / simpleServiceShutdown | 1148–1190 | 5308–5813 | `void(pointer)`；cb0 |
| S / simpleGetVersionInfo | 1385–1426 | 10855–11690 | `void(pointer)`；cb3 / versionInfo |
| S / simpleEnumerateAudioDevices | 2032–2080 | 22039–22875 | `void(pointer)`；cb3 / deviceList |

`cb0=void()`、`cb2=void(bool,string)`、`cb3=void(bool,string,string)` 对应 Rust `extern "C"`；bool 不是 Win32 BOOL。回调离开前复制 NUL 结尾字符串。M 的 createCbGlobalShortcutEvent 位于 101137–101691，声明 `void(int,string,ulonglong)`。CanUseAnneCallback 开关不是 C 调用约定证明，也不代表 ffi-napi-rz 线程行为已完整核实。

[启动 AST 收据](runtime-startup-current-evidence.json)记录 exact host 路由与相关初始化/查询声明；[PE 收据](runtime-pe-current-evidence.json)记录实际候选文件架构、hash 和导出。wrapper 声明相符不代表任意同名 DLL ABI 或回调生命周期已运行验证。

## DLL 选择与隔离 worker

`EnginePaths::discover` 按数字版本选择安装目录；`resolve_host_service` 只选该 CommonDLL 中准确的 mapping_engine.dll/simple_service.dll，不默默回退 Apps/Common。普通 resolve 仍按 CommonDLL → Apps/Common（含一层产品子目录）查找准确名称或合法数字版本文件名，准确名称优先。

PE 收据中的 app-4.0.827 候选 mapping SHA-256 为 `6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a`，simple 为 `f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b`。候选目录名与文件检查不证明当前安装/运行宿主已升级，也不替代官方包字节比较；源包身份与实际安装状态分别见 host 审计。

ServiceClient 启动本应用隐藏的 `--service-worker`，用 CREATE_NO_WINDOW 和 Job Object KILL_ON_JOB_CLOSE 约束自有子进程；Job 分配失败即失败并清理，不绕过隔离。请求为 JSON，响应前缀 `RAZER_UI_SERVICE `，最大帧 4 MiB，必须匹配序号。vendor 日志不能充当响应；无效/不完整帧、断开、序号错误、fatal、超时不能返回假成功。独立线程写管道，使 worker 在读取请求前阻塞也受请求超时约束。

普通请求 15 秒，StartMacroRecording20 秒，DeviceRead35 秒（含 relay 前后观察），Shutdown25 秒；native callback 单次 4 秒。callback 超时使 worker poisoned，迟到回调不能满足下一请求。DLL 用 ManuallyDrop 保留至进程退出；无法确认 native 已消费的 C 字符串在超时后也保留至退出，避免悬空内存。关闭只终止自有子进程，不终止外部 Razer 服务。

查询仍可能加载 DLL、执行 DllMain、Initialize 和管理会话；查询意图不是无副作用运行证明。阻塞请求必须在后台处理；开发阶段未执行这些加载和生命周期路径。

## 原生 HID 资产与 C ABI

传输使用当前官方 host 的 node-rz-hid 0.0.31 原生文件及其 plain C 导出；SetupAPI 仅提供当前接口身份/能力。

- 原始条目：`win-unpacked/resources/app.asar.unpacked/node_modules/node-rz-hid/build/Release/HID.node`。
- [打包资产](../../assets/native/razer-hid-0.0.31.node)：405,704 字节，SHA-256 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`。
- 内层 archive SHA-256 `9d4765d46c5c5e1ff9c11d16452bd12a9eb43f14cd70be893fa843df3882cd90`；ASAR SHA-256 `b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6`。
- [native 证据](receiver-native-hid-current-evidence.json)含 23 个 PE 导出、5 份反汇编收据；准备工具验证签名文件与 ASAR 未签名大小的差异为 PE certificate tail。

| 导出 / RVA | 当前实现证据 | Rust extern C 边界 |
| --- | --- | --- |
| hid_open_path / 0x17250 | RCX 为 NUL path，RAX 为 opaque handle/null | `fn(*const c_char) -> *mut c_void` |
| hid_send_feature_report / 0x17a10 | RCX handle、RDX bytes、R8 length；HidD_SetFeature | `fn(*mut c_void, *const u8, usize) -> c_int` |
| hid_get_feature_report / 0x17ac0 | 同上，buffer 可写；DeviceIoControl(0xB0192) | `fn(*mut c_void, *mut u8, usize) -> c_int` |
| hid_close / 0x17de0 | 清理 opaque handle | `fn(*mut c_void)` |

get-feature 返回实际完成字节数并包含 ReportID，buffer 长度不是已读取长度。Rust 不访问 native handle 内部字段。node.exe 为 delay import，这些 plain C 路径不调用 N-API 注册；加载仍执行 DLL 入口，不能据此在开发时执行它。

[runtime_hid_transport.rs](../../src/backend/runtime_hid_transport.rs)只管理资产落地、字节校验、库/opaque handle 生命周期和 feature 调用，不含 PID 或查询命令。路径为 `%LOCALAPPDATA%/RazerUi/native/<SHA-256>/HID.node`，加载限制为 DLL 目录和系统目录；已有不同字节文件明确失败。

USB 枚举另见 [usb-native 证据](usb-native-current-evidence.json)：runtime_usb.rs 使用 USB_DEVICE GUID `a5dcbf10-6530-11d2-901f-00c04fb951ed`，对应当前 detection.node 的静态枚举范围，不加载该 Node/NAN 模块。

## 接收器 V2 查询协议

[receiver_capabilities.rs](../../src/backend/receiver_capabilities.rs)读取 [生成能力](../../assets/data/receiver-query-capabilities.json)。能力需追踪实际 DeviceInfo、主 feature 配置、工厂分支、继承、传输参数和查询方法；兼容目录成员或共享类存在不足以证明支持。覆盖范围见 [capabilities 证据](receiver-capabilities-current-evidence.json)，未审计/不适用产品保持明确边界。

[receiver_protocol.rs](../../src/backend/receiver_protocol.rs)管理 rzDevice25 V2 envelope；[runtime_receiver.rs](../../src/backend/runtime_receiver.rs)按真实接口选择能力。179 链为 `34340/he → getMultipleDeviceWirelessConnectionStatusV2 → 7755/dc → 84816/VO(J) → 30580/IZ(ue)`；工厂 96204/j 的 LINKER 分支选择继承 Linker 的 rzDevice25LinkerUma。源码 hash/UTF-16 位置保存在 [discovery 证据](receiver-discovery-current-evidence.json)。

只发送 `[80,0,191]` 读取命令，90 字节主体加 1 字节 ReportID；第 88 字节为第 2–87 字节 XOR。179 Linker 事务值为 `224 | transactionId++`，达到 31 前回零；其他绑定使用各自 namespace。0x46/0x41 的 source 特殊事务规则不是本 V2 0xBF 规则。

返回必须匹配实际字节数、事务、类和命令；头状态 success=2、busy=1，其余按错误/重试边界处理。成功 payload 首字节为数量，随后每项 status/pidHigh/pidLow，PID 按高低字节组合；拒绝截断记录。真实 count=0 是成功空观察，unknown status 保留未知。179 工厂将 5ms 基础时间乘五，能力记录 25ms 间隔、OUT=20/IN=10，不推广到未证明产品。

打开前、打开后及成功响应后要求完整当前枚举、非零 ContainerId、唯一 path/instance，以及准确 VID/PID/interface/feature length。缺 MI_XX 保持未知，源 interface=-1 fallback 不能伪装 interface=0。事务计数按真实路径、容器、协议 namespace 隔离并规范化。

本地 named mutex 只协调本应用 worker，不声称获取原宿主私有 mutex；命令/事务校验拒绝交错响应。应用观察预算 10 秒，丢弃迟到结果；完整重试计划最多 5.5 秒有意等待，父请求 15 秒限制卡住的 native 调用。DeviceRead relay 前后查询另受 35 秒外层预算限制。

不发送模式初始化、配对、解绑、映射、持久化命令，不调用 source 发布链。未知设备、失败、超时、截断和不支持不能变成成功空数组。

## 身份映射、原发布器与硬件事件

AvailableDevices.json SHA-256 `428b43ac965095a26d3033e7e4043966eebe05138a3fcd7087e7fe00281be511`，DualDongleCompatibleDevices.json SHA-256 `ad095ba5df1df779c35ec2dc8aeba658b2d359f4c25efa8da4621934a1ba663a`。原始 URL 为 `https://apps.razer.com/synapse/dashboard/` 下同名文件，字节和 HTTP 收据保存在当前 Dashboard 目录。

实际 V2 返回 183 后，才可按 AvailableDevices 的 productId182/dongleId183 和 DualDongle 的 DeviceSiblings:[182]映射鼠标。179 支持表包含 183 仅证明兼容关系；只发现 179 不能制造在线 182。he 排除 65535 及自身 dongleId，Ne 严格用查询 status=1 判在线，0 为离线；固件、edition、serial、profile、ready 各需真实数据。

Ne 对初次离线或缺序列号条目最多安排 5 次递增等待重查；重查后离线条目通过 ye(...,false) 处理，在线但缺序列号可先发布身份并继续等 runtime 数据，不能补造 serial 或 ready。Uma 的 getDeviceMode/setDeviceMode 仅读取/修改本地 isDriverMode 模拟对象，其 SUCCESS 字段不能作为真实设备读取证据。

源 ye 使用真实 container，建立 master179 与 dongle183 关联，写 duallink-devices 并注册 runtime 监听；host `zS → BS → _onConnect → _checkUSBDetail` 将合成 PID183 经目录恢复产品 182、保留 realProductId183。ye(...,false)不删除旧缓存，缓存关联不是当前在线。完整发布/初始化还可能进入映射合并和写入，不能整段作为只读发现执行。

Dashboard 的 getWirelessDevices 另要求 isMultiPairingDevice、setupStatus=ready、isMultiPairingOnDongle，并按 container+从设备 PID 关联；Ct 元数据适配还要求有效 PID、非空 serial、英文 productName。连接查询不足以制造完整 Dashboard ready 状态。消费者及目录见 [publishing 证据](receiver-publishing-review-current-evidence.json)。

真实 SLAVE_CONNECT_EVENT 为 `78548/Be → 34340/kO=oe`；双连接/类别/recordId/eventId/primaryPID/state 过滤与 UI 20 秒加载见 [connect-events 证据](receiver-connect-events-current-evidence.json)和接收器契约。通知 eventValue.state=3 与查询 status=1 是不同字段。native 硬件通知 publisher 仍未接入，不能用普通查询成功或本地操作冒充事件。

## 其他入口与未完成边界

catalog、resolve_path、EnginePaths 为文件系统/静态数据查询。EngineLibrary::load/discover、probe_engine/--probe 均加载 DLL，不是静态 PE 检查；导出名字不证明参数 ABI。SysUtilsNative 加载限制保留，旧探测经历不替代当前生命周期证据。

[ffiLightingDriver.js](../../.ref/host-4.0.827/electron/modules/lighting/ffiLightingDriver.js) SHA-256 `6c47b657aaaf44d899b936ac6cf0fde6c1b5f0ff50c824c887f84b783cccc64c`。GetDllVersion 属性 673–699 为 char*()，FreeString540–571 为 void(pointer)，getDllVersion 方法 2141–2417 复制字符串后调用同库 FreeString。Startup468–487、Shutdown488–508、Configure509–539 分别为 void()、void()、char*(string)。version 有加载前提；startup/shutdown 为生命周期变更；configure/handshake 执行注册和模式命令，属于后置写操作。

RzLightingEngineApi、SysUtilsNative 完整方法 ABI、全局映射读取 schema、localStorageGetItem 对应完整映射读取路径、产品 profile/OLED/Studio LED/设备区域/固件专用查询仍需逐项当前证据。不能由 setter 反推 getter，或由路由/描述符声称 UI 及读取完成。backend::protocol 纯数据模型不等于设备 I/O；Windows 年份/版本/WDL 支持判断是系统查询，不属于 Razer DLL ABI。

维护工具包括 audit-runtime-startup.cjs、audit-receiver-native-hid.py、audit-receiver-capabilities.cjs、audit-receiver-discovery.cjs、audit-receiver-publishing-review.cjs、audit-receiver-connect-events.cjs、audit-mouse-read-capabilities.cjs 及各资源准备/校验工具；选项以工具实现为准。静态通过仅证明对应源码/资产/声明条件，实际设备读取、窗口输入和视觉验收保持未验证。
