# USB 发现、HID 传输与停止生命周期

USB 热插拔监视、HID 句柄、互斥令牌、原生工作队列和 JS 事件订阅分别拥有生命周期。停止监视不等于关闭设备；关闭设备也不等于排空已入队的 I/O、Promise 或回调。应用层触发恢复、停止和退出的顺序见[窗口生命周期](../host/window-lifecycle.md)，设备恢复报文仍按各产品协议判断。

## 当前来源与地址

宿主 4.0.827 的获取与实际包字节见[获取收据](../../../.analysis/inputs/host-current/acquisition.json)及[部署复核](../../../.analysis/inputs/host-current/metadata-recheck.json)。以下 JS 位于[当前 ASAR 静态原件](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/)，区间为原始 UTF-8 字节的半开区间；native 地址均为 RVA，Image Base 均为 `0x180000000`。

| 对象 | 本文实际读取的证据 |
| --- | --- |
| `electron/UsbRzDeviceAction.js` | `initUSB` `[3049,3883)`；`handleAction` `[3883,27427)`；`writeAsync/readAsync` `[27428,28809)`；设备身份及读函数 `[32332,34716)`；`stop` `[34717,34823)` |
| `electron/lib/RzMutx.js` | 完整 1569 字节文件；取得令牌、保有期限、取消等待和释放 |
| `electron/modules/hidHardwareEvents/index.js` | 完整 1826 字节文件；专用事件设备的打开、数据消费者和注销 |
| `node-rz-hid` 0.0.31 | 完整 `nodehid.js` 4379 字节；[HID.node 全函数捕获](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/capture.json)，1641 个函数 |
| `rz-usb-detect` 0.0.22 | 完整 `index.js` 2964 字节；[detection.node 全函数捕获](../../../.analysis/results/native/2009eb2e0853c329612c53416664ab016f287d601edb9ba07dfd981e16cf6f3b/capture.json)，936 个函数 |

JS 查询核对获取索引、ASAR packed 成员的实际字节和官方包同字节来源，解析无 ERROR/missing node；原生原件、逐函数文件及九份全量汇总的实际摘要均复核。两份基准捕获实际使用 IDA 9.1 / Hex-Rays 9.1.0.250226；其中 USB `0x20668`、HID `0x30888` 的原始反编译失败保持不变，完整汇编仍在。下述私有补获已恢复两处完整伪代码，不能将这一结果扩大为全部产品语义、Rust 实现或设备验收完成。

## UTF 转换与类型恢复

两处 owner 分别为 USB `[0x20668,0x2070b)` 和 HID `[0x30888,0x3092b)`。原失败均为 call analysis failed，基准错误地址分别为 `0x206f5` 和 `0x308bb`。重新分析只在保留数据库的私有副本中进行，不改原件、基准捕获、基准数据库或原失败记录。

| 对象 | 成功补获与完整调用方 |
| --- | --- |
| USB | [补获收据](../../../.analysis/results/native/2009eb2e0853c329612c53416664ab016f287d601edb9ba07dfd981e16cf6f3b/supplements/fabbfe5280088b76/capture.json)：9 个完整函数；owner `0x20668`、标量编码器 `0x2a890`、清状态 `0x2a934`、错误处理 `0x2a93c`，以及直接调用方 owner `0x20148/0x204a8/0x15c48/0x16180/0x2b728` |
| HID | [补获收据](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/supplements/c98d562484439a06/capture.json)：8 个完整函数；owner `0x30888`、标量编码器 `0x3ac8c`、清状态 `0x3ad30`、错误处理 `0x3ad38`，以及直接调用方 owner `0x30368/0x306c8/0x24f88/0x3bdf8` |

错误 helper 的全部指令只读取 RCX 的状态指针与 RDX 的错误上下文：清零八字节状态，返回 `-1`，设置上下文 `+0x30` 的一字节错误标志和 `+0x2c` 的四字节错误码 42，没有读取 R8、R9 或栈参数。标量编码器只消费 RCX 输出缓冲区、EDX 标量、R8 状态和输入 R9 错误上下文；其错误尾跳再把 R8/R9 转交给上述两参数 helper。完整调用方也按这些寄存器传参。补获仅据此明确两参数错误 helper 与四参数编码器类型，不猜测额外栈参数。

仅恢复错误 helper 的先前 [USB 尝试](../../../.analysis/results/native/2009eb2e0853c329612c53416664ab016f287d601edb9ba07dfd981e16cf6f3b/supplements/dbfe3d843fea31ec/capture.json)和 [HID 尝试](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/supplements/566417cf283f8a4e/capture.json)仍各有一处 owner 失败；它们保留了编码器尚含两个多余栈参数的数据库类型，不能作为成功收据。最新两份补获均为完整伪代码、零反编译失败、只读数据库校验零异常；逐函数字节、边界、指令字节与入口交叉引用均与基准一致，基准原件和数据库实际摘要也保持不变。只读解析器分别核实原失败已恢复，各自 `remaining_failures=0`；同时各有一条 `verification_failures`，排除上述未完整补获的旧尝试。这两项结果独立保留，不能改写为所有补获零失败，也不能只凭 session 的 passed 标记声称伪代码完整。

恢复出的 owner 是带状态的 UTF-16 code unit → UTF-8 转换。第三参数为空时，分别使用 USB `0x478c8`、HID `0x5dca8` 的全局状态；这是运行库状态，不是设备连接或协议应答：

| 输入与当前状态 | 原版完整指令行为 |
| --- | --- |
| 无待配对状态，普通 BMP 字符 | 交给标量编码器，产生 1–3 字节；NUL 写一字节零并清八字节状态，返回 1 |
| 无待配对状态，高代理 `0xd800..0xdbff` | 将 `((unit << 10) & 0xfc9ffc00) + 0x10000` 存入状态低四字节，返回 0，不输出字节 |
| 有待配对状态，低代理 `0xdc00..0xdfff` | 合成状态低四字节加 `(unit & 0x23ff)`，以独立零状态调用标量编码器，随后清原状态并返回编码长度 |
| 孤立低代理，或待配对后收到非低代理 | 清八字节状态，置上述错误标志／错误码并返回 `-1` |
| 标量编码器的独立边界 | 拒绝代理区标量和大于 `0x10ffff` 的值；输出指针为空时清状态并返回 1，不写输出；普通 ASCII 返回 1，多字节成功清状态并返回 2–4 |

USB `0x15c48/0x16180` 和 HID `0x24f88` 只在当前代码页为 65001 时走此 owner；遇到 `-1` 将格式化输出计数标为失败。字符串转换调用方 USB `0x2b728` / HID `0x3bdf8` 还区分直接输出、剩余容量不足四字节时的临时缓冲、仅计长度、NUL 结束和源游标更新。不能将 owner 的长度或编码成功当作设备传输完成。该运行库链仍需覆盖代理对跨调用、错误后状态清零、NUL、空输出指针、容量边界和调用方失败消费的独立测试；本次未执行原版或 Rust 对照。

## USB 发现与通知交付

`initUSB` 先调用 `startMonitoring()`，再登记 add/remove listener；事件发往仍存活的 webContents，同时进入全局事件总线。`stop()` 只调用 `stopMonitoring()`，没有遍历 HID map，也没有执行设备恢复、注销这些 JS 监听或确认各产品任务完成。

native 登记 `0x4900` 将 find、registerAdded、registerRemoved、startMonitoring、stopMonitoring 分别连接 `0x3620/0x1b80/0x28d0/0x4820/0x48f0`。模块初始化调用 `0x8ce0` 加载系统 `setupapi.dll` 并解析八个 SetupDi 函数，创建两个 auto-reset event：通知 event 初始无信号，交接 event 初始有信号，然后 `0x92d0` 枚举初始设备。缺 DLL 或符号走日志与终止分支；不能写成返回一个空设备列表。

start 设置 active，创建 Windows 监视线程 `0x9060`，初始化两个 uv_signal 对象，再分配 176 字节 uv_work 并登记 execute `0x8b10` / after `0x8b70`。此入口没有将线程、uv 初始化和排队错误逐项转为 JS 失败。JS 自己的 started 标志先置真，也不能证明原生资源全部建立。

Windows 线程注册名为 `ListnerThreadUsbDetection_<threadId>` 的窗口类，创建隐藏通知窗并 RegisterDeviceNotification。WndProc `0x8fe0` 处理 WM_DEVICECHANGE 的 arrival/removal 和 type=5，调用 `0x95c0`。该函数将通知路径转换为实例身份，SetupDi 枚举并精确匹配后，等待交接 event，再构造通知数据并设置通知 event。

arrival 分配带内部身份的 232 字节记录，存入映射并调用 `0x6ee0` 取得真实属性。该属性读取包含 registry、容器 GUID、接口路径与有限等待的接口解析：50 ms 起指数增加，等待预算 5000 ms，错误码 259 进入另一个枚举分支，最多 100 次尝试。它不是无上限的产品初始化重试。removal 的 `0x80e0` 优先按已登记身份取得记录，使用 `0x4f30` 复制 216 字节公开字段，删除映射项并释放原对象；查不到则构造临时记录再次查询属性。

uv execute 等待通知 event；after 仅在 active 时调用新增 `0x1ca0` 或移除 `0x29f0` 回调，移除通知随后释放临时记录，设置交接 event，清 lpMem 并再次排同一 uv_work。回调实际字段包括 locationId、vendorId、productId、productName、deviceContainerId、devicePath、usbInstanceId、manufacturer、serialNumber 和 deviceAddress，经 Node 的异步初始化／销毁通知包围执行。通知身份和产品在线／可读／设置成功不能合为一个状态。

## USB stop 实际等待了什么

stop `0x48f0 → 0x8c20` 的实际顺序是：

1. 清 active，停止两条 uv_signal，设置通知 event。
2. 有窗口时 PostMessage(WM_CLOSE)。
3. 有监视线程时无限等待它退出，关闭线程 HANDLE 并清线程 ID。
4. FreeLibrary(setupapi.dll)，清模块句柄。

Windows 线程正常退出消息循环后注销设备通知、销毁窗口、注销窗口类并清相关全局；注册类失败直接返回，创建窗失败没有对称注销已注册类，注册设备通知失败也没有执行正常尾部的窗口／类清理。stop 等到线程结束不证明这些失败路径已释放全部资源。

stop 没有关闭两个 event、uv_close 两个 signal、释放 uv_work、清空设备映射或 reset 保存的 V8 persistent callbacks。active=false 时 uv after 直接返回，也不再设置交接 event 或释放临时移除记录。start 不重复模块初始化的 `0x8ce0`，因此不能假定一次 stop 后再 start 已完整重建 SetupDi 资源。

还存在需要单独测试的静态交错：窗口线程可能正等待交接 event；stop 仅设置通知 event 并等待窗口线程；随后 after 因 inactive 跳过交接信号。源码未展示一个保证此时能够排空的屏障。这里记录的是等待条件与缺少的保证，未运行并复现该交错，不能称已观察到死锁。

## HID 身份、打开和互斥

`buildDeviceKey` `[32332,32594)` 包含 productId、containerId、interface、usagePage、usage、usbInstanceId。`getDeviceFromDeviceMap` 先精确查找，再执行特定兼容匹配；请求带 instance 时，兼容分支要求已有 instance 为空。其他调用仍可能退到只按 containerId／可选 instance 返回第一条记录的 `getDeviceWithDeviceContainerId`，不能把所有操作都描述为接口身份唯一性已验证。

open 分支 `[6845,10659)` 使用独立的 open mutex；已有记录直接返回 `already opened`。新句柄没有 queryData 时直接登记；有 queryData 时按 queryMethod 调 write 或 sendFeatureReport，以返回 `>0` 或输入长度作为登记条件。write 后安排 `hidRead(...).then` 仅记录日志，不等待或消费读值；这个打开结果不能当成初始化命令已获业务确认。有 queryData 但返回失败／抛异常的分支没有展示对刚打开句柄的统一 close。

remove 分支 `[5486,6845)` 先取得设备列表 mutex，按 product/container/可选 instance 遍历，依次调用设备 close、该设备 mutex.cancel，再删除 deviceMap 和 openMutexMap。未命中也返回 `result:true`，不表示实际发现并关闭了句柄，更不表示完成设备恢复。

sendFeatureReport、write 及其 mutex 变体分别保留锁跨读取或显式 release 的语义。`RzMutx` 的保有期限到达会释放令牌，但不会中止前一原生操作；cancel 取消等待锁、清计时并释放当前令牌，也不取消设备 I/O。显式 release 还要求 sender URL 与持有 URL 相同。页面销毁时释放锁的上层链见[窗口生命周期](../host/window-lifecycle.md#电源事件与设备恢复命令)。

## 同步读、超时与异步队列

同步 `hidRead` `[34337,34513)` 最多五次 `readTimeout(10)`，仅空数组继续。`hidReadAsync` `[34514,34716)` 调 `readTimeoutAsync(10)` 时缺少 await：它立即检查 Promise 的 length，第一轮就返回该 Promise，再由 async 返回值吸收其结果。因此不能把这条路径写成已经执行五次空应答重试。`readAsync` 的 catch 记录错误并返回空数组；空数组无法单独区分无数据和读取失败。

底层 `hid_read_timeout(0x17800)` 在没有 pending read 时，以实例固定 buffer 和 OVERLAPPED 发 ReadFile；发生 pending 后等待 event。timeout>=0 时任何非零 wait 结果都返回 0，保留 pending 标记，后续读取继续等同一请求，没有因超时取消。完成后 GetOverlappedResult 清 pending；reportId=0 时去掉首字节，否则保留，并受调用者容量截断。`hid_set_nonblocking(0x17a00)` 只改 blocking flag；`hid_read(0x179f0)` 按此 flag 选择 -1 或 0 timeout。

`hid_write(0x175e0)` 将短报文补零到 outputReportLength，发 overlapped WriteFile，ERROR_IO_PENDING 时等待 1000 ms 并 GetOverlappedResult。超时／错误分支没有 CancelIo 或等待排空，退出前释放临时 padding buffer。完整汇编还确认：WriteFile 同步返回真会直接跳到尾部，此路径保留初始 `-1`，只有 pending 后成功取回长度才更新返回值。必须保留这项原版差异，不能仅凭驱动接受写入就推导原版向消费者返回成功。

构造器 `0x5440` 以实际路径查全局 map，为每条路径共享一个 worker `0x45d0`，HID 对象持 shared ownership。请求入 `0x5280` 的队列后由 `0x50e0` 顺序取出，执行 `0x4800`：kind 0/1/2/3/4/5 分别为 send feature、get feature、get input、batch feature、write、read timeout。请求保留入队时的 raw HID context；它不是执行时重新读取对象的当前句柄。

Promise 的 `0x15ad0` 建立 deferred 和 TSFN，成功 `0x15fe0` 保存结果函数并标状态 1，失败 `0x160e0` 保存错误并标状态 2，再 release TSFN。finalizer `0x161e0 → 0x15d40` 在 JS 侧生成结果或 Error、resolve/reject deferred，然后释放 request buffer、request、结果函数和 deferred owner。进入队列、传输返回、Promise settle 和页面消费是四个不同边界。

## 事件读与关闭

`nodehid.js` 通过 for-in 绑定可枚举 native 方法；原生注册 `0xcc90 → 0x104d0` 将 close/read 的属性设为 0，其余上述数据方法设为 enumerable=2。因此实例不覆盖 JS 的 close/read 包装器。close 先置 `_closing`、移除全部监听，再 `_raw.close()`，最后置 `_closed`；pause 只改 `_paused`，不取消已开始的 read。

首次新增 data listener 用 nextTick 请求 resume。resume 的回调在成功后，若未 pause 则先排下一次 read，再 emit 本次 data；错误置 pause，closing 时抑制 error 事件。native callback read `0x6770 → 0x61e0` 建立 N-API async_work，`0x63f0` 每次用 50 ms read timeout 等待，直到有结果／错误或检测到对象句柄为空。完整构造汇编经 `0x2f60 → 0xe6e0` 为 HID 对象和 callback 建立强引用；反编译器遗漏该 helper 的第二参数，不能据暂定签名断言没有保活。

N-API completion `0x3d50 → 0x3d70` 按状态和错误选择成功 `0x6560`／失败回调，随后经 `0x3a70 → 0x6710 → 0x39d0` 释放 buffer、async_work 和持久引用。close 没有等待这些 completion，pause 也不改变 native 的 50 ms 循环。

native close `0x94e0 → 0x6170` 与 C 导出 `hid_close(0x17de0)` 均执行 CancelIo、关闭读／写 event 和 file HANDLE、释放错误与读取 buffer、释放 HID context；对象 close 随后清 context 指针。这里使用的 CancelIo 只取消调用线程发起的 I/O，不能当成取消另一个 worker 的请求；没有 CancelIoEx、overlapped 完成等待或队列 join。已入队 raw context 与 close 的先后必须单独验证，静态证据不能冒充已复现的内存错误。

对象 GC finalizer `0x109a0` 调实际析构 `0x53a0`，才在 close 后减少 worker shared count；全局 map 仍保有一份。map 在 CRT atexit 登记的 `0x42760 → 0x122d0` 释放其所有权，最后一份 strong count 才进入 control block `0x10ae0 → 0x4710`。此间接槽已由[私有追加捕获](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/supplements/55780c19f3d50c05/capture.json)核实，基准捕获和数据库保持不变。

worker 析构置 stop、唤醒 condition，存在非当前线程时无限 join 并关闭线程句柄，之后 `0xeaf0` 释放 deque；循环在外层检查 stop，已进入的内层循环继续执行，直到当时可见队列为空，其间没有逐项 stop 检查。若尚未进入该循环就见 stop，可直接退出；deque 清理只释放容器存储，没有逐请求 reject/释放链，不能把 join 当成所有 Promise 都已 settle。`hid_exit(0x16800)` 和 env cleanup `0xc600` 只卸载系统 hid.dll 并清初始化标志，不遍历设备句柄或等待上述 worker。

事件专用设备另由 `hidHardwareEvents` 按 vendor/product/container/usage/usagePage 筛选且要求唯一命中，打开独立 HID 并登记 error/data；数据发往 mappingEngine 已登记 URL，按配置补 reportId。unregister 移除监听、close 并删记录，未命中也返回 true；该返回没有提供在途 callback 排空确认。

## 独立实现与验证边界

本文完成上述当前原件的静态语义与具体间接目标核对，没有增加 Rust 传输或设备功能，没有执行厂商 JS、官方模块、设备操作或应用。通用 USB 包、BLE endpoint 和全部产品协议不能由这两份模块的阅读完成推定；宿主退出上层及 BLE 停止入口继续回查[窗口生命周期](../host/window-lifecycle.md)。

独立 Rust 需要分别拥有监视会话、精确接口身份、设备上下文、请求与回调的代次和停止屏障。取消后拒收旧连接结果；排空 I/O 和回调后再释放 buffer、句柄与系统库。原版缺少的释放保证按项目资源安全约束实现，同时在对照记录中保留实际行为差异，不伪称官方已有这些保证。

| 测试边界 | 独立预期与适用用例 |
| --- | --- |
| 发现／停止 | 初始扫描、重复事件、找不到移除记录、类／窗／通知注册失败、启动即停止、交接中停止、停止后重启、回调已排队 |
| 身份／互斥 | 同容器多个接口、带／不带 instance、兼容记录、重复打开、queryData 失败、取消等待、持有期限到达而 I/O 未完成、错误 URL 释放 |
| 字符串转换 | 普通 ASCII／BMP、代理对跨调用、孤立或错配代理、错误后状态清零、NUL、空输出指针、1–4 字节容量、仅计长度、源游标与格式化失败消费 |
| 数据读写 | reportId 0/非0、截断、补零、同步／pending write、空读、超时保留 pending、OS 错误、get feature/input 的原始长度与业务应答分离 |
| Promise／事件 | 所有六个 request kind、队列前／中取消、未开始与已执行请求、传输结束后 settle 前关闭、pause、最后一个监听删除、数据回调重排 read |
| 退出／释放 | close 与在途 raw context、对象 GC、map 最后一份 owner、worker join、自线程析构拒绝、env cleanup、未 settle 的请求和恢复完成确认 |

纯／模拟测试使用官方源码与上述完整函数中的独立期望，不能用 Rust 编码器生成自己的答案。官方实际功能对照须从显式维护入口执行，核实 Node/N-API 运行时契约、来源、架构、参数、依赖和完整清理，不能用 ctypes 猜测 N-API 方法签名。先只读；可恢复写入按原值记录、交叉读回和完整恢复执行，传输结果、产品业务应答与页面消费分别记录。没有设备或可靠执行条件时，相应实际 DLL／设备结果保持“未验证”；双方未连接、模块加载成功或模拟通过均不算功能一致。
