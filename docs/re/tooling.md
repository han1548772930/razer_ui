# 静态逆向工具与核验边界

IDA 路径：`E:/IDA`（9.1）、`D:/ida/IDA Pro 8.3`（8.3）。GPUI 技能读取 `.agents/skills/`。各份捕获记录实际工具版本，不能跨版本沿用验证结论。

批量原生捕获的 `--workers` 支持 1–4，默认 1。线程池负责调度，每个新模块运行独立 IDA 进程，私有数据库与输出按原件 SHA 隔离；相同字节只排一次。并行批次的共享配置在首次需要新捕获时加锁准备一次，由协调线程按稳定模块顺序发布统一收据，单项失败保留且不阻止其余项。已核验的完整捕获直接复用；全为复用时不启动 IDA 或准备配置。`test_evidence.CaptureStreamingTests` 的 17 项测试使用事件屏障核对实际重叠、上限、失败隔离和稳定发布，并覆盖全复用及非法并发参数，未以睡眠或放宽断言替代顺序检查。

当前 `HID.node`（SHA-256 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`、405,704 字节）的 [实际捕获收据](../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/capture.json) 使用 IDA 9.1、Hex-Rays 9.1.0.250226、Python 3.11.9，完整捕获 1,641 个函数；RVA `0x30888` 的反编译失败及完整汇编保留。其他 IDA 8.3 捕获的函数数与产物摘要不能套用到这份收据。[HID 查询](../../.analysis/results/native/chain-audit.json) 中的工厂、回调、动态导出与共享 worker 链须分别对照其引用的完整函数及实际产物身份；指定查询没有未解边不代表全部 N-API／Promise、产品协议或生命周期已经闭合。当前 GetFeature／GetInput 的完整函数分别位于 RVA `0x17ac0..0x17c43`、`0x17c50..0x17dd3`，Windows 返回长度均为原生已读长度加一；Rust 的 source-host collection 适配与产品重试／取消仍须逐调用方校验。

`capture-corpus --source retained --ida-root E:/IDA --include-path '\.(dll|node|sys)$' --workers 4 --no-refresh-gaps` 本批 884 个不同字节的非 EXE 模块已终态：869 份经校验复用、15 份新捕获，共 2,610,555 个函数，模块级失败为零。[统一原生队列](../../.analysis/results/native-corpus-capture.json)与[独立质量核验](../../.analysis/results/native-corpus-quality.json)分别保留输入、版本、捕获和复核身份；发布前另重读全部 884 份原件及捕获清单摘要。181 份旧 schema 缺少可选完成字段，按完整且稳定的函数清单核验，不补造该字段。

241 个模块合计 869 个函数没有成功生成伪代码；全部失败函数的原始块和 1,622,453 条已枚举汇编项已核对原 PE，共 10,014,652 字节 raw chunks。152 个失败函数的汇编项没有覆盖其中 7,185 字节，这些原始字节仍完整保留。队列另外排除 1,066 份输入，其中 995 份 EXE；它们不计入本批完成数，已有独立捕获的输入须按各自收据另核，不能全部当作尚未捕获。额外服务 EXE 也由单独捕获收据记录。因此 `capture_complete=true` 与 `full_corpus_captured=false` 同时成立，不能将本批终态记为全语料、功能语义或实际 DLL 对照完成。

另有十份服务 EXE 已逐项完成完整产物复核：1442／3942 路由服务，1427／709–711／1398 属性服务，以及 1443／1411／1339／694–696／1330 的 companion 服务。十份共核对 182,605 个函数文件及 90 个输出的实际摘要，原件与 capture 前后不变；其中 1427、709–711、1398 各有七个 Hex-Rays 未决，其余七份为零。各自的 [native 捕获目录](../../.analysis/results/native/)保留终态、版本、函数和失败身份；这些额外收据不改写上述 884 模块队列，也不提升为整个服务的语义或运行验收。

随后十三份服务／本机工具又完成 158,625 个函数文件及 117 个输出的实际摘要复核，来源清单、资源声明与原件重新核字节。当前追加重点共 23 份已核验终态；合计 341,230 个函数文件、207 个输出，Hex-Rays 未决函数 1,408 个（仅表示反编译失败，原始汇编与 chunks 仍按收据保留），这组数量独立于 884 模块队列。

| 追加原件 | 完整函数文件 | Hex-Rays 未决函数 |
| --- | ---: | ---: |
| 4126／4124 两份属性服务 | 72,416 | 14 |
| 1443／1411／1339 三份空间音频配置工具 | 31,301 | 1,365 |
| 两份 Blade 属性迁移工具 | 3,384 | 2 |
| 显示校准报告工具 | 13,292 | 0 |
| 触控板固件版本读取工具 | 8,918 | 6 |
| 1442 安装自定义动作原件 | 716 | 0 |
| 1337／1372／1383 三份 companion 服务 | 28,598 | 0 |

两份属性服务虽然各有 36,208 个函数和相同失败 RVA，`.text` 等实际 section 字节仍不同，不能合并完整语义。空间配置工具的高失败数照录，函数文件及汇编完整不表示伪代码完成。版本读取工具的六个失败函数另核 10 个 chunks／1,191 字节／357 条指令且无覆盖缺口；安装自定义动作的实际来源是 ZIP → 内层 EXE → CAB → MSI → Binary.CustomActionKey，获取记录中的外层容器摘要不能冒充紧邻 MSI。此处均为静态产物与来源核验，没有执行工具、安装器、服务或设备。

另一个四进程批次补齐了两份 [CVR](../../.analysis/results/native/459b3ce56842911d6a55269228882ca47653fefa3dae25ef134a5c38d5807cf8/capture.json)／[CVR](../../.analysis/results/native/cf3f04f5eee9dba455590c0e5118e24d468dd9ad866faad1a2ba89093281d4ec/capture.json)、[GetPackrat](../../.analysis/results/native/79441d216f4205b2018e517939ede8e59b1093351615a621668206e18a9757bd/capture.json)及[本机保留的提升服务](../../.analysis/results/native/fd7dc9d5de93cfb027eda972967f7b0e6871eeb4f6e1cd337dfd6d186572176c/capture.json)。全部完整函数文件与输出均按摘要复核：函数数分别为 13,292、13,292、4,694、4,182，合计 35,460，另有 36 份输出。提升服务有 5 个 Hex-Rays 失败，另核其 5 个完整 chunks、7,293 原始字节及 1,615 条汇编项；其中 387 字节没有枚举为指令，未补造伪代码或指令覆盖。其余三份反编译失败为零。

两份 CVR 来自当前清单声明的 Blade 工具包 1.0.0.5／1.0.0.8，包内版本均为 1.0.81.0；与已捕获的 1.0.0.9 包副本相比，七个实际 section 的原始字节摘要全部相同，完整 PE 字节仍分别记录。GetPackrat 的实际版本资源为 19.4.13.0，不能按文件名把它归为单纯版本查询器。提升服务仍是独立获取收据绑定的本机 1.1.0.8 副本，完整 IDA 捕获没有证明它等同最新官方部署物。这四份捕获不改写 884 模块队列，也不表示报告、触控板或提升功能已完成语义迁移、实际调用或设备验收。

维护入口统一为 `python tools/workspace.py`。原件位于 `.analysis/inputs/`，获取收据及索引保存实际字节 SHA-256；结果位于 `.analysis/results/`，临时数据库位于 `.work/`。静态分析不执行输入；已授权的本地 DLL／设备测试由显式维护命令记录实际调用、结果与清理，执行边界以根目录规则为准。厂商应用、JavaScript、无关 helper、installer 和调试器仍不运行。

`validate-connected-devices` 使用生产 HID／领域协议所有者，在实际物理 PID、接口和 Feature descriptor 匹配的唯一 collection 上查询；默认忽略的集成测试不会随普通纯测试打开硬件。PID679 先读取实际布局，再对已核实布局 1 的 82 个 firmware key IDs 按源码的 15／13 键分批读取 Actuation／Rapid Trigger，不使用写入专用 ALL_KEYS。`--write-restore` 先核对原值可按字节恢复并写入日志，尝试变更后独立恢复及读回。`compare-connected-hid` 将同一路径、同一事务的 GET 报文交给已核实 x64 C ABI 的当前 `HID.node`，按当前选定工厂的十次、5 ms IN 节奏等待 busy 后的终态，保存每次原始返回，再逐字节比较终态；busy 耗尽、短报文或事务／命令不匹配均不能通过。比较绑定当前未变化的 Rust 输入和原始读取收据字节，分别记录句柄、`hid_exit`、DLL 及进程释放。具体功能结果见 [读取](../../.analysis/results/connected-device-verification.json)、[可恢复写入](../../.analysis/results/connected-device-write-verification.json)与[原库比较](../../.analysis/results/connected-native-comparison.json)；匹配离线应答只证明传输一致，实际设置与窗口验收分别判断。

## 无设备测试与官方 DLL 对照工具

全功能测试与实际 DLL 对照的强制要求由 [验证规则](../../.agents/rules/verification.md) 定义。没有设备时，先在 `FeatureTransport`／服务边界使用脚本式模拟、故障注入和有独立来源的应答回放；模拟通过、官方 DLL 对照及真实设备验收分别记录。

工作区统一锁定 Mockall 0.15.0 和 Proptest 1.12.0，协议 crate `razer-device` 通过 `dev-dependencies` 使用。Mockall 用于接口参数、调用次数和顺序约束；已有手写模拟传输继续复用。Proptest 用于参数、畸形报文及操作序列的属性测试；失败样本应保留以便复现。其他 crate 按所属测试需求引用工作区版本，测试库不进入生产依赖。

从工作区根目录运行已有纯／模拟协议测试：

```powershell
python tools/workspace.py validate-rust --suite current-dpi-protocol --suite current-polling-protocol --suite current-power-protocol
```

官方 DLL 对照维护器使用 Windows x64 Python 标准库 `ctypes`、`unittest` 和项目内 PE 检查模块；不执行 JavaScript／N-API 初始化。其纯测试使用 `python -m unittest discover -s tools -p test_native_runtime.py`，不加载真实官方 DLL 或打开设备。实际功能对照继续使用 `compare-local-service-version`、`compare-local-app-files`、`validate-connected-devices` 与 `compare-connected-hid` 的已核实分支；新增功能须补齐对应入口和收据，不能由现有 HID 重放替代业务比较。

cargo-fuzz 0.13.2 已安装，配套独立 nightly 工具链为 `rustc 1.101.0-nightly (19833af72 2026-10-20)`，包含 `rust-src`；工作区默认工具链仍为 1.99.0。安装与版本核验命令如下，不设置默认工具链或工作区 override：

```powershell
rustup toolchain install nightly --profile minimal --component rust-src
cargo install cargo-fuzz --version 0.13.2 --locked
cargo +nightly fuzz --version
```

Windows x64 的模糊目标构建还需匹配的 MSVC AddressSanitizer 运行库。当前仅完成 CLI／nightly 安装与版本核验，尚未建立、构建或执行 fuzz 目标，完整模糊测试环境与覆盖保持未验证。目标需按协议所有者建立，实际执行继续遵守根目录边界，不能加载官方业务 DLL、访问真实硬件或冒充设备验收。

## 缺失 source map 的静态恢复

`analyze-frontend --recover-missing` 核验完整获取收据、原始字节及已有分析产物后，恢复尚未单独拆出的 webpack 模块。`--all-acquired --workers 4` 对当前已获取前端集合并行处理，逐项结果见 [模块恢复收据](../../.analysis/results/frontend/module-recovery.json)。非当前部署集合与无 JavaScript 的集合明确区分，不据目录存在跳过核验。

当前恢复器按 AST 识别被 Sentry 前导代码与顶层逗号表达式包住的 webpack 注册和 bootstrap。它从已核验的可读整包切出完整 factory，保留 module、exports、require 的实参绑定；没有可读整包时才对原件做一次静态格式化。原件、bootstrap、前导代码及其他非 factory 代码仍完整保留。恢复代码位于各集合的 `modules/factories/<原件摘要>/`，provenance 登记完整 factory 原始区间、参数和可读整包区间。

每个 factory 的 map 从可读整包映射按区间切片并组合回官方原件，列坐标使用 UTF-16，文档引用使用 UTF-8 字节。为独立阅读添加的括号没有原始映射。每份分析保留实际恢复工具副本及其摘要；再次使用时核验原件、所有登记输出和工具副本，不能把后来改动的维护脚本冒充原配方。合成 map 是逆向产物，不能冒充官方发布的 map，也不能恢复发布包已经删除的源码注释和原始名称。

模块结构恢复、原分析器的转换／validate 诊断、逐功能语义理解分别报告。已有失败不改成成功；完整词法 factory 可用于继续逆向，但不能凭模块数量宣称页面、原生协议或产品全部理解完成。

## 当前工具

`compare-local-service-version` 使用当前 IDA／Hex-Rays 完整函数捕获核对 Simple Service 版本查询及 COM 生命周期。维护入口验证实际原件字节、已注册服务架构和签名，保存项目内副本，再显式运行默认忽略的只读对照测试。原件身份在 `.analysis/inputs/installed-current/service-acquisition.json`，实际结果在统一验证记录的 `local_runtime_checks.simple_version`；该项与普通纯／模拟验证独立，不推导其它服务能力通过。应用本身不再提供通用 DLL 版本、getter 或 lighting 探测 CLI。

| 工具 | 当前用途与身份 | 能力限制 |
| --- | --- | --- |
| IDA／Hex-Rays | 对已验证私有副本导出全函数、汇编、伪代码、types、strings、imports、exports、xref | 捕获完整不等于全部反编译成功、ABI 已解析或功能已实现；失败保留完整汇编 |
| IDA MCP | `tools/ida/` 固定 2.0.0 快照，提交 `c133c3853faa111a9b00ee615c013b720d0c4acd` | 本轮全函数使用批处理；配置存在不等于连接验收，不调用执行方法 |
| IDAPython／Python | IDA 8.3 捕获实际使用 Python 3.11.6；9.1 配置保留固定 Python 3.11.9 embed | 与产品依赖分开，按各份捕获核验实际运行时，不跨版本沿用身份 |
| Wakaru 1.14.0 | 全量显式 JS 输入，静态拆包、provenance、输出 map、diagnostics | validate、chunk 枚举和 source map 缺失独立记录；成功退出不证明依赖闭合 |
| Tree-sitter 0.25.2／JavaScript 0.25.0 | 按 Python ABI 使用项目内解析器，静态定位完整源码 AST 和原始字节范围 | ERROR／missing node 不允许当成功提取；禁止执行 JS |
| Rust 固定表生成器 | 项目内 Tree-sitter 0.25.2／Rust grammar 0.24.2，`prepare-static-python` 按实际 Python ABI 恢复；`prepare-typed-data` 按实际 Rust 字段和枚举生成多行构造代码，可按 `--symbol` 定向校验；`tools/typed-data.json` 保留输入 SHA-256、字段投影及格式边界 | 仅解析维护输入；不执行厂商代码。恢复自 Rust 的生产字段投影独立标记，不冒充原始证据 JSON；生成成功不代替编译、行为等价性或设备验收 |
| SQLite 资源目录生成器 | `prepare-resource-database`／`prepare-emoji-search`／`prepare-runtime-assets` 共用 `resource_database.py`；本轮 Python SQLite 3.50.4，schema 4；Rust 使用锁定的 rusqlite 0.40.2／libsqlite3-sys 0.38.2 bundled，整数读取做溢出校验 | 迁移表情、静态界面词表与目录、图片选择／几何元数据及资源索引；不迁移设备协议或用户配置。目录为 EXE 旁独立文件，不内嵌。全字段、顺序、重复项回读核验独立于真实窗口与安装验收 |
| ccl_chromium_reader 0.24 | 固定提交 `ef840de30221c4d65bc96d2f4d9057e9ef2f526d`，8 个文件按收据校验 | 只读取公共官方静态缓存，不读取账户、cookie、浏览历史；缓存不是当前部署证明 |
| 7-Zip | 新提取使用 24.02 x64，已有 26.02 收据保留各自身份；版本及 EXE／DLL SHA 随静态提取记录 | 仅识别为 PE 的安装器不能记成已提取全部载荷 |
| ILSpyCmd 9.1.0.7988 | 固定 NuGet 包与 59 个文件，.NET 8 上静态导出 C# 项目和完整 IL | 不执行输入程序集；保留 unresolved 注释、stderr、两阶段退出码；native C++ 使用 IDA |
| innounp 2.71.2 | 当前官方发布资产与完整源码提交独立固定于 `tools/analyzers/innounp-2.71.2/`；静态列举／提取 Inno Setup | 旧 2.71.1 发布资产已不可获取，原收据的恢复失败保留，不用新版本字节替换旧摘要；重建 install_script.iss 只作为分析产物 |
| PyAV 16.0.1／FFmpeg | `prepare-media-analyzer` 按实际 Python 3.14／Windows x64 ABI 取得官方 `cp314-win_amd64` wheel，核验 SHA-256、全部 RECORD 成员及实际导入版本；获取收据位于 `tools/analyzers/media-python/acquisition.json` | 仅静态读取与转换媒体，不运行厂商代码。原八通道 PCM、时间戳与 JPEG 帧分别核验；派生文件在 `.analysis/inputs/prepared-media/` 独立索引，不冒充官方原件，不以转换通过代替播放验收 |

工具获取收据位于 `tools/analyzers/`；前端分析器准备记录位于 [.analysis/results/analysis-preparation.json](../../.analysis/results/analysis-preparation.json)。`analyze-host` 按既有 ILSpy 收据恢复精确匹配的官方包成员，不改写预期摘要。

`prepare-emoji-search` 对 mouse190、keyboard679、Macro 与 Dashboard 的已保留 JS 原件核对实际字节和获取收据，用静态 AST 读取完整字符串，逐项比较 1,936 条 `(emo, name)` 及顺序。SQLite 保存 1,857 条唯一记录，顺序表保留 79 个重复项；全部分组、tab、变体及排除列表分别回读核对，保留 Macro 与 Mouse 分支差异。初始词表共享迁移的非搜索 Rust token 等价性、最终数据库全字段往返核验、完整 callable owners 和解析器版本存于统一 [固定表记录](../../.analysis/results/typed-data.json)。旧重复生产数据、数据宏、分区与 Rust 资源索引已清理，设备协议表继续使用普通有类型构造函数。`catalog_data.py` 静态核对完整构造 AST，将相同子树存为有界且无环的目录 DAG，并生成领域所有者内的普通解码函数；每张目录具有纯 Rust 完整解码检查。`tools/resource-mappings.json` 合并资源 alias／选择条件并保留完整顺序与重复项，迁移与数据库字节等价收据保存在同一固定表记录。搜索数据相等不代表各页面过滤规则相同，也不提升原件部署稳定性或依赖闭包状态。

`prepare-locale-exports` 使用 Python 3.14.7、Tree-sitter 0.25.2／JavaScript grammar 0.25.0 和 json5 0.12.1，核验获取收据后静态读取当前完整 main 的工厂、语言路由和每个字典 export。解析保留实际 lexical scope，识别已支持的字面量与 alias getter；路由必须唯一，无法静态证明的动态值明确失败，不执行 JS。当前完整提取 48,150 条 export，向 10 个语种各补 6 个缺键；58,527 条原有翻译保留，94 处当前 export 与旧值的差异逐项记录。完整函数体／字节范围、实际来源与 baseline 身份、工具版本及合并策略见[词表准备收据](../../.analysis/results/locale-preparation.json)。生成器逐字段回读 schema 4 的 58,587 条翻译并校验顺序、重复、类型及已移除的旧生产路径；该迁移不补造旧词表尚未闭合的厂商来源，也不代表真实窗口语言呈现已验收。

## 主机与安装证据

[本机获取收据](../../.analysis/inputs/installed-current/acquisition.json) 保存 564 个安装文件、5,093 个公共缓存容器和 5,093 份响应资源，获取失败为 0。六份分组包括 PID679 UI／MW、PID190 UI／MW、Synapse Dashboard 和 Chroma Dashboard，无同 URL 字节冲突。通知、设置、宏、配置文件、灯光引擎等其他缓存也保留在同一收据，需沿实际调用链获取当前官方部署后使用。

服务目录只快照 DLL／EXE／NODE 和明确的 `.deps.json`／`.runtimeconfig.json`。日志、数据库、运行配置和活动数据在读文件前排除。产品本地存储仍通过代码、部署清单、模拟数据完整分析，不将用户活动记录当实现证据。

[官方 host 获取](../../.analysis/inputs/host-current/acquisition.json) 本轮取得 36 个文件；六个部署元数据端点前后实际摘要一致，Synapse／Chroma 稳定部署为 4.0.827。主包长度、MD5 和声明逐项核对，但 `host_package_closure_complete` 仍为 false。

官方包静态解包原件与本机主 EXE、ASAR、mapping_engine.dll、simple_service.dll、HID.node 实际 SHA 一致。ASAR 为 `b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6`，静态提取 10,076 个文件，0 个提取未解项；这不消除源码图、外部资源或 ABI 未解项。

[host JavaScript 捕获](../../.analysis/results/host-current/javascript/capture.json) 保留 230 份 JS／CJS 和 693 个恢复产物。106 个引用 source map 未包含在包中；unpack 退出 0、validate 退出 1。原始文件是语义与字节证据，可读文件和合成 import／export 只辅助查询。

`audit-host-local --query macro-recorder` 从当前 PID192／756 middleware、Macro 和 background-manager 原件保留完整 AST owners 与真实 UTF-8 byte 区间，并复核四套既有 Wakaru 捕获的全部原件、产物和分析器字节。完整捕获中的 validate 非零结果与获取缺失不升级为成功；统一 [local host 查询](../../.analysis/results/host-current/local-audit.json) 保留软件录制与 OTFM 写入／移除、PID756 非独占预触发与全局广播、background OTFM 全局锁与完整状态机、An 初始扫描与 q 事件字段差异、接收器缓存最后匹配、serialNumber 筛选及当前功能注入配置。JSON 记录不会补造 getHWModule；原生 transport、完整页面与真实运行验收仍未闭合。

`audit-host-local --query window-lifecycle` 在同一查询收据保留 25 份字节核实的完整宿主 JS 文件及 117 个去重 AST 所有者。输入逐一与 packed ASAR、官方包成员和获取清单核对；窗口／页签身份、120000 ms 默认休眠与显式休眠、URL 内存存储、Windows 原生事件的 JS 消费者、退出拒绝与超时、已登记 HID 恢复命令及 WebSocket／BLE／日志停止分别记录。BLE endpoint 原件的 797 个函数、9 份汇总产物和 16 个完整行为所有者另经实际字节校验，其 UTF 失败的 7 函数 supplement 保留原失败、类型依据和数据库不变性。该查询为静态操作，不执行 JS 或 DLL，不把异步发送、FreeFFI 返回 true、三秒超时或 app.exit 当作释放成功；`refresh-gaps` 保留其失败、剩余调用链及独立实现／实际对照的未验证状态。具体顺序、源码缺陷和对照边界见[窗口生命周期](../products/host/window-lifecycle.md)。

`audit-host-local --query profiles` 在同一收据新增 [profiles 字段](../../.analysis/results/host-current/local-audit.json#/profiles)，`--verify-only` 只独立复核而不收集或发布。当前两套部署和五份宿主原件保存 43 个完整 AST 所有者、32 个选中调用者、439 个原件及 2,157 个校验工件；完整捕获中的两个失败阶段继续保留。54 项 `test_host` 通过，其中 15 项覆盖本查询的字节、完整边界、绑定、获取变更、19 类失败／取消／完成度篡改与发布保护。正式报告原有十个顶层字段逐项保留 canonical 身份。Add 的 action 吞发布异常、Browse／drop 不等待所有添加、刷新早退、扫描结果未关联、重定位仅写 id/path、拖放监听的清理行为分别见 [Profiles](../products/host/profiles.md)。这些维护工具测试不等于 Rust 功能或真实 DLL 对照；全部 scanner、设备自动配置档写入和窗口仍须逐链验证。

Profiles 的 Windows 原生枚举／图标另复用 [SysUtilsNative 完整捕获](../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/capture.json)，独立核 38 个函数、38 个 chunks、7,525 条指令及 36,131 原始字节，1,078 字节未枚举范围保留；九份汇总输出也按摘要核对。同字节的官方主机包成员、实际版本、导出／导入及注册表根表分别确认。枚举仅查询 HKLM／HKCU 下两种 Uninstall 路径，不补成 UWP／快捷方式扫描；图标缓存键、GDI+／PNG／base64、临时文件、短读和失败释放分别记录。这是所选 Windows 静态链，不能扩写为扫描服务、macOS、Rust 或实际 DLL 已验证。

[GMS 续追](../products/host/profiles.md#gms-服务发现排队添加与任务退出)再核同 DLL 的 7 个函数、966 条指令及 4,061 原始字节，5 字节未枚举；服务发现读取注册表，扫描实际经 JS／SignalR 进入托管服务。当前包的三个程序集及同字节 aliases、22 个注册策略源码均核对捕获摘要；另独立解析 CLR MethodDef 与 tiny／fat 方法头，核 46 个方法的 token、RVA、头长与 code size，以及七处原始 IL opcode，未将其宣称为全部托管 IL 逐指令验证。OnStartScan 的 async void 正常主体没有 await 挂起点，异常仍经 builder 脱离外层 Action；WaitAll 的超时 bool 被 pop，finished 通知早于 stopped 和落盘。两把 Promise 锁的反向顺序／重入可形成条件等待环，实际触发与服务时序未执行。分段收集、请求关联丢失、策略停止粒度、取消与资源收束须按文档分别测试，不能由服务发现或静态正常返回证明成功。

同链[本机扫描的路径与去重](../products/host/profiles.md#本机扫描的路径去重与结果发布)继续核七个策略、公共基类和两份 SQLite reader，十一份 C# 输出字节与既有清单一致；25 个所选 MethodDef 身份／方法头独立核对，包含与前片重叠的方法，不直接相加为新增覆盖。两种 Registry 策略的 64 位门禁后实际调用 32 位 getter，由四处原 IL 与 MemberRef 名称共同证明；共享 AddFoundGame 的七处索引／通知调用也另核。不同 HashSet 比较器、数据库失败后返回前缀、无效 GUID、先插入后补字段、两张索引的非事务更新和通知早于保存分别保留。Encoding.Default、HMAC-MD5 大写 hex 与 Guid(string) 属于身份语义，不能擅自改用 Guid(byte[]) 或通用正规化。底层依赖、全策略取消／并发、Rust 与实际功能对照继续未验证。

同链[解析与发现通知](../products/host/profiles.md#解析依赖与发现通知的完成边界)另核 Lib 18 个、Common 9 个、服务 EXE 1 个所选 MethodDef 身份／方法头、完整 IL 及相关输出摘要；集合与此前有重叠，不相加为覆盖总数。路径字典实际由 Controller 注入 CurrentCultureIgnoreCase tuple comparer；服务注入名为 NewtonsoftJsonSerializer 的类实际使用 System.Text.Json，字符串／文件／固定 4096 B 字节入口的参数和错误边界不同。Mihoyo 字符分帧、逐条 JSON catch 与外层 I/O 失败、发现广播先于缓存更新、async void 发送挂起、缓存缺分区仍可回 GS_SUCCESS 及 void 保存失败分别保留。所选构造／调用另核原 PE 字节，完整 JSON／加密存储依赖、所有并发交错、Rust 行为测试与官方实际功能对照仍未由此完成。

[Steam、BattleNet 与 Epic 的本机输入](../products/host/profiles.md#steambattlenet-与-epic-的实际本机输入)另核 Lib 的 39 个、Common 的 4 个所选 MethodDef 身份、原 PE 方法头及关键调用字节，并重新核对两套程序集的全部 246／107 个捕获输出摘要；方法集合有交集，不累加为全策略覆盖。Steam 的两个 bool 结果被 pop、停止库循环后仍可能进入联网补全、数据库适配器丢弃序列化返回值、BattleNet 日志单次读取及 64 位门禁后调用 32 位 getter、Epic 先读两列表却以不同列表作门槛与遍历均分别记录。联网目录与匿名登录属于本项目删除范围，不能因此省略本机文件／数据库／图标和结果发布链。底层 KV／SQLite provider、完整取消与发布并发、独立 Rust 行为测试及实际 DLL／服务对照仍未由此完成。

[GSL 加密文件与重试](../products/host/profiles.md#加密设置文件的读取重试与释放)另核 Lib 5 个、Common 10 个所选 MethodDef／方法头、原 EH sections、TypeRef catch 类型及静态 #US 字段；方法集合含复用。FileMode.Create 会直接重写目标，独占打开探测不读内容且耗尽后仍置完成，外层十次 IOException 也可正常 void 返回；Dispose 顺序和非 IO rethrow 分开。加密字符串 Serialize 返回 Base64，而 DeserializeString 直接取输入的 UTF-8 字节后解密，未执行反向 Base64 解码。Aes 工厂显式参数与框架默认参数分开记录；加载失败后重建本地缓存不证明设备无配置。框架密码／XML 内部、完整数据契约及 Rust／实际 DLL／文件恢复对照仍未验证。

`inspect-corpus --no-refresh-gaps` 对所有索引 DLL／NODE／EXE／SYS 及静态提取的实际 MZ 无扩展名载荷分类 native、CLR、混合 CLR／native 和其他格式，保存 aliases、imports、delay imports、exports、version resources、CLR／ReadyToRun 头及失败。扩展名不代替 CLR 判定，ReadyToRun 不冒充纯 native C++。

corpus schema 3 另从当前官方原始 manifest／生产发现 JSON 建立版本来源链，逐项核对资源的实际长度、SHA／integrity 或源码声明的 MD5，再连接完整静态提取收据、内嵌字节区间、packed ASAR 区间及已验证的同字节 aliases。最新 host 元数据复核失败时不能退回旧稳定收据；installed 路径、PE 版本及继承的 verified 字段都不是来源根。ASAR unpacked sibling 必须另有官方包同字节证明。输入索引、原件及收据在审计结束时复核快照；来源链闭合与依赖、ABI、安装分支、实现及运行验收分别记录。

重复 `inspect-host --extract` 保留原始 extraction exit／argv。旧收据丢失退出码时，只调用 innounp `-t` 或 7-Zip `t` 做静态归档完整性检查，保存真实退出码、输入前后身份、分析器 SHA 和 stdout／stderr 字节摘要；不补写虚构的 extraction exit 0。已验证检查可复用，提取输出与 Inno 全部条目仍重新核对。

官方 native 重查对 manifest 和资源均发送 `Cache-Control: no-cache`、`Pragma: no-cache`，核对最终地址为官方 HTTPS。资源收据保留每次真实 HTTP／长度／SHA／声明差异的扁平 previous_observations；重查仍不匹配的字节保留在 quarantine，不能因多次下载相同而改为 verified。

`acquire-host --revalidate-bootstrap` 以相同禁缓存策略双取当前官方下载页及页内稳定安装入口，保存独立 [bootstrap 复核收据](../../.analysis/inputs/host-current/bootstrap-recheck.json)。当前两轮响应实际字节、官方 HTTPS 最终地址及页面声明均须匹配，corpus 才能将该安装器及完整静态提取后代标为当前来源；主 acquisition 保留不变，真实安装与生命周期仍未执行。

bootstrap 与 metadata 复核各自拥有独立 gaps owner；核对索引原字节、HTTP 200、官方 HTTPS、两份实际样本、前后稳定摘要及与原 acquisition 的一致性。重查失败、未完成、部署漂移或收据身份不符均独立保留，不能被旧获取收据覆盖。

`analyze-host` 默认先导出 version resource 声明 Razer 所属的 CLR 程序集；`--managed-scope all` 包括所有实际 CLR 原件与依赖，范围、当前输入身份、两阶段结果和 pending 清单见 [managed capture](../../.analysis/results/host-current/managed/capture.json)。新增嵌套原件后继续补捕获，不能沿用先前计数当全库闭合。native Razer.WMI.dll 使用 IDA。不能根据 Analytics、Online、Cloud 等文件名排除整份模块，需按实际调用链分类本地和联网行为。

嵌套获取按实际文件头递归 opaque Burn／MSI 流、CAB、ZIP、Compound 和 Inno Setup。`extract-embedded-inputs` 对实际 MZ 输入扫描边界合法 CAB，DLL 与无扩展名文件不会因名称漏掉；exact CLR resource 另核对原件唯一字节匹配与长度前缀。CAB 头合法仅表示候选，必须核对静态解压全部内容。Inno 完整条目表及提取结果保留，重建 script 放 results。失败解压产生的 partial 及后代经 `reconcile-native-extractions` 明确标为未验证，保留实际 bytes／SHA，不用于当前源码、native 或 managed 原件证据。

CAB 候选另静态解析完整 CFHEADER／reserve、CFFOLDER、CFFILE 和 CFDATA 边界；结构通过不代表压缩或校验和验证。当前 237 条候选中，35 条（34 个唯一 SHA）结构失败的固定头全部位于另一份结构完整 CAB 的压缩 CFDATA 内，外层静态提取收据及实际产物全部验证通过；不能把这些切片的解压失败解释为官方原包损坏。候选、失败切片和 partial 不删除，完整表、原始 block 头／范围、外层实际 SHA 与提取收据进入同一 [embedded-inputs](../../.analysis/results/native-packages/embedded-inputs.json)，安装分支、ABI 和产品实现仍分别待完成。

同字节 PE 只扫描一次，但对每个使用的父容器路径验证实际长度／SHA，保留 `container_aliases`，优先完整的官方 manifest 绑定；失败解压副本不参与。family 提取沿这些真实边保留各根资源与 manifest mismatch 状态，避免宿主副本先出现而丢失设备包内嵌 CAB。此前遗漏的 `WdfCoinstaller01009.dll` 内 CAB（`9048c6f6…`）已沿现有 `acquire-family --stage extract-native` 静态提取两份产物，不能据此声称完整本地安装生命周期已逆向。

## 原生捕获与源码连接

每份 `.analysis/results/native/<sha>/capture.json` 保存输入身份、image base、函数边界／RVA、函数块原始 bytes、IDA 枚举的汇编项、成功取得的伪代码、局部类型、结构、xref。每个输出独立 SHA 校验。`--resume` 仅在输入、IDA／Hex-Rays 版本及捕获工具摘要一致时复用函数，并核对函数文件摘要、边界。

`assembly_captured=true` 只表示记录了分析器枚举的指令项，不能直接证明它们覆盖原始函数块每个字节。须将所有指令区间与 chunks 核对，分别保留指令覆盖、原始字节覆盖和 Hex-Rays 失败；块内未被枚举的范围不能凭空解释为指令或补成伪代码。现有完整 raw chunks 可用于继续定点逆向，原捕获失败不改写为成功。模块级批处理成功、全函数枚举和业务语义闭合也分别记录。

IDA 9.1 的批处理与补查明确传入 `-Opdb:off`，依据安装的官方 `cfg/pdb.cfg` 禁止隐式联网下载 PDB；选项与策略保留在 plan／session。它解决分析器下载阻塞，不提供缺失符号或 ABI 证明。

大型 EXE 的候选字符串不能先汇成 Python 全列表。`tools/ida/capture_io.py` 将 dataset 逐记录写成完整 JSON array，聚合 C／ASM 逐函数写入，文件 SHA 按块计算；不截断字符串、函数或 xref 范围。导出前保存私有 packed IDB checkpoint；`--resume` 复用已验证的 packed DB，或明确记录 unpacked DB 恢复及组件摘要，不传会重建数据库的 `-c`。失败尝试与实际日志 bytes／SHA 嵌入同一 session 的 previous_attempts，不覆盖为成功历史。恢复数据库额外通过 IDA `validate_idb(VLD_SILENT)` 只读核验；非零不一致记录阻止 capture_complete，不能把可打开或已修复提示当作完整数据库证明。

恢复库完整性失败时，capture 写明失败阶段并在导出 datasets／函数前退出；下次 `capture-native --resume` 将已诊断失败的数据库逐文件移入同一私有 scratch 的 `invalid-database/<实际 SHA>/`，先校验源／目标绝对路径边界并保留实际长度／SHA。session 的 previous_attempts 同时保留失败 capture 原字节和日志，随后只从已验证 EXE 以 `-c` 建立新库。AppEngine 恢复库本次检出 1,238 处不一致，已保留失败收据并开始干净重建，不能算全函数已捕获。

聚合 C／ASM 完整写入临时文件后才发布，失败保留先前完整输出；捕获收据在聚合文件和全部输出摘要写完前保持 `capture_complete=false`。队列复用完整捕获及新捕获返回成功后，均重新核对实际原件长度／SHA、manifest 输入身份、完整函数数量、唯一 RVA／地址、汇编捕获标记、七类 dataset 与 C／ASM 聚合文件清单及全部产物摘要；不单独信任成功标记。反编译失败不会伪装为完整伪代码。验证中清单或产物变化逐项失败，记录实际捕获清单身份及逐原件前后快照。队列结束时再次复核全部已验证原件快照及读取的语料收据实际 SHA，先完成的原件在后续分析期间改变也会阻止完整批次结论。

已保留实际输入并完成全函数捕获的基础模块包括 HID.node、mapping_engine、simple_service、BLE binding、Razer.WMI、SysUtilsNative、两份 Seiren DLL 和 GenericCamera。数量与 Hex-Rays 失败以各自收据为准。当前 GenericCamera 1.0.35.0／1.0.36.1 基准收据实际使用 IDA 9.1、Hex-Rays 9.1.0.250226、Python 3.11.9，分别保留 2817／2825 个函数；每版各保留一个 UTF 辅助函数的原始反编译失败及完整汇编。两份原件、工具、私有数据库检查点及完整产物摘要均记录在对应 [native 结果](../../.analysis/results/native/)。

GenericCamera 的两个 UTF 失败 owner 已在独立私有副本进行类型恢复，每版补查 7 个完整函数且失败为零，保留实际二／四参数调用证据；原始失败记录不改写。另分别提取两版 base／Pro／X 的六张完整虚表和所有目标，旧版 44 槽、新版 46 槽。新增 shutter-limit 槽使后续偏移变化，Pro RestoreDefaults 的实际命令也不同；这些差异不能由同名导出或函数数量推断。八份补查的精确收据、RVA、协议和版本边界见[摄像头](../products/accessories/camera.md)，不代表所有摄像头业务或真实 DLL／设备对照完成。

两份 Seiren DLL 各 1,947 个函数中的唯一调用分析失败点 RVA `0x3318e`，已使用 IDA 9.1、Hex-Rays 9.1.0.250226、Python 3.11.9 对私有数据库副本定点补查。完整指令和交叉引用证明其为四寄存器参数的 vector reserve 路径，原分析器误推六参数；每份补查各保存八个完整函数，失败及数据库不一致均为零。原全库捕获及其失败记录保持原样，统一 native 收据连接补查的实际字节身份；不能由该点闭合推导其他间接 ABI、产品实现或运行验收已完成。

当前 3595 麦克风查询沿完整 `Ph({})`／动作／reducer／中间件／FFI 到 1.0.35.0 导出和基类 vtable `+304/+312/+80/+336`，逐字节核对设置、读取、闪存保存、重启及两次 10ms 间隔。`DeviceInitialize`、精确 container-ID 匹配、PID `0x0e0b` 工厂分支和基类构造函数的完整函数及实际 PE 指令也纳入查询，验证该产品确实取得相应 vtable。原版开关只设置 dirty，重启动作才执行本地写回；Rust 保留该语义并分离实际传输诊断。证据纳入既有 `camera-local-pages`／`camera-source-review` 查询，纯状态和模拟 KS 测试通过 `camera-mic-state`／`camera-mic-protocol` 套件记录；这些不代表整页或真实设备验收完成。

`audit-native-chain` 连接完整原始 callable、FFI 签名、精确导出、native 函数、imports／xref 和实际 Rust 文件摘要。查询包括 HID feature、mapping engine、simple service、restart notification，统一报告为 [native/chain-audit.json](../../.analysis/results/native/chain-audit.json)。查询无失败不代表完整页面／设备验收；回调、间接目标和 ABI 未解项继续追踪。

chain schema 2 的每条 native edge 以完整函数产物的实际 path／bytes／SHA 引用目标，不重复复制目标的全部入口 xref 和 chunks；全部边均保留，完整汇编、伪代码、原始块和交叉引用仍由对应 capture／functions 原件结果提供。此调整避免高扇入函数让关联报告呈平方膨胀，不减少分析范围。

[鼠标输入监控附片](../../.analysis/results/frontend/mouse-mw-audit.json#/queries/shared_input_monitoring)复用完整原生捕获，核对 151 个函数、153 个 chunk、9,344 条指令、11 张实际表和 165 个完整前端 AST，涵盖七款动态速度／六款实际挂载 Matcher、14 个 source-set 及 1,620 个 artifact；原始块中两字节未枚举指令范围独立保留。冻结 collector 后的独立 verifier 通过，91 个篡改反例全部拒绝；新增 query 保留旧 13 个查询不变。四份 Rust 原件的 37 个完整函数另作静态比较。普通 STOP 最终批／历史、早 ACK／实际注册／shutdown、动态字符串比较／226 再作差与 Matcher 先转 Number／到达顺序分别保留，详见[灵敏度](../products/mouse/advanced-sensitivity.md)。1,156 条未展开直接边、18 条间接／导入边、当前获取限制和全部真实 DLL／设备／窗口结果继续单独未验证，不从静态模型或核对器负例推定功能完成。

restart 查询使用新取得的 [notification-manager 当前原件](../../.analysis/inputs/frontend/notification-manager-current/acquisition.json)，保留 Dashboard caller、WindowStorage 内存／事件、通知 reducer／窗口、完整确认页／样式／语言、FFI 派发和 SysUtilsNative 的 `restartWorkstation`。该导出 RVA `0x70370` 尾调用 `WinExec("shutdown -r -t 0", 0)`，未执行重启。重复同 caller 的 true 生成新数组，重新触发窗口检查；已有窗口不主动 focus；LATER 的 disabled CSS class 不等同 HTML disabled。

`capture-corpus --include-path REGEX` 只决定分批分析顺序；未选项保留在队列，子集成功不能写成全语料闭合。并发分析使用 `--no-refresh-gaps`，由汇总所有者在收据写入后统一刷新缺项。

全函数枚举仍可能漏掉真实指针表指向的跳转入口。`supplement-native --input PROJECTREL --pointer-table-rva RVA --pointer-count COUNT` 从原始 PE 的实际、文件支持、可执行目标验证指针表，在既有私有 IDB 的副本中定点补查；`tools/ida/supplement.py` 仅为字节精确的 x86/x64 直接跳转、x64 内嵌对象析构转发或 x64 绑定成员尾调用 thunk 创建缺失边界，其他未知边界保持明确失败。析构转发须完整匹配 13 字节的取 vtable、同位移调整 this、清零删除标志和间接跳转序列；成员调用须精确匹配 11 字节 `mov rax,[rcx+8]; mov rcx,[rcx+24]; jmp rax`，其成员目标和绑定对象仍须由构造消费者另证。每条指令由 IDA 独立解码，已有指令的头与长度必须一致，不删除未知数据或覆盖其他函数。仅补函数边界不证明间接调用目标。HID 的 `0x10ae0` 首次因原直接跳转规则被拒绝，扩展规则后补获完整汇编／Hex-Rays；当前构造函数及原始 `0x51ba0` 表另证明其调用 `0x4710` worker 析构。补查保留七类信息：原表字节、slot/RVA、完整函数块原字节、汇编、伪代码及类型、xref、直接跳转链。已有完整函数按实际原字节及输出摘要复用，原全库收据与数据库不改写；补查结果位于同一 native SHA 下的 `supplements/`。补查完整不表示 RPC ABI、间接调用、产品功能或真实运行验收完成。

## 前端、资源与完整性

Wakaru 1.14.0 固定源码提交 `8219bf5016a063d7fb215101a8c45d5959ec733f`，使用 `--unpack --level minimal --provenance --emit-source-map --diagnostics --json`，不启用 DCE。显式提交全部 JS；输入 map 的 `sourcesContent`、原始 UTF-8 `[start,end)`、合成 map、恢复模块分别记录。

manifest、HTML、JS、CSS、资源及部署前后摘要共同确定当前入口。source map 404、动态 publicPath、未知 chunk、枚举 null、未解 import 独立记录。语法可解析不能证明 mounted tree、版本分支、状态、Promise、设备写回、持久化或清理完成。

固件专项证据将完整 JS AST 所有者与原生嵌套 CSS 块分别保留。独立复核按实际 UTF-8 字节校验源码身份、完整树节点边界、原文与摘要；CSS 使用处理注释、转义、字符串、圆括号及属性括号的全文平衡块扫描，核对原始 selector 和完整块边界，不把它称为 CSS AST、声明合法性或计算样式验证。原生函数的全部 chunks／指令和真实分发表另与 PE 原字节逐项核对，PE 装载器零填充区由 section header 确认，不伪造磁盘 raw hash。上述静态核对不能替代 Rust 行为测试、实际 DLL／设备结果或窗口视觉与键盘／焦点验收。

[1443 APO 效果专项](../../.analysis/results/native/cf68efbd4d5ceb735585bbacc9d1f635d7d2e9f312fc387502ea0cc8d40e6470/effects-semantics.json)使用本次批量捕获，核对来源容器、19 个完整函数、6 份虚表、模式 GUID、分配失败和音频缓冲分支。唯一 UTF 辅助函数反编译失败保持原样，所选完整汇编另与 PE 比较；十个固定期望向量只属于静态核对。两种模式的效果枚举、固定 E_NOTIMPL 方法、默认 DSP 和其他变体的未闭合边界见[播放与均衡器](../products/audio/playback-and-eq.md#1443-本机-apo-的效果枚举与处理模式)。

[Synapse 自启专项](../../.analysis/results/frontend/synapse-settings-current/local-startup.json)复用当前完整前端捕获，分别校验部署、167 个实际 AST 所有者、53 个完整词法 CSS 块、十语言四词条、字体、八份宿主 ASAR 来源及 16 个原生函数。独立复核不执行 JS，以 AST 字符串片段解码翻译转义，按 CSS 中真实 URL 核媒体获取身份；原 full-capture validate 失败仍照录。异步 init 未被等待、App 注册 gate、每点击独立 timer、乐观状态、session flush 与注册表真实结果分别追踪，详见[设置](../products/host/settings.md#自启专项的验证边界)。

[Chroma 自启分支](../../.analysis/results/frontend/chroma-app-settings-current/local-startup.json)单独核验 209 个完整 AST 所有者、54 个 CSS 块、十语言四词条与四字体，复核实际 locale→chunk／module 和入口 loader。它等待 init，但两个点击只创建 debounce、未调用返回函数；不能复用 Synapse 的保存调度结论。SDK setter 的即时本地更新与宿主命令仍保留，未调度的只是后续 flush。共享宿主／原生引用重新核实字节；完整捕获的 validate 失败和真实窗口／DLL 未验证状态不变。

同一 Synapse 报告的 `dynamic_lighting_control` 以独立审计器核对两设置入口、LE 与宿主：166 个角色、160 个不同完整 JS AST 所有者、38 处绑定、16 项导出、49 条断言、36 条调用／清理边和 234 项证据。六份 Rust 原件、九个 AST 所有者与 1,689 份 Rust 文件的观察者扫描另行保留；三个解析受限文件仍核完整字节与词法引用。31 个篡改反例被独立检查器拒绝。各切片精确合并后为 12,874 项证据，交集身份必须相同；Chroma 的共享宿主基线随新报告重新绑定。意图、进度、Configure／JsonApi 和 pagehide 清理分别核对，见[动态灯光](../products/shared/dynamic-lighting.md#两个设置入口到后台的真实通道)；没有执行厂商 JS 或实际 DLL／设备，不能将静态接线提升为真实控制权转移或 Rust 功能测试通过。

同一自启主收据另保留实际 dashboard 的注册 producer：356 个完整 AST 所有者、191 条静态断言和 84 组完整函数比较，覆盖新条目默认、已有启动字段保留／元数据更新、非 boolean 正规化、Chroma 等待 gate、启动命令重建、几何 flush 与选定卸载消费者。设置入口缺 appName 不代表整个应用没有注册；局部卸载所有者缺重建也不能证明整个卸载链不存在。跨页面事件、实际落盘、installer 后续清理和官方 DLL 对照仍未验证。

[APO4 流属性 RPC](../../.analysis/results/native/81c7c20b867a45e83eaa6ea00163159dfaf353f8577193632d67d1708d685bde/stream-properties-rpc.json)分别核对两产品各 68 个完整函数、5,823 条已枚举指令和 63 条独立断言，恢复两份完整描述符及七条 RPC 路径。完整 chunks 逐字节相等仍不代表每字节已解码：每模块另有 95 字节没有指令项，原始范围继续保留。来源中的 CAB offset 属于解包后的内层 EXE，不能直接套到外层下载包；ZIP 成员另行重读比较。四产品属性 DAT 另核 963 个 ZIP entries、939 个完整 JSON、所有成员 CRC／压缩区／解压字节及 25 项调音差异；定义和默认预设声明不能替代服务实际选择。

同一报告的 2641 基础服务切片另核 88 个完整函数、26 个数据区、18,329 条指令和 47 条独立断言；原始块内未枚举的 1,379 字节／228 范围单列。实际构造／注册表证明 SetStreamProperty 固定返回状态 12，全局 setter 另有校验前去重、通知及 1000 ms 保存；Get 的 stream／global／definition 来源分别追踪。服务 parser 的 430 项候选定义、volatile 初始化及缺 float default 的零解析分支已核，不能当作硬件观测。

`actual_service_consumers` 再将所选服务链扩至 2641／1415／1401／2638，各核 131 个完整函数、38 个数据区、81 条映射指令断言及七组 GUID／PROPERTYKEY 的实际消费者引用，共 89,518 条指令。各自的构造／注册／表槽和地址差异保留，基础 88 函数的形状一致不代表原字节或整个服务等价。麦克风预览明确追到 Windows 音频 COM 的五参数 slot12、两次属性写入与 HRESULT 分支；初读、严格 bool variant、端点配对、热插拔与未闭注销／恢复分别记录。四份完整捕获各自保留七个 Hex-Rays 失败及版本解析限制；指定数据区以外的引用不冒充完整内容证明。全部 DSP／硬件消费者、Rust 替代及实际 DLL／设备对照仍需分别完成。

[Soma 路由服务专项](../../.analysis/results/native/af4fcfb594afeb7e678d4cbe3cf7f41556252a31f7f839b567f2cea855033b82/routing-semantics.json)另核 25 个正式 section：基础六片加 endpoint refresh、VAD／driver、前端桥／命令／页面分支／控件 CSS／横向音量／灯光效果、GLOBAL EQ／异常生命周期、重采样／FFT、非 VAD 四段 flow／异常生命周期／外层所有权，以及 CTC／Virtualizer Front 默认、六支持久化、DB 文件边界和消费者分类。各片按原件 SHA＋RVA、完整 record 和精确 artifact 身份合并，最终为 965 个不同函数、6,091 个去重 artifact；六份前端片去重为 514 个完整 AST owner，不能按裸 RVA 或分片数量相加。基础服务仍为 174 个完整函数、24,414 条指令、10 张实际表、26 条固定断言和六组实际 registrar。ChannelMaps／Delay 消费者沿通知追到内存增益路由表与 EndpointSignalFlow；输入／输出 channel 的两层 key、有序 delay 限幅、NaN 保留和浮点 samples 换算须按实际指令核对，内存更新不证明 stream 或设备写入完成。独立复核直接重读原 PE、描述符和来源，不导入收集器；语义／字段／伪代码／完成度篡改反例分别保留。它使用 ZeroMQ／protobuf Any，16 个实际请求与 36 个 schema 消息分别记录。revision 的同步内存更新、异步状态发布、API6 不匹配仍回 ack、完整控制地址，以及 EndpointVolume／VadChannelVolumes 两条通知触发链的运行时标量／OS 消费者均须按实际边界判断；不复用 APO4 gRPC 结论。现有 Rust 的 schema／真实应答接线和缺失通知、前缀、revision 回退差异分别记录于播放文档，静态复核不替代 Rust 行为测试。完整数据库耐久性、下游 DSP、stream 重建、恢复和实际功能对照继续单独验证。

同一专项的试听切片另核 42 个完整函数、10,601 条指令、281 个证据文件、68 处锚点、30 组直接绑定、2 张虚表和真实 WAV。Play／Stop 的空响应、创建线程后才设置测试标志、Stop 分支没有旧流快照／重建／cursor 重置及最后 upstream join 次序均保留；12 个篡改反例被拒绝。格式／PCM 切片再核 22 个完整函数、7,864 条指令、154 个证据文件、139 处锚点、26 处 COM 调用和 12 个数据区，独立复核拒绝 30 个篡改反例。14 个 PCM 向量及 20 个格式用例是有明确 MXCSR 条件的静态模型，未执行厂商代码；GetMixFormat 的真正生产者、closest-format 丢弃、PCM32 的 `2^31` 系数与部分错误路径保留非负 HRESULT 均按原字节记录。六个基础切片仍单独保留 265 个不同函数、655 个 artifact；完整专项的最终 union 见上段，重叠函数的完整记录须一致，不能按裸 RVA 混算不同 PE。

`endpoint_refresh` 另核 62 个完整函数、12,024 条指令、12 张虚表、19 个数据区、84 处锚点、20 组直接绑定与 327 个 artifact，18 个篡改反例均被拒绝。STOPPED 发布、真实任务虚表、ZMQ PUB 与独立 refresh 的调用者分别核对；自定义 endpoint 分类不冒充 ERole。用于识别用户在场 GUID 与 OS 声明的两份 SDK header 仅最小登记到原件索引，其安装器获取链没有保留；它们不是 Razer 协议证据，也不证明对应 OS 通知已实际执行。

`vad_upstream_and_driver` 分别核对服务 53 个与驱动 194 个完整函数，共 16,606 条指令、14 张表、64 处锚点、41 组直接绑定及 368 个 artifact，45 个篡改反例均被拒绝。专用 IOCTL、落入 PortCls 的 flush、六 u32 头与八路 planar i32、ring／reader／缩放／CPU 转换和正常 join 顺序均依据原字节；driver 全函数捕获不表示全部系统语义或实际设备应答已经验证。

`frontend_audition_bridge` 保留 86 个完整 AST、61 个角色、23 处绑定、10 项导出与 wrapper22／client3 个完整原生函数，共 4,447 个 artifact，29 个篡改反例拒绝。`frontend_command_lifecycle` 再核 90 个 AST、39 个角色、29 处绑定、15 项导出和 169 个 artifact，27 个反例拒绝；它只含 JS，不增加原生函数。`frontend_page_branches` 另核 118 个 AST、58 个角色、38 处绑定、22 项导出、169 个 artifact，原生函数仍为零，36 个反例拒绝。三片按 SHA＋源码区间合并为 249 个 AST，完整保留原捕获失败。实际页面挂载、资源选择、FFI bool、client 丢弃响应，以及 display 根、nav 卸载、Surround cleanup、Profile 恢复、准入／录宏占用／FIFO／abort 分别核对。任务 value 初始化为 undefined 却被写 status 的异常只属静态源码结论，不能用绿色日志或 maker_SUCCESS 认定 DLL 功能完成。

`global_eq_filter_and_snapshots` 另核 59 个完整函数、7,571 条指令、11 张虚表、15 个数据区、11 处构造绑定、43 处锚点、36 组调用及 201 个 artifact，24 个反例拒绝。24 个 EqType 分支、64→112 字节快照、普通有限输入公式、DF2T 历史和 MXCSR 均留原始证据；八路系数与 metadata 分开入队且忽略 enqueue 的行为不能简化成整组已应用。完整数值精度、异常展开与并发仍分别核验。

`global_eq_exception_lifecycle` 再核 46 个完整函数、2,586 条指令、30 个原字节区、11 个 runtime 根、9 个 FH4 根、29 个 unwind 状态、119 个调用点及 119 个 artifact，42 个反例拒绝。26 个新增函数与上一片 20 个依赖函数的完整记录／artifact 交集一致；两片并集为 85 个函数。收集器与独立检查器分别解析实际压缩 unwind／IP 表，保留无 TryBlockMap 的清理、正常入队失败释放与八声道部分发布，不能将析构误写为事务回滚。全部 CRT／队列异常、SEH、外层 catch 与 shutdown 恢复仍未闭。

`resampler_dispatch_priming_fifo` 另核 59 个完整函数、60 个 chunks、6,907 条指令、63 个数据区、5 张表、10 个实际槽和 153 个 artifact，59 个篡改反例拒绝。63 项预热表由完整伪代码与原始指令分别重建，保留唯一缺项及内存 padding；80 个条件静态模型覆盖环境优先级、int32 饱和、CPU 分派及 FIFO／flush 计数，不计算音频值。203 条未展开直接边与深层 FFT／SIMD 数值继续保持未闭，不能把重采样控制链或捕获通过作为独立 Rust 算法、官方实际执行及音频验收。

`resampler_fft_and_stage` 再核 46 个完整函数、10,598 条指令、45 个原字节区、20 处 FFT 槽、8 处机器级 ABI 校正、39 组直接绑定及 81 个 artifact，49 个反例拒绝。默认 provider 的 real FFT plan、19 行 polyphase／两行 halfband 表、Kaiser sinc／I0、float32 四项系数布局与局部插值均有实际指令和数据表；49 个条件模型只核有限结构、地址及数学身份。动态 DFT 长度、CRT 三角函数分派与复数乘法的特殊标量不能被通用库假定替换；84 条未展开边、其他 provider／phase 和逐样本实际对照仍未闭。

`front_ctc_virtualizer_state_and_defaults` 正式切片核对 54 个完整函数、8,785 条指令、6 张表、12 个数据区、25 处锚点、46 组直接绑定、7 处 RIP 绑定、10 个 schema 消息和 302 个 artifact；39,333 字节 raw chunks 中保留 38 字节未枚举范围，31 个篡改反例全部拒绝。本片追到 Front observer、typed pack、revision/diff/copy、初始化读取／默认分支、通知队列和 Any socket 发布：CTC 为 `0x71780 → 0x67180 → 0xeae40 → 0x600e0 → 0x5b7f0`，Virtualizer 为 `0x714b0 → 0x67750 → 0xeae40 → 0x600e0 → 0x5b7f0`。发送返回 1 只表示发布尝试；EAGAIN=11 仍可返回 1，不能当作消费者收到或 DSP 生效。默认 CTC 配置 active=false，Virtualizer 的左右 FIR 各 512 项且为单位脉冲；六支状态／持久化见下一片，耐久、异常 inner、真实 DSP 挂载和 DLL／设备结果仍独立未验证。

`ctc_virtualizer_six_branch_persistence` 独立核对 Front／Side／Rear CTC 和 Virtualizer 六支的 120 个完整函数、16,607 条指令、18 张表、6 份 RTTI、30 个数据区、66 处锚点、194 组直接绑定、24 处 RIP、275 条源码断言、10 个消息与 323 个 artifact；71,861 字节 raw chunks 中有 48 字节未枚举。禁用 collector 后复核通过，46 个反例拒绝，与 Front 基础片去重为 123 个函数。variant4..9、各自 DB key、初读默认 gate、revision、observer、typed 保存和基类 stop/join 顺序逐支核实；owner 缺失不等于底层 DB 空的安全 false。保存 wrapper 丢弃 writer bool，六支没有事务；底层 options 消费者由下一片核对，完整异常和实际设备／DSP／Rust结果未验证。

`ctc_virtualizer_database_boundary` 再核 58 个完整函数、7,130 条指令、4 张表、57 处锚点、33 组直接绑定、4 处 RIP、12 处虚槽、5 个文件 API 与 11 处导入调用、86 条源码断言及 220 个 artifact；29,996 字节 raw chunks 保留 29 字节未枚举范围。禁用所有 collector 后复核通过，48 个篡改反例拒绝。三片去重 178 个函数，本片相对前两片新增 55 个。实际 owner、checksum／cache／snapshot、五参 Put 与三参 batch encoder、日志写入、条件 OS 同步、同步失败保存错误、后台等待及析构槽分别追到原字节；选项零仍写 WriteFile，所选 file 析构自身不 flush、不核 CloseHandle。582 条未展开直接边、其他环境／恢复／异常及实际耐久与功能结果仍未闭，不能将文件调用路径当作已完成整个数据库或 Rust／DLL对照。

`ctc_virtualizer_consumer_inventory` 再核 164 个完整函数、165 个 chunk、17,100 条指令、102 个入口／270 条引用、18 张 callable 表、36 处地址绑定、24 处分派、83 组直接绑定、192 条源码断言及 360 个 artifact；72,388 字节 raw chunks 保留 50 字节未枚举范围。禁用 collector 和构建 plan 后独立复核通过，57 个篡改反例拒绝。四份 CTC 片去重 232 个函数，相对前三片新增 54 个，全专项实际新增 53 个。六支 readonly 的消费者逐项区分 publication、析构保存和请求序列化；另外六个 CopyFrom caller 是 state map 插入／variant 替换。12 条数据引用经原 PE exception directory 证实为 UNWIND_INFO CHAININFO，不能当作间接 DSP 回调；所选通知 worker 在解锁后才调 callable slot2。1,798 条未展开直接边、35 条间接／导入边及实际 WASAPI／DSP 桥仍保留，不能由服务状态复制、FIR 默认或消息发布推断音频生效。

同一收据的 `observer_lock_bindings` 沿 `0x23025/0x230a6` 的实际 call／tail jump、两份导入 thunk、IAT／lookup 和 KERNEL32 原始导入名，确认 callback-list 使用 Acquire／ReleaseSRWLockExclusive。完整函数已在基础片，补充原 PE 绑定不虚增函数或 artifact；状态读取的 shared lock 与 callback-list 独占锁分别建模。

[client 三版本分支](../../.analysis/results/frontend/audio-surround-audit.json#/native_client_branch_boundaries)独立核对 1392／1442／3942 的 manifest、wrapper loader、完整 PE 导出、226 处 `GetProcAddress` 调用及三份 Carol FFI 初始化器，共 455 个完整函数、20,072 条指令和 88,556 字节 raw chunks，其中 105 字节未枚举为指令。1392 的 client 没有所选空间音频导出；1442 的 102 个导出具有活动实现或跳转；3942 的同组导出正常路径固定返回 11。三款 FFI 均为 67＋4 项且未绑定这组接口，不能由导出存在推定页面已调用。禁用 collector 后的独立核对、31 个篡改反例及 22 项 `test_families` 测试通过；命令为 `audit-family-function --family audio --function mw --query audio-surround --native-client-branches check`，`negative` 运行反例。此片与路由 965 函数统计分别记录；活动 FIR 下层、实际 DSP、Rust 独立实现、接线和真实 DLL／设备结果仍须逐版本确认，详见[播放与 EQ](../products/audio/playback-and-eq.md#virtualizerfir-的-client-产品与版本分支)。

同一报告的 [1442 Virtualizer／FIR 活动链](../../.analysis/results/frontend/audio-surround-audit.json#/native_virtualizer_fir_1442)另核 client 79／service 26 个完整函数、14,427 条指令、61,540 原始字节、9 个实际虚表槽、44 条直接绑定、5 项 descriptor 字段及 7 条来源获取边；70 字节未列为指令。正式 `--native-virtualizer-fir check` 独立重读原件通过，`negative` 在禁用 collector 与原生收集 helper 后拒绝全部 43 个篡改反例，当前 `test_families` 共 33 项通过。client 接受正的奇数长度、服务要求偶数，setter 的即时回调不等待服务 ACK，JSON 的未知 selector 在合法解析后可返回 0 且不更新；服务六个消费者忽略 DSP 配置函数的 bool。以上分别说明本地状态、服务提交与 DSP 配置边界，不能合并成音频生效。完整异常／取消／并发释放、全部产品分支、Rust 功能测试和实际 DLL／设备对照仍未验证；既有报告字段的 canonical 身份保持不变。

1442 的 observer → PCM → DSP 续追另核 [同一服务原件](../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/capture.json)的 82 个完整函数、82 个 chunks、11,631 条指令及 50,148 原始字节，160 字节未枚举范围保留；五张核心虚表、九个 invoke 槽和两个常量亦与 PE 一致。三 observer 重读 GLOBAL、状态队列解锁后调用、音频锁、八路前置增益／mute、Front CTC／Virtualizer、Center／LFE 与 Rear／Side 顺序已经追到实际 PCM 消费者。四 FIR、direct／FFT 分支、biquad／MXCSR 与每块推进 0.2 的 Mixer 分别核对，构造的双声道最大块长由原指令确认为 4,800。配置分次重建没有整组回滚证据；Mixer 中间权重分支只对首行做两次 nframes MAC，不能套用标准双声道公式。所选正常 join／析构不等于全局回调静止，异常、并发及实际音频结果继续分别验证；此集合与正式切片有交集，不能相加为新增覆盖。

该版本[全局停止与 observer 所有权](../products/audio/playback-and-eq.md#1442-的全局停止与-observer-所有权)又续核 57 个完整函数、57 个 chunks、3,853 条指令、16,592 原始字节及五张虚表，24 字节未枚举。全局 gate 是 xchg；空 shared_ptr 的原槽循环不是旧借用的等待屏障。最后一次状态引用释放才触发通知线程 stop／join 和 observer map 析构；observer 借用状态 owner，却仅保存音频 manager 的裸指针，因此 PCM join 不能证明全部回调收束。五个 .pdata 根、FuncInfo／UnwindMap／IP 表和本 PE 解码消费者分别复核，所选析构的 noexcept／terminate 与构造 14 条 unwind 各有实际边界；这些 guard 不代表业务恢复。全局并发交错、外来异常、完整持久化及实际功能对照仍未确认，集合也不与 82 简单相加。

`endpoint_nonvad_four_band_flow` 核对 67 个完整函数、8,992 条指令、10 张表、13 个数据区、11 个构造绑定、4 处实际虚表调用、50 条直接绑定及 185 个 artifact。36,817 字节 raw chunks 保留 137 字节未枚举范围；44 个证据篡改反例拒绝。初始化 `0x9febe` 真实尝试八路 band3／20 Hz HIGH_PASS；enabled 与 metadata active、四段索引的 signed gate、八路部分发布和 pending/current 快照分别保留。19 个条件静态模型不替代 Rust／官方计算；所选异常见下一片，浮点全分支与全部并发释放继续追踪。

`endpoint_nonvad_exception_lifecycle` 再核 84 个完整函数、4,306 条指令、89 个数据区、18 个 runtime 根、12 个 FH4 根、41 个 unwind 条目、155 个调用点、12 个指令范围与 157 个 artifact；47 个函数相对基础片新增，原块 3,000 字节。禁用 collector 后复核通过，71 个反例拒绝。真实 EH/IP 表区分已安装资源、局部借用／共享引用 guard 和未转移 raw 对象：系数或 metadata 失败不回滚此前发布、裁剪或成功扩池；`0xa72a0` 的 header `0x68` 含 terminate。9 个条件模型不计算实际音频；外层 catch、队列内部、全部异常／并发与官方实际执行仍未闭。

`endpoint_nonvad_outer_owner_exception` 再核 88 个完整函数、7,530 条指令、123 个数据区、25 个 runtime 根、20 个 FH4 根、112 个 unwind 条目、5 个 try／5 个 handler、605 个调用点、14 个指令范围、17 处直接绑定及 283 个 artifact。32,588 字节 raw chunks 保留 55 字节未枚举；相对旧两片新增 70 函数／27,304 原始字节，禁用 collector／collector decoder 后独立复核和 96 个篡改反例通过。三阶段 node／map 所有权、render state13 catch 不含首次配置 state4、noexcept wrapper 的实际调用状态、CRT abort 和正常 join 后销毁 map 均沿原始元数据核对。六 registrar 的 `0x97810` 与 endpoint-vector 的 `0x97eb0` 分别绑定。7 个条件模型与 649 条未展开边保持有限范围；全部外层通知／初始化异常、CRT／SEH、并发和官方实际功能结果未闭。

`frontend_controls_css_and_display_subtrees` 核对 87 个完整 AST、29 个角色、19 个导出、10 个词法绑定、4 份 CSS 中 447 个完整块及 media 祖先，精确保留 52 个 artifact；另核 3 份 Rust 文件的 8 个完整方法，28 个篡改反例拒绝。3942 Delay／EQ 的 Rl mouseup、preventDefault 与立即求值、选择层、教程动态 MP4、Sensa 来源及 lighting 子控件分别记录，不把纵向行为推广到 module7114 横向音量。当前 Rust 的 WebP、Release 相等值 gate 及 gap 声明差异仅为源码比较，computed style、焦点、视觉、设备结果和 Rust 功能验收仍未验证。

`frontend_horizontal_volume_controls` 再核 103 个完整 AST、60 个角色、21 项导出、18 个绑定、9 份选定原件、58 个完整 CSS 块及 168 个 artifact，另核 2 份 Rust 文件的 8 个方法；35 个证据篡改反例拒绝。包含灯光效果的六份前端片按 SHA＋完整源码区间合并为 514 个 AST，交集保留完整相同记录。横滑条真实 50／120 ms 延时、mouseup 未清 timer、主音量与 channel 不同过滤、卸载、读取分支及逐事件新建 100 ms timer 均由完整 owner 核对；不能虚构跨事件合并。主音量官方 global／endpoint／缓存与 Rust role1/readback 的分支差异继续保持功能未验证。

`lighting_effects` 再核 145 个完整 AST、81 个角色、21 项导出、27 处绑定、8 份选定原件、203 个完整 CSS 块、59 条语义约束及 333 个 artifact；禁用 collector 后复核与 37 个篡改反例通过。十种 Quick 效果逐项确定八个实际参数编辑器及两个 null 内容，颜色草稿／Save／Cancel、Wave 挂载、Meter stepper 和 Advanced Profile 选择分别追到 MW serial Profile storage 与 LE 的真实软件消费者。layout／volume／lighting 三片去重为 308 个 AST、577 个 CSS 块；与另三份前端片共同去重才是 514 个 AST。当前 manifest 的即时硬件同步为 false、退出同步为 true，不代表原值快照、设备 ACK 或完整 native／frame／退出释放；其他产品、真实 DLL／设备和 Rust／窗口验收仍未验证。

宿主报告的 [lighting_frame_lifecycle](../../.analysis/results/host-current/local-audit.json#lighting_frame_lifecycle)另核 48 个完整源码 owner、76 个原生函数和 350 个 artifact，禁用 collector 后独立复核通过，43 个篡改反例全部拒绝；与路由或既有灯光片的交集未相加为总覆盖。3942 Marci 的 40ms 最新帧替换、781 字节 OutputReport／91 字节控制报告、native 未初始化尾部与 JS 零尾、void callback／undefined 返回、同步 WriteFile 计数分支及 direct exit 恢复均有实际边界。SynesthesiaStop 仅核实际 JS 启动与结果消费者，helper 内部、跨层静止和全部恢复未闭；当前 Rust 的 91 字节 writer／PID4115 owner 不代表 Marci 781 字节生产链已实现。详见[帧与退出](../products/shared/dynamic-lighting.md#3942-marci-的引擎帧返回值与退出恢复)，实际 DLL、设备及窗口仍未执行。

[Sensa 停止链](../products/host/sensa.md)另复用同一当前官方 1.0.0.49 包内的 [HapticService 捕获](../../.analysis/results/native/cdd88876f31903e894d681732460adbb82e596df3b7caa747fd1489dc85a3eec/capture.json)与 [RazerProvider 捕获](../../.analysis/results/native/62465547a378d65b03e5ba2b68b1d17ed0bcfd16bdf84c9f4e179ab545b1c06b/capture.json)。独立重读原件、函数索引、完整 chunks 和逐条指令，分别核 32／13 个函数、9,021／1,075 条指令及 41,543／4,712 原始字节；509／1 字节未枚举范围原样保留。另核十项控制表、三处 mixer／StopProgram 槽、49,953 字节内嵌 HTML 与四个完整 JavaScript AST 所有者，静态解析没有执行脚本。HTTP `stop_synesthesia → 0x87`、两工作线程等待、ProviderClean、151 字节部分改写的共享内存消息和按程序名移除记录分别有结果边界；HTTP 200、内部 true、SetEvent 和本地通知均不能证明设备停止。该包内链与仍缺原件的 SynesthesiaStop helper 分别记录，不合并为已闭合的全局退出、Rust 实现或真实 DLL／设备验收。

沿该链另核 [API 捕获](../../.analysis/results/native/cc2a78f8c9aeae01a5b2afb1ddac301b2fee11ca2be07824715060aadfe1f92e/capture.json)的 40 个函数、provider 新增 10 个函数和 [HAR 捕获](../../.analysis/results/native/ed774be3b3fac9d3dad0d75c8ad2e95d019fec1c4720b77957ce434e468c807e/capture.json)的 3 个函数，以及 API／provider 的 19 个实际槽与导入／导出绑定。按原件 SHA＋RVA 合并后为服务 32／provider 23／API 40／HAR 3，共 98 个完整函数、14,403 条指令和 63,143 原始字节，537 字节未列为指令。HAR 删除仅移除记录并调用可选全局回调；设备代理析构只释放名称／引用。下层 RPC／FFI 保存绑定 proxy 地址的两份回调，proxy 析构与全局 `Terminate` 的释放时机不同。已核到 RPC join 与 FFI 析构各自的所有者，HAR 可选回调注册者、全部并发／异常和硬件停止确认仍未闭，不据静态引用关系声称实际悬垂、设备停止或功能对照通过。

[Sensa 的 FFI／RPC 深层清理](../products/host/sensa.md#ffi-的宿主回调与注销边界)另核同一 API 的 85 个函数、90 个 chunks、7,122 条指令、29,262 原始字节及三张 48 字节虚表，357 字节未枚举范围保留；这个集合含旧上下文，不能与 98 相加。日志、设备表、命令表和 JSON 成员分别有实际释放消费者，真实空回调指针会被拒绝而保留旧回调。宿主输出同步借用字符串／结果缓冲，不证明宿主异步完成。RPC 退出事件只在调用间检查；轮询 join、服务启动线程 join 与 runtime 在途调用分别处理。`Terminate` 先释放 API，再析构 RPC 服务；监听使用 DontWait=1，注销使用 WaitForCallsToComplete=0，析构也不消费停止失败的 bool，不能写成已经排空所有调用。完整并发、宿主异步收束、HAR 注册和设备确认仍独立未验证。

同链的 native → 宿主 → renderer 又核 33 个函数、33 个 chunks、6,014 条指令和 27,186 原始字节，89 字节未枚举；实际实例 finalizer 的注册 owner 也包含在内。六份完整 JS 静态解析无错误，38 个完整 AST 节点边界独立复核；五个 packed 成员与 ASAR／完整性条目一致，unpacked node 的签章差异仅按获取索引重建打包前内容，不冒充签名校验。跨线程调用等待的是主进程同步回调，而该回调只向 renderer 发送；renderer 两层不返回／等待命令 Promise。WithResult 未安装时的 getter 可返回 0 且不写输出，不能据状态码宣称读到硬件值。HAR 的全局回调读写已追到四个函数，但缺少当前包未携带的 HidProvider 原件及真实注册者；全局释放、在途队列和实际 DLL／设备确认继续未验证。

命令 vtable `0xfa948+16` 的目标另在[定点补查](../../.analysis/results/native/cc2a78f8c9aeae01a5b2afb1ddac301b2fee11ca2be07824715060aadfe1f92e/supplements/e5d88ab5500bd5f3/capture.json)保留 11 字节、三条 IDA 独立解码指令及操作数，原 PE 与构造的成员／对象地址共同证明尾调用。IDA 的 decode_insn 能识别三字节 `48 ff e0`，create_insn 仍因前缀的数据项划分失败；sanity 返回 1，不能误报为不合法指令，私有处理器显示选项的尝试也未解决。全部失败尝试保留，最终 `capture_complete=false`／无完整 Hex-Rays 函数，原 capture 与原 IDB 摘要不变。原始字节、静态语义与函数化完成分别记录；精确匹配的边界识别由 `NativeSupplementTests` 核对每处字节变更、截断、额外字节及错误架构，不把静态解码计作额外完整函数或实际执行。

[Sensa 新增设备的输入与通知](../products/host/sensa.md#adddevice-的输入登记与读取消费者)另核 API／provider 的 59 个完整函数、59 个 chunks、5,865 条指令及 23,662 原始字节，342 字节未列为指令；该集合含复用，不与前述数量累加。AddDevice 先完整解析、后逐项提交，没有整批回滚，通知 ID 也不去重；外层 JSON null 文本与异常返回 C NULL 分开记录。原 PE 的 .pdata、40 B unwind、EH4 状态／清理表和 catch 返回另核，导入的 CRT handler 仍是外部依赖；解锁、销毁临时对象不等于恢复旧全局设备记录。FFI 覆盖记录、proxy 通知和 provider 同名跳过分别有消费者，HAR 返回值不能代替逐项设备确认。当前宿主未安装 WithResult，不能从成功码推定输出已被写入；全部并发、HAR 回调注册、Rust 行为及实际 DLL／设备对照仍未验证。

[Sensa 移除名单与 HAR 删除](../products/host/sensa.md#removedevice-的实际删除名单与异常边界)继续核 API 27、provider 13、HAR 28 个完整函数，共 68 个 chunks、4,319 条指令及 15,947 原始字节，261 字节未列为指令，含复用不累加。Remove 数组非 string 项跳过；先删除 FFI 节点再复制通知 ID，异常不会整批恢复。proxy 只查表并保留共享引用后通知，没有删除自己的 map；provider 先通知观察者，再删 HAR bodypart／connected-device 和自身记录。HAR 内层丢弃真正删除 bool，device ID=0 首个 processed 后跳过其余设备的删除调用；通知或 true 不等于全部记录／硬件已删除。原 EH4 数据区和 shrink-wrapped 三段 chained unwind 已独立核字节，清理临时对象没有旧记录恢复；完整并发、命令输出、Rust 功能与实际 DLL／设备停止对照仍未验证。

[InterHaptics 安装／卸载旧进程链](../products/host/firmware-and-installation.md#interhaptics-安装与卸载的旧进程退出链)复用当前 1.0.0.49 包的两份解释器及同包 nsProcess：两 PE 各 105 个完整函数、105 个 chunks、8,320 条指令、25,589 原始字节，插件为 11／11／425／1,280，逐条原字节独立复核，均无未枚举字节。这是捕获身份与字节核验数量，语义仅沿所选脚本消费者及插件路径，不称整个解释器全语义完成。两份 NSIS header 按文件中实际 LZMA 属性静态解压为 44,222／32,100 B，entry 表、变量字符串、正目标减一规则和完整跳转消费者分别核对；不执行 installer。helper 缺文件分支仍落到 ExecWait，等待退出后还需分别查进程与服务；Wait 非超时值后的 GetExitCodeProcess bool 未核验，不能据返回变量声称正常退出。安装与卸载的失败继续、50 次服务轮询、80 次目录重试、RunOnce／3010 及插件释放各自保留。SimpleSC 内部完整 SCM 与 helper／HidProvider 原件仍缺，实际安装、服务／设备变化和功能对照仍未验证。

[Visualizer 的 GUID 缓存与退出](../products/host/visualizer.md)另核 [同版 SDK](../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/capture.json)的 30 个函数、30 个 chunks、4,079 条指令及 18,072 原始字节，27 字节未枚举范围保留；实际 Core 包成员、EAT／IAT 和对象布局均另核。40 字节 map 节点仅保存独立 56 字节管道对象的裸指针；最终节点／桶清理没有释放这些值，失权关闭与 UnInit 也不是同一条分支。同步 WriteFile 不核实际计数或 ACK，返回 1 后仍可能因遗留错误值关闭 HANDLE；三次线程等待不核结果。此处界定所选链的静态行为，服务端消费、关闭期间在途调用和真实设备输出仍未证明，不声称整个进程已实测泄漏。

[Visualizer 帧接收与设备写入](../products/host/visualizer.md#帧接收实例路由与设备写入)继续核 DeviceManager、Camy 和 E782 三份原件的 78 个完整函数；按每模块的实际地址区间去重为 172 个 chunks、12,932 条指令及 44,598 原始字节，所选 chunk 无未枚举字节。跨函数共享尾块不重复计数，SDK 的五个发布函数只作复核。GUID→instanceId 管道转发、设备访问共享区的 0x10001／2／3、Camy 专用环缓冲与 91 B HID report 已接到实际消费者；各层丢弃 BOOL／字节数／帧结果、启动查询与帧读回的区别，以及超时后释放继续分别保留。当前所选插件没有逐帧硬件确认，设备访问服务处理器、键鼠共享区与 CJS JSON 接收尚须续追；静态接到 HID 调用不等于真实写回或释放验收。

同一报告的 `endpoint_volume_consumer` 再核 44 个完整函数、5,696 条指令、六表、37 条固定断言及实际 RTTI／variant／COM 调用与释放，173 项证据和六个篡改反例独立复核通过。与基础切片合并后共 502 个去重 artifact，分片交集保留且不重复计算覆盖。通知注册类型与实际 reader 的交叉、两次 OS setter 丢弃 HRESULT、self GUID 过滤前的 GLOBAL 刷新、普通注销与析构注销的不同结果消费者均有原字节依据；不得按名字简化成同名请求直接写 OS，也不能将所选释放函数升级为回调静止或恢复已验收。

自启主收据的 `application_uninstall` 另核 147 个完整 owner、89 个词法绑定、59 条静态断言和 36 条调用边，覆盖后台卸载 handoff、installer 记录／进度、文件和进程返回形状、宿主 CLI 消费及旧 SDK 独立接口。对象 truthy 与 result／exitCode、文件 bool 与 pending 对象判断、KA／xA 闭包、托盘 Fe 与窗口、日志写入与 Run 重建分别核实。静态类型错误路径没有执行厂商 JS，不能作为运行复现或功能对照通过。

`native_uninstall_producers` 再将两份保留官方 4.0.827 包的 CLI producer、真实安装／卸载事件与 HKCU Run 修改连接，保留 342 个完整词法 Lua 函数、94 个记录、20 个生命周期挂载、78 个引用、44 条调用边和 34 条断言。独立复核自行解压 NSIS／Setup Factory、核 CRC、实际字节和 Lua／注释边界，三个篡改反例全部拒绝；原 147 owner 切片及九个反例也继续通过。原参 `REG_SW` 的解析、生成后的卸载器、最终 Mn 传递清理与 OS 实际效果没有因此闭合，Lua 词法块不冒充 AST 或运行验证。

[691 固件包](../../.analysis/results/host-current/firmware-package-691.json)沿官方 manifest／完整包／静态提取／托管与原生消费者核验；原生 SDK、MCU 和 RY 分别保持自身架构、版本、ABI 与返回语义，同名模块不转移身份。完整 IL 方法与反编译文件保留，方法 header／code 长度及实际 PE 字节另行核对；这不冒充独立 IL 指令反汇编，异常表留在完整 IL 中。包内选中原件还直接从 ZIP 再读取、逐字节比较，不执行自解压程序。

该专项完整保留所选原生函数、IL 方法与异常表、反编译文件、独立窗口／九种语言和包内镜像资源；当前数量与输入身份以收据为准。PE 导出目录逐项核对三份模块的 34 个 named exports，资源原始字节、方法 header／code 和所选完整 EH 另行独立复核。HEX 的异常、补洞、callback 顺序及输出失败，WinUSB 的重试 buffer 改写与布尔返回，均核到完整函数和所选指令；bad-SP 反编译参数不作为 ABI 依据。两份 getline 的 112 B EH catch-all 与转换器无本地 catch 分层记录，30 个 filebuf 虚表槽及 locale／关闭顺序均核原字节；另一部署的 CRT 候选单列原件和完整函数，不提升为当前包实际依赖。Nordic CRC 的六个静态算例使用独立标准库公式及固定校验锚点复核，RY 扇区重建和报文静态向量也只验证已记录的算术与字节，不计为 Rust 测试、真实 DLL 功能对照或设备应答样本。

[安装信任链](../../.analysis/results/host-current/installation-trust.json)分别保留当前客户端／helper 的完整 IDA 函数与保留服务的 LLVM 完整 .pdata／leaf 范围；函数 chunk、逐条指令、表槽和数据区重新核原 PE，保留服务的来源与分析器限制不随客户端闭合。CN 解析、PE 与 ICO 的不同策略、EVP 间接方法表、文件路径与发布清理由完整调用链支撑，静态密码算法调用核对不等于真实签名功能对照或全部 CPU 实现已验收。共享页面补充报告独立核对真实挂载树、参数、原 CSS 与媒体字节；另开窗口和宿主消费者须继续沿当前 SDK／preload／host 路由追踪，不能按同名便捷 API 猜测传参。

[固件共享交互补充](../../.analysis/results/frontend/synapse-update-fw-current/page-interactions.json)已将所选 SDK 激活、preload 通用派发、policy=3 创建、页签激活／关闭与目标设置／迁移／App Explorer 消费者连接，分别保留完整词法所有者和 ASAR／官方部署来源。动态 SVG 通过既有官方共享索引边、获取收据及实际字节复用；无新增网络观察。DFU 的 payload 解析、事件路由、进度拒绝与异常结算各自保留，避免把所有无效事件统称为立即失败。生成后回读之外，独立校验完整 AST／JSX 边界、平衡 CSS、复用主报告引用、目标获取绑定及宿主 ASAR 切片；仍不执行厂商代码，也不将输入获取、静态语义与实际窗口／DLL／设备验收合并。

前端获取在单个依赖批次内并行静态枚举独立 JS，遍历及收据发布仍由单一所有者处理。每份 `chunk-references/*.capture.json` 保存确切输入路径／字节、Wakaru 字节身份、参数、完整 stdout／stderr 摘要、退出码和输入前后校验；只有这些身份全部吻合才复用，不以旧输出文件存在代替验证。复用非零退出或 null 枚举仍保留原失败／未解状态。产品族获取合并发布原件索引，避免每产品反复重写完整索引；获取收据与完整分析、语义和实现验收继续分开。

宿主应用 native 资源经 `acquire-native --include-host-app-resources` 与设备资源统一获取。来源必须是实际稳定 HTML 的 manifest link、索引吻合原件及稳定摘要，不能仅凭缓存名或手填 URL；新 native manifest 与页面部署字节漂移时保留失败并要求沿原流程补取页面。设备 manifest 按真实 PID 路径核对，`host_app` scope 按稳定宿主 HTML 声明核对，二者都要求官方 HTTPS。`extract-native-packages --scope host-apps --workers 2` 复用完整静态容器提取，本轮 139 份 PE／容器候选中 16 个容器提取 676 个文件、错误 0；普通 PE 的无归档结果不算安装分支闭合。

`inspect-corpus` 静态解析当前 installer HTML 实际声明的 JS，保留完整下载 owner、数值常量声明、AST 范围、工具版本及实际 SHA。源码以 `Math.floor(.1*(1024*1024))` 得到 104,857 字节分片，`pindex`、循环偏移、请求 start／end 和 Range 绑定均须吻合；再将每份缓存与完整官方资源的精确字节区间比较。本轮 15 份通过的分片在 [corpus](../../.analysis/results/corpus.json) 独立记录并从完整模块分类排除，原缓存保留，不拼装或转移整文件身份；完整资源的当前来源另行核对。策略、区间、快照或父资源来源失败均进入统一 gaps。

本轮 corpus 当前记录 native 1,941、CLR 683、混合 9、其他 39，分类失败 0；500 个官方来源根连接 39,995 条容器边。30 份 native 原件的当前来源仍未证明，包括 8 个 manifest mismatch 资源及其独有后代、两份旧 quarantine、两份已安装 DLL、一份安装时生成的 GMS uninstaller 及本机保留的提升服务 EXE；重复获取不能提升这些原件的身份。此来源盘点独立于尚在推进的全函数捕获、语义及产品实现。

host 子应用的 [当前部署审计](../../.analysis/results/frontend/host-applications.json) 逐文件核对原件，保存各自 manifest 版本、build、gitMetadata、dependencies／resourceManifest；native host 元数据只作上下文。窗口 URL 保留完整 factory／ESM 所有者及表达式，常量拼接沿实际 webpack require／export 解析，拒绝重赋值及运行时 location；运行资源与 source map 缺失分别计数，资源闭包不冒充页面与功能完成。

相对 URL 依据实际 HTTP final_url 及 HTML base 解析，引用边仍归属请求原件；收据同时保存请求身份、响应地址、解析策略和当前 requested_urls。裸目录路径先被保留为文件后遇到子资源时，子资源使用显式 per-URL 存储命名空间，原响应字节不搬移或改写。旧错误基址产生的失败记录与当前请求缺失分开，所有保留原件仍逐项校验字节。

资源准备使用现有统一 `prepare-resources`、`verify-resources`、`prepare-dashboard-images` 和 asset pack 命令。图片结构／像素、GPUI 实际 CPU codec、资源包读取和窗口显示是不同验证层级。转换工具不执行 React，仅处理已支持字面量 SVG／CSS，保存原始及派生 SHA。旧文档的资源与测试数量不能当本轮通过结论。

GPUI 修改前读取用户指定目录的两个 SKILL.md 及适用机制／组件参考，核对 Cargo.lock 版本；页面布局、控件、状态及交互由当前原件证明。

主程序在 GUI 初始化前安装标准库 panic hook，日志由 `razer_storage::store_path()` 定位到同目录的 `panic.log`，Windows 通常为 `%APPDATA%\razer_ui\panic.log`。记录进程、线程、时间、Rust panic 位置／内容及调用栈，并保留原 hook；日志不可写或锁不可用时尽力输出到 stderr，不阻止启动或吞掉 panic。它只补充 Rust panic 诊断，不能保证捕获原生 failfast、abort 或系统直接终止，隔离服务 worker 也不使用这份主程序日志。写入及真实崩溃记录受窗口运行限制，静态核对与编译不算运行验收。

输入获取、静态恢复、语义、Rust 实现、UI／服务连接、纯／模拟验证、运行验收分别记录。证据 SHA 不作为产品兼容门禁。共享协议逻辑独立于 OS，HID、COM、驱动、服务和系统适配保持明确边界。

缺失原件、间接调用、ABI／协议分支、未实现本地链进入统一 `.analysis/results/gaps.json` 并驱动后续实现。账号、云同步、远程下载／更新、在线目录等联网产品功能移除；loopback、本地 socket、固件文件传输、DLL 替换、服务生命周期、本地预设仍须实现。纯／模拟测试不冒充设备写回；本地 DLL／导出、服务及设备功能测试按根目录已授权的来源、架构、ABI、清理链核实和只读优先边界执行，可恢复写入保留原值、恢复与回读结果。应用窗口、厂商 JS、无关 helper 和安装器仍不运行；测试实际调用、返回、比较及未验证范围单独记录，不能由加载成功或单次 getter 推导整页验收。
