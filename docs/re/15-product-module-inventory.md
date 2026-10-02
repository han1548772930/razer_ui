# 产品模块清单与源码取得范围

更新日期：2026-10-02。本页保留旧探测来源，当前取得范围已按官方目录和明确的连接／manifest 声明补查；仅下载和静态解析，未启动应用、执行原版脚本或加载驱动。

最新目录：六份官方清单包含 **449 个主产品 ID**，与旧记录合并为 450 个候选；沿连接别名及 manifest 声明共核查 **596 个 ID**。其中 **331 个产品入口**存在，其资源清单中的 **30,658 份 JS/CSS** 已下载并离线核验。331 个产品在入口或明确 lazy 根组件中找到实际导航数组，共 77 种组合；各入口内部的条件界面仍须继续逐项追踪。完整证据见[官方目录与界面清单](16-product-catalog.md)。

Rust 产品入口已包含 **182、653、777** 及 **3072、3073、3074、3076、3077、3078、3080**。新增七款鼠标垫的本地灯光、帮助和资源见[逐页规格](../screens/17-mouse-mat-lighting.md)，仍有服务和条件界面缺口。代码下载、入口解析、界面实现和服务接通是不同的完成度。

“200”是这些旧探测记录的去重结果，不是官方产品总数，也不是 200 套互不相同的界面。同名产品、配色、键盘布局、接收器、系统设备及内部代号可能对应不同 ID。探测脚本把所有请求异常都跳过，因此记录之外的 ID 也不能直接判定为不支持。

独立应用也已从宿主、主前端和 Settings 的明确路径继续取得，当前逐条结果见[独立应用目录](17-application-catalog.md)。新快照保存在 `.ref/applications`，原先用于适配的 `.ref/frontend` 等资料保持原版本，避免混用不同版本的模块编号或哈希。

## 统计证据与边界

| 原始记录 | 成功记录数 | 成功记录 ID 最小—最大 |
| --- | ---: | --- |
| [低段记录](../../.ref/notes/razer-products-low.txt) | 79 | 70—241 |
| [中段记录](../../.ref/notes/razer-products.txt) | 90 | 515—697 |
| [高段记录](../../.ref/notes/razer-products-high.txt) | 31 | 708—791 |
| 合计／按 ID 去重 | **200／200** | **70—791** |

原始脚本是 [scan-products.ps1](../../.ref/tools/scan-products.ps1)，请求路径为 `https://apps.razer.com/synapse/products/<ID>/ui/manifest.json`，只保存 ID、deviceName、short_name 和首个 icon URL。记录没有保留每次执行的完整参数和失败原因；表中端点是成功记录范围，不是已证明的完整扫描范围。[fetch-device-module.ps1](../../.ref/tools/fetch-device-module.ps1) 是产品代码下载工具，其存在不代表已经下载所有产品。

## 已适配范围与独立应用的代码

以下数量按各自 `asset-manifest.json` 中以 `.js` 或 `.css` 结尾的路径计数，包含清单列出的 service-worker；逐条核对本地路径均存在。它们不是页面数量，也不是所有图片、字体、视频、source map 均完整的声明。

| 源码范围 | JS | CSS | JS/CSS 路径齐备 | 页面边界 |
| --- | ---: | ---: | --- | --- |
| [.ref/frontend](../../.ref/frontend/asset-manifest.json) | 115 | 12 | 是 | 主应用四页、共享 App/Header、IotPopupRoot、Dashboard 配对及共享映射等 |
| [.ref/settings](../../.ref/settings/asset-manifest.json) | 19 | 1 | 是 | 原版 Synapse / General 两页；[下载记录](../../.ref/settings/source.json) |
| [产品 182](../../.ref/devices/182/asset-manifest.json) | 110 | 8 | 是 | Razer DeathAdder V3 Pro |
| [产品 653](../../.ref/devices/653/asset-manifest.json) | 204 | 8 | 是 | Blackwidow V4 Pro |
| [产品 777](../../.ref/devices/777/asset-manifest.json) | 45 | 1 | 是 | RAZER KRAKEN BT SANRIO LIMITED EDITION |
| [Profile Migration](../../.ref/profile-migration/asset-manifest.json) | 58 | 1 | 是 | 独立迁移单页及状态弹层；[下载记录](../../.ref/profile-migration/source.json)、[界面规格](../screens/13-profile-migration.md) |
| [账户菜单](../../.ref/rz-user-profile-menu/asset-manifest.json) | 3 | 1 | 是 | 独立头像菜单；[下载记录](../../.ref/rz-user-profile-menu/source.json)、[界面规格](../screens/15-account-menu.md) |

本地另有 `.ref/background-manager/assets/index-8d39b3d5.js` 单个 bundle、`.ref/release-patch-note` 的入口及 main JS/CSS，以及解包的 `.ref/synapse-asar` 宿主代码。这些资料各自提供服务、窗口入口或界面证据，不能计作更多已完整取得的产品 UI。Settings 清单使用 `/synapse/settings/` 绝对 URL 前缀，核对本地文件时已去除该前缀。

上表保留最初三款产品及独立应用资料；新增七款鼠标垫见[逐页规格](../screens/17-mouse-mat-lighting.md)，其余 321 个产品的 JS/CSS 取得情况见最新目录。原宿主还声明 Chroma、Streamer Companion、THX Spatial Audio、Virtual Ring Light、Cortex、7.1 Surround Sound、Razer Settings 等独立应用路由，详见[子应用与路由](01-ipc-api-surface.md#18-razer-sub-applications-and-routes)。已知启动入口不等于已取得目标应用的完整渲染源码。头像菜单 `/rz-user-profile-menu/` 的入口与清单所列 JS/CSS 已取得；账户会话与反馈宿主仍未接入。

## 已取得源码的入口复核

以下按本轮当前代码更新；缺服务与缺界面分别记录。

| 范围 | 原版挂载证据 | 可行动项 |
| --- | --- | --- |
| 添加 Wi-Fi 设备总入口 | `IotPopupRoot.290be417.chunk.js`，28256 的 `gt` 读取 `iotPopupType`，默认 `GENERAL` | 已接 Dashboard 添加卡及 Gamer Room／Key Light 分支选择、返回路径 |
| Key Light 添加 | 同一 bundle 的 `rt` | 准备、扫码、扫描列表、新设备、网络选择、密码显示与错误、连接进度、成功、无网络及设备异常均有明确状态界面；真实扫描未接入，见[Wi-Fi 添加](../screens/14-iot-add.md) |
| Gamer Room 添加 | 同一 bundle 的 `dt → ze/at/st` | 已补设备列表卡、成功、无网络及分区为空的显式预览；二维码返回准备页，扫描页按有无新设备决定取消/返回 |
| Gamer Room 已连接卡 | `9388.2bec5db3.chunk.js` 的 `je → ue → ne/fe → Q/S` | 补在线／离线／断电／安装卡、两种接管分组、接管开关及卡旁详情弹层；不能只用空组说明代替 |
| 主应用共享 Header | `App.eb32d7cd.chunk.js` 的 Home `x.render`、Header 子组件 | 补兼容模式警告 `T`、离线下拉、待重启更新提示；头像外层入口与独立菜单源码取得范围分开 |
| Dashboard 多设备配对入口 | `4130.155387bf.chunk.js` 的 `box-multi-paring` | 已有独立配对状态机；继续核对 Dashboard 实际入口和绑定状态，仅有 Settings 入口不代表原入口完整 |
| Settings | `720.1e5d1c8f.chunk.js` 的 `ho/uo` | 原版只有 Synapse／General 两页；项目额外“服务连接”页不计入原版覆盖。迁移、灯光、更新说明等条件面板分别核对 |

Key Light 有新设备入网和密码输入；Gamer Room 从既有网络设备列表添加。两者状态集合不同，不能直接套用同一入网流程。

## 探测记录的类别分布

类别仅取原 manifest 首个 icon 路径的文件名，**不是完整硬件分类字段**。空值和 `productCategoryIcon` 是原记录缺失／占位信息，不能自行猜成某种产品。

| 首个 icon 文件名 | 产品 ID 数 |
| --- | ---: |
| 空值 | 8 |
| ACCESSORY | 4 |
| ACCESSORY_MAINBOARD | 1 |
| AUDIO | 1 |
| HUE | 1 |
| KEYBOARD | 66 |
| KEYPAD | 3 |
| MOUSE | 71 |
| MOUSEMAT | 1 |
| MOUSEPLUSMAT | 2 |
| SYSTEM | 41 |
| productCategoryIcon | 1 |
| **合计** | **200** |

## 旧版 200 条成功探测记录

名称优先使用原 `deviceName`，为空时展示原 `short_name`；内部代号保持原样。产品名称未作推测或扩展。“JS/CSS 已取得”不表示业务与硬件服务已实现。

| 产品 ID | 原名称／short_name | icon 类别 | 本地取得状态 |
| --- | --- | --- | --- |
| [70](https://apps.razer.com/synapse/products/70/ui/manifest.json) | Razer Mamba TE | MOUSE | JS/CSS 已取得，未适配 |
| [80](https://apps.razer.com/synapse/products/80/ui/manifest.json) | Razer Naga Hex V2 | MOUSE | JS/CSS 已取得，未适配 |
| [83](https://apps.razer.com/synapse/products/83/ui/manifest.json) | Razer Naga Chroma | MOUSE | JS/CSS 已取得，未适配 |
| [89](https://apps.razer.com/synapse/products/89/ui/manifest.json) | Razer Lancehead | MOUSE | JS/CSS 已取得，未适配 |
| [92](https://apps.razer.com/synapse/products/92/ui/manifest.json) | Razer Deathadder Elite | MOUSE | JS/CSS 已取得，未适配 |
| [96](https://apps.razer.com/synapse/products/96/ui/manifest.json) | Razer Lancehead TE | MOUSE | JS/CSS 已取得，未适配 |
| [98](https://apps.razer.com/synapse/products/98/ui/manifest.json) | Razer Atheris | MOUSE | JS/CSS 已取得，未适配 |
| [99](https://apps.razer.com/synapse/products/99/ui/manifest.json) | Jugan_UI | MOUSE | JS/CSS 已取得，未适配 |
| [100](https://apps.razer.com/synapse/products/100/ui/manifest.json) | Razer Basilisk | MOUSE | JS/CSS 已取得，未适配 |
| [101](https://apps.razer.com/synapse/products/101/ui/manifest.json) | Razer Basilisk | MOUSE | JS/CSS 已取得，未适配 |
| [103](https://apps.razer.com/synapse/products/103/ui/manifest.json) | RAZER NAGA TRINITY | MOUSE | JS/CSS 已取得，未适配 |
| [104](https://apps.razer.com/synapse/products/104/ui/manifest.json) | Razer Mamba Hyperflux | MOUSEPLUSMAT | JS/CSS 已取得，未适配 |
| [105](https://apps.razer.com/synapse/products/105/ui/manifest.json) | Razer Mamba Hyperflux | MOUSEPLUSMAT | JS/CSS 已取得，未适配 |
| [106](https://apps.razer.com/synapse/products/106/ui/manifest.json) | D.VA Razer Abyssus Elite | MOUSE | JS/CSS 已取得，未适配 |
| [107](https://apps.razer.com/synapse/products/107/ui/manifest.json) | Abyssusessential_UI | MOUSE | JS/CSS 已取得，未适配 |
| [108](https://apps.razer.com/synapse/products/108/ui/manifest.json) | Razer Mamba Elite | MOUSE | JS/CSS 已取得，未适配 |
| [110](https://apps.razer.com/synapse/products/110/ui/manifest.json) | Razer DeathAdder Essential | MOUSE | JS/CSS 已取得，未适配 |
| [112](https://apps.razer.com/synapse/products/112/ui/manifest.json) | Razer Lancehead Wireless | MOUSE | JS/CSS 已取得，未适配 |
| [113](https://apps.razer.com/synapse/products/113/ui/manifest.json) | Razer DeathAdder Essential | MOUSE | JS/CSS 已取得，未适配 |
| [115](https://apps.razer.com/synapse/products/115/ui/manifest.json) | Razer Mamaba Wireless | MOUSE | JS/CSS 已取得，未适配 |
| [116](https://apps.razer.com/synapse/products/116/ui/manifest.json) | Gabbielite_UI | MOUSE | JS/CSS 已取得，未适配 |
| [117](https://apps.razer.com/synapse/products/117/ui/manifest.json) | Razer Turret Mouse Xbox One Edition | MOUSE | JS/CSS 已取得，未适配 |
| [120](https://apps.razer.com/synapse/products/120/ui/manifest.json) | Razer Viper | MOUSE | JS/CSS 已取得，未适配 |
| [122](https://apps.razer.com/synapse/products/122/ui/manifest.json) | Razer Viper Ultimate | MOUSE | JS/CSS 已取得，未适配 |
| [124](https://apps.razer.com/synapse/products/124/ui/manifest.json) | Razer DeathAdder V2 Pro | MOUSE | JS/CSS 已取得，未适配 |
| [126](https://apps.razer.com/synapse/products/126/ui/manifest.json) | Razer Mouse Dock | ACCESSORY | JS/CSS 已取得，未适配 |
| [128](https://apps.razer.com/synapse/products/128/ui/manifest.json) | Razer Pro Click | MOUSE | JS/CSS 已取得，未适配 |
| [131](https://apps.razer.com/synapse/products/131/ui/manifest.json) | Razer Basilisk X Hyperspeed | MOUSE | JS/CSS 已取得，未适配 |
| [132](https://apps.razer.com/synapse/products/132/ui/manifest.json) | RAZER DEATHADDER V2 | MOUSE | JS/CSS 已取得，未适配 |
| [133](https://apps.razer.com/synapse/products/133/ui/manifest.json) | Razer Basilisk V2 | MOUSE | JS/CSS 已取得，未适配 |
| [134](https://apps.razer.com/synapse/products/134/ui/manifest.json) | Razer Basilisk Ultimate | MOUSE | JS/CSS 已取得，未适配 |
| [138](https://apps.razer.com/synapse/products/138/ui/manifest.json) | Razer Viper Mini | MOUSE | JS/CSS 已取得，未适配 |
| [140](https://apps.razer.com/synapse/products/140/ui/manifest.json) | Razer Deathadder V2 Lite | MOUSE | JS/CSS 已取得，未适配 |
| [141](https://apps.razer.com/synapse/products/141/ui/manifest.json) | Razer Naga Left Handed Edition | MOUSE | JS/CSS 已取得，未适配 |
| [143](https://apps.razer.com/synapse/products/143/ui/manifest.json) | RAZER NAGA PRO | MOUSE | JS/CSS 已取得，未适配 |
| [145](https://apps.razer.com/synapse/products/145/ui/manifest.json) | Razer Viper 8Khz | MOUSE | JS/CSS 已取得，未适配 |
| [147](https://apps.razer.com/synapse/products/147/ui/manifest.json) | RAZER NAGA CLASSIC EDITION | MOUSE | JS/CSS 已取得，未适配 |
| [148](https://apps.razer.com/synapse/products/148/ui/manifest.json) | Razer Orochi V2 | MOUSE | JS/CSS 已取得，未适配 |
| [150](https://apps.razer.com/synapse/products/150/ui/manifest.json) | Razer Naga X | MOUSE | JS/CSS 已取得，未适配 |
| [152](https://apps.razer.com/synapse/products/152/ui/manifest.json) | Razer DeathAdder Essential | MOUSE | JS/CSS 已取得，未适配 |
| [153](https://apps.razer.com/synapse/products/153/ui/manifest.json) | Razer Basilisk V3 | MOUSE | JS/CSS 已取得，未适配 |
| [154](https://apps.razer.com/synapse/products/154/ui/manifest.json) | Alma_UI | MOUSE | JS/CSS 已取得，未适配 |
| [156](https://apps.razer.com/synapse/products/156/ui/manifest.json) | Razer DeathAdder V2 X Hyperspeed | MOUSE | JS/CSS 已取得，未适配 |
| [158](https://apps.razer.com/synapse/products/158/ui/manifest.json) | Razer Viper Mini Signature Edition | MOUSE | JS/CSS 已取得，未适配 |
| [161](https://apps.razer.com/synapse/products/161/ui/manifest.json) | Razer Deathadder V2 Lite | MOUSE | JS/CSS 已取得，未适配 |
| [162](https://apps.razer.com/synapse/products/162/ui/manifest.json) | cobra | productCategoryIcon | JS/CSS 已取得，未适配 |
| [163](https://apps.razer.com/synapse/products/163/ui/manifest.json) | Razer Cobra | MOUSE | JS/CSS 已取得，未适配 |
| [164](https://apps.razer.com/synapse/products/164/ui/manifest.json) | Razer Mouse Dock Pro | ACCESSORY | JS/CSS 已取得，未适配 |
| [165](https://apps.razer.com/synapse/products/165/ui/manifest.json) | RAZER VIPER V2 PRO | MOUSE | JS/CSS 已取得，未适配 |
| [167](https://apps.razer.com/synapse/products/167/ui/manifest.json) | RAZER NAGA V2 PRO | MOUSE | JS/CSS 已取得，未适配 |
| [170](https://apps.razer.com/synapse/products/170/ui/manifest.json) | Razer Basilisk V3 Pro | MOUSE | JS/CSS 已取得，未适配 |
| [175](https://apps.razer.com/synapse/products/175/ui/manifest.json) | Razer Cobra Pro | MOUSE | JS/CSS 已取得，未适配 |
| [178](https://apps.razer.com/synapse/products/178/ui/manifest.json) | Razer DeathAdder V3 | MOUSE | JS/CSS 已取得，未适配 |
| [179](https://apps.razer.com/synapse/products/179/ui/manifest.json) | HyperPolling Wireless Dongle | ACCESSORY | JS/CSS 已取得，未适配 |
| [180](https://apps.razer.com/synapse/products/180/ui/manifest.json) | Razer Naga V2 Hyperspeed | MOUSE | JS/CSS 已取得，未适配 |
| [182](https://apps.razer.com/synapse/products/182/ui/manifest.json) | Razer DeathAdder V3 Pro | MOUSE | JS/CSS 已取得 |
| [184](https://apps.razer.com/synapse/products/184/ui/manifest.json) | Razer Viper V3 HyperSpeed | MOUSE | JS/CSS 已取得，未适配 |
| [185](https://apps.razer.com/synapse/products/185/ui/manifest.json) | Razer Basilisk V3 X Hyperspeed | MOUSE | JS/CSS 已取得，未适配 |
| [190](https://apps.razer.com/synapse/products/190/ui/manifest.json) | Razer DeathAdder V4 Pro | MOUSE | JS/CSS 已取得，未适配 |
| [192](https://apps.razer.com/synapse/products/192/ui/manifest.json) | Razer Viper V3 Pro | MOUSE | JS/CSS 已取得，未适配 |
| [194](https://apps.razer.com/synapse/products/194/ui/manifest.json) | Razer DeathAdder V3 Pro Hyperpolling Technology | MOUSE | JS/CSS 已取得，未适配 |
| [196](https://apps.razer.com/synapse/products/196/ui/manifest.json) | Razer DeathAdder V3 HyperSpeed | MOUSE | JS/CSS 已取得，未适配 |
| [199](https://apps.razer.com/synapse/products/199/ui/manifest.json) | eden_ui | MOUSE | JS/CSS 已取得，未适配 |
| [203](https://apps.razer.com/synapse/products/203/ui/manifest.json) | Razer Basilisk V3 35K | MOUSE | JS/CSS 已取得，未适配 |
| [204](https://apps.razer.com/synapse/products/204/ui/manifest.json) | Razer Basilisk V3 Pro 35K | MOUSE | JS/CSS 已取得，未适配 |
| [207](https://apps.razer.com/synapse/products/207/ui/manifest.json) | HyperFlux V2 Wireless Charging System | MOUSEMAT | JS/CSS 已取得，未适配 |
| [208](https://apps.razer.com/synapse/products/208/ui/manifest.json) | evelynrefresh_ui | MOUSE | JS/CSS 已取得，未适配 |
| [211](https://apps.razer.com/synapse/products/211/ui/manifest.json) | Razer Basilisk Mobile | MOUSE | JS/CSS 已取得，未适配 |
| [214](https://apps.razer.com/synapse/products/214/ui/manifest.json) | Razer Basilisk V3 Pro 35K Phantom Green Edition | MOUSE | JS/CSS 已取得，未适配 |
| [218](https://apps.razer.com/synapse/products/218/ui/manifest.json) | Razer Cobra HyperSpeed | MOUSE | JS/CSS 已取得，未适配 |
| [221](https://apps.razer.com/synapse/products/221/ui/manifest.json) | Razer Boomslang 20th Anniversary Edition | MOUSE | JS/CSS 已取得，未适配 |
| [222](https://apps.razer.com/synapse/products/222/ui/manifest.json) | Razer Viper V3 Pro SE | MOUSE | JS/CSS 已取得，未适配 |
| [224](https://apps.razer.com/synapse/products/224/ui/manifest.json) | Razer Orochi V2 | MOUSE | JS/CSS 已取得，未适配 |
| [226](https://apps.razer.com/synapse/products/226/ui/manifest.json) | Razer Basilisk V4 Pro | MOUSE | JS/CSS 已取得，未适配 |
| [229](https://apps.razer.com/synapse/products/229/ui/manifest.json) | Razer Viper V4 Pro | MOUSE | JS/CSS 已取得，未适配 |
| [231](https://apps.razer.com/synapse/products/231/ui/manifest.json) | RAZER NAGA V3 PRO | MOUSE | JS/CSS 已取得，未适配 |
| [235](https://apps.razer.com/synapse/products/235/ui/manifest.json) | Razer Basilisk V4 HyperSpeed | MOUSE | JS/CSS 已取得，未适配 |
| [239](https://apps.razer.com/synapse/products/239/ui/manifest.json) | Razer DeathAdder V4 Pro Carbon Fiber Edition | MOUSE | JS/CSS 已取得，未适配 |
| [241](https://apps.razer.com/synapse/products/241/ui/manifest.json) | Razer Mouse Dock V2 Pro | ACCESSORY | JS/CSS 已取得，未适配 |
| [515](https://apps.razer.com/synapse/products/515/ui/manifest.json) | Razer Blackwidow Chroma | KEYBOARD | JS/CSS 已取得，未适配 |
| [521](https://apps.razer.com/synapse/products/521/ui/manifest.json) | Bwtechroma_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [529](https://apps.razer.com/synapse/products/529/ui/manifest.json) | Bwchromaow_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [534](https://apps.razer.com/synapse/products/534/ui/manifest.json) | Blackwidowxchroma_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [542](https://apps.razer.com/synapse/products/542/ui/manifest.json) | Carolinechroma_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [545](https://apps.razer.com/synapse/products/545/ui/manifest.json) | Blackwidowchromav2_UI | 空值 | JS/CSS 已取得，未适配 |
| [550](https://apps.razer.com/synapse/products/550/ui/manifest.json) | Jamiet1_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [551](https://apps.razer.com/synapse/products/551/ui/manifest.json) | Jamiet2_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [552](https://apps.razer.com/synapse/products/552/ui/manifest.json) | Razer Blackwidow Elite | KEYBOARD | JS/CSS 已取得，未适配 |
| [554](https://apps.razer.com/synapse/products/554/ui/manifest.json) | Cynosachroma_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [555](https://apps.razer.com/synapse/products/555/ui/manifest.json) | Tartarusv2_UI | KEYPAD | JS/CSS 已取得，未适配 |
| [556](https://apps.razer.com/synapse/products/556/ui/manifest.json) | CynosachromaPro_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [563](https://apps.razer.com/synapse/products/563/ui/manifest.json) | Charlotte_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [564](https://apps.razer.com/synapse/products/564/ui/manifest.json) | Dana17-2_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [565](https://apps.razer.com/synapse/products/565/ui/manifest.json) | Victoriat3_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [567](https://apps.razer.com/synapse/products/567/ui/manifest.json) | Razer Blackwidow Essential | KEYBOARD | JS/CSS 已取得，未适配 |
| [569](https://apps.razer.com/synapse/products/569/ui/manifest.json) | Lynette_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [570](https://apps.razer.com/synapse/products/570/ui/manifest.json) | Charlotte2_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [571](https://apps.razer.com/synapse/products/571/ui/manifest.json) | Dana_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [574](https://apps.razer.com/synapse/products/574/ui/manifest.json) | Turret Keyboard Xbox One Edition | KEYBOARD | JS/CSS 已取得，未适配 |
| [575](https://apps.razer.com/synapse/products/575/ui/manifest.json) | Cynosalite_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [576](https://apps.razer.com/synapse/products/576/ui/manifest.json) | Charlottese_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [577](https://apps.razer.com/synapse/products/577/ui/manifest.json) | Janett2_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [579](https://apps.razer.com/synapse/products/579/ui/manifest.json) | Huntsmante_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [580](https://apps.razer.com/synapse/products/580/ui/manifest.json) | Tartarus Pro | KEYPAD | JS/CSS 已取得，未适配 |
| [581](https://apps.razer.com/synapse/products/581/ui/manifest.json) | Charlotte3_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [582](https://apps.razer.com/synapse/products/582/ui/manifest.json) | Dana3_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [585](https://apps.razer.com/synapse/products/585/ui/manifest.json) | Ida_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [586](https://apps.razer.com/synapse/products/586/ui/manifest.json) | Razer Blade Stealth 13 | SYSTEM | JS/CSS 已取得，未适配 |
| [587](https://apps.razer.com/synapse/products/587/ui/manifest.json) | Razer Blade 15 Advanced Model | SYSTEM | JS/CSS 已取得，未适配 |
| [588](https://apps.razer.com/synapse/products/588/ui/manifest.json) | Dana17-3_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [589](https://apps.razer.com/synapse/products/589/ui/manifest.json) | Razer Blade 15 Studio Edition | SYSTEM | JS/CSS 已取得，未适配 |
| [590](https://apps.razer.com/synapse/products/590/ui/manifest.json) | Janett2v2_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [591](https://apps.razer.com/synapse/products/591/ui/manifest.json) | Janett3v2_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [592](https://apps.razer.com/synapse/products/592/ui/manifest.json) | Jamiet3_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [593](https://apps.razer.com/synapse/products/593/ui/manifest.json) | Ornatapokemon_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [594](https://apps.razer.com/synapse/products/594/ui/manifest.json) | Razer Blade Stealth 13 Base Model | SYSTEM | JS/CSS 已取得，未适配 |
| [595](https://apps.razer.com/synapse/products/595/ui/manifest.json) | Charlotte5_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [597](https://apps.razer.com/synapse/products/597/ui/manifest.json) | Razer Blade 15 Base Model | SYSTEM | JS/CSS 已取得，未适配 |
| [598](https://apps.razer.com/synapse/products/598/ui/manifest.json) | Dana17-5_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [599](https://apps.razer.com/synapse/products/599/ui/manifest.json) | Mayamini_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [600](https://apps.razer.com/synapse/products/600/ui/manifest.json) | Razer BlackWidow V3 Mini HyperSpeed | KEYBOARD | JS/CSS 已取得，未适配 |
| [601](https://apps.razer.com/synapse/products/601/ui/manifest.json) | Lynette5_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [602](https://apps.razer.com/synapse/products/602/ui/manifest.json) | Blackwidow V3 Pro | KEYBOARD | JS/CSS 已取得，未适配 |
| [605](https://apps.razer.com/synapse/products/605/ui/manifest.json) | Razer Ornata V2 | KEYBOARD | JS/CSS 已取得，未适配 |
| [606](https://apps.razer.com/synapse/products/606/ui/manifest.json) | RAZER CYNOSA V2 | KEYBOARD | JS/CSS 已取得，未适配 |
| [614](https://apps.razer.com/synapse/products/614/ui/manifest.json) | Jamiet1analog_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [616](https://apps.razer.com/synapse/products/616/ui/manifest.json) | Razer Blade 15 Base Model | SYSTEM | JS/CSS 已取得，未适配 |
| [617](https://apps.razer.com/synapse/products/617/ui/manifest.json) | Mayaminijp_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [618](https://apps.razer.com/synapse/products/618/ui/manifest.json) | Margaret_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [619](https://apps.razer.com/synapse/products/619/ui/manifest.json) | Huntsmantev2_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [620](https://apps.razer.com/synapse/products/620/ui/manifest.json) | Razer Huntsman V2 | KEYBOARD | JS/CSS 已取得，未适配 |
| [621](https://apps.razer.com/synapse/products/621/ui/manifest.json) | Charlotte6_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [622](https://apps.razer.com/synapse/products/622/ui/manifest.json) | Dana17-6_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [623](https://apps.razer.com/synapse/products/623/ui/manifest.json) | Razer Blade 15 Base Model | SYSTEM | JS/CSS 已取得，未适配 |
| [624](https://apps.razer.com/synapse/products/624/ui/manifest.json) | Piperan_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [630](https://apps.razer.com/synapse/products/630/ui/manifest.json) | Charlotte7_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [631](https://apps.razer.com/synapse/products/631/ui/manifest.json) | Idav2_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [633](https://apps.razer.com/synapse/products/633/ui/manifest.json) | Dana17-7_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [634](https://apps.razer.com/synapse/products/634/ui/manifest.json) | Dana7_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [642](https://apps.razer.com/synapse/products/642/ui/manifest.json) | Mayaminianalog_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [647](https://apps.razer.com/synapse/products/647/ui/manifest.json) | Selenat2_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [650](https://apps.razer.com/synapse/products/650/ui/manifest.json) | Charlotte8_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [651](https://apps.razer.com/synapse/products/651/ui/manifest.json) | Dana17-8_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [652](https://apps.razer.com/synapse/products/652/ui/manifest.json) | Piper8_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [653](https://apps.razer.com/synapse/products/653/ui/manifest.json) | Blackwidow V4 Pro | KEYBOARD | JS/CSS 已取得 |
| [654](https://apps.razer.com/synapse/products/654/ui/manifest.json) | Cynosapro_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [655](https://apps.razer.com/synapse/products/655/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | JS/CSS 已取得，未适配 |
| [658](https://apps.razer.com/synapse/products/658/ui/manifest.json) | Razer DeathStalker V2 Pro | KEYBOARD | JS/CSS 已取得，未适配 |
| [659](https://apps.razer.com/synapse/products/659/ui/manifest.json) | Selenat3_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [660](https://apps.razer.com/synapse/products/660/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | JS/CSS 已取得，未适配 |
| [661](https://apps.razer.com/synapse/products/661/ui/manifest.json) | Razer DeathStalker V2 Pro | KEYBOARD | JS/CSS 已取得，未适配 |
| [664](https://apps.razer.com/synapse/products/664/ui/manifest.json) | Razer DeathStalker V2 Pro Tenkeyless | KEYBOARD | JS/CSS 已取得，未适配 |
| [669](https://apps.razer.com/synapse/products/669/ui/manifest.json) | Piper9_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [670](https://apps.razer.com/synapse/products/670/ui/manifest.json) | Charlotte9_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [671](https://apps.razer.com/synapse/products/671/ui/manifest.json) | Razer Blade 16 | SYSTEM | JS/CSS 已取得，未适配 |
| [672](https://apps.razer.com/synapse/products/672/ui/manifest.json) | Razer Blade 18 | SYSTEM | JS/CSS 已取得，未适配 |
| [673](https://apps.razer.com/synapse/products/673/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | JS/CSS 已取得，未适配 |
| [674](https://apps.razer.com/synapse/products/674/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | JS/CSS 已取得，未适配 |
| [675](https://apps.razer.com/synapse/products/675/ui/manifest.json) | Razer Ornata V3 Tenkeyless | KEYBOARD | JS/CSS 已取得，未适配 |
| [677](https://apps.razer.com/synapse/products/677/ui/manifest.json) | Selenat2_75_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [678](https://apps.razer.com/synapse/products/678/ui/manifest.json) | Taliat1_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [679](https://apps.razer.com/synapse/products/679/ui/manifest.json) | Taliat1tkl_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [688](https://apps.razer.com/synapse/products/688/ui/manifest.json) | Taliat160_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [689](https://apps.razer.com/synapse/products/689/ui/manifest.json) | RAZER HUNTSMAN V3 X TENKEYLESS | KEYBOARD | JS/CSS 已取得，未适配 |
| [691](https://apps.razer.com/synapse/products/691/ui/manifest.json) | Blackwidow V4 Pro 75% | KEYBOARD | JS/CSS 已取得，未适配 |
| [694](https://apps.razer.com/synapse/products/694/ui/manifest.json) | Razer Blade 14 | SYSTEM | JS/CSS 已取得，未适配 |
| [695](https://apps.razer.com/synapse/products/695/ui/manifest.json) | Razer Blade 16 | SYSTEM | JS/CSS 已取得，未适配 |
| [696](https://apps.razer.com/synapse/products/696/ui/manifest.json) | Razer Blade 18 | SYSTEM | JS/CSS 已取得，未适配 |
| [697](https://apps.razer.com/synapse/products/697/ui/manifest.json) | Razer BlackWidow V3 Mini HyperSpeed | KEYBOARD | JS/CSS 已取得，未适配 |
| [708](https://apps.razer.com/synapse/products/708/ui/manifest.json) | Razer Pro Type Ergo | KEYBOARD | JS/CSS 已取得，未适配 |
| [709](https://apps.razer.com/synapse/products/709/ui/manifest.json) | Piper11_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [710](https://apps.razer.com/synapse/products/710/ui/manifest.json) | Sonya11_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [711](https://apps.razer.com/synapse/products/711/ui/manifest.json) | Kira11_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [716](https://apps.razer.com/synapse/products/716/ui/manifest.json) | Razer BlackWidow V4 Low-profile HyperSpeed | KEYBOARD | JS/CSS 已取得，未适配 |
| [717](https://apps.razer.com/synapse/products/717/ui/manifest.json) | Razer Joro | KEYBOARD | JS/CSS 已取得，未适配 |
| [719](https://apps.razer.com/synapse/products/719/ui/manifest.json) | Talia T1 Refresh UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [720](https://apps.razer.com/synapse/products/720/ui/manifest.json) | Talia T1 TKL Refresh UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [721](https://apps.razer.com/synapse/products/721/ui/manifest.json) | TaliaT1_60%_Refresh_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [724](https://apps.razer.com/synapse/products/724/ui/manifest.json) | BlackWidow V4 Low-profile Tenkeyless HyperSpeed | KEYBOARD | JS/CSS 已取得，未适配 |
| [727](https://apps.razer.com/synapse/products/727/ui/manifest.json) | Razer BlackWidow V4 Tenkeyless HyperSpeed | KEYBOARD | JS/CSS 已取得，未适配 |
| [728](https://apps.razer.com/synapse/products/728/ui/manifest.json) | Razer Huntsman Signature Editon | KEYBOARD | JS/CSS 已取得，未适配 |
| [736](https://apps.razer.com/synapse/products/736/ui/manifest.json) | Sonya12_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [737](https://apps.razer.com/synapse/products/737/ui/manifest.json) | Kira12_UI | SYSTEM | JS/CSS 已取得，未适配 |
| [739](https://apps.razer.com/synapse/products/739/ui/manifest.json) | Razer Reclusa X Mini 65% | KEYBOARD | JS/CSS 已取得，未适配 |
| [740](https://apps.razer.com/synapse/products/740/ui/manifest.json) | Razer Huntsman V3 HE Magnetic Mini 65% 8KHz | KEYBOARD | JS/CSS 已取得，未适配 |
| [741](https://apps.razer.com/synapse/products/741/ui/manifest.json) | Maya T2 TKL SE UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [742](https://apps.razer.com/synapse/products/742/ui/manifest.json) | RAZER HUNTSMAN V3 PRO LOW-PROFILE TENKEYLESS 8KHZ | KEYBOARD | JS/CSS 已取得，未适配 |
| [746](https://apps.razer.com/synapse/products/746/ui/manifest.json) | Razer Huntsman V3 HE Magnetic Tenkeyless 8KHz | KEYBOARD | JS/CSS 已取得，未适配 |
| [747](https://apps.razer.com/synapse/products/747/ui/manifest.json) | Tartarus Pro | KEYPAD | JS/CSS 已取得，未适配 |
| [752](https://apps.razer.com/synapse/products/752/ui/manifest.json) | Selenat2_75_UI | KEYBOARD | JS/CSS 已取得，未适配 |
| [769](https://apps.razer.com/synapse/products/769/ui/manifest.json) | Razer Philips Hue | HUE | JS/CSS 已取得，未适配 |
| [777](https://apps.razer.com/synapse/products/777/ui/manifest.json) | RAZER KRAKEN BT SANRIO LIMITED EDITION | AUDIO | JS/CSS 已取得 |
| [778](https://apps.razer.com/synapse/products/778/ui/manifest.json) | Asrock_UI | ACCESSORY_MAINBOARD | JS/CSS 已取得，未适配 |
| [780](https://apps.razer.com/synapse/products/780/ui/manifest.json) | Jadet1_UI | 空值 | JS/CSS 已取得，未适配 |
| [781](https://apps.razer.com/synapse/products/781/ui/manifest.json) | Gilliant1_UI | 空值 | JS/CSS 已取得，未适配 |
| [782](https://apps.razer.com/synapse/products/782/ui/manifest.json) | Gilliant2_UI | 空值 | JS/CSS 已取得，未适配 |
| [783](https://apps.razer.com/synapse/products/783/ui/manifest.json) | Paige_UI | 空值 | JS/CSS 已取得，未适配 |
| [784](https://apps.razer.com/synapse/products/784/ui/manifest.json) | Joanna_UI | 空值 | JS/CSS 已取得，未适配 |
| [790](https://apps.razer.com/synapse/products/790/ui/manifest.json) | Tiana_UI | 空值 | JS/CSS 已取得，未适配 |
| [791](https://apps.razer.com/synapse/products/791/ui/manifest.json) | miriam-config | 空值 | JS/CSS 已取得，未适配 |
