# Rust 重构状态与剩余差异

更新日期：2026-10-02。用户确认按文档重构后，应用已切换到新的 GPUI Kit 设备工作区。以下描述实际编译入口；原版行为仍以逐页 JS / CONFIG / CSS 及实际资源为准。本轮尺寸、层次、控件样式和资源同步的详细证据见[UI 样式源码复核](06-style-source-audit.md)及[弹层复核](14-overlay-source-audit.md)，旧截图不作为依据。

## 1. 已接入的结构

| 部分 | 当前代码与行为 | 验证范围 |
|---|---|---|
| 应用外壳 | [shell.rs](../../src/shell.rs) 负责主应用、设备标签、历史导航、关闭确认、异步保存 | 主应用四入口与独立 Settings；设备路由不混入 Home/Setting |
| 产品路由 | [nav.rs](../../src/nav.rs) 按 PID 定义：182 Customize/Performance/Power/Calibration；653 Customize/Lighting；777 Sound/Mic/Lighting/Power；七款已核对鼠标垫 Lighting | 未知 PID 不借用类别路由；Pairing 为独立入口；无 Scrolling |
| 设备状态 | [workspace.rs](../../src/features/workspace.rs) 每个设备实例一个 Entity，设备身份包含 PID、serial、container | profile 草稿、控件同步和映射 continuation 留在设备 owner |
| 领域数据 | [settings.rs](../../src/features/settings.rs) 的 ProfileSettings 存在每个 Profile 下 | Sound、Mic、DPI、校准、灯光、键盘及映射分别保存 |
| 控件 | [controls.rs](../../src/features/controls.rs) 保存通用控件和订阅；[sensitivity.rs](../../src/features/sensitivity.rs) 按稳定槽位 ID 保留 DPI 输入与滑条 | render 引用已有 Entity；程序同步不重复发送用户 Change；排序不重建 DPI 控件 |
| 页面 | [audio_page.rs](../../src/features/audio_page.rs)、[device_pages.rs](../../src/features/device_pages.rs)、[customize_page.rs](../../src/features/customize_page.rs) | 页面不导入 AppShell |
| 服务连接 | [runtime_page.rs](../../src/shell/runtime_page.rs) 的设置面板通过后台线程持有 [ServiceClient](../../src/backend/runtime.rs)，点击连接后才启动独立 worker | 分别读取 HID 接口元数据、服务版本和音频列表；各项失败独立显示；断开在后台关闭 worker，保留最近结果；未实际启动验证 |
| 资源 | [resources.rs](../../src/resources.rs)、[打包清单](../../assets/synapse/manifest.json) | include_bytes 打包 PNG / SVG / 动画 WebP / TTF；不在运行时读取 .ref |

旧的 free-function feature 页面和 ui/widgets.rs 不再列入模块声明；文件保留以免删除已有工作区改动。它们不能作为当前运行 UI 的入口依据。domain.rs 中旧的 DeviceFeatures 暂用于旧配置反序列化和 CLI 探测，不能据其字段给新页面增加控件。

## 2. 产品页面完成情况

| 页面 | 已实现 | 尚需接入或进一步核对 |
|---|---|---|
| 182 Customize | 鼠标正面和 DPI 底部叠层、八输入及连线；230px 输入抽屉、所有/自定义筛选、独立滚动和 Tab 焦点滚动；292px 分类映射编辑器、按键录制、输入限制、Standard/Hypershift 独立草稿及保存/丢弃/继续编辑 | 宏/跨设备/Chroma 服务、原生设备映射协议、设备能力与条件多设备配对状态；登录/Quick Remapping 提示须按原触发条件接入，见[警告审计](11-mapping-warnings.md) |
| 653 Customize | 按 edition/layout 解析产品图；16 种已知布局的 1901 个精确 path/circle/rect 命中形状，完整输入列表含无几何拨轮子输入；未知布局只作预览；按输入/层过滤映射；游戏模式、回报率、系统属性；最多四组 Snap Tap 顺序录制与布局按键选择；Command Dial 预设及自定义模式的启用、选择、命名、颜色、排序、删除、重置和独立双方向映射 | 腕托连接与拨轮图层的运行时状态、设备调整模式；宏/跨设备/Chroma 服务和硬件写入；登录提示不适用于本地保存或原自定义拨轮保存分支 |
| 182 Performance | 五个稳定 DPI 槽位、各槽启用与 X/Y 状态、100–30000 step 50；每槽数字/滑条编辑、至少两槽启用、拖放及键盘排序；原 62×26 数字步进外观、上下箭头、滚轮和 300ms 长按步进；阶段关闭时只显示当前槽且保留其他值；普通回报率 125/500/1000、BLE 隐藏、系统属性 | HyperPolling/8K 的接收器、运行数据、固件条件及设备回读；步进事件和实际窗口视觉尚未运行验收 |
| 182 Calibration | Smart Tracking；对称 1–3；lift 2–26、landing 1–25；自动保持 landing < lift；独立持久化首次介绍标记；RESET 是说明 | 设备请求、回读和失败恢复；原文翻译与介绍外观精细对齐 |
| 182 Power | idle 1–15，无总开关；低功耗阈值 5–100 step 5；高于 1000 Hz 禁用阈值 | 有效无线回报率和电源状态来自运行时服务 |
| 777 Sound | 实际产品图、音量标题行开关与声音属性左列、右列独立音频 EQ；六预设、300px 纵向滑条、47px 频段间距、右侧刻度、原版 Reset 和随动数值气泡；-5..5 step 1、Custom 缓存 | 声音设备输出可用性、设备 EQ 差异确认及回读 |
| 777 Mic | 仅独立 mic EQ；五预设、300px 纵向滑条、78px 频段间距、940/473 基线、右侧刻度、Reset 和随动数值气泡；编辑与 Reset 保留 Custom 语义 | 真实设备频率与本包预设采用音频频率这一矛盾仍需硬件验证；保留每段 frequency，未静默改成另一套 |
| 653/777 Lighting | 产品自己的列表与数字 effect ID；profile 亮度；653 闲置/屏幕关闭条件；777 Streamer 插图和外链；按效果编辑颜色、随机、持续时间、方向、屏幕区域、Audio Meter 色彩增强 | 运行时 WDL、应用接管、adjustment mode、硬件效果条件与 special edition；Chroma 安装/激活；设备灯效输出 |
| 七款鼠标垫 Lighting | 3072/3073/3074/3076/3077/3078/3080；按源码区分 100/66 默认亮度、五/七种效果、Wave 11/12 与 1/2、显示器关闭、Reactive 无兼容鼠标提示；独立预览和配置保存；17 种产品及 Dashboard 资源 | 硬件/Chroma/WDL 服务、真实兼容鼠标列表、应用接管和动态窗口验收；见[规格](../screens/17-mouse-mat-lighting.md) |
| 777 Power | enabled + 5–60 分钟 step 1；禁用时滑条不可编辑 | 初始设备读取与提交确认 |
| Pairing | 独立 display mode；DUALLINK 0–11 状态机、扫描与绑定视图、取消/超时/重试、713 警告及解绑确认；未连接时明确不可用 | 真实 DUALLINK transport 尚未接入；不生成虚构设备或配对成功 |
| 十款产品 Help | 设备内路由、原左右区块、产品支持/指南、序列号复制及2秒状态、固件版本、注册；Profile隐藏；1279px换列 | 恢复出厂和序列号重试服务；设置已有独立版本查询，但 Help 的产品 UI/MW/Synapse 字段尚未绑定服务响应；见[帮助页](../screens/11-help.md) |
| 主应用 | Dashboard 原卡片布局、原产品图及折叠；Devices & Modules 独立80px行与模块详情；Gamer Room 横幅、热点、教程、添加流程及独立弹层；Global Shortcuts 捕获、增改复制、删除确认、292px 编辑器、草稿继续和本地保存；已实现原引擎映射/hash 编码；本地快照、设置内产品预览 | Gamer Room IoT服务和模块安装未接入；已证实 ABI 无法读取原快捷键配置，原生替换提交入口保持禁用；部分输出仍需原生 Turbo 事件服务，见[快捷键编码](12-global-shortcut-encoding.md)和[运行时边界](10-runtime-integration.md) |

## 3. 保存与兼容边界

[store.rs](../../src/store.rs) 使用 version=2 的对象，包含 devices、tracking_intro_seen 与默认兼容的 shortcuts、preferences。读取旧的顶层 Vec<Device>（含 UTF-8 BOM）；旧 DeviceFeatures、设备字段和 profile DPI 仍保留。每个 profile 新增可选 settings；缺少时按已确认字段迁移，旧的设备级音频/轮询/电源/灯光只归属当时的 active profile，不能复制到每个 profile。

旧灯效中 Ambient/Fire/Tidal/Wheel 曾被合并为其它枚举，已经丢失的身份无法自动恢复；不会反向猜测用户原本选的效果。旧校准扫描状态也不转为 Smart Tracking 成功。

保存先写临时文件、flush/sync，再备份既有 profiles.json 为 profiles.json.bak，最后 rename。失败会保留 dirty，并显示错误。未知版本或损坏配置导致加载错误；应用禁止覆盖这个文件，不把它当空配置重建。

保存捕获本次设备快照。异步写入期间又发生的编辑仍保持 dirty；成功只更新捕获的已保存版本。首次介绍偏好用已保存设备快照单独持久化，避免顺带提交其它未保存配置。映射空按键名称不能通过“保存全部”绕过校验。丢弃设备更改同时恢复 profile 列表和控件值。

[Profile 管理](../../src/features/profile.rs) 已补齐原更多菜单中的本地新增、复制、原位重命名、删除及 182/653 重置。新增创建默认设置，复制保留独立设置副本；名称按原唯一名规则生成，重命名保留 32 个 UTF-16 单元限制。删除保留最后一个配置并选择首个剩余项；重置区分按键绑定和全部设置。列表增删改与保留的 SelectState 同步，所有离开映射编辑器的操作复用 Continue 草稿保护。详见[界面覆盖](07-page-coverage.md#profile-操作与草稿)。

[本地 Profile 导入导出](../../src/features/profile_transfer.rs) 已接入更多菜单和文件选择器，使用带格式标识、版本、产品及布局的 `.razer-ui-profile.json`。导入校验 2 MiB 大小、字段、范围、重复 JSON 键和设备身份，确认后创建新配置及唯一名称；导出保存当前配置的独立快照。[关联游戏](../../src/features/linked_games.rs) 已提供按 Profile 管理的 `.exe` 文件列表、路径去重、删除、保存/取消，以及异步返回和并发修改保护。它只保存关联，不监控或启动程序，也未实现游戏运行时自动切换 Profile。

以上均为本项目本地格式，与 Synapse 原 profile 服务、原格式和板载槽位的兼容性尚未实现。全局快捷键随本地设备快照一起保存，异步保存只确认捕获的版本；原生引擎编码也不等于注册成功。“保存到本机”不表示设备或原服务已收到设置。

## 4. 新发现并修正的原版依据

### 键盘 Customize 与 Chroma 的几何不同

653 main 模块 **21368** 导出布局 1 的 groupList；模块 **30387** 的 `me` 按模块 **13254** 的 `rE` 布局枚举选择其余布局。16 个已知编号为 1–12、15–18，共 1901 个命中形状；每布局另有 4 个无几何输入供抽屉与编辑器使用。IM 根据 buttonList 的路径、矩形和圆绘制，km 传 minHeight=340，IM 缺省 minWidth=730；产品图由 pM 按 layout 读取并以 830 宽居中。

此前文档把 965 chunk 的 SVG_PRODUCT 960×360 提升为 Customize 命中图，这是不准确的。该资源仍用于 Chroma 区域，保留 SVG 与 DEVICECONFIG，但不叠在自定义页 raster 上。[资源生成工具](../../tools/extract-keyboard.cjs) 只解析 AST 字面量，不执行下载 bundle。

### 灯效数字命名空间

653 模块 13254 的 AoV / iE 产品 ID 为 Static=1、Breathing=2、Spectrum=3、Wave=4、Reactive=5、Ripple=6、Starlight=7、Fire=8、Ambient=11、AudioMeter=12、Wheel=13、Tidal=19。新 Effect 序列化这些产品数字，不能拿 LightingEngine effect ID 代替。

RE 默认参数与 SP/VP/zP/EU/SU/DU/GU/yU 渲染进一步核对：Reactive/Starlight duration=1..3；Audio Meter colorBoost=.25..4、step .25；Tidal direction=1/0；Wave、Wheel=1/2；Ambient 屏幕区域 full/left/top/right/bottom。两色效果按 isRandom/color2=no-color 分支处理。Fire/Spectrum 当前普通分支没有额外参数面板。

## 5. 验证状态

新增测试在 [领域设置](../../src/features/settings.rs)、[生产视图交互](../../src/features/workspace_tests.rs)、[Profile 管理](../../src/features/profile_tests.rs)、[存储](../../src/store.rs) 与 [路由](../../src/nav.rs)。交互测试创建真实 DeviceWorkspace + Root，发送鼠标拖动、点击和键盘事件，检查音频隔离、profile 切换、Smart Tracking 联动、键盘命中、映射退出、设备身份及异步保存版本的 dirty 行为。本轮另覆盖 1080/1280 宽度与 125% 字号比例下的导航居中和鼠标双图坐标，以及 EQ 实际高度、频段间距、右侧 Reset、Mic 容器和开关 Tab/空格操作；Profile 用例覆盖更多菜单、Enter/Escape 重命名、删除保护、草稿继续与 700px 窄窗。均保留为测试源码，本轮不运行测试。

新增源码用例还包括五槽 DPI 的独立状态、向上取整、排序、迁移与延迟事件保护，输入抽屉筛选/Tab 自动滚动/草稿继续，多布局几何、映射辅助键与符号 Shift、未知值保留和能力过滤。关闭、切页、筛选移除行及全局保存的焦点恢复见[独立回归源码](../../src/features/mapping_focus_tests.rs)。本轮另保留[键盘控件](../../src/features/keyboard_controls_tests.rs)、[Profile 导入导出](../../src/features/profile_transfer_tests.rs)、[关联游戏](../../src/features/linked_games_tests.rs)、[快捷键编辑](../../src/features/shortcuts_tests.rs)及[引擎编码](../../src/features/shortcut_engine_tests.rs)回归源码。

验证仅允许 `cargo check --locked --all-targets`，包括测试目标的编译检查。本轮不运行应用、build、测试或 DLL；前序同日检查通过不代表随后新增内容已经检查，最终结果以统一静态检查记录为准。更早的 `cargo test --locked`（30 项通过，其中 7 项生产视图测试）和普通 Clippy 属于前序版本，不代表当前新增用例已执行。

2026-10-01 只读资源校验通过：`python -B tools/validate-resources.py` 核对 160 条源/输出 SHA-256、98 个原 Webpack 请求、56 条动态图片解析项、PNG 尺寸、SVG 视口、嵌入表，以及 16 布局共 1901 个命中形状；`node tools/extract-keyboard.cjs --check` 确认已提取内容与原始 AST 一致。两项均不启动应用或重新生成资源。旧 domain.rs 测试只证明旧兼容模型自己的行为，不作为新页面存在相关功能的证据。

样式依据来自原版 JS、CSS 和资源文件。已有及新增的 headless 测试源码用于验证生产视图布局，本轮未执行；编译检查也不能代替运行结果。真实硬件写入、驱动回读及不同系统 DPI 下的完整窗口表现仍需各自验证；测试中的 125% 字号比例不等于操作系统 DPI 验证。

预览设备明确标为预览；Dashboard/设备工作区仍来自旧快照或本地配置。设置中的 HID 查询独立显示真实接口元数据和失败项，尚未把接口合并为具备完整 DeviceInfo 的设备工作区，也不修改本地 Profile。连接面板只在点击“连接并读取”后启动服务；本轮没有点击或执行该路径。未连接的服务入口保持可解释的不可用状态。

## 6. 2026-10-02 补齐与复核

独立 Settings 已按原版组件树恢复 Synapse / General 分页、通知、推荐、教程、语言和关于区域。设置保存只提交设置快照；即时教程标记使用已保存的其它偏好，避免提交设备、快捷键或设置草稿。推荐重置清除已忽略及已拥有产品，保留类别过滤。详见 [Settings](../screens/12-settings.md)。

Gamer Room 添加流程、教程及配对弹层按实际 JS 挂载与 CSS 最终覆盖重新核对。保存确认改用 Base Dialog 提供原版外观，保留 182/frontend 与 653 各自的纵向位置、400px 面板和黑色 50% 遮罩；配置导入导出、卡内确认、713 警告使用各自样式。详见[弹层审计](14-overlay-source-audit.md)。

主应用 Dashboard 按原 PluginImages 的产品、配色和布局解析资源，不再借用 Customize 图片。资源的实际消费者、条件变体与剩余缺口见[资源使用审计](13-resource-usage.md)。


## 7. 本轮新增界面与剩余边界

- 颜色组件补齐40个预设、16个自定义槽及二级拾色器，支持编辑、删除和保存/取消。自定义颜色单独持久化，不连带提交设备或Settings草稿；系统吸管尚未接通。
- 653 Command Dial补齐行内操作、独立颜色、帮助提示和原始锁定条件；Gaming Mode按布局处理Windows/Menu/Copilot。`Em`中自定义模式及`Switch Applications`的特殊限制按源码保留。
- 板载配置补齐270px面板、槽位、宏内存、同步/错误和独立冲突确认。常规设备等待真实回读；显式示例只在预览设备可用，不修改工作区配置。
- 模块目录增加17类原状态的显式预览，包括下载、安装、取消、错误、移除和固件说明；常规目录仍需真实服务数据。
- Settings补齐语言选项、布局、内嵌社交图标的常态/悬停，以及Windows动态灯光界面。迁移与发布说明取得独立原版源码并接入界面；Synapse 3扫描、转换、导入及真实发布说明读取尚未接通。
- 777删除框按实际挂载修正为top52，并恢复不同的标题、边框、按钮颜色及白色按钮文字；导入/导出不再点击遮罩关闭。

原版不止三个产品模块。官方目录有449个主产品ID；沿明确别名与声明共核查596个ID，已取得331个产品清单所列的30,658份JS/CSS。入口AST识别出331个产品的实际导航数组，Rust当前接入182、653、777及3072/3073/3074/3076/3077/3078/3080；详见[最新产品目录](16-product-catalog.md)和[鼠标垫规格](../screens/17-mouse-mat-lighting.md)。上述界面补齐不等于所有产品或真实宿主服务已完成。

滚动与弹层反馈已定位到内容高度约束、滚动句柄和动画生命周期，修复范围及未验证项见[专项复核](18-scroll-and-motion-audit.md)。本轮仍只执行编译与静态检查。

2026-10-02 补齐 Synapse Introduction Tour 的五步页面和宿主页签，Dashboard／Gamer Room 三段原教程媒体改为内嵌动画播放；Dashboard 分组恢复源码折叠时序。后续补入 Chroma 三步原图教程及独立页签，设置内提供明确预览入口；详见[教程规格](../screens/16-introduction-tour.md)。未适配产品、Chroma 主应用宿主及真实服务缺口仍保留。

同日继续核对用户指出的添加设备层级、按钮宽度和右上角设置 hover：IoT 外层 Modal 与内部 Select 保持正确绘制顺序，教程在完整模态周期内暂停，恢复原外层关闭图标和100/150/300ms挂载／过渡／移除时序。模块按钮使用文字自然宽度、90px最小宽度、原大写规则和200ms整控件透明度过渡；设置齿轮保留方角与原 hover/active 背景。鼠标垫提示补齐原警告图标，Tidal 方向间距及缩放圆角按原规则修正。

上一批静态验证：`cargo check --locked --all-targets`通过；`python -B tools/validate-resources.py`核对396项资源、132个Webpack请求、73个产品图片变体、58个Dashboard变体，以及16布局/1901输入形状；`git diff --check`与修改过的Python工具语法解析通过。新增交互回归仅编译，不运行应用、测试或DLL；该结果不替代真实窗口与硬件验收。

同日继续补入 [Alexa](../screens/18-alexa.md) 的 Home、Skills、Settings、Help、账户状态、安装器、更新说明及退出确认，宿主页签复用原 `alexa` 身份。10种语言的185项原文以 `ALEXA_SOURCE` 独立命名空间保存，避免覆盖主前端同名文案。原图标与关闭/刷新资源来自其自己的 manifest；Alexa 下拉沿用 Kit Select/List 行为，增加原230×27尺寸和100ms延迟、100ms开合过渡。普通入口保留未读取安装/账户状态，示例不触发登录、安装或录音。

[更多应用弹层](../screens/19-app-picker.md) 已按独立 `/rz-app-menu/` 源码接入工具栏，恢复410px宽度、三列96px项目、分组条件、原始图标、标题展开、未读标记和安装状态。安装事实与本地页面可用性分开；本地设备沿产品/容器身份打开原工作区，Alexa、添加Wi-Fi设备和配置迁移各自走既有入口。设置提供隔离的状态预览，不将本地实现计为已安装模块。

七款鼠标垫在控件列之前遗漏的250px产品展示区已补齐，按原图比例显示，保持1024–1220px宽度及共同的水平滚动区。共享点阵定位修正为居中22px背景块的最后2px，包含边缘被裁切的点，并保留原径向淡出。

新增素材统一由[应用资源采集工具](../../tools/fetch-application-assets.py)和[资源准备工具](../../tools/prepare-resources.py)记录来源及哈希。本批最终 `cargo check --locked --all-targets` 通过且无编译警告；资源校验核对421项资源、132个Webpack请求、73个产品图片变体、58个Dashboard变体、16布局/1901输入形状。修改过的Python工具语法解析、10语种各185项Alexa字典和新界面静态图片引用检查通过，`git diff --check`通过。没有启动应用、测试、安装器或DLL；编译通过不代替实际交互验收。未适配产品、其他独立子应用及真实宿主服务仍是后续范围。

## 8. 当前正式源与三处用户反馈

前述来源迁移后的结论以[当前源码记录](20-current-source-version.md)为准：官方Dashboard为0.0.86 / 2609221012；正式宿主更新流已取得4.0.827并静态提取，未升级本机4.0.821。教程、配对SVG、宿主图标/字体、应用目录与10种Dashboard文案已改从当前源重新生成；校验器在读取前拒绝停用目录，防止旧快照哈希仍被当作当前证据。

设备与模块页的额外“Razer应用”按钮已移除。顶部额外常驻保存按钮已移除，但补回了原代码条件出现的未保存配置徽标和全部保存/丢弃菜单，不能概括成“原版没有保存功能”。Gamer Room 教程恢复真实设备组容器坐标和两步偏移，去掉额外行、窗口吸附和高度压缩；精确证据见[工具栏审计](toolbar-current-audit.md)及[教程定位](gamer-room-current-audit.md)。

本地快捷键提交现在独立串行落盘；设备配置、设置、快捷键编辑草稿的保存范围分别保留。排队写入确认捕获版本，丢弃配置保护映射草稿，保存并关闭请求跨队列保留。这里仍是本项目的本机持久化，不代表原版服务或硬件收到写入。完整产品适配和真实服务边界未因这次修正自动完成。

本批最终全目标编译、全仓Rust格式和差异检查通过；422项资源、10种当前Dashboard字典与来源收据、127条快捷键编码静态校验通过。未运行应用或测试，详细范围见[当前来源与静态检查](20-current-source-version.md#本轮最终静态检查)。用户随后手动删除四个旧源码目录；已确认目录不存在，且删除后资源校验仍通过。


## 9. 2026-10-03 ????

??Performance???Kit?????hover???Gamer Room??????????????????????????????????????????[??????](interaction-fixes-current-audit.md)?

Settings?????GET??????????????????render???????????????????????????????About???????????????Settings???????????????????????[????](settings-current-audit.md)?

??????????????????????????[?????????](21-ui-completion-status.md)???ID????????????????????????????????????????
