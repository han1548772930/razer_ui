# RzAudioUtil 当前版本查询链

当前 `RzAudioUtil_v1.0.3.1.dll` 的 IDA/Hex-Rays 证据来自 SHA-256 `9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf` 的私有静态副本。`GetDLLVersion` 位于 RVA `0x1ae10`：函数把四个常量 `1, 0, 3, 1` 格式化为 `"1.0.3.1"`，按 `strlen + 1` 分配内存并复制包括 NUL 在内的字符串；分配失败返回空指针。返回指针由同一 DLL 的 `FreeMalloc`（RVA `0x1aab0`）释放。函数不读取设备、不读取文件，也不依赖初始化状态。

当前 middleware 的 `RzAudioUtil` 声明是 `GetDLLVersion: ["pointer", []]`、`FreeMalloc: ["void", ["pointer"]]`，初始化成功后调用版本查询。现有 Rust 版本查询因此要求调用方提供当前 middleware 的 product ID，并对当前已审计的产品绑定（1398、1422、1427、1446、2638、2641、4124、4126）直接返回同一源语义，响应标记 `transport=source-derived` 和 `vendor_dll_loaded=false`，不加载或执行 DLL。缺少 product ID 或没有当前资源绑定时拒绝，不猜测二进制版本。

产品 1401 当前 manifest 绑定另一份 `RzAudioUtil_v1.0.1.1.dll`（SHA-256 `012b86a320f2f9a1266cd7a0165da7b5e02e0ea2abe2aa065038089020a7bf11`）。其独立 IDA 函数体证明格式常量为 `1, 0, 1, 1`，分配、NUL 复制和释放所有权与上述版本一致。Rust 对这个产品返回 `1.0.1.1`，不把新版本的固定字符串套到旧二进制。

| 当前资源版本 | GetDLLVersion RVA / 结束 RVA | FreeMalloc RVA / 结束 RVA |
| --- | --- | --- |
| 1.0.1.1 | `0x5030 / 0x5186` | `0x4e80 / 0x4e8a` |
| 1.0.3.1 | `0x1ae10 / 0x1af66` | `0x1aab0 / 0x1aaba` |

[专项机器证据](rzaudioutil-dll-version-current-evidence.json) 保留两份完整 IDA 函数体、原件 SHA、机器码 SHA、RVA/边界、交叉引用、九个当前产品 manifest 及全部匹配 wrapper、当前 host 的 `readCString/FreeMalloc` 所有权消费。维护工具 [audit-rzaudioutil-dll-version-current.py](../../tools/audit-rzaudioutil-dll-version-current.py) 重新读取原字节并验证这些证据；`--check` 要求再生成结果与现存 JSON 一致。这里的版本是原函数的确定性语义，不是安装路径中某份 DLL 已经成功加载或已查询的观察。

本链只覆盖版本查询；`Dispatch`、`SetNodeFFIEvent`、AudioEnumerator、AudioRouter 和 MediaPlayer 仍按各自的 IDA/当前 middleware 证据单独实现，不能用版本常量代替这些功能。

已执行 `cargo test --locked -p razer-service native_query::tests`，两项纯 Rust 测试通过：九个产品的版本/原件归属，以及缺失或未证明产品的拒绝路径。测试通过公共版本入口调用 Rust 分支，不加载 DLL。
