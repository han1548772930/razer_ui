# 配置档、映射、触发器与摇杆

适用矩阵见 [家族入口](README.md#产品与适用边界)。4115 的 Customize 走街机方向键与模式分支；其余八款有双摇杆。2650 没有 Triggers 页，不能因共享 MW 有 trigger methods 而添加该页。

## 配置档的真实来源与重连

2636 已接续当前部署 [main.f3d3e2116ff224e8ddf5.js](../../../.analysis/inputs/frontend/gamepad-mw-2636-current-refresh-ec3db4b29c5a/raw/apps.razer.com/synapse/products/2636/mw/main.f3d3e2116ff224e8ddf5.js) 的模块 11365：完整模块 bytes `[1288302,1299268)`，`Z [1296036,1296493)`、`oe [1297366,1299267)`。来源与当前选择复用 [Reset 收据](../../../.analysis/results/frontend/gamepad-mw-audit.json#/profile_reset_chain)，完整导入编排见[本页专节](#2636-当前-import-编排与部分失败边界)。旧部署的偏移不作为当前部署定位。

2676 的已核验原件为 `.analysis/inputs/frontend/gamepad-mw-2676-current/raw/apps.razer.com/synapse/products/2676/mw/main.1a6971221454393778e3.js`，同模块 `[1279467,1290523)`，`Z [1287222,1287688)`、`oe [1288558,1290523)`；来源、实际字节身份及版本见 [2676 MW 获取收据](../../../.analysis/inputs/frontend/gamepad-mw-2676-current/acquisition.json)。本批不重新核对该产品，也不由共享名称扩展其他产品。

两款 `oe` 均在第一次 I/O 前设置导入 guard，再按工作 document 的 **Object.keys 实际字段顺序**读取根字段；缺 document 才构造默认工作容器，默认值不是设备实测。2636 根分发表只有 `powerSaving`；2676 另有 `controllerMode`，经模块 38063 `[1364694,1366483)` 的 `getUsbDeviceEnumerate` 实读 0/1/2，分别投影 HID/Xbox/Xinput。既有 document 缺字段不会补读；逐字段失败继续、最终导入返回不能证明全部读取成功。

`DY` 首次实际设备 I/O 顺序是 **getButtonIDList → getProfileIDList → getActiveProfileID → getNumberOfProfiles → 按每个真实 ID 读取 getProfileNameStructure**，其后 `11365.Z` 的 ID／active／names／count 来自引擎缓存。当前 2636 `DY` 为 `T [2564175,2564970)`、完整引擎 `init [2553425,2556123)`；2676 的 `DY` 为 `A [2569410,2570211)`、`init [2558675,2559788)`。名称读取按 UTF-16 处理 80 字节的 64+16 块，最多三轮，并恢复临时等待调整；2676 完整方法为 `[2561958,2563781)`，2636 名称链复用正式 mapping／polling 收据。

2636 当前启动 `Sn [1659804,1660417)`、连接 `kn [1686243,1686451)`；2676 分别为 `gn [1669824,1670437)`、`On [1696514,1696722)`。启动仅在整个 document 不存在且注册 `loadProfileDataFromDevice` 时导入；连接在已注册时调用同一 importer，再通知 GenericFeatureManager，最后同步 runtime。导入失败不自动清 guard；Reset 明确入口只清导入 guard，不清 `DY` 的独立引擎 guard，普通重连不能据此声称重读全部目录。

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

当前 Rust polling 的 `choice("/profile/pollingRate") → write` 仍没有具名设备请求，也不会因此自动同步 `/profile/pollingRateWireless`。PID2636 最新完整输入选自 [MW 获取收据](../../../.analysis/inputs/frontend/gamepad-mw-2636-current-refresh-ec3db4b29c5a-2/acquisition.json)与 [UI 获取收据](../../../.analysis/inputs/frontend/gamepad-2636-current-refresh-ec3db4b29c5a/acquisition.json)，限定链记录于[当前 polling 静态收据](../../../.analysis/results/frontend/gamepad-mw-audit.json#/polling_native_chain)，入口为 `audit-family-function --family gamepad --function mw --query gamepad-polling-native-chain`。原文档的旧版偏移只作旧边界索引，下面以当前完整 owner 为准。

当前 Customize 实际挂载 `Qm → qm`，有线菜单只有 250／1000，无线只有 250；一个选项时组件直接呈现 active，不能把视觉选中当成实读。`Fm`／`Hm` 先更新 reducer 的请求值，再经 `ON_POLLING_RATE` 进入当前 MW `97901.N → p`。dispatch 用无线 payload 是否定义区分输入，并以 `wireless || wired` 取请求；maker 再按执行时的 `10511._` 判断保存 `pollingRateWireless` 或 `pollingRate`。当前产品未设置 `syncWiredAndWirelessPollingRate`，不能套用其他型号双字段同步规则。卡片自己拥有 focus listener 和 1500ms 检查 interval，卸载时移除／清除；非点击导致的 prop 改变还能重新派发设置，不能把每次 action 等同一次物理写入。

maker 先取得当前 document／active profile、改版本并 `await Pf` 保存，再广播本地完成，最后按授权排 `NORMAL_SKIPPABLE/pollingRate`。`Pf` 在缺 serialNumber 时可直接返回而不保存，外层没有据此区分保存失败。runner `f` 只在 GET 前检查 abort，随后重新取得活动 profile，`Fk` 取其 `obmSlotId[0]`；源码缺值回 0 并对 controller 将 0 转 wire1，这不是设备实际返回的槽。runner 调用 `87887.gN`，仅返回 rate 与请求不同才调用 `_4`，忽略 setter 的返回对象；完成回调以 `value.status=completed` 或 `error=skipped` 发布完成。没有独立写后 GET、应答 profileId 等值校验或自动回滚。GET 传输异常在 helper 内被捕获并返回 0，可使 runner 随后继续尝试 SET；`jsonData.pollingRate` 的访问位于该 catch 之后，不能把所有异常都概括为回 0。

PID2636 注册 `isObm.yes.pollingRate=true`，未注册 high-speed polling；当前实际走 `getProfilePollingRate`／`setProfilePollingRate`。GET 头为 `[2,0,0x8e]`，SET 为 `[2,0,0x0e]`，payload 为 profile ID／period code。1000、500、333、250、200、125、100 分别编码为 1、2、3、4、5、8、10，SET 拒绝其他 rate；这组协议值域与当前 UI 两档不同。decoder `j` 无 status／长度门控，直接解 `data[1]`，未知或缺 period 会投影为 100；该源码回退不能被 Rust 冒充设备实测。继承链为 `Sage → Controller → rzDevice25`，reportId 为 4、数据报文 90 字节、期望 OUT count 为 91；IN 按当前继承实现匹配 transaction／class／command 和 SUCCESS 状态。公共 Host 4.0.896／HID 原生 feature-report、Promise、队列与释放结论复用[已核公共子段](../../../.analysis/results/frontend/audio-mw-audit.json#/auto_pause_hid_chain/shared_feature_transport)，用其精确 pointer／canonical SHA 绑定并在发布前重核。

初始化按实际存在的 profile 字段遍历，`pollingRate`／`pollingRateWireless` 都映射到同一个 getter feature，产品 override 按连接观察选择保存其中一个。首次启动仅在 document 缺失时导入；连接也调用同一 once guard，guard 在 I/O 前置位，失败不会自动清除。连接判断来自 dock probe 的 SUCCESS；catch 保留源码本地 false，不能据此证明有线连接。controller 的 `updateUI` 取所选字段但统一发 `MW_SET_POLLING_RATE_TO_UI`，UI wired／wireless 状态仍分别存在。

恢复 profile 的无线分支调用 `96183.z3`，这是本地缓存 rate／1000 clamp 和 productId 门控，没有 GET。它另行比较 MW `DeviceInfo.dongleId/bleId`，而当前2636 MW 未定义这两个字段；UI `DeviceInfo.dongleId=2636` 不能代入 MW。因此即使 `controllerDeviceInfo.isDongle=true`，实际 PID2636 也不会通过 z3 的产品门禁，不能声称该分支完成了设备恢复。`POLLING_RATE_CHANGE` 通知则按枚举文案数字发布 rate，是独立观察消费者。任务没有把 AbortSignal 传入 HID，transport 的 finally 只请求释放 mutex；卡片卸载、任务取消和 mutex 释放都不证明在途 I/O／callback 已排空或设备已恢复。

本片只闭合2636当前挂载 polling 的有限静态语义与结果消费者。映射保存与配置应用分别见下文对应收据；其他产品／固件／传输、通知订阅、独立 Rust 与实际 UI／后端接线仍须分别完成。纯维护测试、官方实际 addon／设备对照及窗口验收分别记录，不能由本地保存或 completed 广播推定硬件已确认。

两种动作都经 [source_workspace](../../../crates/razer-pages/src/features/workspace/source_workspace.rs) 的 `GamepadProductChanged → capture(snapshot)` 更新当前 profile，再发 `WorkspaceEvent::Changed`。当前 [Shell 分发](../../../crates/razer-shell/src/shell.rs) 对普通 Changed 只同步本机状态；这条路径没有将手柄 mapping/polling 转成实际 OBM 或设备 rate 写入。真正具名的 ControllerTrigger/Lighting/Chroma 请求另有订阅，不能借用它们证明普通 Changed 也传输了设备命令。

4115 `customize` 明确转 [kitsune](../../../crates/razer-pages/src/features/gamepad/kitsune.rs)，独立绘制模式、方向、SOCD 与 polling。其 mode/SOCD 控件仍使用 `choice → write`；应继续逐项核对本 PID 的设置、reset、profile和通知消费者，不得套普通手柄选择表或另一型号已存在的扩展 setter。

### 4115：设备模式与 SOCD 的不同存储和恢复链

本节 MW 使用[当前稳定部署闭包收据](../../../.analysis/inputs/frontend/gamepad-mw-4115-snapshot-81ed5dc91a596f97101f-closure/acquisition.json)实际选中的 `main.81ed5dc91a596f97101f.js`、`8255.78367f9ded0bc62c92d6.js`、`2620.72a37b50422562eadd07.js` 和 `rzDevice25SagePC.b645b2573dcf878c8e1f.js`。这些原件保留于 `.analysis/inputs/frontend/gamepad-mw-4115-snapshot-81ed5dc91a596f97101f/raw/apps.razer.com/synapse/products/4115/mw/`；路径、来源、实际 bytes/SHA 由闭包收据维护。旧 `gamepad-mw-audit.json` 中 4115 的 `main.8c755a3c43990d2c3900.js` 和捕获失败仍属原快照；新部署没有完整 capture，以下是当前原件的定向 AST／调用链阅读，不能将旧 capture 当成新部署已通过。

4115 当前 UI 选择[刷新部署获取收据](../../../.analysis/inputs/frontend/gamepad-4115-current-refresh-ec3db4b29c5a/acquisition.json)的 `main.e1eadc09.js`，不再使用旧 `main.1a52fc92.js` 给当前 MW 证明挂载。Customize 完整 `hm [7882543,7884873)` 在非 macro 显示模式挂载左侧 `Rm/Mm` 与右侧 `Lm({isArcadeController:true})`。当前 `Um [7884879,7885943)` 经过 connect 和保留 props 的 wrapper `[7885600,7885678)`；包装层的 repaint hook 持有 100ms 定时器及 clearTimeout 清理，实际 Customize tab 仍消费该包装后组件。Mode 的 `Mm [7881517,7882542)` 只有 safe、standard 两项；点击先小写化并比较当前值，只有变化才发 `ON_SET_CONTROLLER_MODE_SWITCH`，没有 Apply／Save／Cancel。`Mn [7366968,7367375)` 先乐观更新 `controllerModeSwitcherReducer.mode`，再经 `Pn [7366677,7366967)` 广播；收到 `MW_SET_CONTROLLER_MODE_SWITCH_TO_UI` 或对应本地存储动作时另行更新同一字段。UI 的初始 safe 和 selector 的 safe 回退只是界面初值，不能报告为设备实读。

MW main byte 2686743 附近实际注册 `controllerDPadSOCD/initFromDevice:false`、`deviceReset/isFirmwareReset:false` 和 `controllerModeSwitch`；模块 `1457 [9304,22017)` 按此配置动态加载 `8255` 的 `58255 [549,2381)`。Mode 事件配置 `u [986,1542)` 明确使用 `Storage_Global | Skip_Restore_Device_Data` 和根字段 `controllerModeSwitch`，`updateStorage` 只替换设备 document 的 `{mode}`，不改 active profile。`getRuntimeData [1672,1906)` 从本机 document 读取该根字段，替换已有同名 runtime 消息后投影给 UI；这里没有设备 GET。Mode feature 没有 `onDeviceEvent` 或 `restoreDeviceData`，因此不能因打开页面、连接或切换 profile 就把初始 safe 写回设备。

Mode 的真实 setter 在 chunk8255 `[1906,2278)`：固定第一个参数为 `1`，safe 编码 `0`，standard 编码 `1`；原件对其他字符串也走非 safe 分支，但当前页面只提供这两项。SagePC 的完整当前类继承 controller 协议且没有 Mode／SOCD override。`2620` 的 `setControllerModeSwitch [16995,17249)` 发送头 `[2,12,28]`、payload `[1,option]`；`rt [6011,6079)` 仅在 SUCCESS 时投影应答 `data[1]`。上层只比较 setter 的 SUCCESS，没有追加 GET，也没有比较应答 option 等值；同模块已有的 GET `[2,12,156]`、方法 `[16754,16995)` 并未由本次 Mode 点击或 runtime getter 调用，不能虚构统一写后读回。

两个控件共用的真实提交所有者是当前 MW `1457.taskMakerHandler [15175,18681)`：先按动作授权修改 document、version、`await Pf` 保存，再显式 `updateUI`；获得 deviceAccess 时才排 `NORMAL_SKIPPABLE` 设备任务。`taskRunnerSetDevice [18681,19962)` 保存 execute 的布尔结果并在执行后检查 abort，但即使返回 false，未抛异常的路径仍把 `value.status` 标为 completed；结果回调 `[19962,21899)` 又以这个 status 分类。Mode 的位标志还会按当前按位表达式抑制通用通知，显式本地保存投影仍存在。因此乐观选中、保存完成、通用 completed 与真实 SUCCESS ACK 必须分别记录；原版没有提供自动回滚或写后 GET。

SOCD 另属 active profile：当前 UI `Lm [7876646,7878758)` 支持 `hideNoSocd` 参数，但 4115 的实际调用者没有传入该参数，因此仍挂载 neutral、first、last、absoluteUp、none。点击直接发送 `SET_SOCD_SETTINGS → ON_SET_CONTROLLER_DPAD_SOCD`，没有 Mode 的同值去重。MW `84470 [1800481,1803684)` 的 `initFromDevice:false` 只阻止 DeviceConnected／DeviceReset 回调进入本 feature 的逐槽 SOCD 读取；其显式 `restoreDeviceData` 仍由 profile 恢复调用：当前字段存在则执行 SET，缺字段才先 GET 再 SET，并更新 UI。MW 的恢复总 owner `[2210336,2241820)` 最后调用 `1457.restoreDeviceData [13940,14281)`，不能把“不做连接导入”解释为“永不读取或写入 SOCD”。

SOCD 执行时重新选择 active profile，经当前 `10511.Fk [212431,212559)` 取首个 `obmSlotId`，controller 分支将已存在的 0 转为 wire slot 1；未知／缺失槽不能由本项目补成已确认槽。转换为 neutral0、last1、first2、none3、absoluteUp4 后，`2620.setControllerSOCDCleaningOption [16483,16754)` 发送 `[2,12,27]`、payload `[wireSlot,option]`；GET 方法 `[16225,16483)` 使用 `[2,12,155]`。硬件 SOCD 通知另经 main `[1834547,1835023)` 更新当时 active profile、保存并发布 `MW_SET_CONTROLLER_DPAD_SOCD_TO_UI`，不更新设备根 Mode。原版 GET 失败／未知值回 neutral 的行为只作证据，本项目继续按规则保留失败／未知，不能因此恢复默认值。

统一复核命令为 `python -X utf8 tools/workspace.py audit-family-function --family gamepad --function mw --query gamepad-mode-socd --product 4115`。[现有正式 MW 审计](../../../.analysis/results/frontend/gamepad-mw-audit.json)的 `gamepad_mode_socd_current` 保存本批 59 个完整 AST owner、实际 UI／MW 当前选择、组件挂载与包装、全部所用 factory、命令及应答解析边界、硬件通知和恢复消费者；保留其他字段的 canonical SHA，不覆盖九款基线及旧失败。当前 UI 与 MW 均无完整 capture，收据保持该事实。工具的七项定向纯静态测试覆盖当前选择、原件字节重现、实际 arcade props、GET／SET 差异、语义变更、孤立 loader、通知目标与 false/completed 区分，不执行厂商代码。

以上闭合的是 4115 当前页面、两类存储、设备命令、结果和恢复消费者的静态语义。当前 Rust 的 Mode／SOCD `choice → write` 尚未接通这些具名设备请求；用户写回、全部 profile/reset 分支迁移、实际 DLL／设备功能对照和真实窗口仍未完成。本次没有执行厂商 JS、DLL 或设备，没有新增临时副本或分析数据库。

### 2636 当前 OBM 映射保存与结果边界

当前 MW 选择为 `gamepad-mw-2636-current-refresh-ec3db4b29c5a-2`，实际复用已核验的 [main.f3d3e2116ff224e8ddf5.js](../../../.analysis/inputs/frontend/gamepad-mw-2636-current-refresh-ec3db4b29c5a/raw/apps.razer.com/synapse/products/2636/mw/main.f3d3e2116ff224e8ddf5.js)；UI 选择为 `gamepad-2636-current-refresh-ec3db4b29c5a`。Customize 父组件挂载 [6793 映射编辑器](../../../.analysis/inputs/frontend/gamepad-2636-current-refresh-ec3db4b29c5a/raw/apps.razer.com/synapse/products/2636/ui/static/js/6793.d778641f.chunk.js)，Save 取得当前子编辑器的映射，按 `inputID/isHyperShift` 合并或移除默认项，再调用 `setMappingList`；Cancel 清理并关闭，未保存的关闭动作使用原版保存提示。UI 先替换 `mappingList`，随后发送 `ON_SET_KEYMAPPING`，该乐观草稿不代表设备实测状态。

当前实际分支是产品 `mappingOverrides.taskMakerSetKeyMapping = 21445.Hw` → controller mapping feature → `59639.ew` → `10511.obmEngineController.setSingleButtonMapping`。产品同时选择 `storageOverrides.localStorageHandler` 和 `serialNumberBasedProfileStorage`：活动 profile 从 `profileData[当前 serialNumber].profiles` 按该项的 `activeProfile` 查找。maker 先更新映射与版本、保存本地 document、刷新 runtime profile 信息并发布 `MW_SET_MAPPING_TO_UI`、本地 completed／postprocess，随后才排 `NORMAL/obm` 任务。任务依次提交过滤后的映射及已移除映射的 `BUTTON_LIST` 默认项；当前只使用 `Fk` 取出的首个槽，不走通用 `mt` 的多槽循环。源码的 controller 槽 `0 → 1` 转换与缺失值回退不能当作设备实读槽位。

转换器固定使用 `NormalMode`；当前产品声明支持 Disable、Keyboard、Controller、Multimedia 四类，通用转换器额外含 Mouse／RazerKey 分支不能据此扩大本产品能力。Controller UI 名称先转换为编码枚举，键盘转换使用当前 HID／修饰键表，Multimedia 再交换相邻字节。实际按钮命令如下：

| owner | 命令头 | 请求与结果 |
| --- | --- | --- |
| `setSingleButtonAssignment` | `[80,2,0x0c]` | `profileId, buttonId, mode, functionId, dataSize, dataBytes…`，不足 80 字节补零；源码此处没有独立的超长或 profileId 校验 |
| `getSingleButtonAssignment` | `[80,2,0x8c]` | 请求前三字节 `profileId, buttonId, mode`；`Pn` 仅在 `SUCCESS && packetSizeReturned>0` 时解析，函数数据固定取 `data[5..10]` |

实际 Sage → Controller → protocol25 继承、report ID `4`、90 字节报告体／91 字节 OUT、transaction／class／command 与 SUCCESS 应答门槛，以及 Host／原生 HID 传输和 mutex 释放，复用正式收据的精确 `polling_native_chain` 与共享 HID section。setter 后没有独立 GET。另一个读取生命周期逐按钮读取、逐项捕获异常，跳过 Controller 自映射／零映射，再与 `DEFAULTMAPPINGS` 比较并投影；默认表与本地配置均不是读回结果。

专用 runner 在单次写入前后检查 abort，但会捕获写入异常，随后仍可能更新 memory storage 并标记 completed；没有自动回滚，取消时可能已有部分映射写入。本地 `_9Q/Bl$` 由 Customize reducer 按 timerTick 消费；设备 `Pxp/N2z` 发往独立 OBM reducer，没有匹配 pending entry 时直接忽略。两类消费者均不触发 GET。存储 override 无 serialNumber 时也可直接返回，因此 completed 不能证明落盘、设备成功或读回确认。

统一复核命令为 `python -B -X utf8 tools/workspace.py audit-family-function --family gamepad --function mw --query gamepad-obm-mapping-chain --product 2636`。[正式 MW 审计](../../../.analysis/results/frontend/gamepad-mw-audit.json) 的 `obm_mapping_chain` 保留 128 个角色、158 个完整 AST owner、19 个复用 owner 引用及两个精确上游 section；10 项定向静态测试覆盖真实分支、报文、保存顺序、异常、取消、消费者、依赖变更和失败发布。本片只闭合当前普通按钮映射 Save／结果片段；配置应用、Reset 与 Import 编排由后续专节分别复核，通知生命周期及其他产品／固件／传输另行接续。Rust `set_mapping` 仍只更新草稿／profile mappings 并发 `GamepadProductChanged`，本片 OBM 生产请求、UI／后端接线及实际 DLL／设备对照未完成。本片未执行厂商 JS、DLL、设备或应用，未创建临时副本、数据库或捕获目录。

### 2636 当前配置切换与全部设置字段应用

沿用上文同一当前 UI／MW 获取收据，限定链见 [profile_apply_chain](../../../.analysis/results/frontend/gamepad-mw-audit.json#/profile_apply_chain)。统一复核入口为 `python -B -X utf8 tools/workspace.py audit-family-function --family gamepad --function mw --query gamepad-profile-apply-chain --product 2636`，复用已有 polling、OBM mapping 及公共 Host／HID 的精确 section，不重抓原件。

真实挂载顺序为产品 render → 导航的 `renderProfileBar` → `o_/i_` → dropdown → profile GUID → `Qr` → `SET_SELECTED_PROFILE`。reducer 先乐观更新所选 GUID，再广播 `ON_SWITCH_PROFILE`；调整模式与 dropdown 的禁用条件保留原行为。产品 bootstrap 将切档 override 指向 `31759.NC`，state guard 只授予 exclusive 本地存储动作。maker 先按当前序列号保存 active GUID、version、metadata 与 profile stack，再发布 runtime；UI／系统来源才核对活动槽。

该槽核对读取 `obmEngine.obmJson.activeProfileId` 缓存；不同时才发 `[1,5,4]` SET，SUCCESS 才更新缓存。协议虽提供 `[1,5,132]` GET，当前切档核对没有调用这个 GET。槽 SET 失败在内层只记录日志，仍继续配置应用与 completed；源码也没有随后 GET 或自动恢复旧选中项。document 读取位于外层 try 前，异常可能直接拒绝，不能把所有失败都概括成相同结果事件。

当前 `69427.DEFAULTPROFILE` 有 `guid/name` 两个身份字段与以下 13 个设置字段，未导出 `PROFILE_DATA_PATCH`；导入附加的 `obmSlotId` 是槽元数据。

|实际字段|应用路径|
|---|---|
|`switchOffLighting`|由 brightness 分支消费，无独立设备事件；diff 只有该字段时不会自行触发 brightness|
|`brightness`|`ON_INIT_BRIGHTNESS`，依真实开关注册显示器／idle 事件并执行亮度链|
|`controllerLightingEffects`|generic init → `ON_SET_CONTROLLER_EFFECTS` → `50077/70784`|
|`pollingRate`、`pollingRateWireless`|依实际连接选择字段；有线排 `ON_POLLING_RATE`，无线走已有 `z3` 门控，保留上文未写回的真实边界|
|`mappings`|先投影 UI，再携整份 list、diff 和 version 进入 `ON_SET_KEYMAPPING`；当前 controller wrapper 忽略 diff 参数，从当前 active profile 投影完整映射列表|
|`controllerDigitalActuation`|只处理 active 行，先 mode1，再 point／rapid|
|`controllerAnalogActuation`|只处理 active 行，先 mode0，再 start／end|
|`controllerAnalogCircularity`|standard0、其余1，依次写 controller1、2|
|`controllerDeadzone`|逐行发送 deadzone 与 preventDoubleDeadzone，softBumper／delta 为0|
|`controllerDPadIgnoreDiagonals`|布尔值转换0／1后 SET|
|`controllerSensitivity`|逐行发送 controllerId／value|
|`hapticIntensity`|产品 `ce` override，将源档位映射后依次写 motor5、6、7、8|

`74867.IF/Gn` 以 serial profile handler 选活动配置，完整调用按实际键顺序遍历，diff 调用只遍历 diff 键；先发布列表与选择，再排字段事件。generic default 的 `ye(...).catch` 没有 await，六项 init 重新读取当时配置并先投影 UI。最终 await 的 `GenericFeatureManager.restoreDeviceData` 会逐 handler 捕获异常，不是设备队列排空。因此 profile completed、字段本地保存和设备成功必须分别记录。

六项 generic setter 共用实际 controller 协议：mode `[3,12,21]`、digital `[4,12,22]`、analog `[4,12,19]`、circularity `[3,12,18]`、zones `[9,12,5]`、diagonals `[2,12,23]`、sensitivity `[3,12,2]`。zones 的三个 16 位量由 `76912.E4/l7/Jz` 明确按**高字节在前**编码／解码；不能套用其他命令端序。decoder 仅在 SUCCESS 且返回字节数大于0时投影；上层不比较回显字段，也没有写后 GET。generic runner 还会把已解析对象的 status 改为 completed，只在操作后检查 abort，未把 signal 传入 HID；本地保存失败、跳过、部分写入、取消均不能冒充实测成功。

UI 的 profile 结果按 timerTick 维护 pending／error，不回滚所选 GUID；六项设置的 UI 发布消费本地配置。circularity／diagonals 的结果 case 使用 `CIRCULARITY_MODE/D_PAD_DIRECTIONS`，generic maker 发布 `CONTROLLER_ANALOG_CIRCULARITY/CONTROLLER_DPAD_IGNORE_DIAGONALS`，这些 case 不构成对应 generic ACK 的确认。sensitivity 发布仍保留源码 `Object.assign({}, sensitivityValues)` 的对象投影。卸载清除 profile storage listener、timer 和 dropdown click／blur listener，但不代表在途设备队列已取消或排空。

该收据保留 136 个主链角色及 113 个亮度／灯效／震动角色，共 249 个完整 AST owner，并分别固定正文摘要与精确依赖。这里只确认 PID2636 当前切档／应用的静态语义；Reset 与 Import 编排由后续专节单独核对，通知及其他产品分支继续保留原边界。独立 Rust 实现、生产 UI／后端接线、官方实际 DLL／设备对照与窗口验收仍未完成。本片未执行厂商 JS、DLL、设备或应用，未创建临时捕获、数据库或副本。

### 2636 当前 Reset 的三个实际入口

[profile_reset_chain](../../../.analysis/results/frontend/gamepad-mw-audit.json#/profile_reset_chain) 沿用普通 Apply 的同一当前 UI／MW 获取收据，精确引用 `profile_apply_chain` 及其 mapping／polling／Host-native 依赖。统一复核入口为 `python -B -X utf8 tools/workspace.py audit-family-function --family gamepad --function mw --query gamepad-profile-reset-chain --product 2636`；不重跑 13 个设置字段的普通写入链。

|实际入口|当前源码的动作与边界|
|---|---|
|配置栏菜单 Reset → `qr` 的 Reset Profile|按所选 GUID 进入 `ON_RESET_PROFILE` → 产品 `31759.IH`。克隆 edition0／128／129 共用的默认档，保留原 name／guid／obmSlotId；先替换序列号配置、递增 version、保存并发布 runtime，再依旧 mappings 逐按钮恢复官方默认映射，最后完整调用普通 `IF`。`IF` 应用的是当前 active profile；重置其他 GUID 不会自动切档。这里没有 `setProfileReset` 命令。|
|同一确认框的 Reset Keybinds|实际挂载未传 `isCustomShowReset`，因此该按钮可见；`cXd` 先乐观清空 UI mappingList，再发 `ON_SET_KEYMAPPING`，参数为 `mappingList:[]/isResetKeybinds:true`。存储、移除映射的默认值恢复、真实传输和结果消费者复用上文 mapping；其余 12 个设置字段不因此重置。|
|Help 页 Factory Reset|产品导航实际挂 `SN({}) → ON`，connector 未传 `resetObm`、等待 OBM 完成或 overlay 参数，所以确认后广播 `ON_RESET_DEVICE`。共享 dispatcher 先检查已注册 generic handler；产品 `resetDevice` override 为 `ne`，不能把另一条 `ON_RESET_OBM → dsf` 当成这个按钮的路径。|

配置档 Reset 逐次等待旧映射的 `setSingleButtonMapping`，但不检查已解析返回的 status，也没有写后 GET。`Vir completed/yqg postprocess` 在普通 `IF` 排队后发布，随后才写 task memory；后一步失败仍可能在 completed 之后发 error。结果继续由 Apply 的 timerTick pending／error 消费者处理，不回滚已保存默认档，也不构成设备写入全部结束的屏障。

设备 override 先将本产品文档替换成 `getDeviceDefaultLocalData + next version`，等待保存；再从 linked-game 各项的 devices 数组中移除本 PID，保留其他产品关系及空的 game 项。随后以默认 UserData 调用实际 Controller `setDeviceReset(0)`：有效头为 `[2,0,11]`，载荷 `[1,0]`。源码另有 mode1 条件分支，当前无参调用没有选择它。该层直接返回共享 `sendCommand`，复用精确 HID/native 与 mutex 释放契约，没有额外 Reset decoder、status 检查或独立 Reset GET。

SET promise 返回后才清除导入 guard `te`，接 `DD` 的重新导入入口、DeviceConnected handlers、runtime 同步，最后 DeviceReset handlers。它不清除另一份引擎初始化 guard `S`，所以不能据此断言重新读取了所有 OBM 元数据。导入主体与逐字段编排由下一节的 `profile_import_chain` 接续；本节只确认 Reset 如何调用它。`ne` 捕获并记录异常，没有关联的 Reset success／error 事件，也没有恢复已保存文档、linked-game 关系或部分写入。

两种确认框的外部点击仅取消确认。`qr` 卸载移除 click listener；设备确认框移除 listener 并清除两个布局 timer。Help 自身没有 `componentWillUnmount`，重置创建的匿名 2 秒 timer 未被清除；timer 恢复按钮不代表设备 ACK、重新导入或队列排空。三个 Reset owner 均未向 HID 传递取消信号，也没有旧连接代次检查、补偿写入或在途工作排空。

本节保留 60 个角色、80 个完整 AST owner 和 38 个精确复用 owner 引用；13 项定向静态测试覆盖三个挂载入口、身份与槽保留、保存／写入顺序、默认分支、guard 分离、报文、失败、取消清理、CLI 及依赖变化时拒绝完成。只确认当前 PID2636 这一片静态语义；Import 编排见下一节，通知、其他产品／固件／传输、独立 Rust 与生产 UI／后端、实际官方 DLL／设备结果及窗口验收仍未完成。本批未运行厂商 JS、DLL、设备或应用，未创建临时捕获、数据库或副本。

### 2636 当前 Import 编排与部分失败边界

[profile_import_chain](../../../.analysis/results/frontend/gamepad-mw-audit.json#/profile_import_chain) 从 Reset 的导入缺项接续，精确复用 Reset／Apply／OBM mapping／polling 及共享 HID 依赖。统一复核入口为 `python -B -X utf8 tools/workspace.py audit-family-function --family gamepad --function mw --query gamepad-profile-import-chain --product 2636`。本节核对 module11365 的根字段、目录、逐槽读取、保存与消费者，不重捕 native，也不扩展到 2676 或其他字段底层协议。

`oe` 在第一次 await 前设置 `te=true`，失败不自动清除；`11365.ho` 只清这份标记。`DY` 的 `S` 独立存在，实际 engine.init 返回后才置位，已置位时跳过初始化。导入先读取 document，缺失才创建默认工作容器；按根字段实际键序执行，分发表只有 `powerSaving`。新容器的 `audioHapticIntensity` 仅记录未知字段，不会调用 profile 的音频震动 getter。Power 先读 autoSleep enabled，enabled 才读取秒数并四舍五入为分钟；禁用或时间失败时保留的源码默认5不属于实测时长。

`Z` 先等待 `DY`，然后取得缓存的 profile IDs／active／names／count。目录重建只遍历真实 `profileIDList`，按旧槽匹配仅复用 GUID，创建默认 profile 并替换当前序列号列表；数量不生成额外槽。逐真实 ID 查找 profile，必要时创建新容器，真实 active 匹配时才选择相应 GUID。源码 `setProfiles` 先选择首项，空目录可能抛错，active 不匹配时可能仍选首项；这些本机选择不是设备 active 观测。一个旧 GUID 匹配多个槽时，后续按 GUID 的 `find` 可能只找到首项，源码没有新增冲突校验。

当前默认容器实际 getter 顺序为 `brightness → controllerLightingEffects → pollingRate → pollingRateWireless → mappings → controllerDigitalActuation → controllerAnalogActuation → controllerAnalogCircularity → controllerDeadzone → controllerDPadIgnoreDiagonals → controllerSensitivity → hapticIntensity`。`name/guid/obmSlotId/switchOffLighting` 不进入 getter；分发表虽声明 quickEffects／audioHaptic，当前重建容器没有这些字段，不据此增加读取。每字段的 undefined／error 被单独记录后继续，`status:"FAIL"` 本身不是这一层独立拒绝条件，因此部分结果及默认工作值仍可能留在 document。每槽结束按首个 `obmSlotId` 匹配旧 profile，仅保留 `switchOffLighting` 和 inputID／isHyperShift／outputType／keyboard key 均匹配的 `keyboardGroup.keyGroup`，不复制旧 modifiers、turbo 或任意映射扩展。

目录阶段的外层 catch 之后仍继续：第一次 `Pf(document)` 保存 → `kS` 记录序列号 active GUID → `EA` 更新 profile stack → 第二次 `Pf(document,runtime)` → `qw` 更新 metadata／window storage → 设置 `obmData` 与 `initCompleted` → `50350.pH` 发布。最终保存或发布失败会拒绝，没有原子回滚。READY／isObm 条件下，`pH` 发布 `N_y` 列表／容量、`XAX` 完成与序列号门控的 `Bj5`，再由 `47399.Q` 广播 `REDUX_STORE_UPDATE`，UI OBM reducer 克隆 profiles／macros 并设置初始化标记。源码容量为0，不是实测容量；完成标记与广播均不证明全部 getter 成功。

这条 importer 没有 AbortSignal、旧连接代次检查、独立 timer／订阅／handle 或 finally 清理，也不构成借用 engine／HID 在途工作的排空屏障。收据保留 44 个差量角色、66 个完整 AST owner 与 46 个既有 getter／存储／UI owner 引用；12 项定向静态测试核对实际字节、当前部署变化、两 guard、根与字段顺序、空目录／active、逐项失败、保留规则、两次保存与消费者。已有 Rust 初始化实现及纯／模拟事实继续由下文独立记录；当前编排语义尚未完成对应 Rust 迁移、生产接线或实际 DLL／设备与窗口验收。本批未运行厂商 JS、DLL、设备或应用，未创建临时捕获、数据库、副本或日志。

### 2636 配置任务取消、连接替换与退出

[profile_lifecycle_2636_chain](../../../.analysis/results/frontend/gamepad-mw-audit.json#/profile_lifecycle_2636_chain) 接续 Import 的生命周期缺项，使用相同当前 UI／MW 获取收据，精确引用 Import／Apply／Reset／mapping／polling 及共享 HID。复核入口为 `python -B -X utf8 tools/workspace.py audit-family-function --family gamepad --function mw --query gamepad-2636-profile-lifecycle-chain --product 2636`。本批新增 28 个角色、47 个完整 AST owner，引用 39 个既有产品 owner 与共享 Host／17 个原生函数；未重新捕获 native 或创建分析会话。

任务管理器在当前 MW 上下文只初始化一次，按任务名称建队列，以容器 ID 组成 WebLock 名；任务 ID 区分活动任务，不是连接代次。普通队列取最早任务，可跳过队列取最新任务并对替代项发布 `skipped/result:false`。待执行取消会移除任务、发布 `canceled/result:false` 并释放 ID；运行取消则把任务移入 `cancellingTask`，abort 后立即清空 `currentTask`，由未来 settlement 负责移除和释放。等待 WebLock 的 AbortError 单独发布 `aborted`，不能与 handler 失败、原生 I/O 取消或完成排空混同。

完整当前 MW 的 AST 共找到 3 处 `cancelTask/cancelAllTasks` 调用：两处是管理器内部转发，第三处属于通用 OBM Reset。2636 实际挂载 `deviceResetOverrides.taskMakerResetOBM=ne`，该产品 override 不调用上述取消接口。实际 disconnect、cleanUpDevice 与连接替换所有者也没有调用它们；因此不能从取消 API 存在推断页面退出已经取消全部任务。事件 FIFO 与设备任务队列分别存在；maker 的本机保存／发布、import 与 profile switch 也不能据设备任务的 AbortController 视为可撤销。

`connectCurrentDevice` 可复用当前设备元数据，按容器及产品／BLE／dongle ID 选择，再构建新对象。`connectRZDevice` 在 connectDevice 重试完成之前就替换当前 `window.rzDevice`；所选完整所有者没有连接代次检查或旧任务 abort。Importer 的 `te`、引擎的 `S` 和 setup 防重入标记职责不同，不能作为迟到结果屏障。已有 engine 及其设备引用的复用仍遵循前述独立 guard。这里记录的是已读完整所有者的实际行为；外部通知 producer 尚未展开的部分继续作为依赖，不把它们判为不存在。

产品 onLoad 注册 USB disconnect、BLE remove／disconnect 与 `exitApp` 消费者。移除事件只核容器和当前产品 ID，遵守 ignore 标记，然后重置 runtime metadata 门并调用 cleanUpDevice，没有 await、usbInstanceId 或连接代次检查。`exitApp` 另按 appName、runningApps 和实际 feature 条件决定忽略或清理，不能把每个 exitApp 都视为已经执行关闭。

完整 cleanUpDevice 先执行共享清理并捕获当前设备，再尝试源码规定的退出模式，缺显式配置时为 `[0,0]`；随后关闭设备 BroadcastChannel／移除其事件 listener、清理共享状态、请求 Mapping 移除设备、调用可选 terminate／handleBeforeUnload、等待已有 feature shutdown，最后关闭 MW 窗口。可选 terminate／handleBeforeUnload 没有 await；soft reload 分支跳过窗口关闭。模式失败独立记录，主清理 catch 删除 runtime metadata；入口 try 之前的失败也不能记为完整清理成功。beforeunload 的广播清理只移除 listener，显式 close 才另关闭 channel；两者都不撤销已启动 Promise 或排空设备任务。

Mapping 的 `removeUsbDevice` 请求与 `hid.removeDevice` 是不同调用链。共享当前 Host 构造器把相应厂商的 `nodeUSBEvent-remove` 交给 HID remove，按产品／容器及可选 usbInstanceId 关闭句柄、取消 mutex 并删除 map；窗口 tabDestroyed 仅释放匹配 URL 的 mutex，所选 commonDestroyed 回调只记录日志。既有 17 个原生函数已核对请求复制、deferred 完成、CancelIo／句柄关闭／指针清空，但显式 close 没有 join feature worker 或证明队列／deferred callback 排空。协议 finally 的 mutex 释放也不等于设备完成或旧连接结果失效。

13 项定向静态测试覆盖实际调用方、产品 override、取消与 settlement、旧连接替换、退出顺序、广播清理、共享 HID 边界、当前输入变化及失败发布。Mapping 移除内部的产品接线已由下节接续；USB／BLE producer 与各 feature shutdown 仍保留明确依赖。独立 Rust、生产 UI／后端、实际 DLL／设备并发移除／重连与恢复对照、窗口验收仍分别未完成或未验证。本批未执行厂商 JS、DLL、设备或应用，新增临时资源为零。

### 2636 Mapping 设备移除与最后窗口引用

[device_removal_2636_chain](../../../.analysis/results/frontend/gamepad-mw-audit.json#/device_removal_2636_chain) 将上一节的实际 `Cn／Mn` 及 facade `removeUsbDevice [606027,606660)` 接到当前 Host 4.0.896／Mapping 1.3.17.2。复核入口为 `audit-family-function --family gamepad --function mw --query gamepad-2636-device-removal-chain --product 2636`。本批保留 39 个完整 AST owner、34 个角色，复用并核对 19 个既有原生函数和 6 个实际 PE 虚槽；共享清理复用[既有本机收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_input_shutdown_chain`，USB 构造／析构另精确引用已有 provider 与 hardware-notification 证据，不借用其他产品的通知资格。

当前 DeviceInfo 声明 PID 2636、VID 5426、接口 1，未声明独立 BLE／dongle PID。连接仍按当前源码核对实际容器和候选产品；`controllerDevice.isWireless` 的界面特征不改变 Mapping 所有者类型。`Mn` 先捕获当前设备，并经实际 `X7` helper 取得 Mapping 目标：非 dual-link 或已是 primary 时保留原对象，仅在真实 dual-link 门槛及 primary 不同的条件下查主设备。退出模式作用于先前捕获的当前对象，随后移除取得的目标 descriptor；`Cn` 则直接移除自己捕获的 `e.device`。调用方都没有把 remove 返回的 boolean 作为完整设备清理确认，模式返回也不是 remove 应答。

真实链为 `facade → window.top.apiElectron → packed preload mappingEngineAction → main io.callDLL → win Mapping Host.removeUsbDevice`。payload 保留真实 descriptor。Host 以 `product-container-instance` 建 key，先精确查 map，再按源码的产品／容器及可选实例规则 fallback；取得 fallback 项不会改写稍后使用的请求 key。`createAPIData` 保留 VID、PID、container、instance，并取当前 map 的 guid；不存在或不适用的值不能补成另一设备身份。

共享设备的引用身份是**调用窗口 URL**。add 已就绪设备时仅在 URL 不存在时追加；remove 在 `mutexUsbDevice` 内先滤掉当前 URL。仍有其他 URL 时返回本地 true，不调用原生，也不关闭其他窗口持有的 Mapping 设备；URL 列表为空才保留 completion callback 并进入 native remove。回调 ABI 是 `void(bool,string,string)`，所选 Promise 没有 timeout／AbortSignal。未初始化、参数不符和未找到设备分别返回当前失败结果。

本产品的实际旧式 `addUsbDevice` 在原生明确设置 `type:"usb"`，manager 因而选择 USB 构造与主虚表 `0x2e8d38`。最后引用移除走 `removeUsbDevice 0x28140 → engine+200／0x6630／0x6800 → manager+8／0x57ac0`，复用已核待执行／运行任务取消请求和 USB cleanup `+64／0x53090`。共享 manager 在完成回调前删除并析构其拥有的设备；实际 USB deleting destructor 为 `0xa8fe0 → 0xa896c → 0x656ca`，再沿已核设备线程 join 释放输入状态。不能把共享 BLE deleting destructor `0x54000` 当作 2636 实际 USB 分支，也不重新分析已经完成的共享清理。

原生无效身份、线程、manager 或设备的失败继续保留。最后 URL 的 native completion 为 false 或 FFI 同步抛错后，Host 仍删除请求 key，并不恢复 URL；调用方不检查 false，不能据此声称原生清理成功。普通 remove 持有 mutex 到 completion；已核 `removeUsbDeviceImp` 和 USB／BLE 移除消费者的所有权、锁及错误行为分别复用共享收据。物理移除消费者可以不经过 URL 最后引用门槛，但这些消费者不证明全部通知 producer 已展开。

所选移除与 map 删除没有核对当前连接代次／同一 guid，已有延迟或已发布回调也不会因 URL／map 删除自动失效。局部析构、线程 join 和成功 completion 的已核顺序，只证明相应所有者释放，不证明未来 delayed task 或已入 main-runner／FFI／Electron／JS 队列全部排空。14 项定向静态测试覆盖当前身份、完整桥接、URL 门槛、false／异常、fallback key、实际 USB 虚槽、共享释放边界及输入变化／失败发布；未执行厂商代码，新增捕获、分析会话和临时资源均为零。通知 producer、各 feature shutdown、既有共享队列边界、独立 Rust／UI 接线和实际 DLL／设备对照仍在各自现有任务中保留。

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

本节保留已有实现与纯／模拟验证事实；其中旧 2636 部署偏移只说明原实现的取证范围，当前 Import 编排以[当前收据](../../../.analysis/results/frontend/gamepad-mw-audit.json#/profile_import_chain)为准。部分失败后的默认值保留、恢复 setter 与发布边界仍须在后续迁移中逐项对齐，不能由本批静态闭合提升已有实现的完成范围。

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

## 共享 Mapping 物理输入通知

维护入口 `audit-host-local --query mapping-physical-input-decode` 将结果写入[现有本机链收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_physical_input_decode`。本节只覆盖当前 4.0.896 所含 Mapping 1.3.17.2 的 Windows filter-driver 键盘／鼠标通知解码；它是共享输入链，不能据此宣称某款手柄采用相同报文。来源、实际字节身份、完整函数范围、伪代码／汇编及真实虚表槽由[维护 collector](../../../tools/host.py)复核，注册和通知消费者复用既有 `mapping_input_provider`，不重新捕获整库。

`AC900 → 11A96E → 11A066` 根据当前真实 PID、container GUID 和已发现的驱动接口路径选择 endpoint；接口 GUID 为 `e3be005d-d130-4910-88ff-09ae02f680e9`。`ACCEE` 建立拥有该 endpoint 的 driver thread，观察者是 Mapping input device 的 `base+120` 子对象。`117500 → 110040 → 110450` 创建 DriverImpl，以 `OPEN_EXISTING/FILE_FLAG_OVERLAPPED` 打开实际路径并提交 `0x88883018`，输入为空、输出 304 字节。两个 OVERLAPPED 位于 DriverImpl `+536/+872`，对应输出 `+568/+904`；异步完成接口为 `base+8`。打开句柄及成功提交 I/O 不是实测输入。

`114AD0` 在成功完成一个接收后，先对另一缓冲区重新提交相同 IOCTL，再解析已完成帧。帧偏移 `+8` 的事件码用掩码 `0x16` 仅接受 1、2、4；偏移 `+16` 的输入类型 1／2 分别选择键盘／鼠标。事件码 4 才进入普通输入通知，2 为 hook、1 为 redirect；1／类型 4 另有两字节回调，不能混成统一按键通知。此 owner 未显式检查完成回调的返回字节数，现有静态布局不能证明真实驱动每次都返回完整帧。

| 通知字段 | 键盘 `1150BD` | 鼠标 `11513F` |
| --- | --- | --- |
| 帧 `+20` | 可选 `unitId` | 可选 `unitId` |
| 帧 `+22` | 16 位 `scancode` | 本对象不投影此字段 |
| 帧 `+24` | 16 位 `flag` | 16 位 `id` |
| 帧 `+26` | 本对象不投影此字段 | 有符号 16 位 `data`，先限于 `[-120,120]` |
| JSON 所有者 | `10D010`，`type=keyboard` | `10DC90`，`type=mouse`；`data` 按无符号 16 位输出 |

因此鼠标负滚轮值在 JSON 中表现为限幅后字的无符号值，不能直接替换为负数 JSON。两条路径均通过实际对象池取得并重置对象；`extraInformation` 被清零，解码器没有复制驱动该字段。`unitId` 的是否存在由 DriverImpl `+112` 的 dual-link mode 决定，与数值是否为零无关。`modifiers` 来自 `11C930` 的真实 OS `GetKeyState`，分别查询左右 Alt／Ctrl／Shift／Win，不来自该输入帧，也不能从 scancode 推断 HID usage、产品能力或固件键身份。

普通通知经 driver observer `+16 → 1169F0`，在无输入或 driver thread `+153` 关闭门时拒绝。`11924A` 持有输入引用并排队，`1191E0 → C2090` 通过实际观察者虚槽 `+40` 将同一输入交给已核的 `AA170`，后续由 `802DC` 按注册门和当前对象 JSON serializer 排入既有 type1 回调链。完整 `C2090` 汇编保留了该参数，不能采用其不完整的 guard-call 伪代码原型。任务销毁 `1192D0/1192F6` 释放输入和所有者引用。

失败 I/O 不进入上述成功帧路径；`110FB4` 将 OS 成功或 `ERROR_IO_PENDING=997` 记录为已接受，其余错误真实上报。取消链 `110980` 设置关闭状态、对实际句柄 `CancelIo` 并关闭，`1109D8` 仅在 pending I/O 为零时发出清理完成；driver thread 关闭请求及 `117A40` 的输入门单独保留。这不能证明已排队的 owner-runner／FFI／JS 消息全部排空。现有 Lighting `LedInputMap` 与 region lookup 继续复用 `lighting_input_lifecycle`，本批没有增加产品映射变体。

真实 kernel driver 如何产生这些帧、全部 PID／固件／传输资格、analog／BLE／external／OS-hook、其余 hook／redirect 执行和连接代次取消仍待闭合。[专属静态测试](../../../tools/test_host.py)核对来源、完整捕获、字段／ABI、错误反例和 JSON 往返；本批未执行官方 DLL、驱动或设备，未修改 Rust／UI，实际对照及运行时验收保持未验证。

## 共享 Mapping external 原始硬件输入

维护入口 `audit-host-local --query mapping-external-input-routing` 将本批结果写入[现有本机链收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_external_input_routing`。范围为当前官方 Host 4.0.896 所属 Mapping 1.3.17.2 的 external 原始 HID 输入、硬件事件交付及清理；[collector](../../../tools/host.py)复核原件、完整函数、真实指令／虚槽及当前 Host AST，不重新捕获全库。本节是共享链证据，不证明某款手柄或其他产品实际选择了 external。

`57380` 仅在配置 `type=external` 时创建 `6319C`。其硬件观察者是 `base+128`，表 `2E6668`；注册接口则是 `base+136`，表 `2E6698`。配置解析 `DB242` 的内部 `connectHid/connectFilterDriver` 初值均为 true，再按实际配置解析覆盖；这些是原版内部配置默认值，不能当设备状态或产品已选择该分支。external 初始化 `63590` 仅在 `connectHid` 门允许时，经 `7D260 → AB530 → 11A982` 按实际 PID／container 枚举 HID endpoint；共同的 Windows 身份匹配沿用 `mapping_physical_input_decode` 的 `11A066`，不沿用它的键鼠帧布局。

`ABD80 → AC590 → 123C00/123FE4 → 124130` 为实际 endpoint 建立硬件事件线程，`1234B8` 以 `GENERIC_READ/OPEN_EXISTING/FILE_FLAG_OVERLAPPED` 打开真实路径。`1237D8` 的单次 `ReadFile` 容量为 **100 字节**，两个 `OVERLAPPED` 位于 endpoint `+72/+104`，缓冲区位于 `+136/+237`。完成者 `123AE0` 的 this 为 endpoint `+16`；成功时先重挂另一缓冲区，再将真实非零完成长度经 R9 交给 `1249C0`。后者先核对 endpoint 所有权，再按真实长度复制；不能把 100 字节容量视为固定报文长度，也不能解释成普通键鼠的 304 字节 filter-driver 帧。

`6A330` 复制并排队原始报文，`9D510` 调整回 `base+128`，`6A470` 通过真实虚槽 `+24` 进入 external 的 `64A60`。该 handler 取得当前 tick、丢弃空报文；存在 DeviceData 时会用 `raw[0]` 查询 `E5DEC`，但**不消费分类结果**，随后将整段原始报文传给 `7F17E`。它不剥离前缀或 report ID，也不因此进入 analog 映射执行器。本批尚未确定这些字节的产品内部协议布局。

硬件注册 `6B500` 设置 `base+640`，可选批处理门位于 `+656`，传入 interval 按 uint32 乘 1000 后存于 `+660`；当前 tick 的实际单位不由此推断。`6BB20` 分别持有普通回调 `+648` 与批回调 `+704`。`7F17E` 需全局启用、已注册且有普通回调；普通路径复制 callback／报文／tick 后交给既有 `9E1D0/18DB0`。`2B770` 按无符号字节序列化为数组，以真实四参 ABI `void(const char*, int32, const char*, uint64)` 发出 **type0**。批路径 `7F73E` 保留每个原始报文，达到 10001 条时强制 flush；`7F960` 移走队列后重新核注册等门，`2B8B0` 按到达顺序发出嵌套字节数组，携带 flush 时取得的 tick。

当前 Host `createCbHardwareEvent` 只接收 type0，按真实产品／container／instance 解析当前 map，核对订阅 URL 与 webcontents 存活，再发送 `hardwareevent`，其中 `data.buffer=JSON.parse(payload)`，保留普通数组或批数组的形状。首位订阅者在原生注册成功后增加 URL 并 await callback 设置，但 callback 设置失败不会替换原先的注册结果。后续订阅复用当前模式／interval，不能由 Host 返回成功推断已有真实输入。产品前端如何解析、消费这些 bytes 及其实际 external 配置仍须沿挂载调用取证。

注销 `6B720` 清除注册门及批处理门，保留 callback 与尚存队列；Host 先移除 URL，最后订阅者才请求 native 注销，失败时不恢复 URL。后续批 flush 会在门关闭时释放已移出的报文，但已排队的普通／批回调仍可运行。端点错误 1167 经 `123880` 有界重连，先 `CancelIo/close` 再重开；计数达到 60 后不再尝试。external 清理 `64000 → AC730 → 1247EE → 124950` 对实际端点调用 `123AA0` 取消／关闭，再由 `AC860/123E2A` 释放并 join 硬件线程。端点析构 `123650` 还明确诊断未经 shutdown 的销毁；不能用析构、join 或注销完成推断全部 owner-runner／FFI／JS 队列及旧连接结果已经排空。

`notifyInputForExternalDevice` 是另一条接受 `keyInput` 并 JSON 编码的注入 API，与上述原始 HID／type0 路径分开；其导出存在不证明有实际产品生产者。本批保留该完整 Host owner，只明确这条边界，未扩展 native JSON 映射执行链。后续继续实际 external 产品配置／报文消费者、独立 JSON 注入、BLE／analog／OS-hook 及连接代次取消。[专属静态测试](../../../tools/test_host.py)覆盖来源变化、缺失函数／真实生产边、错误完成长度／前缀／虚槽／事件类型和批阈值、Host 消费变化及 JSON 往返。本批未修改 Rust／UI，未执行官方 DLL、HID 或设备，实际对照与运行时验收保持未验证。

## 共享 Mapping BLE class 1 与 analog stick 输入

维护入口 `audit-host-local --query mapping-ble-analog-stick-routing` 将结果写入[现有本机链收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_ble_analog_stick_routing`。本节只覆盖当前 Host 4.0.896 所属 Mapping 1.3.17.2 的 BLE class 1 报文、输入 ID 251／252 阈值边缘及实际映射消费者；原件、完整 Host AST、原生函数与真实 PE 表由[collector](../../../tools/host.py)复核。本节不证明某款手柄或其他产品实际选择这条链。

`57380` 仅对精确配置 `type=ble` 创建 `52158`；硬件观察者为 `base+128 / 2E4FD8`，其 `+24` 虚槽真实指向 `53C70`。初始化 `52540` 仅在 `connectHid` 字节 `base+177` 允许时排 `7D260`，随后接入上一节已核的 `AB530 → ABD80 → AC590 → 123C00` HID provider；只在这条已证调用关系内复用其收包与清理，不由 BLE 名称猜测 report 容量或套用 304 字节键鼠帧。实际产品的 HID descriptor、报文 ID、固件和传输资格仍待产品挂载证据。

`53C70` 丢弃长度 0／1 的报文，存在 DeviceData 时用 **`raw[1]`** 查询已发布 `reportIDs`。真实 switch 表 `2E55F8` 的 class 1 先复制 `raw[1..end)`，调用 `79BDE` 后释放副本；class 3、7／8 保留原报文，不能概括为所有 BLE 报文都剥离前缀。缺 DeviceData 走原始硬件事件，缺失或未就绪的已发布分类返回 8；class 1 本身不同时转发 `hardwareevent`。

`79BDE` 在副本长度至少 4 时，从 `copied[1]` 开始解三元组：`inputID:u8 + sample高字节 + sample低字节`，ID 0 终止，后续三元组不完整则停止；增量索引明确缩为 `uint8`。因此首个 ID 在原报文偏移 2，sample 到 stick 消费者按 signed16 解释。全局 Mapping 启用且未被 `base+992` 排除时，只有 251／252 进入 `7E8D2`，其他 ID 进入普通 analog key 的 `7DD9C`，其样本／映射／通知见下一专题，完整输出与取消仍待取证。缺失项清理扫描的是 **`base+1488` previous-sample 树**，可对其中实际存在的 251／252 调用零值释放；阈值 held 树在 **`base+1896`**。两树不同，不能由报文省略推断全部 held stick 已释放；当前 sample 循环受 Mapping 门控制，缺失项清理在该门外。

`E6760` 只返回引擎就绪时的已发布 stick 配置；`EC172` 将可选 `analogStick.threshold` 限定为 signed16 的 1..32766。`7E8D2` 缺已发布配置时内部采用 20000，仅属映射计算阈值。样本幅值按 unsigned16 比较，`-32768` 幅值为 32768，等于阈值也激活；held 身份只按输入 ID。新插入发 make／flag 0，已持有不重复，低于阈值且原 held 存在才发 break／flag 1，直接反向但仍超阈值不发新边缘。251 正／非正分别为 right／left，252 正／非正分别为 up／down；break 使用当前样本符号，零值不会自动沿用先前方向。

真实 type 3 对象由 pool 重置或分配，`FC956` 写 axis／flag 到字节 56／57。BLE 映射 delegate 为 `base+120 / 2E4F88`，`+24 → 66D10` 查映射，成功后 `+32 → 69530` 消费：`677BA` 跟踪输入，随后依次尝试 `69728`、`68D0C`，否则 `794E8` 按实际映射／全局快捷键状态进入 `AD0CC` 或 `AEAFE`。本批闭合到这些真实消费者与引用释放，不扩大为全部输出 executor 已完成。`79BDE / 7E8D2` 没有直接调用 `93E18 / 802DC`；不能直接把 stick 边缘接成 type 1 `inputnotified`。

当前 Host `createCbHardwareEvent` 仅消费 type 0 并解析 bytes JSON；`createCbInputNotification` 仅消费 type 1 并原样交付序列化 input。两者不会构造 class 1 stick 事件。`handleBLEEvent` 在此忽略 new／update；允许的厂商断开／移除事件进入 `handleBLEEvent_removeDevice`，等待 native `removeUsbDevice` 完成后，即使返回 false 也删除 callback context 和当前 map。原生 `53090` 排入真实 `AC730`，沿已核 HID 取消／关闭／provider join 释放；owned 报文、sample 树、快照、stick input、映射和阈值配置各按其所有者释放。map 删除、引用释放及 provider shutdown 均不证明 pending delegate／输出／FFI／JS 或旧连接结果已排空。实际产品资格、配置发布与选定输出、report 省略恢复及连接代次取消仍待闭合；[专属测试](../../../tools/test_host.py)核对真实字节／调用／虚槽反例与 JSON 往返。本批未执行官方 DLL／设备，Rust 独立实现、UI 接线、实际对照和运行时验收分别保持未验证。

## 共享 Mapping 普通 analog key、映射与释放

维护入口 `audit-host-local --query mapping-analog-key-routing` 将结果接入[现有本机链收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_analog_key_routing`。它从上一节已经确认的 `79BDE → 7DD9C / 7A656` 接续，只复核当前 Mapping 1.3.17.2 的普通按键、实际映射消费者、通知与局部释放；不重复捕获 BLE／HID 库，不重复 stick 阈值。当前 [collector](../../../tools/host.py)保留 46 个完整原生函数、15 个真实虚槽及 7 个完整 Host／产品 AST，核对 61 个指令锚点和 55 条实际调用边。

`7DD9C` 按 **unsigned16** 消费 sample。`E6204` 缺配置时的内部 packed 值为 `AA00AA00`，其中有效字节为零；只有已就绪的 published `analogData+72` 存在最大值时才限幅，不能将其当作设备默认值或实测状态。`E62F8` 先按真实 `inputID:u8` 取已发布 input；缺失时，`E5DB0` 的实际 special-report class `<2 / ==2` 分别选择 PE 表 `2F0C30 / 2F1030`，其他 class 不构造输入。两表各有 256 个 packed `scancode:u16 / flag:u16`，非零项分别为 111／20；scancode 为零直接返回空，`10C755` 对其余项从真实 pool 构造 type 2，写入字段 `+56/+58`，随后按调用者写 modifiers。

按键状态有不同所有者，不能混作一棵 held 树：`+1136` 保存 input ID 到 input，`+1176` 保存 stateless identity，`+1472` 保存前一 unsigned16 sample／方向，`+1512` 保存 pending mapping。初次 sample 插入从 level 0 开始并发 forced sample event；相同 sample 不再进入后续映射。变化时，`E63AC` 按真实 ID／当前 mode 先查 active-profile actuation，再取 published 默认 actuation。上升调用 input 虚槽 `+88` make、使用 actuation `+12`；下降调用 `+96` break、使用 `+14`。有效 max 存在时用于满量程判断；否则只有 version 2 使用 65535，其余使用 255。缺 input／actuation 的错误路径不表示输出成功。

`9254C` 的旁路通知门按实际 input type 区分：type 2 对应 `+744`，type 35 对应 `+746`，或 input ID 已在 `+1656` 树中。`925D0` 对 make 核对真实零值特例与 `previous <= actuation <= current`，仅新插入才通知；break 经 `93FDB` 核对零值、正常跨阈或满量程特例，仅实际移除才继续 `69AAE → 802DC`。未进入旁路则通过 `9071A` 查实际 mapping；存在 rapid-trigger 配置时，其有效字段 `+14/+18/+21` 必须成立才进入 `92748`，违反断言走原生失败路径；**只有缺 rapid 配置才选择普通 `9307E`**。

`92748 / 9307E` 按真实 output type 选择 joystick、Xbox、type 7／15／26／33／38 等消费者；普通分支另有 type 19 → `88B60`、type 41 → `8D952`。正常边缘经 `9474E / 93FDB`，rapid-trigger 经 `9760C` 的实际 extrema／state 树；选中 generic 分支才在 sample event 与 `93E18` 后进入 `794E8`。本节核实这些消费者与引用边界，映射优先级及每个实际选中 executor 的 OS／设备结果仍待后续沿真实分支闭合，不因共享 dispatcher 扩展全部输出。

| 通知 | 实际原生路径 | 当前 Host 消费 |
| --- | --- | --- |
| sample event，type 6 | `8E764 → 80888 / 80D70 → 81300`，单项／批项各自序列化 | `analogKeyEventList` → `analogKeyEvent`，保留 `jsonEvent` |
| redirect，type 3 | 旁路 `69AAE` | redirect 的实际订阅 URL 列表 → `inputredirect` |
| input notification，type 1 | 旁路 `802DC` 或普通 analog `93E18` | `inputNotificationList` → `inputnotified` |

sample event 的单项门为 callback `+392` 与 `+352` 的已注册 ID，使用计数或 force；批模式另需 `+400` 与 callback `+440`，逐项保存 tick，队列达到 10001 条或到实际 interval 才 flush。`93E18` 使用注册门 `+712`／callback `+720` 和独立的 `+1616` make／break 去重树，随后进入 `8DA4E / 8E0D4`。三条事件的类型、负载、订阅者和状态来源均分开；sample event 不能证明映射或写入成功，tick 单位仍未核实。

缺失按键由 `7A656` 取得或构造真实 input，经 `1009A2` clone 后设置 break，读取 break actuation，以 level 0 进入同一旁路／普通／rapid 链。它仅在相应完成路径按 stateless identity 移除 `+1512`，不能概括为清空所有 held／sample／retained 树。局部 input、mapping、actuation 和 rapid-config 引用有实际释放。SnapTap 回调 `A52C0 / A5400 / A5510` 的 48 字节环境保存的是指向 live 值的指针；真实表的 `A53D0` 只复制三个 16 字节块，`9D320` 只释放环境，不能称为独立拥有 input 深拷贝。`99AAC / 99DBE / 9A20E / 9A692` 的调用、延迟、恢复、取消与排空继续保留未闭合。

产品来源独立核对当前 PID756 MW 的 acquisition、main 原字节和[双轮部署收据](../../../.analysis/results/frontend-revalidation/3445d9b6321b3aead25aae60d308db62eaa5b55750f3d3234182d09d21fe2753.json)：完整默认数据 producer 与词法绑定明确写 report ID 11 为 `analogKeyboardV2ReportID`、4 为 `razerKeyReportID`，默认 actuation 为 19656／1638、`turnOffAnalogToKey=true`，main 实际挂载 `useFeature("analogV3","yes")`。这些是当前产品源码事实；还需接续实际 app-engine 配置 writer、成功 native publication、真实 HID report／传输及固件资格，不能把 Host 版本、默认 report 标签或挂载功能自动推广为全部 live 分支已确认。

[专属静态测试](../../../tools/test_host.py)已通过：来源变化、缺完整函数、错误 sample 宽度／max 有效位／make-break 方向／树偏移、实际调用边、PE 表与虚槽、三类 Host 事件、产品词法绑定、失败或不完整双轮收据均有反例，JSON 往返与输入快照复核通过。本批未创建临时捕获或数据库，无待清理的新中间物；未修改 Rust／UI、未执行官方 JS／DLL／设备。共享订阅与设备移除的接续结论见下一节；完整映射优先级、实际 executor 和产品资格仍待取证，独立实现、UI／后端接线、实际 DLL 对照与运行时验收分别保持未验证。

## 共享 Mapping 输入订阅、设备移除与回调生命周期

维护入口 `audit-host-local --query mapping-input-shutdown-chain` 接续上一节的共享取消项，结果归入[现有本机链收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_input_shutdown_chain`。[collector](../../../tools/host.py)复用当前 Host 4.0.896／Mapping 1.3.17.2 的既有完整捕获，核对 80 个原生函数、9 个真实虚槽、16 个完整 Host AST、44 个指令锚点和 79 条实际引用边；本节不增加产品资格或输出 executor 范围。

| 边界 | 实际行为 | 不能据此确认的行为 |
| --- | --- | --- |
| 全部 sample 注销 `72D30` | 清注册字节 `+400` 和 ID 表 `+352`，再排完成回调 | 不清 interval `+401`、timer `+402`、队列 `+416` 或 callback `+392/+440` |
| interval flush `81300` | 移走整队列；有 callback 且批次非空时投递；清 timer 字节 | 不重新检查注册字节，注销后的旧批次仍可能投递 |
| 通用 callback 清理 `786E0` | 仅重置设备事件 callback `+208` | 不重置 sample callback |
| manager 移除完成 `57E60` | 先删除身份表项并同步析构设备，再报告成功 | 不证明已投往其他 runner／FFI／JS 的事件排空 |
| 输入对象析构 `656CA` | 等设备线程结束后释放实际 held／sample／pending、队列与 callback | 不证明所有未来 delayed task 已执行／销毁，亦不等同设备或 OS 输出恢复 |

`72580` 注册全部事件时写 mode word `1`，`72960` 注册 interval 时写 `257`，并将 `1000 * interval` 的 32 位结果放入 `+404`；已注册分支直接成功，不切换模式。单项 `73530 / 73BA0` 只维护实际 input ID 的 skip／counter／last 表项；注销单项不移除 Host URL。这里的缩放不能证明 tick 的实际单位。

`80D70` 对新 sample 检查注册门及 interval callback，将自有字符串排入 `+416`；达到 10001 项立即 flush，否则由 `+402` 防止重复安排 delayed `81300`。delayed task 的 `9C870 / 627F0` 使用受检查的 unretained device 指针，没有保留 input 对象引用，也未返回显式取消句柄。单项 `9E1E2` 复制 callback 和字符串，批项 `A3F9C` 保留 callback 并移入列表；`9E1D0 → 18DB0` 调用已复制的 callback，不重查订阅。`9E2D0 / 9E2F6 / A4080` 释放捕获值，不能把注销成功当作这些事件已失效。

真实移除从 `removeDevice / removeUsbDevice` 到 engine `+200 → 6630 → 6800`、manager `+8 → 57AC0`。它按实际身份请求清理相关待执行／运行任务，复制设备身份，并把 `57E60` 接到实际 cleanup `+64`。所选 BLE `53090` 先打断等待，再串行执行通用 callback、本地存储、适用的共享 hook、keyboard executor、硬件／驱动线程、DeviceData 与结束回调；具体硬件线程复用已有收据。`78DFC` 清空并释放 `+256` 的 DeviceData，`54930` 标记 cleanup state 4 后投完成回调。

`57E60 → 6269E → 62708 → 6127C / 60DFA → 5F917` 先移除 manager `+352` 表项，清掉其独占 device 指针并调用真实虚槽 `+0`。BLE 的 `54000 → 52394 → 656CA` 先切回共享 InputDevice 虚表，再通过 `1B8E40` 执行 StopSoon、`QuitWhenIdle` 和线程 HANDLE 等待／关闭。此时 `65C90` 的实际 base `+72` 目标 `77C40` 是空操作 thunk，不能当作合成 break。线程结束后才释放 stick-held `+1896`、bypass-retained `+1656`、normal-notification `+1616`、pending mapping `+1512`、key sample `+1472`，以及 callback `+440/+392`、队列 `+416`、注册表 `+352`；pending／retained 节点按各自真实引用释放。manager 成功回调发生在该同步析构返回之后。

当前 Host 先 await 设置 sample callback，却不检查结果就置 `bSetAnalogKeyEventCallback=true`；成功注册才加入 URL。全部注销先过滤当前 URL，最后一个 URL 才请求 native，失败不恢复列表。设备移除的相应 Host owner await native 后删除 map，未按返回的 `result` 保留失败项；USB 路径的 mutex 在 `finally` 释放，BLE 移除及内部移除函数没有自己的 mutex。

重连创建的新表项含新 guid、空 URL 列表与 false callback 标志；type 6 消费者到达时按事件身份查**当前** map，允许源码中的 PID／container／可选 instance 回退，再核 URL 与 webcontents。该完整消费者不核 guid 或连接代次，旧连接迟到事件可能投给同身份且已重新订阅的新表项。`callDLL` 的 shutdown 只等待自身完成、删除该 completion 并置 `isInit=false`，没有清空设备表或所有 pending completion；这些 Promise 中也没有 timeout／取消入口。

[8 项专属静态测试](../../../tools/test_host.py)通过，覆盖来源变化、缺完整 flush、错误注册／timer／callback／状态偏移、完整函数边界、实际 retain／erase／join 引用边、身份／cleanup／析构虚槽混淆和 Host URL／返回值／当前身份消费者变化；JSON 往返与输入快照复核通过。未新增临时捕获、数据库、解压或日志，保留的是后续可复核依赖的正式原件与收据。

实际 runner／sequence-manager 的 delayed task 销毁与 FFI callback 所有权接续见下一节；本节原有收据仍只证明上述订阅／移除边界。实际选中 output／SnapTap 的恢复与取消，以及 PID756 writer／publication／HID／固件资格继续归原有任务。本节未修改 Rust／UI，未执行官方 JS／DLL／设备，完整链、独立实现、接线、实际对照与运行时验收仍分别未完成或未验证。

## 设备延迟任务销毁与独立 FFI 回调队列

维护入口 `audit-host-local --query mapping-delayed-task-lifecycle` 接续上一节的 runner 缺项，结果归入[现有本机链收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_delayed_task_lifecycle`。[collector](../../../tools/host.py)复用当前 Mapping 1.3.17.2 捕获，并核对同一 Host 4.0.896 包内 ffi-napi-rz 1.0.11 的实际 JS 与 unpacked addon；所选完整边界包括 65 个 Mapping 函数、18 个 addon 函数、13 个 PE 虚槽及 13 个源码 AST。上一节的 80 函数订阅／manager 移除收据继续作为依赖，不重复扩展其产品或输出范围。

| 所有者 | 已确认边界 | 仍需区分的状态 |
| --- | --- | --- |
| device `+16 / +24` | `546E8` 保留构造线程当前 default runner，并另建 type 3 设备线程 | 当前 runner 与设备线程 runner 分属不同所有权 |
| thread delegate `+64` | `1B92F0` 在 RunLoop 返回后、线程入口返回前销毁 delegate | 设备 join 不等于独立 main runner 或 JS 事件循环排空 |
| queue handle `341FD0 +16` | `26BBA0 → 26B880` 按 queue `+82` 判断并注销；manager 失效时直接析构 queue | queue handle 引用释放与 manager 独占 queue 释放分别有路径 |
| task queue `260FC0` | 关闭 poster、标记 shutdown，移走并销毁 incoming／work／delayed task | 未执行任务的销毁不会执行任务 body |
| addon 跨线程 work | `A0F0` 排队并等待，`93E0` 调 JS 后 signal，等待方释放 work | work 保存原参数／closure 指针，没有复制其对象或按设备代次取消 |
| callback／environment | buffer finalizer 释放 JS 引用与 closure；环境关闭另走 `uv_close` | finalizer 和环境队列存储析构均不证明在途 producer 已排空 |

真实 runner 从 delegate `2FFC40 +8 → 1B96D0 → 2068A0` 到 controller `341160 +120 → 25B3E0`，取得锁保护的 `controller +440`。该值由 delegate 构造时从 queue handle `+56` 取出，经 manager `+96`／controller `+104 → 25B300` 保存并保留引用。queue handle 独占实际 queue `+24`；manager `+216 → 202E60 → 260760` 创建该 queue，并在 task poster `+32` 留下真实 queue 指针。

runner 的实际表 `341450 +0 → 260150` 移入 callback 并持有 runner 引用；`25FC90` 获得 poster lease 后才取 queue 投递，拒绝投递时由 `267E40 → 1CBD20` 释放 callback。`25FD80` 按 delay 分流：即时任务进入 queue `+40` incoming；同线程延迟任务进入 `+192` heap；跨线程延迟任务先由 `2624B0` 的即时 wrapper 持有完整 168 字节任务，执行时 `2629C0` 移入 heap，未执行时 `266350` 释放嵌套任务。

delegate 析构 `1B9650` 先减 queue handle 引用，再销毁 sequence manager。handle 注销 `26BA40 → 202FD0` 以及 manager 析构 `206960 → 202050` 都会到 `260FC0`：关闭 poster 并清 queue 指针，置 shutdown `+82`，移走 incoming 与 delayed heap，销毁两个 work queue 和保留任务。任务析构 `267F00 → 26DD10 → 1CBD20` 释放弱句柄、runner 与 bind；bind 引用归零调用 **`+16` disposer**。上一节已核 sample bind 的 disposer `627F0` 只释放捕获值，不调用 `81300` flush。直接 queue 析构的 `260C20 / 265C80 / 260DA0` 也覆盖 delayed／work／incoming 释放。因此现已确认设备线程退出会销毁尚未执行的延迟 sample 任务；没有把销毁记成执行或设备输出恢复。

`1350` 在 engine `+56` 创建 type 1 `MappingEngineMainThread`；设备基类析构 `547E0` 只销毁自己的设备线程并减 `+16` 当前 runner 引用。上一节已投 main runner 的事件拥有 callback／payload，native `3E4C0 / 3E520` 只检查 engine 全局存活门。设备移除与 join 没有清空独立 main runner，也没有为 Host 的同身份重连消费者增加代次检查。

同包 `ffi.js` 实际导出 `CanUseAnneCallback() = true`，当前 Host 据此创建并保留 `cbAnalogKeyEvent` buffer。`callback.js` 保留 CIF，Anne 参数直接进入用户 callback，异常走现有 error callback。addon `6D80` 记录 JS 线程 ID，建立环境队列 `+64`、计数 `+96` 和 `uv_async +104`；`94C0` 创建 trampoline `A0F0`，保留 error／callback 引用 `+56/+80`、环境 `+120` 及 Anne 标志 `+128`。同线程直接调用 `8C20`；跨线程分配 128 字节 work、排队、`uv_async_send`，并等待 `uv_cond_wait`。`93E0` 取出任务、执行 callback 后 signal；投递方随后销毁 condition／mutex 并释放 work。完整所选 owner 没有设备身份、连接代次或按设备取消门。

buffer finalizer `A440 → A480 → 87D0` 删除两个 NAPI 引用并释放 closure。环境 finalizer `7250 / 7400` 则申请 `uv_close`，关闭回调 `2750` 才置全局门 `3AF20` 并释放队列存储；`7B20` 的存储清理没有证明已唤醒在途等待者。loader 还保留 native resolver、`FFI_NAPI_RZ_PREBUILD` 和 prebuild 选择；这里核实的是当前安装包候选的来源、字节与调用链，实际运行时选中项仍未验证。

[9 项专属静态测试](../../../tools/test_host.py)覆盖来源变化、不完整任务捕获、runner／heap／disposer 偏移、完整析构／等待／finalizer 边界、嵌套任务与 addon 引用边、实际虚槽混淆，以及 Anne／CIF／loader／Host callback AST 变化，并复核 JSON 往返与输入快照。engine／main runner 的关闭顺序及所需 NAPI／libuv 环境交界接续见下一节；本专题不展开全部 addon 框架。新增静态捕获与临时会话的保留／清理由统一批次收据记录。本专题未改 Rust／UI，未执行官方 JS／DLL／设备；独立实现、接线、实际对照与运行时验收继续保持未完成或未验证。

## Engine 与主任务线程的完整关闭顺序

维护入口 `audit-host-local --query mapping-engine-shutdown-chain` 接续上一节的剩余项，结果归入[现有本机链收据](../../../.analysis/results/host-current/local-audit.json)的 `mapping_engine_shutdown_chain`。[collector](../../../tools/host.py)核对当前 Mapping 1.3.17.2 的 57 个完整函数、同包 ffi-napi-rz 1.0.11 的 10 个完整 addon 函数、2 个实际 PE 虚槽、15 个源码 AST 和 9 处实际 delay-import 调用。设备析构与任务队列的详细语义绑定上一节及输入关闭节的精确 section，不因同一收据新增其他专题而重复捕获。

| 时点 | 真实顺序与边界 |
| --- | --- |
| 进入关闭 | 导出 `1FED0` 经真实 engine 表 `2E0010 +16 → 2090`，锁内清 initialized `+80`，再把 `2510` 投往 engine `+56` 主线程 |
| 主线程串行阶段 | `2510` 置 state `+64=3`；先设备 manager，再 owner `96 → 304 → 312 → 144 → 200 → 224 → 288 → 160 → 88`，最后 `19C30` 置 state 4 并 signal |
| 设备 manager | `571E0` 清理设备，`57090` 请求任务取消，`56F10` 关闭 executor，`56E70` 释放 storage；设备／executor 完成并从实际表移除后才推进 |
| 主线程退出 | `2090` 等到上述 signal 后才清 thread 指针并调 `2FFBE0 +8 → 1B9510`；实际 StopSoon／QuitWhenIdle、HANDLE 等待和 delegate 销毁在外部 completion 之前 |
| 外部 completion | `1FDD0` **先调用 Host callback，再清全局 sample 门 `395760`**；不能把该门记为进入 shutdown 时已关闭 |
| 最终释放 | completion 返回后，`2090 → 17AC → 1CFFD0` 才清 engine `+48` 的 AtExit owner，并按原件处理其注册 callback |

`2510` 构造 once-continuation 的顺序与执行顺序相反：每个新 stage 持有上一条链，最后构造的 `1A620` 最先执行。`1B940` 从 bind `+32/+48/+56` 移出目标、owner 和 next 后调用目标；disposer `1B9C6` 只释放 next／owner，**不会执行 next**。各实际存在的异步 owner 持有自己的释放／next 回调，缺失 owner 则立即继续；这些调用边确认关闭阶段的串行顺序，不代替各服务或 output 的业务逆向。

manager 的 `56B40` 同样反序执行。`571E0` 把 next 放入 `+344`，遍历设备并调用实际 cleanup 虚槽 `+64`；`57E60` 先移除身份、同步析构并等待设备线程，再在设备数 `+376=0` 时投下一阶段。设备 join／delayed sample 销毁复用上一节证据。`56F10` 持有 next `+176`，由 `5EFB0` 删除已完成 executor 并在计数 `+208=0` 后继续；最后 `1A9D0` 清 engine `+152`、析构 manager，再进入余下 engine 阶段。`57090` 的任务取消请求继续由已有 output／SnapTap owner 负责，不能由请求已发出推断恢复完成。

`2090` 没有检查 shutdown post 返回值；`1BA770 → 1BA790 → 1AA5D0` 使用 `INT64_MAX`，实际进入 `WaitForSingleObject(INFINITE)`，所选等待链没有 deadline、取消或 JS loop pump。已未初始化的分支只调用 completion，并在 callback 返回后释放锁，不重复执行关闭／join。正常 signal 仅证明串行阶段已经走到 state 4；随后的 thread 析构才请求退出并等 HANDLE。`1B92F0` 在退出线程入口前销毁 delegate，所拥有队列的未执行任务按上一节的真实 disposer 释放；这不排空独立 addon、Host 或 Electron 队列。

同一官方 ASAR 的 `library.js`、`foreign_function.js` 与 `_foreign_function.js` 已核实际字节、offset、SHA 和完整 AST。Host `initDll` 的 shutdown 定义只有 `void(pointer)`、没有 async options；`Library` 因而选普通 proxy，直接调用 `bindings.ffi_call`。Host 方法写成 `async` 不代表该 DLL 调用进入线程池。`callDLL` 仅在 `cbMap` 保留本次 completion，收到 callback 后删除它并置 `isInit=false`；完整 owner 没有 timeout／取消，也没有清掉设备表或所有 callback。

addon 的实际 PE delay INT／IAT 把所选指令分别绑定到 `node.exe` 的 `napi_get_uv_event_loop`、`uv_async_init`、`napi_set_instance_data`、`napi_make_callback`、`napi_call_function`、`uv_async_send`、`uv_cond_wait`、`uv_cond_signal` 与 `uv_close`，没有只依赖分析器符号名。跨线程 `A0F0` 入队原参数／closure 指针，忽略 `uv_async_send` 返回值并无期限等待；`93E0` **先执行 JS，再 signal**，所选异常路径没有证明所有抛出都经 finally 唤醒等待者。

环境 finalizer `7400` 只在 handle type 1 时请求 `uv_close`；`2750` 的 close callback 才将全局环境门 `3AF20` 置为 1、释放队列存储，`7B20` 不 signal 或 join producer。engine shutdown 没有调用该环境 finalizer。现已静态确认 engine 与主线程关闭的准确完成／释放边界；没有证据把环境存储析构当作全局排空，也不能据此断言必然死锁。实际运行时 addon 选择、并发 environment close、未返回 JS callback、post／send 拒绝、异常退出及 producer 最终释放，仍须在显式对照入口逐项验证。

[14 项专属静态测试](../../../tools/test_host.py)检查来源／依赖 section 变化、不完整函数、逆序阶段与非执行 disposer、initialized／state／计数／回调时点、真实引用边与虚槽、完整 Host／FFI AST、实际 import 名及 IAT 错配，并验证 CLI 失败／输入变化不能升级为完成。本批复用正式捕获，新增 native capture、分析会话、临时数据库、解压、中间文件与日志均为 0；原始证据与正式收据仍是后续复核依赖。本批未修改 Rust／UI，未执行官方 JS／DLL／设备；完整产品语义、独立实现、UI／后端接线、实际对照及运行时验收分别保持未完成或未验证。

## 验证与未验证范围

自动化必须覆盖每款实际页面/固件/传输及其映射类别、取消保存、polling有线/无线、active槽、触发器顺序、slider preview、死区重复警告/prev基线、clutch、失败、旧连接和清理。Power的[页面纯测试](../../../crates/razer-pages/src/features/gamepad/gamepad_sleep_tests.rs)、[协议测试](../../../crates/razer-device/src/gamepad/controller_sleep_tests.rs)、[IPC测试](../../../crates/razer-ipc/src/controller_sleep.rs)及[Shell清理测试](../../../crates/razer-shell/src/shell/gamepad/controller_sleep.rs)覆盖分钟到秒、2636小端/2676大端、初读失败不写默认值、setter异常后GET、原始秒值差异、宏录制只存储、独立存储及pending合并、旧连接、SET前后取消、GET中取消、明确重新进入重读、可见状态与真实清理失败保留及重试。新增 `controller_initialization` 纯/模拟用例覆盖根/真实目录/字段顺序、64+16名称与等待恢复、空目录、单项失败继续、根读取失败不写默认、取消前缀、IPC身份及计划伪造、陈旧连接、301→5→300恢复、宏占用和真实终态到控制字段的消费者。清理负矩阵分别检查服务token和OS释放四种组合、未获得client、已有token但reap暂时失败、无token且已释放不忙重试、相同/不同owner，以及None/Some route解析和同path元数据富化不能绕过barrier。预期协议字节独立于Rust编码器；这些纯/模拟用例不能替代实际DLL和设备对照，也未证明完整映射转换、全部初始化/恢复分支及整个产品完成。

Cargo静态检查与纯测试由统一验证入口执行，其结果只覆盖对应版本与用例；没有执行厂商JS、主应用、无关helper或设备操作。实际DLL/设备功能结果按 [verification.md](../../../.agents/rules/verification.md) 逐项保存相同输入、双方原始输出、状态/应答、差异、原值恢复和释放收据；当前未新增该收据。真实写回、窗口/焦点/像素及不可执行的设备分支保持未验证，不能由静态检查或模拟通过扩大到整个页面或产品。
