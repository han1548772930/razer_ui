# Macro Launch 弹层续接

2026-10-05。当前源码依据是 `.ref/applications/synapse/macro/` 中模块 **58190** 的 `rt/et/z/En/va/ha/ja`、模块 **37226** 的 `showFileOpenDialog`，以及 **13139/47990** 的 Launch 初始模板。用维护中的 Acorn/CSS 工具静态解析，收据见 [macro-launch-current-evidence.json](macro-launch-current-evidence.json)，没有执行参考 JavaScript。

## 已接入

- 新 Launch 动作使用 `RadioIndex:null`：程序和网站均未选择。原本默认选择程序的本地假设已删除；已有保存的 `program/website` 状态保持其含义，空状态对应 null。
- 程序行使用当前源的文件选择入口，替换可任意编辑的路径输入框。既有 GPUI 路径接口按 `Select Launch App`、单文件、无扩展名筛选请求选择；源码 wrapper 的第三参数默认 `[]`，本批没有借用 quick macro 的 `.exe` 限制。
- 使用首个返回路径作为 `Content0`。界面依源代码按反斜杠分割，仅显示文件名；保存的是完整路径。取消文件选择保留原 draft，不读取或执行选定程序。
- 选择器返回时检查请求代数、当前文档、动作索引、原动作快照和当前程序模式。关闭弹层、切换宏、删除/重排动作或切换模式后，旧结果不会写入新草稿。这是本地异步适配的防护，不是对源代码竞态行为的宣称。
- 网站是 `Content1` 的原样文字输入，没有添加源代码不存在的 URL 校验，也没有在确认时打开网址。
- Save 逻辑分别对应 `rt` 的两条 effect：内容变化按当前模式的非空内容计算；模式变化的后置 effect 则启用任意数值 RadioIndex，即使内容仍为空。重选同一模式不重新触发 effect。Save/close 禁用按钮，Cancel 还原原字段，并仅对确实变化的依赖重新计算按钮状态。
- 弹层 Save 把两项内容和模式合并为一次 undo 变更；Macro 外层 Save 才保存共享文档。外点、Escape、切换编辑器/文档与取消不提交 Launch 草稿。移除通用 Popover 后，关闭按钮/保存显式归还页面焦点。
- 按 `rt.X` 恢复 `launch: Program "文件名"` / `launch: Website "原文"`，空模式显示当前语言的 placeholder；删除旧的重复动作标题。

## 样式和定位

容器沿用当前 CustomModal 的 250px、padding20、1px灰边、5px圆角、20px阴影；两个按钮共用实际 `.keymap-action.flex > div.thx-btn` 规则，最小宽100、高27、灰/绿背景、300ms opacity。close 为36px点击区/20px图标和200ms背景过渡。

Radio 使用源20px灰色圆边及10px绿色点；点按200ms ease缩放/透明度过渡。label左内边距30、line-height17，容器上下margin10。程序显示字段142×29，网站164×27，网站下边距20。程序图标按源 right4/top5、20×16显示；禁用时使用真实 `icon_folder_g`，没有自行调色。

通过 `va -> ha -> ja` 确认 Standard/Sequence 的 `rt.W` 第四层祖先是相对定位的 `#item_editor`，第三层是42px动作行。弹层打开时按 `editor.clientHeight - row.offsetTop` 判断：**大于100**才沿触发器下方的静态位置显示，否则 `top:-300px`。这个判断故意不加 scrollTop，不能改为通用 viewport flip。原生实现保留同一方向和编辑器裁剪，随动作行滚动；再次点击 trigger 保持打开。

Phased 的源代码要向上六层，依赖尚未完整接入的阶段容器。本地当前动作列表保持42px平面行，本批没有将其声称为 Phased 完整布局。浏览器 file input 的内联基线、DOM 虚拟行卸载/复用、IME 与实际窗口像素仍未验收。

## 资源与验证

新增 `macro/launch-folder.svg` 与当前 `icon_folder.0ac33709.svg` 字节相同；禁用态复用同源 `macro/drag-folder.svg`。`tools/macro_assets.py` 维护41张 Macro SVG映射，主资源脚本和新增 `tools/prepare-macro-assets.py` 共用它。后者只用标准库复制 SVG 并更新对应的 manifest/embedded 区块，保留其他资源与顺序。

普通 Python 缺少 Pillow；现有 resource-env 的 Pillow 原生扩展加载失败。此次没有安装依赖；资源准备改用上述纯标准库入口并完成。最终资源检查为 **1,113 个主资源、35 个服务 SVG**，没有删除其他登记。

`cargo check --locked --all-targets`、格式化、当前 Macro Text/Launch/editor/Shortcuts 源收据检查、嵌入 JSON、语言键和资源静态校验均通过。只余原有 `customize_page::layer_button` 未使用警告。本批没有运行应用、测试、安装器、参考代码或任何选定程序，未实际打开文件选择器。原生宏执行、录制、设备服务和最终视觉验收仍未完成。
