# 主前端：Dashboard、Gamer Room、设备与模块、全局快捷键

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](../re/20-current-source-version.md)为准。
> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 路由和证据

`[JS]` [App.72827d47.chunk.js](../../.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js) 的 HomePage 根据 active_view 切换页面；name 对应 [main.01550b17.js](../../.ref/applications/synapse/dashboard/static/js/main.01550b17.js) 的 locale 导出。

| 页面 | locale / 导出 | 模块 | 异步 chunk |
|---|---|---|---|
| Dashboard | DASHBOARD_HEADER / Rav | 73435 | 8302、1031、2973、4130 |
| Gamer Room | GAMER_ROOM_HEADER / BSg | 19388 | 8302、9388 |
| Devices & Modules | DEVICES_AND_MODULES_HEADER / iwS | 44442 | 6505 |
| Global Shortcuts | GLOBAL_SHORTCUT_HEADER / fUK | 94608 | 2973、7282 |

第三项不是只有固件更新，第四项已定位，不再标为“未知”。Settings 使用独立 `/synapse/settings/` 和窗口名 settings-synapse，不在四项内。

壳层布局见 [00-app-shell](00-app-shell.md)。HomePage 有 Dashboard 禁用遮罩分支（rgba(0,0,0,.5)、z-index 1000），异步加载不能伪装为可操作完整页面。

## 2. Dashboard

源：[7861.1b0e99a4.chunk.js](../../.ref/applications/synapse/dashboard/static/js/7861.1b0e99a4.chunk.js)、[55 公共 CSS](../../.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css)、[4130 CSS](../../.ref/applications/synapse/dashboard/static/css/7861.a49b4dc6.chunk.css)。

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

这是 Dashboard 图，不能用 Customize 的 prd 图名称替代。资源索引单独列出本地下载情况。教程视频 `Synapse Dashboard Tutorial.4d4e2f9c.mp4` 已从原地址取得并转换为无损动画 WebP，按原 250×190 比例内嵌、静音循环；尺寸、帧延时及源／输出哈希记录在 `tutorial-media-manifest.json`。播放器使用 GPUI 原生动画图像和按片段释放的缓存。

`[RUST 当前实现]` Dashboard 卡片和设备与模块行现在共用 `resources::dashboard_image(product_id, edition_id, layout_id)`，按产品、配色和键盘布局选择原 `PluginImages` 图。2026-10-02 原 URL 核实后覆盖 41 个映射、19 张去重图片；包括 182、653、777 的标准图及已确认的配色/键盘布局。键盘 `layoutId:0` 依原规则按布局 1 处理，未知非零布局或配色保持占位，不暗换其他配色，也不使用 Customize 的产品图。

### 静态入口、折叠与单步教程（2026-10-02）

[main_pages.rs](../../src/shell/main_pages.rs) 已恢复设备、模块、在线服务三个可折叠组。三个标题都由 Base Button 提供键盘激活和 `aria_expanded`，按原 `.collapse` 左对齐，避免 styled Button 内部居中的内容容器改变位置；每组 `margin:10px 0`，展开内容距标题 10，卡间距 20。折叠保留在当前 AppShell 生命周期中，尚未读取原服务的分组持久化或排序。设备名恢复 uppercase 和 10px 横向内边距；本地快照/预览标记保留，不宣称连接或安装状态。空列表采用原 `ee` 的 290×220 虚线卡，提供兼容设备列表和 Razer Store 原链接；标题保留“没有可显示的设备”，避免把未扫描的本地空列表称为硬件未发现。

模块静态项的证据是 `main` 模块 **22431** 的 `AVAILABLE_MODULES`，以及 4130 的 `Li.updateGroupModuleItems`：`iotModule`、`tourModule` 不依赖 installedModules，armory 则另受能力条件过滤。Wi-Fi 卡打开通用添加设备类型选择流程；Tour 卡对应 `X.showModules` / `focusTab` 的独立应用注册项，现打开或聚焦独立的五步 Introduction Tour 页签，详见[教程规格](16-introduction-tour.md)，不共用下述 Dashboard 教程的已读状态。未知安装状态的其他模块及 armory 不伪造为已安装。

四个在线服务来自 `Li.updateGroupServiceItems`，按 Razer Store、Gold & Silver、社区、支持的原顺序、图片、locale 描述和 URL 呈现。`de` 与 `.box-item-full-image` 使用 290×220 整卡、140px 图片、下方 10px 空隙和 14/12px 名称/描述；图片按原 contain 比例显示，描述允许换行，整卡是可键盘访问的外部 Link。

[dashboard_tutorial.rs](../../src/shell/main_pages/dashboard_tutorial.rs) 已实现 `Si` 的一个步骤。`DASHBOARD_TUTORIAL_HEADER` 原文就是 **Gamer Room Dashboard**，正文介绍在 Gamer Room 查看设备；它指向第二个主导航，而不是模块 Tour。导航带上的零尺寸 Base Popover 锚点恢复 `left = 第二导航左侧 − 92`，内容顶部按前端坐标 108（另加宿主 42px）、宽 290、padding 20、橙色 1px 边、圆角 5；36px 指示器中心为第二导航左侧 +50，前端顶部 70。标题 16px、正文 14px、关闭按钮 100×27、距正文 20，使用原 `CLOSE` key。媒体区内嵌原片段，短窗口下正文和操作可独立滚动，指示器保留在滚动层外。关闭按钮、Escape 完成本步骤；点外部不关闭，符合当前 `overlay_closable(false)` 策略，焦点恢复仍待真实窗口验证；独立 `dashboard_tutorial_seen` 参与本地串行辅助保存和设置页重置，设置页丢弃其他编辑不回滚已完成教程。

以上为源码、资源和类型检查范围内的对照；未运行真实窗口测量。推荐/合作伙伴、真实设备去重/状态、安装进度、拖动排序和原服务持久化仍需真实数据链路；Base Popover 的窗口边缘修正及多语言、缩放、焦点恢复仍需窗口验收。

## 3. Gamer Room

源：[9388.974b5d43.chunk.js](../../.ref/applications/synapse/dashboard/static/js/9388.974b5d43.chunk.js)。根 #gamerRoom，含条件 gr-banner、营销内容、教程入口、设备组和连接 popup。

状态来自 gamerRoomReducer 与 IoT/实际设备数据。group 会根据列数、设备过滤、全部组是否为空等更新 banner；不是固定一张横幅。设备卡有 skeleton/安装状态，popup 与目标容器 ID 关联，点击外部关闭；卸载应清理相关监听。

教程使用多步骤弹窗、视频、指示器、Skip/Next；关闭仅更新教程状态，不代表设备设置保存。原引用：

- `Gamer Room Dashboard Tutorial 1.080f80fb.mp4`
- `Gamer Room Dashboard Tutorial 2.b4e338ae.mp4`

两条原视频已从 `https://apps.razer.com/synapse/dashboard/static/media/` 取得，并转换为无损动画 WebP 在 GPUI 内嵌播放；保持原 250×190 比例、静音循环及源帧延时，详见[教程媒体说明](16-introduction-tour.md)。Aether 背景、灯泡/灯带/台灯图片、添加设备和手机应用/二维码资源已纳入嵌入表和来源清单。

`[RUST 当前实现]` [service_pages.rs](../../src/shell/service_pages.rs) 的 `GamerRoomPage` 已实现 531px 营销背景、三处热点与四种产品详情、原产品链接、Synapse 覆盖和 Gamer Room 应用控制两个可折叠组，以及两步教程的上一步/下一步/跳过/完成；完成事件与设置页的教程重置入口分开管理。2026-10-02 根据公共 CSS 修正居中的绿色 24px 标题、16px 副标题、底部外链和四段渐变遮罩；灯泡热点恢复 `left:10%;top:37%`，灯带与台灯保留原位置。热点支持鼠标预览和键盘点击详情，普通台灯与专业版都可从详情到达。没有 IoT 数据时显示分组说明和有效的添加入口，不生成设备卡。

教程保留两步状态，并使用 GPUI Base `PopoverState` 管理 Escape 和焦点生命周期；点外部不关闭。跳过、完成及 Escape 只标记教程已读，不改变设备设置。2026-10-02 沿当前 `19388 / He → je → ze → Pe` 重新核对后，面板直接按原相对 wrapper 的绝对坐标绘制，移除中间无源码依据的说明/重播行、窗口边缘吸附及内部高度压缩。正文滚动区负责窄/短窗口访问，重播仍可通过 Settings 重置教程。具体当前符号、字符偏移与包含块推导见[当前教程审计](../re/gamer-room-current-audit.md)。

教程保留原橙色强调，通过产品主题 token 取色。原热点 SVG 依赖 SMIL 椭圆半径动画，嵌入资源使用原动画采样后的静态帧；没有加入持续重绘计时器。两个空设备组按 `.gr-add-device` 恢复为 186×176 虚线说明卡，只有 Synapse 组显示 `ADD_NEW_DEVICE` 并允许添加；56px 添加图标属于准备弹窗，不放入空组卡。

添加流程的完整来源实际在 [IotPopupRoot.daaa97ef.chunk.js](../../.ref/applications/synapse/dashboard/static/js/IotPopupRoot.daaa97ef.chunk.js)，模块 **28256**。`gt` 按 query 的 `iotPopupType` 选择类型；Gamer Room 的 `ze.handleOpenAddModel` 传 `GAMER_ROOM_DEVICE`，因此进入 `dt → lt/ct`，不是通用类型选择或 Key Light 分支。当前已接入准备说明 → 手机应用/原二维码 → 返回，以及准备说明 → 设备搜索页面 → 返回/关闭；原兼容列表和帮助链接可访问，切步重置到保留的焦点容器。准备说明使用原 520px 内容宽度，二维码使用 600px 内容宽度和 `B` 的 `LIGHTING_DEVICE_MOBILE_QR_DOWNLOAD_APP` 文案。服务不可用页另外提供手机应用设置入口，返回保留搜索页来源，而不是一律退到准备页。

原 `dt` 的发现列表由 `ze/He` 调用 IoTNative 扫描与事件，选择已有网络设备后才进入 `at` 并写 `iot_devices`。本地未接通该 transport，搜索页面明确显示“设备搜索服务未连接”，不启动假计时器、不把未查询结果称为“没有设备”，也不启用识别/添加成功。`CHOOSE_NETWORK`、Wi-Fi 密码、短时切换网络等属于 `rt` 的 Key Light 流程，不能加到 Gamer Room 直接入口。设备卡内 `S/Q` 的电源、覆盖设置与原设备页入口同样需要真实 IoT 身份和状态。

### 弹出层样式核对（2026-10-02）

外层入口证据还包括 [App 的 `z` 组件](../../.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js)：它以 `iot-device-popup iot-device-popup__mt` 包裹 IotPopup iframe，不能只看 iframe 内的通用 Modal。下表列出源码几何和当前实现；这是静态核对，未运行窗口进行像素测量。

| 表面 | 原 JS/CSS 细节 | 当前 Rust |
|---|---|---|
| 添加设备外壳 | 顶部 106；`max-width:850`，`min-device-width:1331` 时 1280；底部贴窗口；`#222`、无边框、圆角 5、wrapper padding 0；全窗黑色 70% backdrop | `AddGamerRoom` 自有 retained entity + Base Dialog，按显示器宽度选断点，显式绘制尺寸和遮罩；不继承 Kit Dialog 的 1/10 顶偏、16px 边距或阴影 |
| 添加标题与关闭 | 标题栏高 36、居中、下边框 `#5d5d5d`；右上关闭区 36×36，白色图标 20×20 | 单独标题栏和原 `icon_close_white`；移除额外底部关闭按钮。原 backdrop 无关闭回调，因此点外不关闭；X/Escape 关闭并恢复打开前焦点 |
| 准备页 | 内容宽 520；图标距顶 30、56×56；说明距图 24；帮助链接上 6/下 22；手机操作条 420×60、40px 图/24px 箭头；第二说明距上 60；搜索按钮距上 27、居中 | 按各区域单独设置间距；恢复下划线外链和原箭头，避免统一 gap 或 outline 外链改变布局 |
| 二维码页 | 内容宽 600；应用图 32、顶部 10；二维码 110×110，下 20；返回居中、距上 30 | 保留真实二维码与下载 URL，扫码说明使用 `B` 的原 key；搜索页进入此处时返回原搜索页 |
| 两步教程 | `#111`、橙色 1px、圆角 5、总宽 290、padding `40px 20px 20px`；两步原锚点 `(220,22)` / `(213,-25)`；36px 指示图；按钮 100×27、间距 10 | `Pe` 的零高 wrapper 位于第一组10px上外距之前；`ie.boxGroup` 的30px被逗号表达式丢弃。以独立设备组容器为原点，不移动面板适应窗口、不固定高度；Base PopoverState 保留焦点/关闭生命周期 |
| 商品营销卡 | `.gr-display` 黑色 50%、圆角 5、min-height 312、min-width 400、padding `36px 24px`；台灯双卡间隔 36；`translateX(-23.5%/-86.5%)`、垂直居中 | Hover Base Popup 与键盘点击 Base Popover 共用同一营销内容和锚点；整张商品卡是外部链接，不再另开带重复标题/关闭 footer 的 Kit Dialog |

原营销卡的 `backdrop-filter:blur(30px)` 没有在当前 GPUI 层实现；静态热点没有 SMIL 动画；两步教程已内嵌原视频的无损动画版本，保持250×190比例，并随正文滚动。商品营销卡仍使用 Base Popup 的窗口边缘策略；教程已按当前源码保留组内坐标。字体基线、最终面板高度、窗口缩放、滚动与焦点回归仍需真实窗口验收。宽屏 Gamer Room 在标准化视口宽度达到2560时恢复2500×930横幅，壳层相应解除1260内容上限。

## 4. Devices & Modules

源：[6505.84205103.chunk.js](../../.ref/applications/synapse/dashboard/static/js/6505.84205103.chunk.js)、[6505 CSS](../../.ref/applications/synapse/dashboard/static/css/6505.9782778c.chunk.css)。

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
| 操作按钮 | `width:max-content`、min-width 90、高 27；12px 大写文字、左右 padding 16、圆角 2；普通底色 `#555`、整控件 hover/pressed opacity 0.8/0.6 |
| 展开详情 | #2d2d2d，padding 20 |
| 警告 | #fd8611；release notes 分类有不同标签色 |

固件更新是有状态的外部/设备操作，不能“点按钮立即显示最新版本”。

`[RUST 当前实现]` 设备行保留从当前工作区打开设备的操作，并提供设备快照详情：产品、序列号、当前设备/接收器固件和本地 Profile 数量。更新固件和移除操作因缺少真实安装/连接/目标版本状态保持禁用。

`ModuleCatalog` 按本 chunk 的 `ne/te` 建立 Alexa、宏、已关联的游戏、反馈、工坊目录，使用实际模块图标；宏/Alexa 的原说明图、中文描述和 Alexa 链接已接入。仅原 `ne` 中带 `detail` 的 Alexa/宏显示展开入口并保留状态，其他模块不伪造说明面板。说明图恢复 `.item-description-image` 的 288×162，说明列为 592px，项目名保持 500px 列宽。目录不是安装结果：安装状态、包大小、版本和可用更新均明确尚未读取，安装按钮禁用，不虚构已安装/可更新/卸载中分组；尤其不把静态目录的 `size:0` 格式化为真实安装包 `< 1 MB`。原 `O` 的卸载确认与清除设置勾选、`L` 的固件 release notes 和 `w` 的进度/重试依赖真实服务项目，尚未获得这些数据。设备行同时区分本地快照和预览数据。

模块说明是行内 `.item-moreInfo`，不是弹窗：背景 `#2d2d2d`、padding 20、最小高 202、图片右侧间隔 20，外链恢复为下划线文字。本项目另有设备快照详情，原 6505 没有可对应的同类信息 Dialog；它只沿用通用 `.modal-content` 的 `#111`、绿色 1px 边框、5px 圆角、400px 宽、`20px 30px` 内边距和居中按钮，数据行属于本地补充界面。该弹窗改为 `ModuleCatalog` 持有的 Base Dialog entity，并保留关闭/焦点恢复；不把它称为原固件或卸载流程。

2026-10-02 对按钮继续核对完整 CSS 级联。操作按钮的最终 `.btn` 来自 `55` 末段，不能只取较早的 100px / 3px 默认样式；6505 再覆盖最小宽度、实际文字宽度和内边距。[service_pages.rs](../../src/shell/service_pages.rs) 用 Base Button 的直接文字子项测量，保留原大写转换，避免 Component Button 内层满宽文字槽影响宽度。目录、设备快照行和模块预览共用该控件；整个按钮按原 `200ms ease-out` 在常态、hover、按下与禁用透明度之间过渡，禁用行为立即生效。详情与移除入口按文字宽度呈现，不再占一个默认按钮宽度。

Gamer Room 热点按 14px / 17px 文字和四周 8px 内边距确定宽度，添加卡保留 186×176 边框尺寸；产品详情标题恢复 18px RazerF5 bold/uppercase，说明为 14px / 17px。[service_button_tests.rs](../../src/shell/service_button_tests.rs) 留有真实文字测量、两种缩放、禁用及键鼠激活的回归源码；本轮只做静态检查与编译检查，尚未执行这些测试或进行实际窗口像素验收。

## 5. Global Shortcuts

源：[4608.e973916f.chunk.js](../../.ref/applications/synapse/dashboard/static/js/4608.e973916f.chunk.js)，模块 94608 导出 GlobalShortcutsContainer。

`Fe` 组织 custom-global-shortcuts 容器、左列快捷键内容、保存提示和映射编辑器。`Oe` 创建 global_shortcuts_container：说明、添加图标、快捷键列表、底部 Add 卡。选中/编辑某项时，添加入口受限。快捷键卡支持编辑和删除确认。

数据包含 guid、inputID、inputModifiers、isHyperShift 和输出 mapping。初始化系统键盘布局、默认 turbos，从本地 synapseGlobalShortcuts 数据加载，并通过 generateAppEngineMappings 生成引擎映射和 hash。宏、Chroma profile、设备变动会触发有效性检查和显示更新；失去焦点/隐藏时调整按键监听。

保存/不保存/关闭有独立路径；宏和 Chroma 功能受模块安装状态影响。不能用“按键文本 + 动作文本”的无状态列表覆盖捕获、草稿、校验、删除和引擎同步。

共有映射编辑器的所有能力不代表全局快捷键都支持；应依 isGlobalShortcut / displayMode 分支裁剪。

`[RUST 当前实现]` [shortcuts.rs](../../src/features/shortcuts.rs) 已提供独立快捷键实体：顶部和底部添加入口、列表编辑/复制、删除确认、292px 编辑器、录制键盘组合、选择四种鼠标输入、Ctrl/Alt/Shift/Win 与 Hypershift 修饰。输出包含启动程序、打开网站、多媒体、Windows 快捷方式和 1–250 个 UTF-16 单元文本；程序使用本机文件选择，网站保存时补全并校验 HTTP(S)，重复输入组合和非法草稿不能保存。录制在 Escape 或失焦时取消，文件选择返回时检查仍在编辑原快捷键。

关闭编辑器和主应用导航均有保存/丢弃/继续编辑；快捷键加入本地 WorkspaceFile，与设备配置共用异步“保存到本机”，后续编辑不会被较早的保存完成事件清除。删除可在保存前通过丢弃恢复。以上是本地编辑能力，宏和 Chroma 仍需真实服务。

[shortcut_engine.rs](../../src/features/shortcut_engine.rs) 已按原 `generateAppEngineMappings` 和后续 reducer 生成输入/输出、DKM 别名、设备变体及稳定 JSON/MD5 hash。每次整表编码，未支持输出、碰撞输入或无法安全编码的目标使整次失败，不部分生成。部分 Windows 操作依赖原生 Turbo GUID 与已注册事件，当前明确报错；已实现输出和逐项证据见[引擎编码审计](../re/12-global-shortcut-encoding.md)。

已证实的宿主 ABI 尚不能读取现有 `synapseGlobalShortcuts`/引擎配置，而原写入是替换整份配置。运行时“提交快捷键”入口因此保持禁用，不以本地列表覆盖未知原配置。编码完成不代表快捷键已注册；后台连接及服务边界见[运行时接入](../re/10-runtime-integration.md)。

## 6. Settings 独立设置

已取得 `/synapse/settings/` 的完整 render 证据，详见[独立设置规格](12-settings.md)。720 chunk 的 `ho/uo` 分别渲染 Synapse 与 General；当前已接入通知、推荐、教程、语言、关于区域及本地偏好保存。启动、迁移及灯光控制权需要原服务，界面明确其未读取或未连接状态。旧 `setting.rs` 不在当前编译入口内，不作为界面覆盖依据。

[runtime_page.rs](../../src/shell/runtime_page.rs) 已挂载“服务连接”面板：初始状态不启动服务，点击“连接并读取”后才由后台线程持有独立 worker，依次请求 HID 接口、服务版本和音频设备。每项结果与错误独立显示，支持刷新、展开读取详情和断开；断开/窗口退出的原生关闭和子进程等待留在后台，最近结果继续显示。HID 内容按接口呈现，包含 USB 标识、产品及序列号等元数据，同一物理设备可有多项；不把它们直接添加为 Dashboard 卡或设备工作区。版本与音频查询目前展示服务信息，未据此修改 Help 或产品 EQ。全部路径只做静态检查，本轮没有运行连接、DLL 或 HID API。

manifest 的 background_color=#ffffff 是 PWA 元信息，不代表应用内白底；运行布局仍以实际 CSS #222 为准。

## 7. 当前 Rust 入口与剩余差异

2026-10-01 已核对 [shell.rs](../../src/shell.rs) 与 [main_pages.rs](../../src/shell/main_pages.rs) 的实际编译入口。主应用四个路由可以分别切换，Settings 从右侧设置入口进入，不再追加进产品导航。旧 [dashboard.rs](../../src/features/dashboard.rs) 未被当前 features 模块声明，不再作为运行界面的判断依据。

| 页面 | 当前已落实的代码 | 尚未接入 |
|---|---|---|
| Dashboard | 290px 整卡打开设备、250×140 图区、20px 间距；设备/模块/在线服务折叠；空列表原双链接；Wi-Fi 与独立 Tour 边界、四个在线服务原卡；单步教程与独立本地已读状态；区分快照/预览 | 拖动排序、推荐/合作伙伴、安装服务分组、真实接口合并与设备状态、原分组持久化；设置中的 HID 元数据查询不生成设备卡；未覆盖的专用 Dashboard 图保留占位 |
| Devices & Modules | 80px 设备行、40px 图区、打开和快照详情；原五模块目录、图标、说明图、详情展开/收起与相关链接 | 真实安装/版本/更新清单、固件检查/升级、卸载/清除设置确认、进度和失败恢复 |
| Gamer Room | 原营销背景/热点、四产品详情、两个折叠组；添加准备/二维码/搜索页面的返回与关闭；两步教程和原片段内嵌播放 | IoT 网络发现、识别/添加、真实设备卡及电源/覆盖设置 |
| Global Shortcuts | 600px 内容区、顶部添加与 70px 添加卡；捕获/验证、编辑/复制/删除确认、292px 映射面板、保存/丢弃/继续、本地持久化；原引擎映射与 hash 编码 | 原快捷键读取协议未证实，整表替换提交禁用；宏/Chroma、原生 Turbo 事件及注册回执仍未接入 |

资源显示按实际用途区分：Dashboard 走专用产品卡资源，Customize 与设备设置页根据设备产品、edition、layout 动态选择其产品图；缺少一种 Dashboard 图不会用 Customize 的 `prd` 图充当同一资源。未连接的服务不显示安装或连接成功。

全部 **14 个普通页面实例**，以及 HELP、独立 Pairing、Profile/板载/关联游戏、抽屉、映射编辑与教程的覆盖记录见[全部页面与附属界面](../re/07-page-coverage.md)。后续验证继续覆盖动态设备增减/去重、卡片目标、安装/升级/失败、分组排序/缩放、快捷键捕获和失焦、草稿保护及资源缺失状态。

当前保留[快捷键交互](../../src/features/shortcuts_tests.rs)与[引擎编码](../../src/features/shortcut_engine_tests.rs)回归源码。本轮只做 `cargo check`；未运行测试、应用、DLL 或原生注册，也未启动快捷键目标程序。

2026-10-02：Dashboard 三个分组改用 [dashboard_group.rs](../../src/shell/main_pages/dashboard_group.rs)。原卡片持续挂载，max-height 300ms ease-in 与 translateY／箭头 300ms linear 分别采样，沿当前值反转；overflow 使用 1s ease 离散关键帧。拖动排序的 show-overflow 分支仍未接入。
