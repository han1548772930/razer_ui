# 当前全部产品 UI 原文细节与代表页语义核对

当前源快照为 2026-10-02；此文档由维护工具静态解析当前产品包，不执行原 JavaScript。它补全 [全产品页面调用链](all-product-page-chains-current.md) 的组件内部结构证据，和 [布局/样式索引](all-product-layout-chains-current.md)、[实际样式加载与字体](ui-style-sources-current.md)、[共享控件审阅](shared-ui-controls-current.md) 配合使用。

## 覆盖与完成口径

已保留 331 个产品 / 1452 个页面 / 52461 个页面组件引用；产品内去重后 40922 个原组件范围。共 353741 个 UI 调用、865110 个直接对象属性、12290 个 helper 对象属性候选、63749 个事件属性、187719 个 class/style 属性、283498 个文案候选、345273 个函数定义、93492 个状态/订阅调用、917928 个条件节点、34321 个循环/迭代节点。

**这些数值是原文范围及语法证据覆盖，不是所有页面已经语义逆向完毕。semantic_complete_pages_claimed=0。** 本节后的 14 个原文契约只覆盖 226 鼠标、691 键盘、1383 耳机和 164 底座的明确分支。未宣称当前 Rust UI 达到原版或 DLL 调用成功；未运行应用、DLL、构建或测试。

## 可回查的数据结构

- [轻量索引](evidence/product-ui-details-current.json.zip)：产品/页面/组件 IDs、页面根、原调用链 edges、unknowns、统计与语义契约；范围按 UTF-16 零起点左闭右开。
- [完整 gzip JSON](product-ui-details-current.json.gz)：每个组件的原文 source、路径、字节 SHA-256，以及每项嵌套证据的 offset/end/node_type。每行一个产品记录，适合流式读取；解压后 1605130343 字节，不应一次加载全部到内存。
- ui_elements 按原调用出现次序记录 callee、组件表达式、完整 ordered props/spreads、jsx 第三 key 参数、原 children 的数组/分支/迭代顺序、事件、class/style 和文案表达式。createElement 的 children 来自第三及后续参数。
- lexical_parent_element 仅是 AST 的嵌套关系；opaque props/helper_object_arguments/helper_children_candidates 不推断合并顺序或最终 props。静态 literal 使用 static_value，不覆盖原 value 范围。
- function_definitions/local_definitions/methods/this_assignments 保存组件内声明与回调候选；state_calls/conditions/loops 保留状态及条件原式。函数 returns 排除嵌套函数，不把回调 return 当父函数 return。
- pages 的 component_detail_ids 对接原 graph_edges；root_component_detail_ids 只从 from=null 的真实根取得。页面 shared 范围在每个产品内去重，不丢失页面引用。

完整数据字节 SHA-256：`db9209d9de5991e19fc29b3ae6f6d21f4ca5a0ad0893facd3a4ab48663e1cb1c`；解压内容 SHA-256：`f555d1bfcaa6895d51af30bf98aa5a3993b4eaf53ec45be86c3ab7a5b490f06d`；输入调用链 SHA-256：`c389f0eb97dca9cb4411687f3be5a646579943de1913c347e0837a9996df2925`。

## 明确未解决的边界

- UI calls are recognized by JSX/jsxs/createElement syntax; the graph retains module proofs, while this document does not recompute every call provider or execute React.
- Lexical containment is not runtime DOM parentage. Element children retain source order/conditions/iterations; component expansion needs passed props, HOC behavior and state observations.
- Every original prop and spread is preserved by range. Props supplied as arbitrary expressions/helper calls remain opaque; object arguments are candidates, not proved merge/override order.
- Local callback references and this-member assignments are candidates within each receipt, not all module/global declarations or guaranteed control-flow values.
- Text props and literals include localization expressions without guessing localized output. Full locale/import binding and rendered values remain separate work.
- Definitions/state/conditions/loops are original syntax; API names do not prove DLL read/write meaning, and handlers are not executed.
- Only the prior bounded graph is expanded; unresolved graph targets, dynamic imports and unreferenced/unsupported modules are not silently covered.
- No app, vendor JS, DLL, build or tests executed. UI operations, DLL mutation/write-back and persistence are all required in the current full implementation scope; this source index alone does not establish their completion.

模块外变量、传入 props、HOC、动态 selector 数据、computed 名称、CSS cascade 和 localization 输出都不能只凭本索引认定已闭环。所有 graph unresolved 保留；包括 React Fragment 条件赋值不能假设 Symbol 环境选分支。原 callbacks 不等于实际用户动作成功；UI Apply/Save 和本地 draft 操作仍在范围，DLL mutation/write-back 仍留待协调集成。

## 代表页原代码语义契约

### 226 · 鼠标 Performance 根布局与 BLE 门控

证据 ID：`mouse-performance-columns`；`.ref/devices/226/static/js/8355.3d5e573e.chunk.js`，UTF-16 `[183668,183979)`；SHA-256 `b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- BodyWidget 的 left 先挂 ys；right 顺序为 e&&Xs、yi、ei。e 等于 !isBle || DeviceInfo.supportBluetoothPollingRate===true，因此 BLE 支持条件只影响这处 Xs 插槽。
- 这里只证明原组件排列和条件；左右列最终宽度、边距、子组件含义必须继续沿组件证据与 CSS 链核对。

### 226 · 鼠标 DPI 切换、阶段与双轴联动

证据 ID：`mouse-dpi-controls`；`.ref/devices/226/static/js/8355.3d5e573e.chunk.js`，UTF-16 `[149688,154848)`；SHA-256 `b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- Widget 子元素按说明、双向 tab 或 stage-control、动态 disabled 主体排列；twoway-lighting no-inner-border 追加动态 class，普通 stage-control 原样带 zIndex:2。
- 缺少或空 dpiStages 时返回 null。阶段网格还依赖 selectedProfile；常规 maxDPI/minDPI/step 分别存在 16000/100/50 的回退，singleDPI 使用 minDPIConfig/maxDPIConfig。
- 关闭独立轴时 y 复制 x；changeX 在未独立时同步 y。changeX/changeY 接收的第三参数会抑制 dispatch，不能省略该条件。
- 禁用当前阶段会选下一可见阶段或首个可见阶段；updateStages 会拒绝非数组及空洞并维护可见回退。阶段关闭和 two-way 分支还含夹取及 updateDPIStage。
- componentDidMount 关闭 configure sensitivity 状态；enableStages 变化会命令式修改 .stage input.slider。不能只把 JSX 描述当作全部交互。
- setActiveDPI/活动索引必须保留原表达式，本审阅不擅自纠正看似异常的索引。

### 226 · Sensitivity Matcher 状态、弹层及消息边界

证据 ID：`mouse-sensitivity-matcher`；`.ref/devices/226/static/js/8355.3d5e573e.chunk.js`，UTF-16 `[176795,182999)`；SHA-256 `b5d160fe0c231186d7f8271d13ece17a3824b0c35c4962cf2ecec72c46352eb8`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 打开会登记 window click；外部点击根据 popup 和 ref containment 判定并发消息、移除监听。关闭时按 calibrationState.state 决定 cancel/reset；卸载也清除监听。
- READY、CALIBRATING、ERROR、COMPLETED 分支决定内容顺序；进度宽度为 process%；完成表格按 currentDpi、newDpi 排列，reset 可点击。
- 原弹层 minHeight:569px；动作区 marginTop:120px；Apply 在非 COMPLETED 时追加 disabled 类。ERROR 还挂 retry/cancel dialog。
- cancel 方法比较整个 this.props.calibrationState 与 CALIBRATING，其他位置比较 .state；这是原文差异，不能改写成统一正确行为。
- Apply 关闭并发送消息，不证明设备保存成功。消息常量 ui/mi/Si/vi/gi/Ci 尚须沿模块定义解析，不在这里猜测 DLL 命令。

### 691 · 键盘 OLED 根布局及编辑门控

证据 ID：`keyboard-oled-root`；`.ref/devices/691/static/js/OLED.b7b95581.chunk.js`，UTF-16 `[98640,99355)`；SHA-256 `ea0480e7c17180944b2aac3d4287abba9eeec3338c54e0327d55e370643deee6`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 根组件保存 popup=false 和 selectedEdit=null。BLE 只阻断 animation/image/emote/banner 编辑入口；progress 取 oledLoadingReducer.oledLoading.type。
- 外层先 Wi、ta；内层先 preview di，再 left[hi,wi]、right[xi,Gi,ea]；可选 pi 弹层收到 selectedEdit/show/onClose，关闭同时清理两个本地状态。
- 页面的 main 异步根仍负责加载/失败分支、result.default 与清理。此记录不运行 lazy loader。

### 691 · 键盘 OLED 首页预览、轮播与应用

证据 ID：`keyboard-oled-home-preview`；`.ref/devices/691/static/js/OLED.b7b95581.chunk.js`，UTF-16 `[79229,86040)`；SHA-256 `ea0480e7c17180944b2aac3d4287abba9eeec3338c54e0327d55e370643deee6`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 读取 home enabled/selected 及 animation/image/emote/banner/media/system/OLEDlang；slides 仅保留 left||right。数量变化重置索引，触发轮播按 (idx+1)%R 与 timeBetweenSlides*1000 更新，清理 interval。R 为 0 的余数问题只记录为原表达式，未作运行结论。
- 首页 tile 顺序是 animation(0)、image(1)、emote(4)、banner(2)、media(5)、system(6)、keyboard/headset(3)。media/system 需要 Synapse 且 BLE 禁用；产品专用 tile 不可编辑。
- home 禁用时叠加 ci；loading 改变外层 disabled class。巨型 base64 产品图仍保留在完整原文范围，MD 不复印图像编码。
- type 6 的 Apply 克隆 system；OLEDlang===1 时用 getTextItem 的 _lang:"zh-CN" 写 deviceLabel，否则取信息标签并 dispatch m；其他类型 dispatch d({enabled:x,selected:e})。这些是 UI action，无 DLL 写回成功结论。

### 691 · 键盘 OLED Dim Display

证据 ID：`keyboard-oled-dim`；`.ref/devices/691/static/js/OLED.b7b95581.chunk.js`，UTF-16 `[95389,96010)`；SHA-256 `ea0480e7c17180944b2aac3d4287abba9eeec3338c54e0327d55e370643deee6`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 选项来自 fi.OLED_DIM_DISPLAY_VALUES||Ji；点击发送 SET_OLED_TIME_TO_DIM，payload 为当前 enabled 和新 value。
- 按钮容器 marginTop:"20px"；!enabled 时叠加遮罩。active 的 concat(e===s&&"active") 可保留 false 字符串，不能静默规范化成空串。
- Ji 的已确认数组回退是 [1,3,5,10,15]，并不证明每个产品配置都采用该值或时间单位。

### 1383 · 耳机 Sound 根布局与状态同步

证据 ID：`audio-sound-root`；`.ref/devices/1383/static/js/6141.5d00192e.chunk.js`，UTF-16 `[415171,416558)`；SHA-256 `de6d49fed2644791427dac537113bf6f66f29f9248fdc3dd7a4b10c350f3e969`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- kr mount 调用 getSettings；THX 改变时同步本地 isTHX；切换 setTHXSpatialEnable 的参数为 !this.state.isTHX。
- 根顺序为 customize-audio flex 容器(Pr)、chart-eq(Wo,In)、eo.A 两列。left 为 mr，right 为 Ao(showGameChatBalance:true)、jo。
- Wo 固定 isStereo:true；In 收到 disableRequiredSynapseForCustom:true、secondTooltip:true、compactMode:true 和 te 中 bandDataConfig/eqChartOpt/yAxisTitle。不能以普通双列控件替代顶部两块。
- 外围 connector 把 spatial/eq/customize 状态与 bindActionCreators 注入 kr；这不证明每次调用都实际到达 DLL。

### 1383 · 耳机音量本地状态及禁用条件

证据 ID：`audio-volume`；`.ref/devices/1383/static/js/6141.5d00192e.chunk.js`，UTF-16 `[284339,285904)`；SHA-256 `de6d49fed2644791427dac537113bf6f66f29f9248fdc3dd7a4b10c350f3e969`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- local volume 初始化自 props；toggle 的 setState 回调调用 setValues→props.setVolume；changeValue 以 !!value 更新 enabled，0 会关闭 enabled；props 变化时 componentDidUpdate 同步。
- 仅 inputSource 属于 aux/bluetooth/optical 且 canDisabledVolume 时 disabled。Widget 使用 hasSwitch、local active、Slider 0..100/step 1/prop debounceTime；customTips 存在回退。
- externalSoundProperties 点击调用 j.MN()；showGameChatBalance 条件挂 wo。changeVolume 的存在不证明该处理器被某个控件实际使用。

### 1383 · 耳机 Game/Chat Balance

证据 ID：`audio-game-chat`；`.ref/devices/1383/static/js/6141.5d00192e.chunk.js`，UTF-16 `[282848,284155)`；SHA-256 `de6d49fed2644791427dac537113bf6f66f29f9248fdc3dd7a4b10c350f3e969`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 本地 balanceMode 初始化自 props；changeValue 在 setState 回调调用 setGameChatBalance；props 变化同步；tutorial 使用 showGameChatTutorial:true。
- 标题行 style 为 marginTop:"10px"、display:"flex"、alignItems:"center"、zIndex:0、gap:"6px"；标题 uppercase，其后 help icon/tip。
- Slider min:0/max:config.max||20/step:1。教程链接 marginTop:"45px"、textDecoration:"underline"。V.* 文案键尚不当作最终翻译。

### 164 · 接收器/底座多设备配对 iframe 边界

证据 ID：`receiver-pairing-frame`；`.ref/devices/164/static/js/main.458d4103.js`，UTF-16 `[4582679,4585320)`；SHA-256 `0324ddd7980caf41f07eeea62979c15579578cba2a13ac302a8347e6d338fb40`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- eD props 含 widgetHeight(默认"100%")、iframeId、deviceInfo、deviceName、allMasters(默认[])；QC.Zv 归一 product/category/boolean flags/string dongleId，JC.eA 归一 masters。
- 同源 URL /synapse/multipairing/ 带 displayMode=multiDevicePairing、可选 containerId、productId/pid、category、配对/productivity flags、deviceName、serialNumber、lang、allMasters JSON。
- 发送 multiDevicePairingInit payload={deviceInfo:r,deviceName:t,allMasters:T}。Ready listener 同时检查 event.source===iframe.contentWindow、event.origin===targetOrigin 和 eventName===multiDevicePairingReady。注册/清理 effect、另一个 init effect 及 onLoad 都保留。
- wrapper position:"relative"、width:"100%"、height:E；iframe id 回退 iframeMultiDevicePairingWidget/frameBorder:0，并以 {...$C,zIndex:1} 定位。$C 的原静态对象单列证据。
- 这里是嵌入入口和消息握手，不证明 multipairing 应用内部流程、无线查询或硬件配对成功。

### 164 · 接收器配对导航分支

证据 ID：`receiver-pairing-route`；`.ref/devices/164/static/js/main.458d4103.js`，UTF-16 `[4587157,4587565)`；SHA-256 `0324ddd7980caf41f07eeea62979c15579578cba2a13ac302a8347e6d338fb40`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- renderView 以 active_view 查找导航 name；Q.d9b 分支按 href 缓存 allMasters，随后挂 ED，传 deviceInfo/deviceName/allMasters；不匹配返回 null。
- ED=P().memo(eD) 为独立包装证据。配对 UI 与硬件操作不能因路由存在就标成完成。

### 691 · Dim Display 默认数组

证据 ID：`keyboard-oled-dim-fallback`；`.ref/devices/691/static/js/OLED.b7b95581.chunk.js`，UTF-16 `[95372,99355)`；SHA-256 `ea0480e7c17180944b2aac3d4287abba9eeec3338c54e0327d55e370643deee6`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 独立模块邻接声明的原表达式；只用于上述对应契约，不泛化到其他命名空间。

### 164 · iframe 静态定位对象

证据 ID：`receiver-frame-style`；`.ref/devices/164/static/js/main.458d4103.js`，UTF-16 `[4582587,4585336)`；SHA-256 `0324ddd7980caf41f07eeea62979c15579578cba2a13ac302a8347e6d338fb40`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 独立模块邻接声明的原表达式；只用于上述对应契约，不泛化到其他命名空间。

### 164 · iframe memo 包装

证据 ID：`receiver-memo-wrapper`；`.ref/devices/164/static/js/main.458d4103.js`，UTF-16 `[4585324,4585336)`；SHA-256 `0324ddd7980caf41f07eeea62979c15579578cba2a13ac302a8347e6d338fb40`。原片段完整保存在 JSON 的 `semantic_review_contracts[].source`。

- 独立模块邻接声明的原表达式；只用于上述对应契约，不泛化到其他命名空间。

## 全产品索引

| 产品 ID | 当前源名称 | 页面 | 原组件范围 | UI 调用 |
|---|---|---:|---:|---:|
| 70 | Razer Mamba TE | 5 | 128 | 1364 |
| 80 | Razer Naga Hex V2 | 5 | 138 | 1396 |
| 83 | Razer Naga Chroma | 5 | 138 | 1403 |
| 89 | Razer Lancehead | 6 | 134 | 1381 |
| 92 | Razer Deathadder Elite | 5 | 127 | 1362 |
| 96 | Razer Lancehead TE | 5 | 124 | 1356 |
| 98 | Razer Atheris | 5 | 118 | 1244 |
| 99 | Razer Jugan | 5 | 126 | 1351 |
| 100 | Razer Basilisk | 5 | 128 | 1363 |
| 101 | Razer Basilisk | 5 | 128 | 1362 |
| 103 | RAZER NAGA TRINITY | 5 | 142 | 1434 |
| 104 | Razer Mamba Hyperflux | 5 | 137 | 1419 |
| 105 | Razer Mamba Hyperflux | 5 | 137 | 1419 |
| 106 | D.VA Razer Abyssus Elite | 5 | 127 | 1358 |
| 107 | Razer Abyssus Essential | 5 | 128 | 1358 |
| 108 | Razer Mamba Elite | 5 | 128 | 1364 |
| 110 | Razer DeathAdder Essential | 4 | 104 | 886 |
| 112 | Razer Lancehead Wireless | 6 | 134 | 1381 |
| 113 | Razer DeathAdder Essential | 4 | 105 | 891 |
| 115 | Razer Mamaba Wireless | 6 | 134 | 1379 |
| 116 | Razer Abyssus Lite | 5 | 128 | 1358 |
| 117 | Razer Turret Mouse Xbox One Edition | 6 | 136 | 1390 |
| 120 | Razer Viper | 6 | 128 | 1364 |
| 122 | Razer Viper Ultimate | 6 | 137 | 1487 |
| 124 | Razer DeathAdder V2 Pro | 5 | 136 | 1203 |
| 126 | Razer Mouse Dock | 2 | 67 | 589 |
| 128 | Razer Pro Click | 4 | 100 | 872 |
| 131 | Razer Basilisk X Hyperspeed | 5 | 106 | 890 |
| 132 | RAZER DEATHADDER V2 | 5 | 132 | 1481 |
| 133 | Razer Basilisk V2 | 5 | 131 | 1474 |
| 134 | Razer Basilisk Ultimate | 6 | 137 | 1490 |
| 138 | Razer Viper Mini | 5 | 128 | 1361 |
| 140 | Razer Deathadder V2 Lite | 5 | 128 | 1360 |
| 141 | Razer Naga Left Handed Edition | 5 | 141 | 1522 |
| 143 | RAZER NAGA PRO | 5 | 150 | 1278 |
| 145 | Razer Viper 8Khz | 5 | 131 | 1471 |
| 147 | RAZER NAGA CLASSIC EDITION | 5 | 142 | 1435 |
| 148 | Razer Orochi V2 | 5 | 120 | 1067 |
| 150 | Razer Naga X | 5 | 138 | 1402 |
| 152 | Razer DeathAdder Essential | 4 | 105 | 892 |
| 153 | Razer Basilisk V3 | 5 | 132 | 1497 |
| 154 | Razer Pro Click Mini | 4 | 113 | 1043 |
| 156 | Razer DeathAdder V2 X Hyperspeed | 5 | 118 | 1061 |
| 158 | Razer Viper Mini Signature Edition | 6 | 125 | 1192 |
| 161 | Razer Deathadder V2 Lite | 5 | 128 | 1360 |
| 162 | cobra | 5 | 39 | 527 |
| 163 | Razer Cobra | 5 | 128 | 1361 |
| 164 | Razer Mouse Dock Pro | 4 | 83 | 693 |
| 165 | RAZER VIPER V2 PRO | 5 | 121 | 1133 |
| 167 | RAZER NAGA V2 PRO | 7 | 166 | 1501 |
| 170 | Razer Basilisk V3 Pro | 7 | 149 | 1330 |
| 175 | Razer Cobra Pro | 6 | 142 | 1230 |
| 178 | Razer DeathAdder V3 | 4 | 104 | 952 |
| 179 | HyperPolling Wireless Dongle | 2 | 53 | 525 |
| 180 | Razer Naga V2 Hyperspeed | 5 | 132 | 1111 |
| 182 | Razer DeathAdder V3 Pro | 6 | 128 | 1153 |
| 184 | Razer Viper V3 HyperSpeed | 6 | 127 | 1156 |
| 185 | Razer Basilisk V3 X Hyperspeed | 7 | 156 | 1601 |
| 190 | Razer DeathAdder V4 Pro | 7 | 147 | 1381 |
| 192 | Razer Viper V3 Pro | 7 | 144 | 1480 |
| 194 | Razer DeathAdder V3 Pro Hyperpolling Technology | 5 | 122 | 1178 |
| 196 | Razer DeathAdder V3 HyperSpeed | 7 | 136 | 1264 |
| 199 | Razer Pro Click V2 Vertical Edition | 4 | 123 | 1143 |
| 203 | Razer Basilisk V3 35K | 5 | 131 | 1239 |
| 204 | Razer Basilisk V3 Pro 35K | 7 | 159 | 1468 |
| 207 | HyperFlux V2 Wireless Charging System | 3 | 68 | 633 |
| 208 | Razer Pro Click V2 | 4 | 124 | 1146 |
| 211 | Razer Basilisk Mobile | 6 | 143 | 1222 |
| 214 | Razer Basilisk V3 Pro 35K Phantom Green Edition | 7 | 161 | 1488 |
| 218 | Razer Cobra HyperSpeed | 6 | 142 | 1230 |
| 221 | Razer Boomslang 20th Anniversary Edition | 7 | 122 | 1087 |
| 222 | Razer Viper V3 Pro SE | 7 | 146 | 1439 |
| 224 | Razer Orochi V2 | 4 | 106 | 896 |
| 226 | Razer Basilisk V4 Pro | 7 | 164 | 1620 |
| 229 | Razer Viper V4 Pro | 6 | 138 | 1327 |
| 231 | RAZER NAGA V3 PRO | 5 | 157 | 1444 |
| 235 | Razer Basilisk V4 HyperSpeed | 6 | 85 | 790 |
| 239 | Razer DeathAdder V4 Pro Carbon Fiber Edition | 6 | 142 | 1367 |
| 241 | Razer Mouse Dock V2 Pro | 3 | 67 | 669 |
| 515 | Razer Blackwidow Chroma | 3 | 130 | 962 |
| 521 | Razer Blackwidow TE Chroma V2 | 3 | 133 | 974 |
| 529 | Razer Blackwidow Chroma | 3 | 130 | 962 |
| 534 | Razer Blackwidow X Chroma | 3 | 133 | 974 |
| 542 | Razer Ornata Chroma | 3 | 133 | 974 |
| 545 | Razer BlackWidow Chroma V2 | 3 | 129 | 955 |
| 550 | Razer Huntsman Elite | 3 | 1 | 1 |
| 551 | Razer Huntsman | 3 | 133 | 974 |
| 552 | Razer Blackwidow Elite | 3 | 132 | 967 |
| 554 | Razer Cynosa Chroma | 3 | 133 | 974 |
| 555 | RAZER TARTARUS V2 | 3 | 120 | 1052 |
| 556 | Razer Cynosa Chroma Pro | 3 | 133 | 974 |
| 563 | Razer Blade 15 | 4 | 163 | 1431 |
| 564 | Razer Blade Pro 17 | 4 | 167 | 1502 |
| 565 | Razer Blackwidow Lite | 3 | 116 | 803 |
| 567 | Razer Blackwidow Essential | 3 | 121 | 846 |
| 569 | Razer Blade Stealth 13 | 4 | 163 | 1431 |
| 570 | Razer Blade 15 | 4 | 163 | 1431 |
| 571 | Razer Blade 15 | 4 | 161 | 1401 |
| 574 | Turret Keyboard Xbox One Edition | 4 | 138 | 988 |
| 575 | Razer Cynosa Lite | 3 | 130 | 962 |
| 576 | Razer Blade 15 | 4 | 163 | 1431 |
| 577 | Razer BlackWidow | 3 | 132 | 967 |
| 579 | Razer Huntsman Tournament Edition | 3 | 135 | 985 |
| 580 | Tartarus Pro | 4 | 152 | 1285 |
| 581 | Razer Blade 15 | 4 | 163 | 1431 |
| 582 | Razer Blade 15 | 4 | 163 | 1431 |
| 585 | Razer Pro Type | 4 | 124 | 832 |
| 586 | Razer Blade Stealth 13 | 4 | 163 | 1431 |
| 587 | Razer Blade 15 Advanced Model | 4 | 167 | 1502 |
| 588 | Razer Blade Pro 17 | 4 | 167 | 1502 |
| 589 | Razer Blade 15 Studio Edition | 4 | 163 | 1431 |
| 590 | Razer BlackWidow V3 | 3 | 136 | 992 |
| 591 | Razer BlackWidow X: Tenkeyless | 3 | 124 | 858 |
| 592 | Razer Huntsman Essential | 3 | 124 | 858 |
| 593 | Razer Pokémon | 3 | 124 | 858 |
| 594 | Razer Blade Stealth 13 Base Model | 4 | 165 | 1443 |
| 595 | Razer Blade 15 Advanced Model | 4 | 169 | 1514 |
| 597 | Razer Blade 15 Base Model | 4 | 165 | 1443 |
| 598 | Razer Blade Pro 17 | 4 | 169 | 1514 |
| 599 | Razer Huntsman Mini | 3 | 135 | 985 |
| 600 | Razer BlackWidow V3 Mini HyperSpeed | 5 | 154 | 1176 |
| 601 | Razer Blade Stealth 13 | 4 | 165 | 1443 |
| 602 | Blackwidow V3 Pro | 5 | 157 | 1188 |
| 605 | Razer Ornata V2 | 3 | 135 | 985 |
| 606 | RAZER CYNOSA V2 | 3 | 133 | 975 |
| 614 | Razer Huntsman V2 Analog | 4 | 168 | 1338 |
| 616 | Razer Blade 15 Base Model | 4 | 165 | 1443 |
| 617 | Razer Huntsman Mini | 3 | 135 | 985 |
| 618 | Razer Book 13 | 4 | 165 | 1443 |
| 619 | Razer Huntsman V2 Tenkeyless | 3 | 140 | 1020 |
| 620 | Razer Huntsman V2 | 3 | 140 | 1022 |
| 621 | Razer Blade 15 Advanced Model | 4 | 170 | 1528 |
| 622 | Razer Blade Pro 17 | 4 | 169 | 1514 |
| 623 | Razer Blade 15 Base Model | 4 | 165 | 1443 |
| 624 | Razer Blade 14 | 4 | 165 | 1443 |
| 630 | Razer Blade 15 Advanced Model | 4 | 169 | 1524 |
| 631 | Razer Pro Type Ultra | 4 | 140 | 1015 |
| 633 | Razer Blade 17 | 4 | 168 | 1498 |
| 634 | Razer Blade 15 Base Model | 4 | 169 | 1514 |
| 642 | Razer Huntsman Mini Analog | 4 | 174 | 1358 |
| 647 | Razer BlackWidow V4 | 3 | 139 | 1010 |
| 650 | Razer Blade 15 Advanced Model | 5 | 174 | 1584 |
| 651 | Razer Blade 17 | 5 | 172 | 1554 |
| 652 | Razer Blade 14  | 5 | 173 | 1570 |
| 653 | Blackwidow V4 Pro | 3 | 165 | 1422 |
| 654 | Razer Cynosa Pro | 3 | 122 | 851 |
| 655 | Razer Ornata V3 | 3 | 135 | 985 |
| 658 | Razer DeathStalker V2 Pro | 5 | 158 | 1196 |
| 659 | Razer BlackWidow V4 X | 3 | 135 | 985 |
| 660 | Razer Ornata V3 | 3 | 133 | 974 |
| 661 | Razer DeathStalker V2 Pro | 3 | 136 | 992 |
| 664 | Razer DeathStalker V2 Pro Tenkeyless | 5 | 158 | 1195 |
| 669 | Razer Blade 14  | 6 | 185 | 1723 |
| 670 | Razer Blade 15 | 6 | 177 | 1603 |
| 671 | Razer Blade 16 | 6 | 188 | 1746 |
| 672 | Razer Blade 18 | 6 | 188 | 1746 |
| 673 | Razer Ornata V3 | 3 | 135 | 985 |
| 674 | Razer Ornata V3 | 3 | 133 | 974 |
| 675 | Razer Ornata V3 Tenkeyless | 3 | 133 | 974 |
| 677 | Razer BlackWidow V4 75% | 3 | 138 | 1004 |
| 678 | Razer Huntsman V3 Pro | 4 | 180 | 1395 |
| 679 | Razer Huntsman V3 Pro Tenkeyless | 4 | 180 | 1394 |
| 688 | Razer Huntsman V3 Pro Mini | 4 | 180 | 1422 |
| 689 | RAZER HUNTSMAN V3 X TENKEYLESS | 3 | 135 | 985 |
| 691 | Blackwidow V4 Pro 75% | 5 | 247 | 2034 |
| 694 | Razer Blade 14 | 7 | 173 | 1674 |
| 695 | Razer Blade 16 | 7 | 176 | 1697 |
| 696 | Razer Blade 18 | 7 | 176 | 1686 |
| 697 | Razer BlackWidow V3 Mini HyperSpeed | 5 | 154 | 1176 |
| 708 | Razer Pro Type Ergo | 4 | 158 | 1374 |
| 709 | Razer Blade 14 | 7 | 233 | 2160 |
| 710 | Razer Blade 16 | 7 | 231 | 2155 |
| 711 | Razer Blade 18 | 7 | 234 | 2167 |
| 716 | Razer BlackWidow V4 Low-profile HyperSpeed | 5 | 166 | 1247 |
| 717 | Razer Joro | 5 | 148 | 1063 |
| 719 | Razer Huntsman V3 Pro 8KHz | 4 | 191 | 1507 |
| 720 | Razer Huntsman V3 Pro Tenkeyless 8KHZ | 4 | 193 | 1509 |
| 721 | Razer Huntsman V3 Pro Mini 8KHz | 4 | 188 | 1519 |
| 724 | BlackWidow V4 Low-profile Tenkeyless HyperSpeed | 5 | 166 | 1246 |
| 727 | Razer BlackWidow V4 Tenkeyless HyperSpeed | 5 | 164 | 1235 |
| 728 | Razer Huntsman Signature Editon | 4 | 191 | 1542 |
| 736 | Razer Blade 16 | 7 | 234 | 2175 |
| 737 | Razer Blade 18 | 7 | 234 | 2175 |
| 739 | Razer Reclusa X Mini 65% | 3 | 143 | 1121 |
| 740 | Razer Huntsman V3 HE Magnetic Mini 65% 8KHz | 5 | 199 | 1624 |
| 741 | Razer Huntsman V3 Tenkeyless 8KHZ | 4 | 191 | 1510 |
| 742 | RAZER HUNTSMAN V3 PRO LOW-PROFILE TENKEYLESS 8KHZ | 4 | 195 | 1525 |
| 746 | Razer Huntsman V3 HE Magnetic Tenkeyless 8KHz | 5 | 197 | 1613 |
| 747 | Tartarus Pro | 4 | 164 | 1405 |
| 752 | Razer BlackWidow V4 75% XBOX 25 Anniversary Edition  | 3 | 137 | 988 |
| 769 | RAZER PHILIPS HUE | 2 | 57 | 429 |
| 777 | RAZER KRAKEN BT SANRIO LIMITED EDITION | 5 | 83 | 630 |
| 778 | ASRock B550 Taichi Razer Edition | 3 | 65 | 444 |
| 780 | Razer Key Light Chroma | 2 | 66 | 630 |
| 781 | Razer Aether Lamp Pro | 2 | 68 | 643 |
| 782 | Razer Aether Lamp | 2 | 68 | 643 |
| 783 | Razer Aether Light Bulb | 2 | 68 | 649 |
| 784 | Razer Aether Light Strip | 3 | 70 | 631 |
| 790 | RAZER AETHER MONITOR LIGHT BAR | 2 | 70 | 670 |
| 791 | Razer Aether Standing Light Bars | 2 | 61 | 575 |
| 1303 | Razer Nommo Chroma | 3 | 77 | 593 |
| 1304 | Razer Nommo Pro | 3 | 91 | 625 |
| 1306 | RAZER NARI ULTIMATE | 7 | 159 | 1029 |
| 1308 | RAZER NARI | 7 | 124 | 949 |
| 1310 | RAZER NARI ESSENTIAL | 6 | 104 | 777 |
| 1312 | RAZER USB AUDIO CONTROLLER | 5 | 87 | 718 |
| 1313 | Razer Kraken Kitty Edition | 6 | 118 | 943 |
| 1318 | Razer Kraken X USB | 3 | 59 | 410 |
| 1319 | RAZER KRAKEN ULTIMATE | 6 | 122 | 951 |
| 1320 | RAZER BLACKSHARK V2 PRO | 6 | 105 | 795 |
| 1321 | Razer USB Sound Card | 5 | 99 | 772 |
| 1325 | RAZER KRAKEN V3 PRO | 7 | 135 | 994 |
| 1328 | Razer Kraken BT Kitty Edition | 5 | 83 | 630 |
| 1330 | RAZER LEVIATHAN V2 | 4 | 96 | 791 |
| 1331 | RAZER KRAKEN V3 HYPERSENSE | 6 | 125 | 955 |
| 1332 | RAZER KRAKEN BT SANRIO LIMITED EDITION | 5 | 83 | 630 |
| 1335 | Razer Kraken V3 X | 3 | 72 | 588 |
| 1337 | RAZER BARRACUDA PRO 2.4 | 6 | 103 | 766 |
| 1339 | Razer Barracuda 2.4 | 6 | 98 | 746 |
| 1342 | Razer Audio Mixer | 6 | 102 | 945 |
| 1346 | Razer Seiren V2 Pro | 3 | 69 | 769 |
| 1347 | Razer Seiren V2 X | 3 | 69 | 769 |
| 1352 | Razer Leviathan V2 Pro | 5 | 127 | 836 |
| 1353 | RAZER KRAKEN V3 | 6 | 122 | 930 |
| 1354 | Razer Leviathan V2 X | 4 | 92 | 696 |
| 1364 | Razer Kitty V2 Pro | 6 | 126 | 958 |
| 1365 | RAZER BLACKSHARK V2 PRO | 6 | 110 | 828 |
| 1370 | RAZER NOMMO V2 PRO | 5 | 106 | 846 |
| 1372 | RAZER NOMMO V2 | 5 | 105 | 856 |
| 1374 | RAZER NOMMO V2 X | 4 | 81 | 638 |
| 1376 | Razer Kraken Kitty V2 | 4 | 78 | 623 |
| 1378 | Razer Kraken Kitty V2 BT Quartz Edition | 4 | 77 | 616 |
| 1382 | Razer WIRELESS CONTROL POD | 2 | 84 | 902 |
| 1383 | Razer Kraken V4 Pro | 9 | 337 | 2387 |
| 1386 | Razer Seiren V3 Mini | 3 | 64 | 780 |
| 1387 | Razer Kraken V4 | 7 | 122 | 1065 |
| 1389 | Razer Kraken V4 X | 4 | 81 | 646 |
| 1390 | RAZER BLACKSHARK V2 HYPERSPEED | 6 | 123 | 1039 |
| 1391 | Razer Seiren V3 Chroma | 4 | 97 | 1045 |
| 1392 | RAZER CLIO | 5 | 83 | 695 |
| 1396 | Razer Barracuda X Chroma | 6 | 91 | 726 |
| 1398 | Razer BlackShark V3 Pro | 7 | 192 | 1527 |
| 1401 | Razer BlackShark V3 | 7 | 188 | 1512 |
| 1404 | Razer BlackShark V3 X Hyperspeed | 4 | 74 | 578 |
| 1406 | Razer Kraken V2 BT Hello Kitty and Friends Edition | 4 | 77 | 616 |
| 1407 | Razer \| Sanrio Characters Limited Edition Wireless Headset | 4 | 77 | 616 |
| 1411 | Razer Barracuda Pro | 6 | 135 | 844 |
| 1412 | Razer Barracuda 2.4 | 6 | 131 | 818 |
| 1415 | Razer Kraken Kitty V3 Pro | 7 | 201 | 1472 |
| 1420 | Razer Hammerhead V3 | 3 | 52 | 409 |
| 1422 | Razer Seiren V3 Pro | 5 | 101 | 1130 |
| 1427 | Razer Madeline T1 | 7 | 211 | 1457 |
| 1439 | Razer Kraken V4 X Sensa HD Haptics | 5 | 135 | 1038 |
| 1442 | RAZER CLIO X | 5 | 112 | 750 |
| 1443 | RAZER LEVIATHAN V2 | 4 | 96 | 791 |
| 1446 | Razer Seiren V3 Pro | 6 | 116 | 1216 |
| 1453 | RAZER MAKO X | 6 | 113 | 916 |
| 1462 | Razer Kraken Kitty V3 | 5 | 92 | 779 |
| 1465 | Razer Hammerhead V3 Chroma | 6 | 88 | 713 |
| 2594 | Razer Turret Mouse Gears Of War 5 Edition | 6 | 136 | 1390 |
| 2595 | Turret Keyboard Gears Of War 5 Edition | 4 | 138 | 988 |
| 2596 | RAZER BLACKWIDOW V3 TENKEYLESS | 3 | 135 | 986 |
| 2629 | Razer Wolverine V3 Tournament Edition | 5 | 116 | 1134 |
| 2636 | Razer Wolverine V3 Pro | 7 | 141 | 1348 |
| 2638 | Razer BlackShark V3 Pro XBOX | 7 | 192 | 1527 |
| 2641 | Razer BlackShark V3 XBOX | 7 | 188 | 1512 |
| 2644 | Razer BlackShark V3 X Hyperspeed | 4 | 82 | 625 |
| 2647 | Razer Wolverine V3 Pro PC | 6 | 114 | 1090 |
| 2650 | Razer Wolverine V3 TE PC | 4 | 104 | 994 |
| 2657 | Razer Hammerhead V3 HyperSpeed | 5 | 93 | 724 |
| 2660 | Razer Hammerhead V3 X HyperSpeed | 5 | 85 | 597 |
| 2664 | Razer Hammerhead V3 X Hyperspeed for Xbox | 5 | 88 | 613 |
| 2668 | Razer Hammerhead V3 X HyperSpeed For PlayStation | 5 | 92 | 619 |
| 2676 | Razer Wolverine V4 Pro | 6 | 137 | 1281 |
| 2684 | Razer Silver T2 X | 5 | 135 | 1272 |
| 2689 | Razer Hammerhead V3 X HyperSpeed Kuromi Edition | 5 | 85 | 597 |
| 3072 | Razer Firefly Hard Edition | 2 | 64 | 531 |
| 3073 | Razer Goliathus Chroma | 2 | 64 | 532 |
| 3074 | Razer Goliathus Extended Chroma | 2 | 64 | 531 |
| 3076 | Razer Firefly V2 | 2 | 64 | 531 |
| 3077 | Razer Strider Chroma | 2 | 64 | 531 |
| 3078 | Razer Goliathus Chroma 3XL | 2 | 64 | 531 |
| 3080 | Razer Firefly V2 Pro | 2 | 64 | 531 |
| 3331 | Razer RipSaw HD | 2 | 29 | 291 |
| 3334 | Razer Stream Controller | 3 | 65 | 718 |
| 3337 | Razer Stream Controller X | 3 | 65 | 718 |
| 3587 | Razer Kiyo | 2 | 39 | 467 |
| 3589 | Razer Kiyo Pro | 2 | 39 | 467 |
| 3590 | Razer Kiyo X | 2 | 39 | 467 |
| 3592 | Razer Kiyo Pro Ultra | 4 | 53 | 705 |
| 3594 | Razer Kiyo V2 Pro | 4 | 53 | 801 |
| 3595 | Razer Kiyo V2 | 5 | 54 | 812 |
| 3596 | Razer Kiyo V2 X | 4 | 50 | 694 |
| 3848 | RAZER BASE STATION CHROMA | 2 | 64 | 531 |
| 3849 | Razer Chroma HDK | 2 | 55 | 503 |
| 3853 | Razer Laptop Stand Chroma | 2 | 64 | 531 |
| 3858 | RAZER RAPTOR 27 | 5 | 79 | 735 |
| 3859 | Lian Li 011 Dynamic | 2 | 64 | 531 |
| 3863 | Razer Tomahawk ATX | 2 | 64 | 531 |
| 3866 | Razer Core X Chroma | 2 | 64 | 535 |
| 3867 | Razer Seiren Emote | 3 | 48 | 348 |
| 3869 | Razer Bungee V3 Chroma | 2 | 55 | 501 |
| 3871 | Razer Chroma Addressable RGB Controller | 3 | 82 | 722 |
| 3872 | Razer Base Station V2 Chroma | 3 | 76 | 595 |
| 3873 | Razer Thunderbolt 4 Dock Chroma | 3 | 71 | 583 |
| 3878 | Razer Chroma Charging Pad 10W Fast Wireless Charger | 2 | 69 | 574 |
| 3879 | Tomahawk Gaming Desktop | 2 | 64 | 531 |
| 3880 | RAZER RAPTOR 27 (165Hz) | 5 | 86 | 770 |
| 3883 | Razer Laptop Stand V2 Chroma | 2 | 64 | 531 |
| 3884 | Razer Chroma Wireless ARGB Controller | 3 | 86 | 747 |
| 3886 | Razer Chroma Wireless ARGB Controller | 5 | 68 | 453 |
| 3893 | Razer Hanbo AIO 240MM ARGB | 3 | 77 | 662 |
| 3894 | Razer Head Cushion Chroma | 2 | 64 | 531 |
| 3900 | RAZER PWM PC FAN CONTROLLER | 2 | 46 | 426 |
| 3907 | Razer Laptop Cooling Pad | 3 | 133 | 1131 |
| 3909 | Razer Freyja | 2 | 67 | 588 |
| 3921 | Razer Core X V2 | 3 | 65 | 508 |
| 3922 | Razer Thunderbolt 5 Dock Chroma | 3 | 73 | 587 |
| 3929 | Razer Monitor Stand Chroma | 2 | 102 | 903 |
| 3932 | Handheld Gaming Dock | 2 | 102 | 904 |
| 3940 | Razer Soma Chroma | 3 | 65 | 546 |
| 3942 | Razer Marci | 7 | 148 | 1219 |
| 3946 | August T2 | 3 | 187 | 1748 |
| 3949 | MarciT2H | 3 | 70 | 597 |
| 4115 | Razer Kitsune | 3 | 67 | 593 |
| 4124 | Razer BlackShark V3 Pro PS | 7 | 192 | 1527 |
| 4126 | Razer BlackShark V3 PS | 7 | 188 | 1512 |
| 4130 | Razer BlackShark V3 X Hyperspeed | 4 | 82 | 625 |
| 4133 | Razer Raiju V3 Pro | 6 | 115 | 1121 |
| 4144 | Razer Raiju V3 Pro Signature Edition | 6 | 115 | 1123 |
| 45066 | Razer Basilisk V3 Pro 35K - XBOX 25 Anniversary Edition | 6 | 90 | 824 |

## 维护及静态校验

运行 `node tools/extract-product-ui-details-current.cjs` 重建；`--check` 对全部源字节 SHA、原文切片、所有嵌套范围、页面引用/edge 和 lexical parent 做验证，并重建归档比对压缩内容、轻量 JSON 与此 MD。仅执行自有静态解析工具，不执行被审阅的包。

检查点保存在 `.work/product-ui-details-cache/`，只有工具及调用链 SHA 都一致且当前源重新核对通过才复用。缓存未代表额外语义验证。此文档与数据不可作为“整个产品包所有逻辑已还原”的证明；下一步须逐个页面沿参数、共享组件、事件/action、host/service/DLL 与样式文案绑定继续闭环。
