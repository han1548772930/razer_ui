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

## OTFM 与软件录制的差别

PID756 的设备事件入口先写一个对象形式的共享窗口记录，字段位于顶层，含模式、设备和 container 身份，没有 `mode:"exclusive"`；随后经全局广播发布 `start-otfm`。

后台管理器接到事件后，先写另一份字符串化 exclusive 记录，再请求名为 `RzGlobalLock_macro_recorder`、带 `steal:true` 的锁。之后注册录制／系统监听、枚举本机设备、选择鼠标移动模式、读取亮度并发送 OTFM 按下事件。真正开始原生录制以前，它还要检查已有录制、暂停映射及全局快捷键，并读取显示器信息。

OTFM 状态机保留按键保存、Escape、系统事件、拔出和超时等转换，并分别处理录制停止、灯光恢复、映射恢复和完成通知。共享窗口记录的移除又受后台待保存动作与完成标志控制，不能用一个“录制结束”回调概括。

初始共享状态扫描读取顶层 `macro_recorder_mode`；软件生产者和实时事件消费者读取 `payload.macro_recorder_mode`。这是当前源码两条路径的实际字段差异。其消费者对于移除／清空事件会重新查询其余 owner，决定是否仍有 exclusive 持有者。

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

完成包装器 `0x20680` 还要求引擎初始化标记仍为真才调用外部回调；初始化完成设置该标记，shutdown 完成清除。页面正在关闭和原生停止之间的迟到结果不能假定一定会回到 JavaScript。普通 `synapseMacros` 与 `synapsePhasedMacros` 有各自的数据服务／查找入口；观察者解析、嵌套与分阶段动作调度、设备消费者及最终输入／设备应答必须继续沿各自链核对，不能从 map ACK 推断它们已经成功。

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

本页直接复核了 [本地完整捕获](../../../.analysis/results/host-current/local-audit.json) `macro_recorder.complete_source_owners` 的 293 个函数／所有者区间及 14 个原件的实际字节；语义说明按这些完整所有者而写。

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

以上是静态语义核对，没有执行厂商 JavaScript 或实际 DLL。动作转换、XML 样本、宏绑定执行及各产品 OTFM 适配仍需自动化测试和当前官方实际 DLL 对照；录制、文件分支或 map 发布的局部闭合不能代表全部宏执行和设备验收完成。
