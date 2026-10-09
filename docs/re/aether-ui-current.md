# Aether Light Strip UI 当前契约

范围为产品784的 CUSTOMIZED 与 TAB_LIGHTING，以及共享产品头部。当前源 `.ref/devices/784/static/js/main.3094caa4.js` 的 SHA-256 为 `3751063a619bd163a7ac9d8bf238a5452154ecf20fbaf04fcaaee4275b65a1f2`，CSS 为 `main.03a1d509.css`。实际组件、SVG、10种语言和资源来源保存在 [当前源码收据](aether-strip-current-evidence.json)；维护工具只解析当前 manifest 声明的代码，不执行下载的 JavaScript。

## CUSTOMIZED 与本地布局

`AetherStrip` 使用实际 `Sg/Gl/ol` 外层和布局组件：四种原SVG形状、每段LED数量、总配置/检测数量、分段识别与刷新。数字输入限制两位十进制手输，步进1；提交最小1，最大为当前段值加剩余LED。切换1–4段按源依次向上取整分配全部检测数量，段识别使用零起点、包含末端的 `colStart/colEnd`，以及 `pollTime:3,sleepInterval:500`。

布局只有在实际观察 online=true、locked=false、power_on=true、synapse_override=true 时可编辑。源初态为 detectedLedCount=0、bendData=[]、isRefreshing=false；正式页面没有默认60 LED设备。源初始/未知占位数值不能作为检测成功。刷新状态仅由显式观察提供，不能由点击自动制造完成。

`snapshot()` 仅保存 `bendData`，由设备级 `source_device_settings._accessory` 持久化，不随 Lighting profile 切换。restore只接受1–4个正整数段，重新规范顺序ID，检查总和溢出及已知检测总数；不从草稿恢复设备在线、锁定、亮度或检测状态。

UI布局保持600px Widget、源边距、RazerF5标题/Roboto正文、62×26输入和原动作图标。刷新旋转图从源SMIL重建2秒角度/弧长时间轴及square cap；减少动态效果时静止。

## 设备轮播与操作

设备卡为248×212外层与内部device两层；未选中透明度0.5，选中/未选中图片与padding按源分开。多设备容器保留左右496px padding和 `scrollWidth/5*index` 居中偏移，单设备无此padding。原CSS `scroll-behavior:smooth` 没有给出持续时间，本地使用相同目标偏移直接定位，平滑滚动的运行一致性仍待验证。

编号指示器使用源药丸边框、26px编号与选中/离线/忙碌颜色。卡片电源、Identify、移除、接管和离线徽标按选中/忙碌/离线可见性矩阵呈现；隐藏动作不能继续命中。当前源调用没有传 `supportsLinked`，内部linked初值false且只有不可达链接按钮能改为true，因此784不应凭共享组件存在新增链接/解除链接形态；`hasPowerButton` 在该调用中采用源默认true。

重命名、移除与接管有原文案/本地确认、关闭与焦点恢复；移除/接管对话框保持源无暗色遮罩。Identify在电源打开观察后等待500ms才可用，关机/未知/新观察和dismiss会取消旧计时。该延迟代码已存在，不再作为未修缺口记录。

编号、徽标、识别和刷新图标的提示框使用源 `[tooltip]` 皮肤、锚点与300ms透明度变化；Identify/Refresh为右对齐、下方5px。帮助提示使用实际 `Fd` 即时显示规则，Lighting Override/Brightness也确实调用此控件。不存在“帮助控件没有调用点”的当前结论。帮助边界仍按窗口viewport实现，源使用 `.main-container > #body-wrapper`；滚动容器内定位与窗口/DPI命中待验证。

## 头部与Lighting

784根始终挂ProfileBar，CUSTOMIZED只是禁用下拉并显示unsupported同步图标；TAB_LIGHTING允许切换profile；HELP保留下拉但同步图标unsupported。不能把禁用当成隐藏。头部复用正式 `SourceProductWorkspace`，不在灯带模块复制导航。

当前Lighting实际根 `aN` 组合 Synapse Override `kd`、Brightness `Wd` 和 Effects `Bd`。Override文案必须是 `SYNAPSE_OVERRIDE_HEADER`/`SYNAPSE_OVERRIDE_TOOLTIP`，不是Windows Dynamic Lighting。Brightness为0–100、步进1，状态保留在实体中，源端点0/100可见。

Quick/Advanced是互斥Tab内容，初始本地视图为Quick；选中状态有源药丸样式与可访问语义。切Tab仅改变本地视图，不代表实际设备效果模式。设备状态未观察时显示Unknown，不能用DEVICE_OFFLINE断言离线。设备开关、亮度、选择器、同步和Studio动作保持不可用；中性开关/滑条位置不是观察值。

## 当前读取与编辑缺口

正式Aether尚无IoT服务状态发布者，`Observation`的状态只有隔离预览样例提供；request在预览记录action/payload，正式页显示服务不可用，不宣称设备已改名、删除、接管、闪烁或写入。样例60/45LED及样例名称不进入实际产品观察。

以下仍需完整接入：真实IoT设备列表/选择、在线/锁定/电源/Override/检测数量观察，Lighting产品效果及Chroma Studio profile目录/当前选择，Override真实说明/图标分支、效果参数编辑、Advanced安装/可用性分支、真实锁定/离线覆盖层，以及Lighting的本地Apply/Save意图与草稿持久化。已有CUSTOMIZED本地布局保存不等于Lighting编辑已完成。设备命名、移除、接管、电源、识别及LED配置的设备/服务写回尚未接通。

HELP内部全部子分支、提示框viewport边界、滚动/平滑居中、字体连续文字换行、焦点及各DPI实际画面尚未验收。页面保持partial，不登记完整产品完成。

## 防回归与验证

`tools/extract-aether-strip.cjs --check` 保持当前源切片/语言/原SVG一致，`tools/validate-aether-strip.py` 校验资源、header门控、分段恢复边界及轮播源结构。Lighting的test-support覆盖正式入口、Override语义、保留的亮度状态、端点、Quick/Advanced双向切换和设备控件禁用。

仅允许运行静态解析、资源/JSON校验、格式化和 `cargo check --locked --all-targets`；测试源码只编译。没有运行应用、测试、下载JavaScript、安装器或DLL，因此不声称设备读写或运行视觉通过。
