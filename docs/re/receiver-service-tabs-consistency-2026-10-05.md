# 无线接收器与服务入口页签一致性复核

2026-10-05。按用户要求设立专门子任务，核验已存在界面的布局、样式、颜色、字体、图标及交互。此次完成投诉直接涉及的 179 主体和宿主页签图标；不将此结果扩展为全部产品页面已验收。未运行应用、构建、测试、安装器或参考 JavaScript。

## 179 的真实挂载与修正

当前 `.ref/devices/179/index.html` / manifest 声明 `main.4849f7ca.js`、`main.c778525e.css`。模块 9473 的 `mE` 默认根依次挂载产品图、左列 `UmaComponent`、右列 `AE/OE`，右列传入 `uppercase:false`。模块 3249 的 `Yh/t` 检查 Windows user agent，Windows 实际选中 `Ie/Te`。不是仅凭 bundle 中出现组件推断挂载。入口无 ProfileBar 的既有独立核验仍有效，见 [头部收据](receiver-profile-header-current-evidence.json)。

本地此前的 accessory 通用分支只有图与通栏单选项，遗漏左侧配对 widget、右侧状态示意，且标题、文本样式与布局均不符。现由 `SourceControls` 的 179 Customize 专用分支渲染：

| 项目 | 当前源码 | 本轮修正 |
| --- | --- | --- |
| 外层 | padding `10px 20px 20px`，body-widgets 最大 1240px、居中折行 | 移除通用 20px 顶边距及额外 gap |
| 产品图 | 250px 高，margin `10px auto`，最小 1024 / 最大 1220px | 使用同一原图与点阵资源，保持独立整行 |
| 两列 | 每列 600px；viewport ≤1279px 时每列左右 margin30 | 修复双列及窄窗口堆叠；不把面板拉成全宽 |
| Widget | `#111`、圆角5、padding `30px 40px`、上下margin10 | 左侧 HyperPolling 的右padding按源改30 |
| 标题 | RazerF5 16px、`#44d62c`、uppercase、下margin20 | 左 `HYPERPOLLING_WIRELESS`；右改为真实 `INDICATOR_LED_V2` |
| 配对入口 | 原44px图标、10px间距，文字14px、line-height44、下划线、hover绿、pressed opacity .7 | 恢复原入口；帮助按钮复用源14px/300ms tooltip控件 |
| 指示灯主体 | 520×227、纵向gap20；选项310px、图容器200×192 | 恢复图文横排；SVG自身保留243×187及源溢出尺寸 |
| 单选项 | 20px外圈 `#737373`、10px绿点、label14/20；过渡200ms ease | 使用Base Radio显式实现几何、颜色、opacity/scale；标签不转大写 |
| 三项行高 | 50 / 64 / 50px，间距10 | 按源固定；说明宽280、左margin30、12px、`#999`、opacity .7 |
| 模式示意 | `rE/sE/TE` 内联 SVG，带独立 SMIL 时间 | 静态序列化10个原始SVG层，native合成opacity；不是截图或新画的接收器 |

电池模式保留源绿色恒亮、红色从2.5s开始每0.3s循环、黄色从1.5s开始每1.3s循环（不将它“修正”为更常见的循环）。警告模式保留0.3s、0.3s、2s的事件链。切换模式、重挂 Customize 或恢复为不同模式时重启该时间轴。`reduce_motion` 是原生无动态偏好的适配，保留静态初帧。

配对入口从此前缺失的控件恢复为源码 `G/se` 局部modal，而不是误接全局多设备配对路由。modal黑色70%遮罩、850px宽、顶部100px、100vh高、36px标题栏、原20px关闭图标与20px SMIL式spinner均来自当前包；坐标加宿主42px TabUI偏移，遮罩不覆盖宿主页签。点击外部与Escape不主动关闭，源G仅提供关闭图标。源`se`挂载时进入`LOADING`并等待`DUALLINK_BIND_INFO`；未连接设备transport时仍停在该源状态，不虚构空设备响应、失败、配对成功或固件状态。此轮并未宣称真实扫描/解绑/固件升级已接通。

原始组件、106条相关CSS规则、SVG层和本地文件哈希见 [机器收据](receiver-current-evidence.json)。维护工具：`node tools/audit-receiver-current.cjs --check`。

## 服务入口与其他宿主页签图标

当前host `Tab.js` 的 `page-favicon-updated` 把真实页面favicon交给`TabUI.changeTabIcon`，后者取数组第一个URL作为背景图。默认值是当前host的`rzAppEngine.ico`，并非鼠标、模块logo或空槽。上述事件、consumer和CSS均有 [独立收据](host-tab-icons-current-evidence.json)。

原代码只特殊处理653、777、179和鼠标垫，其他PID一律显示鼠标。本轮改为从331个当前产品HTML各自`shortcut icon`生成映射，34种资源均已取得和校验；不按产品名字或粗分类猜测。产品相对图标162/3886单独保留作用域。

| 服务入口 | HTML实际图标 |
| --- | --- |
| 740 / 746 / 653 | KEYBOARD |
| 179 / 164 / 241 / 3946 | ACCESSORY |
| 769 Philips Hue | HUE |
| 784 Aether灯带 | IOT_STRIP |
| 778 主板 | ACCESSORY_MAINBOARD |
| 3871 / 3884 ARGB | ACCESSORY_ARGB_CONTROLLER |
| 3886 无线ARGB | 产品自己的 `./productCategoryIcon.svg` |
| 777 音频 | AUDIO |

已同步修正独立页签：Macro取其HTML的KEYBOARD；Alexa取ICO原63px帧；Armory、Profiles、Feedback、Chroma、Tour和主Dashboard各用当前HTML指定文件；迁移沿用已取得的真实favicon；Firmware Update的HTML `href="false"` 不提供图标，使用当前host默认图。默认ICO选20px原帧，与已有独立解码的tray原始RGBA逐字节相同。SVG原字节复制，ICO仅静态解码和PNG编码，见 [下载收据](app-favicons-current-fetch.json)、[ICO转换收据](host-tab-ico-current-conversion.json)。

最初下载遇到自动审批服务HTTP404故障；用户重新授权后由主线程成功完成全部静态下载。现无资源下载阻塞，不能把该中间故障当作最终缺失项。

维护工具：`python tools/audit-host-device-favicons.py --check`、`node tools/audit-host-tab-icons.cjs --check`、`python tools/prepare-receiver-tab-assets.py`。完整资源准备同样导入该准备模块，不会在下一轮恢复错误的5类清单。

## 验证与边界

已执行允许的`cargo check --locked --all-targets`、rustfmt、源码收据重查、资源hash/格式/注册校验、嵌入JSON解析及语言键核验。没有运行应用或测试，不能给出实际窗口绘制、DPI切换、字体栅格化、鼠标命中和硬件服务交互已验收的结论。全站其余服务页面主体尚未逐像素复核。

后续专门一致性任务顺序：先164/241底座主体及配对modal，再769 Hue、778/3871有线ARGB、3884/3886无线ARGB、784 Aether、3946自动化，然后740/746键盘校准与其他独立应用。逐项同时记录真实挂载、尺寸/间距/颜色/字体、交互门控、动画和剩余边界，不以“已有页面”或“能编译”替代一致性结论。
