# 键盘与 keypad 功能阅读索引

本目录覆盖当前 73 个键盘/keypad 产品的实际页面与主要本机调用链。产品页保留每款导航数组、产品参数、真实组件和回调字节位置；功能页解释这些组件如何读取、编辑、传输、消费应答、持久化、取消和清理。源码结构、已还原语义、Rust 实现及设备验收是分别计量的结果。

| 页面或功能 | 阅读入口 | 当前产品分支 |
| --- | --- | --- |
| 产品身份、配置档和板载槽 | [profiles.md](profiles.md) | 本机文档、GUID/预设身份、布局、OBM、宏事务、保存/恢复 |
| Customize | [customize.md](customize.md) | 全部产品；Factory、keypad、two-tap、Normal Mode、BLE 各有实际调用条件 |
| ACTUATION | [actuation.md](actuation.md) | 580、614、642、678、679、688、719、720、721、728、740、741、742、746、747、756 |
| Calibration | [calibration.md](calibration.md) | 740、746 的单键流程；756 V2/轴体行程与映射/板载联动 |
| Lighting | [lighting-power.md](lighting-power.md) | 全部产品；Quick/Advanced、Windows 动态灯光、Factory 和 BLE 条件 |
| Power | [lighting-power.md](lighting-power.md#无线-dim休眠和低功耗说明) | 574、585、600、602、631、658、664、691、697、708、716、717、724、727、756、2595 |
| OLED | [oled.md](oled.md) | 691 当前导航实际挂载；本机图像、文字、媒体/系统监测、语言、HID 分包和取消 |
| 固件、驱动和本机更新 | [firmware.md](firmware.md) | 691/02B4 的 MCU HID/WinUSB 更新器、第二遍读回、RzDev 过滤器与虚拟 HID 边界 |
| Help、设备身份和 reset | [跨产品 Help](../shared/help-reset.md) | 结合每款传入的 resetObm、BLE、OLED、固件说明和 edition |
| 底层灯光引擎 | [共享灯光消费者](../system/customize-lighting.md#quickadvanced-到真实灯光帧) | 设备文档变化 → 引擎 → 真实帧传输 → 产品恢复与退出 |

## 逐产品入口

| PID | PID | PID | PID | PID | PID |
| --- | --- | --- | --- | --- | --- |
| [515](products/515.md) | [521](products/521.md) | [529](products/529.md) | [534](products/534.md) | [542](products/542.md) | [545](products/545.md) |
| [550](products/550.md) | [551](products/551.md) | [552](products/552.md) | [554](products/554.md) | [555](products/555.md) | [556](products/556.md) |
| [565](products/565.md) | [567](products/567.md) | [574](products/574.md) | [575](products/575.md) | [577](products/577.md) | [579](products/579.md) |
| [580](products/580.md) | [585](products/585.md) | [590](products/590.md) | [591](products/591.md) | [592](products/592.md) | [593](products/593.md) |
| [599](products/599.md) | [600](products/600.md) | [602](products/602.md) | [605](products/605.md) | [606](products/606.md) | [614](products/614.md) |
| [617](products/617.md) | [619](products/619.md) | [620](products/620.md) | [631](products/631.md) | [642](products/642.md) | [647](products/647.md) |
| [653](products/653.md) | [654](products/654.md) | [655](products/655.md) | [658](products/658.md) | [659](products/659.md) | [660](products/660.md) |
| [661](products/661.md) | [664](products/664.md) | [673](products/673.md) | [674](products/674.md) | [675](products/675.md) | [677](products/677.md) |
| [678](products/678.md) | [679](products/679.md) | [688](products/688.md) | [689](products/689.md) | [691](products/691.md) | [697](products/697.md) |
| [708](products/708.md) | [716](products/716.md) | [717](products/717.md) | [719](products/719.md) | [720](products/720.md) | [721](products/721.md) |
| [724](products/724.md) | [727](products/727.md) | [728](products/728.md) | [739](products/739.md) | [740](products/740.md) | [741](products/741.md) |
| [742](products/742.md) | [746](products/746.md) | [747](products/747.md) | [752](products/752.md) | [756](products/756.md) | [2595](products/2595.md) |
| [2596](products/2596.md) |  |  |  |  |  |

## 读取和成功状态的含义

打开设备页先走当前产品的连接/初始化读取和活动档选择，不能因为本机无配置或读取失败就覆盖设备。Factory/预设 GUID、本机档、板载 slot 和当前连接实例分别建模。页面默认值、草稿、软件文档与真正 getter/应答结果在功能页逐项说明。

原版许多编辑是乐观 UI：先改变 reducer、保存本机 version、清草稿，再安排可跳过任务或共享消费者。某些 runner 吞局部异常或不检查 setter status；OLED flash 则有明确 command-id/status/分包取消。这些差异必须进入独立实现的状态与错误模型，不能统一解释为“保存成功”。

所有链接的原件来自当前 `.analysis/inputs/frontend/` 收据，使用 UTF-8 字节范围复核。source map 不可用时仍直接还原真实 JS 模块、函数和跨模块消费者；可读名称属于静态分析结果，不声称恢复了未取得的官方源文件名。本文档没有运行厂商 JavaScript、主应用、安装器或动态调试，没有将静态核对冒充真实设备验收。
