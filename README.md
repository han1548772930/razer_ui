# razer_ui

使用 Rust + GPUI Kit 复刻当前稳定版 Razer Synapse 的界面与完整程序功能。

从 [文档索引](docs/README.md)、[全程序范围矩阵](docs/re/full-source-reverse-map.md) 和 [实施路线](docs/re/ui-readonly-first-roadmap.md) 开始。宿主、服务、账户、存储、网络、系统集成、安装更新、原生件、全部产品、界面、交互和资源均以当前官方源码为依据。

项目参考 OpenLogi 的分层方式，拆为 20 个库 crate 和 GUI/agent 两个可执行包，见 [workspace 架构](docs/re/workspace-architecture-current.md)。`razer-shell` 保留窗口和动作协调，应用/产品页面、Dashboard、设置、托盘分别归属独立包。UI 通过 `razer-ipc` 查询后台，不依赖 HID/DLL 实现；协议定义契约，`razer-hid` 实现契约。GUI 入口为 `app/main.rs`，旧 `src/` 已移除，比较副本保留在 Git。

现有鼠标/接收器查询已使用 [跨平台 HID 后端](docs/re/cross-platform-hid-current.md)，这部分不再加载官方 `HID.node`；其他 DLL 功能及 Linux/macOS UI 身份适配仍有缺口。

当前 331 个产品、1419 个主导航页均有部分内容；完整复刻验收的产品仍为 0。独立窗口、弹层和条件分支不包含在这些数量中，详见 [覆盖统计](docs/re/native-product-coverage.md)。

UI 编辑、增删、应用、保存、本地草稿、DLL 查询与设备/服务写回均在当前范围。已接入的直接写入及缺口见 [写入契约](docs/re/device-write-current.md)，现有读取见 [接口盘点](docs/re/dll-readonly-inventory.md)；本地草稿与真实写入结果分别记录。

大体积逆向 JSON 以无损 ZIP 保存在 [全量证据存档](docs/re/evidence/README.md)，参数、函数原文、源码收据与页面细节完整保留。使用现有静态生成工具前可恢复原 JSON 路径，或通过存档工具临时恢复后检查。

继续工作先查 [修复登记](docs/re/ui-fix-registry.json)。已修项保留当前源码和实现指纹，只有相关输入变化或新问题才复查；更新同一记录，不累积修复历史文档。

当前 Dashboard 源码为 `.ref/applications/synapse/dashboard/`，宿主为 `.ref/host-4.0.827/`。只允许格式化、静态解析、资源校验及 `cargo check --locked --all-targets`；不运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。完整约束见 [AGENTS.md](AGENTS.md)。
