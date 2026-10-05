# 已有页面全面一致性复核

2026-10-05。用户要求核对已有页面的全部内容，包括字体、布局、颜色、尺寸、图标、状态、交互和动画，不限于页签。主线程与三个子任务分工检查，并修正有当前源码依据的差异。此文是本轮覆盖索引，不是全部产品验收结论。

使用当前 Dashboard、Host 4.0.827 和各产品自己的当前 manifest/JS/CSS。未读取已禁止的历史源码目录；不执行应用、测试、构建、下载的 JavaScript 或 DLL。

| 范围 | 已确认的差异或核验结果 | 本轮状态 |
| --- | --- | --- |
| 179 无线接收器 | 旧通用页遗漏左列配对入口、右列指示灯布局、原 SVG 动画及局部配对 modal；标题、说明字号和间距不符 | 已按实际挂载组件修正；见[专项记录](receiver-service-tabs-consistency-2026-10-05.md) |
| 鼠标、键盘、音频、手柄、系统、配件的公共正文外层 | 多处顶部 padding 20px，应为 10px；部分页面多出源码没有的产品名、说明行或额外 gap | 已核 278 个公共 body 合同并修正适用外层；公共外层修正不等于逐页内容已验证，见[产品专项](product-content-current-audit.md) |
| Nommo 1303 / 1304 灯光 | 多出未挂载的空闲选项；亮度标题旁开关与 slider 使用了通用控件布局 | 已修正实际挂载的左列、滑条外观及释放时提交规则 |
| Kiyo 3592 / 3594 / 3595 / 3596 | 预览错误嵌入设置列，外层多余 padding，设置列宽度不符 | 已恢复 400px 设置列与独立视频区域；真实相机 transport 未接入 |
| Devices & Modules | 错误/移除状态字号 16px，应为 14px；离线按钮使用了定位、时序不同的通用提示 | 壳层子任务修正字号与当前源码专属提示；见[静态收据](shell-content-consistency-current-evidence.json) |
| 全局字体资源 | 缺 Roboto Light、RazerF5 Thin/Bold；部分字体内部族名/字重与 CSS 别名不符 | 已注册 7 个实际字体面，归一 CSS 族名/字重并逐表证明字形与字宽未改；见[字体与壳层专项](shell-content-consistency-2026-10-05.md) |
| Alexa | 正文后段 CSS 实际覆盖为 Roboto 16px；Skills 列表缩进多了 14px；Settings tooltip 定位和时序不符 | 已按实际根挂载修正；见[独立应用逐页清单](independent-app-content-consistency-2026-10-05.md) |
| Armory | 默认无 feature 观测时，正文横幅和宿主标题使用了 Exchange 分支 | 已统一按当前 hook 初值 false 使用 Workshop；不编造 feature 返回值 |
| 宿主页签图标 | 产品按少量类别猜图标；独立应用只读 HTML，漏掉 Macro / Armory 运行时覆盖 | 已接 331 个产品 HTML 映射，8 个独立应用运行时选择收据；见[运行时证据](runtime-tab-icons-current-evidence.json) |

页面可达、JSON 完整、资源存在和编译通过分别只能证明对应事项，均不能证明布局或功能一致。公共 CSS 覆盖数量也不能当作已逐页审查的数量。

仍需逐项复核：原有十个适配器的页面正文、各产品专用页与条件分支、691 的 lazy CSS 完整级联、配件服务页、各类高级映射、完整弹层/拖拽/键盘与焦点规则，以及 Profiles、Feedback、Chroma、Macro 等独立应用的全部正文状态。各子任务的最终收据应说明本轮具体覆盖项和未覆盖项，不能把概览浏览记为通过。

CSS `normal` 行高、系统字体回退、DPI 下的字体栅格化、鼠标命中、真实滚动、动画绘制和硬件服务行为仍无运行验收结论。本轮遵守禁止运行应用的限制，只执行源码静态解析、资源校验、格式化及允许的 `cargo check --locked --all-targets`。

父线程整合验证已通过：`cargo check --locked --all-targets`（仅保留既有 `layer_button` 未使用警告）、格式化与 scoped diff 检查；1167 项主资源和35项服务SVG校验；37份嵌入JSON检查0失败、4个已注明的结构推断跳过；447个硬语言键无缺失。产品、Alexa、Armory、壳层、服务行、接收器、图标运行时和共享电量提示的专项静态校验通过。没有运行应用或测试，也没有宣称所有页面一致。
