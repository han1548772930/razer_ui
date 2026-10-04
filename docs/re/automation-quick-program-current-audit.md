# Automation 快捷宏 Program 选择器

2026-10-04。当前 PID 3946 `aH`（6920385–6931761）调用 `showFileOpenDialog("Open", false, '[".exe"]')`，
取消或空结果不改路径；Program 只读，整行与文件夹图标调用相同选择动作。Program/Website 分别保留草稿，切换不清空。

原生实现已补同一入口、单文件选择、`.exe` 校验、独立草稿和源 Clear 行为。Clear 删除两个字段并回到 Program；
改变宏类型清空动作，晚到的旧选择结果不能恢复已删除内容。原生选择器取消、错误或无效文件保留先前路径。
保存只向上层发送本地宏配置，取消整个编辑器不发送保存事件；任何路径均不执行。

GPUI 路径选择接口没有扩展名过滤参数，因此系统对话框可能显示其他文件类型，返回后只接受 `.exe`。
发生真实选择错误时显示提示，弹框从源 369px 增至 409px 容纳信息；普通状态保持 369px。
源文件夹资源逐字节复核；没有新增图标替代。

`node tools/audit-automation-quick-program.cjs --check` 静态解析当前源码并比较证据，不写文件。
未运行应用、构建、测试、下载 JavaScript 或 DLL；系统选择器、焦点和像素仍待运行验证。
证据见 [automation-quick-program-current-evidence.json](automation-quick-program-current-evidence.json)。
