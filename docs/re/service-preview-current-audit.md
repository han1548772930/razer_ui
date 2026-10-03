# 服务预览与正式页面入口复核

2026-10-03。用户指出服务预览只有弹层，本轮清点 `SettingsPage::connection` 的入口与实际实例、正式页面调用方。范围是入口和页面组成；不等于对全部 1419 个主导航页逐像素验收。来源仅使用当前产品包、当前 Dashboard 与当前独立应用，版本见 [当前源记录](20-current-source-version.md)。

## 本轮修正

- 服务预览新增直接进入产品标签页的入口：179、164、241、740、746、769、778、784、3871、3884、3886、3946。它们调用同一 `AppShell::add_preview` / `ProductWorkspace`，保留正式导航、主体和页面内弹层入口；其他注册产品仍可通过产品搜索进入。
- 原先若干“预览产品”按钮实际打开独立状态样例，现明确标为状态样例。样例控件和通用 Dialog 是本地开发工具，不是原版界面的一部分。
- Dock 样例默认显示设备主体。从主体打开配对时显示正式 `DockDialog`，保留页面作为背景；关闭返回页面，再次打开恢复弹层和焦点，不再直接替换成没有原版标题栏的弹层正文。
- Aether 样例直接创建独立的 `SourceProductWorkspace`，其产品导航、配置栏、主体使用同一实现。开发场景只操作该独立实例。
- 校准样例也已改为独立 `SourceProductWorkspace`：支持 740 / 746，默认显示介绍、键盘图与校准卡，页面按钮及八个显式场景均使用同一个 CalibrationModal；关闭返回页面。
- Alexa 原先 PreviewAlexa 与普通入口进入同一个实体，正式页无条件显示开发场景条。现在普通实例不显示、不接受场景注入；预览创建独立实例，测试源码覆盖两者状态不互相影响。预览 Dialog 复用正式主体，但不包含宿主工具栏，不能称为完整宿主预览。

## 全部专用预览入口的组成

| 入口 | 实际复用范围 | 仍缺少或需要区分的部分 |
| --- | --- | --- |
| 顶部状态 | 独立 HeaderStatusPreview 状态样例 | 仿 38px 工具栏不是整个 AppShell Header |
| 配置迁移 | 正式 MigrationPage 主体 | 样例 Dialog 缺 HostTab / 独立工具栏；正式迁移入口另有 HostTab |
| 更多应用 | 正式 AppPicker 弹层 | 样例工具栏不是完整宿主，锚点与正式环境不同 |
| 模块 | 独立 ModulePreview 的安装、已安装、固件样例 | 没有直接复用完整 ModuleCatalog / Dashboard |
| 磁轴键盘校准 | 740 / 746 的正式 SourceProductWorkspace 与 CalibrationModal | 已补介绍、键盘图、校准卡及产品导航；场景条和容器仍为开发工具 |
| 灯光设置 | 正式 lighting::content 片段 | 不含 SettingsPage 侧栏和宿主工具栏 |
| Philips Hue | 正式 HueWorkspace 业务主体 | 样例仍是 500px 滚动容器；完整产品入口另提供正式外层。原版 Hue 本来没有配置栏 |
| 主板 / 有线 ARGB | 正式 WiredArgbWorkspace 主体 | 样例控件独立；完整产品入口提供外层导航 |
| 无线 ARGB | 正式 WirelessArgb 主体 | 3886 的端口分支原码不可达，显式编辑器样例不能计入正式页面 |
| Aether 灯带 | 正式 SourceProductWorkspace 及 AetherStrip | 场景条和 Dialog 仍是开发工具；不是原版宿主窗口 |
| 自动化 | 正式 Automation 主体与编辑层 | 完整产品入口提供外层；宏、游戏、快捷键完整子编辑器仍未完成 |
| 鼠标底座 | 正式 DockPairing 主体与 DockDialog | 样例控制条独立；设备服务结果只允许明确选择示例 |
| Alexa | 隔离的正式 AlexaPage 主体 | 普通页已隔离开发控件；样例 Dialog 缺宿主外层 |
| Chroma 教程 | 正式 IntroductionTour 与 HostTab | 本轮未发现单独缺外层的问题；不替代教程自身的视觉验收 |
| 多设备配对 | 正式独立 Pairing 路由 | 与 Dock 的产品配对弹层不同，不能互相替代 |

对应本地实现：`src/shell/settings_page.rs`、`src/shell.rs`、各功能 `preview.rs`、`src/shell/module_preview.rs`、`src/features/keyboard_calibration.rs`。本表记录真实复用范围，不把“同一组件”推断成样式已正确。

## 尚未完成的整页工作

原生覆盖表只统计实际分派内容。设备页的配置菜单、Gamer Room 图标、布局响应式细节、源 hover/tooltip/动画和若干独立 displayMode 分支仍需逐项补齐。模块、顶部状态样例仍不等于完整页面预览；没有通过修改按钮名称把这些缺口算作已实现。

## 已打包页面直接打开

用户随后明确要求已接入界面直接打开，不显示远程 UI 下载或安装门控。模块目录的 Alexa 按钮改为“打开”，经真实本地路由进入 AlexaPage；首次打开与刷新均呈现本地 Home。AppPicker 以独立 bundled_modules 集合展示 Alexa、添加 Wi-Fi 设备及配置迁移，未知的外部安装状态不再隐藏这些本地入口，也不使其落入安装推荐。没有把未知的外部模块安装结果写成 installed；原版安装场景仍可在明确的状态样例里查看。此处的入口行为差异遵循用户要求。

所有图片、颜色、字体、时间参数应回溯到具体来源；不能用“存在描述符”“编译通过”或“样例可打开”代替视觉一致性。运行应用及测试仍按 AGENTS.md 禁止，仅执行允许的静态校验、资源准备、格式化和 `cargo check --locked --all-targets`。
