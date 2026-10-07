# 179 接收器的当前发现与查询契约（2026-10-07）

本批继续此前中断的启动读取调查。已核实当前 179 middleware 的实际查询、HID 封包、返回解析及主从身份映射；**没有把这条链接入 Rust worker 或首页，没有运行查询**。原有启动错误修复见 [启动复核](runtime-startup-verification-2026-10-07.md)，其“自动设备发现尚未实现”的结论仍然有效。

## 可重复的当前取证

`node tools/audit-receiver-discovery.cjs --check` 核验 92 份 manifest 声明的 JS 字节、下载收据和静态语法，保存 34 项 AST 收据。源文件、SHA-256、UTF-16 半开区间及原文见 [协议证据](receiver-discovery-current-evidence.json)。独立子任务另核对 [发布器与目录](receiver-publishing-readonly-review.md)，28 项 AST 和两份官方 JSON。

新增 `tools/middleware-source.cjs` 只静态解析 webpack getter、直接导出和声明，不 require/import/eval 任何厂商模块。它支持本批 middleware 的 function getter 形态。各 lazy chunk 存在相同模块号的不同版本，因此不能把全部文件的声明无条件合并。本批先从 main 的 `Promise.all([2520,6120,4551])` 入口核实 6120，再按实际模块依赖读取；92 份文件全量语法核验与 572 个已解析作用域分开计数。

关键文件：

| 文件 | SHA-256 |
| --- | --- |
| 179 `main.6d1e356030ef563555d4.js` | `b836abc03ea2b5eaec54e8bdfc48d06d5a1bab0bcd0b38499fa1a3c97ecae429` |
| 179 `6120.ef5d145fafdfd0069d53.js` | `1bc185f4b6407b0c5aab6939b827a9f3ccba88e21966c222607bc0e92742beb8` |

这些 MW 字节的已有 HTTP 收据记录 2026-10-07 抓取、2026-10-05 Last-Modified。它与 10-02 已审 UI/host 的版本身份分别记录，不把旧 MW 的符号位置沿用到本批。

## 查询链与传输

1. 当前 host `usb.getDevices` 调用 `rz-usb-detect.find()`。普通 Windows HID collection 列表不是该物理设备列表，也不是接收器配对列表。
2. 179 的 DeviceInfo 是 vendorId 5426、productId/dongleId 179、claimInterface 0。96204/j 的 LINKER 分支专门选择 `rzDevice25LinkerUma`，该类继承 Linker。主从查询目标必须携带真实容器身份。
3. 34340/he 调用 `getMultipleDeviceWirelessConnectionStatusV2()`；7755/dc 转发 84816/VO（当前绑定 J）；其实际命令来自 30580/IZ（ue）：`[80, 0, 191]`，分别为声明的数据长度、命令类和命令 ID。
4. `_createDataSend` 构造 90 字节包：状态 0、Linker 的事务 ID `224 | transactionId++`（到 31 前回零）、三个保留字节、数据长度 80、类 0、ID 191；第 88 字节为第 2 至 87 字节的 XOR。查询没有设置配置的输入数据。
5. 当前 Windows host 在 90 字节包前加 report ID/claimInterface，调用 HID feature report，随后读取 91 字节并去掉首字节再返回 MW。host 在 send/get 之间维护设备 mutex、最小间隔和身份匹配；不能将协议函数硬塞进现有 `Query(callback3)` 的 DLL ABI。
6. `_getUSBTransferInResult` 核对事务 ID、命令类、命令 ID，按返回头状态区分 new/busy/success/failed/timeout/not-supported/unknown。成功码为 2，busy 为 1；重发与重读有独立条件。仅提取成功 payload 之前必须保持这些错误边界。
7. 84816/J 从 payload 第一个字节读设备数，后面每项三字节为 `status, pidHigh, pidLow`，PID 用 `(high & 255) << 8 | (low & 255)`。原 JS 缺少完整长度校验，后续 Rust 解码仍需拒绝截断数据，不能将缺失字节变成成功空列表。

基础时间常量为 5ms，179 构造器传入五倍间隔；maxRetryIn 10、maxRetryOut 20。它们是源码参数，不表示本批执行过等待、打开设备或收发报文。

## 身份与在线状态

新抓取的官方 `AvailableDevices.json` 明确给出 `productId:182,dongleId:183`，`DualDongleCompatibleDevices.json` 的 183 条目有 `DeviceSiblings:[182]`，179 的支持列表包含 183。**只有实际查询返回 183 时，才能按这些条目建立 182 从设备身份；目录本身不是连接结果。**

34340/he 排除 PID 65535 和自身 dongleId；Ne 严格用 `status===1` 判定在线，0 为离线。这与硬件通知中的 `eventValue.state===3` 是两个字段，不能混用。182 的 master 指向 179，沿用真实接收器 containerId，同时保留 dongleId 183。固件、序列号、profile、ready 等仍需各自的真实观察，不能从这条连接查询补造。

## 不能直接照搬为只读的部分

- Uma 的 `getDeviceMode()` 返回本地 `isDriverMode` 的模拟对象，`setDeviceMode()` 也只改该对象；不能把这些方法名称或 SUCCESS 字段当成真实设备读取证据。
- 原 `ye()` 发布器会写 `duallink-devices`、注册 runtime 监听，并可能进入映射合并/写回。当前后置写操作阶段不允许复用其整条初始化链。
- `ye(..., false)` 不新增持久化，但也不删除已有缓存。因此存储里保留的关联不能代表本次在线；新的原生观察模型必须独立于该历史缓存。
- host 合成 USB 接入事件后还会继续初始化与发布。静态核实这段链不代表可以将它完整执行作只读枚举。

下一步实现应拆开真实枚举身份、179 只读查询传输、严格解码、目录映射、会话/连接代际和 UI 观察消费；失败、离线和未读取分别保留。禁止以预览目录替代设备列表，也不能先发写任务来制造读取成功。

本批协议/发布器工作仅包含静态取证和维护工具；真实只读接线、热插拔、窗口视觉与最终产品复刻仍未完成。
