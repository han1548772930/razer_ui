# 滚动与弹层时序复核

更新日期：2026-10-02。按用户反馈自行检查已编译页面的布局树、原始 JS 挂载路径和 CSS 最终覆盖。仅编译与静态核对；没有运行应用或测试，以下不是实际窗口验收结果。

## 滚动区域

Kit 0.7 的 `overflow_*_scrollbar` 包装会将高度约束复制到外层，同时保留内容层的 `max_size`。限高弹窗的内容可能因此被按视口高度测量，形成内容很长却没有有效滚动范围的情况。[SourceScrollable](../../src/ui/scroll.rs) 将高度约束、原生滚动和 Kit Scrollbar 绑定在同一节点，保留调用者的元素 ID 与显式 ScrollHandle。滚轮、触控板、拖动和命中处理仍由框架负责。

| 范围 | 修复 |
| --- | --- |
| AppShell 内容槽 | 有限 flex 区域内用 absolute/inset 子层，令页面的百分比高度有确定依据 |
| Dashboard / Modules / Gamer Room / Shortcuts | 主内容区域采用新的滚动组合 |
| 独立 Pairing | 补齐整页滚动容器 |
| Settings | 选项内容独立滚动，导航与保存底栏保持在外层 |
| Wi-Fi 添加、模块预览、配置迁移、板载配置 | 限高内容与滚动所有者保持一致 |
| 发布说明 | 原有 ScrollHandle 接到真正的滚动节点，切换时回顶部作用于可见内容 |
| Profile 导入／导出／关联程序 | 对话框正文使用有明确高度的内容槽，底栏不随内容滚动 |
| SourceAlert | 长正文可滚动，标题和操作保留在外层 |
| Dashboard／Gamer Room 教程 | 媒体恢复原 250×190 比例；正文限高与原生滚动在同一节点，指示器置于滚动层外 |
| Introduction Tour | 独立双轴滚动视口；原 450px 描述列和 640×360 媒体区在短窗口仍可达 |
| 映射类别栏／正文 | 类别栏 40→250px，展开时独立纵向滚动；正文固定在 left=40 的受限内容槽，类别栏覆盖正文而不推动它 |
| 设备工作区 | 原生滚动及显式句柄继续保留；保存底栏禁止 flex 压缩 |

旧的 `features/engines.rs`、`features/setting.rs` 不在当前模块编译树中，不把修改废弃页面计作修复。[滚动回归源码](../../src/ui/scroll_tests.rs)检查 180px 视口内的 600px 内容能到底、回顶且底栏不移动；只编译，未执行。

## 按源码分别处理动画

| 界面 | 实际源码依据 | 当前处理 |
| --- | --- | --- |
| Dashboard 分组 | `.content max-height:0/2000px`、`.content-inner translateY(-100%/0)`、折叠图标及 expand/close 关键帧 | 高度 300ms ease-in；内容位移与箭头 300ms linear，支持中途反转；固定行高来自 bi.moveItems，卡片保持挂载。1s ease 的 overflow 离散关键帧按缓动进度 0.5 切换，关闭保持裁剪 |
| 共用开关 | `.switch` 背景 300ms、`.handle left:1→15px` 200ms，均为 CSS ease | 值立即更新；背景与滑块独立过渡，支持中途反转与减少动态效果 |
| 映射类别栏 | `.actions max-width:40→250px` 200ms ease，hover 时允许 y 滚动 | 按原轨迹展开／收回，文本不换行，正文位置固定 |
| Introduction Tour 按钮 | `.rz-btn opacity` 200ms ease-out | normal 1／hover 0.8／pressed、disabled 0.6；按独立按钮 ID 保存过渡，鼠标释放／移出复位 |
| 普通下拉 | 182/653/777 `.s3-options` 的 `max-height:0→180px`、`height:0→auto`；箭头 transition | 展开 200ms CSS ease；箭头 300ms ease。按最终尺寸定位再揭露内容，无附加淡入／滑动。关闭保持原来的即时隐藏 |
| Customize 输入抽屉 | `.config-drawer` 的 `left:-230px→0`，正文剩余宽度随之变化 | 200ms ease，固定 230px 内容在过渡期间保留，关闭结束后卸载 |
| 颜色预设板 | `.dropdown-color .s3-options` 的 `transition:all .1s,width 0` | 100ms ease 淡入；箭头 300ms ease；二级拾色器维持原 `display:none/block` 的即时切换 |
| 账户菜单 | `rz-user-profile-menu/Root.012dd389.chunk.js` 的 `m.showDropdown/hideDropdown`；`main.090e2108.css` 的 `.dropdown-razer` | 挂载后等 100ms 加 show；透明度 200ms linear、上移 7px 到原位 200ms ease-out；关闭立即停止接收输入，100ms 后卸载。重开取消旧任务；窗口失焦关闭；Escape 与触发器关闭恢复焦点 |
| Profile 删除／重置 | 182 `main.db20a7c4.js` 的 Profile 组件常驻挂载 `Vu/Gs`，653/777 对应分支；`.profile-del` | 300ms linear 淡入；关闭的 visibility 立即隐藏。没有套用默认 Dialog 滑入 |
| Wi-Fi 添加 | frontend `IotPopupRoot` 的 `gt` 使用 `82830`；`55.4acb3322.chunk.js` 的 modal show 计时器及 `55.a5b041a2.chunk.css` | 挂载后 100ms，面板和遮罩分别以 150ms linear 淡入，无位置变化。当前退出销毁根窗口，没有擅加一个原调用路径未触发的 isMounted=false 退出序列 |
| 板载配置／映射编辑器 | `.obm-menu.key-config` 和 `.key-config` 的 opacity/visibility/left 过渡为 0s | 保持即时显示 |
| 通用确认、发布说明及其他弹层 | 必须结合实际挂载判断，不能只凭 CSS 中存在 transition | 保留已审计位置及样式；不统一添加动画 |

账户菜单的隐藏阶段用无交互的外观节点，避免透明列表仍截获点击。Kit Popup 固定启用遮挡，因此该菜单组合 Base Positioner、PopoverState 和 List：定位、焦点生命周期、键盘导航仍交给框架。正常显示时才遮挡；隐藏阶段允许输入落到页面。

已有检查最终布局的测试显式启用 reduce motion；专门的下拉动画与[账户菜单回归源码](../../src/shell/account_menu_tests.rs)保留正常 motion，检查延迟、位移、Escape、快速重开和隐藏阶段点击。所有用例只编译，未执行。

最终静态检查：`cargo check --locked --all-targets` 通过，无警告；`python -B tools/validate-resources.py` 核对 341 条源／输出哈希、98 个 Webpack 请求、56 个产品变体、41 个 Dashboard 变体与 16 布局／1901 个输入形状；新增 Python 脚本通过 AST 语法检查，JS 清单工具通过 `node --check`。资源校验同时读取 8 个动画 WebP 的 RIFF/VP8X/ANIM/ANMF，核对画布、帧区域、逐帧延时和循环次数。产品目录重新逐文件核验下载哈希，331 个清单共 30,658 份 JS/CSS 齐备；沿明确 lazy 根分支，331 个产品解析到导航，77 种导航签名。

## 待核对范围

- Tooltip 各种原始延迟和淡入策略、状态提示、条件警告尚未全部逐项对齐。
- Dashboard 现有设备、模块、在线服务组已接入卡片拖动、排序和折叠持久化，动态验收仍待执行。`4130 Li.renderGroup → Ii → bi` 没有挂载组拖动手柄、backdrop-box 或 `.show-overflow`，不能把这些未使用的 CSS 当作当前页面缺失功能；安装器／推荐等尚无真实数据的组仍待接入。
- 宿主页签的平滑横向滚动与完整应用注册仍待接入；拖动排序、键盘换位及位置过渡已实现，尚未动态验收。
- 未适配产品、独立应用及硬件服务的运行时分支不在这次 UI 修复覆盖内。
- 实际窗口缩放、触控板、拖动滚动条、最小窗口与快速连续操作尚未动态验收。

产品来源与代码取得进度另见[产品目录](16-product-catalog.md)，不可据此宣称所有产品界面或真实服务已完成。

## Introduction Tour 与媒体

五步 Synapse 教程已恢复，独立宿主页签保持步骤，关闭清理该页签的导航历史，并按原宿主顺序激活右邻、左邻或保留的主应用页面；重开从第一步开始。原源码及限制见[规格](../screens/16-introduction-tour.md)。回归源码覆盖按钮导航、关闭事件、短窗口双轴滚动，仅编译未执行。

[Dashboard 折叠回归源码](../../src/shell/main_pages/dashboard_group_tests.rs)检查中间帧仍保留卡片、max-height 与位移不同步的原行为、快速反转连续性和收起后后续组的位置；只编译，未执行。

## 宿主页签与图标按钮

[HostTabs](../../src/shell/host_tabs.rs) 将打开的页签与设备工作区分开。关闭不删除设备及未保存的配置／映射草稿；Dashboard 或 Ctrl+Shift+T 可以重新打开原工作区，应用保存和退出检查仍覆盖隐藏工作区。主应用页签不可关闭，保留最后访问的主应用页面。Tour 关闭销毁其步骤状态。

原 TabUI `_findNextTab` 的右邻优先、末项选左邻、关闭非当前项保持选中已接入；关闭清理历史中的该页签，避免后退重新打开。关闭按钮与切换按钮为兄弟节点，中键与 Ctrl+W 也走同一关闭逻辑；Ctrl+Tab／Ctrl+Shift+Tab 按顺序循环。移除持有焦点的页签会把焦点放回当前页签；模态 Dialog 获得焦点时不处理这些宿主快捷键。

外层 42px，页签由 20px 图标、上下 6px 和外层各 1px 得到 34px 高；min/max 为 90/240px，间距 4px。恢复 5px 顶部圆角、active／hover 渐变、12px weight 300 文字、长名称 45px 遮罩、24×24 关闭目标和原版三态 SVG。关闭时移除当前节点，其他页签的 left 用 200ms ease 移动，不加淡出或缩小。页签溢出保留横向滚动及原 20px 箭头状态，Windows 三按钮固定 48×42，最大化时切换 restore 素材。设备图标分别来自已保存 manifest 中声明的 MOUSE／KEYBOARD／AUDIO favicon。

共用 `asset_button` 的 SVG 状态改由整个按钮的 hover／active 驱动。帮助图标恢复 24×24、透明背景；active 素材只表示当前帮助页，未选中时按下沿用 hover 素材。均衡器重置沿用自己的三态素材。

[页签回归源码](../../src/shell/host_tabs_tests.rs)覆盖关闭当前／非当前、右邻／左邻、重复打开、恢复顺序和关闭栈去重；仅编译未执行。仍需动态核对窗口缩放、实际像素、焦点与连续操作；不能将静态检查当作运行验收。

OBM 提示已替换原生光标定位，按实际门户／CSS 两条路径分别处理，详见下文。其他 Tooltip 的延时和位置规则继续分别核对。

### 宿主页签拖动与排序持久化

继续对照 `TabUI.onMouseDown/onMouseMove/onMouseUp`、`handleSwitchTabPosition`、`swapPos`、`calculateTabWrapperWidth` 和宿主 `common.js` 的快捷键表。

[拖动实现](../../src/shell/host_tab_drag.rs) 使用 GPUI 原生拖动生命周期，空预览视图避免出现原版没有的浮动副本；实际页签保留在条内并提升绘制层级。主应用页签没有拖动入口。拖动项即时跟随鼠标，相邻项及松手归位使用已有 200ms ease 位置通道；越过相邻项宽度的 1/3 后换位，支持不同宽度。达到滚动边缘调整可见区域，鼠标离开窗口停止本次位置跟踪，释放原生拖动会清理状态。拖动期间关闭图标隐藏。

补齐 Ctrl+Shift+PageUp／PageDown，改变当前页签位置而不改当前页面或导航历史。鼠标切换在按下时执行，与 TabUI 的 mousedown 一致；Base 继续处理键盘和触屏激活，鼠标松开不重复提交导航请求。

原宿主按窗口宽度保留拖动空白：`min(floor((width+1-600)/16.5+10),100)`。左右溢出按钮分别位于页签条两侧，窗口缩小时重新显示当前项。主标题经 App.B／公共模块 57881 核对：只有 zh-CN 显示“雷云”，其余语言显示“SYNAPSE”，宽度随真实文字测量。此前最后一个页签被当作空白子节点跳过的问题已修复。

排序写入本地 WorkspaceFile 的可选 `host_tab_order`，旧文件缺字段时正常读取；恢复时忽略不在当前打开列表的项，新项保持追加顺序。排序通过已有串行辅助保存流程写入，使用设备／快捷键／Settings 的已保存快照，不提交其编辑草稿。保存完成只确认当时捕获的顺序，期间再次换位会继续保存较新的顺序。普通关闭页签不单独产生未保存布局警告。

新增回归源码检查 1/3 阈值、不同宽度、反向移动、旧存储兼容、排序恢复及保存完成晚于新修改的情况；全部只编译，未执行。原 CSS `scroll-behavior:smooth` 的浏览器时序尚未复刻，现有滚动到目标仍采用原生即时定位；不将位置动画冒充滚动动画。

### 板载配置提示与关闭按钮

进一步追查 653 `main.7b71cce5.js` 的 `AR = n(61050)`、`cR`、`pR` 和 `mR` 后，纠正仅依据 `.tip` CSS 作出的定位／关闭判断：

| 入口 | 实际调用与处理 |
| --- | --- |
| 标题／混合配置帮助、BLE 可用内存说明 | `61050`／`mR` 将提示门户化为 `.drop-tips`，以图标 right/bottom 定位；横向不足改为 left−clientWidth，纵向不足改为 bottom−clientHeight。保留位置 100ms ease、透明度 300ms linear、visibility 200ms；离开时立即清空文字，外框继续退出。不同帮助按钮各自保留状态 |
| 配置警告图标 | `cR` 将 `.tooltip_parent` 覆盖到图标位置，提示在图标左下，宽 300px；悬停范围包括提示本身。300ms linear 透明度，失去悬停即隐藏 |
| 锁定槽位 | `pR.onMouseOver` 覆盖 CSS 中的 right/top，设为 fixed，位置为图标 left/top 各加 20px；宽 305px。悬停只跟随图标；隐藏即时，不沿用门户提示的退出时长 |

[SourceTooltip](../../src/ui/source_tooltip.rs) 使用原生 hover、Base Tooltip、Presence／Transition 和 Positioner。小型 Element 只负责源提示测量及在同一帧读取触发器坐标；不依赖鼠标所在像素，也不重新实现输入分发。门户路径先测量隐藏 `.tip`，将不含边框的 clientWidth/clientHeight 用作门户尺寸；标题的 100% 源宽来自 270px 标题的 268px 内框，混合配置为 290px，BLE 为 210px。最终定位仍由 Base 限制在可用窗口内；这是小窗口的原生适配，并非复刻源码允许固定提示溢出窗口的情况。

帮助圆点背景恢复 `#4a4a4a → #6c6c6c`，300ms ease；BLE 信息图标保持透明背景，两者保留原悬停指针。混合配置行恢复 179px 名称宽度与仅在名称前的 10px 间距，移除帮助图标前多加的空隙。锁定槽位采用同一文字容器、170px 文本上限和右侧固定图标。182／653／777 的 `.keymap-head .close` 均为 36×36 目标、20×20 图标、200ms ease 背景；映射编辑器和板载配置已共用该按钮。透明背景过渡使用预乘透明度，避免直接插值 HSL 产生暗闪。

[提示回归源码](../../src/ui/source_tooltip_tests.rs)覆盖无等待显示、光标在图标内移动不改变位置、窗口右下角换边、超出祖先裁剪、200ms 隐藏、快速重入，以及警告与锁定提示不同的悬停范围。`cargo check --locked --all-targets` 通过；用例未执行，悬停命中、实际文字换行及像素动画仍需窗口验收。

### 教程指示、按钮尺寸和右上关闭复核

按反馈回到实际资源及挂载路径，确认后修正，而不是统一替换按钮或给所有弹层添加同一种动画。

| 问题 | 原始依据 | 修正 |
| --- | --- | --- |
| 初次显示的大橙点 | `indicator.b7ce7af4.svg` 的 `Circle_1/2/3`、`Static_Small` 和 SMIL；55 CSS 将 viewBox 50 的图显示为 36px | 静态 SVG 解码不执行 SMIL，会留下整块 Circle_1。改为原生几何绘制：7.2px 常驻中心，三个实心圆按 0/1/2s 起始、2s 线性扩张淡出、每 3s 重启，保留原绘制顺序。减少动态效果时只保留中心 |
| Dashboard 指示位置 | `4130 Si.v()` 设置 panel left = 第二导航 offsetLeft−92、indicator left = offsetLeft+50；CSS top 分别为 108/70，indicator 为 fixed | 指示器从弹框布局分离，仍指向 Gamer Room 导航；恢复初次 100ms 测量等待。宿主标题栏 42px 外加到前端坐标；窗口缩放时跟随原生导航布局同帧定位，弹框限制在窗口内不会拖走指示器。窗口边界限制及不等待浏览器 resize 的 100ms 计时器属于原生适配 |
| Gamer Room 两步指示 | `9388 ce`，55 `.gamer-room-tutorial-modal` 与 override | 用同一原生指示器；第一步 left−9%/top0，第二步 left−10%/top50%，保留原平移与两个弹框锚点；外点不再意外完成教程 |
| 教程按钮和跳过 | 55 教程 CSS；9388 `P.nSM/IUp/DHM/K29` 对应 `TUTORIAL_SKIP/BACK/DONE/NEXT` | Base Button 保留原 100×27 边框盒、12px 字号、黑边、灰／橙背景，不受 Component 默认字号和内距影响，也不被 flex 压窄。禁用状态立即影响交互，透明度用 300ms ease；主按钮 hover 背景立即变化。跳过恢复右20/top15、下划线与绿色 hover，并使用源语言键 |
| 宿主右上关闭／还原拉伸 | Electron `index.css .etabs-window-control-btns`、`TabUI.addWindowControlBtns`、`assets/image/tab/*.svg` | 命中区域仍为 48×42；关闭资源按 12.7×12.7、还原按 13×13 居中。最小化／最大化保留资源自身 48×32 画布，不再把紧裁剪图标拉伸到该画布 |
| Wi-Fi 添加关闭 | 55 `.modal-content .btn-close` | 使用原 `icon_close.55fe41f1.svg`（mapping-close）、36px 目标、20px 图标、右上4px圆角。背景透明→白10%→黑30%，100ms ease-in-out；与 keymap 的 200ms ease、黑10%按下态分开 |
| 未保存确认关闭 | `.save-alert .close` | 保留20px目标、top/right8；移除 Component ghost 额外的悬停背景和原生提示，继续编辑且不提交保存动作 |

本次新增 9 个回归用例，覆盖 [指示动画及按钮尺寸／禁用过渡](../../src/ui/tutorial_tests.rs)、[Dashboard 首次等待和窗口缩放](../../src/shell/main_pages/dashboard_tutorial_tests.rs)、[Gamer Room 步骤和锚点](../../src/shell/gamer_room_tutorial_tests.rs)、[宿主图标与关闭命中](../../src/shell/host_tabs_tests.rs)、[两类关闭过渡和移出取消](../../src/ui/surface_tests.rs)、[Wi-Fi 三种入口关闭](../../src/shell/iot_popup_tests.rs)、[未保存确认继续编辑](../../src/ui/source_alert_tests.rs)。颜色时序用例读取实际生产 Button 实例上应用的样式，交互仍经原生鼠标／键盘事件，不直接调用处理函数。

`cargo check --locked --all-targets` 通过，无警告。用例仅编译，未执行；没有启动应用、服务或 DLL。像素外观、原生窗口关闭与完整焦点链仍须动态验收，不能由这些编译结果推断全部界面完成。

### Dashboard 卡片拖动、层级与布局保存

继续沿 `4130 be.mouseDown → bi.dragMouseDown/getDragingItemPos/estimateDragItemLocation/onMouseUp` 和 `55` 的 `Ve` reducer 核对。[卡片组合](../../src/shell/main_pages/dashboard_cards.rs) 将设备、固定模块、在线服务接入同一个[网格实现](../../src/shell/main_pages/dashboard_grid.rs)，空设备卡保留原有外链、不参与拖动。在线服务仍使用 Base Link，应用命令使用 Base Button。

- 卡片为 290×220，横／纵步距 310／240；网格高度为 `220 + (rows−1)*240`。调整窗口时按现有 `Li.getMaxColumns` 列数重排，不修改设备实体数组。
- 在 mousedown 记录原始抓取点与卡片目标位置，第一像素移动即更新；GPUI 达到自身拖动阈值后接管拖动捕获及释放，使用空预览，卡片仍在原组。限制在 `[0,(columns−1)*310] × [0,(rows−1)*240]`。
- 只有卡片经边界限制后的任一轴位移 **大于 10px** 才取消点击；越界后移回仍不恢复点击。短距离原生拖动和普通点击都只执行一次动作，长拖动不打开设备、教程或浏览器。
- 目标列／行分别使用 `floor((left−145)/310)+1`、`floor((top−110)/240)+1`，末行空格限制到最后一张卡。每组单独移除／插入 ID；缺失 ID 不占位置，新 ID 稳定追加。
- 源 document wheel 监听仅在按住卡片期间安装；GPUI 的滚动量转换为方向相反的 DOM delta，再加入指针位置。阻止页面同时滚动，释放后恢复普通滚轮行为。
- 拖动卡片即时定位；其他卡片及松手归位为 300ms CSS ease。原 `xi` 的层级为普通卡 `count−index+1`、拖动卡 100；本地绘制队列按此排序，保留布局 ID、页面／折叠裁剪和 Base 控件，不让卡片层级盖住弹层。边框为透明／绿色 `#44d62c4d`／绿色实色；source `.box-item` 的 default 光标保持不变。
- 原生拖动取消或卡片在期间移除不触发打开，恢复该组此前顺序；其他分组修改保留。如果中间状态已被另一轮保存捕获，恢复也会重新请求保存。

`WorkspaceFile.dashboard` 是可选本地字段，含 `items_order` 和 `groups_collapsed`；旧文件不含字段时正常读取，不声称兼容原 Synapse 存储。Dashboard 状态独立于设备、快捷键和 Settings 草稿，沿现有串行辅助保存入口落盘；正常拖动在松手时合并写入。每次完成只确认当时捕获的布局，晚到的写入不清除较新的修改。完整保存与设置保存也携带该布局，避免其他保存路径把它重置。

本次新增 11 个回归用例：[网格及真实输入路径](../../src/shell/main_pages/dashboard_grid_tests.rs)覆盖阈值、点击只触发一次、跨行及末行换位、独立组、晚到保存、折叠恢复、动画中间帧、重叠命中层级、滚轮方向和页面滚动、外链误开、缩放、原生取消；[存储用例](../../src/store.rs)覆盖旧字段缺省、布局往返及设备数据不被重排。全部仅编译，未执行；不能把源码核对和 `cargo check` 当作拖动、滚动或窗口渲染验收。

### 标题栏拖动、Windows 还原和 Settings 按钮

根据新增反馈，重新核对 Electron `index.css`、`TabUI.js` 和 `main.js`，同时检查当前依赖的实际 Windows 实现。

- 原 `.etabs-tabs` 及 `.etabs-tabs-wrapper::after` 都是 `-webkit-app-region:drag`，`.etabs-tab` 和窗口按钮是 `no-drag`，拖动页签时 `.etabs-tabs.dragging` 取消原生拖动。此前只给条尾预留的小区域设置了 Drag，页签内剩余空白和上方没有接入。[HostTabs](../../src/shell/host_tabs.rs) 改为整个 42px 容器提供原生 Drag 命中，页签、滚动箭头、三个窗口按钮通过自己的 hitbox 排除；页签排序期间关闭 Drag。双击、拖动最大化窗口等由 Windows 非客户区处理，未另加竞争的鼠标拖窗处理器。
- GPUI 的 Win32 命中检查使用 `hit_test.ids`，仅停止事件冒泡或 `block_mouse_except_scroll` 不足以排除 Drag，因此此处需要 `occlude`。它同时阻断父节点的滚轮事件，设备／Tour 页签把滚轮交给同一个 `ScrollHandle`，沿 GPUI 单轴规则优先使用 x、否则 y，布局仍由原滚动节点限制范围。没有添加占用子节点索引的填充项。
- 原 `main.js` 的 `W.MAXIMIZE` 执行 `isMaximized() ? unmaximize() : maximize()`。当前 `gpui-pre-windows 0.3.7/src/window.rs::zoom` 对可见窗口只调用 `SW_MAXIMIZE`，原先直接调用它导致无法还原。[窗口边界适配](../../src/shell/host_window.rs) 从当前 GPUI Window 借用 HWND，点击时用 `IsZoomed` 查询系统状态，再投递 `SC_RESTORE`／`SC_MAXIMIZE`，避免同步 `WM_SIZE` 重入 GPUI；不缓存第二份最大化状态，不使用系统前台窗口句柄。按钮图标及可访问名称跟随窗口实际状态，鼠标和键盘使用同一回调。仅新增已有锁定版本的 `raw-window-handle 0.6.2` 作为 Windows 直接依赖，无依赖升级。
- Settings `720.1e5d1c8f.chunk.js` 的教程重置和配置迁移都实际挂载 `.setting-block .setting-row .thx-btn.test`；`720.dbc9cca5.chunk.css` 规定 27px 高、最小宽度 100px、横向内距 10px、12px 字号，灰底白字及黑色 30% 边框。两处入口改用[源按钮](../../src/shell/settings_button.rs)，保留 Base Button 的语义及输入处理，允许长文字自然撑宽，不受 flex 压缩；与说明垂直居中。还原 300ms ease 透明度过渡：普通 1、hover .8、按下 .6、禁用 .3；禁用即时阻止动作，不改变尺寸。只修改已核对挂载的两处入口，没有给所有按钮套用相同宽度。

新增 5 个回归用例：[标题栏与按钮输入](../../src/shell/host_tabs_tests.rs)检查空白／页签上方命中、控件排除拖窗、滚轮、最大化图标和 Enter／Space 单次激活；[Settings](../../src/shell/settings_tests.rs)检查实际页面在 1500／1080 宽窗口的按钮尺寸、重置后禁用尺寸不变、长文字、窄行、hover／pressed／disabled 动画及移出取消；[Windows 原生用例](../../src/shell/host_window.rs)检查重复最大化／还原后的边界恢复及外部最大化后的还原。原生用例会创建临时 Windows 窗口，显式标记 `ignore`，需桌面环境单独运行。

`cargo check --locked --all-targets` 通过，无警告；限定文件格式检查和 `git diff --check` 通过。5 个用例均只编译，未运行；原生窗口拖动、双击、缩放／还原和实际像素仍未动态验收。本次结果不代表全产品或全部页面已经完成。
