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

691 OLED 低电量卡保存 `{enabled,value}`，源默认阈值按钮为 `[10,20,30,40,50]`，产品表可覆盖。接收器指示灯在 isDongle=false 时灰态，分别提供 connection status、battery status、battery warning 三种实际 enum。它们与 OLED home screen、键盘 RGB、主机低功耗说明分别拥有状态和命令。

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
