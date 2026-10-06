# razer_ui

使用 Rust + GPUI Kit 复刻当前稳定版 Razer Synapse UI。

从 [文档索引](docs/README.md) 和 [实施路线](docs/re/ui-readonly-first-roadmap.md) 开始。界面、交互和资源以当前官方源码为依据，不凭相近产品猜测。

当前 331 个产品、1419 个主导航页均有部分内容；完整复刻验收的产品仍为 0。独立窗口、弹层和条件分支不包含在这些数量中，详见 [覆盖统计](docs/re/native-product-coverage.md)。

UI 编辑、增删、应用、保存流程和本地草稿现在完成；DLL 查询读取现在核实并接入，只有通过 DLL 修改设备或服务状态、写回和保存留待后续统一接入。详见 [DLL 只读盘点](docs/re/dll-readonly-inventory.md)。

当前 Dashboard 源码为 `.ref/applications/synapse/dashboard/`，宿主为 `.ref/host-4.0.827/`。只允许格式化、静态解析、资源校验及 `cargo check --locked --all-targets`；不运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。完整约束见 [AGENTS.md](AGENTS.md)。
