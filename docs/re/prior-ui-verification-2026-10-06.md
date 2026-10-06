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

首轮发现的刷新风险已修复并再次回读：InputEvent::Change 改 typed/max_len 后现已显式 cx.notify，避免 spinner limit 闭包残留旧状态。同时显式设置 MaskPattern::None；基础库 mask_pattern 会设 mask_pattern_set=true，NumberInput 的 ensure_number_mask 因而直接返回，不会擅自改变源字符串规则。

已核实鼠标去重回调顺序：gpui-pre 0.3.8 的 div click_listeners 按注册顺序 push、按同序执行；装饰器通过 StatefulInteractiveElement::on_click 先注册鼠标标记，BaseButton render 随后才追加 NumberInput 的 on_step。因此鼠标 click 先设 suppress_click 再被 take 消费，键盘 Step 不会被正常前一次鼠标 click 留下标记误吞。Base NumberInput 后置 `.disabled(disabled)` 会覆盖装饰器的 limit 禁用状态；现有 mousedown limit guard 加鼠标 click 去重阻止边界额外步进，但不能据此声称边界按钮的全部指针/无障碍禁用语义与源CSS完全等价。

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

## J–L. 515 Blackwidow Chroma Customize：本轮新增独立复核

本轮只复核产品 **515** 的普通 Customize 挂载及下列三项，不把结论推广至其他键盘。重新读取当前 `.ref/devices/515/static/js/main.f60ca5aa.js`，实算 SHA-256 为 `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8`；本节源范围均为 UTF-16 半开区间。既有 `keyboard-product-pages.json` 只用于定位；NA、DA、Br、ql、Ql、Vl、Zl、MA 等相关切片均与当前主包重新比较，未用旧符号名称替代当前读取。未运行应用、构建、测试、供应商 JS 或 DLL。

**已核实的共同挂载链：**当前导航 `[7531850,7532180)` 挂 `IO`；`IO=TO` 的 `[7377587,7378050)` 挂 `_O`，其 connect 包装 rO。rO 在 `[7374470,7375560)` 的普通内容容器挂 eO；eO `[7348396,7349069)` 包装 $l。$l 的 `[7347880,7348970)` 在 `displayMode !== "macro"` 时直接渲染左列 DA（Gaming Mode）和 Jl（Snap Tap），右列 mA（Keyboard Properties）。三个挂载均没有传 isSystem、deviceType、supportsShortcut 等额外 prop；不能从共享组件支持这些参数推定本根也传了它们。

本地 `src/features/source_workspace.rs:375` 命中 keyboard_products::source_product(515)，`:384` 创建 KeyboardProductWorkspace，`:400` 保留 FamilyBody::Keyboard；`:1144` 渲染该实体。`use_supplement_for()` 对 Keyboard 仅允许 OLED，515 Customize 不会被通用 supplement 替代。`keyboard_products.rs:1313` 的 TAB_CUSTOMIZE 分支调用 customize，`:1297` 起在映射面板后仅追加 gaming_mode。其他 DeviceWorkspace 的 `customize_page.rs:566` 虽调用 `snap_tap_panel`，它不在上述 515 的实体渲染链，不能据此认定本产品已有 Snap Tap。

### J. P2：515 Gaming Mode 缺少 Menu 禁用状态行

- 当前 NA 完整类 `[7276079,7280720)` 在 render 从实际 buttonList 查找 `inputID === "KEY_APPLICATION"`；找到且 `!isSystem` 时挂 `id:"menuKey", disabled:true, active:isWindowsKeyDisabled`。对应条件与状态行可见 `[7278330,7280393)`。DA 的 `[7280729,7281265)` 明确从 gameMode reducer 取 isWindowsKeyDisabled，不是另一个 Menu 字段。
- 515 的当前默认组及 layout 1（UnitedStates）解析到模块 21368；layout resolver `[1149574,1150109)` 对 UnitedStates 和 default 返回 R。当前默认按钮表 `[1395671,1396150)` 包含 KEY_APPLICATION；本地 515 keys 也包含同一 Menu 键。结论限这些已证按钮存在的状态，不声称没有 Menu 的布局也应强制显示。
- 当前入口引入 `ye=a(54693)`（`[6862693,6862860)`）；该模块导出 `HWD → ko`，`ko="DISABLE_MENU_KEY"`，映射见 `[5825410,5825630)`、`[5839580,5840055)`。本地 `keyboard_products.rs:779` 的 gaming_mode 没有 Menu 行，`:836` 之后只给 716 增加了独立 Copilot 条件。
- 影响：515 显示 Windows 禁用状态，却省略同时受 Gaming Mode 管理的 Menu 状态。修复应按实际 KEY_APPLICATION 和 isSystem 条件添加只读行，checked 取同一 isWindowsKeyDisabled；不应新增可编辑的 Menu 开关，也不应把 716 Copilot 条件直接扩张为全产品条件。

窄范围未发现新的普通开关状态差异：本地 set_mode 修改 state、enableInGame、isWindowsKeyDisabled，保留其余字段；主开关关闭后保留 in-game 选择，Alt-Tab/Alt-F4 在关闭时禁用，与该根未传 gameModeButton 的 NA 分支一致。这里只确认这些本地编辑路径；未把任意外部注入的矛盾 state/字段、实时游戏观察或真实禁键效果计为通过。`write()`（`:263`）→ KeyboardProductChanged → source_workspace `:395` snapshot/capture（`:692`）更新活动 profile 的本地 source_settings，没有设备写回。

### K. P2：515 Snap Tap 主体与成对按键编辑操作未挂载

- 当前 Jl `[7342211,7343191)` 读取 snapTapReducer.isEnabled/keyList，并无产品能力隐藏判断；主开关在 adjustmentModeRunning 时请求调整提示，否则切换 enabled。`br → Br` 已回读 `[7160835,7164980)`：未传 hasFWUpdate 时也不产生固件门控，子内容仍渲染。故本项不是只有共享组件存在的候选。
- 实际 ql `[7335991,7341740)` 保留临时列表 U、正在编辑的 pair id、KEY1/KEY2/READY 状态和提示状态。点击现有任一键槽可编辑；“+”在四组或正在录制时禁用，新增带唯一当前 id 的空组，进入第一键捕获。第一键有效后进入第二键；第二键有效才向 reducer 提交列表并退出捕获。成功消息约 3 秒后回到介绍。未完成编辑点击外部/失焦有独立清理规则，不能直接用逐键立即覆盖已保存值代替。
- Vl `[7334403,7335163)` 为每组成对键槽显示本布局键名，支持两槽独立选择；**id=1 的第一组不提供删除按钮**，其他组才调用 q(id)。删除过滤对应组后按顺序重编号。禁止键与重复检查在 `[7335163,7335783)`：KEY_APPLICATION、KEY_LEFT_GUI、KEY_FN、DKM_F6、DKM_D2 禁止；已出现在其他槽中的键同样触发 warning。方向键排除只针对 KEYPAD + DIRECTIONAL；515 是 KEYBOARD，不应复制该额外限制。
- 当前 reducer 初态 `[6906790,6906910)` 为 isEnabled=false、A/D 第一组、mode=LAST_INPUT、pressedKeys 空数组。它与本地静态 profile.snapTap 默认 isEnabled=true 不是同一 owner。当前本地还没有该面板，所以不把此数据差异算作“已显示错误初值”的另一个缺陷；实施时应先决定真实观察、profile 和本地草稿的状态边界，不能直接用目录默认值宣称读取成功。
- 本地 `keyboard_products.rs:1297` 的 Customize 尾部没有 Snap Tap，整个文件无 snapTap 读写或保留编辑状态。已有另一个 DeviceWorkspace 的 snap_tap_panel 不在 515 挂载链，不能覆盖这里的新增、编辑、删除、校验和本地保存缺口。
- 建议先按该根接入开关、pair 编辑/删除/四组上限、重复及禁止键提示和编辑生命周期，再将提交结果纳入现有本地 snapshot/capture。inputredirect、全局输入暂停/恢复、pressedKeys 状态观察及 reducer 广播属于需要独立确认的服务边界；不得用假键事件、假生效或虚构硬件成功补齐。单凭 UI 增删已接入也不能把整个 ql 或真实 Snap Tap 判为完成。

排除一项容易误报的缺失：Jl 虽创建 Ql 的 FN + L SHIFT 快捷键提示，当前父根没有传 supportsShortcut，Br 仅在该 prop 为真时渲染 renderShortcut。因此当前证据不能要求 515 默认显示这条快捷键提示。

### L. P2：515 Keyboard Properties 右列及操作入口缺失

- 当前 `mA=MA`，MA 完整类 `[7281943,7282758)` 在 deviceType 不是 analog 时显示 KEYBOARD_PROPERTIES_HEADER、KEYBOARD_PROPERTIES_TOOLTIP、Windows 图标和 OPEN_KEYBOARD_PROPERTIES 操作。普通父根 `(mA,{})` 没有传 analog；Game Controller 属性行因此不属于本次缺项。
- 源点击明确调用 `Fa.A.OpenKeyboardProperties()`；包装器 `[1775390,1775650)` 在 Electron 下发送 `{action:"OpenKeyboardProperties"}`，否则只在同名 window 函数存在时调用。语言键 SwD→Jo、MxL→$o、Tpt→ti 及 literal 已回读 `[5826454,5828010)`、`[5839580,5840055)`。Windows 版本图标的 LA `[7281266,7281943)` 会查询 getWindowVersion，带缓存；不应把默认 "11" 当作本机已读取版本。
- 本地 515 Customize 没有该右列、文案、图标或命令，`keyboard_products.rs:1297` 只构造单列 gaming_mode。不能将“厂商 DLL 写回后置”解释成删除一个本应保留的系统设置入口。
- 建议补齐该右列和命令意图，核实并复用本机系统设置调用边界及失败处理；实际打开外部窗口未在本轮执行，也没有验收。此次只确认 UI 与 wrapper 契约；当前宿主 main.js 的分派名称存在不等于底层已完成调用审计，不自行编造 DLL ABI。

本轮增加三项可定位缺口，仍为 **515 单个产品的局部静态复核**；没有完成键盘映射编辑器、Lighting、所有布局、像素/焦点或设备读写验收。331 产品、1419 主页面的 partial 口径及完整产品 0 不变。

### J 窄项修复独立回读

已回读主线程新增的 `keyboard_products.rs` Menu 行：仅 product_id=515 且 spec.keys 存在 KEY_APPLICATION 才渲染；文案 DISABLE_MENU_KEY，checked 直接读取 gamingMode.isWindowsKeyDisabled，disabled=true，无点击/变更回调。当前 515 根没有传 isSystem，因此该分支符合本节已证范围。J 的状态行缺失已静态修复；K、L 仍未修，其他产品、其他未核按钮布局、像素和真实禁键行为不随此修复获得验收。

## 3334/3337 冲突确认与播放设备选择：新增实现独立窄回读

已静态读取 `stream_mixer.rs`、`audio_products.rs` 接入及 SourceProductWorkspace capture/restore 链，并逐项对比 `stream-mixer-current-evidence.json` 与两产品当前主包。28 条 AST/语言键/action 收据的 SHA 与 UTF-16 原文全部匹配；两份当前 CSS 的 SHA 和共 34 条规则原文匹配；两种 SVG 在两个产品中的源文件均与本地文件逐字节一致。未运行应用、测试、cargo 或 DLL。

- **冲突确认边界：**request_mixer_enable 只在当前 STREAM_MIXER_HEADER 且具备 mixer state 时工作；已有 warning 时忽略重复请求。开启且 active_mixer=Some(true) 只建立 warning 和焦点状态，不更新 ENABLE 或 emit 草稿变更；Cancel 只 dismiss，Enable 才 edit(ENABLE,true)。原 yU 的 `f && !C`、取消 B(false) 及确认 E(true)/B(false) 对应关系成立。Base Dialog 的 Escape/Enter/backdrop 路径均被显式禁用，没有新增默认提交；当前源仅提供两按钮。
- **禁用与过期选择：**选择回调重新检查当前页、主开关、无 warning、最新观察列表仍包含该名称；过期/禁用回调只 sync 选择器，不写草稿。缺失的当前名称追加 disabled 项，不自动改选其他设备；未知 devices 与已观察但找不到名称分别显示“未读取”和离线状态。源 Jm/Yn 原本以索引读取最新 playbackDevices；本地使用名称校验，未将已断开的旧行当成有效提交。
- **本地与观察分离：**devices、active_mixer、observed_device 保留在 MixerState，观察更新仅 sync/notify，不发 AudioProductChanged；未显式本地选择时 snapshot 移除 playbackMixDevice。首次显式选择把 local_selection 设 true，即使名称恰等于 draft 默认、edit 因值相等提前返回，也另行 emit AudioProductChanged，使 snapshot 新增的本地覆盖进入父层 capture。这条边界已确认，不会因为字符串相等而丢失用户明确选择。
- **生命周期与恢复：**restore 在合并草稿前 dismiss warning，根据保存内容是否有 string playbackMixDevice 恢复 local_selection；合并后 sync_mixer。切页 dismiss warning，迟到的确认/选择再检查 PAGE；不存在跨页确认直接启用的入口。saved=None 时恢复默认草稿并取消本地覆盖，已有真实观察仍用于展示。SelectState 0.7.1 的程序 set_items/set_selected_value 不发 Confirm，因此 sync 不能把观察回流误当用户本地操作。
- **尺寸和原资源：**620×164 面板、20/30 padding、25px 标题图标、20×17 离线 SVG、90×27 按钮、20px 间隔及100ms透明度已对照当前 CSS。源 top:50%/translateY(-100%) 与本地 viewport 中 bottom:50% 的竖向定位含义一致；Base Dialog 的实际 anchored host 为 viewport 原点。没有把这些静态尺寸认定为真实窗口像素通过。
- **图标间距修后回读及保留差异：**主线程已给离线图标容器补 margin-top:5px、margin-right:-10px，并去掉外层 gap_2、改为 items_start；已回读确认，两个边距缺失不再计为未修。当前仍在通用音频列中追加选择器，未还原完整 yU 两总线区域，Select/Tooltip 的原外观与位置也未完成。Base Dialog 的全窗口焦点/遮罩范围与原 DOM containing block 的实际运行关系未验收。MW 发布者未接入，未知状态下的本地编辑不是设备读取或启用成功；运行、查询、预设/通道及 DLL 写回均不在此次通过范围。

本次未发现上述已接入确认/选择路径的确定性本地状态阻断问题，只确认其静态控制流和保存对象边界；原 B 的其余缺项继续保留。

## M. P2：226 Basilisk V4 Pro 滚轮模式、等级与锁定条件缺失

本轮专查 **226 的普通 Customize 滚轮区域**，不扩大至其他 Basilisk 产品，也不把 ADVANCED 动态灵敏度或本项目通用 TAB_SCROLLING 视为同一个界面。仅静态读取当前源、用维护中的 `tools/extract-device-decl.cjs` 解析声明及核对本地状态链；未运行应用、构建、测试、厂商 JS、DLL 或 cargo。

| 当前文件 | 本次实算 SHA-256 |
| --- | --- |
| `.ref/devices/226/static/js/main.08f95762.js` | `ccbd5af37e23d1c64faf62551d15b0ef91d6e89fc06cafab3fb9464c598f884e` |
| `.ref/devices/226/static/js/8355.3d5e573e.chunk.js` | `b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8` |
| `.ref/devices/226/static/js/3485.794fe922.chunk.js` | `42871b0390959f81d70f761f0429137a8311097d4551c5f7b5d3124bd45e6ebd` |
| `.ref/devices/226/static/css/3485.3a185bc1.chunk.css` | `7e17ff497704138c45fe9bb9a8ac781d095825742f9cdf6cb4402d89169c1293` |

以下位置均为 UTF-16。旧 mouse-page-source 清单仅用于找到 8355 的导航，实际跨模块根、子组件和行为均重新读取；该清单里 Customize 的 components 为空不代表当前源没有内容。

### 已核实的实际挂载和本地接入

8355 在导航 `275354` 附近挂 `qe.A`，`[116202,116550)` 的 import 指向模块 42。模块 42 在当前 3485 开头导出 `A → Ks`，`Ks=us`（`99775`）；us `[99485,99775)` 挂 ps，ps `[97707,99484)` 包装 Es。Es `[72814,97698)` 的普通内容在 `[97052,97520)` 挂 es，es `[71722,72279)` 包装 qt。qt `[63881,71713)` 的 `[71496,71713)` 明确在 `displayMode !== "macro"` 时左列挂 **fe**，右列挂另一系统属性组件。因此本次 fe 是该产品普通界面可达内容，不是只从 scrollWheel 字段或共享导出猜测。

本地 `source_workspace.rs:364` 命中 MouseProductWorkspace 并订阅 MouseProductChanged（`:370`）；Mouse 的 use_supplement_for 返回 false，`:1143` 渲染该实体。`mouse_products.rs:1231` 起的 render 把 TAB_CUSTOMIZE 交给 customize，`:1216` 起只在 draft 存在 scrollWheel 时加两个通用 Checkbox：smartReelEnabled、accelerationEnabled。226 当前已满足该条件，所以两个开关已显示且可写；本轮不是把整个滚轮区误记为完全没有内容。

### 源界面操作与具体缺项

当前 fe `[18387,20812)` 从 scrollWheelReducer 取 scrollMode、disabledModes、两个 enabled 和两个 level；`De=Se`，Se `[16931,18137)` 是两个功能共用的标题/开关/说明/等级主体。模式定义 `[18211,18387)` 为 Tactile、FreeSpin、MicroTactile，顺序固定。

| 项目 | 当前可达源规则 | 当前本地差异 |
| --- | --- | --- |
| 三种滚轮模式 | 三段模式控件反映局部选择 t；被 disabledModes 排除的模式不能点击；有效点击先更新局部 t，再请求 setScrollMode | 没有模式控件，也没有 scrollMode 编辑入口 |
| 禁用模式 | 三项复选框最多选两项；已经禁用的项始终可解除。禁用当前局部模式时，按 Tactile→FreeSpin→MicroTactile 顺序选首个仍可用项并请求模式变化，然后提交 disabledModes | 没有 disabledModes 列表、两项上限或当前模式回退；不能用只有两种布尔功能开关代替 |
| Scroll Acceleration 等级 | Se 的滑条 min=0、max=4、step=1，Low/Medium/High 标签，noTip=true；只有 enabled 且未被强制锁定才可改 | 本地只提供 accelerationEnabled；没有 accelerationLevel retained 控件或写入口 |
| Smart Reel 等级 | 同一 0–4 滑条，配两段独立说明，写 smartReelLevel | 本地只提供 smartReelEnabled；没有等级、两段说明或相应写入口 |
| FreeSpin 被禁用 | fe 以 `disabledModes.includes("FreeSpin")` 同时 forceDisabled 两个功能；Se 的显示 active=`enabled && !forceDisabled`，开关回调也检查 forceDisabled，滑条禁用；悬停锁定开关或滑条显示 ENABLE_FREE_SPIN_TO_USE_FEATURE | 当前通用 toggle 没有 disabled 参数，checked 直接读原布尔字段，不消费 disabledModes；恢复包含 FreeSpin 的禁用状态后仍可修改这两个功能 |

最后一项有可确定的本地恢复路径：`mouse_products.rs:188` 的 restore 接受并保留已有 profile.scrollWheel 对象；其中 `disabledModes:["FreeSpin"]`、smartReelEnabled=true 或 accelerationEnabled=true 是源允许保留的状态。当前 `toggle()`（`:594`）会显示原布尔 true 并允许 on_click→write；源 Se 在同样数据下显示关闭并拒绝点击。此处不声称用户已能通过当前本地 UI 创建 disabledModes——该创建入口本身尚缺；缺陷范围是已经保存/导入并恢复的合法对象。修复不能通过把原 enabled 永久改成 false 代替显示派生，因为源解除 FreeSpin 禁用后应重新显示保留的 enabled 状态。

已核对文案导出所在模块，未仅凭重名符号：3485 模块 42 的 `K=s(4693)`；当前 main 的 `b_8:()=>qs` 位于 `265091`，qs 是 SMART_REEL。主包别处存在另一个 b_8 导出，不可混用。其他标题、禁用提示和 Low/Medium/High 均从同一 4693 对应 export/literal 解析。当前 CSS `[16282,18900)` 还给出 36px 三段模式区、选中/禁用状态、分组间隔、锁定开关及滑条透明度、跟随指针的提示；现有两个普通 Checkbox 不覆盖该视觉和交互结构。

### 保存、状态观察与实施边界

当前两个本地开关的编辑链已存在：toggle→`write()`（`:230`）→MouseProductChanged→source_workspace snapshot/capture→活动 profile.source_settings。snapshot 克隆整份 draft，保留未编辑的 scrollMode、disabledModes 和两个 level；restore 也恢复这些字段，故问题不是它们都被本地保存过程删除，而是缺少界面编辑和相应条件。用户本机保存继续经 `shell.rs:1389` 的 save_profiles→PreparedSave→store::write_workspace（`:1439`），有写入失败处理；本轮未做文件往返运行验证，也未把该链当作设备写回。

主包 `[206640,209990)` 的当前 reducer区分 UI 请求和观察：Jgj 发 ON_SCROLL_MODE_V2 时没有立即改 reducer.scrollMode，coV 才更新该字段；功能开关/等级和 disabledModes 有各自请求与回填 action。8355 `[268200,268960)` 的 loadScrollWheelSettings 先合并当前 DEFAULTPROFILE.scrollWheel，再分别派发模式、禁用项、开关和等级到 reducer。因此不能把 reducer 临时初态 Tactile 或本地 profile 默认 FreeSpin 直接描述为真实设备已经读到的模式。

fe 的模式请求使用 module 6079 的 300ms debounce 包装；已静态读取当前 `1102.dc9e537e.chunk.js` 的 `[9760,10712)`，文件 SHA-256 `d41f3163bca18c0ee1f8bd2cd4233ff9d71c9e7adf505a2e971b5f5a6ca0897d`。该包装在 fe 函数体内创建，当前 fe 没有保留或卸载取消；这里只记录其实际构造和延迟，不推定跨重渲染总能合并为最后一次请求。新增本地界面需要明确局部预览、草稿保存和未来设备回填的 owner；不能因 DLL 写回后置而省略 UI，也不能把局部选择标成硬件已经切换。

建议按此 226 专属根补齐模式/禁用项/两组等级和说明，先完成本地受控状态、有效性、回退和恢复链，再独立接入已经证实的只读观察。当前两个 Checkbox 的普通本地保存可保留；FreeSpin 锁定需同时覆盖显示、指针和写入口。其余产品、226 映射编辑器、ADVANCED、完整布局、键盘焦点和真实设备行为未在本轮验收，M 仍是一个产品的一块局部复核。

## L 实施前续证：515 Keyboard Properties 参数、布局与现有系统入口

本轮仅补充 L，不重复 Snap Tap，也未修改实现。重新读取 515 当前主包（SHA 与 J–L 表一致）及 `.ref/devices/515/static/css/main.f65da71b.css`，CSS SHA-256 为 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`。以下位置仍为 UTF-16；CSS 通过维护中的 `tools/css-source.cjs` 静态解析，保留媒体条件。

### 挂载条件、具体参数与事件

- `[7348070,7348440)` 的普通 Customize 分支挂 `vr(direction:"right") → mA({})`；fr/vr 在 `[7160533,7160835)` 分别生成 `.body-widgets.flex` 和 `.widget-col.col-right`。mA=MA，MA `[7281943,7282758)` 没有产品/配置文件/设备在线、Gaming Mode 或 Snap Tap 的可用性检查；唯一相关分支是传入的 deviceType 是否为 `"analog"`。515 此处没有传 deviceType，故标题/帮助/操作分别是 **KEYBOARD_PROPERTIES_HEADER、KEYBOARD_PROPERTIES_TOOLTIP、OPEN_KEYBOARD_PROPERTIES**，不显示 Game Controller 行。
- MA 给通用 br 只传 title、tips、children，没有 hasSwitch、disable、hasFWUpdate 或 supportsShortcut。br/Br 不会为这个调用附加主开关、固件禁用或快捷键区；帮助入口来自 tips，并非需要新增的独立设置开关。
- 子图标为 `pA({})`，pA=LA `[7281266,7281943)`；没有传 windowBigIcon。LA 初始 osVersion="11"，挂载后从模块级 CA 缓存读取，或调用 getWindowVersion 并填缓存；仅结果严格等于字符串 "11" 时附加 windows-11 类，其他结果显示通用 windows 类。该初始占位不能标成已查询到本机 Windows 11。
- 点击文字只调用无参数 `Fa.A.OpenKeyboardProperties()`，当前包装器 `[1775390,1775650)` 在 Electron 下发送 `{action:"OpenKeyboardProperties"}`，没有 serialNumber、profile、page 或 payload；非 Electron 下仅在同名 window 函数存在时调用。MA 不 await 结果，不更新组件/配置数据，也不发 Save/Apply、设备状态或成功提示。因此新增本地入口不应把点击记成 profile dirty 或“设备保存成功”。
- 当前导出绑定已重读：SwD→Jo（`5827506`）、MxL→$o（`5826504`）、Tpt→ti（`5827649`），literal 位于 `[5839930,5840110)`。本地 10 份 locale 已有这三个键；无需硬编码中文或增造同义键。英文本地值为 KEYBOARD PROPERTIES、Launch the Windows Keyboard Properties window.、Open Windows Keyboard Properties。

### 可直接采用的当前布局证据

| 部位 | 当前 CSS / 参数 |
| --- | --- |
| 右列与面板 | `.body-widgets` 最大宽1240并换行（69207）；列宽600（69677），仅 viewport <=1279 时列两侧 margin30（78896，带媒体条件）；`.body-widgets .widget`（69817）min/max-width600、padding30px 40px、margin10px auto、radius5、背景#111 |
| 标题与帮助 | titleRow/标题（73630、73691）为 RazerF5 16px、绿色、uppercase、底部20px；help 在 top/right10（75290），正文提示从 KEYBOARD_PROPERTIES_TOOLTIP 渲染。当前 Br 使用 portal/fixed 边界定位，本地 help_control 的近似定位仍需单独核验 |
| 图文行 | `.img-text.flex` 水平、align-items:center（77076）；Windows 图标44×44、min/max-width44、背景居中且不重复（9010），图标右侧20px（8906）；515 不选 window-big-icon |
| 操作文字 | `.img-text .external`（10400）14px，最终 line-height44px（同规则较早17px被覆盖），灰色、下划线、capitalize；hover绿色（10532），active opacity=.7（10684） |

本地 `surface::page_columns/page_column/panel` 已有600px列、1279px换行规则、面板内边距和标题结构，可在现有两列组织中复用；不要在旧映射面板内部再堆一个属性按钮。源 MA 本身没有额外 ExternalLink 子图标；现有通用 system_button 的 outline 外观和额外图标不能直接当作源样式通过。使用 Base Button 保留键盘/焦点语义并提供上述外观即可；不能因为源是 div 就放弃本地语义控件。

### 原 Windows 图标资源已补足只读验证

515 本地 `.ref/devices/515/static/media/` 缺少这两个通用 SVG。本轮按其当前 asset-manifest 的精确 URL 只读下载到内存，均返回 HTTP 200；没有写入文件，也未执行下载内容：

- `https://apps.razer.com/synapse/products/515/ui/static/media/common-windows-11.d477cadb.svg`：350字节，SHA-256 `aa3676b09d4555dbcfdcb81e8d2413fb0b958aebf49207fae94e1108d8596bba`，与现有 `assets/synapse/windows-11.svg` **逐字节相等**，可复用。
- `https://apps.razer.com/synapse/products/515/ui/static/media/windows_logo.8fb1e7e2.svg`：622字节，SHA-256 `d3a5cbb29c8224935f19a9bf726d04e898f2901ec693d27bc8c72785da05a0c3`，与当前 `.ref/devices/182/static/media/windows_logo.8fb1e7e2.svg` **逐字节相等**，可从该已验证当前文件准备本地 legacy 图标。这里不是仅按相同文件名推断等价。

### 当前宿主契约与本地已有能力

- 当前宿主 `.ref/host-4.0.827/electron/main.js` SHA-256 `0e3b84c11dd3e6d06072f54fb3295404894afcd7d2f7cce814ac20d9cd7d074e`：OpenKeyboardProperties 在通用系统动作分派组（`[51866,53500)`），最终 `return io.callDLL(n,i)`（53421）。`io` 是 FFISysUtils。
- `.ref/host-4.0.827/electron/modules/sysutil/win/index.js` SHA-256 `f6bf22581ac7fce6e32ec1e8ff23bc66aec4a3c6b8e8d14a4a0d57fcbb4a3a0a`：声明 `OpenKeyboardProperties:["void",[]]`（`[1235,2207)`），DLL 注册名 sysUtilsNative（196）；callDLL（4441起）无该动作特判，最终交给 ffi.callDLLMain。这确认源是**零参数、无返回值的系统窗口命令**；未加载 DLL，不能据此声称已查明 native 内部具体启动程序或执行成功。
- 系统版本走独立非 DLL 路径：产品包装器 `[1792037,1792260)` 发 getWindowVersion，宿主 main `43169` 直接调用 go()；`electron/lib/common.js` SHA-256 `b7bfa2eec3da3d20e33add539e6606338c27aa6484bf0c7097a13959a3374dce`，`[4882,5430)` 根据 os.release 缓存版本，其中 Windows 10.0/build>=22000 为 "11"，否则 "10"，也处理旧版 Windows。不要把打开属性窗口误写成读取版本的前置条件。
- 本地 `backend/system.rs:24` 已有 is_windows_11()，只读 CurrentBuildNumber>=22000；读取失败返回 false。可在保留的界面状态中读取一次选择相应图标，不必在 render 内重复查询。它的布尔 fallback 不是完整版本或已成功读取状态；本轮没有执行该查询。
- 本地 `backend/system.rs:177` 的 open(Properties::Keyboard) 已通过 `control.exe` + `main.cpl`、`,@1` 参数启动系统属性；`device_pages.rs:18` 的 system_button 包含失败 notification。另一个 DeviceWorkspace 的 `customize_page.rs:570` 已使用这个后端，但515实际走 KeyboardProductWorkspace，未经过该调用点。因此应复用现有命令/错误处理边界，补当前产品的独立面板和语义按钮；无须为此另加厂商 DLL ABI。该本机命令是本地既有实现，不是从上述 native DLL 源中提取出的命令行，实际窗口行为仍未运行验证。

L 的实现缺口仍是515当前产品未挂右列及操作入口。可实施材料现已包括精确参数、原图标、可见条件、文案、布局、系统动作和现有失败路径；这不等于代码已接入或已通过运行/视觉验收。点击该入口应只打开系统属性，不修改本地草稿，不产生虚构的设备写回或读取成功状态。

## K 最新实施独立回读：515 Snap Tap 本地编辑主体已接入

本节更新原 K 的当前状态，保留前面的缺项和实施建议作为历史依据。已读取新增 `src/features/keyboard_snap_tap.rs`、数据文件、`keyboard_products.rs` 及 `shell.rs` → `product_workspace.rs` → `source_workspace.rs` 的实际转发链。`snap-tap-current-evidence.json` 的 **34 项 AST 收据**均按当前源文件 SHA-256 和 UTF-16 切片重新比较；**113 条 CSS 收据**与维护工具静态解析结果逐项相等，记录的 keyframes 切片也相等；**3 个 SVG**源文件、输出文件和记录 hash 相符。此过程没有执行应用、构建、测试、供应商 JavaScript 或 DLL。

### 已回读的可见控件和状态逻辑

- 仅产品515初始化 Snap Tap retained State，普通 Customize 的左列现在实际调用 `snap_panel`，不再依赖另一套 DeviceWorkspace 的未挂载控件。主开关、成对键槽、非首组删除、四组上限的添加按钮、重复/禁止键提示、成功消息和调整模式提示均已进入该产品的渲染路径。没有强制展示515调用方未传入的 FN + L SHIFT 快捷键提示。
- State 初态使用当前 `ha` 的 `isEnabled=false` 与 A/D、id=1、mode=LAST_INPUT，未拿静态 profile 内另一个 snapTap 对象的 true 作为真实读数。Pair 的附加字段被保留；运行时 pressedKeys 未混入持久化 Configuration。第一组隐藏删除按钮；新增/删除/切换仍明确是本地操作。
- `accept` 在 KEY1 有效时只改 staged 并转 KEY2；KEY2 有效后才提交完整列表，回 READY 并显示约3秒 Success。禁止键及跨槽重复检查会保留修改前的 staged，不把拒绝的键写入列表；Pause 后的 NumLock 抑制也存在。未发现把 KEY1 立即覆盖已提交列表或允许重复键落入配置的确定性错误。
- `remove` 在修改 editing/message 前计算 warning 回退条件，保留源 q 回调读取旧值的顺序；普通删除过滤并重编号。`blur` 清除不完整及 warning 所在组，只有一组且过滤为空时回退 config。关闭开关时 warning 删除/回退路径存在，普通录入状态则随关闭暂停；这与 ql 的 V 分支相符，不应一概改成关闭立即清空录入。
- `snapshot` 以 config 为基线，只有 KEY2 才复制当前组 staged.key1，并过滤不完整组，符合 ql beforeunload 的投影规则。KEY1 更新后发本地 change 并不等于逐键提前提交：发出的保存对象正是这一投影。`restore` 保留顶层 `_snapTapLocalV1`，重建 State 并清除录入、成功定时器及提示；产品内部页面切换先执行 `leave_snap_tap`。未发现恢复时丢弃该局部标记或将旧 profile 的录入继续写到新 profile 的路径。
- 窗口失活观察调用上述 blur 清理；外部点击用实测 pair/add 边界判断，不完整组仍留 warning。新增 AppShell 根捕获只转发到当前 `Location::Device`，再由当前 Customize、enabled、非 READY、无调整提示的条件限制；`snap_key_up` 在处理前 stop_propagation，避免根和面板把同一 keyup 录入两次。这样无效外部点击移走键槽焦点后仍有输入入口。事件实际传播、首帧测量和键盘焦点尚未运行验证。
- 键槽采用外部最小64×44、2px边框，对应源 content-box 的最小60×40；源只有 min-width，没有应强制的 max-width。成对间距10、删除按钮额外左距20和20px原图、64×44添加按钮、提示色及1秒闪烁均有静态对应。调整提示当前为400px宽、25px图标、20/30px内距，并以底部落在 viewport 中线对应源 translateY(-100%)。这些局部对应不代表完整页面像素、文本折行或弹层行为已经验收。

### 仍保留的范围限制

- 原生 keyup 适配只覆盖无歧义子集，包括字母、部分导航键、空格/退格/Tab/Escape/Menu 及 F1–F24。数字、标点、Enter、左右修饰键、小键盘和部分专用键仍未完整录入；未知事件被丢弃，没有用 keyCode=0 冒充有效按键。当前 GPUI Windows 输入将部分原始位置区分折叠，不能靠猜测补全。本轮指出原局限提示范围偏窄后，主线程已改成“本地草稿，尚未写入设备。部分按键的精确录入尚待接入。”，已独立回读；文案更准确并不代表其余按键适配已经完成。
- `SnapTapObservation` 具有 Configuration、Layout、AdjustmentMode、InputRedirect 的分离入口；InputRedirect 核对类型、奇数 release flag、scancode/outputFlag，并仅在 razerKey 时使用专用键 fallback。当前没有真实 middleware 发布者连接，尚不能宣称完整录入、实时布局/调整状态或设备配置读取成功。Configuration 观察不会静默覆盖显式本地草稿；本地发布仅写 `_snapTapLocalV1` 并走既有 profile snapshot/capture，没有 DLL 写回或设备保存成功。
- 产品内部 TAB 和 profile restore 清理已回读；宿主 `navigate` 只关闭 profile 弹层，未调用 `leave_snap_tap`，因此从该设备切到 Dashboard 再回来可以保留未完成录入。根捕获限制确保隐藏时不会继续消费键。此项只记录本地行为；本轮未证明当前宿主标签切换会卸载源 ql，因此不将保留状态直接列成源不一致缺陷，也不将其记为已经验收的卸载清理。

原 K 的“面板与增删编辑主体完全缺失”已经静态修复；当前结论仍为 **partial**，完整按键身份、真实只读发布者、服务捕获暂停/恢复、窗口及像素验证仍未完成。不得把本地编辑器主体存在等同于设备 Snap Tap 已接通。

## L 最新实施独立回读：515 Keyboard Properties 右列与系统操作已接入

已读取新增 `src/features/keyboard_properties.rs`、产品初始化与实际 Customize 两列组合；`keyboard-properties-current-evidence.json` 的 **12 项 AST 收据、66 条 CSS 收据及2个SVG**均与当前文件重新匹配。本节更新前面 L 的实施前结论，未执行系统属性命令或读取本机版本来验证。

- 515初始化时保留图标选择，使用既有 `is_windows_11` 的只读判断；失败回退 legacy，不在每次 render 查询。普通 Customize 的右列已实际挂 `keyboard_properties`，标题、帮助和动作均使用已验证 locale 键；没有误加 analog 的 Game Controller 行或 windowBigIcon。
- 44×44原Windows图标、20px间距、44px高文字行、下划线、灰色/绿色hover和active透明度均有实现。两个输出SVG与515当前原资源逐字节相等，不再只是引用其他产品的同名文件作为依据。通用面板与help_control的完整定位、文字排版仍保留原局部验证限制。
- 语义按钮只调用现有 `system::open(Properties::Keyboard)`，失败进入 notification；没有调用 `write`、发 KeyboardProductChanged 或改变 profile/snapshot，也没有宣称设备读取、写入或保存成功。该本机命令与源零参数 OpenKeyboardProperties 的用途一致；厂商 DLL 仍未加载，本地 control.exe 实际窗口效果未运行验证。

原 L 的右列及操作入口缺失已静态修复。本结论只覆盖这一面板的挂载、资源及动作边界，不把515整页、帮助边界或系统窗口运行结果标为完成。

## N–O. 226 Performance：当前 DPI 与 Polling Rate 独立复核

本轮仅检查 **226 Basilisk V4 Pro 的普通 Performance 页**，不扩大至其他鼠标，不修改主线程正在实现的滚轮区域，也不运行应用、构建、测试或 DLL。已重新读取当前主包及8355（完整 SHA-256 见 M 的当前源表），并把 `mouse-page-source.json` 中226 Performance 的24项组件收据逐一与当前文件 SHA-256、UTF-16切片比较，全部相符。下述结论只来自实际读取的 DPI / Polling Rate 分支；这24项中同时出现的 Sensitivity Matcher 等组件不因此自动通过行为审查。

另读取当前 `.ref/devices/226/static/js/8901.207fa138.chunk.js`，SHA-256 为 `bc4636e381f90c81d757a7d2143bf2f6b534d72a690122e4610131296d7fbf5f`，静态解析出数字框模块4230的 `[5290,11084)`。以下源范围均为 UTF-16半开区间。相关 CSS 只静态解析：main.c2099849.css 的 SHA 为 `68bc9ccdc21fe548330106b3a27beb5049be06f55939e5950677b220f52703df`，8355.f94d4299.chunk.css 为 `ec3c1904f52ffa472fbcaca33ec514681ce2ea2d329279205c0c36ade329c53a`。

### 已证实的具体挂载与条件

8355 `[275451,275518)` 的导航实际挂 `Ri({isBle:this.props.isBle})`；Ri=wi `[183668,183990)`。左列直接挂 ys/Ms，ys `[154857,155307)` 从 dpiStages reducer取得 active、enable、stages、enableStages；Ms `[149688,154848)` → cs/ls `[143618,148820)` → ts/es `[139629,143485)`。该调用没有 useTwoWayTab、noHeader，不能从共享方法推定本页应新增两种模式标签或阶段数量下拉。

右列的 Xs/zs 只有 `!isBle || DeviceInfo.supportBluetoothPollingRate === true` 才挂载；226当前CONFIG模块1057 `[54515,68130)` 明确 `supportBluetoothPollingRate=false`。因此本产品BLE模式下整个 Polling Rate 面板应隐藏。该模块还声明 minDPI=100、maxDPI=50000、dpiStep=1、supportXYDPI=true、isSensitivitySliderWithGrid=true、dualLinkPollingRateLimitHz=1000。8355的 `z=s(1057)` 与 `O=s(4693)` 均已重新读取，未把其他模块重名符号当作参数来源。

本地仍由 `source_workspace.rs` 的 MouseProductWorkspace 分支挂载，Mouse不走 supplement；`mouse_products.rs::render` 的 TAB_PERFORMANCE调用 performance。226没有命中只对70启用的 dpi_rows_70 / dpi_number 分支。以下缺项不能由70专用实现已经存在来抵消。

### N. P2：226 DPI 阶段操作、分段滑条和数值归一化未匹配当前根

| 当前已证行为 | 本地偏差及具体影响 |
| --- | --- |
| stages开启时，Ms把完整 stages交给ls，每组都有可编辑X/Y数值、阶段序号、visible开关和拖动；关闭时只交当前active组，且one-stage的开关/拖动由CSS隐藏 | performance只显示所有序号按钮和当前一组滑条；阶段主开关关闭后仍显示并允许点击全部序号。默认active=2时关闭阶段，再点5，本地仍把active改成5；源关闭态没有这个切换入口，保留当前组可编辑 |
| es按visible重新计算可见序号；visible=false的行不接受序号选择，滑条禁用而数字框仍可编辑；只剩两个visible时禁止再关闭一个 | 本地全部按钮只按数组slot+1生成，不消费visible，没有逐行开关或“两组下限”。恢复合法的visible=false组后仍可通过其按钮选中并启用滑条；不能只降低透明度来修复 |
| Ms.setEnableStage关闭当前组后，先选后方第一个visible，否则首个visible；ls.onDrop根据原顺序移动整组并调整active，Ms.updateStages还修复选中隐藏组的情况 | 本地没有开关、拖排及对应active调整入口。snapshot虽然保留这些字段，仍不等于这些操作已经实现；不应把visible开关当作直接删除数组元素 |
| 226的Qt=true，es确实选用Wt/Ft `[133558,139374)` 的分段DPI滑条；CONFIG没有SENSITIVITY_RANGE_VALUES，因此使用Ft内默认分段 | 本地add_range/SliderState直接把100–50000线性映射。源100/500/1500/10000/15000/50000分别位于0/15/30/45/70/100%；例如800在源约19.5%处，本地约1.4%处，低DPI操作区间明显不同。只让min/max/step相同不能算匹配；源刻度、分段映射和thumb提示均有实际挂载 |
| es传4230整数模式、六位、min100/max50000/step1；空串或单独负号可暂存，blur统一parse/clamp并回填规范文本，Enter/Escape触发blur | 本地非70输入没有同等约束。空文本失焦时parse失败直接返回，保留空框和旧草稿；输入99后失焦会把草稿写100，但当前InputState未回填，仍可能显示99。原因是write_number对非70跳过同path控制项同步，之后只同步SliderState。需同时归一化数据与当前文本 |
| 4230数字框有上下步进、按住每300ms重复、注册后的滚轮及箭头操作 | 本地226只挂普通Input；这些源操作不由已存在但只用于70的数字编辑器覆盖。应按226现用4230与参数单独确认复用，不直接宣布所有鼠标共用实现通过 |

以上阶段条件来自当前Ms/es，未从70借用。源 `.stage.one-stage:hover .icon-draggable,.stage.one-stage:hover .switch` 在main CSS的242803处明确display:none；`.stage.off`的淡化规则在243953处。Ms里虽然有changestageNumIndex方法，实际render没有挂阶段数量下拉，本轮**不将数量下拉列为缺项**。同样，关闭stages不是把整页DPI输入禁用：源仍允许编辑当前组。

窄项一致：本地226的100–50000、step1、支持独立Y参数与当前CONFIG相符；write_number在independent=false时写X会同步Y；toggle关闭independent又调用write_number把Y恢复为X。该XY同步已存在，不报告为缺失。源修改其他visible行会选中该行、修改不可见行保留当前active；本地当前只显示选中行，未来补多行时还必须移植这一条件，而不能只复制现有write_number的70判断。

源还区分`dpiStages.enable`与`enableStages`，后者是来自reducer的整块交互门控。当前226本地没有这条观察；本轮没有穷尽会将enableStages置false的真实发布者，故把它记为待接只读状态条件，不声称已观察到设备处于锁定状态。

### O. P2：226 Polling Rate 未接连接条件、目标字段及限速说明

实际组件zs `[157662,165209)`、connect包装Xs `[165218,165578)` 读取isDongle/isBle及三个独立rate字段。当前 `mouse_products.rs::polling` 固定取 `rates["POLLING_RATE"]` 和 `spec.polling_path()`；226此路径总是`/pollingRate`。MouseProductWorkspace构造只收product_id，restore也没有连接状态参数；SourceProductWorkspace中的`device.use_ble`仅传给Controls等其他分支，没有传给Mouse。这不是字段存不下，而是实际渲染和写入口没有消费它们。

- **BLE整块隐藏缺失：**226的wi门控已证实；本地performance无条件追加polling。因此不能因为生成数据里存在POLLING_RATE_BLUETOOTH数组，就为226新增BLE频率按钮。那个数组在此正常根被外层条件挡住。
- **Dongle与有线写入目标不分：**source `getPollingRateStateKey/getSetPollingRateAction` 在dongle模式选pollingRateWireless，否则有线选pollingRate。本地无论状态都更新`/pollingRate`。比如已恢复pollingRate=1000、pollingRateWireless=4000的dongle配置，源选中4000，本地选中1000，点击还会修改有线字段。两种模式允许的基础六档刚好相同，并不能证明owner也相同。
- **有线标题与提示缺失：**普通Xs未传特殊props；zs在有线模式用WIRED_POLLING_RATE_HEADER / WIRED_POLLING_RATE_V2_TOOLTIP，无线用POLLING_RATE_HEADER / POLLING_RATE_V2_TOOLTIP，并总有POLLING_RATE_DESC。本地固定普通标题，没有上述帮助和正文。当前模块4693的lGq、orU、FGZ、iiK、rJp及其literal已按绑定重新解析。
- **大于1000Hz的说明缺失：**未处于限速时，source按当前模式rate>1000显示POLLING_RATE_WARN（无线）或POLLING_RATE_WARN_NOBATTERY（有线），附LEARN_MORE链接 `https://www.razer.com/technology/razer-hyperpolling#best-practices-tips`。本地可选择2000/4000/8000，却没有相应说明。source并不因此禁用所有高频选项，修复应显示说明而非无条件禁止高频。
- **DualLink条件限速未表达：**zs根据当前产品226/227与duallink-devices中master/slave匹配；isDongle且当前链未连到高频master时，使用CONFIG的1000上限。对于supports8KHzPollingRate的多设备dock，Ls `[157017,157399)` 另要求isDongle且同master至少两项，才降至1000。达到上限后保留完整按钮集，>1000的按钮在呈现和onClick两处禁用，并显示相应DUAL_LINK_LIMITED或MULTI_DEVICE_DOCK_LIMITED_MOUSE说明；本地没有这些状态、条件或写入口保护。本轮只确认条件代码，不声称本机已有此dock或已经读取到这些状态。

源applyPollingRateLimit在既存wireless值过高时还发setPollingRateWireless降档请求；这是与设备写回有关的后续整合边界。当前可先实现真实观察驱动的禁用/说明及明确的本地草稿约束，不应在只读接口工作中执行设备自动降档，也不能把显示限速当作设备已降档成功。连接身份需从已证来源传入，不能把“数据尚未读取”默认为已确认wired或dongle。

已排除的误报：226当前CONFIG没有POLLING_RATE_8K_FW_VERSION，故zs中`ks && ...`固件门控不在此产品生效；基础有线与无线数组均确实含8000，不能凭共享HYPER_POLLING_RATE只有4000而删去8000。普通Xs没有传supportInGamePollingRate，虽然共享Ts与inGamePollingRate reducer存在，**不能据此要求226本页展示in-game区**。本地六个有线档位125/500/1000/2000/4000/8000本身与源一致，未发现基础有线选项数字错误。

### 草稿、读取和复用边界

当前默认profile含五个400/800/1600/3200/6400阶段、active2、enable=true及三个rate字段；source主包的reducer临时初态另有enable=false、不同阶段值及wired125。8355 `[270790,271959)` 的实际配置加载会分开派发dpiStages、pollingRate和pollingRateWireless观察。故本轮不把静态profile与reducer初态不同另报成“读到错误硬件默认”；它们属于不同owner。

本地write/write_number→MouseProductChanged→SourceProductWorkspace.capture仍更新活动profile.source_settings，snapshot克隆完整draft，restore保留已有顶层值；上述界面缺项不意味着所有未显示字段被删除。写回当前InputState的缺陷则会使屏幕文本与这份本地保存对象不一致，应优先修复。没有进行实际文件往返或设备读取验证。

复用70的行、数字框或本机系统按钮可以减少实现工作，但需对照这里的226小写schema、visible条件、分段滑条及连接门控分别适配。N/O是两个具体区域的未完成项，不代表226整页已经审完；Sensitivity Matcher、Mouse Properties、全页布局、辅助功能、真实只读发布者和窗口行为仍不在本轮通过范围。当前locales已具备这里多数标签；MULTI_DEVICE_DOCK_LIMITED_MOUSE只有en.json包含，其余九种语言缺项，实施时需要沿当前源资源及既有fallback处理，不能捏造翻译或设备能力。

## M 最新实施独立回读：226 三模式滚轮编辑器

已读取新增 `src/features/mouse_226_scroll.rs`、MouseProductWorkspace接入、`tools/prepare-mouse-226-scroll.cjs` 和生成数据；将 `mouse-226-scroll-current-evidence.json` 的 **52项源码收据**逐一与当前文件SHA-256、UTF-16切片比较，**83条CSS收据**与维护中的静态解析结果比较，全部相符。另只读查看当前使用的gpui-base SliderState与现有SourceSlider/check_item实现，以确认事件和样式实际含义；没有运行应用、构建、测试、DLL或供应商JavaScript。本节更新原M的缺项状态，不覆盖N/O的Performance问题。

### 已实现且静态一致的部分

- 编辑器仅在product_id=226创建，实际Customize追加该实体；旧的两个通用Checkbox通过`scroll_editor.is_none()`条件避开226，没有形成两套控制同一字段的入口。三个模式及顺序与当前ge一致：Tactile、FreeSpin、MicroTactile；对应选择、禁用勾选、说明和两组0–4等级滑条均已挂载。
- `select_mode`拒绝选择disabledModes中的项；`toggle_disabled`允许移除已禁项，只在少于两项时新增。禁用当前模式时按ge顺序寻找首个未禁模式，先写本地scrollMode再写disabledModes，匹配fe的即时回退顺序。第三个未禁勾选项有disabled呈现且写入口再次检查两项上限，不能通过回调继续禁用全部三种模式。
- FreeSpin被禁用时，`active(feature)`派生为false，开关显示关闭、滑条不可操作；两个原enabled字段及level没有被重写。解除FreeSpin禁用后会恢复保留的enabled状态。主开关写入口同样检查locked，符合Se的`enabled && !forceDisabled`与点击保护；原M“合法恢复状态仍可修改锁定功能”的问题已修复。
- 两层disabled透明度已经按实际树检查：外层swtm-slider-wrapper为0.4/200ms，内层SourceSlider为0.3/300ms，稳态合成0.12。因为本地标签是SourceSlider的兄弟节点，另给标签0.3，使它们也合成0.12；不是额外叠了第三层。源锁定switch是同一元素opacity0.3，SynapseSwitch也只有这一层。check_item保留自身9px底距，外层8px gap与当前CSS共存，不应误删其中任意一项。
- SliderEvent::Change只记录previewing并更新显示，未edit/emit Changed；只有存在预览的Release把0–4整数写进draft并发布。已读gpui-base 0.7.1：拖动/点击轨道发Change，真实交互释放发Release；程序`set_value`只notify，不发Change。故restore、观察同步和失活复位不会冒充一次本地等级编辑。SourceSlider保留无tip的36px高度和轨道/滑块；窗口拖动、触摸和像素结果没有运行验收。
- 锁定开关及slider外包装有悬停跟随指针的ENABLE_FREE_SPIN_TO_USE_FEATURE提示，采用源16/12px偏移、320px最大宽和当前颜色。该tooltip与全页help_control的真实边界/层叠未作窗口验证，不据此宣布完整视觉完成。

### 本轮发现并修复的草稿字段归属问题

初稿的snapshot只保存完整draft，restore则把已有scrollWheel对象的六项字段全部认作显式本地覆盖。但SourceProductWorkspace.restore_active首次就会把补齐默认值的snapshot写入profile.source_settings。因此即使用户未编辑，切换profile再回来也会将所有默认值标成local，后续真实观察无法再显示。这是可由本地状态链确定的恢复缺陷，不是设备暂未连接造成。

主线程已加入根级 **`_scrollWheelLocalFieldsV1`**，本子任务已独立回读修正：新建/无保存值恢复写空数组，Changed同时保存draft和显式字段集，snapshot保留该标记；restore优先按标记恢复字段归属，只有没有标记的旧数据按原有字段作兼容处理。即使用户明确选择了与默认相同的值，Changed也会保存该字段标记，不再因数值相同被parent.write的去重丢掉。自动补的默认值因此不会在再次恢复时升级成用户覆盖；上述问题已静态修复。

纯观察只写observed并notify，不发Changed；`snapshot`只返回draft，未编辑的运行时字段不会进入本地profile。显式编辑仅将对应字段加入local_fields，其他字段仍可显示新观察。restore清空旧profile的observed，要求新profile重新取得真实观察，不能以旧profile的读数填充。产品/源工作区已有ScrollWheelObservation转发入口，但真实publisher仍未连接，不能把入口存在描述为DLL读取已成功。

### 切页与剩余范围

普通产品TAB切换会调用`scroll_editor.deactivate`；restore、窗口失活、功能关闭或FreeSpin禁用也清掉previewing/tooltip并同步已提交等级，未把未释放预览写入snapshot。已确认一处接入边界：SourceProductWorkspace.select_body_page遇到Help role会在MouseProductWorkspace.set_page之前return，因而Help切换没有走这条deactivate链。此处应记为未覆盖的瞬态清理路径；本轮没有运行复现残留tooltip/预览，不把普通TAB已清理扩大成所有页面切换已通过。

三模式、禁用回退、两组等级、保留enabled、释放提交和本地字段标记的主体已静态补齐；原M不再记作主体完全未实现。当前仍为partial：真实只读观察/发布者、窗口输入与完整视觉验收未完成，设备写回仍后置。源fe的300ms模式请求debounce属于后续服务请求，本地即时选择保留为明确草稿，不报告虚构硬件切换或保存成功。

### M/K 补丁续核：Help清理及滚轮观察范围

已窄范围重新读取SourceProductWorkspace、MouseProductWorkspace、keyboard_snap_tap与mouse_226_scroll的最新补丁；本子任务没有运行cargo、应用、测试或DLL。主线程另报告最新cargo check通过，这不是本子任务执行的检查。以下结论覆盖前述Help清理缺口及观察处理，不重做或扩大整个产品验收。

- `select_body_page`现在于Help分支return之前，分别调用Mouse的`dismiss_scroll_editor`和Keyboard的`leave_snap_tap`。鼠标路径清掉预览与tooltip并回同步已提交等级，未发Changed；键盘路径按已有本地snapshot策略结束录入、清消息/提示/定时器。普通导航与页面历史都先更新父层page，再进同一个select_body_page，所以两种到Help的路径均覆盖。前述“Help早退未清理”已静态修复，不继续记为未修缺项。
- `SourceProductWorkspace::captures_snap_keys`明确要求当前页面存在且不是Help，再询问键盘录入状态；keyup入口也先使用同一门控。即使retained键盘子实体仍记着原Customize页，父层Help状态不会继续接受Snap Tap按键。未发现该补丁只挡keydown却漏掉keyup的问题。
- 滚轮`observe`不再对每条未覆盖观察统一调用deactivate：Mode观察不触碰等级预览；等级观察只同步对应slider；enabled/disabledModes观察只有令某功能不可用时才取消该功能的预览并回同步等级。因此Smart Reel状态变化不会无条件清掉仍可用的Acceleration预览。FreeSpin解除锁定会清除锁定tooltip；local_fields覆盖的观察仍不改局部显示或草稿。
- 等级观察本身不清previewing，是同步当前值而非取消整次拖动；当前模块130的componentDidUpdate只在调用者设置cancelMouseUpOnValueChange时才取消，而226 Se未传此prop。已再次把该模块收据与当前2306文件比较。未发现最新处理新增确定性差异。整个观察入口仍不调用edit/Changed，纯runtime值仍不进入snapshot。

当前M的两个已提出接入问题——字段归属恢复和Help瞬态清理——均已静态修复；真实publisher、运行与视觉限制仍保留，不把补丁回读等同于完整设备通过。

### 下一独立小批次建议：226 DPI整数输入的提交归一化

优先修N中“输入文本与保存值不一致”的窄项。它已有226实际es→4230证据，不依赖连接观察、分段轨道或完整阶段列表，可以独立评审和静态验证。范围限226的`/dpiStages/stages/{slot}/x|y`：

1. 接受源允许的临时空串/单独负号及不超过六位数字，拒绝小数和任意字符；提交时按min100/max50000/step1归一化。
2. Blur统一把规范值写回当前InputState、SliderState与本地draft，即使最终数值与原值相同，也必须规范文本。静态追查的代表输入为空串、`-`、`99`、`60000`、`00100`。
3. Enter和Escape按4230触发blur；这里Escape会提交规范值，不能擅自变成撤销。保持非独立Y时X同步Y和既有本地snapshot链；切profile不得把旧输入内容提交到新profile。

现有70数字框可作为结构参考，但其step_dpi_number仍硬编码大写`Y/Independent`，不应只把product_id条件扩成70或226就宣称可复用。此批先收敛提交/文本一致性；阶段多行、visible/拖排、226分段滑条、stepper完整操作和Polling Rate各保留为后续独立项，N/O仍为partial。验证继续采用当前源码/参数解析与实际事件链回读；不运行被禁止的测试或应用。

## N 数值编辑器实施独立回读：226 整数提交及共享步进

本轮读取`mouse_products.rs`、`mouse_dpi_number.rs`、Help接入、`audit-mouse-226-dpi-number.cjs`及生成收据；另读取Cargo.lock实际使用的gpui-base 0.7.1、gpui-pre 0.3.8相关事件实现。未运行应用、构建、测试、cargo或DLL。下面是源与实际事件链的静态结论，不是窗口操作录像或全页验收。

### 当前源证据与已确认修复

最初的4230收据由通用模块查找器取自MapKeyboard chunk，不能只因模块编号相同就作为Performance实际加载证据。本子任务提出后，主线程将工具限定到当前`8901.207fa138.chunk.js`，4230函数范围为UTF-16 `[5295,11084)`（先前`[5290,11084)`包含属性键）。新增主包普通default根`[547879,548029)`收据，明确lazy加载8901、8355并进入4125。最新 **16项AST收据、55条CSS收据、2个SVG资源**均已独立按源SHA-256、UTF-16切片、静态CSS解析及资源字节比较，全部相符。继续使用当前8355的Ri→wi→ys/Ms→cs/ls→ts/es挂载链，不以MapKeyboard副本代替本页来源。

- 数值编辑器只扩展到70和226的DPI路径；226实际range已挂载DpiNumber。新增226使用本身的`x/y/independent`及100–50000、step1，没有再硬编码70的`Y/Independent`。70仍由原dpi_rows_70挂载，沿用自己的schema、边界、步长及可见行选择逻辑；没有将226当前仅显示一行的保护错误加给70全部行。
- 输入验证接受不超过六位数字、可选前导负号以及临时空串/单独负号；拒绝小数及其他字符。`mask_pattern(None)`确实令底层`mask_pattern_set=true`，所以NumberInput.render的`ensure_number_mask()`不会另套默认数字mask。`InputState::set_value`内部明确关闭emit_events，程序规范回填不会发Change而把typed再次设为true。
- Blur先parse、按步长向上归整并clamp，再结束注册/重复步进，写本地draft，并**无条件回填当前InputState**及SliderState。因此最终值与既存slider相同也不会保留非法或带前导零文本。Enter由InputEvent订阅blur；普通Escape经底层action传播后由DpiNumber的keydown blur，二者均提交规范值。这里不把Escape写成取消。

| 静态追查输入/动作 | 当前226结果 |
| --- | --- |
| 输入空串或`-`后Blur | 规范为100，当前文本、slider及本地draft一致 |
| 输入`99`后Blur | 规范为100；不再只保存100而留下99文本 |
| 输入`60000`后Blur | 规范为50000 |
| 既存值100，输入`00100`后Blur | 即使数值未变，文本仍回填100 |
| 输入`800`后Up，当前行值因此改变 | 按源JS字符串加法先得8001；随后状态成为数值，再Up得8002 |
| 点击注册后按箭头或键盘步进 | 仅更新当前数字框/slider预览；Blur才进入write_number与本地snapshot链 |
| 未注册的箭头步进 | 即时进入本地write_number；仍不调用DLL |

`typed`不仅描述是否编辑过，还用于复现4230中字符串与数字的区别：如果计算后行值不变，源componentDidUpdate不会无条件规范字符串，本地也保留typed到Blur。非独立Y时最终X提交仍同步Y；注册预览本身没有提前选择其他阶段或写profile。226暂时只显示当前行，step入口明确拒绝已退出当前行的旧回调；这只是现有一行实现的边界保护，不算完成多行编辑。

### 步进事件、清理及本轮新发现

已静态核对鼠标按钮重复事件：装饰器通过trait on_click先注册抑制标记，Base.NumberInput的语义on_click随后追加；gpui-pre依插入顺序执行同一元素click listeners。故按下已步进后，释放click会消费suppress_click，不会再加一步。自定义on_step替换NumberInput默认apply_number_step，键盘上下键走同一源步进函数。按住每300ms重复，释放、移出、结束编辑或离开窗口活动态终止。边界仍有UI呈现与窗口命中区域未运行验收，不能仅以clamp宣布交互像素完全一致。

普通切页、Help早退分支均已接`dismiss_editors`：丢弃重复Task/registered/typed，并把未提交数字预览恢复成draft；restore同样先换新profile draft、重置编辑态，再将新profile的整数同步给保留的Input/Slider实体。清理本身不发Changed。对有效profile字段，即使稍后处理旧Blur事件，处理器也是读取已经更新的InputState，未发现把旧文本直接携带到新profile的路径。未进行窗口事件时序或实际文件往返验证。

本子任务另外确认wheel分支存在两个窄项差异，已反馈主线程：

1. **滚轮限位owner不对。**源4230.sliderOnMouseWheel先以props.value（当前行预览）判断方向边界，才调用volumeUp/Down。旧本地只检查焦点和注册，直接从文本步进。当前行50000、输入1000未Blur时向上滚，源不步进，旧本地会变10001；当前行100、输入800后向下滚，源不步进，旧本地会变799。主线程已补wheel专用SliderState边界判断，本子任务已读取该补丁；不应把此保护加到键盘/箭头上，因为源区分这些入口。
2. **未注册仍吞页面滚动。**通过Tab聚焦但未点击注册时，源没有window mousewheel监听；旧本地虽然不步进，仍在闭包外无条件stop_propagation。应让未注册分支直接返回，不消费页面滚动。后续修复回读结论见本节续记。

仍保留一个已知范围差异：源在点击注册后监听window mousewheel，本地监听DpiNumber命中范围内的scroll事件。焦点保持在输入框而指针移到框外时，源仍能响应，本地尚无同等全窗口入口。因此本轮可把N中的整数规范提交和已挂载的按钮/键盘步进标记为已静态补齐，**滚轮完整覆盖仍为partial**。N中多行阶段、visible/拖排、关闭阶段后的布局与226分段滑条，以及O中连接条件/频率字段/说明均继续未完成；不能把这次数字框补丁扩大成226 Performance全页通过。所有提交仍是本地草稿，未发生设备读写或设备保存成功验证。

续记：已回读最新wheel补丁。闭包现在返回handled：未注册、owner不可用或slider缺失时不消费事件；注册后的方向边界返回true，消费滚动但不步进；其余才调用step。外层仅在handled=true时stop_propagation。因此上面两项本轮新发现均已静态修复，Tab焦点未注册时的页面滚动不再被该分支吞掉；框外全窗口滚轮仍保留为未实现范围。报告`git diff --check`通过，仅有Git的LF/CRLF转换提示；本子任务未运行cargo，主线程的编译检查另行记录。

## N 阶段行实施前续核：226 与70复用边界

2026-10-06续核。当前226普通Ri→wi→ys/Ms→ls→es挂载链仍成立，es、ls、Ms、ys四项收据已再次按当前8355指纹及UTF-16切片比较一致。本轮读取工作树`mouse_dpi_rows.rs`及共享数字编辑器，聚焦阶段行schema、隐藏/选中、XY回退及拖排；分段滑条的完整算法/CSS由另一个获授权子任务继续独立核验。此节是主线程实施前的约束，不表示新阶段行实现已经通过。

额外静态解析当前8355动作：Ss `[149247,149317)`、vs `[149318,149388)`、Cs `[149389,149426)`、Es `[149427,149470)`、Ds `[149542,149612)`、_s `[149613,149650)`；主包初态Pe `[184107,184421)`、reducer ve `[184673,186587)`。vs发Tme并携带stages/activeStage，ve确实更新两者；Cs发$xU，ve只更新指定行independent，保留active。因此下表的XY例外不仅依据render中的方法名，而有实际动作与reducer支撑。

| 复用点 | 当前226已证行为与应保留的边界 |
| --- | --- |
| schema | 70本地保存使用`DPIStages/Stages/DPIStage`、`X/Y/Independent/Active`；226使用`dpiStages/stages`、`x/y/independent/visible`。行visible与整个配置active是不同含义，不应全局把Active简单改成active。现有spec已提供axis/independent等路径，但行visible读取、drag及XY方法仍须逐处适配。 |
| 全局阶段开关 | `dpiStages.enable=false`只把实际active那一行交给ls，不禁止该行数字/XY编辑。开关、drag因one-stage规则隐藏；单行显示的可见序号重新从1计数，写目标仍是原active槽位。不能保留五个选择按钮，也不能用显示序号1覆盖实际active。 |
| 隐藏行 | visible=false仍留在数组及界面中，序号文字/颜色三角不显示，数字框和XY可操作，slider禁用。关闭最后两条可见行之一被禁用；重新启用隐藏行不改变active。减透明度不能代替禁用slider的事件保护。 |
| 数值编辑后的选择 | Ms.changeDpiValueX/Y提交时只对visible行将active设slot+1；隐藏行编辑保留旧active。注册的数字步进或slider拖动预览在ls内更新，直到最终提交前不走该active选择。226先前为单行编辑器加的“拒绝非current行”临时条件，多行接入后必须移除；阶段总开关关闭时仅current可编辑的保护仍应保留。 |
| XY开关的特殊选择 | 启用独立XY只翻independent；关闭时若x==y也只改independent。只有关闭且x!=y才另发vs，把y=x且active=slot+1，即使该行visible=false也选中。不能套用普通数字提交的“隐藏行不选中”规则。70本地当前dpi_toggle_xy的条件结构与此相同，适配字段即可保留，不能无条件重置Y或选中。 |
| 隐藏当前行的回退 | Ms.setEnableStage先找被隐藏槽位后方第一条visible；没有则找全数组第一条visible。隐藏非current行不改active。70本地visibility回退结构与之相同。 |
| 拖排及选中重映射 | ls以拖动开始时完整数组移动整组，隐藏行也参与。拖动选中行则active跟到新位置；其他行穿过active时按方向±1。例A/B/C/D/E、active=C3，B移到D之后得到A/C/D/B/E，active=2；选中行移到首位则active=1。70本地remove/insert及选中重映射可以保留，必须同步新槽位对应的输入/slider和编辑态。 |
| 拖排后的隐藏active修复 | Ms.updateStages使用与visibility操作**不同**的回退：只找active之后的visible，找不到直接设active=1，即使第1槽也隐藏。70本地drop当前正是find后方/map_or(0)，不能合并成统一“首个visible”助手。此状态可由隐藏行的XY回退选中产生，不能只按理想初态排除。 |
| count及多余入口 | Ms传给updateDPIStage的第三参数不被实际Ds接受；Ds只写stages/activeStage，不能据此新增count更新。正常根没挂阶段数量下拉、useTwoWayTab或noHeader，本轮不新增这些入口。 |

另有两项源细节需明确记录，不能仅凭70本地形状判断：

- 当前ls.onDragStart生成拖拽影子序号时读的是大写`e.Active`，而当前226的profile/reducer及ls行数据只有`visible`。因此此处源生成的序号文本为空；普通行es则正确用visible计数。复用70本地的正常ordinal影子或把该处直接换成visible，都不是当前源码原样行为。可单列为源本身的遗留差异，但不能给这种改动补造“源已证”的理由。影子的250×50、绿色底、移动位置及普通行drag-over上/下边框仍有明确CSS；不能因影子编号异常省略正常拖排。
- 226的`enableStages`是单独观察门控。Ms将disabled仅包住阶段body，主开关单独传disable，标题/说明保持；componentDidUpdate另外给`.stage input.slider`设disabled。70 cI将disabled给整个panel，两者禁用树不同。它不能与用户可编辑的dpiStages.enable合并，也不能把尚未接入的真实观察当作已读取设备锁定状态。

为避免错误复用，已将Ft的几个实际挂载点转交专门子任务：Qt/rs均读取226 `isSensitivitySliderWithGrid=true`；stage容器左右各-20px，large宽300px!important覆盖普通stage宽250px；Ft的Change经ls保持拖动预览、window mouseup再提交；分段位置到DPI用Math.round，数字框仍用ceil；四方向键被阻止。默认stage CSS隐藏slider-tip，独立XY时重开并由指针/按下状态切换X/Y标记，不能复用普通常驻数值tip。本节不重复宣布分段轨道完整通过。

当前原N/O的剩余实现状态仍以最近一次实施回读为准。下一步仅回读主线程新阶段行实现，继续只改本报告，不运行应用、构建、测试、供应商JS或DLL。

### N 阶段行新实现第一轮回读

已读取本次工作树`mouse_dpi_rows.rs`、`mouse_products.rs`、共享数字编辑器，以及`mouse_226_dpi.rs`的owner/preview/commit接入（完整轨道算法和CSS另有独立子任务审查）。`mouse-226-dpi-current-evidence.json`的 **24项AST、154条CSS、9个资源**已逐项与当前文件指纹/UTF-16切片、静态CSS解析和资源字节比较，全部一致。没有运行应用、测试、构建或DLL；下面明确区分主体通过与尚在修正的接入点。

- `performance`现在让70/226都走实际dpi_rows；226不再回到旧“所有序号按钮加一行”的通用界面。`visible_key()`基于当前profile schema选择70的Active或226的visible，axis/independent同样按本产品路径读取。新grid仅为226 DPI路径建立，70继续原slider；未把其他产品视为这轮已验。
- 阶段开关关闭后只保留实际active槽位，计数在过滤后进行，所以可见单行的展示序号为1，编辑路径仍保留原slot。visibility及drag入口只在阶段开启时挂载；XY和数字框仍在。隐藏行保留数组位置，badge内容为空、整体淡化，slider禁用但数字/XY可操作；可见两条下限在呈现与写入口都有检查。开启隐藏行不改变active。
- 关闭当前可见行时的后方/首个visible回退、普通数字提交只选中visible行、XY关闭且x!=y时同步Y并允许选中隐藏行、x==y只改independent，都与前述Ms及动作/reducer一致。共享stepper已去掉226“非当前行一律拒绝”的临时条件；仍保留阶段关闭只允许实际current和关闭Y时拒绝Y的保护。
- 拖排保留完整行对象、移动隐藏行、按移动方向重映射active。若重映射后的active隐藏，继续用“后方visible，否则slot1”，没有误用visibility操作的首个visible回退；count未被附带重写。完成后同步各新槽位Input/Slider并清除数字预览，grid也从新draft重置；不会把百分比position保存成DPI。
- 新分段轨道的position属于GridState自己的实体，MouseProductWorkspace.sliders仍表示实际DPI。Grid Preview只同步实际DPI模型和当前Input并notify，不emit MouseProductChanged；Commit才走write_number→选中/XY联动→SourceProductWorkspace.capture。外部数字步进或restore的set_value只令Grid观察并同步位置，不另造Commit。隐藏、未显示、Y已关闭或阶段拖排时收到旧grid事件会reset而不写草稿。
- snapshot继续只克隆本地draft；restore、Help、普通切页及阶段主开关变化会reset grid pending/pressed并清数字注册。未释放预览因此不进入profile.source_settings。阶段拖动的禁用标记已由独立`dpi_dragging`代替全局has_active_drag，并由StageDrag预览实体的release清理；不再将slider自身GPUI拖动误认为阶段拖排。

本轮已反馈两项收尾差异，后续补丁结论另续记：其一，拖动源行缺当前`.stage.drag-active`的绿色背景/覆盖层，拖拽影子省略了源即使序号为空也创建的圆badge；其二，drop旧回调只凭相同owner/stages/active不能区分同一workspace的两次restore，新profile恰好有相同阶段数组时可通过旧拖动检查。后者属于静态状态隔离缺口，没有运行复现，不以此声称用户数据已被改写。建议对reset/restore增加代际检查，确保旧drag只对发起时的profile状态有效。

原N的多行、visibility/两条下限、单行布局、XY及drop重映射主体已静态补齐。真实enableStages只读观察、完整窗口输入/视觉、O中的轮询连接条件和频率说明仍未通过；完整滑条结论以对应独立审查为准，不能把24项源码收据相符直接等同于整个Performance验收完成。

### N 收尾补丁终核：拖动隔离、视觉与OTFS编辑门控

已读取主线程最新补丁。取证工具新增当前Pe/ve后，**26项AST、154条CSS、9个资源**再次逐项通过源指纹、UTF-16切片、CSS解析和资源字节比较。另独立比对当前226 MW的`main.660230dedaa05e8b00fc.js`和`7846.b84800eaaf18ff1145e5.js`，SHA与`mouse-226-polling-source-review.md`一致；直接读取READY的`SET_ENABLE_STAGES = !isOTFSEnabled`及OTFS切换后true/false发布片段。此处只复核两个具体MW文件及这条来源链，不声称重新审核了全部89个MW文件或运行了OTFS task。

**前轮两个收尾缺口已静态修复。**`draft_generation`每次restore递增，StageDrag携带发起时generation，drop在原owner/数组/active检查外再核generation。同一workspace切到数值完全相同的新profile，也不会接受旧代际拖动的写入。`dpi_dragged_row: Option<usize>`同时标识实际源行和阶段拖动状态，并由预览实体release清空；slider自身的GPUI拖动不会把所有数字框当作阶段拖排禁用。

源行已加relative定位、`#44d62c33`背景以及最后一个child的`#44c62d33`覆盖层；hover/active亦固定相同拖动底色，未再被普通灰色hover覆盖。拖拽影子增加30px圆badge；226真实源编号为空时保留圆形而不捏造序号。上述确认覆盖元素及样式结构，层叠、指针命中与像素仍没有窗口运行验收。

新增`ProductWorkspace → SourceProductWorkspace → MouseProductWorkspace::observe_dpi_editing_enabled(bool)`只接收明确的观察值。Mouse仅对226保存`dpi_editing_observed: Option<bool>`，不写draft或发MouseProductChanged；None按照Pe的UI初态true呈现，代码明确不是成功查询。该运行时状态不随profile保存，不由dpiStages.enable推导；snapshot仍只克隆draft。70的`dpi_editing_enabled()`恒true，且忽略此226观察，不会被新增OTFS门控错误锁住。真实事件生产者尚未调用此入口，不能描述为DLL查询/订阅已经成功。

| 核验入口 | 锁定观察为false时的最新处理 |
| --- | --- |
| 数字输入与Blur/Enter | 呈现disabled；晚到的Blur在parse/write前检查门控并回同步已提交值，不把旧文本保存；Enter触发Blur后走同一保护。 |
| 上下步进/按住重复/已注册滚轮 | step入口同时检查页面、阶段拖动和编辑门控；观察到false时清注册/typed/repeat，预览回draft。 |
| Grid Preview/Commit | 呈现禁用；owner回调另检查门控，不通过即reset；晚到Commit不能写本地snapshot。 |
| 阶段选择、XY、visibility | 按钮disabled，同时各具体写入口先检查门控；既有隐藏选择及XY特殊回退规则保持。 |
| 拖排/主开关 | drag按钮、drop目标提示及主开关禁用；drop和主开关回调另核门控，旧拖动还需通过generation。 |

226的面板标题、帮助和说明不跟随该状态变淡；仅阶段header与rows分别应用0.3，等价于源包住二者的disabled body。主开关独立禁用；用户配置的“阶段关闭只显示当前行”仍是另一个条件。没有把锁状态当作用户关闭阶段，也没有修改count、值或visible字段来模拟锁定。

本轮曾发现并反馈观察清理范围过大：初稿`observe_dpi_editing_enabled(false)`复用dismiss_editors，会顺带`scroll_editor.deactivate`，取消Customize中的无关滚轮等级预览。最新已分出`dismiss_dpi_editors`；OTFS观察及锁下数字Blur仅调用此方法，普通切页/Help仍走`dismiss_editors`清理对应页面瞬态。已回读确认DPI锁观察不会再触碰scroll_editor，也不发本地Changed；该缺口不再记为未修。

在本批已审字段、阶段操作、快照隔离、门控与上述补丁范围内，未发现新的确定代码差异。N阶段主体及观察入口已静态补齐，仍保留真实生产者接线、框外数字滚轮、完整窗口输入/视觉及对应滑条独立审查的限制；O轮询区域仍是单独未完成项。报告diff检查通过（仅LF/CRLF提示），本子任务没有运行cargo、应用、构建、测试或DLL；主线程的编译结果另行记录。

## O 轮询UI实施前续核：字段归属、观察与本地保存

本轮重新读取当前226普通wi/Xs/zs及当前工作树轮询渲染、Mouse/SourceProductWorkspace的capture/restore、ProductWorkspace转发和Shell本地保存链。zs `[157662,165209)`、Xs `[165218,165578)`、wi `[183668,183979)`三项收据已按当前8355指纹与UTF-16切片重新比较一致。原O中BLE隐藏、wired/wireless目标字段、连接标题/帮助、高频说明、DualLink/dock限制缺项在本轮实施前仍存在：现有polling固定`POLLING_RATE`、`spec.polling_path()`及普通标题，226始终写`/pollingRate`；DPI批次没有替代这些功能。

当前源补充位置：Ps `[155469,155946)`是实际频率按钮，选中或disabled时不调用上层；有线bs `[155425,155468)`、无线Is `[155340,155382)`、BLE Os `[155383,155424)`是三个不同动作。配置加载的`pollingRate`/`pollingRateWireless`分支分别派发观察动作，不能因菜单档位相同合并存储owner。以下为独立回读确认的接入要求，尚不表示主线程新轮询实现已通过。

### 未知连接的本地编辑回退

主线程拟保留本地编辑能力，本子任务已核实该选择有当前源初态依据：主包deviceReducer初态的`isBle:false`、`isDongle:false`分别在UTF-16 145119、145128，zs.getPollingRateMode因此选择wired。可在连接观察仍为None时使用有线配置作为明确的本地编辑回退，并说明连接状态尚未读取；不能把None直接改写成一次Wired观察。已知Dongle后显示无线字段，已知BLE按wi隐藏整个面板。

主包Polling reducer初态Ce的wired为125（175029处），而产品默认profile的wired为1000；这是启动临时reducer与已加载profile的不同owner。本地已经有profile草稿时应保留其值，不能为模仿启动临时态而重置为125。未知连接说明属于本地实现的真实性边界，不是原包已经读取设备状态的证据；本轮仍不调用DLL/服务来生成观察。

### 独立字段、显式编辑与保存

| 核验场景 | 实施时应保留的行为 |
| --- | --- |
| wired/dongle切换 | 只切选中字段、按钮数据与标题，不能把pollingRate复制到pollingRateWireless或反向复制；两个字段的显式本地覆盖分别记录。 |
| 已知BLE | 面板隐藏；保留已有wired/wireless/ble草稿字段，不能借隐藏动作删值或强制重置。当前226仍不新增BLE频率编辑入口。 |
| 首次restore后未编辑 | SourceProductWorkspace.restore_active会立即将补齐默认值的Mouse.snapshot写回活动profile.source_settings。仅凭保存对象中“有pollingRate字段”不能判断它是用户覆盖，必须保留独立本地字段标记，避免重现M的默认值升级为local问题。 |
| 点击当前已选中按钮 | Ps的active分支直接no-op，保持此行为，不为“记录意图”增加原源没有的重复选择动作。 |
| 显示观察4000，draft仍为默认1000，再点1000 | 1000在显示上未选中，这是有效本地编辑。即使draft数字本来也是1000，也应标记该字段为local并发布Changed；不能被parent.write的数值相同去重吞掉。 |
| 纯rate/connection/topology观察 | 只改变运行态显示/限制，不写draft、字段标记、Device.source_settings或dirty；后续其他UI编辑的capture也不能顺带把这些观察保存。 |
| 旧文件迁移 | 无字段标记的已有值需明确按旧本地草稿处理；新建/默认补齐必须写空标记。不能用观察到的数值是否等于默认值倒推它曾被用户编辑。 |
| 实际限速观察到来 | 禁用超限按钮并显示对应原因；不得直接把观察8000伪装成已经降到1000。源applyPollingRateLimit调用setPollingRateWireless的自动降档是后置写腿，本地草稿约束与设备当前观察必须区分。 |

已查`source_controls_data.json`、`keyboard_oled_data.json`、`accessory_controls_data.json`三份参与device_fields的实际数据，均无226条目，因此现有226的capture_device_settings不会抽走polling字段；这两项当前归活动`profile.source_settings`。运行连接或DualLink拓扑不应借`source_device_settings`混入本地配置。

Shell保存由点击时捕获的WorkspaceFile调用store::write_workspace，成功后mark_saved捕获的Device快照，状态文字已明确“已保存到本机 · 尚未发送到设备”；保存期间的新编辑继续dirty。新轮询字段/标记应沿这条既有本地链保存，不新增设备成功提示，不在mark_saved时反向更新运行观察。以上只静态读取保存实现，没有实际写文件往返验证。

### restore、页面切换与观察生命周期

- 切profile或Discard会调用Mouse.restore，新profile的两个本地值及local标记应随它恢复。rate观察属于当前profile，旧观察应清空，并通过profile/generation等身份拒绝迟到的旧profile观察；仅清一次Map不足以阻止晚到消息重新填旧值。
- 连接及DualLink拓扑属于设备/连接生命周期。普通profile切换不应把它们从profile草稿恢复或清成“已知wired”；设备身份/连接生命周期变化时则必须能失效旧观察。不能将“没有收到dongle=true”当作已成功读取wired。
- 普通TAB及Help通过同一父层路由切换；新轮询的help/tooltip若保留实体需覆盖Help早退清理。页面切换不应丢失本地字段标记，也不应把旧连接/频率初态重新保存。源focus/1500ms周期刷新DualLink snapshot的生命周期另有依据，静态本地状态不能替代真实publisher。
- rate/connection/topology观察只应处理轮询自身呈现及瞬态；不要复用全局dismiss_editors而取消仍有效的DPI拖动或Customize滚轮预览。编辑禁用条件、链接点击与轮询选择需分别处理，不能把高频说明出现误当成所有高频选项禁用。

当前SourceProductWorkspace→Mouse只已有DPI/滚轮观察转发，尚无轮询连接、独立rate和DualLink的typed接入。新的接线应保留来源/作用域有效性，并使旧按钮回调在连接或profile切换后不能按旧owner写入新字段。具体新实现由主线程继续，完成后本子任务再回读。此节不扩大为O已完成，也不重复将其他子任务的MW全量解析当作本子任务运行验证。
