# 系统：键盘自定义、系统快捷键与灯光

当前 41 个系统产品均挂载 Customize 和 Lighting。Customize 使用系统键盘布局、普通／Hypershift 两层映射和独立的 Fn 主功能状态；Lighting 使用当前配置档的灯光设置，并由中间件和全局灯光引擎分别处理亮度、硬件指示灯与软件灯效。产品清单和导航顺序见 [系统索引](README.md)。

## 实际挂载和产品条件

| 条件 | 当前源码中的产品分支 |
| --- | --- |
| Customize 的 `hasKeyboardControls=true` | 586、587、589、594、597、616、623；其余显式为 false。所有产品均传 `showMouseUse=false`、`isSystemDevice=true` |
| 外层显式提供 Fn／Windows counter key 列表 | 709、710、711、737；736 外层不传。系统键盘组件内部另有自己的 F1…F12、Windows 列表，两层用途不能混同 |
| Snap Tap | 逐一检查 41 个产品的实际系统键盘 render，均在非宏模式挂载开关与键对编辑器；并非仅 711／737 或默认配置含此字段的产品才显示 |
| Logo 下拉显式关闭 | 589、597、618、624、652、669、694 |
| Logo 参数未传 | 569、594、601；控件另检查 `showLogoLighting`、Logo 效果列表及 edition，不能将未传参数写成显式开启 |
| Logo 下拉显式开启 | 除以上两行外的全部系统产品；当前 edition 和产品效果列表仍会影响实际显示 |
| 灯光页 Windows 动态灯光提示 | 694、695、696、709、710、711、736、737；状态为启用且非 BLE 时显示遮罩和设置入口 |
| 灯光节能开关 | 624 挂载旧式 `supportPowerSavingEffect=true`；709、710、711 挂载 `supportPowerSavingMode=true`；736、737 显式传 false |
| 启动动画卡片 | 709、710、711、736、737；还须中间件发布 `isFirmwareSupported` |
| VC 灯卡片 | 由 `DeviceInfo.isVcLightingSupported` 控制。711 为 true；737 为 false，不能因默认配置仍有 `vcLighting` 字段就给 737 显示此卡片 |

## 按钮、Fn 和待保存映射

页面挂载时检查 Chroma 安装状态、读取“不再显示 Windows 登录提醒”的本地偏好、注册外层 Save 回调，等待系统键盘布局，再加载按键和当前映射。版式／edition 决定产品图、SVG 与有效输入，匹配使用 `inputID`、`inputType` 和 `isHyperShift`。

系统键盘的 `func`／`multi` 表示 Fn 行主功能；Standard／Hypershift 是当前查看的映射层。切换 Fn 主功能会重新装载按键，把函数键映射投影到对应层，清除当前选择并关闭映射面板。切换 Hypershift 前若有草稿则调用 Save 提示；接受后重新投影当前配置的映射。F1…F12 和 Windows counter key 在特定主功能／显示层组合中拒绝编辑；Hypershift 下的方向键及 R、P、T、B 等系统保留键也拒绝编辑。`disableHypershiftMapping` 和按钮自身的 `isEnabled` 还有独立门控。

选键面板保存普通映射、组合键、鼠标／灵敏度、宏、多媒体、系统快捷键、文字、配置切换、跨设备、灯光、禁用和 Hypershift 等产品允许的组。共享编辑器内其他分支是否可用仍由当前产品能力控制。Save 调用当前功能编辑器生成 mapping，按输入和层合并；改掉 Hypershift 键时还处理它关联的额外映射。Don't Save 不运行保存合并，而执行对应的后续选择／跳转回调。关闭提醒、Save 和 Don't Save 保留各自的参数与下一动作，不能合并为无条件丢弃。

外层保存提示之外，实际映射编辑器有自己的草稿、关闭过滤和滚动定位监听。当前 737 外层还会在 DOM 按键重建后寻找 `inputID`／`inputID_mapped` 锚点，最多按 120 ms 递增重试 10 次；卸载清除该 timeout 和 storage listener。找不到旧锚点不会建立另一个输入身份。

## 保存、主机映射与 Fn 写入

中间件 `qc` 保存当前配置档的 mappings，调用 `Tc` 生成主机 appEngine、处理宏投影与 turbo 数据，再发布本地完成和后续状态。布局过滤区分日文等键位；turbo 存储冲突最多重新处理三次。随后是否排入板载任务由产品、调用标志和实际能力决定；有共享 OBM runner 不能证明每台系统键盘都写板载映射。

Fn 主功能 maker `_d` 在 `useFunctionKeySwap` 生效时调用 `setFunctionKeySwap`；`functionLighting.functionKey` 生效时调用 `setFnKeyState`，`func` 编码 0、`multi` 编码 1。`persistentInHardware.functionKeyState=true` 使用 profile 1，否则 profile 0。它再更新运行态和配置档的 `functionKeyPrimary`，重建默认函数键映射和 appEngine，写本地存储，发布 `MW_SET_KEY_PROPERTIES_TO_UI`，最后通知主机映射存储消费者。此完整函数没有统一的写后 Fn GET。

初始化有独立 `getFnKeyState` helper，仅当返回数字 `alternateState` 时解释为 func／multi；异常只记录，不返回确认值。系统恢复事件另恢复当前供电的性能设置，并按当前配置写 `setFnKeyState(0, state)`；它的 profile 参数与普通设置路径并不完全相同。

## Gaming Mode、Snap Tap 和触控板

Gaming Mode 保留 Off／On／In Game Only 语义，Windows 键禁用状态随模式切换；Alt+Tab、Alt+F4、部分产品的 Alt+Space 和 Fn Sleep 有独立选项。Windows checkbox 在组件中固定禁用；这不等于整个游戏模式无法修改。中间件区分旧版每配置状态和全局 `gameModeEnabled`，保存 appEngine 的 gamemode，再排队更新真实指示灯和适用的硬件设置。In Game Only 查询 `gms-running-games`；原件在该查询失败时返回关闭状态。系统产品的宿主映射消费者与硬件 GameMode LED 是两条不同链。

Snap Tap 最多保存四对键，初始编辑经历 Ready → Key1 → Key2。第二个键通过检查后提交完整键对；重复／空键有各自提示，新增过程中不能再次新增。录入同时处理浏览器 keyup 和宿主 `inputredirect`，暂停常规映射并注册禁止浏览器输入的宿主动作。失焦、点击外部、删除当前编辑项、beforeunload 和组件卸载各有处理：过滤不完整键对或恢复原列表，回到 Ready，移除输入 listener、解除录入状态并重新启用映射。完成提示使用 3 秒 timeout。

Snap Tap maker 保存 `snapTap` 并重建 appEngine。主机转换器把每对键的 id 写到相应 make／break 输入的 `snaptapId`，保留 func／multi 默认映射差异并重算 hash。共享硬件 runner 只有在 OBM `snapTap`／`killSwitch` 能力成立时才进入真实 GET／SET；普通系统挂载不能自动证明这些硬件分支成立。

触控板开关调用 OS `getTouchPadEnableStatus`，需要切换时先调用 `toggleTouchPadEnableStatus`，再 GET 实际结果，最后广播 UI。`functionLighting.touchpadStatus` 生效时另写设备指示灯；硬件 Fn+T 来源还发本地通知。Windows 触控板设置和鼠标属性入口调用各自本机系统接口。

## 亮度、Logo 和自动关灯

737 的实际亮度控件为 0…100、步长 1；开关与数值分开。接电／电池可分别保存，也可选择统一 `twoMode`。数值提高到非零且原开关关闭时会打开；电池或 twoMode 且亮度至少 80 时显示提示。`DeviceInfo.backlightStep` 用于键盘快捷键及从零重新启用等路径，不是此滑块的步长。

自动关灯包含显示器关闭、空闲超时和适用的低电设置。空闲 1…60 分钟、步长 1；低电 10…50%、步长 1，仅在电池／twoMode 显示。亮度禁用状态决定这些控件是否可编辑。Logo 下拉还要求效果列表非空且 edition 为 0。

亮度 maker 更新所选供电分支与版本、保存、发布本地完成，再按当前真实供电决定是否下发。编辑非活动供电分支只保存。真实 runner 写 BacklightLED；具备 VC 灯的产品另写 region 38。Logo runner 按硬件持久化条件选择 profile，先 `setLedEffect`，再 `setLedState`。这些 setter 的完成和页面配置保存分别结算，没有额外的统一 GET。

`Hc` 保存 switchOffLighting 后分别注册／停止显示电源、空闲和低电观察；关掉自动关灯条件时，可能排入恢复亮度任务并发布 `isLightingOff=false`。显示关闭／空闲造成的临时灯灭与用户关闭亮度开关不共用同一个配置字段。供电切换后使用相应分支并重设观察。

## Quick／Advanced 到真实灯光帧

Quick／Advanced 切换会处理 Chroma 全局启用、选中的本地效果和本机模块安装状态。Chroma Connect 占用、BLE 硬件效果和特定 edition 具有不同路径。只有当前系统挂载传入节能参数时才显示对应节能项。

产品 maker `Cu` 保存 quickEffects、版本和本地 Chroma 用户的启停状态，发布 UI 与设备 window-storage；其完整函数并不直接排入写灯帧任务。真正的软件灯效消费者在全局灯光引擎：

1. `onStorageChange` 按 `synapse_` 设备存储键进入 `onStorageChange_device`，匹配当前在线设备、container、设备 metadata 中的 activeProfileGuid；缺 metadata 时原件使用存储的 activeProfile。多 region／端口另有各自消费者。
2. `applyRegionQuickEffect` 比较当前效果／参数，按产品电池节能条件选择效果，转换 UI 效果，再进入软件或硬件效果路径。可同步板载效果的产品先写 profile 1；“退出时同步”分支只更新退出恢复参数及主机默认设备清单。
3. `applyCustomQuickEffect` 为软件路径设置 driver mode 3、CustomFrame 效果 8，创建引擎、加入真实设备、创建效果、建立灯位映射并按状态启用。Advanced／Chroma Connect／锁屏和 WDL 控制会改变是否启用此 quick 引擎；不是页面保存后无条件抢占其他灯光来源。
4. native 灯效帧事件进入 `processEvent`，按 device handle、link sequence、LED 重排和过滤条件选择设备，在 lighting lock 内调用 `setChromaFrameLightingEngine` 或逐行 `setChromaFrame`。前者按每行三通道 RGB 组织协议包，使用批量 HID control transfer 或逐行批处理发送。

缺少引擎、设备、灯位或 effect 会在相应边界停止。逐行帧 writer 对部分异常只记录并继续，最后构造 SUCCESS；这只是官方 wrapper 的返回语义，不能用它证明每行已传输成功或真实读回一致。厂商 native 引擎的计算、DLL 实现和物理灯光效果仍是独立证据层。

## VC、启动动画、退出与恢复

VC 设置按当前供电／twoMode 选择分支，写 `setVcLedState` 后写 `setVcLedStateInSleep`。电池下睡眠编码区分关闭、持续亮和定时亮，接电使用单独的编码；保存与 UI 更新在该 feature runner 中完成。硬件 VC 快捷键使用另一个 toggle owner。

启动动画先按真实 firmware version 与 `startupAnimationControlFWVersion` 的门槛判定是否支持，然后调用 `getStartupAnimationControl`；只有观测到 `disableAnimation` 才反向发布启用值。设置传 `setStartupAnimationControl(0, enabled ? 0 : 1)`，随后发布 UI；该 owner 没有重新 GET，也没有把返回对象中的所有状态逐项解释为设备确认。

全局灯光退出链先禁止设备输出、停止主机帧传递、terminate 引擎和释放引擎对象，再恢复产品规定的硬件／退出效果、清除设备和关联表，通知主机可关闭并释放相关原生模块。页面卸载、录入结束、产品中间件停止和全局灯光进程退出拥有不同的清理链，不能用其中一个替代其余三个。

## 原件与复核位置

路径前缀均为 `.analysis/inputs/frontend/`；读取核对对应 `acquisition.json` 的原始字节长度及 SHA-256。区间为 UTF-8 半开字节区间。41 产品的页面根和条件参数以各自 [产品页](README.md) 的导航原件为准。

- 737 UI `system-737-current/raw/apps.razer.com/synapse/products/737/ui/static/js/1709.51f8d716.chunk.js`：系统键盘 `[757536,766647)`；Customize 外层 `[768002,794128)`；游戏模式 `[690097,694626)`；Fn 属性 `[747121,748701)`；触控板 `[745525,746989)`；Snap Tap 键对 `[750639,756102)`、开关 `[756558,757519)`。
- 同产品 `9742.82cb0b5c.chunk.js`：自动关灯 `[6909,9868)`；Quick／Advanced 控制 class `[86319,92745)`；亮度／Logo `[95826,97717)`；动态灯光提示 `[97925,98648)`；启动动画 `[98653,98984)`；Lighting 根 `[98985,99366)`。
- 737 中间件 `system-mw-737-current/raw/apps.razer.com/synapse/products/737/mw/6737.85397ce157d4cca228ee.js`：映射生成 `[727416,730792)`、保存 `[760812,771193)`；Fn maker `[883735,886262)`、GET `[1265172,1265457)`、resume `[2356021,2357056)`；游戏 maker `[872543,876558)`、runner `[1202908,1206354)`；Snap Tap maker `[758218,758821)`、主机转换模块 `[21808,24444)`；触控板 `[909462,910493)`。
- 同中间件：亮度 maker `[734664,738167)`、统一供电模式 `[738167,740525)`、开关 `[742116,745201)`、自动关灯 `[750935,753756)`；Quick maker `[792562,795416)`；亮度 runner `[1121341,1123244)`、Logo runner `[1124278,1125608)`；VC 和 feature 分派模块 `[2019109,2032396)`；启动动画 `[36447,37758)`、版本及读取入口 `[1878210,1878863)`。
- 全局 `synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js`：storage 分派 `[908613,909145)`、当前配置消费者 `[844173,849899)`；区域效果 `[823012,825228)`、硬件同步 `[825247,826331)`；软件效果 `[863131,868219)`；帧消费者 `[435194,437352)`、逐行／批写 `[550280,551544)`；退出 `[874724,876284)`。

本页记录当前官方源码的静态行为，未执行厂商 JavaScript、主程序、灯光 DLL 或真实设备。Rust 实现、UI 后端接线、模拟验证和真实运行验收不能由这份语义文档替代。
