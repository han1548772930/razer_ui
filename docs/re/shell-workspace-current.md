# 宿主与本地工作区契约

本文记录当前已实现的宿主页签、本地入口、共享产品正文、配置栏和草稿边界。入口可打开、根组件有收据和静态检查通过，均不表示整页或产品已完成。实现依据为当前 `.ref/applications/`、`.ref/devices/` 与宿主 `.ref/host-4.0.827/`；版本依据见[宿主来源](current-host-version-audit.md)与[Dashboard 来源](20-current-source-version.md)。

## 页签与工作区归属

当前宿主已有 Tab 发起的 `policy=3` 请求添加或聚焦具名宿主页签；`policy=5/6/7/8` 进入原生窗口分支。同名目标的查找发生在创建分支之前。顶层窗口、外链和登录路径应分别核实，不能仅凭 `windowName` 或 `sameWindow` 推定窗口类型。完整执行链见[宿主策略](host-window-policy-current-audit.md)及[机器收据](host-window-policy-current-evidence.json)。

| 应用 | 页签身份 | 本地所有者 |
| --- | --- | --- |
| Alexa | `alexa` | `AlexaPage` |
| Macro | `macro` | `MacroPage` |
| Armory | `armory` | `ArmoryPage` |
| Profiles | `profiles` | `ProfilesPage` |
| Feedback | `feedback-synapse` | `FeedbackPage` |

模块目录与 App Picker 经 `AppShell::open_module_tab` 进入导航保护和 `HostTabs`；页面实体由 Shell 保留。同名打开复用页签，顺序、关闭和键盘切换由宿主处理。Feedback 保留表单草稿与确认层焦点，其根不附加 Dashboard 工具栏。

多设备配对同样使用具名宿主 Tab，以容器、产品与序列号保留身份和复用行为；见[配对窗口契约](multi-pairing-ui-current.md)。`display_window::open_or_focus` 拒绝 `WindowPolicy::Same`。Chroma 的 `policy=5` 窗口与产品 `displayMode=chromaApp` 内嵌正文属于不同层级。

打开本地应用不会写入外部 installed/native 状态。服务预览是隔离的开发入口，其场景不能注入普通页面的账户或设备事实；见[服务预览](service-preview-current-audit.md)。

## 本地设备入口

`ProductWorkspace::has_local_page` 查询已保留的 renderer。Source 分支要求实际 family、dock、accessory 或 supplement 至少支持一个非 Help 页面，并对 Audio、AccessorySystem、Controls 使用逐页能力判断。仅有注册导航、`Pending` 内容或 Help 页面不满足能力。

Dashboard 和 App Picker 可绕过本地内容的安装遮挡，但须同时满足：已有本地 renderer、原 `can_focus` 成立、`no_alive_sign` 不为 true，且阶段属于 `waiting/downloading/installing/syncing/install_canceled/error`。当前源码明确把安装 error/canceled 映射为安装失败；不能据此合并其他错误状态。

原 `ready`、最低固件、固件更新/重启、断电/待机、主机模式、预设加载、Mixer 初始化失败等字段保持各自语义。覆盖仅影响本地打开所需的安装 spinner、重试、文案和图片淡化，不制造配置名、电池或遥测。App Picker 打开时按原 PID/container 重新确认同一个工作区的能力；安装预览默认不启用此覆盖。

证据为[本地入口](local-device-entrypoints-current-evidence.json)和[Dashboard 状态树](dashboard-ui-current.md)，维护工具为 `audit-local-device-entrypoints.cjs` 与 `audit-dashboard-card-state.cjs`。

## 产品独立正文

`chromaApp` 必须同时具有当前源根分支与实际本地 Lighting body。Audio 的 28 产品由各自根、连接包装、renderer 和普通 Lighting 调用关系生成白名单：[逐产品证据](product-mode-current-components.json)、[实际路由表](../../crates/razer-pages/src/features/audio_chroma_modes.json)。1465 使用 `LIGHTING` 键，其余不能据家族名自动开放。

独立正文读取同一产品工作区的编辑状态，不修改主宿主当前页。1303/1304 根仅取消 main-container 最小宽度，body-wrapper 保留 600px；另 26 根同时取消二者最小宽度。源 body padding 为 `10px 20px 20px`，背景 `#222`；不同产品的 serial 条件也分别保留在证据中。

1303/1304 左列叠放亮度、显示器关闭选项，右列显示效果；右列已有专用 Quick/Advanced 及各效果编辑器，详见[Nommo 当前实现](nommo-effects-native.md)。1313 的 cosplay、1342 的区域灯效专用 body、1387/1396 的 WDL 条件及未单独核实产品的完整控件仍需继续完成，不能由相同根名推定一致。

Armory 产品图、3893 仪表及 3907 专用正文以[Armory 产品契约](armory-products-current.md)为准。产品图资源已准备，专用根已有局部实现；不再把它们列为仅有路由或缺图。真实遥测、性能模式和远程映射仍有明确缺口。顶层 Chroma 快速预设使用的编辑接口也不能外推为所有 Audio body 的设备写回能力。

## 产品配置栏

`isEnableProfileBar` 的字段名不等于隐藏整栏。当前根 props、Redux 消费者及页切换调用分别决定可见、下拉可用与同步图标。

| 产品 | 配置栏 | 下拉 | 同步图标 |
| --- | --- | --- | --- |
| 179 / 769 | 不挂载 | 无 | 无 |
| 164 / 241 | 包括 Help | 本地默认可用 | Help 为 unsupported |
| 778 / 3871 | 保留 | 本地默认可用 | 仅 Lighting 启用 |
| 784 | 包括 Help / Customized | Customized 禁用 | Help / Customized 为 unsupported |
| 3884 | 保留 | 仅 Lighting 可用 | 仅 Lighting 启用 |
| 3886 | 保留 | 初始 enableSwitchProfile=true，根无页切换禁用调用 | 仅 Lighting 启用 |
| 3946 | 保留 | 本地默认可用 | Help 禁用；导航更新还排除 Calibration |

以上来自[配置栏收据](product-profile-bars-current-evidence.json)。691 的独立规则见[691 专项](oled-ui-current.md)。动态 adjustment/Loupedeck 条件尚无完整服务映射，默认可用不能外推到全部运行状态。

当前栏使用 250px rename 外框、左右各 10px、230×27px 下拉、Roboto 14px 与 26px loader 区域；禁用 `.3` 透明度只施加一次。更多菜单按逐产品[菜单证据](source-profile-menu-current-evidence.json)消费，重命名、集合编辑和本地确认路径已存在。导入/导出有真实文件选择和弹层，但最终转换/传输能力仍缺，见[配置传输](source-profile-transfer-current-audit.md)。不能把已挂载菜单描述为全部未实现，也不能由菜单存在声明服务成功。

## 页面活动状态与本地保存

Shell 在导航与启动参数处理后的最终 Location 上同步 ProductWorkspace → SourceWorkspace 活动状态，覆盖直接 `--tab` 入口。鼠标只在宿主活动且当前为非 Help 内容时设为 active；失活取消数字 repeat、grid pending/pressed、滚轮预览与 tooltip，并回填已提交的本地值，不提交预览或发出新的 Changed。

鼠标 DPI 的写入口同时检查可见内容、Performance、OTFS、阶段拖排、实际槽位与 independent Y。无效 Enter 在 blur 前返回；slider/grid 的隐藏或旧代际回调不能提交新草稿。当前轮询与滚轮的显式本地字段、观察优先级及剩余生产者边界见[鼠标契约](mouse-ui-current.md)。不能仅因 restore 补齐了默认字段就把字段认作用户显式覆盖。

Source 活动状态还同步 receiver 读取活动，并在失活时取消 Gamepad 范围预览、离开校准。校准观察要求活动校准页与有效 generation；见[2636 校准](gamepad-2636-calibration-native.md)。上述保护不证明每种家族、控件和独立窗口都已验收。Chroma popup/Armory 共享产品 body，其可见性不能一律取自主宿主 Device Location；跨窗口卸载与迟到回调仍需逐入口核对。

Source capture 把 device_fields 分出到 `source_device_settings`，其余归活动 profile。Shell 保存捕获的 WorkspaceFile；异步成功后只把捕获快照标记 saved，保存期间的新修改仍为 dirty。状态明确为“已保存到本机 · 尚未发送到设备”。本地草稿持久化与 DLL 写回不同，UI 编辑、Apply/Save 和本地保存仍在当前范围内。

各家族的 snapshot/restore 只能证明被核对的方法：Snap Tap 保留显式本地配置并重建瞬态；1382 重建 editor/subscription；3334/3337 无显式 local_selection 时不把观察到的 playbackMixDevice 存入快照。2636 deadzone 回退缓存的跨 profile 语义及其他家族完整生命周期仍需专项核实；不得据几处方法抽查声称全部产品完成。真实查询、状态观察、DLL 修改、设备/服务写回及持久化全部继续接入，当前未闭合的链仍登记为实现缺口。

## 公共字体与内容

当前真实字体注册包含 RazerF5 Bold/700、Thin/100、Roboto Light/300，并将 Roboto Medium 按 CSS 映射为统一族名/500。字体准备仅调整必要的 name、OS/2 字重/标志与 head.macStyle，其他字体表逐一比对原始 WOFF2 解压结果，不重画字形或改字宽。维护入口为 `font_assets.py`、`prepare-font-assets.py`，收据为[公共内容证据](shell-content-consistency-current-evidence.json)。

Devices & Modules 安装失败和正在移除文字为 14px；离线提示只在源默认/error 分支出现，安装/下载/canceled-offline 不挂载该 hover 提示。提示使用黑底、`#5d5d5d` 边框、Roboto 14/16、padding 8/10、right −160/top 32 与 300ms linear。完整组、行与安装交互以[模块服务契约](module-service-ui-current.md)为准。

Dashboard 卡片、Gamer Room 与其他应用各有专项；这里不复制其过时待办。CSS normal 行高与 GPUI 默认 phi、中文字体回退、浏览器 local 字体优先级、600/1279/2560 路由级 containing block、backdrop blur，以及提示裁剪/层叠仍需逐处核对，静态字体表一致不等于运行像素一致。

## Alexa 与 Armory 默认内容

Alexa 当前实际挂载树的公共正文继承 Roboto 16px、`#ccc`、1.22 行高，各控件自身字号覆盖仍保留。Home ready 两个 br 对应 19.52px 空行；80vh 基于 42px 宿主 TabUI 下方内容高度。用户名样例只属于隔离 ready 预览，普通未知账户不伪造登录。

Skills 列表文字缩进 20px，五个 SVG 与当前资源逐字节一致；More/Close 颜色为 100ms ease-in-out、透明度 linear，按下 .7，展开 200ms ease-out。Skills 的 100 字重与 Settings 请求 200 在该应用实际字体面集合中均匹配 Thin。圆点仍为本地几何圆，未证明浏览器 list-marker 栅格一致。

Settings 帮助保留 `?`、14px 圆、13px 字/行高、`#5c5c5c` 背景与 `#f4f4f4` 字。提示位于右下 5px、300px 外层、padding 7×8px、14px/1.22、黑底与 `#5d5d5d` 边框；进入等待 100ms 后淡入 100ms，离开淡出移除，窗口失焦关闭。证据及维护工具见[Alexa 收据](alexa-content-current-evidence.json)与 `audit-alexa-content.cjs`。

Alexa 剩余范围包括 Home 各账户/验证码/error 分支的完整布局、真实复制/链接数据、Skills 本地设置持久化和连续缩放测量、真实麦克风/用户名与设置同步、快捷键捕获边界、Help 响应式内容，以及 Installer/Patch Notes/Logout 全部状态；这些不由公共字体修正代替。

Armory 无 feature 响应时使用 hook 初始 false 对应 WORKSHOP；收到有效且全 false 的响应后，源计算可能选择 Exchange。两种状态不可合并，依据为[默认标题证据](armory-default-source.json)。Browse、Downloads、Share 与产品正文其余状态分别见[Armory 应用](armory-app-current-audit.md)、[分享](armory-share-current-audit.md)和[产品根](armory-products-current.md)。

Profiles、Macro、Chroma 与 Studio 当前状态分别由[Profiles](profiles-ui-current.md)、[Macro](macro-ui-current.md)、[Chroma](chroma-dashboard-current.md)、[Studio 属性](studio-properties-current.md)记录，避免在公共报告中保留已经解决的专项待办。

## 验证范围

独立分支范围由 [displayMode 源收据](display-mode-audit.json)和[实际接入矩阵](display-mode-roots-audit.json)维护。Armory、Chroma、Macro 与多设备配对的源分支数不能当作完整页面数量；宿主 tab、独立窗口及产品内嵌根按实际打开链分别判断。维护工具只生成机器收据，不覆盖本契约。

维护工具只静态读取当前源码、CSS、资源和本地实现。允许格式化、资源准备验证及 `cargo check --locked --all-targets`；应用、构建、测试、安装器、供应商 JavaScript 与 DLL 不执行。本文不宣称窗口输入、像素、完整文件往返或真实设备服务验收通过；未核页面继续按[路线图](ui-readonly-first-roadmap.md)逐项推进。
