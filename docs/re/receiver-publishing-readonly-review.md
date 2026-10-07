# 179 接收器发现链的独立静态复核（2026-10-07）

来源为当前 179 middleware、当前 Dashboard 和静态提取的 host 4.0.827。与主任务的协议/framing 审计分开，本记录核对在线条件、目录映射以及发布器的副作用；未运行厂商 JS、应用或 DLL。

[28 项 AST 收据及两份目录](receiver-publishing-review-current-evidence.json)由 `node tools/audit-receiver-publishing-review.cjs --check` 复核。该工具依赖维护中的 `middleware-source.cjs` 和 `webpack-source.cjs`，只用 Acorn 读取语法树。

## 目录与 182 身份

34340 模块的 `G=location.origin+"/synapse"`、`j()` 组成的实际 URL 是：

- `https://apps.razer.com/synapse/dashboard/AvailableDevices.json`
- `https://apps.razer.com/synapse/dashboard/DualDongleCompatibleDevices.json`

已通过 `tools/fetch-duallink-catalogs.py` 将缺失的两个原文件及 `.http.json` 收据保存到 `.ref/applications/synapse/dashboard/`。已有 Dashboard 文件没有被覆盖。新抓的 AvailableDevices 与 `.ref/discovery/catalogs/AvailableDevices.json` 字节一致。

| 文件 | SHA-256 |
| --- | --- |
| AvailableDevices.json | `428b43ac965095a26d3033e7e4043966eebe05138a3fcd7087e7fe00281be511` |
| DualDongleCompatibleDevices.json | `ad095ba5df1df779c35ec2dc8aeba658b2d359f4c25efa8da4621934a1ba663a` |

AvailableDevices 的原条目是 `{"productId":182,"dongleId":183,"isNotChromaDevice":true}`。DualDongle 的 183 条目包含 `DeviceSiblings:[182]`、`DeviceType:"MOUSE"`，ProductInfo 的 EID 0/128 分别为 DeathAdder V3 Pro 与 White Edition。179 条目的 `DevicePairingBuddies` 明确包含 183。目录说明支持的身份关系，不能证明当前接收器真的连接了该鼠标。

## 在线与观察条件

34340 的查询状态枚举是 `disconnected=0`、`connected=1`。`he()` 调用 `getMultipleDeviceWirelessConnectionStatusV2()`，从 `jsonData.status` 过滤 `productId===65535` 和自身 dongleId，再经 AvailableDevices 的 `dongleId` 查找真实产品身份。返回 183 才会映射为 `productId:182,dongleId:183`；只枚举到物理接收器 179 不足以产生这项观察。

`Ne()` 明确以 `e.status===x.connected` 判定在线。首次条目状态 0 或空序列号时，它最多进行 5 次递增等待后重查；重查结束后仍会处理离线条目，但传给 `ye()` 的第二参为 false，不将新条目写入本地存储。在线但尚无序列号时则会先发布身份、继续等待 runtime 数据，不能在原生端借此伪造序列号或完整设备就绪状态。

`ye(...,false)` 不删除已有缓存。因此 `duallink-devices` 中存在某项不能代替新鲜查询的在线状态；历史关联与当前连接需要分开保存和显示。

不要将此枚举与硬件事件中的电源状态混用：91818 的 `getPowerOnStatusFromHardwareEvent()` 判断的是 `eventValue.state===3`。这是另一个字段和契约。

## deviceInfo、master 与 containerId

179 的 21503 模块 DeviceInfo 原文包含 `productId:179,dongleId:179,vendorId:5426,claimInterface:0`。34340 的 `U` 来自 middleware 当前 URL 的 `containerId` 参数，而不是目录或新生成的 ID。

在线查询结果 183 的源发布过程为：

1. `he()` 经目录映射得到产品 182，保留 dongleId 183；名字、版本等优先来自真实 `connectedDeviceInfo`，缺失时仅从官方 DualDongle 目录补产品描述。
2. `Ne()` 调用 `ye({vendorId:5426,productId:182,deviceContainerId:U,dongleId:183},true)`。
3. `ye()` 为新条目加入 `master:{productId:179,dongleId:179,bleId:0,claimInterface:0}`，使用 `5426/183/U` 作为 `duallink-devices` 的键。
4. host 4.0.827 的 `zS` 监听该存储的新增键，`BS` 通过 `{...e,productId:e.dongleId}` 生成 PID 183 的合成 USB 事件；`_onConnect` 接续原有发现流程。
5. 当前 `_checkUSBDetail` 用 AvailableDevices 将 183 还原为 `productId:182,realProductId:183,isDongle:true`，保留同一个 `deviceContainerId:U`。这条路径需要真实查询先证明连接身份。

当前 Dashboard 22534 模块的 `listDeviceContainerIds()` 读取关联条目的 `deviceContainerId/master.productId/productId`。`getWirelessDevices()` 还要求设备 `isMultiPairingDevice`、`setupStatus==="ready"` 且 `isMultiPairingOnDongle()` 通过，再以 containerId 与从设备 productId 联合关联。另一个 `Ct()` 元数据适配器要求合法 productId、非空序列号与英文 productName；接收器查询单独不能满足完整首页状态。

## 可复用的只读边界

可落地的原生读取链是：以真实枚举到的 179/containerId 为查询目标，读取 V2 连接状态，保留原始 status/productId，再用官方目录映射返回的 dongle PID、挂接真实 master/containerId。查询未完成、出错、离线和缺失身份各自保持明确状态。没有查询结果时不能用兼容目录构造已连接 182。

原 `ye()` 及初始化整链不能直接当作只读包装调用：它会写 `duallink-devices`、改变 `ve/Ue/H` 内存数据、注册 `DEVICE_RUNTIME_DATA` 监听；后续 `Ye(Ze)` 会进入主从映射合并与写入流程。`ge()` 删除存储项，`Ce()` 改动设备对象的主从 PID，host 的存储监听会触发设备接入以及 HID 列表刷新。`Ne()` 还管理重试定时器并调用这些发布器。它们可作为静态行为证据，但当前原生只读实现应自行建模观察结果，不能复用整段初始化/发布流程。

本记录没有修改 runtime 发现实现，也没有实际打开设备或发送查询；协议传输的实现与验证状态由主任务单独记录。
