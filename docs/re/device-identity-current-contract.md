# 当前全目录发现与身份读取：2026-10-07

本轮接入的是代码和静态证据，未运行应用、测试、厂商 JavaScript 或 DLL，不能据此声称本机设备已经读取成功。

## 来源与完整范围

- [全目录 MW 清单](middleware-device-bindings-current.json)：官方目录与已有当前 UI 共 332 个产品入口；329 个已解析导出的 DeviceInfo（含 105→104 的源别名）。162 的当前源内联声明 productId=163；3886 使用内联身份对象；这两项尚未证明导出绑定，单独保留原文。680 的官方 MW HTML/manifest 返回 404，缺源没有用旧版替代。
- [宿主当前链](device-identity-current-evidence.json)：4.0.827 的 `checkAllRzDevice` 分别合并 USB、BLE、IoT、monitor；`getRazerDevices` 再更新 HID。`HD/_checkUSBDetail` 区分 product/rep/dongle/ble/wired/xbox/PS/monitor 别名。8 项 AST 均附当前路径、SHA-256、区间和原文。
- [完整身份目录](discovery-catalog-current-evidence.json)：330 行 AvailableDevices 原样投影，不将数组 dongleId 误当标量匹配；歧义保留。目录本身不产生连接观察。
- [原生 USB 证据](usb-native-current-evidence.json)：当前 detection.node 的 SHA-256 和 USB_DEVICE GUID 静态核验；它没有独立 C 枚举导出。本地使用 Windows SetupAPI，不能称为调用厂商 DLL 的独立枚举 API。HID Feature 收发仍使用已核验原版 HID.node C 导出。

## 当前接线

`ServiceRequest::UsbDevices` → 独立 worker 的 `runtime_usb` → RuntimePanel → `discovery::discover` → 首页工作区。此前 runtime_usb 没有被任何模块引用，编译及启动均未覆盖它。现在 USB/HID 分开请求，任一路失败保留另一路的真实结果；失败、格式错误、未结束枚举进入诊断，旧观察失效，已有本地草稿保留。

实际 USB 与 HID 按 vendor/product/container 归组去重，HID 仅作查询传输及描述符补充。接收器 USB 序列号不会作为无线鼠标的序列号。型号/名称可引用静态目录，版本、布局、固件、配置、运行能力仍未查询。连接只显示 USB 接口存在/HID 接口存在/接收器返回的原始状态，不显示虚构的设备配置成功。

## 全目录直接双链查询

`plan-receiver-protocols.cjs` 从每个产品实际 feature/factory/loader 追踪；45 个 isDualLinkDevice 产品中 6 个没有直接 dongleId，不生成直接接收器能力。其余 **39 条**均由 `audit-receiver-catalog.cjs` 独立解析并生成 [运行能力资源](../../assets/data/receiver-query-capabilities.json)和[逐产品证据](receiver-catalog-current-evidence.json)，替代原先仅 179/227 的运行目录。

每项解析自己的 DeviceInfo、构造分支、类继承、V2 调用者、命令 `[80,0,191]`、91 字节报告、接口号、重试与延时、事务前缀及命令例外。鼠标普通接收器前缀 0，键盘前缀 128，底座/Linker 前缀 224；按实际源码生成，不把一项参数推广给所有产品。键盘列表解析允许 `dongleId === pid || productId === pid`，鼠标/Linker 仅允许标量 dongleId 相等；两条身份查询路径保持区别。

查询仍仅提交已核实的读取报文，不执行原始工厂的整段 connect/setup（含设备模式等写操作）。原生响应的长度、事务、命令、状态、记录数和 PID 范围检查继续生效；重复 PID 不能区分身份时整次报错，不采用第一条伪装唯一结果。查询前后重核实际实例，错误不变为空列表。

164/241 的真实 `DUALLINK_BIND_INFO` 已连接到配对页面/弹层，取消和旧代际不回填。只提供真实 status/PID 与官方目录名称/类别，edition/layout/serial 缺省；扫描/绑定/解绑仍保留未发送的 UI 意图。226 轮询页面的 Connection 观察从本次真实目录身份投递，失效时清空；没有生成轮询数值或双链拓扑。

## 仍需逐项完成

- 162/3886 的内联工厂完整绑定及 680 当前缺源。
- Mika、非双链无线、其它协议的设备状态、版本/布局/序列号/配置读取；USB 身份枚举不是这些查询的完成证明。
- 宿主独立 BLE/IoT/monitor 分支、ASRock 检测条件、两类目录歧义的后续身份查询，以及持续热插拔/状态订阅。当前接的是启动/手动刷新观察，不能称实时热插拔已完整实现。
- 原版服务私有 HID 锁不能与本项目本地互斥锁互认；事务校验及错误边界保留，不宣称跨进程设备访问已运行验收。
- 39 条查询能力是源代码/接口核实，实际硬件支持或读取成功须在允许实机验证后确认。

工具仅静态解析与资源准备。`cargo check --locked --all-targets`、格式与 JSON/来源检查单独记录在本轮续接文档；DLL 修改、设备保存、写回仍后置。
