# 当前 AudioRouter 本地 PCM 路由链

依据为当前 `RzAudioUtil_v1.0.3.1.dll`，SHA-256 `9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf`。本轮使用安装的 IDA 8.3 与 Hex-Rays，对私有输入副本做静态分析；没有加载 DLL、调用导出、启动调试器或运行目标程序。

维护证据为 [45 个函数的字节哈希、RVA、边界、伪代码与交叉引用](evidence/audio-router-ida.json)、[当前源码消费与状态语义](audio-router-current-evidence.json) 和 [36 条 JS 收据](audio-router-current-source-receipts.json)。`tools/audit-audio-router-current.py --check` 校验原 PE 函数字节与当前 JS。收据中的 offset 为 Python Unicode 字符位置。

## 当前消费者必须由注册链确认

1422 与 1446 的当前 `main.*.js` 明确注册 `genericFeature` 的 `audio_streamMixer`，开启 `ON_CHANGE_PLAY_BACK_MIX_DEVICE` 等事件。各自当前 `8539.*.js` 的 feature loader 解析 `Audio_StreamMixer`，对应类调用 `EnableRouting`、`RouteDevice`。1398、1427、2638、2641、4124、4126 的共享 bundle 中存在同名类，不等于产品实际实例化了它；不能据此自动为八个产品启用路由或注册通知。

`routeExternalDevice(e,t)` 只传 `{primaryDevice:e,routedDevice:t}`，不传 `primaryDeviceId`，因此路由状态事件的 `deviceId` 为原生默认值 0，不能擅自填入产品 PID。源 `AudioRouter_StatusChange` listener 仅 console log，不触发配置重新加载或伪造查询结果。

UI 动作、当前产品的设备类写回与本模块相互独立。服务 IPC 已实现真实本地路由；页面是否完成必须另有页面事件到设备写入、状态刷新与失败分支的证明，不能把本文件标为整页或整程序已完成。

## 原生命令与状态

| 路径 | IDA RVA | 已恢复语义 |
|---|---|---|
| `EnableRouting` | `0x197a0 → 0xba30` | 要求 `enable`；整数状态默认 1。变化时逐个 start/stop，忽略单个路由 HRESULT，整体返回 `response.enabled` 原 signed i32；Rust 另外保留真实错误诊断。 |
| `RouteDevice` | `0x19760 → 0xc130 → 0xcde0` | 要求两个字符串，primary name 为 key。先停止删除旧节点；routed 空字符串表示删除；新节点初始化或启动失败仍保留，没有回滚旧路由。 |
| 端点匹配 | `0xd1b0 → 0xd0b0` | 分别枚举 active capture 与 render，均用 friendly name 包含 primary 或 routed 的大小写敏感 OR 判定，取真实枚举顺序中第一个。不能直接假设 primary 固定是 capture，routed 固定是 render。 |
| 绑定 | `0x460e0` | 通过真实 IMMDevice ID 取得两个设备，校验 capture flow 1 与 render flow 0，保留实际 COM 对象。 |
| 启动 | `0x46420 → 0x47640`、`0x46490 → 0x48020` | 初始化客户端、事件、缓冲，再创建音频线程；命令状态 2 表示源线程创建成功，不能当作无错误持续传输的证据。 |
| 停止/释放 | `0x464d0 → 0x48250`、`0x46460 → 0x47e70` | stop event、join、释放 buffer/client/clock/events，状态设 0；保留已绑定设备，支持后续 enable。 |
| 数字转换 | `0x42e0`、`0x11180` | JSON bool、signed/unsigned number 的低 32 位与 double 转 i32；非法类型为 nlohmann type error。IPC 使用明确 i32/u32，未复制越界浮点转换的不确定行为。 |

路由 map 按 UTF-16 字符顺序迭代。通知 `0xdce0 → 0xd830` 先通过变化端点 ID 读取真实 friendly name，找第一个匹配路由，仅把该节点 dirty 置 1，重置 3 秒 trailing timer。不能把同一通知广播成所有路由都变更。

`0xde60` 有一条需要保留的原行为：`if state==2` 停止到 0，`else if state==0` 才初始化，因此正在运行的路由第一次回调只停止，不在同次回调再初始化。之后 `0xdb50` 对状态 0 返回 deviceStatus 0，事件 status 也为 0。源码没有在这个回调清除 dirty 位；后续相关通知再次触发 timer 时才会重新进入初始化分支。Rust 没有把该原生行为改成无依据的立即重连。

## 实际 WASAPI 传输

共享 PCM 算法位于 `crates/razer-device/src/audio_router.rs`，平台适配位于 `crates/razer-service/src/platform/windows/audio_router.rs`。服务的 route owner 与 PCM pump 各自拥有线程及 COM apartment；实际绑定的 IMMDevice 通过标准 COM interthread marshal 交给 pump，避免重新按名字或 ID 猜测设备。没有调用 vendor DLL。

`0x47640` 对两端 Activate IAudioClient，读取各自 mix format/device period，使用默认选择的 render mix format 初始化两端。原指令两次 `mov r8d,880C0000h` 验证 flags `0x880c0000`，share mode 0，buffer duration 200000（100 ns），periodicity 0。使用真实事件与 GetBufferSize，取得 IAudioCaptureClient、IAudioRenderClient 与两端 IAudioClock。时钟保留但此 pump 的传输和 drift 分支不读 clock，不能额外补入未证明的同步算法。

`0x48300` 启动 capture 后 render，申请 `Pro Audio` 优先级，等待 `[stop,captureReady,renderReady]`。capture 循环读取所有 packet、复制实际 PCM、ReleaseBuffer。render 按 bufferSize−padding 申请输出、写入 FIFO 数据并 ReleaseBuffer。停止或 owner drop 时发送真实 stop event，等待 pump 退出，依次停止 render/capture，再释放接口和事件。

默认 FIFO 容量为最大 device period 的 600%，silence reserve 300%，50 ms warmup，drift history 2000 ms，240%/360% 阈值。溢出丢半个 FIFO；欠载补到 reserve。满 history 的均值超阈值才设置 correction；每次最多 5/1000 period frames，减少输入时重复最后一帧、增加输入时跳过多余帧。原代码没有对 PCM 数值做插值重采样。

## 服务接口与失败

- `AudioRoutingEnable { enable:i32 }`：返回源 `response.enabled` 与各路由真实错误 diagnostics。整体源成功不表示所有流启动成功。
- `AudioRouteDevice { primary_device, routed_device, primary_device_id:0 }`：成功回传空源 response；失败返回真实错误，新节点仍保留。
- `AudioRouterEvents`：返回实际回调队列与路由观察。源事件字段为 `event:"RzAudioUtilEvent"`、`eventType:"AudioRouter_StatusChange"`、`deviceId`、`status`、`deviceStatus`。

服务 dispatch、portable runtime 与 IPC 已全部接入。Windows 以外明确报该平台没有 Core Audio adapter；共享 FIFO 不依赖 Windows。route owner 独立线程持续处理通知和 3 秒 timer，阻塞 stdin 的 worker 不会使 timer 停止。

Rust worker 的 `Shutdown` 在向父进程发完成帧前释放 portable/native owner，等待路由 pump、Core Audio 通知与接收器读线程结束；父端收到帧后才进入子进程回收。这个次序用于防止父端结束子进程与 Drop 清理竞争，是本应用进程所有权保障，不据此认定原宿主全插件退出链已等价实现。IPC 超时仍可能强制结束其独占子进程，不能证明超时前的系统操作已撤销。

route owner / PCM pump 的初始化 readiness channel 断开时也会保留线程句柄并 join，等待实际 COM、事件和客户端释放；不能由 channel 关闭推定资源已经释放。该失败清理是 Rust 所有权保障，不改原路由替换和 HRESULT 语义。

原代码的空 PCM 指针、超出 40 字节的 format extension，以及超过 silence reserve 读取未初始化内存的行为无法安全照抄：Rust 明确释放已取得 buffer、记录错误；失败 render buffer 用 SILENT flag 释放，不宣称成功传输了原设备数据。packet HRESULT 在诊断中记录并继续 pump；当前 Rust 没有逐字段复制内部 min/max/count 统计结构。source command state 与 `pump_error` 中实际错误、线程退出状态分开保留。

当前 friendly-name property 分支已保留非负 OpenPropertyStore / GetValue HRESULT 及读取失败后的原空字符串；capture ReleaseBuffer 失败记录后继续取下一 packet。仍需静态补齐内部统计逐字段、其他 HRESULT / 释放异常分支，以及各当前 UI 动作到对应设备类的完整提交链；这些不计为完整验收。内部不可达的自定义 format selector 分支没有冒充 UI 能力。

## 验证边界

已执行格式化、上述静态证据校验与 `cargo check --locked -p razer-service --all-targets`。没有运行应用、测试、DLL 或真实设备传输；运行验收尚未执行。源码取得、逆向语义、Rust 实现、页面连接与运行验收分别记录，45 个函数的证据不代表全程序所有 native 分支已完成。
