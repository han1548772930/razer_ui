# 主前端：Dashboard、Gamer Room、设备与模块、全局快捷键

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 路由和证据

`[JS]` [App.eb32d7cd.chunk.js](../../.ref/frontend/static/js/App.eb32d7cd.chunk.js) 的 HomePage 根据 active_view 切换页面；name 对应 [main.7897a4cf.js](../../.ref/frontend/static/js/main.7897a4cf.js) 的 locale 导出。

| 页面 | locale / 导出 | 模块 | 异步 chunk |
|---|---|---|---|
| Dashboard | DASHBOARD_HEADER / Rav | 73435 | 8302、1031、2973、4130 |
| Gamer Room | GAMER_ROOM_HEADER / BSg | 19388 | 8302、9388 |
| Devices & Modules | DEVICES_AND_MODULES_HEADER / iwS | 44442 | 6505 |
| Global Shortcuts | GLOBAL_SHORTCUT_HEADER / fUK | 94608 | 2973、7282 |

第三项不是只有固件更新，第四项已定位，不再标为“未知”。Settings 使用独立 `/synapse/settings/` 和窗口名 settings-synapse，不在四项内。

壳层布局见 [00-app-shell](00-app-shell.md)。HomePage 有 Dashboard 禁用遮罩分支（rgba(0,0,0,.5)、z-index 1000），异步加载不能伪装为可操作完整页面。

## 2. Dashboard

源：[4130.155387bf.chunk.js](../../.ref/frontend/static/js/4130.155387bf.chunk.js)、[55 公共 CSS](../../.ref/frontend/static/css/55.a5b041a2.chunk.css)、[4130 CSS](../../.ref/frontend/static/css/4130.6bdf8dd0.chunk.css)。

```text
dashboard [.reflow]
└─ box-group*
   ├─ backdrop-box
   ├─ title：collapse、drag-div / drag-icon
   └─ content / content-inner
      └─ 设备卡、模块卡、推荐/服务卡
```

数据源包括 validDevices、devices、installedDevices、installedModules、推荐、新发布产品、合作伙伴、分组顺序、卡片顺序和折叠状态。原代码会按 productId、serialNumber、容器/连接状态去重，不应只按类别生成一张卡。

### 布局基线

| 区域 | 原值 |
|---|---|
| dashboard | margin:0 auto，max-width:1220，min-width:620，纵向 |
| reflow | max-width:2460，仅宽屏分支 |
| 分组 | width:100%，margin:10px 0，min-height:18 |
| 卡片排列 | flex-wrap，gap:20 |
| 计算列数 | boxWidth=290、xOffset=310；ResizeObserver 触发重算 |
| 通用卡 | #111、圆角 5、padding 10px 20px 9px |
| 普通产品图区域 | 250×140、contain；商店分支可为高 120 |
| 分组标题 | 14px #ccc，折叠图标 10，拖动图标约 22×19 |
| 设备名 / edition | 14px/16px；edition 12px/14px #707070 |

分组展开使用 max-height/transform，拖动时有 #3333334d 背景和 2px 绿色边。电池位于卡片状态区域，低电量有专用图标；禁用卡同时降低透明度并阻止输入。加载 spinner、安装进度、失败重试、离线状态与可点击入口分别处理。

分组 ID 包括 devices、recommendation、module、onlineService、partnerDeals，以及 syn2、window10、inDevelopment、noDevice、xboxHeadset、xboxController 等特殊状态。是否显示来自实际数据和过滤条件，不应永远展示空组。

### 资源

设备卡使用动态产品 URL，常见形式：

```text
/synapse/products/{productId}/ui/{productId}_{editionId}/PluginImages/
{productId}_{editionId}_{layoutId}_dashboard1x.png
```

这是 Dashboard 图，不能用 Customize 的 prd 图名称替代。资源索引单独列出本地下载情况。教程视频 `Synapse Dashboard Tutorial.4d4e2f9c.mp4` 当前缺失；有引用不表示视频能播放。

## 3. Gamer Room

源：[9388.2bec5db3.chunk.js](../../.ref/frontend/static/js/9388.2bec5db3.chunk.js)。根 #gamerRoom，含条件 gr-banner、营销内容、教程入口、设备组和连接 popup。

状态来自 gamerRoomReducer 与 IoT/实际设备数据。group 会根据列数、设备过滤、全部组是否为空等更新 banner；不是固定一张横幅。设备卡有 skeleton/安装状态，popup 与目标容器 ID 关联，点击外部关闭；卸载应清理相关监听。

教程使用多步骤弹窗、视频、指示器、Skip/Next；关闭仅更新教程状态，不代表设备设置保存。原引用：

- `Gamer Room Dashboard Tutorial 1.080f80fb.mp4`
- `Gamer Room Dashboard Tutorial 2.b4e338ae.mp4`

两条原视频的 `https://apps.razer.com/synapse/dashboard/static/media/` 地址均已只读核查返回 200 和 `video/mp4`，当前提供明确的“播放原教程视频”外链，未在 GPUI 中内嵌播放器。Aether 背景、灯泡/灯带/台灯图片、添加设备和手机应用/二维码资源已从原构建路径取得；普通台灯图来自 9388 模块 71610 的内嵌 PNG。资源由统一生成器纳入嵌入表和清单。

`[RUST 当前实现]` [service_pages.rs](../../src/shell/service_pages.rs) 的 `GamerRoomPage` 已实现 531px 营销背景、三处热点与四种产品详情、原产品链接、Synapse 覆盖和 Gamer Room 应用控制两个可折叠组，以及两步教程的上一步/下一步/跳过/完成；完成事件与设置页的教程重置入口分开管理。热点支持鼠标预览和键盘点击详情，普通台灯与专业版都可从详情到达。没有 IoT 数据时显示分组说明和有效的添加入口，不生成设备卡。

添加流程的完整来源实际在 [IotPopupRoot.290be417.chunk.js](../../.ref/frontend/static/js/IotPopupRoot.290be417.chunk.js)，模块 **28256**。`gt` 按 query 的 `iotPopupType` 选择类型；Gamer Room 的 `ze.handleOpenAddModel` 传 `GAMER_ROOM_DEVICE`，因此进入 `dt → lt/ct`，不是通用类型选择或 Key Light 分支。当前已接入准备说明 → 手机应用/原二维码 → 返回，以及准备说明 → 设备搜索页面 → 返回/关闭；原兼容列表和帮助链接可访问，切步重置到保留的焦点容器。

原 `dt` 的发现列表由 `ze/He` 调用 IoTNative 扫描与事件，选择已有网络设备后才进入 `at` 并写 `iot_devices`。本地未接通该 transport，搜索页面明确显示“设备搜索服务未连接”，不启动假计时器、不把未查询结果称为“没有设备”，也不启用识别/添加成功。`CHOOSE_NETWORK`、Wi-Fi 密码、短时切换网络等属于 `rt` 的 Key Light 流程，不能加到 Gamer Room 直接入口。设备卡内 `S/Q` 的电源、覆盖设置与原设备页入口同样需要真实 IoT 身份和状态。

## 4. Devices & Modules

源：[6505.93b828df.chunk.js](../../.ref/frontend/static/js/6505.93b828df.chunk.js)、[6505 CSS](../../.ref/frontend/static/css/6505.9782778c.chunk.css)。

模块读取 installedDevices、installedModules、availableModules、connectedDevices、deviceRuntimeData、deviceManifest、cachedDeviceInfo、uninstallingDevices/Modules、firmwareUpdateDevices、installerStatus。

实际 return 依次包含：

1. 固件可更新组（hasFWUpdate）。
2. 新设备组（showDescription、installerStatus）。
3. 可用模块组（showDescription）。
4. 已安装/卸载中等项目合并组（noProgressBar）。

内部虽计算 hasUpdateItems，本地这段 return 中还有显式 null；不能按变量名补出额外一定显示的组。

设备项按 productId、序列号、容器和缓存匹配；是否 connected、removable、同产品另有已连接实例，以及 IoT 的 isPowerOn/isLocked/isOnline 都影响操作。安装进度含 downloadedPercent、installedFiles、phase、totalFiles。模块名、包大小和描述来自清单，不是硬编码产品知识。

固件项区分设备/接收器、当前版本/目标版本、支持的更新连接方式。升级按钮只有条件满足才启用；外部 guide 打开系统链接，release notes 可展开。

| 视觉区域 | 基线 |
|---|---|
| .items | 宽 1220，纵向，居中，底部 40 |
| header-title | RazerF5、24px、#44d62c、uppercase |
| item-main-content | 高 80，#111，padding 0 30px 0 20px |
| 图标 | 40×40 |
| 项目名 | 16px #ccc，flex 基础 500，过长省略 |
| 操作按钮 | min-width 90，高 27 |
| 展开详情 | #2d2d2d，padding 20 |
| 警告 | #fd8611；release notes 分类有不同标签色 |

固件更新是有状态的外部/设备操作，不能“点按钮立即显示最新版本”。

`[RUST 当前实现]` 设备行保留从当前工作区打开设备的操作，并提供设备快照详情：产品、序列号、当前设备/接收器固件和本地 Profile 数量。更新固件和移除操作因缺少真实安装/连接/目标版本状态保持禁用。

`ModuleCatalog` 按本 chunk 的 `ne/te` 建立宏、Alexa、已关联的游戏、反馈、工坊目录，使用实际模块图标；宏/Alexa 的原说明图、中文描述和 Alexa 链接已接入。每行可展开/收起详情并保留状态。目录不是安装结果：安装状态、包大小、版本和可用更新均明确尚未读取，安装按钮禁用，不虚构已安装/可更新/卸载中分组。原 `O` 的卸载确认与清除设置勾选、`L` 的固件 release notes 和 `w` 的进度/重试依赖真实服务项目，尚未获得这些数据。

## 5. Global Shortcuts

源：[7282.873c10ab.chunk.js](../../.ref/frontend/static/js/7282.873c10ab.chunk.js)，模块 94608 导出 GlobalShortcutsContainer。

`Fe` 组织 custom-global-shortcuts 容器、左列快捷键内容、保存提示和映射编辑器。`Oe` 创建 global_shortcuts_container：说明、添加图标、快捷键列表、底部 Add 卡。选中/编辑某项时，添加入口受限。快捷键卡支持编辑和删除确认。

数据包含 guid、inputID、inputModifiers、isHyperShift 和输出 mapping。初始化系统键盘布局、默认 turbos，从本地 synapseGlobalShortcuts 数据加载，并通过 generateAppEngineMappings 生成引擎映射和 hash。宏、Chroma profile、设备变动会触发有效性检查和显示更新；失去焦点/隐藏时调整按键监听。

保存/不保存/关闭有独立路径；宏和 Chroma 功能受模块安装状态影响。不能用“按键文本 + 动作文本”的无状态列表覆盖捕获、草稿、校验、删除和引擎同步。

共有映射编辑器的所有能力不代表全局快捷键都支持；应依 isGlobalShortcut / displayMode 分支裁剪。

`[RUST 当前实现]` [shortcuts.rs](../../src/features/shortcuts.rs) 已提供独立快捷键实体：顶部和底部添加入口、列表编辑/复制、删除确认、292px 编辑器、录制键盘组合、选择四种鼠标输入、Ctrl/Alt/Shift/Win 与 Hypershift 修饰。输出包含启动程序、打开网站、多媒体、Windows 快捷方式和 1–250 个 UTF-16 单元文本；程序使用本机文件选择，网站保存时补全并校验 HTTP(S)，重复输入组合和非法草稿不能保存。录制在 Escape 或失焦时取消，文件选择返回时检查仍在编辑原快捷键。

关闭编辑器和主应用导航均有保存/丢弃/继续编辑；快捷键加入本地 WorkspaceFile，与设备配置共用异步“保存到本机”，后续编辑不会被较早的保存完成事件清除。删除可在保存前通过丢弃恢复。以上是本地编辑能力，宏和 Chroma 仍需真实服务。

[shortcut_engine.rs](../../src/features/shortcut_engine.rs) 已按原 `generateAppEngineMappings` 和后续 reducer 生成输入/输出、DKM 别名、设备变体及稳定 JSON/MD5 hash。每次整表编码，未支持输出、碰撞输入或无法安全编码的目标使整次失败，不部分生成。部分 Windows 操作依赖原生 Turbo GUID 与已注册事件，当前明确报错；已实现输出和逐项证据见[引擎编码审计](../re/12-global-shortcut-encoding.md)。

已证实的宿主 ABI 尚不能读取现有 `synapseGlobalShortcuts`/引擎配置，而原写入是替换整份配置。运行时“提交快捷键”入口因此保持禁用，不以本地列表覆盖未知原配置。编码完成不代表快捷键已注册；后台连接及服务边界见[运行时接入](../re/10-runtime-integration.md)。

## 6. Settings 证据边界

宿主 constants / app 路由证明设置应用存在；公共 CSS 中也有 .main-setting、.side-navigation、.setting-content。此前“完全不存在这些样式”的说法不准确。

但本地未取得独立 `/synapse/settings/` 的完整渲染代码，因此 CSS 只能作为样式线索，不能证明账户、启动、通知、迁移等全部选项的顺序/处理。当前 [shell.rs](../../src/shell.rs) 中的设置内容属于本项目实现，不能反过来当原版证据；旧 `setting.rs` 不在当前编译入口中。

[runtime_page.rs](../../src/shell/runtime_page.rs) 已挂载“服务连接”面板：初始状态不启动服务，点击“连接并读取”后才由后台线程持有独立 worker，依次请求 HID 接口、服务版本和音频设备。每项结果与错误独立显示，支持刷新、展开读取详情和断开；断开/窗口退出的原生关闭和子进程等待留在后台，最近结果继续显示。HID 内容按接口呈现，包含 USB 标识、产品及序列号等元数据，同一物理设备可有多项；不把它们直接添加为 Dashboard 卡或设备工作区。版本与音频查询目前展示服务信息，未据此修改 Help 或产品 EQ。全部路径只做静态检查，本轮没有运行连接、DLL 或 HID API。

manifest 的 background_color=#ffffff 是 PWA 元信息，不代表应用内白底；运行布局仍以实际 CSS #222 为准。

## 7. 当前 Rust 入口与剩余差异

2026-10-01 已核对 [shell.rs](../../src/shell.rs) 与 [main_pages.rs](../../src/shell/main_pages.rs) 的实际编译入口。主应用四个路由可以分别切换，Settings 从右侧设置入口进入，不再追加进产品导航。旧 [dashboard.rs](../../src/features/dashboard.rs) 未被当前 features 模块声明，不再作为运行界面的判断依据。

| 页面 | 当前已落实的代码 | 尚未接入 |
|---|---|---|
| Dashboard | 290px 整卡打开设备、250×140 图区、20px 间距、设备分组折叠；区分本地快照/预览 | 服务分组、拖动排序、教程、真实接口合并与设备状态；设置中的 HID 元数据查询不生成设备卡；专用 Dashboard 图不完整时保留占位 |
| Devices & Modules | 80px 设备行、40px 图区、打开和快照详情；原五模块目录、图标、说明图、详情展开/收起与相关链接 | 真实安装/版本/更新清单、固件检查/升级、卸载/清除设置确认、进度和失败恢复 |
| Gamer Room | 原营销背景/热点、四产品详情、两个折叠组；添加准备/二维码/搜索页面的返回与关闭；两步教程和原视频外链 | IoT 网络发现、识别/添加、真实设备卡及电源/覆盖设置；内嵌教程播放器 |
| Global Shortcuts | 600px 内容区、顶部添加与 70px 添加卡；捕获/验证、编辑/复制/删除确认、292px 映射面板、保存/丢弃/继续、本地持久化；原引擎映射与 hash 编码 | 原快捷键读取协议未证实，整表替换提交禁用；宏/Chroma、原生 Turbo 事件及注册回执仍未接入 |

资源显示按实际用途区分：Dashboard 走专用产品卡资源，Customize 与设备设置页根据设备产品、edition、layout 动态选择其产品图；缺少一种 Dashboard 图不会用 Customize 的 `prd` 图充当同一资源。未连接的服务不显示安装或连接成功。

全部 **14 个普通页面实例**，以及 HELP、独立 Pairing、Profile/板载/关联游戏、抽屉、映射编辑与教程的覆盖记录见[全部页面与附属界面](../re/07-page-coverage.md)。后续验证继续覆盖动态设备增减/去重、卡片目标、安装/升级/失败、分组排序/缩放、快捷键捕获和失焦、草稿保护及资源缺失状态。

当前保留[快捷键交互](../../src/features/shortcuts_tests.rs)与[引擎编码](../../src/features/shortcut_engine_tests.rs)回归源码。本轮只做 `cargo check`；未运行测试、应用、DLL 或原生注册，也未启动快捷键目标程序。
