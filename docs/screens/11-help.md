# 设备帮助页：182 / 653 / 777

更新日期：2026-10-01。依据本地原始 JS、CSS、语言常量和资源，不使用截图。

## 1. 入口与源码

HELP 是设备工作区内部页面。它存在于产品根 `navs` 中，由 header 从普通选项卡中移到右侧帮助按钮；点击后切换内容区，不能直接替换为打开统一支持网站。

| 产品 | 原入口与实际 render | 原文件 |
|---|---|---|
| 182 | `KN → WN.render` | [main.db20a7c4.js](../../.ref/devices/182/static/js/main.db20a7c4.js) |
| 653 | `Jd → Qd.render` | [main.7b71cce5.js](../../.ref/devices/653/static/js/main.7b71cce5.js) |
| 777 | `Tv → rv.render` | [main.eb70ce38.js](../../.ref/devices/777/static/js/main.eb70ce38.js) |

三个根入口均只传入 `resetObm`，未传 `hasTutorial`、`hasSystemInfo`、Camo 等开关。共享 Help 内存在的教程、系统图片、BIOS/EC/VBIOS、THX、Camo 区块不属于这三个设备的默认画面。Profile 在 HELP 隐藏；182/777 的隐藏列表还包括 Power/Calibration，653 包括 Power。

## 2. 默认区块和文案

| 列 | 区块 | 原语言 key 和行为 |
|---|---|---|
| 左 | 支持 | `SUPPORT`；三个外链分别为 `VISIT_DEVICE_SUPPORT`、`VISIT_MASTER_PAGE`、`VISIT_SYNAPSE_SUPPORT` |
| 左 | 恢复出厂 | `FACTORY_RESET`、`FACTORY_RESET_PROFILES`、`RESET`；原版确认弹层 300px，最终调用设备重置服务 |
| 右 | 序列号 | 标题 `SERIAL_NUM`、内容 `SERIAL` 加实际序列号；`COPY_SERIAL` / `COPIED_SERIAL`，复制状态保持 2000ms |
| 右 | 设备版本 | `currentFWVersion` 非空才显示；标题 `DEVICE_HEADER`，首行 `FIRMWARE_VERSION`；`VIEW_MORE` / `VIEW_LESS` 控制额外版本 |
| 右 | 产品注册 | 标题 `PRODUCT_REGISTRATION`，链接 `REGISTER_ONLINE` |

额外版本分别是运行时 `uiVersion`、`mwVersion`、`synapseVersion`，对应 `UI_VERSION`、`MW_VERSION`、`SYNAPSE_VERSION`，各自非空时才显示。不能用 Rust 应用包版本填充原设备 UI 版本。原源码还有获取序列号时的 loading、失败重试和联系支持分支，需真实服务响应后连接。

## 3. 产品链接

| 产品 | 支持页 | Master Guide 前缀 |
|---|---|---|
| 182 | `https://mysupport.razer.com/app/answers/detail/a_id/6125` | `https://dl.razerzone.com/master-guides/RazerSynapse3/DEATHADDERV3PRO-00000182-` |
| 653 | `https://mysupport.razer.com/app/answers/detail/a_id/9703/` | `https://dl.razerzone.com/master-guides/RazerSynapse3/BLACKWIDOWV4PRO-00000653-` |
| 777 | `https://mysupport.razer.com/app/answers/detail/a_id/3851` | `https://dl.razerzone.com/master-guides/RazerSynapse3/KRAKENBTSANRIOLIMITEDEDITION-00000777-` |

指南拼接 `lang || "en"` 和 `.pdf`。Rust locale 转为小写 URL 语言，例如 `zh-CN → zh-cn`。公共 Synapse 支持页为 `https://support.razer.com`，注册页为 `https://www.razer.com/product-registration`。

## 4. 布局、状态和资源

两列采用共享 `.body-widgets/.widget-col`：600px 卡片、20px 列距；1279px 以下列有 30px 侧边距并换列，窄窗正文可以滚动。不是随着窗口宽度将卡片内部等比例压缩。卡片左右内距 40px、上下 30px、圆角 5px；标题 RazerF5 16px、绿色、uppercase，标题与内容间距 20px。

`.help-component .support-link` 的上外距为 10px；外链有下划线和原 20×20 外链 SVG，图标左距 5px。`.HelpComponent_btn__dUmx6` 为 27px 高、最小宽 100px、左右 12px 内距、1px 黑边、3px 圆角、`#707070` 底和白色 12px 字；hover opacity 0.8。`.show-all-button` 字号 14px，上下内距 15px，文字靠左，hover 绿色，20×20 箭头左距 4px。

资源同步由 [prepare-resources.py](../../tools/prepare-resources.py) 负责：

- [icon_external_link.48227e72.svg](../../.ref/devices/182/static/media/icon_external_link.48227e72.svg) → [external-link.svg](../../assets/synapse/external-link.svg)。
- 182 main 的内联组件 `Ed` / `qR` 原始 path → [help-less.svg](../../assets/synapse/help-less.svg) / [help-more.svg](../../assets/synapse/help-more.svg)。不是普通下拉箭头的替代图。
- header 使用原帮助 sprite 的 default/hover/active 三个视口，HELP 选中时使用 active。

## 5. 当前实现与测试边界

[help_page.rs](../../src/features/help_page.rs) 已实现三产品帮助路由、产品链接、原区块与文案、序列号复制和 2 秒状态复位、版本展开状态及响应式列。外链使用 GPUI Kit Base Link；按钮和状态由框架控件及保留在工作区的状态管理。切页继续经过映射草稿的保存/丢弃保护。

当前设备快照只有序列号和固件版本，没有 UI/MW/Synapse 运行时版本。设置中的[服务连接](../../src/shell/runtime_page.rs) 已提供独立版本读取与详情展示，但尚未将响应对应到 Help 的产品版本字段，也不以 Rust 包版本代填。恢复出厂按钮禁用并说明设备服务未连接，不能通过清空本地 Profile 冒充硬件重置；原恢复出厂确认、loading 和错误重试要在设备服务接入时完成。

[help_tests.rs](../../src/features/help_tests.rs) 包含产品/语言链接单元测试、三个产品真实帮助入口与 Profile 可见性的 UI 测试、1280→1279→700→1280 的布局和设置保持检查（含 125% 字体缩放），以及真实复制值和 1999ms/2000ms 状态边界。遵照用户要求只编译测试源码，不执行测试或 build；编译通过不等同于运行时视觉验收。
