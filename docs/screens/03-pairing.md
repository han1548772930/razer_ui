# 正在配对（`TAB_PAIRING`，productId `182`）

> 本文只审计 `.ref/devices/182` 的配对实现。`HYPERPOLLING_*` 语言 key 的存在，不足以证明某个产品实际显示该页面或某个按钮。

## 1. 入口与真实条件

- 页面属于 `182` 鼠标模块的 `TAB_PAIRING` 路由。
- 真实组件使用设备类别、dongle/master/slave 数据、扫描状态、已连接序列号和 `canPairTwoDevices` 决定显示内容。
- 共享多设备配对代码包含键盘、鼠标和其他 dongle 分支；本文只记录其中 `MOUSE` / `182` 实际可命中的分支。
- 页面可见不等于配对动作可用：没有 master dongle、扫描结果或连接状态时，按钮/确认区域会进入禁用或空状态。

## 2. 实际组件树

独立配对逻辑在 `main.db20a7c4.js` 中以 `PairingContent_*` CSS module 类和 `Kg` 组件出现；其核心结构为：

```text
PairingContent
└─ .PairingContent_pairingContent
   ├─ .PairingContent_instruction
   ├─ .PairingContent_devicesContainer
   │  ├─ device card / dongle card × scan devices
   │  └─ empty state 或 skeleton state
   └─ 当前选中设备的确认/解除配对区域
```

同时，HyperPolling/Dual-link 分支使用：

```text
duallink-device-content
├─ duallink-device-dongle
│  ├─ duallink-master-dongle（只有 master 分支）
│  ├─ duallink-keyboard-dongle（键盘分支，不属于 182 结论）
│  └─ duallink-mouse-dongle（鼠标分支）
├─ dongle image + dongle-info
└─ contect-animated-dot / mouse-mat connection lines
```

JS 中对鼠标分支明确检查 `category === "MOUSE"`，并用 `productId`、`deviceContainerId`/`dongleId` 生成配对身份键。

## 3. 配对流程

### 3.1 扫描

- 扫描状态由 `scan`、`status`、`status2`、`scandevices` 传入；页面不自行伪造候选设备。
- 扫描中的设备卡可以显示 skeleton：`.PairingContent_skeletonBox`、`.skeletonBadge`、`.skeletonImage`、`.skeletonText`。
- 无候选时使用 `.PairingContent_emptyStateBox`，尺寸 `width:290px; height:220px; border:2px dashed #666; border-radius:5px; padding:36px`。
- 扫描结果通过 `onSelectDevice` 更新选中索引，再由 `onConfirmPair` 执行配对。

### 3.2 确认和解除配对

- `onConfirmPair` 接收鼠标/键盘不同 pairing device key；182 只记录鼠标命中路径。
- `onUnpair`、`onUnpairExternal`、`onReclaimExternal` 是不同操作，不能合并成一个“取消全部配对”按钮。
- 解除配对前使用确认弹层；CSS 中 `.ConfirmDialog_unpairDialog` 为深色弹窗，内联版本为 `.ConfirmDialog_unpairDialogInline`，不是普通页面卡片。
- 配对成功、失败和取消状态由设备服务参数更新；前端不能点击后直接追加一台“已配对设备”。

## 4. 样式证据

### 4.1 PairingContent

| 选择器 | 已确认样式 |
|---|---|
| `.PairingContent_pairingContent` | `width:100%` |
| `.PairingContent_instruction` | `color:#ccc; font-size:14px; margin-bottom:10px` |
| `.PairingContent_devicesContainer` | `display:flex; flex-wrap:wrap; gap:20px` |
| `.PairingContent_emptyStateBox` | `290px × 220px; border:2px dashed #666; border-radius:5px` |
| `.PairingContent_emptyStateText` | `#ccc; Roboto; 14px; line-height:17px; text-align:center` |
| `.PairingContent_skeletonBox` | 半透明黑底 `#0000004d`，纵向排列，`padding:10px` |
| `.PairingContent_skeletonImage` | `248px × 99px`，底色 `#ffffff08`，圆角 `3px` |

### 4.2 设备卡和连线

- `.duallink-device-content` 是配对内容区域，不是通用 `.widget` 的替代实现。
- `dongle-img-box` 与 `dongle-info` 同时显示接收器图像和产品名称；不能只显示 productId 文本。
- `contect-animated-dot`、`master-mousemat`、`master-mousemat-dot` 负责鼠标/接收器连接动画和状态线。
- CSS 中存在 `connected`、`connecting`、`free-state`、`right_to_left`、`left_to_right` 状态资源；状态不同必须使用对应类，不能恒定显示成功连线。

### 4.3 按钮和弹窗

- 取消配对按钮使用次色 `#707070`，鼠标悬停为 `#9b9b9b`；disabled 状态降低 opacity 并禁止 pointer events。
- 普通确认弹窗：背景 `#1a1a1a`、半透明边框、圆角 `8px`、阴影；内联确认框背景同样为深色，边框使用警示色。
- 主页面整体仍遵循该模块实际命中的 `.body-wrapper` / `.body-widgets` 外壳；不要在文档中重复复制所有产品页公共 CSS。

## 5. 状态矩阵

| 状态 | 页面行为 |
|---|---|
| 无 dongle/master | 显示不可用或空状态，不显示可执行的成功按钮 |
| 扫描中 | 显示 skeleton/connecting 状态，候选列表等待服务返回 |
| 有候选设备 | 显示设备卡，可选择后确认 |
| 已配对 | 显示已连接关系和解除配对入口 |
| 解除确认中 | 显示确认弹窗，取消不会改变服务状态 |
| 操作失败 | 保留原状态并显示失败反馈，不写入伪造配对结果 |

## 6. 不应推导的内容

- 不因 `HYPERPOLLING_WIRELESS_DONGLE_HEADER` 存在就断言所有 Razer 设备都有该页。
- 不把键盘分支、通用 MultiDevicePairing 分支写进 182 鼠标的实际布局。
- 不把当前仓库的本地 dongle 快照当成原页面的扫描/配对成功证据。

## 7. 证据文件

- `.ref/devices/182/static/js/main.db20a7c4.js`
- `.ref/devices/182/static/css/main.48c20423.css`
- `.ref/devices/182/manifest.json`
