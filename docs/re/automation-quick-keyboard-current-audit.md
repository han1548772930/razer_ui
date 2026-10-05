# 3946 快捷宏键盘捕获与按键显示

2026-10-05。重新沿当前 3946 manifest 定位 `aH`（6920385–6931761），核对真实挂载的键盘分支、`Te` 开始捕获回调、document keydown/keyup 效果、`tH` 类型图标、`eH` 删除图标与有序 CSS。没有执行下载 JavaScript。

修复此前可持续追加按键、缺少捕获会话的实现。仅空列表的 `+` 开始录入；显示源 28px `Start typing` 输入，首次按键后切换到 pill 并保持监听焦点。任一 keyup 结束录入；去重、最多十键。录入时 Escape 作为按键，非录入时关闭弹框。已有按键旁不添加源码没有的继续录入入口，清空或逐个删除后重新出现 `+`。

GPUI Windows 将 Ctrl、Alt、Shift、Windows 独立发为 `ModifiersChanged`，现按按下边沿收集标签、任一释放结束会话。Caps Lock 事件只提供锁定状态，无法还原物理释放；这项仍有差距，没有模拟按键事件。

按键已补 8×10px 内边距、13px 文本、4px 圆角、`+` 分隔符和源 `eH` 24px 删除叠层，采用源正常/悬停/按下颜色、200ms ease 边框与透明度过渡。宏类型已在当前选项与四个菜单项中显示同一 `tH` 原图标；初始 Action 显示 0.3 透明度键盘图标。删除 SVG 由维护提取器静态转换，纳入原有 automation 资源 manifest（24 项）。

类型选择器已移除框架 Select，改为当前源按钮菜单；行内边距、选中文字、箭头旋转与实际条件挂载见 [菜单审计](automation-quick-menu-current-audit.md)。Caps Lock、系统截获组合键、渲染像素、字体与实际焦点未运行验证。保存仍只产生本地宏候选，未增加执行器或伪造服务结果。

允许的静态验证：`node tools/audit-automation-quick-keyboard.cjs --check`、`node tools/extract-automation.cjs --check`、`python tools/validate-automation.py`、rustfmt；统一 `cargo check --locked --all-targets` 由主任务完成。没有运行应用、构建、测试、安装器、下载 JavaScript 或 DLL。

完整源码范围、挂载分支、CSS 与资源哈希见 [automation-quick-keyboard-current-evidence.json](automation-quick-keyboard-current-evidence.json)。
