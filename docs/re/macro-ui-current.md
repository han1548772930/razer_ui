# Macro 当前界面契约

当前来源是 `.ref/applications/synapse/macro/asset-manifest.json` 声明的应用包，以及 `.ref/host-4.0.827/` 的 Mapping Engine 包装器。Macro 主 JS 的 SHA-256 为 `fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d`；8190 JS 为 `003b116007bb415462e5175c09fc84928d2a20c8d1a7c73113a432643d2cbe9f`。仅进行静态源码、资源、格式和类型检查；应用、测试、厂商 JavaScript 与 DLL 均未运行。下面的“接入”表示代码入口和本地数据链存在，不表示运行或像素验收完成。

## 页面与保存边界

- Macro 是窗口名 `macro`、地址 `/synapse/macro/`、`policy=3,tab_visible=1` 的独立应用。My Macros、Key Binds、Help 分别挂载当前模块 58190、21700、1519；产品的 `displayMode=macro` 是绑定弹层的专用产品根。
- 页面保留自己的 My Macros/Key Binds/Help 历史。Back/Forward 只移动该历史，录制忙碌时不能切页。Help 位于右端；没有宏时禁用未选中的 Key Binds。刷新走当前宏的未保存确认。
- `MacroLibrary` 保存目录、当前项、教程及明确提交的动作；`MacroPage.actions` 和非当前宏草稿保留独立的 Undo/Redo/选择。目录、教程、偏好或导出不能顺带保存未确认的动作。
- 当前动作 Save 只提交当前宏。切页保留草稿，刷新重建已保存文档并清除其他暂存草稿。新建、切换宏或刷新遇到 dirty 时暂停原动作：Save 提交当前动作后继续；Don’t Save 恢复保存基线后继续；关闭或 Escape 保留原草稿并取消跳转。Enter 不被解释为默认保存。
- 工作区 v2 的 `macros` 字段兼容缺省空库；本地单调 ID 删除后不复用。校验唯一身份、高水位、父文件夹、无父链环和引用类型。后台串行写入仅确认捕获版本；写入期间的新提交仍待保存，不覆盖新版本。无法解析的原配置禁止覆盖。
- 本地数字 ID、XML 交换 GUID 和服务 GUID 是不同身份。UI 保存、XML 导出和本地绑定均不表示设备写入成功；DLL 配置修改、设备保存和关联写回仍后置。

## 配置栏、教程与菜单

`Mn` 在已创建但未完成教程、activeRecord、activeSettingWindow 或 Help 时禁用整个 profile bar；录制 opacity 为 .3，其余禁用为 .8。原生 `profile_actions_blocked` 同时守住 New、selector、More、重命名、复制、移动和删除入口，未保存弹层存在时也禁止背后配置操作。Initial 阶段仍允许首次 New；教程已完成的空库仍可打开目录管理文件夹。

教程为 Initial → New → Record → Next → Add → Done → Complete，Skip 直接完成教程。教程期间 palette 和动作列表禁用；教程浮层本身接收点击。普通及 Phased 列表的 Capture 会停止禁用区鼠标事件，因此 onboarding 和录制设置根使用 `.occlude()` 排除下层 hitbox，保证 Next/Skip、配置 radio 和下拉可达。教程状态发布到本地库。

目录与菜单依当前 `Mn/Bt/an/dn/kt/Ut/Rt/Ct/Zs`：

- 顶部菜单提供 Add、Import、Rename、Duplicate、Export、Delete；Upload/Reshare 依真实共享能力，未制造可用状态。目录内 New Macro 经过未保存保护；New Folder 是独立目录操作。
- 新宏和文件夹加入根，文件夹初始不展开。树文件及文件夹均有 Rename/Duplicate/Delete 和右键菜单；顶部 Duplicate 选中新副本，树 Duplicate 保留当前宏。复制保留 live 草稿，递归分配新身份/名称，嵌套宏引用仍指向原目标。
- 搜索为 trim 后忽略大小写；命中文件夹保留全部后代，否则保留匹配路径。树复制使用搜索投影，顶部复制使用完整目录。排序仅作用于根列表；名称比较保留源 UTF-16 两级权重与第二单位反向比较，日期按创建次序。
- 拖到文件夹进入子列表，拖到文件追加至其兄弟列表；拒绝自己和后代。删除文件夹递归删除子宏，当前宏被删后选创建顺序第一项；仅直接删除唯一宏的源条件触发回 My Macros。
- 重命名限制 32 UTF-16 单位，Enter 触发 blur，blur trim 并校验，Escape 恢复。树名称同时检查同级项及宏全局名称；顶部只检查宏名称。

## 动作编辑

600px 编辑器、左侧 250px palette、42px 行与底部 100px drop-space 已接入。列表高度按 `max(outerHeight-208,450)`；≤1120 的定位规则和 ≤900 的 wrapper 规则分别处理。基础 palette 顺序为 Delay、Keyboard、Mouse、Macro、Launch、Command、Text、Loop；AI 模板需要明确能力，Sequence/Phased 禁用 Delay 和 AI 项。

- 全选、部分选中、批量删除、Undo/Redo/Save 作用于真实动作草稿。选择不生成 Undo；有选择时 actionbar 用计数和删除替代时长，录制时隐藏历史/保存。时长计入 Movement.Number。
- 单行复制按当前 reducer 建立配对：Keyboard 两行，Mouse 滚轮单行/按钮两行，Loop 起止两行。单行删除 Keyboard/Mouse 只删点击行；Loop 仅按明确 identity 删除另一端。批量删除仅删选中行。
- 拖选中行移动整个选中组，否则移动当前行；后插规则、配对边界和 Loop 交叉修复按源处理。拖动绑定原文档与动作快照，过期结果不提交。配对身份不由显示文字推测；无对应端的 Loop 只删本行，避免原异常删除不相关末行。
- Delay 模板为 `0`；普通输入整数不补零，最多三位小数，150ms 后更新草稿，blur/Enter 完成。仅宏 delaySetting=2 且 Number 为对象时显示双范围；切换录制设置不把 scalar 自动转成范围。输入定时提交受编辑代际、文档、重排、Undo 和 Save 保护。
- Keyboard 使用当前 144 项目录、20 个布局名称表和 3 个扩展映射。当前窗口原始键捕获保留 92→91 的 keydown 特例、Enter/右 Shift 区分和浏览器 Ctrl/Alt 首项规则；150ms 提交。Standard/Phased 建配对，Sequence State=null，孤立行保持源 flag/State 更新边界。清除空键保留内容，避免源空目录项解引用异常。未知服务布局使用源 UnitedStates/default 回退，不伪造布局观察。
- Keyboard 的 Windows 捕获仅在当前 UI 线程、当前 HWND 的前台消息中工作，RAII 清理；不能代表设备 inputredirect 或全局快捷键服务。过期/失焦/切页提交受文档和动作基线保护；非 Windows 不进入假捕获状态。
- Text 使用独立弹层草稿，只有弹层 Save 提交动作；Cancel/close/外点/切换或外层 Save 丢弃未确认文字。限制 250 UTF-16 单位，emoji 追加末尾；非空改动启用 Save 后再清空仍可保存空串。数据为当前 Macro 的 1703 分类项、1936 搜索项、230 变体；搜索忽略英文大小写，清空可见搜索框保留源已有过滤结果。未知 Windows 版本不启用仅版本 `11` 的过滤。
- Text/emoji 保留源定位阈值、滚动防抖、500ms/300ms 透明度和真实匹配 CSS；分类标题及变体的字面 class 未匹配源 CSS，不擅自套用改名后的格子样式。字符映射表按钮接本地系统入口，没有执行宏动作。
- Launch 初始 RadioIndex=null。程序通过文件选择只保存首个路径，网站保留原文；Save 合并为一次 Undo，Cancel/close/外点不提交。请求代际/文档/模式检查拒绝迟到文件结果，不执行选中程序或打开网站。定位按普通/Phased 祖先和 `editor.clientHeight-row.offsetTop > 100` 条件。
- Nested Macro 使用持久本地目标身份，候选取活动草稿图，排除当前宏和能回到当前宏的路径；重命名随目标更新，目标删除显示占位，不按名称改绑。图遍历有 visited 保护。三态菜单分离编辑框/选项展开，重选同项只收起列表；方向按源 688px 阈值，打开滚到已选项，保留 200ms 高度/100ms 边框/300ms 触发器时序。
- Phased 已有 press/hold/release 分组、活动阶段、新增/拖排归属与阶段头目标；阶段 Record 选择当前阶段并调用共享录制入口。保存的 Phased 文档可显示，类型选择仍依源能力；不会生成假的能力响应。

## 录制与观察

录制设置含 Standard/Sequence/Phased、Recorded/Fixed/Random/None、倒计时与快捷键编辑。快捷键目录为有序 122 项；忽略 F12/Left GUI，保留源修饰键顺序和 keyup 提交。本地快捷键字段仅捕获当前窗口输入，尚未注册系统全局 start/stop。

独立 actor 持有阻塞 `ServiceClient` 和清理生命周期，GPUI 只发送命令并观察异步更新。Start accepted 后保持 Starting，真实 started 才转 Recording；Stop accepted 后保持 Stopping，直到 stopped、转换、映射恢复及 Shutdown 完成。取消、关页、通道关闭及迟到完成都保留原草稿；无法确认清理时明确报错。开发过程中未启动此 actor 或 DLL。

最终转换读取微秒 delay、tick、真实 callback 与鼠标前缀，保留原始 `recorded_input`；按当前 `_r/Nr/Rr/G/H` 分别处理 Standard/Sequence/Phased、重复按下及停止左键/Escape 取消。随机范围不抽样；相邻随机对象不持久化源 `parseFloat(object)` 的 NaN。固定/随机输入在启动前验证有限、非负和范围顺序。

临时预览独立按 `le` 转换，只保留末尾 50 行并跟随滚动，不进入 actions、Undo 或保存基线；预览和最终 Sequence XBUTTON 的源顺序差异保留。有效 stopped 且清理成功后，整次录制一次追加本地草稿。错误、未知输入、overflow、缺失状态或超出数据上限均拒绝假成功。

鼠标轨迹仍缺录制前后 monitor/virtual screen 查询及坐标转换，控件禁用并说明原因；原 callback buffer 不等于屏幕几何观察。没有实际设备录制成功记录。

## XML 与设备绑定

XML 导入/导出已连接文件选择、后台读写、校验、取消/失败与本地库。Version 4 导出当前 actions，包含 Name/MacroEvents/DelaySetting/Guid/MouseMoveType，三空格缩进；导出不等于 Save。新建、复制和导入获得独立交换 GUID。只有文件写入完成才提示导出成功，导入重新检查最新库，不覆盖等待期间的草稿。

支持现有 Delay、Keyboard、Mouse、Movement、Text、Command、Loop、Nested Macro、Launch。导入 Keyboard/Mouse 重建明确身份；Loop 缺少配对 Id 时按源保留，nested 未解析 GUID 不猜目标。scalar Delay 导入依 `hn` 固定三位，与行编辑规则不同。Movement 导入标记 `imported_xml=true`、geometry=None，不冒充录制观察。DTD 拒绝，普通文本和 CDATA 保留。

旧 Synapse 3、对象型随机 Number、phase 以及会被源二次 HTML 实体解码改变的字面文本明确报告不兼容并拒绝整份导入；未实现动作不静默跳过。导出仍保留动作的范围和 phase，因此不承诺全部导出可再次无损导入。

Key Binds 使用当前 `ProductWorkspace` 设备/配置实体与观察订阅。无设备显示源警告并禁用添加；设备消失返回选择，配置/输入失效清除编辑目标。设备、profile、input、Standard/Hypershift 层共同标识本地关联，Save 校验后替换该目标，Cancel 不提交，删除只移除本地关联。绑定目前是会话数据，未接设备写回与关联持久恢复。

当前 `macro_input_catalogs.json` 按能力选择 PhysicalGroups 或 PreparedKeyboardLayout，运行模块不按产品号硬编码分支。已核实的鼠标物理 groupList 保留 8 个输入和 LeftClick 禁用，770×340 图与锚点；已核实键盘按真实 layout/形状处理，未知 layout 不借另一产品目录。其余产品使用已有目录回退，不因此计作 174 个 macro 根逐项完成。专用播放表单保留轮滚限制、1–99 次数和默认 2；其他产品独有条件仍待审计。

182/653 的通用 DeviceWorkspace 映射编辑器也已接共享本地宏库：选择普通/Sequence/Phased 宏、按输入限制播放方式、编辑 1–99 次数，经既有 Save/Cancel 保存本地 profile 映射。Shell 向新建工作区注入并广播同一宏库；删除目标保留失效 ID，不能静默换绑，重命名/类型观察不直接改写已存映射。其持久 `macro_id` 与此处 Key Binds 的会话关联分开，未建立两表双向同步，均不冒充服务 GUID。具体路由、交互与静态来源见 [映射编辑器](09-mapping-editor.md)。

## 剩余项

- 长列表虚拟化、Phased 分组配对线、Le 200ms 虚线动画、200ms leading/trailing 选择/复制防抖。
- 窄窗导航收纳、完整 toolbar tooltip、Command 警告弹层、钢笔编辑光标、滚动条和剩余精确布局/动画。原生 mousedown-out 与源 window click 的外点时机差异仍存在。
- 鼠标轨迹的真实屏幕查询/转换、全局录制快捷键、设备 inputredirect、真实系统键盘布局及服务能力发布者。
- XML 旧版/范围/阶段/字面实体兼容性，以及全产品 macro 根的形状、独有输入门控和播放限制。
- 服务 GUID/设备配置同步、绑定持久恢复与设备写回；这不免除现阶段 UI 编辑/保存缺口。
- 实际窗口的指针命中、键盘/IME/焦点、文件选择及落盘、滚动、缩放、字体和视觉验收。静态检查不能替代这些结果。

## 维护证据与回归入口

`src/shell/macro_page/tests.rs` 使用真实 `TestWindowExt::click/press` 控件路径覆盖教程 New/Next/Done/Skip、配置浮层、Add/Undo/Redo/Save、profile 禁用、未保存 Save/Discard/Escape。录制生命周期用 render-only 状态夹具，不启动 worker。根/列表/状态/行/浮层使用稳定 test-support 标识，按钮沿 Kit instrumentation；这些测试只供全目标编译，尚未执行。

机器证据保留原始来源、SHA、范围、CSS 和资源事实；收据中的 native 文件指纹只代表取证时文件，不能通过刷哈希宣称整个页面重新验收。当前契约的变化才触发相应静态审计和入口复查。

| 范围 | 保留证据 | 维护工具 |
| --- | --- | --- |
| 配置栏/未保存/菜单/键盘提交 | [command flow](macro-command-flow-current-evidence.json) | `audit-macro-command-flow.cjs` |
| 教程与浮层命中条件 | [onboarding](macro-onboarding-current-evidence.json) | `audit-macro-onboarding.cjs` |
| 页面根/动作/XML/CSS | [full page](macro-full-page-source.json)、[editors](macro-editors-current-audit.json) | `audit-macro-full-page.cjs`、`audit-macro-editors.cjs` |
| 目录元数据 | [metadata](macro-metadata-source.json) | `extract-macro-metadata.cjs` |
| 键盘/Text/Launch | [keyboard](macro-keyboard-current-evidence.json)、[text](macro-text-current-evidence.json)、[launch](macro-launch-current-evidence.json) | `audit-macro-keyboard.cjs`、`audit-macro-text.cjs`、`audit-macro-launch.cjs` |
| 选择/行操作/Phased | [selection](macro-selection-current-evidence.json)、[row actions](macro-row-actions-current-evidence.json)、[phased](macro-phased-current-evidence.json) | `audit-macro-selection.cjs`、`audit-macro-row-actions.cjs`、`audit-macro-phased.cjs` |
| 录制设置/事件/当前 host ABI | [options](macro-record-options-current-evidence.json)、[recording](macro-recording-current-evidence.json) | `audit-macro-record-options.cjs`、`audit-macro-recording-current.cjs` |
| 绑定/能力输入目录 | [bindings](macro-bindings-current-evidence.json) | `audit-macro-bindings.cjs` |
| 通用产品 Macro 映射 | [product mapping](product-macro-mapping-current-evidence.json) | `audit-product-macro-mapping.cjs` |
| 嵌套/共享库/快捷键映射 | [shared contract](shortcuts-macro-current-contract.json)、[快捷键契约](shortcuts-macro-current-contract.md) | `audit-shortcuts-macro-contract.cjs` |
| 应用窗口/语言/资源 | [app](macro-app-current-audit.json)、[chrome](macro-app-chrome-audit.json)、[UI extraction](macro-app-ui-audit.json)、[locale](macro-locale-source.json) | `audit-macro-app.cjs`、`audit-macro-app-chrome.cjs`、`extract-macro-locales.cjs`、`prepare-macro-assets.py` |

UI extraction JSON 中旧的语言/资源缺失推断不作为当前缺口依据；语言取专用 18442 → 81250 及英语 fallback，资源取当前 manifest/CSS 和嵌入文件字节。原始提取数据保留以追溯证据，不重新运行错误字典定位逻辑。
