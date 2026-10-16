# razer_ui

资源使用 Git LFS：`assets/` 的资源文件及全仓 JSON（包括配置、词典和测试数据）由 LFS 管理；Rust 资源索引源码及当前原生托盘／菜单直接嵌入的 9 个图标保留普通 Git。克隆后先执行 `git lfs install --local` 和 `git lfs pull` 再构建；这只是开发资源获取，不是产品运行时联网功能。生成的 `target/asset-pack/synapse.pack` 不提交，也不上传到 HF 原文件资源仓库，避免与源资源重复存储。LFS 配置不改写已有提交；仅删除并重建远程仓库不会消除本地旧历史中的大文件。

资源按产品 PID 归档到 `assets/synapse/products/<PID>/`，产品配置在其 `configs/` 下；跨产品共用资源只保存一份，由每个产品的 `resources.json` 引用。顶层仅保留 `products/`、`shared/`、`tables/`、`manifests/`、`indexes/`，分别存放产品资源、共用资源、Rust 资源表、来源清单和资源索引。来源尚未核实的内容明确标记，不冒充已验证的产品资源。资源去重以实际字节一致和全部引用同步为前提，不按文件名或图片外观删除，也不修改原始采集证据。维护命令为 `python tools/workspace.py deduplicate-resources`，复核报告后再加 `--apply`；`python tools/workspace.py organize-resources --apply` 归档支持文件并同步引用。搬移后需重新生成并验证资源包。

使用 Rust + GPUI Kit 完整实现当前稳定版 Razer Synapse 的本地设备配置功能。产品无需账户或互联网连接；设备读写、配置文件、映射与宏、灯光、音频、摄像头、本地服务、持久化和系统集成均在范围内。

当前实现仍有缺项。本轮已从保留证据恢复输入索引并重新进行 [实际字节审计](.analysis/results/input-audit.json)，仍有缺失与身份不符。2026-10-15 已复核宿主 4.0.827 获取收据的 36 个文件，实际长度与 SHA-256 全部匹配；本机主 EXE、ASAR 与三份核对的 DLL／NODE 也与保留原件一致。完整源码闭包、页面和设备验收仍未完成。

从 [文档索引](docs/README.md)、[本地优先范围](docs/re/local-first-scope.md) 和 [完整实现路线](docs/re/ui-readonly-first-roadmap.md) 开始。UI 必须依据当前原始 JS/CSS 和资源，完整追踪挂载组件、产品分支、控件与状态，以及提交、响应、写回、取消、保存和清理链。

工作区按 UI、状态、本地 IPC、服务、协议与平台适配分层，见 [架构与 OpenLogi 参考](docs/re/architecture.md)。OpenLogi 只提供架构参考；Razer 协议需要 Razer 原始源码与二进制证据。证据 SHA 从实际输入字节计算，存于分析索引，不能作为产品运行门槛。

设置中的“所有产品”列出 331 个已注册产品，支持搜索、分类、版本及布局选择，并进入正式产品工作区，而非产品演示弹窗。获取 locator 仍保留 332 个 PID；162 端点的完整 DeviceInfo 实际声明 163，身份错误项不作为额外产品展示。真实连接与未连接的本地预览明确分开；列表覆盖不代表全部页面及设备链已完成。

产品图片从可执行文件旁的 `assets/synapse.pack` 按需读取并校验，不再全部内嵌进 PE。维护时依次运行 `python tools/workspace.py audit-asset-pack --prepare`、`python tools/workspace.py prepare-asset-pack` 和 `python tools/workspace.py verify-asset-pack`，生成文件放在 `target/asset-pack/synapse.pack`；Cargo 的资源构建脚本将该包复制到对应 profile 的 `assets/`。分发时必须同时携带此包，缺失、损坏或资源键集合不匹配会明确报错。当前格式只做字节级去重，尚未实施图片重编码、JSON 二进制化、zstd 或安装器压缩；见 [资源包格式](docs/asset-pack-format.md)。

[tools](tools/README.md) 保存维护入口和分析器配置，前端静态恢复使用 Wakaru，IDA 路径为 `E:/IDA`（9.1）或 `D:/ida/IDA Pro 8.3`（8.3）。实际版本与限制见 [工具说明](docs/re/tooling.md)。[.analysis](.analysis/README.md) 保存原始输入、外部参考和分析结果；测试样本归所属 crate 的 `testdata/`。

检查与测试必须对应当前文件字节；旧 [验证记录](.analysis/validation.json) 在代码／输入变化后不能沿用为通过结论。`tools/workspace.py audit-inputs` 完整列出缺失原件和不匹配样本；静态／纯 Rust／模拟验证按完整功能批次进行，页面完整性与真实运行验收仍未验证。

允许静态分析、格式化、`cargo check --locked --all-targets`、纯 Rust 和模拟 transport 测试。禁止运行应用、厂商 JavaScript、目标 DLL、helper、installer 或操作真实设备。完整规则见 [AGENTS.md](AGENTS.md)。
