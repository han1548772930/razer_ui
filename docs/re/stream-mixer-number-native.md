# 3334/3337 混音数字编辑（partial）

当前来源是各产品自己的 manifest/main 中 uU → SU（AU）以及 main CSS。可重复静态收据见 [证据](stream-mixer-number-current-evidence.json)，维护工具为 `node tools/audit-stream-mixer-number.cjs --check`；只解析源，不执行厂商 JavaScript。共两产品、6 条 AST 收据，4 次原始箭头与共享嵌入 SVG 的逐字节比较。

## 已接入

在现有两条总线推子上挂载专用 MixerNumber 子实体：数字文字编辑、方向键、鼠标点击步进、300ms 长按、输入框内滚轮。文本暂存与父本地草稿分离，输入 Change 不提交，Blur 按源 parseInput 规则转换及夹到 0–100 后提交。Enter、Escape 都触发 Blur，不把 Escape 实现为回滚。空串和单独负号提交为 0。

按当前源保留键入字符串与数字的差别：键入 5 后上箭头是字符串拼接得到 51，未键入的数字 5 上箭头得到 6；减号执行数值运算。maxLength 使用当前值是否小于 0 决定 4 或 3，允许负号和纯数字。显式禁用 Base 默认数字格式化，保持源码的原始文字草稿。

主开关及各总线开关共同限制数字操作，父 edit 在提交时再次检查 gate；只在 STREAM_MIXER_HEADER 消费子事件。父草稿变化同步显示，兼容滑块规范化产生的浮点 JSON 数字；相同父值不覆盖尚未提交的文字，Blur 仍规范化文本。页面切换及 restore 清理暂存与重复任务；松开、移出、禁用、到达边界或窗口失活停止重复。

本地修改进入 AudioProductChanged → 既有 capture/snapshot 链，没有 DLL 写入，也没有设备成功确认。原 profile 与 reducer 初态区分见 [继续记录](ui-readonly-first-roadmap.md)。

## 外观与边界

输入区采用当前最终覆盖后的 62×26、灰色边框、14px 字号、left 6/padding 0 对应文字位置、14×12 常显箭头及当前资源；没有沿用被后段 CSS 覆盖的悬停绿边。Base 提供编辑、焦点与方向键语义，原版 CSS 提供外观参数。

当前仍挂在通用音频面板的标题/推子布局中，未完成原混音根的整行图标、推子与电平表几何；未做窗口像素、输入事件运行验收。原版注册后的窗口级 mousewheel 仍未接，目前仅处理控件内滚轮；超长粘贴的浏览器截断细节、应用级视图移除时的生命周期和全部边缘交互仍需后续复核。两款产品保持 partial，设备选择、输入通道、冲突确认及预设操作继续待办。

格式化、cargo check --locked --all-targets、专用收据及音频静态校验通过；仅三项既有 Rust 警告。嵌入 JSON 51 项通过、0 失败、4 项既有跳过。未运行应用、构建、测试、厂商脚本或 DLL。


独立回读已确认 Change 主动重绘、Base 数字 mask 关闭、鼠标点击回调顺序去重、键盘步进和父草稿恢复链；未发现本批新的数据编辑阻断。另保留 Base NumberInput 后置 disabled 覆盖单个箭头 limit 的表现差异：应用处理器阻止边界鼠标写入且显示半透明，但边界箭头的指针/辅助功能禁用语义尚不能声称完全等价。报告见 [独立验证](ui-readonly-first-roadmap.md)。
