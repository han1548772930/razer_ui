# 静态逆向工具与核验边界

IDA 路径：`E:/IDA`（9.1）、`D:/ida/IDA Pro 8.3`（8.3）。GPUI 技能读取 `.agents/skills/`。各份捕获记录实际工具版本，不能跨版本沿用验证结论。

批量原生捕获的 `--workers` 支持 2–4，默认 4。线程池负责调度，每个新模块运行独立 IDA 进程，私有数据库与输出按原件 SHA 隔离；相同字节只排一次。并行批次的共享配置在首次需要新捕获时加锁准备一次，由协调线程按稳定模块顺序发布统一收据，单项失败保留且不阻止其余项。已核验的完整捕获直接复用；全为复用时不启动 IDA 或准备配置。`test_evidence.CaptureStreamingTests` 的 17 项测试使用事件屏障核对实际重叠、上限、失败隔离和稳定发布，并覆盖全复用及非法并发参数，未以睡眠或放宽断言替代顺序检查。

所有批量 Python 入口都必须启用至少两个 worker；类型表生成也使用线程池；当前捕获、前端恢复、宿主资源获取、产品族收集、资源提取、仪表板资源准备、资源所有权审计、Windows PE 静态检查和本页 gaps 校验均使用有界线程池。前端恢复即使只指定一个来源也使用至少两个线程，捕获、恢复、资源审计和安装包准备入口拒绝 `--workers 1`，避免逆向批次悄悄退回串行。并行任务只返回不可变结果，由单一协调者按输入身份排序、校验原始快照后原子发布；任何一个任务失败都保留失败收据，不能以其他线程成功替代它。

当前 `HID.node`（SHA-256 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`、405,704 字节）的 [实际捕获收据](../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/capture.json) 使用 IDA 9.1、Hex-Rays 9.1.0.250226、Python 3.11.9，完整捕获 1,641 个函数；RVA `0x30888` 的反编译失败及完整汇编保留。其他 IDA 8.3 捕获的函数数与产物摘要不能套用到这份收据。[HID 查询](../../.analysis/results/native/chain-audit.json) 中的工厂、回调、动态导出与共享 worker 链须分别对照其引用的完整函数及实际产物身份；指定查询没有未解边不代表全部 N-API／Promise、产品协议或生命周期已经闭合。当前 GetFeature／GetInput 的完整函数分别位于 RVA `0x17ac0..0x17c43`、`0x17c50..0x17dd3`，Windows 返回长度均为原生已读长度加一；Rust 的 source-host collection 适配与产品重试／取消仍须逐调用方校验。

`capture-corpus --source retained --ida-root E:/IDA --include-path '\.(dll|node|sys)$' --workers 4 --no-refresh-gaps` 本批 884 个不同字节的非 EXE 模块已终态：869 份经校验复用、15 份新捕获，共 2,610,555 个函数，模块级失败为零。[统一原生队列](../../.analysis/results/native-corpus-capture.json)与[独立质量核验](../../.analysis/results/native-corpus-quality.json)分别保留输入、版本、捕获和复核身份；发布前另重读全部 884 份原件及捕获清单摘要。181 份旧 schema 缺少可选完成字段，按完整且稳定的函数清单核验，不补造该字段。

241 个模块合计 869 个函数没有成功生成伪代码；全部失败函数的原始块和 1,622,453 条已枚举汇编项已核对原 PE，共 10,014,652 字节 raw chunks。152 个失败函数的汇编项没有覆盖其中 7,185 字节，这些原始字节仍完整保留。队列另外排除 1,066 份输入，其中 995 份 EXE；它们不计入本批完成数，已有独立捕获的输入须按各自收据另核，不能全部当作尚未捕获。该索引队列按 DLL、NODE、SYS 取样，不排除当前产品范围内的主机、服务 EXE 或本机安装生命周期静态逆向；它们按真实调用链和已有独立收据继续核对。禁止的是运行主应用、安装器及无关 helper，不是读取其静态证据。`capture_complete=true` 与 `full_corpus_captured=false` 同时成立，不能将本批终态记为全语料、功能语义或实际 DLL 对照完成。

另有十份服务 EXE 已逐项完成完整产物复核：1442／3942 路由服务，1427／709–711／1398 属性服务，以及 1443／1411／1339／694–696／1330 的 companion 服务。十份共核对 182,605 个函数文件及 90 个输出的实际摘要，原件与 capture 前后不变；其中 1427、709–711、1398 各有七个 Hex-Rays 未决，其余七份为零。各自的 [native 捕获目录](../../.analysis/results/native/)保留终态、版本、函数和失败身份；这些额外收据不改写上述 884 模块队列，也不提升为整个服务的语义或运行验收。

2026-10-24 另开了一个只针对安装包装层的真实双 IDA 批次：`capture-corpus --source retained --ida-root E:/IDA --include-path 'RzComDriver_v23\\.0\\.6\\.0\\.exe$' --include-path 'RazerChroma-Web-v4\\.0\\.827\\.exe$' --workers 2 --no-refresh-gaps`。协调器先校验原件 SHA，再同时启动两个独立 IDA 进程（批次日志记录 PID `16356` 与 `6380`）；两个模块均为新捕获而非复用，各 105 个函数、0 个捕获失败。独立批次收据另存为 [native-corpus-capture-chroma-20261024.json](../../.analysis/results/native-corpus-capture-chroma-20261024.json)，原件和完整捕获分别为 [RzComDriver](../../.analysis/results/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/capture.json) 与 [RazerChroma-Web](../../.analysis/results/native/1a7eecf362e5de07c9bfd799bcccbdf3327740ffb3d713bc4c3649110bb94586/capture.json)；对应的 [RzComDriver IDA 日志](../../.work/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/batch/ida.log) 和 [RazerChroma-Web IDA 日志](../../.work/native/1a7eecf362e5de07c9bfd799bcccbdf3327740ffb3d713bc4c3649110bb94586/batch/ida.log)均保留 `STATIC-CAPTURE 105/105`。

这两个 PE 的版本资源分别是 Razer ComDriver Installer `23.0.6.0` 和 Razer Chroma Setup `4.0.827.0`。完整反汇编显示它们是 NSIS 自解压／完整性校验／临时文件／子进程包装层：`start`、`DialogFunc`、`StartAddress`、`CreateProcessW`、100 ms `WaitForSingleObject` 轮询、`GetExitCodeProcess`、注册表与文件操作均已捕获；没有从这两个包装器得到 Chroma 帧、HID 报文或设备 ACK。`LoadLibraryExW`／`GetProcAddress` 只证明通用动态解析通道存在，不能推断某个厂商 DLL 已加载。范围内解压载荷、安装器消费者及本地文件／服务安装生命周期继续静态逆向，既有捕获按原件身份复用；不运行这些包装器，也不把安装包装层当作设备后端或提升 Rust 实现、运行时 DLL／设备对照状态。

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

普通 Rust 验证先以 AST 解析模块和字面量 item `include!`，保留实际源码、模块名、导入及局部窗口调用依赖，再筛选测试；被包含文件中的窗口用例同样在执行 Cargo 前拒绝。范围收据记录 `selected_tests`、来源数量和 `unresolved_includes`。动态路径、表达式 include、其他宏展开、类型解析和 cfg 求值仍受限，不将未解析项称为完整验证；实际执行数量与静态枚举分别核对。Matcher 的 8 项漏选测试已接回，当前页面套件实际执行与枚举均为 18 项。

官方 DLL 对照维护器使用 Windows x64 Python 标准库 ctypes、unittest 和项目内 PE 检查模块；不执行 JavaScript／N-API 初始化。纯测试通过统一 verify-analysis-tools 的输入快照与收据所有者执行；定向套件复用 test_evidence.run_verified_suite，并由 verification_environment 持有临时目录，不加载真实官方 DLL 或打开设备。实际功能对照使用 compare-local-service-version、compare-local-app-files、compare-local-lighting、validate-connected-devices 与 compare-connected-hid 的已核实分支；新增功能须补齐对应入口和收据，不能由现有 HID 重放替代业务比较。

compare-local-lighting 将已核实的原件 SHA／PE／ABI、110 个完整函数与 14 个 vtable 原字节绑定到 4.0.896 发布获取收据、完整资源声明及实际 metadata_files。稳定 latest 选择来自 10/29 的双采样部署审计和获取结束时的六端点复核；对照本身不联网，不把保留审计日期写成本次在线最新性。当前[灯光对照收据](../../.analysis/results/lighting-runtime-comparison.json)追加的 attempt 2（共三次，前两次完整保留）已实际执行公开 JSON 软件接口：Static 与静止 Wheel 各九像素逐值相同，1,839 项 Rust／工具输入固定，Rust 前后快照一致。三个隔离进程均正常退出并确认等待，无超时或清理错误；两场景 disable、销毁 engine/device、terminate、停止 timer 和注销 callback 均完成。11 项直接／动态系统依赖及 37 项已加载模块闭包按 System32 核验；module、callback 和已核实的 COM／MF 所有者保留到隔离进程退出。Fade、动态时序、其他参数、物理设备、窗口和原生调用中取消仍未验证；该局部实际一致不代表整个灯光功能完成。

该入口也将来源校验失败和父进程取消追加到同一收据，取消先完成子进程等待并保留已取得的输出，再向调用方传播。输入身份覆盖工作区 Cargo／工具链配置；三次运行共用冻结环境副本，编译参数与 wrapper 环境只保留存在性、长度及 SHA，结束后复核，避免公开配置值。

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

[当前官方 host 获取](../../.analysis/inputs/host-current/releases/4.0.896/acquisition.json)确认稳定版 4.0.896。28 项声明中下载 8 项、299,532,120 字节，核验复用 20 项、34,850,073 字节，六个端点前后稳定。[版本范围提取](../../.analysis/results/host-current/releases/4.0.896/inspection.json)完成 15 个容器并正式索引，无当前失败；4 个普通 PE 保持 `extracted=false`，不作为归档载荷完成。

[当前 ASAR 捕获](../../.analysis/results/host-current/asar/e7a524aca1771251ca4f851441a97cac1c33929891b291ed34deb7adfc1831ec/capture.json)对应 102,875,031 字节原件，完整提取 10,108 个文件、未决为零。六份当前原生捕获合计 16,000 个函数，全部汇编完整；BLE／HID 基础捕获各有 1 个伪代码失败，其原件与数据库保持不变。两份独立私有副本的显式类型补查取得完整伪代码并通过核验，私有副本已按正式清理收据释放。统一 gaps 按六个捕获所有者分别记录语义审计和实现验收 pending；静态捕获、补查成功不等于 Rust 或实际 DLL 功能完成。

[1422 部署复查](../../.analysis/results/frontend-revalidation/0de5320372280283bc23b259f7d8a33465e08da52447f9012de0d1c90f136cd6.json)记录 entry、asset-manifest、manifest 与共享 manifest.js 四项变化，旧 main 可访问不能证明它仍是当前挂载。[当前 1422 快照](../../.analysis/inputs/frontend/audio-1422-snapshot-ed4fa81d/acquisition.json)请求 534 项，取得 503 项、失败 31 项，保留 37 个未决引用；双采样稳定且获取期间来源当前，部署原件无失败。语义、实际挂载与完整依赖闭包继续逐项核对，不能从获取稳定推定已完成。

已安装输入快照及其对应官方包的静态解包原件，与该快照中的主 EXE、ASAR、mapping_engine.dll、simple_service.dll、HID.node 实际 SHA 一致。该快照 ASAR 为 `b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6`，静态提取 10,076 个文件，0 个提取未解项；字节与提取核验仅适用于这份已安装输入快照，源码图、外部资源与 ABI 仍有未解项。4.0.896 的当前 `e7a524aca177…` 捕获按前述独立收据记录。

[已安装输入快照对应的 host JavaScript 捕获](../../.analysis/results/host-current/javascript/capture.json)适用于上述 `b2ce8c54dc5c…` ASAR 及其对应官方包，保留 230 份 JS／CJS 和 693 个恢复产物。106 个引用 source map 未包含在包中；unpack 退出 0、validate 退出 1。原始文件是该快照的语义与字节证据，可读文件和合成 import／export 只辅助查询；当前 4.0.896 `e7a524aca177…` 的 JavaScript 语义和依赖闭包按其自身输入另行验收。

`audit-host-local --query macro-recorder` 从当前 PID192／756 middleware、Macro 和 background-manager 原件保留完整 AST owners 与真实 UTF-8 byte 区间，并复核四套既有 Wakaru 捕获的全部原件、产物和分析器字节。完整捕获中的 validate 非零结果与获取缺失不升级为成功；统一 [local host 查询](../../.analysis/results/host-current/local-audit.json) 保留软件录制与 OTFM 写入／移除、PID756 非独占预触发与全局广播、background OTFM 全局锁与完整状态机、An 初始扫描与 q 事件字段差异、接收器缓存最后匹配、serialNumber 筛选及当前功能注入配置。JSON 记录不会补造 getHWModule；原生 transport、完整页面与真实运行验收仍未闭合。

`audit-host-local --query window-lifecycle` 在同一查询收据保留 25 份字节核实的完整宿主 JS 文件及 117 个去重 AST 所有者。输入逐一与 packed ASAR、官方包成员和获取清单核对；窗口／页签身份、120000 ms 默认休眠与显式休眠、URL 内存存储、Windows 原生事件的 JS 消费者、退出拒绝与超时、已登记 HID 恢复命令及 WebSocket／BLE／日志停止分别记录。BLE endpoint 原件的 797 个函数、9 份汇总产物和 16 个完整行为所有者另经实际字节校验，其 UTF 失败的 7 函数 supplement 保留原失败、类型依据和数据库不变性。该查询为静态操作，不执行 JS 或 DLL，不把异步发送、FreeFFI 返回 true、三秒超时或 app.exit 当作释放成功；`refresh-gaps` 保留其失败、剩余调用链及独立实现／实际对照的未验证状态。具体顺序、源码缺陷和对照边界见[窗口生命周期](../products/host/window-lifecycle.md)。

`audit-host-local --query profiles` 在同一收据新增 [profiles 字段](../../.analysis/results/host-current/local-audit.json#/profiles)，`--verify-only` 只独立复核而不收集或发布。当前两套部署和五份宿主原件保存 43 个完整 AST 所有者、32 个选中调用者、439 个原件及 2,157 个校验工件；完整捕获中的两个失败阶段继续保留。54 项 `test_host` 通过，其中 15 项覆盖本查询的字节、完整边界、绑定、获取变更、19 类失败／取消／完成度篡改与发布保护。正式报告原有十个顶层字段逐项保留 canonical 身份。Add 的 action 吞发布异常、Browse／drop 不等待所有添加、刷新早退、扫描结果未关联、重定位仅写 id/path、拖放监听的清理行为分别见 [Profiles](../products/host/profiles.md)。这些维护工具测试不等于 Rust 功能或真实 DLL 对照；全部 scanner、设备自动配置档写入和窗口仍须逐链验证。

`inspect-host` 对多个独立归档使用 `--workers`（2–16，默认 4）并行运行只读列表／提取，按输入 SHA 排序由协调线程合并 `inspection.json` 和索引；`analyze-host --managed-scope ... --workers N` 对独立 CLR 程序并行调用 ILSpy，每个 worker 只写自己的 SHA 输出目录，协调线程按稳定输入顺序发布 managed capture。`analyze-host --resolve-managed-dependencies --workers N` 同样按程序集并行补充依赖，失败、退出码和诊断逐项保留。

`analyze-frontend --input ... --workers N` 在完成统一 unpack／validate 后并行提取各 source map；命令记录按原始输入顺序发布，恢复器仍由单一协调者校验完整原件。`prepare-local-protocol-tables` 的五个固定输入、`prepare-mapping-capabilities` 的各鼠标挂载输入和 `prepare-mapping-producers` 的各产品输入均使用 2–16 线程（默认 4），只在所有结果收齐后写 Rust 输出和 typed-data 收据；参数为 1、非整数或超出范围时立即失败，不能退回串行生成。

`audit-family-function` 的 `mouse-dpi`、`mouse-customize` 和 `keyboard-artwork` 查询也按产品输入使用 2–16 个 worker（默认 4）；每个 worker 只返回局部 AST、布局、解析和失败结果，由协调线程按产品身份合并并重映射共享几何索引，统一的 Wakaru normalize 仍在合并后执行。其他单一输入的静态查询保持单 analyzer 阶段，不把一个输入伪拆成无独立边界的任务。

[Dashboard 重试安装与无存活恢复](../products/host/dashboard.md#重试安装无存活恢复与服务端状态)另核 `BoxItemDevice` 与 background-manager 的当前字节、`BroadcastChannel` 消息、`noAliveSignMW` 身份键、40,000 ms 存活检查和 `resuscitate`／`_openMW` 门禁。`retryInstall`、窗口重开、中间件握手、重新枚举及设备读回分别记录；消息发送、窗口关闭或后台 Map 清理没有 Dashboard ACK，也不能当作 HID 写入或安装成功。此段静态 AST／字节证据未运行厂商 JavaScript、后台服务、DLL 或设备，Rust UI 与真实对照仍未验证。

Profiles 的 Windows 原生枚举／图标另复用 [SysUtilsNative 完整捕获](../../.analysis/results/native/01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318/capture.json)，独立核 38 个函数、38 个 chunks、7,525 条指令及 36,131 原始字节，1,078 字节未枚举范围保留；九份汇总输出也按摘要核对。同字节的官方主机包成员、实际版本、导出／导入及注册表根表分别确认。枚举仅查询 HKLM／HKCU 下两种 Uninstall 路径，不补成 UWP／快捷方式扫描；图标缓存键、GDI+／PNG／base64、临时文件、短读和失败释放分别记录。这是所选 Windows 静态链，不能扩写为扫描服务、macOS、Rust 或实际 DLL 已验证。

[GMS 续追](../products/host/profiles.md#gms-服务发现排队添加与任务退出)再核同 DLL 的 7 个函数、966 条指令及 4,061 原始字节，5 字节未枚举；服务发现读取注册表，扫描实际经 JS／SignalR 进入托管服务。当前包的三个程序集及同字节 aliases、22 个注册策略源码均核对捕获摘要；另独立解析 CLR MethodDef 与 tiny／fat 方法头，核 46 个方法的 token、RVA、头长与 code size，以及七处原始 IL opcode，未将其宣称为全部托管 IL 逐指令验证。OnStartScan 的 async void 正常主体没有 await 挂起点，异常仍经 builder 脱离外层 Action；WaitAll 的超时 bool 被 pop，finished 通知早于 stopped 和落盘。两把 Promise 锁的反向顺序／重入可形成条件等待环，实际触发与服务时序未执行。分段收集、请求关联丢失、策略停止粒度、取消与资源收束须按文档分别测试，不能由服务发现或静态正常返回证明成功。

同链[本机扫描的路径与去重](../products/host/profiles.md#本机扫描的路径去重与结果发布)继续核七个策略、公共基类和两份 SQLite reader，十一份 C# 输出字节与既有清单一致；25 个所选 MethodDef 身份／方法头独立核对，包含与前片重叠的方法，不直接相加为新增覆盖。两种 Registry 策略的 64 位门禁后实际调用 32 位 getter，由四处原 IL 与 MemberRef 名称共同证明；共享 AddFoundGame 的七处索引／通知调用也另核。不同 HashSet 比较器、数据库失败后返回前缀、无效 GUID、先插入后补字段、两张索引的非事务更新和通知早于保存分别保留。Encoding.Default、HMAC-MD5 大写 hex 与 Guid(string) 属于身份语义，不能擅自改用 Guid(byte[]) 或通用正规化。底层依赖、全策略取消／并发、Rust 与实际功能对照继续未验证。

同链[解析与发现通知](../products/host/profiles.md#解析依赖与发现通知的完成边界)另核 Lib 18 个、Common 9 个、服务 EXE 1 个所选 MethodDef 身份／方法头、完整 IL 及相关输出摘要；集合与此前有重叠，不相加为覆盖总数。路径字典实际由 Controller 注入 CurrentCultureIgnoreCase tuple comparer；服务注入名为 NewtonsoftJsonSerializer 的类实际使用 System.Text.Json，字符串／文件／固定 4096 B 字节入口的参数和错误边界不同。Mihoyo 字符分帧、逐条 JSON catch 与外层 I/O 失败、发现广播先于缓存更新、async void 发送挂起、缓存缺分区仍可回 GS_SUCCESS 及 void 保存失败分别保留。所选构造／调用另核原 PE 字节，完整 JSON／加密存储依赖、所有并发交错、Rust 行为测试与官方实际功能对照仍未由此完成。

[Steam、BattleNet 与 Epic 的本机输入](../products/host/profiles.md#steambattlenet-与-epic-的实际本机输入)另核 Lib 的 39 个、Common 的 4 个所选 MethodDef 身份、原 PE 方法头及关键调用字节，并重新核对两套程序集的全部 246／107 个捕获输出摘要；方法集合有交集，不累加为全策略覆盖。Steam 的两个 bool 结果被 pop、停止库循环后仍可能进入联网补全、数据库适配器丢弃序列化返回值、BattleNet 日志单次读取及 64 位门禁后调用 32 位 getter、Epic 先读两列表却以不同列表作门槛与遍历均分别记录。联网目录与匿名登录属于本项目删除范围，不能因此省略本机文件／数据库／图标和结果发布链。底层 KV／SQLite provider、完整取消与发布并发、独立 Rust 行为测试及实际 DLL／服务对照仍未由此完成。

[Arc、Bethesda、EA、GOG、Mihoyo、Riot 与 Uplay 的本机扫描](../products/host/profiles.md#arc-bethesda-ea-gog-mihoyo-riot-与-uplay-的实际本机扫描)继续核对剩余七个注册策略及其公共 Registry／Path 适配器。选定 ServiceLib 44 个、Common 12 个、服务 EXE 1 个共 57 个方法的身份／方法头、调用与分支摘要已独立复核，并保留 68 个异常区间、55 行定位表和 1901 个调用／操作计数；这些集合与前段重叠，不相加为全策略覆盖。实际输入包括 HKLM Registry32 两视图、Bethesda reader、Arc／EA／GOG 注册表字段、Mihoyo gamedata、Riot rc_live／metadata YAML、Uplay 本地 configuration 文本及路径适配器；逐项停止只位于候选边界，预读取、枚举器和 AsyncTaskMethodBuilder 异常不会自动回滚。完整策略取消／并发、Rust 行为测试及实际 DLL／服务对照仍未验证。

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

每份 `.analysis/results/native/<sha>/capture.json` 保存输入身份、架构、实际 IDA processor、image base、函数边界／RVA、函数块原始 bytes、IDA 枚举的汇编项、成功取得的伪代码、局部类型、结构、xref。每个输出独立 SHA 校验。`--resume` 仅在输入、架构、processor、明确的反编译器可用状态、IDA／Hex-Rays 版本及捕获工具摘要一致时复用函数，并核对函数文件摘要、边界。

没有适用的 Hex-Rays 插件时，只继续静态汇编与数据导出，不调用反编译、失败对象或伪代码缓存 API；每个函数仍以 `RVA:hexrays`、`decompiler_unavailable` 和未验证伪代码状态保留。`capture_complete` 只描述静态导出完整，不能据此减少反编译未决。当前 IA64 原件可由已安装的 `ia.dll`／`ia64l` 处理器加载；实际捕获发现 66 个函数且没有适用反编译器，日志还明确报告该 processor 不支持 `.pdata` 解析，因此也未证明函数发现闭包。旧捕获中的 66 条 SWIG 异常和原始日志保留，不以新能力诊断追写历史或重复执行同一捕获。

`capture-native --exception-regions-only` 从已核验的 IA64 原件解析异常目录的 12 字节 RVA 记录，核对顺序、区间、bundle 对齐、可执行文件字节及 unwind 头的唯一文件映射，并与基础捕获的全部 chunks 比较。入口将基础 IDB 复制到独立私有目录，以安装版帮助定义的 `-a` 关闭自动分析；bootstrap 在任何 `auto_wait` 前分流，导出器再关闭自动分析并比较前后函数和 chunks 的完整快照。普通捕获和 MCP 模式保留原有自动分析顺序。只读 `decode_insn` 按实际返回长度推进，支持当前捕获中的 6／6／4 和长指令 12／4 步长，导出每个区域的完整原字节、指令、数字操作数和已有 xref，结果保存于同一原件的 `exception-regions/<selection>/`。

该模式要求实际 `ia64l`、`ia` processor、64 位与小端状态一致，并核对运行中的入口、capture／support／parser、processor DLL、IDA 内核、IDAPython 及所用 SDK 模块文件。原始工具字节按摘要保留于同一原件的 `tool-sources/`，计划、会话和结果绑定这些快照并在导出前后复核；基础 IDB 的实际身份参与 selection，变化后生成独立尝试，已有输出不可覆盖。此路径不修补 SP／ABI、不调用反编译器；只有函数／chunks 快照未变且自动分析保持关闭，才记录未创建函数和导出完成。基础 IDB、capture 和历史失败保持原字节。当前 71 个异常区域与既有 chunks 的独立覆盖核对保留 9 个区域、1,872 字节缺口；异常表可能描述拆分片段，完整解码不证明独立函数边界、unwind 语义或函数发现闭合，缺失区间和后续语义工作统一进入 gaps。

解码器返回 0 时保留确切未解码范围，再从下一个 16 字节 bundle 继续；游标推进不计为成功解码。当前补充尝试覆盖 71 个区域、34,736 个原字节，实际解码 34,676 字节，6 处各 10 字节仍失败。经已索引的官方指令手册逐位核对，这六个原 bundle 均为 template 0C，三个 41 位 slot 均为 0x8000000，分别表示 nop.m／nop.f／nop.i 0；该独立语义证明不改写 IDA 失败收据、不创建函数，也不提升 66 个原函数的伪代码或发现闭包状态。`reconcile-source-index --instruction-manual` 校验官方 HTTPS 来源、版本元数据、PDF 头、长度及完整字节摘要后登记手册，产品不依赖此分析输入。

IA64 的[静态契约审阅](../../.analysis/results/reverse-pending/review-sources/657105e758f6702691d1f4156168e987cbaac8e9963438647fcc107a57b0ffb8.json)另由 `tools/native_ia64_contracts.py` 逐项核验 66 个既有函数、91 个 chunks、71 个异常区间及 66 份 unwind 流；独立核对导入、重定位、描述符、固定表、完整可执行虚拟字节与额外复制函数。原始 66 条反编译失败、96 字节对齐内容及函数发现的原捕获边界完整保留；手工契约、反编译能力及匹配平台实际执行分别记录。

`bind-native-static-review --kind source-assembly` 通过 `tools/native_source_correspondence.py` 静态调用已核验的 `ml64`，独立解析 COFF 符号与 REL32 重定位，将 28 个 AVX512 owner／helper 的重组装字节与原 DLL 对应；[语义审阅](../../.analysis/results/reverse-pending/review-sources/a669e5958f5aacaf4d7ea8936b93807d5969be8482c26277f5eca160a04e4051.json)另保留 ABI、实际消费者、36 个计算分支、16 个后处理分支及 FMA 源操作数顺序。汇编器临时目录在成功、失败和取消时释放，来源一致不产生伪造 cfunc 或实际 DLL 执行结果。

`bind-native-import-forwarders` 仅接受[精确原始 IAT 跳转证明](../../.analysis/results/reverse-pending/review-sources/9bd12d141163e04dd7dacffc101c05187b55a9a931a8c165b2f489bfc0fbed8c.json)，核对完整六字节机器码、导入槽、变参类型及全部互反调用引用。旧尝试中的类型别名须有独立的 Windows ABI 原件与静态编译证明；捕获中断须覆盖全部原目标、完整 chunks、原错误与退出会话后才能登记历史核销。绑定由显式输入与 review SHA 选择，替换保留旧字节快照；统一[当前复核](../../.analysis/results/reverse-pending/unresolved-current.json)分开记录完整反编译、原函数上下文、机器跳转及静态契约，不提升 Rust、UI 或实际设备验收状态。

`audit-native-attribution --previous-audit-sha256` 仅在完整重新核验后更新输入索引绑定，保留旧审计全文和当前父子输入的完整索引行；函数映射、原始失败及其他证据不得变化。旧索引没有保留的完整行不能追溯宣称一致。历史引用须属于当前审计逐项核验的前序链，孤立同名快照不构成有效依据。

`reverse-pending --recheck-unresolved` 导入既有结果前核对不可变正文、冻结任务身份、唯一性、工具快照、统计与历史增量链。沿用项核对全部嵌套证据；本轮选定项先移除旧通过状态，再重新计算，允许有完整历史原件的正式绑定更新。缺失、损坏或跨任务结果不能被重新封存为通过。

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

同一报告的 [1442 Virtualizer／FIR 活动链](../../.analysis/results/frontend/audio-surround-audit.json#/native_virtualizer_fir_1442)另核 client 107／service 26 个完整函数、15,944 条指令、67,472 原始字节、9 个实际虚表槽、44 条直接绑定、5 项 descriptor 字段及 7 条来源获取边；91 字节未列为指令。客户通知和工作线程异常另核 12 项实际 runtime/FH4 项、15 个去重 PE 数据区及两个 noexcept 根。正式 `--native-virtualizer-fir check` 独立重读原件通过，`negative` 在禁用 collector 与原生收集 helper 后拒绝全部 82 个篡改反例。client 接受正的奇数长度、服务要求偶数，setter 的即时回调不等待服务 ACK，JSON 的未知 selector 在合法解析后可返回 0 且不更新；服务六个消费者忽略 DSP 配置函数的 bool。通知 mutex 的同线程重入实际抛 system_error(code16)，工作线程 noexcept 边界可在异常搜索阶段终止，不能由局部 unlock guard 推定外层继续或停止成功。以上分别说明本地状态、服务提交与 DSP 配置边界，不能合并成音频生效。完整异常／取消／并发释放、全部产品分支、Rust 功能测试和实际 DLL／设备对照仍未验证；既有报告字段的 canonical 身份保持不变。

[3942 VAD 生命周期片](../../.analysis/results/frontend/audio-surround-audit.json#/native_vad_lifecycle_3942)通过统一 `--native-vad-lifecycle capture/check/negative` 维护。44 个完整函数、2,116 条指令、9,151 原始字节、4 个 runtime 根、24 个 FH4 状态、10 个数据区及 45 处指令锚点逐字节核验；18 字节未枚举范围保留，55 个反例被拒绝。阻塞停止绕过 event 关闭、worker 返回被丢弃及 noexcept 搜索阶段终止分别记录；`supersedes` 精确引用旧断言，未改写冻结的历史核验器。详见[线程退出与所有权](../products/audio/playback-and-eq.md#3942-vad-线程退出与资源所有权)，实际 DLL／设备和完整恢复仍未验证。

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

[Visualizer 帧接收与设备写入](../products/host/visualizer.md#帧接收实例路由与设备写入)继续核 DeviceManager、Camy 和 E782 三份原件的 78 个完整函数；按每模块的实际地址区间去重为 172 个 chunks、12,932 条指令及 44,598 原始字节，所选 chunk 无未枚举字节。跨函数共享尾块不重复计数，SDK 的五个发布函数只作复核。GUID→instanceId 管道转发、设备访问共享区的 0x10001／2／3、Camy 专用环缓冲与 91 B HID report 已接到实际消费者；键盘 PID542、鼠标 PID105/114/115 的共享区 worker、XOR、CJS 与直接 HID 分支也已静态闭合到 UnInit。帧发送不等待逐帧 ACK，启动 getter 读回不能替代后续写入确认，停止等待不核结果且 idle thread handle 仍有释放边界。各层丢弃 BOOL／字节数／帧结果、启动查询与帧读回的区别，以及超时后释放继续分别保留。当前所选插件没有逐帧硬件确认，设备访问服务处理器、CJS JSON 接收及其他产品／固件分支仍未闭合；静态接到 HID 调用不等于真实写回或释放验收。

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

`verify-resources --scope keyboard-artwork` 离线枚举类型化键盘主图、Customize、专用图层、Keypad 和映射状态资源，逐项核对当前原件、完整处理模块、固定交付身份以及实际像素解码。AVIF 使用经 wheel／RECORD 校验的 Pillow；SVG 使用同一分析工具目录中的 resvg-py，禁用系统字体和外部资源。校验仅在内存重建输出，不恢复源码媒体或填充安装缓存；缺失原件、未经当前部署核实的缓存、缺失资源锁或解码失败均保留错误。结果统一归属 `.analysis/results/resource-audit.json` 的 `keyboard_artwork`。

实际文件资源优先匹配其完整原件；只有没有实际文件候选时才检查 Keypad inline 分支。CSS inline PNG 只为匹配完整 PNG 内容与精确 selector 的 producer 核验 HTTP 来源，全部扫描输入仍校验实际字节。当前 72 个产品、4,802 条 atlas 使用共得到 1,846 个资源身份；1,845 个通过原件、交付和像素核验，唯一未决项是 747 独立 img SVG 的字体选择。该 SVG 的 RazerF5SemiBold／RazerF5 引用没有内嵌字体声明，页面 font-face 不能自动证明独立图像所用字体；不能以移除文字或开放任意系统字体使静态检查通过。

`prepare-resources --publication-manifest` 将发布输入限定于 `.analysis/inputs/prepared-resources/`。SVG 根节点过滤必须与其当前 acquisition 中的处理模块成对提供，核对实际字节与完整模块，再复用普通准备路径的同一过滤所有者；不能混用其他转换。原图和处理变体分别记录原件、模块、转换与内容哈希，通过现有定向发布入口准备、预演、上传及远端复核后才更新资源锁与映射。不同 edition 的同名布局按完整身份分别保留，不能用 manifest 可读 key 合并；每个资源 context 和已获取媒体的导出依赖边都进入当前来源收据。

GPUI 修改前读取用户指定目录的两个 SKILL.md 及适用机制／组件参考，核对 Cargo.lock 版本；页面布局、控件、状态及交互由当前原件证明。

主程序在 GUI 初始化前安装标准库 panic hook，日志由 `razer_storage::store_path()` 定位到同目录的 `panic.log`，Windows 通常为 `%APPDATA%\razer_ui\panic.log`。记录进程、线程、时间、Rust panic 位置／内容及调用栈，并保留原 hook；日志不可写或锁不可用时尽力输出到 stderr，不阻止启动或吞掉 panic。它只补充 Rust panic 诊断，不能保证捕获原生 failfast、abort 或系统直接终止，隔离服务 worker 也不使用这份主程序日志。写入及真实崩溃记录受窗口运行限制，静态核对与编译不算运行验收。

输入获取、静态恢复、语义、Rust 实现、UI／服务连接、纯／模拟验证、运行验收分别记录。证据 SHA 不作为产品兼容门禁。共享协议逻辑独立于 OS，HID、COM、驱动、服务和系统适配保持明确边界。

缺失原件、间接调用、ABI／协议分支、未实现本地链进入统一 `.analysis/results/gaps.json` 并驱动后续实现。账号、云同步、远程下载／更新、在线目录等联网产品功能移除；loopback、本地 socket、固件文件传输、DLL 替换、服务生命周期、本地预设仍须实现。纯／模拟测试不冒充设备写回；本地 DLL／导出、服务及设备功能测试按根目录已授权的来源、架构、ABI、清理链核实和只读优先边界执行，可恢复写入保留原值、恢复与回读结果。应用窗口、厂商 JS、无关 helper 和安装器仍不运行；测试实际调用、返回、比较及未验证范围单独记录，不能由加载成功或单次 getter 推导整页验收。

## 2026-10-24 四 DLL 并行静态语义批次

本批针对三个 Seiren TUSBAUDIO API 版本和一个 Chroma SDK API DLL，使用 .work/seiren-dll-batch-20261024/analyze_batch.py 的 ThreadPoolExecutor(max_workers=4) 并行读取既有捕获目录，汇总导出、调用/被调用、字符串、导入 xref、错误返回和释放边界。批次收据与摘要保存在 [.work 分析摘要](../../.work/seiren-dll-batch-20261024/analysis.json)；脚本只读，不执行 DLL、服务、设备、应用、安装器或调试器。

| 原件 SHA-256 | 架构／原件字节 | capture.json SHA-256 | 函数／Hex-Rays 失败 | 导出 |
| --- | --- | --- | --- | --- |
| 966162446b255d393c1d272cc1fda3a1eac34c9b649854a61a6d2d77fb65c549 | x64／304,944 | 81ed8145c887c8d4ca642e7a0a3c5071f2c66e9a213829846e356cc831a82095 | 1,170／0 | 84 |
| 0789707daaa4020a1bf80b5ff7d6e6aedd1f2b645afc2cd925ce06ae5d3a36c9 | x64／321,336 | 3c4237581777be020b5bf8e25d64ec4fdcc18ce43fd2dc3bf1a5e1e30265f036 | 1,129／0 | 84 |
| 1c34a749e3cc7deed34e5d0f1fc8fd13dbac659d4991b1e31ba7d4f958a0e475 | x86／281,904 | c921d1282414d52f02f261297aaa276a4eb5970500711386c034641ee6fb8b89 | 1,101／5 | 84 |
| 78a7454edc13006e5c80d5d05d76c8262b1473cb4da6066e6422b1e32d48cef0 | x86／228,552 | 0e269a459a87e2d0af6c8e8008adfb9deba9829d27decde84c9aad32eb821432 | 544／3 | 29 |

三份 Seiren 原件的 API minor 门禁分别为 0xC、0x10、0xC；已在音频文档追到 Enumerate/Open/Close、音量 IOCTL、通知、DSP wrapper、DFU/firmware image 和释放路径。RzAPIChromaSDK 已追到 GUID 身份检查、事件注册返回码、Synapse 状态事件、注册表元数据与共享 mapping/event 导入。以上是静态调用链结果，未将导出存在或捕获完成解释为运行成功；三份 Seiren x86/x64 ABI、五处/三处 Hex-Rays 失败和具体间接服务端仍按各自收据保留。

四个模块原件 SHA、capture SHA、函数/失败数和产物字节已在批次中独立复核；同族版本的函数数不相加为项目覆盖率。实际 DLL/设备对照、读回、错误、取消、恢复和释放验收仍须后续显式测试。
## 2026-10-24 四 DLL 并行 Chroma/Mixer 静态语义批次

本批使用 `.work/chroma-native-batch-20261024/analyze_batch.py` 的 `ThreadPoolExecutor(max_workers=4)`，四个 worker 并行读取已有 IDA capture、导出、imports、字符串和 xref，并在每个任务中独立核对原件 SHA-256、capture SHA-256、函数清单和失败清单。汇总收据为 [`analysis.json`](../../.work/chroma-native-batch-20261024/analysis.json)。脚本没有加载 DLL、启动服务、访问 HID/设备、运行应用或执行安装器。

| 原件（SHA-256） | 字节/架构 | capture SHA-256 | 函数/失败 | 导出 |
| --- | --- | --- | --- | --- |
| `30e64d0f10401a2c847476dfaf024d06550028b34d8775f84695d9713b0d9521` RzChromatic64.dll | 2,835,656 / x64 | `b1b40b40b9ab379fc01fdd6158da45f25712266c1f475ffa9e85dc206446c6c4` | 10,120 / 0 | 588 |
| `babd928b9dbf81ce3a7f203ecfbc01a51fd8f9059b811cde858cabbf04fe257f` RzChromaConnectAPI64.dll | 223,432 / x64 | `bae39fcffaf59a8af8299a5ecad4c0f69f14d9c7382c69b70c77a70ec1571a73` | 840 / 0 | 6 |
| `21ab57c4db9f773b055a89e1e07a13dfe5bd516005d60bdcc250bfe7838fea55` RzChromaBroadcastAPI64.dll | 53,448 / x64 | `88ca1fc8a4cb43695974ef2488c551dda22bf6e9e75eb83d667c9403bdb1adde` | 171 / 0 | 6 |
| `d3cf6173a2e8bb7af0a1066da3bd9d6038270884b6fcdc5afdaefd349755da5d` RzNative_053E_v1.0.16.0.dll | 2,922,184 / x64 | `f4329cca2430a2cd01fee72e6832999fd12d6623c078fd539548cbc62a08e3ef` | 10,432 / 1 | 49 |

RzChromatic 的导出/插件链、Connect/Broadcast 的映射和事件生命周期已追加到 [dynamic-lighting](../products/shared/dynamic-lighting.md)；RzNative_053E 的 MixerSDK/HID/PropertyControl/UnInit 语义已追加到 [microphone-and-preview](../products/audio/microphone-and-preview.md)。四个输入均为不同 SHA；版本函数数不相加为项目覆盖率。静态结果只能作为 Rust 协议实现和回放测试的证据，官方 DLL 对照、服务端应答、设备读回、错误/取消/恢复的真实执行仍保持未验证。
## 2026-10-24 四 DLL 并行设备/音频静态语义批次

本批使用 [.work/native-device-batch-20261024/analyze_batch.py](../../.work/native-device-batch-20261024/analyze_batch.py) 的 ThreadPoolExecutor(max_workers=4)，对四个已有 IDA capture 并行读取原件、capture.json、函数伪代码、导出、imports、字符串和 xref。每个 worker 独立重算原件 SHA-256、capture.json SHA-256 并调用 verify_native_capture；汇总见 [analysis.json](../../.work/native-device-batch-20261024/analysis.json)。脚本没有加载 DLL、启动服务、修改注册表、打开 HID/WinUSB、执行安装器或访问真实设备。

| 原件 SHA-256 / 模块 | 架构/字节 | capture.json SHA-256 | 函数/Hex-Rays 失败 | 导出 |
| --- | --- | --- | --- | --- |
| 05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28 RazerMCUUpdaterInterface.dll | x86 / 42,496 | bc5e006873c56cab498f56fd8b88ba60520755c4ca186a3dd2e830d653864cd0 | 160 / 0 | 24 |
| e268ea62bdfc0a65194f580b66f6d907b1e2bdcf9cb474d737364adcdcabe88e RzChromaSDKProxy_v1.2.3.2.dll | x64 / 359,112 | 00b5bf90b995f1381e55be65150eb67433600fcbbef00e9009d45f0d44bfcbe9 | 1,479 / 0 | 14 |
| 13fc8d59ca268687f490cfaf132f875da1dce2a6d81c434723f614b4385821b6 RzChromaBroadcastManager.dll | x86 / 226,504 | bfb1e7f7e6dcd1662abd46128264418a138f6b5298dc8c2cedd2f4c001104338 | 561 / 0 | 23 |
| 012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11 RzAudioUtil_v1.0.1.1.dll | x64 / 706,248 | 92ed9a6f9651fb6c67ece87ed97667b4c885f23d8fd173eca97d720500608b2d | 2,339 / 1 (0x1800673a3, call analysis -12) | 9 |

RazerMCUUpdaterInterface 的 OpenDevice/OpenWinUsbBootloader、HID Feature 状态轮询、WinUSB 控制传输、new_fw.bin 分块和 DFU 退出边界已追加到 firmware-and-installation；RzChromaSDKProxy 的 JSON Invoke、PollEvents、allocator 与 UnInit 已追加到 dynamic-lighting；BroadcastManager 的效果入口、注册表元数据和单回调错误码已追加到同一文档；RzAudioUtil 的 THX JSON dispatch、媒体命令、错误字符串和 FreeMalloc 已追加到 microphone-and-preview。静态证据不能代替真实设备应答、读回、取消/恢复或释放验收；这些仍保持 Unverified。

## 2026-10-24 全量 DLL/NODE/SYS 四线程静态批处理

为避免逐模块手工启动，`.work/native-bulk-analysis-20261024/analyze_all.py` 使用 `ThreadPoolExecutor(max_workers=4)` 一次枚举全部 1,623 份现有捕获收据，并过滤为 881 个 DLL/NODE/SYS；该批次只读取既有 IDA/Hex-Rays 结果、导出、导入、命名符号和输入字节，不读取或执行 EXE、安装器、DLL、服务或设备。完整汇总收据见 [`capture-index.json`](../../.analysis/results/native-bulk/capture-index.json)。

| 项目 | 结果 |
| --- | ---: |
| 并行 worker | 4 |
| DLL/NODE/SYS 模块 | 881 |
| capture 完整 | 881/881 |
| 函数总数/已捕获 | 2,603,678 / 2,603,678 |
| Hex-Rays 失败条目 | 869 |
| 输入字节 SHA 与收据一致 | 881/881 |
| 扫描耗时 | 约 16.5 秒 |

这份收据证明批量静态捕获和字节身份检查已完成，不等于每个模块的语义、Rust 独立实现、页面接线或官方 DLL/设备运行对照已完成；失败函数、间接调用、服务端应答和未覆盖产品分支继续按产品文档与 `.analysis/results/gaps.json` 追踪。EXE/安装器不纳入当前后续逆向范围。

同一批次随后运行 `.work/native-bulk-analysis-20261024/semantic_index.py`，仍以四个 worker 并行读取 1,623 份收据，实际索引 881 个 DLL/NODE/SYS。它为每个模块记录导出入口及对应函数体、失败 RVA、关键导入族（HID/WinUSB/SetupAPI/Kernel32/Advapi/COM 等）、命名符号和导出函数的关键伪代码/返回行；函数体和原始字节仍以各模块 `.analysis/results/native/<sha>/` 收据为准。汇总索引见 [`semantic-index.json`](../../.analysis/results/native-bulk/semantic-index.json)，本次共覆盖 64,092 个导出，其中 60,843 个有捕获函数体、3,249 个只有导出入口记录；869 个 Hex-Rays 失败条目分布在 241 个模块。批处理耗时约 45.4 秒。该索引是批量静态语义入口，不能把关键字命中或导出存在解释成协议、页面、设备 ACK、读回、取消、恢复或释放已经闭合；后续产品文档继续引用具体函数和原始字节。

## 2026-10-24 Chroma Connect Server frame follow-up

The existing `RzChromaConnectServer.dll` capture was independently checked with `ThreadPoolExecutor(max_workers=4)` for the WebSocket frame and Ping helpers: 6 selected functions, 20 unique chunks, 2,112 instructions, 7,673 raw instruction bytes, 0 unlisted bytes. Original SHA-256 `a49d11fc5f078c4e0019dd0d4972c683755456dcf83ddb1f883cd4a6b26fdbc5`; `capture.json` SHA-256 `2ce03476a1e68777e86009029defa301a17d57723116a21d2ec6f6f3de7eaed9`. The selected set is RVA `4e040,4ec80,5f6a0,5fee0,60000,60470`; all function JSON, chunk and instruction bytes matched the original PE.

## 2026-10-24 四 DLL 并行 Camera/Audio/THX 语义批次

本批使用 [.work/camera-audio-native-batch-20261024/analyze_batch.py](../../.work/camera-audio-native-batch-20261024/analyze_batch.py) 的 ThreadPoolExecutor(max_workers=4) 并行读取已有 IDA/Hex-Rays capture。每个 worker 独立重算原件 SHA、capture SHA，调用完整捕获校验，并把每个函数 chunk 和 assembly instruction 的原始字节映射回 PE section；脚本只做静态读取，没有加载 DLL、启动服务、访问 HID、执行设备写入或运行应用/安装器。汇总收据见 [analysis.json](../../.work/camera-audio-native-batch-20261024/analysis.json)。

| 原件 SHA-256 / 文件 | 原件字节 / 架构 | capture SHA-256 | 函数 / 失败 | 导出 |
| --- | ---: | --- | ---: | ---: |
| 1ea145fe182505cd8ae19a6bc1ea5362c70d18681b51195d026ec57c86ad06d8 RzNative_0E08_v1.0.16.0.dll | 600264 / x64 | 54f729c256fd559ec8c5c286a869f9fcf587e4546d49ab23c112f19114989275 | 2060 / 0 | 91 |
| b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c RzNative_0542_v1.0.25.0.dll | 639624 / x64 | 65b7d807757bfd1188d7886046c5c0b83a0cb47a04cd0f114ba2e5e7541eacab | 2472 / 0 | 44 |
| 8678a5eadd881ea7a569131369b3964d9519f1d584beddd315fb897e6f4af5ca ThxVADCarolNative_v1.0.13.0.dll | 954504 / x64 | 77a1cb3de9321ba4b1cc027d440d7f575aa0e84b82d8d67203b5aa8b074b612f | 2920 / 0 | 71 |
| d4fd77a92c860bb190d7aab604124e9d4343901fcea83069aa99d29db5a9586b RzNative_0E03_v1.0.16.1.dll | 796296 / x64 | 23ce499d0c8fae7c60e75c97072dc080a0876f1c07376aad8d748af523d9a377 | 2634 / 1 | 40 |

四个模块均为 capture_complete；独立字节映射均报告 all_chunks_match_original=true 和 all_assembly_matches_original=true。去重后的 chunk/指令/字节计数依次为 0E08 2066/103613/419265，0542 2478/112768/458516，Carol 2958/173596/673579，0E03 2684/143877/555742。0E03 基础捕获中唯一失败函数的入口为 RVA `0x721f0`；Hex-Rays 在该函数内部绝对地址 `0x180072223`（RVA `0x72223`）报告 call-analysis failed，错误码 -12，不能把错误指令位置写成函数入口。该函数及其字符转换 callee 已通过私有 IDB 类型补查恢复，当前严格复核为恢复 1、保留失败 0、补查错误 0，见[正式复核](../../.analysis/results/native-bulk/reanalysis-verification.json)中对应模块；原始失败、汇编及基础捕获仍保留。伪代码恢复不代表设备功能链或实际 DLL 对照完成。

静态语义已分别记录到 [camera](../products/accessories/camera.md)、[microphone-and-preview](../products/audio/microphone-and-preview.md) 和 [playback-and-eq](../products/audio/playback-and-eq.md)。导出数、函数捕获数或字节一致性不代表 Rust 已实现，也不代表官方 DLL、音频端点、相机属性、HID 报文、读回、失败、取消、恢复和释放已经运行验收；没有设备时的回放和模拟只能作为独立期望，官方实际对照继续保持 Unverified。


## 2026-10-24 Chroma SDK public-method follow-up

Four-thread static byte checks (`ThreadPoolExecutor(max_workers=4)`) reused the IDA 9.1 captures for RzChromaSDK.dll / RzChromaSDK64.dll. The x86 input SHA-256 is `8e1466d0dfd653f0bba61f57117bda85e91ee779e2b6e71ac76f411cfeb0d038`, capture SHA-256 `7d2de3c1ceb4b9860b3eec84bd045a29b4a7d5886cd1d0adc155b675e66c2e32`: 15 selected functions, 15 unique chunks, 2,273 instructions, 6,883 raw/instruction bytes, 0 unlisted bytes. The x64 input SHA-256 is `7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7`, capture SHA-256 `35aabd2f3285e856d762889213e7ec73630b69b116a5ff8b33d76250766540d1`: 15 selected functions, 15 unique chunks, 1,666 instructions, 6,822 raw chunk bytes, 6,484 instruction bytes, 338 unlisted chunk bytes. Every selected function/chunk/instruction and every aggregate output hash matched the retained original. The x86 complete capture retains two Hex-Rays failures outside this selected set (`0x6500`, `0x6510`); x64 retains none. Export validation, null GUID rejection, effect-type/parameter boundaries and window/stop lifecycle are recorded in [dynamic-lighting.md](../products/shared/dynamic-lighting.md). DLL execution, actual device results and Rust differential acceptance remain unverified.
## 2026-10-24 统一产品语义批处理

全量索引保留 881 个按 input 扩展名选出的 DLL/NODE/SYS 模块。[完整索引](../../.analysis/results/native-bulk/semantic-index.json)和[基础捕获统计](../../.analysis/results/native-bulk/capture-index.json)已保存到正式分析目录；[统一报告](native-bulk-reverse.md)不再使用 docs 中 SHA 出现次数筛选或判断语义覆盖。原有 590／576 一类“未覆盖候选”会被报告自己的引用影响，不能作为剩余功能数量。当前报告的检索候选只表示信号匹配，不能证明某项调用链已经解释完成。

报告使用 16 个发布线程读取固定分析索引，保留每个模块的原件/capture 身份、函数和基础失败数、伪代码记录、导出函数体、关键导入和检索候选。基础捕获的 869 条失败与补查恢复结果分开记账，不改写原始失败收据。报告不包含 EXE/安装器后续任务；已有历史 EXE 静态收据不进入本批队列。真实 DLL、服务、设备、错误、取消、恢复、释放和 Rust/页面对照仍按授权边界分别记录，不能以批量索引替代运行验收。

统一续查使用一个协调队列，按本机 16 个逻辑核启用最多 16 个 Python 调度线程和独立静态分析进程；经用户要求并行分配的语义核验任务复用完整捕获，各自负责明确的文档与收据，不重复启动该队列，也不以等待子任务阻塞其他分析。原始 capture-corpus 的现存队列收据声明 module_count=884，但 modules 只保存 836 项，日志也止于 836；这 836 项为 reused_complete_capture，不能据进程退出宣称剩余 48 项已完成或已排除。881 项索引另按捕获 input 的 DLL/NODE/SYS 扩展名筛选，与队列的候选来源和别名筛选不是同一集合。当前逐条复用完整捕获并补查有汇编依据的类型、间接调用、传输和释放；Rust 迁移保持后置。

全库失败函数在同一并行批次中按完整指令／操作数分组，基础捕获共 241 个模块、869 条失败、329 组汇编结构。分组仅用于定位共享问题；实际修复前必须核对调用方和全部相关 callee，不能按同名函数或单一关键字套用签名。UTF-16／UTF-8 转换组逐份核对调用方和三个 callee 的寄存器参数、完整原字节，明确排除反编译器错误推导的第五／第六参数，再由 `supplement-native` 在私有 IDB 副本中应用类型并重新执行 IDA／Hex-Rays。队列与每次实际分析结果见[统一补查收据](../../.analysis/results/native-reanalysis-batch.json)；原 capture 和 IDB 保持不变，恢复伪代码不计作产品功能或实际设备验收完成。

### 统一队列结束与独立复核

实际命令为 `python -X utf8 .work/native-bulk-analysis-20261024/reanalyze_failures.py --workers 16` 和同一脚本的 `--phase demangle --workers 16`；前者完成 56/56，后者完成 59/59，均保留正式 `supplement-native` 计划、IDA 9.1／Hex-Rays 9.1.0.250226 会话和完整函数结果。第二批根据完整的九指令调用方及实际 callee 核实 RCX 输出缓冲区、RDX／R8 输入和 R9D 的 0／1 分支，再修正错误的聚合返回推断；它恢复的 118 条基础失败均经现有严格复核器独立通过。[第二批收据](../../.analysis/results/native-bulk/demangle-reanalysis.json)不以队列退出代替此复核。

同一脚本的 `--phase audit --workers 16` 对全部 241 个基础失败模块调用 `native_reanalysis_resolution`。大函数与 SDK x86 补查后的[正式复核](../../.analysis/results/native-bulk/reanalysis-verification.json)记录：历史基础失败 869、已恢复 212、仍保留 657，审计读取异常 0；其中 212 为此前 176、大函数 34 和 SDK x86 2 条恢复之和。该完整快照另保留 6 个模块的 8 条补查错误：6 条完整结果冲突和 2 条旧不完整补查。字符转换批次的无错误模块为 50/56；不能将队列的 passed=56 写成全部独立复核无误。完整冲突对的实际汇编等非类型字段一致，正文在明确参数重命名后也一致，但声明和摘要不同，仍按严格规则保留冲突；[诊断结果](../../.analysis/results/native-bulk/reanalysis-conflicts.json)只解释差异，不放宽对照或静态验证标准。新的统一剩余队列启动后，进展另记于其独立收据，不覆盖此历史基础失败。

同一脚本的 `--phase wpp --workers 16` 使用固定的两份完整指令模板和现有原件函数验证器，46 个键盘驱动包装函数的输入、完整 chunk 与指令逐字节匹配，合计 2,254 条指令、9,344 字节，错误 0；实际导入另核为 WppRecorder 的 WppAutoLogTrace。完整参数转发和条件分支见[设备通信](../products/shared/device-communication.md#键盘驱动中的-wpp-日志包装边界)，[收据](../../.analysis/results/native-bulk/wpp-wrapper-verification.json)保留每个版本与函数身份。这是完整汇编语义核对，不是重新运行驱动、设备测试或 Rust 功能验收。

### 大函数补查的进程内上限与收据

Hex-Rays 的默认 `MAX_FUNCSIZE = 64` 会拒绝超过 64 KiB 的函数，即使完整汇编和原始 chunk 已捕获。统一入口 `tools/workspace.py supplement-native` 的 `--max-function-kib` 只接受 64–1024 的整数，且仅用于显式选择的已保留函数边界；不能与指针表捕获混用。该设置通过 IDA 9.1 本机 Python API `ida_hexrays.change_hexrays_config` 应用于本次私有 IDB 分析进程，不修改安装目录配置、原件、基础 capture 或原始数据库，也不自动增加或猜测 ABI 声明。

计划的 `function_selection.decompiler_limits` 参与精确 selection 身份，capture 的 `applied_decompiler_limits` 必须与计划完全一致；插件不可用或拒绝设置时立即失败。严格复核继续逐字节核对原件、原始函数边界和实际输出，未应用上限、应用不同上限或篡改原件／汇编均不能被接纳。分析进程成功退出仍不等于恢复成功，必须在补查完整、伪代码存在和独立复核均满足后分别记账。

本批以同一协调脚本的 `--phase large --workers 16` 选择仍报 `too big function` 的 17 个范围内模块、34 个函数，显式设置 1024 KiB，排除安装器插件。[大函数补查收据](../../.analysis/results/native-bulk/large-reanalysis.json)现记录 17/17 模块结束，34 条恢复经随后的 `--phase audit --workers 16` 全库严格复核确认。相关收据正向、篡改反例及原始字节保护通过 `NativeSupplementTests` 定向验证，当前 9 个测试通过；该测试证明维护工具的接受边界，不是 DLL 功能运行测试。

### 统一剩余静态任务与严格复核

统一维护入口为 `python -X utf8 tools/workspace.py reverse-pending --workers 16 --process-workers`，协调者使用有界独立进程处理原生与前端静态任务，绕开 Python GIL 对 CPU 并行的限制。每个 worker 只返回局部结果，记录实际 PID，统一收据由协调者发布；共享前端 AST 输出按来源 SHA 加锁。省略 `--process-workers` 时使用线程池。原生任务复用完整基础捕获和私有 IDB，仅为有准确边界的剩余失败执行正式补查；前端任务复用现有获取、模块恢复和词法 AST 工具。完整任务状态、实际命令、逐项输出与未决原因保存在[统一收据](../../.analysis/results/reverse-pending.json)，摘要同步到[批量报告](native-bulk-reverse.md)。脚本数量和 worker 数量不代表同时运行相同数量的 IDA；原生阶段结束后，前端阶段可只运行 Python 静态工具。

完整基础捕获必须通过 `verify_native_capture`，逐项核验函数清单、完整输出、汇编标记和实际产物字节；`capture_complete` 本身不能作为验收。原件身份在开始、每次原生命令返回、基础捕获核验后和发布前重新核对，基础清单在发布前也必须保持一致。活跃或无法查询的会话不会被重复启动；恢复中断补查仅允许在确认 PID 已消失后执行，并保留旧 session、计划、capture 和日志的原始字节，不能补造退出码。

`--verify-only` 只对已结束队列执行严格复核，不启动 IDA 或网络修复；完整原队列收据以原始字节及 SHA-256 记入当前收据的 `previous_static_run`。分析器源码按实际字节固定在同一结果树的 `tool-sources/`，严格结果分别写入 `verified-native/` 和 `verified-frontend/`，保留原输出。存在失败或严格冲突时，CLI 返回 1；队列已结束、静态通过、语义完整、Rust 实现、页面接线和实际 DLL／设备验收仍分别记录。

`reverse-pending --workers 16 --resume` 只续接同一未结束队列，继承原生／前端范围和已冻结审阅计划，不能同时另传范围或计划参数。原任务 ID、已完成结果及清理记录保留；原收据的完整字节与 SHA-256 记入 `interrupted_static_run`，不补造退出码或完成时间。续接前重新核对原件、基础捕获、计划及结果身份，协调者持有全局独占锁；有 PID 的协调会话还须确认原进程已消失。旧格式没有协调者 PID 时明确记录该限制。已结束但尚未由协调者登记的精确匹配补查直接复核接收，不重复执行；仍存活或无法查询的分析进程继续等待。启动输出分别报告总任务数和实际续接任务数。

源集登记与依赖闭包也分别核对：12 份家族矩阵的 664 个源集及 25 个宿主来源入口均已具备 acquisition，当前共保留 711 个前端源集；这不代表所有声明依赖或动态 chunk 已获取。家族矩阵的静态 JS／CSS／HTML 清单未发现缺文件或长度不符，但仍有声明的 TypeScript／图像 404、动态 chunk 枚举不可用及恢复图问题。枚举不可用的记录数不能换算为缺失文件数。当前普通修复会重试瞬态／损坏输入并保留历史，已记录的 source-declared 404 仍须通过正式的显式 `--retry-declared-missing` 路径核对；不能声称普通修复已把这类缺项补全。

### 剩余失败的有依据批量重分析

`reverse-pending --native-only` 仅处理原生失败与补查验证冲突，不重新扫描前端，不原样重复已经失败的 IDA 参数。可重复提供 `--native-review "项目相对收据.json#/计划指针"`，在同一有界队列批量执行经完整汇编审阅的修复。计划包含原件、基础捕获、完整函数证据的实际字节身份，目标 RVA、明确类型及其依据；执行前再次核对计划与全部证据。已恢复且无冲突的匹配结果才可从队列排除，失败、补查冲突和未决调用继续保留。

`supplement-native --import-type "IAT_RVA=函数指针声明"` 仅接受原 PE 普通导入表中具名且唯一、实际字节可映射的 IAT 槽；延迟导入、序号导入、普通数据或未知间接指针不得套用。IDA 仅在私有 IDB 副本固化完整函数指针类型，记录旧类型、实际应用类型和槽字节；严格复核要求该记录与原件及计划一致。磁盘 IAT 字节与 IDA 加载视图分别保留；IDA 导入注册必须唯一匹配名称、模块及槽宽，不要求加载后槽内容等于原 PE。完整捕获中已经存在的六字节 `FF 25` 导入尾跳 thunk，经原件指令、边界及目标 IAT 核实后，应用同一指针的完整被指向函数类型并单独记账。变参日志函数须保留所有固定参数和省略号，不能以删参数或改成无参函数来消除报错。现有根函数类型修复仍须保留所有实际参数、寄存器与栈清理语义。

`supplement-native --merge-functions "OWNER_RVA=RVA,RVA,..."` 只处理已有完整捕获中经明确审阅的连续单 chunk 函数片段。所有成员须顺序连续、无洞、无重叠，owner 必须是首成员及显式选择的目标；原件字节、完整指令、成员文件身份及非 owner 外部真实 CALL 入口分别核验。该规则不自动证明全部语义，也不据相邻地址自动合并。IDA 仅删除私有 IDB 内对应片段定义并按确切并集重建 owner，原始 PE、基础 IDB 与历史失败不变。manifest 保留全部成员身份及实际应用 chunks；严格复核重新计算并集，仅以完整 owner 输出核对合并基线，不借此删除成员的旧失败或放宽普通函数边界检查。

`--outline-function-rva` 只标记已有完整捕获且经原件逐字节核实的独立 helper，使用 IDA 的 `FUNC_OUTLINE` 表达共享寄存器与调用者工作区，保留全部原指令及边界，不猜普通函数 ABI。`--partition-functions "OWNER_RVA=START_RVA:END_RVA,..."` 另处理完整 owner 内经审阅的嵌入 helper；半开区间必须与完整指令边界、真实 CALL 入边及封闭控制流匹配。owner 残留 chunks 与所有 helper chunks 必须无重叠地精确覆盖原 chunks，包含 padding 字节；只在私有 IDB 重建分区并标记 helper 为 outlined。独立的 `partition_helpers` 保留 helper 完整汇编，严格复核重新核对所有字节、入口引用、应用标志及 owner 伪代码，不把仅捕获汇编的 helper 记为反编译成功。共享 tail 的多入口函数不能由此删除入口或自动套用普通 ABI。

Hex-Rays 失败另保留实际 `code`、`errea`、相对 RVA、描述和附加字符串，原始异常文本继续保留。该信息用于定位真实失败调用或栈位置，不改变原失败及成功判定。

补查同时注册有界、只读的 Hex-Rays 静态观察器。在 microcode、优化、栈地址解析、内联和 callinfo 回调期间立即复制指令及 `map_fict_ea` 返回的标量，回调结束后不保存借用的 MBA、指令或 SWIG 对象。成功函数仅保留小型诊断摘要；失败函数另保留各阶段事件与原始错误地址的映射候选。缺失、BADADDR、冲突、截断、注册或注销失败均明确记账，不改变反编译结果，不将候选自动认定为原件中的真实指令。映射仍须与原 PE 的指令、完整函数、共享尾和实际调用目标独立核对；观察器不修改 microcode、类型、SP、函数标志或输出 callinfo，也不调用需调试器的 dump 接口。

调用观察另逐项冻结 MBA 栈帧、共享尾全部所有者的 frame／SP、真实调用操作数已存类型、分别请求的参数位置及 `callinfo_built` 的调用栈位置。SDK 未返回的字段保持未观测；`GTD_NO_ARGLOCS` 结果不证明位置原本已序列化。无 ABI 或边界修补的新观察通过 `--observation-evidence` 绑定冻结审阅中的明确原因与观察器原字节身份，并进入补查选择摘要；复核时验证审阅、工具与实际捕获身份。由此保留每次不同观察的独立收据，不为区分输出而添加类型、栈修补或放宽恢复条件。

`--share-function-tail "PRIMARY=START:END:OWNER,OWNER[:inline|:retain]"` 表达经审阅的多入口共享尾块。选择器重新核对全部 owner 的原件身份、完整 chunks、指令、双向引用和相对控制流，尾块须为 primary 原入口 chunk 的完整后缀；数据引用和未枚举的 padding 原字节均保留。私有 IDB 只收缩 primary 的入口 chunk 并向所有 owner 附加同一尾块，保留原入口，核对实际 primary、完整 parent 集合、frame、类型及既有栈变化点。`inline` 标记这些 helper 为 `FUNC_OUTLINE`，`retain` 保留 owner 原 flags；两者均不猜测 ABI。非目标 owner 的完整汇编单独记入 `shared_tail_helpers`，不得用汇编记录冒充伪代码。严格复核重新读取基础 IDB 的实际 SHA，并验证所有 owner、目标函数和 helper 的完整原字节及引用；恢复计数仍以完整伪代码和严格验收为准。

`--fresh-inline-function-rva` 仅接受已完整审阅的 outlined helper，或明确标记 `inline` 的共享尾 owner。原 PE 中每个选定 parent 的完整 chunks、helper 原字节与真实五字节直接 CALL 入边均绑定进选择摘要；有独立栈帧的依据必须明确保留。控制器只在 SDK 的 `begin_inlining` 阶段调用 `mba.inline_func`，以 `INLINE_EXTFRAME | INLINE_DONTCOPY` 为每个原 CALL 独立生成 helper；不手写 SP、栈操作数或伪造调用结果。每次生成后重新查找剩余原 CALL 块，记录实际 SDK 返回值，并在 `finally` 停用和注销控制器。普通只读诊断观察器仍独立拥有自己的生命周期。任何原 CALL 缺失、重复、SDK 失败或清理失败都保持未完成；原 CFG 已证明对特定入口不可达的共享尾 CALL，只能通过另一个有完整原件依据的显式选择缩小强制生成范围，不能静默忽略。完整伪代码、未截断诊断、Rust 语义和实际功能对照分别验收。

保留函数重分析仅导出显式选择的完整函数及其全部 chunks。入口直接 JMP 可以指向同一函数的内部块，不因此增造一个函数入口。指针表补查继续沿有界直接 JMP 链追踪真实 callable，保留循环和边界错误；两个模式分别验证。

Supplement 接收先验证整份计划、会话、完整输出、原件字节、边界及 SDK 生命周期，任一结构或身份失败均拒绝整份；全部通过后才逐个完整函数归并。重复 RVA 的完整原件 SHA 不等时保留双方冲突引用，并从有效 complete_functions 和恢复计数中排除该 RVA，基础失败重新列为未决。独立的新父函数可以接收；不会通过去掉 SWIG 地址、忽略诊断或伪代码差异来消除原件冲突。

当前审阅计划分别保留在 [Chroma 事件线程收据](../../.analysis/results/native-bulk/chroma-lifecycle-verification.json)、[设备驱动日志收据](../../.analysis/results/native-bulk/camera-0e06-identity-verification.json)和[音频共用字符查找尾收据](../../.analysis/results/native-bulk/audio-semantic-verification.json)。计划获准执行不等于失败已恢复；只有正式 IDA 输出及完整字节、边界和伪代码独立复核通过后，才减少基础失败计数。共享栈帧、多寄存器输出和内嵌子程序边界无法用普通 ABI 无损表达时继续静态核查，不提交猜测类型。

维护工具的纯测试为 `python -X utf8 -m unittest discover -s tools -p test_reverse_pending.py`、`python -X utf8 -m unittest discover -s tools -p test_evidence.py` 和 `python -X utf8 -m unittest discover -s tools -p test_host.py`；实际数量和结果以当前执行收据为准。反例覆盖过期／冲突收据不得隐去失败、审阅计划与原件执行前再次核验、完整命名 IAT 与序号／非 IAT 拒绝、精确导入 thunk、片段合并和 helper 分区的字节／边界／控制流篡改、共享尾块的 parent／frame／类型／栈点和引用完整性、总队列中断恢复，以及只读诊断的生命周期、失败、截断和映射冲突。这些测试不运行 IDA 或厂商 DLL。

统一 `verify-analysis-tools` 在加载测试套件前及执行结束后分别计算工具与测试输入的实际字节、长度和 SHA，收据保留两份快照及差异。输入变化或消失、零项执行、全部跳过及执行中断均不能发布通过；中断仍写入失败事实并向调用方传播。此检查的时间边界是测试套件加载前至执行结束，不提供实际 DLL、设备或窗口验收。

host 获取与提取候选此前执行的 19 项定向纯测试及其完整输入快照继续保留，不能与新范围相加。当前灯光对照工具定向执行 LightingComparisonTests 共 22 项，失败、错误、跳过均为零，工具与测试输入前后稳定，所属临时目录已释放。新增反例核对当前发布实际字节、双采样缺失、来源错误、声明变化、同 SHA 但 latest 指向另一个 manifest，以及取消、失败和清理；该纯测试收据不代替另行实际执行的 DLL 对照。

validate-rust 分别登记 BLE 当前 DPI、Protocol30 分片与发现清理、SystemMonitor、Seiren、接收器独占、关闭错误收据和下拉位置纯测试；过滤后必须实际执行非零测试，窗口测试仅编译。当前工作区全目标检查及 Protocol30 37 项模拟服务测试通过，1,833 项 Rust 输入保持一致；新增六项覆盖设备获取前的异步所有权。旧 31 项、包含 BLE26 的 81 项及失败批次保留各自完整收据和原始日志，不与 37 相加。warning-lines 包含编译汇总，不等于互不重复的警告数量。BLE 的实际 DLL、设备与窗口验收仍未验证；软件灯光的实际对照另按自己的局部范围记录。

Protocol30 写入在全部分片 dispatch 后开始 500ms 响应预算；write、CCCD enable 及 Connection 内两条 discovery operation 的取消、终态与 Close 分开处理。Connection 先取得真实生命周期所有权，再启动 FromBluetoothAddressAsync；DeviceAcquisition 保留 typed operation、result_observed 和 sticky cleanup 状态，完成操作 Close 后才交给正常初始化。Close 失败保留已观测结果，不重复 GetResults；超时或取消后的 late Completed 只读取结果、收束 operation.Close 并释放结果，不注册 handler、初始化或发布新连接。Status、Cancel、GetResults、Close 的清理错误阻止同容器重连；retained registry 和后台关闭继续持有未终态操作。官方 Connect0x3e540／Completed0x3eb00 只在 AsyncStatus::Completed 时读取结果；Disconnect0x26e00 注销事件后由 0x2200 执行 IUnknown::Release，不能将其记作物理断开或 IAsyncInfo::Close。operation.Close、结果 IUnknown::Release 及父资源 Close 各自保留契约，未增加逐项 service.Close 或 late device.Close。

USB 间隔依据当前主机 UsbRzDeviceAction 与 10/29 的 PID190 middleware、chunk6120；两份双采样复核收据分别覆盖部署根／UI 和显式 chunk，必需原件未变，可选 asset-manifest 缺失另行保留。PID191 是 PID190 的 dongle 分支。首次或充分空闲的 collection 无需固定再等 5ms，每次请求第六参数的 sleepTimeBetweenOut 覆盖值保留，包括显式零值。只有显式 release 且 sender URL 匹配才更新时间；tabDestroyed、timeout 和 batch finally 不更新。batch finally 只有 URL 匹配才释放，不匹配仅警告并保留锁。普通 Send／Get 和整个 Batch 正常返回才更新时间，异常返回不更新时间。batch 的 2ms OUT 间隔位于整个 native batch 调用之前，不能逐报告重复加入。主机 send 的 native 异常与 middleware IPC 异常会被各自 helper 吞为 undefined，不能孤立依据 sendCommand 外层 catch 将这些分支改为立即退出。共享 broker、精确拓扑、连接代次、原生 actor 和全部传输消费者的实现与纯／模拟验证分别登记；跨进程运行、原生 batch 内部语义、完整 sender 身份及实际 DLL／设备时序对照仍未验证。

SystemMonitor 的当前 Hanbo 默认链遵循 Common 建立 Timer2 → Register 建立独立 Timer1 → Timer2 请求 → Service 首次 Initialize → GetAllHardwareInfo → 收帧 → 满足订阅门限后发布，并保留各时间锚。原生建窗存在异步调度边界，静态顺序和模拟结果不证明 OS 相位一致；多订阅较短共享周期与显式 interval setter 分支仍未实现。

`audit-runtime-resources` 的 Current DPI／Matcher 检查覆盖其实际页面、Shell、协议、IPC、发现与服务所有者，公共页面入口仅检查所属组合函数，避免把无关功能算入此范围。规则检查 UI I/O、任务保留与清理、连接／配置身份及官方业务 DLL 加载入口；合法的系统 API、本地主机配置路径和独立通信 worker 不因此被禁止。Matcher scope 保留检查按 AST 参数顺序核对领域、页面与归一化代次及 guard，允许 rustfmt 尾逗号。该静态范围不代表全工作区厂商加载器已清零，也不提供完整类型或宏展开证明。

模块服务图像检查从当前 deployment manifest 选择唯一 JS／CSS，通过完整 AST 核对主 runtime 的 chunk 名、公有路径、main → App → lazy6505／factory44442 挂载链，再沿实际 default export 与图像消费者验证样式和 fallback。历史 chunk 不在当前声明中时直接失败，不以保留文件的存在证明当前性。该检查已通过当前声明的 `6505.98da235d.chunk.js`，资源审计的其他闭包项仍须按自身收据验收。

`refresh-gaps --owner <项目相对收据路径>` 可重复指定语义报告及 `native/<sha>/capture.json`，在同一有界批次重算选定所有者；省略参数仍执行原全量流程。两条路径共用缺项分类规则，保留分析器失败、未决链与实现／验收边界。发布前重读选定来源的实际字节；来源变化、路径逃逸、非规范路径或不适用的所有者均拒绝发布，其他所有者保持原样。当前来源身份记录在 `gaps.json` 的 `selected_refresh`，避免补查一个共享链就重扫整份语料。

`extract-embedded-inputs --pe-review "项目相对收据.json#/指针"` 只提取显式审阅的完整 PE 文件范围，不扫描 MZ 命中来猜文件边界。指针选择单组或多组 `schema_version: 1`、`source_input` 和 `ranges`；每个范围提供安全文件名、精确 offset/bytes/SHA-256、machine、边界依据和证据文件身份。入口先核全部组的当前父输入索引、实际原件／证据字节和目标冲突，再核 DOS/PE 头、段、目录、证书及完整文件长度，全部通过后才登记子原件。IA64 函数描述符与零长度 GlobalPtr 具有单独的解析语义。原审阅字节和被更新的提取报告均按内容哈希保留引用，原失败不会因改用正确子架构而删除。独立子捕获继续使用 `capture-native`；分析器不具备相应处理器／反编译器时保留未验证，不加载驱动或执行安装链。

### 历史临时文件的保守清理

`python -X utf8 tools/workspace.py clean --stale` 只处理当前批次开始前的 `.work` 根层 `.log/.txt/.out` 与根层 `__pycache__/*.pyc`。引用核对使用已安装的 `rg` 固定字符串多模式搜索与 16 个扫描线程，覆盖维护源码、文档、输入索引及正式 JSON 收据；已索引的函数／AST／原始反编译正文不重复扫描。不明类型、仍有引用、活进程路径、链接／junction、新文件及占用文件保持原样，删除前以 Windows 独占句柄重新核对文件身份。该模式不进入原生 IDB、分析工具与当前运行目录，也不删除 `.analysis` 原件或正式结果。

首批共核对 529 个候选，删除 526 个文件、26,397,115 字节（约 25.17 MiB），3 个候选因现有引用保留，保护检查无错误；删除后逐项确认均不存在，8,933 个原有受保护文件及输入／corpus 索引均仍在。清单在当前队列最终发布后合并至统一收据的 `scratch_cleanup`，合并前暂存于受保护的当前运行目录。`test_scratch_cleanup.py` 的 13 项测试中，11 项通过；2 项真实 symlink 创建因本机权限跳过，另有无需该权限的链接拒绝、目录逃逸及真实独占占用反例。`--stale` 保持上述有限范围。

每轮逆向必须按 [AGENTS.md 第 7.1 节](../../AGENTS.md#71-每轮逆向的清理与体积控制必做) 收尾。默认 `python -X utf8 tools/workspace.py clean` 或显式 `--dry-run` 仅生成 `.analysis/results/scratch-cleanup-<UTC时间>-plan.json`，不再清空 `.work`。计划使用保留名称并独占创建，已有计划和 apply 收据不得覆盖。完成证据充分、无后续依赖的 supplement 私有副本和旧资源克隆由计划逐项列出；原始 batch 数据库、活跃／预留任务、当前 verification checkout 及正式引用保留。执行使用 `clean --apply <计划相对路径> --plan-sha256 <实际计划SHA>`，先获取队列与资源写入锁，再重新核对完整引用、预留、完成收据、实际文件身份、路径和占用；首次删除前持久化完整计划范围和身份。实际删除与失败记入统一收据；清理计划不代表已释放空间。

旧上传克隆的清理还核对当前资源 lock、真实校验器、完整发布或固定版本只读预检收据、HEAD／refs、index／tree／worktree、LFS 实际对象与 Git 元数据；存在未发布对象、额外引用、未知文件或重建依据不足时保留。apply 在计划中的目录和文件句柄仍持有时，最后重新枚举完整树、核对目录和文件身份并盘点活动进程；任何外部 Git／Git LFS／Git remote 进程，包括相对工作目录命令，均会阻止上传克隆删除。新增锁、资源或传输文件会在首次删除前拒绝并写失败收据。最终重扫与进程盘点不等于原子禁止所有子项创建，目录句柄本身不提供这种保证。

验证命令由 `verification_environment` 在启动前登记 `target/verification-temp` 子目录的实际身份、创建者 PID 与 OS 创建时间，并持有独立账本锁；`TEMP`／`TMP`／`TMPDIR` 仅指向该命令拥有的目录。正常、失败和取消路径均由创建者释放自己的目录及其中测试文件。进程意外退出后，统一 `clean` 只回收身份仍匹配、创建进程已确认退出的已登记空目录；apply 再核账本、引用、活动及子进程清单和 Windows 独占句柄。未登记、非空、被替换或进程状态未知的目录继续保留，不递归猜测删除。

本轮[清理计划](../../.analysis/results/scratch-cleanup-20261030-ble-open-plan.json)与 [apply 收据](../../.analysis/results/scratch-cleanup-20261030-ble-open-plan-applied.json)均已核实：零候选、零删除、零新增释放，189 项保留，保护检查无错误。临时所有权账本的 57 项全部 released，登记路径均不存在，临时根目录为空；上轮释放的 302,739,266 字节只保留为历史事实。本轮 source-organization-plan 仅生成计划，25 个 package／1,755 个 Rust 文件的架构及资源范围检查通过，完整 1,833 项工作区输入与验证收据一致，零移动／改写／未决，未执行 apply。

队列时间边界仅在顶层完成时间有效、协调进程实例已确认退出时释放；未完成、仍存活、身份未知或仅有嵌套旧完成字段时继续保护。capture-batch 的预留同样逐进程核实终态。已结束任务的历史 scratch／argv／command／log 字段可以与真实依赖区分，但 `source_database` 始终参与引用保护。历史 stdout 只有精确 launch／passed 格式与当前完整会话、capture 和实际退出相符时才解除旧日志路径的保护。伪代码明确未完整的现场仍保留；进程退出码为零不足以准许删除未决数据库。

`python -X utf8 tools/workspace.py compact-analysis --workers 8` 仅为原生根层大型数据集生成 `.analysis/results/storage-compaction.json`，逐项核对当前内容 SHA、文件身份与 capture 身份。`compact-analysis --workers 8 --apply --plan-sha256 <实际计划SHA>` 使用 Windows 文件系统压缩，保持文件路径、逻辑字节与完成状态；压缩前后在同一独占写入句柄核对 SHA、文件身份和写入时间，并以 `FileCompressionInfo` 记录实际占用。已有计划和 apply 收据不得覆盖；完整 apply 日志在压缩首项前持久化，结果位于计划同目录的 `*-apply.json`。旧 schema 或未完成 capture 的状态照原样保留，压缩不补造完成证明。该工具只更改文件存储属性，不执行分析输入、厂商 DLL 或应用。

显式 `compact-analysis --review-receipt <项目相对路径>` 还可将当前 `.analysis/results/reverse-pending/review-sources/<SHA>.json` 纳入同一压缩计划。路径必须处于该边界、没有链接，实际 JSON 字节 SHA 必须等于文件名及计划身份；默认扫描不扩展到其他结果或工作目录。apply 仅使用核验后的原计划，不另接新范围。该路径保持不变，完整逻辑字节和 SHA 经压缩前后核验后才记录实际节省空间，禁止通过删字段或改写完成事实缩小正式收据。

[原生补查实际删除收据](../../.analysis/results/scratch-cleanup-native-final-parser-protected-plan-applied.json)另确认移除 48 组已结束私有副本、144 个文件，实际释放 639,054,683 字节，失败为零；删除后各目标不存在，144 份完成依据的实际 SHA 保持不变。未完成伪代码、原始数据库和后续依赖继续保留。[定向审阅收据压缩](../../.analysis/results/storage-compaction-native-review-final-apply.json)保持 161,350,822 逻辑字节及 SHA 不变，实际释放 121,386,150 字节；两项合计实际释放 760,440,833 字节。较早计划中尚缺释放依据的候选没有执行，不计入上述结果。

[共享算术逐例结果压缩](../../.analysis/results/storage-compaction-power5-family-plan-apply.json)另保持 99,839,842 逻辑字节及 SHA 不变，实际释放 22,204,258 字节，失败为零。该结果服务于十五输入的 7,110 个精确解释用例，仍由[当前语义审阅](../../.analysis/results/reverse-pending/review-sources/5365af43eadad4bdfd75f6be500772264d77e32025bdd28e688a46ee95175c6a.json)正式引用，保留原因明确；没有把有效证据当作临时文件删除。

既有[实际删除收据](../../.analysis/results/scratch-cleanup-plan-applied.json)确认 475 份已结束 supplement 私有副本和 9 份旧资源克隆全部移除，共 484 组目录、77,069 个文件、29,478,199,202 逻辑字节，失败为零；删除后逐项目录不存在，1,425 份正式完成收据 SHA 及输入索引保持一致，223 份源数据库仍在。[大型文件压缩](../../.analysis/results/storage-compaction-apply.json)与[中型文件压缩](../../.analysis/results/storage-compaction-medium-apply.json)分别处理 152／574 个文件，实际减少磁盘占用 40,930,720,539／11,801,813,522 字节，全部内容 SHA 保持不变。后续计划跳过已压缩文件；`test_analysis_storage.py` 的 16 项测试通过，包含真实 NTFS 压缩、计划与文件变化、占用、链接、收据防覆盖和提前持久化日志。1 MiB 阈值的[小型文件计划](../../.analysis/results/storage-compaction-small.json)仅完成校验，尚未执行，节省量不计入已执行结果。其他仍有依赖或尚缺释放依据的工作副本保持保留。

[当前资源克隆删除收据](../../.analysis/results/scratch-cleanup-20261029T175624886699Z-plan-applied.json)确认旧上传克隆已不存在，实际释放 146,659,798 字节。[原生补查清理收据](../../.analysis/results/scratch-cleanup-20261030-host896-plan-applied.json)确认两份结束的私有副本、6 个文件已删除，实际释放 22,050,521 字节，失败为零；六份基础数据库及真实 `source_database` 保留。

[四份 strings 压缩](../../.analysis/results/storage-compaction-20261030-host896-apply.json)与[两份 bindings 压缩](../../.analysis/results/storage-compaction-20261030-host896-bindings-apply.json)保留 218,332,819 逻辑字节、原路径与 SHA，实际释放 134,028,947 字节。该组删除与压缩合计实际释放 **302,739,266 字节**，仅计资源克隆删除、host896 原生补查清理及上述两份压缩正式 apply 收据。

对应主机 4.0.896 获取与补查收尾的[历史清理计划](../../.analysis/results/scratch-cleanup-20261030-host896-plan.json)中，189 条保留记录均有明确原因：136 条未完成、失败或未审；26 条未知或同步锁文件；2 条缺少完成依据；2 条上传重建未获证明；23 条仍有引用或活动依赖。两个 1 字节 `.reservation.lock` 是 `index_lock` 的稳定同步入口，OS 锁在 `finally` 释放并关闭句柄，文件按设计保留，文件存在不代表在途任务。
