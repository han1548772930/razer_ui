# Automation 编辑器锚定删除确认

2026-10-04。当前 PID 3946 AutomationModal（6936081）通过 EH（6934329）把确认框追加到实际编辑器容器。
垃圾桶引用与 footer 的 `getBoundingClientRect()` 决定位置：框左边等于按钮左边，顶部为 footer.top−footer.height−12；窗口变化重测。

确认框宽 300px、自动高度且上限 130px，padding 20px、#111 背景、#FD4949 边框。较长翻译按源容器滚动。
静态 CSS 层叠说明：footer 由 `.modal-patch-notes .modal-footer` 决定 10px/30px 间距与 #555 边框；
确认按钮由 `.keymap-action.flex>div.thx-btn` 保持最小 100×27px，不能直接采用较低优先级注入规则的尺寸。

原生编辑器现已使用锚定弹层；添加模式禁用删除。Cancel 只关闭确认并返回垃圾桶焦点，全部编辑草稿保留；
Confirm 先关闭编辑器，再向真实规则列表发送 Delete(original.id)，一次决策只发送一次删除，不顺带保存草稿。
透明遮挡层截获底层点击；EH/ZL 没有 Escape、Enter 或外部点击关闭处理，原生不让这些操作误关父编辑器。

实际 vM 图标已按当前 AST 提取；其 viewBox 为 10×12、按钮传入 24×24，并使用 currentColor。
图标和按钮沿用 .2s ease；确认框自身源码没有入场/退场动画，因此不添加。

`node tools/audit-automation-delete.cjs --check` 只读比较当前挂载链、CSS、图标与实现证据。
未运行应用、构建、测试、下载 JavaScript 或 DLL；像素、焦点、滚动、窗口缩放行为仍未经运行验证。
完整收据见 [automation-delete-current-evidence.json](automation-delete-current-evidence.json)。
