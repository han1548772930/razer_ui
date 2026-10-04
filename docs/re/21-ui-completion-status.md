# 当前 UI 完成状态

2026-10-04 集成进度：用户报告的hover重复注册、电量父层布局及导航历史接线已作源码修正；Macro和Armory本轮新增内容见[剩余工作](remaining-ui-work.md)与各应用复核文档。所有产品仍为部分实现，未新增任何“完整复刻”产品；格式化、静态解析、资源校验和cargo check不代表实际点击或像素验收。后续旧轮次记录保留为历史。

更新日期：2026-10-04。**目标仍未完成**。本轮恢复旧日志并独立逐项复核，确认旧会话因round-limit停止；331产品/1419主导航页的partial状态不变，不能算完整产品。当前源码要求和禁止运行的约束保持不变。

本轮修复hover重复注册、电量父容器/tooltip/显示条件、错误主导航文案、重复导航箭头与历史衔接，并重写Profiles为真实两个页签。992项资源通过hash/格式/嵌入校验。详见[逐项源码复核](source-ui-review-2026-10-04.md)、[Profiles更正](profiles-app-audit.md)、[导航修复](device-tabs-audit.md)。UI和全部产品接入优先；DLL仅静态读取接口、后端最后统一处理。

**以下是截至2026-10-03的历史记录，不能作为本轮已复核证明。**其中“Profiles五路由”“缺中文无法解析”“宏/Profiles媒体目录不存在”“标签栏必须渲染第二套前进后退”已被当前源码证据推翻，以本轮文档为准。

---


更新日期：2026-10-03。目标是完整复刻当前稳定版 Synapse UI；**尚未完成**。版本依据为 [当前源记录](20-current-source-version.md)，逐产品的实际路由与状态以自动生成的 [原生覆盖表](native-product-coverage.md) 为准。

331 个注册产品包含大量共享页面和连接别名，不能算作 331 套已完整实现的界面。`partial_native` 只表示已有实际内容；入口、描述符、素材下载和编译通过都不能证明视觉或交互等价。较早的“10 个路由、321 个未适配”等统计保留历史意义，不是当前覆盖率。

本批补齐两款磁轴键盘的校准初始页、弹窗和独立状态预览，并恢复三个音频演示页的原版初始画面。细节与限制见 [校准页审计](keyboard-calibration-current-audit.md) 和 [音频演示审计](audio-demo-current-audit.md)。

随后接入 Philips Hue 的连接引导、网桥、亮度、设备列表和五种快速灯效，并加入高级灯效状态及 18 种隔离预览。Hue 的服务和视觉限制见 [当前 Hue 审计](hue-current-audit.md)。

本轮并行接入此前八个完全缺少主体的页面：164/241 鼠标底座、778/3871 有线 ARGB、3884/3886 无线 ARGB、784 Aether 灯带、3946 Base Station V3 自动化。当前 1419 个主导航页中，1392 页有部分原生内容，27 页仍列作原有适配器待重新复核；没有产品被认定为完整复刻。3886 原代码端口分支不可达，显式样例不计作正式端口编辑已完成。

按用户指出的无线接收器下拉框问题，追踪了根 props、消费者和页切换状态：179 原版不挂载配置栏；其他附件页必须区分下拉禁用与仅同步图标禁用。见 [配置栏复核](product-profile-bars-current-audit.md)。端口和灯带布局现按源数据作用域存为设备设置，3946 自动化随配置文件保存。

本轮接入 3592／3594／3595／3596 四个当前 Kiyo 根 CAMERA 页缺失的取景块：变焦、平移／倾斜面板、五个取景预设和预设快捷键，控件与几何算式逐项取自原组件，标签与快捷键输入 ID 来自产品自身本地化及按键表；页面改为原版的 400px `.camera-container` 列。3596 的变焦范围（1–1.4 / 0.01）与 3592／3594 的 maxPanTilt 差异按各自挂载 props 生成，未统一套用。随后按同样的来源规则补上分辨率行（3594／3595／3596，10 项能力回退列表）与水印放置盘（仅 3592 挂载，六个位置，随开关禁用）；3592 不挂载预览块，因此没有分辨率选择器。压缩名跨模块复用，水印位置经导出表加字面量折叠解析，分辨率列表按挂载组件实际引用的绑定定位。实时画面与设备枚举、第三方分支、LDC／分辨率联动的禁用分支、白框拖动和方向键图标仍未完成，见 [取景审计](camera-framing-current-audit.md)。

本轮清点了 `displayMode` 多根机制的真实范围，并提取窗口打开契约。原版不是用路由切换这些根：每个产品包用 `?displayMode=` 选择另一套根组件，Dashboard 通过具名窗口打开它。静态扫描 331 个包（只读注册审计记录的主导航源）得到根级分支：`armory` 226、`chromaApp` 212、`macro` 174、`multiDevicePairing` 30 个产品包；注册审计只把其中 31 条 `multiDevicePairing` 与 1 条 `chromaApp` 记成非主导航，因此覆盖表里的「32」只是已登记的那一片。窗口契约给出 13 个标志（`policy=3` 复用当前窗口、`policy=5` 独立窗口、可见性、`shouldFocus=1`、按模式区分的 `app_icon_path`）、三条窗口名规则（`multi-device-pairing-<containerId>`、`multi-device-pairing-p<pid>-<serial>`、`multi-device-pairing`）与配对窗口的参数构造（`containerId`、`displayMode`、`allMasters`）；打开函数先查同名窗口是否存在，存在则复用并调整可见性。按用户决定，这些根将以**真正的第二个 gpui 窗口**实现，而不是应用内路由。见 [分支审计](display-mode-audit.md) 与 [窗口契约](display-window-contract.md)。

本轮接着补摄像头的取景交互：按原版把「按下白框记录抓取偏移 → 在黑框 `onMouseMove` 里移动白框 → 依次套用四条边界钳制 → 用 `Tm`/`Im` 把像素回算成 pan/tilt」整条链移植进来（`drag_pan_tilt`），白框像素位置改为每次由 `white_origin` 从 pan/tilt 重算，与原版 `useEffect(H)` 的行为一致；黑框的盒模型也纠正为原版的 content-box：内容 220×132 加 1px 边框 = 边框盒 222×134，之前把 220×132 当成了边框盒。上一轮补完的复合禁用条件（`ldc && (4K 30FPS | 1440p 30FPS)`）与左右方向键图标同在本文件对应的[取景审计](camera-framing-current-audit.md)里。

本轮补上相机取景块缺的**共享数字步进器**：原版变焦行是共享设置行 `HM.A`（`hasStepper:!0, allowDecimal:!0, roundUpDecimals:!0, stepValue/minStepper/maxStepper`，`disabledStepper` 接同一道 LDC 复合条件），本地新增 [stepper.rs](../../src/ui/stepper.rs)，外框 60×27 + `1px #5d5d5d`、输入区 58×25 `#111/#ccc/14px`、上下箭头 14×12（`background-size:8px`、上 5px 下 3px 偏移）、hover/focus 才淡入、按下立即一步并每 300ms 重复、禁用 `opacity:.3` 且不响应指针，箭头图标用产品包里同名同哈希的两份 SVG（已按原样打包）。描述符新增 `has_stepper`/`allow_decimal`/`round_up_decimals`，由生成器从该行原文提取，3592/3594/3595/3596 四个产品的变焦行已接入；亮度/对比度/饱和度/锐度/增益/白平衡六行在原版同样带步进器，本地仍只显示数值，记为缺口。同时修正上一轮的一处记录错误：模块目录的 `MACRO` 盒当时**并没有**真正改成直接打开，本轮已改为 `Some(ModulePage::Macro)`，现在四行（Alexa、宏、配置文件迁移、介绍导览）确实直接打开本地页面。

步进器随后扩到原版同样带步进器的图像四行（亮度、对比度、饱和度、白平衡）：四个产品各有 5 行接入（含变焦），步长与范围逐行取证（1／1／1／10／0.1，白平衡 `disabledStepper` 即自动白平衡开启时禁用）。原始锐度与增益两行在原版本身是有条件的 `a&&(…)`／`t&&(…)` 分支，本地相机页尚未挂载这两行，因此记为「条件行未接入」而不是「步进器缺失」。

覆盖审计里最后一块「未复核」也清掉了：十个 `existing_partial_native_adapter` 产品的 **27 个主页面**逐页复核完毕，状态从 `existing_partial_not_reaudited_here` 改为 `partial_native_reaudited`，每页都带本地路由、可达性（`src/nav.rs` 的 `Tab::for_product` 分支）与该产品存在的当前源码数据；**没有**把页面内部逐字段来源、视觉一致性或硬件行为写进依据。剩下的缺口写在[逐页复核报告](legacy-adapter-page-reaudit.md)里（十个产品仍未进入 `source_help` 描述符、182 的规格条目 `pages` 仍为空、653/777 仍靠各自手写页面）。

本轮把宏窗口从「安装门控」变成可打开的本地页面：新增 `Location::Macro` 与 `HostTab::Macro`（标签 id 就用原版窗口名 `macro`，图标复用已打包的 `synapse/module-macro.svg`），模块目录里 `MACRO` 盒现在直接打开它；宏服务与宏数据仍未连接，页面按原版外框（`MacroContainer_my_macro__jCmj8` 只有 `position:static`，`setup_svgs` 是 `display:none` 的图标预载容器）与两个导航标签（`TEXT_NAV_TAB_MY_MACROS` 我的宏、`TEXT_NAV_TAB_KEY_BINDS` 按键绑定）呈现，并显式写明宏服务未连接、不把未知安装状态写成已安装。功能面板（palette）的 11 个条目已从模块 81021 逐条提取（类型、图标文件、文案 key），但宏应用在当前源码包里没有 `static/media`，这 11 个图标全部缺失，因此面板尚未绘制、也没有拿别的图标顶替；部分文案 key 在两个 `trans-zh-CN` 分块里都不存在，同样不臆造。收据见[宏应用界面审计](macro-app-ui-audit.md)。

模块目录也按这份证据对齐了行为：每一行现在带原版的盒名、它聚焦的窗口名与地址（Alexa→`alexa`、宏→`macro`、已关联的游戏→`profiles`、反馈→`feedback-synapse`、工坊→`armory`、配置文件迁移→`syn3-profile-migration`、介绍导览→`synapse-introduction`），本地已实现对应窗口的四行（Alexa、宏、配置文件迁移、介绍导览）直接显示「打开」并进入本地页面，其余显示安装状态未读取并注明原版窗口——原版点击模块盒本来就是 `focusTab(windowName)`，未安装时才先打开 `/installer/#type=module&id=<id>&location=<path>`。

本轮把「宏窗口的打开点」查清了，结论修正了之前的实现计划。宏在原件里是一个**独立应用**（`.ref/applications/synapse/macro/`，72 个分块 7.5 MB，与 `alexa`、`armory`、`profiles`、`settings`、`update-fw`、`introduction-tour` 并列），窗口名就叫 `macro`，地址 `/synapse/macro/`，打开标志 `policy=3,tab_visible=1`；Dashboard 的模块表在同一处登记它（模块 54420），常量表在模块 69937（`O="macro"`），模块盒名在模块 54693，点击盒子走 `focusTab(windowName)`，未安装时先开 `/installer/#type=module&id=macro&location=synapse/macro` 再 `autoOpen`；映射界面里也有 `openMacro`（命中已开窗口则激活）。而 **`displayMode=macro` 不是第二个产品窗口**：它是宏应用「绑定到设备」弹层里的 **iframe**，`src` 带 `displayMode=macro&macro=<id>&containerId=…&deviceEditionInfo=…&serialNumber=…`，标题 `MouseBind`。同样地，`chromaApp` 在当前 Dashboard 里出现 0 次——它属于独立 Chroma 应用窗口。收据与清单见[宏应用审计](macro-app-current-audit.md)、[窗口打开契约](display-window-contract.md)（新增 `named_windows`：11 个具名窗口、2 个隐藏、5 个模块窗口、8 条模块盒去向）与[displayMode 审计](display-mode-audit.md)（新增「各模式由谁打开」）。

本轮修复了一处启动崩溃并补上它对应的回归检查。崩溃发生在 [wired_argb.rs](../../src/features/wired_argb.rs)：内嵌的 `wired_argb_data.json` 里 778 的条目缺少必需字段 `name`，serde 在第一次 `spec()` 时 panic（`missing field 'name'`）。原因是 778 的设备配置本身没有 `deviceName`（它的名字来自运行时型号表：0 = B550、128 = X570、129 = Z690 Taichi Razer Edition），生成器把 `undefined` 交给 `JSON.stringify` 后被静默丢掉，而 Rust 结构仍要求该字段；`--check` 因此一直是「最新」的。处理：生成器显式省略该字段并对任何 `undefined` 字段直接报错；Rust 侧该字段改为 `#[serde(default)]`（它不参与渲染）；新增 [validate-embedded-json.py](../../tools/validate-embedded-json.py)，静态解析每个 `include_str!("*.json")` 的目标类型（支持 `Option`/`#[serde(default)]`/`rename`/`rename_all`/嵌套 `Vec<T>`），核对 22 份内嵌数据是否满足其 Rust 结构——用旧结构跑同一份数据即可复现 `missing field 'name'`，证明这条检查能拦住这类运行时 panic。

同时补完摄像头的复合禁用条件：原版取景组件里 `ldc && (4K 30FPS | 1440p 30FPS)` 一个条件同时禁用变焦步进器、变焦滑块、平移/倾斜面板和预设块（含快捷键），并在禁用时渲染 LDC 说明。本地把它编码为描述符的 `disabled_when_any`（任一条件组全部成立），分辨率取值与 `/camera/ldc` 都由生成器回到源码核对；3592 默认就是 `ldc = true` + 4K 30FPS，因此一打开即为原版禁用态。左右方向键改用已在本地打包的同一份图标（`icon_arrow_left_thin`／`icon_arrow_right_thin`），上下与中心键的图标在当前源码包中不存在，仍保留边框。见[取景审计](camera-framing-current-audit.md)。

按用户「开真正的第二个 gpui 窗口」的要求，本轮落地窗口层与第一个独立根。窗口层在 [display_window.rs](../../src/shell/display_window.rs)：窗口名规则逐字对应 Dashboard 模块 84058 的 `xc()`（容器 → 产品+序列号 → 纯模式名，有单元测试）、`policy=3/5/7` 标志、具名登记表与「同名窗口存在即聚焦」；[pairing_window.rs](../../src/shell/pairing_window.rs) 是该窗口的根视图。审计发现产品包里的 `displayMode=multiDevicePairing` 根只是一个 iframe 宿主：它把 `/synapse/multipairing/` 连同 11 个参数（`displayMode`、`containerId`、`productId`/`pid`、`category`、`canPairTwoDevices`、`isProductivity`、`deviceName`、`serialNumber`、`lang`、`allMasters`）打开，并 postMessage `multiDevicePairingInit`；本仓库的 4130 页面正是被嵌入的那一页，因此窗口直接承载它，`allMasters` 走页面原有的外部记录通道。入口按 Dashboard 设备盒 `box box-multi-paring` 的行为接在配对页设备卡上，guard 与原版一致（`productId` 与 `deviceContainerId` 同时存在）。`deviceInfo`/`deviceName` 没有本地对应字段、`allMasters` 的原版来源（宿主写入的 `connectedDeviceInfo` 投影）尚未审计，两项都记为缺口而不是补默认值。收据见 [配对窗口审计](multi-pairing-window-current-audit.md)，窗口名与标志见 [窗口契约](display-window-contract.md)。

已清点服务预览各入口，新增完整产品标签页入口；Dock 样例先显示主体再打开原配对层；Aether 样例复用完整产品工作区；Alexa 正式页已隔离开发场景选择器。仍只有局部组件或独立样例的入口，明确记录在 [服务预览整体复核](service-preview-current-audit.md)，不冒充整页完成。

后续工作仍包括：页面内完整条件分支与独立模式；鼠标、键盘映射；相机预览；有声演示播放器；附件高级编辑与配对、Hue；自动化宏/游戏/快捷键子编辑器；配置更多菜单和精确动画；OLED 高级内容编辑；托盘的完整账户/通知内容；宿主独立窗口。真实设备、账户、安装和固件服务也没有因 UI 入口存在而接通。

必须继续区分界面状态与设备事实。校准成功、固件完成、安装状态、风扇转速、电池或温度不能通过计时器或本地草稿伪造。独立开发预览可展示明确标记的状态示例。

已执行允许的全目标 `cargo check`、格式检查及静态资源/来源验证；检查仍有未使用导出、字段和死代码警告。947 项嵌入资源的来源、输出格式及哈希校验通过。没有启动应用、构建、测试、安装器、下载的 JavaScript 或 DLL，尚无真实窗口的像素、焦点、滚动和设备往返验收结论。
