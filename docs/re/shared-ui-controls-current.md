# 当前产品共享控件：语义、调用条件与状态样式

审计日期：2026-10-09。依据仅为当前 `.ref/devices/` 的 manifest 声明 JS/CSS，以及覆盖 331 个注册产品的当前页面引用图。本页补充**具体控件行为**，不把相同组件名称、webpack ID 或方法签名当成行为相同，也不宣称整个共享控件库已经完成语义审计。未执行厂商 JavaScript、应用、DLL、构建或测试。

机器证据：[shared-ui-controls-current.json](shared-ui-controls-current.json)。生成器：[audit-shared-ui-controls-current.cjs](../../tools/audit-shared-ui-controls-current.cjs)。源码区间采用 UTF-16 code unit，`end` 不包含，哈希为实际 UTF-8 文件字节的 SHA-256。JSON 的 `seeds`、`methods`、`events`、`defaults`、`callers`、`guards`、`helpers`、`css`、`fonts` 均保留原文与区间，不能用本页概括代替这些证据。

## 语义入口与源文件身份

以下编号用于后文定位。区间为完整类/函数；方法与事件更细的区间在 JSON 中。

| 编号 | 当前源文件 | SHA-256 |
| --- | --- | --- |
| S70 | `.ref/devices/70/static/js/main.8f24b6a1.js` | `be0816be63fe674ea9fc106cbd55f5aabd509e1ff36aaf66e6c002d9ecd1f4b5` |
| S241 | `.ref/devices/241/static/js/main.899712fe.js` | `65fe2c4069ece8270903895ce32c6c610d5eb8fec94bd944f5791c2376509a02` |
| R241 | `.ref/devices/241/static/js/11.219fb515.chunk.js` | `d39889eed8d9c2443c0a57b075018604c8e4283a70ef98dc554ac712f667dc61` |
| P241 | `.ref/devices/241/static/js/914.4c13bdac.chunk.js` | `1320d5e2c7ee42497e590f2dbd979b9d10614573746003b3b87b6695dfc6a9d2` |
| T241 | `.ref/devices/241/static/js/855.b3260189.chunk.js` | `b224317c425ec86a5d4f38b5b72b487842b97c7289cfd207ca23814235d45491` |
| M70 | `.ref/devices/70/static/js/MapMacro.3627a5cd.chunk.js` | `505c8585a44501f2c99e24c3ed2b9ae0ae8a501fd697c5612a80657e030bbc2a` |

| 控件 | 源与区间 | 模块/符号 |
| --- | --- | --- |
| Slider | S70 `[3192,10125)` | `130/T` |
| 另一种 Slider | S70 `[18968,22853)` | `801/n` |
| Number/Stepper | S70 `[185162,190903)` | `4230/o` |
| Dropdown、Option | S241 `[330999,337354)`、`[327837,330526)` | `7734/d`、`7278/c` |
| Keymap dropdown | S70 `[163606,167378)` | `3317/O` |
| Rename input | R241 `[122562,123749)` | `9163/ot` |
| Delete popup | R241 `[119660,122072)` | `9163/et` |
| Action popup | R241 `[78590,82660)` | `9163/ie` |
| Tab、Nav | R241 `[74627,75288)`、`[110724,118314)` | `9163/z`、`9163/We` |
| Product profile bar | R241 `[204808,224102)` | `9163/yi` |
| Modal、独立 root | P241 `[4834,5491)`、`[5494,5812)` | `3746/Z`、`3746/X` |
| Widget help tooltip | S241 `[309009,312474)` | `6299/p` |
| 定位 tooltip、cursor observer | T241 `[12488,15782)`、`[11036,11864)` | `8837/u`、`8837/n` |
| Macro mapping draft | M70 `[888,9000)` | `2508/P` |

## Slider 与 Number 必须区分

`130/T` 只默认 `enableSliderRange=true`；min/max/step/value/active 由调用方传入，不能补造统一默认值。`onChange` 依据 `allowDecimals` 选择 parseFloat/parseInt；dataset 不含输入时 `roundValue` 向后选择数据点。`min=0 && max=1` 时此路径直接不处理；`isRangeLimited` 且上下限有效时拒绝越界输入，并不是把它 clamp 到边界。

鼠标按下设置 `mouseIsDown`，原本 inactive 时激活；拖动中只在 active 条件内更新本地 value，可由 `callOnChangeOnEveryStep` 每步通知，或由 `debounceTime` 延时通知。window mouseup 再传 `changeValue(value, position ?? null, false)`；非拖动 change 传 `changeValue(value,false)`，回调的参数含义不同。外部 value 更新且 `cancelMouseUpOnValueChange` 开启时取消当前拖动和延时。源码没有在 unmount 中统一清掉 debounceTimeout，只移除 mouseup 和 intersection observer，不能在文档中写成自动完成全部清理。

键盘 Home/End/Left/Up/Right/Down 的 keyCode 35–40 均被 preventDefault。thumbTag 与 tooltip opacity 随光标/拖动切换；填充宽度是 `calc(8px + percent*(100% - 16px))`，标签位移含 `11.5px`。位置计算以实际 DOM 尺寸为准，未取得尺寸时可请求下一帧。

`801/n` 是另一份**当前版本仍存在的实现**，此处“另一种”不是过时 reference。它 change 立即调用父级，mouseup 再调用；只拦方向键 37–40，没有 130 的 dataset、debounce、range-limit、cancelMouseUp 路径，填充宽度为 `100*percent%`。产品 70 `6903` DPI 调用明确设置 min=`DeviceInfo.minDPI||100`、max=`DeviceInfo.maxDPI||16000`、step=`DeviceInfo.dpiStep||50`；不能把 130 的语义移植到该 DPI 实现。

`4230/o` 使用 text input，整数正则 `^-?\d+$`，小数正则允许最多三位小数，临时接受空串/单独负号；prefix/unit 移除采用正则转义。`allowLiveUpdate` 开启时，输入变化即把当前 draft 通知父级；这一步不是最终规范化。

最终 parseInput：NaN→0；整数按 stepValue 向上取整，然后先 max、后 min clamp。blur 的小数路径另按 step 修正，并依据 `roundUpDecimals` 决定是否 toFixed(3)。这个修正发生在 parseInput 的 clamp **之后**，因此文档不能宣称任何小数 step/max 组合都绝不越界。Number 的 Enter **和 Escape 都调用 blur 并提交**，没有 Escape 恢复旧值的路径。上下键触发 volumeUp/Down；spinner mousedown 立即一步，之后每 300ms 一步，mouseup/leave 停止；右键被阻止。dataset 的增减按 props.value 的精确索引取值，顶端递增存在取到 undefined 后回到 parseInput 的行为，不应替厂商源码“修正”成想当然的 clamp 逻辑。

点击输入 focus/select 并注册全局 mousewheel，滚轮阻止默认事件；到达边界且继续朝边界滚动时不操作，blur 移除 wheel listener。unmount 只清 intervalId，没有 wheel listener 对称清理。disabled 主要由 CSS `.stepper.disabled` 的 opacity/pointer-events 实现，input 没有 DOM disabled 属性。

真实调用方例子：70/6223 turbo 的 slider/number 范围均 1–20，active 来自 `isTurboEnabled`；70/2508 Macro repeat number 仅 `selectedPlayback===1` 时挂载，范围 1–99，整数、step=1、allowLiveUpdate=true。JSON 保存完整 props 和祖先分支，不能只抄范围。

## Dropdown、菜单、Tab

241/7734 dropdown 初始化 open=false、disabled=false、dataSet=`props.dataSet||[{name:"",content:""}]`；mount 后 dataSet 长度≤1才设 disabled。点击门禁是 `!allowOneItem && state.disabled && name!=="quickEffDrop"`，此外 `props.disable` 会产生 CSS disabled。allowOneItem 可移除 disabled 样式；quickEffDrop 也有例外，不能统一成“只有一个选项就打不开”。空数组、value=-1 或超过末项时 render null。

选中会收起、清 tooltip，再传 optId；isOBMDropdown 的 Option 则传 content。disabled Option 不绑定 onClick，hidden Option不渲染；divider 是独立节点。window click 到 dropdown/option 外、window blur 时收起；此实现没有 Escape、方向键或焦点导航处理。打开位置以 dropdown bottom+2、`min(180,25*选项子节点数+2)` 与 wrapperH/innerHeight 判断上下方向，向上 bottom=28px，向下 top 为27px（dvDrop另加22px），并 scrollTo 已选项。

tooltip 可来自显式 tooltip/disabledTooltip/content；Option 会根据文本是否溢出、tipIcon、disabled 清掉无需显示的 tip。dropdown 自身 tooltip 使用 createPortal 到 document.body，fixed、max-width=300、z-index=150、pointer-events=none；它与 widget tooltip 不是同一种定位合同。70/3317 keymap dropdown 另外使用 GUID 匹配、可显示 slot 图像和 addKeymap 行；available height 的无 wrapperH 默认是688，不是241 dropdown 的 window.innerHeight。

241/9163 action popup 点击三点切换，外部 click/window blur 关闭，可在 adaptivePosition 时调整左右位置。disabledItems 对应项目没有 onClick；LINKED_GAMES 的显示与 gameItems 是否为空相关。它没有 Escape 或 roving focus 逻辑。241/9163 Tab 接受 click 和 code=`Enter`，同步 props.active 到局部 state，没有 Space/箭头切页处理。Nav 用 `normal 12px Roboto` 测量翻译文本，每项另计20px和项间20px，将塞不下的项转入三点菜单；不能固定为“所有标签始终横排”。语言变化清宽度缓存，resize 重算。

## Profile：编辑、删除、重置与保存边界

241 普通 root `nn` 真实挂载 Profile/Nav；`on=[HELP]` 只隐藏 Help 的 profile bar。本页审读该代码的共享分支，不证明 GAMEPAD/OBM 等条件在241设备上实际成立。JSON `nn`、`xi`、`yi` 和各 JSX props 已保存这些分支；跨产品根结构还需各产品入口证明。

Rename input mount 时 focus/select；Enter blur 后发送当前文本；Escape 把 **props.value** 传给父级恢复原值，没有 Number 的 Escape提交合同。普通 profile maxLength=32，controller OBM 分支25，并使用另一个输入几何样式。`yi.setProfileName` 先 ECMAScript trim、收起编辑，再拒绝空白和重复；1867/N 检查 `e.name===t`，大小写敏感。create name 从 Default 或 `<用户>-Default` 起，最多尝试100个编号；duplicate name 对现有 ` (...)` 数字尾缀另有处理，原文在 helpers。

菜单 Add、Duplicate、Rename、Delete、Import、Export、Linked Games、Armory 各有显式 action 分发；delete/reset popup 延迟100ms显示，不能用“以后再接DLL”删掉这些 UI。产品 profile 删除在 `et` 中外部 click 调用 confirmDel(false)，按钮 stopPropagation 后 confirmDel(true)；该类**没有 Escape 监听**。产品 Profiles 应用界面的其他确认合同不能覆盖它。`yi.getDisabledItems` 有最后一个 profile、OBM profile、analog preset、dynamic owner、Armory maintenance/shared 等分支，菜单生成也受类别、supportResetProfile、安装 Armory、sharing gate 与 user guest 条件影响。

Reset popup `Kt` 的局部 show 随 props 更新，外部 click 关闭；Reset keybinds/Reset profile 分别触发自己的动作再关闭。删除确认触发 props.deleteProfile(selectedProfileGuid)。这些 props 在 xi 经 Redux bindActionCreators 连接3112；3112 的 add/rename/delete/reset/import 等只是 dispatch，**不能把 dispatch 写成“DLL 写回已成功”**。

70/2508 Macro mapping 子视图有本地 selectedMacro/selectedPlayback/playbackTimes；changeX 调用 enableSave；mount 通过 setSaveMapRef 暴露 saveMapping。saveMapping 从 state 组成 mapping 数组，重复次数只在对应 playback 模式使用，否则 repeatCount=2。70/5107 的 saveChanges 再根据 TwoTap/globalShortcut/custom command dial 等分支处理，普通分支 setActiveKeyMapping、setMappingList、清 dirty/关闭；closeMapping 检查 dirty 决定显示保存提醒。提醒关闭按钮 dismissSave 只隐藏；dontSave 调用 saveMapRef(false)，确认按钮 submitDialog 调 nextAction，回调连接的原文另存 helpers。不能仅凭按钮名推出设备提交或持久化成功。

241/2478 本地存储 wrapper 的 set/get/remove/updateProfiles 使用 window.localStorage；synapse 产品条目可同步 dongle 项，updateProfiles 保留原 appEngine、更新产品列表和单产品条目。`updateUserData=async()=>{}` 是空实现。这份 JS 的本地状态/存储证据与当前 Rust 项目的 local draft 不是同一份状态，亦不证明后端服务持久化。DLL 写回仍依任务约束延期。

## Modal 与三种 Tooltip 的实际差异

241/3746 Z 绘制 sibling backdrop 与 fixed modal。只有 close 图标绑定关闭回调：无 close 回调直接 removeRoot；同步返回false阻止删除；Promise fulfilled 值不是false才删。X 创建独立 div、Redux Provider，用 ReactDOM render；移除时 unmount 并从指定 container/body 删除。代码**没有 backdrop click、Escape、focus trap 或焦点恢复**。不能因为通常 modal 应该具备这些行为就在文档中宣称厂商实现有。配对控制器的成功/取消导致关闭另见 [receiver-ui-current.md](receiver-ui-current.md) 和 [receiver-241-semantics-current-evidence.json](receiver-241-semantics-current-evidence.json)。

241/6299 Widget help 在 mouseover 设置 showTip 后才测 `.main-container > #body-wrapper` 边界，mouseleave立即移除。右侧越界加 custom-tip；底部越界改 fixed 放 help 右侧8px，不够时放左侧；ignoreBottomOverflow=false时再上移超出底部的量+10。它是 widget 内的 `.tip`，**没有 createPortal**；不能与 dropdown 或8837 Portal tooltip 混用。tip只有 tips 不为 undefined 才生成；fw最低版本 gate 可给 widget加 disabled，fw warning tooltip又有独立 mouse坐标。

241/8837 定位 tooltip 支持 bottom-right/bottom-left/edge/top；target 查找 body-wrapper 和指定元素，缺任一返回0矩形。mounted 状态变化先shouldRender，再通过 setTimeout切 show class；hide反向延时移除，无显式毫秒值，不应写成固定300ms的 JS delay。水平边界保留8px，resize重新调整；target存在时 Portal、pointer-events=none，无 target则由自身 hover管理并嵌入原父节点。

followCursor=true 且 mounted 时走另一条 observer：window mousemove通过 requestAnimationFrame刷新，mouseout隐藏，unmount清RAF与listeners。其 tooltip 使用 inline max-width320、z-index999999、字体14、背景#000、色#ccc、padding8px10px、border#5d5d5d；光标坐标+50、垂直优先+25，底部不足时上方放置，水平中心限制到边缘8px。这份样式不能替代 Widget 的300px/18px行高合同。

## CSS 状态与字体

下列为已核对的**源声明**；实际级联仍需页面树、祖先、后续规则、inline覆盖共同确定。完整1413条包含 source/order/@条件，机器证据保留哈希，不能把候选规则集当成最终浏览器结果。

| 241当前 CSS/区间 | 已确认声明或状态 |
| --- | --- |
| `main.1525b0e6.css [3227,3439)` | body/html #222、#ccc、Roboto/sans-serif、16px、min-height720、max-width1920、overflow hidden |
| 同文件 `[33710,33978)`；`[33589,33642)` | dropdown27px高、14px/17px、border#515151、padding4px5px；disabled opacity.3与pointer-events none |
| 同文件 `[34402,34447)`；`[35006,35079)` | dropdown expand边框#44d62c；选项展开max-height180、border#515151 |
| 同文件 `[35129,35308)`；`[35685,35738)` | option25px高、padding4px、14px；hover#ffffff1a |
| 同文件 `[43100,43247)`；`[43296,43347)` | slider64px高，初始opacity.3/pointer-events none，`.on`启用 |
| 同文件 `[44624,44827)` | slider thumb hover#5d5d5d、active#383838，均2px绿色边框 |
| 同文件 `[203615,203960)`；`[205088,205341)` | stepper62×26、input38×24/14px；hover/focus-within绿色边框且spinner显现 |
| 同文件 `[24387,24566)` | Nav active:hover #44d62c/#111、hover#2d2d2d/#ccc、:active#3cbf27/#111 |
| 同文件 `[74757,74999)`；`[75808,76261)` | Widget help #4a4a4a、300ms背景transition；tip #000/#ccc、14px/18px、max-width300、padding8px10px，hover邻接显示 |
| 同文件 `[33135,33348)` | 定位 tooltip wrapper #000/#ccc、Roboto14px/16px、padding8px10px |
| `914.530f0373.chunk.css [18809,19172)` | 通用 backdrop #000000b3/z1000；modal #111、1px#515151、5px圆角、width850、top100、z1001，另受配对专属类覆盖 |

70的CSS也保留在JSON，因为70 slider/number不能直接用241声明完成挂载证明。241 main CSS `[0,3133)`有14个font-face：Roboto正常/italic，weight300/400/500/700，优先local名称再woff2/woff；RazerF5含normal100/400/600/700及italic400/700，无local项。默认body用Roboto；RazerF5用于有显式选择的标题，不能全局替换。source字体URL为 `/synapse/assets/fonts/...`；本页未宣称这些网络URL在运行时已加载或字体fallback/中文实际glyph完成验证。

## 全产品复用范围与剩余问题

扫描331产品1452页面的当前引用图，得到2186个实际 class区间候选；按方法签名分组只是追查线索。下面是进入页面引用图的产品数/实例数/原文body变体数，不包含所有root/header控件，不能作全控件完成率。

| 候选家族 | 产品数 | class实例 | 原文body变体 |
| --- | ---: | ---: | ---: |
| 130式 slider | 272 | 272 | 184 |
| 另一种 slider | 224 | 394 | 209 |
| number | 123 | 123 | 48 |
| dropdown | 287 | 288 | 190 |
| rename input | 140 | 162 | 79 |
| delete popup | 326 | 329 | 286 |
| action popup | 23 | 23 | 19 |
| widget tooltip | 325 | 325 | 289 |
| positioned tooltip | 270 | 270 | 144 |

每项有实际文件hash/区间、原文body hash、关联page_id及incoming reference原文区间。精确body相同的复用单独列 exact_seed_bodies；例如70 number有2个精确seed实例。变量/导入别名不同会增加原文变体数，因此这个数不是独立行为类型数。不能将48份number body自动并成1套语义。

静态终检：生成器 `--check` 通过；另用独立字节读取校验11069份source区间/原文或body hash，涉及1080个文件，全部一致。调用证据另保留词法definition、webpack require/export getter和HOC参数引用区间，避免只用目标模块号认定实际调用。这些检查不执行reference代码，也不验证运行时控件行为。

目前逐段审读18个seed，追踪其70/241语料中的59个真实JSX调用，另有34份helper、1413条CSS与28个font-face原文收据。keymap、tab/nav/profile、macro root在该页体图中候选数为0，表示图范围没有遍历到它们；本页已有具体root/调用方源证据，不表示产品不存在这些功能。接下来应对其他产品的body变体逐项核对事件/默认值/校验差异，补每个根入口的header/profile实际挂载和完整Apply/Save后端链，再作CSS祖先/inline级联及DLL读写边界审核。本页不把这些剩余工作标成已完成。
