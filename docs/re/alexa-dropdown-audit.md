# Alexa 下拉框源码复核

2026-10-02。适用于 Alexa 设置中的输入设备和语言两个选择器；普通 Synapse 选择器仍保留独立的 `.s3-dropdown/.s3-options` 规则。

依据为 `.ref/applications/synapse/alexa/static/css/main.bbca16c4.css` 和 `.ref/applications/synapse/alexa/static/js/main.05f102d2.js`，仅静态读取。JS 的设置组件 `yE` 实际挂载 `nE`（`DropdownSelector`），其内部挂载 `$p`（`eE`）；两个调用均传 `position:"bottom left"`，未覆盖默认 `maxHeight:110`。

| 部分 | 原始规则与对应实现 |
| --- | --- |
| 触发框 | `.dropdown-selector>.box`：230×27，14px 字、26px 行高、左 padding 6、右侧留 30；父选择器 margin-bottom 10。外部仍可通过 Styled 覆盖尺寸和边距 |
| 触发边框 | #5d5d5d，active/hover 为 #44d62c；100ms ease-in-out。禁用透明度 0.3 |
| 箭头 | #999、10×5 实心三角形，右距 10；旋转 180°，100ms ease-in-out。复用同形状的 `synapse/expand.svg` |
| 菜单 | `.dropdown-razer-2`：黑底，#5d5d5d 边框，外部顶距 2、最大高度 110；`.show` 将上下边框从 0 变为 1 |
| 选项 | 14px 字、1.36 行高、4px×6px padding，计算行高 27.04；普通 #ccc、选中 #44d62c、禁用 #454545；hover 背景 #1a1a1a，100ms ease-in-out |
| 开启 | `$p` 的 `L` 先挂载 height 0，100ms timeout 后设置 `min(scrollHeight+2,maxHeight)`；再等 100ms 才启用溢出滚动。高度与上下边框使用 100ms ease-in-out |
| 关闭 | `$p` 的 `C` 立即设 height 0、隐藏溢出，100ms 后卸载；退出期间不产生新的选择提交 |

实现入口为 `surface::select_alexa`，与 `surface::select` 共用 `SelectState`、Base Select、List、选择事件和按值保留的键盘光标。Alexa 使用 Kit Popup 保存锚点和最终布局尺寸，独立保留退出展示；逻辑关闭时立即把焦点归还触发器，并在捕获阶段阻断退出内容的鼠标按下/抬起。普通 Synapse 继续使用既有 Popover 和 200ms Presence 展开，不改变其 180px 上限及 25px 行高。

Alexa 高度采用框架 Sequence 的单步目标切换；Sequence 保存旧步骤的 timing，在延迟或展开途中关闭时从当前高度继续，不会因临时切换 delay 而跳到完整高度。箭头、触发边框和选项 hover 使用框架 motion transition。减少动态效果时由框架直接采用最终值。

新增回归用例源码 `alexa_waits_before_reveal_and_keeps_closing_content_without_keeping_focus` 覆盖 230×27、2px 间距、前 100ms 高度为 0、200ms 达到 110、退出 100ms 后卸载、退出期间点击不提交也不夺回焦点。检查范围为源码复核和编译；本轮不运行应用、测试用例、原 JS 或设备 DLL，不把测试源码存在当作实测通过。
