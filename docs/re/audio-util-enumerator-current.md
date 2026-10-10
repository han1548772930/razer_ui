# 当前 RzAudioUtil AudioEnumerator 完整读取链

本轮静态还原当前 `RzAudioUtil_v1.0.3.1.dll` 的两个 AudioEnumerator 操作，并新增直接使用操作系统 COM 的 Rust 后端。没有加载厂商 DLL、执行下载的 JavaScript、运行应用或向设备发送命令。这里是两个明确功能的实现，不代表整个 RzAudioUtil 库、所有音频产品或全程序已完成。

## 来源与覆盖产品

DLL SHA-256 为 `9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf`，来自当前中间件 manifest 的公共 AudioDLL 资源。当前产品 `1398、1422、1427、1446、2638、2641、4124、4126` 的 manifest 均绑定此文件；静态核验了八个 manifest 的 hash，并在对应当前 middleware 中提取了 16 个 Playback/Record 实际调用点。相同产品的同名 DLL 或更旧 `1.0.1.1` 不能据此推断相同行为。

[完整机器证据](audio-util-enumerator-current-evidence.json) 包含文件 hash、manifest 绑定、当前调用点及源片段、八段完整指令范围、逐项指令门控、命令表、GUID 和属性键实际字节。维护工具为 [audit-audio-util-enumerator-current.py](../../tools/audit-audio-util-enumerator-current.py)，仅解析源文本、PE 数据和静态反汇编。`--check` 重新生成后按字节核对证据；不执行 DLL。

## 真实调用链

```text
current middleware getWindowsPlaybackDevices / getWindowsRecordDevices
  → audioUtil.dispatch("AudioEnumerator", exact command, {}, result)
  → JSON {cmdClass, command, cmdData}
  → current DLL Dispatch RVA 0x1AF70
  → dispatchAudioEnumerator RVA 0x1660
  → command table / callable vtable +0x10
  → Playback thunk 0x7040 → 0x2110
     Record thunk 0x7000 → 0x2680
  → enumerate-vector helper 0x44AD0 → endpoint implementation 0x44DC0
  → MMDeviceEnumerator / active endpoint collection / default roles / properties
  → ordered friendly-name array {response:{devices:[...]}}
  → dispatch success response copied to result; middleware validates Array.isArray
```

Thunk 的两条指令先调整 callable 的 `this`，再跳到真实实现；不是根据导出名称猜测目标。当前函数图原先没有包含这两个间接 callable 实现，本轮从命令表实际槽值补齐八段范围。其中两个 leaf thunk 没有 `.pdata`；证据明确只覆盖其已证明的九字节 `add/jmp` 正文。

## 端点选择、数据与失败

`CoCreateInstance` 使用类 GUID `bcde0395-e52f-467c-8e3d-c4579291692e`、接口 GUID `a95664d2-9614-4f35-a746-de8db63617e6`、上下文 `0x17`。Playback 为 flow `0`，Record 为 `1`；`EnumAudioEndpoints` 的状态掩码均为 `1`，只枚举 active 端点。不按 Razer VID/PID、名称、设备容器或接口过滤；这是系统音频端点查询，不能代替鼠标/键盘发现，也不能拿该端点 ID 当 Mixer jack ID。

枚举后分别读取默认 Console `role=0`、Communications `role=2` 的端点 ID。`GetDefaultAudioEndpoint` 非零返回时，原程序保留空默认 ID 并继续；成功后 `GetId` 失败则终止本次 helper。默认标记通过完整 UTF-16 ID 比较，不折叠大小写。不额外查询 Multimedia role `1`。

每个 collection item 依次读取 ID、FriendlyName、DeviceDesc，两个属性都通过各自 `OpenPropertyStore(STGM_READ=0)` 获取。共同 fmtid 为 `a45c254e-df1c-4efd-8020-67d146a850e0`，pid 分别为 `14`、`2`。`GetValue` 只接受 `S_OK=0` 或 `INPLACE_S_TRUNCATED=0x401A0`，variant tag 必须为 `VT_LPWSTR=31`。原程序在 Item/GetId/属性打开/属性获取/类型失败时跳过这个 item；不补造名称或 ID。

底层成功记录 stride 为 `0x68`，含 ID、friendly name、description 和两个默认标记。返回给 middleware 的命令实现只复制记录 `+0x20` 的 friendly name，保留 collection 顺序、重复和空名称，不按默认标记重排。当前 middleware 的下游流混音器再按产品名及 Headphones/Aux 等字符串查找，是独立消费者逻辑；本模块没有把那些字符串写死成端点选择规则。

原 helper 对 `CoCreateInstance`、collection 枚举或计数等多个严重失败返回空 vector，命令函数随后仍设置 `HRESULT=0`。Rust 在这些严重失败时显式返回错误；对原程序允许继续的缺失默认端点和 item 跳过保留结果及 diagnostics。这是为避免把实际查询失败伪装成成功的明确差异，不能把 Rust 错误路径登记为原 DLL 完全一致。

## Rust 与实际消费

- [razer-device/audio_util.rs](../../crates/razer-device/src/audio_util.rs) 实现平台无关 flow、完整端点记录、源语义默认标记、原 `{devices:[names]}` 结果和精确 HRESULT 判定。
- [razer-service/audio_util.rs](../../crates/razer-service/src/audio_util.rs) 实现无厂商 DLL 的 Windows COM 查询、端点逐项读取、严格属性判定、失败诊断和内存释放。其他平台返回明确 unsupported；此处 Windows Core Audio 能力不能由通用 HID 替代。
- `ServiceRequest::AudioEndpoints { flow }` 经 worker/portable runtime 调用该适配器。响应分别保留 `source_response`、完整 `observation`、`transport=windows_core_audio`、`vendor_dll_loaded=false` 和证据路径。客户端读取预算为 20 秒；预算不能中断正在执行的 COM 系统调用。原 `AudioDevices` 仍属于 simple_service，两个 schema 没有互换。

COM 调用槽逐项对照机器码和对应接口 ABI：IMMDeviceEnumerator `+0x18` EnumAudioEndpoints、`+0x20` GetDefaultAudioEndpoint；IMMDeviceCollection `+0x18` GetCount、`+0x20` Item；IMMDevice `+0x20` OpenPropertyStore、`+0x28` GetId；IPropertyStore `+0x28` GetValue。各类接口都通过 IUnknown `+0x10` Release。

Rust 单次调用拥有 apartment，`CoInitializeEx(MTA)` 成功和 `S_FALSE` 都平衡一次 `CoUninitialize`；已有不同线程模型导致初始化失败时明确报错。接口包装禁止跨线程移动，全部 interface 在 apartment 关闭前释放。每个 GetId 返回字符串在成功或失败路径使用 `CoTaskMemFree`；属性值只借用 variant 中的字符串，随后调用一次 `PropVariantClear`，不会另一次释放该 borrowed 指针。成功 HRESULT 返回空接口、空字符串、非法 UTF-16 或超长字符串均显式错误，避免复刻源程序的无效指针解引用。

## 状态与仍缺内容

来源取得、两条机器实现语义、共享 Rust、Windows 后端和 IPC 消费分别有证据；当前 UI 尚未消费 `AudioEndpoints`，没有执行应用或 COM 查询，因此运行验收仍未完成。最终工作区统一执行允许的 cargo check 与静态验证；编译通过不替代运行验收。

RzAudioUtil 的 EnableNotification 共享队列、Windows 注册/注销、IPC 和八产品 Shell 生命周期已按[独立 IDA 证据](audio-util-notifications-current.md)实现，运行验收仍未执行。AudioRouter EnableRouting/RouteDevice 写回与清理、MediaPlayer 命令/播放状态/事件仍未全部闭合或实现。两个当前 GetDLLVersion 版本分支（1.0.1.1 与 1.0.3.1）已按独立 IDA 证据接入源语义，但不能把单项或九个产品资源绑定当成整个库已完成。Mixer endpoint-ID/jack/flow 关联也没有由这里的系统端点枚举证明。
