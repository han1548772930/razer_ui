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
