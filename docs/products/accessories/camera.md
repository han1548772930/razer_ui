# 摄像头：预览、设置与原生生命周期

摄像头的可见视频、设备属性、配置档和保存状态来自不同通道。预览 `Playing` 只确认媒体流开始，不能证明图像属性读取成功；写入函数返回、配置写盘和设备闪存保存也不是同一个结果。实际页面树与全部入口见 [3587](products/3587.md)、[3589](products/3589.md)、[3590](products/3590.md)、[3592](products/3592.md)、[3594](products/3594.md)、[3595](products/3595.md)、[3596](products/3596.md)。

## 3587/3589/3590 的摄像头生命周期

三款挂载 Customize，分别选择 `rzCamKiyo`、`rzCamKiyoPro`、`rzCamKiyoX`，跳过通用 HID 设备模式切换。原版在对应原生包装器加载后调用 `startCamera`，锁屏恢复使用重载分支。这是原版摄像头初始化链，不能由 `connectDevice()` 恒定返回的 `{result:true}` 证明设备就绪。

3587 的完整包装器见 [rzCamKiyo](../../../.analysis/inputs/frontend/accessories-mw-3587-current/raw/apps.razer.com/synapse/products/3587/mw/rzCamKiyo.f06094a76b2f0559cebc.js)，3589 见 [rzCamKiyoPro](../../../.analysis/inputs/frontend/accessories-mw-3589-current/raw/apps.razer.com/synapse/products/3589/mw/rzCamKiyoPro.9d1dcc13617e46bf46f9.js)。实际操作包含亮度、对比度、饱和度、白平衡/自动开关、对焦/自动开关、缩放、Windows 摄像头属性页、固件和序列号。3587 的缩放明确只接受 100/110/120/130/140；不能把连续缩放直接送入该方法。3589 额外具有 HDR、HDR mode 和 FOV；FOV=0 只写一次 LDC，FOV=1/2 先写关闭，第一次返回真后再写开启。

原版按逐级缩减的导出集合兼容旧 DLL，最多十次 `DeviceInitialize`，每次失败间隔 100 ms。`isInit` 在 ConfigureFFI 成功后、DeviceInitialize 完成前已置真，因此该字段不是设备初始化确认。非空原生序列号返回会先标成功，再解析 JSON；解析异常仅填空串是源码错误处理，不能把空串标为实测序列号。媒体枚举先按 VID/PID 与指定 Chromium deviceId 匹配，旧包装器随后存在按音视频 groupId 的候选路径；容器身份不能由标签或候选推断。

原生属性页关闭事件只在 `containerId` 相同的实例内转发。`terminate` 先移除该监听，再在已初始化时调用 DeviceTerminate 并清 `isInit`；`destroy()` 调用但不等待异步 terminate。beforeunload 再触发 terminate 并移除自身处理器。这些是当前 JS 的释放顺序，不是对全部原生线程退出的证明。3590 的独立包装器在 [6120 chunk](../../../.analysis/inputs/frontend/accessories-mw-3590-current/raw/apps.razer.com/synapse/products/3590/mw/6120.1c9140951d7633a5df88.js)内，不能仅因独立命名 chunk 缺失而认为该适配器不存在。

## 3592 的独立高级摄像头

3592 使用 [rzCamKiyoProUltra](../../../.analysis/inputs/frontend/accessories-mw-3592-current/raw/apps.razer.com/synapse/products/3592/mw/rzCamKiyoProUltra.1daa9c2dbea30936c698.js)，挂载 Camera/Image 而非旧款 Customize。标准属性之外的高级曝光、HDR、降噪、视野、镜头处理等操作必须由该产品自己的页面 props、配置和原生方法决定。注册 `isAdvancedCamera` 只是选择调度链，不能证明所有 Kimberly 新款处理控件都应出现；本产品的 UI 不继承 3594/3595 的自动构图、快门和闪存重置声明。

完整包装器的实际导出还包括 MicNoiseSuppression、SaveUVCExtCmdToFlash、AutoFocusMode/HDRBrightness、Watermark、ISO、FDAF、AutoFramingSetting、两类 DynamicNoiseReduction、Mirror、Metering、Shutter、EV、AutoFocusLowSensitivity、RestoreDefaults、adaptive/fixed MJPEG 和 StreamingStatus。标准属性 getter 将非空 JSON 解析成对象；ISO、metering、快门、EV、降噪与 MJPEG 使用各自 long/int/bool 结果，不能用一个 JSON decoder 处理全部 getter。未初始化时部分方法返回 0/false/空对象，这些是包装器 guard，不是设备实读。

`SetFieldOfView(0)` 直接写 LDC；1/2 先关闭，返回真才开启。`setLDC` 固定第三参数 0；save 写 `SaveUVCExtCmdToFlash(container,true)`，与 RestoreDefaults 是两条操作。`getChromiumDeviceId/getContainerIdFromDeviceId` 将当前 origin 传给原生身份转换。媒体枚举匹配 VID=1532/PID=0e08 和给定 deviceId，未匹配时仍含 audio/video groupId 候选路径；后者不能代替容器匹配确认。

导出兼容数组有两组：完整摄像头接口，以及 Chromium/container 身份转换扩展。原版 ConfigureFFI 失败时缩减后一组重试；成功后先置 isInit，再最多十次 DeviceInitialize、每次失败等 100 ms，最后注册原生事件。全部初始化尝试失败并不自动清 isInit。属性页关闭事件仅转发同 containerId；terminate 先移除监听，再调用 DeviceTerminate 并清 isInit，destroy 和 beforeunload 不等待 terminate。原生函数存在不证明页面挂载，也不证明调用方消费了失败值。Ultra 的原生协议仍须由该 DLL 自己的完整函数与消费者闭合，不能用 Kimberly 的虚表证明本 DLL 完成。

## 3594 V2 Pro

实际挂载从 createRoot 经 READY 路由进入产品，GU 只挂载当前 Camera/Image/Processing 子页；预览、产品状态和底部保存所有者继续保留。Camera 的 source/resolution 所有者见 [main.bb246bb0.js](../../../.analysis/inputs/frontend/accessories-3594-current/raw/apps.razer.com/synapse/products/3594/ui/static/js/main.bb246bb0.js) 字节 `[6979880, 6986798)`；实际预览 `[6989494, 6997262)`，Image `[7007360, 7011221)`。

视频源由本机枚举得到。原设备源带当前产品约束，已安装的本地虚拟源使用不带这些限制的约束；分辨率受真实 caps 上限限制，再从可用声明项选择。修改 source、分辨率或帧率持久化配置并重新启动预览。设备实例、流代次和完成通知要保持关联：旧的媒体请求返回不能接管新预览，关闭预览、失焦或卸载时停止对应流及其轨道。

页面在预览启用、窗口有焦点且 source 已知时读取一次标准属性缓存，形成设备与配置差异提示。失焦或关闭预览清除该次读取标记；单独更改 source/resolution 不等于重置标记。当前缓存由 11 个真实标准 getter 形成，不能用本地默认配置凑出实测对象。CAMERA 选择把读取结果合入 view/custom image，保留 ISO/快门伴随值，同时同步旧 `cameraMode/cameraValue`；SYNAPSE 选择保留配置意图并重放。提示处理、session 标志与延迟缓存是本链的一部分。

Image 内置预设经 reducer 和本地 profile 进入标准摄像头写入。编辑状态是乐观草稿；Save 在发 `ON_SAVE` 前先清 dirty，因此 dirty=false 不表示闪存保存成功。Reset 与 Save 不能合并：前者发默认设置意图，后者走保存链，原生固件恢复另有独立应答。

HDR 动作先更新 loading/dirty 与配置，再通过停止锁停摄像头，调用 SetHDR；请求启用时继续 SetHDRBrightness，之后广播重启，预览侧使用 3000 ms 重启延时。第一次返回 false 与抛异常的控制流不同：前者仍可能执行后续 brightness；异常中断剩余链，源码没有通用回滚。自动曝光且预览启用时每 3000 ms 查询 ISO，再按源码对数换算进入 reducer；记录日志不能替代 getter 结果。

view mode 快捷键先保存并注册本地快捷键，再由事件广播 viewMode；真正 zoom/pan/tilt 由 Camera 页已挂载的 pan effect 消费。当前在 Image 页收到 viewMode 不能直接算设备视图已写入。共享包虽含 MJPEG/LDC/watermark/autoframing/focus mode/lighting/sharpness/gain 组件，3594 本次实际挂载链没有全部使用它们；bootstrap 能力声明与可见控件分开判断。

## 3595 与 3596 的共同链和差异

共同 Camera/Image 页沿本机 Media Foundation/容器枚举获取设备，标准属性仍走各自属性读取。3595 的 Camera `[5365123, 5366002)`、source/resolution `[5357644, 5364562)`、曝光 `[5259048, 5262583)` 见 [main.19c1ceba.js](../../../.analysis/inputs/frontend/accessories-3595-current/raw/apps.razer.com/synapse/products/3595/ui/static/js/main.19c1ceba.js)。3596 对应 [main.0e57873f.js](../../../.analysis/inputs/frontend/accessories-3596-current/raw/apps.razer.com/synapse/products/3596/ui/static/js/main.0e57873f.js) Camera `[5355385, 5356170)`、预览 `[5357564, 5365330)`、Image `[5371496, 5375357)`。

| 边界 | 3595 V2 | 3596 V2 X |
| --- | --- | --- |
| 分辨率选择 | 通过真实 caps 限制声明选项 | 绕过该 caps 探测分支，仍使用本产品声明选项 |
| 设置可编辑条件 | 允许关闭预览时编辑，另受 loading/第三方源限制 | 预览关闭、busy 或 Windows 属性页打开时禁用 |
| 曝光 | 扩展 UVC；离散快门档、固定 -3…3 EV、metering 的挂载后 -1 初始化与 lighting | 标准手动滑块标签 -10/-2，或自动测光；无扩展曝光声明 |
| 手动缓存 | 对标准 exposure 的符号表示作掩码处理，ISO 仍独立 | 保留该值的符号；不能借用 V2 掩码 |
| 自动构图大小 | balanced=0/far=1/near=2；speed balanced=0/slow=1/fast=2 | bootstrap 未注册该配置；缺项不发 I/O |
| ISO 原生调用 | base ISP 的真实读写 | 该版本 X 虚表对应 ISO 读写槽均为 no-op，不能算设备写入 |
| device mode | 按实际模式链 | `fwNotSupportDeviceMode`，不追加通用模式写入 |

完整曝光动作包含标准 auto/manual mode 与型号自己的 ISO、metering、快门、EV；不能仅改 UI 一个数值。手动差异选择后，即使缓存快门为 0，也按源码安排 500 ms 的专有快门写入并保留伴随值。部分曝光控件为 leading throttle：先发前一完整对象，再编辑本字段；实现时不能误写成 trailing 只发最终单字段。

Image 内置预设由 `DEFAULT_SETTINGS` 与 profile 的 `userSettings` 构造。第一次编辑生成 custom 并保留基线，后续 custom/AWB 编辑更新基线；颜色预设身份、当前设备属性和编辑缓存不可混合。Focus 的 Center/Face 以及 V2 的 ExposureLighting 根据真实固件版本选择整对象写入；再次点击已选项仍可能发动作，不能擅自去重。视图快捷键的实际硬件消费者仍是已挂载的 Camera pan effect。

Reset 使用 300 ms debounce，按当前产品顺序发送默认意图，只重置选中的默认图像预设，并在 500 ms 后更新缓存；缺少 ldc 字段时传 false 不代表原 profile 具有该字段。V2/X 不执行 FWRestoreDefaults，不能因共享包装器有导出就新增固件恢复写入。

## Windows 属性页和原生返回边界

原生属性页通过 base filter 查询 COM 页面接口与 IUnknown，在独立线程建立临时宿主窗并调用 OleCreatePropertyFrame。结束时释放页 CLSID 内存、窗体和 COM 引用、恢复线程 DPI 上下文，最后带 containerId 发布 `displayCameraPropertiesDialogClosed`。关闭方法发送 WM_CLOSE、处理线程 join；显示/关闭导出的固定真返回只表示入口执行，不证明窗口已实际出现或属性已保存。

3595/3596 当前 GenericCamera 1.0.35.0 的属性链 RVA 包含 `0x4cc90`（属性页）、`0x4cf70`（线程）、`0x4d130`（关闭）、`0x3e130`（事件），暴露入口为 `0x5ec10/0x5ecf0`。该事件在 UI 中清除“属性页打开”状态，不能显示保存成功。base ISP 链在互斥内核对实例仍存在，经 IKsTopologyInfo 节点发送、间隔 10 ms 读响应；设备删除、HRESULT 失败和 ISP busy/parameter/preview 错误保留各自失败。3596 的 DNR 写入需要先查询伴随参数，再写本值，不能只迁移可见滑块。

原生版本不能混用：3594 manifest 指向 GenericCamera **1.0.36.1**，3595/3596 为 **1.0.35.0**。后者 base 虚表 RVA `0xb84b8`，X 为 `0xb8d20`，X ISO 两槽指向 `0x1ff40`。具体导出、槽字节、完整伪代码/汇编与原件身份见 [摄像头完整分析](../../../.analysis/results/frontend/accessories-camera-local-pages-audit.json)。其捕获也记录 Hex-Rays 对 `0x870c0` 的一次 call-analysis 失败；不能宣称所有函数伪代码完整。

## 本地范围与验证状态

已安装虚拟摄像头的本机枚举、用户打开已安装程序、Windows 属性页与真实摄像头设置分别沿各自的本机调用链处理。

本文是当前源码、选定原生函数和调用消费者的功能说明。所有 PID 的 Rust 独立实现、全部原生间接调用闭合、UI 接线、模拟验证和真实摄像头验收分别记录；本文没有执行摄像头、媒体捕获、DLL、JavaScript 或主程序。
