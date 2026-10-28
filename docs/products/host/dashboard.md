# 本机 Dashboard 与设备卡片

Dashboard 汇总已发现设备、设备中间件公布的运行摘要和已安装模块，再把操作路由到对应页面。卡片不是另一套设备设置来源：配置档名称、电量、就绪状态和配件来自各自运行摘要，进入卡片也不代表设备读取或写入已经完成。本文依据页面部署 `0.0.86`／build `2610200951` 的当前根和实际引用 bundle；页面部署身份独立于宿主版本。

## 挂载与导航

Synapse 主页面按原件顺序挂载 `DASHBOARD_HEADER`、`GAMER_ROOM_HEADER`、`DEVICES_AND_MODULES_HEADER`、`GLOBAL_SHORTCUT_HEADER`，分别延迟加载模块 502、19388、44442、94608。Devices & Modules 与 Gamer Room 是两个入口，不能因共享主机外壳而合并。Global Shortcuts 的未保存映射通过共享保存提示阻止离开；完成回调后才更新当前导航。

历史记录从 sessionStorage 的 `tabNavigations_dashboard` 与 `currentTabNavigation_dashboard` 恢复。新增导航在当前位置截断前进历史，返回／前进只改变当前索引。外部 `focusSubTab` 消息可导航到 Gamer Room；全局广播另支持 Devices & Modules 和 Global Shortcuts。`dashboardDisabled` 是覆盖整个窗口的半透明阻断层。

根组件发布 `dependent-app-status`，管理模块窗口、设备窗口聚焦和后台窗口激活。窗口激活使用零延迟定时器合并连续请求；卸载会取消该定时器、终止窗口协调器、移除全局消息与 pointerdown 监听并移除运行标记。原件卸载时对 `broadcastMessage` 再次调用 `on`，并非配对的 `off`；两个 `apiElectron.response` 订阅也未在该类中展示释放，不能把它们描述为已完整清理。

## 设备集合与身份

本机设备组组合 `devices`、`validDevices` 和 `installedDevices`：先用 PID 与 container ID 查有效摘要，具有有效序列号时按 PID＋序列号去重；没有有效摘要、但尚未出现在已安装集合中的项保留安装中卡片。连接、安装与设备就绪是不同状态。

当某 PID 已有另一 container 的实际序列号且处于非 off 状态时，缺序列号或为 `NOSERIALNUMBER` 的离线副本被过滤。Xbox／PlayStation 分支有独立条件；兼容设备、Windows 10 设备、Xbox 耳机、Xbox 手柄和开发中设备分别使用分组卡片。空集合才挂载 noDevice。

普通卡片通过 `connectedDevices` 的 window-storage 事件匹配 PID＋container 更新摘要，展示 `loading`、`ready`、`waiting`、`error`、`canceled`、无 alive、`standby` 和 `off` 等状态。当前配置档从 `activeProfile` GUID 在 profiles 中查名称，自动切换另附 Auto；IOT／复合设备使用 `isShownOnUI` 子设备，不能拿主接收器身份代替子设备。

## 卡片动作与电量

卡片把点击动作交给拖动所有者：mousedown 开始会话，mouseup 且位移未超过约 10 px 才执行页面聚焦。需重启、低于最低固件、preset 正在加载、off、待机、固件更新和特定系统检查各有阻断路径。卡片上的固件入口只导航到 Devices & Modules；它本身没有执行固件刷写。

电量显示区分充电、未充电、暂停、警告和离线。`level=-1` 是查询失败值，不能改写为 0%。首次上报以及从 off 恢复时，原版先显示 `-`，约两秒后才公布电量。该计时器在普通卡片的卸载方法中没有取消；window-storage 与自定义 windowstorageevent 则有对应解绑。

Windows Dynamic Lighting 图标同时要求全局功能开启、设备支持、ready 且非 BLE；点击打开对应设置窗口。无 alive 的恢复动作向 background-manager 发 `resuscitate`，不是把卡片立即改为已连接。复合设备数量徽标和 `singleProfile` 的显示各有独立条件。

## 双连接卡片

当前实际挂载链为模块 502 导出的 `Vi → Gi → Hi.renderGroup → Ci(Di) → getProdItems → Ii`。分组 mount 异步查询 `global.multipairing`，只读取返回对象的 `provisionFlags`，先保持 false；查询拒绝仅记录错误。返回后重建卡片并重新计算顺序，该调用方没有取消已发查询的 guard。普通卡片与双连接卡片同时参与设备分组，并非用双连接卡片替换全部普通卡片。

候选来自 `validDevices`，必须同时满足 `isMultiPairingDevice`、`setupStatus === "ready"` 和接收器资格。`isOnlyWirelessMode` 直接通过资格；其余须在原 `devices` 中找到同 PID、同 container 的 `isDongle` 项。当前表达式在 `devices` 缺失或空集合时也放行，不能把这条源码路径解释为已经实测找到接收器。

随后读取本地 `duallink-devices` 并取 `Object.values`；解析失败按空记录处理。每条记录投影 container、`master.productId` 和 slave `productId`，实际筛选 slave 时只比较 container 与 slave PID。若找到 slave，按这些 slave 的序列号剔除候选，留下配对主卡，并按 container 收集 slave 的 `chargingStatus`；未找到则保留全部候选。这里的配对归组来自本地关系记录，不能推定当前实际连接，也不能改成只按 PID 合并。

普通卡片的排序身份是 `${productId}-${deviceContainerId}`，双连接卡片是 `${deviceContainerId}-${productId}`，分别由同一模块的 `LP`／`UU` 生成。配对主卡显示 off 警告要求本卡 off 且该 container 下全部 slave 状态都严格为 off；源码对空数组使用 `every`，结果也为 true。未配对时只检查本卡。警告与真实连接读取不是同一个状态源。

### 卡片导航、候选主设备与图片

卡片只向拖动所有者登记 `overrideAction`；鼠标释放、仍是短距离拖动时才执行。执行时 PID 和 container 必须都存在，构造该 PID 的原 UI 地址，附带 `containerId`、`displayMode=multiDevicePairing`，以及非空时的 `allMasters`。窗口身份优先为 `multi-device-pairing-<container>`；通用命名函数另外支持 PID＋非空序列号，最后才是公共窗口名。窗口打开器先按完整窗口名查询本机客户列表；已有窗口则 activate，没有才 `window.open`，使用 `policy=3,shouldFocus=1`。这里没有直接发送扫描、配对或解绑设备命令。

`allMasters` 的生产顺序和字段校验独立于卡片分组：

1. 依次合并 `connectedDeviceInfo`、`duallink-devices` 中的 owner／master／masterDevice／deviceInfo 候选和当前设备；旧记录只有 master PID／dongle 时单独构造候选。
2. 按数值 PID、dongle ID、edition ID 去重，先出现者保留；不会把后项字段补入前项。
3. 中间集合允许有效设备项或与当前设备 PID 相同的项；最终标准化仍要求有限正数 PID、非空序列号和非空 `productName.en`，不满足则不发布。
4. 对合格项仅保留具名身份字段，以及实际存在的 dongle、category、有效 layout／edition 和 container。不会把本地候选升级为设备观测，也不会生成序列号或名称。

双连接卡片的接收器图有有序候选链，不能套普通设备图或跨产品借图。仅 `canPairTwoDevices`、`isOnlyWirelessMode` 或 ACCESSORY 项可以先使用自带 `src`。PID 或 dongle PID 为 226／227 时选专用资源，edition 128 使用独立图，其他值使用 edition 0；该分支不再构造通用型号 URL。其余型号先请求本 PID／edition 的接收器图，非零 edition 才追加同 PID 的 edition 0。最后依据 productivity／newHyperspeed 标志选择对应通用图，按原顺序去重；出错只推进候选索引，最终失败不会创建成功状态。PID、edition、src 或 container 改变会重置索引。

### 产品页 MultiDevicePairing 分支的实际挂载链

Dashboard 卡片本身只负责把 `deviceInfo`、`deviceName` 与 `allMasters` 交给产品页；真正的多设备配对状态机在产品 UI 的 `main` bundle（164、207）或 241 的动态 chunk 中。三个分支都把配对视图放进 iframe，而不是把设备写入逻辑放在 Dashboard 卡片中。

- **164 分支。** `accessories-164-current` 的 [`main.849ff923.js`](../../../.analysis/inputs/frontend/accessories-164-current/raw/apps.razer.com/synapse/products/164/ui/static/js/main.849ff923.js)（5,441,088 B，SHA-256 `b6c261acaad1278c48efc9e7385594357c2e34d3c487066a6e38f5b1c779d54a`）在 `[4588842,4595715)` 定义 `MultiDevicePairingFrame`。父组件以当前 origin 的 `/synapse/multipairing/` 为目标，固定写入 `displayMode=multiDevicePairing`，并按存在性写入 `containerId`、`productId`/`pid`、`category`、`canPairTwoDevices`、`isProductivity`、`deviceName`、原 URL 的 `serialNumber`/`lang` 和序列化后的 `allMasters`。iframe `load` 与 `multiDevicePairingReady` 都会发送 `multiDevicePairingInit`，消息必须同时满足 `event.source === iframe.contentWindow` 和 origin 相等；卸载时移除 message listener。父窗口还用 `getWindowServiceClients`、`registerWindowListChange` 和 `window-list-change` 观察同一产品窗口，代次变化会丢弃过期查询，外部窗口关闭后调用 `closeWindow`/`window.close`（约 `[4590688,4618419)`）。
- **207 分支。** `accessories-207-current` 的 [`main.ca032f7b.js`](../../../.analysis/inputs/frontend/accessories-207-current/raw/apps.razer.com/synapse/products/207/ui/static/js/main.ca032f7b.js)（5,524,676 B，SHA-256 `a2b14b88d54d1a23610d31b957525e2b87a40e22b8436d253465938dfb1b5c45`）在 `[4584600,4587384)` 复用同一 URL、iframe 握手和窗口观察契约，`allMasters` 通过该分支的 `WU`/`hU` 规范化后再序列化；产品页读取 URL 的 `allMasters` 失败时记录错误并使用空列表。其配对逻辑还包含外部解绑的 BroadcastChannel 追踪：先按 `productId`/`dongleId` 从 `duallink-devices` 过滤并写回本地关系，必要时在约 2 秒后补发主设备解绑，最长约 10 秒关闭频道并释放追踪状态（`[4617262,4624118)`）。
- **241 分支。** `accessories-241-current` 的 [`main.899712fe.js`](../../../.analysis/inputs/frontend/accessories-241-current/raw/apps.razer.com/synapse/products/241/ui/static/js/main.899712fe.js)（526,053 B，SHA-256 `65fe2c4069ece8270903895ce32c6c610d5eb8fec94bd944f5791c2376509a02`）本身不含 `MultiDevicePairingFrame` 字符串；实际配对 widget 在 [`914.4c13bdac.chunk.js`](../../../.analysis/inputs/frontend/accessories-241-current/raw/apps.razer.com/synapse/products/241/ui/static/js/914.4c13bdac.chunk.js)（55,044 B，SHA-256 `1320d5e2c7ee42497e590f2dbd979b9d10614573746003b3b87b6695dfc6a9d2`）的 `[39708,44500)`。该 chunk 使用同一 `DUALLINK_*` 协议和双状态（keyboard/mouse）流程，并通过 `M.A.onBCEvent` 接收 `MW_RESPONSE_UI`；主 bundle 只提供动态加载与普通设备页面，不能把 241 的 widget 语义归到 241 主 bundle 的静态代码。
- **179 分支。** `accessories-179-current` 的 [`main.55a48063.js`](../../../.analysis/inputs/frontend/accessories-179-current/raw/apps.razer.com/synapse/products/179/ui/static/js/main.55a48063.js)（5,511,141 B，SHA-256 `842643ff075f51e886404e1e6c7f4f968a19246469edca9ff3915ec8ddbaa9ca`）没有 `displayMode=multiDevicePairing`、`allMasters` 或 iframe bridge；HyperPolling/Uma 配对状态机直接位于 main bundle 的 `[3980805,3991597)` 与另一个同契约分支 `[4005584,4011093)`。它只有单一 `status`，挂载时发送 `DUALLINK_BIND_INFO`，绑定信息按本地 `K(...,{requireLocalEntry})` 过滤；单候选扫描自动 `DUALLINK_BIND_DEVICE`，绑定成功写 `pairedInfo` 后关闭对话框，解绑成功清空绑定并关闭。`MW_RESPONSE_UI` 错误分别回到 `LOADED`、`SCANNED`、`PAIR_FAILED`（4 秒后 `LOADED`）和 `UNPAIR_FAILED`（4 秒后 `PAIRED`）。另有 `SLAVE_CONNECT_EVENT` 的 20 秒等待指示与 storage 事件重新拉取绑定信息。打开设备页时先查询窗口服务客户，将已登记的 `mw` URL 替换为 `ui` 后以 `shouldFocus=1,policy=3` 打开，并在延迟后通过设备 BroadcastChannel 发送 `TAB_PERFORMANCE`；该分支没有 Dashboard 的 `allMasters` 传参，也不能用 164/207 的双键盘鼠标状态机替代。

三个分支的 widget 都在挂载时先把两个状态设为 `LOADING`（单设备只设一个），随后发送 `DUALLINK_BIND_INFO`。应答按 `category` 将绑定信息拆成 keyboard/mouse：有绑定项进入 `PAIRED`，没有绑定项为 `LOADED`；扫描发送 `DUALLINK_SCAN_DEVICE`，单设备收到一个候选时进入 `PAIRING` 并自动发送 `DUALLINK_BIND_DEVICE`，双设备则分别维护 keyboard 和 mouse 的状态。成功绑定按 category 替换旧项、更新 `pairedInfo`/本地绑定状态；成功解绑按返回的 `productId` 或 `dongleId` 过滤绑定项并进入 `UNPAIRED`。取消会发送 `DUALLINK_CANCEL`，再重新请求 `DUALLINK_BIND_INFO`。

失败路径不是简单清空错误：绑定信息失败回到 `LOADED` 并清空绑定／扫描列表；扫描失败回到 `SCANDED` 并保留另一类别的扫描结果（164 还会在双设备键盘扫描失败时触发后续状态判断）；绑定失败进入 `PAIR_FAILED`，约 4 秒后重置为 `LOADED`；解绑失败进入 `UNPAIR_FAILED`，约 4 秒后恢复 `PAIRED`。164 还在 `SLAVE_CONNECT_EVENT`（非扫描期间）重新请求绑定信息，并在配对期间使用本地条目过滤避免旧绑定覆盖；207/241 的状态 reducer 更直接，扫描结果按类别合并，且 207/241 对特定 dongle `713`（164 对应内部常量）保留自动绑定门禁。产品页的窗口、iframe、BroadcastChannel 和 reducer 卸载清理均属于该产品页生命周期，不能由 Dashboard 的卡片点击或本地 `pairedInfo` 快照替代。

上述结论只来自三份当前产品原件的字节区间、SHA 和静态字符串/控制流；未执行厂商 JavaScript、产品窗口、中间件、官方 DLL 或真实设备，故 Rust 对照、设备写入读回、取消恢复和视觉验收仍保持未验证。

### 配对页面 CSS 的视觉状态与布局证据

配对页面的视觉契约由产品自己的 CSS bundle 提供；不能用 Dashboard 的通用卡片样式替代。以下文件均从当前 acquisition 原件读取，表中 `raw bytes` 是文件实际字节数，`CSS body` 是去掉末尾 `sourceMappingURL` 注释后的 UTF-8 字节数。所有偏移均为 UTF-8 字节偏移；本段使用多线程脚本同时读取五个原件后，再逐块核对选择器和声明。

| 分支 | 原件 | raw bytes | CSS body | SHA-256 | 主要选择器区间 |
| --- | --- | ---: | ---: | --- | --- |
| 164 | [`main.f583d0c3.css`](../../../.analysis/inputs/frontend/accessories-164-current/raw/apps.razer.com/synapse/products/164/ui/static/css/main.f583d0c3.css) | 479,067 | 478,928 | `7a5fc7f29b50fca4f0e98b1110c916dd3635f6f7fc3744f647ae601acc4421ae` | HyperPolling/MultiPairing `[398664,420688)`；MultiDevicePairing `[448272,467899)`；DualLink 内容 `[451944,463898)` |
| 207 | [`main.f0c4ac39.css`](../../../.analysis/inputs/frontend/accessories-207-current/raw/apps.razer.com/synapse/products/207/ui/static/css/main.f0c4ac39.css) | 464,646 | 464,507 | `cc2a7291b818b8ab9dd650b12e3af1efaf8722c94b9077ac41ed4e28d334c5c3` | `MultiDevicePairing` `[421780,441407)`；`duallink-device-content` `[425452,437406)`；名称哈希不同但失败/确认块完整存在 |
| 179 | [`main.c778525e.css`](../../../.analysis/inputs/frontend/accessories-179-current/raw/apps.razer.com/synapse/products/179/ui/static/css/main.c778525e.css) | 460,447 | 460,310 | `1c6a9cc7c5216a838275d9acf663b3446eb54fb43cfb9391ef805e7deec54c77` | HyperPollingWireless 单设备 `[406439,442013)`；没有 164/207 的 `MultiDevicePairing` 页面类 |
| 241 | [`914.530f0373.chunk.css`](../../../.analysis/inputs/frontend/accessories-241-current/raw/apps.razer.com/synapse/products/241/ui/static/css/914.530f0373.chunk.css) | 45,622 | 45,540 | `9f47b7e63b5009f40cd9ad73d1aee28cef28ac844823680985f846dbb081e0be` | `HyperPollingWirelessUma_*` `[0,16751)`；`HyperPollingWirelessMouseDock_*` `[16751,18809)`；`Duallink_*` `[23878,45539)` |
| 241 | [`main.1525b0e6.css`](../../../.analysis/inputs/frontend/accessories-241-current/raw/apps.razer.com/synapse/products/241/ui/static/css/main.1525b0e6.css) | 372,679 | 372,542 | `f4e4b1bcbe811fcb4347c01f942b29d626f5c789691d78048f895970d9d19229` | 仅保留 Dashboard/通用 `.multipairing` 图标与共享 `duallink-device-content` 覆盖；实际 Uma widget 样式在动态 chunk |

CSS 规则确认了以下可见状态和尺寸：

1. `.img-text .multipairing` 使用 44×44 图标；`.productivity`、`.hyperpolling` 分支分别选择独立的多设备/HyperPolling 资源（164/207/179/241 主 CSS 均在约 `[9685,10090)`）。因此三类入口不能复用同一图标或仅按 PID 改文案。
2. 164 的 `MultiDevicePairing_*` 为双设备页面：加载态居中（`loading`），页面容器全高可滚动（`pageContainer`），禁用态为 `opacity:.45;pointer-events:none`，设备图区域高 120px；取消/解绑按钮的正常、hover 和 disabled 规则分别是灰色 `#707070`、hover `#9b9b9b`、disabled 透明度 `.5`。键盘和鼠标类别图标固定 44×44；主/从设备连接示意使用 marching-ants SVG，connected/connecting/free-state 三态由不同背景或 `visibility:hidden` 表示。
3. 164/207 的双设备盒子是两个 250×210 的深色面板，扫描列表可滚动（104px），`box-bindingdevice-tag`、`box-scanningdevice-tag`、`box-unbindingdevice-tag` 使用绿色 `#44d62c`，`box-bindfail-tag`、`box-selectnodevice-tag` 与 `error-box` 使用橙色 `#fd8611`；确认解绑框为约 210px 宽、橙色边框并带 1.2s 过渡。`.showborderline` 以 `0 0 0 2px #44d62c` 表示已选/已绑定视觉边界。
4. 179 的 HyperPolling 单设备 CSS 使用同一 120px 设备图和 90px 最小操作按钮，但页面内容最小宽高为 850×685；其 `MultiPairingHyperPollingWireless_*` 只描述产品内的单独入口，不能据此推断 179 具备 164/207 的双状态 reducer。
5. 241 动态 chunk 将实际 widget 拆为 `HyperPollingWirelessUma_*`、`HyperPollingWirelessMouseDock_*` 与 `Duallink_*`。Uma 的加载、已配对、禁用和设备按钮状态与 164 的语义一致；MouseDock 额外显示设备列表、轮询速率和两个设备受限/警告提示。`Duallink_*` 额外定义 modal header/body、双方已连接 warning、键盘/鼠标类别图标、确认解绑按钮和连接动画；241 主 CSS 没有这些 widget 状态，不能只加载主 bundle 还原页面。

这些规则是 CSS 静态证据，不是已完成的视觉验收：没有执行厂商 JavaScript、挂载产品窗口或截图对照，也没有把样式存在当作 Rust 页面已实现。加载、禁用、扫描、绑定、解绑、失败和取消的真实状态仍须按上面的 reducer/中间件链逐项进行模拟与官方 DLL 对照；当前均保持 `Unverified`。

### 打包代码与实际挂载的边界

同一 56 bundle 内还保留 `kt` 配对窗口类、`_t` iframe bridge 和配对状态机。`kt.renderView` 能组合 `Ot(_t)`，但模块 502 的默认导出和 Dashboard 卡片路径没有挂载 `kt`；其 `connect(...)(kt)` 表达式的结果未成为这里的导出或卡片子节点。因此不能把其中的 window-list watcher、iframe 握手或 `DUALLINK_*` 状态机直接计作 Dashboard 已执行的生命周期。实际导航目标是产品 UI 的 `displayMode=multiDevicePairing` 分支，须沿该入口及 multipairing 页面另行核对。

原件中的 iframe bridge 确实比较 `event.source === iframe.contentWindow` 和确切 origin，收到 `multiDevicePairingReady` 会发送 `multiDevicePairingInit`；同一发送函数也由 iframe 的 onLoad 与独立 effect 调用，不能建模为只在 ready 后发送。effect 清理移除同身份 message 回调。这仅说明保留函数本身的语义，不证明本 Dashboard 挂载了它。配对写入、事件完成与取消仍由真实目标页面和中间件拥有。

## 模块、分组和布局

本机模块从 `AVAILABLE_MODULES` 与安装记录筛选，部分本地模块有自己的额外入口条件。模块卡片按名称打开对应窗口；维护状态可阻止打开，安装中显示 spinner。当前实际模块卡类 `X` 定义了 `getDeviceStatus`，把 enabled／connected、disabled、unknown 映射到绿色、白色和橙色，但该类的 mount 仅调用 `checkUpdating`，连接包装器也没有调用此 helper。状态初值仍为空、disabled 初值为 false，不能把未消费的 helper 当作当前卡片已经接收状态的证据。updating 检查也只在 mount 调用，未展示对应的更新钩子。

普通卡片宽 290 px、高 220 px，横纵间隔均为 20 px；列数按实际容器宽度计算。每组以稳定 `boxId` 维护显示顺序、离线项和新增项，noDevice 不保留到正常排序，特定模块引导保持在末项。折叠状态按组标题保存，拖动重排同步更新组顺序；回位动画为 300 ms。

拖动会话临时注册 document 的 wheel、mousemove、mouseup；正常 reset 会移除三者。分组类没有 `componentWillUnmount`，不能声称中途卸载也执行 reset。Dashboard 容器的 ResizeObserver 以约 60 Hz 节流计算列数，卸载时取消观察，并正确移除自身 storage／memory-storage 监听。介绍横幅关闭后写入本地状态并发出 `show-profile-migration-toast`。

Chroma 的 Studio、Connect、Sensa 和 Visualizer 行为分别见 [Chroma Studio](chroma-studio.md)、[Chroma Connect](chroma-connect.md)、[Sensa](sensa.md)、[Visualizer](visualizer.md)。这些模块的设备任务和退出恢复不归普通 Dashboard 卡片拥有。

### Chroma Dashboard 的隐藏启动与恢复

Chroma Dashboard 使用自己的挂载根，不能继承上面的 Synapse Dashboard 生命周期。当前 `main.e5fe7086.js` 的完整启动回调 `[551412,551634)` 加载 chunks 7714／3874／332，再取模块 `11078.default` 并挂 `windowName:"chroma-app"`。导出包装 `pe`、语言连接层 `Ne` 和真实页面类 `ge` 都在 `332.780bf0e1.chunk.js` 内。`ge` 的初始 `isStartHidden` 仅在 URL 的 `autoStart`、`forceHidden` **同时等于字符串 `"1"`** 时为真，缺少任一个条件都不属于这条隐藏启动分支。

`componentDidMount` 先请求共享管理器打开 background-manager 和 notification-manager，再更新语言并调用隐藏启动处理；前两个异步打开请求没有在这里等待。共享 `forceOpenHiddenTab` 按窗口名取得 navigator lock：已有窗口时移除 create 监听；否则打开指定本机 URL，等待匹配的 create 事件或十秒超时，两条路径都移除监听。异常分支仅记录错误，没有独立的监听移除，不能把锁回调结束写成全部资源释放或后台启动成功。

隐藏启动按 `getTabInfo("audio-visualizer")` 查询窗口集合，再读本机 `synapseAudioVisualizerProfiles.visualizerState`。未找到窗口且保存的状态为 truthy 时才调用 `window.open("/synapse/audio-visualizer/",...,"policy=3,tab_visible=1,")`，随后投递本窗口的 `dependent-app-status:{windowName:window.name,status:"hidden"}`。`getTabInfo` 查询或解析异常会记录后返回 null，因此一次打开请求也不能证明先前确实没有窗口；本机 Visualizer 状态不是设备 GET，打开 URL 也不是灯光引擎确认。

恢复入口包括 `visibleChange` 且 `isVisible`、`changeActiveTab` 且目标为 `chroma-app`、`focus` 且 `isFocus`，以及内部 `USER_CLICK_DESKTOP_ICON` 事件。`handleOnUserRestoreWindow` 先 await `getWindowStorageItem("dependent-app-status")`，然后才进入 try：返回空数组，或所有 `value.windowName` 都不包含 `chroma-app`，会直接返回，连本窗口隐藏状态也不清除。否则按行的 URL 字段 `windowName` 查 `/chroma-app/dashboard`，仅当该行解析后的 value 为 `status:"installing"` 时跳过 `running` 发布；这时仍会在 try 后清除 `isStartHidden`。初次 await 被拒绝发生在 try 外，不能按内部 JSON 解析错误的处理推断其结果。

这里的 `dependent-app-status` 经 Chroma facade → preload `doElectronAction` → main `electronAction` → `WindowStorage.callFunction`。主机按发送者 URL 保存独立 Map；GET 返回含 truthy 值的 `{value,windowName:URL}` JSON 数组，SET 返回**写入前的旧值**并向符合条件的其他窗口发事件。页面对 hidden／running 的 SET 都不等待，因此两个标记只表示窗口协调意图，不能证明安装完成、持久化完成、原生 worker 存活或设备设置写回。

`ge` 没有 `componentWillUnmount`；`handleAppHiddenStart` 丢弃了 `response("tab-message",...)` 的 disposer，内部 desktop-icon 监听也没有在该类中注销。preload 的 pagehide 会另行移除 tab-message 等 IPC 监听，不能据此声称页面类自身已经配对释放。

安装包装的真实挂载链是 `pe → Provider(store) → Ne → ge → B(F) → z(x) → span → Ce`。`z` 读取 `store.user.item`；`x.render` 仅在 `user.id` 为 truthy 时调用 `renderView()`，否则返回 null。当前 store 的用户初值为 `{item:{},isLoggedIn:false}`；`userGet` 对应 action 将其替换为实际 payload，退出 action 才恢复此初值。`ge` 的这条树没有随 `isStartHidden` 改变的 key；`x.componentDidUpdate` 在 `shouldLogin:false→true` 时只调用账号适配器，没有显式卸载／重建 `Ce`。

因此，`Ce` 的空依赖 effect 必须按实际先后顺序解释。若隐藏期间 `user.id` 始终为假，`Ce` 尚未挂载；恢复清除 hidden、之后 `user.id` 首次为真时，新实例会请求安装包装模块 `34461`。若隐藏期间已有 truthy `user.id`，`Ce` 会以 hidden=true 首次挂载并在 effect 中清空组件后早退；此后保持同一用户门禁与组件身份，仅清除 hidden 不会重跑该 effect。用户门禁先返回 null 后重新出现、错误边界切树或整页重建才属于其他挂载路径，恢复事件本身不保证发生这些路径。这是当前完整源码的条件结果，没有执行实际窗口来断言每次隐藏启动都会出现同一现象。账号门禁只保留原版挂载来源，本项目继续按本地优先范围移除账号和登录行为。

`Ce` 请求的 `34461.default` 是安装协调包装 `Y`，并非真正 Dashboard。它初始 `showInstallation=true`，先检查 Chroma，再按结果检查 background-manager；此状态为 false 后才返回传入的 children，并另行投递 `dependent-app-status:running`。phase 的生产链是安装 UI `x` → 所选 `4461` 原件中的 `20682.A` hook → WindowStorage 初始读取／事件 → `installerStatus` reducer。事件中的值变化为 `restart-required`、`skipped`、`completed` 时均映为 completed；初始第一条记录只把 skipped／completed 规范成 completed，不能把两条路径合并。removed 事件被忽略；hook 卸载会注销自己的 `windowstorageevent` 监听，但已发出的初始 Promise 没有取消屏障。phase=completed、Service Worker 的 activated 检查和 running 发布都是各自消费者条件，不确认文件完整、设备初始化或原生释放。源码中的联网更新／安装编排保留为来源上下文，属于本产品排除范围。

真正 children 继续经 `13545.kg` 的异步组件加载 chunks 7122／6883，取 `23322.default → connect(ja(ka))`。异步组件的 effect 依赖实际 lazy 回调；清理只清空本地组件和错误状态，不取消 Promise 或拒绝旧完成结果。`ka.componentWillUnmount` 移除 `dependent-app-status` 与自己的 storage 监听；它丢弃的 exitApp disposer、匿名 DOM 监听，以及 `ja → Pa` 的 window-list-change 订阅和 600 ms 延后关闭更新没有在这些完整所有者中配对释放。页面已移除、状态已清空和全部回调已停止仍须分别验证。

本链完整证据写入现有 [host local-audit](../../../.analysis/results/host-current/local-audit.json) 的 `window_lifecycle.chroma_dashboard_launch`，保留 57 个选定完整 AST 所有者、跨 chunk 导出／词法绑定及真实 host GET／SET／IPC／pagehide 所有者。原件身份为：

| 原件 | 实际字节与 SHA-256 | 完整关键所有者区间 |
| --- | --- | --- |
| [main.e5fe7086.js](../../../.analysis/inputs/frontend/chroma-app-dashboard-current/raw/apps.razer.com/chroma-app/dashboard/static/js/main.e5fe7086.js) | 552,739；`2479098971305d7e9a8fe3b9a820d0da306a9082b14e9277b3784431f9888dec` | 启动 `[551412,551634)`；GET `[57009,57276)`；SET `[56475,56756)` |
| [332.780bf0e1.chunk.js](../../../.analysis/inputs/frontend/chroma-app-dashboard-current/raw/apps.razer.com/chroma-app/dashboard/static/js/332.780bf0e1.chunk.js) | 1,944,306；`33fe6d03703de15d089e86f8ca012e3dc5c7443eec979bf45d10b6f032b86b8c` | 页面类 `[328818,331676)`；用户门禁 `[326057,326958)`；安装门禁 `[328537,328817)`；用户初态 `[1347751,1347776)`；完整 reducer `[1369352,1371458)` |
| [3874.4172ec87.chunk.js](../../../.analysis/inputs/frontend/chroma-app-dashboard-current/raw/apps.razer.com/chroma-app/dashboard/static/js/3874.4172ec87.chunk.js) | 6,606；`848075eeb40b4a0aca9eb95e16d2d23c2024d1bc0b7a4a838af74bf8911e60ec` | store 创建 `[2519,3776)`；reducer 组合 `[3776,4410)` |
| [4461.7a5efde4.chunk.js](../../../.analysis/inputs/frontend/chroma-app-dashboard-current/raw/apps.razer.com/chroma-app/dashboard/static/js/4461.7a5efde4.chunk.js) | 26,673；`ce7f0b7af360a64242945bfcecad3e96a33b397aa1f61862fa5cd67f73c79d2b` | 安装协调包装 `[22482,24743)`；实际安装 UI `[15583,20989)`；状态 hook `[4176,6355)` |
| [6883.53d122e0.chunk.js](../../../.analysis/inputs/frontend/chroma-app-dashboard-current/raw/apps.razer.com/chroma-app/dashboard/static/js/6883.53d122e0.chunk.js) | 240,263；`60ff51dfc51d4e1c90edf4c295eaedce0bea60c44c271d8d4ac5f75598167cfb` | 实际页面类 `[152772,157740)`；window-list hook `[152168,152662)`；导出连接器 `[157746,157924)` |
| [WindowStorage](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/window_storage/index.js) | 2,985；`6a10c8daeaa5a5bfe87f136d666e37c5915a6d62b10bf2e07d6e7f1a86a1f4a7` | `callFunction` `[312,2834)` |

这条本机启动／恢复链的纯静态测试会拒绝错误 owner／导出／词法绑定、截断函数、篡改发布值、删除结果／清理 owner，以及改写门禁、key、effect 依赖、实际 children 和释放边界。所选安装 hook 与另一保留 chunk 中重复编号的模块分别绑定原件，不能用相同模块编号替换来源。没有执行厂商 JavaScript、应用窗口、DLL 或设备；整套 Chroma Dashboard 的卡片和产品语义仍不能由这条有界链路外推。

## 当前页面与 Rust 接线核对

页面的固定几何须与行为一起保留。当前实际加载的 55 样式中，`.dashboard` 常规上限为 1220 px、最小宽为 620 px，`.reflow` 上限 2460 px；1279 px 及以下上限变为 910 px、正文两侧 inset 为 30 px，600 px 及以下上限为 290 px。组件写入的 `width:unset` 又覆盖 1260 px 断点的 `width:min-content`，不能孤立翻译一条 CSS。分组上下 margin 为 10 px，内容与标题相距 10 px、卡片间距 20 px，标题为 14 px。普通卡片 padding 为 `10px 20px 9px`、圆角 5 px、底色 `#111`；无设备卡片保留虚线轮廓。普通／选中／悬停、loading 组的 `visibility:hidden`、折叠的位移／max-height／overflow、拖动层次是独立状态。依据为 [55.4e8559cb.chunk.css](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/css/55.4e8559cb.chunk.css) 的分组／断点 `[182467,186278)` 及实际 `.box-item`／`.box-no-device` selector，未将同包 Gamer Room 或未挂载配对窗口的样式混入普通卡片。

当前 Rust 的实际入口是 [dashboard_cards.rs](../../../crates/razer-shell/src/shell/navigation/main_pages/dashboard_cards.rs)，不能以注册表有某 PID、存在 `DeviceCard` 或导航可达替代页面完成。静态核查结果如下：

| 页面表面／动作 | 当前 Rust 所有者与实际接线 | 尚不能计为完成的边界 |
| --- | --- | --- |
| 布局、设备组与空状态 | [MainLayout](../../../crates/razer-dashboard/src/lib.rs)、`dashboard_device_group`、`dashboard_empty_devices` 保留卡片尺寸、断点和本地空状态；设备卡片来自 `self.devices` 中的 workspace，点击进入该身份的设备页 | 官方 `devices/validDevices/installedDevices` 的去重、占位安装项与 Xbox／PlayStation／Windows 分类组未由这一个遍历完整复现；真实发现与全部卡片分组须分别验证 |
| 普通设备卡片、图像与提示 | [DeviceCard](../../../crates/razer-dashboard/src/dashboard_device_card.rs) 消费 setup、alive、固件、WDL、sub-device、edition 与图像选择；[dashboard_device](../../../crates/razer-dashboard/src/dashboard_device.rs) 独立持有延迟电量观察并按 GUID 查配置名 | 原件的全部产品／系统分支与当前设备生产者仍须逐项对应；本地 preview 图片不能作为连接、电量或配置已读的证据 |
| 卡片排序、点击与取消 | [DashboardGrid](../../../crates/razer-dashboard/src/dashboard_grid.rs) 使用领域卡片身份、`DashboardState`、300 ms 位移动画、10 px 短拖动判定、wheel 与 release；原生 drag preview 的 release 取消只恢复本组顺序且不激活卡片。Base Button 提供独立键盘激活入口 | 分组标题当前只接折叠，未接官方整组拖动排序；跨页面卸载、失焦及没有进入原生 drag 的在途 press 清理仍需逐路径核验，不能由成功 mouseup 推断 |
| 折叠和本地保存 | [DashboardGroupContent](../../../crates/razer-dashboard/src/dashboard_group.rs) 分开处理高度、位移、箭头与 overflow；`DashboardChanged` 经 shell 的串行 workspace writer 保存，并以 captured snapshot 标记已存 | 这是本地布局持久化；不能据此覆盖设备状态、真实服务动作或所有组排序。静态动画实现不等于真实窗口像素／焦点验收 |
| 重试安装／设备恢复 | 卡片将动作交给 shell；当前 `retry` 回调只发出“安装／恢复服务未连接，尚未发送请求”的提示 | 尚未提交原件的 reinstall／resuscitate 服务请求、接收状态或完成释放；该项没有后端闭环 |
| 固件与 WDL 图标 | 固件点击导航到 Devices & Modules；WDL 点击导航到设置入口 | 页面导航分别核验，不代表固件传输、WDL 控制权切换或系统设置页实际执行成功 |
| 模块与附属入口 | Rust 依次构造 Macro、Linked Games、Add Wi-Fi Device、Profile Migration、Tour 五个本地入口；相应动作进入各 owning 页面或 popup | 当前列表按已编译本地功能构造，没有复现官方 installedModules／模块更新观察链；各弹窗和目标功能另行验收，不能因卡片可点击宣称完整实现 |
| 多配对组卡 | 官方候选、关系投影、`allMasters` 和产品 `displayMode=multiDevicePairing` 入口已在前文静态追明 | 当前 `dashboard_device_group` 没有消费这些关系或构造 `Ii` 对应组卡；单独存在配对路由不能补足 Dashboard 的组卡及全部目标分支 |
| 引导和介绍横幅 | [DashboardTutorial](../../../crates/razer-dashboard/src/dashboard_tutorial.rs) 保留教程已读状态、100 ms 定位等待、popover 与独立文案／媒体；介绍横幅的关闭和 Tour 进入 shell 导航／保存 | 须继续对照实际挂载位置、重开、焦点恢复、滚动与全部资源状态，不能以教程 flag 已保存覆盖这些行为 |

已存在的 `dashboard_grid_tests.rs`、`dashboard_group_tests.rs`、`dashboard_tutorial_tests.rs` 与卡片／电量测试是具体测试入口；本轮只静态读取其实现和用例，没有运行这些窗口测试，也没有新增设备或官方 DLL 功能对照。后续用例必须覆盖当前官方候选数据、源值与本地草稿不同、错误／断开／重连、图像失败、拖动取消及卸载，分别记录页面、本地存储、真实服务和设备结果。

### 重试安装、无存活恢复与服务端状态

`BoxItemDevice` 的“重试”不是设备写入或固件恢复按钮，而是把一个已经由中间件判定为失败／无存活的窗口重新交给后台窗口控制器。组件从 `devicesAndModules.noAliveSignPage` 接收恢复名单，并按 `productId` 与 `deviceContainerId` 同时相等匹配当前 `box`；构造函数的 `noAliveSign` 初值为 `false`，只有 `componentDidUpdate` 才把这次匹配结果写入状态。`setupStatus` 为 `INSTALL_CANCELED` 或 `ERROR`，或者该状态为真时，渲染 retry 区；按钮的 `onMouseDown` 先登记拖动容器的 `overrideAction`，真正的 `reInstall` 在父级 mouse-up 判定为点击后执行。`isOnline` 只影响 `disabled` CSS 和离线提示，组件没有等待后台应答的 promise，也没有根据重试结果自行改写 `setupStatus`、电量或连接状态。

`reInstall` 有两个互斥分支：

- `noAliveSign` 为真时创建 `BroadcastChannel("background-manager")`，发送 `{msgName: "resuscitate", msgData: this.props.box}`。消息包含完整 `box`，UI 没有先删名单、重置状态或确认目标窗口存在。
- 否则用 `usb_5426_${productId}_${deviceContainerId}_mw` 作为频道名，调用页面公共 `postBCMsg(CROSS_PAGE_REQUEST, {action: "retryInstall"}, channel)`。代码本身没有发送完成／失败回执，也没有关闭刚创建的 BroadcastChannel；`retryInstall` 的服务端消费者不在 Dashboard bundle 中。

安装器列表的 `Item` 组件是另一条动作路径，不能与设备卡片的 `noAliveSign` 合并。它从 `installerStatus[item.id]` 读取 `phase`、`downloadedPercent`、`installedFiles` 和 `totalFiles`：`downloading` 显示下载进度且按钮发送 `install-cancel` 到 `installation_${item.type}_${item.id}` 后立即关闭频道，`installing` 只显示禁用按钮，`error` 与 `canceled` 分别显示重试动作。设备项的重试创建 `usb_5426_${item.id}_${item.deviceContainerId}_mw`，发送 `{msgName:"crossPageRequest",msgData:{action:"retryInstall"}}` 后立即关闭频道；`macro`、`alexa` 和 `feedback` 则打开各自 installer URL 并记录 `userOpen`。`isOnline` 只加禁用 CSS，维护中状态直接显示维护提示；该组件仍没有成功／失败回执或真实设备读回。

后台 bundle 将 `background-manager` 映射到控制器实例。其 `xD` 身份键是 `${deviceContainerId}_${productId}`，`noAliveSignMW` 是内存 `Map`，持久化键为 `noAliveSignPages`。设备窗口打开后由 `checkAliveSign` 设定计时器（默认 40,000 ms）；超时处理会删除该键的计时器，把原始设备对象放入 `noAliveSignMW`，调用 `closeWindow("usb_5426_${productId}_${deviceContainerId}_mw")`，再把当前 Map 的值数组写回窗口存储。这里的“无存活”是窗口／中间件握手超时，不是 HID 读值为零、设备离线 getter 或电量状态；Dashboard 在 `setupStatus` 为空时只用同一名单显示失败文案。

后台收到 `resuscitate` 后，`QD.resuscitate` 先用同一身份键查 Map；只有命中才删除 Map 项并再次写 `noAliveSignPages`，随后调用 `_openMW`。`_openMW` 依次受卸载中的设备（放入 `pendingByUninstall`）、`canOpenMW` 尚未开启或正在关闭（放入 `pendingOpen`）、已经打开（直接返回）和实际打开窗口四类门禁约束；实际打开仍由 `TabHelper.openDeviceMWTab` 发起，并把 `onOpenFailByNetwork` 与 `checkAliveSign` 作为后续回调。`canOpenMW` 由设备事件监听注册，若初始化尚未完成则等 `check_latest_list_done`；Xbox／PlayStation 在 `processDeviceConnection` 中跳过自动打开，屏幕锁定也只排队连接事件。

后台 bundle 初始化完成后只创建一个 `new BroadcastChannel("background-manager")` 并把 `onmessage` 绑定到 `DC`。`DC` 按 `data.msgName` 查表，`EC.resuscitate` 再转发到 `QD.resuscitate`；这条接收链没有 Dashboard 级别的成功通知。窗口关闭时，`windowCloseListener` 清理该设备的 alive timeout、从 `noAliveSignMW` 删除对应项并处理待打开／待关闭集合；若仍有无存活项且网络检查 `be()` 为真，才再次调用 `resuscitate`。`closeMW` 的发送和重试由每秒轮询的关闭间隔器继续处理，停止、取消、服务 ACK、实际设备恢复和资源释放均不由 Dashboard 组件确认。

因此 Rust 端需要把“页面动作已发出”“后台窗口重开已接受”“中间件握手完成”“设备重新枚举并读回”建模为不同状态，并保留超时、取消、网络等待、卸载等待和窗口释放的证据。仅实现一个 retry 按钮、删除本地 `noAliveSign` 或返回固定“已恢复”都不能替代这条链；当前 Rust 卡片仍只提示服务未接线，故本段是静态逆向结果，不是服务、DLL 或真实设备运行验证。

证据：当前 Dashboard 原件 [56.d9a580a0.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/56.d9a580a0.chunk.js)（155,169 B，SHA-256 `a1c429b4bd88bcd28e46a3958d8fca610b7fa65cd6873c9a907fde916e5e16a8`；`BoxItemDevice` 相关字节区间约 `[6400,17450)`，`reInstall` 位于 `[7579,7930)`，渲染 retry 位于 `[14600,17450)`）。当前清单使用的安装器原件 [6505.98da235d.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/6505.98da235d.chunk.js) 为 43,600 B，SHA-256 `7a5ae9b5b69e433ba754ec2aec8c11814bae7f25805eb7342cf961eaf48c0a4e`，设备重试／取消区间约 `[13053,17400)`。后台原件 [index-b4c3eb99.js](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/assets/index-b4c3eb99.js)（521,794 B，SHA-256 `28475643f2acf1f5e070ad25f9622b1aa1349bae7a57eb6bd8d4ae2d96adba4e`；`QD` 控制器约 `[416000,427200)`，广播初始化约 `[467400,467950)`，`EC` 转发约 `[446983,447130)`）。上述范围只做字节／静态语义核对，未执行厂商 JavaScript、后台服务、DLL、主程序或设备。

## 当前原件

以下读取均核对 source set acquisition 中实际 bytes 和 SHA-256，并确认当前 asset-manifest／lazy loader 的引用；区间为半开 UTF-8 字节。当前 index、asset-manifest 和 manifest 的前后采样一致。本文只记录本机设备、模块和窗口语义。

| 来源 | 实际阅读范围 |
| --- | --- |
| [App.70b4a4e6.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/App.70b4a4e6.chunk.js) | 导航类 `[15370,19001)`；主机窗口根 `[21851,29095)` |
| [main 当前词条](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/main.d1d8c15b.js) | 导航词条声明，byte 137729 起；由 App 的实际四项导航读取对应导出 |
| [56.d9a580a0.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/56.d9a580a0.chunk.js) | 普通设备卡 `[3535,22650)`；模块卡 `[22971,25372)`；分组 `[100828,113893)`；Dashboard `[120446,129390)` |
| 同一 56 原件的双连接链 | 候选校验／标准化 `[48392,49365)`、`[52324,54875)`；图片候选 `[97762,98658)`、实际卡片 `[98659,100508)`；分组接收器资格／关系投影 `[106282,107711)`、实际挂载 `[107712,109268)`；排序身份模块 65153 `[152771,152937)` |
| 同一 56 原件中未由 Dashboard 挂载的配对子树 | iframe bridge `[54968,57579)`、保留的窗口类 `[57945,61885)`；不能以这些完整函数的存在替代实际调用方 |
| 同一 App 原件的窗口路由 | 模块 84058 `[89068,90459)`，窗口身份与已存在窗口聚焦；模块 88263 `[90466,91202)`，设备 UI／MW 频道身份 |
| [2280.e728a644.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/2280.e728a644.chunk.js) | feature 查询模块 42723 `[114374,114691)`，按 key 缓存原 `getAppFeature` 请求结果 |

上述原件区间已直接读取，并用项目管理的 JavaScript AST 解析器核对完整所有者和调用位置，未执行 React 或厂商代码。当前 acquisition 的 `unresolved` 是 chunk 枚举不可用所保留的动态依赖闭包缺项，map 获取失败由独立文件记录维护；二者是不同集合。恢复输入同时含保留的旧部署原件，完整 lexical factory 恢复量不能当作当前 manifest 可达模块数量。已取得原件字节一致、本页具体语义、Wakaru validate 的非零阶段与完整依赖闭包分别核对。

这是对应部署收据下官方前端的静态语义，页面部署身份与宿主版本独立。本文没有据此声明 Rust 后端、界面接线、官方 DLL 实际功能对照或真实设备验收完成，也没有执行厂商 JavaScript、主程序或安装动作。
