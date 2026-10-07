# 全范围复查后续批次：2026-10-07

接续 [前批记录](ui-review-continuation-2026-10-07.md)。用户要求复查所有已写页面与产品，三个子任务本批分别推进公共页面、Studio/输入组及手柄/显示器组；父任务处理 Profiles、跨工作区生命周期与统一检查。全范围仍为 **331 产品、1419 主导航页、33 独立模式页、24 应用入口**，不是把本批几个界面当作全部范围。

## 实际接入与修复

| 范围 | 本批结果 | 尚未完成 |
| --- | --- | --- |
| [Chroma Settings](chroma-settings-current-audit.md) | 当前宿主同窗策略下的 `settings-chroma` 页签；Chroma/General 两页，启动/通知本地草稿、教程重置、WDL 条件、语言/About；迁移及发行说明保持 Chroma 身份；本地保存失败可重试或放弃 | 完整宿主状态发布者、部分教程消费者、账号/更新工具栏、发行说明实际数据及视觉验收 |
| [Studio Reactive/Ripple/Starlight](studio-reactive-ripple-starlight-native.md) | 三种实际属性根、各自色彩/渐变/随机/时长及数值/Playback；保留黑色、1/7 默认渐变限制、跨效果瞬态清理；小数/指数临时值和失焦取整按当前源修正 | 真实设备选择/混合值/区域应用、共享弹层和颜色过渡细节、运行输入与视觉 |
| [Profiles 传输](profiles-transfer-native.md) | Import/Export 弹层、本机选文件、校验及预览、逐配置与逐宏选择、警告提示、取消/过期任务处理；保留逐行宏选择，不再只存跨行并集 | 真正导入应用到本地配置、宏集合导入、官方文件输出、完整 DEFAULTPROFILE 规范化及 Synapse 3 迁移；目前仅保留可撤销本地请求 |
| [2636 校准](gamepad-2636-calibration-native.md) | 左/右选择、五步指导、原图/三圈轨迹、真实错误重试、Done 条件；edition 与缩放；切 Help、离开产品或恢复配置使旧会话失效 | 真实轴/步骤读取发布者、窗口/设备验收；没有设备校准写入 |
| [Raptor 3858/3880 六页](monitor-pages-review-2026-10-07.md) | Gaming/Color/Display 逐组件事件复查；修永久禁用、路径选择、刷新率隐藏条件、FPS 错误门控、输入源图标与大小、系统入口失败反馈；观察与本地请求分离，restore 不再清掉限制状态 | 真实 monitor 发布者、若干父层透明度/加载遮罩/确认层边界、完整布局和视觉验收 |

父任务独立回读后补齐 `ProductWorkspace → SourceProductWorkspace → Gamepad/AccessorySystem` 的校准／显示器观察透传。校准有代际、部件和活动页面检查；显示器观察不进入配置快照。这些是供真实发布者接入的代码链，**不是已读取到设备数据**。

## 子任务扩展与下一队列

[Studio 剩余五根](studio-wave-wheel-pending-review-2026-10-07.md)已继续逐项回读 Wave/Wheel/Tidal/Audio/Generate，共 86 条 AST、102 条 CSS，保留五根未实现和 15 个源资源待准备。Wave 的 0–50 速度、暂停秒/毫秒、10–400 宽度、角度圆盘；Wheel 的 `params2`/中心点生产链；Tidal 哨兵、Audio 小数范围/专有渐变和 Generate 的媒体/历史均是后续具体 UI 工作，不能用共享控件存在替代。

系统/附件分组队列本批从 8 页局部审查扩大到 14 页，另外 311 个主页面和独立模式继续待审。音频、鼠标、键盘、其他手柄以及全部公共应用的先前队列继续有效。完整产品验收仍为 **0**；UI 编辑/保存与只读能力仍在当前范围，只有 DLL 修改设备／服务和持久化写回后置。

## 检查记录

本批执行静态源码解析、当前资源准备/校验、格式与 `cargo check --locked --all-targets`。最终编译通过（25.15 秒，bin 14 条、test target 7 条警告，主要为未接发布者的观察接口及既有未使用项）。本轮修复编译发现的控件类型/导入及发行说明焦点借用后重新检查；不沿用前批结果。全量格式与差异空白检查通过。

- Profiles：28 条 AST / 85 条 CSS / 5 张原 SVG；父内容审计 14 绑定 / 400 CSS / 14 图标。
- Chroma Settings：33 AST / 331 CSS / 10 locale / 25 SVG；注册和 XML 校验通过；独立当前线上文件对照详见子报告。
- Studio 新三根：62 AST / 218 CSS / 8 资源；全属性 13 根/40 收据/19 CSS 文件及现有 Studio 45 收据、145 个网格矩形校验通过。
- 2636：13 UI / 3 MW AST、23 资源；显示器：2 产品 / 6 页面 / 124 实际挂载收据 / 4 SVG。
- 四组新增资源合计 57 项，均已接入 AssetSource 的 load/list；新增 SVG 的 XML 和对应资源收据验证通过。
- 嵌入 JSON：61 文档，0 失败，4 项既有跳过。全范围队列及公共页面当前收据校验通过。

没有运行应用、测试、安装器、下载的 JavaScript 或 DLL；新增回归用例只进行编译检查。没有提交、重置或恢复用户删除的文件。
