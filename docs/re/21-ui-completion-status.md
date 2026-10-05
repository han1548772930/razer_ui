# 当前 UI 完成状态

2026-10-05 本轮继续补齐当前源可直接证明的细节：3858/3880 显示器接入 `uiRestraint` truthy 禁用原因的控件门控，服务字段不写入本地 profile；3884/3886 无线 ARGB 接入自动检测 50/100/700ms 动画、300ms hover 过渡与 3886 待机覆盖层。相关专项静态校验、格式化和 `cargo check --locked --all-targets` 通过；真实设备服务与运行时像素验收仍缺，整体复刻未完成。

2026-10-05 ?????Macro ?????????? Mouse/Loop ??????????Dashboard ?????????????????????[????](continuation-row-actions-2026-10-05.md)??????????????????????????

2026-10-05 最新关联会话续接已修正 Macro 选择与工具栏、179 提示/配对外层、Dashboard 电池状态/mask，以及691 OLED预设卡片；补查14个服务产品的运行时图标模式。统一 `cargo check --locked --all-targets`、格式与相关静态检查通过。具体覆盖、证据和未完成项见[本轮续接](continuation-review-2026-10-05.md)。**整体仍未完成，未运行应用或测试，未作像素一致性结论。**

2026-10-05 已按用户要求开启已有页面的全面一致性复核，三个子任务分别检查公共壳层、产品正文和独立应用。本批已修正179接收器、实际字体面/字重、公共正文间距、Nommo与相机内容、Alexa和Armory正文及运行时页签图标；统一`cargo check --locked --all-targets`与专项静态校验通过。覆盖范围与未核项见[本轮索引](ui-consistency-round-2026-10-05.md)。**已有页面或编译通过均不代表一致，整体仍未完成。**

2026-10-05 Macro Keyboard 已替换自由文本输入，接入当前窗口原始键码捕获、150ms 提交、按下/松开配对更新与本地持久字段，并补原方向图标。见 [键盘专项记录](macro-keyboard-current-audit.md)。设备重定向、配对连线/拖拽细节、原生录制与执行服务仍缺；Phased 编辑器界面和本地排序已接入，整体复刻未完成。

2026-10-05 Macro Launch 已接入 null 初始模式、文件选择器、分离草稿、源按钮状态和 100px／−300px 定位规则，见 [Launch 审计](macro-launch-current-audit.md)。Text/emoji 已按 `58190.dn/on/en` 接入分类、搜索、变体、UTF-16 限制与源 clear-search/CSS 缺陷，见 [文本审计](macro-text-current-audit.md)。两者均未做实际窗口验收。

2026-10-05 嵌套宏菜单续接已补源三态切换、688px 定位阈值、选中项滚动、名称撑宽和展开时序，详情见 [后续记录](macro-nested-menu-followup-2026-10-05.md)。该范围仍无实际窗口验收，整体复刻仍未完成。

2026-10-05 Macro 后续批次已接入共享持久文档库、独立动作草稿、未保存确认、快捷键 Macro 映射及分类型播放集合，并补 3946 类型菜单和重复次数 Stepper；继续接入嵌套宏选择、持久引用及活动图的循环候选检查。交付、验证与仍缺功能见 [Macro 集成记录](macro-library-integration-2026-10-05.md)。**整体复刻仍未完成**，以下记录按各自批次范围理解。

2026-10-05 链接会话续接已补 Devices & Modules 正式服务行、快捷键文本/emoji 与时序、3946 quick macro 捕获会话、3907 独立 Armory 无遥测分支及曲线轴，并修复资源嵌入注册。当前 `cargo check --locked --all-targets`、格式化和专项静态校验通过；**整体复刻仍未完成**。准确交付、验证限制、下载故障和下一步见 [本批交接](continuation-followup-2026-10-05.md)，本页下面的旧批次记录保留为历史。

2026-10-04 产品页文案语言键批次：鼠标、键盘两族产品页不再写中文字面量，面板标题、开关、
滑条行、效果名与输入名全部换成源码自己的语言键（鼠标 40 余处、键盘 13 处），做法是
「用语言包中文值反查键名」+「设备包导出表两跳解析别名」+「语言包审计做闸门」；未知输入/效果
改为原样显示内部名而不是编中文，并有测试固定。语言包审计现在覆盖 409 个字面量键、0 缺失，
新增的 27 个键都在 10 份语言包中存在。逐条键名依据与仍未接入的文件清单见
[产品页文案的语言键对账](product-label-locale-audit.md)。

2026-10-04 语言包收口与手柄功耗/灯光批次：语言包键审计的**待回溯项清零**（5 → 3 → 0）。
`ADVANCED_EFFECT_DETAILS` 的键名本来就是对的（设备包里就是 `oi("ADVANCED_EFFECT_DETAILS")`），
缺的是应用级提取工具看不到设备包表格，新增 `tools/prepare-device-locales.py` 用 JS 感知扫描
从设备包取出 10 种语言文本并合并（`--check` 现已通过）。手柄功耗页按共享组件 `BR`/`GR` 改正：
标签不再是自造的 `"{value} MINUTES"`，而是源码的 `MIN`/`SEC` 模板（`value >= 60` 才用
`MIN` 且数值除以 60，即按秒判断），新增 `i18n::t_value` 做 `{{value}}` 替换与纯函数
`power_saving_label`（含测试）；标题/帮助/说明改用 `POWER_SAVING_HEADER`、
`POWER_SAVING_TOOLTIP`、控制器自己的 `CONTROLLER_POWER_SAVING_DESC`。手柄灯光页改回源码的
「关闭灯光」组件（两个勾选项 + 1–15 滑条与 `1`/`15` 灰标）与带标题开关的亮度组件。
关联游戏弹层标题改用真实存在的 `LINKED_GAME_CHROMA_HEADER`（"Games linked to profile:"），
原先的 `LINKED_GAMES_TO` 在所有当前源码里都不存在。细节与逐条 CSS 对账见
[设备页 CSS 逐条对账](device-page-css-audit.md)。

2026-10-04 手柄扳机页批次：按 `JP`/`QP` 重做扳机页的模拟/数字分支——面板标题改为源码的
`LEFT_TRIGGER_MODE`/`RIGHT_TRIGGER_MODE`，两个分支各自的 `.h1-body` 文案用
`LEFT|RIGHT_TRIGGER_RANGE`（模拟）与 `LEFT|RIGHT_ACTUATION_POINT`（数字），删掉本地自造的
`MINIMUM`/`MAXIMUM`；模拟分支改用源码的双柄滑条（两条 6px 绿条：背景 `#44d62c` + `opacity .3`、
高亮按 `left:start%/right:(100-end)%`，20px 圆柄悬停 `#5d5d5d`/按下 `#383838` 且都带 `2px #44d62c`
描边，`.sliderTipBar` 的绿色数值气泡用零宽容器 + 居中复刻 `translateX(-50%)`，下方 `0`/`100`
两端对齐），越界规则照源码 `Math.min(value,max-1)`/`Math.max(value,min+1)` 写成纯函数并有 3 条测试；
数字分支滑条按源码加 `1%`/`100%` 灰标。`.reset-actuation` 改为下划线重置链接（未偏离默认时
`opacity .3` 且不可点），其 16px 图标 `icon_reset.f416d0b7.svg` 不在已抓取资源里，因此只渲染文字。
语言包键审计的待回溯项从 5 降到 3。细节见 [设备页 CSS 逐条对账](device-page-css-audit.md)。

2026-10-04 勾选项与「关闭灯光」组件批次：`.check-item`/`.check-box` 从「外包一层 mb(9px)」
升级为按当前源码实现的真实勾选框（20×20、圆角 2.4、`#737373`/悬停与选中 `#44d62c`、
`:before/:after` 两段 3px 勾线按 `ticktop .2s`/`tickbottom .1s` 插值、`.check-text` 为
`#ccc` 14px/17px 且 `left:30px;top:2px`、首字母大写），并连同 `.widget .help`、`.foot` 灰标
一起收进 `ui/surface.rs`（`check_item`/`help_control`/`slider_tags`）。同时按源码更正键盘与
鼠标的「关闭灯光」组件：它是**独立组件**（`kI`/`xI`，标题 `SWITCH_OFF_LIGHTING_HEADER`、
提示 `SWITCH_OFF_LIGHTING_TOOLTIP`），内含 `DISPLAY_TURNED_OFF`、`IDLE_FOR_MIN` 两个勾选项与
1–15 的滑条（灰标 `1`/`15`、无标签、30px 缩进 490px 宽），两个勾选项在亮度关闭时禁用；
原先自造的 `SWITCH_OFF_LIGHTING_WHEN_DISPLAY_IS_OFF`/`_WHEN_IDLE`/`MINUTES` 三个键
（源码里不存在，`t()` 未命中会直接显示 key）已删除，语言包键审计的待回溯项从 7 降到 5。
细节与别名解析见 [设备页 CSS 逐条对账](device-page-css-audit.md)。

2026-10-04 色彩页批次：显示器色彩页按源码重做——3858 的色彩温度组件补齐
`COLOR_PROFILE_TOOLTIP` 提示、预设改为 `.btn_group`/`.btn_custom`（`GM` 枚举顺序与
`NORMAL`/`LOW_BLUE_LIGHT`/`WARM`/`COOL`/`SRGB`/`SCARLETT_CUSTOM` 键），红/绿/蓝三行改为
`STA` 行（无提示、无灰标、带 0–100 数值输入框）并包在 `.slide-off`/`.slide-on` 块里；
3880 改为源码的两列网格（`.widget-col` 各 600px，左列 THX Cinema + Color Profile，
右列 HDR + Color Temperature）。新增 `surface::panel_with_title_switch` 复刻
`.titleRow > .title` 里的 `.widget-switch` 位置（THX/HDR 的开关在标题文本之后，帮助按钮仍在
右上角），HDR 提示按 Windows 版本选键，Color Profile 组件按源码渲染禁用下拉与外链
（删除了此前自造的“等待系统提供显示器色彩配置文件。”文案，无数据时走原版自己的空数据分支；
外链点击拉起 `colorcpl.exe`）。同时新增静态审计
[audit-locale-keys.py](<../../tools/audit-locale-keys.py>)：扫描 `src/` 里全部字面量
`t("KEY")` 调用，核对 10 份语言包——383 个硬键、42 个 `t_or` 软键、42 个动态调用，
发现 7 个字面量键不在语言包内（已逐个记录为待回溯），并给出各语言的键数差
（61–118 个，属既有提取差距）。细节见
[配件页面审计](accessory-system-native-ui.md)。

2026-10-04 游戏模式页批次：显示器 Game Mode（`RTA`）此前是「outline 按钮 + 文字标签 + 滑条」
的混合形态，现按源码重建——`.btn_group`/`.btn_custom` 预设按钮（DEFAULT/FPS/MMO/RACING/
STREAMING/CUSTOM，标签键 `SCARLETT_*`）、四条 `STA` 行（`.slider_header.mb10` 名称 + 内联
`.help`/`.tip` + 仅 `maxStep===100` 的 50×27 数值输入框）、overdrive 与 gamma 改为带
`.foot` 灰标的离散滑条（标签位置 0/33%/66.5%/100%，overdrive 用 OFF/WEAK/STRONG 语言键，
gamma 1.4/1.8/2.2 且 3880 加 2.4 提升标）、色域控件 `xrA`（标题 + 选项 `.btn_group` +
**仅在非 Native 时**显示的 `COLOR_GAMUT_WARNING` 与 14px 圆形感叹号图标）。顺带把缺失的
`tooltip_exclamationmark.cc8fb226.svg` 纳入资源（取自 Macro/Profiles 中字节相同的副本，
资源清单 1085 条哈希校验通过）。细节、键名核对记录与仍未接入的轨道视觉见
[配件页面审计](accessory-system-native-ui.md)。

2026-10-04 输入源确认浮层批次：按 `SSA`/`OSA` 重做显示器主输入源控件——`.btn_group.inputSource`
（`gap:20px`）里的 `.btn_custom` 按钮（透明底、`1px solid #5d5d5d`、Roboto 12px 大写、
`padding:7px 16px 6px`、`margin:0`、`.active{#222 + #44d62c}`），点非当前源后按
`confirmInputSourceChange` 弹确认浮层：`.profile-del`（`left:40px;top:165px`、`min-width:300px`、
`#111`、`1px solid #fd4949`、`box-shadow:0 6px 10px 0 #0003`、20px 内边距、`opacity .3s linear`）
+ `CONFIRMATION_DESC` 文本 + 「不再询问」复选 + `#fd4949` 的 `CONFIRM` 键；点浮层外取消、
确认后应用、切换页面丢弃未确认请求，并用 `deferred(...).with_priority(6)` 复刻
`.widgetZIndex{z-index:6}` 的层级。四个输入源图标（`icon_hdmi`/`icon_displayport`/
`icon_usb_typec`/Auto）在已抓取源码中不存在（3858 只抓了 `css/`、`js/`），按"不得猜测"只渲染
文字并在文档中登记缺失文件，未画占位图形。详见
[配件页面审计](accessory-system-native-ui.md)。

2026-10-04 PIP 组件批次：按当前 3858/3880 源码重做显示器 PIP 面板（此前是三组自造按钮）。
新增 `qAA`/`ZAA` 屏幕模型（330×196、主区 186、底部 10px 条、130×30 底座）、`JAA` 的 12 个
放置预设（3 尺寸 × 4 角，SMALL 90×49.5 / MEDIUM 120×66 / LARGE 151×83，z 3/2/1，虚线边框、
悬停 `#ffffff1a`+`#44d62c`、进入选择器后边框转 `#ccc`、选中框 `#222` 且隐藏输入源文字）、
右侧栏 `MODE` 标题 + 两个 144×80 模式按钮（共享 `.btn` 度量与微型屏幕图形）+ `SOURCE` 标题 +
输入源下拉（`$AA`：DP_1 15 / HDMI_1 17 / USB_C 19）。标题键按源码更正为 `MODE`/`SOURCE`，
并记录了下一轮要做的输入源确认浮层（`OSA`：`.alert.profile-del`、`CONFIRMATION_DESC`、
`CONFIRMAION_TEXT`、点外部取消）的完整证据。细节与"按源码保留的 640px 溢出"见
[配件页面审计](accessory-system-native-ui.md)。

2026-10-04 页面可达性批次：新增 `tools/audit-page-coverage.py`，把「产品自己声明的页面
键」与「本地渲染器实际分派」做静态对账，并区分四种情况：家族分派、补充控件描述符、
独立模式根（`StandaloneMode`）、确实没有渲染器。首轮结果暴露了两个真问题：

- **已实现的配件页面全部不可达**：`accessory_system_products.rs` 把
  `_ if !supports_page(...)` 描述符守卫放在实现分支之前，而 Raptor 显示器三页、
  Hanbo/PWM/散热垫的 Performance、Core X V2 的 Customize 都没有生成描述符表，
  于是被守卫挡在一个乱码占位字符串（`"?????????????"`）后面。现已改为实现分支优先，
  并删除该乱码（`shell.rs` 深链失败提示里的同类乱码也换成明确中文）。
- **9 个无线键盘的 `TAB_PAIRING` 不是主导航页**：它们属于 `multiDevicePairing`
  独立导航（注册表里 `role: StandaloneMode`），本地由配对模式根承载，审计不再把它算作缺口。

对账后：本审计覆盖的 5 个家族数据文件共 **1010 个页面槽位 / 279 个产品，没有任何一页缺少
渲染器**（`slots with no renderer at all: 0`），也没有只渲染占位说明的页面。（注册表口径的
331 产品 / 1419 主导航页来自 `tools/audit-native-product-coverage.py`，与本审计的分母不同，
两者不可混用。）同时补齐了三处控件细节：监视器 `.widget .help` 提示控件
（14px、`#4a4a4a` → 悬停 `#ffffff4d`、`.3s`）与 `SourceTooltip` 新增的 `WidgetTip`
分支（`max-width:300px`、14px/18px、`right:14px;top:34px` 锚点）、HDR 提示按
Windows 11 分支（新增只读 `is_windows_11()`）、刷新率计数器位置的 2×2 `#` 方块网格
（27px 行、`min-width:90px`、`1px dashed #ccc`、选中 `#44d62c`/黑字）。依据与仍未接入项
（PIP 方块选择器、`uiRestraint` 设备约束、显示器产品图）见
[配件页面审计](accessory-system-native-ui.md)。

机检：`python tools/audit-page-coverage.py`（含 `--self-test` 的四个合成用例：实现分支在前、
守卫在前、占位分支、完全缺失分支；以及针对仓库源码的守卫顺序检查）。
本批没有运行应用、构建、测试、安装器或下载代码；`cargo check --locked --all-targets`
与格式化、资源/内嵌 JSON 校验、各专项 `--check` 审计全部通过。

2026-10-04 displayMode 根分支批次：本轮先修复了被上一批提交破坏的编译
（`assets/synapse/embedded.rs` 数组在闭合括号后多了一行、`hyperpolling` 图标未进
资源清单；`product_workspace.rs` 的事件转发把 `&Device` 当 `Device`）。随后按当前源码
补齐两个尚未接入的产品侧独立根：

- 新增 `tools/generate-display-mode-roots.cjs` → `src/features/display-mode-roots.json`
  （armory 226、chromaApp 212、macro 174、multiDevicePairing 30，产品 id 直接来自
  `docs/re/display-mode-audit.json`），Rust 侧统一由
  [display_mode_roots.rs](../../src/features/display_mode_roots.rs) 读取；窗口契约的
  `DisplayMode::key()` 也改为引用同一张表，避免手写模式名。
- `chromaApp` 根：不再只对 653 生效，改为「有根分支 + 有本地灯光页」才打开
  （[chroma_product.rs](../../src/features/chroma_product.rs)、
  [product_workspace.rs](../../src/features/product_workspace.rs)）；设备卡按
  `.box-item-device` 修正内边距、名称行与不可用态（`opacity:.3`、`pointer-events:none`）。
- `armory` 根：新增 [device_root.rs](../../src/shell/armory_page/device_root.rs)，在工坊
  窗口按原版 `ie` 分支挂 460/340/420px 的设备面板并挂载该产品映射页；入口是设备
  「分享到工坊」（与分享表单同屏），消息契约（`armoryIframeReady`、
  `armoryMappings-<productId>`、`armory-button-list`、`armory-change-viewIndex`、
  `hypershiftMode`）记录不改用 postMessage。
- 选择器去门控：Chroma Studio 与（本地存在 769 设备时的）Philips Hue 直接打开本地页面，
  不再显示下载/安装门控；其余模块保持门控是因为本地确实没有对应页面。

机检：`node tools/audit-display-mode-roots.cjs --check`（收据
[display-mode-roots-audit.json](display-mode-roots-audit.json)）与
`node tools/generate-display-mode-roots.cjs --check`；逐模式的源码依据与本地偏差见
[本地实现记录](display-mode-roots-implementation.md)。仍未接入：音频/配件家族与没有本地
灯光页/映射页的产品，逐模式写在上面的收据里。本批没有运行应用、构建、测试或下载代码。

2026-10-04 后续接线与参数补齐：182/653 的配置更多菜单现可把当前本地配置带到
Armory 分享草稿；标题/描述、最多十个游戏文件、配置选择与摘要均有当前源码证据，
在线检测和上传仍未接入。Macro 的绑定页新增设备卡、配置选择及产品真实输入区域，
绑定暂存于宏页会话，不代表硬件映射。Base Station V3 自动化补星光颜色/随机/时长、
波浪上下方向、音频表增益，以及快速宏 Program 的本地 `.exe` 选择器。
OLED 改为源码 232×190 画布及固定 232×64 裁剪框；编辑和恢复共用同一几何，
本地图片使用 GPUI 解码与资源缓存。GIF 编码、原生裁剪输出、远端服务、所有产品的
其余分支及运行时视觉验收仍未完成。参见
[Armory 分享](armory-share-current-audit.md)、[自动化](automation-current-audit.md)、
[OLED 当前审计](keyboard-oled-current-audit.md)。下方“本次恢复/最终验证”属于上一批
已完成检查，不能自动作为这些后续编辑的验证结论。

本批后续最终验证：`cargo check --locked --all-targets` 无警告通过，
`cargo fmt --all -- --check` 与 `git diff --check` 通过。资源校验覆盖 1056 项
源/输出哈希、132 个 Webpack 请求、73 个产品变体、58 个 Dashboard 变体，
以及 16 个布局/1901 个形状。内嵌 JSON 核对 24 份文档，0 失败、3 个动态
`Value` 目标跳过结构检查；182 宏输入目录另有绑定 AST 审计。自动化参数/程序
选择、宏绑定、Armory 分享、OLED canvas/worker/crop-language 静态检查通过。
OLED 自审另修 GIF/媒体动画元素 ID、data URL 解码缓存释放及本地资源预算；
这些预算不是厂商文件限制，详情见专项审计。本批没有运行应用、构建、测试或
下载代码。宏绑定仅保留会话状态；完整产品根、远端服务和编码输出仍在清单内。

2026-10-04 本次恢复：模块入口已经由误设的第二系统窗口改为当前宿主 `policy=3`
具名页签，覆盖 Macro、Armory、Profiles、Alexa 与 Feedback。重复打开复用页面与
草稿；配对第二窗口按历史明确要求保留，作为本地例外单列。见[窗口语义更正](independent-module-window-audit.md)。
相机数字输入修复初始化、重复步进、小数格式与最终样式，并补曝光补偿；Feedback
修复隐私/邮箱分支及日志确认点击穿透；OLED 补缩放控件、后台读取与 worker 资源
完整校验。仍未完成全产品 UI、GIF 编码/完整裁剪、真实设备服务或运行时视觉验收。
下文旧轮次的独立窗口与控件缺口结论若冲突，以本段及各专项当前审计为准。

本次最终验证：`cargo check --locked --all-targets` 无警告通过；`cargo fmt --all -- --check`
及 `git diff --check` 通过。资源校验覆盖 1050 项源/输出哈希、132 个 Webpack 请求、
73 个产品变体、58 个 Dashboard 变体、16 个布局/1901 个输入形状。内嵌 JSON 检查
核对 22 份文档、0 失败；另有两个 `serde_json::Value` 动态目标跳过结构检查。
模块注册表、宿主策略、窗口契约、displayMode、导航栏、Feedback、OLED worker 的
静态审计通过。新增页签复用/关闭/重开与旧 Alexa 顺序兼容的回归源码仅编译，未运行。
OLED 后台导入增加请求代次保护，较早文件读取不能覆盖较新的选择。没有运行应用、
build、测试、安装器、下载的 JavaScript/WASM 或 DLL；全产品完成状态仍为 partial。

2026-10-04 集成进度：用户报告的hover重复注册、电量父层布局及导航历史接线已作源码修正；Macro、Profiles、Armory 已接入各自具名 gpui 窗口（`policy=3` 同名聚焦）。本轮又补上 3587/3589/3590 Kiyo 的源码尺寸预览外框、691 OLED 的本地导入/裁剪与主页模式分支、鼠标键盘共享页的 MapSwitchKeymap/CombineMouse 条件映射，以及音频演示的播放器控制条外观；相机流、OLED 传输、硬件映射、音频服务仍保持明确边界，详见[剩余工作](remaining-ui-work.md)与各应用复核文档。所有产品仍为部分实现，未新增任何“完整复刻”产品；格式化、静态解析、资源校验和cargo check不代表实际点击或像素验收。后续旧轮次记录保留为历史。

更新日期：2026-10-04。**目标仍未完成**。本轮恢复旧日志并独立逐项复核，确认旧会话因round-limit停止；331产品/1419主导航页的partial状态不变，不能算完整产品。当前源码要求和禁止运行的约束保持不变。Macro 动作行的本地选择/删除/撤销/重做、Armory Browse/My Downloads 顶栏控件、Profiles 服务未连接提示已继续补齐，仍不代表服务或设备接通。

本轮修复hover重复注册、电量父容器/tooltip/显示条件、错误主导航文案、重复导航箭头与历史衔接，并重写Profiles为真实两个页签；随后补齐Profiles设备/关联弹层的本地交互、Macro编辑器尾部保留区和电量语义资源。当前资源、产品数据与嵌入 JSON 校验通过，`cargo check --locked --all-targets` 与格式检查通过。详见[逐项源码复核](source-ui-review-2026-10-04.md)、[Profiles更正](profiles-app-audit.md)、[导航修复](device-tabs-audit.md)。UI和全部产品接入优先；DLL仅静态读取接口、后端最后统一处理。

**以下是截至2026-10-03的历史记录，不能作为本轮已复核证明。**其中“Profiles五路由”“缺中文无法解析”“宏/Profiles媒体目录不存在”“标签栏必须渲染第二套前进后退”已被当前源码证据推翻，以本轮文档为准。

---


更新日期：2026-10-03。目标是完整复刻当前稳定版 Synapse UI；**尚未完成**。版本依据为 [当前源记录](20-current-source-version.md)，逐产品的实际路由与状态以自动生成的 [原生覆盖表](native-product-coverage.md) 为准。

331 个注册产品包含大量共享页面和连接别名，不能算作 331 套已完整实现的界面。`partial_native` 只表示已有实际内容；入口、描述符、素材下载和编译通过都不能证明视觉或交互等价。较早的“10 个路由、321 个未适配”等统计保留历史意义，不是当前覆盖率。

本批补齐两款磁轴键盘的校准初始页、弹窗和独立状态预览，并恢复三个音频演示页的原版初始画面。细节与限制见 [校准页审计](keyboard-calibration-current-audit.md) 和 [音频演示审计](audio-demo-current-audit.md)。

随后接入 Philips Hue 的连接引导、网桥、亮度、设备列表和五种快速灯效，并加入高级灯效状态及 18 种隔离预览。Hue 的服务和视觉限制见 [当前 Hue 审计](hue-current-audit.md)。

本轮并行接入此前八个完全缺少主体的页面：164/241 鼠标底座、778/3871 有线 ARGB、3884/3886 无线 ARGB、784 Aether 灯带、3946 Base Station V3 自动化。当前 1419 个主导航页中，1392 页有部分原生内容，27 页仍列作原有适配器待重新复核；没有产品被认定为完整复刻。3886 原代码端口分支不可达，显式样例不计作正式端口编辑已完成。

按用户指出的无线接收器下拉框问题，追踪了根 props、消费者和页切换状态：179 原版不挂载配置栏；其他附件页必须区分下拉禁用与仅同步图标禁用。见 [配置栏复核](product-profile-bars-current-audit.md)。端口和灯带布局现按源数据作用域存为设备设置，3946 自动化随配置文件保存。

本轮接入 3592／3594／3595／3596 四个当前 Kiyo 根 CAMERA 页缺失的取景块：变焦、平移／倾斜面板、五个取景预设和预设快捷键，控件与几何算式逐项取自原组件，标签与快捷键输入 ID 来自产品自身本地化及按键表；页面改为原版的 400px `.camera-container` 列。3596 的变焦范围（1–1.4 / 0.01）与 3592／3594 的 maxPanTilt 差异按各自挂载 props 生成，未统一套用。随后按同样的来源规则补上分辨率行（3594／3595／3596，10 项能力回退列表）与水印放置盘（仅 3592 挂载，六个位置，随开关禁用）；3592 不挂载预览块，因此没有分辨率选择器。压缩名跨模块复用，水印位置经导出表加字面量折叠解析，分辨率列表按挂载组件实际引用的绑定定位。实时画面与设备枚举、第三方分支、LDC／分辨率联动的禁用分支、白框拖动和方向键图标仍未完成，见 [取景审计](camera-framing-current-audit.md)。

本轮清点了 `displayMode` 多根机制的真实范围，并提取窗口打开契约。原版不是用路由切换这些根：每个产品包用 `?displayMode=` 选择另一套根组件，Dashboard 通过具名窗口打开它。静态扫描 331 个包（只读注册审计记录的主导航源）得到根级分支：`armory` 226、`chromaApp` 212、`macro` 174、`multiDevicePairing` 30 个产品包；注册审计只把其中 31 条 `multiDevicePairing` 与 1 条 `chromaApp` 记成非主导航，因此覆盖表里的「32」只是已登记的那一片。窗口契约给出 13 个标志（`policy=3` 复用同名窗口、`policy=5` 独立窗口、可见性、`shouldFocus=1`、按模式区分的 `app_icon_path`）、三条窗口名规则（`multi-device-pairing-<containerId>`、`multi-device-pairing-p<pid>-<serial>`、`multi-device-pairing`）与配对窗口的参数构造（`containerId`、`displayMode`、`allMasters`）；打开函数先查同名窗口是否存在，存在则复用并调整可见性。多设备配对、Macro、Armory、Profiles 的本地根现已由第二个具名 gpui 窗口承载，服务内容仍按缺口记录。见 [分支审计](display-mode-audit.md)、[独立模块窗口审计](independent-module-window-audit.md) 与 [窗口契约](display-window-contract.md)。

本轮接着补摄像头的取景交互：按原版把「按下白框记录抓取偏移 → 在黑框 `onMouseMove` 里移动白框 → 依次套用四条边界钳制 → 用 `Tm`/`Im` 把像素回算成 pan/tilt」整条链移植进来（`drag_pan_tilt`），白框像素位置改为每次由 `white_origin` 从 pan/tilt 重算，与原版 `useEffect(H)` 的行为一致；黑框的盒模型也纠正为原版的 content-box：内容 220×132 加 1px 边框 = 边框盒 222×134，之前把 220×132 当成了边框盒。上一轮补完的复合禁用条件（`ldc && (4K 30FPS | 1440p 30FPS)`）与左右方向键图标同在本文件对应的[取景审计](camera-framing-current-audit.md)里。

本轮补上相机取景块缺的**共享数字步进器**：原版变焦行是共享设置行 `HM.A`（`hasStepper:!0, allowDecimal:!0, roundUpDecimals:!0, stepValue/minStepper/maxStepper`，`disabledStepper` 接同一道 LDC 复合条件），本地新增 [stepper.rs](../../src/ui/stepper.rs)，外框 60×27 + `1px #5d5d5d`、输入区 58×25 `#111/#ccc/14px`、上下箭头 14×12（`background-size:8px`、上 5px 下 3px 偏移）、hover/focus 才淡入、按下立即一步并每 300ms 重复、禁用 `opacity:.3` 且不响应指针，箭头图标用产品包里同名同哈希的两份 SVG（已按原样打包）。描述符新增 `has_stepper`/`allow_decimal`/`round_up_decimals`，由生成器从该行原文提取，3592/3594/3595/3596 四个产品的变焦行已接入；亮度/对比度/饱和度/锐度/增益/白平衡六行在原版同样带步进器，本地仍只显示数值，记为缺口。同时修正上一轮的一处记录错误：模块目录的 `MACRO` 盒当时**并没有**真正改成直接打开，本轮已改为 `Some(ModulePage::Macro)`，现在四行（Alexa、宏、配置文件迁移、介绍导览）确实直接打开本地页面。

步进器随后扩到原版同样带步进器的图像四行（亮度、对比度、饱和度、白平衡）：四个产品各有 5 行接入（含变焦），步长与范围逐行取证（1／1／1／10／0.1，白平衡 `disabledStepper` 即自动白平衡开启时禁用）。原始锐度与增益两行在原版本身是有条件的 `a&&(…)`／`t&&(…)` 分支，本地相机页尚未挂载这两行，因此记为「条件行未接入」而不是「步进器缺失」。

覆盖审计里最后一块「未复核」也清掉了：十个 `existing_partial_native_adapter` 产品的 **27 个主页面**逐页复核完毕，状态从 `existing_partial_not_reaudited_here` 改为 `partial_native_reaudited`，每页都带本地路由、可达性（`src/nav.rs` 的 `Tab::for_product` 分支）与该产品存在的当前源码数据；**没有**把页面内部逐字段来源、视觉一致性或硬件行为写进依据。剩下的缺口写在[逐页复核报告](legacy-adapter-page-reaudit.md)里（十个产品仍未进入 `source_help` 描述符、182 的规格条目 `pages` 仍为空、653/777 仍靠各自手写页面）。

本轮把宏窗口从「安装门控」变成可打开的本地页面：新增 `Location::Macro` 与 `HostTab::Macro`（标签 id 就用原版窗口名 `macro`，图标复用已打包的 `synapse/module-macro.svg`），模块目录里 `MACRO` 盒现在直接打开它；宏服务与宏数据仍未连接，页面按原版外框（`MacroContainer_my_macro__jCmj8` 只有 `position:static`，`setup_svgs` 是 `display:none` 的图标预载容器）与两个导航标签（`TEXT_NAV_TAB_MY_MACROS` 我的宏、`TEXT_NAV_TAB_KEY_BINDS` 按键绑定）呈现，并显式写明宏服务未连接、不把未知安装状态写成已安装。功能面板（palette）的 11 个条目已从模块 81021 逐条提取（类型、图标文件、文案 key），但宏应用在当前源码包里没有 `static/media`，这 11 个图标全部缺失，因此面板尚未绘制、也没有拿别的图标顶替；部分文案 key 在两个 `trans-zh-CN` 分块里都不存在，同样不臆造。收据见[宏应用界面审计](macro-app-ui-audit.md)。

模块目录也按这份证据对齐了行为：每一行现在带原版的盒名、它聚焦的窗口名与地址（Alexa→`alexa`、宏→`macro`、已关联的游戏→`profiles`、反馈→`feedback-synapse`、工坊→`armory`、配置文件迁移→`syn3-profile-migration`、介绍导览→`synapse-introduction`）。这些具名窗口在本地均有对应页面，目录和 App Picker 都直接打开本地实现；只有没有本地页面的外部模块才保留安装状态边界。原版点击模块盒本来就是 `focusTab(windowName)`，未安装时才先打开 `/installer/#type=module&id=<id>&location=<path>`。

本轮把「宏窗口的打开点」查清了，结论修正了之前的实现计划。宏在原件里是一个**独立应用**（`.ref/applications/synapse/macro/`，72 个分块 7.5 MB，与 `alexa`、`armory`、`profiles`、`settings`、`update-fw`、`introduction-tour` 并列），窗口名就叫 `macro`，地址 `/synapse/macro/`，打开标志 `policy=3,tab_visible=1`；Dashboard 的模块表在同一处登记它（模块 54420），常量表在模块 69937（`O="macro"`），模块盒名在模块 54693，点击盒子走 `focusTab(windowName)`，未安装时先开 `/installer/#type=module&id=macro&location=synapse/macro` 再 `autoOpen`；映射界面里也有 `openMacro`（命中已开窗口则激活）。而 **`displayMode=macro` 不是第二个产品窗口**：它是宏应用「绑定到设备」弹层里的 **iframe**，`src` 带 `displayMode=macro&macro=<id>&containerId=…&deviceEditionInfo=…&serialNumber=…`，标题 `MouseBind`。同样地，`chromaApp` 在当前 Dashboard 里出现 0 次——它属于独立 Chroma 应用窗口。Macro、Armory、Profiles 的本地窗口接入与服务未连接边界见[独立模块窗口审计](independent-module-window-audit.md)；收据与清单见[宏应用审计](macro-app-current-audit.md)、[窗口打开契约](display-window-contract.md)（新增 `named_windows`：11 个具名窗口、2 个隐藏、5 个模块窗口、8 条模块盒去向）与[displayMode 审计](display-mode-audit.md)（新增「各模式由谁打开」）。

本轮修复了一处启动崩溃并补上它对应的回归检查。崩溃发生在 [wired_argb.rs](../../src/features/wired_argb.rs)：内嵌的 `wired_argb_data.json` 里 778 的条目缺少必需字段 `name`，serde 在第一次 `spec()` 时 panic（`missing field 'name'`）。原因是 778 的设备配置本身没有 `deviceName`（它的名字来自运行时型号表：0 = B550、128 = X570、129 = Z690 Taichi Razer Edition），生成器把 `undefined` 交给 `JSON.stringify` 后被静默丢掉，而 Rust 结构仍要求该字段；`--check` 因此一直是「最新」的。处理：生成器显式省略该字段并对任何 `undefined` 字段直接报错；Rust 侧该字段改为 `#[serde(default)]`（它不参与渲染）；新增 [validate-embedded-json.py](../../tools/validate-embedded-json.py)，静态解析每个 `include_str!("*.json")` 的目标类型（支持 `Option`/`#[serde(default)]`/`rename`/`rename_all`/嵌套 `Vec<T>`），核对 22 份内嵌数据是否满足其 Rust 结构——用旧结构跑同一份数据即可复现 `missing field 'name'`，证明这条检查能拦住这类运行时 panic。

同时补完摄像头的复合禁用条件：原版取景组件里 `ldc && (4K 30FPS | 1440p 30FPS)` 一个条件同时禁用变焦步进器、变焦滑块、平移/倾斜面板和预设块（含快捷键），并在禁用时渲染 LDC 说明。本地把它编码为描述符的 `disabled_when_any`（任一条件组全部成立），分辨率取值与 `/camera/ldc` 都由生成器回到源码核对；3592 默认就是 `ldc = true` + 4K 30FPS，因此一打开即为原版禁用态。左右方向键改用已在本地打包的同一份图标（`icon_arrow_left_thin`／`icon_arrow_right_thin`），上下与中心键的图标在当前源码包中不存在，仍保留边框。见[取景审计](camera-framing-current-audit.md)。

按用户「开真正的第二个 gpui 窗口」的要求，本轮落地窗口层和四个已审计根。窗口层在 [display_window.rs](../../src/shell/display_window.rs)：窗口名规则逐字对应 Dashboard 模块 84058 的 `xc()`（容器 → 产品+序列号 → 纯模式名，有单元测试）、`policy=3/5/7` 标志、具名登记表与「同名窗口存在即聚焦」；[pairing_window.rs](../../src/shell/pairing_window.rs) 是配对根视图，[independent_window.rs](../../src/shell/independent_window.rs) 承载 Macro、Armory、Profiles 三个具名应用根。审计发现产品包里的 `displayMode=multiDevicePairing` 根只是一个 iframe 宿主：它把 `/synapse/multipairing/` 连同 11 个参数（`displayMode`、`containerId`、`productId`/`pid`、`category`、`canPairTwoDevices`、`isProductivity`、`deviceName`、`serialNumber`、`lang`、`allMasters`）打开，并 postMessage `multiDevicePairingInit`；本仓库的 4130 页面正是被嵌入的那一页，因此窗口直接承载它，`allMasters` 走页面原有的外部记录通道。入口按 Dashboard 设备盒 `box box-multi-paring` 的行为接在配对页设备卡上，guard 与原版一致（`productId` 与 `deviceContainerId` 同时存在）。`deviceInfo`/`deviceName` 没有本地对应字段、`allMasters` 的原版来源（宿主写入的 `connectedDeviceInfo` 投影）尚未审计，两项都记为缺口而不是补默认值。收据见 [配对窗口审计](multi-pairing-window-current-audit.md)、[独立模块窗口审计](independent-module-window-audit.md)，窗口名与标志见 [窗口契约](display-window-contract.md)。

已清点服务预览各入口，新增完整产品标签页入口；Dock 样例先显示主体再打开原配对层；Aether 样例复用完整产品工作区；Alexa 正式页已隔离开发场景选择器。仍只有局部组件或独立样例的入口，明确记录在 [服务预览整体复核](service-preview-current-audit.md)，不冒充整页完成。

后续工作仍包括：页面内完整条件分支与独立模式；映射高级动作及硬件提交；相机实时流与取景交互；有声演示原生播放器；附件高级编辑与配对、Hue；自动化宏/游戏/快捷键子编辑器；配置更多菜单和精确动画；OLED 语言/传输及 GIF 处理；托盘的完整账户/通知内容；宿主独立窗口。真实设备、账户、安装和固件服务也没有因 UI 入口存在而接通。

必须继续区分界面状态与设备事实。校准成功、固件完成、安装状态、风扇转速、电池或温度不能通过计时器或本地草稿伪造。独立开发预览可展示明确标记的状态示例。

已执行允许的全目标 `cargo check`、格式检查及静态资源/来源验证；1029 项嵌入资源的来源、输出格式及哈希校验通过。没有启动应用、构建、测试、安装器、下载的 JavaScript 或 DLL，尚无真实窗口的像素、焦点、滚动和设备往返验收结论。

2026-10-04 校准补全：740/746 页面现已接入 `showNotificationBannerCalibration` 持久化、`isFactoryDefaultProfile` 警告与禁用分支，以及失败预览的 15 秒空闲关闭时序；真实校准传输仍保持禁用。

## 2026-10-04 UI follow-up

Macro action rows now support source-shaped drag reordering, including the 100px trailing drop area, page-scoped drag payloads, green insertion borders, and undo/redo state updates. The local event editor covers Delay, Keyboard, Mouse, Loop, Text, Command, and Launch values. Armory static receipts now include content card, lazy list, detail popup, and upload/share modules (27588, 86024, 13476, 66517) plus their CSS selectors; service payloads remain unavailable. These additions are static UI evidence and do not imply recording, device transfer, account, or remote content services are connected.

2026-10-04 receiver follow-up:
- Product 182 battery snapshot corrected from stale 47% to 100%; old workspaces normalize only the confirmed full-state 47% record.
- Product 179 Dashboard PluginImages and current `prd-1x` preview are embedded.
- Product 179 host Tab uses `.ref/applications/synapse/dashboard/shared-favicon/ACCESSORY.svg`, matching the current ACCESSORY source category.

2026-10-05 Phased Macro编辑器已按当前 58190.Ka/Fr 和 25572.G 接入三个阶段、phase-aware 拖放、稳定阶段排序和 Launch 定位，证据见 [Phased 审计](macro-phased-current-audit.md)。原生录制服务、设备传输和实际窗口验收仍未完成。
