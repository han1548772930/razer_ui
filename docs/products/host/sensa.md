# Sensa：触觉来源、设备强度与本机音频配置

本页说明本机触觉混合器：哪些来源参与输出、哪些设备接受输出，以及怎样把音频频段转换成触觉。它同时包含独立应用、设备页嵌入视图和 lighting-engine 后台消费者。页面修改、浏览器存储更新、后台引擎接受和设备应答是不同阶段。

## 页面与状态所有者

独立页依次挂载 Dynamic Haptics、Audio to Haptics Profiles 和 Help。URL 的 `displayMode=chromaApp` 决定容器呈现；同时提供 containerId 和 pid 才进入嵌入视图。其实际查找为 `(deviceContainerId === containerId && productId === pid) || localStoragePId === pid`，取首个匹配项；后一个分支没有再核对 container，不能简写成始终按两字段精确寻址。嵌入视图区分 AUDIO 与其他类别，强度显示值分别使用该分支的应用配置；缺设备时的显示对象不是硬件观测。

根组件读取本机触觉配置、关联游戏、已安装程序、`sensa-devices` 和 `connectedDevices`，订阅 storage、窗口状态和游戏运行事件。只有本机触觉应用配置不存在时才建立应用初始对象；这一条件不涉及 HID 读取失败，也不证明硬件没有配置。

应用初始对象的 version 为 1；Sensa HD 与 Audio to Haptics 各自 enabled=true、gain=4，inGameSession=false。已有配置的版本规范化会增加已有数值版本。设备强度来自设备列表／设备文档的独立字段，不应直接以这份来源配置替代。

左侧来源卡和右侧设备卡分别持有状态。设备卡按产品身份去重呈现；后台设备管理器则使用 productId-instanceId 区分实际实例。这两种身份用途不能混成一份仅按 PID 寻址的设备连接表。

## Dynamic Haptics 的两个来源

Sensa HD 和 Audio to Haptics 各有启用开关与 1–6 的来源增益。修改先派发 reducer，再保存本机 `sensaSource`。Audio to Haptics 的全局开关还同步设备列表。这里的 slider 不含 0；设备卡的总强度 slider 才是 0–6。

沉浸游戏会话中关闭 Audio to Haptics 时，页面显示 Keep On／Turn Off 选择。确定关闭后有该交互专属的 500 ms 事件抑制，再更新会话及设备状态。它不是所有写入失败后的通用弹窗或回滚。

lighting-engine 的 HapticMixerManager 观察存储变化，将来源传到 `SetHapticMixerGain`：sourceId=1 为 Sensa HD，sourceId=0 为 Audio to Haptics。等级转换表按源码顺序为：

| 等级 | 原生增益 |
| --- | --- |
| 0 | 0 |
| 1 | 17 |
| 2 | 33 |
| 3 | 50 |
| 4 | 67 |
| 5 | 83 |
| 6 | 100 |

当前音频捕获原生导出进一步区分两条路径：source 0 将 gain/100 夹在 0–1 并原子保存；source 1 通过本机 Mixer IPC 的 SetProgramVolume 发送四舍五入后的 gain×255/100。后者在 IPC 未初始化或未 ready 时返回 false；其他 sourceId 返回 false。页面保存来源配置并不等价于这两条原生路径都已接受。

## 设备卡、禁用条件与写入目标

支持 standby 的设备若上报 active／standby／off，只有 active 可操作；其他分支检查 isPowerOn===true。重启待生效、standby 和 off 有分别的展示与提示，不以连接列表中存在该项就判定可写。

总强度 0–6，正数表示 enabled；关闭状态的 slider 显示 0。重新开启而原值为 0 时，该交互使用产品表给定值或其分支的 4。Audio to Haptics 复选框有 100 ms 去重；固件待更新、设备非活跃或总强度关闭时禁用。设备行点击会排除内部交互控件；关闭时点击 slider 的位置则按轨道比例计算 round(6×比例)。

设备修改函数先更新 reducer，再比较差异。普通设备构造 `usb_<VID>_<PId或PID>_<container>_mw` 广播频道发送 userCommand 后关闭频道。非音频设备发送完整强度对象；音频分支用 `{hapticIntensity:…}`，强度变化发 `ON_SET_HAPTIC_INTENSITY`，A2H 变化发 `ON_SET_ENABLE_AUDIO_TO_HAPTICS`。该函数返回 true 只表示广播发送路径结束。

DualSense 分支受独立 feature flag 控制，通过本机设备文档更新并比较，最多尝试三次。不能把这份文档重试套用到 HID 广播路径。

两个音频产品的消费者也有真实差异：1383 的 SET_HAPTIC 路径先设置强度，再执行 A2H 调度；1439 对应分支只设置强度。两者 ENABLE 分支均进入各自的 A2H 状态机。不能仅凭共享动作名称补上一条相同的后续任务。

## 配置下拉框与游戏选择

全局预设顺序固定为 Controlled、Balanced、Dynamic、Custom，对应 1、2、3、4。游戏选择下拉先提供继承项和分隔线，再列这四项；具备优化配置及相应版本条件的游戏，才在第二条分隔线后增加 Sensa Immersive Mode。不是字母排序。

选择会立即进入保存链。Custom 保留自己的 band，基预设按固定表重新建立；重置按钮通过 customizedFrom 与当前 band 的比较决定禁用。没有显式配置但存在优化配置时，effect 会自动选择 Immersive，并用 Set 避免重复保存。

游戏查找有两个不同比较：页面匹配使用 exeName.toLowerCase()，specific reducer 更新已有项时比较 exeName 原字符串。文档保留这个差异，不假定所有持久化键均已统一大小写。

specific 动作经 reducer 更新 `sensaAudioHapticProfiles.applicationPresets`。全局输出 UI 还发出 `UPDATE_GENERAL_APPLICATION_OUTPUT`；当前所读 reducer 未发现对应 case，因此不能用这个发出动作本身证明该字段已完成持久化。

## 自定义频段在做什么

图中的横轴是频率、纵轴是增益，编辑的是音频转换规则。SENSA_HD 横轴为 30–8000 的对数坐标；AUDIO 为 20–200 的线性坐标；增益范围 0–100。预设给出的初始可编辑 band 为：

| 类别／预设 | band 范围与增益 |
| --- | --- |
| AUDIO Controlled／Balanced | 20–110，100 |
| AUDIO Dynamic／Custom | 20–200，100 |
| SENSA_HD Controlled | 30–130，70 |
| SENSA_HD Balanced | 30–150，70 |
| SENSA_HD Dynamic | 30–100，30；100–200，10 |
| SENSA_HD Custom | 30–130，70 |

切换基预设时连续派发两次更新：先选基预设，再进入自定义状态。非音频类别可增加第二段或删除第二段；AUDIO 始终只有一段。相邻段的边界限制来自当前其他 band，不允许拖动越过。新增第二段的 gain=50，颜色在 green／cyan 间选择，右边界取第一段右边界加 100 与频率上限中的较小值；第一段已经到上限时，源码可能建立零宽段。

数值输入先保留本地文本；失焦时解析整数并夹入当前范围，Enter 触发 blur。当前输入组件没有 Escape 恢复旧文本的分支。拖图采用 150 ms 的 leading／trailing 调度，拖动结束 flush，卸载 cancel。

band 和预设通过带 saveToLocalStorage 的 action 即时持久化；关闭自定义弹窗不是撤销之前编辑。颜色缺项的补齐是已有本机 band 格式的具体迁移行为，不是设备读取失败后的默认值。

后台把 band 转为原生 profile 的 frequency_range，增益小于 1 时保留，否则除以 100，并复制基 profile 的其余 underlying band 字段。原生 SetHapticProfile 除保存 profile 文本，还解析 volume、transients.volume 与 underlying_bands.volume，再调用 encoder vtable 的实际 profile setter；encoder 非零结果转为失败。

## 游戏会话、捕获与 FFT

HapticMixerManager 初始化设备观察、来源观察、profile 观察及本机体验服务通道。设备 metadata 优先使用 manifest 的 interhapticMetadata，否则请求设备，最多五次；音频强度文档缺项有独立的最多三次、300 ms 间隔读取。它们是当前具体读取链的限制。

游戏开始前保存全局 A2H 和设备 A2H；游戏结束从 priorGameLaunchA2HState 恢复。游戏栈、优化配置、mute／unmute、drop、gain 及体验服务结果均有独立消费者。沉浸模式通过本机 ExperienceService 的 Enable／Disable，要求 response.result===1 才更新 enabled。

当前版本门槛分别处理 mixer 与体验服务：1.0.0.13 用于 mixer 能力，1.0.0.44 决定两种本机消息实现。不能合并成单个“支持 Sensa”开关。

捕获仅在存在活跃触觉设备或 OLED 设备等实际需求时启动。两种 FFT 配置不同：

| 用途 | blockSize | binSize | numBins | sampleRate |
| --- | --- | --- | --- | --- |
| SENSA_HD | 8192 | 2 | 4096 | 20480 |
| AUDIO | 4096 | 5 | 42 | 12000 |

二者 boostLevel=1、logSpectrum=false、enable_mixer=false。前端 FFT hook 通过 KeyStorage 的 `UPDATE_AUDIO_FFT_DATA_TO_UI` 读初值并 register／on；卸载 off／unregister。真正生产者在音频 wrapper 的 FFT 回调，设备列表或频率数据不满足条件时不发布。KeyStorage 的 Electron 分支调用 doKeyStorageAction；非 Electron 分支明确不支持。

音频捕获顺序为 AcquireAV → Start → 注册／处理频率与 FFT 事件；停止先清 FFT、注销，再 Stop／ReleaseAV。软件触觉帧还需 enableAudioHapticMixer、设备 enabled／power／软件处理等条件，不能把所有 FFT 频率直接广播成设备帧。

AcquireAV 原生先解析 endpointId；空值返回 false。已存在同端点实例时直接返回 true，不重新应用随后才读取的配置。新实例读取 capture、blockSize、boostLevel、numBins、binSize、logSpectrum、sampleRate、period_ms、period_ms_fft、saveCapturedData、enable_mixer，创建 RzAV 并按实际初始化结果加入端点管理器。现有实例命中与新实例初始化必须分别理解。

Start 的底层返回取自 recorder start 状态，成功时设置实例 running 标志。Stop 的 recorder 路径通知停止事件、等待完成事件、join 工作线程、关闭两个句柄，并在锁内停止／释放音频客户端及 CoTaskMem 格式数据；返回 0。不能把包装层 bool 与底层 HRESULT 混用。

## 清理与确认边界

后台 stop 逐设备注销；若 OLED 仍需要音频且不是 force，则保留捕获。restart 先强制停止再启动。unload 注销 storage／设备／音频事件，取消调度，释放本机消息订阅、module 和 connection。

原生 Terminate 另清理端点 map、回调管理器、队列与锁相关对象；ReleaseAV 移除指定端点及共享所有权。页面代码中日志虽写 retry once，但对应 catch 本身没有第二次调用；也存在 beforeUnload 与 beforeunload 不同拼写，均按实际代码理解。

本页没有执行页面、原生库或真实设备。已读前端、后台 manager、音频 wrapper 及下表原生函数的静态语义；FFT／haptic 回调内部编码与全部产品的最终传输应答仍需沿其各自消费者继续核实，不能以当前页面保存成功替代硬件确认。

## 本机 Synesthesia 停止链与结果消费者

当前服务原件内还有一套调试 HTML。它的按钮、HTTP 控制、provider 共享内存和 mixer 消费者已能沿完整函数定位；这不证明独立 Sensa 应用调用了该调试入口。各入口的命令与完成条件分别如下。

| 入口／传输 | 已核实的动作 | 当前结果消费者与边界 |
| --- | --- | --- |
| lighting-engine helper 调用 | `simpleLaunchRazerAppNoWait` 启动安装路径下的 `SynesthesiaStop.exe`，参数为空 | 当前保留包没有该 helper 原件；不能从文件名、服务停止函数或 HTTP 表推导 helper 的实现、返回与退出屏障 |
| 服务内嵌调试页面 | Stop 按钮调用 `cmdThenRefresh('stop_synesthesia')`，POST `/command`，body 为 `cmd=` 加编码后的名称 | 先读响应文本，再于 400 ms 后刷新 `/mixer`；没有检查 `response.ok`，HTTP 错误响应也可能触发刷新 |
| 调试 HTTP 控制 | 原始命令表把 `stop_synesthesia` 映射为 `HandlerProc(0x87)` | handler 正常返回后发送 HTTP 200、文本 `OK`；响应不携带 provider、事件发送或硬件读回结果 |
| SCM 服务控制 | `HandlerProc` 的 1／5 分支标记 STOP_PENDING 并通知服务主循环 | 这是整个服务的停止入口；不能把其服务控制码或完成状态替换为 `0x87` |
| Wyvrn 事件 IPC | 文本 `stopSynesthesiaProcess` 解码为事件值 6；当前接收线程在该分支结束当前 batch | 本次没有证明该分支调用 `0x1943f0` 或形成 helper 退出屏障，仍须沿事件所有者继续核实 |
| provider → mixer V1 IPC | `ProviderClean` 先发送程序级 `StopProgram`，命令值为 2 | mixer 按名称移除程序记录并发布本地状态通知；发送方没有等待 ACK，程序记录移除也不是设备已停止的读回 |

调试 HTTP 只绑定 `127.0.0.1`。服务模式的主入口仅在可执行文件目录存在 `debug.flag` 时启动监听；文件中的合法 `port=1…65535` 可改端口，其余分支使用 8787。GET `/` 或 `/index.html` 从 PE 的 RCDATA 101 读取 HTML 并填充日志行。该入口没有证明属于正式页面的挂载树，不能据此增加另一套产品配置界面。

POST handler 在 body 中查找第一个 `cmd=`，把其后全部字节与控制表精确比较；没有 URL 解码、按 `&` 分割或去空白。`cmd=stop_synesthesia` 命中，尾随换行、追加参数或编码后的命令名均不能按同一名称命中；未知或空命令返回 HTTP 400。响应函数调用 `send` 后没有检查其结果，不能把构造了响应等同于客户端已收到响应。

`HandlerProc(0x87)` 的顺序为停止 Synesthesia、清零服务内 `synesthesiaRunning` 标记、写日志。停止函数 `0x1943f0` 依初始化标记清除事件接收和 HAR 计算线程的运行位，按线程 ID 等待两者结束，再遍历已加载 provider，最后关闭并清空 `SynesthesiaLaunched` 事件句柄。对自身线程执行 join 或 join 失败会抛出 C++ 错误；后续清理没有因此自动成立。provider 遍历器通过导出名获取 `ProviderClean`；缺导出或返回 false 时记录日志，仍会执行 `FreeLibrary` 并清空模块项。它没有将这些失败回传 HTTP 调用方。

GET `/mixer` 的 `synesthesiaRunning` 直接来自上述服务字节，`mixerRunning` 来自另一个服务字节。调试 HTML 的 `updateCtrlBadges` 只消费这两个布尔字段。因此 badge 显示 STOPPED 仅说明其读取的服务标记；它不证明 provider 全部清理成功、共享内存命令已消费或实际硬件停止。

`Interhaptics.RazerProvider` 的停止发送链使用两个独立命名对象：

| 对象 | 完整名称 |
| --- | --- |
| 共享映射 | `Global\SharedMemory_InterhapticsMixerCommand-{fc3bf7a7-5224-48d6-8c5d-d5173ca09737}` |
| 事件 | `Global\Event_InterhapticsMixerCommand-{ee4d3efa-297b-40f1-81c9-ecaff6295f02}` |

provider 在 IPC ready 时映射 151 字节；程序名来自已设置的应用名或该函数的 `Native SDK` 分支。编码器清零 `[0,128)` 并复制最多 127 字节名称，`byte[128]=2`，`u64[136]=0`。当前 sender 的完整汇编没有写入 `[129,136)`、`[144,151)`；这些字节保留共享映射的原内容，不能自行补成全零报文并宣称与原件一致。编码成功后调用 `SetEvent`，不检查其结果，随后解除映射、关闭本方事件和映射句柄并清 ready 位；映射失败也进入句柄清理，没有发送成功的确认结果。

mixer 创建的 V1 映射大小为 152 字节、事件为自动复位；接收循环先映射 152 字节读取偏移 136 的长度，再按长度加 151 重新映射。当前包同时有 V2 ring 初始化与消费路径；本次 provider 的 `OpenFileMapping`／`OpenEvent` 明确走 V1，不能把 V2 的契约代入。接收回调由原始 vtable 槽定位至 `0x226e0 → 0x1f4b0 → 0x50b0`；mixer 未运行时，外层回调不进入命令分派。命令值 2 使用 StopProgram 的解码与执行接口，程序名解码后进入 `0x23220`：

1. `0x264f0` 以长度与字节比较程序名，在程序记录序列中移除首个精确匹配项，移动后续记录并释放尾项；同名项并非在这里一次全部删除。
2. `0x263d0` 从余下程序记录中选择满足当前启用、未静音及音量条件的首项；没有满足项时返回空名称。
3. `0x1fe50` 处理所选名称与当前状态，必要时进入本地通知队列。执行函数随后返回 true；外层接收回调不把该返回值作为应答写回给 provider。

`ProviderClean` 在尝试上述发送之后才检查自身初始化标记；未初始化时返回 false。正常初始化分支逐项调用 HAR 的 `RemoveConnectedDevice`，清理设备管理器及共享引用，再清空 API 对象并调用其析构槽，最终清初始化标记并返回 true。以下进一步区分记录移除、代理析构与真正持有传输／回调的对象生命周期；本地通知队列的后续消费者及最终设备停止／应答仍需继续追踪。

### HAR 记录与 API 引用的释放

provider 的实际导入槽 `0x57020` 指向 HAR 的 `RemoveConnectedDevice` 导出 `0x67e80`。这个导出没有设备虚表派发：它在 HAR 的 32 字节记录序列中寻找首个匹配 ID，释放该项偏移 24 的缓冲，移动后项并缩短序列。它没有向调用者返回成功布尔值，也没有在本函数直接执行设备 I/O、发送停止帧或等待设备应答。

移除前还可能调用 HAR 的全局函数指针 `0xa2080`。`RegisterDeviceCallback 0x67d00` 只赋值这个指针，`RemoveDeviceCallback 0x67d10` 只清零；该指针不是 vtable。本次所核服务、provider 和 API 调用链尚未证明其注册者与内部动作，不能把未命中注册入口推成整个项目没有注册，也不能把这个可选回调等同于下述 API 的设备事件回调。

API 的 `RzInterHapticsProxy` 另有两类共享引用：设备信息代理，以及 provider 注册的 `DeviceManager` 回调对象。释放均受实际引用计数约束；清掉一份引用不必然立即销毁对象。已定位的最后一份强引用释放链为：

| 所有者 | 完整静态释放链 | 已确认的结果 |
| --- | --- | --- |
| proxy 设备表项 | `0x663c0 → 0x66520 → 0x66590`；设备信息控制块 vtable `0xfaa98` 的销毁槽 `0x68470` 调实际对象析构 `0x66e80` | 释放设备信息内含的触觉代理引用和名称；控制块自身按弱引用计数另行释放 |
| 触觉设备代理 | 控制块 vtable `0xfaa70` 的 `0x68480` 调对象 vtable `0xfa9f8` 的析构 `0x66f90` | 该析构只处理名称／字符串和对象内存，没有调用设备停止、关闭或 RPC 完成函数 |
| provider 设备引用列表 | `ProviderClean → 0x30540` 逐项减少共享引用 | 最后一份引用才沿对应控制块销毁；枚举函数 `0x67150 → 0x676d0 → 0x65f00` 证明当前这些代理的创建来源 |
| provider 回调对象 | `RzInterHapticsProxy` 析构释放其回调引用；provider 控制块 vtable `0x64378` 的 `0x305c0` 调 `DeviceManager` 析构 `0x2e9e0` | 清理设备记录树、观察者列表及映射；记录析构 `0x2d240` 再减少其中的触觉设备代理引用。单次引用释放不代表回调已排空 |

`DeviceManager` 的设备事件与 `ProviderClean` 也有区别：其 vtable `0x643c0` 的新增／移除槽为 `0x2c540`／`0x2d040`。移除事件消费者会先通知其观察者，按设备名称找记录，再逐 bodypart 调 HAR 删除、调用 HAR `RemoveConnectedDevice` 并移除本机记录。`ProviderClean` 的当前遍历不能被改述为已经执行了这一整条事件消费者。

### 下层传输与回调的生命周期

proxy 构造的两份回调分别绑定 `0x677a0`／`0x67a00` 与 proxy 地址。实际调用槽 `0xfaac0 + 16 → 0x68490` 从绑定对象读取该地址和函数，再调用它；这里没有为 proxy 地址自动建立共享所有权。两个函数分别通过 proxy 所持有的回调对象进入 `DeviceManager` 的新增／移除槽，设备事件来源与 HAR 的单一全局函数指针仍须分开。

下层对象由全局 API 初始化函数 `0xae80` 持有：仅在全局实例为空时，模式 0 建立 `RpcClient`，模式 1 建立 `InterHapticsFFI`。proxy 自己保存该实例的地址。两种对象的注册槽和释放顺序不同：

| 下层所有者 | 回调安装与存储 | 该所有者的释放链 |
| --- | --- | --- |
| `RpcClient` | vtable `0xf8d20 + 56 → 0x3e930` 通过 `0x3c280` 复制并替换偏移 56、120 的两份函数对象 | 析构 `0x3cf20 → 0x3be10` 先通知退出事件，按初始化情况 join 两个线程，再释放 RPC 字符串／binding 和线程对象，最后由 `0x8a60` 释放两份回调。join 仍有自身线程及失败异常分支 |
| `InterHapticsFFI` | vtable `0xfa398 + 56 → 0x5c670` 复制并替换偏移 8、72 的两份函数对象；新增回调非空时还会在锁下复制现有设备列表、解锁后同步调用它 | 析构 `0x59fb0 → 0x59200` 关闭自身事件，释放持有的记录／引用、容器和锁，再释放函数对象；当前这段不能套用 RPC 的两个线程 join 契约，也不能据其析构名称推导硬件停止 |

正常 proxy 析构 `0x67060` 仅释放自己的设备表、锁和共享回调引用，没有在该函数调用下层回调注册槽、全局 `Terminate` 或底层对象析构。因此“proxy 已释放”不能作为“下层回调已注销、无在途回调”的证据，也不能在没有执行证据时直接声称发生了实际悬垂。

全局 `Terminate 0x10000` 才会先清空全局 API 指针、析构原实例，再清理本 DLL 的另一个 RPC 服务对象。`DllMain 0xfd90` 的进程分离分支调用它。provider 是否触发了最后一次 DLL 卸载、其他调用方是否仍持有对象、宿主操作收束与设备通信确认，仍需按相应生命周期继续核实；不能把进程分离清理提前算成每次 `ProviderClean` 都已完成。

上述服务到 provider／proxy 的复核集合按原件身份和 RVA 去重，为服务 32、provider 23、API 40、HAR 3 个完整函数；下述 FFI／RPC 续追的函数另列于原件表。逐项核对捕获索引、raw chunks 与已列指令的实际 PE 字节，未列为指令的 chunk 内容原样保留。控制表、上述实际 vtable 槽及 HTML 内四个完整 JS 函数的 AST 边界同时核对。这里的函数数量只表示静态复核范围，不表示整个 DLL 或所有下游分支完成。包提取的 11 个文件不含 `SynesthesiaStop.exe` 和 `Interhaptics.HidProvider.dll`；完整包提取仅证明该包的内容，不能证明当前宿主获取闭包已完整。实际 DLL、服务、helper、设备写回与硬件停止对照仍未验证。

### FFI 深层记录的实际清理

`InterHapticsFFI` 的成员析构现已追到具体内容，不能把其中每个共享引用或容器都视为设备连接：

| FFI 对象偏移 | 来源与完整成员释放链 | 实际作用 |
| --- | --- | --- |
| `+576/+584` | 构造 `0x581a0 → 0x51b60("RzInterHaptics.FFI")` 返回日志对象与其控制块；`0x59200` 减少该引用 | 这是 `spdlog` 日志对象，不是 HID／RPC 设备句柄。日志引用释放不代表传输关闭 |
| `+512` | 设备表 `0x58840 → 0x58cc0 → 0x58e90 → 0x59180 → 0x588c0 → 0x58d30 → 0x58f00` | 释放表的桶、设备 ID、metadata、属性名称／值字符串及节点。已核成员链没有调用停止帧、宿主设备移除回调或设备应答函数 |
| `+368` | 命令表 `0x58940 → 0x58da0 → 0x58fc0` | 释放命令名称和绑定对象；其 vtable `0xfa948 + 32 → 0x602d0` 仅在对象分配于堆时释放绑定对象内存，不向绑定的 FFI 对象发送取消 |
| `+616` | `0x58e10 → 0x26fa0`，按每项的 JSON 类型标签析构 | 释放 JSON object／array／string／binary 内容与节点；不是已经证明具有停止／排空契约的工作队列 |
| `+608` | 构造用 `CreateEventA(0,0,0,0)` 建立；析构先 `CloseHandle` 并清零 | 这里关闭自身事件，不能套用 `RpcClient` 的退出通知与两个线程 join。该调用的返回值也没有转成设备停止确认 |

JSON 清理的子链 `0x26fa0 → 0x14420/0x25e60` 分别处理对象树与数组，字符串和二进制分支释放其缓冲；`0x14420 → 0x22730` 继续释放键及 JSON 值。其临时容器扩容 `0x13dc0/0x14bb0` 中的分配失败／长度检查也只是容器失败语义，不能把异常处理函数名当作设备或在途传输的取消证据。这里闭合的是成员内容和内存所有权；实际设备／服务写回仍须沿输出消费者另行确认。

### FFI 的宿主回调与注销边界

实际向宿主输出的两类函数由导出 `SetNodeFFIEvent 0x2eb80` 和 `SetNodeFFIEventWithResult 0x2ec50` 安装。两个导出都先检查实际函数指针非空、全局 API 存在且可转换成 FFI 对象；任一条件不满足即返回 0。尤其是传入真实空指针不会清除旧回调，不能把它作为注销入口。安装成功的返回值仅证明函数对象已保存。

| 类别 | 安装／存储 | 实际调用与释放 |
| --- | --- | --- |
| 无返回内容的宿主事件 | `0x2eb80 → 0x5f8f0 → 0x61870`，保存于 FFI `+136`，有效函数对象指针在 `+192` | `0x5a010` 将 JSON 序列化后同步调用 vtable `0xf8a10 + 16 → 0x24ad0`，后者直接转调保存的原始宿主函数。销毁槽 `+32 → 0x24a80` 只按堆标志释放函数包装内存 |
| 有返回内容的宿主事件 | `0x2ec50 → 0x5f9b0 → 0x619d0`，保存于 FFI `+200`，有效函数对象指针在 `+256` | `0x5a110` 分配 4,097 字节缓冲，以 4,096 的容量调用 vtable `0xf8a48 + 16 → 0x24ae0`；后者直接向原始宿主函数传入输入字符串、输出地址与容量。销毁槽 `+32 → 0x24a90` 同样只释放包装内存 |

`0x59200` 先释放 `+264/+200/+136` 的三个函数对象，最后由 `0x8a60` 清理 `+8/+72` 的设备事件回调。上述包装销毁不调用宿主注销函数，也不等待宿主已经发起的异步操作。`0x5a010/0x5a110` 本身是同步转调；输入 JSON 字符串在调用返回后释放，有结果的缓冲在复制成返回字符串后释放，不能把这些借用地址当成宿主持有的长期资源。没有函数时不发送，或者返回空字符串，也不等于设备已执行命令。

命令绑定由构造 `0x581a0` 通过 `0x512b0 → 0x615b0` 保存 `AddDevice`／`RemoveDevice`／`Notification` 的成员地址与 FFI 原始地址；复制 `0x602a0/0x61040` 没有为这个原始地址增加共享引用。命令表销毁因此只结束绑定包装的生命周期。原始 vtable `0xfa948 + 16` 指向 `0x604c0`，原完整捕获索引没有该地址的独立函数；相邻 chunk 为半开区间 `[0x60420,0x604bf)`，下一函数为 `0x604d0`。随后[定点补查收据](../../../.analysis/results/native/cc2a78f8c9aeae01a5b2afb1ddac301b2fee11ca2be07824715060aadfe1f92e/supplements/e5d88ab5500bd5f3/capture.json)从实际六槽表独立解码该入口的完整 11 字节，边界为 `[0x604c0,0x604cb)`：

| RVA | 原始字节 | IDA 静态解码与数据流 |
| --- | --- | --- |
| `0x604c0` | `48 8b 41 08` | `mov rax,[rcx+8]`，取得已保存的成员目标 |
| `0x604c4` | `48 8b 49 18` | `mov rcx,[rcx+24]`，改为绑定的 FFI 原始对象 |
| `0x604c8` | `48 ff e0` | `jmp rax`，尾调用目标，保留 RDX／R8 中的 JSON 输入与输出地址 |

构造分别将 `0x5abd0/0x5b770/0x5b640` 和自身地址装入这三个绑定，`0x5a970` 通过 slot2 调用后消费处理函数的返回码。这闭合了所选命令的实际间接转调，包装本身没有增加共享引用、异步等待或设备确认。补查也保留分析器限制：IDA 能将末条原字节解码为三字节 jump，仍把其冗余 REX 前缀分成数据项，函数化失败；私有处理器显示选项的尝试没有解决，完整 Hex-Rays 伪代码仍未取得。补查保持 `capture_complete=false`，没有改写原捕获／原数据库，也没有用其他模板实例或跳过前缀的伪代码冒充完整结果。

已捕获的具体处理函数 `JsonApiRemoveDevice 0x5b770` 会校验 `devices` 为数组、将其中字符串 ID 转为小写，在设备表锁内删除实际存在的记录并释放上述名称／metadata／属性，解锁后将实际删除的 ID 列表同步通知 `+72` 的设备移除回调。它和 FFI 析构的单纯清表不同。`JsonApiNotification 0x5b640` 当前完整函数只序列化输入并写日志后返回 0；不能把此返回值当作触觉设备已应用通知。命令分发表与绑定调用器的所选转调已静态核实；整个宿主异步完成、全部异常与真实设备结果仍须另验。

这些宿主事件、API 的设备事件回调，以及 HAR 的全局回调是三套机制。当前服务包所含 PE 的实际导入表中，服务和 provider 对 HAR 的导入集均不含 `RegisterDeviceCallback`／`RemoveDeviceCallback`；这只界定这些文件的静态导入，不能排除其他插件、序号／动态解析及尚未获取的依赖注册 HAR 回调。其真实注册者仍未证明。

### AddDevice 的输入、登记与读取消费者

当前 API 原件的 `JsonApiAddDevice 0x5abd0` 已沿前述命令绑定追到完整处理函数 `[0x5abd0,0x5b640)`。它先将整批输入解析到临时设备表并建立通知 ID 列表，随后锁住 FFI `+432`、逐项修改 FFI `+512` 的登记表，解锁后才调用可选的设备新增回调。正常返回 0 所对应的是这些步骤；本函数不构造真实设备应答、硬件读回或 JSON 输出对象。

| `devices` 输入 | 处理函数的实际行为 |
| --- | --- |
| 字段不存在 | 返回 `0x80070490`，不进入临时表／全局提交 |
| 字段不是 object 或 array | 返回 `0x80070057`，不进入全局提交 |
| object | 每个键按原件逐字节 `tolower` 生成设备 ID；值交给 `0x5c0e0` 解析。值不是 object 时仍生成空 metadata 和空 properties，不因这一点拒绝登记 |
| array | 每个元素必须是 string；非字符串抛出 nlohmann type_error 302。字符串 ID 转小写，对应记录的 metadata／properties 均为空 |
| 空 object／array | 仍经过锁和可选通知路径，通知列表长度可为 0；不能把它解释为一次真实设备发现 |

`0x5c0e0` 只接受精确字段名 `metadata`／`properties`：metadata 只有在值为 string 时才复制，内容在这一层不作为设备描述 JSON 解析；缺少或类型不同保留空字符串。properties 只有 object 分支被遍历，只有 string 值进入属性表，其他值被忽略；属性名称同样逐字节转小写，属性值保持原字符串。这里的字段过滤、字符串保存和空记录均不是设备查询结果。

每个小写设备 ID 先进入通知列表，再经 `0x53150 → 0x59930` 放入临时表。相同小写 ID 的后续遍历项整条替换该临时记录，通知列表没有去重；属性键的大小写碰撞也由后续遍历的字符串值覆盖。object 经过当前原件的树迭代器 `0x629a0/0x632f0/0x2fd30/0x3c960`，不能擅自用 JSON 文本的书写顺序代替其实际遍历顺序。通知列表和最终唯一记录表因此是不同数据。

全局提交不是交换整张临时表。锁内 `0x5b35f` 再次调用 `0x53150`，已有节点直接复用，缺节点时可能分配／扩容；`0x5b36e → 0x59650` 移入 metadata，`0x5b383 → 0x62c00` 先清空旧 properties，再移入临时属性表。缺少 metadata／properties、非 object 值以及 array 登记都会按这些规则替换旧记录，不能改述为保留旧字段或合并属性。已有 ID 也进入新增通知，不限于首次插入的设备。`0x5b412` 解锁后，`0x5b461` 才同步调用保存于 FFI `+8` 的函数对象；参数是通知 ID 的指针／数量，回调没有设备确认返回值。

登记记录的查询入口也已核到实际消费者：

| 入口／完整函数 | 读取内容与结果边界 |
| --- | --- |
| FFI 枚举 `0x5a300` | 在同一锁下复制登记表的设备键，并与调用者输出 vector 交换；正常返回 0，捕获异常后返回 `0x80004005`。这是登记表枚举，不是 HID 扫描，遍历也没有规定稳定排序 |
| 属性 `0x5a590` | 设备 ID 和属性名都转小写，再查设备表及记录 `+80` 的属性表；任一项缺失返回 `0x80070490` 且不改写输出，找到才复制已保存的字符串。该函数没有向设备发起查询 |
| metadata `0x5a570 → 0x5d9f0` | 设备缺失返回 `0x80070490`；已存 metadata 非空时直接复制。为空且 WithResult 函数存在才生成 `GetMetadata` 并同步取得返回字符串；两者均无时返回 0 且不改写输出。锁覆盖查表及这次同步宿主调用，不可套用“先解锁再通知”的新增回调顺序 |
| 信息／触觉代理 `0x67690/0x67100/0x675d0/0x675b0` | 分别复制已存名称、取得已有触觉代理共享引用、转调下层属性槽、转调下层 metadata 槽。`0x676d0 → 0x65f00` 构造代理时仅保存下层地址和名称，没有在构造中打开设备或确认其设置 |

FFI vtable `0xfa398` 的边界为 64 字节、8 槽；紧接的 `0xfa3d8` 是 `RzInterHaptics.FFI` 字符串。信息代理 `0xfaa28` 为 32 字节、4 槽，触觉代理 `0xfa9f8` 为 40 字节、5 槽，API proxy `0xfaa50` 为 24 字节、3 槽；各表后方的 RTTI 不能并入方法槽。当前宿主只安装无结果事件回调，因此 metadata 为空时不能仅凭 renderer 有同名分支就断言这里必然取得设备描述。

### AddDevice 的异常清理与外层返回

当前原 PE 的 `.pdata 0x13773c` 将 AddDevice 绑定至 unwind `0x1157f0`、`_GSHandlerCheck_EH4 0xdc7f4` 和 FuncInfo `0x11581c`。FuncInfo 的 header 为 `0x08`，没有本函数 catch 表；原件 unwind map `[0x115825,0x1158a2)` 有 19 项，IP map `[0x1158a2,0x1158f3)` 有 37 项。按这些原始表及清理动作核对，失败发生的阶段会影响全局表是否已经改变：

| 失败位置 | 已核异常清理与已发生的修改 |
| --- | --- |
| 临时解析／字符串与属性分配 | 清理局部字符串、迭代器、通知 vector 和临时设备表；尚未进入全局提交 |
| 锁内提交 `0x5b35f/0x5b36e/0x5b383` | EH state 15 先经 `0x4acd0` 解锁，再经 state 1 的 `0x3bd90` 清通知列表、state 0 的 `0x59120 → 0x58840` 清临时表。清理动作没有恢复全局旧记录；先前已完成的项目不能视作整批回滚 |
| 解锁后的新增回调 `0x5b461` | 此时为 state 1，只清通知列表及临时表；全局提交已走完。通知失败或其下游异常不能被解释为登记从未发生 |

这里按 EH4 ABI 解读当前 PE 的原始表，ABI 头只作格式辅助；该 PE 经 `0xddd98`／导入槽 `0xf6650` 调外部 `__CxxFrameHandler4`，不包含其完整压缩表读取器。本段没有声称已取得本 PE 内不存在的 CRT decoder，也没有执行 CRT、注入异常或证明实际发生过部分提交。`0xef270/0xef280/0xef2a0/0xef2d0/0xef310` 及 `0x589c0/0x59140` 的完整清理函数分别释放临时桶、缓冲、属性表及受状态位保护的字符串，不能据此补造设备恢复或业务回滚。

导出 `JsonApi 0x2e880` 的实际返回尤其不能从反编译主函数是否显示 catch 来判断。它用 `0x207d0(output,0)` 初始化 JSON null，经 `0x5a970` 得到处理函数返回码，**仅当输出已经是 object 时才写入 `HRESULT` 键**。AddDevice 不修改该输出；因此正常完成、缺少 devices 或 devices 类型错误等返回码分支，在序列化及分配成功时都返回装有四字符 `null` 的 C 字符串，不能期待它们自动变成 `{"HRESULT":...}`。`0x274f0 → 0x27820` 的 null 分支确实写入这四个字符，`0x24350` 用 malloc 复制后交调用者，配对释放为 `FreeMalloc 0x2e860`。

外层异常另有明确原件绑定：JsonApi 的 try 表 `0x109ead` 覆盖 states 0–12，handler 表 `0x109eb5` 指向 catch funclet `0xe5430`，继续地址为 `0x2eaab`，这里清零 EAX 后返回 **C 空指针**。`JsonApiNoReturn 0x2ead0` 的 try 表 `0x10a068`／handler 表 `0x10a070` 则指向 `0xe54c0`，继续到 `0x2eb73`，不返回 JSON 也不传出处理函数返回码。这些是由原表与完整 funclet 汇编共同确认的 catch 分支，不能将 JSON 文本 `null`、C 空指针、内部 HRESULT 或设备成功合为一种结果。API 未初始化／不是 FFI、被捕获异常和返回字符串分配失败均可能得到 C 空指针；后两类可发生在全局表已修改之后，空返回不证明无副作用。

### 新增通知在 proxy 与 provider 中的实际作用

`0x677a0` 收到新增 ID 后，在 proxy 自身锁下逐项建立信息代理、替换 proxy 设备表引用，解锁后才调用其持有的 DeviceManager 新增槽。它捕获该路径的异常，没有把 DeviceManager 的结果交回 AddDevice。实际 provider vtable `0x643c0 + 8` 指向 `0x2c540`：空列表返回 `0x80004005`；非空时先同步通知观察者，再逐设备按名称查本机记录。已有名称直接跳过登记，所以 FFI 同名记录被替换并再次通知，不代表 provider 重新解析了其 metadata 或重建 HAR 记录。

新名称分支经信息代理 `+16 → 0x67100` 取得触觉代理，再经触觉代理 `+8 → 0x675b0 → FFI +8 → 0x5a570` 取 metadata；返回码为负时清理本项后继续。成功才将字符串交 HAR `AddDevice`（provider 导入槽 `0x57050`，调用 `0x2c7d0`），随后解析 `Bodypart`／`BodypartID` 和字符串 `DeviceName`；名称经 `0x2c3e0` 在当前映射中作子串匹配，无匹配返回 `0xffffffff`。之后调用 HAR `AddConnectedDevice`（槽 `0x57060`，调用 `0x2cc0e`）及 `GetFrameSize`（槽 `0x57038`，调用 `0x2cc2b`），把返回的 ID／帧长、bodypart 列表及代理引用存入 provider 记录；符合静音记录条件时另调用 `MuteBodyparts`。

本函数把上述 HAR 返回值保存到记录，没有在此逐项验证为真实设备确认；HAR AddDevice 已发生之后，后续 metadata 类型／解析异常也不能从局部字符串和共享引用清理推成 HAR 已恢复。AddConnectedDevice 还具有前述先调用全局可选回调、再修改 HAR 记录的顺序。provider／proxy／FFI 的三个表、观察者通知、HAR 记录及最终硬件状态分别核对；当前这条登记链没有补出尚未证明的设备写入、读回、请求确认或恢复消费者。

这一段的复核集合为 API 54 个完整函数及 provider 5 个完整函数，共 59 个 raw chunks、5,865 条已列指令。捕获索引 SHA、23,662 字节 raw 和 23,320 字节已列指令逐项匹配当前原 PE；其余 342 字节保留原始内容，不补造指令。该集合包含复用上下文，不能与前面的 85／33 个函数相加。原始 vtable、三套 EH 绑定／原表与实际调用点另行核对；本批没有运行厂商代码、服务、设备或主程序。

独立 Rust 测试及官方实际 DLL 对照须分别覆盖字段缺失／错类型、非 object 设备值、空列表、大小写碰撞与重复通知、整条覆盖旧字段、解析前失败、提交中异常、通知后失败、输出分配失败、JSON `null` 与 C 空指针、WithResult 缺失时输出不变、同名 provider 记录不刷新及 HAR 返回值保存。对照还须记录三个登记表的实际前后状态和清理，不能只比较返回文本；真实设备应答、原值恢复与释放仍为未验证。

### RemoveDevice 的实际删除名单与异常边界

`JsonApiRemoveDevice 0x5b770` 的完整边界为 `[0x5b770,0x5c040)`。缺少 `devices` 返回 `0x80070490`，不是 array 返回 `0x80070057`；array 内只处理 string，**非 string 元素直接跳过**，不能套用 AddDevice 数组分支的类型异常。字符串逐字节转小写，先全部复制到输入 ID vector；完成解析与日志后，再为实际删除通知预留空间，随后锁 FFI `+432`。这里的输入名单和真正删除名单是两个容器。

锁内逐 ID 用 `0x466d0/0x524f0` 查 FFI `+512` 的登记表。缺项跳过；找到才改桶与链表、减记录数，经 `0x58e90` 释放 key／metadata／properties 并释放节点，**之后**才在 `0x5bd55 → 0x6a50` 复制该 ID 到通知 vector；扩容分支为 `0x5bd6d → 0x308c0`。重复小写 ID 在第一次删除后即查不到，不能按输入次数生成移除事件。通知 vector 虽预留了元素空间，长字符串复制仍可能分配失败；当前节点可能已删除却尚未加入通知，先前删除也没有整批恢复动作。

`0x5bd84` 解锁后，`0x5bdd1` 才同步调用 FFI `+72` 的移除函数对象，其有效对象指针在 `+128`，参数是实际删除 ID 的指针与数量。只要函数对象存在，空输入、全为非 string 或全未命中也可发送空名单；它没有设备应答返回值。本处理函数同样不构造 JSON 输出对象，前述外层 JSON 文本 `null`、C 空指针与内部 HRESULT 的区别继续适用，不能据返回文本推断每个设备的删除情况。

异常绑定已按原 PE 的 `.pdata 0x137754`、unwind `0x1158fc` 与 FuncInfo `0x115924` 核对。header `0x08` 没有本函数 catch 表；unwind map `[0x11592d,0x115958)` 为 7 项，IP map `[0x115958,0x11598b)` 为 22 项。锁内复制通知的调用处为 state 6：先由 `0x4acd0(frame+0x70)` 解锁，再以 state 5 的 `0x3bd90(frame+0x50)` 清通知、state 0 的 `0x3bd90(frame+0x38)` 清输入。解锁后回调处已是 state 5，不再做锁清理。解析阶段的 `0xef3c0/0x8300` 只释放受状态位保护的临时字符串；这些动作没有将已删记录重新插回。

### 移除通知经过 proxy 与 provider 的顺序

proxy 的完整移除消费者 `0x67a00` 先确认 DeviceManager 回调引用存在，预留信息代理通知 vector，再锁自身 `+32`。它经 `0x654a0 → 0x64980` 查 proxy `+112` 表，命中就增加信息代理共享引用并加入通知；这两个查找函数只比较 hash／长度／字节并写查找结果，**本消费者没有删除 proxy 表项**。`0x67b8c` 解锁后，`0x67b9d` 才调用 DeviceManager 的 `+16` 移除槽，随后 `0x667f0 → 0x64430` 释放本次通知 vector 的共享引用。释放这些临时引用不能改述为 proxy 表已经清空或信息代理已经销毁。

这一路的 catch 也有原表依据：FuncInfo `0x118984`，try 表 `[0x1189a1,0x1189a9)` 覆盖 states 0–2，handler 表 `[0x1189a9,0x1189b1)` 指向空 catch funclet `0xf1140`，继续地址 `0x67bac`。锁内 state 2 先解锁，再清临时 vector；DeviceManager 回调处为 state 1，只清 vector。异常被本层捕获，不作为 RemoveDevice 的设备结果传出，也没有重新登记 FFI 项或修复 provider／HAR 状态。

provider 的实际移除槽 `0x2d040` 对空代理列表返回 `0x80004005`；非空时先在 `0x2d084` 同步通知观察者，随后才按代理名称修改自己的记录。其完整顺序为：

1. 按名称经 `0x300e0/0x301b0` 查记录；不存在则跳过该代理。
2. 用记录 `+112` 的 HAR device ID，遍历 `+80..+88` 的 bodypart ID，逐项调用 `DeleteBodyPart(deviceID,3,bodypartID)`（`0x2d11a`）。
3. 再次按名称查表；不存在时进入 `"invalid map<K, T> key"` 异常，而非补造一条记录。存在才将 `+116` 的 connected-device ID 传入 HAR `RemoveConnectedDevice`（`0x2d162`）。
4. 再次取得名称，经 `0x2fce0` 求相等键区间、`0x30d80` 实际移除 provider 树节点。单项分支 `0x2ab60` 完成摘链／树平衡与减计数，`0x2d240` 释放 bodypart vector、减少触觉代理共享引用并清名称，最后释放节点。

观察者通知先于上述删除，三个名单不能视为同时发布。provider 的 FuncInfo `0x686f0` 没有本函数 catch 表；其 `[0x686f9,0x68700)` unwind map 只有通过 `0x1ddf0 → 0x1e390` 清临时名称的动作，未登记恢复先前 bodypart、HAR connected-device 或 provider 节点的动作。后续异常可能使当前链停在已经通知／已经部分删除的阶段，最终由 proxy 的上述 catch 收束。正常返回 0 也没有逐项消费 HAR 的设备确认。

### HAR bodypart 删除的实际结果与清理

HAR `DeleteBodyPart 0x67a50` 的完整 chunk 为 `[0x67a50,0x67b73)`，不是首条 `.pdata` 的短区间：原表 `0xa6ef4/0xa6f00/0xa6f0c` 将它分成三个连续区间，后两段的 chained unwind 均回到 `0x96310`，没有为本导出增加业务 catch。全局对象 `0xa2088` 为空时直接返回；非零 device ID 按整数键查 HAR 表，找不到就结束。device ID 为 0 时沿设备链遍历，但在某次内部 `0x2eb40` 首次返回 true 后，不再对剩余设备调用删除；不能把这一分支描述为必然从所有设备删除同名 bodypart。

provider 传入的第二参数 3 由 `0x2f1f0` 选择当前存在的三个 120 字节子对象：偏移 `0/120/240` 的 optional 标志分别在 `112/232/352`。`0x2eb40` 对每个选中对象调用 `0x29370`，**不消费其返回值**，而是在调用后将自身标志置 true；因此 true 只说明至少处理了一个子对象，不能证明目标 bodypart 存在或已从所有子对象删除。选中项在使用时 optional 无值会走 `0x28e00` 的 `bad_optional_access` 分支；导出本身没有把这些内部结果转成成功／失败应答。

`0x29370` 找不到 bodypart 时返回 false；找到才先复制该项的嵌套数据，随后清对应 112 字节槽的启用标志，摘除 hash 节点并释放其内容，再由 `0x29af0` 重算本机列表。该重算从末项向前构建当前项列表，再逐项重建后续列表，经 `0x2b760` 移交 vector；这里核到的是内存中的索引与列表，没有新增设备写入或读回。复制分配失败发生于本项摘除前；重算若在已摘除后失败，异常清理仅释放局部副本／临时 vector，未登记重建旧 hash 节点或撤回先前子对象删除的动作。

这些异常状态也已核到当前原表：`0x2eb40` 的 FuncInfo／unwind／IP 为 `0x902ac/0x902b5/0x902bc`，清选中对象指针 vector；`0x29370` 为 `0x8f678/0x8f681/0x8f688`，清局部嵌套副本；`0x29af0` 为 `0x8f75c/0x8f765/0x8f76b`，经受状态位保护的 `0x7c660 → 0x209b0 → 0x21230` 清临时整数 vector。这三个 FuncInfo 都没有本函数 try/catch 表。之后的 `RemoveConnectedDevice` 仍按前述先调用可选全局回调、再移除首个 connected-device 记录执行；该回调的真实注册者与最终硬件停止仍未证明。

本段复核 API 27、provider 13、HAR 28 个完整函数，共 68 个 raw chunks、4,319 条已列指令；捕获索引 SHA、15,947 字节 raw 和 15,686 字节已列指令逐项匹配当前原 PE，其余 261 字节保留原始内容。集合含前段复用上下文，不能直接累加完成度。独立 Rust 测试与官方实际 DLL 对照须分别覆盖混合类型数组、大小写重复、未命中／空通知、删除后通知复制失败、proxy 表仍保留、观察者先通知、HAR ID 为 0／不存在、子对象存在但 bodypart 缺失、重算中断、部分删除及局部清理；记录 FFI／proxy／provider／HAR 各自的前后状态与事件顺序。本次只做静态核对，没有执行厂商代码、注入运行时异常或进行硬件停止／原值恢复验证。

### RPC 工作线程与本地服务的结束

`RpcClient` 的两个线程现已闭到实际等待点，均使用构造 `0x3b7d0` 建立的手动复位退出事件 `+288`：

| 线程 | 创建与消费者 | 退出与失败语义 |
| --- | --- | --- |
| 设备事件轮询 | `0x3f900 → 0x30e40 → 0x3ca50`；线程对象在客户端 `+184` | 每次以 10 ms 调 `WaitForSingleObject`，仅 `WAIT_TIMEOUT` 时继续轮询；其他结果离开循环。连接有效时调用 RPC stub `0x69760`（procedure 7），将返回内容送 `0x3dd20`，随后释放 RPC 返回缓冲。解码器只将有效的 `DeviceAdded`／`DeviceRemoved` 与数组 `payload` 中的字符串交给对应同步回调 |
| 等待服务出现 | `0x3fc80 → 0x30e80 → 0x3cb10`；线程对象在客户端 `+208` | 等待 `Global\RazerInterhapticRpcSyncEvent` 和 `+288` 两个事件；仅结果 0 标记连接并进入注册 `0x3e980 → 0x69d20`（procedure 0）。自身退出事件、失败及其他非零等待结果不进入该注册分支；随后复位并关闭本线程建立的命名事件。安全描述符／事件创建失败只记录错误后返回 |

析构 `0x3be10` 的实际顺序为 `SetEvent(+288)`、按线程 ID 是否非零分别 `0x41c00` join `+184/+208`，再由 `0x3cf60` 释放 RPC binding 字符串与全局 `Handle 0x1315a8`，释放线程对象和设备事件回调。正常 join 成功才证明本对象的轮询线程及其中同步回调已返回；10 ms 是两次 RPC 调用之间的等待时间，不是正在执行的 `NdrSendReceive` 的超时或取消。当前这条退出链没有把退出事件转换为正在执行的 RPC 调用取消，因此不能宣称停止在 10 ms 内完成。

`0x41c00` 和线程对象清理使用的 `0x41c60` 对无效线程 ID、自身线程 join、底层 join 失败分别抛出 C++ 错误 1、5、2，只有成功才清空线程对象。`0x3c0c0` 在仍持有线程时先对其停止状态 `0x40270` 发出停止请求再 join，按引用计数释放停止状态；最终仍有线程 ID 则 `terminate`。这些失败分支不能报告为正常释放。析构没有消费 `SetEvent`、`RpcStringFreeA` 和 `RpcBindingFree` 的失败状态来形成外部成功／失败结果；`+288` 事件本身的配对关闭也未在所核正常析构链中证明，不能用关闭命名同步事件替代这一项。

轮询异常消费者 `0xea6a0` 在仍连接且 ping 返回 1722 时清掉连接标志、释放 binding、重建连接并重新启动服务检测线程；`0x3f630` 主伪代码没有展示全部异常路径，完整汇编还包含保存异常码、去重记录日志和返回异常码的分支及 `RpcExceptionFilter` chunk。RPC stub `0x69760` 的异常清理 chunk 调 `NdrFreeBuffer`，与轮询拥有并释放的返回内容缓冲是不同资源。恢复、缓冲清理和退出不能合并成一个“断开即停止成功”状态。

同一 DLL 的 RPC 服务对象另由 `StartRPC 0xff60 → 0x4a250` 建立，和客户端两个线程不是同一个所有者。`Terminate 0x10000` 先清空并析构全局 API，随后清空并析构这个 RPC 服务对象。后者 `0x4ae10 → 0x4cda0` 在启动标志为真时先 `RpcMgmtStopServerListening(0)`，成功后才 `RpcServerUnregisterIf(0,0,0)`；任一步失败记录错误并返回 0，只有两步成功才清启动标志。析构不检查这个布尔返回，仍继续 join 自身线程、释放记录表和锁。

服务线程 `0x4a250 → 0x46730 → 0x4c9c0` 在调用 `RpcServerListen` 前，由 `0x4cfa0` 打开命名同步事件并尝试 `SetEvent`，随后关闭这份句柄；打开失败只记录错误。这份通知先于实际 listen 结果，不能作为监听成功确认。`RpcServerListen(1,1234,1)` 的第三参数 `DontWait = 1`；注销的第三参数 `WaitForCallsToComplete = 0`。因此 join 这个启动线程不能证明全部 RPC runtime 调用已排空。启动、停止／注销失败和全局 API 先于 RPC 服务析构的真实顺序必须分别测试，不能据 `Terminate` 的函数名补成“先停止接收、等待全部在途调用、最后释放 FFI”。本批仅静态核实这些实际分支，没有执行官方 DLL 或制造并发故障，不能据此断言已发生运行时悬垂或硬件错误。

后续自动化与实际 DLL 对照须据这些独立期望覆盖：FFI 空指针注册被拒且旧函数不被清除、包装释放与宿主操作完成分开、设备移除实际名单与析构清表分开、RPC 等待失败／退出事件／同步回调在途、join 自身线程及失败、ping 异常与重连、NDR 缓冲释放、服务停止与注销分别失败，以及非等待注销后的真实调用收束。真实硬件停止、原值恢复与读回仍须各自提供实际收据，不能由此静态生命周期核对替代。

上述 FFI／RPC 续追与复用上下文共复核 85 个函数、90 个 raw chunks、7,122 条已列指令；捕获索引哈希、29,262 字节 raw 内容和已列指令均逐项匹配原 PE。已列指令覆盖其中 28,905 字节，剩余 357 字节保留原始内容，不补造指令或行为。三张宿主／命令绑定 vtable 各按 48 字节、6 槽核对；后续 RTTI 或字符串不并入槽位。此集合含前述复用函数，不能与上文 98 个函数简单相加；另列 `0x604c0` 的 11 字节静态解码不计作新增完整 IDA／Hex-Rays 函数。

### HAR 回调时机与插件加载者

HAR 的 `AddConnectedDevice 0x67b80` 在全局 HAR 对象、输入数组和正数数量检查通过后，先同步调用可选的 `0xa2080`，之后才分配／复制 bodypart 数组、增加记录 ID 并插入 32 字节记录；输入不满足条件返回 9999。`RemoveConnectedDevice 0x67e80` 同样先调用该回调，再查找目标 ID 和删除记录，目标不存在也可能已经调用过。两处调用均不传新旧设备 ID。这个回调既不是“已添加”通知，也不是“已删除”确认；只有回调返回后，当前记录操作才继续。

`RegisterDeviceCallback 0x67d00` 的完整函数仅写指针，`RemoveDeviceCallback 0x67d10` 仅清零。HAR 当前完整捕获中该全局槽的实际访问已核到这两处写入和上述两处调用，没有从导出名称推导额外注册行为。服务的完整启动函数 `0x4e790` 在建立 `SynesthesiaLaunched` 手动复位事件并初始化 HAR 后，依次调用 `0x4d980` 尝试加载 `Interhaptics.HidProvider.dll` 和 `Interhaptics.RazerProvider.dll`；事件已存在则关闭本次句柄并返回 0。

插件加载器 `0x4d980` 按传入文件名调用 `LoadLibraryA`，仅对精确匹配的 Razer provider 尝试 `ProviderSetApplicationName`，随后对各插件解析并调用 `ProviderInit`。初始化返回 false 才进入已核的 `FreeLibrary` 分支；缺少库或导出进入各自日志分支，不能合并为加载成功或已完成释放。上述两个完整服务函数没有安装 HAR 全局回调，但插件内部仍是独立边界。当前服务包的完整提取清单没有 `Interhaptics.HidProvider.dll`，其原件及调用链尚未取得；不能猜它就是注册者，也不能因当前服务／provider 的静态导入未命中就宣布此回调无人注册。真实注册者及其动作继续保持未确认。

### 当前宿主的回调所有者与退出动作

当前宿主 ASAR 中，`preload.js` 的 `doDLLMainAction`／`doDLLMainActionAsync` 分别通过 `ffiPreload`／`ffiPreloadAsync` IPC 进入主进程 `FFIPreloadMain.callDLL`／`callDLLAsync`；lighting-engine 模块 4407 的 `yL`／`oE` 与这两路对应。Sensa 的这条链不是同文件中另外的子进程 FFI 通道。

`FFIPreloadMain` 按 channel 分别持有 `ffiLibMap`、`ffiCallbackMap` 和调用互斥锁。`ConfigureFFI` 遇到已有 channel 时只维护 URL 列表并报告已经加载，不重新加载。`_handleAction_SetNodeFFIEvent` 已有 channel 回调时复用或跳过，返回 true；新建时把真正的 native callback Buffer 传给 DLL，再保存到 `ffiCallbackMap`。保存不以 DLL 返回 true 为条件。因此 renderer 参数中的 `actionArgs: null` 并没有传成 native 空指针，也没有注销旧回调；它进入的是宿主专门创建回调的分支。

该回调解析 JSON，在 channel URL 列表匹配且 WebContents 存活、未崩溃、未 disposed 时调用 `send`：`event` 分支发送完整对象，`events` 分支发送数组。没有目标窗口或 JSON 解析／发送失败不会形成设备成功应答。`FreeFFI` 在同步／异步分发中直接返回 true，另有同名空 async 处理器；所核完整类也没有移除 `ffiCallbackMap` 项的路径，不能把这个 true 当成注销、卸载或队列排空。

调用互斥锁覆盖的是对应 channel 的宿主调用。`callDLLAsync` 等待 native async 返回，不等待 renderer 事件消费者；其 `_callDLLFunctionAsync` 的回调把结果参数用于 resolve，没有把同次 native error 参数转换为 Promise rejection，外围捕获还会记录错误后返回未定义值。这些行为与设备完成是不同边界。`ConfigureFFI_APIToCallWhenExit` 只登记动作字符串；`callApiWhenExit` 遍历登记项并逐项等待 `callDLL`。主进程完整 `quitAppFinal` 先等待子进程退出动作，再执行主进程设备退出、shutdown 和登记的 exit API；这没有建立 renderer 触觉任务集合的 join。

### 从 native 回调到设备调用的不同完成点

lighting-engine 的完整 `initRzInterHaptics` 按版本声明 `JsonApi`、`JsonApiNoReturn`、`FreeMalloc`、`GetDLLVersion`、`StartRPC`、`SetOperatingMode` 和 `SetNodeFFIEvent`，另在 1.0.0.1 分支加入 `Terminate`。配置后顺序为模式设置、安装无结果回调、`StartRPC`、登记退出 `Terminate`；当前完整 wrapper 没有声明或安装 `SetNodeFFIEventWithResult`。模式／注册调用被 await 不代表其返回被核对：该初始化未逐项检查这些调用的结果，正常走到末尾就返回 true。

| 完成点 | 当前完整消费者 | 实际证明的范围 |
| --- | --- | --- |
| native 输出函数返回 | API `0x5a010` 同步转调宿主；node 的同线程／跨线程桥见下节 | 宿主本次同步 JS 调用已经返回；不证明 renderer 已消费 |
| 主进程事件发送返回 | `FFIPreloadMain` 创建的 callback 只解析、筛选窗口和 `webContents.send` | 事件已交 IPC；不等待页面工作或设备应答 |
| renderer listener 返回 | wrapper 注册 `response("RzInterHapticsCallback", e => { ... })`，只调用 `interHapticsCallback(e)`；HapticMixerManager 安装的回调按 `enableAudioHapticMixer` 门禁调用 manager | 两层均不 return／await 下层 Promise，不能把 listener 返回当成命令完成；门禁为 false 时根本不进入 manager |
| 设备方法的 Promise | `onRzInterHapticsCommands` 按小写 device ID 查本地映射，仅将 `sethapticmode`／`gethapticmode` 路由到对应方法并在内部 await | setter／getter 结果仅在这一层可用；truthy 结果触发的 `sendNotification` 没有被 await，不能从外层 IPC 得到该完成点 |
| 数据分块提交 | `SetHapticData` 分支调用 `sendHapticData`，后者按最多 64 字节 payload 分块，前缀为逐块递减的剩余块数，尾块不显式填充 | 命令分支没有 await；分块循环也没有 await `setInterhapticsStreamFrame`。同步 try/catch 不覆盖这些未等待 Promise 的后续 rejection，也没有在这里给出设备读回确认 |

Native sender 的实际分支与这些消费者必须一起解释：

| API 函数 | 已核输入／结果与输出路径 |
| --- | --- |
| `RegisterSender 0x5c870` | 保存 sender PID／名称，调用可选的 sender 回调，再经 `0x5a010` 输出 `NewSender`；renderer 对此分支只记录日志 |
| `SenderSetHapticData 0x5e1f0` | 空数据返回 `0x80070057`；小写设备 ID 在 FFI 设备表中不存在则返回 `0x80070490`。存在时把 sender PID、设备 ID 与字节数组放入 `SetHapticData` 事件，经 `0x5a010` 后返回 0；这里没有等待真实设备写入或读回 |
| `SenderSetHapticParam 0x5ece0` | 将设备 ID／参数名转小写，缺设备返回 `0x80070490`；存在时增加 FFI `+604` 的 request ID 并发送含参数值的 `SetHapticParam` 事件，之后返回 0；本函数没有匹配 `Notification` 并等待其结果的步骤 |
| `SenderGetHapticParam 0x5ce20` | 同样检查设备表；**仅有结果回调 `+200` 非空才构造 `GetHapticParam` 并调用 `0x5a110`**，再由 `0x59650` 将返回字符串移入调用者输出。该回调为空时正常返回 0，却不改写输出字符串。当前已核的 Sensa 宿主链只安装无结果回调，不能因 renderer 有同名 switch 分支就说本 native 路径必然发出该查询 |

`interHapticsJsonApi` 会 await native `JsonApi` 调用，但丢弃返回内容；其 Add／Remove 包装也不检查结果。Manager 在 Add 调用返回后保存本地设备映射，在 Remove 调用返回后删除映射。因此“映射已登记／已删除”、native 表的实际修改和硬件状态也须分别核对。`Notification` 当前仍落到前述仅序列化／日志的 `0x5b640`，不能补成与 request ID 关联的 native 设备确认。

### node 跨线程队列、回调 Buffer 与取消边界

当前宿主依赖 `ffi-napi-rz` 的 `CanUseAnneCallback()` 返回 true，Sensa 注册又明确请求 `useNewCallback: true`。完整 `Callback()` 保留 CIF Buffer，并将用户函数的同步返回直接交给 `ref.set`，没有 await Promise。对应 `ffi_bindings.node` 原件的完整函数已追到实际等待／唤醒和 finalizer：

| 所有者 | 完整静态链 | 已确认的行为与边界 |
| --- | --- | --- |
| NAPI instance／事件循环 | 分配／注册 `0x7250`；初始化 `0x6d80`；consumer `0x93e0` | `0x7250` 分配 328 字节 instance，经 `napi_set_instance_data` 注册 `0x7400` 为结束回调，并导出绑定 `0x6d80` 的 initializeBindings；后者记下所属线程 ID，初始化 `instance +104` 的 `uv_async`、`+24` 互斥锁并 unref，Callback 导出绑定 `0x94c0` |
| callback closure | `0x94c0` | 保存 JS invoke 和错误报告的 NAPI 引用、instance 地址及 Anne 模式；libffi trampoline 进入 `0xa0f0`，返回的 callback Buffer 注册 `0xa440` finalizer。这与 DLL 内的函数包装分别拥有资源 |
| 同线程调用 | `0xa0f0 → 0x8c20 → 0x23a0` | 在模块退出标志未置位时，同 instance 线程直接同步进入 `napi_make_callback`；没有 Promise 完成检测 |
| 其他线程调用 | `0xa0f0` 入队、`uv_async_send`、`uv_cond_wait`；`0x93e0 → 0x8c20` 后 `uv_cond_signal` | native 线程等到所属事件循环处理本项并结束同步 JS 调用；之后才销毁该项的条件变量／互斥锁并释放内存。这里没有超时或取消 token；唤醒不证明 IPC 消费或设备任务结束 |
| 参数与错误 | `0x8c20 → 0x8830/0x8580`；JS `errorReportCallback` | Anne string 分支按 NUL 终止长度建立 JS UTF-8 字符串，没有把 API 临时输入地址暴露为长期字符串借用；错误经同步错误报告或抛出分支处理。JS 错误报告使用 nextTick 抛错，也不能充当设备取消或恢复 |
| callback Buffer finalizer | `0xa440 → 0xa480 → 0x87d0 → 0xd060`；另有释放 thunk `0xd800` | 删除两份 NAPI 引用并释放 libffi closure／内存；`0xd060` 是分配器回收与合并／VirtualFree 链，不是设备或宿主任务的取消。上述 finalizer 不执行 API 注销、等待在途调用或排空工作队列 |
| instance 结束 | `0x7400` 在条件 `+120 == 1` 时调用 `uv_close(...,0x2750)`；后者进入 `0x7b20` | 关闭回调置模块退出标志，销毁 instance 锁并释放队列容器与 instance；`0x7b20` 本身不执行排队项目、不向等待线程发送条件信号。此处尚未证明整体退出已收束全部在途项；不能据静态顺序直接断言实际发生死锁或悬垂 |

Renderer 的完整 `handleBeforeUnload` 会等待当前音频捕获停止，注销 OLED media event、订阅／模块／连接，移除列出的窗口／音频／设备 listener，并取消 immersive debounce；**该函数没有撤销 `setRzInterHapticsCallback`、保存／调用 `response` 返回的移除闭包，或 join 命令 Promise 集合**。`preload.response` 确实返回 removeListener 闭包，但 Sensa 这次注册没有保存它。这个结论限定于已核完整所有者，不能扩大成全项目绝无清理。

这次追加共复核 33 个完整 native 函数、33 个 raw chunks、6,014 条已列指令：27,186 字节 raw 与捕获索引及原 PE 匹配，指令覆盖 27,097 字节，另 89 字节保留为未列指令的原始内容。宿主 5 个 packed JS 成员逐字节匹配 ASAR 及其完整性条目；unpacked node 的打包前内容按当前获取索引记录的签章差异重建后匹配 header，未将此当作 Authenticode 签名校验。完整 JS 均经 AST 解析并核对下列所有者边界；没有执行厂商 JS、DLL、应用或设备。

这些分支的自动化与官方实际功能对照须分别覆盖 HAR 回调早于记录修改、无匹配记录、callback 重复注册／native 注册失败后的持有、无目标窗口、事件门禁、native 返回与延迟 renderer／设备 Promise、64 字节边界与尾块、同步异常与异步 rejection、WithResult 缺失时输出未改写、Add／Remove 返回内容未消费、退出时排队与在途项，以及 finalizer／native 注销／设备恢复的不同所有者。静态捕获、队列入队、IPC 发送和本地映射变化均不能替代实际 DLL／设备结果；这些运行对照与真实清理仍为未验证。

## 当前 Rust 入口与能力核对

当前 [ChromaPage::modules](../../../crates/razer-app-pages/src/chroma/chroma_page.rs) 固定列出 Sensa HD，但 Install 按钮禁用且没有 Sensa 打开事件；[app_picker_host.rs](../../../crates/razer-shell/src/shell/app_picker_host.rs) 的实际可启动模块也不含它，强制 Open 进入“窗口服务尚未连接”通用分支。本批未找到独立 Sensa 根、Dynamic Haptics、Audio to Haptics Profiles、嵌入视图或上述完整 mixer／profile／游戏会话链的 Rust 生产所有者，不能以模块图标、注册名称或其他设备自己的触觉 slider 判断本应用已接通。

| 官方链 | 必须对应的独立实现与页面行为 |
| --- | --- |
| 独立根与设备嵌入 | 根拥有本地配置、device／memory／storage／游戏事件，完整挂载两页／Help／引导；嵌入根按实际 predicate、类别与强度字段选择对象。当前没有该页面树、选择／焦点、说明、禁用原因和释放消费者 |
| 来源与设备强度 | 来源 1…6 和设备 0…6 的单位／身份／确认分别处理；Source gain、本地设备修改、真实设备写入、会话关闭弹层和 A2H 的 100 ms／500 ms 门禁不能合并为本地布尔保存 |
| 预设、游戏与自定义 | 四项顺序、继承／Immersive 条件、两次 action、band 范围、150 ms 调度、关闭即时保存和前台／游戏恢复分别实现；未知字段及失败不能伪装成默认已生效 |
| FFT、mixer 与输出 | 按实际端点和两套采样参数拥有捕获／DSP／profile setter／本机 IPC／设备输出与应答；旧会话释放后才能切换。底层官方 DLL 仅作分析和显式对照，产品仍需纯 Rust 实现 |
| Synesthesia 停止与释放 | helper、调试 HTTP、SCM 和事件 IPC 分别追踪；服务线程停止、provider 返回、151 字节发送、程序记录移除、状态通知与硬件确认分别建模。不能用 HTTP 200、badge 或服务标记替代真实设备停止确认 |

当前嵌入 arrow `[953726,954755)` 和独立根 function `[954765,959571)` 均已按完整 JS 的 AST 边界重新核对，真实设备卡 arrow 为 `[814058,817579)`；获取收据、入口、manifest 和当前部署原件的实际字节匹配且前后未变。来源／设备交互的合并区间 `[832287,841235)` 只是多个所有者的定位上下文，不能将它当成单个完整函数或以其存在宣称所有页面状态已核完。

后续自动化须覆盖实际设备寻址／同 PID 多实例、active／standby／off／restart-required、未知观测、来源与设备不同强度、会话 Keep On／Turn Off、预设与 band 边界、真实读取与草稿不同、异步取消和恢复／释放。通过显式对照入口再核官方实际 mixer／profile／FFT／设备函数的输入输出与副作用；模拟、共享控件测试和双方均无设备都不等于功能一致。本批只静态核对，没有执行 UI、官方 DLL、音频捕获或真实触觉写入。

## 原件与位置

位置均为原始 UTF-8 半开字节区间；来源、版本和字节身份见当前获取索引及对应 acquisition.json。恢复文件及合成映射只是帮助阅读，原始字节与完整词法绑定保持权威。

| 所有者 | 原件／位置 |
| --- | --- |
| Sensa 主模块 | [1766.f84665e2.chunk.js](../../../.analysis/inputs/frontend/chroma-app-sensa-hd-current/raw/apps.razer.com/chroma-app/sensa-hd/static/js/1766.f84665e2.chunk.js)，模块 24032；独立页 `[954765,959571)`、嵌入页 `[953726,954755)` |
| 来源／设备交互 | 同上，`[832287,841235)`、设备卡 `[814058,817579)`、修改 `[812120,813906)`、广播 `[620986,622170)` |
| 预设与编辑 | 同上，预设顺序 `[674523,674783)`、游戏下拉 `[866803,874579)`、自定义面板 `[935267,937838)`、图 `[913253,924340)`、输入 `[900139,901257)` |
| FFT 订阅与 KeyStorage | 同上，`[949204,949939)`、`[941263,942926)` |
| lighting-engine 消费者 | [main.a383ea9e.js](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js)，HapticMixerManager `[1021563,1060988)`；等级表 `[490513,490584)`；音频 wrapper 模块 `[88968,118518)` |
| 音频原生完整函数 | [捕获](../../../.analysis/results/native/9a87a6db3eb1d2e7733af008010a44c5b2427157a2040b73fe69004f92ae404d/capture.json)，Initialize `0x65010`、AcquireAV `0x65110`、ReleaseAV `0x666a0`、Start `0x66b90`、Stop `0x66eb0`、SetHapticProfile `0x6c770`、SetHapticMixerGain `0x6ff00`；内部 `0x81cf0/0x39370/0x494d0/0x92a0` |
| 1383 音频消费者 | [3487.9ef7f3a64b64937d21fb.js](../../../.analysis/inputs/frontend/audio-mw-1383-current/raw/apps.razer.com/synapse/products/1383/mw/3487.9ef7f3a64b64937d21fb.js)，ENABLE `[1216046,1216271)`、SET_HAPTIC `[1216271,1216526)` |
| 1439 音频消费者 | [1540.74616a72eedcd692973f.js](../../../.analysis/inputs/frontend/audio-mw-1439-current/raw/apps.razer.com/synapse/products/1439/mw/1540.74616a72eedcd692973f.js)，ENABLE `[1144148,1144373)`、SET_HAPTIC `[1144373,1144591)` |
| 当前服务包谱系 | [资源 manifest](../../../.analysis/inputs/official-native-current/manifests/27f1518bf49220045840c8688b1ca83f47eb38bb8591868bed085a33782bdb68/manifest.json) 的 `resourceManifest.resources[12]`；[完整提取收据](../../../.analysis/results/native-packages/33954697ea3c99d8a7e1ffd99255834c29a01e4d0712b9e8fbc83f5db346d83f/inspection.json) |
| 服务调试 HTML 与 HTTP 控制 | [服务捕获](../../../.analysis/results/native/cdd88876f31903e894d681732460adbb82e596df3b7caa747fd1489dc85a3eec/capture.json)；入口 `0x184c70`、端口 `0x184770`、监听 `0x1999f0`、accept `0x1997d0`、HTTP `0x1964f0`、响应 `0x1950d0`、`/mixer` `0x1953a0`；RCDATA `[10,101,1033]` RVA `0x2c1440`／49,953 字节，控制表 `0x240ed0`／10×24 字节 |
| HTML 完整 JS 所有者 | 同上 RCDATA 的原始字节区间：`cmd [19869,20299)`、`cmdThenRefresh [20311,20749)`、`refreshStatus [21758,21952)`、`updateCtrlBadges [21964,22452)`；script `[14210,49924)`，完整 AST 静态解析 |
| 服务停止与 mixer 消费 | 同上；服务 `0x193260`、控制 `0x193570`、Synesthesia stop `0x1943f0`、provider 遍历 `0x4e440`；mixer `0x1dac0/0x1efb0/0x226e0/0x1f4b0/0x50b0/0x5830/0x637c/0x6490/0x23220/0x264f0/0x263d0/0x1fe50`；vtable `0x1e7050/0x1e5440/0x1e54f8` |
| provider 停止发送与释放 | [provider 捕获](../../../.analysis/results/native/62465547a378d65b03e5ba2b68b1d17ed0bcfd16bdf84c9f4e179ab545b1c06b/capture.json)；`ProviderClean 0x2eab0`、发送 `0x2e180`、编码 `0x2c1e0/0x2c2c0`、长度 `0x2c2b0`、程序名 `0x2d390`、IPC 打开 `0x2dfb0`、初始化 `0x2e710`、API 定位 `0x31740/0x31830/0x31ac0/0x31b30`；StopProgram vtable `0x64440` |
| 当前 API proxy 析构 | [API 捕获](../../../.analysis/results/native/cc2a78f8c9aeae01a5b2afb1ddac301b2fee11ca2be07824715060aadfe1f92e/capture.json)，`GetRzInterHaptics 0xfde0`、构造 `0x660b0`、vtable `0xfaa50`、析构 `0x67060`、容器释放 `0x663c0`；不据此推导全部设备与回调清理 |
| HAR 记录移除与可选回调 | [HAR 捕获](../../../.analysis/results/native/ed774be3b3fac9d3dad0d75c8ad2e95d019fec1c4720b77957ce434e468c807e/capture.json)，导出 `RemoveConnectedDevice 0x67e80`、`RegisterDeviceCallback 0x67d00`、`RemoveDeviceCallback 0x67d10`；provider 实际导入槽 `0x57020`，调用指令 `0x2eb6b` |
| proxy 内含设备代理与控制块 | 同上 API 捕获；`0x66520/0x66590/0x67150/0x676d0/0x65f00/0x68470/0x66e80/0x68480/0x66f90/0x68390/0x683b0/0x66d30/0x66d60`；vtable `0xfaa98/0xfaa70/0xfaa28/0xfa9f8` |
| provider 回调对象与记录清理 | 同上 provider 捕获；`0x2c540/0x2d040/0x305c0/0x243b0/0x305e0/0x2e9e0/0x2f9a0/0x2f830/0x2d240`；vtable `0x64378/0x643c0`；引用创建上下文 `0x2d520` |
| API 设备回调绑定与消费者 | 同上 API 捕获；回调引用 setter `0x67be0`、事件消费者 `0x677a0/0x67a00`、绑定调用器 `0x68490`，vtable `0xfaac0` |
| 下层回调所有者与全局结束 | 同上 API 捕获；模式 setter `0xff50`、初始化 `0xae80/0xb220`、构造 `0x3b7d0/0x581a0`、注册 `0x3e930/0x5c670`、函数对象替换／释放 `0x3c280/0x40520/0x8a60`、析构 `0x3cf20/0x3be10/0x3cf60/0x41c00/0x59fb0/0x59200/0x58e10`、Terminate `0x10000`、DllMain `0xfd90`；vtable `0xf8d20/0xfa398` |
| FFI 成员及 JSON 清理 | 同上 API 捕获；日志来源 `0x51b60`；设备记录 `0x58840/0x58cc0/0x58e90/0x59180/0x588c0/0x58d30/0x58f00`；命令表 `0x58940/0x58da0/0x58fc0`；JSON `0x26fa0/0x14420/0x25e60/0x22730/0x13dc0/0x14bb0` |
| FFI 宿主输出与包装 | 同上 API 捕获；导出 `0x2eb80/0x2ec50`、保存 `0x5f8f0/0x5f9b0/0x61870/0x619d0`、发送 `0x5a010/0x5a110`、调用器 `0x24ad0/0x24ae0`、销毁槽 `0x24a80/0x24a90`；vtable `0xf8a10/0xf8a48` 各 48 字节、6 槽 |
| FFI 命令绑定与具体消费者 | 同上 API 捕获；`0x512b0/0x615b0/0x602a0/0x61040/0x602d0/0x5a970/0x5b770/0x5b640`；vtable `0xfa948` 为 48 字节、6 槽；`+16 → 0x604c0` 在同 SHA 的 `supplements/e5d88ab5500bd5f3/capture.json#/decoded_thunks` 保留 `[0x604c0,0x604cb)` 的 11 字节／三指令及操作数，函数化／伪代码失败独立保留 |
| RPC 客户端线程与恢复 | 同上 API 捕获；`0x3f900/0x30e40/0x3ca50/0x3dd20/0x69760/0x3fc80/0x30e80/0x3cb10/0x3e980/0x69d20/0x3c0c0/0x41c60/0x40270/0x3c3e0/0x3e680/0x3f630/0x69c10/0xea6a0`；全局 binding `0x1315a8`；`0x3f630` 异常 chunk `[0xeaa10,0xeaa2c)`，`0x69760` 异常 chunk `[0xf1200,0xf121b)` |
| RPC 服务启动与结束 | 同上 API 捕获；`StartRPC 0xff60`、构造 `0x4a250`、线程入口 `0x46730`、监听 `0x4c9c0`、同步通知 `0x4cfa0`、停止 `0x4cda0`、析构 `0x4ae10`、记录表清理 `0x4a920/0x4a890/0x464d0/0x46450`；listen 指令 `0x4cc9b`，非等待 unregister 指令 `0x4ceb4` |
| HAR 回调与服务插件加载 | 同上 HAR／服务捕获；HAR `0x67b80/0x67d00/0x67d10/0x67e80`，回调调用指令 `0x67bc4/0x67eaa`；服务 `0x4e790 → 0x4d980`，两个插件加载调用 `0x4e812/0x4e845`，`LoadLibraryA 0x4d9b9` |
| 当前宿主 FFI owner | [FFIPreloadMain.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/ffi/FFIPreloadMain.js)；async native 调用 `[1143,2273)`、配置 `[2273,3226)`、退出登记 `[3227,3623)`、callback `[6351,7719)`、空 FreeFFI `[7719,7757)`、callDLL `[8564,11243)`、callDLLAsync `[12005,13913)`、exit 遍历 `[13914,14253)` |
| 宿主 IPC 与退出 | [preload.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/preload.js)：response `[4287,4424)`、main IPC `[5289,5349)`／`[5350,5420)`；[main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js)：quitAppFinal `[23367,24455)`、主进程 handler `[59202,59228)`／`[59257,59293)` |
| 当前 callback JS 包装 | [callback.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/node_modules/ffi-napi-rz/lib/callback.js)：完整 Callback `[723,2926)`、错误报告 `[354,560)`；[ffi.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/node_modules/ffi-napi-rz/lib/ffi.js)：CanUseAnneCallback `[2735,2827)` |
| node closure／跨线程／释放 | [node 捕获](../../../.analysis/results/native/543b068fea0c3878380fb2b30508ad4fd970fcbb9ec0df6d24a163633be3512a/capture.json)：`0x23a0/0x2750/0x6af0/0x6d80/0x7250/0x7400/0x7b20/0x8580/0x87d0/0x8830/0x8c20/0x93e0/0x94c0/0xa0f0/0xa440/0xa480/0xd060/0xd800`；make_callback `0x243e`、instance_data `0x72d5`、async_send `0xa246`、cond_wait `0xa254`、cond_signal `0x9483`、add_finalizer `0x9cb4` |
| lighting 回调配置与消费者 | 同上 lighting-engine JS；模块 4407 `[601302,605719)`、完整 wrapper class `[287197,319263)`、init `[291436,292830)`、JsonApi／Add／Remove／Notification `[292856,293089)`／`[293117,293277)`／`[293308,293441)`／`[293472,293544)`、listener `[297897,297957)`；manager class `[991554,995137)`、命令 `[992291,993433)`、stream `[994244,994514)`、setter `[994534,994803)`、getter `[994873,995136)` |
| HapticMixerManager 回调与取消 | 同上 lighting-engine JS；init `[1022623,1023059)`、callback `[1022752,1022814)`、DLL load `[1053392,1053710)`、完整 beforeUnload `[1025551,1026276)` |
| FFI sender 事件与输出 | 同上 API 捕获；`0x5c870/0x5e1f0/0x5ece0/0x5ce20/0x5a010/0x5a110/0x59650/0x5c040/0x505c0`；Get WithResult 检查从 `0x5d17a` 起、调用 `0x5d73b`、输出移入 `0x5d747`；事件名原字节 RVA `0xfa4e8` |
| AddDevice 输入、提交及外层导出 | 同上 API 捕获；`0x2e860/0x2e880/0x2ead0/0x5a970/0x5abd0/0x5c0e0/0x57d70/0x57eb0/0x53150/0x53460/0x59930/0x62c00`；锁／解锁 `0x5b311/0x5b412`，全局修改 `0x5b35f/0x5b36e/0x5b383`，新增回调 `0x5b461` |
| AddDevice 登记表的读取与代理 | 同上 API 捕获；`0x5a300/0x5a570/0x5a590/0x5d9f0/0x67690/0x67100/0x675d0/0x675b0/0x67150/0x676d0/0x65f00/0x677a0`；vtable `0xfa398`／64 字节、`0xfaa28`／32 字节、`0xfa9f8`／40 字节、`0xfaa50`／24 字节 |
| AddDevice 与导出异常清理 | 同上 API 捕获；`0xdc7f4/0xe5430/0xe54c0/0x59120/0x3bd90/0x4acd0/0x589c0/0x59140/0xef270/0xef280/0xef2a0/0xef2d0/0xef310`；Add `.pdata 0x13773c`、unwind `[0x1157f0,0x115818)`、FuncInfo `0x11581c`、unwind map `[0x115825,0x1158a2)`、IP map `[0x1158a2,0x1158f3)`；导出 `.pdata 0x134ef8/0x134f04`、FuncInfo `0x109e50/0x10a030`、catch 表／handler `0x109ead/0x109eb5` 与 `0x10a068/0x10a070` |
| AddDevice JSON 与迭代上下文 | 同上 API 捕获；`0x207d0/0x24350/0x274f0/0x27820/0x22390/0x22c00/0x21660/0x22470/0x629a0/0x632f0/0x2fd30/0x55560/0x55500/0x3c740/0x3c960/0x407b0/0x40bc0`；HRESULT 键原字节 `0xf7598`，catch 继续地址 `0x2eaab/0x2eb73` |
| 新增事件的 provider 消费者 | 同上 provider 捕获；完整 `0x2c540/0x2c3e0/0x300e0/0x301b0/0x533b0`；HAR 导入槽 `0x57050/0x57060/0x57038/0x57068`，调用 `0x2c7d0/0x2cc0e/0x2cc2b/0x2cd90`；信息／触觉代理调用 `0x2c60b/0x2c687/0x2c751` |
| RemoveDevice 解析、删除与通知 | 同上 API 捕获；`0x5b770/0x466d0/0x524f0/0x58e90/0x59180/0x588c0/0x58d30/0x58f00/0x6a50/0x308c0/0x30f80`；锁／节点释放／通知复制／解锁／回调指令 `0x5bc60/0x5bd3e/0x5bd55/0x5bd84/0x5bdd1`；FuncInfo `0x115924`、unwind map `[0x11592d,0x115958)`、IP map `[0x115958,0x11598b)` |
| proxy 移除消费者与 catch | 同上 API 捕获；`0x67a00/0x654a0/0x64980/0x64430/0x64600/0x64c30/0x65050/0x667f0/0x68080/0x691d0/0xf1140`；查表／解锁／DeviceManager 调用 `0x67add/0x67b8c/0x67b9d`；FuncInfo `0x118984`、unwind map `[0x118991,0x1189a1)`、try／handler／IP `[0x1189a1,0x1189a9)`／`[0x1189a9,0x1189b1)`／`[0x1189b1,0x1189c5)` |
| provider 移除与节点清理 | 同上 provider 捕获；`0x2d040/0x2fce0/0x30d80/0x2ab60/0x30800/0x2d240/0x1e250/0x1e390/0x1ddf0`；观察者／bodypart／connected-device／节点移除调用 `0x2d084/0x2d11a/0x2d162/0x2d19b`；FuncInfo `0x686f0`、unwind／IP `[0x686f9,0x68700)`／`[0x68700,0x6870a)` |
| HAR bodypart 删除、列表重算与清理 | 同上 HAR 捕获；`0x67a50/0x2eb40/0x2f1f0/0x29370/0x29af0/0x29860/0x2b640/0x2b760/0x2b810/0x2d010/0x27480/0x26ca0/0x273b0/0x27950/0x27a70/0x27c20/0x2c5a0/0x209c0/0x21230/0x7cb90/0x7c660/0x26a80/0x209b0/0x28e00/0x28e30/0x21f50/0x21620`；完整导出 `.pdata 0xa6ef4/0xa6f00/0xa6f0c`；内部 FuncInfo `0x902ac/0x903e4/0x8f678/0x8f75c`，均无本函数业务 catch |
