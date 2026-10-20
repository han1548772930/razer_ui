# 鼠标：动态灵敏度、旋转与灵敏度匹配

这三项功能解决不同问题。动态灵敏度按输入速度改变输出倍率；旋转补偿改变传感器坐标方向；灵敏度匹配采集两只鼠标的移动量，计算一个 DPI 并保存为可选记录。它们不是操作系统指针速度的三种名称。

## 产品和版本分支

当前动态灵敏度与旋转完整审计覆盖 PID190、192、196、222、226、229、239。PID190 的 Advanced 导航受固件门槛 `0.0.18.8` 控制；PID229 同为 `0.0.18.8`，PID222／239 导出 `0.0.0.0`。PID226 是独立页面与 BLE 协议分支，没有相同的 firmware-floor 导出，不能直接套用 PID190 的条件。

PID190 的旋转和动态灵敏度均以当前配置档为作者数据来源。审计中的 `getter_direct_callers=[]`／`getter_consumers=[]` 表示这些 getter 没有在已捕获的实际页面初始化链中成为编辑器恢复来源；有 getter 方法并不等于打开页面会读回它。

## 动态灵敏度

页面开关对应 `state`，0 为关闭，1 为开启；模式 0、1、2、3 分别是 `CLASSIC`、`NATURAL`、`JUMP`、`CUSTOM`。前三种使用原件中的固定控制点；Custom 另存 `points`、`templateId` 和用于设备传输的 `customSensorAccelerations`，不能只保存绘图点而省略采样曲线。

图横轴的词条为 `DYNAMIC_SENSITIVITY_INPUT_SPEED`，纵轴为 `DYNAMIC_SENSITIVITY_RATIO`。纵轴表示倍率。横轴的实时值由输入监控回调计算：同批鼠标移动的绝对 X/Y 量累计后取向量长度，再除以 `timeTick` 差值／10000。该计算式本身不证明它是米／秒或厘米／秒，正文不为原件没有标出的物理单位作换算。

Custom 拖动的真实约束是：首点 X 固定为 0，后续点至少相隔 2；倍率下限 0.1，上限为当前纵轴网格末值。横轴缩放间隔在 5～40 之间、每次 5，显示七个间隔；纵轴缩放间隔在 0.25～1 之间、每次 0.25，显示六个间隔。缩放改变可见坐标范围，不是另一个设备速度设置。鼠标按下选点、文档级 mousemove 更新点、mouseup 提交；相关监听在 effect 清理中移除。Ctrl＋滚轮同步调整两个缩放轴。

`k(points)` 先用单调分段三次插值生成 1000 个中间点，再按 X=1…250 采样成 250 个倍率值，缺少精确命中时沿用前值，最终值至少为 0.1。选择模板、切换到 Custom 和拖动结束都会触发相应曲线提交。

中间件 `Vo` 先把 `dynamicSensitivity` 写入当前配置、推进 53 位版本并保存，然后把设备工作排入 `NORMAL_SKIPPABLE` 的 `setting` 类别。`yi` 的设备顺序是：

1. `setProximitySensorAccelerationState(1,state)`；关闭时到此完成。
2. 开启时调用 `setProximitySensorAccelerationMode(1,mode)`。
3. 仅当 mode=Custom 且数组长度恰为 250 时，把 Float32 数组视为 1000 个字节，每批最多 32 字节／8 个点，调用 `setProximitySensorAccelerationCurveData`；完整批次后等待 15 ms。

该 writer 的完成依据是各 setter 应答，没有在结束时执行 state/mode/curve GET 比较。入口检查 AbortSignal；异常进入失败结果并 reject。不能将其完成描述为已读回全部 250 点。

实时速度监控是另一条链。`DYNAMIC_SENSITIVITY_UI_COMMAND START` 先替换同名 input listener，再启动监控；STOP 移除 listener、停止监控并清理累计状态。速度向 UI 的发布限频为约 60 Hz，图中的活动面积在 150 ms 无更新后复位。图表 effect 会在开关变化时发 START／STOP；本次复核的 effect 没有额外的卸载 STOP 返回函数，不能据此宣称它具有该清理动作。

## 旋转补偿

PID190 的编辑器是开启开关和 −44°…44°、步长 1° 的滑块。预览鼠标图片使用同一个角度旋转。编辑提交 `rotation={isEnabled,value}`，中间件 `Fo` 保存配置和版本，再排队 `vi`。

`vi` 按产品的 `useMappingEngine.config.setMouseRotation` 和固件条件分流。传感器路径调用 `setProximitySensorAngleTune(1,enabled?1:0,value)`；旧固件映射路径调用 mapping engine 的 `setMouseSensorRotationAngle`，关闭传 0，开启转换为 `(value+360)%360`。两个后端参数不同，负角度不能不经判断地直接传给映射接口。

Protocol25 GET/SET 命令分别为 `[3,11,148]`／`[3,11,20]`；PID226 BLE 分支为 `[0,15,148]`／`[2,15,20]`。writer 按 setter 应答结束，没有写后 angle GET。非 UI 发起的完成路径另向页面发送 `MW_SET_ROTATION_TO_UI`；本地缺少 rotation 字段时，不能把 reducer 初值说成设备测量结果。

## 灵敏度匹配

PID190 的匹配器以 `supportDpiMatcher` 为入口门槛。START 从当前本地选中档位读取 X DPI，记录主鼠标与参考鼠标的移动事件，目标累计距离是 `5 × currentDpi`。主／参考设备以 containerId 优先识别，尚无 containerId 时再使用产品身份和输入监控条件。

它只统计共同时间窗口内正方向的 X 移动量，结果为 `round(sourceDistance/mainDistance × currentDpi)`。进度完成前最多为 99%；采样结束停止监控并进入确认状态。任一输入流连续 500 ms 没有事件有独立的定时错误路径。

CONFIRM 为结果创建 GUID、`dpi={x:newDpi,y:newDpi,independent:false,visible:true}` 和名称，更新当前档位，再分别提交档位变更与匹配记录变更。记录保存在根 `sensitivityMatcher.profiles`，当前配置保存 `selectedSensitivityMatcher`。参考设备经过共享接收器／键盘时会从本地双连接关系解析实际鼠标身份；不能拿接收器名称直接替代参考设备名称。

CANCEL 移除采样 listener、停止监控、发送取消状态并重置累积；RESET 和 GET_STATE 有各自状态事件；删除当前选中的匹配记录可能选择第一条剩余记录，并将其 DPI 写入当前档位；删除全部记录清空记录及选择。匹配器不是只读计算器，确认／删除操作可能改变档位和本地配置。

会话状态机还单独拥有宏录制共享锁、设备排他锁、存储锁与 retained task。匹配器以 `ifAvailable=true` 申请宏／设备锁，冲突立即拒绝；idle/error 释放 retained lock 并移除任务，confirming 先释放采集锁，再执行本地确认保存。输入监控回调、启动／停止 ACK、listener 移除和 mapping engine 关闭是不同生命周期动作。

## 原件与复核位置

- UI：`.analysis/inputs/frontend/mouse-190-current/raw/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js`，模式／固定曲线／插值代码在 `[5425000,5429025)` 内，图表与动态开关在 `[5429025,5440076)`，旋转编辑器 `[5440651,5442191)`；动态轴词条导出在字节 241468 附近；匹配 UI 命令在字节 5403822 附近。
- 中间件：`.analysis/inputs/frontend/mouse-mw-190-current/raw/apps.razer.com/synapse/products/190/mw/6120.c8b93f8fc599c9fca67a.js`，完整模块 32807 `[1137407,1146378)` 含匹配计算与监控；动态 writer `yi` `[1819790,1821159)`、本地保存 `Vo` `[1632451,1632868)`；旋转 writer `vi` 与完成消费者 `[1818402,1819790)`、保存 `Fo` `[1632086,1632451)`。本页直接复读了这些原始字节。
- 分支追溯：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries.dynamic_sensitivity.products`／`ui_products`、`queries.rotation.products`、`queries.dpi_matcher.complete_owners`／`source_semantics`，含其余 PID、BLE、固件、锁与主机 ABI 原件定位。

这些是前端／中间件静态语义；获取闭包中的 source map 缺失仍按各 source set 收据保留，真实输入采样、设备写入和窗口交互未在本次执行。
