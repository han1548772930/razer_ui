# 尚未完成的界面与产品

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
| 3592 / 3594 / 3595 / 3596 Kiyo | Framing controls, resolution, watermark tray, LDC/resolution disable branch, white-box drag, directional assets, and stepper timing are integrated. Live stream and device enumeration retain an explicit source-sized unavailable boundary; third-party branches, missing directional assets, and six image steppers remain open. See camera-framing-current-audit.md and camera-preview-placeholder-audit.md. |
| 3587 / 3589 / 3590 Kiyo | 共享 Customize 布局已补 520×292 相机预览外框和明确不可用边界；实时流、设备枚举、取景与叠加层仍未实现 |
| 1392 / 1442 / 3942 audio demo products | Source poster, dimensions, floating preference, and the source-sized control bar are restored. The progress/volume sliders retain local preview values, while native audio playback, live timing, floating video and complex mappings remain open; clicking reports the service boundary. See audio-demo-playback-boundary.md. |
| 164 / 241 Mouse Dock | dongle 713 额外确认、部分配对状态动画、弹层 viewport / 滚动几何等价 |
| 778 ASRock B550 / 3871 Chroma ARGB | 部分 300ms hover、自动检测点击短动画、步进器长按重复 |
| 3884 / 3886 无线 ARGB | 原版检测动画、hover 过渡、tooltip 时序；3886 原代码端口分支不可达，不能把样例编辑器算成原版普通页 |
| 784 Aether Light Strip | 轮播平滑居中、Identify 延迟、部分 tooltip 触发/定位和底部提示定位 |
| 769 Philips Hue | 发现、连接、设备状态的完整交互和精确视觉验证；已接入主体与引导不等于全部完成 |
| 3946 自动化 / Base Station V3 | 完整宏录制器、游戏浏览与关联、快捷键子编辑器；每种灯效参数面板；删除确认锚定、原始图标槽位及过渡动画 |
| 原有十个适配器 | 182、653、777、3072、3073、3074、3076、3077、3078、3080 的 27 个主页面已逐页复核（`partial_native_reaudited`，带本地路由与源码依据），见 [逐页复核](legacy-adapter-page-reaudit.md)；仍未进入 `source_help` 描述符、182 规格条目 `pages` 为空、653/777 仍走各自手写页面，仍是部分实现 |
| 设备页顶栏电量 | 已按当前源码接入（状态机 `off`/`Charging`/`charging100`/`NoCharge_BatteryFull`/`batt-warning`/`ReachChargingLimit`，图标与文案逐条取证，见 [电量依据](battery-indicator-audit.md)）；原版的 `hideBattValue` 与耳机左右耳电量因本地无数据来源未实现 |
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

## 资源准备更正（2026-10-04）

宏manifest声明的229个SVG和Profiles的工具栏/弹层SVG已按官方静态地址准备，资源清单共1029项并通过来源、格式、嵌入键校验，未执行下载JavaScript。宏palette的11个图标不再缺失。应用专用语言包正在按真正的加载链复核，不能继续以公共语言包里找不到同名键判断无译文。资源到位不等于UI已经实现。

2026-10-04 calibration follow-up: the introduction banner now persists the audited `showNotificationBannerCalibration` key, factory-default profile warning/disabled branch follows `isFactoryDefaultProfile`, and the explicit failure preview mirrors the source's 15-second idle close. The real calibration transport remains unavailable and its live Next action stays disabled.

2026-10-04 Macro follow-up: action parameter editors now cover Delay, Keyboard, Mouse, Loop, Text, Command, and Launch as local draft controls; event rows support source-shaped drag reorder and the 100px trailing drop area with undo/redo integration. Real recording, device transfer, XML import/export, complete folder menus, and independent binding remain service-boundary gaps. See [macro-current-source-review.md](macro-current-source-review.md).
