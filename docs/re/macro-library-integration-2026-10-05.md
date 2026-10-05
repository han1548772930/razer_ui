# Macro 共享文档与快捷键集成：2026-10-05

续接 `01a109ca-c22e-73e3-9de0-a3eba5f79bc6` 的当前稳定版 UI 复刻。本批完成共享本地 Macro 文档、快捷键宏映射、未保存提示，以及 3946 类型菜单和重复次数输入的接入。**总体目标仍未完成，没有将任何产品标为完整复刻。**

证据仅使用当前应用 manifest 声明的源码，主契约来自当前 Macro 和 Dashboard。没有读取或重建用户删除的四个历史目录，没有执行厂商 JavaScript、应用、构建、测试、安装器或 DLL。主契约的 AST、CSS、源哈希和本批 Rust 文件指纹见 [当前契约](shortcuts-macro-current-contract.md) / [机器收据](shortcuts-macro-current-contract.json)。文件指纹锁定复核版本，不是语义测试。

## 已实现的本地行为

| 范围 | 行为 |
| --- | --- |
| 共享文档库 | Shell 创建并持有 `Entity<MacroLibrary>`，Macro 窗口与快捷键共享；不依赖先打开 Macro 窗口才能获取目录 |
| 文档与草稿 | 每个文档分别保存动作副本；当前草稿与暂存的非当前草稿保留动作、撤销/重做和选区，不写入其他文档 |
| 未保存提示 | 新建宏、切换另一宏、刷新时按当前源挂起动作；保存/不保存后继续，关闭或 Escape 取消挂起并保留草稿；新建文件夹和 Macro 内页导航不额外提示 |
| 复制 | 顶栏复制活动草稿并选中已保存的副本，原文档草稿仍在；树菜单复制活动内容但保持当前文档；复制不静默保存原件 |
| 显式保存 | 输入框还未失焦时即可启用保存；点击先提交行编辑器，再保存当前文档；局部 Launch 弹窗仍按自身保存/取消边界工作 |
| 目录操作 | 新建、重命名、递归复制、移动、删除及教程状态发布到共享库；删除清除对应草稿与会话设备绑定 |
| 快捷键宏映射 | 可选真实本地宏，包括空宏；当前源名称比较器及稳定创建顺序；重命名/重排保留身份，配置宏入口直接打开已实现窗口 |
| 播放集合 | 普通宏为 Once / NTimes / ContinuousToggle / Queue；Sequence 和 Phased 分别使用自己的单项集合；普通全局快捷键不包含 ContinuousHeld |
| 重复次数 | NTimes 显示 1–99 专用 Stepper；保留源输入期字符串语义和 `"2" + 1 → 21` 行为；更换映射重置输入，即使数值相同；非 NTimes 保存统一写 2 |
| 嵌套宏 | 接入 Type 7 的候选选择、持久本地引用、活动草稿路径检查、重命名随目标更新、删除目标显示占位、撤销/重做与复制保留原引用；详见 [专项](macro-nested-current-audit.md) |
| 3946 菜单 | 按当前 aH 的条件挂载、菜单行、图标、选中色、外点关闭和重复选择清空动作行为替换通用 Select；触发器边框/箭头保持 300ms ease |

Macro 保存提示使用当前 `Gr` 的语言键、400px 宽、20/30px 内边距、绿色边框、原关闭 SVG、两枚按钮和按钮透明度过渡。源背景条件挂载时已可见，未据 CSS 声明臆造入场淡入。焦点由本地 Dialog 生命周期管理，未冒充宿主 `setForceFocus` 服务调用。

## 本地文件格式与写入边界

工作区格式仍为 v2，新增带默认值的 `macros` 字段，旧 v2 文件和旧顶层设备数组在读取时得到空库。文件中保存 `next_id`、`entries`、`current`、`tutorial`。每个 entry 包含本地 ID、名称、类型、父文件夹、已保存动作和 `macro_type`；文件夹展开状态不序列化。

ID 是本地单调 `u64`，高水位随工作区保存，删除后不复用；不是原生 GUID。每个动作保留原有六个本地草稿字段 `kind/value/secondary_value/number_min/number_max/state`，另增默认未分配的可选 `macro_id`；并不是原生宏事件格式。读取和写入都校验非零唯一身份、高水位、父项存在且为文件夹、无父链环、文件夹不含动作，以及当前宏确实存在。嵌套引用另外校验类型与高水位，已删除目标保留为失效身份。

两个既有序列化写入路径都保留 Macro 快照；排队的设备保存开始写入时重新取得最新已提交文档库。成功写入只将捕获的 Macro 版本设为磁盘基线，不覆盖当前版本；I/O 期间的新提交保持 pending，并由既有串行续写机制处理。活动动作草稿独立于这个基线，不能被目录变化、教程状态或后台偏好保存顺带写入。

本次只修改工作区代码和审计资料，没有运行应用，也没有向真实用户配置写入示例库或演示快捷键。原配置不能解析时沿用既有禁止覆盖机制。`ShortcutOutput::Macro` 的引擎编码明确返回服务未接入，未把本地身份写成原生 GUID 发送。

## 源码缺陷与明确的本地适配

- 当前源原地排序目录却保留数字索引；本地按文档 ID 保持选中，避免持久引用换成另一宏。
- ID `0` 只代表空库占位，首次有宏后选首项。已删除的非零引用保持失效、保留原名称并禁止保存；只有一个替代候选时允许手动展开修复，其余单选情况按源禁用展开。
- 当前源类型变更分支错误引用延迟枚举中的 SEQUENCE；本地按已核实的真实类型集合处理。载入的播放方式若与所选文档类型不符，不能保存错误组合。
- 原 `changeTimes` 不校验临时空白/减号。本地类型化保存拒绝这类草稿；不能把这条数据保护规则描述为原版校验。
- Stepper 最终 CSS 覆盖结果只有 `opacity .2s`，边框即时变化；箭头在所有自定义状态中 opacity 都为 1。已接入根节点 200ms ease，支持既有减少动画设置；更正了先前「缺边框过渡」的描述。源 HTML 的负数 maxlength 与本地减号草稿长度仍有已记录的狭窄差异。

细节见 [Stepper](shortcuts-stepper-current-audit.md)、[3946 菜单](automation-quick-menu-current-audit.md)及[当前 Macro/快捷键契约](shortcuts-macro-current-contract.md)。

## 静态验证

最终 `cargo check --locked --all-targets`（含嵌套宏接入）成功，仅既有 `customize_page::layer_button` dead_code 警告。未运行测试 target；`cargo check` 只做类型检查。

本批检查通过：

- `cargo fmt --all -- --check`；include 文件 `shortcuts_macro.rs`、`shortcuts_mapping.rs`、`shortcuts_text.rs` 另行 `rustfmt --check`。
- `audit-shortcuts-current.cjs`、`audit-shortcuts-macro-contract.cjs`、`audit-shortcuts-stepper.cjs` 的 `--check`。
- `audit-automation-quick-menu.cjs`、`audit-automation-quick-keyboard.cjs`、`audit-automation-quick-program.cjs`、`audit-macro-editors.cjs`、`audit-macro-bindings.cjs` 的 `--check`。
- `validate-automation.py`：六种类别、七种灯效、实际参数分支、十份语言表、24 个 SVG。
- `validate-resources.py`：1,112 项源/输出哈希与嵌入键、35 个服务 SVG、133 个 Webpack 请求、74 个产品变体、59 个 Dashboard 变体、16 个键盘布局和 1,901 个输入形状。
- `validate-embedded-json.py`：33 份语法解析、30 份结构检查，三份既有无法推断结构的文件跳过结构检查，0 失败。
- `audit-locale-keys.py --check`：442 个字面量键、0 缺失；42 个软 fallback、51 个动态调用另行记录，五个既有软键缺失未冒充硬键通过。
- 限定 `src tools docs assets Cargo.toml Cargo.lock .gitignore` 的 `git diff --check`；三个新维护工具已加入 `.gitignore` 的允许列表，未暂存或提交用户更改。

上述是源码、资源和类型检查，没有字体、缩放、焦点、滚动、动画或真实交互的窗口验收结论。

## 后续缺口

嵌套宏选择、持久身份引用和活动图循环候选检查已接入；[菜单后续](macro-nested-menu-followup-2026-10-05.md) 继续补了定位阈值、选中项滚入、独立展开状态及过渡，仍缺编辑光标、滚动条外观和实际窗口验收。完整 Sequence/Phased 创作、真实录制、宏编辑器 emoji、XML 导入/导出、原生服务同步、全部产品绑定分支仍未完成。

3907 已连接状态、其他产品条件分支/弹层/动画、服务观测适配器和原始缺图仍在 [完整待办](remaining-ui-work.md) 中。下载审批基础设施故障及未补齐原始图片延续 [上一批记录](continuation-followup-2026-10-05.md)，本批没有绕过审批或增加网络操作。总体 goal 保持 active。
