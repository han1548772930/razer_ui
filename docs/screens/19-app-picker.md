# 更多应用弹层

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](../re/20-current-source-version.md)为准。
本文记录已取得的原版源码，供本地界面对照。取得文件不代表应用已安装，也不代表原生服务已接通。

## 来源

- 外层工具栏：`.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js`，模块 `96776`；CSS `55.4e8559cb.chunk.css`。
- iframe：`https://apps.razer.com/rz-app-menu/`，原始响应和 SHA-256 见 `.ref/applications/rz-app-menu/source.json` 与各文件的 `.http.json`。
- 独立应用：`static/js/main.83ced465.js`、`Root.fddb4c8a.chunk.js`、`AppFolderInstallationChecker.34ed98f2.chunk.js`、`static/css/main.3f2a3e21.css`。均仅按文本读取，未执行。
- 页面和清单中的 11 个 JS/CSS 资源已取得；媒体和运行时组成的路径另行追踪。

## 工具栏和关闭行为

工具栏从左至右在可选 OLED/配置迁移入口之后放“更多”，再放设置和账户。更多按钮占 46×38，20×20 的 `icon_app.b648a5e5.svg`，hover 背景 `#2d2d2d`；tooltip 键为 `MORE`。有未读功能时图标带绿点，点击会清除 `hasUnread`。

点击切换 iframe 显示。iframe 定位 `top:40; right:0; width:410px`，无边框；高度为内部 `.app-explorer-popup.scrollHeight + 5` 与 `window.innerHeight - 80` 的较小值，在打开、窗口改变和设备行数变化时重算。iframe 内根节点始终带 `show`，外层直接切换 display/opacity/pointer-events，因此不能把公共 CSS 的淡入误当成每次打开都播放的动画。

外部 mousedown 关闭；外层窗口失焦时，焦点若仍在按钮或 iframe 内则保留，否则关闭。子页面 `iframe-ready` 后，宿主发送 `popUpData`，包含 `validDevices/currentApp/storeInstalledModules`；子页面 `close-popup` 关闭，`add-wifi-device` 打开全局添加 Wi-Fi 设备弹层后关闭，`update-height` 触发重算。源码未给这部分单独的 Escape/按键处理；GPUI 实现仍须遵循 Kit 的键盘关闭、触发器焦点恢复和按钮激活约定。

## 内容与尺寸

顺序为设备、模块、其他已安装应用、推荐应用、发现更多链接。设备组仅在 Synapse 显示；空组不渲染；有 currentApp 才计算推荐。设备、模块、其他应用均为空时，发现更多链接上方显示 150 高的 `discover-more-razer-apps-2x.2f6a6bb1.avif`，全宽 cover、底部间距 10。

独立 CSS 的开头有不依赖 `.app-explorer` 祖先的规则。后面的同名公共工具栏规则要求该祖先，不能据此把内部弹层再次偏移 40px。

| 部分 | 原版规则 |
| --- | --- |
| 弹层 | 410 宽；黑底；1px `#5d5d5d` 边框；无圆角；内容超高滚动 |
| 分组容器 | 水平 padding 15；分组垂直 padding 10，底边 1px `#5d5d5d` |
| 分组标题 | 12px、`#999`、大写、继承行高、底部间距 10 |
| 网格 | flex wrap；item `flex:0 0 33%`、高 96；按下 opacity .5 |
| 项内部 | padding 10px 3px、圆角 5；hover `#2d2d2d`、提升绘制层级 |
| 图标 | 40×40、contain、居中、底部间距 5 |
| 标题 | Roboto 14px、行高 1.2、`#ccc`、宽 110、居中 |
| 长标题 | 以 `normal 14px Roboto` 测量，超过 110 宽时两行截断、高 33；hover 高度自动并显示完整内容 |
| New 标记 | 白底黑字、黑边 2、圆角 10、10px 字、padding 2px 6px、right22/top−12 |
| 页脚 | padding 10px 15px 20px；链接 14px 灰字下划线、hover 绿、按下 .7；颜色/opacity .3s |
| 外链图标 | 20×20、左边距 5，随链接变色；目标 `https://razer.com/pc/software` |

CSS 存在 `.device-item{height:100px}`，但本次 Root 实际项类名为 `item` 加可选 `device-item-elipsis`，不能据此把所有设备项设为 100 高。设备标题使用当前语言、语言小写、en 的回退顺序；模块标题转英语首字母大写并保留 `HD`。

## 显示条件与排序

Synapse 模块须出现在 `installedModules` 键中，且不在 `uninstallingModules` 中；Armory 另须有 `installedVersion`。Chroma 模块须在 `synapseInstalledModules` 列表中，顶层 `/chroma-app/dashboard/` 使用传入 `storeInstalledModules`。`group-item-order.module` 以去连字符、小写后的名字匹配；有匹配项时使用匹配后的子集，否则使用默认顺序。

Synapse 设备须 `setupStatus == ready`，排除 container `philips-hue`、`deviceInitStatusFail == mixer_system_check_failed`、`powerStatus.chargingStatus == off`。先按 `dashboard-order.devices_order` 的 `sectionPos` 排序，再在数量吻合时应用 `group-item-order.devices` PID 前缀排序。

其他应用排除 currentApp，且同时要求 `installedModules` 和 `getInstalledRazerAppEngineProduct()` 的安装查询。查询失败返回空列表。推荐仅为 Synapse 推荐 Chroma、Chroma 推荐 Synapse，且推荐对象未出现在其他已安装应用中。原版首次渲染等待原生安装查询结束，本地实现应区分“未知”和“确认未安装”。

Armory 维护状态禁用点击，并显示 `ARMORY_MAINTENANCE_DESC`；安装版本存在且 `readFeatures` 未包含模块键 `armory` 时显示字面值 `New`。点击将模块键标记已读。

## 模块路由

以下按原始默认顺序列出。URL 模块使用 `policy=3,tab_visible=1,shouldFocus=1`。打开前先按完整 windowName 查询现有窗口，有则激活，否则新开；操作完成刷新其他应用安装列表并关闭弹层。

| currentApp | 模块 | windowName | URL／动作 |
| --- | --- | --- | --- |
| Synapse | linkedGames | profiles | `/synapse/profiles/` |
| Synapse | add-wifi-device | add-wifi-device | 发 `add-wifi-device`，不新开 URL |
| Synapse | macro | macro | `/synapse/macro/` |
| Synapse | alexa | alexa | `/synapse/alexa/` |
| Synapse | feedback | feedback-synapse | `/feedback/?app=synapse&path=%2Fsynapse%2Fdashboard` |
| Synapse | syn3-profile-migration | syn3-profile-migration | `/profile-migration/` |
| Synapse | armory | armory | `/synapse/armory/` |
| Chroma | chroma-studio | chroma-studio | `/synapse/chroma-studio/` |
| Chroma | audio-visualizer | audio-visualizer | `/synapse/audio-visualizer/` |
| Chroma | chroma-connect | chroma-connect | `/synapse/chroma-connect/` |
| Chroma | sensa-hd | sensa-hd | `/chroma-app/sensa-hd/` |
| Chroma | philips-hue | philips-hue-ui | `/synapse/products/769/ui/` |

设备打开 `/synapse/products/{pid}/ui/index.html?containerId={container}`，窗口名 `usb_5426_{pid}_{container}_ui`，`shouldFocus=1,tab_visible=1`。其他应用列表为 Chroma、Synapse、Streamer Companion、Virtual Ring Light，分别路由 `/chroma-app/dashboard/`、`/synapse/dashboard/`、`/alisha/`、`/natalie/`，使用独立应用窗口策略。

图标根路径 `/synapse/assets/imgs/apps/`；Alexa 为 `logo_alexa.svg`。其他已使用的图标有 `logo_chromastudio.svg`、`logo_add_wifi.svg`、`logo_synapse.svg`、`logo_chromaconnect.svg`、`logo_macro.svg`、`logo_visualizer.svg`、`logo_linked_games.svg`、`logo_streamer_app.svg`、`logo_natalie.svg`、`logo_feedback.svg`、`logo_hue.svg`、`profile_migration_logo.svg`、`logo_sensa.svg` 及随产品配置选择的 Armory 图标。

## 安装与 Alexa 服务边界

推荐应用安装由 `AppFolderInstallationChecker` 查询 window storage `appInfo`、原生产品列表及 `userDataDir\\Apps\\{folderName}` 是否存在；需要安装时打开原版隐藏安装器窗口。不能将页面切换或本地资源存在当作安装成功。

安装进度来自 `{installerId}_installer-status/install-progress/download-progress/installer-error`，推荐 ID 为 `common/chroma-app-folder` 或 `common/synapse-folder`；原始状态有 downloading、installing、completed、skipped、restart-required、canceled、error。完成类状态统一 completed。安装中整组项 opacity .3 且不可点击，标题后显示 16×16 绿色旋转指示。若没有服务，应提供明确的不可用或未知状态，不伪造下载进度。

工具栏的可选 Alexa 圆形按钮由 `alexaSettings.showHeader` 控制，功能是通过 `alexa-broadcast` 开始/停止录音；thinking/speaking 阶段忽略点击，教程指示由 `showTutorial` 控制。它与更多应用中的 Alexa 页面入口是两个独立操作。

Alexa 页面媒体以其自己的 `asset-manifest.json` 为准：`alexa-3.2a54a651.svg`、`alexa-2.0d4598c5.svg`、`alexa.b9b62050.svg`。主前端的 `alexa.110b43c3.avif` 属于模块卡片，不能替代页面中的教程或工具栏资源。

GPUI 接入采用保留的 Popover 状态、原生按钮键盘行为和 shell 领域事件。页面身份复用同一窗口/页签 ID，明确区分本地可浏览界面、真实安装状态和服务能力；安装、登录、录音均不由静态预览自动启动。

## 当前实现与验证边界

`src/shell/app_picker.rs` 已实现工具栏触发器和保留的 Base `PopoverState`；用 Base `Positioner` 处理锚点和窗口边缘，Base `Button` 构成原版三列网格，Base `Link` 提供发现更多链接。每个按钮的完整 96 高格子都可激活，内部自然高度的内容区单独负责圆角 hover 背景。关闭、鼠标和 Enter/Space 走同一动作路径，Escape 恢复触发器焦点。短窗口在弹层自身滚动；高度换算扣除宿主页签条后仍保留原版底部 40px，长标题 hover 保持 96 高的格子并在后续行上方展开。窗口失活时仍检查触发器及弹层内焦点，保留原版 iframe 的内部焦点保护。

`AppPickerCatalog` 把安装清单、原生应用查询、设备状态与本地可打开页面分开传入。`None` 表示未读取，明确的空集合才表示确认无已安装记录；未读取时显示状态文案，不显示推荐或空安装插画。模块/应用是否能打开由独立 capability 集合控制。工具栏绿点使用独立 `unread_features` 输入并通过 `UnreadCleared` 事件清除，不从 Armory 的 `readFeatures` 推断。缺少图片时保留 40×40 位置，不尝试加载空路径。

原始设备顺序只有长度相等时尝试 PID 前缀重排；本地对失效或重复的顺序额外校验，映射未完整覆盖时保留按 `sectionPos` 排好的设备，避免过时的偏好删掉仍连接的设备。模块顺序仍遵循原版“有匹配则显示该子集”的规则。

设置中的“更多应用 · 界面预览”有 10 种明确的合成状态：Synapse、Chroma、空记录、未读取、Armory 维护、推荐可安装、推荐下载中、推荐安装中、过滤与排序、窗口服务不可用。“过滤与排序”包含正在卸载的宏模块与混音器检查失败设备。预览与真实工具栏实体隔离，可安装状态也只记录点击对象，不启动安装、录音或外部窗口。

`app_picker_tests.rs` 包含源码回归：安装事实与本地 capability 不混用、原生查询交集、Armory 版本与移除条件、Chroma 模块来源、设备过滤与失效顺序、三列布局与 96 行高、禁用操作、鼠标/Tab/Enter/Space/Escape 路由与焦点、缩放后的短窗口滚动及数据更新。按当前授权只做格式与编译检查，这些测试未执行，动态视觉与交互仍待运行验收。
