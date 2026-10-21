# 鼠标：灵敏度、DPI 档位与轮询率

本页解释 Performance 页面实际改变的对象。DPI 是传感器分辨率；DPI 档位是配置档中可切换的 X/Y 数值集合；轮询率是设备向主机上报的频率。三者分别有命令和结果消费者，不能把“档位表写入成功”解释成“当前 DPI、板载配置和轮询率均已确认”。

## 适用产品与输入

所有数值必须来自该 PID 当前 `DeviceInfo` 和实际挂载的控件。以 PID190 为完整复核样本：其 `minDPI=100`、`maxDPI=45000`、`dpiStep=1`，支持独立 X/Y、板载配置与多设备接收器。它的 UI 和中间件默认配置均有五个档位：400、800、1600、3200、6400，`active=3`，`enable=true`；`active` 是从 1 开始的档位身份。默认配置描述显式创建／重置时使用的内容，不能证明当前设备具有这些值。

| 输入 | 实际对象与约束 |
| --- | --- |
| X、Y DPI | 当前配置档的 `dpiStages.stages`；独立 XY 关闭时，修改 X 同时修改 Y，关闭独立 XY 会将 Y 归并为 X |
| 当前档位 | `dpiStages.active`；仅选择存在的档位，界面档位编号从 1 开始 |
| 档位启用、可见性、顺序 | 本地配置的 `enable`、逐项 `visible`、数组顺序；设备返回的数值表不能反推出这些编辑属性 |
| 有线／无线轮询率 | 当前配置档各自的轮询率字段，单位 Hz；选项随连接类型、接收器、固件及产品分支改变 |
| 游戏内轮询率 | 独立的 `inGamePollingRate` 开关和 wired/wireless 值，不能用普通轮询率一个字段替代 |

PID182 的接收器路径必须区分自己的逻辑接收器 183 与共享主接收器 179/164：前者查询 `getProfilePollingRate(1)`，后者查询 `getUSBHighSpeedPollingRate(1)`。普通主接收器路径会将写入限制到 1000 Hz。这是 PID182 的分支，不能据此限制所有鼠标。

## 页面编辑与真实写入

PID190 的 `pA` 控件持有档位编辑行为。数值、开关、显隐、排序和独立 XY 操作进入 `IE` reducer，再发出 `ON_STAGES_CHANGE`。选择另一档位的 `setStage` 还单独调用 `setActiveDPI`；普通数值编辑并不自动等同于该瞬时命令。

中间件 `25947.ft` 的处理顺序是：选择当前配置、复制档位、更新 active/count/enable、推进文档版本、保存配置并通知本地状态消费者，然后提交设备任务。其“本地 completed”消息发生在设备任务确认之前，因此文件保存与设备完成必须分别呈现。设备任务使用 `NORMAL_SKIPPABLE` 队列，后续修改可能取消或替换尚未完成的工作。

PID190 `96571.Xe` 将启用状态与可见档位转换为设备表，调用 `setDPIStages(1)`，检查取消，再调用 `getDPIStages(1)` 逐项比较。具备 `taskRunnerSetDPI.setDpiLevel` 的分支改用 `setDpiLevel(1)`，其确认范围不同，不能声称执行了同一张表的读回。

瞬时 DPI 任务 `96571.et` 则针对 profile/class 0：先读取 `getDpiLevel(0)`，相同则跳过写入；不同则 `setDpiLevel(0)` 后再次读取并比较 X/Y。它在读取、写入和读回之间检查取消。profile/class 0 的当前 DPI 和 profile/class 1 的档位表是两类观测。

板载同步先从真实板载清单中选出所有 GUID 与当前配置匹配的槽位，再逐槽位预读并按需写入；该路径的 setter ACK 不是另一次写后 GET。不得根据配置页的槽位数量自行构造设备槽位或把一次当前表写入当作全部板载保存完成。

PID182 的轮询率写入同样先保存所选有线／无线配置字段和版本，再进入任务：预读、相同则跳过、setter、独立 GET 比较。板载轮询率任务另行按真实匹配槽位预读并写入。

## 打开、重连与事件

PID190 中间件 `31468.fn` 经 `60763.w` 读取以实际序列号选择的本地配置；`deviceMetadatas[serial].activeProfileGuid` 决定当前配置，不能取列表第一项冒充选中值。后续恢复链发布 `MW_SET_STAGES`，再提交 `ON_STAGES_INIT`。该产品实际挂载配置没有启用 `loadProfileDataFromDevice`，泛型代码中的 getter 分支不构成它的“设备 GET 还原全部 UI 属性”路径。

这里的缓存恢复是原版实际路径；`visible`／`independent`／`enable` 来自本地配置，设备 getter 不返回这些属性。因此，页面中的这些值与真实 GET／读回数值的来源不同。

硬件 DPI 事件是否改变编辑器必须逐个追踪其消费者。PID182 当前 `84910.Be` 没有 DPI_CHANGE 分支；解析器存在该事件不等于页面已经消费。其灵敏度映射动作有另一条 `ON_STAGES_CHANGE(fromDevice)` 路径，会更新当前配置档位并发布 `eCI`。

## 失败、取消与退出

原版 reducer 保留已编辑的乐观值，任务错误通过 action/error 通道发布；上述完整函数体没有“任务失败后把编辑器回滚到默认值”的处理。`data not match`、取消和读取失败分别走其错误分支。PID182 的异常返回 0 和未知轮询率编码的 decoder 默认值是源码行为，不能据此称设备实测返回了 0 Hz 或支持默认频率。

任务队列拥有取消、锁、跳过和结算；`cancelAllTasks` 能取消当前任务并处置待执行任务，但 PID182 完整 `90012.hn` 中没有通用的该调用，不能把队列 API 的能力当成退出函数实际执行的动作。灵敏度匹配的采样和停止路径见 [高级灵敏度](advanced-sensitivity.md)。

## PID190 的实际页面与 Rust 对照

当前导航实际挂载 `uS`，而不是把所有 Performance 控件依名称全部加入页面。完整组件只有两列：左列 `MA → pA` 灵敏度、`gA → GA` Windows 鼠标属性；右列先按 `isBle` 隐藏或显示 `XA → xA` 轮询率，再挂载 `NS → dS` 灵敏度匹配器。轮询率调用明确传入 `supportInGamePollingRate:true` 和 `supportColor:true`；此根没有额外的电源卡片，也没有通用 Apply／Save 页脚。

`MA` 从当前 `selectedProfileGuid` 查找真实选中的 profile，档位为空时 `pA` 整体返回 null。多档位编辑继续受 selectedProfile 条件约束。PID190 未传入 `useTwoWayTab`，因此这里显示 Sensitivity Stages 开关；不能把其他型号的 Basic／Advanced 双向页签移植到此页。`sA → oA` 另持有拖动预览与数值预览，释放才把相应数值提交给父组件；拖动时调整被选档位身份，不能只重排 DPI 数值。

| 页面对象 | 当前官方行为 | 当前 Rust 所有者与核对结果 |
| --- | --- | --- |
| 根与列顺序 | 左为灵敏度／系统属性，右为条件轮询率／匹配器 | `mouse/mod.rs::performance`、`mouse_properties.rs`、`dpi/mouse_matcher_owner.rs` 有对应组合；PID190 的匹配器单独创建，其他 PID 不能据此计为已覆盖 |
| 卡片与换行 | `.body-widgets` 为 wrap 行、最大宽 1240px；列宽 600px；卡片宽 600px、内边距 30px 40px、上下外边距 10px、圆角 5px；1279px 以下列增加左右 30px | `mouse_performance_layout.rs` 保留固定卡片宽度和行换行；最终像素、缩放、翻译换行、滚动和 tooltip 定位尚未运行核对 |
| 档位与 XY | stages 开关、逐行可见性、独立 XY、当前档位、拖动与单档位分支；关闭 XY 必要时把 Y 合并到 X | `dpi/mouse_dpi_rows.rs`、`mouse_dpi_number.rs`、`mouse_current_dpi.rs` 有对应处理；页面草稿和 GET／读回行值分开，不能用草稿证明设备已确认 |
| 轮询率与状态 | BLE 不挂载轮询率；有线／接收器选择对应字段，选中按钮不重复提交；彩色标记和游戏内轮询率为独立子界面 | `polling/mouse_polling.rs`、`mouse_polling_view.rs`、`mouse_polling_ingame.rs` 有连接分支、颜色按钮和独立游戏设置；真实服务／设备与交互验收仍未执行 |
| 高轮询率提示 | 普通轮询率大于 1000 时显示对应警告；原件另带远端技术说明链接 | Rust 当前呈现警告文案；远端内容按本地优先范围排除，不能添加禁用的联网占位入口。警告文案、排版及条件仍须独立验收 |
| 系统鼠标属性 | `GA` 点击调用 `OpenMouseProperties`；`UA` 初始 Windows 11 图标，随后通过版本查询／共享缓存切换图标 | `mouse_properties.rs` 用系统属性适配器，工作区创建时选择系统版本对应图标；图标初始化时序与原版不同，OS 动作成功／失败和资源呈现未运行核验 |
| 匹配弹层 | mount 发 GET_STATE；READY／CALIBRATING／COMPLETED／ERROR 决定步骤、进度、结果表、重试和按钮；关闭与取消选择不同命令，unmount 移除 window click listener | `mouse_dpi_matcher.rs`、`mouse_dpi_matcher_state.rs`、`mouse_matcher_owner.rs` 有保留状态、菜单、弹层、mount／unmount 及代次；Shell 的 `dispatch_mouse_matcher` 和 retained service 消费实际请求。不能仅因控件存在就推断输入捕获和恢复已经验证 |
| 读取中／失败／断开 | `pA` 的空数组、profile 门禁、disabled class 与 `xA` 的当前选择来自各自 producer；所读根及灵敏度组件没有通用读取重试条 | Rust `mouse_current_dpi.rs::current_dpi_status` 在灵敏度卡片内显示读取中／断开／错误与 Retry；这是额外的 Rust 状态表面，尚未证明与原版页面的出现位置、文案和恢复动作一致，不能算 1:1 已完成 |

当前 Shell 实际订阅 `MouseCurrentDpiRequested`、`MousePollingRequested`、`MouseInGamePollingRequested` 和 `MouseMatcherRequested`，分别进入保存／设备任务或匹配服务；这一点优于只有本地草稿变化的页面。但官方 DLL 实际对照、真实设备写回及窗口视觉／焦点验收均未执行，因此上表中的“有对应处理”只表示静态调用链已找到。

完整页面对照继续覆盖阶段行的 hover、隐藏、拖动取消、输入焦点、Enter／Escape、弹层外部点击、错误首次到达、切换 profile、切换连接和关闭中的迟到结果。原件本身 `cancel` 比较的是整个 `calibrationState` 对象，`closePopUp` 则比较其 `.state`；应按两个完整处理函数分别记录，不能为了使界面看起来一致而自行合并为一条取消语义。

## 原件与复核位置

以下偏移均为原始 UTF-8 文件的半开字节区间。正文复核直接读取原始字节，并核对既有收据中的 SHA-256 和完整函数体；来源哈希只用于分析追溯。

- PID190 UI：`.analysis/inputs/frontend/mouse-190-current/raw/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js`，模块 8193 `DEFAULTPROFILE` `[4824983,4825337)`、`DeviceInfo` `[4825338,4826176)`；`IE` `[4988325,4990268)`；`pA` `[5391072,5396030)`；游戏内轮询率 reducer 从字节 5031428 开始。
- PID190 页面树与完整处理函数：同一 UI 原件的 `uS` `[5420789,5421093)`、导航 `[5595494,5595852)`、`mr` `[5278788,5278944)`、`BO` `[5373209,5373366)`、`oA` `[5383612,5388813)`、`MA` `[5396036,5396489)`、`UA` `[5396502,5397140)`、`GA` `[5397152,5397563)`、`vA` `[5397659,5398102)`、`BA` `[5398103,5399083)`、`xA` `[5399323,5402967)`、`XA` `[5402973,5403292)`、`dS` `[5413672,5420120)`、`NS` `[5420126,5420788)`。已按获取收据核实原件字节身份，并通过 AST 词法绑定解析别名及完整所有者。
- PID190 CSS：同 source set 的 `static/css/main.7ce428ff.css`；行布局 `[70430,70530)`、列宽 `[70900,70965)`、卡片 `[71013,71242)`、窄窗列边距及 media 条件 `[79961,80013)`、轮询率按钮排列 `[425404,425463)`；匹配弹层高度规则 `[431195,431259)`／`[438126,438196)`，完整宽度 media 分支 `[439646,439749)`／`[439749,439834)`。这些区间与原始 CSS 一致，不将静态样式读取当作运行截图比较。
- PID190 中间件：`.analysis/inputs/frontend/mouse-mw-190-current/raw/apps.razer.com/synapse/products/190/mw/6120.c8b93f8fc599c9fca67a.js`，`60763.w` `[2187170,2187906)`、`25947.ft` `[1497619,1499546)`、`96571.Xe` `[1693320,1694924)`、`96571.et` `[1694924,1696239)`。
- PID190 挂载配置：`.analysis/inputs/frontend/mouse-mw-190-current/raw/apps.razer.com/synapse/products/190/mw/main.46378a7add1c518d1ac8.js`，模块 4638 默认配置 `[144667,145373)`。
- 精确追溯索引：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries.current_dpi_profile.owners`、`exports`、`source_semantics`；PID182 为 `queries["mouse-receiver-polling"].products[product_id=182].semantics`，含原始 UI／中间件／主机／native 对照位置。

这里只完成上述源码语义的静态说明。当前获取收据仍包含不可用 source map 和未解决引用；本页不宣称全部 76 个鼠标 UI 已逐项完成，也不代表 Rust、窗口或真实设备运行验收通过。
