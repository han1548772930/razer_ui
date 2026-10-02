# Profile Migration：独立配置迁移应用

更新日期：2026-10-02。原入口是Settings720 `Ta`打开`/profile-migration/`，实际根`xD → OD → wg`；源码已静态下载59份JS/CSS，见[下载记录](../../.ref/profile-migration/source.json)和[主bundle](../../.ref/profile-migration/static/js/main.512f18b6.js)。没有执行下载脚本或启动原宿主。

这是单页分组选择界面，不是按路由切换的多步向导。主区域有迁移说明、已连接设备/模块及未连接设备分组。设备和模块卡支持展开、全选/半选、独立条目选择，条目身份使用guid和owner；日期、损坏文件、游戏关联、未使用宏和宏缺失各有原图标与说明。

[当前实现](../../src/shell/profile_migration.rs)接入Settings“配置迁移”按钮。未知数据时明确提示尚未连接扫描服务，迁移按钮禁用；显式“界面预览”可检查上述卡片和空状态，不读取或写入真实配置。

| 源状态 | 画面与行为 |
| --- | --- |
| `Rt`扫描 | 300px进度弹层 |
| `Ot`准备 | 400px进度弹层，允许取消 |
| `Ot`迁移 | 400px进度弹层，取消禁用 |
| `wt`完成 | 48px完成图、游戏/宏独立警告、Continue |
| `Lt`部分失败 | 失败设备列表、红色边框、OK |
| `Rg`空状态 | 600px内容区与四步说明 |

状态弹层使用原黑70%遮罩与#111面板。预览按钮只切换示例，不生成真实迁移结果。实际功能仍需要`GetSynapse3Accounts`、`ReadSynapse3*`、XML转换器、设备身份/模块状态以及importQueue的真实服务回调；本地Profile JSON导入不等价于Synapse 3迁移。
