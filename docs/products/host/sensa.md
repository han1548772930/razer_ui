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

## 当前 Rust 入口与能力核对

当前 [ChromaPage::modules](../../../crates/razer-app-pages/src/chroma/chroma_page.rs) 固定列出 Sensa HD，但 Install 按钮禁用且没有 Sensa 打开事件；[app_picker_host.rs](../../../crates/razer-shell/src/shell/app_picker_host.rs) 的实际可启动模块也不含它，强制 Open 进入“窗口服务尚未连接”通用分支。本批未找到独立 Sensa 根、Dynamic Haptics、Audio to Haptics Profiles、嵌入视图或上述完整 mixer／profile／游戏会话链的 Rust 生产所有者，不能以模块图标、注册名称或其他设备自己的触觉 slider 判断本应用已接通。

| 官方链 | 必须对应的独立实现与页面行为 |
| --- | --- |
| 独立根与设备嵌入 | 根拥有本地配置、device／memory／storage／游戏事件，完整挂载两页／Help／引导；嵌入根按实际 predicate、类别与强度字段选择对象。当前没有该页面树、选择／焦点、说明、禁用原因和释放消费者 |
| 来源与设备强度 | 来源 1…6 和设备 0…6 的单位／身份／确认分别处理；Source gain、本地设备修改、真实设备写入、会话关闭弹层和 A2H 的 100 ms／500 ms 门禁不能合并为本地布尔保存 |
| 预设、游戏与自定义 | 四项顺序、继承／Immersive 条件、两次 action、band 范围、150 ms 调度、关闭即时保存和前台／游戏恢复分别实现；未知字段及失败不能伪装成默认已生效 |
| FFT、mixer 与输出 | 按实际端点和两套采样参数拥有捕获／DSP／profile setter／本机 IPC／设备输出与应答；旧会话释放后才能切换。底层官方 DLL 仅作分析和显式对照，产品仍需纯 Rust 实现 |

当前嵌入 arrow `[953726,954755)` 和独立根 function `[954765,959571)` 均已按完整 JS 的 AST 边界重新核对，真实设备卡 arrow 为 `[814058,817579)`；获取收据、入口、manifest 和当前部署原件的实际字节匹配且前后未变。来源／设备交互的合并区间 `[832287,841235)` 只是多个所有者的定位上下文，不能将它当成单个完整函数或以其存在宣称所有页面状态已核完。

后续自动化须覆盖实际设备寻址／同 PID 多实例、active／standby／off／restart-required、未知观测、来源与设备不同强度、会话 Keep On／Turn Off、预设与 band 边界、真实读取与草稿不同、异步取消和恢复／释放。通过显式对照入口再核官方实际 mixer／profile／FFT／设备函数的输入输出与副作用；模拟、共享控件测试和双方均无设备都不等于功能一致。本批只静态核对，没有执行 UI、官方 DLL、音频捕获或真实触觉写入。

## 原件与位置

位置均为原始 UTF-8 半开字节区间；来源、版本和字节身份见对应 acquisition.json。恢复文件及合成映射只是帮助阅读，原始字节与完整词法绑定保持权威。

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
