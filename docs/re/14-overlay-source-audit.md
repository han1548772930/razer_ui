# 弹出层源码复核

更新日期：2026-10-02。用户指出弹出层样式有偏差后，单独对照原版JS挂载与CSS最终覆盖。不能用同一个默认Dialog样式代替所有弹层；尺寸均为原CSS基准，通过项目rem比例换算。

| 弹层 | 原版依据 | 本轮处理 |
| --- | --- | --- |
| 映射未保存确认 | 182 `QP.renderSaveAlert`、653对应方法；`.save-alert`、`.backdrop`、`.keymap-action` | [SourceAlert](../../src/ui/source_alert.rs) 基于Base Dialog，400宽、20×30内边距、绿色1px边框、5px圆角、居中16px标题、14px正文及27px双按钮；右上关闭继续编辑。遮罩独立黑50%，移除Kit默认阴影/滑入 |
| 保存确认位置 | 182及frontend 55为top50%/translateY(-100%)；653最后覆盖为top30%/translateY(-50%) | 根据产品保留不同锚点，未统一成窗口中心或Kit默认10%顶距；短窗口限制可用高度 |
| 映射右编辑器 | 182 5107及原`.key-config`系列 | 保留292宽；复核36px标题栏、分隔、原阴影、选中分类资源和27px按钮，见[映射审计](09-mapping-editor.md) |
| Profile菜单/确认 | `.profile-del`、原更多菜单 | 已有Base Popover，不叠加Kit面板内距；保留155px菜单及300px确认框 |
| Command Dial删除/重置 | 653 `Fs/vs` 实际渲染及 `.command-dial .icon-delete .profile-del` 最终覆盖 | [keyboard_controls.rs](../../src/features/keyboard_controls.rs) 使用图标旁Base Popover，300px宽、20px内距；删除距图标顶部26px、右缘向左11px，重置顶部42px、右齐。删除橙框但实际 `del-title-normal` 仍是红标题，配灰色单按钮；重置红框/红标题/红单按钮；外点及Escape关闭 |
| 全局快捷键删除 | 7282 `.shortcut_item .profile-del` 与 `DELETE_SHORTCUT` / `DELETE_SHORTCUT_MESSAGE` / `REMOVE` | [shortcuts.rs](../../src/features/shortcuts.rs) 从居中Dialog改为行内锚定Base Popover，left274/top53、300px宽、红框和27px单按钮；保留原0/6/10px黑20%阴影，删除后焦点回到列表 |
| 配置导入/导出 | `ImportExportModal`系列 | [ProfileDialog](../../src/features/profile.rs) 基于Base Dialog，602×481、top104、36px居中标题、黑70%遮罩、独立正文与底栏。内容仍是明确标识的本项目文件格式 |
| Gamer Room添加与教程 | `IotPopupRoot`的`ct/B/ze`和55 CSS；9388教程 | 独立Base Dialog及Base Popover，复核标题栏、页面步骤、原锚点与遮罩，见[主前端页面](../screens/10-main-frontend-pages.md) |
| 配对卡内确认与713提示 | frontend4130、182 `BL/DualLinkWarning` | 前者保留230px卡内自适应确认；后者为独立400px全窗口警告，背景/段落/按钮顺序分别核对，见[配对](../screens/03-pairing.md) |
| Calibration首次说明 | 182 `DD` 及 `.welcome` 系列 | 原版是无遮罩的页内说明，不改成全窗口Dialog；保留尺寸、关闭图标和大写标题 |
| 颜色与二级拾色器 | 653 `JM → BM`、777对应组件；`.color-options` / `.picker-container` 最终CSS | [lighting_color.rs](../../src/features/lighting_color.rs)补40预设、16自定义槽、选中标记、上下展开、右键编辑/删除、色面/亮度与HEX/RGB；二级保存/取消返回色板，取消不改颜色。Dial按最终CSS隐藏无颜色 |
| 653板载配置 | `fR → LR/UR/gR`，槽ID为2–5 | [onboard_memory.rs](../../src/features/onboard_memory.rs)补270px面板、宏内存和独立冲突Dialog；冲突不是Popover子生命周期，关闭菜单不意外销毁警告。示例仅在标记的预览设备内可用 |
| 配置迁移状态 | 独立应用 `hg → Rt/Ot/wt/Lt` | [profile_migration.rs](../../src/shell/profile_migration.rs)保留300/400px进度/结果框、黑70%遮罩、准备可取消与迁移禁止取消；明确标示示例状态，真实扫描/迁移未接通 |

框架继续负责焦点陷阱、键盘和事件分发，应用提供原版外观及各层关闭行为。自定义层的关闭先恢复有效触发焦点，再执行后续动作；文件选择返回继续核对工作区和Profile身份。

后续复查纠正777删除框：`bP`实际将它放在更多按钮包装内，`.flex.profile-bar>div.profile-del`直接子选择器不匹配，使用top52。边框和按钮为#c8323c、按钮白字，标题仍#fd4949。导入/导出的原backdrop没有点击handler，已禁止外点关闭；标题栏关闭图标20px、右距5。通用确认采用原#999关闭图标。Component Button内部字号不继承外层text_size，12px动作已指定对应size。

原始导入/导出、关联游戏等依赖宿主的业务尚未恢复为原版服务。静态样式对齐不等于实际窗口验收；本轮保留相关几何与交互回归源码，仅执行`cargo check --locked --all-targets`，不运行应用或测试。

Snap Tap 的布局按键选择、设置服务说明、应用退出保存和设备快照详情属于本地补充界面，没有对应原版挂载证据；不把它们记作原版弹层复刻。原Snap Tap通过真实键盘输入录制，布局选择仅补充无法区分的左右修饰键及数字键盘输入。
