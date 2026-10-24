# 691 OLED 主屏、媒体、语言与设备传输

691 当前导航明确包含字面名称 `OLED`：nav id=3，懒加载 chunks 3725/7077 后挂模块 42553。它不是仅注册而未挂载的页面。导航的 `cs=["OLED",Power,Help]` 只控制配置档栏显示；OLED 页仍由 active_view 直接选择并渲染。RGB 灯光及接收器指示灯另见 [lighting-power.md](lighting-power.md)。

## 挂载、版式与编辑所有者

页面顺序为全局下载状态、错误提示、Home Screen Display，随后左列 OLED 亮度和语言，右列返回主屏时间、dim、screensaver。页面实体持有“是否打开编辑窗口”和 selectedEdit；每个内容编辑器持有自己的草稿。progress 状态使主屏选择区域灰态；BLE 下 animation/image/emote/banner 的编辑入口拒绝打开，media/system 整卡禁用，语言设置同样禁用。BLE 仍可选择硬件已有主屏内容，不能据“编辑禁用”推断 OLED 整体不可用。

`HomeScreenDisplay_body` 桌面最小宽度为 1220 px；1279 px 以下改为 600 px 且 padding 为 30/25 px，卡片横向换行、间距 10 px。媒体和 banner 预览使用中列 236 px 的三列 grid。内容弹窗 animation/image/emote 最大宽 800 px，其余 850 px；emote 不使用编辑区滚动，其余内容保持原版 scroll。裁剪框固定 232×64、比例 3.625，不可改变 crop box，图片可以移动和缩放。

主屏卡以协议 type 标识：animation=0、image=1、banner=2、keyboard=3、emote=4、media=5、system=6。keyboard 为不可编辑的本机设备信息预览；卡片 hover 才显示 Edit/Apply，已选中 Apply 灰态。media/system 额外显示依赖本地 Synapse 服务的提示。Home Screen 总开关保留所选 type。

## 首次读取与硬件事件

MW 691 明确选择 `rzDevice25Oled`，规格为 232×64、帧间隔 33…1000 ms、最多 150 帧。连接后非 BLE 注册 OLED 写入进度回调；setup/reconnect 在设备 READY 且有 serial 的适用路径执行初始化，而不是仅依据本机有没有文档就恢复默认。

初始化 `22001.O` 先加载本地字体，非 BLE 设置系统监测周期，按 `productId_serialNumber` 初始化 OLED 资源 IndexedDB，读取真实 lowPowerMode，再进入 `C`。只有低功耗关闭时按以下顺序读取：

1. animation 启用位、各槽 GUID、当前 animation 参数；
2. image 启用位、各槽 GUID、当前 image 参数；
3. 当前 active item；
4. 返回主屏时间、dim、screensaver、OLED brightness；
5. language、低电量警告百分比；
6. 设初始化完成标记，再合批发布 UI 状态。

每一步要求相应设备返回 SUCCESS；未知 type、未知时间编码和读取失败抛出错误。页面 reducer 的预览默认文字、假定 CPU/温度/日期或默认选图不属于这次真实读取。当前 `getOLEDLowPowerMode` helper 失败后记录并返回 0，是原件中的失败回退边界，不能据此宣称低功耗已经确认关闭。

GUID 全零/空值标识该槽使用设备原生内容；非零保存为 custom GUID。自定义图片只在 IndexedDB 的 GUID 与设备所读 GUID 一致时投影成 src，否则返回空 src；本机没有媒体原件并不证明设备没有自定义图。active item 为 OFF 时保留既有 type 且 enabled=false。BLE 遇到 system/media active item 时，原版会真实写回 ANIMATION，然后投影该结果；这是明确的产品恢复链，不应以通用“只读打开”替代。

硬件 OLED menu 事件另触发真实重读 Home Screen、返回主屏时间、dim/sleep、OLED brightness、键盘 dim/sleep 和轮询率；Chroma effect 事件进入灯效消费者；lowPowerMode 事件更新运行态并停止/恢复媒体和系统监测。Command Dial 状态事件先读 `getOLEDCommandDialModeState`，再按实际 mode identity 更新本机当前档和 UI。单纯收到通知不等于已经知道设置的具体值。

## animation 与 image 草稿

animation 编辑器复制当前六槽列表和 selectedIdx。允许 GIF 文件、每槽 enable、选中、替换和恢复本槽原生内容；最后一个 enabled 槽不能关闭，关闭当前槽会选下一个 enabled 槽。image 同样处理十槽，只接受 PNG/JPG/JPEG/BMP。槽的稳定身份和位置各有用途，不以预览文案作为存储身份。

文件先进入裁剪草稿。GIF 在独立 worker 进行帧处理，worker progress/complete/error 分开处理；静态图输出 232×64 base64。30/15 FPS 决定 worker animationDelay=3/6。Cancel 终止 worker，卸载也终止；当前组件创建 object URL 后没有明确 revoke，不能写成资源已全部释放。裁剪成功只更新编辑器草稿，不立即刷设备。

编辑器 Cancel 恢复打开时列表/选中项并关闭；Apply 才 dispatch 列表。UI reducer 将槽转换为 `{ggid,isEnabled,isCustom,src}`，另比较 src 形成 modifiedSrc，携带 selectedIdx 发送 `ON_SET_OLED_HOME_SCREEN_DISPLAY_SETTINGS`。页面立即更新本地显示并登记 action，设备传输在后续队列完成。

MW 保存 animation/image 前先写启用位，再对 modifiedSrc 逐槽读取设备 GUID。需要改变时将原图转为设备数据；不需要改变时只更新资源关联。自定义图的 GUID 由内容产生，恢复原生内容使用全零 GUID。animation 写槽 index+2，emote 使用独立槽 1；image 使用其独立 index。每个成功写入才更新对应运行态和 IndexedDB 资源。全部目标完成后才设置当前 gifIndex/imageIndex（协议为 1 起始）。取消或失败通过 finally 发布实际剩余状态，不能把整个草稿无条件当成设备已完成。

## emote、banner、media 与 system

emote 是当前官方本地 emote 词表及图片，搜索忽略空格和大小写。选中项先解析 lazy 资源；Apply 要求 isDataMatch，发送 emote identity 和图像数据，Cancel 恢复打开时对象。设备写前读取 emote GUID，仅在内容不一致时转换/写入。

banner 草稿包括图像启用、原生图身份、上下位置、文字、字体、字号、粗斜体/下划线和四向滚动。Reset 使用当前 reducer 的原版 banner 默认对象。文字按实际 DOM 宽高限制：横向 920 px，纵向为有图 176/无图 256 px，超出时去掉末字符并显示提示。Apply 将预览文字渲染成黑底 PNG：横向按 232 的整数倍取宽，纵向按有图 44/无图 64 的整数倍取高，再发送 imageData 和文本样式。设备先写 banner 参数，比较 GUID 后才写像素；当前 finally 仍会保存 banner 并投影 UI，即便局部取消，因此“本机保存”必须与传输结果分别记录。

media 草稿包含曲目信息启用/上下位置和 visualizer 启用/三种样式。两个开关互相约束以避免两项都关闭；Reset 开启曲目信息 top 和 visualizer0。Apply 编码为 visualizerState/type、mediaState/position。消费者先写真实 AUDIO_DISPLAY 参数并要求 SUCCESS，再配置本机音频观察、启动 `rzAudioMon`，有用户修改时保存本机文档。

system 是三页、每页左右两槽的拖放编辑，内容用领域 tag/type。可以选页、拖放/删除 tag、调轮播间隔、摄氏/华氏、日期格式和 12/24H；三页左右全空时 Apply 灰态。Cancel 恢复所有打开时草稿；Reset 仅恢复源码列明的页/间隔/温度项，不能额外假定它同时重置日期/时间。语言决定写入设备的 label；预览 CPU/日期等静态默认样例不得当成硬件遥测。

system 消费者对三页生成带标签和日期的背景，读取每页 GUID，仅发送变化图像，随后配置六槽 type、占位坐标、间隔和温度/时间格式；启动真实系统监测。系统数据回调将 CPU/GPU 使用率和温度、内存、电池值投影 UI，同时发送 `setOLEDUpdateSystemInformation`。参数宽度改变时重算坐标；日期被选用时有独立跨日 timer 重绘日期背景。音频回调按所选 visualizer 写幅度，曲目信息在按产品/serial 的锁内合成 `Title - Artist`，对比 GUID 后写曲目文字。音量/静音单独走 debounce 的 `setOLEDUpdateVolume`，不是重写一幅静态截图。

## 标量设置、语言和确认

| 控件 | 请求与实际结果路径 |
| --- | --- |
| OLED brightness | 20…100、step1；maker 再 clamp，NORMAL 队列真实 setter 要求 SUCCESS；硬件增减走 NORMAL_SKIPPABLE，BrightnessToggle 不适用 |
| 返回主屏 | 默认按钮 5/10/15/20 秒，产品表可替换；编码映射 `5→1,10→2,15→3,20→4,0→0`，未知值拒绝 |
| dim | 默认按钮 `[1,3,5,10,15]`，保留 enabled；禁用时内容遮罩；maker 将 disabled 编码为 0 |
| 低电量警告 | 保存 enabled/value，disabled 同样编码为 0；位于 Power 页 |
| screensaver | 无/三幅原生图；协议分别编码 enabled 和 item，runner 成功后更新运行态 |
| language | UI 默认 English=0、简体中文=1，协议词表还包含 ja/de/kr/fr；产品提供的 UI 表决定当前可选项 |

亮度、返回主屏、dim、低电量和 screensaver runner 检查真实 setter status，错误置 result=false。它们不是统一写后再 GET；硬件 menu 消费者才走对应真实 getter。部分 task callback 只检查 value 是否存在并打印成功色，因此日志外观不能代替 result/status。

语言下拉维护尚未 Apply 的选择，与 reducer 当前值分开。原始 language 大于 127 时按 `255 & ~value` 还原显示身份。Apply 后先读取设备 language，同值直接结束；否则加载当前本地语言 bin，先将当前语言设为补码/更新中编码，再执行 `setOledFlashBlock(1,bytes)`。只有传输 SUCCESS 才写最终目标 language；USER_CANCEL 清进度并保留实际阶段，finally 总是把当前 language 投影到 UI。它不把取消包装成目标语言成功。

语言变更回执增加计数，已选 system 时按新语言重新生成标签/页面；首次 effect 不重复提交。语言加载来源是当前原件的本地 chunk/bin，不等同于允许本项目运行时任意远端下载业务内容。

## 进度、取消、重试和 reset

`OledDownload` 依据目标字节数和有线/接收器/亮度条件估算耗时，选择 simple 或 progress；保存 target、currentItem、totalItems、progress，过滤重复百分比，最后一项 100% 才清进度。该进度是传输事件，不是设备连接或配置档保存状态。

animation/image/reset 显示逐项进度；reset 取消入口禁用。普通 Cancel 发送 `ON_SET_CANCEL_OLED_DOWNLOAD_PROCESS` 并立即清 UI progress；MW 用 IDLE 类别的取消任务调用 `stopDataSendOnErrorOrCancel`，真正停止由分包循环检查 stopDataSend。语言 Cancel 先显示原版确认，确认动作按实际回调发送 `ON_CANCEL_OLED_LANGUAGE_UPDATE`；关闭确认不自动改变设备状态。

失败保存 errorPayload。Retry 复用消费者保存的最后完整 payload，重新进行 GUID/传输步骤；Revert 的 runner 清下载队列和状态，没有通用“把旧硬件图回写”的补偿过程。Reset 来自 Header 的明确用户动作：读取各槽 GUID，只替换真正自定义/不同的图像，停止两个监测，选 animation 并逐类写默认原生图/文字，finally 发布 reset 结果和每类当前数据。BLE reset 只选 animation 的独立路径。源码中 USER_CANCEL 分支退出循环后并未统一把 success flag 改为 false，不能将该 flag 描述为所有槽读回一致。

## 像素、分包、HID 应答和释放

图像转换将 RGBA 计算为 `floor((.299R+.587G+.114B)/16)` 的 4 bit 灰度，相邻两像素分别放低/高半字节。转换数据先放三项 32 bit 头（frameCount、width、height），再放像素。GIF 按 disposal/帧 patch 合成，受本产品 maxFrames 和帧间隔限制；banner/系统标签由真实文字布局生成，不能用任意字体或占位图替代。

`rzDevice25Oled` 先连接独立 OLED flash interface，再连接普通 Protocol25 interface，分别有最多三次连接尝试；随后探测批量 OLED 与异步 write/read 支持。它拥有按 container/interface 命名的 flash lock、stopDataSend、批缓冲及进度 emitter。

flash report 为 64 字节，数据包携带 60 字节内容和 16 bit 包序号；block、GIF、banner、audio、system、image 使用各自 header/data command。GIF 有 512 字节媒体头，含帧数、间隔、GUID、像素加和校验；静态内容有宽高/GUID/校验的媒体头。GUID 和校验各自承担协议语义，不是分析输入的 SHA 门禁。

header/data 写通过 `hid.write` 或 `hid.writeAsync`，每个 report 前加 reportId。读回必须长度匹配、command id 匹配；状态 1 为 busy、2 为 SUCCESS、3 为失败、5 为不支持，其余未知。busy/可重发错误依协议重试，unknown/unsupported 结束。批模式使用 `hid.writeOledBatch`，带 packetSize/maxRetryOut/maxRetryIn；只有实际 flush 成功才可认定该批写成功，buffer append 的 SUCCESS 不是设备应答。底层 writer 遇 stopDataSend 返回 USER_CANCEL；最终仍执行批缓冲 flush，取消不能宣称绝无最后一批 I/O。

上述 flash 是 OLED 内容/语言的本机传输，不是执行设备固件升级。本文仅静态还原了命令参数与原版调用链；没有实际执行这些写入。

### Node binding 与 Windows HID 传输原件

上面的 JS 协议现在可以继续落到实际 Node/HID 边界。当前 [rzDevice25Oled.d8bbbaf867fd89e488bb.js](../../../.analysis/inputs/frontend/keyboard-mw-691-current/raw/apps.razer.com/synapse/products/691/mw/rzDevice25Oled.d8bbbaf867fd89e488bb.js) SHA-256 为 `3c9f66d42c3eab893d01a40b4e957382a651095ef62c964920d55ecb5a7f6c7d`。模块 `39257` 的构造/连接/锁/应答 owner 是 `[4212,11752)`，批写 `[12233,14281)`，数据格式与取消循环 `[14897,19455)`，各内容媒体传输 `[19455,23814)`。常量的有效命令 ID 是：block header/data `2/3`，GIF header/data `4/5`，banner header/data `6/13`，audio text `14/15`，system image header/data `16/17`，home image header/data `18/19`。每个 flash 命令先构造 64 字节零缓冲，Electron 发送时前置 report ID 形成 65 字节；data 包在 `[4..]` 放长度、索引或图像数据，不能把 64 字节 report 当成 64 字节可用 payload。

`_sendCommandOLEDFlash` 通过 `hid.write` 或检测到 `writeAsync` 支持后使用 `hid.writeAsync`，发送返回值本身不参与成功判定；只要调用未抛出就先把本次写状态置为 `STATUS_SUCCESS`，随后仍执行读取。`_getOLEDInResult` 要求读取长度恰为 64，比较响应 `data[0]` 与请求的 command ID，读取 `data[1]` 作为状态：1 busy 会重试，2 才成功，3 失败并结束本次内层读取，5 不支持和其他未知值结束且不重发。空响应、长度不符和命令 ID 不匹配只产生错误文本；外层 `OLED_FLASH_COMMAND_MAX_RETRY_OUT` 循环据错误类型继续或结束。这里没有 checksum 或全媒体内容读回，协议应答只确认该 report 的命令处理状态。

批模式的边界不同：`addOLEDFlashBuffer` 只把 `[reportId,...64B]` 追加到内存并立即返回 `STATUS_SUCCESS`；真正的 `sendCommandOLEDFlashBatch` 才调用 host `hid.writeOledBatch`，空 buffer 甚至直接返回 SUCCESS。host 原件 [UsbRzDeviceAction.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/UsbRzDeviceAction.js) SHA-256 为 `bdd1b91f94898e7d89e13584d132155526b7ca477275c9b0037876f42f617816`：`writeAsync` `[27428,28165)`、`readAsync` `[28165,28809)`、`writeOledBatchMutex` `[28809,30177)`、`writeOledBatch` `[30177,31395)`、`hidReadOled` `[31395,31502)`、`getOledFlashInResult` `[31502,32332)`。批写每次按 65 字节切片，默认 `maxRetryOut=3`、`maxRetryIn=20`（调用方可以显式覆盖），先要求 `writeAsync` 返回等于 chunk 长度，再用五次 `readTimeout(10)` 轮询取 64 字节回执；status 1/3 允许该块重发，status 2 才结束该块，空/非法回执使批写返回 false。批 flush 的布尔值与逐块状态必须单独记录，不能用 buffer append 的 SUCCESS 替代。

对应 Node 原件 [HID.node](../../../.analysis/inputs/installed-current/files/app-engine/resources/app.asar.unpacked/node_modules/node-rz-hid/build/Release/HID.node) SHA-256 为 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`，独立 `capture.json` SHA-256 为 `d9e52173142940c7d022bccddcdbb47afe1596e8c2d92190e39929f56d7e95f1`，Image Base `0x180000000`。用有界 `ThreadPoolExecutor(max_workers=4)` 并行校验了本页涉及的 22 个完整函数；每个函数 JSON SHA、每个 chunk 原字节及 assembly bytes 都与实际 PE 一致，合计 22 functions、22 chunks、4,305 instructions、18,071 raw bytes。整个 capture 的 `capture_complete=true`，但 `all_pseudocode_captured=false`（另有一个未选中的函数失败），因此该批不能扩展成整个 Node 模块已闭合。

| 原生节点 | 已核实行为 | 证据 |
| --- | --- | --- |
| `write` / `writeAsync` binding | `write`（`0xa110`）要求单个 array/buffer，数组元素必须是整数；关闭句柄或 `hid_write<0` 抛 TypeError。`writeAsync`（`0x9750`）复制同样的 payload 入 worker request type 4，worker 仍调用同一个 `hid_write`；Promise resolve 是写入字节数。 | [0xa110](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/a110.json)、[0x9750](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/9750.json)、[0x4800](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/4800.json) |
| `hid_write` | 发送长度短于设备 report 长度时先补零；用 `WriteFile` + OVERLAPPED，pending 后最多等 1000 ms，成功返回 `NumberOfBytesTransferred`，失败格式化 Windows 错误并返回 `-1`。选中 timeout 分支没有显式 `CancelIo`。 | [0x175e0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/175e0.json) |
| `readTimeout` / `readTimeoutAsync` binding | `readTimeout`（`0x6fb0`）要求 timeout，使用 2048 字节缓冲并同步返回数组；`readTimeoutAsync`（`0x6cd0`）入 worker request type 5。OLED host 实际批/普通读路径使用前者，不能因为存在 async export 就写成页面正在异步读。 | [0x6fb0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/6fb0.json)、[0x6cd0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/6cd0.json) |
| `hid_read_timeout` | `ReadFile` pending 时按调用 timeout 等待；等待超时返回 `0`，没有在该分支主动取消。首次读取非 pending 错误会 `CancelIo` 并返回 `-1`。成功包若首字节为 0，会剥去 report ID 后复制；首字节非 0 时保留首字节。OLED command-ID 比较因此依赖真实 report ID/驱动返回布局。 | [0x17800](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f)、[0x179f0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/179f0.json) |
| worker 与批入口 | worker 分发 type 4/5 并在失败时附带 `[HID.WRITE]` 或 read 错误；`sendFeatureReportInBatch` 另要求 ArrayBuffer，和 OLED `writeOledBatch` 的逐块 `writeAsync` 路径不是同一 API。 | [0x4800](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/4800.json)、[0xc8d0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/c8d0.json) |
| close 与清理 | Native `hid_close`/JS close 会 `CancelIo`、关闭事件和 HID handle、释放对象；Promise finalizer 释放 request/buffer/context。worker 析构置 stop、通知并 join 后清队列；关闭 owner 没有证明先等待所有队列请求，join 是独立的停止边界。 | [0x17de0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/17de0.json)、[0x6170](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/6170.json)、[0x94e0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/94e0.json)、[0x15d40](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/15d40.json)、[0x4710](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/4710.json)、[0x50e0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/50e0.json) |

页面 `stopDataSendOnErrorOrCancel` 只设置共享 stop 标志，分包循环在下一块前检查；已开始的 `writeAsync`/`readTimeout` 没有页面 abort signal。`readTimeout(10)` 的五次尝试和 `_sendOledFlashData` 最后的批 flush 仍可能产生最后一批 I/O；取消、Promise 完成、批 flush 成功和设备内容已经恢复是不同状态。当前证据没有写后完整媒体 GUID/像素读回，也没有证明 reset 失败会自动恢复旧图。

切换内容先停止 `rzSystemMon`/`rzAudioMon` 和日期 timer；选择 system/media 后按实际内容重新启动所需监测。设备 remove 路径执行 OLED uninit：非 BLE 解除写进度回调和系统监测设置，取消日期 timer，再进入设备通用释放。flash 对象 beforeunload 调用 terminate、移除自身 listener、清 emitter。普通内容组件的轮播 interval 与拖放外部 click listener 有对应 cleanup，GIF worker 有卸载 terminate；匿名函数/未释放 URL 等局部边界按原件保留。

## 原件定位与验证边界

UI 当前收据为 [691 UI](../../../.analysis/inputs/frontend/keyboard-691-current/acquisition.json)，MW 当前收据为 [691 MW](../../../.analysis/inputs/frontend/keyboard-mw-691-current/acquisition.json)。MW 本轮成功取得的 134 份原件已逐一校验文件长度和 SHA-256，一致；缺失的官方 source map 未用作停止追踪的依据，以下直接还原实际 JS 模块/函数。所有范围是原始 UTF-8 字节半开区间。

| 原件 | owner |
| --- | --- |
| UI `7668.5b809e87.chunk.js` | 实际 OLED nav `[157108,157785)`；设备页 owner `[150871,161474)` |
| UI `OLED.5202d0af.chunk.js` | 模块42553 `[956,86773)`；裁剪 `[9543,12432)`；animation `[20076,21797)`；image `[23866,25258)`；emote `[26347,27741)` |
| 同 UI chunk | banner `[42801,49593)`；media `[50540,52787)`；system `[60698,65395)`；主屏 `[67635,74352)`；标量/进度/错误/top `[76103,86772)` |
| UI `main.c20bafd3.js` | 主屏 reducer 模块79826 `[711712,728496)`；标量/进度 reducers `[796000,803450)` |
| UI `OLED.888f5528.chunk.css` | 当前主屏卡片、cropper、编辑窗口、预览与所有状态的完整样式原件 |
| MW `main.58e63fd267c2f91d3ebe.js` | 产品选择 `rzDevice25Oled` 与 232×64 规格附近 `[193795,194440)` |
| MW `6737.b169abe06a428339f90f.js` | setup初始化调用附近 `[1211770,1212010)`；remove/uninit附近 `[1187900,1188450)`；进度/menu事件模块13881 `[1254000,1259330)` |
| 同 MW chunk | 读取/初始化/广播/释放模块22001 `[1265284,1272998)`；内容消费者88728 `[1272999,1297203)`；makers `[1331320,1334259)`；scalar/retry runners `[1532400,1541800)` |
| 同 MW chunk | 系统/音频观察81067 `[1720074,1723883)`；图像/文字转换43429 位于 `[1259330,1264596)` |
| MW `5222.f118a4b3924936cd64a3.js` | OledUtils、资源 IndexedDB、OledDownload 进度与取消的完整所有者 |
| MW `rzDevice25.0cfe1013577a467100b3.js` | Home 参数 `[238796,240611)`；亮度/语言 `[240611,242384)`；active item `[246352,247009)`；GUID `[247824,248373)`；槽启用 `[250713,252167)` |
| MW `rzDevice25Oled.d8bbbaf867fd89e488bb.js` | 构造/连接/锁/应答 `[4212,11752)`；批写 `[12233,14281)`；数据格式/取消循环 `[14897,19455)`；各内容媒体传输 `[19455,23814)` |

本页描述已追踪的官方静态语义；Rust 独立实现、UI/后端接线、原生服务二进制完整语义与真实设备验收仍分别计量。没有将本地资源缓存、前端样例值、外层 completed、源码结构抽取或静态分析误写为设备实测通过。
