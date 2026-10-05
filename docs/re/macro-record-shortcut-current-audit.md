# Macro 录制快捷键捕获（2026-10-05）

源码为当前 Macro `8190.61506f5b.chunk.js` 的 58190.Lr/Rr，SHA-256 `003b116007bb415462e5175c09fc84928d2a20c8d1a7c73113a432643d2cbe9f`；AST 收据见 `macro-record-options-current-evidence.json`。不使用旧宿主或旧 frontend。

## 已接入的本地行为

`record_shortcut.rs` 的 ShortcutUi 使用现有 `keyboard_windows::Capture::start_stream/take_all`，只处理当前 UI 线程、当前 HWND、当前聚焦快捷键字段。没有安装全局/低级键盘钩子，没有 Razer inputredirect、全局快捷键注册或录制服务调用。

- 使用 `record_options_data.json` 中 122 项的原始顺序，按 browser keyCode 找第一项；不会根据物理位置擅自区分源码 browser 路径不区分的左右 Ctrl/Alt。Rr 仅在 keydown 将 92 转为 91，该非对称行为保留。
- Lr 的 ignoreKeys 为 F12 和 Left GUI。它们不写入快捷键。
- 修饰键按第一次按下的顺序存储，重复 keydown 不新增项目。纯修饰键组合在所有修饰键释放后提交。
- 普通键在 keyup 提交；若当时有修饰键，锁住该组合直到全部修饰键释放，避免后续 keyup 覆盖它。独立普通键可继续替换前一次值。
- 清除只清空已选择值，保留当前监听会话；使用源码相同的 close-enclosed 图。`synapse/shortcuts-search-clear.svg` 与当前 Macro `icon_close_enclosed.6056b667.svg` 字节一致，SHA-256 `f254f96aad55e9b5c101cfcc1ab2c0874ef0ced6f8a45e601c8ad45d0482930f`。
- 窗口失活保留 listening 状态并卸载当前捕获；窗口恢复且原字段仍聚焦时重新捕获。失活时丢弃尚未完整的修饰键，避免其他窗口的释放事件丢失导致残留组合。
- 字段失焦、外部左键释放、菜单关闭、实体释放都会退出；队列溢出取消会话，保留此前完整值，不提交截断事件。
- 控件恢复源 100% 宽、margin-top 5px、padding 5px、min-height 27px、13.5px/17px 字体、200ms 边框变化；清除区域 25×25px，图 20px。未知系统键盘布局用当前 resolver 的 default 名字表；缺失名字安全回退 inputID，不虚构系统布局结果。

`keyboard.rs` 同时加入 `new_action_items` 的数据变更限制：record options 打开时不新增事件；Sequence/Phased 不新增 Delay。此限制与父线程的界面禁用状态配套。

## 边界

这里只设置录制快捷键的本地偏好，不开始录制，也不宣称快捷键已注册到系统或设备。非 Windows 无原始消息捕获实现。Source 的 window click 由原生控件外部 mouse-up 实现；窗口失活时主动丢弃未完整组合是避免挂起键态的本地处理。原生焦点和浏览器事件传播并未通过运行画面验证。

静态读源、格式化和 diff 检查已完成。最终 `cargo check --locked --all-targets` 由父线程汇总，未运行应用或测试。
