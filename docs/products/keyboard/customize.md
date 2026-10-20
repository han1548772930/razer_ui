# 自定义按键与映射编辑器

Customize 的主任务是选择产品图上的实际输入，编辑普通层或 Hypershift 层的输出，再交给映射中间件、本机文档和板载同步队列。页面保存、主机 AppEngine 编译、板载任务完成是不同阶段。本文先给出已逐函数核对的 679 完整链，其他产品的挂载差异由本目录的产品结构页和下述分支对照承接。

## 原件入口

679 UI 原件根为 `.analysis/inputs/frontend/keyboard-679-current/raw/apps.razer.com/synapse/products/679/ui/static/js/`，MW 原件根为 `.analysis/inputs/frontend/keyboard-mw-679-current/raw/apps.razer.com/synapse/products/679/mw/`。版本与哈希见 [UI 收据](../../../.analysis/inputs/frontend/keyboard-679-current/acquisition.json)、[MW 收据](../../../.analysis/inputs/frontend/keyboard-mw-679-current/acquisition.json)。本页区间均为 UTF-8 字节半开区间。

| 文件 | 模块/函数 | 字节区间 |
| --- | --- | --- |
| UI `main.194d8a7b.js` | 可编辑页面 `mh` | `[8012600,8038805)` |
| UI `main.194d8a7b.js` | Customize 工厂/可编辑选择 `fh` | `[8042270,8042623)` |
| UI `main.194d8a7b.js` | actions 模块 `61350` | `[6066872,6071160)` |
| UI `main.194d8a7b.js` | `SET_MAPPINGLIST` reducer case | 起于 `7363493` |
| UI `2383.d3b2762a.chunk.js` | 普通映射编辑器模块 `12383` | 模块起于 `573`；保存方法见下文 |
| UI `TwoTapKeyMapping.270f8638.chunk.js` | 模块 `67742`，`k` | 类构造与生命周期 `[1472,11248)`；render `[11248,15435)` |
| MW `6737.d9169f212a8242741503.js` | `10745` 命令分发；`42124.Lt` | case 起于 `101939`；`Lt` `[813699,818838)` |
| MW `6737.d9169f212a8242741503.js` | AppEngine 编译 `ut`；OBM 分解 `zt` | `[786208,789070)`；`[821845,822621)` |
| MW `6737.d9169f212a8242741503.js` | 板载单映射 runner `85225.ft` | `[1425807,1426568)` |
| MW `obmEngineKeyboard.fdf63782d77cc1c9a909.js` | `setSingleButtonMapping` | `[87837,89860)` |

工厂身份与完整 UI owner 捕获见 [keyboard-main-audit.json](../../../.analysis/results/frontend/keyboard-main-audit.json) 的 `keyboard_page_layout.products[product_id=679].factory_customize_chain`；本页补充实读上述保存消费者。

## 页面由谁持有

主窗口 `JU` 挂载 Customize 连接器 `Gh`。Factory Default 仍能打开标签页，但 `fh` 只挂产品图和工厂档说明；普通活动档才挂 `Ph/mh`。宏窗口也复用静态/可编辑选择，且设置 `displayMode:"macro"`。身份与启动顺序见 [profiles.md](profiles.md)。

`mh` 持有当前被映射键的 DOM anchor、保存提示状态、下一动作回调、丢弃动作回调、按键映射视图，以及重新寻找 DOM anchor 的 timeout。真正的输出编辑值和保存回调属于懒加载编辑器；父页面通过 `setSaveRef` 取得它，不能把父组件自己的提示状态当成全部映射内容。

挂载时 `mh` 先查询 Chroma 是否完整安装、读取 Windows 登录提示的本机隐藏标记，注册保存提示回调，然后等待系统键盘布局、合并当前 view 的主映射/side-panel 映射、加载按钮显示，再注册本机 `storage` 监听并安排 anchor 同步。Chroma 安装状态为 loading 时暂不显示映射 popup；这不是键盘断连状态。

## 按键选择和层切换

`loadFilteredMappings` `[8020049,8021569)` 根据当前 `isHyperShiftOn` 选择相应映射，等待对应 layout 的按钮数据，并把输出类型翻译成按钮显示。通用组件同时保留 system、controller、mouse attachment 路径；只有当前产品调用方满足条件时才进入这些路径。

`updateActiveButton` `[8030430,8030951)` 阻止以下输入进入编辑：按钮 `isEnabled` 为 false；产品禁止的身份；当前为 Hypershift 而按钮禁止该层重映射（特定 multi keyProperty 例外）。可编辑键先检查自定义 command-dial 映射，再设置 activeButton、打开面板，并选取当前键的实际 active mapping。

页面显示的映射类型不只有键盘输出。当前 owner 包括键盘、Dynamic Keystroke、摇杆、keymap、控制器、鼠标、滚动、灵敏度、宏、跨设备、配置档切换、Chroma、Hypershift、启动程序、旋钮、多媒体、音频、应用专属、Windows 快捷键、文本、背光、禁用和全局亮度的显示函数。显示函数存在与当前产品的功能列表可选是两回事；编辑器还通过产品配置、层、安装模块、已有值与输出条件过滤真正的功能项。

配置档切换输出用目标 GUID 解析名称。目标配置档或关联灯光身份不存在时，显示函数采用其源码规定的默认映射呈现。跨设备输出还区分目标已连接、目标配置档和灯光身份有效性；不能只凭“有一个旧映射对象”显示目标当前已在线。

## 两段触发映射

679 `mh.renderPopup` 对 `isTwoTapKeyMap && activeButton.inputType==="AnalogInput"` 懒加载模块 `67742`，其他可编辑输入使用模块 `12383`。宏模式的 two-tap 视图只渲染对应宏功能部分。

`TwoTapKeyMapping` 用 `inputID`、`AnalogInput` 和精确 Hypershift 层查当前映射。主/次输出存在同一外层 `mapping` 数组，外层持有 inputID、inputType、analogInput 和 isHyperShift；每段输出持有 actuationPoint、输出组，主段还可持有 Rapid Trigger。保存时不会把输入身份字段复制到每个内层输出。

第二段只有确实有非 `defaultGroup` 输出时才认为存在。产品/函数禁用条件可强制隐藏第二段。主段的默认输出若带非默认触发点或 Rapid Trigger，会转换为显式 keyboardGroup，避免“默认映射”标记吞掉这次参数。

触发间隔处理见 `onActuationPointChange` `[5481,7045)`：两段在 slider 单位下至少相差 `10`；普通全行程分支最高 `40`，另一产品类型分支最高 `21`，low-profile 分支最高 `28`。拖动第二段可同步向前调整第一段，拖动第一段可同步向后调整第二段；low-profile 还会根据可用余量禁用第二段。其值必须经过 converter，不把 `10/40` 直接当作毫米。

`getTwoTapMapping` `[3847,5349)` 从主/次编辑器的 getter 取得最新输出。次功能关闭时，第二段恢复成源码的 defaultGroup 对象；主段 Rapid Trigger 会保留，但普通编辑器在保存 joystick/controller 或真正第二输出时删除不兼容的 Rapid Trigger。该兼容决策在保存消费者中完成，不只是显示灰色控件。

## 保存、丢弃和取消各做什么

父页面的保存提示逻辑：

| 操作 | 实际调用 |
| --- | --- |
| 打开提示 | `displaySaveAlert(nextAction,anchor,dontSaveAction)` 保存三项回调并显示提示 |
| Save | `saveChanges` `[8029511,8029743)` 恢复 anchor、关闭提示、调用子编辑器保存 getter，然后执行 nextAction |
| Don't Save | `[8029744,8029850)` 调用子回调 `false`，再执行明确的 dontSaveAction |
| 关闭提示 | `dismissSave` `[8029212,8029340)` 关闭提示、以 `false` 调用子回调，并退出 configure-sensitivity 状态 |

子编辑器的 `saveMapping` `[35722,35862)` 在 true 时执行 `saveChanges()`，然后重置本地 dirty/canSave；它本身没有等待异步设备回执。`clear` 关闭选中键和映射面板。`saveChanges` `[41089,43925)` 先检查 joystick 方向选择，再调用当前输出表单的 getter；two-tap 进入 `saveTwoTapChanges` `[36650,40739)`，全局快捷键进入自己的身份链，自定义 command-dial 输入进入 `updateCustomCommandDialMapping`，普通键则更新当前 view 的 mapping list。

two-tap 保存按输入身份和层定位原项，处理新增/替换/删除默认项，同时使同一实际键两个层的第一触发点保持同步。需要保存非默认触发值而另一层尚无对象时，会创建另一层的显式基础映射。宏绑定与显示模式、原输出类型分别判断；不能把“保存一个按键”固定解释成只有一条 mapping 变化。

编辑器的 `enableSave` 同时判断输出是否有效、是否真的改动以及 two-tap joystick 选择是否就绪；保存按钮还被过小 actuation gap 阻止。Hypershift 重映射和灵敏度 clutch 有各自原版确认/警告链，不由通用 dirty 状态统一替代。

## UI 保存如何到中间件

`61350.setMappingList` 的 `p` `[6069329,6069392)` dispatch `SET_MAPPINGLIST`，必要时更新 mappings-updated 标记。reducer 立即更新 UI mappingList，并发送：

```text
ON_SET_KEYMAPPING
payload = { mappingList: [...] }
```

因此这是官方的乐观 UI 行为；编辑器关闭和 dirty 清除早于板载队列完成。`setMappingListToUI` 的 `P` `[6069456,6069497)` 只发布 UI 读入 action，方向与用户保存不同。

MW 状态机的 `ON_SET_KEYMAPPING` 按 obmReady 决定动作集合：都可有 updateLocalStorage/generateAppEngine，只有条件满足才带 updateObm。命令分发把新映射、select-all、reset-keybinds、version 和允许动作交给 `42124.Lt`。之后根据活动档的 `isEnableKeyboardKeys` 再排键盘启用状态事件，并清 spinner；这也是同一调用链的下游动作。

## MW 本机事务和 AppEngine

`Lt` 依次执行当前布局过滤、输入/控制器注册调整、release 阈值标准化、读取活动配置档。没有活动对象时直接返回。模拟键上的 OTFM 宏输入会转换成该产品需要的 AnalogInput 双段表示，并采用原映射或预设默认触发点。

允许生成 AppEngine 时，先处理 Mod Tap 的特定右侧修饰键，加入可中断宏/DKS 的 interruption 条件，并移除与 DKS 冲突的 single-key Snap Tap 成对键；再调用 `ut` 生成 runtime 映射。保存位置按产品是 active keymap 或普通 mappings 分支区分。reset-keybinds 命中预设身份时重新用该预设 factory 生成映射。

`ut` `[786208,789070)` 读取 Synapse/default turbo 文档，按当前布局过滤输入，分别生成普通、AnalogInput、方向键/dpad 映射以及 actuation/rapidTrigger 集合，计算完整 appEngine hash。turbo 存储发生 lockable-storage 冲突时最多循环三次；这不是对任意设备命令统一重试三次。

本机文档递增 version，通过 `Pf(document,false)` 保存，再投影 runtime window storage。随后发布清理过内部字段的宏映射和真实保存后的 UI mappings。正常发出 taskMaker completed；异常发 error，并在 finally 清 spinner。该完成发生在下面板载队列入队之前，不能替代板载回执。

## 板载同步、返回值与取消

允许 updateObm、不是特殊 keypad 分支且不是 select-all 时，`Lt` 将同步任务入队。`zt` `[821845,822621)` 找活动 GUID 实际占用的板载槽，并额外纳入槽 1，计算删去映射的默认恢复项、宏增删、板载可保存输出、actuation/Rapid Trigger 批次，以及 Mod Tap 更新。

单映射由 `Ht` `[819437,819857)` 入队，runner `85225.ft` 在开始和每槽写入后检查 AbortSignal。它为 profile-navigation 补协议参数，再逐槽调用 `obmEngine.setSingleButtonMapping`。这里的逐槽异常被记录后继续；当前 runner 最终仍可能写 completed。该行为必须和实际底层返回值区别描述，不能把外层 completed 解释成所有槽逐条确认通过。

`obmEngine.setSingleButtonMapping` 将 output group 编成 buttonId、layer mode、fnId、data、thresholdL/H。忽略表中的键或 fnId 非零却无 payload 的项会跳过；模拟键和普通键分别调用 `setSingleAnalogKeyAssignment` / `setSingleKeyAssignment`。只有返回 `STATUS_SUCCESS` 才更新引擎的 buttonAssignments 影子状态，随后把 `obmJson` 附到返回对象。该函数没有额外 getter。

Rapid Trigger 和触发点分批写分别在该引擎 `[102011,103239)`、`[103240,104472)`，Mod Tap `[104473,105728)`。它们验证 profile/列表和功能选项，以协议表规定的最大批次数分组。当前代码在 await 未抛异常时设置批结果成功并更新影子属性；失败可能以返回状态或异常两种方式到来，需按底层函数实际契约理解，不能额外虚构读回。

## 页面更新与退出清理

`mh` 在活动配置档、Hypershift、标签页、映射、宏、可用设备、Chroma 安装状态改变时重算显示；有 unsaved changes 时不自动 reload。切回 Customize 而 active key 的 DOM anchor 已失效时，以 requestAnimationFrame 和有限次 timeout 重新寻找当前输入 ID；上次寻找 timeout 会被清除。

`mh.componentWillUnmount` 清当前 timeout 和 storage listener。two-tap 组件卸载移除捕获 scroll listener。普通编辑器卸载移除 click、scroll、resize listener 并 disconnect observer；挂载还订阅 saveAll/discardAll，而该组件卸载函数未显式取消这两个匿名订阅。这里记录实际清理范围，不能以“组件消失”推断所有回调自动解除。

这些都是静态源码行为。页面没有等待物理写入再关闭的路径，并不等于设备写入不需要追踪；当前文档保留两种事件层级及原件的失败/取消差异。
