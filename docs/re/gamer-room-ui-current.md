# Gamer Room 当前设备、营销与教程契约

当前Dashboard来源为9388/19388的He→je→ze→fe→S、营销o→b→u、教程Pe，以及55 CSS、50527/v开关和26079 debounce。实际实现为gamer_room.rs、gamer_room_devices.rs、gamer_room_presentation.rs及gamer_room_hotspot.rs，主路径接收workspace中实有Device.sub_devices。

## 设备组与意图

设备分两组，卡186×176、图片100×100，包含名字/skeleton/安装状态、power/offline、online/offline popup、Synapse Override与Settings入口。isSynapseOverride=false进第二组，true或缺席进第一组，显式null不当缺席。父产品780/3880/3858及子产品780按源过滤；对象展开保持字段覆盖方向，缺当前语言name保留skeleton，不拿title替代name条件。普通产品目录不产生IoT设备。

本地产品页已存在时可直接打开，无须伪造installedDevices或READY。Settings携带精确PID/serial/container至产品workspace。Power在mousedown沿500ms debounce尾部形成ON_SET_POWER_STATE_IOT意图，Override形成ON_SET_SYNAPSE_OVERRIDE；没有IoT transport时只报告未发送，不修改isOnline/isPowerOn/isSynapseOverride。

收展保留正文树，max-height0↔2000为300ms ease-in，正文-100%↔0和箭头-90°↔0各300ms linear，overflow按源1秒expand关键帧。内容高度按实有卡计算。help是独立14×14控件、ml5，tip left0/top20/min-width300、8/10内距、14/16字体、300ms透明度；Smart Home使用SMART_HOME_TOOLTIP，背景按预乘sRGB/alpha过渡。

## 营销与热点

横幅是否显示由两组为空条件决定。产品名#fff、18px RazerF5/bold，Learn More用当前21×20 inline外链path，文字/图标hover变绿。热点hover打开，离开产品面板关闭，只有面板点击进入真实官方产品URL；不锁定热点点击状态。

面板min-width400/min-height312/padding36/24，仅双灯说明min-width284、双卡gap36；按实测盒子保留-23.5%/-86.5%、-50%变换及原横幅/滚动裁剪，不用通用popup窗口钳制。热点blur5、面板backdrop blur30尚未实现。

原gamer_room_hotspot_animation.53dd5566.svg按当前manifest内容指纹离线恢复，SHA-256为16ea857d1571db9eb72fbe03d580373c1c780618fdf74574aa89bd8a7e6b9e88。这是当前内容指纹匹配，未声称独立live下载比对。原图固有36×36、中心18/18，只绘可见白色无填充ellipse；fill-opacity0和空time_group不变成额外图层。

周期1.3333333秒循环，keyTimes 0/.25/.625/1对应radius 2/10/17.5/17.5、stroke 2/4/1/1、opacity 1/.2/0/0；三段保留原keySplines .333 0 .833 1、.167 0 .833 1、0 0 0 0。最后37.5%不可见，矢量居中描边保留小数。热点z1、负10重叠、营销面板z2；减少动画为本地首帧暂停适配。

## 教程挂载与定位

He的dashboard.flex行内width:unset，je的子树纵向无gap；ze逗号表达式丢弃boxGroup，因此marginTop30不生效。Pe relative wrapper高度0，位于第一组10px上margin之前；面板默认left220/top22，第二步left213/top-25。面板宽290、边1、padding40/20/20、内容宽248；250×190媒体按宽缩放，高度依文字/媒体流，不给两步同高。

指示器36×36相对面板padding box：第一步left-9%/top0，第二步left-10%/top50%且垂直居中；面板自身没有垂直居中或窗口碰撞翻转。设备容器正常min620/max1220，≤1279最大910，≤600最大290/min0，≥2560最大2500，margin-bottom50；教程随该容器，不叠加失效组偏移。

受控Popover保留焦点捕获/恢复及覆盖登记，面板最后绘制并遮挡下层指针，滚轮交给正文；不添加8px边缘吸附、独立max-height或滚动区。第一页Previous禁用、Next切第二步；第二页Previous回第一步、Done完成；Skip/Done标记本地已读并关闭，外点不完成。Add设备模态暂时隐藏教程，关闭恢复原步骤。教程z1050，高于设备popup21/help100，使用当前两项媒体。

## 剩余项与证据

原生installedDevices/noAliveSignPage、安装继承及更多源属性缺真实服务输入；srcSet多分辨率、非ASCII localeCompare和完整Unicode capitalize未实现。组收展只在实体内保存，groupsCollapsed存储未接。中途动画逆转、跨弹层层级、shrink-to-fit、视频inline基线、中文normal行高、滚轮/焦点/窄短窗和实际像素仍待验收。

[当前组件/CSS](gamer-room-current-evidence.json)由audit-gamer-room-current.cjs维护；[热点XML与轨道](gamer-room-hotspot-current-evidence.json)由extract-gamer-room-hotspot.py维护；[离线恢复事实](current-media-offline-recovery.json)由recover-current-media.cjs维护。资源准备使用gamer_room_assets.py；[教程媒体清单](../../assets/synapse/tutorial-media-manifest.json)保留转换来源。现有gamer_room_tutorial_tests.rs覆盖真实wrapper/容器/面板位置与切步，仅编译，不运行。没有应用、下载JS、安装器或DLL执行验收。
