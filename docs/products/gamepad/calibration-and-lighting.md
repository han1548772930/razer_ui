# 校准、硬件灯光与软件灯光

## 校准状态机和采样

当前 2636 完整所有者是 `.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/main.f0ca95429464cd09cbdb.js` 的模块 66118，UTF-8 bytes `[1289001,1313647)`。该版本保留 V1/V2；2676/2684 当前 descriptor 选择 Version 3，其余六款双摇杆产品选择 Version 2。处理器工厂按真实 Version 分派，不能仅看到共享 V1 类就按固定时间放行下一步。

部件 ID 是 None=0、左摇杆=1、右摇杆=2、左扳机=3、右扳机=4。摇杆流程依次为右、下、左、上、旋转、完成；每一步将设备 ADC 读数与本步目标比较。V2 进入时先读取实际中心，超出 `CenterPointThreshold × Range.max` 的轴才以产品理想中心作为计算基准。采样只接受严格位于原件 Range.min/max 之间的值。

V1 的 `next` 直接处理当前步并前进；V2 的 `next` 要求 `isStepValid`。V2 的方向步骤须先达到该方向边界，再回到中心阈值内；旋转步骤检查四个边界。`MW_UPDATE_CONTROLLER_CALIBRATION_PROGRESS` 除 step、stepDesc、error 外，还包含 V2 的 isStepValid。UI 按该事件呈现有效性和下一步提示；2636 当前页面在方向步骤提供 Cancel，并非每一步都挂 Next 按钮。不能用播放完说明动画代替用户真实移动。

| 当前产品 | ADC 范围 / 理想中心 | 参数 |
| --- | --- | --- |
| 2629 | 0–1024 / (512,512) | Version 2；DeadzoneRatio 3；AllowedDeviationPercent 9；BoundaryTolerance 12；ValidityThreshold 0.3；CenterPointThreshold 0.1 |
| 2636、2647、2650、4133、4144 | 0–4096 / (2048,2048) | Version 2；前述参数中 ValidityThreshold 为 0.35，其余同列 |
| 2676、2684 | 0–8192 / (4096,4096) | Version 3；ValidityThreshold 0.35；TriggerMax 2048；独立按压/释放流程 |

这些是产品计算常量，不是设备当前校准值。2636 descriptor 中该对象在 main byte 2637735；2629 在其 MW `main.b468970129c30a905292.js` byte 2625198；2676 的 `main.1a6971221454393778e3.js` byte 2693370、2684 的 `main.35aed041f572227bf5d5.js` byte 2688018 保留各自 V3 全部参数。每款完整原件由其 `gamepad-mw-<PID>-current/acquisition.json` 管理。

V2 将实际移动换算成 -100…100 的 window storage 坐标，限幅后发布，回到中心的值设为 0；该预览坐标不同于原始 ADC，也不同于校准数据。UI 的坐标投影和旋转方向不能反向写回 ADC。处理器有独立采样 timer 和预览节流 timer；reset/stop 清空部件、步骤、采样、边界与有效性，并移除校准预览 window storage。

## 计算、写入、失败和取消

2636 `processCalibrationData` bytes `[1296483,1298184)`：先检查用户移动有效性，计算四次 center reading 的平均值；任一样本偏离均值超过 `DeadzoneRatio/100 × IdealCenterPoint` 就失败。边界从实际 min/max 朝该中心收缩 BoundaryTolerance（当前 12%）。`validateCapturedCalibrationData` 对数量不足或相对理想中心偏差过大只是记录 warning，不能擅自改成新的阻断条件。

写入按部件映射到 `setAnalogInputCalibrationDataAll`，仅填写当前部件的 Lx/Ly/Rx/Ry midpoint/min/max 或 Lt/Rt min/max。设备命令失败异常由 save wrapper 捕获并返回 false，随后报告 `Command Failed`；complete 发布 Step_Error 或 Step_6_Complete，最后 stop 清理。源码这里没有另加一轮独立 GET 逐字段比较，因此不能把“显示完成”解释成进行了另一个已验证的读回流程。取消/关闭走 stop/reset，不把半截样本写为最终数据。

V2 完整方法位置：start `[1305238,1306254)`，next `[1306254,1306549)`，prepareNextStep `[1307352,1307657)`，采样 `[1307862,1309146)`，有效性判定 `[1309322,1311799)`，预览换算 `[1312270,1312882)`，预览清理 `[1312882,1312993)`。Trigger 校准的动作、部件和模拟显示与摇杆流程分别处理，不能把二者的 UI 动画状态或 ADC 量纲合并。

## 2676/2684：V3 扳机校准与真实读回

2676 当前 V3 完整类位于上述 main bytes `[1324111,1338948)`，工厂 `getCalibrationHandler` `[1301124,1301511)` 明确选择第三代处理器。非扳机部分沿 V2；旋转步骤由 `rotateComplete()` 显式置有效并继续，普通 Next 在旋转步不推进。扳机流程为 Press → Release → Complete，不能套用摇杆右/下/左/上四个方向。

start 先查询 restingTriggerValue；有效时使用实读值，缺失或越界时源码明确使用 `Range.max` 作为这次计算基线。2676 `start [1327169,1327767)` 与查询 wrapper `s [1339352,1339509)` 一起说明该错误分支：wrapper 捕获查询异常并返回未定义。该计算基线不是设备实测结果，更不是通用默认写回授权。按压阈值为 `18% × TriggerMax`；释放阈值为 `restingValue - min(1% × travelHeadroom,11% × TriggerMax)`。值需达阈值且连续至少五个稳定采样；相邻变化不超过 travelHeadroom 的 1% 才累计稳定次数。满足后按压保持 2000 ms、释放保持 3000 ms，再各取五次真实采样、间隔 10 ms 求平均。有效性变为 false 时取消 hold timer，不能只等计时器结束便宣告有效。

第二次初读属于 `prepareNextStep [1328693,1329743)`：先将 movement 重置到 resting，再发独立查询；缺失、非有限数或越界时保留 reset movement，不重新套用 `Range.max`。普通 `getUserMovementPoints [1330318,1331138)` 则跳过无效查询的 movement/stability 赋值，继续按保留值验证有效性，因此不能仅因一次跳过的 poll 取消已有有效 hold。最终五次 hold 采样独立要求每次真实查询有效；其中任意一次失败都进入采样失败，不能复用 baseline 或上一条 movement。2684 的完整对应模块 36840 位于其当前 MW main `[1323068,1338374)`，上述阶段语义相同，产品阈值仍分别取各自 descriptor。

计算最终范围时，按压样本减去 `5% × TriggerMax`、释放样本加 `2% × TriggerMax`，源码随后 round/abs。范围要求有限数、min>Range.min、max<Range.max、min 小于本侧 PressThreshold、max 大于 ReleaseThreshold、宽度>300。2676 左侧 500/1490、右侧 600/1600；2684 左侧 400/1400、右侧 500/1500，不能合并成一套阈值。

`writeAndVerifyTriggerCalibration` 先验证并真实 SET，再 GET 同部件的 `getAnalogInputCalibrationDataAll`，逐一比较 min/max，并重新验证范围。读回失败、差异和传输失败各返回明确错误；只有读回相符才发 Step_3_Trigger_Complete。该确认链区别于 2636 V2 的普通完成路径。失败和 stop/reset 清采样、hold/update/user-movement timer 及预览 window storage；33 ms 节流坐标仅服务动画，不充当 ADC 或写回确认。

## 2636：设备硬件效果

2636 通过 `supportsLighting:true` 挂载 Lighting，硬件效果所有者为其 MW main 模块 70784 bytes `[1331417,1334682)`。效果导入先真实 GET；遇到 effectId 13 再取扩展静态值，实读 index>8 才进入 custom 模式。preset、custom 与配置草稿中的效果选择是分开的来源。brightness 的队列入口是模块 30525 内 `ot [1755579,1758379)`，真实设备 runner 为模块 85191 内 `He [1952298,1954981)`；其实际 GET 与源码 floor/ceil 百分比换算参与是否写入的判断。模块 97901 `[1334683,1336614)` 实际负责 polling rate，不能作为亮度队列证据。

设备适配器来源是 `.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/rzDevice25Sage.b9b57f19edfe6aa9f049.js`，模块 50207 bytes `[549,4807)`。**2676 虽包含共享 70784，却挂载 SagePC 并明确 `supportsLighting:false`**；SagePC 78786 继承 98773，而非声明扩展静态 methods 的兄弟 Sage 50207。不能给 2676 添加 2636 Lighting 页，也不能猜测兄弟类的命令来解释共享通知中残留的调用。

## 4115：软件灯光、调色板与退出

4115 在当前 bootstrap 中选择 `lightingType:"chroma"`，UI descriptor 带 `isWaveWithSpeed:true`、`WAVE_DIRECTION:"LeftRight"` 和普通 profile menu。该路线是持续的本机效果计算与帧输出，区别于 2636 的设备 preset getter/setter。

完整 chain 在 `.analysis/results/frontend/gamepad-main-audit.json` 的 `gamepad_lighting.palette_profile_chain`、`lighting_engine`、`native_engine`。4115 MW 原件为 `.analysis/inputs/frontend/gamepad-mw-4115-current/raw/apps.razer.com/synapse/products/4115/mw/main.8c755a3c43990d2c3900.js`；产品定义 69427 bytes `[1497444,1499733)`，共享硬件效果 helper 70784 `[1505544,1508787)`，其存在不改变 Chroma 挂载选择。UI 原件 `.analysis/inputs/frontend/gamepad-4115-current/raw/apps.razer.com/synapse/products/4115/ui/static/js/main.1a52fc92.js` 的实际 palette editor/profile save/refresh 消费者由上述 audit 完整保存。

软件引擎以 25 fps 生成帧，将 8×10 周边映射到 1×32 灯，按源码 BGR 转换和无颜色 sentinel 处理；Wave speed 保留小数。设备帧分为 25+7 两段 OUT；该发送路径没有 IN/device ACK，不能编造每帧读回成功。色板编辑、保存 custom colors、刷新 profile 色板和重建 effect handle 是独立动作；profile 内持久化的是颜色与效果参数，不是可跨进程复用的引擎句柄。

生命周期持有 device/engine/effect handle 与输入 bridge。暂停只是停效果/帧发布，不等于销毁 device；恢复重新激活当前 owner；退出执行源码的 spectrum 恢复，再移除效果、释放设备/引擎与通知/COM 资源。旧 generation 帧不得投给新连接。native `RzLightingEngineApi_v4.0.55.0.dll` 在此是分析对象：完整动作分发 RVA 0x1fd90、create device 0x20300、destroy device 0x20c50、add effect 0x21ac0、输入事件 0x27d60、terminate 0x2a5c0 的伪代码与交叉引用由 `native_engine` 记录；不得把它的生产调用描述成独立 Rust 实现。

来源与验证边界：[2636 UI/MW 收据](../../../.analysis/inputs/frontend/gamepad-mw-2636-current/acquisition.json)、[2676 MW 收据](../../../.analysis/inputs/frontend/gamepad-mw-2676-current/acquisition.json)、[2684 MW 收据](../../../.analysis/inputs/frontend/gamepad-mw-2684-current/acquisition.json)、[4115 UI 收据](../../../.analysis/inputs/frontend/gamepad-4115-current/acquisition.json)、[4115 MW 收据](../../../.analysis/inputs/frontend/gamepad-mw-4115-current/acquisition.json)。静态源码、Rust 纯测试与实际功能对照分别记录；应用、厂商 JS、真实校准和灯光写入均未在此执行。

## 实际挂载页面与 Rust 调用方

本节重新核对官方完整 AST 所有者与当前 Rust 生产调用方。九款根入口、所用 2636 CSS、2636/2676 MW 原件的实际字节和 SHA-256 已与获取收据匹配；它们是收据记录的部署身份与获取时稳定版本，不表示全部动态依赖或缺失 source map 已补齐。逐型号入口见 [产品矩阵](README.md#产品与适用边界)，不能由同一通用校准类或 Lighting 组件推定每款都有该页。

| 当前挂载分支 | 官方完整 UI 所有者 | Rust 页面与生命周期 |
| --- | --- | --- |
| 2629/2636/2647/2650/4133/4144 的 V2 校准页 | 2636 `main.d624c214.js` 的 `qM → ZM → XM`，完整 `XM [7490330,7495673)`；读取 step/part/isStepValid 和真实输入，Start 左/右、Cancel、完成与错误重试分别 dispatch | [gamepad_calibration.rs](../../../crates/razer-pages/src/features/gamepad/gamepad_calibration.rs) 的 `has_page_calibration` 精确选择六款；独立 state 持有步骤、有效性、输入和 generation，产品图/模拟器/方向层与错误 Dialog 有实际 renderer。错误窗口保留 focus/return_focus，关闭 Escape 和 backdrop，离页发 Stop |
| 2676/2684 的选择页与摇杆弹窗 | 2676 `np [7640097,7640651)`、`Mm [7623394,7623872)`、`hm [7616062,7623393)`；2684 为 `Ic [536679,537226)`、`cc [521301,521773)`、`lc [514052,521300)` | [selection](../../../crates/razer-pages/src/features/gamepad/gamepad_calibration_selection.rs) 持有 hover/部件选择，[摇杆 popup](../../../crates/razer-pages/src/features/gamepad/gamepad_calibration_popup.rs) 在 Calibration 与 Thumbsticks 都可打开，分别管理 focus、Start/Stop、错误重试及旋转计数 |
| 2676/2684 的扳机弹窗 | 2676 `ap [7639732,7640096)`、`tp [7634906,7639731)`；2684 `Oc [536320,536678)`、`Rc [531543,536319)`，仅 open 且选中部件时挂载，effect 清理 dispatch stop | [trigger_calibration](../../../crates/razer-pages/src/features/gamepad/gamepad_trigger_calibration.rs) 与 [renderer](../../../crates/razer-pages/src/features/gamepad/gamepad_trigger_calibration_renderer.rs) 独立处理部件、generation、实测移动、2/3 秒视觉计时、frame 取消和返回焦点，视觉 timer 不产生设备完成 |
| 2636 硬件 Lighting 与 4115 软件 Lighting | 根 nav 分别为 `[7507062,7507512)`、`[7518552,7518774)`；其余七款根没有 Lighting | [lighting](../../../crates/razer-pages/src/features/gamepad/gamepad_lighting.rs) 与 [quick_lighting](../../../crates/razer-pages/src/features/gamepad/gamepad_quick_lighting.rs) 区分 preset/custom、色板/取屏色、亮度与熄灯；切离页面取消控件编辑与页面观察，后台效果由连接 owner 管理 |

2636 `main.3d9eb3d1.css` 的 `.calibration-area [460291,460408)` 为最小宽 850px、margin `11px 30px 30px 23px`；产品图 `[460514,460644)` 最大宽 372px；说明区 `[462538,462642)` 宽 460px、最小高 44px；按钮规则 `[462793,462973)` 高 27px。这些尺寸在 Rust 有明确处理，但 PlayStation offset、edition 图片、现代弹窗、缩放媒体规则、错误/加载/图片失败和键盘行为仍需各自核验，不能把 2636 的 CSS 当成九款统一规格。

### V2 与 V3 摇杆：intent 尚无执行消费者

[calibration state](../../../crates/razer-pages/src/features/gamepad/gamepad_calibration_state.rs) 生成 `CalibrationIntent`，页面 `request_calibration` 实际发出事件；外层 [SourceProductWorkspace](../../../crates/razer-pages/src/features/workspace/source_workspace.rs) 与 [ProductWorkspace](../../../crates/razer-pages/src/features/workspace/product_workspace.rs) 提供 `calibration_intent/observe_calibration`。当前挂载订阅实际只转发 `TriggerCalibrationIntent`、`ControllerTriggerRequested`、Lighting 与 Chroma 请求，未订阅 `CalibrationIntent`；[WorkspaceEvent](../../../crates/razer-pages/src/features/workspace/workspace.rs) 与 Shell 也未接这一类摇杆校准任务。全 `crates` 的实际调用检索仅见上述接口和测试注入，未见生产观察发布者。

因此六款 V2 页及 2676/2684 摇杆 popup 的 Start/Next/RotateComplete/Stop 仍缺真实采样、计算、写入、失败与释放消费者。已有状态机、错误 Dialog、纯计算器及 V3 扳机链均不能补足该边界。页面离开发 Stop 是正确的调用方结构，但没有后台接收者时不能描述成已停止真实设备会话；低死区进入摇杆校准也受同一缺口约束。

#### 当前 Rust 事件边界的逐点核对

当前调用方的差异可以从实际源文件逐点复核：`source_workspace.rs:3401` 只订阅 `TriggerCalibrationIntent` 并发出 `WorkspaceEvent::TriggerCalibrationRequested`；`workspace.rs:139` 也只有对应的扳机事件枚举，`razer-shell/src/shell.rs:1545` 只处理该事件。`source_workspace.rs:1566` 和 `product_workspace.rs:1981` 暴露的 `calibration_intent` 只是读取页面状态，`observe_calibration`（`source_workspace.rs:1576`、`product_workspace.rs:1991`）只是写入观察结果，均没有把 V2 `CalibrationIntent` 转成后台任务的生产者。全仓检索只找到这些 accessor、观察入口和测试注入，未找到 V2 采样/SET/GET 的生产发布者；因此不能把已有接口名称或页面事件当作设备校准已经接线。

### V3 扳机：已有请求、采样与 SET/GET 链

`TriggerCalibrationIntent` 经 SourceProductWorkspace → `WorkspaceEvent::TriggerCalibrationRequested` → [Shell controller_calibration](../../../crates/razer-shell/src/shell/gamepad/controller_calibration.rs)，实际执行 Start/Poll/Cancel IPC。[服务 owner](../../../crates/razer-service/src/runtime/controller_calibration.rs) 持有 HID、采样/hold 时间、观察队列与 join，使用 [纯 Rust session](../../../crates/razer-device/src/gamepad/controller_calibration.rs) 和 [协议适配](../../../crates/razer-device/src/gamepad/controller_calibration_protocol.rs)。完成要求匹配 operation/part/node/product，SET 后 GET 同部件并逐项比对 min/max，才能发布 `device_write_verified`；stop 等待旧 worker 清理，连接身份和 generation 拒绝旧结果。这是实际存在的实现，不能将两种 intent 混为全部未接线。

读回失败或差异由`sample_and_advance → fail`保留Error progress；service将操作发布为failed，并分别记录`device_write_attempted/device_write_verified`，不能因SET已发送便确认完成。Shell先消费实际progress，再将terminal failed交`finish_trigger_calibration_submission(Err)`；页面的`finish_submission`停止视觉任务、清除移动并保存submission_error，不自行生成Complete。取消在实际传输前后及等待期间核对，SET已发生时仅标记attempted而非伪造恢复原值；关闭popup、Stop与设备写回确认是不同结果。

Shell worker 持有独立的业务结果与清理收据。Cancel 与 Shutdown 的错误分别保留；旧 pump 被移除或观察 channel 关闭后，worker join 仍向当前 Stop、后续 owner 和退出链传递真实清理结果及历史诊断。业务失败但清理确认的请求允许重试；清理未确认时不会仅凭线程退出允许同设备重新开始。

清理 owner 同时包含 workspace 身份、实际设备任务 scope 与 physical PID。Stop 只收当前 workspace 的 worker；快速 Stop 后 Start 仍在后台等待同一实际设备的清理收据，不会绕过已移交 Stop task 的前任。其他设备不继承该屏障；同 generation 但不同 owner 的迟到观察也会被拒绝。重复 Stop 合并前任 task；退出开始即保留每个 owner 的 completion，后台继续拥有真实 worker 和清理 task。退出失败导致取消退出时，失败收据仍在 Shell，不能因排空退出队列而使新 Start 绕过。

每条已创建 ServiceClient 的路径都显式执行 `finish_termination`，包含 resolve 失败和正常协议结束。服务清理确认、最终 OS／管道释放和历史诊断是三个独立事实。`ShutdownConfirmation` 仅由匹配请求 ID、单一 data/error 结果且 `stopped:true` 的真实服务回执产生；ServiceClient 在继续回收进程和 join 管道前保存它。关闭请求接纳、`is_stopped`、进程退出和 `finish_termination` 成功均不能替代该回执。

释放失败时校准 worker 继续持有实际 client，在后台重试已核实的终止链，完成真实释放后才结束。已收到匹配 ShutdownConfirmation 且最终 OS／管道释放成功时，收据保留先前 Cancel、Shutdown 调用及 OS 回收的异常诊断，并允许同 owner 重新开始；临时异常不会再被当作永久未释放。缺少真实服务回执时，即使 OS 已退出仍保持清理未确认并阻断。历史诊断随 worker／Stop 收据传递，在收据领取和退出时记录到现有日志；它们不会把业务失败改成成功，也不会冒充设备校准、原值恢复或新的设备实测。

仍有明确适用范围与差异：

- 服务 `verify_product` 当前仅接纳 2676/2677、2684/2685 的已核实有线物理身份并拒绝 dongle/BLE；interface、Report10 与长度继续以实际观察核实，不能扩展为无线校准已实现。
- Rust `start/prepare/poll` 分别实现上述阶段语义，内部类型区分实读 resting、源码 Range.max 计算基线、实际 movement 与 reset movement。Release 的初读缺失仍须重新建立稳定性并完成五次有效采样；Stop 或采样取消清除半截样本，重新开始只使用新查询。这些分支的纯测试不能替代官方实际结果逐项对照，也不能作为其他功能通用回退的授权。
- [焦点 owner](../../../crates/razer-shell/src/shell/gamepad/controller_calibration_focus.rs) 将真实 tab/window 状态交给扳机 popup，首次 inactive 不关闭，曾 active 后失活才关闭；摇杆 popup 的缺失后台、采样失败、旧部件、取消期间 SET/GET 和窗口释放仍需分别验收。

### 灯光：已有后台 owner 与未完成的读取核对

[lighting runtime](../../../crates/razer-pages/src/features/gamepad/gamepad_lighting_runtime.rs) 维护串行请求、profile 快照、revision 和取消 guard；请求经外层具名事件到 [Shell lighting](../../../crates/razer-shell/src/shell/gamepad/controller_lighting.rs)，写入前排本机保存，再交 [retained owner](../../../crates/razer-shell/src/shell/gamepad/controller_lighting_owner.rs) 与实际服务/设备协议，返回时核实身份、应答和观察结果。2636 的协议实现区分 Read、ReadBrightness、Initialize、Set 与 brightness 的先读后按需写；其存在不能使 2676 获得 Lighting 能力。

当前 `request_controller_lighting` 从已恢复设置排 Initialize，再排 brightness；它没有在该入口排效果 Read。Initialize 还包含官方 `50077 [1329404,1331416)` 的 PC-mode 选项查询/最多三次尝试及既有 profile 效果任务。因此“协议有 getter”“初始化已应答”与“页面确实导入设备效果”须分别核对，继续追官方首次 document 导入 guard、feature init 与结果消费者，不能自行把缓存或默认效果称为实读。

4115 色板与 Chroma 配置经 [controller_chroma](../../../crates/razer-shell/src/shell/gamepad/controller_chroma.rs) 独立保存、刷新和观察；服务 [lighting_engine](../../../crates/razer-service/src/runtime/lighting_engine.rs) 当前调用共享 Rust renderer，HID 帧提交位于 [controller_software_lighting](../../../crates/razer-device/src/gamepad/controller_software_lighting.rs)。两段 OUT 的提交结果与效果/亮度应答不同，没有每帧 IN/ACK；退出恢复 spectrum、停止帧、释放效果/设备/引擎及输入订阅分别确认。页面切换不应被误读为整个持续效果 owner 已销毁。

[controller_power](../../../crates/razer-shell/src/shell/gamepad/controller_power.rs) 读取 `profile.switchOffLighting/brightness`，观察显示器和 idle 事件后向 retained lighting owner 应用熄灯策略；它不是 Power 页的设备休眠 setter，后者见 [映射与电源边界](mapping-and-sensitivity.md#power分钟参数独立设备确认与生命周期)。

## 自动化与实际 DLL 对照边界

当前 [校准页面测试](../../../crates/razer-pages/src/features/gamepad/gamepad_calibration_tests.rs)、[V3 计算测试](../../../crates/razer-device/src/gamepad/controller_calibration_tests.rs)、[Shell 校准 owner 测试](../../../crates/razer-shell/src/shell/gamepad/controller_calibration.rs)、[IPC client 测试](../../../crates/razer-ipc/src/client_tests.rs)、[Shutdown 回执测试](../../../crates/razer-ipc/src/worker_shutdown.rs)、[灯光协议测试](../../../crates/razer-device/src/gamepad/controller_lighting_tests.rs)、[帧提交测试](../../../crates/razer-device/src/gamepad/controller_software_lighting_tests.rs) 及各 runtime/owner 内测试只覆盖其明确用例。V3 session 当前有 9 项纯测试，Shell 校准有 15 项模拟 IPC、线程与 completion 测试；实际执行与通过状态以统一验证收据为准，未执行项不能据测试存在而记为通过。测试注入 observation 不代表生产接线已补齐。

`start_uses_only_its_source_range_baseline_and_prepare_retains_reset_movement`、`unavailable_poll_keeps_source_movement_and_stability_without_starting_a_new_hold`、`missing_release_prepare_read_retains_reset_baseline_but_requires_fresh_stability_and_samples` 和 `cancellation_during_sampling_clears_partial_samples_and_restart_uses_only_new_reads` 对应阶段读取与取消边界。Shell 的 `start_after_stop_waits_for_the_exact_cleanup_receipt_and_preserves_its_failure`、`canceled_exit_retains_each_actual_owner_receipt_and_clean_owners_can_retry`、`retained_exit_receipt_waits_asynchronously_and_wakes_on_actual_completion` 与 `termination_failures_keep_the_actual_resource_owner_until_reaping_succeeds` 覆盖清理屏障、退出取消、异步完成通知和真实所有者保留；均不执行官方 helper、DLL 或真实设备。

IPC 的 `shutdown_joins_both_pipe_workers_and_reports_all_cleanup_failures` 验证匹配回执已确认后，管道错误仍返回且 confirmation 不丢；`service_rejection_and_wrong_reply_cannot_become_shutdown_confirmation_after_process_release` 与 `shutdown_confirmation_requires_the_exact_successful_service_reply` 拒绝服务错误、错 ID、缺字段、矛盾结果和非 true 回执。Shell 的 `final_process_release_cannot_replace_a_missing_service_teardown_confirmation` 证明最终进程释放不会补造原生服务确认；终止重试用例同时要求实际资源所有者保留到释放、保留两次历史异常，并在成功收据后允许后续请求。

每个产品/部件/传输分别覆盖正常、无效读数、部分初始化、真实移动、hold失效、SET/GET差异、未知回应、旧连接、取消/清理失败；灯光另覆盖色板、取屏色取消、效果切换、无 ACK 帧、暂停/恢复和退出恢复。预期来自当前官方原件或独立采集，实际 DLL/设备对照依 [verification.md](../../../.agents/rules/verification.md) 保存双方输入/原始结果、差异、原值恢复与释放收据。无法完整恢复校准数据或灯光状态时不得开始写入。本轮未执行官方 DLL、设备校准、应用或窗口验收，以上运行时与实际功能对照均保持未验证。
