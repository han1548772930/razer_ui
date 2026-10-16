# 完整分析与实现路线

从原始输入生成全量分析，完整实现所有本地功能，包括设备变更、写回和持久化。[本地优先范围](local-first-scope.md) 排除的联网功能须从 UI 和 dispatch 清除。

## 2026-10-16 全产品并行实现

全产品、本地全页面的原始任务尚未完成，当前主任务保持进行中。三个子任务分别负责当前来源与资源、全部产品 UI 对照、本地平台／设备调用链；主任务负责集成、音频及统一验证。以下记录区分已落地具体链与仍需继续实现的部分，不能作为整页完成结论；缺项仍集中维护在 [gaps.json](../../.analysis/results/gaps.json)。

| 负责人／范围 | 已落地位置与行为 | 继续位置 |
| --- | --- | --- |
| `mouse_source_integration`：鼠标滚轮 153／170／203／204／214／226／45066 | [页面任务](../../crates/razer-pages/src/features/mouse/scrolling/mouse_scroll_task.rs)、[串行保存](../../crates/razer-shell/src/shell/mouse/mouse_scroll.rs)、[直连传输](../../crates/razer-discovery/src/direct/mouse_scroll.rs)：用户编辑、300ms 模式 debounce、53 位 profile 版本保存、typed IPC、接收器分支、226 mask→mode、取消与清理；保存仅合并本次操作的字段，保留原版乐观 UI | startup／profile restore 的滚轮初始化和事件、Smart Reel 实际 OBM GUID、45066 实际首匹配 slot；详见 [MW 证据](../../.analysis/results/frontend/mouse-mw-audit.json) |
| `product_ui_parity`：八款游戏手柄及共用控件 | [配置恢复](../../crates/razer-pages/src/features/gamepad/gamepad_profile.rs) 恢复扳机、摇杆与 OBM slot 的已保存字段，校验 side、值域与产品分支，保留扩展字段；[Stepper](../../crates/razer-widgets/src/stepper.rs) 按当前完整组件仅在点击输入后注册窗口滚轮，失焦清理 | 通用调色板与 Studio 三类取屏色 owner 正在接线；随后继续手柄 lighting 的完整 mounted 子树。现有部分控件、profile 恢复与模拟测试不代表八款整页完成；详见 [main 证据](../../.analysis/results/frontend/gamepad-main-audit.json) 与 [MW 证据](../../.analysis/results/frontend/gamepad-mw-audit.json) |
| `accessories_camera_system`：Kiyo V2 PID3595 MIC | [MIC 控件](../../crates/razer-pages/src/features/workspace/source_controls/camera_mic.rs)：`CAMERA_MIC_SETTINGS`→`setAudioFunction(1/0)`→10ms→flash save→10ms→reboot；保留 dirty／pending／reboot 状态及取消／清理；刷新当前 native 静态捕获并保留反编译失败 | 新增编译测试及统一测试仍待复跑，继续其余摄像头／附件／系统页面。当前 [摄像头报告](../../.analysis/results/frontend/accessories-camera-local-pages-audit.json) 对应 gaps 仍有未解语义／失败及整页待验收项 |
| 主任务：音频／视频 | [页面播放器](../../crates/razer-pages/src/features/audio/audio_media.rs) 已挂载 1352／1392／1442／3942，连接按需媒体、原八通道 PCM、实际已消费音频时钟、后台帧解码、拖动 seek 及旧代次隔离、播放／暂停／全屏；后三款保留循环和跨页浮动播放／拖动／关闭，1352 保留专用尺寸与音频模式选择器 | 继续原版全部快捷键、点击／双击与焦点语义，真实音量／保音高变速、forceOpen 分支及 SURROUND 完整组件／后端；全屏事件顺序仍须逐完整库函数核对。没有真实窗口／播放验收；见 [音频 demo 证据](../../.analysis/results/frontend/audio-demo-audit.json) |
| `local_backend_parity`：手柄／混音器及平台链 | 八款手柄 Feature collection 与三款 V3 校准接通 source-host 语义；Mixer PID1342 的 Feature64→18 与 Output66→Input16 接线、20 次重试及取消／身份门禁；HID GetInput 返回长度按当前原生完整函数修正 | 正在连接取屏色的后台窗口、截图、流式回调、Done／取消和释放；继续所有产品的原生协议／服务链，模拟传输通过不等于真实设备写回 |
| `current_source_audit`：来源／依赖／资源 | 官方当前稳定通道及本机活动主程序 4.0.827 已复核，138 个宿主文件与官方提取字节一致；664 个 UI／MW 入口本批复核稳定，保留原件未发现字节损坏；补充精确原版 SVG、媒体分析器与资源发布验证 | 继续实际声明缺失文件、拆包失败、间接调用及完整依赖闭包；原生全语料由统一 IDA 队列继续补捕获；source map 缺失、未声明 manifest probe 与实际运行依赖缺失分别记录 |

私有 Dataset `han1548772930/razer-ui-assets` 已完成网页上传，[资源锁](../../tools/asset-release.json) 固定提交 `bf645f811bd14e5ab654eb64f835a79d6a69b2df`。332 个产品分组、7,768 个载荷文件、1,513,885,677 字节已逐项核对：5,275 个 LFS 对象核对 SHA-256 和长度；2,493 个普通 Git 对象以维护快照的 SHA-256、长度及 Git blob OID 交叉验证。主仓库仅保留 Rust 元数据表及 9 个原生托盘／菜单图标。不要重新启动批量下载、上传或推送 GitHub。

资源运行时通过统一 manifest、安装目录 `assets/cache` 和 4 个异步 worker 按实际请求获取单个文件；相同文件去重，先验证缓存、长度与 SHA-256，缓存命中不联网；下载后原子发布。页面订阅加载完成事件，保留失败及重试，生命周期结束取消任务；不预取其他产品，不恢复源码 `assets/`。共享字体按实际依赖请求，本机凭据不写入源码、清单、安装包或日志。安装生命周期统一在 [安装配置](../../packaging/windows/RazerUI.iss) 和 [维护工具](../../tools/windows_installer.py)；当前尚无 release EXE／ISCC，未生成或执行安装包。其余账户／云同步／联网业务仍排除，遵循 [集中配置、类型化数据与安装资源规则](../../AGENTS.md)。

应用启动、编译和安装均不依赖 `synapse.pack`；独立 pack 工具仅用于维护验证。无人调用的 `assets/native/razer-hid-0.0.31.node` 已删除，字节相同的静态分析原件保留在 `.analysis/inputs/`。固定设备能力、配对规则和默认值随 Rust 类型化表交付，不依赖远程资源。

固定设备表由 `prepare-typed-data` 及专用协议表生成器维护，使用所属能力模块的普通构造函数；界面静态目录由 `prepare-resource-database` 生成独立 SQLite，使用普通有类型读取函数。生成时校验真实 Rust 字段、值域、可选字段和产品分支，维护投影保留于 `.analysis/inputs/typed-tables/` 并标明其与原始证据的区别。旧 `src/data/`、分区、读取宏、重复 JSON 和空注册模块已清理。`razer-pages/testdata` 的完整图片和生产 JSON 已删除，仅保留验证具体行为所需的三个小型 Rust 夹具。所有转换仍须通过调用接线、等价性测试与编译；文件／服务报文和导入导出的动态 JSON 边界单独保留。

验证须区分范围：鼠标子任务报告 123 项纯 Rust／模拟测试通过；gamepad 的协议、i18n、保存回归和摄像头 MIC 的局部测试通过，完整合并后测试未完成。最新统一入口收据 [validation.json](../../.analysis/validation.json) 为 `2026-10-16T06:57:15.697066+00:00`，仅运行 `mouse-scroll-persistence` 四项，命令通过但 `inputs_unchanged=false`、整体 `passed=false`，不代表当前全工作区通过。逐命令日志在 `.analysis/verification/`。资源包迁移后的定向 `cargo check` 与包校验已通过，仍不能替代全工作区检查。应用、安装程序、厂商代码、真实窗口及设备验收均未运行。

## 全产品实现路线

2026-10-15 当前范围是全部源码产品与全部本地页面，不限本机连接设备或单一 PID。统一获取 locator 仍保留 332 个 PID，真实产品注册为 331 个：162 端点的 PWA manifest 曾被误当作产品身份，而完整 DeviceInfo 实际声明 163；获取证据保留，错误生产注册已隔离。locator 仅用于获取，不是行为证据。当前正在对所有产品族 UI／MW 原件进行 repair 和完整静态提取。必须按当前 index、manifest 和入口 lazy loader 确认挂载关系；获取目录中附存的旧 main 即使哈希正确，也不能当作当前挂载页面。

2026-10-14 获取的官方生产通道 4.0.827 宿主收据包含 36 个文件；2026-10-15 重新计算其实际长度与 SHA-256，全部匹配。安装主 EXE、ASAR、mapping_engine.dll、simple_service.dll 与 HID.node 均与保留的宿主解包原件一致；主 EXE PE 版本为 `4.0.827.114`。本轮又完成 637 个本机安装文件、4,681 个公共静态缓存容器及响应资源的获取，解析失败为 0；5 个前端缓存组中 PID179 MW 保留 2 个版本冲突。宿主收据仍标记依赖闭包不完整，缓存亦不能证明当天部署；旧 ASAR／JS 提取总量和页面版本须逐项核对当前原件与输出。原始端点、版本、字节身份和失败见 [产物索引](../../.analysis/results/README.md) 及各 acquisition 收据。

同日 01:36:25 UTC 完成 [官方部署元数据复核](../../.analysis/results/host-current/metadata-recheck.json)：6 个官方 JSON 端点双取，全部与 2026-10-14 基线字节一致，Synapse／Chroma 声明版本仍为 4.0.827。仅部署元数据得到复核，未下载安装器，也不据此宣称宿主／设备源码与依赖闭包完整。

启动状态已按当前 MW 的完整 `runtimeData` 与 `setupOnDeviceReady` producer 接线：统一 `audit-runtime-setup --family all` 查询 332 个获取入口，330 个真实 runtime PID，105→104、162→163 保留源码别名；117／2594 初始为 `unknown`，其余已查询分支为 `waiting`。页面模块实际构造后发布源码 `READY`，其后仍独立处理 profile 初始化、设备 GET／SET 和失败，不能把 READY 当作设备写回成功。结果保存在各族现有 `*-mw-audit.json` 的 `runtime_setup`；本次语义查询失败为 0，源依赖图仍保留 Sentry ESM／CommonJS 等未解边，不宣称完整闭包。

启动修复同步保留 discovery 实际读值，Dashboard 在进入原码允许接收电量的阶段时接受已有读值，避免设备已读取而卡片仍显示旧状态；状态文案不再由 Rust 的“本地草稿”等诊断覆盖。设备 workspace owner 使用产品与容器身份，读取序列号不再使页签／待完成请求失配；旧页签顺序仍可恢复。164／179／241 接收器自身固件读取按完整源 `getExtendedFirmwareVersion` 接通 `[4,0,135]`，使用各自已核实的接口与传输参数，未添加鼠标或电池能力。179 的其他初始化字段仍须逐链继续完成。纯 Rust／模拟检查通过维护入口 `validate-rust --suite startup-state` 统一执行；真实应用和设备验收未运行。

当前并行实现记录（具体链落地不等于整页完成）：

| 范围 | 已落地的具体链 | 仍需继续实现／核对 |
| --- | --- | --- |
| 鼠标 PID70 Performance | DPI 编辑与档位选择 → 串行本地保存 → `[7,4,5]` SET ACK；独立 XY 开关按原源码仅保存；连续编辑合并、作用域取消、过期响应隔离及退出清理 | 完整 profile 切换、硬件事件同步、映射／宏的其他链及其他型号分支 |
| 鼠标滚轮 153／170／203／204／214／226／45066 | 源控件编辑 → 每次接受的变更立即进入串行 profile／53 位版本保存 → 独立任务合并 → 有线、自有／共享接收器 typed IPC → Feature SET；模式保留 300ms debounce，226 保留 mask 后 mode；额外 ACK／GET 诊断不覆盖源乐观 UI，失败保留部分结果并继续独立任务，作用域取消与退出清理已接 | startup／profile restore 的滚轮事件、Smart Reel 实际 OBM GUID 关联及 45066 的实际首匹配 slot 仍须继续接入；当前仅用户编辑链落地，整页和真实设备／窗口未验收 |
| 键盘 PID515 Lighting | 从当前 MW 核验 interface 2／BacklightLED 5，补齐 legacy 亮度 GET／SET，复用 UI → discovery → service 链；修正亮度请求超时 | GamingMode 全部交互与产品分支；其余设置页的完整组件树和动作 |
| 摄像头 PID3594 | 曝光重置按当前源码依次提交自动曝光、测光、补偿，独立任务失败继续且不回滚草稿；预览等待 OnFlush 后 Shutdown，并在停止／失败时清帧 | HDR 握手、快捷键、分辨率分支和其余整页交互 |
| 本地壳与 Chroma Settings | 移除账户／登录／在线通知和更新入口；保留本地托盘；动态照明设置按钮连接本机 Windows 设置 | 继续核对其余 mounted 本地页面和平台分支 |
| 设置“所有产品” | 两个设置入口共用本地产品列表，覆盖注册表 331 个产品；支持名称／PID 搜索、版本与该型号实际布局、本地图片；通过正式 ProductWorkspace 路由打开，不创建产品演示弹窗；精确匹配的真实连接优先，否则隔离为离线草稿 | 用户要求的本地浏览入口，不冒称厂商原生设置页签；正式页面仍须逐项完成完整组件／动作清单，路由可达不等于页面完成 |
| IPC 与批量工具 | IPC 帧错误、无效响应和 EOF 及时结束等待；六产品族并行采集与共享缓存；报告重新核对原件身份，不沿用缺失收据的成功状态 | 所有产品资源闭包、间接调用及未实现链持续按 gaps 驱动 |
| 公共页面滚动与列布局 | 固定页签、BodyWrapper 视口持有滚动状态，释放 flex 最小尺寸；正文保持自然高度；通用列保持 row＋wrap、600px 卡片居中、窄窗列外边距，移除额外 gap | 共享组件／CSS 查询的旧片段总量与零失败结论尚未本轮复核；真实窗口、缩放和滚轮验收未执行；产品专用布局继续逐页核对 |
| Dashboard 图片 | 按目录版本、文件名版本、布局、腕托和密度选择本地图片；sidepad／mat 观察经设备模型、产品正文与 Dashboard 接线 | 旧文档的请求、图片与失败总数需从实际原件／收据重算；参数静态定位失败和官网缺失资源继续获取／逆向；publisher 的纯／模拟验证与真实设备验收分别记录 |
| 鼠标／键盘页面图片和几何 | 鼠标 75 款 91 个 Customize surface 的完整 producer、固定几何与 footer 已查询；键盘 72 入口的版本／布局／密度 atlas、SVG producer 已接，60 款 Customize 消除重复间距，756 校准保留 680px 专用列；四个 keypad AVIF 转为当前 codec 可读 PNG | 本批统一编译／纯 Rust 验证尚未完成；全页组件、状态、交互与写回继续推进；获取与图片准备不代表完整页面验收 |
| 接收器布局和图片 | 164／241／207 正文 inset、卡片间距、滚动分支与宿主 42px 起点按完整源修正；207 主图和 pairing helper 使用精确本地 3x 图片，目录／文件版本、布局、腕托、密度独立选择 | 资源闭包失败保留；独立配对、错误、取消、清理和设备写回继续核对；真实窗口与设备未验收 |
| 鼠标电源与 BLE 分支 | 42 款电源卡片、alwaysRead 初始化与本地草稿所有权已接；199／208 使用源码六个秒数按钮；204／45066 BLE 隐藏电源页，170／214／235 隐藏校准页，历史导航跳过隐藏页 | Protocol30／真实 WinRT GATT 分支在本批继续收尾；源码完整确认、编译与模拟验证分别记录 |
| Seiren 1422／1446 EQ | 已按当前 mounted 布局接专用页面生命周期、动态频段恢复／快照、typed 请求与串行本地持久化钩子；当前 EQ／effect 协议和完整源证据继续合并 | 专用 graph、增删／提交／确认、mic／monitor、频谱、Tutorial 与录音 footer 的真实本地链继续实现，不以草稿或空图代替 |
| 接收器／键盘事件 | 持续 interrupt reader 与 Feature 查询句柄分离；Unpair ACK 后的额外诊断不覆盖源 UI 成功状态；独立重连与 owner 清理正在接线 | 重连批次及本轮资源合并后统一编译／纯 Rust 模拟验证；真实设备未验收 |

共用 HID 读写现按当前 `UsbRzDeviceAction.js` 与 `HID.node` 对照：GET 使用 `reportId ? reportId : claimInterface`，SET 使用 `reportId ?? claimInterface`；GET 去掉返回首字节，不额外要求回显请求 ID。产品 factory 的实际接口、报告 ID、事务前缀、等待和重试分别保留，接收器 master 绑定后不再被逻辑产品默认值覆盖。Windows source-host 路径按 collection 最大长度传输，并将 hidapi 非零报告前缀的返回长度转换为原 HID.node 语义。具体完整函数、物理分支及 Rust 对应见 [共用 HID 查询](../../.analysis/results/native/chain-audit.json)，不能据此标记全部产品全部协议闭合。

PID182 休眠的接收器链已连接页面、discovery、typed IPC 和 service，覆盖自有 183 与源码支持的共享 master。当前 `85191.Rt` 的顺序为 SET 秒数、取消检查、一次独立 GET 并比较；不预读、不因旧值相等跳过，不要求 ACK 回显设置值。前后真实 peer 状态和响应身份分别校验，失败不回滚原版乐观 UI；该产品源码只有 wired／dongle，不添加 BLE 分支。完整 caller、任务、继承方法和 native 对应继续保存在 [鼠标 MW 查询](../../.analysis/results/frontend/mouse-mw-audit.json)。

电源页按当前 reducer 和组件补正：42 个产品初始省电值为 5，34 个产品低电量阈值初值为 30，不能从 profile 模板猜测 UI 初值；后续真实 getter／本地设置按各自源码覆盖。38 个 alwaysRead 产品跳过省电项磁盘保存，不再让磁盘错误阻断设备写入，并迁移移除无源码依据的 `_mousePowerDraft`。设备级电源任务跨 profile 切换保留，物理连接销毁时取消；199／208／214／229 等真实本地保存分支保留。标准滑块使用原版数字气泡、轨道、端点和禁用样式，拖动预览、松手提交；27 处帮助按钮恢复独立右上定位。完整 shared-page-layout 查询随本轮当前部署获取继续复核，静态／模拟测试与真实窗口验收分别记录。

本轮另按当前完整组件／CSS 修正鼠标 Customize 按钮 margin 所有权、禁用 hover 和 tooltip 子项间距；键盘帮助图标回到卡片右上位置，Hypershift 提示按型号保留宽度与断点，756 的 FN＋Backspace 卡片恢复源尺寸。Seiren 1422／1446 下拉框恢复源尺寸、边框、选项与展开动画，提交值和键盘游标分离，Enter／Escape／外部关闭与失焦遵循独立的焦点和关闭链；无采集时清空频谱而不清除独立 meter 观察。配对入口保留完整源 metadata，不用测试专用裁剪 payload 代替真实 caller。

针对本地反馈，公共 CheckItem 恢复当前 `.check-text` 的相对定位和 `min-width:max-content`，以实际字体度量保留文字宽度，避免中文被 20px 方框挤成竖排；游戏手柄数值提示和 Audio Mixer 数值包装按各自源规则修正，未全局强制单行。真实发现设备与离线预览共用当前厂商名称字典，保留用户改名，规范语言代码别名。保留的选择项现在保存源文案键／产品自己的词典，在显示时取当前语言，不把语言切换当作设备变更，也不翻译用户预设名。Sound 六型号和 Mixer 的纯测试覆盖同一选择项切换语言、选择身份／禁用状态不变及与内置文案键同名的用户预设。

图片检查分开核对资源解码与请求选择：设置中的版本／布局候选从当前源支持域取得，不能通过删掉缺图候选掩盖遗漏；本地产品浏览可使用同 PID 的真实 ProductImage 备用，遵守其版本／布局重试规则，不跨型号借图。真实键盘版本与布局读取保留为独立观察，不从初始化值或预览默认值伪造设备响应；直连、接收器和 BLE 的实际分支分别核对。

编译警告与 UI 偏差分别核对：修复跨模块可见性、漏更新的调用签名和测试 helper 重复编译，移除没有生产 caller 的复制代码、演示兼容弹窗及范围外社交／远程更新占位；未添加 blanket allow 或关闭 warnings。零编译警告也不能证明原页面视觉、全部行为或设备写回一致。

生产 `razer-model::demo` 与壳层合成设备注入已移除。离线 `product_preview` 只从真实注册表构造本地草稿，设置状态未知，不编造固件、电池、连接观察或已初始化功能；测试数据归 owning crate 的 `testdata/`。历史持久化只清除同时匹配旧 PID、serial 和 container 的精确合成记录，不误删真实或未知设备。

全量媒体与巨量字面量不再进入编译：固定窗口控制 SVG 和 9 个原生 RGBA 图标内嵌；界面目录、表情和资源索引在 EXE 旁独立 SQLite，所有实际打开的界面按文件加载媒体并缓存。`tools/resource-mappings.json` 替代旧 Rust 资源片段，保留 first-match、重复 list 顺序、图片选择条件、sprite 与 GPUI fallback。缺失或损坏保留真实错误；未打开的页面或产品不预取。转换等价、完整原件解析、纯／模拟测试和真实 UI 验收分别记录，不能把数据库生成当作全页面完成。

本轮改动仍在合并，旧的 1,115 项测试结论不适用于当前代码；当前 [validation.json](../../.analysis/validation.json) 尚未通过新一轮检查。待本批 Rust／资源冻结后，通过维护入口统一执行 `cargo check --locked --offline --workspace --all-targets` 与适用的纯 Rust／模拟 transport 测试，记录实际测试数量、失败和验证期间输入身份。窗口测试不在允许执行范围内。

输入身份审计独立于代码测试。本轮恢复索引并完成本机静态获取后，2026-10-15 01:33:09 UTC 的 [input-audit.json](../../.analysis/results/input-audit.json) 记录 80,674 项：77,478 verified、3,037 missing、159 mismatch，error／index error 均为 0；其中 17 个 fixtures 全部 verified，整体仍未通过。身份不符集中于 157 个 acquisition 和 2 个 probe 收据，不能改写预期摘要掩盖；缺失项也不能因当前 acquisition 未引用而删除。后续元数据获取和 capture 会继续改变索引与原件，须重新审计并结合产品族／层矩阵核对；缺失原件、source map、过期 capture、未解引用和 ABI／实现缺项均保留。

本轮最终图片准备冻结后，通过 `verify-resources` 重建 [resource-audit.json](../../.analysis/results/resource-audit.json)：7,559 项源图片全部通过，PNG 5,569、SVG 1,845、WebP 141、GIF 4，验证期间图片输入身份未变。扫描在资源表移出 Rust 后仍追踪全部原图，不能只检查残余内嵌图。此次 ProductImage 域保留 314 个产品、2,083 张图、209 份完整重试 producer 及 1 个 Hue 集成 logo，资源／导出失败为 0；Dashboard 保留 4,435 张图和 3,439 个真实失败候选请求。9 个官方缺图变体有完整 caller、资源上下文及双密度 404 证据（103／147 的 edition 128—131，196 的 edition 128，均 layout 0），不借用其他产品图片，也不通过删掉可选版本掩盖。此检查不证明整页完成，实际 codec、版本／布局选择与真实窗口分别验收。

最终静态 pack 准备和逐项验证保留 11,773 个唯一资源键、7,066 个内容块，原 alias 字节总量 2,142,884,686 经字节去重为 1,213,897,101 字节内容，加索引后的正式包为 1,214,965,051 字节。7 个重复 alias 的实际内容一致，异字节冲突为 0；输入在生成前后均重新核验。完整 `SynapseAssets::list/load` 回环和应用构建检查需使用当前包继续执行，容器层通过不替代产品页面验收。

当前源码与分析收据保留在 `.analysis/results/`，完整 Rust 检查和纯 Rust／模拟 transport 测试收据见 [validation.json](../../.analysis/validation.json)。源码图校验失败、缺失 source map 和资源仍明确保留；未运行应用、厂商代码或真实设备操作。

| 工作 | 必须保留的可复核结果 |
| --- | --- |
| 获取并核对当前原始输入 | 来源、获取时间、版本关系、原始包／文件字节、动态 SHA、缺失输入 |
| 统一提取完整前端 | 全部 JS/CSS/HTML、source map、资源引用、模块依赖、实际挂载组件、所有产品／版本／附件分支及完整函数体 |
| 统一提取宿主与本地服务 | 本地 IPC、参数与返回、事件与状态、存储、系统集成、安装和生命周期调用链 |
| 统一提取原生库及插件 | IDA／Hex-Rays 全函数与边界、RVA、完整伪代码／汇编、xref、类型与协议证据及失败项 |
| 补齐跨层语义 | UI 调用方→参数→transport／服务→响应→状态→写回／持久化→取消／清理；间接调用与缺少 producer 明确标未解 |
| 完整 Rust 与 UI 实现 | 协议逻辑、平台适配、服务、UI 的具体代码对应和每项剩余缺口 |
| 当前验证 | 静态检查、纯 Rust／模拟 transport 测试、输入与代码身份、失败结果和禁止执行的运行验收状态 |

通过 `tools/workspace.py` 维护统一入口，分析缺项集中写入 `.analysis/results/gaps.json`。工作流记录工具版本、输入边界、每项提取结果和失败原因；提取后复用结果查询，对明确未解项补查。前端恢复使用 Wakaru 1.14.0；原生分析使用 `E:/IDA` 的 IDA Pro 9.1／Hex-Rays 9.1.0.250226。本轮已重新取得并捕获 GenericCamera、HID.node、simple_service.dll、mapping_engine.dll 与 BLE binding.node；完整函数数量、反编译失败和原始汇编以对应实际输入 SHA 的当前 `capture.json` 为准，不沿用旧工具或旧输入的统计。能力、输入摘要与限制见 [工具说明](tooling.md) 和 [产物索引](../../.analysis/results/README.md)。捕获成功不代表协议或适配完成。

遵循根目录 AGENTS.md 的持续实现规则：未实现项必须继续完成，不能只登记缺口或留下 TODO、空函数、占位接口、恒定未连接返回或仅有 mock。缺证据时继续静态逆向并补齐真实调用链；具体平台适配和 UI／后端接线均属于交付范围。真实窗口／设备验收仍受执行限制约束，不能伪称通过。

每个本地 UI 页面逐项核对：

- 实际 mounted root、完整组件树和所有产品／版本／附件条件。
- 布局、尺寸、字体、颜色、图片／图标、文案、标签页、面板、抽屉、弹窗和提示。
- 普通、选中、禁用、加载、空值、错误状态，以及键盘、焦点和滚动行为。
- 每个本地动作的完整提交／响应、确认、乐观更新、失败、刷新、取消、保存与资源清理链。
- 具体当前源码位置、Rust/UI/backend 对应和剩余缺项；本地草稿与设备／服务写回分别记录。

GPUI 修改前遵循 [AGENTS.md](../../AGENTS.md) 中指定的 GPUI Kit 与设计 skills，并对照项目锁定 API。通用设计指南不能授权精简或重新设计 Razer 原页面，也不能替换原始确认和失败语义。

页面只有在全部源码→Rust→UI／服务清单完成后，才能标为完整实现。路由、descriptor、哈希、snippet、wrapper 或部分控件均不足以验收。源码获取、逆向语义、实现、接线、静态／模拟验证和实际运行验收分别记录；缺项不能标为完成或永久延期。

允许静态解析、资源准备、格式化、`cargo check` 及纯 Rust／模拟 transport `cargo test`。应用、厂商代码、DLL、helper、installer、真实服务和真实设备操作继续禁止；真实窗口和设备验收保持未验证。当前新检查结果见 [验证记录](../../.analysis/validation.json)。
