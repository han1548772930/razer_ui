# 摄像头：预览、设置与原生生命周期

摄像头的可见视频、设备属性、配置档和保存状态来自不同通道。预览 `Playing` 只确认媒体流开始，不能证明图像属性读取成功；写入函数返回、配置写盘和设备闪存保存也不是同一个结果。实际页面树与全部入口见 [3587](products/3587.md)、[3589](products/3589.md)、[3590](products/3590.md)、[3592](products/3592.md)、[3594](products/3594.md)、[3595](products/3595.md)、[3596](products/3596.md)。

## 实际页面树和生命周期所有者

| 产品 | 当前根组件挂载的导航顺序 | 页面外继续持有的所有者 |
| --- | --- | --- |
| 3587 / 3589 / 3590 | Customize、Help | Customize 内先预览/对焦，再 Image；展开预览是独立本机窗口 |
| 3592 Ultra | Camera、Processing、Image、Help | 产品根中的预览、配置差异处理和配置档状态；非 Help 显示底栏 |
| 3594 Pro | Camera、Processing、Image、Help | 预览、配置差异处理、首次说明和产品状态；非 Help 显示 Reset/Save 底栏 |
| 3595 V2 | Camera、Processing、Image、MIC、Help | 同上；MIC 是设备状态，独立于所选摄像头 profile |
| 3596 V2 X | Camera、Processing、Image、Help | 同上；没有 MIC 页，首次说明使用本型号分支 |

这里区分根组件、活动子页和共享组件：切换现代摄像头子页只替换 `renderView()` 的结果，预览和设置差异 owner 仍在根中。切到 Help 时根组件只给预览传 `display:none` 并移除底栏，没有据此卸载预览或终止设备实例。3592 根 `BG` 为 `[5381502,5392581)`；3594 `GU` 为 `[7029106,7043677)`；3595 `Vh` 为 `[5410958,5425542)`；3596 `sh` 为 `[5393539,5406689)`。后面三者的完整 owner 与实际字节见[相机页面捕获](../../../.analysis/results/frontend/accessories-camera-local-pages-audit.json)。隐藏状态不等于收到视频停止应答，页面切换也不是物理设备断开。

3592/3594/3595/3596 的当前 CSS 都为实际高级页面定义 400 px 设置列、`27px 20px` 内边距、`#111` 背景；左右分隔块上下间距各 20 px、1 px `#222` 边框。视频占余下宽度，底栏高 45 px、左侧宽 400 px。旧款实际视频元素是 520×292，`.main_preview` 高 292 px、背景 `#222`。新版 CSS 仍含旧款样式，不能由这些规则的存在推定旧款组件也被挂载。

## 3587/3589/3590 的摄像头生命周期

三款挂载 Customize，分别选择 `rzCamKiyo`、`rzCamKiyoPro`、`rzCamKiyoX`，跳过通用 HID 设备模式切换。原版在对应原生包装器加载后调用 `startCamera`，锁屏恢复使用重载分支。这是原版摄像头初始化链，不能由 `connectDevice()` 恒定返回的 `{result:true}` 证明设备就绪。

3587 的完整包装器见 [rzCamKiyo](../../../.analysis/inputs/frontend/accessories-mw-3587-current/raw/apps.razer.com/synapse/products/3587/mw/rzCamKiyo.f06094a76b2f0559cebc.js)，3589 见 [rzCamKiyoPro](../../../.analysis/inputs/frontend/accessories-mw-3589-current/raw/apps.razer.com/synapse/products/3589/mw/rzCamKiyoPro.9d1dcc13617e46bf46f9.js)。实际操作包含亮度、对比度、饱和度、白平衡/自动开关、对焦/自动开关、缩放、Windows 摄像头属性页、固件和序列号。3587 的缩放明确只接受 100/110/120/130/140；不能把连续缩放直接送入该方法。3589 额外具有 HDR、HDR mode 和 FOV；FOV=0 只写一次 LDC，FOV=1/2 先写关闭，第一次返回真后再写开启。

原版按逐级缩减的导出集合兼容旧 DLL，最多十次 `DeviceInitialize`，每次失败间隔 100 ms。`isInit` 在 ConfigureFFI 成功后、DeviceInitialize 完成前已置真，因此该字段不是设备初始化确认。非空原生序列号返回会先标成功，再解析 JSON；解析异常仅填空串是源码错误处理，不能把空串标为实测序列号。媒体枚举先按 VID/PID 与指定 Chromium deviceId 匹配，旧包装器随后存在按音视频 groupId 的候选路径；容器身份不能由标签或候选推断。

原生属性页关闭事件只在 `containerId` 相同的实例内转发。`terminate` 先移除该监听，再在已初始化时调用 DeviceTerminate 并清 `isInit`；`destroy()` 调用但不等待异步 terminate。beforeunload 再触发 terminate 并移除自身处理器。这些是当前 JS 的释放顺序，不是对全部原生线程退出的证明。3590 的独立包装器在 [6120 chunk](../../../.analysis/inputs/frontend/accessories-mw-3590-current/raw/apps.razer.com/synapse/products/3590/mw/6120.1c9140951d7633a5df88.js)内，不能仅因独立命名 chunk 缺失而认为该适配器不存在。

三款 Customize 的实际 `$S` owner 分别为 `[5090438,5090740)`、`[5071721,5072023)`、`[5074262,5074564)`；顺序是 `wS` 摄像头/预览，再 `JS` 图像。3587/3590 只传 `supportsFocus:true`，3589 还传 `supportsFieldOfView:true` 和 `supportsHDR:true`。3589 的完整预览 owner `YS` `[5051000,5060780)` 包含预览开关、自动/手动对焦、HDR、HDR mode、三档 FOV、展开/收起和重连；不能把这些动作归为一个静态视频框。

| 旧款控件 | 3587 | 3589 | 3590 |
| --- | --- | --- | --- |
| 手动对焦 | 0…255，step 1 | 0…600，step 1 | 0…255，step 1 |
| 白平衡 | 2000…7500，step 10 | 2000…7500，step 10 | 2800…6500，step 1 |
| 亮度 / 对比度 / 饱和度 | 各 0…255，step 1，滑块和步进器 | 同左 | 同左 |
| HDR / FOV | 不挂载 | HDR 开关、Dark/Bright mode；窄/中/宽分别写 2/1/0；HDR 开时显示 1080p 30fps，关时 1080p 60fps | 不挂载 |

图像组件还持有预设、自定义基线、自动白平衡与设备/配置差异弹层；其完整 owner 分别为 `[5083245,5090437)`、`[5064528,5071720)`、`[5067069,5074261)`。预览关闭、loading、第三方占用和 Windows 属性页状态影响编辑；自动白平衡打开时禁用手动数值与渐变滑块。选择 CAMERA 与 SYNAPSE 各自提交对应完整数据，不是关闭弹层就算设置相同。比较 effect 在部分白平衡相同分支直接 `return` 整个 effect，这项原版行为也不能擅自解释成全部字段比较通过。

旧款预览用 VID/PID 与已解析设备 ID 构造约束；同型号多实例时再请求真实硬件 ID。停止递增请求代次、停止已有 track 并清 `srcObject`；迟到的 `getUserMedia` 返回会停掉刚返回的流。失焦、锁屏和预览关闭进入自己的停止链，解锁使用 reload 分支。展开先停止内嵌预览，再按 container 打开独立窗口；窗口关闭通知与卸载清理调用折叠 owner。原版该卸载回调本身只调用展开窗口关闭链，不能将其描述为已证明所有 track、监听和原生线程排空。HDR/FOV/Reset 有停流、loading 和 1000 ms 等待；模式重配和预览恢复应按实际返回消费者逐项测试。

## 3592 的独立高级摄像头

3592 使用 [rzCamKiyoProUltra](../../../.analysis/inputs/frontend/accessories-mw-3592-current/raw/apps.razer.com/synapse/products/3592/mw/rzCamKiyoProUltra.1daa9c2dbea30936c698.js)，挂载 Camera/Processing/Image 而非旧款 Customize。标准属性之外的高级曝光、HDR、降噪、视野、镜头处理等操作必须由该产品自己的页面 props、配置和原生方法决定。注册 `isAdvancedCamera` 只是选择调度链，不能证明所有 Kimberly 新款处理控件都应出现；本产品的 UI 不继承 3594/3595 的自动构图、快门和闪存重置声明。

[当前 UI 原件](../../../.analysis/inputs/frontend/accessories-3592-current/raw/apps.razer.com/synapse/products/3592/ui/static/js/main.323f0a8a.js)中，Camera `eG` `[5341858,5342156)` 依次挂载 view、focus、exposure，没有新款 Camera 页的 source/resolution 选择块。view `[5339463,5341291)` 为 1…4 倍、step 0.1；LDC 开启且分辨率为 4K 30FPS 或 1440p 30FPS 时，一并禁用 zoom、pan/tilt、预设和快捷键并显示说明。focus `[5249862,5251820)` 的手动范围是 1…450，自动模式含对焦 mode 和 tracking；lighting 在该组件中独立出现，不能套用新款禁用 lighting 的 props。exposure `[5246370,5249855)` 包含 Reset、ISO、离散快门、metering、补偿及 1000 ms leading throttle；特定 metering 的补偿下限为 -1，其他为 -3。

Processing `[5356038,5358150)` 挂载 resolution/MJPEG、HDR、LDC、低光补偿、2D/3D DNR；MJPEG `[5352885,5356037)` 包含自动开关、Performance/Quality 或手动 10…90。Image `[5362868,5366820)` 的实际调用是 `DG({})`，因此 **watermark 默认启用挂载**，包含开关及位置网格；sharpness/gain 默认不挂载。该页还挂载图像预设、anti-flicker、mirror 和 Windows 属性页。这里与 3594/3595/3596 根显式 `supportWatermark:false` 的行为不同。

完整包装器的实际导出还包括 MicNoiseSuppression、SaveUVCExtCmdToFlash、AutoFocusMode/HDRBrightness、Watermark、ISO、FDAF、AutoFramingSetting、两类 DynamicNoiseReduction、Mirror、Metering、Shutter、EV、AutoFocusLowSensitivity、RestoreDefaults、adaptive/fixed MJPEG 和 StreamingStatus。标准属性 getter 将非空 JSON 解析成对象；ISO、metering、快门、EV、降噪与 MJPEG 使用各自 long/int/bool 结果，不能用一个 JSON decoder 处理全部 getter。未初始化时部分方法返回 0/false/空对象，这些是包装器 guard，不是设备实读。

`SetFieldOfView(0)` 直接写 LDC；1/2 先关闭，返回真才开启。`setLDC` 固定第三参数 0；save 写 `SaveUVCExtCmdToFlash(container,true)`，与 RestoreDefaults 是两条操作。`getChromiumDeviceId/getContainerIdFromDeviceId` 将当前 origin 传给原生身份转换。媒体枚举匹配 VID=1532/PID=0e08 和给定 deviceId，未匹配时仍含 audio/video groupId 候选路径；后者不能代替容器匹配确认。

导出兼容数组有两组：完整摄像头接口，以及 Chromium/container 身份转换扩展。原版 ConfigureFFI 失败时缩减后一组重试；成功后先置 isInit，再最多十次 DeviceInitialize、每次失败等 100 ms，最后注册原生事件。全部初始化尝试失败并不自动清 isInit。属性页关闭事件仅转发同 containerId；terminate 先移除监听，再调用 DeviceTerminate 并清 isInit，destroy 和 beforeunload 不等待 terminate。原生函数存在不证明页面挂载，也不证明调用方消费了失败值。Ultra 的原生协议仍须由该 DLL 自己的完整函数与消费者闭合，不能用 Kimberly 的虚表证明本 DLL 完成。

## 3594 V2 Pro

实际挂载从 createRoot 经 READY 路由进入产品，GU 只挂载当前 Camera/Image/Processing 子页；预览、产品状态和底部保存所有者继续保留。Camera 的 source/resolution 所有者见 [main.bb246bb0.js](../../../.analysis/inputs/frontend/accessories-3594-current/raw/apps.razer.com/synapse/products/3594/ui/static/js/main.bb246bb0.js) 字节 `[6979880, 6986798)`；实际预览 `[6989494, 6997262)`，Image `[7007360, 7011221)`。

视频源由本机枚举得到。原设备源带当前产品约束，已安装的本地虚拟源使用不带这些限制的约束；分辨率受真实 caps 上限限制，再从可用声明项选择。修改 source、分辨率或帧率持久化配置并重新启动预览。设备实例、流代次和完成通知需要分别关联；独立实现必须拒收旧请求并完成流/track 清理。原版预览的卸载 effect 清计时、移除相应监听并清播放标志，不能仅由此声称已确认每个在途媒体请求及原生线程退出。

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

## 现代型号的实际控件分支

以下为各根本次实际传入的 props 与子页本体，不是共享包的潜在能力并集。

| 控件 / 页面 | 3594 Pro | 3595 V2 | 3596 V2 X |
| --- | --- | --- | --- |
| zoom / pan-tilt | 1…4，step 0.1；pan/tilt 最大绝对值 10 | 1…4，step 0.1；最大绝对值 20 | 1…1.4，step 0.01；最大绝对值 20 |
| 自动构图 | view 明确 `supportAutoFrame:false` | 相同 | 相同 |
| 自动对焦 | 不挂载 mode；有 tracking、无 lighting；手动 1…450 | V2 mode 受固件比较 gate 控制；有 tracking、无 lighting；手动 1…450 | V2 mode 受固件 gate；无 tracking/lighting；手动 0…440 |
| Processing | HDR 开关及 mode，无 MJPEG；低光带说明；只挂载 3D DNR | MJPEG 自动开关、HDR 及 mode；低光带说明；2D+3D DNR | 不挂载 HDR/MJPEG；低光无说明；2D+3D DNR |
| Image | sharpness/gain/watermark 均显式 false；保留预设、亮度、对比度、饱和度、白平衡、anti-flicker、mirror、Windows 设置 | 相同 | 相同 |
| MIC | 无 | 双开关草稿与设备重启动作 | 无 |

3595 Processing `[5379183,5380764)` 传入的 `disableHDRSwitch:true` 名称容易误导：实际 HDR/MJPEG 组件 `[5376642,5379182)` 仅用它跳过 MJPEG 的 Performance/Quality 或手动质量子选择，MJPEG 自动开关、HDR 开关和 HDR mode 都仍被挂载。3594 则传 `supportMJPEG:false`，跳过整个 MJPEG 块。3596 Processing `[5365337,5366736)` 没有该组件，不能根据 middleware 中存在 HDR/MJPEG task 就在本型号补出控件。

V2/X 的对焦 gate 还取决于当前固件比较，最低声明分别是 `1.0.2.3` / `1.0.2.0`。当前比较器先检查完整字符串相等，缺项/无效版本按源码路径处理，其余以 coerce 的前三段作严格 greater；同前三段但第四段不同不等于支持。不能只用普通四段数值比较替换后声称完全还原。

共同设置行在挂载时展开，折叠只隐藏本行内容，不写设备；switch、Reset、stepper、checkbox 和帮助各有自己的点击消费者。HDR 的两段切换控件点击任意一半都会翻转，点击已选半边仍是动作。帮助正文是浮层，普通宽 210 px、警告宽 280 px，hover 颜色过渡 300 ms、可见层级 9999；禁用容器同时限制交互。设置差异弹层只提供 CAMERA/SYNAPSE 两个选择，本体没有通用取消/回滚动作；本地 Dialog 应保留明确焦点，不能以 Escape/背景点击擅自提交或丢弃其中一方设置。

媒体预览的 loading、disabled、第三方占用及重连、预览关闭及启用分别由真实枚举/捕获和用户意图驱动。3594/3595 的 `DisplayPreviewWhenDisabled:true` 允许关闭预览时继续编辑；3596 为 false，因此关闭预览、busy 和打开 Windows 属性页会禁用对应设置。不能把空设备列表、读取失败、显示本地预设或 `Save` 清 dirty 合并成“设备状态已确认”。

## 3595 MIC 的提交与确认

MIC `Ph` 的完整 UI owner 是 `[5389440,5392821)`，根实际构造 `Ph({})`，没有传 `openTabReboot`。切换 Adaptive Noise Reduction 或 Crystal Clear Voice 只改 reducer 并置 `isMicDirty`，出现带设备重启链接的警告；点击链接提交两项完整值。该次实际挂载走本机 `dgq → ON_SET_CAMERA_MIC_SETTINGS`，没有进入该共享组件另带的固件下载窗口分支。

动作工厂 `[91443,94649)` 和 reducer `[4832208,4845589)` 显示：`dgq` 在收到设备结果前就清 dirty 并广播，`bC6` 更新两项提交值但不将较晚编辑的 dirty 清掉。middleware [6259.28aada3b1b270a805dc1.js](../../../.analysis/inputs/frontend/accessories-mw-3595-current/raw/apps.razer.com/synapse/products/3595/mw/6259.28aada3b1b270a805dc1.js) 的完整 custom handler `[1117493,1118835)` 不进入普通 RzTaskManager；它调用 `setAudioFunction`，捕获异常后仍广播 submitted values 和 `status:"completed"`。这不是 native 成功确认，更没有独立读回两项值。初始化的 `getAudioFunction` 则是另一次真实读取，不能从 profile 恢复值替代。

native 的 SET → flash save → reboot 三步链见下文。独立测试必须分别保存乐观 UI 行为、真实命令应答、设备读回与重启/重连结果；其中任一步失败都不得由该 `completed` 广播冒充硬件成功。

## Windows 属性页和原生返回边界

原生属性页通过 base filter 查询 COM 页面接口与 IUnknown，在独立线程建立临时宿主窗并调用 OleCreatePropertyFrame。结束时释放页 CLSID 内存、窗体和 COM 引用、恢复线程 DPI 上下文，最后带 containerId 发布 `displayCameraPropertiesDialogClosed`。关闭方法发送 WM_CLOSE、处理线程 join；显示/关闭导出的固定真返回只表示入口执行，不证明窗口已实际出现或属性已保存。

3595/3596 当前 GenericCamera 1.0.35.0 的属性链 RVA 包含 `0x4cc90`（属性页）、`0x4cf70`（线程）、`0x4d130`（关闭）、`0x3e130`（事件），暴露入口为 `0x5ec10/0x5ecf0`。该事件在 UI 中清除“属性页打开”状态，不能显示保存成功。base ISP 链在互斥内核对实例仍存在，经 IKsTopologyInfo 节点发送、间隔 10 ms 读响应；设备删除、HRESULT 失败和 ISP busy/parameter/preview 错误保留各自失败。3596 的 DNR 写入需要先查询伴随参数，再写本值，不能只迁移可见滑块。

原生版本不能混用：3594 manifest 指向 GenericCamera **1.0.36.1**，3595/3596 为 **1.0.35.0**。当前前端、包装器与原生调度的联合证据见 [摄像头分析](../../../.analysis/results/frontend/accessories-camera-local-pages-audit.json)；原生完整捕获和追加核验见下文。原捕获中的两处 UTF 转换反编译失败已经通过原汇编支持的类型恢复补齐，原捕获保留不变；这不表示所有摄像头功能已经实现或经设备验证。

## GenericCamera 原件、版本与虚表

两份原件均为 x64，Image Base 为 `0x180000000`。以下地址全部为所属版本的 RVA；不能把一个版本的槽序或地址直接用于另一个版本。

| 当前产品 | native PID / 工厂对象 | manifest 版本 | 对应虚表 RVA / 槽数 |
| --- | --- | --- | --- |
| 3594 V2 Pro | `0x0e0a` / Pro | 1.0.36.1 | `0xb9690` / 46 |
| 3595 V2 | `0x0e0b` / base | 1.0.35.0 | `0xb84b8` / 44 |
| 3596 V2 X | `0x0e0c` / X | 1.0.35.0 | `0xb8d20` / 44 |

两个 DLL 均包含这三种工厂分支，不能因此改变实际产品的版本关联。[1.0.35.0 完整捕获](../../../.analysis/results/native/84a540e5d1747a8c1912e2f4710850b9397ccaf4ea812be03f20da8f27aa9ae7/capture.json)保留 2817 个函数，[1.0.36.1 完整捕获](../../../.analysis/results/native/daee9fe0f5e9f9fa2a5fe83d9da56081464cc540eeb4452a81e81788c71bf3c2/capture.json)保留 2825 个函数。两者均保留完整函数汇编、交叉引用、类型和全量汇总，实际分析器为 IDA 9.1 / Hex-Rays 9.1.0.250226。

以下追加捕获直接重读各原件的指针表字节，记录每槽文件位置、原始指针、实际目标和完整函数文件哈希。六张表全部解析完成，无新增反编译错误，基准捕获与基准数据库均未修改。

| 分支 | 1.0.35.0 表与目标证据 | 1.0.36.1 表与目标证据 |
| --- | --- | --- |
| base | [`0xb84b8` / 44 槽](../../../.analysis/results/native/84a540e5d1747a8c1912e2f4710850b9397ccaf4ea812be03f20da8f27aa9ae7/supplements/0659bae5b0a4fa70/capture.json) | [`0xb94b8` / 46 槽](../../../.analysis/results/native/daee9fe0f5e9f9fa2a5fe83d9da56081464cc540eeb4452a81e81788c71bf3c2/supplements/cd8c332b26f674d2/capture.json) |
| Pro | [`0xb8680` / 44 槽](../../../.analysis/results/native/84a540e5d1747a8c1912e2f4710850b9397ccaf4ea812be03f20da8f27aa9ae7/supplements/724b68ecfed0a268/capture.json) | [`0xb9690` / 46 槽](../../../.analysis/results/native/daee9fe0f5e9f9fa2a5fe83d9da56081464cc540eeb4452a81e81788c71bf3c2/supplements/714cbef074560636/capture.json) |
| X | [`0xb8d20` / 44 槽](../../../.analysis/results/native/84a540e5d1747a8c1912e2f4710850b9397ccaf4ea812be03f20da8f27aa9ae7/supplements/a77d31ad236b9ae9/capture.json) | [`0xb9d40` / 46 槽](../../../.analysis/results/native/daee9fe0f5e9f9fa2a5fe83d9da56081464cc540eeb4452a81e81788c71bf3c2/supplements/211c6a80ccb7f68e/capture.json) |

下表是导出包装器和实际虚调用的对应关系，偏移相对于对象虚表起点。`0x00` 是删除析构槽；固件、序列号和设置槽仍须继续走对应对象的实际实现，不能只迁移导出入口。

| 1.0.35.0 偏移 | 1.0.36.1 偏移 | 实际职责 |
| --- | --- | --- |
| `08/10/18/20` | 相同 | firmware / serial / edition / timestamp |
| `28/30/38/40/48` | 相同 | HDR / LDC / autofocus mode / HDR brightness 或 mode / mic noise suppression |
| `50/58` | 相同 | SaveUVCExtCmdToFlash / UVCExtCmdStatus |
| `60/68` | 相同 | watermark SET/GET |
| `70/78` | 相同 | ISO SET/GET |
| `80/88/90` | 相同 | FDAF SET/GET / AutoFramingSetting |
| `98/a0` | 相同 | DynamicNoiseReduction SET/GET |
| `a8/b0` | 相同 | mirror SET/GET |
| `b8/c0` | 相同 | metering SET/GET |
| `c8/d0` | 相同 | shutter SET/GET |
| 无 | `d8/e0` | ShutterLimitControl SET/GET |
| `d8/e0` | `e8/f0` | EV SET/GET |
| `e8/f0` | `f8/100` | AutoFocusLowSensitivity / RestoreDefaults |
| `f8/100` | `108/110` | adaptive MJPEG GET/SET |
| `108/110/118` | `118/120/128` | fixed MJPEG GET/SET / StreamingStatus |
| `120/128` | `130/138` | autoframing SET/GET，GET 仅为内部虚方法 |
| `130/138` | `140/148` | audio SET/GET |
| `140/148` | `150/158` | autoframing speed 与 size SET/GET |
| `150/158` | `160/168` | reboot / 实际 ISP 传输 |

新版增加 `SetShutterLimitControl` / `GetShutterLimitControl` 导出，但只有 Pro 的槽目标 `0x52710/0x52790` 发送 `a0 1c` 命令。SET 为模式 1、输入在第 3 字节；GET 为模式 0、结果取应答第 3 字节。base/X 两槽均指向 `0x4e070`，仅返回 false；GET 导出忽略这个返回值，输出其预先清零的临时值。该零值不是实测快门限制。

Pro 的恢复命令还有实际版本变化：旧版 `0x526c0` 写入 `a0 0a 01 00 00 00 00 00`，新版 `0x52860` 写入 `a0 0a 02 00 00 00 00 00`。这两处均以完整原汇编中的立即数核实，不能把旧版命令用于当前 3594。其余配对槽的伪代码地址归一化比较中，音频保存链的 reboot 槽同步由 `0x150` 移到 `0x160`；该比较只是定位版本差异，不能代替逐功能的设备对照。

## native 初始化、实例身份与退出

旧版 `DeviceInitialize(0x57640)` 经 `0x3bc50` 扫描工厂，精确比较完整 container 字符串的长度与字节。已存在的同 container 实例直接成功；新实例只接受枚举结果里的 `0x0e0a/0x0e0b/0x0e0c`。base 构造器 `0x4ab30` 建立互斥、队列与 worker，Pro/X 随后替换虚表；DirectShow 连接 `0x4aee0` 枚举 video moniker，核对 DevicePath 并绑定 IBaseFilter。连接成功后才保留共享实例；失败释放临时对象。新版对应入口为 `0x577e0/0x3bcc0/0x4abd0/0x4af80`。

DLL 的初始化自身最多尝试 **30 次**，每次失败执行 **1000 ms Sleep，包括最后一次失败**，再返回 false；这是包装器外层十次、失败间隔 100 ms 之外的重试。该同步 native 过程没有观察到取消参数，页面退出不能仅以丢弃 JS Promise 证明 native 已停止。控制函数名含 `Async` 也不证明已经排到 worker；标准属性和 ISP 操作的当前路径直接在每实例互斥保护内调用。

`StartCamera/StopCamera` 的旧版 RVA `0x58870/0x58940`，新版 `0x58a10/0x58ae0`，只检查精确 container 是否有已初始化对象，未执行视频流启动或停止。真实预览由媒体请求、流和 track 的所有者控制。`EnumerateCameraList` 刷新物理列表，并关闭、移除不再存在的已连接项；导出固定返回 true 不表示一定发现设备。

旧版 `DeviceTerminate(0x57830)` 先走断开 `0x4b680`：关闭属性页、释放并清空 IBaseFilter，停止 worker。停止函数 `0x4a9b0` 清 active、唤醒条件变量并 join；同线程 join 和无效 join 走错误分支，不能当成功。随后 terminate 再关闭属性页、移除匹配项并释放强/弱引用；未找到实例返回 false。析构函数 `0x4ae30` 处理队列和字符串，并检查是否仍有 joinable 线程。新版 terminate、断开、worker 停止分别为 `0x579d0/0x4b720/0x4aa50`。这条清理链不能由 UI 清空一个连接标志替代。

属性页关闭事件在原回调返回前使用临时 UTF-8 字符串，回调必须立即复制；该借用指针不能交给 `FreeMalloc`，也不能留给后台延后读取。`SetNodeFFIEvent` 是进程级指针赋值，未在 `DeviceTerminate` 中看到按实例清空全局回调。与此相对，字符串 getter 导出另行分配返回副本，由调用者 `FreeMalloc`；传入 null 时该释放导出仍返回 true。事件订阅、返回缓冲区和实例线程是三个独立所有权边界。

## 标准 CameraControl 与 VideoProcAmp

标准属性没有走下面的八字节 ISP 命令，而是 IBaseFilter 的 COM 接口。旧版 Camera getter/setter owner 为 `0x4c270/0x4c510`，实际 COM 函数为 `0x4b8e0/0x4b750`；ProcAmp 对应 `0x4c780/0x4ca20` 与 `0x4be70/0x4bce0`。包装器持实例互斥后调用；QueryInterface 获得的接口在完成后 Release。

| 接口 | IID | 当前读写所用 property ID |
| --- | --- | --- |
| IAMCameraControl | `c6e13370-30ac-11d0-a18c-00a0c9118956` | Pan=0、Tilt=1、Roll=2、Zoom=3、Exposure=4、Iris=5、Focus=6、Flash=7、LowLightCompensation=19 |
| IAMVideoProcAmp | `c6e13360-30ac-11d0-a18c-00a0c9118956` | Brightness=0、Contrast=1、Hue=2、Saturation=3、Sharpness=4、Gamma=5、ColorEnable=6、WhiteBalance=7、BacklightCompensation=8、Gain=9、AntiFlicker=13 |

COM Set 使用 vtable `+0x20`，auto=true 传 flag 1，false 传 2；HRESULT 非负即返回成功，未追加统一 GET 或回滚。Get 使用 `+0x28`，`isAuto` 判断为返回 flags **恰好等于 1**，不作位掩码包含判断。当前标准读写路径没有 GetRange，界面范围仍须追溯每产品 props、配置和固件分支，不能根据接口名称推断设备已提供范围。

原版存在需要单独保留的错误行为：实际 getter 先清零 value/flags；QueryInterface 成功后，即使 Get 的 HRESULT 为负，也只记录日志并继续序列化 value 和 isAuto。若驱动未修改输出，就得到 `{"value":0,"isAuto":false}`；QueryInterface 失败返回空串，缺少 IBaseFilter 的分支返回 `{}`。这些原始返回可以成为差异测试的观察值，但**失败后零值不能成为 Rust 的设备已确认状态**，不能以匹配该 JSON 为理由吞掉传输错误。

## ISP 传输、应答与产品命令

旧版公共 guard `0x4edf0` 在实例互斥内恢复缺失的 base filter，再确认精确 container 仍存在，调用对象的 `+0x158`；新版 guard `0x4eea0` 调用 `+0x168`。实际目标为 base `0x4e6b0/0x4e760`、Pro `0x52d90/0x52f30`、X `0x552f0/0x55490`，斜线前后分别为旧、新版本。

拓扑查询 IID 为 `720d4ac0-7533-11d0-a5d6-28db04c10000`，节点接口 IKsControl 为 `28f54685-06fd-11d2-b27a-00a0c9223196`。三种属性集 GUID 直接从两份原件字节解码，值相同；节点和 property ID 仍有以下分支。

| 分支 | 属性集 GUID | 节点 / SET 与 GET property ID | 应答条件 |
| --- | --- | --- | --- |
| base | `23e49ed0-1178-4f31-ae52-d2fb8a8d3b48` | 遍历拓扑节点 / 1 与 2 | GET HRESULT 必须为 0 且 response[0]=0；`ff` 无设备、`84` 预览开启、`85` 参数、`86` busy |
| Pro | `455a4152-5f52-5355-4243-414d5f455854` | 固定节点 3 / 均为 1 | GET HRESULT=0 且 response[0]=0；1 参数错误、2 命令错误、其他失败 |
| X | `0fb885c3-68c2-4547-90f7-8f47579d95fc` | 遍历拓扑节点 / 均为 5 | GET HRESULT=0 且 response[0]=0；状态码同 Pro |

`IKsControl::KsProperty` 的 ABI 是 **六个参数**：接口、32 字节 KSP_NODE、header 长度 32、八字节输入或应答、payload 长度 8、返回长度指针。x64 原汇编明确在 RCX/RDX/R8/R9 和调用前栈 `+0x20/+0x28` 准备这些参数；base 伪代码中看似只有四个参数是类型推断损失，不能据此建立 FFI。SET flags=`0x10000002`，随后 Sleep 10 ms，再用 GET flags=`0x10000001` 取应答，初始 response[0] 为 `ff`。base 即使 SET 失败也继续 GET，最终依据 GET/status 决定结果；Pro 的设备移除检查只记录日志后仍继续该次 native 调用，X 则退出。这些差异必须保留在原版观察记录中，不能自行补成统一提前失败或统一回滚。

下表描述 **1.0.35.0 base 目标**的八字节 payload，未列字节为 0；下标从 0 开始，`BE32` 是四字节大端。控制报文的输出字段和应答成功与否须分别检查；写函数复制输入或某个应答字段不等于额外读回了整个设置。

| 功能 | 请求及结果 |
| --- | --- |
| firmware / edition / timestamp | 请求分别 `c0 03 01` / `c0 02 01` / `c0 04 01`；取应答 `[2..6)` / `[2]` / `[2..7)` |
| serial | `c0 01 index`，index=1/2/3，逐页追加应答 `[3..8)` 并间隔 10 ms；非空缓存后重用，失败可能留下部分缓存 |
| HDR / LDC / autofocus / HDR brightness / mic | HDR=`ff 02 enable`；LDC=`ff 01 arg1 03 arg2`；其余命令号分别为 `ff 06` / `ff 07` / `ff 04` |
| flash save / extension status | save=`c0 03 a8`，base 函数不使用传入 bool；status=`c0 03 11`，返回应答 `[2..7)` 五字段 |
| watermark | SET=`c0 08 01 arg1 arg2`，GET=`c0 08 00`，返回应答 `[2]/[3]` |
| ISO | SET=`c0 09 01 value`；GET=`c0 09 08`，GET 从应答 `[3..7)` 解 BE32 |
| FDAF / AutoFramingSetting | FDAF SET=`c0 0a 01 mode 00 00 00 companion`，GET=`c0 0a 00`，结果为应答 `[2]/[7]`；另一个 setting 方法使用 `ff 14 arg1 arg2` |
| DNR | 仅接受 selector=1/2；SET=`c0 0e selector value`，GET=`c0 0e selector 02`，取应答 `[3]` |
| mirror / metering / EV | SET=`c0 0e command value`，command 分别为 3/4/5；GET 分别为 `c0 0e 03 02` / `c0 0e 04 10` / `c0 0e 06`，结果取应答 `[3]` |
| shutter | SET=`c0 09 05 BE32`，GET=`c0 09 06`，应答 `[3..7)` 为 BE32 |
| RestoreDefaults | `c0 0e 07`；与页面 Reset、flash save 和 restore+reboot 分开 |
| adaptive MJPEG | GET=`c0 0e 0a input`；SET=`c0 0e 09 mode 00 value`；取应答 `[3]` |
| fixed MJPEG / streaming | GET fixed=`c0 0e 0c`，SET fixed=`c0 0e 0b value`，streaming=`c0 09 0a`；取应答 `[3]` |
| autoframing | SET=`01 00 01 value`，内部 GET=`01 00 00`；结果取应答 `[3]` |
| audio | SET=`c0 0e a0 adaptive crystal`，GET=`c0 0e a1`，GET 取应答 `[3]/[4]` |
| autoframing speed / size | SET=`c0 0e 0d speed size`，GET=`c0 0e 0e`，取应答 `[3]/[4]` |
| reboot / restore+reboot | `c0 0e 07 mode`，mode 分别为 2/1 |

audio SET 不能作为单条命令实现：第一条 ISP 成功 → Sleep 10 ms → 调用实际虚表 flash-save(true) → 成功后再 Sleep 10 ms → 调用实际 reboot(2)，任一步失败终止后续操作，没有通用恢复原值。因 flash-save 和 reboot 都是虚调用，X 最终使用 X 的保存与重启命令，不能硬编码 base payload 替代。

Pro 的固件/序列号/edition 命令使用 `a0` 命令族，序列号页为 0/1/2；DNR 忽略 base selector，metering、mirror、shutter、自动构图和 MJPEG 分别落到 Pro 覆盖函数。ISO GET 把 response[3] 的 1…7 映射为 100/200/400/800/1600/3200/6400；其他码可能保留导出初始化值。EV 表在旧版静态初始化 `0x1020` 中完整建立，code 0…18 的表示值依次为 `60,57,53,50,47,43,40,37,33,30,27,23,20,17,13,10,7,3,0`；SET 选择与输入绝对差最小的条目，平局保留靠前者，再发送条目 code。GET 仅对 code≤18 取表值。该表示不能直接当作 UI 的有符号 EV。Pro 的 speed/size SET 是先 speed `a0 17` 再 size `a0 16`，GET 分两次请求；第二次失败时第一项可能已经写入或返回，没有事务回滚。

X 的 DNR SET `0x53ed0` 必须先用内部 GET `0x540c0` 读取另一 selector：selector 1 对应 response[3]、2 对应 response[2]，再发 `28 00 companion/new-value` 保留另一参数；伴随读取失败不写入。mirror SET/GET 用命令 `29/09`，metering SET/GET 用 `25/05`；metering 输入 `0/1/5` 转成协议 `2/1/3`，其余失败。shutter SET/GET 用 `27/07`，实际仅发送或读取一个字节，不能复用 base/Pro BE32。FDAF SET 用 `2e`，将 mode!=1 的布尔值写入报文；GET `0e` 以 response[2]==0 生成结果。X 的 ISO 两槽与 extension-status 槽指向纯返回 0 的 `0x1ff40`，无设备 I/O；不能把共享导出列表当作该型号的能力表。

## 媒体身份转换、局部反编译恢复与测试边界

旧版 `GetChromiumDeviceId(0x5edd0)` 经 `0x3e3b0` 精确查找已初始化 container，从其 DevicePath 截取 `#{` 前部分，再与 Media Foundation 枚举的路径对应；`GetContainerIdFromDeviceId(0x5ef20)` 经 `0x3ee40` 对当前枚举项生成 ID、按完整字节比对，最后与已初始化实例的路径对应并返回 container。缺少 origin/deviceId、没有匹配或没有已初始化实例时，各自保留原版空值/`{}` 分支，不能按设备标签补出身份。

ID 生成 helper `0x1b880` 使用 HMAC-SHA256：key 为调用方 origin 字符串，数据依次是原始设备 ID 的 UTF-8 与 salt，输出 32 字节的小写十六进制。当前调用传空的显式 salt，因此走缓存读取 `0x1b5a0/0x1ac00`，来源为本机用户 Preferences 的 `electron.media.device_id_salt`。找不到盐返回空值，不能擅造新盐并声称与现有媒体 ID 相同。枚举 `0x1be10` 用 MFStartup/MFEnumDeviceSources，读取符号路径与显示名，正常尾部 Release 各 activate、CoTaskMemFree 输出和列表、Release attributes、MFShutdown；提前失败路径必须逐分支核对，不能由正常尾部推定全部路径释放正确。

本机已安装摄像头程序查询沿独立导出处理。`CheckSpecificCameraAppInstalled` 按完整名称分支；其中一个分支要求软件检测、驱动安装和驱动启用三个真实布尔量同时为真。`CheckCameraAppsInstalled` 接收 JSON 数组，非数组返回 `{}`，未知项仍存在原件的待实现文本分支；它不构成安装成功或全部本机程序已受支持。`LaunchCameraApp` 查本机快捷方式/已安装程序入口，`OpenAudioProperties` 调用系统控制面板且固定返回 true，均不证明窗口实际出现。本分析未执行这些启动入口。

旧版 UTF owner `0x870c0` 和新版 `0x87ad0` 的 call-analysis 错误已分别在 [旧版类型恢复](../../../.analysis/results/native/84a540e5d1747a8c1912e2f4710850b9397ccaf4ea812be03f20da8f27aa9ae7/supplements/b0e66f000087fa60/capture.json)、[新版类型恢复](../../../.analysis/results/native/daee9fe0f5e9f9fa2a5fe83d9da56081464cc540eeb4452a81e81788c71bf3c2/supplements/36fc68c635f7593e/capture.json)补齐。每份保留 owner、scalar 编码、清状态/错误 helper 和调用方共 7 个完整函数。汇编证明错误 helper 仅消费 RCX/RDX，清 surrogate 状态并写错误上下文；原六参数推断错误。恢复后可读出 UTF-16 surrogate 配对、1…4 字节 UTF-8 编码及无效序列失败路径。这是字符串运行库辅助函数的恢复，不是 ISP 或设备业务功能的验收。

本地核验已重新检查两份原件身份、基准清单内全部函数与九个输出文件的哈希、六张表的实际字节和目标文件、追加 session 与基准不变性。两份原始反编译失败各有一项经核实恢复，剩余该类失败为零；函数语义、独立 Rust 实现和设备测试仍按功能分别验收。自动化测试需覆盖每个属性与型号/版本、报文编码、每一步应答、错误 HRESULT、未知协议值、伴随读取失败、部分写入、取消、重连代次、缓冲区与线程释放。官方实际 DLL/设备对照尚未在本轮执行，全部保持未验证；尤其不以模拟报文一致、双方未连接或原版失败零值相同作为通过依据。

## 本地范围与验证状态

已安装虚拟摄像头的本机枚举、用户打开已安装程序、Windows 属性页与真实摄像头设置分别沿各自的本机调用链处理。

### 当前 Rust 的真实生产路径

现代设置的具体链是 `SourceControls → camera_runtime → camera::commands → CameraStart/CameraTaskStart → runtime::portable → camera_controller::Controller → ScopedCameraSession`。[服务入口](../../../crates/razer-service/src/camera.rs)把标准属性交给 Rust DirectShow adapter，扩展与型号功能交给 Rust 编码/执行器；[Windows 标准属性](../../../crates/razer-service/src/runtime/windows/camera.rs)、[Pro 扩展](../../../crates/razer-service/src/runtime/windows/camera_extensions.rs)、[V2/X 拓扑适配](../../../crates/razer-service/src/runtime/windows/camera_focus_family.rs)最终调用系统 COM/IKsControl。预览由[本地媒体适配](../../../crates/razer-platform/src/camera/camera_preview/windows.rs)使用 Media Foundation。上述具体路径没有转发 `GenericCamera.dll`、调用 `EngineLibrary` 或回退官方业务 DLL；其余残留生产 DLL 路径必须另行逐项清理，不能由这一结论宣称全工作区已替代。

| 能力 / owner | 实际读到的本地行为与验收边界 |
| --- | --- |
| 3594/3595/3596 设置 | [camera_runtime](../../../crates/razer-pages/src/features/workspace/source_controls/camera_runtime.rs)持队列、worker、取消信号和独立 observations；[submission](../../../crates/razer-device/src/camera/camera_submission.rs)校验产品并编译标准/扩展/型号命令。已有实现与接线仍需按全部参数、分支和真实返回逐项对照 |
| 3594/3595/3596 媒体 | [camera_media](../../../crates/razer-pages/src/features/workspace/source_controls/camera_media.rs)持 Session、帧、scope revision、capture owner 和 retirement receipt；替换会话先停止接纳，后台等待 `session.shutdown()`，拒收旧 owner 结果。这证明存在清理设计，不证明已经过真机验收 |
| 现代 Windows 属性页 | 请求由独立 retained task 持有，不占普通设置队列；[camera_properties](../../../crates/razer-service/src/runtime/windows/camera_properties.rs)拥有页数组、临时窗、定时取消、DPI 与 COM 引用。页面/profile 的普通替换不取消它，设备 scope 替换/退出才取消 |
| 现代 cache / Reset / Save / MIC | [source state](../../../crates/razer-pages/src/features/workspace/source_controls/camera_source_state.rs)从 11 个 getter receipt 构造缓存；[footer](../../../crates/razer-pages/src/features/workspace/source_controls/camera_footer.rs)与 [MIC](../../../crates/razer-pages/src/features/workspace/source_controls/camera_mic.rs)已有各自提交消费者。草稿、dirty 和广播值仍不能算设备确认 |
| 3587/3589/3590 | [旧款 renderer](../../../crates/razer-pages/src/features/workspace/source_controls.rs)的 `render_legacy_camera_preview` 仍显示固定“实时预览不可用 / 当前未接入摄像头流服务”。scope、request edit、媒体订阅及 submission 的产品条件排除了三款；通用编辑/持久化不能代替真实预览和属性写入。属于尚未完成的范围内链路 |
| 3592 | 已有 Camera/Processing/Image 描述和通用列渲染，但相同产品条件仍排除 Ultra 的媒体/设备提交；footer 与设置差异 Dialog 也只在 3594…3596 分支挂载。不能把界面数据或共享 COM 代码的存在当作 Ultra 已接通 |

### 需要继续收敛的生命周期和状态差异

[workspace 的 `sync_camera_activity`](../../../crates/razer-pages/src/features/workspace/source_workspace.rs)把“当前不是 Help”并入物理 scope 的 active 判定。切到 Help 因而调用 `set_camera_scope(None)`：停止 jobs、取消属性页、清独立 observations、退休媒体 session，并重置 MIC scope。这与前述官方根保留预览/差异/设备状态、仅隐藏视频及底栏的语义不同。设备实例生命周期、宿主焦点与活动子页必须分别持有，不能用这项更早的清理冒充 1:1 还原；应覆盖在途 HDR、Save、属性窗、MIC 编辑及 Help 往返测试。

MIC 已用 `observation: Option<[u8;2]>` 区分读取收据，但当前 scope 设置同时把 `values` 清成 `[false,false]`，渲染只消费这些值写成的 draft；读失败只记录 diagnostic。未观测、读取中、读取失败和真实关闭尚未完整传递到控件呈现，不能把画出的关状态当实测。修正需要保留官方草稿/dirty 的乐观顺序，同时单独表达设备观测状态，不能擅增自动回滚或重试写入。

属性页还有独立失败状态边界：`request_camera_properties` 在异步打开前把 `windows_properties_open` 置真；`finish_properties` 仅在返回 `PropertiesClosed` 时允许清除。若查询页接口、创建宿主窗或 OleCreatePropertyFrame 失败，当前消费路径只打印错误而不结算这个打开状态。3596 的禁用条件依赖该布尔量，必须测试失败后的实际窗口/释放状态和后续可操作性；既不能伪造 `PropertiesClosed` 成功，也不能让已释放的失败请求永久冒充仍打开的窗口。当前业务还广泛使用 `Value` 作为 profile/字段路径边界，完整强类型状态清理亦未完成。

### 当前 camera-local-pages 与 camera-source-review 收据

当前联合审计收据 [`accessories-camera-local-pages-audit.json`](../../../.analysis/results/frontend/accessories-camera-local-pages-audit.json) 的 SHA-256 为 `05bc74fc0b7a4e5aa49dfe370c612a7edf57968050f144ebde3626861a5e525e`，覆盖 3594、3595、3596 三个当前入口、219 个完整 owner；三份页面层 `failures` 与 `unresolved_chains` 为空，但这只表示已选静态 owner 查询收束，不表示整页验收。3594 记录 51 个 owner、27 个控件比较和 20 条本地链；3595/3596 共享 1.0.35.0 原生捕获，各记录 2,817 个完整函数并保留 `0x870c0` 的 Hex-Rays 失败。3594 使用 1.0.36.1 捕获的 2,825 个完整函数并保留 `0x87ad0` 失败；两处均有独立补查收据，原始失败不能从基准收据中删除。

3595 与 3596 收据仍各保留同一条剩余边界：完整的 Image/Camera/Processing 展示、产品/固件分支和快捷键行为需要产品级整页核对，已核的 footer、processing、native vtable 和 source cache 不能提升为整页完成。当前静态结果明确了输入、页面 owner、协议分支、Rust 接线和失败/取消/释放观察；官方 DLL 的逐功能结果、真实设备应答/读回、写入恢复以及窗口与媒体验收仍为 `Unverified`，不可由捕获成功或模拟传输一致替代。

### 分层验证

| 层次 | 本次可支持的结论 |
| --- | --- |
| 来源获取 | 阅读使用当前获取收据与实际原件；三款现代联合报告的 12 份原件及 219 份完整 owner 切片已作字节核对；旧款/Ultra 另核对部署来源、原件与词法绑定 |
| 逆向语义 | 本文列出的页面、消费者及选定 native 路径已阅读；未将完整函数捕获数量等同于全部业务语义闭合 |
| Rust 独立实现 | 现代具体 DirectShow/KS/MF 路径已有；旧款、Ultra 及上述状态/lifecycle 差异仍未完成 |
| UI / 后端接线 | 现代已有明确消费者，但存在 Help owner 差异；旧款/Ultra 仍缺实际链路；尚非全部页面逐状态验收通过 |
| 静态 / 模拟验证 | 本次进行来源、原始字节、完整 owner、native 捕获/追加证据和文档引用核对；现有 Rust 测试的存在不代表本轮运行通过 |
| 官方实际 DLL / 设备对照 | 本轮未执行，保持未验证；无设备时先做独立期望的纯逻辑、模拟传输和有据回放，不能用双方未连接或模拟一致替代 |

后续自动化覆盖应包含每个已挂载/未挂载分支、固件 gate、数字编辑边界、已选项再次点击、折叠/焦点/键盘、预览加载/关闭/占用/错误、真实 cache 与差异选择、source/resolution/设备切换、Help 往返、取消后迟到结果、部分写入、读回和完整释放。官方对照必须执行对应 DLL 功能并记录本地结果、实际应答、差异及恢复；加载成功、固定真返回、middleware `completed` 与仅停止线程都不足以判定功能通过。本文没有执行摄像头、媒体捕获、DLL、JavaScript 或主程序。

## GenericCamera 原生检查与相机应用边界（2026-10-24）

当前两份 GenericCamera 原件的静态捕获收据分别是 [`84a540e5…/capture.json`](../../../.analysis/results/native/84a540e5d1747a8c1912e2f4710850b9397ccaf4ea812be03f20da8f27aa9ae7/capture.json) 与 [`daee9fe0…/capture.json`](../../../.analysis/results/native/daee9fe0f5e9f9fa2a5fe83d9da56081464cc540eeb4452a81e81788c71bf3c2/capture.json)。前者保留 2,817 个完整函数，后者保留 2,825 个完整函数；两份均有一个 Hex-Rays call-analysis 失败（分别为 RVA `0x870c0` 与 `0x87ad0`）。完整汇编、失败记录和补查收据保持在各自结果目录，不能把补查伪装成基准伪代码成功。

### 外部相机应用检查

- `CheckCameraAppsInstalled` 要求输入 JSON 为数组；传入 `{}` 会产生 `camera apps should be an array!`，而不是静默当成空列表。宿主经 `CRazerDevicesManager::CheckCameraAppsInstalled` 保存到应用状态；两个版本的 owner 分别位于 RVA `0x5f1e0` 与 `0x5f5a0`（对应函数链接见上方 capture 目录）。
- 默认检查项包含 `Camo Studio`、`NVIDIA Broadcast` 和 `XSplit VCam`。未命中时各自结果保持 false／未安装边界，不能把名为 `Pending implement` 的 UI 文案当成已安装或成功。
- Camo 分支使用 helper RVA `0x3f7f0`，读取 `CamoStudioVersion`、`CamoDriverInstalledState`、`CamoDriverEnableStatus`，再由 `CheckSpecificCameraAppInstalled`（RVA `0x40be0`）组合专用结果。NVIDIA 分支使用 `0x3fb90`，写入 `BroadcasterResult` 与 `BroadcasterVersion`；XSplit 分支使用 `0x3fe00`，写入 `XSplitVCamResult` 与版本字段。第二版的专用 helper 位于 `0x40c50` 与 `0x411d0`。这些结果是应用存在性探测，不是摄像头设备 ACK。

### 摄像头生命周期、列表与属性窗口

- `StartCamera`／`StopCamera` 的两个版本 owner 分别覆盖 RVA `0x58870/0x58940` 与 `0x58a10/0x58ae0`；它们围绕容器、采集 track 和停止路径管理本地资源，返回值不能替代帧读回或设备确认。
- `EnumerateCameraList` 先枚举 DevicePath 与属性，再经过过滤 helper `0x5eb70`／`0x5ef30`；列表返回 1 只说明该枚举分支的状态，不能推断摄像头已打开。对应函数的原始捕获位于两个 capture 目录的 `functions/5eb70.json`、`functions/5ef30.json`。
- `DisplayCameraPropertiesDialog` 通过 base filter owner RVA `0x4cf70` 检查 `m_baseFilter == nullptr`，属性页关闭使用 `WM_CLOSE`；COM 属性框架、窗口句柄和失败清理必须与 `CloseCameraPropertiesDialog` 分开测试。其余属性入口覆盖 `0x5ec10/0x5ecf0`、`0x5efd0/0x5f0b0`，不能只记录打开入口而省略关闭路径。

本段只确认原件字节、函数身份、输入校验和静态调用边界；没有执行 DLL、应用、摄像头或媒体采集，也没有确认真实设备帧、属性写回、读回、取消、恢复或完整释放。独立 Rust 实现仍需按已核 ABI 和每个产品分支实现，并以显式对照入口记录官方 DLL 与 Rust 的输入、输出、错误、读回和清理差异。

## Kiyo 0E08/0E03 原生相机链（2026-10-24 四线程静态捕获）

本节记录两个 Kiyo 原生版本的独立调用链。两份原件的函数集合和初始化边界不同，不能把一个版本的行为泛化到另一个版本；下表的函数数是各自 capture 的完整集合，重复版本不相加。

| 原件 | 原件字节 / SHA-256 | capture.json SHA-256 | 函数 / Hex-Rays 失败 | 导出入口 |
| --- | ---: | --- | ---: | ---: |
| RzNative_0E08_v1.0.16.0.dll | 600264 / 1ea145fe182505cd8ae19a6bc1ea5362c70d18681b51195d026ec57c86ad06d8 | 54f729c256fd559ec8c5c286a869f9fcf587e4546d49ab23c112f19114989275 | 2060 / 0 | 91 |
| RzNative_0E03_v1.0.16.1.dll | 796296 / d4fd77a92c860bb190d7aab604124e9d4343901fcea83069aa99d29db5a9586b | 23ce499d0c8fae7c60e75c97072dc080a0876f1c07376aad8d748af523d9a377 | 2634 / 1 | 40 |

完整批次摘要和原始字节核验在 [camera-audio-native-batch analysis.json](../../../.work/camera-audio-native-batch-20261024/analysis.json)。对应的 [0E08 capture](../../../.analysis/results/native/1ea145fe182505cd8ae19a6bc1ea5362c70d18681b51195d026ec57c86ad06d8/capture.json) 和 [0E03 capture](../../../.analysis/results/native/d4fd77a92c860bb190d7aab604124e9d4343901fcea83069aa99d29db5a9586b/capture.json) 均为完整捕获。独立 PE 映射检查确认两个模块所有已列 chunk 和 assembly 指令均与原件字节一致；0E08 为 2066 个唯一 chunk、103613 个唯一指令、419265 个唯一 chunk 字节，0E03 为 2684、143877、555742。0E03 的 0x180072223 仍保留 call-analysis failed（code -12），所以不能把该模块写成全函数语义闭合。

### 0E08 的 COM、设备登记和窗口停止

0E08 的 DeviceInitialize（RVA 0x50bb0）先调用 CoInitializeEx(0, 2)。S_OK/S_FALSE 和 RPC_E_CHANGED_MODE 都进入设备管理器；只有前两种结果调用 CoUninitialize。管理器 0x401a0 先复用已登记 ID，否则刷新视频设备枚举，按 168 字节记录中的字符串精确比较，创建共享 RzDeviceAccess 并打开；只有打开成功才加入活动列表。DeviceTerminate（0x50d50）同样处理 COM 模式，找到精确 ID 后先停止相机窗口，再清理命令执行器、在 SRW 锁下释放接口并删除列表记录。窗口停止路径关闭窗口、清理句柄并对非当前线程执行 join；自 join 或 join 失败会抛出异常，不能用返回成功代替线程已退出。

StartCamera/StopCamera（0x51ca0/0x51d70）在导出层只做管理器查找，当前捕获没有帧接收或读回证据。SetNodeFFIEvent（0x50a70）只保存回调指针并返回 1；没有看到等待在途回调完成的链。FreeMalloc（0x50a80）对非空指针调用 free 并返回 1，Rust 侧必须保持与该导出分配器的边界，不能把任意 Rust 分配指针交给它。

### 0E08 属性命令和异常恢复

Brightness 的 getter/setter（0x51e40/0x53450）使用 VideoProcAmp 属性 0，WhiteBalance 使用属性 7，Focus 使用 CameraControl 属性 6；通用 getter 0x4a130 和 setter 0x4a5d0 通过命令执行器在共享锁下访问实时接口。getter/setter 的超时计数达到三次会调用软件恢复 0x4c930，再各重试一次；重试失败只记录日志并返回失败，不能回退到旧缓存。无接口或异常分支会保留序列化的空对象或失败布尔值，不能解释为设备返回了默认值。

SaveUVCExtCmdToFlash（0x51ba0）和 RestoreDefaults（0x571a0）分别转发设备虚表命令；本层没有证明 flash 写入后的读回，也没有闭合默认表内容。0E08 还导出了对比度、色相、饱和度、锐度、伽马、增益、背光、曝光、HDR、自动对焦、MJPEG、镜像和曝光测光等更宽属性集合；导出数量只说明 ABI 表面，若分支主体返回 0 或空对象，不能据此声称该属性已经接通。

### 0E03 的版本差异

0E03 的 Initialize（0x4e4a0）和 Terminate（0x4e530）只是日志包装并返回 1，不负责设备生命周期；实际入口是 DeviceInitialize（0x4e660）和 DeviceTerminate（0x4e730）。后者按精确 ID 查找设备，停止线程、释放接口、移除枚举记录并释放引用。DeviceInitialize 不重复 0E08 的 COM 模式门禁。SetEventInterface（0x4e3a0）只检查三个参数非空，当前函数体没有保存或注册接口的动作。

0E03 的 Brightness/Contrast/Saturation/WhiteBalance/Focus/Zoom getter/setter 分别转发属性编号 0、1、3、7、6、3；执行器只对超过约 500 ms 的调用记录日志，没有 0E08 的三次超时恢复重试。Initialize/Terminate 导出返回 1 不能替代 DeviceTerminate 的真实移除。该版本的 FreeMalloc（0x4e370）经内部分配器 0x69060 释放；在 Rust 侧必须按版本分别配对，不能假设与 0E08 的 CRT free ABI 相同。

两个版本的 UVC 扩展、设备身份字符串、属性窗口和相机应用入口仍只完成静态语义核对。没有运行 DLL、Media Foundation、设备或窗口，也没有执行写入、读回和恢复；官方结果与 Rust 结果的对照、失败/取消/重连和完整释放均保持 Unverified。
