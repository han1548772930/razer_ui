# 226 普通 Performance 分段 DPI 滑条：当前源独立复核

复核日期：2026-10-06。本子任务只静态解析当前 226 的源文件和当前 4.0.827 包中的 React DOM 依赖，没有运行厂商 JavaScript、应用、构建、测试或 DLL，也没有修改界面实现。结论适用于普通 Performance 的 `es → Wt/Ft` 路径，不自动覆盖其他产品或 displayMode。

可复核数据在 [mouse-226-dpi-slider-review-evidence.json](mouse-226-dpi-slider-review-evidence.json)，由 [review-mouse-226-dpi-slider-source.cjs](../../tools/review-mouse-226-dpi-slider-source.cjs) 静态生成。`--check` 比对重新提取的完整 JSON；收据包含 16 项 AST、99 项 CSS。CSS 收据保留相关基础及阶段规则，是否挂载仍须按下述实际节点判断，不能把收据中的其他 slider 变体全套进来。

## 当前源与范围

以下区间均为 JavaScript UTF-16 半开区间。

| 文件 | SHA-256 | 本轮证据 |
| --- | --- | --- |
| `.ref/devices/226/asset-manifest.json` | `d3112f2e93e99a6b12a514307f53918e944fee599c3ea92ae3cd0e2f9b11a5e2` | 声明 main、8355 JS/CSS |
| `.ref/devices/226/static/js/main.08f95762.js` | `ccbd5af37e23d1c64faf62551d15b0ef91d6e89fc06cafab3fb9464c598f884e` | CONFIG 模块 1057 `[54515,68130)`；辅助模块 1867 的包含式区间判断 `[94242,94261)` |
| `.ref/devices/226/static/js/8355.3d5e573e.chunk.js` | `b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8` | 模块 4125：配置解构 `[133453,133503)`，Ft `[133558,139363)`，Wt `[139372,139374)`，Qt `[139553,139628)`，es `[139629,143474)`，ls `[143618,148809)` |
| `.ref/devices/226/static/css/main.c2099849.css` | `68bc9ccdc21fe548330106b3a27beb5049be06f55939e5950677b220f52703df` | slider 基础/large/grid/阶段 cascade |
| `.ref/devices/226/static/css/8355.f94d4299.chunk.css` | `ec3c1904f52ffa472fbcaca33ec514681ce2ea2d329279205c0c36ade329c53a` | 已解析；本普通阶段路径没有覆盖下列 slider 几何的规则 |
| `.ref/host-4.0.827/electron/assets/js/react-dom/18.2.0/umd/react-dom.production.min.js` | `61de6fd556cade38efeec58c0587853d3d7c689ee1bb84fab3a254b409954501` | 当前官方 host 静态依赖：w `[6409,6819)`、S `[6819,7164)`、x `[7164,7322)`，说明 controlled input 仍更新 defaultValue；不是浏览器运行结果 |

226 `index.html` 指向 React/React DOM 18.2.0，产品版本为 0.0.14、buildVersion 2608120952。CONFIG 的完整 export 表没有 `SENSITIVITY_RANGE_VALUES`，所以 `kt` 缺省；DeviceInfo 明确 minDPI=100、maxDPI=50000、dpiStep=1、isSensitivitySliderWithGrid=true、supportXYDPI=true。`Qt` 为真，`es.renderSlider` 实际挂载 `Wt`，其别名正是 `Ft`。

## 数学映射与浏览器 range 边界

`es` 给 Ft 传入 min=100、max=50000、step=1；`Ft` 中配置常量 `Bt/Zt/Gt` 也来自 226 的 DeviceInfo。默认分段如下，`p` 单位为百分数而非 0–1 比例：

| DPI 区间 | p 区间 | HTML range 的本段 customStep |
| --- | --- | --- |
| 100–500 | 0–15 | 15/400 = 0.0375 |
| 500–1500 | 15–30 | 15/1000 = 0.015 |
| 1500–10000 | 30–45 | 15/8500 ≈ 0.00176470588235 |
| 10000–15000 | 45–70 | 25/5000 = 0.005 |
| 15000–50000 | 70–100 | 30/35000 ≈ 0.000857142857143 |

- `parseSliderValueToDPIValue(p)` 从左向右取首个 `p <= segment.to` 的段。结果为 `Math.round((p - previous_to) / segment.step) * props.step + start_dpi`；首段 previous_to=0、start_dpi=props.min，其余使用该段 fromValue。分界点属于左段；有效域内为非负舍入，半整数向上。
- `parseDpiValueToInputValue(dpi)` 首段选 `dpi <= toValue`，后续选 `dpi > fromValue && dpi <= toValue`；先把 customStep 设为命中段的 step，再返回首段 `(dpi-min)/props.step*segment.step` 或后续 `(dpi-fromValue)/props.step*segment.step+segment.from`。
- 数学锚点为 `100→0, 500→15, 1500→30, 10000→45, 15000→70, 50000→100`。例如 800→19.5，1800→约30.5294117647。普通线性 100–50000 滑条的低 DPI 位置不符合当前源。
- Ft 的 `<input type="range">` **没有**传 min/max，靠 HTML range 默认 0–100；`step` 是 state.customStep。不能把它直接替成整数百分比步长，也不能将 UI 的 DPI step=1 误用为百分比 step=1。
- `onChange` 读取浏览器已给出的 `target.value`，转 Number 后确定新段、更新 customStep，再做上述分段转换。因此事件值可能先经过此前 customStep 的浏览器处理，跨段操作不能宣称是完全无量化的连续数轴。
- **静态不能证明精确的浏览器量化时序。** 缺 min 属性不等于步长基准恒为 0：当前 React DOM 的 controlled input 更新函数 w 仍调用 x 同步 defaultValue，挂载 S 也赋 defaultValue。value 内容属性参与 HTML 步长基准，更新次序以及 Chromium 在拖动中重新匹配 step 的细节不能由 Ft 算法单独证明。本轮没有执行浏览器；不提供虚构的跨段逐事件数值轨迹。数学正反映射是已经核实的要求，浏览器量化细节仍是未验收边界。

## 预览、提交、键盘与提示

`Ft.onMouseDown` 只把 mouseIsDown 设 true；有 thumbTag 时立即显示 tip、隐藏贴在 thumb 上的 X/Y 字符。`onChange` 更新 inputValue，并调用 `changeValue(dpi, mouseIsDown)`。`ls` 在拖动时更新自身当前行的 DPI 预览，不继续调用上层保存；只有第二参数为 false 才转交上层。Ft 在全 window 注册 mouseup，有正在进行的按下时转 false 并发送 `state.value`，不是直接把最后百分比当 DPI。父级传回 value 后，Ft 才同步 state.value 及反向百分比。没有源证据支持本滑条增加 debounce 提交。

四个方向键（keyCode 37/38/39/40）全部 `preventDefault()`，没有用方向键调用 changeValue。不要沿用通用原生 slider 的方向键增减来冒充 Ft 行为。Ft 这段没有禁止 Home/End；也没有足够静态证据给未显式处理的键凭空定义设备提交。数字输入的按键/滚轮/步进器是模块 4230 的另一路，不受此结论否定。

提示内容与条件：

- XY 关闭时 thumbTag 为空，`.stage .slider-container .slider-tip{display:none}`；普通阶段不弹数值 tip。
- XY 开启时 X 行传 `thumbTag:"X"`，Y 行传 `"Y"`。Ft.updateValue 优先把 tip.innerText 设 thumbTag，所以浮动提示显示 X/Y，**不是 DPI 数字**。
- mount 初始化 tip opacity=0、thumbTag opacity=1。
- `getPosition()` 返回 `p/100*(W-16)-tip_width/2+8`，其中 W 为 mount 时测得的 slider 宽，缺省 300。`onMouseMove` 用当前 slider 左边界，`onMouseEnter` 用 mount 时缓存 leftBound；比较相对指针 x 是否在 `[getPosition()+3, getPosition()+19]`，辅助模块 1867 判断含两端。
- 未按住时 move 按上述区间切换两层 opacity；按住时 move 保持现状。enter 本身没有 mouseIsDown 保护。leave 无条件隐藏 tip、显示 thumbTag；mouseup 只提交，不自行重置提示。这些条件不能简化成“hover 整个 slider 就显示数值”。
- 原源未挂 window blur 取消处理，unmount 只移除 mouseup 监听。实现若为避免离页遗留拖动而补本地清理，应记录为本地生命周期处理，不宣称来自 Ft 的 blur 分支。

## 当前 cascade 的尺寸、颜色与动画

以下 CSS offset 来自主 CSS 的 UTF-16 源；CSS 是静态证据，不等价于窗口像素验收。

| 部分 | 当前源要求与证据 |
| --- | --- |
| 容器 | `.stage .slider-container` offset242937：height20px、margin-left10px、width250px；`.slider-container.large` offset47601 的 width300px!important **覆盖250px** |
| enabled/disabled | 基础容器 opacity .3、pointer-events none、opacity .3s（43151）；`.on` 1/auto（43347）。但 `.stage .slider-container` opacity1（242886）覆盖基础淡化；隐藏阶段由 `.stage.off .slider-wrapper` opacity .3（243953）控制。不要再给 inactive slider 叠 .3 形成 .09。pointer-events none 仍保留 |
| slider 轨道 | track/fill 高6px；track `#44d62c4d`，fill `#44d62c`。no-bordered 将两者 radius 设0（47647）。stage 直接子元素除 tip 之外 top0、bottom0、上下 margin auto（243123），固定高轨道据此垂直居中 |
| thumb | 通用 thumb 16px，实际 `.slider.thumb-sm` 覆盖为12×12（48057）；radius8px；背景 `#44d62c`。hover `#5d5d5d`+2px `#44d62c` 边，active `#383838`+同边。transform .2s、background .3s（44426–44878） |
| fill 与 thumb 字符 | JS fill.width=`p/100*(W-16)+8`；**虽然 thumb 实际12px，源仍用16**。thumbTag.left=`3.1+287.5*(p/100)`px，是固定插值，不是常规 thumb中心对齐的重推导公式 |
| thumbTag | color `#212121`，font-weight700；no-bordered 覆盖 font-size10px、height:auto、line-height2.2、margin-left0、width:auto（48119）；z-index99，pointer-events none。不要沿用默认12px/16px宽字框 |
| 浮动 X/Y tip | `.stage.y-enabled .slider-tip` bottom25px、display:block、opacity0（243809）。基础 tip 绿色 `#44d62c`、字色 `#212121`、12px、line-height14px、padding4px 8px、radius3px、width max-content；最终 transition `left 0s,opacity 0s` 覆盖前一条 opacity .3s（45081），所以 tip 显隐不应新增300ms渐变 |
| 刻度线 | `.range-custom` display:flex、margin-top2px!important（47736）；`.range-item` absolute、1×16px、背景 `#204c19`、translateX(-50%)（47806）。不应替成等距网格 |
| 刻度标签 | `.range-item div` 11px、margin-top24px（47978）；继承 body 的 Roboto、`#ccc`（body规则）。标签为原始整数，未格式化为1.5K/10K |

主刻度：0% 的100、15%的500、30%的1500、45%的10000、70%的15000、100%的50000。0%刻度内容 justify-content:start，100%为end，其余center。额外无文字细刻度仅为：57.5、60、62.5、65、67.5，以及72.5、75、77.5、80、82.5、85、87.5、90、92.5、95、97.5。45–57.5 之间没有从均匀网格推出来的47.5/50/52.5/55线。

Ft 自己没有 SVG/位图资源：thumb、track、tick、tip 都来自 CSS，X/Y来自文本；无需为滑条本身生成图标。阶段 ordinal、XY按钮、拖排图标属于父级 es，不能由本滑条报告自动验收。字体来自当前 CSS 声明的 `/synapse/assets/fonts/Roboto-*.woff2`，本轮不重新声明字体资源已准备的覆盖面。

## 实现核对边界

本报告给出真实源的参数与分支，并未读取或批准主线程本轮正在修改的最终实现。需要最终实现逐项确认：分段正反映射；动态步长或诚实记载的原生差异；按住预览、释放提交；四方向键不步进；隐藏行只有外层 .3；300×20容器与12px thumb；原始非均匀刻度；XY文字提示的条件、位置和零时长显隐。

`node tools/review-mouse-226-dpi-slider-source.cjs --check` 只证明提取收据对应当前文件。浏览器 range 量化、native pointer capture、不同缩放的字体/像素位置和真实设备数据流均没有因此获得运行验收；当前任务继续保持部分实现。

## 2026-10-06 实现首次回读（历史快照，以下方补丁复核为准）

本次还读取了新 `src/features/mouse_226_dpi.rs`、`mouse_products.rs` 的 grid 接口与 `mouse_dpi_rows.rs`，以及 Cargo.lock 实际选用的 gpui-base 0.7.1 `src/slider.rs`。这是主线程修改中的快照；以下已报给主线程，不能把本段当成其后补丁仍有同样问题的证据。

已静态确认的正向路径：分段数据和数学正反映射与上表一致；百分比 `position` 与真实 DPI `model` 分离；`Preview` 只刷新真实数值 SliderState/Input，不写 draft；`Commit` 进入 write_number，同步 linked Y 的草稿及其数值实体，Y grid 可由观察更新；阶段拖排 guard 已换为独立 dpi_dragging，不会再把 slider 自身的原生 drag 当成阶段拖排。

首次回读发现的确定问题：

1. **最右端 step 查找可能 panic。** Base `update_value_by_position` 先限指针范围，再按 step 舍入，舍入后没有再次 clamp。当前 step=.015 的最右值可为100.005，.0375可为100.0125；Grid 虽在 from_percent 内 clamp，但随后直接对原百分比查 `segment.to` 并 unwrap，会找不到段。须在所有后续映射前统一限至0–100。
2. **按住 thumb 原位松开缺 Release。** Grid capture 已设置 pending/pressed，但 Base SliderThumb 的 mousedown 只 stop_propagation，只有 update_value_by_position 才设置 Base dragging。无移动时 Base handle_release 不发事件，Grid pending/pressed 留存，之后 move 不再刷新 tip，源的同值松开提交也缺失。须由真实 mouseup/out 处理并与 Base Release 去重，不能伪造 Change。
3. **预览回传的反向同步跳过。** Grid Change 先把 this.value 改为整数 DPI，父 model 同值回传后 observe 的 `this.value != value` 为 false，百分比不会重新按整数 DPI 反算；源 Ft 只要 props.value 相对上次 prop 变化即重新反算。当前只在 Release 反算。可独立保留上次外部 model 值来判定 prop 变化；动态分段尤其第五段可能出现细小的百分比偏差。
4. **CSS层次和命中框。** 初版把 tick 放在轨道/填充之后，又把 input 放在 X/Y 字符和 tip 之后；主线程已识别此项并正在调整。初版 input/Track 为 top2/height16，而源 input 是 top7/height6，thumb12溢出；扩大整个轨道命中区域并非源几何。视觉中心相同不代表 hover/click 范围相同。
5. **键盘/可访问性尚未贯通。** Base Slider 的可访问性 Increment/Decrement 只 set_value/notify，不 emit Change；当前 Grid 只订阅 Change/Release，所以此类动作可移动 percent 而不改整数 model/draft。Base Slider 也没有 FocusHandle/tab_stop，初版 Grid 未保留焦点，外层 on_key_down 不能单独证明源 range 的 Tab 可达性。可访问性值范围0–100与源 range 百分比相符，但完整动作与焦点路径仍需完成。

此回读未运行 cargo 或任何测试；源提取的 `--check` 与实现编译是不同证据。上述问题的最终状态应以其后独立回读或主线程明确列出的补丁为准。

## 2026-10-06 补丁窄范围复核

已重新完整读取 `mouse_226_dpi.rs` 和 `mouse_products.rs::edit_dpi_grid` 当前补丁；这次同时读取 Cargo.lock 选中的 gpui-base 0.7.1 slider 与 gpui-pre 0.3.8 的 effect 队列实现，未运行应用、测试或 DLL。首次回读列出的五项已按如下范围推进，不能继续把旧快照当成当前缺陷：

| 旧项 | 当前实现的静态证据 |
| --- | --- |
| 最右端 panic | `Change` 先 `bounded=percent.start().clamp(0.,100.)`，from_percent 和 segment 查找均使用 bounded；值超界时 position 本身也设回 bounded。当前数据最后段 to=100，有限指针值不会再因100.005一类舍入结果缺段 |
| 原位 thumb 松开 | 新 `release` 由 Base Release 与 input 的真实 mouseup/up_out 共同调用；先清 pressed，再 `pending.take()` 只提交一次。thumb 未移动时 input 的真实松开仍能完成同值提交；Base 另外发 Release 时不会重复保存 |
| prop 回传反算 | 独立 `row_value` 记录上次外部 model；observe 按 row_value 比较，而非按已更新的 preview value 比较。父级 Preview 回传新整数 DPI 时会同步 value、row_value，再执行 sync_position。恢复 reset 也同步 row_value |
| 层次/命中框 | 子项顺序已是 ticks→track/fill→input→thumbTag/tip；input top7/height6，thumb相对 top−3/size12，恢复源6px输入框及12px溢出thumb的静态几何；tip显隐仍为即时opacity，未添加错误动画 |
| Tab/语义动作 | GridState 保留 FocusHandle，外层 track_focus/tab_stop(enabled)，pointer按下显式focus；四方向键仍拦截。独立 position.observe 补上 Base 只notify的Increment/Decrement。这里确认代码接线，不据此声称屏幕阅读器/实际Tab运行验收已完成 |

### position.observe 桥是否误把程序同步写成本地草稿

当前静态结论：**没有发现当前程序同步路径会误触发这条 Commit 桥**。依据不只是 expected_position 的注释，而是逐一读取当前所有写入者及框架时序：

1. `position` 是 GridState 的私有实体。新建 default_value 初始化后，当前应用代码仅有两处 `position.set_value`：`sync_position` 和 Change 的越界修正。两处都先设置 expected_position，再调用 set_value；reset、model观察均走 sync_position。
2. 程序同步的 notify 观察读取的是实体当前值，而不是过期的事件数值。多个同步连续发生时，expected_position 与最后一次写入保持一致；相等即 return，不 emit Edited。
3. gpui-base `SliderState::update_value_by_position` 按顺序 emit Change、notify。gpui-pre `Context::emit` 将 Effect::Emit push_back，App::flush_effects 用 pop_front 处理；notify同样入队。因此普通指针 Change 先执行订阅，设置 pending/expected，再到位置观察；超界修正也事先记录 expected。它不靠未核实的同步回调假设。
4. `SliderIndicator` 的 prepaint 只 set_bounds，不 notify；它不会被误识别成数值调整。步长 builder 也不会单独修改位置或发 notify。
5. Base 的可访问性 Increment/Decrement 调用 set_value/notify而没有 SliderEvent；这类独立位置变化可到达 observer。observer 先更新 expected_position，若 pending/pressed 仍为真则不提交；空闲时换算真实 DPI 并发送 Commit。父层仍检查 Performance、当前可编辑观察、行可见性、阶段开关所显示行、XY轴条件、阶段拖排状态；拒绝路径 reset 当前 grid，不伪造设备成功。
6. Commit 引起父层 model 更新后，row_value观察通过 sync_position 记录新的 expected，再写百分比；这个闭环不会把自己的回传再次当作新语义动作。

这条桥的范围取决于 `position` 的写入封装。如果后续增加新的程序 set_value 调用，必须同样先登记 expected_position；本报告不替未来调用者证明安全。当前位置比较使用同一 f32 值的赋值及读取，没有额外计算差异；native分段使用f32、浏览器动态step与value属性的精细量化差异仍沿用前文未验收边界。

保留的验证限制：浏览器/原生不同缩放下的真实hover hitbox、thumb溢出命中、mouse capture、Tab导航和辅助功能客户端事件仍没有运行证据；当前完整父页与其他产品不能由此宣布验收。此次只把上述具体静态缺陷闭合，并确认目前不存在已发现的程序同步冒充本地用户写入路径。
