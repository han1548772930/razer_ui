# 1352 Leviathan V2 Pro 音量页面与系统端点链路

当前源逐字节依据在 [UI 专项证据](leviathan-volume-ui-current-evidence.json)；Windows 原生实现的 IDA/Hex-Rays、RVA、回调与 Core Audio 依据在 [原生音量证据](simple-audio-volume-current-evidence.json)。本专项只说明已核实的 1352 音量链路，不代表整个声音页或其他产品完成。

## 当前原始调用

1352 middleware factory `35924` 启用 `audioProtocol.simpleService`，FeatureManager 将其合并到实际实例。当前产品第一页挂 `LG`，`LG` 挂 connected `jU`，对应 `XU` 音量组件；组件接 volumeReducer，范围为 0–100，step 为 1。`changeValue` 设置 `{isEnabled:!!value,value}`，因此 0 静音、非零解除静音；toggle 只翻转 enabled，保留 value。实际 Range 指针拖动先更新组件内部状态，释放提交。源 Range 的键盘分支可立即提交，当前 gpui-kit Slider 尚未实现这一分支。

volumeReducer 根据 enabled 是否变化发出 `ON_CHANGE_VOLUME_ENABLED` 或 `ON_CHANGE_VOLUME_VALUE`，middleware 状态机调用对应任务，任务经 audioProtocol 的 simpleService 分支执行 `simpleSetSpeakerVolume(endpointId,!isEnabled,value)`。原任务不会依据 setter 回调的布尔值判断设备确认；当前 Rust 会明确显示失败、部分更新及回读差异。

初始 `getSpeakerVolume` 返回成功时，把 `muted`、`volume` 转换成 `{isEnabled:!muted,value:volume}`，发布 MW 更新；默认 `maxTry=8`、`delayInMs=2000`。这一音量初始化读取与键盘保存配置触发 setter 的初始化不是同一行为。

## 系统 endpoint 身份

`getAudioDeviceId` 从 `simpleEnumerateAudioDevices` 的真实列表筛选 `type === "speaker"`，保留源返回顺序，再取首个 `containerId === rzDevice.containerId`。1352 没有启用 `useVirtualAudioChannel`，不能额外套用 Headphone 名称过滤。最终传递的是该记录的 `.id`，不是 USB ID、ContainerId 或 HID collection key。

原代码容器不匹配时，再按 `name.includes(rzDevice.device.productName)` 取首项。当前模型的 `Device.product_name` 来自目录静态名称，尚没有这个原始发现字段的可信会话来源，因此 Rust 容器匹配失败会明确报错，未将目录名冒充硬件观察。此回退仍是缺口。

## Rust 页面与提交

保留的页面 controller 位于 `crates/razer-pages/src/features/audio_volume.rs`，由 AudioProductWorkspace 持有；Source/Product workspace 转发带 generation 的 typed intent，`crates/razer-shell/src/shell/audio_volume.rs` 在后台读取真实 AudioDevices、解析 endpoint，再发 AudioVolumeRead/Write IPC。适配器隔离在 service，页面与公共状态逻辑不直接调用 Windows 或 DLL。

只有真实连接的 1352 声音页激活时开始读。成功读更新独立的可见音量和观察状态，不改本地配置快照，不触发保存事件；仅用户 slider Release/toggle 更新本地草稿并请求真实写入。默认 50/false 和本地 restore 均不会自动写设备。

请求共用保留的 worker 槽，epoch 和 edit revision 排除关闭页面、连接变化、恢复配置及后续编辑产生的旧结果。新用户请求取消旧的尚未提交工作，并排队保留最新意图；已经提交的物理操作仍等待完成，没有伪装撤销。应用后台队列串行，service 另持有实际 endpoint 的进程间锁。

回执校验系统 endpoint ID、音量范围、写前成功状态、HRESULT 与 source result、请求值与提交标志的关系。写操作的原响应、部分修改、后续回读和通信进程退出分别处理；退出失败保留已经取得的真实回执。操作后 endpoint 发生变化会说明设备可能已更新、当前状态未确认。

## 未完成与验证边界

- 缺少真实 productName 来源的名称回退；原缓存、音频事件注册和外部音量变化通知尚未全部连接。
- 原始完整初始化重试尚未等价：false getter 记录会重试九次；初始 endpoint 缺失及适配器错误当前明确失败。
- 原共享任务 memory/version、全局锁状态机及 Range 键盘分支尚未全部替代。
- 1352 其余声音组件仍需分别按当前源完成；本专项不扩大其他产品的能力。

维护工具 `python tools/audit-leviathan-volume-ui-current.py` 重新核实当前源哈希、UTF-16 范围和消费者字节，`--check` 检查证据一致性。开发中未运行应用、DLL、项目测试或设备通信；静态检查不等于运行验收。
