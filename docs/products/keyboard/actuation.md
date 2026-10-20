# 触发距离、Rapid Trigger 与模拟键协作

本页从当前实际挂载的 Actuation 组件解释模拟键选择、触发点、Rapid Trigger、Snap Tap 和双键优先级。映射编辑器中的双段触发见 [customize.md](customize.md)，磁轴校准及 756 的轴体行程联动见 [calibration.md](calibration.md)。UI 选中值、配置档内容、板载命令应答和硬件事件分别说明。

## 产品分支与实际组件树

当前实际挂载 Actuation 的 PID 为 614、642、678、679、688、719、720、721、728、740、741、742、746、756。每款的原件、获取收据、入口和实际组件字节位置见 [逐 PID 产品页](products/)。以下分组来自这些原件的完整顶层函数和连接器，未根据名称或通用库符号推断能力。

| 产品 | 实际页面 |
| --- | --- |
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
