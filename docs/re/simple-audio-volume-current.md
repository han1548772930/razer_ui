# 当前 simple_service 音量和静音读取、写回

当前官方 host 4.0.827 的 `simple_service.dll`（SHA-256 `f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b`）中，speaker 和 microphone 的 getter/setter 最终进入同一套按音频端点 ID 查找、读取和修改的实现。[36 个 IDA/Hex-Rays 函数正文](simple-audio-volume-current-evidence.json) 保存原字节哈希、函数边界、交叉引用、指令和伪代码；新增分析仅使用私有副本和静态数据库，没有加载 DLL、调用导出、运行调试器或系统音频操作。

## 当前真实产品、页面和参数

产品 1352 Razer Leviathan V2 Pro 的当前 middleware `main.6debfb04ffc1be201fd1.js` 中，module 35924 明确导出 `audioProtocol.simpleService.registerAudioServiceEvent=true`；module 47973 将该特征对象实际合并到 FeatureManager。这是产品 gate 的直接证据，其他产品不能仅因共享 bundle 包含相同方法就算启用。

当前 UI `main.95a4f703.js` 第一导航项挂载 `LG`，其左列挂载连接 `volumeReducer.volume` 的 `jU/XU`。滑块为 0..100、step=1，切换静音保留数字；滑块提交使用 `{isEnabled:!!value,value}`，因此非零值解除静音，0 同时静音。原 `VU=yU(bU)` 在拖动中更新局部状态，window mouseup 提交；这个挂载未传 every-step 或 debounce 选项。Redux 分别生成 `ON_CHANGE_VOLUME_ENABLED` / `ON_CHANGE_VOLUME_VALUE`，middleware 事件任务调用音频接口；simpleService 分支传入 `mute=!isEnabled` 和数字值。读取只在 `result=true` 时转换成 `{isEnabled:!muted,value:volume}` 更新页面。

`getAudioDeviceId` 首先解析 `simpleEnumerateAudioDevices` 数组，精确按 `type=speaker/microphone` 过滤。普通分支取第一个 containerId 与当前 rzDevice.containerId 相等的记录；启用 useVirtualAudioChannel 的分支还要求名称含 `Headphone` 或 `Microphone`。两者未命中时，再按 `name.includes(rzDevice.device.productName)` 取第一项。最后保存该记录真实 `.id`，没有用 USB、HID、产品 ID 或名称拼音频端点 ID。1352 factory 没有启用 useVirtualAudioChannel；该共享分支与缓存/重新订阅机制仍分别保留在收据中。本项目 discovery 当前为页面构造的静态产品名称不等同于这个原始设备观察字段，不能直接拿来证明 name fallback 已完全复现。

原 middleware setter 任务等待方法返回后，没有检查其布尔结果就标记 completed、更新任务缓存。这是原程序的行为缺陷，不能将它解释为设备必定成功。Rust 保留实际 setter 的结果和回读失败，二者分别呈现。

## 导出、任务和真正系统接口

| 功能 | 导出 RVA | singleton 虚表 slot → 方法 | AudioServiceWin slot → 最终实现 |
| --- | --- | --- | --- |
| GetMicrophoneVolume | `0x179f0` | `+0xc8 → 0x5550` | `+0x48 → 0x3a850 → 0x3a856` |
| SetMicrophoneVolume | `0x17dc0` | `+0xd0 → 0x57f0` | `+0x50 → 0x3a8f0 → 0x3a8f6` |
| GetSpeakerVolume | `0x19750` | `+0x100 → 0x67c0` | `+0x80 → 0x3a850 → 0x3a856` |
| SetSpeakerVolume | `0x19b20` | `+0x108 → 0x6a60` | `+0x88 → 0x3a8f0 → 0x3a8f6` |

虚表依据 singleton 表 `0x1f2040` 和已确认 AudioServiceWin 表 `0x1f54d0` 的实际指针字节。导出先创建拥有回调的 closure，跨服务线程、audio thread 执行，缺线程或实例时返回 false/reason，最终完成后释放引用。当前 Windows FFI getter 回调为 `bool,string,string,bool,uint8`，wrapper 输出 `{result,reason,deviceId,muted,volume}`；setter 为 `bool,string`，输出 `{result,reason}`。回调-map 项在 Promise 完成后删除。完整线程生命周期仍不是这四个函数的实现完成范围。

`0x3a856` 从原哈希表按 UTF-8 ID 等长、逐字节比较查找（`0x3b6a6/0x3bd26`），读取端点缓存 byte101 muted、byte100 volume；缺 ID 返回 `Error : Cannot find audio device.`。初始化/通知更新这些字段：scalar 经 `0x37a56` 的 f32 乘 100，再转换成 f64 加 0.5、截断为整数低字节，不能改成 f32 加 0.5 或不同 rounding。通知 `0x37ad0` 同时更新 muted/volume 后按 flow 分发事件。

`0x3a8f6 → 0x37b34` 是实际写回：与缓存不同才先调用 `IAudioEndpointVolume::SetMasterVolumeLevelScalar(uint8/100.0f,NULL)`（vtable +0x38），负 HRESULT 立即失败；成功更新 volume 缓存，然后仅在 muted 不同时调用 `SetMute(bool,NULL)`（+0x70）。静音失败保留已经成功的音量修改，不撤销、不回滚。原 u8 参数没有 100 上限 clamp；Core Audio 可以拒绝超过 1.0 的 scalar。缺端点音量接口返回 E_FAIL，系统写失败由上层转换为 `Error: Failed to set volume.`。

literal endpoint ID 的系统获取已追到 `0x3e9e4`：MMDeviceEnumerator `GetDevice` slot +0x28，成功后 `GetState` +0x30，检查 active bit 1，失效释放接口。音量激活 `0x3fb86` 使用 IID `5cdf2c82-841e-4546-9722-0cf74078229a`、CLSCTX=0x17；getter scalar +0x48、mute +0x78。IID 与比例/rounding 常量均由原 PE 实际字节核验。原 CreateDevice 另有 default/loopback 特殊 ID 分支，此次明确 endpoint-ID API 不冒充已实现这些分支。

## Rust、连接与资源释放

[共享数据和换算](../../crates/razer-device/src/simple_audio_volume.rs) 不依赖 OS；[公共服务入口](../../crates/razer-service/src/simple_audio_volume.rs) 在 Windows 分发至 [Core Audio 实现](../../crates/razer-service/src/runtime/windows/audio_volume.rs)，其他系统明确 unsupported。`AudioVolumeRead {device_id}` 和 `AudioVolumeWrite {device_id,mute,volume:u8}` 已接 portable worker。读取保留原 getter 字段；写入把原回调结果放在 `source_response`，同时返回真实 previous、可选 observation、observation_error、两项提交标志及 HRESULT。没有加载厂商 DLL、把局部 draft 当设备响应，或将发送完成当回读一致。

每次调用拥有 COM apartment，S_OK/S_FALSE 各平衡一次 CoUninitialize；错误返回非空接口由共用 COM RAII 释放，ID 字符串按 CoTaskMemFree 清理，接口在 apartment 关闭前释放。空/含 NUL ID、错误 scalar、失效端点或失败读取明确报错；不会从未初始化值制造 successful zero。操作范围持有端点 ID 派生的 OS file try-lock，跨本应用 worker 串行化读取或完整 read/mutate/read-back；busy 立即返回，不阻塞、不重试、不删除 lock 文件。这是本应用协调政策，与原线程队列/通知缓存不同，也不能排除其他程序同时修改系统音频。

## 尚未等价与验收

原 getter 读取长期通知维护的缓存，Rust 每次实时查询；原读取失败可能保留旧缓存，Rust 报实际失败。长期 endpoint map、通知注册/注销、默认设备与事件历史、虚拟通道缓存重选/重新订阅和完整服务线程生命周期继续缺失。当前 factory/page/caller 已静态闭合；产品 UI 的具体端点关联、读取、提交、失败和刷新实现由页面专项独立记录，不能因有 IPC 就算完成全部产品界面。

四个导出的具体语义、共享实现和 Windows 系统写回已取得证据；运行验收未执行，整个 DLL 完成数仍为 0。复核使用 `python -X utf8 tools/audit-simple-audio-volume-current.py --check`；`--acquire` 仅进行私有副本 IDA 静态分析。
