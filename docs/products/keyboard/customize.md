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

全部 73 个键盘当前顶层 Customize 已逐个核对，普通 keyboard 都显式传 `showMouseUse:false,hasKeyboardControls:true`。以下分支来自真实调用方；它们与共享编辑器内部的能力过滤共同决定 UI：

| 产品 | 顶层参数和生命周期差异 |
| --- | --- |
| 555 | 另传 `isKeypad:true` |
| 580 | `isKeypad:true`，activeButton 的 keyPadGroup.type 为 Keypad 时更新本地 isTwoTapMapping，再给编辑器 two-tap 参数 |
| 747 | 同样按当前 keypad 输入切换 two-tap，并传 disableActuationPoint；不能把所有选择都当 AnalogInput |
| 614 | two-tap、disableActuationPoint；本顶层不另按 Factory 选择静态页面 |
| 631 | 额外传本产品 keyProperty |
| 642、688 | 按 Factory 选择静态/可编辑页面；可编辑传 two-tap |
| 678、679、719、720、721、740、741、742、746 | 同样的 Factory 分支，另传 disableActuationPoint |
| 728 | 顶层不走上述 Factory 分支，传 two-tap/disableActuationPoint；内部仍有 Windows Mode 相关条件 |
| 756 | Normal Mode 提示；Factory 静态/可编辑分支均按 BLE 增加 disabled，另传 two-tap/disableActuationPoint |
| 550、691 | 真正 lazy owner 模块78697 后到共享17852，仍显式传普通 keyboard 参数；AsyncComponent 只拥有加载/错误，不拥有映射草稿 |
| 708、739、752 | 分块页面挂普通 keyboard owner，行为不能以 chunk 名推断为另一套映射身份 |

550 `8697.e3105e91.chunk.js [673,928)` 与 691 `Customize.d6e5b9b1.chunk.js [685,940)` 的包装继续渲染 `17852.A`。Factory、keypad 和 BLE 的选择须留在实际产品 owner；不能在共享组件里统一硬编码为“所有键盘都支持同一编辑器”。

555、580、747 的 ProductImage 由实际 Customize 挂载持有。普通产品页和每个 Armory 内嵌根分别保留图片探测、已发布的两档密度、100 ms 重新挂载任务及错误；Armory 根以稳定实体身份显式打开、关闭和释放，主窗口切到 Lighting 不会关闭仍存在的 Armory 根。图片探测在 prepaint 借用实际绘制视图的共享资源缓存，异步完成通知该视图；render 只组合已发布状态。属性变化保留旧 srcSet，直至当前两个探测均完成；离页取消任务、释放像素，并拒绝旧元素重新挂载。

747 的三个动作须分别追踪。`ON_SET_REGISTER_UI_ANIMATION_EVENTS` 的 thunk 只向 UI store dispatch；当前完整 store 的 thunk middleware 和全部 root reducers 没有后端转发消费者，因此 Rust 不发送虚构设备请求。`ON_SET_ANALOG_JOYSTICK_MODE` 修改 UI 状态并控制 MW41243 的 `isSendDataToUI` 门闩，不写设备，不清除最后一份 gamepadTester。真正的 `ON_SET_DIRECTION_MODE` 则进入 `keypadJoystickAnalogConfig` 设备队列，当前调用方、两个 setter 与保存完成边界见下述接续；真实设备确认及恢复仍独立验收。

747 原始 controller report 是恰好 15 字节、首字节为 0 的数据。module47561 按五组无符号小端数减 32768 解码，按 `Math.round(axis / 32767 * 1000)` 归一化，MW41243 经 15 ms leading/trailing throttle 发送 `MW_UPDATE_GAMEPAD_TESTER_VALUES`；当前 UI reducer 直接保留六字段 payload。页面直接使用千分单位坐标，不能复用 679 的计算后 gamepad output 观察器或另一个 controller action 的乘 1000 消费者。静态消费链、Rust 模拟传输和真实设备／原版 DLL 对照分别验收。

747 的[既有 OBM 与 Fe 依赖审阅](../../../.analysis/results/reverse-pending/review-sources/696c378d2a44c84dfdeb8da295e0658836b8a0d1fec0845cf54c93eec7d7f4c1.json)保留旧部署的 191 个完整 owner，覆盖 24 个 OBM、105 个 Fe 传递依赖工厂和传输消费者。当前接续位于 [host-current/local-audit.json](../../../.analysis/results/host-current/local-audit.json) 的 `keypad_mapping_consumers`，复核入口为 `python -X utf8 tools/workspace.py audit-host-local --query keypad-mapping-consumers`。本批按[当前 UI 收据](../../../.analysis/inputs/frontend/keyboard-747-current-refresh-ec3db4b29c5a/acquisition.json)、[当前 MW 收据](../../../.analysis/inputs/frontend/keyboard-mw-747-current-refresh-ec3db4b29c5a-2/acquisition.json)选择原件，并以[双轮部署复核](../../../.analysis/results/frontend-revalidation/9df084ddd0e0c34e1e81a46e0a62ee4e9db46e0df66daf1021f41ad6f4c4f318.json)核对实际使用的 5 个 UI、8 个 MW 文件。18 个 Fe helper 与 21 个 OBM／协议／生命周期方法中，36 个完整正文逐字节相同，3 个仅有已核实的 `h/y` 局部标识符重排；这不把旧版全部工厂或产品提升为当前完成。

当前真实挂载为 UI `main → 6633:97822 → 3496`；产品明确传入 `isKeypad:true`，再挂 `KeypadButtonPanel:30910` 和方向选择器。普通映射编辑器 `43807` 在 `3807`，`2667` 只对应 CSS；AnalogInput 的 two-tap `67742` 仍是独立分支。MW `1610` 的 `B=i(32226)` 经 `32226.A` 实际转出 `4325.RzAppEngineMapping_default`，后者调用 `Fe`；不能误选同一 MW 中另一份同名内联编译器。`du` 按真实四／八向输入、当前模式及精确层身份补齐缺失默认项，再交给 Fe 并计算 AppEngine hash。

方向任务先保存当前活动档的 `directionMode`，不等待 `Nl` 排入的设备任务，就刷新默认映射并重新装载 active keymap。`Rl` 顺序 await `[1,2,56]` 的 report mode 和 `[2,2,57]` 的 mapping mode：四／八向传 report mode 1、对应 mapping mode 1／2，其他模式传 0；两个 setter 的返回 status 都未检查，AbortSignal 在两次调用之后才检查，也没有回滚。重新装载前的 `[2,2,54]` active-keymap 写入会逐槽检查 SUCCESS、失败返回 false，但 `loadKeyMappings` 丢弃该布尔值后仍发送映射事件。异常、队列 completed 和设备确认必须分别记录。

本机映射任务区分临时 keypad context 与持久化数据，按实际允许动作生成、保存并发布 UI；存储的 PostSetLocalStorage 另行异步发布 AppEngine 数据。OBM 经 `handleOBMUpdates → Fu → 34737.H6 → 55665.UQ`，按显式 keymapId、当前／首个 keymap 的 slot、最后 slot 1 的顺序确定目标，临时切换强制 1；逐槽异常被记录后继续，外层 completed 不保证每条写入成功。`setSingleKeyAssignment` 的常量仍为 `[80,2,13]`，实际传 `packetSizeSet=10`，报头只传 profile、key、mode、fnId、fnDataSize 和五个数据字节；`thresholdL/H` 只进入请求缓存。先 await 的 `[1,2,55]` target SET 布尔值仍未检查；单键 SUCCESS 才更新请求缓存，没有附加 GET、目标恢复或整链事务锁，echo 也不是独立读回。

当前普通编辑器的 Save 由父组件 await `saveRef(true)`，但子回调同步调用 `saveChanges()` 后就清 dirty，不返回 MW／OBM promise；reducer 立即存入并发送 mapping list。Don't Save 和关闭提示传 false，各自执行原版续行，不调用保存器，也不恢复设备。7 项定向纯维护工具测试覆盖当前来源变化、实际 compiler 接线、严格层匹配、10 字节报文、保存与队列完成误判、槽选择、失败和双轮来源门禁；未执行厂商 JS、DLL、设备或应用窗口。

Fe 按 `isHyperShift===false/true` 精确筛选，缺字段不等于普通层；Hypershift 八向生成要求对应 cardinal 和两条 diagonal 都存在。BUTTONS 模式默认读取 `outputFlag=2`，生成键盘 flag 2/3，顺序 up、left、right、down；DIRECTIONAL 模式明确选择原始 `flag=0`，生成 flag 0/1，cardinal 顺序 up、right、left、down，普通层 diagonal 默认 Home、PageUp、End、PageDown。普通 joystick axes/raxes 的松开输出强制 `[0,0,0]`，不能合并这两套 flag 或以数组下标替代输入身份。

当前共享原生接续位于 [host-current/local-audit.json](../../../.analysis/results/host-current/local-audit.json) 的 `keypad_mapping_current_native`，复核入口为 `python -X utf8 tools/workspace.py audit-host-local --query keypad-mapping-current-native`。本批直接复用 Mapping 1.3.17.2、SHA `c64e5394…` 的完整捕获，核对 75 个完整函数、10,054 条指令、24 个实际 PE 槽和 42 条直接调用边，不新建捕获。旧 `keypad_mapping` 的 78 个函数、9,958 条指令只证明 AppEngine 4.0.827、SHA `6eabdf…` 的原件，不能补齐缺失的当前消费者。

原生 analogStick 输入由同一收据的 `mapping_ble_analog_stick_routing` 持有，包括输入身份、阈值、真实样本边沿和 delegate 接线；本批不重复解析报告。它与上述 15 字节 UI report 分开，原生对象的阈值初值不能作为设备实测或首次连接写入依据。当前输入委托 `0x66d10` 的 make 按 gated gamemode、gated otfs、dkp、snaptap、global shortcut、ordinary 顺序查找，命中即止；关闭映射时只查询独立的 gated gamemode／ordinary bucket。break 直接使用 active mapping 与无状态输入身份快照，不重跑 make 优先级。

`0xf24c3 → 0xff70c → 0x85f0a` 保留 dpad 数组顺序，按当前 active input 快照选择首个符合项，clone／分配失败也不继续选后项。device+1288 拥有两个无状态身份字符串组成的 pending pair 列表：make 追加后达到两项，先由 `0x88654 → 0x79262 → 0x878ae` 执行最老项，再经 `0x87842` 弹出并释放，然后处理新输出。break 弹出最老 pair，匹配松开身份；仅重选仍 active 的另一非空身份，并将其与空身份重新排队。`0x878ae` 按原 dpad index 取输出，不合成通用 key-up、回滚或设备恢复；空列表、身份不符、映射失活、缺少输出和 clone 失败保留实际无输出／日志边界。

`0x87006 → 0x101474/0x1015ac` 只修改已有 joystick state，button 1..24 使用字节计数；轴贡献按无状态输入身份替换，零贡献删除，每轴合并最强正负值，-128/+127 组合另加 1 回到 0。`0xf2a6d` 保留 xyz 低于 -127 截为 -128 的非对称解析。`0xaf5ca` 仅将 changed 字段送驱动，并独立通知计算后的输出；该通知可能早于 I/O 完成，不能当作物理读回。type41 Xbox 在 `0x87424` 保留独立下游边界，不沿用 joystick 的结果语义。

`0xad0cc` 将 type21 keyboard 交给已有 timed executor 或驱动队列，type27 mouse 交给驱动队列；`0x118940` 的实际虚表槽 +96 指向 `0x112ee0`，构造零初始化的 32 字节 `IOCTL 0x88883020` 输入。joystick button 为 kind7、flag/button 在字节 8/9；axes／raxes 为 kind5/6，signed xyz 加 128；POV 为 kind9，按 x/y 符号生成八向 0..7、neutral8。提交、ERROR_IO_PENDING、实际 OVERLAPPED 完成及 `0x110dda → 0x116020` 的 bool 回调分别记录；bool 也不是独立读回。仅 keyboard／mouse 的 false 回调经 `0xa3650` 尝试 OS SendInput，joystick 的空回调不具有该回退。

`0x656ca` 先 join 设备线程，再释放 pending pair、状态和 executor；`0x9acfa` 只销毁 pair 字符串，不执行释放映射。共享 completion、CancelIo／close、pending-I/O-zero 与线程释放分别引用当前 `mapping_physical_input_decode`、`mapping_input_shutdown_chain`。8 项定向维护工具测试覆盖旧原件替代、实际 compiler 接线、缺失 deferred／release owner、PE 字节及重算哈希、关键直接调用、完整语义正文、驱动槽和正式依赖引用。747 AnalogInput／two-tap、temporary-keymap、存储／配置／模式 producer 与 pending output／重连代次接线，以及其他 dpad 输出、timed keyboard、全局快捷键和 Xbox 下游仍是后续独立功能链；Rust 实现、UI／后端接线、实际 DLL／设备对照和运行验收均未由本切片完成。

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

## 678／688 的映射编译与 Gamepad Tester

这两款使用各自当前部署的 `analogV3` 非固件映射链。正式证据统一保留在 [gamepad_tester 查询](../../../.analysis/results/frontend/keyboard-mw-audit.json)的 `products[].non_firmware_mapping_review`，包含实际启动分块、完整模块依赖、映射事务、编译函数、事件回调和 UI 消费者；Rust 尚未接通的消费者与这组已核对的官方语义分别记录。

| PID | 当前 MW 原件与完整 owner 字节范围 |
| --- | --- |
| 678 | [6737.b4bf391c075aa0879a1d.js](../../../.analysis/inputs/frontend/keyboard-mw-678-current/raw/apps.razer.com/synapse/products/678/mw/6737.b4bf391c075aa0879a1d.js)：`42124.tB → Ot [1374188,1381127)`；`42124.yH → rt [1347244,1350084)`；`14486` Events `[1071655,1072234)`、注销 `[1072429,1072729)`、回放 `[1066002,1066398)` |
| 688 | [6737.9be565ca0f83234feb88.js](../../../.analysis/inputs/frontend/keyboard-mw-688-current/raw/apps.razer.com/synapse/products/688/mw/6737.9be565ca0f83234feb88.js)：`42124.tB → Lt [1369720,1374859)`；`42124.yH → ut [1342229,1345091)`；`14486` Events `[1059248,1059831)`、注销 `[1060028,1060330)`、回放 `[1053572,1053972)` |

两款实际入口都等待 `2291`、`6737`、`1364`，随后加载 `16737`；`1364` 由入口分块表指向各自的 `onLoad`。`32226` 实际转出 `69183.RzAppEngineMapping_default`。沿这四个已选分块逐一核对，每款均有 107 个唯一的静态依赖；未选 worker／其他分块中的相同模块 ID 不参与这条调用链。各原件 SHA-256、完整 owner 的边界与正文 SHA-256 随查询保留，没有执行厂商 JavaScript。

`ON_SET_KEYMAPPING` 先按状态机确定本机保存、AppEngine 和 OBM 动作，再进入各自 `tB`。事务从新的 mapping 中查找 `AnalogInput` 的 `controllerGroup.controllerAssignment`，只匹配 `52418.$T` 的十种模拟扳机／摇杆方向。非固件模式且 analogV3 生效时，有匹配项就先注销、再注册 Events；无匹配项只注销。数字按钮和仅有 `joystickGroup` 的映射不满足这个注册条件。注册参数为 `50`；两款实际 `DeviceInfo` 没有开启 8k 分支，不能把参数解释成每 50 个物理报告必然产生一次回调。

两款均明确启用双字节 actuation。编译器分别调用 `2291` 中的普通映射与模拟映射算法，保留普通层／Hypershift、双段输出、键盘／鼠标组合、模拟控制器与 DKS 分支，再形成 `mappings`、`actuations`、`rapidTriggers` 和完整 hash。678 的模拟集合／逐映射 owner 为 `[48969,49912)`／`[51144,53279)`；688 对应 `[49088,50033)`／`[51287,53424)`。Turbo 文档的三次循环仅由 `RzLockableStorageError` 驱动，不能扩展为任意传输重试。事务保存递增版本、发布 runtime 文档与 UI 后，才发本机 completed 并安排 OBM；本机落盘或此 completed 都不是 HID 写回确认。

`14486` 注册前查询实际注册状态，已经注册则返回；只有 native 注册返回 true 才附加 `xboxGamepadEvent`。注销先查询，再执行 native 注销；只有其返回 true 才解除当前回调，false 分支保留警告，不能当作已经释放。事件只消费匹配当前产品 ID 的输出 JSON。单样本立即发布；数组逐项解析、每项间隔 5 ms。新 callback 会替换旧 token，使旧数组在下一检查点停止；空数组不制造零样本。当前注销函数没有清 token 或取消已开始的数组回放，JSON 解析也没有同一 `finally` 清理，必须保留这些实际边界。

678 的实际 UI class `[8119801,8129284)` 和 reducer `[7467339,7473717)` 来自 [main.f78f0dcf.js](../../../.analysis/inputs/frontend/keyboard-678-current/raw/apps.razer.com/synapse/products/678/ui/static/js/main.f78f0dcf.js)；688 对应 `[7747789,7753565)` 和 `[7247462,7253840)`，来自 [main.e09fd702.js](../../../.analysis/inputs/frontend/keyboard-688-current/raw/apps.razer.com/synapse/products/688/ui/static/js/main.e09fd702.js)。两款在非 macro 显示模式挂 Tester，读取独立 `customizeReducer.gamepadTester` 的六个模拟输出值。实际挂载 class 没有 in-view `IntersectionObserver` 或 `toggleXboxRegistration` 调用；共享 action 和 interval5 方法存在，不代表这两款页面启用了现代产品的可见性注册链。678 的 class 卸载只移除 resize，未显式移除已注册的容器 scroll；688 该 class 没有对应 mount/unmount 方法。

这组观察表示映射输出，不能冒充物理控制器读回、设备已确认设置或保存草稿。维护工具的六个纯测试覆盖非固件／其他编译分支、注册与注销失败门禁、旧回放、现代可见性分支拒收、过早 completed、证据冲突和增量发布保留既有 679／747 证据。它们不执行官方 DLL，也不证明这两款独立 Rust 编译器、页面消费者或设备对照已经完成。

## 板载同步、返回值与取消

允许 updateObm、不是特殊 keypad 分支且不是 select-all 时，`Lt` 将同步任务入队。`zt` `[821845,822621)` 找活动 GUID 实际占用的板载槽，并额外纳入槽 1，计算删去映射的默认恢复项、宏增删、板载可保存输出、actuation/Rapid Trigger 批次，以及 Mod Tap 更新。

单映射由 `Ht` `[819437,819857)` 入队，runner `85225.ft` 在开始和每槽写入后检查 AbortSignal。它为 profile-navigation 补协议参数，再逐槽调用 `obmEngine.setSingleButtonMapping`。这里的逐槽异常被记录后继续；当前 runner 最终仍可能写 completed。该行为必须和实际底层返回值区别描述，不能把外层 completed 解释成所有槽逐条确认通过。

`obmEngine.setSingleButtonMapping` 将 output group 编成 buttonId、layer mode、fnId、data、thresholdL/H。忽略表中的键或 fnId 非零却无 payload 的项会跳过；模拟键和普通键分别调用 `setSingleAnalogKeyAssignment` / `setSingleKeyAssignment`。只有返回 `STATUS_SUCCESS` 才更新引擎的 buttonAssignments 影子状态，随后把 `obmJson` 附到返回对象。该函数没有额外 getter。

Rapid Trigger 和触发点分批写分别在该引擎 `[102011,103239)`、`[103240,104472)`，Mod Tap `[104473,105728)`。它们验证 profile/列表和功能选项，以协议表规定的最大批次数分组。当前代码在 await 未抛异常时设置批结果成功并更新影子属性；失败可能以返回状态或异常两种方式到来，需按底层函数实际契约理解，不能额外虚构读回。

## 页面更新与退出清理

`mh` 在活动配置档、Hypershift、标签页、映射、宏、可用设备、Chroma 安装状态改变时重算显示；有 unsaved changes 时不自动 reload。切回 Customize 而 active key 的 DOM anchor 已失效时，以 requestAnimationFrame 和有限次 timeout 重新寻找当前输入 ID；上次寻找 timeout 会被清除。

`mh.componentWillUnmount` 清当前 timeout 和 storage listener。two-tap 组件卸载移除捕获 scroll listener。普通编辑器卸载移除 click、scroll、resize listener 并 disconnect observer；挂载还订阅 saveAll/discardAll，而该组件卸载函数未显式取消这两个匿名订阅。这里记录实际清理范围，不能以“组件消失”推断所有回调自动解除。

这些都是静态源码行为。页面没有等待物理写入再关闭的路径，并不等于设备写入不需要追踪；当前文档保留两种事件层级及原件的失败/取消差异。
