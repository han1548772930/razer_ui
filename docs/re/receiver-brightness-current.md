# 当前 164 / 241 接收器亮度提交

本条链依据当前产品 middleware 的实际注册、TaskRunner、设备方法和命令构造实现，不调用原 DLL。完整源码正文、SHA-256、UTF-16 偏移和获取收据见 [证据 JSON](receiver-brightness-current-evidence.json)。[维护审计工具](../../tools/audit-receiver-brightness-current.cjs)只解析原 JavaScript 的 AST，不执行厂商代码。

## 原代码链路

两产品实际启动注册均为 `rzDevice25`，亮度区域 `15`。164 使用 `LINKER`，241 使用 `LINKERMULTIDEVICES`。正常 `taskRunnerSetBrightnessToDevice` 选择 **profile 1**，先读取 `getBrightness`；百分比相同时跳过 setter。未注册的 eGPU、IoT、显示器、Philips Hue 分支不用于这两个接收器。

`getBrightness` 经 module 7755 → 13953 `Fm` → 78196 `_9 = [3,15,132]`；`setBrightness` 经 module 7755 → 13953 `n4` → 78196 `ne = [3,15,4]`。payload 为 `[profile,region,brightness]`，读请求末字节为零。写入值使用 `floor(percent/100*255)`，真实响应三字节按 `ceil(raw/255*100)`还原。

这是主设备 Linker 的 **E0 | counter**、模 31 事务链。241 子设备配对使用的 mouse / keyboard lane 事务号不适用于这里。

## Rust 与调用连接

当前 `_getUSBTransferInResult` 在每次 OUT 后先把 `resendOutCommand` 置为 false；busy 响应置为 true，随后 IN 异常的 catch 保留该值，因此仍会重发 OUT。没有先出现 busy 的 IN 异常以及发送异常终止该命令。这与“一遇到异常就停止”不同；亮度、序列号和配对命令已按同一源分支实现。164 的精确源节点为 module 7755、UTF-16 `[780039,783082)`；主 Linker `sendCommand` 为 `[4118,7021)`，完整正文与哈希见证据 JSON。

亮度响应 parser 只提取返回的 profile、region 和 raw，不比较前两者与请求值。Rust 保留这些真实返回值；payload 的 slice 按实际返回字节数截取。完整 header、规范化 transport 的 Report ID 和可构造 Rust `Brightness` 所需的三个字节是类型/边界要求，不能称作原版额外的成功确认条件。

[共享协议](../../crates/razer-device/src/receiver_brightness.rs)负责原 pre-read、条件写入及 setter 响应；原 TaskRunner 没有 setter 后追加 getter，也不比较新的 getter 值。页面单独的读取请求与写入任务分开。[Windows 适配](../../crates/razer-service/src/runtime/windows/receiver_brightness.rs)隔离实际 HID transport 和异步 owner。共享协议不依赖 Windows；其他平台若没有对应设备适配，返回明确不支持。

IPC `ReceiverBrightnessRead` / `ReceiverBrightnessWrite` 指定保留的 command path 与 Container。提交返回实际 `previous`、可选 `acknowledged` / `observed`、`write_attempted`、`write_sent`、`source_completed` 与 `error`。`source_completed` 仅指原 pre-read/conditional setter 链完成，不表示另一次硬件回读。相同百分比时 `observed` 为原pre-read；实际调用setter后仅保存setter响应，不伪装成getter观察。原TaskRunner完成后发布requested brightness的host/memory链仍是缺口。失败不能解释为设备没收到写入，也不能伪造回滚。

Help Reset 由原默认 profile 的 brightness enabled / value 选择提交值；本地配置保存、亮度提交、后续效果与映射刷新分别记录结果。接收器亮度已确认不等于整个设备重置已完成。

## 未完成项与验证边界

原 TaskRunner 的 host 亮度发布、memory storage 刷新，以及其他产品与特殊设备分支仍是缺口。本文不将这些标为完成。

验证使用静态当前源码审计与 `cargo check --locked --all-targets`。没有运行应用、DLL、厂商 JavaScript 或真实设备操作；运行时验收尚未进行。

2026-10-10 执行 `cargo test --locked -p razer-device --lib receiver_`：22 项纯 Rust/模拟 transport 测试通过，覆盖 busy 后 IN 异常、无 busy 的异常、下一 OUT 重置标志、发送异常、不追加 getter、不比较 setter 返回值、原样保留 profile/region、ASCII 序列号空结果和实际长度截取。存储调用者单独要求实际非空序列号，getter 不伪造读取失败。已去掉没有原链依据的亮度/序列号固定十秒总超时，保留源重试次数和 Rust 请求的实际 owner/cancellation 边界。测试没有访问设备、加载 DLL 或创建 GUI，不能代替真实设备或像素验收。

## 本地 profile 发布与原 host 缓存边界

原 `taskMakerSetBrightness` 在排入设备任务前通过 `20236.lY` 摘取 `productId`、`activeProfile`、`profiles{name,guid}` 并写入 WindowStorage 的 `connectedDevices`；`20236.I4` 合并 runtime 字段，只在序列化结果改变时再发布。当前 Rust worker 在真实序列号拥有的 source document 成功本地持久化后，通过独立 `LocalProfile` 消息更新当前 workspace 的 profile，再提交设备命令。此时设备请求仍 pending；设备失败和 cleanup 错误不会抹去已发布的本地状态。

这是实际页面本地状态链的顺序修正，不能等同于原 host WindowStorage/MemoryStorage 事件已完成。原 `Vb7` 成功后另更新请求值、时间、字符串版本与 `usb_VID_PID` 键；原 middleware webContents URL、接收者订阅、全局 memory cache 及清理链仍须连接，不能用短期 worker 的 Map 或猜测 URL 代替。新增精确 caller/helper 正文见 [页面证据](receiver-brightness-page-current-evidence.json)。
