# Feedback 应用当前源码审计

2026-10-04。入口来自当前 `rz-app-menu` 模块表：窗口名为
`feedback-synapse`，地址为 `/feedback/?app=synapse&path=%2Fsynapse%2Fdashboard`，
打开参数是 `policy=3`、`shouldFocus=1`、`tab_visible=1`。当前官方入口的 HTML、
`asset-manifest.json` 与清单声明的 27 个 JS/CSS 文件已保存到 `.ref/applications/feedback/`；
发现工具只读取路径、清单和响应字节，没有执行下载的 JavaScript。

页面主组件是当前 chunk `496.003ef6c6.chunk.js` 的模块 4496 `ps`。虽然组件定义了
`renderLeft`，实际 `render` 只挂载 `renderMainPage()` 和 `renderFooter()`；表单容器
宽度是 500px，页面顶部留白 30px，正文右栏的表单最小高度是 372px，页脚高度是
60px。当前 CSS 中 textarea 的默认高度是 252px，存在 email 邻接表单时覆盖为 213px。
页面颜色、按钮和输入框沿用当前 Feedback CSS：背景 `#222`、正文 `#ccc`、强调绿
`#44d62c`、边框 `#5d5d5d`、橙色服务提示 `#fd8611`。

源码常量模块 4166 给出 email/subject 最大长度 255、description 最大长度 32000，
默认 form 高度 504；输入处理器另外把 subject 限制为 50 个 JavaScript UTF-16
长度单位。类别枚举实际只有 1（Feedback/Feature Request）、3（Customer Support）
和 4（Privacy/Security Enquiries）。类别 1 的 email 可选，类别 3 需要 email；类别
3 显示发送日志勾选（`REPORT_BUG_TAB` 未在当前枚举声明），类别 4 显示隐私说明和外链。当前 native 页面只保留本地
draft、校验、下拉选择和字符计数；提交、日志导出、设备枚举、账户状态和联网查询
保留明确的未连接边界，外链沿用源码 URL。

十种语言的 103 条专用文案由 loader 模块 3272 实际指向的 chunk 模块静态解析到
`FEEDBACK_SOURCE` 命名空间，收据见 [feedback-locale-source.json](feedback-locale-source.json)。
组件、CSS、常量和本地路由的机器检查见
[`tools/audit-feedback-app.cjs`](../../tools/audit-feedback-app.cjs) 与
[`feedback-app-current-evidence.json`](feedback-app-current-evidence.json)。

这项接入完成的是当前可验证的表单外观和本地草稿交互，不把反馈提交、日志收集、设备
查询或服务器成功页伪造成已接通功能。
