# 当前鼠标底座配对页复核

2026-10-07 续查见 [接收器交互复核](receiver-interactions-verification-2026-10-07.md)：纠正 164 名称误导航、241 edition=0 匹配、鼠标/键盘顺序及已配对行布局。下文 2026-10-06 的“下一轮”与“仍未完成”是当时记录；设备名基础导航已局部接入，完整连接/配置门控和真实服务仍未完成。

产品 164 Mouse Dock Pro 与 241 Mouse Dock V2 Pro，2026-10-03。来源、Acorn UTF-16 组件偏移、当前 JS/CSS/manifest SHA-256 见 [原码证据](dock-pairing-current-evidence.json)。164 普通页为 TAB_CUSTOMIZE，241 为 TAB_PAIRING；独立 pairing 根并不等于普通页配置栏条件。

本轮补齐 164 原本漏取的主包内十语言字典，使用挂载点指定的 `zia_pairing.8cf19beb.avif` 单设备图（77×60），不再混用一般产品大图。241 增加取消配对完成 / 配对完成翻译键。共 27 个资源，`validate-dock-pairing.py` 校验当前源、组件区间、20 个语言表和资源声明/内容哈希。

页面保留产品横幅、配对入口、单/双设备信息、多设备工具条件及原接收器警告。弹层保留扫描、无结果、多候选、配对、失败、已配对、取消确认、取消中、完成状态；扫描/配对有状态门控。没有设备服务时明确不可用，不用计时器生成成功。独立样例共 18 个场景，配对观测不进入配置文件。

源样式修正包括：双设备卡片左右 20px 内边距、选中卡片外侧 2px 描边、Roboto 正文；单设备弹层 #111 背景 / #515151 外框与标题边线；双设备 #222 背景 / #5d5d5d 标题边线、RazerF5 16px 标题；双设备正文底部 150px；原进度 SVG 的 1 秒循环旋转与选择按钮 .3 秒透明度过渡。页面/单设备/双设备取消配对按钮保留各自 hover / pressed 规则。

样例原先默认直达没有标题栏的弹层正文，现默认进入设备页并通过正式 DockDialog 打开配对；关闭返回设备页，可再次打开。服务预览另提供 164 / 241 的正常产品标签页入口。

2026-10-06 配对握手与警告分支批次。按当前源把 713 设备的额外手续补成源里的两步握手，并把两条警告的
条件与配色对齐：

- 713 握手（`241/914.4c13bdac.chunk.js`）：`bindDevice` 里 `P.dongleId !== 713` 才直接
  `DUALLINK_BIND_DEVICE`，等于 713 时只 `SET_DUALLINK_WARNING(true)`；设备页 `Ps` 的 effect
  （约 50032）把它换成 `SET_CONTINUE_PAIRING(true)`；配对工具的 effect（约 40667）再在
  `continuePairing` 为真时用 `mode:1` 绑定扫描结果里 `dongleId === 713` 的那台并清零。
  本地把这三步按同序实现在 `dock_pairing/dialog.rs`（`duallink_warning` → `continue_pairing`
  → 绑定），没有再自造“暂未接入”的提示；没有服务时仍由 `request()` 明确拒绝，不伪造绑定。
- 警告条件：页面与弹层都按 `DeviceInfo.showBothDevicesConnectedWarning` 门控
  （`dock_pairing_data.json` 的 241 config 本就有该字段），弹层另外要求鼠标与键盘两条通道都有
  已配对设备（源 `we(bindInfo)`）；`V = !W && …` 的压制关系也照搬：两条通道都在时不再显示
  “无法配置轮询率”那一行。
- 警告配色/形态：页面两条警告用 `.dongleWarningText` / `.bothDevicesPollingCappedText`
  的 12px `#999`；弹层提示用 `.Duallink_bothDevicesConnectedWarning`
  （`margin:50px auto 0;max-width:520px;gap:10px`）与 `_Text` 的 14px/17px `#ccc`；
  轮询率说明行用 `.pollingRateInfo{color:#999;margin-top:10px}`。不再借用主题的
  `muted_foreground`。
- 配对工具说明的语言键：164 的当前源写 `ENABLE_LAUNCH_PAIRING_UTILITY_INFO`，241 的当前源写的是
  拼错的 `ENABLE_LAUNCH_PARING_UTILITY_INFO`（241 alias 表 `Gi`）。本地之前两个产品都取正确拼写，
  于是 241 实际落到公共语言包。现在键写进各自 config 的 `launchUtilityInfoKey`，并补齐 241 设备包
  十语言的该键文本；`validate-dock-pairing.py` 新增断言，要求每个产品的该键必须存在于它自己的十份
  语言包里，公共语言包不能顶替。

### 下一轮可用的源码事实（本轮只核对，未改代码）

- **设备名链接**：`q`/`Z`（配对工具 914 包的 `pairedTitle` / `deviceRow`）只在
  `Es(e, devices)` 为真时才把设备名渲染成链接，否则是普通 `<span>`：
  `Es = (e, s) => { const t = xs(e.productId); const n = e.editionId;
  return s.some(x => xs(x.productId) === t && (!n || x.editionId === n)) }`
  —— 即“该设备存在于 App 的设备列表里”。链接本身是
  `<span className={"hyperpolling-span-hover " + deviceNameLink} onClick={() => z(e)}><u>{name}</u></span>`，
  CSS 为 `.deviceNameLink{cursor:pointer;text-decoration:underline}` 加
  `.body-widgets .widget .hyperpolling-span-hover:hover{color:#44d62c}`（类名
  `HyperPollingWirelessMouseDock_deviceNameLink__jCiyT`）。句子来自
  `SEAMLESS_AUTO_PAIRING_PAIRED_DESCRIPTION`（本地数据已含 `{{deviceName}}` 占位）。
- **点击行为 `z(e)`**：从设备列表/`ready` 的运行时里解析 `deviceContainerId`，然后
  `window.open("<origin>/synapse/products/<productId>/ui/index.html?containerId=<id>&subTab=<tab>",
  name, "shouldFocus=1,policy=3,tab_visible=1,subTab=…")`，没有容器时走
  `activateWindowServiceClient`。本地等价的“打开某设备页”是
  `AppShell::navigate(Location::Device(key))`，可从设备页经 `WorkspaceEvent` 请求
  （`ProductWorkspace` 已把子工作区事件 1:1 转发给 shell）；渲染条件需要的“设备列表”
  目前只在 `AppShell::devices` 里，要按 `profiles_page.set_devices` 的既有模式下发。
- **轮询率说明行**：`Ps` 里 `I = pairedInfo.some(e => hs(e, validDevices,
  connectedRuntime, productId))`，随后 `I ? MOUSE_DOCK_PAIRING_POLLING_RATE_INFO :
  CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT`。`hs` 的第一个分支就是
  `if (e.status === 1) return true;`，其余分支查 `validDevices`、`connectedRuntime`
  里 `setupStatus === "ready"` 的设备或该 dock 运行时的 `subDevices`。也就是说“已连接”
  至少要 `pairedInfo` 条目带 `status`/运行时数据，本地 `state::Peer` 现在没有这些字段。
  注意 1404/1465 的同一控件用的是 `MOUSE_DOCK_PAIRING_POLLING_RATE_DISCONNECTED_INFO`
  （“确保已配对设备已连接并安装”）而非 241 的 `CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT`。
- 本地 `DockDialog::request` 在 `!preview` 时直接返回“配对服务暂不可用”，即真实服务回流
  尚未接入，上面两项目前只在预览数据下可见；实现前需要先确定设备列表/连接状态的数据源，
  不伪造。

仍未完成：实际配对传输、服务事件回流与超时；设备名链接（源 `.deviceNameLink` 点击打开设备页）与
`I ? MOUSE_DOCK_PAIRING_POLLING_RATE_INFO : CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT`
的“已连接”分支需要本地还没有的 `pairedInfo[].connected` 数据源；原版部分状态动画、弹层完整
viewport/滚动等价与键盘焦点实窗验证；当前弹层可视高度限制为 viewport 减 100px，而原泛用 Dialog CSS
写 100vh，该滚动几何差异尚需进一步核对调用环境。上层配置更多菜单也未完成。上述局部修正不是完整视觉验收。

验证仅为允许的提取一致性、资源校验、格式检查及统一 `cargo check --locked --all-targets`；没有运行应用、测试或下载代码。
