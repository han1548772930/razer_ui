# Local-first product scope

- The product must work without an account or Internet connection. Implement all local device configuration, host/background services, native libraries and plugins, storage, OS integration, local installation and lifecycle paths. See `docs/re/local-first-scope.md`.
- Implement device discovery, direct communication, queries, state observation, mutations, device/service write-back, profiles, mappings/macros, lighting, audio/camera settings, local presets, persistence, errors, cancellation and cleanup. Local firmware transfer from a local file and local DLL replacement/write-back remain in scope.
- Remove accounts/login, cloud identity, cloud synchronization/storage, online catalogs/content, remote downloads, remote update orchestration and other Internet-dependent features from product UI and dispatch paths. Do not retain disabled placeholders or fabricate successful operations.
- Local IPC, loopback and sockets communicating with installed local services remain in scope. Classify features by their actual call chains: Cloud EQ local preset storage, DSP/HID writes and local restore are required; remote catalog retrieval and cloud sync are excluded.
- Keep shared Rust protocol logic independent of the OS. Isolate platform-specific HID, COM, driver and service adapters; unsupported capabilities must be explicit. External projects provide architecture references only. Razer protocol evidence must come from current Razer source and binaries.
- Distinguish local draft persistence, simulated transport results and device/service write-back. Do not fabricate successful device reads, writes or saves.

# Full UI implementation

- Implement each local UI page completely from current original JS/CSS and resources. Follow `docs/re/ui-readonly-first-roadmap.md`.
- Trace the mounted component tree and all product/version/accessory branches. Match layout, sizes, typography, colors, images/icons, wording, tabs, panels, drawers, dialogs, tooltips and keyboard/focus behavior, including normal, selected, disabled, loading, empty and error states.
- Connect every action, including edit/add/delete and Apply/Save/Cancel, through caller, parameters, transport/service, response, state refresh, confirmation, cancellation, persistence and cleanup.
- Preserve original optimistic updates, failure and confirmation semantics. Do not invent dialogs, rollback or getter-to-UI updates without a current-source call chain. Keep additional Rust diagnostics separate from source UI state.
- Record source acquisition, reverse-engineered semantics, Rust implementation, UI/backend connection, static/mock verification and runtime acceptance separately. Partial implementation must list remaining items. A page is complete only when its entire source-to-Rust/UI/backend checklist is satisfied; routes, descriptors, wrappers and subsets of controls are insufficient.
- Keep missing source, unresolved indirect calls, ABI/protocol branches and absent implementations explicit. Do not label them completed or permanently deferred.
- 未实现的功能必须继续实现，不得仅登记缺口后留空或结束对应任务。缺项清单用于驱动后续实现，不是交付结果；必须沿当前源码继续完成具体逻辑、平台适配、UI／后端接线、持久化、取消和清理。
- 不得用 TODO、空函数、占位接口、恒定返回“不支持／未连接”或仅有 mock 的实现代替范围内功能。尚缺证据时继续获取和静态逆向，查清实际调用链后实现；不得杜撰协议、界面行为或伪造成功。仅 AGENTS.md 已明确排除的联网功能可以移除；受执行限制禁止的真实运行验收须如实保持未验证。

# Configuration and typed data

- 按完整工作区整理架构，覆盖根程序、全部 crate、目标平台分支、模块依赖、生成器、测试夹具及安装生命周期；维护入口为 `tools/workspace.py organize-sources`。目录规划必须核对所有模块／嵌入引用，记录全工作区清单，不以只整理一个产品族或几个大文件代替全量检查。
- 依赖朝稳定的下层流动：根程序和 shell 组合功能，页面负责交互与页面状态，领域类型、存储、IPC、协议及系统适配各有明确所有者。共享协议和存储核心不得依赖页面或 GPUI；公共能力经类型、命令、事件和访问器提供，禁止跨 crate `include!` 私有实现。
- crate 内按能力组织文件，视图、状态、命令、服务、取消／清理和测试归同一功能；不建立全局 `data/`、`views/`、`models/` 等杂物目录。小型 crate 保持简单，只有独立状态、生命周期或稳定共享边界时才拆 crate。
- 静态目录使用普通有类型函数读取，协议固定表使用普通构造函数或常量；不得生成 `xxx_data!()` 包装宏、巨量字面量分区或空的数据注册模块。生成器必须同步维护所有者、调用方、清理路径和完整验证，不能重新生成已移除的旧布局。
- 页面 entity、任务、订阅、焦点和滚动状态由实际生命周期所有者保留；render 只读取状态和组合界面。设备／连接切换时取消在途工作，拒绝旧代次结果；后台停止须完成已核实的恢复和释放链，不能用伪造成功或直接退出代替。
- 文档描述本项目的规则、现有实现与验证边界，不写外部参考项目名称；参考原件的完整来源、固定版本、实际字节和适用范围单独留在分析获取收据中。

- 集中管理路径与资源来源：内置数据文件、安装目录、运行时资源目录、缓存目录、资源仓库及版本须有明确的统一入口。业务模块引用命名常量、类型或访问器，不在调用点散布 `include_str!`／`include_bytes!` 相对路径、拼接安装路径或重复远端地址。编译期内置数据与可变运行时配置分别建模，避免任意文件路径替代已核实的协议数据。
- 稳定的设备能力、协议参数、设置、命令、响应与持久化结构使用 Rust `struct`／`enum` 和具名字段；解析时校验必需字段、值域、版本与产品分支。业务逻辑不得大量使用 `serde_json::Value` 的字符串下标并依靠 `null` 或 `unwrap_or_default()` 掩盖缺字段、错误类型和未观测状态。
- 随程序版本固定的能力表、配对规则、协议映射、初始化值及恢复默认值，直接使用有类型的 Rust 常量、静态表、构造函数或 `Default` 实现；默认值必须有当前源码依据。不得只把 JSON 移到 crate 的 `src/data/`、包装成字符串常量或在启动时反序列化，就视为完成类型化。由维护工具生成 Rust 表时，生成器须校验完整字段及产品分支，原始证据保留在分析输入或维护来源中，不成为产品运行时依赖。
- 固定数据完成 Rust 转换、调用接线及等价性验证后，删除 `crates` 内对应原 JSON 和失效读取路径，避免双份生产数据。确需保留的原始证据存放统一分析输入目录；测试夹具只保留验证具体行为所需的内容，不把整份生产 JSON 搬进 `testdata/` 作为替代清理。
- 原始 JSON 仅在明确的文件／通信反序列化、版本迁移及动态扩展边界使用；通过有类型的转换／访问器进入业务逻辑。只有当前调用链确实要求读取、导出或写回某种厂商文件／本地服务报文时，才保留该外部格式及已核实的扩展字段；不得把“兼容原版文件格式”泛化为内部数据继续使用 JSON 的理由，也不得在没有实际交互需求时复制厂商内部存储格式。
- 用户实际配置与固定默认值分别建模：内存中使用有类型的领域对象，磁盘文件在读写边界序列化／反序列化；本程序自己的持久化格式按实际需求设计，维护版本迁移、扩展字段保留、冲突检测及失败语义。不得为消除 JSON 而丢字段、杜撰默认值或改变已核实协议。修改内存对象或本地文件不等于设备／服务已写入成功。
- 字段、命令、资源及产品身份采用稳定的领域类型或具名常量；禁止把文案、列表下标或分散的字符串当作持久化身份。集中入口应有清晰职责，避免把所有业务状态堆入一个全局配置对象。
- 代码注释只解释当前行为、协议依据、安全约束和必要设计原因；不保留“修正记录”、修改日期、开发过程、自述或历史前后对比。验证状态、缺项及变更过程写入现有的统一工作流记录，必要的来源引用、生成器标识和安全说明保留。

# Installation and runtime resources

- 静态界面词表、图像选择／几何元数据及资源索引使用随程序交付的本地 SQLite 目录库，由统一维护入口校验完整字段、产品分支、顺序和重复项并生成；业务通过有类型的普通函数读取。数据库以独立文件随安装交付，禁止 `include_bytes!` 内嵌数据库；在安装缓存内原子发布只读版本，不依赖网络、开发目录或分析输入。迁移验证通过后删除旧生产词表、读取宏、数据分区、索引副本和空注册模块；设备能力、协议表、配对规则、初始化值和恢复默认值仍按上述规则使用有类型的 Rust 数据。
- `crates/razer-resource-db/installation.json` 是目录库来源、安装文件名及资源缓存相对目录的唯一配置；Cargo 准备、运行时定位和安装器共用。Cargo 只生成小型安装常量并复制独立目录库到输出目录，不下载资源或把目录库编入程序。缺少目录库且没有有效缓存时明确失败，不恢复到开发目录，不静默返回空数据。
- 必需的 9 个原生 RGBA 图标集中由 `razer-assets::NativeIcon` 提供；托盘、菜单和资源加载器复用同一字节入口，不在业务模块重复嵌入。其余图片、字体和视频按实际资源请求进入缓存。
- 面向用户交付安装包；安装目录、版本、快捷方式、卸载及资源清理由同一安装生命周期配置维护。普通权限运行必须能够写入其资源缓存；不以要求管理员权限规避目录选择错误。
- 用户明确授权的静态资源下载是本地优先范围的资源交付例外：应用运行时从配置的资源仓库按当前产品、当前实际用到的文件加载，禁止启动时全量下载／解压或将资源恢复到源码仓库 `assets/`。账户、云同步、在线目录及其他已排除的联网业务仍不在范围内。
- 已获取资源缓存在程序安装目录的统一资源缓存位置，供再次启动和离线复用；程序不依赖开发机、分析报告或临时目录。资源索引和安装必需的小型资源保留在代码仓库，图片／视频等载荷不内嵌为整套程序镜像，也不要求整套本地 pack 才能启动或编译。
- 资源流向固定为“私有资源仓库 → 按需获取 → 安装目录缓存 → 当前产品界面”，有效缓存供后续启动及离线复用；仅请求当前界面实际需要的文件及共享依赖，缓存命中时不请求网络。固定协议表、配对规则和恢复默认值随程序交付，不放入远程图片／视频资源仓库作为本地设备功能的运行前提。
- 加载任务由明确的生命周期所有者管理：异步执行、同一文件请求去重、有界并发、长度与内容校验、原子发布，以及真实的加载／失败／重试／取消和清理。缓存命中不联网；未请求的其他产品不预取。资源失败不得伪装为空图、成功或设备已连接。
- 私有资源访问凭据由本机凭据存储或显式配置提供，不写入源码、资源清单、安装包或日志。资源更新保留可用缓存；卸载清理范围限定为本程序安装及缓存目录，并明确保留用户配置的语义。

# GPUI Kit implementation

- Before GPUI changes, read `.agents/skills/gpui-kit/SKILL.md` and `.agents/skills/gpui-kit-design-guides/SKILL.md`, including applicable Coding Guides, Design Guides and mechanism/component references.
- Confirm API signatures against the locked GPUI Kit version. Keep retained state, subscriptions, tasks, focus, stable domain IDs, scrolling and side effects with their proper owners.
- Use semantic components for behavior and product-owned theme/scale tokens for source-verified presentation. Match the original Razer pages; source-derived exceptions to guide defaults need current-source citations.
- Apply the execution limits below to skill verification recipes. Real-window/application/device acceptance remains unverified.

# Current source

- Use the latest official stable Razer Synapse sources. Dashboard originals and their deployment-stability receipt are under `.analysis/inputs/frontend/synapse-dashboard-current/`; 124 referenced source maps are unavailable. Complete current host and device source sets still require acquisition and a version audit.
- Retained originals are under `.analysis/inputs/`, indexed by `.analysis/inputs.json`. Verify acquisition provenance, current version, manifests, byte identity and dependency/resource closure before using them as implementation evidence.
- Do not recreate, read or use `.ref/frontend/`, `.ref/synapse-asar/`, `.ref/host-4.0.821/` or `.work/latest-source-check/host-4.0.821/` as implementation evidence.
- Use maintained static extraction tools in `tools/`. Do not use historical `.ref/tools/` scripts.
- Analysis results must identify complete original inputs, tool versions, full source/function bodies, failures and unresolved chains.

# Analysis workflow and layout

- Use installed `E:/IDA` (IDA Pro 9.1) or `D:/ida/IDA Pro 8.3` and Hex-Rays for native static analysis. Analyze verified private input copies; retain byte hashes, image base, RVA/function boundaries, complete pseudocode/disassembly, types and cross references. Inspect current JS/CSS directly for page behavior.
- Record analyzer versions, capabilities and limitations in `docs/re/tooling.md`.
- Maintain one comprehensive workflow through `tools/workspace.py`. Capture all pages, branches, JS/CSS/resources, host/background services, native libraries, local IPC/storage/OS integration and lifecycle paths. Reuse complete captures for queries; supplement explicit unresolved items.
- Keep current workflow documents in `docs/`, maintained tools in `tools/`, original inputs/references/results in `.analysis/`, and disposable scratch in `.work/`. IDA MCP configuration and plugins belong under `tools/ida/`. Keep analysis tools separate from product runtime dependencies.
- Keep missing inputs, unresolved calls and unimplemented local chains in `.analysis/results/gaps.json`. Do not create a separate script or progress document for each detail, or retain historical archives.
- Build/test fixtures belong to the owning crate's `testdata/`. Builds and tests must not depend on scratch paths or analysis reports.
- Compute evidence SHA-256 from actual input bytes and store it in evidence manifests. Use project-relative paths. Do not use workstation paths or source/code evidence hashes as runtime compatibility gates. Preserve protocol-defined hashes, checksums and versions.

# Execution limits

- Allow static analyzers, source parsing, resource preparation/validation, formatting, `cargo check --locked --all-targets`, pure Rust unit tests and simulated transport tests with `cargo test`.
- Do not run the application, standalone builds for runtime acceptance, vendor JavaScript, target DLLs/exports, helpers or installers. Do not invoke debugger execution or perform real vendor-service/device operations.
- Downloading official source/binary evidence for static analysis is permitted acquisition work. It does not add Internet-dependent product features to the implementation scope.
