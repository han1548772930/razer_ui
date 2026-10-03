# Philips Hue 当前源码审计

2026-10-03。本页状态为 **partial_native**，不表示完整视觉等价或已连接真实 Hue 服务。

## 来源与实际入口

- 当前产品 769：`.ref/devices/769/static/js/main.ad1113f8.js`，SHA-256 `3d1660626594af99c54c622b2e1caf3190e172e8703392a65286e81b9a3fff9d`。
- 样式：`.ref/devices/769/static/css/main.341edb83.css`；清单、CSS 哈希、组件范围与默认值见 [结构化证据](hue-current-evidence.json)。
- 仅用 Acorn 解析下载源码；没有执行下载的 JavaScript。`tools/extract-hue.cjs --check` 可核对生成数据。
- 导航条 `l` 的组件分支与真正入口 `qA` 相反。实际 `qA` 在 `isPaired=true` 时挂载 `pA`，否则挂载 `Fi`，本实现按实际入口分派。
- `Xo` 没有收到 `renderProfileBar`，因此 Hue 不显示通用设备配置文件下拉框。

## 已挂载的原生内容

`src/features/hue.rs` 保留状态与控件实体；`hue/onboarding.rs`、`bridge.rs`、`brightness.rs`、`effects.rs` 分别拥有对应界面，外层 `SourceProductWorkspace` 只负责路由与本地草稿。

- 连接引导：标题、说明、扫描、扫描中、手动 IP、找到网桥、等待 Push-Link、失败与重试等待界面。
- 已配对工作区：两个 600px 栏，网桥、灯效、亮度和设备四块内容；保留共享的 1280px 栏切换规则。
- 网桥：开关、娱乐区选择、刷新与删除入口、关闭和占用状态、接管控制与移除确认提示。
- 亮度：总亮度开关、全局/逐灯方式、0–100 控件、设备离线禁用与计数。没有设备时计数为零，不使用源码 reducer 中的种子计数 `1/1/1`。
- 快速灯效顺序来自当前 `QUICK_EFFECTS`：环境感知 11、音频计 12、呼吸 2、光谱循环 3、静态 1。已接入屏幕区域、色彩增强、双色/随机颜色和单色控件。光谱循环没有额外参数。
- 高级灯效：Chroma 未安装图文和已安装但无配置的状态；安装、启动、跨设备同步服务未接入，相关命令不可用。
- 自定义文案来自 `_i` 的十种语言，回退由模块 37 的 `Nm` 导出确认是 `en`。共有文案通过当前模块 4693 的导出核对。
- 提取 48 个灯具 archetype 路径、网桥/断开等 SVG、精确刷新/删除路径和安装图，共 62 个资源。资源来源、请求 URL 和输出 SHA-256 见 `assets/synapse/hue-manifest.json`。原 AVIF 静态转换为 PNG。
- 当前 Hue 的 40 个预设颜色与 `no-color` 顺序已和共享原生颜色选择器核对一致；这不替代整个颜色编辑器的视觉验收。

## 保留的源码行为

- `Pi` 输入初始是四个空值，不是 reducer 中的 `192.168.50.158`。`parseInt` 会移除前导零、解析数字前缀并把大于 255 的值截到 255；文本框的 HTML `pattern` 不阻止负号等输入。本实现保留这个实际行为，而非擅自改为严格 IPv4 校验。
- IP 键盘行为来自 `onKeyUp`：空值 Backspace 退到前一段，三字符值前进到后一段，首尾留在有效段内。
- 配对按钮请求 `WAIT_USER_CLICK_PAIR`，但 `Bi` 没有对应内容；必须等待服务提供 `PAIRING`。没有添加自动跳转。
- `vi` 的“重试”和“从头开始”按钮没有绑定回调。失败预览保留正常外观；`RETRY_PAIR` 预览显示禁用外观。
- `isGlobalBrightness` 缺失时视觉上默认选中；源回调否定原字段，所以第一次点击会写入 `true`，第二次才切换逐灯方式。
- `NA` 接收 `handleSelectDevice`，但实际 JSX 没有使用它。没有添加源码中不存在的勾选框。原生控件按灯具容器 ID 和区域 ID 保持身份，不使用位置序号。
- 灯具按 `deviceContainerId` 去重；通道数取 `name` 的去重数量。离线判断和连接计数来自灯具 `isOn`。

## 状态与当前限制

真实入口初始未配对，扫描按钮因缺少 Hue 通信适配而不可用。没有伪造已发现网桥、扫描结果、配对成功或移除成功。开发设置中的“预览 Philips Hue…”使用独立实体，可选择 18 种状态；示例灯具和娱乐区明确使用 Sample 名称，不写入设备或配置文件。

网桥状态、IP、分组、灯具列表、Chroma 安装状态和全局高级灯效开关不会写入配置文件。支持的本地草稿字段为 `brightness`、`quickEffects` 和 `ports`。预览操作保留源码的本地请求状态，刷新/配对/移除不会用计时器模拟服务回应。

仍未完成：Hue 网络发现和服务消息往返；实际灯具输出；Chroma 安装/启动/配置列表/同步；首次高级灯效提示及持久化；扫描和配对动画；滑块、步进器、提示框与禁用状态的全部原样式细节。普通滑块暂使用 GPUI 组件，尚未还原源 64px 容器、数值气泡位置与全部指针状态。颜色编辑器复用现有实现，动态语言切换和所有弹层的真实窗口行为仍需验收。

## 允许的验证

已通过 `cargo check --locked --all-targets`，包括新增契约测试的编译；测试未执行。静态校验覆盖来源哈希、导出文案、五种灯效、62 个资源和实际路由；格式与资源总表另外验证。没有运行应用、构建、测试、安装器或 DLL，也没有实际窗口截图、焦点/滚动与设备往返验证结果。编译通过不作为 UI 完成证明。

复查命令：

```text
node tools/extract-hue.cjs --check
.work/resource-env/Scripts/python.exe -X utf8 tools/prepare-hue.py
python -X utf8 tools/validate-hue.py
python -X utf8 tools/audit-native-product-coverage.py
```
