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

## 四类引擎的实际协议边界

本节分别沿 PID92、99、165、163 的当前入口、特性表与设备实现 JS 追到设备方法。92 显式选择 Pixart，165 显式选择 Orca5，163 显式选择 Pixart3349；99 的入口没有相同 `useFeature` 调用，其默认特性表声明 `calibrationInterfaces.smartPixartCalibrationEngine={}`。四者均选择 `rzDevice25`；92／99 的 MW `claimInterface=0`，165 的 wired 165／dongle 166 也声明接口 0 并启用双连接，163 的 MW DeviceInfo 没有声明同一接口字段，须继续服从实际设备工厂与连接实例，不能从 UI 或其他型号补一个值。这里的 Protocol25 参数不证明其他产品的 BLE、配件或固件分支已验证。

令 C 为 calibration class；Pixart、SmartPixart、Orca5 为 0，Pixart3349 为 1；这四类的 sensor 均为 4。下列三元组按 `[数据长度,commandClass,commandId]` 记录，长度含 C 与 sensor，不是参数数组的长度：

| 设备方法 | 当前命令 | 实际 payload／结果 |
| --- | --- | --- |
| GET／SET state | `[3,11,131]`／`[3,11,3]` | GET 分配 `[C,4,0]`；SET 为 `[C,4,state]`；decoder 将前三字节投影为 class、sensor、state |
| GET／SET configuration | `[10,11,133]`／`[10,11,5]` | 固定 10 字节，前两字节为 C、4，余位复制参数；decoder 却逐项访问 `data[2]…data[10]`，命名为 `parm0…parm8` |
| GET／SET control | `[4,11,137]`／`[4,11,9]` | SET 为 `[C,4,type,stage]`；Manual 的 type=0，Start=0、Stop=1；成功且返回长度大于零才投影 type／stage |
| SET lift setting | `[4,11,11]` | `[C,4,liftMode,liftHeight]`；成功且返回长度大于零才投影这四字段 |
| SET lift→track | `[18,11,12]` | 固定 18 字节，前两字节为 C、4，后面复制输入并保留零填充；当前 setter decoder 只取 `parm0…parm4` |
| SET track→lift | `[18,11,13]` | 同为固定 18 字节；当前 setter decoder 取 `parm0…parm7`，不能因方法名自行交换两个解码器 |

这些 GET 方法的存在不表示 Apply 会读回。当前 lift-setting／双表代码中的 GET 命令字面量在逗号表达式里被丢弃，并未形成同名 GET 方法或调用链；不能据此设计一条“官方已执行 GET”的确认路径。各引擎 Apply 的实际完成依据见下文。

Protocol25 将报文放入 90 字节帧：数据长度在 offset 5，commandClass／commandId 在 6／7，payload 从 8 开始，校验和在 88。构帧只复制声明长度以内的输入；接收端按应答自身的长度截取 `data`，另外保留 `rawData`。返回 transactionId、命令类或命令号不符会重试；status byte=2 才是 SUCCESS，其他状态按 busy、failed、timeout、not-supported 等分流，达到失败边界会抛异常。引擎的显式 FAIL 分支与传输抛异常必须分别测试，不能默认所有失败都能到达 `calibration-notification`。完整传输和接收器所有权见 [Performance](performance.md)。

Pixart 的预设／手动转换确实构造九参数数组，而上述 configuration writer 的容量是 `2+8`；第九参数向 Uint8Array 索引 10 的写入不会进入发送数组。GET decoder 仍读取第九参数；当实际应答长度不超过 10 时，`parm8` 是缺失值，不能从原始 90 字节帧尾部补零冒充返回值。真实固件是否返回更长的数据必须实测，本次没有执行该对照。保留九参数作者结果、十字节发送数据及实际应答三份数据，才能定位差异发生在哪一层。

Protocol25 校准通知由 record 5 或双连接 secondary record 9 进入通知解析器，eventId=10，第三字节为值：Start=0、End=1、Halt=2、Reset=3、NormalSpeed=4、TooFast=5、Cancelled=6。适配器传出 `eventId`／`eventValue.value`，各 manager 再交当前 activeEngine。测量的鼠标左键来自另一条 inputevent；Escape 通过 virtualKey=27 的全局快捷键回调到当前状态。事件、setter ACK 与 configuration GET 不是同一结果。

## 实际页面挂载、布局与产品差异

PID190 的完整 `mS` 将欢迎提示与 Smart Tracking 面板直接挂在 body，面板没有外层左栏包装。说明后是非对称开关、帮助提示和普通／双阈值滑块，再显示硬件 Reset 的标题及说明。普通档位 1～3 使用无浮动数值提示的滑块；非对称上限可由产品 props 覆盖。三个滑块均传 `cancelMouseUpOnValueChange=true`：外部值在拖动期间变化时有独立处理，不能只复制 slider 的范围。

PID196 是单独页面分支：没有非对称开关与双阈值控件，只有两项 radio，提交值分别为 1、2，显示 LOW／HIGH 词条；`1mm`／`2mm` 是其 DOM ID，不能据此给其他引擎档位增加毫米单位。PID226 的同类 Smart Tracking 组件在独立 `4659` chunk；其主导航在 BLE 时移除 Calibration tab，Advanced tab 仍保留，不能把两者的 BLE 可见性合并。

PID70 的表面校准包含独立的记录卡片和选垫／测量浮层。记录先把默认项排前，再按 `calibrationDate` 排序；非默认项有删除和重新测量动作，新增项是否出现及可操作由 `isShowAddSurface`／`featureAccess` 控制。选垫浮层有选垫、准备、移动、测量、完成和失败分支，Start 还受 `calibrationReady=READY` 控制；完整 `render` 为 Philips／其他传感器以及左右手分别选择测量插图，不能只做通用的下一步页面。Complete 的 Done、本地记录保存及后续设备 Apply 是分开的动作。

CSS 的 `.calibration-body` 最大宽 600 px；欢迎卡背景与边框均为 `#2d2d2d`、圆角 5 px、内边距 20×30 px，关闭热区 36×36 px、内部图 20 px，hover／active 切换绿色原图，active 另降透明度。测量浮层相对主机顶部 105 px 并限制可用高度，选垫浮层宽 800 px、最小宽 546 px；非对称开关的 help 位移和 tooltip 顶部 25 px 均有独立规则。PID226 的页面规则位于 `4659.a39320d9.chunk.css`，必须与主 CSS 和 lazy 页面一起解析。

## Smart Tracking 与非对称中止

普通模式的 `trackingDistance` 为 1～3、步长 1。非对称模式分别编辑 `liftOffDistance`（2～26）和 `landingDistance`（1～25），步长均为 1。抬起值降低到不高于落下值时，控件把落下值改为 `max(1,liftOff-1)`；落下值提高到不低于抬起值时，控件把抬起值改为 `min(landing+1,26)`。这些是源码的档位编号，不应直接标成毫米。

PID170 的 SmartTrackingEngine 先 `setProximitySensorState(0,4,true)`，然后以 `Universal_Lift_Cut_off=1` 设置 `trackingDistance-1`。非对称分支先启用传感器，再设置 `Preset_Calibration_Hysteresis=4`，最后把 `[liftOffDistance-1,landingDistance-1]` 作为 configuration 写入。失败通过 calibration-notification 发布；当前完整引擎没有匹配 lift-setting getter 的写后比较。

Orca5 的 165、167、178、180、182、184、194 在普通跟踪先写传感器 false，218 写 true；Orca5Plus 全部写 true。它们的非对称分支均写 true，检查每个前序 setter 的 SUCCESS 才推进。缺少 landingDistance／liftOffDistance 时原件只记录参数缺失，没有发成功通知。管理器的 `apply` 没有 await 内部调用，非对称分支本身也没有把 `presetTracking()` Promise 返回给顶层；不能把调用入口返回当作所有设备命令完成。

Pixart3349 使用新 class=1、sensor=4：先写传感器 true，再把 liftOff 原值放入单字节 configuration。disable 写 false 并发布 `onDisable`，Reset 硬件事件另转为校准通知。它不使用 Orca5 的两个减 1 阈值，也不把 liftOff 当作表面 GUID。

具体到发送数据，Orca5 普通档位 k 的两次写入为 state `[0,4,0]`（218 为 `[0,4,1]`）和 lift setting `[0,4,1,k-1]`。非对称先写 `[0,4,1]`、再写 `[0,4,4,0]`，最后发送长度 10 的 configuration：`[0,4,liftOff-1,landing-1,0,0,0,0,0,0]`。该分支的成功不会额外发布统一完成通知；有错误才通知，且传感器失败／lift-setting 失败／configuration 失败的发布内容不同。缺少任一非零阈值只打印日志、不写 configuration，也不补成功事件。Orca5 没有表面采样、计时器、Stop 或先读原值再恢复的流程。

Pixart3349 的写入为 state `[1,4,1]` 和 configuration `[1,4,liftOff,0,0,0,0,0,0,0]`，liftOff 不减 1。它也只在失败／Reset 路径发布统一通知；`apply` 没有 await `presetTracking()`，manager 的转发入口同样没有返回内部 Promise。liftOff 为 0 或缺失时，原件不写 configuration，却把前一步传感器结果置入错误变量并发布 FAIL。disable 的 `onDisable` 也没有在完整 manager 中被转发为统一 `calibration_events`。取消一个页面请求不能据此推定传感器已禁用或原值已恢复。

其真实 reset 硬件事件会发布校准通知。界面首次介绍弹层的显隐另存在本地偏好键，关闭介绍不会修改设备跟踪参数；介绍记录与设备校准状态是不同对象。

## Pixart：测量完成与 GetData 分支

Pixart 管理器按 control type 选择 Preset 或 Manual。Manual 为 Ready → Initiating → Starting → Calibrating → StopCalibration → Ready，另有 GetData 和 Apply。Initiating 开传感器后注册 Escape 和本次鼠标输入、启动 35 秒 timer；第一次左键进入 Starting，写 `setProximitySensorControl(0,4,Manual,Start)`，等待 Start 事件进入 Calibrating。普通阶段 timer 为 60 秒。

Calibrating 收到 End 不立即发布测量数据，只置 `doSpecialHandling=true`；下一次左键清理临时监听后直接进入 GetData，避开再次 Stop。通常左键或 timer 到期进入 StopCalibration，写 Manual/Stop 后等待 End；只有 End 触发 `getProximitySensorConfiguration`，成功才把 jsonData 发布为测量结果，失败发布 Halt。GetData 同样读 configuration 后发布 End／Halt，但没有再写 Stop。

Preset 和 Manual 的 Apply 都是传感器 true → `setProximitySensorConfiguration`；以最后 setter status 发布 `onApply`，没有 Philips 的 calibrationData／threshold／Z/Q 计算，也没有写后 GET 比较。Ready disable 独立写传感器 false。测量新建、重命名、重新校准和 profile 保存沿共享 calibration maker 处理，设备取得的 configuration 与本地鼠标垫元数据分开保存。

PID92 的预设从自身 `sensorData.MATS_PRESETS` 按 surfaceId 查表，再取 `values[liftOff-1]` 的 Param0…Param8；完整表有 24 个表面、各 10 档。比如 Invicta_Speed 的第一／第十档分别为 `[10,16,131,0,0,0,0,64,0]`／`[10,16,131,0,0,0,0,24,0]`。这份表属于 PID92 的固定能力数据，不能把同名表面或相同引擎当作其他产品的表值证据。

手动结果保留 `Param0=parm0`、`Param1=parm1`、`Param7=parm8`。档位 k=1 生成 `[Param0,Param1,2,0,0,0,0,16,0]`；k>1 使用当前产品乘数 `[1,.9,.8,.7,.6,.5,.4,.3,.2,.1][k-1]`，生成 `[10,16,131,0,0,0,0,max(3,round(Param7×multiplier)),0]`。这里的 max 仅描述有限数分支：若 parm8 未返回，原版 round 得 NaN、`n<3` 不成立，写入 Uint8Array 的值变为 0；不能宣称缺字段也保证最小 3。发送仍受上述十字节 configuration 容量限制。

完整 manager 只转发两个子引擎的 `calibration-notification`，并未订阅 Apply 的 `onApply` 或 disable 的 `onDisable`。共享设备任务对选垫先区分 default／custom／preset：default 等待 disable，custom／preset 分别转换参数后调用 apply，但没有等待内部异步状态完成就把任务标为 completed。测量结束后另有直接 writer，等待 state SUCCESS 再等待 configuration SUCCESS，并在前后检查 AbortSignal；不能用前一种任务的 completed 代替后一种真实 setter 结果。

Escape 在活动阶段发布 Cancelled，停止 timer／临时键盘和鼠标监听；Starting、Calibrating 还写 Manual/Stop，再禁用传感器后回 Ready。Initiating／StopCalibration 依据 busy／escape 标志延后清理。manager destroy 从共享 inputevent 和 hardwareevent 移除具名回调，并调用两个子引擎 destroy；Manual destroy 只移除 Escape，不会额外替活动状态执行 Stop、清 timer 或禁用传感器。`lockMouse`／`unLockMouse` 固定返回 true，不能称为真实 OS 锁鼠标。

正常 End／GetData 成功只发布实测 configuration 并回 Ready，不写传感器 false；取消的 false 是明确的停用动作，也不是读取原状态后的恢复。PID92 的临时鼠标回调要求本机 productId、左键 DOWN=1、type=`mouse` 及 `toProcessInputEvent=true`；SmartPixart 的对应回调还接受 `dualLinkPrimaryPId`，不能把这两个完整回调统一。上游输入适配的容器过滤须另核对，不能用这处 PID 判断证明同 PID 的两个物理实例已区分。

## SmartPixart：普通、预设与手动跟踪

SmartPixart manager 按 SmartTracking=3、Preset=1、Manual=0 选择三个子引擎。普通 Smart Tracking 的 UI 值 1、2、3 映射为 liftHeight 0、1、2，liftMode=1；非对称使用 liftMode=2，(landing,liftOff)=(1,2)、(1,3)、(2,3) 分别编码 0、1、2。该分支先将传感器设为 false，再写 `setProximitySensorLiftSetting`，随后在子引擎发布 calibration-notification。当前 manager 只为 Preset／Manual 注册通知转发，没有为 SmartTracking 注册同类转发，不能推定该通知总到达统一 UI 消费者。

Preset 普通模式为 liftMode=3，按 surfaceDistance 选择固定八字节配置；迟滞为 liftMode=4，以 liftOffDistance 选五字节 trackToLiftData，以 landingDistance 选八字节 liftToTrackData。Apply 顺序为传感器 true → liftSetting → configuration，或先 liftToTrack 表、后 trackToLift 表。原件 Preset 表写入没有逐步核对每次返回值，最终 `onApply` 使用最后返回 status；manager 只订阅 calibration-notification，没有订阅这一路 `onApply`。

Manual 普通模式为 liftMode=5、迟滞为 6；启动先设传感器 true 并写所选 liftMode／liftHeight，等待左键后写 Manual/Start。状态为 Ready → Initiating → Starting → Calibrating → StopCalibration → Ready；Calibrating 左键或 timer 后总进入 Stop，不具有 Pixart 的提前 End GetData 分支。Stop 收到 End 后读取 configuration，只取 `parm0`／`parm1` 为实测 A／B，成功后才提交测量结束动作。Manual Apply 先开传感器，写单配置或先 liftToTrack 再 trackToLift；双表中前表成功才写后表，最后发布 calibration-notification。

手动参数按实测 A、B 和档位 k 生成：r=k−1、q=B+3r、d=(A−8)/10；首字节通常为 A−r×d，第十档且该值大于 10 时限制为 10；第二字节在 k>6 时为 56，否则 48。单表输出八字节，trackToLift 为五字节，liftToTrack 为八字节，尾部阈值按 k 分支为 15／13／10／8。非对称两向表有各自固定旗标，不能互相复制。以上表格和公式是传感器参数编码，没有毫米语义。

### SmartPixart 固定表与实测转换

PID99 当前普通预设和两个方向表的完整值如下。普通按 surfaceDistance 取行；非对称分别按 liftOffDistance、landingDistance 取行。三张表不能混用，Manual 也不复用这些固定值。

| k | 普通预设八字节 | track→lift 五字节 | lift→track 八字节 |
| --- | --- | --- | --- |
| 1 | `52,48,3,8,10,5,15,15` | `47,48,3,10,15` | `47,48,3,136,10,5,15,15` |
| 2 | `47,48,8,8,10,5,15,13` | `43,48,6,10,13` | `43,48,6,136,10,15,15,13` |
| 3 | `42,48,11,8,11,5,15,10` | `39,48,9,10,10` | `39,48,9,136,10,15,15,10` |
| 4 | `37,48,14,8,14,5,15,10` | `35,48,12,12,10` | `35,48,12,136,12,15,15,10` |
| 5 | `32,48,17,8,17,5,15,10` | `31,48,15,15,10` | `31,48,15,136,15,15,15,10` |
| 6 | `27,48,19,8,19,5,15,8` | `27,48,18,18,8` | `27,48,18,136,18,15,15,8` |
| 7 | `22,56,23,8,23,0,14,8` | `23,56,21,21,8` | `23,56,21,136,21,0,14,8` |
| 8 | `17,56,26,8,26,0,14,8` | `19,56,24,24,8` | `19,56,24,136,24,0,14,8` |
| 9 | `12,56,29,8,29,0,14,8` | `15,56,27,27,8` | `15,56,27,136,27,0,14,8` |
| 10 | `7,56,32,8,32,0,14,8` | `10,56,30,30,8` | `10,56,30,136,30,0,14,8` |

Manual 的完整输出可写成以下具名分量：p 为上文首字节，h=`k>6?56:48`，v=`max(q,10)`，t 在 k=1／2／3…5／6…10 时分别为 15／13／10／8。

- 普通：`[p,h,q,8,v,k>6?5:0,k>6?15:14,t]`。
- track→lift：`[p,h,q,v,t]`。
- lift→track：`[p,h,q,8,v,k==1?5:(k>6?0:15),k>6?15:14,t]`。

全部输出通过 Uint8Array 赋值，分数截断及超出一字节的转换属于原始编码行为，不能另作四舍五入或饱和裁剪。独立期望样本 A=52、B=3 时，普通 Manual k=1 为 `[52,48,3,8,10,0,14,15]`，k=10 为 `[10,56,30,8,30,5,15,8]`，与同档固定预设不同。非对称先发送 lift→track，成功后才在 Manual Apply 发送 track→lift；Preset Apply 则没有这一逐步 SUCCESS 门禁。

Manual Initiating 检查开传感器的结果，却没有检查随后 lift-setting 的 status 才注册监听；Starting 必须等待独立 Start 通知，setter ACK 本身只启动 60 秒 timer。Calibrating 接到 End 会先转发 `data=null` 的通知，公共消费者把页面投影为失败且不保存结果；它仍保持当前采样状态，之后左键／timer 走 Stop，只有 Stop 中 End→GET→A/B 才有可保存实测值。这条提前 End 路径与 Pixart 的 `doSpecialHandling`／GetData 明确不同。

### 手动采样的终止、恢复与保存

Pixart 与 SmartPixart 的 timer 和 Escape 必须按状态比较，不能统一成“取消后还原全部配置”：

| 当前状态 | Escape | timer／设备事件 |
| --- | --- | --- |
| Ready | 发布 FAIL／Cancelled，不发 Stop | Reset 可发布通知；没有活动测量 timer |
| Initiating | 标记 escape，必要时等当前开传感器操作结束，再清临时资源、写 state=false、回 Ready | 35 秒到期同样停用并发布 Halt；Reset 清临时资源后回 Ready，没有相同 false 写入 |
| Starting | 清临时资源，Manual/Stop→state=false→Ready | 60 秒到期执行相同写入，但该方法不额外发布 Halt；Start 通知才进入 Calibrating |
| Calibrating | 发布 Cancelled，清临时资源，Stop→state=false→Ready | timer／普通左键进入 StopCalibration；Pixart 提前 End 后的左键改走 GetData；Reset 回 Ready，不伪造读回 |
| StopCalibration | 清临时资源、state=false→Ready，不补第二次 Stop | timer 到期另发 Halt；任意 calibration-change 都清临时资源并回 Ready，只有 End 才 GET，Reset／Halt 转发相应通知 |

临时清理指 stopTimer、逻辑 unLockMouse、关闭本次鼠标处理标志及注销 virtualKey=27／具名键盘回调；不是恢复旧表面、lift mode 或原先 sensor state。正常 End 也没有复原整份设备配置。Pixart GetData 在 GET 前已执行同一临时清理，完整状态并未给在途 GET 增加连接代次／取消 guard；迟到 GET、Reset 与关闭必须作为独立竞态用例。

公共通知消费者先更新页面，再针对有数据的 End enqueue `ON_CALIBRATION_END`。Pixart 保存其九参数转换及 `sensorData`；SmartPixart 实测为 `{A,B}`，另由普通／非对称手动设置入口生成上面的表。保存 maker 创建 GUID、按 `MOUSE MAT` 去重、推进版本并保存本地记录，然后才排设备任务；recalibrate 查找旧名称并保留旧 GUID／名称，记录集合的该项只更新 calibrationDate，不能据此声称旧 sensorData 已全部替换。SmartPixart 的 manager apply 也没有返回内部完成 Promise；实测 A/B 保存、表参数生成、设备 Apply 和实际 ACK 必须分别对照。

共享屏幕状态观察只在锁定期间及解锁后 1 秒内抑制 Reset 的业务处理，不执行旧值恢复。对未被抑制的 Orca5／Orca5Plus Reset，消费者取该产品本地默认 calibration，设 `smartTracking.trackingDistance=3` 后保存并投影 UI；其他引擎 enqueue `resetCalibration` 的配置事件。这个“来源明确的硬件 Reset 消费”不同于读取失败后写默认值，不能从它推广首次连接默认覆盖。

SmartPixart 的单次 Escape／timer 清理与状态转换相联系；manager 的共享 input／hardware 监听使用匿名闭包，完整 manager 没有 destroy。共享 cleanupCalibration 直接调用 `manager.destroy()` 后才置空，因此在这类实例上不能声称 cleanup 已完成或订阅已释放。Orca5／Orca5Plus／Pixart3349 manager 同样没有 destroy；这是真实静态边界，不额外设计一条退出恢复链。

共享 cleanup 还没有对上述 screenlocked／screenunlocked 订阅执行相应 off。Pixart 的具名 manager destroy 与 SmartPixart／Orca5／3349 缺少 destroy 必须分别记录；只把 manager 指针置空、页面显示关闭或将任务标为取消，都不能代表底层临时写入、快捷键、timer 与共享订阅已经释放。

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

## 当前 Rust 接线与覆盖边界

| 层级 | 当前所有者 | 实际能力与限制 |
| --- | --- | --- |
| 页面 | `calibration/mouse_calibration_190.rs`、`mouse_calibration_70.rs`、`mouse_calibration_preset.rs`、`mouse_calibration_state.rs` | 已有 Smart Tracking、196 radio、表面记录、改名／删除、选垫与测量状态分支；拖动预览和松手提交分开，欢迎偏好关闭不提交设备设置。页面可见不等于对应引擎已有真实传输 |
| 动作与保存 | Shell `mouse_tracking.rs`、`mouse_philips.rs` | 消费 `MouseCalibrationRequested`，分别调度 tracking setter、本地保存、Philips 配置及测量。保持请求 scope／generation、序列、取消 guard 和 retained worker；保存重排与设备结果消费分别处理 |
| Smart Tracking | Device `mouse_tracking.rs`／`mouse_tracking/submission.rs` → Service `mouse_tracking_controller.rs` → portable 或 Windows `mouse/mouse_tracking.rs` | 当前产品能力**仅 170、204、226、45066**；使用真实 Feature HID、身份检查、传输锁与逐步 ACK。没有有效 lift-setting GET，ACK 不得投影为阈值实测成功 |
| Philips | Device `mouse_philips.rs` 及 `measurement` → Service `mouse_philips_controller.rs`／`mouse_philips.rs` → 对应平台与输入／硬件事件 owner | 当前类型化产品**仅 70**；包含传感器、calibrationData、threshold、测量控制、Z/Q 处理、事件、结果保存及取消清理路径，Mini 和 Manual 另有状态 |

`mouse_calibration_state::emit_calibration` 只有在上述能力存在时才构造相应类型化设备请求。Shell `dispatch_mouse_calibration` 对既无 tracking、又无 Philips 配置／测量的命令明确拒绝，错误为该产品分支没有已连接传输。因此，190／192／196／222／229／239 等 Smart Tracking 页面和其他 Philips、Pixart、SmartPixart、Orca5、Pixart3349 页面，不能因显示控件、改动本地草稿或已经发出通用事件而算作独立 Rust 完成。上文全引擎表记录官方语义，Rust 必须继续沿各实际产品入口、引擎、传输与结果消费者迁移，不能把 170 的实现直接扩成全家族。

已接入的 tracking setter 与本地保存是两个命令：页面依次发 `SET_SMART_TRACKING_VALUE`、`SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI`，后者没有设备 ACK 的含义。Philips 测量 End 没有 raw data 时当前页面进入错误，携带真实结果后才允许进入结束／保存链；取消、错误、真实测量结果和记录保存各有结果入口。切设备、切连接和退出会封口请求、撤销拖动预览并保留后台清理所有权；这些 Rust 生命周期不能反过来写成所有官方 manager 都拥有 destroy 或默认恢复行为。

当前纯测试已覆盖 Orca Plus 每个已接入产品的命令顺序、距离界限、逐步 ACK、部分失败、取消与传输目标；Philips 有写入链、测量状态及结果测试，页面和 Shell 另核对旧请求、保存与退出。它们只能证明相应测试条件；其余引擎迁移、各固件／传输／平台分支、完整 UI 交互以及官方实际 DLL 对照仍未完成。设备 ACK、实测读取、作者配置和草稿须继续独立保留。

四类新增协议与语义至少需要以下独立用例；本次只做静态逆向和文档核验，未把它们写成已经通过的测试。

| 测试组 | 独立期望与必须比较的状态 |
| --- | --- |
| Protocol25 编码／解析 | 逐条核对命令三元组、class、sensor、固定长度、零填充、90 字节帧与校验；错 transactionId／命令、各 status、短应答、异常及重试结束分别比较 |
| Pixart 九参数边界 | 分开核对作者九参数与发送八参数；GET 返回 10／11 字节、缺 parm8、NaN→Uint8 的零值和正常最小 3 分支，不以补零消除差异 |
| Pixart 产品表 | 24×10 个 PID92 原始条目及首／末档样本；其他型号按自身原件取表，未知 surfaceId／越界 k 不回退到一个默认鼠标垫 |
| SmartPixart 固定与实测表 | 三张十档表逐字节核对，A/B、k=1／6／7／10、分数及一字节溢出；普通／非对称 lift mode、非法迟滞组合与两方向发送顺序分别比较 |
| Start／End 与结果消费者 | Start setter ACK 后仍等 Start 事件；提前 End 在 Pixart 只置标志、SmartPixart 先发 null 数据；正常 Stop End 才 GET；Stop 中非 End 通知不伪造测量数据 |
| 取消／timer／关闭 | 每个活动状态注入 Escape、各 timer、部分写入后取消、destroy 与迟到 GET；分别记录 Stop、false 写入、timer、快捷键、鼠标标志、共享订阅和最终状态 |
| Orca5／3349 | 165 普通 false 与 218 true、减 1 与原值、class 0／1、缺参数／零值、逐步 FAIL 与抛异常、成功没有统一通知；禁止以调用返回或缺错误事件推定成功 |
| 保存与 Reset | 本地完成先于设备任务、不同版本和保存失败、recalibrate 的名称／GUID／日期／sensorData 各字段；锁屏、解锁窗口与真实 Reset 事件分别验证 |

实际官方 DLL 对照仍须经过独立维护入口，在相同原值、设备身份、固件、传输和输入序列下执行对应功能，并保留 DLL 原始结果、Rust 结果、事件、真实应答／读回及各自恢复／释放收据。上述 setter 没有独立 GET 的分支按其实际确认契约比较；写入测试所需原值和安全恢复能力必须另外核实。不能用官方 HID 重放成功或两个 manager 都没有通知证明整条校准功能已一致。

## 原件与复核位置

- PID70 引擎：`.analysis/inputs/frontend/mouse-mw-70-current/raw/apps.razer.com/synapse/products/70/mw/philipsEngineManager.2703b2e8e046279839a4.js`。Ready `[2021,3881)`、Initiating `[4178,8202)`、StopCalibration／统计 `[15357,20285)`、Manual Apply `[20582,22412)`、Mini Apply `[45193,46825)`、Preset Apply `[52274,54086)`、manager `[55118,57004)`；本页复读以上原始区间。
- PID70 转换／保存：`.analysis/inputs/frontend/mouse-mw-70-current/raw/apps.razer.com/synapse/products/70/mw/6120.7fff31f083f844383e26.js`，手动转换 `[1035212,1036484)`，测量结束保存 `Rt` `[1551805,1554435)`。
- Smart Tracking 实际引擎：`.analysis/inputs/frontend/mouse-mw-170-current/raw/apps.razer.com/synapse/products/170/mw/9318.22c896a3706c9ac20036.js`，完整模块 69318 `[924,4537)`。
- Pixart：`.analysis/inputs/frontend/mouse-mw-92-current/raw/apps.razer.com/synapse/products/92/mw/pixartEngineManager.4696a25592494d62bb34.js`，Ready `[1801,3520)`、Initiating `[3817,7060)`、Starting `[7357,10728)`、Calibrating `[10731,13601)`、Stop `[13898,17645)`、Apply `[17942,19117)`、GetData `[19414,22576)`、Manual `[22597,25038)`、Preset `[25048,29271)`、manager `[29323,30985)`。
- SmartPixart：`.analysis/inputs/frontend/mouse-mw-99-current/raw/apps.razer.com/synapse/products/99/mw/smartPixartEngineManager.06281ae41ff840612091.js`，普通引擎 `[1029,5629)`、Preset `[5639,10621)`、Manual 完整状态及引擎 `[10631,31837)`、manager `[31889,33413)`；同目录 `6120.87cdb69a54dbbee728ed.js` 的常量模块 `[467637,469496)`、固定表／手动转换 `[2351237,2353543)`、选择与事件 `[2354392,2357098)`、普通转换 `[2358399,2359399)`、预设／手动调用及 cleanup `[2359405,2360873)`。
- Orca5：`.analysis/inputs/frontend/mouse-mw-165-current/raw/apps.razer.com/synapse/products/165/mw/orca5CalibrationEngineManager.4a67b1e63c3d01a6922b.js` `[1354,4512)`；218 差异为 `.analysis/inputs/frontend/mouse-mw-218-current/raw/apps.razer.com/synapse/products/218/mw/orca5CalibrationEngineManager.fe710a2501f870054f31.js` `[1862,4520)`。
- Pixart3349：`.analysis/inputs/frontend/mouse-mw-163-current/raw/apps.razer.com/synapse/products/163/mw/pixart3349CalibrationEngineManager.115aea4c73bd29878d0a.js` `[1345,3680)`；185／211 完整 class 与本节操作相同，仅打包绑定不同。
- Smart Tracking UI：`.analysis/inputs/frontend/mouse-190-current/raw/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js`，完整 `mS` declarator `[5422282,5425028)`；196 的 `main.e5afc608.js` 完整 radio 页面 `iS` `[5397354,5398348)`；226 的 `4659.47cd5287.chunk.js` 完整 `ai` `[88815,91496)`，导航完整 class `[131243,141873)`，后两者分别位于各自产品的 `mouse-<PID>-current/.../ui/static/js/`。
- PID70 表面 UI：`.analysis/inputs/frontend/mouse-70-current/raw/apps.razer.com/synapse/products/70/ui/static/js/main.8f24b6a1.js`，选垫／测量 class `[5554294,5562390)`，记录与欢迎 class `[5563147,5576873)`；对应 `main.dd229426.css` 保留欢迎、浮层、选垫与 tooltip 规则。以上范围均按完整 AST 复核，不使用词条命中附近的片段代替完整组件。
- 完整定位与字段表：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries.calibration.roles`、`engine_states`、`fixture_expectations`、`save_boundaries`、`event_adapters` 和 `queries.tracking.products`。

四类协议追加复核分别使用各自 `mouse-mw-<PID>-current/raw/apps.razer.com/synapse/products/<PID>/mw/` 原件。下表均为完整设备方法的 UTF-8 半开字节范围；各行方法分别核对收据及原始字节，没有从 PID70 的 Philips 方法名推导：

| PID | 公共 chunk | SET state | GET／SET configuration | SET control／lift setting | SET lift→track／track→lift |
| --- | --- | --- | --- | --- | --- |
| 92 | `6120.e883f04535036eddbd4d.js` | `[841703,842123)` | `[843007,843400)`／`[843400,844015)` | `[848074,848531)`／`[849356,850000)` | `[850000,850778)`／`[850778,851604)` |
| 99 | `6120.87cdb69a54dbbee728ed.js` | `[841681,842101)` | `[842985,843378)`／`[843378,843993)` | `[848052,848509)`／`[849334,849978)` | `[849978,850756)`／`[850756,851582)` |
| 165 | `6120.f94e73c8ca92ae272492.js` | `[840849,841269)` | `[842153,842546)`／`[842546,843161)` | `[847220,847677)`／`[848502,849146)` | `[849146,849924)`／`[849924,850750)` |
| 163 | `6120.74eaf108b0b312289007.js` | `[841498,841918)` | `[842802,843195)`／`[843195,843810)` | `[847869,848326)`／`[849151,849795)` | `[849795,850573)`／`[850573,851399)` |

PID92 公共 chunk 的命令声明区间 `[788819,790102)`、configuration decoder `[791835,792072)`、构帧 `_createDataSend` `[811316,811610)`、接收 `_getUSBTransferInResult` `[812845,815888)` 和 `sendCommand` `[815888,820107)`提供容量、长度与异常边界。其 `rzHardwareEventsProtocol25Parser.c99ec39102a4bd16f8b5.js` 完整 class 为 `[665,7706)`，对应公共 chunk 通知常量模块 50652 `[992287,1003197)`。PID92 主入口为 `main.d12eb2e6066d05a84cec.js`，DeviceInfo `[160686,160988)`、Pixart 选择调用 `[181424,181489)`；完整 sensor 表 declarator `[137537,159358)`、乘数 `[159359,159391)`，所有 24 个表面记录均处于该完整表中。

PID99 `main.7e37ac20c6b4d2d3f809.js` 的默认引擎 pair `[125823,125878)`、DeviceInfo `[157640,157934)`和完整启动回调 `[175476,175793)`一起核对。SmartPixart 表／公式区间在对应公共 chunk `[2351237,2353543)`，普通／预设／Manual 入口转换完整 declarator 分别为 `[2358368,2359401)`／`[2359402,2359571)`／`[2359572,2360127)`；完整 manager `[31889,33413)`、Manual Calibrating declarator `[20458,23108)`、Stop `[23403,27176)`位于前述 `smartPixartEngineManager.06281ae41ff840612091.js`。PID165 `main.c1690aa9826e996b22d0.js` 的 Orca5 选择 `[159719,159789)`、DeviceInfo `[138307,139214)`；PID163 `main.40531c9424e9355daf88.js` 的 3349 选择 `[182243,182318)`、DeviceInfo `[158352,158996)`分别复核，不从同族门禁推断。

保存／终止还复读了完整 owner：PID92 公共 chunk 中九参数转换模块 98570 `[1029538,1035295)`、End 保存 `[1550615,1553245)`、配置保存 `[1553854,1555519)`、选垫 writer `[1736778,1738630)`、测量直接 writer `[1738630,1740413)`；PID99 End 保存 `[1550729,1553359)`、配置保存 `[1553968,1555633)`、屏幕状态 `[2353969,2354359)`、事件消费者 `[2355434,2357098)`、cleanup `[2360128,2360163)`。PID165／163 的实际 Reset 消费者分别为各自公共 chunk `[2365870,2367552)`／`[2366550,2368232)`，不能只凭公共模块内存在 Orca 字符串证明实际消费者行为。

本批鼠标专项复核对报告中的 44 份 JS 原件、996 个完整 owner 验证了实际长度、SHA-256、函数体字节和 AST 边界；本页新增 UI／CSS 另依对应获取收据核对。输入版本和缺失 source map／引用闭包仍以各 source set 为准，不能由局部解析成功推断全部输入齐全。

源码获取、逆向语义、Rust 实现、UI／后端接线、静态／模拟验证和运行时验收分别记录。本次未执行厂商 JS、官方 DLL、测量或设备写入；全部功能与当前官方实际 DLL 的结果、真实应答、错误、取消、恢复及释放对照保持**未验证**。尚未迁移的产品引擎和上述未连接传输不能以模拟测试或两边均未连接抵作完成。
