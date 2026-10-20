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

1422 当前 UI `684.f6b88b50.chunk.js` 中 footer 所有者 `[349190,353043)`，实时预览 `[339375,349157)`。footer 在 EQ、Effects 和 Stream Mix 挂载；只有浏览器音频准备、设备 ready、当前窗口 active 且 visibility 为 visible 时允许捕获。采样率来自产品 `supportedMicSamplingRates`（缺项时原版使用 48/96 kHz）；切换属于独立 `ON_SET_SAMPLING_RATE_VALUE`，不归混音 preset。

实时预览不是设备 setter：它取得真实麦克风 stream，关闭 echo cancellation/noise suppression/AGC，建立 AudioContext、gain 与 analysers，再绘制波形。启动使用 generation 计数；旧 `getUserMedia` 完成后若代次不同立即 stop tracks。关闭、不可见、切设备与 unmount 增加代次，清 retry/interval/animation frame，disconnect nodes、stop tracks、close context、清 ring buffer 与历史电平。

控件同时使用硬件 meter 与 PCM：有效外部百分比映射到 dB，并以本机 analyser 校准；外部数据不足时使用 analyser。图形的 6 秒历史、-3/-12/-18/-60 dB 辅助线、平滑与峰值标记属于显示，不证明 EQ/DSP 已生效。捕获失败的重试最多执行源码计数规定的分支，不能显示设备已连接或生成模拟波形。

## 录音与回放路由

1422 UI 模块 9374，`38.d64aff18.chunk.js` bytes `[51422,58031)`。录音优先选真实枚举中带 `Service (Do not use)` 的 input，找不到时使用系统捕获选择；按当前 sampling rate、单声道关闭 echo/noise processing。MediaRecorder 按实际支持顺序选择 pcm/L16/wav/webm/mp4，停止后以实际 MIME 和字节形成录音，stop tracks。空录音不启用播放。

播放独立查找对应 service output，建立 blob URL，发送 `{switchstate:1}`，等待 UI 消费 `MW_UPDATE_MIC_RETURN_SWITCH_TO_UI`，在可用时 `setSinkId`，再启动音频。源码还有 BroadcastChannel 和手动 reducer 通知分支，超时后继续的行为不能算硬件 return-switch 确认。真正的 return-switch 设置仍需协议应答；不能把 Redux 值当 ACK。

停止、ended、播放异常与 unmount 都发送 `{switchstate:0}`。停止还 pause、回到时间 0、撤销 blob URL、移除 ended listener；unmount 释放捕获 tracks 与播放资源。源码部分异步等待和 monitor interval 的生命周期并不都由完整取消任务包住，应逐项记录其实际释放，不宣称存在源码未提供的取消屏障。

## 麦克风设置向导

同一 `684.f6b88b50.chunk.js` 完整向导 `[366649,383572)` 与 PCM capture `[353065,363586)`。进入时只保存一次快照：mic volume、noise gate、AI suppression、compressor、limiter、reverb、选中 EQ、vocal bass/exciter；100 ms 后应用源码测量默认值。Cancel 和关闭按钮用该快照逐项经原动作恢复，再关闭。Finish 将测量得到的 mic gain/noise gate 和用户选择的用途/距离参数提交，然后关闭；这些动作会产生真实设备/DSP 工作，不能只改向导 UI。

语音/环境分别最多捕获 10 秒，记录其当前测试类型后分发分析。选择设备会排除 default、stream/playback mix、service、Mic (Dry)；优先当前设备名且带 microphone 的真实输入，再用源码规定的实际枚举候选。语音分析在无有效音频或 metric 小于 -100 时不提交增益；环境录音过短仅输出源码警告。gain/noise 算法分别位于模块 9909/596，不能用固定推荐值替代采样。

PCM capture 拥有 loading/recording/error、tracks、AudioContext、worklet、splitter、analysers、计时器及 animation frame。开始失败逐项 disconnect/stop/close；worklet blob 在 finally 撤销。停止按真实 context sample rate 合并第一通道 PCM 后调用完成消费者，再清状态；unmount 取消计时并释放节点/stream/context。不得执行厂商 JS 来“验证”该流程，本轮仅静态读其完整函数。

## 固件效果与参数区别

1422/1446 还有 noise gate、AI suppression、compressor、limiter、reverb、de-esser、AGC 与旋钮。数值范围、signed 编码、profile ID、写入顺序分别属于具体协议；参数 EQ、HPF 与 vocal 效果见 [EQ](playback-and-eq.md#参数均衡器与效果顺序)。AGC 当前 `initFromDevice:false`，布尔 coercion 包含 true/1/字符串 true/1，真实 setter 为 `(profile=1,module=1,state)`。旋钮 restore 先读取，值相同跳过，不同或失败后进入源码 setter；直接设置不带额外猜测的读回。generic 参数提取只取 `dialControl`，不把相邻 `mappingList` 当设备协议。

音频输入、语音/环境样本和录音内容均为本机能力；不需要账户、云目录或网络。来源、原生声明和完整调用者见 `audio-MapAudio-audit.json` 的 `seiren_eq.products[].complete_function_owners/complete_modules`。页面读写、独立 Rust 实现与真实运行验收分别记录；本文只说明当前源码语义。
