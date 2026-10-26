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

属性页的静态增量见 [`70dcb7e0…`](../../../.analysis/results/reverse-pending/review-sources/70dcb7e0ddce72ce37c75ab4939e43c5620caa55984d1efee870ce7e22eec14f.json)：七款产品、145 个完整原件 AST 切片和六份不同 DLL 的 64 个完整函数引用。每个切片重新核对原件字节、SHA 和实际 AST 边界；函数重新核对 PE 导出、函数边界与全部捕获 chunk。正文存储复用完整原件及捕获，增量为 288,045 字节，不复制整套报告，不修改旧 review。

### 打开、关闭通知与读取消费者

3594/3595/3596 的 Image 页实际点击 Windows 链接后，广播 `SET_SHOW_CAMERA_ADVANCED_SETTINGS` 并立即把 reducer 的 `IS_WINDOWS_UVC_SETTINGS_OPEN` 置真。MW 经当前事件 owner 排入 `NORMAL_SKIPPABLE`、`imageSetting` 任务；3594 内联 task maker，3595/3596 经 module 30525 的 `FVj`。module 85191 的 `F8l` 只在开始前检查 abort，随后等待 `showSettingsPage()`；false 返回被忽略，异常仅打印后仍置 `completed`，`OKH` 完成消费者仅打印日志。因此排队完成不能证明窗口出现、设置成功或取消已传到属性线程。

实际包装器在 `isInit` 为真时调用 `DisplayCameraPropertiesDialog(container)`，否则返回 false。两个 GenericCamera 版本的显示与关闭导出在缺少匹配实例时也固定返回 1。属性线程依次查询 `ISpecifyPropertyPages`（IID `b196b28b-bab4-101a-b69c-00aa00341d07`）和 `IUnknown`（IID `00000000-0000-0000-c000-000000000046`），调用 `GetPages(+0x18)` 和 `OleCreatePropertyFrame`。这两个调用的 HRESULT 都没有进入原版结果消费者；页数组未在调用前初始化。第二次 QI 失败会直接返回，未走第一次已取得页接口的 Release。早退也没有发送关闭事件，不能把打开布尔量清理、COM 释放与原版固定真返回视作同一确认。

所有六份 DLL 的关闭事件只构造 `event:"displayCameraPropertiesDialogClosed"` 和 `containerId`，不包含 Apply/Cancel、HRESULT、变化字段或保存结果。自然返回与主动关闭都可能产生同名事件；原版主动关闭在 HWND 非空、active 为真时，先发事件，再清 active、发送 WM_CLOSE、销毁窗口及 join。因此消费者收到事件不保证释放链已经结束，也不能由该事件还原用户按了 Apply、OK 或 Cancel。

MW 的同 container 监听依次等待 module 87887 的 `db`、`yp`，然后广播 `MW_SET_CURRENT_CAMERA_SETTINGS` 和 `MW_SET_IS_CAMERA_ADVANCED_SETTINGS_OPEN:false`。`db` 首先并发读 brightness/contrast/saturation/white balance；高级分支再并发读 zoom/pan/tilt/focus/exposure/low-light/anti-flicker。它只更新这些标准属性缓存，扩展曝光分支还固定把缓存 exposureValue 置 0，没有顺带读取全部 ISP 属性。`yp` 则只对实际 `rzCamKiyoX` 类读取 zoom，并在已有 camera 配置时写入当前 profile；对现代 Kimberly 包装器不执行该写入。两者内部捕获错误后正常结束，外层仍可能广播旧缓存或部分缓存并清打开状态。原版的 `Number(value||0)` 也必须与读取失败区分，不能成为本地实现的实测零值。

现代 reducer 把缓存发布为 `currentCameraSettings`，保留在产品根的差异 owner 比较它与当前 profile，按差异打开 CAMERA/SYNAPSE 选择层。两项选择经实际字段动作提交被选值，500 ms 后请求 `reloadAdvancedCameraSettingsCache`；它们不是 Windows 属性框的 Apply/Cancel，也没有由这一步隐式确认 flash save。预览 owner 没有以 `isWindowsUVCSettingsOpen` 为依赖的停流/重启 effect；该布尔量进入编辑禁用判定。旧款 Image owner 则注册 `changeActiveTab → closeCameraAdvancedSettings()`，该 effect 没有返回监听释放函数。不能把不同产品的 tab 切换和媒体停止统一处理。

### 各原件的线程与窗口释放差异

| 产品 / 原件 | 属性 frame / 启动线程 / 关闭 join / 事件 RVA | 实际窗口与 COM 行为 |
| --- | --- | --- |
| 3587 / 0E03 1.0.16.1 | `4a660 / 4a950 / 4aa70 / 43200` | `Button/Locator` HWND 存在实例 `+200`，线程对在 `+472`；frame 正常尾部释放两个 COM 接口，但没有释放 `GetPages` 数组，也没有销毁宿主窗口；关闭入口另行销毁并 join |
| 3589 / 0E05 1.0.15.1 | `4a770 / 4aa50 / 4ab70 / 43360` | 同类 `+200` HWND 与 `+472` 线程对；正常 frame 同样不释放页数组、不中止宿主窗口 |
| 3590 / 0E06 1.0.13.1 | `4a230 / 4a520 / 4a640 / 430c0` | 同类 HWND 与线程布局；close 先清 HWND，再清标题，最后 join，不能仅按其他版本的伪代码地址套用 |
| 3592 / 0E08 1.0.16.0 | `4aa50 / 4aec0 / 4afe0 / 41df0` | shared SRW lock 检查 filter；属性线程自行 `CoInitializeEx(STA)`，接受 `RPC_E_CHANGED_MODE`，只对本次 S_OK/S_FALSE 配对 `CoUninitialize`。HWND 存在 `+8`，正常 frame 销毁并清零；未见页数组释放；线程对在 `+488` |
| 3594 / GenericCamera 1.0.36.1 | `4cd30 / 4d010 / 4d1d0 / 3e1a0` | 局部 `STATIC/Host` HWND；正常返回释放页数组、宿主窗和 COM 接口、恢复 DPI；线程对在 `+544` |
| 3595/3596 / GenericCamera 1.0.35.0 | `4cc90 / 4cf70 / 4d130 / 3e130` | 同类局部宿主窗与释放序列；分别由 `5ec10/5ecf0` 导出进入。3594 的对应导出为 `5efd0/5f0b0` |

GenericCamera 两版 frame 的完整汇编都只把 `CreateWindowExW` 返回值留在局部寄存器，未赋给 close 检查的实例 `+8`；正常尾部反而把 `+8` 清零。close 的 HWND 为空分支直接等待已有属性线程，不发送 WM_CLOSE。它与旧款/Ultra 实际保存 HWND 的链条不同；所选属性线程入口与 frame 也没有 COM 初始化/反初始化调用，不能借用标准属性 worker 的 COM 初始化证明该线程有 apartment。以上是原版静态行为；实际窗口、跨线程关闭和错误释放仍须执行受控对照，不能宣称已经通过。

包装器 terminate 先移除本实例事件监听，再调用 DeviceTerminate 并清 `isInit`；destroy/beforeunload 不等待异步 terminate。GenericCamera 的原生断开先 close/join，再 Release filter 和停止 worker，随后再次 close、移除实例并释放引用。显示线程在 active 尚未置真时的重复打开、早退不发关闭事件、close 等待在途 modal，以及重连时旧异步读取都应分别测试；不能只移除 UI 订阅就报告线程已结束或设置已恢复。显示/关闭导出、标准 getter、ISP ACK 与媒体 track 各有自己的成功、取消和释放边界。

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

两版三种传输均以 GET HRESULT **恰好为 0** 和应答状态 0 判成功，未额外检查返回长度；非零状态立即结束该次交换，没有对 busy 的通用重试。base/X 在 GET HRESULT 非零时可继续下一节点；取得节点接口后遇到设备移除则直接跳出，没有经过该节点的 Release，外层拓扑接口仍在正常尾部释放。Pro 固定节点的创建 HRESULT 与节点指针未受后续独立检查，移除检测也不阻止此次虚调用。正常路径释放不能掩盖这些错误分支；Rust 必须把原版结果观察与自身句柄释放分别验证。

当前三个现代产品的完整 module 85191 还显示了结果消费差异。DNR runner 顺序写 selector 2（2D）、selector 1（3D），第一项返回 false 仍调用第二项；两项结果保留在 `apiResult`，完成消费者却只检查 `status:"completed"`。Save runner 调用 `saveUVCExtCmdToFlash`，完成消费者检查 completed 与返回布尔量，但无论结果先广播 `SET_HAS_VIDEO_RESTART:true`；HDR 同样先广播重启，再检查两个返回值的合取。它们都只在 runner 开始时检查 abort，没有把取消信号传入 COM/ISP、10 ms 等待或后续命令，也没有通用原值恢复和独立属性读回。需要保留 UI 既有的乐观顺序，同时禁止把 completed、预览重启或成功日志当作设备写回确认。

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

当前联合审计收据 [`accessories-camera-local-pages-audit.json`](../../../.analysis/results/frontend/accessories-camera-local-pages-audit.json) 的 SHA-256 为 `703b8b65f535c6780d2239f3bd4869098bea354194bfb4b4583983ad6cdf7d55`，现代产品部分覆盖 3594、3595、3596 三个当前入口、219 个完整 owner，旧款选择与标准属性增量由其中 `legacy_selection` 独立引用；三份页面层 `failures` 与 `unresolved_chains` 为空，但这只表示已选静态 owner 查询收束，不表示整页验收。3594 记录 51 个 owner、27 个控件比较和 20 条本地链；3595/3596 共享 1.0.35.0 原生捕获，各记录 2,817 个完整函数并保留 `0x870c0` 的 Hex-Rays 失败。3594 使用 1.0.36.1 捕获的 2,825 个完整函数并保留 `0x87ad0` 失败；两处均有独立补查收据，原始失败不能从基准收据中删除。

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

- `StartCamera`／`StopCamera` 的两个版本 owner 分别覆盖 RVA `0x58870/0x58940` 与 `0x58a10/0x58ae0`；它们仅确认精确 container 的已初始化实例是否存在。采集流和 track 由独立媒体 owner 管理，返回值不代表开始或停止了采集。
- `EnumerateCameraList` 先枚举 DevicePath 与属性，再经过过滤 helper `0x5eb70`／`0x5ef30`；列表返回 1 只说明该枚举分支的状态，不能推断摄像头已打开。对应函数的原始捕获位于两个 capture 目录的 `functions/5eb70.json`、`functions/5ef30.json`。
- `DisplayCameraPropertiesDialog` 的线程 launcher 是 RVA `0x4cf70`，属性 frame 是 `0x4cc90`，关闭/join 是 `0x4d130`。launcher 对缺失 filter 调用恢复 owner 后重新检查；close 是否发送 WM_CLOSE 取决于实例 HWND，不能从函数名推定。导出覆盖 `0x5ec10/0x5ecf0`、`0x5efd0/0x5f0b0`，完整版本差异和清理边界见上方属性页段。

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

## 0E05／0E06 的 ISP 传输与确认差异

本节分别依据 [0E05 1.0.15.1 完整捕获](../../../.analysis/results/native/5d2bcb82f6e288394a58199e72dde4c13e1fd03174cd55ff39d7a3d9fdfbffb0/capture.json)和 [0E06 1.0.13.1 完整捕获](../../../.analysis/results/native/48b542a7034eca9b4128d86693ffbe2e1748ffd6a58c15091ea5a53e2ae55bdd/capture.json)。两者 Image Base 均为 `0x180000000`。原件、完整函数块、逐条汇编和捕获产物已经独立核对；本节追到实际 COM／KS 调用，不能由同名导出推定产品行为相同。

0E05 的 `IspCommand` 在 `0x4b2f0` 创建任务，原始 PE 的任务虚表 `0xa0a40+16` 指向 `0x4c140`，再调用 `0x4aef0`。0E06 对应任务表 `0xa1ae0+16` 指向 `0x4c190`，再调用 `0x4af50`。两者均从设备对象 `+40` 查询拓扑接口，调用接口 `+64` 取节点数、`+80` 建立节点接口，再经节点接口 `+24` 发送 32 字节属性头和 8 字节 payload。每次操作前重新按设备 ID 查询 manager，设备移除时返回失败；这不构成对象引用或并发移除已安全的证明。

| 边界 | 0E05 | 0E06 |
| --- | --- | --- |
| 写属性 ID／flags | `1 / 0x10000002` | `5 / 0x10000002` |
| 写返回判断 | 未检查写入 HRESULT，随后继续读 | 仅 HRESULT 非负才继续读 |
| 写读间隔 | 10 ms | 5 ms |
| 读属性 ID／flags | `2 / 0x10000001` | `5 / 0x10000001` |
| 传输 helper 成功 | 读 HRESULT **等于 0** 且 response byte 0 等于 0 | 读 HRESULT 非负；由命令调用方继续检查 response byte 0 |

原 PE 的 GUID 常量也分别核实：两者拓扑接口为 `720d4ac0-7533-11d0-a5d6-28db04c10000`，节点接口为 `28f54685-06fd-11d2-b27a-00a0c9223196`；0E05 属性集合是 `23e49ed0-1178-4f31-ae52-d2fb8a8d3b48`，0E06 是 `0fb885c3-68c2-4547-90f7-8f47579d95fc`。不能只替换属性 ID 而保留另一产品的集合 GUID。

0E06 写失败路径未见与成功分支相同的节点 `Release`；两版在设备移除分支也未见先释放本轮已取得节点。必须保留失败路径与接口所有权的实际差异，不能把正常路径存在 `Release` 写成所有分支都完整清理。

0E05 的 `SetHDR`（`0x4f0f0`）发送 `ff 02 enable 00 00 00 00 00`，`SetHDRMode`（`0x4f240`）发送 `ff 07 mode_low8 00 00 00 00 00`，`SetLDC`（`0x4f390`）发送 `ff 01 arg2_low8 03 arg3_low8 00 00 00`。`SaveUVCExtCmdToFlash`（`0x50820`）的第二参数未在函数主体消费。0E06 不能直接从导出套用此结论：设备构造和实际虚表 `0xa19a8` 另证 `+40 → 0x4aba0` 为 HDR、`+80 → 0x4ae30` 为 flash；两者使用各自 ISP helper，并由调用方检查响应首字节。

flash 的实际 8 字节为 `c0 03 a8 00 00 00 00 00`，不能把伪代码十进制常量当成十六进制报文。0E06 的虚表 `+48/+56/+64/+72` 分别指向 `0x4ac20/0x4acb0/0x4ad30/0x4adb0`，报文分别为 LDC `ff 01 arg2 03 arg3 00 00 00`、`ff 06 value 00 00 00 00 00`、`ff 07 value 00 00 00 00 00`、`ff 04 value 00 00 00 00 00`。导出 `SetMicNoiseSuppression`（`0x504e0`）明确消费 `+72`；`+56/+64` 尚不能凭内部命令存在推定成当前 UI 能力，0E06 的导出表也没有同版本 0E05 的 `SetHDRMode`。`0x4aa50` 的请求首字节 3，成功后分别取 response byte 1、2、3..7；`0x4aaf0` 的请求首字节 4，返回 byte 2..5 和 byte 6，其中最后一项写入 WORD 的值仅来自一个字节，不能擅自合并 byte 6/7。

两版的标准属性导出另按实际调用核对：Brightness／Contrast／Saturation／WhiteBalance 的 VideoProcAmp 编号为 `0/1/3/7`，Focus／Zoom 的 CameraControl 编号为 `6/3`。0E05 的 VideoProcAmp getter `0x4a0d0` 先检查设备接口和两个 out pointer，0E06 的 `0x49bb0` 仅先检查两个 out pointer，设备接口检查位于后续执行链；这一区别须保留为失败与断连测试分支。JSON 属性 getter、ISP 状态 getter 和序列号／身份转换各有独立消费者，不能以属性编号表覆盖整个 DLL。

两版 `Initialize` 只记录日志并返回低字节 1，不能当作设备初始化。0E05／0E06 的任务等待入口分别为 `0x48d70`／`0x48890`，使用 4,000 ms 等待；超时分支关闭 active 标志，打开线程、detach、调用 `TerminateThread` 并关闭 handle。这是原件的强制终止路径，不证明硬件状态恢复、在途 COM 调用完成或安全取消。应答、超时、重连、恢复、引用释放以及官方 DLL／设备实际对照分别保持 Unverified。

## 0E06 的媒体身份转换、结果消费者与失败释放

本节仅覆盖 `0E06 1.0.13.1` 的身份转换分支，依据[逐函数与原件字节核验收据](../../../.analysis/results/native-bulk/camera-0e06-identity-verification.json)。18 个完整函数的 3,876 条指令、17,336 字节及 7 处常量均与保留 PE 原件一致；前端从当前保留的 `3590/mw/6120` 原件重新解析 4 个完整 AST 节点。收据包含输入身份、获取收据、原件清单、RVA、函数边界、逐函数产物身份和消费者正文，不能据此推广其他产品／版本或宣称全部相机功能已完成。

`GetChromiumDeviceId(0x50a30) → 0x43340` 先用 `0x416a0` 按完整 container 字节精确查找已初始化实例；无匹配返回文本 **`{}`**（原件 `0x9c604`），不是实测媒体 ID。匹配后取实例 `+144` 的宽 DevicePath，经 `0x7120` 截取第一处 `#{` 之前的内容，再与 `0x7230` 当前 MF 枚举项的规范化路径按完整长度和宽字节比较。找不到 `#{` 时规范化结果保留空串，不能回退原路径。**已有实例但没有匹配的 MF 项时，原始媒体路径保留空串，随后仍调用 HMAC**；只要 salt 与计算有效，可能返回非空 hash，故非空 ID 本身不能证明媒体路径已匹配。

`GetContainerIdFromDeviceId(0x50ba0) → 0x43dc0` 先建立空输出，host 或 deviceId 为空时直接返回该空值。其他分支对各 MF 项的完整原始路径生成 HMAC，精确比较 hash 长度与字节，再与已初始化实例的规范化路径比较，命中后才返回该实例 `+48` 的 container。无 hash 命中、没有已初始化实例或其路径不匹配，均保留空结果；显示名不参与身份匹配，也不能代替 container 确认。

`0x7230` 顺序是 `MFStartup(0x20070,0) → MFCreateAttributes → SetGUID → MFEnumDeviceSources`。实际属性 GUID 字节另行核实：source type 为 `c60ac5fe-252a-478f-a0ef-bc8fa5f7cad3`，video source type 为 `8ac3587a-4ae7-42d8-99e0-0a6013eef90f`；两次 `GetAllocatedString` 先取 friendly name `60d0e559-52f8-4fa2-bbce-acdb34a8ec01`，再取 symbolic link `58f0aad8-22bf-4f8a-bb3d-d2c4978c6e2f`。输出项跨度 128 字节，`+0/+32/+64/+96` 依次为宽 symbolic link、规范化宽路径、宽 friendly name 和空窄字符串。身份计算使用完整 symbolic link 的 UTF-8，规范化路径只用于设备关联；不能交换两者作为 HMAC 输入。

HMAC helper `0x6ca0` 接收 raw ID、host 和显式 salt；显式 salt 为空时调用 `0x69c0 → 0x60a0`，从 `CSIDL_LOCAL_APPDATA/Razer/RazerAppEngine/User Data/Default/Preferences` 读取 `electron.media.device_id_salt`。`electron/media` 必须为对象，salt 必须为字符串；不存在、错误类型、空值或读文件失败均不能生成替代盐。缓存仅在非空时保留，采用锁内再次检查；空结果后续会重读，已缓存非空值则不在本链重新读盘。这些函数没有写 Preferences 或产生随机 salt 的操作。

`BCryptOpenAlgorithmProvider` 使用 `SHA256`、`Microsoft Primitive Provider` 和 flags `8`，`BCryptCreateHash` 的 key 为 host 字节；`0x5f50` 先更新 raw UTF-8 ID，再更新 salt。有效 hash 对象的零长度更新不调用 `BCryptHashData` 且返回成功；缺少 hash 对象或更新失败返回假。Open／Create 失败先记录日志后仍到更新检查，不是直接从错误记录处返回；更新或 Finish 失败最终为空结果。成功 Finish 固定输出 32 字节，`0xabd0` 用原件 `0xa2180` 的 `0123456789abcdef` 表转为 64 个小写十六进制字符。

释放差异必须保留：`0x6ca0` 的 `0x7005…0x7021` 汇编在 hash 非零时调用 `BCryptDestroyHash`；provider 非零却跳过 `BCryptCloseAlgorithmProvider`，零分支传给后者的是栈内 handle 槽地址。此处是已核实的静态控制流与传参差异，**没有执行测量泄漏，也不能描述为已完成配对释放**。MF 枚举的正常尾部释放两项字符串、各 activate、列表及 attributes，再调用 `MFShutdown`；但 CreateAttributes 失败直接返回，SetGUID／Enum 失败释放 attributes 后返回，三者均没有走到 `MFShutdown`。单项 `GetAllocatedString` 失败直接转到该 activate 的 Release，跳过该项的 `CoTaskMemFree`，因此第二次调用失败时也跳过对第一项已分配字符串的释放。

两个导出按返回内容的 `strlen+1` 通过 `0xbc248` 的自定义 allocator 或默认分配入口申请缓冲区，再复制含 NUL 的内容并释放临时 `std::string`。虽然 caller 伪代码省略复制 helper 的源参数，`0x50b51…0x50b5a`、`0x50cc1…0x50cca` 汇编均明确设置 `R8=source / RDX=capacity / RCX=destination`。空串和 `{}` 也可以对应非空返回指针，分配成功不能当成设备匹配成功。`FreeMalloc(0x4e0a0)` 调用本地释放入口；当前 host 的实际静态释放消费者及自定义 allocator 的限制见下节。

前端 `rzCamKiyoX` 的两个方法将 `protocol + "//" + host` 原样作为 origin，`isInit` 为假时返回空串、不调用 DLL。`GET_CAMERA_HARDWARE_ID` 消费者完整分支位于 `6120` 字节 `[1194075,1194742)`：先枚举一次媒体列表，筛选 label 含 `1532` 的 `videoinput`，每轮并行调用反向转换；只在 Promise fulfilled 且返回 container 严格等于当前 container 时向 UI 发布对应 deviceId。最多 9 轮，未匹配轮等待 1,000 ms；该分支循环内没有重新枚举，不能把等待描述为已发现新设备。正向 wrapper 位于 `[747478,747734)`，反向 wrapper 位于 `[747217,747477)`，不对原生返回值额外解析为 JSON。

该分支后续测试应分别覆盖缺少初始化实例、无 MF 匹配仍计算空 raw ID、路径缺少 `#{`、空／错误类型 salt、非空 salt 缓存重用、UTF-8、零长度更新、各 BCrypt／MF 失败、空串与 `{}` 的结果消费者和全部释放分支。此处仅完成所列静态证据和语义核验，未新增 Rust 行为测试，未执行官方 DLL／真实设备对照，也未执行 UI 窗口验收；这些验收均保持 Unverified。

### 当前宿主的分配器、解码与释放消费者

相机基类实际是 `6120` 的 module `7846`，`callDLLApi` 调用 module `99494.uu`；后者直接等待 `apiElectron.doDLLMainActionAsync`。当前宿主 `preload.js` 将其转成 `ipcRenderer.invoke("ffiPreloadAsync",channel,arg)`，`main.js` 的对应 handler 调用 `FFIPreloadMain.callDLLAsync`，不是旧 `ffiMain.js` 或子进程路径。该链及以下依赖的原件身份、ASAR 内实际字节／签名后 unpacked 依赖关系、完整 AST 所有者和 native 函数均记录在同一[身份转换收据](../../../.analysis/results/native-bulk/camera-0e06-identity-verification.json)的 `allocator_host_followup` 字段。

当前 `3590` wrapper 的 ConfigureFFI 声明含 `FreeMalloc` 及两个返回 `pointer` 的身份导出，**不声明或调用 `SetAllocator`**。宿主 ConfigureFFI 只调用 `ffi-napi-rz.Library(dllPath,apiObj)` 并登记 channel；registry 的 `registerMainDLL` 仅维护 Map。实际 `library.js` 按 `apiObj` 的键解析导出并建立函数，不调用这些业务函数；`dynamic_library.js` 的加载入口在当前 native addon `0xb010` 做 UTF-8 路径转换、加载前检查后调用 `LoadLibraryW`，符号解析 `0xb1b0` 调用 `GetProcAddress`。这些已核实的当前调用方没有暗中为相机设置自定义分配器。

相机 `SetAllocator(0x4e0c0)` 只把 `RCX` 存入全局 `0xbc248` 并返回低字节 1，无签名／来源检查、不释放原分配器，也不追踪尚未归还的缓冲区；传零仅改回默认分支。该槽位在 PE `.data` 的零填充虚拟尾部，不能把它当成原件文件内有 8 字节零。`Terminate(0x4e260)` 仅日志并返回 1，没有清空此槽。当前已核实的 wrapper 没有为该 setter 安装或注销 callback 的生命周期；这与导出保留了一个可供其他调用者使用的能力不同，不能凭导出存在要求 Rust 常规路径安装 callback。

默认分配为 `0x6a0e0 → 0x71570 → HeapAlloc(hHeap,0,size)`，释放为 `FreeMalloc → 0x69830 → 0x71510 → HeapFree(hHeap,0,pointer)`；两者读取同一全局 `0xbbf60`，CRT heap 初始化 `0x7bbf0` 将其设置为 `GetProcessHeap()`，结束入口 `0x7bc20` 将其清零。默认分配失败返回 null，HeapFree 失败写入 CRT 错误状态；`FreeMalloc` 仍固定返回低字节 1，且 JS 的签名为 `void`，因此不能把该返回或 finally 已进入当作真实释放成功确认。自定义分支只替换分配函数、**没有配对 deallocator**；任意自定义内存不能因此认定可被同一 process heap 释放，也不能把本源码当作安全通用 allocator 接口。

`FFIPreloadMain.callDLLAsync` 先取得同 channel 的 Mutex，ConfigureFFI 成功后把普通动作交给 `_callDLLFunctionAsync`。后者等待真实 `.async` callback，callback 只 resolve 第二个参数，未把第一个 `err` 变成 rejection；返回对象若有 `readCString`，先执行字符串读取，**读取成功后**才设置释放标志，finally 再直接调用同库的 `FreeMalloc(rawPointer)`。正常空字符串与 `{}` 也走该释放；readCString 抛错时标志仍假，不能描述为已释放非空指针。FreeMalloc 自身抛错由外层 catch 记录，已解出的字符串仍可能返回调用方；原件没有独立清理成功状态或自动重试。

编码依赖已追到当前实际 `ref-napi` 原生 addon：`ref.js` 的 `Buffer.prototype.readCString` 和 `PointerBuffer.prototype.readCString` 均将指针与 offset 交给 native `readCString`。该 addon 注册函数 `0x6c10` 把回调 `0x67c0` 绑定到属性 `readCString`；回调经 `0x3900` 取得实际指针和 offset，拒绝 nullptr，扫描到 NUL 后明确调用 **`napi_create_string_utf8`**。因此这一版身份字符串的宿主解码实际为 UTF-8，不再只凭 JS 方法名或注释推断；扫描并未使用身份导出的分配容量作为上限。native addon 的函数 Image Base 同为 `0x180000000`，其 RVA 与相机 DLL 的 RVA 属于不同原件，不能混用。

宿主 `FreeFFI` 的 async dispatch 直接返回 true，`_handleAction_FreeFFI` 为空；并未调用依赖库已提供的 `dllObj.close()`。因此单次结果指针的释放与卸载 DLL、清理 callback Map／library Map 是不同操作。相机普通 terminate 也没有清空 allocator；此处补清了当前正常读取的静态释放消费者及其异常分支，尚未以执行证明每次 HeapFree、库卸载或 callback 排空成功。后续差分用例需覆盖 decode 抛错、FreeMalloc 失败、callback err／返回值组合、同 channel 并发、配置复用及退出时在途调用；本轮仍未执行 DLL 或 UI。

## 3590／0E06 自动与手动对焦的实际写回链

本节只核当前保留的 `3590` UI／MW 与 `0E06 1.0.13.1`，完整来源、AST 所有者、原件字节与逐函数核验保存在同一[相机收据](../../../.analysis/results/native-bulk/camera-0e06-identity-verification.json)的 `focus_writeback_followup`。25 个完整 AST owner 及 29 个完整 native 函数的 2,513 条指令、10,670 个指令字节均已核对；完整块共 10,699 字节，其中没有独立指令项的 29 字节 `90/cc` 单列保留。以下严格区分页面配置、任务完成、COM 返回与真实设备读回，不以相同名称推广其他摄像头版本。

### 页面输入与提交时机

当前 Customize 描述符 `[5239677,5239739)` 给 `$S` 传 `supportsFocus:true`；`$S` `[5074262,5074564)` 先挂载 `wS`，后挂载图像组件。`wS` 将 Redux 的 `camera`、预览／占用／loading／Windows 属性页状态交给实际 `YS` `[5053541,5063321)`。自动开关复制完整 camera，仅在 `auto/manual` 间翻转 `cameraMode`；手动滑块仅在数值不同于 `cameraValue` 时复制并提交完整 camera，范围 `0…255`、step `1`。自动模式隐藏手动滑块；没有本行的 Apply、Save、Cancel 或失败回滚动作。

对焦容器在第三方占用、视频尚未播放或 Windows 属性页打开时带 `disabled`；开关另以预览关闭／loading 设置 `disable`，手动滑块的 `active` 为预览开启且非 loading。`NS` `[5049495,5049602)` 先 dispatch `SET_CAMERA_INFO`，再取一份 default profile、替换其 camera 并 dispatch `UPDATE_PROFILE`。`webcamReducer` `[4794512,4798868)` 立即替换页面 camera、广播 `ON_SET_CAMERA_INFO` 并登记任务状态，**此处没有等待设备确认再更新开关／数值**。Reset 是显式提交 `DEFAULT_SETTINGS.camera` 与图像动作；不能把普通打开、读取失败或断开等同于用户 Reset。

共用滑块 `RS` `[5043152,5049444)` 的 change、滚轮、按键及 keyup 都可能调用父回调，父层的相同数值检查才抑制重复数值；不能擅自改成仅松手提交。点击滑块注册 window 的 wheel／keydown／keyup，`onBlur` 删除这三项，而 `componentWillUnmount` 只删除 mouseup。该释放差异是当前源码事实，尚未执行测量监听残留。预览的代次只拦截迟到的媒体流，不能据此声称对焦 DLL 请求也被取消。

### 初始读取、配置重放与结果消费者

当前 MW `main.48235a32a00742140113.js` 的 runtime 标记 `isCamera:true`，派生 feature 是 `isCamera:{config:{}}`，末尾产品覆盖没有启用 `isAdvancedCamera`。`6120` 的缓存读取 `Qe` `[2315989,2317390)` 先并行等待 brightness／contrast／saturation／whiteBalance；**只有高级相机 gate 才继续读取 zoom／pan／tilt／focus／exposure／low-light／anti-flicker**。本产品旧款分支只发布图像读取结果。包装器 ConfigureFFI、GetDLLVersion 和 DeviceInitialize 重试链也没有调用 GetCameraFocus，不能把当前页面显示的 `cameraMode/cameraValue` 说成启动时已读得的对焦状态。

另一方面，profile 重放 owner `[1401821,1432893)` 会把保存的 camera 发布给 UI，并排入 `ON_SET_CAMERA_INFO`；这是配置意图引起的真实写请求，不是设备观察结果或首次初始化判定。当前默认 camera 为 `cameraMode:"auto" / cameraValue:128 / cameraEnabled:false`，没有 `zoom` 字段。该原版重放行为需要与本项目的实测状态／配置／草稿规则分别建模，不能仅因配置存在就标记设备已确认或补出无依据的默认设备状态。

相机事件 dispatcher `[1291673,1347617)` 将决策中的 `updateLocalStorage` 与 `deviceAccess` 交给 task maker `Nn` `[1610646,1611468)`。有 `deviceAccess` 时，后者读取本地 cache、递增版本并替换活动 profile 的完整 camera；有 `updateLocalStorage` 时先等待存储 helper，再把动作排入 `NORMAL_SKIPPABLE / imageSetting`。这些存储步骤发生在原生写结果之前，不能当成写回成功确认。

实际 runner `on` `[1780473,1781159)` 先检查一次 abort，再等待 `setCameraFocus(cameraValue,cameraMode==="auto")`。它**不检查 setter 返回的 false**；对当前 `28836` 的 Kiyo X 实例还会等待 `setCameraZoom(camera.zoom)`，包装器原样转发 zoom、固定第三参数 false。默认 camera 没有 zoom，不能在逆向结果里擅自将缺字段补成 0。随后 runner 直接置 `completed`、更新 memory camera cache 并写 memory storage；本链没有调用 GetCameraFocus 读回。result consumer `rn` `[1781159,1781681)` 将 `completed`，甚至 `error==="skipped"`，发布为完成；因此任务完成与 COM 成功、设备状态确认均非同义。

共享任务 owner `[5768,10741)` 对 skippable 类别取最新 pending 项，其他 pending 项调用 skipped consumer 后释放。取消尚未取得 Web Lock 的任务可得到 AbortError；锁内任务开始后，`cancel` 只 abort controller，本 runner 不在原生 await 后再次核信号，任务 finally 仍调用 result consumer。pending cancel 传入 `{value:null,error:"canceled"}`，而 `rn` 的错误分支直接读取 `a.value.params`，存在静态对象形状不匹配；此处没有执行断言其实际异常时机。取消、跳过、旧连接结果和 native 强制停止必须分别测试，不能统一记作没有写设备的成功取消。

### 原生 COM、确认边界与断连释放

包装器明确声明 `GetCameraFocus:[pointer,[string]]` 和 `SetCameraFocus:[bool,[string,long,bool]]`。getter 在 `isInit` 时 JSON.parse 返回字符串，null／空串或未初始化时返回空对象；该 guard 不是设备读取成功。`GetCameraFocus(0x4ea30)` 精确查已初始化 container，再把 CameraControl 属性 **6** 交给 `0x49860`；缺实例或读取失败返回已分配的 `{}`。成功编码 `0x49f00` 将 signed long 经 `%d` 写入 **JSON 字符串** `value`，`isAuto` 也为字符串，且仅 flags **恰好等于 1** 时为 `"true"`。不能将 getter 的未知 flags 当成已证明合法的手动状态，也不能把 `{}` 补成本地默认值。

getter task 原 PE 虚表 `0xa1970+16 → 0x4c300 → 0x49620`，setter 为 `0xa1b50+16 → 0x4c2b0 → 0x49560`。两者从设备 `+40` 的 base filter QueryInterface；IID 原字节 `0x8e730` 为 `c6e13370-30ac-11d0-a18c-00a0c9118956`。getter 调接口 `+40` 的 Get，setter 调 `+32` 的 Set；属性均为 6。setter 将 int32 value 原样传递，flags 按 `(bool_byte ^ 1)+1` 计算，正确 bool 的自动 `1→1`、手动 `0→2`；没有范围查询、clamp、HID 应答或后续 getter。QueryInterface 与 Get／Set 以 HRESULT 非负为成功，已取得的属性接口随后 Release；**该 bool 只承载这一 COM 完成判断，不证明镜头位置或固件状态已经读回确认**。

属性任务经过 `0x48890` 排入专用线程；`0x48340` 在该线程 CoInitialize(0)，正常退出配对 CoUninitialize，初始化失败记录后退出。任务等待上限 4,000 ms；超时分支清 active、detach、TerminateThread 并关闭所打开 handle；超过 500 ms 的属性调用只记录耗时，没有自动重试。强制终止可能绕过正常 COM／任务清理，不能描述为协议取消或恢复原值。上述行为均由完整函数和原 PE 的 task 虚表槽核对，未执行原生线程。

`DeviceTerminate(0x4e470) → manager 0x41b00` 按精确 container 停属性窗口，先 Release／清零 base filter，**再**调用 executor Stop `0x48cf0` 等待线程，随后删除实例与列表项。join helper `0x5697c` 为无限 WaitForSingleObjectEx 后 CloseHandle；自 join 和 join 失败进入异常，不能以出口常量真概括所有路径。析构 `0x49080` 仍有停止和队列释放链；本节所选 getter／setter 从 manager 找到裸实例到 COM 使用间未见会话代次或共享所有权保留，且 filter 释放先于 executor join，因此不能据静态正常释放路径宣称并发断连已安全。

本节补齐所列对焦输入、配置、消费者、COM 及取消／释放的静态证据；没有修改 Rust，没有执行厂商 JS、DLL、主应用、窗口或摄像头。后续需逐项覆盖独立设备值与配置不同、自动／手动及边界、字符串／未知 flags、getter 失败、setter false 仍 completed、缺 zoom、跳过与两阶段取消、断连、HRESULT 失败、4 秒超时、恢复和监听清理。官方 DLL 与独立 Rust 的实际功能对照、真实写入／读回／原值恢复及窗口验收全部保持 **Unverified**；捕获数量和本节闭合不代表整个产品或 DLL 语义已完成。

## 3590／0E06 与 3589／0E05 的白平衡读写和页面门禁

本节沿当前保留的两款 UI／MW 与各自原生库逐项核对白平衡，限定为 `3590／0E06 1.0.13.1`、`3589／0E05 1.0.15.1`。完整 AST、原 PE 字节、函数与实际虚表槽记录在同一[相机收据](../../../.analysis/results/native-bulk/camera-0e06-identity-verification.json)的 `white_balance_writeback_followup`；45 个新增完整 AST owner、11 个复核的既有 owner，以及两库合计 24 个完整函数的 1,843 条指令、8,277 个指令字节通过 8 线程有界核验。完整函数块 8,289 字节，其中无独立指令项的 12 字节另列保留。静态语义、页面执行、官方 DLL 实际对照与真实设备验收分别记录，不由同族名称推定其余分支已验证。

### 白平衡开关、范围与配置冲突选择

两版完整图像 owner 分别位于 UI UTF-8 字节 `[5067069,5074261)` 与 `[5064528,5071720)`，正文相同。其中 `id:"autoFocus"` 的 checkbox 和 `V(...,"setFocus",...)` **实际修改 `whiteBalance.isAuto`，不是 CameraControl Focus**；它保留白平衡数值，手动改数值则保留 `isAuto`。3590 的产品参数为 `2800…6500 / step 1`，3589 为 `2000…7500 / step 10`；各自默认 image 的白平衡为自动、数值 `5000`／`4000`，不能用共享组件的 fallback 覆盖产品参数或把配置默认值标为设备观测。

手动 stepper 需 `!isAuto && !isSettingChanged`；手动 slider 另需预览开启、非 loading、非第三方占用。图像行在预览关闭、loading 或第三方占用时带 `disabled`，Windows 属性页打开则禁用整个图像面板。编辑先更新组件状态，必要时切到 custom，再提交完整 image；当前绑定的 action 集合没有单字段 `setWhiteBalance/setBrightness/setContrast/setSaturation`，所以不能凭 `V` 中留有这些可选调用就描述成当前挂载的独立 setter 路径。

冲突弹层仅提供 CAMERA／SYNAPSE 两项。CAMERA 把 `currentImageSetting` 转为 custom，SYNAPSE 取所选配置 dataSet；**两项均可继续走 `setImageSetting` 的写动作**，不是 CAMERA 选择纯采纳读值而绝不写设备。差异 effect 遍历 object 字段时，双方 auto 都真或双方 manual 且 value 相同会直接 return 整个 effect，不能说所有剩余字段因此都一致。该 owner 没有通用 Cancel 或失败自动回滚。

### 初始读取与配置重放顺序

3590 的完整 setup `gn` `[1373141,1390492)` 在本地 profile 准备 `vn` 后等待 `L.rf/Qe`；该读取依次发起 brightness、contrast、saturation、whiteBalance 并等待同一 `Promise.all`，并非串行等待前一属性。后续才经 `Dn → An → Mn` 应用 profile；Mn 的 `imageMode/image` 分支又会排入 `ON_SET_IMAGE_SETTING`，因此启动读得的状态与随后重放的配置意图必须分开。此顺序只证明该已核实 setup 路径，不推定所有 resume 路径均重新读取。

两款 Qe owner 的正文与哈希相同；读取结果通过 `MW_SET_CURRENT_IMAGE_SETTINGS` 到 UI 的 `currentImageSetting`，不是替换配置 image。原版转换使用 `Number(value||0)` 和 `JSON.parse(isAuto||"false")`，wrapper 又在未 init、null 或空串时给空对象；这些均不能在本项目冒充已观测的设备 0／手动状态。Qe 捕获异常仅记录日志，外层 setup 仍可继续，不能把流程继续当成初始化读取成功。所选产品的相机 feature config 均为空、没有启用高级读取 gate，不可从共享高级组件存在推定调用了曝光等额外 getter。

### 整组写任务的结果边界

当前 `setImageSetting` action 先 dispatch 图像配置并更新 profile；reducer 立即替换配置、广播 `ON_SET_IMAGE_SETTING`，不是等待设备确认。MW task maker 先读取 cache、递增版本并修改活动 profile，在 `updateLocalStorage` 为真时先持久化，随后才排入 `NORMAL_SKIPPABLE / imageSetting`。实际 runner 按 **brightness → contrast → saturation → whiteBalance** 顺序 await；白平衡仅在对象存在时调用，参数为原样的 `value,isAuto`。

两版 task maker、runner 与 result consumer 的完整正文分别相同；task maker 的绑定标识实际为 `I6_.volumeSetting / DEk.version`，传入的是原输入版本 `l`，不是刚写进 cache 的新版本 `c`。这些标识和版本不能因任务名为 imageSetting 就自行改写成另一种源码语义。

该 runner 忽略四个 setter 的 bool；即便其中抛异常，catch 只记日志，**仍将任务置为 completed**，而前面的异常可使后面的白平衡根本没有执行。result consumer 只输出日志，没有 getter 读回、设备确认或先前属性恢复。故单个 false、部分成功、白平衡未调用、任务 completed 和配置已持久化必须分别建模。共享任务的 pending 替换、执行前取消和执行后 abort 继续受上节已核实的队列语义约束，不能把 abort 或 skipped 当成已恢复设备。

### 原生属性与产品差异

两库白平衡均为 VideoProcAmp 属性 **7**：0E06 的 `GetWhiteBalance 0x4f540 → 0x49bb0`、`SetWhiteBalance 0x4eff0 → 0x49d60`，0E05 的 getter 为 `0x4fa70 → 0x4a0d0`、setter 为 `0x4f500 → 0x4a290`。0E05 两个排任务入口均先检查 base filter；0E06 排任务前只在 getter 检查两个 out pointer，filter 检查留在 COM 执行 helper。两者断连与失败发生时机不同，不能只共用属性编号就认为行为完全一致。

原 PE 的 VideoProcAmp IID 两版均为 `c6e13360-30ac-11d0-a18c-00a0c9118956`，不能误用对焦的 CameraControl IID。0E06 getter 虚表为 `0xa1a58+16 → 0x4c260 → 0x497a0`、setter 为 `0xa1b18+16 → 0x4c210 → 0x496e0`；0E05 对应为 `0xa09b8+16 → 0x4c210 → 0x49ca0` 与 `0xa0ab0+16 → 0x4c1c0 → 0x49be0`。每张表的 `+0` clone 也按各自原件核实，不能因两库出现相同 RVA 而交叉套用函数。

setter 原样传递 int32 数值，自动／手动 bool 分别映射 flags `1/2`；COM Get／Set 以 HRESULT 非负判断成功，随后释放取得的属性接口。成功 getter 把 signed 数值和 `isAuto` 编成 JSON **字符串**，仅 flags 恰为 1 时为 true；无实例或读取失败返回 `{}`。本链没有 Set 后自动 Get、硬件 ACK、范围查询或数值 clamp；这些返回与页面 completed 均不能替代真实设备读回和恢复确认。

两库任务等待仍为 4,000 ms，强制终止不构成属性恢复。当前 0E06 实际导出表没有 Exposure 命名导出；共享高级曝光组件与 Windows 属性窗口是不同入口，不能凭前者推造 `3590 → Get/SetCameraExposure` 调用链，也不能据此宣称 Windows 属性窗口中的系统控件已完成核对。

本节为静态证据收束，不执行厂商 JS、官方 DLL、主应用或设备；独立 Rust 行为测试、官方实际功能对照、真实写入／读回／原值恢复及窗口验收仍为 **Unverified**。

## 3590 旧款预览的迟到结果与卸载边界

本节补的是已读完整 owner 中的具体异步缺口，不将前述预览概要重复当成完整释放证明。6 个完整 AST 节点和来源身份保存在同一相机收据的 `legacy_preview_late_result_followup`；只覆盖当前 `3590` 原件，没有把结果推广到其他产品或宣称宿主／原生断连链已闭合。

`YS` `[5053541,5063321)` 的启动 `Z` 在初次枚举后及 `getUserMedia` 返回后比较代次，后者迟到时停止刚取得的 tracks。但**`await video.play()` 后没有再次检查代次**：等待期间若 `Q` 已停流并清空 `srcObject`，旧成功仍可能随后设置 playing=true；旧 play 失败则停止其捕获的流、直接清视频元素的 `srcObject`，没有判断该元素是否已挂上更新的流。这是完整源码可构造的控制流分支，尚未运行重现。`Z(true)` 还可绕过“已有 srcObject”检查重新获取流；Z 内替换前没有独立停止旧流，不能只凭调用了 reload 就认定原流已释放。

初次 `await X()` 媒体枚举位于后续 try/catch 之外，发生在 in-flight 标志置真之后；其 rejection 不经过该 catch 的清标志分支。停止 `Q` 会先递增代次、清 in-flight，再处理视频元素及 tracks；找不到元素时仅清 playing 后返回，单个 `track.stop()` 异常被吞掉，均没有逐 track 的 ended 确认。应分别测试枚举失败、捕获迟到、play 迟到、缺元素和停止异常，不能把这些统一标成正常关闭。

挂载 effect 的卸载回调只调用折叠窗口 owner `Ae`，**没有调用 Q 或递增媒体代次**。焦点定时器 cleanup 只清尚未触发的 timer；已经进入 `getWindowStatus` await 的回调没有卸载后的复核。焦点 hook `fS` `[5050887,5051749)` 尝试调用 response 返回的取消函数，但其 `HS/getFocusTab` 异步结果也没有卸载／代次 guard；本节没有继续推定宿主 response 的实现或监听排空。媒体的 `getUserMedia/play/track.stop` 与 DLL 的 DeviceTerminate／卸载是不同边界，本 owner 不会因关预览就自动证明后者已执行。实际设备、窗口与对应恢复／释放验收仍为 **Unverified**。

## 收据附录：共享原生捕获失败的静态证据

本附录记录同一维护收据中的原生失败审阅，不将音频驱动函数归为相机能力。完整原件身份、原字节、边界与交叉引用在[相机收据](../../../.analysis/results/native-bulk/camera-0e06-identity-verification.json)的 `native_failure_review`。原 47 项导入 ABI 计划保持原样，另有 2 项共享帧合并计划。证据核验使用 8 个线程；两份 x64 原件已由统一 IDA 队列在私有数据库副本完成合并与反编译，下面分别记录已恢复部分与仍待边界处理的部分。未执行目标、DLL 或设备。

### 24 个 positive-SP 失败的字节分区

两份 x64 原件各有 12 个 outer、27 个内部 CALL／RET 子程序，共 24／54 个。每个实际 outer 均逐项核对原 PE、指令字节和完整 chunks；内部区间与外层余区间无重叠，字节并集严格等于原 chunks，保留全部对齐字节。两原件相同 RVA 的这些 outer 原字节一致，形成下列 4 个模板；内部区间相对 outer 入口，右端不含。

| 模板 | 每原件 outer 入口 RVA | 字节数 | 内部相对区间 |
| --- | --- | ---: | --- |
| A | `0x284a0 / 0x2df40 / 0x338c0` | 367 | `[0x60,0x118)` |
| B | `0x28610 / 0x2e0b0 / 0x33a30` | 676 | `[0xa0,0x234)` |
| C | `0x2a370 / 0x2fe10 / 0x35790` | 2,602 | `[0x184,0x207)`、`[0x497,0x552)`、`[0x7bf,0x88a)` |
| D | `0x2ada0 / 0x30840 / 0x361c0` | 4,248 | `[0x189,0x2d7)`、`[0x554,0x792)`、`[0x9f1,0xb49)`、`[0xd8f,0xf13)` |

完整分区、每段原字节及真正 CALL／跳过正文的跳转保存在 `mixer_boundary_partition_review`。代表模板的 38 条边现已扩展逐项核对为全部 **228 条真实 E8 CALL**，均进入对应内部入口；原 PE 的 CALL 位移、返回位置及全部指令字节已核实。内部 CFG 同时追踪条件分支两后继，**不能在首个 RET 截断**：`0x2af29` 的 RET 位于 `0x2b02f`，后面仍有跳回主体的负值处理块，完整区间必须到 `0x2b077`。以上尚未应用为 IDA 新边界。

### 9 种内部模板的真实寄存器结果

`mixer_internal_abi_review` 保存全部输入、写集、相同字节实例和 caller 消费者。内部子程序不是普通 C ABI，不能仅选单个显眼结果而丢弃附加状态。

| 代表入口 RVA | 读取的输入寄存器 | caller 实际消费结果 |
| --- | --- | --- |
| `0x28500` | `XMM0,6,7` | `XMM4` |
| `0x286b0` | `XMM0,6,7,8,9,10` | `XMM2`；重建 `XMM6/7` 供下一轮调用 |
| `0x2a4f4` | `XMM0,1,8,9,10,13` | `XMM2` 与累计 `XMM13` |
| `0x2a807` | `XMM0,1,8,9,10,11,12,13` | `XMM2` 与累计 `XMM13` |
| `0x2ab2f` | 同上一项加 `XMM14` | `XMM2` 与累计 `XMM13` |
| `0x2af29` | `RCX,R8…R15,XMM8/9`；保留或条件改写 `RAX` | `XMM2` 与累计 `RAX` |
| `0x2b2f4` | `EAX,XMM0…3,8…11` 及两段共享栈 | `XMM2` 与累计 `EAX` |
| `0x2b791` | 同 `0x2af29` 加 `XMM10` | `XMM2` 与累计 `RAX` |
| `0x2bb2f` | 同 `0x2b791` 加 `XMM11/12` | `XMM2` 与累计 `RAX` |

C 类三个模板逐次将输入 OR 入 `XMM13`，outer 最终在 `0x2ad47` 执行 `pmovmskb EAX,XMM13` 并保留 `0x8888` 位。D 类整数输入模板遇负值写 `RAX=1`，其余路径保留此前值；outer 先将 RAX 清零，结束仍消费该状态。D 类 SIMD 输入模板以 `OR EAX,EBX` 累积状态，outer 最后保留 `0x8080` 位。

`0x2b2f4` 三次栈读原字节为 `660f6f742408`、`660f6f7c2418`、`660f6f742408`，对应 helper 入口 `RSP+8` 的 16 字节、`RSP+0x18` 的 16 字节及重读前者。caller 先在自身 `RSP`／`RSP+0x10` 写常量，CALL 压入 8 字节返回地址后精确对应。全部 54 个 helper 均无显式栈调整、栈写、PUSH／POP 或嵌套 CALL，各有唯一 `C3`，只弹出 8 字节返回地址，没有额外参数清栈；仅上述模板的 6 个实例有合计 18 次数据栈读取。A／B／C／D 的 caller 帧相对 outer 入口分别为 `-0x38/-0x68/-0xa8/-0x108`，helper 入口再减 8 字节。完整逐体核验在 `mixer_internal_outline_preflight`。

这些是保留 caller 寄存器及共享帧的真实局部 CALL／RET 体。当前字节全部嵌在 outer 的完整 chunk 中，普通合并不能解决其局部调用语义。后续可在私有 IDB 中精确分区并标记 `FUNC_OUTLINE`，由反编译器在 caller 内处理原有寄存器、共享栈和多结果，而不是编造普通 ABI 或丢弃累计状态；该方案尚未运行、尚未移除 24 条 positive-SP 失败。SIMD 浮点仍依赖 MXCSR，也未执行计算对照。

### x64 共享帧与 x86 错误路径

两份 x64 原件的 `0x7fa70 / 0x92960 / 0xaff50 / 0xcd650` 均在调用 `0x7f620` 时失败。捕获将连续控制流拆成 `[0x7f620,0x7f653)`、`[0x7f653,0x7fa5c)`、`[0x7fa5c,0x7fa64)` 三片：首片 `SUB RSP,0xd8`，条件跳到末片或落入正文；正文使用已有帧保存／恢复非易失寄存器，再落入 `ADD RSP,0xd8;RET`。两个内部片入口没有真实 CALL，不能沿用其伪代码推得的 25 参数。

原 PE 的 `.pdata` 确实登记三段，三个 unwind flags 均为 **0**，**不含 `UNW_FLAG_CHAININFO`**；三者均描述同一 `0xd8` 栈分配。三片全部捕获字节／指令匹配原件，无缝并集 1,092 字节，两库相同，SHA-256 为 `795315791cc651960400406c59fdd6c2c139192c2b34977d1049284d75b2494a`。完整证明在 `mixer_x64_shared_frame_merge_review`。统一队列已在两份私有数据库中完成精确合并：每库 `0x7f620` 及 4 个 caller 均保存完整汇编和非空伪代码，合计恢复原 8 条 caller 失败；`mixer_shared_frame_merge_result` 逐项保留两个 manifest、10 个完整函数及 expected/applied chunks 一致证据，原 capture 和原数据库当前字节哈希均未改变。

完整 ABI 由每库 10 处真实 CALL 核实：RCX 为原地读写 float buffer、EDX/R8D 为 signed int32、R9 为只读 float 表。第 5 参数槽在 caller 的 `RSP+0x20` 写 DWORD 0，callee 不读；第 6 槽在 caller 的 `RSP+0x28` 写 signed int32，callee 的入口 `RSP+0x30` 读取，值为 `0x100/0x400`。返回前 EAX 是零扩展到 RAX 的原始整数运算载体，部分 caller 经 epilogue 继续透传，**不是 HRESULT、成功状态，也不能概括为 void**。`0x7f579` 的 CALL 位于 base 捕获未覆盖的 `[0x7f4c8,0x7f5b2)`，补充的 LLVM 静态反汇编与原 PE 字节单独保存在 `mixer_merge_abi_followup`，不冒充已恢复的 IDA 函数。

x86 的 4 条失败共同指向 `0x92e27`：真实输入为 `EAX` 错误种类、`EDX` 操作、`ECX` 名称指针和 `ST0` 结果，返回 `ST0`。helper 入口 `ESP+4` 保存 16 位 x87 控制字并将其地址继续传给错误处理；`ESP+0xc/+0x10` 是原始 double 两半，`ESP+8` 是外层返回地址，不能当数值实参。返回前按真实分支恢复控制字。`0x92f45` 本身是入口栈顶已有控制字的退出尾块，不能假设普通独立函数序言。`mixer_call_analysis_static_review` 保留 12 个失败 caller 与 3 个 x86 handler 的原件字节核验及指令。

### 65 个多精度 caller 的完整 outline 候选闭包

同一收据的 `shared_multiprecision_outline_preflight` 保存 13 份原件的完整审阅与 13 项新计划，位于 `plans[49…61]`（从 0 计数）。每份均核对全部 5 个失败 caller 及 **6 个**真实直调 helper，合计 65 个 caller、78 个已有独立边界的 helper、338 条 E8 调用；不是只处理第一处报错或原来列出的 4 个 callee。78 个 helper 的完整 chunks、指令、入口 flags 与源 PE 字节由维护选择器逐项核实，13 项选择 smoke 均通过；尚未启动该批 IDA 或据此恢复失败。

常见布局的 6 个 helper 为 `0x22a20 / 0x23380 / 0x23c40 / 0x23de0 / 0x24740 / 0x250a0`，另一布局按其实际重定位入口逐项记录。`0x23380` 和 `0x24740` 有自身返回，另外两条真实 E9 为 `0x22a20 → 0x23c80` 与 `0x23de0 → 0x250e0`；目标分别位于 `0x23c40`、`0x250a0` 正文中部，13 库共 26 条共享 continuation 边。两段 continuation 的所有者也纳入每项 6-helper 计划，没有伪造新的 `0x23c80/0x250e0` 独立边界、删除跳转或用普通 ABI 掩盖寄存器和共享工作栈。

候选处理仅在私有 IDB 的既有 helper 上标记 `FUNC_OUTLINE`，输出目标仍是 5 个 caller；不与边界合并混跑，不改任何机器指令。真实的中部外跳可能仍限制反编译恢复，必须以实际 manifest、完整函数、原字节及原捕获／数据库未改证据验收；smoke 或 outline 标记成功均不等于 65 个原失败已消除，也不等于算术行为已与官方执行一致。

本附录中只有明确记录的两份 x64 共享帧合并与 8 条原失败恢复已完成；24 条 positive-SP 分区、x86 错误路径、多精度 outline 候选及其余失败仍分别保留。独立 Rust、UI／后端接线、官方 DLL 实际功能对照、真实设备和运行验收均未由本轮静态恢复证明，继续保持未完成／未验证。

