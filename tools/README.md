# 维护与分析工具

本目录维护 `workspace.py` 入口、静态分析器和配置。完整分析要求见 [实现路线](../docs/re/ui-readonly-first-roadmap.md)。

| 位置 | 用途 |
| --- | --- |
| [workspace.py](workspace.py) | 目录盘点、清理、输入／文档校验和 Rust 验证 |
| [analysis.py](analysis.py) | 前端获取／恢复、原生输入索引、IDA 配置与批量函数导出 |
| [families.py](families.py) | 全产品族 UI／middleware 获取、独立版本配对、native 资源来源闭包与静态恢复矩阵 |
| [asset_release.py](asset_release.py) | 私有资源仓库快照、上传核验、固定提交与有类型运行时索引 |
| [catalog_data.py](catalog_data.py) | 完整构造 AST 等价校验、去重目录编码及所属模块的普通有类型读取函数 |
| [resource_database.py](resource_database.py) | 生成／回读核验随程序交付的 SQLite 表情、界面词表、页面分支与资源索引 |
| [resource_scope.py](resource_scope.py) | 检查 Rust overlay／延迟绘制边界的资源缓存所有者，由统一维护入口调用 |
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
python tools/workspace.py audit-family-function --family keyboard --function main --query global-shortcuts
python tools/workspace.py audit-family-function --family mouse --function main --query mouse-tracking --prepare
python tools/workspace.py audit-family-function --family mouse --function main --query mouse-calibration --prepare
python tools/workspace.py audit-family-function --family system --function main --query shared-preferences
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
python tools/workspace.py audit-runtime-resources
python tools/workspace.py prepare-app-picker --check
python tools/workspace.py prepare-static-python
python tools/workspace.py prepare-media-analyzer
python tools/workspace.py prepare-native-cursors --sources INPUTJSON --group 102
python tools/workspace.py prepare-emoji-search --check
python tools/workspace.py prepare-locale-exports --acquisition .analysis/inputs/frontend/mouse-190-current/acquisition.json --source .analysis/inputs/frontend/mouse-190-current/raw/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js --check
python tools/workspace.py prepare-resource-database --check
python tools/workspace.py prepare-typed-data --check
python tools/workspace.py prepare-typed-data --check --record-verification
python tools/workspace.py prepare-typed-data --symbol TEXT_DATA --symbol KEYBOARD_MAPPING_TEXT_EMOJI_DATA --symbol MOUSE_MAPPING_TEXT_CURRENT_DATA --symbol SHORTCUTS_CURRENT --check
python tools/workspace.py prepare-device-write-table --check
python tools/workspace.py prepare-local-protocol-tables --check
python tools/workspace.py prepare-storage-tables --check
python tools/workspace.py organize-sources
python tools/workspace.py reconcile-source-index --frontend RECEIPT --publication MANIFEST
python tools/workspace.py audit-reference --directory REFERENCE --commit COMMIT --retained-snapshot
python tools/workspace.py audit-lfs
python tools/workspace.py prepare-dashboard-images --source-preview-only --no-refresh-gaps
python tools/workspace.py validate-rust --suite asset-pack-core
python tools/workspace.py validate-rust --suite workspace --suite model --suite storage --suite assets-resource-runtime --suite assets-resource-identity --suite assets-fixed-icons
python tools/workspace.py validate-rust --suite mapping-engine --suite mapping-service --suite mapping-connections --suite mouse-tracking-protocol --suite mouse-tracking-session --suite mouse-tracking-ipc --suite mouse-tracking-routing --suite mouse-tracking-service --suite mouse-tracking-state --suite mouse-tracking-shell
python tools/workspace.py validate-rust --suite profiles-transfer-codec --suite ui-preferences-state --suite mouse-philips-protocol --suite mouse-philips-ipc --suite mouse-philips-routing --suite mouse-philips-service --suite mouse-philips-shell --suite mouse-philips-state
python tools/workspace.py validate-rust --suite macro-recording-ipc --suite macro-shortcut-resume
```

`PROJECTREL` 为 `.analysis/inputs/` 下已登记的 DLL／NODE／EXE。`capture-native` 使用 IDA 批处理与 Hex-Rays 保存全函数、汇编、CFG、xref、类型、字符串、导出／导入和失败；结束后正常退出。`--resume` 补齐同一输入的缺失结果，`--mcp-only` 使用独立私有副本启动查询会话。

`prepare-static-python` 按当前 Python ABI 恢复项目内固定版本的 JavaScript／Rust 解析器，校验发行元数据、wheel 字节及两种语法的静态解析。`prepare-typed-data` 支持重复的 `--crate`／`--symbol` 定向选择；选项不匹配时失败，不能把零张表记为验证通过。

`audit-runtime-resources` 用 Rust AST 遍历根程序与全部 crate 的生产资源 literal、宏 token、完整函数所有者及生产目录维护输入，核对固定资源目录；动态站点单独列出 caller 有限域与尚未证明的分支，通配命中不能证明覆盖。`--repair` 先依据当前 3884／3886 完整 JS／CSS 原件及固定载荷验证无线 ARGB 交互图层并补 alias，不恢复媒体到源码目录。统一结果保存在 `.analysis/results/resource-audit.json`；文件缺项、资源作用域违规或输入快照变化使命令失败。`resource_scope.py` 检查 overlay 回调、early return、导入别名与显式宏诊断，`organize-sources` 调用同一所有者；它不做 Rust 类型推导或宏展开，间接回调保持显式诊断。生产 overlay 使用共享 `deferred_resource`；`resource_scope` 保留原布局节点与边界，自定义 element 的 `ResourceLayer` 在布局前创建并保留到 prepaint／paint，以同一窗口图片缓存处理弹层内的真实资源请求。静态所有权检查不替代真实窗口显示验收。

`prepare-app-picker --check` 从当前 RzAppMenu 原件核验 34 个分类向量与 331 个产品的首个 favicon fallback，检查固定 alias 字节及原始选择顺序，不依赖已删除的开发媒体目录。目录元数据、两张所属小型 SVG 夹具的纯 CPU 解码、资源作用域结构与真实窗口绘制分别验证，静态通过不代表窗口验收。

`prepare-media-analyzer` 按当前 Python ABI 恢复固定 PyAV 16.0.1 的官方 PyPI wheel，逐项验证发布长度、SHA-256、完整 RECORD 清单和成员字节后写入 `tools/analyzers/media-python/`。获取收据保存 PyPI 原始元数据、wheel、全部提取文件及实际 FFmpeg 库版本；分析器用于静态解码／准备已核验的媒体，不属于产品运行依赖，也不执行厂商代码。

`prepare-resource-database` 生成唯一的 `assets/synapse/resource-catalog.sqlite3`；`prepare-emoji-search` 和 `prepare-runtime-assets` 使用同一实现。四份原始 JS 核验获取收据、实际字节和完整搜索字符串，数据库保存 1,857 条唯一记录与 1,936 项原始顺序。分组值、变体和排除列表分别去重，Mapping、无排除项的 Mouse 分支和 Macro 保留各自的标签、分组顺序、tab 目标及系统版本规则。工具逐字段回读所有表情与完整资源索引，记录 SQLite 实际版本、输入／输出 SHA；`--check` 校验同一数据库。旧重复词表和生成 Rust 资源索引不再保留。新 JS 来源由 `prepare-emoji-search --source PROJECTREL` 重复登记；完整 callable owner、解析器版本和等价性记录位于 `.analysis/results/typed-data.json`。产品不读取分析目录或远程词表。

schema 4 的 `ui_product`、`ui_product_category`、`ui_product_edition`、`ui_navigation`、`ui_page` 保存完整产品目录，包含 331 个产品、365 条导航分支及 1,459 个页面。维护输入为 `.analysis/inputs/typed-tables/razer-catalog/product_registry.json`，生成时检查全字段、顺序、稳定身份、primary／连接分支和 333 个 Help 展示记录选择键并完整回读；`audit-registry-identity --product PID --prepare` 依据当前原始挂载链更新同一维护源与数据库，保留原有导航／页面身份和展示记录选择键。启动在窗口创建前保留一次有类型的目录快照，未使用的源码路径、哈希、组件表达式与分析状态仅保存在 `.analysis/results/registry-source-evidence.json`，不进入运行时目录。迁移等价性、单独的 Help 连接修复和当前源码覆盖分别记录。

同一目录库的 `ui_locale`／`ui_translation` 保存 10 个语种、58,587 条翻译，当前完整数据库为 47,710,208 字节。`prepare-locale-exports --acquisition RECEIPT --source JS [--check]` 核对完整原件、静态字典导出及唯一语言路由，维护 `.analysis/inputs/typed-tables/razer-i18n/locales.json` 和同目录原字节 baseline，并通过统一 SQLite 生成器回读全字段、顺序与重复项。当前导出提取仅向各语种补 6 个缺键，保留全部旧值，94 处已有值差异见[词表准备收据](../.analysis/results/locale-preparation.json)。旧 `locales/*.json` 生产文件与嵌入读取已移除；`prepare-locale-keys` 写入同一 canonical／SQLite 所有者，不重建旧目录。`razer-i18n` 查询保留的不可变 `DictionarySnapshot`，与 GPUI 共用当前语言状态；运行时不读分析输入，旧词表的完整厂商来源仍须继续核实。

`prepare-native-cursors --sources INPUTJSON --group 102` 接受包含 `path` 和 `acquisition` 的 JSON 列表，`--sources -` 从 stdin 读取。它核验原始 PE／获取收据后静态解析 `RT_GROUP_CURSOR` 与 `RT_CURSOR`，校验 RVA、资源边界、尺寸、热点与 DIB／AND 数据，重建完整 CUR；不加载 DLL。原件、解析范围、实际 SHA 和八帧光标位于 `.analysis/inputs/native-resources/acquisition.json`；输出通过统一资源映射与固定版本按需交付。PE 资源恢复不代替原生函数分析或真实取屏色验收。

四份表的维护输入由现有 Rust 构造数据恢复生产字段投影，`tools/typed-data.json` 的 `maintenance_recovery` 保留原文件字节摘要、缺失维护输入的旧摘要和未恢复的证据字段。该恢复不代表找回原始证据 JSON；搜索字符串另与实际原始 JS 全序列比较，其他字段通过迁移前后 Rust token 等价性核对。原件部署稳定性、完整来源闭包和运行验收仍分别判断。

`audit-host-local --query macro-recorder` 复用并核对当前 PID192／756 middleware、Macro 和 background-manager 完整 Wakaru 捕获的全部输入、输出与工具字节，保留原 validate 失败；从原始 JS 的实际 UTF-8 AST 区间保存完整 An／q、软件录制和 OTFM set/remove、全局锁与订阅／清理、接收器布局与功能配置 owners，统一写入 `.analysis/results/host-current/local-audit.json`。PID756 的预触发对象没有 exclusive mode，和 background 随后写入的独占记录分开；初始扫描和实时事件的字段层级差异、JSON 记录没有硬件 getter、运行时缓存最后匹配语义也分别保留。实际原生 transport、产品接线、完整页面与真实验收仍须独立完成。

`audit-family-function --family keyboard --function main --query global-shortcuts` 核对当前 Dashboard 的完整编译键表、默认 Turbo 顺序、事件序列和常量，保留挂载页面／行警告／输入捕获、store／factory、规范 JSON／hash、Macro 发布和宿主适配器的完整所有者。输入捕获的固定协议解码使用 module 46114.Rk 的独立 144 行 typed Rust 表，由普通构造函数提供，逐字段与原顺序比较并核验生成字节，保留重复 Num Lock 和与编译键表不同的 outputFlag；不从编译表推导，不依赖运行时目录库。查询保存 708／707／706 共享的完整 DKMKEYS、DeviceInfo 与 middleware → connectedDevices → validDevices producer，核对三项 scan redirect。宿主 `NoBrowserInputHandler` 保留完整处理函数、sender 注册／注销、状态初始化及 IPC 分支；该门控只跳过宿主快捷键处理，不能据此禁用 DOM 键盘捕获。原生静态范围覆盖存储 singleton、异步 ACK、开关 vtable／初始值和 shutdown 恢复链；ACK 仅表示原生内存发布，不证明 parser 接受、OS 执行或设备写回。结果写入现有 `keyboard-mw-audit.json`，完整页面和真实运行验收另行记录。

`audit-family-function --family keyboard --function mw --query keyboard-local-controls` 从 PID691／740／746 当前 UI 与 middleware 的 HTML、manifest 和部署稳定收据选择原件，保留指示灯／OLED 与磁轴校准的完整模块和 callable。每个 callable 仅登记自身命中的语义角色，结束时重新核验全部获取字节和收据；缺失 source map、资源闭包及获取失败继续独立保留。结果更新 `keyboard-main-audit.json` 的同一 query，不把该范围的静态调用链闭合记为整个键盘页面或运行验收完成。

`audit-family-function --family mouse --function mw --query mouse-local-profile-schema` 默认核对全部已获取的当前鼠标 middleware，保留 schema 常量、完整迁移、序列号选择、存储及各 edition 默认配置导出，检查源声明的逻辑产品与 dongle／BLE 别名。指定 `--product` 时重新验证选中项，未选收据必须再次通过原件／产物身份检查才保留，各项保留自己的工具与采集时间。结果写入 `mouse-mw-audit.json` 的 `local_profile_schemas`；schema12、无 schema、源产品身份不符分别记录，不据此扩大运行时反序列化范围。

`mouse-dpi-profile` 同时将变换输出的 validate finding 按 provenance 回溯完整原始闭包。当前 PID190 的 Sentry CommonJS 与 ReactRedux UMD 参数变换问题均保留原始 factory／完整脚本及绑定；失败的 ESM 输出不作为实现依据。原 Wakaru 非零状态保持不变，未知 finding、缺少原件或无法找到完整作用域继续阻断对应静态结论，不能用原件恢复证明替代应用运行验收。

`audit-family-function --family mouse --function main --query mouse-tracking` 从当前部署入口与实际 manifest 选择 PID170／204／226／45066 的 main、协议和 tracking engine 原件，保存完整 factory、直接命令、传输分支与界面范围的 AST 所有者。命令头解析真实 lexical binding，逗号表达式取最后值；事务编号与重试规则分别核验。`--prepare` 将所需独立协议期望和原件身份写入设备 crate 的小型夹具；完整函数体保存在统一 `mouse-mw-audit.json`。固定来源字节、未解引用、缺失 source map、实现模拟验证和真实设备验收保持分开。

`audit-family-function --family mouse --function main --query mouse-calibration` 核对 PID70 的真实 Philips 引擎构造分支、预设／手动／Mini apply 顺序、12 个数值的字节序、完整 16 项垫面表和自定义高度／阈值公式。手动／Mini 各六个完整状态包含样本方差、Z／Q 分组统计、超时、结束查询、保存转换、重校准日期和命名碰撞链；硬件记录与 parser、宿主注册／清理和原生 worker join 分别保留。默认 UI 身份沿 lexical binding 验证为 `undefined`，初始 sensorType 和 Mini Halt 面板沿 producer 常量验证；host add ACK 与 driver-ready 事件须同时成功。`--prepare` 准备所属设备协议夹具，完整所有者与独立获取边界写入现有 `mouse-mw-audit.json`。原件 lockMouse 是空操作，不代表原生锁鼠标；静态证据及模拟 transport 测试不证明完整页面或真实窗口／设备／服务验收。

`audit-family-function --family system --function main --query shared-preferences` 从 740／746／756、Hue 与 Armory 的 5 套当前获取收据重新选择 HTML／manifest 指定原件，保存完整页面 factory、存储适配器与 read／write 回调；沿实际 key 常量、默认值、缺值初始化、乐观修改和监听清理确认 4 项本地偏好。结果保存在统一 `system-mw-audit.json`；查询不读取临时调查正文、不执行原始 JS，也不把未获取的引用或 source map 标为已完成。

`audit-family-function --family accessories --function main --query hanbo-local-runtime` 在同一 `accessories-main-audit.json` 查询内保留当前完整页面／设备／本地服务所有者及图表呈现审阅：实际 Roboto font-face、旋转轴标题、语言切换时的 cleanup 提交、模式按钮状态优先级与 300ms CSS 过渡、禁用内容透明度和输入边界。字体测试夹具从获取收据核验的 WOFF 原件逐表解压，与 sfnt 的全部 tag、长度、校验和及原始 table 字节比较；仅允许 head checksumAdjustment 重算，另验证 sfnt 整文件校验和。夹具身份与实现字节分别记录，不能证明所有系统 fallback 字体或真实窗口／服务／设备验收；未闭合 producer 继续保留。

原生批处理与指针表补查使用 IDA 9.1 官方 `pdb.cfg` 支持的 `-Opdb:off`，禁止隐式 PDB 下载阻塞。关闭符号下载不代表原 ABI 已解析。相同 SHA 的活动数据库不得重复启动。

dataset、聚合 C／ASM 和文件摘要使用有界内存写入，不先保留全部字符串或反编译文本；流式输出仍保存全部记录。导出前保存私有 IDB checkpoint；`--resume` 在输入、loader 一致时重新打开已有 packed／unpacked 数据库，不用 `-c` 擦除分析。原失败 session 与日志字节保留于新 session 的 previous_attempts。恢复 DB 用 IDA 只读一致性检查，未解决的不一致阻止完整捕获。

`extract-embedded-inputs` 仅保留在原始程序集内唯一匹配且长度前缀一致的 CLR 资源，以及实际 MZ 文件中边界合法的 CAB 候选；DLL 和不带扩展名的载荷同样扫描。CAB 范围合法不等于可完整解压，后续必须交由静态解包器验证。`acquire-family --stage extract-native` 按真实文件头递归 7-Zip 支持的容器和 Inno Setup，安全核对完整列表；Inno 重建的 `install_script.iss` 只存 results，不冒充原件。交替补提取与资源扫描，直至不再新增原件，再重做 corpus 和 managed/native 捕获。

`reconcile-native-extractions` 将解压失败留下的局部文件及其后代标为未验证，保留实际 bytes／SHA 和失败收据；输入分析、递归提取及 managed/native 捕获拒用这些文件作为原始证据。原始官方 manifest 不匹配同样保留，显式调查不能改成验证通过。

`inspect-corpus --no-refresh-gaps` 的 schema 3 从实际当前 manifest、生产发现与资源字节追踪来源，连接完整静态提取、内嵌区间、packed ASAR 及已验证同字节 aliases；本机路径、版本字符串、继承 verified 字段和单独 ASAR unpacked header 不提供当前版本证明。最新 host 元数据失败不回退旧稳定收据，结束时复核输入索引与全部使用的原件／收据快照。来源字节、依赖／ABI、安装生命周期及产品验收分别记录。

`capture-corpus` 复用已完成捕获及新捕获返回成功后，均重新核对原件实际长度／SHA、capture 输入身份、完整函数数量、唯一 RVA／地址、汇编捕获标记、七类 dataset 与 C／ASM 聚合文件清单和全部产物摘要；输出路径不得越过所属 capture 目录。函数反编译失败与完整汇编捕获分别保留。验证中清单或产物变化同样失败，每项失败保留并继续其余输入；批次结束重新核对所有已验证原件快照及最初实际读取的 corpus 收据 SHA，原件或清单漂移阻止全库捕获结论。

重复 host 提取保留真实 extraction exit／argv。旧重复收据丢失退出码时，静态调用 innounp `-t` 或 7-Zip `t`，保存实际退出码、分析器及输出字节摘要；复用前重新核验输入、检查输出、提取文件和完整 Inno 条目，不编造提取成功。官方 native manifest／资源复查均禁缓存并验证官方 HTTPS 最终地址，previous_observations 保留真实历次差异；quarantine 不因重复观测相同字节而提升为 verified。

`acquire-native --include-host-app-resources` 从当前稳定宿主应用的实际 HTML `link rel=manifest`、原件索引与前后稳定摘要收集 native 清单，并与保留设备清单一同获取；当前 manifest 与调用页面部署不匹配时明确失败，不能单靠新清单提升来源。后续复查保持已选宿主范围，不丢其他设备。`extract-native-packages --scope host-apps`／`--scope all` 复用 family 的完整安全静态容器流程，`--workers` 默认 2，保存真实列表、提取、内嵌及 manifest 绑定；不执行安装器。候选 PE、实际归档提取、无归档限制和语义闭合分别记录。

`inspect-corpus` 中的 `cached_download_fragments` 从当前 installer HTML 实际声明的 JS 静态解析完整下载 owner、常量和 `pindex`／Range 绑定，逐份比较源码定义的偏移与完整官方资源实际 bytes。本轮 15 份 104,857 字节缓存均通过区间核对并独立保留，从完整 PE／其他模块分类排除；不重建原件或转移完整文件身份。父资源的当前官方来源另行验证，策略／字节／快照失败及未证明的父来源均由同一 corpus gaps owner 保留。

`organize-sources` 用 Rust AST 解析现有模块，再按功能生成物理目录迁移计划，同时核对 `include!`、嵌入数据和维护工具引用。默认只写计划；`--apply` 在确认源字节和目标路径未变后逐文件搬移、同步路径，仅移除已经为空的原目录，不操作 Git 索引。完整映射和摘要保存在 `.analysis/results/source-organization.json`，重复规划应没有迁移或路径改写。它同时记录全部工作区 manifest、各目标／构建／测试的内部与外部依赖、源码规模、实际嵌入资源和独立安装库，并检查无 GUI 核心的传递依赖、领域／存储契约所有者、资源会话及产品保存边界。快捷键输入／hash、跨设备 held 目标、原子资源发布、映射／宏录制 worker、宏文件传输、共享 UI 偏好和退出排空使用明确所有者守卫，并用非法副本、别名、旧路径及无关代码反例核对。映射控制检查实际 `start_with_registry` 入口、共享 `apply_with` 与公共 helper，以及关联 owner／revision／target 的 typed 回执；global capture 检查混合 input／mode 的同一 poll。设备与全局快捷键会话检查共用 admission 锁、关闭后拒收和 FIFO Finish；快捷键 actor 不得合并跳过编译上下文。产品退出分别检查停止交互与排空已入队 mapping 两阶段，shell 仅在已请求保存全部排空后停止映射；后台等待者覆盖保存回执后才创建的 worker。统一清理 hooks 单次消费并由保留任务等待全部 completion，再关闭资源请求、后台等待资源线程退出，最后请求平台退出。`on_app_quit` 只作统一清理兜底或同步 UI 回收；功能模块内仅依赖该有限等待回调的后台 await／spawn／join 会被拒绝。边界检查与资源作用域检查各自报告失败；静态检查只将 `ensure!` 和 `matches!` 的参数重新解析为普通 Rust 表达式，不执行或通用展开宏，也不完成类型推导，不能替代编译及功能验证。

来源索引由实际获取／发布所有者维护。`reconcile-source-index` 仅离线核验保留收据的完整字节、官方入口与 manifest、原有部署稳定性证明，或已固定资源版本、alias 与发布清单；`--frontend RECEIPT` 和 `--publication MANIFEST` 均可重复指定。通过后发布完整来源集合，在 `inputs.json` 的 `source_publications` 保留原索引行、新身份、原因和验证范围，不把缺失依赖或 source map 改成完成。它不重新下载、上传或直接按当前文件内容刷新摘要。

`audit-reference --retained-snapshot` 仅适用于已有完整 Git tree 审计收据且当前没有可用 Git 元数据的参考快照，核对原清单、全部文件字节、Git blob 身份、模式及 workspace manifests。`REFERENCE` 是参考目录，`COMMIT` 是原固定提交；该模式不重新获取 Git 对象、不检查上游是否更新，保留原审计时间和适用边界，不能与 upstream 复查或恢复原字节选项混用。

`validate-rust` 将精确命令、实际执行测试数、日志摘要和验证前后代码身份统一写入 `.analysis/validation.json`。架构整理可选择上述工作区、领域、存储与资源套件；`assets-resource-runtime` 在无 GPUI 的 `razer-resources` 中使用模拟传输，`assets-resource-identity` 只做元数据和 CPU codec 检查，`assets-fixed-icons` 核对固定图标。该命令只执行纯／模拟测试，不应从普通 `#[test]` 属性推断测试没有窗口副作用。已授权的真实 DLL／设备检查使用下述显式维护命令，窗口、应用、厂商 JavaScript 和安装器仍不执行。

设备读取接线的验证必须注入与本地缓存／默认值不同的设备返回，检查数据实际进入原控件的状态入口，并覆盖局部成功、失败、未知、断开、重连与用户编辑。未返回字段不得补造，失败不得回退缓存或默认值；首次初始化与恢复的顺序使用源产品自己的证据，不增加通用只读或默认写入流程。写入分别核对真实发送、应答、协议要求的读回和页面刷新；文件保存、同一输入的序列化往返及附加诊断不能代替这些验证。实际已执行范围仍以统一收据为准，不能从此验证要求推定全部产品链已通过。

`mapping-service` 通过内存传输验证设备映射／按键捕获／宏录制、取消、恢复、替换和 Rust panic 后的收尾；`mapping-connections` 只验证发现连接的纯状态规则。held 跨设备目标、快捷键编码与宏录制解码／XML 编解码由 `model` 覆盖；宏文件传输、旧快捷键归档及偏好磁盘迁移由 `storage` 覆盖。`profiles-transfer-codec` 验证共享源 hash 的导入边界，`ui-preferences-state` 只运行无窗口的偏好状态／队列测试。`mouse-tracking-*` 和 `mouse-philips-*` 选择协议、会话、IPC、发现路由、服务、页面请求状态及 shell 结果语义的纯／模拟测试，共享 feature submission 的取消／join／panic 由这两类 controller 用例覆盖；`mouse-philips-state` 覆盖 `calibration_70::state` 的面板转移与请求生命周期。`keyboard-latest-restore-*` 分别验证设备协调器、保留的退出队列及模拟 publisher 的映射注册／恢复；`keyboard-game-lifecycle` 验证退出冻结、手动配置与回执寿命；`keyboard-actuation-lifecycle` 用屏障与模拟 transport 验证已接受 FIFO、退出取消后的继任队列、真实范围取消及 Shutdown／释放完成。它们不启动应用、窗口或真实设备。`assets` 套件显式排除会创建 headless GPUI 窗口的 `image_cache::tests`，排除清单与边界随验证收据保存。

`macro-recording-ipc` 核对暂停／恢复回执的完整字段、失败原因与错误类型，`macro-shortcut-resume` 使用模拟原生回调验证普通 source false、独立快捷键恢复及退出必须确认已拥有的映射抑制恢复。原生 producer 和录制 consumer 共用 IPC 类型，普通响应数据与清理失败分别判断；守卫同时检查共享 capture 生命周期和回执所有者。

`session-monitor-lifecycle` 仅调用注入模拟 worker 的会话观察 owner，验证退出前 stop／join、创建前取消、回调上下文释放、错误和 panic；单次停止通知失败后，用显式模拟时间推进验证有界重查、等待失败及清理错误保留，不创建原生窗口或注册实际系统通知。静态守卫要求 shell 先释放事件 pump，经统一退出清理在后台 shutdown；平台 stop 与 Drop 不得等待窗口生命周期锁或 join，消息观察不得恢复为无限 `GetMessageW` 等待。

`device-change-monitor-lifecycle` 通过模拟线程、内存队列和注入注册／注销函数验证 stop／join、失败与 panic、队列已满时停止、部分注册回滚和回调上下文释放；不调用实际系统通知。`settings-runtime-state` 只运行发现状态及模拟连接 worker 测试，显式跳过两项 GPUI 窗口测试；区分退出准备、用户断开和最终应用清理，失败观察与失效连接仍保留后台 join 所有者。`ipc-client` 使用内存流、模拟进程和线程屏障，核对部分启动、进程回收、管道 join、关闭回执、失败保留与再次清理，不启动 helper。`thx-service` 使用模拟 HID／RPC、内存 duplex 和模拟 socket 所有者；`thx-shell-lifecycle` 只验证独立观察与错误的进度合并。THX shell 的线程创建失败归属由 AST 守卫与工作区编译检查覆盖，不能据此宣称执行过真实清理验收。

`audit-lfs` 只读核对 Git 属性、索引中的 LFS 指针和本机对象的实际 SHA-256／长度，同时列出尚未暂存的新增或修改、缺失对象和超过 100 MiB 的普通文件。结果保存在 `.analysis/results/git-lfs-audit.json`，验证前后索引与属性必须不变；不会 stage、commit、push，也不代表远端对象已上传。

`prepare-dashboard-images --source-preview-only` 复用完整的当前 index／manifest 与资源上下文提取，仅重新准备 ProductImage 预览域，避免重复获取全部 Dashboard 图片。仍核对原始字节，保留缺失与提取失败；缺失的已声明依赖沿同一获取流程补取。`--no-refresh-gaps` 仅供拥有最终统一刷新步骤的批次使用。Webpack 数字模块 ID 按 JavaScript 整数字面量静态解码，支持压缩器的 `36e3`、十六／二／八进制形式，拒绝非整数；不执行厂商 JavaScript。

`fetch-frontend` 保存 HTML、manifest、全部已声明资源、静态 chunk／import／CSS 引用、HTTP 失败和版本稳定性检查；原始文件不执行。`analyze-frontend` 显式提交所有获取的 JS，保存完整模块、provenance、输出 maps、诊断及图校验结果，并提取可用的 `sourcesContent`，随后恢复尚未拆出的完整 webpack factory。

已有完整捕获用 `analyze-frontend --input DIR --recover-missing` 核验后补恢复；全量入口是 `analyze-frontend --all-acquired --recover-missing --workers 4`，并行数限 1–8。`frontend.py` 按 AST 定位被前导代码／逗号表达式包住的注册和 bootstrap，从已核验可读整包切出带完整参数绑定的 factory，按 UTF-16 坐标切片 source map 并回溯官方原件。合成包装无原始映射，原始局部名称不作推定。完整整包保留非 factory 代码，每份捕获保存实际恢复工具副本和所有新产物身份；缺少可读整包才静态格式化一次，不执行目标 JavaScript。

批量结果为 `.analysis/results/frontend/module-recovery.json`，每个集合的 `analysis.json` 与 `modules/provenance.json` 保留原件区间、参数、恢复配方及诊断。复用必须核验实际原件和全部登记输出。恢复结构不抹去原分析器失败，也不等于逐功能语义已经闭合。`--refresh` 只重建派生模块、提取源码及旧恢复中间目录，不删除获取原件。

相对引用按实际响应 final_url 与 HTML base 解析，边的 source 仍是请求原件，避免服务器补目录斜杠后产生假 404。收据保存当前 requested_urls，过期请求失败与当前缺失分别报告；原件仍全部核对 bytes／SHA。既有裸路径文件挡住子资源目录时，子资源写入 raw-by-url 命名空间，保留原响应不变。

`acquire-host-apps` 沿当前窗口描述符与 `window.open` 获取官方子应用，保留完整 factory／ESM owner。动态 URL 只静态解析字符串拼接及当前部署内可核验的 webpack 导出；重赋值、运行时位置和其他动态表达式保持未解。实际应用版本／build／gitMetadata 来自各自原始 manifest，不能统一称为 native host 版本。资源缺失分别列出 source map 与运行资源，获取稳定和恢复器成功不代替 mounted tree 或本地功能验收。

`acquire-host --metadata-only` 先核对现存宿主收据中的原始 JSON 身份，再复取官方部署 manifests、生产发现及产品发布元数据；跟随更新的 manifest／prod endpoint，双取核对稳定性。新字节按实际 SHA 存放，独立获取收据为 `.analysis/inputs/host-current/metadata-recheck.json`，比较报告为 `.analysis/results/host-current/metadata-recheck.json`；只索引 inputs 下收据，不覆盖旧 acquisition.json、不下载安装包。版本相同或元数据稳定不能证明完整宿主／设备依赖闭包。`--repair-metadata-index` 仅本地校验误登记 results 收据的现存字节，将其原样复制到 inputs 并修正唯一索引项；保留旧 results 文件，在 inputs.json 的 migrations 中记录原索引行、实际字节身份、工具身份及迁移证明。冲突或字节身份不符时拒绝修复，不重新下载或放宽证据边界。

`acquire-host --revalidate-bootstrap` 独立双取当前官方 Synapse 下载页及其明确声明的稳定安装入口，使用 no-cache、官方 HTTPS 重定向与响应身份校验；只保存惰性归档证据，不执行安装器。结果为 inputs／results 下的 `host-current/bootstrap-recheck.json`，保留全部 HTTP 记录、稳定性、实际旧字节比较及扁平前次观测，不覆盖主获取收据。corpus 仅在两份实际页面及安装器样本均匹配后建立 bootstrap 来源根；此证明不代替静态提取及安装生命周期审计。

`refresh-gaps` 对 bootstrap／metadata 复核分别建立 owner，核对收据索引字节、官方 HTTP 200、两份实际样本及稳定摘要，保留未完成、失败及与原 acquisition 的部署差异；旧主收据不能覆盖新的复核失败。

`acquire-family` 不截断候选产品 ID，UI 与 middleware 分别保存原件及覆盖矩阵；获取识别 asset-manifest 和 middleware 的 webpackManifest。`--stage repair` 保留上次失败记录并重试非 404 下载及部署稳定性失败；加 `--retry-declared-missing` 还会重取实际源码声明的非 map 404，未声明的探测和缺失 source map 分别保留，不扩成猜测下载。`--stage analyze` 仅在原件与全部产物的当前字节吻合时复用结果，最多同时运行两个恢复任务。`--stage native` 根据已验证的官方 middleware manifest 获取全部声明资源，并合并已保留的其他产品端点；只校验文件大小、SHA、integrity 与来源，不运行下载的安装器或库。执行完毕、图验证通过、语义审计和产品实现验收分别记录。

`--family` 支持 mouse、keyboard、audio、gamepad、system、accessories 及 all；`--layer both` 同时处理 UI／MW。`--product` 可重复指定单个产品族中的 ID，其余产品的矩阵行保留。批量任务共用已核验的同 URL 资源，并将完整获取和静态恢复分别限制为三项和两项并发；不会随产品族数量乘倍。候选表仅提供端点 ID，不能作为协议或页面证据。矩阵中的获取运行结束、字节核验、声明资源获取、部署稳定性与实现验收分别计数。

批量前端获取和恢复均仅在最外层收尾时刷新一次 gaps，避免每个产品全量重扫及索引锁竞争；单次 `fetch-frontend` 和 `analyze-frontend` 仍默认刷新。缺项清单刷新失败与已保存的产品获取／分析结果分开报告，不将收尾锁失败改写成获取器／分析器失败，也不掩盖原始异常。中断重启仍必须核验输入及所有输出字节，不能按目录存在跳过。

拥有完整收尾流程的调用方可向 `acquire-family` 传 `--no-refresh-gaps`，串行完成矩阵复核、缺失／过期 capture 补齐和源码查询后再统一 `refresh-gaps`；默认行为不变。矩阵复核从实际获取收据和输入／产物字节重新计算状态，不按旧计数跳过刚补获的产品或依赖。

`--stage report` 将缺失的源码声明 source map、未声明 manifest 探测、实际声明的非 map 文件和未解析 chunk loader 分开计入完整矩阵与 `current_sources.devices.reference_closure`。Wakaru 无法安全规范化的 factory 会在既有 `analysis.json` 中补充完整原始 AST、参数、声明体边界、实际字节 SHA、转换器失败和 provenance 对照；保留原始参数复用及自由变量，不把原件可读等同于转换或语义审计通过。

`shared-page-layout` 查询按 HTML／manifest 可达关系核对原件，保留完整 BodyWidgets、WidgetCol、WidgetContainer、CheckItem AST owner、CSS／media 规则和解析器版本，并复用六族当前完整 capture。缺少完整 capture 时仍从已核验的当前原件提取完整 AST，但保留 capture 失败，不将查询标为完整成功。默认使用该族完整 locator 并集，不限已连接设备；带 `--product` 可定向补查。查询失败、源码恢复器失败和缺失资源分别保留，不能把共享容器核对当作整页实现或窗口验收。

`inventory` 盘点目录；`verify` 核对输入字节、完整前端／原生产物 SHA、函数 RVA／原始 bytes、Markdown 链接、Rust 嵌入文件及已有测试记录对应的代码身份。`refresh-gaps` 从现有获取／分析记录重建缺项关联，保留每条图校验问题，不重跑分析器；获取与分析分别归属，补资源不会覆盖已有分析问题。`validate-rust` 执行纯／模拟 library 测试及 workspace 全目标静态检查。`clean` 仅删除 `.work/`，分析器会话仍活动时拒绝清理。

前端恢复使用固定 Wakaru 1.14.0，IDA 路径为 `E:/IDA`（9.1）或 `D:/ida/IDA Pro 8.3`（8.3）。每份捕获核验实际工具版本、输入和产物字节，保留反编译失败。全量提取需保存所有输入、完整函数体、模块／资源引用和失败项；调用、状态、持久化与清理语义逐链核对。结果存 `.analysis/results/`，缺项集中于同一清单。详见 [工具说明](../docs/re/tooling.md)。

`prepare-resources` 核验获取收据和原始 bytes 后准备实际媒体文件并更新统一资源映射，支持静态 AVIF 转换、受限字面量 React SVG 和已实现的 SMIL 动画采样。请求带 `reuse_locked_alias` 时，核验原件与已有固定资源的长度和 SHA 后只增加 alias，不恢复媒体或改变固定版本；显式 `source_lf_to_prepared_crlf: true` 只接受不含 CR 的原件，并验证可逆换行转换后的全部 bytes。alias 复用批次与媒体生成批次分别提交，来源与转换记录进入既有 `typed-data.json`。`prepare-dashboard-images` 按当前完整 Dashboard 图片调用方，对全部产品 ID 及版本／布局／腕托／鼠标垫请求进行获取和稳定性核验；参数 locator 不提供图片或当前语义。资源失败保持失败，不能生成虚假替代图。全族 scope 合并 registry 与产品候选共 332 个唯一 PID；`--stage report` 从实际获取／probe 收据恢复矩阵字段，分别核对原件与分析产物，不依赖缺失的旧汇总字段。

`python -B tools/workspace.py deduplicate-resources` 只生成 `.analysis/results/resource-deduplication.json`，不移动资源。检查归属、逐字节共享关系、全部生产引用及历史 provenance 差异后，`--apply` 仅接受未变化的完整 dry-run 快照；异常会恢复被改写和删除的文件。单产品图片及配置归入 `products/<PID>/`，跨产品内容显式共享，无可证归属的内容隔离至 `shared/source-unverified/`。迁移保持逻辑 alias 顺序、实际 bytes、各来源记录及历史 `original_paths`；源码未核验与物理去重分别记录。资源准备与迁移共用 OS 文件锁和实际内容索引，后续准备复用已有相同 bytes；准备引入新的跨产品使用后，应再次运行此布局核对。apply 后须明确重新准备资源发布快照、固定提交与运行时索引，复做相关族审计及适用纯测试；该工具不修改 Git 索引、历史或 LFS 配置，也不执行应用或厂商代码。

完整原图的 `verify-resources` 属于显式准备原文件后的维护校验：SVG 解析完整 XML，检查遗留 React 属性和 `<use>` 的内部目标；位图使用固定 Pillow 验证结构并完整解码像素。输入摘要、逐文件结果、失败及验证期间的图片身份保存在 `.analysis/results/resource-audit.json`，失败与过期收据进入统一 gaps。它不属于 Cargo 前置步骤。资源纯测试通过编译索引检查当前 alias 与产品分支，使用必要的内联合成 SVG／PNG 检查锁定 GPUI codec；真实产品图片不放进 `testdata/`，也不在测试中全量加载或下载。AVIF 的维护转换仍须从核验过的原件准备，两类检查均不能代替布局、交互或设备验收。

工具路径相对项目根，证据哈希从原始字节生成。来源获取和分析目标只作静态数据处理；已授权的本地 DLL／导出、服务与设备功能测试须先核实来源、版本、架构、ABI 和清理链，先只读核对，可恢复写入保留原值、恢复及回读结果。应用窗口、厂商 JavaScript、无关 helper、installer、固件刷写和不可逆操作仍不运行。实际调用、返回、比较与未验证范围分别记录；允许的 Rust 检查与纯／模拟测试及完整执行边界见 [AGENTS.md](../AGENTS.md)。

`validate-connected-devices --product PID` 显式执行 owning crate 中默认忽略的真实 HID 集成测试，按当前源码的物理 PID、接口与实际 Feature descriptor 选择唯一 collection，并保留实际查询、原始应答与释放结果。PID679 的逐键读取只在实际布局匹配已核实清单后执行，GET 不使用 ALL_KEYS。普通 `cargo test` 不打开硬件。`--write-restore` 覆盖已核实的键盘亮度、PID679 Continuous Rapid Trigger，以及 PID190 临时 DPI、轮询率和休眠时间：先记录完整原始值，仅在可以逐字节恢复时验证变更，写入失败或 panic 仍执行恢复并独立读回。鼠标检查完整档位表、活动 DPI 的 Z 字段、轮询／休眠原始应答及接收器在线状态，不写板载配置；未知或无法精确恢复的原值禁止 SET。失败或进程超时不算恢复成功。新增鼠标夹具本轮仅作纯／模拟验证，没有执行真实操作；实际执行始终受根目录当前执行限制约束。收据分别为 `connected-device-verification.json` 和 `connected-device-write-verification.json`，均归入现有 `.analysis/results/`。

硬件测试直接运行时同样检查每个读取和恢复确认，恢复失败不能返回测试成功。只读测试超时会回收并等待测试进程树；写入测试超过维护时限后继续等待原值恢复与释放，不强杀可能正在恢复的所有者。原值记录和开始事件在操作时立即刷新到日志，缺失最终结果时保留已尝试操作及未知恢复状态。`connected-verification-boundary` 套件只执行这些结果判定的纯测试，不选择默认忽略的硬件测试。

`compare-connected-hid --library PATH` 复用上述只读实测报文，要求读取收据的完整 Rust 输入快照仍与当前字节一致，并在比较期间保留同一份收据。在独立限时进程中调用字节、架构、导出和 ABI 均已核实的当前 `HID.node` C 接口，不执行 JS／N-API。工具拒绝配置 SET、未知产品／接口、错误长度、校验和、profile 和参数。当前已核实工厂按十次、5 ms IN 节奏等待 busy 后的终态，保存每次原始返回，比较读取收据中同一 OUT 后的终态而非第一份 busy 报文；短返回、busy 耗尽和事务／命令不匹配明确失败。相同路径与事务的完整返回字节进入 `connected-native-comparison.json`；相同离线或错误应答只证明传输结果一致，不能证明缺席设备的设置有效。加载、句柄关闭、`hid_exit`、DLL 释放和进程等待分别保留。

`compare-local-service-version` 核对当前 Simple Service 的完整函数字节、COM 标识、vtable 方法和释放顺序，再核实本机注册的 Elevation 服务架构及 Razer 签名。服务原件的私有副本和获取收据进入输入索引；限时独立测试比较官方 Initialize／GetVersionInfo／Shutdown 回调与 Rust 直接 COM 返回。结果保留于 `.analysis/validation.json` 的 `local_runtime_checks.simple_version`，普通 `validate-rust` 不覆盖该独立收据。该操作只读版本，不证明其它服务操作、设备、窗口或安装已验收。

`keyboard-metadata` 复用当前 middleware 获取收据，逐产品校验原件字节并保留完整注册、DeviceInfo、继承类、getEdition／getSerialNumber、命令／解码器、传输默认值、物理路由工厂及身份缓存／runtimeData 默认值所有者。结果追加至 `keyboard-mw-audit.json`；通过检查的有线、自有无线接收器及 BLE 产品生成 `razer-device` 元数据传输描述，双链路共享接收器另保留 cached-master 工厂及命令命名空间证据。原始依赖闭包缺项独立保留，不把厂商 UI 的布局／版本默认值视为硬件读取成功。该查询不执行厂商代码或设备操作。

应用媒体资源按“私有 HF 仓库 → 当前实际文件请求 → 安装目录 `assets/cache/` → 当前产品界面”流转。`tools/asset-release.json` 固定仓库提交、文件长度／SHA-256 和产品依赖；`prepare-runtime-assets` 解析 `tools/resource-mappings.json`并生成上述 SQLite 目录库，`verify-runtime-assets` 检查相同输出。无需本地图片，也不下载资源；`--check-local` 仅用于明确恢复了原文件的维护工作区。`razer-resource-db::InstallationPaths` 统一定位安装缓存与其 `catalogs/` 子目录；独立目录库在打开 UI 前核验并离线原子发布，缺失／损坏从 EXE 旁安装文件恢复；缺少有效文件明确失败。目录库只读打开、校验 schema 和完整性，领域 owner 保留有类型快照；媒体保留异步去重、并发限制、内容验证及原子缓存，缓存命中不联网。凭据仅从本机配置读取。

`verify-asset-release --remote` 核对已固定远端清单及 LFS 文件身份；`publish-asset-release` 在核验后固定新提交；`pin-asset-release --revision COMMIT` 核验已上传版本。`fetch-asset-release --product PID` 仅为维护人员显式恢复选定产品与共享依赖，普通编译和产品启动均不调用。完整原图核验须明确使用维护来源，不将其复制到 crate 测试夹具。

`publish-asset-release --add-files .analysis/inputs/resource-publication.json --dry-run` 核对选定原件、获取收据和新增 alias，准备具体增量而不联网；去掉 `--dry-run` 后只复制新文件至禁止 LFS 自动下载及换行转换的隔离仓库，保留既有媒体指针，新增路径逐项使用 `git lfs track --filename`，验证固定基线与全部远端身份后发布并固定新提交。清单必须包含每份原件及获取收据的路径、长度和 SHA-256；已有文件或 alias 的不同内容会明确失败。`--prune-maintenance-tables` 仅允许逐项核对固定远端身份后移除已迁移的 `assets/synapse/tables/*.rs` 维护片段，删除清单和实际身份保留在统一 `.analysis/results/asset-release-update.json`。独立复核远端普通 Git blob 时可用 `verify-asset-release --remote --from-dir SOURCE` 提供同一隔离仓库作为 SHA-256 字节依据；不会恢复源码 `assets/` 或下载 LFS 载荷。发布后运行 `prepare-resource-database` 同步目录库中的固定版本和新增 alias。

`prepare-typed-data [--check] [--crate CRATE]` 使用 `tools/typed-data.json` 和 `.analysis/inputs/typed-tables/` 的维护输入，按实际 Rust struct／enum、字段、值域和产品分支生成普通有类型构造函数；界面目录由 `catalog_data.py` 路由到同一 SQLite，并在领域所有者中生成普通读取函数；专用设备写入、读取／身份／接收器、存储默认值分别由 `prepare-device-write-table`、`prepare-local-protocol-tables`、`prepare-storage-tables` 维护。固定协议构造函数随程序编译，界面目录随独立数据库安装，产品不解析这些维护 JSON；外部报文、动态扩展和用户配置只在有类型的边界读写。原 JSON 不在 crates 中保留第二份生产数据。

`prepare-local-protocol-tables --current-reads [--check]` 直接复核当前产品 190、679 的获取收据、部署稳定性、实际输入字节、物理工厂与 GET／解码链，维护普通 Rust 读取能力构造函数并保留其他产品。键盘布局清单沿当前 UI 映射解析固件键 ID，同时核对 Rust 与 Python 只读验证工具的键清单；完整函数、来源和等价结果写入统一 `typed-data.json` 的 `current_local_read_table`。该模式不执行设备或厂商代码，也不恢复已退役的读取 JSON。

完整 `--check --record-verification` 对全部维护源、实际 Rust schema、构造函数／读取函数及独立目录库作等价性核验，开始与结束分别核对输入字节；成功后在统一 `typed-data.json` 收据登记完整身份，并将维护输入纳入 `inputs.json`。该选项不接受产品、crate 或 symbol 子集，也不修改产品输出。构造数据投影仍标明维护用途及未建立厂商原件来源；`refresh-gaps` 重新按实际文件和该收据判断缺失、身份变化与验证过期，并保留尚未恢复的原始证据字段。

目录 schema 的类型移动到独立 crate 时，recipe 可用 `reader_scoped_types` 显式保留原有读取函数的词法类型和子解码作用域；工具核对实际工作区导入、唯一声明及该 schema 中的真实使用。既有外部类型继续使用自身作用域，不能因增加 `use` 就改变未迁移目录的 codec 或字节。

目录读取函数发布保留准备时实际读取的所有者字节，整批写入前核验全部输出基线；并行编辑、新增模块或删除文件会使该批失败，避免旧文件快照覆盖其他任务的修改。检查模式仅比较，不发布生成文件。

`prepare-mapping-capabilities [--check]` 校验当前挂载的鼠标入口、manifest 和完整 DeviceInfo factory，从 AST 导出静态固件映射能力，生成 `razer-model` 的普通有类型构造函数。缺失字段保持缺失，稀疏数组与重复属性保留原始语义证据；源投影与生成 Rust AST 全字段回读逐项比对。`prepare-mapping-producers [--check]` 维护当前鼠标按键状态生产函数及依赖元数据；两类命令均不执行厂商 JavaScript，生成或静态等价校验不代表真实设备写入通过。

`prepare-windows-installer` 从已存在的 GUI EXE 和显式 DLL 准备严格载荷，`verify-windows-installer --stage DIRECTORY` 复核实际 SHA、PE 架构、普通／延迟导入闭包和图标；`compile-windows-installer` 仅调用已有 ISCC 生成安装包。当前缺 release EXE 与 ISCC，没有生成或运行安装包。输入、缓存、升级与卸载语义见 [Windows 安装交付](../docs/windows-installation.md)。

独立 `asset-pack-core` suite 只验证容器 wire fixture 与错误处理。保留的 `audit-asset-pack`／`prepare-asset-pack`／`verify-asset-pack` 属于显式本地容器维护，需要完整原文件与相符清单；生成的 `target/asset-pack/synapse.pack` 不属于当前应用运行或安装依赖。旧全量 `SynapseAssets.list/load` pack 回环已移除；当前资源纯测试验证索引闭包、产品选择、缓存和模拟请求，只用必要的内联合成 codec 样本，不下载全套图片。格式和边界见 [资源交付与容器格式](../docs/asset-pack-format.md)。

`python tools/workspace.py organize-resources --apply` 将来源清单归入 `manifests/`，索引归入 `indexes/`，同步物理路径和文档引用；资源 alias 及全部图片选择条件只由 `tools/resource-mappings.json` 维护，发现旧 Rust 资源片段则拒绝静默搬回生产路径；产品配置保留在 `products/<PID>/configs/`。目录索引只保留实际输出：被更新的活跃 alias 绑定到当前已核验文件，已无生产引用的悬空记录从当前索引移除，其完整旧记录与判定保留于同一 `resource-deduplication.json`。逻辑 alias 不改名。
