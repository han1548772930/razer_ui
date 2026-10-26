# 配置档、映射、触发器与摇杆

适用矩阵见 [家族入口](README.md#产品与适用边界)。4115 的 Customize 走街机方向键与模式分支；其余八款有双摇杆。2650 没有 Triggers 页，不能因共享 MW 有 trigger methods 而添加该页。

## 配置档的真实来源与重连

当前 2636 的完整导入所有者位于 `.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/main.f0ca95429464cd09cbdb.js`，模块 11365，bytes `[1271752,1282718)`；`Z [1279480,1279946)`、`oe [1280816,1282718)`。2676 对应当前原件为 `.analysis/inputs/frontend/gamepad-mw-2676-current/raw/apps.razer.com/synapse/products/2676/mw/main.1a6971221454393778e3.js`，同模块 `[1279467,1290523)`，`Z [1287222,1287688)`、`oe [1288558,1290523)`。原件来源、实际字节身份和版本见 [2636 MW 获取收据](../../../.analysis/inputs/frontend/gamepad-mw-2636-current/acquisition.json) 与 [2676 MW 获取收据](../../../.analysis/inputs/frontend/gamepad-mw-2676-current/acquisition.json)，并与 `.analysis/inputs.json` 核对。以下初始化结论以这两款原件为已核对范围，不由共享名称扩展到其余产品。

`oe` 在进入时即设置一次初始化 guard，随后读取或创建工作的本机 document；按其 **Object.keys 实际字段顺序**处理根字段，再执行 `DY`。2636 根分发表只有 `powerSaving`，新 document 的 `audioHapticIntensity` 仅产生未知功能警告；2676 根分发表支持 `powerSaving` 和 `controllerMode`，新 document 也按这一字段顺序读取。已有 document 缺少某字段时不会合成该字段的 getter。2676 的 `controllerMode` 经模块 38063 `[1364694,1366483)` 的 `getUsbDeviceEnumerate` 实读 0/1/2，分别投影 HID/Xbox/Xinput；未知值及失败保持失败，不能跳过这一阶段声称根导入完整。

`DY` 首次初始化引擎的实际设备 I/O 顺序是 **getButtonIDList → getProfileIDList → getActiveProfileID → getNumberOfProfiles → 按每个真实 ID 读取 getProfileNameStructure**，2636 引擎 `init [2504404,2505517)`、2676 `[2558675,2559788)`。2636 的实际导出 `DY` 为 `T [2515148,2515949)`，2676 为 `A [2569410,2570211)`。上层 `11365.Z` 在 `DY` 返回后访问 ID、active、names、count 是已有引擎缓存的投影，不能当成新的 HID 次序。名称读取按 UTF-16 处理 80 字节的 64+16 块，最多三轮，并在临时调整等待后恢复；2636 完整方法 `[2507687,2509510)`、2676 `[2561958,2563781)`。

槽对照函数将设备结果与本地集合匹配；找不到该设备槽时，源码创建一个有新 GUID 的 profile，并使用设备返回的名称和 `obmSlotId`。遍历真实 ID 时更新 active profile，再按该 profile 的实际字段顺序读取功能。导入结束依次保存 document、记录序列号对应 active GUID、更新运行时 metadata 与 `obmData/initCompleted`，最后经 `50350.pH` 发布。

字段分发表包括 brightness、quickEffects、mappings、pollingRate/pollingRateWireless、analog circularity、D-pad diagonals、digital/analog actuation、deadzone、sensitivity、haptic/audio haptic 和硬件 lighting effects；是否执行由当前 profile 的字段与挂载 feature 决定。`name/guid/obmSlotId/switchOffLighting` 不作为设备功能 getter。仅 `switchOffLighting` 和符合键身份、模式、输出类型条件的 `keyboardGroup.keyGroup` 从既有本机配置保留。最后依次保存 profile/document、记录序列号对应 active GUID、更新运行时信息并向 UI 发布。

源码的每个字段 getter 错误会被其循环捕获、记录并继续其他字段；整个 `oe` 返回不代表每个字段读取成功。源码还在缺本机 document 时构造默认 document，这个对象是读取工作的容器，不能把它的默认字段报告为设备实读。

2636 的启动所有者是模块 43001 的 `dn [1625087,1625700)`，2676 实际是模块 62885 的 `gn [1669824,1670437)`：先检查本机 document，仅在整个 document 不存在且注册 `loadProfileDataFromDevice` 时进入导入，已有 document 单独缺 `powerSaving` 不会因此启动导入。连接事件处理分别为 `hn [1650465,1650673)`、`On [1696514,1696722)`，先调用设备导入，再向 GenericFeatureManager 发布 DeviceConnected，最后执行 runtime 同步。`oe` 失败也不会自动清除 guard，普通重连不能据此声称重新读取所有槽；reset 的明确入口清除导入 guard，但不会自动清除 `DY` 自己的引擎初始化 guard。

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
| D-pad | diagonals 与 SOCD 是两种 feature | 2676/2684/4133/4144 的 SOCD `initFromDevice:true`；4115 为 false，2636 当前根没有挂载该 generic feature，不能仅因底层同名方法而启用 |

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

### 2636 的 OBM 映射读写原件边界

2636 的 MW 并不是把 Customize 的 `choice → write` 直接当成设备映射写入。当前 [main.f0ca95429464cd09cbdb.js](../../../.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/main.f0ca95429464cd09cbdb.js) 的 controller OBM owner 位于模块 11365 `[1271752,1282718)`；初始化后由 `obmEngineController` 维护真实 `rzDevice`、`buttonIdList`、`profileIdList` 与 `activeProfileId`。`getDataFromDevice` 先取得本机 document，再按真实活动槽调用 `getAllButtonAssignmentInfo(profileId)`；后者逐个调用 `getSingleButtonAssignment(profileId, buttonId, NormalMode, 10)`。每一项只在 `STATUS_SUCCESS && packetSizeReturned>0` 时解析 `classId, buttonId, mode, functionId, dataSize, dataArray[5]`，单项异常记录并继续，不是整张映射表原子读取。`functionId` 为 Controller 且其返回的第一个数据字节等于当前 button ID 或为零时，该项会被跳过；其余项再按 `fwIDToMapping/HIDToMapping` 转换为 UI 映射，并与 `DEFAULTMAPPINGS` 比较。默认映射和本机 document 是比较基线，不是设备读回结果。

写入入口是同一 owner 的 `setSingleButtonMapping(profileId, mapping)`（原件约 `[2512007,2512600)`）。`buttonMappingToFnDataByte` 先用 `inputIDToMapping.controller.NormalMode[inputID].fwID` 取得固件按钮 ID，再只为原件已有的 `DisableGroup`、`MouseGroup`、`KeyboardGroup`、`RazerKeyGroup`、`MultimediaGroup`、`ControllerGroup` 转出 `fnId`、`fnDataByte` 和 `fnDataByteSize`；未知 output type 只记录 warning，不会凭 UI 文案生成新的协议类别。该转换器把 `buttonMode` 固定为 `NormalMode`，所以它不能证明 Hypershift 层已写入；Hypershift 必须沿另一条有明确 mode 的 producer 单独核对。`DisableGroup` 的 payload 长度为零，其余 payload 会补零到命令缓冲区的 80 字节。

2636 `rzDevice25` 的实际命令头及其解析器如下（均为 class `0x02`）：

| 原生 owner | 命令头 | 请求／应答字段 |
| --- | --- | --- |
| `setSingleButtonAssignment` `[1089569,1091400)` | `[80,2,0x0c]` | `profileId, buttonId, mode, functionId, dataSize, dataBytes…`，不足 80 字节补零；仅传输结果由 `hn` 解析为 `STATUS_SUCCESS`／`packetSizeReturned` 与五字节 `dataArray` |
| `getSingleButtonAssignment` 同一 owner 后半段 | `[80,2,0x8c]` | 请求前三字节 `profileId, buttonId, mode`；成功且有 payload 才产生 `functionId/dataSize/dataArray`，缺 payload 保持未观测 |
| `setSingleKeyAssignment` `[1091400,1093500)` | `[80,2,0x0d]` | 与普通按钮相同的五字节函数数据，解析器 `An` 以 `keyId` 命名输入身份 |
| `getSingleKeyAssignment` 同一 owner 后半段 | `[80,2,0x8d]` | 请求 `profileId, keyId, mode`；成功且有 payload 才产生 `functionId/dataSize/dataArray` |
| `getButtonAssignmentList` | `[80,2,0x8e]` | 返回 `classId, mode, idOffset, byteData`；它不是 `getAllButtonAssignmentInfo` 的逐按钮结果 |
| `getKeyAssignmentList` | `[80,2,0x8f]` | 返回 `classId, mode, keyIdList`；与按钮列表使用不同命令和身份字段 |
| `setSingleAnalogKeyAssignment` `[1099000,1101200)` | `[80,2,0x12]` | 额外带 `thresholdL/thresholdH` 后再带函数数据；`Nn` 只有成功且有 payload 才填充阈值、函数和数据 |
| `getSingleAnalogKeyAssignment` | `[80,2,0x92]` | 请求 `profileId, keyId, mode`，回读包含 `thresholdL/thresholdH` 与函数数据 |

上表中的 GET 头 `0x8c/0x8d/0x8e/0x8f/0x92` 来自同一当前 MW 原件的常量表（约 `[993769,994300)`），不能用普通 setter 的 `0x0c/0x0d/0x12` 代替。每个 setter 先校验数值、`Uint8Array` 和 NaN；校验失败在发送前抛 `ERROR_INVALID_INPUT_VALUE`，因此本地实现不能把类型错误折叠成设备失败。`sendCommand` 返回成功也只代表当前报告的协议状态；官方 owner 没有在 `setSingleButtonMapping` 后追加逐项 GET，`getDataFromDevice` 的下一次读取属于另一个生命周期。

板载任务 `Et`（原件约 `[1982411,1983500)`）先检查 abort，再按 `obmSlotIds` 顺序逐槽调用 `obmEngine.setSingleButtonMapping`；单槽异常只记录并继续。循环结束后再次检查 abort，随后才写 task-runner memory storage 并标记 `completed`。因此 runner 的 `completed` 不能解释为所有槽都收到成功应答；取消可能发生在部分槽已经写入之后，也没有原件提供的自动回滚。Rust 侧当前 `set_mapping` 只更新 `/profile/mappings` 并发 `GamepadProductChanged`，没有调用上述 profile/slot/mode/函数数据命令；这正是“页面草稿存在”与“设备映射已写回”之间尚未闭合的边界。

### Triggers 与 Thumbsticks：已有独立传输链

[triggers](../../../crates/razer-pages/src/features/gamepad/gamepad_triggers.rs) 实际挂左右产品图、Analog/Digital radio、range 双柄、1%–100% actuation、Rapid Trigger、reset及帮助；range预览不进入已提交 profile。2647 [navigation](../../../crates/razer-pages/src/features/gamepad/gamepad_navigation.rs) 使用官方四段固件比较语义，一次组件生命周期内只移除不重新插回，缺读数不构成设备支持证据。

[trigger runtime](../../../crates/razer-pages/src/features/gamepad/gamepad_trigger_runtime.rs) 将触发器、deadzone/sensitivity、preventDoubleDeadzone 与 circularity 分成类型化 operation，串行排队并捕获 profile、revision 与连接 guard。外层实际订阅 → `WorkspaceEvent::ControllerTriggerRequested` → [Shell controller_triggers](../../../crates/razer-shell/src/shell/gamepad/controller_triggers.rs) → discovery → typed IPC → [设备协议](../../../crates/razer-device/src/gamepad/controller_triggers.rs) 的生产链存在。写入先排本地保存，再核对当前唯一设备、请求 scope、响应 node/product/operation、固件应答；读操作必须携带实际 profile 观察，旧 revision 不能覆盖新草稿。写入应答、真实读取和本地保存按原件完成契约分开，不额外宣称统一 setter 后 GET。

[thumbsticks](../../../crates/razer-pages/src/features/gamepad/gamepad_thumbsticks.rs) 独立持有 Slider Entity、focus、preview、延迟提交、产品图、clutch assignment、模拟器和教程；legacy 的 Release 等待100ms后提交，2676/2684走现代控件。切页取消 pending编辑，重新进入 Thumbsticks 为本次挂载重新采集 rollback基线。真实输入另由 [controller_input](../../../crates/razer-shell/src/shell/gamepad/controller_input.rs) 发布，不能用调节slider后的草稿充当手柄移动。两类页面及IPC、协议已有纯/模拟测试入口，仍需按产品、档位、未知值、失败和重连逐项验证。

### 低/零死区警告与前次基线

2636 当前 `YM [7469637,7480185)` 的完整 constructor `[7469666,7477090)` 在 mount 保存左右初值，仅选择不低于7时替换相应 prev值；Continue不替换prev。低于7先更新死区并显示警告，关闭才重设记录的prev；Recalibrate只切 Calibration。通用模块 `99661 [6756139,6757811)` 不提供backdrop或Escape关闭。Rust [deadzone_dialog](../../../crates/razer-pages/src/features/gamepad/gamepad_deadzone_dialog.rs) 分别保存focus/return_focus、展开状态与`low_deadzone(path,previous)`，Continue、关闭回退、重新校准分别处理；prev与当前值不同时，关闭经`write → thumbstick_changed`提交恢复请求，不是只重绘当前值。

同值关闭仍有调用方差异：官方完整`onCloseDialog [7473389,7473742)`无条件调用`setDeadzoneData`；Rust的`dismiss_deadzone(true)`只调用会忽略同值的`write`，没有`commit_thumbstick_value`对同值的显式`thumbstick_changed`补发。因此当前挂载初值已低于7、再次选择同值并关闭的路径缺少这次派发。须独立覆盖该用例并继续核对官方MW的去重与设备结果，不能仅凭UI的重复派发推定一定多发一条HID命令。

现代2676/2684的Recalibrate直接打开相应摇杆popup，而legacy请求页面导航；前者发出的仍是缺少生产消费者的`CalibrationIntent`，见 [校准接线边界](calibration-and-lighting.md#v2-与-v3-摇杆intent-尚无执行消费者)。不能将提示关掉或弹层打开当成完成设备校准。

2636 CSS 的 alert `[451577,451871)` 宽402px、padding/gap20px；标题 `[451871,452047)` 16px；说明 `[459040,459171)` 最大宽360px，展开 `[459613,459815)` 最大高200px、height/opacity分别250/200ms。Rust保留这些尺寸与独立height/opacity/箭头动画，并关闭Escape/backdrop取消；仍需检查每款实际级联、页面坐标、文案、资源、焦点、重复触发、Continue后再次低值、离页与重新挂载。状态或尺寸局部对应不构成窗口1:1验收。

### Power：分钟参数、独立设备确认与生命周期

当前Power必须与熄灯策略区分。2636实际组件`dm → Am [7359971,7360964)`、2676的`bm → wm [7628260,7629255)`均以`noSwitch:true`挂载，并从当前`powerReducer.powerSaving`读状态。有效档位来自2636模块22127的`h [627906,627923)`、2676模块14160的`N [442910,442927)`，均为`[5,15,30,45,60]`；组件直接显示该数字，说明`CONTROLLER_POWER_SAVING_DESC`明确为“设备在待机以下时间(分钟)后将关机”。不能误用组件内未生效的fallback`[1,3,5,10,15]`。

Rust `power()`从独立SQLite目录库中的`spec.power_minutes`生成`[5,15,30,45,60]`，`power_saving_label`按实际挂载组件原样显示分钟数字；说明文案提供单位。选项不再经过其它秒值控件的格式化规则。[分钟档位测试](../../../crates/razer-pages/src/features/gamepad/gamepad_sleep_tests.rs)同时核对两款实际产品目录和独立期望标签。

Power选择经类型化`ControllerSleepRequested`进入独立保存与设备队列；本地设备根字段`controller_power_saving`、待提交草稿和真实秒值观察分别持有。官方按钮另外调用`en.IS/Qa.IS({IdleMinute:e})`；它是统计上报，项目排除该联网行为。真正的本机休眠链由同一次点击的`dispatch(Em/Bm(...))`进入以下消费者。

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

初始导入仍按本页前述`11365.Rt`顺序：在profile读取之前，按root字段顺序处理全局`powerSaving`。其getter owner为2636 `I [1273773,1274328)`、2676 `h [1281488,1282043)`，选择已注册`getPowerSaving:Q`。`Q`真实GET秒值，SUCCESS时`round(timeToSleep/60)`，并从结果值形成isEnabled；查询异常返回未定义，外层记FAIL。该wrapper还构造初始`{value:5,isEnabled:true}`，工作document也可能包含旧值；这些分支和缓存均不得称为设备实读，不构成读取失败后写入默认值的授权。getter成功先修改工作document；虽然其对象声明了`updateUI → _yk`方法，`11365`导入循环没有调用该方法，不能据此声称GET当场执行了UI投影。

真正的连接后根字段消费者在2636 `An [1650674,1651999)` → `Sn [1651999,1653444)`，2676为`Cn [1696723,1698048)` → `Mn`。先刷新宏独占与device locks，再读取根document、按字段顺序处理；`powerSaving`分支先经`_yk`投影UI，再派发`ON_SET_POWER_SAVING`，来源为`LOCALSTORAGE`，最后进入profile apply。`83789.w`及其guard的完整范围为2636 `[2225612,2226832)`、2676 `[2276226,2277446)`：未录宏时`LOCALSTORAGE`只获得deviceAccess，其余UI/runtime/device来源同时获得storage与deviceAccess；录宏时`LOCALSTORAGE`直接忽略，其余来源仅storage。这条恢复链会进入前述真实SET与独立GET，不能把初始化概括成提前读取一次；例如实读301秒投影成5分钟后，源码恢复提交300秒并要求GET严格等于300。导入guard保留时runtime仍可能从本机根值进入恢复，不能把旧缓存称为本次实读；Rust接入必须保留当前读取出处、连接代次和恢复来源，读取失败、缺值、未知值或wrapper默认均不得触发默认覆盖。

`IvY`将completed/skipped与error分别派为`MW_UPDATE_POWER_SAVING_VALUE_UI_ACTION_DEVICE`或storage来源的对应事件，随后发post-process结束事件。UI的`ce/Oe/Se`分别维护本机结果、设备结果和动作清理；2636完整范围`[6795617,6796830)`，2676`[7043084,7044297)`。错误进入`errorActions`，这些消费者本身不回滚已经optimistic更新的`powerSaving`，也不把GET确认值再次当用户动作提交。skipped只表示被较新任务替代，不能标为该请求执行过真实SET。

两款当前`81988 [553,10730)`队列保留current/pending/cancelling所有者：skippable取最新待执行项并跳过、释放其余pending；运行任务cancel发abort，等任务settle后调用结果回调、调度后续和dispose；pending取消直接发送`value:null,error:"canceled"`。Power组件没有Cancel按钮或卸载时恢复原值的动作，abort只在runner的SET前后检查，不能撤销已传输的SET，且该runner未在GET返回后再检查abort。另有须实际对照的源异常边界：`IvY`的error分支直接取`a.value.params`，而pending取消及等锁AbortError会传`value:null`；不能把该链描述为所有取消均已可靠清理。应用实现与对照测试须分别记录任务取消、真实设备当前值和资源释放结果，不能自行新增默认值覆盖或伪造回滚成功。

#### Rust独立实现与观察边界

[页面runtime](../../../crates/razer-pages/src/features/gamepad/gamepad_sleep_runtime.rs) → `WorkspaceEvent::ControllerSleepRequested` → [Shell owner](../../../crates/razer-shell/src/shell/gamepad/controller_sleep.rs) → 当前唯一HID collection → [类型化IPC](../../../crates/razer-ipc/src/controller_sleep.rs) → [服务适配](../../../crates/razer-service/src/runtime/controller_sleep.rs) → [独立Rust协议](../../../crates/razer-device/src/gamepad/controller_sleep.rs)构成生产链。服务核对VID、当前逻辑/物理产品、interface、descriptor中的91字节report、连接身份和期限；每次真实传输前后重新核实collection。协议使用前述实际产品端序，SET异常保留后继续独立GET，以原始秒值严格判定确认或差异，不以UI四舍五入相同、setter成功或本地文件写入成功替代。

每次用户选择立即产生独立存储意图，不等待活动设备操作结束。统一writer先保存捕获的设备根设置；成功后才允许进入`NORMAL_SKIPPABLE`设备队列。活动任务保留，新成功保存的待执行任务替换旧pending；被替换项的本地保存仍有独立结果，不称为已执行SET。宏录制占用时保留存储、禁止设备访问；提交和存储完成时再次核实占用状态。旧profile中的`controller.power.powerSaving`只在产品、形状和值域合法且各profile无冲突时迁入设备根字段；profile快照不重复保存该字段。

真实观察保留`Unobserved/Reading/Observed/Failed/Disconnected`状态和完整`u16`秒值；页面使用现有本地化词条与语义status呈现读取中、失败、断开及未观测。未列出的分钟或非整分钟秒值明确显示实际秒值，不选造默认档位。读取失败不触发SET，不自动忙循环重试；明确离开再进入Power允许重新GET，仍在途的读取不重复提交。

[页面外初始化所有者](../../../crates/razer-shell/src/shell/gamepad/controller_initialization.rs) 通过[连接与文档所有者](../../../crates/razer-pages/src/features/workspace/source_workspace/controller_initialization.rs)、[类型化 IPC](../../../crates/razer-ipc/src/controller_initialization.rs) 和[独立协议](../../../crates/razer-device/src/gamepad/controller_initialization.rs) 持有连接代次、完整路由、字段计划和真实结果。根字段按实际文档键序生成；仅无文档时选择原件根字段读取计划，既有文档缺字段不补读。`serde_json` 在页面边界显式启用 `preserve_order`。2636 先按 connect 链真实读取 dock probe `[4,0,147]`，再进入根字段：Power、2676 mode `[1,0,211]`，随后 Button ID `[80,2,132]`、Profile ID `[80,5,129]`、Active `[1,5,132]`、Count `[1,5,128]` 和按真实 ID 的 Name `[80,5,136]`；名称最多三轮、每轮 64+16 字节，扩大 OUT/IN 与 IN 等待，OUT 等待保持原值，后续字段恢复正常等待。空/截断/矛盾目录失败，不产生 slot1 或空名称假成功。

当前 owner 按计划字段顺序独立读取数字/模拟触发器、圆度、死区、灵敏度、D-pad、haptic、2636 brightness/硬件灯光和轮询；单字段失败保留具体失败，并继续后续字段。结果经真实槽匹配本机稳定 GUID、真实名称和 active；已有文档在途被编辑时拒绝替换，断开、退出及 owner 替换取消 guard，迟到结果不能发布。正式保存使用共享 writer 的版本冲突与所有者检查；保存成功后才投影当前真实 Power 读数，并按宏占用规则发 `LOCALSTORAGE` 恢复 SET/独立 GET。301 秒保留为初读事实，投影5分钟的恢复请求确认300秒；缺值、失败、未知模式、默认容器和旧本机值不授权恢复。Trigger/Lighting/Chroma/Power 页内请求等待该链；初始灯光自动应用还要求当前 profile 的 brightness 和硬件 effect 均有真实观察，读取失败不能写其默认值。

轮询与映射原值由 [controller_profile](../../../crates/razer-device/src/gamepad/controller_profile.rs) 读取。2636 挂载 profile period `[2,0,142]`，code `1/2/3/4/5/8/10` 对应 `1000/500/333/250/200/125/100 Hz`；2676 明确挂载 high-speed `[2,0,192]`，code `1/2/4/8/16/32/64` 对应 `8000/4000/2000/1000/500/250/125 Hz`，未知 code 保留原值及 `None`，不采用 JS 的100/8000回退。每个真实 Button ID 的 normal-mode `[80,2,140]` 保留 slot/button/mode/function/dataSize/五字节原值及逐项失败、取消前缀。协议与枚举位置分别是2636 module14397@895919、82929@901137、assignment decoder@999583、engine detail@2513420、g2@2491377、polling getter@2542437、OBM mount@2664899；2676对应900386、905604、1004806、2567691、2545620、high-speed mount@2722718。

轮询的结果消费者也按原件分支处理：2676 `controllerDevice.config [2721836,2721972)` 明确 `syncWiredAndWirelessPollingRate:true`，`Bs/m [1292659,1292915)` 将同次实读同时写入两字段。2636 `Bs/E [1284854,1285110)` 则依真实 `isDongle` 选择字段；其 connect `z [2524527,2524861)` 在没有独立 dongleId 时仍会执行 `getDockExtendedFirmwareVersion` 探测，不能由同 PID 或无 peer 推定 wired。Rust 在根读取前通过实际 `[4,0,147]`、无 payload 的 Feature 应答观测；只有匹配 transaction/class/id、完整91字节report、状态2才形成 `ConnectionProbeObservation`。成功即证明 dongle，空/短的成功 payload 不强造版本号；至少4字节时另保留可选版本。该真实结果驱动2636 polling 的 wireless 字段，wired 字段保持独立。失败/取消保留错误与未知连接分类，不能采用 JS catch 的 false 当作已确认 wired，也不据猜测改任一字段。

映射导入由 [controller_mapping](../../../crates/razer-device/src/gamepad/controller_mapping.rs) 的具名领域类型实现：真实 input ID、generic HID、修饰键顺序、扩展键、鼠标／媒体／电源／配置档／灵敏度／控制器／Razer key／Windows shortcut／2676 Perfect180 均按当前 module 10511→4442→58701 转换。两款 module 58701 字节完全相同，共用一个所有者；其 126 个 KeyInput 的 HID 与身份已逐项核对。该表不能直接使用现有 app-engine DATA_0 代替：后者缺 HID 103/144/145/231，79..82 还存在 DPAD 身份记录，当前手柄则是 KEY_*_ARROW，并具有 Extended_00。这里不把其他产品表的别名或缺项当作当前手柄协议。

module 11365 的 DEFAULTMAPPINGS 过滤精确保留产品差异：2636 默认只有 M1..M6；2676 另有 SHARE_BUTTON keyboardGroup，源码只比较非控制器默认的输入／输出类型，因此 SHARE 的键值与修饰键不同也会被省略。仅在 inputID、isHyperShift、outputType 与 keyboard key 全部匹配时复制本机 keyGroup；不复制旧 modifiers、turbo 或任意旧映射扩展。profile 外部扩展及 switchOffLighting 继续保留。源 engine 每键 catch 后返回部分数组，导入据此替换本地 mappings 缓存；逐键原始读取和具名转换失败分别保留在连接结果及 mapping_projections，空缺键不被标为设备默认。取消或旧代次不能发布。宏目录不存在时按源保留空 id，且明确标为未解析；未知 HID、零周期 turbo 以及该产品没有转换器的 function 保留具体失败，不补默认映射。

importer 的一次调用标记由工作区所有者跨连接保留，进入即设置，失败和连接取消不会自动清除；重新创建工作区才具有新的模块生命周期。引擎目录完成事实独立保留，不能冒充重连后的新观察。启动 dn/gn 仅在 document 不存在时导入；连接 hn/On 在注册 loadProfileDataFromDevice 后直接调用 Rt，由 importer guard 去重，不可把启动阶段的 document 条件套到全部连接路径。2676 module 12601 的 taskMakerResetDevice（main byte 2714213 附近）在真实 setDeviceReset 后调用 11365.ho，只清 importer，不清 engine guard。Rust 手柄 reset、reset 后引擎目录复用、完整 profile apply 与 Customize 映射／polling 用户写入仍未完成，不能由本轮导入转换宣称整个根 gap 或产品完成。

当前设备配置档列表按真实槽目录重建。2636／2676 的 module 31759.eL（main byte 1337217／1369018）仅按 profileIDList 顺序创建新默认工作容器，按旧 obmSlotId 匹配并复用 source GUID；11365 随后调用 52448.setProfiles（1281575／1289380）替换当前序列号的列表，不合并目录外的旧槽。其他序列号的 profileData 和设备根扩展不在这个替换范围。逐槽读取枚举的是重建后当前默认容器的 getter 顺序，因此初始化计划不使用旧 profile 的键序或旧字段缺失推定跳过读取。容器默认字段仍不是设备实测状态，真实失败继续独立保留。

Rust 以当前 Device 的真实目录顺序原子构建新列表并保留匹配的 source GUID；旧 profile 的一个 GUID 对应多个槽时，各槽继续保留该 GUID，内部 Profile.id 则采用稳定槽身份区分。旧 obmSlotId 中首个仍存在的槽保留原内部 ID，其余槽由原 ID 和真实槽号派生；目录重排及再次导入保持稳定。活动配置档按真实 active slot 选择内部 ID，页面选择、修改和保存均按这个 ID 定位，source GUID 不充当列表下标。取消、目录缺槽、非法槽字段及冲突必须在替换前失败，保留原列表。

2636 dock probe 的当前完整来源链是 mount `[2665606,2665685)` 的 `isWireless:true`、factory `z [2523895,2524861)`、`isDongle` 消费 `[2514917,2515138)`、87969 getter `[1045319,1045536)`、12844 `Qp/x [887007,887236)`、14397 `mf/K [898293,898359)`，以及 encoder `[1032513,1032972)`、response reader `[1034207,1037249)`、sendCommand `[1037249,1041468)`。当前 `Sage` → 98773 → 87969 继承闭包没有 dock/sendCommand override；原件实际 bytes/SHA 与既有 acquisition 匹配，不由同族其他产品推导。

设备连接guard和本机存储guard分离：连接变化拒绝旧设备结果并丢弃旧设备pending，但保留已授权的存储完成责任；准备退出关闭新设备任务并取消旧连接请求，取消退出只重新GET，不恢复默认值或撤销已发送SET。离开Power本身不发送回滚。

Shell分别记录业务观察、匹配的服务teardown回执、最终进程/pipe释放和历史清理错误。`ServiceClient::is_stopped()`只表示停止接收请求；真正的`ShutdownConfirmation`只能由匹配Shutdown请求ID且`stopped:true`的响应产生，随后OS或pipe失败也不丢失token。后台owner仍调用`finish_termination()`，在Shutdown后及每次termination后重新取得typed token；真实句柄未释放时保留同一client继续释放。只有token存在、进程退出及pipe线程join全部真实完成、后台任务结束，才移除owner并释放页面队列。未实际获得client的提交前取消或spawn失败不要求不存在的服务回执。

没有token时，即便OS资源已经释放，`CleanupState`仍保留未确认entry并阻止同一资源的新操作及退出；此时不忙重试已经没有句柄的进程。barrier比较workspace、逻辑/物理PID、container和统一device-task Scope；同一path补全serial/manufacturer等信息、或同container从未解析route变成完整collection，均不能绕过旧失败。同一资源的阻断与结果发布身份分别判断，发布仍严格核对完整retained node；别的container、物理产品和已明确不同的collection可独立继续。服务token已经存在而OS释放稍后成功时允许继续，历史错误保留在诊断与收据中，不把既往错误永远当成未完成释放，也不把资源释放当成设备写入确认。

[controller_power.rs](../../../crates/razer-shell/src/shell/gamepad/controller_power.rs) 实际读取的是`profile.switchOffLighting/brightness`并观察显示器/idle事件，交给lighting owner执行亮度熄灭/恢复。该链已有生命周期和实际灯光命令，不消费Power的`powerSaving.value`，不能作为本页设备休眠已完成的证明。

## 2676 SOCD 连接读取与即时写入

2676 当前根在 MW main byte 2723004 挂载 `controllerDPadSOCD/initFromDevice:true`。module 62885 的连接 On（1696514）先 await 已注册的 `loadProfileDataFromDevice`，再 await generic manager 的 DeviceConnected，最后恢复其余 runtime。module 1457 的 onDeviceEvent（1399719）依次 await feature 连接回调并 await Pf 保存。Rust 的 importer 首段只有真实目录、profile 与根字段完成本机保存后才进入连接 phase；2676 每次连接 generation 单独读取 SOCD，再保存并放行 runtime。11365 importer 的 once guard 跨重连保留，10511 engine 目录也是独立缓存事实；复用该目录不标成新实测，也不重复整套 importer。

module 84470（1378063）的 initDataFromDevice 原样遍历 engine.profileIDList，为每个 wire slot 查找首个 `Fk(profile)==wireId` 的本机 profile，仅在匹配后 GET。10511.Fk（2570408）的 controllerDevice 分支把**已存在的**首个 obmSlotId 0 转为协议槽 1；本项目拒绝缺失槽号，保留 source slot 与 wire slot 的区别。连接匹配集合使用 Fk 结果，但 engine 原始目录 0 不转成 1，也不会凭目录 0 给 source slot 0 发送 SOCD GET。多个 profile 映射同一 wire slot 时只有源码 find 的首个 profile 消费该观察。GET 的 `[2,12,155]`／payload `[wireSlot]` 返回原始 option，未知 u8、失败和未观测分别保留，不采用原版失败/未知回 neutral 的回退。

首次 importer 的 profile 字段读取同样使用 Fk，目录和名称始终使用原始目录槽。2676 的 getDataFromDevice 闭包逐项为：11365 mapping（1279936）与 brightness（1282425）；19485 polling／wireless（1291986）；70784 lightingEffects（1360310）；84392 digital（1345387）；33710 analog（1296612）；69005 circularity（1298736）；91564 deadzone（1343766）；90707 ignoreDiagonals（1342430）；59785 sensitivity（1376946）；11365 haptic wrapper 转47442（1347719）。2636 的10511（2503776）也将 controllerDevice 中已存在的0映射为1。共享 `controller_profile::wire_slot` 统一这两款已核分支；外层 ProfileObservation.slot、目录、名称与本机 obmSlotId 保留 source0，Polling／Mapping／Lighting 的原始应答身份保留 wire1，并按该 wire 身份校验，不能把实报1改写成0。两款的完整当前字段序列均有独立固定帧测试；audioHapticIntensity 的当前 wrapper 无GET，未新增虚构命令。

UI `main.5ee115d4.js` iL（7536074）及 Customize 挂载（7548709）明确传入 `hideNoSocd:true`，未传 isArcadeController；可见 radio 顺序为 neutral、first、last，没有 Apply/Cancel。标题、帮助、说明分别为 `SOCD_SETTINGS`、`DPAD_SOCD_TOOLTIP`、`DPAD_SOCD_SETTINGS_DESC`，每项保留当前 `*_SOCD_DESC`。锁定 GPUI Kit 的 Base Radio 持有真实选中、焦点及键盘行为，页面按原 CSS `.socd-settings-container` 的左边距、上边距和说明字号呈现。radio 当前值只取本连接 typed GET 或明确的用户草稿，旧持久化值不冒充实测；草稿、已保存偏好、GET 观察与 SET 应答各有独立状态。

UI SET reducer（7115231）先乐观更新 value，再发送 `ON_SET_CONTROLLER_DPAD_SOCD`；MW 1457.taskMakerHandler（1402037）按 `updateStorage(active profile) → version → await Pf → updateUI → LOCAL_STORAGE completed → NORMAL_SKIPPABLE device task` 执行。Rust 的 source owner 绑定稳定 profile ID、source slot 和连接 generation，Shell 使用最新 saved snapshot 仅投影该 profile 的两个 SOCD 字段并经统一 writer 原子保存，保留其他 profile、根字段、并发已保存更改和未知扩展。保存失败不提交设备任务；仅保存、宏占用、取消与设备成功不能混为一谈。device queue 保留正在运行的任务，只跳过较旧 pending；profile／连接／退出变化取消旧 guard，迟到结果不能覆盖新 owner。持久化后的 SET 失败或取消不回滚已保存偏好。

明确的用户草稿、实际应答及失败诊断按完整 profile／source slot／generation 保存，切档往返仍能呈现本连接的用户动作，不能重新选回旧GET，也不能把同槽另一个profile的应答当成当前profile成功；重连清理该动作状态，再由新连接读数建立实测。每次保存仍独立排空，较旧保存失败不得替换较新用户动作状态；切档后返回同profile也不恢复已取消的设备guard，存储完成只更新真实保存事实，不自动重新提交旧SET。

同一设备的前一个服务尚在清理时，Shell 保留新请求及等待任务，计入退出排空；真实清理释放后重新核对完整 route、请求身份与宏占用，再提交 SET。等待期间切档、断开或退出会取消旧请求，不能因普通清理占用提前消费已保存的最新动作。磁盘原子提交与当前连接所有者分别记录；提交后连接变化只阻止旧设备请求，不将已经成功的文件写入误报为保存失败。

准备退出会取消初始化读取并清除当前观察，取消退出则重新开放同一连接的读取门；不重置 importer once、不伪造新的物理连接、不恢复旧观察。Shell 在退出取消后的 workspace 恢复完成后立即重新调度初始化；2676 重新执行真实 generic GET、保存完成再开放 SOCD，2636 恢复各 feature 原有读取。未完成首次导入保存时仍不能跨过保存门，也不因退出取消向设备发送本地默认值。

MW 84470.execute 再次读取 active profile，再经 Fk 发 SET，存在原版迟到任务跨 profile 的风险；Rust 将动作绑定到点击时真实 profile。`2620.4dda40775797696f3bf0.js` setter（16580）构造 `[wireSlot,option]`，命令 `[2,12,27]`；mode 映射 neutral0、last1、first2、none3、absoluteUp4。当前物理分支为 wired 2676/2677、wireless 2680/2681、VID5426、interface1；SagePC bootstrap（2720868）使用 report10、91 bytes 和 OUT／OUT_IN／IN 各30ms。独立 Rust 编码复用现有 Protocol25 传输，按 transaction／class／command 和 SUCCESS 状态确认并保留原始 ACK payload。源 setter 不核 ACK option 等值，也没有 SET 后 getter，因此本项目不增加 GET，不把 ACK 伪装成新的 GET 观察。

原版 1457.taskRunnerSetDevice（1405540）即使 execute 返回 false 也标 completed；generic 动态通知名与当前 UI 的 D_PAD_DIRECTIONS 通知分支也不一致。Rust 根据真实应答、传输异常与取消保留结果，不以 UI completed 日志代替成功。Shell 复用既有独占 ServiceClient、完整 route 身份、请求 guard 与统一 device-task scope；服务响应经 typed IPC 校验后才能发布结果。匹配 ShutdownConfirmation、最终进程／pipe 释放和历史清理错误分开保存，真实释放未完成时保留同一 cleanup owner，继续阻止同资源新任务，不能仅因拿到 SET 应答就放开清理屏障。

证据原件沿用当前 acquisition：`.analysis/inputs/frontend/gamepad-mw-2676-current/acquisition.json`（MW main、2620、SagePC）和 `.analysis/inputs/frontend/gamepad-2676-current/acquisition.json`（UI JS/CSS）。完整项目相对路径、实际 bytes/SHA 与依赖闭包由统一 inputs/gaps 校验维护；没有复制整套分析原件、创建逐问题脚本或临时分析数据库。本链真实设备 SET、实际 DLL 功能对照以及窗口／焦点／像素运行时验收均未执行，保持未验证。

## 验证与未验证范围

自动化必须覆盖每款实际页面/固件/传输及其映射类别、取消保存、polling有线/无线、active槽、触发器顺序、slider preview、死区重复警告/prev基线、clutch、失败、旧连接和清理。Power的[页面纯测试](../../../crates/razer-pages/src/features/gamepad/gamepad_sleep_tests.rs)、[协议测试](../../../crates/razer-device/src/gamepad/controller_sleep_tests.rs)、[IPC测试](../../../crates/razer-ipc/src/controller_sleep.rs)及[Shell清理测试](../../../crates/razer-shell/src/shell/gamepad/controller_sleep.rs)覆盖分钟到秒、2636小端/2676大端、初读失败不写默认值、setter异常后GET、原始秒值差异、宏录制只存储、独立存储及pending合并、旧连接、SET前后取消、GET中取消、明确重新进入重读、可见状态与真实清理失败保留及重试。新增 `controller_initialization` 纯/模拟用例覆盖根/真实目录/字段顺序、64+16名称与等待恢复、空目录、单项失败继续、根读取失败不写默认、取消前缀、IPC身份及计划伪造、陈旧连接、301→5→300恢复、宏占用和真实终态到控制字段的消费者。清理负矩阵分别检查服务token和OS释放四种组合、未获得client、已有token但reap暂时失败、无token且已释放不忙重试、相同/不同owner，以及None/Some route解析和同path元数据富化不能绕过barrier。预期协议字节独立于Rust编码器；这些纯/模拟用例不能替代实际DLL和设备对照，也未证明完整映射转换、全部初始化/恢复分支及整个产品完成。

Cargo静态检查与纯测试由统一验证入口执行，其结果只覆盖对应版本与用例；没有执行厂商JS、主应用、无关helper或设备操作。实际DLL/设备功能结果按 [verification.md](../../../.agents/rules/verification.md) 逐项保存相同输入、双方原始输出、状态/应答、差异、原值恢复和释放收据；当前未新增该收据。真实写回、窗口/焦点/像素及不可执行的设备分支保持未验证，不能由静态检查或模拟通过扩大到整个页面或产品。
