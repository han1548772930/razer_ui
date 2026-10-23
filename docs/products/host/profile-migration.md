# 本地配置迁移

配置迁移读取本机已有归档，将选中的配置档、宏、灯光工作室效果和本地游戏关联转换到目标存储。扫描到文件、转换成功、写入本地文档以及设备实际采用新配置分别是不同状态；迁移窗口没有把整个导入过程包装成一次设备写入。

## 当前入口和状态所有者

入口来自[全局设置](settings.md)的迁移按钮。Synapse 与 Chroma 分别使用自己的窗口身份；已打开时激活现有窗口。实际页面是 [main.512f18b6.js](../../../.analysis/inputs/frontend/profile-migration-current/raw/apps.razer.com/profile-migration/static/js/main.512f18b6.js)，来源、获取版本和字节身份由[获取收据](../../../.analysis/inputs/frontend/profile-migration-current/acquisition.json)维护。以下区间均为该原始文件的 UTF-8 字节区间。

收据在 2026-10-21 重新获取核对入口、asset-manifest 和 manifest，三个端点在获取前后均保持同字节，主脚本仍为 `main.512f18b6.js`。recorded 时间属于此次核对，文件 acquired 时间仍是首次取得日期；`declared_references_fetched=false` 和动态分块／map 未决项仍存在，不能把稳定性核对当作全部输入闭包已完成。

[共享窗口交互专项](../../../.analysis/results/frontend/synapse-update-fw-current/page-interactions.json)补齐另一条实际入口：固件工具栏以空 props 挂载迁移组件，生成 `/profile-migration/?app=undefined`。目标保留这个非空字符串，仅 BodyWrapper 的窗口名映射回到 synapse，迁移主体仍把字符串传给 toolbar；这与设置按钮明确传应用身份的路径分别记录。policy=3 已追到宿主实际 Tab.init，tabClosed 也已找到返回固件迁移指示器的消费者；它们不确认迁移提交、原生工作取消或资源释放。

目标 BodyWrapper 的 focus／changeActiveTab／activeTabChanged listener 有配对 disposer，但异步焦点查询无取消／代次保护，accessibility MutationObserver 未保留 disconnect。主体 init／scan 的 effect 未返回原生取消／释放链；语言与设备监听的配对移除不能替代它。本文后续迁移队列与文件／设备消费者仍分别核对，不从窗口生命周期推定数据迁移完成。

根组件先更新真实 `validDevices`，注册状态、进度、历史及警告回调，再依次执行 `init → scanData → getBareScannedData`。页面标题为 `PROFILE MIGRATION`，外层窗口焦点组件负责 body 的显示。语言变化会重新触发初始化 effect，不能假定初始化只发生一次。

| 所有者 | 字节区间 | 责任 |
| --- | --- | --- |
| 原件读取器 `QI` | `[2808490,2824208)` | 原生导出声明、字符串转换、文件读取入口 |
| 本地归档包装器 `ug` | `[2827890,2833032)` | 枚举归档、简表、完整 XML 和嵌套 feature |
| 迁移管理器 `Eg` | `[2833046,2845649)` | 扫描、产品转换、依赖关联、提交选中项 |
| 导入队列 `zI` | `[2790174,2808439)` | 新身份分配、写入顺序、本地文档、历史和游戏关联 |
| 进度弹层 `fg` | `[2845698,2847163)` | scanning/preMigrating/migrating/success、关闭和警告 |
| 迁移说明与按钮 `mg` | `[2847170,2848259)` | 本地目录入口、迁移按钮与选择状态 |

说明块使用原版深色圆角面板、绿色标题和 `MIGRATE_PROFILES` 按钮。未选择项时按钮呈现 disabled 类；目录链接在有选中项时显示，打开 `C:\ProgramData\Razer\Razer Synapse3 Data`。这只是打开本地文件夹，不触发扫描以外的写入。设备、宏和工作室列表的勾选用 `type + id + owner + guid` 关联原记录；同一 GUID 来自不同归档时仍需保留 owner，不能按名称或列表位置合并。

## 实际挂载的页面、弹层与 Rust 现状

原版的 mounted 根 `OD` 不只是说明页：它实际执行 `init → scanData → getBareScannedData`，再把扫描结果、ready 设备、已安装模块、迁移历史和待选项传给 `wg`。这与本项目当前 [MigrationPage](../../../crates/razer-app-pages/src/profiles/profile_migration.rs) 的生产状态不同。Shell、产品设置与 Chroma 独立窗口调用的 `new/new_for` 都将 scenario 固定为 unknown、groups 设为空；只有独立 `new_preview/choose` 生成样例设备／宏／工作室和示例历史。本轮未找到生产入口向页面提供扫描结果或迁移管理器回调，不能把预览状态列为可用迁移功能。

| 实际页面／状态 | 当前官方 JS／CSS 行为 | 当前 Rust 路径与差异 |
| --- | --- | --- |
| 扫描中与无归档 | `Rt` 显示 scanning 文案和 loader；扫描后无可见组则挂载 `Rg` 的无归档说明及四步指引，语言 unknown 时 `wg` 返回 null | 生产入口没有开始扫描，也不会把 unknown 转为 empty；预览可显示 scanning 和 empty，但这两者尚无真实扫描状态来源，不能由初始空 groups 推断确实没有归档 |
| 连接／未连接设备和模块 | 非空 device items 匹配 ready productId，已安装宏／工作室模块进入连接组；新增连接组自动选中。未连接设备必须能从本地 connectedDeviceInfo 找到产品元数据；该分支排除 PID770，两组显示还排除 ID179 | 页面已有连接／未连接分区及折叠交互、勾选和卡片布局，但卡片来源只在 `choose` 预览构造；真实产品、归档 owner、连接切换与模块安装条件尚未输入 |
| 卡片、部分选中与依赖提示 | `ct/yg/Xe` 用 guid+owner 保留项身份，父复选框有 partial 状态；名称溢出才显示 tooltip。设备项 warning、历史 linkedGameFail、unusedMacros 各有优先级；模块的 unused 样式并未统一禁用勾选 | 已有示例的逐组／逐项选择、展开及 tooltip，不能证明真实 warning／历史／依赖列表已消费。原版模块有历史时显示 Date.now()，设备显示实际 migratedDate，不能用一套通用日期替代两种消费者 |
| 目录链接与迁移按钮 | `mg` 仅有选中项时显示本地目录链接，点击实际打开备份目录；MIGRATE 取真实选中项的 warning，再执行 startMigrateItems | Rust 目录按钮仅写入“界面预览”notice，没有打开目录；MIGRATE 只展示 Preparing 弹层，没有读取、转换、导入队列、持久化或设备消费者 |
| preMigrating／migrating | `fg` 设置窗口 manual-close；`Ot` 绑定实际进度，只有 preMigrating 的 Cancel 可用，其关闭调用 cancelMigration。migrating 的 Cancel disabled；进度到 100 才解除 manual-close | Preparing 的进度固定 0，Migrating 固定 42%；取消只关闭预览。Dialog 的 on_close 对任意 phase 都关闭，所以 Esc 在示例 Migrating 也生效；没有真实后台任务可据此宣称已取消或清理 |
| success、warning 和失败明细 | `wt` 分别消费 linkedGameWarning／unusedMacroWarning；Continue 有失败组时先显示 `Lt`，其列表来自 failToMigrateData；OK 后回 IDLE | 当前 flags、失败设备和宏清单均为样例；Continue/OK 有本地状态切换，没有真实迁移结果、部分落盘、关联超时或失败条目收据 |
| 关闭与焦点 | 原版外层窗口焦点所有者控制 body 显示，manual-close 的设置／移除是宿主窗口契约，弹层没有自行注册通用 Esc 取消链 | Rust 保留 overlay_focus 和 prior_focus，开启聚焦、关闭恢复；锁定 GPUI Kit 的 Dialog 默认 Cancel 允许 on_close，因此上述 Esc 行为可静态确认。焦点恢复不等于已经复现宿主 manual-close、迁移取消和文件队列释放 |

原版 CSS 还明确：卡片 wrapper 最大高 440、宽 290，图片 250×140、内部项目滚动区最大高 160；分组最大宽 1240，分组折叠使用 0.3 秒 max-height 过渡；进度 popup 为 #111、1px 绿色边框，失败改 #fd4949。scanning popup 宽 300，其余宽 400。Rust 已有部分对应尺寸，但 popup 使用主题色、额外边框／滚动边界与预览说明；未运行窗口和视觉测试，完整正常、hover、partial、warning、禁用、加载、空和错误分支均不得登记为像素还原已通过。

上述 mounted owners 的半开字节区间为：`Xe [2177995,2180405)`、设备卡 `ct [2186154,2189280)`、`Ot [2190016,2190556)`、`Rt [2190557,2190829)`、`wt [2190882,2191704)`、`Lt [2191705,2192519)`、`pg [2848265,2848967)`、模块 `yg [2859158,2861354)`、空态 `Rg [2862184,2863126)`、列表 `wg [2863141,2866422)`、根 `OD [2918933,2920130)`；视觉来自同次获取的 [main.e18d10bc.css](../../../.analysis/inputs/frontend/profile-migration-current/raw/apps.razer.com/profile-migration/static/css/main.e18d10bc.css)。本轮用静态语法树读取完整函数，未执行厂商 JavaScript。

## 本机归档的真实读取

当前原件读取器来自 [Sy3DataMigration_v1.0.13.0.dll](../../../.analysis/inputs/host-current/blobs/79a960e86bde7ea3082ef66a26b0f88701dad3664e5e67e86c889b1fef6193a6/Sy3DataMigration_v1.0.13.0.dll) 的[完整静态捕获](../../../.analysis/results/native/79a960e86bde7ea3082ef66a26b0f88701dad3664e5e67e86c889b1fef6193a6/capture.json)。这是本项目独立 Rust 文件读取器的语义依据；产品不能加载该业务 DLL。

前端从本地 `installedResources` 的 Common/Synapse 项找到 `Sy3DataMigrationDLL`，再定位归档读取器。此路径解析仅说明原版调用方，不能作为本项目运行时依赖。`Init` RVA `0xe0d0` 只记录初始化并返回 1，不能证明目录存在或归档有效。

`GetSynapse3Accounts` RVA `0xf1f0` 使用 `SHGetFolderPathW(CSIDL_COMMON_APPDATA)`，追加 `Razer\Razer Synapse3 Data`。目录筛选函数 `0xe0f0` 只收集匹配 `\d{4}-\d{2}-\d{2}-\d{2}-\d{2}-\d{2}` 的时间戳子目录，再经 `0x1c310 → 0xe5e0` 按目录名升序排列。`0xe8f0` 逐个时间戳目录收集以 `RZR_` 开头的目录，同一归档身份由后来的时间戳覆盖，形成身份到最新归档目录的映射。这里的身份是离线归档命名空间，不需要登录。

通用列表 `0xf9b0` 与文件读取 `0x10700` 使用已建立的映射，拼接：

`<CommonAppData>\Razer\Razer Synapse3 Data\<时间戳>\<归档身份>\Emily3\<分类>\<文件名>`。

| 分类 | 当前读取内容 |
| --- | --- |
| AppSettings | 本机应用设置 XML |
| Macros | 宏文件、Guid/Name 与事件内容 |
| LinkedGames | 本地游戏及设备配置关联 |
| LightingPacks | 工作室 LightingEffects、设备布局和层 |
| Devices | DeviceInfo、配置档文件、Features 和子目录 feature |
| 其他已声明分类 | Data、GlobalShortcuts、Overclock、Presets 和本机游戏列表；按实际消费者使用，不能从导出存在推断全部已迁移 |

设备路径助手 RVA `0x14110` 按 `-` 分割设备身份：普通 PID 使用 `Devices\PID`；带设备序列号的身份使用 `Devices\PID\SN`，随后追加 Profiles 或 Features 等实际分类。feature 列表导出 `0x14bf0` 经该助手追加 `Features\<profile-guid>`。不能仅保留数字 PID 而丢失同型号不同设备的归档分支。

边界字符串由 `0xda70` 使用 `MultiByteToWideChar(CP_UTF8)` 转换为 Windows 宽路径；列表由 `0xdb60` 转回 UTF-8，以逗号分隔返回。实际内容由文件流读取到 malloc 分配的 NUL 结尾字符串；`FreeMalloc` RVA `0xdf70` 对非空指针执行 free。未知归档身份、系统目录失败和异常可以返回空指针；打开文件失败的部分分支会返回已分配的空串。它们均不能解释成一份有效的空配置，也不能据此回填默认值。

## 扫描、转换和依赖

`scanData` 先发布 scanning，清空扫描/转换/游戏关联表，等待 1000 ms，再并行读取各归档。简表读取设备身份和 `DeviceInfo.xml`；配置名和 GUID 来自实际 Profile XML。宏通过 Guid/Name 提取简表，超过 500000 字符的文件额外标记 `isLarge`。工作室简表读取 `LightingEffects > Guid/Name`。没有文件、XML 无效、缺少必需节点和不支持的转换分别处理，不能由简表名称推断完整内容已读到。

完整设备读取先枚举该配置的 feature 文件，再读各文件 XML；嵌套 feature 默认子目录为 `"1"`。XML parser 使用 `ignoreAttrs:true, explicitArray:false`，因此单项和数组需按各转换器的实际规则处理。`EqualizerV3` 的嵌套分支将嵌套 `StandardEQs` 转作 `SystemEQs`，并保留外层原有 StandardEQs。顶层 feature、嵌套 feature 和读取失败各有自己的有效性条件；不能把缺失 feature 补成零或默认设置。

设备转换按通用转换表、共享功能转换表与产品转换表共同选择，使用实际 metadata、产品 ID、Type 和宏依赖。产品身份规范化只用于定位转换规则，owner 和原 GUID 仍保留。未找到转换器或转换异常会影响项目的 warning 和可迁移结果，不能因为当前连接设备同类就套用另一型号。

宏转换保留原事件和延迟语义，并追踪其被哪些设备配置使用。配置中的宏、工作室效果、Specific Profile 和跨设备 Specific Profile 关联需要先建立旧 GUID 到新 GUID 的对应表。游戏关联按设备 PID 或 PID-SN、配置 GUID 筛选；未迁移的依赖不能被静默当作已导入。

工作室转换 `[2834517,2838116)` 读取真实 DevPositions、父设备、序列号、edition 和 layout。坐标按原版缩放到 40 倍，保留旋转、父子设备关系和当前设备匹配。层区分 group 与 effect，audiometer 转为 audio、colorwheel 转为 wheel；单个 region/cell 与数组分别处理，保留选中 LED 的 row/col、可见性、分组和顺序，最后反转层列表。设备身份和选区都参与转换，不能只导入效果名称和颜色。

## 提交顺序和本地写入

点击迁移从当前选中项中收集 warning 项供结束页显示。管理器按 owner、type、id 和 GUID 找回完整转换对象，收集实际游戏关联，然后把队列副本交给导入所有者。导入前为全部选中对象分配新 GUID；名称以 `<原名> - S3` 为基础做本地去重，不能改写原归档。

队列将宏放到数组末尾，再从末尾 pop，因此宏先导入。其余设备/模块同样按队列顺序处理，每组完成后间隔 150 ms；最后执行游戏关联，再发布进度 100、历史和 doneMigrating。组进度按队列长度计算，不是文件字节进度或设备传输比例。

| 写入对象 | 实际目标与确认边界 |
| --- | --- |
| 宏 | 当前宏集合、编辑器配置/文件夹及运行时宏记录；按当前显示器和动作分支转换事件、计算 hash、维护新 GUID/name 对照和延迟策略 |
| 设备配置 | 必须存在目标 `synapse_<PID>` 文档；版本递增、追加新配置，修正宏/效果/配置跳转依赖，保留产品特有 keymap、side panel 和默认分组语义 |
| Stream Mixer 预设 | 导入非默认预设，重新分配 GUID 与名称 |
| 工作室 | 当前本地工作室数据库、用户 profile 索引和迁移历史；重映射效果 GUID/title 与设备关联 |
| 本地游戏关联 | 先按 GUID 或规范化 exe 查现有游戏；需要新增时调用本机 `gms-api-request`，等待实际事件，5000 ms 未确认则发警告；保留已有设备关联 |
| 迁移历史 | `syn3MigratedItem` 下当前目标本地命名空间；保留每轮历史，页面取最新一轮，宏依赖可复用最近持久化映射 |

设备导入函数 `[2801650,2806071)` 写的是配置文档，不能标为设备写回已确认。导入对象写盘前会去掉仅用于迁移的 owner、warning、usedMacro 和 linkedGame 等字段；缺失宏依赖在各产品分支中产生警告、删除引用或使用已有默认映射规则。不能自行添加一套统一回滚或把某一种产品分支推广到所有键位。

部分 import catch 只记录错误并继续队列；doneMigrating 把管理器状态改为 success，并不证明每条写入、游戏关联和依赖都成功。结束页先显示本地游戏/未使用宏警告；有转换 warning 的项目可继续打开失败清单。历史、warning、真实存储结果与设备实测状态必须分别保留。

## 宏迁移的提交与实际消费者

`importMacro` 的目标不是浏览器旧 `synapseMacros` 字符串，而是与当前宏页面共用的 IndexedDB 数据库 `macros`、同名 object store 和 `macros-change-channel`。读取使用 `macros-idb` shared navigator lock，写入使用 exclusive lock；set 在 readwrite 事务内读取旧列表、clear、按 GUID 逐项 put 并 commit，等待事务完成才发 `{oldValue,newValue}`。这确认单次 IDB 列表事务，不涵盖后面的编辑配置、迁移历史和原生发布。

完整宏提交按下列次序执行：

1. 读取现有 IDB 列表；依据本轮缓存的新 GUID 映射修改导入项名称和身份，名称以 ` - S3` 结尾，建立文件夹索引项及 `macroMigrateInfo`。名字去重查的是追加前的现有列表，不能假定同批每个输入名称都已经互相去重。
2. 把导入项追加到现有列表，读取真实 monitor info 和 virtual screen rect，设置动作转换库上下文。只有屏幕列表含 primary monitor 时才返回投影；异常或缺 primary 导致本层失败，并非允许用空屏幕生成一份成功宏。
3. 对合并后的整个列表重建 appEngine，既处理导入项也处理已有宏。移动模式优先读取 actionBar 的 mmtSetting，再接受已有 appEngine 的数值／字符串模式；动作经实际转换库逐项转为运行事件并 flatten，计算包含 mouseMoveType 和 events 的 hash。所有动作类型、分阶段宏和已有数据保留仍须按转换器逐支测试，不能只验证导入项名称。
4. 等待 IDB set 完成，再读取并解析 `macroEditorConfig`，追加索引，必要时设置首个 currentProfile，随后写回编辑配置。本方法没有为配置缺失／profiles 缺失提供有效默认对象；IDB 完成之后的配置错误会产生部分提交。
5. 等待 `syn3MigratedItem` 迁移历史保存，再把运行投影发给映射引擎的 `synapseMacros`。运行投影删除编辑字段和 macroEvents，按原规则插入 virtual delay：连续动作计数达到 5 时可补 3 ms，尾部不足时补 20 ms；随后重算运行 hash。

编辑配置和历史使用 `RzLockableStorage`：先读当前字符串，再在 `STORAGE_LOCK_<key>` exclusive navigator lock 内核对旧值仍相同，才执行 localStorage.setItem。默认超时 5000 ms，冲突值、受限字符串和等待锁取消会拒绝调用。该 helper 的锁租约与 IDB 事务不同；当前宏编辑器持有 shared 编辑锁时，迁移可能在 IDB 已提交后等待或失败，不能将两次写入当成一笔事务。

最后的前端 `localStorageSetItem` 等待主机调用，并把 `{result,reason}` 取成 result；`importMacro` 仅 await，没有检查返回 false。主机实际 ACK 的完整静态链见 [宏原生发布边界](../shared/macros.md)：它只确认原生内存 map 阶段，不确认动作消费者解析、设备绑定或输入执行。迁移历史先于这个调用保存，所以存在历史已写入而原生发布失败的路径。

已打开的宏页面通过 `macros-change-channel → IDB change → h.vh` 消费新列表。remoteUpdate reducer 把收到的宏标记为 saved，替换 macroList 并深复制到 savedMacros；相同 GUID 的撤销历史和 deviceList 从旧对象带回，列表／文件夹索引重新协调。它不是只往现有 Draft 后追加若干行，也不是设备写回事件。后到的 macroEditorConfig storage 事件另外进入 editor 分支；两次事件之间可能存在中间状态。

这个列表事件还使宏 reducer 再次调用 `qs` 发布运行数据，该调用仍不等待原生结果。因而“迁移器直接发布”和“已打开宏页收到 IDB 后再发布”是两个真实 producer，不能在测试中假定只调用一次。宏页卸载会成对移除该 change 和 storage 监听；IDB 模块本身没有频道 close。迁移队列没有等待这些其他窗口的消费完成。

`importMacro` 的 catch 只记录日志，没有回滚已提交的 IDB、编辑配置、历史，或清除先前建立的宏 GUID 映射，也没有向 handleQueue 返回结构化失败。队列仍可进入其他设备导入并引用该映射，最后发布 doneMigrating。恢复评估必须逐项读取实际存储与运行结果，不能根据 success 页面或迁移历史补成“宏已完整可用”。本轮仅记录这一源码行为，不增加原件没有的产品回滚流程。

| 需要分开验证的边界 | 独立期望与对照范围 |
| --- | --- |
| 转换前失败 | 无 primary monitor、显示器查询失败、动作转换失败；记录 IDB 前后的实际差异，确认没有伪造设备状态 |
| IDB 提交 | 重复 GUID、已有宏、同批重名、写事务失败／取消与 channel 广播内容；仅事务完成才能标记 IDB 已提交 |
| 后续存储失败 | macroEditorConfig 缺失、结构无效、并发旧值冲突和锁等待超时；保留 IDB 已提交但配置／历史未提交的真实差异 |
| 原生发布失败 | false、异常、停止期间迟到回调；比对实际 DLL 的输入／map 结果／消费者状态，不能用历史存在代替 ACK |
| 跨窗口消费 | 列表事件、后到配置事件、当前 Draft、同 GUID 的历史／绑定和重复 producer；保留事件次序并核对真实结果 |
| 取消与恢复 | 只在 2000 ms preMigrating 阶段可取消；开始执行后不得宣称撤销磁盘提交、停止原生消费者或自动恢复旧宏 |

该表是已定位的自动化／实际 DLL 对照要求，本轮没有执行这些运行用例；纯逻辑、模拟传输和真实 DLL／设备结果分别记账。

## 取消和清理

进入 preMigrating 后有 2000 ms 的准备窗口。此时临时覆盖 `cancelQueue`，取消会拒绝准备 promise 并清空队列；开始 handleQueue 后该函数被改为空函数。原版 Cancel 按钮仅在 preMigrating 可用，migrating 时禁用；这不能描述成可中断进行中的文件 I/O、撤销已导入文档或设备恢复。

进度层在 scanning、preMigrating、migrating 和 success 时显示；scanning 宽度为 300，其他为 400。preMigrating/migrating 设置窗口手动关闭约束，进度达到 100 时移除。关闭结束页重置 idle 和本地弹层状态，不能据此声称队列有事务回滚。

管理器 `init` 注册匿名 storage 回调，目标命名空间变化时更新归属或调用取消；当前注册链没有对应的同身份注销。根 effect 因语言变化可再次注册队列回调。独立实现需要由窗口生命周期所有者持有订阅、任务和目标代次，确保切换或关闭后旧扫描/导入结果不发布到新目标，同时保留源码实际允许的取消阶段。

## 验证边界

宏迁移补充范围重新校验了前端原件实际 bytes／SHA-256，并用静态 AST 核对完整所有者：`importMacro` `[2796978,2799433)`、队列 `[2794993,2796478)`、历史 `[2796478,2796816)`、显示器输入 `[2789647,2789768)`、YI/WI `[2790062,2790173)`、IDB 所有者 `[2444295,2445496)` 及其前后的 open／channel 绑定。存储锁所有者位于原件 `[2194703,2197162)`；前端原生结果投影 `[1962055,1962290)`。宏页的完整监听所有者 `[552591,556818)` 和 remoteUpdate reducer `[153738,155603)` 均来自 [当前宏原件](../../../.analysis/inputs/frontend/synapse-macro-current/raw/apps.razer.com/synapse/macro/static/js/main.f7cc2d30.js)，后续 `qs`／原生 map 链按共享宏文档定位。

本文核对当前完整前端与本地归档 DLL 的静态捕获，描述的是本机行为及已核实的失败/取消语义。静态捕获所有函数不代表每个产品转换器、原生导出与存储分支已经完成 Rust 独立实现；迁移与设备配置页的后续读取/写回仍分别验收。未运行迁移窗口、厂商 JavaScript、DLL、本机服务或真实设备，实际导入、存储冲突与清理保持未验证。
