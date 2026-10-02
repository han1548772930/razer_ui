# 全局快捷键：原生引擎编码

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](20-current-source-version.md)为准。

实现为 [shortcut_engine.rs](../../src/features/shortcut_engine.rs)。`encode_shortcuts(&[Shortcut])` 生成原引擎的 `{mappings,hash}`，不访问设备、不注册快捷键、不启动进程。整个列表全部编码成功后才返回；任一不支持项返回错误，调用方可继续保留本地配置和原引擎状态。

## 已对照的调用链

| 原始位置 | 证据与实现 |
|---|---|
| [当前4608](../../.ref/applications/synapse/dashboard/static/js/4608.e973916f.chunk.js) `94608` 内 `setDataToMappingEngine` | 先调用 `generateAppEngineMappings`，再对只有 mappings 的对象计算 hash。 |
| [2280](../../.ref/applications/synapse/dashboard/static/js/2280.4d9e22ab.chunk.js) 模块 36383 / 42358 | `getAppEngineMapping` 按输出组分派，`Ot` 将 downInput/downOutput 和 upInput/upOutput 组成两条映射。引擎条目只包含 input/output，不包含本地 profile 或快捷键身份。 |
| 42358 `vt` / `Qe` | 键盘输入 `{type:"keyboard",scancode,hypershift,flag,modifiers}`；按下使用 `outputFlag ?? flag`，松开再加 1。泛化 ALT/CTRL/SHIFT/GUI 掩码为 1/2/4/8；左侧为 256/512/1024/2048，右侧为 16/32/64/128。这些不能和 Win32 RegisterHotKey 掩码混用。 |
| 42358 `rt` / `Dt` | RightClick 4/8，ScrollButton 16/32，Button4 64/128，Button5 256/512。它们是引擎鼠标位域。rt 的这些分支返回对象没有 data，序列化必须省略，不能补 0。 |
| 2280 模块 629 | 原始键表以 scancode 和 outputFlag 提供编码，不从 Windows 虚拟键或 HID 编号推算。静态提取 127 个非零扫描码输入，包含国际键和原始扩展标记。 |
| [55](../../.ref/applications/synapse/dashboard/static/js/55.3f1ab18c.chunk.js) 模块 59007 `customizeReducer` | 真正调用 `localStorageSetItem` 前，对 Backspace/B/Space 追加 DKM_SB_251/252/253 别名，再通过 `ye` 追加两套输入：mouse/keyboard 的 unitId 为 0、43981；razerKey 的 reportId 为 4、8。顺序为原输入、别名、第一套变体、第二套变体，之后才算 hash。 |
| 7282 `Oe.componentDidMount` / 42358 `mt` | `setDKMKeys` 提供 251/252/253 的真实 key，mt 生成 `{type:"razerKey",key,hypershift,flag,modifiers}`，flag 为 0/1。 |

因此仅调用 `generateAppEngineMappings` 的中间结果还不是此页面完整的服务提交对象。本实现保留 59007 的变体，而不是从扫描码猜测设备标识。不同本地按键若解析为同一原生扫描码、标记和辅助键组合，整次编码拒绝，以免生成重复输入覆盖。

[extract-shortcut-engine.cjs](../../tools/extract-shortcut-engine.cjs) 使用 Acorn 读取字面量与 `parseInt` 字面参数，生成 [shortcut_engine_keys.rs](../../src/features/shortcut_engine_keys.rs)。不 eval、不 require 原 bundle；表头记录源文件 SHA-256。

## 编辑、保存与本地检查

GPUI 的键事件只有 Ctrl/Alt/Shift/Win 布尔值，没有原生录制器的扫描码及左右标记。因此新录制使用原引擎已有的泛化 `CTRL/ALT/SHIFT/GUI`，辅助键控制也按通用含义保存。旧 `KEY_LEFT_*` / `KEY_RIGHT_*` 记录保留原值、在组合标签中明确显示左右侧；用户重新录制或重新勾选对应辅助键后才转成通用值。读取旧文件和检查配置不会修改已有语义。

7282 `updateMapping` 对空的 modifiers 调用 `getModifiers`，默认补 Ctrl+Shift。本地键盘录制和鼠标输入选择遵守此默认；提交空辅助键草稿时再次归一化，并在重复检测中使用同一默认值。没有把无辅助键的录制默认为裸按键全局映射。

本地保存异步完成时只将捕获的快捷键列表标为 saved，期间新增修改继续保持未保存。切页可保存 / 丢弃当前快捷键草稿或继续编辑；已加入列表但尚未写磁盘的修改继续保留。程序文件选择使用草稿代数加身份验证，关闭后重新编辑同一条记录、切换类别及后发的文件选择均会使先前回调失效。

页面的“检查配置”调用实际 `encode_shortcuts`，显示通过或具体编码错误，不显示技术 hash、不触发运行时调用；有打开的草稿时先完成编辑。检查结果在后续编辑时清除。“应用到引擎”保持禁用，原因是“暂时无法读取引擎中的现有快捷键，尚不能应用。”因此本地保存、可编码状态及引擎应用不会混淆。

当前重复检测拒绝完全相同的输入及扫描码别名，但不把旧“左 Ctrl+K”和新“Ctrl+K”等不同掩码直接判成相同。启用引擎前还需核实并处理通用辅助键与左右侧辅助键的匹配重叠；目前没有用未经证实的原生匹配算法模拟冲突，应用入口继续禁用。

## 输出覆盖

| 类别 | 已编码行为 | 当前限制 |
|---|---|---|
| Program | 74488 的 .exe 分支：launch、原 path/pathToCheck、startHidden=false，松开 disabled。 | 仅绝对 .exe 路径；原非 exe 分支会拼 cmd /c，当前明确报错。 |
| Website | 74488 的 `cmd /c start "" "URL"`，startHidden=true；沿用本地 HTTP(S) 归一化。 | 引号、控制字符及 `%`、`!`、`^` 不能安全放入当前已证实的 shell 分支，明确报错，不提交。 |
| Text | 42358 的 clipboard / id:text，松开 disabled；原文本保留。 | 继续遵守本地 250 个 UTF-16 单元限制。 |
| Multimedia | 42358 `ce` 的 10 项本地菜单操作全部编码。音量/前后曲输出准确键盘按下、松开；播放和静音为带 10ms delay 的 multi；麦克风和全静音使用 audio，repeat=1。 | 当前仅四种双边缘鼠标输入和键盘输入；滚动单边缘触发的 Turbo 分支未开放。 |
| Windows | Calculator、MSPaint、Notepad、Snipping Tool、Task Manager、Copilot、User Directory、File Explorer；SwitchApps；Mail、ThisPC、Refresh；DisplayBrightnessUp/Down。 | 下列 12 个操作因原生 Turbo 依赖而拒绝。 |

`SwitchApps` 的按下为 Alt down / 10ms / Tab down，松开为 Tab up / 10ms / Alt up。亮度在松开时输出 `driverBrightnessStop`。原生 launch 的 `startHidden` 只有 cmd 前缀时才存在；普通 calc 等不补 false。

## 不能伪造的 Turbo 依赖

42358 `generateDefaultTurbos` 通过 `gt` 为预设事件生成随机 GUID；事件结构在同包模块 51480 的 `createEvent / getData`。Windows `fe` 对 PowerUserMenu、ShowDesktop、CycleApps、CloseApp、Cut、Copy、Paste、WindowsZoomIn/Out、OfficeZoomIn/Out 输出引用该 GUID 的 turbo。只有 `{mappings,hash}` 无法代表这些事件已经向引擎注册，因此当前返回明确错误。

`LockComputer` 的全局快捷键分支同样选择 turbo，但当前原默认 Turbo 列表没有 LockComputer。不能生成无 GUID 的引用，也不能擅自替换成独立 Win+L 实现。未来接入需同时解决默认事件持久身份、事件注册、失败及回滚；事件源码存在不等于引擎已具备这些事件。

## 哈希算法

2280 模块 19019 `getHash` 浅删除顶层 `hash` 和 `gamemode`，再调用 [main](../../.ref/applications/synapse/dashboard/static/js/main.01550b17.js) 模块 5371 的稳定 JSON stringify：对象键按 JavaScript UTF-16 顺序递归排序，数组保留顺序，紧凑序列化。然后把所有 `<` 替换成字面 `\u003C`，交给模块 32937 的 MD5，以小写十六进制表示。

Rust 实现显式排序，不依赖 serde_json 是否启用 preserve_order；摘要依赖 `md5` crate。空对象映射 `{ "mappings": [] }` 的原规范摘要是 `de2ccb2014607a84df08306567cd96f0`。包含 `<`、中文、emoji 的完整输入变体摘要另作为固定回归向量，避免 Unicode、转义和排序差异。

## 验证范围

[编码回归源码](../../src/features/shortcut_engine_tests.rs) 覆盖完整 JSON / 独立计算的固定摘要、四种鼠标、国际键、Pause/扩展键、左右与通用辅助键掩码、输入变体/DKM 别名、音频标志和时序、可用 launch、无法安全传递的 shell 值、未注册 Turbo 和重复原生输入。[界面回归源码](../../src/features/shortcuts_tests.rs) 覆盖真实录制与控件、Ctrl+Shift 默认、旧记录保留、配置检查和禁用应用入口、迟到的文件选择结果。仅提供并编译检查回归源码，没有执行测试、运行 DLL、注册引擎、启动目标或验证硬件行为；最终 `cargo check` 状态由项目总体记录维护。
