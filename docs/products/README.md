# 产品、界面与本机功能逆向

本目录按产品族、具体型号和共享功能分别解释当前官方源码。逐型号文件保留实际导航顺序、条件分支、组件调用参数和原件位置；功能文件解释界面在操作什么、输入含义、状态来源、计算、设备／服务调用、结果消费者与清理。联网业务不在本文档范围内。

## 产品入口

| 产品族 | 当前登记 PID 数 | 功能与逐型号入口 |
| --- | --- | --- |
| 鼠标 | 75 | [自定义、性能、灵敏度、校准、滚轮与电源](mouse/README.md) |
| 键盘 | 73 | [配置档、自定义、触发、校准、灯光、电源与 OLED](keyboard/README.md) |
| 音频 | 77 | [播放、EQ、麦克风、预览与混音器](audio/README.md) |
| 系统 | 41 | [键盘、灯光、散热、显示、电池与声音](system/README.md) |
| 配件 | 56 | [摄像头、接收器、底座、灯光与散热](accessories/README.md) |
| 手柄 | 9 | [映射、摇杆、扳机、校准与灯光](gamepad/README.md) |

登记表当前共 331 个 PID。实际源码是页面和分支的依据；登记表没有的独立当前入口仍需阅读，见 [鼠标入口 162](mouse/products/162.md)。登记词条 `TAB_PAIRING` 不自动构成已挂载页签，实际配对入口结合 [接收器与底座](accessories/receiver-and-dock.md) 及 [窗口生命周期](host/window-lifecycle.md) 阅读。

## 宿主本机功能

| 页面／功能 | 界面与行为说明 |
| --- | --- |
| Dashboard | [设备卡片、模块、本机启动与导航](host/dashboard.md) |
| Profiles | [本机游戏、设备与配置档关联](host/profiles.md) |
| Settings | [常规设置、重置、配对与本机状态](host/settings.md) |
| Macro | [录制、步骤编辑、播放、保存和映射](shared/macros.md) |
| Chroma Studio | [场景、图层、区域、时间轴与本机媒体](host/chroma-studio.md) |
| Chroma Connect | [本机应用、设备与 Quick 效果](host/chroma-connect.md) |
| Visualizer | [音频捕获、FFT、灯光帧与输出](host/visualizer.md) |
| Sensa HD | [来源增益、设备强度、频段与游戏会话](host/sensa.md) |
| 配置迁移 | [本地文件、解析、应用与结果](host/profile-migration.md) |
| 固件与安装生命周期 | [本地载荷、写入、结果与清理](host/firmware-and-installation.md) |
| 窗口生命周期 | [窗口复用、休眠、焦点、托盘与退出](host/window-lifecycle.md) |
| 系统集成 | [系统事件、进程观察、退出信号与原生清理](host/system-integration.md) |

## 共享交互

| 主题 | 说明 |
| --- | --- |
| [源码阅读约定](shared/source-reading.md) | 产品／页面／功能关系、证据回查及完成边界 |
| [设备发现与直接通信](shared/device-communication.md) | USB／HID 发现、队列、I/O、回调与停止释放 |
| [设备配置档与游戏关联](shared/profiles-linked-games.md) | 配置身份、切换、导入导出与关联 |
| [颜色选择器](shared/color-picker.md) | 色值、输入、拖动及保存消费者 |
| [动态灯光](shared/dynamic-lighting.md) | 系统灯光控制权与设备分支 |
| [帮助与重置](shared/help-reset.md) | 本机版本、序列号、具体重置调用与结果 |

## 核验边界

当前逐产品结构收据见 [产品阅读结果](../../.analysis/results/product-documentation.json)，前端完整模块及合成映射见 [恢复结果](../../.analysis/results/frontend/module-recovery.json)。每份功能说明继续指向原始文件区间、完整函数捕获与实际消费者，不以目录、组件数或分析器退出码代替语义结论。

源码获取、语义理解、Rust 独立实现、UI／后端接线、静态／模拟验证和运行验收分别判断。本目录记录静态逆向；未执行厂商 JavaScript、应用程序、安装器或真实设备操作，不代表 Rust 实现、页面还原和真实运行验收已经完成。
