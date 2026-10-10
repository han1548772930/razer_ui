# 当前灯光后台与原生链

当前 source 是 `.ref/applications/synapse/lighting-engine/`，2026-10-10 从生产地址静态取得 HTML、两个 manifest 及它们声明的 155 个 JS/CSS 文件，共 158 个输入。入口 `static/js/main.3588205d.js`。第二次独立读取 HTML、两个 manifest 与 main，字节均与已保存输入一致。所有 vendor 文件只作为数据读取；未执行 JS、应用或 DLL。获取与 SHA 校验记录在 `lighting-engine-current-source-acquisition.json`。

## 实际资源归属

当前 `LightingEngineModule.initRzLightingEngine()` 读取 `installedResources`，选择 `synapse ?? Common` 数组，再按 `name === "LightingEngineDLL"` 取 `filePath`，拼接 `userDataDir\\Apps\\filePath`。`initRzLightingDriver()` 以相同链选择 `LightingDriverDLL`，传给 `initLightingDriver()`，随后通过 `doLightingDriverAction` 发 `InitDLL` 到 host 的 `lightingDriver` IPC。

初始化传入路径为空时，Windows默认值是 `userDataDir\\Apps\\Synapse\\RzLightingEngineApi.dll` 与 `userDataDir\\Apps\\Synapse\\lighting_driver.dll`；Darwin使用同位置dylib。这是原代码的空参数默认值，不是加载失败后再换路径重试。动态 `installedResources.filePath` 与默认值分开记录，不能用下载文件名猜安装路径。`lighting_driver` 和 `RzLightingEngineApi` 是不同库，Driver默认位置不是CommonDLL。Rust清单将未确定的固定路径记为None，保留原模板和默认路径；静态路径候选不等于已观察到安装位置，也不会执行DLL。

当前官方 background resource 元数据列出：

| 资源 | 版本 | 原始字节数 | SHA-256 |
|---|---|---:|---|
| LightingDriverDLL | 1.9.14.0 | 968392 | `9b701a56c5ed0c1c30d26452fb5752ee328304a197f9a36cd3c51e0cd6a4de6b` |
| LightingEngineDLL | 4.0.55.0 | 2153672 | `cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa` |

字节取得、PE exports 与 metadata 在 `lighting-native-current-acquisition.json`；完整 AST loader、调用方法和资源归属证据在 `lighting-native-current-source-evidence.json`。维护的 inventory 生成器以这些具体 loader 收据绑定资源；没有据此推断所有产品使用全部库接口。

## Native 静态证据

在私有输入副本上使用已安装 IDA Pro 8.3 与 Hex-Rays。保留 Driver 94 个、Engine 112 个被选函数的完整伪代码、原机器码 hash、RVA/end、反汇编和 incoming xref：

- `evidence/lighting-driver-ida.json`
- `evidence/lighting-engine-ida.json`

这是 export/direct reference/string anchor 的静态选择，不等于所有函数语义闭合。`tools/validate-lighting-native-ida-current.py` 将每个被选函数的字节重新对照原 PE 校验。

Driver 主要链：

| 入口/内部函数 RVA | 已确认行为 |
|---|---|
| `0x1640` Startup | 初始化全局 Driver，注册 protocol factories 与 timer watcher；首次分配的 Driver 默认 pause=false、skip_wdl=false |
| `0x1460 → 0x6b90 → 0x5f40` Configure | 将 JSON 解析到命令 dispatch；输出 JSON 增加 `succeeded`，返回字符串由 FreeString 释放；未初始化/空输入返回 null |
| `0xa770` register | 解析 handle、write_function、protocol、category、report_id、batch_processing、is_wdl_supported、row_delay_ms（默认 1）与映射；重复 handle 覆盖注册项 |
| `0xae10` unregister | 删除对应 handle/link key；不存在时返回失败并附 `Device handle … not registered` |
| `0xb790` mode.set | 只有布尔类型的 pause、skip_wdl_devices 改状态；缺字段仍返回成功 |
| `0x90f0` HookLightingCallback | 取得给定库/函数的真实 register export，将内部 callback 注册给它，保留原 callback 指针；找不到返回 false |
| `0xa100` SetWriteFFICallback | 仅首次建立 FFIWriter，后续调用不替换已有 writer |
| `0x7ec90` FFIWriter | 将真实二进制 buffer 转数值数组，发 `{function_name,buffer,device_handle}` 到 host callback；callback 类型 void，原 native 此处不能得知最终 HID 成败 |
| `0xf4d0` ProcessDeviceFrame | 根据 protocol translator 分支处理；skip_wdl 对支持 WDL 的设备直接跳过；batch 分支聚合 reports 后发送 |
| `0x2e500 → 0x2edd0 → vtable 0xb3968 → 0x7feb0` rzDevice25 | 注册并选 `protocol::Rzp25NewChroma` translator |

新取得的后台源码显示 `protocol.use_base_class` 配置列表包含 `rzDevice25Linker`、DualLinkMouse、DualLinkKeyboard、Oled、Noa。注册时传 `protocol=device.name` 和 `base_class_name=device.baseClassName`；原生按指定 fallback 选择 translator，不能把所有产品一律当 rzDevice25。

## 已实现的共享直接写回

`crates/razer-device/src/lighting_driver.rs` 恢复 `Rzp25NewChroma` 的 RGB frame 逐行写出，不加载 DLL。`NewChroma` 保留 protocol owner 的 counter（初值 1），每行生成 91 字节（含 report ID）的 report：

- transaction 为 `(counter & 31) | (category << 5)`，counter wrapping increment。
- report offset 7/8 为 `0x0f/0x03`；payload 为 profile、region、row、first/last column、该行 RGB。
- `0x7fc30` 的 checksum 为 report byte 3 到最后字节的 XOR（计算前 checksum 字节为 0），写在 offset 89。
- 原顺序逐行写入，相邻行等待 registration row_delay_ms，最后一行不等待。
- `FrameWriter` 的实际 write 与 wait 返回错误时立即停止；不能将原 void callback 已调用冒充真实设备成功。

Native 对 payload 越界有“仅日志后返回 true”的分支；Rust 对倒置 bounds、长度不匹配、过长行明确返回错误，没有伪造写出。Rust 对尾行 255 有有限循环，不复制 native uint8 自增可能溢出的循环。此二处差异分别属于错误报告与越界保护，不作为正常设备行为的依据。

## 尚未闭合的完整范围

目前新增的是当前后台 source 全量取得、两个库归属与被选 native bodies，以及一种 native RGB translator 的直接写回。Engine 的全部 effect 渲染、时间/输入事件调度、ChromaSDK 接管、batch/其他 protocol translator、LED remap、WDL/IoT/InterHaptics adapter、完整生命周期和产品 UI → engine → write → refresh 链仍需要逐项实现和核对。共享 codec 尚未接入真实产品帧生成 owner，不能标成 quickEffects 全功能完成，也不能将资源或函数数量算作整库实现完成。

静态 source/PE/IDA 收据校验与 `cargo check --locked -p razer-device --all-targets` 已通过。按当前约束没有运行设备写回、应用、测试或 DLL；runtime acceptance 未执行。
# 清单类型约束

`RzLightingEngineApi` 和 `lighting_driver` 是当前 lighting-engine 源码单独声明的两个资源。清单分别使用 `lighting_engine`、`lighting_driver`，Rust `LibraryKind` 必须包含对应类别；动态路径字段保持可选，不编造安装路径。`validate-embedded-json.py` 静态检查 serde unit enum 的完整可接受取值及清单所有嵌套字段。编译检查不能触发运行期JSON解析，因此不能单独证明清单有效。此校验不执行原生库或设备操作，也不代表灯光引擎全部功能完成。
