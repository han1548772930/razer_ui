# Macro 按键绑定：当前源码与本地实现

2026-10-04。本专项接通 Macro → Key Binds → 设备选择 → 产品输入视图 → 本地关联 → 绑定卡片与移除。
关联只保存在当前 `MacroPage` 会话，不代表录制、宏执行、服务发现或设备写入成功。

## 当前源码证据

- Macro 当前 `1700.a2780a35.chunk.js` 模块 **21700**：`U` 是绑定页，`M` 是设备选择弹层，`I` 是设备/绑定卡片，`E` 是产品 iframe。
- `U/M` 的设备来自 `commonReducer.validDevices`，由 `connectedDeviceInfo` 补充本地化名称；`allRZDevices` 参与产品 ID / dongle ID 解析。无当前宏时不显示绑定页，无设备时显示警告并禁用添加卡片。
- iframe 参数包括 `displayMode=macro`、`macro`、`containerId`、`deviceEditionInfo`、`serialNumber`。选中设备消失后回到选择；返回与关闭是不同动作。
- 182 当前 `main.db20a7c4.js`：**1368** 的 `groupList` 是实际 8 个物理输入；业务根 **HM → BM** 载入它并设置 macro 模式；**KP** 是鼠标图和连线。
- 182 当前 `MapMacroKey.639e3304.chunk.js` 模块 **4369** 是专用绑定表单，显示当前宏、设备、配置、播放方式和可选重复次数。列表来自当前主包 **9267** 的 `je/Ze`，滚轮排除 held/toggle；次数 1..99，默认 2。未生成虚假的 Sequence/Phased 宏。
- 当前 Macro 主 CSS、1700 CSS 和 182 主 CSS 的静态收据保留规则顺序与媒体条件；不依赖旧前端或旧宿主快照。

字节偏移、源片段、SHA-256、locale export、CSS、资源及目录统计见
[macro-bindings-current-evidence.json](macro-bindings-current-evidence.json)。生成器
[audit-macro-bindings.cjs](../../tools/audit-macro-bindings.cjs) 支持 `--check`。

## 已接入的行为与几何

- Shell 传入现有 `ProductWorkspace`。弹层只读取设备/配置，不调用配置激活、保存或设备写入。设备观察者与弹层订阅由持有者保存，重复打开会替换旧订阅。
- 本地目标由设备身份、profile ID、input ID、Standard/Hypershift 层共同标识，每个目标仅保留一个关联。Save 验证设备、配置与当前输入目录；新的本地关联可替换该目标的旧宏。取消不添加关联；删除宏或其父文件夹时清理相应关联。
- 设备消失返回选择；配置或输入失效清除编辑目标。选设备后优先选择其 active profile，缺失时取实际首项。配置下拉仅改变绑定目标。绑定卡片从当前实体读取名称，移除只删除本地关联。
- 绑定卡片 290×230、margin 15；保留 `.box` 相对 top 13 和 `.box-group` bottom margin 23。选择卡片 300×200。添加卡片为 2px 虚线边框、40×40 源图标。无设备警告 top margin 22、图标 20×17。
- 弹层背板 top 86，面板目标 top 106、宽 1020；视口不超过 1028 时宽 100%。标题 36 高、RazerF5 16/19，返回/关闭按钮 36×36、图标 20。进入动画使用独立 Presence 通道：透明度 100ms、位置 300ms。Base Dialog 提供焦点约束和 Escape 行为。
- 182 使用 770×340 原图、原连线锚点及底部循环按钮覆盖图。LeftClick 保持禁用；没有把更大的 DEFAULTPROFILE 映射数组当作物理按键。
- 653 根据实际 layout 读取目录和形状。其它已提取键盘复用静态形状及现有 `KeyRegion` 的曲线/圆形/矩形命中；没有形状的真实输入保留目录项。其它鼠标暂为原图加真实输入目录列表。
- 专用表单保存本地 playback/repeat 元数据，次数复用数字步进器。182 编辑面板使用 HM 的 `50% + 220px` 定位。新增颜色集中在产品 token 层。

## 资源与验证

新增 `synapse/macro/binding-more.svg`、`binding-close.svg`，已纳入资源准备脚本、总 manifest 和 Rust 嵌入表。
关闭 SVG 不在 Macro media manifest 中，但当前 1700 CSS 的 `url()` 明确引用它；按该确切 URL 从官方主机取回并做 XML 检查。
返回和底部光晕复用与当前 Macro 源逐字节相同的 Profiles 别名。四项均有 SHA-256 与 byte equality 收据。

已执行：

- `node tools/audit-macro-bindings.cjs` 与 `--check`；
- `.work/resource-env/Scripts/python.exe tools/prepare-resources.py`；
- `python tools/validate-resources.py`：1056 条资源、132 个 Webpack 请求、73 个产品图片变体、58 个 Dashboard 图片身份、16 个键盘布局/1901 个形状通过静态校验；
- 集成代理执行 `cargo check --locked --all-targets`，包括播放方式和重复次数表单，无警告通过。

未运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL；没有实际窗口交互或像素验收结果。

## 仍未完成

输入目录覆盖不能等同于全部 **174 个 macro 产品根**的完整复刻。本专项细查集中在 182 根和 Macro 应用；653 与其它键盘复用已提取布局，其余鼠标为目录形式，尚未逐产品重建原图、侧面/层切换、独有禁用策略及全部播放限制。目录统计或编译通过不能作为全量完成证据。

设备写入、宏执行/录制、关联持久化与恢复、完整服务 GUID/设备同步、Sequence/Phased 数据、其它产品特有播放限制与精确弹层几何仍缺。配置菜单/电量栏、更多菜单关闭过程及全部 hover 动画仍需继续校对。当前会话提示是本地能力边界，不是官方 UI 成功反馈。
