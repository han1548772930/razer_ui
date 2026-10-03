# Aether Light Strip 当前源码与原生页面审计

2026-10-03。仅静态解析当前产品 `784`；没有运行应用、构建、测试、下载的 JavaScript 或 DLL。

## 当前证据

- `.ref/devices/784/static/js/main.3094caa4.js`，SHA-256 `3751063a619bd163a7ac9d8bf238a5452154ecf20fbaf04fcaaee4275b65a1f2`。
- 当前 CSS `main.03a1d509.css`、asset-manifest、JSX SVG、十种语言、外层 `Sg/Gl/ol` 及导航证据由 `tools/extract-aether-strip.cjs` 重新解析；记录在 `aether-strip-current-evidence.json`。此工具用 Acorn 读取 AST，未 evaluate/require 下载代码。
- `prepare-aether-strip.py` 准备 23 个有出处的资源：四种灯带布局、四个分段标识、识别/刷新图标、IoT 设备卡按钮、edition 0 原产品图、原问题图标及刷新 SVG。媒体文件来自当前 manifest；HTTP 与内容 hash 保存在原资源旁。
- 主实现为 `src/features/aether_strip.rs` 和同名目录。没有使用已删除的历史 Dashboard/ASAR/host 目录。

## 原生已接入部分

`CUSTOMIZED` 使用四种源码 SVG 布局选项、每段 LED 数量输入、总配置/检测数量、识别和刷新。数量输入保留 2 位手输限制、步进 1、提交时 min 1、max 当前分段值加剩余 LED；切换形状按源码 `$s` 依次 ceil 均分。分段识别 payload 使用零起点及包含末端的 `colStart/colEnd`、`pollTime:3`、`sleepInterval:500`。

页面外层复用了 `Sg` 所挂载的 IoT 卡片：当前设备图片/名称、选择其他卡片、编号指示器、重命名、识别、电源、移除和接管入口。移除与接管弹层使用源码文案、边框和按钮颜色；原 `sl.defaultProps.backdrop=false`，因此不制造暗色遮罩。接管列表按锁定设备生成复选项。关闭/Escape 回到原焦点；切换页面/恢复设置调用 `dismiss`。

布局与输入遵守当前 CSS 的 600 宽 widget、40/30 内边距、RazerF5 16 标题、Roboto 14 主体，形状图标 40、padding 9/15、gap 10、radius 3；输入 62×26、方向标识 24×25、识别/刷新图标 27。颜色取源码对应的 `AetherStripColors` 及已匹配的全局主题语义角色。形状选择和识别图标的 300ms ease-in-out、输入边框的 300ms ease、帮助按钮的 300ms ease 均单独实现。

刷新环按 `spinner.ef2d0235.svg` 的 SMIL 重建：26×26、viewBox 100、r 30、stroke 10，2 秒周期、线性 keyTimes 0/.5/1，角度 0/180/720、弧长占比 .1/.5/.1、底环 alpha .3、前景 square cap。减弱动态效果时显示固定帧，不启动虚假刷新。布局帮助和分段提示使用 Base Tooltip/Positioner；帮助按 Fd 的即时挂载行为显示，分段提示采用源码 300ms linear opacity。

## 外层 ProfileBar：不能把禁用当成隐藏

在 main JS 的 UTF-16 offset `4598709`，`lg=[We._$r,We.Cir]` 解析为 `[HELP,CUSTOMIZED]`；`Rg=[We.Cir]` 解析为 `[CUSTOMIZED]`。根 `renderProfileBar` 在 `4603559` 始终传入 `qS`，同时传 `hasTHXWarning:false`、`hasBattery:false`、`hasOBM:false`、`hasGamerRoom:true`。`isEnableProfileBar` 仅决定同步图标的 unsupported 状态，不能决定是否挂载下拉框。

`ZS.renderProfileBarIcon` offset `4270075` 根据 `!isEnableProfileBar` 切换 `.loader.disable`；`renderProfileBar` offset `4274096` 始终创建 profile 区；`enableSwitchProfile` offset `4273874` 由 reducer 的同名值控制禁用。根 `componentDidMount` offset `4602220`、`componentDidUpdate` offset `4602859` 按 `!Rg.includes(active_view)` 设置它。

| 页面 | ProfileBar | 下拉框 | 同步图标 |
| --- | --- | --- | --- |
| CUSTOMIZED | 保留 | 禁用 | unsupported |
| TAB_LIGHTING | 保留 | 可用 | 正常/服务同步态 |
| HELP | 保留 | 可用 | unsupported |

`pI.render` offset `4161165` 的 profile-wrapper、navs-wrapper、right 都保留。CSS 为 min-height 48、背景 #222、底边 2 #000；profile-wrapper flex `1 0 25%`、right `1 1 25%`、navs-wrapper `1 0 max-content`；profilebar 26 高，loader 26×26、左 margin 10，rename-rect 250 宽。同步图标来自 `profile-default.f608d82c.svg` / `profile-unsupported.671bbbc6.svg`；Gamer Room 图标 `icon_gamer_room.730988fc.svg` 为 25×25；Help 使用 `icon_help.377359c3.svg` 的 default/hover/active 符号。

这些公共导航修正在父任务的 `source_workspace.rs` 集成，模块不复制一套 header。状态预览直接创建独立的 `SourceProductWorkspace` 并通过 `aether_preview_page()` 使用其中同一个 Aether 实体，保留正式 CUSTOMIZED/LIGHTING/HELP 标签和工具栏。

## 状态与服务边界

源码初始值是 `detectedLedCount:0,bendData:[],isRefreshing:false`；没有默认 60 LED 设备。真实页面保持未知服务状态，不从 profile 或本地设备描述捏造在线、锁定、开灯、Synapse Override 和 LED 检测结果。

可编辑条件按源组合：locked / offline / power-off / no-Synapse-Override 均禁用布局；locked 或 no-override 显示 Gamer Room 控制提示；locked/offline 另有状态提示。刷新中只对应显式观察状态。操作仅记录开发预览中的源 action 和 payload，不将请求自动变成成功，不移除设备、不解除锁定、不假装灯带实际闪烁或更新。

`snapshot()` 只包含 `bendData`。源码布局从选定 IoT 设备的 `argb.numberOfLeds.strip` / `MW_AETHER_SET_BEND_DATA_FROM_LOCALSTORAGE` 恢复，并非 profileReducer；父任务已将 `_accessory` 存在 `Device.source_device_settings`，避免 Lighting 切换 profile 改变设备布局。`restore(Option<&Value>)` 不接受检测数量和权限；只接收 1–4 段正整数并检查加法溢出与已知检测总数，分段 id 重新规范为源顺序。

隔离开发预览含 13 个场景：1/2/3/4 边、未分配 LED、检测 0、刷新中、离线、其他应用锁定、关灯、Gamer Room 控制、多设备、服务不可用。60/45 LED、名称与状态在预览文件中明确声明为示例。主产品状态不因预览操作改变。

## 校验及尚未证明的部分

`node tools/extract-aether-strip.cjs --check` 和 `python tools/validate-aether-strip.py` 校验当前 source hash、原片段、10 locale、23 资源、header 条件以及状态持久化边界。rustfmt 已执行；`cargo check --locked --all-targets` 由父任务统一完成并报告最终结果。

本次没有窗口截图、像素测量、真实键盘/鼠标操作或硬件联调，不能把静态检查写成视觉验收。真实 IoT 列表更新、发现、设备命名/移除/接管/电源/识别和写 LED 命令的 transport 仍未接入。预览中的跨卡片选择是本地样例切换，未模拟远端确认。

仍需在允许运行 UI 后检查或进一步修正的已知细节：轮播原版 `scroll-behavior:smooth` 的居中滚动与电源打开后 500ms Identify 使能延迟；IoT 卡/编号 tooltip 的原触发时序；离线提示在滚动 viewport 底部 50 的定位；帮助 tooltip 的溢出边界目前采用窗口 viewport，源 Fd 使用 `.main-container > #body-wrapper`。这些不计为逐像素完成项。Lighting 内容和 HELP 内容继续由既有正式工作区负责，本模块没有声称重新审计它们全部内部控件。
