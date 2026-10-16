# 维护与分析工具

本目录维护 `workspace.py` 入口、静态分析器和配置。完整分析要求见 [实现路线](../docs/re/ui-readonly-first-roadmap.md)。

| 位置 | 用途 |
| --- | --- |
| [workspace.py](workspace.py) | 目录盘点、清理、输入／文档校验和 Rust 验证 |
| [analysis.py](analysis.py) | 前端获取／恢复、原生输入索引、IDA 配置与批量函数导出 |
| [families.py](families.py) | 全产品族 UI／middleware 获取、独立版本配对、native 资源来源闭包与静态恢复矩阵 |
| [asset_release.py](asset_release.py) | 私有资源仓库快照、上传核验、固定提交与有类型运行时索引 |
| [rust_data.py](rust_data.py)、[typed_tables.py](typed_tables.py) | 按实际 Rust schema 校验并生成可读 Rust 数据表、协议能力与本地默认值 |
| [windows_installer.py](windows_installer.py) | Windows 安装包静态输入准备、PE 依赖闭包和封装校验 |
| `analyzers/wakaru-1.14.0/` | 专业静态前端恢复工具及动态 SHA／获取来源记录；可执行文件不入 Git |
| `ida/` | IDA Pro／Hex-Rays MCP 主分析器的项目配置与插件；只做静态分析，配置放项目内，不能放 `.work/` |

在项目根运行：

```powershell
python tools/workspace.py fetch-frontend --url https://apps.razer.com/synapse/dashboard/index.html --output .analysis/inputs/frontend/synapse-dashboard-current
python tools/workspace.py analyze-frontend --input .analysis/inputs/frontend/synapse-dashboard-current
python tools/workspace.py acquire-family --family mouse --layer ui --stage all --workers 2
python tools/workspace.py acquire-family --family mouse --layer mw --stage all --workers 2
python tools/workspace.py acquire-family --family mouse --layer mw --stage native --workers 2
python tools/workspace.py audit-family-function --family keyboard --function mw --query keyboard-metadata
python tools/workspace.py acquire-family --family all --layer both --stage probe --workers 12
python tools/workspace.py acquire-family --family keyboard --layer both --product 515 --stage all --workers 2
python tools/workspace.py audit-family-function --family system --function main --query shared-page-layout
python tools/workspace.py audit-host-local --query macro-recorder
python tools/workspace.py inspect-corpus
python tools/workspace.py acquire-native --include-host-app-resources --no-refresh-gaps
python tools/workspace.py extract-native-packages --scope host-apps --no-refresh-gaps
python tools/workspace.py extract-embedded-inputs --source-prefix .analysis/inputs/ --no-refresh-gaps
python tools/workspace.py acquire-family --family all --layer mw --stage extract-native --include-manifest-mismatches --no-refresh-gaps
python tools/workspace.py reconcile-native-extractions --no-refresh-gaps
python tools/workspace.py analyze-host --managed-scope all
python tools/workspace.py acquire-host-apps --stage report --no-refresh-gaps
python tools/workspace.py acquire-host --metadata-only
python tools/workspace.py acquire-host --revalidate-bootstrap
python tools/workspace.py acquire-host --repair-metadata-index
python tools/workspace.py configure-ida
python tools/workspace.py capture-native --input PROJECTREL
python tools/workspace.py supplement-native --input PROJECTREL --pointer-table-rva 0x1000 --pointer-count 7 --no-refresh-gaps
python tools/workspace.py verify-resources
python tools/workspace.py verify-runtime-assets
python tools/workspace.py prepare-typed-data --check
python tools/workspace.py prepare-device-write-table --check
python tools/workspace.py prepare-local-protocol-tables --check
python tools/workspace.py prepare-storage-tables --check
python tools/workspace.py organize-sources
python tools/workspace.py audit-lfs
python tools/workspace.py prepare-dashboard-images --source-preview-only --no-refresh-gaps
python tools/workspace.py validate-rust --suite asset-pack-core
```

`PROJECTREL` 为 `.analysis/inputs/` 下已登记的 DLL／NODE／EXE。`capture-native` 使用 IDA 批处理与 Hex-Rays 保存全函数、汇编、CFG、xref、类型、字符串、导出／导入和失败；结束后正常退出。`--resume` 补齐同一输入的缺失结果，`--mcp-only` 使用独立私有副本启动查询会话。

`audit-host-local --query macro-recorder` 复用并核对当前 PID192／756 middleware、Macro 和 background-manager 完整 Wakaru 捕获的全部输入、输出与工具字节，保留原 validate 失败；从原始 JS 的实际 UTF-8 AST 区间保存完整 An／q、软件录制和 OTFM set/remove、全局锁与订阅／清理、接收器布局与功能配置 owners，统一写入 `.analysis/results/host-current/local-audit.json`。PID756 的预触发对象没有 exclusive mode，和 background 随后写入的独占记录分开；初始扫描和实时事件的字段层级差异、JSON 记录没有硬件 getter、运行时缓存最后匹配语义也分别保留。实际原生 transport、产品接线、完整页面与真实验收仍须独立完成。

原生批处理与指针表补查使用 IDA 9.1 官方 `pdb.cfg` 支持的 `-Opdb:off`，禁止隐式 PDB 下载阻塞。关闭符号下载不代表原 ABI 已解析。相同 SHA 的活动数据库不得重复启动。

dataset、聚合 C／ASM 和文件摘要使用有界内存写入，不先保留全部字符串或反编译文本；流式输出仍保存全部记录。导出前保存私有 IDB checkpoint；`--resume` 在输入、loader 一致时重新打开已有 packed／unpacked 数据库，不用 `-c` 擦除分析。原失败 session 与日志字节保留于新 session 的 previous_attempts。恢复 DB 用 IDA 只读一致性检查，未解决的不一致阻止完整捕获。

`extract-embedded-inputs` 仅保留在原始程序集内唯一匹配且长度前缀一致的 CLR 资源，以及实际 MZ 文件中边界合法的 CAB 候选；DLL 和不带扩展名的载荷同样扫描。CAB 范围合法不等于可完整解压，后续必须交由静态解包器验证。`acquire-family --stage extract-native` 按真实文件头递归 7-Zip 支持的容器和 Inno Setup，安全核对完整列表；Inno 重建的 `install_script.iss` 只存 results，不冒充原件。交替补提取与资源扫描，直至不再新增原件，再重做 corpus 和 managed/native 捕获。

`reconcile-native-extractions` 将解压失败留下的局部文件及其后代标为未验证，保留实际 bytes／SHA 和失败收据；输入分析、递归提取及 managed/native 捕获拒用这些文件作为原始证据。原始官方 manifest 不匹配同样保留，显式调查不能改成验证通过。

`inspect-corpus --no-refresh-gaps` 的 schema 3 从实际当前 manifest、生产发现与资源字节追踪来源，连接完整静态提取、内嵌区间、packed ASAR 及已验证同字节 aliases；本机路径、版本字符串、继承 verified 字段和单独 ASAR unpacked header 不提供当前版本证明。最新 host 元数据失败不回退旧稳定收据，结束时复核输入索引与全部使用的原件／收据快照。来源字节、依赖／ABI、安装生命周期及产品验收分别记录。

`capture-corpus` 复用已完成捕获及新捕获返回成功后，均重新核对原件实际长度／SHA、capture 输入身份、完整函数数量、唯一 RVA／地址、汇编捕获标记、七类 dataset 与 C／ASM 聚合文件清单和全部产物摘要；输出路径不得越过所属 capture 目录。函数反编译失败与完整汇编捕获分别保留。验证中清单或产物变化同样失败，每项失败保留并继续其余输入；批次结束重新核对所有已验证原件快照及最初实际读取的 corpus 收据 SHA，原件或清单漂移阻止全库捕获结论。

重复 host 提取保留真实 extraction exit／argv。旧重复收据丢失退出码时，静态调用 innounp `-t` 或 7-Zip `t`，保存实际退出码、分析器及输出字节摘要；复用前重新核验输入、检查输出、提取文件和完整 Inno 条目，不编造提取成功。官方 native manifest／资源复查均禁缓存并验证官方 HTTPS 最终地址，previous_observations 保留真实历次差异；quarantine 不因重复观测相同字节而提升为 verified。

`acquire-native --include-host-app-resources` 从当前稳定宿主应用的实际 HTML `link rel=manifest`、原件索引与前后稳定摘要收集 native 清单，并与保留设备清单一同获取；当前 manifest 与调用页面部署不匹配时明确失败，不能单靠新清单提升来源。后续复查保持已选宿主范围，不丢其他设备。`extract-native-packages --scope host-apps`／`--scope all` 复用 family 的完整安全静态容器流程，`--workers` 默认 2，保存真实列表、提取、内嵌及 manifest 绑定；不执行安装器。候选 PE、实际归档提取、无归档限制和语义闭合分别记录。

`inspect-corpus` 中的 `cached_download_fragments` 从当前 installer HTML 实际声明的 JS 静态解析完整下载 owner、常量和 `pindex`／Range 绑定，逐份比较源码定义的偏移与完整官方资源实际 bytes。本轮 15 份 104,857 字节缓存均通过区间核对并独立保留，从完整 PE／其他模块分类排除；不重建原件或转移完整文件身份。父资源的当前官方来源另行验证，策略／字节／快照失败及未证明的父来源均由同一 corpus gaps owner 保留。

`organize-sources` 用 Rust AST 解析现有模块，再按功能生成物理目录迁移计划，同时核对 `include!`、嵌入数据和维护工具引用。默认只写计划；`--apply` 在确认源字节和目标路径未变后逐文件搬移、同步路径，仅移除已经为空的原目录，不操作 Git 索引。完整映射和摘要保存在 `.analysis/results/source-organization.json`，重复规划应没有迁移或路径改写。它保留原模块接口，不替代编译及功能验证。

`audit-lfs` 只读核对 Git 属性、索引中的 LFS 指针和本机对象的实际 SHA-256／长度，同时列出尚未暂存的新增或修改、缺失对象和超过 100 MiB 的普通文件。结果保存在 `.analysis/results/git-lfs-audit.json`，验证前后索引与属性必须不变；不会 stage、commit、push，也不代表远端对象已上传。

`prepare-dashboard-images --source-preview-only` 复用完整的当前 index／manifest 与资源上下文提取，仅重新准备 ProductImage 预览域，避免重复获取全部 Dashboard 图片。仍核对原始字节，保留缺失与提取失败；缺失的已声明依赖沿同一获取流程补取。`--no-refresh-gaps` 仅供拥有最终统一刷新步骤的批次使用。Webpack 数字模块 ID 按 JavaScript 整数字面量静态解码，支持压缩器的 `36e3`、十六／二／八进制形式，拒绝非整数；不执行厂商 JavaScript。

`fetch-frontend` 保存 HTML、manifest、全部已声明资源、静态 chunk／import／CSS 引用、HTTP 失败和版本稳定性检查；原始文件不执行。`analyze-frontend` 显式提交所有获取的 JS，保存完整模块、provenance、输出 maps、诊断及图校验结果，并提取可用的 `sourcesContent`。

相对引用按实际响应 final_url 与 HTML base 解析，边的 source 仍是请求原件，避免服务器补目录斜杠后产生假 404。收据保存当前 requested_urls，过期请求失败与当前缺失分别报告；原件仍全部核对 bytes／SHA。既有裸路径文件挡住子资源目录时，子资源写入 raw-by-url 命名空间，保留原响应不变。

`acquire-host-apps` 沿当前窗口描述符与 `window.open` 获取官方子应用，保留完整 factory／ESM owner。动态 URL 只静态解析字符串拼接及当前部署内可核验的 webpack 导出；重赋值、运行时位置和其他动态表达式保持未解。实际应用版本／build／gitMetadata 来自各自原始 manifest，不能统一称为 native host 版本。资源缺失分别列出 source map 与运行资源，获取稳定和恢复器成功不代替 mounted tree 或本地功能验收。

`acquire-host --metadata-only` 先核对现存宿主收据中的原始 JSON 身份，再复取官方部署 manifests、生产发现及产品发布元数据；跟随更新的 manifest／prod endpoint，双取核对稳定性。新字节按实际 SHA 存放，独立获取收据为 `.analysis/inputs/host-current/metadata-recheck.json`，比较报告为 `.analysis/results/host-current/metadata-recheck.json`；只索引 inputs 下收据，不覆盖旧 acquisition.json、不下载安装包。版本相同或元数据稳定不能证明完整宿主／设备依赖闭包。`--repair-metadata-index` 仅本地校验误登记 results 收据的现存字节，将其原样复制到 inputs 并修正唯一索引项；保留旧 results 文件，在 inputs.json 的 migrations 中记录原索引行、实际字节身份、工具身份及迁移证明。冲突或字节身份不符时拒绝修复，不重新下载或放宽证据边界。

`acquire-host --revalidate-bootstrap` 独立双取当前官方 Synapse 下载页及其明确声明的稳定安装入口，使用 no-cache、官方 HTTPS 重定向与响应身份校验；只保存惰性归档证据，不执行安装器。结果为 inputs／results 下的 `host-current/bootstrap-recheck.json`，保留全部 HTTP 记录、稳定性、实际旧字节比较及扁平前次观测，不覆盖主获取收据。corpus 仅在两份实际页面及安装器样本均匹配后建立 bootstrap 来源根；此证明不代替静态提取及安装生命周期审计。

`refresh-gaps` 对 bootstrap／metadata 复核分别建立 owner，核对收据索引字节、官方 HTTP 200、两份实际样本及稳定摘要，保留未完成、失败及与原 acquisition 的部署差异；旧主收据不能覆盖新的复核失败。

`acquire-family` 不截断候选产品 ID，UI 与 middleware 分别保存原件及覆盖矩阵；获取识别 asset-manifest 和 middleware 的 webpackManifest。`--stage repair` 保留上次失败记录并重试非 404 下载及部署稳定性失败；`--stage analyze` 仅在原件与全部产物的当前字节吻合时复用结果，最多同时运行两个恢复任务。`--stage native` 根据已验证的官方 middleware manifest 获取全部声明资源，并合并已保留的其他产品端点；只校验文件大小、SHA、integrity 与来源，不运行下载的安装器或库。执行完毕、图验证通过、语义审计和产品实现验收分别记录。

`--family` 支持 mouse、keyboard、audio、gamepad、system、accessories 及 all；`--layer both` 同时处理 UI／MW。`--product` 可重复指定单个产品族中的 ID，其余产品的矩阵行保留。批量任务共用已核验的同 URL 资源，并将完整获取和静态恢复分别限制为三项和两项并发；不会随产品族数量乘倍。候选表仅提供端点 ID，不能作为协议或页面证据。矩阵中的获取运行结束、字节核验、声明资源获取、部署稳定性与实现验收分别计数。

批量前端获取和恢复均仅在最外层收尾时刷新一次 gaps，避免每个产品全量重扫及索引锁竞争；单次 `fetch-frontend` 和 `analyze-frontend` 仍默认刷新。缺项清单刷新失败与已保存的产品获取／分析结果分开报告，不将收尾锁失败改写成获取器／分析器失败，也不掩盖原始异常。中断重启仍必须核验输入及所有输出字节，不能按目录存在跳过。

拥有完整收尾流程的调用方可向 `acquire-family` 传 `--no-refresh-gaps`，串行完成矩阵复核、缺失／过期 capture 补齐和源码查询后再统一 `refresh-gaps`；默认行为不变。矩阵复核从实际获取收据和输入／产物字节重新计算状态，不按旧计数跳过刚补获的产品或依赖。

`shared-page-layout` 查询按 HTML／manifest 可达关系核对原件，保留完整 BodyWidgets、WidgetCol、WidgetContainer、CheckItem AST owner、CSS／media 规则和解析器版本，并复用六族当前完整 capture。缺少完整 capture 时仍从已核验的当前原件提取完整 AST，但保留 capture 失败，不将查询标为完整成功。默认使用该族完整 locator 并集，不限已连接设备；带 `--product` 可定向补查。查询失败、源码恢复器失败和缺失资源分别保留，不能把共享容器核对当作整页实现或窗口验收。

`inventory` 盘点目录；`verify` 核对输入字节、完整前端／原生产物 SHA、函数 RVA／原始 bytes、Markdown 链接、Rust 嵌入文件及已有测试记录对应的代码身份。`refresh-gaps` 从现有获取／分析记录重建缺项关联，保留每条图校验问题，不重跑分析器；获取与分析分别归属，补资源不会覆盖已有分析问题。`validate-rust` 执行纯／模拟 library 测试及 workspace 全目标静态检查。`clean` 仅删除 `.work/`，分析器会话仍活动时拒绝清理。

前端恢复使用固定 Wakaru 1.14.0，IDA 路径为 `E:/IDA`（9.1）或 `D:/ida/IDA Pro 8.3`（8.3）。每份捕获核验实际工具版本、输入和产物字节，保留反编译失败。全量提取需保存所有输入、完整函数体、模块／资源引用和失败项；调用、状态、持久化与清理语义逐链核对。结果存 `.analysis/results/`，缺项集中于同一清单。详见 [工具说明](../docs/re/tooling.md)。

`prepare-resources` 核验获取收据和原始 bytes 后准备实际嵌入资源，支持静态 AVIF 转换、受限字面量 React SVG 和已实现的 SMIL 动画采样。`prepare-dashboard-images` 按当前完整 Dashboard 图片调用方，对全部产品 ID 及版本／布局／腕托／鼠标垫请求进行获取和稳定性核验；参数 locator 不提供图片或当前语义。资源失败保持失败，不能生成虚假替代图。全族 scope 合并 registry 与产品候选共 332 个唯一 PID；`--stage report` 从实际获取／probe 收据恢复矩阵字段，分别核对原件与分析产物，不依赖缺失的旧汇总字段。

`python -B tools/workspace.py deduplicate-resources` 只生成 `.analysis/results/resource-deduplication.json`，不移动资源。检查归属、逐字节共享关系、全部生产引用及历史 provenance 差异后，`--apply` 仅接受未变化的完整 dry-run 快照；异常会恢复被改写和删除的文件。单产品图片及配置归入 `products/<PID>/`，跨产品内容显式共享，无可证归属的内容隔离至 `shared/source-unverified/`。迁移保持逻辑 alias 顺序、实际 bytes、各来源记录及历史 `original_paths`；源码未核验与物理去重分别记录。资源准备与迁移共用 OS 文件锁和实际内容索引，后续准备复用已有相同 bytes；准备引入新的跨产品使用后，应再次运行此布局核对。apply 后须明确重新准备资源发布快照、固定提交与运行时索引，复做相关族审计及适用纯测试；该工具不修改 Git 索引、历史或 LFS 配置，也不执行应用或厂商代码。

完整原图的 `verify-resources` 属于显式准备原文件后的维护校验：SVG 解析完整 XML，检查遗留 React 属性和 `<use>` 的内部目标；位图使用固定 Pillow 验证结构并完整解码像素。输入摘要、逐文件结果、失败及验证期间的图片身份保存在 `.analysis/results/resource-audit.json`，失败与过期收据进入统一 gaps。它不属于 Cargo 前置步骤。资源纯测试通过编译索引检查当前 alias 与产品分支，使用必要的内联合成 SVG／PNG 检查锁定 GPUI codec；真实产品图片不放进 `testdata/`，也不在测试中全量加载或下载。AVIF 的维护转换仍须从核验过的原件准备，两类检查均不能代替布局、交互或设备验收。

工具路径相对项目根，证据哈希从原始字节生成。所有目标只作静态数据处理；禁止执行厂商应用／JavaScript／DLL／helper／installer、真实服务和真实设备。允许的 Rust 检查与纯／模拟测试见 [AGENTS.md](../AGENTS.md)。

`keyboard-metadata` 复用当前 middleware 获取收据，逐产品校验原件字节并保留完整注册、DeviceInfo、继承类、getEdition／getSerialNumber、命令／解码器、传输默认值、物理路由工厂及身份缓存／runtimeData 默认值所有者。结果追加至 `keyboard-mw-audit.json`；通过检查的有线、自有无线接收器及 BLE 产品生成 `razer-device` 元数据传输描述，双链路共享接收器另保留 cached-master 工厂及命令命名空间证据。原始依赖闭包缺项独立保留，不把厂商 UI 的布局／版本默认值视为硬件读取成功。该查询不执行厂商代码或设备操作。

应用资源按“私有 HF 仓库 → 当前实际文件请求 → 安装目录 `assets/cache/` → 当前产品界面”流转。`tools/asset-release.json` 固定仓库提交、文件长度／SHA-256 和产品依赖；`prepare-runtime-assets` 解析代码维护表并生成 `assets/synapse/tables/remote-assets.rs`，`verify-runtime-assets` 检查输出一致性。两者无需本地图片，也不下载资源；`--check-local` 仅用于明确恢复了原文件的维护工作区。运行时异步请求去重、限制并发、验证并原子缓存；缓存命中不联网，编译不准备 pack。凭据仅从本机配置读取。

`verify-asset-release --remote` 核对已固定远端清单及 LFS 文件身份；`publish-asset-release` 在核验后固定新提交；`pin-asset-release --revision COMMIT` 核验已上传版本。`fetch-asset-release --product PID` 仅为维护人员显式恢复选定产品与共享依赖，普通编译和产品启动均不调用。完整原图核验须明确使用维护来源，不将其复制到 crate 测试夹具。

`prepare-typed-data [--check] [--crate CRATE]` 使用 `tools/typed-data.json` 和 `.analysis/inputs/typed-tables/` 的维护输入，按实际 Rust struct／enum 与字段、值域、产品分支生成可读构造表；专用设备写入、读取／身份／接收器、存储默认值分别由 `prepare-device-write-table`、`prepare-local-protocol-tables`、`prepare-storage-tables` 维护。生成物随程序编译，产品不解析这些固定 JSON；外部报文、动态扩展和用户配置只在有类型的边界读写。原 JSON 不在 crates 中保留第二份生产数据。

`prepare-windows-installer` 从已存在的 GUI EXE 和显式 DLL 准备严格载荷，`verify-windows-installer --stage DIRECTORY` 复核实际 SHA、PE 架构、普通／延迟导入闭包和图标；`compile-windows-installer` 仅调用已有 ISCC 生成安装包。当前缺 release EXE 与 ISCC，没有生成或运行安装包。输入、缓存、升级与卸载语义见 [Windows 安装交付](../docs/windows-installation.md)。

独立 `asset-pack-core` suite 只验证容器 wire fixture 与错误处理。保留的 `audit-asset-pack`／`prepare-asset-pack`／`verify-asset-pack` 属于显式本地容器维护，需要完整原文件与相符清单；生成的 `target/asset-pack/synapse.pack` 不属于当前应用运行或安装依赖。旧全量 `SynapseAssets.list/load` pack 回环已移除；当前资源纯测试验证索引闭包、产品选择、缓存和模拟请求，只用必要的内联合成 codec 样本，不下载全套图片。格式和边界见 [资源交付与容器格式](../docs/asset-pack-format.md)。

`python tools/workspace.py organize-resources --apply` 将资源表归入 `tables/`，来源清单归入 `manifests/`，索引归入 `indexes/`，同步 Rust include、JSON 物理路径和文档引用；产品配置保留在 `products/<PID>/configs/`。目录索引只保留实际输出：被更新的活跃 alias 绑定到当前已核验文件，已无生产引用的悬空记录从当前索引移除，其完整旧记录与判定保留于同一 `resource-deduplication.json`。逻辑 alias 不改名。
