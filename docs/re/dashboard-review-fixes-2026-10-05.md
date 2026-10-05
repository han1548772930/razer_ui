# Dashboard 当前源码修正（2026-10-05）

本批接续逐页审计 D01–D10。证据来自当前 `.ref/applications/synapse/dashboard/` 的 `22534/Pi/xi/z/G/V/K/ji` 与实际挂载节点适用的 CSS，没有使用已删除的旧目录。验证只采用静态解析、资源校验、格式化及 `cargo check --locked --all-targets`；没有运行应用或进行截图比对。

## 已落地

- D01：首页加入当前 `ji` 的双应用介绍横幅，并与 Chroma `62296/Pn` 共用经 CSS/文案静态比对的组件。保留 1220/2500 最小/最大宽、531 最小高、42/24 标题、100 图标、140 箭头以及原版 CTA 样式和 200ms ease-out 不透明度过渡。关闭状态写入本地 Dashboard 偏好；两个已实现导览直接打开。源关闭时附带的 profile-migration toast 尚未接入。
- D02：恢复当前源表八个 Dashboard 模块入口。Macro、Linked Games、Alexa、Feedback、Wi-Fi 加入、Profile Migration、Armory、Tour 均连接已有页面/窗口。Armory 真实服务 feature flags、维护状态仍需接入；没有把独立 Chroma Studio 冒称已实现。
- D03/D08：设备名按 `G/V` 的 zh-cn/en 选择及 `name[locale] → baseProductName → name.en → 空` 回退；加入 `K` 的 edition 行（不按 edition_id 猜名称、不做语言回退）。配置行精确解析本地 active ID 所指的 GUID，不使用回退首配置的 helper。支持 source `subDevices.isShownOnUI`、`notShowProfileName`、单配置例外和 `autoSwitch` 后缀。支持 source waiting/downloading/installing/syncing/updating/error 文案分支；断电时不挂配置行。
- D04 基础分支：新增独立 Dashboard 电池组件，避免套用设备工具栏的不同分档。按当前 `29228/pV` 字面值处理普通充电、断电、错误、非充电、暂停充电；10–19% 均向下分档，不沿用工具栏的特殊 20% 档；非充电 -1 同时隐藏数值和图标。支持 showBatteryValue/hideBatteryIcon/isBatterySupported/isExternalBatt 条件及挂载、off→on 后 2 秒数值延迟。字体14、right/top10、图标盒26/背景20、低电量红色、普通 off 红色 mask 均有当前 CSS 依据。
- 补回 source `count`/多子设备数量与显示序号角标；普通断电、待机等已知源状态降低图像与名字组 opacity .3，并禁用对应打开动作。
- D05/D06：空设备卡恢复透明底与 `NO_DEVICE_FOUND` 原文。
- D07：Dashboard 自身恢复 50px 底部 margin。
- D09：收展箭头显式正常 #999 / hover #fff，避免 SVG mask 继承 #ccc。
- D10：容器宽与卡片列数分离，恢复 >600 CSS px 下 620 最小宽及 910/1220/2460 最大宽条件。列数使用当前 `Pi.getMaxColumns` 算法；拖动 x 上限使用实际容器宽减 290，而非最后一整列。

## 模型与证据

`Device.dashboard` 是本地存储中明确分组的可选源展示字段；没有服务观测时保持 None。它不是声称与源 wire object 逐字段兼容的传输适配器。保留 `Device.sub_devices` 的真实原始对象，供复合设备及 Gamer Room 分别静态解码；没有将普通设备目录制造为已连接 IoT 设备。

`tools/audit-dashboard-device.cjs` 生成 `dashboard-device-current-evidence.json`：7 个 AST 合约、2 个枚举、68 条 CSS、26 项原始/输出资源哈希。Dashboard 没有缓存的部分通用电池文件来自当前产品 182；逐项要求它们的带 hash 文件名出现在 Dashboard 实际 CSS 中，且文件哈希仍与资源收据一致。暂停充电使用现有源 SVG viewBox 切片。

共享横幅的独立收据是 `app-introduction-banner-current-evidence.json`，由 `tools/audit-app-introduction-banner.cjs` 生成；仅正规化两个应用的资源根路径后，横幅 CSS 和语言 key 相同。

## 仍未完成，不能算作像素复刻通过

- `icon_device_power_state_off.6b8c9694.svg`、`xbox-icon.11f09412.svg`、`ps-icon.54354df0.svg` 当前文件缺失。对应独立图标保持原尺寸空槽，未用普通断电图标代替另一资源。Xbox/PlayStation 的专属图像、快捷键文案和 tooltip 仍未接入。
- Battery tooltip 的源警告特殊宽度、控制器边界翻转，以及 CSS mask 默认定位/重复规则还需进一步核对。仅基础 hover/300ms opacity 和普通定位已挂回。
- 非 ready 设备的完整 spinner/retry/install/error 树、restart toast、firmware icon/详情、WDL、presetLoading、初始化失败、推荐和合作优惠的数据条件尚未完整实现。
- `Device.dashboard` 的真实服务 adapter、字段更新流没有凭空补成已连接；本批只提供可接受真实源状态的展示模型。源 powerStatus 缺 level 的情况在当前本地 PowerStatus 类型中尚不能表达。
- 首页 <=600 断点与宿主最小宽交互、复杂拖动中断、组层级和 tooltip 层级还需静态深入；不以已有基础几何推导全部边界已经一致。
