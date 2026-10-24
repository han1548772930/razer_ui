# 窗口、页签、休眠与退出生命周期

本文记录主机的本地窗口协调、URL 存储、电源事件和退出顺序。窗口已隐藏、页签已销毁、恢复命令已发送、原生线程已退出是不同事实，不能使用同一个成功标志。

## 当前原件与查询范围

当前稳定宿主为 4.0.827；获取来源、实际字节身份和本轮部署复核见[宿主收据](../../../.analysis/inputs/host-current/acquisition.json)及[元数据复核](../../../.analysis/inputs/host-current/metadata-recheck.json)。原始文件位于[官方 ASAR 静态提取目录](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/)。以下区间均为原始 UTF-8 字节的半开区间，不引用恢复代码的字符偏移。

`python tools/workspace.py audit-host-local --query window-lifecycle` 核对原件、packed ASAR 中的实际成员字节、官方包别名及包来源，保留 25 份完整 JS 文件和 117 个去重 AST 所有者，并核对 BLE endpoint 原件及 16 个完整原生函数，结果写入[统一宿主查询](../../../.analysis/results/host-current/local-audit.json)的 `window_lifecycle`。这些所有者包含完整文件和具体函数，数量不是已完成的产品功能数。当前查询无解析／字节校验失败，但 `source_chain_complete=false`，原生释放、全部页面消费者和运行时验收分别保留未验证状态。

| 原始文件 | 完整行为所有者与字节区间 |
| --- | --- |
| `main.js` | `electronAction` `[39326,58313)`；退出准入 `Dn` `[20963,23367)`；最终清理 `Mn` `[23367,24455)`；窗口退出计时 `Un` `[37592,38836)` |
| `components/Tab/Tab.js` | 创建 `[1798,5805)`；事件 `[5806,11406)`；重建 `[12712,12816)`；销毁 `[12948,13363)`；休眠判断 `[13364,14029)`；休眠 `[14029,14640)` |
| `components/Tab/TabStore.js` | 定时登记 `[1253,2153)`；取消 `[2220,2427)`；活动页状态 `[2915,3108)`；停止休眠列表 `[4613,4855)` |
| `components/Tab/TabManager.js` | 活动页切换 `[4417,4815)`；休眠调度 `[4815,5166)`；显式休眠 `[7716,7847)`；恢复 `[7847,8049)` |
| `components/Tab/common.js` | close 处理 `[2086,2490)`；closed 处理 `[2602,2992)`；`willClose` 更新 `[23178,23248)`；整窗休眠 `[26079,26493)`；恢复重建 `[27097,27281)`；聚焦 `[27945,29209)` |
| `modules/window_storage/index.js` | 命令分派 `[312,2834)`；发送条件 `[2835,2949)` |
| `modules/memory_storage/index.js` | 命令分派 `[312,3486)`；发送条件 `[3487,3601)` |
| `modules/sysutil/win/index.js` | 包装器配置 `[486,4440)`；原生事件消费者 `[5977,7357)` |
| `modules/ffi/FFIPreloadMain.js` | 退出命令登记 `[3227,6350)`；退出／关机／挂起遍历 `[13914,16082)`；同步调用通道 `[11244,12004)` |
| `UsbRzDeviceAction.js` | 事件及所有权 `[692,3049)`；默认恢复 `[34823,39409)`；钩子控制 `[39410,40566)`；挂起／退出命令 `[40741,52348)` |
| `preload.js` | `pagehide` `[2361,2636)` |
| `WssAction.js` | 启动／客户事件 `[279,1973)`；停止 `[2546,2965)` |
| `modules/noble/index.js` | BLE 停止入口 `[18893,19038)`；下层 `endpoint-detect/index.js` 的 stop `[1483,1536)` |
| `mainSubFunction.js`／`lib/common.js` | 设置退出事件 `[10609,10693)`；超时 helper `[8788,8944)` |
| `lib/useLogger.js` | 消费队列 `[5375,7418)`；关闭 `[7418,7509)`；退出日志处理 `[16055,16763)` |

## 身份、显示和焦点

`frameName`／`windowName` 标识实际页签或独立窗口，`windowId` 标识其所属原生窗口。活动页、强制焦点、原生窗口焦点、可见性、最小化和休眠分别存储；不能由 `winMap` 有一条记录就判定窗口或设备仍然在线。

主机的 focus 动作先寻找命名窗口或页签所属窗口，再请求活动页切换；存在 `subTab` 时向实际页签发送 `focusSubTab`。公共 `focusWindow` 对休眠的整窗执行重建，对休眠页签先移除休眠登记并 `reInit`；普通路径按 `willShow` 决定 show、restore 和 focus，再广播 `appFocus`。广播和重建请求不提供页面已初始化或设备读取已完成的确认。

窗口打开处理先解析 URL、`frameName` 和 feature。已存在且仍存活的同名目标复用其焦点链；policy 3 的 `attachToWindow`、policy 5／7／8 的独立窗口路径不能合成一个窗口分支。设备窗口身份和 Dashboard 的 PID／container 导航需要共同核对，见[Dashboard](dashboard.md)。

[固件共享窗口专项](../../../.analysis/results/frontend/synapse-update-fw-current/page-interactions.json)进一步连接真实调用方：SDK 的 activation 经通用 `doElectronAction` 保留 subTab／willShow，main 进入 focus／页签激活／可选 focusSubTab。新页 policy=3 沿 Tab 的 window-open handler、common.createNewTab、TabUI、preload 的 tabEvent 到 TabManager 创建实际 Tab 并 init；已有页切换则发布 activeTabChanged。所选页签关闭路径核对允许关闭／force-focus，按可用分支等待 beforeunload 并发布带 frameName 的 tabClosed；固件迁移指示器消费该事件，但该通知不确认文件迁移、设备恢复或 DLL 释放。设置／迁移的具体焦点订阅和未配对资源分别见[设置](settings.md)与[配置迁移](profile-migration.md)。

## macOS SysUtils branch and event subscription boundaries

The current host keeps a separate [`modules/sysutil/mac/index.js`](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/sysutil/mac/index.js) implementation (4,963 bytes, SHA-256 `b739d78a2253d67de0e2aae4f3e2fc6f4bfc049bd26f0d55a8c5fdee861bc299`); Windows `SysUtilsNative.dll` behavior cannot be reused as its contract. `initDll` first checks the dylib with `fs.existsSync`, then registers `GetDylibVersion`, `FreeMalloc`, `SetNodeFFIEvent`, `Initialize`, workstation power, application/window queries, network status, GMS3, foreground-window, and keyboard-layout exports through `ConfigureFFI`. It sets `isInit` only when `callDLLMain(ConfigureFFI)` is truthy, then separately calls `GetDylibVersion` and `SetNodeFFIEvent` with `useNewCallback:true`; missing files and exceptions are logged without a failure state or release acknowledgement.

Foreground subscriptions are de-duplicated by sender URL. `StopMonitorForegroundWindow` removes that URL, returns `true` while other URLs remain, and awaits `callDLLMainAsync` only for the last subscriber. Keyboard layout uses a separate `keyboardLayoutChangeEventList` on start, but the original `StopMonitorKeyboardLayout` filters `foregroundEventList` and checks the foreground-list length before deciding whether to call native code. That can retain keyboard subscriptions or skip the final native stop. `getNetworkStatus` short-circuits to `true` when `isInit` is false, so it is not an online-state observation; other `callDLL`/`callDLLAsync` paths forward directly to the Mac FFI.

The callback parses JSON and returns when `event` is absent. `networkStatus` emits a global event, `systempoweroff` emits the local event, and `suspend` emits a global event. Forwarding skips destroyed, crashed, or disposed webContents; `foregroundWindow` and `keyboardlayoutchange` are filtered by their respective URL lists while other events broadcast. This proves JS dispatch only, not dylib-thread, callback-handle, power-operation, or Rust-observer release. The Rust workspace has no matching Mac SysUtils export table or the two independent URL subscription sets; dylib loading, event results, cancellation, and release remain unexecuted.

## 关闭、隐藏与真正销毁

普通 close 处理先隐藏窗口；`willClose=false` 时阻止默认关闭。policy 5 也有独立的 hide／preventDefault 分支；policy 7 在最小化时隐藏；policy 8 在允许关闭时先关闭 childViews。页面工具栏的 `handleCloseBtn` 还受 `closeWindowAction` 控制，可能关闭，也可能只隐藏并安排休眠。

实际 closed 事件区分休眠倒计时、全局退出和普通关闭。倒计时中关闭且未全局退出时保留休眠记录；全局退出删除记录；普通允许关闭或 policy 7 才进入对应移除路径。`setWillCloseWindow` 通过 `Map.set` 存入新对象，closed 回调先保存的旧对象仍保留原 `willClose`；不能因随后写入 false 就断言该分支永远不清理。

Tab 的 destroy 先从父窗口移除 view，再请求 `webContents.close({waitForBeforeUnload:true})`，仅在缺少 close 方法时使用 destroy。发出 close 不等于 beforeunload、设备恢复和原生资源释放已完成。独立 simple window 的 destroyed 回调通过箭头函数保存 URL，而 Tab 的 destroyed／render-process-gone 回调创建的 `sender.getURL(){return this.url}` 没有保存 Tab URL；不能将两种清理写成同一正确闭包。

## 页签与整窗休眠

默认休眠间隔为 120000 ms。普通 `beforeHibernate` 先读取 WindowStorage 的 `STOP_HIBERNATE`，解析各 URL 的值并展开，再判断当前活动页和窗口可见性、拒绝列表及手动关闭规则。活动且可见的页不休眠；需要继续观察的非休眠项会重新安排计时。异常只记录，不能推导为已完成休眠。

固定拒绝列表包含灯光引擎、OTFM、后台管理、托盘、通知管理和游戏服务代理等；名称匹配 `*_mw`、包含 install／download 及手动关闭标记也影响页签判断。保留原件里的在线同步项仅用于解释来源，不把它接入本产品。

显式 `hibernatePage` 使用零延迟回调直接调用 `tab.hibernate()`，不先执行普通 `beforeHibernate` 的 STOP 列表检查。整窗 `hibernateWindow` 还要求应用在已启动列表、名称可休眠且当前状态为 NONE；进入 ON_COUNTDOWN 后才遍历所属页签。不能把显式页签休眠、隐藏后整窗调度和默认空闲计时互相替代。

Tab 休眠移除活动页状态并销毁其 webContents，保留重建所需身份和 feature。恢复使用原 Tab 的 `reInit`／创建链，再请求激活；独立窗口重建显式清除自动启动和后台启动选项，并设置 `isRestoreFromHibernate=true`。恢复页面后仍须由对应设备功能按真实初始化／读取顺序重新观测状态，不能把旧配置写回视作恢复完成。

## URL 存储与通知

WindowStorage 和 MemoryStorage 是宿主按 URL 划分的内存 Map，不是磁盘文件或设备状态。get 返回所有 URL 下该 key 的 truthy 值组成的 JSON 数组；false、0 和空字符串不会出现在返回数组中。set 返回旧值；MemoryStorage 的 NoEvent setter 返回 true 且不广播，不能据此证明业务动作成功。

写入可指定 payload.url，remove 使用 sender URL，注册也按 URL。事件发送排除 sender、远端和已销毁目标；已知新窗口 URL 需要显式注册，兼容旧窗口 URL 的条件另行保留。MemoryStorage 的 `targetUrlArray` 按子串逐项匹配，同一 URL 命中多个片段可能收到多次事件，不能自行去重后宣称与原版相同。clear／reset 没有逐项 removed 广播。

Tab 清理所构造的新 sender 没有 `url` 字段，`getURL()` 返回 undefined；WindowStorage 因而未证明删除真实 Tab URL 的记录，MemoryStorage 则在缺 URL 时直接返回 true。`preload` 的 pagehide 只直接移除 `tab-message`、`async-message`、`windowStorageEvent` 和 `window-list-change` 监听，关闭本地 BroadcastChannel，并按 iframe 条件发送 close-iframe；不能扩展为全部 IPC、存储注册或原生回调已注销。设置写盘的另一条 session flush 链见[全局设置](settings.md)。

## 电源事件与设备恢复命令

Electron powerMonitor 的 suspend／resume 在 Windows 刻意忽略。Windows 的实际 JS 消费者为 SysUtils 的 `handleFFIEvent`：解析原生 JSON，缺 event 时返回；`systempoweroff` 发包装器事件，suspend 发全局事件并按 `exitWhenSleep` 决定是否请求 quit，resume 发全局恢复事件，随后向有效 webContents 转发。前台窗口／键盘布局事件额外按订阅 URL 筛选，其他事件走默认广播。原生事件产生、线程与释放另见[系统集成](system-integration.md)。

UsbRzDeviceAction 的 suspend 顺序为禁用全部设备钩子 → 已登记的默认恢复命令 → 挂起命令 → 退出命令；lock-screen 只禁用钩子并执行默认恢复。resume／unlock-screen 查询锁屏状态后按分支重新启用钩子；查询异常或缺 SysUtils 的兼容分支也会发启用请求。exit-app 执行默认恢复和退出命令。`tabDestroyed` 按事件 URL 释放该页持有的设备 mutex；`stop()` 本身只调用 stopMonitoring。

这里的恢复数据来自各产品已经登记的命令，不能据此生成通用出厂默认值。挂起／退出登记的身份包含 container、claim interface、PID、归一化的 usagePage／usage、可选 USB instance 和 transactionIdCmdSteer；相应移除要匹配同一身份，不能只按 PID 清空全部设备。

发送器按命令下标轮转设备，按实际接口选择 HID handle。90 字节报文在适用的 reportLength 分支加上 reportId（缺省使用 claimInterface）；明确协议为 `"25"` 或协议缺省且长度为 90／91 时才进入相应写入路径。默认恢复的兼容路径可能调用异步 sendFeatureReport 并仅注册 then／catch，外层不会等待；其他路径也不能因为函数名含 NoAsync 就推导全部 I/O 已完成。

同步发送最多尝试三次，返回长度 91 或 65 后才进入相应应答处理。默认恢复只为已识别的 device-mode／LED-matrix／Chroma 命令追加读回；device-mode 最多读五次，其他最多三次。挂起／退出的 busy 与 timeout 分支不同于默认恢复，读异常和重试耗尽也没有统一的失败返回。因此必须保存实际发送、全部应答及错误，不能用函数返回或日志末尾证明确认成功。

默认恢复在发送长度被接受后、尚未确认设备成功时就记录 `sentSetDeviceModeCommands`；退出序列用该集合跳过同身份同命令目标的 device-mode。该集合只能解释原版去重，不能作为已观测设备模式或恢复成功状态。

## 应用退出与最终释放

`setCanExitAppEngine` 维护阻止退出的应用名列表。命名应用在名单中时拒绝退出；整宿主退出在名单非空时前两次拒绝并递增计数，随后进入强制继续分支；`forceQuick` 是独立的函数参数。拒绝时发送 `exitAppRejected`，不是已取消全部后台工作的确认。

获准后 `Dn` 更新已启动应用列表／全局退出状态，广播 `exitApp` 及剩余 runningApps，等待 200 ms，再发全局 exit-app。无需继续保留灯光引擎时，最多十轮、每轮 200 ms 等待：系统关机看 canShutdown，普通退出看 exitCompleted；超时仍继续关闭。随后 await `Ln.stop()`（本机插件 WebSocket 服务），按完整退出或 simple quit 选择要关闭的窗口。

WssAction 的 stop 请求关闭现有客户、清除各客户 30000 ms pong interval，再调用服务器 close；它在 close 回调执行前就返回 `{result:true,reason:""}`，该 await 没有等待服务器停止。关闭错误只记录日志，成功回调才清空服务器及 clients。连接按四种允许的 clientName 入表；同名新连接替换旧项，而旧连接的 close 回调只按名字删除，也可能删除后来的客户及计时器。命名相同不代表同一连接，这条原版语义必须在生命周期测试中单独覆盖。

`Un` 对全退出逆序请求关闭所有窗口，10 秒后再请求一次，另过 5 秒调用最终清理，再过 5 秒调用 app.exit。三个计时边界都不是释放确认。window-all-closed 也会调用 `Mn`，原件没有在这些入口之间展示一个保证只执行一次并等待同一完成结果的屏障。

`Mn` 的实际顺序如下：

1. 标记 isQuit，停止托盘重建订阅并取消 250 ms 合并计时。
2. 等待各隔离 FFI 进程的 `APIToCallWhenExit` 遍历。
3. 执行主进程登记的退出设备命令、关机命令，再等待通道退出 API 遍历。
4. 调用 `mainSubFunction.setExitEvent`，不 await 其 SysUtils 请求；随后对 SysUtils Terminate 使用独立 3000 ms 超时及 catch。
5. 停止 USB 监控，执行 `stopBLEEndpointMonitoring`；再对 mappingEngineShutdown、simpleServiceShutdown 各使用独立 3000 ms 超时及 catch。
6. 等待 `saveLoggerBeforeQuit`，调用 app.quit，销毁 tray。

BLE 停止先调 endpoint adapter 的 stopMonitoring，再对缓存的外设调用 disconnect；没有等待断开回调，也没有在这段函数中清空全部缓存或确认 native 线程退出。`setExitEvent` 的系统事件对象及原生清理见[系统集成](system-integration.md)。部分前置 await 没有局部 catch，抛出时后续步骤不保证继续；三秒 helper 仅执行 Promise.race，不取消原调用。before-quit 另停止本地监控并请求 Haptic.stop；账号和在线退出逻辑只保留来源上下文，属于产品排除范围。

日志退出先通过 Promise.all 等待各进程／URL 的 close，之后才对剩余 waits 实施 30000 ms 轮询上限，末尾清 interval 并置 isQuited。这个上限不覆盖前面的 Promise.all。日志 consumer 的部分文件／transport 错误只记录或丢弃缓存；超时继续退出也不能声称全部日志已持久化，更不能作为设备命令完成证明。

FFIPreloadMain 的普通退出 Map 按 channel 保存 API 名；设备／关机／挂起 Map 按 `channel-productId-deviceContainerId` 保存命令数组。后一组遍历从 key 拆回 channel，按顺序调用不经过普通 mutex 的同步通道，命令间使用 payload.delay（缺省 2 ms），不核验统一完成结果。普通 callDLL／callDLLAsync 有按 channel 的 mutex，这不自动覆盖同步退出通道；FreeFFI 的公开分支只返回 true，内部对应 handler 也是空操作，均未释放库或回调。

隔离 FFI manager 逐进程 await 调用并捕获异常，但“该调用已返回”和“进程／原生 worker 已退出”仍需下层证明。此处保存官方调用仅用于逆向与显式对照；Rust 产品不能复制这些 DLL 转发器作为实现。

## BLE endpoint 原生监控与释放

宿主 endpoint adapter 的 `start`／`stop` 继续进入 `node-rz-ble-endpoint-detection` 的 JS 包装器。包装器另存进程内 `started`：先改标志，再调用原生入口；没有就绪或停止完成回调。它注册一次 add／remove 回调，并按 VID／PID、VID 和通用名发布多组事件；这些发布名称不证明实际对象有数值型 VID／PID。宿主 find 会接受 Mouse，持续 add／remove 分支却只接受 AudioEndpoint、Keyboard、HIDClass，不能将两种入口的产品集合合并。

当前官方宿主包内的 `app.asar.unpacked/node_modules/node-rz-ble-endpoint-detection/build/Release/detection.node` 与保留别名按实际字节核实一致。其[完整捕获](../../../.analysis/results/native/d5c680965d558be4381aee898ba9c451b4c4e50bd78dc4a67579487eead93f13/capture.json)为 IDA 9.1／Hex-Rays 9.1.0.250226、797 个函数；本轮重新验证全部函数与 9 份汇总产物，并逐原件字节核对下列 16 个完整函数。以下地址为该原件的 RVA。

| 所有者 | 原件中的完整行为 |
| --- | --- |
| addon 注册 `0x3af0` | 注册 find、registerAdded、registerRemoved、startMonitoring、stopMonitoring；动态解析系统 SetupAPI，创建两个 auto-reset event，初始状态分别未触发／已触发，再执行 `0x6ab0` 初始设备清单读取 |
| start `0x39d0` | active 为 false 时先置 true，CreateThread 启动 `0x5cc0`，未保存线程 HANDLE；初始化两个 uv signal handle，分配 176 字节 work 并排队，不检查线程／uv 就绪结果 |
| 窗口线程 `0x5cc0`／WndProc `0x5c60` | 注册按线程 ID 命名的窗口类，创建隐藏窗口并登记两种设备接口通知；进入按该窗口过滤的 GetMessage 循环。WndProc 将设备到达／移除传给 `0x72e0`；所读线程函数未保存通知 HANDLE，也没有退出后的通知注销、DestroyWindow 或 UnregisterClass |
| work `0x5a00` | active 时启动信号 2／15 的处理并无限等待第一个 event；该等待只对应通知工作，不等待窗口线程结束 |
| after-work `0x5a60` | active 时按 add／remove 标志调用消费者，remove 分支释放临时记录，触发第二个 event、清 lpMem，再用原 work 重新排队；active 已清时整个分支跳过，包含这次临时记录清理和生产者握手 |
| stop `0x3aa0` | active 时清标志、停止两个 uv signal、SetEvent 唤醒 work；没有投递窗口退出消息、join、uv_cancel／uv_close、event 关闭或回调注销。未启动分支的完整汇编没有给 RAX 赋值，不能把反编译返回值解释为稳定 bool 契约 |
| 回调登记 `0x1c00`／`0x2480` | 保存各自 V8 persistent callback 和门禁；重复登记路径覆盖全局指针，没有在这些函数内释放旧 persistent。add／remove 消费者为 `0x1d20`／`0x25a0`，stop 未清其门禁和指针 |
| find `0x2d00` → `0x5b10` → `0x3020` | 独立 uv 工作遍历原生清单、复制匹配记录、构造 JS 对象并调用回调，再释放其结果清单与 work 上下文；这条完成回调不等于持续监控线程已经退出 |

设备事件生产者 `0x72e0` 先规范化实例路径，查询 SetupAPI；匹配对象后无限等待第二个 event，再构造新增／移除记录，发布给 work。来源还受 `BTHLEDevice\\`／`BTHENUM\\` parent 和实际 deviceClass 条件限制。stop 没有触发第二个 event，after-work 在 active=false 时也跳过此握手；因此源码没有给出生产者等待和通知线程已结束的证明。这里记录的是静态路径，尚未动态复现阻塞、泄漏或竞态。

这段 native 没有向宿主 JS 包装器提供“所有通知、回调与线程都已排空”的统一确认；不能由 started=false、SetEvent 返回、find Promise 已结束或宿主 disconnect 调用推导完整释放。Rust 实现仍须由会话所有者停止新工作、取消在途传输、排空回调并等待线程，然后释放资源，并分别保留原版停止行为和自身清理证据。

基准唯一的 UTF 辅助函数失败 `0x1b2a8` 已依据其完整寄存器、尾调用和被调函数汇编恢复类型；[独立 supplement](../../../.analysis/results/native/d5c680965d558be4381aee898ba9c451b4c4e50bd78dc4a67579487eead93f13/supplements/71c2cd4583fc0dc8/capture.json)保留 7 个完整函数、0 失败，基准捕获与数据库未变，完整性问题为 0。它只解决反编译缺口，不能提升为 BLE 协议、全 DLL 语义或真实设备验收完成。

## 实现与验证边界

本查询完成的是上述宿主原件的字节核对和具体语义阅读；没有运行厂商 JS、应用窗口、官方 DLL 或设备，也没有因这份文档新增 Rust 功能。独立 Rust 实现、全部产品接线、官方实际功能对照和运行时验收不能由该查询通过推导。

| 验证对象 | 必须能区分的输入与结果 |
| --- | --- |
| 关闭和焦点 | hide／close／closed、willClose、policy 3／5／7／8、前台／后台恢复、旧窗口身份回调 |
| 休眠 | 活动可见、隐藏、STOP 列表、手动关闭、显式零延迟、取消倒计时、重建失败及再次恢复 |
| URL 存储 | false／0／空值、指定 URL 写入、sender 移除、重复目标匹配、注册／注销、销毁时缺 URL |
| 电源和 HID 恢复 | suspend／lock／resume／unlock、真实登记身份、缺 handle、异步兼容路径、busy／timeout／错误应答与恢复确认 |
| 退出 | 拒绝退出计数、应用退出与全退出、两种灯光完成旗标、超时、前置 Promise 失败、重复最终清理、在途命令和回调释放 |

无设备时先对真实状态所有者和模拟传输运行可执行测试，期望值来自以上原件。官方 DLL 实际对照使用已核实 ABI 的显式入口，先只读；设备恢复写入只有在原值、交叉确认及完整恢复／清理条件均成立后执行。主应用窗口和其他受限运行验收保持未验证，统一缺项继续进入 `.analysis/results/gaps.json`。
