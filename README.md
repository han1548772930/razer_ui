# razer_ui

使用 Rust + GPUI Kit 复刻当前稳定版 Razer Synapse UI。

从 [文档索引](docs/README.md) 和 [实施路线](docs/re/ui-readonly-first-roadmap.md) 开始。界面、交互和资源以当前官方源码为依据，不凭相近产品猜测。

项目按 OpenLogi 的职责边界拆为 20 个库 crate 和 GUI/agent 两个可执行包，见 [workspace 架构](docs/re/workspace-architecture-current.md)。`razer-shell` 保留窗口和动作协调，应用/产品页面、Dashboard、设置、托盘分别归属独立包。UI 通过 `razer-ipc` 查询后台，不依赖 HID/DLL 实现；协议定义契约，`razer-hid` 实现契约。GUI 入口为 `app/main.rs`，原 `src/` 保留为参考，不参与编译。

现有鼠标/接收器查询已使用 [跨平台 HID 后端](docs/re/cross-platform-hid-current.md)，这部分不再加载官方 `HID.node`；其他 DLL 功能及 Linux/macOS UI 身份适配仍有缺口。

当前 331 个产品、1419 个主导航页均有部分内容；完整复刻验收的产品仍为 0。独立窗口、弹层和条件分支不包含在这些数量中，详见 [覆盖统计](docs/re/native-product-coverage.md)。

UI 编辑、增删、应用、保存流程、本地草稿及 DLL 只读查询属于当前工作范围；通过 DLL 修改设备或服务状态、写回和保存留待后续统一接入。当前接线及缺口见 [DLL 只读盘点](docs/re/dll-readonly-inventory.md)。

继续工作先查 [修复登记](docs/re/ui-fix-registry.json)。已修项保留当前源码和实现指纹，只有相关输入变化或新问题才复查；更新同一记录，不累积修复历史文档。

当前 Dashboard 源码为 `.ref/applications/synapse/dashboard/`，宿主为 `.ref/host-4.0.827/`。只允许格式化、静态解析、资源校验及 `cargo check --locked --all-targets`；不运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。完整约束见 [AGENTS.md](AGENTS.md)。
