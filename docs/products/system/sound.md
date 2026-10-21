# 系统：声音、空间处理与本地应用预设

当前独立 Sound 页面仅在 PID694、695、696、709、710、711、736、737 的实际导航树中出现。声音设置作用于当前系统播放端点及其本地音频处理器；内置扬声器与 3.5 mm 耳机分别选择 `standard`／`headphones` 配置，不能仅依据页面上的产品图判断端点已连接。

## 版本和实际挂载

| 产品 PID | 当前源码分支 |
| --- | --- |
| 694、695、696 | 旧式空间音频页面；挂载应用输出、系统声音属性、音量、归一化和语音清晰度；产品 DeviceInfo 没有 `thxVersion="thxv4"` |
| 709、710、711 | DeviceInfo 明确 thxv4；`showSpatialUI` 控制应用输出和增强卡片，隐藏时保留系统音量与声音属性。709／711 的页首另传 `isThxOfBlades=true` |
| 736、737 | 同属 thxv4；增加独立 THX 音频客户端运行状态遮罩与页面禁用 |

这八个页面均仍挂载旧 EQ chart，并传入 `style.display="none"`。被挂载的组件会执行自己的订阅和初始化，但不能将其描述为用户可见的第二套 EQ 页面。694／695／696／710／736 类中保留 profile 新建、复制、改名和删除方法；当前 Sound render 没有因此自动出现相应按钮。

thxv4 的 `rc` hook 只有在支持值成为布尔值后才决定显示空间 UI。true 显示，false 隐藏；“由不支持变为支持且未关闭过提示”才打开新功能提示。它保存 `previousThxState`／`popupClosed`，关闭后执行调用方的选页回调。hook 返回独立 `isLoading` 字段，但 PID737 外层 wrapper 未消费该字段，不能据此称页面已挂载读取遮罩。

## 打开、端点选择和读取

Sound 页 `componentDidMount` 调用 `getSettings`，并从 Redux 的空间音频状态同步页首开关。thxv4 中间件实际枚举 OS 音频端点与默认播放端点，按产品 `patternAudioName` 或 Realtek 名称模式过滤；特定 containerId 标记内置扬声器，其他匹配项标记 3.5 mm。它为所选端点构造产品 ID 与 `speakers`／`headphones` 标识，调用音频处理器 AddDevice、CheckStatus、启用 EQ，并核对所需驱动版本后发布空间支持值。

默认端点改变时重新读取系统音量／静音，发布当前端点信息，选择该端点的本地预设，再分别提交空间开关与 EQ 恢复事件。旧版路径先等待 2 秒，调用其音频设备清单和 render device API，再按真实 plugType 选择内置或 3.5 mm。两者不是一条固定端点路径。

thxv4 属性边界中，`deviceInfo` 是 `{productId,endpointId}` 序列化结果；GetProperty 返回 JSON 后解析，SetProperty 发送设备身份、属性名、序列化值及可选类型。原版通过官方 native 音频层实现这些调用；这里记录其契约，不把该 DLL 包装器视为本项目已完成独立实现。

## 音量、空间开关和增强

| 控件 | 数值与实际读写 |
| --- | --- |
| 音量／静音 | 0…100、步长 1；滑到 0 会将组件 `isEnabled=false`。thxv4 使用 OS 默认播放端点的 `simpleGetSpeakerVolume`／`simpleSetSpeakerVolume(deviceId,!isEnabled,value)`；旧版使用 render volume／mute getter 和 setter |
| 空间音频 | 当前配置档的 `thxSpatialAudio.isEnabled`；thxv4 写 `spatial_enabled`，缓存已确认调用值以跳过重复值；旧版读取当前 processing 后切换，部分版本还重新应用当前 EQ |
| 声音归一化 | 独立 `{isEnabled,value}`；通用控件仅当 `hasSlider` 才显示滑块。产品配置 min=0、max=100；thxv4 读取值乘 100，写入除 100，属性为 `volume_leveling_enabled`／`volume_leveling_level` |
| 语音清晰度 | 0…100、步长 1，开关与值分开；thxv4 的 level 同样在 0…1 与 0…100 间换算，属性为 `voice_clarity_enabled`／`voice_clarity_level`。旧版方法名称使用 VocalClarity |

组件先保留编辑值，再发相应 action；外部 props 改变后同步。maker `ud` 更新当前配置档字段、版本并广播本地完成，然后按功能队列排入设备任务。具体 helper 先读当前开关／level，仅在不同且范围有效时写；关闭开关时不必改 level。其返回对象保留 `isSetEnableSuccess`、`isSetLevelSuccess`、`errorMsg`，其中 `notCallingAPI` 表示没有调用，不能解释成 setter 成功。

空间开关的 maker `od`、setter runner 和本地完成也是独立消息。若 helper 捕获异常只返回错误描述，泛型 runner 仍可能推进 completed 和任务缓存；原件没有统一的写后读回，也没有把这些失败全部回滚为默认值。声音属性与系统音量混音器入口调用本机系统功能。

## 本地 EQ 数据和编辑生命周期

产品固定数据保存 standard／headphones 两套预设，频率为 31、63、125、250、500、1000、2000、4000、8000、16000 Hz，增益 −12…+12 dB。隐藏的共用 chart 对 standard 只编辑后七段，向中间件提交时前补三个 0；headphones 编辑十段。选预设立即提交；拖动先改组件 bandData，window mouseup 才提交；Reset 恢复当前预设默认曲线，Reset Zero 清零当前模式。组件在挂载时注册 mouseup／hoverPoint／unhoverPoint，卸载时移除同一处理器。

EQ maker `sd` 更新当前配置档的 selectedPreset 与 current 数组，并把所有配置档的 selectedMode 同步为当前播放端点类型。它先保存与发本地完成，再排队发送。thxv4 先按需 SelectPreset(`profile_presets`, presetName)，再逐段设置 `eq_band_1_gain_db` 至 `eq_band_10_gain_db`，类型 Float；任何一个 SetProperty 返回 false 时提前停止后续段。旧版则按 SetRenderPreset → Batch(true) → EQGains → Batch(false) 的顺序执行。

这些方法有捕获异常后更新 runtime EQ 数据的路径；setter ACK、缓存改变和十段全部写入是不同证据。可见 Sound 页隐藏 chart 的事实也不代表不存在上游本地预设消费者。

## 应用预设与前台事件

应用卡片按名称排序，移除缺失项；同一个 exe 可有数组形式。每项以 gameId、exeName 和输出 `{type,guid}` 保存。新增入口显示本地应用选择，删除先询问再更新本地移除清单；滚动关闭下拉和删除提示。某些 exe 的 Immersive／Competitive 输出由当前版本固定映射表追加，不能给所有程序都显示该选项。

前台应用事件进入 `taskMakerSetActiveSpeakerEqualizer`：先找当前配置档的 applicationPresets；普通输出解析成 game/movie/music/voice/custom 的本地增益，支持的 immersive／competitive 输出从版本固定表选择环境名。命中时设置当前应用预设；未命中且此前有应用覆盖时恢复配置档选中的预设。最终排入独立 `activeEqualizer` 队列，并按需要调整空间处理。页面保存应用映射与当前前台实际激活值分开。

## 独立客户端与清理

736／737 的中间件为本地 THX 客户端调用 registerAppLaunched，成功后注册 `appLaunchedChanged`；页面取得运行状态时显示遮罩。destroy 明确移除事件处理器并调用 unregisterAppLaunched。音频处理器的 terminate 调用 RemoveDevice；端点切换前也先移除已添加的设备。普通页面卸载的 chart 监听清理不能代替这些中间件资源释放。

## 实际页面与当前 Rust 对照

PID737 的实际导航先取得 `Xn → Jn`。产品根 `renderView` 在选择 Sound 时用 `cloneElement` 注入 `showSpatialUI` 等字段；外层 hook wrapper 又拥有新功能弹层，所以只读导航中的 `Xn({})` 会漏掉真实能力来源。`Jn` 始终挂载页首 `go` 和 `display:none` 的 `rn` EQ；随后按 `showSpatialUI` 改变两列。开启空间 UI 时，左为应用输出和系统声音属性，右为音量、声音归一化、语音清晰度；关闭时，左只保留音量，右只保留系统声音属性。客户端运行遮罩是另一条件，与 hook 的未观测字段不同。

| 页面或动作 | 当前官方行为 | 当前 Rust 对照 |
| --- | --- | --- |
| 可见树与隐藏 EQ | `rn` 虽挂载但隐藏；可见树随空间能力切换列和卡片，不能把 EQ 数据表当作可见页面 | [system/mod.rs](../../../crates/razer-pages/src/features/system/mod.rs) 的 `sound` 在左列依次画 volume／normalization／clarity 和 THX 卡，右列始终画 `equalizer`，包含扬声器／耳机、预设和逐段 slider。这将原版隐藏 EQ 变为可见编辑器，并改变实际卡片顺序和能力分支 |
| 页首、能力与客户端状态 | 页首按真实输出类型选择图像和文案；空间支持成为布尔值后才显示相应控件；736／737 的真实客户端状态可禁用页面并挂独立遮罩 | Rust 此 owner 没有对应端点／空间支持／客户端状态观察、页首图像和新功能弹层；只要 profile 字段存在就显示卡片，THX toggle 固定可用。不能用本地默认值或是否有字段证明能力已观测 |
| 音量与增强控件 | volume 自身含系统混音器入口；normalization 的 `defaultProps.hasSlider=true`，受开关和实际输出来源条件影响；voice clarity 另有两段说明 | Rust 用相同通用 toggle／range 结构呈现三项，未接实际端点、设备音量变化与完整说明／提示；另一个 `ms-settings:sound` 按钮是有效 OS 入口，但不能代替音量读取、混音器、空间处理器或增强 setter |
| 本地应用输出 | 按真实目录排序、过滤 missing；新增、输出选择、删除确认、滚动关闭菜单，以及前台应用触发预设是不同动作；保留本地模式，移除范围外在线链接 | Rust `sound`／`equalizer` 没有应用列表、添加／删除／输出选择或前台覆盖消费者。项目其他地方能保存 linked games 或提供音频产品页面，不证明笔记本 Sound 已接通这条链 |
| 草稿、持久化与设备结果 | 端点切换、profile EQ、空间开关和 DSP 属性分别有 producer、队列与完成语义；隐藏 chart 仍执行自己的订阅和释放 | `SystemProductChanged → source_workspace.capture → WorkspaceEvent::Changed` 只产生当前 profile 快照；本轮沿 [workspace 生产订阅](../../../crates/razer-pages/src/features/workspace/source_workspace.rs)、[Shell](../../../crates/razer-shell/src/shell.rs) 与 [保存后续操作](../../../crates/razer-shell/src/shell/persistence.rs) 未找到该系统声音页的端点／DSP 提交、读回、事件、取消与释放。Seiren 和其他音频产品的独立 Rust 服务属于另一条已接线能力，不能替代本页链 |

客户端运行遮罩的后载 CSS 将 wrapper 置于 top 85px、z-index 1；内容宽 600px、上边距 85px、背景 `#111`、1px 绿色边框、5px 圆角和 20px padding，标题为 RazerF5 16px／15px，正文为 Roboto 14px／17px。它与新功能弹层拥有不同的布局和关闭消费者，不能合并为通用错误提示。当前这些尺寸只经完整规则核对，窗口、焦点、字体实际度量和像素仍未验收。

后续自动化验证需覆盖支持值未知／true／false、不同真实端点与默认端点切换、客户端状态到达／释放、应用 missing／增删／前台恢复、隐藏 chart 挂载与卸载，以及真实读取与本地草稿不同的情况。协议对照分别核对 volume/mute、DSP 字段换算、逐段 EQ 失败停止、通知顺序、RemoveDevice 与退出释放，不能用已保存的增益、UI 已切换或 setter 返回 true 证明全部生效。官方实际 DLL／服务与设备对照未在本轮执行；本节也不将 PID737 的具体树扩展为其他七款页面全部一致。

## 原件与复核位置

所有路径以 `.analysis/inputs/frontend/` 为前缀，读取核对各自 `acquisition.json` 的原始字节长度和 SHA-256；区间为 UTF-8 半开字节区间。

- PID737 UI `system-737-current/raw/apps.razer.com/synapse/products/737/ui/static/js/8923.a0bbaf75.chunk.js`：音量 `[196806,198371)`；系统声音属性 `[198662,199848)`；归一化 `[201609,203062)`；清晰度 `[203368,204547)`；页首 `[204908,205842)`；EQ `[212143,228289)`；应用预设 `[281091,289304)`；Sound `[290640,292317)`；空间 UI hook `[395291,396995)`。
- 同一原件：Sound connect 完整绑定 `[292323,292935)`；产品根 `renderView` 完整 arrow `[407940,408215)`；hook 外层完整 function `[412833,413240)`；新功能弹层完整绑定 `[396996,398302)`；客户端运行弹层完整绑定 `[290268,290630)`；归一化默认参数赋值 `[203062,203113)`。以上范围沿完整 JS 的 AST 和词法绑定复核，获取收据、入口、manifest 和相关 CSS 的实际字节均匹配，读取前后身份未变。
- PID737 同 source set 的 `static/css/8923.779862c6.chunk.css`：客户端运行遮罩五条完整规则 `[69791,70918)`。它们提供当前布局依据，不代表本项目窗口运行结果已经比较。
- 同产品中间件 `system-mw-737-current/raw/apps.razer.com/synapse/products/737/mw/6737.85397ce157d4cca228ee.js`：playback maker `[860908,861513)`；空间开关 maker `[861513,862810)`；EQ maker `[864533,866412)`；前台 EQ 消费者 `[866412,867845)`；通用 feature maker `[870019,871330)`；EQ runner `[1233957,1234777)`；前台 runner `[1234917,1235216)`；空间 runner `[1244365,1244887)`。
- 同一中间件：端点读取和重选 `[1274381,1279684)`；归一化／清晰度／音量 helper 表 `[1280786,1284348)`；空间处理 helper `[1284697,1285828)`；EQ helper `[1285912,1287578)`；native 属性模块 `[1577326,1594856)`，其中 GetProperty `[1581865,1582116)`、SetProperty `[1582117,1582313)`、逐段 EQ setter `[1587019,1587289)`、terminate `[1583436,1583522)`；客户端监控注册 `[2029215,2029717)`、注销 `[2029718,2030232)`、init/destroy `[2030662,2030886)`。

本页为静态源码说明；未执行音频 DLL、厂商 JavaScript、主程序或真实音频设置。原生 DSP／系统服务实现、Rust 独立实现和真实验收仍按各自证据记录。
