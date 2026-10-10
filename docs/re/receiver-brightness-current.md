# 当前 164 / 241 接收器亮度提交

本条链依据当前产品 middleware 的实际注册、TaskRunner、设备方法和命令构造实现，不调用原 DLL。完整源码正文、SHA-256、UTF-16 偏移和获取收据见 [证据 JSON](receiver-brightness-current-evidence.json)。[维护审计工具](../../tools/audit-receiver-brightness-current.cjs)只解析原 JavaScript 的 AST，不执行厂商代码。

## 原代码链路

两产品实际启动注册均为 `rzDevice25`，亮度区域 `15`。164 使用 `LINKER`，241 使用 `LINKERMULTIDEVICES`。正常 `taskRunnerSetBrightnessToDevice` 选择 **profile 1**，先读取 `getBrightness`；百分比相同时跳过 setter。未注册的 eGPU、IoT、显示器、Philips Hue 分支不用于这两个接收器。

`getBrightness` 经 module 7755 → 13953 `Fm` → 78196 `_9 = [3,15,132]`；`setBrightness` 经 module 7755 → 13953 `n4` → 78196 `ne = [3,15,4]`。payload 为 `[profile,region,brightness]`，读请求末字节为零。写入值使用 `floor(percent/100*255)`，真实响应三字节按 `ceil(raw/255*100)`还原。

这是主设备 Linker 的 **E0 | counter**、模 31 事务链。241 子设备配对使用的 mouse / keyboard lane 事务号不适用于这里。

## Rust 与调用连接

[共享协议](../../crates/razer-device/src/receiver_brightness.rs)负责原 pre-read、条件写入及 setter 响应；原 TaskRunner 没有 setter 后追加 getter，也不比较新的 getter 值。页面单独的读取请求与写入任务分开。[Windows 适配](../../crates/razer-service/src/runtime/windows/receiver_brightness.rs)隔离实际 HID transport 和异步 owner。共享协议不依赖 Windows；其他平台若没有对应设备适配，返回明确不支持。

IPC `ReceiverBrightnessRead` / `ReceiverBrightnessWrite` 指定保留的 command path 与 Container。提交返回实际 `previous`、可选 `acknowledged` / `observed`、`write_attempted`、`write_sent`、`source_completed` 与 `error`。`source_completed` 仅指原 pre-read/conditional setter 链完成，不表示另一次硬件回读。相同百分比时 `observed` 为原pre-read；实际调用setter后仅保存setter响应，不伪装成getter观察。原TaskRunner完成后发布requested brightness的host/memory链仍是缺口。失败不能解释为设备没收到写入，也不能伪造回滚。

Help Reset 由原默认 profile 的 brightness enabled / value 选择提交值；本地配置保存、亮度提交、后续效果与映射刷新分别记录结果。接收器亮度已确认不等于整个设备重置已完成。

## 未完成项与验证边界

原 TaskRunner 的 host 亮度发布、memory storage 刷新，以及其他产品与特殊设备分支仍是缺口。本文不将这些标为完成。

验证使用静态当前源码审计与 `cargo check --locked --all-targets`。没有运行应用、DLL、厂商 JavaScript 或真实设备操作；运行时验收尚未进行。
