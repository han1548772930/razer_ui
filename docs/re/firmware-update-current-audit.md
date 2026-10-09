# 当前固件更新界面复核

复核日期：2026-10-03。对象为当前生产入口 `/synapse/update-fw/`，不是 Dashboard 内嵌的一段推测流程。本文重新解析了当前 JS 的声明、状态机、组件与 CSS；符号和行为均来自下列文件。

## 版本与证据边界

- 本地目录：`.ref/applications/synapse/update-fw/`。
- `index.html` 的 `noscript#version` 与 `manifest.json`：`version: 1.0.0`、`buildVersion: 2604010417`。
- HTML 内提交：`ac15b1efd731968040266c4bcf243825a8e1505b`；该提交同时出现在 main JS 的 `SENTRY_RELEASE`。
- 2026-10-02 独立 HTTP 下载记录：`.work/latest-source-check/firmware-update-live/report.json`，包含请求地址、HTTP Date、ETag 和 SHA-256。记录中 5 项均与当前 `.ref` 相同；不能将这一日期说成 2026-10-03 的在线复核。

| 文件 | SHA-256 |
| --- | --- |
| `index.html` | `2da7c0bc38f762dc382cc5aaf0b94f35853586b939733fe2d80756ed82732349` |
| `manifest.json` | `f2622cfdd2542285e8e2717f0349352474dbcf4c6940b02662f1f2431ccee6f2` |
| `asset-manifest.json` | `b9f4c0cf32d27f504e561f764c78e6a04130498df5c0daf0297e1ff8cc984911` |
| `static/js/main.69cc5fbd.js` | `af6f716bd8e37774bac6d5a18d9f8f314ed3865d74c55740761452610543f772` |
| `static/css/main.3e3077d6.css` | `652c485ac88a166e3f16f17754173253a15ba00f1a2609f32c1e4b8c45505a50` |

原包只作静态输入，未执行其中的 JavaScript、DLL 或安装程序。没有运行应用、构建或测试。GPUI 原生交互能力与 Razer 原版行为在下文分开说明。

## 当前原代码定位

下面是对上述 main JS 用 Acorn 解析得到的声明区间，区间采用 JavaScript 字符串的零起点 UTF-16 偏移 `[start, end)`。同名短符号可能在别的模块重复，不能只搜索一个字母就认为找到了依据。

| 声明 | 区间 | 本次核对内容 |
| --- | --- | --- |
| `Ee` | `1768256..1768692` | 所选语言 → 英语 → 原始 key；模板参数替换 |
| `da` | `1823125..1823669` | 5 种阻断提示的标题、正文、按钮数量 |
| `Nr` / `Ur` | `1858189..1860289` | 产品图片与 edition/layout 回退 |
| `to` | `1885615..1885693` | 当前项与总项数条件 |
| `no` | `1885694..1885791` | `resetIndex` 只重设 `currentItem:1` |
| `ao` | `1885792..1890714` | XState 状态机、服务结果与 `previousUpgradeRes` |
| `lo` / `uo` | `1892243..1893009` | 欢迎页、笔记本供电提示、下一步 |
| `ho` / `mo` | `1893026..1893850` | 进度标题、0 值 disabled、进度宽度变更 |
| `Do` / `fo` | `1895212..1896492` | Prepare / ReadyToUpgrade / Upgrade 的按钮状态 |
| `Uo` / `Ro` | `1897546..1899124` | 部分完成、无需更新文案分支、下一连接提示 |
| `$o` / `Po` | `1899542..1902459` | USB/接收器切换、匹配设备计数、接收器图示 |
| `Fo` / `Ho` | `1902590..1904840` | 全部完成列表与无线连接提示 |
| `Vo` / `xo` | `1904997..1907823` | 已完成/未完成分组与支持条件 |
| `Wo` / `Ko` | `1907857..1908963` | 结果页、Start Over 与 Close |
| `Zo` / `ei` | `1909981..1911985` | 阻断提示按钮、原生 button 的 disabled 状态 |
| `ri` | `1912154..1914718` | 阶段渲染与告警继续/取消处理 |

辅助证据：`zo` 是 `.firmware-upgrade-modal-overlay` / `.firmware-upgrade-modal-content` 的容器；紧接其后的 `Yo` 是 20 × 20 实心告警 SVG。`Jr` 为准备服务，`Qr` 为安装服务，`Zr` 为检测服务，`Xr` 检查 `firmware_upgrade_active_device`。这些服务仍未接入 Rust。

## 阶段与交互

`ao` 的正文阶段为 `Launch`、`Prepare`、`ReadyToUpgrade`、`Upgrade`、`PartialCompleteUpgrade`、`SwitchDeviceMode`、`CompleteDone`、`CompleteSkipOrFail`，另有执行退出服务的 `Close`、`Exit`。

| 原版条件 | 原版结果 | 当前原生界面 |
| --- | --- | --- |
| Launch 的 Next | Prepare | 显式预览可进入准备页 |
| Prepare 成功 | ReadyToUpgrade；若 `skipUpgrade` 则部分/全部完成 | 展示对应阶段；预览回调不代表真实服务结果 |
| ReadyToUpgrade 的 Update | 先经 `Xr` 检查其他更新设备，再发 Next 进入 Upgrade | 按钮与等待提示可预览；未实现实际互斥服务 |
| Upgrade 期间 | Update 与 Skip 不可操作；服务结果决定完成或失败 | 对应禁用状态、失败预览与关闭告警 |
| 部分完成的 Next | 当前项加一，进入 SwitchDeviceMode | 进入另一连接的图文说明 |
| SwitchDeviceMode | 连接已改变且只匹配 1 个设备时允许 Update | 只以明确标为预览的连接事件改变状态 |
| Skip 或失败 | CompleteSkipOrFail，保留此前成功记录 | 已完成/未完成分组、Start Over |
| Start Over | `no.resetIndex`，保留 `previousUpgradeRes` | 保留示例成功项；拒绝旧 generation 的回调 |

切换界面的 `$o` 明确计算匹配设备数量并要求 `1===e`。匹配来源包括相同序列号，以及原码特定的关联 PID / 接收器空序列号条件。当前 Rust 预览只有“0 / 1 / 多个”的显式样本，不是这套硬件发现算法的替代实现。

原代码的 `Aa` 将 PID 692 映射为大接收器、176 映射为小接收器，其余回退小接收器。本地新增“鼠标：USB → 大接收器”预设，让原有的大接收器图示可被实际选到；该预设不推断接入设备的 PID 或能力。

有一个必须保留的原版细节：`ao.handlePrepareProcessResult` 将 `skipUpgrade` 写成 `SUCCEED`，`Uo` 见到这一记录会显示 `UPDATE_SUCCESSFUL`，不会因为跳过写入就自动显示 `NO_UPDATE_REQUIRED`。重试后的预览已跟随这个判断。“原界面‘无需更新’分支”只用于直接检查 `Uo` 中另一个文案分支，不能当作已证明的实际设备路径。

这个分支还有一处原代码比较：`connectType.toLocaleUpperCase() !== bn.DONGLE`，其中 `bn.DONGLE` 是小写 `"dongle"`。因此 USB / dongle 都进入设备类别文案分支；本地也不因产品常识擅自改成 `DONGLE_IS_LATEST`。该 key 虽在源里存在，不等于当前分支实际会选中它。

## 阻断提示、关闭和焦点

`da` 中短常量的文本定义可在 main JS `1759108` 附近核对：`c="RETRY"`、`A="UPDATE_IN_PROGRESS"`、`D="DEVICE_IS_DISCONNECTED"`、`f="CANCEL_UPDATE"`、`C="UPDATE_FAILED"`、`T="UPDATE_FAILED_DETAIL"`。

| 类型 | 标题 | 正文 key | 按钮 |
| --- | --- | --- | --- |
| WAIT_FOR_OTHER_DEVICES | 无 | WAIT_FOR_OTHER_DEVICES | 无 |
| DEVICE_IS_UPGRADING | UPDATE_IN_PROGRESS | DEVICE_IS_UPGRADING | CONTINUE_WITH_UPDATE |
| DEVICE_IS_DISCONNECTED | DEVICE_IS_DISCONNECTED | DEVICE_IS_DISCONNECTED_DETAIL | CANCEL_UPDATE、RETRY |
| ISOLATE_DEVICE_FAIL | UPDATE_FAILED | UPDATE_FAILED_DETAIL | CANCEL_UPDATE、RETRY |
| UPDATE_FAILED | UPDATE_FAILED | UPDATE_FAILED_DETAIL | CANCEL_UPDATE、RETRY |

`ri` 的取消处理清除 error 并发送 `Close`；继续处理只是清除 error、设置 `ignoreError: true`。因此按钮虽然写着 Retry，它并不直接重启升级；真正的重新开始属于结果页 Start Over。Rust 保留这个区别。

`zo` / `Zo` 没有遮罩点击、Escape 或默认 Enter 的处理。原生实现用 GPUI Kit `Dialog` 捕获焦点，明确拒绝遮罩、Escape 和空白处 Enter 的默认关闭；实际按钮仍通过 GPUI `Button` 获得键盘操作。焦点捕获、焦点可见边框与恢复是原生适配，不宣称是原网页已实现的能力。预览失败会移除其触发按钮，因此关闭提示后恢复到保留的页面焦点；其他提示保留原焦点。

`allow_close` 在本地 Upgrade 阶段显示单按钮继续提示。预览完成后清除描述已结束操作的提示。原版 host/SDK 拒绝退出消息与服务生命周期尚未连接，不能把这套本地门控当作真实固件写入保护已经验证。

## 本次按原 CSS 修正的可见差异

CSS 的 `.product`、`.header`、`.progress_box`、`.version`、`.switch-connection`、`.result` 与 `.firmware-upgrade-modal-content` 均已直接复查。

- 部分完成的 `.tutorial_more_detial` 现在放在 `.header` 内，保留原来的 `margin-top:10px`、`padding-bottom:116px` 与每项 `margin-bottom:12px`，消除此前把 header 底部 20px 留白插到详情前面的偏移。
- 全部完成列表使用 `Fo` 的**当前连接**顺序；不再按开始升级的连接排序。失败/部分完成分组沿用 `Vo` 的设备、接收器顺序。
- 无线提示按 `Fo` / `Vo` 的当前 USB 连接条件显示。原先误用了初始连接，USB → 接收器和接收器 → USB 两条路径会显示相反的提示。
- 告警改用当前 `Yo` 的实心 SVG，移除 Lucide 近似图形。`tools/prepare-resources.py` 只解析该声明的字面量 path / fill；生成 `assets/synapse/firmware-warning.svg` 并记录来源 SHA。
- `.cancel-btn:hover` 为 `#555`，`.continue-btn:hover` 为 `#00e600`；改为对应 `FirmwareColors` token，不再复用普通按钮的 opacity hover。
- 取消按钮大写；继续按钮保留译文大小写，因为只有 `.cancel-btn` 定义 `text-transform:uppercase`。继续按钮前景为原 CSS 的黑色。
- 等待其他设备提示按原 CSS 使用 440 × 74、无额外 padding；其余提示宽 480、最小高 162、padding 22px 34px。标题行高为 20px，正文行高为 21px。

源码通过 `surface::css` 映射原 CSS 尺寸，颜色留在语义 token 层。原生滚动、最小内容空间与焦点边框属于宿主适配。没有运行窗口进行像素测量，不能声称所有尺寸/字体已通过实际截图比对。

## 语言来源

`tools/extract-firmware-locales.cjs` 通过 Acorn 静态读取 `asset-manifest.json` 列出的 10 份 locale bundle，只接受模块内字符串字面量、别名和显式导出 getter。输出与 SHA 见 `docs/re/firmware-locale-source.json`。

当前原版缺项也必须保留：

- `DEVICE_IS_UPGRADING` 在全部 10 语言中都没有导出，`Ee` 最终返回这个未加命名空间的原始 key。
- `SWITCH_MOUSE_CONNECT_TO_DONGLE`、`SWITCH_KEYBOARD_CONNECT_TO_DONGLE` 只有英语导出，其余 9 种语言回退英语。
- 英语提取 46 项，其他语言各 44 项；47 个被检查 key 中的缺项按 receipt 记录，不补造翻译。

`text()` 使用 `i18n::t_or("FIRMWARE_SOURCE.<key>", key)`，避免把 Rust 内部命名空间泄露给用户。

## 仍未完成的真实能力

默认状态是本地增加的 `Unknown`。`ri` 在未知状态原本返回空内容；Rust 明确说明尚未连接固件服务，并禁用更新。只有 `preview_enabled` 的专门入口能选择示例阶段。设备名、版本、进度和连接数标为本地预览；没有将快照中的普通设备身份提升为固件能力证据。

预览每 450ms 增加 10% 与等待示例 3 秒后消失，均为本地演示控制，**不是** `Jr`、`Qr` 或后台固件工作的时序。任务被 entity 持有，切换场景取消任务，generation / 阶段 / component 联合校验拒绝旧回调。预览完成不改真实 Device，不写版本、不写固件、不写 host storage。

尚未接入：真实 `firmwareInfo` / `firmwareUpdateInfo`、版本与支持连接条件、设备隔离/恢复、发现与连接模式变化、活动升级互斥、SDK/二进制下载与写入、真实进度/错误/取消/退出回调。`Close` / `Exit` 当前只关闭本地页面，不模拟服务清理成功。不能据此宣称固件功能或全部界面已经完成。

## 验证记录

- `node tools/extract-firmware-locales.cjs --check`：10 语言与来源 receipt 一致。
- 当前 main JS 已通过 Acorn 静态解析，上表依据实际声明重新定位。
- `rustfmt --edition 2024 crates/razer-app-pages/src/firmware_update.rs`：完成，涵盖 state 子模块。
- 资源维护脚本统一准备与验证：432 条来源/输出 SHA 及嵌入 key 通过，包含新告警 SVG。
- 增补最终连接与结果顺序的纯状态回归断言；没有执行测试。`cargo check --locked --all-targets` 由主线程统一记录，本文不提前宣称通过。
