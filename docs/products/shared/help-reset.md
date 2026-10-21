# 帮助、设备信息与恢复操作

帮助页同时承载设备身份、固件与软件版本、操作教程、支持链接和恢复操作。它不是一个统一的“恢复默认”接口：不同产品传入的 `resetObm`、连接方式、OLED 能力、音频重启和固件恢复参数会选择不同路径。

各产品实际挂载的 Help 组件、调用参数与完整组件字节位置见各族 `products/<PID>.md`。下列跨产品行为只适用于列出的原件；组件中存在某个条件分支，不表示该型号启用了该分支。

## 设备身份与版本从哪里来

PID 70、679、563、3594 和 2684 的当前通用 Help 组件在挂载时分别读取 UI、MW 与应用版本。UI 版本来自 `noscript#version` 的 JSON，并组合 `version.buildVersion`；保留 `VERSION_PLACEHOLDER` 的内容不作为有效版本。MW 版本来自宿主窗口存储；2684 按 URL 中的 `containerId` 及产品／dongle ID 选择记录，3594 的当前代码直接取返回数组第一项，二者不能混写为同一种关联规则。

应用版本来自本机 `apps` 记录中 `name === "synapse"` 的项，显示时把第一段替换为 `4`。固件、BIOS、EC 与 VBIOS 值则由组件 props 提供，不来自上述软件版本。更多信息按钮仅改变组件 `viewMore`，不发起固件查询。

通用组件将传入序列号保存为组件初值；复制调用 `navigator.clipboard.writeText`，立即显示已复制状态，2 秒后清除。原件没有等待剪贴板 Promise 的成功应答。带 Camo 分支时，序列号校验、重试请求和授权码属于另一条带账号／联网依赖的业务链；不能把它当作所有产品的序列号读取过程。

## 恢复确认弹窗实际触发什么

通用组件先打开确认框；取消关闭弹窗并恢复按钮，确认关闭弹窗、禁用按钮，再执行 `resetDevice`。PID 70／679／563／3594 使用 `resetObm && !isBle` 选择调用方传入的恢复函数，否则向 MW 发送 `ON_RESET_DEVICE`。PID 2684 额外判断产品类别：鼠标允许在相应 BLE 条件下使用 `resetObm`。这属于保留组件版本的真实差异。

`oledfactoryReset` 成立时，组件另外发送 `ON_OLED_FACTORY_RESET_DEVICE`。部分 OLED 分支的 `confirmResetOled` 再把调用方的 `oledResetInitialState` 转为 `ON_RESET_OLED` payload：

| UI 字段 | 提交字段与转换 |
| --- | --- |
| emote 图片 | `imageData = emote.src` |
| banner 图片开关／位置 | `imageState = enabled ? 1 : 0`；`imagePos = position === "top" ? 0 : 1` |
| banner 图片 | 保留 `imageData`，`imageIndex = 0` |
| media 信息／频谱开关 | 转成 `mediaState`／`visualizerState` 的 0、1 |
| system 幻灯、温度、时间 | `timeBetweenSlides → interval`、`temperatureUnit → tempFormat`，同时携带日期、时间格式和 slides |

这里的初始化对象由该产品调用方提供；不能从组件的字段转换推断所有产品使用同一套内容。`isNonSupportFactoryReset`、`isSupportResetOLED`、`isBle`、`firmwareResetProps` 与产品类别继续决定显示普通恢复面板、固件说明面板还是 OLED 确认内容。

通用 Help 在发出恢复动作后用 2 秒定时器恢复按钮。PID 1342 的音频 Help 使用独立 `resetAudio`：先设置 `isRestart`，1 秒后调用 props 的音频恢复函数，并清除该标志；它的设备恢复只调用传入的 `resetObm`，按钮也用 1 秒定时器恢复。**这些 UI 定时器表示界面等待时长，不能解释成设备应答或读回确认。** 实际恢复范围继续由对应产品 MW、任务和原生消费者决定。

PID 756 是另一条明确分支。当前 `Tt` 两个实际调用方都设置 `waitForObmResetCompletion`、`showFactoryResetOverlay` 和 `useResetObmForBle`；一个传入 `resetObm`，另一个不传且设置 `disableFactoryResetForBle`。恢复最多等待 4 秒；等待期间 `initObmCompleted` 或 `profiles` 的引用改变会提前解除按钮与遮罩。遮罩中 `OK` 只设置 `isFactoryResetAcknowledged`，不会产生恢复成功结果，也不直接结束恢复。未启用这些 props 的通用 Help 不能套用这套行为。

## 691：Help 到 OLED，再到设备 OBM

691 的 Help 实际挂载 `HelpComponent` 模块 88934，父页面把 `ps.resetDevice` 传为 `resetObm`。非 BLE 下确认调用该回调，再调用 `resetOled(true)`；键盘 OLED 确认使用 `confirmDel`，**不走同组件保留的 `confirmResetOled` 通用字段转换**。BLE 下该 Help 组件直接发送 `ON_RESET_DEVICE`：其 MW 状态项只授予 `updateLocalStorage(shared)`，没有 `deviceAccess`；`resetDevice` consumer 进入同一 OBM maker，但此处不排物理 reset 任务，只执行本机默认档案追加、active／version／metadata 保存与发布。随后普通档案观察者的同步须继续按各设置消费者判断；不能把父回调中保留的 BLE 分支或 OLED runner 的 BLE 分支当作该 Help 消息已执行的动作。

非 BLE 的真实顺序如下；它既包含 OLED 内容恢复，也包含设备内档案、宏和按键的恢复：

1. `is` thunk 先调用 `8816.mk`，按该产品 OLED FPS 词表选 `face-tongue-animated`，等待 lazy 图片解析后才提交 `SET_OLED_RESET`。reducer 从当前默认 OLED 对象、已解析 emote 和当前 OLED language 构造 payload；简体中文为 `zh-CN`，其余此处用 `en` 生成 system label。随后发送 `ON_RESET_OLED`，保存 `isResetting=true` 和 `shouldResetOBM=true`。这条当前键盘链没有把 lazy 函数直接交给 BroadcastChannel。
2. MW 的 OLED 状态机进入 `resetOled`，task maker 将完整 payload 放入 `NORMAL` OLED 队列。runner 按实际 `runtimeData.useBle` 选择完整恢复 `88728.Fv` 或只切 animation 的 `88728.oP`；后者是 runner 的独立 BLE 分支，不能据此替代上述 Help BLE 消息的消费者追踪。
3. 完整恢复先读 animation、image 各槽 GUID，仅对非原生槽准备恢复数据；另比较 emote GUID、准备 banner。banner helper 先读 GUID，又把同一局部变量覆盖为 home-screen 参数 setter 的响应；正常 setter parser 不产出 `imageGUID`，GUID 转换函数对缺值返回空串，因此这里不能描述为“仍拿刚读出的 banner GUID 判断相同”。它继续按默认文字渲染并准备 banner 数据。
4. 在本机文档中更新 media/system 默认对象，启动 `reset` 下载进度，停止 `rzSystemMon`、`rzAudioMon`，选择 animation，然后顺序传输。animation 使用槽 `index+2`、image 使用自身 index，二者写全零原生 GUID；emote 使用 GIF 槽 1 与内容 GUID，banner 使用文字槽 0。成功槽才更新资源 IndexedDB／本机对应对象。该恢复函数没有重置所有标量、重写每个 enable 位或逐槽 GET 验证，不应扩张其恢复范围。
5. `finally` 先保存当前本机文档，再发布 `MW_SET_OLED_RESET_DONE_TO_UI`、active animation 和 animation／image／emote／banner／media／system 的各自当前数据。异常置 `isSuccess=false` 并重新抛出；`USER_CANCEL` 分支仅取消下载并跳出循环，没有统一把该标志改为 false。若 `finally` 中的保存本身失败，后续完成广播也无法视作已发送。
6. UI 的 reset reducer 收到上述完成 action 后，仅检查 `shouldResetOBM`，**不检查 payload 的 `isSuccess`**，就发 `ON_RESET_OBM` 并清 `isResetting`。因此 OLED 失败后的 OBM 动作确实属于当前源码行为；不得把完成 action、2 秒按钮计时或 `isSuccess=true` 单独解释成全部 OLED 槽已确认一致。
7. OBM maker 先发自己的 `completed` action，再取消已有任务并排入设备 OBM reset；本机档案处理保留既有档案，追加经主机名、唯一名称和新 GUID 形成的默认档案，更新 active profile、版本与设备 metadata 后保存。691 的 edition 0／128／130／131 当前共用默认档：brightness=50／enabled、Spectrum effect=3、有线／无线 polling=1000、gaming state=0 且禁用键标志均 false、Snap Tap=false、mappings=[] 和产品默认 command-dial modes。这并不同时重置设备级 OLED／dim／power-saving 对象。
8. 保存调用的 `10745.IF` 不是仅更新 UI：它对新 active profile 发布列表／设置，并分别排入 `ON_INIT_BRIGHTNESS`、`ON_SET_EFFECTS`、`ON_SET_GAME_MODE`、`ON_SET_KEYMAPPING`、command-dial mode／mapping、`ON_SET_SNAP_TAP` 等设置消费者，最后调用 feature manager 的 `restoreDeviceData`。polling 分支在 BLE 明确跳过；quick effects 按 BLE 硬件支持表选择。后续仍走各能力自己的队列、协议和确认，不能把一次 profile 保存或 maker 日志当作这些设备写入全部完成。通用设置消费者见 [灯光与电源](../keyboard/lighting-power.md) 及 [按键映射](../keyboard/customize.md)。持久化函数 `89423.Pf` 自身还检查实际 serial；缺 serial 时只记录不保存，调用返回不等于已写入文档。
9. 当前产品明确选 `obmEngineKeyboardCalMaxMem`，继承 keyboard 的实际 reset 实现，且 `resetProfile=false`、`macros=true`。它删除第一个保留档案以外的设备档案、格式化宏存储，再比较 normal／hypershift 默认映射；有差异才调用 `setSingleKeyAssignment`。单键失败由原件逐键 catch／记录，并不自动使外层整次 reset 拒绝。最后重读 macro storage info，返回当前 OBM JSON，由 runner 更新 OBM memory、可用空间、`initCompleted` 并广播。

| 691 OBM 恢复中的设备命令 | 实际编码与确认 |
| --- | --- |
| 删除额外设备档案 | `[size=1,class=0x05,id=0x03]`，data[0] 为实际 profile ID |
| 格式化宏存储 | `[6,0x06,0x0a]`，macroId=0、action=`MemoryFormat=2`；随后等待 100 ms |
| 查询格式化状态 | 最多 3 次 `[6,0x06,0x8a]`，同时要求传输 `SUCCESS` 和返回 status=2 才提前返回；循环耗尽仍返回最后原始响应 |
| 查询宏存储信息 | `[14,0x06,0x8e]`，解析真实 maxId、maxMemory、availableMemory、recyclableMemory |
| OLED active animation／读取槽 GUID | `[2,0x17,0x0d]`／`[22,0x17,0x8f]`；内容分包、busy／错误应答及释放另见 [OLED 传输](../keyboard/oled.md) |

OBM runner 在调用 `resetDevice()` 前、采用返回数据后分别检查取消 signal；引擎内部删除／格式化／逐键恢复没有同等的逐步取消检查。恢复页面本身不提供 OLED reset Cancel，底层取消也不能撤销已写入的槽、删除或宏格式化。`resetDevice` 对 format 结果的处理还允许空对象，外层返回 OBM JSON 不保证所有先前命令成功。完整 OLED reset 停止两个监测，但本函数没有调用取消日期 interval 的 `x()`；不能把该 reset 的直接清理范围写成全部 timer 已释放。独立实现及测试须保留每一步真实结果，不能从最终 JSON、UI 解锁或空宏列表推断全部设备恢复成功。普通 OLED 内容编辑、取消／重试与移除释放继续见 [OLED 页面](../keyboard/oled.md)。

本项目目前在 [Help 分发](../../../crates/razer-shell/src/shell/receiver/receiver_reset.rs) 的 `oled_reset_obm` 分支明确返回“尚未实现”，没有把这条 691 OLED→OBM 链接到生产设备传输。上述静态闭合增加的是官方语义证据，不代表 Rust 恢复实现或真实设备／DLL 对照已通过。

## 教程与链接

教程按钮分别调用 `setShowInstructions(true)` 或 `changeAnalogTutorialStatus(true)`；不是设备恢复。固件更新提示向共享窗口频道发 `navigateDeviceAndModuleView`，由 Dashboard 接续导航；点击提示本身不执行固件刷写。

支持页、语言对应 PDF 和注册页属于原版在线入口。源组件通常使用 `masterGuide + lang + ".pdf"`，部分产品根据 edition 另选 PDF。Camo 分支还包括本机应用启动和在线授权码。它们在源码中实际存在，但账号、在线支持与远程更新编排位于本项目本地优先范围之外；本机设备信息、教程、恢复及本地文件动作要分别理解。

## About 的独立入口与版本

当前 `/synapse/settings/` 的 General 实际经 `uo` 挂载 About `Fs`；左列语言、右列 About。`Fs` 首次挂载读取 Dashboard manifest，组合 `4 + 原 version 后续段 + buildVersion`，catch 才读本机 `RazerApp.getAppData("synapse","version")`。这属于官方源码的取值顺序；本项目离线 About 从安装目录库读取已核实的 Dashboard 元数据，不在页面挂载时联网，也不把失败后的 `4.0.0` 当作已查询的安装版本。

稳定版 logo 来自模块 7795，beta 标记选 2304；图片居中、自然高 70 px、max-height=120 px、下距 20 px。版本与版权均为 14 px／17 px、`#999`、居中、下距 20 px；年份由本机日期生成。EULA、支持、条款、隐私、社交与订阅链接是原版实际存在的联网入口，按本项目范围删除，不保留禁用按钮或伪成功操作。本机 About 保留 logo、静态来源版本、版权和商标文案。

本项目存在两个实际呈现所有者：[产品设置的 About](../../../crates/razer-settings/src/preferences/settings_page.rs) 由 Shell 的 `product_settings_page(false)` 挂载，使用上述 Dashboard 目录元数据；[独立宿主设置窗口](../../../crates/razer-settings/src/window/settings_window.rs) 由 `open_settings_window` 挂载，使用另一份 `settings_window_data.version`。后者当前目录值的维护收据仅为旧 Rust 构造器投影，不是官方来源证明；当前 `/settings/` 的实际部署、General/About 组件及版本 producer 原件仍需取得。不能用 `/synapse/settings/` 的 `Fs` 或两个 Rust 页面外观相似就宣布独立宿主 About 已完成源码一致性验证。

## 系统信息的两个布局

当前完整读取链覆盖 PID 694、695、696、709、710、711、736、737。694／695／696 的调用方选择 basic；709／710／711／736／737 选择 image。包内保留两个组件不表示两个都挂载。

basic 展开后列出设备名、CPU、RAM、序列号、型号、OS、前两个 Display 名称及软件信息。image 上部展示设备图与 CPU／GPU／RAM，展开区遍历全部 Display，显示 OS 和软件信息；NVIDIA 项在两个候选软件字段都存在时优先取 `NVIDIANvApp`。各布局的空字符串及“未安装”展示文本是具体字段的原版呈现规则，不是整个系统查询失败后的替代结果。

MW 的能力分支调用 `getBladeDisplayData()`，解析返回 JSON、保存 `systemDetails` 并发布对应 UI action；reducer 把 payload 留在 `systemInfo`。读取异常的 catch 只记录错误。页面的展开动作仅切换显示状态。

原生报告返回 JSON 中的 `Display` 键可以是 `null`；每项的 `Name` 和原版拼写 `Manufactuer` 需按 producer 理解。显示查询、驱动查询、固件查询与全量保存是不同导出，不应拿显示页已取得的数据冒充完整报告。

## 保存完整系统报告

点击下载发送 `ON_STORE_SYSTEM_INFO_DATA`，进入状态机和 TaskMaker。当前 TaskMaker 使用 color-profile 命名的队列／版本结构；这是原件真实复用关系，不能按日志名字把它归为灯光设置。

执行器再次调用 `getAllBladeInfo()`，解析 JSON 后以两空格缩进保存。建议文件名为 `systemDetails.DeviceName + " - " + Date.now() + ".synapse4"`，对话框标题为 `Save as`，按钮为 `Save file`，过滤器为 `Synapse 4 (*.synapse4)`。

宿主按消息发送者定位 BrowserWindow，默认目录使用已记住的目录或系统 Downloads；取消不更新目录。取得路径后调用 `fs.writeFile`，写入错误只在回调中输出。这个宿主 case 没有把对话框及写盘回调完整 await 为设备任务结果，因此原版执行器的 SUCCESS 日志不能证明文件已写完。文档保留这条实际链，不补出源码没有的事务确认。

官方本地 wrapper 把原生字符串复制为 JS 字符串后通过 `FreeMalloc` 释放。`Terminate`、`TerminateWhenExit` 及 DllMain 是另一条原生生命周期链；已有捕获保存其函数与交叉引用，仍有间接和下层 helper 边未闭合，不能据此宣称整个 DLL 已理解完成。

## 本项目当前接线与仍待完成的边界

以下检查的是现行调用方和消费者，静态实现存在与实际设备／DLL 对照分别记录。

| 功能／所有者 | 当前 Rust 的真实路径与限制 |
| --- | --- |
| Help 身份、UI build、复制 | [SourceHelp](../../../crates/razer-pages/src/features/help/source_help.rs) 保留 mounted serial、复制任务和页面实体；[UI build](../../../crates/razer-pages/src/features/help/source_help_versions.rs) 读取安装目录库，不冒充运行时 MW 版本 |
| MW／应用版本 | [runtime_versions](../../../crates/razer-pages/src/features/help/source_help_runtime_versions.rs) 已有 owner／generation 校验和观察 API；全 workspace 调用核对未找到 Shell 请求消费者或生产 observation 构造，当前未接到真实 storage／本机版本 producer |
| 164／241 Help reset | [receiver_reset](../../../crates/razer-shell/src/shell/receiver/receiver_reset.rs) 进入真实 receiver profile 保存、设备操作及灯光恢复；不能泛称仅改草稿，产品链见 [接收器与底座](../accessories/receiver-and-dock.md) |
| 1398 firmware／hardware reset | [help_hardware_reset](../../../crates/razer-shell/src/shell/receiver/help_hardware_reset.rs) 保留旧档、生成新默认档，先进入持久化队列，保存后才采用并提交真实 `FactoryReset1398`；设备身份、连接代次、profile 和实际 acknowledged receipt 都要匹配。已保存默认档与硬件失败分别保留；换页／断开不采用旧回执 |
| 1342 audio restart | [restart](../../../crates/razer-shell/src/shell/audio/audio_mixer/restart.rs) 经独立服务解析真实有线设备，按原序列调用 Rust 驱动 IOCTL，记录每流返回值并完成进程清理。非零原生返回值仍继续后续流；只确认 IOCTL 序列，不声称音频已经恢复，不替换 profile。它与右列 factory reset 是不同动作 |
| 691、其他 OLED／OBM／factory 与 ADD_PROFILE | 上述已解析的官方链尚未全部迁入 Shell；OLED、非 1398 hardware、default-profile adapter 及其他产品的恢复分发仍明确返回未实现错误。1342 的 factory reset 也不能借 audio restart 视为已接通 |
| 8 款系统信息与报告 | [help_system_info worker](../../../crates/razer-shell/src/shell/host/help_system_info.rs) 已从页面真实挂载发查询，通过 [独立 Windows producer](../../../crates/razer-service/src/native/help_system_info/windows.rs) 访问 WMI、DXGI、显示／设备枚举与注册表，未加载官方业务 DLL。完整报告重新查询，后台文件对话框、取消、原子写入结果与清理分别处理；不像官方未 await 的 case 那样把发出保存请求视为写盘完成 |
| 系统信息的输出差异 | 官方 display producer 把 NVIDIA 临时值复制到 Intel 字段，本项目保留实际 Intel 查询值；缺失注册表、EC、摄像头及驱动值保持未观测。此为显式差异，不能标为 DLL 字节完全一致 |
| 失败在页面中的呈现 | `SourceHelp.reset_error`、系统信息 `State.error` 已存储；当前 render 未读取这些错误，系统信息缺值仍以原版空字段布局呈现。Shell 诊断不能代替页面“未读取／读取中／失败／断开”的完整区分，该 UI 失败链仍未完成 |

当前卡片内确认由 [inline popup](../../../crates/razer-pages/src/features/help/source_help_inline.rs) 持有：300 px、20 px padding、Roboto 14／17、红色边框和单个确认按钮，普通按钮下方、OLED／firmware 使用各自位置；点外部关闭，300 ms 透明度动画。`legacy-delete`／`plain-delete`／`normal-remove`／`mixer-remove` 的 Delete、Remove、Reset 文案分别保留。不能统一替换成一个屏幕中央的系统 Dialog；目前非 inline 的通用 Dialog 分支仍只允许 164／241 提交。

[页面离开与设备切换](../../../crates/razer-pages/src/features/help/source_help.rs) 取消 copy、popup、audio timers，递增 confirmation／request generation；[756 遮罩](../../../crates/razer-pages/src/features/help/source_help_reset.rs) 阻止后层输入，OK 不产生设备 receipt。Shell 对已接受的工作保留取消标志与 worker，退出时回收。取消仅结束本次请求／拒绝旧代次结果，不宣称撤销已经写入设备或已经持久化的档案。

范围内每条分支仍须按 [验证规则](../../../.agents/rules/verification.md) 做独立期望、模拟传输与当前官方实际 DLL 结果对照。现有 [1398 reset 测试](../../../crates/razer-shell/src/shell/receiver/help_hardware_reset_tests.rs)、[1342 restart 测试](../../../crates/razer-shell/src/shell/audio/audio_mixer/restart_tests.rs)、[系统报告测试](../../../crates/razer-shell/src/shell/host/help_system_info_tests.rs) 和 [Help 身份测试](../../../crates/razer-pages/src/features/help/source_help_reset_identity_tests.rs) 的存在不代表全部产品覆盖或本轮已执行。本轮只做文档与原件核对，未执行 DLL、服务／设备动作或运行时验收；实际对照保持未验证。

## 证据入口与阅读边界

| 已直接核对的所有者 | 当前原件与 UTF-8 字节区间 |
| --- | --- |
| 通用 Help，PID 70 | [main.8f24b6a1.js](../../../.analysis/inputs/frontend/mouse-70-current/raw/apps.razer.com/synapse/products/70/ui/static/js/main.8f24b6a1.js)，`[5667140,5686347)` |
| 通用 Help，PID 679 | [main.194d8a7b.js](../../../.analysis/inputs/frontend/keyboard-679-current/raw/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js)，`[7728572,7747842)` |
| 音频 Help，PID 1342 | [main.2623357c.js](../../../.analysis/inputs/frontend/audio-1342-current/raw/apps.razer.com/synapse/products/1342/ui/static/js/main.2623357c.js)，`[7311062,7315670)` |
| 756 等待、告警与实际父 props | [3021.0f2a9d4a.chunk.js](../../../.analysis/inputs/frontend/keyboard-756-current/raw/apps.razer.com/synapse/products/756/ui/static/js/3021.0f2a9d4a.chunk.js)，完整 `Tt` `[124862,144723)`，props `[338949,339053)`／`[339465,339572)`；遮罩样式为 [main.79343ec3.css](../../../.analysis/inputs/frontend/keyboard-756-current/raw/apps.razer.com/synapse/products/756/ui/static/css/main.79343ec3.css) |
| About、实际 General 挂载及稳定版图标 | [720.1e5d1c8f.chunk.js](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/js/720.1e5d1c8f.chunk.js)，完整 `Fs` `[77038,80062)`、`uo` `[215413,215565)`；[对应 CSS](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/css/720.dbc9cca5.chunk.css) 与 [原版 logo](../../../.analysis/inputs/frontend/synapse-settings-current/raw/apps.razer.com/synapse/settings/static/media/razer_synapse_4-1.56e5c81a.svg) |
| 系统 basic／image 组件 | [main.f0afa1aa.js](../../../.analysis/inputs/frontend/system-694-current/raw/apps.razer.com/synapse/products/694/ui/static/js/main.f0afa1aa.js)，`[8459274,8461357)`／`[8470784,8473047)` |
| 系统 UI reducer | 同一原件，`[7832194,7832902)` |
| 完整报告 TaskMaker／执行器 | [6737.43877d4945b570a8510a.js](../../../.analysis/inputs/frontend/system-mw-694-current/raw/apps.razer.com/synapse/products/694/mw/6737.43877d4945b570a8510a.js)，`[939263,939979)`／`[1506934,1508211)` |
| 宿主保存 case | [electron/main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js)，`[51081,51562)` |

系统信息全部 8 个产品的 UI、MW、native wrapper、原生函数及版本对应关系保存在 [local-audit.json](../../../.analysis/results/host-current/local-audit.json) 的 `help_system_info`。该报告同时含本项目实现说明，不能把那些说明当作官方行为。本文仅采用经原件核对的源码部分。

691 的新增结论使用以下完整所有者；UTF-8 字节区间均在当前原件中重新核对，未执行厂商代码：

| 691 原件 | 完整所有者与用途 |
| --- | --- |
| UI [7668.5b809e87.chunk.js](../../../.analysis/inputs/frontend/keyboard-691-current/raw/apps.razer.com/synapse/products/691/ui/static/js/7668.5b809e87.chunk.js) | 实际父页 `ps` `[150871,161474)`；解析 emote 的 thunk `is` `[150336,150566)` |
| UI [HelpComponent.ad246184.chunk.js](../../../.analysis/inputs/frontend/keyboard-691-current/raw/apps.razer.com/synapse/products/691/ui/static/js/HelpComponent.ad246184.chunk.js) | `Ie` `[25197,43583)`，含确认、BLE 路由、按钮计时及实际 OLED popup；对应卡片样式 [4203 CSS](../../../.analysis/inputs/frontend/keyboard-691-current/raw/apps.razer.com/synapse/products/691/ui/static/css/4203.77307df6.chunk.css) |
| UI [main.c20bafd3.js](../../../.analysis/inputs/frontend/keyboard-691-current/raw/apps.razer.com/synapse/products/691/ui/static/js/main.c20bafd3.js) | `oledResetReducer` `[805175,806922)`；默认 emote 模块 8816 `[15333,35077)`，其解析函数 `[34831,34972)` |
| MW [6737.b169abe06a428339f90f.js](../../../.analysis/inputs/frontend/keyboard-mw-691-current/raw/apps.razer.com/synapse/products/691/mw/6737.b169abe06a428339f90f.js) | 完整内容／reset 模块 88728 `[1272999,1297203)`；完整／BLE reset 函数 `[1293673,1297202)`／`[1293356,1293672)`；reset maker `[1331313,1331493)`、runner `[1532622,1532893)` |
| 同 MW 原件：OBM、结果消费者与保存 | reset-device consumer `[1155710,1155997)`；OBM maker `[1429888,1431586)`、runner `[1614659,1616197)`；档案设置重放 `vo` `[1230387,1249568)`；持久化 `M` `[1845888,1848307)`；metadata `I` `[1844070,1844557)` |
| MW [main.58e63fd267c2f91d3ebe.js](../../../.analysis/inputs/frontend/keyboard-mw-691-current/raw/apps.razer.com/synapse/products/691/mw/main.58e63fd267c2f91d3ebe.js) | 691 默认 profile `L` `[149430,149928)`、edition 表 `G` `[151736,151761)`、构造 `v` `[151762,151903)`；产品 feature 安装选择 actual `obmEngineKeyboardCalMaxMem` |
| MW [obmEngineKeyboard.266ed0f666298a631c2f.js](../../../.analysis/inputs/frontend/keyboard-mw-691-current/raw/apps.razer.com/synapse/products/691/mw/obmEngineKeyboard.266ed0f666298a631c2f.js) | keyboard 实际 `resetDevice` `[101087,104854)`；继承的 `formatDevice` `[41078,41784)`；单键写入 `[91336,93359)`；选用子类 [CalMaxMem](../../../.analysis/inputs/frontend/keyboard-mw-691-current/raw/apps.razer.com/synapse/products/691/mw/obmEngineKeyboardCalMaxMem.44803907e879eb32e63c.js) |
| MW [rzDevice25.0cfe1013577a467100b3.js](../../../.analysis/inputs/frontend/keyboard-mw-691-current/raw/apps.razer.com/synapse/products/691/mw/rzDevice25.0cfe1013577a467100b3.js) | `FormatStorage` `[167806,168198)`；delete `[170783,171101)`；macro-info `[163306,163846)`；active-item `[246626,247009)`；GUID `[247824,248373)`；参数 setter `[239206,240611)` 及 parser `oa` `[107997,109710)` |

本次复用现有 acquisition 与完整 capture：所用 691 原件的实际长度／SHA-256 与两份记录一致；两层 capture 各有失败阶段，未将其改写为整套成功。系统信息报告中的 262 个完整源码所有者及 1398 [恢复审计](../../../.analysis/results/frontend/audio-help-reset-audit.json) 的 24 个所有者也逐一核对了完整切片、实际 SHA 与 AST 边界，无差异；这些数量只是本次重核范围，不代表整个 DLL、所有恢复或所有产品已闭合。源图缺失、未决原生间接调用、独立宿主 About 输入缺失和各产品设备链继续分别保留。

全产品恢复的具体字段、设备命令和恢复后事件仍需逐产品接续；本文件不把通用 Help 按钮闭合等同于全产品恢复协议闭合。本次仅静态阅读，未执行页面、DLL、文件对话框或恢复写入。
