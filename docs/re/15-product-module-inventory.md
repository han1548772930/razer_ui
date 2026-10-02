# 产品模块清单与源码取得范围

更新日期：2026-10-02。本页按本地原始探测记录和实际目录核对；没有启动应用、执行原版脚本或重新批量探测产品。

原版不止三个产品。三份 CDN 探测记录共保存了 **200 个不同产品 ID** 的成功 manifest 响应，成功记录的最小／最大 ID 是 **70／791**。当前完整取得入口及 manifest 所列 JS/CSS 的产品模块只有 **182、653、777**；其余 197 个产品在本项目中只有探测记录，不能据此声称已取得或已实现它们的页面。

“200”是这些旧探测记录的去重结果，不是官方产品总数，也不是 200 套互不相同的界面。同名产品、配色、键盘布局、接收器、系统设备及内部代号可能对应不同 ID。探测脚本把所有请求异常都跳过，因此记录之外的 ID 也不能直接判定为不支持。

## 统计证据与边界

| 原始记录 | 成功记录数 | 成功记录 ID 最小—最大 |
| --- | ---: | --- |
| [低段记录](../../.ref/notes/razer-products-low.txt) | 79 | 70—241 |
| [中段记录](../../.ref/notes/razer-products.txt) | 90 | 515—697 |
| [高段记录](../../.ref/notes/razer-products-high.txt) | 31 | 708—791 |
| 合计／按 ID 去重 | **200／200** | **70—791** |

原始脚本是 [scan-products.ps1](../../.ref/tools/scan-products.ps1)，请求路径为 `https://apps.razer.com/synapse/products/<ID>/ui/manifest.json`，只保存 ID、deviceName、short_name 和首个 icon URL。记录没有保留每次执行的完整参数和失败原因；表中端点是成功记录范围，不是已证明的完整扫描范围。[fetch-device-module.ps1](../../.ref/tools/fetch-device-module.ps1) 是产品代码下载工具，其存在不代表已经下载所有产品。

## 实际取得的代码

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

原宿主还声明 Chroma、Streamer Companion、THX Spatial Audio、Virtual Ring Light、Cortex、7.1 Surround Sound、Razer Settings 等独立应用路由，详见[子应用与路由](01-ipc-api-surface.md#18-razer-sub-applications-and-routes)。已知启动入口不等于已取得目标应用的完整渲染源码。头像菜单 `/rz-user-profile-menu/` 的入口与清单所列 JS/CSS 已取得；账户会话与反馈宿主仍未接入。

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

## 全部成功探测记录

名称优先使用原 `deviceName`，为空时展示原 `short_name`；内部代号保持原样。产品名称未作推测或扩展。“JS/CSS 已取得”不表示业务与硬件服务已实现。

| 产品 ID | 原名称／short_name | icon 类别 | 本地取得状态 |
| --- | --- | --- | --- |
| [70](https://apps.razer.com/synapse/products/70/ui/manifest.json) | Razer Mamba TE | MOUSE | 仅探测记录 |
| [80](https://apps.razer.com/synapse/products/80/ui/manifest.json) | Razer Naga Hex V2 | MOUSE | 仅探测记录 |
| [83](https://apps.razer.com/synapse/products/83/ui/manifest.json) | Razer Naga Chroma | MOUSE | 仅探测记录 |
| [89](https://apps.razer.com/synapse/products/89/ui/manifest.json) | Razer Lancehead | MOUSE | 仅探测记录 |
| [92](https://apps.razer.com/synapse/products/92/ui/manifest.json) | Razer Deathadder Elite | MOUSE | 仅探测记录 |
| [96](https://apps.razer.com/synapse/products/96/ui/manifest.json) | Razer Lancehead TE | MOUSE | 仅探测记录 |
| [98](https://apps.razer.com/synapse/products/98/ui/manifest.json) | Razer Atheris | MOUSE | 仅探测记录 |
| [99](https://apps.razer.com/synapse/products/99/ui/manifest.json) | Jugan_UI | MOUSE | 仅探测记录 |
| [100](https://apps.razer.com/synapse/products/100/ui/manifest.json) | Razer Basilisk | MOUSE | 仅探测记录 |
| [101](https://apps.razer.com/synapse/products/101/ui/manifest.json) | Razer Basilisk | MOUSE | 仅探测记录 |
| [103](https://apps.razer.com/synapse/products/103/ui/manifest.json) | RAZER NAGA TRINITY | MOUSE | 仅探测记录 |
| [104](https://apps.razer.com/synapse/products/104/ui/manifest.json) | Razer Mamba Hyperflux | MOUSEPLUSMAT | 仅探测记录 |
| [105](https://apps.razer.com/synapse/products/105/ui/manifest.json) | Razer Mamba Hyperflux | MOUSEPLUSMAT | 仅探测记录 |
| [106](https://apps.razer.com/synapse/products/106/ui/manifest.json) | D.VA Razer Abyssus Elite | MOUSE | 仅探测记录 |
| [107](https://apps.razer.com/synapse/products/107/ui/manifest.json) | Abyssusessential_UI | MOUSE | 仅探测记录 |
| [108](https://apps.razer.com/synapse/products/108/ui/manifest.json) | Razer Mamba Elite | MOUSE | 仅探测记录 |
| [110](https://apps.razer.com/synapse/products/110/ui/manifest.json) | Razer DeathAdder Essential | MOUSE | 仅探测记录 |
| [112](https://apps.razer.com/synapse/products/112/ui/manifest.json) | Razer Lancehead Wireless | MOUSE | 仅探测记录 |
| [113](https://apps.razer.com/synapse/products/113/ui/manifest.json) | Razer DeathAdder Essential | MOUSE | 仅探测记录 |
| [115](https://apps.razer.com/synapse/products/115/ui/manifest.json) | Razer Mamaba Wireless | MOUSE | 仅探测记录 |
| [116](https://apps.razer.com/synapse/products/116/ui/manifest.json) | Gabbielite_UI | MOUSE | 仅探测记录 |
| [117](https://apps.razer.com/synapse/products/117/ui/manifest.json) | Razer Turret Mouse Xbox One Edition | MOUSE | 仅探测记录 |
| [120](https://apps.razer.com/synapse/products/120/ui/manifest.json) | Razer Viper | MOUSE | 仅探测记录 |
| [122](https://apps.razer.com/synapse/products/122/ui/manifest.json) | Razer Viper Ultimate | MOUSE | 仅探测记录 |
| [124](https://apps.razer.com/synapse/products/124/ui/manifest.json) | Razer DeathAdder V2 Pro | MOUSE | 仅探测记录 |
| [126](https://apps.razer.com/synapse/products/126/ui/manifest.json) | Razer Mouse Dock | ACCESSORY | 仅探测记录 |
| [128](https://apps.razer.com/synapse/products/128/ui/manifest.json) | Razer Pro Click | MOUSE | 仅探测记录 |
| [131](https://apps.razer.com/synapse/products/131/ui/manifest.json) | Razer Basilisk X Hyperspeed | MOUSE | 仅探测记录 |
| [132](https://apps.razer.com/synapse/products/132/ui/manifest.json) | RAZER DEATHADDER V2 | MOUSE | 仅探测记录 |
| [133](https://apps.razer.com/synapse/products/133/ui/manifest.json) | Razer Basilisk V2 | MOUSE | 仅探测记录 |
| [134](https://apps.razer.com/synapse/products/134/ui/manifest.json) | Razer Basilisk Ultimate | MOUSE | 仅探测记录 |
| [138](https://apps.razer.com/synapse/products/138/ui/manifest.json) | Razer Viper Mini | MOUSE | 仅探测记录 |
| [140](https://apps.razer.com/synapse/products/140/ui/manifest.json) | Razer Deathadder V2 Lite | MOUSE | 仅探测记录 |
| [141](https://apps.razer.com/synapse/products/141/ui/manifest.json) | Razer Naga Left Handed Edition | MOUSE | 仅探测记录 |
| [143](https://apps.razer.com/synapse/products/143/ui/manifest.json) | RAZER NAGA PRO | MOUSE | 仅探测记录 |
| [145](https://apps.razer.com/synapse/products/145/ui/manifest.json) | Razer Viper 8Khz | MOUSE | 仅探测记录 |
| [147](https://apps.razer.com/synapse/products/147/ui/manifest.json) | RAZER NAGA CLASSIC EDITION | MOUSE | 仅探测记录 |
| [148](https://apps.razer.com/synapse/products/148/ui/manifest.json) | Razer Orochi V2 | MOUSE | 仅探测记录 |
| [150](https://apps.razer.com/synapse/products/150/ui/manifest.json) | Razer Naga X | MOUSE | 仅探测记录 |
| [152](https://apps.razer.com/synapse/products/152/ui/manifest.json) | Razer DeathAdder Essential | MOUSE | 仅探测记录 |
| [153](https://apps.razer.com/synapse/products/153/ui/manifest.json) | Razer Basilisk V3 | MOUSE | 仅探测记录 |
| [154](https://apps.razer.com/synapse/products/154/ui/manifest.json) | Alma_UI | MOUSE | 仅探测记录 |
| [156](https://apps.razer.com/synapse/products/156/ui/manifest.json) | Razer DeathAdder V2 X Hyperspeed | MOUSE | 仅探测记录 |
| [158](https://apps.razer.com/synapse/products/158/ui/manifest.json) | Razer Viper Mini Signature Edition | MOUSE | 仅探测记录 |
| [161](https://apps.razer.com/synapse/products/161/ui/manifest.json) | Razer Deathadder V2 Lite | MOUSE | 仅探测记录 |
| [162](https://apps.razer.com/synapse/products/162/ui/manifest.json) | cobra | productCategoryIcon | 仅探测记录 |
| [163](https://apps.razer.com/synapse/products/163/ui/manifest.json) | Razer Cobra | MOUSE | 仅探测记录 |
| [164](https://apps.razer.com/synapse/products/164/ui/manifest.json) | Razer Mouse Dock Pro | ACCESSORY | 仅探测记录 |
| [165](https://apps.razer.com/synapse/products/165/ui/manifest.json) | RAZER VIPER V2 PRO | MOUSE | 仅探测记录 |
| [167](https://apps.razer.com/synapse/products/167/ui/manifest.json) | RAZER NAGA V2 PRO | MOUSE | 仅探测记录 |
| [170](https://apps.razer.com/synapse/products/170/ui/manifest.json) | Razer Basilisk V3 Pro | MOUSE | 仅探测记录 |
| [175](https://apps.razer.com/synapse/products/175/ui/manifest.json) | Razer Cobra Pro | MOUSE | 仅探测记录 |
| [178](https://apps.razer.com/synapse/products/178/ui/manifest.json) | Razer DeathAdder V3 | MOUSE | 仅探测记录 |
| [179](https://apps.razer.com/synapse/products/179/ui/manifest.json) | HyperPolling Wireless Dongle | ACCESSORY | 仅探测记录 |
| [180](https://apps.razer.com/synapse/products/180/ui/manifest.json) | Razer Naga V2 Hyperspeed | MOUSE | 仅探测记录 |
| [182](https://apps.razer.com/synapse/products/182/ui/manifest.json) | Razer DeathAdder V3 Pro | MOUSE | JS/CSS 已取得 |
| [184](https://apps.razer.com/synapse/products/184/ui/manifest.json) | Razer Viper V3 HyperSpeed | MOUSE | 仅探测记录 |
| [185](https://apps.razer.com/synapse/products/185/ui/manifest.json) | Razer Basilisk V3 X Hyperspeed | MOUSE | 仅探测记录 |
| [190](https://apps.razer.com/synapse/products/190/ui/manifest.json) | Razer DeathAdder V4 Pro | MOUSE | 仅探测记录 |
| [192](https://apps.razer.com/synapse/products/192/ui/manifest.json) | Razer Viper V3 Pro | MOUSE | 仅探测记录 |
| [194](https://apps.razer.com/synapse/products/194/ui/manifest.json) | Razer DeathAdder V3 Pro Hyperpolling Technology | MOUSE | 仅探测记录 |
| [196](https://apps.razer.com/synapse/products/196/ui/manifest.json) | Razer DeathAdder V3 HyperSpeed | MOUSE | 仅探测记录 |
| [199](https://apps.razer.com/synapse/products/199/ui/manifest.json) | eden_ui | MOUSE | 仅探测记录 |
| [203](https://apps.razer.com/synapse/products/203/ui/manifest.json) | Razer Basilisk V3 35K | MOUSE | 仅探测记录 |
| [204](https://apps.razer.com/synapse/products/204/ui/manifest.json) | Razer Basilisk V3 Pro 35K | MOUSE | 仅探测记录 |
| [207](https://apps.razer.com/synapse/products/207/ui/manifest.json) | HyperFlux V2 Wireless Charging System | MOUSEMAT | 仅探测记录 |
| [208](https://apps.razer.com/synapse/products/208/ui/manifest.json) | evelynrefresh_ui | MOUSE | 仅探测记录 |
| [211](https://apps.razer.com/synapse/products/211/ui/manifest.json) | Razer Basilisk Mobile | MOUSE | 仅探测记录 |
| [214](https://apps.razer.com/synapse/products/214/ui/manifest.json) | Razer Basilisk V3 Pro 35K Phantom Green Edition | MOUSE | 仅探测记录 |
| [218](https://apps.razer.com/synapse/products/218/ui/manifest.json) | Razer Cobra HyperSpeed | MOUSE | 仅探测记录 |
| [221](https://apps.razer.com/synapse/products/221/ui/manifest.json) | Razer Boomslang 20th Anniversary Edition | MOUSE | 仅探测记录 |
| [222](https://apps.razer.com/synapse/products/222/ui/manifest.json) | Razer Viper V3 Pro SE | MOUSE | 仅探测记录 |
| [224](https://apps.razer.com/synapse/products/224/ui/manifest.json) | Razer Orochi V2 | MOUSE | 仅探测记录 |
| [226](https://apps.razer.com/synapse/products/226/ui/manifest.json) | Razer Basilisk V4 Pro | MOUSE | 仅探测记录 |
| [229](https://apps.razer.com/synapse/products/229/ui/manifest.json) | Razer Viper V4 Pro | MOUSE | 仅探测记录 |
| [231](https://apps.razer.com/synapse/products/231/ui/manifest.json) | RAZER NAGA V3 PRO | MOUSE | 仅探测记录 |
| [235](https://apps.razer.com/synapse/products/235/ui/manifest.json) | Razer Basilisk V4 HyperSpeed | MOUSE | 仅探测记录 |
| [239](https://apps.razer.com/synapse/products/239/ui/manifest.json) | Razer DeathAdder V4 Pro Carbon Fiber Edition | MOUSE | 仅探测记录 |
| [241](https://apps.razer.com/synapse/products/241/ui/manifest.json) | Razer Mouse Dock V2 Pro | ACCESSORY | 仅探测记录 |
| [515](https://apps.razer.com/synapse/products/515/ui/manifest.json) | Razer Blackwidow Chroma | KEYBOARD | 仅探测记录 |
| [521](https://apps.razer.com/synapse/products/521/ui/manifest.json) | Bwtechroma_UI | KEYBOARD | 仅探测记录 |
| [529](https://apps.razer.com/synapse/products/529/ui/manifest.json) | Bwchromaow_UI | KEYBOARD | 仅探测记录 |
| [534](https://apps.razer.com/synapse/products/534/ui/manifest.json) | Blackwidowxchroma_UI | KEYBOARD | 仅探测记录 |
| [542](https://apps.razer.com/synapse/products/542/ui/manifest.json) | Carolinechroma_UI | KEYBOARD | 仅探测记录 |
| [545](https://apps.razer.com/synapse/products/545/ui/manifest.json) | Blackwidowchromav2_UI | 空值 | 仅探测记录 |
| [550](https://apps.razer.com/synapse/products/550/ui/manifest.json) | Jamiet1_UI | KEYBOARD | 仅探测记录 |
| [551](https://apps.razer.com/synapse/products/551/ui/manifest.json) | Jamiet2_UI | KEYBOARD | 仅探测记录 |
| [552](https://apps.razer.com/synapse/products/552/ui/manifest.json) | Razer Blackwidow Elite | KEYBOARD | 仅探测记录 |
| [554](https://apps.razer.com/synapse/products/554/ui/manifest.json) | Cynosachroma_UI | KEYBOARD | 仅探测记录 |
| [555](https://apps.razer.com/synapse/products/555/ui/manifest.json) | Tartarusv2_UI | KEYPAD | 仅探测记录 |
| [556](https://apps.razer.com/synapse/products/556/ui/manifest.json) | CynosachromaPro_UI | KEYBOARD | 仅探测记录 |
| [563](https://apps.razer.com/synapse/products/563/ui/manifest.json) | Charlotte_UI | SYSTEM | 仅探测记录 |
| [564](https://apps.razer.com/synapse/products/564/ui/manifest.json) | Dana17-2_UI | SYSTEM | 仅探测记录 |
| [565](https://apps.razer.com/synapse/products/565/ui/manifest.json) | Victoriat3_UI | KEYBOARD | 仅探测记录 |
| [567](https://apps.razer.com/synapse/products/567/ui/manifest.json) | Razer Blackwidow Essential | KEYBOARD | 仅探测记录 |
| [569](https://apps.razer.com/synapse/products/569/ui/manifest.json) | Lynette_UI | SYSTEM | 仅探测记录 |
| [570](https://apps.razer.com/synapse/products/570/ui/manifest.json) | Charlotte2_UI | SYSTEM | 仅探测记录 |
| [571](https://apps.razer.com/synapse/products/571/ui/manifest.json) | Dana_UI | SYSTEM | 仅探测记录 |
| [574](https://apps.razer.com/synapse/products/574/ui/manifest.json) | Turret Keyboard Xbox One Edition | KEYBOARD | 仅探测记录 |
| [575](https://apps.razer.com/synapse/products/575/ui/manifest.json) | Cynosalite_UI | KEYBOARD | 仅探测记录 |
| [576](https://apps.razer.com/synapse/products/576/ui/manifest.json) | Charlottese_UI | SYSTEM | 仅探测记录 |
| [577](https://apps.razer.com/synapse/products/577/ui/manifest.json) | Janett2_UI | KEYBOARD | 仅探测记录 |
| [579](https://apps.razer.com/synapse/products/579/ui/manifest.json) | Huntsmante_UI | KEYBOARD | 仅探测记录 |
| [580](https://apps.razer.com/synapse/products/580/ui/manifest.json) | Tartarus Pro | KEYPAD | 仅探测记录 |
| [581](https://apps.razer.com/synapse/products/581/ui/manifest.json) | Charlotte3_UI | SYSTEM | 仅探测记录 |
| [582](https://apps.razer.com/synapse/products/582/ui/manifest.json) | Dana3_UI | SYSTEM | 仅探测记录 |
| [585](https://apps.razer.com/synapse/products/585/ui/manifest.json) | Ida_UI | KEYBOARD | 仅探测记录 |
| [586](https://apps.razer.com/synapse/products/586/ui/manifest.json) | Razer Blade Stealth 13 | SYSTEM | 仅探测记录 |
| [587](https://apps.razer.com/synapse/products/587/ui/manifest.json) | Razer Blade 15 Advanced Model | SYSTEM | 仅探测记录 |
| [588](https://apps.razer.com/synapse/products/588/ui/manifest.json) | Dana17-3_UI | SYSTEM | 仅探测记录 |
| [589](https://apps.razer.com/synapse/products/589/ui/manifest.json) | Razer Blade 15 Studio Edition | SYSTEM | 仅探测记录 |
| [590](https://apps.razer.com/synapse/products/590/ui/manifest.json) | Janett2v2_UI | KEYBOARD | 仅探测记录 |
| [591](https://apps.razer.com/synapse/products/591/ui/manifest.json) | Janett3v2_UI | KEYBOARD | 仅探测记录 |
| [592](https://apps.razer.com/synapse/products/592/ui/manifest.json) | Jamiet3_UI | KEYBOARD | 仅探测记录 |
| [593](https://apps.razer.com/synapse/products/593/ui/manifest.json) | Ornatapokemon_UI | KEYBOARD | 仅探测记录 |
| [594](https://apps.razer.com/synapse/products/594/ui/manifest.json) | Razer Blade Stealth 13 Base Model | SYSTEM | 仅探测记录 |
| [595](https://apps.razer.com/synapse/products/595/ui/manifest.json) | Charlotte5_UI | SYSTEM | 仅探测记录 |
| [597](https://apps.razer.com/synapse/products/597/ui/manifest.json) | Razer Blade 15 Base Model | SYSTEM | 仅探测记录 |
| [598](https://apps.razer.com/synapse/products/598/ui/manifest.json) | Dana17-5_UI | SYSTEM | 仅探测记录 |
| [599](https://apps.razer.com/synapse/products/599/ui/manifest.json) | Mayamini_UI | KEYBOARD | 仅探测记录 |
| [600](https://apps.razer.com/synapse/products/600/ui/manifest.json) | Razer BlackWidow V3 Mini HyperSpeed | KEYBOARD | 仅探测记录 |
| [601](https://apps.razer.com/synapse/products/601/ui/manifest.json) | Lynette5_UI | SYSTEM | 仅探测记录 |
| [602](https://apps.razer.com/synapse/products/602/ui/manifest.json) | Blackwidow V3 Pro | KEYBOARD | 仅探测记录 |
| [605](https://apps.razer.com/synapse/products/605/ui/manifest.json) | Razer Ornata V2 | KEYBOARD | 仅探测记录 |
| [606](https://apps.razer.com/synapse/products/606/ui/manifest.json) | RAZER CYNOSA V2 | KEYBOARD | 仅探测记录 |
| [614](https://apps.razer.com/synapse/products/614/ui/manifest.json) | Jamiet1analog_UI | KEYBOARD | 仅探测记录 |
| [616](https://apps.razer.com/synapse/products/616/ui/manifest.json) | Razer Blade 15 Base Model | SYSTEM | 仅探测记录 |
| [617](https://apps.razer.com/synapse/products/617/ui/manifest.json) | Mayaminijp_UI | KEYBOARD | 仅探测记录 |
| [618](https://apps.razer.com/synapse/products/618/ui/manifest.json) | Margaret_UI | SYSTEM | 仅探测记录 |
| [619](https://apps.razer.com/synapse/products/619/ui/manifest.json) | Huntsmantev2_UI | KEYBOARD | 仅探测记录 |
| [620](https://apps.razer.com/synapse/products/620/ui/manifest.json) | Razer Huntsman V2 | KEYBOARD | 仅探测记录 |
| [621](https://apps.razer.com/synapse/products/621/ui/manifest.json) | Charlotte6_UI | SYSTEM | 仅探测记录 |
| [622](https://apps.razer.com/synapse/products/622/ui/manifest.json) | Dana17-6_UI | SYSTEM | 仅探测记录 |
| [623](https://apps.razer.com/synapse/products/623/ui/manifest.json) | Razer Blade 15 Base Model | SYSTEM | 仅探测记录 |
| [624](https://apps.razer.com/synapse/products/624/ui/manifest.json) | Piperan_UI | SYSTEM | 仅探测记录 |
| [630](https://apps.razer.com/synapse/products/630/ui/manifest.json) | Charlotte7_UI | SYSTEM | 仅探测记录 |
| [631](https://apps.razer.com/synapse/products/631/ui/manifest.json) | Idav2_UI | KEYBOARD | 仅探测记录 |
| [633](https://apps.razer.com/synapse/products/633/ui/manifest.json) | Dana17-7_UI | SYSTEM | 仅探测记录 |
| [634](https://apps.razer.com/synapse/products/634/ui/manifest.json) | Dana7_UI | SYSTEM | 仅探测记录 |
| [642](https://apps.razer.com/synapse/products/642/ui/manifest.json) | Mayaminianalog_UI | KEYBOARD | 仅探测记录 |
| [647](https://apps.razer.com/synapse/products/647/ui/manifest.json) | Selenat2_UI | KEYBOARD | 仅探测记录 |
| [650](https://apps.razer.com/synapse/products/650/ui/manifest.json) | Charlotte8_UI | SYSTEM | 仅探测记录 |
| [651](https://apps.razer.com/synapse/products/651/ui/manifest.json) | Dana17-8_UI | SYSTEM | 仅探测记录 |
| [652](https://apps.razer.com/synapse/products/652/ui/manifest.json) | Piper8_UI | SYSTEM | 仅探测记录 |
| [653](https://apps.razer.com/synapse/products/653/ui/manifest.json) | Blackwidow V4 Pro | KEYBOARD | JS/CSS 已取得 |
| [654](https://apps.razer.com/synapse/products/654/ui/manifest.json) | Cynosapro_UI | KEYBOARD | 仅探测记录 |
| [655](https://apps.razer.com/synapse/products/655/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | 仅探测记录 |
| [658](https://apps.razer.com/synapse/products/658/ui/manifest.json) | Razer DeathStalker V2 Pro | KEYBOARD | 仅探测记录 |
| [659](https://apps.razer.com/synapse/products/659/ui/manifest.json) | Selenat3_UI | KEYBOARD | 仅探测记录 |
| [660](https://apps.razer.com/synapse/products/660/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | 仅探测记录 |
| [661](https://apps.razer.com/synapse/products/661/ui/manifest.json) | Razer DeathStalker V2 Pro | KEYBOARD | 仅探测记录 |
| [664](https://apps.razer.com/synapse/products/664/ui/manifest.json) | Razer DeathStalker V2 Pro Tenkeyless | KEYBOARD | 仅探测记录 |
| [669](https://apps.razer.com/synapse/products/669/ui/manifest.json) | Piper9_UI | SYSTEM | 仅探测记录 |
| [670](https://apps.razer.com/synapse/products/670/ui/manifest.json) | Charlotte9_UI | SYSTEM | 仅探测记录 |
| [671](https://apps.razer.com/synapse/products/671/ui/manifest.json) | Razer Blade 16 | SYSTEM | 仅探测记录 |
| [672](https://apps.razer.com/synapse/products/672/ui/manifest.json) | Razer Blade 18 | SYSTEM | 仅探测记录 |
| [673](https://apps.razer.com/synapse/products/673/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | 仅探测记录 |
| [674](https://apps.razer.com/synapse/products/674/ui/manifest.json) | Razer Ornata V3 | KEYBOARD | 仅探测记录 |
| [675](https://apps.razer.com/synapse/products/675/ui/manifest.json) | Razer Ornata V3 Tenkeyless | KEYBOARD | 仅探测记录 |
| [677](https://apps.razer.com/synapse/products/677/ui/manifest.json) | Selenat2_75_UI | KEYBOARD | 仅探测记录 |
| [678](https://apps.razer.com/synapse/products/678/ui/manifest.json) | Taliat1_UI | KEYBOARD | 仅探测记录 |
| [679](https://apps.razer.com/synapse/products/679/ui/manifest.json) | Taliat1tkl_UI | KEYBOARD | 仅探测记录 |
| [688](https://apps.razer.com/synapse/products/688/ui/manifest.json) | Taliat160_UI | KEYBOARD | 仅探测记录 |
| [689](https://apps.razer.com/synapse/products/689/ui/manifest.json) | RAZER HUNTSMAN V3 X TENKEYLESS | KEYBOARD | 仅探测记录 |
| [691](https://apps.razer.com/synapse/products/691/ui/manifest.json) | Blackwidow V4 Pro 75% | KEYBOARD | 仅探测记录 |
| [694](https://apps.razer.com/synapse/products/694/ui/manifest.json) | Razer Blade 14 | SYSTEM | 仅探测记录 |
| [695](https://apps.razer.com/synapse/products/695/ui/manifest.json) | Razer Blade 16 | SYSTEM | 仅探测记录 |
| [696](https://apps.razer.com/synapse/products/696/ui/manifest.json) | Razer Blade 18 | SYSTEM | 仅探测记录 |
| [697](https://apps.razer.com/synapse/products/697/ui/manifest.json) | Razer BlackWidow V3 Mini HyperSpeed | KEYBOARD | 仅探测记录 |
| [708](https://apps.razer.com/synapse/products/708/ui/manifest.json) | Razer Pro Type Ergo | KEYBOARD | 仅探测记录 |
| [709](https://apps.razer.com/synapse/products/709/ui/manifest.json) | Piper11_UI | SYSTEM | 仅探测记录 |
| [710](https://apps.razer.com/synapse/products/710/ui/manifest.json) | Sonya11_UI | SYSTEM | 仅探测记录 |
| [711](https://apps.razer.com/synapse/products/711/ui/manifest.json) | Kira11_UI | SYSTEM | 仅探测记录 |
| [716](https://apps.razer.com/synapse/products/716/ui/manifest.json) | Razer BlackWidow V4 Low-profile HyperSpeed | KEYBOARD | 仅探测记录 |
| [717](https://apps.razer.com/synapse/products/717/ui/manifest.json) | Razer Joro | KEYBOARD | 仅探测记录 |
| [719](https://apps.razer.com/synapse/products/719/ui/manifest.json) | Talia T1 Refresh UI | KEYBOARD | 仅探测记录 |
| [720](https://apps.razer.com/synapse/products/720/ui/manifest.json) | Talia T1 TKL Refresh UI | KEYBOARD | 仅探测记录 |
| [721](https://apps.razer.com/synapse/products/721/ui/manifest.json) | TaliaT1_60%_Refresh_UI | KEYBOARD | 仅探测记录 |
| [724](https://apps.razer.com/synapse/products/724/ui/manifest.json) | BlackWidow V4 Low-profile Tenkeyless HyperSpeed | KEYBOARD | 仅探测记录 |
| [727](https://apps.razer.com/synapse/products/727/ui/manifest.json) | Razer BlackWidow V4 Tenkeyless HyperSpeed | KEYBOARD | 仅探测记录 |
| [728](https://apps.razer.com/synapse/products/728/ui/manifest.json) | Razer Huntsman Signature Editon | KEYBOARD | 仅探测记录 |
| [736](https://apps.razer.com/synapse/products/736/ui/manifest.json) | Sonya12_UI | SYSTEM | 仅探测记录 |
| [737](https://apps.razer.com/synapse/products/737/ui/manifest.json) | Kira12_UI | SYSTEM | 仅探测记录 |
| [739](https://apps.razer.com/synapse/products/739/ui/manifest.json) | Razer Reclusa X Mini 65% | KEYBOARD | 仅探测记录 |
| [740](https://apps.razer.com/synapse/products/740/ui/manifest.json) | Razer Huntsman V3 HE Magnetic Mini 65% 8KHz | KEYBOARD | 仅探测记录 |
| [741](https://apps.razer.com/synapse/products/741/ui/manifest.json) | Maya T2 TKL SE UI | KEYBOARD | 仅探测记录 |
| [742](https://apps.razer.com/synapse/products/742/ui/manifest.json) | RAZER HUNTSMAN V3 PRO LOW-PROFILE TENKEYLESS 8KHZ | KEYBOARD | 仅探测记录 |
| [746](https://apps.razer.com/synapse/products/746/ui/manifest.json) | Razer Huntsman V3 HE Magnetic Tenkeyless 8KHz | KEYBOARD | 仅探测记录 |
| [747](https://apps.razer.com/synapse/products/747/ui/manifest.json) | Tartarus Pro | KEYPAD | 仅探测记录 |
| [752](https://apps.razer.com/synapse/products/752/ui/manifest.json) | Selenat2_75_UI | KEYBOARD | 仅探测记录 |
| [769](https://apps.razer.com/synapse/products/769/ui/manifest.json) | Razer Philips Hue | HUE | 仅探测记录 |
| [777](https://apps.razer.com/synapse/products/777/ui/manifest.json) | RAZER KRAKEN BT SANRIO LIMITED EDITION | AUDIO | JS/CSS 已取得 |
| [778](https://apps.razer.com/synapse/products/778/ui/manifest.json) | Asrock_UI | ACCESSORY_MAINBOARD | 仅探测记录 |
| [780](https://apps.razer.com/synapse/products/780/ui/manifest.json) | Jadet1_UI | 空值 | 仅探测记录 |
| [781](https://apps.razer.com/synapse/products/781/ui/manifest.json) | Gilliant1_UI | 空值 | 仅探测记录 |
| [782](https://apps.razer.com/synapse/products/782/ui/manifest.json) | Gilliant2_UI | 空值 | 仅探测记录 |
| [783](https://apps.razer.com/synapse/products/783/ui/manifest.json) | Paige_UI | 空值 | 仅探测记录 |
| [784](https://apps.razer.com/synapse/products/784/ui/manifest.json) | Joanna_UI | 空值 | 仅探测记录 |
| [790](https://apps.razer.com/synapse/products/790/ui/manifest.json) | Tiana_UI | 空值 | 仅探测记录 |
| [791](https://apps.razer.com/synapse/products/791/ui/manifest.json) | miriam-config | 空值 | 仅探测记录 |
