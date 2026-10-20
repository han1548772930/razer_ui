# 磁轴按键校准

本页解释当前实际挂载校准页的 PID 740、746 与 756。740/746 使用单键三步窗口，756 使用 V2 多键页面、硬件校准事件和轴体类型流程。两套功能的模式命令、状态来源和退出顺序不同。以下先解释 740/746，再解释 756。

## 来源和产品边界

获取版本及原件哈希分别见 [740 UI](../../../.analysis/inputs/frontend/keyboard-740-current/acquisition.json)、[740 MW](../../../.analysis/inputs/frontend/keyboard-mw-740-current/acquisition.json)、[746 UI](../../../.analysis/inputs/frontend/keyboard-746-current/acquisition.json)、[746 MW](../../../.analysis/inputs/frontend/keyboard-mw-746-current/acquisition.json) 收据。

完整模块/函数体定位见 [keyboard-main-audit.json](../../../.analysis/results/frontend/keyboard-main-audit.json) 的 `local_controls_source_review`。以下区间是原件 UTF-8 字节半开区间，均已与捕获函数体核对。

| 产品 | 文件（均相对于仓库根） | 所有者 |
| --- | --- | --- |
| 740 | `.analysis/inputs/frontend/keyboard-740-current/raw/apps.razer.com/synapse/products/740/ui/static/js/9908.0c364f6c.chunk.js` | 校准窗口函数 `[268831,280096)` |
| 746 | `.analysis/inputs/frontend/keyboard-746-current/raw/apps.razer.com/synapse/products/746/ui/static/js/9908.23d31a59.chunk.js` | 模块 `63183`；校准窗口 `Ea` `[268537,279802)` |
| 740 | `.analysis/inputs/frontend/keyboard-mw-740-current/raw/apps.razer.com/synapse/products/740/mw/6737.79a0a4bc1de86a784853.js` | 模块 `61450` 的 `se` `[1034290,1040347)` |
| 746 | `.analysis/inputs/frontend/keyboard-mw-746-current/raw/apps.razer.com/synapse/products/746/mw/6737.fc465333b7989483d35d.js` | 模块 `61450` 的 `se` `[1646778,1652835)` |
| 746 | `.analysis/inputs/frontend/keyboard-mw-746-current/raw/apps.razer.com/synapse/products/746/mw/rzDevice25.a61f8ae60b5fb86f35d7.js` | 模块 `74503` 的 bottom/top setter、getter；见协议表 |
| 746 | `.analysis/inputs/frontend/keyboard-746-current/raw/apps.razer.com/synapse/products/746/ui/static/js/main.ba862d90.js` | actions 模块 `10054`；`keyboardSwitchCalibrationReducer` |
| 746 | `.analysis/inputs/frontend/keyboard-746-current/raw/apps.razer.com/synapse/products/746/ui/static/js/1891.b73e225c.chunk.js` | 输入重定向帮助模块 `90584` `[1760491,1764386)` |

740/746 校准窗口的完整函数等长；实读差异仅为 interval ref 与 DOM ref 的局部变量名称互换。两份 MW `se` 完整类函数体相同。共享语义因此建立在当前函数体上，而不是依据产品名称推断。其他键盘只有同名库符号，并不足以证明挂载了本页窗口。

## 窗口和状态

窗口有三段步骤指示：选键、底部校准/核验、顶部校准/核验；内部 step 数值 `4` 用于顶部校准后的结果阶段。界面状态分别保留选中按键、原始捕获输入、备用别名、步骤、等待中、当前步已完成，以及 reducer 的 `currentStepSuccess`、`hasError`。

只把 `customizeReducer.buttonList` 中 `inputType:"AnalogInput"` 的键用于捕获结果匹配。步骤 1 未选到具有 `defaultValue`、`buttonKey` 或 `inputID` 的按键时，下一步禁用；异步等待期间也禁用。按键进入后续步骤后不会继续因当前 activeButton 改变而换掉正在校准的对象。

界面接收 `inputredirect` 的 JSON 输入，匹配 scancode/flag；Razer 特殊键有单独路径。当前表额外加入 Fn 的 scancode `0x7e`。Windows/Left GUI、RO/Before Left Shift、日文变换键等具有明确别名映射；这解决物理输入身份与页面按钮身份的对应，不能靠显示名称相似替代。

## 打开、前台切换与输入服务

当 `open` 且 native window/tab helper 判定当前窗口活动时，窗口调用初始化函数 `K`。它使用 initialized ref 和 pending Promise ref 协调并发调用；执行顺序是：

1. `AppEngine.disableMapping()`。
2. `90584.S5()` 查找当前实际设备，确认尚未重定向时才启用输入重定向。
3. 保存本窗口输入监听函数并订阅 `inputredirect`。
4. dispatch `initCalibration()`，把 UI reducer 恢复到校准起始状态。
5. 设置 initialized 标记。

`initCalibration()` 在此只是 Redux action，不发送 HID 校准模式命令。真正进入 Testing Mode 在 MW 的 bottom-start 方法中发生。

模块 `90584` 按产品 category 选择键盘专用的 `isKeyboardInputRedirectEnabled`、`enableKeyboardInputRedirect`、`disableKeyboardInputRedirect`，或通用版本；它依据当前 USB/BLE/receiver 和已保存的 dual-link 关系选择实际设备。没有找到设备时帮助函数返回；不能将该返回当作设备已完成重定向的读回。

窗口监听真实的 `foregroundWindow` 事件。只有 native helper 仍活动且当前事件 executable 是源码指定的主机程序时才初始化；其他前台变化调用清理 `Z`。窗口不会因“用户正在看这张页面”自行制造前台事件。

初始化异常只在 `catch` 中记录，随后清空 pending ref；源码没有在该 `catch` 内逐步撤销已经完成的 disable/redirect 操作。这是当前失败路径的实际边界。

## 按钮如何进入 MW

actions 模块 `10054` 把当前按键 ID 包装为单元素 payload 数组，分别发送 bottom-start、top-start、bottom-verify、top-verify；停止动作不携带按键数组。reducer 将这些动作发到 `window.mwBroadcastChannel`，同时维护任务签名；MW 的 generic feature 事件表分别注册：

| 事件 | 参数字段 | MW 方法 |
| --- | --- | --- |
| `ON_START_BOTTOM_ANALOG_CALIBRATION` | `bottomCalibration` | `startBottomMagneticAnalogCalibration` |
| `ON_START_TOP_ANALOG_CALIBRATION` | `topCalibration` | `startTopMagneticAnalogCalibration` |
| `ON_VERIFY_BOTTOM_ANALOG_CALIBRATION` | `verifyBottomCalibration` | `verifyBottomMagneticAnalogCalibration` |
| `ON_VERIFY_TOP_ANALOG_CALIBRATION` | `verifyTopCalibration` | `verifyTopMagneticAnalogCalibration` |
| `ON_STOP_CALIBRATION` | `stopCalibration` | `stopCalibration` |
| `ON_RESTORE_ANALOG_CALIBRATION_DATA` | `calibratedKeys` | `restoreAnalogCalibrationData` |

这些事件声明 `Storage_NoSave`。顶部核验成功后的本机记录写入由 `se` 自己执行，不能从事件属性推断“校准不保存任何本机记录”。746 注册该 feature 的所有者是 MW main 模块 `86191`，它同时注册 analog OBM、gamepad、interrupt、dual-keypress priority、custom game-mode 等 feature。

## 实际执行顺序

1. 步骤 1 点击下一步：界面进入步骤 2，等待并发送 bottom-start。MW 先调用 `setDeviceModetoTestMode()`，再解析当前键，调用 bottom setter。只有 setter 返回 `status:"SUCCESS"` 才发布当前步成功。
2. 步骤 2 点击下一步：界面进入步骤 3，发送 bottom getter。只有返回 `jsonData.result===0` 才核验通过；否则执行停止并发布失败。
3. bottom 核验成功后，步骤 3显示可继续状态。继续时进入步骤 4，发送 top setter。
4. top setter 成功时，步骤 4 effect 用一次性 ref 自动发送 top getter，避免每次成功状态变化都重复核验。
5. top getter 返回 `result===0` 时，MW 先更新并写入当前 serial 的本机已校准键记录，再发布 top verified 成功；之后重新读取本机文档，发布 `MW_UPDATE_ANALOG_CALIBRATED_KEY_LIST_TO_UI`。

无按键资料、setter 不成功、getter 结果非零或抛异常，均走各方法的失败路径。bottom-start/top-start 的成功表示命令开始被接受；它与 getter 核验成功是不同阶段。

## 键 ID 与线协议

`getKeyDataByInputId` 将 `KEY_WINDOWS` 改为 `KEY_LEFT_GUI` 后查产品键表；Fn 使用 `analogKeyID:59` 的专用对象；日本布局下 `KEY_BACKSLASH` 明确采用 `analogKeyID:42`。这些分支在 MW 解析实际输入，不由 UI label 推导。

| 动作 | 命令三元组 `[长度,class,id]` | 746 协议方法字节区间 | payload / 结果 |
| --- | --- | --- | --- |
| bottom 开始 | `[1,254,46]` | `[131891,132395)` | payload byte 0 为 analogKeyID |
| bottom 核验 | `[2,254,174]` | `[132395,133102)` | 结果至少 2 byte；`data[0]` 为键，`data[1]` 为 result |
| top 开始 | `[1,254,45]` | `[133102,133600)` | payload byte 0 为 analogKeyID |
| top 核验 | `[2,254,173]` | `[133600,134292)` | 与 bottom 结果布局相同 |
| 进入测试模式 | `[2,0,4]` 的 set-device-mode | `setDeviceModetoTestMode` `[134695,134872)` | mode `2`、param `0` |
| 停止/恢复 Driver Mode | 同一 set-device-mode | `setDeviceMode` `[130076,130241)` | mode `3`、param `0` |

四个校准方法都拒绝 `undefined` 或 `NaN` 的键 ID。getter 仅在传输 `SUCCESS` 且 `packetSizeReturned>=2` 时建立 `jsonData`；结果 `0`=pass，`1`=fail，`2`=not finished，其余保留为 Unknown。MW 核验只有 `0` 成功，不把未完成当作仍在后台成功等待。

## 本机已校准记录与恢复

`se.init()` 调用 `restoreAnalogCalibrationData()`。该恢复只读本机设备缓存并向 UI 发布列表，没有逐键重新执行 top/bottom getter，也没有把缓存重新写成设备校准参数。

读取兼容两种已存格式：旧格式是直接的 `{inputID,...}` 数组；新格式是 `{serialNumber,calibratedKeys}` 数组。新格式必须匹配当前 runtime serial；没有 serial 时不返回其他设备的条目。写入时没有 serial 则不增添记录；旧数组在写入时装入当前 serial 分组，同一 inputID 不重复追加，`KEY_LEFT_GUI` 归一为保存用的 `KEY_WINDOWS`。

因此打开界面时显示“曾校准”的来源和本次 getter 返回的通过结果不同。源码恢复方法没有读到实际设备校准数据，不能称为整机状态核验。

## 失败、重试、超时和关闭

窗口以 1 秒 interval 维护时间检查。经过 15 秒后，仅在当前 error ref 为真时才执行关闭；源码不是无条件 15 秒校准超时。输入回调和每次步骤推进重置这次计时。步骤超过 3、状态重置或清理时会清 interval。

失败界面的按钮调用重置，再 dispatch stop 和 UI init，以便重新开始。此处存在一个应保留的源码细节：746 `setCalibrationError(false)` action 确实携带 false，但当前 `keyboardSwitchCalibrationReducer` 的 `SET_CALIBRATION_ERROR` case 无条件写 `hasError:true`；真正复位发生在后续 `INIT_CALIBRATION`。不能把 action 的参数直接当作 reducer 已实现的状态变化。

关闭函数 `B` 先重置 UI，再调用 `Z()` 和外部 `onClose()`；它没有等待 `Z()` Promise 完成后才关闭显示。`Z` 在 initialized 为真时等待已有 pending 操作，然后按源码顺序：`enableMapping()` → 禁用输入重定向 → 移除本窗口 `inputredirect` 监听 → dispatch stop → 设 initialized=false。MW stop 调用 `setDeviceMode(Driver_Mode,0)`，正常结束发布 `{status:"stop",isSuccess:true}`，抛异常时发布 false。它没有再读取 device mode；返回未抛异常和模式读回确认不能混称。

前台观察 effect 的卸载函数移除 foreground listener、停止监视并调用同一个 `Z`。该入口涵盖窗口卸载的清理；中途初始化失败、已开始 UI 关闭和 MW stop 的实际完成仍是不同异步阶段。

## 756：V2 页面和来源

756 来源见 [UI 收据](../../../.analysis/inputs/frontend/keyboard-756-current/acquisition.json) 与 [MW 收据](../../../.analysis/inputs/frontend/keyboard-mw-756-current/acquisition.json)。UI 路径前缀为 `.analysis/inputs/frontend/keyboard-756-current/raw/apps.razer.com/synapse/products/756/ui/static/js/`，MW 为 `.analysis/inputs/frontend/keyboard-mw-756-current/raw/apps.razer.com/synapse/products/756/mw/`。

| 文件 | 当前所有者与字节区间 |
| --- | --- |
| UI `main.3fabc4f8.js` | 常量模块 `66663` `[599329,600390)`；`keyboardSwitchCalibrationReducerV2` `[213249,218325)` |
| UI `3021.0f2a9d4a.chunk.js` | 页面函数 `[311975,319272)`；actions `No` 起于 `310260`；行程窗口 `uo` `[296553,298566)` |
| MW `main.4ac4b05541bb1859fa0e.js` | 产品 feature 注册模块 `86191` `[224217,235743)` |
| MW `6737.90f8e2e80827f3801f8e.js` | V2 feature 模块 `4368` `[1011027,1026276)`；行程变换 `88758` `[2312641,2319990)`；连接状态 `18065` `[2319991,2320767)` |
| MW `757.2d5b3153bc88ac375f77.js` | `40757.reconcileSwitchTravelMappings` `[558,3089)` |
| MW `rzDevice25.40fe6f559bba152076d4.js` | 模式 `[130155,130699)`；状态 `[130699,131546)`；轴体 `[131546,132349)`；复位 set/get `[133210,134171)` |
| MW `rzHardwareEventsProtocol25Parser.72f78fc37aa526db9d50.js` | `parseNotificationEvents` 中 `ANALOG_USER_CALIBRATION_EVENT` 分支 |

产品 main 明确注册 `magnetic_analog_configuration_v2`，generic feature 表把它映射到 `4368.P`。它与旧 `magnetic_analog_configuration` 是不同 feature，不能从公共库含有旧类就推断 756 使用三步窗口。

## 756：页面操作和计时

页面挂载调用 `getKeySwitchType`。其 MW 消费者读取本机按 serial 的轴体记录并应用行程限制，没有调用硬件轴体 getter。feature 初始化恢复本机已校准列表；显式 `getCalibratedKeyList` 才批量读取设备的校准状态。设置轴体或复位轴体后，页面另外发出该查询。

单击切换选择，Ctrl 保留其他选中键；产品图支持区域选择。开始时没有选择则使用当前 `buttonList` 的全部实际输入。有选择时只在 UI 中建立这些键的待校准列表；Enter 命令本身不携带所选键数组。因此“选中集合”与设备校准模式参数不同，不能将页面选择推断为设备只上报这些键。

首次开始受本机提示确认标记约束；之后调用 start。校准中再次点击开始按钮则 `stop(true)` 保存；Cancel、倒计时归零及窗口失去可见条件使用 `stop(false)`。保存或保存动画期间锁定交互。Factory Default 的说明和内容灰态均要求 Normal Mode。

reducer 开始时立即进入 `calibrating` 并设置 60 秒；每秒 tick 减一。每次有效校准状态事件会重新设为 60 秒，缺少 inputId 的状态通知也会在校准中重置计时。剩余 15 秒显示倒计时提示。它是无新事件后的会话计时，不是每键固定运行 60 秒。

成功事件会把其他尚为 start/failed 的键显示复位为 idle，更新当前目标为列表中首个未成功的键；start/failed 则把事件键设为目标。页面另外用输入身份和变化计数控制按键动画。显示的已校准数量是本机/查询列表与本次 session 成功键的并集。

导航离开通过父页面登记的提示回调处理。Continue 只关提示并清待导航回调，继续校准；Don't Save 调用 `stop(false)` 后立即执行保存的导航回调，没有等待设备 Abort 完成。保存期间 Don't Save 禁用。

## 756：命令、硬件事件和本机记录

| 入口 | 参数与实际行为 |
| --- | --- |
| `ON_START_ANALOG_MAGNETIC_KEY_CALIBRATION` | 清 session 集合，`setAnalogUserCalibrationModeCtrl(Enter=1)`；必须返回 `SUCCESS` |
| `ON_STOP_ANALOG_MAGNETIC_KEY_CALIBRATION` | `stopMagneticKeyCalibration.save`，除显式 false 外均保存；保存发 `Exit=0`，丢弃发 `Abort=2`；必须返回 `SUCCESS` |
| `ON_GET_ANALOG_MAGNETIC_KEY_CALIBRATED_KEY_LIST` | 按当前产品键表全部 analogKeyID 加 Fn `59`，排序去重，每批最多 `40` 键查询；只有状态 `1` 收入已校准列表 |
| `ON_RESET_ANALOG_MAGNETIC_KEY_CALIBRATION` | 先真实读全键状态，必要时执行整机校准数据复位并轮询 |
| `ON_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATION_EVENT` | 由真实硬件通知进入 MW 队列，转成 UI 单键状态；不是 UI 自行产生成功 |

事件均声明 `Storage_NoSave`，feature 自己维护 `magneticKeyCalibratedKeys:[{serialNumber,keyIds}]`。按键转换保留 Fn `59`、日文反斜杠 `42` 和 Left GUI/Windows 别名。

通知号 `128` 的 parser 从 bytes `2/3/4` 提取 keyId、keyStates、calibrationResult。hardware listener 把它作为 `MW_ACTION_FROM_DEVICE` 入队。`MAKE=2` 映射 start；其他 keyStates 下 `RES=0` 映射 failed，其他结果映射 success。当前映射没有进一步验证未知结果枚举；不能把它写成“仅 SUCCESS=1 才成功”。成功事件加入 session 集合，再发布 `MW_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATION_STATUS_TO_UI`。

停止先复制并清 session，再发送 Exit/Abort。源码的 `finally` 无论退出命令是否成功都重新读取本机记录；save=true 且本次有成功键时，仍将它们与旧记录并集、排序、保存并发布列表。否则发布旧记录。这意味着列表通知不能独立证明 Exit 成功。reducer 收到列表即进入 idle、清 session 和 timer；必须分别解释命令异常与该列表消费者。

| 线协议动作 | `[长度,class,id]` | 数据与结果 |
| --- | --- | --- |
| 校准模式 | `[1,2,64]` | byte 0 为 Exit/Enter/Abort；SUCCESS 时取应答 byte 0 建立 mode |
| 查询校准状态 | `[0,2,194]` | 动态 payload 每键 2 byte，偶数字节为 keyId；返回每 2 byte 为 keyId/status；0 未校准，1 已校准，其余 Unknown |
| 设置轴体 | `[0,2,67]` | 每条 3 byte：keyId/switchIndex/keyTravel；最大 26 条；SUCCESS 时建立发送数据摘要 |
| 整机复位开始 | `[1,2,65]` | 发送 byte 0=0；应答必须恰好 1 byte，否则改为 FAIL |
| 整机复位进度 | `[1,2,193]` | result 0=Reset fail、1=Resetting、2=Reset done |

复位有独立防重入标记。首次真实读取为空时，只清 session/本机记录；非空则发复位、等待 1000ms，再最多查询 20 次，后续各间隔 100ms。result=2 完成；result=0 时仍读取全键列表，只有列表空才接受完成；1 继续，其余失败。UI 反馈至少保持 3000ms；异常路径重新发布开始时的缓存或已经读到的实际列表，清防重入后再抛原始异常。不能把复位按钮点击时 UI 清空列表当成设备已复位。

## 756：轴体、行程与映射联动

Razer 轴体 index=1，行程固定 `40`；Generic index=2，可设整数 `20..40`。行程窗口先经过 converter 再以 mm 标示，对应 2.0–4.0mm。混合选择的轴体显示 null、混合行程显示 undefined；不能用第一枚键覆盖混合状态。Generic 选择先确认，再输入行程；只有 `MW_UPDATE_ANALOG_MAGNETIC_KEY_SWITCHTYPE_STATUS_TO_UI` 的新 token 且 `Success` 才自动转入校准。

set payload 为 `setMagneticKeySwitchType:[{inputId,switchIndex,keyTravel}]`。reducer 先乐观合并显示；MW 严格验证整数及分支值域，转换实际键 ID，然后按 26 条分批写硬件，每批检查 `SUCCESS`。本机保存结构为 `magneticKeySwitchTypes:[{serialNumber,switchTypes}]`；Razer/40 的默认项从表中删除。只有内容变化才写存储，单独向 UI 发布 Success/Fail。失败不会由该 reducer 自动撤销先前的乐观轴体值。

设置、恢复读取、复位轴体共用串行 Promise queue。入队时捕获 RzDevice 对象和 `18065.fP()` 连接代次；开始、每批结束、读取缓存及异步加载后检查，旧连接抛错且不向新连接发布 Fail。连接清理调用 `18065.rY()`，递增代次并清空当前行程约束。此保护在轴体链明确存在；不能泛称全部校准方法都有同一保护。

应用已存轴体先建立当前设备行程表。756 的 8k analog 分支还刷新 analog event skip-count 注册，再发布 UI 轴体表。启用 reconcile 时，`40757` 比较旧/新行程限制并完成以下消费者：

1. `88758` 限定真实模拟键、有效整数行程和低于产品最大值的缩短行程；按键触发/释放阈值截到最大行程，并保持源码规定的 release 间隔。
2. 调整 DKS down/up 顶底点、analog controller 的 travelRange 与归一坐标，并处理确实存在双输出时两段间距；Fn 不自动生成缺失的基础映射。
3. 遍历全部本机配置档，仅为改变的档重建 AppEngine，递增文档 version、写设备缓存及 window storage；活动档变化再发布真实保存后的 mapping list。
4. BLE 分支到此结束。其他连接遍历活动档槽 1 及 GUID 对应的未锁定实际板载槽，计算必要触发点差异，按协议最大批次调用 `obmEngine.setActuationPoint`，每批明确要求 `SUCCESS`。
5. 每次异步边界核对设备、引擎和连接代次；过期结果停止。初始化读取板载数据后也可再次 reconcile，不是只在当前轴体窗口中执行。

轴体复位发送 `{keyId:255,switchIndex:255,keyTravel:255}`，之后清当前 serial 的本机轴体记录并执行同一联动。它与整机校准数据复位是两项不同操作，各有单独确认窗口。

## 756：可见性与释放

校准中注册 beforeunload、visibilitychange、screenlocked、systemsuspend 和 native tab-message。切换到其他 tab、document 不可见，或 native focus 消息查询窗口得到最小化/不可见时，只调用一次 `stop(false)`。异步窗口查询有 active 标记，effect 清理后不继续使用结果。effect 结束逐一取消上述监听和 native disposer；1 秒 timer 单独清除；向父页面登记的导航提示函数在卸载时置空。

本页完成的是三款当前源码的静态链路核对；没有运行厂商 JavaScript、应用程序或执行设备校准。
