# 当前 Settings 源码、页面与功能边界

两个设置应用分别核对，不能把独立 `/settings/` 的 Software/Systray/General 用于 Synapse 内部 `/synapse/settings/`。

| 当前应用 | 原代码与详细依据 |
| --- | --- |
| Synapse Settings | `.ref/applications/synapse/settings/`；[独立生产读取与字节比较](settings-current-source-check.json)、[实际挂载与入口证据](settings-entrypoints-current-evidence.json) |
| 独立 Settings | `.ref/applications/settings/` 与 `.ref/host-4.0.827/`；[根组件、CSS、语言与宿主证据](settings-window-current-evidence.json)、[具体呈现核对](settings-presentation-current.md) |
| 版本与宿主 | [当前 Dashboard 版本](20-current-source-version.md)、[当前宿主静态提取](current-host-version-audit.md) |

## Synapse 的实际页面

当前720 chunk模块4914：`kr → po + mo`。`po` 挂顶部导航 `Ji`，`$i` 只有Synapse/General；`mo` 按active_view挂 `ho/uo`。`ho → Q/ee` 左栏自动启动 `Ks`、通知 `bn`、推荐 `Ia`；右栏教程重置 `Un`、配置迁移 `Ta`、满足系统版本条件才挂设备灯光 `ia`。`uo` 左栏是语言与Release Notes `Te`，右栏是About `Fs`。

原代码没有整个设置主体的外层Dialog。旧 `.side-navigation`/`.setting-content` CSS没有出现在当前挂载链，不实施250px侧栏或80px尾部偏移。已按实际正文修正内距10/20/20、最大1240、600px列、卡片上下10px margin、30/40px内距、5px圆角及18px RazerF5标题；窄窗口按同特异性媒体规则的最终顺序处理，不添加无依据的列间gap。推荐开关位于标题内，不重复显示标题。详见 [入口与几何](settings-entrypoints-current.md)。

| 原功能链 | 当前实现与必要缺口 |
| --- | --- |
| `Ks` 读 `getAppAutoStart/getMinimizedOnStartUp`、点击对应setter | 当前只有本地 `startup_draft`；原宿主/系统读取与setter尚未接通，不表示已应用 |
| `bn` 通知localStorage、storage监听、`updateAppData` | 本地偏好可编辑持久化；原宿主appData和跨窗口状态发布仍缺 |
| `Ia` show/ignoredList/showNewRelease/showNewDeals | 本地偏好及分类已有编辑；真实目录和原宿主存储同步仍缺 |
| `Un` 教程键及设备级AI教程对象重置 | Shell已有教程状态处理；全部原键、原对象更新和跨窗口回收不能用单一本地标记替代 |
| `Ta` 激活/创建 `syn3-profile-migration` | 正式按钮及顶部按钮共用正式HostTab，已移除外层样例Dialog；实际扫描/迁移原生链仍缺 |
| `ia` OS build≥22631或22621且revision≥2506；控制权、switching、原存储、`msSettings` | 支持门控及说明已有消费者；未知控制权禁用切换，真实观察和写回仍缺 |
| `Te` 语言、engineVersion≥4.0.633、loading、BroadcastChannel回应 | 本地语言及Release Notes入口存在；原版本门控、完整loading/回应仍缺 |
| `Fs` Dashboard manifest，异常回退 `getAppData("synapse","version")` | About用已获取当前官方manifest；原运行期宿主回退未接通，不用Rust crate版本冒充 |

原版没有第三个“服务连接”设置导航。已移除该页、产品预览选择器、源码展示及测试快捷按钮，并移除对应设置事件与初始化订阅。设置只呈现原 Synapse/General；服务状态与设备发现仍由已有后台 owner 管理，不依赖该诊断页挂载。原内部校准、配对、编辑、删除弹层保留各自确认语义。

## 独立 Settings

`SettingsWindow` 是独立宿主窗口。窗口policy、分组、字体、Software标题、空Launcher预览、输入标签、Systray描述、About链接与图标分别见 [窗口证据](settings-window-current-evidence.json)及[呈现契约](settings-presentation-current.md)。维护工具为 `prepare-settings-window.cjs`、`prepare-settings-window-assets.py`、`audit-settings-presentation-current.cjs`。

安装应用/Widgets目录、Launcher排序/显示项、安装/卸载/刷新、宿主localStorage发布及系统状态仍有缺口；本地偏好不等于原宿主保存，资源和描述器不等于功能完成。Systray与托盘设置继续沿各自owner核对。

当前仅做静态解析、资源/哈希校验与 `cargo check --locked --all-targets`。没有运行应用、测试、DLL、厂商JavaScript或硬件操作；画面、字体、DPI、滚动、焦点、锚点、动画及系统写回均未运行验收。整个Settings仍是部分实现，所有必要缺口继续属于全量目标。
