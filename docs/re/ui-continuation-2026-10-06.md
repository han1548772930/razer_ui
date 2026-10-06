# 当前源码复刻继续记录（2026-10-06）

本轮仍以当前 Dashboard、host 4.0.827 及各产品 manifest 声明的源文件为依据。
执行范围限于源码解析、资源获取/转换/校验、格式化和
`cargo check --locked --all-targets`。没有运行应用、构建、测试、安装器或厂商代码。

## 已接入的界面与条件

- [Nommo 1303/1304](nommo-effects-native.md)：灯光页右列快速/高级效果、六种灯效
  参数、颜色弹层、方向切换及对应过渡。后续批次已接通专用 Studio 根，按钮直接
  打开 Chroma 宿主中的 Studio 子标签，不显示下载/安装门控。
- [四款相机](camera-presentation-current-audit.md)：首批 58 个分组、45 个折叠区域、
  31 项帮助和 3 项条件警告；修正 3594、3595 的质量控件挂载，补回 3592 PROCESSING
  的分辨率选择。[提示层级](camera-tooltip-layer-current-audit.md)按原始悬停状态在
  200/9999 之间即时切换，原淡入和定位过渡保持各自时序。
  后续预览区域在 3594/3595/3596 增加原本挂载的来源、Camo、眼睛切换和关闭提示，
  总计增至 64 个分组、51 个折叠区域、34 项帮助；3592 CAMERA 原本没有该组件，
  不补造。3594/3595 的 Camo 入口连接本产品 HELP，3596 使用原外部链接。
- [第一批 Armory 独立根](armory-audio-entrypoints-2026-10-06.md)与
  [第二批](armory-audio-next-roots-2026-10-06.md)：新增 1319、1325、1328、1330、
  1331、1332、1335。1335 使用其本包实际 `specialAudio` 条件的 390px 图框，不能
  沿用普通 250px 外框。包括原有根，共补齐 13 款产品、21 个产品/edition 身份的
  41 张精确原图/Dashboard 图；不通过相近产品图代替缺失资源。
- AppPicker 以本地可打开能力显示已实现入口，移除虚构的 Chroma 安装记录。
  有可显示内容时不追加安装状态提示；原本未实现的目标仍不冒充已实现。
- [设备入口](local-device-entrypoints-2026-10-06.md)：以实际持有的 renderer 和非帮助页
  判定本地能力，解除六种安装阶段对卡片/选择器的门控。保留 setupStatus 原值和最低
  固件、待机/断电、主机模式、Mixer 错误、预设加载、更新/重启等真实条件。
- [公共滑条](source-slider-current-audit.md)：为已核验的源 CSS 恢复 300ms
  透明度和滑柄背景过渡，保留即时禁用、即时边框及 Nommo 关闭亮度仍能拖动的特例。
- [步进箭头](stepper-visibility-current-audit.md)：Nommo 恢复 hover/focus 显示与
  100ms linear 透明度变化；通过 CSS 优先级核对保留四款相机的常显覆盖。
- [1383 Kraken V4 Pro OLED](audio-oled-home-native.md)：按自身 reducer 初值补回七张 Home Screen Display 卡片，
  支持本地模式选择、关闭遮罩和 Media 编辑草稿。其他编辑器未用相近页面替代。
  静态恢复 27 项素材，校验灰度像素与动画帧时长；预览中的系统数据来自源码初值，
  不是本机遥测。补入当前主机原版 RazerF5 SemiBold 600，避免使用合成字重。

## 先前批次验证（七卡和 Media 阶段）

- `cargo fmt --all -- --check` 与 `cargo check --locked --all-targets` 通过。
  后者仅保留 `layer_button`、`select_profile`、`set_monitor_runtime` 三项既有未使用方法警告。
- 统一资源校验通过：1257 项共享资源源/输出哈希、35 项服务 SVG、27 项 OLED 素材；
  同时检查 133 项 Webpack 请求、74 种基础产品图、59 种基础 Dashboard 图及
  16 个键盘布局的 1901 个输入形状。Armory 专用表另核验 41 张图、21 个产品/edition 身份。
- 原生产品数据校验通过；嵌入 JSON 校验为 41 份、0 失败、4 项既有结构推断跳过。
  这不表示跳过的结构也经过同一 schema 校验。
- 相机挂载/条件/提示层级、设备直达入口、Dashboard 状态、Nommo、Armory、OLED、
  滑条和步进箭头均有对应的当前源码静态收据。没有进行运行时像素或交互验收。

## 静态校验发现并修复的数据串接错误

`accessory_controls_data.json` 中产品 **126** 的 `TAB_LIGHTING` 首段曾错误包含
`179:hyperpolling-pairing`，源码也指向产品 **179**，绑定的
`/runtime/indicatorLedStatus` 在 126 配置中不存在。该错误使 Razer Mouse Dock 的灯光页出现
接收器配对入口，并导致原有数据校验失败。

当前 126 的 `main.766604be.js`（SHA-256
`c9043f49ba6f2dc94f46e6839e19c7246112e6177e8d3493ee6fd4904bb0a135`）
中真实灯光根 `Wa` 位于 UTF-16 区间 `[3913666,3913924)`：左列是 `Va/Ba`，
右列是 `H/fa`。它与 179 的接收器根是不同组件树；126 配置和原生成器均没有这条
179 配对 action。已移除误插的整段，179 仍由自己的专用 receiver 页实现配对。

`validate-native-product-data.py` 现在额外要求每条控件 key 的产品前缀和已声明
source 路径归属其自身产品。没有通过放宽控件种类或允许缺失状态路径掩盖错误。

## 完成边界

上述是明确的局部实现范围。原有 331 个产品入口、1419 个导航页的“部分接入”
统计不能据此改成全产品完成。剩余产品内部控件、独立根、服务观察态及交互细节
继续按源码逐项处理。运行窗口、像素、DPI、焦点和真实设备响应均未验收。

## OLED 编辑器后续并行批次

继续以1383自己的当前源码补齐五个编辑器，现有六个可编辑模式均可从EDIT打开。
补入固定预设槽、104个表情、本地裁剪预览、横幅四方向和字体选项、系统信息三张幻灯片，
并完成共享弹层、null状态恢复和“需要Synapse”提示8px边界处理。
资源增加到四套共176项；具体交互、来源和剩余边界见
[OLED编辑器续接](audio-oled-editors-2026-10-06.md)。上面的27项计数是先前批次的验证记录。

这一批的 `cargo check --locked --all-targets`、格式化检查、源码/资源只读校验均通过，
保留相同三项既有Rust警告。嵌入JSON增加到45份、0失败、4项原有跳过。横幅textarea默认
样式通过当前官方宿主的浏览器版本字符串与匹配的上游UA CSS恢复，不再随意设定其padding。
上述通过仍不表示应用已运行或全产品复刻已完成。

## 中断后的续接批次

- [独立 Settings](settings-window-implementation.md)：托盘四个设置命令打开或聚焦
  `/settings/` 对应窗口，首次选择 Software，重入保留页签；源中不存在旧的
  `settingsScrollToSection` 监听，不强制跳转 Notifications。语言与本地设置同步。
- [Studio 独立根](chroma-studio-native-root.md)与[宿主接线](chroma-studio-shell-integration.md)：
  接入专用画布、图层和工具入口、本地历史与草稿保存。其与 Chroma Dashboard
  保持各自页面身份；原始设备集合为空，不以产品目录填造连接状态。
- [OLED runtime 复核](audio-oled-runtime-current-audit.md)：修正重复语言观察清除暂存值、
  profile restore 和页面重入的状态一致性，保留源码挂载条件和真实观察边界。
- [托盘提示与空态交互](tray-account-continuation-2026-10-06.md)：补回设置提示的
  延迟/淡入淡出、失焦关闭和文本按钮过渡，处理此前丢弃的访客命令。

上述记录只覆盖本批可由当前源码证明的实现；Studio 属性编辑器、设备/LED 服务、
Settings 宿主目录、托盘实际账户/通知与其余产品的未完成项仍保留在各审计中。

Studio 窗口归属经当前宿主 `PC.init` / `MC.handleOpen` 重新取证：它是 Chroma
父窗口中的 policy-3 子标签，父窗口为 1280×720、最小 600×500。没有把它当作
另一独立系统窗口。Settings 的最小尺寸也已从当前 constants.js 的
`WINDOW_SIZE_DEFAULTS` 追溯为 600×500，只有托盘齿轮首次启动覆盖为 1000×768。

最终格式检查与 `cargo check --locked --all-targets` 通过，保留原有三项未使用
方法警告。49 份嵌入 JSON 为 0 失败、4 项原有结构推断跳过；统一资源校验通过
1257 项共享资源、35 项服务 SVG、176 项 OLED 素材、49 项新增根/宿主 SVG 和
1 项 OLED runtime 图标，并继续核验产品图与 1901 个键盘输入形状。
Settings、Studio、托盘和 OLED 的当前源码提取/只读检查通过。没有执行应用、
构建、测试、安装器、厂商 JavaScript 或 DLL；不以编译通过代替像素/运行时验收。


## 属性编辑、只读路线与文档清理批次

- Studio 接入 Ambient 区域预设、模糊度、原始滑条及 Static HSV 调色器，保留工具切换和临时 effectLayer.params 语义。临时属性修改不冒充区域应用或设备保存；原生区域选择与取色仍需服务证据。
- 颜色编辑补齐源 HEX 展开、RGB 数字过滤与上下键、失焦及 Enter/Escape 提交、自定义颜色删除和禁用边界。自定义颜色仅会话保存；HEX 显示大小写仍有源 CSS 差异。
- 图层补齐命名、复制、删除、效果切换及选择恢复；嵌套分组和拖排尚未完成，效果切换菜单形式仍有差异。图层收据为 14 条。
- 独立 Settings 补齐社交图标遮罩、过渡和提示尺寸时序。保持当前宿主窗口策略。
- 新增 [DLL 只读盘点](dll-readonly-inventory.md) 与 [实施路线](ui-readonly-first-roadmap.md)。UI 编辑、增删、应用、保存和本地草稿现在实现；DLL 查询读取现在核实，只有 DLL 修改状态、写回和保存后置。当前官方封装的静态证据仍不等于实际安装 DLL 的 ABI 或读取结果已验证。
- [文档清理](documentation-cleanup-2026-10-06.md)删除 5 份过期总览，重写当前入口，纠正失效引用和历史注释，清除损坏文字并保留独有页面证据。

本批 cargo fmt 和 cargo check --locked --all-targets 通过，保留原有 3 项未使用方法警告。
Studio 根、属性、颜色、图层和 Settings 当前源码检查通过；属性收据 36 条、13 个效果根、16 份 CSS，缺失为 0。
嵌入 JSON 为 49 份、0 失败、4 项原有跳过。资源检查为 1257 项共享资源、35 项服务 SVG、176 项 OLED 素材、63 项独立根 SVG、1 项 OLED runtime 图标；并检查 133 项 Webpack 请求、74 个产品变体、59 个 Dashboard 变体和 1901 个键盘输入形状。

未运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。上述结果不代表视觉或真实设备验收；Ambient/Static 之外的 11 个 Studio 属性根仍待接入，全产品完整复刻仍未完成。


## Spectrum 时长接入批次

接入 [共享时长控件](chroma-studio-duration-native.md) 到 Spectrum：原始三档毫秒映射、标签与帮助、滑条临时编辑与提交、源 SVG 预览及逐帧动画。补齐窗口失活和宿主重开的绑定清理，共享滑条 FocusHandle 设为 Tab stop。保持 effectLayer 临时参数语义，没有写入设备或图层文档。

当前 Studio 原始资源增至 47 张，统一独立根 SVG 数增至 68。5 张新增时长预览图均保持原始 SVG 字节，viewBox 230×80。属性 CSS 收据已包含 duration-preview。四种时长数组从当前模块 6257 静态提取；只有 Spectrum 使用了新组件，其余根没有据此计作接入。

cargo fmt、cargo check --locked --all-targets 通过，仍为原有 3 项未使用方法警告。Studio 当前源码、44 条根收据、36 条属性收据、14 条图层收据及资源静态检查通过。嵌入 JSON 49 份、0 失败、4 项原有跳过。未运行应用、构建、测试或 DLL。

下一项继续 Spectrum 渐变编辑器（3690），包括预设、渐变 stop 增删拖动、颜色编辑和自定义模式。Spectrum 仅部分接入，其余 10 个属性根仍未接入；全产品完整复刻仍未完成。


## Spectrum 渐变编辑器接入批次

[渐变审计](chroma-studio-gradient-native.md)记录本批来源和边界。接入五组预设、自定义渐变缓存、色标增删拖动、颜色编辑、100ms 预设交互过渡和弹层延迟；保留临时参数与提交的分离。共享调色器支持 208px 弹层布局，修正回到初始颜色时遗漏提交及画布裁剪。

属性提取器补入 selector/dropdown/gradient CSS 模块，现为 39 条模块/函数收据和 19 份相关 CSS。仍为 47 张 Studio 资源、68 项独立根 SVG。cargo fmt、cargo check --locked --all-targets、当前源收据、资源与嵌入 JSON 检查通过；49 份 JSON、0 失败、4 项既有跳过；Rust 仍为原有 3 项未使用方法警告。JSON 静态解析器不能识别泛型内的限定类型路径，已改为正常类型导入，未放宽验证规则。

未运行应用、构建、测试或 DLL。Spectrum 现在已有时长与渐变，仍需核对原始光标、弹层边缘行为、触发框过渡与实际像素；其他 10 个属性根未接入。下一批继续这类共享控件边界及 Breathing 的独立颜色、时长和 Playback 链。


## Breathing / Fire 属性接入批次

[本批审计](chroma-studio-breathing-fire-native.md)记录双色弹层、随机开关、Breathing 时长和 Playback 本地参数链。新增共享颜色下拉、Checkbox、Playback 控件；修复初稿 GPUI API 用法、菜单动态宽度和重复步进的 300/400ms 边界。工作参数保留 Breathing 特有的黑色/无色哨兵；原始哨兵 RGB/HEX 字段显示尚有差异。没有写入设备区域或 DLL。

当前 5 个属性根局部挂载，其余 8 个未挂载，取代上文较早批次的 10/11 个剩余数量。所有根仍为 partial。源主收据 45 条、属性收据 40 条、19 份 CSS；新增复选框 keyframes 原文。格式化、cargo check、源收据/资源及嵌入 JSON 校验通过；仍为 3 项既有 Rust 警告，49 份 JSON / 0 失败 / 4 项既有跳过。未运行应用、构建、测试或 DLL。

用户要求的独立核验子任务完成[首批报告](prior-ui-verification-2026-10-06.md)，确认 4 项既有 UI 缺陷，主实现尚未修复。子任务继续追踪 1382 映射候选和鼠标页面，主线程后续按报告当前源码逐项修复，并继续 Studio 剩余根/共享控件差异。


## 独立复核首项修复及第二批结果

修复 3334/3337 的四个已接入混音推子：新增组合 gate，使源主开关和各自总线开关共同控制显示/事件写入。生成器重新校验当前根 SHA 与 UTF-16 原文，保留补充收据；描述符 diff 已确认仅新增这两个产品四项 gate。未把同家族其他产品未经核对一起修改。

独立子任务回读确认 A 的静态修复，并将 1382 的实际 MapAudio 加载链及鼠标 70 DPI stages 条件/拖排追加为确定缺陷。当前报告 A–F 共 6 类，A 静态修复，B–F 待完成；上文“四项均未修复”仅指前一时点。

修复后的 cargo fmt、cargo check --locked --all-targets 通过，保留原 3 项警告。音频静态验证检查 76 产品、1756 控件、77 组 EQ、2829 个当前源哈希；3 个既有空页面保持显式缺口。嵌入 JSON 49 份、0 失败、4 项既有跳过。未运行应用、测试或 DLL，未完成视觉/设备验收。


## 独立复核 C：716 Copilot 状态行

已补入 716 的 Copilot 禁用状态行，沿用当前源码的实际键表条件与 isWindowsKeyDisabled 勾选状态，始终不可编辑。当前语言符号已通过模块 54693 静态解析核实，10 份本地语言文件包含原键。修复范围只包括当前已复核的 716，不将其他键盘仅凭键名推定为已接入。

cargo fmt、cargo check --locked --all-targets 和鼠标/键盘静态验证通过；仍有 3 项既有未使用方法警告。独立报告 A、C 已静态修复，B、D、E、F 继续待办；尚无运行、像素或设备验收。Studio 仍为 5 个属性根局部接入、8 个未挂载。


## Settings D 的空目录分支与本地偏好链

当前 9762:fe/ge/ve、6584:u/h 已重新静态读取：无 app.items 时仍有 showMenu，储存键为 systrayDblClickAction，源 ge 显示文案由 DROPDOWN_SYSTRAY_1 提供。新增 `settings_systray_action.rs`，按当前 CSS 布置触发器、选项、箭头与高度时序。选择通过 SettingsPage 修改本地 AppPreferences，复用已有辅助保存链；托盘双击读取同一偏好。旧本地工作区缺少字段仍默认显示菜单。

没有虚构已安装应用，真实应用目录和 launch 分支保持待办；该项只算局部修复。页面切换/刷新清理下拉计时任务，键盘焦点/外部点击由 Base Popover 管理。边框/hover 过渡、完整键盘和边缘行为仍需继续对齐。独立复核子任务已收到回读请求，包含前批 716 修复。

cargo fmt、cargo check --locked --all-targets 和 Settings 源码校验通过（33 AST 收据、556 CSS 规则、10 语言）；嵌入 JSON 49 份 / 0 失败 / 4 既有跳过，Rust 为 3 项既有警告。未运行应用、构建、测试或 DLL。


## Settings 收起反馈修复，鼠标 70 DPI 改造准备

独立复核已确认 716 窄范围修复及 Settings showMenu 的本地保存/托盘消费链，同时指出外部关闭触发器 active 提前结束。主线程现已将 active 与 open/mounted 分离：外部收起延后 100ms 清除，选项点击仍立即清除。格式化和 cargo check 通过，3 项既有警告未变。

鼠标 70 的挂载链与原生多行编辑将作为下一阶段工作。新增 `tools/audit-mouse-70-dpi.cjs` 和 `mouse-70-dpi-current-evidence.json`：7 项当前 AST 收据、2 份 CSS、SHA/manifest 校验。已追通关闭阶段仅当前行、开启时逐行编辑、可见阶段选择和拖排后的 activeStage 修正规则；数据提取不算界面完成。没有为赶进度只隐藏阶段按钮来替代完整多行编辑。

未执行应用、构建、测试、下载的 JavaScript 或 DLL。目标继续保持未完成。


## 鼠标 70 DPI 多行编辑接入与独立回读

新增 `src/features/mouse_dpi_rows.rs`，仅对已核对挂载链的产品 70 接入逐阶段 X/Y 编辑、独立 XY、阶段可见性及拖排。关闭阶段时保留当前行可编辑；至少保留两个可见阶段；隐藏或移动当前阶段按当前源重映射活动阶段。拖排后同步保留的输入/滑块状态，拖放校验所属视图和原始草稿快照。UI 修改仍进入既有本地草稿链，未接入 DLL 写回。

当前 70 配置声明 minDPI=100、maxDPI=16000、dpiStep=1，未声明 dpiStepInBox 或 isSensitivitySliderWithGrid；该源的滑块与输入步长均回落至 1。9 张原始 SVG 已按当前 manifest 取得，并逐字节核对与复用资源一致。证据校验仍为 7 项 AST、2 份 CSS。

原生输入、滑块与开关皮肤、悬停才显示操作、精确拖动预览/插入提示和部分边界尚未对齐，因此只是局部接入。独立验证子任务已恢复，对实现与当前源单独回读，报告仍使用 partial 口径。

cargo fmt --check、cargo check --locked --all-targets 及 DPI 收据校验通过；Rust 保留 3 项既有未使用方法警告。鼠标/键盘静态验证覆盖 76 个鼠标规格/862 个输入及 71 个键盘/6494 个形状；资源校验通过。没有运行应用、构建、测试、厂商 JavaScript 或 DLL；运行交互和视觉仍未验收。


## 鼠标 70：独立复核后的 XY 分支及悬停修正

独立复核追到 cI.toggleY → II/$xU、TI/Tme → zE reducer：仅切换 Independent 时不改变活动阶段；关闭独立 Y 且 X≠Y 时才同步 Y=X，并将该行设为活动阶段，即使该行隐藏。原生现用专用 `dpi_toggle_xy` 保留这两个分支，避免普通数值编辑的 visible 条件改变源行为，同时同步保留的输入/滑块实体。

XY、可见性开关和拖动柄现在随行悬停显示；单行模式继续隐藏后两者。插入提示按拖动方向显示上/下边框，同一位置与其他视图拖动不显示提示。增加原始活动阶段索引越界保护，避免异常草稿在重排时越界访问。

证据新增 TI、II、zE，总计 10 项 AST、2 份 CSS、9 张已核对资源；审计脚本加入版本控制白名单，避免遗漏可复现工具。首次 cargo check 发现 Switch 本身不提供 group_hover，改由外层布局持有悬停样式后检查通过，仍为 3 项既有警告。保留原始输入/滑块/开关皮肤、拖动浮层、XY 提示和其他细节缺口，F 仍为 partial。


同批继续修复独立复核发现的数字草稿边界：产品 70 阶段输入限制为最多 6 位数字及可选负号，清空或仅负号提交按 0 再钳制为 100；提交后总是回填规范文本，即使滑块已在相同边界值，避免输入 16001 后仍显示越界文本。当前模块 4230 已加入原文收据，总计 11 项。修改仅作用于已核对的 70 阶段输入；未将其他产品按名称类推。拖动浮层文案已核实 4693:Z8V→FE→MOVE_STAGE 并修正，编号尚需追 Active 与 visible 的来源差异。数字键盘/滚轮/长按步进和原始皮肤仍需完整对齐。

数字修复最终格式化与 cargo check --locked --all-targets 通过，仍为 3 项既有警告；11 项源码收据及鼠键规格校验通过。独立子任务已静态回读确认 XY 和两项数字边界修复，报告保留剩余交互、外观及默认状态拖动编号疑点；未进行任何禁止的运行验证。


## 产品 70 数字交互与 XY 提示

新增 `mouse_dpi_number.rs`，采用 Base NumberInput 提供方向键/语义步进，独立保留点击注册、文本类型、重复任务及鼠标点击去重状态。对照 4230 与 EI 的本地预览分支，点击框后的步进只更新显示，失焦提交；未注册的步进直接修改本地草稿。接入每 300ms 长按重复、控件内滚轮、Enter/Escape 失焦和 XY 提示。

新取回的两个 stepper SVG 与嵌入资源逐字节一致；审计补入通用 stepper/spinner CSS，仍为 11 项 AST/2 份 CSS，资源增加至 11 张。当前实际实现与剩余窗口级 wheel、原控件布局、动画及拖动编号问题统一记在 [产品 70 编辑器记录](mouse-70-dpi-native.md)，不将 partial 升为完成。

本批最终 cargo fmt 与 cargo check --locked --all-targets 通过，Rust 仍为 3 项既有未使用方法警告。11 项收据、鼠标/键盘规格及资源校验通过；未运行应用、测试、下载的 JavaScript 或 DLL。独立复核新增 4115 Kitsune 的设备图/列布局/SOCD 说明差异，已进入剩余队列。


## Kitsune 原根布局与相机快捷键禁用

[4115 Kitsune 专门实现](kitsune-native.md)已接入当前设备 SVG、左侧 polling/mode、右侧五种 SOCD 单选/独立说明及原图。单选项使用源 200ms Ease 过渡，layout_id 从设备元数据传入，不对其他 layout 强行显示 layout 0。42 AST/8 CSS/2 SVG 已生成并静态校验，工具加入白名单，资源表 load/list 均已登记。帮助提示、macro 独立条件和真实读取仍未完成，G 按局部修复记录。

另按独立 H 修正 3592 快捷键禁用遗漏：keys 渲染消费同一 disabled 条件，禁用时整体 opacity=.3、去除悬停/指针提示；开始监听、捕获及清除写入口均重新检查当前条件，防止旧事件绕过。保留真实 inputredirect 与全局快捷键服务缺口，不把本地草稿当作设备保存。

最终 cargo fmt/cargo check --locked --all-targets 通过，仍有 3 项既有警告；Kitsune --check、资源、嵌入 JSON 均通过（50 份/0 失败/4 既有跳过）。原生描述符静态校验通过，但不据此宣布运行禁用交互或全部产品完成。

独立回读已确认 G 的主体布局/原图/五条 SOCD 描述及本地写入链补入，H 的监听开启、捕获及清除三条本地 UI 写入口均受当前禁用条件限制。Kitsune 收据另补当前 Radio 模块 49412，总计 42 项，并检查所有本地语言文件含本页所用键。G 仍 partial，H 为已追踪缺陷的静态修复。


## Kitsune 帮助提示与 2636 低死区回退

三个 Kitsune 面板已接入原 tips 文案及 14px 帮助图标，复用已核对的即时 portal 和图标底色 300ms 过渡。原 questionmark 资源逐字节匹配共享 SVG，准备器记录 shared_assets 并核对字节；补入 O_ help/tip CSS。共享定位 helper 仍以视口近似 body-wrapper 边界，保留差异。宏分支正在核对实际选择资格，不能把源码内存在分支等同于当前产品可达入口。

独立 I 的 2636 回退规则已局部修复：进入 THUMBSTICKS 时缓存左右当前死区，只有 >=7 编辑刷新缓存；继续使用低值不覆盖回退基准。7→3→Continue→5→关闭现在回退到 7，其他手柄未未经核对推广。完整零值/低值弹窗、Recalibrate、关闭按钮与展开说明仍待接入。

独立复核已确认 2636 回退缓存窄修复；Kitsune 提示保持边界近似的 partial 口径。Macro 可达性已核到当前选择器与宿主运行时转发：4115 默认类别不入选，当前静态源无 supportMacro=true 证据，不能据共享 hP 分支宣称缺少一个必须新增的入口。另已修复独立报告 G/H/I 尾段的中文编码并由子任务回读。格式化、cargo check、42 项 Kitsune 收据与嵌入 JSON 校验通过；未运行应用、测试、厂商脚本或 DLL。


## 2636 警告弹窗与独立 1382 复核

2636 已从内联通用提示改为 Base Dialog：零死区和低死区文案分支、Continue 保留值、右上角关闭回退、Recalibrate 发事件经父工作区导航到校准页、展开说明的 250ms 最大高度与 200ms 透明度/箭头过渡。没有新增硬件校准或 DLL 写入。定位使用原生页容器测量及源码 margin -40/-20、内部 margin-top 110；主页面其他布局尚未完成，不能认定整体像素一致。

来源见 [专项说明](gamepad-2636-dialog-native.md) 与 [22 项 AST/CSS/资源收据](gamepad-2636-dialog-current-evidence.json)。本轮 cargo check --locked --all-targets 通过，原有 3 条 unused-method 警告；未运行应用、测试、厂商脚本或 DLL。

独立子任务已重审 1382 AudioFunction（报告 E）：默认 audioGroup 和后端 AudioDevices 查询已存在，应补的是当前 UI 对查询结果的消费、Playback/EQ 完整子编辑器，以及父 Save/Cancel 与本地草稿链。不能重复报告为后端枚举功能不存在。报告包含当前入口、设备相交条件、payload 字段和不可臆造的校验限制。

2636 本轮独立静态复核确认三条操作链、22 项源码收据和展开过渡，仍记录 Base Dialog 全窗口输入/焦点约束与源容器遮罩范围的差异。资源校验通过（新增 2 个手柄弹窗 SVG），嵌入 JSON 为 50 项通过、0 失败、4 项既有跳过。


## 1382 Playback 临时编辑与只读数组消费

已新增 control_pod_audio.rs，并在1382已启用的音频映射行挂载。实现 Playback 三动作、双设备/None、已保存断开端点回填、本地 Save/Cancel、可选 audioGroup payload 恢复；打开编辑器时以现有 ServiceClient 在后台查询 AudioDevices，不调用 DLL 写入。当前 Rust 返回已解析数组，不能沿用网页 wrapper 的 deviceList 字符串层级。

独立复核发现并促成修复两项边界：Reset 祖先路径必须使旧编辑器失效，且 Save 需再核身份/类型/启用/基准；null/空串/0 首设备 ID 应按源假值处理。异步读取只更新原编辑器，不跨编辑器写状态。

EQ 当前只显示源无可用设备分支：本地设备目录、预览和历史固定快照不能作为官方 validDevices。真实观察、EQ 设备相交及完整子编辑器、父映射根/dirty提示仍待完成。详见 [实现与边界](control-pod-audio-native.md)。


## 1382 自身关闭时的保存提示

已追当前主根 displaySaveAlert/dismissSave/saveChanges/dontSave/nextAction/dontSaveAction/renderSaveAlert 及913 closeMapping/saveMapping。31项源码收据、4份CSS和当前共享关闭图标hash记录在 control-pod-audio-current-evidence.json。

实现仅覆盖编辑器自身×：dirty时提示；Save提交并关闭，Don’t Save只清dirty并canSave=true但保留内容/编辑器；提示×保留dirty、canSave=true；底部Cancel直接关闭。没有将源特殊分支改成常规放弃并切换，也没有虚报换键、切页、历史导航或刷新流程已完成。新control_pod_audio_warning.rs含源码面板几何、100ms显现和300ms按钮透明度，已获独立静态行为/尺寸核验。


## 1382 换输入与产品导航保护

本轮延续上一批自身关闭提示，将现有音频输入切换以及SourceProductWorkspace产品页/前后导航接入同一保存保护。产品页面和历史索引在确认前不变；Save先提交旧草稿再执行原切页或历史方法，Don't Save/提示×不执行目标。目标音频入口在提交后重验，旧实体/基准值保护保留。无dirty清旧编辑器后直接导航，包含Help页。

新增当前 clickBtn/changeView/navigateBack/navigateForward 静态证据，累计35项源码收据、4份CSS和关闭图标收据。应用级跨设备/主导航、其他映射类型、外部点击排除表、面板折叠和刷新仍待接，不能把本批计为完整父映射根。

本批独立静态复核确认事件FIFO下capture先于导航、历史回放不追加条目，未发现本批明确阻碍。格式化、cargo check及35项源码收据检查通过，仍为原有3条unused-method警告；未运行应用、测试、厂商JS或DLL。


## 1382 EQ 条件投影与本地提交

接入设备选择、三种 EQ 动作及五种预设，按当前源分别计算合格 AUDIO 产品存在条件 w 和 containerId 相交的端点 K，保留按完整数组长度触发的投影及源 canSave/dirty 回调规则。运行时记录不混入本地 profile；当前只有观察转发入口，真实宿主 validDevices 发布链仍未接通。

本地映射按源条件构造 equalizerPayload，缺失可选字段不写成 null。所选索引失效或对应实时记录消失时，普通 Save 和未保存提示 Save 均显示错误、保留草稿且不导航，不自动改选其他设备。详见 [1382 当前实现](control-pod-audio-native.md)。独立子任务已将复核结果追加到报告 E，并继续逐项核对既有界面和产品。

cargo check --locked --all-targets 通过，仅三项既有未使用方法警告；未运行应用、构建、测试、厂商 JavaScript 或 DLL。当前仍为局部实现，真实读取发布链、完整映射根和应用级保护保持待办。


## 3334/3337 混音运行态初值修复

独立验证第二批发现：两款当前 yU 根读取 streamMixerReducer，但描述符生成器提前复制 DEFAULTPROFILE 的 true/50，遮住后续 reducer 初态分支。现已限定这两个经过核对的产品，从当前 Wn 对象静态提取主开关、监听开关及两条总线；同时检查 yU 的四个字段确实选择 streamMixerReducer，并保存初态和 selector 的源哈希、UTF-16 范围及原文收据。

无已保存草稿时，主开关现在为 false、两条总线均为 isEnabled=false/value=0，监听仍为 true。原 profile 默认字段保持原文；既有本机草稿继续由 restore 合并，不把用户已编辑值强制迁移为新初值。这只是来源修正，不能视为真实设备状态已读取。

独立复核还明确了数字直接编辑、冲突开启确认、Playback 设备选择和离线提示、输入通道增删以及 preset shortcut 缺口；继续逐项实现，不能归入 DLL 写回后置。

cargo check --locked --all-targets 通过，保留三项既有警告。音频静态校验通过 76 产品、1756 控件、77 组 EQ 和 2829 个当前源哈希，3 个空页面仍显式保留。嵌入 JSON 为 51 项通过、0 失败、4 项既有跳过。未运行应用、构建、测试、厂商 JS 或 DLL。


3334/3337 两条混音总线已补入[数字编辑与步进](stream-mixer-number-native.md)：文字暂存、Blur/Enter/Escape 提交、源字符串加号语义、主开关与总线组合禁用、300ms 长按及本地草稿同步。原窗口级滚轮、完整行布局及边缘交互保持 partial；设备选择、通道增删、冲突确认及预设操作未因此计为完成。6 条 AST/4 次资源比较、cargo check 与静态数据校验通过，未运行应用、测试或 DLL。


## 2026-10-06：混音确认/播放选择与独立复核 J–L

3334/3337 已接入 [开启冲突确认与播放设备选择](stream-mixer-selection-native.md)：确认前不提交、取消/确认、本地选择保存、断开名称回填、主开关和过期菜单确认保护，以及原警告资源。运行列表/冲突状态与本地覆盖分开，未选择不把观察值写入快照。28 项当前 AST 收据、4 次产品资源比较、cargo check 和相关静态校验通过；真实 MW 发布者未接，不能计为 DLL 读取完成。完整布局、电平表、输入通道增删、预设/快捷键和窗口级滚轮继续待办；产品仍为 partial。

用户要求的独立子任务已完成 515 Blackwidow Chroma 本批复核，新增 [报告 J–L](prior-ui-verification-2026-10-06.md)：Gaming Mode 缺少 Menu 禁用状态行；Snap Tap 成对编辑、新增/删除与校验主体未挂载；Keyboard Properties 右列与操作入口缺失。已核对正常挂载链、源码范围和本地实现，不把共享组件存在视为界面已完成。其中 J 的 Menu 只读状态行已按 515 和 KEY_APPLICATION 条件补入，checked 取 isWindowsKeyDisabled，没有新增写入口；K/L 的 Snap Tap 与系统属性入口继续进入后续修复队列。只读接口继续核实，DLL 写回仍后置。


## 续接：515 Snap Tap / Keyboard Properties（2026-10-06）

新增 `keyboard_snap_tap.rs`、强类型静态数据、`prepare-snap-tap.cjs`、3 SVG：本地 reducer/staged 分离、四组上限、KEY1/KEY2、重复/禁键、删除回滚、失焦/快照、一秒闪烁和三秒消息。快照仅通过 `_snapTapLocalV1` 认定本地修改，保留旧 profile 种子；AppShell 只转发当前可见设备释放事件。4 类观察只预留入口，没有真实发布者。详见 keyboard-515-snap-tap-native.md 的明确剩余能力和宿主生命周期限制。

新增 `keyboard_properties.rs`、`prepare-keyboard-properties.cjs`、2 SVG，在 515 右列连接现有 `system::open(Properties::Keyboard)` 与失败通知；无 dirty/设备写回。详见 keyboard-515-properties-native.md。独立子任务扩充 L 可实施证据并回读本批实现，另完成 226 滚轮 M；下一批应依据 M 补三模式/禁用回退/等级/FreeSpin 锁定。

已通过允许的 cargo check、格式化、52 份嵌入 JSON（0失败/4既有跳过）及当前源/资源静态校验。未运行应用、构建、测试、系统属性窗口、厂商JS或DLL。整个 goal 保持 active，331 产品 / 1419 主页面 partial、完整产品 0；不得重置工作区现有修改。

## 226 滚轮批次续接（2026-10-06）

[实现记录](mouse-226-scroll-native.md)与 `mouse-226-scroll-current-evidence.json` 记录本批三模式、禁用回退、两组等级与 FreeSpin 派生锁定。`ScrollWheelEditor` 只服务 226，Changed 原子保存完整草稿及 `_scrollWheelLocalFieldsV1`；观察不进 snapshot，等级 Change 预览 / Release 提交，模式服务请求 debounce 留待写回阶段。Help 路径已清理滚轮瞬态并停止隐藏 515 Snap Tap 的按键捕获。

本批 cargo check、格式化、52 项 AST / 83 条 CSS 校验及 53 项嵌入 JSON 校验通过，未运行应用/测试/DLL；仅三项既有 dead-code 警告。独立子任务继续专门回读已有产品，报告 M 已核主体/字段归属，N/O 已明确下一批 226 Performance 缺口。真实滚轮观察发布者、整页布局、运行视觉与全部产品复刻继续未完成，完整产品仍为 0。

## 226 DPI 数字框续接（2026-10-06）

[整数框](mouse-226-dpi-number-native.md)已接 226 当前选中行，规范空值/99/上限的显示与本地保存，补 Enter/Escape、注册预览、键盘/300ms 步进、控件内滚轮和页面清理。当前源 4230、es/ls/Ms 与参数由新工具 `audit-mouse-226-dpi-number.cjs` 记录，16 项 AST、55 条 CSS、2 次资源比较通过；缺失的原箭头已从 manifest 指定 URL 补抓且与嵌入资源字节相同。

共用 `mouse_dpi_number.rs` 不再硬编码 Y/Independent，Base 自动 mask 显式关闭；70 对应行为继续保持。Help 方法改名为 `dismiss_editors`，同时取消数值预览和滚轮瞬态。独立子任务已完成回读；发现的滚轮预览值限位及未注册吞事件两项已修复并确认。最终 cargo check/格式化/16 项源码收据检查通过；嵌入 JSON 仍为 53 项通过、0 失败、4 项既有跳过。226 阶段条件/逐行可见性与拖排、分段滑条、Polling Rate N/O、真实只读发布者和全项目目标保持未完成，不运行应用/构建/测试/DLL，不重置已有修改。
## 226 DPI 阶段与分段滑条续接（2026-10-06）

新增 `mouse_226_dpi.rs` / `mouse_226_dpi_data.json` 和 `prepare-mouse-226-dpi.cjs`，现 26 AST / 154 CSS / 9 资源；阶段行复用已独立适配 70/226 schema，包含 visible、两类回退、XY、拖排和 restore 代际。Grid 百分比位置与实际 DPI 模型分开，拖动只预览、释放提交；端点取整越界、原位松开、回传反算、层次及焦点问题均已按独立回读修正。详见 [本批记录](mouse-226-dpi-native.md)。

用户新增要求多个子任务同时校验原代码，现有 verify_existing_ui 与新增 verify_dpi_slider_source / verify_polling_read_source 分别形成报告。轮询子任务补取 `.ref/middleware/226/` 89 个当前 manifest JS，全部仅下载/静态校验，未执行。`SET_ENABLE_STAGES=!isOTFSEnabled` 生产链已证；UI 已接独立 Option 观察门控及所有写入口检查，只清 DPI 预览，真实发布者尚未连接。Polling getter 为 HID 协议、mapping isOtfsActive ABI/owner 仍需后续核实，不能混用现有 Query 或执行写任务作查询。

下一批继续 226 O 的 BLE/连接身份、轮询字段和限速说明，再推进产品其他区域与完整目标。所有未提交工作与 Cargo.lock 保留；全目标 active、完整产品 0，不运行应用/构建/测试/DLL。
