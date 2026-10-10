# 当前整个程序的依赖、消费者和未闭合链

2026-10-10，以 `.ref/host-4.0.827/`、当前产品/middleware/native 证据和当前 `crates/` 为依据。本记录横向核对整个程序，补充 [全量逆向地图](full-source-reverse-map.md)；不把导出、路由、源码获取或静态调用图记为完整实现。

全量目标仍包括所有产品、页面、宿主、服务、原生库、插件、设备通信、DLL 替代、存储、安装、安全和生命周期。你的版本是本地版，因此不实现在线升级清单、远程下载、云端升级编排及其他网络专属升级流程；本地安装、离线安全校验、本地服务生命周期和设备固件/配置链路仍然属于范围。宿主存储与 helper 静态取证是当前已推进的子链，不能把它们当成总范围边界；设备通信、产品页面、固件传输和设备 DLL 写回仍必须继续逆向、实现并接入。

[机器矩阵](whole-program-closure-current-evidence.json) 由 `tools/audit-whole-program-closure-current.py` 生成，登记当前源文件 SHA-256、原函数范围、Rust 定位、每库资源与声明、PE 依赖、插件平台、本体缺失和明确缺口。工具仅解析静态文件，不执行厂商 JavaScript、DLL、EXE、应用或硬件命令。

## 全量核对结果

| 层 | 当前可核实范围 | 完成边界 |
| --- | --- | --- |
| native library | 49 库逐项登记，771 个唯一静态候选声明 | 声明不等于原 DLL 内部语义，泛型 getter 不等于 Rust 协议替代；整库替代完成数仍为 0 |
| 当前 PE 内部图 | 81 个字节校验过的文件，包含产品、CommonDLL 和 Windows 插件 | 导入表与潜在路径只能定位依赖，间接调用、动态加载、线程和全部返回语义仍需逐项消歧 |
| IDA/Hex-Rays 当前取证 | 85 个输入，84 个已取证，320,630 个 native 函数索引、10,630 个选定函数正文；3 个 mixed CLR 库保留独立托管索引及 native 正文 | 1 个 ARM64 插件尚无受支持分析后端；未选定函数、间接调用及 managed IL 桥接仍缺，正文数量不代表整库语义闭合 |
| 宿主原生依赖 | corpus 中 64 个 native_binary 条目，包含 4 DLL 和 60 `.node` 路径 | 不止此前内部图的 19 个 Windows 插件，还包含 macOS/Linux/Android 预编译插件、不同架构及 Node ABI；路径数量不代表唯一二进制数量 |
| helper/system 程序引用 | 第一方宿主 JS 中 9 个 `.exe` 名称引用；当前官方包静态取得 4 个 CommonDLL helper PE（Power/Security/EngineMon/Handle） | 4 个本体和 CLI 字符串已留证，但分支语义、进程结果、注册表/服务副作用和 Rust 替代消费者仍未闭合 |
| 宿主及共享原生链 | 本记录核对 25 个域 | 本地草稿、请求事件、生命周期片段分别记录；没有全程序等价验收结论 |

`IoTNative`、`NanoleafNative`、`PhilipsHueNative`、`RzNative_0518` 在当前库 inventory 中没有已获取并归属的资源。这里描述的是该清单的资源证据缺口，不据此推定其他位置不存在同名文件，也不推定库没有功能。动态归属和签名冲突保持未知，不能用其他产品补齐。

当前 lighting-engine 后台的 HTML、manifest 和声明的 JS/CSS 共 158 个输入已静态取得，入口和 manifest 第二次独立读取字节一致。其真实 loader 从 `installedResources` 的 `synapse ?? Common` 数组按 `LightingDriverDLL`/`LightingEngineDLL` 名称取 `filePath`，拼接 `userDataDir/Apps/filePath`；Driver 接 host `lightingDriver` 的 `InitDLL`，Engine 接动态 FFI 初始化。因此两个当前官方字节已分别归属 inventory 的 `lighting_driver` 与 `RzLightingEngineApi`，动态路径与 Synapse fallback 单独留证，不能按名称合并或猜成 CommonDLL 路径。IDA/Hex-Rays 保留 Driver 94 个、Engine 112 个被选函数正文，当前仅实现共享 Rzp25NewChroma 逐行 report/CRC/transaction/write/delay；尚未连接产品 renderer，也未闭合全部 effects、调度、其他 translator、平台 adapter 和 UI/运行验收。见[灯光原链与实际边界](lighting-native-current.md)、[获取收据](lighting-native-current-acquisition.json)与[loader 归属收据](lighting-native-current-source-evidence.json)。

## 原链与 Rust 的实际边界

| 域 | 当前原代码链 | 已有 Rust 消费者与缺口 |
| --- | --- | --- |
| 启动、命令行与单实例 | `electron/main.js` 的 ready、启动集合、窗口和退出控制 | `razer-app::run` 是启动编排；原参数、恢复、单实例冲突和失败重试尚未逐支等价 |
| KeyStorage | `electron/keyStorage.js`，`setItem/getKeys/getItem/removeItem/registerEvent/unregisterEvent` | `razer-storage::HostStorage` 已实现 URL 订阅、`hasValue`、接收方过滤、删除事件和 IPC 事件队列；真实宿主窗口注册及页面消费者尚未连接；本地草稿是另一作用域 |
| MemoryStorage | `modules/memory_storage/index.js`，per-URL Map、targetUrlArray、旧/新值、reset | 已实现有序 Map、JS truthy getter、旧/新值事件、targetUrlArray 和 reset；真实页面与跨窗口生命周期尚未接入 |
| WindowStorage | `modules/window_storage/index.js`，per-URL Map、事件、clear/remove | 已实现 clear/remove 的返回差别、订阅门控和 reset，并接 worker IPC；窗口注册/关闭与 UI 动作仍缺真实消费者 |
| 模块安装/卸载/取消/清除设置 | 当前 Dashboard 交互 → 宿主 `preload.js` 通道 → 原后台服务 | 确认面板发 `ModuleCatalogEvent::ServiceCommand`；`shell.rs` 明确报告“未执行”，没有 installer、缓存校验、后台状态发布和清设置提交 |
| 主程序升级与本地包生命周期 | `mainSubFunction.upgradeAppEngineVersion` → `simpleLaunchUserAppProcess` → `RzPowerTool.exe`；`lib/aio.js` 日期门控 | 已取得并静态核对 RzPowerTool PE 和 10 个 CLI 字符串；网络专属升级流程不实现，本地包安装、版本注册、旧版清理、relaunch 和相关 helper 分支结果仍未闭合 |
| 固件更新 | 主宿主打开更新程序与当前 `update-fw` 页面链 | `firmware_update/state.rs` 是明确的本地 preview；不代表固件传输、硬件进度、成功、取消或恢复 |
| Windows 服务 | `serviceFunction.js` → simple launch → `RzPowerTool --get-service-status/--start-service/--stop-service` | IDA 已核实 12 个允许服务的查询、启动/停止、依赖递归及 checkpoint/waitHint 等待；`razer-platform::windows_service_status` 与 shared worker IPC 已实现，无需加载 helper。保留原依赖失败忽略和父服务退出码语义，退出码 1 不证明所有依赖成功。其他平台明确不支持 SCM。原页面/host 消费者、非默认 locale及提权 launcher 仍缺；原 helper requireAdministrator，当前 worker 不获取提权令牌，权限失败如实返回。IPC 超时不撤销可能发生的服务副作用；`NativeRuntime::Shutdown` 是独立 mapping/simple 生命周期 |
| 设备安全 | `modules/security/win/index.js` → `RzSecurityTool --verify-device-security` | PE 和命令入口已取证；设备安全验证分支、返回值和 Rust consumer 尚未闭合 |
| 通知与深链 | `nativeNotificationHandler.js` → addon worker → HMAC URI → 全窗口事件 | 缺少 addon 替代、safeStorage、加密 key 文件、10 天 TTL、一次性 key 删除与 pending-protocol 队列 |
| 账户与身份 | `lib/identityPipe.js`、`getIdentityFeature.js` → identity 服务事件 | Guest 展示不能替代身份管道、认证响应、凭据、安全存储和退出广播 |
| IoT/LampArray | `IoTNativeAction`、`LampArrayAction` → 原 API/transport | `GamerRoomEvent::DeviceCommand` 到 shell 后明确未发送；缺少真实电源/帧写入、通知及响应刷新 |
| 灯光 | 当前 lighting-engine 的 installedResources loader → Engine/Driver 独立库 → Configure/protocol/register/callback → host HID/IoT/LampArray 写出 | 两库字节、loader 归属、206 个被选 IDA 正文与一种共享直接 RGB translator 已留证；`lighting.rs` 仍有原库适配，完整 effects/产品 renderer、区域、调度、其他协议与回调生命周期尚未替代 |
| speaker/microphone 音量与静音 | 当前1352 factory、Sound 页面、volume reducer/task、endpoint resolver → simple_service 导出 → IDA共同端点方法 | 四个原生接口已通过共享结果结构、Windows Core Audio 和 typed IPC 替代，不加载该 DLL；1352 speaker 页面真实读取、松开/静音提交、回读、取消及旧响应过滤已连接。本地草稿与实际观察分开。真实 productName 回退、原持续通知缓存、完整初始化重试、其他产品和 microphone 页面消费者仍缺；其他平台明确不支持该系统能力 |
| FFI 与子进程 | Main/Sub loader、FFIProcess ready/crash、exit/suspend/shutdown 回调表 | worker 隔离、有限 getter 和模块保留存在；每库 ABI、各类关闭回调、崩溃重建和挂起恢复尚未全部闭合 |
| 整体退出 | `main.js` 的 `quitApp/finalQuit`、cannot-exit 计数、poweroff、应用集合 | worker 录制与 mapping/simple shutdown 有实现；宿主完整顺序、阻止退出、pending callback 和全插件释放未等价 |
| 兼容与互斥 | `lib/exeCompatibility.js`、`lib/RzMutx.js` | 原 Windows 兼容检查、注册表分支、令牌所有权、取消及互斥生命周期仍缺对应消费者 |

以上列的是同一全量实现范围中的当前缺口。读取、观察、写入、Apply/Save/Cancel、持久化、失败和清理都需要现在按证据实现；没有独立的写回延期阶段。

SysUtilsNative 的滚轮行数 getter/setter 已按 IDA 原码由 [wheel_scroll.rs](../../crates/razer-platform/src/wheel_scroll.rs) 和 shared IPC 替代，保留 SPI 参数、BOOL 返回及 `-1` 位模式。182 前台匹配、配置、高分辨率/haptic 任务和页面消费者仍未闭合。两条系统子链的完整证据见 [滚轮](sysutils-wheel-scroll-current-evidence.json) 与 [服务查询及控制](powertool-services-current-evidence.json)。鼠标普通/profile/高速回报率、182 休眠及回报率页面、9 款键盘亮度读写当前边界另见 [设备写入](device-write-current.md)。跨平台页面发现和共享直接路由已连接，未知接口号、未实现 relay/物理分组及三平台验收分别保留在 [HID 契约](cross-platform-hid-current.md)。

原生音频列表另见 [simple_service 的 IDA 链路与 Rust 消费者](simple-audio-current.md)：32 个函数证据已核实实例/虚表、四字段 schema、Core Audio/SetupAPI 和初始化遍历顺序，`AudioDevices` 已连接设置页与 Control Pod，并不再加载原 DLL。原服务的持续事件缓存、通知/音量/会话订阅、完整失败语义仍缺；此子链不改变整库替代完成数为 0 的结论。

simple_service 的 speaker/microphone 音量 getter/setter 已进一步追到共同的端点方法、Core Audio 虚表、量化、先音量后静音及部分失败行为，见 [IDA 与当前1352调用链证据](simple-audio-volume-current-evidence.json)。共享结果结构与 Windows 端点实现已接 `AudioVolumeRead/Write`；1352 的实际页面消费者、真实 endpoint 选择、队列、取消、回读和清理另见 [页面审计](leviathan-volume-ui-current-audit.md)。原持续通知缓存、完整端点生命周期、原 productName 回退及其他产品页面消费者仍需逐项完成。这一子链不能代表整个 simple_service 等价完成。

## 原库接入不能记作不依赖原库

`native_query.rs::version` 的 RzAudioUtil 分支已按两个当前资源的 IDA 正文返回各自版本语义，不加载原 DLL：八产品为 1.0.3.1，1401 为 1.0.1.1，未知/缺产品拒绝。其他库的版本分支仍使用原导出和原 allocator。`native_read.rs::getter` 要求源声明、ContainerId 调用、init/terminate、FreeMalloc 和匹配资源；随后通过 `EngineLibrary::load` 载入原 DLL，并用 `ManuallyDrop` 保留模块至隔离 worker 退出。这是源码门控的原库查询，不是这些函数内部的 Rust 实现。`SysUtilsNative` 还有明确的加载限制；无参版本候选也不能绕过资源、生命周期和返回所有权门控。

当前 RzAudioUtil 的端点通知已恢复 callback 过滤、payload、重复 enable、整类 disable 与 Core Audio 注册/注销，接共享 worker IPC。八产品启动注册及实际导出类 `init` 逐一核对后，只有 1422/1446 的 `audio_streamMixer` 自动启用通知；另六产品的通用 `audioUtil` 初始化不能由共享包代码推定 enable。Shell 为这两个实际消费者接通真实观察作用域、retained worker drain、失连/退出 disable 与 Shutdown，退休线程保留清理所有权。原产品监听事件的大小写不匹配和仅日志行为也已保留，不能声称已替代 simple_service 的持续设备缓存，详见 [端点通知](audio-util-notifications-current.md)。

AudioRouter 的 [45 个 IDA 函数与实际路由实现](audio-router-current.md) 已补入：共享 FIFO/漂移补偿、Windows WASAPI 采集到播放、真实 IMMDevice 跨线程传递、EnableRouting/RouteDevice、通知 timer 与停止释放通过 typed IPC 替代原库。1422/1446 的真实 feature 注册与 class methods 单独留证。设备类提交和完整混音器页面消费者、原统计字段及部分异常/释放重试分支仍有缺口；不能由服务路由完成推定产品整页或整库完成。

SysUtilsNative 前台监控已按 IDA 恢复独立线程、Windows hook、300ms debounce、Explorer 重查、UWP child path 与事件格式，接 URL 订阅和 worker IPC；产品激活条件及实际 profile/滚轮/haptic 消费链仍需取证后接入。182 middleware 公共 task 的存在不证明 DeathAdder 支持或启用了该功能。独立键盘布局 getter 已按原调用线程 KLID、十六进制转换和 signed int 位模式接 IPC；两秒变化timer、线程归属及原host Stop错误分支已有独立实现，实际产品caller/UI消息线程接入仍有缺口。系统属性/显示设置五入口已按IDA WinExec命令连接现有页面。分别见 [前台监控](sysutils-foreground-current.md)、[键盘布局](sysutils-keyboard-layout-current.md)、[布局监控](sysutils-keyboard-monitor-current.md)与[系统启动](sysutils-system-launch-current.md)。

164/241 的 Scan/Pair/Unpair 已按实际 category factory 恢复事务位、报文与事件完成条件；命令 ACK 与硬件成功分开。独立 [中断读取](hid-interrupt-current.md) 已实现，配对 Session 要求先有真实事件订阅，失败/取消走 Scan4 和清理。mapping_engine 的准确全部collection selector、真实中断provider、异步服务及Dock消费者已接，配对后真实edition/layout、V2连接刷新和本地物理键缓存也已实现。原runtime/serial发布、profile合并及mapping重连和其他平台等价物理分组仍是缺口，不能将这些子链当完整接收器功能。详见 [接收器契约](receiver-ui-current.md)。

164/241 的 Help Reset 已追到实际无 key 的 `taskMakerResetOBM` 分支：当前两产品未注册通用 DeviceResetFeature，也不在硬件 OBM reset 分支，不能发送猜测的固件 reset 命令。真实 `[22,0,130]` 串号查询用于默认文档与 serial metadata；UI 快照及 USB serial 不能替代该值。源 schema-13 默认配置、缺串号 activeProfileGuid 的初始 profile、重置时追加唯一默认 profile、版本递增、本地 CAS 保存和页面刷新已连接。该 JSON 文件是应用本地适配。随后的[接收器亮度](receiver-brightness-current.md)使用主 Linker E0 / profile 1 / region 15，严格保留原pre-read/conditional setter，不追加getter或值比较，setter回应与独立getter观察分开。效果、映射及原host memory/cache发布仍未闭合，不能报告全部重置完成。

页面细节与原交互始终属于全量范围。[设置实际挂载和入口](settings-entrypoints-current.md)按当前组件调用链核对卡片和列几何；用户要求保留的“服务连接”仅作测试入口，打开实际页面 owner，不使用简化样例 Dialog。未挂载的历史侧栏CSS不用于实现。[164/241 Lighting](receiver-brightness-page-current.md)及[1342 Effects](audio-mixer-page-bindings-current.md)各自保留具体控件、菜单、参数和真实服务缺口，路由或静态编译通过不代表整页完成。

接收器当前 Linker 的 busy 后 IN exception、首次 IN/send exception、每次 OUT 标志重置、实际 payload slice 和独立串号空结果已按原分支修正；本地 source profile 在保存后、设备任务前发布到真实 workspace，设备失败不抹去本地结果，原 host storage URL/全局缓存仍未接通。CmMixerLib 的[查询结果](audio-mixer-read-outcomes-current-evidence.json)恢复第五次 busy 后格式化以及 Peak 清零失败不覆盖已读样本；[1342 Help](audio-mixer-help-current-evidence.json)连接一秒 spinner 后的 ResetStream 0–8 序列，原 native 错误码与真正传输异常分开，回执不冒充设备恢复。Mic Monitor 的实际 AudioCamy caller 仍缺失，不能用同名 CmMixerLib 属性替代。

[Alexa 输入与本地状态](alexa-input-local-state-current.md)已移除正式页示例麦克风，恢复原过滤、Default/null、已选对象和菜单标签分离以及原键本地合并保存。媒体枚举/devicechange 平台适配、账户与广播仍未接通。宿主 Map primitive key 类型和数字相等恢复见[存储](host-storage-current.md)，对象身份和非 JSON wire 类型仍是缺口。

SysUtilsNative 的 `OpenGameController` 已通过 IDA 原函数恢复为 `WinExec("control joy.cpl",5)`，连接 614/642/678/679/688 五个实际模拟键盘页面调用者，原第二行 SVG、列位置、间隔与交互样式单独留证，见[模拟键盘系统属性](keyboard-analog-properties-current.md)。这是当前源码实际注册的系统属性操作，不把共享组件的存在当作其他产品已启用。

机器矩阵的 `literal_rust_locators_not_consumer_proof` 仅供找文件。字符串出现在 diagnostics/catalog/路径列表中，不能计入函数实现；矩阵不会从这些定位自动生成“完成”状态。

## 可继续闭合的链

1. **宿主 Key/Memory/WindowStorage 消费者**：已按三个当前 JS 本体实现共享状态与 worker IPC，详见 [宿主存储](host-storage-current.md)；继续接真实窗口注册/关闭与逐页面动作，保留它们与本地草稿文件的区别，不能将后端实现计作完整 UI 链。
2. **服务/安全/升级 helper**：4 个当前 PE 已静态取得并记录 SHA-256、架构、导入和命令字符串；下一步恢复参数解析、服务控制、注册表、进程/文件操作、结果和失败，再接本地 Rust 消费者。不能只复制 JS launch wrapper。
3. **全插件传输**：逐项匹配当前包的 Node 插件源代码/PE、原 JS caller、发现身份、事件和退出顺序；共享 HID/BLE/serial/网络编码保留跨平台，真正的平台适配分别实现。第三方不同架构预编译件不能由 Windows 导出表替代取证。
4. **本地安装/卸载真实状态发布**：继续当前 background-manager 与 installer 资源声明的实际 consumer，接本地包、签名/散列、取消、clear-settings、失败和完成刷新；当前确认框已发出的 command 必须接这条生产链。网络专属升级清单/下载/云编排按本地版本要求排除。

这些是依赖可定位的下一条完整链，不是关闭其他范围。当前未执行应用、厂商代码、DLL 或硬件验证；静态检查结果仅支持文件/结构/编译一致性。2026-10-10 当前批次 `cargo check --locked --all-targets --workspace` 与格式检查通过；70 项共享设备协议测试、4 项 host storage 测试、1 项 native inventory 解析回归及 5 项 Alexa 输入/合并测试通过，均为纯 Rust 或模拟传输，不代表真实窗口、设备或原服务运行验收。

复核命令：`python -X utf8 tools/audit-whole-program-closure-current.py --check`。若并行开发改变 Rust 行号或源清单，先重新审查对应事实，再重生成矩阵；不能用更新 hash 掩盖原行为或 consumer 缺失。
