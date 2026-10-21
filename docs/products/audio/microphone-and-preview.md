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
