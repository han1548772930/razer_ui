# 播放、均衡器与环绕

当前音频产品没有一条通用的“写配置即生效”链路。音量可能属于 OS 端点；EQ 可能属于 THX DSP、固件预设、板载多槽或麦克风参数 EQ；页面还可能把持久化字段作为编辑来源。适用型号与实际注册分支见 [家族矩阵](README.md#产品与适用边界)，具体页面与 props 见 [逐产品原件](products/)。

## 播放与真实音频端点

旧音频 restore 的 `speakerVolume/volume` 分支读取 `getDefaultSpeakerDevice()` 的 `volume/muted` 后发布 `MW_CHANGE_VOLUME_TO_UI`，不把保存的音量直接当作当前系统音量。`micVolume` 也有实读当前 capture 端点的路径；安装 `audioBasicFeature` 或 THX generic 后，旧分支主动跳过，由相应 feature 处理，不能两套同时执行。

例如 1342 当前 MW `5736.fdd55044a41484df28aa.js` 的完整 `il`，bytes `[1460380,1476924)`，依次保存/发布 document，按精确 active GUID 找 profile，再遍历实际拥有的字段。THX v3/Carol 的 `audioEqualizer` 发布选择模式与预设，组装 `{mode,eqData:{name,current},version}` 进入事件队列；THX v2 使用另一结构与 `ON_SET_EQUALIZER`；安装 `audioEqualizer_fw` 或 THX generic 时跳过旧 EQ restore，最后进入 GenericFeatureManager 的 restore。

`thxSpatialAudio` 先发布本地选择，再核对设备是否为当前默认输出；不是默认输出时跳过 `ON_SET_SPATIAL_ENABLE`。这项跳过是源码的路由语义，不能记录为 DSP setter 成功。音频设备名、endpoint ID、container ID 和产品 PID 分属不同身份，按真实枚举结果传递。

## 固件预设和板载索引

| 产品分支 | 源码索引/作用域 | 必须保留的差异 |
| --- | --- | --- |
| 1365 | default 0；game/music/movie 7/8/9；esports 250–254；custom 255 | `useEqualizerOBM` 仅 250–255，不包括标准三项 |
| 1383 | 0/7/8/9/255 | OBM 仅 255 |
| 1387 | 0/7/8/9/255 | OBM 为 7/8/9/255 |
| 1396 | 0/7/8/9/255；固件支持，profile global | 不与旧 THX v3 的文件 EQ 混用 |
| 1404、2644、4130 | 0/1/2/3；固件支持，profile global | `skipUpdateaudioEqualizerToLocalStorage`；2644/4130 另有语音提示 feature |
| 1453 | 0/1/2/3/16；非 global | 当前显式 `audioEqualizer_fw` 的 map 包含 custom 16，设备类型允许 `rzDeviceJolie` |
| 1462 | 1/2/3/31；非 global | `avoidReadingEQFromFW:true`；`onlyCustomHasEQBands:true` |
| 1427 | THX v4 与固件同步；0/1/2/3 | band 的负值使用 `128+abs(value)`，读回以 `128-value` 解码；不是二补码 |
| 1465 | `audioEqualizer_fw` | `useEQBandSettingWithPreset:true`、`onlyCustomEQHasBands:true`；另有 profile 2 的 save-effect |
| 2657、2660、2664、2668、2689 | 协议 4.0 耳塞，固件支持，非 global | 操作按耳塞的实际 transport 与 feature，不能调用 2.5 的同名方法 |

显式 `AudioEqualizer_FW` 的完整模块 57283 可以在各 PID 的 `7283.*.js` 核对。1453 的 `7283.d1f11cf349c6803a4c9d.js` 全文件 4998 bytes；2657 的 `7283.d739a1e04541416d4900.js` 全文件 4927 bytes。`setupRenderEQData/getRuntimeData` 使用本地 profile 数据，并不执行固件 EQ 读取。setter 把 `eqData.current` 转为有序 `{index,level}`，再按预设 map 调用协议 helper。

这个外层 `setAudioEqualizer` 等待内部调用，却不消费其布尔返回；内部也可能在类型不匹配或异常时返回 `false`，外层仍返回 `true`。因此需要分别理解“源码外层结果”“实际调用是否发生”“原始协议应答”。restore 随后的 UI 更新从 runtime cache 取得，也不是独立读回。是否实际使用这份 lazy module，由各产品 feature 装配决定，不能把包中存在的类视为全部型号的活动链。

## 本地多槽 EQ 与实际设备数据

1398、1401、2638、2641、4124、4126 的 `cloudBasedEqualizer` 名称沿用源码；其设备 EQ 与本地预设读写属于本机能力，在线目录/账户部分不在产品范围。以下完整所有者来自 1398 MW `main.11fdb3ee2d36c7e8efa4.js` bytes `[950699,959614)`。

读槽先 `getSpeakerEQOrderInfo(index)`，正常槽再 `getSpeakerEQBandByPosition(index)`。顺序信息包含 index、语音提示索引、enabled、active、预设 ID 和 custom；band 另行读取，不能靠 index 推断开关。`operationMutex` 控制初始化，`deviceAccessMutex` 串行设备访问。断开或 reset 清除 `initDone/dataRestoreDone`；PostRestore 在配置要求时重新读取；硬件 EQ 变化通知仅在初始化后进入本地列表、活动标志和 UI。

原版对 `0xff` 无效槽保留旧 band；读取异常时还返回默认 EQ，`checkActiveEq` 会从旧活动槽、enabled 槽或首槽补选活动项。这里是原版数据修补的实际边界，不能将其转为本项目“读取失败后的设备实测状态”。真实观测、文件默认值与草稿必须分开。

Stereo 写入为 batch start → 每槽 band → 每槽 order metadata → batch stop；每项检查 `status=="SUCCESS"`。Spatial 路径只写选中槽 order metadata。改变活动项也使用 batch start → 每槽 order → batch stop。原版 catch 后释放 mutex，但失败分支没有 finally 补发 batch stop；不能声称存在自动回滚或设备恢复。footsteps 的 band 配置同时有固定表和本地预设存储读取，不能依赖在线获取才能执行本地 DSP 写入。

## 参数均衡器与效果顺序

1422/1446 的参数 EQ 属于麦克风设备，不是上表十段播放 EQ。当前完整 feature 为模块 65561，1422 `5561.bee7bca633d9da9fd151.js` bytes `[549,9915)`；profile 映射为模块 23374，`3828.c5e55b0ac9b8ddaac249.js` bytes `[549,1098)`。

| 对象 | UI/领域含义 | 固件编码或顺序 |
| --- | --- | --- |
| preset（MW/API 映射） | podcast、arena、fpscallout、studio、instrument、custom | preset ID 6/7/8/9/10/16；对应 profile ID 1/2/3/4/5/6 |
| frequency | 20–20000 Hz | u16 大端；源码位运算会截断小数 |
| gain | -12.0 至 +12.0 dB，0.1 步长 | 乘 10 后 signed u8 二补码，与 1427 的编码不同 |
| Q | 0.3–15.0，0.1 步长 | 乘 10 存入字节 |
| band | 稳定 band ID、status、filter type | profile、direction、band、status、freq、gain、Q、filter 的完整字段 |
| HPF | enabled 与档位 0–5 | profile 1；禁用写 0 |
| vocal bass/exciter | profile 的独立效果 | 保留 band 8/9，300 Hz low shelf / 6000 Hz high shelf；amount-50 的有符号十分位；Q byte 7 |

单 band 修改先真实 `getEQPreset(0)`，核对成功且有 `jsonData`，将设备 preset ID 映射到实际 profile，再 `setParametricEQSetting(profile,direction=0,...)`。完整切预设是 profile 1 清十个 status → 选择 preset → 顺序写全部 bands → 本地 profile 的 vocal bass → vocal exciter → HPF。初始 `initDataFromDevice` 只补/发布本地 EQ 结构，不读硬件；名称不能替代函数体。

源码检查单 band 的 status，但完整预设流程没有传递清 status、选 preset、band 循环及 HPF 的全部返回；vocal bass/exciter 返回失败会阻断后续阶段。HPF setter 的 `true` 也只表示 await 未抛出，getter 在失败时保留内部初值。`getEQActiveBands` 使用 status 值作为 getter 的 band 实参，而不是循环下标；不能把此方法改写成自行推测的正确读取顺序或用它证明当前页面已读到完整 EQ。

## Seiren 实际 EQ 页面与当前 Rust 接线

本节只覆盖 1422/1446 的实际 mounted EQ 页面。两款均由 UI 模块 6007 挂载，但必须分别读取自己的主入口、产品配置、middleware 与资源，不能把同名组件当作型号兼容性证据。

| PID | 实际 UI 原件 | 完整 EQ factory（UTF-8 bytes） | 产品配置中的页面预设 |
| --- | --- | --- | --- |
| 1422 | [684.f6b88b50.chunk.js](../../../.analysis/inputs/frontend/audio-1422-current/raw/apps.razer.com/synapse/products/1422/ui/static/js/684.f6b88b50.chunk.js) | `[55022,94170)` | [main.61ab07b9.js](../../../.analysis/inputs/frontend/audio-1422-current/raw/apps.razer.com/synapse/products/1422/ui/static/js/main.61ab07b9.js) 模块 8193，五项数组 `[209344,209396)` |
| 1446 | [684.9501aa81.chunk.js](../../../.analysis/inputs/frontend/audio-1446-current/raw/apps.razer.com/synapse/products/1446/ui/static/js/684.9501aa81.chunk.js) | `[67423,106618)` | [main.9f98c3ff.js](../../../.analysis/inputs/frontend/audio-1446-current/raw/apps.razer.com/synapse/products/1446/ui/static/js/main.9f98c3ff.js) 模块 8193，五项数组 `[211862,211914)` |

当前页面的五个预设是 `podcast/arena/fpscallout/studio/custom`；`instrument` 只出现在上述 MW/API 映射，不能凭协议映射给页面增加第六个按钮。[Rust EQ model](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq_model.rs) 的 `PRESETS` 与这两个实际数组一致。各预设的选中 band、band 列表和 HPF 分别保存，不能把“切预设”简化为改变标题或共用第一组 band。

已逐段核对的页面行为及当前所有者如下。这些是静态语义与接线结论，不代表设备效果已经实测一致。

| 行为 | 当前官方 mounted owner | 当前 Rust 所有者与核对结果 |
| --- | --- | --- |
| 麦克风、耳机、sidetone 三组控制 | 1422 monitoring `[58171,62903)`；1446 `[70597,75329)` | [EQ 页面](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq.rs) 为三组值各保留 300 ms 提交任务；静音立即提交，不取消先前排队的音量。AGC 禁用 mic fader/数值编辑；Stream Mixer 禁用 sidetone，并有随鼠标定位的提示；[graph 控件](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq_graph.rs) 持有相应禁用和 meter 呈现。 |
| 曲线、band 与参数 | 1422 graph `[68916,78106)`、band `[78107,79170)`、properties `[80759,83577)`；1446 graph `[81342,90532)` | graph/math/model 分别负责 20–20000 Hz 对数坐标、滤波器响应与稳定 band ID。当前 model 支持五种滤波器、最多八个 band、至少保留一个；pass 类型的 gain 为 0 且禁用编辑，shelf 的 Q 显示 1 且禁用编辑。删除所选 band 时按原数组相邻项选中，不能用排序后的显示下标充当协议 ID。 |
| de-esser、vocal bass、exciter 与 HPF | 1422 effects `[91280,93856)`；1446 `[103709,106282)` | 三个效果共享 20 ms throttle 与 200 ms 最后一次提交任务；HPF 独立立即提交。曲线计算包含实际启用的 bass/exciter/HPF；编辑曲线和数值时的草稿不能冒充设备读回。 |
| 预设、reset、flat、filter tutorial | 两款模块 6007 的完整 wrapper、graph 与 properties | [EQ 页面](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq.rs) 有独立预设、reset、flat 与五类滤波器帮助入口；帮助的选中类型、展开、关闭和焦点属于页面所有者。reset 比较源码默认的有效 band 与 HPF；flat 是另一动作，不能互相代替。 |

页面的第一行是 microphone monitoring 与 visualizer，第二行是 band、properties、effects。CSS 必须按实际 lazy import 和 webpack link 插入顺序计算：1422 为 `main.2715ebd0.css → 38.0df290bd.chunk.css → 684.51447135.chunk.css`，1446 为 `main.2715ebd0.css → 287.cae4ef23.chunk.css → 684.8af27a50.chunk.css`。visualizer 宽 1000 px、graph 840×242 px；三个下排面板宽 200/400/580 px，间距 20 px；预设按钮至少 150×36 px。Rust graph 常量和页面两行布局已能对应这些尺寸，仍需逐状态视觉验收，不能以常量相同宣称整体 1:1。filter tutorial 的 800×420 px 最小尺寸、五组图文、300 px tooltip 和下拉展开/失焦关闭也属于实际页面范围。

真实动作已能沿当前文件追到独立实现：

| 阶段 | 当前生产所有者 | 必须分别保留的结果 |
| --- | --- | --- |
| 页面排队 | [audio_seiren_eq_write.rs](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq_write.rs) | `SeirenEqRequested` 持有 entity/epoch/sequence/guard、设置快照与 `Change`。同类未执行项可合并；预设和 band 的有效先后顺序保留；取消后等待工作结束才释放在途槽。 |
| 事件与持久化 | [source_workspace.rs](../../../crates/razer-pages/src/features/workspace/source_workspace.rs)、[Shell EQ](../../../crates/razer-shell/src/shell/audio/seiren_eq.rs) | 订阅真实 `SeirenEqRequested`，按 NoSave 与需要保存的字段分流；需要保存时成功后才提交设备，并在保存前核对、合并当前 profile/device/global 字段。单独的 `AudioProductChanged` 只捕获设置快照，不能代替设备请求。 |
| 保留会话与传输 | [seiren_audio_operations.rs](../../../crates/razer-shell/src/shell/audio/seiren_audio_operations.rs)、[audio_notifications_worker.rs](../../../crates/razer-shell/src/shell/audio/audio_notifications_worker.rs)、[服务 EQ](../../../crates/razer-service/src/runtime/windows/audio/seiren_eq.rs) | 使用当前唯一物理路由、保留会话与顺序队列；切连接后使旧 guard 失效。采样率动作先 ready=false、停止 routing，执行真实请求，再按源码恢复 routing 并发布实际结果。服务核对 target/container/instance，保留超时与关闭清理。 |
| 独立设备与系统音频逻辑 | [设备 EQ](../../../crates/razer-device/src/audio/seiren_eq.rs)、[会话 owner](../../../crates/razer-service/src/runtime/windows/audio/seiren_session_owner.rs)、[驱动](../../../crates/razer-service/src/runtime/windows/audio/seiren_driver.rs)、[端点](../../../crates/razer-service/src/runtime/windows/audio/seiren_endpoint.rs) | HID 协议由 Rust 编码/执行；另一分支由 Rust 使用精确设备接口、`DeviceIoControl`、系统音频端点及回调完成。`NativeOperation` 是本项目的操作类型，并不表示调用官方业务 DLL；当前会话不加载厂商库，也不伪报厂商 DLL 版本。 |

`source_result` 是按官方外层消费者保留的结果；实际传输阶段、应答、skip、错误和 `confirmed()` 必须另行检查。设备 disconnected/failed/unobserved 与持久化默认值、当前草稿分别处理，不能用初始化的 profile 或页面已显示曲线推断 EQ 实测成功。当前初始化遮罩来自实际 ready 条件，也不是 DLL 初始化或 DSP 效果确认。

### 验证与索引边界

本轮只进行了静态核验：引用的官方原件、CSS、向导资源和获取收据共 80 份身份匹配，保留的 92 个完整 module body、128 个完整 function owner 与原件字节及 body SHA-256 一致，36 份完整 JS 静态解析无错误。其中也包括 [麦克风文档](microphone-and-preview.md#1446-mic-页面与映射) 使用的两个 MW feature；这些数量不表示整个输入闭包、整个音频族或全部设备分支完成。两款 UI 获取收据仍分别有 35/47 项未决闭包，完整设备与主机源码的版本审计仍需继续。

`audio-MapAudio-audit.json` 的原件身份和完整 owner 可定位上述证据；其中 `rust_implementation/connected_local_chains` 仍含已不存在的 `seiren_eq_native.rs` 和旧 ABI adapter 描述，`rust_ui_implementation` 也只是先前文件快照。这些旧 Rust 身份不能作当前接线或完成依据，必须以本节实际文件重新生成后再核验。本文未把旧报告中的“已接线”标签或 query 无失败计数当作验收结论。

每个 PID 都仍需将官方实际 DLL 的对应功能结果与本地 Rust 对照，覆盖五个页面预设及协议有效分支、band/filter/status、bass/exciter/HPF、端点音量/静音、sampling/mode、错误、取消、恢复和释放；DLL 外层返回与真实应答应分栏保留。可定位的纯测试与模拟传输不等于已取得这些实测结果，本轮没有执行官方 DLL 或设备对照。MIC 底栏挂载、向导层级和独立映射链的已知缺口见 [麦克风文档](microphone-and-preview.md#当前-rust-已接线部分与缺口)。

## 前后音箱环绕与时序校准

1392、1442、3942 当前挂载 surround；1392 的完整 UI/MW/native-wrapper 所有者保留于 `audio-surround-audit.json`，另外两款分别保留自身输入与绑定，不能只按 Carol 同名适配器合并。

页面先枚举真实可用 front endpoints。开启 Surround 时，若没有已选 front endpoint，挂载选择弹层；Cancel 关闭并撤回本次启用选择，Confirm 采用实际条目的 ID/name。设备列表空时禁用切换并显示源码警告。前后音量、各自三段 EQ、replicate、delay 与试听是独立动作。

Time Alignment 控件为 -100 至 +100、步长 1，标签是毫秒；native wrapper 的 `SetComboDelay` 实际把值除以 100 后传出，不可根据标签自行改成除以 1000。音量同样除以 100；EQ 各自传 JSON 数组。试听状态变更、关闭 Surround 和组件 unmount 都发送停止测试信号。

任务先检查取消。模式任务先更新 runtime 草稿，再检查 front identity，再调用 `SetComboMode`；该调用成功后按模式写 rear/front EQ，关闭时写零值。front endpoint 选择成功后更新 runtime，再异步触发已启用的模式任务；rear volume 大于 0 时先解除 rear mute。其他音量/EQ/delay/replicate 任务以实际 `result/reason` 决定完成，再发布 runtime。不能用 optimistic runtime 修改证明原生操作已接受。

设备事件 `onAudioDeviceStateChanged` 仅匹配保存的 front endpoint ID：state 4 断开时读取并关闭 combo，清 runtime front identity 并发布禁用/移除；state 1 重连时读取并启用 combo，恢复其 ID/name，发布 UI，再排队 front-speaker 事件。读取失败与已确认关闭不同，不能额外补一个默认端点。

## 演示与来源

1352 的本地演示不循环、不带自定义 controls、不浮动；1392/1442/3942 循环、有 controls 且可浮动。四款都使用系统默认 render endpoint、不静音、不归 profile 所有。关闭播放、全屏和浮动状态属于媒体生命周期，不能顺便改设备 EQ。完整原件、CSS 与媒体收据见 `audio-demo-audit.json`。

本页深层证据分别见 `audio-mw-audit.json` 的 `audio-local-profiles`、`audio-MapAudio-audit.json` 的 `seiren_eq/leviathan`、`audio-help-reset-audit.json` 的完整 EQ/reset 所有者，以及 `audio-surround-audit.json`。本文描述当前原件的静态语义；官方包装器 ABI、设备应答和本项目独立实现状态分别核验。
