# Wi-Fi 添加设备：Gamer Room / Key Light

本页记录 `src/shell/iot_popup.rs` 与原版 IoT 弹层的静态核对结果。真实扫描、网络状态、配网和安装服务尚未接入；默认入口显示状态未读取，设备、网络、扫描动画和成功页只能通过明确标注的界面预览进入。

## 来源和入口

| 来源 | 核对内容 |
| --- | --- |
| `.ref/frontend/static/js/IotPopupRoot.290be417.chunk.js`，模块 `28256` | `gt` 类型选择；`dt` Gamer Room 流程；`rt` Key Light 流程；`B` 手机二维码；`ze` 扫描选择；`Fe` / `Be` 列表分区；`it` 网络表单；`st` 网络错误；`Ze` 设备错误；`at` 成功页 |
| `.ref/frontend/static/js/App.eb32d7cd.chunk.js`，`z` | 外层 `iot-device-popup iot-device-popup__mt`、关闭按钮、嵌入 IoT 页；`focusIotDeviceTab` 后关闭弹层并定位产品页 |
| `.ref/frontend/static/css/55.a5b041a2.chunk.css` | 实际生效的弹层、表单、列表、按钮、二维码与成功页样式 |
| `.ref/frontend/static/css/IotPopupRoot.56b22315.chunk.css` | `NoGamerRoomDevice` / `NoKeyLightDevice` 的空列表样式 |
| `.ref/frontend/static/js/main.7897a4cf.js`，模块 `54693` | `M.kEL` 对应 `NO_DEVICE_FOUND`，用于一个分区为空而另一分区有设备的普通提示行 |

原版 `gt` 仅将 `iotPopupType=GAMER_ROOM_DEVICE` 作为直接入口，其余值初始进入 `GENERAL`。从通用入口进入 Gamer Room 时有类型返回按钮；从 Gamer Room 直接进入时不显示该按钮。Key Light 的准备页始终保留回到类型选择的按钮。

## 步骤与状态

| 页面 / 操作 | 原版行为与当前实现 |
| --- | --- |
| 类型选择 | 两张类型按钮分别进入 Gamer Room 或 Key Light 准备页；兼容列表是独立外链 |
| 准备页搜索 | 原版挂载 `ze` 后开始扫描；本实现真实入口进入「状态未读取」，显式预览入口进入扫描画面 |
| 扫描中 | 显示分区加载状态；刷新按钮禁用；返回到准备页 |
| Gamer Room 扫描完成 | 不展示新设备分区；即使已有网络设备，底部仍为「取消」和绿色刷新按钮 |
| Key Light 新旧设备都有 | 展示两个分区；底部为「返回」和普通刷新按钮 |
| Key Light 只有已有设备 | 已有分区显示设备、新设备分区显示 `NO_DEVICE_FOUND`；底部为「取消」和绿色刷新按钮 |
| Key Light 只有新设备 | 已有分区显示 `NO_DEVICE_FOUND`、新设备分区显示设备；底部为「返回」和普通刷新按钮 |
| 全列表为空 | 使用各类型专属空态；Gamer Room 多一个兼容列表外链；底部为「取消」和绿色刷新按钮 |
| 已有网络设备 | 点击示例设备进入已有设备成功预览；识别按钮仅说明预览操作，不发送指令 |
| 新 Key Light | 点击示例设备进入网络选择与密码表单 |
| 手机二维码返回 | `rt` / `dt` 传给 `B` 的目标始终是相应准备页；Key Light 从网络错误进入二维码后也返回准备页 |
| 配网返回 | 返回设备选择；销毁当前表单的密码、可见性及网络选择状态 |
| 密码错误 / 连接中 | 两者仍是同一个表单；重试时保留输入、清除错误并显示连接中；连接按钮禁用，返回仍可用 |
| 入网后未找到 / 无法添加 | 取消和再次扫描均返回设备选择 |
| 成功页添加更多 | 返回设备选择 |
| 自定义 / 查看进度 | 原版发送 `focusIotDeviceTab`；本实现仅显示预览提示，不创建或安装设备 |
| 关闭 / Escape | 下拉展开时先收起下拉；关闭外层时淡出并在 300 ms 后卸载，清空密码与网络选择并恢复之前的焦点；减少动态效果时立即完成 |

`ze` 的页脚条件是 `!isScanning && !newDeviceList.length`，不是「设备列表为空」。当前按这一条件恢复 Gamer Room 及只有已有 Key Light 时的取消按钮、117 px 宽度和绿色刷新样式。原版扫描页刷新按钮的文案直接使用 `M.uIN`（网络设备标题），设备错误页才使用 `M.Z$Z`（再次扫描）；保留这个调用差异。

密码、密码可见性和网络索引在原版 `it` 中都是 `useState`。退出 `CHOOSE_NETWORK` 会卸载表单，因此当前在退出时清空密码、恢复隐藏、选择第一个示例网络；新选择预览场景时也统一重置。表单内部从就绪到连接中不执行此重置。

原版 `st` 只读取 `goToQRCode`，但 Gamer Room 的 `dt` 传入的是 `goToQRCodeSHDevice`。因此 Gamer Room 网络错误页的手机按钮没有可调用目标，当前保留这个原始行为。Wi-Fi 未启用时的 Windows 设置文字在原版也未绑定处理函数，不自行启动系统设置。

## 关键几何与样式

尺寸按原 CSS 的 16 px 基准换算为 `surface::css`；一像素边框仍由框架的边框样式处理。

| 对象 | 原 CSS / 当前约束 |
| --- | --- |
| 外层弹层 | 顶部 106 px；默认最大宽 850 px；设备宽度至少 1331 px 时最大宽 1280 px；限制在当前窗口内 |
| 标题栏 | 高 36 px，底部分隔线，右侧 36 px 关闭按钮及 20 px 图标 |
| 类型卡片 | 250 × 150 px；顶部 46 px、底部 20 px；两卡间隔 20 px |
| Gamer Room 准备页 | 宽 520 px；主图 56 px；手机入口宽 420 px |
| Key Light 准备页 | Wi-Fi 图标 48 × 36 px；手机入口宽 486 px、顶部间距 60 px |
| 手机二维码 | App 图标 32 px；二维码 110 × 110 px |
| 扫描页 | 宽 600 px；分区和行宽 420 px；设备选择 375 × 40 px；识别按钮 40 × 40 px；间隔 5 px |
| 扫描骨架 | 原 `ee` 始终保留 420 × 130 px 的 SVG 布局；裁成一行时仍占 130 px，当前同样保留高度 |
| 普通命令按钮 | 原 `.customize-setting-button` 高 27 px、最小宽 90 px、文字 12 px |
| 刷新按钮 | 原 `.icon-btn` 高 27 px、横向内边距 12 px；19 px 图标在文字左侧，间隔 7 px；普通状态采用背景色与正文色边框；主状态为绿色 |
| 网络选择 | 表单宽 300 px，选择器宽 285 px；密码框宽 279 px、高 23 px；连接提示区保留 104 px 高度；按钮组上间距 47 px |
| 网络错误手机入口 | 520 × 60 px |
| 设备错误 / 成功 | 最小高度分别 367 px / 403 px；已有设备图 250 × 140 px，新设备成功图标 48 px |

按钮使用 GPUI Kit `Button` / `BaseButton` 语义和键盘行为。12 px 命令按钮使用 `.xsmall()`，14 px 手机入口使用 `.small()`：Kit 0.7 的内部文字容器会按组件尺寸设置字号，仅在外层设置 `text_size` 不足以覆盖内部文字。刷新按钮单独组合图标和文字，避免 `Button.label(...).child(icon)` 将图标排到右侧。

弹层新增的状态/预览选择栏属于复刻项目的显式预览入口，不是原版 IoT 内容。保留这一说明，避免把额外一行的高度误认为已与原版实测完全一致。

## 外层挂载、关闭与叠层复核（2026-10-02）

实际 Gamer Room 入口是 `9388 ze.handleOpenAddModel → setOpenIOTPopup(true, "GAMER_ROOM_DEVICE") → 55 commonReducer → App.K.componentDidUpdate → App.z`。`z` 的外层 Modal 以可变化的 `isMounted` 包含 iframe；iframe 内的 `IotPopupRoot gt` 则始终传 `isMounted:true`。因此，不能从内层的固定值推断关闭时立即销毁窗口。

`55` 模块 `82830` 在挂载后 100 ms 添加 `.show`，关闭时立即移除 `.show`，300 ms 后才卸载 DOM 和 iframe。CSS 的面板与遮罩透明度过渡均为 150 ms linear。当前分别保留挂载、显示和关闭中状态；延迟结束前保留模态遮罩与焦点，完成后才发送一次 `DismissEvent`。关闭会取消未完成的入场任务，重复关闭不重启退出计时。教程和营销内容在整个退出期间继续被遮挡；Gamer Room 在弹层卸载后恢复之前的教程步骤。

外层关闭按钮实际是 `.modal.iot-device-popup .close`，其资源为 `icon_close_white.8ab462b8.svg`（文件名为 white，真实路径填充为 `#ccc`）。本地复用字节一致的 `calibration-close.svg`；36×36 命中区、20×20 图标、透明底，hover 白色 `#1a` 透明度、active 黑色 `#1a` 透明度，即时切换。没有 `.btn-close` 的右上 4 px 圆角、100 ms 背景过渡或黑色 30% 按下态。为键盘路径保留可见焦点框。

Kit 0.7 的 Base Dialog 使用 `10 + layer` 绘制优先级，Base Popup 固定为 `POPUP_PRIORITY = 100`。GPUI 对同优先级按插入顺序稳定排序；当前外层 Dialog 使用 `POPUP_PRIORITY - 10`，并在页面弹出内容之后挂载，其自身下拉再从 Dialog 内部挂载。顺序是页面内容／弹出内容、模态遮罩与面板、模态内下拉。遮罩显式 `occlude` 并阻止滚轮传播；不以更高的无差别优先级盖住自身网络和场景下拉。焦点陷阱、Tab、Escape 及下拉的取消仍由 Kit 原语处理。

回归源码位于 [iot_popup_tests.rs](../../src/shell/iot_popup_tests.rs) 和 [gamer_room_tutorial_tests.rs](../../src/shell/gamer_room_tutorial_tests.rs)：覆盖已有 Popup 上方的遮罩、滚轮隔离、下拉可点击、两层 Escape、双向 Tab 限制、触发器焦点恢复、100/300 ms 生命周期、重复关闭以及教程步骤恢复。用例尚未执行，不能作为运行成功的证据；编译结果由主任务统一记录。

## 外链

| 用途 | 地址 |
| --- | --- |
| Gamer Room 设置帮助 | `https://mysupport.razer.com/app/answers/detail/a_id/5753` |
| Gamer Room 兼容设备 | `https://mysupport.razer.com/app/answers/detail/a_id/5895` |
| Key Light 设置帮助、两种二维码页的 FAQ | `https://mysupport.razer.com/app/answers/detail/a_id/5911` |
| Key Light 兼容设备 | `https://mysupport.razer.com/app/answers/detail/a_id/6194` |
| Gamer Room App | `https://rzr.to/gamer-room-app` |
| Razer Streaming App | `https://rzr.to/streaming-dl` |

## 验证范围和未接入能力

本轮仅做源码、CSS、资源和状态转移的静态核对，由主任务统一执行编译检查。没有运行应用、worker、IoT DLL、build 或 tests，也没有窗口截图或显示比例实测。

原版的 30 秒扫描、每 1500 毫秒网络轮询、MAC / SSID 去重、识别设备、写入 Wi-Fi 配置、真实配网结果、`iot_devices` 持久化和产品安装仍需真实服务契约。当前没有调用这些能力，没有计时后伪造扫描完成，也不会把预览成功写入真实设备列表。
