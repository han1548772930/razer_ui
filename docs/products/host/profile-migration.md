# 本地配置迁移

配置迁移读取本机已有归档，将选中的配置档、宏、灯光工作室效果和本地游戏关联转换到目标存储。扫描到文件、转换成功、写入本地文档以及设备实际采用新配置分别是不同状态；迁移窗口没有把整个导入过程包装成一次设备写入。

## 当前入口和状态所有者

入口来自[全局设置](settings.md)的迁移按钮。Synapse 与 Chroma 分别使用自己的窗口身份；已打开时激活现有窗口。实际页面是 [main.512f18b6.js](../../../.analysis/inputs/frontend/profile-migration-current/raw/apps.razer.com/profile-migration/static/js/main.512f18b6.js)，来源、获取版本和字节身份由[获取收据](../../../.analysis/inputs/frontend/profile-migration-current/acquisition.json)维护。以下区间均为该原始文件的 UTF-8 字节区间。

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

## 取消和清理

进入 preMigrating 后有 2000 ms 的准备窗口。此时临时覆盖 `cancelQueue`，取消会拒绝准备 promise 并清空队列；开始 handleQueue 后该函数被改为空函数。原版 Cancel 按钮仅在 preMigrating 可用，migrating 时禁用；这不能描述成可中断进行中的文件 I/O、撤销已导入文档或设备恢复。

进度层在 scanning、preMigrating、migrating 和 success 时显示；scanning 宽度为 300，其他为 400。preMigrating/migrating 设置窗口手动关闭约束，进度达到 100 时移除。关闭结束页重置 idle 和本地弹层状态，不能据此声称队列有事务回滚。

管理器 `init` 注册匿名 storage 回调，目标命名空间变化时更新归属或调用取消；当前注册链没有对应的同身份注销。根 effect 因语言变化可再次注册队列回调。独立实现需要由窗口生命周期所有者持有订阅、任务和目标代次，确保切换或关闭后旧扫描/导入结果不发布到新目标，同时保留源码实际允许的取消阶段。

## 验证边界

本文核对当前完整前端与本地归档 DLL 的静态捕获，描述的是本机行为及已核实的失败/取消语义。静态捕获所有函数不代表每个产品转换器、原生导出与存储分支已经完成 Rust 独立实现；迁移与设备配置页的后续读取/写回仍分别验收。未运行迁移窗口、厂商 JavaScript、DLL、本机服务或真实设备，实际导入、存储冲突与清理保持未验证。
