# 当前 RzAudioUtil 端点通知

依据当前 `RzAudioUtil_v1.0.3.1.dll`，SHA-256 `9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf`。新分析使用安装的 IDA Pro 8.3 / Hex-Rays，在 `.work/ida-static/audio-util-notification/` 私有副本上静态执行；没有运行导出、调试器、应用或系统通知注册。完整函数边界、原字节哈希、伪代码和交叉引用保留在 [IDA 正文](evidence/audio-util-notification-ida.json)，摘要及八产品当前 JS 收据在 [验证依据](audio-util-notifications-current-evidence.json)。

`dispatchAudioEnumerator` 的 EnableNotification 表在 RVA `0x99450`，调用槽 `+0x10 → 0x6fc0 → 0x2bf0`。读取 `cmdData.enable`；缺参数返回 `0x80070057` 和原错误字符串。原数值转换接受 bool 和数字，当前八产品 wrapper 传入 bool。返回的 `response.enabled` 是整数；Rust typed IPC 只暴露当前 wrapper 使用的 bool 输入。

每次 enable 向 callback vector 添加同类型 callback，只有 vector 从空变为一项时才调用 `0x44b00` 注册 Core Audio。因此重复 enable 原本会重复发送同一个事件。disable `0x3950` 通过 callback 类型比较移除所有匹配项，vector 为空时调用 `0x44c00` 注销。注册器 `CoCreateInstance` 用 CLSCTX `0x17`；IMMDeviceEnumerator `+0x30` Register、`+0x38` Unregister。原 dispatcher 忽略注册结果并填 enabled；Rust 返回真实注册/注销错误，避免把系统失败标为成功。注销失败保留 listener owner，使后续 disable/drop 能重试。

IMMNotificationClient 的五种通知转为原内部编号：Removed=0、Added=1、Default=2、State=3、Property=4。`CRzAudioEnumerator` 的 `0x3bd0` 仅转发 `0/1/3`；default/property 到此被过滤。Added/Removed/State helper 要求非空 endpoint ID。State 数值、Default flow/role、Property key 和通知种类均未进入原事件字段。

`0x3360 → 0x1abb0 → SetNodeFFIEvent` 构造并提交：

```json
{"event":"RzAudioUtilEvent","eventType":"AudioEnumerator_DeviceChange","endpointId":"实际端点 ID"}
```

当前产品 1398、1422、1427、1446、2638、2641、4124、4126 初始化均 enable。通用 JS wrapper 根据原 `eventType` 原样 `emit`；产品监听的是 `AudioEnumerator_deviceChange`，字母 d 大小写与原二进制不一致，而且 handler 仅 `console.log`。没有源依据表明此链更新缓存、刷新设备列表或改变页面状态；Rust 保留原 payload，未虚构自动刷新。

[共享队列](../../crates/razer-device/src/audio_notification.rs) 保留原过滤、事件字段、重复订阅和整类取消语义。[服务生命周期](../../crates/razer-service/src/audio_notification.rs) 与 [Windows 适配器](../../crates/razer-service/src/platform/windows/audio_notification.rs) 分离；后者直接实现 Core Audio callback ABI、QueryInterface、引用计数、注册/注销和 COM apartment 生命周期，无厂商 DLL。首次 enable 按原码先加入 callback 再 register，避免注册期间的真实通知漏掉订阅；注册失败清理本次订阅并返回错误。真实 OS callback 入队，worker 的 `AudioNotificationsDrain` 取得已观察事件；`AudioNotificationsEnable` 控制保留 listener。其他平台明确 unsupported。空事件数组表示没有排队事件，不代表读取过设备成功。

Shell 的[产品监听生命周期](../../crates/razer-shell/src/shell/audio_notifications.rs) 已连接真实 IPC：仅为八个当前源码核实的产品、唯一实际设备观察启动独立保留 worker，每个实例只 enable 一次；重复 discovery 不重复注册。保留 worker 持续 drain 实际 OS 通知，校验完整原事件字段并保留事件大小写，仅记录诊断，不刷新产品缓存或页面。1401 使用独立 1.0.1.1 资源，未接入本通知能力。缺失连接或接收器 peer 未确认会释放当前 owner；线程收到停止信号后 disable 并 Shutdown，应用退出先向所有 owner 发停止信号，在后台等待并发清理。IPC 请求保持现有有限超时，注销/读取/退出错误保留日志，owner 仍存在时通过现有通知显示，未把失败记为成功。

源码取得、29 个函数语义、Rust 队列、Windows 适配、IPC 路由及 Shell 生命周期分别保留；`cargo check --locked -p razer-service -p razer-ipc --all-targets` 和 `cargo check --locked -p razer-pages -p razer-shell --all-targets` 已通过。未运行通知注册、测试或应用，运行验收未完成。单项不能证明整个 RzAudioUtil 完成，AudioRouter、MediaPlayer 和独立 1.0.1.1 MediaRecorder 仍需继续。
