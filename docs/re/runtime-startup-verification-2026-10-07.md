# 启动设备信息与 DLL 查询复核（2026-10-07）

后续已核实 179 的 V2 查询、HID 报文、183→182 映射及发布器副作用，见 [查询契约](receiver-discovery-read-contract.md)；仍未将自动发现接入 Rust worker/首页，不能把取证更新当成实际读取已完成。

本轮按用户指出的启动读值问题重新追查 `AppShell::new → store`、服务面板 → worker → native callback，以及当前正式宿主源码。仅静态读源、PE 文件字节和允许的 cargo check；没有运行应用、测试、worker、安装器或 DLL。

## 已确认并修复

1. **启动显示的设备并不是一次 DLL 查询。** 此前没有存档或存档读取失败时都会调用 `model::measured_devices()`，放入固定 182/179、固定容器与序列号、固件、电量和 READY 状态。现在这两个分支都保持空列表；真实查询缺失不能用历史样例补齐。显式产品预览继续由设置入口提供。
2. **存档不是本次设备观察。** 载入本地工作区后，身份、产品能力、全部 profile 和 `source_device_settings` 草稿保留；电量、固件、安装/升级/供电临时状态失效，Dashboard 显示“本地配置 · 设备状态未读取”。本地实现的产品页面照常可打开，不被 UNKNOWN 当成安装门控。会话标记 `local_snapshot` 不序列化。
3. **删除固定电量改写。** 原 `normalize_known_measurements` 将 182 的 `47 + NoCharge_BatteryFull` 改成 100，没有本次读值依据。当前 Dashboard 的电量与充电状态有独立输入，不能凭状态枚举制造百分比。
4. **服务 DLL 选择修正。** 当前宿主 `main.js` 正常模式从本宿主版本的 `CommonDLL` 加载确切的 `mapping_engine.dll` / `simple_service.dll`；只有显式 debug 模式才用 `Apps/Common`。Rust worker 现在同样只从所选已安装宿主 CommonDLL 取两个确切文件，不再悄悄换成产品子目录/下载目录的同名 DLL。宿主目录与版本文件按数字分段排序，修复 4.0.999/4.0.1000 的字符串顺序错误；通用发现保持目录优先级且只接受有效 `.dll` 版本文件。
5. **查询失败不能显示成功。** 空指针不再变成成功的空字符串；空查询返回报错。音频 `deviceList` 必须是合法 JSON 数组，错误文本不能退化成“已收到服务信息”。服务面板缺任一请求结果也不再声称完整成功。零项数组仍是有效空结果。
6. **修正导出说明。** 当前封装调用的是未修饰导出名；本机所检查文件同时具有未修饰和 C++ 修饰别名。诊断清单改为当前封装使用的名称，不再宣称“只有修饰名”。导出存在不等于 ABI/线程/实际响应验证完成。

## 当前源码与二进制证据

[`runtime-startup-current-evidence.json`](runtime-startup-current-evidence.json) 保存 14 个重新解析的 AST 收据，含 SHA-256 与 UTF-16 半开区间：当前宿主 main 的正常/debug 路径、两个 service wrapper 的 API 与方法、`UsbRzDeviceAction::usb.getDevices`，以及当前 Dashboard 22534 的 z/G/V/K。由 `node tools/audit-runtime-startup.cjs --check` 复核，未求值任何厂商脚本。

[`runtime-pe-current-evidence.json`](runtime-pe-current-evidence.json) 保存实际读取的两个 x64 PE 文件、大小、SHA、导出 RVA 与同地址别名。`python tools/audit-runtime-pe.py --check` 仅读字节复核 8 个本批查询/生命周期导出。

| 本次实际文件位置 | SHA-256 |
| --- | --- |
| `C:/Program Files/Razer/RazerAppEngine/app-4.0.827/CommonDLL/mapping_engine.dll` | `6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a` |
| 同目录 `simple_service.dll` | `f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b` |

本轮发现本机已经存在上述 app-4.0.827 目录，区别于 10-02 文档记录的当时安装状态；本轮未执行升级，也未断言该目录就是正在运行的宿主。此收据证明文件身份与导出，不宣称它们与官方更新容器中的二进制已逐字节比较。源码依据仍为已有独立取证的当前 4.0.827，不使用任何被禁止的旧源目录。

## 仍未接通的真实读取

- 原宿主设备枚举实际走 `usb.getDevices → rz-usb-detect.find()`，随后进入 Background/MW 初始化和运行状态发布；`simpleGetVersionInfo` 与 `simpleEnumerateAudioDevices` 不是完整 Razer 产品列表接口。
- 现有 Rust `HidDevices` 是 Windows HID collection 元数据枚举，不等价于 USB 物理设备、接收器配对从设备、已加载 profile、固件、电量或 READY。此次没有把 HID 条目直接转换成已就绪设备，也没有从音频端点猜产品。
- 服务面板仍为用户触发的实际读取入口；自动启动设备发现、热插拔、当前 profile/容器的发布者及消费者接线仍待实现。移除旧样例并不代表自动读取已完成。
- 现有核心 wrapper 回调类型重新核实，但未执行 DLL；真实读取、回调线程、设备响应和运行视觉都未验收。DLL 修改、写回和设备保存仍后置。

本轮 `cargo check --locked --all-targets` 已通过，仍为三项既有 dead-code 警告；最终跨任务格式/资源验证结果由总复核记录统一列出。
