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

窗口以 1 秒 interval 维护时间检查。746 `U` 的完整回调在超过 15 秒时执行 `return s&&(clearInterval(...),w(null),f(!1)),void B()`：逗号表达式末尾的 `B()` 无条件调用关闭，只有前面的显式清 interval/loading 操作受当前 error ref 控制。不能把这里读成“error 为真才关闭”。关闭 `B` 又通过 `V` 清理 timer。输入回调和每次步骤推进重置这次计时；步骤超过 3、状态重置或清理时也会清 interval。

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

## 实际页面树与 Rust 对照范围

本轮重新计算所用当前 JS/CSS 原件的字节数及 SHA-256，与获取收据匹配。部署文件名、入口和获取时稳定性作为版本身份，不把共用库里的类存在当作产品挂载证明。除上文协议函数外，补充以下完整 AST 所有者；区间仍为 UTF-8 字节半开区间。

| 产品/文件 | 完整所有者与页面边界 |
| --- | --- |
| 740 UI `9908.0c364f6c.chunk.js` | root `to [294348,305734)`，nav `[301748,302227)`：Customize → Actuation → Lighting → Calibration → Help |
| 746 UI `9908.23d31a59.chunk.js` | root `to [294038,304968)`，nav `[301116,301595)`；实际校准页面 `Aa [281661,283010)`：提示条 → 产品 SVG → 左列校准 widget → 用户显式打开的 `Ea` |
| 756 UI `3021.0f2a9d4a.chunk.js` | root `nr [330271,343239)`；非 BLE nav `[338420,339056)` 挂 Calibration，BLE nav `[339057,339575)` 不挂；页面连接器/组件 `Do [310998,319273)` |
| 同一 756 chunk | 产品图与框选 `ro [290770,296153)`；行程窗口 `uo [296553,298566)`；首次开始 `vo [298658,299633)`；Generic 说明 `go [299634,300772)`；局部 reset `So [300773,301510)` |
| 同一 756 chunk | Calibration 面板 `Co [301511,303634)`；轴体面板 `xo [303679,305874)`；双 tab widget `_o [307320,309235)`；离开提示 `yo [309236,310259)` |

键盘 PID 752 的 [当前入口收据](../../../.analysis/inputs/frontend/keyboard-752-current/acquisition.json) 对应 `3717.9326e687.chunk.js` root `oa [179097,186758)`、nav `[184295,184519)`，只有 Customize、Lighting、Help。该型号不应因公共库含校准字段而获得本页；其他型号需要各自入口证据。

### 740/746：已有页面、协议与释放链

Rust 的 [页面和窗口](../../../crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration.rs) 明确区分真实 `Live` 与仅供预览的 `Sample`，页面 Start 不由预览成功驱动。其产品图、intro 提示、Factory 灰态与说明、三段步骤、捕获键框、等待/成功/失败、Cancel/Next/Done 均有实际 renderer。窗口保存返回焦点与自身 FocusHandle，使用锁定 GPUI Kit 的 Dialog；原版 `.choose-a-mat` 的 800px 最小宽、宽屏 850px 上限、顶部 100px 与滚动内容在 Rust 有明确尺寸处理。这是静态组件对应，尚未作窗口渲染验收。

[keyboard_calibration_live.rs](../../../crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration_live.rs) 保留按键身份别名、Fn 输入、原始/备用捕获、initialized、步骤、loading、completed、top verify 一次性标记和 15 秒计时。按步骤发送 bottom-start → bottom-verify → top-start → top-verify，已校准列表与本次成功状态独立消费。`start_live_timer` 超过 15 秒关闭与上文逗号表达式的实际行为对应；不能沿用有误的条件解释修改它。

真实请求经 [Shell 校准 owner](../../../crates/razer-shell/src/shell/keyboard/keyboard_legacy_calibration.rs) 和 [串行 worker](../../../crates/razer-shell/src/shell/keyboard/keyboard_legacy_calibration_worker.rs) 处理：绑定实际连接、读真实 serial/layout、恢复该 serial 的本机列表；modal epoch 拒绝旧窗口事件；foreground 观察核实本项目当前 executable；输入重定向确认实际目标设备后才向页面发布 initialized。worker 对已获资源保留清理责任，不能由页面关闭替代服务恢复。

[独立协议实现](../../../crates/razer-device/src/keyboard/keyboard_legacy_calibration.rs) 的 capability 只接纳 740/746，bottom 前先 Testing=2，top/bottom getter 读取第二返回字节判断 `0`，停止写 Driver=3，保留失败时的 stop 结果。本机 [校准记录存储](../../../crates/razer-storage/src/keyboard_calibration.rs) 只有 top verified 后才保存并追加 UI 列表；存储失败会保留失败并执行 stop，不能把校准缓存当成硬件查询。

关闭和卸载发 Unmount，由 worker 停 foreground、恢复 mapping/redirect、执行 stop、关闭服务并释放 suppression/lifecycle。该链在静态源码中可追踪，不代表当前本机驱动、服务、HID 或官方 DLL 已成功执行；输入注册、前台变化、部分初始化失败、存储失败、断开及清理失败仍需逐项测试和实际对照。

### 756：Rust 命令与状态消费者的缺口

[keyboard_calibration_v2.rs](../../../crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration_v2.rs) 已包含 Calibration / Switch Type 两个 segment、选键/全选、start/save/cancel、60 秒会话计时与末 15 秒提示、首次说明、本机不再提示偏好、Generic 说明、Bottom-out 行程、两种 reset 确认和 Unsaved 导航提示。选中集合、当前目标、session 状态、本机/查询 calibrated 列表和 switch 表分别持有；[artwork owner](../../../crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration_v2_artwork.rs) 有框选、按压/失败动画、保存反馈、toast 与卸载取消。以下接线缺口使它尚不能成为可用设备校准功能。

| 边界 | 官方消费者 | 当前 Rust 的首个缺失/差异 |
| --- | --- | --- |
| 七项页面命令 | MW `4368` 对 start/stop/reset/get-list/get-switch/set-switch/reset-switch 各有 feature event；进入真实硬件/本机记录流程 | 页面确实经 `v2_message → request_actuation_message` 发出七种 `ON_*ANALOG_MAGNETIC*` 命令；[Shell 分发](../../../crates/razer-shell/src/shell.rs) 仅拦截 756 全局触发/Rapid 和旧校准命令，其余进入 [keyboard_submission](../../../crates/razer-device/src/keyboard/keyboard_submission.rs) 的解析器，那里没有 V2 intent 分支，返回未接该任务链的错误。七项命令尚无生产执行 owner |
| 四类 MW 回应 | `Do` 消费校准列表、逐键状态、轴体表、轴体写入状态/token | `observe_keyboard_calibration_v2` 目前只在 ProductWorkspace / SourceProductWorkspace 转发到页面；未找到 Shell/设备事件分发对它的实际调用。因此有 reducer 和手工注入状态用例不等于收到真实设备事件 |
| 连接/模式初始化 | root 按实际 BLE 选择 nav，Normal Mode 决定 Factory 灰态；mount get-switch，feature init 恢复本机列表，显式 get-list 才真实查询 | 页面提供 `observe_calibration_v2_connection`，但外层也只有转发接口，没有实际状态发布调用；normal_mode 在初始化设 true、is_ble 保持未观测。首次非 BLE 时额外发送 get-list 的 Rust 分支还需按官方读取顺序核对，不得代替 feature init |
| 进入、保存、取消与 reset | Enter/Exit/Abort、真实通知 128、每批查询、reset 轮询、serial 记录及错误/释放均由 `4368` 和硬件消费者执行 | 当前页面只改变 phase、生成动作；后台缺口也使 stop(false)、reset 与窗口/会话取消无法完成硬件退出或恢复确认。不能把 UI phase=Idle/Saving 或动画完成视为设备成功 |
| 轴体与行程 | 串行 queue、RzDevice/连接代次保护、26 条批写、serial 存储、8k event 重新注册和全部配置档/DKS/控制器/板载槽 reconcile | 页面已有乐观 switch 表和整数行程 UI，但缺少上述实际 owner 与回传；本页 [轴体联动](#756轴体行程与映射联动) 仍是官方语义证据，不能描述成 Rust 已执行 |
| 自动转入校准 | 仅新 `switchTypeUpdateToken` 且 `Success` 执行 pending 自动开始 | Rust 用 `auto_start_after_switch`，在任意下一条 switch status 上消耗它；没有对应 token/请求身份字段。即使补上回传，也需拒绝较早请求/旧连接的 Success |

Rust 已将隐藏/最小化后的窗口状态、锁屏、系统挂起、退出等入口接到 `cancel_keyboard_calibration_v2`，见 [窗口焦点](../../../crates/razer-shell/src/shell/gamepad/controller_calibration_focus.rs) 与 [会话生命周期](../../../crates/razer-shell/src/shell/keyboard/keyboard_lifecycle.rs)。普通 blur 保留校准、只有真实隐藏/最小化才取消，这一判断与源码对应；取消命令的实际硬件消费者仍必须补齐。

### 756：视觉、弹层与状态细节

756 `3021.b1237c23.chunk.css` 已按完整规则核对，以下固定坐标均来自当前官方页面，不是通用组件默认值。Rust 有部分显式尺寸对应，但弹层仍复用了统一 `div` 内容构造，不能据此宣称完整还原。

| 页面部件 | 官方 CSS/状态 | Rust 核对 |
| --- | --- | --- |
| 双 segment widget | `[88805,89840)`：680px 宽、273px 最小高、20px 顶 margin；active tab 绿色渐变 border，校准中不能进入 Switch Type | 主面板尺寸、tab 图标/绿色渐变、busy/saving 禁用条件有对应 |
| 轴体列表和提示 | `[91820,94497)`：列表 90..184px、单项最小高 46px、radio 20px、独立 manufacturer、selected-count 浮层 | Rust 有对应条目尺寸、混合轴体无选中态和 hover 提示；显示的缺项默认值是源码 UI 约定，不是硬件实测 |
| 首次/Generic 弹层 | `[79269,81343)`：420px/446px，top 230px/280px；首次标题中的警告图 24×21，复选框与按钮各有独立 margin | Rust 保留宽度/位置和动作，但首次图标被放到说明之后；统一 gap/标题构造尚未对应全部 CSS 和结构 |
| Bottom-out | `[76771,79269)`：850px、top 106px 至 bottom 0；36px 独立标题栏，内容 padding 20px 30px、gap 30px、可滚动；slider 204×70 旋转，264px 外框 | Rust 是统一 warning 边框/20px padding 的内容卡，标题/关闭未形成同一 36px 标题栏；使用普通竖向 Slider，尚未对应原版端点标签、视觉轨道和滚动内容结构 |
| 局部 reset | `[81343,82442)`：限制在面板的 overlay，300px 宽、top 75%、错误色标题/边框/确认按钮；点外部关闭 | Rust 有面板范围和关闭动作，仍需对应错误色标题/按钮及准确 padding/margin；两个 reset 的设备和存储语义必须分别测试 |
| Unsaved / 保存反馈 | `[87571,88952)`：400px 离开提示、保存时禁用 Don't Save；独立保存 spinner、100ms 切换、按键动画及 3 秒 toast | Rust 保留导航待提交项、Continue/Don't Save 动作、100ms 与 3 秒反馈，并以 artwork generation 取消旧动画；后台未接通时不得生成成功反馈。prompt 目前为 deferred div，没有对应源码的 dialog/aria-modal/标题关联语义或自身焦点所有者 |

官方警告 SVG 与普通/磁轴/low-profile 动画、refresh 普通/hover 图使用现有 [统一资源映射](../../../tools/resource-mappings.json)；本轮核对相关原件身份，没有运行媒体或窗口验收。

## 测试与实际对照要求

[旧校准纯协议测试](../../../crates/razer-device/src/keyboard/keyboard_legacy_calibration_tests.rs) 覆盖 Testing/Driver 命令、getter 第二字节与别名；[旧页面测试](../../../crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration_tests.rs) 区分预览样例和真实入口；V2 文件内已有 session/列表分离和失败 switch response 的状态测试。本轮读取其源码，未运行这些测试；它们也没有提供本轮当前官方 DLL 实际执行结果。

所有产品/布局/传输分支均须补齐从页面输入到后台实际执行、回应消费者和清理的自动化用例。旧版必须核对输入重定向部分成功、15 秒关闭、top getter 一次性、serial 保存失败、前台切换、modal epoch、断开和 stop/释放失败；V2 必须核对 Enter 失败、无 inputId 的计时重置、未知通知、Exit 失败仍发布列表的源码 finally、reset 的全键查询/轮询、混合轴体、过期 token、26/40 条分批、全部 profile reconcile 与连接代次取消。

预期报文、事件顺序和失败结果须来自当前官方原件或实际采集，不能由 Rust 被测编码器生成。实际 DLL/设备对照按 [verification.md](../../../.agents/rules/verification.md) 记录双方原始结果、差异和完整清理；校准数据 reset、轴体或校准写入只有在可记录并完整恢复原值时才能执行。当前没有这样的对照收据，相关设备校准、恢复与运行时验收保持未验证；静态页面对应和模拟状态测试不能替代它们。
