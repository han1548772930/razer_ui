# 回报率来源、共享状态与实际读值复查

2026-10-07。本批按用户要求移除以型号命名的回报率运行文件；`mouse_226_polling.rs` 和临时的 `mouse_182_polling.rs` 均已移走。完整页面、完整产品仍未验收。

## 当前结构

- [mouse_polling.rs](../../src/features/mouse_polling.rs)：共享 Scope、Observation、RuntimeState，以及当前前端的字段、配对身份、候选档位和限速选择逻辑。没有型号 ID、接收器 ID、档位数组、固件门槛或型号策略分派。
- [mouse_polling_view.rs](../../src/features/mouse_polling_view.rs)：现有 JSON 产品工作区的面板与本地覆盖标记。
- [mouse_polling_profile.rs](../../src/features/mouse_polling_profile.rs)：原有 ProfileSettings 工作区的面板与独立有线/无线草稿。两个 owner 共用同一 RuntimeState，不再从型号文件借用观察 API。
- [mouse_polling_source_data.json](../../src/features/mouse_polling_source_data.json)：当前源提取结果。产品 ID 仅选择其实际 CONFIG；表中常量不是设备读取结果。

维护工具 [prepare-mouse-polling-model.cjs](../../tools/prepare-mouse-polling-model.cjs) 逐份读取当前 manifest、AST、CSS 与原 SVG。当前独立重审的是 182 和 226 两个普通 Performance 根；其余产品不能因存在共享模型自动视为接入或验收。收据见 [mouse-polling-model-current-evidence.json](mouse-polling-model-current-evidence.json)。

## 原码分层与每类数值来源

| 输入 | 当前源与实际用途 |
| --- | --- |
| 产品、dongle 身份 | 各产品模块 1057 的 DeviceInfo；用于该产品自身的配对记录选择。182 Rm 用严格 dongleId 比较；226 zs 的 isCurrentDevice 用当前配置 productId/dongleId 做 String 比较。没有从 179 或 183 的目录条目推造连接。 |
| 基础、无线、HyperPolling 与 8K 档位 | 各自 CONFIG 的 POLLING_RATE、POLLING_RATE_WIRELESS、HYPER_POLLING_RATE、HYPER_POLLING_RATE_SUPPORT_8K；由普通根实际挂载的 Rm/zs 使用。 |
| 主接收器身份集合 | 当前 9937 的 zrS/WgN 导出；182/226 的 checkIfPairedWithHyperPollingDevice 使用。226 额外接受实际 master.supports8KHzPollingRate===true。 |
| 固件门槛 | CONFIG 的 POLLING_RATE_8K_FW_VERSION；没有该导出时不自行补门槛。固件值来自实际 DEVICE_RUNTIME_DATA/当前只读查询，而不是该常量。 |
| 版本比较 | 当前 1867.U5 及其 npm semver valid/coerce/gt 依赖 4547/8988/6130/8490/8816/1468/2574/2314/8209。锁中 semver 和 regex 用于合法语义版本及源码 COERCE；保留 U5 的字符串相等早返、无效 coerce 返回和独立 SemVer 对象的引用比较语义。不能把第四段改成自创数值大小比较。 |
| 共享连接限速 | DeviceInfo.dualLinkPollingRateLimitHz；dock cap 与最少 master 记录数取自 zs.applyMultiDeviceDockPollingCap 和 Ls 的真实 AST 字面量。仅该源存在这些函数时才有对应规则。 |
| 高频提示边界、无线初态 | Rm/zs 的条件表达式，以及各自 polling reducer 的 pollingRateWireless 临时初态。初态仍标识为本地配置，不能冒充设备值。 |
| BLE 可见性 | 182 OM 直接在 isBle 时不挂 Rm；226 wi 根据 isBle 与实际 DeviceInfo.supportBluetoothPollingRate 门控，本产品为 false。提取器对这两个普通根记录 hidden，并在源门控变化时要求重新取证；没有借这个字段宣称已经实现 BLE 回报率编辑。 |
| 标题、说明、帮助 | 当前 4693。修正原 182 注释把 FGZ/lGq 错称 HYPERPOLLING 的问题；实际为 POLLING_RATE_HEADER/WIRED_POLLING_RATE_HEADER。 |

源 U5 的特殊行为没有被美化：coerce 返回不同的 SemVer 对象，`o===n` 不是版本字符串相等；不同四段原字符串即使前三段相同，也不能用本地数值比较让 8K 自动通过。缺少固件观察仍不进入此门槛判断。异常固件字符串的源判断结果也不等于成功读取或设备能力实测。

## 界面与读值接线

182 原有适配器实际走 ProductWorkspace → DeviceWorkspace，之前修改 mouse_products 通用 family 不能修复它。本批接入独立 wired/wireless 草稿与实际连接观察，去掉以旧 real_product_id/use_ble 判断当前连接的分支。标题、帮助、说明、高频警示、外链尺寸按当前源补正，移除错误套用的 polling-rate 上距。Power 的低功耗锁定同时改读当前连接对应回报率；写入口也检查锁定条件。

设备发现分两阶段：接口/peer 事件先建立或更新工作区并写 Connection；在参数查询开始前捕获 owner/profile/connection scope，最终 DeviceValuesObserved 使用原 scope 发布 Rate/Firmware。最终事件不再次重置连接。profile 切换、恢复或连接代际改变会拒绝旧参数；设备级只读值独立保留在 serde-skip 元数据，不写 profile/draft，不触发 Changed，也不宣称设备保存成功。

源静态默认值与真实观察分开：未编辑字段可显示真实 Rate；显式 local override 优先且注明尚未发送到设备。连接未知、回报率未读取、HyperPolling 配对能力未读取均有对应提示。基础档位不是已成功读到的硬件配置。

Windows 和外链图已逐字节确认可复用已注册的通用资源。另补 polling-info 到资源 load/list；此前只准备了磁盘文件、未在资源加载器登记的路径不再作为成功加载证据。

## 尚未完成

- 当前没有 duallink-devices 完整快照生产者，因此 HyperPolling 配对能力与 dock 拓扑仍未知；不由目录猜出 8K。共享固件和 Rate 消费链已接线，当前独立审计的只读查询生产链仅覆盖 182，不能据此宣称 226 或其他型号已能读取；未运行应用或 DLL 验证。
- 182 Rm 的候选表检查发生在 componentDidMount；226 zs 另有 focus/1500ms 快照更新。共享公式不等于完整生命周期相同；在快照生产者接入前还需保留各源的挂载/刷新节奏，不能由每次收到快照就统一重算宣称原行为验收。
- 182 DPI 仍有数字框注册/预览与 window wheel、阶段细节等旧差异；Customize、Calibration、Help 与全部其他产品继续逐页审查。本批回报率修复不是整鼠标完成。
- 验证仅为当前源静态解析、资源字节与登记检查、格式化；Cargo 由父任务统一执行。不运行应用、测试、安装器、下载 JS 或 DLL。
