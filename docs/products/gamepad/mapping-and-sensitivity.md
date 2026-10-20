# 配置档、映射、触发器与摇杆

适用矩阵见 [家族入口](README.md#产品与适用边界)。4115 的 Customize 走街机方向键与模式分支；其余八款有双摇杆。2650 没有 Triggers 页，不能因共享 MW 有 trigger methods 而添加该页。

## 配置档的真实来源与重连

以当前 2636 的完整导入所有者说明共享流程：`.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/main.f0ca95429464cd09cbdb.js`，模块 11365，bytes `[1271752,1282718)`；`Z`、`X`、`oe` 分别在 `[1279486,1279943)`、`[1279944,1280537)`、`[1280816,1282717)`。

`oe` 有一次初始化 guard。先取本机 document，创建工作的 document；按其**实际字段顺序**读取全局功能，再执行 `DY`，随后顺序取得设备 profile ID list → active profile ID → profile names → number of profiles。槽对照函数将设备结果与本地集合匹配；找不到该设备槽时，源码创建一个有新 GUID 的 profile，并使用设备返回的名称和 `obmSlotId`。遍历真实 ID 时更新 active profile，再读取该 profile 实际存在的功能字段。

字段分发表包括 brightness、quickEffects、mappings、pollingRate/pollingRateWireless、analog circularity、D-pad diagonals、digital/analog actuation、deadzone、sensitivity、haptic/audio haptic 和硬件 lighting effects；是否执行由当前 profile 的字段与挂载 feature 决定。`name/guid/obmSlotId/switchOffLighting` 不作为设备功能 getter。仅 `switchOffLighting` 和符合键身份、模式、输出类型条件的 `keyboardGroup.keyGroup` 从既有本机配置保留。最后依次保存 profile/document、记录序列号对应 active GUID、更新运行时信息并向 UI 发布。

源码的每个字段 getter 错误会被其循环捕获、记录并继续其他字段；整个 `oe` 返回不代表每个字段读取成功。源码还在缺本机 document 时构造默认 document，这个对象是读取工作的容器，不能把它的默认字段报告为设备实读。

启动所有者是同一 main 的模块 43001：`dn` bytes `[1625087,1625700)`，先检查本机数据，仅在没有数据且注册 `loadProfileDataFromDevice` 时进入导入。连接事件处理 `hn` bytes `[1650465,1650673)` 先调用设备导入，再向 GenericFeatureManager 发布 DeviceConnected，最后执行后续连接处理；该调用仍受 `oe` 的 guard 限制，guard 保持时直接返回，不代表每次重连均重新读取所有槽。device reset 另外清除导入 guard，再重新读取；重连不是向设备写入旧配置的通用授权。

映射 getter 由模块 11365 的 `f.getDataFromDevice` → `obmEngine.getAllButtonAssignmentInfo(slot)` → `y` 处理。`y` 与产品 `DEFAULTMAPPINGS` 比较 input ID、输入/输出类型与 controller assignment，仅保留与默认不同的映射。默认映射表不是设备返回；保留本机 keyGroup 也不是设备读回。修改由具体 ON_SET/ON_CHANGE 任务进入 feature queue、存储和真实 setter，不应把普通本机 profile 编辑描述成设备 OBM 写入确认。

## 参数、单位与产品差异

| 对象 | 当前源码的表示 | 约束或分支 |
| --- | --- | --- |
| 左/右 trigger、stick | `controllerId:1/2` | 这是设备控制器身份；配置数组顺序不是身份 |
| Analog trigger | `startRange/endRange/isActive` | UI 百分比区间；起点小于终点，终点至 100；仅 active 行进入 setter |
| Digital trigger | `actuationPoint/isRapidTrigger/isActive` | 操作控件 1%–100%；未激活的源码默认记录允许 actuationPoint 0；Rapid Trigger 与 active 是不同标志 |
| Deadzone | `deadzones:[{controllerId,value}]`、`preventDoubleDeadzone` | UI 档位为 `0,3,5,7,9,11,13,15`；传输 deadZone 是两个字节，不能把 8 个档位下标当数值 |
| Sensitivity | `sensitivityValues:[{controllerId,value}]` | 档位 `0,10,20,30,40,50,60,70,80,90,100`；setter 传真实值 |
| Circularity | `standard` / `circular` | FW 值分别 0/1；写入左右两个 control ID；getter 路径以左侧值更新共享选项 |
| Polling | 有线与无线字段独立 | 2647/2676 的挂载配置明确同步两字段；其他产品不能套用该规则；PlayStation、无线菜单有自己的档位 |
| D-pad | diagonals 与 SOCD 是两种 feature | 2684/4133/4144 的 SOCD `initFromDevice:true`；4115 为 false，不能在启动时补一个猜测的 getter |

八款双摇杆产品的两个档位数组已直接核对各自当前 UI 原件：2629 `main.000315e2.js` bytes 7526386/7526410；2636 `main.d624c214.js` 7469578/7469602；2647 `main.7aa41341.js` 7431184/7431208；2650 `main.257d35dd.js` 7390526/7390550；2676 `main.5ee115d4.js` 7599408/7624376；4133 `main.d34fa647.js` 7556926/7556950；4144 `main.a5c50629.js` 7501285/7501309。各原件的完整目录均为 `.analysis/inputs/frontend/gamepad-<PID>-current/raw/apps.razer.com/synapse/products/<PID>/ui/static/js/`。2684 对应 `8169.3cf63c70.chunk.js` bytes 497479/522272，路径见家族入口。

2647 当前 UI descriptor `minFWSupportTriggers:"1.4.0.0"` 位于其 main byte 6055392 附近。根导航使用读到的固件按源码比较并移除 Triggers；不能把没有固件读数视为已达到版本。2650 根数组从一开始就没有 Triggers，属于另一种原因。

## 真实命令和应答

当前 2636 的下层协议完整原件为 `.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/2620.82f73bd688720e4613d7.js`，模块 98773。命令表第一项是参数长度，后两项是 command class/ID；这里 class 为 `0x0c`。

| 功能 | GET / SET ID | 传输内容与结果消费者 |
| --- | --- | --- |
| Sensitivity | `0x82 / 0x02` | classId、controlId、sensitivity 三字节；GET 方法 bytes `[7888,8153)` |
| Deadzone | `0x85 / 0x05` | classId、controlId，三个双字节值 deadZone/softBumper/deltaThreshold，加 preventDoubleDeadzone；GET `[8567,8820)`，SET `[8820,9390)` |
| Circularity | `0x92 / 0x12` | classId、controlId、option；GET `[11206,11480)` |
| Analog range | `0x93 / 0x13` | classId、controlId、startRange、endRange；GET `[11898,12173)` |
| Trigger mode | `0x95 / 0x15` | classId、controlId、mode；analog=0，digital=1 |
| Digital trigger | `0x96 / 0x16` | classId、controlId、actuationPoint、isRapidTrigger；GET `[13268,13534)` |

解码器只在返回 status 为 success 且存在返回 payload 时设置 `jsonData`；上层必须消费状态与真实字段。Analog setter 的完整上层在 2636 main byte 1287460 附近：按 active 行先 `setTriggerMode(...,0)` 再 `setAnalogTriggerRangeAdjustment`。Digital 在 byte 1318683 附近先 mode 1 再写 actuation/rapidTrigger。两个调用是顺序传输，不能合并为一个虚构的原子操作。死区 wrapper 明确传 `softBumper=0,deltaThreshold=0`，双死区预防独立映射为 1/0。

2636 的事件所有者把 `ON_SET_CONTROLLER_DEADZONE`、`ON_SET_CONTROLLER_SENSITIVITY` 等映射到具名 storage key、payload extraction、protocol function 和返回 UI 事件。它先从当前 document/active profile 定位槽，而不是接受 UI 随意提供槽号。UI optimistic reducer、document 保存完成和协议操作完成是三个可分别失败的阶段。关闭、换连接和取消不得把旧任务的返回应用到新设备；源码任务失败事件与 source time tick 应沿原消费者保留，不能因 UI 已先更新而伪造确认。

## 实时输入与通知

挂载 parser 见家族入口。输入回调进入 MW 的 `MW_UPDATE_CONTROLLER_BUTTON_EVENT_TO_UI`，按源代码做重复帧处理和约 33 ms 的 leading/trailing 发布，再由 Customize/Triggers/Thumbsticks 的实时指示器读取。亮度、映射、profile、haptics、battery、RF、SOCD、reset 等通知各有 payload 与消费者；普通模拟轴输入不等于配置 getter。设备离开、parser/连接改变或会话结束后，原连接的输入缓存和节流定时器都必须失效。

来源收据：[2636 MW](../../../.analysis/inputs/frontend/gamepad-mw-2636-current/acquisition.json)。跨产品 parser、bootstrap、完整输入 producer 与 native callback 依赖见 `.analysis/results/frontend/gamepad-main-audit.json` 内保留的完整模块引用及 `.analysis/results/frontend/gamepad-mw-audit.json` 的九款当前 source set。这里的命令说明不构成硬件写入测试记录。
