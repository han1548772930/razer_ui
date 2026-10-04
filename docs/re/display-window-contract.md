# 当前窗口打开契约

Dashboard 通过 `window.open` 打开具名页面；宿主根据策略选择已有窗口内的页签或新的系统窗口。不能仅根据 `windowName` 推断它是第二个系统窗口。下表来自当前 Dashboard 包（`.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js`，SHA-256 `c962f6962441b1b0…`），由 `tools/extract-window-contract.cjs` 静态提取，没有执行下载的 JavaScript。策略的实际执行链见 [宿主策略审计](host-window-policy-current-audit.md)。

## 窗口标志

| 标志 | 值 | 含义 |
| --- | --- | --- |
| `sameWindow` | `policy=3` | 复用当前窗口 |
| `diffWindow` | `policy=5` | 独立窗口 |
| `diffWindowSingleProcess` | `policy=7` | 独立窗口（单进程） |
| `tabVisible` | `tab_visible=1` | 标签可见 |
| `tabInvisible` | `tab_visible=0` | 标签隐藏 |
| `windowVisible` | `browser_visible=1` | 窗口可见 |
| `windowInvisible` | `browser_visible=0` | 窗口隐藏 |
| `autoFocus` | `shouldFocus=1` | 打开后聚焦 |
| `chromaIcon` | `app_icon_path=ChromaApp/icon.ico` | Chroma 窗口图标 |
| `synapseIcon` | `app_icon_path=Synapse/icon.ico` | Synapse 窗口图标 |
| `commonIcon` | `app_icon_path=Common/icon.ico` | 通用图标 |
| `streamerCompanionIcon` | `app_icon_path=StreamerCompanion/icon.ico` | Streamer Companion 图标 |
| `virtualRingIcon` | `app_icon_path=VirtualRingLight/icon.ico` | 虚拟环形灯图标 |

## 窗口名

窗口名由容器或产品与序列号决定，同一设备重复打开会命中同一个窗口：

- `multi-device-pairing-${String(s)}`
- `multi-device-pairing-p${n}-${i}`
- `multi-device-pairing`

打开函数先查询该名字的窗口是否已存在：存在则复用并调整可见性／子页，否则 `window.open(url, name, flags)`。

## 多设备配对窗口

路由 `/synapse/products/${e}/ui/index.html`，参数：

| 参数 | 取值表达式 |
| --- | --- |
| `containerId` | `String(t)` |
| `displayMode` | `"multiDevicePairing"` |
| `allMasters` | `JSON.stringify(s)` |

## 具名应用窗口

Dashboard 模块 54420 用同一张表登记所有具名窗口（窗口名符号在模块 69937 里定义，模块盒名在模块 54693 里定义）：

| 变量 | 窗口名 | 地址 | 打开标志 |
| --- | --- | --- | --- |
| `n` | `profiles` | `/synapse/profiles/` | `policy=3,tab_visible=1` |
| `i` | `feedback-synapse` | `/feedback/?app=synapse&path=+window.location.pathname` | `policy=3,tab_visible=1` |
| `o` | `macro` | `/synapse/macro/` | `policy=3,tab_visible=1` |
| `r` | `alexa` | `/synapse/alexa/` | `policy=3,tab_visible=1` |
| `l` | `background-manager` | `/background-manager/` | `policy=7,browser_visible=0` |
| `d` | `notification-manager` | `/notification-manager/` | `policy=6,always_on_top=1,hide_from_taskbar=1,browser_visible=0,skipTaskbar=true,hasShadow=0,minimum_width=0,minimum_height=0,resizable=0` |
| `c` | `armory` | `/synapse/armory/` | `policy=3,tab_visible=1` |
| `p` | `firmware_update` | `/synapse/update-fw/` | `policy=3,tab_visible=1,shouldFocus=1` |
| `u` | `synapse-introduction` | `/synapse/introduction-tour/` | `policy=3,tab_visible=1` |

隐藏窗口（`browser_visible=0`）：`background-manager`、`notification-manager`。

模块窗口（安装后才打开的独立应用窗口）：`macro`、`alexa`、`armory`、`syn3-profile-migration`、`synapse-introduction`。

## 模块盒点击去向

模块列表里每个盒子点击后聚焦同名窗口；未安装时先打开安装器，再自动打开窗口：

| 盒名 | 窗口名 |
| --- | --- |
| `MACRO` | `macro` |
| `DASHBOARD_ALEXA` | `alexa` |
| `DASHBOARD_WORKSHOP` | `armory` |
| `LINKED_GAMES` | `profiles` |
| `PROFILE_MIGRATION` | `syn3-profile-migration` |
| `FEEDBACK` | `feedback-synapse` |
| `FEEDBACK_ON_BETA` | `feedback-synapse` |
| `TOUR` | `synapse-introduction` |

## 各 displayMode 的参数

| 模式 | 参数 |
| --- | --- |
| `macro` | macro=<id> |
| `chromaApp` | serialNumber=<serial> |
| `armory` | serialNumber=<serial> |
| `multiDevicePairing` | containerId, allMasters, productId/pid, category, canPairTwoDevices |

## 对本仓库的要求

- `policy=3` 模块使用宿主具名页签；确需第二窗口的策略使用系统窗口。两种情形均先按名字查找并聚焦已有页面。多设备配对保留历史明确要求的第二窗口，这是偏离其源 `policy=3` 调用的本地例外。
- 窗口标志决定可见性与聚焦：`sameWindow` 表示复用当前窗口，`diffWindow` 才开新窗口。
- 图标按模式区分（`chromaApp` 用 Chroma 图标），标题与 favicon 由该根自己设置。
- 关闭语义也来自根：例如多设备配对窗口会在配对对象窗口关闭后自行关闭。

逐字段收据见 [机器可读契约](display-window-contract.json)。

重新生成：`node tools/extract-window-contract.cjs`；校验：`node tools/extract-window-contract.cjs --check`。
