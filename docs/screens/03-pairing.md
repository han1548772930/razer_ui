# 配对：独立窗口与共享配对状态机

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 路由和源码

`[JS]` [182 main](../../.ref/devices/182/static/js/main.db20a7c4.js) 中，根 displayMode 为 `multiDevicePairing` 时进入 `GG → UG`。此容器的导航只有 TAB_PAIRING，历史含 deviceRoot / TAB_PAIRING。它不是 182 普通导航第五/第六项。

`PG`（memo 包装为 mG）创建 `/synapse/multipairing/` iframe。真正扫描/绑定/解绑视图在 [frontend 4130 chunk](../../.ref/frontend/static/js/4130.155387bf.chunk.js)，不是仅靠 182 bundle 就能说明完整配对。

## 2. 容器通信

iframe query 包含 displayMode、containerId、productId、pid、category、canPairTwoDevices、isProductivity、deviceName、serialNumber、lang、allMasters。`hG` 解析并整理 allMasters 数组；不能直接信任任意 JSON 形状。

父子通过 `multiDevicePairingInit` / `multiDevicePairingReady` 握手，并校验 origin 和 event.source。容器监听 registerWindowListChange；对应设备窗口消失时关闭配对视图，并通过 generation / cleanup 防止旧订阅继续更新。

GPUI Kit 可用原生页面/窗口重建这个领域流程，不必保留 iframe，但需要保留设备身份、初始化就绪、生命周期和请求响应边界。

## 3. 状态与设备列表

共享状态含 status、status2、bindInfo、scanedInfo、selectedIndex、dongleId、lang。原始常量对应：

| 值 / 符号 | 已确认用途 |
|---|---|
| 0 / Ge | 初始化加载 |
| 1 / Ve | 就绪；尚无绑定或回到可操作状态 |
| 2 / Ke | 扫描中 |
| 3 / Ze | 扫描完成/返回结果，允许空列表 |
| 4 / ze | 绑定中 |
| 5 / Ye | 已绑定状态 |
| 6 / qe | 绑定错误 |
| 7 / Je | 卡片的解绑/收回处理中状态；由列表投影也会生成 |
| 8 / Xe | 发起解绑后的主流程状态 |
| 9 / Qe | 已解绑/卡片未绑定 |
| 10 / $e | 解绑错误 |
| 11 / et | 收到绑定响应并合入 bindInfo 后的状态；卡片按已配对展示 |

状态 7 与 8 不能合并成一个“成功”值。状态 11 来自 DUALLINK_BIND_DEVICE 响应，不应由按钮点击直接设定；它也不等于已经另行验证设备所有功能可用。

canPairTwoDevices 时 status / status2 分别对应键盘和鼠标通道。绑定数据、扫描数据和 allMasters 候选共同生成卡片；有同产品排重、外部已配对设备、收回/重新配对及无结果分支，不能简单用一条 Vec<String> 代替。

## 4. 请求与回调

| 操作 | 真实消息 / payload | 成功处理 |
|---|---|---|
| 初次读取 | DUALLINK_BIND_INFO，{} | 合并有效数组，双设备按 category 设置通道，再触发扫描 |
| 扫描 | DUALLINK_SCAN_DEVICE，{status:1, category} | 合并扫描结果，双设备按 KEYBOARD/MOUSE 通道更新 |
| 绑定 | DUALLINK_BIND_DEVICE，{mode:1, device} | 必须存在 payload.device；按 category 替换 bindInfo，设置 et |
| 解绑 | DUALLINK_UNBIND_DEVICE，{productId, category} | 对应设备 connected=0，通道进入 Qe |
| 取消 | DUALLINK_CANCEL，{} | 再发 DUALLINK_BIND_INFO 刷新 |

单设备时 category 为 null。双设备按 status 通道选 KEYBOARD/MOUSE。dongleId=713 有特殊绑定续接路径，不应把所有设备都统一为同一种一次性请求。

错误处理：

- 绑定信息错误：回就绪，清空绑定/扫描数组。
- 扫描错误：进入扫描结果态，按通道保留/过滤其他类别结果，可能显示空结果。
- 绑定错误：进入 qe，约 4 秒后回就绪并清理列表。
- 解绑错误：进入 $e，约 4 秒后回到 Ye。
- 监听在卸载时注销；重复打开不能叠加响应 handler。

这些定时器是错误展示/恢复，不是模拟硬件成功。

## 5. 画面、键盘与资源

视图以主设备、从设备、扫描候选和连接状态卡片组织。已配对卡 hover 提供 Unpair；错误/已解绑提供 Pair；扫描/绑定/解绑时显示对应忙碌状态。确认弹窗处理 Tab、Escape、Enter/Space，不能只实现鼠标点击。

[4130 CSS](../../.ref/frontend/static/css/4130.6bdf8dd0.chunk.css) 包含 PairingContent / MultiDevicePairing 的模块样式。产品图来自动态产品资源与卡片数据，不是统一通用鼠标图。

原引用 `frontend/static/media/dongle-pairing.62f44d13.svg` 在本地缺失，其他目录也未找到；不能写成资源已到位。内嵌状态 SVG、原 CSS 以及可定位的产品图可以继续使用。详见 [资源缺口](../re/04-resource-index.md)。

## 6. 重构前基线与验收

`[RUST 基线]` [pairing.rs](../../src/features/pairing.rs) 与 AppShell::toggle_pairing/complete_pairing/unpair_all 仅更新本地状态；complete_pairing 拼出“已配对设备 n”，不是扫描结果，也没有 DUALLINK 响应驱动。

`[建议]` 用明确状态机和类型化请求保存设备身份；界面只根据结果变更配对结论。验收覆盖空结果、多候选、双通道、绑定确认、解绑失败、取消、关闭设备窗口、重开去重、键盘焦点及断连。当前静态审计不证明上述硬件请求已在 Rust 实现。

## 7. 当前原生实现与原版细节（2026-10-02）

当前页面由 [pairing_page.rs](../../src/shell/pairing_page.rs) 和 [pairing_state.rs](../../src/shell/pairing_state.rs) 负责；第 6 节的 `features/pairing.rs` 是旧版基线，不是当前路由。

- 4130 的 `Y`（收回外部设备）与 `z`（显式解除外部配对）是两条流程。前者发送一次解绑，在响应或 2 秒后开始真实扫描；后者先发送 dongleId，再在确认或 2 秒后发送 productId，保留总共约 10 秒的等待边界。Rust 已拆分请求；计时器只触发后续请求，不把外部记录标记为成功解绑。
- 收回后必须重新扫描到同一设备身份才会继续绑定；仅 productId 相同的另一台设备不会被自动选中。外部记录断开后没有直接绑定入口；它必须重新出现在真实扫描结果中。替换本机同通道已有配对时，先等每个解绑确认，再绑定目标。
- 原版 `pi` 会在键盘扫描或绑定失败后继续鼠标扫描。Rust 保留该分支，并分别保留通道请求目标与 4 秒错误恢复任务，避免一个通道的恢复清除另一个正在处理的通道。
- 原版取消分支连续发送 `DUALLINK_CANCEL` 与 `DUALLINK_BIND_INFO`，没有 CANCEL 响应处理。Rust 只发送一次取消并刷新绑定信息；退出、接收器变更、外部所属接收器记录变更都会废弃旧请求编号和任务。重新进入已初始化页面会重新读取真实绑定。
- 确认层保留 Tab 焦点限制、Escape、Enter/Space、外部点击关闭和焦点返回。按钮自行处理键盘激活，容器只处理自身获得焦点时的确认，避免同一次按键重复提交。打开确认层期间刷新和接收器切换禁用；设备信息变化后旧确认失效。
- 4130 `T` 的 `powerStatus.chargingStatus`、`chargingStatus`、大小写不敏感的 off/standby/sleep，以及 connected/status 影响图像和名称的变暗，不会因此抹去已知配对记录。产品名按语言大小写兼容匹配并回退到有效英文名称。
- 4130 `wt` 使用 `PluginImages/*_dashboard3x`，键盘 layout 为 1、鼠标为 0；配对卡现在使用专门的 dashboard 图片解析，缺图时显示类别占位。Customize 的 `prd_*` 图片不作为配对卡图片。

尚未接入真实 DUALLINK 传输、allMasters/外部所属接收器记录和运行时设备数据源，HID 枚举与键位 DLL 都不能替代这些响应。三个传输/元数据入口保留精确说明；默认页面不制造候选、扫描结果或配对成功。本轮仅做静态检查和编译检查，新增 [回归用例](../../src/shell/pairing_tests.rs) 未运行，硬件交互与原生窗口视觉/焦点仍待实际运行验证。

### 弹出层专项对照

4130 `Zs` 的配对/解绑确认始终传入 `inline: true`：卡片内整面 `#111` 遮罩、顶部居中、宽 230、padding 20、圆角 5、橙色 1px 边框、`#1a1a1a` 内容底色；对话框本身没有 220 高度限制。正文为 `#999`、12/14.4 字号/行高，仅段落之间间隔 10；下方只有一个居中的 90×27 继续按钮，按钮 padding 0、圆角 3、`#222` 文字，hover 为 `#ff9530`。当前 Rust 按该层级布局，去掉了误加的固定最小高度、拉伸按钮和末段额外空白。

713 续接使用的是 [182 main CSS](../../.ref/devices/182/static/css/main.48c20423.css) 的 `DualLinkWarning`，不是上述卡片确认。已改为独立窗口居中的 400 宽警告层：`#111111b3` 遮罩、`#111` 内容底色、20 内边距、3 圆角、橙框和三段专用警告文案。动作顺序保留“继续配对”在左、“使用键盘接收器”在右；后者发送取消并刷新，不假装已切换硬件。原生 Base Dialog 提供独立叠放层及焦点限制；背景点击不关闭该警告，Escape 执行取消。

普通设备卡的徽标恢复为左上角绝对定位（10,10），不占用图片/名称的垂直空间；扫描/配对/解绑忙碌徽标位于 140 高图片区域左上角。去掉原版不存在、会撑高卡片的额外来源与错误段落；服务错误仍在列表上方显示。骨架卡恢复 10 内边距、76×20 描边徽标、248×99 图片区和 16/8 间隔。相关几何与 713 双动作回归用例已添加，仅编译、未执行。
