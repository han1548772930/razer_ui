# 鼠标：动态灵敏度、旋转与灵敏度匹配

这三项功能解决不同问题。动态灵敏度按输入速度改变输出倍率；旋转补偿改变传感器坐标方向；灵敏度匹配采集两只鼠标的移动量，计算一个 DPI 并保存为可选记录。它们不是操作系统指针速度的三种名称。

## 产品和版本分支

当前动态灵敏度与旋转的专项静态审计覆盖 PID190、192、196、222、226、229、239。固件条件由 Advanced 面板读取 `currentFWVersion` 后计算；面板存在、控件可编辑和底层命令完成是不同条件。

| PID | Advanced 固件门槛 | 官方实际挂载结构 |
| --- | --- | --- |
| 190 | `0.0.18.8` | 动态面板直接挂在 body；旋转在左栏，另挂空右栏 |
| 192 | `1.2.9.9` | 与 190 相同的面板层级 |
| 196 | `1.0.1.0` | 动态与旋转直接作为 body 的同级子组件 |
| 222 | `0.0.0.0` | 动态与旋转在同一个左栏 |
| 226 | 没有相同的 floor 导出 | 独立 chunk 直接传 `isCurrentFWSupportAdvanced=true`；面板层级同 190 |
| 229 | `0.0.18.8` | 动态与旋转在同一个左栏，另挂空右栏 |
| 239 | `0.0.0.0` | 动态与旋转在同一个左栏，另挂空右栏 |

PID190 的旋转和动态灵敏度均以当前配置档为作者数据来源。审计中的 `getter_direct_callers=[]`／`getter_consumers=[]` 表示这些 getter 没有在已捕获的实际页面初始化链中成为编辑器恢复来源；有 getter 方法并不等于打开页面会读回它。

PID226 的 `loadDynamicSensitivitySettings`／`loadRotationSettings` 另把该产品 `DEFAULTPROFILE` 与配置档已有字段合并后投影到 UI；其他产品按配置档实际存在的键派发。这个 UI 初值分支不能扩大成设备无配置的判定，也不能产生默认值写回。

## 布局、资源与交互边界

已复读七个产品各自的 CSS，PID226 的页面规则位于 `4659.a39320d9.chunk.css`。公共 body 是最大宽 1240 px、居中、可换行的横向容器；动态面板为左边距 10 px、最大宽 1200 px、内边距 40 px。宽度相同的选择器仍会受上表不同父容器约束，不能据此把所有产品统一成同一棵布局树。

旋转面板高 380 px；十字线宽 165 px、线宽 2 px，鼠标 SVG 盒为 70×108 px，位置 `left=264.5, top=148`。七份 `icon_rotation_mouse.6099cba7.svg` 的完整字节一致，因此 Rust 共享一份旋转图有原件依据。当前 Rust 将图放入 180×180 px 旋转画布，内部几何须与原始盒和中心一起核对；共享资源本身不是产品差异。

动态图的浮动提示使用固定定位、14 px 字体、5 px 内边距、4 px 圆角和 `#222` 背景，坐标标签为 10 px。教程浮层为 800×420 px、上边距 134 px，媒体栏宽 320 px、文字栏宽 480 px；教程开关、首次显示偏好和关闭只改变本地界面状态。PID226 没有同一教程入口，PID196 的入口另受固件条件控制。图表拖动、Ctrl＋滚轮、选择模板和设备提交需分别检验，单张静态页面不能证明这些动作一致。

## 动态灵敏度

页面开关对应 `state`，0 为关闭，1 为开启；模式 0、1、2、3 分别是 `CLASSIC`、`NATURAL`、`JUMP`、`CUSTOM`。前三种使用原件中的固定控制点；Custom 另存 `points`、`templateId` 和用于设备传输的 `customSensorAccelerations`，不能只保存绘图点而省略采样曲线。

图横轴的词条为 `DYNAMIC_SENSITIVITY_INPUT_SPEED`，纵轴为 `DYNAMIC_SENSITIVITY_RATIO`。纵轴表示倍率。横轴实时值由输入监控回调按产品分支计算：六款累计相对 X/Y 的绝对值，226 累计当前与上次相对 X/Y 之差的绝对值，再将累计向量长度除以 `timeTick` 差值／10000；具体门禁与类型边界见下文。该计算式本身不证明它是米／秒或厘米／秒，正文不为原件没有标出的物理单位作换算。

Custom 拖动的真实约束是：首点 X 固定为 0，后续点至少相隔 2；倍率下限 0.1，上限为当前纵轴网格末值。横轴缩放间隔在 5～40 之间、每次 5，显示七个间隔；纵轴缩放间隔在 0.25～1 之间、每次 0.25，显示六个间隔。缩放改变可见坐标范围，不是另一个设备速度设置。鼠标按下选点、文档级 mousemove 更新点、mouseup 提交；相关监听在 effect 清理中移除。Ctrl＋滚轮同步调整两个缩放轴。

`k(points)` 先用单调分段三次插值生成 1000 个中间点，再按 X=1…250 采样成 250 个倍率值，缺少精确命中时沿用前值，最终值至少为 0.1。选择模板、切换到 Custom 和拖动结束都会触发相应曲线提交。

中间件 `Vo` 先把 `dynamicSensitivity` 写入当前配置、推进 53 位版本并保存，然后把设备工作排入 `NORMAL_SKIPPABLE` 的 `setting` 类别。`yi` 的设备顺序是：

1. `setProximitySensorAccelerationState(1,state)`；关闭时到此完成。
2. 开启时调用 `setProximitySensorAccelerationMode(1,mode)`。
3. 仅当 mode=Custom 且数组长度恰为 250 时，把 Float32 数组视为 1000 个字节，每批最多 32 字节／8 个点，调用 `setProximitySensorAccelerationCurveData`；完整批次后等待 15 ms。

该 writer 的完成依据是各 setter 应答，没有在结束时执行 state/mode/curve GET 比较。入口检查 AbortSignal；异常进入失败结果并 reject。不能将其完成描述为已读回全部 250 点。

### 动态灵敏度命令帧、响应和中止边界

`queries.dynamic_sensitivity` 对七份当前 MW 原件的完整 protocol factory、方法体和 writer 做了逐字节复核。190、192、196、222、229、239 使用各自 `6120.*.js`，226 使用独立的 `7846.8d3e3465fc6ae4ee8a38.js`；这组通用 proximity-sensor wrapper 的命令三元组相同，226 的差异在输入监控和 BLE 设备路由。这个结论只覆盖动态灵敏度 writer 调用的 wrapper，不替代下方旋转补偿另有的 BLE 命令分支。实际 `sendCommand` 的参数和成功解码如下：

| 操作 | 命令三元组 | 请求数据 | 原件解码 |
| --- | --- | --- | --- |
| 加速度开关 GET／SET | `[2,11,144]`／`[2,11,16]` | GET `[profileId]`；SET `[profileId,state]` | `data[0]` 为 profile，`data[1]` 为 state |
| 加速度模式 GET／SET | `[2,11,145]`／`[2,11,17]` | GET `[profileId]`；SET `[profileId,mode]` | `data[0]`、`data[1]` 为 profile／mode，并映射 mode 枚举 |
| 曲线设置 GET | `[3,11,146]` | `[profileId,mode]` | `profileId`、`mode`、`inputOffset`；审计中没有对应的曲线设置 SET 调用 |
| 曲线数据 GET／SET | `[37,11,147]`／`[37,11,19]` | `[profileId,mode,totalPointSize,pointOffset,pointSize]` 加数据字节 | GET 仅在 status 成功且有返回包时取前五个字段和 `data[5..]`；SET 不建立 `jsonData` |
| 角度补偿 GET／SET | `[3,11,148]`／`[3,11,20]` | GET `[profileId]`；SET `[profileId,enabled,angle]` | `profileId`、`state`、`angleTune` |

曲线数据方法只对 `mode=Custom` 和数值参数作硬校验；`totalPointSize != 250` 或 `pointSize > 8` 是日志警告而非该方法自身的拒绝条件。SET 还要求末尾参数为 `Uint8Array`，短于 37 字节时用零补齐，包装器没有显式截断更长数组。writer 将 250 个 Float32 值的原始 `Float32Array.buffer` 按 1000 字节拆成 32 字节一批，偏移依次为 0、8、…、248 点，每批 `pointSize=8`，完整批次之间等待 15 ms；这不是把 250 个数直接作为 250 个 HID 字节。`get` 的 `Ar`、`Or`、`Cr`、`Mr` 解码器不检查 status 或返回长度，只有曲线数据 GET 检查；因此 Rust 不能把 `jsonData` 存在解释成设备确认，必须保留状态和长度诊断。

`yi`／`ni` 在开始时只检查一次 `AbortSignal`：已取消时直接记录 `canceled` 并返回，不设置成功或失败结果；随后先 SET state，关闭状态立即以 setter 应答成功并跳过 mode／curve，开启才 SET mode 和可选的 32 字节曲线批次。中间批次没有再次检查取消，也没有补发 state/mode/curve GET；任一命令抛错会把结果置为失败、保存异常并 reject。`Vo`／226 对应 maker 先修改 active profile 的 `dynamicSensitivity`、推进 53 位版本并保存，再把 writer 放入 `NORMAL_SKIPPABLE` 的 `setting` 队列，所以本地持久化和设备应答必须分别记录。

证据锚点：PID190 原件 SHA-256 为 `583205c646ef639bc529824df5e0364c944b66c15c0f6a2742d2630ba58bb0db`，完整 protocol factory 为 `[736583,925561)`，writer 为 `[1819790,1821159)`；PID226 原件 SHA-256 为 `9d1c0e708337caa9208d1f6696bc023b9eed59f2382d19f349e932638086bf9a`，protocol factory 为 `[732754,918181)`，writer 为 `[1761855,1763213)`。其余五个 PID 的完整 source、SHA、方法范围及监控回调均保留在 `.analysis/results/frontend/mouse-mw-audit.json` 的 `queries.dynamic_sensitivity.products` 和 `complete_owners`；这七份是独立原件，不能用 190 的字节收据替代其他产品。

迁移时必须分别对照：九种命令的输入字节、无效参数异常、短／长曲线包、setter ACK 与独立 GET、AbortSignal 发生在首包和中途的结果、队列跳过、失败后的释放，以及 226 的 BLE 输入监控分支。当前仍只有静态源码和模拟边界分析，未执行官方 DLL、厂商 JS 或真实设备，实际应答、写回恢复和释放均保持未验证。

实时速度监控是另一条链。190／192／196／222／229／239 的 `DYNAMIC_SENSITIVITY_UI_COMMAND START` 先 off 再 on 同名 input listener，226 仅 on；之后均等待启动监控。STOP 先移除 listener，再等待停止监控并重置采样状态。速度向 UI 的发布限频为约 60 Hz，发布后也执行该重置；226 的重置清除 lastX／lastY，却没有重置 lastTimeTick。图中的活动面积在 150 ms 无更新后复位。图表 effect 会在开关变化时发 START／STOP；本次复核的 effect 没有额外的卸载 STOP 返回函数，不能据此宣称它具有该清理动作。

## 旋转补偿

PID190 的编辑器是开启开关和 −44°…44°、步长 1° 的滑块。预览鼠标图片使用同一个角度旋转。编辑提交 `rotation={isEnabled,value}`，中间件 `Fo` 保存配置和版本，再排队 `vi`。

`vi` 按产品的 `useMappingEngine.config.setMouseRotation` 和固件条件分流。传感器路径调用 `setProximitySensorAngleTune(1,enabled?1:0,value)`；旧固件映射路径调用 mapping engine 的 `setMouseSensorRotationAngle`，关闭传 0，开启转换为 `(value+360)%360`。两个后端参数不同，负角度不能不经判断地直接传给映射接口。

Protocol25 GET/SET 命令分别为 `[3,11,148]`／`[3,11,20]`；PID226 BLE 分支为 `[0,15,148]`／`[2,15,20]`。writer 按 setter 应答结束，没有写后 angle GET。非 UI 发起的完成路径另向页面发送 `MW_SET_ROTATION_TO_UI`；本地缺少 rotation 字段时，不能把 reducer 初值说成设备测量结果。

## 灵敏度匹配

当前实际挂载匹配器的产品已分别追到 UI、MW 命令入口、完整采样模块、输入事件适配、锁、保存、DPI 任务和协议方法。190／192／222／226 的卡片位于性能页右栏，229／239 位于左栏；226 的 BLE 条件只控制轮询率控件，未移除匹配器。六款均由导航第 2 项引用性能页，再挂载 connect 包装的 Matcher class；包装从 sensitivityMatcherReducer 传入 calibrationState，页面的 renderView 实际选择并呈现导航组件。PID196 留有相关 reducer，仍不能据此判定页面挂载。

| PID | 输入归属与匹配记录 | DPI 与传输分支 |
| --- | --- | --- |
| 190 | module 32807；初次按产品／接收器识别，随后优先按 containerId 区分两条输入流；名称保存为 `en`／`zh-cn` 对象 | wired 190／dongle 191；MW `claimInterface=0`；有双连接配置 |
| 192 | 完整采样 factory、输入管理器、状态机、命令入口、记录保存和 DPI runner 与 190 字节一致；产品身份仍取自己的 DeviceInfo | wired 192／dongle 193；`claimInterface=0`；有双连接配置 |
| 222 | 上述完整采样及相应消费者与 190 字节一致；不能因此改用 190 的 PID 或能力值 | wired 222／dongle 223；`claimInterface=0`；有双连接配置 |
| 226 | 独立 module 14786；按产品身份判断输入归属，不使用 190 的 containerId 优先分支；双连接参考设备额外识别 241；新记录名为字符串 | wired 226／dongle 227／BLE 228；`claimInterface=3`；有双连接配置，BLE 实例为 `rzDevice30` |
| 229 | module 32807 的绑定名称不同；逐项核对后的采样、计算、状态转换和保存顺序与 190 相同，名称为双语对象 | wired 229／dongle 230；`claimInterface=3`；挂载配置为 `isDualLinkDevice=false`，并设置 `setDeviceMode.ignoreCheckMode=true` |
| 239 | 完整采样及上述消费者与 190 字节一致；UI 仍有自己的组件和资源分支 | wired 239／dongle 240；`claimInterface=3`；双连接及模式配置同 229 |

上述接口号是 MW 当前 `DeviceInfo` 的值；不能用 UI 中同名对象或另一产品的接口号代替物理路由。各产品启动均显式启用 `supportDpiMatcher` 和 `inputMonitoring`。190／192／222／229／239 的本批 `DeviceInfo` 没有声明 BLE PID，不能因通用工厂支持 BLE 就给它们增加 BLE 分支。

### START、事件采集与计算

UI 发 `DPI_MATCHER_UI_COMMAND`，payload 的 type 为 START／CONFIRM／CANCEL／RESET／GET_STATE／DELETE_PROFILE／DELETE_ALL_PROFILES。MW 接收入口先执行当前命令门禁，然后将这个 namespace 直接交给匹配 session；普通设备事件另交 TaskMaker。session 入口再次检查 `supportDpiMatcher`。

START 的源码顺序是：从本地所选配置的 `dpiStages.stages[active-1].x` 取得 currentDpi → 设 `maxTravelDistance=5×currentDpi` → 从主机 window storage 取 `connectedDevices` → 注册 `inputMonitoringEvent` → 等待 `startMouseInputMonitoring()` 返回 → 向状态机发 `ON_DPI_CALIBRATION_START`。因此监控启动发生在该状态机申请采集锁之前，不能把流程重述成先取得锁才启动监控。原件对缺少／假值 X 使用 `||100`，这不是设备 GET，也不是允许 Rust 在读取失败后以 100 覆盖设备的依据。

输入来自主机推送的 `inputMonitoringEvent`，每批先解析 `jsonEvent`，再解析逐项 `data`，只将 `type=mouseMove` 的记录纳入移动量。六款 Matcher 的采样 callback 均明确创建 `{timeTick:+e.timeTick}`，随后才把同一个事件对象 push 到主流／参考流和主流 buffer；原生十进制字符串在这个边界转为 Number。这与下文保留字符串的动态速度 callback 不同。这里没有名为 POLL 的厂商匹配命令。约 60 Hz 的节流只发送 GET_STATE，令状态机将已有进度投影到 UI；Rust 的内部 POLL／队列接收是自己的传输组织，不能作为厂商输入采集已验证的证据。

190／192／222／229／239 在有 containerId 时优先认当前主鼠标；只知道参考 containerId 时按“非参考”判断主鼠标。尚无这些身份时匹配 DeviceInfo 的 productId／dongleId／bleId；另允许当前双连接记录中、containerId 相符的 `master.dongleId`，后者还要求已启用 inputMonitoring。226 仅执行这一产品／双连接判断，不检查传入 containerId；两个同 PID 实例的区分不能擅自按 190 补上。

进度按主鼠标累计正向 X 距离除以目标距离计算，完成前最多 99%。每轮从主事件 buffer 取最多 100 项，仅累计不早于两条输入流首项 tick 的较大值且 `x>0` 的项；主距离严格超过目标时记录该批最后时间为 stoptime。后续达到目标、且 `stoptime <= source.events[last].timeTick` 时，先清两个定时器、移除 listener、等待 STOP，再计算结果。这些 stoptime、buffer tick 和比较均为 Number。结果在 `[Math.max(firstMain,firstSource), Math.min(lastMain,lastSource)]` 内分别累计 `x>0`，取 `Math.round(sourceDistance/mainDistance×currentDpi)`。数组通过 forEach／push／splice 保留到达顺序，完整 factory 没有排序；first／last 是首末到达项，不能改写为整条流的最小／最大时间。计算不累计 Y、不取负 X 绝对值，也没有 DPI step 对齐、上下限裁剪或零分母检查；这些不能被模拟器悄悄补齐后宣称完全等价。

两条输入各收到至少一批后才设 500 ms 无事件定时器；每次对应批次重置自己的定时器，空 mouseMove 批次与完全没有回调不是同一条件。同模块另有动态速度监控分支，不参与 Matcher 的距离比计算；不能把动态速度算法移植为 Matcher 计算。

### 确认、记录与设备写入

CONFIRM 先按参考 containerId 查 `connectedDevices`。参考为 179／164 或键盘时，再从 `duallink-devices` 的 master.productId 找实际鼠标；226 在这一步还识别 241。190／192／222／229／239 将参考英文名、中文名或电脑名组织成双语对象，226 直接用参考英文名或电脑名字符串。共同命名 helper 比较的是 `existing.name === candidate`；对象名称不是按语言字段深比较，不能宣称具有与字符串相同的去重语义。

随后创建 GUID 与 `dpi={x:newDpi,y:newDpi,independent:false,visible:true}`，只在 active 对应档位存在时替换它，先向 UI 发布 stages，再依次 enqueue `ON_STAGES_CHANGE`、追加匹配记录、enqueue `ON_DPI_CALIBRATION_CONFIRMED`，最后清空采样状态。根 `sensitivityMatcher.profiles` 保存记录集合；当前选中配置的 `selectedSensitivityMatcher` 保存引用，不能将两者都放进当前 DPI 观测。

两条 enqueue 对应不同工作：档位任务先保存 dpiStages、count／active／enable 和 53 位版本，发布本地 completed，再派发设备任务；匹配记录任务另保存根记录、选中 GUID、再次推进版本并通知 UI。档位保存还按当前 X 搜索匹配记录并发布 selection，随后记录保存才发布确认的 GUID。采样计算完成、匹配文件保存完成、DPI 任务完成必须分开记录。

DELETE_PROFILE 从根记录过滤 GUID。仅当删的是当前选中记录且仍有剩余项时，将第一条剩余项的 DPI 写入当前档位并 enqueue stages 变更；之后无论删的是哪一条，记录保存 payload 都取第一条剩余项 GUID，空集合则为 undefined。DELETE_ALL_PROFILES 保存空记录和空字符串选择，不发送新的 DPI 档位；它没有恢复旧 DPI 的动作。RESET 只把状态机送回 idle 并重置采样，不删除记录、不执行恢复 DPI。选择已有记录则由 UI 先提交当前档位替换，再单独发 `ON_UPDATE_SENSITIVITY_PROFILE`；重命名经 trim、32 字符输入限制、重名及保留名校验后也走这条记录保存事件，不触发采样。

各产品当前挂载的 `taskRunnerSetDPI` 为 `setDpiStages`，未找到切到 `setDpiLevel` 的实际 useFeature 覆盖。转换器先按 enable／visible 形成 `[index,x,y,0]` 表，active 从可见项中重算。设备 runner 执行 `setDPIStages(1,active,count,table)` → 检查取消 → `getDPIStages(1)`，逐行比较设备 index 与本地 index+1，以及 X／Y／末字段。190／192／222／229／239 的循环以零 index 哨兵结束；226 另有 `offset<array.length` 限制。原件当前表比较没有额外核对 GET 的 active/count，也没有对读回长度作完整双向相等检查；不能把“当前函数 completed”扩大为完整表头和长度均一致。通用 setDpiLevel 分支只有 setter 结果，确认范围不同。

USB／普通接收器路径的 DPI 表 GET／SET 为 `[80,4,134]`／`[80,4,6]`，payload 是 class、active、count，再逐项 index 和大端 X／Y／末字段，实际 SET 长度 `3+7×count`。226 BLE 的命令为 `[0,11,132]`／`[255,11,4]`，class 交给 sendCommand 的独立参数；payload 从 active、count 开始，逐项数值为小端，GET 的 classId 为 undefined。BLE writer 传入的长度仍为 `3+7×count`，而末个写入字段结束于 `2+7×count`；该额外字节也是原件行为，不能按 USB 结构重放。

设备任务还从真实 OBM 清单选出与当前 profile GUID 相同的槽位，逐槽预读，数值或 active/count 不同才 SET，并收集错误；这条板载路径没有写后独立 GET。226 有自己的 `rzDevice30`、多槽 OBM 和 BLE framing，不能因计算公式一致就复用 USB 数据包。底层帧、接收器与服务边界继续见 [Performance](performance.md)。

### 锁、失败与关闭

六款状态机均有 idle → requestingLock → calibrating → confirming 及 error 分支。宏录制锁是 shared，设备锁是 exclusive；两次都用 `ifAvailable=true`，冲突返回 idle。TaskMaker 另持有 `STORAGE_LOCK_<key>` 的 exclusive 存储锁。idle／error 释放 retained 采集锁并移除 `dpiMatcher` task；confirming 发布结果状态后释放采集锁；确认写盘走 `lockHeld=true` 的决策，不重新取得那把采集设备锁。

正常完成会清两个输入定时器并 await STOP。CANCEL 会 off listener → await STOP → 发取消 → 重置；其函数本身未 clear 两个定时器。500 ms 超时会发 error、重置、off listener，但没有 STOP；RESET 和 DELETE 后的采样重置同样没有 STOP 或定时器清理。锁冲突回 idle 也没有补发 STOP。完整 session 没有把 start／stop 返回的 `{result:false}` 转换为匹配错误的判断，也没有包住所有命令的统一 catch。必须将这些当前官方行为与 Rust 的实际终止、错误和释放分开对照，不能从状态变 idle 推定原生输入监控已停止。

UI 的 closePopUp 检查 `.calibrationState.state`，采样中发 CANCEL，其他状态发 RESET；cancel 则比较整个 calibrationState 对象与 CALIBRATING 常量，不能合并成相同判断。unmount 仅移除 window click listener，没有从该方法发送 CANCEL。226 的记录菜单／删除全部弹层使用 portal、其 Matcher 浮层没有其他五款的 popupBodyRef／mount overflow 设置；239 的匹配记录还保留自己的菜单样式。关闭、外部点击、错误首次到达、重试和菜单定位都须按当前完整 class 验证。

官方主机将 start／stop 请求交给映射引擎。原生 START 从 `0x4f7b0` 经 engine queue、monitor queue 到 `0xcb100`；service 的 void 启动调用创建 owner 并排入另一 UI 线程的 `0xd0f40` 后，monitor queue 即排入 `{result:true,reason:""}`。逻辑 started 位为 InputMonitorImpl＋41，实际 Raw Input 注册位为 handler＋96，两者不是同一个状态。窗口创建和注册失败另发 error 事件，不会改写已经发出的启动 ACK。重复 START 返回 false／`already started`，未启动 STOP 返回 false／`has not started`；缺 engine thread、monitor thread 或 service 各有独立 reason。

实际输入使用 message-only HWND 和 Raw Input，注册鼠标 usagePage=1／usage=2、flags=`0x2100`。相对移动量取 RAWINPUT＋36／＋40 的有符号 i32；事件 JSON 中 timeTick 为微秒十进制字符串，时钟为 QPC 换算或毫秒 fallback×1000。普通批次仅在距上次发布严格大于 100000 微秒时 flush，首次只建立基线；button 类型另有立即 flush。两次 GetRawInputData 读取失败只日志并丢弃该次输入，没有在这些分支追加监控 error 事件。原生事件回调在 Win64 消费 `(deviceInfoJson,int32 eventType,jsonEvent)` 三个参数，类型为 keyboard=0／mouse=1／error=2；主机 FFI 多声明的第四个 ulonglong 未被 JS consumer 使用，不能解释为逐项 timeTick。

普通 STOP 的 `0xce4d0` 先排入已知设备的 pending 最后批次，清除 device＋32／pending＋72，保留 last-publication-tick＋112，然后释放输入 owner 并排入 UI 注销／窗口销毁。普通 START 复用 service，没有清除该时间历史；device removal 的 `0xcc8c0` 则先清整个＋112，再处理移除设备。UI 注销 flags=1／target=null，即使注销 API 返回失败也清 handler 注册位，STOP ACK 不据此变为失败。ACK 只说明这些任务已排入队列，未证明 UI 清理完成、所有回调送达或线程 join；完整 engine shutdown 才另走等待线程退出与 CloseHandle 链。当前 JS 不提供“每次 RESET 已终止 native thread”的证据。Rust 等待自身 RawInput 线程结束的清理结果须独立对照，不能与官方普通 STOP 的完成边界混为一项。

### 可独立复核的测试边界

以下是由上述完整源码确定的必要比较项，尚未在本次文档工作中执行。输入与期望必须分别保留，不可由 Rust 被测函数生成相同的预期。

| 用例 | 必须保留的原版结果与比较边界 |
| --- | --- |
| 正／负 X、非零 Y、两流起点不齐、窗口端点相等 | 只累计共同闭区间内正 X；负 X 与 Y 不增加距离；独立计算距离、round 前比值和最终 DPI |
| 原生字符串 `"9" → "10"`、`"10" → "9"`、`"0"` 与乱序到达 | Matcher 先 unary+ 再保留到达顺序；动态速度保留字符串。分别比较类型、门禁、首末项和区间，不能统一数值排序 |
| 主距离恰好等于／严格超过目标，参考流仍落后 | 分别记录 stoptime、进度和完成时刻；不得把“进度到 99%”当成采样结束 |
| 空移动数组批次、仅单边事件、500 ms 中断 | 比较 timer 建立／续期、错误事件、listener 与 STOP 顺序，区分空批次和没有回调 |
| 两只同 PID、containerId 缺失、接收器、双连接参考与 241 | 分别验证 226 的身份分支及其他五款的 containerId 优先规则，不能统一使用 PID190 规则 |
| 双语对象名、226 字符串名、重名与电脑名 | 比较原始名称类型、严格相等去重、GUID 引用和 UI 消费；电脑名取自已核实的主机接口 |
| 确认、删除选中项／非选中项／最后一项、清空、RESET | 分别核对两次保存、版本推进、selection、stages 通知和真实 DPI 写入；清空／RESET 不虚构恢复 DPI |
| USB／226 BLE、SET 成功但 GET 不符、读回短表、取消发生于 SET 后 | 比较实际字节序、长度、逐行比较范围、应答／GET 和终止顺序；普通表和 OBM 槽位分别断言 |
| 启动／停止返回 false、锁失败、timeout、CANCEL、窗口关闭和卸载 | 保留原版缺少 STOP／timer 清理的分支与 Rust 自身清理结果；不得把两者的 idle 状态作为清理一致的证据 |
| START ACK 后窗口／注册失败、普通 STOP 后再 START、移除设备 | 分别比较逻辑 started、实际注册位、error 事件、保留／清除的 tick 历史、最终批排队和实际清理；不能把 ACK 当作 OS 完成或 join |

官方没有对 DPI 比值计算额外做正有限数校验；Rust `dpi_matcher::Session` 有相应校验。零主距离、非有限结果与越界 DPI 必须记录两边各自的输出和拒绝边界，不能靠预先过滤输入消除这一差异。

## 当前 Rust 生产链与未完成项

| 功能 | 当前真实消费者与后端 | 核对结果与边界 |
| --- | --- | --- |
| 动态灵敏度 | Pages `dynamic_sensitivity/mouse_dynamic.rs` → Shell `mouse_dynamic.rs`／`mouse_dynamic_save.rs` → Service `mouse/mouse_dynamic.rs` → Device `mouse_dynamic`／`mouse_sensor` → Feature HID 与接收器适配 | 七个 PID 有产品能力及真实命令链；保存先提交当前配置档与版本，随后派发设备工作。曲线作者点、250 个 Float32 值、设备观测和提交结果分别持有；不能仅以产品能力表宣称全部固件／传输分支完成 |
| 旋转 | Pages `mouse_rotation_state.rs` → Shell `mouse_rotation.rs`／`mouse_rotation_save.rs` → direct sensor、PID226 BLE 或 `mapping_runtime/backend/rotation.rs` | 有真实 HID／GATT 应答与旧固件映射路径；映射路径继续到 Rust driver owner 和 `mapping_native_session.rs` 的 `CreateFileA`／`DeviceIoControl`，该链没有把官方业务 DLL 作为后端 |
| 灵敏度匹配 | Pages `dpi/mouse_matcher_owner.rs` → Shell `mouse_matcher.rs` → Service `dpi_matcher_worker.rs`／`dpi_matcher_input.rs` → DPI 传输与本地存储 | PID190 已接入采样、共享锁、确认、删除、真实 DPI setter 及保存；`prepare_matcher` 明确只创建 PID190，尚未接入上文已确认挂载的 192／222／226／229／239 |

匹配器的产品限制同时存在于 Pages `prepare_matcher` 和 Service worker 的路由／选项构造；Device session 的 `PRODUCT_ID` 也为 190。Service `Options::confirmation_name` 目前只含 179／164／键盘参考分支及双语名称。接入其余五款必须同时迁移各自物理身份、226 字符串记录与 241 分支、BLE 228 的 DPI 传输、输入归属、结果消费者和清理，不能只放宽页面的产品判断。

动态与匹配器的输入监控共用 `mouse_input_monitoring::Session` 的 START／POLL／STOP 收据。Windows 消费者为 `runtime/windows/mouse/input_monitoring.rs`，继续到 `razer-platform` 的独立 RawInput 线程；设备身份通过当前 HID／SetupAPI 结果核对。部分 START 后丢失应答仍保留需要 STOP 的所有权；停止采集、停止服务和等待线程退出分别处理，不能用页面 guard 失效代替实际清理。

实时速度必须保留产品分支。190／192／196／222／229／239 累计当前 Raw Input 的 `abs(x)`／`abs(y)`，门禁原式为 `lastTimeTick !== 0 && timeTick > lastTimeTick`；226 则要求上次 X、Y、tick 都严格不等于数值 0，累计 `abs(x-lastX)`／`abs(y-lastY)`，没有新 tick 比较门禁。两者均以 `sqrt(sumX²+sumY²)/(sumTicks/10000)` 计算，但 226 每次都计算，并只把 NaN 转为 0，Infinity 不会被这个判断清除。这里的 X／Y 本身已经是原生相对移动量，不能把 226 的再次作差解释为另一种绝对坐标输入。

tick 类型同样属于协议语义：原生批次把 `timeTick` 序列化为十进制字符串。190／192／196／222／229／239 的 factory 绑定 `3569.Sl`，226 绑定 `53981.uK`；七份 helper 均是没有 reviver 的 `JSON.parse(e)`。动态 callback 直接保存 `l=e.timeTick`，首次保存后，两侧为字符串的 `>` 执行字典序比较，减法才转数值；`"9" → "10"` 不能按数值递增推定会通过门禁，`"0" !== 0` 也为 true。Matcher 的另一 callback 则在入 buffer 前使用 `+e.timeTick`，其后比较为数值，不能套用动态速度的字典序结论。当前 Rust 速度实现把字符串解析为 f64 再比较，因此边界测试必须保留原始类型和位数，不能仅用数值时钟模型宣称全等。

当前 `razer-device/src/mouse/mouse_dynamic_input.rs::Speed` 和 Service 的 `mouse_dynamic.rs::monitor_with` 使用统一的绝对移动量算法，没有按 226 选择另一计算器。从空状态开始，同批包含 `(tick="10000",x=3,y=4)` 和 `(tick="20000",x=3,y=4)` 时，独立期望为上述六款 5、226 为 0；第二条改为相同 tick、`x=6,y=8` 时，226 的表达式产生 Infinity，统一 Rust 计算器保持 0。同批改为 `(tick="9",x=3,y=4)` 和 `(tick="10",x=6,y=8)` 时，六款为 0、226 为 50000；数值门禁会产生另一结果。收据用 `positive_infinity` 字符串保留非有限语义，不写非标准 JSON Infinity token。这些均为完整源码推导的独立用例，尚非实际 DLL 或设备测量结果。

停止时最后一批事件的传递也需要单独比较。当前 Rust 的 `stop_mouse_input_monitoring` 等待线程停止，接收 final events 并将各设备的剩余条目 flush 到 `pending`，随后用 `State::default()` 替换整个状态；STOP 的 `Receipt` 不带事件，最后这批 `pending` 没有返回给请求方。官方 `0xce4d0 → 0xce5d8 → 0xce9ec` 则先向 observer 所属 runner 排入最后批次，再排停止完成；这里只证明 post 的顺序，不证明所有 observer 的实际送达都早于 ACK。Matcher 正常完成、CANCEL 和动态 STOP 又会先移除自己的 JS listener，再等待 STOP，故不能仅从传输差异断言匹配结果已受影响。现有 Rust 运行时纯测试覆盖阈值、移除与身份失败，尚没有覆盖 STOP 最后批次的交付契约。

当前动态页面会在进入 Advanced／连接后请求独立 GET，写入完成后再发单独的 GET；`Requests` 将未观测、读取中、断开、失败、真实读取与显式编辑分开，并拒绝旧连接／旧编辑结果覆盖新状态。可编辑条件使用当前编辑或真实读取的 enabled／mode，未从本地配置回退。官方本页的编辑器恢复来源是配置档，writer 以 setter ACK 结束；因此必须分别对照官方作者状态、Rust 草稿和独立实测结果，并继续核对额外读取对加载、失败及监控启动的影响。额外 GET 不能写成官方原有的初始化或确认步骤。

布局尚有确定差异：`MouseProductWorkspace::advanced` 统一先放动态面板，再创建旋转左栏，没有实现 196、222、229、239 在上表中的挂载分支。现有动态纯测试涵盖完整 Custom 参数、未知观测值、重连、过期结果和监控独立 guard；设备模拟测试涵盖 250 点字节序、setter ACK 与独立 GET、部分失败和取消。旋转另有 BLE 事务／分片应答和映射 driver 取消／释放测试。它们不能替代缺失产品接线、窗口交互验证或官方实际 DLL 对照。

## 原件与复核位置

- UI：`.analysis/inputs/frontend/mouse-190-current/raw/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js`，图表完整 declarator `[5429003,5438134)`、动态面板 `[5438796,5440075)`、旋转编辑器 `[5440651,5442191)`、Advanced 根 `[5442192,5442683)`；匹配记录 class `[5404635,5412649)`、匹配浮层 class `[5413672,5420120)`、性能页 class `[5420789,5421093)`。
- Advanced 根的其余完整 declarator：192 `main.e3a31907.js` `[5615083,5615574)`；196 `main.e5afc608.js` `[5415512,5415925)`；222 `main.38e03b8d.js` `[5560029,5560485)`；226 `4659.47cd5287.chunk.js` `[33639,33901)`；229 `main.551458e6.js` `[5435381,5435875)`；239 `main.6bc74a2d.js` `[5430996,5431517)`，均位于各自 `mouse-<PID>-current/raw/apps.razer.com/synapse/products/<PID>/ui/static/js/`。
- 匹配器性能页完整 class：192 `[5593696,5593984)`、222 `[5538642,5538930)`、226 `[84364,84702)`、229 `[5413950,5414282)`、239 `[5408964,5409296)`，文件同上一项。此处通过 class → connect 包装 → JSX 调用核对挂载，不以导出名或 reducer 存在代替挂载证据。
- 中间件：`.analysis/inputs/frontend/mouse-mw-190-current/raw/apps.razer.com/synapse/products/190/mw/6120.c8b93f8fc599c9fca67a.js`，完整模块 32807 `[1137407,1146378)` 含匹配计算与监控；动态 writer `yi` `[1819790,1821159)`、本地保存 `Vo` `[1632451,1632868)`；旋转 writer `vi` 与完成消费者 `[1818402,1819790)`、保存 `Fo` `[1632086,1632451)`。本页直接复读了这些原始字节。
- 分支追溯：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries.dynamic_sensitivity.products`／`ui_products`、`queries.rotation.products`、`queries.dpi_matcher.complete_owners`／`source_semantics`，含其余 PID、BLE、固件、锁与主机 ABI 原件定位。
- 实时速度 callback 的完整 AST 区间由 `queries.dynamic_sensitivity.products[*].monitor_callback` 关联当前原件：190 `[1145946,1146377)`、192／196 各自原件的 `[1146149,1146580)`、222 `[1145936,1146367)`、226 `[1139320,1139784)`、229 `[1105123,1105554)`、239 `[1148419,1148850)`。七份原始字节、完整 AST 边界及 body 摘要分别重核；同偏移不能合并为同一来源。
- 共享输入监控专项位于同一报告的 `queries.shared_input_monitoring`：保留 151 个完整原生函数、153 个 raw chunk／39740 字节、11 张实际 PE 指针表、122 条 unwind 记录、165 个完整前端 AST owner、14 份源码集和 4 份 Rust 源码的 37 个完整函数。原 PE、捕获指令与边、源码哈希及 AST 边界独立重读核验；禁用 collector 后的 verifier 通过，91 项篡改负例全部拒绝。12 组速度模型区分 Number 合成输入与原生兼容字符串输入，另保留 Matcher unary+／到达顺序反例；这些不构成已执行的 Rust 或官方 DLL 功能对照。1156 条未展开直接边和 18 条间接／导入调用仍明确保留。

Matcher 追加复核使用各自 `mouse-mw-<PID>-current/raw/apps.razer.com/synapse/products/<PID>/mw/` 下的原件。下表为完整 owner 的 UTF-8 半开字节范围，不能将不同产品的相同模块号当作同一原件：

| PID | 公共 chunk | Matcher | input manager | 命令入口 | 状态机／采集锁 |
| --- | --- | --- | --- | --- | --- |
| 190 | `6120.c8b93f8fc599c9fca67a.js` | `[1137413,1146378)` | `[390828,425675)` | `[1156710,1157566)` | `[1897119,1899005)`／`[1957181,1958665)` |
| 192 | `6120.8edb169da43f3f616033.js` | `[1137616,1146581)` | `[391031,425878)` | `[1156913,1157769)` | `[1897322,1899208)`／`[1957384,1958868)` |
| 222 | `6120.e58c89f147aa90207bcd.js` | `[1137403,1146368)` | `[390818,425665)` | `[1156700,1157556)` | `[1897109,1898995)`／`[1957171,1958655)` |
| 226 | `7846.8d3e3465fc6ae4ee8a38.js` | `[1131269,1139785)` | `[388034,422601)` | `[1145537,1146380)` | main `[136495,138371)`／`[193476,194928)` |
| 229 | `6120.65d8621c9f8551e22ecc.js` | `[1096590,1105555)` | `[381222,415462)` | `[1115860,1116716)` | `[1813799,1815685)`／`[1871234,1872718)` |
| 239 | `6120.7add40c8ada52a178a2b.js` | `[1139886,1148851)` | `[393301,428148)` | `[1159183,1160039)` | `[1899682,1901568)`／`[1959744,1961228)` |

226 的状态机／锁位于同目录 `main.6e4159f30f7ef28ad719.js`，其余行的这些 owner 均在对应公共 chunk。保存与设备确认使用同一批各产品公共 chunk 的以下完整 owner：

| PID | 匹配记录保存 | DPI maker | DPI runner |
| --- | --- | --- | --- |
| 190 | `[1631693,1632086)` | `[1497619,1499546)` | `[1693320,1694924)` |
| 192 | `[1631896,1632289)` | `[1497822,1499749)` | `[1693523,1695127)` |
| 222 | `[1631683,1632076)` | `[1497609,1499536)` | `[1693310,1694914)` |
| 226 | `[1443485,1443869)` | `[1249585,1251622)` | `[1637601,1639288)` |
| 229 | `[1570348,1570741)` | `[1439126,1441053)` | `[1616664,1618268)` |
| 239 | `[1634251,1634644)` | `[1500135,1502062)` | `[1695881,1697485)` |

六款 Matcher UI 浮层完整 class 同上述各自 UI 文件：190 `[5413672,5420120)`、192 `[5586579,5593027)`、222 `[5531525,5537973)`、226 `[77491,83695)`、229 `[5406833,5413281)`、239 `[5401783,5408276)`。226 UI `main.6201645c.js` 的 rename `[210862,211578)`／selected `[212546,213201)` 与 190 UI 的 `[5028166,5028881)`／`[5029865,5030519)` 分别复核，不能以浮层 class 替代记录 reducer。226 MW `rzDevice30.9235e566507b474af951.js` 的 DPI stages GET `[71555,71855)`、SET `[71855,72460)`、decoder `[19221,19504)`提供 BLE 字节与长度依据；USB 方法在对应公共 chunk，190 GET／SET `[800312,800657)`／`[800657,801392)`，226 `[796031,796372)`／`[796372,797103)`。

输入身份以对应获取收据及 `.analysis/inputs.json` 的实际长度与 SHA-256 核对；本批鼠标复核验证了专项报告中 44 份 JS 原件、996 个完整 owner 的原始字节及 AST 边界。PID190 CSS 最初来自安装缓存，获取收据另有两次官方 HTTP 返回与保留字节一致的复核，不能仅凭缓存标志判定当前性。缺失 source map 和引用闭包未决项仍按各 source set 保留。

源码获取、逆向语义、Rust 独立实现、UI／后端接线、静态／模拟验证和运行时验收分别记录。本次未运行厂商 JS、应用、官方 DLL 或真实设备；所有对应功能的官方实际 DLL 结果、设备实测、恢复和释放对照仍为**未验证**。空设备、模拟一致和已有纯测试不能把这些项改为通过。
