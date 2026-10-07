# Raptor 当前页面契约

3858 与 3880 的 Gaming、Color、Display 按各自当前挂载链核对；[源码收据](monitor-pages-review-current-evidence.json)记录 124 条关系、CSS 及四张来源图标。六页均有部分原生内容，Lighting/Help 仍需独立复查。

| 页面 | 当前实现 | 未完成 |
| --- | --- | --- |
| Gaming | 六预设、本地编辑转 Custom、亮度/对比度；3880 独立拥有 gamut 和扩展 Gamma 条件 | 整体 featureDisabled 容器透明度、真实预设/设置与 MW 限制发布 |
| 3858 Color | 预设与仅 Custom 显示的 RGB 编辑 | 真实同步/可见性刷新、父与子控件禁用层级 |
| 3880 Color | 真实多选项时可选择 Color Profile，按源路径包含匹配；颜色管理链接有键盘操作及 restriction | THX/HDR 完整观察、原下拉定位/上翻/提示与禁用外观 |
| Display | 四张原来源图标、68px 等宽按钮、PIP/PBP 和位置/尺寸；Adaptive Sync 关闭变暗，FPS 不误用 refreshRate 限制 | loading backdrop/spinner、document 范围外点取消、完整焦点与左列层级 |
| 3880 Refresh Rate | 无 restriction 时显示胶囊/Windows 链接；没有真实选项时不以 60/120/144/165 初值假装设备支持 | supportedRefreshRate 更新与完整状态发布 |

`set_monitor_runtime`接收完整快照；缺失或无效字段清除相应观察及旧 restriction。观察状态不进入本地 profile；恢复/Discard 不能解除运行限制。`pending_color_profile`和`pending_refresh_rate`仅表示未发送的选择意图，下一份完整观察清除它们，不构造设备成功。

真实发布者尚未接入，未来接线须区分完整快照与部分事件、连接代际和事件顺序。系统命令失败在页面显示，未在开发验证中启动系统设置。维护工具 `review-monitor-pages.cjs --check`只做源/资源核对；回归源码只编译，窗口与硬件未运行验收。

## 参数、布局与本地操作

共享提取依据为 [accessory-system-source.json](accessory-system-source.json)、[准备收据](accessory-system-native-audit.json)和 [widget 布局](monitor-widget-layout-current-evidence.json)；当前 Rust 为 [accessory_system_products.rs](../../src/features/accessory_system_products.rs)。这些收据保存实际源片段与 hash，不能用旧过程文档中“尚未接入”替代当前代码状态。

Gaming 显示所选预设，首次手工更改先复制到 customData 并选择 Custom。Overdrive 为 Off0/Weak1/Strong2；3858 Gamma 为 1.4/1.8/2.2，3880 额外 2.4 及 Native/Rec.709/DCI-P3。非 Native gamut 禁用 contrast/gamma；3880 Custom 非 Native 还限制 Color。PIP 启用限制 HDR/Adaptive Sync；THX Cinema 限制 Gaming/Color，sRGB 限制 THX。Windows HDR、重复显示和真实 uiRestraint 不能从这些本地请求反推。

Color 温度预设按源 Normal5、LowBlueLight12、Warm4、Cool8、sRGB1、Custom11；仅 Custom 显示 RGB0–100 输入，源没有这三行的端点标签或独立 tooltip。3880 两列为左 THX Cinema/Color Profile、右 HDR/Color Temperature，标题开关紧跟标题，帮助在右上；有禁用原因时位于组件体首行，使用源 14px 感叹图。widgetContent 的 20px 间距仅在实际源有该容器的组件上应用，不推广到其他配件。

Color Profile 选项来自实际 colorProfiles/selectedColorProfile；没有真实选项时禁用空下拉。Refresh Rate 来自 supportedRefreshRate/selectedRefreshRate，空观察不得显示 seed 为已支持值。两者的选择是未发送意图；收到新完整快照即清除 pending。颜色管理和显示设置链接调用对应系统入口并显示错误，不宣称设备设置成功。

主输入源为 Auto0/HDMI17/DP15/USB-C19，四张原图标按等宽 68px 按钮挂载；改变来源按 ask_again 先确认，取消不改变本地值。确认框的“Don't ask me again”只影响本地确认选择；document 级外点范围与完整焦点仍列为未完成，不以局部点击处理宣称全局等价。

PIP 二级输入仅 DP15/HDMI17/USB-C19，不能选 Auto。模式屏幕 330×196、主区 186 高、底脚 130×30；尺寸 Small1/Medium2/Large3、角位 0–3，共 12 个矩形，按源 z 序选择，hover 隐藏来源文字。右侧 MODE 按钮 144×80 与 290px 列保留固定几何，不能为了整齐消除源溢出；GPUI/content-box 差异仍需像素验收。

FPS Counter 使用独立位置枚举 1–4，关闭时 body opacity0.3 且禁止交互，不能误用 PIP 角位或 refreshRate 限制。HDR 提示按 Windows11 与其他系统分别选源键。帮助与 slider 采用共享源控件，完整父子禁用透明度、loading、popup 定位/上翻和实窗动画仍未完成。

Hanbo/PWM/Cooling Pad 的当前内容与缺口统一见 [cooling-ui-current.md](cooling-ui-current.md)。设备服务写回后置不等于省略当前本地编辑、Apply/Save 和取消流程。
