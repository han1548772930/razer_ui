# 架构与证据身份

工作区将 UI、状态、服务和设备协议分层。UI 使用源代码核对的控件和交互；共享 Rust 协议逻辑独立于操作系统；HID、COM、驱动及本地服务适配归平台层。跨平台缺失能力必须明确返回缺失，不能生成成功的设备结果。

| 层 | 现有 crate | 责任 |
| --- | --- | --- |
| 应用与页面 | `razer-app`、`razer-shell`、`razer-pages`、`razer-app-pages`、`razer-dashboard`、`razer-settings`、`razer-tray` | 窗口、页面挂载与本地动作协调；不直接承担设备协议 |
| 表达与本地状态 | `razer-widgets`、`razer-assets`、`razer-i18n`、`razer-model`、`razer-state`、`razer-storage`、`razer-catalog` | 来源资源、类型、状态及本地持久化；草稿与设备状态区分 |
| 本地通信与服务 | `razer-ipc`、`razer-agent`、`razer-service` | 类型化请求／响应、后台操作、生命周期、取消和失败 |
| 协议与系统适配 | `razer-device`、`razer-hid`、`razer-discovery`、`razer-platform` | 协议编解码、身份与能力、transport、设备发现及平台集成 |

这些分层和代码存在不代表全程序实现已完成。具体本地链与未解项按 [完整实现路线](ui-readonly-first-roadmap.md) 核对，Razer 协议证据始终来自当前 Razer 输入。

crate 内按功能组织源码、相关测试和配套数据。`razer-device` 使用 `mouse/`、`keyboard/`、`audio/`、`camera/`、`gamepad/`、`receiver/`、`mapping/`、`transport/` 等目录；`razer-pages` 在产品族内继续按 DPI、映射、校准、灯光、混音等功能归组。`razer-app-pages` 按页面功能归组，shell 的协调逻辑与服务的平台适配也保留各自的功能目录。小而职责集中的 crate 保持简单结构。

物理目录通过显式模块路径保持已有 Rust 模块接口和状态所有权；移动文件不改变设备协议、UI 行为或数据格式。可执行的迁移规划和完整模块／嵌入文件引用检查统一在 `python tools/workspace.py organize-sources`，`--apply` 执行迁移并记录输入、输出摘要和路径映射。测试 fixture 仍归所属 crate 的 `testdata/`；原始采集证据与旧分析结果不随源码搬移改写。目录调整后必须重新做全目标静态编译和适用的纯／模拟测试。

## 启动与按需加载

`razer-app` 只读取、解析一次 workspace 文件，将同一结果用于语言选择和 `AppShell` 恢复。恢复时先逐设备建立页面 owner 与订阅，随后一次同步跨设备列表；运行期间新增设备仍立即同步。读取失败作为同一错误传递，不将第二次读取的不同快照混入启动状态。

设置的两个入口仅在实际打开“所有产品”时创建 `ProductsView`，后续保留该 entity、过滤条件、选择与滚动状态。语言变更更新已创建的目录；尚未创建的目录使用打开时的语言。导航负责创建与订阅，render 不执行这些副作用。

六族 `source_product` 通过 `SourceCatalog` 索引原始静态 JSON 中的产品 ID，仅在请求该 ID 时解析完整规格，并在所属 `OnceLock` 保留稳定引用。未知 ID 不解析其他产品。原数据、重复 ID 的首条选择语义和调用方接口不变；纯测试核对六族每条原始记录、真实 typed lookup、重复 ID、未知 ID 和引用稳定性。该优化减少首次窗口期间的解析与分配，但未执行应用计时，不能据此给出实际启动秒数或将延迟全部归因于配置文件。

## OpenLogi 源码参考

旧文档记录 2026-10-13 参考提交 `74d243fcf1a86007df9401499bff3716663d54d8`；2026-10-15 本轮盘点未找到 `.analysis/references/OpenLogi-current/acquisition.json`，因此不能沿用 37 个文件已完整保留的结论。以下列出原参考的定位和设计边界，需恢复固定提交原件并核对获取收据与字节身份后再作源码证据使用。OpenLogi 仅作架构参考，不使用其 HID++／UVC 实现推断 Razer 协议，也未运行 OpenLogi。

| 待恢复的参考源码 | 项目设计边界 |
| --- | --- |
| `openlogi-ipc/src/ipc.rs`：类型化 Agent 服务与独立协议版本 | 本地通信以明确的请求、返回和协议版本表达；Razer 的设备能力与 wire protocol 要独立从当前 Razer 源码验证 |
| `openlogi-ipc/src/client.rs`：handshake 与 ProtocolSkew | 以通信契约判断兼容性，不把开发机路径或分析源码 SHA 当设备可用条件 |
| `openlogi-ipc/src/client.rs`：Observer／Ledger 的连接所有权 | 观察代次与在途请求随连接销毁，忽略陈旧应答，避免旧服务响应覆盖新状态。Razer UI 的 getter／乐观更新规则仍以实际源码链为准 |
| `openlogi-ipc/src/transport.rs`：跨平台本地 transport | Unix socket／Windows named pipe 属于本地通信；平台端点与帧边界集中管理，本地 IPC 不应按联网功能删除 |
| `openlogi-agent/src/lifecycle.rs` 与 `shutdown.rs` | 启动、等待需求、获得设备资源、运行、停止由单一生命周期所有者协调；托盘／进程替换只发请求，释放设备占用与 hook 后结束，不能把销毁藏在页面按钮里 |
| `openlogi-agent/src/binary_watch.rs` | 自身程序替换／卸载观察与逆向原始证据校验是不同职责；本地 DLL 替换仍应有明确取消、释放、写回与重启链 |

这些设计边界不能替代当前源码证据。当前项目仍需逐条实现并验证设备发现、能力报告、本地服务、状态观察、取消和资源清理，不能仅凭 crate 分层或引用 OpenLogi 标成完成。参考项目的在线更新、账号外链等功能不会因参考其架构而进入本产品范围。

## 哈希与迁移

证据 SHA-256 用于回答“这份分析针对哪些原始字节”。它由工具计算并写入 [输入索引](../../.analysis/inputs.json) 或新分析结果；源版本变化就重新分析对应输入，Rust 代码变化就重新验证对应代码。不要把证据 SHA 常量放进产品运行兼容性条件。

同一字节在任何电脑上 SHA-256 都一样。Git 换行转换、重新压缩、包版本变化或修改源码会改变字节，因此哈希也会改变。验证原始输入时保留原始字节，不能归一化后再宣称原始 SHA 相同。

路径使用 checkout 相对路径，工具从项目根解析。复制项目及原始输入到另一台电脑后可重新计算并校验；如果只拉取 Git，被忽略的输入需要另行复制或重新获取。协议内定义的校验和、哈希和协议版本属于协议逻辑，必须保留。
