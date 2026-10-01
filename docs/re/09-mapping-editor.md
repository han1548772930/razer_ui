# 映射编辑器：源码对照与本地实现

本轮修改 `src/features/mapping_editor.rs`、`mapping_keys.rs`、`mapping_tests.rs`。参考依据为 `.ref` 中 JS/CSS 和提取出的配置数据；未使用旧截图推断功能。

## 原始证据

| 范围 | 原始入口 / 数据 | 本轮修正 |
|---|---|---|
| 编辑器壳 | 182 `5107.48fb3f50.chunk.js`；653 `3241/2667` | 保留 292 外宽、36 标题、40 收起分类轨 / 230 展开轨、250 正文含 20 内边距；分类和正文独立滚动。颜色通过产品主题角色取得，避免散落 raw 色值。 |
| 键盘类别 | 182 `MapKeyboard.090e6ef5.chunk.js`，模块 6223；main 模块 6114 | 补齐 Symbols 中无独立 inputID 的符号选项；按 `getMappingData` 的规则保存真实物理键 ID 加 Shift，例如 `+` → `KEY_EQUAL` + `KEY_LEFT_SHIFT`。保留国际布局输入 ID。 |
| 修饰键 | 6223 `loadModifierButtons / selectModifier / getModifiers / getMappingData` | 分别保留显式辅助键与符号必需 Shift，编码时按键族合并；保留用户选择的右 Shift，切组不丢辅助键。Shift、Ctrl、Alt 左右互斥，Win 仅左侧。录制与下拉组仍分开。 |
| Turbo | `MapMouse.d4aa1eba.chunk.js` 模块 9119；6223 `_checkDisableTurbo`；main 9937 的 `LrF` | 依据真实 buttonKey 和 `disableTurboInAssignment` 裁剪；1–20 次/秒，默认 7；182 重复滚动仅对允许 Turbo 的输入开放。 |
| 灵敏度 | `MapSensitivity.ed0c234c.chunk.js` 模块 6903 / 1628；main 1057 DeviceInfo | Clutch 默认 800；联动时 X 修改同步 Y，关闭独立 XY 时令 Y=X；100–30000、步长 50，数字失焦/Enter 回退或归一化。阶段预览与本地五槽状态共用 enabled 和 independent，并提供进入 Performance 的未保存继续流程。 |
| Profile | `MapSwitchProfile.061df243.chunk.js` 模块 8361；main 9267 `we` | 区分 NextProfile、PreviousProfile、CycleUp、CycleDown、Specific；Specific 下拉排除当前 profile。单 profile 禁止保存，失效目标不能保存。 |
| Launch | `MapLaunch.3a752e17.chunk.js` 模块 8693；main 1867 的 `Gz` | 程序 / 网站使用单选；增加本机文件浏览。网站补全 https，保留 http(s)，空地址 / 错误域名禁存。仅写本地映射，不启动程序。 |
| Brightness | 653 `MapBrightness.5fb51317.chunk.js` 模块 37009；main 29267 | 三种真实操作 ID 与原语言键：BrightnessUp / BrightnessDown / BrightnessToggle。 |
| 每输入能力 | 653 main 各布局 `groupList`，由 `keyboard_source_for_layout` 提供 | 通过抽屉共享的完整 SourceInput 缓存读取 functionList / isEnabled / disabled / disableHypershiftMapping，包含没有命中几何的拨轮子输入；未知布局不开放映射。 |

### Hypershift 灵敏度证据更正

6903 的 `buildDataSet` 在 Hypershift 使用 `filter(e => e.id !== g.GGY)`。
main 9937 明确导出 `GGY:()=>m`，且 `m="DPI_OnTheFly"`。
因此 Hypershift 排除的是 **即时灵敏度**，不是 Clutch。底部 `CycleUpSensitivityStages` 则同时排除 Clutch 和即时灵敏度。本轮实现与测试按此修正。

653 中禁止 Hypershift 映射的键以各布局 `disableHypershiftMapping` 为准，包含 F9、F10、F11、F12、Pause、Application；不再从键名自行猜测。

## 编辑与保留语义

- 录制使用捕获阶段接收 Space、Enter、Tab 等，避免按钮先处理按键。Escape 先退出录制，非录制时关闭编辑器并进入原有未保存继续流程；Escape 键本身仍可在 Navigation 中选择。
- 程序文件选择异步返回时核对 profile、层及 input 身份，避免把结果写入已切换的编辑对象。
- 保存、取消、切页和抽屉关闭统一处理焦点与录制状态；未保存确认选择“继续编辑”不覆盖原返回目标。抽屉在“自定义”筛选下保存 Default、原行消失时，焦点返回筛选框。抽屉内 Tab 导航会将当前行滚入视口，关闭抽屉会将列表入口滚入正文视口。
- 键盘下拉保留实际选项身份，调整 Turbo 不会将物理 `=` 加显式 Shift 擅自显示为 `+`；再次确认当前录制分组也不清空已经录制的键。持久化保存最终组合键，重新打开时进入录制分组，不保存上次选择符号或辅助键的操作历史。
- `local-mapping:v1:` 仍是本地配置格式，不是 Synapse 原生协议。可保存 Keyboard、Mouse、Sensitivity、Multimedia、Windows、Text、Profile、Launch、Brightness、Default、Hypershift、Disable。
- 未知版本、未知字段及未知旧值原样保留。打开它们不会阻止保存其他配置；编辑器仍明确表示该映射不可编辑。切换功能后又还原相同语义时恢复原字符串，不制造旧格式迁移或脏状态。
- 标准鼠标层仍要求至少保留一个左键单击；普通层 / Hypershift 层继续使用独立 bindings。
- 宏、跨设备和 Chroma 没有可供本地实现操作的实际服务 / 资源，保留不可保存状态。原生键注入、硬件写入及登录阶段映射警告不是本轮伪装实现的功能。

## 验证状态

前序修改已对编辑器和测试文件运行 `rustfmt --edition 2024 --config skip_children=true`，完成语法解析和格式整理。本轮不运行应用、build 或测试；当前最终静态检查结果统一记录在[重构状态](03-implementation-gap.md#5-验证状态)，不以历史编译结果代替当前检查，也不把编译通过记作测试执行通过。

增加的行为回归覆盖符号到物理键转换、旧 `Ctrl++` 回读、未知字段保留、数字回退 / 边界、网站补全 / 无效值、按布局与层过滤、Hypershift 灵敏度真实限制、原值恢复不标脏、未知值不阻塞其他配置、Specific 不可指向当前 profile。原有录制、保存 / 丢弃 / 继续编辑、主点击限制、UTF-16 文本上限测试保留。

[焦点回归源码](../../src/features/mapping_focus_tests.rs) 覆盖拒绝切键后返回原输入、干净及未保存抽屉关闭、切页、全局保存释放编辑器焦点，以及保存不抢占其他弹层。编辑器和抽屉另补实际下拉选择、左右 Shift、Turbo 同步、Tab 滚动和筛选移除行的回归源码；均只做编译检查。
