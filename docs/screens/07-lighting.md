# 灯光：653 键盘与 777 耳机

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 实际组件树

`[JS]` [653 main](../../.ref/devices/653/static/js/main.7b71cce5.js)：`Fh → yh`。

```text
条件 fh：Windows Dynamic Lighting 提示
左列：rP → _P 亮度；AP → TP 关灯条件
右列：Gh → Hh 灯效
```

[777 main](../../.ref/devices/777/static/js/main.eb70ce38.js)：`YG → WG`。

```text
左列：QM → qM 亮度；VG Stream Reactive Lighting
右列：fG → gG 灯效
```

777 此树没有 TP 关灯条件卡。182 无 Lighting 根页面。

## 2. 亮度、关闭条件和系统接管

| 项目 | 实际行为 |
|---|---|
| 亮度 | 0–100，step 1，带启用状态；属于当前 profile 的 brightness |
| nanoLeafEnabled | 影响控件可用状态，不能忽略 |
| adjustment mode | 调整模式运行时有阻止操作/提示分支 |
| 653 displayOff | 显示器关闭时熄灯选项 |
| 653 idleEnable | 闲置关灯启用项，slider 1–15 分钟 |
| 653 brightness 关闭 | 关灯条件卡相关编辑禁用 |
| hideLightingIdle | 控制是否显示 idle 分支 |
| 653 WDL | DeviceInfo.isWDLSupported 且动态灯光开启、非 BLE 时显示提示 |

DeviceInfo 中的 backlightStep=5 不自动等于亮度 slider 的 step；以实际组件参数为准。当前入口为 step 1。

## 3. 快速灯效列表与选择条件

`[CONFIG]` 653 模块 78193、777 模块 7816：

| 产品 / 列表 | 顺序 |
|---|---|
| 653 QUICK_EFFECTS（12） | Ambient、Audio Meter、Breathing、Fire、Reactive、Ripple、Spectrum、Starlight、Static、Tidal、Wave、Wheel |
| 653 HARDWARE_QUICK_EFFECTS（7） | Breathing、Reactive、Spectrum、Starlight、Static、Tidal、Wave |
| 777 QUICK_EFFECTS（4） | Audio Meter、Breathing、Spectrum、Static |
| 777 HARDWARE_QUICK_EFFECTS（8） | Audio Meter、Breathing、Reactive、Ripple、Spectrum、Starlight、Static、Wave |

`Hh/gG.checkSpecialEdition` 先按 `isBle && DeviceInfo.ble?.useHardwareEffect` 选择基础列表，再应用 specialEditionOverrides。653 静态配置没有 ble；777 的 ble.useHardwareEffect=false。不能简单把“653 + BLE”判为硬件效果，也不能把 777 的 8 项硬件表当默认选项。

灯效标识必须是产品枚举的真实 ID。Ambient、Fire、Tidal、Wheel 不能分别借用 Off/Breathing/Wave；名称正确但发错 ID 仍是功能错误。

重构时进一步核对 653 main 模块 13254：产品层 AoV 的数字为 Static=1、Breathing=2、Spectrum=3、Wave=4、Reactive=5、Ripple=6、Starlight=7、Fire=8、Ambient=11、AudioMeter=12、Wheel=13、Tidal=19。它们与 LightingEngine_EffectId 不是同一套编号。Rust 新 Effect 序列化产品数字，内部控件另用稳定名称标识。

本包普通参数编辑器 SP/VP/zP/EU/SU/DU/GU/yU 已核对：Breathing/Starlight/Tidal 有两色及随机开关；Reactive/Starlight duration 1–3；Wave 左/右 1/2，Wheel 顺/逆时针 1/2，Tidal 向外/内 1/0；Ambient 选择 full/left/top/right/bottom；Audio Meter colorBoost 为 .25–4、步长 .25。Fire/Spectrum 的普通分支没有额外参数编辑区。所有字段属于对应 effect 的缓存，切换效果不能共用一份参数覆盖其它效果。

## 4. 快速 / 高级模式与 Chroma

普通分支显示 Quick / Advanced 双模式；只有确实满足硬件效果条件时直接渲染 quick。共享的 SensaHD 等其它产品分支不能当作 653/777 固定能力。

Hh/gG 持有 isAdvanced、allChromaResourcesInstalled。安装完整性来自异步 checkIsFullyInstall，并与 `installedModules` 中的 `chroma-app` 安装标记共同判断；设备 is_chroma_device 不能替代此状态。

高级页的内容和入口需要安装状态。focusChromaApps 先检查目标窗口，存在则 activateWindowServiceClient；不存在时，资源不完整会 triggerInstall，然后打开 `/chroma-app/dashboard`。不是一颗永久 disabled 的“安装”按钮。

Chroma Visualizer / Connect 正在接管时，普通编辑区可能禁用，并显示正在使用的应用名称/图标和管理入口；BLE 键盘有例外条件。安装状态、接管状态、设备支持能力是三个不同字段。

颜色、颜色数量、速度、方向等由选中灯效的实际参数编辑器决定，不是所有灯效共享两个 RGB/速度字段。还应区分选项 preview 与持久化的 effect settings。

## 5. 777 Stream Reactive Lighting

VG 渲染 Stream Reactive Lighting 标题、外部按钮、bG 插图及说明。链接为 `https://www.razer.com/streamer-companion-app`。此卡介绍/引导 Streamer Companion App，不是灯效枚举中的一种 effect，也不是 653 的闲置熄灯卡。

## 6. 本页资源

| 用途 | 来源 |
|---|---|
| 653 布局与状态 | [main.5425442a.css](../../.ref/devices/653/static/css/main.5425442a.css) |
| 777 布局与状态 | [main.e4bab2aa.css](../../.ref/devices/777/static/css/main.e4bab2aa.css) |
| 777 Streamer 插图 | stream 1x / 2x / 3x AVIF，构建 hash 分别 b197a2c3 / c872cf03 / e6572c2c，完整路径见资源索引 |
| 快速灯效选项图标 | 产品 media 中的 effect SVG 及组件内嵌 SVG |
| 高级 / 同步 / Chroma 图标 | chroma studio、sync、运行应用图标相关资源 |
| 颜色、速度和方向 | color picker / direction / speed 图标，按编辑器实际选用 |
| 警告 / tooltip | WDL 提示和通用帮助图标；不是所有设备固定显示 |

[资源索引](../re/04-resource-index.md) 列出具体文件和变体。灯效动画是引擎/设备行为，不能把静态预览图片当作实时输出实现。协议命名空间见 [灯光动作表](../re/02-lighting-actions.md)。

## 7. 重构前基线与验收

2026-10-01 当前界面已按 `Hh/gG → VU → SP/VP/zP/yU` 与实际 CSS 再次修正，未参考旧截图：

- Quick/Advanced 使用原 36px 外框和 26px 选项；Quick 内容在切换条下方 20px，包含说明、150px 效果下拉和同步状态。效果参数不再错误地随亮度开关一起消失；原亮度开关只控制亮度与 653 关灯条件的可编辑状态，Chroma/nanoLeaf 接管是另一组运行时条件。
- 颜色标签位于色块上方，触发器为 53×27。Breathing/Starlight/Tidal 的两种颜色同时保留；随机颜色只禁用这两个控件，不清空已选颜色，也不增加原版没有的“第二种颜色”复选框。颜色与随机项按原组件横向排列；持续时间保留短/中/长标记。
- 颜色面板以原 `mM` 的 40 个色值和“无颜色”资源为基础，宽 250；Reactive 不提供无颜色。GPUI Base ColorPicker/ColorSwatch 和 Popover 负责焦点、键盘与关闭行为，原有两个 ColorPickerState 及领域订阅继续拥有颜色值。自定义颜色当前通过原 state 的 HEX 输入提交；原版 16 个自定义色槽、编辑/删除菜单及独立拾色面板尚未完整移植。
- Wave/Wheel/Tidal 改回 42px 方向胶囊和 20px 原 SVG，保留左右、顺逆时针、内外不同 ID；Ambient 改为五个 24×16 的屏幕采样区块，不再用五个宽文字按钮。
- 高级区域恢复 `install_chroma` 的 520×180 插图和说明，移除当前原路由没有渲染的 Customize/Chroma 区域预览。实际安装、应用接管与同步状态尚未接入宿主；本地入口只提供真实外链，不声称已安装或同步成功。
- 777 Streamer 保留 520×180 图片、y=133 的外链按钮及原说明；653 闲置关灯保持 1–15 分钟端点。

Audio Meter 的 colorBoost 已改为原 `DU` 的60×27数值步进器：范围 .25–4、步长 .25，保留部分输入草稿，在 Enter/失焦时向上对齐下一档（如0.26→0.5），上下键和右侧原SVG箭头可步进。输入实体独立保留，切效果/Profile时从对应领域值同步。新增输入及量化测试仅编译，未执行。原按住箭头300ms连发、聚焦后滚轮步进仍未接入；653本包 `DU` 读取的 `ble.quickEffectUseHardware` 未开启，当前接数值分支，不把通用颜色分支当作本设备当前行为。

`[RUST 基线]` [lighting.rs](../../src/features/lighting.rs) 的主要差异：

- 653 显示了 12 个名字，但 Ambient→Off、Fire→Breathing、Tidal/Wheel→Wave，语义不等价。
- 777 使用 10 项通用列表，实际默认只有 4 项。
- hardware_effect 使用 `653 && use_ble`，资源 ready 使用 is_chroma_device，条件不符。
- 亮度读写 global_brightness；普通效果只取 zones.first()，缺 profile 内完整数据。
- 缺 653 关灯条件/WDL、777 Streamer、高级模式与真实安装状态链。
- 当前 UI setter 没有接到灯光 DLL；命令行 FFI 演示不能证明界面已接通。

`[建议]` 先精确表达产品 effect ID/参数，再用 GPUI Kit Select、Switch、Slider、ColorPicker 和异步安装状态呈现。验收覆盖列表顺序/ID、连接条件、profile 隔离、亮度禁用、每种效果参数、Chroma 未安装/部分安装/接管、WDL、Streamer 外链、失败与回读。
