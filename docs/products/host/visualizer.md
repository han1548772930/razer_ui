# Visualizer：音频怎样变成灯光

Visualizer 从所选 Windows 音频端点取得频谱，在 256×64 的逻辑画布合成前景与背景，再按设备类别映射灯珠并提交 Chroma。页面预览显示这份逻辑画布；预览开关与真实输出开关是不同状态。

## 挂载和五个面板

当前主入口加载模块 7089，其 Provider 下依次挂载错误边界、版本标签、Chroma 争用提示、页面、观察组件和资源预热节点。实际设置区分为两行：第一行是音源／启用条件及预览，第二行是前景、背景、属性，间距 20 px、最大宽度 960 px。切到 Help 替换设置区，不表示整个根组件卸载。

| 面板 | 操作与含义 |
| --- | --- |
| Chroma Visualizer | 总开关、音频端点下拉、仅选定程序运行时启用、程序增删 |
| Preview | 显示合成画布；按当前总状态和选中程序运行状态决定可见性 |
| Visualization | 前景效果、颜色／渐变、亮度 1–100 |
| Background | 背景效果、颜色／渐变、Reactive Background、亮度 1–100 |
| Properties | Decay 1–10（Slow→Fast）、Scale 1–10 |

前景效果顺序为 SpectrumCycling、Static、Wave、Wheel、Custom。背景在同样顺序后另加 Off。当前传给下拉框的就是这些数组，不能按名称重排或给前景补 Off。

## 状态与初始化

初始对象包含 AlwaysOff、关闭预览、背景 Off／亮度 30、前景 SpectrumCycling／亮度 100、Decay 5、Scale 5、音源下标 0、空应用列表，以及八种选中设备类别。reducer 初始化时把本机保存对象合并到这些应用配置中；它们不是设备读回值。

主页面挂载时分别构造 Chroma SDK wrapper 和 Visualizer engine。后者注册帧、批量帧、预览、音频设备改变、应用运行改变的事件。页面 beforeunload 处理移除自己的 beforeunload 监听，调用两套 terminate；当前回调不 await 两个调用全部完成，也没有在该 class 中定义配对的 componentWillUnmount。

音源面板发起 DLL 初始化；初始化状态为真后，观察组件先获取音频列表，再重启效果。应用状态变化、语言、共享 Chroma Connect 列表及本机保存项还有独立消费者，不能用“根组件已初始化”概括所有观察流程。

## 音源列表、排序与选择

本机音频列表不是固定词表。当前原生枚举先加入 Default Playback Device，再加入所有活动播放端点，最后加入活动录音端点。每项输出 name、deviceId、isCapture。默认播放项与它对应的显式播放端点可同时出现在列表中。

前端对默认项名称做语言替换，保留原列表顺序；没有通用按名称排序。选中信息同时保存下标及对象。列表变化后，主页面按之前名字查找；找不到选下标 0。音源面板还根据持久化 selectedAudioDeviceInfo.name 尝试恢复，但当前下标为 0 时保持默认项。

Electron wrapper 的 `setAudioDeviceIndex` 只把命令放入 200 ms debounce 后立即返回 true。action 随后保存下标和对象并更新 reducer。因此这份 UI 更新不证明原生切换已完成。原生 setter 会把负数或越界下标改成 0；捕获已启动时重新建立录音器。

音频设备改变事件在 100 ms 后通知页面重读列表；audioCaptureError 分支在仍标记捕获时停止并重新启动捕获。这是该 engine 的具体事件处理，不能扩写为全产品统一重试。

## 三种运行状态与程序列表

运行状态分 AlwaysOff、AlwaysOn、EnabledForSelectedApps。总开关使用 600 ms debounce；关闭时同步修改持久化状态及 Chroma 模式，开启时根据“仅选定程序”字段恢复 AlwaysOn 或 EnabledForSelectedApps。

SDK 初始化后等待 600 ms，再从共享 Chroma Connect 数据查 Visualizer 应用和全局 enabled。未取得可用运行列表时最多按 1 秒间隔检查 30 次；达到次数仍失败会调用当前切换回调。SDK ready、Connect enabled、应用 running 与用户开关分别存在，不能合并成一个成功标志。

程序列表添加通过本机 exe 文件对话框选第一项，身份取文件 basename，按当前字符串规则检查重复；图标由系统查询。reducer 用 `name` 升序排序。删除后重新设置引擎应用名列表。引擎每 2 秒检查列表中的任一进程是否运行：匹配时启动捕获，不匹配则停止捕获并发送空帧；变化时发 appLaunchedChange。

原生 IsAppRunning 解析逗号分隔名称，使用进程枚举、OpenProcess、模块 basename 查询并比较；匹配或失败结束时关闭已打开进程句柄。它检查进程名，不是当前前台窗口，这与配置档的前台自动切换是不同功能。

## 输入框与拖动提交

亮度、Decay 和 Scale 使用实际 stepper 与 slider 组件。文本输入点击后 focus 并全选；onChange 先用数字表达式检查，非法文本临时变 0；失焦时 parseInt，NaN 取 0，再夹在调用方 min/max 内并向父组件提交。Up／Down 调整 1；Enter 和 Escape 都触发 blur，当前组件没有 Escape 恢复原值的分支。

slider 的 change 更新面板临时值并标记正在拖动；全局 mouseup 清除标志，由 effect 提交。输入框提交也进入同一个面板值，但不需要等 slider mouseup。全局 mouseup 在 slider 卸载时注销。

背景开启 Reactive 时禁用手动背景亮度；背景 Off 时禁用该组。总开关关闭由覆盖层阻止各设置面板交互。没有独立 Apply／Cancel 弹窗。

## 效果参数和画布计算

前景与背景分别保存 effect、亮度、静态颜色、渐变色标及自定义颜色组。静态颜色从 `#RRGGBB` 转 `0xRRGGBB`；no-color 保留原 token。渐变编辑把位置转换为 `ceil(pos×100)`，颜色转换为整数；整个色标列表为纯白时，当前转换函数将它们改为黑色。空 Custom 色标会阻止预览面板发起效果更新。

engine.start 先 await stop，再检查初始化、设备列表和运行状态，重建前景、背景、输出三个 256×64 数组。像素更新、Chroma 输出、预览分别使用 40 ms interval；“仅选定程序”另有 2 秒 interval。当前像素计时器条件引用 `backgroundEffect === $.dg.off` 与 `$.dg.Off` 两个不同成员，文档保留其拼写，不替它推定为已关闭像素更新。

两层支持静态填色、HSV Spectrum、围绕 `(128,32)` 的色轮、横向 Wave 和分段线性渐变。亮度转色值的系数为 floor(2.55×brightness)。渐变按画布横坐标映射 0–100，逐 RGB 通道插值并应用亮度。

合成用 `fft[x] > (64-y)/64` 选择前景，否则选择背景。最上行额外由 fft[5] 生成左右延展；Reactive Background 使用同一频谱量调制背景，持续安静达到 background_timeout 后再走渐变逻辑。每种设备类别通过自己的行列映射从逻辑画布取色，不能直接把 256×64 原样发往所有设备。

预览把整数 RGB 转 ImageData，创建 ImageBitmap 再绘制到 canvas。此处读取到的 effect 未调用该 ImageBitmap 的 close；当前文档只记录实际资源操作，没有据此宣称已测得内存增长原因。

## 帧提交与本地 Chroma 协议

native SDK 模式按一次输出收集多种设备帧，转换为 create_effect 的数组；其他模式逐设备发帧事件。每个颜色先把 `0xRRGGBB` 的红、蓝通道互换为 `0xBBGGRR`，原生 JSON 消费者再按 32 位整数取值，不重复交换。默认设备数组及其实际输出如下；矩阵尺寸来自主包模块 3932，不能从某台设备的灯珠数推断。

| 类别 | 前端帧尺寸／JSON 形状 | wrapper 效果 | 原生效果调用 |
| --- | --- | --- | --- |
| keyboard | 6×22 矩阵 | CHROMA_VISUALIZER | CreateKeyboardEffect，类型 50，528 字节颜色矩阵 |
| keypad | 4×5 矩阵 | CHROMA_CUSTOM | CreateKeypadEffect，类型 2，80 字节颜色矩阵 |
| headset | 1×5 展开为 5 项 | CHROMA_CUSTOM | CreateHeadsetEffect，类型 4，20 字节颜色数组 |
| mouse | 9×7 矩阵 | CHROMA_CUSTOM2 | CreateMouseEffect，类型 8，252 字节颜色矩阵 |
| mousepad | 1×20 展开为 20 项 | CHROMA_CUSTOM2 | CreateMousepadEffect，类型 6，80 字节颜色数组 |
| chromalink | 1×5 展开为 5 项 | CHROMA_CUSTOM | CreateChromaLinkEffect，类型 1，20 字节颜色数组 |
| camy | 8×8 矩阵 | CHROMA_CUSTOM | 固定 GUID `d678d34a-336c-45e1-af84-e2319a47755c`，CreateEffect 类型 7；包装为 3616 字节结构，矩阵行跨度为 30 个颜色 |
| esther | 61×25 矩阵 | CHROMA_CUSTOM | 映射 GUID `035f9b21-9dc7-4480-81a0-ef0027f1fc41`，CreateEffect 类型 8；包装为 160016 字节结构，矩阵行跨度为 200 个颜色 |

一维转换的循环用列下标写入输出；有多行时后行会覆盖前行，不能把它概括成任意矩阵的通用 flatten。当前选中的一维类别都只有一行。常量和部分引擎分支还出现 kitty、nommo、nommo pro，但默认设备数组不含它们；当前 buildCustomFrameData 也没有 nommo／nommo pro 分支，不能仅因枚举存在就标记为接线完成。

SDK wrapper 同时保留 localhost REST 会话实现和 native 实现。REST init 向本地 54235 服务注册，取得 sessionid／uri，每 10 秒 heartbeat，结束发 DELETE。当前初始化 action 优先要求 native SDK 可用；不能因为 wrapper 有 REST 方法，就称当前 action 会在 native 失败时自动选 REST。

native 初始化根据 installedResources 选择原件并核 API 版本，调用 Init、GetChromaSDKVersion。原生 Init 接受可选路径；当前调用传空字符串，因此沿 Program Files 下的 `Razer Chroma SDK/bin/RzChromaSDK64.dll` 加载路径，再绑定 InitSDK、UnInit 和各类别效果函数。未加载函数在实际消费者处产生异常；加载 wrapper 成功并不证明这些下层函数全部可用。这是官方原件的调用关系，不是本项目接入官方业务 DLL 的实现方案。

JsonApiNoReturn 并非仅入队：它在当前函数内解析 JSON，将 action 转小写，再同步进入 dispatcher 和设备效果消费者，最后销毁临时 JSON／结果，不把 status 返回宿主。dispatcher 当前处理 init 和 create_effect；数组逐项执行，status 变量保留最后一次返回值，没有汇总每一项成功。wrapper 再返回空对象，不能据此确认整批帧或逐灯珠写回。init 的 InitSDK 消费者还会在调用返回后设置 initialized 位，即使 SDK 返回非零；这份位同样不能作为真实输出成功状态。

类别消费者把 JSON 颜色复制到清零的本地缓冲后调用函数指针，最后一个效果 ID 输出指针均传空，因此走即时应用路径。整数转换接受该 JSON 库的布尔、整数及浮点数分支，其他类型抛出 type_error；矩阵复制按输入长度循环，未在这些函数中限制行列数量，也未校验 RGB 范围。这里描述实际解析边界，不把厂商缓冲写入方式当作安全的任意长度输入契约。

已核验的下层 SDK 对键盘类型 50 复制 6×22 色值，把内部帧类型设为 17，再在控制权／启用位均允许时写入 `Global\\{74164FAD-E73C-4FA1-A9AA-70813315ED9C}` 共享区。该区大小为 `0x9c98`；写者按当前下标清空并复制 `0xb98` 字节帧，在偏移 2960 写入 GetTickCount64 时间戳，执行原件的 XOR 数据变换，再在 0–9 间推进下标并 PulseEvent 通知 `Global\\{45C97C2C-2D50-4F30-B50E-AFBB1CE22E93}`。鼠标类型 8 同样进入控制位检查，复制 9×7 色值到 `0x1c0` 字节帧，写入大小 `0x3a28` 的共享区 `Global\\{0DBE78AC-AC93-408F-A27E-8F61EA067B05}`，更新时间戳、变换数据并通知 `Global\\{0DB0CEFA-C51E-4255-87FB-2D36A0159896}`。这些发布函数没有等待服务消费或设备确认；外层即时 Create 函数仍可返回 0。

camy 的 GUID 分支转入专用设备帧，等待当前并行复制／登记任务结束，再将 `0x27f28` 字节帧复制到该设备共享区并通知事件，随后释放临时堆缓冲。通用 GUID 分支另有 `\\\\.\\pipe\\{GUID}` 输出，单次 WriteFile 请求 `0x27f78` 字节；消费者未比较实际写入字节数，也没有读取设备应答。这两条是不同的本地传输所有者。

esther 还有具体版本边界：wrapper 在该 GUID 下构造 CustomV2；这份已核验的下层 SDK 没有该 GUID 的专用分支，通用消费者对类型 8 返回 4319。wrapper 收到 4319 后清除 CustomV2 可用位，此后同分支直接返回 0，跳过下层调用。后续 0 不表示设备接受帧，也不能据此声称所有已安装 SDK 版本行为一致。

## 帧接收、实例路由与设备写入

本节限定于获取收据中的 Core `4.0.2.08241000` 与 Devices `4.0.3.09211418`。SDK、设备管理器与四个设备插件的原件、所选完整函数捕获及 PE 节映射已按实际字节核对；导出／导入、GUID、管道前缀和 XOR 表按各自引用位置核查。通用 GUID 管道、camy、键盘与鼠标共享区有不同接收链，不能按帧大小或 GUID 外观合并。

### 通用 GUID 管道的实际接收端

设备管理器 `0x2280 → 0x2400` 从 SDK 安装路径下的 `Devices.xml` 读取 `deviceid`，把每个 `devicepid.pid` 映射到其父节点 `device.id` 的 GUID。当前文件有 14 个该映射组；这是当前原件的目录内容，不是全部设备能力数。插件成功初始化后，`0x31c00 → 0x2640` 提取设备 PID 及大写 instanceId：先查 `PID | (GetEID() << 16)`，不存在再查 PID；没有可用导出时该扩展值取 0。

`0x2a00` 为同一 GUID 复用或创建服务器，`0x18f0 → 0x1a30` 用 `StringFromCLSID` 组成 `\\.\pipe\{GUID}`，启动 `0x2bf0 → 0x1af0`。这与 SDK `0x2a770` 的名称完全对应。`0x1c90` 创建 duplex、overlapped、message-mode 的管道，输入／输出缓冲均为 `0x27f78`；这个大小已由汇编 `0x1cb9/0x1cdd/0x1ce1` 复核，不能使用反编译中未赋值的局部变量解释参数。

下游由 `0x2000` 按 instanceId 登记客户端：它从大写设备路径中取 PID 后第一个 `#` 与最后一个 `#` 之间的内容；`0x1b9a0 → 0x1ba60` 打开 `\\.\pipe\<instanceId>`。服务器 `0x1e50` 每次请求读取 `0x27f78` 字节，`0x1f20 → 0x1bc70` 把同一缓冲依次转发给当前 GUID 下的实例。因此，面向 GUID 的 SDK 入口与面向 instanceId 的插件入口之间确有管理器转发，不是同一个管道名称。

接收函数没有比较 `NumberOfBytesRead`，缓冲只在 worker 启动时清零；转发同样不比较 `NumberOfBytesWritten`，也不向 SDK 发送逐实例结果。客户端 `WriteFile` 返回真后仍读取 `GetLastError`，遇到 232／233／109 会关闭句柄；返回假也关闭。外层丢弃其 BOOL。短读、短写、遗留错误值与下游缺席都必须单独测试，不能用最外层返回值判定整批成功。

当前 XML 的 PID `1364 → {09A2DE6C-101D-4432-BC33-8A8E3337E5E4}`、`3942 → {A3151A1B-9CC6-4189-BD56-369512E2BF7D}`、`3940 → {71FBE710-F299-40D7-B15D-3A87C0A3F385}` 同时指向 `E78202AB-415E-4680-AD16-D4484286B61F.dll`。该插件 `0x11680 → 0x1580` 构造完全相同的实例管道；它虽然读取 `.dat` 中 GUID 并调用 `StringFromCLSID` 校验，却没有把该 GUID 拼到实际管道名。只找到相同 `0x27f78` 读写大小不足以证明路由相同。

该插件 `0x11520` 成功读取后进入 `0x11d30`，也不比较实际读入长度。类型 7 经 `0x120c0` 使用 `.dat` 中逐灯珠映射：每个 16 位项的高字节为行、低字节为列，从帧偏移 `32 + 4×(30×row+column)` 取低三个颜色字节，按产品映射输出连续 RGB；第二映射只在对应产品位启用时处理。类型 6 取帧第 913 个 32 位项作单色；0／9／10 清黑；11 进入 `0x15810` 释放 SDK access 并销毁设备访问对象；没有类型 8 的处理分支。

输出 `0x15a80` 先尝试该插件自己的 JS 管道消费者，否则走设备访问 IPC。后者 `0x15950 → 0x6c80` 建立 `RzDeviceAccess_%04X_%04X` 对应的全局 Mem／Exec／Done／Free 对象；客户端本身也调用 CreateEvent／CreateMutex／CreateFileMapping，故对象存在不证明服务端在线。`0x15690` 先发 `0x10001` 初始化，帧经 `0x7030` 发 `0x10003`，`0x15810` 发 `0x10002` 释放。共享区大小为 `0x20014`，请求命令／长度位于 `+4/+8`，输入始于 `+12`；应答长度／数据／状态位于 `+0x1000c/+0x10010/+0x20010`。取得 Free mutex 和等待 Done 各最多 1 秒，取出真实服务状态再释放 mutex；这比单向 pipe 多了服务完成边界，但本节尚未闭合该服务处理器到硬件的写入／读回。`0x11d30` 仍丢弃帧输出结果，`0x15690` 部分失败分支也返回 0；页面不能据此显示已应用。

设备管理器的发现／加载还包括 `0x31000` 读取 XML 的 HID／USB／COM 产品分支，`0x31c00` 复制实例 `.mod`、调用签名检查、加载并执行 `Init(devicePath)`，只有 Init 返回 0 才登记；`0x32330` 调 GetEID、UnInit、FreeLibrary 后删除实例文件与登记。这里仅核对签名检查的调用及失败分支，没有据此扩写其内部验证能力。这些是官方宿主关系，不是本项目通过插件代理官方 DLL 的方案。

移除实例沿 `0x28a0 → 0x2b30 → 0x2100` 删除客户端；最后一个实例移除时删除相应 GUID 服务器的登记。服务器停止 `0x1990` 先清下游 map，再发 stop event、设置停止位，最多等待 worker 5 秒，但不核对等待结果就继续关句柄。`0x1d30` 的连接事件只在部分 overlapped 分支关闭；插件 `UnInit 0x14e20` 对三个 worker 各等 5 秒也不核对结果。不能把这些有限等待称为所有在途 I/O 与线程已完成释放。

### Camy 专用共享区及直接 HID 分支

当前 XML 将 PID `3867/3868` 映射到 `1B375B0D-1538-4B73-B592-2A73D7D4D8CB.dll`。插件 `0xdb00` 明确区分两者：`3867/0x0f1b` 使用 camy GUID `d678d34a-336c-45e1-af84-e2319a47755c`，`3868/0x0f1c` 使用 `c8ec9711-1b50-41c8-9be2-d9ede0a37a5e`。它们共用插件不代表共用 GUID。

插件 `0xd530` 创建 `Global\{GUID}`、映射并清零 `0x18f798` 字节，再创建 manual-reset 的 `Global\event_{GUID}`；SDK `0x5fb0` 打开的正是这些同名对象。SDK `0x11d90 → 0x62d0` 把类型 7 的 `0xe10` 字节颜色矩阵复制到 `0x27f28` 字节帧的 `+8`，随后发布到 `mapping + 8 + 0x27f28×index`，写末尾时间戳、推进 0–9 下标并 PulseEvent。插件 `0xdb00` 等数据／停止事件，从同槽复制完整帧，只在首项为 `PID | 0x800000` 时解码。这条专用链不经过前述 `0x27f78` GUID 管道。

对类型 7，Camy 把 8×8 矩阵重排为 4 个 segment，每段 16 色，顺序分别为第 7 行→第 6 行、第 5 行→第 4 行、第 3 行→第 2 行、第 1 行→第 0 行；每行取 8 色，输入行跨度仍为 30 个 32 位颜色。每色按低字节、次字节、第三字节复制，不再次交换红蓝。每段形成 88 字节内部命令，命令号 `0x0f03`、body 长度 `0x47`、段号 0–3、起止列 `0/15`，RGB 从 body `+5` 起，命令 `+84` 保存 4 ms 延迟。类型 6 取帧第 903 个 32 位项作单色，0／9／10／11／13 清黑，类型 2 另发 `0x0f02`。这里类型 11 的清黑与通用插件的释放 access 是不同语义。

`0x4a90` 有两个实际输出分支。CJSDevice `0x6d40` 先探测设备 mutex 是否存在，成功后立即 ReleaseMutex／CloseHandle，没有等待取得所有权；随后 `0x7730` 打开按设备路径构造的 JS 管道，`0x7b20 → 0x7840` 写 NUL 结尾 JSON，字段包含 eventId、rowIndex、start、numCol、numRow、regionId、payload、linkDeviceSeqId 和 frame。关键失败边界是：mutex 存在但管道打开失败时，`0x6d40` 仍返回 1，使 `0x4a90` 跳过直接 HID；实际 WriteFile 的 BOOL 也没有传回帧 worker，没有逐帧应答。

直接 HID 分支从系统 Hid.dll 取得 `HidD_SetFeature`，按当前设备路径打开句柄；存在设备 mutex 时最多等 1 秒，获得后先等 2 ms，不存在且错误为 2 时另走无 mutex 分支。每个灯光命令组成 91 字节 report：`[0..1]=0`、`[2]=0x0c`、`[3..4]=0`、`[5..6]=00 47`、`[7..8]=0f 03`、`[9..79]=71 字节 body`、`[89]=XOR(report[3..88])`，其余为零。两条 mutex 分支均调用 SetFeature 后直接按内部命令延迟 Sleep，未检查 BOOL；常规尾部设置 5 秒 idle timer 并返回 1。存在实际 HID 写入不代表该帧已被设备确认。

插件初始化 `0xe920` 限定 VID `0x1532`，创建 CDevice 后还执行 `0x4f80` 的 `0x86` 查询；该查询使用 SetFeature／GetFeature 和独立重试，属于启动读取，不能充当后续灯光帧的读回确认。初始化先 CreateThread 启动帧 worker，随后才创建其 stop event；公开返回 0 没有核对两者是否成功。帧循环按约 30 fps 节流，既不验证 SDK 时间戳，也不生成逐帧序号或设备 ACK。

UnInit `0xeba0` 发停止事件、设停止位，无限等待并关闭两个 worker，随后关闭 stop event、析构 CDevice、关闭共享状态句柄。帧 worker 退出经 `0xe870` 关闭数据事件、Unmap、关闭 mapping 及 namespace／boundary。CDevice 析构 `0x5790 → 0x60b0 → 0x3640` 移除共享设备条目、释放状态 view／mapping，再停止 idle worker、关闭 HID／timer 并析构 CJSDevice；其中 idle worker 只等待 1 秒，不核对结果且未关闭其 thread handle 就清零，不能宣称所有异步资源均已安全释放。

上述三份接收端原件共核对 78 个完整函数、172 个不同 chunk、12,932 条不同地址指令及 44,598 字节原始块，所选 chunk 全部有对应指令字节；SDK 的 5 个发布函数另行复核，不按新增成果重复累计。静态链已从 GUID 发布接到实例插件，并从 Camy 专用帧接到真实 HID 调用位置。没有执行 DLL、服务或设备，不把帧送达、服务完成、设备确认及释放验收合并。

### 键盘／鼠标共享区的实际消费者

当前 XML 将键盘 PID `542/0x021e` 指向 `704A4B8F-0CD6-4D79-BFE7-8B1190E4A85C.dll`，鼠标 PID `105/0x0069`、`114/0x0072`、`115/0x0073` 指向 `2980B2EA-827E-49A6-A545-4C1DA85A6CCB.dll`。两份原件均为 x86、Image Base `0x10000000`。本段仅闭合这些 PID 的 Visualizer 帧路径；键盘 Init 明确按 PID 选择不同 worker，鼠标 PID `104` 还有独立的 mouse／mousepad 双 worker，不能将本段结果扩到共用 DLL 的其他分支。

键盘 `0x1010 → 0xdd20` 打开或创建与 SDK 相同的 `Global\{74164FAD-E73C-4FA1-A9AA-70813315ED9C}`、映射 `0x9c98` 字节；Init `0x17840` 创建数据事件、启用／禁用事件和停止事件，将消费下标设为当前写下标，再为 PID 542 启动 `0x15930`。它在数据事件后从 `mapping + 8 + 2968×readIndex` 复制 `0xb98` 字节并解码，按 0–9 推进直到追上写下标。只有 lighting-enabled 位为 1，且 header 为 `PID | 0x10000`、`0x1ffff` 或 PID 本身时，才进入 `0xecd0`；`0xeb60` 独立监听启用／禁用／停止事件更新该位。读取和共享区存在并不证明当前具有灯光控制权。

双方的 XOR 已核到实际表和分派体。SDK 表位于 `0x31b80`，键盘表位于 `0x23108`，鼠标表位于 `0x132b0`，512 字节逐字节相同；索引取键盘 `frame+2960` 或鼠标 `frame+440` 时间戳的低 7 位，若索引 125–127 则减 3。每个选中的 DWORD 四个字节分别 XOR `table[index]`、`table[index+129]`、`table[index+258]`、`table[index+387]`。键盘实际变换半开区间为 `[72,2956)`，鼠标为 `[36,408)` 及 `[424,428)`；帧头和时间戳不参与。本段核对了 SDK 与两个插件的全部对应 lambda／转调体，不能把整个结构不分字段地 XOR，或只按共享区同名推断编解码一致。鼠标 `0xce20` 的反编译带 bad-SP 提示，表复制、时间戳偏移、参数与分派入口另按完整汇编核实。

键盘 `0xecd0` 对类型 7／17 使用同一分支，Visualizer 的类型 17 因此有实际消费者。它从 `frame + 76 + 88×row` 取六行、每行 22 个颜色，只复制各 DWORD 的低三个字节，形成六个 88 字节内部命令：命令号 `0x0f03`、body 长度 `0x47`、body 前两字节 0、行号 0–5、起止列 0／21、其后 66 字节 RGB。第七个命令为 `0x0f02`、12 字节 body，起始 `00 00 08 00`、其余为零；七条命令各保存 2 ms 延迟，再一次交给 `0x4b60`。类型 11／2 的 `00 00 03 00` 命令走另一分支，不能统一解释为清黑或逐帧释放。

鼠标 `0x1010` 打开或创建 `0x3a28` 字节的同名鼠标共享区；Init `0xef30` 创建数据／停止事件，所选三个 PID 共用 `0xd7d0`。该 worker 从 `mapping + 8 + 448×readIndex` 复制 `0x1c0` 字节，经过 `0xce20` 解码，仅接收 `PID | 0x20000`、`0x2ffff` 或 PID header。类型 8 实际只取帧 DWORD `[56]` 与 `[91]`：分别对应 9×7 颜色矩阵中的 `(row=2,col=3)`、`(row=7,col=3)`。它构造 71 字节 body，前四字节为 0、`body[4]=1`，两色低三个字节依次位于 `body[5..7]` 与 `[8..10]`，其余为零；以 `0x0f03` 提交，再等待 2 ms。这些两区设备没有将 63 色原样逐灯珠输出。

两个插件都先尝试 CJSDevice：键盘 `0x70a0 → 0x7aa0 → 0x7bb0`，鼠标 `0x6490 → 0x7790 → 0x78b0`。两者仅探测 mutex 存在，随即 ReleaseMutex／CloseHandle，没有等待；mutex 存在而 pipe 打开失败时仍返回 1，使上层跳过直接 HID。打开成功后写 NUL 结尾的 JSON，WriteFile 的实际长度未比较，BOOL 未传回帧消费者，没有读取逐帧 ACK。键盘将多行颜色组入一次 JSON，鼠标按当前命令 body 转换；本段只闭合发送者，不把 JSON 写出等同于 CJS 接收服务已经写入硬件。

直接 HID 分支分别为 `0x4b60` 与 `0x48a0`，使用系统 Hid.dll 的 SetFeature、按当前设备路径打开 access 0／share 3 的句柄，并使用与 Camy 相同的 91 字节 report 外壳及 `[3,89)` 字节 XOR。键盘逐条发送六行和提交命令，鼠标发送两色命令；两条 mutex 分支均不检查 SetFeature 的 BOOL，也没有逐帧 GetFeature。键盘汇编 `0x4e1f/0x5027` 调用后直接进入 Sleep。设备 mutex 等待超时、句柄打开失败等分支仍可返回 1；尾部设置 5 秒 idle timer。启动读取 `0x50b0/0x51b0` 则独立执行 SetFeature／GetFeature、核对响应状态及长度；该 getter 的存在不能充当后续帧的确认。

键盘 UnInit `0x17de0`、鼠标 `0xf2a0` 发停止事件并置停止位，对各自 worker 最多等待 2 秒，均不核等待结果就关闭句柄及析构设备。设备析构沿 `0x5930 → 0x6220 → 0x38c0`、`0x5890 → 0x61b0 → 0x3870` 移除共享登记、Unmap／Close 状态区，停止 idle worker并关闭 HID／timer；idle worker 仅等待 1 秒、不核结果且未关闭 thread handle就清零。帧共享区另由全局退出清理 `0x20cb0/0x11d90` 释放，不能描述为 UnInit 本身已经释放全部 mapping，或所有在途工作均已结束。

这两份接收端新增核对 47 个完整函数边界（其中键盘全局初始化仅有汇编）、93 个不同 chunk、9,762 条不同地址指令及 34,310 字节原始块，无未枚举字节；SDK 编解码对应的 16 个函数另作复核，不与此前覆盖重复相加。所选 PID 已从 SDK 共享帧追到真实 HID 调用位置；其他键盘 worker、PID 104 的双输出、CJS JSON 实际消费者、通用设备访问服务的硬件处理器、确认及真实释放结果仍未验证。

## 音频原生消费者

当前 Audvis 原件和完整捕获的字节身份已重新核对。Image Base 为 `0x180000000`。Initialize 创建 CAudioVisualizer 和 MMDeviceEnumerator、注册设备通知；其公开成功标志没有完整传递每一个 COM 失败。StartCapture／StopCapture 的公开返回也会在异常记录后返回 1，不能以此确认实际捕获成功。

录音器按 endpoint ID 或当前名称分支查找；找不到指定端点时取得默认播放端点。另有针对 THX 端点的实际注册表查找，键值为 HpVirtAudDeviceId／HpFallbackAudDeviceId。这里只描述原件这条明确分支，不把它变成产品通用失败规则。切换先停止、析构旧录音器，再分配新对象并调用初始化／启动；切换函数的返回是 mutex 解锁结果，没有传播两个内部调用的 HRESULT。

录音器通过 GetMixFormat 取得并保留 WAVEFORMATEX，按原 nChannels 分配两个各 `512×nChannels×4` 字节的缓冲。录音端点采用 shared 模式，播放端点加 loopback flag `0x20000`。播放初始化失败的特定分支将声道数改为 2，重算 block align／bytes per second；扩展格式同时把 channel mask 改为 3，再次 Initialize。该链没有检查 PCM 位深或 IEEE float subtype，没有通用 PCM 转 float、重采样或 mono 专用分支；Initialize、重试 Initialize 和 Start 的部分返回值也没有被完整检查。

StartAddress 先 CoInitialize，成功才进入录音 worker，返回后 CoUninitialize。worker 持有录音器 mutex，读取 device period，但实际使用独立的 20 ms waitable timer；GetService 取得 IAudioCaptureClient 后 ResetEvent(done)，循环 GetNextPacketSize → GetBuffer → 复制 → ReleaseBuffer。GetBuffer 返回的 SILENT／DISCONTINUITY／TIMESTAMP_ERROR 标志没有参与分支处理。无包时等待 stop／timer，只有 timer 的等待结果继续采集。

复制在独立 critical section 中进行。令 `take=min(剩余 packet frames,512-offset)`，原件实际执行 `memcpy(buffer+4×offset, packet+4×consumed, nBlockAlign×take/nChannels)`，再按 take 更新 offset／consumed；offset 达 512 时切换双缓冲，必要时推进读下标。对 float stereo，单次长度是 `4×take` 字节；这不是按所有声道复制 `take` 个完整交织帧。该公式及后续索引已与汇编核对，不能在说明中替换成标准环形采样算法。

DSP 使用 `N=256`、512 个 float 工作空间及两个 256 项系数表。处理顺序如下；后面输出的 256 项是平滑包络，不是 256 个独立的原始 FFT bin。

1. 仅在读写下标不同、确有新缓冲时处理。`i=0..510` 写入 `work[i]=(src[nChannels×i]+src[nChannels×i+1])×0.5×(300×scale)`；只取前两个声道，最后一个 work 元素未由这个循环写入。
2. 前 256 个 float 乘 Hann 表 `window[i]=(1-cos(2πi/256))/2`。RVA `0x2b70` 只有逐元素乘法及 SIMD／尾部处理，本身不是 FFT。
3. RVA `0x2d20` 调 `0x3020` 完成 bit reversal、radix-2 butterfly，处理 512 个 float；后者最后统一乘 `1/512`，再由前者重组实数频谱。过程在原工作空间内执行，不另行分配 FFT 缓冲。
4. 前 256 个 float 再乘权重；`weight[2j]=weight[2j+1]=2j/256+0.04`，`j=0..127`。随后对 `k=0,2,…126` 求 `sqrt(work[k]²+work[k+1]²)`，上限 1.9，小于 0.1 置 0；仅当大于旧包络时写 `envelope[2k]` 与 `envelope[2k+1]`。实际更新下标为 `0/1,4/5,…252/253`，没有连续覆盖每项。
5. 每次 getter／push 先令全部旧包络乘 `clamp(10×max(abs(decay-10),1),1,100)/100`，再接收新谱峰，最后每 8 项求均值并回填该组。没有新缓冲仍执行衰减与分组平均；UI 的 Decay／Scale 是这些算法参数，不是音量百分比。

旧 GetAudioData 把 256 项格式化为 `%f` 逗号分隔的新字符串；支持 1.0.17.0 API 时，前端配置 40 ms threadpool timer 推送 `{"event":"onAudioData","payload":[…]}`。push 先复制包络到局部数组并构建临时字符串，回调返回后立即释放该字符串，接收方必须在返回前复制。前端收到 JSON 后把 payload 交给 fft_fltr，再由独立像素计时器合成画布。采集、原生 push、像素合成、灯光提交和预览各有自己的计时器。

旧前端轮询函数在空字符串或异常时返回 256 个 0，startCapture 还会在 await 实际调用前设置 isStartCapture；这两项是原前端的缓存／错误行为，不能移作本项目的“已观测静音”或“采集成功”状态。

## 原生内存与资源所有权

| 资源 | 所有者、借用及释放边界 |
| --- | --- |
| WASAPI packet | IAudioCaptureClient 借出；每包复制后 ReleaseBuffer，不能交给异步画布或继续保存指针 |
| mix format、音频双缓冲 | 录音器持有；析构 CoTaskMemFree(format)，释放两个采集缓冲、work／weight／window；普通 Stop 不释放这些缓冲 |
| COM 与捕获线程 | worker 自己 CoInitialize／CoUninitialize，结束释放 capture client；Stop 停止并 Release audio client；录音器／Visualizer 析构各自释放 MMDeviceEnumerator |
| GetAudioData 新字符串 | 设置过 allocator 时用该 allocator，否则 malloc；默认分配路径由 FreeMalloc 释放。不能把宿主 allocator 与默认 free 混配 |
| onAudioData 临时字符串 | push 函数拥有，callback 只借用到返回；SetNodeFFIEvent 的回调指针另有注册生命周期，不能保存临时字符串地址 |
| Chroma JsonApi 返回字符串 | SDK wrapper 另行 malloc 一份 NUL 结尾结果，由该 DLL 的 FreeMalloc 释放；JsonApiNoReturn 销毁内部结果，不转移字符串所有权 |
| Chroma 颜色参数 | 类别消费者本地缓冲由同步 Create 调用复制；当前未请求效果 ID，不建立延后 SetEffect 的页面所有权 |
| Chroma 共享区、管道 | SDK 对象持有 mapping handle／view；析构逐项 UnmapViewOfFile／CloseHandle。事件通知句柄在单次 PulseEvent 后关闭。GUID 管道缓存值另持 HANDLE／名称；写失败及失去控制权的具体分支可关闭句柄，但缓存析构仅回收节点／分配块，不析构这些值，详见下述链路 |

## 停止、错误与释放

engine.stop 先清四个 interval，正在运行时依次停止捕获、发送空帧、发 stop 事件。空帧当前逐设备发普通帧事件，再发空预览；不是 CHROMA_NONE，也不是用批量 native 事件发送。清 interval 没有等待已开始的 async handler 全部返回，不能据此补出旧代次结果已被隔离的保证。

native StopCapture 的顺序是先停止录音器，再取消 push timer。录音器发送 stop event、无限等待 done、关闭线程及两个事件，再取得自身 mutex，Stop 并 Release audio client。它等待的是 done event，没有等待线程 handle；worker 发 done 后仍要解 mutex、CoUninitialize 并退出。done 初始为 signaled，由 worker 获得 capture client 后 reset；CoInitialize 失败或前置 GetDevicePeriod／timer 失败可以绕过正常 done 尾部，不能称每条启动／失败路径均完成同一停止握手。

Terminate 停止并注销 MMDevice 通知，取消 push timer，但没有销毁全局 Visualizer、recorder 或 timer 对象；音频缓冲及格式实际在录音器析构链释放，重新 Initialize 会先 Term、析构旧全局对象再创建新对象。UseAudioDataPushTimer 替换／删除旧 timer 时执行取消、CloseThreadpoolTimer、CloseThreadpool、销毁函数对象并释放 context；该链未调用 WaitForThreadpoolTimerCallbacks。StopCapture／Terminate 的单纯取消同样没有等待在途 callback，不能把它们写成已经完成所有异步资源释放。

Chroma wrapper 的公开 Terminate 调 UnInit，不直接 FreeLibrary；其对象析构才调用下层 UnInit、FreeLibrary 并清函数指针。这份下层 SDK 的 UnInit 发停止事件，对三个线程分别最多等待 10 秒，但没有核对等待结果就继续关句柄、析构对象。共享区发布、取消计时器、函数返回与线程完全退出是不同事实；纯 Rust 所有者仍须按本项目生命周期规则处理取消、在途工作与释放，不能照搬这些未确认边界。

### GUID 管道缓存、控制权与 SDK 退出

本段限定于当前获取收据所列 Core `4.0.2.08241000` 中的 x64 SDK，已复核清单、下载包及两份同字节提取原件。SDK 对象 `+1888` 的 map 保存 40 字节节点：GUID 位于 `+0..15`、管道对象裸指针位于 `+16`、next 位于 `+24`、hash 位于 `+32`。管道对象另外分配 56 字节，保存 GUID、`+16` 的 HANDLE 及 `+24` 的名称字符串；不能把 map 节点与管道值当作同一分配。

即时 GUID 发布 `0x11f50` 先检查控制权／启用位，查不到缓存时分配管道对象，经 `0x2a770` 打开成功才由 `0x17cd0 → 0x21710` 插入。打开失败的分支显式关闭有效 HANDLE、经 `0x28670` 销毁名称，再释放对象；成功插入后的值没有这条局部释放链。`0x17cd0` 覆盖已存在节点时也只是改写 `+16`，不是旧值析构。初始化／rehash 的 `0x1f930/0x27420` 只管理桶和节点链接。

`0x2a770` 用 `StringFromCLSID` 构造 `\\\\.\\pipe\\{GUID}`，已有有效 HANDLE 时返回 0，否则单次 `CreateFileW(GENERIC_WRITE, share=0, OPEN_EXISTING, flags=0)`；本函数没有 `WaitNamedPipe` 或重试循环。失败返回实际 OS 错误。231 分支只判断句柄非空后尝试关闭，不能据此推导它已成功关闭无效句柄。

`0x2a950` 同步请求写入 `0x27f78` 字节，初始化写入计数但不比较它，没有读应答。汇编 `0x2a9e7..0x2aa20` 显示：先保存 `WriteFile` 的 BOOL；即使为 1，仍读取 `GetLastError`，在 232／233／109 时关闭句柄，关闭成功才清零，最后仍返回原 BOOL。因此，遗留错误值可能使“返回 1”和“该 HANDLE 已关闭”同时成立；外层 `0x11f50` 又丢弃该 BOOL。这里既没有逐帧服务消费确认，也没有设备灯光读回。

控制权更新 `0x134f0` 与最终析构是两条不同路径。它在 access 从 0 变 1 时遍历缓存重开，在旧 access 为 1、进入失权分支后才遍历关闭有效 HANDLE，关闭成功才置零；Synapse 在线但 active PID 为 0 时还可能保持旧 access。初始化入口 `0x81f0/0x86f0` 启动三条 worker，分别沿 `0x7060 → 0x140b0`、`0x7070 → 0x14310`、`0x7080 → 0x14600` 监听应用、在线状态和会话。前两条在停止事件分支直接退出，不再调用控制权更新；第三条直接修改 access、按会话事件调用效果恢复，退出只清理自己的监听事件，外层再释放其会话 mapping／view，没有进入 GUID 缓存关闭循环。

`UnInit 0x28d90 → 0x8c20` 先把 access 清零，发送停止事件，并在等待线程前关闭该停止句柄；随后三个 `WaitForSingleObject(10000)` 的结果均未核对，就继续关闭线程句柄。不能把这里的 access 清零视为已经执行 `0x134f0` 的旧值为 1 分支。正常对象析构 `0x7e10 → 0x18e80 → 0x27630` 仅把节点链回 free list、减少 count、释放桶／分配块；没有解引用节点 `+16`，也没有调用管道值的 `CloseHandle`、名称析构或对象释放。构造异常清理 `0x2c3a1 → 0x17da0 → 0x18e80` 使用同一个 map 清理。其他共享区和 private namespace 的 `0x5f00` 释放真实存在，但不是这些 GUID 管道值的释放。

本段独立按 PE 节映射复核 30 个完整函数、30 个 chunk、4,079 条指令及 18,072 字节原始块，另有 27 字节未枚举为指令；EAT／IAT 和上述关键分支也已核对。这证明所选退出链没有释放缓存值，不能扩写为整个进程已经实测泄漏，或所有 SDK 版本均相同。服务端管道路由已由上节的设备管理器／实例插件补充；全部并发／异常、关闭期间仍在途的调用与真实设备输出仍未验证，本批未执行 SDK。

当前前端 terminate 返回 true 并清 nativeDLL 引用，不代表每条底层释放都有独立成功应答。部分 event.on 在 effect／class 挂载时注册但没有对应组件卸载 off；共享观察者也有独立的 beforeunload 清理。本文逐条记录真实清理，不补出统一恢复或回滚。

争用提示读取共享应用状态，提供切换 Chroma 应用／控制权入口。按钮可能启动本地 Chroma 服务、改变本机模式并在 2 秒后重读／刷新页面；这与效果写后读回不同。

## 当前 Rust 页面与输出所有者

本轮从模块入口核对到当前 Shell，尚未找到上述 Visualizer 实际页面的生产挂载。[ChromaPage::modules](../../../crates/razer-app-pages/src/chroma/chroma_page.rs) 将 Visualizer 列为禁用的 Install 按钮，仅为 Studio 发出打开事件；[app_picker_host.rs](../../../crates/razer-shell/src/shell/app_picker_host.rs) 的 launchable／bundled modules 没有 Visualizer，强制提交其 Open 也落入通用“窗口服务尚未连接”提示。入口存在、资源目录存在或共享控件可用均不能计为本页已实现。

| 当前官方实际所有者 | 当前 Rust 对照与后续范围 |
| --- | --- |
| 根 `ac → nc → Jr`，Help 与五面板的选择、Chroma 争用提示和观察者 | 未找到对应页面 entity、五面板布局、全部开关／下拉／程序增删、Help 切换、输入／拖动／焦点和资源状态；不能以 Chroma Dashboard 或设备 Lighting 弹层代替 |
| 所选音源、三态运行策略、画布／预览和多组 timer | 未找到本模块的端点列表／通知、录音 owner、频谱／合成引擎、独立预览与输出状态、应用运行观察及取消／释放。Seiren 的 WASAPI 与音频 meter 是具体产品的另一条已接线链，不能仅因也有采集就认定本模块完成 |
| 原生音频算法及默认八类设备帧 | 本页记录的 DSP、颜色转换、矩阵与本地 SDK 发布边界仍须独立 Rust 实现并连接实际消费者。现有产品／手柄灯光任务不自动覆盖这些类别、帧尺寸、控制权与服务完成契约；禁止把本页官方 DLL 接入生产作为替代 |

本批重新核对当前 UI 获取收据、主入口与实际 lazy 分块的字节身份及完整 AST，并读取五面板 owner。Provider 完整绑定 `ac [248025,248106)` 和其 `nc [247302,248018)` 是两个不同声明，不能把横跨它们的上下文称为一个完整根函数。主页面 `Jr [239449,242301)` 的确由 Help 条件替换设置区，但不会因此卸载根的 engine；这个生命周期也须对应到 Rust 所有者。

后续自动化应分别验证音源列表更新／名称恢复、开关与程序运行、600 ms debounce、slider／文本提交、前景／背景 Off 与 Reactive、实际 DSP 公式、八类帧、空帧停止、迟到通知与完整清理。独立期望使用本页原件与对应原生函数；再经显式入口逐项比较官方实际 DLL／服务结果。没有端点或设备时先测纯算法和模拟传输，实际捕获、帧消费／硬件输出和窗口像素保持未验证。本批没有修改生产代码或运行这些对照。

接收链还须逐项验证 GUID→PID／EID→实例路由、短读／短写、成功返回伴随遗留错误值、下游断开、10 槽环推进、两种 Camy PID、30 列跨度与四段反向行序、固定 91 字节报文及 XOR、CJS mutex 存在但 pipe 失败的分支、服务 1 秒超时、类型 11 的不同消费者，以及停止时仍有阻塞 I/O／idle worker 的释放。期望报文与映射直接取已核实原件；不能用 Rust 编码器生成自身期望，也不能以官方逐帧不返回硬件确认的事实伪造一个确认结果。

## 原件位置

| 所有者 | 当前原件／UTF-8 半开字节区间 |
| --- | --- |
| 入口、Provider、五面板 | [App.0e524954.chunk.js](../../../.analysis/inputs/frontend/synapse-audio-visualizer-current/raw/apps.razer.com/synapse/audio-visualizer/static/js/App.0e524954.chunk.js)，Provider 完整绑定 `[248025,248106)`、根组合完整绑定 `[247302,248018)`、主页面 `[239449,242301)` |
| 音源／运行条件、预览 | 同上，`[170862,176770)`、`[177150,181238)` |
| 前景、背景、属性 | 同上，`[230238,233663)`、`[224547,228439)`、`[228449,230092)` |
| 输入与 slider | 同上，`[184671,186531)`、`[181239,184659)` |
| 应用列表和排序 | 同上，`[166745,167859)`、`[22256,22315)` |
| reducer 初始／消费者 | 同上，`[21231,25954)` |
| action、持久化、参数转换 | 同上，`[94846,102994)` |
| Chroma wrapper／Visualizer engine | 同上，`[109347,117885)`、`[118301,151864)` |
| 设备类别、矩阵尺寸与默认顺序 | [main.53b4ac36.js](../../../.analysis/inputs/frontend/synapse-audio-visualizer-current/raw/apps.razer.com/synapse/audio-visualizer/static/js/main.53b4ac36.js)，模块 3932，`[18783,20440)` |
| Audvis 原生身份、完整伪代码／汇编索引 | [capture.json](../../../.analysis/results/native/b1df319d51ba69a89e7ccff9f74834238b9c3514ca858c849cdcee84696d8ba6/capture.json) |
| 原生公开函数 | 同捕获，RVA `0x58c0/0x5b90/0x5bf0/0x6010/0x61e0/0x6220/0x65b0/0x6620/0x6b50/0x6fa0/0x7a10` |
| 原生枚举／切换／停止／频谱／推送 | 同捕获，RVA `0x3630/0x3db0/0x3fa0/0x4030/0x35c0/0x66e0`；录音器 `0x1820/0x1ad0/0x2400/0x24f0` |
| Audvis 采集线程、DSP 与析构 | 同捕获，RVA `0x2770/0x2b40/0x2b70/0x2d20/0x3020/0xf6a0/0xfda0/0xfdc0/0xff30/0x1630/0x34e0`；`0x24f0/0x2770` 已同时核对完整汇编 |
| Chroma JSON wrapper 身份、完整伪代码／汇编索引 | [capture.json](../../../.analysis/results/native/925f71bdf212aea61b6ba745384a0803510aeeff3f31c136cbbb5892821b2c6f/capture.json) |
| wrapper 解析、分发、初始化／析构 | 同捕获，RVA `0x37f70/0x38040/0x1f30/0x3c60/0x2b70/0x57c0/0x4660/0x4530/0x37b20/0x37cb0/0x14d0/0xbea0/0x37f60` |
| wrapper 类别、颜色与矩阵消费者 | 同捕获，RVA `0x7340/0x7df0/0x8720/0x8fd0/0x96a0/0x9da0/0xa470/0xb020/0xb110/0xb270/0xb3d0/0xb490/0xb5f0/0xb7c0/0xb890/0xb9f0/0xbac0/0xbc00/0xbcd0/0xe830` |
| 下层 SDK 原生身份、完整伪代码／汇编索引 | [capture.json](../../../.analysis/results/native/7ab56ea40c67450f81772cd35c3dae8e36fc2ca43110a2d8294dcac4b2e364e7/capture.json) |
| 下层 SDK 类别、即时应用与 IPC | 同捕获，RVA `0x29c30/0x29de0/0x2a100/0x2a1b0/0x2a2f0/0x2a450/0xab20/0xb6d0/0xbd80/0xbfc0/0xc380/0xc710/0xf310/0xfcf0/0x20f0/0x2300/0x2490/0x2ba0/0x2da0` |
| 下层 GUID、专用帧、管道与释放 | 同捕获，RVA `0x28e00/0x8da0/0xd1d0/0xd410/0xd560/0x11d90/0x11f50/0x62d0/0x2a770/0x2a950/0x28c90/0x28d90/0x8c20/0x7e10`；键盘 vtable `0x31740` 的首项已按原件字节核对为 `0x2300` |
| GUID 缓存、控制权及三条 worker 的本批核对集 | 同捕获，RVA `0x70d0/0x7e10/0x81f0/0x86f0/0x8c20/0x11f50/0x134f0/0x13300/0x13aa0/0x140b0/0x14310/0x14600/0x7060/0x7070/0x7080/0x6e70/0x17cd0/0x21710/0x1f930/0x27420/0x18e80/0x27630/0x17da0/0x2c3a1/0x28670/0x2a770/0x2a950/0x28c90/0x28d90/0x5f00`；与此前集合存在交集，不按批次相加 |
| SDK 包来源、版本与原始下载身份 | [acquisition.json](../../../.analysis/inputs/official-native-current/acquisition.json) 及 [manifest.json](../../../.analysis/inputs/official-native-current/manifests/27f1518bf49220045840c8688b1ca83f47eb38bb8591868bed085a33782bdb68/manifest.json)；Core `4.0.2.08241000`、Devices `4.0.3.09211418` |
| 当前插件及 PID→GUID 映射 | [Devices.xml](../../../.analysis/inputs/official-native-current/extracted/ec6976f2cde35363d9bdd44e802e5b59b3e11cee628119a14402f3637cd6af9c/$PROGRAMFILES/Razer%20Chroma%20SDK/Devices.xml)，实际字节已与获取输入核对 |
| GUID 服务器、实例转发及发现／移除 | [capture.json](../../../.analysis/results/native/e032ca4b3366f84be73803de94373887590f56a1660c5a50262424f243c74060/capture.json)，Image Base `0x10000000`；RVA `0x17f0/0x1890/0x18f0/0x1990/0x1a30/0x1af0/0x1c90/0x1d30/0x1e50/0x1f20/0x1f90/0x2000/0x2100/0x2280/0x2400/0x2640/0x28a0/0x2a00/0x2b30/0x2bf0/0x1b850/0x1b930/0x1b9a0/0x1ba60/0x1bc20/0x1bc70/0x1ec40/0x1e610/0x31000/0x31750/0x318d0/0x31a50/0x31c00/0x32330/0x3c60/0x14110/0x3c40/0x14100/0x5cc0/0x5d80` |
| 通用实例帧解码及设备访问 IPC | [capture.json](../../../.analysis/results/native/58e96aa12a0476a03820add862b3b31e3d6f24b6e63c01855e51fbe80cc3da51/capture.json)，Image Base `0x10000000`；RVA `0x1580/0x10c60/0x10e50/0x112e0/0x11520/0x11680/0x11d30/0x120c0/0x15a80/0x7030/0x15690/0x15950/0x15810/0x6c80/0x14bb0/0x14e20` |
| Camy 共享区、四段重排、HID／JS 输出与清理 | [capture.json](../../../.analysis/results/native/d34b09f03dc71d691f5261c4acf90ac2258d2329786fec3dd77b5926c56cb8ea/capture.json)，Image Base `0x10000000`；RVA `0xe920/0xeba0/0xdb00/0xd530/0xe870/0x4a90/0x4f80/0x5580/0x2c30/0x5790/0x60b0/0x3640/0x2ba0/0x6d40/0x7730/0x7840/0x7b20/0x2400/0x25b0/0x2760/0x2950/0x8900`；GUID 原字节位于 `0x130bc/0x130ac`，pipe 前缀位于 `0x125ec` |

本文已核对六份原生原件的实际字节、完整捕获及以上所述调用路径；格式、缓冲复制、DSP、当前默认八类帧、SDK 发布、GUID 实例转发和 Camy 直接 HID 写入分别记录。颜色拾取器、宿主 FFI 字符串／回调桥接、键盘／鼠标共享区消费、设备访问 IPC／CJS 服务的硬件链、设备写入确认及 SDK 全部版本分支仍需独立闭合。本文没有修改 Rust 或 UI，也未执行音频捕获、灯光输出、厂商 JS 或主应用；静态语义、独立实现、接线、模拟验证及真实验收保持分别记录。
