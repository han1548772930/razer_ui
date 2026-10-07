# Dashboard 当前界面与设备卡契约

当前来源为 `.ref/applications/synapse/dashboard/` 的22534/Pi/xi/z/G/V/K/ji、29228/pV及实际匹配的CSS。设备卡、介绍横幅和模块入口已在正式主页面挂载；状态观察、本地操作和服务写回保持分离。

## 页面、设备名称与入口

双应用介绍横幅复用当前ji/Pn的共同组件：1220–2500宽、最低531高、42/24标题、100图标、140箭头、源CTA和200ms透明度。关闭存本地偏好，两个tour进入真实页面；附带profile-migration toast仍未接。

八个Dashboard入口为Macro、Linked Games、Alexa、Feedback、Wi-Fi加入、Profile Migration、Armory、Tour，进入各自已实现根。Armory特性/维护状态需要真实观察。设备名沿当前语言→baseProductName→英语→空回退，edition行只显示实有edition_name。配置行用active ID精确查找，不回退首项；subDevices.isShownOnUI、notShowProfileName、单配置例外和autoSwitch后缀各自处理，断电不挂配置行。

空卡透明底且使用NO_DEVICE_FOUND；count/显示序号、多子设备角标和断电/待机的名字/图像透明度按源条件。容器与列数分离：>600时最低620，最大910/1220/2460随条件，列数用Pi.getMaxColumns，拖动横向上限为实际容器宽减290。Dashboard下间距50，收展箭头#999/hover#fff。

## 卡片状态与操作

- 非ready图像自身blurred(.3)、图像容器disabled、名字组disabled是不同条件；显式false的supportsStandbyMode不走缺省standby/off。
- Xbox Controller/Headset/Earbuds和PS普通/Arcade保留各自原图、145/250×140几何、主机快捷键行；PS不加普通blurred，主机名字单独乘.3。standby-off、Xbox、PS原图均已准备。
- restart/canceled/ready/error及waiting-off不显示spinner；error/canceled/noAliveSign有retry。离线retry按源pointer-events:none交给外卡拖拽。源2秒spinner的角度0/180/720、弧长10%/50%/10%、方端点用原生几何动画呈现，减少动画可关闭。
- presetLoading只取deviceState.presetLoading.value，显示半透明黑全卡、底部250×140内容与5px进度；宽度100ms ease-in，三点600ms及150/300ms错峰。mixer_system_check_failed独立失败遮罩并阻断默认打开。
- restart/固件/WDL分别显示源提示；WDL有固件信息时left39。固件只匹配原firmwareUpdateDevices的PID+serial+needsUpgrade，不用合并列表推测。
- 内部操作记录overrideAction，只有短拖拽释放才执行；键盘阻止重复激活外卡。minRequiredVersion、PS/Xbox逻辑短路，preset/off/standby/init-fail/updating阻断默认打开。固件导航Devices & Modules，WDL导航Settings，restart只形成通知请求。

## 电池观察与显示

Dashboard使用独立pV分档，10–19向下取档，非充电-1同时隐藏数值/图标；充电、暂停、错误、断电、showBatteryValue/hideBatteryIcon/isBatterySupported/isExternalBatt分别控制。只有ready/waiting将powerStatus接纳为电池显示状态，离开后保留已接纳值；spinner仍看原始powerStatus。

首次挂载及符合条件的off→on有2秒-%占位。计时器只结束占位，不修改电量/连接/安装状态。普通图标26、standby24；通用断电mask用26区域，Xbox固有26、PS固有24且左上定位，不把background-size规则误用于mask。tooltip按实际高度居中，音频看category，主机提示不被普通断电覆盖，语言空白按normal/pre-line处理。

## 观察边界与剩余项

Device.dashboard为可选源展示投影，Device.sub_devices保留真实对象。它们不是完整wire适配器，未接实时reducer/storage桥；不能靠目录、计时器或本地页面存在制造noAliveSign、WDL、固件、安装或预设进度。powerStatus缺level、对象身份替换但字段不变、storage removal/clear和历史相关standby仍缺完整表达/更新。

retryInstall/resuscitate/restart notification没有服务adapter，点击报告请求未发送并保持观察。固件导航的sessionStorage标志和进入后平滑滚至首行仍缺。推荐/合作优惠数据条件、≤600与宿主最小宽、复杂拖动中断、组/tooltip层级、警告宽度、mask重复边缘仍需完整验收。缺嵌入普通设备图时保留尺寸槽；仅真实加载失败才可走源0/0图回退。

字体normal行高、中文fallback、pre-wrap/shrink-to-fit、组合状态提示裁剪、缩放/焦点/指针和动画没有运行验收。已接入状态树不能因此计为完整设备或真实读取成功。

## 保留证据

- [设备字段/基础布局](dashboard-device-current-evidence.json)：`audit-dashboard-device.cjs`。
- [状态树/电池生命周期/原始素材](dashboard-card-state-current-evidence.json)：`audit-dashboard-card-state.cjs`、`dashboard_card_assets.py`、`prepare-dashboard-card-assets.py`。
- [共同介绍横幅](app-introduction-banner-current-evidence.json)：`audit-app-introduction-banner.cjs`。

16个状态SVG逐字节复制当前源，PS原AVIF转换为RGBA PNG并保留输入输出摘要；资源状态以当前素材及专门资源收据为准，保留原取证数据。维护核对为静态来源/资源/格式及允许的类型检查，未运行应用、测试、安装器、厂商JavaScript或DLL。
