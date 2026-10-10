# 当前直接设备写入

当前直接 DPI/USB 高速回报率/休眠计时写入资产覆盖 22 款产品的 51 项操作，其中休眠计时覆盖 20 款鼠标；每款产品分别核对 factory、setter、参数编码、应答 parser、实际 caller 和读回链。另有产品 1342 Audio Mixer 的源核实 DSP/寄存器、硬件音量/静音、六条硬件路由以及 22 条驱动矩阵路由写入与 agent/IPC 入口。页面写入事件尚未连接，其他产品和未闭合分支仍需继续实现。开发验证仅进行静态源码/PE 分析、资源检查、格式化和 `cargo check`；运行验收未进行。

本次修复静态解析器未识别 DPI setter 三个转发参数的问题；同时把高速回报率产品选择改为 AST feature 核对，正确识别带第三个 options 参数的 `useFeature`。没有把普通 `getPollingRate` 或板载 `getProfilePollingRate` 套用高速命令：这两支仍保留缺口，必须单独核实当前 caller 的 feature 条件。

## 当前已实现的协议入口

| 产品/操作 | 当前 Razer 原码 | 实现及范围 |
| --- | --- | --- |
| 生成资产中已核实产品 / 活动 DPI X/Y | `setDpiLevel(0,x,y)`，header `[7,4,5]`；X/Y/Z 大端 u16，原 caller 省略 Z、helper 默认 0 | 按各产品源范围先读当前值；不同才设置；检查原 setter parser 的应答，再回读比对 X/Y；不写持久化 DPI 阶段表 |
| 生成资产中已核实产品 / USB 高速回报率 | `setUSBHighSpeedPollingRate(profile,hz)`，header `[2,0,64]`；profile caller 默认 1，Hz 转当前源枚举编码 | 只有 factory、feature、caller、setter/parser 和对应读回证据均通过才发布能力；检查应答并回读，回读是本应用确认策略 |
| 生成资产中 20 款鼠标 / 休眠计时 | `getTimeToSleep [2,7,131]`、`setTimeToSleep [2,7,3]`；两字节大端，无 selector；原 task setter 后读回，不符抛 `data not match` | `Idle { raw_time:u16 }` 已接直接 HID 和 worker/IPC，getter 自动进入现有观察链；UI 的分钟转换、任务缓存/版本持久化和取消链仍需接入 |
| 1342 / Audio Mixer DSP、麦克风监听 | `CmMixerPropertyControl` 37 项分发中的 26 个 DSP/寄存器控制，已包含 `RazerT2MicMonitorVolumeControl`（`0x5ffc002c`）及固件只读查询 | selector/mailbox、类型、范围、量化及回读确认由 `audio_mixer.rs` 执行；固件版本拒绝写入 |
| 1342 / 五个硬件端点 | `Volume/Mute/Peak` 的 jack/flow 分支展开 15 个控制项 | 音量 Both/0/1 按原顺序及位保留规则；静音按原极性；峰值读取后清零，不接受峰值 setter。COM 分支仍缺实现 |
| 1342 / 六条硬件混音路由与 22 条驱动路由 | `RazerT2MixerSettingControl`，`0x5ffc0010` | 硬件路由位读取/设置并回读；Windows 平台驱动矩阵按原布局读改写并回读，ResetStream 按原 0..8 顺序发送；真实 ContainerId 与驱动接口关联尚未实机验收 |

完整原文、SHA-256、UTF-16 半开区间、delegate、字节编码器、parser 与实际 caller 保存在 [写入证据](device-write-capabilities-current-evidence.json)。[生成器](../../tools/audit-device-write-capabilities.cjs) 只用 Acorn 解析当前原件并校验原读取/factory 收据，不导入厂商 JS。运行资产为 [device-write-capabilities.json](../../assets/data/device-write-capabilities.json)。

休眠计时由 [逐产品审计](../../tools/expand-idle-write-capabilities.cjs) 保留实际 `isBattery=yes` 注册及 setter/readback task。8 个产品原包装的字节 helper 导出名和 `rzError` 日志模板变量不同，逐一核实两个 helper 都为高/低字节函数、错误类的字段与实际参数转发后才规范化比较；没有把任意不同函数按名称放行。原 caller 普通产品先乘 60，`singleProfileDevice` 则使用原 UI 值；协议 API 保留原始 u16，不猜分钟，也不设置休眠使能位。读取扩展至 31 款产品/149 项查询，新的计时读数保存在 `idle_raw_time`，不会改写本地草稿。

设备 API 位于 [device_writes.rs](../../crates/razer-device/src/device_writes.rs)，协议不依赖系统后端或原 DLL。复用 source-derived Feature envelope、事务、状态及重试参数，按各产品当前源门控选择能力；不能从共享基类方法存在推定全部产品支持。

## 路由、确认与失败

IPC 增加 `HidNodeWrite { node, product_id, setting }`、`DeviceWrite { target, setting }` 和 `HidNodeMixerWrite { node, product_id, target, value }`。前者走跨平台 HID node，Mixer 请求使用精确 Audio Mixer collection 和源核实 Report；后者复用现有 Windows 真实 ContainerId/接口观察。所有请求都在 agent 执行并回读确认；接收器 relay、BLE 和未闭合 COM/驱动分支仍缺逐产品证明，不套用直接设备报文。

同一打开的 transport/锁覆盖读取、设置与回读，操作期间重查真实身份。DPI 校验当前产品源码范围；回报率只接受原枚举。发送完成、收到 setter 应答和设置回读成功分别处理；超时、断连、应答失配或回读不符返回错误，不构造成功状态。应答不明时提示设备可能已接受设置，调用方需要重新读取；不自动以本地草稿覆盖真实观察。

写入确认过程使用 20 秒应用预算，IPC 为 30 秒；这是本项目上限，不是厂商协议常量，不能中断阻塞中的 OS 调用。直接 DPI/高速回报率/休眠值已经一致时返回实际读取结果、`changed=false`，不下发 setter；休眠的预读与跳过相同值是本应用策略，原 task 总是先 setter 再 getter。Mixer 按原设置配方执行并另外回读，没有这一跳过逻辑。

## 未完成的范围

上述直接写入已接到 agent/IPC，尚未接产品页面的编辑事件；本地编辑/保存继续是本地草稿。UI 接入需要对应连接代际、配置作用域与异步请求队列，不能把本地保存改名为设备写入成功。Mixer 原件 hash、连续指令、声道、类型/范围、COM/driver 与差异详见 [专项协议](audio-mixer-dll-protocol-current.md)。

这不是所有 DLL 全功能完成。全量分母、二级库、COM/驱动/服务与未恢复函数继续见 [DLL 全量清单](dll-function-inventory.md)、[内部通信逆向](dll-device-communication-current.md) 和 [Mixer 二级链](audio-mixer-dll-protocol-current.md)。键盘、耳机、灯光、映射、配对、固件以及其他产品写操作仍需逐项闭合原件证据并实现。没有进行实机或三平台运行验收。
