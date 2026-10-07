# Studio 属性当前契约

当前实现和当前 `.ref/applications/synapse/chroma-studio/` 的差异以本文件为准。已修复条目保留证据和验证边界；只有源码、实现或相关行为再次变化时才重做核实。描述符、路由和默认字段不计为 UI 完成。

## 当前已挂载界面

`StudioProperties::render` 已挂载 Ambient、Static、Spectrum、Breathing、Fire、Reactive、Ripple、Starlight、Wave、Wheel、Tidal、Audio，共十二个属性根。Generate 尚未挂载。十二个根都有明确未验收边界，不能据此声称效果、Studio 或设备完整复刻。

| 区域 | 已接入的当前行为 | 仍需保留的限制 |
| --- | --- | --- |
| Ambient / Static | 屏幕预设与 Blur；单色编辑器 | 屏幕采样、自定义区域选择和原生取色尚需真实服务 |
| Spectrum | 专用渐变族和三档时长 | 混合值、服务预览及共享调色器差异未完成 |
| Breathing / Fire | 双色本地编辑；Breathing Random、时长和 Playback | FU 哨兵的异常 HEX/RGB 展示仍未对齐 |
| Reactive | 单色、Random、三档时长；保留黑色 0，不挂 Playback | 混合选择、服务预览及共享组件边界 |
| Ripple | 默认渐变族；Speed 1–50；Width 100–400、步长 100；Playback | 服务预览和完整窗口输入未验收 |
| Starlight | 默认渐变、Random；Density 1–10；时长；不挂 Playback | 随机仅禁用渐变，不代表真实光效已应用 |
| Wave | 渐变；Speed/Pause 同排；Width/Split 同排；Playback | 角度仍为旧数字/滑条表达，真实 80px 圆盘和联动待实现 |
| Wheel / Tidal | 各自颜色、Speed、方向组和 Playback | 中心点按钮、画布生产链与真实设备命中尚缺 |
| Audio | 专用渐变、Low/High；Boost/Auto 同排；Decay | 混合参数和实际引擎预览尚缺 |

## 已记录的修复

由 `review-studio-properties-current.cjs` 从当前清单重新读取，[当前收据](studio-properties-current-evidence.json)包含 11 项修复契约 AST、15 项剩余工作 AST、11 张 CSS 表和八份方向 SVG 原字节收据。

| 修复项 | 当前源码约束 | 原生结果 |
| --- | --- | --- |
| Audio Auto 排列 | 9120 将 Checkbox 作为 768 的 children；768 将 children 放入 input-group-stepper 右列；412 CSS 对齐底部、列间距 20px | Auto 位于 Boost 数字输入右侧；启用 Auto 只禁用 Boost 输入/滑条，Auto 自身仍可操作 |
| Audio 小数 | Boost 0.25–4/0.25，Decay 0.1–2/0.1；2245 在 step≥1 时才阻止小数点；7660 归一化后保留两位小数 | 小数输入、归一化和 f32 滑条同步保持独立，不将程序同步当成用户新修改 |
| Wave Pause | 1591:E 的 2245 数字框显示 pause/1000，提交 Number(value)×1000；它是 Speed 的右列 | UI 显示 0–60 秒；参数保存毫秒；没有 Pause 滑条或范围脚标 |
| Wave Split | 1591:E 的 Width 右列挂 5196 Switch，标签在上；32×18 轨道、14px 圆柄 | 用受控开关替换独立复选框，随 Width 输入同行；沿既有工作参数事件提交 |
| Wheel/Tidal 方向 | 8379:y / 8154:y 的组容器 onClick 翻转 params2.counterclockwise，两个子按钮没有独立赋值事件 | 点击任意半边，包括当前已选半边，均翻转一次；组边缘也使用同一动作；旧代际事件不能修改新属性 |
| 方向外观 | 30px 高、最小 104px 宽、3px 圆角、20px 源图标；选中底色与非选中按压状态独立 | Clockwise/Counterclockwise 与 Outward/Inward 使用当前 gray/black 八份 SVG，保留源标签与共享轮廓 |

Auto/Split 的显示状态由 `StudioProperties` 控制，`StudioNumeric` 只报告带代际的意图；数字、复选框/开关和右列输入没有另建持久化来源。Pause 的秒/毫秒转换发生在属性边界，不改变共享数字控件的显示单位。

新增 test-support 用例覆盖方向已选半边点击、Audio Auto 的右侧与底边布局、Auto 状态事件。只进行编译检查，不运行测试。维护工具的 `--check`、Studio 55 份原始 SVG 资源验证与定向 rustfmt 已通过；统一 `cargo check --locked --all-targets` 由主任务记录。未运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。

## 工作参数和保存边界

当前 `9286:HZ` 经 reducer 合并工作 `effectLayer.params`，移除被修改的 paramsMixed 键并设置 isActive。`XF` 合并独立 params2，方向和中心点使用该缓冲区。`9286:U` 才是应用到设备区域的动作。参数缓冲区、已应用区域和设备持久化不能互称。

Pen/Bucket 可以在没有设备选择时编辑工作参数；Select/Move 在当前无真实选择时禁用。切换图层替换工作参数；Pen↔Bucket 保留；涉及 Select/Move 的切换按源合并/重置。Reset 使用当前效果画笔默认值；中心点的完整 reset 尚须随中心点工作流核实。

源码连续 slider change 与 afterChange 的预览请求不同，且预览只在源指定工具条件下调用。当前本地事件和本地图层文档保存都不等同于引擎预览、真实区域应用或设备保存成功。只读观察/真实设备选择仍在当前范围，只有 DLL 修改与持久化腿后置。

## 剩余明确缺口

1. **Wave 圆盘。** 1591 的角度控件为 80px 圆盘和 0–359 数字框。角度以实际框中心两个轴各减 2px 后 atan2 求值，经 floor 取整，上方 0°、顺时针增加；指针按半径 clientWidth/2−10 并 round 定位。拖动、窗口级释放、非主键和重布局行为仍未接为该界面，不能把旧角度滑条标记为完成。
2. **Wheel/Tidal 中心点。** 属性按钮分别控制 editor.isCenterPointActive / isTidalCenterPointActive；画布按缩放/偏移换算坐标，只有范围内的有效点击才写 centerPointX/Y、centerPointX2/Y2 和 deviceWithCenterPoint。设备命中必须来自真实 device.items；没有命中为 null。按钮、坐标转换、绘制、方向图及 reset 是一条工作流，不能单独放置假成功按钮。
3. **Generate。** 当前媒体输入、预览、替换/删除、Generate 与前后配置导航均未挂载。文件选择接受 image/*、.mp4，拖放按 image/video MIME；新媒体清历史，Generate 才加入 mediaList 并提交配置。最多五份配置、满时丢最旧；文件身份为 btoa(encodeURIComponent(JSON.stringify(path)))。当前源生成随机配置，不能标为调用 AI 服务。此项保留在 UI 范围，本轮不扩页面。
4. **共享属性。** paramsMixed/真实能力过滤和设备选择、原生取色/屏幕区域、引擎预览与读取发布者未完整接通。FU=1677721600 的源异常 HEX/RGB 展示、HEX 大小写、非 portal 弹层边缘行为、快捷切换勾选动画、完整颜色过渡、原图光标和浏览器数字暂存/滚轮行为仍有差异。所有精确像素、缩放、辅助功能和完整窗口输入仍须在允许运行后验收。

## 保留的当前证据

- [全部属性源与 JSX 清单](chroma-studio-properties-source.json)：`prepare-chroma-studio-properties.cjs` 维护；十三个源根、数值范围和缓冲规则。
- [本轮契约及剩余工作源](studio-properties-current-evidence.json)：`review-studio-properties-current.cjs` 维护；替代带错误“根未挂载”状态的旧 pending 收据。
- [Reactive/Ripple/Starlight 独立收据](studio-reactive-ripple-starlight-current-evidence.json)：各懒加载根独立解析，保留 62 项 AST、CSS 与资源证据。
- [Studio 主源清单](chroma-studio-source.json)、[颜色源码](chroma-studio-color-source.json)及原始素材清单 `assets/synapse/chroma-studio-assets.json` 继续由现有维护工具核对。
- [Studio 界面与共享控件](studio-ui-current.md)记录颜色、渐变、四种已挂载时长控件、图层、宿主页签和本地保存的实际实现与边界。
