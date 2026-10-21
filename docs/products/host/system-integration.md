# Windows 系统事件与原生退出清理

本文沿 SysUtils 的事件生产、线程所有者和清理函数核对宿主退出。JS 层的广播、设备恢复及退出编排见[窗口生命周期](window-lifecycle.md)。事件已经发出、停止标志已经设置、线程已经结束和资源已经释放分别判断。

## 当前原件与证据范围

当前稳定宿主的来源与部署复核见[宿主获取收据](../../../.analysis/inputs/host-current/acquisition.json)及[元数据复核](../../../.analysis/inputs/host-current/metadata-recheck.json)。官方包静态提取的 `CommonDLL/SysUtilsNative.dll` 与保留的本机副本按实际字节核对一致；PE 为 x64，DllMain 中的版本日志为 `1.1.31.0`、功能标记为 `foreground-window-debounce`。

[完整原生捕获](../../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/capture.json)使用 IDA 9.1、Hex-Rays 9.1.0.250226，保留 3,867 个完整函数及 9 份汇总产物；所有函数文件和汇总产物的实际摘要已重新核对。下文地址均为该原件相对 Image Base 的 RVA，Image Base 记录在捕获收据中。

原捕获的 `0xaad5c` 反编译失败保留不变。[独立追加捕获](../../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/supplements/ae9e32d7d19af63d/capture.json)依据完整汇编修复编码辅助函数的参数推断，重新捕获 7 个相关函数和调用者。追加捕获的伪代码完整、数据库一致性问题为 0，基准捕获和数据库均未改动；解析器核验后该原失败已解析，仍不等于整个 DLL 的功能语义、Rust 实现或实际对照已经完成。

宿主 FFI 声明和事件消费者的原始字节、完整 AST 所有者见[统一宿主查询](../../../.analysis/results/host-current/local-audit.json)的 `window_lifecycle`，其中 `modules/sysutil/win/index.js` 的声明／配置为 `[486,4440)`，事件消费者为 `[5977,7357)`。本页静态阅读不执行 DLL、厂商 JS、系统命令或真实设备操作。

## 初始化与回调注册

`Initialize` 导出 `0x6bd10` 跳转至 `0x6bc10`。第一次调用初始化临界区，调用 `CoInitializeEx(NULL, COINIT_MULTITHREADED)`；成功后继续 `CoInitializeSecurity`。COM 失败只记录日志，之后仍调用进程电源设置路径、设置 initialized 标志，并把返回值低字节设为 1；重复调用也返回低字节 1。电源设置 `0x792a0` 传入版本 1、ControlMask=4、StateMask=0 的结构，不应扩展成关闭全部节流策略。宿主声明读取 bool，因此该结果不证明 COM、安全初始化或电源设置修改成功。

`SetNodeFFIEvent`（`0x6dfc0`）先保存全局回调指针。传 NULL 只清这一指针并返回 1，不停止已有监视器，也不清除网络和进程观察者分别保存的回调。传非 NULL 才启用事件门禁，并按缺失情况创建下列对象：

| 所有者 | 创建行为 | 注册返回能证明的范围 |
| --- | --- | --- |
| 网络观察者 | `CRZNetworkMonitor`／`CRZNetworkManager`，保存独立网络回调及启用标志 | 已走注册路径；COM 订阅和之后的事件仍有自己的失败条件 |
| 本地应用集成 worker | `CRegisterDiscord`，创建 auto-reset event，执行 `WSAStartup(2.2)`，创建 `0x5ad80` 线程 | 函数没有等待线程就绪，也未保存 CreateThread 返回的 HANDLE |
| 进程启动观察者 | 创建 `CAppLaunchMonitor`、注册集合和 auto-reset event，另存回调与全局 owner | 此时尚未证明 WMI worker 已启动或订阅成功 |
| 系统隐藏窗口 | 全局 hWnd 为空时创建 `0x6dd40` 线程 | 未保存线程 HANDLE，没有等待窗口创建；函数固定返回 1 |

DllMain 的进程 attach（`0x3c200`）也调用初始化。回调注册不能代替逐对象的初始化结果，更不能把 NULL 注册当作完整注销屏障。

## 系统隐藏窗口与事件来源

`0x6dd40` 注册窗口类，WndProc 为 `0x6c270`，创建名为 `RazerAppEngine Window` 的窗口并移出可见区域。创建后设置 timer 1001、周期 5000 ms，登记显示电源、合盖和 WTS session 通知，随后进入 GetMessage／DispatchMessage 循环。WTS 注册标志在调用后直接设为 true，没有以 API 返回值确认。

WM_CREATE 调用 `SetProcessShutdownParameters(0x3ff, 1)`，登记 HID interface 设备通知和 suspend／resume 通知；还包含读取系统语言的 PowerShell 命令链。这是源码事实，本次没有执行该 helper。

电源消息先检查全局回调与事件门禁。原件中显示电源状态变量的初始字节值为 1，`displayPowerState`（`0x6bf80`）只返回这个进程内变量，没有重新查询 OS。

| Windows 输入 | 真实输出和状态变化 |
| --- | --- |
| WM_POWERBROADCAST，`PBT_APMSUSPEND`（4） | 发布 `suspend` |
| `PBT_APMRESUMESUSPEND`（7） | 发布 `resume`；保存的显示状态为 0 时改成 1 |
| `PBT_APMPOWERSTATUSCHANGE`（0x0a）／`PBT_APMRESUMEAUTOMATIC`（0x12） | 只记录日志，没有发布上述 resume 事件 |
| `PBT_POWERSETTINGCHANGE`，`GUID_CONSOLE_DISPLAY_STATE` | 读取 payload `+20` 的单字节，保存原值；0／1／2 分别发布 `displaypoweroff`／`displaypoweron`／`displaypowerdimmed`；未知值保留且只记录日志 |
| `GUID_LIDSWITCH_STATE_CHANGE`，值 1 | 先发布 `lidopened`，保存显示状态 1，再发布 `displaypoweron` |
| 合盖值 0 | 先发布 `lidclosed`，Sleep 150 ms 后枚举显示器；计数为 0 且原显示状态非 0 时保存 0 并发布 off。枚举失败也在原状态非 0 时发布 off，不能把该事件解释为成功查询到显示器关闭 |
| WM_WTSSESSION_CHANGE，1／2 | 读取当前用户名，分别发布 `os_user_connect`／`os_user_disconnect`；其他 WTS 代码没有这两个事件 |
| timer 1001 | 调用 GetLastInputInfo 后发布 `systemidle` 及 idleTime；本地临时 buffer 不转移所有权 |

两份电源 GUID 的实际原件字节分别为 `6fe69556-704a-47a0-8f24-c28d936fda47` 和 `ba3e0f4d-b817-4094-a2d1-d56379e6a0f3`。原版显示状态是这些消息和本地分支维护的状态，不能把它提升为每次调用时的即时 OS 观测。

`StartDetectSwitchUserWindow`／`StopDetectSwitchUserWindow`（`0x73cc0`／`0x73d10`）只处理 WTS 通知登记和标志，不创建、结束或 join 上述窗口线程。

## 系统关机请求与十秒计时

WndProc 收到 WM_QUERYENDSESSION（0x11）时，在回调及门禁允许的条件下发布 `systempoweroff`，调用 `ShutdownBlockReasonCreate`，理由固定为 `Shuting down...`，然后设置 timer 1002、间隔 10000 ms，最后无条件返回 0。读取到的 canShutdown 只用于日志，该函数没有按它决定是否返回允许结束。

timer 1002 将 canShutdown 标志设为 true、停止此 timer、销毁 shutdown block reason。这个动作由计时触发，不检查宿主退出、设备恢复应答、服务释放或其他 worker 的结束。WM_ENDSESSION 只记录日志并返回 1。WM_DESTROY 则设 canShutdown、停止两个 timer、PostQuitMessage 并注销 HID 设备通知。

因此 `systempoweroff` 先由此窗口生产，再由宿主 JS 消费；十秒过期只证明执行过计时分支。宿主设备恢复和最终退出的等待／超时需要继续按[窗口生命周期](window-lifecycle.md#应用退出与最终释放)核对。

另两个导出 `SetShutdownBlockReasonCreate`／`SetShutdownBlockReasonDestroy`（`0x73eb0`／`0x73f10`）的实际汇编使用 `movsxd rcx, ecx`，把 32 位有符号整数扩展成 HWND；字符串参数只用于日志，create 仍使用固定理由。不能把该声明当成通用的 x64 指针宽度接口或直接照抄到系统 API 绑定。

## Terminate 的实际清理顺序

宿主声明 Terminate 为 void。导出 `0x6bf30` 调用 `0x6bd20` 后继续日志 flush 虚调用（`0xd140`），没有返回汇总清理状态；反编译显示的 RAX 不是所有 worker 已停止的 bool。

`0x6bd20` 仅在 initialized 标志为 true 时执行以下顺序：清 initialized、设 terminating → 停前台窗口观察者 → 禁用全局事件 → 销毁网络对象 → 通知并销毁应用集成对象 → CloseColorPicker → 注销 WTS → 禁用、通知并销毁进程启动观察者 → 注销设备及 suspend 通知 → 关闭命名 ExitEvent → 注销两个电源通知 → CoUninitialize → DeleteCriticalSection。再次调用跳过该清理体。

该完整函数没有 DestroyWindow、向系统隐藏窗口投递 WM_QUIT、等待其线程结束，或清空全局 callback 指针的步骤。它执行注销和对象释放，但这些步骤不能一并报告为回调已排空。不同所有者的下层行为如下。

### 前台窗口观察者

`0x73b80` 在临界区内清监视标志，调用 `0x41250`／`0x41300` 停止 owner，再析构 `0x40920`。

`0x41300` 先设 stopping，并在回调 mutex 下销毁回调对象。普通非当前线程分支在必要时等待窗口创建，等待上限为 1 秒；释放 WindowBase 或向已有窗口发送 WM_QUIT 后调用 `0xb880`，后者确实执行 thread join 并清线程字段。这里的一秒是等待窗口创建的上限，不是后续 join 的完成上限。

从观察线程自己的回调重入停止时，原版采用 detach 分支，清线程和窗口字段，避免 join 自己；不能把这个分支也描述成已经等待线程返回。WindowBase 析构 `0xaff0` 对其自有窗口发送 WM_QUIT 并注销窗口类，这个窗口与系统全局隐藏 hWnd 的所有者不同。

### 进程启动观察者与 WMI

`RegisterAppLaunched`（`0x79680`）转换 UTF-8 输入后进入 `0x2c00`：必要时先置 active，再创建并保存 thread HANDLE（owner `+112`），随后把非空名称／路径放进 mutex 保护的集合。被登记的应用已在运行时可由 `0x2950` 的查询立即发 true 事件；该事件和以后 WMI 通知具有不同来源。

worker `0x3600` 调用 `0x1ac0`，连接 `ROOT\CIMV2`，设置 proxy blanket，通过 UnsecuredApartment／ObjectStub 建立 sink，再向 IWbemServices 注册两条 `WITHIN 1` 的 WQL：`__InstanceCreationEvent` 和 `__InstanceDeletionEvent`，TargetInstance 均为 Win32_Process。两次异步注册没有在该函数中检查 HRESULT；日志 Init OK 不证明订阅成立。

sink `0x1450` 读取 TargetInstance、__Class、Name 和 ExecutablePath，creation class 发布 isLaunched=true，其他到达该分支的 class 发布 false。`0x2100` 只有在名称或路径命中登记集合、回调和门禁存在时发布 `appLaunchedChanged`；`name`／`path` 字段分别取决于哪种身份命中，不保证总是同时存在。JSON 序列化字符串在回调返回后立即释放。

成功初始化的 worker 等待 owner event；唤醒后调用 IWbemServices 的 CancelAsyncCall，再 Release services、locator、UnsecuredApartment、stub、sink 及查询所得接口。CoCreateInstance 初始失败会清 active；ConnectServer／CoSetProxyBlanket 失败虽释放已获取的相应接口，却没有在该分支清 active。后续对象创建、QueryInterface、异步注册也没有一个汇总成功门禁，必须逐 API 保存真实结果。

最后一项取消注册（`0x2f00`）、Terminate 和析构（`0x7b8f0`）都会设 active=false、SetEvent，并在 thread HANDLE 存在时 CloseHandle；观察到的路径没有先 Wait/join。析构接着释放登记集合及 owner，也没有关闭 owner 的 event HANDLE。worker 入口自己还会设 active=true，sink 使用另存的全局 owner 指针；Terminate 只清主 lpParameter，未清该别名和回调。静态证据说明 owner、worker 和回调尚无排空证明，不能由此声称实际运行已发生某种竞态或访问错误。

### 网络与本地应用集成对象

原件虚表 `0xddc90` 首槽为 `0x5d530`（原始指针字节 `30d5058001000000`），清独立网络回调并销毁网络 manager；manager 虚表 `0xddca0` 首槽为 `0x5d1a0`（`a0d1058001000000`），进入 `0x5d1e0`。

manager 在 connection point 非空时调用 Unadvise，然后立即把 owner `+0x28` 的 connection point 和 `+0x30` 的接口指针清零，随后才条件检查并调用 Release。这一顺序由 `0x5d227..0x5d25f` 的完整原汇编确认，因此正常走到 Unadvise 的分支没有执行后面这两个 Release；不能把该函数描述为已完整平衡 COM 引用。实际引用数量及运行影响未进行动态确认。

应用集成对象的虚表 `0xddba0` 首槽为 `0x5b2a0`（`a0b2058001000000`）：设 active=0、SetEvent，销毁字符串并释放对象；该函数内没有 join、关闭 event 或 WSACleanup。worker `0x5ad80`／`0x5bb90` 使用该对象和 event，创建处没有保存线程 HANDLE。这里只证明停止信号的源码路径，未证明 worker 已不再访问上下文。屏幕取色器有自己的窗口与线程清理链，见[颜色选择](../shared/color-picker.md)。

### 显式 Terminate 与 DLL detach

DllMain detach 的顺序为图标缓存停止 `0x44080` → 清 hInstance → `0x73d40(0)` → `0x6bd20`。`0x73d40(0)` 关闭 GDI+，并处理网络／应用集成对象的额外销毁；它不是系统隐藏窗口的 DestroyWindow 或 join。显式 Terminate 与 DLL detach 覆盖的资源不同，不能互相代替，也不能在运行时对照中凭 FreeLibrary 返回推导所有回调已不可达。

## 命名退出事件与系统动作

`CreateExitEvent`（`0x79400`）创建 `Global\RazerAppEngineExitEvent`，auto-reset、初始未触发，保存到全局 hEvent；已有 HANDLE 时直接返回 1，创建失败返回 0 并记录 GetLastError。

`SetExitEvent`（`0x79470`）先另行创建 `Global\RazerAppEngineExitingEvent`，成功则 SetEvent；然后若全局 hEvent 非空就 SetEvent 并返回 1。它没有检查两次 SetEvent 的成功值，也没有在函数内关闭新建的 ExitingEvent HANDLE。Terminate 关闭的是保存的 ExitEvent HANDLE。返回 1 因而只对应全局 hEvent 存在并走了发信号分支，不是监听方已经退出的应答。

系统动作导出 `0x70350..0x703a0` 调用 WinExec，命令分别为 `shutdown -h -t 0`、`rundll32.exe user32.dll, LockWorkStation`、`shutdown -r -t 0`、关机分支的 `shutdown -s [-f] -t 0`，以及 `rundll32.exe powrprof.dll,SetSuspendState 0,1,0`。WinExec 返回只对应进程启动接口结果，不是工作站已完成动作或设备已恢复。关机原生函数读取一个 bool 来选择 force 分支，而当前宿主声明数组写为无参 void；这个差异需要保留，不能猜测参数后执行。

## 独立实现和验证边界

本页完成的是所列原件的静态语义核对与追加捕获，没有新增 Rust 系统适配功能，也没有执行官方 DLL、退出系统、窗口运行验收或设备恢复。原版的未检查返回、未等待释放和缓存状态须在对照中保留证据；Rust 所有者仍须遵守[原生资源规则](../../../.agents/rules/platform-ffi.md)，保证取消后排空回调和 worker，再释放其上下文，不能把原版没有的完成证明补写成官方行为。

| 测试对象 | 无设备可执行的独立测试 | 官方实际对照／验收边界 |
| --- | --- | --- |
| 初始化与注册 | COM 各阶段失败、重复初始化、NULL／重新注册、窗口创建失败与迟到完成 | 逐真实 API 结果和事件观察；固定 bool 不能证明初始化全部成功 |
| 电源事件 | 4／7／0x12 差异、显示 0／1／2／未知值、合盖枚举失败、事件顺序、旧代次回调 | 系统消息和真实事件来源单独核对，不把进程内缓存当即时 OS 查询 |
| 关机协调 | 注入 WM_QUERYENDSESSION 与逻辑 timer，核对事件、返回 0、十秒门禁及重复请求 | 不实际触发关机；受限系统动作与真实关机验收保持未验证 |
| 进程观察 | 名称／路径分别命中、初始运行、创建／删除、WMI 部分失败、停止前后回调与取消清理 | 核实完整 ABI、COM 线程与 sink 生命周期后再执行独立只读比较；不能把双边没有事件当一致 |
| 退出释放 | 重复 Terminate、前台回调重入、join／detach、worker 未就绪、在途回调和释放顺序 | 当前未实际执行；线程结束、COM 引用和 HANDLE 清理必须分别有收据 |
| 命名事件 | 创建失败、已有对象、缺 hEvent、SetEvent 失败、重复调用、每个 HANDLE 的关闭 | 真实调用须隔离并核对监听方与全局命名副作用；不能用返回 1 代替接收方确认 |

全功能测试与官方实际执行比较按[验证规则](../../../.agents/rules/verification.md)实施。纯／模拟测试、静态捕获、官方实际功能对照、真实设备写回及运行时验收分开记录；未覆盖的 DLL 功能、平台分支与下层生命周期继续登记到统一 `.analysis/results/gaps.json`，不由本页局部链路推导全部完成。
