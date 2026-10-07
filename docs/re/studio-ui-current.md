# Studio 当前界面与本地保存契约

当前来源是 `.ref/applications/synapse/chroma-studio/`，独立于 Chroma Dashboard 和产品的 `displayMode=chromaApp`。入口 `xt` 挂载 4264 EditorCanvas，`nt` 定义 38px 工具栏与 48px 导航；本地 editor 负责正文，Shell 负责宿主边框。源清单与 AST 范围见[Studio 主收据](chroma-studio-source.json)。

## 宿主、入口与保存

当前宿主把 Studio 注册为 `chroma-app` 内的 policy 3 子页签 `chroma-studio`。Chroma 父窗为 1280×720、最小 600×500，TabUI 使用 42px 几何；Dashboard 与 Studio 保留独立根。App Picker、快捷入口、Chroma 模块行及 Nommo 的 `AudioStudioRequested → OpenStudio` 打开真实本地 Studio，不以 Dashboard 代替，也不更改外部安装或账户状态。[窗口收据](chroma-studio-window-source.json)记录宿主注册、重命名限制、favicon 与 TabUI CSS。

`AppShell` 持有 `StudioSession`。重复打开聚焦 Chroma 宿主，关 Studio Tab 返回 Dashboard，关窗后仍保留未保存内存草稿。宿主重建时刷新 handle 和 subscription，切换根分别恢复其焦点；Settings/Tour 激活所属主窗口。源 favicon 与当前 Chroma 原图字节一致，不能用图标一致代替窗口行为验收。

Save 写入工作区存储目录的 `chroma-studio-draft.json`，为本地版本化格式。读取拒绝未知字段/版本、未知效果、重复 ID 和不能表示的参数；无效文件报错且不以空文档覆盖。写入先核实观察到的磁盘字节，暂存并 sync，再备份旧字节和 rename；外部变更阻止写入。只把成功写入的捕获快照标记 saved，期间的新编辑仍 dirty。

写入进行时 Save 仍可提交最新快照，包括 Undo 回到先前 saved 的情况；后来的显式保存排在当前任务后，失败保留错误。Shell Exit 等待已请求的 Studio/workspace 保存，Studio 保存失败取消退出；没有提交的图层编辑不自动 Save。文件保存不调用硬件服务，也不声明厂商格式兼容。

## 图层、工具与画布

左侧图层栏 250px，效果库按源顺序、76×72px tile；可选图层、空组、复制、删除、可见性、重命名和本地 Undo/Redo 已挂载。Rename 按 32 UTF-16 单元限制，Enter/blur 提交非空 trim 结果，Escape 取消，同种类重名不提交。

| 操作 | 当前规则 |
| --- | --- |
| 默认与复制标题 | 源 locale key 可重复；用户字面标题保持字面值；同类重名寻找首个 `(1)/(2)…`，替换已有数字后缀 |
| 新空组 | 使用最小空闲本地化组号，包含删除/重命名留下的缺口 |
| Change Effect | 排除当前效果及无实际支持设备的 Reactive/Ripple；保留 ID/visibility，替换效果名称、标题、数值与默认参数 |
| 删除 | 不能删除最后一个效果；删当前效果后选第一个剩余效果 |
| Undo/Redo | 若当前 ID 仍存在则保留，否则选第一个效果 |
| 可见性/复制 | 选择第一个可见效果，使用源 first-item 后备；空组不是效果选择 |

图层细节源证据为[图层收据](chroma-studio-layers-source.json)。Change Effect 当前采用 GPUI submenu；源是在同一个 popup 内替换内容，这项布局差异仍存在。嵌套/有内容的组、组复制、拖排与嵌套可见性尚未实现，不能把平面图层操作视为全部组功能。

没有真实枚举的设备/LED 时画布为空，目录产品不变成已连接设备。Move 工具绘制当前 480×480 原 SVG 网格、145 个矩形、中心轴和 zoom≤50% 的细网格截止；网格解析见[网格收据](chroma-studio-grid-source.json)。Reactive/Ripple 添加和 Change Effect 的能力入口仍禁用，但这不等于其属性 renderer 未挂载。

S/P/B/M 工具、Ctrl+Z/Shift+Ctrl+Z、Ctrl+S/L/D/E、源 zoom 档位及预览/设备名偏好已接入本地状态。Ctrl+F 当前回到 100%；按选择内容 fit 仍需实际布局。设备几何、快速 LED 选择、画布平移/旋转/绘制、中心点、剪贴板、可缩放面板/紧凑断点、profile/add/import/export/link-games、帮助/教程及服务弹层仍有缺口。

## 属性根与工作参数

Ambient、Static、Spectrum、Breathing、Fire、Reactive、Ripple、Starlight、Wave、Wheel、Tidal、Audio 十二个属性根已挂载，Generate 未挂载。逐根范围、修复和未完成的角度圆盘/中心点/Generate 以[属性当前契约](studio-properties-current.md)为准。

`effectLayer.params`、独立 params2、已应用设备区域和设备持久化是不同状态。Pen/Bucket 可编辑无设备选择时的工作参数；Select/Move 没有实际选择时禁用。连续预览与提交保持各自边界；参数修改本身不把设备空文档标记 dirty。图层文档的本地 Save 不等于参数已应用到设备或引擎预览成功。

## 共享颜色控件

`StudioColor` 持有四个输入、亮度滑条、色域拖动与订阅。owner `set_value` 不发 Changed，禁用时丢弃未提交输入；输入完成、色块选择、指针/滑条 release 才通知工作参数。Static 的色域为 228×136，dropdown 为 208px，后者 HEX/RGB 宽 65/40px、色块内距 3.5px。

色域使用源水平 RGB hue 渐变与纵向白色覆盖，亮度独立；HSV 转换与 191/255 阈值包含灰色特例。无色为 None。HEX 1–3 位重复到六位、4–5 位补 f、空值为黑；RGB 去非数字和前导零并限 255。聚焦选中文本，blur/Enter/Escape 提交，输入键事件阻止 Studio 工具快捷键。当前 HEX 字段输出小写，FU 哨兵异常 HEX/RGB 等显示仍有源差异，详见属性契约。

八个源色块后最多八个会话自定义色，允许重复；只有自定义色有删除菜单，以稳定 ID 防止旧菜单删错项目。原用户存储桥 4809/8531 未接，自定义色只在当前应用会话保留。原生 eyedropper 因 showColorPicker/采样完成服务缺失而禁用，不合成取色成功。源颜色和资源见[颜色收据](chroma-studio-color-source.json)与[共享属性收据](chroma-studio-properties-source.json)。

## 时长控件

共享 `StudioDuration` 当前用于 Spectrum、Breathing、Reactive、Starlight，毫秒值分别取各效果根，不能把 Spectrum 数组推广到其他效果。Spectrum 为 75480/37740/18870ms。滑条拖动只更新临时位置，release、键盘和辅助功能提交映射值到工作参数。

五张当前 SVG 提供普通慢/中/快与 Reactive 中/快预览。预览宽 218px 平铺、高 100px、纵向偏移 13px，保留 SVG 230:80 比例；每 requestAnimationFrame 增加 1.5/1.75/2 CSS 像素，不能改称按秒速度。hover 预览/滑条或拖动时播放，停止保留相位；窗口失活清交互，跨宿主重建绑定。三档 Slider 保留键盘方向、Home/End、禁用值恢复与焦点入口。

## 渐变控件

共享 `StudioGradient` 使用 Spectrum/Audio/Default 三个源族，实际效果选择和限制由当前数据表驱动。Spectrum 有五组预设与独立自定义缓存，至少 2 个、最多 10 个 stop；其他族不套用它的范围。触发框打开宽 230px、padding 10px 的弹层，条 180×16px、颜色编辑区 208px，预设 25×16px。

stop 可选择、增删、拖动和改色。180px 原画布按像素中心取样，位置四舍五入两位；相邻间隔为 `ceil(6/(180−6)×100)/100`，显示使用源 7px 防重叠顺移。稳定 ID 保持回调身份；键盘方向/Home/End/Delete 使用同一边界。连续改色只更新临时草稿，release/输入完成/预设/增删才发布 colorStops/colorStopsCustom；百分比整数序列化，无色省略 Color。

弹层先零高度挂载，100ms 后展开；关闭立即归零，100ms 后卸载。失焦保留弹层并清拖动，重开选择首个 stop；重建宿主保留自定义缓存。缓存不声称为已保存用户配置。Base Popover 的焦点、Escape、外点关闭及窗口边缘避让与源非 portal 定位仍有差异；触发框 100ms 边色过渡、原 SVG 自定义光标、Canvas2D 像素量化和缩放还未完成验证。sRGB 预乘 alpha 显示/取样不等于浏览器像素已经一致。

## 验证与真实边界

维护入口为 `prepare-chroma-studio.cjs`、`prepare-chroma-studio-properties.cjs`、`prepare-chroma-studio-color.cjs`、`prepare-chroma-studio-layers.cjs`、`validate-chroma-studio.py`、`validate-chroma-studio-layers.py`、`audit-chroma-studio-window.py`，属性专项另有 `review-studio-properties-current.cjs`。工具只静态解析当前源码与资源；55 项原 SVG 及各自 manifest/source/output 收据继续保留。

真实设备选择、paramsMixed、屏幕区域/采样、原生取色、引擎/帧传输和读状态发布者仍未完整接通；DLL 写回后置。字体 normal/fallback、所有窗口输入、动画和像素没有运行验收。允许格式化与 `cargo check --locked --all-targets`，未运行应用、构建、测试、安装器、下载 JavaScript 或 DLL。
