# 当前公共滑条动画复核（2026-10-06）

范围：3858、3880、3893、3900、3907、3921、1383，以及 Nommo 1303/1304
所用普通 `.slider`。只解析各产品当前 manifest 声明的 JS/CSS，不执行下载的代码。
维护工具为 `tools/audit-source-slider.cjs`，逐产品的 manifest、样式和组件源码、
SHA-256 与 UTF-16 字符位置保存在 `source-slider-current-evidence.json`。

本轮此前的检查只覆盖静态端点，遗漏了两条实际生效的过渡：

| 元素 | 当前源声明 | 本地修正 |
| --- | --- | --- |
| `.slider-container` | `opacity .3s`，默认 `.3`，`.on` 为 `1` | 使用每个 SliderState 的独立 300ms ease 通道；禁用操作立即生效 |
| `::-webkit-slider-thumb` | `transform .2s,background .3s` | 背景在绿、悬停灰、按下深灰之间按 300ms ease 插值 |
| 滑柄边框 | hover/active 为 `2px solid #44d62c` | 即时改变；边框不在 transition 属性列表内 |
| `.slider-tip` | 后面的 `transition:left 0s,opacity 0s` 覆盖前面的 `.3s` | 继续即时跟随数值；没有增加气泡淡入或拖动滞后 |

当前普通滑柄这三个状态没有 transform 端点，因此没有凭声明中的 `transform .2s`
添加缩放或位移。背景采用 RGB 通道插值，避免框架只适用于近灰色的 HSL 线性插值
让绿色在渐变途中变成另一种色相。动画由 GPUI Base motion 管理，支持中途反向和
减少动态效果；状态以保留的 SliderState 身份隔离，不以页面列表序号作键。

`source_thumb` 继续返回 Base SliderThumb，由框架处理拖动；额外监听仅记录颜色状态。
按下观察放在 capture 阶段，避免被 Base 防止轨道跳动的冒泡拦截吃掉；在滑柄外释放
也清除按下状态。普通 SourceSlider 的关态仍立即禁用操作；Nommo 的 brightness
保留源 `no-pointer` 特例，关闭亮度时仍可拖动恢复亮度。

验证命令：`node tools/audit-source-slider.cjs --check`（使用现有 Acorn 依赖缓存），
Rust 格式化和主任务统一的 `cargo check --locked --all-targets`。
这不证明浏览器与原生窗口的实际栅格化、命中、DPI 或交互时序已经完成运行验收；
没有运行应用、构建、测试、安装器或厂商 JS/DLL。
