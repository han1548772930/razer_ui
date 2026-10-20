# 鼠标：表面校准与抬起／落下跟踪

Calibration 页面包含两类实际功能：通过移动鼠标测量表面的校准流程，以及设置跟踪中止／恢复阈值的 Smart Tracking 流程。它们使用不同引擎、传感器选择值和完成事件；页面同名不代表设备命令相同。

## 已核实的分支

| 当前分支 | 作用与适用边界 |
| --- | --- |
| PID70 `philipsCalibrationEngine` | 预设鼠标垫、手动测量和 mini 校准；class=0、sensor=5，具有多状态测量、Z/Q 数据与本地鼠标垫记录 |
| PID170／204／226／45066 `orca5PlusCalibrationEngine` | Smart Tracking 与非对称抬起／落下阈值；class=0、sensor=4，无 PID70 那套表面测量状态机 |
| PID190 当前 `mS` 控件 | Smart Tracking 编辑器实例：普通档位与非对称双滑块、首次说明显隐；不据此断言它使用 PID70 测量协议 |

以上是直接复核和既有完整审计的范围。其他 PID 的引擎由家族目录的当前产品配置表定位，不能从传感器品牌、DPI 上限或相似外观推定。

## Smart Tracking 与非对称中止

普通模式的 `trackingDistance` 为 1～3、步长 1。非对称模式分别编辑 `liftOffDistance`（2～26）和 `landingDistance`（1～25），步长均为 1。抬起值降低到不高于落下值时，控件把落下值改为 `max(1,liftOff-1)`；落下值提高到不低于抬起值时，控件把抬起值改为 `min(landing+1,26)`。这些是源码的档位编号，不应直接标成毫米。

PID170 的 SmartTrackingEngine 先 `setProximitySensorState(0,4,true)`，然后以 `Universal_Lift_Cut_off=1` 设置 `trackingDistance-1`。非对称分支先启用传感器，再设置 `Preset_Calibration_Hysteresis=4`，最后把 `[liftOffDistance-1,landingDistance-1]` 作为 configuration 写入。失败通过 calibration-notification 发布；当前完整引擎没有匹配 lift-setting getter 的写后比较。

其真实 reset 硬件事件会发布校准通知。界面首次介绍弹层的显隐另存在本地偏好键，关闭介绍不会修改设备跟踪参数；介绍记录与设备校准状态是不同对象。

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

## 原件与复核位置

- PID70 引擎：`.analysis/inputs/frontend/mouse-mw-70-current/raw/apps.razer.com/synapse/products/70/mw/philipsEngineManager.2703b2e8e046279839a4.js`。Ready `[2021,3881)`、Initiating `[4178,8202)`、StopCalibration／统计 `[15357,20285)`、Manual Apply `[20582,22412)`、Mini Apply `[45193,46825)`、Preset Apply `[52274,54086)`、manager `[55118,57004)`；本页复读以上原始区间。
- PID70 转换／保存：`.analysis/inputs/frontend/mouse-mw-70-current/raw/apps.razer.com/synapse/products/70/mw/6120.7fff31f083f844383e26.js`，手动转换 `[1035212,1036484)`，测量结束保存 `Rt` `[1551805,1554435)`。
- Smart Tracking 实际引擎：`.analysis/inputs/frontend/mouse-mw-170-current/raw/apps.razer.com/synapse/products/170/mw/9318.22c896a3706c9ac20036.js`，完整模块 69318 `[924,4537)`。
- Smart Tracking UI：`.analysis/inputs/frontend/mouse-190-current/raw/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js`，`mS` 从字节 5422000 后开始、在动态灵敏度常量前结束。
- 完整定位与字段表：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries.calibration.roles`、`engine_states`、`fixture_expectations`、`save_boundaries`、`event_adapters` 和 `queries.tracking.products`。

本页记录静态执行语义，未运行厂商 JS、未执行测量或设备写入；本地服务／真实设备取消与恢复效果仍未验证。
