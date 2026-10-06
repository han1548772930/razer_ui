# 本地设备页面的直接打开能力（2026-10-06）

本轮先核查真实入口，再修改安装呈现。Dashboard 的 `can_focus` 原本就没有禁止普通下载/安装阶段，不能把它整个删除：该函数保护最低固件要求、正在加载预设、断电/待机、Xbox/PlayStation 模式、Mixer 初始化失败、固件更新和重启分支。

实际缺口有两处：应用选择器在 `ready=false` 时隐藏整个设备；Dashboard 的设备卡片虽然可点，图上的安装重试仍改走安装请求，安装 spinner、变淡和文案也继续盖住本地入口。

## 本地页面能力来自实际 renderer

`ProductWorkspace::has_local_page` 读取已经持有的原生工作区。Source 分支继续检查真实持有的 family、dock、accessory 和 supplement 实体，必须至少有一个已支持的非帮助页。Audio、AccessorySystem 和 Controls 再使用它们的实际逐页支持判断。

仅有注册导航不算本地页面；`FamilyBody::Pending`、只存在 Help 内容均不构成此能力。该方法只读，不创建 renderer、不修改产品状态，不声称所有已注册页都已完整复刻。

## 明确的安装阶段及保留的状态

允许本地入口绕过安装呈现的阶段只有：`waiting`、`downloading`、`installing`、`syncing`、`install_canceled`、`error`。此外必须同时满足：

- 真实本地 renderer 能力为真；
- 现有 Dashboard `can_focus(device)` 为真；
- 原始 `no_alive_sign` 不是 true。

Error 的含义已经回溯当前代码，而非按名称猜测。当前 Dashboard 模块 22534/z 把 `Dh.ERROR` 和 `Dh.INSTALL_CANCELED` 同时映射到 `INSTALLATION_FAILED`；当前应用选择器 Root 的 `_installer-error` 分支也明确派发 `deviceSetup/setupStatus: error`。Mixer 失败、固件最小版本和 noAliveSign 是独立字段，未与此安装错误合并。

`Updating`、`RestartRequired`、Unknown、Initializing、Unsupported 不属于这次覆写范围。固件、主机模式、断电/待机、预设加载、Mixer 初始化失败和设备恢复入口均未改成新的产品目标；重启仍使用既有重启通知分支，固件图标仍进入设备与模块页面。

## 实际修改

Dashboard 根据单独的 `open_without_installation` 展示能力去掉安装 spinner、安装重试、安装文案和非就绪图片淡化。原始 `ready` 仍仅由原始 setupStatus 比较得到；没有把它 OR 成 true，没有伪造配置名，也没有改变电池观测接纳、固件、WDL 或设备遥测字段。

应用选择器保留原始 ready 字段，同时允许符合上述能力的安装阶段设备显示；打开时仍按原产品 ID 与 container ID 选择同一个工作区，并重新确认它持有本地内容。host 也补齐了原本漏投影的真实 Mixer 初始化失败字段。

安装状态预览没有本地覆写能力：`PickerDevice::new` 默认 `open_without_installation=false`，已有独立推荐/下载/安装预览继续保留原始状态。原始设备快照和完整源状态 renderer 都保留，因此可以继续观察未获本地能力的原安装分支。

## 静态证据与限制

[可重复审计](local-device-entrypoints-current-evidence.json) 保存当前厂商 AST 的阶段表、错误派发、安装失败文案和原选择器 ready 条件，并检查本地入口连接、真实 renderer 判断及原始 runtime 条件是否仍存在。

```text
node tools/audit-local-device-entrypoints.cjs --check
node tools/audit-dashboard-card-state.cjs --check
```

仅进行了静态源解析、Rust 格式化和静态契约检查。没有运行应用、构建、测试、下载的 JavaScript 或 DLL；统一 `cargo check --locked --all-targets` 由主任务记录。没有进行运行时或逐像素验收。
