# 691 当前页面专项核验（2026-10-05，未完成）

范围为 BlackWidow V4 Pro 75% 的当前 manifest、实际懒加载根、OLED 与 Power 中已存在的组件。只做静态解析，不运行应用、构建、测试或供应商代码。

维护工具 `tools/audit-keyboard-691-current.cjs` 生成 `keyboard-691-current-evidence.json`，`--check` 检查收据；`--assets` / `--assets-check` 提取/核验四个内联 SVG 和原始键盘 PNG，对应 `keyboard-691-assets-current-evidence.json`。工具依赖现有 `.work/audit-js/node_modules`。

## 源码链和外层

当前入口 `main.e51ce83a.js` 通过 Promise.all 加载 4141/1068/5171/8987/7668，挂载 82189。miniCssF 将基础 CSS 映射为 `5171.1330bdc6.chunk.css`，导航 CSS 为 7668。Customize 另加载 2535（其中 config-wrapper 高度覆盖为 385px），OLED 加载 7077，Power 加载 3730。专项收据保存入口、CSS 映射、页面根和选择器的哈希与偏移。

基础 `body,html` 为 Roboto 16px、#ccc、#222，body-wrapper 为 10px 20px 20px。本地删除了 691 的旧 p20 特例，使用已核验公共外层。

82189.Ca 在 OLED/Power/Help 将 isEnableProfileBar 设为 false；55714.Kt 的 renderProfileBarIcon 只据此显示 loader disable，不能据此隐藏 profile bar。Dropdown 是否禁用由 enableSwitchProfile 另行控制。父线程据此更新共享 profile_bar.rs。

## 本轮修复

- OLED 首页按源码显示 animation/image/emote/banner/media/system/keyboard 七张卡片。恢复 236px 卡片、19px 标题区、64px 内容、选中边框、悬停操作层；键盘预览用源码内联 PNG。删除原有额外的模式选项按钮、530×120 假预览框和产品内部实现说明。
- 保留真实 PresetEditor、CropDraft、OledMediaEditor，动画/图片导入、裁剪、Apply 与媒体编辑入口保持可用。旧 Emote 弹窗只有一段服务不可用说明；旧 Banner 弹窗只有传输说明，Top/Bottom 按钮没有处理器。它们不是已实现的编辑器，本轮移除假弹窗并将对应未实现 edit 置灰。
- 首页大屏宽 1220px、小屏宽 600px，小屏左右内距 25px。下面为两个 600px 固定列，widget 自带上下 10px，取消额外的 20px 列内间隙。
- 标题使用 RazerF5/16px，正文继承 Roboto/14px/#ccc；标题开关不再重复绘制标题文字。六个 widget 恢复右上角 14px 帮助入口与源码 tooltip 文案。
- 亮度恢复 64px 容器、6px 轨道、16px 手柄、数值提示及 20/100 端点。语言保留选择后 Apply 的暂存流程、补码字节处理与 BLE 禁用。Home/Dim 使用原始数字按钮，去除通用按钮外观。
- 屏保恢复两列 260×68px、10px 间隙、原始图片 256×64px、无屏保的括号文字、源选中/hover 边框。
- Power 已存在的 dim/sleep 两个 widget 恢复标题旁开关、DIM_LIGHTING_HEADER、帮助和 48×27px 原始数字按钮，不再为每项添加 min。

2026-10-05 续接已补预设编辑器卡内操作、当前标题与说明、主页标题 hover，以及 requires-Synapse/BLE 提示文案，见 [预设卡片续接](oled-preset-cards-current-audit.md)。提示 portal 的时序/定位和实际画面仍未验证。

## 仍需完成

- Emote/Banner/System 真实预览和编辑器；目前保留七卡框架，但这三张内容为空、edit 禁用，是明确的源码差异。不能将此状态算作页面一致。
- Power 另外三个 source widget：低电量 OLED 提醒、indicator、低功耗模式信息。
- OLED 系统幻灯片选择时的特殊 dispatch、服务下载/进度/loading、提示 portal 精确位置与时序、原始滑条释放提交和 hover transition。现有持久化不等于硬件已写入。
- Customize、Lighting、Help 的内部结构与专用控件，其他键盘产品页面。
- CSS normal 行高、原生字体 fallback、浏览器和 GPUI 栅格化/换行；任务禁止运行，因此未做截图一致性声明。

当前只证明收据列出的组件与修复；未宣称整页或所有已有页面一致。最终 cargo check 由父线程统一执行。
