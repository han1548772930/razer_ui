# 键盘触发行程页静态接入

2026-10-03，当前 `.ref/devices/` 中的产品入口和实际挂载页。证据由 `tools/extract-keyboard-actuation.cjs` 静态 Acorn 解析产生，记录于 `src/features/keyboard_actuation_data.json`；未执行下载的 JavaScript。

接入 580、614、642、678、679、688、719、720、721、728、740、741、742、746、747 的首批触发点编辑。各产品分别使用其 `DeviceInfo.analogSpecs.actuationInfo`，不把 12 个单位与 1638 个单位的模拟轴换算混用。第一代的显示偏移单独保留；742 的最大行程和 747 的默认值也保留自身定义。

使用原有按键几何，按源代码常量及模拟输入类型过滤不可选择按键；禁用映射、动态键程、模拟控制器、摇杆移动及已有第二段动作限制也参与判断。支持多选、全选、取消选择、改触发点和同步至全部可用按键。数值持久化到当前配置／键映射的 `mappings[].mapping[].actuationPoint`，使用原始整数单位。创建新触发点时建立标准及 Hypershift 两份映射；改映射和重置按键功能保留触发点与已有快速触发数据。

范围检查通过 `tools/validate-keyboard-actuation.py`。`cargo check --locked --all-targets` 通过；没有运行应用或测试。

## 同步操作、警告与逐产品样式

`tools/audit-keyboard-actuation-sync.cjs` 对上述 15 个产品分别解析当前 manifest、实际挂载触发点类、同步操作及 reducer、模块 3342 的映射辅助函数、对应 CSS 和原 SVG；结果为 [当前同步证据](keyboard-actuation-sync-current-evidence.json)。厂商脚本仅作为 Acorn/CSS 数据解析，没有执行。

- 源同时挂载“同步所有按键”和“同步选中按键”。按钮绑定在 35×25px 图标区域；旁边 14px/14px、27px 高、白色下划线文字是展示元素。两列各占 50%，源 210×33px 的图标按背景坐标 `(7,-5)` 裁切，不缩放成 20×20px 图标。hover 只变绿色边框，禁用整行 opacity=.3，字体继承 Roboto。图标来自每款产品自己的当前 manifest URL，15 份读取字节一致。
- 14 个产品行 padding 为 `10px 10px 0`；747 是 `10px 0`，且容器高 60px。`keyboard_actuation_presentation_data.json` 保存这些各自的 CSS 能力和父树 factory-profile 禁用条件；共享实现不写产品编号分支。
- 同步的原 payload 为 `{0:makeActuationPointValue,1:makeActuationPointValue}`。本地同步两点使用同一个原始整数值；普通触发点拖动保留独立 release 值。源值与滑块显示值分别保留，避免 580 的未整除默认原始值 4 被滑条取整为 0 后又同步回去。
- 模块 3342 的生成器逐款核对：720 的当前 `aC` 同时更新 `mapping[1]` 为 `defaultGroup` 时的触发点，新建 secondary 也使用同步两点；其余 14 款保留各自默认 secondary。本地通过该 AST 导出的能力选择，不能把 720 行为推广到其他产品。正常 `useNikoFpsAnalogConfig` 未启用分支只看 make 点是否不同于默认值来决定新建映射，不因为 release 不同单独新建；该运行开关的真实生产者和特殊默认集合仍待接入。
- “同步选中”的 `isActive` 来自父树 `buttonList.some(isFocused)`；当前使用拥有者的选中集合。同步所有另有 `isDisabled`：初始禁用，选择变化／编辑重新允许，原映射非空时同步后 2500ms 禁用。本地延迟状态用 profile/page 生命周期保护，切页或恢复草稿后旧计时不能影响新拥有者。
- 警告精确检查 `DeviceInfo.AnalogGenVersion === "analogV2" && sliderValue < 10`，不用偏移量推导型号能力。文字使用源 `#fd8611` 和上距 20px。
- 12 款实际父树包含 factory-profile `.disabled`，580/728/747 当前挂载树没有该父门控。前者处于 factory profile 时，本地按键选择、编辑和同步不能修改草稿。factory 警告整体布局尚未在这一项完整复刻。

同步和上述状态只修改本地草稿，不调用 DLL，不显示设备保存成功。真实 actuation/传感器／adjustment 状态读取尚未完整接入，不能把默认配置、目录或本地选择当成设备确认结果。

这些仍是部分实现：尚缺快速触发完整编辑、各产品 Snap Tap／实时传感器／硬件调整、原版动画、触发点整体重置确认、混合选择值完整优先级、共享 2 秒 spinner 及默认映射清理的完整运行条件。742 的普通编辑 custom-release 分支还需独立接入。当前触发点控件为普通滑块，还未复刻原版垂直动画布局。全产品覆盖报告只将这些页记为 `partial_native`，不表示完成还原。静态解析、资源校验、格式化及允许的编译检查不能代替真实视觉、键盘焦点、设备或窗口验收。
