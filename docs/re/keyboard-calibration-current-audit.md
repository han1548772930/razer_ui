# 磁轴键盘校准页（2026-10-03）

本次读取当前产品 740、746 的实际导航和挂载组件，补上此前只有标题的 `TAB_CALIBRATION` 页面。实现为 [keyboard_calibration.rs](../../src/features/keyboard_calibration.rs)，来源、字符偏移、CSS 和 20 份语言包指纹记录在 [keyboard_calibration_data.json](../../src/features/keyboard_calibration_data.json)。未读取停用的参考目录。

| 产品 | 当前页面源 | SHA-256 |
| --- | --- | --- |
| 740 Huntsman V3 HE Magnetic Mini 65% 8KHz | `9908.1f02bab3.chunk.js` | `90d8e757ed43ccecb5a19a1cd7dd89db75087f6af54e00e10515db192f71beda` |
| 746 Huntsman V3 HE Magnetic Tenkeyless 8KHz | `6498.86d76d1a.chunk.js` | `e8cf975f0d6737be756bac3860dc761684c4c43596c6530b6abcb579bc0a9c13` |

页面保留介绍横幅、只读键盘展示、左列校准卡片及提示。弹窗使用 GPUI Base Dialog 的焦点陷阱、Escape 和关闭回调；关闭、切换页面及恢复配置时释放弹窗。原始 `.choose-a-mat` 才是挂载容器：宽度上限 850，顶部距视口 100，延伸至底部。源码中另一套未挂载的 `.modal` 的 980 × 620 规则没有作为实现依据。几何通过 `surface::css` 随 rem 缩放。

成功、失败和提示 SVG 从当前 JSX 字面量静态提取，未用近似图标替换。按钮、正文、三步提示的 17 个语言键与两款产品的 10 种当前语言逐项相同。设置的开发区提供 8 种独立示例：等待按键、已选择、按到底、校验按下位置、松开、校验释放位置、成功、失败。等待状态只能手动切换示例，不自动产生设备成功回应。

根据用户对“只有弹层”的反馈，开发预览已改为独立的完整 SourceProductWorkspace，可切换 740 / 746，默认停留在 TAB_CALIBRATION，不自动弹窗。原版介绍、键盘图和校准卡与普通产品页复用；页面开始按钮和八个显式样例均打开同一个 CalibrationModal。取消、Escape 和成功完成均返回页面。测试源码随之更新，不再用单独绘制的弹层片段代替实际页面流程。

真实校准需要 `initCalibration → calibrateBottom → verifyBottom → calibrateTop → verifyTop`、`stopCalibration`、输入重定向和前台窗口监视。当前没有该传输，因此正式入口的“下一步”禁用并说明暂不可用；它不捕获系统键盘、不修改按键映射、不产生配置脏标记。示例状态不连接真实设备。

仍未完成：设备传输及事件时序、错误后 15 秒关闭、真实按键捕获、出厂配置禁用分支、介绍关闭状态的跨启动持久化、光标和载入动画。静态布局不代表实际窗口像素已验收，因此这两页仍为 `partial_native`。

可复核命令：`node tools/extract-keyboard-calibration.cjs --check`。两项新增 UI 交互检查源码覆盖打开/关闭、焦点恢复、rem 几何、配置不变及示例等待状态；仅经 `cargo check --locked --all-targets` 编译，没有运行测试、应用、构建、安装器或厂商代码。

## Native completion notes (2026-10-04)

The native page now mirrors the source's three remaining UI contracts without claiming a device response:

- `showNotificationBannerCalibration` is persisted in the local workspace directory, so closing the introduction banner hides it on the next start.
- The `isFactoryDefaultProfile` branch renders the audited factory-profile warning and removes the calibration start controls while that profile is active. Selecting another local profile updates the branch and dismisses any open modal when returning to the factory profile.
- The source's 15-second input-error idle watchdog is represented in the explicit failure preview. It closes that preview modal after 15 seconds; live calibration remains disabled because `initCalibration` and the input redirect transport are not available in this reconstruction.

No application, downloaded script, DLL, or build artifact was executed.
