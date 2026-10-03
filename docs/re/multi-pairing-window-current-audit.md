# 产品配对窗口（displayMode=multiDevicePairing）审计

按用户要求，这一族独立根用**真正的第二个 gpui 窗口**实现。本报告给出原版该根的来源收据：产品包里的根只是一个 iframe 宿主，真正的内容是 Dashboard 的多设备配对页（本仓库已实现的 4130 页面）。
## 产品包里的根（.ref/devices/180/static/js/main.ca4353ee.js，SHA-256 `47351314e8697bb9…`）

- 路由：`/synapse/multipairing/`；iframe id：`iframeMultiDevicePairingWidget`。
- 参数（按原代码写入顺序）：

| 参数 | 取值表达式 | 写入条件 |
| --- | --- | --- |
| `displayMode` | `"multiDevicePairing"` | 总是写入 |
| `containerId` | `e.searchParams.get("containerId")` | 按条件写入 |
| `productId` | `String(r)` | 按条件写入 |
| `pid` | `String(r)` | 按条件写入 |
| `category` | `String(T)` | 按条件写入 |
| `canPairTwoDevices` | `String(i.canPairTwoDevices)` | 按条件写入 |
| `isProductivity` | `String(i.isProductivity)` | 按条件写入 |
| `deviceName` | `String(_)` | 按条件写入 |
| `serialNumber` | `e.searchParams.get("serialNumber")` | 按条件写入 |
| `lang` | `e.searchParams.get("lang")` | 按条件写入 |
| `allMasters` | `JSON.stringify(A)` | 按条件写入 |

- 加载后 postMessage `multiDevicePairingInit`，payload 字段：`deviceInfo`、`deviceName`、`allMasters`；并等待 `multiDevicePairingReady` 后再发。
- favicon 取 `static/media/hyperpolling_icon.b7c3d035.svg`，窗口标题用文案 _ud 加后缀 `...`。
- 窗口名与策略由宿主决定：名字规则见 [窗口契约](display-window-contract.md)，本窗口用 `policy=3`（复用同名窗口）与 `shouldFocus=1`。

## Dashboard 的入口（.ref/applications/synapse/dashboard/static/js/7861.1b0e99a4.chunk.js，SHA-256 `9fc4c16bfd646066…`）

设备盒 `box box-multi-paring` 被按下时，先判 `if(!e||!t)return;`，然后写参数并交给宿主：

| 参数 | 取值表达式 |
| --- | --- |
| `containerId` | `String(t)` |
| `displayMode` | `"multiDevicePairing"` |
| `allMasters` | `JSON.stringify(s)` |

窗口标志：`sameWindow`、`autoFocus`。

## 本仓库的实现

- [display_window.rs](../../src/shell/display_window.rs)：窗口名、策略标志、具名登记与「存在即聚焦」；名字规则按原版 `xc()` 写成并有单元测试。
- [pairing_window.rs](../../src/shell/pairing_window.rs)：该窗口的根视图，承载 4130 页面；`allMasters` 走页面原有的 `set_external_devices`，返回时关闭自己的窗口。
- 入口：配对页设备卡（整卡点击），guard 与原版一致（`productId` 与 `deviceContainerId` 同时存在），缺一不动作。

## 已知缺口

- deviceInfo / deviceName 在本地页面没有对应字段，未映射
- allMasters 的原版来源是宿主写入的 connectedDeviceInfo 投影（Dashboard jt/Et），尚未审计，调用方没有真实记录时传 None

逐字段收据见 [机器可读证据](multi-pairing-window-current-evidence.json)。

重新生成：`node tools/audit-multi-pairing-window.cjs`；校验：`node tools/audit-multi-pairing-window.cjs --check`。
