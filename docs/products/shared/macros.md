# 宏：录制、编辑、设备触发与停止

宏页面编辑的是带 GUID 的动作序列。按下录制按钮、配置录制快捷键、编辑宏中的按键，以及从设备发起板载即时录制（OTFM）是不同入口；它们共用部分录制服务，同时有不同的状态机、锁和保存消费者。

## 页面中的对象

宏列表、文件夹、当前 GUID、已保存宏、当前编辑动作和录制配置分别存在。动作编辑支持按键、鼠标、延时、文本和其他宏引用等实际动作类型；reducer 根据动作 `Type` 进入对应字段更新器。动作插入、复制和修改会保存撤销历史、清空前进历史并把当前宏标记为未保存。分阶段宏还按当前 phase 或放置位置赋予动作阶段。

`recordProfile` 中的录制快捷键和开始延时不等于某一个宏的动作。按钮入口在有当前宏时处理倒计时；再次操作会取消尚未结束的倒计时。开始后，按钮切换为停止，停止原因由 `stopBy` 传给后处理，而不是只保存一个笼统的结束标记。

## 软件录制的启动顺序

`hr` 先建立状态机、迁移／读取本机宏。发现已有原生录制在运行时，先调用停止并注销录制事件，再初始化本页的录制状态、共享窗口状态和事件消费者。它还注册锁屏与用户切换处理。

完整 `RECORDING` 状态机先注册录制事件，再查询 `hasMacroRecordingStarted`。未运行进入可开始／等待快捷键分支；已运行进入 `startedByOther`。该状态供页面表示另一入口启动的录制，不能按本页按钮刚刚启动来解释。

按钮开始调用 `startMacroRecording("kSoftware")` 并等待 `RECORDING` 事件。快捷键分支也使用该模式。进入 `disablingMapping` 时，源码调用：

1. 向 `RzWinStorage_macro_recorder` 写入字符串化的 `{mode:"exclusive",payload:{macro_recorder_mode:"kSoftware"}}`，此调用没有被等待。
2. 等待 `disableMapping()`；该状态的成功与错误转换都会进入“检查仍在执行的宏”。
3. 查询录制前的显示器和虚拟桌面信息，然后进入 `recording`。这次显示器查询失败时，源码明确以空数组／空对象继续该状态机分支。

软件录制的这个启动状态只调用 `disableMapping`。按键编辑器与录制快捷键输入框会同时暂停映射和全局快捷键，见后文；不能把三个入口的调用混写。

## 录制事件到底记录什么

当前原生录制器在拥有 Windows 消息循环的线程上安装低级键盘和鼠标钩子。键盘事件保留普通／系统按下和释放；源码不统一过滤注入事件和重复事件。鼠标事件命名了移动、左右中键、滚轮和水平滚轮；其他消息仍可保留为数值动作。

键盘和鼠标共用前一个墙钟微秒时间戳。首个间隔为 0，后续间隔保留原版有符号 32 位截断语义。移动使用批次，阈值为 1000 且条件为超过阈值，因此会出现 1001 项的移动批次；其他事件会交付前面的移动缓存。停止还交付尾批。

普通动作回调携带 FILETIME 纪元墙钟微秒；开始、停止及停止尾批使用另一条单调时钟。不能把回调到达 UI 的时间当成原版动作时间。`recordingHandler` 与 `reserveMouseMovePositions` 分别把单项和移动批次放入状态机上下文。

## 停止、后处理与保存

显式停止按钮和全局录制快捷键发送 `ON_STOP_RECORDING`。状态机进入 `stoppingRecorder`，复制动作数组、清空部分快捷键缓存，调用共享窗口项删除，然后调用停止保护函数 `dr`。原生 `ON_RECORDING_STOPPED` 再推动录制后显示器查询与后处理。

原生停止事件直接到达 `recording` 时，会直接进入录制后查询，绕过 `stoppingRecorder`。这一分支没有隐含执行共享窗口项删除。

后处理事件携带停止原因、录制快捷键、开始 tick、动作、鼠标移动模式和前后屏幕信息。`Nr` 做对应动作整理并分派保存 action；保存 reducer 的回调发送 `ON_MACRO_SAVED`。状态机随后等待 `enableMapping()` 和 `enableGlobalShortcut()`，成功后清空暂存动作并返回 `idle`。该恢复状态的错误转换没有同样的成功清空动作。

当前 `ON_MACRO_ITEM` 的转换数组把无条件 `recordingHandler` 放在前面；后面的 Escape keyup abort 条件因此被前面的候选遮挡。`abortingRecorder` 发出 `aborted` 事件，但这个完整状态没有出边；`stoppingRecorderAfterAborted` 在该机中没有入边。这里记录原版实际图，不能把状态名当成必然执行的停止链。

页面卸载调用的原生停止函数、按钮显式停止、原生停止事件和窗口状态清理不是同一个调用。其差别直接影响哪些后处理和恢复动作会发生。

## 按键编辑与快捷键输入

键盘动作编辑器的点击进入会暂停映射和全局快捷键，失焦／退出后恢复两者。录制快捷键编辑器同时接收浏览器键码和输入重定向事件，并在点击／聚焦、失焦、外部点击与卸载时管理暂停和恢复。

底层调用返回 `{result,reason}`。`result:false` 是原版回调结果数据，不能把“Promise 完成”解读成操作返回 true；传输拒绝则是另一条异常路径。

### 映射与快捷键暂停的原生所有者

[当前对照查询](../../../.analysis/results/host-current/local-audit.json)的 `macro_recorder.native_mapping_suppression` 已沿四个导出、实际虚表、线程投递、状态消费者和释放路径保留73个完整原生函数、8段实际PE表及22个槽位目标。原件为当前宿主包内 `mapping_engine.dll`，3,945,672字节、版本1.3.16.5，Image Base `0x180000000`；下列地址均为RVA。这个范围解析了暂停语义，不代表整个映射引擎已经迁移或完成实际DLL对照。

| 边界 | 当前官方实际路径与结果 |
| --- | --- |
| 查询映射开关 | `isMappingEnabled 0x286a0 → engine vtable+208 → 0x68f0` 读取全进程的原子字节 `0x3811d8`。它不是当前页面／设备的计数，也不证明任何宏已经停止 |
| 禁用映射 | `0x28c20 → engine+216 → 0x6b90 → 0x6d50 → manager+24 → 0x580b0`。true才改为false，通知各输入设备，并把 `0x57090` 排回manager线程；该任务取消活动执行、清等待队列并停止exclusive executor。导出completion在这些任务排队后调用，**没有等待每个设备通知和宏清理完成** |
| 恢复映射 | `0x28910 → engine+224 → 0x6900 → 0x6ac0 → manager+16 → 0x57f70`。false才改为true，`0x5804c` 逐设备投递启用通知，再报告成功；不重新创建已经停止的宏 |
| 重复映射操作 | 已关闭时再disable为 `false / "already disabled"`，已开启时再enable为 `false / "already enabled"`。这两个导出不带caller身份或配对release token；一个调用者的enable可以释放另一个调用者之前的disable，不能按原生引用计数实现 |
| 输入消费者 | InputDevice、BLE、USB、external、OS-hook构造函数安装的对应接口表均以 `+8/+16 → 0x6a490/0x6ab90` 投递 `0x6a5e0`，由设备线程消费映射状态变化。`0x66870/0x66d10/0x909da` 在端点／动作选择时读取同一个全局字节；通知到达、端点重选、OS动作与导出bool是不同观测 |
| 禁用快捷键 | `0x4d8c0 → engine+888 → 0x15280 → 0x15440` 取得HotkeyService lease并存入engine `+232`。重复disable不再借用，但返回 **true**，reason为 `"Warning: Already disabled"`。有thread而底层service未建立时，借用可为空，当前owner仍可完成true，不能据此确认快捷键已停 |
| 恢复快捷键 | `0x4d5b0 → engine+880 → 0x14fb0 → 0x15170 → 0x1ae04/0xc9282` 清空并释放这个lease。原先无lease返回 `false / "hotkey was not disabled"`；缺thread返回false。它与映射开关的重复调用结果不同 |
| 真正的暂停计数 | `0xc6690/0xc91f0` 创建计数为0的对象并持有基础引用到1；`0xc9246` 的 **1→2** 调 `SuspendHotkeyHandling(true)`，`0xc9282` 的 **2→1** 调false。其他内部借用可以继续保持暂停，但导出的重复disable没有增加一次借用，因此不是每caller一对计数 |

快捷键暂停的OS后端也已追清：`0xc7f90 → 0xc9338 → 0xc9364` 创建的是 `RegisterHotKey/WM_HOTKEY` 服务，实际表 `0x2e97c0` 的 `+8` 为 `0xc6ff0`。暂停逐项走 `+40 → 0xc9aa0` 撤销注册；恢复逐项走 `+32 → 0xc97c0` 重新注册，再调用 `+16 → 0xc9790`。后者保持已为true的enabled不变；原先disabled时，仅在实际注册树非空才置true。`0xc97c0` 只有RegisterHotKey成功才插入树，因此原先disabled且全部重新注册失败／没有快捷键时仍为false，部分失败但至少一项成功才可满足置true的条件。暂停分支的逐项注销本身没有无条件清enabled，不能将“全部失败仍false”套用到所有先前状态。每项失败会记日志并继续；`0xc94c0` 还检查enabled与消息786，另由 `0xc741a` 投递观察事件。**这条导出链没有停用另一套GlobalShortcutWin低级钩子**，不能见到相同“快捷键”名称就合并两条后端。

`0x15170/0x15440` 完成的是lease操作，不等待已经投递的service线程任务，也不汇总逐项OS注册结果。四个导出真实ABI均为void、以 `callback(bool,const char* reason)` 交付；`0x28740/0x28a50/0x4d3e0/0x4d6f0` 仅在callback非空且DLL gate开启时转发。`0x1fb30` 在初始化回调前开gate，`0x1fdd0` 在shutdown回调后关gate。宿主四个完整wrapper用GUID在cbMap保留FFI callback，回调后删除；没有给这段Promise定义独立超时／取消，丢失回调不能解释成操作已取消。

当前Rust [共享控制](../../../crates/razer-ipc/src/mapping_control.rs)与 [执行所有者](../../../crates/razer-service/src/runtime/windows/mapping_runtime/backend.rs)用scope集合协调本程序的暂停；`Core::mapping/shortcuts` 的聚合ActionReply与上述原始DLL回调不是同一种结果。`execution_confirmed` 接受上述三种重复恢复／映射原因作为“目标状态已满足”，但仍保留原Observation；不能在差分比较中把其聚合通过改写成DLL返回true。录制结束、状态开关已恢复、等待宏取消和真实OS快捷键恢复须分别对照。

必要的独立测试包括：映射首次／重复enable与disable的原始bool和精确reason、不同caller交错操作、回调早于设备线程通知／取消完成、缺thread／manager、快捷键首次／重复disable与无lease的enable、额外内部lease保持暂停、service未初始化、RegisterHotKey部分失败与全部失败／空注册树下的enabled区别、回调gate关闭及卸载时cbMap清理。表和函数原字节核验已通过，**这组行为测试、实际DLL输出和设备／OS副作用对照尚未执行**；不能用纯状态模拟替它们登记通过。

## OTFM 与软件录制的差别

PID756 的设备事件入口先写一个对象形式的共享窗口记录，字段位于顶层，含模式、设备和 container 身份，没有 `mode:"exclusive"`；随后经全局广播发布 `start-otfm`。

后台管理器接到事件后，先写另一份字符串化 exclusive 记录，再请求名为 `RzGlobalLock_macro_recorder`、带 `steal:true` 的锁。之后注册录制／系统监听、枚举本机设备、选择鼠标移动模式、读取亮度并发送 OTFM 按下事件。真正开始原生录制以前，它还要检查已有录制、暂停映射及全局快捷键，并读取显示器信息。

OTFM 状态机保留按键保存、Escape、系统事件、拔出和超时等转换，并分别处理录制停止、灯光恢复、映射恢复和完成通知。共享窗口记录的移除又受后台待保存动作与完成标志控制，不能用一个“录制结束”回调概括。

初始共享状态扫描读取顶层 `macro_recorder_mode`；软件生产者和实时事件消费者读取 `payload.macro_recorder_mode`。这是当前源码两条路径的实际字段差异。其消费者对于移除／清空事件会重新查询其余 owner，决定是否仍有 exclusive 持有者。

## 114 个当前产品触发入口

[逐产品触发结果](../../../.analysis/results/frontend/background-manager-current/otfm-trigger-owners.json)沿各自当前 HTML／main 的异步 chunk 引用，逐一解析事件处理器、词法绑定、DeviceInfo 常量和实际广播，得到 **114 个不同产品键、114 个 producer**。按获取目录为 41 个 system、73 个 keyboard；这只是证据目录分类，不能替代源码的 DeviceInfo.category。完整证据为 229 个实际 JS 原件中的 2,160 个 AST 所有者，没有未解析的 producer 参数绑定。两个不属于当前入口引用的旧分块已排除；后台唯一的 `start-otfm` 监听不是 producer。

| 实际 producer 模块 | 当前产品键 |
| --- | --- |
| 10745（79） | 545、550、551、563、564、565、570、571、574、576、577、579、580、581、582、587、588、589、591、592、594、595、597、598、600、601、602、606、614、617、621、622、623、624、630、633、634、642、650、651、652、653、659、660、661、664、669、670、671、672、673、674、675、678、679、688、689、691、694、695、696、697、709、710、711、716、719、720、721、724、727、728、736、737、740、741、746、747、2596 |
| 47012（29） | 515、521、529、534、542、552、554、556、567、575、585、586、590、593、599、605、616、618、619、620、631、647、654、655、658、677、717、752、2595 |
| 76979（4） | 555、569、708、742 |
| 26568（1） | 756 |
| 62005（1） | 739 |

各入口从实际设备 decision 事件找到 `openOTFM`，先写对象形式的 recorder WindowStorage，再执行事件 task，然后调用同一词法 owner 的 GlobalBroadcastChannel.postMessage。广播含六个字段：`eventType="start-otfm"`、当前 container、DeviceInfo.productId、runtimeCache.layoutId、按该产品号／container 构成的 `usb_5426_…` deviceId、`macro_recorder_mode="kOTFM"`；频道再补 source／timerTick。PID 来自产品常量，不能误写成枚举到的接收器号。找不到 `openOTFM` 时，原版先移除当前事件，后面仍读取其 type；不能把这条分支描述为安全提前返回。

例如 PID737 的实际入口为 `6737.85397ce157d4cca228ee.js [75238,75820)`，main 的 `[90052,90118)` 异步加载包含该 chunk；DeviceInfo 的 `[25214,25756)` 明确 productId=737，container 和 runtimeCache 分别经模块7414／52262导出。每个产品都保留对应主入口引用及参数绑定，不能拿这个示例代替其余 113 项。模块号相同也不代表产品常量、能力或全部设备写回分支相同。

触发入口与下一节的 102 项参与表具有不同职责：二者交集 77，表内另有25项，producer另有37项。必须继续按真实查表顺序、USB／BLE／接收器身份及主从设备判断参与结果；**入口存在、广播成功、后台参与、宏保存和真实设备生效仍是分别验证的结果**。

## OTFM 的完整设备表与实际参与对象

后台 [OTFM 原件](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/assets/otfm-d56d4f50.js) 的 `Wt` 完整表为 `[119904,146443)`。逐个 AST 调用和字面量依赖解析得到 **102 个不同键，无重复覆盖：99 个键盘、2 个鼠标、1 个附件**。它不是所有产品均可发起 OTFM 的能力声明；它决定一次后台录制时哪些连接对象可参与灯光和输入。下表保留全部键及真正选用的类，不以同族推断缺失产品。

| 默认类／类别 | 完整表内产品键 |
| --- | --- |
| rzDevice25／键盘 | 542、550、551、552、554、556、565、567、574、575、577、579、585、590、591、592、593、599、600、606、614、617、619、620、642、647、653、655、658、659、660、661、664、673、674、675、677、678、679、688、689、697、716、719、720、721、724、727、728、739、740、741、742、746、752、756、2595、2596 |
| rzDevice25LedMatrixBlade／键盘 | 563、570、571、595、601、618、624、630、633、634 |
| rzDevice25WirelessKeyboard／键盘 | 604、625、656、662、698、713、722、725、757；配对对象改选 rzDevice25DualLinkKeyboard |
| rzDevice30／键盘 | 603、626、657、663、693、699、714、723、726、758 |
| rzDevice25／鼠标 | 103、117 |
| rzDevice25LedMatrix／键盘 | 515、521、529、534、545 |
| rzDevice25MikaWirelessKeyboard／键盘 | 2308、2593 |
| rzDevice25LegacyHardwareEffect／键盘 | 654 |
| rzDevice25ChromaEffect／键盘 | 602、605 |
| rzDevice25Oled／键盘 | 691、692 |
| accessory／不建立灯光设备 | 179；其 ignoredRazerKeys 仍被输入过滤使用 |

`bo [152052,153437)` 每次重新建立键盘／鼠标 Map，合并真实 USB、BLE 枚举与本机 `duallink-devices`，逐设备等待 `Ao [153438,160935)`；单个异常只记录并继续。`Ao` 以 `dongleId || productId` 查表，无表项或 accessory 返回 false；从共享 `connectedDevices` 中按 container 和 category 找对象，以 `primaryPid || productId` 生成 `synapse_<pid>`。缺匹配对象时直接访问其字段会抛错，并非自动选择 PID0 或默认设备。

配对键盘通过 manifest 的 `dualLinkClassName` 另选类；有 master 时还创建 DualLinkMouse，以 master 的 dongle／interface 连接，再登记 secondary 身份。随后等待连接、必要时查询 dual-link 信息，才放入 Map、注册 unsupported mapping。**等待 connect Promise 与检查连接结果是两件事**：这个调用方没有统一核其返回的成功字段，也没有逐次失败后的完整 disconnect 回滚。表中 interface 0／2／3、双链参数、协议30及各类 overrides 必须按真实类继续测试，不能由这张表认定全部 HID 分支已完成。

灯光服务遍历整个 Map；它不只控制最初发起录制的键盘。10 个 LedMatrixBlade、565 和两只鼠标都没有宏指示灯；654、593、567、565 的 effects 是 `static,static`，不进入 ReactiveShort 分支，654 还使用 region=5。其他表项的灯光、输入忽略项和接口分别保留在完整表，不从键盘类别猜测。输入忽略集合由触发 PID、双链 secondary、已配对身份共同形成；`Ut=[84,85,83]`、`Vt=[82]`，不同产品选择不同组合，179 的忽略规则也不能丢弃。

## 两张表的25／37差异与保存的实际结果

[资格及保存结果](../../../.analysis/results/frontend/background-manager-current/otfm-eligibility.json)保留2,387个完整AST所有者、100个原件中的精确区间、51份当前获取收据，以及目录原始行和所有差异产品的DeviceInfo。表内25项与producer外37项现已逐键追清，不能把两张表取并集当作支持清单。

表内25项有22个传输别名键，其中20个组成以下10组；每一组的primary均已有当前producer：

| primary | dongleId | bleId |
| --- | --- | --- |
| 602 | 604 | 603 |
| 600 | 625 | 626 |
| 658 | 656 | 657 |
| 664 | 662 | 663 |
| 691 | 692 | 693 |
| 697 | 698 | 699 |
| 716 | 713 | 714 |
| 724 | 722 | 723 |
| 727 | 725 | 726 |
| 756 | 757 | 758 |

另外2308同时是574和117的dongle，2593同时是2595和2594的dongle；身份关系不唯一，不能仅凭alias选primary。剩余103／117是鼠标参与项，在本次当前producer集中没有对应入口；179是被Ao明确排除的accessory，但其ignoredRazerKeys仍参与过滤。这里的“没有入口”只限已审计当前集，不宣称全部来源均不存在其他触发路径。

producer多出的37项按**实际DeviceInfo.category**分类如下；其primary及目录声明的dongle／BLE都不在Wt：

| 类别 | 全部差异键 |
| --- | --- |
| SYSTEM（31） | 564、569、576、581、582、586、587、588、589、594、597、598、616、621、622、623、650、651、652、669、670、671、672、694、695、696、709、710、711、736、737 |
| KEYPAD（3） | 555、580、747 |
| KEYBOARD（3） | 631、708、717 |

这37项均沿实际输入dispatcher → ON_OTFM入队 → 状态guard／锁 → allow dispatcher → TaskMaker执行入口 → openOTFM → 广播解析，相关提取未决为0；TaskMaker初始化属于setup前置，先于这条输入处理链。guard读取事件配置和状态，并不查Wt；通用JS路径存在仍不能证明对应固件会发出该输入。`bo`普通USB／BLE投影不携带dongleId或master；`Ao`先做`Wt.get(e.dongleId||e.productId)`，通过后才查connectedDevices.primaryPid。**primaryPid不能补救前面失败的查表**。真实duallink记录可以带dongleId、master和isDualLinkPairedDevice，使已命中的表项改选其`protocol.dualLinkClassName`；className并非来自配对记录，目录声明也不证明这种运行记录存在。

始发入口`co`本身没有Wt门槛。它顺序调用lo／锁／观察者／bo／go／fo／Lo，不检查这些helper fulfilled false；Promise rejection仍会中断。`fo`按始发container从真实USB／BLE枚举找输入记录，与参与Map独立；RegisterDevice结果未被等待或核对。`Lo`即使在键盘Map找不到该container，也会把亮度失败的100交给状态机，再向Yt发ON_OTFM_PRESS。因此可以出现“后台录制已启动、始发设备却未成为可读写参与对象”，不能把两者合成资格成功。

这条差异还会到达**保存失败**：Ct的idle第一次收到ON_OTFM_PRESS才设置keyboardStarted；录制中再次收到ON_OTFM_PRESS时只更新keyboardEnded，普通另一键盘输入不替换始发身份。processingPostRecording把原keyboardStarted交给Ro，后者执行`S=to.get(u.containerId||Yt)`后立即读`M=S.localStorageKey`，没有S检查，也没有挑另一键盘。若正常Lo(Yt)是首个PRESS且该container未进入键盘Map，异步listener会在这里抛错，早于宏`n.set(D)`、设备配置持久化和ON_MACRO_SAVED。该状态只处理SAVED／REJECTED，不能凭之前录制启动声称已保存或自恢复。

上述失败结论有确切时序条件：更早的异步unsupported-mapping PRESS可能改变首次keyboardStarted；实际配对参与对象或同container另一有效记录也可能改变Map。它不是对37款设备的运行失败断言。测试须分别固定首次PRESS来源、普通／配对枚举、相同／不同container、主从alias、硬件是否真实发事件、注册结果、缺Map前的亮度100及保存抛错点；实际DLL和设备对照继续保持未验证。

## OTFM 的按键选择、灯效恢复与保存结果

完整状态机 `Ct [106327,119495)` 的 `startingRecorder` 按 `result.result` 决定后续灯效；多数 LED／恢复状态的成功和拒绝出口却指向同一下一状态。录制中再次按 OTFM 进入分配按键；录制／分配阶段的Escape keyup、锁屏、切换用户、拔出和120秒无动作超时进入取消，普通动作和首次鼠标移动分别重置超时。分配键超时为120秒，有鼠标移动时另留10秒选择relative／absolute／foreground；**此选择阶段的Escape keyup或analog scancode1仅跳过选择，继续后处理，并不取消宏**。键盘的1／2／3与analogKey的scancode2／3／4是不同输入表示。

`ho/mo [148514,149844)` 在重置后的首 300ms 过滤触发键，累积按下／释放配对；razerKey1 管理 Hypershift gate，analog release 会减 flag 再交消费者。`wo [149925,150848)` 根据布局键表构造 `ON_MACRO_ITEM`。`assignkey` 为 true 时订阅输入并置 `qt=true`，普通 recorder item 此后被 `Io [151075,151235)` 丢弃；退出分配仅取消订阅，这段没有把 qt 重置为 false。不能把浏览器按键、原生 recorder 回调和设备输入通知当作同一事件源。

完整服务 `$o [166585,172266)` 的真实结果有以下边界：

- `serviceGetDeviceBrightness` 对缺设备／异常返回 100，状态机据此跳过或执行灯效。这是官方局部回退路径的证据，100 不能标为设备读回；本项目仍须遵守未观测与失败分离规则。
- 宏指示灯 Static 先 GET 当前 effect，必要时改 effect，再写 LED；Blinking 写 effect 后点亮。ReactiveShort／Long 先等亮度写入，再广播 `SET_QUICK_EFFECT`。广播没有灯效完成 ACK；Long 分支实际也检查 ReactiveShort 的存在性。
- 错误灯以 100ms interval 交替调用开／关 10 次，interval 内没有等待各次写入；计数完成不证明十次设备应答全部成功。
- `serviceRestoreLightingEffect` 先等 2000ms，再从当前本地 profile 取亮度和 quickEffects。每个对象先等亮度调用，再广播 `RESTORE_LIGHTING_EFFECT`；外层 `Promise.allSettled` 没有检查 rejected 项，effect 广播也未被等待。因此状态机可继续 enablingMapping／completed，**不能称为真实原值已恢复**。这里是本地 profile 的退出目标，非录制前硬件全状态备份。
- `serviceCheckingServiceRecovered` 交给 retry 的函数恒为 true，没有检查服务、连接或设备；进入 recovered 状态不能提升为实际恢复。该源码事实不是本项目可采用的服务健康判据。

后处理 `Ro [161207,166522)` 仍在 recorder 停止前运行：先清理 numpad 身份，含鼠标移动时检查虚拟桌面几何是否变化，通过后才丢开头孤立 keyup、补未释放键的 keyup。几何变化发 `ON_MACRO_REJECTED`；当前状态机走恢复灯效→错误灯→enable→completed，**这一分支没有经过 stopMacroRecording**。保存被接受时则等 `ON_MACRO_SAVED` 后才停止 recorder。两条路径不能写成统一的“先停止再保存”。

新宏生成 GUID／name，映射选择 Once、UI repeatCount=2；真实投影仍按 Once 编译器决定次数。普通键、DKM、AnalogInput 各有对象结构；替换已有 AnalogInput 时保留其触发点／rapidTrigger，并补第二段 defaultGroup。保存顺序为更新编辑配置、绑定草稿及普通／analog appEngine与hash → 等IDB set → 等原生 `synapseMacros` 发布Promise → 递增version并以 `p(A,Me)` 构建profile → 按条件向带container的键发布映射 → 等浏览器设备profile保存 → 发 `ON_MACRO_SAVED`。映射发布保留 `B` 存在才写实际PID键、`W!==M` 才写另一PID键的条件，并非固定两次。最多三次原生发布虽然被await，**均不检查返回的false，fulfilled false仍继续保存和发事件**；只有Promise rejection才中断后续。发布没有共同事务，也不等待数据解析、设备映射应用或硬件确认。异步事件处理器没有覆盖整段的catch，异常拒绝可能使状态机继续等保存事件，不能凭前面已写入部分数据显示整体成功。

`completed` 将录制活动标记清除，再释放锁／删除共享项、停止系统和 recorder 监听；这些调用不全被等待。已取消分支与在途保存事件可以交错，不能让旧完成事件释放新录制的所有者。上述完整所有者没有统一关闭本次创建的每个设备 transport，也不确认所有指示灯／输入已恢复；独立 Rust 生命周期、全部产品事件消费者及真实 DLL／设备对照仍须分别实现和验收。

OTFM 测试须覆盖表内每类／接口／双链分支、枚举局部失败、多设备同时参与、起始亮度0／失败、两个输入源与300ms边界、修饰键／Hypershift、无鼠标／含鼠标、几何变化拒绝、各保存阶段失败、原生map拒绝但Promise完成、取消／拔出／锁屏与在途保存、2000ms恢复及部分拒绝。期望应区分原版真实行为与本项目明确禁止的失败回退；纯测试、DLL执行、真实恢复与页面提示分别留收据。

## XML 导入、导出与文件写入

导出先 `structuredClone` 当前 GUID 对应的宏，把 `appEngine.mouseMoveType` 转成 actionBar 的 `recordProfile.mmtSetting`，再生成 Version=4 的 XML。外层字段为 Name、MacroEvents.MacroEvent、DelaySetting、Guid、Version 和 MouseMoveType；后者缺失时写 `none`。XML 使用 compact 模式和 3 个空格缩进，文件名为宏名称加 `.xml`。导出的是当前编辑对象，并不先要求该对象已经进入 `savedMacros`。

文件保存经 `showFileSaveDialog` 进入 Electron。主进程选择上次保存目录或 Downloads，显示保存对话框后直接调用 `fs.writeFile`；取消、路径缺失和写入错误分别停留在该异步链／日志中。主进程的这个 action 没有把对话框和文件写入回调返回给调用方，宏菜单也没有等待保存调用。因此导出 action 发出、桥接 Promise 完成和文件确实写入是三个不同事实，不能据前两者显示已成功落盘。

导入读取文件选择器的第一个 `.xml` 文件，通过 `FileReader.readAsText` 后解析；完整输入所有者没有注册 FileReader 的 `onerror`。解析回调递归解码字符串中的 XML 实体，只有 `Macro.Version[0]` 严格为字符串 `"4"` 时进入 V4 转换器，其余进入 DOMParser 和旧格式转换器。该回调没有直接处理 XML 解析库返回的 error 参数，不能把后续存在分支判断概括成完整的损坏文件校验。

V4 转换器读取名称、GUID、延时设置、移动模式和动作，重新建立编辑历史及动作 ID。Type 的实际分支包括 actionBar、0 延时、1 键盘、2 鼠标按钮、3 移动、4 文本、5 命令、6 循环、7 嵌套宏和 10 附加动作；其他 Type 不生成动作。键盘／鼠标配对在后续项中寻找匹配 Type 和释放状态，没有同时核对同一键码／按钮，不能按理想的配对算法描述这一原件。

旧格式转换器另处理扫描码和扩展键标记、按下／释放栈、滚轮、移动时间与坐标、循环、命令和嵌套宏 GUID。鼠标移动模式存在 `1→2、2→1、3→3` 的转换；扫描码 29 的歧义选项排除 KEY_PAUSE。导入 reducer 再执行重名处理、生成新 GUID，旧格式路径把鼠标按钮值减 1，并重建原生动作投影及 hash。新宏同时加入编辑列表和已保存列表，附加文件夹索引记录；只有原列表为空时才把当前 GUID 切到导入项。它不是覆盖导入文件中的 GUID 所指旧宏。

旧格式转换结果与导入 reducer 对 `appEngine.mouseMoveType` 的直接访问还需要连同全部格式样本核对；静态存在旧格式分支不等于所有旧 XML 均能成功导入。所有动作类型、数值精度和未知字段的独立转换测试仍需逐项建立。

## 已保存宏、编辑配置与原生运行投影

当前宏有三个独立持久化／发布对象，不能合成一个“保存成功”位：

| 对象 | 内容与实际写入边界 |
| --- | --- |
| 浏览器 IndexedDB | 数据库 `macros`、版本 1、同名对象仓库，`guid` 为 keyPath 和唯一索引；保存完整的已保存宏动作 |
| 浏览器 localStorage | `macroEditorConfig` 保存当前 GUID、firstTime、文件夹、列表索引及录制配置；不是设备实测状态 |
| 原生 local storage | `synapseMacros` 或 `synapsePhasedMacros` 发布运行所需动作及 hash；该名称对应映射引擎内存服务，不是浏览器 localStorage／IndexedDB 的同一存储 |

首次迁移先读 IDB，以现有 GUID 集合筛去浏览器旧 `synapseMacros` 中的重复项，等待合并写入完成后才设 `macroMigratedToIDB=true`。随后读取编辑配置和 IDB：已保存列表来自 IDB 的深复制，编辑列表清空选择／配对标记和撤销／重做历史。迁移标志不表示设备已经接收宏，也不是设备首次初始化条件。

IDB 的 `get` 使用名为 `macros-idb` 的 shared navigator lock，`set/add` 使用 exclusive lock；三者均建立 readwrite 事务。`set` 在一个事务里读旧列表、clear、逐条 put、commit，等待 `oncomplete` 后才向 `macros-change-channel` 发布 `{oldValue,newValue}`；事务错误拒绝调用。`add` 实际使用 put，因此是按 GUID 更新或插入，但广播的 newValue 直接拼接旧数组，重复 GUID 时不能把广播数组当成唯一键存储的原样快照。这个模块通过频道的 `onmessage` 发本地 change 事件，未在同一写入方法里自发 change，也没有频道 close 所有者。

`Js` 的投影来源是 `savedMacros`，不是尚未保存的 `macroList`。IDB 数据去除 actionLoad、deviceList 和 saved；运行投影还去除 macroEvents，整理 appEngine.events 并重算 hash。`qs` 先调用原生 `localStorageSetItem`，但既不等待该 Promise，也不检查返回结果；前端适配层把主机的 `{result,reason}` 投影成 result，reason 不再向该调用方保留。它根据参数和当前宏类型选择普通／分阶段键，之后才按参数等待 IDB 写入，再写编辑配置。这三个对象没有共同事务，传入 `appEngine:false` 也不会完全抑制原生发布，而是使该条件走普通宏键。

普通动作编辑会保留 Draft，并把宏标为 `saved:false`；保存／导入等 reducer 分支先更新逻辑已保存对象，再调用 `qs`。常规 reducer 不等待 `qs`，部分分支随即释放 `MacroLocker`；该锁持有的是 `STORAGE_LOCK_synapseMacros` 和 `STORAGE_LOCK_macroEditorConfig` 的 shared lock，与 IDB 的 exclusive lock 不同。OTFM 模式可跳过 IDB／配置写入而继续运行投影发布。带保存的 reload 分支等待 `qs` 后再解锁和刷新，带保存的关闭窗口分支只发运行投影便关闭，不能假定它等待 IDB 落盘或原生 ACK。

`macro-saved-report` 的 detail 直接取 reducer 结果中当前宏的 saved，供窗口关闭策略消费；它不是事务、原生解析或设备写回确认事件。独立 Rust 实现需要分别保留 Draft、已持久化对象、原生运行数据是否已确认及设备状态，不能把这个 UI 标记当作后端提交结果。

## 原生发布 ACK 的完整边界

主机映射适配器在未初始化时返回 `{result:false,reason}`；已初始化时保留 `void(bool,string)` 回调，调用实际导出，回调到达后才删除 cbMap 项。这个方法未设置超时或取消；异常与 `result:false` 也不等于成功。

当前 `mapping_engine.dll` 的静态调用链为 `localStorageSetItem`（RVA `0x20920`）→ 映射线程队列（`0x2fa0`）→ 本地存储服务分派（`0x3160`）→ 存储线程队列（`0xd46ec`）→ `SetItemOnThread`（`0xd4900`）。这里的导出虚调用已经核对构造函数及原始 PE vtable：Image Base 为 `0x180000000`，表 RVA `0x2df010` 的 `+0x20` 槽实际指向 RVA `0x2fa0`，不是仅按函数名猜定间接目标。

空 key 或空 value 返回 `invalid parameter`，映射／存储线程缺失返回 `invalid thread`，服务缺失返回 `invalid local storage service`。`SetItemOnThread` 在原生 map 中新增键、替换不同字节的值，或在值相同时保持不变；新增／变化通过观察者任务队列通知消费者，相同值不再通知。随后排队返回 true 和空 reason。这个函数体没有写宏 XML、浏览器 IDB、设备或 HID，也没有等待宏消费者解析与执行，因而 true 只确认这次原生 map 阶段。

完成包装器 `0x20680` 还要求引擎初始化标记仍为真才调用外部回调；初始化完成设置该标记，shutdown 完成清除。页面正在关闭和原生停止之间的迟到结果不能假定一定会回到 JavaScript。普通 `synapseMacros` 与 `synapsePhasedMacros` 有各自的数据服务／查找入口；以下分别记录播放与输入消费者，不能从 map ACK 推断解析、执行或设备写入已经成功。

## 数据观察者、运行快照与删除行为

`0x562c8` 分别创建普通 MacroData（`0xf68c0`）和 PhasedMacroData（`0x107a24`）；`0x56850` 先注册五个存储键的观察者，再异步读现有值，`0x5eb10` 仅将成功且非空的初值交给服务。PE 表 `0x2e4c88` 的 +8／+16 都指向 `0x5cb50`，分别处理新增／变化；+24 是删键 `0x5cc20`，+32 是清空 `0x5ccb0`。新增／变化先替换 manager 原始字符串缓存，再经服务表 `0x2ecbe8+8 → 0xf6a70` 排队到专用数据线程 `0xf6bd0`。**存储 ACK 不等待解析完成**，这两条异步链没有共同完成条件，也不能固定其物理调度先后。

普通格式器 `0xf7106` 为输入建立候选 Map：顶层必须数组，每项必须对象；guid 必须字符串且批内不重复，appEngine 必须对象，hash 必须字符串。这里不检查 GUID 非空或 UUID 格式，绑定动作 parser 的非空要求属于另一层。若已发布 Map 中同 GUID 的 hash 字节相同，直接复用旧 MacroEvents，跳过摘要验证和新 events／mouseMoveType 读取；相同 hash 搭配新正文并不保证新正文生效。

新项／变化项由 `0xf86ae` 复制整个 appEngine，只删除其顶层 hash，再以原生 JSON writer（options=0、最大深度200）和 `0x1b3a30 → 0x1f3600` 计算小写 MD5，按字符串字节比较。额外字段也参与；另一个排除 gamemode 的摘要入口不能套到这里。writer 明确转义 `<`、U+2028、U+2029，键序与数值格式还必须用独立样本比较。mouseMoveType 缺失／非字符串时为内部0；表 `0x2ecd20` 的顺序为 none／absolute／foreground／relative，未知字符串为内部4并使批次失败。events 必须数组，但本阶段只深复制，不逐项解析动作；空 events、坏动作与实际执行失败是独立分支。

`0xf6bd0` 仅在整批成功后才加锁、更新key并交换候选／已发布Map。任一坏项、重复GUID、摘要不符或构造失败都不会部分发布，也不发送成功变化通知。因此可能同时存在**已更新的存储原值、已更新的manager字符串缓存、仍旧的运行快照**；再次提交完全相同字节不会触发存储变化，不能假定会自动重试解析。这些对象都不代表设备实测状态。

变化GUID列表只对成功的新项／hash变化项经 `0x6237c` 追加。成功发布 `[]` 或省略旧GUID会移除已发布对象，却不把被删GUID加进变化列表。普通表 `0x2e4da8+8 → 0x5e710` 与分阶段表 `0x2e4dc8+8 → 0x5e4e0` 都进入 `0x5e600`：仅按列表GUID扫描活动树并调用 `0x12772a` cancel，不进一步按 caller／普通或分阶段类型区分，也不扫 pending queue、exclusive slot或嵌套依赖。不能写成所有宏更新、删除都会停止所有相关播放。

删除整个存储键／清空走 `0x5cc20 / 0x5ccb0 → 0xf6f60`，只清已发布key／Map，不发送上述变化列表；也不撤销已排队的数据任务或建立提交代次。删除ACK不能证明旧解析已撤销。普通查找 `0x5d020 → 0x5cf60 → 0xf7066` 与分阶段 `0x5ce40 → 0xf7066` 在读锁内查已发布Map并增加引用；缺owner／Map／GUID返回空，不以manager原始字符串替代。

观察任务 `0xb1640` 在交付前核注册身份和序号；`0xb0ab0` 移除观察者，再注册产生新序号。manager 关闭 `0x56b40` 的 continuation 顺序为 CleanupDevices `0x571e0` → Stop executions `0x57090` → ShutdownMappingExecutors `0x56f10` → CleanupLocalStorage `0x56e70`，最后才释放存储引用和数据owner表。普通／分阶段析构 `0xf69c6 / 0x107b2a` 清两份Map、key、线程和引用；`0x1b8d40 → 0x1b8590` 安排StopSoon，`0x1a9710` 等线程退出并关闭句柄。最终 `0xd3c70 → 0xd3d1e` 退订存储观察。不能通过提前删除数据owner替代这条停止与释放链。

此链补核124个函数、124chunks、10,682条汇编及5组虚表；capture所记SHA、chunkSHA、实际PE机器码均一致。必须测试整批坏项、空／重复GUID、同hash异正文、Unicode／数值／额外字段、无效新值与旧快照、相同字节重发、GUID省略／删键／清空与在途解析、普通／分阶段同GUID、队列／嵌套、观察者重注册和关闭时序。静态语义闭合不代表Rust已实现或实际DLL对照已通过。

## 运行投影与绑定播放方式

保存链的 `Ks` 实际调用模块 13139 导出的 `convertMacroDataToAppEngine`，其完整动作转换器是 `m`；这不是仅凭同名函数选取的未使用副本。键盘经当前键表转换扫描码和 flag，空状态生成 down、up 两项；仍保留动作自带 ScanCode／flag 的修正分支。普通宏的 hash 输入是 events 与最终 mouseMoveType；分阶段宏先按 phase 整理 press／hold／release，再转换各数组，hash 只包含这三个数组。编辑器内部 `standard/sequence/phased ↔ 0/1/2` 与设备绑定的播放方式字符串是两组身份，不可互换。

| 编辑动作 | 实际运行投影 |
| --- | --- |
| 固定／随机延时 | 秒乘 1000 后 Math.round，生成 delay.ms 或 randomDelay.minMs／maxMs；转换器的 Number／空值处理与 native 数值解析是不同边界 |
| 鼠标按钮 | 左键 down/up 为 1/2，右键 4/8，中键 16/32，X1 64/128，X2 256/512；State=null 时转换器实际先生成 up、再生成 down，与键盘空状态的顺序不同 |
| 双击／滚轮 | 双击仅 State=0 时生成 1、2、10ms 延时、1、2；垂直滚轮 id=1024，水平 id=2048，正向 data=120、反向 data=65416（低 16 位的 -120） |
| 鼠标移动 | absolute／foreground 使用主显示器宽高归一化到 65535 后四舍五入；relative 按录制前选中的显示器 dpiScaleX/Y 换算。缺失主显示器直接抛错；每个样本的时间另转 delay，保留累计小数补偿 |
| 文本／命令／本地文件 | 文本生成 clipboard/text 后接 500ms；命令生成 launch 后接 50ms，taskmgr 有专门命令转换。附加启动动作还保留 startHidden、pathToCheck 和自身延时，不能统一成一个裸字符串 |
| 循环／嵌套宏 | 循环开始为 flag=0、repeat=次数，结束为 flag=1；嵌套为 macro、guid、flag=0，前端不提供 repeat，默认值由嵌套 native 解析上下文决定 |

当前宏主原件与 PID3946 MW 的绑定编译器具有下表投影。表中“未提供”表示字段为 undefined 并在 JSON 中省略，不能擅自补成 1；具体产品是否提供这些方式还要由其 mounted 面板和能力分支决定。

| macroPlaybackOption | downOutput | upOutput | 当前 native 消费边界 |
| --- | --- | --- | --- |
| Once | macro、flag=0、repeat=1 | disabled | 忽略残留 repeatCount，显式只执行一轮 |
| NTimes | macro、flag=0、repeat=repeatCount | disabled | 按实际整数 repeat 解析 |
| ContinuousHeld | macro、flag=0、repeat 未提供 | macro、flag=1 | 绑定上下文默认 repeat=0，无限重复；释放取消匹配的普通活动项 |
| ContinuousToggle | macro、flag=2、repeat 未提供 | disabled | 有普通活动项则取消，没有则开始；parser 不读取 repeat |
| Sequence | macro、flag=3、repeat 未提供 | disabled | 独立 cursor 路径，parser 不读取 repeat |
| Queue | macro、flag=4、repeat 未提供 | disabled | 队列默认 repeat=1；不是普通活动表的去重路径 |
| `"Phased"` | phasedMacro、flag=0 | phasedMacro、flag=1 | 分阶段类型 31，使用独立数据源 |
| `"PhasedMacro"` | **macro、flag=5** | disabled | 当前普通宏 parser 只接受 0…4，不能把此字符串自动解释为 phasedMacro |

普通宏解析器 RVA `0xed8d2` 先要求非空 GUID 和整数 flag，以 unsigned 判定 0…4。顶层 `0xef470` 与 multi `0xf2f76` 传默认 repeat=0，并允许 toggle；嵌套事件解析器 `0xecee3` 传默认 repeat=1，之后拒绝 flag≥2。flag=4 先改默认 repeat 为 1，再尝试可选 repeat；flag=0/1 保留调用方默认值，flag=2/3 不读取 repeat。可选 repeat 缺失或类型不可接受时不会自动变成 1，已接受整数的 32 位位型直接保存，不能凭反编译形参签名声称存在次数上下界检查。

顶层 parser 支持 phasedMacro；multi.outputs 的 `0xf2f76` 没有 type=31 分支，会走 Unsupported。分阶段 parser `0xf36f2` 在 flag≥2 时构造错误文本却继续分配对象，执行入口 `0x8d1b8` 才再次拒绝非法 flag；缺失 delay 由 `0x102eb0` 设为 30ms。必须分别测试“parser 返回对象”“留下错误文本”和“实际被执行”，不能把这三项合成一次成功。

## 普通循环、队列与逐次播放

普通宏设备触发经 `0x893dc → 0x5d600 → 0x61aa0 → 0xa9500`，其中 manager vtable RVA `0x2e4cd8` 的 +48 槽已核为 `0x5d600`。活动表以 GUID＋caller 区分：flag=0 有同键活动时忽略新触发；flag=1 signal 匹配活动；flag=2 在取消／启动之间切换。它们不与 queue exclusive slot 合并，不能把释放普通 Held 宏解释为停止同 GUID 的队列任务。

flag=4 把 GUID、endpoint、mapping、caller 追加到 manager 队列，不按 GUID 去重。`0x5f086` 在队列非空、exclusive executor 空闲且没有等待的全局 stop completion 时取队头；排队失败继续取下一项。`0x5de50 → 0x5eeec` 在这一 exclusive executor 完成后清 slot，再启动下一项。因此队列内部一次一个活动，普通活动表仍可独立运行。`0x127892` 只确认线程排队，实际执行要到 `0x127aa0 → 0x128692 → 0x12b5b0`。普通执行栈达到 100 时拒绝嵌套；缺 GUID／空 events 失败；repeat=0 是无限，非零在每轮后递减。

Sequence 在 `0x893dc` 提前转 `0x89968`，使用设备＋GUID 的 cursor map。`0x8edfc` 查普通宏数据，拒绝缺宏／空事件；hash 变化会替换上下文并重置 cursor。每次触发由 `0x8f0e4` 推进到一个可消费动作：keyboard down 发送后克隆成 release 再发送，原始 keyup 跳过；鼠标按钮同样补 release 并跳过原始 up，滚轮直接发送一次。循环维护自己的计数与回跳；delay、mouseMove、audio 等不具备普通 runner 的同等执行分支，不能因普通事件转换器支持就声称 Sequence 全支持。

Sequence 嵌套由 `0x8f5e0` 在父上下文保留子 GUID／cursor：子未完成时父项不前进，子到尾才移除并继续父项。顶层到尾可从头开始，但单次调用有一次 wrap 的 gate，避免只有不可执行项时无限扫描。普通 runner 的 100 层栈限制不能直接套给这条递归链。launch／clipboard 的推进 bool 与 OS 副作用确认不同；坏对象／parse 失败也不能统一改写成普通 runner 的 status=1 或页面提示。

Sequence 的实际输入发送是 `0x85ea8 → 0x10f582 → 0x1186c0 → 0x112c60`，最后一个目标已核实为 DriverImpl vtable `0x2f0990` 的 +96 槽。驱动线程缺失回调 false；`0x114850` 在实际 I/O completion 的 status=0 时交付 true，经 `0x110b5a → 0x115da0 → 0x118840` 返回调用线程。`0xa3910` 仅在这一驱动发送回调 false 时尝试系统 SendInput，fallback 结果不再返回 Sequence cursor；`0x85ea8` 自己缺少 driver owner 时只记录日志。不能据此给所有普通键盘执行失败添加同样的回退。

## 分阶段数据、执行与取消

分阶段数据使用 `0x107a24` 的独立 owner／线程；查找 `0x5ce40 → 0xf7066` 读取 synapsePhasedMacros。`0x107bd0 → 0x107d30 → 0x1080b2` 构造候选 map，核对唯一 GUID、appEngine／hash 以及 pressEvents、holdEvents、releaseEvents 数组；相同 GUID／hash 复用旧对象，变更重新核对。候选全部成功才在锁内替换活动 map 并通知消费者。发布 map 的 ACK 早于这条解析结果，不代表新数据已生效。

实际 phasedMacro 仍经 `0xa9500` 按 GUID＋caller 管理活动，flag=0 在同键运行时忽略，flag=1 取消；`0x127aa0 → 0x129998 → 0x12ab8e` 执行三个阶段：

1. 执行 press，检查取消。
2. 反复执行 hold 直到取消；hold 空时按 delay 等待，delay=0 时直接等取消信号。
3. 正常取消后执行 release；release 不在每项入口因为取消信号已置位就退出。press／hold 的硬失败可提前跳过 release，仍进入通用 pressed-map 清理。

阶段 helper 仅用 `0xedce6` 解析 keyboard／mouse，相邻等价动作经 virtual +32 判定后不重复发送，但保留等待。已发送的 down/up 维护 pressed map 和上一动作。原件的 parse 失败会跳过该项且可保持 bool=true；非 object 与实际发送 bool=false 则失败。此控制流已同时核对汇编，不能把原件的跳过当作已经验证并执行该动作。正常取消后 release 即使成功，最终仍是取消 status=2；数据 owner 析构 `0x107b2a` 另清候选／活动 map、停止数据线程并释放引用，不属于单个播放项的 ACK。

## 键盘、鼠标与系统输入参数

JSON 整数解析 `0xdb930 / 0x1b4940` 要求当前 JSON 库的 signed32 integer kind；`0xdb873` 从该整数截取低 16 位，并无严格 u16 范围检查。键盘 scancode／flag、鼠标 id／data／unitId 与 x/y 的类型及截断必须在输入边界分别测试，不能用安全 Rust 的强制范围检查冒充当前 DLL 的结果。

| 输入 | parser、身份与执行行为 |
| --- | --- |
| Keyboard | `0x10bc0d` 要求 scancode、flag；modifiers 默认 0，unitId／delayInMs／releaseAfterMs 可选。构造 `0x10cbc4` 清零 Windows 字段；pressed 身份 `0x10d180` 含 type、scancode、flag & 0xfffe 和显式 unitId，不含 up bit。flag bit0 为 up，bit1/2 在系统发送中合成 extended |
| Mouse button／wheel | `0x10bfed` 要求 id；data、modifiers 默认 0，unitId 可选。validator／down／up 检查的是 id＋data 的完整 DWORD，不能只检查 id。pressed 身份 `0x10dd90` 合并 down/up 为 3、12、48、192、768 等配对身份并保留显式 unitId，wheel 使用完整描述且不进入按钮 pressed map |
| MouseMove | `0xedada` 要求 signed32 x/y，负数保留；type=28。普通 macro frame 的 mouseMoveType 决定 absolute／foreground／relative，不从数值大小猜模式 |

专用键盘链 `0x130c1a → 0x126902 → 0x126a80 → 0x12ee96 → 0x12f3ce → 0x12fd66` 中，排队、合法性与实际发送不是同一结果：合法性仅要求 type=21、scancode≠0；endpoint 有效时用空 completion 发送并返回 true，没有把实际 I/O bool 合入当前键盘事件结果。系统重复参数由 `0x12f312` 读 SPI：默认 delay=250ms、rate=30ms；delay 为 250×值＋250，rate 为 int(1000/(值×0.887＋2.5))。delayInMs 可覆盖，last-key／timestamp 拒绝旧重复任务；releaseAfterMs 每 250ms 递减，减后小于 0 才发 release，不是精确 deadline。

系统键盘 helper `0x11c510` 构造 40 字节 INPUT，type=1、wScan=扫描码；dwFlags 为 SCANCODE(8)、扩展标志(1) 和 KEYUP(2) 的组合。扫描码 29 且 flag bit2 置位还递归发送扫描码 69、仅保留 up 位的动作。modifiers／unitId 不在这个 helper 中另行注入；SendInput(1,…,40) 返回 1 才是该 helper 的 true。鼠标按钮对应 Windows LEFT 2/4、RIGHT 8/16、MIDDLE 32/64、X 128/256（mouseData=1/2）；垂直／水平滚轮分别用 2048／4096，**data 恰为 120 才发送 +120，其他值都发送 -120**。驱动路径保留原始 data 位型，非标准滚轮值会在两条路径产生差异。

直接 endpoint vtable RVA `0x2f2e70` 的发送槽已核对为 `0x134c80`／`0x135880`，IOCTL=0x88883020，包长 32 字节；Sequence 的 DriverImpl 输入发送也使用该 IOCTL 与布局：

| 偏移／字段 | kind=1 键盘 | kind=2 鼠标 |
| --- | --- | --- |
| 0 u32、4 u32 | 0、1 | 0、2 |
| 8 u16、10 u16 | unitId、scancode | unitId、raw movement flags |
| 12 u16、14 u16 | keyboard flag、保留位 | button id、button data |
| 16 u32 | 转换后 modifiers | raw buttons |
| 20 i32、24 i32 | 0、0 | x、y |
| 28 u32 | 0 | 转换后 modifiers |

`0x1219dc` 将输入 modifiers bit4/5/6/8/9/10 映射到输出 bit8/7/6/2/1/0。`0x13579a` 的同步成功或 ERROR_IO_PENDING(997) 仅表示 accepted；`0x135c20` 的实际完成 status=0 才交付 true。Stop `0x134af0` 取消 I/O 并关闭句柄，`0x134b64` 等 pending 归零后完成 shutdown；关闭期间某个完成项可被移除而不再交付原 bool，不能编造“取消必回 false”的统一语义。

## 鼠标坐标转换与输入结果消费者

普通鼠标移动 `0x130dc6` 的三条路径须分别测试：

| 模式 | 当前 native 计算与状态 |
| --- | --- |
| absolute=1 | x/y 直接交 `0x1314d0`，不重复执行前端按屏幕归一化 |
| foreground=2 | 连续移动段首项读取前台窗口矩形与虚拟屏幕宽高（SM 78/79）并缓存；px=int(left＋x×width/65535)，再算 int(px×65535/virtualWidth)，y 类似。原件使用浮点后截断，没有减虚拟桌面原点，也没有额外零尺寸保护 |
| relative=3 | 首个样本只记录原点并成功，不发送位移；后项发送与上一样本的差。每轮普通宏开始会重置移动状态，不能将录制样本直接作为相对增量逐项发送 |

`0x1314d0` 的 PE 静态映射 RVA `0x2f28e0` 为 [1,1,3]：absolute／foreground 交 endpoint mode=1，relative 交 mode=3。`0x135880` 将它们转为 raw movement flags=1／0 并保留 signed32 x/y，结果来自实际 I/O completion；endpoint 无效时转系统 helper `0x11cbd3`，前两种模式用 MOVE|ABSOLUTE(0x8001)，relative 用 MOVE(1)，**没有 VIRTUALDESK(0x4000)**。系统 SendInput 返回 1 才回 true；未知模式回 false。

普通事件分派 `0x12be6e` 中，mouseMove 的 completion 接 `0x12c8f0`，合入 frame.valid 并推进 iterator；keyboard／button 使用各自排队或有效性 bool，没有逐项等待物理输入完成。audio／clipboard／delay／randomDelay 的底层 bool 未合入这一 frame，launch 和 nested 另有返回值路径。鼠标重复中键 down 有 `0x10e2f0` 的完整 DWORD=16 gate，pressed map 已含对应身份时直接推进为 true；不能泛化为任意按钮的重复按下去重。

## 文本粘贴、剪贴板恢复与失败结果

clipboard 的 parser `0xed512` 要求字符串 id／text。实际 id 表 `0x2edcf0` 为 text、image，但 parser 只接受 text（内部 0）；image 和未知 id 都失败，不能把存在枚举项当成可执行功能。`0xfd72c` 保留文本，`0x1295ca → 0xb9f3c → 0xbb6b6` 取得全局 ClipboardService 并调用 PasteText，传当前 endpoint data 和 wait=true。缺服务、线程或 endpoint 返回 false；线程排队成功后等待 `0x1b9fa0`，没有独立超时或宏取消参数。这里最终返回的是排队 bool，不是剪贴板写入、驱动应答或接收应用的文本确认。

实际任务 `0xbbc50` 经 `0xbc6de` 放入独立的串行任务列表：标记序列运行；仅在活动粘贴计数从 0 变 1 时暂存原剪贴板；`0xbd090` 写入文本；调用 endpoint SendPaste；结束序列并安排恢复。暂存 `0xbc970 → 0xbd2f0` 枚举并读取支持的 text/plain、text/html、text/rtf、text/uri-list、image/png，回调分别记录格式位；不是任意格式的原样字节备份。

文本写入从 ScopedClipboardWriter `0x1dc768 / 0x1dc35e` 到 Windows vtable `0x33ce70` +256（`0x22e640`），经有限的 5ms 间隔 OpenClipboard 重试后 EmptyClipboard，最终 +264（`0x22e730`）创建带终止 NUL 的 UTF-16 HGLOBAL，按 CF_UNICODETEXT=13 调 SetClipboardData。打开失败时不写入；SetClipboardData 失败则 `0x22e830` 释放 HGLOBAL。它们没有向这一粘贴序列返回写入成功 bool；CloseClipboard 和必要的 RevertToSelf 由 `0x22c85a` 完成。

SendPaste 的间接目标已核实：`0x8dc00` 是 +32 槽 thunk，当前 ClipboardEndpointWin vtable `0x2e8e50` 的该槽为 `0xb82e0`，排队到 `0xb8490`。它按 endpoint identity 建立／复用直接输入 endpoint，连接失败只记录日志并继续 completion，没有“目标已粘贴”的成功事件。有效 endpoint 的 `0xb91a0 → 0xb9560` 实际顺序为 Ctrl down（扫描码 29、flag=0、后等 0ms）、V down（47、0、50ms）、V up（47、1、0ms）、Ctrl up（29、1、10ms）。`0xb9800` 用输入 bool 决定是否保留该项等待，再继续下一项，未将失败汇总成 PasteText 的 false；弱引用／endpoint 无效时也会推进 completion，不能用回调到达证明已发出 Ctrl+V。

这条粘贴链还以 `0x11c460` 读取当时左右 Alt／Ctrl／Shift／Win 的 GetKeyState；外层 `0xb9050 → 0x11b849` 先发对应 release，Ctrl+V 后 `0xb8c00 → 0x11be52` 补回 down。辅助发送 `0x11bc70` 有自己的 endpoint／系统发送选择，结果不汇总给宏；不能把它与普通键盘 pressed-map 或粘贴主键序列的失败行为合并。

`0xb90e0 → 0xbc400 → 0xbd190` 把发送 continuation 返回服务线程，安排 500,000µs 后执行 `0xbcdd0`，随后 signal 等待者；**等待结束早于延迟恢复**。恢复计数递减，仍有并发粘贴时保留原备份；最后一项按 text → HTML → RTF → URI → PNG 的优先分支恢复一种受支持表示，原备份无对应格式时清空。不能宣称它完整恢复所有第三方格式，也不能把前端额外 500ms delay 等同恢复 ACK。宏取消信号没有传入已排队的粘贴序列或该计时器；通用宏完成不等待这一恢复结果。

服务停止另走 `0xba770 → 0xba910`，将 thread_state 置 3，销毁当前线程的 Clipboard 实例并调用 endpoint CleanupBeforeShutdown（`0xb7850 → 0xb79e0` 清 active／待清理 endpoint map），经 `0xbc2e0` 向 owner 线程交付完成；状态再置 4，`0xbc6de` 不再接受新序列。析构 `0xba1b8` 清 singleton、停止／释放线程和备份对象。这条停止链没有单独返回每个文本写入、Ctrl+V 或原剪贴板恢复的 bool；须把服务清理完成与各项恢复结果分别验收。

## 程序启动参数、错误与进程生命周期

launch parser `0x10be63` 要求 path 字符串，pathToCheck 可选且默认空，startHidden 可选且默认 false；不存在“仅允许 exe”的 parser 判断。`0x1297d8 → 0x11ae38` 仅在 pathToCheck 非空时转换成 Windows 路径并调用 `0x1b1430` 的 GetFileAttributesW；返回 INVALID_FILE_ATTRIBUTES 就写 reason=`file not found` 并失败。这个检查只判断路径存在，目录也可能满足，不校验 path 命令中的其他参数／依赖文件。

启动选项由 `0x1b2dd0` 初始化：wait=false、无额外 token／job／显式环境块或继承句柄列表；startHidden 写入隐藏选项，`0x1b1720` 尝试把工作目录设为 GetTempPathW 的结果。path 转成宽字符命令行后交 `0x1abf10`，当前选项实际进入 CreateProcessW，lpApplicationName=NULL；STARTF_USESHOWWINDOW 下 nShow 为 hidden=0／normal=1。它不等待被启动程序完成，也不检查退出码。

创建失败后才退到 ShellExecuteExW：verb 的实际 UTF-16 原件为 open，lpFile 是同一整段 path，lpParameters／lpDirectory=NULL，fMask=SEE_MASK_NOCLOSEPROCESS(0x40)，nShow 同上。Shell 失败写 reason=`fail to launch`；成功时关闭返回的 hProcess。正常 CreateProcess 的持有句柄交 `0xcf650 → 0x1a7cf0` 的 ScopedHandle 释放链；GetHandleVerifier 分派的本地实现 `0x1efc70 → 0x1ef9c0` 实际调用 CloseHandle。当前启动选项没有终止或等待逻辑。**宏取消／结束不表示已启动的进程被结束**，不得自行增加 TerminateProcess 作为通用清理。

普通 macro 的 `0x12be6e` 将这次 launch bool 经 `0x12c8f0` 合入 frame.valid，reason 保留在 frame；最终 status=1 才由 manager 发错误。launch 的 true 是 OS 创建／Shell 请求成功，与新程序初始化、目标文件处理或命令实际完成不同。直接映射 executor `0x127aa0` 同样检查 launch bool；Sequence 经自己的 cursor／设备分派使用推进结果，不能复用普通 frame 的错误语义。

## 宏音频动作到 Windows 音量 endpoint

audio parser `0xed10b` 的实际 id 表 `0x2edb50` 为下表七项。all／mic／speaker 至少要有一个有效整数 volume 或 mute；volume 仅接受 0…100，mute 仅接受 0…2（0=解除静音，1=静音，2=切换）。四个步进 id 直接构造动作，不消费额外 volume／mute；未知 id 拒绝。这里有明确范围检查，与 keyboard／mouse 的低 16 位截断不同。

| id／内部值 | `0x128e76` 的实际动作 |
| --- | --- |
| all／0 | 有 volume 时先依次排 speaker、mic 设置；有 mute 时按各自相同值设置。toggle 读取服务的已初始化缓存，两个 endpoint 可读时，“任意一个未静音”即将两者置静音；只有一个可读时按该项切换 |
| mic／1、speaker／4 | 各自设置 volume／mute；toggle 来自 `0xb29f8 / 0xb2994` 的缓存快照，不额外向 OS 同步 GetMute |
| micVolumeDown／2、micVolumeUp／3 | 分别调用 `0xb436c / 0xb3fd2` 的 microphone step down／up |
| speakerVolumeDown／5、speakerVolumeUp／6 | 分别调用 `0xb3180 / 0xb351a` 的 speaker step down／up |

全局音频线程由 `0xb215c` 获取。setter／step wrapper 使用空 completion，把任务投向服务线程；实际服务 vtable `0x2e8aa0` 的 +88…+144 分别是 speaker mute／volume／up／down、mic mute／volume／up／down（`0xb4d40、0xb4e80、0xb4fd0、0xb5110、0xb5250、0xb5390、0xb54e0、0xb5620`）。构造 `0xb4b2c → 0xb5788` 分别保留 flow=0 的播放 endpoint 和 flow=1 的录音 endpoint；`0xb5cd2` 读取 default endpoint、role=1，并以 IID `5cdf2c82-841e-4546-9722-0cf74078229a` Activate IAudioEndpointVolume，未按某个宏 GUID 指定设备。

monitor 的 PE vtable `0x2e8b40` 与短 thunk 汇编已核实：volume 到 `0xb6230` 的 +56，mute 到 `0xb6280` 的 +112，step up／down 到 `0xb62f0 / 0xb62c0` 的 +136／+144，再进入 IAudioEndpointVolume 的相同槽。volume 的汇编以 float(volume)/100.0 交 SetMasterVolumeLevelScalar，mute 交 SetMute，step 交 VolumeStepUp／Down，event context 都为 NULL。缺 endpoint 返回 E_FAIL；HRESULT<0 生成含 errorCode、errorString 的 reason 和 false，成功才返回 true，不能将线程排队当作这些 COM 返回值。

结果 consumer 仍有上下文差别：服务线程把真实 bool／reason 经 `0xb4a42 → 0x62510` 投回请求线程，但宏用的是空 completion；`0x128e76` 不等待该结果，普通 macro `0x12be6e` 还直接以 true 推进 audio／clipboard。直接映射 executor `0x127aa0` 会检查 helper 的即时 false，分别产生 audio fail／clipboard fail，却仍未取得异步 COM／粘贴结果。不能把直接映射的检查套给普通宏，也不能用普通宏 status=0 证明音量或文本修改成功。

OS 音量观察是另一条链：`0xb6320` 的 OnNotify 从通知结构读取 mute 与 scalar，按 int(scalar×100＋0.5) 投回 monitor owner；`0xb61d0 → 0xb0c20 / 0xb0ef0` 更新服务缓存并发布观察者通知。宏没有把某次 setter 与这个通知配对形成读回确认。停止宏也没有撤销已排队音频任务或恢复原音量；monitor 清理入口 `0xb6220 → 0xb598e → 0xb5e9a` 注销 endpoint／设备通知并释放 COM 引用，`0xb4bfc` 另释放两个 monitor。这是长期服务生命周期，不能让每个宏播放项提前释放共享服务，或把未发生的原值恢复报告为成功。

## 音频服务重建、观察者与宿主事件

服务启动 `0x19a40 → 0xb2164 / 0xb24b8 → 0xb2640` 创建服务，经`0xb4c70 → monitor vtable+24 → 0xb6210 → 0xb59c0`初始化两个monitor。`0xb6210`丢弃初始化HRESULT并恒返回0，服务启动者也不检查该结果。`0xb59c0`先注册IMMNotificationClient，再`0xb5cd2(true)`读取default role=1端点；GetDefaultAudioEndpoint、Activate、读取mute／volume、注册音量通知是不同结果。取得IMMDevice后，即使后续Activate或读值／注册失败，仍可能经service+24／+32（`0xb10d0 / 0xb1150`）把缓存标为initialized；读值失败可保留构造零值，**服务初始化完成与缓存initialized均不能当作真实端点已读取**。

默认设备通知表`0x2e8c10+48 → 0xb63f0`只过滤flow，未检查role。它将ID排到owner的`0xb6100`：ID相同直接结束；不同则先`0xb5e9a`注销旧音量通知并释放旧端点，再`0xb5cd2(false)`重建，但不检查重建结果，仍经service+40／+48（`0xb11d0 / 0xb1480`）发布default变更及更新缓存。失败重建也可能产生空ID／名称及零值事件，不能用default事件确认新端点可用，更不能据此触发默认音量覆盖。

普通音量通知经`0xb0c20 / 0xb0ef0 → 0xb0cd0`，default变更经`0xb11d0 / 0xb1480 → 0xb1286`，都排回观察者注册线程。`0xb1640`交付前核observer身份及序号，`0xb0ab0`退订使尚未交付的旧任务失效。engine真实表`0x2df430`的+8／+16为speaker／mic音量`0x17ee0 / 0x18010`，+24／+32为default speaker／mic`0x18130 / 0x182a0`；四者先检查engine初始化和hook。音量两项随后捕获hook再经`0x1d38e → 0x1d190 / 0x1d1a4`排回engine请求线程，第二级任务不再检查初始化。因此清hook和退订不能撤销已经派生的外部回调；default两项则直接调用注册hook。

当前[主机音频适配器](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/mapping_engine/win/index.js)将四种回调依次投影为type3／2／1／0的`speakervolumechanged`／`microphonevolumechanged`／`defaultspeakerdevicechanged`／`defaultmicrophonedevicechanged`，只向订阅URL匹配且仍有效的webContents广播。首个订阅者先入list、等待原生register为true，再依次设置四hook，**不检查四个set callback的false**；后续订阅者入list便返回true。退订先移除当前URL，最后一个才调用原生unregister，失败则回填URL。宿主订阅成功、原生服务就绪、观察值和写回确认仍是独立边界。

全局停止`0x1a4c0`先清初始化及四hook，再`0xb2986`退订、`0xb2732 / 0xb28c0`在音频线程经`0xb4cf0`停止两个monitor；各自`0xb6220 → 0xb598e`先注销音量通知／释放端点，再注销设备通知。销毁service后回owner，经`0x1a8e0`释放AudioServiceThread。宏停止不等同这个全局服务停止，也不能把第二级回调已排空作为未经核实的额外保证。

本扩展另核72函数／74chunks／3,374条汇编及5组表，均与同一当前原PE相符。新增测试分支包括role差异、相同／不同ID、无默认端点、Activate／读值／通知注册局部失败、重建失败仍发事件、退订重注册代次、两级队列与shutdown交错、首个／后续URL与四hook局部失败。未执行DLL／COM／应用，实际音频变化与清理验收仍未验证。

## 播放停止、释放与最终状态

`0x12772a → 0x11a922` 置取消信号，普通 runner 在下一步及可取消的等待中观察。`0x57cd4 → 0x57c64` 按 caller 删除其 pending queue，并取消匹配的 queue 活动与普通活动；`0x5d770 → 0x5d890` 还按 caller＋mapping 匹配。全局 `0x57090` 保存 stop completion，取消全部活动、清 pending queue 并取消 exclusive executor；完成消费者 `0x5de50` 等活动计数归零后才投递该 stop completion。这些 owner 不可由页面提前丢弃。

通用清理 `0x12a8ca` 克隆 pressed-map 项，经 virtual +96 转 release，再走键盘／鼠标发送，随后清 map、Stop endpoint、清 owner 并排队停止键盘线程。键盘 release 由 `0x10d340` 将 flag OR 1；鼠标 `0x10e110` 转成对应 up。原件不检查这些 release 的发送结果，通用最终完成也不等待带空 completion 的 endpoint Stop 或键盘 Stop，因此不能把宏完成描述为“所有释放和 pending I/O 已确认”。

最终回调状态的汇编实参为：valid 且未取消=0、invalid=1、valid 且已取消=2。manager+32 的 vtable `0x2e4d38` +8 → `0x5dc90 → 0x5de50` 仅对 status=1 发 type=5 error；取消不是统一错误提示。设备错误到宿主及产品消费者的后续边界见 [接收器／底座的普通宏执行链](../accessories/receiver-and-dock.md)。数据发布 ACK、executor 结束、stop completion、线程清理与实际释放结果需要各自保留，不能用同一个 completed 标记互相代替。

## 播放与输入的自动化／实际 DLL 对照要求

下列分支必须以独立期望逐项测试，并经显式测试入口与当前官方实际 DLL 功能结果比较；模拟与静态字节核对只证明各自范围。本页本次扩展没有执行 DLL、系统输入、厂商 JS、应用或设备，以下均不登记运行通过：

- 绑定每种播放方式、undefined／0／非零／负数 repeat、Phased 与 PhasedMacro、顶层／multi／nested 上下文及不合法 flag；覆盖同 GUID 不同 caller、重复触发、held release、toggle、queue 与普通活动并存。
- Sequence 的按下补释放、跳过原始 up、wheel、无可执行项、hash 替换、循环／嵌套 cursor、driver completion false 后的系统发送及缺 driver owner；分别观察 cursor 推进与真实输入结果。
- 分阶段三个数组、空 hold／delay=0、press／hold 失败、正常取消后的 release、parse 跳过、非 object、相邻等价项、数据候选失败及相同 hash 复用；不能用普通 runner 样本代替。
- 键盘扫描码／扩展位／unitId／modifiers／重复／releaseAfterMs；鼠标全部按钮及特殊 id＋data、标准与非标准滚轮值；signed32／低 16 位截断、负坐标、多显示器／DPI、foreground 舍入与 relative 首样本／每轮重置。
- 每种发送的排队失败、同步 accepted、pending 后失败、endpoint 无效、取消时不再回调、pressed-map 补释放、caller／mapping／全局停止和线程清理；收据分别记录事件 bool、frame status、stop completion、真实应答与恢复结果。
- clipboard 的 text／image／未知 id、Unicode 与终止 NUL、支持／未知原格式、原剪贴板快照与优先恢复、并发粘贴计数、OpenClipboard／SetClipboardData 失败、Ctrl+V 各步输入失败、修饰键暂释／恢复、500ms 延迟恢复和服务停止；排队返回／等待结束／实际文本／恢复必须分开记录。
- launch 的空／缺失／错误类型字段、存在／不存在／目录 pathToCheck、引号与参数、隐藏状态和工作目录、CreateProcess 失败后 Shell 成功／失败、文件未找到／启动失败的 reason、成功后的进程存续与句柄关闭；仅启动进程不能充当命令结果对照。
- audio 七种 id、volume／mute 范围、缺字段与 step 忽略字段、all toggle 的两端／单端／未初始化缓存、COM HRESULT／reason、通知缓存与宏完成先后、普通事件与直接映射消费者及取消后的已排队操作；可恢复对照记录真实初始音量／静音、通知／读回、恢复与释放。

有设备的可恢复写入对照须先读初值，写入后读回，再完整恢复与清理；没有相应设备／驱动时，只完成逻辑、模拟和有依据的回放，实际设备应答保持未验证。普通数据观察／快照／删除及音频服务重建／宿主观察者已补齐上述静态链；每个产品的 OTFM 触发与设备映射链仍须分别核对和测试，以上局部闭合不代表全部宏功能已经实现。

## 页面、弹窗与当前 Rust 调用方

官方 mounted 根 `$a` 实际按 page.current 挂载 My Macros（58190）、Key Binds（21700）或 Help（1519），同时挂载未保存确认、命令警告、OTFM 阻止交互提示和受能力控制的教程。Shell 生产路径已经创建并保留 [MacroPage](../../../crates/razer-app-pages/src/macros/macro_page.rs)，但页面功能、共享宏库落盘、软件录制及设备按键触发仍有不同的完成边界。

| 页面／状态 | 当前官方 mounted 行为 | 当前 Rust 生产路径及差异 |
| --- | --- | --- |
| 导航、历史和窄窗口 | `zn` 有 My Macros、Key Binds、Help，录制期间阻止切换；无宏时禁用 Key Binds；按翻译完成与窗口宽度测量，把放不下的导航移入更多菜单。初始页还接受本地 view 参数 | `chrome.rs` 与根页面已有三页、返回／前进、无宏禁用和录制状态判断，但导航固定布局，没有同一宽度测量／溢出菜单链；初始页固定 My Macros，history 是会话 Vec。刷新进入未保存确认后只重建本页 actions，不重新读取共享库或重跑官方初始化 |
| 宏选择、文件夹、搜索与上下文菜单 | `Mn/Bt/an` 有新建宏／文件夹、搜索投影、展开、拖放、复制、重命名和删除。新宏或切换宏前检查 saved；Header 与树的复制／名称条件各自不同。32 字符输入、空值／重复名称恢复按原消费者处理 | `state/tree/chrome` 已有真实本地集合修改、层级拖放、搜索、菜单、重命名和删除确认，动作进入共享 MacroLibrary；尚不能据这些集合变化判定设备映射引用已更新或触发已确认。文件夹／宏删除、唯一剩余宏页面跳转、草稿复制及跨文件夹重名仍需逐分支测试 |
| 编辑器、动作栏与空状态 | `Fr/ja/br` 挂载 42px 动作行、虚拟滚动、配对提示、拖入／拖动、选择、撤销／重做和 Save；空列表、录制、设置与教程分别控制禁用 | `body/row_*/selection` 已有延时、键盘、鼠标、嵌套宏、启动程序、命令、文本、循环的本地编辑器和 undo/redo，保存只将动作草稿发布至共享库。未完成全部动作／版本分支与原生执行的逐项对照，不能把可编辑或本地编译通过升级为执行语义一致 |
| Standard／Sequence／Phased | `Ka` 只在分阶段分支挂载三阶段、各阶段录制／删除／展开、phase radio、跨阶段拖放及分阶段配对；`Fr` 的 activePhase effect 按 macroType 触发 | `phased.rs` 有三阶段、局部展开、radio、阶段删除／拖放和保留状态；已有 active phase 的发布。但根 `render` 调用 `sync_phased_mount`，该函数可能修改 entry、发布共享库并 notify，仍有 render 副作用需迁出；这不是已经完成生命周期架构的证明 |
| 录制按钮、倒计时和实时动作 | `xa/Gr/br` 有再按取消倒计时、按钮停止、快捷键停止、录制设置关闭、实时行和录制后自动滚动；原版完整状态机还区分其他入口已开始、恢复失败和 OTFM | `recording.rs` 有 Idle/Countdown/Starting/Recording/Stopping、generation、所属 document、实时 preview 及 actor updates，完成后把动作追加至本地草稿；错误不提交 preview。可确认 UI 已有真实异步消费者，但其他入口、所有恢复／设备 OTFM 和对应实际结果尚需分别验收 |
| 录制选项、快捷键和帮助提示 | `$r/Lr/Rr/vr/Ur` 分别拥有设置、快捷键输入、延时和鼠标移动选项，按 macroType 和已存在动作决定可修改内容；外部点击／失焦／卸载控制暂停与恢复 | `record_options/record_shortcut/record_help` 保留输入、焦点、数值 debounce、代次和全局快捷键 watch；关闭选项停止捕获，Shell 根据页面捕获状态暂停音频快捷键。快捷键注册、输入捕获及暂停恢复结果必须与录制成功分开，不能把设置面板关闭当作恢复已确认 |
| 键盘、文本、表情和启动程序弹层 | `yn` 的键盘编辑具有延迟提交及失焦释放；`dn` 的文本只有自身 Save 提交、关闭丢弃，表情按自身列表和搜索插入；嵌套宏排除当前／循环引用；启动程序弹层另有独立草稿 | Rust 键盘 capture 有 retained session、150ms 提交、失焦清理和 RAII；文本有 250 UTF-16 单元限制、自身 Save／Cancel、表情与搜索状态。启动程序选择器校验 generation、document、index 和原动作快照，取消丢弃独立草稿；外层 Save 不提交尚未保存的文本／启动程序草稿。这些局部链已存在，全部位置、焦点、输入序列与原版对照仍未运行 |
| 未保存确认与关闭 | `Gr` 用 setForceFocus 管宿主窗口，Save／Don't Save 后继续 suspendedAction，关闭按钮仅取消继续；窗口关闭和 reload 有各自的保存发布参数 | `unsaved.rs` 在 New／Select／Refresh 前保留 PendingAction 和返回焦点，Save／Discard 后继续；Dialog 禁止 backdrop 和 Enter 默认提交，Esc 通过 on_close 取消继续。Shell 退出不生成这套宏草稿确认，只等待已有持久化和录制清理；尚未等价复现官方独立宏窗口关闭策略 |
| XML 文件正常、取消、损坏和保存失败 | `Mn` 导入第一个 XML，保留 V4／旧格式分支；导出当前编辑对象，文件写入与 UI saved 不同 | [transfer.rs](../../../crates/razer-app-pages/src/macros/macro_page/transfer.rs) 已执行真实后台 XML 读取／写入；Transfer 身份拒绝迟到结果，Drop 取消。导出用当前 actions，导入重新分配本地 ID／GUID 并处理重名；取消不伪造文件成功。仍需按本页 XML 分支与实际 DLL 结果对照，不能只运行 encode/decode 自回环 |
| Key Binds 卡片、设备选择与按键面板 | `U` 从真实 macro.deviceList 与 validDevices 合成绑定卡，处理同 PID 多设备、profileGUID、inputId、Hypershift、BossKey 和移除菜单；`M` 打开当前产品的 displayMode=macro 页面，传 container/edition/serial 身份，真正的参数与写入由对应产品消费者处理 | [bindings.rs](../../../crates/razer-app-pages/src/macros/macro_page/bindings.rs) 有设备／profile／物理输入／层／播放方式／次数、Save、Back 和断连重置，但 Save 的 BindingEvent 只更新 `MacroPage.local_bindings`。该数组不在 MacroLibraryFile 内，没有发产品映射命令或持久化；删除也只移除本数组。当前 Key Binds 不具备实际绑定闭环 |
| Help、教程、命令与 OTFM 提示 | Help 挂载支持 widget；My Macros 有首次引导及 capability 控制的教程；mounted 根另有命令 warning 和不能从外部点击关闭的 OTFM 提示 | Rust 有支持页和本地 Initial/Record/Add 引导，但没有与根对应的全套能力教程、命令／OTFM 弹层数据消费者。不能把录制 status 文本代替官方提示、设备状态或未观测分支 |

绑定弹层的实际键盘链也与设备写入无关：其 Dialog 的 Enter 调用 `save` 后返回 false，因此保持弹层；Esc 有当前 input 时只退回按键面板，否则关闭，关闭恢复 return_focus。该结论来自锁定 GPUI Kit revision `1676826d9d49b90f4d7b6808b69924f58796f6f1` 的 Cancel/Confirm 决策实现及本页回调，不是根据控件名称推断。原版产品面板的 Apply/Cancel／设备确认、录制取消及本地绑定数组必须独立记录。

## 共享库持久化、外部更新与离开页面

本项目 MacroPage 的 `save_actions` 把 actions 复制到页内 saved_actions，再调用 `MacroLibrary::replace`；此时只是编辑器逻辑保存。Shell 的 MacroLibraryChanged 订阅把快照同步给快捷键与设备 workspace，随后 `save_auxiliary_preferences` 在后台实际执行 `write_workspace`。成功回调只确认写入的快照，并对对应已保存 revision 发布产品映射文档；失败写入保留真实错误。编辑器 saved_actions、MacroLibrary.pending、磁盘写入及映射／设备确认是四个不同事实。

外部 profile 宏导入有独立 MacroLibraryProfileImported 事件，Shell 会更新快捷键和设备 workspace；但当前 MacroPage 只在 `new` 读取一次 library.snapshot，此后的本地 entries/actions 没有订阅此事件或共享库变化。页面 `refresh` 也只从自己的 entries 重建 actions。已打开宏页面时发生外部导入／更新，页面可能继续展示旧副本，再次 publish_library 还会用旧 entries 替换共享库；这一消费者缺口须先解决并测试，不能将 Shell 已收到事件解释为打开中的宏编辑器已同步。

离开宏 Location 或隐藏／退出窗口时，Shell 调用 `deactivate_recording`，停止快捷键捕获、关闭 watch 并发送录制 Cancel。`request_exit` 观察 `recording_shutdown_pending`，清理结束前不继续退出，再取消文件 Transfer。actor 保有清理线程，页面 generation/document 检查防止旧录制结果写入新文档；这些是已有调用链，未执行真实钩子、映射恢复或设备验收。页面 Esc 还会 finish_pending_edits：数字／键盘编辑可被提交至草稿，文本和启动程序走自己的丢弃链，因此不能统一描述成 Esc 撤销所有正在编辑的数据。

本轮页面视觉依据包括 [main CSS](../../../.analysis/inputs/frontend/synapse-macro-current/raw/apps.razer.com/synapse/macro/static/css/main.9ea5d7e7.css)、[编辑器 CSS](../../../.analysis/inputs/frontend/synapse-macro-current/raw/apps.razer.com/synapse/macro/static/css/8190.5ef5dbbc.chunk.css) 和 [绑定 CSS](../../../.analysis/inputs/frontend/synapse-macro-current/raw/apps.razer.com/synapse/macro/static/css/1700.1adda236.chunk.css)。例如未保存面板为 400px、#111 背景、绿色 1px 边框、20×30 padding；分阶段 header 高 50，普通行高 42，阶段背景／禁用 radio／hover 动作图标和 tooltip 各自有条件。Rust 有部分相应几何与动画，但没有运行窗口、图像、焦点／键盘或所有宽度状态验收，不能登记整页 1:1 已通过。

后续测试须分别覆盖三页与窄窗口导航、首次引导／能力教程、文件夹与宏菜单、当前和非当前草稿、全动作编辑／配对／循环／嵌套、三类宏和阶段拖放、录制每个开始／停止来源、恢复失败、文本／表情／文件选择取消、共享库外部导入、绑定每个产品／布局／附件／Hypershift／播放方式，以及设备切换后的旧结果与真实写入确认。本轮仅静态核对，没有新增“已通过”的运行或 DLL 收据。

## 证据定位

本页的维护查询 [本地完整捕获](../../../.analysis/results/host-current/local-audit.json)分别保存宏页／后台／宿主、114个当前producer、25／37差异资格与保存消费者的完整所有者和原件身份；精确数量以本页下方当前查询结果为准。语义说明按完整AST、原生函数和实际原件字节而写。

获取收据在 2026-10-21 重新核对入口、asset-manifest 和 manifest，三个端点在获取前后均保持同字节，主脚本仍为 `main.f7cc2d30.js`。收据的 recorded 时间记录此次核对，文件 acquired 时间仍保留首次取得日期；`declared_references_fetched=false` 及缺失 map／动态分块未决项继续保留，部署稳定不代表资源与依赖闭包完整。

| 所有者 | 原件与 UTF-8 字节区间 |
| --- | --- |
| 状态机、启动 | `synapse-macro-current/.../main.f7cc2d30.js`：`hr` `[670408,670958)`；`RECORDING` `[674904,682224)` |
| 后处理、宏保存 reducer | 同原件：`[682309,686326)`、`[739505,756173)` |
| 按钮／倒计时 | `synapse-macro-current/.../8190.a1e2b107.chunk.js`：`[1411647,1413120)` |
| 按键编辑、录制快捷键 | 同分块：`[1391473,1394655)`、`[1433519,1437619)`、`[1438179,1439968)` |
| 导航、树和根挂载 | 宏 main：`Bt [601078,601915)`、`an [603236,607130)`、`Mn [633195,641824)`、`zn [649074,654598)`、未保存 `Gr [689211,690349)`；根 `$a` 的 local 页面／弹窗消费者位于 `[712495,715521)` |
| mounted 动作编辑器与阶段 | 8190 分块：文本 `dn [1382149,1385850)`、嵌套 `En [1385888,1387684)`、行 `va [1404516,1411348)`、阶段 `Ka [1413121,1416958)`、列表 `ja [1417272,1420936)`、动作栏 `br [1448474,1454702)`、根 `Fr [1454703,1457927)` |
| 绑定页面／面板／卡片 | [1700 分块](../../../.analysis/inputs/frontend/synapse-macro-current/raw/apps.razer.com/synapse/macro/static/js/1700.978c1608.chunk.js)：`I [1968,4078)`、`M [5188,7812)`、`U [7866,11200)`；Help 位于 [1519 分块](../../../.analysis/inputs/frontend/synapse-macro-current/raw/apps.razer.com/synapse/macro/static/js/1519.cf991d46.chunk.js) 的 mounted C |
| 原生钩子、时钟、尾批、退出 | 捕获中的 `native_macro_input`、`native_global_shortcut_clock` 和完整原生函数 RVA；输入身份见 [获取收据](../../../.analysis/inputs/frontend/synapse-macro-current/acquisition.json) |

本页 XML／存储扩展另复核了 [宏主原件](../../../.analysis/inputs/frontend/synapse-macro-current/raw/apps.razer.com/synapse/macro/static/js/main.f7cc2d30.js)、录制分块与 acquisition／[静态提取收据](../../../.analysis/results/frontend/synapse-macro-current/analysis.json) 的 bytes／SHA-256，及主机 ASAR 的实际字节与 [逐项提取收据](../../../.analysis/results/host-current/asar/b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6/capture.json)。UTF-8 范围仍为半开区间。

| 所有者 | 原件／完整函数定位 |
| --- | --- |
| XML 输入、导出菜单、保存桥接 | 宏 main：`[638110,639544)`、`[640454,641077)`、`[227804,228093)`；[主进程 main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js) `showFileSaveDialog` case `[51081,51562)` |
| V4／旧格式转换 | 宏 main：V4 动作 `[611991,614773)`、导入对象 `[614913,616122)`；旧 DOM `[625178,625831)`、formatter `[625831,628550)` |
| IDB、初始化、投影和写入 | 宏 main：IDB／频道 `[550745,552565)`；迁移／读取 `[667805,670402)`；`Js/qs` `[738474,739489)`；完整 reducer `[739505,756173)`；MacroLocker `[91215,91850)` |
| 原生适配器及 map 写入 | 宏 main 前端适配 `[274038,274273)`；[mapping_engine/win/index.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/mapping_engine/win/index.js) `[63388,64154)`；[原生捕获](../../../.analysis/results/native/6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a/capture.json) 中 `0x20920、0x1e454、0x1000、0x1060、0x2fa0、0x3160、0xd46ec、0xd4900` |
| 通知、完成及数据分支 | 同原生捕获完整伪代码／汇编：`0xd539c、0xd52d2、0xd570c、0x20680、0x1fb30、0x1fdd0、0x519fc、0xf68c0、0x107a24、0x5cf60、0x5ce40` |
| 普通宏观察、候选解析与快照交换 | 同捕获：`0x562c8、0x56850、0x5eb10、0x5cb50、0x5cc20、0x5ccb0、0xf6a70、0xf6bd0、0xf7106、0xf86ae、0x1b3a30、0x1f3600、0x6237c、0xf6f60、0xf7066`；观察／消费者／模式表 `0x2e4c88、0x2ecbe8、0x2e4da8、0x2e4dc8、0x2ecd20` |
| 改动通知、活动取消、退订与关闭 | `0x5e710、0x5e4e0、0x5e600、0xb1640、0xb0ab0、0x56b40、0x571e0、0x57090、0x56f10、0x56e70、0xf69c6、0x107b2a、0x1b8d40、0x1b8590、0x1a9710、0xd3c70、0xd3d1e` |
| OTFM 全表、枚举与参与对象 | [后台 OTFM 原件](../../../.analysis/inputs/frontend/background-manager-current/raw/apps.razer.com/background-manager/assets/otfm-d56d4f50.js)：`Wt [119904,146443)`、`bo [152052,153437)`、`Ao [153438,160935)` |
| OTFM 状态、输入、保存与恢复 | 同原件：`Ct [106327,119495)`、`ho/mo [148514,149844)`、`wo [149925,150848)`、`Io [151075,151235)`、`Ro [161207,166522)`、`$o [166585,172266)` |

播放／OS 输入扩展另核对了宏 main 与录制分块的 acquisition 字节长度／SHA-256，以下前端区间都重新解析为完整 AST 所有者；未执行脚本。native 函数 JSON 对照 capture 所记 SHA-256，各 chunk 对照原件 PE 字节与自身 SHA-256，逐条 assembly.bytes 也对实际 PE；虚表槽、坐标模式映射及最终状态的栈实参直接复核原始字节／汇编，未仅依靠伪代码恢复的函数原型。

| 所有者 | 原件／完整函数定位 |
| --- | --- |
| 动作转换器及真实导出链 | 宏 main：模块 13139 `[68786,90653)`、导出对象 `[68923,69495)`、键码 `f [70816,71371)`、动作 `m [71374,74936)`；wrapper `[76620,76769)`、generateAppEngine `[87791,88097)`；保存调用 `Ks [735253,735344)`、普通／三阶段投影 `[735465,736177)` |
| 绑定方式与编辑器宏类型 | 宏 main：运行绑定 `[306411,306933)`；编辑器类型表 `[465282,465391)`、类型 reducer `[148799,149809)`、三阶段重建 `[154015,155136)`；PID3946 编译器与设备触发见接收器／底座文档 |
| Parser 与模式上下文 | 同 mapping_engine 原生捕获：`0xed8d2、0xef470、0xf2f76、0xecee3、0xf36f2、0xedce6、0xdb930、0xdb873、0x1b4940、0x101ff0、0x102eb0` |
| 普通、队列、逐次触发 | `0x893dc、0xa9500、0x5f086、0x5eeec、0x127892、0x128692、0x12b5b0、0x89968、0x8edfc、0x125da8、0x8f0e4、0x8f5e0` |
| 分阶段数据与执行 | `0x107a24、0x107bd0、0x107d30、0x1080b2、0x107b2a、0x5ce40、0xf7066、0x8d1b8、0x129998、0x12ab8e` |
| 键盘／鼠标参数及重复 | `0x10bc0d、0x10bfed、0x10d180、0x10dd90、0x10e270、0x10dfd0、0x10e030、0x10e2f0、0x12f292、0x12f312、0x12f3ce、0x12fd66、0x12fe22、0x12ff70、0x12fb30` |
| DriverImpl 与系统输入 | `0x85ea8、0x10f582、0x1186c0、0x112c60、0x114850、0x110b5a、0x115da0、0x118840、0xa3910、0x11c510、0x11cbd3、0x1219dc` |
| 直接 endpoint 与坐标 | `0x134c80、0x135880、0x13579a、0x135c20、0x134af0、0x134b64、0xedada、0x130dc6、0x1314d0`；表 `0x2f2e70` 与 `[1,1,3]` 映射 `0x2f28e0` |
| 分派结果、停止与释放 | `0x12be6e、0x12c8f0、0x12ca5c、0x12a8ca、0x10d340、0x10e110、0x12772a、0x57090、0x57cd4、0x57c64、0x5d770、0x5d890、0x5dc90、0x5de50` |
| Clipboard parser、排队与暂存 | `0xed512、0xfd33a、0xfd72c、0x1295ca、0xb9f3c、0xbb6b6、0xbe4b0、0xbe646、0xbbc50、0xbc6de、0xbc970、0xbd2f0、0xbd980、0xbd9d0、0xbda00、0xbda90、0xbdb50`；表 `0x2edcf0` |
| Clipboard Windows 写入与粘贴输入 | `0xbd090、0x1dc768、0x1dc35e、0x1ddbae、0x22e640、0x22c704、0x22c85a、0x22e730、0x22e7ad、0x22beaa、0x22e830`；`0x8dc00、0xb74fc、0xb82e0、0xb8490、0xb91a0、0xb9560、0xb9800、0x11c460、0x11b849、0x11be52、0x11bc70`；表 `0x33ce70 / 0x2e8e50` |
| Clipboard 完成、恢复与释放 | `0xb90e0、0xbc390、0xbc400、0xbc5e0、0xbd190、0xbcdd0、0x1b9fa0、0xba770、0xba910、0xbc2e0、0xb7850、0xb79e0、0xba1b8` |
| Launch parser、OS 创建与句柄 | `0x10be63、0x1297d8、0x11ae38、0x1b1430、0x1b2dd0、0x1b1720、0x1abf10、0x1aba20、0xcf650、0x1a7cf0、0x1ef7e0、0x1efa60、0x1ef8e0、0x1efc70、0x1ef9c0`；Shell verb UTF-16 字节 `0x2f112a` |
| Audio parser、线程与 COM setter | `0xed10b、0xfcab2、0x128e76、0xb215c、0xb2994、0xb29f8、0xb2ca0、0xb3030、0xb33d0、0xb3760、0xb3af0、0xb3e80、0xb4220、0xb45b0、0xb4a42、0x62510`；服务 `0xb4b2c、0xb4d40、0xb4e80、0xb4fd0、0xb5110、0xb5250、0xb5390、0xb54e0、0xb5620`；表 `0x2edb50 / 0x2e8aa0`、float 100.0 字节 `0x2e8a98` |
| Audio monitor、通知与释放 | `0xb5788、0xb59c0、0xb5cd2、0xb6230、0xb6280、0xb62f0、0xb62c0、0xb6320、0xb61d0、0xb0c20、0xb0ef0、0xb6220、0xb598e、0xb5e9a、0xb4bfc`；表 `0x2e8b40`、IAudioEndpointVolume IID 字节 `0x2e8d00` |
| Audio 服务重建、观察者与全局停止 | `0x19a40、0xb2164、0xb24b8、0xb2640、0xb4c70、0xb6210、0xb63f0、0xb6100、0xb10d0、0xb1150、0xb11d0、0xb1480、0xb0cd0、0xb1286、0xb1640、0xb0ab0、0x17ee0、0x18010、0x18130、0x182a0、0x1d38e、0x1d190、0x1d1a4、0x1a4c0、0xb2986、0xb2732、0xb28c0、0xb4cf0、0x1a8e0`；表 `0x2e8c10 / 0x2df430` |
| 宿主 Audio 订阅与四事件消费者 | 上述主机音频适配器：register `[68943,70544)`、unregister `[70545,71686)`；四回调 `[96754,97235)`、`[97236,97729)`、`[97730,98378)`、`[98379,99042)` |

`python -X utf8 tools/workspace.py audit-host-local --query macro-recorder` 当前复核3,778个完整source owner，包含114个当前producer及其main引用、25／37差异和37项input／guard／task链，以及本页宏页／后台／宿主、映射／快捷键暂停和选定原生依赖；failures为空、snapshot_stable为true。两份逐产品结果各有重叠，不能把2,160与2,387简单相加作为总owner数。39项维护工具测试通过，结果用于核验证据身份、AST、投影与捕获边界。

以上是静态语义核对，没有执行厂商JavaScript或实际DLL。动作转换、XML样本、宏绑定执行及各产品OTFM适配仍需功能自动化测试和当前官方实际DLL对照；维护工具测试不等于这些功能测试。录制、文件分支、map发布或部分输入链的局部闭合不能代表全部宏执行和设备验收完成。
