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

179 Widget 帮助采用其实际 portal：悬停立即创建/显示，离开立即隐藏，14px/18px 字体、内容最大宽 300px、层级 10001 及源窗口边界调整；帮助圆点背景保留 300ms 过渡。配对工具保持源 850px 宽、内容最小 850×685、顶部/宿主页签偏移、纵向滚动和标题上圆角。外部点击或 Escape 不主动关闭；本地关闭后恢复焦点属于框架适配，此处不将它泛化为241原modal的行为。加载旋转图保留源圆弧与 square cap。

## 164/241 配对与帮助边界

164 已配对名称打开配对工具；241 名称走设备导航，其源允许缺省/0 edition 匹配实际版本，非零 edition 精确匹配，PID 0 不生成链接。241 页面展示顺序为鼠标在前、键盘在后；它的可伸缩说明列、名称 17px 行高、按钮非对称 padding 与 164 的 369px 固定说明列不能混用。

正式 `DockPairingEvent`/observation 链保留本地 Scan/Pair/Unpair 意图、取消、确认、真实结果与错误。Bindings 更新父卡，读取错误和成功空数组分开；只有确实存在的 PID/edition/layout 才能选精确设备图片。查询类别/名称可按当前目录确定，不能补造 edition/layout/serial。单候选只产生 Bind 意图，不能据此显示成功。

241 轮询率说明仍需完整源条件 `V = !W && (I ? Y : K)`：I 是源 hs 的绑定/目录/runtime连接谓词，Y/K 检查本地profile是否存在polling属性，W检查鼠标和键盘都已绑定并受警告旗标门控。它们不比较实际Hz上限，当前 `!both_devices_capped` 不能代替它；双设备分行还受 `J = t.length > 1 && (W || I)` 约束。具体原文与来源见下文241语义审查。164/241 的临时名称/连接提示、结果计时、独立模式入口和实窗输入仍需分别完成。

164/241 Lighting 实际挂普通快捷效果根，不能以 bundle 中同时存在的端口组件作为主体证据。亮度/熄灯范围和禁用依赖已有当前收据；241的真实八效果、同步及Quick/Advanced原码主体现已在下文细读，164各分支及Rust完整对照/接入仍未完成。164/179/241普通HELP Reset允许本地确认/取消与撤销待发请求，但不改变设备状态或本地配置；其他OBM/OLED/固件Reset不能套用这条契约。

164/241 的产品横幅、单/双设备信息与正式 DockDialog 使用各自 [配对源证据](dock-pairing-current-evidence.json)和资源。164 指定 zia_pairing 单设备图；单设备弹层为#111/#515151，双设备#222/#5d5d5d、共享CSS左右20px卡片内边距及外侧2px选中描边；241实际挂载再用inline将左右内边距覆为10px，见下文父子树级联，不能把共享规则作为241最终值。进度原 SVG 作 1 秒旋转，动作透明度保留源 300ms；这些静态声明不代表完整 viewport/滚动或实窗焦点验收。

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

鼠标导航到 Performance，键盘导航到 Customize。179 父卡/工具的后续 Bindings 查询已将当前目录映射后的真实从设备增量发布到产品工作区；保留接收器 container 和物理 PID，只替换该接收器的 peer 观察，不清空其他接口。此接线限于 179，不能扩称 164/241 或全部鼠标发现均完成。`34340/he/Ne → host` 的发布行为是静态依据，不执行其 localStorage 写入流程。

Shell 在发起和返回时核对真实 owner、非 PREVIEW/DEMO、容器、物理 PID、工作区实体与身份；还核对请求代际和全局 discovery revision。关闭/取消、owner 失效或新发现使旧结果无效。先把 Bindings/读取失败交给 feature，只有 feature 接受才发布全局设备，避免连接更新触发新查询而使当前结果过期。

成功空列表清除该接收器关联；查询失败使该范围的旧观察过期为未知，partial 身份错误保留已知 peer 并将缺席保持未知。真实导航只接受唯一、非样例的工作区；歧义不能借相同 PID/edition 绕过。增量新增工作区不会凭连接查询补造参数，也尚未自动衔接完整 Firmware/Battery 等字段重读；这些仍需各自读取生命周期。

## 查询与生命周期

自动父卡查询必须满足：真实 USB/HID 连接观察、合法非零且带花括号的 UUID、非 PREVIEW/DEMO 身份、工作区激活、当前为 Customize。激活、连接观察和切页都重新判定；本地缓存身份不能单独启用读取。

父卡与工具共用全局唯一请求代际。打开工具会暂停父卡请求/重试，关闭后只有符合读取条件才恢复。离开页面、停用或取消后，迟到响应不能覆盖当前状态。

后端 `project_receiver_query(receiver_pid, container, &raw)` 将同一真实查询纯投影为设备观察和配对描述，启动发现也使用此入口。它核对查询名、VID/物理 PID、接口、report 长度、非零带括号容器及条目数量，并要求实际 path/instance 字段存在；真正的接口唯一性与查询前后身份校验仍由 native worker 执行。原始 Value 不被修改。

投影保留同一接收器 container、physical_product_id、raw peer PID 与全部原始 status，不补 edition/layout/serial/profile/ready/telemetry。sentinel 与独立接收器自身行不生成设备；产品自己的 dongle PID 可按当前目录表示其无线设备。未知/歧义目录项或多个 raw PID 归一到同一产品时，保留错误及其他确定设备，不用插入顺序选状态。配对名称/类别转换失败与设备观察分开，不能把 partial 当成功空全量。源 AST、目录对照和回归源码见 [纯投影证据](receiver-query-projection-current-evidence.json)；`ye` 的存储写入/运行监听仍不执行。

待确认且已确认在线时，最多安排 30 次延迟 1 秒的父卡重读；未知或离线不能维持此重试链。错误显示明确失败和 Retry，成功空数组清除绑定。源 `Te` 忽略空数组，本实现明确区分成功无绑定与读取失败，避免把旧绑定当作当前成功结果。

## 硬件通知与仍未接入的读取

真实连接通知的源链是 `78548/Be → 34340/kO=oe → MW_RESPONSE_UI/SLAVE_CONNECT_EVENT → 9473/Te`。它要求双连接设备、异类别过滤分支、recordId 为 5 或 9、无线事件 ID 为 9 或 53、当前 PID 等于 `dualLinkPrimaryPId`，且 `eventValue.state=3`。`Te` 收到广播后显示加载，20 秒后结束；该广播本身不提供绑定 payload。

V2 查询 `status=1` 是在线结果，与硬件通知的 `eventValue.state=3` 不同。当前未接入真实硬件通知消费者，因此不能把绑定查询成功、在线状态变化或本地操作冒充 `SLAVE_CONNECT_EVENT`。源 `oe` 还会调用 `ye` 写 `duallink-devices` 并注册 runtime 监听，不能整段复用为只读 callback。

固件版本、80 字节 IC 分类、固件流程和完整 runtime 元数据仍需各自真实读取。164/241 的连接/限速条件、参数主体及独立模式须继续按其源证据逐页完成。DLL 修改、设备/服务写回及 DLL 持久化保持后置。

native 模块的 ABI、隔离 worker、严格报文验证与调用范围统一维护在 [DLL 只读接入表](dll-readonly-inventory.md)。源码目录只说明能力与身份关系：例如 `183→182` 必须先有真实接收器查询返回 183，并保留同一真实 container。当前 `ye(...,false)` 不删除历史缓存，缓存存在不能替代本次在线；当前 he/Ne/host 原始发布器会写存储、注册 runtime 监听并触发初始化，不能整链执行作只读验证。

## 防回归与验证

`receiver_page_state_tests.rs` 覆盖部分发现保留在线导航、缺席未知与完整缺席离线、精确 edition 确认、重复 owner 歧义、有限重试、过期结果，以及实际父卡挂载。`receiver_pairing_state_tests.rs` 覆盖结果代际、操作种类、未发送意图、失败恢复和成功关闭。`source_workspace/tests.rs` 覆盖自动读取身份守卫。

允许的验证为静态工具、资源/XML/JSON 校验、格式化及 `cargo check --locked --all-targets`。维护命令为 `audit-receiver-pairing-current.cjs --check`、`audit-receiver-current.cjs --check`、`audit-receiver-connect-events.cjs --check`、`audit-receiver-publishing-review.cjs --check`、`audit-receiver-query-projection.cjs --check`。纯投影与 Shell 范围替换另有独立回归源码，仅编译；本项目约束禁止运行应用、测试、下载 JavaScript、安装器或 DLL。字体、焦点、命中、动画与 DPI 的运行视觉验收仍未完成，不能据静态通过声称实际读取运行成功或整个接收器完成。

## 241 原产品的主根、Pairing 父树与真实传输

本节是当前产品 241 源文件的逐分支语义审查，不能扩展为 164、179 或全部产品已完成。专用 [语义收据](receiver-241-semantics-current-evidence.json) 由 `node tools/audit-receiver-241-semantics-current.cjs --check` 复核，包含 83 项 UI/middleware AST、5 项当前 host 传输锚点、6 个父层条件和 9 个 effect。418 条 CSS 是保留选择器与顺序的上下文规则集合，其中包含共享组件规则，不表示全部都匹配本页或已经计算最终浏览器布局。

范围均为 UTF-16 代码单元、end 不包含尾端；SHA-256 对实际 UTF-8 文件字节计算。当前 Pairing 模块 `3746` 位于 `.ref/devices/241/static/js/914.4c13bdac.chunk.js`，SHA-256 `1320d5e2c7ee42497e590f2dbd979b9d10614573746003b3b87b6695dfc6a9d2`。核心父层 `Ps` 为 `48748..54333`，对话框控制器 `ds` 为 `38810..44775`，展示父层 `ts` 为 `12229..14401`，单双列 `as` 为 `14543..32045`。共享正常根来自当前 `11.219fb515.chunk.js` 的 `9163`，具体范围保存在专用收据中；不得从旧相邻产品的压缩符号推导。

### 正常根、安装门控与父层生命周期

`9163/ln` 根据 Redux `deviceSetup.setupStatus` 选择：`ready → rn → nn`，其他状态挂 `Y`。`Y` 首次等 2 秒再显示安装容器；若状态已不是 ready/unknown，则提前显示并清除定时器。安装内容 `J` 分别消费 waiting/downloading/installing/completed/canceled/error；下载中 Cancel 发往 `installation_device_241` 的 `install-cancel`，错误或取消时 Retry 在 500ms 后发 `USER_COMMAND/retryInstall`，离线时附禁用样式。它不是直接调用配对 DLL 的页面。`ln.onceReady` 初始 false，其现有类定义没有把它置 true；不能据变量名宣称进入 ready 后必定冻结根组件。

`nn` 真正注册 Pairing、Lighting、Help；`Oi=s(3746)` 明确将普通 Pairing 挂到本节审查的树。初始 active_view 来自 sessionStorage 导航历史，否则 Pairing；正常渲染依次为产品头、导航/ProfileBar、`3358/Body` 内的当前页面。ProfileBar 的源排除列表仅 `[Help]`。`Body` 默认附 `scrollable`，换 activeView 时 scrollTo(0,0)；只有 Customize 被传入 no-scroll，不能把 Pairing 主体做成固定裁切。

`3746/bs → 1422/BodyWidgets → [Us, js= memo(Ps)]`，其中 `Us` 受 DeviceInfo.UseSVGForProductImage 决定，241 使用自己的 SVG banner。UI DeviceInfo 明确 `productId=dongleId=241`、ACCESSORY、claimInterface=3、canPairTwoDevices/isPairingDock/showBothDevicesConnectedWarning/showOriginalDongleConnectedWarning=true；这些旗标是本页分支依据。

父 `Ps` 读取 pairedInfo、validDevices、语言与 `ps()` 的 connectedRuntime。挂载及 validDevices 改变时发 `DUALLINK_BIND_INFO` 并刷新 runtime。其 MW_RESPONSE_UI 命名回调在 BIND_INFO 时将非数组 payload 归为 []、更新 pairedInfo 并刷新 runtime；BIND/UNBIND/SLAVE_CONNECT_EVENT 时重新查询。这个父回调没有读取 error 字段，不能将其行为写成严格区分查询失败与成功空数组。父 effect 返回同一回调的 offBCEvent 清理；`ps` 同样为 connectedDevices 的 WindowStorageEvent 返回 offEvent。runtime 的字符串外层数组还会逐条 JSON.parse(value)，解析/读取失败归 []，这是源降级行为，不是证明当前在线。

### I / W / K / Y / J / V 与名称导航

以下是父 `Ps` 实际表达式及被调用函数的含义，不能用一个 `both_devices_capped` 布尔值代替。

| 条件 | 当前源含义 | 显示用途 |
| --- | --- | --- |
| I | 任一 binding 满足 hs | 选 connected 说明文本，并参与 J/Y |
| W | showBothDevicesConnectedWarning 且 we(pairedInfo) | 鼠标、键盘都存在绑定时的双设备警告；we 不检查在线 |
| K | 任一 binding 的 Cs(pid,serial) 为真 | 未判 connected 时的 polling 说明依据 |
| Y | 满足 hs 的 binding 中任一 Cs 为真 | 判 connected 时的 polling 说明依据 |
| J | pairedInfo.length > 1 且 (W 或 I) | 类别标注的鼠标行、键盘行 |
| V | !W 且 (I ? Y : K) | 是否显示 polling 说明；I 决定 Lps/VYK 文案 |

`hs` 依次接受 binding.status===1、validDevices 中同 PID、独立 ready runtime 中同 PID、ready 的 241 runtime.subDevices 中同 PID。它只归一化 PID，不检验 edition。`Cs` 读取本地 `synapse_<pid>`：优先 serial 对应 deviceMetadatas.activeProfileGuid，再 activeProfiles[serial]，再 activeProfile/首 profile；活跃 profile 或任意 profile 含自身属性 pollingRate/pollingRateWireless/pollingRateBle 即为真。它没有比较属性值、实际 Hz 或上限。此前文档“实际 polling 配置”的简写应按本节理解为属性存在性，不能据此补造设备读取。

无绑定显示图标与 Launch Pairing Utility。非空时显示可伸缩说明列和 Unpair：J 为真使用鼠标、键盘类别行；仅一个 binding 使用 paired-with 句子；多个 binding 且 J 为假使用两条 paired-with 句子。W、original-dongle warning 各自独立附加。original warning 合并 validDevices、本地 connectedDeviceInfo/devices 中同 PID 的 dock/原 dongle 记录；runtime 还比较同 PID 的不同 ready container，以及独立设备和 dock subDevice 的 container。这些是源缓存/运行态谓词，不证明一次完整硬件读取。

`Ds` 使用 productName ?? name 的语言对象，然后语言原键、小写键、en、空串；如果 productName 存在但不含语言，不能再回退另一份 name。paired-with 名称先 lowercase 再单词首字母大写，类别行保留原名称大小写。可点击条件 `Es` 则要求非零 PID、validDevices 非空、同 PID，edition 缺省/0 为通配，非零必须相同。**连接显示 hs 与可导航 Es 是两条不同条件**。

导航优先 validDevices container，其次独立 ready runtime，最后 dock 的 subDevice；键盘目标 Customize，鼠标 Performance。已存在 UI 时 activateWindowServiceClient + subtab 广播；存在 middleware URL 时替换 mw→ui 再开 policy=3、tab_visible=1；否则拼源默认 URL 后 400ms 广播，其他默认 200ms。当前源默认 base 已为 `https://apps.razer.com/synapse`，拼接处又写 `/synapse/products/...`；这是可见的双 synapse 字面表达式，不应把经手工修正的 URL 说成原代码。

### Dialog 状态、动作与响应

原 `Ge` 为 LOADING=0、LOADED=1、SCANNING=2、SCANDED=3、PAIRING=4、PAIRED=5、PAIR_FAILED=6、UNPAIRING=7、JUSTUNPAIR=8、UNPAIRED=9、UNPAIR_FAILED=10、JUST_PAIRED=11。列中 connected=0/1/2 是空闲/动画/展示绑定态，不能与 V2 的原始 status=1 混同。`ds` 以 Redux pairedInfo 初始化 bindInfo，首次查询令两槽 LOADING。渲染 gate 只直接判断第一槽 p===LOADING；不能从对称的 status2 名称推导对称加载根。

| 源触发 | 请求 / 状态 | 源返回消费 |
| --- | --- | --- |
| Add/Rescan | 对应槽 SCANNING，SCAN(status=1, category=KEYBOARD/MOUSE) | 仅一个匹配类别候选时自动进 PAIRING 并发 BIND；零/多个进 SCANDED |
| Select/Pair | 当前 selectedIndex 有候选时 PAIRING，BIND(mode=1,device) | 替换同类别 binding，当前槽 JUST_PAIRED，更新 Redux |
| Unpair | 先 UNPAIRING 供确认；Cancel 回 PAIRED | Confirm 后 JUSTUNPAIR，发 UNBIND(productId,category) |
| Cancel Pairing | CANCEL + BIND_INFO + closeDialog | 源 CANCEL middleware 无独立 UI 成功 ack |
| BIND_INFO | 非空按 KEYBOARD/MOUSE 存在决定两槽 PAIRED/LOADED | 空或 error 时两槽 LOADED，并清空绑定/候选 |
| BIND error | 对正在 PAIRING 槽置 PAIR_FAILED | 4 秒后两槽 LOADED，源清空绑定/候选 |
| UNBIND error | 对正在 JUSTUNPAIR 槽置 UNPAIR_FAILED | 4 秒后对应槽 PAIRED |
| UNBIND success | 对正在 JUSTUNPAIR 槽置 UNPAIRED | 以 binding.dongleId !== payload.productId 过滤 Redux；不是按 binding.productId 过滤 |

dongle713 的警告链为单键盘扫描/候选选择触发 isDualLinkWarning，父 Ps 将警告转 continuePairing，ds 从候选中筛 dongleId===713 并以 mode1 发 BIND。continuePairing 分支没有 first candidate 缺席检查，这是源行为；本地不可据此制造成功。成功后的自动关闭由 BIND 响应分支检查回调所捕获的 p/I 是否已 PAIRED/JUST_PAIRED 决定，closeDialog 再延迟 1 秒移除 imperative modal，不等于每次 Bind 成功都立即关闭双列。

`ds` 的 MW 响应 effect 范围 `40826..44206` 依赖 [p,I]，注册匿名 onBCEvent，但没有返回取消订阅；与父 Ps/ps 的清理不同。专用证据只证明此处 AST 没有 cleanup，不断言已运行发生了几次重复订阅。其他需保留的原表达式包括：Ze.SET_LANG 算出 fallback t 却返回原 payload.lang.toLowerCase()；SET_ERROR 只在 payload.status===UNPAIR_FAILED 时将 bindInfo.connected 改成 2，status2 分支不对称；os 引用未定义在 Ge 内的 PAIRED_FAILED 拼写；ss 的 SCANDED 多候选尾部分支被此前 includes(SCANDED) 条件覆盖。应记录差异并决定移植策略，不能把新保护逻辑说成反编译所得。

### 本页动作实际经过 HID，不统一经过产品 DLL

当前 middleware 主链为 `45601/_e 注册 USER_COMMAND → fe → 34340/qe → Ke[type]`。UI 的五类 DUALLINK 动作在 Ke 内有实际 handler，而普通命令继续进入状态机队列。`34340` 位于 `.ref/middleware/241/6120.0efaddd43f12eb2fb450.js`；其字节哈希、每个函数范围及 HTTP manifest 收据均在专用证据中，不能拿共用 bundle 里某个未挂载 DLL wrapper 代替该调用。

`20236/f2(category,true)` 在 canPairMultipleDevices 或 UMA 条件下按类别构造 `48320` 键盘 / `14770` 鼠标类，二者继承 `7755/il=rzDevice25`；设备参数保留 master productId/container/claimInterface，延时采用基础常量的 5 倍。rzDevice25 的四个方法调用 `84816` helper，再执行 sendCommand。实际命令如下；表中的“读”是设备语义读，HID 仍需发送查询报文，不能据 transport send 字样把它归为配置写回。

| 操作 | 原 header [packetSize,class,id] | 数据与解析 |
| --- | --- | --- |
| getMultipleDeviceWirelessConnectionStatusV2 | [80,0,191] | 回复第 0 字节是数量，随后每组三字节 status、PID 两字节；helper 没有在循环前显式验证 data 的完整长度 |
| deviceScan | [1,0,70] | 输入扫描 status；回复 status 与扫描 enum |
| setDevicePairingMode | [3,0,65] | mode、dongle PID 的低/高字节；回复 mode/enum/productId |
| setDeviceUnpair | [2,0,66] | dongle PID 两字节；回复 productId |

Electron 分支的 sendCommand 使用 `hid.sendFeatureReportMutex` 或 `hid.sendFeatureReport`，随后 `_getUSBTransferInResult` 通过 `hid.getFeatureReport` 读回复；protocol="25"、reportLength=91，锁名带 PID/container。`99494/NH → window.top.apiElectron.doRzDeviceAction → 当前 host preload invoke("rzDeviceAction") → main 的 An.handleAction → UsbRzDeviceAction`。host 的发送 case 调 node-rz-hid 的 hidDevice.sendFeatureReport，读取 case 调 getFeatureReport，保留 container、interface、PID、usage、USB instance 的设备匹配条件。没有 Electron 时还存在源 WebUSB controlTransferOut 分支。**因此 241 本节配对/V2 路径的原代码依据是 HID transport；产品 DLL、host native addon 和 HID 不可混为一条链。** 本节没有执行任何 transport。

34340/_e 在方法不存在时返回 []；存在时查询 native/wrapper 的 V2，过滤 65535/self dongle，以 AvailableDevices 等目录映射名称、edition/layout，缺资料时有默认 0 的源元数据降级。SCAN 注册 record 5/9、event55 的 Scan Status Update，在 End 时筛 AvailableDevices/DualDongleCompatibleDevices 与 DevicePairingBuddies；Bind 注册 event54，在 timeout=3 返回错误，success=2 组装 device 并广播。Unbind 有 slave→master 路由和失败后重新查询判断；不能仅依据 JS handler 存在宣称写回已接入。

结果广播之后 `ye` 仍会写 duallink-devices、注册 connectedDevices/runtime 监听；`Ne` 还负责重复查询与设备发布。当前范围只静态记录这些副作用，没有将整函数当只读回调执行。Scan/Bind/Unbind 的 UI 操作继续属于范围内；真正设备/服务配置写回保持后置。

### 样式与间隙：必须按实际父子树叠加

CSS 以当前 `main.1525b0e6.css` 和 `914.530f0373.chunk.css` 为据，规则收据保留 offset、选择器、属性顺序与 important。全局 body/html 为 Roboto,sans-serif、16px、#ccc、#222，min-height720、overflow hidden；主体 Body 为 padding10px 20px 20px、min-width600、flex1、height100%，滚动由带 id 的 scrollable 规则接管。BodyWidgets flex 行折行、居中、margin:auto、max-width1240；Widget 为 #111、radius5、min/max-width600、margin10px auto、padding30px 40px、14px，标题 RazerF5 16px/#44d62c、margin-bottom20。banner 使用独立 height250、min-width1024、max-width1220；不能只按 600px 内容卡推导整页最小宽。

241 无绑定行 gap20、图标40×40；已绑定 pairInfo gap10。contentGroup flex1、min-width0、gap20，pairedDes 的 flex1/width:auto 覆盖共享 369px。名称与类别行 line-height17px；deviceList 只有列方向，没有额外 row gap。Unpair 按钮的 241 覆盖为 min-width90、height:auto、line-height14、margin-top9、padding7px 16px 6px、border #ccc、align-self:flex-start；不能沿用共享 height28/line-height28/padding0 5。polling 说明 #999/12px、margin-top10；两类父页 warning gap10、margin-top20、12px/#999，图标20×20，段落默认 margin 被本页规则归零。

modal 基类为 width850、max-width100%、height100vh、left50%、top100、translateX(-50%)、radius5，Z 对外框和 backdrop 加 inline position:fixed；本页 extraClass 覆盖背景 #222、border none、overflow hidden。header 保留基类 height36、display:flex/center，却覆盖为 RazerF5 16px/19px、#999 和 1px #5d5d5d shadow；close 为36×36、图20×20。本页 body flex1/min-height0、overflow-y:auto、overflow-x:hidden、padding-bottom150；该 150px 属于滚动 body，不能任意加到父页卡间距。

241的Widget帮助 `6299/p` 将.tip渲染在widget内；它没有179那条createPortal链，不能共用portal定位结论。配对imperative modal `3746/Z/X` 的当前原文只提供close图标/回调卸载root，没有Escape、backdrop click、focus trap或恢复焦点的实现；框架若需要适配必须单列，不得归为原码行为。完整控件合同见 [共享控件](shared-ui-controls-current.md)。

介绍区 margin20px auto 10px、width790、max-width100%。双列 master 图与名称在前，随后动画连接条，再 keyboard-left/mouse-right 的卡片；其顺序与父页 mouse-first 不同。duallink-device-content width600；连接条 height30、margin10px auto 13px，双列 master-mousemat 宽520。卡片 250×210、背景#111、radius5，两卡之间右列 margin-left20，选中用外侧 2px #44d62c box-shadow。共享卡 CSS padding0 20px，但本页 is 传 inline paddingLeft/Right="10px"，最终水平内边距须保留 10px，不能只按 CSS 截图改成20。设备图 height120；候选列表有 height104 的独立滚动区。确认框 width210、橙色边框、padding-top18/padding-bottom20、margin-top10，动作钮28px/12px。

Dialog 双设备警告 margin50px auto 0、max-width520、gap10、14px/17px/#ccc，不能沿用父页 20px/12px/#999。Precautions 在 isPairingDock 下 width680（覆盖普通520）、min-height85、margin20px auto auto、padding0 12px；图60、规则 margin-left25、14px/#999。结果条 width520、height20、margin20px auto auto、14px/#44d62c，左右250且右侧 margin-left20。这些数值是静态规则和实际 inline 依据，尚不是 DPI、字体度量、滚动视口和实窗焦点验收。

### 241 Lighting：实际三张卡与八种效果

此前只完成 Pairing，本轮继续读了当前 Lighting/Help 的实际正文。新增 [Lighting/Help证据](receiver-lighting-help-current-evidence.json) 保存75段原文收据、14个效果 switch case、25个Help方法和479条CSS上下文规则；工具 `node tools/audit-receiver-lighting-help-current.cjs --check` 仅静态解析。以下限定241，不按共享代码里存在的全部字段启用别的产品能力。

Lighting 主体是 `9259/Mn [496322,496536)`，仍在当前 `main.899712fe.js`（SHA-256 `65fe2c4069ece8270903895ce32c6c610d5eb8fec94bd944f5791c2376509a02`）。顺序为 BodyWidgets → 左列 Brightness `M→L`、Switch Off Lighting `Ln→Rn` → 右列 Effects `fn→Cn`。没有 Pairing 的产品banner，也没有本页独立 Apply/Save 按钮。下述编辑回调进入源 Redux，不能作为设备已写回或保存成功的证据。

| 控件 | 实际值、状态与回调 |
| --- | --- |
| Brightness | 默认 profile 为 enabled=true/value100；开关保留数值、翻转enabled，经setState回调调用setBrightness。滑块0–100，默认步长1，`changeValue`先写enabled=true；值0时还追加一次enabled=false的setState/回调，不能说只调用一次。slider active 为 nanoLeafEnabled&&brightness.isEnabled。adjustmentModeRunning 时开关直接返回，mousedown另显示提示并preventDefault。 |
| Logo brightness | 共享实现需要 supportLogoBrightness 才显示；241调用 `M` 不传此旗标，不能因为共享代码出现LOGO就增加一行。 |
| Switch Off Lighting | 第一项翻转isDisplayOn，第二项翻转isIdleEnabled；闲置滑块1–15、step1，仅idleEnabled&&brightnessOn时active。handler保留另外两个字段。默认profile为false/false/1；构造器闲置state15不是profile读数。brightnessOff会禁用两个checkbox。hideLightingIdle不成立才显示idle；241descriptor没有此开关。 |
| Effects页签 | isAdvanced初始来自lightingEffectsReducer.isChromaEnabled；点击Quick/Advanced更新本地state，再调用setChromaEnabled。wrapper另受nanoLeafEnabled禁用，ChromaVisualizer运行会禁用一般两页签。BLE硬件效果、Sensa、power-saving、portComponent是共享条件，241当前直接挂 `fn {}`，不能把它们全画出来。 |
| Quick dropdown | 真正数据为241的QUICK_EFFECTS，不是共享switch全部13种效果。选项按数组序号找effectId，再从当前缓存或默认setting复制；Wave没有缓存时按241 CWCCW得默认direction12，Tidal没有缓存时默认direction1。 |
| Quick同步 | mount查询memory并订阅memorystorageevent，unmount解除该事件；普通效果要求memory devices多于一个且存在不同PID，Radiate则用另一个supportEffects谓词。点击只在canSyncQuickEffect成立时dispatch syncChromaEffect，并为图标加turn，1200ms去除。不得凭按钮可见伪造同步完成。 |
| Advanced | `dn`检查Chroma资源完整且installedModules含chroma-app，检查结束前保留invisible占位。具备资源时显示说明、profile提示/下拉、Launch；无profile仍挂空dataset下拉。缺资源显示安装按钮与说明。onInstalled只更新本地resource flag，尚不是设备设置保存。 |

241的QUICK_EFFECTS按原数组顺序为 **Audio Meter(12)、Battery Level(14)、Breathing(2)、Fire(8)、Spectrum(3)、Static(1)、Tidal(19)、Wave(4)**。默认selectedEffectId=3（Spectrum）。实际参数区如下，依据为 `8193:factory`、`3254:AoV/U2A`、`9259:vt`及各控制器的完整原文：

| 效果 | 本241实际参数区与动作 |
| --- | --- |
| Audio Meter | 241没有isAudioMeterWithFlow/ble硬件色picker旗标，进入rt。Color Boost数值框0.25–4、step0.25、maxLength4、允许小数并向上归到0.25倍数；callback提交 `{colorBoost:parseFloat(value)}`，没有spread旧setting。不能增加共享Tt的耳机/麦克风Flow两个按钮。 |
| Battery Level | 说明文字＋help＋电池渐变图＋0%/100%；没有数值编辑callback。图是CSS素材，不是实时电量条。 |
| Breathing | 两个颜色下拉＋Random checkbox。Random时两个颜色禁用，callback spread当前setting后改color1/color2/isRandom。默认color1=#00ff00、color2=no-color、isRandom=false。 |
| Fire | 仍是合法可选effectId8；当前renderEffect switch没有Fire参数case，返回null。其参数区为空，不能补一套根据别的效果推测的颜色/速度控件。 |
| Spectrum | 241无useOptionButtonGroup，case直接返回null，保留dropdown/同步区。共享De的Duration分段按钮没有在本241分支挂载。 |
| Static | 一个颜色下拉，hideNoColor=true；变更提交 `{color1:value}`，此handler没有spread旧setting。 |
| Tidal | 两颜色＋Random、下面方向按钮。默认外/内值1/0；Random禁用两颜色，方向不随Random隐藏。父容器分别inline position:relative、zIndex2与1；不能将颜色popup层级和方向层级统一抹平。默认两色#00ff00/#0000FF。 |
| Wave | 241没有speed旗标，进入He，仅方向；CW/CCW在11/12间翻转，选中状态比较direction===11。不能沿用通用左右方向1/2，也不能挂共享Ye/qe的速度控件。 |

`vt.getDefaultProfileQuickEffectId()` 将默认effectId传给期待数组index的changeEffect，再返回默认ID；它在当前index=-1的render value分支中调用。源码确有ID与index语义混用，不能自行改正常后称“原逻辑”。`checkSyncQuickEffect`的嵌套JSON解析、部分componentDidUpdate查询没有catch；来源缺失/坏JSON不等于空设备集合。`Cn.setChromaEnableLocalStorage`内部shadow了t，后续t始终未赋值；不能据方法名宣称它已更新Chroma存储。

Effects动作继续到 `5107`：setSelectedEffect从当前deviceReducer取isBle，补useHardwareEffect并dispatch；setChromaEnabled/setCustomColors/sync只dispatch，cache/getCached以selectedProfileGuid发动作。它们没有设备确认返回；241 descriptor默认profile也不等于一次DLL读出的配置。brightness、idle和effects的下游观察/回写仍须分开接。

Lighting的左右列各600px，列height:fit-content；BodyWidgets折行居中/max1240；卡片各自margin10px auto、padding30px 40px、14px/#111/radius5。小于等于1279px时列另有左右30pxmargin，不是给所有卡片加60px内边距。Brightness标准slider高64，轨道底距25/高6；Switch Off卡有has-slider，才附左距30/宽490。页签外框36px/radius18/padding5，内部lighting-effect26px/radius13/padding5px 10px，再受no-inner-border和后续modes-tab规则覆盖；active为#44d62c/#111，hover/pressed独立。modes-area上padding20，Advanced invisible保持display:block但visibility:hidden。Quick dropdown右margin20/max-width150，sync说明宽340；这些是原选择器声明，实际尺寸还须按inline、父树和条件核对。

### 241 Help：实际两列、数据来源及重置确认

真正入口是当前 `11.219fb515.chunk.js`（SHA-256 `d39889eed8d9c2443c0a57b075018604c8e4283a70ef98dc554ac712f667dc61`）的 `9163/Oa [264028,265089)` → `Ia [245316,263984)`。正常根只传 `resetObm:this.resetDevice`，connect注入语言、序列号、masterGuide/supportPage、固件版本、isBle等Redux字段。Ia.defaultProps仅给resetTitle；不能把共享hasTutorial/hasTHXPartialAudio/hasCamoStudio/hasSystemInfo当本241启用能力。

本241实际左列先Support，随后Factory Reset；右列先Serial Number，currentFWVersion成立才有Firmware，最后Product Registration。共享系统信息、教程、THX、Camo块各受未传的旗标控制，本入口没有这些块。Support内部按顺序显示有supportPage时的Device Support、`${masterGuide}${lang||"en"}.pdf`、`https://support.razer.com`；语言PDF没有此处fallback存在性检查。241 descriptor提供自己的MasterGuide URL与supportPage，实际显示字段来自Redux，不能把descriptor静态值当请求成功。

链接包装 `Li→Ui` 生成的是 `a href:null`，点击有目标URL时preventDefault并通过 `ki→f.A.openExternalWindow(url)` 交宿主打开，随后调用传入onClick（默认空函数）发visit-support统计；不是普通href/target=_blank导航。Product Registration目标为 `https://www.razer.com/product-registration`。Support行上距10，文字下划线/#ccc，hover #44d62c；external-link用20×20伪元素、左距5，跟随hover着色，不能将伪元素漏掉或用托盘图标替代。

Serial初始拷贝props.serialNumber到state；默认非Camo入口没有componentDidUpdate同步serial的路径。Copy仅在navigator包含clipboard时调用writeText，马上将isSerialCopied置true，2000ms改回false；原码没有await/失败捕获，Copied标签不证明clipboard成功。按钮用class disabled而非此处原生disabled。retry/loading/error流程属于hasCamoStudio条件，241不能无依据挂出NOSERIALNUMBER/Camo激活码/登录块。

Firmware整个卡受currentFWVersion gate；非SystemInfo分支先显示Current Firmware，有newFWVersion时显示橙色#fd8611下划线可点击提示，点击只广播navigateDeviceAndModuleView到Dashboard。Show All/Show Less切本地viewMore；展开后仅非空uiVersion/mwVersion/synapseVersion各加margin-top10。版本来源分别为noscript#version的version.buildVersion、host windowStorage中的MW_VERSION（还按containerId/PID筛选）、localStorage apps的synapse条目（强制第一段为4）；三者不同，不能统一成DLL版本。读取/parse失败记录错误，不填猜测版本。Show All上下padding15/14px，箭头20×20/左距4。

本241 descriptor没有isNonSupportFactoryReset/isSupportResetOLED，普通reset块满足条件。Reset按钮打开同卡内confirmation，inline width300/top:auto，title取源当前语言的productName，message依isOBMDevice选择；不使用OLED分支的top125/left140。Cancel只关popup并解除busy。Confirm先关popup、设busy，再走normal-root resetObm，向middleware广播ON_RESET_DEVICE `{timerTick:undefined,payload:{}}`，并发统计。2000ms定时解除busy不是设备成功ack；按钮busy时原生disabled并显示spinner。本阶段保留编辑/确认UI，实际设备写回继续后置，不能伪报Factory Reset完成。

同样，hasSystemInfo固件30%/70%列、OLED重置payload、教程、Camo联网license等共享分支保留在证据中，没有挂到241。它们须由其他产品的真实caller/descriptor验证，不能直接通用启用。共享确认popup的outside-click/键盘细节见 [共享控件](shared-ui-controls-current.md)。

### 本轮明确未完成的边界

本轮已说明241普通主根、Pairing父卡/对话框、Lighting三张卡及八个真实效果、Help本产品挂载的两列、状态来源/动作、主要布局声明和HID wrapper→当前host边界。仍未完成全部产品所有displayMode、78548硬件通知注册全生命周期、全部颜色picker/安装资源内部状态、目录/图片语言每个组合、lazy CSS最终级联、初始化服务错误恢复和实体设备读数验证；node-rz-hid与产品DLL的C/C++内部不能据JS包装宣称恢复原始工程。当前Rust对照仍须按源条件逐项核查，本次仅补文档/静态证据，没有修改Rust、vendor或接入写回。
