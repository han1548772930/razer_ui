# 尚未完成的界面与产品

2026-10-06 OLED后续：1383的动画/图片/表情/横幅/系统信息编辑器已接入原EDIT入口，
与已有Media共同使用源弹层。当前范围和真实未完成项见
[OLED编辑器续接](audio-oled-editors-2026-10-06.md)；下文旧日期记录中的“五编辑器未接入”已过时。

2026-10-06 当前续接进展见 [本轮记录](ui-continuation-2026-10-06.md)：Nommo 1303/1304 效果参数、相机分组与条件、七个新增 Armory 独立根和精确产品图片、AppPicker 本地入口、公共滑条过渡已继续接入。下文是按日期保留的历史记录，不可把早期缺口或早期“已实现”结论直接作为当前实现证据。尤其 2026-10-04 所称“Chroma Studio 已本地实现”不准确：本地 Chroma Dashboard 与独立 Studio 是不同目标，当前不能把 Dashboard 跳转计作 Studio 实现。当前主页面覆盖口径见 [原生页面覆盖](native-product-coverage.md)，所有产品仍为部分实现。

2026-10-06 784 Aether 链接形态与图标提示框收口：源里 `Gl` 的两个调用点都不传 `supportsLinked`（恒 undefined）、`hasPowerButton` 恒为默认 true、卡片 `<ol>` 没有 `isLinked`，因此 `.button--cta-link`、`.indicator.linked`、`.device.linked` 在当前页面**不可达**——本轮不再把它们列为缺口，也不实现不可达 UI（相关文案 `DEVICES_*` 本地已有，将来某产品真传该 prop 可直接接）；同时 `TipCommand::icon` 的 `trigger.origin + (2, 34)` 无源码依据，改用全局 `[tooltip]:before{right:0;top:calc(100% + 5px);width:auto;white-space:nowrap}` 对应的 `TipAnchor::Right` + 5px。机检并入 `tools/validate-aether-strip.py`。

2026-10-06 784 Aether 徽标可见性：四个徽标按源改成「每张卡片都渲染、按状态决定可见」——`power/find` 仅选中且非离线（忙碌时仍可见）、`del` 仅选中且(`:hover` 或离线或忙碌)、忙碌徽标在任意忙碌卡片上（选中用 `busy-btn-white`/`#fff` 边框/`top:calc(50% - 8px)`，否则 `busy-btn`/`hsla(0,0%,44%,.302)`/`- 10px`，42×27 居中，选中时 `:hover #707070`）、离线徽标在任意离线且非忙碌卡片上（选中 `right:10%;top:34%`，否则 `right:25%;top:40%`）；`tools/validate-aether-strip.py` 增加 11 条 CSS 与本地状态表达式断言。**数据缺口**：`isLinked`（`.device.linked` 的隐藏分支）与 `hasPowerButton` 本地 `Observation` 里没有，已登记。

2026-10-06 784 Aether 编号项 tooltip：按源 `.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px);width:fit-content}` 用新的通用挂载 `SourceTipWrap`（`TipAnchor::Start` 左对齐、间距 10px）替换 Kit tooltip，并与徽标共用同一个 `[tooltip]` 皮肤；`TipAnchor` 的 `left:50%` 改名 `Half` 以免与左对齐混淆。**仍未实现**：`.button--cta-link` 连接按钮（四个图标状态）与 `.indicator.linked` 组；`TipCommand`（Icon/Help）仍无调用点（784 的 Aether 页区段里没有 `hasSwitch`/`tips`，本地也没有帮助图标），其 `+2/+34` 无源码依据，待确认后删除或按源接上。

2026-10-06 784 Aether 卡片徽标 tooltip：按源实现 `[tooltip]` 的三种锚点（`.anchor--left{left:50%}`、`.anchor--right{right:0}`、`.anchor--middle{left:50%+translateX(-50%)+width:200px+break-spaces}`）与全局皮肤（`line-height:16px`、`nowrap`、`width:auto`、`padding:8px 10px`、`.3s linear`、`top:calc(100% + 5px)`），新增 `TipAnchor`/`Kind::Badge`/`SourceTipItem` 取代 Kit tooltip 与无依据的 `+2/+34` 偏移；四个徽标的锚点按源分别为左/左/右/中。（编号项部分已收口。）**仍未实现**：`.button--cta-link` 连接按钮与 `.indicator.linked` 组；`TipCommand`（Icon/Help）目前无调用点，其 `+2/+34` 偏移无源码依据，需接上 Fd help portal 或删除。

2026-10-06 784 Aether 编号指示器：按源补上 `.indicator--list`（`#111`/`1px #ccc`/圆角 40/`padding:5px`/`margin-right:10px`）药丸容器与 `.indicator--container` 的居中/`margin-top:10px`，编号项 26×26、`rounded(30px)` 与选中/悬停/警告配色保持与源一致；`tools/validate-aether-strip.py` 增加 `.indicator--container/-list/-item`、配色与 tooltip 覆盖的 CSS 断言和本地标记。（徽标部分已在上一条收口。）**同时登记仍未实现**：编号项的 `[tooltip]` 定位（`right:auto;top:calc(100% + 10px);width:fit-content`）、`.button--cta-link` 连接按钮（含四个图标状态）与 `.indicator.linked` 的 `button--cta-show-*` 组；本地暂用 Kit tooltip 近似。

2026-10-06 784 Aether 轮播容器与居中：按源码 `H()` 把 `scrollLeft` 设为 `carousel.scrollWidth / 5 * index`（`AetherStrip` 新增 `carousel_scroll: ScrollHandle`，挂载/切换重算、0.5px 内不重复设置），容器改为 `.carousel--inner` 的 `padding:0 496px` + 无滚动条可滚动行（源码 `overflow:hidden`，不再用会画滚动条的 `scrollable_both()`），单设备走 `.carousel--inner.center`；`scroll-behavior:smooth` 无源码时长，按同一偏移量直接定位、不自造缓动。`tools/validate-aether-strip.py` 已把容器、偏移量与两层卡片结构一并断言。

2026-10-06 外壳条件块事实登记（`centerTips`→`.widget-body-tooltip`、`disableSwitchTips`→`.widget-switch-tooltip`、`hasFWUpdate`/`fwUpdateComponent`→`.warning-fw-update`）：读完外壳 `render()` 后确认触发条件与全部 CSS，并逐一找出真正传值的当前源——分别是 207/717、580、耳机 1398/1401/1404/2638/2641/2644/4124；这些产品本地都还没有页面，因此本轮不实现无入口的假 UI，只登记在 [组件页 CSS 审计](device-page-css-audit.md)，等对应家族接入时再做（`warning-fw-update` 还依赖本地没有的固件版本数据）。

2026-10-06 784 Aether 轮播盒模型：按当前源把卡片拆成 `.carousel--item`（248×212、`flex:none`、`justify-center`、`padding:0 30px`）与内部 `.device`（未选中 `opacity:.5` + `margin-top:75px`、选中 `padding-top:28px`）两层，图片 237/154.8 与动作按钮定位不变；`tools/validate-aether-strip.py` 增加六条 CSS 与两层结构断言。（其中「容器与选中居中」已在上一条收口。）

2026-10-06 组件帮助提示按源码改回 portal 语义：`.tip body-widget-tip-portal` 由 `createPortal` 挂到 `body`，`.body-widget-tip-portal{opacity:1;position:fixed;right:auto;visibility:visible;z-index:10001}` 覆盖 `.tip` 的淡入，坐标由外壳 `positionTip()` 逐步回退（默认 `widget.right-14-tipWidth`/`widget.top+34`，右/左溢出贴容器，下溢出翻到 `.help` 右侧、再右溢出翻到左侧、仍溢出上移 10px）。本地 `SourceTooltipKind::WidgetTip` 原先按 300ms 淡入 + priority 200 + 只做静态锚点，现与 `ReceiverWidgetPortal` 共用 `shell_tip_position`（0ms、10001、完整回退）；容器仍以窗口视口近似（源码用 `.main-container > #body-wrapper`，本地不测量），已写入证据。机检 `node tools/audit-widget-tip-position.cjs --check`，覆盖广度 299/331 个设备包。

2026-10-06 显示器显示页结构：按源码改成 `.body-widgets.flex` 双列（每列 `.widget-col` 600px）并采用各产品的列内顺序（3858：来源/PIP ｜ 自适应同步/HDR/FPS 计数器；3880：来源/PIP ｜ 自适应同步/刷新率/FPS 计数器），同时把 PIP、自适应同步、FPS 计数器、HDR、THX 五个组件的开关移到标题行（新增 `surface::panel_with_title_switch_opt` 实现 `hasSwitch:!disabledReason` 时完全不渲染开关），删除本地自加的带标签开关行与 FPS 组件里错放的 `uiRestraint.refreshRate` 说明，并补上缺失的 `FREE_SYNC_MSG` 正文。机检 `node tools/audit-monitor-widget-layout.cjs --check` 已纳入两页列组成、五个 `hasSwitch` 表达式与本地标记。

2026-10-06 3880 显示页刷新率组件（`jSA`）：删掉自写占位面板，按源码实现 `title:REFRESH_RATE_HEADER`/`tips:PERFORMANCE_MODE_SCREEN_REFRESH_RATE_TOOLTIP` + `zrA` 限制行 + `.widgetContent`（说明段、`.PillsSelectBox` 胶囊、含 `{{displaySettings}}` 链接的第二段），胶囊 CSS 逐条照抄，选项走 `supportedRefreshRate`/`selectedRefreshRate`（源的 60/120/144/165 兜底），链接走 `backend::system::open_display_settings()`（对应源 `msSettings("display")`，方案取自 Dashboard 自身的 `cmd /c start ms-settings:` 写法，已在审计里注明 DLL 不可读）。机检 `node tools/audit-monitor-widget-layout.cjs --check` 已把该组件的键映射、CSS 与本地标记全部纳入。

2026-10-06 显示器组件体间距：新增 `surface::widget_content` 实现 `.widgetContent{display:flex;flex-direction:column;gap:20px}`，按逐包枚举的结果只接在源确实使用 `.widgetContent` 的两处（Game Mode 的 6 个子元素、色彩温度的 2 个子元素，`.slide-off` 内三行保持紧贴）；PIP/Adaptive Sync 的 `.widgetContent` 只有 1 个子元素，间距不适用；同时登记「3880 显示页应渲染刷新率组件（`$T1`＝REFRESH_RATE_HEADER：说明段 + `supportedRefreshRate` 胶囊 + 打开 Windows 显示设置的链接），本地仍用 3858 的角落网格组件」这一产品分支缺口。机检 `node tools/audit-monitor-widget-layout.cjs --check`。

2026-10-06 无线 ARGB 端口行：`.icon-close-glitter` 按源改为「`.`port-item` 悬停才出现」（源用 `display:none`，所以未悬停时不占位，出现时 `margin-left:-10px`），本地按行记录悬停状态后再挂载删除控件；`.port-add-bend` 按产品各自的声明实现（3884 的 `.underline:after` 1px 灰线悬停转绿 + 文字转绿，3886 只有 `text-decoration:underline` 且无悬停变色），颜色进 `theme::Colors::add_bend()`，机检 `tools/validate-wireless-argb.py` 新增两组断言。

2026-10-06 设备约束原因行：`zrA`（3858）/`iTA`（3880）的原因段落改为按源码放在每个组件体的**首位**（`exclamationText[ mb20]` 的 14px 圆点 + 继承色文字），补齐了此前完全缺失的 Gaming 与 Color 两处，并把 THX、HDR、Color Profile、PIP、Adaptive Sync、FPS 计数器六处从「控件下方」移回源码次序；`prepare-accessory-system-products.py` 新增 6/9 个 `zrA`/`iTA` 用点、默认类名、首个子的位置、色域覆盖与 `GM` 预设枚举断言，并补回 `initial.uiRestraint`。仍未核对 `.widgetContent{gap:20px}` 的列间距在各组件上的表现。

2026-10-06 配件系统滑条：源 `OTA` 的 `.slider-container` 外观（64/36px 容器、`bottom:25px` 的 6px 轨道与 `#44d62c4d` 底、`calc(8px + p*(100% - 16px))` 填充、16px `#44d62c` 拇指与 hover `#5d5d5d`/active `#383838` 加 2px 绿边、`bottom:42px` 绿底气泡）已抽成共享的 `src/ui/source_slider.rs`，配件行与 1383 OLED 亮度行都改用它；颜色集中在 `theme::SliderColors`，机检 `node tools/audit-source-slider.cjs --check`。风扇转速行的源控件结构尚未核对，`.thumb-tag` 悬停互换未实现。

2026-10-06 3884/3886 无线 ARGB：图标 `:hover`/`:active` 换色与自动检测关键帧已按当前源 CSS 实现。准备工具把源规则命中的那一层拆成 `currentColor` 图层（共 24 个派生资源），颜色取源声明本身（`#44d62c`/`#39a029`/`#96ef89`/`#7de36c`/`#c8323c`/`#d97077`/`#feab59`/`#b15e0c`），并播放 `zoomout`/`zoomoutc`/`zoomoutf`/`zoomin` 的 50/700/700/100ms 时间线；原先本地那套 300ms 透明度淡入是近似，已删除。见 [无线 ARGB 审计](wireless-argb-current-audit.md)。原版 tooltip 时序、3886 小数输入与实窗像素仍未验证。

2026-10-06 164/241 鼠标底座：713 设备的额外手续已按当前源补成 `isDualLinkWarning` → `continuePairing` → `mode:1` 绑定的两步握手（不再使用本地占位提示）；页面与弹层两条警告改为按 `showBothDevicesConnectedWarning` 与「鼠标+键盘都在」门控，配色回到源选择器（12px `#999` / 14px `#17` `#ccc`），配对工具说明改用各产品包自己的语言键（241 源码拼写为 `ENABLE_LAUNCH_PARING_UTILITY_INFO`）。见 [配对页审计](dock-pairing-current-audit.md)。

2026-10-06 1383 Kraken V4 Pro OLED 页已按当前源改成两列五控件形态（亮度 30–100、语言暂存+APPLY、48×27 延迟/变暗按钮与 `#111` 禁用底衬、260×68 屏保磁贴），描述符改由准备工具从证据生成，见 [OLED 审计](audio-oled-1383-current-audit.md)。Home Screen Display 卡片与六个编辑器、语言下载/上传、dongle 警告判定与滑条拇指资源仍缺。

2026-10-05 本轮已接入 Macro 逐行复制/删除与成组拖动、Mouse/Loop 成对编辑、Dashboard 展示状态投影；OLED 子任务被限流未新增修改。完整 Phased、其余 OLED 编辑器、产品专用状态与实际画面验收仍未完成，详见[续接记录](continuation-row-actions-2026-10-05.md)。

2026-10-05 本轮已接入 Macro 全选/顶部操作、键盘配对高亮、Dashboard 电池状态和691 OLED预设编辑卡片。Macro 每行复制/删除、成组拖动与 Phased 阶段编辑已按当前源码接入；录制、执行、设备服务和实际画面验收仍未完成。详见 [Phased 审计](macro-phased-current-audit.md)。

2026-10-05 用户要求把已有页面全部纳入字体、布局、颜色、图标、功能和动画复核。三个专项任务正在按当前实际挂载源码核对；本轮已暴露公共正文 padding、RazerF5 粗体未注册、Nommo 多余选项、相机分列及 Alexa 正文/提示等差异。见[完整覆盖索引](ui-consistency-round-2026-10-05.md)。以下可达性统计不是视觉或功能验收；旧记录中“已有”页面仍须逐项复核。

2026-10-05 Macro Keyboard 已接入当前窗口捕获、成对键事件和持久字段；设备输入重定向、原光标、配对连线/动画、序列和实际窗口验收仍缺。

2026-10-05 Macro Launch 已补文件选择器和源草稿/定位规则，Phased 六层定位已连接阶段编辑器，详见 [Phased 审计](macro-phased-current-audit.md)。Text/emoji 与实际窗口验收仍待完成。

2026-10-05 嵌套菜单续接：编辑框与展开列表已分状态；重选同一宏只收起列表；补 688px 源阈值、当前帧定位、选中项滚入、名称撑宽和菜单/触发器过渡。见 [专项后续](macro-nested-menu-followup-2026-10-05.md)。钢笔编辑光标、滚动条外观及实际窗口验收仍缺；下面同日较早批次的定位/滚入/动画待办以本条为准。

2026-10-05 Macro 后续批次已接入共享文档库、持久 ID、动作草稿、保存提示、快捷键选择与分类播放。嵌套宏、Phased 阶段编辑、原生录制/执行、硬件服务和实际窗口验收中，Phased 界面与本地排序逻辑已接入，录制/设备服务仍是边界。

2026-10-05 链接会话续接：已补快捷键文本 emoji/字符面板、3946 quick macro 捕获会话、3907 独立 Armory 无遥测分支及纵轴、Devices & Modules 源记录投影与正式服务行。3894 DEFAULT 经当前静态产品配置证明不可达，不再列作必须补造的页面；3894 是 Head Cushion Chroma，旧 PWM 名称错误。验证及明确边界见 [本批记录](continuation-followup-2026-10-05.md)。整体目标仍未完成，以下旧轮次与专项新记录冲突时以同日续接记录为准。

2026-10-04 语言包键清零批次：`python tools/audit-locale-keys.py --check` 的待回溯项现在为
**0**。前几轮登记的 5 处全部收口：`SWITCH_OFF_LIGHTING_WHEN_DISPLAY_IS_OFF`/`_WHEN_IDLE`
（第 6 轮 → `DISPLAY_TURNED_OFF`/`IDLE_FOR_MIN`）、`MINIMUM`/`MAXIMUM`（第 7 轮 →
`<SIDE>_TRIGGER_RANGE`/`<SIDE>_ACTUATION_POINT`）、`MINUTES`（第 8 轮 → 源码 `GR` 组件的
`MIN`/`SEC` 模板）、`LINKED_GAMES_TO`（第 8 轮 → `LINKED_GAME_CHROMA_HEADER`，
"Games linked to profile:"）、`ADVANCED_EFFECT_DETAILS`（第 8 轮 → 键名无误，文本由新工具
`tools/prepare-device-locales.py` 从设备包语言表合并进 `locales/`）。审计现在覆盖 382 个字面量
`t()` 键、42 个软键与 42 个动态调用；动态键仍无法静态解析，只能逐个回溯（当前 42 处都已在
别处核对过键名）。语言包之间的键数差（de/fr 93、es 88、ja/kr 97、pt-BR 96、ru 94、zh-CN 61、
zh-TW 118）来自各语言包本身的缺失，不是本地漏提取。

2026-10-04 页面可达性批次：`tools/audit-page-coverage.py` 现在给出「主导航页是否都有渲染器」
的机检答案——**没有渲染器的页面槽位为 0**，剩余一行是 9 个无线键盘的 `TAB_PAIRING`，
它属于 `multiDevicePairing` 独立模式根（本地配对窗口承载）。本轮修掉了让 Raptor 显示器、
Hanbo、PWM 风扇控制器、散热垫、Core X V2 的已实现页面**完全不可达**的描述符守卫顺序问题，
并补齐监视器提示控件与刷新率计数器方块网格、HDR 的 Windows 11 分支。仍未接入的配件细节
（真实 `uiRestraint` 服务填充、显示器产品图、ICC/刷新率、散热曲线硬件值）
见[配件页面审计](accessory-system-native-ui.md)。

2026-10-04 displayMode 分支批次：产品侧四个根分支现在都有本地实现与机检收据，
逐产品的归属表由 `tools/generate-display-mode-roots.cjs` 从审计结果生成
（`src/features/display-mode-roots.json`），不再按单个产品写死。逐模式的规则、
颜色尺寸依据和本地偏差见[本地实现记录](display-mode-roots-implementation.md)。

- `chromaApp`（212 个产品包有根分支）：Chroma 窗口的设备卡在有本地灯光页时直接打开弹层根
  （适配器的灯光标签，或 source 家族的 `TAB_LIGHTING` 页），无灯光页时保留原版
  `.box-item-device .disabled`（`opacity:.3`、不响应指针）。设备卡几何按
  `.box-item-device`（290×245、`padding:10px 20px 20px`、`#111`/2px/圆角 5、
  hover `#44d62c4d`、active `#44d62c`）修正；`.name-tag` 不再居中，行高 16px。
- `armory`（226 个产品包有根分支）：工坊窗口新增设备面板，挂载该产品的映射页
  （适配器 Customize 或 source 家族的 `TAB_CUSTOMIZE`），高度按原版 `ie` 分支
  KEYPAD 460 / HEADSET·AUDIO 340 / 其他 420；入口是设备「分享到工坊」，与分享表单同屏。
- `macro`：产品侧分支仍是宏应用 iframe，本地由宏页承载（见宏审计）。
- `multiDevicePairing`：具名第二窗口（见配对窗口审计）。

仍未接入：音频/配件家族以及没有本地灯光页或映射页的产品（不打开，而不是伪造页面）；
两个根的 postMessage 往返在本地改为同进程直接挂载，消息名仅作契约记录。

2026-10-04 应用选择器去门控：Chroma Studio 已本地实现（Chroma 窗口页面），
选择器改为直接打开；Philips Hue 的模块页就是 769 产品工作区，本地存在该设备时也直接打开，
否则明确说明缺少设备。仍保留门控的只有本地确实没有页面的模块/应用
（`audio-visualizer`、`chroma-connect`、`sensa-hd`、Streamer Companion、Virtual Ring Light）。

2026-10-04 后续进展：Armory 本地分享表单已接到 182/653 配置更多菜单；Macro 新增
设备/配置/真实产品输入区域的会话内绑定；3946 自动化的星光、波浪、音频表参数
与快速宏 `.exe` 文件选择已落地。OLED 裁剪画布/移动/中心缩放与持久化几何已按当前
Cropper 源修正。上述均为本地 UI 进展；Armory 完整详情/上传、宏设备服务、GIF 编码
与原生裁剪输出仍缺，以下旧轮次描述若冲突以各专项当前审计为准。

2026-10-04 恢复会话后的更正：Macro、Armory、Profiles、Alexa、Feedback 的普通入口
已按 4.0.827 宿主实际处理接入同一窗口的具名页签。`policy=3` 不表示第二个系统
窗口；多设备配对按历史明确要求保留第二窗口，并标记为偏离其源调用的本地例外。
见[宿主页签更正](independent-module-window-audit.md)。以下旧进度中的窗口解释以此为准。

本轮同时修复相机步进器与滑块的共同初始化、鼠标释放重复步进、最终 CSS 覆盖和
小数格式，增加真实挂载的曝光补偿数字框；Feedback 补齐隐私类别与日志确认表单，
OLED 补齐缩放控制与后台文件读取并校验当前 worker 资源；后续已补齐裁剪画布几何。
GIF 编码、原生裁剪输出、设备传输及实际像素/交互验收仍缺，不能把资源齐备或编译
通过算作产品完成。

2026-10-04 本次集成：Macro、Armory、Profiles 已接入当前源码登记的具名 gpui 窗口（窗口名分别为 `macro`、`armory`、`profiles`，策略为 `policy=3`，同名再次打开时聚焦），并保留各自的页面外框与明确服务边界。Macro已替换占位正文，接入原始无宏布局、显式新建宏/文件夹、选择搜索排序、重命名复制删除、教程、无设备绑定分支、Help，本地动作草稿的插入/选择/删除/撤销/重做/保存，Delay/Keyboard/Mouse/Loop/Text/Command/Launch 参数编辑，以及编辑器底部源码定义的100px拖放保留区和事件行排序，并接入顶部历史；其真实录制、设备传输、XML、完整文件夹菜单和独立产品绑定模式仍未完成，详见[宏复核](macro-current-source-review.md)。Armory已纠正默认可见导航、补内部历史、介绍横幅、左侧 68142 搜索入口（200px 输入框与 300ms 防抖壳）和 Browse 顶栏多条件筛选、My Downloads 顶栏单排序控件及本地选中态；远端建议/结果、服务筛选/排序、详情弹层和内容服务仍缺。Profiles已接入真实两页签、设备磁贴、设备关联弹层、Profile选择/重命名、关联游戏磁贴和本地已知关联的切换，并为游戏扫描、已安装程序刷新、Profile服务菜单显示未连接提示；安装程序扫描、全局游戏目录与设备子设备数据仍缺，不能把空目录或本地关联当作服务同步结果。相机页保留实时流不可用边界，音频演示保留播放服务不可用边界，见对应审计文档。电量块补齐当前挂载的`role="img"`语义，电量/托盘过渡保持源码时序。后端与DLL修改继续后置。

2026-10-04 更正：旧日志停止原因是轮次上限，目标没有完成。旧表中的Profiles五路由、无法还原文案、缺少全部宏/Profiles图标及设备标签栏重复箭头均不再成立。最新修复和明确未完成项见[逐项复核](source-ui-review-2026-10-04.md)、[Profiles](profiles-app-audit.md)、[导航栏](device-tabs-audit.md)。以下旧轮次明细保留为历史，不是新增验收结论。


2026-10-03 当前状态：331 个注册产品、1419 个主导航页都有入口与部分内容，其中 1392 页记为 partial_native，27 页是原有十个适配器的部分实现。本轮没有把任何产品标记为完整复刻；“完全没有主体”的主导航页计数为零，并不表示条件分支、弹层、独立模式或视觉细节已完成。

## 已确认的缺口

| 产品或范围 | 尚未完成的 UI |
| --- | --- |
| Dashboard 模块目录 | 目录里 7 个已登记模块（alexa / macro / linked-games / feedback / armory / profile-migration / tour）全部带 `native_page`，`src/shell/devices_modules_catalog.rs` 对它们只渲染「打开」并 `emit(ModuleCatalogEvent::OpenModule(page))`（源码注释即“Opening a compiled local page is not an installer receipt”），不存在下载/安装门控；`src/shell/module_preview.rs` 的下载/安装进度只服务没有本地页的源模块，与当前 Dashboard 的 `/installer/` 流程一致。已按静态解析核对（`SourceModuleCatalog.ids` ∩ `MODULES` 全部命中 `native_page`）。 |
| 鼠标、键盘共享页面 | 完整自定义映射、高级动作、替代布局和条件分支；源产品工作区的配置更多菜单、部分右侧图标；`.widget .help` 帮助入口已统一改用共享 `surface::help_control`（Snap Tap 帮助与非对称中止说明等，源声明与本地指纹见 [设备页 CSS 审计](device-page-css-audit.md)）；691 命令拨盘帮助已改用源门户（`<i className="help">` + `bottom-right` 即时挂载富文本），并新增共享 `[tooltip]` 属性徽标 `src/ui/attribute_tip.rs`（源全局 `[tooltip]:before`，已用于拨盘 `icon-add`；有覆盖规则的页面仍用各自 kind），见 [691 审计](keyboard-691-current-audit.md) |
| displayMode 独立窗口 | 四种根级分支的**打开者**都已从当前源码查清：`multiDevicePairing`（30 包）已按原版做成真正的第二个 gpui 窗口（具名单例、存在即聚焦，配对页设备卡为入口，见 [配对窗口审计](multi-pairing-window-current-audit.md)）；`macro`（174 包）不是第二个产品窗口，而是宏应用窗口（`macro`／`/synapse/macro/`／`policy=3,tab_visible=1`）在「绑定到设备」弹层里的 iframe，宏窗口本地已实现外框与两个导航标签（见 [宏应用审计](macro-app-current-audit.md)、[界面审计](macro-app-ui-audit.md)）；`chromaApp`（212 包）在 Dashboard 里出现 0 次，属于独立 Chroma 应用窗口；`armory`（226 包）属于 `armory` 应用窗口，本地已实现窗口外框与四个导航标签（精选推荐／浏览／我的下载／我的上传），并在模块目录里改为直接打开，见 [Armory 依据](armory-app-current-audit.md)；资料分享服务与内容未接入。模块目录七个盒子现在有六个直接打开（Alexa、宏、已关联的游戏→profiles 窗口、Armory、配置文件迁移、介绍导览），只有 `feedback` 仍是门控：它的窗口名 `feedback-synapse` 与 URL 都取自模块表，但该应用源码不在当前提取范围内，无法复刻界面。表与打开参数见 [模块注册表依据](module-registry-audit.md)。具名窗口清单、标志与模块盒去向见 [窗口契约](display-window-contract.md)。窗口图标（`app_icon_path`）在 gpui 里没有对应字段 |
| 740 / 746 磁轴键盘 | 完整页与配套校准弹层已接入；载入条（`KeyboardSwitchCalibrationModal_loading` 2s `ease-in-out` 循环、`0%{left:-25%}to{left:100%}`）与光标（`_caret` 1.1s `steps(1)` 的 0/50/100 亮、25/75 灭）动画、错误后 15 秒关闭、出厂配置禁用分支与介绍关闭持久化都已按源接入（见 [校准审计](keyboard-calibration-current-audit.md) 与 `src/features/keyboard_calibration.rs`）；仍缺的是真实设备传输/事件时序与真实按键捕获 |
| 691 BlackWidow V4 Pro 75% | OLED 预设导入/裁剪、主页 Emote/Banner/Media/System/Keyboard 分支已接入本地草稿；语言选择/下载、GIF 帧处理、设备传输进度/错误和完整 hover 动画仍缺；OLED 主页卡片的两个提示已按源接入（BLE 禁用走 `[turn-off-ble-tooltip]:before` 的 `SourceTooltipKind::OledBleDisabled`（含编辑控件），requires-Synapse 图标走即时挂载的 `.tooltip-razer.bottom-right`，见 [BLE 提示审计](oled-ble-tooltip-current-audit.md) 与 [卡片审计](oled-preset-cards-current-audit.md)） |
| 3592 / 3594 / 3595 / 3596 Kiyo | 已接入方向图、图像四行/变焦及实际挂载的曝光补偿步进器；最终 CSS、长按/释放、键盘/文字提交、小数及动态最小值按当前调用链纠正。锐度和增益在这四个根关闭，不再列作缺失行。实时流、设备枚举、第三方分支和实际交互验收仍缺，见 camera-framing-current-audit.md。 |
| 3587 / 3589 / 3590 Kiyo | 共享 Customize 布局已补 520×292 相机预览外框和明确不可用边界；实时流、设备枚举、取景与叠加层仍未实现 |
| 1392 / 1442 / 3942 audio demo products | Source poster, dimensions, floating preference, and the source-sized control bar are restored. The progress/volume sliders retain local preview values, while native audio playback, live timing, floating video and complex mappings remain open; clicking reports the service boundary. See audio-demo-playback-boundary.md. |
| 164 / 241 Mouse Dock | 页面/弹层警告条件与配色、713 两步配对握手与各产品自己的说明键已按当前源接入；设备名链接已按源接入（源 241 `914.*` 的 `Es(peer, devices)` 命中应用设备列表时渲染 `hyperpolling-span-hover` + `<u>` 链接并 `z(e)` 切换设备：本地 `DockPairing::known_devices` + `DeviceLinkRequested` → `WorkspaceEvent::OpenDevice` → `AppShell::sync_known_devices`/`navigate(Location::Device)`；CSS `cursor:pointer;text-decoration:underline` 与 hover `#44d62c` 已对齐，见 [dock 校验](dock-pairing-current-evidence.json)）；「已连接」文案分支需要本地还没有的 `pairedInfo[].connected`、部分状态动画、弹层 viewport/滚动等价与键盘焦点实窗验证仍缺 |
| 1383 Kraken V4 Pro | OLED 页五个 `.widget` 已按当前源形态接入（含三张已备屏保预览）；Home Screen Display 卡片网格与动画/图片/表情/横幅/音频表/系统信息六个编辑器、设备语言下载与上传、dongle 警告判定、`path.0086a00e.svg` 拇指资源仍缺 |
| 778 ASRock B550 / 3871 Chroma ARGB | 页面主体、自动检测 50/100/700ms 动画与 LED 步进器 300ms 长按重复已接入；3871 的检测/刷新图标提示已按源改为即时挂载（`#icon-detection-wrapper`/`#icon-refreshing-wrapper` + 共享 `crate::ui::hover_tip` 的 `.tooltip-razer.bottom-left`，778 源码本身没有这两个图标，见 [审计](wired-argb-current-audit.md)；LED 数量提示已改为共享的 `.tooltip-razer.bottom-right`（`hovered_info` 即时挂载 + 源里只有数字为绿的分段上色）；帮助入口改用共享 `surface::help_control`，并核实图标资源与共享图标字节相同）；真实 ARGB 端口服务、检测结果和运行时视觉验收仍缺 |
| 3884 / 3886 无线 ARGB | 自动检测图标的 50/100/700ms 点击动画、300ms hover motion 已接入；检测/刷新图标的提示已按源接入（`#icon-detection-wrapper`/`#icon-refreshing-wrapper` 上 `on_hover` 即时挂载、`.tooltip-razer` 皮肤、100ms 淡入、`.absolute().top_full().right_0().mt(5px)` 的右缘对齐 + 下方 5px；原因查明：`StatefulInteractiveElement` 只对带 id 的元素成立，见 [审计](wireless-argb-current-audit.md)；端口 LED 数量提示也已按源改为 `.tooltip-razer.bottom-right`（含源里只有数字为绿的 `ledCount` 分段上色）；端口帮助入口改为共享 `surface::help_control`（源的 `.widget .help` + `.tip`，14px/绝对定位已还原），见同审计）；源 path 颜色插值、3886 原代码端口分支不可达仍待逐像素/服务验收，不能把样例编辑器算成原版普通页（见 [动画依据](wireless-argb-motion-evidence.md)） |
| 784 Aether Light Strip | 轮播平滑居中、Identify 延迟、部分 tooltip 触发/定位和底部提示定位 |
| 769 Philips Hue | 发现、连接、设备状态的完整交互和精确视觉验证；两个亮度滑条已改用共享 `SourceSlider`（源 `OT` 的 64px 容器、`.slider-tip` 数值、`.track`/`.left`、滑柄 hover/active 与 `.foot` 两端标签，见 [Hue 审计](hue-current-audit.md)）；高级灯效教程点已按源用 `isShowTutorialHue` 做跨启动持久化（点击写入 `false`，`is_loading` 变化时重算，见 [Hue 审计](hue-current-audit.md)）；配对中的进度动画已按源补上（`PAIRING` 的 80px 绿条 2s 线性循环 `left:-80px→100%`；`SCANNING` 本身只有文字与取消按钮，没有进度条，见 [Hue 审计](hue-current-audit.md)）；扫描的 13 秒超时也已按源接入（两个「开始扫描」按钮启动、离开扫描中清掉、手动 IP 搜索不启动，见 [Hue 审计](hue-current-audit.md)）；步进器/提示框等控件细节仍缺 |
| 灯效 widget 帮助 | 已按源接入有条件帮助（`.widget .help`，蓝牙 `EFFECTS_BLE_TOOLTIP`／常规 `EFFECTS_TOOLTIP`，源 `cs` 的 `tips={t?uTU:i?px6:Zsw}` 经模块作用域解析确认）；Sensa HD 的 `SENSA_HD_TOOLTIP` 分支缺本地设备标志，未建模，见 [设备页 CSS 审计](device-page-css-audit.md)。 |
| 效果参数文案 | 效果方向/屏幕按钮的 4 个字面量标签已换成唯一命中的源文案键（`CLOCKWISE`/`COUNTER_CLOCKWISE`/`TOP`/`BOTTOM`）；其余 7 个标签（整个屏幕/左侧/右侧/向左/向右/向外/向内）与 Chroma 同步块的字面量在本地文案表里没有唯一命中的键，需回到各自源标记再定，见 [设备页 CSS 审计](device-page-css-audit.md)。 |
| 3946 自动化 / Base Station V3 | 实际挂载的 Static/Breathing/Starlight/Wave/Audio Meter 参数已接入；Fire/Spectrum 无参数面板。完整宏录制器、游戏浏览与关联、快捷键子编辑器、删除确认锚定、原始图标槽位及过渡动画仍缺 |
| 原有十个适配器 | 182、653、777、3072、3073、3074、3076、3077、3078、3080 的 27 个主页面已逐页复核（`partial_native_reaudited`，带本地路由与源码依据），见 [逐页复核](legacy-adapter-page-reaudit.md)；仍未进入 `source_help` 描述符、182 规格条目 `pages` 为空、653/777 仍走各自手写页面，仍是部分实现 |
| 设备页顶栏电量 | 已按当前源码接入（状态机 `off`/`Charging`/`charging100`/`NoCharge_BatteryFull`/`batt-warning`/`ReachChargingLimit`，图标与文案逐条取证，见 [电量依据](battery-indicator-audit.md)）；`hideBattValue` 已按产品工作区里写死的 prop 接入（15 个 `!0` 产品只显示图标、不渲染百分比也不挂载提示，并加 `.hideBattValue{margin-right:17px}`；见 [电量依据](battery-indicator-audit.md) 与 `tools/audit-battery-indicator.cjs` 的产品表比对）；耳机左右耳电量仍因缺设备数据未实现 |
| 3880 Raptor 27 165Hz | 已接入 NSA 两列 Color 页面、HDR、THX Cinema、色域与 PIP/显示器控件；`uiRestraint` 的源字段门控已接入本地渲染，真实宿主/设备服务填充、系统 ICC 枚举和产品图仍缺 |
| 所有产品 | 仍无实际窗口的字体度量、缩放、焦点、滚动和动画逐像素验收结论；不能以检查通过认定视觉完整 |

## 不属于单个设备的界面

- 完整托盘账户、Widgets / Notifications、多应用内容及动态高度。
- 账户登录、宿主独立 Settings 窗口、固件/重置等完整服务界面。
- 模块目录、顶部状态、更多应用和部分设置样例的完整外层复用；具体范围见 [服务预览复核](service-preview-current-audit.md)。
- 其余未接入的独立应用与产品模式；已有本地入口会直接打开，不能以一个下载状态卡替代已实现的页面。

真实配对、校准、相机流、音频处理、自动化执行、设备遥测、账户和安装服务也仍有独立缺口。它们与 UI 还原分开记录，不用示例状态冒充设备事实。

本清单是已知缺口，不是声称未列出的细节已经完成。逐产品、逐页面路由见 [原生覆盖表](native-product-coverage.md) / [机器可读清单](native-product-coverage.json)，总体状态见 [当前 UI 完成状态](21-ui-completion-status.md)。

## 第 13–30 轮新增或修正（均带审计脚本与依据文档）

| 内容 | 依据文档 | 机检脚本 |
| --- | --- | --- |
| 设备页顶栏电量补到 182 型产品的 `DeviceWorkspace::toolbar`（原先只加在源工作区） | [battery-indicator-audit.md](battery-indicator-audit.md) | `audit-battery-indicator.cjs` |
| 托盘：弹窗 360×200、左键不再切换、位置 `x = 托盘x − w/2`、`y = 托盘y − h` 并夹在 `rcWork` | [host-tray-audit.md](host-tray-audit.md) | `audit-host-tray.cjs` |
| 设备页共享控件：`.widget` 600×`padding:30px 40px`、标题 `#44d62c/RazerF5/16px/mb 20px`、`.h1-body`、`.check-item`、`.widget .content`、轮询率面板与警告块 | [device-page-css-audit.md](device-page-css-audit.md) | `audit-device-page-css.cjs` |
| 动作分类栏图标与悬停底色 `#393939` | [action-category-icons-audit.md](action-category-icons-audit.md) | `audit-action-category-icons.cjs` |
| 模块目录七个盒子六个直接打开（含 maco/armory/profiles） | [module-registry-audit.md](module-registry-audit.md) | `audit-module-registry.cjs` |
| 关联游戏磁贴：`.list-box` + 每游戏一张 `.linked-game-tile` + 固定 `.add-new`（40px `icon_add` 资产），接进关联游戏对话框 | [linked-game-tile-audit.md](linked-game-tile-audit.md) | `audit-linked-game-tile.cjs` |
| profiles 窗口：应用自己的五条路由视图 + 返回/前进历史 + `.razer-profiles` 视图容器 | [profiles-app-audit.md](profiles-app-audit.md) | `audit-profiles-app.cjs` |
| 设备页标签栏：悬停文字 `#ccc`、按下底色 `#3cbf27`、大写、`.nav.back/.nav.forward` 箭头与页历史、溢出 `.dots3` 菜单（`.profile-act` 面板 + `.act` 行） | [device-tabs-audit.md](device-tabs-audit.md) | `audit-device-tabs.cjs` |
| `.hover-border` 的 `transition:border-color .2s`（profile 栏「更多」与标签溢出共用） | 同上 | 同上 |
| 对话框按钮 `.thx-btn`（绿/灰两态、大写、`1px #0000004d`、悬停 `opacity .8`） | [thx-button-audit.md](thx-button-audit.md) | `audit-thx-button.cjs` |
| 导入/导出底栏：`.import-profile-btn-group`、导出模式的 `willNotImport` 文案、关联游戏弹层打开时导航行 `opacity .5`（`div.nav-tabs.disabled`） | [import-export-footer-audit.md](import-export-footer-audit.md) | `audit-import-export.cjs` |
| 宏窗口顶栏：46px / `#222` / 2px `#000` / 居中（`.nav-wrapper` + `.module-nav`） | [macro-app-chrome-audit.md](macro-app-chrome-audit.md) | `audit-macro-app-chrome.cjs` |

## 语言包键审计（2026-10-04）

`python tools/audit-locale-keys.py --check` 扫描 `src/` 中全部字面量 `t("KEY")` 调用并核对
`locales/` 的 10 份语言包：**409 个硬键、42 个 `t_or` 软键、42 个动态调用，缺失 0 个**
（`KNOWN_MISSING` 现为空表）。`t()` 未命中时 rust-i18n 会原样返回 key（见 `src/i18n.rs`
的 `has`），因此缺失数是这条链路的直接指标。历史缺口与其收口方式：

| 键 | 收口方式 |
| --- | --- |
| `ADVANCED_EFFECT_DETAILS` | 键名无误（设备包里就是 `oi("ADVANCED_EFFECT_DETAILS")`）；文本由 `tools/prepare-device-locales.py` 从设备包语言表合并进 10 份语言包 |
| `MINUTES` | 源码无此键；手柄省电取值改用共享组件 `GR` 的 `MIN`/`SEC` 模板（`>= 60` 才用 `MIN` 并除以 60） |
| `LINKED_GAMES_TO` | 源码无此键；改用 `LINKED_GAME_CHROMA_HEADER`＝"Games linked to profile:" |
| `MINIMUM` / `MAXIMUM` | 源码无此键；手柄扳机页改用 `<SIDE>_TRIGGER_RANGE`/`<SIDE>_ACTUATION_POINT` |
| `SWITCH_OFF_LIGHTING_WHEN_DISPLAY_IS_OFF` / `_WHEN_IDLE` | 改用 `DISPLAY_TURNED_OFF` / `IDLE_FOR_MIN` |

别名解析记录见 [设备页 CSS 逐条对账](device-page-css-audit.md)。

另有一类**不经过语言包的界面文案**，上面的审计看不见：产品页三族（鼠标、键盘、手柄）
已经在 2026-10-04 全部改为源码语言键（鼠标 40 余处、键盘 13 处），未知输入/效果原样显示
内部名而不编中文，并有测试防止回退；逐条键名依据见
[产品页文案的语言键对账](product-label-locale-audit.md)。仍带标签类中文字面量的模块（按数量降序）：
`device_pages.rs` 62（旧版设备页渲染器，仅 `system_button` 被复用，是否仍挂载待确认）、
`mapping_editor.rs` 45、`profile_transfer.rs` 45、`shortcuts.rs` 44、`onboard_memory.rs` 20、
`audio_page.rs` 17、`profile.rs` 17、`keyboard_controls.rs` 16、`linked_games.rs` 15、
`customize_page.rs` 14、`settings.rs` 12、`keyboard_calibration.rs` 10，以及若干「界面预览」
工具模块（`dock_pairing/preview.rs`、`hue/preview.rs` 等，不进应用 UI）。`surface::note(...)`、
Tooltip 与 `accessibility_label` 的中文是本项目自己的「服务未接入」提示，不是雷云文案。

另有 42 个 `t_or` 软键中 5 个不在语言包内（`AETHER_SERVICE_UNAVAILABLE`、`REFRESH_RATE`、
`SCARLETT_CONFIRM`、`SCARLETT_CONFIRMAION_TEXT`、`SCARLETT_CONFIRMATION_DESC`）：前两个走
自带兜底文案，后三个是原版自带的 `SCARLETT_*` 描述名（真实键是 `CONFIRM` 等，已按基础键取值）。
各语言包比英文少 61–118 个键，属既有提取差距，审计只做提示、不判失败。

## 资源准备更正（2026-10-04）

宏manifest声明的229个SVG和Profiles的工具栏/弹层SVG已按官方静态地址准备，资源清单共1029项并通过来源、格式、嵌入键校验，未执行下载JavaScript。宏palette的11个图标不再缺失。应用专用语言包正在按真正的加载链复核，不能继续以公共语言包里找不到同名键判断无译文。资源到位不等于UI已经实现。

2026-10-04 calibration follow-up: the introduction banner now persists the audited `showNotificationBannerCalibration` key, factory-default profile warning/disabled branch follows `isFactoryDefaultProfile`, and the explicit failure preview mirrors the source's 15-second idle close. The real calibration transport remains unavailable and its live Next action stays disabled.

2026-10-04 Macro follow-up: action parameter editors now cover Delay, Keyboard, Mouse, Loop, Text, Command, and Launch as local draft controls; event rows support source-shaped drag reorder and the 100px trailing drop area with undo/redo integration. Real recording, device transfer, XML import/export, complete folder menus, and independent binding remain service-boundary gaps. See [macro-current-source-review.md](macro-current-source-review.md).
