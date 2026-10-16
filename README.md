# razer_ui

使用 Rust + GPUI Kit 实现当前稳定版 Razer Synapse 的本地设备配置功能。设备读写、配置文件、映射与宏、灯光、音频、摄像头、本地服务、持久化和系统集成均在范围内；完整页面与设备链仍在实现，不能将产品列表覆盖视为功能完成。

静态图片、字体和视频存放在私有 Hugging Face Dataset `han1548772930/razer-ui-assets`，按产品和共享资源保留原文件，不需要 ZIP 或整套 pack。程序只请求当前界面实际使用的文件，在当前 EXE 所在目录的 `assets/cache/` 校验并缓存；有效缓存不请求网络，支持下次启动及离线复用。尚未缓存的资源离线时会保留真实加载失败状态。静态资源获取是用户授权的交付例外，账户、云同步、在线目录和远程更新等业务仍不在范围内。

源码 `assets/` 保留独立 SQLite 目录库、安装／托盘／菜单必需的 9 个 RGBA 图标，以及 4 个固定窗口控制 SVG，其余载荷由 `.gitignore` 排除。`tools/asset-release.json` 固定资源仓库提交及每个文件的长度、SHA-256 和产品依赖；`assets/synapse/resource-catalog.sqlite3` 保存完整表情词表、分组、变体、版本过滤及资源索引，以独立文件随安装交付，不内嵌 EXE；`crates/razer-resource-db/installation.json` 统一维护文件名与缓存目录。启动在当前安装的 `assets/cache/catalogs/` 原子发布按内容版本区分的只读数据库，缺失或损坏可从 EXE 旁的独立目录库文件离线恢复；业务读入有类型的快照，render 不执行 SQL。Cargo 检查不下载资源、不恢复媒体，也不要求 `synapse.pack`。资源仓库使用独立 Git LFS；主仓库其余 JSON 的 LFS 规则见 `.gitattributes`，克隆后使用 `git lfs install --local` 和 `git lfs pull` 获取被跟踪的维护文件。

私有资源凭据从本机凭据存储、`hf auth login` 的本地令牌文件或显式 `HF_TOKEN` 读取，不进入源码、资源索引或安装包。资源请求异步执行、按文件去重、限制并发并校验内容，失败可重试。完整流程与保留的独立容器格式见 [资源交付与容器格式](docs/asset-pack-format.md)。

用户分发入口是 [Windows 安装包](docs/windows-installation.md)，默认安装到当前用户可写目录；升级保留资源缓存，卸载可选择保留或清理缓存。安装脚本与静态准备工具已提供；当前没有生成或运行安装包，仍缺 release EXE 与 ISCC 输入。

固定设备能力、协议映射、配对与恢复默认值使用有类型的普通 Rust 构造函数或常量；表情、界面目录、图片选择／几何元数据和资源索引使用上述本地数据库，通过领域所有者的普通有类型函数读取。维护入口为 `python tools/workspace.py prepare-typed-data --check` 和 `prepare-resource-database --check`，专用协议／存储表命令见 [tools](tools/README.md)。原始维护 JSON 保留在分析输入，产品不依赖这些 JSON 或分析目录。资源映射维护清单位于 `tools/resource-mappings.json`，替代旧 Rust 片段，维护时校验完整字段与原始顺序。用户配置在明确的持久化边界序列化，不把本地保存视为设备写入成功。

从 [文档索引](docs/README.md)、[本地优先范围](docs/re/local-first-scope.md) 和 [完整实现路线](docs/re/ui-readonly-first-roadmap.md) 开始。UI 必须依据当前原始 JS/CSS 和资源，完整追踪挂载组件、产品分支、控件与状态，以及提交、响应、写回、取消、保存和清理链。

工作区按 UI、状态、本地 IPC、服务、协议与平台适配分层，见 [架构说明](docs/re/architecture.md)。外部项目用于学习安装、目录、缓存与生命周期等架构；Razer 协议依据当前 Razer 原始源码与二进制。证据 SHA 从实际输入字节计算，存于分析索引，不能作为产品运行门槛。

设置中的“所有产品”列出 331 个已注册产品，支持搜索、分类、版本及布局选择，并进入正式产品工作区，而非产品演示弹窗。获取 locator 仍保留 332 个 PID；162 端点的完整 DeviceInfo 实际声明 163，身份错误项不作为额外产品展示。真实连接与未连接的本地预览明确分开；列表覆盖不代表全部页面及设备链已完成。

`prepare-runtime-assets`／`verify-runtime-assets` 从 `tools/resource-mappings.json` 和已固定的资源清单生成／核验同一 SQLite 资源目录，无需本地图片载荷。`fetch-asset-release --product PID` 仅供维护人员明确恢复选定产品原文件，不属于启动或编译流程；普通使用无需执行。来源获取、静态逆向、Rust 实现、接线、纯测试与真实验收分别记录。

[tools](tools/README.md) 保存维护入口和分析器配置，前端静态恢复使用 Wakaru，IDA 路径为 `E:/IDA`（9.1）或 `D:/ida/IDA Pro 8.3`（8.3）。实际版本与限制见 [工具说明](docs/re/tooling.md)。[.analysis](.analysis/README.md) 保存原始输入、外部参考和分析结果；测试样本归所属 crate 的 `testdata/`。

检查与测试必须对应当前文件字节；旧 [验证记录](.analysis/validation.json) 在代码／输入变化后不能沿用为通过结论。`tools/workspace.py audit-inputs` 完整列出缺失原件和不匹配样本；静态／纯 Rust／模拟验证按完整功能批次进行，页面完整性与真实运行验收仍未验证。

允许静态分析、格式化、`cargo check --locked --all-targets`、纯 Rust 和模拟 transport 测试。禁止运行应用、厂商 JavaScript、目标 DLL、helper、installer 或操作真实设备。完整规则见 [AGENTS.md](AGENTS.md)。
