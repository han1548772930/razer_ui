# 740 / 746 校准页重新核对（2026-10-05）

本轮按当前产品包重新解析实际 JSX、作用域别名和 CSS，修正旧实现；旧文档“已接入”不等于布局一致。
静态工具为 `tools/audit-keyboard-calibration-modal.cjs`，收据为
`keyboard-calibration-modal-current-evidence.json`。偏移采用 JavaScript UTF-16 索引。
740 的实际 chunk 是 `9908.1f02bab3.chunk.js`，746 是 `6498.86d76d1a.chunk.js`；
工具逐包确认 `Aa → _a → Ea → Ca=Sa`，并继续解析 `ye/Me/De` 的跨模块导出。
CSS 只取各产品 main 及实际校准 chunk，保留同名规则、媒体条件和完整关键帧。

## 本次修正

| 当前源行为 | 原本地偏差 | 修正后的消费者 |
| --- | --- | --- |
| 740/746 根在 `selectedProfileGuid` 改变时与 `af06d371-861f-4b98-8d78-bfaef5cfdebf` 精确比较 | 按“factory default”等名字猜测，普通重命名配置会误禁用 | `source_workspace.rs` 的初始化和选择配置均用真实 `Profile.guid`；函数限定已核对的 740/746 |
| `Aa` 保留介绍、键盘和校准卡的整个 `body-widgets.disabled`，opacity .3；另挂 factory warning | 删除开始按钮/说明，把警告改成普通卡片 | 保留内容和按钮，禁用点击/焦点入口；独立465px警告、#fd8611边框、20px内边距、3px圆角、0 6px 10px #0003阴影；top45%、底边锚定对应 translateY(-100%) |
| 警告语言键 `g.ebo → FACTORY_DEFAULT_PROFILE_WARNING_DESC` | 错用了宏编辑器的 `FACTORY_DEFAULT_PROFILE_NOTICE` | 改用真实警告与14px原始三点路径（现有profile-more SVG的路径一致） |
| `_a` 开始按钮与说明分别有20px块间距 | 4px自造间距 | 按块级相邻margin折叠计算；146×27开始按钮保留居中 |
| `Sa` 使用 `.head .close`（36×36、内部20px图标 `icon_close.55fe41f1.svg`）；hover/active背景、200ms ease | 错用介绍横幅的24px白色close | 独立modal close实现，复用字节一致的mapping-close资源；介绍关闭保留原24px |
| `.choose-a-mat` min-width800；>=1400px媒体规则width1050、被Ea inline max-width850限制；top由100%到100px、300ms ease | 所有宽度固定850、强制夹到视口；无开场过渡 | 小视口保持800px源溢出规则，宽视口850；100ms线性遮罩、300ms位置过渡 |
| `.contentHidden` 是 visibility:hidden | 成功/失败时直接卸载内容 | 保留section布局、隐藏绘制；footer按原absolute/bottom0挂在modal |
| 空键名caret：1.1s steps(1)，0/50/100% opacity1，25/75% opacity0 | 静态 `|` | 保留800字重与四段闪烁；减少动态效果设置下静止 |
| 加载条2s ease-in-out、25%宽，从left−25%到100% | 使用二次函数近似CSS曲线 | 使用CSS cubic-bezier(.42,0,.58,1)曲线 |
| `F(calibrateBottom)` 进入step2时置loading，`verifyBottom`并不重新置loading | 把verifyBottom误当加载状态，缺少calibrateBottom等待样例 | 新增独立“校准按下位置”样例；verifyBottom无伪造加载条，下一步在未成功时不推进 |
| 次按钮只有opacity transition声明，无hover opacity变更 | 自加hover .8 | 删除不存在的hover暗化 |

减少动态效果只是本地无障碍适配；关闭时 `Ea` 返回null，故没有另造退场动画。
成功和失败仍只来自显式开发样例；正常设备入口没有硬件传输，不产生成功或进度结果。

## 尚未完成

- 实际 `inputredirect`、校准SDK、foreground事件与原版15秒错误watchdog完整时序仍未接通。
- 键盘图与本页外层的所有几何仍需另行核对；上述修正不代表整页像素完成。
- 原HTML三点图标带负margin，当前TextView保留路径和14px尺寸，尚未等价实现负margin基线。
- 步骤连线的浏览器dotted栅格和本地绘制仍须继续核对；不可把基本尺寸正确当作像素相同。
- 当前检查不运行应用或测试；已有测试只调整为新的800px规则、减少动态效果和显式等待样例，并由允许的 `cargo check --locked --all-targets` 检查编译。

可复核：`node tools/audit-keyboard-calibration-modal.cjs --check`。
