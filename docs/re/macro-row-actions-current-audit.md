# Macro 逐行操作与成组拖动（2026-10-05）

依据当前 Macro manifest 中的 58190 `va/ha/H/F/B/V/b/k/ja/br/je/yt`、25572 `O/R/C/P/M`、15030 `cf`，以及 main 业务 reducer 的 `HH/Xg/T9/dt/FZ`。静态收据由 [audit-macro-row-actions.cjs](../../tools/audit-macro-row-actions.cjs) 生成，记录实际组件、reducer、CSS、资源与本地文件哈希。没有执行厂商 JavaScript。

## 本地修改

- 事件行恢复 70% 内容、30% 操作列，移除把 palette 名称无差别加到事件行上的文字；Text、Command、Macro 与 Loop 使用 `va` 实际挂载的标签。
- 右侧接入复制、删除及拖动标记。复制/删除按钮槽为 20px，右间距 16px，按下透明度 .3，操作列显隐 200ms。拖动标记的最终 CSS 被更具体的 `control_action span` 覆盖为 20×20，保留原 SVG。复制/删除提示使用 14px/16px、8×10px padding、黑底灰边、top 35px 及 300ms opacity；实际裁剪与层叠未运行验收。
- `R` 复制后插在当前行之后。Keyboard 无论原 State 是否 null，都产生新 identity 的 down/up 两行，并保留扩展 flag 的偶数基值；Mouse 的 6–9 滚轮项保持单行，其余复制为 0/1 两行；Loop 复制为 start/end。新副本不选中，原选中项按索引变化保留。
- `Xg` 逐行删除 Keyboard/Mouse 时只删除点击行；删除 Loop 时移除有明确相同 identity 的另一端。`FZ` 批量删除仍只删除选中行，不自动扩大范围。对没有对应端的旧 Loop，本地只删除当前行，避免原 `splice(indexOf(undefined),1)` 删除不相关末行的异常行为；这项本地保护不是源码等价声明。
- `O` 新建 Mouse、Loop 补齐真实配对结构。Standard/Phased Mouse 创建两行，Sequence Mouse 创建一条 null State；Loop 始终创建 start/end。旧的纯显示行继续保持未配对，不根据文字猜测录制数据。
- Mouse 下拉修改按 `C`：0–5 按钮函数改为 6–9 时合并为单行，再改回时在当前行前插入 down；有对应端时更新同一按钮值。Loop 数字提交同步另一端，移除源码没有的 start/end 切换按钮。
- 拖放目标使用源码的后插规则。本地不存 actionBar sentinel，因此行 i 的后插位置是 i+1，actionBar 对应 0，末尾保留区对应列表长度。palette 点击插在最后一个选中行之后，无选中项时追加；已有选择保留。
- 从选中行开始拖动会移动整个选中组，顺序取原列表；未选中行只移动自己。配对另一端不在组内时限制插入边界；两端一起移动则解除该对的边界。拖到组内目标不修改列表。Loop 交叉边界修复按 `T9` 的身份与位置计算。成功操作进入一次撤销历史。
- 拖动提示恢复当前暗色 SVG、280×40、绿色/不允许时红色、14px 粗体，以及指针减 10px 的位置。事件行下边界恢复 1px 实线，移除旧 2px 虚线。输入区域阻止启动整行拖动。

配对 ID 是本地草稿身份。分配扫描当前草稿、保存内容、撤销/重做与其他保留文档；持久校验增加 Mouse/Loop 元数据类型、范围和歧义检查。拖放还核对页面、文档与开始时的动作快照，拒绝旧文档或已被修改的列表。没有接通或伪造设备录制、执行、传输服务。

## 验证与边界

本轮仅允许类型检查、格式化、静态解析、资源准备与验证；整合结果见[续接记录](continuation-row-actions-2026-10-05.md)。没有运行应用或测试。20 个组件/帮助函数、5 个 reducer、13 个原 SVG 的审计数量不代表验收页面数。

仍缺完整 Phased 分组/阶段目标、200ms leading/trailing 选择与复制防抖、配对连线与动画、Command 警告弹层、其他参数编辑器的剩余样式/行为。真实滚动、焦点、鼠标命中、提示裁剪、字形和运行画面未经验证。此次普通列表修改不能算完整 Macro 或全产品完成。
