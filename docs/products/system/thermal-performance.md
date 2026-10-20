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

编辑 CPU 参数先更新超频草稿与 saved/canApply；Apply 调用真实应用动作，并在尚未保存时另保存所选超频配置。保存 maker `Wd` 只保存文件；应用 maker `$d` 向超频 runner 排队。应用链调用已拆分的本地控制服务；`splitOCCoreVoltageOffset` 分支等待 1 秒后另发电压偏移。Reset Active 修改所选配置的编辑内容，与发送真实 Apply 是不同动作。

PID737 的 GPU 编辑器保留独立草稿：GPU offset −1000…400，VRAM offset −1000…3000，步长 1；Cancel 恢复 Redux 中的当前设置，Apply 才提交 `Qt(draft)`；单项 Reset 把该项置 0 并提交。电池时动作禁用。736 另禁止 CPU、GPU 同时选择最高档；711／737 允许各自的 Max 选项，并核对 Core Voltage Lock。

## 打开、切换、失败与停止

初始化读取真实 thermal fan ID 列表；本地配置恢复先发 `MW_SET_PERFORMANCE` 类状态，再按真实供电分支排队恢复。UI 与中间件的默认构造器并不完全相同：例如 PID737 UI 初始模式为 Balanced，中间件默认接电模式为 Maximum Power；不能从其中一个常量推断新连接的实测模式。电池优化恢复还具有系列及 `getBatteryOptimizerFromFW` 门控，见 [显示与电池](display-power.md)。

maker 保存和 runner 传输均有独立异常通道。runner 在设备调用后检查 abort，失败时 `result=false` 并拒绝；成功后才更新任务内存缓存，回调再发布 device completed/error 并结算事件。原件将队列 `skipped` 作为完成消费；页面中的乐观编辑不会因此成为额外 GET。

模式切换明确停止两个转速追赶 interval。系统监控有 start／stop 操作，但在接电且智能曲线运行时，页面关闭与停止显示监控不是同一个生命周期事件，后台仍需要温度控制回调。本页仅记录已读到的停止入口；不把泛型队列 `cancelAllTasks` API 的存在解释为每个退出分支都调用它。

## 原件和复核位置

所有区间均为原始 UTF-8 半开字节区间；读取时与各自 `acquisition.json` 的长度、SHA-256 核对。41 个产品的模式表、挂载与原始路径分别见 [系统索引](README.md) 和逐产品页的导航证据。

- PID737 UI `system-737-current/raw/apps.razer.com/synapse/products/737/ui/static/js/main.35ffe4e2.js`：产品模块 78193 `[447919,473718)`；Performance reducer `[785268,794060)`。目录前缀为 `.analysis/inputs/frontend/`。
- 同产品 UI `8923.a0bbaf75.chunk.js`：性能卡片 `[81721,98128)`；CPU 编辑器 `[107125,112217)`；智能风扇卡片 `[151290,158350)`；GPU 编辑器 `[159795,161047)`、范围 `[161052,161158)`；Performance 页面 `[161162,163790)`。
- PID737 中间件 `system-mw-737-current/raw/apps.razer.com/synapse/products/737/mw/6737.85397ce157d4cca228ee.js`：`Cd` `[891235,894465)`；硬件模式切换 `bd` `[894465,898545)`；`wd` `[898618,900111)`；`et` `[1134887,1138701)`、回调 `[1138701,1139769)`；固定转速 runner `[1139769,1141249)`、回调 `[1141249,1142277)`。
- 同一中间件：CPU 优化 `[920017,921616)`；超频保存 `[921616,922667)`；Reset `[922667,923640)`；Apply maker `[923640,924091)`、runner `[1229868,1230554)`；真实 fan ID getter `[1261762,1261936)`；低功率适配器 getter `[1268934,1269705)`；监控与插值模块 91058 `[2057043,2071520)`，固定停止 `[2069978,2070121)`、曲线停止 `[2071376,2071519)`。
- 同产品中间件 `rzDevice25.1042baf62cda9ce0c1bc.js`：thermal 编解码模块 13546 `[1260,12012)`；速度发送 generator `[2579,2956)`，模式发送 generator `[3488,3924)`。控制服务、平台桥和 native 二进制仍是不同证据层，本页不把 JavaScript 调用名当作底层 ACPI／驱动实现已经闭合。

本页是官方源码的静态语义说明；没有执行厂商 JavaScript、主程序或真实设备。Rust 实现、UI 后端接线、静态／模拟验证和真实运行验收分别记录，均不能由本页替代。
