# Studio Breathing / Fire 本地属性接入：2026-10-06

## 当前来源

只使用 `.ref/applications/synapse/chroma-studio/` 的当前 manifest 文件。Breathing 根为 2777，Fire 根为 5591；共用颜色下拉 16、调色器 1698、Checkbox 9741、Playback 5805、数字输入 2245、时长 1991、下拉生命周期 1592。完整源码、文件 SHA-256、UTF-16 范围及 CSS 见 [属性源码收据](chroma-studio-properties-source.json)。属性提取器现有 40 条模块/函数收据、19 份 CSS，并单独保留 tickTop/tickBottom 的原始 keyframes。

## 已接入

- `StudioProperties` 挂载 Breathing 的 COLOR、PROPERTIES、PLAYBACK 三段与 Fire 的热/冷双色段；帮助、文案使用当前语言表。Breathing 时长位置映射为 `[28000,14000,6000]` 毫秒，复用已核实的时长预览。
- 双色 trigger 为 53×27px、swatch 为 20×20px，两项间隔 10px；颜色弹层宽 230px、内边距 10px、内部调色区宽 208px，第二个弹层左移 63px 回到 `.colors` 原点。共用调色器已有 HEX/RGB、亮度、预设和会话自定义颜色的本地编辑。
- Breathing 随机颜色开关修改本地 `randomColor`，禁用两个颜色控件，色块以 100ms 变透明。Checkbox 使用 Base 的焦点、键盘和无障碍行为；方框 20px、边框与圆角、标签间距、勾线坐标和角度来自当前 CSS。tickTop 前 100ms 保持零，再用 100ms 展开；tickBottom 为 100ms，最终高度为 keyframes 的 9px，覆盖普通 checked 样式的 9.6px。
- Breathing `onAfterChange` 依源 truthiness 将无色和数值 0 都转换为 `FU=1677721600`；Fire 保留数值 0，无色移除对应字段。哨兵仍保留在工作参数中，没有被保存成黑色。
- Playback 的开始/结束选项、语言键和上下限从 6257/126 静态提取。随机开始切换为 never、cycles=-1；其他开始切换为 after、上次输入次数或 1；非 after 结束保存 -1。随机开始隐藏 toggle，toggle 文案取当前开始方式。点击同一选项不修改参数。
- 次数范围 1–100，清空输入可暂存 0，失焦后钳制；过滤非数字、前导零、最大值，Enter/Escape 结束输入，方向键增减。按下箭头先等待 300ms，再以 100ms 间隔重复，首次数值变化在 400ms；短按在释放/移出时执行一次，300–400ms 之间释放不执行。边界、窗口失活和禁用均终止继续修改。
- 开始/结束下拉占满实际容器宽度，弹层宽度来自触发框布局测量；未写死成调色器的 228px。下拉先以零高挂载、100ms 后显示；关闭立即收高，100ms 后卸载；箭头旋转 100ms。

这些事件只更新本地工作参数，不写入图层 Document 或设备区域。源 `9286:U` 的设备区域应用与 DLL 修改/写回仍按路线后置。独立窗口重建时重新绑定输入订阅；同一 Playback 根挂载期间保留已输入次数，离开根后清除。

## 仍有差异，不能视为完整验收

- 哨兵显示尚未完全复刻：当前对 `FU` 显示无色色块并使用无色编辑器；原 `9170:iN` 会得到 `[25600,0,0]`，调色器对该数组特殊处理，但 HEX/RGB 字段可能保留 `64000000` / 红通道 25600。该异常字段路径仍需单独实现，不能把当前无色字段称为一致。
- Base Popover 的窗口边缘避让不同于源非 portal CSS；下拉项方向键导航、触发框边框以及 Checkbox/spinner 背景颜色的 100ms 过渡仍需补齐。Checkbox 快速反复切换使用 Base Presence 反转插值，和 CSS 重新开始 keyframes 尚未完全对齐。
- 共用调色器的 HEX 显示大小写、原生取色服务、真实设备混合选择/预览仍沿用[已有缺口](chroma-studio-color-native.md)。临时工作参数不代表设备保存成功。
- 本轮没有运行程序或进行视觉/DPI 对比。允许的检查通过不证明像素一致。

## 本批检查

`cargo fmt --all`、`cargo check --locked --all-targets` 通过，保留原有 3 项未使用方法警告。Studio 主数据/属性/颜色提取器 `--check`、45 条 Studio 源收据、47 张 Studio 素材、145 个网格矩形、统一资源检查和嵌入 JSON 检查通过；JSON 为 49 份、0 失败、4 项既有跳过。未运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。

本批新增 2 个局部接入属性根；当前 Ambient、Static、Spectrum、Breathing、Fire 共 5 个根有本地实现，其余 8 个尚未挂载。全项目仍是 331 个产品 / 1419 个主导航页部分接入，完整复刻验收产品为 0。
