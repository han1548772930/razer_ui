# 当前 simple_service 音频列表与 Rust 消费者

使用本机 IDA Pro 8.3 + Hex-Rays 静态分析当前 host 4.0.827 的 `simple_service.dll`，原件 SHA-256 为 `f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b`。分析输入是工作目录中的独立副本，未调用导出、未启动调试器、未执行目标代码。保留 [32 个函数的伪代码、机器码哈希及交叉引用](simple-audio-current-evidence.json)，由 [静态审计工具](../../tools/audit-simple-audio-current.py) 复核原字节。

## 原 DLL 的实际链路

`simpleEnumerateAudioDevices@0x16d40 → singleton +0xb0@0x4cb0 → task@0x32082 → dispatch@0x32270 → instance +0x30`。实例并非猜测：`0x31db0` 调用 `0x38f60` 创建 `AudioServiceWin`，写入 service `+0x10`，再调用实例 `+0x20` 初始化。构造器绑定虚表 `0x1f54d0`；该表 `+0x20=0x39070`、`+0x30=0x3a170`，因此导出最终进入这一个 JSON 组装函数。

初始化 `0x39070 → 0x397aa` 用 Core Audio 枚举 `eAll=2 / ACTIVE=1`。`0x3e864` 创建 MMDeviceEnumerator，CLSCTX 为 1；只在 `CO_E_NOTINITIALIZED` 且允许重试时，`CoInitializeEx(NULL,4)` 后重试一次。名称使用 `{a45c254e-df1c-4efd-8020-67d146a850e0}, pid=14`；读取成功但不是 VT_LPWSTR 或指针为空时，原函数保留空名称。

端点类型来自 `IMMEndpoint::GetDataFlow`：0 为 `speaker`，1 为 `microphone`，失败为 `all`。原端点初始化还要求以 CLSCTX 0x17 激活 `IAudioEndpointVolume`，Rust 读取同样核对这一条件，随后释放接口；没有执行音量设置。

ContainerId 首先走 SetupAPI 音频类 `{4d36e96c-e325-11ce-bfc1-08002be10318}`、flags 0x14，读取实例 ID 的 `SWD\MMDEVAPI\` 后缀。原 CRT `0x1d2d94 → 0x1d2de8` 先转小写，再与端点 ID 比较。属性是 `{8c7ed206-3f8a-4827-b3ab-ae9e1faefc6c}, pid=2`，DEVPROP_TYPE_GUID=13。失败则读 Core Audio 同一属性，要求 VT_CLSID=72；均无值时保留空字符串。两支 GUID 均由 `0x75c10` 格式化为大写带花括号，再转 UTF-8；`0x86130` 是字符编码转换，不能误认为大小写转换。

`0x3a170` 返回数组，每项包含 `type/id/containerId/name`。遍历顺序来自实例内部 libc++ 哈希表的链表，既非名称排序，也非枚举顺序。原 64 位哈希的四个长度分支、load factor=1、bucket 增长、插入及 rehash 已按 `0x3b820/0x3be88/0x1f746` 还原；Rust 不使用随机哈希表代替。此顺序影响 Control Pod 页面在没有历史选择时选取的第一个 speaker。

## 实现与连接

[共享数据及顺序实现](../../crates/razer-device/src/simple_audio.rs) 不依赖系统；[Windows 适配器](../../crates/razer-service/src/simple_audio.rs) 负责 COM、SetupAPI、HRESULT 和资源释放。`ServiceRequest::AudioDevices` 已在 portable worker 分发到该实现，返回原数组 schema，现有设置页和 Control Pod 的消费者继续接收这个请求。此请求不再加载原 DLL；`SimpleVersion` 和其他原生服务功能仍有原 DLL 路径。

只初始化/撤销本次调用拥有的 COM apartment，接口按 RAII Release，PROPVARIANT 按 PropVariantClear 释放，GetId 的内存按 CoTaskMemFree 释放。系统专属能力仅在 Windows 实现，其他平台明确返回未移植错误，不伪造空数组。

## 尚未等价的行为与验收边界

speaker/microphone 的四个音量和静音 getter/setter 已另用 36 个 IDA 正文追到同一端点缓存和 Core Audio 操作，并实现无厂商 DLL 的真实读取、音量/静音写回及 IPC。具体源参数、1352 当前产品 gate/页面、部分失败顺序、回读及 COM 释放见 [音量专项](simple-audio-volume-current.md)。长期缓存/通知与完整产品页面连接仍分别登记，四个功能不计为整服务完成。

- 原服务初始化一次后，用端点通知、状态变化和名称变化持续维护对象/哈希表。当前替代每次请求重新枚举；只还原当次完整初始化列表的顺序。长期运行中的新增/删除/更名历史、通知注册、音量和会话回调仍是待实现功能，不能把原服务全部算完成。
- 原初始化对 Item/名称失败可能停止后仍标记部分成功，AddAudioDevice 失败另有返回分支；原导出未核对全部初始化状态就能查询缓存。替代明确报告读取失败，不把部分数组冒充完整观察。这是有意保留的诊断差异，不是完全等价的错误语义。
- 原端点对象重开端点 ID、安装渲染通知和音量/会话订阅；当前列表适配读取返回字段并核对音量接口可访问性，没有复刻全部订阅生命周期。初始化期间断连的时序尚未验证。
- 源 UTF-16 转换可替换非法字符；替代对非法字符串报错，并对端点数量/字符串扫描设置应用上限。它们是应用策略，不是原协议常量。
- 32 个函数静态恢复和 cargo check 不代表全 DLL 已替代；未执行应用、DLL、设备、测试或运行验收。

复核：`python -X utf8 tools/audit-simple-audio-current.py --check`。重新生成 IDA 证据使用 `--refresh --ida <idat64 路径> --python-dir <兼容 Python 目录>`；工具只为分析进程设置环境，不修改全局注册表、IDA 安装或系统 Python。
