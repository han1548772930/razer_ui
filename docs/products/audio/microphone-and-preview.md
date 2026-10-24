# 麦克风、监听、录音与设置向导

适用页面和功能组合以 [产品矩阵](README.md#产品与适用边界) 及每个 PID 的实际组件为准。Mic volume、OS mute、硬件静音、sidetone、return switch、PCM capture 与 DSP 效果是不同状态，不能用一个 `isEnabled` 代替全部。

## 音量来源与写入结果

1422/1446 的 `Audio_StreamMixer` 覆盖通用 mic/HP volume 事件：二者为 `Storage_NoSave`，monitor 为 `Storage_Global`，EQ/AGC/旋钮属于 profile。页面初始化按 HP volume → HP mute → mic volume → mic mute 实读；只有成对 getter 都返回值时才发布对应控件。`getRuntimeData` 也补入缺失的本地 sidetone 字段并发布；这个字段是本地默认，不是同一批 getter 的设备观测。

`setMicVolume/setHPVolume` 先写 level，再写 mute 的反值 `!isEnabled`。level 返回 `false` 仍继续 mute；抛出异常会跳出 catch。最终 `isSetLevelSuccess && isSetEnableSuccess` 才是外层结果，实际每次调用的返回仍需保留。设备身份分别是原版 `DeviceId_MarieMic` 262145、`DeviceId_MarieHP` 524289，不能换成 UI 行号。

monitor 也按 level → enable 顺序。若 Stream Mixer 实际模式为 1，源码跳过 monitor 写入并返回 `true`；这不是一次 setter 应答。restore 中 monitor 有每次 restore 的 guard，NoSave mic/HP 不从 profile 重放。当前 1422 的完整函数在 MW `8539.79323eca355460a8ff0d.js`：实读 `[1694310,1695123)`，HP `[1708671,1709152)`、mic `[1709152,1709635)`、monitor `[1709635,1710249)`。

其他音频产品的 mic 路径须区分 `simpleService`、`rzDevice`、THX generic/v2/v3 与系统静音。1439 的 `audioBasicFeature` 明确 `isHwMicMuteOnly:true`；1465 明确为 `false`。2657 等耳塞的 basic audio 也是 hardware-mic-mute-only，不能把 OS 静音和硬件静音双写为一项通用动作。

## 电平事件与页面消费者

原版 AudioMarie 先匹配当前 `containerId`，再发布事件；`levelMeterPumpup` 将 device ID 映射到设备枚举。`Audio_StreamMixer.handleAddLevelMeterListener` 去掉 device ID 0 后发布实际电平，mic/HP `deviceVolume` 更新各自 volume/mute；`deviceMute` 还配对停止/开启设备电平和后台录音，再更新相应控件。原版某些缺失字段用 0 参与呈现，这属于源码消费者语义，不能将缺字段记为已测得 0 音量。

开始/停止混音电平监听分别 `onEvent/offEvent` 配对，并开启/关闭 meter。原生包装器 `handleBeforeUnload` 调用 terminate、删除 unload listener、清除事件；terminate 先注销 host listener，再在初始化标志存在时 `DeviceTerminate`。该标志在 ConfigureFFI 成功后、DeviceInitialize 前设置；DeviceInitialize 返回 false/异常不会自动证明标志已清除。不能把模块加载成功称为设备初始化成功，也不能让产品依赖官方包装器完成这些能力。

## 页面实时预览

1422 当前 UI `684.f6b88b50.chunk.js` 中 footer 所有者 `[349190,353043)`，实时预览 `[339375,349157)`。1446 自身 `684.9501aa81.chunk.js` 中分别为 `[361700,365553)`、`[351885,361667)`。1422 的 footer 在 EQ、Effects 和 Stream Mix 挂载；1446 还包含 MIC。两款根 render 都只排除 Lighting 和 Help，不能将 1422 的三页名单共用给 1446。只有浏览器音频准备、设备 ready、当前窗口 active 且 visibility 为 visible 时允许捕获。采样率来自产品 `supportedMicSamplingRates`（缺项时原版使用 48/96 kHz）；切换属于独立 `ON_SET_SAMPLING_RATE_VALUE`，不归混音 preset。

实时预览不是设备 setter：它取得真实麦克风 stream，关闭 echo cancellation/noise suppression/AGC，建立 AudioContext、gain 与 analysers，再绘制波形。启动使用 generation 计数；旧 `getUserMedia` 完成后若代次不同立即 stop tracks。关闭、不可见、切设备与 unmount 增加代次，清 retry/interval/animation frame，disconnect nodes、stop tracks、close context、清 ring buffer 与历史电平。

控件同时使用硬件 meter 与 PCM：有效外部百分比映射到 dB，并以本机 analyser 校准；外部数据不足时使用 analyser。图形的 6 秒历史、-3/-12/-18/-60 dB 辅助线、平滑与峰值标记属于显示，不证明 EQ/DSP 已生效。捕获失败的重试最多执行源码计数规定的分支，不能显示设备已连接或生成模拟波形。

## 录音与回放路由

1422 UI 模块 9374，`38.d64aff18.chunk.js` bytes `[51422,58031)`；1446 是自身 `287.d23d42d9.chunk.js` 的同模块 `[146833,153442)`。录音优先选真实枚举中带 `Service (Do not use)` 的 input，找不到时使用系统捕获选择；按当前 sampling rate、单声道关闭 echo/noise processing。MediaRecorder 按实际支持顺序选择 pcm/L16/wav/webm/mp4，停止后以实际 MIME 和字节形成录音，stop tracks。空录音不启用播放。

播放独立查找对应 service output，建立 blob URL，发送 `{switchstate:1}`，等待 UI 消费 `MW_UPDATE_MIC_RETURN_SWITCH_TO_UI`，在可用时 `setSinkId`，再启动音频。源码还有 BroadcastChannel 和手动 reducer 通知分支，超时后继续的行为不能算硬件 return-switch 确认。真正的 return-switch 设置仍需协议应答；不能把 Redux 值当 ACK。

停止、ended、播放异常与 unmount 都发送 `{switchstate:0}`。停止还 pause、回到时间 0、撤销 blob URL、移除 ended listener；unmount 释放捕获 tracks 与播放资源。源码部分异步等待和 monitor interval 的生命周期并不都由完整取消任务包住，应逐项记录其实际释放，不宣称存在源码未提供的取消屏障。

## 麦克风设置向导

同一 `684.f6b88b50.chunk.js` 完整向导 `[366649,383572)` 与 PCM capture `[353065,363586)`。进入时只保存一次快照：mic volume、noise gate、AI suppression、compressor、limiter、reverb、选中 EQ、vocal bass/exciter；100 ms 后应用源码测量默认值。Cancel 和关闭按钮用该快照逐项经原动作恢复，再关闭。Finish 将测量得到的 mic gain/noise gate 和用户选择的用途/距离参数提交，然后关闭；这些动作会产生真实设备/DSP 工作，不能只改向导 UI。

1446 的完整向导是自身 `684.9501aa81.chunk.js` `[379159,396082)`，PCM capture 为 `[365575,376096)`。1422 根 render `[432538,434713)`、1446 `[445101,447401)` 均将 `isMicSetupOpen` 向导作为根节点的独立兄弟挂载，位于 footer 条件之外。这两个范围是静态 AST 的完整 `method_definition` 边界。根还在设备未 ready 时显示 `processing-layout`；不能把遮罩、footer 是否挂载、PCM 是否已取得以及向导是否打开合并成一个 ready 状态。

语音/环境分别最多捕获 10 秒，记录其当前测试类型后分发分析。选择设备会排除 default、stream/playback mix、service、Mic (Dry)；优先当前设备名且带 microphone 的真实输入，再用源码规定的实际枚举候选。语音分析在无有效音频或 metric 小于 -100 时不提交增益；环境录音过短仅输出源码警告。gain/noise 算法分别位于模块 9909/596，不能用固定推荐值替代采样。

PCM capture 拥有 loading/recording/error、tracks、AudioContext、worklet、splitter、analysers、计时器及 animation frame。开始失败逐项 disconnect/stop/close；worklet blob 在 finally 撤销。停止按真实 context sample rate 合并第一通道 PCM 后调用完成消费者，再清状态；unmount 取消计时并释放节点/stream/context。不得执行厂商 JS 来“验证”该流程，本轮仅静态读其完整函数。

## 固件效果与参数区别

1422/1446 还有 noise gate、AI suppression、compressor、limiter、reverb、de-esser、AGC 与旋钮。数值范围、signed 编码、profile ID、写入顺序分别属于具体协议；参数 EQ、HPF 与 vocal 效果见 [EQ](playback-and-eq.md#参数均衡器与效果顺序)。AGC 当前 `initFromDevice:false`，布尔 coercion 包含 true/1/字符串 true/1，真实 setter 为 `(profile=1,module=1,state)`。旋钮 restore 先读取，值相同跳过，不同或失败后进入源码 setter；直接设置不带额外猜测的读回。generic 参数提取只取 `dialControl`，不把相邻 `mappingList` 当设备协议。

## 1446 MIC 页面与映射

1446 根导航的实际数组位于 [684.9501aa81.chunk.js](../../../.analysis/inputs/frontend/audio-1446-current/raw/apps.razer.com/synapse/products/1446/ui/static/js/684.9501aa81.chunk.js) `[441038,441391)`：依次为 `MIC/TAB_EQ/TAB_EFFECTS/STREAM_MIXER_HEADER/TAB_LIGHTING/HELP`，MIC 真实挂载模块 5303。该模块完整 factory `[53500,64483)` 内，`J` 页面类 `[60847,64175)` 是两列设备控制：左侧 `dialControl` tabs、设备图与左右连线，右侧单击说明、MultiFunction 开关、双击/三击映射与长按说明。关闭 MultiFunction 时两组 tap 下拉项禁用；说明提示包含三段实际文案。不能用 1342 Mixer 的 mute-button 页面代替此组件。

当前实际导出 `Y` 在 `[64175,64483)` 连接的是 `J`；不能采用同模块早先保留的另一页面类。`J` 的实际 `K` 菜单仅有 Cycle Chroma、Toggle Chroma、分隔线、禁用标题及当前产品的 `QUICK_EFFECTS` 项。`setMuteButtonMapping` 克隆完整列表，按稳定 `inputID` 寻找 `MUTE_BUTTON_DOUBLE_TAP/MUTE_BUTTON_TRIPLE_TAP`，删除先前 `outputType` 的内容，再构造新的 assignment/config。该泛型函数虽还保留 multimedia 分支，但当前菜单不会生成这些选项，不能据此扩展 MIC 界面。读取选中项也要检查对应 assignment 与 effect ID；显示下标不是持久化身份。必须保留未编辑的其他映射和字段，不能只保存一个选项名称。

两项动作具有不同的后端语义，均已沿当前 MW 原件追到实际消费者：

| 动作 | 完整官方所有者 | 实际提交和结果 |
| --- | --- | --- |
| MultiFunction | [7174.dc4453fd7710ace93efc.js](../../../.analysis/inputs/frontend/audio-mw-1446-current/raw/apps.razer.com/synapse/products/1446/mw/7174.dc4453fd7710ace93efc.js) 模块 87174，factory `[559,3283)` | `ON_TOGGLE_MULTI_FUNCTION` 属于 `Storage_Profile`。setter 为 `setChromaTap(profile=1, enabled?1:2, SingleTap, CycleEffect)`，检查协议 `SUCCESS`；异常或非成功均返回 false。restore 先 `getChromaTap(1, SingleTap)`，与 profile 的目标值比较，必要时才写；getter 失败进入源码 setter。profile 默认/恢复动作与设备实际读回必须分别保留，不能由本机无文件推断需要初始化。 |
| 双击/三击映射编辑 | [6370.286ec634bda2c5a79084.js](../../../.analysis/inputs/frontend/audio-mw-1446-current/raw/apps.razer.com/synapse/products/1446/mw/6370.286ec634bda2c5a79084.js) 模块 16370，factory `[559,2659)` | `ON_SET_MUTE_BUTTON_MAPPING` 将 `mappingList` 保存到当前 profile 的 `mappings` 并发布 UI。其 `execute` 直接返回 true，未在此函数写入 tap 映射协议；不能将此 true 解释为设备 ACK。 |
| 硬件 tap 的映射消费 | [8539.84676161aef6933093a6.js](../../../.analysis/inputs/frontend/audio-mw-1446-current/raw/apps.razer.com/synapse/products/1446/mw/8539.84676161aef6933093a6.js) 完整 tap 回调 `[1490518,1495295)` | `tapMode → 稳定 inputID → 当前 profile.mappings → assignment`，再构造并排队实际本机事件。分支包含混音输入/输出音量、mic 音量/静音、旋钮功能、灯光循环/开关/指定效果；部分动作先真实读取系统音量或当前本地混音 preset。未知 tap、缺映射、缺目标通道与异常均按实际分支停止，不推断动作成功。 |

tap 回调还消费单击、长按及当前 profile 中的其他映射，其支持范围比本页双击/三击下拉更广，不能据此给页面添加额外选项。以上两个 lazy feature 的来源、版本、长度与 SHA-256 已与当前 MW 获取收据核对；原件可用于静态逆向，生产路径仍应由 Rust 实现其完整读写、映射持久化、硬件事件与释放链。

## 当前 Rust 已接线部分与缺口

当前 1422/1446 已能定位独立 Rust 的音频逻辑，已接线部分不应因旧索引中出现 `native` 或 ABI 名称而被误记为官方 DLL 后端。下面记录的是本轮实际回读的生产文件，不沿用先前报告的 Rust 文件哈希：

| 能力 | 当前所有者 | 已找到的真实链与生命周期 |
| --- | --- | --- |
| EQ analyser 和独立实时预览 | [audio_seiren_audio_runtime.rs](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_audio_runtime.rs)、[audio_seiren_microphone.rs](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_microphone.rs)、[waveform](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_microphone_waveform.rs) | EQ analyser 仅在实际 EQ 页、活动/可见且 ready 时捕获；footer 另有 preview owner，不能共用一个“正在录音”状态。挂载后 500 ms 再允许启动；捕获失败最多五次，间隔按 1/2/4/8 秒退避；无有效 PCM 时不产生录音或模拟波形。 |
| capture 服务 | [Shell capture](../../../crates/razer-shell/src/shell/audio/seiren_audio_capture.rs)、[服务 capture](../../../crates/razer-service/src/audio/seiren/capture.rs)、[Windows capture](../../../crates/razer-service/src/runtime/windows/audio/seiren_audio_capture.rs) | `SeirenAudioCaptureRequested → 服务 Start/Poll/Stop → WASAPI/COM`。检查产品/session、started、有限 PCM 与实际采样率；Start 返回形状错误也进入 Stop/Shutdown 清理。切页、断开或退出发取消，保留 worker 到清理结果和 join；旧请求不得覆盖新 owner。 |
| 录音与回放 | [Shell playback](../../../crates/razer-shell/src/shell/audio/seiren_audio_playback.rs)、[服务 playback](../../../crates/razer-service/src/audio/seiren/playback.rs)、[RecordingFile](../../../crates/razer-storage/src/audio_recording.rs) | preview、record、measurement 和 playback 分别拥有任务。真实 PCM 形成 clip；空/无效录音不能播放。播放锁定当前有线物理路由，先检查 `ReturnSwitch(true)` 的 `report.confirmed()`，再执行实际播放；结束、失败、取消都恢复 `ReturnSwitch(false)`。Stop/Shutdown 确认先于删除临时 PCM，必要时重建服务连接以完成恢复。 |
| 设置向导 | [wizard](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_microphone_wizard.rs)、[view](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_microphone_view.rs)、[setup seen](../../../crates/razer-shell/src/shell/audio/seiren_microphone_setup.rs) | 只保存一次快照，mic volume 来自实际 endpoint 观测；未观测时不伪造可恢复原值。100 ms 默认设置、48 kHz 实测、取得真实帧后开始 10 秒计时、重录冷却、语音/环境算法均有独立所有者。Cancel 经原操作恢复 mic/DSP/EQ/效果；Finish 重放选中动作，完成后更新本地 setup seen 并恢复焦点。当前关闭动作会提交异步操作；仍须核对每项应答，不能以向导已经消失判定设备恢复成功。 |
| 端点、驱动、meter 与重配 | [seiren_session.rs](../../../crates/razer-service/src/runtime/windows/audio/seiren_session.rs)、[session owner](../../../crates/razer-service/src/runtime/windows/audio/seiren_session_owner.rs)、[driver](../../../crates/razer-service/src/runtime/windows/audio/seiren_driver.rs)、[endpoint](../../../crates/razer-service/src/runtime/windows/audio/seiren_endpoint.rs) | 独立 Rust 串行 worker 使用精确 container/interface、overlapped IOCTL 和系统 `IAudioEndpointVolume`/通知回调。setter 后真实 observe 并核对 level/mute；端点重建只恢复重配前新鲜实读的 mic/HP 原值，自发重连不写旧缓存。取消时 `CancelIoEx` 后等待在途请求 drain，退出关闭通知、meter、端点并 join。当前 session 不加载厂商业务 DLL，`version()` 为 None。 |

实际 mounted tree 与当前 Rust 仍存在以下明确差异，不能据上表认定两款页面完成：

| 缺口 | 当前证据与影响 | 后续实现所有者和验收范围 |
| --- | --- | --- |
| 1446 MIC 漏挂 footer | [seiren_microphone_mounted](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_microphone.rs) 只接受 EQ、Effects、Stream Mix，缺 `MIC`；[页面根](../../../crates/razer-pages/src/features/audio/mod.rs) 和 footer 自身均依赖此条件。官方 1446 只排除 Lighting/Help，所以 MIC 页面缺实时预览、采样率、录音、播放及向导入口。 | audio page/microphone owner 按 PID 的实际导航修正挂载，覆盖 1422 三页、1446 四页及两个排除页的状态/取消测试。不能无条件给其他音频产品挂此 footer。 |
| 向导错误依赖 footer 挂载 | 页面根将 `seiren_microphone_wizard` 放在 `seiren_microphone_mounted()` 内；原版将其作为根的独立兄弟。当前切出这组页面会隐藏向导；`cancel_seiren_microphone` 停止捕获/回放，却不移除 wizard 或其默认设置任务，不能视为用户 Cancel 已恢复快照。 | 页面根与 wizard owner 按原版独立挂载，分别核对导航、窗口关闭、断开、延时默认设置、测量停止、Cancel 恢复、Finish 提交及焦点。修正挂载后仍需等待真实服务清理和设备结果。 |
| MIC 的旋钮/多功能/映射整链未接齐 | [page_body](../../../crates/razer-pages/src/features/audio/mod.rs) 只有 Seiren EQ/Stream Mixer 专用渲染，1446 MIC 落入通用目录 panel；当前目录只列 MultiFunction toggle。`edit` 发 `AudioProductChanged`，其 [workspace 消费者](../../../crates/razer-pages/src/features/workspace/source_workspace.rs) 只捕获快照；`dispatch_seiren_microphone_control` 仅转 DSP modules 2–6，未找到 MultiFunction/双击/三击映射的对应生产消费者。 | 1446 MIC 的页面、typed profile/mapping、协议与硬件事件 owner，完整实现本节 5303/87174/16370 及 tap 回调调用链。验证旋钮切换、MultiFunction get/比较/set/status、映射持久化/恢复、各 assignment 分支、错误与释放；只修改本地 toggle 不算成功。 |

## 样式、资源与验证范围

CSS 加载顺序见 [EQ 页面](playback-and-eq.md#seiren-实际-eq-页面与当前-rust-接线)。当前实际 footer 高度为展开 145 px/折叠 45 px，400 ms `cubic-bezier(.4,0,.2,1)`；live/setup 悬停为 200 ms；record/stop 图标 24 px，play 12×16 px，禁用透明度 0.5。录音脉冲的早期规则与后载 684 规则存在覆盖，最终采用后者的 0.4 透明度，不能仅取第一次匹配。录音标题和向导倒计时有独立字距，不能套全局默认字体间距。

向导的后级 `.modal` 样式覆盖通用弹层：最小宽 544 px、最大宽 90%、最大高 90vh、header 36 px、body 可滚动、按钮 90×27 px。用途/降噪/距离选项不处于 Dashboard 祖先内，不能套 Dashboard 卡片的 hover/selected 边框。当前 GPUI view 的关闭按钮和 Cancel 进入快照恢复，遮罩点击不关闭；键盘、焦点、窗口关闭和全部中断状态仍要逐项与实际原组件核对及验收。

每个 PID 均使用自身实际目录的 14 个向导/录音资源：五个用途、三个环境、两个距离 AVIF，以及测量录音、录音、播放、停止 SVG。本轮对这 28 份原件的长度和 SHA-256 已与收据核对；共享 runtime key 不等于可借用另一型号图片，也不证明安装缓存中的转换像素或运行时加载已验收。

音频输入、语音/环境样本和录音内容均为本机能力；不需要账户、云目录或网络。官方原件与完整调用者可由 `audio-MapAudio-audit.json` 的 `seiren_eq.products[].complete_function_owners/complete_modules` 定位；其中旧 Rust 身份与 footer 范围说明仍需刷新，不能覆盖本次直接读取的 1446 MIC 证据，详见 [索引边界](playback-and-eq.md#验证与索引边界)。

源码获取、逆向语义、独立实现、UI/服务接线、静态/模拟验证与实际 DLL/设备对照分别记录。本轮仅静态读取与字节/语法核验，没有执行厂商 JS、应用、官方 DLL 或设备。尚需对每款实际 DLL 分别完成 HP/mic 读取顺序、meter/volume/mute 通知、sampling/mode 重配、return switch、所有 DSP 设置、失败、取消、恢复及释放的功能结果比对；PCM/系统音频行为还需当前设备与端点实测。没有设备时纯逻辑、模拟传输与有来源回放可先验证，但这些实际对照仍保持未验证，不能用模拟一致或双方均未连接结案。

## Seiren TUSBAUDIO 原生 API：设备句柄、音量/DSP 与 DFU 边界（静态捕获）

本批同时读取三个同名版本的 TUSBAUDIO API DLL。四线程只做原件校验、既有 IDA 捕获产物读取、导出/调用/字符串/xref 汇总；没有加载 DLL、打开设备、调用服务或执行 DFU。

| 原件（SHA-256） | 架构／大小 | 捕获收据 | 导出／函数／失败 | API 版本分支 |
| --- | --- | --- | --- | --- |
| RazerSeirenV3Chromaapi_x64.dll 966162446b255d393c1d272cc1fda3a1eac34c9b649854a61a6d2d77fb65c549 | x64，304,944 B，ImageBase 0x180000000 | [capture.json](../../../.analysis/results/native/966162446b255d393c1d272cc1fda3a1eac34c9b649854a61a6d2d77fb65c549/capture.json)，SHA-256 81ed8145c887c8d4ca642e7a0a3c5071f2c66e9a213829846e356cc831a82095 | 84／1,170／0 | CheckApiVersion(5, minor<=0xC)，GetApiVersion=0x5000C |
| RazerSeirenV3Chromaapi_x64.dll 0789707daaa4020a1bf80b5ff7d6e6aedd1f2b645afc2cd925ce06ae5d3a36c9 | x64，321,336 B，ImageBase 0x180000000 | [capture.json](../../../.analysis/results/native/0789707daaa4020a1bf80b5ff7d6e6aedd1f2b645afc2cd925ce06ae5d3a36c9/capture.json)，SHA-256 3c4237581777be020b5bf8e25d64ec4fdcc18ce43fd2dc3bf1a5e1e30265f036 | 84／1,129／0 | CheckApiVersion(5, minor<=0x10)，GetApiVersion=0x50010 |
| RazerSeirenV3Chromaapi.dll 1c34a749e3cc7deed34e5d0f1fc8fd13dbac659d4991b1e31ba7d4f958a0e475 | x86，281,904 B，ImageBase 0x10000000 | [capture.json](../../../.analysis/results/native/1c34a749e3cc7deed34e5d0f1fc8fd13dbac659d4991b1e31ba7d4f958a0e475/capture.json)，SHA-256 c921d1282414d52f02f261297aaa276a4eb5970500711386c034641ee6fb8b89 | 84／1,101／5 | CheckApiVersion(5, minor<=0xC)，GetApiVersion=0x5000C |

三份 DLL 的 84 项导出集合相同，差异主要在位数、内部 RVA 和 API minor 门禁。设备/请求入口在 wrapper 层把模块级 CriticalSection 传给内部函数；x64 主要是 __fastcall/指针宽度，x86 使用 __stdcall 或 __cdecl/32 位句柄，不能把一个 ABI 的声明直接复用到另一版本。x86 捕获保留五个 Hex-Rays 失败 RVA（0x145d0、0x15b00、0x21c04、0x27fe9、0x28d68），因此不能把其余导出数量写成全量伪代码闭合。

### 已由调用链静态确认的设备边界

- TUSBAUDIO_EnumerateDevices（x64 966：0x19a0，x64 078：0x19c0，x86：0x1770）刷新内部设备树，并在 PnP 路径中调用 SetupDiGetClassDevsW；TUSBAUDIO_GetDeviceCount 从内部指针区间计算数量。PnP 状态不可用的路径静态返回 -301989806，但没有设备运行结果。
- TUSBAUDIO_OpenDeviceByIndex／TUSBAUDIO_OpenDeviceByChannelIdString 先校验输出指针并把句柄清零，再检查已枚举列表、索引/通道字符串，分配设备对象、打开内部句柄并登记 map。已看到的错误分支包括非法参数 -301989823、未初始化 -301989776、越界 -301989817、分配失败 -301989856、登记失败 -301989855；通道查找的 1168 分支映射为 -301989759。这些是静态错误映射，不是现场设备返回值。
- TUSBAUDIO_CloseDevice（x64 966 0x1930、x64 078 0x1950、x86 0x1730）在锁内验证句柄和 map 槽，清除槽、解除设备对象、调用虚表清理与析构；陈旧/无效句柄静态返回 -301989816。TUSBAUDIO_UnregisterPnpNotification 清回调字段并注销内部通知，但未看到“设备已经停止”或硬件确认。
- 音量入口的静态请求布局已经闭合到共享异步 I/O helper：SetVolume（x64 966 0x2350、x64 078 0x2370、x86 0x1bf0）提交 12 字节输入，含两个通道/参数、16 位音量值和保留字段；IOCTL 为 0x8088225C。GetVolume（x64 966 0x2000、x64 078 0x2020、x86 0x1a80）将输出清零后以 12 字节请求/响应调用 0x80882258，再写回结果。SetMute/GetMute、SetDspProperty/GetDspProperty 共享同类 wrapper 和设备锁，但各属性的完整字段、协议应答与读回仍需独立回放核对，不能由导出存在推定完成。
- 共享 helper 使用 DeviceIoControl、OVERLAPPED 和事件；ERROR_IO_PENDING (997) 进入最多 1,000 ms 的 WaitForSingleObject/GetOverlappedResult 路径，超时或中止调用 CancelIo，最终非零状态静态映射到 -301989885。这是等待/取消代码路径，不代表传输成功或设备确认。
- TUSBAUDIO_RegisterDeviceNotification/ReadDeviceNotification 通过同一设备 helper；DFU 入口包含 StartDfuDownload（x64 966 0x2380、x64 078 0x23a0、x86 0x1c00）、Upload、Revert、Status 和 EndDfuProc，字符串表含 TUsbAudio_DFU_Event_%s 与多种 TSTATUS_*。LoadFirmwareImageFromBuffer/File、GetFirmwareImage 和 UnloadFirmwareImage 形成内存映像生命周期；x64 卸载清理指针和大小，x86 0x1c70 释放 unk_100414B0 并清零大小后返回 0。未执行刷写、回滚、传输读回或恢复。

### 迁移和验证边界

以上只证明原件中的导出、内部调用目标、错误/释放路径和 Win32 I/O 边界。它没有证明任何设备实际存在、服务已连接、IOCTL 得到成功应答、DSP/音量值已写入，或 DFU 已完成。Rust 侧必须按每个 API minor 分支、x86/x64 ABI、句柄代次、读/写/取消/超时/释放分别建模；随后用有来源的应答回放和显式官方 DLL 对照测试记录输入、输出、状态变化、读回、失败、取消、恢复及清理。当前三份 DLL 与 Rust 的运行结果均为 **未验证**。
## MixerSDK 原生模块：RzNative_053E 的设备枚举、HID 与属性控制

本节是对当前捕获的 `RzNative_053E_v1.0.16.0.dll` 的静态语义记录。它与前面的 TUSBAUDIO API 是不同的原生链：此模块导出 MixerSDK 包装和一组 hidapi 风格函数，不能因为都属于音频页面而合并 ABI。

| 原件 SHA-256 | 架构/大小 | capture SHA-256 | 函数/Hex-Rays 失败 | 导出 |
| --- | --- | --- | --- | --- |
| `d3cf6173a2e8bb7af0a1066da3bd9d6038270884b6fcdc5afdaefd349755da5d` | x64, 2,922,184 B | `f4329cca2430a2cd01fee72e6832999fd12d6623c078fd539548cbc62a08e3ef`（[`capture.json`](../../../.analysis/results/native/d3cf6173a2e8bb7af0a1066da3bd9d6038270884b6fcdc5afdaefd349755da5d/capture.json)） | 10,432 / 1 | 49 |

### 初始化和设备列表

- `Initialize`（RVA `0x35230`）在全局锁内做幂等检查；已初始化时直接返回成功。首次调用依次进入 `MixerSDKLib_Init`、最多十轮的 `CreateDeviceList`、render 类型 `GetDeviceCount(0)`、capture 类型 `GetDeviceCount(1)`，再以参数 `0x1532, 0x053E` 调用 `MixerSDKLib_OpenHID`。失败轮次间隔 500 ms；只有 HID 打开并设置内部 active 标记后才继续写入八次 `RazerT2ResetStream`，最后置 initialized 标志。任一步失败都返回失败并保留错误日志。
- `MixerSDKLib_Init`（`0x334a0`）加载依赖库并解析 `CmMixerLibInit`、`CmMixerLibUnInit`、`CmMixerGetDeviceById`、`CmMixerGetDeviceCount`、`CmMixerCreateDeviceList`、`CmMixerPropertyControl`、`CmMixerOpenHID`、DSP 固件版本、回调和日志入口；分配四个 1024 字节工作缓冲，创建隐藏窗口线程。依赖加载、导出解析或 `CmMixerLibInit` 失败均返回 0，不能把对象分配成功当作初始化成功。
- `MixerSDKLib_CreateDeviceList`（`0x34420`）只在依赖函数存在时调用底层列表；底层非零错误映射为 0。`GetDeviceCount`（`0x34180`）对类型 0 要求返回数量为 4、类型 1 要求为 5，否则记录 `Incomplete Type` 并返回 0。`GetDeviceById`（`0x33e50`）将底层四字段 `ID/JackType/DataFlow/DeviceState` 格式化成 JSON 字符串；未初始化返回错误码 `131073`，底层错误原样传给错误包装器。

### HID、属性和通知

- `MixerSDKLib_OpenHID`（`0x34ff0`）先调用底层 `CmMixerOpenHID`，成功后再进入内部 `InitMixerHIDDevice`；两步任一失败都返回 0。导出的 `hid_init`、`hid_exit`、`hid_enumerate`、`hid_open`/`hid_open_path`、`hid_close`、`hid_read`/`hid_read_timeout`、`hid_write`、`hid_send_feature_report`、`hid_get_feature_report`、`hid_set_output_report` 和字符串 getter 与此链并存，调用约定不能从名称猜测。
- `MixerSDKLib_PropertyControl`（`0x345e0`）要求 SDK active、属性名和输入值均非空；准备两个 1024 字节缓冲及额外参数，在共享 mutex 下最多重试三次，每次后按 `10 * attempt` ms 退避。失败返回 `Call CmMixerPropertyControl failed`；读路径会特殊处理 `RazerT2DSPEQBandControl` 和 `RazerT2GetPeakValue`，其余读结果从内部缓冲生成返回对象。临时字符串和两个工作缓冲在成功、失败和参数错误路径都清理。
- `SetNodeFFIEvent`（`0x37320`）仅在 active 且回调非空时保存回调指针并返回 1，否则返回 0。导出表还含 mixer 输出峰值回调开关、DSP/固件版本和分配器入口；这些函数的完整消费者和跨线程回调时序尚未在本捕获中闭合。

### 释放、失败和未验证边界

- `MixerSDKLib_UnInit`（`0x33b00`）先 `FreeLibrary` 底层模块，再 `CancelIo`、关闭 overlapped 事件和设备句柄、释放本地内存、释放 mutex、停止 timer、销毁隐藏窗口并调用 `CmMixerLibUnInit`。底层反初始化失败会记录错误并返回 1，但清理标志仍按原路径更新；`Terminate`（`0x35730`）据此清理日志状态并清除顶层 initialized 标志。
- 该模块含 `DeviceIoControl`、`CreateFileW/A`、读写、线程、事件和 62 个 `CloseHandle` 相关导入，并包含 `device or resource busy`、`already connected`、`not connected`、协议错误和 `Call CmMixerPropertyControl failed` 字符串。一次 Hex-Rays 失败为地址 `0x1801e42d5` 的 `call analysis failed (code -12)`，原始字节仍保留，不能把 10,432 个函数宣称为全部已反编译。
- 本次只确认导出、内部调用、锁/重试/释放和错误分支；没有执行 HID、MixerSDK、IOCTL、DSP/EQ 写入或设备读回。Rust 端仍需按设备类型、属性名、回调代次、取消和 `UnInit` 清理建立独立状态，并用只读回放及官方 DLL 对照记录真实应答；当前运行结果为**未验证**。
## RzAudioUtil_v1.0.1.1.dll：THX 音频 JSON dispatch 与媒体命令边界（静态捕获）

原件 [RzAudioUtil_v1.0.1.1.dll](../../../.analysis/inputs/official-native-current/blobs/012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11/RzAudioUtil_v1.0.1.1.dll) 为 x64 PE，706,248 字节，SHA-256 012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11，Image Base 0x180000000。IDA 捕获 [capture.json](../../../.analysis/results/native/012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11/capture.json) SHA-256 92ed9a6f9651fb6c67ece87ed97667b4c885f23d8fd173eca97d720500608b2d，2,339 个函数、1 个 Hex-Rays 失败（0x1800673a3，call analysis code -12），导出 9 个入口。

- [GetDLLVersion 0x5030](../../../.analysis/results/native/012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11/functions/5030.json) 把版本格式化为 1.0.1.1，以 DLL 内部分配器返回字符串；调用者应以 [FreeMalloc 0x4e80](../../../.analysis/results/native/012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11/functions/4e80.json) 释放非空指针。SetNodeFFIEvent 只登记节点事件入口并返回 1，捕获未证明回调何时触发或由谁 join。
- [Dispatch 0x51f0](../../../.analysis/results/native/012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11/functions/51f0.json) 解析 JSON 请求的 cmdClass 字段并要求它是 string；未知类别构造 error，已识别 handler 以返回码非负时标记 status: success，否则标记 status: error，最后序列化 response 并返回新分配的 C 字符串。类型错误、JSON 异常和 handler 错误在静态代码中均有独立分支，不能把返回指针非空当作业务成功。
- [dispatchMediaPlayer 0x37240](../../../.analysis/results/native/012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11/functions/37240.json) 从 command 字段分派至少 Open、Close、IsMediaReady、CloseVideoWindow 以及其余媒体 lambda；[dispatchMediaRecorder 0x1620](../../../.analysis/results/native/012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11/functions/1620.json) 当前直接返回 0。捕获字符串还包含 device or resource busy、resource unavailable try again、not connected、protocol error、文件 reopen/flush/write 失败和播放错误文本，说明错误映射存在，但没有从该 DLL 单独闭合到设备读回。
- Dispatch 的 response 还会写入 status 和 handler 返回的 result，并通过日志记录 req -> resp；分配、异常清理和字符串生命周期由 nlohmann JSON/内部 malloc 路径管理。Rust 端需要把输入类型错误、未知 cmdClass、handler 错误、空结果和释放分别建模。

本批未执行媒体设备、音频会话、官方 DLL 或服务；Hex-Rays 的一个失败函数保持原始失败收据，未用猜测伪代码补齐。真实播放/录音、取消、恢复、读回和释放仍为 Unverified。

## RzNative_0542_v1.0.25.0：Camy 麦克风 HID/TUSBAUDIO 链（2026-10-24 四线程静态捕获）

[RzNative_0542 capture](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/capture.json) 的原件 SHA-256 为 b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c，原件 639624 字节；capture SHA-256 为 65b7d807757bfd1188d7886046c5c0b83a0cb47a04cd0f114ba2e5e7541eacab，包含 2472 个函数、0 个 Hex-Rays 失败和 44 个导出入口。四线程批次收据在 [camera-audio-native-batch analysis.json](../../../.work/camera-audio-native-batch-20261024/analysis.json)。独立 PE 映射确认 2478 个唯一 chunk、112768 个唯一指令和 458516 个唯一 chunk 字节均与原件一致；该计数只描述本版本，不与其他 Camy 或 Seiren 版本相加。

### 设备登记、退出与分配器

DeviceInitialize（RVA 0x3c740）调用管理器 0x3a280。管理器拒绝空 ID，按宽字符串比较枚举出的 96 字节记录，已有有效句柄时复用，否则创建共享 CCamyDevice、执行 open 0x4650，只有打开成功才加入活动列表。DeviceTerminate（0x3c820）转发到 0x3a700，删除匹配列表节点并递减共享引用；当前导出层没有把底层 TUSBAUDIO/驱动句柄的最终关闭完整展开，不能只用返回布尔值代表硬件已经关闭。SetNodeFFIEvent（0x3c4d0）在全局保存回调指针并返回 1，没有显示等待在途通知的 join。FreeMalloc（0x3c4e0）对非空指针直接调用 free 并返回 1。

### 8 字节 HID 音量、静音和滤波器报文

SetMic_Volume（0x3f450）构造 8 字节输出报文。初始字节为 f0 00 01 00 00 00 00 00，输入音量写入 byte[3]，byte[7] 为 0x0f - byte[4] - byte[5] - byte[6] - byte[3] 的低八位，然后只检查 HidD_SetOutputReport 的布尔返回值；没有同一导出的读回确认。GetMic_Volume（0x3f630）先发 f0 10 01 00 00 00 00 ff，请求 byte[2] 为 1，最多五次间隔 30 ms 的输入报告轮询，要求首字节为 f0 并以 byte[2] 匹配请求结束。循环结束后只检查最后一次 GetInputReport 的 BOOL，再返回 byte[3]，因此最后一帧状态不匹配也可能返回一个值；设备不存在、发送/接收失败和真实音量 0 在该接口上可能都表现为 0。

SetMic_Mute（0x3f850）使用 f0 00 02 value 00 00 00 checksum，其中 checksum 是 0x0e - value；GetMic_Mute（0x3fa30）发送 f0 10 02 00 00 00 00 fe 并沿用五次、每次 30 ms 的轮询。SetMic_HPF（0x3ecf0）写内部命令 0x6810，GetMic_HPF（0x3ee70）发送 f0 10 03 00 00 00 00 fd；SetMic_Limiter（0x3f0a0）写 0x6890，GetMic_Limiter（0x3f220）发送 f0 10 04 00 00 00 00 fc。四组 getter 都使用最后一次输入报告的 BOOL，而不是强制要求最终 byte[2] 匹配；Rust 实现必须保留未观测、失败与实际 0 的三态，不能把该返回值直接当作已确认设备状态。

SetMicMonitorEnable（0x40100）查找设备和 TUSBAUDIO 接口，把 payload byte 写成 a2 == 0 的反值语义，并调用 TUSBAUDIO_AudioControlRequestSet，选择器为 11、长度 1、超时 100；底层返回 0 才被包装为成功。这个函数体没有证明完整 DSP 参数集，也没有读回。SetMicMonitorLevel、SetLevelMeterEnable、SetLevelMeterPumpInterval、SetMasterSampleRate、SetMode、MixEnable/MixLevel、DeviceVolume/DeviceMute、StreamMixMonitor、PlaybackExternal 和 RouteExternal 均继续走设备对象内部命令；当前收据只确认导出到内部调用的方向，未把每个命令的服务端应答闭合。

### 录音、路由与失败边界

GetLevelMeter（0x3d180）和 GetRouterStatus（0x3dd40）返回分配的 JSON；Windows 播放、录音和音频设备列表分别由 0x3d760、0x3da50、0x3d470 提供。StartBackGroundRecording（0x40620）要求设备及对象偏移 56 的录音状态并转发 0x56ec0；StopBackGroundRecording（0x40780）对未知设备返回失败、对没有录音对象的已知设备返回成功，否则转发 0x575c0。SetMicMonitorEnable 的成功条件、音量/静音 setter 的只写语义和 getter 的轮询缺陷必须在错误、取消、重连和释放测试中分别记录，不能用本地缓存或模拟报告伪造读回。

### 后台捕获的启动、停止与线程清理

本小节仅适用于上述 `0542 v1.0.25.0` 原件及保留的 `AudioCamyT1` 调用分支。新增的[静态核验收据](../../../.analysis/results/native-bulk/audio-semantic-verification.json)以 4 个 Python worker 并行核对原件、完整函数和前端 AST：8 个完整函数、914 条指令、3,953 字节指令及 2 份前端原件的 6 个完整所有者均通过实际字节／获取身份核对；这不是 DLL 执行、Rust 测试或设备验收。

- **请求与消费者**：保留的 [AudioCamyT1](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/AudioCamyT1.6bbbde5180d72769f276.js) 完整赋值节点 `[15420,15694)`、`[15695,15966)` 仅在 `isInit` 时以 `containerId` 调用对应导出，并原样返回布尔结果；未初始化返回 false。其 `init` 完整方法 `[18437,21435)` 声明两项均为 `bool(string)`，默认选择 `RzNative_0542.dll`。上层 [6259 MW 原件](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/6259.2eb539ec769147cd201d.js) 的完整变量声明 `[1883091,1883433)`、`[1883434,1883776)` 分别先 `onEvent/offEvent`，再以 `Promise.allSettled` 并发开始／停止后台捕获与 meter；聚合结果未被检查，不能将动作结束当作两项都成功。该 MW 捕获还包含其他产品分支，本小节不推断这些分支都使用 `0542`。
- **启动成功的边界**：`0x56ec0` 已 active 时直接返回 1；否则尝试取得对象 `+96` 的 mutex，获取真实 endpoint，激活音频客户端，读取 mix format，以共享模式、flags `0xC0000`、buffer duration `200000`、periodicity `0` 初始化，取得 buffer size、设置事件并获取 capture client。原件 GUID `0x7ac30/0x7ac20` 的 16 字节分别与 `IAudioClient/IAudioCaptureClient` 一致，虚表槽位另与 Windows SDK 接口声明核对。准备失败的正常分支关闭事件、Release 已获取接口并释放 format。准备成功后设置 active、保存事件，再创建 `0x57910` 线程；返回 true 只覆盖已 active 或线程创建完成的分支，**不确认工作线程中的 `IAudioClient::Start` 成功**。
- **它消费包，不提供录音 PCM**：完整工作函数 `0x57720` 调用 `IAudioClient::Start`，但不检查其 HRESULT；循环先检查 active 与上次 `GetBuffer` 的 HRESULT，再无限等待事件，不检查等待返回值。每次 `GetBuffer` 成功后仅以实际 frame count 调用 `ReleaseBuffer`；PCM 指针、flags 和时间位置没有在该完整函数中复制、持久化或发送给 UI。因此这条后台链不能充当页面录音／波形的 PCM 来源，也不能与前述浏览器录音链合并。`ReleaseBuffer` 的 HRESULT 同样没有参与最终布尔结果。
- **停止与释放的所有者**：`0x575c0` 的完整汇编在 `0x575e3` 以 byte `xchg` 原子清 active。保存的 event 非空时执行 `SetEvent`；thread ID 非零则拒绝自 join，并调用 `_Thrd_join`，正常完成后清线程句柄／ID 与事件槽；无 event 的正常路径也返回 1。资源实际由 worker 退出路径释放：`0x5782e` 原子清 active → `IAudioClient::Stop` → `CloseHandle(event)` → Release audio client → Release capture client → `CoTaskMemFree(format)`；线程入口 `0x57910` 最后广播线程退出条件并释放参数块。对象析构 `0xa990` 再调用同一 Stop，然后销毁 mutex、释放两个字符串并释放对象，仍有 joinable thread 时走 `terminate`，不是静默丢弃任务。
- **失败不能升级成确认**：Stop 忽略 `SetEvent` 的 BOOL；worker 忽略 Start/Stop 与 ReleaseBuffer 的 HRESULT。自 join 与 join 失败分别抛 C++ error 5／2；导出 catch 保留初始 false。worker 可因 `GetBuffer` 失败自行退出并关闭 event，此时对象保存的 event 槽要到后续 Stop 才清零。以上必须在模拟测试与实际 DLL 对照中分别覆盖；不能把导出的正常路径 true 解释为有效 PCM、API 成功或设备应答。线程创建失败后的异常展开、其他版本和完整设备注销的间接析构分支不由本小节宣称闭合。

### 电平包、事件消费者与路由状态的实际差异

同一[核验收据](../../../.analysis/results/native-bulk/audio-semantic-verification.json)的 `meter_router_chain` 另含 2 个模块的 26 个完整函数、3,874 条指令／17,208 字节，以及 7 个完整前端 AST 所有者的实际字节校验。本节从 `0542 v1.0.25.0` 继续追入同一 PID 驱动包中的 x64 API 原件，不把前述其他音频 API 版本视为等价实现。[驱动 INF](../../../.analysis/inputs/official-native-current/extracted/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/Driver_Setup/W10_x64/RazerSeirenV2Pro.inf) 第 40 行登记 `VID_1532&PID_0542`，第 85 行的 InterfaceGUID 与原生 `0x5ad30` 使用的值一致；这证明保留包的静态对应关系，未证明当前系统注册表实际选中了该文件。

| 边界 | 完整原生函数与实质语义 |
| --- | --- |
| 设备与驱动选择 | 两项导出都先以 `0x3a080` 按不区分大小写的 container 查找活动设备。电平控制使用对象 `+32` 的驱动持有者，路由查询使用 `+40` 的路由持有者。驱动初始化 `0x5ad30` 经 `0x2a80` 查 `HKCR/CLSID/{接口 GUID}/InprocServer32`，`0x1ed0` 解析 API；底层设备实际以 `TUSBAUDIO_OpenDeviceByIndex(0, out_handle)` 打开，不是再次按 container 精确选择。多设备时两层身份是否一致仍需实际对照，不能擅自把索引 0 解释为已确认的目标设备。 |
| 电平原始包 | `0x4130` 构造两个小端 u32：`77,301`，随后为每通道 8 字节记录：`direction:u32, group:u8, channel:u8, level:i16`。direction 为 0／1，group 为 0／1／2，channel 为该组序号；读取使用记录偏移 `+6` 的有符号 16 位电平。`0x42b0` 按组与数量选择相应记录区间，不能用 UI 列表下标代替组或通道身份。 |
| 真实 DSP 调用参数 | `0x5bfb0` 调用 API 表 `+496`。`0x1ed0` 的完整汇编将 `GetProcAddress("TUSBAUDIO_GetDspProperty")` 存到对应槽。调用前的 `0x5c00c–0x5c033` 明确保留 `RCX=handle, RDX=buffer, R8=length`；Hex-Rays 显示的单参数调用遗漏后两项，不能据此生成 ABI。驱动 [Get 导出 `0x1cc0`](../../../.analysis/results/native/adaf8f7d7455b6fa628752bd692fa6b65a1d341fde07962fa12ad9eac30a9f3f/functions/1cc0.json) 的寄存器搬运与内部 `0xd560` 也确认三参数。 |
| IOCTL 与等待 | 同包 x64 API 的 `GetDspProperty 0x1cc0 → 0xd560 → 0x3d60` 使用 `0x80882200`，同一 buffer 同时作输入／输出；Set 的 `0x2120 → 0x113e0` 使用 `0x80882204`，仅输入。使能包是 12 字节的三个 u32：`77,300,enable`。空 buffer／长度、无效句柄、无底层句柄分别映射 `-301989823/-301989816/-301989837`。两项均向等待层传 `0xffffffff`；`0x6d00` 每次等待 1,000 ms，超时继续循环，**不是总计 1,000 ms 超时**。后续未决取消路径仍经 `CancelIo/GetOverlappedResult` 循环收尾，不能把已有通用超时描述套到此分支。 |
| JSON 与数值 | `0x60f0` 生成 `{"meterInfo":[{"deviceId":…,"levelMeter":[…]}]}`。每项为 `truncate((clamp(raw/100.0,-60,0)+60)*100/60)`，按本函数浮点指令运算：`-6000/-3000/0` 分别映射 `0/50/100`。非法记录指针／索引使用 `-12000`，也会映射 0；这不是实测静音。内部模式字段 `+72==1` 时设备 ID `131074` 强制为 0；另一模式字段 `+80==0` 时只填设备 ID `262145`，预分配列表的其他默认记录可保留 deviceId 0，不能当成真实设备。底层查询失败和未知 container 返回分配的 `"{}"`；分配失败才可能返回 NULL。 |

**轮询与停止**：`0x51a0` 是独立 meter 线程，由 `0x5190` 转入；仅在回调已登记且本地 enabled 标志开启时读取并发送事件。读到 `"{}"` 时重新构造请求并使能，不发电平事件。事件完整封套是 `event:"deviceCamyT1Changed"`、`eventType:"levelMeterPumpup"`、真实 `containerId`、`levelMeters:{meterInfo:…}`。该线程还按启动时计算的计数阈值检查后台 capture 状态并尝试重启，不能将一次捕获停止当成此 meter 循环也已停止。`SetLevelMeterPumpInterval 0x40480` 接受小于 `0xffffff` 的 unsigned 值，低于 180 时钳为 180，再原子保存并 `SetEvent`；未检查 `SetEvent` 结果。`SetLevelMeterEnable 0x3fdf0 → 0xa530` 先修改本地标志再调用底层，同值直接返回 true；底层对象仅在 SetDspProperty 成功时更新自身 active，禁用还清电平 buffer 长度。因此本地请求标志与底层确认可以不一致。设备断开 `0x4e90` 清 enabled、把 interval 原子设为 `0xffffff`、唤醒线程，但最多等 1,000 ms；超时只记录日志后继续关闭句柄，并非已证明所有无限等待的驱动 I/O 都已 drain。

**两种前端消费者并不相同**：[AudioCamyT1](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/AudioCamyT1.6bbbde5180d72769f276.js) 的 `getLevelMeter` 完整节点 `[6786,7251)` 仅检查返回值不等于空字符串，就设 `STATUS_SUCCESS`、解析 JSON 并补 `deviceIdName`；所以 `"{}"` 也会被包装成 success，不能据此认定有观测值。事件处理完整节点 `[2586,2941)` 先拒绝不匹配的 container，再补字段 `deviceIdEnum` 并发布事件。[MW 事件消费者](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/6259.2eb539ec769147cd201d.js) 完整声明 `[1154444,1155863)` 从 `levelMeters.meterInfo` 取数组、过滤 deviceId 0，再向 UI 发布；缺数组时用空列表呈现，不证明设备返回了零电平。Rust 的未观测／失败状态不能从这些显示回退值推导为成功。

**GetRouterStatus 的 ABI 不一致必须保留**：`0x3dd40 → 0x8230` 的实际返回是新分配的 JSON C 字符串，不是 bool。找到路由节点时字段为 unsigned 数字 `status`（节点内部 kind `+256==2` 时为 1）及 `deviceStatus`。`0x593d0` 根据节点内两份 endpoint ID 经 `IMMDeviceEnumerator::GetDevice` 查询，两个 `IMMDevice::GetState` 输出都等于 1 时 `deviceStatus` 才为 1；输出先置 4，但两次 GetState 的 HRESULT 未检查，JSON 所有者也忽略 helper 的 HRESULT。缺持有者／节点／设备时为 `"{}"`。当前 `AudioCamyT1.init` 却把此导出声明为 `bool(string,long)`，`getRouterStatus` 完整节点 `[8432,8682)` 只透传结果，不解析 JSON。不能把这个 bool 解读为任一 JSON 字段，也不能靠猜测改变导出签名或宣称原版结果一致。对保留 MW 中同名成员调用的有界 AST 核对仅找到委托包装，未找到解析此 JSON 的实际业务消费者；动态／计算属性调用和其他版本仍须继续追踪。以上释放以实际 malloc 输出和 `FreeMalloc 0x3c4e0` 为边界，未执行该不匹配声明。

本轮没有加载 DLL、打开 TUSBAUDIO、发送 HID 报文或执行官方读回。Rust 独立实现与官方实际调用结果、设备应答、初始值恢复、取消和完整句柄释放均保持 Unverified；没有设备时只能用独立模拟传输和有依据的回放覆盖报文编码，不能把回放一致当成硬件验收。

### DSP 内核插件、样本来源与缓存电平

上述 DSP IOCTL 已继续追入同包的主驱动与混音插件。[本轮收据](../../../.analysis/results/native-bulk/audio-semantic-verification.json)的 `kernel_meter_chain` 以 4 个 Python worker 复用完整捕获，核对 2 个模块、32 个完整函数、5,617 条指令／23,723 字节和 14 个原始虚表槽。主驱动 SHA-256 为 `20a95b09c53e880b96c2a18f48faf6eaecbd493fb861d5a6fdef2daee87dab2e`；[混音插件捕获](../../../.analysis/results/native/7c8d1f4f8fe45ee44bcd1610312d791c76b73e3c3688d92ad972ba575853f6c3/capture.json)对应 `RazerSeirenV2Pro_mixer.sys`，SHA-256 为 `7c8d1f4f8fe45ee44bcd1610312d791c76b73e3c3688d92ad972ba575853f6c3`，版本资源为 `5.30.0.0`。两份捕获的 Image Base 都为 `0x140000000`；这仍是保留驱动包的静态身份，不是当前机器的实际加载结果。

- **间接调用的真实目标**：主驱动 [工厂获取 `0x30c70`](../../../.analysis/results/native/20a95b09c53e880b96c2a18f48faf6eaecbd493fb861d5a6fdef2daee87dab2e/functions/30c70.json)通过附加设备栈的 QueryInterface 请求取得插件接口；混音插件 [`0x5000`](../../../.analysis/results/native/7c8d1f4f8fe45ee44bcd1610312d791c76b73e3c3688d92ad972ba575853f6c3/functions/5000.json)逐项核对 GUID `ea47cd8b-ed19-429e-a183-af9f8e3e33ee`、size `40`、version `1310`。主驱动还核对插件 API version `327710`。插件工厂虚表 `0x85b8` 的创建／删除槽实际为 `0x4de0/0x4e50`；创建沿 `0x212c → 0x1d5c` 分配并安装对象虚表 `0x8308`。该表 `+96/+104/+112` 的实际 PE 指针依次为 Set `0x3140`、post-set `0x2570`、Get `0x23e0`。没有从默认虚表或文件名猜测实际目标。
- **内核应答与副作用**：主驱动 [`0x21ed0`](../../../.analysis/results/native/20a95b09c53e880b96c2a18f48faf6eaecbd493fb861d5a6fdef2daee87dab2e/functions/21ed0.json)要求设备状态为 1；Get 另要求输入／输出长度相等，buffer 非空、长度非零。状态未就绪、空 buffer、长度不符／为零和默认 DSP 对象分别返回 `-301989805/-301989823/-301989821/-301989840`。Get 只在插件返回 0 时填输出长度；完成时写入 `IoStatus.Information`，非零内部错误经 `v3 | 0x20000000` 写入 `IoStatus.Status` 后完成 IRP。Set 在自旋锁内调用 `+96`，释放锁后仍调用 `+104`；**Set 失败也会到 post-set hook**，不能把 hook 发生当作写入成功，也不能省略这次状态更新调用。
- **使能只是算法开关**：[`0x3140`](../../../.analysis/results/native/7c8d1f4f8fe45ee44bcd1610312d791c76b73e3c3688d92ad972ba575853f6c3/functions/3140.json)要求命令 300 长度至少 12 字节，随后仅把 `u32[2] != 0` 保存到对象 byte `+312` 并返回 0；[`0x23e0`](../../../.analysis/results/native/7c8d1f4f8fe45ee44bcd1610312d791c76b73e3c3688d92ad972ba575853f6c3/functions/23e0.json)的同命令读取此开关。构造初值为关闭。该响应不提供新样本，不是固件电平读回；关闭开关也不等于清零各通道保存的电平。
- **301 读取缓存，可部分修改输出**：Get 命令 301 沿 [`0x33c0`](../../../.analysis/results/native/7c8d1f4f8fe45ee44bcd1610312d791c76b73e3c3688d92ad972ba575853f6c3/functions/33c0.json)遍历 `floor((length-8)/8)` 个记录，不额外要求余数为 0。direction 为 0 选择对象 `+288` 数组，任一非零值选择 `+296` 数组；这不是严格的 0／1 枚举校验。每项先由 `0x345c/0x34ac` 校验 group／channel，再把对应 32 字节状态的 i16 `+16` 复制到请求记录的 `+6`。未知 group／越界 channel 返回 `-301989823/-301989817`；后项失败前，前面的记录可能已改写，不能将错误输出当作一帧完整有效观测。此 Get 没有刷新样本，也不要求开关处于开启状态。
- **真正的样本生产者**：主对象 `+3816` 等于 stream 子对象 `+2720` 内的 `+1096`；构造和消费者共同证明两处使用的是同一个选定 DSP 对象。主驱动 `0x46870/0x46f30` 在普通处理路径调用虚表 `+64/+72`，实际到混音插件 `0x2590/0x2c80`；插件仅在 byte `+312` 开启时把有符号 32 位 stream 样本送入 [`0x12d0`](../../../.analysis/results/native/7c8d1f4f8fe45ee44bcd1610312d791c76b73e3c3688d92ad972ba575853f6c3/functions/12d0.json)。每通道状态由 `0x100c` 初始化为窗口／剩余样本数 `64/64`、累加值 0、电平 `-12000` 和三项滤波状态 0。算法按 `sample >> 8` 的平方累加，经过窗口均值、`0x1694` 的查表数值变换、`0x1034` 的带饱和三状态滤波及 `0x14ac` 的归一化多项式变换，最后以常量 `19266/7936/600` 和下界 `-12000` 写入 i16 电平。完整移位、符号、饱和、查表原始字节及状态均已保留，不能用通用浮点 RMS／对数近似代替并声称数值等价；独立 Rust 的逐位运算及实际 DLL 差分仍待验证。
- **旧值与释放必须分开判断**：stream 的 `+4281` 分支绕过插件，`+4280` 分支改调 `+80/+88`，这两个槽在当前混音插件都指向空操作 `0x1030`。因此即使使能与 Get 均成功，也可能读到初始化值或旧值，不能据此认定刚取得设备电平。退出 [`0x21860`](../../../.analysis/results/native/20a95b09c53e880b96c2a18f48faf6eaecbd493fb861d5a6fdef2daee87dab2e/functions/21860.json)先将选定指针重置到内嵌默认对象，再调用实际插件 `+24`（本插件为空操作）和工厂 `+8`。工厂随后经 `0x4e50 → 0x1f50 → 0x1e88 → 0x328c` 释放两份电平数组及 stream buffer，并清持有者；这些释放语句不单独证明所有在途回调／I/O 已停止。

本节闭合了该包内「原生 Get/Set DSP → API IOCTL → 主驱动 → 已核实插件 → stream 样本算法／缓存 → 响应与释放」的静态目标与相关分支。它不证明设备实际加载了此插件、当前 stream 已开始、缓存值具有新鲜性，亦不替代全产品／版本分析。实际官方执行对照、Rust 数值一致性、真实设备状态与取消／恢复仍保持 Unverified。

### 外接路由、PCM 桥接与失败恢复

该分支此前仅确认导出方向；现已沿相同原件继续核对完整实现。[收据](../../../.analysis/results/native-bulk/audio-semantic-verification.json)新增 `external_route_chain`：4 个模块、67 个完整函数、7,663 条指令／31,239 字节，以及 2 份前端原件的 6 个完整 AST 所有者。它与电平批次存在复用函数，不将各批数量相加当作唯一函数数。

- **前端可能掩盖原生失败**：[AudioCamyT1](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/AudioCamyT1.6bbbde5180d72769f276.js) 的 route wrapper `[10330,10607)` 在已初始化时把 `containerId/deviceId/name` 原样传给 `SetRouteExternalDevice`，否则返回 false；playback wrapper `[9804,10066)`同样透传。上层 [MW](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/6259.2eb539ec769147cd201d.js) 的 `bindExterDevice` 完整节点 `[2145075,2145551)` 处理 ID 和自绑定检查后，虽然 await 原生布尔结果，却仅记日志并固定返回 true。`initDevice` `[2143385,2144777)` 依序绑定输出、写音量／静音、遍历输入并绑定外接设备，不核对这些布尔结果。不能以 MW 返回 true 或预设初始化结束判断真实路由成功。`fullInit` `[1197,1804)` 和 `initStreamMixerSettings` `[1805,2585)` 是明确存在的设置方法；它们不证明任意首次连接都应执行默认值覆盖。
- **先改 DSP，再替换路由，不是事务**：[`SetRouteExternalDevice 0x3fc60`](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/functions/3fc60.json) 拒绝 NULL 名称，但允许空字符串。`0x3bae0 → 0xba80` 按 code page 65001 转为 UTF-16；名称包含 `Razer Seiren V2 Pro` 时被清为空。device ID `131074` 先经 `0x5c580` 改写 playback external 标志并重算 DSP，忽略该结果，然后才执行路由。独立 `SetPlaybackExternal 0x3ff70` 同样经 `0x5c580`：标志在校验之前更新，且 `0x5d0c0` 的 DSP 写入结果没有进入其最终 0 返回。两者都可能留下部分状态，不能自动补造回滚或声称全部已确认。
- **停止旧路由后才建新路由**：[`0x582c0`](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/functions/582c0.json) 在路由 mutex 内先停止／释放已有桥接、清 kind 并删除旧节点；空名称随后作为解除绑定返回成功。非空名称新建节点，经 [`0x58890`](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/functions/58890.json) 初始化，owner 已 active 时再准备／启动。端点以名称子串及枚举顺序选择：Aux 1／2／3 与 `Playback Mix` 各有分支，不是以 UI 列表序号或完整 endpoint ID 精确匹配。初始化失败不恢复已删除的旧路由；新节点可停留在 kind 0／1。kind 1 表示端点绑定，kind 2 表示准备与 worker 请求被接受，正是前述 `GetRouterStatus.status` 的来源，**不是已确认 PCM 正常传输**。
- **真实底层包含系统音频与 DSP 两条链**：`0x68830` 用两个实际 endpoint ID 获取 `IMMDevice`，经 `IMMEndpoint::GetDataFlow` 要求前者 capture、后者 render，再以 AddRef／Release 持有；helper `0x6a330` 未检查 GetDataFlow 的 HRESULT，其输出先置 2。准备 [`0x6a950`](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/functions/6a950.json)先释放旧客户端／worker，激活两个 `IAudioClient`、读取 mix format、按内部模式选择格式，再以 shared mode 和 flags `0x880c0000` 初始化两边、设置事件、取得 capture/render client 与 clock。`0x6a070` 的 Hex-Rays 伪代码遗漏 Activate 输出参数并显示成保存 NULL；实际 `0x6a0bc–0x6a0ef` 的完整汇编证明 stack out pointer 及真实返回指针保存，不能按该伪代码重建错误 ABI。IAudioClient／CaptureClient／RenderClient／Clock 和 IMMEndpoint 的原始 GUID 字节已分别核对。
- **桥接 worker 才传输 PCM，启动返回早于它**：`0x6ba10 → 0x6c290` 创建线程后设置 active；ResumeThread 失败只读取错误，不阻止返回 0。[`0x6bfa0`](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/functions/6bfa0.json)在线程内依次 Start 两个客户端，只有两项都为 0 才进入 stop／capture／render 三事件循环，启动失败直接退出，未把异步失败反映回路由 setter。capture 消费者 `0x6b0e0` 将实际 PCM 复制到有界环形缓冲，再按实际 frame count ReleaseBuffer；它接收 flags，但没有依据 silence flag 的补零分支。render 消费者 `0x6b3f0` 按 padding 获取空间，处理启动静音、占用量与漂移，复制环形数据，再 `ReleaseBuffer(frame_count,0)`。捕获／输出错误分别更新内部计数和错误字段，不能由 setter 的 true 代表后续样本无误；该桥接与前述只消费、不复制 PCM 的后台捕获是不同链。
- **DSP 包与应答边界**：`0x5d0c0 → 0x3f10` 产生小端 `u32[77,201]` 加 6 字节记录 `group:u8/channel:u8/gain:i32`；通常 gain 为 `(volume<<24)/100`，mute 为 0。`0x5d430 → 0x5d560 → 0x3d60` 产生 `u32[77,200]` 加 8 字节记录 `input_group/input_channel/output_group/output_channel/gain:i32`，均经已核实的 SetDspProperty／`0x80882204` 内核链。混音插件 `0x3d80/0x3e40` 将增益夹到 `[0,0x4000000]`，变更时置 dirty，后项非法时立即返回，存在部分更新。`0x5d560` 另发 monitoring AudioControlRequestSet：unit 11、request 1、channel 0，control 1 的反向 bool（长度 1）和 control 2 的 `trunc(volume*35/100-15)<<8` i16（长度 2），timeout 均 100；对应 API 请求字为 `wValue=0x0100/0x0200`、`wIndex=0x0b00`。完整汇编及 API 构包共同核实参数，不能仅按寄存器位置猜测 control／channel 的名称。调用链丢弃若干返回码，且未自动 getter 读回，DSP 接受、设备应答与 UI 显示需分开验收。
- **monitoring 已追到 USB 提交与取消**：同包 x64 API 的 [`0x17b0 → 0x99f0 → 0x9c90`](../../../.analysis/results/native/adaf8f7d7455b6fa628752bd692fa6b65a1d341fde07962fa12ad9eac30a9f3f/functions/99f0.json) 核对 handle 后构造 24 字节请求描述符，经 `0x80882081` 提交。主驱动 `0x21ed0 → 0x2d6d0` 核对设备／AudioControl 状态，将真实接口号填入 wIndex 低字节，并生成 `URB_FUNCTION_CLASS_INTERFACE=0x1b`，经内部控制 `0x220003` 送下层 USB 驱动；常量另与已哈希的 Windows SDK `usb.h/usbioctl.h` 核对。`0x6360` 等待请求完成，100 ms 超时或其他非零等待结果会先 `IoCancelIrp`，再无限等待完成，返回后才释放 IRP。成功时真实 URB transfer length 进入 IOCTL 的完成字节数，并由 API 写入调用方 out count；`0x5c630/0x5c6c0` 却未检查这项计数。因而传输状态为 0 也不能自动解释为完整长度及设备设置已读回；本节没有执行这些请求。
- **停止／释放不等于恢复原绑定**：`0x6bba0` 发送 stop event 后调用 `0x6c320(INFINITE)`，后者以 1,000 ms 间隔循环而无总超时；SetEvent 失败只读出并丢弃错误，等待失败可保留 thread handle，调用方仍清 active／关闭事件。`0x6bc10` 随后 Release capture/render/client/clock、关闭事件、释放环形缓冲及清格式，`0x69590` 再释放时间统计数组和两个 endpoint。设备断开 `0x4e90 → 0x59530` 此处仅移除路由通知回调，不能与路由对象最终析构混为一谈。选中的替换／关闭函数没有旧名称快照与失败恢复；UI Cancel、重连通知、整个 owner 的析构时机及在途处理静止仍须由各自生命周期核对，不能自行增加通用回滚。

本节新增了路由 setter、实际 Core Audio PCM 写入、相关 DSP 包及 monitoring USB 提交／取消的静态链。此前 `GetRouterStatus` 的 JSON／bool ABI 差异依然成立：已核对的绑定和预设消费者没有读回它，不能据此推断动态／计算属性调用不存在。当前仍缺窗口动作的完整分派及取消消费者、真实 USB 应答／设置读回、完整重连／通知恢复和其他版本分支；实际 DLL／设备读回与 Rust 差分均为 Unverified。

### 1346 实际挂载、路由动作、持久化与恢复

本节只对应已保留的 1346 当前 UI／MW 与上述 `0542 v1.0.25.0` 二进制，完整 AST、虚表原始字节与函数记录收于同一 [音频语义收据](../../../.analysis/results/native-bulk/audio-semantic-verification.json) 的 `mounted_stream_mixer_chain`；不扩展到其他 PID、固件或平台。

- **实际页面挂载已连通**：[UI 主文件](../../../.analysis/inputs/frontend/audio-1346-current/raw/apps.razer.com/synapse/products/1346/ui/static/js/main.4d98fb93.js) 的 `createRoot → Wh → Bh/vh → Ch/ch` 仅在 setup 为 READY 时挂载 `dh/Nh`。`Nh` 的导航 id 2 持有 `gP/GP → hP`，`renderView` 实际选择该组件。`hP` 完整节点 `[5159673,5171099)` 通过 Redux connect 取得 mixer 状态和动作，实际渲染 playback 下拉、Aux 外接输入下拉、monitor 开关、增益、预设菜单与弹层。它不是未挂载的同名组件。`dh` 的对象字面量重复声明 `isDeviceReady`，后面的 `micReducer.isDeviceReady` 覆盖前面的 mixer ready；页面 processing 遮罩不能证明 mixer 写入已经完成。
- **产品工厂分支已核实**：当前 [MW 引导文件](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/main.497e651650c1643d6420.js) 明确选择 `useFeature("rzDeviceType", "AudioCamyT1")`；工厂的 `K.AudioCamyT1` 加载 chunk `4300`／module `32510` 并赋给 `jf.rzDevice`。通用的 `rzDevice25AudioCamy`／module `69354` 不是此 1346 挂载链选择的实现，不能拿同名能力替代当前调用方的证据。
- **下拉动作确实进入原生路由 setter**：Aux 下拉调用 `changeExternalInputDevice → aP → CHANGE_EXTERNAL_INPUT_DEVICE`，reducer 先合并 `exterInputDevice`，再发布 `ON_MODIFY_INPUT_CHANNEL`；playback 下拉调用 `changePlaybackMixDevice → QL → CHANGE_PLAYBACK_MIX_DEVICE`，reducer 检查索引后先更新名称，再发布 `ON_CHANGE_PLAY_BACK_MIX_DEVICE`。`vh` 将 `mwBroadcastChannel.postMessage` 转为 `USER_COMMAND`；[MW 主文件](../../../.analysis/inputs/frontend/audio-mw-1346-current/raw/apps.razer.com/synapse/products/1346/mw/6259.2eb539ec769147cd201d.js) 的 `he → fe → TypedRzStateMachineEvent → 状态机 streamMixer 动作 → rz_ui_action 分派 → Ho → di → bindExterDevice` 最终到前述原生 `SetRouteExternalDevice`。这些 UI 名称和选中状态是乐观值，不是 native ACK／读回。
- **缓存提交早于设备任务**：`Ho` `[1626461,1628115)` 先修改当前预设／设备缓存、递增版本并 await `setLocalStorageStreamMixerCache`，随后才排入 `NORMAL_SKIPPABLE` mixer 任务。该 setter 使用锁存储模块 `81988`：shared 锁读取旧字符串，再以 exclusive 锁逐字检查当前字符串仍等于旧值后执行 `window.localStorage.setItem`；冲突或非法字符串拒绝，不能描述成无条件覆盖。设备 runner `di` `[1806076,1809322)` 没有回滚缓存或调用 `GetRouterStatus`，还会在功能布尔结果未成功时继续写 task memory cache。`bindExterDevice` 丢弃 false 的问题仍在；add 分支的 `!results.find(value => value !== true)` 还会将首个失败值为 false／undefined 的数组判断为成功。必须分别保留原始结果、缓存、UI 草稿与设备实测状态。
- **Cancel 有明确边界**：mixer 启用冲突弹层的 Cancel 只关闭局部显示状态，尚未发送启用命令；确认才发送。删除弹层 `mc/Uc` 的外部点击传 `confirmDel(false)`，`hP.Ne(false)` 仅关闭；确认才派发删除。reset 弹层 `dC` 的外部点击只调用关闭，重置按钮才调用传入的 reset。普通路由下拉没有 Apply／Cancel 草稿提交。通用 task queue 的 pending 取消发布 `canceled` 并 dispose；运行中取消只对 AbortController 发信号，navigator lock 等待可因此中断，但 `di` 本身不检查 signal，也无设备撤销／恢复链。因而不能把队列 cancel 当成已取消正在执行的 DLL 写入，不能自行新增统一“取消即回滚”。
- **预设变更会再次写设备**：`Ko` `[1624798,1626461)` 在选择、添加、删除、重命名、复制及 reset 分支修改持久化对象后，调用 `initStreamMixerDevice`。其完整 service constructor `[2141377,2152535)` 读取缓存及设备 mode、处理活动窗口、必要时再设 mode；仅当读取出的 mixer enable 为真时执行 `initDevice` 和 shortcut 注册。当前原件的 reset 使用 `guid !== requested` 查找且只重赋局部变量，duplicate 也使用不等比较；不能将预设按钮结束或缓存写入当作目标预设设备设置已恢复。读取异常返回 false／空对象也是现存上游行为，不构成本项目以默认值冒充设备状态的依据。
- **路由重连已追到原生通知消费者**：真实 PE 虚表确认 `IMMNotificationClient` 回调经 `0x6a890/0x6a8b0/0x6a8d0/0x6a8f0/0x6a910` 转至 enumerator 的 default／added／removed／state／property 消费者。它们分别发内部事件 2／1／0／3／4；router 的 [`0x595e0`](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/functions/595e0.json) 只接受 0／1／3。`0x58f20` 以既有 endpoint ID 或名称重新匹配路由并置待处理标记；`0x64df0` 设相对时间 `-30000000`，即 3 秒后处理，连续通知会重设同一个 timer。到期经 `0x64e90 → 虚槽 +8 → 0x64e60 → lambda`，一支执行 [`0x59730`](../../../.analysis/results/native/b06973af45bcc6cc6c10dd74e2fc0f1b9caa8124b0416e8f869b5640fc795a8c/functions/59730.json)：停止／释放已运行路由、重新绑定、检查两端 `GetState == 1`，owner active 且端点有效时 prepare／start；仍沿用前述异步 worker 成功边界。另一支发设备列表变化通知。
- **恢复结果有事件链，但已核页面消费者不呈现 route status**：`0x59730` 发 12 字节的 `deviceId/status/deviceStatus`，其中 status 为 kind 是否等于 2；经实际函数对象虚槽 `0x76f20 +16 → 0x117e0 → 0x9880` 形成 `deviceCamyT1Changed / routeDeviceStatusChange` JSON，交给 `SetNodeFFIEvent` 保存的回调。另一路是 `deviceChange`。`AudioCamyT1.handleEvent` 核对 container ID 后把事件继续 emit；实际窗口激活链 `Nh.switchService → STREAM_MIX → startStreamMixerRecording → onEvent(J)` 注册的完整 `J` 只处理 meter、volume、mute、limiter，其余走日志 default，未更新 route 状态。已核对的这条挂载路径没有 `GetRouterStatus` 调用；此结论只限保存原件中的直接／字面量调用及已闭合消费者，不能推断其他版本或计算属性分派不存在。
- **最终 owner 析构与单次断连已区分**：`DeviceTerminate` 导出 `0x3c820 → 0x3a700` 从 manager 链表移除匹配对象并递减 strong reference，最后一个引用释放时，经原始 control-block 虚表 `0x79ed8 +0 → 0x3bed0` 执行设备析构。设备析构先 `0x4e90` 断连，再经 router 真正虚槽 `0x7ae18 +8 → 0x58080` 释放两个 delayed timer、enumerator、回调及路由树；树节点 `0x5a130 → 0x670d0 → 0x68780 → 0x69590` 最终停止桥接、释放 COM／event／ring 并销毁 critical section。timer 析构 `0x5a990` 会 disarm、等待 callbacks 并 CloseThreadpoolTimer，但其保存的回调对象先于等待被清理，不能擅自宣称所有并发竞态已被排除。单独 `0x4e90` 只拆通知和设备线程，不能等同于该最终析构。JS 的 `terminate` 会 await `DeviceTerminate`，但 `handleBeforeUnload` 和通用 shutdown 调用点没有 await 这个 Promise，窗口关闭不等于原生释放确认。

上述结论为完整保留 AST、原始 PE 字节和既有 IDA 捕获的静态闭环。本轮没有执行厂商 JS、应用、DLL 或设备；实际窗口像素／焦点、host 事件桥在运行时的送达顺序、取消在途设备操作、重连后的 PCM／设置读回、timer 并发安全及 Rust 差分仍为 Unverified。跨 PID／固件／平台的恢复分支不能由本节替代。

### 原生补查的 ABI 核对与限制

完整声明、调用方／被调方原始汇编身份及显式补查计划保留在 [音频语义收据](../../../.analysis/results/native-bulk/audio-semantic-verification.json) 的 `native_failure_review`。首批 7 个模块的 14 条字符查找反编译失败，通过恢复共用 scalar tail 的 EAX 字符参数、首个栈指针参数及 EAX 返回值，已在独立 IDA 补查中严格恢复；这只证明对应反编译结果恢复，不证明整个模块或实际设备功能已完成。保留的旧失败补查没有被删除或改记为成功。

- **USB ia32 的三条失败调用链**：保留原件 `node.napi.node` 的 `0x85e0/0x137d0/0x34da1` 都调用 `0x1170`。其完整正常路径只在栈上构造 `bad array new length` 对象，并以 ECX 交给终止链，不读取普通传入栈参数；原伪代码的两个参数并无相应输入。计划分别恢复 `0x1170` 为无参数 `cdecl + noreturn`、`0x1140` 为唯一 ECX 对象的 `thiscall` 构造函数，以及 `0x10c0` 为唯一 ECX 对象的 `thiscall + noreturn` 链。未确定的回调／虚目标没有被猜测声明。该组 13 份完整函数证据、754 条指令／2,093 字节已对原 PE 核实；`plans[7]` 已由统一队列实际执行 IDA 补查，三条原始失败经严格复核恢复。
- **KernelTraceControl x86 的十条失败调用链**：符号中的逻辑参数不能直接当作编译后全部放在栈上的参数。正常调用方的实际寄存器赋值、被调方读取和所有返回清栈均已核对，计划位于 `plans[8]`；29 份完整函数证据、1,588 条指令／4,302 字节已对原 PE 核实，统一队列已实际执行 IDA 补查，十条原始失败经严格复核恢复。`0xb784` 的两个未使用栈槽仍须保留，不能因优化后不读取而删除；WIL 相关函数也不能凭错误的 hidden-this 注释增加参数。

| 被调方 RVA | 已核实的传参／返回约定 |
|---|---|
| `0x504b` | `fastcall`，CL 为开关、EDX 为 trace handle 指针，另有六个栈槽；`ret 0x18`。 |
| `0x7abd` | `thiscall`，仅 ECX 对象；普通 `ret`。 |
| `0x8fb8` | `fastcall`，ECX／EDX 与两个栈槽；`ret 8`。 |
| `0x8f4a` | `fastcall`，仅 ECX／EDX，EAX 返回源指针；普通 `ret`。 |
| `0xb784` | `fastcall`，ECX／EDX 与三个栈槽；`ret 0x0c`，两个未用槽仍占位。 |
| `0x171e9` | `fastcall`，两个寄存器参数与四个栈槽；`ret 0x10`，没有 hidden this。 |
| `0x17a98/0x17ad3` | 三个普通参数的 `fastcall + noreturn`；第三参数已优化不用。`0x17ad3` 下游还以常量覆盖行号／HRESULT，不能声称其调用方实际初始化了 EDX。 |

- **THX 路由模块的六条分配异常调用链**：`plans[9]` 已核实 `0xd020` 为仅接受 ECX 对象并以 EAX 返回该对象的 `thiscall` 构造函数，`0xd040` 为不读取传入参数的 `cdecl + noreturn` 本地异常构造／抛出链；后者在栈上构造 12 字节对象，交给已核类型的 `_CxxThrowException@8`。该组 9 份完整函数、233 条指令／679 指令字节均已对原始 PE 核实。统一队列的独立 IDA 补查已将 `0x7e3d0/0x7f880/0xba400/0xba460/0xc50d0/0xf13e0` 六条原始失败严格恢复；实际补查收据与仍然失败的旧补查记录分别保存在 `native_failure_review.throw_helper_group`，没有删除旧失败或将其改为成功。

#### 15 模块 x87 共享异常处理批组

此前 USB 数学失败 `0x6413f/0x64ae5 → 0x64bb7` 属于跨模块共用的 x87 异常处理契约。现已用 4 个 Python 工作线程统一核对 15 个保留模块：86 份完整函数、5,035 条捕获指令／15,334 指令字节及 71 处直接传入 CALL，覆盖其中 39 条当前原始失败。原件、捕获清单、完整函数记录、全部 chunk 与逐条指令字节均回映至各自实际 PE；这些计数按完整捕获函数累计，共用尾部可能重复，不能理解为唯一物理地址或字节数。详细调用上下文、owner 的真实 chunk 与候选计划保存在 `native_failure_review.x87_shared_error_group` 和 `plans[10]` 至 `plans[24]`。

两个入口都以 EAX 接受错误类别、EDX 接受运算编号、ECX 接受名称指针，并消费当前 80 位 x87 ST0。单参数入口先建立 `0x20` 字节局部异常记录，以 `FSTP qword` 将 ST0 舍入存入 `record+0x18`，复制原始 double 参数后调用本模块实际的 `__87except`，再以 `FLD qword` 返回处理后的 double。双参数入口先保存 EAX 错误类别、复制第二个原始 double，然后 JMP 至单参数入口 `+9` 的 FSTP 处，共用相同 EBP frame 并保留 ST0／EDX／ECX；不能给共用主体或继承 frame 的 `__check_range_exit`／`__math_exit` 任意套普通 ABI。

下表的栈偏移从第一个栈参数开始，**不包含 helper 自身 CALL 压入的返回地址**：

| 显式位置 | 物理内容与已观测读取 |
|---|---|
| `^0.2` | 原调用方保存的低 16 位 x87 control word；helper 比较／恢复 word，并将其地址传给 worker。 |
| `^2` 至 `^3` | 四字节控制字槽的高两字节，保持未观测，不声明为读取值。 |
| `^4.4` | 原调用方返回地址形成的物理占位；helper 不读取，但首个 double 的位置必须跨过它。 |
| `^8.8` | 第一个原始 double，由 `EBP+0x10/+0x14` 复制至 `record+8`。 |
| `^16.8` | 仅双参数入口读取的第二个原始 double，由 `EBP+0x18/+0x1c` 复制至 `record+0x10`。 |

15 个实际 worker 均已逐一核对：从第三个栈参数取得控制字指针后，执行 `movzx ecx, word ptr [eax]`，再复制至 worker 自己的 dword 局部。后续局部 dword 运算与 `_ctrlfp` 的 `0xffff` mask，不能证明原调用方控制字槽的高 16 位曾被读取。返回结果来自可由 worker 修改的局部记录 `+0x18`，不能把进入 helper 时的 ST0 当成最终结果。

候选单参数声明如下；双参数声明仅在末尾增加 `double second_original_argument@<^16.8>`，完整两份声明均保留在每个计划内：

```c
double __usercall recovered@<st0>(
    int error_class@<eax>, int operation@<edx>, const char *name@<ecx>,
    _TBYTE current_result@<st0>, unsigned short saved_control_word@<^0.2>,
    unsigned int original_return_address_hole@<^4.4>,
    double original_argument@<^8.8>);
```

`_TBYTE` 明确保留 FSTP 之前的 10 字节浮点输入；返回类型 double 对应实际 FLD qword。显式栈位置、全部栈参数同时显式定位、首个栈参数偏移为零及 `_TBYTE` 类型的分析器语法依据，来自本机已保留的 [IDA 9.1 帮助获取收据](../../../.analysis/inputs/platform-abi-current/ida-9.1/acquisition.json) 及其 `1361.html/1492.html`。该帮助只证明分析器语法说明，厂商物理契约另由完整调用方／被调方 PE 字节证明；尚未实际验证这些候选能被 parser 接受、应用或恢复反编译，不计入已恢复条数。

| 计划 | 原件 SHA-256 前缀 | 单／双参数入口 RVA | 当前原始失败数 |
|---|---|---|---:|
| `10` | `1c34a749` | `0x27eb7 / 0x27ea0` | 3 |
| `11` | `6d05dd14` | `0xd6707 / 0xd66f0` | 2 |
| `12` | `6ea4d24a` | `0x26b57 / 0x26b40` | 2 |
| `13` | `7ae19c97` | `0x551297 / 0x551280` | 5 |
| `14` | `7d802669` | `0x27eb7 / 0x27ea0` | 3 |
| `15` | `7f0a7a7e` | `0x26b57 / 0x26b40` | 2 |
| `16` | `8bce5272` | `0x26b57 / 0x26b40` | 2 |
| `17` | `92a153d9` | `0x27eb7 / 0x27ea0` | 3 |
| `18` | `92aa36b6` | `0x26b57 / 0x26b40` | 2 |
| `19` | `96d4277c` | `0x12f87 / 0x12f70` | 2 |
| `20` | `ac71a2fa` | `0x1434c7 / 0x1434b0` | 2 |
| `21` | `c2d481fa` | `0xb607 / 0xb5f0` | 2 |
| `22` | `c5389725` | `0x422d7 / 0x422c0` | 2 |
| `23` | `d9e1dfbb` | `0x163d47 / 0x163d30` | 5 |
| `24` | `9cbed3ae` | `0x64bb7 / 0x64ba0` | 2 |

15 个单／双入口的完整指令序列仅将 `E8 rel32` 的四字节 displacement 归零后，各自具有同一个规范化哈希；原始 SHA、原字节和各自实际 CALL 目标仍分别保留。THX `0x13883d` 与另一支持模块的 `0x157a5d` 虽已有伪代码，也作为真实传入调用方完整核对；既有伪代码警告不因此变成语义正确的证据。本组未执行官方 DLL、Rust 差分或真实设备功能，相关维度保持 Unverified。

65 条同形算术汇编失败分属 13 个保留模块，其中 12 个产品模块共用五套调用方形状，另一支持模块为重定位后的对应形状。`0x23380/0x22a20/0x23de0/0x24740` 使用调用方 RSP 工作区、非标准寄存器输入及多个寄存器输出；后两类还跨入共用尾部。仅声明普通 `fastcall`、丢弃多寄存器输出，或重复原先只提高大小上限的失败补查，均不能算恢复。函数运行时入口须按 `.pdata` 的 BeginRVA／展开信息核准；入口前有 13 字节保存指令及 fallthrough，不能据此判定运行时入口或保留边界错误。该组仍保留为复杂契约未决，不与上述已恢复的 14 条混算。

以上均为静态分析与反编译补查边界；上述字符查找、USB、KernelTraceControl 和 THX 分配异常四组合计恢复 33 条原始失败，实际结果由[统一收据](../../../.analysis/results/reverse-pending.json)及对应 `reviewed-native` 记录保留。x87 批组的 39 条候选仍不计入恢复。没有执行官方 DLL 功能结果对照、真实设备写入／读回或 Rust 差分验收。

## Simple Service 的麦克风异步边界

依据 [simple_service 1.2.2.9 完整捕获](../../../.analysis/results/native/2c24c80c8d3ea19d84f223ec58ae2b9b02c882f578e06b2c8272c545c1dbfd5d/capture.json)，Image Base 为 `0x180000000`。单例构造 `0x1000` 安装的主虚表在原始 PE 的 `0x1f2040`；`+8/+16/+200/+208` 分别指向初始化 `0x12f0`、关闭 `0x19c0`、读取麦克风音量 `0x5550`、设置麦克风音量 `0x57f0`，不是根据方法名推测的虚调用。

导出 `simpleGetMicrophoneVolume`（`0x179f0`）将 NULL ID 转为空字符串，创建一次性完成回调，交给主线程队列。`0x5550 → 0x5710 → 0x32ac6` 继续转到音频服务对象；主线程或音频服务对象缺失时构造失败结果，错误分别是 `Error : Invalid thread.` 和 `Error: Invalid audio service thread`，失败携带的 mute／volume 0 不能标成设备确认值。getter 完成函数 `0x17700` 仅在用户回调存在且初始化标志仍开启时调用，参数包含 success、error_reason、device_id、muted、uint8 volume；字符串只在本次回调期间借用。

setter 导出 `0x17dc0` 经 `0x57f0 → 0x59e0 → 0x32dfa`，保留 mute bool 和 volume byte。完成函数 `0x17bf0` 只回传 success／error_reason；当前链中没有自动串联 getter，不能伪造写后读回。关闭导出 `0xf090` 先清除初始化标志，再排关闭任务；普通麦克风完成回调会被标志抑制，而关闭完成回调 `0xef90` 仍执行。因此必须区分请求已排队、完成已产生、用户回调被抑制和真实设备状态。

`0x12f0` 创建 `SimpleServiceMainThread`；`0x19c0` 排关闭工作后等待内部事件，再销毁线程持有者与对象。初始化回调不携带设备状态，不能据此判定设备已连接。本文记录静态调用语义，Rust 迁移、实际 DLL 行为与设备结果尚未验收。

### 音频线程、Core Audio 与缓存来源

该麦克风分支继续追到了 DLL 内的 `AudioServiceWin`，不能把名称中的 Service 直接解释为进程间协议。`0x31db0 → 0x38f60` 创建对象并安装原始虚表 `0x1f54d0`，`+72/+80` 分别指向 `0x3a850 → 0x3a856` 和 `0x3a8f0 → 0x3a8f6`。读取从精确 ID 对应的设备对象取缓存 byte `+101` 的 mute 与 `+100` 的 volume；未知 ID 返回 `Error : Cannot find audio device.`。因此该 getter 返回的是监听器维护的设备缓存，不是每次重新调用 Windows getter。

初始化 `0x37144` 获取实际 endpoint，`0x3fb86` 激活音量接口；`0x37a56` 从接口 `+72` 读取 scalar，按 `int(scalar*100+0.5)` 保存 volume；接口 `+120` 成功时更新 mute。失败没有证明字段已观测，不能把对象构造初值当成实测。设备监听注册由 `0x38690` 执行，音量通知消费者 `0x37ad0` 在值变化时更新缓存，并仅在事件启用且 flow 分支匹配时通知对应消费者。

实际写入 `0x37b34` 先比较 volume，变更时经接口 `+56` 写 scalar，成功后才更新 byte `+100`；再比较 mute，变更时经 `+112` 写 mute，成功才更新 byte `+101`。浮点实参由完整汇编的 `cvtsi2ss/divss XMM1` 传入，Hex-Rays 的整数函数签名遗漏了该实参，不能直接拿伪代码生成 ABI。前一步成功、后一步失败会保留已完成的音量变化，未见整组回滚或自动读回；两字段不变时本层可直接返回成功而不发写入。

关闭沿 `0x38fde` 清设备表、释放监听器并失效弱引用；设备析构 `0x36fea` 再清两种监听持有者、线程引用、字符串和音量接口。间接的 CFG 跳转仍须与实际虚表消费者共同核对，不能把释放语句存在等同于已确认所有在途通知静止。此链的 OS 音量行为与厂商设备固件写回分别验收，不应将系统 endpoint 成功当作另一条 HID／DSP 链通过。
