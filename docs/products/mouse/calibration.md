# 鼠标：表面校准与抬起／落下跟踪

Calibration 页面包含两类实际功能：通过移动鼠标测量表面的校准流程，以及设置跟踪中止／恢复阈值的 Smart Tracking 流程。它们使用不同引擎、传感器选择值和完成事件；页面同名不代表设备命令相同。

## 当前引擎分支

| 当前分支 | 作用与适用边界 |
| --- | --- |
| Philips：70、80、83、89 | 预设鼠标垫、手动测量和 mini 校准；class=0、sensor=5，具有多状态测量、Z/Q 数据与本地鼠标垫记录 |
| Pixart：92、96、98、100、101、103、104、105、106、107、108、112、117、120、138、140、147、150、161、2594 | 预设和手动表面测量；class=0、sensor=4，结果为 configuration 参数，另有提前 End 后的 GetData 状态 |
| SmartPixart，入口显式选择：115、124、131、143、148、153、156 | Smart Tracking、预设、手动；普通／迟滞 lift mode 与单参数／双参数表 |
| SmartPixart，派生特性默认选择：99、110、113、116、122、128、132、133、134、141、145、152、154、199、208、224 | 使用同一管理器；是否暴露 Calibration 页仍由实际导航和 BLE 条件决定 |
| Orca5：165、167、178、180、182、184、194、218 | 普通跟踪与非对称阈值；218 的普通跟踪开关写入与其他七项不同 |
| Orca5Plus：158、170、175、190、192、196、203、204、214、221、222、226、229、231、235、239、45066 | 普通跟踪与非对称阈值；class=0、sensor=4，无表面移动测量状态机 |
| Pixart3349：163、185、211 | 只写新 class=1 的单项 liftOff；没有手动表面测量状态机 |

表中引擎按入口 `useFeature` 和派生特性选择核对；同时对每个当前产品的完整引擎 class 语法树逐项比较，区分打包标识重命名和实际分支差异。管理器存在不等于页面可访问，也不等于某次打开已执行设备测量；具体导航以家族目录为准。

## Smart Tracking 与非对称中止

普通模式的 `trackingDistance` 为 1～3、步长 1。非对称模式分别编辑 `liftOffDistance`（2～26）和 `landingDistance`（1～25），步长均为 1。抬起值降低到不高于落下值时，控件把落下值改为 `max(1,liftOff-1)`；落下值提高到不低于抬起值时，控件把抬起值改为 `min(landing+1,26)`。这些是源码的档位编号，不应直接标成毫米。

PID170 的 SmartTrackingEngine 先 `setProximitySensorState(0,4,true)`，然后以 `Universal_Lift_Cut_off=1` 设置 `trackingDistance-1`。非对称分支先启用传感器，再设置 `Preset_Calibration_Hysteresis=4`，最后把 `[liftOffDistance-1,landingDistance-1]` 作为 configuration 写入。失败通过 calibration-notification 发布；当前完整引擎没有匹配 lift-setting getter 的写后比较。

Orca5 的 165、167、178、180、182、184、194 在普通跟踪先写传感器 false，218 写 true；Orca5Plus 全部写 true。它们的非对称分支均写 true，检查每个前序 setter 的 SUCCESS 才推进。缺少 landingDistance／liftOffDistance 时原件只记录参数缺失，没有发成功通知。管理器的 `apply` 没有 await 内部调用，非对称分支本身也没有把 `presetTracking()` Promise 返回给顶层；不能把调用入口返回当作所有设备命令完成。

Pixart3349 使用新 class=1、sensor=4：先写传感器 true，再把 liftOff 原值放入单字节 configuration。disable 写 false 并发布 `onDisable`，Reset 硬件事件另转为校准通知。它不使用 Orca5 的两个减 1 阈值，也不把 liftOff 当作表面 GUID。

其真实 reset 硬件事件会发布校准通知。界面首次介绍弹层的显隐另存在本地偏好键，关闭介绍不会修改设备跟踪参数；介绍记录与设备校准状态是不同对象。

## Pixart：测量完成与 GetData 分支

Pixart 管理器按 control type 选择 Preset 或 Manual。Manual 为 Ready → Initiating → Starting → Calibrating → StopCalibration → Ready，另有 GetData 和 Apply。Initiating 开传感器后注册 Escape 和本次鼠标输入、启动 35 秒 timer；第一次左键进入 Starting，写 `setProximitySensorControl(0,4,Manual,Start)`，等待 Start 事件进入 Calibrating。普通阶段 timer 为 60 秒。

Calibrating 收到 End 不立即发布测量数据，只置 `doSpecialHandling=true`；下一次左键清理临时监听后直接进入 GetData，避开再次 Stop。通常左键或 timer 到期进入 StopCalibration，写 Manual/Stop 后等待 End；只有 End 触发 `getProximitySensorConfiguration`，成功才把 jsonData 发布为测量结果，失败发布 Halt。GetData 同样读 configuration 后发布 End／Halt，但没有再写 Stop。

Preset 和 Manual 的 Apply 都是传感器 true → `setProximitySensorConfiguration`；以最后 setter status 发布 `onApply`，没有 Philips 的 calibrationData／threshold／Z/Q 计算，也没有写后 GET 比较。Ready disable 独立写传感器 false。测量新建、重命名、重新校准和 profile 保存沿共享 calibration maker 处理，设备取得的 configuration 与本地鼠标垫元数据分开保存。

Escape 在活动阶段发布 Cancelled，停止 timer／临时键盘和鼠标监听；Starting、Calibrating 还写 Manual/Stop，再禁用传感器后回 Ready。Initiating／StopCalibration 依据 busy／escape 标志延后清理。manager destroy 从共享 inputevent 和 hardwareevent 移除具名回调，并调用两个子引擎 destroy；Manual destroy 只移除 Escape，不会额外替活动状态执行 Stop、清 timer 或禁用传感器。`lockMouse`／`unLockMouse` 固定返回 true，不能称为真实 OS 锁鼠标。

## SmartPixart：普通、预设与手动跟踪

SmartPixart manager 按 SmartTracking=3、Preset=1、Manual=0 选择三个子引擎。普通 Smart Tracking 的 UI 值 1、2、3 映射为 liftHeight 0、1、2，liftMode=1；非对称使用 liftMode=2，(landing,liftOff)=(1,2)、(1,3)、(2,3) 分别编码 0、1、2。该分支先将传感器设为 false，再写 `setProximitySensorLiftSetting`，随后在子引擎发布 calibration-notification。当前 manager 只为 Preset／Manual 注册通知转发，没有为 SmartTracking 注册同类转发，不能推定该通知总到达统一 UI 消费者。

Preset 普通模式为 liftMode=3，按 surfaceDistance 选择固定八字节配置；迟滞为 liftMode=4，以 liftOffDistance 选五字节 trackToLiftData，以 landingDistance 选八字节 liftToTrackData。Apply 顺序为传感器 true → liftSetting → configuration，或先 liftToTrack 表、后 trackToLift 表。原件 Preset 表写入没有逐步核对每次返回值，最终 `onApply` 使用最后返回 status；manager 只订阅 calibration-notification，没有订阅这一路 `onApply`。

Manual 普通模式为 liftMode=5、迟滞为 6；启动先设传感器 true 并写所选 liftMode／liftHeight，等待左键后写 Manual/Start。状态为 Ready → Initiating → Starting → Calibrating → StopCalibration → Ready；Calibrating 左键或 timer 后总进入 Stop，不具有 Pixart 的提前 End GetData 分支。Stop 收到 End 后读取 configuration，只取 `parm0`／`parm1` 为实测 A／B，成功后才提交测量结束动作。Manual Apply 先开传感器，写单配置或先 liftToTrack 再 trackToLift；双表中前表成功才写后表，最后发布 calibration-notification。

手动参数按实测 A、B 和档位 k 生成：r=k−1、q=B+3r、d=(A−8)/10；首字节通常为 A−r×d，第十档且该值大于 10 时限制为 10；第二字节在 k>6 时为 56，否则 48。单表输出八字节，trackToLift 为五字节，liftToTrack 为八字节，尾部阈值按 k 分支为 15／13／10／8。非对称两向表有各自固定旗标，不能互相复制。以上表格和公式是传感器参数编码，没有毫米语义。

SmartPixart 的单次 Escape／timer 清理与状态转换相联系；manager 的共享 input／hardware 监听使用匿名闭包，完整 manager 没有 destroy。共享 cleanupCalibration 直接调用 `manager.destroy()` 后才置空，因此在这类实例上不能声称 cleanup 已完成或订阅已释放。Orca5／Orca5Plus／Pixart3349 manager 同样没有 destroy；这是真实静态边界，不额外设计一条退出恢复链。

## PID70 表面测量流程

管理器拥有 Preset、Manual 和 Mini 三个引擎，按 control type 切换 activeEngine。Manual 的状态依次为 Ready → Initiating → Starting → Calibrating → StopCalibration → Ready；Apply 是单独状态。开始时启用传感器，再设置采样参数，注册本次键盘／鼠标监听并启动定时器，等待鼠标左键推进测量。

采样参数为 sample_rate=10、sample_count=0、measured_result=0。Initiating 超时 35000 ms，普通阶段超时 60000 ms；Mini 的校准窗口另为 5000 ms。设备事件区分 Start、End、Halt、Reset、Normal_Speed、Too_Fast、Cancelled，不能把“鼠标已经开始移动”当作完成。

StopCalibration 先提交 `setProximitySensorControl(...,Manual,Stop)`。收到 End 后，才调用 `getProximitySensorCalibrationData`；GET 成功后结合已收集 Z/Q 记录计算结果并发布，GET 失败发 Halt。Reset/Halt 有独立通知路径，随后回 Ready。

Z 数据乘 32 后计算均值与样本标准差（分母 n−1）；Q 数据以最高位分两组，其余七位除以 4 后计算 2 的幂，取两组均值的较小值作为 VPower；记录多于 31744 项时使用源码的 10000 分支。它们是校准算法中间量，不是页面可以任意改写的配置字段。

## 预设、记录与保存

预设鼠标垫具有固定 `surfaceDistance` 与十档 sqList；预设转换依赖本机保存的 `presetSensorData` 基线。只有表格而没有基线，不能复现完整预设转换。手动测量记录包含 sensorData、GUID、名称、surfaceId、type 和 calibrationDate；新自定义名称从 `MOUSE MAT` 开始，用空格加 1～100 后缀查重。

手动转换使用实测 mean、standardDeviation 和 VPower，调整四个 referenceHeight 并计算十档 liftOffThreshold。源码从 `3*standardDeviation/5` 得到的底值限制在 100～350，另计算上值及十档插值；这不是通用毫米换算。重新校准分支保留原记录 GUID／名称，并更新 calibrationDate，同时将新转换结果送入后续设备任务；不能假定它会用全新记录替换全部旧字段。

`25947.Rt` 先组织记录、更新 selectedProfile、推进版本，按本地保存开关写入并刷新 UI，再按设备访问开关排队 apply。另一个配置保存 owner 把本地持久化参数和设备派发参数分开。因此，“鼠标垫记录保存完成”“设备测量 End”“参数已 Apply”是三个节点。

Preset／Manual Apply 的设备顺序为：启用传感器 → 写 calibrationData → 写 threshold；每一步检查前一步返回状态，最后发 onApply。Mini Apply 只执行前两步，没有 threshold 写入。所述 apply 的完整函数体没有另做写后 calibrationData GET。

## 取消、失败与清理

Initiating 和 StopCalibration 都记录 `isBusy` 与 `isEscapePressed`。Escape 先发布 Cancelled；忙碌时延后清理，当前异步步骤返回后再停止定时器、释放本次鼠标／键盘监听、禁用传感器并回 Ready。定时器过期执行同类清理并发布 Halt。Ready 状态收到 Escape 也发 Cancelled，而不是成功 End。

源码 `lockMouse`／`unLockMouse` 当前为空操作，调用它们不证明操作系统鼠标被锁定。管理器级硬件／输入 listener 在构造时注册，属于它的较长生命周期；引擎的本次 Escape／mouse listener 和 timer 则有独立清理。不能把局部 stop-listening 当成 manager 的全部订阅已释放。

共享 `initCalibration` 创建实际选中的 manager 后订阅 calibration_events 和 screenlocked／screenunlocked。硬件 Reset 在屏幕锁定或刚解锁的一秒窗口被抑制；Orca5／Orca5Plus 的普通 Reset 分支取当前官方默认 calibration，设 trackingDistance=3，保存并投影；其他引擎排 `ON_CALIBRATION_CONFIG_CHANGE` 的 resetCalibration 动作。该特定 Reset 消费者不能扩大为读取失败、首次发现或无本地文件都执行默认写回。

所有事件先送 UI 事件消费者；End 必须带真实 data 才排 `ON_CALIBRATION_END` 和对应 UI task；Halt 设置 ERROR、必要时更新 Mini 面板，并排 `ON_CALIBRATION_HALT`。Cancelled／NormalSpeed／TooFast 没有再次进入 End 保存分支。`setSmartCalibrationDataFromUIToLocalStorage` 更新本地 calibration 的异步存储回调记录失败，没有将本地保存的返回值转换为设备应答。

## 原件与复核位置

- PID70 引擎：`.analysis/inputs/frontend/mouse-mw-70-current/raw/apps.razer.com/synapse/products/70/mw/philipsEngineManager.2703b2e8e046279839a4.js`。Ready `[2021,3881)`、Initiating `[4178,8202)`、StopCalibration／统计 `[15357,20285)`、Manual Apply `[20582,22412)`、Mini Apply `[45193,46825)`、Preset Apply `[52274,54086)`、manager `[55118,57004)`；本页复读以上原始区间。
- PID70 转换／保存：`.analysis/inputs/frontend/mouse-mw-70-current/raw/apps.razer.com/synapse/products/70/mw/6120.7fff31f083f844383e26.js`，手动转换 `[1035212,1036484)`，测量结束保存 `Rt` `[1551805,1554435)`。
- Smart Tracking 实际引擎：`.analysis/inputs/frontend/mouse-mw-170-current/raw/apps.razer.com/synapse/products/170/mw/9318.22c896a3706c9ac20036.js`，完整模块 69318 `[924,4537)`。
- Pixart：`.analysis/inputs/frontend/mouse-mw-92-current/raw/apps.razer.com/synapse/products/92/mw/pixartEngineManager.4696a25592494d62bb34.js`，Ready `[1801,3520)`、Initiating `[3817,7060)`、Starting `[7357,10728)`、Calibrating `[10731,13601)`、Stop `[13898,17645)`、Apply `[17942,19117)`、GetData `[19414,22576)`、Manual `[22597,25038)`、Preset `[25048,29271)`、manager `[29323,30985)`。
- SmartPixart：`.analysis/inputs/frontend/mouse-mw-99-current/raw/apps.razer.com/synapse/products/99/mw/smartPixartEngineManager.06281ae41ff840612091.js`，普通引擎 `[1029,5629)`、Preset `[5639,10621)`、Manual 完整状态及引擎 `[10631,31837)`、manager `[31889,33413)`；同目录 `6120.87cdb69a54dbbee728ed.js` 的常量模块 `[467637,469496)`、固定表／手动转换 `[2351237,2353543)`、选择与事件 `[2354392,2357098)`、普通转换 `[2358399,2359399)`、预设／手动调用及 cleanup `[2359405,2360873)`。
- Orca5：`.analysis/inputs/frontend/mouse-mw-165-current/raw/apps.razer.com/synapse/products/165/mw/orca5CalibrationEngineManager.4a67b1e63c3d01a6922b.js` `[1354,4512)`；218 差异为 `.analysis/inputs/frontend/mouse-mw-218-current/raw/apps.razer.com/synapse/products/218/mw/orca5CalibrationEngineManager.fe710a2501f870054f31.js` `[1862,4520)`。
- Pixart3349：`.analysis/inputs/frontend/mouse-mw-163-current/raw/apps.razer.com/synapse/products/163/mw/pixart3349CalibrationEngineManager.115aea4c73bd29878d0a.js` `[1345,3680)`；185／211 完整 class 与本节操作相同，仅打包绑定不同。
- Smart Tracking UI：`.analysis/inputs/frontend/mouse-190-current/raw/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js`，`mS` 从字节 5422000 后开始、在动态灵敏度常量前结束。
- 完整定位与字段表：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries.calibration.roles`、`engine_states`、`fixture_expectations`、`save_boundaries`、`event_adapters` 和 `queries.tracking.products`。

本页记录静态执行语义，未运行厂商 JS、未执行测量或设备写入；本地服务／真实设备取消与恢复效果仍未验证。
