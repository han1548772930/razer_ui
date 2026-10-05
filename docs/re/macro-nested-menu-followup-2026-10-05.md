# 嵌套宏菜单状态、定位与时序续接

本轮继续当前稳定版 Macro `58190.je`、`38162.c`、`97278.c` 的嵌套选择器；当前应用 manifest 限定的源码、CSS 和本轮 Rust 指纹均在 [机器收据](shortcuts-macro-current-contract.json)。没有使用用户删除的历史目录或运行厂商代码。

## 当前源与本地变化

| 项目 | 源行为与本轮实现 |
| --- | --- |
| 三种状态 | 普通文本、显示编辑框但收起选项、显示编辑框并展开选项。`choice_action` 控制编辑框，独立 `NestedDropdownState.expanded` 控制列表 |
| 重选相同项 | `38162.selectOption` 收起列表，reducer 不创建更改，`je` 的 selected-index effect 不触发；现在保留编辑框，不再立刻回普通文本 |
| 更换项 | 文档身份改变后回普通文本，保留上一轮的撤销、持久引用与循环检查 |
| 禁用条件 | 候选为空时编辑框禁用；单候选可展开。空名称不生成 option 节点，方向计算只统计实际渲染的行 |
| 方向阈值 | 打开时 `trigger.bottom + 2 + min(180, rendered_rows*25 + 2) >= 688` 则向上；调用者没有传 wrapperH。方向保持到下一次展开，不因动画高度变化而反复翻转 |
| 坐标换算 | 新 overlay 在 prepaint 使用同一帧触发框坐标，减去 Macro 页面根的原生宿主偏移得到源 web-content 坐标；不再用通用 Popover 的实际视口 auto-flip/clamp |
| 上下间隔 | down 为触发框 bottom+1；up 为触发框 top−1−当前菜单高度，来自 top:27+margin-top:1 和 27px 父框的 bottom:28 |
| 选中项滚动 | 每次展开将 ScrollHandle 设置为所选行的 offsetTop（25px 行高），超出部分由布局约束；源 inactive 行没有 selected class，因此禁用选中项不作自动滚入 |
| 宽度 | 编辑框 160px；选中标签的 padding-box 最大宽 134px。菜单 min-width:100%，按 nowrap 名称固有宽度加水平内边距/边框撑宽，不再固定为 160px |
| 菜单时序 | height:0/auto 离散切换；max-height 0→180 使用 200ms ease，border-bottom 0→1 使用 100ms ease；收起时立即卸载行，不增加淡出；编辑框隐藏时菜单立即不绘制 |
| 触发器时序 | 边框/opacity/箭头为 300ms ease，保留共享 motion 的减少动画适配；hover、expanded 与禁用颜色按当前 CSS |

`15030.th/s` 在打开某个自定义输入前关闭其他编辑器；本地沿用单一 `choice_action` 并先提交待结束的普通行编辑。新增收据还记录 `68511.uM = macroRecordingEvent`：源收到录制事件会关闭编辑器，但本地真实录制器尚未接入，不能声称已经收到该事件。保存确认打开时清除行选择弹层，避免它在确认框上继续接受输入。

外点关闭目前使用原生 mousedown-out 和当前帧触发框/菜单命中范围，源用 window click；Escape 由 Macro 页既有关闭路径处理。这个事件时机差异仍明确保留。代码未新增引擎执行或原生录制接口。

## 验证与剩余范围

`cargo check --locked --all-targets` 已成功；曾等待同一工作区的 Cargo 锁，等待期间跟踪原检查会话，没有重启检查或终止其他进程。只有既有 `customize_page::layer_button` dead_code 警告。

源码契约 `--check`、格式检查、嵌入 JSON 校验和语言键校验通过。442 个字面量语言键没有缺失；33 份嵌入 JSON 语法解析成功，其中三份既有文件跳过结构推断。没有运行应用、构建、测试、安装器或 DLL。

仍未完成钢笔编辑光标和菜单滚动条外观的原生接入。字体实际度量、首帧几何、菜单裁剪/叠放、焦点和过渡时序没有经过运行窗口验收；源 CSS 与本地公式静态一致不等于渲染像素已验收。原生宏录制、完整 Sequence/Phased 创作以及全局产品 UI 待办仍见 [完整清单](remaining-ui-work.md)，总体 goal 保持 active。
