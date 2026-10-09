# 当前 DLL 内部与设备通信逆向

本轮开始分析已取得的全部当前产品 DLL 的机器码，不再只登记导出名。范围为 **58 个产品 DLL、4 个宿主 CommonDLL 和 19 个宿主 Windows 原生插件，共 81 个 PE 文件**。原生插件包含 x86、AMD64、ARM64 和不同 Node ABI 的版本；不表示它们全部在当前宿主运行。其他平台 `.node` 和只取得包清单的外层辅助文件没有混入这个分母。

源来自当前产品 manifest 收据与 `.ref/host-4.0.827/` 官方包提取记录，逐文件核对 SHA-256。没有加载 DLL、执行厂商 JavaScript、运行应用或向设备发送命令。

## 本轮取得的二进制正文

[逐文件摘要](dll-device-communication-current-summary.json) 保存全部 81 个文件的身份、状态和计数；[完整机器码图](dll-device-communication-current.json.gz) 保存具体指令、RVA、代码段 hash、导出入口、导入槽、潜在调用路径和未解开的间接调用。

| 项目 | 当前结果 | 含义 |
| --- | ---: | --- |
| PE 文件 | 81 | 58 产品 DLL + 4 CommonDLL + 19 宿主 Windows 插件 |
| 导出条目 | 2831 | 包括转发、非代码与未分析架构，不能全算成可调用 API |
| 分析图入口 | 57960 | 导出入口、直接跳转/调用及相邻 unwind 区间；可能是同一逻辑函数的不同入口，不能当作原 C++ 函数数 |
| 到选定系统 API 的潜在路径 | 8791 | 仅机器码中的语法可达链，尚未证明分支可达性、参数或设备适用性 |
| 整个 DLL 语义完成 | 0 | 尚未完整恢复虚函数、回调、动态加载、参数和产品门控 |
| 设备命令执行 | 0 | 按项目约束保持静态分析 |

MSVC 将同一逻辑函数拆成多个相邻 `RUNTIME_FUNCTION` 区间，区间结束不等于函数返回。工具单独记录跨区间的普通顺序执行边，避免丢掉后续 WriteFile/GetInputReport。导出之外的 Node 注册回调、虚表、工作线程和动态函数指针仍需独立追踪；导出为 0 或入口图小，不能说明插件没有功能。ARM64 文件只验证了字节和导出，不声称完成反汇编；含 CLR header 的 DLL 也没有完成 IL 恢复。

## “直接与设备通信”的实际边界

原代码包含不同底层链路，不能把所有 DLL 统一替换成一种 HID 包。以下分类按已保存的导入及机器码路径导航，不根据库名认定整个库的职责，也不表示替换已经完成。

| 原码通道 | 当前具体依据 | 接入前还需要恢复 |
| --- | --- | --- |
| Razer HID protocol 25 | 当前 `UsbRzDeviceAction`、`protocol25` 和 `node-rz-hid/HID.node`；见 [接收器 HID](receiver-native-hid-current-evidence.json) | 每个产品的接口、report ID/长度、命令构造、响应匹配、状态码、重试和事件；不可把 241 的报文套到所有产品 |
| 产品 DLL 内的 HID report | Audio Mixer 两层 DLL 已取得明确分发表和报文 helper，见 [Mixer 专项](audio-mixer-dll-protocol-current.md) | 属性参数、selector、范围/比例、返回消费者和实际型号门控；查询也需要发出请求包 |
| WinUSB/固件接口 | `Razer_Upgrade_SDK.dll` 的导入与潜在调用路径 | 端点、控制请求及固件状态机；不能把升级命令当普通配置 |
| 驱动 IOCTL | 二进制图保留 `DeviceIoControl` 的实际调用位置；Mixer ResetStream 等有独立路径 | 精确控制码、对象/句柄来源、输入输出布局与错误处理 |
| Windows COM/音频/媒体 | 原 DLL 包含 COM 激活和 Media Foundation 路径；所有间接接口槽继续保留未知 | GUID、接口槽、音频 endpoint 身份、DSP/服务转换；纯 HID 无法覆盖这些已经确认存在的调用 |
| 外部服务、RPC、socket | 当前 THX、routing 和 Hue 等文件具有相关导入；实际潜在路径逐文件记录 | 服务地址、进程/连接生命周期、消息帧、订阅、缓存与失败语义；导入存在本身不证明产品已激活 |
| 动态加载的二级库 | DLL 机器码中 LoadLibrary/GetProcAddress；Mixer 已追到具体存储槽和二级函数 | 库路径选择、实际资源版本、函数指针类型、初始化和卸载；导出相同不代表同一个 DLL |

不提前决定某个产品“必须依赖 DLL”或“已经可以完全去掉 DLL”。只有完整恢复所需功能的底层协议、参数和生命周期后，才能逐项建立替代实现。保留原 DLL 作为本项目内部 transport 与用 Rust 复现协议，也是两种不同的实现策略；外部依赖能否删除应以这项选择及实际 OS 接口代码为依据。

## 第一条深入恢复的二级 DLL 链

当前产品 1342 manifest 同时拥有 `RzNative_053E_v1.0.16.0.dll` 和 `CmMixerLib_v1.0.1.0.dll`。已核对 parent 的 `GetProcAddress("CmMixerPropertyControl")` 返回值保存到 singleton 的 `+0x30`，属性控制入口从同一槽取值并调用；child 按 UTF-16 属性名在 37 项表中选择目标。具体机器码、表指针和报文构造见 [Mixer 专项](audio-mixer-dll-protocol-current.md)。

这条链说明只恢复外层 `MixerSDKLib_PropertyControl` 的 FFI 签名会遗漏 37 个内部属性处理函数。另发现当前 JS 出现 `RazerT2KeyShifterLevelEnable`，但该 child 分发表没有这项；不补造函数、不假定成功。

## OpenLogi 的参考范围

已按用户提供的仓库静态审阅 [OpenLogi](openlogi-device-communication-review.md)，固定 commit `1505c6525470bc0a38ae3ba79d70e950347d2532`，校验 157 份文件。可参考 transport、协议、逻辑设备、接收器子设备、能力发现、请求响应关联和错误状态的分层。

OpenLogi 使用 Logitech HID++ 1.0/2.0，不能据此采用 Logitech feature ID、report `0x10/0x11` 或接收器槽位协议控制 Razer。其源码还直接调用 Windows HID API；“直接通信”并不意味着底层没有操作系统接口。本轮只增加证据，不改动本项目依赖决策。

## 后续逐项恢复顺序

1. 根据真实产品 factory、DeviceInfo 和已加载资源追到实际通道，不能用共享文件的存在认定产品启用全部功能。
2. 对每个必要 API 追 export → 虚表/分发表/函数指针 → 二级库/驱动/服务/OS；记录初始化、工作线程、回调和卸载。
3. 对每个设备请求记录接口选择、report/控制码、字节布局、校验、响应字段、状态、超时/重试与消费者。getter/setter 名称只用于导航，不能作为副作用判定。
4. 证据闭合后接入 source-verified 读取和状态观察；保留未知和断连错误。UI 编辑/本地草稿照常保留，真实设备设置写回仍按 AGENTS.md 后置。

全量图只是所有已取得 PE 的内部正文起点。它不覆盖尚未取得的原生包，不将以上步骤登记为已经完成。

## 静态复核

```text
python tools/audit-dll-device-communication.py --check
python tools/audit-cmmixer-protocol-current.py --check
node tools/audit-openlogi-reference.cjs --check
```

全量工具只调用现有 `dumpbin` 读取机器码。`.work/dll-device-communication-static/` 保存以 binary SHA 与 dumpbin SHA 标识的规范化反汇编缓存；`--refresh-disassembly --check` 可重新读取全部指令再核对。缓存原文也有独立 SHA；没有执行待分析的 DLL。
