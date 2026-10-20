# 鼠标：滚轮模式、Smart Reel 与滚动加速

滚轮卡片修改当前配置的 `scrollWheel`。模式控制机械／触觉滚动方式；Smart Reel 控制自动切换；加速控制虚拟滚动加速；level 字段是相应功能的档位。滚轮方向的按键映射属于 [Customize](customize.md)，并非这组设置。

## 产品差异与输入

| 当前完整审计 PID | 模式版本 | 其他实际分支 |
| --- | --- | --- |
| 153、170 | 普通模式 | 模式、Smart Reel、加速开关，没有本文所述 level 编辑 |
| 203、204、214、45066 | 普通模式 | 具有 Smart Reel／加速 level；45066 的模式板载同步只选第一个真实匹配槽位 |
| 226 | Mode V2 | Tactile、FreeSpin、MicroTactile；禁用模式集合；Smart Reel／加速 level |

PID226 的 level 滑块是 0～4、步长 1，属于档位而非百分比。模式的协议值分别是 Tactile=0、FreeSpin=1、MicroTactile=2。界面最多允许禁用两个模式；禁用当前模式时，立即选择顺序中第一项仍可用模式。FreeSpin 被禁用时，Smart Reel 和加速卡片同时锁定，并显示其原因提示。

模式按钮先更新局部选中态，300 ms debounce 后提交，外部 scrollMode 变化另同步局部态。禁用模式的操作是另一个 action，不经过该模式按钮的延时分支。

## 保存、设备工作与结果

UI reducer 对各字段发送 `ON_SCROLL_MODE`／`ON_SCROLL_MODE_V2`、`ON_SMART_REEL`、`ON_SCROLL_ACCELERATION` 及对应 level／disabled-modes 事件。中间件先保存当前配置字段和版本，再派发设备任务。配置恢复依次处理 Mode、Smart Reel、可选 threshold、Acceleration、可选 level；PID226 最后恢复 Mode V2 的模式屏蔽与选中模式。

PID226 `85191` 的任务分别调用：

| 设置 | 实际 setter |
| --- | --- |
| 加速开关 | `setScrollVirtualAccelerationState(profile,0/1)` |
| 加速档位 | `setScrollVirtualAccelerationLevel(profile,level)` |
| Smart Reel | `setScrollModeAutoSwitchState(profile,0/1)` |
| Smart Reel 档位 | `setScrollWheelSmartReelModeThreshold(profile,level)` |
| 滚轮模式 | `setScrollFreeNotchedMode(profile,0/1/2)` |

这些 writer 在入口检查取消，捕获传输错误并 reject；完成依据是 setter 应答，完整函数中没有普遍追加 GET 比较。Mode V2 成功应答中的 mode 可以映射回界面值；普通分支的 UI 发布条件不同，不能用同一套消费者解释全部产品。

真实板载清单由 retained device session 读取最大容量、数量、顺序 ID 和完整身份，再按当前配置 GUID 选槽。Smart Reel 保留全部匹配槽位；PID45066 模式路径只取第一个匹配。当前配置的 GUID、本地数组序号和真实槽位 ID 是不同对象。

## Mode V2 的恢复与失败

PID226 `restoreScrollDisabledModes` 先从所选配置规范化 disabledModes，再从当前 scrollMode 与可用模式中得到 active mode，提交禁用集合，最后发布两个字段的 UI 投影。只有这条实际 restore 链能说明恢复顺序，不能把 reducer 初值当成设备读回。

原版错误通道记录失败，编辑器保持乐观字段；未见通用“失败回滚上一次配置”的动作。取消入口返回、未发出的 debounce、已保存配置、已发送设备命令和任务结算是不同阶段。源码存在若干入口仅记录 canceled 并返回的 writer，这不证明设备已还原，也不证明先前本地保存被撤销。

## 原件与复核位置

- PID226 UI：`.analysis/inputs/frontend/mouse-226-current/raw/apps.razer.com/synapse/products/226/ui/static/js/6424.1f24d872.chunk.js`，卡片、0～4 滑块、模式顺序、禁用规则与 300 ms debounce 位于 `[214200,218600)`；直接复读原始字节。
- PID226 中间件：`.analysis/inputs/frontend/mouse-mw-226-current/raw/apps.razer.com/synapse/products/226/mw/7846.8d3e3465fc6ae4ee8a38.js`，`restoreScrollDisabledModes` `[1066824,1067253)`；加速 writer `Wn` `[1713707,1714580)`、level `$n` `[1714580,1715368)`、Smart Reel `Yn` `[1715368,1716190)`、threshold `Jn` `[1716190,1716937)`、模式 `qn` `[1716937,1718065)`；直接复读完整函数。
- 全产品分支索引：`.analysis/results/frontend/mouse-mw-audit.json` 的 `queries["mouse-scroll"].products[product_id].semantics`／`lifecycle_association_chains`／`complete_owners`，完整模块由该 query 的 `complete_owners[key]` 定位回当前原件。

本文完成上述静态语义记录，未执行真实滚轮切换、板载写入或窗口验收。
