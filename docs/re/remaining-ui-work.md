# 尚未完成的界面与产品

2026-10-05 ?????Macro ?????????? Mouse/Loop ??????????Dashboard ?????????????????????[????](continuation-row-actions-2026-10-05.md)??????????????????????????

2026-10-05 本轮已接入 Macro 全选/顶部操作、键盘配对高亮、Dashboard 电池状态和691 OLED预设编辑卡片。Macro 每行复制/删除、成组拖动与 Phased 阶段编辑已按当前源码接入；录制、执行、设备服务和实际画面验收仍未完成。详见 [Phased 审计](macro-phased-current-audit.md)。

2026-10-05 用户要求把已有页面全部纳入字体、布局、颜色、图标、功能和动画复核。三个专项任务正在按当前实际挂载源码核对；本轮已暴露公共正文 padding、RazerF5 粗体未注册、Nommo 多余选项、相机分列及 Alexa 正文/提示等差异。见[完整覆盖索引](ui-consistency-round-2026-10-05.md)。以下可达性统计不是视觉或功能验收；旧记录中“已有”页面仍须逐项复核。

2026-10-05 Macro Keyboard 已接入当前窗口捕获、成对键事件和持久字段；设备输入重定向、原光标、配对连线/动画、序列和实际窗口验收仍缺。

2026-10-05 Macro Launch 已补文件选择器和源草稿/定位规则，Phased 六层定位已连接阶段编辑器，详见 [Phased 审计](macro-phased-current-audit.md)。Text/emoji 与实际窗口验收仍待完成。

2026-10-05 嵌套菜单续接：编辑框与展开列表已分状态；重选同一宏只收起列表；补 688px 源阈值、当前帧定位、选中项滚入、名称撑宽和菜单/触发器过渡。见 [专项后续](macro-nested-menu-followup-2026-10-05.md)。钢笔编辑光标、滚动条外观及实际窗口验收仍缺；下面同日较早批次的定位/滚入/动画待办以本条为准。

2026-10-05 Macro 后续批次已接入共享文档库、持久 ID、动作草稿、保存提示、快捷键选择与分类播放。嵌套宏、Phased 阶段编辑、原生录制/执行、硬件服务和实际窗口验收中，Phased 界面与本地排序逻辑已接入，录制/设备服务仍是边界。

2026-10-05 链接会话续接：已补快捷键文本 emoji/字符面板、3946 quick macro 捕获会话、3907 独立 Armory 无遥测分支及纵轴、Devices & Modules 源记录投影与正式服务行。3894 DEFAULT 经当前静态产品配置证明不可达，不再列作必须补造的页面；3894 是 Head Cushion Chroma，旧 PWM 名称错误。验证及明确边界见 [本批记录](continuation-followup-2026-10-05.md)。整体目标仍未完成，以下旧轮次与专项新记录冲突时以同日续接记录为准。

2026-10-04 语言包键清零批次：`python tools/audit-locale-keys.py --check` 的待回溯项现在为
**0**。前几轮登记的 5 处全部收口：`SWITCH_OFF_LIGHTING_WHEN_DISPLAY_IS_OFF`/`_WHEN_IDLE`
（第 6 轮 → `DISPLAY_TURNED_OFF`/`IDLE_FOR_MIN`）、`MINIMUM`/`MAXIMUM`（第 7 轮 →
`<SIDE>_TRIGGER_RANGE`/`<SIDE>_ACTUATION_POINT`）、`MINUTES`（第 8 轮 → 源码 `GR` 组件的
`MIN`/`SEC` 模板）、`LINKED_GAMES_TO`（第 8 轮 → `LINKED_GAME_CHROMA_HEADER`，
"Games linked to profile:"）、`ADVANCED_EFFECT_DETAILS`（第 8 轮 → 键名无误，文本由新工具
`tools/prepare-device-locales.py` 从设备包语言表合并进 `locales/`）。审计现在覆盖 382 个字面量
`t()` 键、42 个软键与 42 个动态调用；动态键仍无法静态解析，只能逐个回溯（当前 42 处都已在
别处核对过键名）。语言包之间的键数差（de/fr 93、es 88、ja/kr 97、pt-BR 96、ru 94、zh-CN 61、
zh-TW 118）来自各语言包本身的缺失，不是本地漏提取。

2026-10-04 页面可达性批次：`tools/audit-page-coverage.py` 现在给出「主导航页是否都有渲染器」
的机检答案——**没有渲染器的页面槽位为 0**，剩余一行是 9 个无线键盘的 `TAB_PAIRING`，
它属于 `multiDevicePairing` 独立模式根（本地配对窗口承载）。本轮修掉了让 Raptor 显示器、
Hanbo、PWM 风扇控制器、散热垫、Core X V2 的已实现页面**完全不可达**的描述符守卫顺序问题，
并补齐监视器提示控件与刷新率计数器方块网格、HDR 的 Windows 11 分支。仍未接入的配件细节
（真实 `uiRestraint` 服务填充、显示器产品图、ICC/刷新率、散热曲线硬件值）
见[配件页面审计](accessory-system-native-ui.md)。

2026-10-04 displayMode 分支批次：产品侧四个根分支现在都有本地实现与机检收据，
逐产品的归属表由 `tools/generate-display-mode-roots.cjs` 从审计结果生成
（`src/features/display-mode-roots.json`），不再按单个产品写死。逐模式的规则、
颜色尺寸依据和本地偏差见[本地实现记录](display-mode-roots-implementation.md)。

- `chromaApp`（212 个产品包有根分支）：Chroma 窗口的设备卡在有本地灯光页时直接打开弹层根
  （适配器的灯光标签，或 source 家族的 `TAB_LIGHTING` 页），无灯光页时保留原版
  `.box-item-device .disabled`（`opacity:.3`、不响应指针）。设备卡几何按
  `.box-item-device`（290×245、`padding:10px 20px 20px`、`#111`/2px/圆角 5、
  hover `#44d62c4d`、active `#44d62c`）修正；`.name-tag` 不再居中，行高 16px。
- `armory`（226 个产品包有根分支）：工坊窗口新增设备面板，挂载该产品的映射页
  （适配器 Customize 或 source 家族的 `TAB_CUSTOMIZE`），高度按原版 `ie` 分支
  KEYPAD 460 / HEADSET·AUDIO 340 / 其他 420；入口是设备「分享到工坊」，与分享表单同屏。
- `macro`：产品侧分支仍是宏应用 iframe，本地由宏页承载（见宏审计）。
- `multiDevicePairing`：具名第二窗口（见配对窗口审计）。

仍未接入：音频/配件家族以及没有本地灯光页或映射页的产品（不打开，而不是伪造页面）；
两个根的 postMessage 往返在本地改为同进程直接挂载，消息名仅作契约记录。

2026-10-04 应用选择器去门控：Chroma Studio 已本地实现（Chroma 窗口页面），
选择器改为直接打开；Philips Hue 的模块页就是 769 产品工作区，本地存在该设备时也直接打开，
否则明确说明缺少设备。仍保留门控的只有本地确实没有页面的模块/应用
（`audio-visualizer`、`chroma-connect`、`sensa-hd`、Streamer Companion、Virtual Ring Light）。

2026-10-04 后续进展：Armory 本地分享表单已接到 182/653 配置更多菜单；Macro 新增
设备/配置/真实产品输入区域的会话内绑定；3946 自动化的星光、波浪、音频表参数
与快速宏 `.exe` 文件选择已落地。OLED 裁剪画布/移动/中心缩放与持久化几何已按当前
Cropper 源修正。上述均为本地 UI 进展；Armory 完整详情/上传、宏设备服务、GIF 编码
与原生裁剪输出仍缺，以下旧轮次描述若冲突以各专项当前审计为准。

2026-10-04 恢复会话后的更正：Macro、Armory、Profiles、Alexa、Feedback 的普通入口
已按 4.0.827 宿主实际处理接入同一窗口的具名页签。`policy=3` 不表示第二个系统
窗口；多设备配对按历史明确要求保留第二窗口，并标记为偏离其源调用的本地例外。
见[宿主页签更正](independent-module-window-audit.md)。以下旧进度中的窗口解释以此为准。

本轮同时修复相机步进器与滑块的共同初始化、鼠标释放重复步进、最终 CSS 覆盖和
小数格式，增加真实挂载的曝光补偿数字框；Feedback 补齐隐私类别与日志确认表单，
OLED 补齐缩放控制与后台文件读取并校验当前 worker 资源；后续已补齐裁剪画布几何。
GIF 编码、原生裁剪输出、设备传输及实际像素/交互验收仍缺，不能把资源齐备或编译
通过算作产品完成。

2026-10-04 本次集成：Macro、Armory、Profiles 已接入当前源码登记的具名 gpui 窗口（窗口名分别为 `macro`、`armory`、`profiles`，策略为 `policy=3`，同名再次打开时聚焦），并保留各自的页面外框与明确服务边界。Macro已替换占位正文，接入原始无宏布局、显式新建宏/文件夹、选择搜索排序、重命名复制删除、教程、无设备绑定分支、Help，本地动作草稿的插入/选择/删除/撤销/重做/保存，Delay/Keyboard/Mouse/Loop/Text/Command/Launch 参数编辑，以及编辑器底部源码定义的100px拖放保留区和事件行排序，并接入顶部历史；其真实录制、设备传输、XML、完整文件夹菜单和独立产品绑定模式仍未完成，详见[宏复核](macro-current-source-review.md)。Armory已纠正默认可见导航、补内部历史、介绍横幅、左侧 68142 搜索入口（200px 输入框与 300ms 防抖壳）和 Browse 顶栏多条件筛选、My Downloads 顶栏单排序控件及本地选中态；远端建议/结果、服务筛选/排序、详情弹层和内容服务仍缺。Profiles已接入真实两页签、设备磁贴、设备关联弹层、Profile选择/重命名、关联游戏磁贴和本地已知关联的切换，并为游戏扫描、已安装程序刷新、Profile服务菜单显示未连接提示；安装程序扫描、全局游戏目录与设备子设备数据仍缺，不能把空目录或本地关联当作服务同步结果。相机页保留实时流不可用边界，音频演示保留播放服务不可用边界，见对应审计文档。电量块补齐当前挂载的`role="img"`语义，电量/托盘过渡保持源码时序。后端与DLL修改继续后置。

2026-10-04 更正：旧日志停止原因是轮次上限，目标没有完成。旧表中的Profiles五路由、无法还原文案、缺少全部宏/Profiles图标及设备标签栏重复箭头均不再成立。最新修复和明确未完成项见[逐项复核](source-ui-review-2026-10-04.md)、[Profiles](profiles-app-audit.md)、[导航栏](device-tabs-audit.md)。以下旧轮次明细保留为历史，不是新增验收结论。


2026-10-03 当前状态：331 个注册产品、1419 个主导航页都有入口与部分内容，其中 1392 页记为 partial_native，27 页是原有十个适配器的部分实现。本轮没有把任何产品标记为完整复刻；“完全没有主体”的主导航页计数为零，并不表示条件分支、弹层、独立模式或视觉细节已完成。

## 已确认的缺口

| 产品或范围 | 尚未完成的 UI |
| --- | --- |
| 鼠标、键盘共享页面 | 完整自定义映射、高级动作、替代布局和条件分支；源产品工作区的配置更多菜单、部分右侧图标 |
| displayMode 独立窗口 | 四种根级分支的**打开者**都已从当前源码查清：`multiDevicePairing`（30 包）已按原版做成真正的第二个 gpui 窗口（具名单例、存在即聚焦，配对页设备卡为入口，见 [配对窗口审计](multi-pairing-window-current-audit.md)）；`macro`（174 包）不是第二个产品窗口，而是宏应用窗口（`macro`／`/synapse/macro/`／`policy=3,tab_visible=1`）在「绑定到设备」弹层里的 iframe，宏窗口本地已实现外框与两个导航标签（见 [宏应用审计](macro-app-current-audit.md)、[界面审计](macro-app-ui-audit.md)）；`chromaApp`（212 包）在 Dashboard 里出现 0 次，属于独立 Chroma 应用窗口；`armory`（226 包）属于 `armory` 应用窗口，本地已实现窗口外框与四个导航标签（精选推荐／浏览／我的下载／我的上传），并在模块目录里改为直接打开，见 [Armory 依据](armory-app-current-audit.md)；资料分享服务与内容未接入。模块目录七个盒子现在有六个直接打开（Alexa、宏、已关联的游戏→profiles 窗口、Armory、配置文件迁移、介绍导览），只有 `feedback` 仍是门控：它的窗口名 `feedback-synapse` 与 URL 都取自模块表，但该应用源码不在当前提取范围内，无法复刻界面。表与打开参数见 [模块注册表依据](module-registry-audit.md)。具名窗口清单、标志与模块盒去向见 [窗口契约](display-window-contract.md)。窗口图标（`app_icon_path`）在 gpui 里没有对应字段 |
| 740 / 746 磁轴键盘 | 完整页与配套校准弹层已接入；载入/光标动画、错误后关闭时序、出厂配置禁用分支及介绍状态持久化仍有缺口 |
| 691 BlackWidow V4 Pro 75% | OLED 预设导入/裁剪、主页 Emote/Banner/Media/System/Keyboard 分支已接入本地草稿；语言选择/下载、GIF 帧处理、设备传输进度/错误和完整 hover 动画仍缺 |
| 3592 / 3594 / 3595 / 3596 Kiyo | 已接入方向图、图像四行/变焦及实际挂载的曝光补偿步进器；最终 CSS、长按/释放、键盘/文字提交、小数及动态最小值按当前调用链纠正。锐度和增益在这四个根关闭，不再列作缺失行。实时流、设备枚举、第三方分支和实际交互验收仍缺，见 camera-framing-current-audit.md。 |
| 3587 / 3589 / 3590 Kiyo | 共享 Customize 布局已补 520×292 相机预览外框和明确不可用边界；实时流、设备枚举、取景与叠加层仍未实现 |
| 1392 / 1442 / 3942 audio demo products | Source poster, dimensions, floating preference, and the source-sized control bar are restored. The progress/volume sliders retain local preview values, while native audio playback, live timing, floating video and complex mappings remain open; clicking reports the service boundary. See audio-demo-playback-boundary.md. |
| 164 / 241 Mouse Dock | dongle 713 额外确认、部分配对状态动画、弹层 viewport / 滚动几何等价 |
| 778 ASRock B550 / 3871 Chroma ARGB | 页面主体、自动检测 50/100/700ms 动画与 LED 步进器 300ms 长按重复已接入；真实 ARGB 端口服务、检测结果和运行时视觉验收仍缺 |
| 3884 / 3886 无线 ARGB | 自动检测图标的 50/100/700ms 点击动画、300ms hover motion 已接入；源 path 颜色插值、tooltip 挂载时序、3886 原代码端口分支不可达仍待逐像素/服务验收，不能把样例编辑器算成原版普通页（见 [动画依据](wireless-argb-motion-evidence.md)） |
| 784 Aether Light Strip | 轮播平滑居中、Identify 延迟、部分 tooltip 触发/定位和底部提示定位 |
| 769 Philips Hue | 发现、连接、设备状态的完整交互和精确视觉验证；已接入主体与引导不等于全部完成 |
| 3946 自动化 / Base Station V3 | 实际挂载的 Static/Breathing/Starlight/Wave/Audio Meter 参数已接入；Fire/Spectrum 无参数面板。完整宏录制器、游戏浏览与关联、快捷键子编辑器、删除确认锚定、原始图标槽位及过渡动画仍缺 |
| 原有十个适配器 | 182、653、777、3072、3073、3074、3076、3077、3078、3080 的 27 个主页面已逐页复核（`partial_native_reaudited`，带本地路由与源码依据），见 [逐页复核](legacy-adapter-page-reaudit.md)；仍未进入 `source_help` 描述符、182 规格条目 `pages` 为空、653/777 仍走各自手写页面，仍是部分实现 |
| 设备页顶栏电量 | 已按当前源码接入（状态机 `off`/`Charging`/`charging100`/`NoCharge_BatteryFull`/`batt-warning`/`ReachChargingLimit`，图标与文案逐条取证，见 [电量依据](battery-indicator-audit.md)）；原版的 `hideBattValue` 与耳机左右耳电量因本地无数据来源未实现 |
| 3880 Raptor 27 165Hz | 已接入 NSA 两列 Color 页面、HDR、THX Cinema、色域与 PIP/显示器控件；`uiRestraint` 的源字段门控已接入本地渲染，真实宿主/设备服务填充、系统 ICC 枚举和产品图仍缺 |
| 所有产品 | 仍无实际窗口的字体度量、缩放、焦点、滚动和动画逐像素验收结论；不能以检查通过认定视觉完整 |

## 不属于单个设备的界面

- 完整托盘账户、Widgets / Notifications、多应用内容及动态高度。
- 账户登录、宿主独立 Settings 窗口、固件/重置等完整服务界面。
- 模块目录、顶部状态、更多应用和部分设置样例的完整外层复用；具体范围见 [服务预览复核](service-preview-current-audit.md)。
- 其余未接入的独立应用与产品模式；已有本地入口会直接打开，不能以一个下载状态卡替代已实现的页面。

真实配对、校准、相机流、音频处理、自动化执行、设备遥测、账户和安装服务也仍有独立缺口。它们与 UI 还原分开记录，不用示例状态冒充设备事实。

本清单是已知缺口，不是声称未列出的细节已经完成。逐产品、逐页面路由见 [原生覆盖表](native-product-coverage.md) / [机器可读清单](native-product-coverage.json)，总体状态见 [当前 UI 完成状态](21-ui-completion-status.md)。

## 第 13–30 轮新增或修正（均带审计脚本与依据文档）

| 内容 | 依据文档 | 机检脚本 |
| --- | --- | --- |
| 设备页顶栏电量补到 182 型产品的 `DeviceWorkspace::toolbar`（原先只加在源工作区） | [battery-indicator-audit.md](battery-indicator-audit.md) | `audit-battery-indicator.cjs` |
| 托盘：弹窗 360×200、左键不再切换、位置 `x = 托盘x − w/2`、`y = 托盘y − h` 并夹在 `rcWork` | [host-tray-audit.md](host-tray-audit.md) | `audit-host-tray.cjs` |
| 设备页共享控件：`.widget` 600×`padding:30px 40px`、标题 `#44d62c/RazerF5/16px/mb 20px`、`.h1-body`、`.check-item`、`.widget .content`、轮询率面板与警告块 | [device-page-css-audit.md](device-page-css-audit.md) | `audit-device-page-css.cjs` |
| 动作分类栏图标与悬停底色 `#393939` | [action-category-icons-audit.md](action-category-icons-audit.md) | `audit-action-category-icons.cjs` |
| 模块目录七个盒子六个直接打开（含 maco/armory/profiles） | [module-registry-audit.md](module-registry-audit.md) | `audit-module-registry.cjs` |
| 关联游戏磁贴：`.list-box` + 每游戏一张 `.linked-game-tile` + 固定 `.add-new`（40px `icon_add` 资产），接进关联游戏对话框 | [linked-game-tile-audit.md](linked-game-tile-audit.md) | `audit-linked-game-tile.cjs` |
| profiles 窗口：应用自己的五条路由视图 + 返回/前进历史 + `.razer-profiles` 视图容器 | [profiles-app-audit.md](profiles-app-audit.md) | `audit-profiles-app.cjs` |
| 设备页标签栏：悬停文字 `#ccc`、按下底色 `#3cbf27`、大写、`.nav.back/.nav.forward` 箭头与页历史、溢出 `.dots3` 菜单（`.profile-act` 面板 + `.act` 行） | [device-tabs-audit.md](device-tabs-audit.md) | `audit-device-tabs.cjs` |
| `.hover-border` 的 `transition:border-color .2s`（profile 栏「更多」与标签溢出共用） | 同上 | 同上 |
| 对话框按钮 `.thx-btn`（绿/灰两态、大写、`1px #0000004d`、悬停 `opacity .8`） | [thx-button-audit.md](thx-button-audit.md) | `audit-thx-button.cjs` |
| 导入/导出底栏：`.import-profile-btn-group`、导出模式的 `willNotImport` 文案、关联游戏弹层打开时导航行 `opacity .5`（`div.nav-tabs.disabled`） | [import-export-footer-audit.md](import-export-footer-audit.md) | `audit-import-export.cjs` |
| 宏窗口顶栏：46px / `#222` / 2px `#000` / 居中（`.nav-wrapper` + `.module-nav`） | [macro-app-chrome-audit.md](macro-app-chrome-audit.md) | `audit-macro-app-chrome.cjs` |

## 语言包键审计（2026-10-04）

`python tools/audit-locale-keys.py --check` 扫描 `src/` 中全部字面量 `t("KEY")` 调用并核对
`locales/` 的 10 份语言包：**409 个硬键、42 个 `t_or` 软键、42 个动态调用，缺失 0 个**
（`KNOWN_MISSING` 现为空表）。`t()` 未命中时 rust-i18n 会原样返回 key（见 `src/i18n.rs`
的 `has`），因此缺失数是这条链路的直接指标。历史缺口与其收口方式：

| 键 | 收口方式 |
| --- | --- |
| `ADVANCED_EFFECT_DETAILS` | 键名无误（设备包里就是 `oi("ADVANCED_EFFECT_DETAILS")`）；文本由 `tools/prepare-device-locales.py` 从设备包语言表合并进 10 份语言包 |
| `MINUTES` | 源码无此键；手柄省电取值改用共享组件 `GR` 的 `MIN`/`SEC` 模板（`>= 60` 才用 `MIN` 并除以 60） |
| `LINKED_GAMES_TO` | 源码无此键；改用 `LINKED_GAME_CHROMA_HEADER`＝"Games linked to profile:" |
| `MINIMUM` / `MAXIMUM` | 源码无此键；手柄扳机页改用 `<SIDE>_TRIGGER_RANGE`/`<SIDE>_ACTUATION_POINT` |
| `SWITCH_OFF_LIGHTING_WHEN_DISPLAY_IS_OFF` / `_WHEN_IDLE` | 改用 `DISPLAY_TURNED_OFF` / `IDLE_FOR_MIN` |

别名解析记录见 [设备页 CSS 逐条对账](device-page-css-audit.md)。

另有一类**不经过语言包的界面文案**，上面的审计看不见：产品页三族（鼠标、键盘、手柄）
已经在 2026-10-04 全部改为源码语言键（鼠标 40 余处、键盘 13 处），未知输入/效果原样显示
内部名而不编中文，并有测试防止回退；逐条键名依据见
[产品页文案的语言键对账](product-label-locale-audit.md)。仍带标签类中文字面量的模块（按数量降序）：
`device_pages.rs` 62（旧版设备页渲染器，仅 `system_button` 被复用，是否仍挂载待确认）、
`mapping_editor.rs` 45、`profile_transfer.rs` 45、`shortcuts.rs` 44、`onboard_memory.rs` 20、
`audio_page.rs` 17、`profile.rs` 17、`keyboard_controls.rs` 16、`linked_games.rs` 15、
`customize_page.rs` 14、`settings.rs` 12、`keyboard_calibration.rs` 10，以及若干「界面预览」
工具模块（`dock_pairing/preview.rs`、`hue/preview.rs` 等，不进应用 UI）。`surface::note(...)`、
Tooltip 与 `accessibility_label` 的中文是本项目自己的「服务未接入」提示，不是雷云文案。

另有 42 个 `t_or` 软键中 5 个不在语言包内（`AETHER_SERVICE_UNAVAILABLE`、`REFRESH_RATE`、
`SCARLETT_CONFIRM`、`SCARLETT_CONFIRMAION_TEXT`、`SCARLETT_CONFIRMATION_DESC`）：前两个走
自带兜底文案，后三个是原版自带的 `SCARLETT_*` 描述名（真实键是 `CONFIRM` 等，已按基础键取值）。
各语言包比英文少 61–118 个键，属既有提取差距，审计只做提示、不判失败。

## 资源准备更正（2026-10-04）

宏manifest声明的229个SVG和Profiles的工具栏/弹层SVG已按官方静态地址准备，资源清单共1029项并通过来源、格式、嵌入键校验，未执行下载JavaScript。宏palette的11个图标不再缺失。应用专用语言包正在按真正的加载链复核，不能继续以公共语言包里找不到同名键判断无译文。资源到位不等于UI已经实现。

2026-10-04 calibration follow-up: the introduction banner now persists the audited `showNotificationBannerCalibration` key, factory-default profile warning/disabled branch follows `isFactoryDefaultProfile`, and the explicit failure preview mirrors the source's 15-second idle close. The real calibration transport remains unavailable and its live Next action stays disabled.

2026-10-04 Macro follow-up: action parameter editors now cover Delay, Keyboard, Mouse, Loop, Text, Command, and Launch as local draft controls; event rows support source-shaped drag reorder and the 100px trailing drop area with undo/redo integration. Real recording, device transfer, XML import/export, complete folder menus, and independent binding remain service-boundary gaps. See [macro-current-source-review.md](macro-current-source-review.md).
