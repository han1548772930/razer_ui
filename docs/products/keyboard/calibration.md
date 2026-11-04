# 磁轴按键校准

本页解释当前实际挂载校准页的 PID 740、746 与 756。740/746 使用单键三步窗口，756 使用 V2 多键页面、硬件校准事件和轴体类型流程。两套功能的模式命令、状态来源和退出顺序不同。以下先解释 740/746，再解释 756。

## 来源和产品边界

获取版本及原件哈希分别见 [740 UI](../../../.analysis/inputs/frontend/keyboard-740-current/acquisition.json)、[740 MW](../../../.analysis/inputs/frontend/keyboard-mw-740-current/acquisition.json)、[746 UI](../../../.analysis/inputs/frontend/keyboard-746-current/acquisition.json)、[746 MW](../../../.analysis/inputs/frontend/keyboard-mw-746-current/acquisition.json) 收据。

740/746 的完整模块/函数体定位见 [keyboard-main-audit.json](../../../.analysis/results/frontend/keyboard-main-audit.json) 的 `local_controls_source_review`；该节产品集合不包含 756。756 当前会话使用同一收据的 `keyboard_756_calibration_session_chain`，不能借用 740/746 的完成状态。以下区间均为原件 UTF-8 字节半开区间。

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

756 当前会话由统一选择器核实 [UI 收据](../../../.analysis/inputs/frontend/keyboard-756-current-refresh-ec3db4b29c5a/acquisition.json) 与 [MW 收据](../../../.analysis/inputs/frontend/keyboard-mw-756-current-refresh-ec3db4b29c5a-2/acquisition.json)。UI 原件前缀为 `.analysis/inputs/frontend/keyboard-756-current-refresh-ec3db4b29c5a/raw/apps.razer.com/synapse/products/756/ui/static/js/`；MW 原件在不带 `-2` 的同名 refresh 目录。正式结果保留完整 AST 所有者、原件字节身份及精确共享依赖，不重复捕获整库。

| 文件 | 当前所有者与字节区间 |
| --- | --- |
| UI `main.67cc2e45.js` | `keyboardSwitchCalibrationReducerV2` `[211293,216608)`；常量模块 `66663`；新增停止完成事件 `G3J` |
| UI `3230.8e48a2ba.chunk.js` | 连接器/页面 `Mo [309323,321461)`；实际产品 root `rr [332486,345524)`；root 连接器 `lr [345530,346719)` |
| MW `main.423ea9b0242da188f465.js` | 产品模块 `86191` 实际注册 V2 feature |
| MW `6737.e9cfdfde12b1ade72a64.js` | feature 模块 `4368 [1011027,1026276)`；generic manager `ye [1068138,1078404)`；硬件生命周期模块 `93182 [1176311,1188807)` |
| MW `rzDevice25.2f39d1d815a9b796d2ea.js` | 会话模式方法 `[130155,130699)` |
| MW `rzHardwareEventsProtocol25Parser.54523ff85b6974b87475.js` | 模块 `79885`：通知记录 5/9 的事件 128 |

产品 main 明确注册 `magnetic_analog_configuration_v2`，generic feature 表经模块 `4368` 的导出 `MA` 取得类 `P`，并非导出 `P`。它与旧 `magnetic_analog_configuration` 是不同 feature，不能从公共库含有旧类就推断 756 使用三步窗口。Enter → 通知状态 → Save/Abort → 列表/停止完成消费者、下文全列表批次与整机 reset，以及当前 USB 通知提供者分别保留有界静态证据；轴体完整写入/reconcile、其他实际分支与真实设备结果仍须分别完成。

## 756：页面操作和计时

非 BLE 产品 root 的 `componentDidMount` 明确依次发 `getCalibratedKeyList()`、`getKeySwitchType()`；校准页面自身挂载另发 `getKeySwitchType()`。这修正了原有“首次额外 get-list 尚待核对”的不确定项。轴体读取消费者使用本机按 serial 的记录并应用行程限制，没有调用硬件轴体 getter。feature 初始化仅恢复本机已校准列表，不能代替 root 的显式硬件列表查询。

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
| `ON_GET_ANALOG_MAGNETIC_KEY_CALIBRATED_KEY_LIST` | 按当前共享键表全部正数 analogKeyID 加 Fn `59`，排序去重得到 `111` 键，按 `40/40/31` 查询；只有状态 `1` 收入已校准列表 |
| `ON_RESET_ANALOG_MAGNETIC_KEY_CALIBRATION` | 先真实读全键状态，必要时执行整机校准数据复位并轮询 |
| `ON_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATION_EVENT` | 由真实硬件通知进入 MW 队列，转成 UI 单键状态；不是 UI 自行产生成功 |

事件均声明 `Storage_NoSave`，feature 自己维护 `magneticKeyCalibratedKeys:[{serialNumber,keyIds}]`。按键转换保留 Fn `59`、日文反斜杠 `42` 和 Left GUI/Windows 别名。

通知的已核实前端接入为 `RzMappingEngine.registerHardwareEvent` → `apiElectron.doMappingEngineAction` 外部边界；返回的 `hardwareevent` 按 container 查服务表，再进入 Protocol25 parser、监听器和 MW 队列。parser 对记录 5/9 的通知 `128` 从 bytes `2/3/4` 提取 keyId、keyStates、calibrationResult，使用整个 `e.data.buffer`，本地分支没有独立长度校验。listener 检查电源/观察者并以 `MW_ACTION_FROM_DEVICE` 入队；没有会话代次或选中集合条件。

`MAKE=2` 映射 start；其他 keyStates 下 `RES=0` 映射 failed，其他结果映射 success。未知非零结果也进入 success，定义中的 Aborted 不由该映射产生。成功事件加入 session 集合，再发布 `MW_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATION_STATUS_TO_UI`。feature 和 UI reducer 都没有以校准 active/选中键过滤有效输入；晚到成功事件可能在 stop 清空后再次进入集合，不能将轴体链的连接保护套用于这里。

停止先复制并清 session，再发送 Exit/Abort。源码的 `finally` 无论退出命令是否成功都重新获取本机记录，使用此时的 runtime serial；没有捕获开始时 serial。save=true 且本次有成功键时，合并排序，缓存存在则等待保存后发布；缓存不存在仍警告并发布合并列表。若 finally 内获取/保存继续抛异常，会覆盖退出异常并阻止后续列表发布。因此发布列表不能证明 Exit 或持久化成功，也没有自动回滚。

当前列表消费者 `fmF` 进入 idle、清目标/session/timer，保留停止 pending/token。另一消费者 `G3J` 对 `MW_UPDATE_ON_STOP_ANALOG_MAGNETIC_KEY_CALIBRATION_UI_ACTION_DEVICE` 清 pending、保存 status/message 并递增 token，错误状态也递增。generic manager 的被跳过任务可发 `completed`；页面只看 token 变化便调用等待的恢复回调，没有核对停止成功。列表、停止回执和硬件退出必须分别记录。

| 线协议动作 | `[长度,class,id]` | 数据与结果 |
| --- | --- | --- |
| 校准模式 | `[1,2,64]` | byte 0 为 Exit/Enter/Abort；SUCCESS 时取应答 byte 0 建立 mode；高层只检查 status，不比较回显 mode，也无独立 GET |
| 查询校准状态 | `[0,2,194]` | 动态 payload 每键 2 byte，偶数字节为 keyId；返回每 2 byte 为 keyId/status；0 未校准，1 已校准，其余 Unknown |
| 设置轴体 | `[0,2,67]` | 每条 3 byte：keyId/switchIndex/keyTravel；最大 26 条；SUCCESS 时建立发送数据摘要 |
| 整机复位开始 | `[1,2,65]` | 发送 byte 0=0；应答必须恰好 1 byte，否则改为 FAIL |
| 整机复位进度 | `[1,2,193]` | result 0=Reset fail、1=Resetting、2=Reset done |

## 756：全列表批次与 all-data reset

统一入口为 `python tools/workspace.py audit-family-function --family keyboard --function mw --product 756 --query keyboard-756-calibration-batch-chain`，正式结果归 [keyboard-main-audit.json](../../../.analysis/results/frontend/keyboard-main-audit.json) 的 `/keyboard_756_calibration_batch_chain`。本查询从当前 session 收据的剩余批次链接续，复用同一 UI/MW、Host feature-report 传输和当前 USB 通知 provider；不重做校准弹层或轴体 reconcile。

- 全列表来自共享键表模块 `22574` 的 150 行与专用 Fn `59`，去除零值并按数值去重排序，实际为 111 键；不是页面选中集合，也没有按当前布局过滤。依次发送 40/40/31 条 `[keyId,0]`，显式数据大小为 80/80/62 byte。每批必须 `SUCCESS`；解码只取完整双字节记录，忽略奇数尾字节，只有 status=1 进入列表。返回的键身份、数量与重复项没有按请求重新校验；缺少 `jsonData.keyStatus` 时该批按空表处理。任一批抛错不会发布已取得的部分列表。
- 普通 get-list 等待完整设备读取，再保存当前 serial 的 `magneticKeyCalibratedKeys`，最后映射为 UI inputId 发布；保存失败阻止本次列表发布。缺少本机文档只警告，仍可发布设备查询列表；缺少 serial 时记录 setter 不写入。共享存储按排除 version 的哈希去重，缓存先于写入更新；这里的 hooks 没有启用 `useLockableStorage`，实际走普通 `localStorage`，`PostSet` 的映射发布没有被等待。文件保存及该副作用不能当作设备确认。
- Reset All 的取消只关闭确认窗；确认后清页面选择/目标，暂存原显示列表供动画使用，再发送 reset。reducer 立即清空列表并进入 saving。后台有独立防重入标记，先取缓存作失败恢复列表，再完整查询设备；只有设备查询成功后才用实读列表替换恢复值。实读为空则跳过复位命令和轮询，只清 session 并保存空表；非空才发 `[1,2,65]`、payload `0`。应答必须恰好 1 byte，但其 `res` 值没有被高层检查。
- 复位请求成功后等待 1000ms，再用 `[1,2,193]` 最多轮询 20 次，首轮立即、后续 19 轮各等 100ms。每次必须 `SUCCESS`；result=2 直接完成，不再全量读回；result=1 继续；result=0 另读完整列表，只有空表才接受完成，否则立即失败；其余值失败。异常文案中的 2000ms 是固定轮数说明，不是含传输和全表复查的严格总时限。
- 成功路径先清 session、将恢复列表设为空，再保存空表。因此最后保存失败时，session 和随后 UI 列表仍可能已清空，而旧持久化记录仍在。其他失败保留预读设备列表，预读失败则保留先前缓存。后台捕获原错误后等待到 reset 开始至少 3000ms，再发布恢复列表，清防重入标记后重新抛错；原错误优先于恢复 UI 时的错误，没有设备回滚写入。`fmF` 收到列表就回到 idle，因此 idle、空列表或动画结束均不能证明 reset 成功。
- 批次读取、复位、轮询与保存没有连接代次、`AbortSignal` 或轴体 queue 的保护。每个读取方法独立捕获设备对象，保存时又查询当前 serial，重连可能混合旧设备结果与新 serial。generic runner 只在 descriptor 返回后检查取消，此时轮询、保存或 UI 副作用可能已经发生。该动作没有新增通知注册/退订或 native I/O 排空；通知生命周期继续精确复用下文 provider 收据。

本片段覆盖当前非 BLE PID756 的上述源码语义。没有执行厂商 JS、官方 DLL、应用或设备复位；没有创建临时副本、数据库、日志或新捕获。独立 Rust、真实 UI/后台接线、当前 DLL/设备对照与运行验收仍未完成/未验证。

## 756：轴体、行程与映射联动

统一入口为 `python tools/workspace.py audit-family-function --family keyboard --function mw --product 756 --query keyboard-756-switch-type-chain`，结果归 [keyboard-main-audit.json](../../../.analysis/results/frontend/keyboard-main-audit.json) 的 `/keyboard_756_switch_type_chain`。它从全列表/reset 收据的首个剩余链接续，复用当前会话、serial 存储、feature-report 传输和校准通知 provider，只补轴体选择、串行写入、恢复与行程联动的实际调用方。

Razer 轴体 index=1，行程固定 `40`；Generic index=2，MW 要求整数 `20..40`。行程窗口先经过 converter 再以 mm 标示，对应 2.0–4.0mm。混合选择的轴体显示 null、混合行程显示 undefined；缺项使用页面约定值，不能视作设备实测。Generic 选择先确认，再输入行程，提交前保存所选键的旧轴体表；下一次轴体状态 token 为 `Success` 才自动转入校准。该 token 没有请求 ID，不能描述成与本次写入严格关联。

set payload 为 `setMagneticKeySwitchType:[{inputId,switchIndex,keyTravel}]`。MW 先完整验证并转换键身份，保留 Fn59、日本布局 backslash42 和 Windows/GUI 别名，无法查到的键跳过。空/非数组输入只警告，全为未知键则直接返回；外层这两种情况仍会发布 `Success`。有效项按 26 条分批发送 `[0,2,67]`，每项为 keyId/index/travel 三字节，每批明确要求 `SUCCESS`。Protocol25 成功后的 `jsonData` 是发送字节摘要，不是设备轴体表读回；协议层没有 MW 的整数/范围验证，只拒绝空条目及字段 undefined/NaN，再赋入 Uint8，字段 null 没有单独拒绝。

硬件写入先于本机缓存检查和保存，前批已写后遇后批失败、缓存缺失、保存失败或 reconcile 失败都没有回滚。按当前 serial 合并 `magneticKeySwitchTypes:[{serialNumber,switchTypes}]`，Razer/40 从覆盖表删除；按 keyId 排序比较，仅内容变化才写存储。缺 serial 的 setter 不写入，保存后也没有额外代次检查。`get-switch` 只取本机 serial 表、应用约束并发布 UI，不重发硬件轴体值，也不是硬件 getter。

设置、读取已存表与复位轴体共用串行 Promise queue。入队时捕获 RzDevice 对象和 `18065.fP()` 连接代次，开始及多个异步边界检查；queue 用独立 catch 允许后续任务继续，同时向调用者保留原错误。开始检查位于 setter 内部 try 之外，失败不发布轴体 Fail；setter 捕获到错误也仅向当前连接发布 Fail。该 queue 没有 AbortSignal 取消。连接清理调用 `18065.rY()`，递增代次并清空当前设备行程约束，不能据此推断全部校准方法都有此保护。

应用已存轴体先建立当前设备行程表。756 的 8k analog 分支还按当前 polling rate、行程约束和已登记的 all/single 选择刷新 analog event skip-count；刷新函数将异常转为警告，部分注册失败只返回。随后发布 UI 表，没有新的代次检查。启用 reconcile 时，`40757` 比较旧/新行程限制，已核对以下调用语义：

1. `88758` 限定真实模拟键、有效整数行程和低于产品最大值的缩短行程；按键触发/释放阈值截到最大行程，并保持源码规定的 release 间隔。
2. 调整 DKS down/up 顶底点、analog controller 的 travelRange 与归一坐标，并处理确实存在双输出时两段间距；Fn 不自动生成缺失的基础映射。
3. 遍历全部本机配置档，仅为改变的档重建 AppEngine，递增文档 version、写设备缓存及 window storage；活动档变化再发布真实保存后的 mapping list。
4. BLE 分支在本机保存/发布之后结束。其他连接遍历活动档槽 1 及 GUID 对应、`lock/locked` 均未设置的正数实际板载槽，去重后比较当前板载阈值，只对必要差异按 15 条调用 `obmEngine.setActuationPoint`，逐批要求 `SUCCESS`。不存在对应 `obmJson` 槽则跳过。
5. reconcile 按调用者、设备和连接代次检查，板载循环另外核对引擎身份。过期分支直接 return，既有保存/写入不回滚；外层 setter 没有最终检查，因此这种 return 后仍可能发布 `Success`。不能将其归纳成每处过期都抛错或所有异步结束都有 guard。

reducer 在 set/reset 时先乐观合并/清空显示；全表响应只接受数组。任何轴体 status 都递增 token，只有 Success 合并返回项，Fail 保留原乐观值。Generic 校准取消/离开时，恢复等待任意停止 token 或 2 秒兜底，再写旧表并查列表；最多两次，每次 3 秒超时，任意下一条轴体状态 token 都可能触发重试或收尾。最后失败/超时也会重读本机轴体表与校准列表并解锁；unload 直接开始，卸载有 mounted 标记但没有完整取消全部超时。恢复交接、解锁及重读本机表均不能等同设备恢复确认。

轴体复位发送 `{keyId:255,switchIndex:255,keyTravel:255}`，之后清当前 serial 的本机轴体记录并执行同一联动。它没有 setter 专用的 Success/Fail 状态发布，通用任务结果与表发布须独立消费；与整机校准数据 reset65/C1 各有单独确认窗口。取消确认窗只关闭，不发 reset。

本片段未重新捕获校准查询/轮询或 native 通知。上述 AppEngine 生成编排、当前 OBM 引擎阈值缓存/协议和 MappingEngine 存储交接已由下一专题接续；旧收据保留其原有观察边界。analog skip-count 的原生注册仍独立未决，不能借校准通知 128 的 provider 结论宣称完成。没有修改产品 Rust，没有执行厂商 JS、DLL 或设备写入，没有新增临时副本/分析会话；现有实现与 UI/后台接线未在本批重验，实际对照和运行验收仍未验证。

## 756：AppEngine 生成与 OBM 阈值应用

统一入口为 `python tools/workspace.py audit-family-function --family keyboard --function mw --product 756 --query keyboard-756-appengine-apply-chain`，归 [keyboard-main-audit.json](../../../.analysis/results/frontend/keyboard-main-audit.json) 的 `/keyboard_756_appengine_apply_chain`。从轴体收据首个未决项接续，按精确 section SHA 复用当前 MappingEngine producer/storage、analog 结果消费者、shutdown 及 Protocol25/HID 传输，不重复捕获原生函数。该专题确认生成编排、当前 OBM 阈值写入和实际存储交接；全部输出执行器、条件 GameMode helper 和其他 analog 注册仍有独立边界。

`40757` 只为改变的 profile 调用 `50574.generateAppEngineFromProfileMapping`。生成前先按当前 profile/行程 clamp，再经 `6258.zm` 修复/规范真实输入、补空 keyboardGroup 并处理现有输出分支；实际 `DeviceInfo.category=KEYBOARD` 不进入 keypad 专用路径。按当前布局过滤键并排除 modTapGroup，普通输入调用当前 `9299` singleton 的普通生成器，AnalogInput 调用 analog 生成器；每项先深拷贝。合并 mappings、actuations、rapidTriggers 后再套当前行程约束，只过滤非 hypershift、同 scancode 的 analogKey→keyboard 自映射。hash 使用实际工具，排除 hash/gamemode 字段；生成函数本身没有连接代次或 AbortSignal，保护来自外层 reconcile。

turbo 先读本机默认/用户表，补缺省并剔除旧 profile 的 turbo 引用，再参与生成。CAS 只有命名 `RzLockableStorageError` 触发重试，最多三次；其他 CAS 异常被吞，第三次冲突仍可能返回生成结果，而前序读取/JSON 解析仍能拒绝 Promise。用户 turbo 的 native 发布没有 await。没有把这些源码行为改写为统一重试或事务成功。

当前注册明确选择 `48363 obmEngineKeyboardCalMaxMem → 96236 → 2050`，setup 将真实 `OBMSpecs.supportedFeature` 传给 init，其中 `analogKeyboardV3=true`，旧 analogKeyboard 路径被关闭。`setActuationPoint` 先验证槽与非空数组，再按 15 条调用 `setMultiAnalogKeyActuationPoint(0,profileId,rows)`。报文 header 为 `[80,2,25]`，payload 前两字节是 dataType/profileId，每项五字节依次为 keyId、thresholdH 高/低、thresholdL 高/低，实际长度 `2+5n`；协议层最多 15 条，不额外做整数/阈值范围/空表判断，位运算及 Uint8Array 决定转换。

真实 Protocol25 对最终 FAIL 会抛异常，Electron 分支 finally 释放 HID mutex，因此不能把“OBM setter 没检查 resolved result.status”写成真实 NACK 成功。成功 ACK 仅在 packetSizeReturned>0 时解码，未另发 GET，也不核对返回键/阈值与提交项逐一相等。setter 在传输返回后标 SUCCESS，将**提交项**深拷贝进当前 obmJson.buttonAssignments.actuationPoint；255 更新全部已有项，普通键缺 assignment 则可能在硬件已写、缓存部分改变后抛错。每批异常写 FAIL/error 后继续，通用多批调用的后批成功能覆盖 status 而保留旧 error；当前 reconcile 通常每次最多 15 条，只检查 status。setter 内无 mutex、连接代次、取消或回滚，外层 guard 也不撤销既有写入。

本机保存沿既有 `89423 → LocalStorageHook → PostSet → 1101 GameMode projection → mappingEngineSetLocalStorage`。Hook 不等待 lockable CAS 和 PostSet；FW analog 分支异步生成原版 GameMode 投影。mapping 去重快照先更新，再 await facade，返回 bool 不被消费，失败后的同内容发布可能被去重跳过。当前 Host 的 bool/string callback、native storage queue、USB candidate 解析/发布及 analog 消费者复用共享正式证据；存储 callback 完成、解析完成、服务发布和设备确认分别判断。window storage 只投影当前产品/active GUID 与 profile name/GUID，合并 runtimeData 后发服务，不构成阈值写回。

取消/重连仍由外层 reconcile 核对设备、代次及 OBM 引擎；stale return 不取消此前副作用。现有 cleanup/shutdown 的已证实释放边界保留，未推断全部 JS/FFI callback 或 timer 已 drain。`78127` 的 analog skip-count 原生注册仍未决，不能用已确认的校准通知 128 provider 替代。现有 Rust `keyboard_latest_mapping` 普通/analog 生成、actuations/rapidTriggers 合并、自映射过滤与 hash，以及 `keyboard_actuation_write`/worker 提交路径均保留，本批未修改或重验；完整源码→独立 Rust→UI/后台、当前 DLL/设备对照和运行验收仍分别待完成。

## 756：可见性与释放

校准中注册 beforeunload、visibilitychange、screenlocked、systemsuspend 和 native tab-message。切换到其他 tab、document 不可见，或 native focus 消息查询窗口得到最小化/不可见时，只调用一次 `stop(false)`。异步窗口查询有 active 标记，effect 清理后不继续使用结果。effect 结束逐一取消上述监听和 native disposer；1 秒 timer 单独清除；向父页面登记的导航提示函数在卸载时置空。

上述监听 effect 会释放自身订阅和 interval；Generic 恢复另有 2/3 秒 timeout，不能据此称所有在途恢复已取消。generic manager 在 descriptor 执行后才检查 AbortSignal；V2 feature 没有 shutdown 方法。设备注销仅在返回 true 后移除硬件 listener，当前未证明原生通知回调/I/O 排空。

会话协议复用 [Host 收据](../../../.analysis/results/host-current/local-audit.json) 的精确 `/otfm_device_writeback`：当前 Host 4.0.896、Protocol25 90 字节帧、ACK 和 HID feature I/O/mutex 释放，复用 8 个原生函数而不重新捕获。该链只证明 feature I/O；异步硬件通知依据以下独立 provider 收据。没有执行厂商 JavaScript、应用程序、官方 DLL 或设备校准。

## 756：当前 USB 硬件通知提供者与退订

正式入口为 `python tools/workspace.py audit-host-local --query mapping-hardware-notification-chain`，结果位于 [Host 收据](../../../.analysis/results/host-current/local-audit.json) 的 `/mapping_hardware_notification_chain`。它从当前会话收据接续，精确复用 `/mapping_external_input_routing` 的共享 HID 读取、`/mapping_input_producer_eligibility` 的实际 USB 选择/已发布报告表，以及 `/mapping_input_shutdown_chain` 的完整设备移除边界；不重捕获整库。本批新增核对 26 个现有捕获中的分发/生命周期函数，复用 64 个函数，核实 17 个真实 PE 虚表槽。

| 边界 | 当前官方静态行为 |
| --- | --- |
| 实际注册 | 非 BLE `93182 → 22967.registerDevice` 先执行默认 `skipFilterDriver=false` 的 `addUsbDevice`，再注册硬件事件。Host 与原生 `277D0/57380` 选择 `usb/A8730`，不是根据功能名推定 BLE/external 构造器 |
| 分发与状态 | 当前 MappingEngine 1.3.17.2 导出经 `2E0010` 槽 `256/264/272/280` 入 engine runner，再经 manager `2E5A88` 槽 `48/56/64/72` 查身份并调整到 input 接口 `base+136`。注册只置 `base+640`；query 也只读此位，不证明端点打开或已收到通知 |
| 实际读取 | USB 主虚表 `2E8D38` 的初始化 `52540` 在 `connectHid+177` 为真时进入共享 `7D260` provider；与 `connectFilterDriver+176` 分开。当前 PID/container 选端点，`CreateFile(GENERIC_READ, OVERLAPPED)` 后用 100 字节双 buffer `ReadFile`，成功完成后按实际长度复制到设备队列；不是 feature send/get，也不是 304 字节 filter-driver 输入帧 |
| 记录 5/9 → 128 | USB 自身 `2E8DE8+24 → A8B20` 按已发布 `bluetoothLE` 选择 `raw[0]`/`raw[1]` 分类。756 显式报告表只有 `4/11`，未映射的 `5/9` 为类别 8，直接将完整 vector 交给 `7F17E`，此分支不剥离 prefix。所选非 BLE 记录因而保持原始字节进入现有 Protocol25 parser 的通知 `128`；这证明输入字节的静态处理，不代表已实测固件产生该通知 |
| Host 多订阅者 | 首个 URL 调用原生注册，成功后加入列表并等待 callback setter，但忽略 setter 返回的 bool；后续相同普通模式只加入 URL。硬件事件使用独立 callback，与完成 callback 分别持有；发送前核对实际 USB map、订阅 URL 与 webContents 存活，不含连接代次条件。硬件订阅方法没有独立 mutex/timeout |
| 普通退订 | Host 先移除 URL，最后一个才调用原生注销，失败不恢复 URL。`6B720` 清 registered/interval 标志后发布完成，不清 callback、不取消 ReadFile，也不等待已经复制并排队的 callback。MW 服务即使收到 false 也删除自身 deviceMap；上层 listener 只在 true 时移除，这些成功条件不能合并 |
| 完整设备释放 | 最后一个设备所有者移除才走 manager/main cleanup；USB cleanup 槽指向 `53090 → AC730`，逐端点 `123AA0` 执行 `CancelIo` 并关闭句柄，随后删除并 join 硬件线程。`A8FE0 → A896C → 656CA` join 设备线程后销毁状态和 callback。原版没有由这些局部操作提供通用的 owner-runner/FFI/Anne/Electron 队列排空屏障 |

原生 `7F17E` 只在全局使能、已注册且 callback 存在时发布；排队任务已保留 callback/vector/tick，执行时不重读注册位。普通退订完成不能保证之后没有迟到事件，URL/container 复用也不是连接代次 token。当前收据不扩大到音频 tap67、其他报告/传输分支或驱动安装；实际通知采集、断开/重连、跨队列释放与官方 DLL/设备对照保持未验证。

本批未创建临时副本、解压目录、日志或新分析会话；当前原件、正式收据和仍被引用的捕获保留。新增纯静态反例检查为 `python -m unittest test_host.MappingHardwareNotificationChainTests`（在 `tools/` 运行），覆盖真实订阅顺序、被忽略的 setter 结果、失败退订、源消费者、原生槽/interval0/HID gate、释放函数与依赖变化；不执行原版代码。

## 实际页面树与 Rust 对照范围

740/746 既有页面核对与 756 当前会话核对分别保留证据；本批未重跑全页 CSS 或 Rust 实现。部署、原件字节及实际挂载决定边界，不能由公共库含类推断产品使用。756 当前完整 owner 以会话收据为准。

| 产品/文件 | 完整所有者与页面边界 |
| --- | --- |
| 740 UI `9908.0c364f6c.chunk.js` | root `to [294348,305734)`，nav `[301748,302227)`：Customize → Actuation → Lighting → Calibration → Help |
| 746 UI `9908.23d31a59.chunk.js` | root `to [294038,304968)`，nav `[301116,301595)`；实际校准页面 `Aa [281661,283010)`：提示条 → 产品 SVG → 左列校准 widget → 用户显式打开的 `Ea` |
| 756 UI `3230.8e48a2ba.chunk.js` | root `rr [332486,345524)`；非 BLE nav 挂 `Mo`，BLE nav 不挂；页面连接器/组件 `Mo [309323,321461)`；导航校准状态由 `lr` 连接 |

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
| MW 回应 | 当前 `Mo` 消费校准列表、逐键状态、轴体表、轴体写入状态/token，另有停止完成 pending/token | `observe_keyboard_calibration_v2` 目前只在 ProductWorkspace / SourceProductWorkspace 转发到页面；未找到 Shell/设备事件分发对它的实际调用。Rust 页面还没有当前停止完成 token 消费者。有 reducer 和手工注入状态用例不等于收到真实设备事件 |
| 连接/模式初始化 | root 按实际 BLE 选择 nav，Normal Mode 决定 Factory 灰态；非 BLE root 明确 get-list → get-switch，页面另 get-switch，feature init 单独恢复本机列表 | 页面提供 `observe_calibration_v2_connection`，但外层也只有转发接口，没有实际状态发布调用；normal_mode 在初始化设 true、is_ble 保持未观测。额外 get-list 已有当前官方 root 依据；真实发布和读取顺序仍须接通 |
| 进入、保存、取消与 reset | Enter/Exit/Abort、真实通知 128、每批查询、reset 轮询、serial 记录及错误/释放均由 `4368` 和硬件消费者执行 | 当前页面只改变 phase、生成动作；后台缺口也使 stop(false)、reset 与窗口/会话取消无法完成硬件退出或恢复确认。不能把 UI phase=Idle/Saving 或动画完成视为设备成功 |
| 轴体与行程 | 串行 queue、RzDevice/连接代次保护、26 条批写、serial 存储、8k event 重新注册和全部配置档/DKS/控制器/板载槽 reconcile | 页面已有乐观 switch 表和整数行程 UI，但缺少上述实际 owner 与回传；本页 [轴体联动](#756轴体行程与映射联动) 仍是官方语义证据，不能描述成 Rust 已执行 |
| 自动转入校准 | 仅新 `switchTypeUpdateToken` 且 `Success` 执行 pending 自动开始 | Rust 用 `auto_start_after_switch`，在任意下一条 switch status 上消耗它；没有对应 token/请求身份字段。即使补上回传，也需拒绝较早请求/旧连接的 Success |

Rust 已将隐藏/最小化后的窗口状态、锁屏、系统挂起、退出等入口接到 `cancel_keyboard_calibration_v2`，见 [窗口焦点](../../../crates/razer-shell/src/shell/gamepad/controller_calibration_focus.rs) 与 [会话生命周期](../../../crates/razer-shell/src/shell/keyboard/keyboard_lifecycle.rs)。普通 blur 保留校准、只有真实隐藏/最小化才取消，这一判断与源码对应；取消命令的实际硬件消费者仍必须补齐。

### 756：视觉、弹层与状态细节

以下尺寸保留自先前获取的官方 `3021.b1237c23.chunk.css` 与 Rust 源码核对；当前已选中 `3230` 部署，本次仅接续会话语义，尚未逐项复核其 CSS/视觉差异。不能将下表旧快照的局部对应宣称为当前页面完整还原。

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

当前会话维护检查为 `python tools/workspace.py audit-family-function --family keyboard --function mw --product 756 --query keyboard-756-calibration-session-chain`。纯静态反例 `python -m unittest test_families.Keyboard756CalibrationSessionChainTests`（在 `tools/` 运行）13/13 通过，覆盖实际挂载、初始化顺序、模式/事件值、finally 与 serial、停止独立消费者、恢复交接、取消和共享证据边界。结果不执行官方 JS/DLL，不验证 Rust 或设备。源码链、独立实现、UI/后端接线、官方实际对照和运行验收仍分别保持未完成/未验证。

全列表与 all-data reset 的定向静态反例为 `python -B -X utf8 -m unittest -v test_families.Keyboard756CalibrationBatchChainTests`（在 `tools/` 运行），12/12 通过，覆盖真实全键集合与批次、返回记录边界、保存/发布顺序、空表短路、ACK、轮询、错误恢复、取消/重连、精确共享依赖与失败收据；不代表对应 Rust 或实际设备行为测试通过。

轴体批的定向静态反例为 `python -B -X utf8 -m unittest -v test_families.Keyboard756SwitchTypeChainTests`（在 `tools/` 运行），16/16 通过，覆盖整数/分支与键别名、26 条报文、发送摘要与读回区别、空/未知键 Success、部分写入后失败、serial 去重保存、队列代次、行程各消费者、全 profiles/实际 slots/BLE 分支、无请求身份的 UI token、恢复超时及共享依赖/失败收据。该查询只验证静态维护工具，未执行厂商 JS、Rust 产品或真实设备功能。

AppEngine/OBM 应用批的定向静态反例为 `python -B -X utf8 -m unittest -v test_families.Keyboard756AppEngineApplyChainTests`（在 `tools/` 运行），15/15 通过，覆盖实际引擎与能力、输入/布局/turbo、生成合并/hash、阈值报文、真实 NACK 抛错与 mutex 释放、ACK/GET 区别、提交项缓存与部分失败、未等待存储和去重快照、旧代次及精确共享边界。没有修改或重验现有 Rust，未执行官方 JS/DLL/设备功能，不能据此提升运行验收状态。

[旧校准纯协议测试](../../../crates/razer-device/src/keyboard/keyboard_legacy_calibration_tests.rs) 覆盖 Testing/Driver 命令、getter 第二字节与别名；[旧页面测试](../../../crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration_tests.rs) 区分预览样例和真实入口；V2 文件内已有 session/列表分离和失败 switch response 的状态测试。本轮读取其源码，未运行这些测试；它们也没有提供本轮当前官方 DLL 实际执行结果。

所有产品/布局/传输分支均须补齐从页面输入到后台实际执行、回应消费者和清理的自动化用例。旧版必须核对输入重定向部分成功、15 秒关闭、top getter 一次性、serial 保存失败、前台切换、modal epoch、断开和 stop/释放失败；V2 必须核对 Enter 失败、无 inputId 的计时重置、未知通知、Exit 失败仍发布列表的源码 finally、reset 的全键查询/轮询、混合轴体、过期 token、26/40 条分批、全部 profile reconcile 与连接代次取消。

预期报文、事件顺序和失败结果须来自当前官方原件或实际采集，不能由 Rust 被测编码器生成。实际 DLL/设备对照按 [verification.md](../../../.agents/rules/verification.md) 记录双方原始结果、差异和完整清理；校准数据 reset、轴体或校准写入只有在可记录并完整恢复原值时才能执行。当前没有这样的对照收据，相关设备校准、恢复与运行时验收保持未验证；静态页面对应和模拟状态测试不能替代它们。
