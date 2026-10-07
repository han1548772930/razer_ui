# 鼠标仍未读取：当前排障事实与修复

用户继续反馈鼠标未读取后，本批优先检查真实发现链。没有运行应用、测试、DLL、厂商 JavaScript 或硬件探针；静态检查通过不表示本机鼠标已经读取成功。

已有本机 Razer 日志记录过 **DeathAdder V3 Pro／炼狱蝰蛇 V3 专业版，产品 182**，以及 **179 HyperPolling 接收器**。其中查询 peer PID 183 对应当前目录的 182；本轮静态检查确认该映射唯一且正确。日志、注册表留存的 USB/HID 容器都是历史证据，不当作本次在线结果。未把旧序列号、电量、固件或配置灌入应用。

启动链实际存在：`AppShell::new` 在订阅安装后调用 `RuntimePanel::discover_devices`，后台 worker 分别请求 `UsbDevices` / `HidDevices`，调用 `discovery::discover`，再发布 `DiscoveryObserved` 供 shell 创建或更新工作区。SimpleService 的版本/音频读取不是鼠标查询的替代品。现有 `debug.log` 只留有旧 SimpleService 记录，没有这次本项目发现失败的 USB/HID/Feature 响应，因而尚不能确定用户这次失败发生在哪一步。

发现并修复了当前原码可以直接证明的差异：原版允许同设备多个 HID collection，本地此前要求匹配候选数必须恰好等于 1。

- 当前 4.0.827 `connectHidDevice` 使用 `for(const l of s)`，在 VID/PID/container 和 claimInterface 匹配的多个 collection 中依次尝试，成功后 break。源文件 `.ref/host-4.0.827/source-evidence/background-current-source.js`，SHA-256 `382ad87a9715c6417bac9d9f89a0a557521eaed4ed56a2512bc50c2d449b77c0`，AST 区间 `[20486,21538)`。本批重新核对文件 hash、原文和 MethodDefinition 边界；原收据位于 `receiver-capabilities-current-evidence.json`。
- 本地 `discovery::receiver_request` 与 worker `select_target` 原先两层以 `len == 1` 拒绝整组，导致同一真实接收器的合法多 collection 不能继续查询。现改为 `query_receiver` 按枚举次序尝试同 VID/PID/container/接口/91 字节 Feature 的候选，任何路径读取失败仍记录原因；成功必须通过完整 binding 结构校验，全部失败保持错误。
- worker 仍要求指定路径只出现一次，并逐项验证真实产品、容器、实例、接口、Feature 长度及查询前后身份。协议事务 ID、命令、返回状态、长度校验没有放宽；没有选择另一个容器来冒充成功，也未添加设备配置写入。
- 启动发现、底座配对信息查询、接收器配对信息查询共用该实现。两个仅编译的回归案例覆盖首 collection 失败后第二成功、其他容器/接口不参与、畸形回复或全部失败不返回空成功。

本机历史日志的某旧容器确有多条 MI00 collection 的逐次打开和失败；另一个近期容器在 2026-10-04 18:47 的官方日志只列一条 MI00 并成功打开。因此多 collection 是已证实的结构缺陷，但没有本项目当前枚举响应，不能断言它就是这次问题的唯一根因。

另一明确缺口是此前只有启动一次与连接页手动刷新，没有持续 USB/HID 拓扑订阅。主任务负责新增拓扑事件触发、忙时合并刷新及持久诊断。USB/HID 热插拔通知不等于接收器内部无线开关事件；后者仍需对应 MW 状态订阅。

原配鼠标 dongle 还有一个身份边界：目录中的 `dongleId` 本来指向鼠标产品，查询返回与物理 PID 相同的 peer 行仍能包含鼠标无线状态。发现层原先一律跳过 `raw_pid == physical_pid`；现在仅对独立接收器自身行保留这一过滤，对目录已确认的鼠标 dongle 保留真实 peer 回复。单有 dongle USB 接口、没有 online peer 回复时，不会继续读取或宣称鼠标在线。

## 最小设备读值链

新增维护工具 `tools/audit-mouse-read-capabilities.cjs` 静态解析当前 182 MW 的 manifest 范围模块、继承、factory、调用参数与解码器，输出 `mouse-read-capabilities-current-evidence.json` 和 `assets/data/device-read-capabilities.json`。当前能力范围为 **一个已审计产品 182、五项查询、38 个 AST 收据**；这不是全鼠标能力完成。未执行厂商模块、bootstrap、task runner 或 DLL。

共享实现按逻辑产品的 source capability 选命令，再以实际物理 PID、容器、HID 路径与实例选传输。182 当前 `main.7d7bac778fbfcdb02c10.js` SHA-256 为 `6350402b2a4743363838628b6e74a42131517e0859908ea8ce04fbb666428cab`；业务块 `6259.98d162c59be4fff20fa2.js` SHA-256 为 `58569c0ec63da4acafb2bc561ee63bcb865ada345ce0cbb95e1143661672698e`。鼠标继承链为 `rzDevice25DualLinkMouse` → `rzDevice25` → `rzDevice`，有线 factory 使用同一 `rzDevice25` 基类读方法。HID 传输作为 worker 的共享同级模块供接收器与鼠标读值使用，没有产品专用 worker。

| 设备字段 | 原方法与参数 | 命令头 `[长度,class,id]` | 解码边界 |
| --- | --- | --- | --- |
| 固件 | `getExtendedFirmwareVersion()` | `[4,0,135]` | 四字节版本；不伪造其他 IC 数据 |
| 电量 | `getBatteryLevel(0)` | `[2,7,128]` | 电池 ID 必须回显 0；`floor(raw/255*100)` |
| 充电状态 | `getChargingStatus(0)` | `[2,7,132]` | 回显 0；只接受当前充电枚举 |
| 回报率 | `getUSBHighSpeedPollingRate(1)` | `[2,0,192]` | 回显 profile 1；只接受当前高回报率枚举 |
| 即时 DPI | `getDpiLevel(0)` | `[7,4,133]` | 回显 class 0；X/Y 大端解码，保留包含 0 的原始 u16 值 |

182 bootstrap 明确启用 highSpeedPollingRate；调用层默认 profile 1，并不等于读取本地选中的任意云配置。DPI 方法在原 task runner 的 read-before-write 路径中存在；这里只取独立 getter，不调用含 `setDpiLevel`、缓存写入的完整任务。固件、电量和即时 DPI 是设备会话观测，不用于伪装完整 profile 已同步；已有本地草稿和设备保存流程保持不同职责。

最终回读修正了把 UI DPI 输入范围用于拒绝设备读值的问题：87969 的 `getDpiLevel`、其 helper 和结果 parser 都没有 min/max 校验；85191 的 caller 直接比较读取到的 X/Y 与目标值。现按原 API 接受 X/Y 的全部 u16 值，包括 Y=0；这不推断 0 是何种设备模式或默认值。UI 编辑上下限仍仅作为源证据保留，已从运行时读能力中移除。

传输使用 ReportID 0、91 字节 Feature；90 字节 packet 的第 2–87 字节 XOR 写到第 88 字节。原基类 constructor 令 `transactionId=0`；getter 先在值为 31 时复位，再返回后递增，因此外部序列为 `0,1,...,30,0,1,...`，与本地 modulo 计数一致。鼠标前缀为 0；179 接收器自身查询的 224 前缀不能套用到被中继的鼠标。两个会设置另一个事务标志的原命令均不属于这五项读能力。维护工具核验 checksum、字段偏移、事务、解析器、调用参数与引用的原收据，源证据变化时失败并要求重新审计。响应额外校验长度、事务、命令、状态和请求 ID 回显；未知充电/回报率值保持错误，不使用源中的回报率默认 8000 来冒充实际读取值。

无线中继读取前后都重新查询同一物理接收器，要求真实 peer PID 对应本逻辑产品且状态为 1；中间 Feature 使用源 factory 的鼠标事务命名空间与实际接收器 claimInterface。打开前后及完成后复核设备实例。各阶段有观察预算，父进程给合并读请求 35 秒上限，超时停止自有 worker。锁只协调本应用的 worker，不能保证排除其他厂商进程；无法验证或响应冲突继续报告错误。

设备发现与读值分为两阶段：`discover` 返回接口/peer 快照后即发布；shell 完成连接更新与 workspace/profile/connection scope 捕获后发送 acknowledgement，后台确认后才开始独立 `read_device_values`，填充可得字段和逐字段错误；慢查询不会延后首次显示鼠标。没有收到确认时明确报告参数查询未开始。第二次更新使用之前捕获的 scope 消费 polling，不能在完成后重取 scope 套给新配置。只读数值是会话态，不写入本地设备库冒充下次实际读取。

本批新增仅编译的协议案例覆盖固件、百分比、回报率、DPI 解码及 0/65535 原始值保留，以及未知枚举、错误请求 ID、截断、过期事务和命令不匹配；没有运行这些测试。backend / shell 改动已格式化，维护工具生成与 `--check`、差异空白检查由静态方式验证，Cargo 由主任务统一执行。仍没有本项目当前机器上的真实发现/查询结果，因此不能宣称鼠标已经读取成功或问题已获硬件验证。
