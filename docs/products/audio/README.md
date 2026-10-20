# 音频：当前源码中的产品与本机能力

当前保留的 UI 与 MW 包覆盖 77 个 PID。本目录先用每款 MW bootstrap 的实际 `useFeature` 参数区分设备、服务和固件分支，再沿页面与结果消费者说明功能。相同耳机名称、相同页面标题或共享模块存在，均不能替代当前产品实际注册的 feature。逐产品的页面顺序、组件参数和条件见 [products](products/)。

## 产品与适用边界

表中的 THX 版本、原生包装器和 `hasDll` 都是原版的调用来源说明，不表示本项目允许把官方业务 DLL 作为运行后端。原生链路须继续追到协议、服务、DSP 或 OS 音频端点，并独立实现。

| PID | 当前 MW 设备适配器 | 本机功能与产品差异 |
| --- | --- | --- |
| 777、1328、1332 | `AudioAWKittyBLE` | 独立 BLE 音频适配器；bootstrap 未注册后列三款的 THX v2 与秒制省电参数 |
| 1378、1406、1407 | `AudioAWKittyBLE` | THX v2；省电值按秒处理；不能继承前列产品的配置解释 |
| 1303、1304 | `rzDevice25` | 分别声明 `nommoChroma`、`nommoPro`；`simpleService` 音频事件；`PROTOCOL25` 硬件事件 |
| 1306、1308、1310 | `AudioAvneraDevice` | THX v3、独立 Chroma 接口、秒制省电；1306 额外注册电池能力 |
| 1312 | `AudioKrakenTE` | THX v3；使用系统静音；页面包装与端点状态须按本 PID 读取 |
| 1313 | `AudioKrakenKitty` | THX v3、独立 Chroma；亮度 region 为 4 |
| 1318 | `AudioKrakenXUSB` | THX v2 |
| 1319 | `AudioKrakenUltimate` | THX v3、独立 Chroma、秒制省电 |
| 1320 | `AudioAvneraDeviceOdessaT1` | THX v3、秒制省电、电池 |
| 1321 | `AudioKrakenOdessaT2` | THX v3；独立当前页面包装 |
| 1325 | `AudioNXPDevice` | THX v3、独立 Chroma、秒制省电；电池配置明确 `useNotification:false` |
| 1330 | `rzDevice25Noa` | THX v3、THX 设备接口、独立 Chroma、Control Pod、硬件 bass boost |
| 1331、1353 | `AudioKrystalT2` | THX v3、独立 Chroma |
| 1335 | `AudioKrystalT3` | THX v1；独立 Chroma；亮度 region 为 15 |
| 1389 | `AudioKrystalT3` | `simpleService`；硬件事件为 `AUDIO_APRIL`，不能仅因适配器同名套用 1335 |
| 1337、1339 | `AudioMxicDevice` → `AudioMxicMabelT1` | `AUDIO_MXIC_MABELT1`、remote mode、连接延迟 1000 ms、THX v3；1337 注册通知式电池 |
| 1411、1412 | `AudioMxicDevice` → `AudioMxicMabelT1_ecr` | 独立 `AUDIO_MXIC_MABELT1_ECR` 解析器；同样 remote mode/1000 ms；1411 注册电池通知 |
| 1342 | `AudioMixer` | 独立混音与效果预设；多灯光设备；独立 Chroma |
| 1346 | `AudioCamyT1` | 虚拟音频通道、Stream Mixer；`setMappingEngineLocalStorage:false`；原版原生包装器有电平监听入口 |
| 1347 | `AudioCamyT2` | 同类虚拟通道与 Stream Mixer；独立产品原生包装器 |
| 1386 | `rzDevice25AudioCamyT3V2` | `PROTOCOL25`、虚拟通道、独立原生包装器；Stream Mixer 的不可用输入包含 `Headphone` |
| 1391 | `rzDevice25AudioCamy` | `PROTOCOL25`、虚拟通道；Stream Mixer 的不可用输入列表为空 |
| 1422 | `rzDevice25AudioMarieT1` | 参数 EQ、Marie 固件效果、旋钮、指示灯、AGC、Stream Mixer；mapping engine 不保存产品本地配置 |
| 1446 | `rzDevice25AudioCamyT2V3R` | 同类 EQ/效果/混音；另注册 `muteButtonMappings`、`multiFunction`，不能删去这两个分支 |
| 1352 | `rzDevice25Noa` | 声明 `noaT1`；`simpleService`、`PROTOCOL25`；bass/output/mode 使用各自真实命令 |
| 1354 | `rzDevice25NoaT3` | `simpleService` 的硬件事件静音处理开启；`PROTOCOL25`；EQ 刻度与范围单独配置 |
| 1364 | `AudioKittyV2Pro` | THX v3 |
| 1365 | `AudioMxicDevice` | remote mode、THX v3、电池通知；硬件 EQ 7/8/9/255 与电竞预设 250–254；OBM 仅 250–255 |
| 1370、1372、1374 | `rzDevice25NommoV2Pro` | THX v3、独立 Chroma、Control Pod、硬件 bass boost |
| 1443 | `rzDevice25NoaT2ECR` | THX v3、THX 设备接口、独立 Chroma、Control Pod、硬件 bass boost |
| 1376 | `AudioKrakenKittyT3` | THX v2 |
| 1382 | `rzDevice30Layla` | 启动设备模式 `[5,0]`，退出 `[5,1]`，`ignoreCheckMode:true`；模式生命周期独立 |
| 1383 | `AudioBillieT1` | THX v3、电池通知、audio-to-haptics、OLED、固件亮度状态；硬件 EQ 0/7/8/9/255，OBM 仅 255 |
| 1387 | `AudioBillieT2` | THX v3、电池通知；同一组硬件 EQ 索引，但 OBM 包含 7/8/9/255 |
| 1390 | `AudioOdessaT2Wireless` | remote mode、THX v3、电池通知；硬件 EQ 分支独立 |
| 1392 | `rzDevice25Carol` | `thxCarol`、实际设备列表、虚拟路由 `Razer Clio Surround`；电池通知 |
| 1442 | `rzDevice25Carol` | `Razer Clio X Virtual Surround`、独立 rear endpoint/routing service；查询电池但隐藏图标、外部供电；另有 `devicePowerMode` |
| 1396 | `AudioMabelT3C` | 系统音量/静音、真实侧音；THX v1；固件 EQ、profile global，0/7/8/9/255 |
| 1398、2638、4124 | `AudioValkyrie` | `AUDIO_VALKYRIE`、THX generic、电池/dongle；本地 EQ、双路输出、ANC、省电、滚轮、麦克风、指示灯、HyperSpeed、game/chat balance、设备重置与 THX v4 render/capture |
| 1401、2641、4126 | `AudioValkyrie` | 同类独立产品注册；当前 generic feature 列表没有上述三款的 `activeNoiseCancellation` |
| 1404 | `rzDevice25ValkyrieT3` | THX v1；固件 EQ、profile global；硬件预设 0/1/2/3；跳过 EQ 到本地存储的旧同步路径 |
| 2644、4130 | `rzDevice25ValkyrieT3` | 同类固件 EQ；额外注册 `configurableVoicePromptFeature` |
| 1462 | `rzDevice25ValkyrieT3` | 固件 EQ 但 `isProfileGlobal:false`、`avoidReadingEQFromFW:true`、仅 custom 有 band；预设 1/2/3/31 |
| 1415 | `AudioBastetT1` | THX generic、电池/dongle；固件 mic、game/chat、THX v4 render/capture 与 fw sync |
| 1420 | `AudioLindaT3` | 系统音量/静音、设备侧音；独立音频适配器 |
| 1427 | `rzDevice40HeadsetAiroha` | `PROTOCOL40_HEADSET`，OUT/IN 各 30 ms、OUT→IN 100 ms、重试 transaction mismatch；实读 ANC、MFB、麦克风、省电、语音提示、AI 降噪与 THX v4/FW 同步 |
| 1439 | `rzDevice40BillieT3H` | `PROTOCOL40_EARBUDS`、`simpleService`、延迟到 MW init 后加载配置；协议 4.0 haptics；basic audio 的硬件麦克风静音分支 |
| 1453 | `rzDeviceJolie` | `PROTOCOL40`；独立 Chroma/Control Pod/bass；固件 EQ 0/1/2/3/16；设备音源、默认输入源有实读初始化 |
| 1465 | `rzDevice40` | `PROTOCOL40_HEADSET`；固件 ANC/EQ/mic、AI 降噪、THX activation、profile 2 的 save-effect；basic audio 实读且不是 hardware-mic-mute-only |
| 2657 | `rzDevice40` | `PROTOCOL40_EARBUDS`；OUT/IN 各 50 ms、OUT→IN 100 ms；固件 EQ、通知式电池、手势/点击速度/提示音/自动暂停/省电；另有 ANC 等级 |
| 2660、2664、2668、2689 | `rzDevice40` | 同类耳塞传输与 generic 功能；当前列表没有 2657 的 ANC；`deviceMode:false`，不可自行追加设备模式初始化 |
| 3334、3337 | `rzDeviceChelsea` | 虚拟通道、Loupdeck；禁用注册通用 hardware event；混音排除 Microphone/Headphone；初始固件门槛分别 `0.2.13`、`0.2.26` |
| 3867 | `AudioCamyT3Device` | 虚拟通道；音频协议使用 `mappingEngine` 并注册 audio service event |
| 3909 | `rzDevice25` | `PROTOCOL25_ESTHER`、单配置；不查询电池、隐藏图标 |
| 3942 | `rzDevice25MarciT1` | `PROTOCOL25_ESTHER`、`thxCarol`、独立虚拟环绕/rear routing service、keep-alive；power mode、haptics、dim lighting |
| 3949 | `rzDevice25` | `PROTOCOL25_ESTHER`、单配置、外部供电、keep-alive；power mode/haptics；未注册 3942 的 `dimLighting` 与 Carol 路由 |

## 功能说明入口

- [播放、EQ 与环绕](playback-and-eq.md)：音量/静音、固件预设索引、参数 EQ、真实端点与前后环绕。
- [麦克风、监听与预览](microphone-and-preview.md)：NoSave 音量、侧音、录音/播放、测量向导、停止和退出清理。
- [混音与本地预设](mixer.md)：1342 效果库与 Stream Mixer 是两条不同生命周期；包含路由、增删改、快捷键、活动窗口与事件消费者。
- [配置档与导入](../shared/profiles-linked-games.md)、[帮助与重置](../shared/help-reset.md)：与音频特有的 restore/设备 factory reset 顺序一起阅读。

## 灯光电源与实际状态

灯光必须跟随每款实际挂载的 Lighting 组件、region 与独立 Chroma 接口。1335 的 region 15、1313 的 region 4、1383 的固件亮度状态、3942 的 dim-lighting generic feature 分属不同入口，不能从通用 `quickEffects` 字段推断全部存在。现有页面的实际选择条件见各 PID 文件。

Power 页面中的省电、关机、充电与电池各有独立状态来源。`powerSavingUnit:"second"` 决定参数单位；`shouldQueryBatteryStatus` 决定是否查询；`hideBatteryIcon` 只决定呈现；`usesExternalPowerSupply` 与通知能力另行配置。隐藏电池图标不能被解读为设备已充电，也不能把无查询值填为 0% 或 100%。

## 原件与阅读边界

每款原件入口为 `.analysis/inputs/frontend/audio-<PID>-current/acquisition.json` 与 `audio-mw-<PID>-current/acquisition.json`。上表由全部 77 款 MW main 的实际 AST `useFeature` 调用核对，具体 lazy feature、参数及页面挂载继续由相应功能正文和逐产品文件追溯。获取来源、部署稳定性、实际字节身份与依赖由收据及 `.analysis/inputs.json` 维护。

深层证据使用 `.analysis/results/frontend/audio-mw-audit.json`、`audio-MapAudio-audit.json`、`audio-surround-audit.json`、`audio-demo-audit.json`、`audio-help-reset-audit.json` 的完整函数与当前原件。函数名带 `getRuntimeData`、`initDataFromDevice` 或返回 `true` 并不自动代表设备读取/确认：正文逐层说明其真实来源及未消费的应答。源码语义、Rust 独立实现、UI/后端接线、纯/模拟验证与真实运行验收分别判断；本文不将静态阅读记为设备验收。
