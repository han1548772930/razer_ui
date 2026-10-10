# 当前公共应用 UI 细节与操作链

本页把22个实际HTML应用的控件、条件、原回调与样式来源展开，补充 [应用结构链](all-application-chains-current.md) 和 [应用native链](application-native-current.md)。范围还含两个无当前HTML的登记端点。来源保持2026-10-02固定的当前生产语料，本轮2026-10-09仅静态读取，没有运行应用、厂商JS、DLL、构建或测试，也没有修改Rust实现。

[独立机器证据](evidence/application-ui-details-current.json.zip)保留165个应用内UI来源文件的18,432个JSX/createElement候选、3,619个事件属性、23,420条与字面class/基础元素匹配的CSS、全部所遇font-face原文、51个人工语义锚点和475段已逐字节重验的既有细节原文。每段收据都有原文件SHA-256、UTF-16开始/结束区间和片段SHA-256；CSS条件与声明顺序保留。此前旧式 `Object(jsx)` 与现代 `(0,jsx)` 分别按AST解包，不因CRA编译形式漏掉Natalie页面。

这些数字是源码索引数量：共享控件、副本、SVG与未挂载分支都可能在其中。子控件记录的是原调用顺序；CSS、portal或flex可改变实际视觉顺序。回调属性不等于独立业务功能，字符串class匹配不等于浏览器computed style。**完整UI语义完成数仍为0**。本页不复核本地Rust是否符合这些来源，也不把已有本地草稿计为原设备保存。

## 逐端点审查状态

“部分细查”表示本页人工锚点或已重验的专门证据覆盖了实际控件/状态，仍有明确未解分支；“控件索引”表示本轮完成源参数/回调/CSS留证，尚未把每个业务调用追到结果和清理。22个HTML入口不是22个已完成界面，Background Manager是后台基础设施。

| 当前端点 | UI调用/事件属性候选 | 本轮细节与既有当前契约 | 尚未还原的边界 |
| --- | ---: | --- | --- |
| /alisha/ | 2155 / 303 | 部分细查：四tab实际认证门控、SDK警告、设置lazy层、登录生命周期 | alerts/editor/connections内全部编辑、保存、灯光worker与服务回执 |
| /background-manager/ | 0 / 0 | 无JSX可见UI；[资源owner链](application-native-current.md)单列 | 后台状态发布不能算已还原可见界面 |
| /chroma-app/dashboard/ | 1599 / 308 | 部分细查：[Dashboard](chroma-dashboard-current.md)、Chroma应用原文重验 | 非空SDK服务组、拖动优先级、全部启停/维护/失败分支 |
| /chroma-app/settings/ | 575 / 101 | 部分细查：[Settings](chroma-settings-current-audit.md)，原控件与回调重验 | 全部教程消费者、host观察回执、样式组合态 |
| /cortex/ | 0 / 0 | 无当前HTML，不能借其他应用推定 | 当前独立源缺失 |
| /feedback/ | 358 / 67 | 部分细查：完整ps表单/提交/错误/结果类；[表单契约](feedback-app-current-audit.md) | 真实账户、日志归档、服务器与结果页消费 |
| /natalie/ | 495 / 76 | 部分细查：hash页面、pod门控/本地设置/原生操作；[Ring Light](ring-light-ui-current.md) | 全部账户许可、跨屏/DRM、原生回执与监听清理 |
| /profile-migration/ | 363 / 59 | 原source重验与控件索引；[迁移标题栏](profile-migration-header-audit.md) | 正式/preview、扫描/选择/转换/取消/结果全部状态机 |
| /rz-app-menu/ | 43 / 7 | 控件索引；实际iframe消息、设备ready筛选、模块/应用分组已读 | 全部安装状态、maintenance、推荐与父窗口处理器 |
| /rz-user-profile-menu/ | 35 / 9 | 部分细查：账户/访客顺序、余额/反馈/退出、iframe消息、200ms重入保护 | 父窗口账户生产者与退出确认的完整对端 |
| /settings/ | 706 / 125 | 部分细查：Software→Systray→General实际lazy根；[窗口契约](settings-current-audit.md)原文重验 | 软件非空目录、5槽、Widgets、host同步与错误回执 |
| /sophie-lite/ | 459 / 71 | 部分细查：audio/activation/settings/promo四分支、分段码、设备菜单、两步setup | 全部license/云状态及真实音频native结果、销毁 |
| /sophie/ | 1034 / 234 | 部分细查：许可根、AUDIO/EQ/CALIBRATION/DEMO、Onboarding与设置 | 原native写入结果、每个profile/EQ/stream分支、音频测试与远端许可 |
| /synapse/ | 0 / 0 | 无当前HTML；正式Dashboard来源另列 | 不能从登记URL补造顶级HTML |
| /synapse/alexa/ | 760 / 83 | 部分细查：安装、登录/激活/ready、设备、设置；本页保留完整锚点 | 地区/语言/授权轮询/服务/退出/安装结果全部对端 |
| /synapse/armory/ | 1824 / 291 | 当前[Armory审查](armory-app-current-audit.md)原文重验、控件索引 | Browse/Share/贡献表单、全部displayMode及真实网络 |
| /synapse/chroma-studio/ | 1924 / 596 | 当前[属性契约](studio-properties-current.md)原文重验、Editor/模态/控件索引 | 真实LED、全部图层拖排/编组/关联/导入与文件落盘 |
| /synapse/dashboard/ | 2727 / 618 | 部分细查：xi/z/ji、[Dashboard](dashboard-ui-current.md)、Banner/Card原文重验 | 状态生产者、非空/异常组合、全部displayMode与服务请求 |
| /synapse/introduction-tour/ | 201 / 36 | 部分细查：Synapse5步/Chroma3步、媒体、Previous/Next/GetStarted/Skip | 源码路径判定与host路由联动、媒体加载错误与实际滚动 |
| /synapse/macro/ | 900 / 272 | 部分细查：profile菜单/未保存操作及[Macro](macro-ui-current.md)编辑/录制原文重验 | 全部快捷键能力、录制屏幕与inputredirect、共享/导入与服务持久化 |
| /synapse/profiles/ | 772 / 153 | 部分细查：Games/Devices导航、搜索/排序/关联/传输；[Profiles](profiles-ui-current.md) | 非空游戏目录、扫描、所有.serial/container分支与实际文件输出 |
| /synapse/settings/ | 606 / 108 | 部分细查：startup/notification/WDL/About；[当前设置原文](settings-presentation-current-evidence.json) | host与OS真实观察、版本/release notes、教程与完整同步 |
| /synapse/update-fw/ | 305 / 39 | 部分细查：阶段render、错误遮罩、双按钮防重复、结束/重来；[固件契约](firmware-update-current-audit.md) | 状态常量至完整服务actor、preflight/prepare/下载/硬件结果 |
| /systray/systrayv2/ | 591 / 63 | 当前[Tray契约](tray-ui-current.md)原文重验、控件/CSS索引 | 左/右宿主生命周期、动态高度、Widgets/多应用/账户真实数据 |

## Dashboard：先状态，再控件与布局

新锚点 `dashboard-main-sections` 保留xi完整类（7861 chunk的117297–130421），`dashboard-device-card-state-tree`保留z类（19894–39109），`dashboard-introduction-banner`保留ji（133175–136928）。这些作用域不是只摘导航名称：包含初始化、storage监听、门控、render与原动作处理。设备主卡的retry、预设加载、固件/重启/WDL、Xbox/PS、电池和默认打开分别留在完整class中；不能把“设备对象存在”化简成可点击ready。

既有Card/Banner精确原文已重新比对当前源，具体颜色/像素与生命周期继续由 [Dashboard契约](dashboard-ui-current.md)及其专门机器证据维护：卡宽290、内容图250×140、列表容器宽与列数是两个计算、底部50间距；名字/图像disabled、blurred与电池placeholder各有独立条件。Banner是同一源组件的两应用布局，标题、图标、按钮及关闭本地偏好不等于安装/账户查询。CSS的hover、opacity、media、动画声明全部保持原次序；本轮没有运行字体、缩放或像素对照。

## Settings：没有全页统一Save可代替这些动作

独立窗口 `ss` 的实际root在97 chunk的190482–193232。render先共用toolbar，再Software/Systray/General导航，正文分别lazy加载9302/9762/3414；selectedTab和语言变化由componentDidUpdate处理。顶层没有因本页存在就可视为host设置读取成功的分支。

Synapse `Ks`（720 chunk的80201–81361）以true/true作为两个React初值，effect再读取 `getAppAutoStart` 与 `getMinimizedOnStartUp`。父checkbox即时调用 `setAppAutoStart`、改本地state并延迟发布；子项调用另一个host setter，`disabled:!n`，关父项没有清除子项值。将它们还原成一个立即“已保存”提示会遗漏原链。notification、WDL、About的完整bn/ia/Fs锚点与原CSS收据独立保存，不能把它们合并成同一个布尔开关。

样式需分别沿根祖先核对：独立窗口Software/Systray/Widgets的列表与滚动容器，Synapse `panel_with_control`、widget间距和help/tip portal拥有不同条件。字体、标题大小、checkbox disabled以及About的SVG/社交hover由各当前证据维护；全局字体/颜色不能覆盖局部控件规则。本项目LocalStartupDraft只是当前实现的本地偏好边界，不是上述host query/setter结果，本轮没有修改它。

## Profiles：导航、游戏列表与设备目标不同

`profiles-games-devices-root`保留Ba（9449 chunk的197107–199676）。初始view来自sessionStorage历史及当前索引；切换后剪掉forward历史再追加。普通changeView遇mapping changed只记录并不直接updateView，back/forward则调用未保存提示路径；不能根据相同标题认定三个入口行为一致。

重新核实oe完整Games类：搜索按lowercase+trim，Views分别All/Linked/Removed；Linked要实际可见关联设备，Removed点击走Add。排序使用名字升/降、最后游玩时间或次数。Refresh同时请求scan并置syncing，Add tile由syncing加disabled类；“刷新”本身不证明得到游戏目录。Ga的设备列表要求实际activeProfile/profiles，IOT显示子设备有额外匹配；串号、container、pid与同游戏关联不是只比较名字。

来源布局保留Games卡290×220、关联卡240×190、Devices图片250×140，以及Games与Devices不同最小宽/正文位置。Profile下拉的目标、重命名、增删/复制、关联弹层、Import/Export的UI回调分别见 [当前契约](profiles-ui-current.md)和本页重验的content/transfer原文。原服务动作、本地草稿与文件导出不能因按钮叫Save而归成一次设备写入。

## Macro：未保存续接、菜单能力与独立编辑草稿

`macro-command-root`保留Mn（632159–640788）。Add、Import、Duplicate、Rename、Delete在原connect actions中先经lock再dispatch；选项受sharing、canReshare、shared GUID、教程/录制/setting window/OTFM等条件影响。文件input限定.xml，宏v4与旧版解析分支独立；Export克隆当前macro再生成XML并调用文件保存对话框。未知当前profile不应回退并导出首项。

Add发现当前宏未saved时挂suspendedAction并打开未保存层。`macro-unsaved-dialog` Gr（688165–689303）在显示状态通知host setForceFocus；关闭只隐藏提示。Save/Discard两按钮各dispatch自己的动作，再关层并执行原suspendedAction，不能只关闭提示。其他键盘/Text/Launch/Nested/Phased/录制设置的完整当前原文已重验；Text/Launch弹层确认与外层宏Save是两个提交阶段。它们的本地draft不能代替录制、全局快捷键、文件和设备映射的实际服务結果。

本页保留原源的可疑语句，如Mn的style键 `positoin` 及匿名storage listener删除形式；源码异常另列边界，不悄悄修正后宣称与原码一致。准确操作/布局继续由 [Macro当前契约](macro-ui-current.md)维护，本轮没有运行其测试或录制actor。

## Sophie Lite：独立7.1应用不能由产品音频页代替

人工锚点覆盖真实 `Vn→Rt→{kt,nn,un,Zn}`。view只支持audio、activation、settings、spatial-audio-upgrade，未知返回null；promo隐藏header，footer还要云promo开启、处于audio/activation且没有Spatial Audio许可。`oa`把框架登录、在线/离线、语言、pubsub、toolbar与broadcast连接到Redux与可见状态；有无surround license、virtual device及setup完成分别控制隐藏/禁用广播。

- audio先mode图，再surround switch，再output device selector。switch的disabled类取 `!virtualDevice.id || isSurroundSoundDisabled`；selector的warning有setup遮罩/虚拟设备未发现/输出名字分支，tooltip只在原dropdown与overflow/disabled条件显示。
- `bt`选择设备先比较并调用真实 `ie.setOutputDevice(e,true)`，再写user data/dispatch；其越界原判断是 `e<0 && e>=items.length`，不是正常的OR范围判断。`vt`也先调用原生surround setter，随后才用license/device/disabled门控更新Redux。不能把后半段guard说成原生操作前校验，不能用这些setter做只读验证。
- activation guest根走登录提示；非guest根走4×5输入加连字符，长度23才启用提交，提交中禁用。粘贴剔连字符，只允许短段或完整20字；空段Backspace/Delete和自动跳下一段有自己的处理。返回error恢复提交状态，只有实际devices非空才触发setup。
- 两步setup第一步Next只改step；第二步Done在 `!virtualDevice.id && devices.length>0` 时disabled，有单独的定位selector和tooltip。设置页按General→About渲染；language即时动作，auto launch与minimized分别调用host，不通过一个共用Apply。About版本源字面值是1.1.85，更新检查和1秒成功符号不等于新DLL已安装。
- promo是5段carousel；Prev首段禁用，Next末段禁用。倒数第二段请求coupon，末段显示video、trial与可选coupon；Copy采用原textarea/execCommand并有2秒状态。coupon未取得不显示真实优惠码。

样式原文明确Lite的 `.page` 600×568且受max-width/max-height约束，body `#222/#ccc`、14px、line-height1.22；mode 248×248。audio switch label为绿色 `#44d62c`、RazerF5及fallback、16px/18px。settings wrapper padding10px 20px，panel `#111`、padding20px 24px 20px 30px、20px底间距；panel标题18px。激活输入268×27、`#111`底、`#5d5d5d`边、focus绿色/error `#c8323c`；setup容器600×568、内容padding20px 30px。不能借Synapse全局16px或普通产品widget尺寸替换这些独立根。

## Sophie：Spatial Audio的许可、导航和原生操作

`xl`（925149–929030）是实际许可/设置render gate：isActiveSP为null不挂主界面，true挂Ju，false挂Onboarding；setting另挂Ll，不能把“未激活”等同于“无设备”。许可默认值来自真实缓存subscription/expiry/isDeactived字段，onMount再请求version/devices/subscription，并订阅原更新器事件。主源版本1.0.257、host版本与THX DLL版本分开。

`Nu`明确AUDIO、EQ、CALIBRATION、DEMO四个descriptor；`Yu`保存自己的导航历史并在切换后setActiveSound(-1)。AUDIO是持常驻音频根的特殊render值，其他nav由descriptor选择。`Jc`的output变更、virtual output名称相等判断、warning、两步setup与firstTimeTrial分别留证；点击Sound Properties走host系统入口，不能替换成任意本地设置页。

EQ `$i`保留10点绘图、chart与569×337 canvas叠层、mouse拖动、reset、prop变化同步以及window mouseup/mousemove的注册/卸载；本页没有因控件叫slider就抽象成同一个产品EQ。CALIBRATION `pc`把channel volume、角度/坐标、playIndex、distance与test状态分开：自动测试按1500ms步进，off/auto切换和channel更新有原回调，没有查询成功依据时不能播放或伪造校准结果。

`$u`的兑换码仍是4段maxlength5，但它额外处理左右箭头、错误code/网络状态和23字符时的fn调用，不能照搬Lite input。`cl`保存七个Onboarding页面、pricing/activation/video/trial子view及overlay条件。General设置先语言dropdown（会translateCustomProfiles），40px后自动启动树；About独立。CSS同一文件先有body/html宽1000、高689px!important与Roboto16，后有body14px、line-height1.36、`#222`；要按声明顺序与!important计算，不能挑一段当整应用统一风格。原生音频/stream/EQ设置写回属于当前完整实现范围，须按 [native边界](application-native-current.md)恢复并接通提交、响应与状态刷新；可编辑UI的存在不代表写回已经完成。

## Alisha、Ring Light及辅助应用的实际条件

Alisha的 `st`（1026838–1030829）默认只有Dashboard；任一Twitch/StreamLabs/Huya/Douyu认证为真才开放Dashboard/Alerts/Editor/Connections四tab，否则退回Dashboard。SDK分别not_run/outdated/disabled/success，警告与refresh按钮各自渲染；setting使主根隐藏并挂另一lazy层，不能把它当第五tab。原effect注销 `alisha-open-setting` 时重造匿名函数，不能宣称监听可靠移除。alerts/editor/connections的全部回调已进控件索引，尚未逐条解释灯光工作器与写入结果。

Natalie `nc`保留pod/ring/license/setting/version五hash，未知为null；`Li`的SHOW_POD、配置、恢复完成flag分别门控。localStorage默认50及finally flag不证明原生亮度/显示器查询成功。设置页General→Display&Camera→Account→Update→About是滚动section，pod控制与网页ring又不同；亮度、厚度/色温、同屏、camera properties与DRM实际链见 [Ring Light](ring-light-ui-current.md)，原生操作与本地配置防抖分开。

账户菜单N按非guest显示name、余额、Profile、Password、Logout；guest走Login，然后两者共有Feedback、可选Rating和Exit。余额未观察到显示loading，链接动作有200ms禁用/关闭节拍；onmousedown外点与window blur关层。iframe只传消息不能证明账户读取完成。应用菜单另一iframe只接受ready/非mixer失败/非断电设备，按设备/模块/其他app分组，不以source module存在冒充已安装。

Introduction Tour是Dn/xn/jn/Fn完整锚点：Synapse5段640×360 video，Chroma3段570×420 image；首步Previous guard，末步按钮变GetStarted并closeWindow，Skip也closeWindow。媒体未载入返回null而没有造成功图像。当前源码Un按pathname判断app，Fn再据常量选择steps，本页没有执行路由或声称每个host URL都命中正确集合。

Feedback的完整ps类保留分类/校验/提交中disabled、日志确认、错误/成功/下载结果render；[当前表单契约](feedback-app-current-audit.md)的UTF-16与空白规则原文已重验。Alexa保留qE安装页、Bp登录/设备码/ready视图、Yp设备开关及yE设置。Firmware保留ri阶段switch、zo已知错误类型才挂遮罩、Zo取消/继续各自首次点击后disabled，以及Wo的StartOver/Close独立状态；按钮只发送原actor事件，不能把点击解释为固件升级成功。这些对端服务仍需逐链审查。

## 证据使用与剩余工作

维护工具 [audit-application-ui-details-current.cjs](../../tools/audit-application-ui-details-current.cjs) 的 `--check` 重读当前源、验证应用图/HTML/CSS hash、全部片段及既有细节原文，并对比独立JSON。它不运行React、WebView、GPUI或DLL。机器记录的 `file_details.ui_calls` 可查每个控件props、条件祖先和子项次序，`semantic_anchors` 查完整人工作用域，`applications.css` 查selector/条件/原声明；动态class、props spread、未解析label、跨iframe/广播对端仍需沿引用展开。

接下来仍要逐页把原点击/输入→state/reducer→host/native/网络→失败/取消→卸载闭合；每种displayMode、popup及产品能力不能被24端点表替代。本页所列UI编辑/增删/Apply/Save/Cancel全部继续在接入范围；本地draft、host偏好、外部文件和设备/服务DLL持久化须分别标明。设备/服务 DLL 写回仍有实现缺口；UI 操作、提交响应与真实状态均需核实，不能用空集合/计时器/source stub 伪造成功。
