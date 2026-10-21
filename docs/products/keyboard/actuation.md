# 触发距离、Rapid Trigger 与模拟键协作

本页从当前实际挂载的 Actuation 组件解释模拟键选择、触发点、Rapid Trigger、Snap Tap 和双键优先级。映射编辑器中的双段触发见 [customize.md](customize.md)，磁轴校准及 756 的轴体行程联动见 [calibration.md](calibration.md)。UI 选中值、配置档内容、板载命令应答和硬件事件分别说明。

## 产品分支与实际组件树

当前实际挂载 Actuation 的 PID 为 580、614、642、678、679、688、719、720、721、728、740、741、742、746、747、756。580/747 的导航名称经过 `toUpperCase()`，实际仍在 nav 数组且由 active_view 渲染。每款的原件、获取收据、入口和实际组件字节位置见 [逐 PID 产品页](products/)。以下分组来自这些原件的完整顶层函数和连接器，未根据名称或通用库符号推断能力。

| 产品 | 实际页面 |
| --- | --- |
| 580 | 专用 keypad 图；左 Snap Tap、Actuation，右 Rapid Trigger；图定位 `calc(50% - 165px)`，不另挂 Continuous 控件 |
| 747 | 专用 keypad SVG，读取本产品 minWidth/minHeight；左 Snap Tap、Single Key Snap Tap、Actuation，右 Rapid Trigger、Continuous、Dual Keypress Priority；图定位 `calc(50% - 200px)` |
| 614、642 | 产品图；左 Snap Tap、Actuation，右 Rapid Trigger；没有另挂 Continuous Rapid Trigger 控件 |
| 678、679、688 | 产品图；左 Snap Tap、Actuation，右 Rapid Trigger、Continuous Rapid Trigger |
| 719 | 产品图；左 Snap Tap、Single Key Snap Tap、Actuation，右 Rapid Trigger、Continuous Rapid Trigger、Dual Keypress Priority |
| 720、741、742、746 | 与 719 的控件顺序一致，产品图明确传 `useProductSvg:true` |
| 721、740 | 上述六控件，SVG 图另传本产品 `SVG_CONFIG.minHeight` |
| 728 | 六控件；SVG 最小高度取配置或 `360`；Factory 提示限定 Windows Mode，内容灰态仅依 Factory 判断 |
| 756 | 六控件；SVG 取本产品最小高度；Normal Mode 挂电源模式提示；内容灰态条件为 `normal&&(factory||ble)` |

614/642 也在 Actuation 顶层按 Factory 身份挂说明和灰态。756 的说明条件使用源码的 `a=!!ble,o=a&&!factory,r=!a&&(factory||o)` 再与 normal 相与；其 BLE 提示支路不能依据文案存在自行视为已挂载。

## 精确来源

679 UI 前缀为 `.analysis/inputs/frontend/keyboard-679-current/raw/apps.razer.com/synapse/products/679/ui/static/js/`，版本和哈希见 [收据](../../../.analysis/inputs/frontend/keyboard-679-current/acquisition.json)。下表均为原始 UTF-8 字节半开区间。

| 原件 | owner |
| --- | --- |
| 679 `main.194d8a7b.js` | top `CU` `[8095538,8097076)`；Actuation `Wh` `[8050168,8058294)`；Rapid Trigger `Jh` `[8059354,8068960)` |
| 同文件 | 选择产品图 `OU` `[8085160,8092570)`；continuous `uU` `[8093365,8093782)`；Snap Tap `DU` `[8094542,8095537)` |
| 同文件 | converter `31867.J` `[886253,887022)`；actions `62905` `[6093397,6094614)`；触发 reducer helpers `[7383207,7386628)`；Rapid helpers `[7390390,7392616)` |
| 756 UI `3021.0f2a9d4a.chunk.js` | Actuation `Zi` `[196018,205101)`；Rapid `tn` `[206315,215747)`；双键录制 `Zn` `[241401,247245)`；单键录制 `oa` `[253194,259502)`；top `[261375,263080)` |
| 756 MW `6737.90f8e2e80827f3801f8e.js` | 双键优先级 `31982` `[1002858,1006162)`；中断键 `56839` `[1006163,1011026)`；通用 feature `61450` `[1026888,1078405)` |
| 同 MW 文件 | 双键 Snap Tap runner `gi` `[2026181,2028078)`；单键 set `fi` `[2028078,2029960)`；单键 toggle `yi` `[2029960,2031892)`；Continuous runner `Xe` `[1936651,1937772)` |

756 UI/MW 获取元数据分别在 [UI 收据](../../../.analysis/inputs/frontend/keyboard-756-current/acquisition.json)、[MW 收据](../../../.analysis/inputs/frontend/keyboard-mw-756-current/acquisition.json)。756 的命令实现位于 `rzDevice25.40fe6f559bba152076d4.js`，下文协议表直接对应该原件。

## 数值来源与产品规格

converter 将协议值除以产品 `unit`，四舍五入为 slider 单位，再以十分之一显示；反向将 slider 乘 unit。614/642 的页面显示另外叠加 `offSet:1.5`，所以不能把同一 slider 值套用于全部产品。

| 产品 | unit / min / max / defaultValue / minBreak | 额外分支 |
| --- | --- | --- |
| 580 | `12 / 0 / 255 / 4 / 12` | eventDataSize=255，源码 `useTwoBytesActuation=true`，offset=1.5；不能由数值小自行改判单字节 |
| 747 | `1638 / 1638 / 65520 / 32760 / 256` | eventDataSize=65535，双字节 |
| 614、642 | `12 / 0 / 255 / 0 / 12` | 单字节，offset=1.5 |
| 678、719、728 | `1638 / 1638 / 65535 / 19656 / 256` | 双字节 |
| 688 | `1638 / 1638 / 65520 / 19656 / 256` | 双字节 |
| 679、720、741 | `1638 / 1638 / 65520 / 19656 / 1638` | 双字节 |
| 721、740、746、756 | 同上一行 | High Sensitivity 源规格另有 defaultValue=`13104`，其余界限相同 |
| 742 | `1638 / 1638 / 45864 / 19656 / 1638` | low-profile、事件范围 45864、图示行程上限 2.8mm |

这些值来自各产品 main 的 `analogSpecs` 对象，是转换/默认构造规格；它们不是本次设备读取。`createDefaultActuation` 还按 High Sensitivity GUID 选择对应规格，构造 `{0:default,1:max(default-unit,minBreak)}`。缺少当前映射时控件可以显示该 UI 默认值，但不代表已经读到设备同值。

## 选择产品图

产品图使用输入身份和当前产品/布局的按钮表，排除预设热键、禁止键及不能参与的特殊映射。全选与逐键选择有不同 reducer：全选额外应用产品 exclusion 表；当前档切换清选中集合。图层同时显示实际配置档的触发点/Rapid Trigger 与独立按压观察值，选择动作本身不发送新的设备配置。

679 `OU` 挂载注册 resize、产品滚动容器 scroll、mousedown 和 keydown。卸载移除 resize、mousedown、keydown；当前函数未显式取消该 scroll listener。选择列表变化时控件维护最近加入键的栈；不是简单读取数组第一个值。

## 触发点编辑、同步与重置

679 `Wh` 调整值先经 converter。硬件 Adjustment Mode 正在运行时只显示已有提示；否则更新选中按钮的 UI actuation，再以当前按钮和 mappingList 生成保存动作。全选状态另更新 `actuationAllKeys`。

选中集合含不同值时，组件统计普通层 AnalogInput 第一段参数，按最近加入键或出现频次决定代表显示。没有匹配项时采用产品 UI 默认；不发起“默认值写回”。reset 按已有映射、保留 Rapid Trigger、禁用映射及其他输出类型逐项计算，随后进入 `ON_SET_KEYMAPPING`；不能把它实现成清空全部映射。

Sync all/selected 使用不同键集合，all 排除禁止映射。679 的同步参数是 `{0:make,1:make}`。2 秒“同步”反馈与 2.5 秒禁用切换只是前端 timer，不是设备确认。reducer helpers `qa/Qa` 生成 mapping list，并在对应路径另外发送 `ON_SYNC_GLOBAL_ACTUATION`；保存通过 [映射中间件和板载队列](customize.md#板载同步返回值与取消) 完成。

756 `Zi` 增加真正的 `getMaximumActuation`：只有产品声明 dynamic maximum 且有选中键时，读取所选键有效 `keyTravel` 的最小值，经 converter 与产品 max 取较小值。控件保留视觉最大值，同时限制可编辑值并显示缩短行程说明。它没有仅依据“Generic”文字推断所有键相同行程。行程修改还会调整其他配置档、DKS 和板载槽，详见 [756 行程联动](calibration.md#756轴体行程与映射联动)。

## Rapid Trigger 和 Continuous Rapid Trigger

Rapid slider 与灵敏度反向映射：679 使用 `10*parseFloat((max-value+0.1).toFixed(1))` 转成协议值，并提供逆函数。下压调整与开关采用 500ms debounce；未启用独立 upstroke 时 break 跟随 make。保存同时携带选中键、启用状态、当前默认触发值及适用的预设身份，不只写一个开关。

控件 mount 的本地 `isEnabled:false` 是初始化 UI 状态。随后从普通层映射第一段 Rapid Trigger 统计代表值；缺少参数时重置本地显示。不能用初始 false、空映射或 slider 数值认定设备实际关闭。新版控件还存在 `every()` 返回布尔值再尝试读 `.continuous` 的源码写法；理解实际显示时需继续依据后续按键 mapping 的赋值，不能假设该布尔值就是协议对象。

Rapid reducer 的 set 分支先发送 `ON_SYNC_GLOBAL_RAPID_TRIGGER`，再发送 `ON_SET_KEYMAPPING`。Sync all/selected 与单次 slider 修改的参数不同；全选还维护 `rapidTriggerAllKeys`。设置独立 upstroke、全键同步及预设档均有各自判断。

Continuous 控件单独读 `continuousRapidTriggerReducer.isEnabled`，Adjustment Mode 阻止修改。MW runner 先读取当前档与实际板载槽，调用 slot 1 和同 GUID 槽的 `setAnalogKeyContinuousRapidTrigger`；取消标志在后续检查。756 BLE 分支直接返回任务 completed，未发送该命令；因此这里的任务完成不能描述为 BLE 硬件已写入。

## 双键 Snap Tap 录制

旧控件的开关使用 500ms 节流/去抖包装；新版可配置多组。756 `Zn` 的实际状态包括录制阶段 READY/KEY1/KEY2、提示状态、编辑组 ID、暂存组、当前档与布局。最多 4 组，新增默认 mode=`LAST_INPUT`，每组具有稳定的键身份和当前页面维护的组 ID。

进入录制执行 disableMapping、注册 native no-browser-input handler、启用输入重定向。若 Single Key Snap Tap 开启，先发临时关闭并在恢复时按记录恢复。输入来自 keyup 或 `inputredirect`，按 scancode/flag 及产品布局匹配；特定反斜杠 HID 有专门处理，重复键/空键和 keypad directional exclusion 均先校验。

录到第二键才提交完整组并显示 3 秒反馈。点击外部、窗口 blur、beforeunload 对未完成组采用不同过滤/保留策略；删除组会重排页面 ID；修改 mode 直接走组保存。Adjustment Mode 下新增/编辑/删除显示原有提示，不创建另一套确认。

恢复录制调用 enableMapping、注销 no-browser handler、停止输入重定向，必要时恢复被临时关闭的 Single Key Snap Tap。effect 移除 mousedown、keyup、blur 和 inputredirect；卸载额外调用最新恢复函数。当前源码的提示 timer 与事件解除不是同一动作，不能以 component unmount 推断在途异步命令均被取消。

MW `analog_obm` 将 `snapTap` 保存到活动档，再排 NORMAL_SKIPPABLE runner。支持新版板载 Snap Tap 的产品读取 slot 1 当前配置，比较 enabled 和有效键组，必要时设置 slot 1，并对活动档对应槽写入；旧能力分支使用 Keyboard Kill Switch getter/setter。runner 在异常时报告失败，末尾检查 AbortSignal；未另外执行 setter 后 getter。这里不能把旧 kill-switch 分支与新版多组 payload 混成相同协议。

## Single Key Snap Tap

单键编辑保留 `{keyMapMake,keyMapBreak,id}`，同样最多 4 组。实际物理键与 break 输出键不是同一种协议字段：下游将 make 转 analogKeyID，将 break 转十进制 HID keyCode。

756 `oa` 查询当前映射是否分配 DKS、双段触发或 analog gamepad/controller；冲突显示原有提示并进入对应编辑键，不允许靠改显示绕过。挂载会清理不完整组及特定 DKS 冲突；profile 改变重置录制状态。输入的重复 make 检查跨组，break 使用其自己的禁止检查。

录制入口禁用 mapping、注册 native no-browser handler、保存确切 inputredirect 回调并启用重定向。恢复解除该回调、启用 mapping、停止重定向。blur、点击外部和 beforeunload 按各自逻辑提交完整组；beforeunload 通过 ref 读取最新值。此 owner 的 effect 明确取消窗口/文档监听；与双键录制共用的资源不能因为名称相近就由另一组件代管。

MW 有两种写法：set runner 使用 class/slot 1，再等待其余去重槽的 Promise；toggle runner 使用 class 0，随后 `forEach` 发其他槽命令且未 await。空组走真实 getter/禁用链，检查应答与读到的有效键，不发送虚构空数据。两种 runner 都检查取消，但 toggle 的任务完成并不等待这些未 await 的其他槽结果。

## 双键优先级与中断键

Dual Keypress Priority 读取独立 reducer 的 `isEnabled/keys`，使用共享双键输入组件；声明 `dualKeypressAlphanumericOnly` 的产品加字母数字 validator，开关和修改键数组分别 dispatch。MW `31982` 读取活动档，分别合并有提供的字段，保存并发布 UI；将键去重后转 analogKeyID，先 slot 1 再同 GUID 槽调用设备 setter。源码对未知查键使用 `0`，且该方法 await 命令后未检查返回 status、没有 getter；不能把 generic task completed 当成逐槽严格核验。

中断键由宏/DKS 设置及相关映射消费者使用。`56839` 只给宏或 DKS 映射添加/移除 `interrupts`；`KEY_ANY` 或 `interruptionKey:"any"` 转空具体列表，其协议 mode 为 0，指定键为 1。活动档保存后需要时重新排 `ON_SET_KEYMAPPING` 以重建 AppEngine；按产品分别支持 interruptibleMacro、interruptibleDynamicKeystroke，写 slot 1 和活动 GUID 槽。restore 有显式 current-context 检查，普通路径与 restore 的并行/串行方式不同。

## 580 与 747 的 keypad 分支

两款实际 nav 都是 Customize → ACTUATION → Lighting → Help；产品规格分别取自各自的 `analogSpecs`，不能套用其他型号。580 `main.1648cf28.js` 的图/触发/Rapid/Snap owner 分别为 `[7478586,7485975)`、`[7397981,7406020)`、`[7407028,7416469)`、`[7419775,7425426)`；747 `7727.e7997edf.chunk.js` 对应 `[251495,258546)`、`[195696,203322)`、`[204260,213038)`、`[217798,223553)`，另有单键录制 `[230062,236353)`。

580/747 图组件在 KEYPAD 分支渲染专用键图，默认最低尺寸 350×450，并以当前 inputID/AnalogInput、焦点和 mapping 判断可选键；选择后才激活触发/Rapid 控件。触发改变经过各自 converter，再依次 setActuationToBtnList/setActuation；sync all 过滤禁止映射、sync selected 只处理选中键。Reset 只在用户明确点击原版 reset popup 后运行，Adjustment Mode 拦截编辑。580 使用 offset1.5，747 不使用该偏移。

580 的 Snap Tap 是旧版单组 key1/key2 编辑：keyup/inputredirect 录制，blur 恢复打开时组，点外部按当前阶段提交；成功提示为 5 秒。录制停止重新 enableMapping、注销 no-browser handler、停输入重定向；挂载订阅 pressedKeys 以显示 active/snapped，切离 ACTUATION 清 pressedKeys。747 是新版最多四组、LAST_INPUT 默认和 3 秒反馈，另挂 Single Key Snap Tap、Continuous 和 Dual Keypress；single 的 DKS/双段/控制器冲突、beforeunload 最新 ref、临时关闭/恢复 single 及 duplicate 过滤与本页前述新版消费者一致。方向模式为 DIRECTIONAL 时，两种录制均按 keypad exclusion 拒绝保留的方向输入。

747 Rapid slider 明确 0.1…1.0、step0.1，upstroke 可独立；580 取本产品 Rapid 最大灵敏度常量，并保留预设 GUID 分支，不能统一写成 747 数值。两者 set/sync 都生成 mapping 保存动作；有些通用类虽含 toggleContinuous 方法，580 当前顶层没有额外挂该控件。

两款 MW 的当前原件已核对实际 Snap Tap/Continuous/板载消费者：[580 MW 收据](../../../.analysis/inputs/frontend/keyboard-mw-580-current/acquisition.json)、[747 MW 收据](../../../.analysis/inputs/frontend/keyboard-mw-747-current/acquisition.json)。Continuous runner 分别为 `ze [1155709,1156739)`、`Ye [1815994,1817024)`，先 slot1 再同活动 GUID 槽，末尾检查取消；未统一核验 setter status。Snap Tap 新能力分支先 get/比较再 setSnapTapCtrl；旧分支使用 Keyboard Kill Switch。747 板载 addProfile `[1841836,1847626)` 额外同步 thumbstickOutput/thumbstickOrientation，不能以普通键盘的板载参数替代。Single set/toggle 的活动槽与其他槽等待差异仍须保留；这些实际调用不因共享代码在其他产品中出现就自动成为别款能力。

## 已核对的线协议

756 的当前 `rzDevice25` 使用以下命令。相同方法名在其他产品库出现，不足以证明该产品挂载或调用。

| 动作 | `[长度,class,id]` | payload |
| --- | --- | --- |
| 触发点 set/get | `[80,2,25]` / `[80,2,153]` | profile、mode；每键 keyId + thresholdH/L 各双字节；每批最多 15 键 |
| Rapid set/get | `[80,2,26]` / `[80,2,154]` | profile、mode；每键 keyId、enabled、up/down sensitivity 各双字节；每批最多 13 键 |
| Continuous set/get | `[2,2,32]` / `[2,2,160]` | profile、enabled |
| 双键 Snap Tap set/get | `[15,2,39]` / `[15,2,167]` | profile、组数、enabled，每组三字节 keyId1/keyId2/mode |
| Single Key Snap Tap set/get | `[11,2,44]` / `[11,2,172]` | class/profile、键数、enabled；每条 keyId/keyCode；最多 4 条 |
| Dual Keypress Priority set | `[7,2,49]` | profile、键数、enabled、最多 4 个键 ID，未使用位置填零 |

setter 验证、传输成功、附加 parser 结果及上层 runner 是否消费返回 status 是不同边界。本文记录的是静态源码链；没有执行设备写入、校准或运行应用程序。

## 当前 Rust 页面与实际挂载树的核对

本节核对当前源码文件中的页面、动作消费者和释放链，不把已有协议方法或预览界面计作整页完成。部署身份取自上述获取收据；本轮重新计算所用 JS/CSS 和媒体原件的字节数与 SHA-256，均与收据匹配。679 的入口所有者为 `JU [8109259,8118712)`，nav 数组为 `[8114950,8115348)`；756 为 `nr [330271,343239)`，非 BLE 与 BLE nav 数组分别为 `[338420,339056)`、`[339057,339575)`，都实际挂载 Actuation。

已核对的具体反例是键盘 PID 752：其 [UI 原件收据](../../../.analysis/inputs/frontend/keyboard-752-current/acquisition.json) 对应 `3717.9326e687.chunk.js` 的完整 root `oa [179097,186758)`，nav `[184295,184519)` 仅挂 Customize、Lighting、Help。main 中即使保留 `ACTUATION` 常量，也不能给该型号添加本页或磁轴校准。此结论仅适用于已经核对的 PID 752，其他型号需要各自入口证据。

Rust 页面入口在 [keyboard/mod.rs](../../../crates/razer-pages/src/features/keyboard/mod.rs) 的 `ACTUATION → actuation_page`；实际控件和草稿状态由 [keyboard_actuation.rs](../../../crates/razer-pages/src/features/keyboard/actuation/keyboard_actuation.rs) 持有。逐项结果如下。

| 功能/分支 | 官方当前行为 | 当前 Rust 对应与差异 |
| --- | --- | --- |
| 679 选键和代表值 | `OU/Wh/Jh` 维护逐键选中、最近加入键及批量选择频次；选择本身不写设置 | `select_actuation_key/refresh_actuation_selection` 有独立选中集合、选择历史和频次分组，保留 actuation 与 Rapid 各自代表值。这是可确认的状态实现对应；数据来自当前草稿，不是硬件实测证明 |
| 679 模拟按压反馈 | 传感器 level、按键身份与 mapping 输出 flag 分别参与颜色和深度；不能用 slider 草稿代替 | [keyboard_analog_observation.rs](../../../crates/razer-pages/src/features/keyboard/actuation/keyboard_analog_observation.rs) 独立持有观察反馈，679 slider 使用观察深度和颜色；[keyboard_observation_tasks.rs](../../../crates/razer-pages/src/features/keyboard/actuation/keyboard_observation_tasks.rs) 保留订阅身份、取消与等待释放。该调用链存在，真实注册/事件/释放仍须实际对照 |
| 触发/Rapid 修改时序 | `Wh/Jh` 与 756 `Zi/tn` 对触发 slider、Rapid 开关/slider 使用 500ms debounce；各编辑/同步/重置入口检查 Adjustment Mode 并显示原有提示 | Rust 在 retained slider 的 `SliderEvent::Change` 中立即调用 `apply_actuation/apply_rapid`，Rapid/Continuous 开关也直接提交；这些方法未接 Adjustment Mode 的拦截。Snap 自己的 500ms timer 和提示不覆盖其他控件 |
| 756 触发释放阈值 | `Zi.onMakeActuationPointChange` 在未启用 custom release 时令 release 等于新 make，启用时取旧 release 与新 make 的较小值 | `apply_actuation_points` 的普通编辑统一生成 `{0:raw,1:default_release}`，只在 synchronize 时令两者相等；756 普通编辑的输入映射已经在此产生差异，不能用其他型号的 release 行为替代 |
| 触发重置 | `Wh/Zi` 都有图标、hover 提示、点击外部关闭和确认；确认才计算保留的映射并提交 | Rust 仅在 `analog_gamepad_layout()` 或 PID 756 绘制图标；`reset_analog_actuation` 又要求 `analog_gamepad_layout()`，因此 756 点击确认直接返回。其他采用 `Zi` 布局的分支还须补齐实际挂载的 reset 控件 |
| Snap Tap / Single Key / Dual Priority | 产品树见上表；756 `ha` 左列 `Yn/ra/Fi`、右列 `sn/Nn/ua`，明确包含两种 Snap Tap 与 Dual Priority | `actuation_page` 只调用 [analog_snap_panel](../../../crates/razer-pages/src/features/keyboard/actuation/keyboard_snap_analog.rs)，该 panel 限于 Analog Gamepad 布局；[snap_tap 初始化](../../../crates/razer-pages/src/features/keyboard/actuation/keyboard_snap_tap.rs) 也只允许已实现的普通型号或该布局。其余已挂载 Actuation 型号的 Snap 分支，以及本页 Single Key / Dual Priority 控件尚未形成对应挂载和录制链；底层有 setter 不能补足这些入口 |
| 独立 Continuous | 679 `uU` 与 756 `yn [237558,238069)` 从独立 reducer 恢复开关值，并在 Adjustment Mode 拦截编辑 | Rust `continuous_supported()` 漏 PID 756；`continuous_enabled` 仅初始化为 false 并由本地 toggle 修改，没有接入来源状态恢复。显示 false 不能解释为真实设备关闭 |
| 756 动态行程上限 | `Zi` 对选中键取有效 keyTravel 最小值，另保留 visualMax 并显示缩短行程说明 | Rust `maximum_actuation()` 有对应的取最小值逻辑，但依赖 `calibration_v2.switches`；轴体状态的生产消费者尚未接通，见 [校准接线边界](calibration.md#756rust-命令与状态消费者的缺口)。普通 `Slider` 也未对应原版独立 visualMax 与缩短行程说明 |
| Factory / Normal / BLE | 679 Factory 锁定；756 提示与内容灰态采用不同的 normal/factory/ble 表达式 | Rust renderer 已分别计算 overlay 和 wrapper_disabled，不能据此认定状态来源完整。756 normal_mode 初始为 true，所提供连接/模式观察接口尚无 Shell 实际调用，仍需接真实来源 |

`sync_actuation/start_sync_feedback` 已分别实现全选/选中集合、两秒反馈与两秒半禁用时刻，并在页面离开时取消 timer、递增生命周期。它们只证明 UI 时序有实现；反馈动画不消费设备成功结果。`reset_analog_actuation` 保留映射分支的实现也只对满足其实际 guard 的型号生效。

## 页面动作到写入、观察和清理

[keyboard_actuation_submission.rs](../../../crates/razer-pages/src/features/keyboard/actuation/keyboard_actuation_submission.rs) 为命令捕获当时的 profile/payload 和 generation，pending 请求逐个完成，较早失败不会被后续成功抹除。Shell 的 [事件分发](../../../crates/razer-shell/src/shell.rs) 先选择 756 retained restore intent、旧校准 intent，再进入 [keyboard_actuation_write.rs](../../../crates/razer-shell/src/shell/keyboard/keyboard_actuation_write.rs)；后者解析本地动作、保存对应配置，再把已准备 intent 交给设备/配置档身份限定的 FIFO。

[keyboard_actuation_worker.rs](../../../crates/razer-shell/src/shell/keyboard/keyboard_actuation_worker.rs) 经独立服务/实际路由提交；[keyboard_submission.rs](../../../crates/razer-device/src/keyboard/keyboard_submission.rs) 保留已观测板载槽、命令尝试和 setter 应答，不能由磁盘保存推断 HID 已确认。`MappingDraftOnly/AppEngineOnly` 明确没有 Feature setter；普通 mapping 修改、全局 actuation/Rapid 与 Continuous 具有不同任务。756 的普通 mapping/部分控件先经过页面 `apply_latest_source_reduce`，再由 [retained restore owner](../../../crates/razer-shell/src/shell/keyboard/keyboard_latest_restore.rs) 及其 worker 处理；不能因这个页面 reducer 只改草稿而直接断言整个写入链都只保存文件。

FIFO 的连接/配置档失效停止后续 setter，退出正常排空已接纳命令，核验服务 shutdown 并等待 worker/transport 销毁；观察订阅则由其各自 owner 解除。以上是实际存在的 Rust 清理结构，不代表所有上表缺失控件已使用它，也不代表普通测试执行了当前官方 DLL。错误保留在请求/诊断状态，前端两秒 spinner 与真实写入结果仍须分别验收。

## 视觉、资源与测试边界

679 `main.1c5a651a.css` 的完整规则规定：`.body-widgets [68750,68850)` 最大宽 1240px；`.widget-col [69220,69312)` 宽 600px；widget `[69360,69589)` 内边距 30px 40px；动画规则 `[449092,449311)` 保留 140px 盒子与 1.5 倍绘制；reset popup `[451669,452302)` 为最小宽 300px、2px `#ff4500` 边框和独立 hover/显示状态。Rust 已显式保存对应动画盒/绘制尺寸与 popup 部分尺寸，但仍须按完整根树和其他型号 CSS 检查控件间距，不能由几个尺寸吻合宣称页面 1:1。

官方 `Actuation.28db05a7.gif`、`MagneticActuation.56318359.gif`、`LowProfileActuation.e86e6ea4.gif` 和 Rapid tooltip GIF 的当前原件已核对；其项目 alias 由 [资源映射](../../../tools/resource-mappings.json) 统一维护。普通、磁轴、low-profile 动画的选择在 Rust 有明确分支。本轮未启动窗口、加载动画或执行设备操作，渲染与媒体播放验收仍未验证。

自动化用例必须逐产品覆盖选择排除、混合值、最近键/频次、普通编辑与 Sync 的阈值差异、500ms 时序、Adjustment/Factory/Normal/BLE 分支、reset、录制冲突、双键/单键保存、每个实际槽的传输失败、取消、旧连接结果和退出释放。已有 [sync feedback 纯测试](../../../crates/razer-pages/src/features/keyboard/actuation/keyboard_sync_feedback.rs)、[协议提交测试](../../../crates/razer-device/src/keyboard/keyboard_submission_tests.rs) 与 [FIFO/清理测试](../../../crates/razer-shell/src/shell/keyboard/keyboard_actuation_worker_tests.rs) 提供局部测试入口；本轮仅检查其源码，没有运行这些测试，也没有取得官方 DLL 实际功能对照收据。

验收依 [verification.md](../../../.agents/rules/verification.md) 分开记录源码获取、语义逆向、Rust 实现、UI/后台接线、纯/模拟测试、实际 DLL/设备对照与运行时验收。模拟报文或两边均未连接不能把本页提升为完成；无法完整恢复原值的写入和校准数据变更不得开始实际对照。
