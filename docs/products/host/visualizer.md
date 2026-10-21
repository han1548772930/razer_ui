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
| Chroma 共享区、管道 | SDK 对象持有 mapping handle／view；析构逐项 UnmapViewOfFile／CloseHandle。事件通知句柄在单次 PulseEvent 后关闭。通用管道按 GUID 缓存，错误分支可关闭／置空；成功缓存的值释放仍需沿各表节点销毁核对，不能以清空表等同句柄已关闭 |

## 停止、错误与释放

engine.stop 先清四个 interval，正在运行时依次停止捕获、发送空帧、发 stop 事件。空帧当前逐设备发普通帧事件，再发空预览；不是 CHROMA_NONE，也不是用批量 native 事件发送。清 interval 没有等待已开始的 async handler 全部返回，不能据此补出旧代次结果已被隔离的保证。

native StopCapture 的顺序是先停止录音器，再取消 push timer。录音器发送 stop event、无限等待 done、关闭线程及两个事件，再取得自身 mutex，Stop 并 Release audio client。它等待的是 done event，没有等待线程 handle；worker 发 done 后仍要解 mutex、CoUninitialize 并退出。done 初始为 signaled，由 worker 获得 capture client 后 reset；CoInitialize 失败或前置 GetDevicePeriod／timer 失败可以绕过正常 done 尾部，不能称每条启动／失败路径均完成同一停止握手。

Terminate 停止并注销 MMDevice 通知，取消 push timer，但没有销毁全局 Visualizer、recorder 或 timer 对象；音频缓冲及格式实际在录音器析构链释放，重新 Initialize 会先 Term、析构旧全局对象再创建新对象。UseAudioDataPushTimer 替换／删除旧 timer 时执行取消、CloseThreadpoolTimer、CloseThreadpool、销毁函数对象并释放 context；该链未调用 WaitForThreadpoolTimerCallbacks。StopCapture／Terminate 的单纯取消同样没有等待在途 callback，不能把它们写成已经完成所有异步资源释放。

Chroma wrapper 的公开 Terminate 调 UnInit，不直接 FreeLibrary；其对象析构才调用下层 UnInit、FreeLibrary 并清函数指针。这份下层 SDK 的 UnInit 发停止事件，对三个线程分别最多等待 10 秒，但没有核对等待结果就继续关句柄、析构对象。共享区发布、取消计时器、函数返回与线程完全退出是不同事实；纯 Rust 所有者仍须按本项目生命周期规则处理取消、在途工作与释放，不能照搬这些未确认边界。

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

本文已核对三个原件的实际字节、完整捕获及以上所述调用路径；格式、缓冲复制、DSP、当前默认八类帧与 SDK 本地发布边界分别记录。颜色拾取器、宿主 FFI 字符串／回调桥接、共享区／管道的服务端消费和设备写入确认、SDK 全部版本分支仍需独立闭合。本文没有修改 Rust 或 UI，也未执行音频捕获、灯光输出、厂商 JS 或主应用；静态语义、独立实现、接线、模拟验证及真实验收保持分别记录。
