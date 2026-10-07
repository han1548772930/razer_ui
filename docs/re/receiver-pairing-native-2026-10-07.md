# 179 配对工具：完整状态主体与真实观察边界

本批继续同日第一轮[接收器交互复核](receiver-interactions-verification-2026-10-07.md)。第一轮记录的“179 只有 LOADING 主体”已由本批补齐一批实际界面；不是仅添加路由或描述符。当前源仍为 `.ref/devices/179/static/js/main.4849f7ca.js`，SHA-256 `f5d6efe15f34167d61020469f151c3a0827f695cb35bc49cbdd32c23957fb874`。

## 挂载与本地实现

静态复核 `9473/mE → Te → G/se → re → ie/oe/ne/ae/_e`。源码 `W.UNPAIRING` 是用户确认界面，`W.JUSTUNPAIR` 才是发送解绑后的进度态；本地分别命名 `ConfirmUnpair`、`Unpairing`，避免把打开确认框当作设备正在解绑。

实现拆到 `receiver_pairing_state.rs` 与 `receiver_pairing_view.rs`，原 `receiver.rs` 保留接收器主体和 modal frame，加载以外的内容真实挂入同一个配对工具：

| 源状态/分支 | 已挂载内容 |
| --- | --- |
| LOADING | 原 20px 加载图；等待本次会话的真实 Bindings 观察 |
| LOADED | 键盘、鼠标两张 250×210 卡，20px 间距；77×60 原接收器图；固件版本未知时 Select 禁用 |
| SCANNING / UPGRADING / PAIRING / JUSTUNPAIR | 单张 510×210 卡、源 24px/1秒进度图、对应文案；仅由真实 publisher 的进度观察进入 |
| SCANNED 无候选 | NO_DEVICE_FOUND 与 Rescan |
| SCANNED 多候选 | 104px 可滚动候选列表、源单选项、数量/类别标题、Cancel/Pair；保存本地选择 |
| SCANNED 单候选 | 根据 `se` 发出该真实候选的 Bind 意图；未被真实 publisher 接收时明确标为未发送，不伪造配对进度/成功 |
| PAIRED | 观察到的设备名或 PID、可用的同身份原 Dashboard 图片、Unpair 入口 |
| UNPAIRING（本地确认） | 210px 橙色边框确认层、Cancel/Confirm；Cancel 仅撤销本地意图并保留真实绑定 |
| PAIR_FAILED / UNPAIR_FAILED | 原失败文字；失败与成功空列表分开保存 |
| UNPAIRED / 配对完成 | 只在真实成功观察后显示完成文案；没有定时器制造结果 |

保留当前源内容背景 `#222`、最小 850×685 内容区、790px introduction、600px设备区、连接虚线、类别图、五条注意事项、卡片绿色外框及确认层。连接虚线按原 SVG 的 `2 20` dash、22px offset、0.5秒线性循环实现；操作图按源 CSS 1秒旋转。减少动态效果时使用静态帧。

179 的产品名称键 `HYPERPOLLING_WIRELESS_DONGLE` 不在现有公共语言文件。本批没有把键名直接显示出来，而是静态解析 `5955/t5O` 的十个实际语言 namespace/getter，将本批 179 文案写入 `receiver_pairing_data.json`。配对文字优先读当前产品字典。

## 观察、意图与主线转发

`SourceControls → SourceProductWorkspace → ProductWorkspace` 转发 `ReceiverPairingEvent`。公开入口统一从 `crate::features` 导出，事件保留 `session()` 与 `intent()`：

- 打开工具发 `QueryBindings`；候选 Cancel 和读取错误的 Retry 也发 `QueryBindings`；关闭发 `Cancel`。
- Scan、Bind、Unbind 只创建本地意图，**不会**自行改为扫描中、配对中或成功。按钮的选择/确认/取消操作本地可用；意图留在本次会话，不进入 profile 草稿、local Save 或 DLL 持久化。
- `ReceiverPairingObservation::bindings/scanned/bound/unbound/failed/progress/firmware_version` 接收真实结果。关闭后以及旧 session 的观察全部拒绝。已撤销的意图不能再接受对应进度确认。
- `ReceiverPeer::queried(pid, status)` 只含实际硬件查询的两个字段。未知名称显示 `PID n`；不会补造 serial、edition、layout、category、固件或图片身份。status 为 0 的已关联条目仍是离线条目，不能当作在线成功。
- 设备图片仅按实际存在的 PID/edition/layout 三元组精确命中资源表；不使用通用 Dashboard helper 的 layout 归一/替代。缺少元数据或原图就保留图像区域，不能换用其他设备/版本图片。
- 未发送意图与真实读取失败分别显示本地说明。原 `se` 在读取错误后也进入 LOADED 双卡，本地另外保留失败标记和 Retry，避免这条呈现分支被误判为成功读取空列表。

主线程另行实现 shell 订阅与实际 179 只读查询。本子任务没有执行硬件查询，也没有为 Scan/Bind/Unbind/固件意图增加设备写回 consumer。固件本批仅保留非空 `currentFWVersion` 的 Select 门控和真实 Upgrading 观察的展示；80字节 IC 分类、固件资源/安装流程仍须独立集成，不能用当前 status/PID 查询补出这些字段。

按主线程要求，`DeviceWorkspace`、`SourceProductWorkspace` 的连接观察同步更新 `device` 与 `saved`，`ProductWorkspace` 转发两个 body；连接观察不会导致本地 profile dirty。模型字段与实际发布器由主线程单独审计。

## GPUI Kit test-support 与静态验证

真实生产路径的 modal、加载、内容、类别卡、候选列表/数量、进度、失败、完成、本地意图和确认层均有稳定 `receiver-*` ID 与 `.test_support()`；语义容器补 `role`/`aria_label`。Base Button/Base Radio 自身带框架 test support，并显式设置可访问名称、checked、disabled；各操作入口可从生产元素树定位。

保留六个状态机测试源码，覆盖：未发送意图不冒充进度、撤销后拒绝进度、旧/已关闭 session、取消解绑保留观察、单候选仅产生意图且保留未知 payload 字段、PID/status 不补造元数据，以及读取失败与真实空列表区别。**未运行测试**；用户最新要求的重点是 GPUI Kit test-support，不能将这些普通状态测试当作界面测试已经执行。

本批维护工具：

- `node tools/audit-receiver-pairing-current.cjs --check`：15个当前 AST 节点、115条 CSS、10份产品语言字典、9项资源和本地文件 SHA。
- `.work/resource-env/Scripts/python.exe tools/prepare-receiver-pairing.py`：独立获取 179 的9项 manifest 资源；8个 SVG 与既有 dock-164 资源逐字节相同后复用，`uma_pairing.d84abdc7.avif` 无缩放转 RGBA PNG。
- `node tools/audit-receiver-current.cjs --check`：接收器原主体、108条 CSS、10个指示灯 SVG 层。
- `rustfmt --edition 2024 --config skip_children=true`：本批 Rust 文件。

收据：[receiver-pairing-current-evidence.json](receiver-pairing-current-evidence.json)。允许的 `cargo check --locked --all-targets` 由主线程统一执行。没有运行应用、测试、厂商 JavaScript、安装器或 DLL，不能宣称实际窗口像素、鼠标命中、真实固件/扫描/配对流程已验收。

仍待逐项完成：源成功响应后的1秒关闭、失败后的4秒恢复（本批不引入计时器）、配对主页面 `Te` 的临时名称/加载/连接条件、固件实际分类与流程、动态候选身份/原图资源继续覆盖，以及实际窗口的字体、连续文字换行、弹层裁剪/焦点和各类输入验收。以上不是完整 179 产品完成声明。
