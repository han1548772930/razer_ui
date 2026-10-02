# 源码审计依据与更正记录

> 文档审计后已接入新的 Rust 设备工作区，当前实施进度见[重构状态](03-implementation-gap.md)。本文中“本轮仅审计”指前一轮文档任务。

## 1. 范围与证据等级

审计日期：2026-09-30。以本地原版和当前 Rust 工作区为对象。本轮仅修订文档，没有加载 DLL、运行原版宿主或写入设备。

| 标记 | 证据 | 可以证明的范围 |
|---|---|---|
| JS | 根入口、render、事件、reducer | 页面归属、分支、调用和状态变化 |
| CONFIG | 产品配置 | 静态能力、默认值和候选项 |
| CSS | selector 与覆盖顺序 | 基线布局、尺寸、状态样式 |
| RESOURCE | context import、资源模块、manifest、实际文件 | 文件映射及下载情况 |
| RUST | 当前工作区源码 | 当前实现，含用户已有的未提交修改 |
| 建议 | 基于以上证据提出的方案 | 后续实现要求，不是已经完成的行为 |

locale 只能解释文案。共享组件、导出常量和资源存在，均不能单独证明产品使用该功能。必须沿着根路由跟到实际 render 和 props。

## 2. 原包版本

宿主：[package.json](../../.ref/synapse-asar/package.json)，应用名 `razerappengine`，版本 **4.0.563**。

| 产品 | 主 JS | 主 CSS | 模块版本 / 构建号 | commit |
|---|---|---|---|---|
| 182 | [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) | [main.48c20423.css](../../.ref/devices/182/static/css/main.48c20423.css) | 1.0.0 / 2607020654 | `87521bb44f19f46da616dd9caca045672cf6f3be` |
| 653 | [main.7b71cce5.js](../../.ref/devices/653/static/js/main.7b71cce5.js) | [main.5425442a.css](../../.ref/devices/653/static/css/main.5425442a.css) | 1.0.1 / 2607280325 | `e80b682b145095965d03075fdbc5ea279422716b` |
| 777 | [main.eb70ce38.js](../../.ref/devices/777/static/js/main.eb70ce38.js) | [main.e4bab2aa.css](../../.ref/devices/777/static/css/main.e4bab2aa.css) | 1.0.1 / 2607021103 | `53bab6de991026377087c363c0e27fdba42869ee` |

主前端：[main.7897a4cf.js](../../.ref/frontend/static/js/main.7897a4cf.js)、[App.eb32d7cd.chunk.js](../../.ref/frontend/static/js/App.eb32d7cd.chunk.js)。独立产品不共享同一份短符号命名；不能跨 bundle 搜同名类后认定行为相同。

## 3. 可复核的定位步骤

1. 从应用入口定位产品根 `navs`，解析 name 符号对应 locale key。
2. 跟随 wrapper、connected component、render，记录父组件实参和条件。
3. 向上定位 selectedProfile / reducer / DeviceInfo，向下定位事件、请求与回调。
4. 在同一产品 CSS 中核对最终覆盖，不能只取 selector 第一次出现。
5. 解析 Webpack request → module ID → chunk/main → media；另查内嵌 SVG 和动态 URL。
6. 检查 Rust 的入口是否可达、handler 是否连接、setter 是否接后端。

本轮使用语法解析辅助阅读压缩 JS，没有执行 bundle。引用指向原始文件，组件名、字段和模块 ID 用作搜索锚点。

JavaScript 解析器偏移以 UTF-16 code unit 计数，不能直接作为 Python 字符串索引。Windows 文件系统又不区分大小写，`mM.js` 与 `MM.js` 不能作为两份独立类证据。复核应以原 bundle 的词法作用域和正确偏移为准。

## 4. 产品路由证据

182 根类 `GM` 的声明如下，省略无关 props：

```js
navs: [
  {id: 1, name: aE.M9m, component: jsx(em, {...})},
  {id: 2, name: aE.hIF, component: jsx(lM, {})},
  {id: 3, name: aE.Q2y, component: jsx(MM, {})},
  {id: 4, name: aE.tPc, component: jsx(mD, {})},
  {id: 5, name: aE._$r, component: jsx(KN, {...})}
]
```

分别为 Customize、Performance、Power、Calibration、HELP。`displayMode === "multiDevicePairing"` 另进 `GG → UG`，不在普通 navs 内。

653 根类 `zh` 挂载 `nP`（Customize）、`Fh`（Lighting）、`Jd`（HELP）。

777 根类 `Ov` 挂载 `WM`（Sound）、`KM`（Mic）、`YG`（Lighting）、`KG`（Power）、`Tv`（HELP）。

header 将 HELP 过滤到帮助入口。三个产品都没有独立 Scrolling；对应旧页面文档已删除，滚轮/拨轮说明归入 Customize。

## 5. 页面和数据锚点

| 页面 | 实际入口/组件 | 数据 |
|---|---|---|
| 182 Customize | `em → $P → JP → QP`；图 `KP` | 产品模块 1057；button / mapping reducer |
| 182 Performance | `lM → OM`；`AM → IM`；`dm → Rm` | DPI、dpiStages、profile、dongle/固件 |
| 182 Calibration | `mD → PD → pD → LD` | smartOnlyCalibrationReducer.smartTracking |
| 182 Power | `MM`；`CM`、`pM` | 闲置时间、阈值、有效回报率 |
| 653 Customize | `nP → tP → eP → $m`；`zm → km` | 产品模块 78193；SVG_PRODUCT / DEVICECONFIG |
| 653 Lighting | `Fh → yh`；`_P`、`TP`、`Hh` | brightness、lighting、安装状态 |
| 777 Sound | `WM → VM`；`dM`、`GM` | 产品模块 7816；playback、audioEq |
| 777 Mic | `KM → kM`；`GM` | micEq；当前入口没有额外音量卡 |
| 777 Lighting | `YG → WG`；`qM`、`VG`、`gG` | 亮度、Streamer、灯效 |
| 777 Power | `KG → kG → zG → wG` | power saving，父组件 min=5 |

## 6. 主应用路由

| HomePage 导出 | locale key | 模块 / 主要 chunk |
|---|---|---|
| Rav | DASHBOARD_HEADER | 73435 / 4130 |
| BSg | GAMER_ROOM_HEADER | 19388 / 9388 |
| iwS | DEVICES_AND_MODULES_HEADER | 44442 / 6505 |
| fUK | GLOBAL_SHORTCUT_HEADER | 94608 / 7282 |

Settings 使用 `/synapse/settings/`、窗口名 `settings-synapse`。2026-10-02 已取得独立 720 chunk 的 JS/CSS，`ho/uo` 分别给出 Synapse / General 的完整组件树；不再依据共有 `.main-setting` CSS 推测其选项，详见[设置规格](../screens/12-settings.md)。

## 7. 旧推断更正

| 原推断 | 核对结果 |
|---|---|
| TAB 常量顺序就是路由 | 必须读取根 navs |
| 182 校准有表面列表、扫描、完成流程 | 实际入口为 Smart Tracking |
| Lift-off 在 Performance | 在 Calibration；性能右列为 polling 和 Windows 属性 |
| 653 没有游戏模式 | `km` 实际渲染 `JL → QL` |
| 777 Sound / Mic 都是 940 宽 EQ 卡 | 只有 Mic 调用传 `widgetId:"eqBox"` |
| 777 Lighting 有闲置关灯卡 | 实际左列是亮度和 Stream Reactive Lighting |
| PNG import 必须对应同名 PNG | 构建后大部分对应 AVIF |
| FFI 文件存在即表示 UI 接通硬件 | 需要完整 setter → 请求 → 回调证据 |
| 资源都已拉取 | 主前端有 273 个非 map manifest 路径缺失 |

`.ref/tools` 中旧文档生成器不能作为原版行为的证据，尤其不能再按共享常量、共有 CSS 或能力词汇生成全部页面。

## 8. 文件指纹和边界

| 文件 | SHA-256 |
|---|---|
| frontend main.7897a4cf.js | `31ddd5fea766268af557f35cbc45aee127f6571c6f0e42907780686324592aa3` |
| 182 main.db20a7c4.js | `7626cc9c0a20cf491a481c701829429ac8f23c9c8c5b907736505f70d3b9bcc7` |
| 182 main.48c20423.css | `dec1b71e91cc5e261b9c84bfcdfe501d5c6aa7f16491e103561a5fd69e916a1e` |
| 653 main.7b71cce5.js | `5854d3cb89f5e01eeb258f3218301988e3509e27f38ed23ddd77040e39dcb6ab` |
| 653 main.5425442a.css | `87289ab38f2c6ee8008c5e65c9e3ecc826ecf442665c46d0495548f97703e10a` |
| 777 main.eb70ce38.js | `7c3cc1d1ff8a95bd42462c8bb0f3b9573276d9bc35e103e805d9d02ce18053b4` |
| 777 main.e4bab2aa.css | `28d5a7ea7b3c38cff5a467b325203841f21cdd14a213e274ffcddc7cf9843427` |

未验证：设备通信成功、固件最终量化、真实模块安装状态、缺失资源画面和其它产品。独立 Settings 已获得源码并实现本地界面，原设置服务尚未接通。原包内部矛盾保留记录，不能按常识静默修正后宣称一致。

## 9. 前次文档校验（2026-10-01）

已检查 19 份 Markdown 的 UTF-8、代码围栏、732 个本地链接，以及资源表中的 request / 模块 / 文件映射；未发现断链或编码损坏。重新计算的 manifest 缺失数量与资源索引一致，以上 7 个原始文件 SHA-256 均已复核。`git diff --check` 通过。

与该次只读文档审计开始时的哈希基线相比，`src`、`assets`、`locales` 文件未改变；用户此前的源码/图片改动保留。该次删除无实际产品根路由支持的 `screens/06-scrolling.md`，有效滚轮/拨轮内容归入 Customize，并清理了索引引用。后续实现修改及当前验证状态见[重构状态](03-implementation-gap.md#5-验证状态)。
