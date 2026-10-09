# 托盘当前界面、会话与关闭契约

来源为当前 `.ref/host-4.0.827/` 与 systrayv2。公共呈现、翻译和语义命令在 `crates/razer-tray/src/lib.rs`，Windows 注册/原生菜单/窗口位置在 `tray/windows.rs`，账户分支在 `tray/account.rs`。这一区分不表示 macOS/Linux 注册已经完成；官方 host 本身在 macOS 跳过 LeftSystray。

## 关闭、图标与点击

主窗口 X/原生关闭按当前 `closeWindowAction=HIDE` 隐藏，托盘 Synapse 恢复保留的 GPUI 实体。Exit 排空已请求写入，不自动保存未提交草稿；固件升级保护保留，退出写失败恢复窗口并显示错误。托盘创建失败时关闭退出，避免留下不可恢复的隐藏窗口。没有实现 Electron renderer hibernation 或 host force-close-window。

当前宿主的右键菜单实际经过 Electron 41.2.0 `NotifyIcon::PopUpContextMenu` → Chromium Views `MenuRunner`，并非 Win32 HMENU；宿主还固定 `nativeTheme.themeSource="dark"`。此前把两者视为同一种原生菜单是不准确的。

本地保留 `tray-icon` / `muda` 的 HMENU 适配器，单应用顺序为 Synapse、分隔、Settings、Log In、分隔、Exit；tooltip 为 Razer。用户允许少量托盘框架视觉差异，并要求不修改本地第三方依赖。菜单绘制、系统字体、背景、间距和圆角差异没有消除；缺少的 GPUI 菜单样式接口已整理为 [待提交 issue](../gpui-popup-menu-issue.md)，未提交到外部平台。Windows 图标选择改为读取实际 HMENU 的 `COLOR_MENU`，不再错误地以 `AppsUseLightTheme` 推断菜单背景。此处保留原始深浅图标并确保原生适配器内的可读性，是明确的框架差异，不能称为官方固定深色呈现。

未构造其他应用安装状态或账户会话。通知区图标按系统小图标尺寸选择当前 ICO 原始16/20/24/32/40/48/64帧，当前应用菜单图标用20帧；不缩放512帧、不改色或透明度。齿轮和用户图按当前 host `getCachedThemeIcon` 保留原始 PNG 像素和尺寸：齿轮14×14、浅色用户12×16、深色用户14×14。`muda 0.21.0` Windows `to_hbitmap` 仍把菜单图像绘入16×16位图；应用图标的安装目录来源也未接入。

注册在主循环开始后进行，以 set_tooltip 的 NIM_MODIFY 结果确认，失败最多尝试四次。诊断先清 LastError，记录窗口归属、完整性级别、提权和作业 UI 限制；NIM_ADD/set_visible 的库返回值不足以证明 Windows 注册成功。已有环境取证显示工作区可继承 Low Mandatory Level 会使 exe 以 Low 运行而无法注册 Medium Explorer 通知区；本地输出目录约束见 [开发说明](../development.md)。本次没有运行托盘或更改 ACL。

单击等待200ms后转发当前 host 的 `click`，由 renderer 读取窗口状态并切换显示/隐藏；双击取消单击等待并聚焦面板。失焦300ms隐藏，隐藏取消未完成显示任务；后台原生调整尺寸完成后才显示，避免 WM_SIZE 重入。系统 launch 分支及多应用目录仍未接入。

## 面板与账户分支

未登录 DOM 挂载60px登录行与60px单应用区，宽360、Roboto16/1.22、源背景/边框；实际可见范围取决于网页请求的视口高度，不能把 DOM 子节点总高直接当窗口高。应用区单独持有1px上边框，内部启动按钮为59px，悬停不改变分隔线。登录、启动和账户名字按钮显式覆盖 Base Button 默认1倍行高；启动标题按源 `.launcher .title` 大写，并保留省略处理。按压只降低启动图标透明度，外框保留源700px最大高度和默认箭头。根容器修正为源 `min-height:100%`，允许内容撑高；此前 `height:100%` 会改变外框下边线与内容溢出的布局关系。

网页请求高度按当前源码单数 `.app.list-unstyled` 查询；当前渲染实际为 `.apps`，未登录期只计算60px标题高度。已补查 preload → host `SET_BOUNDS` → Electron `BaseWindow::SetBounds` → `NativeWindowViews::SetBounds`：`resizable:false` 时设置新的最小/最大尺寸为请求尺寸。因此 host 初始 minimum_height=200 不能作为最终高度下限，之前的360×200判断已纠正。Windows 未登录分支按360×60请求、`trayY-60`定位，保留源码右侧10px间距并去掉仅账户/body分支拥有的纵向夹取；源 `body overflow:hidden` 对应本地独立 viewport 裁切，源 `.systray` 本身是固定360px块容器。账户导航保留 `.navbar` 与独立 `.tabs` 两层，齿轮按钮拉伸到整行；本地不再用纵向 flex 代替块容器。

独立复核补齐宿主实际创建链：`convertFeatureToWindowOptions` 在 policy6 时给出 frameless/transparent 并保留布尔 `resizable:false`，`Tab/common.js` 的 `new BrowserWindow({...t,...})` 继续传递它；Windows `CanResize()` 在 frameless 分支直接读取 `resizable_`。源码 `597/a` 的第一轮横向限制使用主屏 **WorkRect** 宽度（字段 width，或左右端点绝对值之差），不是 MonitorRect 宽；本地第一轮已改成同样的工作区范围，保留后续独立的左右端点判断。源码使用 Electron DIP 坐标，部分 monitor 字段只除 dpiScaleX、其他字段不除；本地仍使用物理坐标和窗口 scale_factor 适配，尚未证明混合DPI或所有任务栏位置像素等价。

阴影也不能只读创建参数：宿主 `b` 对布尔值原样返回，`hasShadow:0!==b(r.hasShadow,1)` 会把传入的 false 转成 true。当前 Electron Windows 构造仍在 translucent 且 frameless 时将 `params.shadow_type` 设为 `kNone`；这一条件链已保留原文证据，没有通过运行观察确认最终窗口阴影。

60px仅对应当前无账户分支。当前源码 `597/c` 的账户高度为 body 子节点 `offsetHeight` 总和加153，夹取到400–700px；通知页复用Widgets高度。本地在 `on_children_prepainted` 中读取实际子节点高度，通过 `window.defer` 缓存测量结果，再在实体更新之外调整原生窗口。首次布局前用账户下限400px，不再固定700px；点击与设备状态更新只读缓存，不调用布局 API。设备行变化递增布局版本，旧回调不能覆盖新数据，调整尺寸保留托盘锚点且忽略未变化的高度。源码 `_f` 实为无操作函数，本地布局观察是 GPUI 适配，不应称为官方 devicecount 触发的窗口重排。

2026-10-09用户报告的左键崩溃来自此前 `requested_height` 在点击/实体更新中调用 `layout_as_root`，违反 GPUI 阶段限制；该调用已删除。修复只经过静态检查，没有运行托盘验证。

源码 DOM 中只有 `user.item.id` 成立才挂载 navbar / `systrayBody`，源初始 user.item 为 `{}`、launchers 为 `[]`。本地现在挂载 Guest 展示分支，并提供一行激活当前进程的 Synapse 页脚；这是本地呈现选择，真实账户和官方 launcher catalog/preferences 发布者尚未接通，不代表读取到了 Guest 凭证或官方安装应用列表。字体资产已注册Roboto，但实际回退、混合DPI/多屏/非底部任务栏尚未运行验收。

`TraySession` 明确区分 SignedOut、Guest、Authenticated。账户头部、访客/用户头像、Guest或razerId、登录/View Online 按钮、Widgets/Notifications导航、设备行、空内容及通知加载呈现都已有实际组件。当前默认仅用于源 Guest 展示分支；没有真实账户发布者驱动 Authenticated 分支，不能把本地访客标签当作 host session。头像/名字、通知已加载为空与尚未加载分别保留状态。

设置齿轮提示立即挂载，100ms后开始100ms linear淡入；离开/窗口失焦淡出，100ms后卸载，重入取消旧任务。提示宽300、右对齐、位于按钮下、padding7/8、边框灰、14px/1.22、层级1060。空Widgets/Notifications设置按钮保留100ms颜色与按压透明度；访客按钮发 account-guest-logout，未连接服务时明确报错，不伪造登出/登录成功。

Settings、quick-panel、Widgets、Notifications 四种命令都打开当前独立 Settings 窗口，默认 Software；当前包没有旧 section listener，不能据命令名造新标签。仅 quick-panel 首次创建传1000×768最小尺寸，其余保留host默认。命名窗口已存在则聚焦并保留页面；详见 [Settings 实现](settings-current-audit.md)。

## 当前原码的完整托盘分支图（2026-10-09 静态语义复核）

本节补的是当前托盘自身的实际可见分支和动作链。依据为 [托盘语义原文收据](tray-semantic-current-evidence.json)：135 个 AST/宿主原文区间、11 个 CSS 文件中的329条相关规则、30个输入文件的字节 SHA-256，以及11组人工逐项读过的语义结论。它不是全项目语义完成证明，也不表示本地组件已经实现所有分支。原码中的不一致保留为不一致，不替换成推测的产品意图。

| 层次 | 当前原码条件与行为 | 收据入口 |
| --- | --- | --- |
| 宿主左键 | `LeftSystray` 在 Windows 创建隐藏的300×200 policy6窗口；第一次点击若窗口不存在只重新初始化。已有窗口隐藏时先align，200ms后发送click并focus。双击取消click/blur计时器，发送double-click并focus；blur等待300ms隐藏。macOS跳过该窗口。 | `LeftSystray:*` |
| 加载失败 | `did-fail-load` 标记hasError并创建/显示razer-id；production/beta用id.razer.com，其余用id-staging。重新初始化登录成功后关闭该登录窗口，发送logIn。 | `LeftSystray:createLoginPage/createSystrayWindow/reInit` |
| Renderer左键 | `Je`读取真实getWindowStatus，再触发`Ze`查询已安装RazerAppEngine产品，最后按isVisible取反显示。align调用597/c。双击读取systrayDblClickAction；默认showMenu只显示，launch分支要求登录且razer-id窗口不存在，否则请求登录。 | `tray:2554:Je/Ze` |
| 根与账户 | 初始user.item={}、应用/launcher/installedApps空数组、通知hasItems=false。无id只挂载登录header及有launcher时的footer；有id才有navbar/body。Guest头像/名字/View Online按钮走logOut；真实账户走viewProfileInWeb忙状态；signed-out走logIn。 | `tray:2554:C/T/W/ae/oe/se/Re` |
| 页签 | Widgets/Notifications来自W0；badge统计所有通知条数。齿轮隐藏面板并launchSettings，minimum1000×768。footer不存在时body增加taller class。 | `tray:2554:he/Re` |
| Widget选择 | showWidget&&hasWidget，Spatial存在时排除Surround；按widgetIndex排序。Synapse/Chroma还需installedApps包含该引擎。六个lazy分支为Surround、Spatial、Synapse、Chroma、Cortex、Gold；Guest不挂Gold。 | `tray:2554:Oe` |
| Launcher | module9001/h要求installedModules与isAppInstalled共同成立；f建立catalog/运行客户对象/hibernation/sort/hidden，v从保存launcher数组或按title排序的前五个普通app产生footer。website打开浏览器并区分Guest/认证；已运行app广播restoreApp，其余兼容分支或创建命名窗口；登录窗口存在时排队。 | `tray:9001:h/f/v/S/g`、`tray:2554:qe/Qe/me` |
| 通知 | 获取失败只记录错误，维持未完成加载；成功按startDate倒序。卡片先打开URL，再等待mark-read返回成功才改本地status。Clear请求原RazerApp异步dismiss流程，但组件没有await就清空本地items。pending、已加载空、已加载有卡片是三个独立分支。 | `tray:2554:_e/F/Se/Ne`、`RazerApp:*Notifications` |
| 右键 | 当前host按User Data/Apps中识别出的应用目录建菜单，处理失败待重试项、翻译/beta图标、条件分隔线、Settings聚焦/启动、登录退出及运行应用数量对应的Exit/Exit All。Guest的文字条件与点击条件不同，见下表。全局登录退出向活跃webContents广播，再通知LeftSystray。 | `host:zn/xn/qn/Hn` |

宿主接口不是一个直接的“托盘DLL”。桥接模块7217的callElectronAction交给preload.doElectronAction，再进入main.js的electronAction switch。getWindowStatus从发起窗口读取状态，SET_BOUNDS调用同一窗口setBounds；GET_WINDOW_SERVICE_CLIENTS列举tab/window/webview和Hibernated状态。宿主constants的ACTIVE_WINDOW_SERVICE_CLIENT实际字符串就是`activateWindowServiceClient`，共享focus分支聚焦窗口/tab，subTab再发focusSubTab。getWindowStorageItem/registerWindowStorageEvent及memory等堆叠case必须继续读到`resetWindowStorage/resetMemoryStorage`的共享delegate，不能把空case当无功能。

window_storage按来源URL持有Map；getWindowStorageItem返回跨URL的`[{value,windowName}]` JSON包，仅保留truthy值。原生事件仅送其它活跃、允许订阅的webContents；值本身仍由各页面发布。memory_storage拥有独立Map及memoryStorageEvent。其JSON层、窗口/URL作用域和订阅清理都有原文收据；这些host存储并不等于设备DLL观察。

## 六组Widget的原码动作与数据边界

| Widget | 可见分支、状态和动作 | 当前静态证据边界 |
| --- | --- | --- |
| Surround219 | app未运行显示启动提示；运行时按hidden决定开关区。mount/appLaunch请求getSurroundSoundOn/Disabled/Hidden，监听对应change；点击广播toggleSurroundSound，也保留旧兼容对象调用。 | 回调直接调用thunk creator，见下一节，不能据此声称Redux收到状态。 |
| Spatial3190 | spatialAudioWidgetsHidden中的spatial-audio-on/profiles分别隐藏字段，两者均隐藏返回null；运行且未整体隐藏显示开关/当前profile，未运行显示启动提示。关/disabled时profile区禁用；profiles<2禁用标题；dropdown用portal，body高-10边界，blur收起。选择广播setSpatialAudioProfile。 | 查询/广播名字按原文保留，包括spatialAudioDisabledOnChange和spatialAudioHiddenOnChange；recipient最终设备写入另行审计。 |
| Synapse5492 | connectedDevices→去重/ready+serial过滤→hidden与container过滤→子设备展开→widgetIndex/title排序。windowStorageEvent响应connectedDevices重查，dashboardDevices过滤当前列表。运行或hibernated显示设备列表，否则骨架与启动提示；空devices与devicesConnected不等时整节不显示。 | 托盘只消费MW发布结果；没有从tray读取一个DLL就直接得到所有配置的链路。全产品发布者/查询映射不在本节宣告完成。 |
| Chroma2215 | 运行或hibernated挂三种子Widget，未启动用骨架。chromaAppWidgetsHidden隐藏Studio/Visualizer/Connect。Studio按currentUserId取profile，选择写renderer存储currentProfile；Visualizer按当前user取visualizerState并写同一用户的存储行。 | 5586 hook读取synapseInstalledModules：已有模块聚焦模块，否则聚焦Chroma对应subTab；两者可关闭配对窗口。这里的本地设置变化不是设备DLL写回成功。 |
| Chroma Connect1899 | 从memory分别读devices/apps，订阅两类键；实际DOM只渲染connectedDevices，并按名字隐藏行。enable=0为off；启用且running=0为disconnected，running=1为connected。开关向memory action key提交`{action:"setAppEnabled",payload:{name,enable}}`。 | 读取connectedApps不等于渲染它。提交memory action不等于下游DLL执行/确认；消费者完整链仍待审计。 |
| Cortex2033 | 运行或hibernated显示game-launcher与button，两者都隐藏返回null。游戏卡点击隐藏面板并广播cortexLaunchGame；封面error准备hasError/icon回退。btnState=boost广播cortexBoost，否则cortexRestore。未运行显示启动提示。 | 原码广播与image-error回调的thunk调用均不应被写成已确认Redux更新。 |
| Gold3482 | Guest上游不挂载；gold/silver隐藏项均成立返回null。balance.hasItems=false用loading素材，否则显示两值。点击viewAccountSummaryInWeb，pending期间disabled；不隐藏面板，成功then解除busy。 | 无catch/finally，因此静态代码不能保证失败后busy解除。账户余额不是设备数据。 |

Synapse设备列表先按完整devices.length设置60×count的minHeight，再过滤MIXER_SYSTEM_CHECK_FAILED渲染行；因此可见行数不必等于高度计算数。设备title语言只在zh-cn用zh-cn，其余用en。profiles先自然排序，若有isShownOnUI子设备则用其profiles/activeProfile覆写；该覆写数组不重新排序。Auto标签追加` (Auto)`、隐藏三角，并允许profile点击继续冒泡到设备行；不支持profile显示文字Default。动态Loupedeck分支与一般profiles<2禁用规则不同。

设备行激活UIWindowName；若Synapse处于Hibernated/hibernateStatus3则激活Synapse，3000ms后focusDeviceTab，否则0ms后focus。电源off不发focus。3858/3880改container为monitor，780/781/782/783/784/790/791改为iot。选择配置向MWWindowName BroadcastChannel发两条原始消息：ON_SWITCH_PROFILE和crossPageRequest/switchProfileBySystray，含GUID与实际子设备container；发送即关闭channel，没有设备成功确认。

## 原码中的不一致必须保留

| 当前实际原文 | 对后续还原/文档的约束 | 收据 |
| --- | --- | --- |
| Re写`visibility:a?"visible":"none"`；独立`M.W0.widgets`常量分支每render都更新body高度缓存。 | none不是有效visibility值；不能描述为等价hidden，也不能把缓存分支解释为仅Widgets页运行。 | `tray:2554:Re` |
| 空Widgets按钮调用launchSettings({section:"notifications"})，随后广播widgets scroll。 | 保留调用参数与广播的差异；不能凭按钮名字改为源代码不存在的section。 | `tray:2554:Oe` |
| footer先用installedApps过滤n为a，CSS apps-N、单项title、tooltip首尾仍按n.length。 | 显示条数a.length与源样式计数不同，不能用过滤后计数冒充原行为。 | `tray:2554:me` |
| Ne用HH:DDA，isSame(now,"m")。 | Moment的m为minute，不能称“同月通知日期”。保留原码格式，翻译意图未知。 | `tray:2554:Ne` |
| er/o已JSON.parse客户列表返回数组；5492/y非hibernated分支在激活后又JSON.parse该数组。 | 配对窗口cleanup静态存在输入类型冲突；不能描述为保证关闭。5586的相邻hook直接在数组上find，没有该二次解析。 | `tray:7660:o`、`tray:5492:y`、`tray:5586:p` |
| synapseWidgetsBatteryHidden只产生showBatteryWidget；row解构的是showBatteryValue，没读取showBatteryWidget。 | 偏好字段存在不等于该源版本实际隐藏电池。不能把两个字段合并解释。 | `tray:5492:x/y` |
| ReachChargingLimit的计算class为`battery-N -paused`；CSS选择器为`.battery-N-paused`。 | 原class含空格，与CSS拼接后缀不匹配，不能宣称暂停图必然命中。 | `tray:5492:y`、492 CSS rules |
| 219 change回调调用l/c/i，3190调用u/m/g/f/v，2033调用h/x；这些函数返回thunk。 | 这些回调没有dispatch也未调用返回thunk；connect虽然绑定同名props，组件没有解构使用它们。Cortex图片error也直接h(array)。这里只确认静态遗漏，不声称实际运行最终无其它更新来源。 | 三个模块component及thunk原文 |
| 右菜单label使用`an&&!sn`决定Log Out，handler只用an决定logout/login。 | Guest状态下显示Log In却可能点击logOut；不能按文案推断动作。 | `host:zn` |
| 双击launch fallback的title查找用`a.appName`，a是外层logger import。 | 保留来源不一致；不能静默改成fallback行自身appName后称为原码。 | `tray:2554:Je` |

## 非设备Widget的布局、字体与颜色依据

以下均为当前CSS声明；完整选择器、条件、URL、声明和font-face区间在同一语义JSON中。继承、浏览器嵌套规则及实际字形结果未运行验证。

| 区域 | 原始数值/声明 |
| --- | --- |
| 全局 | `*`字体链Roboto、Arial、Microsoft YaHei New、Microsoft Yahei、微软雅黑、宋体、SimSun、STXihei、华文细黑、sans-serif；body #ccc、16px、1.22、overflow:hidden。 |
| 通知 | 卡片#111，卡片间10px，最后0；图高102px；info padding20px；title单行大写、line-height1、下距5px；description14px/#999，两行截断/34px，下距4px；helper12px/#999，日期右距10px。spinner31×31绝对居中；空态padding20px，14px文本，下距10px。 |
| Surround/Spatial | switch padding21px 20px 20px，标签右距10px。Spatial profile图40×40、背景30px、右距10px；label12px大写/#999，下距2px；title高18px、右留18px，下拉三角5px边框；active/hover #44d62c。item disabled opacity.3并pointer-events:none。 |
| Chroma | item flex/max-height80px；info左padding10px；title12px/#999/大写；profile下距5px。实际SVG/背景素材及profile portal按对应子组件receipt定位，不以抽象box替代。 |
| Cortex | games margin-top19px、左右padding13px；卡片margin5px，图155×88、透明2px边框、radius5px，hover边框#44d62c；游戏hover黑遮罩.3，pressed白遮罩.1；fallback icon40px，play30px。最后子节点下距20px，games后button上距5px。 |
| Gold/Silver | item各50%宽、padding10px 20px、max-height60px；图36×36/右距10px。value为RazerF5开头字体链、20px、下距2px；gold #ffa800，silver #30d5ff；loading60×16，上距4px下距6px。 |

维护验证：`node tools/audit-tray-semantic-current.cjs --check`会重新解析所有引用源码、原码异常断言与相关CSS并比较生成JSON。当前只确认上述托盘组件自己的分支与直接host接口；音频/Cortex广播接收者、Chroma memory动作消费者、所有connectedDevices产品发布者到DLL的完整链仍未在本节逐条闭合。现有运行/截图与账户真实性不在静态检查的证明范围。本轮未修改Rust/UI、执行应用、厂商JS或DLL。

## 剩余项

设备行接入已有实际连接/电量观察；图标使用当前源码分类 SVG，产品名称使用当前来源目录的语言字段。当前配置名称若来自本地工作区草稿，会用提示标识本地状态，不能作为设备活动配置读取成功的证据。真实账户登录/登出/在线档案、通知、完整Widgets状态字段、配置切换、官方安装目录与启动偏好、多应用菜单/启动/退出、Exit All、系统launch双击动作仍未接入。当前头部/空态组件的存在不代表账户数据读取成功。按住鼠标移动时 GPUI hover 与 DOM hover 存在边缘差异；原生菜单、焦点、透明窗口、字号、动态高度、DPI和动画均未运行验收。

## 保留证据

- [当前线上源校验](tray-live-source-check.json)、[菜单与窗口来源](tray-source-receipts.json)、[关闭链](save-close-current-source.json)。
- [UI/CSS](tray-ui-current-evidence.json)、[账户组件/提示](tray-account-current-evidence.json)、[ICO原始帧](tray-icon-validation.json)。
- [当前呈现修正](tray-presentation-current-evidence.json)保留本轮JS AST、CSS、host图像加载和PNG尺寸收据；维护检查 `node tools/audit-tray-presentation.cjs` 与 `.work/resource-env/Scripts/python.exe tools/validate-tray-assets.py`。
- [窗口与框架链](tray-window-current-evidence.json)保留静态PE版本串/摘要、Electron版本固定URL/摘要、host/preload至尺寸约束的源片段和明确的平台差异；维护工具 `python tools/audit-tray-window-current.py`，可用 `--fetch` 仅下载惰性文本来源，不执行它。
- [账户与应用状态边界](tray-state-current-evidence.json)、[设备行与图标](tray-widgets-current.md)、[页脚与版本](tray-footer-current-evidence.json)分别保留当前源码状态生产者、492组件和页脚CSS/版本依据。
- 维护工具 `prepare-tray-account.cjs`、`audit-tray-account.cjs`、`prepare-tray-account-assets.py`、`tray_assets.py`；主资源清单保留图像输入输出摘要。

这些收据记录静态来源和取证时文件，不能据编译/摘要匹配声称真实窗口或登录服务已经验收。应用、测试、安装器、厂商JavaScript和DLL均不在开发验证执行范围。
