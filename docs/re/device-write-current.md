# 当前直接设备写入

当前直接 DPI/回报率/休眠计时写入资产覆盖 22 款鼠标的 64 项操作，其中休眠计时覆盖 20 款；每款产品分别核对 factory、setter、参数编码、应答 parser、实际 caller 和读回链。另有 9 款键盘的活动矩阵亮度，以及产品 1342 Audio Mixer 的源核实 DSP/寄存器、硬件音量/静音、六条硬件路由与 22 条驱动矩阵路由。182 鼠标休眠、当前回报率与 9 款键盘亮度已连接页面事件、agent/IPC 和真实回读；其他页面和未闭合分支仍需继续实现。开发验证仅进行静态源码/PE 分析、资源检查、格式化和 `cargo check`；运行验收未进行。

回报率逐产品核对 feature 注册默认值、OBM 启动覆盖、实际连接选择器及读写任务。物理直接连接的普通分支为 148、156、165、185、211、222，板载 profile 分支为 124、143、167、170、175、180、182、184、192、196、204、218、231、45066，高速分支为 226、235。接收器动态连接状态尚未闭合，运行时明确拒绝未证明的回报率 relay 路由。证据由 [回报率静态审计](../../tools/expand-polling-capabilities.cjs) 保留。

## 当前已实现的协议入口

| 产品/操作 | 当前 Razer 原码 | 实现及范围 |
| --- | --- | --- |
| 生成资产中已核实产品 / 活动 DPI X/Y | `setDpiLevel(0,x,y)`，header `[7,4,5]`；X/Y/Z 大端 u16，原 caller 省略 Z、helper 默认 0 | 按各产品源范围先读当前值；不同才设置；检查原 setter parser 的应答，再回读比对 X/Y；不写持久化 DPI 阶段表 |
| 6 款鼠标 / 普通回报率 | getter `[1,0,133]`、setter `[1,0,5]`，无 profile selector；Hz 编码为 1000:1、500:2、333:3、250:4、200:5、125:8、100:10，未知读码按原源回退 100Hz | 仅物理产品匹配时发布能力；读取 data[0]；原任务先读、不同才写并回读，Rust 保留此确认链 |
| 14 款鼠标 / profile 回报率 | getter `[2,0,142]`、setter `[2,0,14]`，caller 默认 profile 1；相同 Hz 枚举及 100Hz 回退 | getter 校验 profile 并读取 data[1]；先读、设置、回读 |
| 226、235 / USB 高速回报率 | getter `[2,0,192]`、setter `[2,0,64]`；profile 默认 1，Hz 转当前源枚举，未知读码回退 8000Hz | 实际 `isHyperpollingDevice=true` 分支已核实；原 polling task 包含读写确认，并非仅本应用补加回读 |
| 生成资产中 20 款鼠标 / 休眠计时 | `getTimeToSleep [2,7,131]`、`setTimeToSleep [2,7,3]`；两字节大端，无 selector；原 task setter 后读回，不符抛 `data not match` | `Idle { raw_time:u16 }` 已接直接 HID 和 worker/IPC；182 页面 Release 按源将 1..15 分钟乘 60 后提交；其余产品页面及原任务缓存/版本持久化/取消链仍缺 |
| 600、602、664、697、708、716、717、724、727 / 键盘活动亮度 | getter `[3,15,132]`、setter `[3,15,4]`；活动 profile 1、AllRegion 0；写值 floor(percent/100*255)，读值 ceil(byte/255*100) | 页面滑块拖动预览、Release/toggle 提交到 worker/IPC；region 查询 `[80,15,128]` 的首条记录取 region ID，空返回保持 0，仅查询拒绝回退 1；setter 应答后另加本应用回读确认；硬件 profile 同步及原存储任务仍缺 |
| 1342 / Audio Mixer DSP、麦克风监听 | `CmMixerPropertyControl` 37 项分发中的 26 个 DSP/寄存器控制，已包含 `RazerT2MicMonitorVolumeControl`（`0x5ffc002c`）及固件只读查询 | selector/mailbox、类型、范围、量化及回读确认由 `audio_mixer.rs` 执行；固件版本拒绝写入 |
| 1342 / 五个硬件端点 | `Volume/Mute/Peak` 的 jack/flow 分支展开 15 个控制项 | 音量 Both/0/1 按原顺序及位保留规则；静音按原极性；峰值读取后清零，不接受峰值 setter。COM 分支仍缺实现 |
| 1342 / 六条硬件混音路由与 22 条驱动路由 | `RazerT2MixerSettingControl`，`0x5ffc0010` | 硬件路由位读取/设置并回读；Windows 平台驱动矩阵按原布局读改写并回读，ResetStream 按原 0..8 顺序发送；真实 ContainerId 与驱动接口关联尚未实机验收 |

完整原文、SHA-256、UTF-16 半开区间、delegate、字节编码器、parser 与实际 caller 保存在 [写入证据](device-write-capabilities-current-evidence.json)。[生成器](../../tools/audit-device-write-capabilities.cjs) 只用 Acorn 解析当前原件并校验原读取/factory 收据，不导入厂商 JS。运行资产为 [device-write-capabilities.json](../../assets/data/device-write-capabilities.json)。

休眠计时由 [逐产品审计](../../tools/expand-idle-write-capabilities.cjs) 保留实际 `isBattery=yes` 注册及 setter/readback task。8 个产品原包装的字节 helper 导出名和 `rzError` 日志模板变量不同，逐一核实两个 helper 都为高/低字节函数、错误类的字段与实际参数转发后才规范化比较。原 caller 普通产品先乘 60，`singleProfileDevice` 则使用原 UI 值；协议 API 保留原始 u16，也不设置休眠使能位。读取扩展至 31 款产品/157 项查询，计时读数保存在 `idle_raw_time`，不会自动改写本地草稿。182 的 [UI 链证据](mouse-idle-ui-current-evidence.json) 分别登记拖动预览、Release 提交、pending、旧响应过滤及真实回读更新。

键盘 [静态取证工具](../../tools/audit-keyboard-settings-current.cjs)、[原链证据](keyboard-settings-current-evidence.json) 与 [能力资产](../../assets/data/keyboard-settings-capabilities.json) 单独保存当前源码和 UI 挂载门禁；708 的实际亮度组件位于 manifest 声明的 lazy chunk，717 的 getter header 引用经 AST 解析为相同已验证命令后才比较。它们不计入上述鼠标 64 项操作。

182 [回报率页面证据](mouse-polling-ui-current-evidence.json) 核实 Rm 按钮包括重复点击当前 Hz 的提交链，失败后可重试。当前有线写入使用已核实的活动/default profile 1 入口，读回确认后单独更新设备观察。原 Ye(activeProfile,obmData) 返回匹配 GUID 的 slotId 数组，rt 对真实槽位逐个读写；这一板载持久化链尚未实现，不能以 profile 1 替代。Idle/Polling 页面共用待处理槽；底层仍以路径文件锁覆盖整个读取、设置和回读，竞争返回错误，原任务队列的等待/跳过/合并另有缺口。

182 [DPI 页面证据](mouse-dpi-ui-current-evidence.json) 已追到实际默认 setDpiStages 分支：`[80,4,6]` / `[80,4,134]`，有效数据长度为 `7*count+3`，包含活动阶段及逐阶段 X/Y/Z。[独立阶段表协议与路由证据](mouse-dpi-stages-current-evidence.json) 已恢复 profile1、setter index0/getter index1、可见行过滤与活动位置重映射，接通共享HID/Windows真实目标、IPC及typed回读验证；setter应答保留原80字节数据，不把原任务忽略的jsonData当成完整getter表。当前 selector 0 的活动 DPI API 不等于这一页面链。设备表不含隐藏行、enabled或independent，不将读数伪装成完整profile；页面消费者、实际板载槽位获取、版本/任务缓存与持久化仍需完成。

设备 API 位于 [device_writes.rs](../../crates/razer-device/src/device_writes.rs)，协议不依赖系统后端或原 DLL。复用 source-derived Feature envelope、事务、状态及重试参数，按各产品当前源门控选择能力；不能从共享基类方法存在推定全部产品支持。

## 路由、确认与失败

IPC 包含 `HidNodeWrite { node, product_id, setting }`、`DeviceWrite { target, setting }` 和 `HidNodeMixerWrite { node, product_id, target, value }`，另有独立键盘亮度 Read/Write 请求。HID node 入口使用跨平台后端；Device target 入口复用 Windows 真实 ContainerId/接口观察；Mixer 使用精确 Audio Mixer collection 和源核实 Report。请求在 agent 执行并回读确认；接收器 relay、BLE 和未闭合 COM/驱动分支仍缺逐产品证明。

页面读写统一经 [direct.rs](../../crates/razer-discovery/src/direct/mod.rs) 选择来源核实的接口。Windows ContainerId 解析隔离在单独适配文件；Linux/macOS 使用真实 HID node、原始路径字节及 usage/interface 身份，描述符查询只观察传输规格。未知接口号、节点不唯一或 report 长度不符明确失败，不构造 GUID、不选取首个 VID/PID。页面完成回调同时过滤连接、节点、配置及请求作用域。

同一打开的 transport/锁覆盖读取、设置与回读，操作期间重查真实身份。DPI 校验当前产品源码范围；回报率只接受原枚举。发送完成、收到 setter 应答和设置回读成功分别处理；超时、断连、应答失配或回读不符返回错误，不构造成功状态。应答不明时提示设备可能已接受设置，调用方需要重新读取；不自动以本地草稿覆盖真实观察。

写入确认过程使用 20 秒应用预算，IPC 为 30 秒；这是本项目上限，不是厂商协议常量，不能中断阻塞中的 OS 调用。直接 DPI/高速回报率/休眠值已经一致时返回实际读取结果、`changed=false`，不下发 setter；休眠的预读与跳过相同值是本应用策略，原 task 总是先 setter 再 getter。Mixer 按原设置配方执行并另外回读，没有这一跳过逻辑。

## 未完成的范围

182 休眠、当前回报率与上述 9 款键盘亮度已接页面编辑事件，校验真实目标、请求值、verified 与回读，并过滤旧连接/作用域响应。恢复配置或断连不释放仍运行的写入槽，真实请求结束后再清理，避免启动竞争 worker；旧作用域结果不发布当前设备成功。其他页面的本地编辑/保存仍是本地草稿。原 profile/version 持久化、NORMAL_SKIPPABLE 队列、memory cache 和 worker abort 传播仍需逐项实现。

键盘 Lighting 页面取得真实有线连接后可初次读取亮度，并独立呈现观察值、加载与错误；不以设备读数覆写本地草稿。读写共用槽，编辑版本过滤旧读取，排队的用户写入在读取结束后优先提交。初次读取是本应用观察策略：原初始化实际是保存 profile → ON_INIT_BRIGHTNESS → setter，逐产品原链已保留。该初始化恢复、profile producer/global lock、回读到全部控件同步及原 `adjustmentModeRunning`/`nanoLeafEnabled` 状态生产链和控件门控仍未完成；不能据此标记整个亮度页面完成。Mixer 原件 hash、连续指令、声道、类型/范围、COM/driver 与差异详见 [专项协议](audio-mixer-dll-protocol-current.md)。

这不是所有 DLL 全功能完成。全量分母、二级库、COM/驱动/服务与未恢复函数继续见 [DLL 全量清单](dll-function-inventory.md)、[内部通信逆向](dll-device-communication-current.md) 和 [Mixer 二级链](audio-mixer-dll-protocol-current.md)。其他键盘功能、耳机、灯光、映射、配对、固件以及其他产品写操作仍需逐项闭合原件证据并实现。没有进行实机或三平台运行验收。
