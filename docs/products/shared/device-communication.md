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

## 共享映射 descriptor 的存储、发布与事件分类

[完整共享链审阅](../../../.analysis/results/reverse-pending/review-sources/21fe84d134926df8d0aecdcd5bbc215e298bd2a4709a9a964d7e344624b27960.json)复用当前 `mapping_engine.dll` 捕获，核对 106 个完整函数、11,395 条指令、七张虚表和三个宿主完整字段。所有保留函数块、指令及数据区均逐字节对照原 PE；查询入口为 `python -X utf8 tools/workspace.py audit-host-local --query mapping-descriptors`，正式字段为[宿主审阅](../../../.analysis/results/host-current/local-audit.json)的 `mapping_descriptors`。这些地址适用于该收据绑定的当前原件，不能按 DLL 文件名泛化到其他版本。

`localStorageSetItem(0x20920)` 拒绝空 key／value，经引擎主虚表槽 32 到 `0x2fa0 → 0x3160 → 0xd46ec → 0xd4900`，在存储服务线程更新 map。新 key 通知 added；值字节相同不通知；变化时同时携带新旧值通知 changed。源线程收到的 bool/string 回调只表示存储操作结果，没有包含 descriptor 解析、映射发布或设备应答。`0x20680` 还要求全局启用位仍置位才调用回调。宿主三个字段将 FFI 回调保留在 `cbMap`，Promise 完成后才删除；此源码没有为关停后被抑制的回调补发取消或成功结果。

存储观察者使用输入对象的主虚表。基类、USB、BLE、external、OS hook 五种构造器都将 added／changed 槽 8／16 指向 `0x65cc0`，删除槽 24 指向 `0x662b0`，清空槽 32 指向 `0x66590`。`0xb1640` 在锁内重核当前登记 token 后才分派排队通知。硬件事件使用对象 `+128` 的另一张虚表；即使共用 `0x6a470` 跳板，也不能把主表删除槽误接为硬件事件函数。

身份来自 native `0xdb630` 的物理设备字段：vendorId 为 `+32` 的 u16，productId 为 `+34` 的 u16，editionId 为 `+36` 的 u32，serialNumber 和 containerId 分别在 `+88/+112`。`0x780b0` 按以下顺序先登记再异步读取，条件为空的键不登记：

| 顺序 | 实际键 |
| --- | --- |
| 1 | 有 containerId 时 `synapse_PID_containerId` |
| 2 | 有 serialNumber 时 `synapse_PID_editionId_serialNumber` |
| 3 | `synapse_PID_editionId` |
| 4 | `synapse_PID` |

`0x555b0` 只将成功且非空的读值存入缓存；最后一个 base-key 回调排后续初始化。`0x78be0` 按缓存树顺序尝试，首个 `SetData` 成功即停止。后续 added／changed 则先更新缓存，再比较 incoming key 与当前发布 key，字节字典序较小时跳过，否则仅将非空值交给 `SetData`。不能自行改为“键越具体始终优先”。`DeviceData` 自身的 parser 不从 descriptor 的 vendorId／productId／category 决定订阅身份，前端逻辑 productId 与 native key 中的物理 PID 必须分开核对。

`0xe3f1e` 将解析结果放在 candidate `+48`，与 published `+56` 分离。version 与三个 bool 的读取失败被官方调用方忽略，保留 candidate 构造值；reportIDs 缺失或非 object 同样跳过。defaultMappings object、activeProfile string 和 profiles array 则是必要结构，其映射和 analog 分支继续调用各自 parser。这些内部解析初值不是设备读值，也不允许据此向设备写默认配置。

reportIDs 的每个 key 先解析 signed int，再截为低 uint8；每个 present value 必须是 JSON string。九项名称表的前八项对应 class 0–7，第九项 hardwareEventReportID、未知名称及没有匹配的 reportID 均为 class 8。class 0–2 另更新 candidate 的 special selector。USB 的 reportID 取自 `raw[bluetoothLE]`；分类读取的是 published 中的 report map，class 8 进入已保留的硬件事件路径。名称数量、数字或缓存内容不能单独证明设备能力和发布成功。

`SetData(0xe3c90)` 要求引擎 ready 且完整解析成功，但即使返回 true，也只有 change byte `+65` 的 bit 0 置位才加锁、更新生效 key 并把 candidate 移入 published。reportIDs 单独变化不设置该位；default/profile hash、active selection 和 analog 变化要沿实际所有者核对。解析失败保留旧 published 指针；下面的共享 Profile 副作用仍须单独处理，不能据此认定状态完全不变。删除当前生效 key 才清 DeviceData 并按剩余缓存顺序寻找首个成功值；删除其他 key 不替换当前发布值。全清空则清缓存及 DeviceData 后刷新。

[默认与配置档 hash 复用增量](../../../.analysis/results/reverse-pending/review-sources/4b228a4f2121c88570c573c8e45db32823696fe18c67b86f387df4308038b5c1.json)核对另外 18 个完整函数、2,133 条指令及 MD5 初始状态，引用基线中的七个完整 parser 所有者，不重复复制。`0xe72fa` 要求 defaultMappings 为 object，但其中 appEngine 缺失或类型不符时跳过该子解析并返回成功；存在 object 时则必须有 string hash。hash 与已发布默认映射相同会直接共享旧映射，不重验传入 payload；首次或变化时才执行 `0xe8518 → 0xe871e`，成功后设置 change byte。

`0xe75a0` 要求 profiles 为 array，逐个检查已访问元素的 object 类型与 string guid，只解析首个匹配 activeProfile 的条目，后续 inactive 条目不继续验证。无匹配不会给 candidate 的 Profile 槽赋值；匹配条目没有 appEngine object 时可接受新建的空 Profile。存在 appEngine 时要求 string hash，gamemode 读取失败则使用本次局部 false。

相同 profile hash 的行为有共享状态副作用：`0xe7e86` 直接改已发布 `Profile+152` 的 gamemode，再把同一引用放入 candidate；随后跳过 hash 校验和映射重析，也不设置 descriptor change byte。该修改发生在后续 descriptor 校验之前，因此后续失败也不能视为回滚。不同 hash 则先校验，再给新 Profile 赋 hash／gamemode，并解析 profile 专有的可选 actuations／rapidTriggers／snaptaps 和必需 mappings；只有成功才置 change byte。此处是本地解析与引用状态，不是设备写入或读回。

[gamemode 实际消费者增量](../../../.analysis/results/reverse-pending/review-sources/0a7c96095dbcbd6eddbeffc3679f1b68088ad9ac77f2632d4d3b91628f813c54.json)另核五个完整函数、560 条指令。有效 getter `0xe5a36` 读取 published descriptor 的独立 byte `+140`，它由 parser 从 Profile `+152` 复制；因此相同 hash 时只改共享 Profile，不会同步更新当前生效的 gamemode 缓存。`0x8eb70` 比较该 getter 与自身 byte `+800`，先更新本地观察标记和原子字节，再经 `0xc438a → 0xc45b0 → 0xc0e76` 排设备模式服务通知。服务找不到 identity 时只日志，不创建设备；命中时修改登记记录的 byte `+48` 并通知观察者，相同状态的检查只影响日志。这些服务状态与通知仍不代表硬件写回，后续接收者和实际产品消费者继续独立核对。

`0xe8518` 复制 appEngine object，去掉 hash 与 gamemode，经原生 JSON writer 序列化，再由 `0x1b3a30 → 0x1f3600/0x1f3710/0x251cc0` 计算 MD5；16 字节摘要经 hex 编码与 ASCII 小写转换后逐字节比较传入 hash，传入字符串没有另做大小写归一化。相同存储 hash 分支完全绕过此校验。JSON writer 的精确顺序、数字与字符串编码，以及所有 input／output 类型分支仍需继续闭合，不能假定任意 JSON 序列化库等价。

[观察者生命周期增量](../../../.analysis/results/reverse-pending/review-sources/e494e365791a35f4cd40e077f25dfab10b8f1e105c6b2288cb9654ed8cca5f15.json)另核对 19 个完整函数和 1,236 条指令，不复制上述 106 个函数。USB、BLE、external 与 OS hook 析构均到 `0x656ca`；它先恢复 input 基类虚表，再向设备线程排 `0x65c90`。此时该调用读取主表槽 72，实际目标为 `0x77f00` 空操作，不能解释为派生设备恢复或写入确认。

`0x656ca → 0x1b8670` 先请求所属线程停止，经 `QuitWhenIdle` 结束循环，有线程句柄时无限等待并关闭，随后才释放所属字段。最终 `0x547e0` 尾跳 `0xd3c70 → 0xd3d1e`，在服务锁下快照各 key 的观察者列表，再由 `0xb0ab0 → 0xb1ebc` 逐列表移除此 identity；先前排队的通知还会被 `0xb1640` 的登记 token 检查拒绝。注销本身没有独立的在途回调排空应答。服务析构 `0xd3ef2` 核对调用线程、清全局服务指针、停止并等待存储线程，再销毁观察者和字符串容器；全局已清后才析构的观察者跳过服务移除。

本节闭合存储通知、身份、报告分类、发布门禁及上述观察者和所属线程的边界。每个产品实际 publisher／合并结果、全部 mapping parser 与执行器、各引擎／设备关停发起方、外部生产者及逐请求取消／恢复仍需接回完整调用方；Rust 独立实现、UI／后台接线、官方实际功能与真实设备对照保持各自状态。

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

## 键盘驱动中的 WPP 日志包装边界

[46 份驱动的完整函数复核](../../../.analysis/results/native-bulk/wpp-wrapper-verification.json)覆盖两个完整的 49 指令模板，共 2,254 条指令、9,344 字节函数块。16 个线程逐份重新核对原件、capture、完整函数摘要、所有 chunk 和逐条指令字节，并核实 `WppAutoLogTrace` 来自 `WppRecorder` 导入，46 项均通过。这里复用已有 IDA 9.1 捕获，不运行驱动；Hex-Rays 失败记录保持原样，不把汇编已解释记成反编译已恢复。具体产品、版本、路径及 RVA 逐项保留在收据中。

例如 [RzDev_0266 的 RVA 0x6024](../../../.analysis/results/native/00ee8da220c01074b72b19ddee1297993999bc303d9ada0627b593f02c198517/functions/6024.json)，Image Base 为 `0x140000000`。函数从 RCX 保存首参数，从 DL 取 8 位等级，从 R8D 保存 32 位 token，从 R9W 取 16 位消息编号，第五个栈参数作为消息指针继续转发。令 `t` 为原 R8D、`k=t>>16`、`q=((t-1)>>5)&0x7ff`、`b=(t-1)&31`，减法按 32 位无符号规则执行。它在全局控制区的 `80*k+44+4*q` 读取位图，在 `80*k+41` 读取等级阈值。仅当指定位为 1 且阈值不低于 DL 时，经 CFG 间接调用日志 sink；RCX 为控制区 `80*k+24` 的值，EDX 固定 `0x2b`，R8 为第五个入参，R9D 为消息编号，首个栈实参为零。两个模板只有指令调度与清零写法不同，不能误记为一版丢弃消息指针。

无论该条件分支是否执行，函数随后均调用 `WppAutoLogTrace`：依次传递原首参数、8 位等级、原 token、第五个入参、16 位消息编号及零。IDA 将控制区全局命名为 `DeviceObject`，该名字本身不证明它是当前连接设备对象；本函数中的位图和等级过滤也不能用来推断设备能力、连接状态或某次 HID 写入成功。条件 sink 的实际目标、全部调用方和产品 I/O 链仍需各自追踪，这 46 处辅助函数不能充当 46 项设备功能完成。

此处独立测试预期是 token 高低位的分解、`t=0` 的回绕、bit 0/31、等级阈值等于／低于输入等级、16 位消息编号截断，以及条件 sink 被跳过时仍进入 AutoLog 的调用顺序。未执行官方驱动、DLL、Rust 行为测试或真实设备对照，不据原字节一致性提升这些验收状态。

## 三架构驱动的安全设备创建与注册表顺序

[完整静态审阅](../../../.analysis/results/reverse-pending/review-sources/ce3a65e7b4e221f2f360278c40bdc026361416c5f9d0e75624aabf01a05bed0b.json)保留 x86、x64、IA64 的 35 项 IOCTL 分支及安全设备创建链；[统一接线收据](../../../.analysis/results/native-bulk/camera-0e06-identity-verification.json)另行记录 110 个完整辅助函数、6,698 条指令的独立原 PE 字节复核。缓存的 `IoCreateDeviceSecure` 存在时直接转发九个原参数；缺失时才进入内置兼容路径。IA64 使用函数描述符，不能把描述符地址当代码入口。`IoValidateDeviceIoControlAccess` 仅在初始化中查找并缓存，当前完整派发器没有调用它，不能据此推导每项 IOCTL 已校验访问或输入长度。

兼容路径先清空输出设备指针，按 ClassGuid 读取或创建类设置，再在缺少可用 Security 时解析 `D:P(A;;GA;;;SY)(A;;GA;;;BA)`。所需的注册表写入先于 `IoCreateDevice`；Class、NoDisplayClass、NoUseClass 和 Properties\\Security 的先前成功写入不会因后续失败回滚。创建成功后才根据安全描述符计算访问掩码、打开实际设备、设置安全性并关闭临时句柄；安全设置失败会删除刚创建的设备，只有完整成功才发布输出指针。

设置结构分别保留字段值与 presence bits。加载默认安全描述符执行 `flags = 2`，会清除 DeviceType、Characteristics 和 Exclusive 的存在位，因此该分支即使仍留有注册表数字，也按入口参数创建。默认 SDDL 解析又设置 DACL-defaulted；后续读取会明确丢弃这种描述符，再走默认 SDDL 路径。这些内核安全配置的缺值语义不适用于产品设备设置的失败回退或默认值写回。

三架构还有同一条条件释放缺口：Security 捕获出非空分配后，安全信息验证在所有权转入 settings 前失败，退出链只释放仍为空的 settings 指针。收据保留精确分支和地址；独立 Rust 实现须按项目所有权约束释放资源，同时保留该原版行为差异。当前只完成静态字节和上述语义核对，未执行驱动、安装、设备写入或实际功能对照。
