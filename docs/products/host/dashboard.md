# 本机 Dashboard 与设备卡片

Dashboard 汇总已发现设备、设备中间件公布的运行摘要和已安装模块，再把操作路由到对应页面。卡片不是另一套设备设置来源：配置档名称、电量、就绪状态和配件来自各自运行摘要，进入卡片也不代表设备读取或写入已经完成。

## 挂载与导航

Synapse 主页面按原件顺序挂载 `DASHBOARD_HEADER`、`GAMER_ROOM_HEADER`、`DEVICES_AND_MODULES_HEADER`、`GLOBAL_SHORTCUT_HEADER`，分别延迟加载模块 502、19388、44442、94608。Devices & Modules 与 Gamer Room 是两个入口，不能因共享主机外壳而合并。Global Shortcuts 的未保存映射通过共享保存提示阻止离开；完成回调后才更新当前导航。

历史记录从 sessionStorage 的 `tabNavigations_dashboard` 与 `currentTabNavigation_dashboard` 恢复。新增导航在当前位置截断前进历史，返回／前进只改变当前索引。外部 `focusSubTab` 消息可导航到 Gamer Room；全局广播另支持 Devices & Modules 和 Global Shortcuts。`dashboardDisabled` 是覆盖整个窗口的半透明阻断层。

根组件发布 `dependent-app-status`，管理模块窗口、设备窗口聚焦和后台窗口激活。窗口激活使用零延迟定时器合并连续请求；卸载会取消该定时器、终止窗口协调器、移除全局消息与 pointerdown 监听并移除运行标记。原件卸载时对 `broadcastMessage` 再次调用 `on`，并非配对的 `off`；两个 `apiElectron.response` 订阅也未在该类中展示释放，不能把它们描述为已完整清理。

## 设备集合与身份

本机设备组组合 `devices`、`validDevices` 和 `installedDevices`：先用 PID 与 container ID 查有效摘要，具有有效序列号时按 PID＋序列号去重；没有有效摘要、但尚未出现在已安装集合中的项保留安装中卡片。连接、安装与设备就绪是不同状态。

当某 PID 已有另一 container 的实际序列号且处于非 off 状态时，缺序列号或为 `NOSERIALNUMBER` 的离线副本被过滤。Xbox／PlayStation 分支有独立条件；兼容设备、Windows 10 设备、Xbox 耳机、Xbox 手柄和开发中设备分别使用分组卡片。空集合才挂载 noDevice。

普通卡片通过 `connectedDevices` 的 window-storage 事件匹配 PID＋container 更新摘要，展示 `loading`、`ready`、`waiting`、`error`、`canceled`、无 alive、`standby` 和 `off` 等状态。当前配置档从 `activeProfile` GUID 在 profiles 中查名称，自动切换另附 Auto；IOT／复合设备使用 `isShownOnUI` 子设备，不能拿主接收器身份代替子设备。

## 卡片动作与电量

卡片把点击动作交给拖动所有者：mousedown 开始会话，mouseup 且位移未超过约 10 px 才执行页面聚焦。需重启、低于最低固件、preset 正在加载、off、待机、固件更新和特定系统检查各有阻断路径。卡片上的固件入口只导航到 Devices & Modules；它本身没有执行固件刷写。

电量显示区分充电、未充电、暂停、警告和离线。`level=-1` 是查询失败值，不能改写为 0%。首次上报以及从 off 恢复时，原版先显示 `-`，约两秒后才公布电量。该计时器在普通卡片的卸载方法中没有取消；window-storage 与自定义 windowstorageevent 则有对应解绑。

Windows Dynamic Lighting 图标同时要求全局功能开启、设备支持、ready 且非 BLE；点击打开对应设置窗口。无 alive 的恢复动作向 background-manager 发 `resuscitate`，不是把卡片立即改为已连接。复合设备数量徽标和 `singleProfile` 的显示各有独立条件。

## 双连接卡片

独立双连接卡片受 `global.multipairing` 控制。候选设备必须声明 `isMultiPairingDevice`、处于 ready，并通过同 container＋PID 的 dongle 检查，或明确为 `isOnlyWirelessMode`。随后读取本地 `duallink-devices`，按真实 master／slave 关系组织配对集合与每个 container 下的充电状态。

这里的配对关系决定卡片归组、身份与状态，不能仅按相同 PID 把两只设备合并，也不能由本地关系记录推定当前实际连接。普通卡片与双连接卡片是不同挂载分支；配对编辑和设备传输仍由对应 multipairing 页面与中间件拥有。

## 模块、分组和布局

本机模块从 `AVAILABLE_MODULES` 与安装记录筛选，部分本地模块有自己的额外入口条件。模块卡片按名称打开对应窗口；维护状态可阻止打开，安装中显示 spinner。状态 `enabled`／`connected`、`disabled`、`unknown` 分别采用绿色、白色和橙色。模块的 updating 检查只在 mount 调用，未展示对应的更新钩子。

普通卡片宽 290 px、高 220 px，横纵间隔均为 20 px；列数按实际容器宽度计算。每组以稳定 `boxId` 维护显示顺序、离线项和新增项，noDevice 不保留到正常排序，特定模块引导保持在末项。折叠状态按组标题保存，拖动重排同步更新组顺序；回位动画为 300 ms。

拖动会话临时注册 document 的 wheel、mousemove、mouseup；正常 reset 会移除三者。分组类没有 `componentWillUnmount`，不能声称中途卸载也执行 reset。Dashboard 容器的 ResizeObserver 以约 60 Hz 节流计算列数，卸载时取消观察，并正确移除自身 storage／memory-storage 监听。介绍横幅关闭后写入本地状态并发出 `show-profile-migration-toast`。

Chroma 的 Studio、Connect、Sensa 和 Visualizer 行为分别见 [Chroma Studio](chroma-studio.md)、[Chroma Connect](chroma-connect.md)、[Sensa](sensa.md)、[Visualizer](visualizer.md)。这些模块的设备任务和退出恢复不归普通 Dashboard 卡片拥有。

## 当前原件

以下读取均核对 source set acquisition 中实际 bytes 和 SHA-256；区间为半开 UTF-8 字节。本文只记录本机设备、模块和窗口语义。

| 来源 | 实际阅读范围 |
| --- | --- |
| [App.e11c179b.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/App.e11c179b.chunk.js) | 导航类 `[15591,19227)`；主机窗口根 `[22634,29960)` |
| [main 当前词条](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/main.e60840ab.js) | 导航词条声明 `[138806,138937)` |
| [56.5863ebef.chunk.js](../../../.analysis/inputs/frontend/synapse-dashboard-current/raw/apps.razer.com/synapse/dashboard/static/js/56.5863ebef.chunk.js) | 普通设备卡 `[3547,22762)`；模块卡 `[23083,25484)`；分组 `[102444,115568)`；Dashboard `[122126,131070)` |

这是当前官方前端的静态语义。本文没有据此声明 Rust 后端、界面接线或真实设备验收完成，也没有执行厂商 JavaScript、主程序或安装动作。
