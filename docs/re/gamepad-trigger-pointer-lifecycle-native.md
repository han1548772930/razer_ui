# 手柄扳机滑条：命中、预览与提交复核（2026-10-07）

本批继续检查已写的扳机界面，不把路由挂载或静态编译等同于界面完成。涉及 2629、2636、2647、2676、2684、4133、4144 七款产品；实现位于 `crates/razer-pages/src/features/gamepad_products.rs`。

## 当前来源

[逐产品证据](gamepad-trigger-lifecycle-current-evidence.json)保留各自 manifest、JS/CSS SHA-256、UTF-16 切片位置和当前绑定。`python tools/audit-gamepad-trigger-lifecycle.py --check` 本次再次读取 `.ref/devices/<pid>/` 中的实际文件，核对 21 个 AST 切片、77 条 CSS 收据和各产品的 reset/mode helper。没有读取已废弃来源，也没有执行厂商 JavaScript。

七款当前双柄组件的共同约束为：

- 两个 `input type="range"` 均为 0–100；各自 `onChange` 只改组件内状态，`onMouseUp` 才调用父级 `changeValue(start,end)`。
- 起点使用 `Math.min(value,end-1)`，终点使用 `Math.max(value,start+1)`，至少相差 1。
- `useEffect` 分别在外部 `minValue/maxValue` 改变时更新组件取值。
- `.rangeSlider input[type=range]` 为 `pointer-events:none`；20px 的 `::-webkit-slider-thumb` 为 `pointer-events:auto`。后绘制的终点 thumb 在重叠区位于前面。
- input 宽度为容器的 100%，有 `margin-left:-10px`；原生 20px thumb 的中心沿 `W-20px` 移动。高亮条另有 `margin-left:-10px;margin-right:12px`，数值提示保留源码 `.sliderTipBar{width:96%}`，并不擅自把这些不同几何统一。
- Reset 只更新当前 Analog 或 Digital 分支的字段。每个产品自己的 reset helper 和 mode 枚举已核验，没有将一款产品的默认值套给其他产品。

## 本次修复

此前整个 40px 容器接收按下，以鼠标最接近哪个端点来选柄。这会让点击轨道也改变取值，并且鼠标从圆柄边缘按下或释放时会因绝对坐标重新定位而跳值。现在只在实际 thumb 上开始操作，保存抓取位置、原值与有效行程，用鼠标位移更新预览。端点绘制和位移换算采用同一 `W-20px` 行程。

拖动时按窗口范围观察移动与左键释放，鼠标离开轨道仍可更新预览。thumb 自身也保留释放/外部释放处理；首次处理取走 pointer/preview，后续重复释放不再提交。鼠标不再按住、窗口失活、页面切换、恢复本地快照及外部扳机模式/范围更新均清掉旧预览。按下态优先于 hover，避免按住圆柄时反而显示 hover 背景。

每个 thumb 使用 GPUI Kit Base Button 保留稳定标识和焦点，并覆写为 Slider 辅助角色，提供当前值与 0–100、step 1 信息。方向键、Home/End 遵循原生 range 的取值规则，仍只变组件预览；源码没有键盘释放提交回调，因此未凭空加入该设备提交语义。Escape 作为本地取消预览操作。键盘预览之后抓取同一范围的 thumb，会从预览值继续。

Reset 现由 Base Button 提供命令语义和禁用状态。处理时重新读取当前模式，清除该范围的旧预览，将当前分支的两个字段一起改好后只发一次 `GamepadProductChanged`。这避免保存订阅看到中间字段组合。已移除未被渲染的 start/end `SliderState` 及其允许两端相等的旧回调；数字触发点和其他普通滑条保持原有状态实体。

## 验证与边界

已运行上述 Python 静态来源核验以及目标 Rust 文件格式化。统一 `cargo check --locked --all-targets` 由主任务执行并记录。没有运行应用、构建命令、测试、安装器、下载的 JS 或 DLL。

鼠标和键盘显示的预览不进入 `snapshot()`；鼠标释放和 Reset 仅修改本地草稿，并发出本地变更通知。没有新增设备读写调用或成功回执。像素布局、平台原生 range 行程与 GPUI 指针路由仍需允许运行后的窗口验收；本批不据此声称七款扳机界面或整款产品已完成。
