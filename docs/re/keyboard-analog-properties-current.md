# 当前模拟键盘的 Windows 属性链

当前 614、642、678、679、688 的页面有实际 `deviceType:"analog"` JSX 消费，不是共享 bundle 中孤立的系统 wrapper。[AST、标签导出、CSS 与资源证据](keyboard-analog-properties-current-evidence.json)保留每个产品的属性组件、别名和实际调用点。614/642 在右列，678/679/688 在左列；其他产品不能仅因存在共享类就加入 analog 分支。

模拟分支标题为 `WINDOWS_ANALOG_PROPERTIES`，帮助为 `WINDOW_PROPERTIES_TOOLTIP`。第一行使用 Windows 图标和 `OPEN_KEYBOARD_PROPERTIES`；第二行距第一行 15px，使用当前 `controller_icon.b3be182d.svg` 与 `OPEN_GAMECONTROLLER_PROPERTIES`。两行图标均为 44×44，图标右侧间隔 20px，链接字体 14px、行高 44px、下划线，普通色 #ccc、hover #44d62c、active opacity .7。颜色映射到产品主题 token，尺寸使用现有 rem 转换，行为使用 GPUI Kit BaseButton 的键盘、焦点和点击机制。

第二行点击从当前组件 `OpenGameController()` 经 Electron wrapper 与当前 host `sysutil/win/index.js` 的 `void()` 声明进入 `SysUtilsNative.dll`。IDA 8.3 与 Hex-Rays 在私有副本静态恢复 RVA `0x3ffd0`：`WinExec("control joy.cpl",5)`。原 DLL SHA-256 为 `01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318`，[原函数字节、伪代码与交叉引用](evidence/sysutils-game-controller-ida.json)可校验，没有执行导出或应用。

Rust 的 `Properties::GameController` 通过独立 Windows adapter 使用原命令与 SW_SHOW 5，不加载原 DLL。WinExec 失败返回页面错误通知；源 host 的 void FFI 丢弃该返回，两者分别保留。其他平台明确返回 Windows only。两个属性入口均为用户点击后实际发起，不在 render 中执行。

`keyboard_properties.rs` 已补两行及原 analog 文案，`keyboard_products.rs` 按上述实际消费者和列位置接入。新的 controller SVG 来自 614 当前 asset-manifest 指向的官方资源，并由 hash、HTTP 收据及其他四个产品相同 manifest 资源名校验；未绘制替代图标。

维护工具 `tools/prepare-keyboard-analog-properties.cjs --check` 静态解析当前 JS/CSS、核对原 PE 函数字节哈希、校验 SVG 字节和输出收据。`cargo check --locked -p razer-pages --all-targets` 已通过。没有运行应用、测试、DLL 或系统属性命令；运行验收尚未执行。本项完成不等于五个产品的其余页和设备功能全部实现。
