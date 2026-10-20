# 校准、硬件灯光与软件灯光

## 校准状态机和采样

当前 2636 完整所有者是 `.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/main.f0ca95429464cd09cbdb.js` 的模块 66118，UTF-8 bytes `[1289001,1313647)`。该模块同时保留两代处理器；当前八款双摇杆产品 descriptor 选择 `ControllerCalibrationParams.Version:2`，不能仅看到 V1 类就按固定时间放行下一步。

部件 ID 是 None=0、左摇杆=1、右摇杆=2、左扳机=3、右扳机=4。摇杆流程依次为右、下、左、上、旋转、完成；每一步将设备 ADC 读数与本步目标比较。V2 进入时先读取实际中心，超出 `CenterPointThreshold × Range.max` 的轴才以产品理想中心作为计算基准。采样只接受严格位于原件 Range.min/max 之间的值。

V1 的 `next` 直接处理当前步并前进；V2 的 `next` 要求 `isStepValid`。V2 的方向步骤须先达到该方向边界，再回到中心阈值内；旋转步骤检查四个边界。`MW_UPDATE_CONTROLLER_CALIBRATION_PROGRESS` 除 step、stepDesc、error 外，还包含 V2 的 isStepValid。UI 按该事件控制 Next；不能用播放完说明动画代替用户真实移动。

| 当前产品 | ADC 范围 / 理想中心 | 参数 |
| --- | --- | --- |
| 2629 | 0–1024 / (512,512) | Version 2；DeadzoneRatio 3；AllowedDeviationPercent 9；BoundaryTolerance 12；ValidityThreshold 0.3；CenterPointThreshold 0.1 |
| 2636、2647、2650、2676、2684、4133、4144 | 0–4096 / (2048,2048) | Version 2；前述参数中 ValidityThreshold 为 0.35，其余同列 |

这些是产品计算常量，不是设备当前校准值。2636 descriptor 中该对象在 main byte 2637735；2629 在其 MW `main.b468970129c30a905292.js` byte 2625198。每款完整原件由其 `gamepad-mw-<PID>-current/acquisition.json` 管理。

V2 将实际移动换算成 -100…100 的 window storage 坐标，限幅后发布，回到中心的值设为 0；该预览坐标不同于原始 ADC，也不同于校准数据。UI 的坐标投影和旋转方向不能反向写回 ADC。处理器有独立采样 timer 和预览节流 timer；reset/stop 清空部件、步骤、采样、边界与有效性，并移除校准预览 window storage。

## 计算、写入、失败和取消

2636 `processCalibrationData` bytes `[1296483,1298184)`：先检查用户移动有效性，计算四次 center reading 的平均值；任一样本偏离均值超过 `DeadzoneRatio/100 × IdealCenterPoint` 就失败。边界从实际 min/max 朝该中心收缩 BoundaryTolerance（当前 12%）。`validateCapturedCalibrationData` 对数量不足或相对理想中心偏差过大只是记录 warning，不能擅自改成新的阻断条件。

写入按部件映射到 `setAnalogInputCalibrationDataAll`，仅填写当前部件的 Lx/Ly/Rx/Ry midpoint/min/max 或 Lt/Rt min/max。设备命令失败异常由 save wrapper 捕获并返回 false，随后报告 `Command Failed`；complete 发布 Step_Error 或 Step_6_Complete，最后 stop 清理。源码这里没有另加一轮独立 GET 逐字段比较，因此不能把“显示完成”解释成进行了另一个已验证的读回流程。取消/关闭走 stop/reset，不把半截样本写为最终数据。

V2 完整方法位置：start `[1305238,1306254)`，next `[1306254,1306549)`，prepareNextStep `[1307352,1307657)`，采样 `[1307862,1309146)`，有效性判定 `[1309322,1311799)`，预览换算 `[1312270,1312882)`，预览清理 `[1312882,1312993)`。Trigger 校准的动作、部件和模拟显示与摇杆流程分别处理，不能把二者的 UI 动画状态或 ADC 量纲合并。

## 2636：设备硬件效果

2636 通过 `supportsLighting:true` 挂载 Lighting，硬件效果所有者为其 MW main 模块 70784 bytes `[1331417,1334682)`，brightness queue 为模块 97901 `[1334683,1336614)`。先真实 GET；遇到 effectId 13 再取扩展静态值，实读 index>8 才进入 custom 模式。preset、custom 与配置草稿中的效果选择是分开的来源。brightness 的实际 GET 与源码 floor/ceil 百分比换算参与是否写入的判断。

设备适配器来源是 `.analysis/inputs/frontend/gamepad-mw-2636-current/raw/apps.razer.com/synapse/products/2636/mw/rzDevice25Sage.b9b57f19edfe6aa9f049.js`，模块 50207 bytes `[549,4807)`。**2676 虽包含共享 70784，却挂载 SagePC 并明确 `supportsLighting:false`**；SagePC 78786 继承 98773，而非声明扩展静态 methods 的兄弟 Sage 50207。不能给 2676 添加 2636 Lighting 页，也不能猜测兄弟类的命令来解释共享通知中残留的调用。

## 4115：软件灯光、调色板与退出

4115 在当前 bootstrap 中选择 `lightingType:"chroma"`，UI descriptor 带 `isWaveWithSpeed:true`、`WAVE_DIRECTION:"LeftRight"` 和普通 profile menu。该路线是持续的本机效果计算与帧输出，区别于 2636 的设备 preset getter/setter。

完整 chain 在 `.analysis/results/frontend/gamepad-main-audit.json` 的 `gamepad_lighting.palette_profile_chain`、`lighting_engine`、`native_engine`。4115 MW 原件为 `.analysis/inputs/frontend/gamepad-mw-4115-current/raw/apps.razer.com/synapse/products/4115/mw/main.8c755a3c43990d2c3900.js`；产品定义 69427 bytes `[1497444,1499733)`，共享硬件效果 helper 70784 `[1505544,1508787)`，其存在不改变 Chroma 挂载选择。UI 原件 `.analysis/inputs/frontend/gamepad-4115-current/raw/apps.razer.com/synapse/products/4115/ui/static/js/main.1a52fc92.js` 的实际 palette editor/profile save/refresh 消费者由上述 audit 完整保存。

软件引擎以 25 fps 生成帧，将 8×10 周边映射到 1×32 灯，按源码 BGR 转换和无颜色 sentinel 处理；Wave speed 保留小数。设备帧分为 25+7 两段 OUT；该发送路径没有 IN/device ACK，不能编造每帧读回成功。色板编辑、保存 custom colors、刷新 profile 色板和重建 effect handle 是独立动作；profile 内持久化的是颜色与效果参数，不是可跨进程复用的引擎句柄。

生命周期持有 device/engine/effect handle 与输入 bridge。暂停只是停效果/帧发布，不等于销毁 device；恢复重新激活当前 owner；退出执行源码的 spectrum 恢复，再移除效果、释放设备/引擎与通知/COM 资源。旧 generation 帧不得投给新连接。native `RzLightingEngineApi_v4.0.55.0.dll` 在此是分析对象：完整动作分发 RVA 0x1fd90、create device 0x20300、destroy device 0x20c50、add effect 0x21ac0、输入事件 0x27d60、terminate 0x2a5c0 的伪代码与交叉引用由 `native_engine` 记录；不得把它的生产调用描述成独立 Rust 实现。

来源与验证边界：[2636 UI/MW 收据](../../../.analysis/inputs/frontend/gamepad-mw-2636-current/acquisition.json)、[4115 UI 收据](../../../.analysis/inputs/frontend/gamepad-4115-current/acquisition.json)、[4115 MW 收据](../../../.analysis/inputs/frontend/gamepad-mw-4115-current/acquisition.json)。本页只陈述静态源码行为与证据，未运行应用、厂商 JS、校准或灯光写入测试。
