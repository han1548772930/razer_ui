# ARGB 当前界面契约

范围为 778/3871 有线 ARGB 与 3884/3886 无线 ARGB 的 Customize。依据为各自当前产品包的挂载组件、reducer、CSS 和资源，不从兼容目录或共享组件存在推定页面完成。维护证据为 [有线](wired-argb-current-evidence.json)、[无线](wireless-argb-current-evidence.json)及各自资源 manifest。

## 页面、观察与本地草稿

[WiredArgbWorkspace](../../crates/razer-pages/src/features/wired_argb.rs) 和 [WirelessArgb](../../crates/razer-pages/src/features/wireless_argb.rs) 已由 SourceProductWorkspace 挂载。正式页尚无端口发现/电源/保护/连接状态服务 adapter，观察缺失时明确不可用；显式开发预览只提供样例，不进入实时设备列表，不产生设备成功或设备保存。

端口布局存于设备级 source_device_settings，独立于 Lighting profile。当前源的 portsReducer / ON_SET_PORT_VALUES 管理布局；DEFAULTPROFILE 或 loadActiveProfileSettings 不加载 ports。切换灯效配置不能重置物理布局。本地 schema 不代表已经证明原服务的磁盘存储格式。

有线草稿保存名称、strip/fan 模式、两套分段、Chroma 提醒关闭状态及可选的自动检测请求。无线 snapshot 仅保存 ports；notice dismissed 字段不序列化，restore 会重置它并为分段重建稳定身份。检测数量、active ports、最大能力、电源、保护、连接、刷新结果和安装状态不进入草稿。restore 校验已知端口、字段、长度与数值，不能把保存的数据还原成真实观察。

| 产品 | 当前挂载与差异 |
| --- | --- |
| 3871 | 130×270 控制器图，4–6 端口在左、1–3 在右，1024px 条件移图到列上方；无电源/保护/无设备/刷新分别呈现 |
| 778 | 主板居中，端口 ID 2147483651/2147483652/2147483656 过滤排序，600px 卡片；edition 0/128/129 使用各自源图；无 3871 检测/刷新图标入口 |
| 3884 | 260×260 产品图，电源/自动检测/刷新、待机、Bluetooth/mobile sync、保护、数量警告；允许多风扇 |
| 3886 | 另有 DC 供电提示；源 TopViewComponent 的 `!u.type === BLE_MOBIL` 比较 Boolean 与数值 3 恒为 false，因此普通页不挂端口卡片；显式预览可检查已定义但不可达的编辑器，不能计为正式页面覆盖 |

无线无电源分支未给共享 warning 组件传 overlay，提示保持普通文档流；不能因 CSS 有 warning-container--overlay 就制造弹层。有线控制器总量警告按实际挂载位置显示在顶部 20px 的覆盖层。

778/3871/3886 的 isEnableProfileBar 只控制非 Lighting 页的同步图标；普通 ProfileBar 是否禁用由 enableSwitchProfile 等实际消费者决定。3884 则由根组件在非 Lighting 页派发 setProfileDropdownState(false)，使下拉禁用。四者都保留 ProfileBar，不能统一隐藏或一律禁用。

## 端口编辑规则

有线名称 Enter/失焦提交，空名拒绝，UTF-16 长度上限 32，Escape 取消。strip 最多四段；加弯折递归 ceil 分配并保留最小值规则，删除不重分配，第四段不超过第二段，联动相等的第二/第四段。切换 strip/fan 保留两套值；新增风扇保留旧值并追加 20，总量达到观察到的上限时禁用。

有线最小 LED 为 1，只有观察到 per-port maximum=80 时提供 fan 模式；选择值为 6/8/9/10/12/15/16/18/20/22/24/25/32/40。端口上限警告与控制器 240 LED 警告分开。关闭警告不修改计数或硬件，改变布局重新启用 Chroma 提醒。

3884 最小值 1、每端口上限来自真实能力、多风扇每次追加 20，并按实际 active ports 计算 240 总上限。3886 最小值 4，数值编辑上限 max(detectedLedCount,40)，警告门槛 120，即使原字符串写 80 也不擅自归一化；它只保留一个 fan。两者本地数值为整数，3886 源数值控件允许小数的差异仍未处理。无线正式编辑目前还受 preview/ports_visible 门控，真实观察接入和正式可编辑链需继续完成。

有线按住步进已实现：立即一步，每 300ms 重复，到边界、释放、移出或代际失效时停止，不附带设备写入。无线额外分段的删除图标只在对应行 hover 时挂载，左移 10px；3884 的 add-bend 使用 1px 伪元素等价线及绿色 hover，3886 用自身下划线且不添加源不存在的绿色 hover。

## 提示、资源与运动

两类端口帮助使用当前 widget.help / widget.tip：14px 圆点在 top/right10px，背景 300ms 过渡；提示按共享 surface::help_control 的源几何呈现，不能替换成默认 Kit 延迟提示。

3871/3884/3886 检测和刷新提示已接入即时 hover 挂载及 100ms 淡入，无展示延迟；bottom-left 为目标右缘对齐、下方 5px。LED 数量提示使用 bottom-right 左缘对齐，只把数量着绿。共享实现为 [hover_tip.rs](../../crates/razer-widgets/src/hover_tip.rs)，完整窗口边界、遮挡和实窗命中仍需验收。

有线检测图保留原 SVG 路径、mask 及内嵌位图，拆出 2 秒环旋转和 18 条 LED 透明度轨道：0.5 秒亮、0.5 秒灭、0.1 秒错开、1.8 秒循环，减少动态效果时静止。环使用离散帧，不宣称连续像素等价。

有线 AutoDetectionIcon 已有按代际触发的 100/700ms 动画、50ms 阶段及 300ms hover 插值，但当前 Rust 动的是整张图片的 opacity；源 detect-b/c/f 的独立缩放/扩散尚未等价实现。不能以相同时间常量声称完整图形动画完成。

无线源交互图已按实际着色层拆分：检测 hover/active 为 #44d62c/#39a029，启用环 hover 为#96ef89，电源、关闭、警告和删除各保留自己的源色。着色目标没有对应过渡时立即切换。自动检测逐层实现 50ms glyph .8→1、700ms 绿色 .4→2 及灰色 .2→1.4 扩散、100ms 启用 glyph opacity0→1/scale1.8→1，使用源 ease-out；首次未操作图保持静止，reduce_motion 保留静态层。这些动画不生成服务观察。

## 验证与未完成范围

extract-wired-argb.cjs / extract-wireless-argb.cjs 的 --check 核对当前挂载、字段和文案；validate-wired-argb.py / validate-wireless-argb.py 核对源 hash、精确区间、源图、派生层、提示和原生标记。资源准备工具只处理静态资产。当前核验通过不代表像素、焦点、缩放、弹层/滚动和连续输入已运行验收。

仍需真实只读状态 publisher、按产品完成正式编辑/本地 Apply/Save 消费链、Chroma 安装/同步状态和剩余控件细节。UI 操作与本地草稿保持当前范围；DLL 修改与设备/服务写回尚未接通。没有运行应用、测试、下载的 JavaScript 或 DLL。
