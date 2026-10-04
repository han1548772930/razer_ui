# 模块注册表（module → 窗口 / URL / 打开参数）

2026-10-03。机器可读结果 [module-registry-audit.json](module-registry-audit.json)，脚本 [tools/audit-module-registry.cjs](../../tools/audit-module-registry.cjs)（`--check` 失败即报错）。
源码：`.ref/applications/rz-app-menu/static/js/main.83ced465.js`。

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
| alexa | `ModulePage::Picker(Alexa)` | 直接打开 |
| macro | `ModulePage::Macro` | 直接打开（窗口名 `macro`） |
| linked-games | `ModulePage::Profiles` → `Location::Profiles` | **本轮新接入**：按模块表打开 profiles 窗口 |
| armory | `ModulePage::Armory` | 直接打开 |
| profile-migration | `ModulePage::Picker(ProfileMigration)` | 直接打开 |
| tour | `ModulePage::IntroductionTour` | 直接打开 |
| feedback | 仍为门控 | 窗口名 `feedback-synapse`、URL 已按模块表原文写死；**该应用源码不在当前提取范围内**，无法复刻其界面，因此不伪造 |

新页面 [src/shell/profiles_page.rs](../../src/shell/profiles_page.rs) 给出 profiles 窗口外框（窗口名 `profiles`、`/synapse/profiles/`、`policy=3,shouldFocus=1,tab_visible=1`）与诚实正文。profiles 应用的导航是状态驱动（源码里没有可提取的路由表或 nav 数组），所以**没有臆造标签页**；配置文件列表与编辑、本地/云配置文件、导入导出、已关联游戏列表等待该应用视图接入。
