# 键盘灯光与无线电源

本页覆盖全部键盘的 Lighting，以及实际挂载 Power 的 16 个产品。亮度、灯效、熄灯观察、无线休眠和 OLED 电源分别有不同的状态与消费者；本机配置更新、任务完成、真实传输返回和设备读回不能合并为一个成功状态。OLED 内容编辑见 [oled.md](oled.md)。

## 实际挂载与产品条件

所有键盘 Lighting 的基本顺序都是左侧亮度、熄灯设置，右侧灯效。550 与 691 的导航先加载包装组件，再分别到 `712.1b199b2a.chunk.js`、`6664.ee391f68.chunk.js` 的真实页面；包装组件本身不包含设置表单。

| 产品或条件 | 当前顶层调用方的差异 |
| --- | --- |
| 555 | 给灯效组件传本产品 `DEFAULT_CUSTOM_EFFECT_SETTINGS` |
| 654 | 灯效传 `customRadioPicker:true` |
| 642、678、688、679、719、720、721、740、741、742、746 | Factory 配置档显示原版说明并使内容灰态 |
| 728 | Factory 说明还要求 Windows Mode；内容灰态仍只判断 Factory |
| 756 | Normal Mode 先挂电源模式说明；Factory 说明要求 `factory&&normal&&!ble`，内容灰态为 `normal&&(factory||ble)` |
| 574、579、599、605、614、617、619、620、647、655、659、664、673、675、677、717、2595、752 | 顶层无条件挂 Windows 动态灯光组件，组件内部再检查运行状态 |
| 590、600、602、653、658、661、689、697、708、739、642、678、688、679、716、724、727、719、720、721、740、741、742、746、728、756 | 顶层先检查本产品 `isWDLSupported`；642、678、688、716、724、727、728 另传 `supportDynamicLighting:true` |
| 691 | 条件挂动态灯光组件并传 `notSupportDongleWDL:true` |
| 2596 | 当前原件依次挂一个受 `isWDLSupported` 控制的动态灯光组件和一个无条件的同组件；不能将这两个实际调用写成一个 |
| 其余产品，包含 550、747 | 顶层没有额外挂动态灯光组件 |

动态灯光组件读取当前 `isDynamicLighting`、BLE 和调用方参数，决定遮罩、说明及跳转当前设置窗口；设置入口带来源参数和窗口复用参数。`isIframe` 影响目标窗口和文案，不能从产品配置常量直接认定 OS 当前已接管灯光。

Power 的实际挂载顺序如下。其余键盘没有当前 Power 导航，不能因为共享库存在休眠函数就为它们增加页面。

| 产品 | 实际内容 |
| --- | --- |
| 574、585、600、602、631、658、664、697、2595 | `dimLighting`，然后数值型 `powerSavingValue` |
| 708、716、724、727 | 左侧 dimKeyboardLighting、显式 title/tips/description 的低功耗说明；右侧 powerSaving |
| 717 | 同样左右布局，低功耗说明不传上述自定义文本参数 |
| 756 | 左侧 dimKeyboardLighting、FN + Backspace 低功耗说明；右侧 powerSaving |
| 691 | 左侧 dimKeyboardLighting、OLED 低电量警告、接收器指示灯；右侧 powerSaving、FN + ESC 低功耗说明 |

## 亮度、熄灯与当前状态

515 亮度组件使用 `{value,isEnabled}`。开关保留数值，slider 范围 0…100；调整 slider 先发送 enabled=true，数值到 0 时另发送 enabled=false，因此一次拖动可能产生两次设置动作。硬件 Adjustment Mode 有原版阻止/提示路径。共享组件的 Logo 参数由调用方另决定，不能据其中有 Logo 代码就声称每款键盘都显示 Logo slider。

熄灯组件保存 `{isDisplayOn,isIdleEnabled,idleMinutes}`，idle 为 1…15 分钟。亮度关闭使相关内容灰态；产品 `hideLightingIdle` 可以隐藏 idle 组。它配置主机屏幕/空闲观察，而不是把这两个布尔值解释成设备返回的电源开关。

756 的 UI reducer 在 enabled 不变时发送 `ON_SET_BRIGHTNESS`，变化时发送 `ON_CHANGE_ENABLE_STATE`，同时立即更新页面并登记动作；熄灯 reducer 立即更新并发送 `ON_SWITCH_OFF_SETTING`。设备端/中间件返回使用另外的 action。UI slider 已移动、动作入队和设备结果具有明确的时间先后。

MW brightness maker `rf` 读取活动档或 single-profile 数据，更新亮度和 version，保存本机文档，发布 UI completed，之后才排 `NORMAL_SKIPPABLE` 亮度 runner。Two-mode、供电方式、enabled 变化、板载槽和电量限制都有各自分支。键盘硬件亮度事件由 `cf` 处理：增减限制在 0…100，Toggle 保留数值，达到上下限时直接结束，不能把硬件快捷键当成新建配置档。

真实 runner `je` 在开始和后续位置检查取消，按设备区域调用 `setBrightness`，必要时同步活动 GUID 对应板载槽。OLED 设备另有 `setChromaBrightnessStatus`。当前源码在部分 setter 返回非 SUCCESS 时仅记录错误，仍会继续返回 result=true；此函数没有统一的写后 `getBrightness`。文档中的任务 completed 因而不表示所有区域/槽已获得读回确认。

## 主机空闲、屏幕事件与恢复

`pf` 保存熄灯设置并按 display/idle 开关注册或移除观察。取消两个熄灯条件且运行态仍为熄灯时，会排恢复亮度任务。事件所有者 `97161` 将 `displaypoweroff`、`displaypoweron`、`displaypowerdimmed` 分别转换为状态 0、1、2 的 `ON_DISPLAY_POWER_CHANGE`；`systemidle` 转为带真实 idleTime 的 `ON_SYSTEM_IDLE`。

亮度状态机在 enabled 且用户启用屏幕熄灯时，屏幕状态 0 进入 offByRuntime 并请求 `setBrightnessOff`；状态 1 请求重新点亮。idleTime 达到 `60000*idleMinutes` 时标记 systemIdleHandled 并熄灯，重新低于/等于阈值且此前处理过时恢复。OTFM 与宏录制有各自拒绝路径，不会无条件覆盖录制灯效。

消费者 `df` 重新读取活动档、当前供电/Two-mode 条件，使用保留的亮度数值排 on/off 任务，更新运行态 `isLightingOff`。它不是把亮度永久改成零的普通 slider 保存；部分 two-mode 分支另有本机写回。当前函数空 catch 会吞掉异常，因此不能根据它结束推断恢复成功。

display 与 idle 的注册/移除使用对应 handler；模块停止路径移除 window-storage、memory-storage 等订阅，并在设备卸载路径尝试 `setDeviceMode(0,0)`、移除 storage 和 screenunlocked 监听。共享系统 lid-open 分支以新匿名函数 off 的写法不能证明原监听已解除；恢复/释放应按实际 owner 和 callback 身份核对。

## Quick、Advanced 与颜色草稿

Quick 控件依据当前产品的 `QUICK_EFFECTS` 顺序显示，但持久化身份是 effectId。换效果优先使用该效果缓存参数，否则采用官方默认效果设置；Wave/Tidal 的无存值方向还有产品差异。效果表单由 effect enum 和当前产品功能共同选择，包括 Breathing、Reactive、Ripple、Starlight、Wave、Wheel、Audio Meter、Ambient、Battery、Tidal 等；共享表单存在不等于当前产品必定可选。

挂载时读取 Chroma 设备 memory storage、注册其事件、取得缓存效果设置，并处理 Reactive 适用条件；卸载移除 memory-storage 监听。同步按钮取决于实际多设备或 Radiate 支持条件，Wave 另带 indexDirection/numberOfWaveDirection。1200 ms 按钮旋转仅是 UI 反馈。当前 Quick render 在所选 effect 不存在时会调用默认 effect 选择和 changeEffect，确实含副作用；不能将它描述成仅查询或设备已返回默认效果。

Advanced 选择保存 Chroma profile GUID；下拉索引只用来取得 GUID。没有本机 profiles 时显示原版空态。组件观察安装模块和安装完成事件，并通过服务 client 聚焦现有 Studio 窗口或请求本地 Chroma 窗口。原版在线获取、安装/下载和账号消费者不属于本项目 local-first 产品路径；这些原版条件不构成本项目保留联网按钮或禁用占位符的理由。

Quick/Advanced 切换发送 Chroma enabled，退出 Advanced 恢复当前档 quickEffects。BLE 且 useHardwareEffect 时直接挂 Quick，不挂该切换。Chroma Connect/Visualizer 接管通常使普通表单灰态，BLE keyboard 分支另有例外。edition 可以改变效果列表，因此产品/edition 身份必须进入效果选择。

756 effects reducer 将 `ON_SET_EFFECTS` 的 effectId、setting、inactive effect、global 标记和 regionId 与本地显示分开建模；产品 `useHardwareEffectByMW` 时另发 `ON_SET_HARDWARE_EFFECT`。自定义色、Chroma profile、同步 Quick、Chroma enable、节能效果及动态灯光状态使用各自 action，不能用一个“灯光值”包办。

## 保存如何到真实灯光帧

756 `Vf` 更新活动档 quickEffects、inactive 设置、edition 默认值、version 和 runtime selectedEffect，再写本机文档/window storage 并发布 completed。它本身没有排真实帧传输；非 BLE Chroma 设备还会更新本机 Chroma 用户 enabled=false。`Yf` 的 BLE 路径实际调用 `getChromaEffect(profile1,region0)` 并要求 SUCCESS，解析硬件 effect/color/direction/rate；非 BLE 路径处理硬件快捷键事件，在相同效果下按源码轮换颜色/方向，不能冒充一次 getter。

真实消费者是共享 lighting-engine：

```text
设备文档 window/local storage
→ onStorageChange(synapse_ 前缀)
→ onStorageChange_device(设备实例/metadata 活动 GUID/产品)
→ applyRegionQuickEffect
→ syncHWEffect（产品要求时同步或登记退出硬件效果）
→ applyCustomQuickEffect（driver mode 3、custom frame 8、引擎/设备/效果/map）
→ 引擎 processEvent
→ setChromaFrameLightingEngine / setChromaFrame
→ USB 逐行或批量写入
```

硬件专有效果和电池节能效果另走 applyQuickEffect。OLED 菜单灯效还有 `setOLEDMenuChromaEffect`；不能把普通 RGB 行数据当成 OLED 媒体内容。

processEvent 使用全局灯光锁，部分锁占用状态会跳过帧。帧 writer 的部分逐行异常只记录后继续，最后仍构造 SUCCESS；一般帧路径不等待设备效果读回。因此，UI completed、引擎 enable、writer SUCCESS 均不能单独证明所有 LED 已呈现目标帧。完整共享参数、帧锁与退出消费者见 [系统灯光链](../system/customize-lighting.md#quickadvanced-到真实灯光帧)。

引擎退出设置 isExiting，停止主进程灯光帧，terminate 引擎，清理高级/预览及区域对象，执行按产品登记的硬件恢复，清设备实例 map、linkedDevices，并通知主进程允许关闭/已完成。需要恢复的 effect 由当前产品和退出数据决定，不是统一写一份默认配置。

## 无线 dim、休眠和低功耗说明

574 的 dim 是 `{isEnabled,value}`、1…15 分钟；sleep 为数值 `powerSavingValue`、1…15 分钟，可按产品改用按钮组。756 使用独立的 `dimLighting` 和 `{isEnabled,value}` 型 powerSaving，当前两组按钮均为 `[1,3,5,10,15]`。低功耗说明卡本身没有 setter；FN + Backspace 或 FN + ESC 是产品顶层传入的原版快捷键提示，不能为说明卡虚构一个开关。

dim maker `Sy` 保存全局 dimLighting/version 后排 runner；powerSaving maker `Dy` 先读取真实 adapter getPowerSaving，非 UI 来源时按产品条件采用读值，再保存/排任务。旧 `nv`/574 `Tn` 保存数值型 powerSavingValue。读值、修改请求和本机旧值在这些函数中的来源不同。

| 真实适配路径 | 当前检查与结果 |
| --- | --- |
| dim read `ce` | 取得 battery identity 后调用 `getTimeToDim`，要求 SUCCESS |
| dim write `Ae` | 先读，enabled 或秒数不同且 value 为 1…15 才写 `setTimeToDim(0,enabled,60*value,20,…)`；catch 仅 log，没有通用写后读回 |
| power read `ue` | `getAutoSleepMode(0)` 解析 enabled，再 `getTimeToSleep`，将秒转换为分钟 |
| power write `de` | 先读，enabled 变化才 `setAutoSleepMode`；value 变化且在 `(0,60]` 时 `setTimeToSleep(60*value)`；catch 仅 log |
| 旧数值 sleep runner `oi` | `setTimeToSleep(60*minutes)` 后检查取消，再 `getTimeToSleep` 严格比较秒数；不符抛出 `data not match` |

dim/power 新 runner 在 adapter 后检查取消并写 memory cache，但上述 adapter 吞异常，outer completed 不能升级为严格硬件确认。选中的 rzDevice25 `setTimeToDim` wrapper 仅接收 profile/enabled/seconds/brightness 四个参数；adapter 额外传 batteryId 不证明该 wrapper 把它编码进协议。

## 756 Rust 恢复 worker 的读写与结算边界

当前独立实现的 756 电源动作位于 [keyboard_latest_restore_worker.rs](../../../crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_worker.rs)。`Action::Dim` 先执行 `ReadTimeToDim {slot:0}`，只有分钟值为有限的 1…15 且 enabled 或秒数发生变化时，才发送 `TimeToDim {slot:0, enabled, seconds:60*minutes, brightness:20}`。`Action::Sleep` 先分别读取 `AutoSleepMode {slot:0}` 和 `TimeToSleep`，再按差异写入 `AutoSleepMode` 及 0…60 分钟的 `TimeToSleep`。两条路径都在每个 `execute` 前后检查 generation；写入经 `require_acknowledged` 要求非空且无错误的传输应答。

上述 ACK 仍只是本次 SET 的传输回执：这两个 action 在写入后都没有再次执行对应 GET，因此不能把 `Ok(())` 当成设备已读回目标值。官方 756 新 dim/power adapter 同样没有统一写后 GET；旧数值型 `oi` runner 才在 `setTimeToSleep` 后执行取消检查和 `getTimeToSleep` 的严格秒数比较（见上一节）。本项目的对照测试必须把 SET ACK、写后 GET、页面/本机缓存和真实设备状态分别记录。

恢复循环对单个 action 的错误会发送 `Diagnostic("配置恢复：…")` 后继续处理后续 action，最终仍发送 `Finished`；只有外层恢复闭包本身返回错误时，`Finished` 的 `Result` 才为 `Err`。所以 `Finished(Ok)` 不能证明本批所有灯光/电源动作都已 ACK，更不能替代写后读回。取消或旧 generation 会在下一次检查拒绝继续传输，但已收到的 SET ACK 不会因此自动产生回滚。该差异需要在模拟传输测试以及未来的官方 DLL/真实设备对照收据中单独覆盖。

691 OLED 低电量卡保存 `{enabled,value}`，源默认阈值按钮为 `[10,20,30,40,50]`，产品表可覆盖。接收器指示灯在 isDongle=false 时灰态，分别提供 connection status、battery status、battery warning 三种实际 enum。它们与 OLED home screen、键盘 RGB、主机低功耗说明分别拥有状态和命令。

## 756 Power 命令到真实 HID 的输入、应答与释放

上一节的 `ce/Ae/ue/de` 是页面级读改写入口；本节把当前 756 `rzDevice25` 适配器继续追到主机 HID 节点。它只证明 756 这条键盘 Power 分支的静态协议和资源生命周期，不把同一命令头泛化到没有该页面的键盘，也不把 Lighting 的共享帧协议重复记在这里。共享灯光翻译、逐行写入、失败吞收和引擎退出见[系统灯光链](../shared/dynamic-lighting.md)。

### 命令头、负载与响应字段

当前 MW 原件 [rzDevice25.40fe6f559bba152076d4.js](../../../.analysis/inputs/frontend/keyboard-mw-756-current/raw/apps.razer.com/synapse/products/756/mw/rzDevice25.40fe6f559bba152076d4.js) 的 SHA-256 为 `9788a6b975de1b2eb771d167318b74f0d45359a6b0039ff7582af011e9632de3`。Power 包装器位于 `[140022,140615)`；协议函数模块 `34836` 位于 `[42178,50940)`，常量模块 `31509` 位于 `[50940,53652)`。时间字段由同一 MW 依赖的 [main.4ac4b05541bb1859fa0e.js](../../../.analysis/inputs/frontend/keyboard-mw-756-current/raw/apps.razer.com/synapse/products/756/mw/main.4ac4b05541bb1859fa0e.js)（SHA-256 `5e7adbd276b0da6a4a4f06c6dbc7d7b07400b3fd7fd17eb8eea54a7948b4923d`，module `28277` `[139850,142406)`）按大端组合：`E4(seconds)` 是高字节，`l7(seconds)` 是低字节，`Jz(hi,lo)` 是 `(hi << 8) | lo`。因此不能把 Power 的秒数按小端或 UI 分钟值直接写入。

| 页面动作 | 请求头 `[payloadLen,class,id]` | 请求 payload | 成功响应 `data` 的页面解码 |
| --- | --- | --- | --- |
| `getTimeToSleep` | `[2,0x07,0x83]` | 两字节零缓冲 | `data[0]`、`data[1]` 直接用 `Jz` 得到秒数；该 decoder 没有状态或长度保护 |
| `setTimeToSleep(seconds)` | `[2,0x07,0x03]` | `[E4(seconds),l7(seconds)]` | 同上；包装器只拒绝 `undefined/isNaN`，不在自身检查整数、有限性或页面范围 |
| `getAutoSleepMode(profile)` | `[2,0x07,0x88]` | `[profile]` | 仅在 `STATUS_SUCCESS && packetSizeReturned>0` 时取 `{profileId:data[0],enabled:data[1]}`，没有 `>=2` 检查 |
| `setAutoSleepMode(profile,enabled)` | `[2,0x07,0x08]` | `[profile,enabled]` | 同上；`undefined/isNaN` 是包装器唯一参数门禁 |
| `getTimeToDim(profile)` | `[5,0x07,0x8b]` | `[profile]` | 成功且 `packetSizeReturned>0` 时取 `profileId:data[0]`、`enabled:data[1]`、`timeToDim:Jz(data[2],data[3])`、`percentage:data[4]`，没有精确五字节检查 |
| `setTimeToDim(profile,enabled,seconds,brightness)` | `[5,0x07,0x0b]` | `[profile,enabled,E4(seconds),l7(seconds),brightness]` | 同上；包装器要求四个参数不是 `undefined/isNaN`，但值域由页面 adapter 负责 |

上述六个命令都走 `sendCommand` 的默认 `checkTransferIn=true`。适配器 `_createDataSend`（`[112401,112840)`）先构造不含 report ID 的 90 字节 body：`body[0]=0`、`body[1]=transaction`、`body[5..7]=payloadLen/class/id`、payload 从 `body[8]` 开始复制，`body[88]` 是 `body[2..87]` 的 XOR，`body[89]=0`；Electron 主机再补 report ID，实际发送长度是 91。transaction 从 0 到 31 循环，初始值未由这组原件单独证明。该 XOR 是发送帧编码，不等于设备业务确认。

`_getUSBTransferInResult`（`[113928,116968)`）取回帧后保存头八字节和按 `packetSizeReturned=body[5]` 截出的 data，逐项比较 transaction (`body[1]`)、class (`body[6]`) 和 id (`body[7]`)；不匹配会标记重新发送。状态处理是：`0 NEW_COMMAND` 失败并要求重发，`1 BUSY` 失败但继续本次 GET 重试，`2 SUCCESS` 成功，`3 FAILED` 失败并重发，`4 TIMEOUT` 失败并重发，`5 NOT_SUPPORTED` 失败且不重发，其他值为未知失败且不重发。只有另一个 `SetDeviceMode` 命令在显式忽略超时选项时可把 `TIMEOUT` 当成功，Power 六命令不在该例外内。选中的函数没有全帧长度或 checksum 校验；`maxRetryIn/maxRetryOut` 的具体数值也未在本节证据中硬编码。事务／类／命令头匹配和 `STATUS_SUCCESS` 只能表示该传输应答被接受，不能代替设备状态读回。

### HID.node 的实际读写边界

主机 Electron transport 的 [UsbRzDeviceAction.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/UsbRzDeviceAction.js) SHA-256 为 `bdd1b91f94898e7d89e13584d132155526b7ca477275c9b0037876f42f617816`。`hid.sendFeatureReport` 把 90 字节 body 按 report ID 补成 91 字节后调用 node binding；返回的数值只是写入字节数。`hid.getFeatureReport` 取回结果后通常剥去首字节 report ID；如果 binding 返回空值，适配器会填充 90 个零字节再进入上述头和状态解码，不会回退本机配置。发送端与读取端的 `reportId` 选择还不完全相同：读取使用 truthy 检查，report ID 为 0 时会改用 `claimInterface`。

对应原生文件 [HID.node](../../../.analysis/inputs/installed-current/files/app-engine/resources/app.asar.unpacked/node_modules/node-rz-hid/build/Release/HID.node) SHA-256 为 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`，`capture.json` SHA-256 为 `d9e52173142940c7d022bccddcdbb47afe1596e8c2d92190e39929f56d7e95f1`。本批用 `ThreadPoolExecutor(max_workers=4)` 并行核对了 12 个完整函数：每个函数 JSON SHA 与 capture 一致，每条 assembly bytes 与 PE 原字节一致；合计 12 functions、12 chunks、1,989 instructions、8,674 raw bytes。capture 本身 `capture_complete=true`，但全模块 `all_pseudocode_captured=false`，另有一个无关地址失败，不能把这批核对扩大为整个 HID.node 已闭合。

| 节点 | 静态行为 | 函数证据 |
| --- | --- | --- |
| JS `getFeatureReport` binding | 精确接收两个数值参数，分配零缓冲和 Promise 请求，类型为 GET(1)，不先检查 HID 指针是否已打开，排入 worker 队列 | [0x7340](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/7340.json)、[0x5280](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/5280.json) |
| JS `sendFeatureReport` binding | 数组元素按 `uint32→u8` 转换（或复制 Buffer），关闭设备时先报 `Cannot write to closed device`，类型为 SEND(0)，同样入队 | [0x7f40](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/7f40.json)、[0x5280](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/5280.json) |
| worker 分发 | 类型 0 调 `hid_send_feature_report`，返回 -1 时以保存的 HID 错误拒绝，否则 resolve 返回字节数；类型 1 调 GET，返回 -1 时拒绝，否则按请求长度转换 Buffer | [0x4800](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/4800.json) |
| 实际 SET | 空 HID 指针返回 -1；调用 `HidD_SetFeature`，失败读取 `GetLastError/FormatMessageW` 并返回 -1，成功只返回请求长度，没有任何响应读取 | [0x17a10](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/17a10.json) |
| 实际 GET | 空指针返回 -1；以 OVERLAPPED 调 `DeviceIoControl(...,0xB0192,...)`，对 pending 结果调用 `GetOverlappedResult(...,TRUE)`；成功返回 `BytesReturned+1`。此函数没有超时、用户取消或 `CancelIoEx`，也没有走 `HidD_GetFeature` | [0x17ac0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/17ac0.json)、[0x165f0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/165f0.json) |
| Promise 清理与关闭 | Promise 完成／拒绝后释放 buffer、request、错误字符串、function/context；JS close 进入内部 close，取消 I/O、关闭句柄、释放对象并清空指针 | [0x15d40](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/15d40.json)、[0x94e0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/94e0.json)、[0x6170](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/6170.json) |
| worker 停止 | 析构设置 stop、通知并 join，然后清队列；worker 外层检查 stop/wait，但停止检查不是每个已取请求的取消点 | [0x4710](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/4710.json)、[0x50e0](../../../.analysis/results/native/f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f/functions/50e0.json) |

页面 runner 的 abort 检查发生在 adapter 前后；`sendCommand` 到 node GET/SET 的这条链没有传入 abort signal，GET 的同步等待也没有时间上限。`hid.releaseMutex` 只允许持有相同 `senderURL` 的页面释放 mutex；关闭内部会 `CancelIo`，但所选 close owner 没有在释放前等待全部排队请求的证据，worker 析构的 join 才是明确等待点。因而“页面任务已取消”“mutex 已释放”“Promise 已完成”和“设备已恢复原值”是四个独立状态，当前原件没有证明取消会自动回滚 Power 设置。

### 对照边界

本节只完成原件、函数边界、报文编码和错误/释放语义的静态核对；没有运行厂商 JavaScript、主程序、HID.node、官方 DLL 或真实设备。Rust 实现仍需用独立期望测试覆盖六个命令的大小端字节、transaction/class/id 匹配、六种状态、busy/重发、unsupported/unknown、空读取、取消与关闭清理，并在允许的显式对照入口中先只读比较官方实际 GET；可恢复写入还要记录原值、官方写入及独立读回、Rust 写入及独立读回、恢复和清理。当前没有设备或实际运行收据时，Power 写回、固件行为和 UI 视觉结果必须保持未验证。

## 原件与核对边界

各 PID 的完整路径和当前收据见 [产品页](products/)。代表性获取记录：[515 UI](../../../.analysis/inputs/frontend/keyboard-515-current/acquisition.json)、[574 UI](../../../.analysis/inputs/frontend/keyboard-574-current/acquisition.json)、[574 MW](../../../.analysis/inputs/frontend/keyboard-mw-574-current/acquisition.json)、[756 UI](../../../.analysis/inputs/frontend/keyboard-756-current/acquisition.json)、[756 MW](../../../.analysis/inputs/frontend/keyboard-mw-756-current/acquisition.json)、[691 UI](../../../.analysis/inputs/frontend/keyboard-691-current/acquisition.json)、[灯光引擎](../../../.analysis/inputs/frontend/synapse-lighting-engine-current/acquisition.json)。以下为原始 UTF-8 字节半开区间。

| 原件 | owner 区间 |
| --- | --- |
| 515 `main.f60ca5aa.js` | brightness `[7966013,7968431)`；off `[7968844,7971173)`；Quick `[8024490,8031316)`；effects `[8054786,8061345)` |
| 574 `main.4e1cb347.js` | dim `[7597679,7598733)`；numeric sleep `[7599637,7600347)` |
| 756 `main.3fabc4f8.js` | effects reducer `[153683,160246)`；brightness `[201796,204172)`；off `[204199,205309)` |
| 756 `3021.0f2a9d4a.chunk.js` | power `[263428,264428)`；dim `[264522,265466)`；低功耗说明 `[267083,267315)`；Power top `[267968,268498)` |
| 550 `712.1b199b2a.chunk.js` | Lighting top `[94954,95167)` |
| 691 `6664.ee391f68.chunk.js` | Lighting top `[85642,85952)` |
| 691 `Power.0125d4c0.chunk.js` | Power top `[118754,119116)` |
| 756 `6737.90f8e2e80827f3801f8e.js` | brightness maker `[1678309,1681055)`；hardware brightness `[1687765,1690563)`；runtime on/off `[1691959,1693497)`；off maker `[1693497,1696256)` |
| 同 MW | Quick maker `[1737247,1740151)`；hardware effect `[1740151,1745147)`；dim/power makers `[1808814,1811133)`；brightness runner `[1929188,1931091)` |
| 同 MW | dim runner `[2013219,2013757)`；power runner `[2014372,2014910)`；numeric sleep runner `[2015567,2016408)`；observers/handlers `[2094469,2095626)`；dim/power adapters `[2417840,2419073)` |
| 756 `rzDevice25.40fe6f559bba152076d4.js` | sleep/mode/dim wrappers `[140022,140615)`；brightness wrappers `[207480,207863)`；OLED brightness status `[257040,257458)` |
| lighting-engine `main.a383ea9e.js` | storage router `[908613,909145)`；device consumer `[844173,849899)`；region `[823012,825228)`；hardware sync `[825247,826331)`；custom effect `[863131,868219)` |
| 同引擎文件 | frame event `[435194,437352)`；frame writer `[550280,551544)`；exit `[874724,876284)` |

本页是静态追踪的官方行为说明，不表示 Rust/UI 独立实现、真实设备写回或运行时验收已经完成。原件中吞异常、乐观显示和缺少读回的边界均按实际链保留；未运行主应用、厂商 JavaScript、安装器或动态调试。

PID0542/PID0543、PID0521/PID0524 的 NSIS 驱动安装器属于音频设备资源，已移至[安装生命周期文档](../host/firmware-and-installation.md)；它们的包装层不作为键盘 Lighting/Power 协议证据。RzDev 虚拟 HID 只能作为通用驱动边界，真实键盘协议、灯光帧、Power 读回和设备确认仍需由键盘专用原生链核对。
