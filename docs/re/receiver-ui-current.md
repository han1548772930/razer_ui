# 接收器 UI 当前契约

本文记录正在维护的接收器界面、观察边界与防回归条件。源码依据为当前产品包、当前 Dashboard 和 host 4.0.827；不以旧过程报告或相近产品的界面推定完成。

## 页面与源码

179 的 Windows Customize 主树为 `9473/mE → Te + Indicator`。`Te` 是配对父卡，`G/se → re → ie/oe/ne/ae/_e` 是它打开的工具弹层；两者都必须挂载。源文件为 `.ref/devices/179/static/js/main.4849f7ca.js`，SHA-256 `f5d6efe15f34167d61020469f151c3a0827f695cb35bc49cbdd32c23957fb874`。

164/241 使用各自的 `DockPairing` 路径，207 的当前描述符为 Seamless Auto Pairing 与 Battery Health Optimizer。179 的修复不能代表这些页面完成。`hyperpolling-pairing` 泛型 renderer 没有当前描述符引用，179 实际使用专用 `render_receiver`。用户尚未指定接收器型号，不能将其问题唯一归因于 179。

179 默认导航只有 Customize/Help，源普通根明确 `renderProfileBar:false`，因此保留头部空间但不显示配置文件下拉；共享 bundle 中的宏 ProfileBar 不是该普通根的依据。164 默认 Customize/Lighting/Help，`multiDevicePairing` 为独立根；241 默认 Pairing/Lighting/Help。

维护证据：

- [父卡和配对工具](receiver-pairing-current-evidence.json)：实际 AST、10 种产品语言、115 条 CSS、配对图片和原生文件指纹。
- [主树与指示灯](receiver-current-evidence.json)：挂载链、108 条 CSS、原始 SVG 动画层。
- [179 头部条件](receiver-profile-header-current-evidence.json)：普通根与 Header 的实际 ProfileBar 门控。
- [底层查询协议](receiver-discovery-current-evidence.json)、[he/Ne 与 host 发布链](receiver-publishing-review-current-evidence.json)、[产品身份与名称](receiver-catalog-current-evidence.json)。
- [硬件连接事件](receiver-connect-events-current-evidence.json)：事件生产者、过滤条件、广播与 UI 消费者共 11 项 AST。
- [164/241 配对交互](receiver-interactions-current-evidence.json)、[Lighting/Help](dock-lighting-help-review-current-evidence.json)为各自独立证据，不能由路由登记替代界面核验。

## 已实现的 179 页面行为

父卡具有空、待确认和已确认绑定三种内容。名称来自真实 payload，按源的本地产品名、本地名称、英文产品名、英文名称顺序回退，再执行源大小写规则；完全缺失时明确显示 PID。没有 edition/layout/serial 就保持缺失，不补造精确设备图或设备就绪状态。

父卡按源显示 44px 图标、369px 描述区、条件名称链接与 Unpair。正常 Unpair 使用 27px 高度/行高及 `#cfcfcf` 边框；加载时使用 28px 高度/行高、`#5d5d5d` 边框、0.8 透明度并禁用。英文已确认分支宽 90px。Unpair 只打开工具，不能直接执行设备解绑。

待确认名称旁的 Skeleton 使用原始 80×16 SVG 拆出的底层与渐变层，原生动画为 1.5 秒从 -80 到 80 平移及 0.5 秒停顿；减少动态效果时静止。原图和两层都记录源/输出哈希，渲染资源已注册。

工具保留加载、类别选择、候选选择、进度、配对结果、解绑确认与错误内容。Scan/Bind/Unbind 产生可撤销的未发送本地意图。只有真实 publisher 回传且代际、操作类型匹配的结果才能改变设备结果。

真实 Bind/Unbind 成功后延迟 1 秒关闭工具；普通绑定查询、候选结果或本地点击不会触发成功关闭。真实 Bind 失败后 4 秒恢复空的 Ready 内容；Unbind 失败后 4 秒恢复已观察的绑定。新请求、关闭、重开及取消使旧计时票据失效。

Indicator 编辑和保存属于本地草稿，不等于设备设置保存。打开工具、设备观察与页面导航不修改该草稿。

两列各 600px；外层最大 1240px，窄窗口按源折行。左卡 padding 为上/下 30px、左 40px、右 30px。Indicator 选项为 20px 外圈、10px 选中点、200ms 过渡及源 50/64/50px 行高；10 个原 SVG 层按各自时间轴合成，切换/恢复模式会重启，不能替换为一般化闪烁。

179 Widget 帮助采用其实际 portal：悬停立即创建/显示，离开立即隐藏，14px/18px 字体、内容最大宽 300px、层级 10001 及源窗口边界调整；帮助圆点背景保留 300ms 过渡。配对工具保持源 850px 宽、内容最小 850×685、顶部/宿主页签偏移、纵向滚动和标题上圆角。外部点击或 Escape 不主动关闭，关闭图标回到原焦点。加载旋转图保留源圆弧与 square cap。

## 164/241 配对与帮助边界

164 已配对名称打开配对工具；241 名称走设备导航，其源允许缺省/0 edition 匹配实际版本，非零 edition 精确匹配，PID 0 不生成链接。241 页面展示顺序为鼠标在前、键盘在后；它的可伸缩说明列、名称 17px 行高、按钮非对称 padding 与 164 的 369px 固定说明列不能混用。

正式 `DockPairingEvent`/observation 链保留本地 Scan/Pair/Unpair 意图、取消、确认、真实结果与错误。Bindings 更新父卡，读取错误和成功空数组分开；只有确实存在的 PID/edition/layout 才能选精确设备图片。查询类别/名称可按当前目录确定，不能补造 edition/layout/serial。单候选只产生 Bind 意图，不能据此显示成功。

241 轮询率说明仍需完整源条件 `V = !W && (I ? Y : K)`：I 是连接观察，Y/K 是实际 polling 配置。当前 `!both_devices_capped` 不能代替它；双设备分行还受 `J = t.length > 1 && (W || I)` 约束。164/241 的临时名称/连接提示、结果计时、独立模式入口和实窗输入仍需分别完成。

164/241 Lighting 实际挂普通快捷效果根，不能以 bundle 中同时存在的端口组件作为主体证据。亮度/熄灯范围和禁用依赖已有当前收据；效果参数、同步及 Quick/Advanced 完整主体仍未完成。164/179/241 普通 HELP Reset 允许本地确认/取消与撤销待发请求，但不改变设备状态或本地配置；其他 OBM/OLED/固件 Reset 不能套用这条契约。

164/241 的产品横幅、单/双设备信息与正式 DockDialog 使用各自 [配对源证据](dock-pairing-current-evidence.json)和资源。164 指定 zia_pairing 单设备图；单设备弹层为#111/#515151，双设备#222/#5d5d5d、左右 20px 卡片内边距及外侧 2px 选中描边。进度原 SVG 作 1 秒旋转，动作透明度保留源 300ms；这些静态声明不代表完整 viewport/滚动或实窗焦点验收。

241 配对 dongle713 的源握手为设置 duallink_warning → 设备页转 continue_pairing → 工具从当前候选中取 dongle713 以 mode1 产生 Bind 请求，随后清标记。本地保留此顺序，没有候选就保持无绑定；正式 request 发 DockPairingEvent 并等待对应观察，不能将预览进度或本地意图当已发送/成功。已有 Bindings observation 链不代表实际 Scan/Pair/Unpair 传输已接入。

页面/弹层警告先受 showBothDevicesConnectedWarning 门控，弹层另需鼠标和键盘都有绑定；页面警告为 12px #999，弹层为 14px/17px #ccc、margin-top50px、max-width520px。该条件不能替代上文完整 I/Y/K/W 轮询条件。164 工具说明键为 ENABLE_LAUNCH_PAIRING_UTILITY_INFO，241 源拼写为 ENABLE_LAUNCH_PARING_UTILITY_INFO，分别从产品自身语言表取值。validate-dock-pairing.py 已静态核对两产品当前收据、语言和资源，不能用共享公共翻译掩盖缺键。

## 宿主页签与图标

179/164/241 普通产品页签使用各自当前 HTML 的 ACCESSORY 图标。164 的 `displayMode=multiDevicePairing` 是真实独立根并覆盖 HyperPolling 图标；不能把它套到普通 164 或 179。独立配对窗口当前没有 TabUI 图标槽，不能凭资源存在制造新产品页签。

当前 host 的 `page-favicon-updated → TabUI.changeTabIcon` 接受真实页面 favicon，缺失时使用原 `rzAppEngine.ico`。产品 HTML 图标由各自声明生成，运行时覆盖则必须核实实际挂载根；Dashboard 未挂载组件中的 HyperPolling 图标不是主 Dashboard 图标。Macro、Armory、Chroma 等应用的状态覆盖各有独立生产者，不能用五类通用图代替。保留 [host](host-tab-icons-current-evidence.json)、[运行时应用](runtime-tab-icons-current-evidence.json)、[产品模式](product-mode-tab-icons-current-evidence.json)与 [ICO 准备](host-tab-ico-current-conversion.json)收据；这不代表所有产品所有模式已完成视觉验收。

## 部分发现结果与设备导航

父卡接收 `ReceiverDevicesObservation`，其中 `connected` 只包含已确认在线且有实际产品工作区的 `(product_id, edition_id)`，`complete` 表示该次发现是否完整。默认值是 `partial([])`。

| 观察 | 父卡连接状态 | 导航 |
| --- | --- | --- |
| complete/partial 均含当前 PID | 已确认在线 | 还需实际目标唯一、可匹配 edition 和已知类别 |
| complete 不含当前 PID | 已确认不在线 | 禁用 |
| partial 不含当前 PID | 未知 | 禁用 |

部分发现失败不丢弃其他已确认在线的设备；错误不能把未查明设备改成离线。整体失败不伪造成功空列表。宿主保留实际 workspace identity 对应关系，不能仅用 PID/edition 选中同型号的离线、缓存或 PREVIEW/DEMO 工作区。

PID 在线判断与绑定确认是不同条件：待确认转已确认仍要求绑定 payload 的 PID + edition 与实际设备精确匹配。缺 edition 的查询结果可以指向唯一实际页面，但不能借导航的 edition 补写 binding 元数据。断开或未知时保留既有的真实绑定显示。

鼠标导航到 Performance，键盘导航到 Customize。读取到的从设备必须经当前官方目录映射、保留真实接收器 container 关联后，才能新增或更新产品工作区。启动发现已有接收器查询；父卡后续单次 Bindings 查询向全局发现/新增产品工作区的增量发布仍未完成，不能声称全部鼠标发现问题已解决。`34340/he/Ne → host` 的发布行为是静态依据，不能通过执行其 localStorage 写入流程来实现只读观察。

## 查询与生命周期

自动父卡查询必须满足：真实 USB/HID 连接观察、合法非零且带花括号的 UUID、非 PREVIEW/DEMO 身份、工作区激活、当前为 Customize。激活、连接观察和切页都重新判定；本地缓存身份不能单独启用读取。

父卡与工具共用全局唯一请求代际。打开工具会暂停父卡请求/重试，关闭后只有符合读取条件才恢复。离开页面、停用或取消后，迟到响应不能覆盖当前状态。

待确认且已确认在线时，最多安排 30 次延迟 1 秒的父卡重读；未知或离线不能维持此重试链。错误显示明确失败和 Retry，成功空数组清除绑定。源 `Te` 忽略空数组，本实现明确区分成功无绑定与读取失败，避免把旧绑定当作当前成功结果。

## 硬件通知与仍未接入的读取

真实连接通知的源链是 `78548/Be → 34340/kO=oe → MW_RESPONSE_UI/SLAVE_CONNECT_EVENT → 9473/Te`。它要求双连接设备、异类别过滤分支、recordId 为 5 或 9、无线事件 ID 为 9 或 53、当前 PID 等于 `dualLinkPrimaryPId`，且 `eventValue.state=3`。`Te` 收到广播后显示加载，20 秒后结束；该广播本身不提供绑定 payload。

V2 查询 `status=1` 是在线结果，与硬件通知的 `eventValue.state=3` 不同。当前未接入真实硬件通知消费者，因此不能把绑定查询成功、在线状态变化或本地操作冒充 `SLAVE_CONNECT_EVENT`。源 `oe` 还会调用 `ye` 写 `duallink-devices` 并注册 runtime 监听，不能整段复用为只读 callback。

固件版本、80 字节 IC 分类、固件流程和完整 runtime 元数据仍需各自真实读取。164/241 的连接/限速条件、参数主体及独立模式须继续按其源证据逐页完成。DLL 修改、设备/服务写回及 DLL 持久化保持后置。

native 模块的 ABI、隔离 worker、严格报文验证与调用范围统一维护在 [DLL 只读接入表](dll-readonly-inventory.md)。源码目录只说明能力与身份关系：例如 `183→182` 必须先有真实接收器查询返回 183，并保留同一真实 container。当前 `ye(...,false)` 不删除历史缓存，缓存存在不能替代本次在线；当前 he/Ne/host 原始发布器会写存储、注册 runtime 监听并触发初始化，不能整链执行作只读验证。

## 防回归与验证

`receiver_page_state_tests.rs` 覆盖部分发现保留在线导航、缺席未知与完整缺席离线、精确 edition 确认、重复 owner 歧义、有限重试、过期结果，以及实际父卡挂载。`receiver_pairing_state_tests.rs` 覆盖结果代际、操作种类、未发送意图、失败恢复和成功关闭。`source_workspace/tests.rs` 覆盖自动读取身份守卫。

允许的验证为静态工具、资源/XML/JSON 校验、格式化及 `cargo check --locked --all-targets`。维护命令为 `audit-receiver-pairing-current.cjs --check`、`audit-receiver-current.cjs --check`、`audit-receiver-connect-events.cjs --check`、`audit-receiver-publishing-review.cjs --check`。本项目约束禁止运行应用、测试、下载 JavaScript、安装器或 DLL；测试源码仅编译。字体、焦点、命中、动画与 DPI 的运行视觉验收仍未完成，不能据静态通过声称实际读取运行成功或整个接收器完成。
