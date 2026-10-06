# 226 Performance：DPI 整数框本地编辑

2026-10-06。修复独立报告 N 中 DPI 数字框的提交与显示不一致，接入源码中的步进编辑操作。本文件记录最初数字框批次；后续已接入 [逐行阶段与分段滑条](mouse-226-dpi-native.md)，因此“仅当前选中行”等初批范围以该续接记录为准。Polling Rate 条件继续未完成。

## 当前证据与参数

普通 Performance 导航挂 Ri/wi → ys/Ms → cs/ls → ts/es；es 的 X/Y 数字框挂模块 4230，六位整数、min=100、max=50000、step=1。当前 CONFIG 未声明 dpiStepInBox，所以 Kt 回退 Jt；这里没有 decimals、dataset 或 liveUpdate 参数。

来源仅为当前 `.ref/devices/226/`：8355.3d5e573e.chunk.js、8901.207fa138.chunk.js、main.08f95762.js 及 manifest 声明的 CSS/资源。完整 SHA 和 UTF-16 范围见 [当前证据](mouse-226-dpi-number-current-evidence.json)，由 `tools/audit-mouse-226-dpi-number.cjs` 静态提取 **16 项 AST、55 条 CSS、2 次原图字节比较**。主包 default 根明确一起加载 8901/8355；4230 收据限定 8901 中的副本，不使用 MapKeyboard 的同号模块替代挂载证据。CSS 收据包含 stepper/spinner 的通用与其他容器规则，只有实际挂载容器适用的规则作为本控件样式依据。

226 manifest 中的两张 stepper SVG 本地曾缺失，已从当前产品的原资源 URL 获取，字节分别与既有 `stepper-up.svg` / `stepper-down.svg` 相同；没有因文件名相同直接推定内容一致，也未执行下载脚本。

## 已接入的行为

`mouse_products.rs` 仅对已独立核对的 70/226 DPI 路径启用整数控件状态，226 当前选中行的 range 挂 `mouse_dpi_number.rs::DpiNumber`，由 Base NumberInput/InputState 提供输入、焦点和方向键行为。

- 允许整数、临时空串和单独负号；源最大六位数字。显式关闭 Base 自动数字 mask，源整数校验拥有文法；该修复也适用于已存在的 70 编辑器。
- Blur 解析并钳制，统一更新草稿、当前 InputState 和 SliderState。空串、`-`、`99` 提交后均显示/保存 `100`，超上限归为 `50000`；即使 slider 已经等于结果也回填规范文本。
- Enter/Escape 触发失焦提交，不把 Escape 解释成撤销。输入文字时通知外层重绘，typed 状态能及时更新箭头边界显示。
- 点击数字框后选择文本并注册预览状态；此后的步进先更新本地行显示，Blur 才写入 profile 草稿。未注册时步进直接进入本地编辑链，保持 ls/4230 的参数语义。
- 增减按钮、方向键和控件内滚轮按源步长处理；按下立即步进、每 300ms 重复，释放/移出停止。保留源整数 volumeUp 对刚输入字符串的加法语义：例如输入 `200` 后按增加先成为 `2001`，再经解析/钳制；不擅自改成算术 `201`。
- 滚轮专用边界取行的当前预览值（对应 props.value），不取尚未提交的输入文字。行值 50000 时输入 1000 再向上滚仍被拦截；行值 100 时输入 800 再向下滚也被拦截。仅 Tab 聚焦、尚未点击注册时不吞页面滚动。键盘和按钮不套用这条滚轮专用条件。
- 共用 helper 通过 spec 的 x/y/independent 或 X/Y/Independent 字段检查 Y 条件；没有把 70 的大写 schema 套到 226。226 当前仅渲染选中行，已退场行的延迟步进回调不能修改草稿。
- 普通 TAB、Help 与配置恢复取消重复任务；离开页面同步数字控件到已提交草稿，防止 retained 实体保留未提交预览。Help 的共用入口现名为 dismiss_editors，也继续清理滚轮预览。

本地编辑沿 MouseProductChanged → SourceProductWorkspace.capture / snapshot 保存。没有连接真实 DPI 观察发布者，没有 DLL 修改、设备保存或成功确认。源码 profile 默认值与 reducer 瞬态初值仍须分别看待。

## 保留的差异

数字框采用当前 62×26、14px、两张原箭头及 hover/focus 外观；整体仍在通用 range 行内。完整 stage 行布局、数字文本与箭头的原始重叠几何、100ms/300ms 过渡、辅助功能限值、超长粘贴的浏览器截断及原窗口级 mousewheel 继续待办。Base NumberInput 在装饰按钮后设置整体 disabled，边界按钮仍靠应用写入口拒绝与透明度表达，不能报告单个箭头的辅助功能禁用语义已经完全对齐。

226 的主阶段开关关闭后仍有通用序号列表、visible/拖排尚未接入、DPI 滑条仍是线性而非当前源分段映射；本批数字框接入不抵消这些 N 缺口。O 的 BLE/连接字段/真实限速观察也仍待补。窗口事件、像素、设备状态和实际保存往返未运行验收。

`cargo check --locked --all-targets` 与格式化通过，仅三项既有 dead-code 警告；226 数字框和 70 DPI 的源码/资源校验通过。独立子任务已核实 16/55/2 收据、空值/负号/99/60000/00100 回填、typed 连续步进、注册预览、70 schema、Help/恢复及按钮回调顺序；发现的两处滚轮边界已修复并回读，见 [复核报告](prior-ui-verification-2026-10-06.md)。这些是静态结论，未运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。
