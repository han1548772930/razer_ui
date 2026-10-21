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

## 当前页面挂载、草稿与真实请求

本节核对当前 [GamepadProductWorkspace](../../../crates/razer-pages/src/features/gamepad/mod.rs) 的实际 renderer 与调用方，不能由页面数组或底层 setter 推断动作完成。它按页面键 dispatch Customize、Triggers、Thumbsticks、Lighting、Power、Calibration，Help 由共享 workspace 挂载。九款当前原件的 root/nav 已回读并核对获取收据字节身份，产品差异见 [家族入口](README.md#产品与适用边界)：2650 没有 Triggers；2647 另外受实读固件控制；2676/2684 使用现代 Thumbsticks；4115 是独立街机树；Power 仅2636/2647/2676/4133/4144，Lighting 仅2636/4115。

### Customize：图像存在，映射与 polling 写入尚未闭环

[customize_artwork](../../../crates/razer-pages/src/features/gamepad/gamepad_customize_artwork.rs) 已按产品/edition/layout 绘图、按钮热区、选择与 hover。普通手柄选中键后，`customize` 依据 functionList 绘制 Default、Controller assignment 和 Disable 的 Button 列表；`set_mapping` 只修改 `/profile/mappings` 并发 `GamepadProductChanged`。该实现没有完成原版 key mapping drawer、全部输出类别、编辑暂存、Save/Cancel 和离开提示。

2636 官方 `main.d624c214.js` 的实际 Customize 为 `Kh → Wh → Yh → Vh`；完整 `Vh [7421814,7448114)`、render `[7446451,7448113)` 保留映射面板与编辑消费者，`saveChanges [7438900,7439132)`、`dontSave [7439133,7439239)` 分别调用 `saveMapRef(true/false)` 后继续各自待执行动作。这里只把实际存在的本机编辑、保存和取消链作为还原依据；联网/账户动作仍按项目范围排除。

当前 polling 的 `choice("/profile/pollingRate") → write` 同样没有具名设备请求，且页面没有因此自动同步 `/profile/pollingRateWireless`。官方2636 MW 完整 `97901 [1334683,1336614)` 会按真实连接选择有线/无线字段、保存并排 runner，runner 获取当前档真实槽后先读 rate、不同才 setter；2647/2676 的双字段同步另由各自挂载配置决定。不能由有线菜单的当前数字推定无线字段或设备已变更。

两种动作都经 [source_workspace](../../../crates/razer-pages/src/features/workspace/source_workspace.rs) 的 `GamepadProductChanged → capture(snapshot)` 更新当前 profile，再发 `WorkspaceEvent::Changed`。当前 [Shell 分发](../../../crates/razer-shell/src/shell.rs) 对普通 Changed 只同步本机状态；这条路径没有将手柄 mapping/polling 转成实际 OBM 或设备 rate 写入。真正具名的 ControllerTrigger/Lighting/Chroma 请求另有订阅，不能借用它们证明普通 Changed 也传输了设备命令。

4115 `customize` 明确转 [kitsune](../../../crates/razer-pages/src/features/gamepad/kitsune.rs)，独立绘制模式、方向、SOCD 与 polling。其 mode/SOCD 控件仍使用 `choice → write`；应继续逐项核对本 PID 的设置、reset、profile和通知消费者，不得套普通手柄选择表或另一型号已存在的扩展 setter。

### Triggers 与 Thumbsticks：已有独立传输链

[triggers](../../../crates/razer-pages/src/features/gamepad/gamepad_triggers.rs) 实际挂左右产品图、Analog/Digital radio、range 双柄、1%–100% actuation、Rapid Trigger、reset及帮助；range预览不进入已提交 profile。2647 [navigation](../../../crates/razer-pages/src/features/gamepad/gamepad_navigation.rs) 使用官方四段固件比较语义，一次组件生命周期内只移除不重新插回，缺读数不构成设备支持证据。

[trigger runtime](../../../crates/razer-pages/src/features/gamepad/gamepad_trigger_runtime.rs) 将触发器、deadzone/sensitivity、preventDoubleDeadzone 与 circularity 分成类型化 operation，串行排队并捕获 profile、revision 与连接 guard。外层实际订阅 → `WorkspaceEvent::ControllerTriggerRequested` → [Shell controller_triggers](../../../crates/razer-shell/src/shell/gamepad/controller_triggers.rs) → discovery → typed IPC → [设备协议](../../../crates/razer-device/src/gamepad/controller_triggers.rs) 的生产链存在。写入先排本地保存，再核对当前唯一设备、请求 scope、响应 node/product/operation、固件应答；读操作必须携带实际 profile 观察，旧 revision 不能覆盖新草稿。写入应答、真实读取和本地保存按原件完成契约分开，不额外宣称统一 setter 后 GET。

[thumbsticks](../../../crates/razer-pages/src/features/gamepad/gamepad_thumbsticks.rs) 独立持有 Slider Entity、focus、preview、延迟提交、产品图、clutch assignment、模拟器和教程；legacy 的 Release 等待100ms后提交，2676/2684走现代控件。切页取消 pending编辑，重新进入 Thumbsticks 为本次挂载重新采集 rollback基线。真实输入另由 [controller_input](../../../crates/razer-shell/src/shell/gamepad/controller_input.rs) 发布，不能用调节slider后的草稿充当手柄移动。两类页面及IPC、协议已有纯/模拟测试入口，仍需按产品、档位、未知值、失败和重连逐项验证。

### 低/零死区警告与前次基线

2636 当前 `YM [7469637,7480185)` 的完整 constructor `[7469666,7477090)` 在 mount 保存左右初值，仅选择不低于7时替换相应 prev值；Continue不替换prev。低于7先更新死区并显示警告，关闭才重设记录的prev；Recalibrate只切 Calibration。通用模块 `99661 [6756139,6757811)` 不提供backdrop或Escape关闭。Rust [deadzone_dialog](../../../crates/razer-pages/src/features/gamepad/gamepad_deadzone_dialog.rs) 分别保存focus/return_focus、展开状态与`low_deadzone(path,previous)`，Continue、关闭回退、重新校准分别处理；prev与当前值不同时，关闭经`write → thumbstick_changed`提交恢复请求，不是只重绘当前值。

同值关闭仍有调用方差异：官方完整`onCloseDialog [7473389,7473742)`无条件调用`setDeadzoneData`；Rust的`dismiss_deadzone(true)`只调用会忽略同值的`write`，没有`commit_thumbstick_value`对同值的显式`thumbstick_changed`补发。因此当前挂载初值已低于7、再次选择同值并关闭的路径缺少这次派发。须独立覆盖该用例并继续核对官方MW的去重与设备结果，不能仅凭UI的重复派发推定一定多发一条HID命令。

现代2676/2684的Recalibrate直接打开相应摇杆popup，而legacy请求页面导航；前者发出的仍是缺少生产消费者的`CalibrationIntent`，见 [校准接线边界](calibration-and-lighting.md#v2-与-v3-摇杆intent-尚无执行消费者)。不能将提示关掉或弹层打开当成完成设备校准。

2636 CSS 的 alert `[451577,451871)` 宽402px、padding/gap20px；标题 `[451871,452047)` 16px；说明 `[459040,459171)` 最大宽360px，展开 `[459613,459815)` 最大高200px、height/opacity分别250/200ms。Rust保留这些尺寸与独立height/opacity/箭头动画，并关闭Escape/backdrop取消；仍需检查每款实际级联、页面坐标、文案、资源、焦点、重复触发、Continue后再次低值、离页与重新挂载。状态或尺寸局部对应不构成窗口1:1验收。

### Power：页面的分钟参数与写入缺口

当前Power必须与熄灯策略区分。2636实际组件`dm → Am [7359971,7360964)`、2676的`bm → wm [7628260,7629255)`均以`noSwitch:true`挂载，并从当前`powerReducer.powerSaving`读状态。有效档位来自2636模块22127的`h [627906,627923)`、2676模块14160的`N [442910,442927)`，均为`[5,15,30,45,60]`；组件直接显示该数字，说明`CONTROLLER_POWER_SAVING_DESC`明确为“设备在待机以下时间(分钟)后将关机”。不能误用组件内未生效的fallback`[1,3,5,10,15]`。

Rust `power()`从`spec.power_minutes`生成选项，当前独立SQLite目录库与维护输入也均保存`[5,15,30,45,60]`，却经`power_saving_label`按秒格式化，呈现5/15/30/45秒和1分钟，与上述实际页面不同。现有`power_saving_labels_follow_the_source_rule`测试只证明该helper的规则，不能证明本产品调用者的单位正确；必须用当前实际挂载的手柄分钟语义独立验证。

Power选择写`/controller/power/powerSaving/value → choice → write → GamepadProductChanged`，没有设备休眠setter、实际读取或确认请求。官方按钮另外调用`en.IS/Qa.IS({IdleMinute:e})`；它是统计上报，项目排除该联网行为。真正的本机休眠链由同一次点击的`dispatch(Em/Bm(...))`进入以下消费者。

#### 2636/2676 的实际休眠链

本节的 MW 完整原件分别是上述2636 `main.f0ca95429464cd09cbdb.js`与2676 `main.1a6971221454393778e3.js`；[2676获取收据](../../../.analysis/inputs/frontend/gamepad-mw-2676-current/acquisition.json)管理该 main、`2620.4dda40775797696f3bf0.js`和`rzDevice25SagePC.aa862e488f664a09de0e.js`。本次重新核对所用原件的实际字节、SHA-256和无语法错误的完整AST；现存统一capture的validate失败仍未被该局部核对消除，不能据此扩大完整语义获取状态。

| 完整所有者（UTF-8 bytes） | 2636 | 2676 |
| --- | --- | --- |
| UI action / optimistic reducer | `Em [7359810,7359847)`；`powerReducer [6870514,6871253)` | `Bm [7628205,7628242)`；`powerReducer [7109954,7110693)` |
| 产品override注册 | `injectData("getDeviceOverrides") [2665778,2666370)` | 同名注册 `[2722065,2722657)` |
| 实际getter / setter override | `Q [2656327,2656637)`；`Z [2656638,2656838)` | `Q [2712220,2712530)`；`Z [2712531,2712731)` |
| `30525.BX4` task maker | `fn [1841318,1842456)` | `yn [1888768,1889906)` |
| `85191.DVE` runner / `IvY`结果回调 | `kt [2002115,2003206)`；`Pt [2003207,2003861)` | `Pt [2050849,2051940)`；`Lt [2051941,2052595)` |

点击提交的是`h4K → ON_SET_POWER_SAVING`，payload为`powerSaving:{isEnabled,value}`，并非共享模块另一条`ON_SET_POWER_SAVING_VALUE`数值路径。UI先更新`powerReducer.powerSaving`，建立带timerTick的待处理动作并向MW广播。`83789.powerSavingDataAction → w`按宏录制状态及事件来源决定`updateLocalStorage/deviceAccess`：普通UI事件在允许设备访问时同时获得两项；宏录制占用时仅保存本机设置，来自storage的事件还可能被拒绝。当前`43001`按该决策将事件的`powerSaving`对象交给`30525.BX4`，不能把UI先改变或仅存储完成当成设备确认。

maker在允许存储时读取当前device document，更新根字段`powerSaving`与version，经当前产品storage override保存，并调用`96183.qw`更新运行时元数据；休眠设置不属于active profile里的亮度字段。允许设备访问时，排`NORMAL_SKIPPABLE / powerSaving`任务，任务主体为`85191.DVE`，回调为`IvY`；否则只完成本机动作。maker的storage receipt、memory cache及最后`xb`回调不等待设备runner的最终完成，均不能代替设备结果。

runner先保存本次参数并检查abort，按当前产品注册选择`powerSavingOverrides.setPowerSaving`，实际为表中的`Z`：仅取数值`value`，真实调用`setTimeToSleep(60*value)`，不额外发送auto-sleep开关。`Z`捕获setter异常并只记录日志；runner随后再次检查abort，再真实`getTimeToSleep()`，要求返回秒值严格等于`60*value`，不相等报`data not match`，getter/传输异常也进入失败。这里没有setter前预读，没有用setter返回payload作为最终比较，也没有失败时自动恢复旧值。共享runner另有`DeviceInfo.singleProfileDevice`原值分支，但这两款当前DeviceInfo均未声明该字段；2676的`useFeature("isSingleProfile","yes")`是另一项配置，不能据此把分钟换算删掉。

#### 相同休眠命令头下的产品端序

bootstrap的实际适配选择分别为2636 `rzDevice25Sage`（三个等待均10ms）与2676 `rzDevice25SagePC`（三个等待均30ms、reportId10）。两者继承的controller层为各自`2620`模块98773，继续到87969；**2636的Sage50207又重载休眠方法，2676的SagePC78786没有该重载**。不能由相同命令名或共享祖先推定payload相同。

| 当前实际分支 | GET / SET定义 | 秒值编码与解码 |
| --- | --- | --- |
| 2636 `rzDevice25Sage.b9b57f19edfe6aa9f049.js` | GET `[2,7,131]`，SET `[2,7,3]`；`getTimeToSleep [2950,3059)`、`setTimeToSleep [3059,3171)`，完整codec方法`[3171,3373)` / `[3373,3741)` | SET低字节在前；GET将`data[1],data[0]`交给高/低组合器，即小端。5分钟=300秒，payload为`[0x2c,0x01]` |
| 2676 SagePC继承`main`的57413 | 相同GET / SET定义，来自57938；57413完整模块`[948581,957426)` | 标准codec按高/低字节写入并读回，即大端。同为5分钟，payload为`[0x01,0x2c]` |

上述三元组依次为参数长度、command class和command ID。`87969.sendCommand`完整方法在2636 main `[1037249,1041468)`、2676 `[1042591,1046810)`：按实际host/Electron环境走HID feature report或USB control transfer，执行OUT、IN、应答状态与重试；失败抛出带命令和产品身份的错误，HID mutex在finally释放。休眠调用未关闭IN检查，因此不能套用软件灯光的无ACK帧语义。高层最终比较的是独立GET结果，实际DLL/设备执行是否一致仍须对照收据。

#### 初读、结果消费者与取消

初始导入仍按本页前述`11365.Rt`顺序：在profile读取之前，按root字段顺序处理全局`powerSaving`。其getter owner为2636 `I [1273773,1274328)`、2676 `h [1281488,1282043)`，选择已注册`getPowerSaving:Q`。`Q`真实GET秒值，SUCCESS时`round(timeToSleep/60)`，并从结果值形成isEnabled；查询异常返回未定义，外层记FAIL。该wrapper还构造初始`{value:5,isEnabled:true}`，工作document也可能包含旧值；这些分支和缓存均不得称为设备实读，不构成读取失败后写入默认值的授权。`_yk`是独立的读取投影入口，与用户`h4K`请求分开。

`IvY`将completed/skipped与error分别派为`MW_UPDATE_POWER_SAVING_VALUE_UI_ACTION_DEVICE`或storage来源的对应事件，随后发post-process结束事件。UI的`ce/Oe/Se`分别维护本机结果、设备结果和动作清理；2636完整范围`[6795617,6796830)`，2676`[7043084,7044297)`。错误进入`errorActions`，这些消费者本身不回滚已经optimistic更新的`powerSaving`，也不把GET确认值再次当用户动作提交。skipped只表示被较新任务替代，不能标为该请求执行过真实SET。

两款当前`81988 [553,10730)`队列保留current/pending/cancelling所有者：skippable取最新待执行项并跳过、释放其余pending；运行任务cancel发abort，等任务settle后调用结果回调、调度后续和dispose；pending取消直接发送`value:null,error:"canceled"`。Power组件没有Cancel按钮或卸载时恢复原值的动作，abort只在runner的SET前后检查，不能撤销已传输的SET，且该runner未在GET返回后再检查abort。另有须实际对照的源异常边界：`IvY`的error分支直接取`a.value.params`，而pending取消及等锁AbortError会传`value:null`；不能把该链描述为所有取消均已可靠清理。应用实现与对照测试须分别记录任务取消、真实设备当前值和资源释放结果，不能自行新增默认值覆盖或伪造回滚成功。

当前Rust只有前述草稿`Changed`，没有这条休眠Read/Set请求、秒值读回观察、设备失败或取消回执消费者。已有其它产品的Power IPC、手柄trigger确认或本机熄灯owner均不能补足该入口；应按实际产品端序与来源建立类型化休眠操作和独立观测，再把上述当前UI动作及失败/取消状态完整接上。

[controller_power.rs](../../../crates/razer-shell/src/shell/gamepad/controller_power.rs) 实际读取的是`profile.switchOffLighting/brightness`并观察显示器/idle事件，交给lighting owner执行亮度熄灭/恢复。该链已有生命周期和实际灯光命令，不消费Power的`powerSaving.value`，不能作为本页设备休眠已完成的证明。

## 验证与未验证范围

自动化必须覆盖每款实际页面/固件/传输及其映射类别、取消保存、polling有线/无线、active槽、触发器顺序、slider preview、死区重复警告/prev基线、clutch、失败、旧连接和清理。Power另覆盖分钟到秒、2636小端/2676大端、实际getter初读、setter异常后的GET、读回差异、宏录制只存储、skipped/pending取消、SET后取消及GET中取消；设备值与草稿不能相互冒充。已有 [页面纯测试](../../../crates/razer-pages/src/features/gamepad/gamepad_products_tests.rs)、[协议测试](../../../crates/razer-device/src/gamepad/controller_triggers_tests.rs)、[IPC测试](../../../crates/razer-ipc/src/controller_trigger_tests.rs) 及runtime内用例仅覆盖其明确行为；官方预期必须独立于Rust编码器。

本轮仅核对静态原件、AST边界、Rust生产caller、只读目录值与文档引用，没有重跑Cargo测试，没有执行厂商JS、主应用、helper或设备操作。实际DLL/设备功能结果按 [verification.md](../../../.agents/rules/verification.md) 逐项保存相同输入、双方原始输出、状态/应答、差异、原值恢复和释放收据；本轮没有新增该收据。真实写回、窗口/焦点/像素及不可执行的设备分支保持未验证，不能由现有模拟通过扩大到整个页面或产品。
