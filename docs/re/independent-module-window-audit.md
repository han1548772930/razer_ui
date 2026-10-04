# 模块宿主页签接入审计

2026-10-04 更正：旧实现将 `policy=3` 解释为“第二个 gpui 窗口且同名复用”，
只改策略标签而没有核对宿主执行链，这个结论不成立。4.0.827 `Tab.js` 将此策略
送入已有窗口的标签栏；该处理器的新窗口分支对应 `policy=5/6/7/8`，两种分支之前
都会检查同名页面。完整调用链与哈希见[宿主策略审计](host-window-policy-current-audit.md)。

| 模块 | 宿主页签名 | 本地页面 |
| --- | --- | --- |
| `alexa` | `alexa` | `AlexaPage` |
| `macro` | `macro` | `MacroPage` |
| `armory` | `armory` | `ArmoryPage` |
| `linkedGames` | `profiles` | `ProfilesPage` |
| `feedback` | `feedback-synapse` | `FeedbackPage` |

模块目录及 App Picker 共用 `AppShell::open_module_tab`，通过现有导航保护和
`HostTabs::visit` 打开并聚焦具名页签。页签顺序、关闭、重新打开和键盘切换由宿主
处理；页面实体由 AppShell 保留。Feedback 保留草稿和确认层焦点，其根下不添加
Dashboard 工具栏。旧 `independent_window.rs` 的重复包装已删除。Alexa 页签采用
原始 `alexa` 名称，并兼容旧 `host-alexa` 保存顺序。

多设备配对按历史明确要求保留第二个 gpui 窗口和具名单例。这是本地例外：其原入口
也传 `sameWindow`，不能再声称第二窗口等价于原策略。局部窗口层使用 `Different`
表达本地选择，并拒绝用 `Same` 新建系统窗口。

直接打开入口不写入外部安装状态；各应用的设备、账户、扫描、日志、提交等服务边界
未改变。本轮只做静态与编译检查，不构成实际窗口或像素验收。

## 历史记录（以下独立系统窗口结论已被上文更正）

2026-10-04。当前 Dashboard 源码把 Macro、Armory、Profiles 和 Feedback 登记为具名应用根。它们
相对于 Dashboard 是独立窗口，但模块表传入 `sameWindow`（`policy=3`）、`shouldFocus=1`
和 `tab_visible=1`：同名窗口已存在时聚焦，不创建第二份实例。

| 模块 | 窗口名 | 地址 | 本地根 |
| --- | --- | --- | --- |
| `alexa` | `alexa` | `/synapse/alexa/` | `independent_window::AlexaWindow` |
| `macro` | `macro` | `/synapse/macro/` | `independent_window::MacroWindow` |
| `armory` | `armory` | `/synapse/armory/` | `independent_window::ArmoryWindow` |
| `linkedGames` | `profiles` | `/synapse/profiles/` | `independent_window::ProfilesWindow` |
| `feedback` | `feedback-synapse` | `/feedback/?app=synapse&path=%2Fsynapse%2Fdashboard` | `independent_window::FeedbackWindow` |

入口位于 `src/shell.rs::open_independent_module`，窗口登记和同名聚焦位于
`src/shell/display_window.rs`。窗口层使用 gpui 的第二个窗口承载这些根，并把模块表的
`policy=3` 保留在登记策略中；这不会把三个应用误合并到 Dashboard 的应用内路由。

2026-10-04 入口复核：顶部 App Picker 之前仍只把 Alexa、Wi-Fi、Profile Migration
登记为 bundled/launchable，因此 Macro、linkedGames 和 Armory 即使已编入本地也没有
可用入口。本轮在 `src/shell/app_picker_host.rs` 补齐这三个能力，并分别路由到同一个
具名窗口打开器（linkedGames → Profiles、macro → Macro、armory → Armory）。模块名、
窗口名和 URL 沿用当前 rz-app-menu 模块表；未写入 installed_modules/native_apps，
本地可打开不代表读取到了外部安装结果。`tools/audit-module-registry.cjs` 现在同时
检查 App Picker 的两个能力列表和直接打开路由。

页面内容仅使用已提取的当前稳定版资源和本地工作区状态。宏服务、Workshop 内容与
下载服务、Profiles 的安装扫描/全局游戏目录/设备子设备服务仍未接入，因而页面会保留
明确的未连接边界，不把空列表或本地草稿显示成真实服务结果。

2026-10-04 Feedback 接入：发现工具补上查询参数插值之前的固定路径，取得当前生产
入口的 HTML、清单和全部声明的 JS/CSS（27 项）。实际组件 `4496.ps.render` 仅挂载
`renderMainPage` 与 `renderFooter`，没有旧绿色左栏；十语言文案由 `3272` 的实际加载
表解析到 `FEEDBACK_SOURCE`。模块目录现在直接打开 `feedback-synapse` 并复用同名
窗口。提交和日志收集保留服务未连接状态，未向官方反馈 API 发送内容。此处只记录
本地表单与入口，不构成真实服务或像素验收。


Alexa named-window follow-up (2026-10-04): the current Dashboard registry records `alexa` at `/synapse/alexa/` with `policy=3,tab_visible=1`; the module box and App Picker now route to `ModulePage::Alexa`, which opens `independent_window::AlexaWindow`. The existing `AlexaPage` remains the same local source-audited page, so Settings preview can continue using its separate preview dialog. `display_window::open_or_focus` reuses the `alexa` name and focuses an existing window. Account, installer, microphone and Alexa service state remain unavailable without the native service and are not synthesized.
