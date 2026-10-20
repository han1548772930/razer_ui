# 颜色选择与屏幕取色

普通产品颜色弹层和 Studio 颜色编辑器都让用户从色板、数值或屏幕采样选择颜色；它们对“取到颜色”和“提交颜色”的处理不同。取色工具本身没有直接完成设备灯光写入。

## 普通产品弹层

PID70 的完整颜色组件把选择保存在本地 `selectedColor`、输入字段和标记中。屏幕取色的 `Color` 事件调用 `onChangeHex` 更新这些预览；`Done` 回调只记录结束，不调用父级保存。只有 `saveColor` 才调用父组件传入的保存回调，`cancelColor` 调用父级取消回调。

因此，“屏幕取色完成”与“确认此灯效颜色”是两次动作。最终改变哪个配置档、灯效或设备字段，继续由挂载该弹层的父级保存函数决定。

普通包装器在调用原生 Show 以前设置 `isRegisterColorPicker`，回调先检查该标志。Done 先向上发布，再清除这个门禁。显式 Close 则先清除门禁再调用原生关闭。完整普通 picker 组件本身没有调用 Close；组件卸载会移除鼠标和 picker 事件监听。

## Studio 编辑器

Studio 的 `Color` 事件调用 `onChange`，同时更新本地 RGB／HSV、数值框和预览。`Done` 使用当前 ref 值调用 `onAfterChange`，携带 RGB 数据和数值颜色。

Studio 包装器的 Done 经过延后处理，先等待 `closeColorPicker`，之后才向 UI 发出结束事件。Show 以前设置 `isOnColorPicker`，Close 先清除；终止函数还处理现有区域窗口关闭、FFI 释放和初始化状态清空。普通弹层与 Studio 不能共用一个猜测的 Done 提交语义。

## 原生窗口到底怎样取色

两条当前原生实现都会创建 layered、tool、topmost 弹出窗口，并把虚拟桌面截取到兼容内存 DC；后续 `GetPixel` 采的是这张固定快照。鼠标移动并不重新抓取实时桌面。

最初的 `WM_START_COLOR_PICKER` 使用 `GetCursorPos` 的全局坐标直接取样，没有减去虚拟桌面的 left/top。后续鼠标移动和按下使用有符号 16 位 client 坐标。两种坐标来源是原版的实际差别，不能把它们写成已经统一的坐标算法。

原生 `COLORREF` 转 RGB24 时交换红蓝字节。自定义光标来自资源组 102，热点、DIB 像素和 AND mask 都是取色交互的一部分。最初的按下处理发布一次颜色并设置为非 topmost；鼠标移动和失焦处理又把窗口设为 topmost。

SysUtils 的 Done 先投递内部消息，再发布事件；Studio 的 Done 发布后依靠 JS Close、消息与线程 join 链结束。替换取色器会先销毁并等待前一个 owner。

## 原件与调用者

完整原件路径与已核对的 16 个完整所有者在 [取色捕获](../../../.analysis/results/host-current/local-audit.json) 的 `screen_color`。

| 功能 | 原件与字节区间 |
| --- | --- |
| 普通产品完整颜色弹层 | `mouse-70-current/.../main.8f24b6a1.js` `[5404008,5413927)` |
| 普通 Show／Close 和回调门禁 | 同原件 `[4797134,4797299)`、`[4797300,4797610)`、`[4816696,4816914)` |
| Studio 完整颜色编辑器 | `synapse-chroma-studio-current/.../606.b47ade77.chunk.js` `[861,9355)` |
| Studio 原生包装器 | `synapse-chroma-studio-current/.../main.6b22e9cc.js` `[233246,239514)` |
| 宿主声明与消息转发 | `.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/sysutil/win/index.js` `[243,7358)` |

各产品灯效弹层的父级保存消费者、不同版本与附件分支继续在对应灯光功能文档中核对；本页描述的是这两条已直接核实的 picker 行为。
