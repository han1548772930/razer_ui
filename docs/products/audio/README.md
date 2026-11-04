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

## 耳塞自动暂停

2657、2660、2664、2668、2689 均实际注册 `audioAutoPauseControlFeature`。本链通过统一来源选择器读取最新 refresh 原件，审核完整 AST 所有者；所选五款 MW 和 UI 当前均未保留配套全量 capture，这一缺项继续写入收据。入口为 [2657 MW 获取收据](../../../.analysis/inputs/frontend/audio-mw-2657-current-refresh-ec3db4b29c5a-2/acquisition.json)、[2657 UI 获取收据](../../../.analysis/inputs/frontend/audio-2657-current-refresh-ec3db4b29c5a/acquisition.json) 与 [统一 MW 分析收据](../../../.analysis/results/frontend/audio-mw-audit.json)。`auto_pause_chain` 保留五款 MW 与 2657 UI 的语义；`auto_pause_ui_consumers` 补齐其余四款 UI；`auto_pause_hid_chain` 复用两节已校验的所有者，继续到实际报文、当前 Host、原生 Feature Report 应答和结果消费者。共享 Host／原生段只维护在其 `shared_feature_transport` 字段，供其他真实使用 Feature Report 的功能引用。

读取命令为 `[1,24,158]`、参数 `[1]`；写入为 `[2,24,30]`、参数 `[1,0|1]`。成功响应消费 `data[0]` 的 profile ID 与 `data[1]` 的状态。MW getter 在错误或异常时返回 `{isEnabled:false}`，不能据此把设备标为已确认关闭。四款新 main 经实际导出调用命令 helper；2689 仍由设备 chunk 内联调用，不能复用旧偏移替代当前证据。

配置字段为 `audioAutoPauseControl.isEnabled`，工厂默认开启。实际 profile 按 serial metadata 的 active GUID 选择，该 metadata 无 active GUID 时采用首个 profile。恢复时仅在字段缺失时临时采用开启值，先读设备比较，再按差异写入；setter 返回 false 仍继续发布 UI。`getRuntimeData` 则直接重新发布本地 profile 字段，不读取设备。2657 UI 初始关闭，切换先更新 reducer 再发 MW 事件，任一耳塞断开时禁用；profile 消费者也直接使用本地字段。

通用任务先更新、保存 profile 并通知 UI，再排入 `NORMAL_SKIPPABLE` 设备任务。执行返回 false 时结果虽为 false，仍标记 completed 并发送 completed 通知；异常走拒绝与错误分支。取消检查发生在等待执行之后，未把 signal 传入此 setter。自动暂停类没有自己的定时器、监听器或 shutdown；通用 shutdown 仅调用确实存在的 feature 方法。这些通知不能代替设备确认。

五款均实际选择 `rzDevice40`，库版本 `0.0.340`，最大 OUT／IN 尝试次数分别为 20／10。注册的 OUT／IN 间隔均为 50 ms，OUT→IN 为 100 ms；类型为 `AudioHeadsetSpeakerMic`，启用事务号不匹配时重试。Electron 发送 63 字节协议体加 Report ID `2`，共 64 字节；当前 Host `4.0.896` 走 `hid.sendFeatureReportMutex`，旧版本门槛另有普通 Feature Report 分支。出站长度返回 64 后才延迟并调用 `hid.getFeatureReport`，不能把长度相等当业务应答。五款均保留实际类加载器与实例工厂；2689 使用独立设备 chunk、内联命令 helper，其 packet-size override 与其他四款有差异，但自动暂停使用默认 `-1`，本命令内容相同。

当前 Host 的 `node-rz-hid` 为 `0.0.31`。Host 对本链 64 字节原样发送；90 字节且长度为 91／未指定的分支才加 Report ID。原生发送经工作队列调用 `HidD_SetFeature`，成功返回提交长度，失败拒绝 Promise。取回经 `DeviceIoControl(0xB0192)` 与等待式 `GetOverlappedResult`，返回 `BytesReturned+1`，经实际虚表消费者转换为包含 Report ID 的 JS 数组；Host 默认移除首字节再交给 MW。此段复用已有完整 IDA 捕获，核对 17 个函数与两个虚表槽，未重新捕获整个原生库。

MW 逐项核对事务号、命令类和命令 ID，再按状态字节处理：`2` 成功；`0/3/4` 失败并允许重发；`1` 忙时继续读；`5` 不支持及未知状态不重发。假值取回被替换成零数组，不构成设备观测；代码没有独立的响应长度或校验和校验。成功后才消费 profile／state 字节，setter 没有再调用 getter 核对写回或校验 profile 回显。WebUSB 分支使用 class/interface、OUT `9`、IN `1`、value `768`、读取 90 字节及同一 ACK 消费者，实际执行仍未验证。

Electron `finally` 会请求释放 Host mutex；Host 按发送者 URL 释放，移除设备时关闭 HID、取消 mutex 并移除映射。原生完成消费者释放复制缓冲、工作项和 Promise 状态；显式 close 取消 I/O、关闭句柄、释放原生对象并清空指针。所选调用没有传递 AbortSignal，base `destroy/terminate` 为空，close 没有等待 Feature Report 队列及回调全部结束。上述为静态源码边界；真实设备应答、并发移除、完整停止、官方 DLL／设备对照、Rust 独立实现与 UI／后端接线仍分别保持未验证。

其余四款的当前 UI 均实际挂载 `TAB_ENHANCEMENT`，由获取收据逐款追到页面与 DOM 根：

| PID | 当前页面原件与实际分支 |
| --- | --- |
| [2660](../../../.analysis/inputs/frontend/audio-2660-current-refresh-ec3db4b29c5a/acquisition.json) | 主包请求 612 chunk、模块 1546；页面通过 selector 读取断连状态 |
| [2664](../../../.analysis/inputs/frontend/audio-2664-current-refresh-ec3db4b29c5a/acquisition.json) | 开关与页面在 main 中；setup READY 后挂载，页面通过 selector 读取断连状态 |
| [2668](../../../.analysis/inputs/frontend/audio-2668-current-refresh-ec3db4b29c5a/acquisition.json) | 主包请求 348 chunk、模块 1210；Enhancement 类通过连接后的 props 取得断连状态 |
| [2689](../../../.analysis/inputs/frontend/audio-2689-current-refresh-ec3db4b29c5a/acquisition.json) | 主包请求 612 chunk、模块 1546；实际 owner 按本款原件单独核验 |

四款开关初值均为关闭，切换先改 UI、发送 `ON_SET_AUDIO_AUTO_PAUSE_CONTROL` 并追加动作记录；`MW_UPDATE_AUDIO_AUTO_PAUSE_CONTROL_TO_UI` 直接替换字段。本地读取按 `synapse_<PID>.activeProfile` 选择 profile，无 activeProfile 时取首个；存储缺失或 profile 列表为空时返回 false。该 UI 读取路径与 MW 的 serial metadata 选择器分别记录，均不代表设备读回。自动暂停 reducer 只有设置与更新两个分支，没有通用任务完成、错误或回滚处理。

断连禁用由 `hasDisconnectedEarbud` 控制，默认 false；实际电池状态消费者计算两耳状态，CSS 将容器透明度设为 `.5`、阻止 widget 内 div 的指针事件。开关没有独立加载／错误／取消分支。Widget 退出仅取消提示框 RAF；三款延迟加载组件退出时清空本地组件／错误状态，没有取消 import。页面共用消息消费者在 `beforeunload` 移除已登记的 EventEmitter 监听，未关闭接收通道，hook 也没有页面级退订；发送器仅在每次发送后关闭临时通道，发送异常只记录警告。以上不能证明原生任务取消、读回成功或完整 drain。

## PID2657 主动降噪

证据归[统一 MW 分析收据](../../../.analysis/results/frontend/audio-mw-audit.json)的 `headset_2657_anc_chain`。同一收据的 `product_identity_scope` 按当前 DeviceInfo 与硬件身份划分 77 个保留音频端点：49 款实体耳机、1312／1321 两款耳机 USB 适配器，其余不是耳机；该划分不代表全部 Host SKU，也不用 Windows 端点名判断。

只有 2657 注册 `activeNoiseCancellation{supportsANCLevel:true}`，其余四款耳塞未注册。读取 `[1,24,134]`、参数 `[1]`；写入 `[3,24,6]`、参数 `[1,状态,等级]`，固件状态 Off 0、ANC 1、Ambient 80，页面 ANC／Ambient 分别用 10／255，Reduce 切换等级 1／10。Reduce 入口受最低固件 `1.4.6.0` 门控，比较函数不是普通四段序数：相同核心版本但第四段不同仍走阻止分支，缺版本时不隐藏。

配置未设 `initFromDevice`，所以连接时不读设备，构造时的 runtimeState 只是本地默认值。setter 与 runtimeState 相等时跳过 I/O，恢复路径传入的强制参数未被消费；返回的失败结果不被检查也会更新 runtimeState，之后相同重试被跳过。通知 `[5,33,状态]` 只含状态：更新 profile 和 UI，保留本地等级，不更新 runtimeState，也不与写请求关联；缺状态字节时变为关闭。这些本地值都不是设备实测。真实应答、通知投递、独立 Rust、UI／后端接线和官方对照保持未验证。

## 耳塞固件能力（2657、2660、2664、2668、2689）

证据归同一收据的 `headset_earbuds_protocol40_chain`；[冻结原件](../../../.analysis/results/reverse-pending/review-sources/653a1fdb81d7437c1b0fc1f6bdfa06eadddb9ef23d8f22f408e6ee10795a652e.json)保留 1311 个完整 AST 所有者和按产品计算的归一化分组。传输复用 `auto_pause_hid_chain`（Report ID 2、64 字节 Feature Report），硬件通知注册复用 `headset_2657_anc_chain`，不重做。2660／2664／2668 与 2657 属同一构建线，下列所审 MW 所有者归一化后一致；**2689 是较旧的独立构建**，差异单列。收据的 `source_chain_complete` 为 `false`，未闭合范围见本节末。

实际路由：通用事件表在状态机根 `on` 中最后合并，因此 `ON_SET_AUDIO_EQUALIZER`、`ON_SET_POWER_SAVING` 走通用 feature；状态机表与 task maker 中同名的旧分支在这五款上不执行，不能据此实现。

| 功能 | 读取 | 写入 | 响应消费 |
| --- | --- | --- | --- |
| EQ 预设 | 无可达路径 | `[4,24,0]` `[1,1,0,索引]` | 仅记录日志 |
| EQ 频段 | 无可达路径 | `[12,24,1]` `[1,1,10 个等级]`，负值 `0x80\|绝对值` | 仅记录日志 |
| 电池 | `[0,7,128]` 空参数 | — | `data[0]` 左、`data[1]` 右，level 取最小值 |
| 连接状态 | `[1,24,154]` `[1]` | — | `data[1]` 状态 |
| 手势 | `[2,24,162]` `[1,0]` | `[2,24,34]` `[1,0\|1]` | `data[1]` |
| 点击速度 | `[2,24,167]` `[1,0]` | `[4,24,39]` `[1,1,高,低]` | `data[2]<<8\|data[3]` |
| 语音提示 | `[2,24,144]` `[1,255]` | `[3,24,16]` `[1,255,状态]` | `data[2]` |
| 省电开关 | `[1,7,136]` `[1]` | `[2,7,8]` `[1,0\|1]` | `data[1]`，要求返回长度大于 0 |
| 省电时间 | `[0,7,131]` 空参数 | `[2,7,3]` 大端秒数 | `data[0]<<8\|data[1]`，无状态判断 |
| 勿扰 | `[1,24,133]` `[1]` | `[2,24,5]` `[1,0\|1]` | `data[1]` |
| 麦克风静音 | `[2,24,132]` `[1,2]` | `[3,24,4]` `[1,2,状态]` | `data[2]`，1 为静音 |
| 切换音源 | — | `[0,24,29]` 空参数 | 无消费者 |
| THX 激活码 | `[0,24,161]` 空参数 | — | `data[0]` 长度，其余字节解码且不按长度截断 |

连接顺序：`setupOnDeviceReady` 仅在本地存储缺失时创建默认配置，然后初始化 feature、合并状态机事件、注册硬件通知，再读电池。电池为 off（耳塞未连接）时不触发 `DeviceConnected`，也不恢复配置，所以未连接不是初始化依据。已连接时先发 `DeviceConnected`，再加载本地 profile 并按注册顺序执行 `restoreDeviceData`，每个事件各有异常捕获。重连由电池或连接通知在运行状态为 off 时触发；2660／2664／2668／2657 在同一次重连里会强制应用两次 profile，EQ 等恢复写入可能发送两次，2689 无此无条件分支。

- **固件 EQ**：配置档字段 `audioEqualizer`，预设索引 game 1、music 2、movie 3、custom 31，2689 以外另有 voice 4；未知名称编码为 0 且不报错。页面 10 段、-6..6，乐观更新，无确认或回滚。**没有任何可达的 EQ 设备读取**，显示值始终是本地配置。`setAudioEqualizer` 忽略内层结果，传输失败仍返回 true。恢复时总是把本地预设和频段写入设备，属于源码规定的恢复写入。`EQ_SETTING_CHANGE` 在 2657 同线路被分派，但解析器没有该事件的字段，消费者因无效频段直接返回；2689 不分派。页面回退表（game `-3,-3,-4,…`）与 MW 配置默认表（game `2,2,3,…`）不同，产品默认值以 MW 表为准。
- **电池**：`useNotification:true` 使状态机不启动周期检查，但页面在 `batteryValue<0` 时会发 `CHECK_BATTERY_STATUS` 触发一次读取。未连接返回 `off/-1/{}`；每侧 255 映射为 -1；两侧都连接取最小值，否则取最大值或 -1。耳塞分支不读充电状态。通知 `[5,49,左,右]` 和 `[5,56,类型,状态,蓝牙状态]` 无长度校验；电池通知沿用旧 `chargingStatus`，重连时可能发布 off 加新电量；连接状态为 0 时保留旧电量，且不发 `DeviceDisconnected`。页面的 `hasDisconnectedEarbud` 只在非 off 且某侧电量不大于 0 时为真，全部断开时为 `batt-off`，不会触发禁用；电量读取异常反而返回 `NoCharge/-1`，会触发禁用并再次请求读取。这些都不是实测断开。
- **传输差异**：2657 同线路的电池和连接状态命令按 `Mouse(0)` 生成事务号，2689 按实例类型 `AudioHeadsetSpeakerMic(3)` 生成（`|0x60`），THX 五款均按 `DockDongle(7)`。独立实现须保留实际字节。2657 同线路把大于 100 的电量钳为 100，2689 不钳。
- **手势、点击速度、语音提示**：默认配置手势和提示开启，点击速度不在默认配置中。恢复时缺失的点击速度按 normal 处理，设备不同就写入 60，且不存回配置。速度表 fast 30、normal 60、slow 90、slowest 120。读取失败返回 `isEnabled:false` 或 `-1`：手势、提示在本地为 false 时会跳过写入，点击速度则强制写入。页面选择器最低固件 2657 `1.3.6.0`、2660／2664 `1.3.1.0`、2689 `1.0.2.0`；**2668 不传该属性，没有选择器，但 MW 仍注册并在恢复时写入**。
- **省电**：单位为分钟，页面选项 5/15/30/45/60，默认配置 true/5，页面初值 false/15。事件按全局字段保存，而 `restoreDeviceData` 读取活动配置档中的同名字段，默认配置没有该字段，**连接时不会把本地省电写回设备**。READY 广播会实读设备，但读取异常时返回模块默认对象（且该对象会被成功读取改写），不能视为实测。`ON_GET_POWER_SAVING` 没有页面发送者。2657 同线路的 setter 与从默认值起步的 runtimeState 比较，例如设备为 15 时选 5 会跳过 I/O 并报告成功，2689 无此比较。facade 捕获传输错误后，feature 仍报告成功。
- **其他**：勿扰写入前先读，读取异常进入任务拒绝分支。切换音源忽略页面传入值，且无状态更新。麦克风静音通知 `[5,17,状态]` 排入设备事件；2657 同线路解除静音后调用的侧音恢复因没有 `setSidetone` 方法而不写入。`sideToneSupportType:rzDevice` 虽已注册，实际挂载的麦克风页只有麦克风控件，未发现可达的侧音设备路径。

取消与释放沿用既有结论：取消检查在执行之后，不传入 setter；这些 feature 没有自己的定时器、监听器或 shutdown；未证明队列排空。

本节未闭合范围：`audioBasicFeature` 的系统音量／麦克风音量只追到 MW 的 `callSimpleServiceAction`，未追 `simple_service.dll`；`getDefaultSpeakerDevice` 与 `discordCertified` 未追；2664／2668 页面 DeviceInfo 的 `isChromaDevice:true`、`productIdForChroma:3865` 没有灯光页，其他消费者未审；2664 的 `bleId` 在 MW 与页面不一致（2661／2666），消费者未审；页面布局、资源与弹层内容不在本设备链范围内。真实应答、通知、重连顺序、固件断电后是否保留设置、独立 Rust、UI／后端接线与官方 DLL／设备对照均未验证。

## 灯光电源与实际状态

灯光必须跟随每款实际挂载的 Lighting 组件、region 与独立 Chroma 接口。1335 的 region 15、1313 的 region 4、1383 的固件亮度状态、3942 的 dim-lighting generic feature 分属不同入口，不能从通用 `quickEffects` 字段推断全部存在。现有页面的实际选择条件见各 PID 文件。

Power 页面中的省电、关机、充电与电池各有独立状态来源。`powerSavingUnit:"second"` 决定参数单位；`shouldQueryBatteryStatus` 决定是否查询；`hideBatteryIcon` 只决定呈现；`usesExternalPowerSupply` 与通知能力另行配置。隐藏电池图标不能被解读为设备已充电，也不能把无查询值填为 0% 或 100%。

## PID1446 当前初始化与终止

当前链见 [1446 MW 获取收据](../../../.analysis/inputs/frontend/audio-mw-1446-current-refresh-ec3db4b29c5a-2/acquisition.json) 与 [统一音频收据](../../../.analysis/results/frontend/audio-MapAudio-audit.json) 的 `marie_1446_lifecycle_chain`；复核入口为 `audio-marie-1446-lifecycle-chain`。实际启动经 `setupOnDeviceReady`、generic feature manager、`audio_streamMixer` 注册表调用 `Audio_StreamMixer.create/init`。主包的 `supportStreamMixer` 配置与 generic feature 事件启用表是两份独立输入。Mixer 在加载资源前置 `initDone=true`；Marie 初始化返回 false 时只记日志，仍继续其余初始化。

Manifest 指向 `RzNative_05a6_v1.0.7.0.dll`；运行路径则从已安装资源缓存按名称及 `usedBy` 选择，静态清单不能证明实际加载文件。Wrapper 按 `ConfigureFFI → isInit=true → GetDLLVersion → SetNodeFFIEvent → DeviceInitialize` 执行，最后的 bool 不控制 `isInit`。终止先移除 Host 监听，再等待 `DeviceTerminate` 调用但丢弃其 bool；`beforeunload` 不等待终止，也不注销原生回调或请求 `FreeFFI`。

当前原生版本函数真实返回 `1.0.7.0`。本批复用 1422 的精确共享证据，比较 22 个生命周期函数；17 个机器字节相同，5 个函数的地址差异均追到实际字符串或表。真正的产品选择差异是驱动 CLSID `{BB4BBA6B-01EF-4028-9BB7-6E03446CC32F}`，不能套用 1422 的注册路径。初始化超时不取消排队任务、container-only 复用、终止仅排队移除及 meter 超时继续析构等限制仍保留；实际安装路径、注册路径、加载模块、完整回调停止、独立 Rust 与 UI／后端接线、官方 DLL／设备对照和运行验收分别保持未验证。

## PID1446 多功能开关

当前静态链见 [1446 MW 获取收据](../../../.analysis/inputs/frontend/audio-mw-1446-current-refresh-ec3db4b29c5a-2/acquisition.json)、[1446 UI 获取收据](../../../.analysis/inputs/frontend/audio-1446-current-refresh-ec3db4b29c5a/acquisition.json) 与[统一 MW 分析收据](../../../.analysis/results/frontend/audio-mw-audit.json) 的 `multi_function_1446_chain`；复核入口为 `audio-1446-multi-function-chain`。实际挂载从 DOM、Redux Provider、setup READY、产品根组件进入 `MIC` 路由，再以同包 `Promise.resolve` 加载模块 5303。开关使用 `broadcasterReducer.isMultiFunctionEnabled`，点击先更新控件和 reducer，再发送 `ON_TOGGLE_MULTI_FUNCTION`；关闭后双击、三击映射行采用 disabled 样式。静音键映射的实际消费链见下一节，不能由本开关推断完成。

MW 注册 `multiFunction` 时设置 `initFromDevice:true`，但它的连接初始化只补齐本地 profile 缺失字段，默认 `true`，没有读取设备。`getRuntimeData` 也只发布当前本地 profile。恢复时先查询 profile 1、SingleTap，再比较设备 `status===1` 与本地字段；不同或返回非 SUCCESS 时写入，getter 抛异常则由 manager 捕获该事件。setter 返回 `false` 仍不阻止恢复路径发布本地值。缓存读取和持久化均为同步实现，按 serial metadata 选择 active GUID，无该 GUID 时选首个 profile；这些数据不是设备实测。

真实适配链为 `4076 → 66744 → 83266`。GET 命令为 `[4,15,147]`、参数 `[1,0,0,0]`；SET 为 `[4,15,19]`，开启参数 `[1,1,0,1]`、关闭 `[1,2,0,1]`。输入枚举 SingleTap 为 0、CycleEffect 为 1；通用响应显示枚举却把 tapMode 0 标为 doubleTap，保留这一源码差异，恢复逻辑实际只比较原始 status。成功解码消费 `data[0..3]` 的 profile、status、tapMode、functionId，没有另行校验 payload 长度。

本产品使用 claim interface 3、Report ID 7、65 字节报告；发送间隔 5 ms、OUT 到 IN 25 ms、IN 间隔 5 ms，最多 OUT 20 次、IN 10 次。64 字节协议体含事务号 1 至 31、8 字节头及参数；对 `[0,62)` 做 XOR，写到索引 62。当前 Host 原样发送该报告，取回后移除 Report ID；MW 核对事务号、命令类和命令 ID，再处理状态字节，发送长度 65 不是业务确认。setter 没有后续 getter 或字段回显确认；原码没有独立的响应长度、校验和验证。只复用现有 `shared_feature_transport` 的 Host 与 17 个原生 HID 函数，不复用耳塞的 Report ID 2、64 字节产品格式，也未重做 Marie 生命周期。

通用任务先持久化、通知 UI，再排入 `NORMAL_SKIPPABLE/multiFunction`。setter 的异常或非 SUCCESS 被转换为 `false`，runner 仍可将任务标记 completed 并发送 completed 通知；开关 reducer 不消费这些通用通知，也没有据此回滚。取消检查发生在设备执行之后，没有向 setter／原生队列传入 signal。feature 自身无计时器、订阅或 shutdown；Electron `finally` 释放 Host mutex，不能证明原生队列和回调已排空。UI 异步加载退出只清空组件与错误，未取消 import；共享消息清理移除监听但不关闭接收 BroadcastChannel。

本批仅完成该功能的当前静态语义核对及维护工具反例测试，未执行厂商 JS、DLL、服务或设备，未产生新的原生捕获、分析会话或临时目录。真实 ACK、断连／取消、完整释放、独立 Rust、UI／后端接线与实际 DLL／设备对照分别保持未验证；捕获中其他函数的 Hex-Rays 失败事实继续保留，不算作所选 17 个函数失败，也不据此宣称全库完整。

## PID1446 静音键映射与实际 tap 消费者

当前证据归[统一 MW 分析收据](../../../.analysis/results/frontend/audio-mw-audit.json) 的 `mute_mapping_1446_chain`，复核入口为 `audio-1446-mute-mapping-chain`；从上一节精确收据与 `/unresolved_chains/2` 接续，复用已核验的挂载、配置档、通用任务与 Camy 传输所有者。界面仍是实际 `MIC` 页面，只编辑双击、三击两行；菜单依次为循环、开关、分隔、禁用标题与 8 个指定 QuickEffect（ID 顺序 `12,2,8,3,7,1,4,13`）。编辑克隆完整 mappings，按稳定 inputID 替换一行并删除该行旧输出对象，保留其他映射。未知 assignment 的 `findIndex` 仍为 `-1`，下拉组件因此不呈现，不能改写成默认首项。

`ON_SET_MUTE_BUTTON_MAPPING` 进入通用权限与任务链，先按当前 serial profile 保存 `mappings`、发布 `MW_SET_MAPPING_TO_UI`，再排入 `NORMAL_SKIPPABLE/muteButtonMappings`。该 feature 的 `execute` 只检查 payload 与 mappingList 是否存在，没有设备映射 setter、getter 或协议写入；completed 只表示该本地处理完成。实际显示读取 `customizeReducer.mappingList`，编辑事件由 broadcaster reducer 发出，两者不能混为同一状态所有者。注册使用 `initFromDevice:false`；恢复只补缺失本地字段并发布，运行时同样发布本地 profile，不是设备读回。

实际 tap 从注册的 Mapping Engine 硬件事件按 containerId 进入 Protocol25 parser：`buffer[0]=5`、`buffer[1]=67`，`buffer[2]` 为 tapMode，`buffer[3]` 为 functionNumber。当前 selector 只使用 tapMode，`0/1/2/3` 分别查单击、双击、三击、长按的稳定 inputID，再从事件到来时的当前 profile 取得映射。缺文档、配置档、映射或未知 tap 不派发动作；callback 自身没有再次检查多功能开关。当前菜单只产生三类 chromaEffectGroup 输出，不据通用消费者中的其他分支新增 UI 功能。

循环按当前 QuickEffect ID 取下一项，未知 ID 从第一项开始，映射中的 direction 不参与这一计算；指定灯效使用 config 设置或源码默认表，其中 Spectrum、Fire 没有对应默认设置对象。两者均派发 `ON_SET_EFFECTS`，实际 maker 保存已有 profile 的 quickEffects、更新运行时缓存并发布 UI；本产品未注册 OLED 或 saveEffectToDevice，不能把该 maker 的 completed 当作灯光设备 ACK。软件灯光后台输出见后文，tap 通知生产与注销边界见下一节。

开关要求 brightness 对象具有 isEnabled 字段，克隆后反转该字段，派发 `ON_CHANGE_ENABLE_STATE`，先保存并发布本地状态，再执行亮度任务。当前 Camy 调用为 `setChromaBrightness(value) → newChroma_setChromaBrightness(1,0,value)`，命令 `[3,15,4]`、参数 `[1,0,floor(min(value,100)/100*255)]`，关闭时 value 为 0。底层沿上一节的 Report ID 7、65 字节报告与精确共享 Host/HID 证据；包装层捕获异常后仍返回参数对象，runner 不核 setter status，也没有 getter 读回。因此本地显示、排队、任务 completed 与真实设备确认分别记录。

本批保留硬件监听的替换、注销及 deviceMap 清理，映射 feature 无自有计时器、监听或 shutdown；下拉组件退出移除 click/blur 监听。未证明原生队列与回调排空，也未执行厂商 JS、DLL、服务或设备。维护工具反例测试覆盖当前来源、完整 AST、菜单与状态消费者、实际 tap、命令、失败语义、依赖变化和发布失败；未创建新捕获、分析会话或临时副本。独立 Rust、UI／后端接线、实际 DLL／设备对照和运行验收分别保持未验证。

## PID1446 tap 通知生产、投递与注销

证据归同一[音频收据](../../../.analysis/results/frontend/audio-mw-audit.json)的 `tap_notification_1446_chain`，复核入口为 `audio-1446-tap-notification-chain`；精确接续 `mute_mapping_1446_chain/unresolved_chains/0`，复用共享主机通知、USB 生产与停止证据。当前产品选择普通 Protocol25 parser，开启 `skipFilterDriver`，实际调用 `addUsbDeviceWithoutFilterDriver`。原生配置关闭过滤驱动，保留默认 `connectHid=true`，经 USB 构造和 HID 初始化启动原始输入；静态资格来自本产品，没有借用其他 PID 的 BLE、固件或 report 表。

本产品声明 report 4 为 razerKey、report 5 为 hardwareEvent；bootstrap 配置 `setMappingEngineLocalStorage:false`，实际音频配置写入者调用 `window.localStorage.setItem`。这些声明与写入不证明原生 descriptor 已发布，也不能据当前未解析的其他发布者判定功能不存在。已核实的原生 classifier 在硬件类别或未映射／无 descriptor 分支均把原始 report 5 交给 category 8，完整保留前缀。HID 输入使用重叠读取，容量 100 字节，按实际完成长度复制到设备队列；通知 callback 序列化原始无符号字节及 tick，再由 Host 按当前设备 key、订阅 URL 和窗口存活状态投递。

MW 先按 containerId 查 `deviceMap`，再等待实际 parser 的 lazy import；等待后不复查 map 或连接代次。Protocol25 消费 `[5,67,tapMode,functionNumber]`，没有独立长度校验；普通产品分支匹配 productId 后进入既有 tap 消费者，按事件到来时的当前配置档执行。映射、QuickEffect 和亮度输出语义复用前文。本地注册成功不是一次物理 tap：Host 无过滤驱动注册路径实际判断 callback 对象的真值，未检查其 `result` 字段；共享硬件注册也不消费 callback 设置的布尔结果。

`Ds` 先移除并清空 tap listener，再等待注销。MW 对履行的注销 promise 即使结果为 false 仍删除 map；拒绝可能保留 map，但 listener 已移除。Host 先删 URL，再执行最后一个原生注销；失败不恢复订阅。原生注销仅清注册与间隔状态，不清 callback、不取消 HID 读取，也不等待已复制到 owner 队列的 callback；已排队对象不重查注册状态。完整移除另有取消／关闭 endpoint、等待设备线程退出的链，但未证明 owner、FFI、Electron 全部排空。普通退出先 `Ds` 后最终移除，更新清理先移除后 `Ds`；外层异常可能跳过后续步骤，没有统一 finally 屏障。

本批只闭合这条静态通知链，复核实际源码、完整机器码及现有伪代码，不创建原生捕获、分析会话或临时副本。当前固件／传输的真实报文、原生 descriptor 发布状态、实机 tap、跨层连接代次排空、独立 Rust、UI／后端与实际 DLL／设备对照分别保持未验证；全产品及全源码逆向完成状态不提升。

## PID1446 QuickEffect 软件输出

当前输出链归同一[音频收据](../../../.analysis/results/frontend/audio-mw-audit.json)的 `quick_output_1446_chain`，复核入口为 `audio-1446-quick-output-chain`；详细帧、传输及停止语义统一维护在[灯光专题](../shared/dynamic-lighting.md)。实际 LE adapter 是 `rzDevice25AudioCamyT3V2`，与本页 MW adapter 分开。profile 保存触发软件效果，5×12 矩阵映射到设备的 1×10 灯位。

当前原生驱动对协议名精确查表，不能接受该 LE adapter；未消费帧随后进入 JS fallback，沿实际 Camy／batch 写出 Report ID 7、65 字节报告。发送长度、batch 完成和软件 SUCCESS 均不是设备 ACK／读回。八种效果的实际参数绑定已由 `quick_effect_binding_1446_chain` 接到既有共享数值算法，详细依据见灯光专题；完整跨层停止、恢复、独立 Rust 和实际 DLL／设备对照继续分别保持未完成或未验证。本产品 tap 通知的静态生产与停止边界见上一节。

## 原件与阅读边界

每款基础原件入口为 `.analysis/inputs/frontend/audio-<PID>-current/acquisition.json` 与 `audio-mw-<PID>-current/acquisition.json`；已有更新 snapshot 的路由使用统一来源选择器，并保留候选、获取失败和 capture 缺失，不能固定读名称带 current 的旧目录。[1422](products/1422.md) 的 UI 已定位到 `snapshot-ed4fa81d`，其当前挂载与 meter 消费者归 `audio-MapAudio-audit.json / seiren_eq_current_ui`。上表由全部 77 款 MW main 的实际 AST `useFeature` 调用核对，具体 lazy feature、参数及页面挂载继续由相应功能正文和逐产品文件追溯。获取来源、部署稳定性、实际字节身份与依赖由收据及 `.analysis/inputs.json` 维护。

深层证据使用 `.analysis/results/frontend/audio-mw-audit.json`、`audio-MapAudio-audit.json`、`audio-surround-audit.json`、`audio-demo-audit.json`、`audio-help-reset-audit.json` 的完整函数与当前原件。函数名带 `getRuntimeData`、`initDataFromDevice` 或返回 `true` 并不自动代表设备读取/确认：正文逐层说明其真实来源及未消费的应答。源码语义、Rust 独立实现、UI/后端接线、纯/模拟验证与真实运行验收分别判断；本文不将静态阅读记为设备验收。
