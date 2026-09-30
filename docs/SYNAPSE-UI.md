# 雷云 4 前端：完整功能与布局

> 本文只写**有证据**的内容，每条都标出处。没有依据的推测不写进来。
>
> **上游产物**：完整功能清单见 [`SYNAPSE-FEATURES-FULL.md`](SYNAPSE-FEATURES-FULL.md)
> （4992 条真实文案 / 1101 个命名空间）。

---

## 0. 证据从哪来

| 证据 | 位置 | 说明 |
|---|---|---|
| 前端入口 HTML | `.ref/frontend/index.html` | 4.6 KB，直接抓自 `apps.razer.com` |
| 资源清单 | `.ref/frontend/asset-manifest.json` | 679 条目，**89 个命名模块** |
| JS/CSS 分块 | `.ref/frontend/static/` | 127 个，9.4 MB |
| 语言包 | `locales/zh-CN.json`、`locales/en.json` | 4992 / 5014 条 |
| 全部语言 | `.ref/frontend/locales/` | 10 种，各约 4950–5014 条 |
| 抓取脚本 | `.ref/tools/fetch-frontend.ps1`、`fetch-chunks.ps1` | 可复现 |
| 语言包导出 | `.ref/tools/extract-locale.js` | 在 Node 里执行 webpack 模块 |

所有结论都标注 `[HTML]`（出自入口 HTML）、`[清单]`（出自 asset-manifest）、
`[文案]`（出自语言包）、`[CSS]`（出自样式分块）、`[JS]`（出自脚本分块）。

---

## 1. 技术栈与版本 [HTML]

雷云 4 的界面是一个 **Create React App** 构建的 Web 应用，由 Electron 外壳加载。

### 1.1 运行时依赖

| 库 | 版本 | 用途 |
|---|---|---|
| React | 18.2.0 (UMD) | UI 框架 |
| react-dom | 18.2.0 | 渲染 |
| react-redux | 8.0.5 | 状态管理 |
| `@sentry/react` | 7.72.0 | 错误上报 |
| `@sentry/tracing` | 6.19.7 | 性能追踪 |
| pkijs | 3.0.15 | 证书 / 公钥加密 |
| buffer | 6.0.3 | 二进制处理 |

### 1.2 构建信息

```
应用版本   0.0.86
build      2609150817
git 分支   master-21sep2026
git 提交   4073c224f2f452aa18e3128b9c7ee82b94e6ffc3
构建时间   1789460826551
```

### 1.3 内部依赖仓库 [HTML]

`commonrepository`、`jstestrzdevice`、`loginrepository`、`razer-anne-utilities`、
`razer-app`、`razer-cli-tools`、`razer-config`、`razer-version`、`rz-app-feature`

> 内部代号 **`project_anne_dashboard`** —— 见 JS 分块里的
> `self.webpackChunkproject_anne_dashboard` [JS]。

### 1.4 运行形态 [HTML]

- 入口路径 `/synapse/dashboard/`
- `<html lang="en">`：语言**由前端运行时切换**，不是服务端决定
- 注册了 **Service Worker**，并有 `ChunkLoadError` 自动重载逻辑
  （分块加载失败时注销 SW 并刷新，1 分钟内不重复）
- 根节点 `<div id="root">` 内先放一个启动动画
  （`.rz-init-loader` / `.rz-init-spinner`，品牌绿 `#44d62c`）

---

## 2. 布局

### 2.1 应用外壳

从 `[JS]` 与 `[CSS]` 可确认的顶层结构：

```
<App>
├── Header            顶部栏（含 header-avatar，用户头像）
├── DeviceList        设备列表
└── Content           内容区
    └── body-wrapper-content / content__body / child-content
<IotPopupRoot>        弹窗根（与 App 并列）
```

| 结构 | 证据 |
|---|---|
| `Header` / `DeviceList` / `Content` | `[JS]` App 分块里同时出现这三个标识 |
| 内容区三层包裹 | `[CSS]` `.body-wrapper`、`.body-wrapper-content`、`.content`、`.content__body`、`.child-content` |
| 抽屉层 | `[CSS]` `.config-drawer`、`.config-drawer-content`、`.close-drawer` |
| 弹窗根 | `[清单]` 独立模块 `IotPopupRoot.js` + `[CSS]` `.modal-content/.modal-title/.modal-desc` |
| 顶部下拉 | `[CSS]` `.dropdown-razer`、`.dropdown-item`、`.dropdown-divider` |

> **修正**：此前我把外壳写成「侧边栏 + 区块」，那是**从 i18n key 名前缀推断的、没有依据**。
> 读到前端后可确认的顶层结构是 **Header + DeviceList + Content**，加一个并列的弹窗根。

### 2.2 页面内的导航：标签页

标签页的完整词汇表有 **25 个**，来自两处并已对齐：

- 语言包 **23 个** `TAB_*`（`locales/zh-CN.json`，都有中文原文）；
- 设备模块额外导出 `TAB_KEY_BINDS`、`TAB_MY_MACROS`
  （文案键为 `TEXT_NAV_TAB_KEY_BINDS`=**按键绑定**、`TEXT_NAV_TAB_MY_MACROS`=**我的宏**）。

| key | 中文 | key | 中文 |
|---|---|---|---|
| `TAB_HOME` | 首页 | `TAB_POWER` | 电源 |
| `TAB_CUSTOMIZE` | 自定义 | `TAB_BATTERY` | 电池 |
| `TAB_KEY_BINDS` | 按键绑定 | `TAB_AUDIO` | 音频 |
| `TAB_MY_MACROS` | 我的宏 | `TAB_SOUND` | 声音 |
| `TAB_GAMING` | 游戏 | `TAB_MIC` | 麦克风 |
| `TAB_LIGHTING` | 灯光 | `TAB_MIXER` | 混音器 |
| `TAB_EFFECTS` | 效果 | `TAB_EQ` | 均衡器 |
| `TAB_COLOR` | 颜色 | `TAB_ENHANCEMENT` | 增强 |
| `TAB_CALIBRATION` | 校准 | `TAB_HAPTICS` | 触觉 |
| `TAB_PERFORMANCE` | 性能 | `TAB_OLED` | OLED |
| `TAB_DISPLAY` | 显示 | `TAB_PAIRING` | 正在配对 |
| `TAB_SCROLLING` | 滚动 | `TAB_SETTING` | 设置 |
| `TAB_DEMO` | 演示 | | |

> **注意**：这 25 个只是**词汇表**。某个设备实际显示哪几个标签页，
> **不在这份清单里**，而是写在该设备的模块中，见 §2.5。

`[CSS]` 里存在 `.cb-tabs` 与大量 `tab` 相关类，与「内容区内切换标签页」一致。

### 2.3 页面单元

| 单元 | 证据 |
|---|---|
| 设备卡片 | `[CSS]` CSS-module 组件 `DeviceCard_deviceCard__*`、`DeviceCard_deviceImage__*`、`DeviceCard_connected__*`、`DeviceCard_categoryIcon__*` |
| 图形/曲线 | `[CSS]` `.canvas_graph_container`、`.canvas_graph_container_wraper`、`sliderChart` ——**确实有 canvas 绘图** |
| 滑杆 | `[CSS]` `sliderChart` |
| 配置抽屉 | `[CSS]` `.config-drawer`、`.config-btns-wrapper`、`.config-row` |
| 动作栏 | `[CSS]` `.action_bar_wrapper`、`.action-wrapper` |
| 步骤指示 | `[CSS]` `.caliSteps`（校准步骤） |

### 2.4 弹窗与教程

| 弹窗 | 证据 |
|---|---|
| 通用弹窗 | `[清单]` `IotPopupRoot.js` |
| 教程弹窗 | `[CSS]` `.dashboard-tutorial-modal`（含 `__btn`、`__indicator`、`__wrapper`） |
| 带视频的教程 | `[CSS]` `.analog-tutorial-modal__video` |
| 确认框 | `[CSS]` 组件 `ConfirmDialog` |
| 动态灵敏度教程 | `[清单]` `DynamicSensitivityTutorialPopup.js`、`DynamicSensitivityTutorialIcon.js` |
| 遥测/监控面板 | `[CSS]` `MonitoringDashboard`、`MonitoringToggle` |

---

## 2.5 设备页面：每个产品一份独立应用

**这是最关键的一点**：雷云主界面只提供外壳（Header + DeviceList + 内容区）。
真正的设备页面是**按产品单独下发的 React 应用**：

```
https://apps.razer.com/synapse/products/<productId>/ui/
```

因此「某个设备显示哪些标签页」**不在主前端里**，而在该设备模块自己的代码中。
本仓库已下载两份完整模块到 `.ref/devices/`（抓取脚本
`.ref/tools/fetch-device-module.ps1`）。

### 2.5.1 两份模块的对照

| 项 | 鼠标 | 键盘 | 耳机 |
|---|---|---|---|
| productId | **182** | **653** | **777** |
| 设备名 | Razer DeathAdder V3 Pro | Blackwidow V4 Pro | RAZER KRAKEN BT SANRIO LIMITED EDITION |
| 模块内部名 | `Litat1_UI` | `Selenat1_UI` | `Kittybt_UI` |
| 入口 HTML | 4 819 B | 4 744 B | 4 715 B |
| `asset-manifest.json` | 58 100 B | 74 513 B | 39 634 B |
| 代码分块 | 118 个 / 6.8 MB | 212 个 / 15.8 MB | 46 个 / 5.8 MB |
| 命名模块 | **48** 个 | **75** 个 | **6** 个 |
| 其中 `Map*` 动作编辑器 | 32 个 | 32 个 | **0 个** |
| 其中硬件协议解析器 | **0 个** | **27** 个 | **0 个** |
| 版本（edition） | 6 个 | 4 个 | 5 个 |
| 页面标签页 | 6 个 | 5 个 | 6 个 |

> 三点值得注意：
>
> 1. **鼠标模块完全不含协议解析器，键盘模块含 27 个** —— 解析器按需下发。
> 2. **耳机模块只有 6 个命名模块、没有 `Map*` 动作编辑器** —— 耳机不能重映射按键。
> 3. **耳机的版本号里出现兄弟产品 ID**（`1328_0`、`1332_0`）——
>    同一模块可服务多个 productId 变体。

### 2.5.2 该设备实际显示的标签页

顺序取自模块内常量的**声明顺序**（即源码顺序）。

**鼠标（productId 182，DeathAdder V3 Pro）—— 6 个**

| # | key | 中文 |
|---|---|---|
| 1 | `TAB_CUSTOMIZE` | 自定义 |
| 2 | `TAB_PERFORMANCE` | 性能 |
| 3 | `TAB_PAIRING` | 正在配对 |
| 4 | `TAB_CALIBRATION` | 校准 |
| 5 | `TAB_POWER` | 电源 |
| 6 | `TAB_SCROLLING` | 滚动 |

**键盘（productId 653，BlackWidow V4 Pro）—— 5 个**

| # | key | 中文 |
|---|---|---|
| 1 | `TAB_CUSTOMIZE` | 自定义 |
| 2 | `TAB_PERFORMANCE` | 性能 |
| 3 | `TAB_LIGHTING` | 灯光 |
| 4 | `TAB_POWER` | 电源 |
| 5 | `TAB_SCROLLING` | 滚动 |

**耳机（productId 777，RAZER KRAKEN BT SANRIO）—— 6 个**

| # | key | 中文 |
|---|---|---|
| 1 | `TAB_CUSTOMIZE` | 自定义 |
| 2 | `TAB_LIGHTING` | 灯光 |
| 3 | `TAB_CALIBRATION` | 校准 |
| 4 | `TAB_POWER` | 电源 |
| 5 | `TAB_SOUND` | 声音 |
| 6 | `TAB_MIC` | 麦克风 |

**三类对比**

| 标签页 | 鼠标 | 键盘 | 耳机 |
|---|:--:|:--:|:--:|
| 自定义 | ✅ | ✅ | ✅ |
| 性能 | ✅ | ✅ | — |
| 灯光 | — | ✅ | ✅ |
| 校准 | ✅ | — | ✅ |
| 电源 | ✅ | ✅ | ✅ |
| 滚动 | ✅ | ✅ | — |
| 正在配对 | ✅ | — | — |
| 声音 | — | — | ✅ |
| 麦克风 | — | — | ✅ |
| **合计** | **6** | **5** | **6** |

结论很明确：标签页是**按设备类型（乃至按具体产品）下发**的。
`TAB_LIGHTING` 对键盘与耳机有、对鼠标没有（该鼠标无灯）；
`TAB_PERFORMANCE`/`TAB_SCROLLING` 只有鼠标与键盘有；
`TAB_SOUND`/`TAB_MIC` 只在耳机上出现。

### 2.5.2.1 逐界面文档

每个界面另有一份独立文档，写明它的**布局**（UI 分区 + 布局类名）与**功能项**
（雷云真实文案逐条列出）：**[`docs/screens/`](screens/README.md)**

| 界面 | key | 出现于 | 功能项 |
|---|---|---|---|
| [自定义](screens/01-customize.md) | `TAB_CUSTOMIZE` | 鼠标 · 键盘 · 耳机 | 192 |
| [性能](screens/02-performance.md) | `TAB_PERFORMANCE` | 鼠标 · 键盘 | 52 |
| [正在配对](screens/03-pairing.md) | `TAB_PAIRING` | 鼠标 | 68 |
| [校准](screens/04-calibration.md) | `TAB_CALIBRATION` | 鼠标 · 耳机 | 74 |
| [电源](screens/05-power.md) | `TAB_POWER` | 鼠标 · 键盘 · 耳机 | 90 |
| [滚动](screens/06-scrolling.md) | `TAB_SCROLLING` | 鼠标 · 键盘 | 55 |
| [灯光](screens/07-lighting.md) | `TAB_LIGHTING` | 键盘 · 耳机 | 131 |
| [声音](screens/08-sound.md) | `TAB_SOUND` | 耳机 | 264 |
| [麦克风](screens/09-mic.md) | `TAB_MIC` | 耳机 | 106 |

### 2.5.2.2 界面里的布局是怎么还原的

设备模块的 JS 是压缩过的，但**类名**与**文案 key** 不会被压缩掉，于是：

- **布局**：取设备模块 CSS 里的语义类名（如 `custom-profile-bar`、`keymap-component`、
  `action_bar_wrapper`、`combined-key-pair`），按语义归入 UI 分区，即得该页的排布。
  哈希类名（CSS-module 自动生成，如 `a227b74c`）剔除。
- **文案**：取语言包里该页命名空间的 key，中文为雷云原文。

还原脚本：`.ref/tools/gen-screen-docs.js`。

### 2.5.3 「自定义」标签页的内部结构

两个模块声明了完全相同的一套 key，可据此还原该页的构成：

| key | 中文 `[文案]` | 含义 |
|---|---|---|
| `STANDARD` | 标准 | 第一层按键层 |
| `HYPERSHIFT` | Hypershift | 第二层（按住 Hypershift 键时生效） |
| `HYPERSHIFT_TOOLTIP` | — | 第二层说明 |
| `KEYMAP` | 按键映射 | |
| `PROFILES` | 配置文件 | |
| `PROFILE` | 配置文件 | |
| `MOUSE_USE` | 鼠标使用 | 仅鼠标相关 |
| `MOUSE_USE_TOOLTIP` | — | |
| `LEFT_HANDED` | 左手 | 左右手切换 |
| `RIGHT_HANDED` | 右手 | |
| `ADD` `IMPORT` `RENAME` `DUPLICATE` `EXPORT` `DELETE` | 添加 / 导入 / 重命名 / 复制 / 导出 / 删除 | 配置管理菜单 |
| `RESET_PROFILE` `RESET_PROFILE_TITLE` | 重置配置 | |
| `RESHARE_TO_WORKSHOP` `RESHARE` `RESHARE_DESC` | 分享到创意工坊 | |
| `SYNCING_PROFILES` | 同步配置中 | 云同步状态 |
| `PROFILE_DISABLED_TOOLTIP` | — | 配置被禁用时的提示 |

> 也就是说，**自定义页 = 标准/Hypershift 两层 + 按键映射 + 配置管理**，
> 不是简单的一张按键表。

### 2.5.4 共用面板组件

两个模块的「非 Map、非解析器」模块**完全一致**，共 11 个：

| 模块 | 作用 |
|---|---|
| `KeyMappingComponent.js` + `.css` | 按键映射主界面 |
| `KeypadButtonPanel.js` | 键区按键面板 |
| `ButtonPanelComponent.js` | 按键面板 |
| `TwoTapKeyMapping.js` + `.css` | 双击（Two-Tap）映射 |
| `DynamicSensitivityTutorialPopup.js` | 动态灵敏度教程弹窗 |
| `DynamicSensitivityTutorialIcon.js` | 对应图标 |
| `AppFolderInstallationChecker.js` | 模块安装检查 |
| `service-worker.js` | 离线缓存 |
| `main.js` / `main.css` | 模块入口 |

### 2.5.5 动作编辑器（32 个）

两个模块各自携带**完全相同的 32 个** `Map*` 动作编辑器：

| | | | |
|---|---|---|---|
| `MapAILauncher` | `MapAppSpecific` | `MapAudio` | `MapBrightness` |
| `MapBrightnessGlobal` | `MapControlKnobFunction` | `MapControllerPlaystation` | `MapControllerV2` |
| `MapDefault` | `MapDialFunction` | `MapDisable` | `MapDynamicKeyStroke` |
| `MapGlobalSensitivity` | `MapGlobalSwitchProfile` | `MapHyper` | `MapInterDevice` |
| `MapJoyStick` | `MapKeyboard` | `MapKeyboardCombineMouse` | `MapLaunch` |
| `MapLighting` | `MapMacro` | `MapMacroKey` | `MapMedia` |
| `MapMouse` | `MapRoller` | `MapScrolling` | `MapSensitivity` |
| `MapSwitchKeymap` | `MapSwitchProfile` | `MapText` | `MapWindows` |

> 主前端（`asset-manifest.json`）另列有 `MapPerfect180` 与 `MapPerfect180GameList`，
> 共 34 个；设备模块里没有这两个，只有 32 个。

### 2.5.6 设备图片

每个版本各一套，**1x / 3x 两种分辨率 × avif / png 两种格式**：

```
products/<id>/ui/<id>_<edition>/PluginImages/<id>_<edition>_0_dashboard1x.avif
products/<id>/ui/<id>_<edition>/PluginImages/<id>_<edition>_0_dashboard1x.png
products/<id>/ui/<id>_<edition>/PluginImages/<id>_<edition>_0_dashboard3x.avif
products/<id>/ui/<id>_<edition>/PluginImages/<id>_<edition>_0_dashboard3x.png
```

本机日志里出现过 18 条不同的图片路径（涉及 productId 179 与 182）。

### 2.5.7 版本（edition）

同一个 productId 下可有多个版本，路径形如 `<id>_<edition>`：

| productId | 版本 |
|---|---|
| 182 | `182_0`、`182_128`、`182_129`、`182_130`、`182_131`、`182_132`（6 个） |
| 653 | `653_0`、`653_128`、`653_129`、`653_130`（4 个） |

各版本在 `manifest.json` 的 `productTranslations` 里给出**本地化的产品名**：

```json
"182_0": {
  "en": { "PRODUCT": "Razer Deathadder V3 Pro",
          "DASHBOARD_NAME": "Razer Deathadder V3 Pro",
          "DASHBOARD_EDITION": "" }
}
```

### 2.5.8 设备模块的发现方式

`<productId>` 取 USB PID 的十进制。用 `.ref/tools/scan-products.ps1` 逐个探测
`manifest.json`，实测结果：

| 区间 | 结果 | 设备种类 |
|---|---|---|
| 1 – 499 | 79 个 | **全部为鼠标与鼠标底座** |
| 500 – 700 | 90 个 | **键盘**（BlackWidow / Huntsman / Cynosa / Ornata / DeathStalker）、Blade 笔记本、Tartarus 键区 |
| 701 – 1100 | 31 个 | 键盘（Pro Type Ergo、BlackWidow V4 HyperSpeed、Huntsman V3 Pro/HE）、耳机（Kraken）、Chroma 设备（Philips Hue） |
| **合计** | **200 个** | |

完整清单（productId / 设备名 / 模块内部名）：
`.ref/notes/razer-products.txt`、`razer-products-low.txt`、`razer-products-high.txt`。

---

## 3. 功能全清单

**完整清单**： [`SYNAPSE-FEATURES-FULL.md`](SYNAPSE-FEATURES-FULL.md) ——
4992 条真实文案，分 1101 个命名空间，逐条列出 `key / 中文 / English`。

最大的几个命名空间（条目数）：

| 命名空间 | 条数 | 领域 |
|---|---|---|
| `TEXT` | 153 | 文本功能 |
| `AUDIO` | 91 | 音频 |
| `RAZER` | 90 | 品牌/通用 |
| `OLED` | 84 | OLED 显示屏设备 |
| `PERFORMANCE` | 73 | 性能 |
| `TOGGLE` | 65 | 开关类设置 |
| `MIC` | 55 | 麦克风 |
| `LIGHTING` | 49 | 灯光 |
| `CUSTOMIZE` | 48 | 自定义/按键 |
| `THX` | 48 | THX 音效 |
| `POWER` | 41 | 电源 |
| `SCROLL` | 40 | 滚轮 |
| `DYNAMIC` | 38 | 动态键程 |
| `CALIBRATION` | 35 | 校准 |

### 3.1 动作映射编辑器（34 个）[清单]

这是**按键可绑定的动作全集** —— 每个 `Map*` 模块对应重映射菜单里一个动作的配置界面：

| 模块 | 对应动作 |
|---|---|
| `MapDefault` | 默认 |
| `MapDisable` | 禁用 |
| `MapMouse` | 鼠标功能 |
| `MapKeyboard` | 键盘功能 |
| `MapKeyboardCombineMouse` | 键盘 + 鼠标组合 |
| `MapMacro` | 宏 |
| `MapMacroKey` | 宏按键 |
| `MapMedia` | 多媒体 |
| `MapLaunch` | 启动程序 |
| `MapWindows` | Windows 快捷键 |
| `MapText` | 文本功能 |
| `MapSensitivity` | 灵敏度 |
| `MapGlobalSensitivity` | 全局灵敏度 |
| `MapBrightness` | 亮度 |
| `MapBrightnessGlobal` | 全局亮度 |
| `MapLighting` | 灯光 |
| `MapScrolling` | 滚动 |
| `MapRoller` | 滚轮功能 |
| `MapDialFunction` | 多功能旋钮 |
| `MapControlKnobFunction` | 控制旋钮 |
| `MapSwitchKeymap` | 切换按键映射 |
| `MapSwitchProfile` | 切换配置文件 |
| `MapGlobalSwitchProfile` | 全局切换配置文件 |
| `MapHyper` | Hypershift |
| `MapDynamicKeyStroke` | **动态键程** |
| `MapPerfect180` | **Perfect 180** |
| `MapPerfect180GameList` | Perfect 180 游戏列表 |
| `MapAppSpecific` | 按应用 |
| `MapInterDevice` | 设备交互 |
| `MapJoyStick` | 摇杆 |
| `MapAudio` | 音频 |
| `MapAILauncher` | AI 启动器 |
| `MapControllerV2` | 手柄 V2 |
| `MapControllerPlaystation` | PlayStation 手柄 |

### 3.2 硬件协议（30 个解析器）[清单]

`rzHardwareEvents*Parser` 模块，每个对应一类设备/协议的硬件事件解析：

| 分组 | 解析器 |
|---|---|
| 通用协议 | `Protocol25`、`Protocol30`、`Protocol40`、`Protocol40ParserEarbuds`、`Protocol40ParserHeadset` |
| 键区/OLED | `Protocol25Keypad`、`Protocol25Oled`、`Protocol30Oled`、`Protocol25Glitter` |
| 手柄 | `Protocol25Controller`、`Protocol25Controller8k`、`Protocol25ControllerRecordID05`、`Protocol25PlayStationController`、`Protocol25PSArcadeController` |
| 音频芯片 | `AudioAvnera`、`AudioNXPP`、`AudioMxic`、`AudioMxicMabelT1`、`AudioMxicMabelT1_ecr`、`AudioApril`、`AudioBillieT1`、`AudioOdessaT2Wireless`、`AudioValkyrie`、`AudioMixer` |
| 命名产品 | `KateT2`、`NommoV2`、`Esther`、`Elsa`、`Layla`、`PWMPCFan` |

---

## 4. 本地化 [HTML] [文案]

- 支持 **10 种语言**：`zh-CN`、`zh-TW`、`en`、`ja`、`kr`、`de`、`es`、`fr`、`ru`、`pt-BR`
- 语言包是**独立分块**（`trans-<locale>.js`），按需加载
- HTML 写死 `lang="en"`，实际语言由前端运行时设置

本项目的对应实现：`locales/zh-CN.json`、`locales/en.json`，
用 `rust-i18n`（gpui-component 内部使用的同一套），见 `src/i18n.rs`。

---

## 5. 与本项目的关系

| 前端事实 | 本项目怎么做 |
|---|---|
| UI 是 React Web 应用 | 用 gpui-kit（Rust 原生）重写 |
| 语言包 10 种 | 复用 `zh-CN` / `en` 原文 |
| 21 个标签页 | `src/nav.rs` 的 `Tab` 枚举，逐项对应 |
| 34 个动作编辑器 | `src/model.rs::BUTTON_ACTIONS`（41 个动作名） |
| `Map*` 的动作名 | 逐字取自主包常量块，见 `src/model.rs` |
| 后端引擎是 DLL | 方案 2：Rust 通过 FFI 直接调用，见 `src/backend/` |

**仍未对上真实界面的**：区块 ↔ 标签页的归属、间距与层级、控件形态。
这些需要读分块里的 JSX（`createElement` 调用）或真实界面截图。
