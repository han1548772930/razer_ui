# 接收器及底座第二轮独立复查（2026-10-07）

本轮由独立子任务从当前产品原码复核 179、164、241 的已实现页面与配对工具。仅静态解析当前 `.ref/devices/<pid>`；没有运行应用、构建、测试、安装器、下载 JavaScript 或 DLL。该记录持续更新，不表示三个产品或所有页面完整验收。

## 来源与实际挂载

| 产品 | 当前源文件及 SHA-256 | 本轮回读范围（Acorn UTF-16） |
| --- | --- | --- |
| 179 | `.ref/devices/179/static/js/main.4849f7ca.js`；`f5d6efe15f34167d61020469f151c3a0827f695cb35bc49cbdd32c23957fb874` | `J` 4086288–4086596；`ae` 4086881–4087571；`oe` 4090530–4093036；`ne` 4093040–4096137；`re` 4096141–4097083；`se` 4097087–4103118；`Te` 4103122–4108440；`mE` 4243839–4244810 |
| 164 | `.ref/devices/164/static/js/main.458d4103.js`；`0324ddd7980caf41f07eeea62979c15579578cba2a13ac302a8347e6d338fb40` | `Re` 66663–71018；`Pe` 72173–72932，及配对状态组件（范围详见收据） |
| 241 | `.ref/devices/241/static/js/914.4c13bdac.chunk.js`；`1320d5e2c7ee42497e590f2dbd979b9d10614573746003b3b87b6695dfc6a9d2` | `Es` 45838–46091；`Ps` 48748–54333；`bs` 54400–54531，及配对状态组件（范围详见收据） |

收据：[179 配对主体](receiver-pairing-current-evidence.json)、[三个产品交互](receiver-interactions-current-evidence.json)、[164/241 工具、语言与资源](dock-pairing-current-evidence.json)。上述范围直接回读当前源码，未把历史链接改名当作重新审计。

179 已确认实际挂载 `mE → Te → G/se → re → ie/oe/ne/ae/_e`；本地 `receiver.rs → receiver_pairing_body` 挂载加载、双类别选择、候选、进度、已配对、确认解绑及错误主体。本地 Scan/Bind/Unbind 均产生未发送意图，只有真实 publisher 观察可改变设备结果。

## 本轮修复：179 操作代际及进度

原本 `session` 仅在打开弹层时递增。撤销扫描/绑定后，同一会话的迟到 `scanned/bound/unbound/failed` 仍会被接收；新一次查询也可能被旧查询覆盖。本轮将 `ReceiverPairingEvent::session()` 明确为**操作代际**：每次请求、关闭及取消递增。观察必须同时匹配当前代际与请求种类；重复结果不能再次自动触发 Bind。

取消解绑及“撤销未发送意图”现在发出 `Cancel`，供 shell 丢弃该工具的查询。取消只撤销本地意图并保留已观察绑定，不制造解绑成功。关闭或重新打开后，旧响应不能再修改当前弹层。shell 消费者应始终回传它接收的 `event.session()`；父任务已确认其查询实现遵守此契约。

此前 `Progress` 清掉 `pending` 后，升级 → 扫描的下一个进度观察无法匹配；扫描结果也可能丢掉没有元数据的候选类别。现在单独保留 `active`，`pending` 只表示尚未被接收的本地意图。类别从活动请求读取，进度后最终结果仍按同一操作代际接收。

候选刷新保留仍有效的选择索引，列表变短时将索引限制在新列表内，防止可见 Pair 点击后因索引越界无动作。这是本地状态边界修正，原 `J` 仅合并 `SET_SCANNED_INFO`、没有替本地做越界处理。Peer 的语言查找容忍 locale 大小写差异，避免真实 `zh-CN` 字段因 UI 传 `zh-cn` 错误退回英文。

更新既有状态机测试源码并补入迟到结果、查询代际、错误种类、连续进度、自动绑定代际及候选缩短覆盖。**没有运行测试**；这些源码不能作为 GPUI 实窗或交互运行验证。

## 本轮修复：底座提取一致性

`node tools/extract-dock-pairing.cjs --check` 原先失败，而 `python tools/validate-dock-pairing.py` 通过。只读比较确认差异仅为已有人为补充的两个 `config.launchUtilityInfoKey` 与 241 对应十种语言，没有源文件变化。

维护提取器现从当前 `4693/JWw`（164）及 `4693/szJ`（241）读取各自标签：164 为 `ENABLE_LAUNCH_PAIRING_UTILITY_INFO`，241 源拼写为 `ENABLE_LAUNCH_PARING_UTILITY_INFO`。生成配置及完整十种语言后刷新收据，不再由重新生成删除已实现的产品文案。

## 当前明确缺口

- 179 `Te` 主页面的临时配对名称、加载反馈、连接条件和设备导航尚不完整；对话框状态补齐不等于该父页面完成。
- 179 固件版本与 80 字节 IC 分类、固件流程未接入。当前查询 PID/status 不足以启用这些分支，不能推断固件值。
- 179 源真实成功响应后的 1 秒关闭，以及 Bind 失败后 4 秒恢复 LOADED、Unbind 失败后 4 秒恢复 PAIRED，尚未实现。只对真实结果恢复呈现不等于制造设备成功；这些 UI 工作仍在当前范围。
- 164/241 在本轮第二批前缺少正式观察入口，`!preview` 请求直接退出，状态保持 Loading。第二批已补入下述路径；实际只读发布器由父任务接入，扫描/绑定/解绑的 DLL 写操作仍后置。
- 241 轮询率行必须按 `V = !W && (I ? Y : K)` 判定，`I` 来自 `hs` 的连接观察、`Y/K` 来自 `Cs` 的实际 polling 配置；现有仅 `!both_devices_capped` 不等价。连接/能力字段未知时不能以目录或预览补造。
- 所有窗口字体、换行、焦点、命中、裁剪和 DPI 的运行/视觉验收仍未执行。设备写回及 DLL 持久化继续后置。

本轮允许的检查及后续批次结果将在下方追加；父任务统一执行 `cargo check --locked --all-targets`。

## 第二批：164/241 正式配对工具观察和本地操作

再次回读 164 `2715/le`（63623–66659）和 241 `3746/ds`（38810–44775）：两者初始请求 `DUALLINK_BIND_INFO`；真正读取失败进入 LOADED 空卡；扫描结果为数组、Bind 结果为 `{device: ...}`；单候选自动提交绑定意图，241 按类别维护两通道。

新增 `dock_pairing/observation.rs` 并接入正式路径：

- `DockDialog → DockPairing → SourceProductWorkspace → ProductWorkspace` 发出 `DockPairingEvent`，`observe_dock_pairing` 逆向回传真实结果；打开、Retry 和候选 Cancel 产生读取请求。
- Scan、Pair、Confirm Unpair 在正式模式下保留本地意图并显示“尚未发送到设备”，允许撤销；不会直接跳为扫描中、配对中、解绑中或完成。只有明确的真实 `progress`/`result` 观察可以进入对应状态。
- 每个请求采用全局唯一操作代际并匹配消息种类。关闭、Esc 取消确认、按钮取消确认和撤销未发送意图都清除活动操作并发出 Cancel；旧读取、已撤销或重复结果不能改当前状态。
- 真实 Bindings 更新正式父页面的已配对名称和类别；真实错误与成功空数组分开处理并显示错误和 Retry。格式错误也进入明确失败，不能把缺类别或错误载荷当作空设备列表。
- 164 只读结果允许省略 category，按源单鼠标通道呈现；241 需要真实或官方目录解析的 MOUSE/KEYBOARD 类别。edition/layout 为 `Option`，没有读取就保持缺省；名称未知显示 PID，不补造 serial、edition、layout 或设备图。
- 原始候选 JSON 完整保留，绑定意图复用该载荷；不再由本地选定字段重建并丢掉未知字段。`dongleId` 为 0 时与源 `dongleId || productId` 一致回退到 productId。
- 工具图片改为真实 PID/edition/layout 的精确资源匹配，缺字段不调用会归一 layout 的通用 Dashboard helper。

维护交互收据现包含 17 项 AST（新增 le/ds）、56 条专项 CSS 以及新 observation 文件哈希。正式 modal、loading、content、通道和本地意图区域补 `.test_support()`、稳定 ID 与语义 role；Base Button/Radio 延续框架支持。没有执行 GPUI 测试或实窗。

## 本组逐页面复查状态

| 产品 | 页面 / 独立根 | 本轮状态 | 仍需工作 |
| --- | --- | --- | --- |
| 179 | TAB_CUSTOMIZE / 配对弹层 | 原码回读；代际、取消、进度及候选边界已修 | 父页临时名称/连接/导航、固件、结果恢复计时及视觉验收 |
| 179 | HELP | 普通 Reset 当前源回读；本地确认/撤销已修 | 其余帮助子分支和完整视觉验收 |
| 164 | TAB_CUSTOMIZE / 单设备配对工具 | 原码回读；观察/本地请求/正式挂载已补 | 实际读取与条件数据、源结果计时、临时名称/连接提示 |
| 164 | TAB_LIGHTING | 亮度/熄灯及普通快捷效果挂载已回读 | 效果参数、同步与高级效果主体 |
| 164 | HELP | 普通 Reset 当前源回读；本地确认/撤销已修 | 其余帮助子分支和完整视觉验收 |
| 164 | 独立 multiDevicePairing | 本轮未复查 | 挂载、特定状态及正式入口仍需单独核对 |
| 241 | TAB_PAIRING / 双设备配对工具 | 原码回读；观察/本地请求/正式挂载已补 | I/Y/K/W 连接/能力条件、源结果计时、完整焦点/输入 |
| 241 | TAB_LIGHTING | 亮度/熄灯及普通快捷效果挂载已回读 | 效果参数、同步与高级效果主体 |
| 241 | HELP | 普通 Reset 当前源回读；本地确认/撤销已修 | 其余帮助子分支和完整视觉验收 |

以上只登记实际回读范围。接收器、附件、音频其他已实现页面继续由总复查队列逐项处理，不能因为同家族已修一页就标记全部已审。

本轮静态检查：`audit-receiver-pairing-current.cjs --check`、`extract-dock-pairing.cjs --check`、`validate-dock-pairing.py` 已通过；交互收据在加入当前 le/ds 和 observation 实现后刷新。Cargo 结果由父任务统一记录。

父任务随后已接入 `shell` 订阅 → 当前产品已核实的 native V2 只读查询 → 官方目录身份/类别/名称解析 → `DockPairingObservation`。最小真实 payload 不填 edition/layout/serial；无法唯一映射则整次报告错误，不能静默丢弃条目后伪造空结果。本轮没有执行该查询或 DLL，因此只能记录读取链代码已连接，不能记录设备读取运行通过。

## 第三批：Lighting 实际分支与 HELP 确认

[专项收据](dock-lighting-help-review-current-evidence.json)由 `tools/audit-dock-lighting-help-current.cjs` 从当前完整文件解析 17 个精确 AST 节点和局部变量绑定，共覆盖上述 5 个产品页面。

- 164 `vr`（4421298–4421512）及 241 `Mn`（496322–496536）分别无参数挂 `hr` / `fn`；父 Effects 的 `renderQuickEffect` 因没有 `portComponent` 选择普通 `Os→Is` / `mt→vt`。旧 descriptor 误用迹线内同时出现的端口类 `Ir` / `Jt` 作证据。本轮修正维护生成器与两条 descriptor，不能据端口控件推断底座 UI。
- 164 普通快捷效果 `Is` 为 4381892–4388540；241 `vt` 为 457853–464479。亮度与熄灯控件的现有数值范围/禁用依赖已核对；普通快捷效果的颜色等参数、同步、Quick/Advanced 实际主体仍未挂载。
- 三产品 HELP 的 `confirmDel` 均有确认/取消并调用普通 `ON_RESET_DEVICE` 分支，本地确认按钮原先永久禁用。本輪仅为已核实的 164/179/241 开放本地确认：确认后关闭弹层、保留待发请求、页面显示未发送并可撤销；取消/关闭、产品身份和页面代际变化会使旧确认失效。没有修改设备状态或重置本地配置，不把 UI 确认算作设备重置成功。其他 OBM/OLED/固件 Reset 分支未套用。

| HELP 来源 | SHA-256 | AST 范围 |
| --- | --- | --- |
| 164 `main.458d4103.js` | `0324ddd7980caf41f07eeea62979c15579578cba2a13ac302a8347e6d338fb40` | 4507751–4526342 |
| 179 `main.4849f7ca.js` | `f5d6efe15f34167d61020469f151c3a0827f695cb35bc49cbdd32c23957fb874` | 4543328–4561696 |
| 241 `11.219fb515.chunk.js` | `d39889eed8d9c2443c0a57b075018604c8e4283a70ef98dc554ac712f667dc61` | 245316–263984 |

241 Lighting `main.899712fe.js` 的 SHA 为 `65fe2c4069ece8270903895ce32c6c610d5eb8fec94bd944f5791c2376509a02`；164 Lighting 与表中同一主文件。完整绑定链、亮度/熄灯范围及 native 文件哈希见专项收据。

生成器复现另修正了 179 既有手工 indicator renderer 与三个说明字段会被重生成丢弃的问题；这部分现在由当前 `OE` 的源收据与语言导出产生。生成前后逐产品深比较确认语义变化只有 164/241 的效果证据，179 实际内容保持一致。

扩组结果另见 [音频 76 产品复查](audio-review-2026-10-07.md)。本组剩余 [System/AccessorySystem/SourceControls 队列](accessory-system-review-queue-2026-10-07.json)包含 89 产品 / 325 主页面及 3 个独立模式；只给本轮实际回读的 8 页标记局部子树复查，其余 317 页和独立模式明确待审，完整 UI 通过数仍为 0。
