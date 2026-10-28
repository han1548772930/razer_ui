# 键盘固件、驱动与本机更新边界

本页记录 BlackWidow V4 Pro 75%（PID `0x02B4`）相关的本机固件更新器和 Windows 驱动静态链。宿主层的产品选择、固件包准备、事件消费者和恢复状态见[本地固件与安装生命周期](../host/firmware-and-installation.md)；本页只补充键盘原生 DLL、`RzDev_02b4.sys` 及其 HID/请求边界。所有结论来自原始字节、独立捕获和四线程复核，没有运行更新器、驱动、DLL、安装器或设备。

当前[官方产品 manifest](../../../.analysis/inputs/official-native-current/manifests/5a4be4dd242682b12825839af3b3440903975f5c1631597de39f5f2b49733421/manifest.json) 的产品身份为 `691`；USB 硬件 PID `0x02B4` 的十进制值为 `692`，两者不可互作别名。下面的驱动请求审阅只选择当前 manifest 产品 `691` 所声明的 `NormalDriver_PID02b4Drv`。

## 原件、捕获与复核

| 对象 | 原件和版本身份 | 静态捕获与复核结果 |
| --- | --- | --- |
| MCU 更新 DLL | [`RazerMCUUpdaterInterface.dll`](../../../.analysis/inputs/official-native-current/extracted/840128a4f3b8d5c9d04fc60967136a635dbf7fe9f1134b2be0e8d140f348b542/RazerMCUUpdaterInterface.dll)，32 位 PE，42,496 B，SHA-256 `05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28`，Image Base `0x10000000` | [`capture.json`](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/capture.json)，SHA-256 `bc5e006873c56cab498f56fd8b88ba60520755c4ca186a3dd2e830d653864cd0`；160/160 函数，189 个去重代码块，8,213 条去重指令，25,864 B 原始指令，0 个未列指令字节。 |
| 02b4 驱动 | [`RzDev_02b4.sys`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/KB/RzDev_02b4.sys)，x64 PE，65,272 B，SHA-256 `de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0`，Image Base `0x140000000` | [`capture.json`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/capture.json)，SHA-256 `b7ef7cc01de8f5bcd175fc6dd13000941f1281ea9ea2290b88c3d81967fc0462`；123/123 函数，123 个去重代码块，7,402 条去重指令，31,400 B 原始指令。另有 186 B 对齐填充（`CC`、`90` 和多字节 NOP），已逐字节核对，不计入代码。 |

MCU 更新器来自本地固件包 `840128a4f3b8d5c9d04fc60967136a635dbf7fe9f1134b2be0e8d140f348b542`（容器 SHA-256 `840128a4f3b8d5c9d04fc60967136a635dbf7fe9f1134b2be0e8d140f348b542`）；02b4 驱动来自 `PID02b4Drv_v23.0.0.0.exe`（容器 SHA-256 `cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f`）。两次 IDA 批处理均为静态目标，执行标志为 `target_execution=false`；完整捕获不等于已完成设备语义或运行时验收。

## MCU 导出面

下面的 RVA 以 DLL Image Base `0x10000000` 为基准；每个入口链接到独立的原始函数 JSON。

| 序号 | 导出 | RVA | 序号 | 导出 | RVA |
| ---: | --- | ---: | ---: | --- | ---: |
| 1 | [BackToDefault](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/2130.json) | `0x2130` | 13 | [SetOledBrightnessScaling](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/43a0.json) | `0x43A0` |
| 2 | [DFUExitEx](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/2080.json) | `0x2080` | 14 | [SetOledScreenLowPowerMode](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4210.json) | `0x4210` |
| 3 | [GetDeviceFWVersion](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1f40.json) | `0x1F40` | 15 | [SetTransactionID](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/41f0.json) | `0x41F0` |
| 4 | [GetDeviceSerialNumber](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/42f0.json) | `0x42F0` | 16 | [UninstallBootLoaderDriver](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1cc0.json) | `0x1CC0` |
| 5 | [GetDockMACAddress](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/49c0.json) | `0x49C0` | 17 | [WinUSB_SendCommand](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1820.json) | `0x1820` |
| 6 | [GetOledBrightnessScaling](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4410.json) | `0x4410` | 18 | [delay](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1450.json) | `0x1450` |
| 7 | [GetOledScreenLowPowerMode](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4270.json) | `0x4270` | 19 | [reset_ps5](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4a50.json) | `0x4A50` |
| 8 | [InstallBootLoaderDriver](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1ca0.json) | `0x1CA0` | 20 | [upgrade_firmware_process](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/3e80.json) | `0x3E80` |
| 9 | [OpenDevice](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1ce0.json) | `0x1CE0` | 21 | [upgrade_progress_percent](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/41d0.json) | `0x41D0` |
| 10 | [OpenWinUsbBootloader](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/19d0.json) | `0x19D0` | 22 | [upgrade_ps5_process](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4880.json) | `0x4880` |
| 11 | [SetDeviceMode](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1ff0.json) | `0x1FF0` | 23 | [usbControlIn](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1770.json) | `0x1770` |
| 12 | [SetDongleCommandOption](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4200.json) | `0x4200` | 24 | [usbControlOut](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/16c0.json) | `0x16C0` |

> 校验时按原始 `entrypoints.json` 与函数边界复核了上述 24 个入口。导出存在只说明 ABI 表存在，不能替代调用者参数、设备应答或固件版本确认。

## 更新器的写入、读回和错误语义

- [OpenDevice](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1ce0.json) 按 HID 类 GUID 枚举 VID/PID，并要求 `FeatureReportByteLength == 91`；失败会释放 preparsed data、设备列表和句柄。WinUSB 引导枚举由 [OpenWinUsbBootloader](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/19d0.json) 独立管理，不能把两种句柄混用。
- 内部 [0x1520 report 事务](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1520.json) 固定处理 91 B Feature report：对 offset `3..88` 做 XOR，写入 offset `89`（第 90 字节），SET 最多尝试三次，再以 GET 最多轮询二十次。原件区分写失败、读取失败、忙和设备状态；上层将状态 `2` 作为成功门禁，不能只看 `HidD_SetFeature` 的 BOOL。
- [upgrade_firmware_process](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/3e80.json) 在普通分支打开进程目录的 `new_fw.bin`，解析 HEX 后先发 `0x10/0x01` 擦除，再按最多 64 B 发 `0x02` 数据。随后第二遍用 `0x10/0x83` 读取实际设备字节并比较；写失败、读失败和字节不等分别保留为独立失败。入口的版本门禁使用 `0x10/0x80`，并按大端读取响应中 offset `16..17` 的 PID；不匹配返回非零。
- `normal=false` 的 WinUSB 分支把本地块经 OUT `0x87` 发送，并用 OUT `0x82` 擦除 512 B；捕获没有看到对应的逐字节设备读回，因此不能把两分支合并成同一确认语义。成功结束的 `DFUExitEx`/PS5 结束 report 只发退出动作，返回值不是最终固件版本。
- [WinUSB_SendCommand](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1820.json) 的控制 OUT 之后还要等待并读取固定状态字节；读回失败或 `0xFF` 会重试，不能按第一次 OUT 的 BOOL 判定成功。`GetDeviceFWVersion`、序列号和 MAC 是独立导出，升级成功没有自动把这些读取结果变成设备已确认状态。
- [upgrade_progress_percent](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/41d0.json) 只更新进度状态；回调返回值未形成取消门禁，捕获没有取消令牌、恢复写入或中断后重连确认。`InstallBootLoaderDriver`/`UninstallBootLoaderDriver` 仅走 `ShellExecuteA`，不能把进程启动或句柄返回值当作安装完成。

## 02b4 驱动的产品接口与 INF 分支

该驱动没有 PE 导出；`DriverEntry` 位于 RVA `0x7B10`。同一产品包包含键盘、鼠标、虚拟消费者接口，不能把包内鼠标路径或虚拟 PDO 的静态响应冒充键盘固件读回。

| INF | 绑定接口 | 关键安装身份 | SHA-256 |
| --- | --- | --- | --- |
| [`RzDevU_02b4_Kbd.inf`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/KB/RzDevU_02b4_Kbd.inf) | `HID\Vid1532&Pid02b4&MI01&Col01` | `UpperFilter`，BlackWidowV4Pro75% | `e44d565d85d71f069a163a71fedb71324d3e9b76208072573bab84bb0a39ecd6` |
| [`RzDevU_02b4_Kbd2.inf`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/KB/RzDevU_02b4_Kbd2.inf) | `HID\Vid1532&Pid02b4&MI00` | `UpperFilter`，`Kbd2Device1` | `9fc33c244096af6ba69d3083891419edc1e388bb7cbcde0168c9ff00433f8c8a` |
| [`RzDevU_02b4_Mou.inf`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/RzDevU_02b4_Mou.inf) | `HID\Vid1532&Pid02b4&MI02` | `UpperFilter`，`ControlDevice1` | `1548fc598a60a3f9382b1510eeb6861d0dcf1a47bebae55ddadb8f0a6c610f59` |
| [`RzDevU_02b4_Mou2.inf`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/RzDevU_02b4_Mou2.inf) | `HID\Vid1532&Pid02b4&MI01&Col02` | `UpperFilter`，`Mou2Device1` | `643cfbc3386e25bd3780daa8af58ebb27e594e5296f22eec55d49865f91e4b9d` |
| [`RzDevU_02b4_Dkm.inf`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/Lower/RzDevU_02b4_Dkm.inf) | `USB\...&MI01` | `LowerFilter`，`DkmKey1`、`Virtual_Con_02b4=1` | `06b7a387ba6e24a438748ab7f4e318810feb030454b7a7d0e500f985cc2cc953` |
| [`RzDevU_02b4_MPos.inf`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/MPos/RzDevU_02b4_MPos.inf) | `USB\...&MI02` | `LowerFilter`，`MouseExDevice1` | `13a7ad0787e86cd072e1bdf3bb8898f4e8626658e4d4616d48797c1ecf4a9fce` |
| [`RzDevU_02b4_VCon.inf`](../../../.analysis/inputs/official-native-current/extracted/cd5c94cbe4cdb3a873671a8e328ac1c5ad3df3af266353b2359f7e5c48c4034f/Drivers/x64/VCon/RzDevU_02b4_VCon.inf) | `RZVIRTUAL\Vid1532&Pid02b4&MI00&Col03` | `mshidkmdfUpper`，`ConsumerControlDevice1` | `3bb411a35cabed5be7dd508295aa5bf1ec22a8b18ba284c2abc7cda1622aaa26` |

## 键盘请求、虚拟 HID 与真实设备边界

- [AddDevice 初始化 `0x27E0`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/27e0.json) 读取硬件 ID、符号链接、容器属性和注册表开关。`Virtual_Kbd`/`Virtual_Kbd_%04X` 走 [虚拟子设备创建 `0x5F74`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/5f74.json) 的类型 `2`；键盘、鼠标、JS 和消费者 PDO 使用不同 HID 描述符和 report 描述符。它们是输入注入/读取接口，不是 MCU bootloader。
- [虚拟 HID 分发 `0x6B60`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/6b60.json) 对 `0xB0003`/`0xB0007` 返回固定 HID/report descriptor；`0xB000B` 只把 READ_REPORT 转发到手工队列；`0xB000F` 和列入同一分支的 `0xB0013`、`0xB001F`、`0xB0023`、`0xB002B`、`0xB0191`、`0xB0192` 完成请求，不执行物理 Feature report 传输。未知代码返回 `STATUS_NOT_SUPPORTED`。输出缓冲小于 4 B 时只完成状态，否则以实际取出的输出长度完成；这两个分支都不能作为固件读回 ACK。
- [物理过滤器 `0x3260`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/3260.json) 对 `0xB000B`/`0xB000F` 仅在 flags `0x140` 下走合成路径；键盘 class connect（`0xB0203`）保存原始 ConnectData 并替换回调，disconnect（`0xB0403`）返回 `STATUS_NOT_IMPLEMENTED`。USB URB function `9` 注册 [完成回调 `0x3A60`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/3a60.json) 后才转发到底层 target；这仍是输入报告链，不是固件更新。
- [合成 READ `0x3914`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/3914.json) 在有缓存报告时用 `-10000`（100 ns 单位，约 1 ms）启动 timer，然后把请求放入 manual queue；转发失败会完成错误。 [合成 WRITE `0x400C`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/400c.json) 从 HID_XFER_PACKET 读取非空 report，复制到 64 B 上下文并完成 `0`；函数内没有可见的 `<=64` 长度门禁，Rust 侧必须先做边界校验，不能照搬潜在越界语义。
- [报告消费者 `0x2750`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/2750.json) 设置缓存长度、pending 标志并启动同一 timer；[timer drain `0x2650`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/2650.json) 从物理合成队列取请求、复制缓存并清 pending，输出缓冲获取失败时保留 pending 并完成该次请求。它没有设备读回校验；排队期间的取消由下节的 WDF 所有权契约处理。
- [输入报告 `0x3C80`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/3c80.json) 只处理 1..64 B。DkmKey 报告 ID `4/8` 与键缓存比较，先发释放再发按键并替换缓存；mouse、MPos 和 MouseEx 分支走各自回调。 [键盘 class 回调 `0x3670`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/3670.json) 只改输入单元 ID 后调用原回调；这些都不能推导固件版本、写入 ACK 或恢复成功。

## 控制查询、队列取消与父对象释放

- [查询接口路由 `0x1A50`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/1a50.json) 把 `0x88883010` 路由到 HID SET、`0x88883014` 路由到 HID GET、`0x88883140` 关闭 target。 [HID SET `0x3C10`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/3c10.json) 和 [HID GET `0x25EC`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/25ec.json) 先惰性创建 target（[0x382C](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/382c.json)），再通过 [同步 IOCTL `0x3B40`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/3b40.json) 返回 `NTSTATUS`。该调用没有配置 bytes-returned、协议状态解析或取消句柄；缺 driver/device 时函数可返回 `0`，缺 target 则返回 `0xC0000001`，所以 NTSTATUS 成功不等于 MCU/HID 应答成功。
- 物理设备的真实清理注册链为：[Driver Add `0xD000`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/d000.json) → [设备创建 `0x527C`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/527c.json)。后者在 `WdfDeviceCreate` 的 56 B `WDF_OBJECT_ATTRIBUTES` 中注册 `EvtCleanupCallback=0x41A0`、context type `0xB018`；[构造 `0x46CC`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/46cc.json) 将 context vtable 设为 `0x9248`。[cleanup wrapper `0x41A0`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/41a0.json) 要求全局 driver 仍存在且 [关联查询 `0x58A8`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/58a8.json) 成功，随后经 vtable `+8` 调用 `0x2010`。不能将 [控制 PDO 创建 `0x4944`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/4944.json) 注册的 `0x1FC0`/context `0xB040` 当成此物理设备的 cleanup。
- [设备清理 `0x2010`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/2010.json) 清空关联字段，调用 [关联拆除 `0x5624`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/5624.json) 与 [target 关闭 `0x2054`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/2054.json)，最后清空 context `+32` 的设备字段。`0x2054` 先清 context `+1120`，再调用 `WdfIoTargetClose` 和 `WdfObjectDelete`；[回调注册 `0x1630`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/1630.json) 支持的槽位在注销时恢复原回调。以上是驱动的 cleanup 回调及关联拆除，不能把其返回 `0` 当作所有子对象、在途 timer 或设备应答都已结束。

### 02b4 的 WDF 请求与 timer 所有权

平台 ABI 和生命周期依据独立保留于 [KMDF 1.15 acquisition](../../../.analysis/inputs/platform-abi-current/wdf-kmdf-1.15/acquisition.json)：5 个版本匹配头文件、6 个 API 契约及 3 个许可证，共 146,431 B；25 个声明绑定准确原始行区间和 SHA。它们只解释平台调用，不提供 Razer 协议或设备成功证据。驱动 `WdfVersionBind` 的静态绑定记录位于 RVA `0xB0D0`：Size `0x30`、版本字段 `1/15/0`、函数数量 `444`、函数表地址槽 `0xB3F8`。以下偏移与 [1.15 函数枚举](../../../.analysis/inputs/platform-abi-current/wdf-kmdf-1.15/headers/wdffuncenum.h) 一致，均为 x64 的 8 B 表项。

| 表偏移／索引 | 已核平台函数 | 当前驱动消费者 |
| --- | --- | --- |
| `1216 / 152`、`1264 / 158` | `WdfIoQueueCreate`、`WdfIoQueueRetrieveNextRequest` | 创建 manual queue；`0x2650` 取物理合成 READ |
| `2248 / 281` | `WdfRequestForwardToIoQueue` | `0x3914` 的物理合成 READ；`0x6B60` 的虚拟 READ |
| `2104 / 263`、`2120 / 265`、`2160 / 270` | `WdfRequestComplete`、`WdfRequestCompleteWithInformation`、`WdfRequestRetrieveOutputBuffer` | 入队失败立即完成；取出后检查输出、复制并完成 |
| `2544 / 318`、`2552 / 319`、`2568 / 321` | `WdfTimerCreate`、`WdfTimerStart`、`WdfTimerGetParentObject` | `0x27E0` 创建、`0x3914/0x2750` 启动、`0x4140` 找回物理 context |
| `1360 / 170`、`1664 / 208` | `WdfIoTargetClose`、`WdfObjectDelete` | `0x2054` 关闭及删除本地 target |

物理合成路径在 flags `&0x140` 时由 `0x27E0` 创建 96 B queue config：`DispatchType=3`（manual）、`PowerManaged=0`（`WdfFalse`），无自定义 `EvtIoCanceledOnQueue`、`EvtIoStop` 或 `EvtIoResume`，queue attributes 参数为空，父对象默认为传入的物理 `WDFDEVICE`。context `+1720` 保存队列；`+1712` 保存 timer；`+1728/+1732/+1736` 分别为 pending、长度和 64 B 缓存。该函数丢弃 queue-create 的 `NTSTATUS`，随后按 timer-create 状态继续；不能将这个控制流视为队列一定创建成功。

40 B timer config 指定 callback `0x4140`、`Period=0`、`AutomaticSerialization=0`；56 B attributes 明确 `ParentObject=context+32` 的物理设备。[timer callback `0x4140`](../../../.analysis/results/native/de33422e04960735e09df34c2f96dc9db9cc7d04c57f0df56726cfbe0ab239d0/functions/4140.json) 由 `WdfTimerGetParentObject` 找回同一 context，再调用 `0x2650`。`0x3914` 有 pending 时先以 `-10000` 启动 timer，再入队；`0x2750` 更新缓存后也启动同一 timer。约 1 ms 是相对调度时间，不是请求超时，timer-start 的返回也未形成成功门禁。`0x2650` 只取一个请求；获取输出缓冲成功后复制报告、清 pending，并立即按缓存长度完成；失败时保留 pending，用输出长度变量完成该次错误。函数没有保存请求指针或异步等待设备。

[Forward 契约](../../../.analysis/inputs/platform-abi-current/wdf-kmdf-1.15/contracts/nf-wdfrequest-wdfrequestforwardtoioqueue.md) 明确：入队成功后框架拥有请求，在重新交给驱动前可以取消它且不通知驱动；[Retrieve 契约](../../../.analysis/inputs/platform-abi-current/wdf-kmdf-1.15/contracts/nf-wdfio-wdfioqueueretrievenextrequest.md) 则在取出成功后将请求所有权交给驱动。这里入队失败由原调用立即完成，取出成功后由 `0x2650` 同步完成。因此未调用 `WdfRequestMarkCancelable` 不构成“排队请求不能取消”的证据；不能再为这个排队阶段虚构一条驱动取消回调。此结论不扩展到控制查询的同步发送、已取出请求的真实竞态或固件操作取消。

虚拟 PDO 是另一条对象链：`0x5F74` 的默认 queue 使用 parallel dispatch、`PowerManaged=2`（`WdfUseDefault`），manual queue 使用 dispatch `3`、`PowerManaged=0`，后者 attributes 的父对象明确为虚拟 PDO，句柄存入虚拟 context `+16`。`0x6B60` 的 `0xB000B` 只向这个队列转发请求。完整捕获中，索引 `158` 的唯一取请求调用为 `0x2650`，参数来自物理 context `+1720`；没有将虚拟 `+16` 读请求接入该取样消费者的静态调用，不能把两条队列合并而声称虚拟 READ 已收到物理样本。

[Queue 创建契约](../../../.analysis/inputs/platform-abi-current/wdf-kmdf-1.15/contracts/nf-wdfio-wdfioqueuecreate.md) 和 [Timer 创建契约](../../../.analysis/inputs/platform-abi-current/wdf-kmdf-1.15/contracts/nf-wdftimer-wdftimercreate.md) 规定父对象删除时框架删除对应 queue/timer；原件没有显式 `WdfTimerStop` 或 `WdfIoQueuePurge` 不等于它们没有框架释放所有者。[对象删除契约](../../../.analysis/inputs/platform-abi-current/wdf-kmdf-1.15/contracts/nf-wdfobject-wdfobjectdelete.md) 同时指出删除可在子对象实际删除前返回，子对象删除顺序不可预测。Rust 必须分别持有请求、队列、timer 与连接代次，并验证取消、移除、回调排空和释放；本轮没有安装或运行驱动，没有测得取消完成状态、移除时序或 timer/cleanup 竞态，运行验收保持 `Unverified`。

正式机器审阅位于 [keyboard-mw-audit.json](../../../.analysis/results/frontend/keyboard-mw-audit.json) 的 `queries.driver_request_lifecycle`。统一入口 `python tools/workspace.py audit-family-function --family keyboard --function mw --query keyboard-driver-request-lifecycle --product 691` 只更新该有界节点；`--native-driver-lifecycle check` 从原始 PE、完整捕获和平台契约独立复核，`--native-driver-lifecycle negative` 检验拒收反例。审阅只保留 17 个选中完整所有者的引用和正文身份，校验时重读全部 123 个函数、31,400 B 原始代码和 7,402 条指令，不复制整套函数正文。误用硬件 PID `692`、误接控制 PDO cleanup、将虚拟队列冒充物理取样、把 timer 当作超时、改写 ABI 或提升运行验收均须拒绝。

## PID 02D7 驱动链

`PID02d7Drv_v25.0.0.0.exe` 所属容器 SHA-256 为 `41b3202af490adefdd8ed4864e05242c362ec74ae5d9f24a35a44a3dd93d5723`。本节只记录其中的 `RzDev_02d7.sys` 静态输入：x64、65,392 B，SHA-256 `bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46`；其 [capture.json](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/capture.json) SHA-256 为 `45e9bcff2d27541ff54f754fd17566c0b2ccf08da286aa72123fa4c9c981d122`，121/121 函数、121 个代码块、7,394 条指令、31,205 B 原始代码、0 个 Hex-Rays 失败。对应 [RzDevU_02d7_Kbd.inf](../../../.analysis/inputs/official-native-current/extracted/41b3202af490adefdd8ed4864e05242c362ec74ae5d9f24a35a44a3dd93d5723/Drivers/x64/KB/RzDevU_02d7_Kbd.inf) 为 2,007 B，SHA-256 `30131c38e2621c56fa71fb821c63cb28ba636ec3834785474e27a6895986f813`，`DriverVer=04/21/2025,10.0.15063.0`，键盘接口为 `HID\Vid_1532&Pid_02d7&MI_01&Col01`，过滤器为 `RzDev_02d7`。

- [AddDevice `0x27B0`](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/functions/27b0.json) 读取设备 ID 和注册表配置，再由 [虚拟键盘创建 `0x5FE4`](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/functions/5fe4.json) 建立 `Virtual_Kbd` 类型 2 的 PDO。[HID/report descriptor `0x6BD0`](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/functions/6bd0.json) 提供虚拟输入描述符。
- [物理 class connect `0x32B0`](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/functions/32b0.json) 保存 READ/WRITE 的 URB 路径；[合成 READ/WRITE `0x3980/0x407C`](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/functions/3980.json) 及 [URB 完成 `0x3CF0`](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/functions/3cf0.json) 只闭合输入报告转发和缓存，不证明 MCU 命令或固件 ACK。
- [报告消费者 `0x3AD0`](../../../.analysis/results/native/bc46c2713e2df81f85af55aa138944d05d028be7fe06afd487e7bcd936e5cf46/functions/3ad0.json) 更新回调输入单元并转发原始报告。MI_03 HID、MI_02 USB lower-filter 和 `RZVIRTUAL` consumer-control INF 分支分别注册不同的对象；`DriverEntry` 的 WDF 初始化与释放没有出现统一的请求取消、队列清空或设备应答路径。

该驱动与 02b4 的对象布局和 report 分支不能按产品名合并。当前只核对静态输入、虚拟 HID、物理过滤器和释放边界；没有运行驱动、设备、报告读回或恢复，状态保持 `Unverified`。

## PID 006A 驱动链

包容器 SHA-256 为 `302b4ab125e11ce18c42e21e009b11753af3ec5478ac339585e108350a69134b`。同包的两个 x64 驱动版本分别为：

| 模块 | 原件 SHA-256 / 字节 | capture SHA-256 | 捕获结果 |
| --- | --- | --- | --- |
| `RzDev_006a.sys` | `b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a` / 54,152 B | `26ce4befc5f303d496413d0a4766b14556e315945ee00f07a5a1dc0cbf809ba4` | 114/114 函数、114 个代码块、6,189 条指令、25,342 B、0 失败 |
| `RzDev_006a_1.sys` | `4dbc616f7ea4390d77edf466c31797f53ec1b6b593164c8c93b4aa9ffbbbedb2` / 50,568 B | `ada5cfd374e9abf2e0cf609c21df519b6988b3ae697960078be6bdfb8cee445b` | 112/112 函数、112 个代码块、5,572 条指令、22,208 B、0 失败 |

对应 [RzDevU_006a_Kbd.inf](../../../.analysis/inputs/official-native-current/extracted/302b4ab125e11ce18c42e21e009b11753af3ec5478ac339585e108350a69134b/Drivers/x64/KB/RzDevU_006a_Kbd.inf) 为 1,968 B，SHA-256 `2daf4923663e17a39baf2aaacd347de5bc0c664dd7f0d0b01640701d30cb4a5e`；它绑定 `HID\Vid_1532&Pid_006a&MI_01&Col01`，使用 `UpperFilters=RzDev_006a`，并分别为 `MI_00`、`MI_01`、`RZVIRTUAL` consumer-control 注册不同过滤器。

安装分支由当前同包的编译指令选择：[安装器完整捕获](../../../.analysis/results/native/302b4ab125e11ce18c42e21e009b11753af3ec5478ac339585e108350a69134b/capture.json) 共 93 个函数、0 个捕获失败。它读取 `$WINDIR\System32\kernel32.dll` 文件版本高 DWORD，执行 `&0xffffffff`，再按有符号 32 位整数与 `0x000a0000` 比较：小于时进入 KMDF 1.9 分支，相等或大于时进入 KMDF 1.15 分支。版本数据读取失败时空字符串参与数值解析得到零，仍走前者；这只说明原安装器的分支行为，不表示版本读取成功。两个分支写出的文件名都为 `RzDev_006a.sys`；保留原件中的 `_1` 来自静态解压的重名处理，不是原安装器的版本命名，也不能据 `Synapse3` 卸载目录推断雷云版本。

完整指令还通过同包 `System.dll` 的真实 `Call` 导出执行 `GetCurrentProcess`、`IsWow64Process`，随后将输出参数从插件栈弹入变量 37；字符串等于 `0` 时跳过后续 amd64 DPInst 调用。这里没有独立的本机架构判定，不能把 WOW64 输出或安装器退出视为已安装驱动或设备 ACK。插件调用桥 `0x27e8` 的 Hex-Rays 正栈偏移警告原样保留，全部原始指令与 PE 字节独立校验，ABI 动态执行仍未验证。

正式选择查询位于 [mouse-mw-audit.json](../../../.analysis/results/frontend/mouse-mw-audit.json) 的 `queries.driver_install_selection`：统一入口 `python tools/workspace.py audit-family-function --family mouse --function mw --query driver-install-selection` 扫描全部 334 份已保留原生资源 manifest，再按共享容器字节确定使用者；本容器的目录产品为鼠标 `106`，键盘过滤接口不改变这个身份。`--native-install-selection check` 独立重读原件、完整函数、版本 API 表、压缩记录和两份驱动 WDF 绑定；`negative` 验证阈值、跳转、文件、压缩记录、来源遗漏和验收提升等拒收反例。全局 acquisition 的 `complete=false` 与未验证资源边界继续保留，不将此查询扩大成整套安装生命周期完成。

- 新版 [AddDevice `0x23BC`](../../../.analysis/results/native/b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a/functions/23bc.json) 创建 `Virtual_Kbd`；[virtual HID `0x52F0`](../../../.analysis/results/native/b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a/functions/52f0.json) 返回 HID/report descriptor，[virtual dispatch `0x5A54`](../../../.analysis/results/native/b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a/functions/5a54.json) 处理 `0xB0003`、`0xB0007`、`0xB000B` 等请求。`0xB000B` 的 READ_REPORT 只进入驱动手工队列，不是 MCU 读回。
- [物理 dispatch `0x2D54`](../../../.analysis/results/native/b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a/functions/2d54.json) 处理 class connect 与 URB function 9；[合成 READ/WRITE `0x321C/0x3830`](../../../.analysis/results/native/b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a/functions/321c.json) 写入手工队列和 64 B 缓冲区并完成请求。[报告消费者 `0x3518`](../../../.analysis/results/native/b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a/functions/3518.json) 区分 report ID 4/5；pending timer 使用 `-200000`（100 ns 单位，约 20 ms），[queue drain `0x2258`](../../../.analysis/results/native/b455facc4d1c400bd5c84d82dca2d0924d31c8eb555b072c390065f48dd6090a/functions/2258.json) 负责排空挂起请求。
- 旧版使用 [AddDevice `0x25E0`](../../../.analysis/results/native/4dbc616f7ea4390d77edf466c31797f53ec1b6b593164c8c93b4aa9ffbbbedb2/functions/25e0.json)、[virtual HID `0x4C28`](../../../.analysis/results/native/4dbc616f7ea4390d77edf466c31797f53ec1b6b593164c8c93b4aa9ffbbbedb2/functions/4c28.json)、[virtual dispatch `0x5304`](../../../.analysis/results/native/4dbc616f7ea4390d77edf466c31797f53ec1b6b593164c8c93b4aa9ffbbbedb2/functions/5304.json) 和 [physical dispatch `0x2F00`](../../../.analysis/results/native/4dbc616f7ea4390d77edf466c31797f53ec1b6b593164c8c93b4aa9ffbbbedb2/functions/2f00.json)。两个版本都没有在选定函数中出现 `WdfRequestMarkCancelable`、`WdfIoQueuePurge` 或 `WdfTimerStop` 的显式调用；挂起 READ 的取消和设备 ACK 仍未闭合。

PID 006A 的两个版本只完成静态驱动和请求生命周期核对。没有安装驱动、打开设备、发送 HID、执行读回或恢复；Rust 实现必须把虚拟 HID 状态、物理报告、队列超时、取消和释放分别建模，不能把 `NTSTATUS` 或队列完成当成设备成功。

## 后续对照测试边界

实现阶段要把 MCU 更新器的 HID/WinUSB 写入、第二遍逐块读回、版本重连读取、失败、取消、恢复和句柄释放拆成独立状态；02b4 驱动的虚拟 descriptor、manual queue 和物理回调不能充当这些状态的成功证据。无设备时先使用来自上述捕获的输入、状态和失败分支做纯逻辑/模拟传输测试；有设备时先只读获取初始版本和 report，任何可恢复写入都必须记录原值、读回确认、恢复原值并清理。当前没有执行官方 DLL、驱动、服务或设备对照，所有动态验收状态保持 `Unverified`。
