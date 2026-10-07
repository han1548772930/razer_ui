# 映射编辑器：源码对照与本地实现

实现入口为 `src/features/mapping_editor.rs`、`mapping_keys.rs`；各产品的当前 JS/CSS 与提取数据定义可用功能和编辑条件。

## 原始证据

| 范围 | 原始入口 / 数据 | 当前行为 |
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
| Macro | 182 的 5107 → MapMacro/2508，653 的 3241 → MapMacro/82508；[当前收据](product-macro-mapping-current-evidence.json) | 实际 DeviceWorkspace 编辑器接共享本地 MacroLibrary，按宏类型/当前输入显示播放方式，次数 1–99，保存到对应 profile/层/拨盘的本地映射。 |

### Hypershift 灵敏度过滤

6903 的 `buildDataSet` 在 Hypershift 使用 `filter(e => e.id !== g.GGY)`。
main 9937 明确导出 `GGY:()=>m`，且 `m="DPI_OnTheFly"`。
Hypershift 排除 **即时灵敏度**。底部 `CycleUpSensitivityStages` 同时排除 Clutch 和即时灵敏度。

653 中禁止 Hypershift 映射的键以各布局 `disableHypershiftMapping` 为准，包含 F9、F10、F11、F12、Pause、Application；不再从键名自行猜测。

## 编辑与保留语义

- 录制使用捕获阶段接收 Space、Enter、Tab 等，避免按钮先处理按键。Escape 先退出录制，非录制时关闭编辑器并进入原有未保存继续流程；Escape 键本身仍可在 Navigation 中选择。
- 程序文件选择异步返回时核对 profile、层及 input 身份，避免把结果写入已切换的编辑对象。
- 保存、取消、切页和抽屉关闭统一处理焦点与录制状态；未保存确认选择“继续编辑”不覆盖原返回目标。抽屉在“自定义”筛选下保存 Default、原行消失时，焦点返回筛选框。抽屉内 Tab 导航会将当前行滚入视口，关闭抽屉会将列表入口滚入正文视口。
- 键盘下拉保留实际选项身份，调整 Turbo 不会将物理 `=` 加显式 Shift 擅自显示为 `+`；再次确认当前录制分组也不清空已经录制的键。持久化保存最终组合键，重新打开时进入录制分组，不保存上次选择符号或辅助键的操作历史。
- `local-mapping:v1:` 仍是本地配置格式，不是 Synapse 原生协议。可保存 Keyboard、Mouse、Sensitivity、Macro、Multimedia、Windows、Text、Profile、Launch、Brightness、Default、Hypershift、Disable。
- 未知版本、未知字段及未知旧值原样保留。打开它们不会阻止保存其他配置；编辑器仍明确表示该映射不可编辑。切换功能后又还原相同语义时恢复原字符串，不制造旧格式迁移或脏状态。
- 标准鼠标层仍要求至少保留一个左键单击；普通层 / Hypershift 层继续使用独立 bindings。
- 跨设备及 Chroma 类别仍显示不可用；Macro 的通用编辑路径与宏页的 Key Binds 会话表分别记录，见下节及 [Macro](macro-ui-current.md)。原生键注入、设备写回与本地映射保存分开。

## Macro 本地选择与保存

`ProductWorkspace::new` 对已有 Tab 的产品创建 `DeviceWorkspace`，182/653 的输入能力实际开放 Macro，不是不可达示例。Shell 将同一 `MacroLibraryFile` 注入新建工作区并广播库变化；`mapping_macro.rs` 保留 selector、playback 和 Stepper 实体，`mapping_macro_data.json` 只包含逐源核实的产品能力，不增加运行时产品号分支。

普通宏在普通键上提供 Once、NTimes、ContinuousToggle、ContinuousHeld、Queue；ScrollUp/ScrollDown 仅 Once、NTimes、Queue。Sequence/Phased 分别只有对应单项。产品映射不能采用全局快捷键排除 ContinuousHeld 的集合。两个当前产品没有 singleProfileDevice 标志，源 held 特例产品表也未包含它们。

空动作文档仍可选择，文件夹不进入列表；无宏时显示源 CREATE_MACRO_MSG 并禁用保存。宏列表按当前 3466 比较器稳定排序，以本地文档 ID 保留选中。正常单候选不能展开；引用删除后仅剩一个候选时允许展开修复，单一播放选项也允许修复不兼容旧值。CONFIGURE_MACROS 打开已实现的本地 Macro 页，不要求虚构安装状态。

映射存储 `{macro_id,name,playback,repeat_count}`，没有 native `guid`。NTimes 为 1–99，两位整数；其他方式写 2。空串/减号等临时输入不能保存；重新打开编辑器重置数字输入的旧焦点草稿和重复任务。库重命名刷新显示名，删除或类型改变保留非零引用/原值并要求显式修复，观察本身不提交或制造脏映射。没有改动的失效旧值仍可保留，不阻塞其他配置的本地保存。

编辑器 Save/Cancel/未保存继续复用原映射生命周期；普通层、Hypershift 和自定义拨盘仍保存到各自容器。宏页 Key Binds 的会话关联没有因此变成双向同步或持久设备绑定。真实宏执行、服务 GUID、设备配置与 DLL 写回未接入。

`audit-product-macro-mapping.cjs --check` 核对两款当前 manifest、懒加载路由、MapMacro、播放表、比较器和 CSS。源组件依赖数组索引及部分更新分支会丢掉限制，本地按身份保留并重新应用输入资格；源 `AX$.SYNAPSE.SEQUENCE` 的错误不作为类型规则。

## 验证状态

允许的验证见 [开发检查](../development.md)。回归源码仅编译；不能将编译通过记作测试执行、完整交互或设备写回验收。

增加的行为回归覆盖符号到物理键转换、旧 `Ctrl++` 回读、未知字段保留、数字回退 / 边界、网站补全 / 无效值、按布局与层过滤、Hypershift 灵敏度真实限制、原值恢复不标脏、未知值不阻塞其他配置、Specific 不可指向当前 profile。原有录制、保存 / 丢弃 / 继续编辑、主点击限制、UTF-16 文本上限测试保留。

[焦点回归源码](../../src/features/mapping_focus_tests.rs) 覆盖拒绝切键后返回原输入、干净及未保存抽屉关闭、切页、全局保存释放编辑器焦点，以及保存不抢占其他弹层。编辑器和抽屉另补实际下拉选择、左右 Shift、Turbo 同步、Tab 滚动和筛选移除行的回归源码；均只做编译检查。

[Macro 回归源码](../../src/features/mapping_macro_tests.rs) 使用真实 click/press 覆盖普通键播放/次数/保存与放弃、空库到达/Configure 导航、删除后修复、重命名与类型改变、临时次数禁存和键盘滚轮播放限制；没有运行这些测试或执行宏。
