# 自定义：182 鼠标与 653 键盘

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 入口与编辑状态

`[JS]` [182 main](../../.ref/devices/182/static/js/main.db20a7c4.js)：`em → $P → JP → QP`；[653 main](../../.ref/devices/653/static/js/main.7b71cce5.js)：`nP → tP → eP → $m`。两个 wrapper 均传 `showMouseUse:false`，653 另传 `hasKeyboardControls:true`。共享组件存在不代表当前入口使用它。

页面先展示可配置输入，再打开对应映射。关键状态：activeButton、buttonList、mappingList、isHyperShiftOn、isMappingOpen、hasUnsavedChanges、当前 anchor、selectedProfile、映射草稿。普通层/Hypershift 层按 inputID 区分，不能只换按钮标题。

777 根导航没有 Customize。

`[RUST 当前实现]` [customize_drawer.rs](../../src/features/customize_drawer.rs) 接入原 `ButtonPanelComponent`（182 模块 5035 / 653 模块 95035）的 230px 输入列表：67px 顶栏、所有/自定义筛选、最小 50px 行高，列表和设备正文各自滚动。两种产品的 drawer-open 正文均为 1000px（外加两侧 20px 内边距），小窗口保留横向滚动；653 最终覆盖色为 `#2b2b2b`，182 为 `#2d2d2d`。653 使用实际 layout 的完整 groupList，保留无图形的拨轮子输入及原版隐藏/禁用条件。

输入列表和设备图共用映射继续流程，切键、关闭、切层均保护草稿；点击已经选中的输入或层不弹出无效确认。列表发起的编辑器锚点为 `left=210px`，图上发起的编辑器按原 left/right 规则定位并限制在可视范围内。182 切层后关闭抽屉，653 保留抽屉并更新筛选结果；两者对应原 `KP.updateHypeShift` / `km.updateHypeShift` 的不同处理。

每个当前布局输入保留独立焦点，Tab 进入屏幕外的行时按筛选后的行索引滚动。关闭抽屉返回开关焦点；“自定义”筛选下恢复默认映射并移除当前行时，编辑继续与焦点回退仍由设备工作区处理。相关回归源码包含布局 16、窄窗和筛选行消失场景，本轮未运行。

## 2. 182 鼠标设备区

实际视图为 `KP`：

```text
config-wrapper dot-bg（height 340、min-width 770、max-width 1220）
└─ config-block（770 × 340）
   ├─ Tp / config-ctx：标注连线画布
   ├─ 左侧 buttonList 索引 [0, 2, 6, 5, 7]
   ├─ zp：按 productId / edition / layout 选择图片
   └─ 右侧 buttonList 索引 [1, 3, 4]
config-row：抽屉开关、Standard / Hypershift、tooltip
条件多设备配对入口 / 替代内容
```

索引仅描述该数组的视觉排布；Rust ID 应取 inputID / buttonKey。左列索引 7 实际传 `side:"right"`，不能按所在列自行修改。

`getCycleImage` 是额外叠层，正面 `zp` 始终保留；不能用底部图替换正面。`.config-profile-img` 为 `right:calc(50% - 187px);top:-77px`，在 770px config-block 内 x=272。正面 x=235，二者均显示为 300×340。按钮透明底、高 30、行距 50，hover #383838、active #111；连线圆点半径 2.5、选中圈半径 5.5。详细复核与当前接入见[样式审计](../re/06-style-source-audit.md)。

hover 影响连线/高亮，clickBtn 在有草稿时先调用 displaySaveAlert。索引 7 关联底部 DPI 按钮图；`displayCycleButton` 控制额外 cycle DPI 图。Standard/Hypershift 切换经过未保存保护，再清理活动按键、映射面板和抽屉；tooltip 宽度基线 362。

配对卡使用 `DeviceInfo.isMultiPairingSupported && state.showMultiPairingFeature`，满足走 `WP`，否则走 `sp`。不是始终显示。

## 3. 653 键盘设备区和卡片

实际视图 `zm → km`，`TM` 负责键盘交互图，`MM` 等负责图片叠层。edition=130 使用 white 资源，layout 缺省为 1。

653 CSS 覆盖共享尺寸：config-wrapper height=385；config-block height=387、width=100%；box-img width=830，白色版本 830.8；Armory 相关显式尺寸 830.9×386.9。不能沿用鼠标 770×340。

```text
groupList 按键命中区 + 产品图 + 条件腕托 + 拨轮 hover
抽屉 / Standard-Hypershift（macro displayMode 有不同控制）
body-widgets
├─ 左列
│  ├─ JL → QL：游戏模式
│  ├─ wm：Snap Tap
│  ├─ Um → Pm：回报率
│  ├─ hp → Up：Windows 键盘属性
│  └─ rL → _L：条件配对卡
└─ 右列
   └─ Am → Tm：Command Dial
```

`_L` 在没有 `DeviceInfo.dongleId` 时直接返回；本地 653 静态配置没有该字段，不能描述为默认可见配对卡。

### 游戏模式

`QL` 有启用开关、enableInGame 和禁用快捷键项。Windows 键项固定受限制；Alt+Tab、Alt+F4 按配置处理，`notSupportAltF4` 影响后者。Menu、Copilot、Alt+Space、Fn+Sleep 来自可选能力分支，不能无条件显示。数据属于游戏模式配置，不是普通按键映射。

### Snap Tap

`wm` 读取 `snapTapReducer.keyList/isEnabled`，`Ym` 提供键对编辑。adjustmentModeRunning 会阻止操作并显示提示；存在功能键时 tooltip 有专门分支。启用、键对与键盘高亮必须对应稳定按键 ID。

已进一步核对 653 的 `Ym`：`P.length >= 4` 限制最多四组；禁用 `KEY_APPLICATION`、`KEY_LEFT_GUI`、`KEY_FN`、`DKM_F6`、`DKM_D2`，同组及跨组均不允许重复按键。该数字来自当前产品实际编辑器。

`[RUST 当前实现]` [keyboard_controls.rs](../../src/features/keyboard_controls.rs) 支持最多四组本地键对、顺序录制、已有按键替换和删除。新增键对只有两键均有效时才写入 Profile，Escape、取消或焦点离开撤销未完成录制。旧 `snap_keys` 兼容首组，新 `snap_pairs` 保存完整列表；输入必须存在于当前布局且符合禁键和去重规则。

GPUI 在 Windows 下的普通按键事件不能保留所有左右修饰键和主/数字键盘 Enter 的物理区别。录制期间的“选择布局按键…”或 Alt+↓ 打开可搜索的当前布局选择器，补足这些输入并继续当前录制步骤。此选择器是本地可靠输入途径；设备调整模式和硬件 Snap Tap 提交尚未接入。

### 回报率、系统属性、板载

候选为 **125、250、500、1000、2000、4000、8000 Hz**，位于 Customize 左列。与 182 的 dongle/固件回报率不能共用一个固定列表。Windows 键盘属性是宿主系统操作。

配置含 `OBMSlots:4`；这是能力值，不表示显示一段槽位文字就实现了 profile 写入/回读。

`[RUST 当前实现]` 653 非 BLE 的配置栏提供原 OBM 图标和板载内存说明，四槽均标记“未读取”；写入按钮禁用。182/777 不显示此入口。不填充本地 Profile 为硬件槽位，也不显示模拟同步结果。

### Command Dial 与滚动功能归属

`Tm` 支持模式启用、选择/高亮、排序、重置，模式具有稳定 uid。653 `km → Am/Tm` 仅传 `clickBtn`，没有传 `hasSynapseCustom`；当前产品的自定义输入为 **ScrollRight / ScrollLeft 两个方向**，不能采用共享组件另一个三方向分支。当前选用模式与高亮编辑模式分开。

`[RUST 当前实现]` 本地 Command Dial 支持八个预设与最多 100 个自定义模式；自定义模式有独立 uid、最多 40 个 UTF-16 单元的名称、颜色和双方向映射。新增颜色为原默认 `#FEED03`，两方向初始为 Disable。支持拖放及按钮排序、启用/选用/高亮、删除确认和整体重置确认；至少保留一个启用的预设，停用或删除当前模式时按列表循环选下一个启用模式。重置恢复八预设并保留其既有身份。

自定义模式插图已按原 `Em`/`.custom-mode .digital-dial` 修正为 `digital-dial.1e701318.avif`，64×64 并使用 3px 模式色圆边。标题说明入口支持点击和键盘操作，展示原 `COMMAND_DIAL_USAGE_1…4`（按压切下一个、Shift+按压切上一个、颜色指示）及原指示灯图标；这段说明不声明已向硬件写入配置。

点击高亮自定义模式的方向进入既有映射编辑器，按模式 uid 独立保存到 `mappings`，不改普通层或 Hypershift 绑定。实际 SourceInput 的 `isEnabled:true` 与 `disabled:true` 分别控制可编辑能力和普通输入图/列表隐藏；拨轮入口允许的分类严格为 Keyboard、Mouse、Device Brightness、Multimedia、Windows Shortcut、Disable，没有 Default/Hypershift。切方向、切模式及离开编辑器继续保留草稿确认；设备命令提交仍未接入。

182 滚轮上/下属于按键映射的 scrolling/mouse 功能；653 拨轮和多媒体滚轮属于此设备区及 Command Dial。三个已审计产品均无 Scrolling 根页面，因此已删除旧独立页面文档。不能因 `TAB_SCROLLING`、`ROLLER_ASSIGNMENTS` 或通用滚轮资源存在，就添加自由滚动、智能切换、阻力控制等无实际路由依据的卡片。

## 4. 保存、关闭和映射限制

182 `QP` 的调用契约：

| 动作 | 实际处理 |
|---|---|
| 请求保存提示 | displaySaveAlert(nextAction, tmpRef, dontSaveAction) 保存回调和 anchor |
| 保存 | await saveMapRef(true)，按需重载映射，再 nextAction |
| 不保存 | await saveMapRef(false)，再 dontSaveAction |
| 关闭提示 | dismissSave(true) 传到 saveMapRef，并处理 sensitivity 配置状态 |
| 打开输入 | 检查 disableHypershiftMapping、isEnabled、限制列表后才选中 |
| 获取映射 | 按 inputID、普通/Hypershift 层或自定义 Command Dial mode 查找 |

关闭、丢弃、保存的回调链不一样。不能统一“关闭即保存”，也不能丢失继续切页/切键的回调。

共有映射处理包含 Default、Keyboard、Mouse、Scrolling、Sensitivity、Macro、Inter-device、Profile、Chroma、Hypershift、Launch、Multimedia、Windows shortcut、Text、Disable 等；实际可选项由设备、输入、层和模块安装状态裁剪，不能全部展开给每一个输入。

`[RUST 当前实现]` [mapping_editor.rs](../../src/features/mapping_editor.rs) 已替换单一功能下拉框。182 `QP.renderPopup → 5107` 与653 `renderPopup → 3241/2667 → 13241` 使用相同的类别编辑结构：292px外壳、36px标题、40px收起分类轨（悬停展开230px）、250px正文含20px内边距。分类轨和正文独立滚动，位置由Customize容器叠放，标题关闭与正文取消都进入未保存继续流程。

| 分类 | 本地控件与校验 |
|---|---|
| Keyboard | MapKeyboard（182 1192、模块6223）：录制实际按键；字母数字/功能/数字键盘/导航/修饰/符号组使用main模块6114稳定inputID；可选左右修饰键；Turbo数字框与滑杆，1–20次/秒 |
| Mouse | MapMouse（6920/9119）：点击、双击、按钮4/5及四向滚动；182 extendSupportMappings允许的重复滚动；仅输入允许时显示Turbo |
| Sensitivity | MapSensitivity（3400/6903）：离合器、升/降阶段、即时调整和循环阶段；离合器X/Y数字框与滑杆，100–30000、步长50；底部循环按钮排除离合器/即时，Hypershift排除即时调整（DPI_OnTheFly） |
| Multimedia / Windows | MapMedia（4157/7068）、MapWindows（4978/7569）的真实操作ID与命令列表；选项保存为映射，不即时启动或注入系统输入 |
| Text | MapText（470/1817）：96px多行文本区域、UTF-16字符计数；保存要求1–250字符 |
| Profile / Launch | Next/Previous/CycleUp/CycleDown/Specific 操作；Specific 排除当前 profile，单配置或失效目标禁存；程序/网站单选与本机文件浏览，网站补全 https 并验证 HTTP(S) 域名 |
| Device brightness | 653 MapBrightness（9282/37009）：提高/降低/开关三个单选项；182不显示 |
| Default / Hypershift / Disable | 明确功能说明；Hypershift层不再显示Hypershift赋值 |

182滚轮上/下只允许DEFAULT、KEYBOARD_FUNCTION、MOUSE_FUNCTION、MACRO、MULTIMEDIA、DISABLE；653按每个输入的functions裁剪。182标准层以最终绑定校验至少保留一个左键单击：先给其他按钮分配Click后才能重绑左键，且最后一个主点击不能移除。

普通映射持久化继续使用profile内独立的`bindings`/`hypershift_bindings`字符串Map，自定义拨轮映射保存在对应模式内。旧`default`、`disable`、`Hypershift`、鼠标ID和`keyboard:Ctrl+C`可以读回；打开不会迁移或标脏。新增结构用`local-mapping:v1:`+JSON编码，支持修饰键、Turbo、XY与多行Unicode；未知值可关闭保留，不能当作已支持操作执行。该格式是本地配置，不是原生设备服务协议。宏、跨设备和Chroma分类显示缺少实际服务/资源的原因并禁止保存；AI Launcher、动态键程/手柄/高级滚动等未满足产品与服务条件的分支不开放。

保存映射、放弃并继续、继续编辑三条continuation仍覆盖切键/层/页/profile及profile管理；非法草稿不能保存。新增mapping_tests覆盖键录制的继续流程、层隔离、旧值兼容、UTF-16文本、范围及主点击限制。

`[JS 条件分支]` Windows 登录提示只适用于 KEYBOARD/SYSTEM 类别的对应原保存分支，OK 隐藏提示并持久化隐藏偏好；自定义 Command Dial 的提前返回保存分支和 Global Shortcuts 不触发。当前只保存本地配置，因此不显示已影响登录的提示。Quick Remapping 冲突属于原游戏手柄重映射小组件，不能按普通绑定重复推导；实际小组件和冲突事件未接入。完整条件见[映射警告审计](../re/11-mapping-warnings.md)。Chroma 映射仍需真实安装与资源状态；本地录制、符号到物理键转换、修饰键互斥、Turbo、异步文件选择及未知映射保留见[映射编辑器审计](../re/09-mapping-editor.md)。

## 5. 实际资源

| 用途 | 原始名称 → 本地文件 |
|---|---|
| 182 标准正面 1x | ./182_0/img_prods/prd-1x.png → [prd-1x.cb2eedcf.avif](../../.ref/devices/182/static/media/prd-1x.cb2eedcf.avif) |
| 182 标准正面 3x | ./182_0/img_prods/prd-3x.png → [prd-3x.674b18f0.avif](../../.ref/devices/182/static/media/prd-3x.674b18f0.avif) |
| 182 底部按钮 | [1x](../../.ref/devices/182/static/media/litat1-profile-button-1x.ba4ac8e1.avif)、[3x](../../.ref/devices/182/static/media/litat1-profile-button-3x.99ffa7e1.avif) |
| 653 标准 layout 1 | ./653_0/img_prods/1-1x.png → [1x](../../.ref/devices/653/static/media/1-1x.ab26aeb7.avif)、[3x](../../.ref/devices/653/static/media/1-3x.8d525695.avif) |
| 653 腕托 | [wrist-without-gap.efc0505f.avif](../../.ref/devices/653/static/media/wrist-without-gap.efc0505f.avif) |
| 653 Command Dial 自定义模式 | [digital-dial.1e701318.avif](../../.ref/devices/653/static/media/digital-dial.1e701318.avif)；说明入口使用 [dial-3x-1-2x.a7526df5.avif](../../.ref/devices/653/static/media/dial-3x-1-2x.a7526df5.avif) |
| 653 Customize 交互图 | main 模块 21368 groupList；IM 根据 buttonList 的路径/矩形/圆生成命中元素，730×340 |
| 653 Chroma 区域图 | [965.8f16fe94.chunk.js](../../.ref/devices/653/static/js/965.8f16fe94.chunk.js)，模块 10965，SVG_PRODUCT / DEVICECONFIG；不是 Customize 的坐标系 |
| 点阵、映射线、hover | CSS / canvas / 内嵌 SVG，不是产品截图 |

653 支持的布局编号为 1–12、15–18；layout 1 Chroma SVG viewBox 为 0 0 960 360；Customize 则使用 IM 的 730×340 与 groupList 输入路径。两个坐标系不可互换，位图与命中区应使用同一 edition/layout。全变体映射见 [资源索引](../re/04-resource-index.md)。

`[RUST 当前实现]` 已接入全部 16 种已知布局的产品图、原始路径命中与输入列表。缺省 layout 0 使用布局 1；未知布局仍只显示回退预览，不启用错误的输入区域。鼠标图上的映射摘要通过编辑器解码显示功能名称，不显示本地序列化字符串；主点击及 Hypershift 限制同时作用于图上输入和抽屉。

## 6. 重构前基线与验收

`[RUST 基线]` [customize.rs](../../src/features/customize.rs) 仅渲染 productId=182，653 普通 Customize 会是空态。[keyboard.rs](../../src/features/keyboard.rs) 的独立 Tab::Keyboard 不在正常产品导航中，keyboard_svg_card 也只是文字列表。

重构前 182 的 mouse_visual 使用圆角容器、通用 Mouse 图标和文字；connection_canvas 是几条固定直线，当时尚未使用原版产品图和逐键连线。`assets/mouse-182-dashboard3x.png` 当时未被 src 引用；它与 Customize 的 prd 图用途不同。该段仅保留重构前差异，当前产品图、输入抽屉和分类编辑器状态以上文及[重构状态](../re/03-implementation-gap.md)为准。

当前设备图/命中几何与设备工作区的映射草稿分开，编辑和退出使用 GPUI Kit 输入、选择器、弹层和焦点机制。[键盘控件回归源码](../../src/features/keyboard_controls_tests.rs) 覆盖键对迁移、四组限制、禁键/去重、录制取消与布局选择、拨轮稳定身份/双方向保存隔离等路径。本轮只做 `cargo check`，不执行测试、应用或 DLL。后续实际验收仍包括逐键命中/hover、底部 DPI、普通/Hypershift 与自定义模式隔离、三种退出路径、Profile 切换、不同布局、Snap Tap 录制、Command Dial 管理及后端失败。

`Em` 的原始条件也予以保留：当只剩一个启用的默认模式时，已启用的自定义模式开关同样锁定；自定义模式若精确命名为 `Switch Applications`，也受 Alt+Tab 限制。这些行为依据实际条件表达式，不能只看组件 props 推断。
