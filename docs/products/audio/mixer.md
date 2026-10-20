# 混音、路由与本地预设

1342 的设备 profile/效果 preset，与 1422/1446 等 Stream Mixer 的独立 preset 库不是同一个对象。它们有不同 GUID、作用域、默认构造、恢复与删除规则。适用产品见 [矩阵](README.md#产品与适用边界)，普通 profile 名称与导入见 [共享配置档](../shared/profiles-linked-games.md)。

## 1342 的文档与恢复

UI 最初的 `ed` 只有临时 Default 身份，没有 DSP 字段。连接处理 `ws` 检查真实 per-device 本地 document；只有 document 不存在的分支调用 `getDeviceDefaultLocalData`，随后进入 feature/device 初始化。页面 restore、缺某个 preset 字段或设备读失败不重新调用工厂。

当前完整 restore 位于 1342 MW `5736.fdd55044a41484df28aa.js`：profile `il` `[1460380,1476924)`、preset `nl` `[1476925,1478142)`、bus `ol` `[1478143,1480459)`。`il` 按精确 `activeProfile` GUID 选择并只遍历实际 own keys。`playbackMix/streamMix/lineOut/voiceChat` 每个实际存在的 bus，先 volume，再 mic/chat/music/game/optical/lineIn/system 的固定路由；voiceChat 排除 chat。mic/XLR output-mixer 分支还受 Basic/Advanced mic setup 条件影响。

`nl` 精确匹配 `activePreset`，发布 preset 的 GUID/name/mapping，再按实际 own keys恢复 voiceChanger、echoReverb、keyShifter、vocalFading。缺 active preset 没有“第一项当默认”的一般 fallback，也不会创建一套新库。每个动作经事件任务、设备 setter 与消费者处理；本地库发布不是设备 ACK。

## 1342 的编辑与取消边界

| 操作 | 当前源码的本地变化 | 后续工作 |
| --- | --- | --- |
| Add | clone `DEFAULTPRESET` 并合并调用者给的新 GUID/name，追加并选中 | 保存后 `nl` 恢复实际效果 |
| Duplicate | 必须找到指定 source GUID，clone 后合并新身份 | 追加、选中、保存、`nl` |
| Reset | 用 `DEFAULTPRESET` 替换精确目标内容，保留 name/GUID/mapping | 保存、发布、`nl` |
| Rename | 修改目标 name、增加 source version | 保存/发布；没有附加 DSP restore |
| Delete | 仅在大于一项时删除；有效指定后继优先，否则此显式删除动作选第一项 | 保存/`nl`；若 long-press mic binding 指向被删项，改为源码 `voiceChanger` |
| Select | 保存给定 active GUID，增加 version | `nl`，不能因同名而改选另一项 |
| Fader | 缺 fader 时只创建该字段对象，与 action payload 浅合并 | 发布旧/新值与版本，按 diff key 排队实际任务 |

“无通用首项 fallback”不等于删项没有后继选择；上表的首项选择是 Delete 的明确分支。UI 输入遵循 32 个 UTF-16 code units 的 maxlength、focus/select-all、Enter blur、Blur 当前字符串、Escape 用原 prop 字符串调用相同关闭回调。产品 callback 以 ECMAScript trim 判空，按大小写敏感完全相等拒绝重名；Escape 正常关闭而不改名。不能统一增加确认/回滚弹窗。

## Stream Mixer 的独立所有者

1422/1446 当前 `Audio_StreamMixer` 注册 output volume/mute、stream monitor、playback device、input add/remove/edit、preset 增删改重置/选择及快捷键。1422 完整类位于 `8539.79323eca355460a8ff0d.js` `[1692782,1722287)`；具体 input exclusions/固件门槛在 1346/1347/1386/1391/3334/3337 等 bootstrap 分别配置，见家族矩阵。

Stream Mixer 库独立保存 devices、presets、activePresetGuid；每个 device 记录启用状态。profile 的 EQ/效果、根 samplingRate 与这份 preset 库分别保存，不能迁成一个全局配置。输出端点、外部 input、当前连接 product ID 与动态 UUID 通过各自真实来源构造。

初始化先创建真实音频包装器和 audio-util，启用设备变化通知，读取实际 mixer mode，再处理 active-window ownership 和预设快捷键，最后启用 meter 并订阅 window-storage event。另一个窗口已经持有 mixer 时，当前设备若仍启用先关闭，再发布“其他窗口活动”；当前设备已启用时重新设为 true，取得当前窗口 ownership 并初始化实际选中 preset。读取/包装器失败和关闭状态必须区分，不能把原版 catch 的 false 当实读关闭。

## 初始写入和输入路由

`initDevice(preset)` 按以下源码顺序执行：stream monitor → stream volume → stream mute → playback volume → playback mute → playback output device → 每条 input 的 level/enable → 可配置且可见 input 的外部路由。enable 受 `isVisible` 与自身 enabled 共同约束。

这里存在必须保留的实际命名差异：初始化把 `playbackMixInputVolume` 传给 `setStreamMixLevel`，把 `streamMixerInputVolume` 传给 `setPlaybackMixLevel`；独立 edit 分支按其字段调用相应 stream/playback setter。不能仅按字段英文猜测交换，并把猜测当源码结论。

输入 Add 使用并行四项 level/enable，汇总每项结果，必要时增加真实外部路由；Remove 分别禁用两路并取消外部路由；Edit 依 `isToggle` 选择 enable 或 level，配置型 input 还更新外部 endpoint。原版局部变量有后一次返回覆盖前一次失败的分支；最终 boolean 不等于所有阶段都成功，实际调用/返回要分别看。

启用/禁用 mixer 先在 runtime 更新目标、向 UI 发未就绪，关闭 routing，再真实 `setStreamMixerEnable`；失败抛入 catch，成功发布结果，再在启用时开启 routing。不能只修改本地 `devices[].isStreamMixerEnabled` 或 UI 开关就结束动作。

## Stream Mixer 预设、快捷键和消费者

Add 生成明确默认 preset；Duplicate 必须精确找到源，否则返回 false；Reset 只保留目标 GUID/name，重建内容；Rename 不触发设备重放。Select、Add、Delete、Reset、Duplicate 保存库后调用 `initActiveStreamMixerPreset`，只有实际 mixer enabled 时才重放；disabled 时源码明确跳过。Delete 仅在多于一项时执行并采用 action 的 selected GUID。

快捷键以真实 virtual key 与修饰位组成唯一身份，map 值是 preset GUID。已有快捷键不重复注册；修改先注销旧项，成功注册后更新 map；取消 mapping 注销对应原生 global shortcut。硬件快捷键通知解析修饰位/virtualKey，匹配目标 GUID，保存 active preset、重放，再产生 3 秒通知。当前翻译按设备 edition/current language 选择，不能用固定显示名称当产品或存储身份。

活动窗口变化读取 window-storage key，交给 `setupActiveStreamMixer`：当前 ownership 不同才处理；空值删除 ownership；其他窗口接管且本设备开启时关闭本设备并保存；本窗口接管时记录窗口并重放精确 active preset。若库中引用不存在，源码没有一般创建默认 preset 的恢复分支。

页面的双 fader、routing matrix、reset popover、快捷键捕获/编辑与 reducer 均有完整所有者；电平、设备音量和静音消费者见 [麦克风与监听](microphone-and-preview.md#电平事件与页面消费者)。退出须分别核对 meter listener、global shortcut、routing notification、native DeviceTerminate 与窗口 ownership；当前完整类没有一个涵盖全部注册项的通用 destroy，不能宣称仅卸载页面便完成所有原生释放。

## 原件与验证边界

1342 完整调用者、preset 输入、所有字段与顺序见 `audio-mw-audit.json` 的 `audio-local-profiles/audio-preset-inputs`。Stream Mixer 的完整 UI/MW、默认构造、设备事件、通知及原生 wrapper 见 `audio-MapAudio-audit.json` 的 `seiren_eq`。这些文档记录当前源码语义，不表示官方 DLL 生产依赖已被替换，也不表示运行验收通过。
