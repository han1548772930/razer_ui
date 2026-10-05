# Gamer Room：2026-10-05 现行源码修复

本轮只使用 `.ref/applications/synapse/dashboard/` 当前 manifest 中的源文件；没有读取已废弃的四个参考目录，没有执行应用、构建、测试、下载的 JavaScript 或 DLL。

原始页级审计见 [ui-source-review-2026-10-05.md](ui-source-review-2026-10-05.md) 的 G01–G07。本记录描述实际修改及边界，不把静态编译或存在一个 renderer 等同于像素一致、硬件连接成功。

## 当前证据

- `9388.974b5d43.chunk.js / 19388`：`He → je → ze → fe → S`；营销 `o → b → u`；两步提示 `Pe`。
- `55.4e8559cb.chunk.css`：`#gamerRoom`、公共 `.dashboard .box-group`、`.help`、`.switch` 和 `expand` 关键帧。
- `50527/v` 是当前 popup 使用的 32×18 switch；`26079` 是 `fe` 实际导入的 lodash debounce。
- 维护工具 `node tools/audit-gamer-room-current.cjs --check` 静态核对 30 个 AST 合约、133 条 CSS、10 个资源哈希。收据为 [gamer-room-current-evidence.json](gamer-room-current-evidence.json)，包括完整原文、位置、SHA-256、文案 key、setup 状态和图片回退 URL。

## 已实施

实现从共享 `service_pages.rs` 拆入 `src/shell/gamer_room.rs`、`gamer_room_devices.rs`、`gamer_room_presentation.rs`，热点动画独立在 `gamer_room_hotspot.rs`；原调用者仍通过 `service_pages::GamerRoomPage` 使用。主路径由 shell 将 workspace 的实有 `Device.sub_devices` 同步给页面。

| 审计项 | 本轮结果 |
| --- | --- |
| G01 | 增加 `subDevices` 数据入口、两个真实设备组、186×176 设备卡、100×100 图片、名称/骨架/安装状态、power/offline 图标、online/offline popup、Synapse Override 和 Settings 事件；banner 由两组是否为空决定，不再无条件常驻。没有 `subDevices` 的普通产品目录不会成为 IoT 设备 |
| G02 | 收起时保留正文树。max-height 0↔2000 的 300ms ease-in、正文 −100%↔0 的 300ms linear、箭头 −90°↔0 的 300ms linear 分开处理；展开的 overflow 跟随当前 1s ease 关键帧，不以直接卸载代替动画。正文采用实际内容高度，不套用 Dashboard 的 220px 卡高 |
| G03 | help 是标题收展按钮旁的独立 14×14 控件，margin-left5；tip left0/top20/min-width300/padding8 10、14/16 字体、黑底/1px #5d5d5d 边框、300ms linear opacity；背景 hover 300ms ease。第二组实际文案修为 `SMART_HOME_TOOLTIP`，由当前 `c.O29` 解析，而非旧本地 key |
| G05 | 产品名明确 #fff、18px RazerF5/bold；Learn More 增加当前 `o.render` 内联的 21×20 外链 path，文字及图标 #ccc→#44d62c |
| G06 | 删除 `active_product/open_product` 点击锁定分支。热点 hover 打开，离开产品面板关闭；只有产品面板自身点击打开官方产品地址 |
| G07 | 取消固定 400/652px panel width、所有说明强制284px及 Base Popup 的视窗夹取。panel 保留 min-width400/min-height312/padding36 24；只有双灯说明 min-width284，双卡 gap36。按面板自身实测盒子施加 −23.5%/−86.5%、−50% 变换，并保留原 banner/滚动裁剪 |

设备状态来自 `Device` 与实收的子项字段，遵循源对象展开的字段覆盖方向。父产品 780、3880、3858 被过滤；子产品780也不渲染。`isSynapseOverride:false` 入第二组；true 或字段缺席入第一组；显式 null 不冒充缺席。来源没有对应语言的 name 时保留 skeleton，title 不替代源的 name 存在条件。

本地有产品页面的条目可使用已实现页面，因此不再依赖下载/安装器门控；这只是用户要求的入口策略，不生成 `installedDevices`、安装日期或设备 READY 回执。Settings 产生精确产品/序列号/容器标识的打开事件，由 shell 连接产品 workspace。

Power 沿当前 `fe` 的鼠标按下提交路径，在新建的 500ms debounce 尾部发出 `ON_SET_POWER_STATE_IOT` 请求。Override 发出 `ON_SET_SYNAPSE_OVERRIDE`。两者均不修改 `isOnline/isPowerOn/isSynapseOverride` 观测，定时器不会伪造硬件成功。当前没有可信 IoT transport，实际应答接入仍待完成。

新增弹层保留源裁剪并按图层绘制；两步提示使用当前 `.gamer-room-tutorial-modal__wrapper` 的1050层，位于设备弹层21和help100之上。help背景色按预乘sRGB/alpha插值，保留 #4a4a4a→#ffffff4d 的300ms ease，避免用直线Hsla颜色插值改变半透明中间帧。

新增资源 `gr-external-link.svg` 直接提取当前营销 JSX path；`gr-help.svg` 是当前 CSS 指定 `tooltip_questionmark.96138d2f.svg` 的字节复制。维护中的 `tools/gamer_room_assets.py` 可重复准备资源；主资源清单及嵌入表已更新。原有八个设备/popup SVG 重新从当前19388提取并核对清单哈希，没有用相似图标代替。

## 热点原 SVG 的离线恢复与完整动画

此前缺失的 `gamer_room_hotspot_animation.53dd5566.svg` 已通过当前 manifest 的内容指纹离线恢复。维护工具以166个已缓存的当前 Dashboard 原SVG校准，文件名指纹均等于原始字节 MD4 的前8位；现存候选 `assets/synapse/gr-hotspot-source.svg` 唯一命中 `53dd5566`，完整MD4是 `53dd5566d963d7c28e53733c006c064e`，SHA-256为 `16ea857d1571db9eb72fbe03d580373c1c780618fdf74574aa89bd8a7e6b9e88`。恢复过程及当前 manifest/JS 引用见 [current-media-offline-recovery.json](current-media-offline-recovery.json)。这是当前内容指纹匹配，**不是独立 live 下载逐字节比对**；原先网络错误和自动审批基础设施503记录没有改写成HTTP成功。

实现只从恢复后的当前 `.ref/applications/synapse/dashboard/static/media/` 文件提取。`python tools/extract-gamer-room-hotspot.py --check` 校验XML、恢复记录与manifest哈希，并核对生产数据 `src/shell/gamer_room_hotspot_data.json` 和 [gamer-room-hotspot-current-evidence.json](gamer-room-hotspot-current-evidence.json)。

- 当前 `19388/j` 仅挂 `img.pulse`；当前manifest全部CSS没有 `.pulse` 的额外尺寸规则，图片规则仅指定z-index1。恢复36×36固有尺寸和36×36 viewBox，去掉本地强制40×40。SVG内部 `width:100%;height:100%` 填充自身图像视口。
- 两级 `translate(16.69,17.643)` 与 `translate(1.31,0.357)` 得到中心(18,18)。第一条ellipse的fill-opacity恒0，末尾 `time_group` 为空；这两者的动画不能被画成额外圆盘或整体淡入。只有白色、无填充、居中描边的ellipse可见。
- 周期严格使用源 `1.3333333s`，begin0、repeatCount indefinite。四条可见ellipse轨道中的rx/ry完全一致，静态核对后作为同一个radius轨道采样：

| keyTime | radius | stroke-width | opacity |
| --- | --- | --- | --- |
| 0 | 2 | 2 | 1 |
| 0.25 | 10 | 4 | 0.2 |
| 0.625 | 17.5 | 1 | 0 |
| 1 | 17.5 | 1 | 0 |

三段各用原 `keySplines`：`0.333 0 0.833 1`、`0.167 0 0.833 1`、`0 0 0 0`，没有替换成统一ease。最后37.5%周期保持不可见，下一周期回到源首帧。矢量绘制用两个SVG半圆弧和带小数的居中stroke，避免CSS border把描边宽度取整。三个被动热点保留source z-index1、原负10px重叠，并位于营销面板z-index2之下。框架降低动态效果选项启用时，本地选择暂停源首帧；此辅助选项不是原SVG自带的media分支。

旧 `gr-hotspot.svg` 静态资源仍可由准备工具重现，但主界面已不再使用它；其清单来源改为当前恢复文件并明确引用离线恢复收据。

## 明确未完成

- **G04 背景模糊**：锁定的 `gpui-pre 0.3.7` Style 暴露 box-shadow blur，没有 CSS backdrop-filter 的场景采样接口。热点 blur5、面板 blur30 仍未实现；半透明底和投影不能被写作模糊已相符。没有把旧截图烘焙为背景代替动态源行为。
- native `installedDevices/noAliveSignPage`、安装中继承条目、更多源设备属性尚缺服务输入；只有明确字段才接入。`srcSet` 多分辨率选择未实现；普通 `src` 和当前 `sKg + devices/{productId}/cover.png` 回退已保留。
- 数字 title 排序已保留；非 ASCII 的 JS `localeCompare` 本地化排序、CSS capitalize 的完整 Unicode 行为尚未复刻。
- 收展状态在页面实体内保留，尚未连接源 `groupsCollapsed` 持久存储；中途逆转及跨弹层层级尚需继续静态逐项核对。
- GPUI 与浏览器的 shrink-to-fit 细节没有通过运行/截图验证；本轮恢复原约束和百分比计算，不能仅凭这些声明所有窄屏布局已完全一致。

验证限于上述静态解析、资源准备及哈希校验。热点修改后的格式化和全项目 `cargo check --locked --all-targets` 由主任务统一执行；本文件不预先宣称其结果。
