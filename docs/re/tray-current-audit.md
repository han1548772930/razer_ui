# 当前托盘与关闭行为复核

2026-10-03。仅静态读取当前 host 4.0.827 与 systrayv2；未运行应用、测试、安装包或下载的 JavaScript。

## 来源

- `tray-live-source-check.json`：当前线上 systray 清单及 16 项相关资源的逐字节核验。
- `tray-source-receipts.json`：菜单、窗口、网页、布局和 10 个语言包的 SHA-256。
- `save-close-current-source.json`：当前 Dashboard 工具栏、设备映射编辑器及宿主退出调用链的定位片段。
- `assets/synapse/manifest.json`：托盘 ICO／PNG 到 RGBA、Synapse SVG 的输入输出哈希。

## 保存与关闭

当前 Dashboard 有条件显示的未保存配置菜单包含 Save All／Discard All。设备映射编辑器也有未提交映射的 save-alert。这些不能等同于每个设备页底部一直出现的“保存到本机／丢弃更改”。后者是本地添加，已移除。

宿主 `main.js` 默认 `closeWindowAction=HIDE`，`TabUI.closeWindow` 进入 `handleCloseBtn`，并非通用退出保存确认。退出 `Dn` 检查不能退出的应用后执行关闭；没有本地通用保存弹窗。已移除该弹窗。

Windows 主窗口 X／原生关闭现在隐藏窗口，托盘 Synapse 入口恢复窗口。托盘 Exit 排空已经请求的写入，不把未提交草稿自动保存。固件更新退出保护保留；退出过程中写入失败会恢复窗口显示错误。托盘创建失败时关闭仍退出，避免留下无法恢复的隐藏窗口。

本地隐藏保留 GPUI 实体；没有实现 Electron renderer hibernation，也没有接入宿主 `force-close-window` 参数。

## 托盘已接入的范围

`tray-icon 0.26.0` 管理图标，`muda` 提供 Windows 原生右键菜单。菜单不是自绘仿制。当前本地只拥有一个 Synapse 应用、没有账户会话，菜单对应这一状态的顺序：Synapse、分隔线、Settings、Log In、分隔线、Exit。使用原版托盘图标、系统明暗对应的设置／用户图标、原版字符串，tooltip 为 Razer。

左键面板由独立 GPUI PopUp 窗口承载。布局对应 systrayv2 无 `user.item.id` 分支：登录提示及单应用启动项；宽 360，登录行 60，启动行 60，黑色边框，背景／悬停／字体色来自原版 CSS。没有虚构账户、设备或通知数据。面板保留独立实体、焦点、订阅及异步任务。

单击等待 200ms 后切换显示；双击取消单击等待并执行默认 showMenu；失焦等待 300ms 后隐藏。隐藏取消尚未完成的显示任务。原生调整尺寸在受管理的后台任务中等待完成，之后才显示，避免同步 WM_SIZE 重入 GPUI 借用。

## 尺寸的源代码差异

网页 `main.9579c403.js` 模块 597 的 signed-out 分支只取 `.header-2.clientHeight`，并查询单数 `.app.list-unstyled`；实际启动列表是复数 `.apps launcher list-unstyled`。因此网页请求高度为 60，而不是两行相加。宿主 `LeftSystray` 另设 `minimum_height:200`，`convertFeatureToWindowOptions` 映射为 `minHeight`，`SET_BOUNDS` 直接转发 Electron `setBounds`。

本地保留 200 最小窗口高度及网页 `trayY-60` 定位。之前把外框也固定为 122px 是错误的：当前 CSS 是 `min-height:100%`，现已让外框背景和边框填满窗口，两行内容仍占 120px。模块 597 使用主屏幕 WorkRect 横向约束；没有擅自替换为最近显示器完整钳制。Electron 对最小尺寸／透明区域的最终裁剪，以及 GPUI Root 的背景合成，**尚无运行验证**。混合 DPI、多显示器和非底部任务栏也未达到已验证一致的状态。

## 未完成项

用户随后报告 `cargo run` 时托盘不显示。静态排查确认 `tray-icon 0.26.0` 在 `NIM_ADD` 失败时仍返回成功；此前代码没有检查系统注册结果。现已把创建延后到主循环开始后，通过 `set_tooltip` 的 `NIM_MODIFY` 返回值确认注册，失败最多尝试四次并输出 `[tray]` 诊断。`set_visible` 本身不报告 Win32 失败，不能独自作为成功依据。诊断前清空 LastError，失败日志同时记录窗口归属、完整性级别、提权状态及作业 UI 限制。代码通过编译检查，环境定位见下一段。

后续用户日志定位到启动环境：应用 token integrity RID 为 4096（Low），Explorer 为 8192（Medium），均未提权、非 AppContainer。只读 `icacls` 检查确认仓库根目录、`target/debug` 和 `target/debug/razer_ui.exe` 均有可继承 Low Mandatory Level 标签。该标签使生成的 exe 以 Low 运行，无法注册 Medium 桌面的通知图标。无需管理员运行，也不应为此移除工作区的沙箱 ACL；在普通终端使用项目外的构建目录，见 [本地启动说明](../development.md)。尚未收到调整输出目录后的运行确认。

- 原版 Log In 打开自己的 800×600 `razer-id` 宿主窗口并建立会话；本地尚无此服务。点击当前入口显示既有服务未连接状态，不能完成登录，也没有改成浏览器登录冒充原版行为。
- 登录／访客会话后的账户头部、导航、设备、通知、服务数据及动态高度尚未实现。
- 原版安装目录发现、多应用启动／退出、Exit All Apps 尚未接入；当前菜单只代表本地拥有的 Synapse。
- `systrayDblClickAction` 的配置入口及 `launch` 分支尚未接入；只有原版默认 showMenu。
- Settings 进入本地设置页，未复刻独立宿主 Settings 窗口。
- 原生菜单、字体、透明窗口、焦点和 DPI 均只做了静态检查，不能宣称逐像素或运行行为完全一致。

## 已执行验证

`cargo fmt --all`、`cargo check --locked --all-targets` 通过；仍有既有未使用元数据／方法警告。资源验证通过 586 项；鼠标／键盘数据验证通过 75／71 个规格；通用控件与 Help 数据验证通过。没有执行测试或应用。

产品整体仍是进行中的实现，见 `native-product-coverage.md`，不能据此认定 331 个产品已经完成。

## 用户报告样式及图标偏暗后的复核

本次重新读取当前 systrayv2 的主 CSS、554 CSS/JS 和宿主图标路径，
不是依赖上文的旧结论。分支原文和位置保存在 `tray-ui-current-evidence.json`，
主 CSS 的哈希也已补入 `tray-source-receipts.json`。

| 差异 | 当前源码 | 修正 |
| --- | --- | --- |
| 登录提示字号 | body 为 16px、行高 1.22 | 删除本地 14px 覆盖，恢复字号、行高及底部 1px 内边距 |
| 外框范围 | `.systray` 的 `min-height:100%` | 外框填满窗口，不再在 122px 处结束 |
| 应用入口按下态 | `li:active>.icon{opacity:.3}` | 只降低图标透明度，标题和背景不变暗 |
| 通知区图标 | host `new Tray(rzAppEngineIco)`，Windows 路径指向当前 ICO | 按系统小图标尺寸选择 ICO 原始小尺寸帧 |

ICO 包含为 16、20、24、32、40、48、64px 等尺寸准备的独立位图。
旧脚本 `Image.open(...).convert(...).resize(...)` 默认读到实际为 512px
的 PNG 帧，再缩小到 32px；该帧在 ICO 目录中标记为 256px，所以还会触发
Pillow 尺寸警告。新脚本直接读取目标帧，不调色、不改透明度、不插值。
7 个通知区尺寸及 20px 右键菜单图标的全部 RGBA 字节均与官方帧相等，
见 `tray-icon-validation.json`。比较图保存在 `.work/tray-icon-comparison.png`。

## 会话和内容分支：源码条件及实现边界

| 官方条件 | 官方界面/行为 | 本地状态 |
| --- | --- | --- |
| 无 `user.item.id` | 60px 登录提示，无导航和内容区；有启动项时显示底栏 | 已接入，本次修正样式 |
| 有 ID 且 `isGuest` | 访客头像、Guest/razerId、登录按钮；点击经 `logOut` 进入会话流程 | 尚未接入真实会话，不把本地主工作区的访客标签当作宿主会话 |
| 有 ID 且非访客 | 用户头像、razerId、View Online；调用 `viewProfileInWeb` | 尚未接入 |
| 有 ID | Widgets/Notifications 页签、设置入口和内容区 | 尚未接入 |
| Widgets | 按 showWidget/hasWidget、安装状态和 widgetIndex 筛选排序；部分服务对访客隐藏 | 尚未接入 |
| Notifications 未完成加载/已加载为空/有内容 | 分别为加载状态、空提示与设置入口、通知列表；有内容时才显示 Clear | 尚未接入，不能伪造通知或一直显示加载状态 |
| 单/多启动项 | 单项显示图标和标题；多项显示图标及方向相关提示；过滤未安装应用 | 当前只有本地 Synapse 单项 |

因此两行登录菜单不能算整个托盘已经完成。账户服务、设备/小组件、通知、
多应用注册和动态高度仍需后续实现。

## 平台边界

`src/shell/tray.rs` 现在独立于 Windows 条件编译，持有公共菜单描述、翻译、
弹出界面、焦点及关闭命令；UI 不调用 Win32。
`src/shell/tray/windows.rs` 负责 Windows 托盘注册、原生菜单转换、系统图标尺寸、
窗口定位、显示/隐藏及消息接入。`TrayColors` 与 async-channel 也在公共层。
`tray-icon` 库本身支持多个平台，但本项目的 macOS/Linux 注册适配仍未实现；
本次拆分不能描述为已完成跨平台托盘。当前官方宿主自身也在 macOS 跳过 LeftSystray。

本次验证：`cargo check --locked --all-targets` 通过（既有未使用项警告仍在），
611 项资源校验、8 个官方 ICO 帧逐像素比对通过。未启动应用或运行测试，
不声称已完成真实窗口视觉验收。


## 2026-10-03??????????

`tray-icon` ??? `[dependencies]`??? `ksni`????? GTK ????
?? `tray.rs` ?? TrayIcon?????????????????????
AppShell ?????????????Windows ????? HWND ???
DPI ??????? Shell_NotifyIcon ?????

???? `LeftSystray.init/getInstance` ???? macOS????????
?????????????????????Linux ????????
GPUI ?????????????KSNI ???????????????
???????????macOS/Linux ???? rzAppEngine.png ???
16?16 RGBA?Windows ?? ICO ???????

Windows `cargo check --locked --all-targets` ? 612 ????????
9 ? ICO/PNG ?????????????????????????
?? macOS/Linux ?????????????????????????
????????????Widgets/Notifications ??????????
