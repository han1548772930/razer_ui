# 静态逆向工具与核验边界

IDA 路径：`E:/IDA`（9.1）、`D:/ida/IDA Pro 8.3`（8.3）。GPUI 技能读取 `.agents/skills/`。各份捕获记录实际工具版本，不能跨版本沿用验证结论。

2026-10-16 对官方保留原件 `HID.node`（SHA-256 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`）重新全函数静态捕获，实际 IDA 8.3、Hex-Rays 8.3.0.230608、Python 3.11.6；1,679 个函数、1,688 个产物逐字节校验，RVA `0x30888` 的反编译失败及完整汇编保留。[HID 查询](../../.analysis/results/native/chain-audit.json) 对 34 个当前产品来源完成工厂分支核验，并连接 31 个完整 native 函数、23 处已解析间接调用：12 个 `hid.dll` 动态导出全局、批量 Feature 回调及共享 worker 销毁链。此处 0 个未解间接调用仅指所选函数和指定调用上下文，不代表所有 N-API／Promise 回调或全部产品链已解析。捕获不能视为全部产品读写完成；未执行目标程序或设备操作。

维护入口统一为 `python tools/workspace.py`。原件位于 `.analysis/inputs/`，获取收据及索引保存实际字节 SHA-256；结果位于 `.analysis/results/`，临时数据库位于 `.work/`。不运行厂商应用、JavaScript、目标 DLL／导出、helper、installer、调试器或真实设备。

## 当前工具

| 工具 | 当前用途与身份 | 能力限制 |
| --- | --- | --- |
| IDA／Hex-Rays | 对已验证私有副本导出全函数、汇编、伪代码、types、strings、imports、exports、xref | 捕获完整不等于全部反编译成功、ABI 已解析或功能已实现；失败保留完整汇编 |
| IDA MCP | `tools/ida/` 固定 2.0.0 快照，提交 `c133c3853faa111a9b00ee615c013b720d0c4acd` | 本轮全函数使用批处理；配置存在不等于连接验收，不调用执行方法 |
| IDAPython／Python | IDA 8.3 捕获实际使用 Python 3.11.6；9.1 配置保留固定 Python 3.11.9 embed | 与产品依赖分开，按各份捕获核验实际运行时，不跨版本沿用身份 |
| Wakaru 1.14.0 | 全量显式 JS 输入，静态拆包、provenance、输出 map、diagnostics | validate、chunk 枚举和 source map 缺失独立记录；成功退出不证明依赖闭合 |
| Tree-sitter 0.25.2／JavaScript 0.25.0 | 按 Python ABI 使用项目内解析器，静态定位完整源码 AST 和原始字节范围 | ERROR／missing node 不允许当成功提取；禁止执行 JS |
| Rust 固定表生成器 | 项目内 Tree-sitter 0.25.2／Rust grammar 0.24.2，`prepare-static-python` 按实际 Python ABI 恢复；`prepare-typed-data` 按实际 Rust 字段和枚举生成多行构造代码，可按 `--symbol` 定向校验；`tools/typed-data.json` 保留输入 SHA-256、字段投影及格式边界 | 仅解析维护输入；不执行厂商代码。恢复自 Rust 的生产字段投影独立标记，不冒充原始证据 JSON；生成成功不代替编译、行为等价性或设备验收 |
| SQLite 资源目录生成器 | `prepare-resource-database`／`prepare-emoji-search`／`prepare-runtime-assets` 共用 `resource_database.py`；本轮 Python SQLite 3.50.4，schema 1；Rust 使用锁定的 rusqlite 0.40.2／libsqlite3-sys 0.38.2 bundled，整数读取做溢出校验 | 仅迁移静态表情展示数据与资源索引；不迁移设备协议或用户配置。全字段、顺序、重复项回读核验独立于真实窗口与安装验收 |
| ccl_chromium_reader 0.24 | 固定提交 `ef840de30221c4d65bc96d2f4d9057e9ef2f526d`，8 个文件按收据校验 | 只读取公共官方静态缓存，不读取账户、cookie、浏览历史；缓存不是当前部署证明 |
| 7-Zip | 新提取使用 24.02 x64，已有 26.02 收据保留各自身份；版本及 EXE／DLL SHA 随静态提取记录 | 仅识别为 PE 的安装器不能记成已提取全部载荷 |
| ILSpyCmd 9.1.0.7988 | 固定 NuGet 包与 59 个文件，.NET 8 上静态导出 C# 项目和完整 IL | 不执行输入程序集；保留 unresolved 注释、stderr、两阶段退出码；native C++ 使用 IDA |
| innounp 2.71.2 | 当前官方发布资产与完整源码提交独立固定于 `tools/analyzers/innounp-2.71.2/`；静态列举／提取 Inno Setup | 旧 2.71.1 发布资产已不可获取，原收据的恢复失败保留，不用新版本字节替换旧摘要；重建 install_script.iss 只作为分析产物 |

工具获取收据位于 `tools/analyzers/`；前端分析器准备记录位于 [.analysis/results/analysis-preparation.json](../../.analysis/results/analysis-preparation.json)。`analyze-host` 按既有 ILSpy 收据恢复精确匹配的官方包成员，不改写预期摘要。

`prepare-emoji-search` 对 mouse190、keyboard679、Macro 与 Dashboard 的已保留 JS 原件核对实际字节和获取收据，用静态 AST 读取完整字符串，逐项比较 1,936 条 `(emo, name)` 及顺序。SQLite 保存 1,857 条唯一记录，顺序表保留 79 个重复项；全部分组、tab、变体及排除列表分别回读核对，保留 Macro 与 Mouse 分支差异。初始词表共享迁移的非搜索 Rust token 等价性、最终数据库全字段往返核验、完整 callable owners 和解析器版本存于统一 [固定表记录](../../.analysis/results/typed-data.json)。旧重复生产数据与 Rust 资源索引已清理，设备协议表继续使用 Rust 类型数据。搜索数据相等不代表各页面过滤规则相同，也不提升原件部署稳定性或依赖闭包状态。

## 主机与安装证据

[本机获取收据](../../.analysis/inputs/installed-current/acquisition.json) 保存 564 个安装文件、5,093 个公共缓存容器和 5,093 份响应资源，获取失败为 0。六份分组包括 PID679 UI／MW、PID190 UI／MW、Synapse Dashboard 和 Chroma Dashboard，无同 URL 字节冲突。通知、设置、宏、配置文件、灯光引擎等其他缓存也保留在同一收据，需沿实际调用链获取当前官方部署后使用。

服务目录只快照 DLL／EXE／NODE 和明确的 `.deps.json`／`.runtimeconfig.json`。日志、数据库、运行配置和活动数据在读文件前排除。产品本地存储仍通过代码、部署清单、模拟数据完整分析，不将用户活动记录当实现证据。

[官方 host 获取](../../.analysis/inputs/host-current/acquisition.json) 本轮取得 36 个文件；六个部署元数据端点前后实际摘要一致，Synapse／Chroma 稳定部署为 4.0.827。主包长度、MD5 和声明逐项核对，但 `host_package_closure_complete` 仍为 false。

官方包静态解包原件与本机主 EXE、ASAR、mapping_engine.dll、simple_service.dll、HID.node 实际 SHA 一致。ASAR 为 `b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6`，静态提取 10,076 个文件，0 个提取未解项；这不消除源码图、外部资源或 ABI 未解项。

[host JavaScript 捕获](../../.analysis/results/host-current/javascript/capture.json) 保留 230 份 JS／CJS 和 693 个恢复产物。106 个引用 source map 未包含在包中；unpack 退出 0、validate 退出 1。原始文件是语义与字节证据，可读文件和合成 import／export 只辅助查询。

`audit-host-local --query macro-recorder` 从当前 PID192／756 middleware、Macro 和 background-manager 原件保留完整 AST owners 与真实 UTF-8 byte 区间，并复核四套既有 Wakaru 捕获的全部原件、产物和分析器字节。完整捕获中的 validate 非零结果与获取缺失不升级为成功；统一 [local host 查询](../../.analysis/results/host-current/local-audit.json) 保留软件录制与 OTFM 写入／移除、PID756 非独占预触发与全局广播、background OTFM 全局锁与完整状态机、An 初始扫描与 q 事件字段差异、接收器缓存最后匹配、serialNumber 筛选及当前功能注入配置。JSON 记录不会补造 getHWModule；原生 transport、完整页面与真实运行验收仍未闭合。

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

每份 `.analysis/results/native/<sha>/capture.json` 保存输入身份、image base、函数边界／RVA、函数块原始 bytes、完整汇编／伪代码、局部类型、结构、xref。每个输出独立 SHA 校验。`--resume` 仅在输入、IDA／Hex-Rays 版本及捕获工具摘要一致时复用函数，并核对函数文件摘要、边界。

IDA 9.1 的批处理与补查明确传入 `-Opdb:off`，依据安装的官方 `cfg/pdb.cfg` 禁止隐式联网下载 PDB；选项与策略保留在 plan／session。它解决分析器下载阻塞，不提供缺失符号或 ABI 证明。

大型 EXE 的候选字符串不能先汇成 Python 全列表。`tools/ida/capture_io.py` 将 dataset 逐记录写成完整 JSON array，聚合 C／ASM 逐函数写入，文件 SHA 按块计算；不截断字符串、函数或 xref 范围。导出前保存私有 packed IDB checkpoint；`--resume` 复用已验证的 packed DB，或明确记录 unpacked DB 恢复及组件摘要，不传会重建数据库的 `-c`。失败尝试与实际日志 bytes／SHA 嵌入同一 session 的 previous_attempts，不覆盖为成功历史。恢复数据库额外通过 IDA `validate_idb(VLD_SILENT)` 只读核验；非零不一致记录阻止 capture_complete，不能把可打开或已修复提示当作完整数据库证明。

恢复库完整性失败时，capture 写明失败阶段并在导出 datasets／函数前退出；下次 `capture-native --resume` 将已诊断失败的数据库逐文件移入同一私有 scratch 的 `invalid-database/<实际 SHA>/`，先校验源／目标绝对路径边界并保留实际长度／SHA。session 的 previous_attempts 同时保留失败 capture 原字节和日志，随后只从已验证 EXE 以 `-c` 建立新库。AppEngine 恢复库本次检出 1,238 处不一致，已保留失败收据并开始干净重建，不能算全函数已捕获。

聚合 C／ASM 完整写入临时文件后才发布，失败保留先前完整输出；捕获收据在聚合文件和全部输出摘要写完前保持 `capture_complete=false`。队列复用完整捕获及新捕获返回成功后，均重新核对实际原件长度／SHA、manifest 输入身份、完整函数数量、唯一 RVA／地址、汇编捕获标记、七类 dataset 与 C／ASM 聚合文件清单及全部产物摘要；不单独信任成功标记。反编译失败不会伪装为完整伪代码。验证中清单或产物变化逐项失败，记录实际捕获清单身份及逐原件前后快照。队列结束时再次复核全部已验证原件快照及读取的语料收据实际 SHA，先完成的原件在后续分析期间改变也会阻止完整批次结论。

本轮重新取得实际输入并完成全函数捕获的基础模块包括 HID.node、mapping_engine、simple_service、BLE binding、Razer.WMI、SysUtilsNative、两份 Seiren DLL 和 GenericCamera。数量与 Hex-Rays 失败以各自新收据为准。2026-10-16 以 IDA 8.3／Hex-Rays 8.3.0.230608、Python 3.11.6 和当前维护捕获工具完成 GenericCamera 1.0.35.0（2,860 个函数）及 1.0.36.1（2,868 个函数）的完整静态捕获；前者 RVA `0x870c0`、后者 RVA `0x87ad0` 的调用分析失败及完整汇编如实保留。两份原件、工具摘要、私有数据库检查点、全部函数／数据集／聚合产物摘要均记录在对应 [native 结果](../../.analysis/results/native/)；旧 1.0.36.1 收据缺少捕获工具摘要的问题已通过实际重捕获消除，未手填分析器元数据。

当前 3595 麦克风查询沿完整 `Ph({})`／动作／reducer／中间件／FFI 到 1.0.35.0 导出和基类 vtable `+304/+312/+80/+336`，逐字节核对设置、读取、闪存保存、重启及两次 10ms 间隔。`DeviceInitialize`、精确 container-ID 匹配、PID `0x0e0b` 工厂分支和基类构造函数的完整函数及实际 PE 指令也纳入查询，验证该产品确实取得相应 vtable。原版开关只设置 dirty，重启动作才执行本地写回；Rust 保留该语义并分离实际传输诊断。证据纳入既有 `camera-local-pages`／`camera-source-review` 查询，纯状态和模拟 KS 测试通过 `camera-mic-state`／`camera-mic-protocol` 套件记录；这些不代表整页或真实设备验收完成。

`audit-native-chain` 连接完整原始 callable、FFI 签名、精确导出、native 函数、imports／xref 和实际 Rust 文件摘要。查询包括 HID feature、mapping engine、simple service、restart notification，统一报告为 [native/chain-audit.json](../../.analysis/results/native/chain-audit.json)。查询无失败不代表完整页面／设备验收；回调、间接目标和 ABI 未解项继续追踪。

chain schema 2 的每条 native edge 以完整函数产物的实际 path／bytes／SHA 引用目标，不重复复制目标的全部入口 xref 和 chunks；全部边均保留，完整汇编、伪代码、原始块和交叉引用仍由对应 capture／functions 原件结果提供。此调整避免高扇入函数让关联报告呈平方膨胀，不减少分析范围。

restart 查询使用新取得的 [notification-manager 当前原件](../../.analysis/inputs/frontend/notification-manager-current/acquisition.json)，保留 Dashboard caller、WindowStorage 内存／事件、通知 reducer／窗口、完整确认页／样式／语言、FFI 派发和 SysUtilsNative 的 `restartWorkstation`。该导出 RVA `0x70370` 尾调用 `WinExec("shutdown -r -t 0", 0)`，未执行重启。重复同 caller 的 true 生成新数组，重新触发窗口检查；已有窗口不主动 focus；LATER 的 disabled CSS class 不等同 HTML disabled。

`capture-corpus --include-path REGEX` 只决定分批分析顺序；未选项保留在队列，子集成功不能写成全语料闭合。并发分析使用 `--no-refresh-gaps`，由汇总所有者在收据写入后统一刷新缺项。

全函数枚举仍可能漏掉真实指针表指向的跳转入口。`supplement-native --input PROJECTREL --pointer-table-rva RVA --pointer-count COUNT` 从原始 PE 的实际、文件支持、可执行目标验证指针表，在既有私有 IDB 的副本中定点补查；`tools/ida/supplement.py` 只允许为字节精确的 x86/x64 直接跳转或 x64 内嵌对象析构转发 thunk 创建缺失边界，其他未知边界保持明确失败。后者须完整匹配 13 字节的取 vtable、同位移调整 this、清零删除标志和间接跳转序列，每条指令仍由 IDA 解码；仅补函数边界不证明虚调用目标。HID 的 `0x10ae0` 首次因原直接跳转规则被拒绝，扩展规则后补获完整汇编／Hex-Rays；当前构造函数及原始 `0x51ba0` 表另证明其调用 `0x4710` worker 析构。补查保留七类信息：原表字节、slot/RVA、完整函数块原字节、汇编、伪代码及类型、xref、直接跳转链。已有完整函数按实际原字节及输出摘要复用，原全库收据与数据库不改写；补查结果位于同一 native SHA 下的 `supplements/`。补查完整不表示 RPC ABI、间接调用、产品功能或真实运行验收完成。

## 前端、资源与完整性

Wakaru 1.14.0 固定源码提交 `8219bf5016a063d7fb215101a8c45d5959ec733f`，使用 `--unpack --level minimal --provenance --emit-source-map --diagnostics --json`，不启用 DCE。显式提交全部 JS；输入 map 的 `sourcesContent`、原始 UTF-8 `[start,end)`、合成 map、恢复模块分别记录。

manifest、HTML、JS、CSS、资源及部署前后摘要共同确定当前入口。source map 404、动态 publicPath、未知 chunk、枚举 null、未解 import 独立记录。语法可解析不能证明 mounted tree、版本分支、状态、Promise、设备写回、持久化或清理完成。

前端获取在单个依赖批次内并行静态枚举独立 JS，遍历及收据发布仍由单一所有者处理。每份 `chunk-references/*.capture.json` 保存确切输入路径／字节、Wakaru 字节身份、参数、完整 stdout／stderr 摘要、退出码和输入前后校验；只有这些身份全部吻合才复用，不以旧输出文件存在代替验证。复用非零退出或 null 枚举仍保留原失败／未解状态。产品族获取合并发布原件索引，避免每产品反复重写完整索引；获取收据与完整分析、语义和实现验收继续分开。

宿主应用 native 资源经 `acquire-native --include-host-app-resources` 与设备资源统一获取。来源必须是实际稳定 HTML 的 manifest link、索引吻合原件及稳定摘要，不能仅凭缓存名或手填 URL；新 native manifest 与页面部署字节漂移时保留失败并要求沿原流程补取页面。设备 manifest 按真实 PID 路径核对，`host_app` scope 按稳定宿主 HTML 声明核对，二者都要求官方 HTTPS。`extract-native-packages --scope host-apps --workers 2` 复用完整静态容器提取，本轮 139 份 PE／容器候选中 16 个容器提取 676 个文件、错误 0；普通 PE 的无归档结果不算安装分支闭合。

`inspect-corpus` 静态解析当前 installer HTML 实际声明的 JS，保留完整下载 owner、数值常量声明、AST 范围、工具版本及实际 SHA。源码以 `Math.floor(.1*(1024*1024))` 得到 104,857 字节分片，`pindex`、循环偏移、请求 start／end 和 Range 绑定均须吻合；再将每份缓存与完整官方资源的精确字节区间比较。本轮 15 份通过的分片在 [corpus](../../.analysis/results/corpus.json) 独立记录并从完整模块分类排除，原缓存保留，不拼装或转移整文件身份；完整资源的当前来源另行核对。策略、区间、快照或父资源来源失败均进入统一 gaps。

本轮 corpus 当前记录 native 1,940、CLR 683、混合 9、其他 39，分类失败 0；500 个官方来源根连接 39,995 条容器边。29 份 native 原件的当前来源仍未证明，包括 8 个 manifest mismatch 资源及其独有后代、两份旧 quarantine、两份已安装 DLL 和一份安装时生成的 GMS uninstaller；重复获取不能提升这些原件的身份。此来源盘点独立于尚在推进的全函数捕获、语义及产品实现。

host 子应用的 [当前部署审计](../../.analysis/results/frontend/host-applications.json) 逐文件核对原件，保存各自 manifest 版本、build、gitMetadata、dependencies／resourceManifest；native host 元数据只作上下文。窗口 URL 保留完整 factory／ESM 所有者及表达式，常量拼接沿实际 webpack require／export 解析，拒绝重赋值及运行时 location；运行资源与 source map 缺失分别计数，资源闭包不冒充页面与功能完成。

相对 URL 依据实际 HTTP final_url 及 HTML base 解析，引用边仍归属请求原件；收据同时保存请求身份、响应地址、解析策略和当前 requested_urls。裸目录路径先被保留为文件后遇到子资源时，子资源使用显式 per-URL 存储命名空间，原响应字节不搬移或改写。旧错误基址产生的失败记录与当前请求缺失分开，所有保留原件仍逐项校验字节。

资源准备使用现有统一 `prepare-resources`、`verify-resources`、`prepare-dashboard-images` 和 asset pack 命令。图片结构／像素、GPUI 实际 CPU codec、资源包读取和窗口显示是不同验证层级。转换工具不执行 React，仅处理已支持字面量 SVG／CSS，保存原始及派生 SHA。旧文档的资源与测试数量不能当本轮通过结论。

GPUI 修改前读取用户指定目录的两个 SKILL.md 及适用机制／组件参考，核对 Cargo.lock 版本；页面布局、控件、状态及交互由当前原件证明。

输入获取、静态恢复、语义、Rust 实现、UI／服务连接、纯／模拟验证、运行验收分别记录。证据 SHA 不作为产品兼容门禁。共享协议逻辑独立于 OS，HID、COM、驱动、服务和系统适配保持明确边界。

缺失原件、间接调用、ABI／协议分支、未实现本地链进入统一 `.analysis/results/gaps.json` 并驱动后续实现。账号、云同步、远程下载／更新、在线目录等联网产品功能移除；loopback、本地 socket、固件文件传输、DLL 替换、服务生命周期、本地预设仍须实现。纯／模拟测试不冒充设备写回；真实窗口、应用、服务和设备验收依执行限制保持未验证。
