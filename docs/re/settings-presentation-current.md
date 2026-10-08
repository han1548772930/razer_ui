# Settings 当前页面呈现契约

本项只核对独立 `/settings/` 窗口的 Software 标题、Systray 空预览与标签、General 的 About 链接及窗口基础字体。来源为当前 `.ref/applications/settings/`；不与 `/synapse/settings/` 页混用，也不代表整个 Settings 已完成。

源码、SHA-256、字符偏移、原始函数与 CSS 规则见 [精确证据](settings-presentation-current-evidence.json)。维护工具为 `node tools/audit-settings-presentation-current.cjs --check`，仅使用 Acorn/CSS 静态解析，不加载官方 JavaScript。

| 部位 | 当前官方依据与实现 |
| --- | --- |
| 窗口正文 | 主 CSS `body` 的 `line-height:1.22` 保留；根视图加载的 97 CSS `body,html` 设置 Roboto、16px。各 `.body-widgets .widget` 自身仍为 14px。对应 `SettingsWindow::render`，使用相对行高以继承到不同字号 |
| Software 标题 | `9302/ee` 挂载 `.installed-software .title-bar > .title`；921 CSS 设置 RazerF5、22px、300、绿色、大写及 `flex:1 0 auto`。本地补齐这些样式，标题占剩余空间，不再使用普通 14px 文本 |
| Software 自动更新 | 同一 `ee` 的 `.installed-action.flex > .action.disabled`，容器垂直居中，动作 14px、下划线、`#707070`、左右 margin10、透明度 .3。保留源禁用语义，不提供假的更新操作 |
| 空启动器预览 | `9762/x` 始终挂载 `.apps`，包括实际过滤结果 `L=[]`。762 CSS 设置 360×60、背景 `#111`、1px `#707070` 边框和下间距20。本地不再使用20px空白替代它，不填充未观察到的安装应用 |
| 输入标签 | `9762/x` 的 Preview/Order、`9762/fe` 的 Systray Icon、`3414/u` 的 Language 使用 `.settings .input-label`，CSS 明确700、大写及下间距10。复用局部 `input_label`；下拉实体仍负责实际选择与焦点 |
| Systray 描述 | `fe` 的 `.desc` 未指定灰色，沿父视图继承正文颜色；移除本地额外灰色覆盖 |
| About 法律链接 | `3414/be` 的四个外链之间明确挂载三个 `|`；414 CSS 给分隔符左右 margin6、灰色，外链默认箭头。本地补节点，保持现有 Link 的外部打开与键盘行为 |
| About 社交标题 | `be` 给 Connect With Us 指定 `textTransform:"uppercase"`，本地执行相同大小写转换 |

所有颜色使用现有主题或颜色 token，尺寸使用项目 `surface::css`。没有改第三方依赖、添加 vendor 或重写控件交互。

允许的检查为资源/证据静态校验、格式检查及由主任务统一执行的 `cargo check --locked --all-targets`。窗口、实际字形、缩放、焦点、命中、像素与动画尚未运行验收。安装应用与 Widgets 目录及 Launcher 选择仍缺真实服务发布者，不因本项样式修复认定这些功能已完成。
