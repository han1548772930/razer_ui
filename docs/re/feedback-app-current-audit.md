# Feedback 应用当前源码审计

2026-10-04。入口来自当前 `rz-app-menu` 模块表：窗口名为
`feedback-synapse`，地址为 `/feedback/?app=synapse&path=%2Fsynapse%2Fdashboard`，
打开参数是 `policy=3`、`shouldFocus=1`、`tab_visible=1`。`windowName` 是应用路由标识；
当前 host 4.0.827 的 `Tab.js` 在这个入口调用 `sendCreateNewTabAction(existingWindow)`，
因此它是现有宿主窗口的具名页签。原生页面提供 `focus` 供首次打开和返回页签时使用，
保留草稿；日志确认层打开时返回该层的焦点。
当前官方入口的 HTML、
`asset-manifest.json` 与清单声明的 27 个 JS/CSS 文件已保存到 `.ref/applications/feedback/`；
发现工具只读取路径、清单和响应字节，没有执行下载的 JavaScript。

页面主组件是当前 chunk `496.003ef6c6.chunk.js` 的模块 4496 `ps`。虽然组件定义了
`renderLeft`，实际 `render` 只挂载 `renderMainPage()` 和 `renderFooter()`；表单容器
宽度是 500px，页面顶部留白 30px，提交中/错误分支 `.form` 的最小高度是 372px，页脚高度是
60px。当前 CSS 中 textarea 的默认高度是 252px，存在 email 邻接表单时覆盖为 213px。
页面颜色、按钮和输入框沿用当前 Feedback CSS：背景 `#222`、正文 `#ccc`、强调绿
`#44d62c`、边框 `#5d5d5d`、橙色服务提示 `#fd8611`。

源码常量模块 4166 给出 email/subject 最大长度 255、description 最大长度 32000，
默认 form 高度 504；输入处理器另外把 subject 限制为 50 个 JavaScript UTF-16
长度单位。类别枚举实际只有 1（Feedback/Feature Request）、3（Customer Support）
和 4（Privacy/Security Enquiries）。类别 1 的 email 可选，类别 3 需要 email；类别
3 显示发送日志勾选（`REPORT_BUG_TAB` 未在当前枚举声明）。类别 4 的隐私内容由 CSS
绝对定位覆盖下层字段，`top:35px;bottom:0;z-index:1`；类别选择保留在上层且宽度为 50%。
当前 native 页面保留被覆盖字段的草稿实体，只渲染可见的隐私内容，以免键盘进入遮挡字段。
当前 native 页面只保留本地
draft、校验、下拉选择和字符计数；提交、日志导出、设备枚举、账户状态和联网查询
保留明确的未连接边界，外链沿用源码 URL。当前 root 并未挂载带日志下载按钮的
`renderLeft`，因此不添加那个历史/未挂载区域的按钮。

本轮按当前挂载 AST 与 CSS 修正了这些偏差：

- 字段顺序是类别/软件、email、device、subject、description、计数、日志勾选、submit。
  `synapse` 是真实软件标识；空类别和设备选择使用可见占位文案，占位不成为下拉选项。
- subject 超过 50 UTF-16 单位时拒绝整次编辑，不把新值截成前 50 单位；email 和 description
  分别以 255 和 32000 UTF-16 单位限制长度。字符计数同样使用 UTF-16。
- `gn` 的 ASCII 邮箱语法完整记录在 evidence：本地部分禁止首尾/连续点、域名与末级域名
  分别按源规则检查，末级域名首字母且至少两个字母数字。没有新增 Rust 依赖。
  `hn` 的空白检查采用 ECMAScript 集合，包含 BOM、不把 U+0085 当空白。
- 保留 guest 默认状态；email 的必填/可选提示随类别变更，email/subject/description 的 Blur
  都会触发源邮箱校验。视觉错误为 `#fd4949`，`errorEmail` 不额外覆盖 `enableSubmit` 条件。
- 隐私视图含全部三段说明，Privacy Policy 指向
  `https://www.razer.com/legal/customer-privacy-policy`，Contact us 指向
  `https://www.razer.com/privacy-enquiries`。不再在隐私说明下暴露普通字段和提交按钮。
- Customer Support 未勾选日志时，提交先显示源码的三段日志说明和两个选择；任一选择只
  更新本地草稿与未连接提示。模态支持取消、Escape、焦点隔离，无日志收集/上传。
  确认内容由 GPUI Base `DialogPopup` 承载，其 `occlude` 阻止面板文字区域的点击落到
  关闭背景；遮罩关闭与焦点隔离仍归 Base `Dialog` 管理。
- 页脚恢复两行，并把本地化 `{{a}}…{{aEnd}}` 解析为行内链接；不会把模板标记展示给用户。
  字段边框由外层框承担 hover/focus，输入编辑仍由 GPUI Kit 保持。

`surface::select` 新增的 `placeholder` 是可选展示字段，只在没有选中值时使用；默认调用
保持原行为。它不进入 SelectState 数据和候选列表，也不改变确认/取消流程。

当前 `ps` 组件没有 `type:"file"` 的用户附件控件。`onSubmit` 把草稿交给 `Jn`，
后者仅在 `withLogs` 为真时调用 `qn` 收集日志并编码为 `zip_log`，再 POST 到
`/api/feedback/report/submit`。`qn` 对每个待加入条目先单独压缩，累计压缩长度若超过
52,428,800 字节（50 MiB）便跳过该条目；它不是用户附件大小校验，也不保证最终 ZIP
不超过 50 MiB。`Jn`、`qn` 的完整当前 AST 收据和这条边界已加入 evidence；原生页面
不收集这些日志、存储或设备数据，也不发送请求。

十种语言的 103 条专用文案由 loader 模块 3272 实际指向的 chunk 模块静态解析到
`FEEDBACK_SOURCE` 命名空间，收据见 [feedback-locale-source.json](feedback-locale-source.json)。
组件、CSS、常量、服务调用边界和本地路由的机器检查见
[`tools/audit-feedback-app.cjs`](../../tools/audit-feedback-app.cjs) 与
[`feedback-app-current-evidence.json`](feedback-app-current-evidence.json)。

这项接入完成的是当前可验证的表单外观和本地草稿交互，不把反馈提交、日志收集、设备
查询或服务器成功页伪造成已接通功能。

验证：`node tools/audit-feedback-app.cjs --check` 为 `problems=0`，`rustfmt` 已通过。
`cargo check --locked --all-targets` 由主线统一执行，结果以本轮主线报告为准。
没有运行应用、构建、测试、下载 JavaScript 或 DLL。日志确认已按源码的 `bottom:98px`
及 `520×175` 面板定位；Feedback 不附加宿主的本地状态栏，保持页底坐标一致。
原生富文本链接样式、面板点击与键盘焦点恢复仍须在获准运行后做
视觉/交互核验。静态片段检查不构成程序执行等价证明，也不能作为像素一致证明。
