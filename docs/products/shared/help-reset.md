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

本项目存在两个实际呈现所有者：[产品设置的 About](../../../crates/razer-settings/src/preferences/settings_page.rs) 由 Shell 的 `product_settings_page(false)` 挂载，使用上述 Dashboard 目录元数据；[独立宿主设置窗口](../../../crates/razer-settings/src/window/settings_window.rs) 由 `open_settings_window` 挂载，使用另一份 `settings_window_data.version`。独立 `/settings/` 当前原件已补齐，真实 General/About 及完整 producer 见[全局设置](../host/settings.md#独立-about-的真实版本-producer)：它先读托盘 manifest 的缓存／网络数据，异常才取本机 `chroma-app` 的版本字段；与 `/synapse/settings/` 的值源不同。Rust 静态目录值仍未据此完成真实观察接线，两个页面外观相似也不能证明版本生命周期一致。

## 系统信息的两个布局

当前完整读取链覆盖 PID 694、695、696、709、710、711、736、737。694／695／696 的调用方选择 basic；709／710／711／736／737 选择 image。包内保留两个组件不表示两个都挂载。

basic 展开后列出设备名、CPU、RAM、序列号、型号、OS、前两个 Display 名称及软件信息。image 上部展示设备图与 CPU／GPU／RAM，展开区遍历全部 Display，显示 OS 和软件信息；NVIDIA 项在两个候选软件字段都存在时优先取 `NVIDIANvApp`。各布局的空字符串及“未安装”展示文本是具体字段的原版呈现规则，不是整个系统查询失败后的替代结果。

MW 的能力分支调用 `getBladeDisplayData()`，解析返回 JSON、保存 `systemDetails` 并发布对应 UI action；reducer 把 payload 留在 `systemInfo`。读取异常的 catch 只记录错误。页面的展开动作仅切换显示状态。

原生报告返回 JSON 中的 `Display` 键可以是 `null`；每项的 `Name` 和原版拼写 `Manufactuer` 需按 producer 理解。显示查询、驱动查询、固件查询与全量保存是不同导出，不应拿显示页已取得的数据冒充完整报告。

## 保存完整系统报告

点击下载发送 `ON_STORE_SYSTEM_INFO_DATA`，进入状态机和 TaskMaker。当前 TaskMaker 使用 color-profile 命名的队列／版本结构；这是原件真实复用关系，不能按日志名字把它归为灯光设置。

执行器再次调用 `getAllBladeInfo()`，解析 JSON 后以两空格缩进保存。建议文件名为 `systemDetails.DeviceName + " - " + Date.now() + ".synapse4"`，对话框标题为 `Save as`，按钮为 `Save file`，过滤器为 `Synapse 4 (*.synapse4)`。

宿主按消息发送者定位 BrowserWindow，默认目录使用已记住的目录或系统 Downloads；取消不更新目录。取得路径后调用 `fs.writeFile`，写入错误只在回调中输出。这个宿主 case 没有把对话框及写盘回调完整 await 为设备任务结果，因此原版执行器的 SUCCESS 日志不能证明文件已写完。文档保留这条实际链，不补出源码没有的事务确认。

当前 Blade2 wrapper 继承的 `callDLLApiAsync` 经公共模块的 `oE` 调用 `apiElectron.doDLLMainActionAsync`，再由 preload 的 `ffiPreloadAsync` 进入宿主 `FFIPreloadMain.callDLLAsync`；这条报告路径使用主进程 FFI。宿主等待实际 FFI callback，成功 `readCString` 后在 `finally` 调用 `FreeMalloc`。原版 callback 没有把 `err` 转成 Promise rejection；释放标志也在 `readCString` 成功后才设置，读取字符串抛错的分支不能认作已执行释放。

报告 runner 在原生查询、JSON 解析、发出保存请求之后才检查 `AbortController.signal.aborted`。`RzTaskManager` 的取消只清待办、abort 当前任务并把它移入 cancelling 列表；它没有等待原生调用结束或撤销文件写入。取消可以阻止尚在等待的 Web Lock，或使 runner 在检查点拒绝结果，不能解释为取消已经进入 DLL 的查询。694 的 `exitApp` 消费者调用停止观察循环与 `cleanUpDevice`，没有 await 后者，也未直接调用报告任务的 `cancelAllTasks`；不能由窗口退出推断报告自动取消。

## 普通销毁与宿主退出是两条调用链

8 款 wrapper 的 `destroy()` 仅 await `terminate()`；后者在 `isInit && supportTerminate` 时调用 `Terminate`、记录返回值，再清 `isInit`／`lib`，没有 `finally` 保证异常路径清理。四份实际 `Terminate` 都只写日志并返回 1，因此普通 destroy 本身不停止 CTR、不销毁通知窗口，也不卸载原生库。

`init()` 则在 `ConfigureFFI` 后设置 `isInit`，读版本，再注册 `ConfigureFFI_APIToCallWhenExit("TerminateWhenExit")`、设置原生事件 callback，最后设置 `supportTerminate`。它按 API 分组降级重试；第 4 组才含 `TerminateWhenExit`，降级成功时可能没有该导出。宿主注册只把名称存入 Map，不核对导出是否存在；wrapper 对注册结果也只写日志。因此已初始化、`supportTerminate=true` 或注册返回成功，都不能证明退出函数实际存在并执行。

宿主 `quitApp` 广播 `exitApp` 后固定等待 200 ms，没有收齐各 MW 清理 Promise。`quitAppFinal` 随后依序等待 subprocess 的退出动作、调用 main FFI 的设备退出／shutdown 动作，再 await `callApiWhenExit`。后者按注册 Map 逐项调用 `callDLL`；`callDLL` 与 `callDLLAsync` 使用同一个 channel Mutex，因此该 Blade 退出动作会排在同 channel 已进入／先排队的主 FFI 查询之后。这个串行关系不覆盖未 await 的文件写盘回调；设备退出／shutdown 的同步 `_callDLLNoAsync` 也不经过该 Mutex。其他服务的 3 秒 Promise timeout 不是 Blade 退出动作的超时；另有 force-exit 分支在调用 `quitAppFinal` 后再设 5 秒 `app.exit` 定时器，但 JS 定时器不能为同步 native 调用提供原生取消或有界回收保证。

`FreeFFI` 的两个 dispatch 分支直接返回 true，实际 `_handleAction_FreeFFI` 为空，未移除 FFI library／callback Map。`TerminateWhenExit` 卸载的是它自己加载的 CTR 模块；不能把它、`FreeMalloc` 或 `lib=undefined` 写成宿主已卸载 blade2Native／已排空全部 callback。

## 四份原生退出链的实际顺序

下表均为当前实际原件的 RVA，区间结束地址不含在内；Image Base 为 `0x180000000`。版本链接保留完整伪代码，函数块与原件 PE 字节已分别核对。

| blade2Native 版本／PID | `Terminate` | `TerminateWhenExit` | CTR stop helper | 通知 WndProc／线程 | 业务 DllMain |
| --- | --- | --- | --- | --- | --- |
| [1.0.45.0](../../../.analysis/results/native/a55c0305b4b683a82a13f3062b042420ba4d8540ec49807d0c3b319a2261a333/all-functions.c)，694 | `4a6d0..4a6ee` | `4f970..4fa0f` | `66f0..67d6` | `4a340..4a459`／`4a460..4a663` | `372b0..37487` |
| [1.0.46.0](../../../.analysis/results/native/8c0b60c7011d6b3841bee6ef9d37719720288222abc8b656a9e494f7bdb77f4f/all-functions.c)，695／709／710／736 | `4a9d0..4a9ee` | `4fc20..4fcbf` | `66f0..67d6` | `4a640..4a759`／`4a760..4a963` | `372b0..37487` |
| [1.0.43.0](../../../.analysis/results/native/25e02087a7667d1a1557eec141992fd1c3df2014c152d3a8069deeafec03e22e/all-functions.c)，696 | `51a50..51a6e` | `57260..572fe` | `6bc0..6ca6` | `516c0..517d9`／`517e0..519e3` | `3a1a0..3a3b1` |
| [1.0.50.0](../../../.analysis/results/native/6f6c875527ca8a147ef07a2737979a5fa4af1f11b99f5e26be5128bc77baca73/all-functions.c)，711／737 | `4d2b0..4d2ce` | `52480..5251f` | `6730..6816` | `4cf20..4d039`／`4d040..4d243` | `372f0..374c7` |

`TerminateWhenExit` 先向非空通知窗口同步 `SendMessageW(WM_QUIT=0x12)`，再调用 `_ColorProfileStopNew`，最后对非空 CTR module 调 `FreeLibrary`。它不消费 stop 的 bool；`FreeLibrary` 失败只写日志，仍把 module 全局变量清零，外层最后返回 1。`ColorProfileStop` 也始终返回 1；`_ColorProfileStopNew` 才将 helper 的 0 转为 true。1.0.43.0 额外捕获异常，前者异常仍为 true，后者为 false；这些 bool 都不能单独证明真实清理成功。

通知窗口的 WndProc 确实消费 `0x12` 并 `DestroyWindow`，再由 `WM_DESTROY` 调 `PostQuitMessage`；消息线程退出循环后清 `hWnd`、`UnregisterClassW`。同步发送返回只确认窗口过程已经处理，不确认线程末尾已完成。四版 `RegisterForNotifications` 保存线程 handle；上述退出导出没有 wait／join／`CloseHandle`，也没有清 Blade2 的显示事件 callback 或宿主 callback Map。1.0.43.0 的 Node 显示事件 callback 有异常捕获，其余三版没有同样的 catch；不能统一补成“所有 callback 已排空”。

stop helper 仅在 CTR module 存在且 API 表已初始化时工作：若 started 为真，先调用 `Stop`，再 `GetCTRStatus(&status)`，忽略两者返回码，只用 `status==3` 判定 helper 成功，否则置 `E_FAIL=0x80004005`；getter 的错误分支可以不写 out 参数，helper 却仍比较未初始化的局部值。随后调用 `SetEventHandler(nullptr)`、忽略返回、清 started。未加载或未 started 也可能得到 helper 的 0。两条 “Freeing Library／Free Library done” 日志之间没有 `FreeLibrary`，真正卸载只发生在前述退出导出。

四版动态 loader 已定位到真实名字：API 对象偏移 `+32=Stop`、`+40=GetCTRStatus`、`+56=SetEventHandler`；1.0.43／45／46 的 loader 为 `4ce0..4e25`，1.0.50 为 `4d20..4e65`。它逐个 `GetProcAddress` 后没有完整空指针核验便设 initialized；不应把未核对的 API 表当作有效功能状态。以下继续追到对应 CTR 实体，不能停留在“间接调用未解析”。

## CTR 的停止等待、释放与恢复边界

8 份当前产品 manifest 分别声明下列工具包；这里的版本是包版本，不能当作 CTR 的 PE 文件版本。包仅静态解包，未运行安装器。

| 包版本／PID | 实际 CTR 停止与主线程 | callback 处理 |
| --- | --- | --- |
| [1.0.0.5 获取／解包收据](../../../.analysis/results/native-packages/a9641484797285cd033ac9bac4b95e9e060d6bdf88920fa00c9d1fd28fa3e8e3/inspection.json)，694／696 | [完整函数](../../../.analysis/results/native/a94c2fc0630ed45c5f84d263e0d1a37009a52d98e207aecb61b4625639256ff4/all-functions.c)：`Stop 2fb60..2fbdb` 调 `SetEvent`，主线程 `30080..3023e` 等同一事件后清理 | `SetEventHandler 2fd50..2fd80` 为普通指针赋值 |
| [1.0.0.8 获取／解包收据](../../../.analysis/results/native-packages/03d4c4414184e0a8b76ce4ca33b5372a0bfa10b29ba897894a1ab354dc67a003/inspection.json)，695／709／710／711 | [完整函数](../../../.analysis/results/native/255dc26b0f2a5512d2ef96790de504633eb075f948f35d34517d9445e4cf032e/all-functions.c)：`Stop 2eed0..2ef5a` 向主窗口发 `WM_COMMAND(0x111),wParam=0x401`；`WndProc 2f400..2f492` 转 `PostQuitMessage(1)`，主线程 `2f4a0..2f6d2` 退出消息循环后清理 | `SetEventHandler 2f0d0..2f100` 用 `InterlockedExchange64`；主线程末尾也清该指针 |
| [1.0.0.9 获取／解包收据](../../../.analysis/results/native-packages/3bd1c893b0aeffff995c886b05f716647291bc5f1279fa8aa3fefe51ec33012d/inspection.json)，736／737 | [完整函数](../../../.analysis/results/native/63342f70552a9f8dc80f15e4a9205b9e55aeabc540ef0d20d26ecaa40415ef6e/all-functions.c)：上述 Stop／WndProc／主线程的 RVA 相同，已另核本版实际函数和字节，全局状态及部分 vtable 地址不同 | 同样用原子交换；仍不等于等待在途 callback 结束 |

三版 `Stop` 对 status=3 返回 -5、status<2 返回 -7；可停止分支发事件／消息后，每 1500 ms 轮询直到 status=3，**没有超时**，也不核对 `SetEvent`／`PostMessageW` 的 BOOL。.8／.9 在等到 3 后额外睡 50 ms，.5 没有这一段。因此 CTR 路径确有等待，不能因外层退出导出没有显式线程 wait 就说整个链“不等待”；事件／消息投递失败或清理未达状态 3 时也可能一直等待。

.8／.9 的主线程按非空对象分支依次执行 EventListener 析构、`Product.dispose`、Product 析构与 PDIThread 清理，再清 callback、`CoUninitialize`，最后统一发布状态 3；空指针会跳过相应对象分支。实际 Product vtable 分别为 RVA `1adea0`／`1ade90`，`+56` 都指向 `38800→23960`，`+0` 都指向 `38ee0`：前者保存 cookie／XML、清事件表与插件、注销热键并清工作线程；后者注销 WinEvent hooks 并释放 base 容器。.5 的 Product 表为 `1aeca0`，`+56→38d00→23ff0` 还先注销 WTS session notification；它的主线程在事件唤醒后，最多再用 100×100 ms 检查相应 display plugin 状态，再按非空对象分支 dispose／析构 Product、清全局工作线程并发布 3。这些发布顺序不等于已证明所有 OS 线程 handle、窗口和事件都已关闭。

工作线程清理 helper `41d0` 通过实际 PDIThread vtable 的 `+48` 传入 2000 ms，再析构对象：.5 为表 `1aba58→65c0..68a9`，.8／.9 为表 `1aaa28→64e0..6804`。目标函数提交 EmptyJob、等待队列操作；等待结果 status=5 会调用 `TerminateThread`，不是全部分支都正常排空后 join。后续析构会关闭对象的两个事件 handle、清队列，不能由此扩展为已经正常释放全部关联线程资源。

.8／.9 的插件释放 `26e90` 按实际类型选择 stop／dispose 再析构，并非统一“恢复原 ICC”。三版 `IccProfile_CTR` 对象 `+208` 的实际 vtable 分别为 `1aee68`／`1ae028`／`1adfe0`，其 `+48` 都指到仅 return 的 `4150`；该目标本身没有设备恢复。.8／.9 `DisplaysManager_CTR` 对应表则为 `1adbd0`／`1add00`，`+48→351e0..352c5`，清内部工作线程、两个 buffer 与对象，再发布插件 status=3，函数中没有写回原 ICC。cookie／XML 持久化、该 stop 路径或总状态 3 均不能证明全部色彩、显示及设备设置已恢复；其余插件与运行时类型分支仍须逐项追踪。

MW 的 `cleanUpDevice` 本身确有按能力和 `persistentInHardware` 选择的风扇／性能／超频恢复、THX 与监测清理，不能反向归结成“退出没有恢复”。其中 `hasCustomColorProfile` 调用 `colorProfileStop()` 后把 fresh-install 标记存为 false，不检查 native bool；该本机标记不是设备恢复确认。其他恢复域继续按对应产品链核对，不能由本文的 CTR 停止推断全部恢复完成。

四版业务 DllMain 的 process attach 读取版本、初始化本机日志并保存 hInstance；process detach 只记日志，未调用 `TerminateWhenExit`。CTR 的业务 DllMain 也只在 attach／detach 保存／清除 hModule。CRT dispatcher、onexit 与静态对象析构是另层链：已核对的路径释放日志、字符串和运行库状态，不能把这些析构名称当作 CTR stop、通知线程 join 或设备恢复的证明；本文未声称穷尽所有 CRT／插件虚调用。

## 色彩停止、ICC 安装与原默认配置的恢复

当前 694／695／696／709／710／711／736／737 的 MW 模块 `21705` 已逐份核对完整 AST、导出 getter 与词法绑定。`Hb` 是停止 owner，`Rv` 是启动 owner；`cleanUpDevice` 按 `hasCustomColorProfile` 等待 `Hb`，没有用它的结果判定设备恢复。`Hb` 等待 `colorProfileStop()` 后存 `FRESH_INSTALL_COLOR_PROFILE=false`，不检查返回值，异常只记日志。包装层未初始化时，stop 返回 `undefined`，该标记仍可写入；它只表示本机流程执行，不证明 ICC、校色或设备状态已经恢复。

启动 owner 默认从计数 0 开始，首次调用失败后最多再试 3 次，间隔为 1000／2000／4000 ms；它不向消费者返回最终 native bool，异常也只记日志。`Pn` 设置本机 `Blade10Tools/CTR.dll` 路径；初始化读取 fresh-install 标记及检查本地色彩文件后，按当前分支选择启动和停止。该标记不是“设备首次配置”的协议证据，不能据此泛化出设备默认值覆盖。`oC` 读取 DLL 版本第三段是否至少为 59 来设置 `CRTOptimized`；读取异常时的 false 也不是设备能力的实测值。

四份 Blade2 原件的新增入口如下；RVA 区间结束地址不含在内，原件与 Image Base 同前表。

| blade2Native 版本 | CTR 启动 helper | 直接设置 Windows 默认 ICC 的导出 |
| --- | --- | --- |
| 1.0.43.0 | `6720..6bb2` | `SetColorManagementDefaultICCProfile 508e0..50d5e` |
| 1.0.45.0 | `6310..66eb` | `SetColorManagementDefaultICCProfile 49870..49bc2` |
| 1.0.46.0 | `6310..66eb` | `SetColorManagementDefaultICCProfile 49b70..49ec2` |
| 1.0.50.0 | `6350..672b` | `SetColorManagementDefaultICCProfile 4c230..4c582` |

启动 helper 通过前表已核对的 CTR API 表注册 callback、调用 `Start`，再轮询 `GetCTRStatus`。后面的状态轮询最多额外等待 3×1000 ms，只对状态 4 判失败；耗尽次数仍为其他状态时可以继续设置 started。`CheckService==-1` 才在此阶段转失败，服务过旧只记日志。不能把这 3 秒说成整个启动的超时：CTR 的 `Start` 本身先创建主线程，随后可一直等待初始化状态离开 1。Blade2 的事件 callback 只记录 `259` 事件的 `-2`／`-11`，没有把它转换为 MW 的恢复确认；这组入口没有取消参数。

CTR 确有原默认 ICC 的备份与恢复实现，但其调用方是**批量安装预设**，不是上述 `Stop` slot。构造函数的真实 vtable 写入、完整函数和实际 PE 字节分别核对如下；.8 与 .9 同 RVA 的函数也各自核过原件。

| CTR 包版本 | 对象根表／`+360` 表 | 保存默认文件名／写回默认文件名 | 批量安装 owner／匹配显示器后安装 |
| --- | --- | --- | --- |
| 1.0.0.5 | `1aeea8`／`1aedb8` | `209c0..20c32`／`20c40..20e21` | `384d0..38a05`／`38a10..38b97` → `36b30..36d6d` |
| 1.0.0.8 | `1ae068`／`1adf78` | `1ff20..20192`／`201a0..20381` | `37fd0..38505`／`38510..38697` → `36630..3686d` |
| 1.0.0.9 | `1ae0b0`／`1adf88` | `1ff20..20192`／`201a0..20381` | `37fd0..38505`／`38510..38697` → `36630..3686d` |

批量 owner 的 `this` 是对象 `+360` 接口；它先通过对象根表 `+32` 保存各显示器当前用户默认 ICC 文件名至对象 `+336` 的 display-ID map，再遍历本机 `icc` 及 `icc/common` 的预设，最后通过根表 `+40` 写回保存的文件名。安装入口实际调用 `DisassociateColorProfileFromDeviceW`、`UninstallColorProfileW`、`InstallColorProfileW`、`AssociateColorProfileWithDeviceW`。这不是完整 Windows 色彩状态快照：保存的是当前用户、ICC 类型 0、subtype 5、profile ID 1 的默认文件名，没有备份原关联列表、配置文件内容或 per-user 开关；本次读不到的 map 项也没有在保存入口统一清空。

保存入口只检查 `WcsGetDefaultColorProfileSize`，没有检查随后 `WcsGetDefaultColorProfile` 的 BOOL，仍扫描输出并写入 map。写回入口对 `WcsSetDefaultColorProfile` 失败只记日志，没有汇总失败，也没有独立读回。批量 owner 不消费逐项安装结果与最终写回结果，正常尾部返回 1；可见恢复位于尾部顺序调用，不能据此声称异常、取消或所有安装失败分支都完成了补偿。该安装接口由 CTR 初始化的 `IIccProfile_RZS` 分支消费：.5 为 `305f0..3199a`，.8／.9 为 `2fa90..30e3a`。有可用 `native.bin`、需要生成 ICC 且 HDR 条件允许时才进入相应批量安装；既有 ICC 与其他产品分支不能套用这一初始化写入。

预设选择与手动 ICC 又是两种真实写入。CTR 的预设 setter（.5 `37ee0..383a7`，.8／.9 `379e0..37ea7`）通过显示器身份选择预设文件，必要时启用 per-user profiles，再调用 subtype 5 的 `WcsSetDefaultColorProfile`。外层 `SetDisplayMode` helper（.5 `33ad0..3422a`，.8／.9 `32f70..336ca`）在真正 WCS 写入前更新当前 mode 全局值；内层失败且错误不是 2 时仍可返回 0，错误 2 的补装分支也忽略再次安装与设置的结果。Blade2 将 CTR 的 0 映射为 true，故这个 true 不能单独证明系统 ICC 已切换。

四版直接 ICC 导出则将 UTF-8 文件名匹配至当前枚举表，再通过 Windows WCS 设置，使用 subtype **0**，与 CTR 的 subtype **5** 不同，不能合并比较。必要时它先调用 `WcsSetUsePerUserProfiles` 并改变 scope；末尾返回 WCS setter 的 BOOL，但仍没有读回或原值恢复。上述具体 ICC 操作直接调用 WCS；不能把 C++ 的 `RTDynamicCast` 当作 COM 接口查询。COM 生命周期在 CTR 主线程的 `CoInitializeEx(0,2)`／`CoUninitialize`，其初始化 HRESULT 没有被消费；线程、callback 与 stop 释放边界仍以前节为准，未增加“全部排空”的结论。

自动化对照必须分别记录这两种 subtype、scope、原默认文件名、per-user 状态、关联与安装状态，并比较 DLL 原返回、Rust 返回及独立 WCS 读回。模拟应覆盖读取失败、非文件缺失的设置失败、部分安装失败、尾部恢复失败、未初始化 wrapper 与停止后标记写入；不能以官方 bool、mode 缓存或 fresh-install 标记代替实测。实际写入对照须先确认能够完整备份并恢复所有受影响状态，否则保持未执行；本节尚未执行 DLL／OS 写入或设备验收。

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

上述报告／退出链的测试须分别覆盖：原生返回字符串的成功与异常释放、任务在锁等待／查询中／保存请求后取消、同 channel 查询与退出的先后、降级 API 表缺少退出导出、CTR 未启动／已停止／正常停止、状态 getter 失败、事件／消息投递失败、状态迟迟不达 3、工作线程等待失败及 `FreeLibrary` 失败。无设备时先以当前原件独立期望模拟这些状态和副作用；实际 DLL 对照仍须核实输入、版本、完整清理及允许的执行边界。错误码相同、返回 true、模拟状态 3 或两边未连接均不能算实际功能通过。

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
| Blade2 初始化、普通 destroy／terminate、系统报告与 stop | [rzDevice25LedMatrixBlade2](../../../.analysis/inputs/frontend/system-mw-694-current/raw/apps.razer.com/synapse/products/694/mw/rzDevice25LedMatrixBlade2.e873e947647bf67aeacb.js)：`init [4916,8493)`、`destroy [10282,10395)`、`terminate [10395,10712)`、`getAllBladeInfo [15279,15512)`、`colorProfileStop [17431,17665)`；其余 7 款完整 owner 见下述报告 |
| 实际继承的 main async 路由 | [rzDevice25LedMatrixBlade](../../../.analysis/inputs/frontend/system-mw-694-current/raw/apps.razer.com/synapse/products/694/mw/rzDevice25LedMatrixBlade.5a2f363ccf2909c58b93.js)：完整模块 77127 `[555,13021)`，继承赋值 `[1726,1892)`；同 MW 6737 的完整公共模块 19635 `[341306,343997)`、`oE` `[343502,343739)` |
| MW 退出、恢复与报告取消消费者 | 同 MW 6737：onLoad `[310475,312268)`、`cleanUpDevice Gn [137790,142080)`、色彩 stop／标记 `[362772,362954)`、报告完成消费者 `[1508211,1509037)`；Task／Queue／Manager 完整变量 `[1337211,1338122)`／`[1338123,1340586)`／`[1340587,1342184)` |
| preload 与宿主真实退出顺序 | [preload.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/preload.js)：main sync／async `[5289,5349)`／`[5350,5420)`；上述 main.js：`quitApp Dn [20963,23367)`、`quitAppFinal Mn [23367,24455)`、force-exit timer `[38615,38786)` |
| 宿主 FFI 互斥、callback／指针释放及空 FreeFFI | [FFIPreloadMain.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/ffi/FFIPreloadMain.js)：async 调用 `[1143,2273)`、sync 字符串释放 `[7758,8563)`、两个 dispatch `[8564,11243)`／`[12005,13913)`、退出注册 `[3227,3623)`／消费者 `[13914,14253)`、空释放字段 `[7719,7757)` |

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

本次复用现有 acquisition 与完整 capture：所用 691 原件的实际长度／SHA-256 与两份记录一致；两层 capture 各有失败阶段，未将其改写为整套成功。系统信息报告中的 262 个完整源码所有者及 1398 [恢复审计](../../../.analysis/results/frontend/audio-help-reset-audit.json) 的 24 个所有者也逐一核对了完整切片、实际 SHA 与 AST 边界，无差异。上述新增退出链另核对 7 份 DLL 原件、118 个引用函数、121 个函数块及其反汇编机器码，均与原件 PE 一致；8 份 manifest 的版本／包绑定、3 份包及 CTR 静态解包收据、host ASAR 与三个所用提取文件的实际字节也一致。分析器为 IDA 9.1／Hex-Rays 9.1.0.250226，身份与来源保留于 [获取收据](../../../.analysis/inputs/official-native-current/acquisition.json)、[输入索引](../../../.analysis/inputs.json) 及各包 inspection，不作为产品运行门禁。1.0.45／46／50 的 capture 各仍有一个其他函数的 Hex-Rays 失败，未把引用函数核验改写为整库反编译成功。

已解析的 main FFI 路由、退出注册消费、四版通知窗口／CTR stop helper 和三版 CTR 实体停止目标分别形成上述静态证据；其余源图缺失、原生间接调用与插件／产品恢复分支仍分别保留。独立宿主 About 的当前原件及真实版本 producer 已在设置文档补齐，Rust 接线另行核验。核验数量不代表整个 DLL、全部恢复、Rust 迁移或实际 DLL／设备对照完成，也不提升现有审计报告的整体未完成状态。

色彩链另外逐份核对下列当前 MW 的完整 owner。区间是 UTF-8 原始字节半开区间；启动／停止 owner 均从模块 `21705` 的实际导出绑定取得，cleanup 为 `hasCustomColorProfile → Hb` 所在完整函数。

| PID／MW 原件 | cleanup | 启动 owner | 停止 owner |
| --- | --- | --- | --- |
| 694／[6737.43877d4945b570a8510a.js](../../../.analysis/inputs/frontend/system-mw-694-current/raw/apps.razer.com/synapse/products/694/mw/6737.43877d4945b570a8510a.js) | `[137790,142080)` | `[362122,362349)` | `[362772,362954)` |
| 695／[6737.6b49213efda7170b54b7.js](../../../.analysis/inputs/frontend/system-mw-695-current/raw/apps.razer.com/synapse/products/695/mw/6737.6b49213efda7170b54b7.js) | `[1217905,1222344)` | `[1356015,1356240)` | `[1356655,1356850)` |
| 696／[6737.884d7b06158311ba879b.js](../../../.analysis/inputs/frontend/system-mw-696-current/raw/apps.razer.com/synapse/products/696/mw/6737.884d7b06158311ba879b.js) | `[1195577,1199918)` | `[1328882,1329107)` | `[1329522,1329702)` |
| 709／[6737.d34a3f9a638030f35c6a.js](../../../.analysis/inputs/frontend/system-mw-709-current/raw/apps.razer.com/synapse/products/709/mw/6737.d34a3f9a638030f35c6a.js) | `[1211756,1215586)` | `[1349213,1349440)` | `[1349863,1350060)` |
| 710／[6737.0a09be13ed46538a373d.js](../../../.analysis/inputs/frontend/system-mw-710-current/raw/apps.razer.com/synapse/products/710/mw/6737.0a09be13ed46538a373d.js) | `[1211756,1215586)` | `[1349213,1349440)` | `[1349863,1350060)` |
| 711／[6737.097a717c0f79e9e4ca4b.js](../../../.analysis/inputs/frontend/system-mw-711-current/raw/apps.razer.com/synapse/products/711/mw/6737.097a717c0f79e9e4ca4b.js) | `[1211766,1215596)` | `[1349223,1349450)` | `[1349873,1350070)` |
| 736／[6737.45e4affd7675b37f3c36.js](../../../.analysis/inputs/frontend/system-mw-736-current/raw/apps.razer.com/synapse/products/736/mw/6737.45e4affd7675b37f3c36.js) | `[1211756,1215586)` | `[1349213,1349440)` | `[1349863,1350060)` |
| 737／[6737.85397ce157d4cca228ee.js](../../../.analysis/inputs/frontend/system-mw-737-current/raw/apps.razer.com/synapse/products/737/mw/6737.85397ce157d4cca228ee.js) | `[129173,132985)` | `[361208,361433)` | `[361848,362043)` |

这组色彩证据的[完整分析结果](../../../.analysis/results/host-current/color-profile-cleanup.json)保留 112 个完整源码 owner、前述 7 份 DLL 捕获中的 84 个语义函数身份与原始 chunks／交叉引用，以及 24 个实际 vtable 区域；函数块与原始 PE 一致，所用函数没有缺失伪代码。1,613 个来源、获取收据与捕获产物在发布前重新核对长度和 SHA-256，没有变化；结果不依赖临时目录。完整源码与 native 收据继续复用当前获取记录和捕获，未运行新的 IDA 队列或改变原捕获中的其他失败状态。这些静态核验不证明 Rust 独立实现、官方实际 DLL 对照、系统状态恢复或设备验收已完成。

全产品恢复的具体字段、设备命令和恢复后事件仍需逐产品接续；本文件不把通用 Help 按钮闭合等同于全产品恢复协议闭合。本次仅静态阅读，未执行页面、DLL、文件对话框或恢复写入。
