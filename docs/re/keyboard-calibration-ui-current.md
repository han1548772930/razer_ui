# 键盘校准当前契约

740 与 746 使用各自当前校准 chunk。挂载、CSS、UTF-16 范围与原资源见 [弹层收据](keyboard-calibration-modal-current-evidence.json)及 [页面数据](../../crates/razer-pages/src/features/keyboard_calibration_data.json)；维护工具为 `extract-keyboard-calibration.cjs --check`、`audit-keyboard-calibration-modal.cjs --check`。

`keyboard_calibration.rs`保留介绍、键盘展示、校准卡与独立弹层。出厂配置按真实 Profile GUID 精确判断；该分支保留整个 body-widgets 并置为 disabled/opacity 0.3，另显示 465px、橙色边框警告，不通过配置名字猜测。介绍关闭状态保存在本地工作区。

开始按钮与说明保持源块间距。弹层最小宽 800，宽屏受 inline max-width 850 限制；顶部由视口底部至 100px，位置 300ms ease、遮罩 100ms linear。独立 36×36 close 使用 20px 原图；关闭直接卸载，不附加退场动画。成功/失败分支隐藏内容但保留布局，底栏独立锚定。

caret 使用 1.1 秒四段闪烁；加载条高 5、宽 25%，两秒 ease-in-out 从 left -25% 至 100%。`calibrateBottom` 等待与 `verifyBottom` 是不同状态，后者不会制造新的加载条。显式预览可以选择各步骤，失败预览有 15 秒关闭计时；预览不发送设备命令、不制造实际采样成功。

真实 `initCalibration → calibrateBottom → verifyBottom → calibrateTop → verifyTop`、stopCalibration、inputredirect、前台窗口监视及其错误 watchdog 尚未接入。正式未知状态不自动推进；只读/设备操作边界须分别接线。键盘图的完整外层几何、警告三点的负 margin 基线及步骤 dotted 栅格仍未运行验收。所有回归源码只做编译检查。
