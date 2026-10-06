# 步进箭头显隐的当前 CSS 复核

2026-10-06。Nommo 1303/1304 的步进器初态隐藏箭头，hover/focus-within 时显示。
`.icon.spinner.down,.icon.spinner.up` 指定 `visibility 0s,opacity .1s linear`：
可见性即时切换，透明度按 100ms linear 变化；离开后不保留一个仍可见的淡出层。

达到数值边界的箭头仍有 `pointer-events:none`。然而 hover/focus-within 规则比
通用 `.disabled` 选择器的优先级高，其可见时透明度仍为 1，不能自行改成 0.3。
数值边界处的本地输入处理保持现有约束；静态 CSS 对照不构成实际命中测试。
当前 Kit NumberInput 会在装饰箭头之后覆写按钮 disabled，边界处仍保留按钮命中区域；
本地通过边界判断及 click 抑制阻止重复步进，但尚未复刻 CSS `pointer-events:none`
的指针穿透。这一限制不能用透明度校验通过来掩盖。

四款相机 3592/3594/3595/3596 的末尾规则明确覆盖为常显，并将到边界的箭头设为
0.3。共享 Stepper 因此新增独立的 `reveal_spinners_on_hover()` 能力，只由本次
已核验的 Nommo 调用；相机和自定义宏映射保留各自的可见策略。

维护工具 `tools/audit-stepper-visibility.cjs` 从各产品当前 manifest 读取 CSS，
解析选择器、优先级和声明顺序，验证六产品的 36 种状态，产出
[逐项证据](stepper-visibility-current-evidence.json)。`--check` 只读比较收据。
没有运行应用、测试、构建或厂商代码，运行时焦点、像素与命中仍未验收。
