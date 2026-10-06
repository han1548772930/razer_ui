# 此前已实现界面与产品独立复核

2026-10-06。用户要求单独开启核验子任务。本记录为第一批静态复核，不能作为全产品或视觉验收结果。只修改本报告；没有修改并行开发中的 Studio，也没有运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。

## 核验方式与范围

重新读取当前工作树的实际渲染器、描述符和路由，再对照当前产品包及 `/settings/` 源文件。已有收据只用于定位；下述音频及 Settings 收据的 SHA-256 和 UTF-16 范围已经与实际源文件再次比对。未访问 AGENTS.md 禁止的旧源目录。应用 `skills/gpui-kit` 和 `skills/gpui-kit-design-guides` 的审查要求，源产品行为优先于通用设计建议。

| 范围 | 本批实际检查 | 结论范围 |
| --- | --- | --- |
| 音频 3334、3337 | Stream Mixer 根、禁用链、预设操作与本地音频描述符 | 确认两类缺陷，见 A、B |
| 键盘 716 | 当前 Gaming Mode 挂载、Copilot 条件和本地 Gaming Mode 渲染 | 确认条件行遗漏，见 C |
| 独立 `/settings/` | Systray 的 launcher、双击动作、widgets 根；本地同页 | 确认双击动作下拉未实现，见 D；其他目录依赖分支仍未验收 |
| 键盘 515、697 | 原包键表/条件、部分 Gaming Mode 代码 | 辅助定位，不能把 716 的结论推广成这些产品整页通过或失败 |
| 音频 1382 | 本地 podMappings、当前 main → 913 → MapAudio 挂载、保存对象 | 第二批确认编辑器缺失，见 E |
| 鼠标 70 | Performance 的 cI → aI 阶段列表、关闭条件、拖排及 X/Y 更新 | 第二批确认阶段条件/编辑流缺失，X/Y 联动窄项静态一致，见 F |
| 鼠标/键盘主路由 | ProductWorkspace、SourceProductWorkspace、mouse/keyboard 渲染分支及覆盖统计器 | 排除仅由描述符中出现 TAB_PAIRING 推断“主导航漏页”的误报；非全家族复核 |

## A. P2：关闭 Stream Mixer 主开关后，本地总线推子仍可编辑

产品 3334、3337 的 `STREAM_MIXER_HEADER`。初始数据均为主开关 `true`、两个总线 `isEnabled=true`、`value=50`。将主开关改为 `false` 时，本地 `edit()` 只改该字段；总线开关仍为 `true`。随后 `enabled()` 只检查推子的 `enabled_by` 指向的总线开关，所以推子仍启用，滑块事件可以继续写本地草稿。

- 实现：`src/features/audio_products.rs:404`（单层 enabled_by）、`:473`（edit）、`:709`（slider 分支）。描述符：`src/features/audio_products_data.json:41397` 和 `:41417`（3334），`:41553` 和 `:41573`（3337）。生成器 `tools/prepare-audio-products.cjs:183` 附近只为推子生成总线条件。
- 当前源：3334 `main.496bb585.js`，UTF-16 `[4503735,4514366)`；3337 `main.a048e901.js`，`[4503695,4514326)`。根从 `e.isStreamMixerEnabled` 取得 `C`，外层为 `className:"widget-containers  ".concat(C?"":"disabled")`，两个总线和输入通道的 `uU` 均传 `isDisable:!C`。
- 影响：UI 禁用状态及草稿编辑与源不一致，与 DLL 写回是否接入无关。
- 建议：为这些具体控件表达主开关与总线开关的组合条件，并在事件写入口使用相同条件；不要仅修改 opacity。其他音频产品使用相同生成器，但本批只把 3334、3337 认定为源已复核。

修复续记（同日）：主线程已加入 `AudioControl.enabled_all`，重新生成的 3334/3337 四个总线推子同时具有原总线 `enabled_by` 和主开关 `enabled_all`。本子任务已重新读取当前生成器的源 hash/范围校验、描述符四项条件及 `enabled()` 的 `.all(...)`，确认静态禁用链已修正；`edit()` 仍调用同一判定。保留上面的原始发现作为修复依据，不将其继续计为未修代码缺陷。主线程负责本轮 cargo check；窗口中焦点、拖动和禁用状态的运行验收仍未执行。

## B. P2：Stream Mixer 独立预设编辑流被通用面板省略

同两个产品的本地主页只有主开关、Stream Mix、Playback Mix 三个 section，没有 Stream Mixer 自己的 preset 状态或菜单。产品顶栏的设备 Profile 菜单不能代替源 `streamMixerReducer.presets` / `selectedPresetGuid`。

- 当前源同 A。根接收 `addStreamMixerPreset`、`selectedStreamMixerPreset`、`duplicateStreamMixerPreset`、`renameStreamMixerPreset`、`resetStreamMixerPreset`、`deleteStreamMixerPreset`。`Re` 命令分支支持新建、复制、重命名、重置、删除；重命名 `maxLength:32`；重置和删除分别在 100ms 后打开；只剩一个 preset 时禁用删除；挂载 `wI` 重置与 `JT` 删除弹层。
- 实现：`src/features/audio_products.rs:827` 的 `page_body()` 完全依描述符 section 渲染，只有 1383 OLED 和 1303/1304 Nommo 特化；3334/3337 没有 preset 特化。`tools/prepare-audio-products.cjs:173` 起只提取四项 bus 状态，丢弃了上述 preset 状态和操作。
- 影响：用户要求现在完成的增删改、确认取消及本地保存流程缺失。不能把这整段归为“等 DLL 写入”。
- 建议：单独实现源 Stream Mixer preset owner/菜单/弹层/草稿持久化，并保留 DLL 提交意图与真实写回的边界。输入通道添加/删除、设备选择及快捷键也在该根中，但其完整条件仍需逐项复核，不在本批宣称已审完。

### 3334/3337 第二批：具体控件、初始状态与本地保存复核

本批只静态读取两产品当前主包和本地实现，未执行任何应用、构建、测试、供应商脚本或 DLL。3334 主包 SHA-256 为 `320597b62d8538c929ca38bbf594297f7151582c0ac61b3224ba86faa0d7eeca`；3337 为 `835ff3af59d774b85c02bfe97fae62e45ec0921218884fcc6f38224c4103b0b3`。重新计算指纹，并逐字比较两产品根和 uU，两者分别完全相同；没有由共用文件名推断产品行为。

本批当前源位置均为 UTF-16，3334 / 3337 顺序：根 `[4503735,4514366)` / `[4503695,4514326)`；connect `[4502941,4503735)` / `[4502901,4503695)`；uU `[4487717,4493354)` / `[4487677,4493314)`；Wn 初态 `[4027897,4028383)` / `[4027857,4028343)`。下表的“缺失”指当前可核实 UI 功能，不因 DLL 写入后置而自动延期。

| 控件或动作 | 当前源行为 | 当前本地实现与结论 |
| --- | --- | --- |
| 主开关 | 关闭直接 E(false)；开启时 activeStreamMixerStatus 为真先显示警告，取消仅关提示，确认才 E(true) | 描述符 toggle 直接 edit；无该观察状态及确认分支。普通开关可编辑，冲突状态下的确认流未接入；不能伪造 active 状态 |
| 监听目标 | 两总线各有耳朵入口，共用 isStreamMixMonitor，图标状态相反 | 单个 AUDIO_MONITORING checkbox 可改同一布尔值；数据可编辑，双入口、图标与提示未还原 |
| 两总线静音与滑块 | uU 的独立 isEnabled，0–100、步长 1；主开关禁用链见 A | 本地 toggle/slider 均有写入口；A 的主开关禁用修复仍在。这里只确认状态编辑，不验收图标和布局 |
| 两总线数字输入 | uU 明确挂载 SU：maxLength=3、0–100、stepValue=1、allowDecimals=false，调用与滑块相同的 P | `audio_products.rs` 的 slider 分支只有格式化数值文字与 Slider，没有数字输入实体/回调。**直接数字编辑缺失** |
| Playback 输出设备 | T 按名称匹配 P；未找到时追加 disabled 的已存项并显示 warning，选择调 changePlaybackMixDevice | 本地页无该 select、playbackDevices 观察或离线显示路径，不能由已有总线音量替代 |
| 输入通道显示/删除 | h 排除 FU 中固定物理通道，b/W 只显示 isVisible；每个 CU 删除回调 r(E) | 本地只有三组 section，无 inputChannels owner、通道卡片或删除入口 |
| 添加输入 | 隐藏类别 V 调 z 标 isVisible；外部输入按可用设备、已显示 W 和 Aux 空位计算 Y/w，再由 K/z 添加 | 没有对应列表与新增动作；不得仅把 profile.otherCategories 静态目录画出便宣称完成 |
| 外部设备切换 | 每个外部通道按 exterInputDevice 匹配 recordDevices；离线追加 disabled 项，选中不同项才 changeExternalInputDevice | 无选择器或相关草稿状态；实际设备列表需真实观察来源 |
| 通道双推子与 Link | uU 改音量保留两边差值；越界条件下拒绝该次动作。静音 p/M 只改本侧 | 本页未挂载通道，所以尚不可验收。本地通用 otherCategories 代码直接复制对侧同字段，不能直接复用于此源行为：例如 40/70 链接后前者改 50，源为 50/80；静音也不应跟随复制。此为接入风险，不计作当前可达产品交互缺陷 |
| Preset shortcut | 根将 shortcutMapping 交给 gU，updateMappingToButton 调 changeStreamMixerPresetShortcutMapping | 无快捷键状态/入口；与 B 的 preset 增删改一起未接入，设备 Profile 菜单不覆盖它 |
| Windows 音量选项与电平 | 根挂载 openWindowProperties 入口及两个总线/各输入的 OU 电平观察 | 本地无该操作入口和电平观察 UI；本批未追到系统调用或电平消息的底层协议，不能自行补写调用 |

**P2：初始状态取错 owner。** connect 明确从 `streamMixerReducer` 取主开关、总线、preset、通道等；Wn 初始化主开关 false、两总线 isEnabled=false/value=0，列表为空。当前生成器 `prepare-audio-products.cjs:173–180` 优先把 profile 的主开关 true、总线 true/50 复制到 `device.streamMixerSettings`，因此本地页的首次默认值来自另一 owner。这不是两产品当前挂载根的 reducer 初态。后续真实消息可以更新源状态，但本地既无该观察链，也不能把历史 profile 默认值当作本次读取成功。修复应采用已确认的 reducer 状态边界，保留本地草稿恢复与真实观察的区别；不要简单把所有已存用户值改成零。

本地编辑与保存链已窄查：`audio_products.rs` 的 toggle/Slider Change → `edit()`（重检 enabled）→ `sync()` → `AudioProductChanged`；`source_workspace.rs:432` 捕获完整 snapshot，`capture()` 按设备字段规则拆分，再写入设备/活动 profile 的本地模型。Shell 收到 Changed 只同步目录并通知，不立即落盘；用户本机保存走 `save_profiles()` → 排队 `PreparedSave` → `store::write_workspace`，有写入错误分支。Audio restore 先用当前 descriptor 初始化，再 merge_known 已存内容并 sync。故已有六个控件的局部变更具备本机保存链，但缺失的 preset/inputChannels/shortcut 根本没有对应状态，不能宣称它们也能保存。此链没有调用 Stream Mixer DLL 写回，本批也未做文件往返运行验证。

本批将数字编辑、冲突开启确认、输出/输入设备 UI、输入通道、preset shortcut 及错误初态归属补入待完成清单。B 的预设缺失仍成立；不重复计数为新的完整产品数量。运行布局、快捷键 gU 内部编辑细节、SU 键盘提交时序及底层实时观察协议仍需后续逐项复核。

### 3334/3337 初态修复独立回读与数字输入候选

已回读主线程最新生成器、两产品 JSON、当前源与 Audio restore/merge_known。上述“初始状态取错 owner”已在当前新建默认状态修复，不再计为未修缺陷；其余第二批 UI 缺项不因此获得验收。

- 生成器仅在 3334/3337 的 STREAM_MIXER_HEADER 分支查 Wn，并验证源 SHA-256、原始切片及 yU selector 中四字段确实指向同一参数的 streamMixerReducer；从当前 literal 读取值，而非另写 false/0 常量。此步骤先于 profile fallback，后者新增 `!draft.device.streamMixerSettings`，不会再次覆盖。
- 两份生成数据的 device.streamMixerSettings 均为主开关 false、监听 true、两总线 isEnabled=false/value=0；profile 中原始 true/50 保留。当前页面控件路径仍在 device.streamMixerSettings，不会读到保留的 profile 影子默认值。
- 新增四条 native-coverage 收据已逐条与当前源比较一致：3334 Wn literal `[4027900,4028382)`、selector `[4502958,4503693)`；3337 为 `[4027860,4028342)`、`[4502918,4503653)`。这里的范围是表达式，和前批含 Wn/yU 声明前缀的范围不同。
- restore 仍先 clone 当前 spec，再 merge_known 相同路径与类型的保存值；没有将已有本地 true/50 草稿强制迁成零。无保存值时采用修正初态，有已存草稿时按原本本地恢复规则保留。没有引入真实读取成功标志或 DLL 写回。

数字直接编辑的后续候选已定位：3334 `class AU` 起点 `4475162`，`SU=AU` 别名起点 `4480954`；3337 分别 `4475122`、`4480914`。均为本节已验指纹的当前主包。uU 的两处 SU 挂载参数和 P 回调见第二批 uU 范围；不要从别处同名 minified 符号取实现。

本次已读 AU 的核心提交行为：文本允许空串、单独负号及整数暂存；当前挂载未传 allowLiveUpdate，普通输入只改组件局部文本。blur 解析整数，NaN→0，按 stepValue 向上取整并夹至 min/max，然后通知父层。Enter 与 Escape 都调用 blur，**Escape 不是取消回滚**。方向键及上下按钮调用增减，鼠标按住每 300ms 重复，右键不执行；数字输入点击注册滚轮处理。其 active/CSS 禁用、焦点、滚轮清理与文本状态参与加减的细节尚需在具体实现时逐项核对，不把通用 NumberInput 的默认语义当作源等价。

本轮只做静态读取与报告更新；没有运行生成器、应用、构建、测试、DLL 或文件往返验证。

### 3334/3337 SU/AU 数字输入实施前细查

再次只读当前 AU、uU 及两产品 `main.24a0913d.css`。CSS 两份 SHA-256 同为 `b517361ef6c609ce7e88b76eac96123e0de9a585926c21220d66da0fd589f2ac`。以下先写 3334 UTF-16 位置，3337 同 JS 项均减 40；CSS 两产品位置相同。此段是实施依据，不是新增控件已完成的验收。

| 项目 | 当前源条件与不可省略的细节 |
| --- | --- |
| uU → SU props | 两侧分别 active=对应 isEnabled、value=对应 value、maxLength=3、stepValue=1、maxValue=100、minValue=0、allowDecimals=false；未传 dataset、unit、prefix、allowLiveUpdate。setParentState 包装在 4490401 / 4491571，只有一个形参 `e=>P(side,e)` |
| 回调第二参数 | AU sendToParent 会调用 setParentState(value,isRegisterEvent)，但上述包装丢弃第二参数；P 再检查主开关 C 并更新对应总线值。不得将第二参数误当“仅拖动/仅释放时才提交”的契约 |
| 文本接收 | handleChange 4475720 允许空串、单独负号或整数正则 `^-?\d+$`；普通键入只改局部 string，不立刻更新父值。非法文本拒绝，不是自动删除全部非法字符 |
| 动态长度 | render 4479811：按当前局部 `a<0` 判断 maxLength 是 4 或 3，不是按是否包含负号。空串或单独“-”不满足小于零，仍为3；“-1”等数字负串才扩成4。这里原挂载没有 prefix/unit 的额外长度 |
| 加减的 JS 类型语义 | volumeUp 4477577 在 allowDecimals=false 时直接 `e+1`，然后 parseInput；局部 e 经键入成为 string，所以“5”加一次是51，“50”加一次是501再夹100。初始/父同步的 numeric 5 加一次才是6。volumeDown 4477948 使用 `e-1`，会进行数值转换，故“5”减一次为4。不能统一先 parse 文本再加减后宣称复刻 |
| 提交与规范化 | parseInput 4476372 用 parseInt，NaN→0，按 step 向上取整并夹0–100；blur 4476674 先解除滚轮注册，将局部 state 改成规范化 number，设 isKeyInput 再通知父层。Enter 和 Escape（4477426）均 blur 提交。方向键不先 blur |
| 父值回流 | componentDidUpdate 只有 props.value 改变或 isKeyInput 为真才同步局部 value；步进 sendToParent 本身不 setState。若父值因夹边界而未变，不应假定局部文本一定随步进规范化；blur 路径有不同的 isKeyInput 行为 |
| 步进按钮 | mouseDown 立即一次，清旧 interval 后每300ms重复（4478370）；mouseUp/mouseLeave 停止，右键 preventDefault 不步进。卸载清 interval。spinner 的边界禁用使用严格 `a===max/min`，暂存 string 与 numeric 边界不等同 |
| 滚轮 | 点击输入 focus/select 后在 window 注册 mousewheel；先用当前 props.value 检查边界，再调用同一加减。blur 解除监听。此为窗口级监听，局部 GPUI ScrollWheel 不能无说明地宣称完全等价 |

CSS 最终级联已核对，不能只采用首段 stepper 规则：

- `[207127,...]` 通用规则给 stepper 62×26、input 38×24/14px、spinner 14×12；箭头分别引用 `stepper_up.dcb04520.svg` 和 `stepper_down.349f755c.svg`，右侧、顶部/底部定位。禁用 opacity=.3、pointer-events:none。
- `430342` 起后段改 stepper 为 position:relative、灰边；`430394` 起 input left:6px、padding:0、text-align:left；`430444` 起 spinner 常显，不能沿用早期默认隐藏/hover才显。
- `441701` 附近 level-input 容器为62×26、margin-left:10px；`441811` 的 `.component-container .adjust-container .adjust-level-container .level-input .stepper` 灰边比通用 `.stepper:hover/:focus-within` 更具体，因此此挂载下边框仍为 #5d5d5d。按全局绿色 hover 边框实现会忽略最终覆盖。

剩余验收边界仍包括窗口级滚轮、焦点与选择范围、鼠标按住跨窗口状态、完整 uU 行和两总线布局；主线程补入局部专用输入子实体后应逐项回读其文本/数值状态与保存链，不能仅凭存在 NumberInput 或步进按钮结案。

### 3334/3337 数字输入首轮实现独立回读

已只读 `stream_mixer_number.rs`、AudioProductWorkspace 接入、审计脚本与收据，并追读 Cargo.lock 对应 gpui-base 0.7.1 的 NumberInput、InputState、Button 和 mask。未执行应用、构建、测试、供应商脚本或 DLL。

- 专用子实体仅挂两个产品的两条总线 value 路径；普通文字有局部 typed 状态、整数/负号及动态长度验证。typed 加号拼接、减号数值转换、blur 夹0–100并转回数字状态已实现；父值确实变化才 observe 回填，父值不变时保留键入文本。InputState::set_value 明确不发 Change，因此程序回填不会因事件排队误变成 typed。
- NumberInput 自带动作转交自定义 on_step，未使用它的默认数值步进；其默认 number mask 在 separator=None 时不会重写该整数文本。Enter 订阅和外层 Enter/Escape 均归到 blur，未将 Escape 写成回滚。鼠标点击先在 down 步进并启动300ms重复，on_step 通过鼠标点击标记抑制对应 click 再次步进；keyboard Step 不设鼠标标记。
- enabled 同时取当前页、master 与总线门，step/blur 均检查，父 edit 仍二次检查。disable、页面变更、restore 会停止 repeat；页面/restore 清局部文本并重置注册状态，离页后的子事件还在父订阅检查页名。失活窗口在下一次重复 tick 停止；mouseUp/out、hover leave 均取消。窗口级 mousewheel 尚未接入，现为控件范围事件。
- 初值/observe 用 as_f64 再取整数，可消费 normalized 生成的50.0等 JSON 数值；不会因为 as_i64 失败显示成0。父 edit→sync→snapshot→capture 的既有本地保存链保留，未新增设备写接口。
- `stream-mixer-number-current-evidence.json` 六条 AST 收据的 hash/UTF-16 切片重新逐项匹配，四项源/共享 SVG 逐字节相等，两个 CSS hash 相符。审计脚本只解析当前源码、扫描CSS和比较资源，没有导入或执行供应商模块。该收据不代表窗口像素验收。

首轮发现一项刷新风险已发主线程：InputEvent::Change 改 typed/max_len 后未显式 cx.notify；typed 又在父 render 中被捕获用于 spinner limit，输入子实体刷新不能作为父闭包即时更新的保证。需要在此分支通知 MixerNumber 重绘，避免原 numeric100 的禁用增量闭包残留到新键入文本。此项修复回读前不宣称首轮完全通过。

布局仍是既有通用总线行中嵌入专用数字框，不等同完整 uU 行；窗口级滚轮、焦点选择与实际鼠标/键盘运行验收仍 partial。禁用时 blur 放弃未提交文本是本地保护规则，不能据此推称逐事件重现源CSS禁用行为。

## C. P2：键盘 716 的 Gaming Mode 漏掉 Copilot 禁用状态行

- 当前源 `.ref/devices/716/static/js/main.a149c89a.js`：UTF-16 `[7000600,7000890)` 从 `buttonList` 查找 `DKM_D2` 或 `DKM_F6` 设置 `T`；`7001923` 开始为 `id:"copilotKey",disabled:!0,active:o,onCheck:()=>{},name:ye.kzF`。源默认按钮表在 `646216` 含 `inputID:"DKM_D2"`。本地 716 键表也确实包含它。
- 挂载并非仅共享死代码：`vL` 是该 Gaming Mode 的 connect 包装，当前 Customize 在 `[7097350,7097650)` 的 `"macro"!==this.props.displayMode` 分支直接挂载 `(0,hn.jsx)(vL,{})`。
- 实现 `src/features/keyboard_products.rs:779` 的 `gaming_mode()` 仅渲染 Windows、Alt-Tab、可选 Alt-F4 与游戏内开关；未渲染 Menu/Copilot 条件行。`:821` 的 Windows 禁用行不能代替 Copilot 行。
- 影响：716 在 Gaming Mode 面板缺少源已挂载的设备特有状态，不能因为这是不可点击复选框而省略。
- 建议：从实际按钮列表分别派生 Menu 与 Copilot 条件，使用各自源语言键及源 active 状态。515 等包含 Menu 键的产品是后续逐产品复核候选；没有在本批未经完整挂载追踪就扩张确认范围。

修复续记（同日）：已回读主线程最新 `keyboard_products.rs`。产品 716 分支从 keys 查找 DKM_D2/DKM_F6，新增 `DISABLE_COPILOT_KEY` 行，checked 读取 `gamingMode.isWindowsKeyDisabled`，disabled=true。与本批已核实源条件、状态和只读行为一致，C 的该项代码缺失已修复；其他产品的 Menu/Copilot 条件仍不因此获得验收结论。

### C 的主线程修复状态：2026-10-06

已在 `KeyboardProductWorkspace::gaming_mode()` 补入 716 的 Copilot 状态行。条件仍从实际 `spec.keys` 查找 DKM_D2/DKM_F6；行始终 disabled，checked 直接读取 `gamingMode.isWindowsKeyDisabled`。开启/关闭 Gaming Mode 通过既有 set_mode 同步该字段；行自身没有写事件。本次未把 716 的源码条件推广到其他产品。

主线程再次读取当前源并解析模块 54693，确认当前入口的 `ye=a(54693)`、`kzF -> Ro -> "DISABLE_COPILOT_KEY"`；10 份本地语言文件均已包含该键。已核实本地 716 键表包含 DKM_D2。cargo fmt、cargo check --locked --all-targets 与鼠标/键盘数据静态校验通过，保留 3 项既有 Rust 警告；没有运行/视觉验收，Gaming Mode 整页的布局、样式和其他条件仍需继续核对。

## D. P2：Settings 的 Systray 双击动作仍是不可操作文字

- 当前源 `.ref/applications/settings/static/js/762.98d21f1a.chunk.js`，模块 9762 `fe`，UTF-16 `[33937,34929)`：从本地存储读取 `ve`，默认 `{type:"showMenu"}`；选项为 showMenu 加非 website 应用的 `{type:"launch",payload:e.name}`；点击打开 `.sys-icon-dropdown-selector`，选择后更新 React state 并 `c.A.set(ve,e)`。即使应用列表为空，也保留 showMenu 选项及下拉行为。
- 实现 `src/shell/settings_window.rs:133` 的 `systray()`，`:173` 起仅为一个带边框的 `div`，内容固定 `DROPDOWN_SYSTRAY_1`；没有 retained selection、下拉事件或本地保存。
- 影响：该项已显示，但 UI 修改/选择操作未接入；这不是只有 DLL 写回缺失。源码 launcher 槽位和 widgets 的实际目录需要真实读取，不能为了补 UI 伪造已安装应用。
- 建议：先接入有证据的本地选择状态、真实可用选项和下拉交互，单独标记宿主配置同步/执行边界；随后将托盘行为与同一个状态 owner 连通。

### D 修复续记：空应用目录分支已局部接入

同日已静态回读 `settings_systray_action.rs`、`preferences.rs`、`SettingsPage`、`settings_window.rs`、`shell.rs`、`tray.rs`。当前已加入 ShowMenu 选项、保留弹层状态、100ms 展开等待及高度变化、选择后的本地保存。新增字段 `systray_double_click: Option<TrayDoubleClickAction>` 对旧文件缺字段回退 ShowMenu，与 9762:fe 的空目录默认相符。

已核对保存链：选项 → `set_tray_double_click()` → `changed()`/SettingsEvent::Changed → `AppShell::save_auxiliary_preferences()` → `WorkspaceFile.with_preferences()` → `store::write_workspace()` → 成功后 `SettingsPage.mark_saved()`。写入对象是本地工作区；失败处理没有宣称宿主配置写入成功。托盘双击分支从同一 SettingsPage owner 读取 ShowMenu，取消单击等待后调用现有 show_popup。未加载 DLL。应用目录、launch 选择及宿主配置同步仍未接入，故 D 只能记录局部修复。

本批重新核对当前 `762.98d21f1a.chunk.js` 模块 6584：`u` UTF-16 `[8019,11431)`、触发器 `h` `[11536,12169)`；文件 hash 与上表同一当前源一致。还有一项可定位时序差异：

- 点击弹层外时，源 `bodyOnClick()` 调用 hideDropdown，将高度设 0，100ms 后才 onHidden；9762:fe 的 m(false) 在 onHidden 内执行。因此源触发器的 active 绿色边框与箭头状态保持到收起结束。
- 本地 `Popover.on_open_change(false)` 直接 `set_open(false)`，立即改变用于 trigger 边框和 arrow 旋转的 `self.open`；外部点击路径比源提前清除 active 状态。需要区分外部收起与选择项收起，不能一律延迟：选项点击时源 9762 本来就立即 m(false)。
- 尚未运行窗口，GPUI Popover 的外部点击、Escape、焦点恢复及布局边缘只能记为未验收，不能从该时序核对推导完整弹层一致。

## E. P2：1382 的 Audio Function 子编辑器与保存对象缺失（持续独立复核）

2026-10-06 再次核对当前主包、懒加载编辑器、Rust 与后端；没有执行应用、vendor JS、构建、测试或 DLL。以下源位置均为 UTF-16。主包 SHA-256 及 913/MapAudio 指纹见本文指纹表；本次重新计算一致。

### 可达入口与当前实现纠正

- 当前 main.275d3b94.js 的 DeviceInfo 在 34943 附近包含 isAdvancedMapAudioFunction:true；ControlPodSourceClick（130252 附近）是 isEnabled:true，默认 AUDIO_FUNCTION，functionList 含 AUDIO_FUNCTION。不是仅凭 chunk 名称猜测功能存在。当前 activeButton 编辑入口在 4687237 附近加载 1977/913/2667，913 chunk 在 17723 附近按 I.atf 加载 2321/6799 并取 8191（MapAudio）。
- main 模块 9267 的 hW（4065512）明确为 SwitchAudioEq、SwitchPlaybackDevice 两个高级分类。Qn（4065785）为 CycleUpSoundDevice、CycleDownSoundDevice、ToggleSoundDevices；Hs（4065967）为 CycleUpAudioEq、CycleDownAudioEq、SpecificAudioEq；ft（4066329）为 default/game/movie/music/custom 五种 EQ。
- **纠正此前 E 的过度表述**：当前 prepare-audio-products.cjs:421 已为每个 podMappings 预置 audioGroup（源默认对象或 CycleDownSoundDevice + SwitchPlaybackDevice）。因此切换 outputType 后并非必然没有 audioMode/audioAssignment。真正缺口是只保留这一个预置动作，不能选择/编辑/提交完整高级映射。
- 当前生成器 :422—426 只提供 outputType、Reset 与 multimediaGroup 条件行；Audio Function 被标成 SWITCH_PLAYBACK_DEVICE，没有 audioGroup 条件编辑区。audio_products.rs:242 的 SelectEvent、:474 edit 直接修改草稿；:663 render_control 的 select 只是通用 Select。:537 发 AudioProductChanged，source_workspace.rs:404 捕获整个 snapshot。该链有本地草稿保存意义，但不是源子编辑器 Save，也不是 DLL 写入。

### 子编辑器与只读数据：逐项确认的缺项

1. **分类及选项**：需两个高级音频分类，各自三种单选；SpecificAudioEq 显示五种 EQ 选择。当前只可选择 outputType=audioGroup，无法表达其余动作。不要引入 GameChat/BluetoothVolume/FootstepScaling 等另一个产品 flag 分支。
2. **Playback 列表**：MapAudio 4516 附近调用 simpleEnumerateAudioDevices，解析 deviceList JSON，筛 type=speaker 并以 name 作为 content。ToggleSoundDevices 才显示第一/第二设备选择，第二列额外有 None；源把已保存、当前枚举缺失的 playbackDevices 补回列表，标 isDisconnected 和 disabled，且显示断开说明。不存在的真实设备不能用猜测 ID 填充。源未见排除左右选同设备的规则，不得自行声称源有去重限制。
3. **EQ 可用设备**：ze 以 validDevices 中 category=AUDIO 且 subCategory 为 SPEAKER 或 SPEAKER_HEAD_CUSHION 的 deviceContainerId 集合筛选 G.containerId，得到 K 和播放 ID 列表 he。无合格音频产品显示专用红字，另有 FAQ /answers/detail/a_id/13138。本地没有相交列表、无可用提示或此分支入口。
4. **已有映射回读**：Be 依据当前 assignment/assignmentValue 匹配模式与动作；ToggleSoundDevices 读 assignmentGroup.playbackPayload 的设备 ID、已保存设备对象；枚举默认值不得覆盖已保存设备。源当前 Be 没有全面恢复 EQ payload 中所有下拉索引，不应假设它具有尚未追到的“完整 EQ hydration”。
5. **查询已有但界面未接**：2321.f952c48b.chunk.js 的 simpleEnumerateAudioDevices 经 callSimpleServiceAction 发送同名 action（8563 附近），返回非 Electron 不支持结果。文件 SHA-256 为 09240bab9c188804d8b60ff7edac21e27d57e96dc0813acab14a9b328e351ab7。本地 backend/runtime_native.rs:216 已有 ServiceRequest::AudioDevices → simpleEnumerateAudioDevices，只读能力并非完全不存在；1382 AudioProductWorkspace 尚未消费该响应。后续应接已核实契约，显式保留空/错误/离线状态，静态开发阶段不加载 DLL 验证。

### Save 对象、父层提交与取消边界

- MapAudio 的 Ee（5681 起）调用主包 rq（4053698）构造含 outputType、isHyperShift、inputType、inputID 的 mapping，再写 audioGroup，返回 mapping 数组；rq 不保留任意旧组字段。Ts（4066209）将翻译键转换为对应动作 ID。
- 基本 audioGroup 写 audioMode、audioAssignment。EQ 在 K 非空时写 equalizerPayload.selectedPlaybackId、razerAudioSpeakerIds、selectedSpeakerPID；SpecificAudioEq 额外写 specificEqValue。ToggleSoundDevices 且 G 非空时写 playbackPayload.firstDeviceId、secondDeviceId、playbackDevices 两个对象。
- 注意实际源边界：Ee 的 ToggleSoundDevices payload 条件由 E[J] 决定，并不额外检查当前模式 A 是否为 Playback。源也没有在 Ee 中统一校验空设备列表、左右重复或所有选中 ID 的有效性。实现必须先还原已确认条件，不要将理想化校验标成源行为。
- 编辑状态 A/J/R/ie/oe/ce/xe 保存在组件 state；setSaveMapRef 注册 Ee，父 913 的 enableSave（45490）区分 canSave、isMappingChanged、requireSynapse 三种状态。不同控件回调及 effect 的参数和依赖并不一致，例如 EQ 设备/预设回调只改索引；不能用“任意变化均 enable Save”替代当前源。需要在实现时逐个保留回调及 effect 时序。
- 913 saveChanges（41913）调用 saveMapRef，合并/替换 mappingList，然后 setActiveKeyMapping、setMappingList、清 dirty、清编辑器；closeMapping 的关闭链清 activeButton、映射面板和 dirty。原实现随后涉及设备状态更新的部分应统一后置，但 UI 草稿编辑、Save 合并、Cancel 丢弃及未保存提示现在就应做。本地当前 Reset 仅恢复整段默认对象，不能替代取消子编辑草稿。

### 具体实施清单与验收界限

按顺序补：专用 Audio Function 入口与临时编辑状态 → 当前只读枚举/有效产品数据适配 → Playback/EQ 控件及空/断开提示 → Ee 同构 mapping 构造 → 父层本地 Save/Cancel/dirty 流 → 接现有 capture 与本地持久化。每步注明源范围，不将 routes、默认 audioGroup、后端符号存在或本地文件落盘当作整页完成；最终 DLL 状态修改/写回/保存仍统一后置。

### 只读契约续查：Rust 已解包，不能照搬 JS 的 deviceList 层级

已读取当前 host-4.0.827 的 electron/modules/simple_service/win/index.js，以及 1382 的 2321 wrapper、MapAudio 和本地 runtime.rs/runtime_native.rs。

当前宿主 wrapper 实算 SHA-256：`7b16cb4d3a053004d0674de99390d3f3d552022406192898c3f4ff51ca889566`。

| 层级 | 实际返回与处理 |
| --- | --- |
| 原生 simpleEnumerateAudioDevices | void(callback(bool, string, string))；第三参数是设备列表 JSON 文本 |
| 当前宿主 JS wrapper | 在 UTF-16 22039 起注册该回调，22537 附近包装成 `{result:成功布尔, reason:原因, deviceList:第三参数文本}` |
| 1382 2321 → MapAudio | 2321 原样返回服务 action 结果；MapAudio 取 result.deviceList，JSON.parse 后筛 type=speaker，并以 name 作为显示 content |
| Rust NativeRuntime | runtime_native.rs:78 callback3 复制第三参数到 value；:129 call 在 success=false 时返回错误；:216—229 取 value 并 serde_json::from_str，合法数组直接成为 Value::Array，不带 result/deviceList 外壳 |
| Rust ServiceClient | runtime.rs 的 ResponseEnvelope 是内部传输封套；request 最终返回 response.data 本身（约 :210），没有再包一层 deviceList |

因此 UI 接 ServiceRequest::AudioDevices 时应直接消费成功返回的数组，不能取 value["deviceList"] 再解析。现有 NativeRuntime 解析失败会回退 Value::String；适配器应将非数组、无效结构作为格式错误，与成功空数组分开，不能静默当空设备成功。bool=false、加载错误、超时、通道错误保留为读取失败；没有 result 字段不等于失败，因为 Rust 已将其转为 Result。

接线需要保留真实端点的 id/name/type/containerId 以及原对象中后续保存需要的字段；不要仅存显示名称或从产品名生成 ID。源 Playback 可以显示所有 speaker；EQ 则还必须与官方 validDevices 的 deviceContainerId 相交。成功枚举音频端点不能独自证明某个端点支持 Razer EQ。

### 当前本地设备集合不是官方 validDevices

- Shell.devices 可从本地 WorkspaceFile 恢复；没有文件或读取失败时还使用 model::measured_devices 的固定历史快照。后者即使有原始观察背景，也不是本次真实连接/状态读取证据。
- add_preview / demo::registered_preview 从产品注册目录合成界面设备，serial_number 为 PREVIEW-{pid}、device_container_id 为 preview-{pid}，并将 setup_status 设为 Ready。Ready 与 Audio 类别因此不能独自证明它是当前真实音频产品；重新读取保存过的预览也不改变其来源。
- Shell.sync_known_devices 只传播 (product_id, edition_id)。RuntimePanel.read_services 获取 HID、版本和音频读数，但没有生成与官方 DEVICE_RUNTIME_DATA 等价的有效产品记录。HID 的 PID/路径/序列号亦不能填造 category/subCategory/设备容器对应关系。
- 现有 model::Device 没有明确的观测来源字段，也没有满足此过滤所需的音频 subCategory；不可把所有本地 Device 或所有已下载 audio_products_data 产品直接当作 MapAudio 的 validDevices。

建议独立保存只读观察状态：音频端点查询结果与官方有效产品记录分开携带结果/错误/时间及来源。官方产品记录须有真实 productId、deviceContainerId、category=AUDIO、subCategory=SPEAKER 或 SPEAKER_HEAD_CUSHION，随后按 containerId 精确相交。缺少第二类来源时，Playback 可使用真实查询端点，但 EQ 保持未取得合格设备，不虚构产品匹配。保留已保存离线端点只用于源允许的显示和草稿恢复，不将其加入真实在线产品集合。

### 1382 Playback 首批实现独立回读

已读取新增 control_pod_audio.rs 及 AudioProductWorkspace 挂载/restore、SourceProductWorkspace capture 链。此前 E 的“无子编辑器”现在仅代表初始发现，当前已具备下列局部功能；EQ 不冒用本地目录作为真实 validDevices。

- 仅 1382、已选 outputType=audioGroup 且源输入目录包含的行显示编辑器入口；打开时再次检查当前控件 enabled。编辑器独立保存模式/动作/设备 ID 与设备对象，Cancel 不调用父 edit。
- Rust 枚举结果直接按数组解析，没有错误读取 JS.deviceList 外层；失败有 read_error，读取中有 loading。speaker 数据保留原对象并添加 content=name；未保存首设备时取真实列表首项。离线已存对象被补回且 disabled；第二设备额外 None。select_device 再拒绝 disabled 对象，不仅依赖下拉视觉禁用。
- Save 生成 source-shaped mapping，父级只把 audioGroup 写入同一 podMappings，以保留其他本地类别草稿；随后 AudioProductChanged → capture → active profile.source_settings。restore_pod_audio 在 generic merge_known 后恢复可选 playbackPayload/EQ payload，避免只恢复默认模式/动作而丢失高级字段。没有将此映射发送 DLL。
- restore 与切页会清 editor 和 subscription；重新打开替换订阅。枚举任务使用所属 editor 弱实体 update_in，关闭后返回不会写入新 editor；任务只更新局部设备列表，不自动写父草稿。没有发现这条读取回调跨 profile 保存的路径。

**本次发现、后续已静态修复的缺陷**：audio_products.rs 的 edit 仅在 path 等于 editor_path（以 /outputType 结尾）时清 editor。同行 Reset 实际写 /profile/podMappings/{id} 整段基路径，因此打开子编辑器后 Reset 不销毁旧 editor/subscription。旧 Save 回调不核当前 outputType、enabled 或 original baseline，可以把打开时旧 audioGroup 写回已经重置的映射。应覆盖修改祖先路径/同一映射的失效条件，并在 Save 重检目标身份与当前值；不能只依赖切页/restore 的清理。

另一个窄边界差异：源枚举后判断 `!payload.firstDeviceId && !ie`，包含 null、空串、0 等假值；当前 observe_speakers 用字段不存在且 first_id==0，仅覆盖缺字段。既有 firstDeviceId 为 null/空串/0 时源会择首设备，本地不会。此为异常/边界存储值的静态差异，不将其推广为正常非空设备 ID 回读失败。

修复后独立回读：上述 Reset 与首设备假值两项现均已修复，不再记当前未修缺陷。

- 最新 edit 先检查 enabled，再以映射基路径判断同路径、祖先、后代修改并清 editor/subscription；Reset 写整段基路径被覆盖，等值 Reset 也通知重绘，旧编辑器不会留在画面。
- Save 订阅先比对当前 editor.entity_id 与事件 emitter，再检查当前 outputType=audioGroup、当前控件 enabled，以及 group_path 当前值仍等于打开时 baseline，全部通过才写。旧实体、已替换目标或已改变草稿不会回写；同一映射 Save 自身触发清理不影响已通过检查的本次提交。
- 最新 truthy 对 null/false/0/空串返回 false；首设备默认条件与源两个假值判断对应。第二设备假值回退 None，离线补回也要求 ID 为真值。
- 初始 payload hydration 仅在 playback 动作为 ToggleSoundDevices 时发生，与 Be 的对应分支一致；其他动作不误带入此前存储的切换设备状态。枚举所需的原始已保存对象仍由 original 读取。

本轮只回读这些修复及其相邻调用链，没有运行应用或 DLL，也没有将 EQ、布局及未保存切换提示升级为完成。


当前 Save 仍沿用 Ee 的 J/G 条件：列表为空或读取失败时 Toggle payload 可能省略，这是源本身已记录的条件，不能把它自动报告成新实现独有的数据丢失。全量 source canSave/effect 时序、父层未保存切换提示、EQ 真设备列表和精确控件布局仍 partial。本轮未执行应用、服务worker、DLL、构建或测试。

### 1382 父未保存提示：三个动作并不共用“丢弃后继续”逻辑

本次只追当前 main.275d3b94.js 的 oL/IL 根和 913.3b7aa5fd.chunk.js 的 $t，未修改实现。指纹仍同上；下面位置为 UTF-16。不能套用常规 Save/Discard/Cancel 对话框惯例。

父 oL 的 displaySaveAlert（4682452）签名为 `(nextAction, targetDom, dontSaveAction)`，打开提示并分别记录三个值；setKey（4666081）仅更新定位用 mappingCurrentKey，不等于选择新输入。componentDidMount（4688158）把此 displaySaveAlert 注册给上层 IL。renderSaveAlert（4684687）直接绑定三个动作，没有以子 canSave 关闭父 Save 按钮。

| 提示动作 | 当前源调用 | 对 dirty/编辑内容/目标的影响 |
| --- | --- | --- |
| Save | 父先 setKey(tmpRef)、关提示，再 await saveMapRef(true)，最后 nextAction() | 913 saveMapping(true) 调实际 saveChanges，合并映射并 clear；再执行第一个目标回调。父提示不按子 canSave 过滤；913 saveChanges 只有其自身 TwoTap 无效选择保护。 |
| Don't Save | 父关提示，await saveMapRef(false)，只调 dontSaveAction() | 913 不运行 saveChanges/clear；设置本地 canSave=true、isMappingChanged=false，并 updateMappingChanged(false)。若调用者未提供第三回调，目标不切换，编辑器及其未提交子状态继续保留。 |
| 提示右上 × | dismissSave(true) 关提示，await saveMapRef(false,true)，不调两个目标回调 | 913 设置本地 canSave=true、isMappingChanged=false，但第二参数 true 抑制 updateMappingChanged(false)，所以外层 global dirty 保持。MapAudio 子编辑状态没有被丢弃。另有 isConfigureSensitivityStages 专用状态复位分支，不应无条件套到普通音频映射。 |

913 closeMapping（35986）与 saveMapping（36558）是上述区别的直接证据。clear 会清 activeButton、关闭映射面板并清本地/全局 dirty；saveMapping(false) 不调用 clear。即使局部 dirty 被设 false，MapAudio A/J/设备选择仍存在，不能以 dirty 标志反推编辑值已经恢复默认。

具体触发入口：

- **换产品输入按钮**：当前 clickBtn（4659019）在 global dirty 且已有 activeButton 时调用 displaySaveAlert(updateActiveButton, targetDom)，没有第三回调。因此第一次 Don't Save 不切到所点按钮；再点击时因 global dirty 已清，才走普通切换。Save 提交后执行 updateActiveButton；×仍保留外层 dirty，下一次切换可再次提示。
- **切页及历史导航**：IL.changeView（4694846）只传 updateView；navigateBack/Forward（4695577/4695843）只传 updateNavigationView。对应提交函数在 4695067/4695941 更新 active_view 和 selectedTab。故 Don't Save 也不立即切页/前后跳转，仅清 global dirty；不能强行在“不保存”后消费第一个待执行目标。
- **映射面板右上 ×或一般外部点击**：913 closeMapping(false) 脏时调用 displaySaveAlert(clear)，仅第一个回调。Save 后 clear；Don't Save 留面板及子编辑状态，仅清 global dirty；提示 ×也留面板，并保留 global dirty。外部点击并非所有目标均触发：windowClick 有 config-btn/svg-key/save-close 等排除列表和 raw-text option 排除，不应直接实现成无差别失焦即关。
- **子编辑器底部 Cancel**：913 65462 附近直接 closeMapping(true)，立即 clear，不先弹未保存提示；这是另一条行为，不是父提示 ×。
- **刷新图标例外**：913 windowClick 在 33952 附近对 icon-refresh 特意传 displaySaveAlert(t,null,t)，其中 t 为 clear 后延迟 500ms reload。它有第三回调，所以此处 Don't Save 与 Save 均执行后续刷新；这证明必须保留两个独立 continuation，而不是用一个统一目标回调。
- **展开/收起输入面板**：主包 toggleButtonPanel（4659722）也仅传 updateToggleButtonPanel；Don't Save 不自动执行面板切换。

实现边界：区分子编辑状态、本地 canSave/dirty 与外层 global dirty；存储 Save 后回调和 Don't Save 后回调两个槽。Don't Save 不应一律丢弃子草稿或消费 Save 回调；提示 ×不应清 global dirty；子 Cancel 则是明确立即丢弃。父 Save 走其实际提交回调，不默认复用子 Save 按钮的可用门。仅最终设备写入后置，本地映射合并及上述状态转换仍在当前 UI 范围。尚未将所有其它产品/映射类型的特殊调用者推广到 1382。

### 编辑器自身 × 的未保存提示：新增实现窄复核

已静态读取最新 control_pod_audio_warning.rs、control_pod_audio.rs 及现存父 Save guard；没有执行应用、构建、测试或 DLL。此结论仅覆盖编辑器自身 ×，不代表其它待离开操作已经统一接入。

- request_close 在 changed=false 时发 Cancel 直接关闭，changed=true 才新建 WarningState 并聚焦提示；对应 913 closeMapping(false) 的脏状态分支。
- Don't Save 设 can_save=true、changed=false，仅隐藏 warning 并恢复原焦点，不销毁 editor、不覆盖其 Playback 状态；对应没有第三 continuation 的 closeMapping(false)。此后再次点编辑器 × 会因 changed=false 直接关闭，符合源外层 dirty 已清后的路径。
- 提示自身 ×只设 can_save=true 并隐藏 warning，保留 changed；不会错误触发 Cancel 或待离开动作。当前没有独立模拟 913 内部 isMappingChanged 的局部标志，但这条路径的外层保护状态与草稿保存行为已对应。
- 提示 Save 不读普通 can_save，直接发 mapping 的 Save 事件；父订阅仍有当前实体、outputType、enabled、baseline 安全校验，正常目标通过后只写本地 audioGroup 并关闭 editor。子编辑器底部 Cancel 仍直接发 Cancel，不强制经过提示。
- 已对照当前 main.1ea893bf.css：面板 400px、20px/30px 内距、5px 圆角、绿色边框、标题/正文行高、20px 关闭图标与8px偏移有对应。源 save-alert 为 fixed top:50% + translateY(-100%)；本地 popup bottom:50% 使底边落在 viewport 中线，不是把面板中心放在中线，静态定位对应。显现使用100ms线性opacity，按钮hover/press与300ms透明度过渡也有对应。
- 源 backdrop 的定位/百分比高度依赖其DOM包含块；本地 Base Dialog 使用viewport门户及focus_trap。未做真实遮挡范围、焦点和窗口像素验收，不将参数对应升级为完全视觉一致。

未发现本批自身 × 提示的明确功能阻碍。该批次尚未接入换输入、切页和导航前后；其后续状态见下方「音频输入切换及产品内部导航保护」。折叠面板及外部点击排除表仍未完整接入；不得用 warning 组件存在证明所有父未保存流程都完成。

### 音频输入切换及产品内部导航保护：最新窄复核

已回读最新 Departure、open_pod_audio、defer_pod_navigation、SourceProductWorkspace 的 set_page/step_page_history 及订阅。未修改实现或执行应用、构建、测试、DLL。

- 打开另一现有 audioGroup 输入前先 request_departure(Input(path))；同一输入直接返回。目的路径必须仍是1382源目录中的可编辑音频输入；完成 Save 后重新调用 open_pod_audio，也会重新检查目标 outputType/启用状态。此范围不是所有通用输入或所有 assignment 类型。
- WarningState 保存一个 Departure。提示 Save 发 SaveThen；Don't Save 与提示 ×只隐藏提示，不发目标事件，分别清/保留 changed；因此不会误把“不保存”当作立刻切换目标。已有 warning 不被重复请求替换，暂存目标不会在同一提示内被无关请求覆盖。
- 父订阅核对 emitter identity、当前 outputType/enabled 及原始 baseline，通过后才写原输入并取出 Departure；校验失败不执行目标。随后清旧 editor/subscription，再打开目标输入或发 AudioNavigation。旧 editor 的迟到事件不能把新 editor 当保存目标。
- set_page 在合法页和同页检查后、truncate/push/page赋值前调用 defer；step_page_history 在can_step_history检查后、索引移动前调用defer。因此提示出现时页与历史保持原值；Don't Save/×不修改它们。无dirty导航先清旧 editor，再由原函数完成切换。
- History 保存的是前进/后退请求，续执行仍调用 step_page_history，只移动历史索引，没有误走 set_page 再追加一条历史。普通 Page 续执行才进入正常截断/追加路径。未发现本轮导致重复新增历史的调用链。
- 已核 Cargo.lock 实际锁定 gpui-pre 0.3.8；该库 Context::emit 向 pending_effects push_back，flush_effects 按 pop_front 处理。成功编辑先排入 AudioProductChanged，之后才排入 AudioNavigation，故同一队列内父 capture 先于续导航；不是依赖同步事件回调的猜测。等值保存不发 Changed，但父现有快照已包含同值，仍可安全续导航。

本轮已覆盖的输入切换/产品内页及历史保护未发现明确功能阻碍。**主 Shell 的换设备、换应用、全局历史、profile操作、其它assignment编辑器、折叠面板与任意外部点击不在此结论范围**；不能将本次局部导航保护写成全部未保存流程完成。

### EQ 条件及 Save 时序：可直接实施的源规则

本轮重新读取当前 MapAudio chunk，范围为 ze、[w] effect、EQ 回调、Ee；不以缺少当前实机发布链阻止实现这些已确定的纯状态规则。位置均为 UTF-16。

1. **w 与 K 分开建模**：ze（5392）从 validDevices 中筛 AUDIO + SPEAKER/SPEAKER_HEAD_CUSHION，取 deviceContainerId 数组 i；w 的目标值 t 仅为 i.length>0。K 为 G.filter(endpoint=>i.includes(endpoint.containerId))，he 为 K 的 id 数组。故 w=true/K为空是合法可达组合，不得把 w 改成 K 非空。源按 w 控制显示EQ控件还是不可用提示，空K时仍可显示动作与预设选择。
2. **最终 Save 门与中间回调**：[w] effect（5290）在 k且A==0时调用 enableSave(w,true,false)。ze 发现 t!==旧w时先 setW(t)，若A==0还立即调用 enableSave(!t,true,false)；随后 w 更新触发 effect，最终覆盖为 enableSave(t,true,false)。因此EQ模式下w从false→true：先canSave=false后true；true→false：先true后false。两轮都标dirty=true，requireSynapse=false。不能把ze的反值当稳定状态，也不应删除这一顺序后声称逐回调相同。
3. **依赖范围照源记录**：ze的effect依赖只有 validDevices.length/G.length；[w] effect只依赖w，不随A单独改变重跑。同长度替换不自动触发ze，K/he的同长内容变化也不必然重注册保存closure。这是当前源限制，若采用更完整观测刷新需注明为本地额外处理，不能冒充已证实源行为。
4. **分类选择读旧A**：7864回调先setA(next)，再按 `!k || w || oldA==0` 设置canSave并标dirty。高级分支w=false时，Playback→EQ得到false，EQ→Playback得到true。分类切换本身不会触发仅依赖[w]的effect，不能按新A重新推导覆盖结果。
5. **EQ三种回调不设dirty/enableSave**：Fe（7691）仅setR；Ce（7704）仅setCe；we（7718）仅以content查P索引后setXe。不要给它们套普通Playback的enableSave(true,true,...)。但3212的setSaveMapRef effect依赖R/ce/xe，故这些变化会更新保存函数所见索引，并非“不参与保存”。

Ee（5681）的边界：

- 选中动作不存在或content是假值时，audioGroup返回空对象；外层仍返回一个mapping数组，而不是空mapping数组。
- 有有效EQ动作、K为空时，仍有audioMode/audioAssignment，但不生成equalizerPayload。w可为true且保存门可为true，不能自行把它解释为禁用Save。
- K非空时取K[R]的id/containerId，并在当前validDevices查对应deviceContainerId；selectedSpeakerPID直接读找到项的productId，没有optional chaining。若R越界或对应产品记录消失，通常会在此处抛TypeError，而不是自动回退首项或返回成功。源没有在K缩短时清R的逻辑。
- SpecificAudioEq取N[ce]?.id；越界时JS值undefined，JSON序列化会省略该字段，不会写null或假造default。
- 本地应对无有效选中端点/产品对应项返回明确保存错误并保留编辑内容，防止Rust索引panic或错误成功；这是将源失败情形转成可见错误，不能自动选择另一个设备。合法源路径的mapping结构仍按Ee构造。
- Ee 的Playback payload仍独立按J/G判断，不能因为当前是EQ就无条件删掉该分支。不要将现有源码的模式、缓存索引和保存闭包简化成单一“有效设备即保存成功”状态。

以上是实现规则与失败边界，不宣称真实EQ设备已读到，也不将它作为继续实现UI的阻断。未执行供应商脚本、DLL或测试。

### EQ 条件控件与本地保存：最新实现窄复核

已独立静态回读最新 `control_pod_audio.rs`、`control_pod_audio_warning.rs`、`audio_products.rs` 与 `source_workspace.rs`；包含 `RuntimeAudioDevice` 改为保留原记录字段的版本。没有运行应用、供应商脚本、构建、测试或 DLL。本段更新此前 EQ 尚无控件的历史状态，仅确认下述实现范围。

- 观察入口只接收独立的运行时记录，初始为空；未用 Shell 目录、预览产品或已存 profile 填充。外层入口明确标注当前 host 发布者尚未连接。记录保留可选原字段，筛选只认 AUDIO + SPEAKER/SPEAKER_HEAD_CUSHION；投影依赖完整 validDevices 长度及 G 长度。w 取合格产品是否存在，K 单独取 containerId 交集；同长度替换保留源的投影限制。
- EQ 面板按 w 显示设备选择、三项 EQ 动作与 SpecificAudioEq 条件下五项预设；w=false 显示源不可用说明，兼容性链接始终存在。w=true/K为空没有被错误改成不可用。R 不因列表缩短被自动重置。
- 分类回调依据旧 A/w 设置 canSave；EQ 设备、动作、预设回调仅更新索引。初始 EQ 的 [w] 效应标 dirty 并关闭 Save；合格设备存在状态改变后，采用源两阶段回调的最终一致状态。这里没有逐帧重现中间反值，不能声称每个 React 回调时刻与原生渲染完全相同。
- mapping 在 K 非空时取当前 R 的端点及对应运行时记录，保存 selectedPlaybackId、selectedSpeakerPID、完整 K 的 ID 与条件预设；缺失可选字段不伪造值。K 为空时省略 equalizerPayload；Playback payload 仍独立按 J/G 条件构造。普通 Save 与未保存提示 Save 均处理无效 R/缺对应记录的错误，保留编辑内容，显示错误，不发 Save/SaveThen 或导航事件。此可见错误属于对源异常路径的本地安全处理。

本批未发现上述条件路径的新阻断缺陷。仍未验收真实 validDevices 发布链、设备实际读取结果、全部布局及辅助功能，也未声明重现源保存闭包全部缓存时序。本地 Save 仍仅进入现有草稿合并与本地持久化；DLL 状态修改、写回与保存继续后置。

## F. P2：鼠标 70 的 DPI stages 关闭条件与多行编辑流未还原

仅确认产品 70（Razer Mamba TE）当前 Performance 页；没有把共用 Rust 渲染器的所有鼠标一并判定。

- 当前 `.ref/devices/70/static/js/main.8f24b6a1.js`，`cI` 从 UTF-16 `4583066` 开始，`uI` connect 包装从 `4588100` 开始；Performance 组件 `zI` 在 `4595678` 直接挂载 `(0,St.jsx)(uI,{})`，没有传 useTwoWayTab。
- `cI.render()` 对列表传入 `dpiStages:this.props.dpiStagesOn?this.props.dpiStages:[this.props.dpiStages[this.props.activeStage-1]]`。因此关闭 stages 后，只显示当前阶段的一行；不是把所有阶段保留为可切换按钮。打开时 aI/EI 渲染每阶段的 DPI 编辑行及拖动控制，EI 的 onDrop 会同时修正排序和 activeStage。
- 实现 `src/features/mouse_products.rs:550` 的 `performance()` 不读取 stage_enable 值来控制阶段按钮或编辑行：关闭后 `0..count` 仍生成所有按钮，点击仍 `write(active_path, slot+1)`。开启时只渲染当前选中阶段的输入与滑块，没有原版的每阶段多行和拖排入口。
- 建议：按当前产品源补阶段开关条件、每行可见/激活状态、拖排及 activeStage 重映射，分别维护本地编辑和最终设备提交。不要简单把整个 DPI 面板禁用，源在 stages 关闭时仍允许编辑当前 DPI。

### 本项已静态核对一致的窄范围

源 `cI.changeDpiValueX` 在 independent 为 false 时同时更新 x/y；`toggleY` 关闭独立 Y 且 x!=y 时令 y=x。本地 `mouse_products.rs:253` 的 `write_number()` 和 `:535` 的 toggle 回调保留了这两条数据更新语义。这里只确认计算/调用路径一致；没有运行窗口验证，也不证明 F 的整体列表、范围样式或其他鼠标正确。

## 本批排除的误报与状态口径

- `mouse_products.rs` 有 TAB_PAIRING 占位，`keyboard_products_data.json` 有若干 TAB_PAIRING 字符串，不能直接据此认定当前主导航漏页。本批检查的 600、697、716 主导航记录均不含 TAB_PAIRING；覆盖生成器也明确排除了鼠标占位分支。要检查独立模式，必须另追其挂载入口。
- 当前 `native-product-coverage.json` 把产品标为 partial，并明确完整复刻数量为 0。未发现它把本批产品标成完整完成。A–D 是 partial 实现内部的实际差异，不应以“已有路由”消去这些缺口。
- 旧 `tray-current-audit.md` 曾说 Settings 只有本地设置页；当前 `settings_window.rs` 已有独立根。这是历史记录，不能再照抄为当前实现状态。

## 当前源指纹

| 源文件 | 本批重新计算 SHA-256 |
| --- | --- |
| `.ref/devices/3334/static/js/main.496bb585.js` | `320597b62d8538c929ca38bbf594297f7151582c0ac61b3224ba86faa0d7eeca` |
| `.ref/devices/3337/static/js/main.a048e901.js` | `835ff3af59d774b85c02bfe97fae62e45ec0921218884fcc6f38224c4103b0b3` |
| `.ref/devices/716/static/js/main.a149c89a.js` | `c4d563c8ed9d05de9268905fad8e600a7bd8710c3340fe774d78f980fc7d3de7` |
| `.ref/applications/settings/static/js/762.98d21f1a.chunk.js` | `dc81bdfc01d7a37fcede73dd345db0220ae040b9dba3c2ca81e5a81f1ebebcf1` |
| `.ref/devices/1382/static/js/main.275d3b94.js` | `75b22375449ce8e6706b85587273b9ad4663851f7c6da4a33a9decdd2f26732b` |
| `.ref/devices/515/static/js/main.f60ca5aa.js` | `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8` |
| `.ref/devices/697/static/js/main.9db50ae0.js` | `575eefa164cb9555083e91a29ccdfa588cc55f2c275ebfa1651591fb8e56d502` |
| `.ref/devices/1382/static/js/913.3b7aa5fd.chunk.js` | `56904c79bcabedba2a722c2920acba16e6ae207336781d24a028dfc9c6145423` |
| `.ref/devices/1382/static/js/MapAudio.d46d2ed7.chunk.js` | `ad829ab689db2656680951324cfe42c7e1e422e40121ff507f4caab471533409` |
| `.ref/devices/70/static/js/main.8f24b6a1.js` | `be0816be63fe674ea9fc106cbd55f5aabd509e1ff36aaf66e6c002d9ecd1f4b5` |

没有执行 cargo check：本批只新增报告，主线程仍在修改代码；重复编译不能证明这些交互与源一致。静态推导不代替窗口中的禁用、焦点、动画、像素和设备读写验收。其余产品、相机、手柄、系统/附件、OLED、账户和其他宿主根仍须继续独立核验。


## 主线程后续修正：Settings 收起时序与鼠标 70 证据

2026-10-06：已修正上述 Settings 外部关闭时序。`SystrayActionSelector` 将触发器 active 与下拉 open/mounted 分开：外部关闭在 100ms 收起期间保留 active，计时结束再清除；选择 showMenu 仍立即清除 active。离开页面同时清理这些状态和任务。cargo fmt 与 cargo check --locked --all-targets 通过，仍为 3 项既有警告；未做运行验收。

鼠标 70 新增可复现的 [当前 DPI 收据](mouse-70-dpi-current-evidence.json)，由 `tools/audit-mouse-70-dpi.cjs` 静态提取当前 AST 及 CSS。后续已加入 `mouse_dpi_rows.rs`，F 由未接入推进为部分实现；不能再引用此前“本地主体编辑器尚未改造”的状态。当前收据包含 10 项节点及 2 份 CSS，本子任务最新执行 `node tools/audit-mouse-70-dpi.cjs --check` 通过。

### 鼠标 70 新编辑器复核续记

已重新读取 `mouse_dpi_rows.rs`、`mouse_products.rs` 和当前 cI/EI/XT，并补追 action/reducer，而不是只相信生成器状态文案。

- stages 关闭时仅渲染实际 selected 索引一行；可见单行编号从 1 计，实际写入索引保留，不误写第一个原始阶段。
- 可见行普通 X/Y 编辑会激活该行，隐藏行普通数字编辑保留 active；linked X 同步 Y。源 XT 的隐藏行数字输入仍可编辑、slider 禁用，本地当前区分相符。
- 可见性开关保留至少两条可见阶段；隐藏当前行时选择后续第一条可见行，找不到再取首条可见行。
- 拖排采用 remove/insert 并按移动区间重映射 selected；hidden active 的后续修复与源 cI.updateStages 同序。本地增加 owner/快照/active 越界保护。正常有效阶段状态的映射静态一致；未运行拖放。
- 已回读最新 hover 控件显隐、隐藏行拖动图标 opacity，以及上移顶部/下移底部插入指示；这些此前观察到的差异已修正。

**复核中发现并已修正的 XY 分支差异**：旧版 `dpi_toggle_xy` 无条件调用普通 write_number(Y)，导致可见非当前行且 X=Y 时错误激活，隐藏行且 X!=Y 时反而不激活。源 cI.toggleY 仅在关闭且 X!=Y 时调用 TI；TI 派发 de.Tme，zE 对其无条件赋 active=payload.activeStage（主包导出位置 4154539、reducer 4227959 起）。最新专用分支已照此实现：只在需重置 Y 时无条件 active=index+1，其他情况不改 active。本子任务已独立回读修复，不再把这两条记作未修缺陷。

**仍确认的输入边界缺陷**：`mouse_products.rs` 的 add_range 输入回调仍只在 parse::<f32>() 成功时提交，失败直接 return；write_number 仅当 slider 值不等于提交值时同步输入文本。源 XT 使用模块 4230 的 VT.A，allowDecimals=false、maxLength=6；该模块 parseInput 对空/“-”按 0 处理再 clamp，handleBlur 无条件把规范化值设回输入。静态可推导两例：清空输入失焦，源显示 100，本地保留空字符串及旧草稿；当前为 16000 时输入 16001 失焦，源显示 16000，本地草稿钳回 16000 但因 slider 已是 16000 而跳过文本同步，仍显示 16001。应独立实现当前产品的输入规范化与源数字编辑器交互，不能仅在数值发生变化时回写文本。

数字编辑器的方向键、滚轮、连续步进、maxlength/字符约束，原滑块/开关皮肤、XY tooltip、拖动预览三角及像素位置仍不完整；不因列表和映射正确就将 F 或产品 70 标为完成。主线程报告本批 fmt/cargo check 与鼠键静态数据检查通过；本子任务没有重复执行编译，亦未运行应用、测试或 DLL。

数字输入修复续记：已回读最新 add_range，产品 70 阶段输入增加可选负号及六位数字校验；空值/单负号按 0 进入归一化，Blur/Enter 无条件回写规范化文本。因此前段所列“空值失焦保留旧值”和“16001 钳制但文本残留”两例已静态修复，不再视为未修缺陷。当前模块 4230 已加入收据，最新 `--check` 为 11 项 AST、2 份 CSS，通过。方向键/滚轮/长按及精确原控件样式仍须后续接入；没有运行验收。

拖动预览文本已回读改为 MOVE_STAGE（当前 70 使用语言模块 4693，Z8V→FE→MOVE_STAGE）。编号另保留待核项：源 EI.onDragStart 检查大写 Active，而默认 Redux KE.stages 只含小写 visible，uI/EI 直接传递，没有在这条默认链中添加 Active。因此默认状态路径的源预览编号为空字符串；尚未审完 profile/MW 状态注入，不能把这一发现推算为所有状态下都为空，也不能把正常行 ordinal 自动当作预览编号。主包 KE/wE 位于 4227xxx 段，hash 同上；未执行源代码。

## G. P2：4115 Kitsune Customize 的设备预览、列布局与 SOCD 说明缺失

下一批转向非鼠键家族，仅对 4115 当前挂载作确认。重新读取 `.ref/devices/4115/static/js/main.1a52fc92.js`，SHA-256 为 `03ad7f2d6eafc52a1766a7b850c0d03912344a5bfcd00695b62c5baff76254aa`，与既有 gamepad-product-evidence 定位一致。

- TAB_CUSTOMIZE 页描述位置 `6717445`，其 render 在 `6717479` 挂载 MP。MP 是 hP 的 connect 包装；hP UTF-16 `[6703297,6705685)` 首先渲染 `.config-wrapper.dot-bg`，含 420px canvas 与按产品、editionId、layoutId 选择的 mP SVG 设备图。非 macro 模式下，左列挂载 SP polling 与 PP Mode Switcher，右列挂载 CP，明确传 `isArcadeController:true`。
- 本地 `gamepad_products.rs:687` 的 customize 创建一个只有 TAB_CUSTOMIZE 标题和产品名称的 left panel，arcade 分支跳过按钮区；随后将 polling、mode、SOCD 全部追加到 right 列。没有设备图或源两列分组。即使所有数据字段已经可改，这也不等价于源 Customize 布局。
- CP 源范围 `[6696902,6699024)`：五个 SOCD 单选项各有独立 `.msg` 说明，底部根据 isArcadeController 选择 NP/RP 图片；标题也有该条件。本地 `gamepad_products.rs:806` 起仅有一个通用说明加五个选择按钮，缺少逐模式说明和配图。不能以通用描述符/按钮清单视为这块已完成。
- PP 源 `[6702274,6703296)` 的 safe/standard 两个值与描述分别保留，本地对应选择分别写 `/controller/modeSwitcher/mode`；SOCD 对应 `/controller/dpad/socdSettings/value`。本地 `write()` 发 GamepadProductChanged，SourceProductWorkspace 在 `:378` 订阅并 capture snapshot：有本地草稿更新路径，不是直接写 DLL。当前硬件模式/SOCD 只读观察尚未在本批追到，不把 profile/default 初始化当成设备读取成功。
- 修复建议：按 4115 自己的 root 还原设备图、布局、SOCD 条件标题/逐项说明/配图；保留现有本地值编辑。polling 的游戏内及设备连接条件需要另追，未在本批默认认定全部按钮可用。

排除一个候选误报：4115 数据描述符带共享 M1–M4 button_controls，但实际 hP 只渲染空的 `.config-btns.flex.right`，没有逐按钮映射元素。因此不能仅凭共享按钮元数据就指控本地 arcade 跳过 M1–M4 编辑器；要继续验证的是当前 Kitsune 根实际挂载的内容。其余八个 gamepad 产品尚未因本批抽样获得任何验收结论。

## 鼠标 70 数字交互再次回读

已静态读取新增 `mouse_dpi_number.rs`、当前输入事件订阅，以及 Cargo.lock 实际锁定的 gpui-base 0.7.1 NumberInput、InputState 与 mask 实现。

- 点击数字区域后记录 registered；源 4230 也由 handleClick 注册，而不是仅获焦即注册。本地 registered 期间 step 只更新输入/滑块预览，未调用 write_number，失焦才回写草稿及 active；对应 EI.changeX/Y 在第三参数为 true 时仅改 EI 本地 state 的边界。Tab 获焦且未点击的情况并未强行注册，保留源区分。
- Base NumberInput 的自带默认 step 已由 on_step 替换，方向键分发进入本地 StepAction。鼠标按下立即步进、300ms 重复、抬起/离开停止已存在；鼠标 Click 抑制用于避免释放时再次步进。尚无真实窗口验证点击事件顺序或遮挡，不将这些静态存在项标为运行通过。
- Base NumberInput.render 自动调用 ensure_number_mask，设置 Number{separator:None,fraction:None}。该 mask 在无 separator 时保留原文；InputState 先调用应用 validate 再执行 mask。因此已核实它不会删除前导零、归一单负号，也不会绕过当前整数 validate 放入小数或加号。此处依据实际 0.7.1 实现，不依据组件名推断。
- 当前控件只接收焦点控件内滚轮，源注册后接收 window mousewheel；这个范围差异仍保留。数字区域宽度/布局、边框与透明度动画、精确 spinner 显隐仍属部分实现。

复核中发现并已修正的细节：源 4230 在 typed 字符串后 volumeUp 只 sendToParent；componentDidUpdate 只有数值 prop 变化或 isKeyInput 才用 props 替换 state.value。如果某一步结果恰等 EI 旧值，源仍保留原 typed 文本，而初版 step_dpi_number 总把 typed=false 并覆盖文本。已重新读取最新实现：prop_changed 比较 slider 保留的 EI 值，refresh_text=!state.typed||prop_changed；值未变化时保留 typed 与文本，失焦再归一化。该路径静态已修复，不再记为待修；窗口运行仍未验收。

## H. P2：3592 取景禁用条件未覆盖快捷键捕获与清除

本批只确认 3592 Kiyo Pro Ultra 的当前 CAMERA 分支。源 `.ref/devices/3592/static/js/main.323f0a8a.js` SHA-256 重新计算为 `e2752209d1aa227aca1dbaee32f7ef1ef261ac9b8ccb906636976174dc4e464b`。

- 主包 UTF-16 `4539834` 附近的 i()：LDC 启用且当前分辨率为 4K 30FPS 或 1440p 30FPS 时返回 true。它禁用变焦/平移，并把五个 preset 与快捷键 qh 一起包在 `className:i()?"disabled":""` 中。当前主 CSS `.disabled{opacity:.3;pointer-events:none}`。qh 范围 `[4534812,4539011)` 含输入捕获与清除按钮。
- 本地 `source_controls_data.json` 的 `3592:shortcut-key` 正确包含这两组 disabled_when_any；默认 profile 的 ldc=true、分辨率 3840×2160@30，正处于源禁用条件。
- `source_controls.rs:1392` 的 keys 分支直接调用 render_shortcut_key。该函数 `:1130` 起没有消费 disabled：on_click 直接开始 listening、on_key_down 调 capture_shortcut、清除按钮调用 write_path。capture_shortcut `:710` 和 write_path `:666` 也不检查该控件条件。`camera_sections.rs` 的内容挂载没有额外包住这个组合的禁用容器。
- 因此同一页面 preset 按钮已禁用，快捷键却仍可点击、清除和更新当前 viewPreset 的本地 shortcutKey，与当前源码相矛盾。此处不需要运行或 DLL 即可从调用链确认；其他相机虽然共用控件，本批没有把它们自动列为源已复核。
- 建议：将同一 disabled 判定用于快捷键视觉、监听开启、捕获和清除写入口；不能仅设 opacity。真实 inputredirect 注册、映射/全局快捷键暂停及恢复仍是独立服务接入问题，不得伪造成功。本批只核实本地快捷键草稿写入链，未声称设备快捷键已保存。


## G 修复独立回读：4115 主体布局与本地操作已补入

已独立读取 kitsune.rs、kitsune_data.json、prepare-kitsune.cjs 及当前收据，并从当前主包切出 OP/SP、CP、pP、PP、hP/MP；未运行应用或供应商脚本。

- 4115 Customize 已转入专用渲染；source_workspace 将 device.layout_id 传入工作区。预览为 340px 高、770px 内层及 251px 原始 SVG。当前产品上下文只提供 4115_0/svg_prods/0.svg，因此 layout 0 使用该图、其他 edition 回退 edition 0 的资源范围成立。
- 左列 polling 与 safe/standard、右列五项 SOCD 独立说明及 Kitsune 配图均已存在；SOCD 使用 arcade 分支标题 SYq。当前 CSS 的 socd-settings-container、msg、mode-option 及预览尺寸已回读，原 G 主体缺项不再标为未实现。
- Radio 来源为主包模块 49412（DP.A）。当前主 CSS 给出 20px 外圈、10px 内点、30px 文字缩进及 .2s ease；本地 Base Radio 与 200ms motion 有对应实现。未声称键盘、焦点、动画运行通过。
- polling 唯一选项总显示选中与 OP 一致。mode/SOCD 写 controller 草稿，通过 GamepadProductChanged 进入 capture，不是 DLL 写入。源 PP 缺少 mode 时回退 safe；外部缺字段状态的本地注入行为仍未完整核查。
- 此轮回读时 tips、macro 条件可达性及真实状态读取仍待进一步核实，保留 partial。41 项 AST、8 个 CSS 文件、2 个 SVG 不等于全行为验收；未重复主线程 cargo check。

## H 修复独立回读：3592 禁用条件已覆盖快捷键本地写入口

已回读 source_controls.rs：keys 分支将统一 disabled 传入渲染，禁用时透明度 .3，监听边框与 hover/指针不再显示为可操作。监听开启、清除点击都通过当前 control 和 draft 再判定 disabled。capture_shortcut 入口也重检，禁用时清除 listening 并返回，不能进入 write_path；覆盖进入监听后 LDC/分辨率改变再按键的路径。

禁用仍按 disabled_when_any 的任一组、组内全部条件判断；3592 的 LDC + 4K30/1440p30 条件传至三个入口。未发现第二条绕过它们的捕获写入调用。H 确定性本地缺陷已静态修复；通用 write_path 不是权限边界，结论限已追踪 UI 调用链，不包括真实 DLL、全局快捷键注册/恢复或窗口事件验收。

## I. P2：2636 低死区警告的关闭回退与操作不一致

本批核对 2636 Wolverine V3 Pro 当前 THUMBSTICKS。源 .ref/devices/2636/static/js/main.82d8a835.js 实算 SHA-256 为 a5b49ece13b13202b65e97d565b77ca61d22c8c6e14f9ec560dcee68d1155f9a。以下为 UTF-16 位置。

- 构造状态在 6673895 附近将 prevLeftDeadzone/prevRightDeadzone 初始化为当时左右值。handleThumbstickChange 仅在新值至少 7 时更新回退状态（6668854）。onContinue 只关弹窗；onCloseDialog（6670735）恢复对应 prev。componentDidUpdate 只检查 mapping，不会因低死区提交而更新 prev。
- 本地 thumbsticks 每次低于 7 都将点击前当前草稿存入 low_deadzone，Cancel 恢复此快照。因此初始化 7 → 选择 3 → Continue → 选择 5 → 关闭警告，源恢复 7，本地 Cancel 恢复 3。这是状态更新链的明确差异，不依赖 DLL 或运行 UI。
- 源 6674483 之后的实际弹窗包含零死区/低死区分别的标题和双段说明、Recalibrate、Continue、独立关闭及可展开 deadzoneDecs。Recalibrate 关闭弹窗并切换 Calibration 页。本地所有值共用 LOW_DEADZONE_INFO_TITLE/DETAIL，在页面底部追加普通 panel，仅 Continue/Cancel，缺少上述分支与导航。真实校准服务尚未接入不妨碍补纯 UI 导航。
- 修复应保留左右回退状态生命周期和警告操作含义；不得假造摇杆读数或校准成功。校准页目前仍为禁用按钮和未连接说明，本批未判完成。

排除两个候选误报：2636 Power 的 dm → Am 明确传 noSwitch:true，本地无省电主开关不是该根的遗漏；Prevent Double Deadzones 源也分别反转左右布尔值并以 AND 为 checked，不应仅凭混合状态直觉改成统一赋值。结论不自动推广至其他 gamepad。


### 2636 专用弹窗最新独立回读

已读取 gamepad_deadzone_dialog.rs、gamepad_products.rs、source_workspace.rs，并将 gamepad-2636-dialog-current-evidence.json 的 22 项 AST 收据逐项与当前文件 UTF-16 切片比较，全部相符；未执行应用、构建、测试、资源生成器或 DLL。以下更新原 I 的当前状态，原发现保留为历史原因。

- 已区分 ZERO_DEADZONES_WARNING / LOW_DEADZONES_WARNING 与各自 DESC_1，二者共用 LOW_DEADZONES_WARNING_DESC_2；与当前源 zeroDeadzoneWarning/lowDeadzoneWarning 的标签导出一致。原“零/低共用标题”的缺项已修复。
- Continue 调 dismiss_deadzone(false)，不恢复数值；右上关闭调 true，取 low_deadzone 快照恢复；Recalibrate 先 false 关闭再发 GamepadCalibrationRequested。父层 subscribe_in 调 set_page_key(TAB_CALIBRATION)，更新父导航并 select_body_page 更新子页。未发现该链会只改局部标题或遗漏实际页面切换；没有发出设备校准指令。
- previous_deadzones 的进入页面初始化、仅 >=7 编辑更新、Continue 不改回退基线仍保留；初始化 7 → 3 → Continue → 5 → 关闭的确定性回退差异已修复。restore/set_page 均清理现有弹窗状态，避免把关闭操作应用于已切换的 profile。
- 已有可展开说明：标题/正文/信息图标、0↔200px max-height（250ms）、opacity（200ms）、箭头 -90↔90 度（200ms）对应当前 CSS；原“展开内容缺失”不再成立。原源按钮的 aria-expanded 尚未见本地显式对应；像素与辅助功能不因视觉控件存在而自动验收。
- 定位已用 thumbstick_bounds 的实测 origin，应用 -20px/-40px 外距和内部 +110px；canvas 在边界改变后请求 refresh。实际 gpui-base Dialog 以 viewport 原点 anchored，绝对 popup 不走其默认居中流式布局，因此未发现重复叠加父容器坐标的问题。滚动、边界测量首帧与像素位置仍未运行验证。
- **保留确定的范围差异**：Base Dialog 内部还有覆盖整个 viewport 的 backdrop 包装层及 focus_trap。当前代码只把可见 backdrop/popup 定位到测量容器，包装层仍在窗口外余区域捕获鼠标按下；源 99661 只是 sensitivity-container 中的 absolute alert-backdrop。不能把视觉起点相同等同于背景交互及焦点范围完全一致。此项未阻断弹窗本身三个操作，不将其描述成导航失败。

本轮原 I 的弹窗主体缺项已静态修复；仍为局部完成，不包含校准页真实 UI/状态读数、DLL 接入、焦点和窗口视觉验收。

## 4115 Macro 可达性复核：不得由共享分支推定缺少绑定入口

已重新读取当前 Macro 的 main.3f4b9604.js 与 1700.a2780a35.chunk.js，而非只使用旧收据。两者 SHA-256 分别为 fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d、817af243f7964927bb63ff9ce11c26cef4535b770013a541daa8852a80c8415c。

- 1700 的模块 21700、M（UTF-16 5190）先按 supportMacro 为真，或类别 KEYBOARD/MOUSE/MOUSEPLUSMAT/KEYPAD/SYSTEM，或 productId=3907 筛选设备。4115 当前 DeviceInfo 为 GAMEPAD，不命中类别或特例。当前 AvailableDevices 的 4115 条目只有 productId 和 psModeIds，不提供 supportMacro。
- Macro 主包 547200 起的 b 从 getWindowStorageItem(DEVICE_RUNTIME_DATA) 读取每项 value JSON，过滤 READY、有 serialNumber 且未关机后派发 validDevices。434194 的设备投影仅使用 a.supportMacro=!!e.supportMacro，不会因存在 macro 路由而设为真。4115 当前主包未找到 supportMacro 标志，因此本次不能证明该产品应出现在绑定选择器。
- 宿主 4.0.827 的 electron/modules/window_storage/index.js 的 getWindowStorageItem 只是汇集窗口 storageMap 中已有值并序列化返回；未为 4115 或 GAMEPAD 添加宏能力。文件 SHA-256 为 6a10c8daeaa5a5bfe87f136d666e37c5915a6d62b10bf2e07d6e7f1a86a1f4a7。未执行 DLL 或查询运行时，也未穷尽所有可能的外部发布者，所以不能反向断言任何运行时状态下 supportMacro 永远不可能为真。
- 真正选择设备后，M 直接从 allRZDevices 找产品并拼接 /products/{pid}/ui/index.html?displayMode=macro&macro=...&containerId=...&deviceEditionInfo=...&serialNumber=...；E（4094）把 URL 给 iframe。这里不是一个绕过 M 筛选的宿主 openDevice 入口。宿主通用 URL/窗口能力也不证明选择器可达。
- 4115 主包确有 URL 分支（6777787），指向 Lh → Ch（6764415），后者加载 groupList 并传 displayMode=macro，再挂 Ah 映射容器。因此“代码存在宏分支”成立，“当前正常设备选择路径应提供 4115”没有成立。此前 G 的 hP 宏条件只能记为条件分支待核，不能将新增 4115 Macro 入口列为已证实缺项。

当前建议：保留本地 Macro 输入目录不提供 4115 的现状；能力旗标的真实来源属于后续只读状态审计。若以后取得当前服务发布 supportMacro=true 的静态证据，再追完整 iframe 映射根，不凭共享条件强行加入口。这不是全局断言该产品硬件绝不支持宏。

## I 窄项修复回读及 Kitsune tips 更新（前批次历史）

已回读 2636 previous_deadzones：进入 THUMBSTICKS 时缓存两侧当前值；仅该产品非 sensitivity 的至少 7 编辑更新缓存，低值警告读取缓存；Continue 不修改缓存，Cancel 恢复警告快照。原 I 中 7→3→Continue→5→关闭应恢复 7 的确定性差异已静态修复。没有把此状态逻辑推广到其他手柄；该批次当时尚未接入零死区文案、完整 modal、Recalibrate、展开说明；这些内容已在本轮完成主体实现和静态复核，最新状态及剩余差异以本报告 I 节为准。

Kitsune 已新增三块面板的 help_control 接入，不再把三个提示入口一概记为缺失。helper 以 viewport 近似源 O_ 的 body-wrapper 定位边界，精确边界及真实窗口行为仍为 partial。
