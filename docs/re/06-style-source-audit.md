# UI 样式复核与资源同步（2026-10-01）

本轮只以 `.ref` 的原版 JS、CSS 和实际资源为依据。项目中的旧截图不作为规格、尺寸或验收依据。本文记录本轮新增的源码结论及实现；完整功能缺口仍见[重构状态](03-implementation-gap.md)。

## 1. 原版定位入口

| 范围 | 原文件及定位标记 |
|---|---|
| 设备导航、鼠标区、点阵、模式开关 | [182 CSS](../../.ref/devices/182/static/css/main.48c20423.css)：`.toolbar`、`.nav-tabs`、`.config-wrapper`、`.config-btns`、`.config-profile-img`、`.dot-bg`、`.hyper-wrapper` |
| 鼠标绘图与按钮文案 | [182 JS](../../.ref/devices/182/static/js/main.db20a7c4.js)：`KP.render/drawLines/getCycleImage`、`Yp.render`、`wp`；模块 6482 的 `h5` 导出 |
| 键盘图和高亮 | [653 JS](../../.ref/devices/653/static/js/main.7b71cce5.js)：`km → TM/IM`、`Wp → Yp`；[653 CSS](../../.ref/devices/653/static/css/main.5425442a.css)：`.custom-keyboard-svg`、`.svg-key`、`.isAssignment` |
| Sound/Mic EQ、音量和直播灯效 | [777 JS](../../.ref/devices/777/static/js/main.eb70ce38.js)：`VM`、`GM → uM → cM`、`UM`、`dM`、`qM`、`VG`；[777 CSS](../../.ref/devices/777/static/css/main.e4bab2aa.css)：`.sliderChart__*`、`.vertical-slider__*`、`#eqBox`、`.stream-*` |
| 宿主条 | [Electron index.css](../../.ref/synapse-asar/electron/index.css)：`.etabs-tabgroup` 及系统按钮 |

压缩 JS 的符号只在对应的文件版本内有意义。CSS 应检查所有同名规则及媒体查询；例如 653 后部覆盖了共享的 config-wrapper 高度，不能只取首次命中。

## 2. 已纠正的外壳与公共控件

- 宿主栏 42、工具栏 38、设备导航 48；工具栏底色是 `#222`，设备名在中间。原实现把本地保存状态放在设备名的位置，已改为底部状态行。
- 原导航为 12px 文案、28px 高胶囊、14px 圆角、20px 项间隔。现使用框架 Button 的焦点与激活行为，配合 TabList/Tab 语义，不再套默认页签外观。
- Profile 与帮助区域分担导航两边的空间。窗口变窄时 Profile 选择框可以收缩；帮助图标的边距放在子元素上，避免右侧 padding 让中间导航偏左 5px。
- 默认 GPUI Switch 是 36×20，且主题半径为 3 时轨道会成为圆角矩形。原版是 32×18、14px 深色圆形滑块、16px 轨道圆角。`surface::SynapseSwitch` 使用 Base Switch 的受控值、焦点与键盘行为实现原版尺寸。
- 亮度、音量的开关放回标题行。原版 `qM/dM` 通过 `hasSwitch` 传给 widget，不存在另一行重复标题。
- 本地保存仍是本项目独立能力。设备保存/丢弃栏只在有草稿时显示；底部状态行说明本地保存或错误，不代表硬件写入成功。

尺寸由 `surface::css` 按 16px 基字号转换为 rem。点阵与产品/命中坐标使用同一比例；描边仍保留设备像素边界。

## 3. 鼠标与键盘区

182 的 `h5` 明确为 `cvsW=310`、`cvsH=340`、`xLeft=3`、`xRight=307`、`yStart=16`、`lineLength=53`、`dotRadius=2.5`、`circleRadius=5.5`。画布距 770px config-block 左边 230px。

原版鼠标按钮是透明底文字命令，30px 高、50px 行距，hover 为 `#383838`，active 为 `#111`。左侧按内容右对齐，右侧左对齐，不能改成两列等宽描边卡片。左列末项虽然位于左边，`KP` 仍传 `side="right"`，它的 margin 方向也单独处理。

`getCycleImage` 与 `zp` 同时存在于 render 中：底部 DPI 资源是额外叠层，不是替换正面图。两张资源均为 900×1020，在布局中按 300×340 显示；正面位置 x=235、y=0，底部叠层位置 x=272、y=-77（`right:calc(50% - 187px)`）。因此叠层相对正面偏移为 +37、-77。hover/选中 DPI 输入时启用叠层，连线锚点改为 185、155。

连线已恢复圆形端点和活动外圈。Standard/Hypershift 改为 36px 外框内的两个 24px 选项，普通层绿色，Hypershift 橙色。产品区点阵按 `.dot-bg` 的 22px 网格和 `.dim-corner` 渐隐绘制。

653 的原版 `.svg-key` hover/active 是 2px 轮廓；普通映射填充绿色 40%，Hypershift 映射填充橙色 70%。已修正默认灰色 Button 选中块。[keyboard_geometry.rs](../../src/ui/keyboard_geometry.rs) 按 groupList 的 path/circle/rect 同时绘制和命中：保留原 Bezier 控制点，以非零绕组解析曲线相交，鼠标按下/抬起均在真实形状内才激活，圆角外侧和孔洞不吞掉下层点击。焦点、Space/Enter 继续由 Base Button 管理。布局 1 的 Enter 实际是横向圆角 path；不能将其它布局的 L 形 Enter 描述为布局 1。当前已接入全部 16 个已知布局，共 1901 个命中形状。

[customize_drawer.rs](../../src/features/customize_drawer.rs) 已接入 230px 左侧输入列表：67px 顶栏、所有/自定义筛选、最小 50px 行，列表与设备正文独立滚动。原版没有文本搜索框。列表打开后正文保留 1000px 加两侧 20px 内边距；列表来源编辑器锚定 left=210px，图上来源按原左右规则定位并限制在窗口内。抽屉、图上输入和映射编辑器使用同一继续流程及能力过滤，详见[自定义规格](../screens/01-customize.md)。

## 4. EQ 的真实结构

`uM` 的顺序是：频段列表 → Y 轴文字 → Reset。原实现把刻度放在左边、重置放在底部，还同时显示 SVG 和通用重置图标；已按原组件恢复。

| 项目 | 源码契约与当前接入 |
|---|---|
| 普通预设 | 高 27、最小宽 90、12px 文案、gap 12；允许换行 |
| Mic 预设 | `tabScale=true`，宽为内容区 +10px，等分伸展 |
| Sound 频段 | `narrow`：左 padding 15、右 16，输入容器宽 16，合计 47 |
| Mic 频段 | `wide`：左右各 31，输入容器宽 16，合计 78 |
| 纵向 range | 总长 300、轨道宽 6、圆形滑块 16；滑块中心有效行程 284 |
| 轨道显示 | 两侧暗色、中央零点亮色标记；不显示从最小值到当前值的实心进度填充 |
| 频率 | 低于 1000 用 `Hz`，其余用 `kHz`；不是无单位的数字 |
| Reset | 20×20 图标，在 300px 高区域中垂直居中；使用原 default/hover/active SVG |
| Mic 容器 | 宽/最小宽/最大宽 940、最小高 473；Sound 使用普通 widget 列 |

特别注意：GPUI Component `Slider::vertical()` 默认自身高度为 120，给外层容器设置 300 并不会自动覆盖它。本轮 EQ 使用 Base Slider/Track/Indicator/Thumb，显式设置真实轨道高度，并继续引用 owner 内原有 SliderState 和订阅。拖动、Custom/Reset、Sound/Mic 数据隔离未换成新的状态模型。

数值提示已改为原 `cM` 的随动气泡：26×20、左偏移 −28、3px 圆角、绿色背景/黑字，正值带 `+`、零显示 `0`，不追加 dB。原脚本的 `top=-20-2.84*百分比` 配合 span 的 top302，得到实际纵向位置 `282−284*归一化值`；气泡只在悬停滑条时出现，不参与布局。真实视图测试源码检查 −5/0/+5 的位置、尺寸和悬停显隐；本轮未执行，静态检查状态见[重构状态](03-implementation-gap.md#5-验证状态)。

## 5. 资源同步方式

[生成器](../../tools/prepare-resources.py) 现在同时处理位图、字体、SVG 与几何清单；以前单独复制的 EQ Reset、Chroma 和 Command Dial SVG 已纳入生成过程。

- 新接入 `.ref/devices/182/static/media` 的历史箭头、设置、profile、Synapse 标志与帮助图标。
- `icon_help.377359c3.svg` 是带 `default/hover/active` 视口的精灵。原版 CSS 通过 URL fragment 选视口；本地生成器为三个状态设置独立根 viewBox，保留原路径，不让 native decoder 把整张精灵作为一个图标。
- EQ 三态 SVG、653 dial-mapping/Chroma 图直接复制，资源内容不改写。
- 抽屉两态和原closepanel图标已接入；映射分类直接使用14枚原SVG和原CSS内嵌灯光PNG；帮助页内联上下箭头保留原path。宏、跨设备和Chroma等分类的服务不因图标已同步而启用。
- 字体转换保留源字体时间戳，避免每次生成仅因执行时间改变输出。

[manifest.json](../../assets/synapse/manifest.json) 逐条记录 source/output 和两侧 SHA-256；[embedded.rs](../../assets/synapse/embedded.rs) 由同一清单生成，`resources.rs` 直接包含此表，避免“生成了文件但没打包”或运行时依赖 `.ref`。本轮随后还扩充了 DPI 阶段、Windows 属性、校准关闭、主应用操作、灯光以及动态产品变体，最终条数以生成清单和资源校验结果为准。

本轮只读复核命令：

```powershell
node tools/extract-keyboard.cjs --check
python -B tools/validate-resources.py
```

生成所需 Python 包见 [requirements-resources.txt](../../tools/requirements-resources.txt)。校验脚本只使用标准库。本轮两项只读复核通过，检查 160 条源/输出 SHA-256、98 个原 Webpack 请求、56 条产品解析项、PNG 格式与尺寸、SVG/帮助视口、嵌入清单一致性，以及 16 个布局 / 1901 个命中形状；未重新生成媒体或运行应用。

## 6. 验证范围与剩余项

[生产视图测试](../../src/features/workspace_tests.rs) 增加了 1080/1280 宽及 125% 字号比例下的导航和鼠标双图布局；验证 Sound/Mic 47/78 间距、实际轨道高度、右侧 Reset、940px Mic 容器以及开关的 Tab/空格路径。原有保存、切 Profile、映射退出和灯效编辑测试继续覆盖状态回归。

尚未完成：映射中的宏/跨设备/Chroma 服务、登录映射警告和部分条件分支、部分通用控件的步进/气泡/刻度/动画、宿主完整应用标签及全部状态图标。产品图片与 Customize 几何均已按 edition/layout 接入；腕托及拨轮图层仍需运行时状态。设备能力、电池、Chroma 安装和硬件 transport 不能从静态资源推定为可用。

## 7. 主应用各页面同步修正

[main_pages.rs](../../src/shell/main_pages.rs) 现在分别实现四个主应用入口，原版依据仍是 HomePage 的实际 lazy/render，而非 `TAB_*` 常量集合。

| 入口 | 本轮落地 | 仍受数据/资源约束的部分 |
|---|---|---|
| Dashboard | `4130` + `55 CSS` 的 290px 整卡按钮、250×140 产品图区域、14px 居中文字、20px 卡间距和设备组折叠 | 只展示本地已知设备；安装、推荐、营销和在线服务分组没有数据时不伪造；排序/拖放尚未完成 |
| Devices & Modules | 从 Dashboard 独立成列表；`6505` 的 80px 行、40px 图区、24px 绿色标题、27px 操作按钮 | 本地快照不能称为已安装状态；模块安装/固件更新分组须等待真实服务数据 |
| Gamer Room | `9388/He` 与 `55 CSS` 的 186×176 添加设备卡、虚线边和说明 | 连接入口为服务未连接状态；本地缺失营销图片和教程视频，不构造假的房间设备 |
| Global Shortcuts | `7282/Oe` 的说明、右上添加入口和 70px 虚线添加卡 | 按键捕获、AppEngine 和全局注册未接入，添加入口禁用；不是已完成的快捷键编辑器 |

182 Dashboard 使用已有的专属 Dashboard PNG，来源与动态原 URL 记录在资源清单。653/777 的 `PluginImages/*dashboard*` 不在本地，使用标志占位；不会把 Customize 的产品图改名当作 Dashboard 图。产品预览按钮移入本项目的设置页，避免混入原版设备卡列表。

## 8. 性能、校准、电源与下拉细节

- Performance 按 `IM → jm → Vm` 使用固定五槽竖排阶段：通常 68px，各槽 X/Y 独立时 114px；各槽保留数字输入、250px 滑条、启用状态和独立 XY。至少两槽启用，关闭阶段显示只隐藏其他槽且保留值；原拖动图标支持拖放，另提供 Alt+上下方向键排序。稳定槽位 ID 保留控件实体、轴值和活动身份，旧配置按既有阶段值迁移；原版数字步进按钮细节仍待对齐。
- 校准介绍按 `DD` 修正为 26px 标题、36×36 关闭命中区内的 20px 原图；保持介绍确认偏好的持久化行为。此前文档写成 20px 标题已纠正。
- 182/777 电源继续使用各自的范围和禁用规则，恢复滑条端点/数值提示；777 开关放入卡片标题行。
- Profile、映射与灯效下拉使用同一个 `surface::select` 外观包装：27px 高、14px 文字、透明底、直角边框、原展开箭头、展开/焦点绿色边和 0.3 禁用态。原 SelectState、选择事件与键盘/焦点仍由框架管理。
- 普通菜单已通过 Base Select + Base Popover + List 组合恢复黑底、25px 行、无外围 padding、无勾选占位、绿色已选文字和白色10% hover；保留原 SelectState 与 Confirm 订阅，完整说明见[页面覆盖](07-page-coverage.md)。
- Audio Meter 的 `DU` 已从滑条改为 60×27 数字步进框：右侧18px上下箭头使用原 SVG，范围 .25–4、步长 .25。输入期间保留草稿，Enter/失焦提交，按原 Stepper 44230 和 DU 的规则向上对齐到下一档；此前四舍五入已纠正。方向键和箭头由 Base NumberInput 驱动，当前没有实现原版按住箭头每300ms连发和聚焦后滚轮步进。

## 9. 动态产品图片与对应几何

原版产品图根据 context import、DeviceInfo 与实际设备数据中的 productId/editionId/layoutId 选择，不是一张固定文件。当前 [model.rs](../../src/model.rs) 保存 edition/layout 身份；[resources.rs](../../src/resources.rs) 的 `device_image(pid, edition_id, layout_id, part)` 查询生成的产品资源索引。编译打包仍使用 include_bytes，运行时根据设备身份选择对应项，不访问 `.ref` 文件夹。

实际 UI 已接线：`surface::product_image/product_banner` 接收三个身份字段；Customize 鼠标正面与 DPI 底图分别解析 Product/MouseBottom，键盘产品图按 layout 解析；Sound 的耳机图同样读取当前设备 edition。生成器另按原 source context 同步 KeyboardWrist、KeyboardDial 和 Streamer 资源。KeyboardDial 在该 resolver 中对应 `km` 的 roller 图层，edition 130 用白色 roller，其余用普通 roller，不等于 Command Dial 卡片中的另一张 dial 插图。

缺失 edition 按原 ProductImage 的回退逻辑尝试 edition 0，保留同一 layout。`km.render` 传入 `layoutId || 1`，因此旧配置没有 layout 或为 0 时使用原布局 1 并启用对应命中。非零且无法识别的编号显示基础布局预览，这是本项目补充的失败显示策略；解析结果标记 `is_fallback_preview`，不启用命中或映射。全部 16 个已知布局使用各自位图、精确形状和完整 groupList；每布局 4 个无几何拨轮子输入也接入抽屉和编辑器。腕托连接及拨轮 hover 图层仍缺运行时状态，不能把子输入已可编辑记成真实设备图层已接通。

全部主页面和附属界面的实际入口及剩余差异见[完整覆盖记录](07-page-coverage.md)。

## 10. 原版响应式规则与正文滚动

本次逐条读取 media 外层后再判断规则，避免把窄屏覆盖当作全局尺寸。来源为 [182 CSS](../../.ref/devices/182/static/css/main.48c20423.css)、[653 CSS](../../.ref/devices/653/static/css/main.5425442a.css)、[777 CSS](../../.ref/devices/777/static/css/main.e4bab2aa.css)，以及主应用 [55 CSS](../../.ref/frontend/static/css/55.a5b041a2.chunk.css)、[6505 CSS](../../.ref/frontend/static/css/6505.9782778c.chunk.css)。

| 范围与媒体条件 | 原版实际规则 | 本轮生产代码 |
|---|---|---|
| 三产品默认 `.body-widgets/.widget-col` | 容器 max-width 1240、flex-wrap、居中；列 width 600；卡片 min/max-width 600。`div.flex>div{flex:auto}` 让宽屏列分担剩余空间，600px 卡在列中居中 | `surface::page_columns` 保留 600px 卡和 20px 中缝；1280px 视口的 1240px 正文可并列两卡 |
| 三产品 `@media(max-width:1279px)` | `.widget-col{margin:0 30px}`；两列加侧边余量后不再能同行 | `DeviceColumns` 在断点下转纵向，保留固定 600px 卡和左右 30px 占位；修复此前 1279px 仍按 600+20+600 错误并列的问题 |
| 三产品正文默认 | `.body-wrapper{min-width:600px;padding:10px 20px 20px}` | 设备正文继续使用左右 20px；不套主前端窄屏 30px padding |
| 三产品 `@media screen and (max-width:1279px)` | `#body-wrapper.body-wrapper.scrollable{min-height:auto;overflow:auto}`；另有 noScrollX/custom-scrollable 分支 | 普通本地设备正文使用双轴滚动；固定产品图和卡片不被压缩；导航与本地保存栏保留在滚动区外 |
| 182 Customize 默认 / 653 产品覆盖 | 鼠标 config-wrapper min770、config-block770×340；653 覆盖 wrapper高385、block高387/宽100%，普通图宽830、白色830.8。653 `@max1250` 还显式允许 Customize 横向滚动 | 保留图层逻辑尺寸；正文最小内容宽覆盖实际图宽与两侧 padding，窄窗可滚动到完整设备图和下方映射操作 |
| 777 Sound/Mic 默认 | `.widget-prod{min-width:1024px;height:250px}`；`#eqBox` 固定940px；没有将 Mic 改成流体宽度的媒体覆盖 | Sound 头图和 Mic 卡保持源码最小宽，窄窗由正文横向滚动；600px Sound 左右卡仍按1279断点上下排列 |
| 主应用 `@media(max-width:1279px)` | `.body-wrapper{padding:10px 30px 20px}`；`.dashboard{max-width:910px;margin:inherit}` | 主应用采用其独立 gutter 和卡列计算，不复用设备正文的20px规则 |
| 主应用 `@media(max-width:600px)` | `.dashboard{max-width:290px;min-width:inherit}` | 普通 Dashboard 限为一张290px卡；仍保留原最小容器及滚动边界 |
| 主应用 Dashboard 条件 `.reflow` | `Li.render` 仅设备数量>4时加 reflow；其 max-width2460 的选择器优先级高于普通 `.dashboard` 窄屏覆盖 | 主应用按真实设备数量切换对应上限；不是所有宽屏都无条件扩到2460 |
| Devices & Modules | `.items/.item` width1220；本地6505 CSS没有折列媒体规则 | 固定1220行布局保留，窄窗横向滚动，不压扁名称/状态/操作列 |

主 Dashboard 的 `@max1260{width:min-content}` 还受 `Li.render` 的内联 `style:{width:"unset"}` 覆盖；不能只抄 CSS 后宣称主页面按 min-content 收缩。`Li.getMaxColumns` 根据实际容器宽度与290px卡/20px间距计算列数；本地对应的布局计算用于生产视图，并增加边界单元测试。

GPUI 侧将窗口宽度按当前 rem 相对16px的比例还原为来源 CSS 单位，再判断1279边界。因此字号125%时，组件尺寸和换列边界一起缩放；这仍是本项目字号比例适配，不等同于已验证全部操作系统 DPI。

正文使用 owner 内保留的 `ScrollHandle`、原生 `overflow_scroll` 和 GPUI Kit 滚动条。动态 `device-body-*` ID 指向真实视口，滚动内容单独保留自然高度和来源最小宽；窗口缩小时固定图和映射操作可达，窗口放大后恢复相应列布局，不重建 Profile/Slider/Select 状态。

[workspace_tests.rs](../../src/features/workspace_tests.rs) 保留生产视图用例，覆盖1279/1280往返、125%等比例窗口、菜单展开和领域值保留、700×400短窗双轴滚动及终点钳制，以及777 Mic在1100→700→1100时固定940px面板、滚动至16kHz/Reset、末频段实际拖动和所有频段实体保留；主页面另有生产布局函数的单元测试。[lighting_input.rs](../../src/features/lighting_input.rs) 包含草稿校验、向上量化与真实输入提交/键盘步进用例。本轮不执行测试或应用，当前静态检查状态统一记录在[重构状态](03-implementation-gap.md#5-验证状态)；不把历史编译结果或未执行用例当作通过。
