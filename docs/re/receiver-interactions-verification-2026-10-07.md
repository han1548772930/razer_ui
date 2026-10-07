# 接收器与底座配对页：当前源码交互复核

2026-10-07。范围为已实现的 179 HyperPolling 接收器、164 Mouse Dock Pro、241 Mouse Dock V2 Pro。只读当前产品 manifest 声明的 JS/CSS，用 Acorn 取实际挂载组件和回调；没有执行参考 JavaScript、应用、构建、测试或 DLL。

## 本轮修复

| 界面 | 当前源码事实 | 本地修正 |
| --- | --- | --- |
| 164 已配对名称 | `2715/Re` 的正式 `pairedTitle` 调用 `g` 打开配对工具 | 不再误用 241 的设备导航事件；已知/未知设备名称都保留配对入口 |
| 241 设备名导航 | `3746/Es` 接受缺省或 0 的 `editionId`；产品 ID 经 `Number` 归一 | 本地 u32 产品 ID 按同值匹配；edition 为 0 时解析已登记设备的真实 edition 再发送原有导航事件，非零仍精确匹配，0 产品 ID 不生成链接 |
| 241 双设备顺序 | `Ps` 的 `Z(x, mouse), Z(H, keyboard)` 和 `q(x), q(H)` 均为鼠标在前 | 页面按鼠标、键盘排序，不改变弹层原有通道存储顺序 |
| 241 已配对行 | 图标与说明组间距 10px；组内说明与取消配对按钮间距 20px，居中对齐 | 恢复两层布局，保留说明区 `flex:1;min-width:0` |
| 241 名称列表 | `.deviceList` 为无 gap 的纵向 flex，行高 17px | 去掉额外的 4px 行间距，恢复 17px 行高 |
| 164 已配对行 | `.pairedDes` 宽 369px，外层 `justify-content:space-between` | 恢复说明宽度与分布；不再沿用 241 的可伸缩说明布局 |
| 164/241 已配对行 | 两个产品的 `pairInfoBox` 都有下 margin 20px | 在完整行下保留 20px，警告不再紧贴主体 |
| 241 页面取消配对按钮 | `height:auto;align-self:flex-start;padding:7px 16px 6px;line-height:14px` | 去掉固定 28px 高度与对称 6px padding；保留源 9px 顶 margin |

241 名称导航与 164 名称配对入口使用 Base Button 承担键盘、可访问名称和焦点行为。导航继续复用现有 `DeviceLinkRequested → WorkspaceEvent::OpenDevice`；没有增加设备写入或成功响应。布局尺寸使用现有 `surface::css` rem 映射，颜色仍取现有源主题角色。

## 179 回读结果

重新核对 `9473/mE → Ie/Te + AE/OE` 的 Windows 根、三种指示灯模式、SVG 分层/SMIL、Widget portal、`G/se` 的配对加载态及关闭路径。本轮未发现需要改动 `receiver.rs` 的新增确定偏差。

`receiver-current-evidence.json --check` 起初因 `source_controls.rs`、`source_tooltip.rs`、`surface.rs` 的旧 native SHA 失败。回读当前 receiver 挂载/恢复、本地 edit 路径及专用提示接口后，运行原静态生成器刷新三项 native SHA；参考源指纹、组件、108 条 CSS、10 个原图层及 `receiver.rs` 本身均未变化。刷新 hash 不代表其他产品使用这些共享文件的所有分支均已重新验收。

## 原码指纹与收据

新收据：[receiver-interactions-current-evidence.json](receiver-interactions-current-evidence.json)，生成器 [audit-receiver-interactions-current.cjs](../../tools/audit-receiver-interactions-current.cjs)，共 15 项 AST 与 56 条专项 CSS；偏移为 Acorn UTF-16 单位。

| 当前产品文件 | SHA-256 | 主要范围 |
| --- | --- | --- |
| 164 `main.458d4103.js` | `0324ddd7980caf41f07eeea62979c15579578cba2a13ac302a8347e6d338fb40` | `Re` 66663–71018；`Pe` 72173–72932 |
| 241 `914.4c13bdac.chunk.js` | `1320d5e2c7ee42497e590f2dbd979b9d10614573746003b3b87b6695dfc6a9d2` | `Es` 45838–46091；`Ps` 48748–54333；`bs` 54400–54531 |
| 179 `main.4849f7ca.js` | `f5d6efe15f34167d61020469f151c3a0827f695cb35bc49cbdd32c23957fb874` | `Te` 4103122–4108440；`OE` 4238117–4240854；`mE` 4243839–4244810 |

允许的静态验证已通过：

- `node tools/audit-receiver-interactions-current.cjs --check`
- `node tools/audit-receiver-current.cjs --check`
- `python tools/validate-dock-pairing.py`：两个当前底座源、20 个语言映射、27 个资源
- `rustfmt --edition 2024 src/features/dock_pairing.rs`

Cargo 由主线程合并其他子任务后统一检查。未做实际窗口、DPI、字体或鼠标命中验收。

## 仍未完成

179 目前只实现源 `se` 的等待 `DUALLINK_BIND_INFO` 加载态；不能把它算作完整配对工具。源已定义的已加载/扫描/候选/配对/解绑/固件条件等 UI 与真实观测仍需继续接入；本轮没有把无 transport 当作空设备响应。

164/241 的实际配对读回仍未连接；运行页不能依赖预览场景证明设备读回。241 `Ps` 的轮询率说明还需完整 `V = !W && (I ? Y : K)` 条件：`hs` 查询连接状态，`Cs` 查询产品配置是否包含 `pollingRate` / `pollingRateWireless` / `pollingRateBle`；现代码的 `!both_devices_capped` 不等价。双设备按类别行显示还受 `J = t.length > 1 && (W || I)` 控制；当前 241 配置打开 W 时的双设备路径已覆盖，其他条件仍不能凭通道数推断。名称大小写、长名称连续文字换行、临时配对/加载分支、配对弹层完整视口滚动及焦点实窗验证继续待办。

DLL 修改状态、设备/服务写回与持久化仍后置。本轮没有修改本地配置草稿结构，也未增加模拟设备成功。
