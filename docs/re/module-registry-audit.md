# 模块注册表（module → 窗口 / URL / 打开参数）

2026-10-03。机器可读结果 [module-registry-audit.json](module-registry-audit.json)，脚本 [tools/audit-module-registry.cjs](../../tools/audit-module-registry.cjs)（`--check` 失败即报错）。
源码：`.ref/applications/rz-app-menu/static/js/main.83ced465.js`。

2026-10-04 已追踪 4.0.827 宿主执行链：本表的 `sameWindow` 模块进入当前窗口的具名
页签，旧文档把它解释成第二个 gpui 窗口的结论已撤回。模块目录和 App Picker
共用 `open_module_tab`；见[宿主策略审计](host-window-policy-current-audit.md)。

## 打开参数枚举（`ZP`）

| 名字 | 值 |
| --- | --- |
| `sameWindow` | `policy=3` |
| `diffWindow` | `policy=5` |
| `diffWindowSingleProcess` | `policy=7` |
| `tabVisible` / `tabInvisible` | `tab_visible=1` / `tab_visible=0` |
| `windowVisible` / `windowInvisible` | `browser_visible=1` / `browser_visible=0` |
| `autoFocus` | `shouldFocus=1` |
| `chromaIcon` / `synapseIcon` / `commonIcon` | `app_icon_path=ChromaApp/icon.ico` / `Synapse/icon.ico` / `Common/icon.ico` |

新建窗口用的完整策略串（同 armory）：`policy=5,tab_visible=0,app_name=synapse,width=1280,height=720,minimum_width=600,minimum_height=500,app_icon_path=Synapse\window.ico`。

## 模块表

| moduleName | windowName | url | openParam |
| --- | --- | --- | --- |
| `linkedGames` | `profiles` | `/synapse/profiles/` | sameWindow + autoFocus + tabVisible |
| `add-wifi-device` | `add-wifi-device` | `null` | — |
| `macro` | `macro` | `/synapse/macro/` | sameWindow + autoFocus + tabVisible |
| `alexa` | `alexa` | `/synapse/alexa/` | sameWindow + autoFocus + tabVisible |
| `feedback` | `feedback-synapse` | `/feedback/?app=synapse&path=%2Fsynapse%2Fdashboard` | sameWindow + autoFocus + tabVisible |
| `syn3-profile-migration` | `syn3-profile-migration` | `/profile-migration/` | sameWindow + autoFocus + tabVisible |
| `armory` | `armory` | `/synapse/armory/` | sameWindow + autoFocus + tabVisible |

注意 `linkedGames` 的窗口名是 **`profiles`**：原版点「已关联的游戏」打开的是 profiles 窗口（同窗口、聚焦、标签栏可见），不是新进程窗口。安装侧也印证这一点：Dashboard 的 `loadFeatures()` 用
`window.open("/installer/#type=module&id=linkedGames&location=synapse/profiles","synapse_install_linkedGames","policy=7,tab_visible=0,browser_visible=0")` 安装该模块。

## 本地模块目录现状

| 模块盒 | 本地目标 | 说明 |
| --- | --- | --- |
| alexa | `ModulePage::Alexa` | 直接打开宿主 `alexa` 页签 |
| macro | `ModulePage::Macro` | 直接打开（窗口名 `macro`） |
| linked-games | `ModulePage::Profiles` → `profiles` 宿主页签 | 按模块表以 `policy=3` 打开并聚焦已有页签 |
| armory | `ModulePage::Armory` | 直接打开 |
| profile-migration | `ModulePage::Picker(ProfileMigration)` | 直接打开 |
| tour | `ModulePage::IntroductionTour` | 直接打开 |
| feedback | `ModulePage::Feedback` | 当前官方 `/feedback/` HTML、清单与 27 个 JS/CSS 已静态取得；直接打开 `feedback-synapse` 宿主页签，提交和日志收集仍未连接服务 |

当前 Profiles 页面按 module 43 的实际挂载复刻 Games 和 Devices 两个页签，复用本地 `ProductWorkspace` 列表与用户关联；服务扫描仍未连接。Feedback 的实际根仅挂载 500px 表单和页脚，旧 `renderLeft` 方法未被调用。具体组件、输入限制与服务边界见 [Feedback 审计](feedback-app-current-audit.md)。七个模块盒现在均有直接打开入口；入口可用不代表设备、账户或远端服务已连接。

2026-10-04 选择器（App Picker）去门控：模块行只要在本地有页面就直接打开，不再落回
「窗口服务尚未连接」。除上表七个之外，Chroma 应用的 Chroma Studio 也在本地实现
（打开 `chroma-app` 窗口里的 Chroma 页面），已加入 bundled/launchable 列表；
Philips Hue 的模块页就是 769 产品工作区，仅在本地存在该设备时登记并直接打开，
没有设备时明确说明而不是显示安装门控。`audio-visualizer`、`chroma-connect`、
`sensa-hd`、Streamer Companion 与 Virtual Ring Light 在本地没有页面，保留门控。
`tools/audit-module-registry.cjs --check` 会核对这条规则（含 PhilipsHue 的条件登记）。
