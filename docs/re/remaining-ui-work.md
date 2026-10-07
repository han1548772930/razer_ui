# 当前剩余工作

更新：2026-10-07。按 [实施路线](ui-readonly-first-roadmap.md)逐项推进，完整复刻尚未完成。最新批次见 [续接记录](ui-continuation-2026-10-07.md)：接收器/底座、预览名称及手柄扳机局部修复已静态验证；179 实际自动发现、配对工具后续状态和 241 完整条件仍在队列。

1. 收齐 Studio Ambient、Static 的交互边界；Spectrum 时长和渐变、[Breathing/Fire 本地编辑](chroma-studio-breathing-fire-native.md)已局部接入，继续核对[弹层、光标与服务差异](chroma-studio-gradient-native.md)，并逐项接入其余 8 个属性根及共享控件，保留设备条件和临时参数语义。
2. 补齐 Studio 分组、拖排、配置、导入导出、关联与独立模式，见 [根审计](chroma-studio-native-root.md)、[属性审计](chroma-studio-properties.md)、[图层审计](chroma-studio-layers-native.md)。
3. 依据 [DLL 只读盘点](dll-readonly-inventory.md)核实实际 DLL 身份、导出和 ABI，补全查询、观察与 UI 消费链。静态封装证据不等于实际读取成功。
4. 逐产品检查控件、弹层、条件、动画与独立根；[331 个产品、1419 个主导航页](native-product-coverage.md)已有部分内容，完整验收产品为 0。
5. UI 编辑、增删、应用、保存和本地草稿现在实现；通过 DLL 修改状态、写回和保存最后统一接入。

Studio 原生取色、区域选择、设备 LED 数据尚需服务证据，自定义颜色仅会话保存。Settings 宿主目录、托盘真实账户与通知、OLED 运行数据仍有缺口，见 [Settings](settings-window-implementation.md)、[托盘](tray-account-continuation-2026-10-06.md)、[OLED](audio-oled-runtime-current-audit.md)。

只允许静态解析、资源验证、格式化与 `cargo check --locked --all-targets`；应用运行、像素、DPI、焦点和真实设备响应未验收。批次详情见 [续接记录](ui-continuation-2026-10-06.md)。

独立复核确认的 3334/3337 混音组合禁用链已修复并静态核实。716 Copilot 状态行也已补入。混音预设编辑流、Settings 双击动作下拉、1382 Audio Function 的真实 EQ 设备观察/完整父映射根/应用级未保存保护、鼠标 70 DPI stages 条件/拖排仍需修复，见 [独立报告](prior-ui-verification-2026-10-06.md)。


Settings 双击动作已补 showMenu 空目录分支、本地保存和托盘读取；仍需真实目录、启动应用分支及剩余视觉/交互差异。独立复核 D 继续按局部修复记录。

鼠标 70 DPI 已依据 [当前收据](mouse-70-dpi-current-evidence.json) 接入逐行 X/Y 编辑、阶段关闭时仅显示当前行、可见性开关与拖排活动阶段重映射。已按独立回读修正 XY 关闭的独立分支，并补悬停操作和方向插入提示；[数字编辑器与 XY 提示](mouse-70-dpi-native.md)已局部接入；窗口级滚轮、精确控件样式/动画、拖动预览和边界继续待办，F 保持 partial。Settings 外部收起 active 的 100ms 时序反馈已修正，其余 D 缺口保持。

独立 G 的 [4115 Kitsune 设备预览、原列布局和 SOCD 说明/配图](kitsune-native.md)已局部接入；提示、独立 macro 条件与真实读取继续待办。独立 H 的 3592 快捷键禁用已补渲染与所有写入口检查，已获独立静态回读确认；见 [独立报告](prior-ui-verification-2026-10-06.md)。

独立 I：2636 低死区警告已接入零/低文案、Continue、关闭回退、Recalibrate 父工作区导航和展开说明动画；独立静态复核确认操作链，主页面布局、弹层输入拦截范围及运行时读状态仍为 partial。Kitsune 三个帮助提示已接入，剩余实际容器边界；Macro 入口资格按当前运行时能力另审。

1382 [Playback 子编辑器](control-pod-audio-native.md)已接只读 AudioDevices 数组、临时编辑、断开回填、None 和本地 Save/Cancel；Reset/旧编辑器覆盖保护与可选 payload 恢复已补。真实 EQ validDevices 及完整父映射 UI 仍待完成，DLL 写回继续后置。

1382 编辑器自身 × 的未保存提示已按源接入并获独立静态复核：Save提交后关，Don’t Save清dirty但留在子编辑器，提示×保留dirty；后续已接现有音频输入之间及产品内部页/历史导航保护；原映射根、其他类型及应用级跳转保护仍待完成。


1382 EQ 的条件设备下拉、三种动作、五种预设及本地 equalizerPayload 已接入并获独立静态复核。合格产品存在与匹配播放端点非空分别判断；索引失效时普通/提示 Save 保留草稿并报错。仅新增真实 validDevices 的观察边界，宿主发布者尚未连接，不能计为读取完成。上述局部进展不改变 331 产品 / 1419 主页面 partial、完整产品 0 的口径；DLL 修改状态、写回和保存仍统一后置。


3334/3337 混音初值已按独立复核修正为当前 streamMixerReducer 的主开关关闭、总线静音/0；生成器增加初态与 selector 原文收据，既有本地草稿仍保留。数字直接编辑、冲突开启确认、设备选择/离线提示、输入通道增删和预设快捷键保持未完成，详见独立报告 B 的第二批复核。该修正不表示真实设备读取或混音完整界面已完成。


3334/3337 两条混音总线已补入[数字编辑与步进](stream-mixer-number-native.md)：文字暂存、Blur/Enter/Escape 提交、源字符串加号语义、主开关与总线组合禁用、300ms 长按及本地草稿同步。原窗口级滚轮、完整行布局及边缘交互保持 partial；设备选择、通道增删、冲突确认及预设操作未因此计为完成。6 条 AST/4 次资源比较、cargo check 与静态数据校验通过，未运行应用、测试或 DLL。


## 2026-10-06：混音确认/播放选择与独立复核 J–L

3334/3337 已接入 [开启冲突确认与播放设备选择](stream-mixer-selection-native.md)：确认前不提交、取消/确认、本地选择保存、断开名称回填、主开关和过期菜单确认保护，以及原警告资源。运行列表/冲突状态与本地覆盖分开，未选择不把观察值写入快照。28 项当前 AST 收据、4 次产品资源比较、cargo check 和相关静态校验通过；真实 MW 发布者未接，不能计为 DLL 读取完成。完整布局、电平表、输入通道增删、预设/快捷键和窗口级滚轮继续待办；产品仍为 partial。

用户要求的独立子任务已完成 515 Blackwidow Chroma 本批复核，新增 [报告 J–L](prior-ui-verification-2026-10-06.md)：Gaming Mode 缺少 Menu 禁用状态行；Snap Tap 成对编辑、新增/删除与校验主体未挂载；Keyboard Properties 右列与操作入口缺失。已核对正常挂载链、源码范围和本地实现，不把共享组件存在视为界面已完成。其中 J 的 Menu 只读状态行已按 515 和 KEY_APPLICATION 条件补入，checked 取 isWindowsKeyDisabled，没有新增写入口；K/L 的 Snap Tap 与系统属性入口继续进入后续修复队列。只读接口继续核实，DLL 写回仍后置。


### 2026-10-06 515 / 226 复核续接

515 [Snap Tap](keyboard-515-snap-tap-native.md)主体、本地增删/校验/快照和 [Keyboard Properties](keyboard-515-properties-native.md)右列命令已接入。仍缺数字/标点/Enter/修饰键等精确输入、真实 Snap Tap 观察发布者、源输入服务、全部帮助/宿主生命周期/视觉验收；不计为完整产品或读取完成。独立子任务已核关键状态链和当前源码收据。

226 [滚轮主体](mouse-226-scroll-native.md)已接三模式、禁用最多两项与选中回退、两组 0–4 等级和 FreeSpin 派生锁定；保留原 enabled，等级释放后才进入本地草稿。独立字段集合避免默认快照阻挡真实观察，Help 切换清理已补。52 项 AST / 83 条 CSS 收据及允许的静态检查通过；真实观察发布者、整页布局和运行视觉仍未完成，不计完整产品或读取成功。

独立报告新增 226 Performance 的 N/O：DPI 逐行阶段条件/可见性/拖排、分段滑条、数字编辑归一化，以及 Polling Rate 的连接目标字段、BLE 隐藏、真实观察限速和提示仍待修复。不能把 70 专用控件或共享组件存在计为 226 已接入，未知连接也不能默认冒充已确认 wired/dongle。

N 的[整数框提交与步进](mouse-226-dpi-number-native.md)现已局部接入：空值/负号/越界 Blur 同步规范文本、值和滑条，Enter/Escape、注册预览、方向键/300ms 步进及当前资源。70/226 共用控件已按 schema 适配并明确关闭自动 mask；16 项 AST、55 条 CSS 和两次资源比较通过。完整 stage 行、分段滑条、窗口级滚轮、数字框精确样式/全部边界及 N/O 其余问题仍未完成。

后续 N 的[阶段行与分段滑条](mouse-226-dpi-native.md)主体已接入并由多个子任务分别回读：visible/拖排/XY、段内取整与动态步长、Preview/Commit、原位释放、Tab/辅助功能同步及 OTFS 编辑锁。真实发布者、浏览器动态量化、窗口输入/视觉及框外数字滚轮仍待办。当前 MW 缺失源已静态补齐，O 的轮询连接字段/BLE/限速说明已得到更完整生产者证据，下一批实现；完整产品仍为 0。
