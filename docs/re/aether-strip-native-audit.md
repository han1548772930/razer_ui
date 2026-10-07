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

## 2026-10-06 设备轮播的盒模型与居中（`.carousel--item` / `.device`）

当前源（784 `main.3094caa4.js` + `main.03a1d509.css`，表达式同样出现在 780–784、790、791
七个 IOT 包）：

- 容器：`<div class="widget-prod dot-bg carousel--widget"><div class="carousel" ref><div class="carousel--outer">
  <div class={"carousel--inner " + (1===devices.length ? "center" : "")} ref>…卡片…</div>
  <div class="carousel--indicator">…</div>…</div><div class="dim-corner"/></div>`；
  `.carousel--inner{align-items:flex-end;display:flex;margin:0 auto;overflow:hidden;padding:0 496px;
  scroll-behavior:smooth}`、`.carousel--inner.center{overflow:visible;padding:initial;width:fit-content}`。
- 居中算法：`const H=()=>{r.current&&(r.current.scrollLeft=s.current.scrollWidth/5*O)}`，`O` 是选中下标，
  `useEffect(()=>{H()},[O])`（切换与挂载时都执行）；`496 = 2×248`，`/5 ≈ 一屏五张卡片`，
  所以每换一项滚动约一张卡片宽度，选中卡片落在中间。
- 卡片盒模型：`.carousel--item{align-items:center;display:flex;flex:none;flex-direction:column;
  height:212px;justify-content:center;padding:0 30px;position:relative;width:248px}`；
  `.carousel--item .device{opacity:.5}`、`.device.active{opacity:1;padding-top:28px}`、
  `.device:not(.active){margin-top:75px}`；`.device--img{width:154.8px;padding-bottom:12px}`、
  `.device.active .device--img{width:237px;transform:translateY(3px)}`、
  `.device.linked .device--img{width:204px}`、`.device.active .device--name{font-size:14px}`。

本轮本地改动（`src/features/aether_strip/device.rs`）：卡片拆成
`.carousel--item`（248×212、`flex:none`、`justify_center`、`padding:0 30px`）与其内部的
`.device`（未选中 `opacity:.5` + `margin-top:75px`，选中 `padding-top:28px`）两层，图片宽度仍是
源里的 237 / 154.8，动作按钮的绝对定位仍相对卡片（源里 `.device--badge` 也是相对
`.carousel--item{position:relative}`）。此前本地是单层卡片、`justify_end`、`padding:0 5px`。

机检：`tools/validate-aether-strip.py` 新增 `.carousel--item`/`.device*` 的六条 CSS 声明、
七个包里的居中表达式与本地两层结构标记。

2026-10-06 补上容器与居中（同一轮续做）：`device.rs` 的多设备分支现在渲染
`padding:0 496px` + `overflow_scroll()` + `scrollbar_width(px(0.))`（源码是 `overflow:hidden`，
没有滚动条）+ `track_scroll(&self.carousel_scroll)`，并在构建时按源码 `H()` 设置
`self.carousel_scroll.set_offset(point(bounds.width / 5. * index, 0))`（`AetherStrip` 新增
`carousel_scroll: ScrollHandle` 字段，挂载/切换都会重算，`<0.5px` 内不重复设置，避免渲染循环）；
单设备分支保持 `.carousel--inner.center` 的形态（居中、无内边距、不滚动），不再使用会画出滚动条的
`scrollable_both()`。`scroll-behavior:smooth` 没有在源码里声明时长（由 UA 决定），因此本地按审计到的
同一偏移量直接定位，**不自造缓动或时长**；这一点写在 `tools/validate-aether-strip.py` 的注释里。

## 2026-10-06 编号指示器与卡片徽标的 tooltip 契约

当前源（784 `main.03a1d509.css` + `main.3094caa4.js`）：

- 容器：`<div class="carousel--indicator">{indicator}</div>`，其中
  `.indicator--container{align-items:center;display:flex;justify-content:center;margin-top:10px;
  position:relative;z-index:1}`、`.indicator--list{background:#111;border:1px solid #ccc;
  border-radius:40px;display:flex;margin-right:10px;padding:5px}`、
  `.indicator--item{border-radius:30px;color:#ccc;display:flex;flex:none;font-size:14px;height:26px;
  justify-content:center;margin-right:10px;position:relative;width:26px}`（`:last-child` 去右边距），
  选中/悬停 `#44d62c`+`#111`，`busy`/`offline` 文字 `#fd8611`，选中且 busy/offline 或悬停时底色
  `#fd8611`+`#111`。编号项来自 `for(t…) push(<div class={"indicator--item"+offline/busy/active}
  onClick={()=>setIndex(t)} tooltip={device.title}>{t+1}</div>)`。
- 指示器 tooltip：全局 `[tooltip]:before{background-color:#000;border:1px solid #5d5d5d;color:#ccc;
  content:attr(tooltip);font-size:14px;line-height:16px;opacity:0;padding:8px 10px;pointer-events:none;
  position:absolute;right:0;text-align:left;top:calc(100% + 5px);transition:visibility 0s,opacity .3s
  linear;visibility:hidden;white-space:nowrap;width:auto;z-index:100}` 之上，编号项再覆盖成
  `.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px);width:fit-content}`（左对齐、
  下方 10px、宽度自适应且不换行）。
- 卡片徽标（`.device--badge`，`position:absolute` 相对 `.carousel--item`）在轮播里另有覆盖：
  `.device--badge[tooltip]:before{display:none;left:50%;width:fit-content;z-index:3}`
  ——**默认不显示**；只有选中卡片上 `.anchor--left`/`.anchor--right`/`.anchor--middle` 各自
  `:hover` 时才 `display:block`，其定位分别是 `left:50%`、`right:0`（沿用全局）、
  `left:50% + translateX(-50%) + width:200px + white-space:break-spaces`。JSX 里
  `device--cta-enable anchor--middle`（接管）、`device--cta-power … anchor--left`（电源）、
  `device--cta-find anchor--left`（Identify）、`device--cta-del anchor--right`（移除）与
  `device--badge-offline anchor--right` 都带 `tooltip`。
- 多设备时指示器右侧还有连接按钮：`{supportsLinked && <div class="button--cta button--cta-link"
  role="button" onClick={()=>setLinked(true)} tooltip={…a0H}/>}`，CSS
  `.indicator--container .button--cta{background-position:50%;background-repeat:no-repeat;
  background-size:18px 18px;height:18px;position:relative;width:18px}` + `:before{left:0;
  max-width:300px;white-space:pre-line!important;width:max-content}`，图标按状态在
  `link-btn.fed18bb7.svg`/`link-hovered-btn.0fde7f57.svg`/`unlink-btn.db633bad.svg`/
  `unlink-hovered-btn.67ac688d.svg` 间切换；`.indicator.linked` 还有一整套
  `button--cta-show-active/locked/busy` 文本按钮形态。

本轮本地改动：编号项装进 `.indicator--list` 药丸（`bg(Colors::background())`＝`#111`、
`1px cx.theme().foreground`＝`#ccc`、`rounded(40px)`、`padding:5px`、`margin-right:10px`），
外层 `justify_center` + `margin-top:10px` 对应 `.indicator--container`，编号项仍是 26×26、
`rounded(30px)`、选中/悬停/警告配色与源码一致（此前本地把编号项裸放在 10px `gap` 的行里，
没有 `.indicator--list` 这层）。

2026-10-06 卡片徽标的 `[tooltip]` 已按源实现：`source_tip.rs` 新增 `TipAnchor`
（`Left`＝`left:50%`→提示框左边缘落在触发元素中线、`Right`＝`right:0`→右边缘贴右边缘、
`Center`＝`left:50%+translateX(-50%)`）与 `Kind::Badge{anchor,gap}`，皮肤按全局
`[tooltip]:before`（`font-size:14px`、`line-height:16px`、`padding:8px 10px`、`#000`/`1px #5d5d5d`/
`#ccc`、`white-space:nowrap`、`width:auto`、`.3s linear`），`.anchor--middle` 用 200px 宽 +
`break-spaces` 对应的换行、`gap = 5px`；新的 `SourceTipItem` 取代原来的 `card_action`，四个徽标按
源分别传 `anchor--left`（电源、Identify）、`anchor--right`（移除）、`anchor--middle`（接管），
并且只有选中卡片才渲染这些徽标、提示框才可能显示（对应
`.device--badge[tooltip]:before{display:none}` + `.device.active …:hover:before{display:block}`）。
原来这里用的是 Kit tooltip 与 `Kind::Icon` 的 `trigger.origin + (2, 34)`（该偏移在源码里没有依据）。

2026-10-06 同一轮续做：编号项的 `[tooltip]` 也按源接上。`TipAnchor` 增加 `Start`
（`right:auto`、没有 `left` —— 提示框左边缘贴住触发元素左边缘，源码里
`.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px);width:fit-content}` 就是这一条），
原来的 `left:50%` 改名 `Half` 以免与「左对齐」混淆；新增通用挂载 `SourceTipWrap`（可包住任意触发
元素，复用同一个 `TipLayer`），编号项不再用 Kit tooltip，改为 `SourceTipWrap::new(id, device 名称,
TipAnchor::Start, 10px, item)` —— 与徽标共用全局 `[tooltip]` 皮肤（`14px/16px`、`padding:8px 10px`、
`#000`/`1px #5d5d5d`/`#ccc`、`nowrap`、`.3s linear`）。

**仍未实现（已登记，不伪造）**：`.button--cta-link` 连接按钮（含
`link/link-hovered/unlink/unlink-hovered` 四个图标状态与 `.indicator--container .button--cta:before`
的 `left:0;max-width:300px;white-space:pre-line`）与 `.indicator.linked` 的
`button--cta-show-active/locked/busy` 组；另外 `source_tip.rs` 里的 `TipCommand`
（`Kind::Icon`/`Kind::Help`）当前仍没有任何调用点（`hasSwitch`/`tips` 在 784 的 Aether 页区段里
都不出现，本地页面也没有帮助图标），其中 `Kind::Icon` 的 `+2/+34` 偏移无源码依据——下一轮确认
该页确实没有 `.help`/`.tip` 后删除这段死代码，或按源接上。

## 2026-10-06 卡片徽标的可见性矩阵

源码 JSX 对每张卡片**无条件**渲染这四个徽标（`hasPowerButton` 才渲染电源徽标），可见性完全由
CSS 决定，基础规则是 `.device--badge{…position:absolute;visibility:hidden}`：

| 徽标 | 默认 | 选中卡片 | 选中+离线 | 选中+忙碌 | 忙碌（非选中） | 位置 |
| --- | --- | --- | --- | --- | --- | --- |
| `device--cta-power` | 隐藏 | **可见** | 隐藏 | 可见 | 隐藏 | 选中 `left:6%;bottom:54%` |
| `device--cta-find` | 隐藏 | **可见** | 隐藏 | 可见 | 隐藏 | 选中 `left:6%;bottom:37%` |
| `device--cta-del` | 隐藏 | 仅 `:hover` | 可见 | 可见 | — | 选中 `right:10%;top:22%` |
| `device--cta-enable` | 隐藏 | — | — | **可见**（白色变体） | **可见** | 居中 `left:50%`、`top:calc(50% - 10px)`（选中 `-8px`）、`translate(-50%,-50%)`、42×27 |
| `device--badge-offline` | 隐藏 | 隐藏（`right:10%;top:34%`） | **可见** | 隐藏 | 隐藏 | 非选中 `right:25%;top:40%` |

对应规则原文：`.device.active .device--cta-power{…visibility:visible}`、
`.device.active .device--cta-find{…visibility:visible}`、`.device.active:hover .device--cta-del{visibility:visible}`、
`.device.active.offline .device--cta-del{visibility:visible}`、
`.device.active.offline .device--cta-find,.device--cta-power{visibility:hidden}`、
`.device.active.linked .device--cta-find,.device--cta-power{visibility:hidden}`、
`.device.active.busy .device--cta-del,.device--cta-find,.device--cta-power{visibility:visible}`、
`.device.busy .device--cta-enable{…visibility:visible;width:42px;height:27px;top:calc(50% - 10px)}`（选中
`.device.active.busy .device--cta-enable{background-image:busy-btn-white;border-color:#fff;top:calc(50% - 8px)}`
且 `:hover{background-color:#707070}`）、`.device.offline .device--badge-offline{visibility:visible}`、
`.device.busy .device--badge-offline{visibility:hidden}`、
`.device.active.offline .device--badge-offline{height:24px;visibility:visible;width:24px}`。

本轮本地改动（`src/features/aether_strip/device.rs`）：这四类徽标不再只挂在选中卡片上，而是按上表
在每张卡片上按状态渲染——`power_find = selected && (busy || !offline)`、
`remove = selected && (offline || busy)`、`offline && !busy` 才画离线徽标；移除徽标默认
`invisible()` 并用卡片 `.group(id)` + `group_hover` 在悬停时 `visible()`（源码的
`.device.active:hover`）；忙碌徽标改用 `busy-btn`/`busy-btn-white` 两种皮肤、`top:84.5`/`82.5`
（`calc(50% ± …)` 减去 27px 高度的一半）、非选中卡片用 `#707070` 的 30.2% 透明边框；离线徽标非选中
卡片位置改成 `right:62;top:84.8`（`right:25%;top:40%`）。这些徽标都是绝对定位，所以本地用「按可见性
决定是否渲染」代替 `visibility`，效果一致且不会让隐藏按钮仍可命中。

**数据缺口（已登记）**：`.device.linked .device--badge{visibility:hidden}` 与
`.device.active.linked …{visibility:hidden}` 需要每台设备的 `isLinked`，本地 `Observation` 里还没有
这个字段；`hasPowerButton` 同理，所以电源徽标本地目前总是渲染（仅用 `power_on` 是否为 `Some` 决定
可用性）。

## 2026-10-06 链接形态与图标提示框：两条「缺口」的收口

**链接/解除链接按钮与 `.indicator.linked` 在当前 784 页面不可达**（所以本地不实现它是忠实，
不是缺 UI）。源事实：

- 指示器组件 `El` 的 props 是 `{devices, isLinked, setLinkedDevice, selectedIndex, supportsLinked,
  devicesData}`；未链接形态渲染 `.indicator--list`，只有 `supportsLinked` 为真时才追加
  `.button--cta.button--cta-link`（点击切到链接形态），链接形态才渲染
  `.indicator--wrapper`（`button--cta-show-active` 用 `DEVICES_LINKED_STATUS` 的
  `{{number1}}/{{number2}}`、`button--cta-unlink` 的提示框是 `DEVICES_ARE_LINKED`、
  `button--cta-show-locked` 用 `DEVICES_LOCKED_STATUS`、`button--cta-show-busy` 用
  `DEVICES_OFFLINE_STATUS`）。
- 轮播组件 `Gl=e=>{let E=e.devicesData,a=e.changeLinked,t=e.selectedDeviceIndex,_=e.supportsLinked,
  i=e.hasPowerButton,o=void 0===i||i,n=e.iotProps;…}`，而两个调用点都只传
  `devicesData`/`changeLinked`/`selectedDeviceIndex` —— `supportsLinked` 恒为 `undefined`、
  `hasPowerButton` 恒为默认 `true`；`isLinked` 是 `Gl` 内部 `useState(!1)`，只被那个不可达的
  链接按钮置真。设备卡片 `<ol>` 也没有 `isLinked` 属性（只有 `m.length>1&&R` 的合成卡片才传
  `isLinked:!0`，同样不可达）。
- 因此本轮**不**实现这些形态；本地已有的 `LIGHTING_DEVICE_TAKE_CONTROL_DESC`（`We.gxE`）正好也是
  `button--cta-show-locked` 的提示框 key。`{{number1}}/{{number2}}` 这类文案在本地
  `locales/*.json` 里已存在（`DEVICES_LINKED_STATUS` 等），将来若某个产品真的传
  `supportsLinked`，可以直接接入。

**图标提示框不再用无依据的偏移**：`TipCommand::icon`（页面里的 Identify 每个灯段按钮与刷新按钮）
原先把提示框放在 `trigger.origin + (2, 34)`，源码里没有这样的常量。当前源里这类图标与徽标同属
`[tooltip]` 伪元素，且**没有** identify/refresh 的专用覆盖规则，所以走全局
`[tooltip]:before{…right:0;text-align:left;top:calc(100% + 5px);white-space:nowrap;width:auto;
`z-index:100}`；本地改为 `Kind::Badge{anchor: TipAnchor::Right, gap: 5}`（与该皮肤同一套测量）。

## 2026-10-07 TAB_LIGHTING preview parity

The current source renderer at `main.3094caa4.js` offset `4601197` composes three source widgets: the Synapse Override widget (`kd`, title switch and explanatory body), Brightness (`Wd`, title switch and a 0�C100 slider), and Quick Effects (`Bd`, Quick/Advanced mode surface with effect selector/sync row or Chroma Studio profile selector). The local `AetherLightingPage` now keeps those widgets in the registered `TAB_LIGHTING` body, with stable test-support anchors for each control and both effect surfaces.

The preview has no DLL/service observation, so switches, slider, selectors, sync and Chroma Studio actions are disabled and values are shown as Unknown. It does not claim Synapse Override, power, online, lock, selected effect, profile, or effect catalog state. Remaining integration gaps are read-only device status observation, effect/profile catalogs and current selections, IoT device carousel selection, and later device mutation/write-back.
