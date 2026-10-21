# 静态逆向工具与核验边界

IDA 路径：`E:/IDA`（9.1）、`D:/ida/IDA Pro 8.3`（8.3）。GPUI 技能读取 `.agents/skills/`。各份捕获记录实际工具版本，不能跨版本沿用验证结论。

当前 `HID.node`（SHA-256 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`、405,704 字节）的 [实际捕获收据](../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/capture.json) 使用 IDA 9.1、Hex-Rays 9.1.0.250226、Python 3.11.9，完整捕获 1,641 个函数；RVA `0x30888` 的反编译失败及完整汇编保留。其他 IDA 8.3 捕获的函数数与产物摘要不能套用到这份收据。[HID 查询](../../.analysis/results/native/chain-audit.json) 中的工厂、回调、动态导出与共享 worker 链须分别对照其引用的完整函数及实际产物身份；指定查询没有未解边不代表全部 N-API／Promise、产品协议或生命周期已经闭合。当前 GetFeature／GetInput 的完整函数分别位于 RVA `0x17ac0..0x17c43`、`0x17c50..0x17dd3`，Windows 返回长度均为原生已读长度加一；Rust 的 source-host collection 适配与产品重试／取消仍须逐调用方校验。

`capture-corpus --source retained --ida-root E:/IDA --include-path '\.(dll|node|sys|customactionkey)$' --no-refresh-gaps` 本批选择 884 个不同字节的非 EXE 模块。工具核验完整旧捕获的所有产物后复用，对缺项使用当前 IDA／Hex-Rays；未选 EXE 保留在同一队列，不能把本批结果记为全语料闭合。保留原件的当前版本来源仍另行审计。进度、失败、输入快照及实际工具版本见 [统一原生队列](../../.analysis/results/native-corpus-capture.json)，不使用未结束旧队列的累计数作为本轮完成结论。

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

已保留实际输入并完成全函数捕获的基础模块包括 HID.node、mapping_engine、simple_service、BLE binding、Razer.WMI、SysUtilsNative、两份 Seiren DLL 和 GenericCamera。数量与 Hex-Rays 失败以各自收据为准。当前 GenericCamera 1.0.35.0／1.0.36.1 基准收据实际使用 IDA 9.1、Hex-Rays 9.1.0.250226、Python 3.11.9，分别保留 2817／2825 个函数；每版各保留一个 UTF 辅助函数的原始反编译失败及完整汇编。两份原件、工具、私有数据库检查点及完整产物摘要均记录在对应 [native 结果](../../.analysis/results/native/)。

GenericCamera 的两个 UTF 失败 owner 已在独立私有副本进行类型恢复，每版补查 7 个完整函数且失败为零，保留实际二／四参数调用证据；原始失败记录不改写。另分别提取两版 base／Pro／X 的六张完整虚表和所有目标，旧版 44 槽、新版 46 槽。新增 shutter-limit 槽使后续偏移变化，Pro RestoreDefaults 的实际命令也不同；这些差异不能由同名导出或函数数量推断。八份补查的精确收据、RVA、协议和版本边界见[摄像头](../products/accessories/camera.md)，不代表所有摄像头业务或真实 DLL／设备对照完成。

两份 Seiren DLL 各 1,947 个函数中的唯一调用分析失败点 RVA `0x3318e`，已使用 IDA 9.1、Hex-Rays 9.1.0.250226、Python 3.11.9 对私有数据库副本定点补查。完整指令和交叉引用证明其为四寄存器参数的 vector reserve 路径，原分析器误推六参数；每份补查各保存八个完整函数，失败及数据库不一致均为零。原全库捕获及其失败记录保持原样，统一 native 收据连接补查的实际字节身份；不能由该点闭合推导其他间接 ABI、产品实现或运行验收已完成。

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

主程序在 GUI 初始化前安装标准库 panic hook，日志由 `razer_storage::store_path()` 定位到同目录的 `panic.log`，Windows 通常为 `%APPDATA%\razer_ui\panic.log`。记录进程、线程、时间、Rust panic 位置／内容及调用栈，并保留原 hook；日志不可写或锁不可用时尽力输出到 stderr，不阻止启动或吞掉 panic。它只补充 Rust panic 诊断，不能保证捕获原生 failfast、abort 或系统直接终止，隔离服务 worker 也不使用这份主程序日志。写入及真实崩溃记录受窗口运行限制，静态核对与编译不算运行验收。

输入获取、静态恢复、语义、Rust 实现、UI／服务连接、纯／模拟验证、运行验收分别记录。证据 SHA 不作为产品兼容门禁。共享协议逻辑独立于 OS，HID、COM、驱动、服务和系统适配保持明确边界。

缺失原件、间接调用、ABI／协议分支、未实现本地链进入统一 `.analysis/results/gaps.json` 并驱动后续实现。账号、云同步、远程下载／更新、在线目录等联网产品功能移除；loopback、本地 socket、固件文件传输、DLL 替换、服务生命周期、本地预设仍须实现。纯／模拟测试不冒充设备写回；本地 DLL／导出、服务及设备功能测试按根目录已授权的来源、架构、ABI、清理链核实和只读优先边界执行，可恢复写入保留原值、恢复与回读结果。应用窗口、厂商 JS、无关 helper 和安装器仍不运行；测试实际调用、返回、比较及未验证范围单独记录，不能由加载成功或单次 getter 推导整页验收。
