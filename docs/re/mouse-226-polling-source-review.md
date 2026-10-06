# 226 Polling Rate、连接观察与 enableStages 原码独立复核

日期：2026-10-06。范围仅限 226 Basilisk V4 Pro 普通 Performance 的 Polling Rate 及其连接/限制来源，以及主线程要求追查的 DPI `enableStages` 来源。本报告不修改应用实现，不替代其他产品或整个 Performance 页验收。

结论：原报告 N/O 的 BLE 隐藏、无线/有线目标字段、限速与说明缺项均成立；本次补齐了此前缺失的 **当前 226 MW 源码**，进一步证明 `enableStages` 是 OTFS 运行状态控制的编辑锁，并找到了可供后续扩展的 `isOtfsActive` 只读 wrapper 声明。实际 native ABI 与生产者接线尚未完成，不能把已有 HID 枚举或通用服务查询当作这些状态已经读取成功。

未运行应用、构建、测试、安装器、厂商 JavaScript 或 DLL。只读取当前源码，以 HTTP 下载源码字节，使用维护环境中的 Acorn 静态解析。没有使用停用的旧参考目录或 `.ref/tools/`。

## 1. 当前 MW 补源与版本一致性

本地原先仅有 `.ref/discovery/products/226/mw/manifest.json`。从官方 `https://apps.razer.com/synapse/products/226/mw/` 补取到 [.ref/middleware/226/](../../.ref/middleware/226/)：

| 文件 | HTTP / 内容 SHA-256 |
| --- | --- |
| `index.html` | 200 / `c18eb69bccf2abe98e8327653fab372c4141d19d1381a17e0d196e9a0fdc3407` |
| `manifest.json` | 200 / `59ff1b03868280d15ddf4f7d7add57cffe448a5a820a4957025dd1391b1796b7` |
| `webpackManifest.json` | 200 / `ff38d0c2367b14bf49e6c810aaf486fa5cfe2be5f7b3d7c11520010bbb912e93` |
| `main.660230dedaa05e8b00fc.js`，下文 MW-M | 200 / `3159334a82a0a9576cf8fbf5983dc8594eb378307d34b01fdab0ed49f06c76a5` |
| `7846.b84800eaaf18ff1145e5.js`，下文 MW-C | 200 / `dcc50b174ef63fa3f12cab2a3c9fde22fc288ee6c20d213bc8e30c38b80eaa80` |

首次探查的 `asset-manifest.json` 返回 404；当前 HTML 明确声明的是 `webpackManifest.json`，随后按这个真实声明获取。404 已单独记录，未将错误页保存成有效源码。

`webpackManifest.json` 声明的 **89 个 JavaScript 文件全部 HTTP 200，逐文件 SHA 与旁置 `.http.json` 相符，89 个文件全部通过 Acorn script 静态解析**。每个文件均保留原 URL、最终 URL、时间、HTTP 状态、长度、SHA、ETag 和 Last-Modified。未下载 manifest 内驱动/安装器资源，未调用任何厂商代码。

[STATIC-ACQUISITION-RECEIPT.json](../../.ref/middleware/226/STATIC-ACQUISITION-RECEIPT.json) 记录全量文件清单和验证结果。当前 manifest 字节与 2026-10-02 的 discovery 副本完全一致；HTML 的 `noscript#version` 与 manifest 的 `version`、`buildVersion`、`gitMetadata`、`dependencies` 完全一致，HTML 的 main 脚本名也与 webpack manifest 一致。版本为 `0.0.14 / 2608110744`，commit `28c811ddfc27805627fda55d8a0426481236a34d`。HTTP Last-Modified 已变为 10-05，但 manifest 内容并未变更；不把时间变化推断为源码版本变化。

这证明本次取得的是当前官方 HTML/manifest 声明的一致资源集合；manifest 不提供逐 chunk 的独立签名，因此不声称 manifest 对每个 chunk 做了密码学认证。

## 2. 其他重新读取的当前源

以下偏移均为 0 起始、右端不包含的 **UTF-16** 偏移；压缩文件的单行行号不能替代它。

| 简称 / 文件 | SHA-256 |
| --- | --- |
| UI-M：`.ref/devices/226/static/js/main.08f95762.js` | `ccbd5af37e23d1c64faf62551d15b0ef91d6e89fc06cafab3fb9464c598f884e` |
| UI-P：`.ref/devices/226/static/js/8355.3d5e573e.chunk.js` | `b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8` |
| BG：`.ref/host-4.0.827/source-evidence/background-current-source.js` | `382ad87a9715c6417bac9d9f89a0a557521eaed4ed56a2512bc50c2d449b77c0` |
| USB：`.ref/host-4.0.827/electron/UsbRzDeviceAction.js` | `bdd1b91f94898e7d89e13584d132155526b7ca477275c9b0037876f42f617816` |
| WS：`.ref/host-4.0.827/electron/modules/window_storage/index.js` | `6a10c8daeaa5a5bfe87f136d666e37c5915a6d62b10bf2e07d6e7f1a86a1f4a7` |
| ME：`.ref/host-4.0.827/electron/modules/mapping_engine/win/index.js` | `32b0dab3a1a83a970b2501608d53c6544cab0c9a2b47054d7bd9a660fe4133c0` |
| `.ref/discovery/catalogs/AvailableDevices.json` | `428b43ac965095a26d3033e7e4043966eebe05138a3fcd7087e7fe00281be511` |

BG 与 `.ref/applications/background-manager/assets/index-8d39b3d5.js` 本次重新计算的 SHA 完全相同。当前目录中的每个符号均重新读取，没有把旧审计符号改路径后当作新证据。

## 3. 普通 Performance 挂载与 Polling 行为

UI-P 的导航 `[275451,275518)` 挂 `Ri({isBle:this.props.isBle})`。`Ri=wi`，`wi` 在 `[183668,183991)`；右列 `Xs/zs` 条件为 `!isBle || DeviceInfo.supportBluetoothPollingRate === true`。UI-M CONFIG 模块 1057 `[54515,68130)` 明确 226 的 `supportBluetoothPollingRate=false`。**BLE 时整个面板隐藏**；不能因为共享组件存在 BLE 数组或第三个 rate 字段就给本页添加 BLE 按钮。

`zs` 与 connect 包装 `Xs` 的完整定位窗口为 UI-P `[157662,165591)`。重新核实：

- `getPollingRateMode`：支持的 BLE 模式→`ble`；否则 `isDongle || isBle`→`wireless`；其余→`wired`。226 的 BLE 已被外层阻止挂载。
- `getPollingRateStateKey/getSetPollingRateAction` 同步选 `pollingRateBle`、`pollingRateWireless` 或 `pollingRate`。基础有线/无线数组均为 125、500、1000、2000、4000、8000；值相同不代表存储 owner 相同。
- 有线标题 `WIRED_POLLING_RATE_HEADER`，无线标题 `POLLING_RATE_HEADER`；tooltip 也分别为 `WIRED_POLLING_RATE_V2_TOOLTIP` 与 `POLLING_RATE_V2_TOOLTIP`。
- 未被限制且所选 rate 大于 1000 时，有线用 `POLLING_RATE_WARN_NOBATTERY`、无线用 `POLLING_RATE_WARN`，并给出 `https://www.razer.com/technology/razer-hyperpolling#best-practices-tips` 链接。
- 限制通过按钮 class 与点击守卫同时生效，保留完整按钮集；不把高频选项从列表删掉。
- 本页调用没有 `supportInGamePollingRate`。虽然当前 MW 的 DeviceInfo 声明此能力，仍不能用 MW 能力描述替代 UI 实际挂载证据。

## 4. isBle / isDongle 的真实来源

UI-M `[147253,147389)` 的 `deviceReducer` 消费 `MW_IS_BLE` 与 `MW_IS_DONGLE_DEVICE`。UI-M `g` `[544553,546835)` 建立设备 BroadcastChannel，订阅 `REDUX_STORE_UPDATE`，将单项或数组的 `{type,payload}` 派发进 Redux；初始化向 MW 请求 `runtimeData`。这是原 UI 的观察入口。

新增 MW-C 模块 50350 `[1142379,1158883)` 中，`ge` `[1147338,1153139)` 是实际 runtimeData 发布函数。其 `me/initBroadcastChannel` 在 1153139 后订阅 `UI_REQUEST_MW` 并将 runtimeData 请求路由到 `ge`；`G.YM.Lu.O_` 经模块 53981→13708 `[257368,259073)` 解为字面 `runtimeData`。发布模块 47399 `[2152814,2153200)` 的 `a/Q` 最终调用 `postBCMsg(REDUX_STORE_UPDATE,payload)`，与 UI 接收链对应：

1. 读取 `runtimeData`；compound 情况选 `subDevices.find(isShownOnUI)`。
2. 当真实 `rzDevice` 存在且 `rzDevice.productId === DeviceInfo.dongleId` 或 controller 专用判别为 true，发布 `MW_IS_DONGLE_DEVICE:true`，位置 1148895。该处没有相应 false 分支，原代码依赖页面/运行状态生命周期；不能将“没有收到 true”包装为一次成功的 wired 读取。
3. 在 `setupStatus===READY` 分支，把 `MW_IS_BLE: g.useBle` 放入发布批次，位置 1149436。

MW-C `connectRZDevice` 位于模块 87887 `[2107426,2151662)`，函数 `F` 为 `[2112614,2116353)`：连接流程先把 `runtimeData.useBle=false`；对 `rzDevice25` feature，实际枚举 PID 等于 `DeviceInfo.bleId` 时选 `rzDevice30` 类并置 `useBle=true`，否则选择普通/duallink 类；随后实际 `connectDevice` 完成才返回设备。MW-M DeviceInfo `[230233,231060)` 证明本产品 IDs 为 226/227/228、claimInterface=3。仅构造一个默认 `Device` 不等同这个流程完成。

BG `[408925,410220)` 的 `_checkUSBDetail` 也依据真实枚举记录，将 `realProductId=e.dongleId||actualPID` 与 AvailableDevices 的 dongleId/bleId 比对；`HD` `[415488,415572)` 处理标量或数组。该 catalog 当前 226 条目确为 `{productId:226,dongleId:227,bleId:228}`。这里补强了 ID 语义，但没有证明本地 `real_product_id` 的任何默认值是实时观察。

当前 [model.rs](../../src/model.rs) 有 `real_product_id/use_ble`，多处构造默认 `use_ble=false`；没有带有效性/来源的鼠标 `isDongle` 观察。[mouse_products.rs](../../src/features/mouse_products.rs) 的 workspace 没有消费上述 MW 连接观察。后续可接 typed connection observation，但应区分未知、已知 wired、已知 dongle、已知 BLE，并在设备身份/连接生命周期变化时失效旧观察。

## 5. enableStages 是 OTFS 运行编辑锁

这一点不能通过只搜索字段字面名得出结论：MW 不包含 `enableStages` 字面值，但使用同名动作字符串。

| 环节 | 当前证据 |
| --- | --- |
| UI action | UI-M `C.TY$ → Ht="SET_ENABLE_STAGES"`，声明 124483；reducer 在 186310 将 payload 写 `dpiStages.enableStages` |
| MW action | MW-M 模块 67061，`TY$ → At`，`At="SET_ENABLE_STAGES"` `[47377,47399)` |
| READY 初次发布 | MW-C 1149497：`SET_ENABLE_STAGES` 的 payload 为 `!isOTFSEnabled` |
| OTFS 切换后发布 | MW-C 1253995–1254210：更新 `isOTFSEnabled=t` 后，true 发 false；false 发 true |
| 实际事件接入 | MW-C 1289117、1289309：`ON_OTFS/ON_OTFS_OFF` 路由至同一 `Ct` task，传 `isKeyDown/isInterDeviceOTFS` |

原逻辑在关闭 OTFS 的分支还执行 OSD 清理；OTFS task 本体涉及临时 sensitivity mappings 与设备运行状态变更，**不能为读取编辑锁而运行这条 task**。

因此 `enableStages` 与 profile 的 `dpiStages.enable` 不同：前者是 MW 运行状态驱动的可编辑性，后者是用户配置。只读观察可接收 `SET_ENABLE_STAGES` 的明确 bool 并影响编辑入口；它不应写进本地 profile snapshot，也不能用 `dpiStages.enable` 推算，更不能称为已从 DPI 硬件配置读到。

## 6. DualLink / dock 限制来自共享观察数据

UI-P `getDualLinkDeviceList` 解析该页面 `localStorage['duallink-devices']`，取 `Object.values`；异常返回空列表。当前设备匹配以产品 CONFIG 的 productId/dongleId 与记录 slave（缺省记录自身）或 master 做字符串比较。

限制规则重新核实：

- DualLink：`isDongle && currentPairExists && !connectedToHyperPollingMaster`，采用 CONFIG 的 `dualLinkPollingRateLimitHz=1000`。
- 高频 master：当前 source 两个固定 productId 常量为 179、164（UI-M `[535875,535888)`），或 master 的 `supports8KHzPollingRate===true`。
- Multi-device dock：辅助 `js/Ls` UI-P `[156914,157407)` 要求 `supports8KHzPollingRate===true`，按 master productId/dongleId 交集统计至少两项，且当前 `isDongle`，则限制 1000。其内部交集是严格 includes，不能擅自改成前述字符串比较。
- multi-device dock 的优先级高于普通 DualLink；显示不同来源提示。
- 挂载读取 snapshot；窗口 focus 以及每 1500ms 比较 JSON 字符串，有变化才重算；卸载清除 listener/interval。连接 props 改变也重算。

MW-C 模块 66825 `[1159328,1199513)` 的 `ye/addDevice` 在 1176900 附近维护这个 key。保存记录时，`master` 从**当前 master 产品 CONFIG**取得 productId/dongleId/bleId/claimInterface/**supports8KHzPollingRate**，并写 browser localStorage。随后从 window storage 读取 DEVICE_RUNTIME_DATA 以补 slave serial。`supports8KHzPollingRate` 在此是源 CONFIG 能力元数据，不能说成刚执行硬件探测获得。

BG `[263052,263312)` 有同 key 的共享记录读法；后续 USB remove handler 删除同 master/container 的条目，并通过 storage 事件派发增删。这说明该数据带实际服务生命周期，既不是只要看到一个接收器就能编造的 pairing，也不是恢复一次本地 draft 后永久有效。

源 `applyPollingRateLimit` 还调用 `setPollingRateWireless` 自动降档。这是**写操作**。当前阶段只能表达已观察限制、禁用高频编辑和明确本地草稿约束，不能执行设备降档，更不能在未观察到新值时把设备当前频率显示成已降档。

## 7. Polling 读取链、存储 owner 与 native 边界

UI-M Polling reducer `[175019,176978)` 区分两类动作：

- `POLLING_RATE_SET_RESULT / POLLING_RATE_WIRELESS_SET_RESULT` 是 UI 命令侧，更新本地 reducer 并发出 `ON_POLLING_RATE`，进入状态机处理。
- `MW_SET_POLLING_RATE_TO_UI / MW_SET_POLLING_RATE_WIRELESS_TO_UI` 只接收值。UI-P 配置加载 `[271795,271973)` 同样通过这两个观察动作恢复不同字段；不是设备读取函数。

MW-C `nt` `[2086669,2087130)` 从当前 profile/local storage 取得无线字段，依据当前高频能力限制显示值，发布 `MW_SET_POLLING_RATE_WIRELESS_TO_UI` 后还 enqueue `ON_POLLING_RATE`。**虽然前半段名义上“load/update UI”，整个函数不是纯查询，不能为读取值复用执行。**

硬件 getter 为 MW-C `Oe` `[2129928,2130303)`：高频能力成立时 `getUSBHighSpeedPollingRate(profileId=1)`，否则根据 OBM capability 走 `getProfilePollingRate` 或 `getPollingRate`。MW-M 258588 处真实 feature 注入 `highSpeedPollingRate/yes {isHyperpollingDevice:true,isHyperpollingWireless:true}`；MW-C `Ye` `[2082677,2083239)` 同时判断实际连接和 DualLink 状态，不能只凭 feature 名强制所有分支走高频协议。

进一步追到 MW-C 模块 12844：`getUSBHighSpeedPollingRate` 对应 `Y` `[631415,631554)`，构造含 profileId 的 payload，调用 `rzDevice.sendCommand`，再由 `$` `[631088,631414)` 解码 `data[1]` 成 rate。模块 14397 的 `LF→ue` `[641753,641781)` 为 `Uint8Array([2,0,192])`，`VE→ve` `[641810,641848)` 为 `Get USB High Speed Polling Period`；基础 `getPollingRate` 对应 `w` `[627421,627515)` 与 `k` `[627129,627420)`。这是协议读请求，**不是一个已经证明可直接 FFI 调用的同名 DLL 导出**。

`rzDevice25.sendCommand` 从 MW-C 778728 开始，Electron 分支在 779856 处发 `hid.sendFeatureReportMutex` 或 `hid.sendFeatureReport`，包含 PID、VID、container、claimInterface、reportId、reportLength、protocol 和 dataSend。当前 USB host 调用的是 `node-rz-hid`；USB 枚举 `[3997,4727)` 则调用 `rz-usb-detect.find()`。必须区分查询意图、发送 query report 和纯元数据枚举：现有 Rust `HidDevices` 只读 HID collection 元数据，没有实现这条产品协议读取，也没有产品 container/连接成功事实。此处没有获得可供 Rust 直接照搬的 polling DLL C ABI，因此不新增猜测性 FFI。

## 8. 可列入后续的源证只读接口：isOtfsActive

当前 ME wrapper 确实声明 `isOtfsActive:["void",["pointer"]]`（属性起点 9016）；本次按原始文件直接用 JavaScript 字符串索引得到完整方法 UTF-16 `[87310,88158)`：

```text
isOtfsActive(callback)
callback: void(bool result, string reason, bool isEnabled)
JS result: { result, reason, isEnabled }
```

方法要求 `isInit`；未初始化返回 `{result:false,reason:...}`。FFI 异常也保留失败，不应将 false 状态字段当作一次成功的 inactive 查询。MW-C 403130 处有对应 JS `isOtfsActive` wrapper，通过 `callMappingEngineAction` 调用；本次在 89 个当前 MW 文件中未发现这个 wrapper 的实际调用点，故**没有证明该查询直接提供产品 `runtimeData.isOTFSEnabled` 的同一 owner/作用域**。

现有 `runtime_native.rs::Callback3/Query` 是 `bool,string,string`；该新查询需要第三参数为 bool 的独立声明，不能复用。此报告只证明当前 wrapper 声明、回调形状与调用方式，没有验证实际被 `EnginePaths` 发现的 DLL hash/架构/导出/calling convention，也没有执行初始化或查询。事件订阅 `registerDeviceModeChangedEvent` 是另外的运行状态变更边界，不能当作纯查询夹带接入。

WS 的 `getWindowStorageItem` `[1265,1591)` 读取 host 内存 map，返回跨窗口 `{value,windowName}` 数组序列化字符串；这不是 browser localStorage 的通用 getter。ME 当前 wrapper 有 `localStorageSetItem`，没有 `localStorageGetItem`，更不能由 setter 名反推一个 getter ABI。获取 `duallink-devices` 应保留明确生产者与 scope，不能误用 host window storage 或 mapping storage 假装读取到了同一 key。

## 9. 当前本地差异与落地顺序

本次回读时，[mouse_products.rs::polling](../../src/features/mouse_products.rs) 仍固定 `rates["POLLING_RATE"]` 和 `spec.polling_path()`；226 始终写 `/pollingRate`，无 BLE 外层门控、连接标题/tooltip、警示链接、DualLink/dock 观察或点击限速保护。以上不是参数表缺失，而是实际控件和 owner 接线缺失。主线程同时进行的 DPI 阶段实现不在本报告改动范围内。

可安全推进的下一步：

1. 用当前 UI 原码完成连接对应 panel、标题、tooltip、字段选择、限制说明与本地草稿；未知连接保留未知语义，不把默认 bool 标为成功查询。
2. 为 MW 的 connection、独立 wired/wireless rate、DualLink snapshot、`SET_ENABLE_STAGES` 建立 typed observation；观察值与用户本地覆盖、profile snapshot 分开。
3. 真实 publisher 对接前补齐容器/设备身份与观察有效性；当前 `HidDevices/AudioDevices/GlobalMode/GlobalShortcuts` 都不能直接充当这些 observation 的来源。
4. `isOtfsActive` 可进入逐项 native binary 静态校验清单；先证明 actual binary 与 wrapper，再讨论对应 owner，不能只增加请求枚举就算完成。
5. Polling native query 继续沿本报告协议/host 链静态核查，设备/服务写回、OTFS task、自动降档和 pairing 存储写操作留在后续整合阶段。

本报告和 89 个静态解析通过只证明上述已读取原码，不证明运行视觉效果、设备读值、当前已连接拓扑或完整产品已完成。
