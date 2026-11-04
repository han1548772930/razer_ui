# 系统：性能模式、风扇与处理器设置

Performance 改变当前配置档在接电和电池两种供电状态下的性能设置。模式、风扇控制方式、手动转速、CPU/GPU 档位和超频配置是不同字段；温度与当前转速由另一路监控消息提供。当前 41 个系统产品均挂载此页，逐产品入口见 [系统产品索引](README.md)。

## 模式和数值分支

下表直接来自各产品 UI 的 `SupportedPerformanceModes`。转速单位为 RPM，括号为该模式在静态定义中的默认手动转速；页面收到 `fanValues` 后，按模式的 `id - 1` 替换最小、最大与默认值，因此表中数值不是对连接设备实测范围的替代。B=Balanced，G=Gaming，C=Creator，U=Ultrabook Low Power，X=Custom，S=Silent，T=Turbo，P=Performance／Maximum Power，H=Hyperboost，E=Battery Saver。模式名称和是否显示在电池页由原件的模式枚举、标题与 `isShowOnBattery` 控制。

| 产品 PID | 静态模式与手动转速范围 |
| --- | --- |
| 563、576 | B 3500–5700（4500）；G 4500–5700（5000） |
| 564、588 | B 3700–5300（4500）；X 未定义手动转速范围 |
| 569 | B、C、U 2200–5500（3800） |
| 570 | B 3700–5300（4500）；C、G 4000–5300（4600） |
| 571 | B 3700–5300（4500）；G 4000–5300（4600） |
| 581、587、589、595、621 | B 3700–5300（4500）；X 4000–5300（4600） |
| 582 | B 3500–5500（4200）；C、G 3900–5500（4500） |
| 586、594 | B、E、G 2500–5500（4000） |
| 597 | B 4300–5500（4900）；C、G 4900–5500（5200） |
| 598、622、633 | B、X 2300–4300（2900） |
| 601 | B、E、G 2500–5600（4000） |
| 616 | B、C、G 3600–5200（4300） |
| 618 | B、E、P 2700–5500（4000） |
| 623 | B、C、G 4200–5400（4800） |
| 624 | B、X 3300–5400（4300） |
| 630 | B 3400–5200（4100）；X 4000–5300（4600） |
| 634 | B 4200–5400（4800）；C、G 4800–5400（5100） |
| 650、670 | B、S 3400–5200（4100）；X 4000–5300（4600） |
| 651 | B、S 2200–3800（3000）；X 4000–5300（4600） |
| 652 | B、S、X 3300–5400（4300） |
| 669 | B 2000–5400（3800）；S 2200–3800（3000）；X 2000–5400（4300） |
| 671 | B、S、H 3400–5200（4100）；X 4000–5300（4600） |
| 672 | B 2200–3800（3000）；S 3700–5300（4500）；X 1700–4300（3000） |
| 694 | B、T 2000–5400（3800）；S 2200–3800（3000）；X 3300–5400（4300） |
| 695 | B、T、X、H 3300–5400（4300）；S 2200–3800（3000） |
| 696 | B、S 3700–5300（4500）；T 3300–5400（4300）；X 4000–5300（4600） |
| 709 | B、P、X、E 3300–5400（4300）；S 2200–3800（3000） |
| 710、736 | B、P、X、E、H 3300–5400（4300）；S 2200–3800（3000） |
| 711、737 | B、P、T、X、E 3300–5400（4300）；S 2200–3800（3000） |

早期页面把刷新率、GPU 模式或分辨率卡片放在 Performance 内；650–652 的此类控件也不能因为没有独立 Display 标签而忽略。独立显示页及其实际挂载分支见 [显示与电池](display-power.md)。

| 实际挂载分支 | 产品 PID 与条件 |
| --- | --- |
| CPU/GPU 切换按钮式 Custom | 587、588、589；622 另传 `AVAILABLE_CUSTOM_MODE` 数值范围 |
| CPU Boost 式 Custom | 595、621、624、630、633、650、651、652、669、670、671、672、694、695、696、709、710、711、736、737 |
| 独立 CPU Boost 启停控件 | 652 的挂载显式传 `supportCpuBoostEnabled=true` |
| 最大风扇选项 | 669、670、671、672、694、695、696；709、710、711、736、737 的挂载显式关闭 |
| CPU 电压优化器 | 669、694、709、710；与完整 Intel 超频编辑器不同 |
| CPU 超频编辑器 | 671、672、695、696、711、737；736 显式不挂载此编辑入口 |
| 主机智能风扇曲线和 GPU 超频 | 709、710、711、736、737；旧式固定风扇 UI 在这些挂载中关闭，改由新风扇卡片处理 |

Hyperboost 的实际显示还要求接电、适配固件、支持的系统 SKU，以及风扇配件列表至少有一项 `fanReady=true`。硬件 Fn+P 切换使用同一模式表，先过滤电池／接电可用项；当前供电只有一个可用模式时不轮换。CPU/GPU 优化开启、低功率适配器以及电池状态会进一步限制模式选择。

## 编辑、持久化与设备任务

UI reducer 先修改所选 `performance.plugged` 或 `performance.battery` 分支，再发送 `ON_SET_ACTIVE_PERFORMANCE_MODE`；其 payload 包含 `isBatteryMode`、`performanceMode` 和该模式完整的 `performanceSettings`。固定转速使用 `ON_SET_FAN_SPEED_VALUE`。曲线编辑和恢复默认曲线同样发送完整模式设置；温标与 RPM／百分比显示偏好使用独立 `ON_SET_FAN_CURVE_SETTING`，不等于修改设备控制模式。

PID737 中间件 `Cd` 读取实际选中的本地配置档，在需要保存时更新模式、active、曲线和版本，停止旧固定／曲线追赶计时器，写入本地存储并广播页面状态。它随后广播 maker 的 `completed`，再按当前真实供电状态决定是否排队执行设备任务。编辑另一种供电分支只保存配置；该分支在供电切换后恢复。任务完成回调 `tt` 是独立的设备结算通道，不是前述本地完成消息。

runner `et` 使用初始化实际读取的 `compactDeviceData.fanIdList`，分别为每个 thermalId 构建指令。普通模式调用 `setThermalFanMode`，手动模式另调用 `setThermalFanSpeed`，然后调用 `setOverClockingMode`。Custom 模式还有 CPU `setAdvFanMode`、GPU `setAdvFanModeWattage`、功率限制及优化器分支。低功率适配器会强制使用对应 Balanced AC thermal mode；部分 Creator 机型转换为 Gaming mode；支持 Balanced DC 的分支使用其专用编码。

协议转换器 `50779` 将 RPM 除以 100，默认 class/profile 为 1；auto 编码为 0，manual 和 smartFanCurve 均为 1。`rzDevice25` 将 fan speed 编为 `[classId, thermalId, speed]`，fan mode 编为 `[profileId, thermalId, fanMode, fanModeValue]`；fan mode 发送后等待 200 ms。应答解析仅在 `SUCCESS` 且返回长度大于 0 时解释字段。上述 runner 没有统一的写后 GET；固定和曲线模式的当前转速另由监控链读取。

## 智能风扇曲线和监控

709、710、711、736、737 的曲线分别保存在接电／电池、性能模式、CPU／GPU 温度分支。新卡片提供 Auto、Fixed、Smart 三种控制方式；固定转速步长为 100 RPM。Smart 仅在接电显示；Battery Saver 等受限模式会由 effect 将不允许的控制方式切换回 Auto。Custom 的可用性还取决于 `enableAllFanCurveModes`。

CPU 与 GPU 曲线点同时含 `temperature`、`cpuFanSpeedValue`、`gpuFanSpeedValue`。709、710、736 的页面传入 200 RPM CPU/GPU 偏移；711、737 传入 0。页面有摄氏／华氏、RPM／百分比切换、CPU／GPU 温度源、同时使用两种温度、点编辑和恢复当前温度分支默认值。恢复不重置另一条温度曲线。实际图形默认隐藏 GPU 线，但曲线数据仍保留两种风扇转速。

主机监控回调接收 CPU、GPU、RAM、存储数据；Auto 读取两个风扇的当前转速，Fixed 显示追赶计时器更新后的转速。Smart 要求所选温度传感器状态为 `enable`；同时使用 CPU/GPU 时两者均须有效，分别插值后选择 CPU 风扇目标较高的一组设置。插值先验证所有点、按温度排序；低于第一点时从产品规定最低温度和设备最小转速插值，高于最后一点保持最后一点，中间线性插值并取整。PID737 的配置最低温度为 CPU 40 °C、GPU 30 °C，最高编辑温度为 CPU 100 °C、GPU 90 °C。

Smart 的目标通过 `setThermalFanSpeed(1, CPU/GPU, floor(RPM/100))` 持续下发；只在 setter 返回 `SUCCESS` 时更新对应 `lastSet`。固定和曲线追赶分别使用具名 `navigator.locks` 和 1 秒 interval；实际转速通过 `getThermalFanCurrentSpeed` 乘以 100 转成 RPM。固定目标差值小于 100 RPM 后停止；曲线追赶的停止判据实际只比较 CPU 风扇差值，再把两种目标向下取到 100 RPM。原件 getter 失败时返回的 3000 RPM 是其回退代码行为，并非实测结果。

## CPU／GPU 编辑与 Apply

超频配置档独立于设备配置档，以自己的 GUID 选择。PID737 的 CPU UI 对 core voltage offset 使用 DeviceInfo 的 −0.195…0.195 覆盖通用 −0.5…0.5，步长 0.005；短时功率 1…200 W、步长 0.125；持续功率 1…190 W、步长 0.125；时间窗 1…128 s，并提供源码枚举的离散选项。单独电压优化器使用 −30…−1、步长 1。电池、BIOS 要求和 OC Lock 位分别控制启用状态。

PID737 的 GPU 编辑器保留独立草稿：GPU offset −1000…400，VRAM offset −1000…3000，步长 1；Cancel 恢复 Redux 中的当前设置，Apply 才提交 `Qt(draft)`；单项 Reset 把该项置 0 并提交。电池时动作禁用。736 另禁止 CPU、GPU 同时选择最高档；711／737 允许各自的 Max 选项，并核对 Core Voltage Lock。

编辑 CPU 参数先更新超频草稿与 saved/canApply。当前 PID737 的 Apply 在 50 ms 定时回调中先将草稿投影到 `overclockingActualSetting`、提交 Apply 并关闭可应用状态；尚未保存时另发保存动作。保存 maker `vp` 只保存配置；应用 maker `hp` 向超频 runner `ji` 排队。CPU Cancel 调用 Reset Active maker `mp`，读取所选已保存配置，由 `Ip` 发布 `GaL` 恢复编辑内容，随后重新排队真实 `ON_APPLY_OVERCLOCKING_PROFILE`。因此此按钮也可能写设备，不能按纯草稿取消实现。

当前 CPU 链的正式证据归入 [系统中间件收据](../../../.analysis/results/frontend/system-mw-audit.json) 的 `performance_native_chain`，复核入口为 `audit-family-function --family system --function mw --product 737 --query system-737-performance-native-chain`。实际挂载为导航 → `Ji` → `As`；只在 CPU 优化开启、非电池且非低功率时显示，另传 `includeCoreVoltageLock=true`。

参数投影使用控制 ID 34（电压偏移）、48（持续功率）、47（短时功率）、49（短时功率开关）、66（时间窗）。bulk 先删除与缓存相等的项；本型号明确启用 `splitOCCoreVoltageOffset`，因此 bulk 删除 34，runner 等待 1 秒后独立提交它。bulk 非空时按“批量请求 → GetAll 请求”发送；拆分电压的重试由具名锁串行化，拒绝旧值，最多 5 次、每次等待 `2000 × 次数` ms。回调另有 1 秒重试定时器，当前 stop 仅注销容器事件，未发现此处取消该定时器或等待全部回调退出。

实际原生分流是 `RzIntelOverClock` 1.2.23.0 → 本地管道请求 → `RzIntelOverClockDLL` 1.2.23.0 → 混合 CLR 方法。两层原生完整函数、CLR vtable 到 MethodDef `4B/50/51` 的原字节关联及 11 个完整 IL/C# 方法已复核。bulk 将 34 从 V 转为 mV、49 转为布尔状态，核对已有值与范围后调用 `Tune`；拆分电压构造 `164…171` 与 `3500…3515` 的提案，调用 `ProposeChange`、`ApplyChanges`。完整方法中的 103 条 C# 未解析诊断保留，未将原字节/IL 核验当成 SDK 底层语义完成。

请求导出的 bool 仅反映发送路径，runner 不等待异步控制结果，而且包装层捕获并记录异常、未检查 bool。`GetAllTuningControlData` 与 tuning-changed 事件合并返回字段；UpdateAll／UpdateCore 只合并每项 `error==""` 的结果，再发布页面状态和保存缓存。bulk 在 `Tune` 前构造每项结果，顶层 Tune 错误未成为 JS 合并门禁，部分失败不能提升为整次写入成功。`completed`、乐观投影、缓存保存与真实控制确认必须分别记录。

当前服务增量归入同一[系统中间件收据](../../../.analysis/results/frontend/system-mw-audit.json)的 `overclock_service_chain`，复核入口为 `audit-family-function --family system --function mw --product 737 --query system-737-overclock-service-chain`。它精确引用前述 Apply 收据及[当前服务版本证据](../../../.analysis/results/frontend/accessories-main-audit.json)的 `hanbo_system_monitor_versions`，复用已核验的 12 个超频原生函数、11 个托管方法和 6 个服务函数；新增 27 个原生函数、2 个完整托管注册方法和当前 Host 回调所有者。原生正文继续保存在已有捕获中，收据只保留可复核引用。

当前稳定版声明对应 `RzDLLService` 1.0.30.0，不是旧 1.0.29.0，也不代表已观测本机正在加载它。shim 将完整请求 JSON 按实际字节长度写入固定命名管道，指定 `dllName=RzIntelOverClockDLL` 与 `handleOnOneThread=true`。服务不就绪或写失败时，请求进入同一 mutex 保护的 pending 队列并返回 false；连接后的重发在首次失败时停止，随后仍清空整个队列，包括失败项和未尝试项。发送成功返回 true 同样不等待超频结果。

服务完整读取管道消息，按首条请求绑定当前 DLL 名称与 IPC 对象并选择 worker；FIFO 任务通过实际 PE 虚槽 `CRzTask+8 → RVA 0x47fa0` 执行。分发器按 DLL 名缓存模块，首次加载会调用路径／签名门禁 helper、处理 `relateDllPath`、加载模块并调用可选 `PreInitialize`／`Initialize`；这些门禁 helper 的内部算法仍未展开，初始化失败也不能推断模块已经卸载。目标函数接收 JSON 指针并返回 C 字符串，服务先复制到 NUL，再调用可选 `FreeMalloc`。已覆盖分发错误构造 FAIL/error 回复，但无效 DLL 名或缺少当前 IPC 等路径不保证回复。

普通结果和 SDK 回调均将正文与五字节 `<EOF>` 分两次发送，第二次发送结果不构成完成门禁。服务最多尝试 50 次写入；只有完整块推进偏移，部分写入重试原位置，109／232 标记 IPC 失效，233 在同一预算内重试。shim 的同步读取上限是 500 次循环，不是时间超时；原件仅在新增片段中搜索 EOF，并另有特殊处理，不能据此保证任意跨片段标记或多帧支持。整段正文与 EOF 未观察到统一写锁。

两个新增托管方法核实了服务回调指针的保存，以及 SDK 的 Propose／Apply 事件注册；保留原始 MethodDef、RVA、IL、异常区间和全部 8 条 C# 诊断。结果由 receiver 解析并加上当前注册容器事件，再由 Host 发给当前有效 channel／URL 消费者，接回前述 `D` 与 merger。服务按当前 DLL 名找 IPC，shim 按当前容器转发，未发现请求 ID 或连接代次；不能把迟到回复、Apply 状态、GetAll 或逐项无 error 的结果合并为统一硬件 ACK。

停止仍只注销当前容器事件；原生 `DeviceTerminate` 关闭 receiver 线程与管道句柄但不 join，`Terminate` 单独释放读取 buffer。服务全局退出的实际顺序是模块 Terminate／有条件 FreeLibrary 在先，拒收新任务和停止 worker 在后；Terminate 返回 false 时保留模块。worker 超时路径可能强制终止线程，剩余排队任务回池。上述入口均不证明 SDK 或在途回调已经排空。

本段确认 PID737 Apply 的请求、当前服务分发／回复和既有结果消费者静态边界。SDK 客户端生命周期及当前 XTU ServiceInfo 服务候选的静态语义接续于下文；IPF／驱动、其他 thermal／GPU／监控分支及另外 40 款系统产品仍分别待闭合。独立 Rust、UI／后端接线和官方 DLL／真实设备对照仍未验证。

PID737 SDK 生命周期归入同一[系统中间件收据](../../../.analysis/results/frontend/system-mw-audit.json)的 `overclock_sdk_lifecycle_chain`，复核入口为 `audit-family-function --family system --function mw --product 737 --query system-737-overclock-sdk-lifecycle-chain`。它从前述服务收据接续，复用现有完整捕获，核验 12 个 mixed CLR 方法、44 个 SDK 方法、2 个原始 CLR fixup 与 4 个原生函数，保留完整 IL、实际 MethodDef、代码字节、异常区间及 C# 诊断边界。当前 manifest 选 `RazerIntelOverClock` 1.0.4.0，包内 SDK 与 XtuCommon 实际 PE 版本均为 10.0.1.35；本项没有观察本机安装或程序集绑定结果，不能改用另外两套产品包。

`InitOverClockSDK` 先准备 GC 句柄容器；初始化标记已经为 true 时直接返回。正常路径检查 `XtuService`，创建 `IntelOverclockingLibrary`，调用 `Initialize` 并丢弃 bool，再用 `InitializeCheck` 判断。后者在本包实际再次调用 `Initialize`。SDK wrapper 持有五个进程共享 singleton 的静态接口，依次短路初始化 ServiceInfo → Monitoring → Tuning → Profile → AppProfile；GC 句柄只拥有 wrapper 引用，不能视为拥有这些 singleton 的完整生命周期。服务检查实际使用 `OpenSCManager`／`OpenService`／`QueryServiceStatusEx`，停止或正在停止时还会调用 `StartService` 并轮询；它不是纯读取操作。

ServiceInfo 连接本地 ServiceInfo 管道并核对兼容性；Monitoring 读取监控元数据；Tuning 读取控制元数据、连接 Tuning 管道、丢弃当前客户端 proposal、重扫 controls、启动事件库并订阅四个事件，特定条件下还清除 `KeepVoltageOverridesAdaptive`。Profile 创建本地读取／存储 helper，AppProfile 删除基线档、读取本地配对并创建 hotkey hook。这些是本包真实初始化副作用；未展开的元数据、存储和 hook 内部逻辑分别保留为依赖。失败返回 false 时已经成功的 singleton 和副作用不会统一回滚，下次会重试未初始化的 owner；Tuning 对事件库初始化的 bool 未作门禁便继续设置自身标记。

共享 `ConnectionHelper.ConnectToServiceEndpoint<T>` 最多尝试 550 次，每次设置 55 秒的 `OperationTimeout`；connect 返回 false 或抛出其 catch 内的异常后等待 100 ms，包括最后一次。55 秒不是整个循环的总期限。创建 channel 与设置 timeout 在该 catch 外；失败 channel／factory 没有在此 helper 中 Close／Abort／Dispose。ServiceInfo、Monitoring、Tuning 和 Event 分别连接 `net.pipe://localhost/ExtremeTuningUtility/` 下的同名本地服务端点；ServiceInfo 的实际服务生产者与兼容性消费者见下文，其余服务与驱动依赖继续独立核查。

两个 delegate helper 按需缓存静态原生回调。SDK 的事件 add／remove 使用 `Delegate.Combine`／`Delegate.Remove` 与 `Interlocked.CompareExchange` 循环，只改变订阅列表，不等待已经取得 delegate 的调用。Event singleton 自己持有后台 STA 轮询线程和事件 proxy；Reset 仅清初始化标记并设置停止标记，没有 Join 或关闭管道，内层 `GetNextEvent` 排空循环也不检查停止标记。再次初始化会清停止标记并添加恢复事件订阅；Reset 没有对应移除该订阅。

原生 Terminate 先按标记释放程序集 resolver，失败便返回；随后 `ReleaseOVerClockSDK` 按顺序移除 Propose、清 helper、移除 Apply、清 helper、清注册标记、销毁并置空 GC 句柄容器。原始 IL 在 RVA `0x40668` 的 `IL_0092`／`IL_0093` 明确把 `gIntelOverClockInit` 写成 **true**，不能按预期改写为 false。GC 句柄析构只释放 `GCHandle` 和原生容器；此链没有调用 SDK 的 Reset／Dispose，也没有等待在途回调。SDK 的独立 Reset API 确实存在，分别清缓存、标记并传播 Reset；其存在不能证明 Terminate 实际执行了它。本层静态结论是上述已发生与未发生的调用，完整服务／驱动语义、Rust 迁移、UI／后端接线及实际 DLL／设备对照继续保持未完成或未验证。

### PID737 XTU 服务信息与兼容性

PID737 XTU ServiceInfo 归入同一[系统中间件收据](../../../.analysis/results/frontend/system-mw-audit.json)的 `overclock_service_info_chain`，复核入口为 `audit-family-function --family system --function mw --product 737 --query system-737-xtu-service-info-chain`。本项精确接续 SDK 生命周期及其服务信息未决项，复用正式完整捕获，核验服务、CoreServer、ServiceInformation、PlatformInformation、SDK 与管道 binding 共 63 个完整 IL 方法；接口声明单独保留，未计作方法捕获。IL、MethodDef、原字节与异常区间共同核验，完整程序集的未解析 C# 诊断继续保留。

当前 manifest 声明 `RazerXTU` 1.0.8.0；该资源同时包含 `RazerXTU_v0.1.0.7.msi` 与 `RazerXTU_v0.1.0.8.msi`。本项沿后者的 MSI／CAB 正式提取血缘核对实际 PE 版本 10.0.1.35，其 SDK／XtuCommon 与上文客户端包原字节相同；前者仍为 10.0.0.44 候选。实际安装器／OS 选择、程序集绑定与本机加载版本均未观察，不能把当前分析候选视为已安装事实。

`Program.Main → ServiceBase.Run → XtuService1.OnStart` 启动不保留句柄的前景线程，依次执行 `CoreServer.Initialize → Start`，并订阅 `SessionEnded`。CoreServer 经实际 Unity 初始化绑定 `ServiceInformation`／`PlatformInformation` singleton，再创建单实例 `XtuServiceInfoImpl`。Start 先用自定义本地管道 binding 打开 `ServiceInfo`，加入 Builtin Administrators SID，接收超时为 `TimeSpan.MaxValue`；只有已保存兼容性值为 0 或 1 才继续打开 Tuning／Monitoring／Event。binding 通过反射设置 WCF listener 的 `AllowedUsers`，这些所选方法没有捕获反射或 Open 异常。

`IXtuServiceInfo.Connect()` 只返回 true，不检查注入对象、IPF、硬件或兼容性。`LookupItem` 优先查动态字典并当场执行委托，其次查静态字典，未知键返回空字符串；构造器保留原字典引用，没有复制。注入为空仍能 Connect，但字典保持 null，lookup 没有空值保护或异常捕获。`SystemCompatibilityFlags` 在 `ServiceInformation.ObtainCPUInfo` 第一项写入，来自 PlatformInformation 已保存的 ulong 转十进制字符串；它不是动态查询。关键错误提前返回时该键仍存在，其余 CPU／服务信息不因此保证完整。

兼容性生产链为 `PlatformInformation.Initialize → PopulatePlatformStatus → GetSystemCompatibilityFlags`。它消费实际 IPF 状态、mailbox completion／读取结果、IA Overclocking／Undervolt Protection 状态、Windows build 与处理器 WMI：IPF 无效版本／错误／未运行／需重启分别映射 512／1024／2048／16384，硬件锁定为 8192，driver 访问异常为 unchecked 1，非 Intel 为 256。微码、Windows 补丁、UV 与 VBS 继续按原件位标志合并；VBS 仅在非 all-clear 且非 driver-failure 的分支加入，不能改写为无条件检查。Windows 可修补性要求 build ≥ 22000 且 InstallationType 不含 server，解析与 WMI 失败遵循所选原件 catch。底层 IPF 角色激活／重启、provider、mailbox／驱动传输与真实硬件响应仍为独立依赖。

SDK `DoInitialize` 在连接成功后先保存 channel、设置 `connectionAvailable=true`，随后读取兼容性；只有恰好 0 才置初始化成功，其他值获取问题列表并抛异常，外层 Initialize 捕获后返回 false。此失败保留 channel 与 available 标记。`TryLookupItem` 对空 channel、空返回及捕获到的 lookup 异常返回 false；兼容性消费者据此返回 unchecked 1，`Convert.ToUInt64` 则位于该 catch 之外，坏文本或溢出可继续抛出。未连接时枚举 `ServiceController`，XtuService 正在运行返回 2048，否则返回 4096；枚举／状态异常未在此捕获，controller 也未在此 Dispose。

SDK ServiceInfo Reset 只清初始化、视图缓存和 available，不清 channel，也不 Close／Abort／Dispose；前述 wrapper Terminate 没有调用 Reset。服务普通 OnStop 依次调用 CoreServer Stop、Dispose、服务 Dispose，SessionEnded 分支才先移除自身订阅。CoreServer Stop 先停正在运行的监控，依次 Abort 四个 host，再通知／释放 notifier；Dispose(true) 依次释放 host、监控接口和 notifier。锁的 finally 只负责 `Monitor.Exit`，异常仍可中断后续释放；没有启动线程 Join／cancel，也没有普通 OnStop 取消 SessionEnded 订阅或统一在途排空。host 字段未置空，后续 Start 不会重建已 Abort 的非空 ServiceInfo host。

本项完成所选 10.0.1.35 服务信息与兼容性链的静态语义；Monitoring／Tuning／Events、SDK 存储／元数据／hook、安装选择和 IPF／驱动继续保持各自边界。现有 [Windows 服务适配](../../../crates/razer-platform/src/platform/windows/services.rs) 仅有 `XTU3SERVICE` 的 SCM 控制，不能视为该 WCF 兼容性链已经由 Rust 实现。本批未修改 Rust、未新增提取／会话／临时输出、未执行官方代码；独立实现、UI／后端接线、真实 DLL／服务／设备对照与运行验收仍未验证。

## 打开、切换、失败与停止

初始化读取真实 thermal fan ID 列表；本地配置恢复先发 `MW_SET_PERFORMANCE` 类状态，再按真实供电分支排队恢复。UI 与中间件的默认构造器并不完全相同：例如 PID737 UI 初始模式为 Balanced，中间件默认接电模式为 Maximum Power；不能从其中一个常量推断新连接的实测模式。电池优化恢复还具有系列及 `getBatteryOptimizerFromFW` 门控，见 [显示与电池](display-power.md)。

maker 保存和 runner 传输均有独立异常通道。runner 在设备调用后检查 abort，失败时 `result=false` 并拒绝；成功后才更新任务内存缓存，回调再发布 device completed/error 并结算事件。原件将队列 `skipped` 作为完成消费；页面中的乐观编辑不会因此成为额外 GET。

模式切换明确停止两个转速追赶 interval。系统监控有 start／stop 操作，但在接电且智能曲线运行时，页面关闭与停止显示监控不是同一个生命周期事件，后台仍需要温度控制回调。本页仅记录已读到的停止入口；不把泛型队列 `cancelAllTasks` API 的存在解释为每个退出分支都调用它。

## 实际页面与当前 Rust 对照

PID587 的导航把 Performance 指向 `rS → ES`。`ES` 先用真实 `fanValues` 覆盖模式表，再创建 `.blade-performance → n_ → i_(left/right)`：左列只有一个 `xd` 性能卡片，右列依次挂 `tS → eS` 显示设置和 `sS → iS` GPU 模式。它没有单独的风扇卡片；Balanced 的固定风扇控制嵌在左侧性能卡片内，切到 Custom 时由同一卡片内的 CPU／GPU 控件取代。PID587 的 `isCpuGpuToggle:true` 选择按钮式分支；不能根据同一个共享组件里还存在 slider、CPU Boost 或超频编辑器，就把那些分支计为该产品实际页面。

`xd` 持有独立的供电编辑页签、模式、风扇方式和手动值，并持续消费 Redux 更新。实际 `isOnBatteryPower` 改变会同步页签；低功率适配器、CPU／GPU 优化、固件、Hyperboost 配件就绪和 Battery Saver 能力分别参与模式显示或禁用。固定风扇区还受 `!W && supportFixedFanSpeed`、非 Custom／Silent／Hyperboost 以及 `canUpdateFanMode` 限制。切换编辑页签不能作为已经读取真实供电或切换实际电源的证明。

| 页面或动作 | 当前官方行为 | 当前 Rust 对照 |
| --- | --- | --- |
| 两列与卡片顺序 | PID587 左为完整性能卡，右为内屏刷新／外部显示卡、GPU 模式卡 | [system/mod.rs](../../../crates/razer-pages/src/features/system/mod.rs) 的 `performance` 把风扇另放右侧，Custom 另建左侧卡片；`display` 只组合刷新按钮和 OS 链接，没有这里的完整外部显示／GPU 卡片。实际树与官方不同 |
| 卡片与模式外观 | `.body-widgets` 行换行、最大宽 1240px；列宽 600px；卡片宽 600px、圆角 5px、上下外边距 10px、内边距 30px 40px；模式格 250×90px，奇数格右边距 20px，另有 hover／active 边框 | Rust 使用共享 `surface::panel`、纵向模式按钮和 note；本项尚未证明布局、图标、字体、状态颜色、缩放、换行与 tooltip 位置一致，不能由模式表相等提升为页面完成 |
| 真实读取与初值 | 模式、供电、`fanValues`、可更新性和显示能力分别由状态 producer 提供；页面种子不是设备实测 | `new` 从 `spec.initial` 建立 `draft`，供电页签固定为 `plugged`；`restore` 再合并本地 profile／device 配置。该 owner 没有对应的真实 thermal／供电／显示观测入口，也没有把未观测、错误与已确认值分别建模 |
| 模式门禁与编辑 | 模式过滤之外，还核对优化器、低功率、固件和配件就绪；按供电分支发送完整性能设置 | Rust `performance` 主要按模式静态标志过滤，并以草稿中存在对应对象决定 enabled；`choice` 修改 JSON 草稿，不能证明真实门禁、初始化或设备写入已对应 |
| 固定风扇与推荐值 | PID587 Balanced 接电分支内置 Auto／Manual、说明、风扇图标、100 RPM slider 和可点击推荐值；范围由实际 `fanValues` 替换 | Rust 独立右侧卡片提供按钮和 slider，缺上述完整结构与推荐值动作；旧产品范围来自静态表，`modern_fans` 另硬编码 AC 1000…5100／DC 1000…3000，未消费真实 fan 范围 |
| 显示能力与 GPU 切换 | `eS` 消费实际刷新列表和 `displayMode`，外屏独占时禁用内屏并显示提示；`iS` 按 DDS／supportedFlag 分支显示 GPU 控件，暂存 previousMode，Cancel 恢复，Restart 调用 `restartSystem(mode)` | Rust `display` 只按本地列表长度禁用按钮，并提供 `ms-settings:display`；未找到该 owner 的 GPU 草稿、重启确认／取消、DDS 或实际显示模式消费者。本项是页面与后端缺项，不应通过补静态按钮宣称闭合 |
| 智能曲线与恢复 | PID737 的实际曲线卡具有图形、温标／单位、温度源、点编辑；Reset 仅恢复当前温度分支，另一分支保留 | Rust `fan_curve` 只显示摄氏温度文本与 CPU／GPU 转速 slider；Reset 将整个 `smartFanCurve` 替换为 `spec.initial`，会同时重置另一温度分支及该对象内的选择设置，与官方恢复范围不同 |
| CPU／GPU 超频弹层 | PID737 保留独立超频配置和 GPU 草稿、Apply／Cancel／Reset，以及电池／BIOS／锁位条件；它们不是普通模式按钮 | Rust `performance` 主要挂 CPU／GPU 档位和最大风扇 toggle；未发现上述编辑器、配置消费者、Apply／Cancel 及独立设备确认链，不能由 `cpu_options`／`gpu_options` 计为超频已实现 |
| 提交、保存与确认 | 官方 maker 保存与设备 runner 分离，当前真实供电决定是否执行设备任务；随后独立发布 device completed/error | Rust `changed → SystemProductChanged → SourceProductWorkspace::capture` 只更新当前 profile 的 `source_settings` 并发 `WorkspaceEvent::Changed`。Shell 此分支只同步映射／设备上下文和通知，没有系统性能提交；现有 `ProductSave` 也没有该性能操作的后续设备任务。即使该快照随后写入本地文件，也不构成 thermal／CPU／GPU 设置成功 |

上述生产链核对覆盖 [source_workspace.rs](../../../crates/razer-pages/src/features/workspace/source_workspace.rs) 的 system 构建／订阅、`capture` 和 restore 分支，以及 [shell.rs](../../../crates/razer-shell/src/shell.rs) 的 `WorkspaceEvent::Changed` 消费者、[persistence.rs](../../../crates/razer-shell/src/shell/persistence.rs) 的保存后续操作。此处没有把配件散热器的 SystemMonitor／Hanbo 服务当成笔记本性能页的设备后端。

后续验证必须分别覆盖 PID587 的 Balanced／Custom、AC／DC、真实 fanValues、外屏独占、DDS／非 DDS 与重启 Cancel，以及 PID737 的两条不同曲线、仅当前温度 Reset、草稿 Cancel 和 Apply 失败。模拟读取值须与种子和本地配置不同；实际设备／服务对照需按官方完成契约核对结果与清理，不能擅自添加统一 GET 或以保存完成替代设备完成。本批只完成这些分支的静态页面与生产调用方核对，未运行窗口或官方 DLL，也没有把两个产品扩展为 41 款页面全部通过。

## 原件和复核位置

所有区间均为原始 UTF-8 半开字节区间；读取时与各自 `acquisition.json` 的长度、SHA-256 核对。41 个产品的模式表、挂载与原始路径分别见 [系统索引](README.md) 和逐产品页的导航证据。

- PID587 [获取收据](../../../.analysis/inputs/frontend/system-587-current/acquisition.json) 与 `raw/apps.razer.com/synapse/products/587/ui/static/js/main.4b36ee81.js`：导航 `_U` `[7955492,7955756)`、实际根 `ES` `[7798243,7798947)`、连接 `rS` `[7798953,7799040)`、行 `a_` `[7528104,7528271)`、列 `o_` `[7528283,7528451)`、性能 `xd` `[7770911,7788092)`、显示 `eS` `[7788302,7790517)`／`tS` `[7790523,7790964)`、GPU `iS` `[7793016,7798061)`／`sS` `[7798067,7798242)`。完整 JS 经 Tree-sitter 解析并沿词法绑定确认上述所有者，输入前后身份未变。
- PID587 同 source set 的 `static/css/main.0b2f94f9.css`：行 `[69207,69307)`、列 `[69677,69769)`、卡片 `[69817,70046)`、窄窗列边距的完整 media 规则 `[78871,78923)`、模式格／hover／active `[367033,367750)`。CSS、HTML 和 manifest 均按获取收据核对实际字节；本项没有运行厂商 JavaScript 或像素比较。
- PID737 UI `system-737-current/raw/apps.razer.com/synapse/products/737/ui/static/js/main.35ffe4e2.js`：产品模块 78193 `[447919,473718)`；Performance reducer `[785268,794060)`。目录前缀为 `.analysis/inputs/frontend/`。
- 同产品 UI `8923.a0bbaf75.chunk.js`：性能卡片 `[81721,98128)`；CPU 编辑器 `[107125,112217)`；智能风扇卡片 `[151290,158350)`；GPU 编辑器 `[159795,161047)`、范围 `[161052,161158)`；Performance 页面 `[161162,163790)`。
- PID737 中间件 `system-mw-737-current/raw/apps.razer.com/synapse/products/737/mw/6737.85397ce157d4cca228ee.js`：`Cd` `[891235,894465)`；硬件模式切换 `bd` `[894465,898545)`；`wd` `[898618,900111)`；`et` `[1134887,1138701)`、回调 `[1138701,1139769)`；固定转速 runner `[1139769,1141249)`、回调 `[1141249,1142277)`。
- 同一中间件：CPU 优化 `[920017,921616)`；真实 fan ID getter `[1261762,1261936)`；低功率适配器 getter `[1268934,1269705)`；监控与插值模块 91058 `[2057043,2071520)`，固定停止 `[2069978,2070121)`、曲线停止 `[2071376,2071519)`。上述区间不替代本段当前 CPU Apply 收据。
- 当前 PID737 CPU Apply 使用 [MW 获取收据](../../../.analysis/inputs/frontend/system-mw-737-current-refresh-ec3db4b29c5a-2/acquisition.json) 与 [UI 获取收据](../../../.analysis/inputs/frontend/system-737-current-refresh-ec3db4b29c5a/acquisition.json)：UI `7934.25938d65.chunk.js` 的 `As` `[117566,124258)`、`Ji` 及导航；MW `6737.fd73d47a440529775da0.js` 的 maker `hp` `[1314723,1315174)`、runner `ji` `[1493964,1494650)`、回调 `Yi` `[1494650,1495678)`。完整 AST 所有者、当前 manifest、两层原生函数与托管方法的实际字节身份统一由 `performance_native_chain` 保存和校验，不平行维护副本。
- 同产品中间件 `rzDevice25.1042baf62cda9ce0c1bc.js`：thermal 编解码模块 13546 `[1260,12012)`；速度发送 generator `[2579,2956)`，模式发送 generator `[3488,3924)`。控制服务、平台桥和 native 二进制仍是不同证据层，本页不把 JavaScript 调用名当作底层 ACPI／驱动实现已经闭合。

本页记录官方源码语义及已核对的 Rust 生产调用链；没有执行厂商 JavaScript、主程序、官方 DLL 或真实设备。源码获取、逆向语义、Rust 独立实现、UI／后端接线、静态／模拟验证和真实运行验收分别记录，均不能互相替代。
