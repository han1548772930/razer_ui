# 当前全程序核对与实现路线

范围为整个原程序：启动与退出、宿主/模块/后台服务、窗口与托盘、账户、网络、安全、存储与迁移、系统集成、安装更新、协议/原生件、全部产品与界面。每条链核对入口、参数、状态、订阅、读写、响应、错误、取消和资源释放；UI 编辑、本地保存和设备确认分别登记。当前直接协议接入见 [设备写入](device-write-current.md)。

界面部分逐项核对当前官方源码，修复缺失的控件、操作、条件和布局。界面分母为 331 个产品、1419 个主导航页、33 个独立页及 24 个应用入口；路由或描述符存在不算界面完成，完整产品验收仍为 0。

## 修复记录与继续工作

按 [全量源码地图](full-source-reverse-map.md) 的范围矩阵核对各功能域，补充原源码契约与实际实现缺口。入口、签名或 AST 统计不能作为语义完成证明；每项需同时登记原链证据、Rust 消费者和验证边界。

页面细节继续从 [产品内部树与动作](product-ui-details-current.md)、[共享控件交互](shared-ui-controls-current.md)、[应用内部细节](application-ui-details-current.md)、[全页面样式与字体](ui-style-sources-current.md)进入。核对到真实caller和挂载条件后才能使用共享功能；参数/事件/CSS候选仍不是逐分支语义完成，也不是Rust对照或视觉验收。241的Lighting/Help实际分支已补入 [接收器契约](receiver-ui-current.md)，配对、灯光和重置的真实提交/响应链尚未接通，不能由本地操作推定设备成功。

1. 先查 [修复登记](ui-fix-registry.json)及对应当前契约。源码依据、实现和防回归检查未变化的已修项不重复改写；收到新的问题或指纹变化时，只复查受影响的具体行为。
2. 按 [当前缺口](remaining-ui-work.md)和[逐页队列](ui-review-queue-2026-10-07.json)推进。旧报告中的“未实现”不得覆盖当前代码与登记结果。
3. 每项记录具体行为、当前源证据、实现文件、允许的检查结果及未验收边界。更新同一记录，不追加日期命名的续接、复查或修复历史文档。
4. 修改源码依据或实现后，先核对变化、完成必要检查，再更新该修复的指纹；禁止为通过校验而盲目刷新全部记录。

## 当前范围与顺序

- 现有界面的按钮、编辑、增删、Apply/Save、取消、未保存保护及本地草稿均在当前范围。产品能力由各产品的实际挂载链生成，不能在共享实现中写产品编号分支。
- DLL 只读查询及观察结果到界面的链路同步补齐。枚举、连接状态、绑定元数据、设备配置和服务状态分别核实，未知不能冒充成功或空集合。
- 设备/服务写回及持久化纳入当前范围，按每项当前原码的参数、返回、生命周期与目标条件接入。本地保存必须明确为本地草稿，未实现写入的功能不能显示设备成功。
- 实际窗口、焦点、命中、像素、动画和硬件结果尚未运行验收，不用静态检查代替。

## 固定工作入口

| 范围 | 当前契约 |
| --- | --- |
| 宏按钮、录制、编辑与未保存流程 | [Macro](macro-ui-current.md) |
| 接收器父页面、配对及发现边界 | [Receiver](receiver-ui-current.md) |
| 鼠标能力、滚轮、DPI 与轮询 | [Mouse](mouse-ui-current.md) |
| Studio 属性与共享控件 | [Studio](studio-properties-current.md) |
| DLL 查询、ABI 与消费者 | [查询接口](dll-readonly-inventory.md) |
| 设备写入、应答与回读 | [写入接口](device-write-current.md) |
| 宿主、账户、存储、网络、安全、安装更新与退出 | [宿主全链](host-architecture-current.md)、[全程序矩阵](full-source-reverse-map.md) |
| 产品与入口覆盖 | [覆盖统计](native-product-coverage.md) |

只使用 [Dashboard 当前版本](20-current-source-version.md)、[host 4.0.827](current-host-version-audit.md)及各产品/应用的当前源码。遵守仓库 AGENTS.md：仅允许格式化、静态源码解析、资源准备/校验和 `cargo check --locked --all-targets`；禁止运行应用、构建、测试、安装器、厂商 JavaScript 或 DLL。
