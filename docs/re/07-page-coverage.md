# 已取得源码的页面入口与附属界面覆盖

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](20-current-source-version.md)为准。

更新日期：2026-10-02。范围来自本地原版根路由、实际 render、父组件传参和 CSS；旧截图不参与比较。这里统计 **21 个主应用及产品普通页面实例**：主应用 4 个、182 鼠标 4 个、653 键盘 2 个、777 耳机 4 个、七款鼠标垫各 1 个灯光页。同名 Lighting/Power 按产品分别统计，因为父组件、字段和控件不同。帮助、独立配对、Settings、Introduction Tour、Alexa 的四页与安装器、更多应用弹层另列，不能用一个 Tab 数量代表完整 UI。

当前 Rust 产品入口包括原三款及 3072/3073/3074/3076/3077/3078/3080；新增七款的效果、默认值、帮助与资源差异见[鼠标垫规格](../screens/17-mouse-mat-lighting.md)，这些仍不是原版全部产品。目前沿官方目录和明确声明核查 596 个 ID，已取得 331 个产品资源清单所列的 JS/CSS，331 个入口已解析出实际导航数组；下载不计作界面实现。来源与统计边界见[最新产品目录](16-product-catalog.md)。原宿主另有独立子应用。

## 1. 根路由和当前页面

主应用原始文件是 [App.72827d47.chunk.js](../../.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js)。HomePage 的 nav 数组使用 `Rav/BSg/iwS/fUK`，render 按同一 `active_view` 进入四个 lazy 模块。产品根分别在 [182 main](../../.ref/devices/182/static/js/main.db20a7c4.js)、[653 main](../../.ref/devices/653/static/js/main.7b71cce5.js)、[777 main](../../.ref/devices/777/static/js/main.eb70ce38.js)。Rust 正常入口以 [nav.rs](../../src/nav.rs)、[shell.rs](../../src/shell.rs)、[workspace.rs](../../src/features/workspace.rs) 为准。

| 页面实例 | 原版实际入口 | 当前 Rust 画面与交互 | 仍有差异 |
|---|---|---|---|
| 主应用 Dashboard | HomePage `Rav` → 模块 73435 / chunk 4130 | 290px 产品卡、250×140 图区、整卡打开设备、设备分组折叠；本地快照与预览标记；41 个产品/配色/布局图映射 | 服务推荐、安装过程、分组拖动排序未接入；原片段现已内嵌，播放与内存仍待运行验收 |
| 主应用 Gamer Room | HomePage `BSg` → 19388 / 9388；`He.render` → 条件 `o` banner + `oe` 设备组 | 531px 营销横幅、原热点与商品详情、186×176 添加卡、两步教程；添加设备的准备/QR/搜索/返回步骤及独立弹层 | 真实 IoT 卡与扫描连接服务未接入；两步原片段已内嵌，短窗口正文可滚动 |
| 主应用 Devices & Modules | HomePage `iwS` → 44442 / 6505；最后 return 中四个 `H` 分组 | 已与 Dashboard 分开；本地设备采用 80px 行、40px 图区、名称/状态/打开列 | 新设备、可用模块、固件更新和卸载分组需要真实服务数据 |
| 主应用 Global Shortcuts | HomePage `fUK` → 94608 / 7282；`Fe` → `Be/Oe` + 映射编辑器 | 独立 600px 内容区、顶部 Add 与 70px Add 卡；按键捕获、鼠标输入、修饰键/Hypershift、列表增改复制/删除、292px 编辑器、草稿继续、本地持久化及原引擎编码 | 原配置读取 ABI 未证实，原生替换提交禁用；宏/Chroma 和部分输出所需原生 Turbo 服务未接入 |
| 182 Customize | `GM.navs → em → $P → JP → QP`；设备图 `KP` | 八输入、双图叠层/连线、Standard/Hypershift；230px 输入抽屉及筛选、Tab 焦点自动滚动、292px 分类编辑器、按键录制和草稿继续 | 宏/跨设备/Chroma 服务、运行时能力、原生设备协议；登录/Quick Remapping 提示按[真实条件](11-mapping-warnings.md)保留为未接入分支 |
| 182 Performance | `GM.navs → lM → OM`；`AM/IM`、`dm/Rm` | 五个稳定 DPI 槽位、每槽启用/XY/数值编辑、至少两槽启用、拖放/键盘排序；原尺寸数字箭头、滚轮、300ms 长按；回报率和系统属性入口 | HyperPolling/8K 运行时条件及设备回读；未运行步进交互和视觉验收 |
| 182 Power | `GM.navs → MM`；`CM/pM` | 闲置时间和低功耗阈值，回报率条件禁用 | 实时无线/电源状态 |
| 182 Calibration | `GM.navs → mD → PD → pD → LD` | Smart Tracking、对称/非对称数值、联动和首次介绍 | 原介绍外观仍需细化；实际设备请求未接入 |
| 653 Customize | `zh.navs → nP → tP → eP → $m`；`zm/km` | 16 布局 / 1901 个精确命中形状、完整输入抽屉含拨轮子输入、按输入与层过滤的分类编辑器；游戏模式、最多四组 Snap Tap 录制/按键选择、回报率；Command Dial 预设与自定义模式管理及独立双方向映射 | 腕托/拨轮图层运行时状态、设备调整模式、硬件写入及外部服务 |
| 653 Lighting | `zh.navs → Fh → yh`；`_P/TP/Hh` | 亮度、关闭灯光条件、产品灯效和参数、效果下拉 | WDL/应用接管、Chroma 安装和硬件输出 |
| 777 Sound | `Ov.navs → WM → VM`；`dM/GM` | 音量、系统属性、六预设和音频 EQ；300px轨道/47px间距/右刻度与Reset；原随动数值气泡 | 实际播放设备、EQ差异确认 |
| 777 Mic | `Ov.navs → KM → kM → GM` | 单独 mic EQ；940px 卡、300px 轨道/78px 间距；独立草稿 | 固件频率语义和设备回读；无源码依据的 Mic Gain/Sidetone 不添加 |
| 777 Lighting | `Ov.navs → YG → WG`；`qM/VG/gG` | 亮度、Streamer 区域和该产品灯效/参数 | 硬件灯效、Chroma/Streamer 服务状态 |
| 777 Power | `Ov.navs → KG → kG → zG → wG` | 独立省电开关和 5–60 分钟滑条 | 设备初值、提交和失败回读 |

主应用页面入口集中在 [main_pages.rs](../../src/shell/main_pages.rs)，全局快捷键实体由 [shortcuts.rs](../../src/features/shortcuts.rs) 实现；设备页面集中在 [customize_page.rs](../../src/features/customize_page.rs)、[device_pages.rs](../../src/features/device_pages.rs)、[audio_page.rs](../../src/features/audio_page.rs)。当前所有普通路由均有入口，但入口存在不表示完整业务已实现。旧 `src/features` 中未被 [mod.rs](../../src/features/mod.rs) 声明的 free-function 页面不计入运行覆盖。

## 2. 帮助、独立窗口和附属界面

| 界面 | 原版挂载证据与边界 | 当前状态 |
|---|---|---|
| Alexa | `/synapse/alexa/` 的 `QE` 按安装数据选择 `qE` 或 `zE`；后者挂载 `Bp/Yp/yE/OE` | Home、Skills、Settings、Help、账户分支、安装器、更新说明和退出确认已接入独立页签；重复进入复用实体，关闭后重开重置；未知安装/账户不伪装成功，示例状态通过明确预览选择器查看，见[Alexa 规格](../screens/18-alexa.md) |
| 更多应用 | 主前端 `96776` → 独立 `/rz-app-menu/` iframe | 原 410px 三列弹层、设备/模块/其他应用/推荐条件、未读提示、禁用和安装态预览；本地设备可打开已有工作区，安装与原生应用查询仍未知，见[应用选择器](../screens/19-app-picker.md) |
| 三产品 HELP | 根 navs 分别为 182 `KN`、653 `Jd`、777 `Tv`；header 将 HELP 移到右侧入口 | 已实现设备内帮助页、产品支持/指南、序列号复制及2秒复位、固件版本和注册，Profile 在此隐藏；恢复出厂、序列号重试和产品 UI/MW/Synapse 字段绑定待接，设置中的独立版本查询不代替该绑定，见[帮助规格](../screens/11-help.md) |
| 多设备配对 | 182 `displayMode === multiDevicePairing` → `GG/UG`；`PG/mG` 创建 `/synapse/multipairing/`；主前端 4130 挂载扫描/绑定视图 | DUALLINK 0–11 状态机、候选卡、取消/超时/重试、713 警告、解绑确认及生命周期保护已实现；真实 transport 未接入，无虚构设备或成功状态 |
| Settings | 独立 720 chunk 的 `ho/uo` 组件树与对应 CSS | Synapse / General 的启动、通知、推荐、教程、语言、关于区域及偏好保存已实现；推荐重置与即时教程标记保持原范围；原启动、迁移、灯光服务未接入，见[设置规格](../screens/12-settings.md) |
| Profile 下拉与管理 | 182 `GM.render → SD/lD.renderProfileBar`；653 `zh → hD/UD`；777 `Ov → VP/bP` | 本地新增、复制、原位重命名、删除、182/653 重置、本项目格式导入导出及更多/确认界面已实现；最后一项禁删并保留映射草稿保护；原格式和原生 Profile 服务未接入 |
| 板载配置 OBM | 653 `UD.renderOBM → fR → LR/UR/gR`；182、777 根明确 `hasOBM:false` | 已补270px面板、当前混合配置、ID为2–5的四槽、分配选择、锁定/同步/失败、宏内存与独立冲突警告；真实回读仍未知且禁写，PREVIEW设备可显式查看状态示例 |
| Linked Games | 182 `GM.render → d_`、653 `zh → wh`、777 `Ov → Gm`；受 selectedProfile/showLinkedGames 控制 | 本地关联弹层支持 .exe 选择、路径去重、添加/删除、保存/取消和 Profile 隔离；原游戏库与运行时自动切换未接入 |
| Customize 左侧抽屉 | 182 `QP.render` 挂载输入列表 lazy 模块 5035，653 对应模块 95035；`isPanelOpen` 控制 `drawer-open` | 230px 列表、所有/自定义筛选、切键与关闭、独立滚动、Tab 自动滚到目标行、输入/层过滤、草稿继续及关闭后的焦点返回已接入；原版此处没有文本搜索 |
| 映射右侧编辑器 | `QP.renderPopup` 根据 activeButton 和 Chroma 加载状态挂载模块 5107；TwoTap 等有独立能力条件 | 292px 分类编辑器、录制、修饰键、Turbo、灵敏度、文本、Profile、Launch 等本地编辑已接入；宏/跨设备/Chroma 缺服务时不可保存，见[映射审计](09-mapping-editor.md) |
| 映射未保存确认 | `QP.renderSaveAlert` 保存 nextAction/dontSave 回调；主全局快捷键另有 `Fe.renderSaveAlert` | 设备内部切键、切页及 Profile 管理走保存/丢弃/继续编辑；跨宿主标签保留设备 Entity 与映射草稿；主应用四页切换保护快捷键草稿；非法草稿禁存 |
| Windows 登录映射警告/冲突提示 | `QP.renderWindowsLoginWarningAlert`、`isConflictedQuickRemapping` 条件组件 | 已审计真实条件；本地保存未应用映射，故不显示登录影响提示；Quick Remapping 缺实际小组件与冲突事件，见[警告审计](11-mapping-warnings.md) |
| Calibration 介绍 | 当前182 `LD` 根据全局持久化标记显示 `DD`；这是页面流中的介绍卡片，只有关闭按钮，无模态遮罩 | 本地首次标记已持久化，介绍与600px widget分别布局，窄窗最小宽度及关闭后键盘焦点已修正；见[当前专项审计](smart-tracking-current-audit.md)，未运行窗口验收 |
| Snap Tap 键对编辑 | 653 `wm` → `Ym`，读取 keyList/isEnabled，源码限制最多四组，adjustmentModeRunning 阻止操作 | 最多四组顺序录制/删除，跨组去重和禁键校验；Escape/失焦取消未完成键对；布局选择器补充左右修饰键与数字键盘 Enter；设备调整状态待接 |
| Command Dial 模式编辑 | 653 `km → Am/Tm` 未传 `hasSynapseCustom`，实际自定义输入为 ScrollRight/ScrollLeft；选用 uid 与高亮 uid 分开 | 已实现最多 100 个自定义模式、名称/颜色、启用/选用/高亮、拖放/键盘排序、删除/重置确认及双方向独立映射；至少保留一个启用预设，硬件写入待接 |
| Dashboard 教程 | chunk 4130 的教程状态与视频入口 | 已嵌入原视频帧，保留教程步骤与已读状态；独立五步 Synapse 和三步 Chroma 教程另有各自页签，见[教程规格](../screens/16-introduction-tour.md)；未运行播放验收 |
| Gamer Room 教程/连接 popup | chunk 9388 的 `ce` 教程、多步控制；添加入口挂载 `IotPopupRoot` | 两步教程 Base Popover 与添加流程 Base Dialog 已接入；原设备扫描服务未接通，不生成房间设备或连接成功 |
| 快捷键删除确认 | chunk 7282 的快捷键卡 `deleteShortcut`/onDeleteShortcut 和删除确认组件 | 本地删除确认、取消及保存前丢弃恢复已实现；原生配置读取/替换仍受 ABI 边界限制 |

原版还会按账户、模块安装、设备在线、固件和应用状态显示更新/离线/重启/Armory 分享等浮层。这些条件界面的共享实现存在于本地包中，但不因此变成三个产品的默认主页面。对于缺少独立 render 的其他应用，如 Armory、Chroma Studio，本文只记录已有入口证据，不补造完整路由清单。

## 3. 下拉框细节与实际落地

182/653/777 的 `.s3-dropdown/.s3-options` 使用同类普通下拉样式：触发器 27px、14px 文字、透明底、1px `#515151` 边框；hover/展开为绿色边；右侧 29×25 箭头区域内放 10px `icon_expand`，展开转 180 度；禁用不接收输入并降为 0.3。普通菜单最高 180px，黑底，选项 25px 高，已选绿字、hover 白色 10% 叠加。颜色选择器和 dual-item 映射菜单属于不同分支，不能套这个普通下拉尺寸。

[synapse_select.rs](../../src/ui/synapse_select.rs) 的 `surface::select` 已应用于实际 Profile、映射功能、Lighting Effect 下拉：触发器 27px，菜单黑底、1px `#515151` 方角边框、零外围内距，25px 行内保留 4px padding，选中只用绿字，hover 白色 10% 叠加，无勾选图标占位，含边框最高 180px。原 10px 箭头放入 29×25 区域，展开转向；禁用不打开并保持值。映射功能宽度按 `.key-config .body .dropdown-area` 使用 210px。

GPUI Kit 0.7.0 的 Select 菜单内置布局没有公开外观配置，且 `SelectDelegate::render_item` 实际仍包默认行。应用改用公开 Base Select、Base Popover 与 List delegate 组合：框架负责方向键、选择游标、Enter、Escape、外点关闭、焦点及弹层定位；应用只提供外观并把确认写回原 SelectState、发出原 Confirm 事件。调用点从当前领域数据供给选项，映射保留设备能力过滤和已存映射回退。模块内保留真实交互测试源码，覆盖选择事件、Escape、禁用、外部值保持与行布局；本轮不运行测试或应用，静态检查状态见[重构状态](03-implementation-gap.md#5-验证状态)，不能将编译通过当作运行时视觉验收。

### Profile 操作与草稿

[profile.rs](../../src/features/profile.rs) 根据三个产品共享行为实现：新增使用计算机名加 `-Default`（无计算机名则 `Default`），重名添加空格数字；复制当前配置使用去掉末尾数字副本后缀的 `名称 (n)`，选择最小可用编号。两者创建独立身份和设置；新增使用默认设置，复制保留当前配置的独立设置副本。重命名按原 `JN/QN` 在原选择框位置编辑，宽屏 230×27、32 个 UTF-16 单元限制、trim 与同名检查，Enter/失焦提交、Escape 取消。窄窗选择区允许收缩，Profile 图标和更多按钮保持可见。

更多菜单按原 `jN` 分组，使用 Base Popover 与框架 List 负责焦点、方向键、Enter、Escape 和外点关闭，应用提供黑底、1px 边框、155px 宽、27px 命令行和 9px 分隔。删除和重置沿用 `.profile-del` 的 300px、20px 内距、红边和 27px 按钮，使用原中文确认文案；777实际嵌在更多按钮的相对定位包装内，因此直接子选择器top=100不匹配，实际top=52，其余产品top=42。777边框/按钮为#c8323c、按钮白字，标题仍#fd4949。182/653保留两类重置，777不显示重置。

删除最后一个配置在界面和领域操作两层禁止；删除当前配置后选择第一个剩余配置。所有会清除映射编辑器的动作继续进入既有 `Continue` 保存/丢弃/继续编辑路径，Profile 切换保留各自设置草稿。操作仅改变本地设备工作区，使用现有保存/丢弃流程；板载、原生动态配置和 Armory 仍需真实服务状态。

[profile_transfer.rs](../../src/features/profile_transfer.rs) 接入本地导入/导出弹层与异步文件选择。`.razer-ui-profile.json` 包含明确格式、版本、产品/布局及单个 Profile 设置；导入检查大小、完整字段、范围、重复键和身份，确认后创建独立 Profile 并处理重名，导出保留当次快照。它不是原 Synapse 的 base64/hash 文件封装。文件选择返回时校验设备/Profile 目标，错误显示在原弹层，不覆盖其它配置。

[linked_games.rs](../../src/features/linked_games.rs) 接入每个 Profile 的关联列表：浏览已有 `.exe`、规范化路径并去重、删除、保存/取消，最多 128 个本地关联；保存时核对 Profile 和原列表，避免异步返回或并发修改覆盖其他设置。此列表不执行/监控游戏，也没有原生自动切换 Profile 服务。

[profile_tests.rs](../../src/features/profile_tests.rs) 保留独立副本、名称、最后一项删除、保存身份和重置范围的领域用例，以及真实菜单/输入/确认路径的交互用例，涵盖 Enter/Escape、禁用删除、取消/确认、映射草稿保护及 700px 窄窗 containment。本轮未运行这些用例；静态检查状态见[重构状态](03-implementation-gap.md#5-验证状态)。

## 4. 设备图片按身份解析

原版通过产品 context import 和 `productId/editionId/layoutId` 选择鼠标、键盘和耳机资源，DPI 底部、腕托和拨轮还有各自图层；Dashboard 又使用独立 PluginImages URL。这些图应从实际资源文件同步，不能把旧截图或一种产品正面图通用于所有用途。

当前图片选择读取设备身份，经过资源 resolver 查找已打包资源。鼠标正面与底部层、键盘产品图、音频产品图均采用该接口。缺失 edition 按原版 ProductImage 的回退逻辑查 edition 0，并保留同一 layout。`km.render` 的 `layoutId || 1` 使缺省/0布局显示布局1并启用对应命中；非零未知编号使用本项目提供的基础键盘预览，禁止命中与映射。16 个已知布局的位图、1901 个精确命中形状及完整 groupList 已分别接入产品图、抽屉和编辑器。资源的具体变体、hash 与同步链见[资源索引](04-resource-index.md)和[样式审计](06-style-source-audit.md)。

后续仍需接入有源码依据的运行时条件、原生设备映射、Profile 原格式/板载/游戏自动切换、宏/跨设备/Chroma、模块安装及快捷键原配置读取。原引擎编码已完成当前可支持输出，未注册 Turbo 的分支明确报错；不能将本地快捷键列表直接覆盖未知原配置。已有抽屉、映射编辑器、五槽 DPI、键盘控件、Profile 导入导出/关联和快捷键操作均保留回归源码；本轮仅做 `cargo check`，不运行测试、应用或 DLL，仍需实际窗口视觉与键盘验收。功能范围按已取得源码和实际产品能力确定，不按资源目录里出现的通用常量增建页面。

本轮对弹层位置、尺寸、遮罩与实际关闭方式的源码核对见[弹层审计](14-overlay-source-audit.md)。

## 5. 本轮补入的条件界面

- 灯光与拨轮共用原版40预设、16自定义色槽与二级拾色器，支持右键编辑/删除、删除补位、HEX/RGB/色面/亮度，以及明确的保存/取消。自定义槽共享并独立保存；Dial的无颜色项按最终CSS隐藏，系统吸管仍需原生服务。
- Devices & Modules 的下载、校验、安装、取消、重试、卸载和固件详情以17类明确标记的预览状态补齐，可从设置“服务连接 → 本地工作区 → 预览模块界面”检查；正常页面仍没有真实安装器回读，不把预览当作安装过程。
- Settings设备灯光按原Windows版本条件显示，并补两种模式的正文、警示和切换状态；独立预览不修改控制权。
- Settings的配置迁移入口已接入独立主页面、设备/模块卡片、半选、空状态与扫描/迁移/结果弹层，详见[配置迁移](../screens/13-profile-migration.md)。扫描和转换提交服务仍未接通。

## 2026-10-02 教程与折叠补齐

Dashboard 的 Tour 卡已打开独立五步 Introduction Tour，关闭清理页签历史、重开从第一步开始；与 Dashboard 单步和 Gamer Room 两步的已读状态独立。[教程规格](../screens/16-introduction-tour.md)记录媒体来源、短窗口滚动和剩余 Chroma 分支。Dashboard 三组的折叠恢复原 300ms 高度／位移／箭头通道及 overflow 关键帧；只编译回归用例，未运行窗口。

同日后续补入 `jn` 的 Chroma 三步教程：Quick Effects、Advanced Effects、Chroma Apps，使用对应 570×420 原图及完整 locale 正文。设置内明确预览入口打开独立页签，与 Synapse 五步各自保存步骤、滚动及关闭生命周期；关闭后重开从第一步开始。该内容适配不代表 Chroma 主应用宿主或服务已接通，见[教程规格](../screens/16-introduction-tour.md#chroma-分支)。
