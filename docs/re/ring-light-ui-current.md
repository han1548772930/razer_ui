# Virtual Ring Light 当前源语义与 DLL 链

这是独立应用 `/natalie/`，不能以产品音频页、相机页或 Synapse 产品导航代替。来源为已固定的当前生产包，不是本轮重新查询在线版本。主 JS 的应用模块为 webpack 321；实际常量为应用版本 `1.0.137`、应用名 `virtual-ring-light`。宿主版本是4.0.827，两者分开记录。

[语义原文收据](ring-light-current-evidence.json)保存86个模块内绑定的完整原文、原始SHA-256与UTF-16范围，以及20次原生getProc声明、1099条CSS规则和17个font-face。JS来源为 `.ref/applications/natalie/static/js/main.35e04e8c.chunk.js`，CSS为 `main.fb527a4d.chunk.css`；文件和HTTP身份由静态工具核对。本文只解释已实际阅读的部分，不将整应用计为语义完成。

## 页面根与窗口

`nc.getDisplayMode()` 读取hash，去掉第一个 `#` 后，空值回落 `pod`；不是产品displayMode。`renderView()`有五个明确分支，未知hash返回null。

| hash | 实际挂载 | 初始窗口与条件 |
| --- | --- | --- |
| pod | Redux Provider → qo/Zo账户控制器 → Li控制盘 | 255×255；Ja初始化更新器；账户控制器没有user时不挂控制盘 |
| ring | Vo网页SVG补光层 | 1920×1080；这一网页层与原生ShowRingWindow是两种实现路径 |
| license | Provider → ko授权页 | 1280×720、最小同值、居中；按ba授权观察控制可见 |
| setting | Provider → go设置页 | 820×720、最小同值、居中、autoResize=false |
| version | tc版本信息 | 没有对应switch初始化Ko分支，不能套用设置页窗口参数 |

原 `Ko` 通用选项为policy6、不可调整、controlBrowserVisibility=false、autoCentralize=false、tabVisible=false，并初始化框架E。`Na`先查询实际window service clients，已有同名窗口则激活，缺失才window.open：名字分别为 `natalie`、`natalie-ring`、`natalie-setting`、`natalie-license`。Na的pod首次open写250×250，nc随后初始化255×255；保留两个阶段，不改成一个猜测尺寸。

Zo首次挂载调用getUserLoggedIn、读取当前user与ba授权，然后注册userGet/logOut/online/offline/languageChange/broadcastMessage。userGet异步查询Co，异常时r=null；失败进入授权窗口，自动启动且最小化的首次进入条件则隐藏。这里的handleGetLicense在无errorCode时，即使active_status不是成功也返回true；不能把函数名解释成已证实授权有效。卸载只明确取消broadcastMessage，其他匿名监听没有同等off证据，不能补称统一清理。

## 控制盘状态与原生观察

Li的最终render门控为 `!S || !r || !_` 则null：S来自SHOW_POD可见意图，r为读取/编辑的本地配置，_在恢复位置流程后置true。g是原生初始化准备链标志；`La().finally(...)`置g=true，其finally收到的参数不是Promise成功值，不能据 `e.Devices` 代码声称已取得实际显示器列表。

位置恢复两次Te均通过finally继续，最后设置窗口大小并置_=true；因此_也不证明屏幕查询或位置恢复成功。rt初始stroke/brightness/temp均50、isShowRing=true，是本地默认配置，不能当作原生亮度观测。rt通过nt写localStorage键 `virtual-ring-light`，stroke/temp/brightness/位置保存套300ms防抖；cloneMonitorObject的dpiScaleY原文复制dpiScaleX，不能在源语义文档中悄悄改正为实际Y。

挂载后Pa建立DLL对象；保存配置由rt.get取得，stroke/temp分别进入本地state，isShowRing与isSameDisplayWithPod分别控制开关与同屏。At从window.getMonitorInfo取得当前显示器并按deviceName数字排序；`St(name)`把数字转换为零基编号。编号、原生GetMonitorList结果与配置currentScreen属于不同输入，不能用一个替代另一个。

| 动作/观察 | 原链与分支 |
| --- | --- |
| 电源 | 普通状态pe切换U；DRM suspend时先publishCheckingDRM，确认可接续才切换；F变true时关闭ring并置U=false |
| 亮度旋钮 | Da(Z.current,value) → 原生SetScreenBrightness；g时另外rt.setBrightness本地保存；原调用没有把返回bool映射成确认提示 |
| 厚度/色温 | 本地y/w变化 → Ae → AdjustRingWindow(radius,color,thickness)；g时另存本地配置 |
| 开启补光 | le(true)查询显示器，确认te.current授权门控，算厚度/半径/颜色 → ja → ShowRingWindow，广播活动屏幕并保存屏幕；le(false)调用CloseRingWindow |
| ringMonitorChanged | 原生ffiEvent JSON → GetRingWindowMonitor → 当前索引/同屏判定 → AdjustRingWindow、亮度重读、保存屏幕 |
| displayedChanged | U为true时在2500ms与4000ms分别重开ring；原effect没有为这两个timer提供取消句柄 |
| 控制盘拖动 | 左键排除knob/power/close/setting后记录局部坐标；mousemove更新窗口bounds；mouseup恢复窗口标志、查询屏幕与保存位置 |
| 关闭按钮 | CloseRingWindow、隐藏控制盘、S=false、关闭设置页；没有直接window.close销毁pod |
| 设置页请求 | GET_SETTING_PAGE_DATA → 摄像头列表 + 延迟1000ms发送自动启动状态；不能补成实时成功回执 |

厚度实际计算：百分值>=100时使用99，范围为屏幕宽度的13%到20%；半径项按82%到75%插值后取一半，再加厚度一半。原js对screen/radius/color使用数值转换和Math.round，不能按一般灯效百分值套公式。

Ei DRM使用pubsub里的检查/回应/接管事件，按同appName、session id过滤；等待确认的Promise有1000ms成功与1100ms失败两个定时分支。尚未沿框架subscription追到远端实现，不把本地计时或来源默认值当作真实授权成功。

## 网页ring与授权页面

Vo维护厚度、颜色、路径长度、拖动、same-display等状态。接收SHOW_RING/CHANGE_BRIGHTNESS/CHANGE_STROKE/CHANGE_TEMP/CHANGE_SCREEN_EVENT/SET_SAME_DISPLAY_WITH_POD：亮度仍调用原生Da；厚度映射13%到20%；色温索引Fo数组，>=100也钳到99；屏幕移动按dpiScaleX/Y换算窗口bounds。同屏时阻止拖动；跨屏拖动结束更新配置并重读亮度。该effect注册broadcastMessage/logIn，没有展示对应统一取消逻辑，不能假设无残留监听。

ko授权页面按真实 `user.razerId` 分为兑换码表单与登录提示；中文标题仅zh-cn使用N。mount把窗口调整到1280×720、隐藏框架设置按钮、body背景black。兑换正则 `^([^\w\d]*)$` 检查无word字符的输入，禁用规则不是“必须合法产品序列号”。提交Do后根据errorCode/error/code记录表单错误，再执行Co重查；Co/Po向 `/api/license/api/v1/license/app_id/0007` 发送代理POST，payload中的实际方法为GET；Do使用同代理端点、实际POST及app_id/service_code=0007，不能把常量b=1061误用到此请求。

Po会更新框架user data的natalieSubcription/isDeactived。ba只在该subscription.active_status等于1时返回user data。这些是账户/许可状态，不是设备配置。OAuth、代理、重试Ra及服务器正文仍需继续追踪；没有执行请求。

## 设置、更新和版本

go添加Escape关闭、body背景#222、居中、请求user info/balances以及setting page data；卸载明确删除keydown，userGet匿名监听没有同等off。主内容vo按顺序挂载General、Display & Camera、Account、Update、About，目录导航以各section offsetTop滚动并按160px阈值更新活动项，不能用五个独立route代替。

- General/to：language来自Redux；自动启动/最小化由广播AUTOSTART状态观察。父项调用setAppAutoStart，子项仅E.isAutoStart为真才修改最小化并重新发布状态。语言特定PDF后缀转换单独保留。
- Display/lo：At实际查询显示器，读取rt.currentScreen.deviceName判活动屏；same-display广播到pod再保存。摄像头观察过滤devicePath含root#camera的条目；点击设备uuid广播打开原生camera properties dialog。Identify按钮在此源码只调用p重新获取显示器列表，不凭名称补造屏幕数字覆盖层。
- Account/fo：avatar/razerId/balance来自user/store，头像和名称打开profile web，余额打开account summary；logout隐藏pod、调用框架logout并关闭ring/setting。Username和默认图是fallback展示，不是读到真实账户。
- Update/Ro：处理didNotFindUpdate、didFindUpdateEx、downloadProgress、requestLaunchInstaller、downloadCancelled/downloadFailed/error；下载、取消、安装按钮依isCompleted/isDownloading/isCancelling切换。requestShutdown当前分支为空；安装与下载传到No/Wa路径，未执行也不宣称成功。
- About/so/co：版本取T、copyright固定2021，反馈先聚焦已有feedback否则框架launch；许可/隐私/社区链接各有原来源。

tc版本页并发请求四份manifest，保留fulfilled结果；显示loading，finally恢复false。该列表含 `/systray/right/` 与 `/systray/left/`，只是本应用当前源码中的版本查询目标，不能据此替换当前host的systrayv2入口；没有重新取得这些远程目标。Qo请求timeout=5000ms，错误继续抛给allSettled。

## DLL加载、声明和错误边界

实际入口Pa为 `new Ca; ha.natalie.init()`，没有传DLL参数；Ca.init默认参数为空，故此调用使用 `window.userDataDir + \\Apps\\VirtualRingLight\\VirtualRingLight.dll`。Ca也支持其他调用者传入路径；Pa这条链可以确认fallback，不能据此断言所有实例都用同一文件。通道是 `new window.FFILibrary`、setAllocator/setEventInterface/getProc/call，与middleware ConfigureFFI、host ffi-napi不同。

Ca注册beforeunload并terminate；原生onffievent解析t.ffi，displayCameraPropertiesDialogClosed调用CloseCameraPropertiesDialogMsg，再emit原始JSON供消费者。加载失败isInit=false；getProc或Initialize抛出时也false。但Initialize没有检查返回bool，未抛出后直接isInit=true，因此此flag不等于原生初始化成功。

20次getProc声明原编码保留在机器证据。GetCameraList先声明b，后又声明c，是原源码冲突；不能自动选一个生成Rust ABI。ShowRingWindow/AdjustRingWindow包装返回true表示call未抛异常，没有消费call返回值；CloseRingWindow包装即使调用未抛也固定返回false。GetMonitorList JSON.parse异常没有本地catch。必须按真实行为和错误传播接线，不能用包装器返回值伪造硬件成功。

RzSparkle为第二条原生链：Ja根据框架版本选择用户目录或RzSparkle.dll，关闭自动更新检查/可shutdown并设appcast/relaunch。Wa.init成功路径末尾是表达式1，没有return；Ja比较 `0===Wa.init(Ya)` 的fallback条件不能当作已证明新旧版本检测有效。底层FFILibrary编码解释、DLL文件获取/PE身份和正文、更新服务内部未在本收据恢复，不执行初始化/更新。

当前host兼容性另存9个原文锚点：4.0.827 preload暴露的是apiElectron对象，没有该文件直接expose的FFILibrary；旧doDLLAction/Async仅记录deprecated日志，新的主/子进程FFI使用不同入口。网页原At调用window.getMonitorInfo并消费deviceName数字，而host的apiElectron.getMonitorInfo引用未在其actionEnum定义的GET_MONITOR_INFO；constants同样缺少该键，main却有该case，返回deviceName为Electron display.id。原网页、当前host接口、潜在框架适配是三层，不能把原代码中“调用了getMonitorInfo”写成已经证明三层正常衔接；是否存在其他适配/意外undefined分支的运行效果仍未知，不能在静态阶段宣称可运行。

## 样式实际来源

CSS存在大量共享规则，必须按顺序核对覆盖：body/html在offset17588声明1000×688!important，但92626又置width/height为unset，其中height也是important；不能仅取第一个规则推窗口尺寸。默认color#ccc、Roboto/sans-serif、16px；RazerF5声明100/300/400，Roboto有400/500不同unicode范围。字体本体/加载/fallback与最终栅格仍未验收。

| 实际选择器 | 原布局/颜色 |
| --- | --- |
| .circle-pod | left4px、240×240；普通pod窗口255×255不能推内容填满 |
| .circle-pod .circle-wrapper | 240×240、圆形、#222、绝对定位z3及原阴影 |
| .circle-pod .top-control | 120×130、右上、#222、z1，上/下右圆角13% |
| .circle-pod.off 各旋钮 | opacity .4、pointer-events none；电源SVG相应#707070 |
| .circle-pod .title | left90/top10、width58；电源居中transform translate(-50%,-50%) |
| .bar | #191919、36px；setting_bar另固定top35px |
| .license-screen | RazerF5、padding100px 375px 180px、居中flex；title#ccc、description16px |
| .setting-layout .no-name-bar | height100vh、overflow auto、top20px；section widget后续覆盖560px |

旋钮Vt的圆弧/角度计算、视觉路径、knob与tooltip原文已保存：16px track、6px stroke、240px容器、禁用class、绿#44d62c/#2d5925、红/橙/白/青/蓝色温渐变；细分边界、hover命中与拖动角度尚未全部逐数值审查。CSS层叠、外部媒体和字体资源不因规则索引完成而计为视觉一致。

## 维护与尚未完成

运行 `node tools/audit-ring-light-current.cjs --check` 静态复核文件/HTTP身份、module321、绑定全文、getProc及CSS/font声明。工具只解析源数据；未运行应用、下载JS、测试、DLL或任何更新器。

仍须继续追：框架E/tt与当前host兼容路径、所有modal与共享控件的每个分支、license重试/离线/过期与DRM所有交错、Redux reducer及余额/更新所有错误状态、资源实际挂载/字体层叠、DLL PE及正文和服务协议。本应用完整语义完成仍为0；实际UI接入也独立记录。
