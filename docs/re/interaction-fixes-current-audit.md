# 性能页崩溃与 Gamer Room 悬停修正

核验日期：2026-10-03。只做源码解析、格式化和全目标编译；没有运行应用或交互测试。

## 按钮重复 hover

本机当前依赖 `gpui-component 0.7.0/src/button/button.rs` 的 `RenderOnce` 会为可交互且未选中的按钮设置 `hover` 和 `active`。`gpui-pre 0.3.7/src/elements/div.rs` 的同名样式只能设置一次，再次设置触发 `hover style already set` 断言。`cargo check` 不会执行渲染，因此不能单独排除这类崩溃。

当前编译模块中确认的冲突包括：

- 鼠标 Performance 的回报率按钮和 Windows 属性入口。
- Help 的复制序列号／恢复出厂按钮。
- Snap Tap 添加键对按钮。
- 灯光颜色选择器触发器、普通与空白自定义色块。

这些需要自定义边框、透明度或阴影的控件改用 `gpui_kit::base::Button`；Kit 继续负责点击、焦点和键盘激活，原页面提供外观。保留尺寸、选中边框、禁用行为及可访问名称，禁用控件不添加悬停样式。没有通过去掉框架断言或禁用调试断言掩盖问题。

补充的[回报率回归源码](../../crates/razer-pages/src/features/sensitivity_tests.rs)渲染 Performance，切换三个普通回报率并反复渲染，覆盖曾经触发断言的未选中按钮。现有 Help、灯光及键对交互源码一并纳入全目标编译。尚未实际执行这些用例。

## Gamer Room 产品弹层

当前源：[9388.974b5d43.chunk.js](../../.ref/applications/synapse/dashboard/static/js/9388.974b5d43.chunk.js)，SHA-256 `3f13c14fe1b040b3747825c7c308198fcc4a78f7a0aab11d7924fa79a000474a`。UTF-8 解码后零基字符偏移：

| 位置 | 实际代码 |
| --- | --- |
| 3237 | `handleMouseEnter=e=>...setState({hoveredItem:e})`，热点进入后选择产品 |
| 3315 | `handleMouseLeave=()=>...setState({hoveredItem:null})` |
| 5040 | `onMouseLeave:this.handleMouseLeave` 挂在产品弹层的父节点；横幅本身没有移出即关闭监听 |

本地此前把关闭监听同时挂在横幅和产品弹层上。Kit `Popup` 在延后绘制层调用 `.occlude()`，显示弹层后底下的横幅不再命中，从而触发横幅的移出监听，关闭弹层后又露出热点。这与用户报告的反复闪烁相符。

[service_pages.rs](../../crates/razer-app-pages/src/service_pages.rs) 已移除额外的横幅关闭监听，仅由产品弹层移出关闭；切换产品时旧弹层的移出通知不能清掉新选中的产品。热点重复进入同一产品不再无条件通知重绘。添加设备模态打开时仍清理后台产品弹层。

[Gamer Room 回归源码](../../crates/razer-app-pages/src/gamer_room_tutorial_tests.rs)对三个热点分别反复在同一位置命中，检查弹层不消失，移动到弹层内保持显示、移出后关闭。该用例只编译，尚不能代替实窗验证。

## 已有滚动条修正的收尾

[scroll.rs](../../crates/razer-widgets/src/scroll.rs) 已使用 `Scrollbar::new(&scroll)` 的固定 `ScrollHandle` 视口，不使用把滚动内容子布局当视口的 `viewport_from_layout()`。本轮保留这项已落地修正，格式化并重新编译[滚动回归源码](../../crates/razer-widgets/src/scroll_tests.rs)与 Dashboard 网格回归源码；覆盖滚动后的轨道点击、拖动、固定导航／底部边界。未运行窗口，不能声称这些坐标已通过实际交互验收。
