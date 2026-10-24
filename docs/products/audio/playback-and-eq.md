# 播放、均衡器与环绕

当前音频产品没有一条通用的“写配置即生效”链路。音量可能属于 OS 端点；EQ 可能属于 THX DSP、固件预设、板载多槽或麦克风参数 EQ；页面还可能把持久化字段作为编辑来源。适用型号与实际注册分支见 [家族矩阵](README.md#产品与适用边界)，具体页面与 props 见 [逐产品原件](products/)。

## 播放与真实音频端点

旧音频 restore 的 `speakerVolume/volume` 分支读取 `getDefaultSpeakerDevice()` 的 `volume/muted` 后发布 `MW_CHANGE_VOLUME_TO_UI`，不把保存的音量直接当作当前系统音量。`micVolume` 也有实读当前 capture 端点的路径；安装 `audioBasicFeature` 或 THX generic 后，旧分支主动跳过，由相应 feature 处理，不能两套同时执行。

例如 1342 当前 MW `5736.fdd55044a41484df28aa.js` 的完整 `il`，bytes `[1460380,1476924)`，依次保存/发布 document，按精确 active GUID 找 profile，再遍历实际拥有的字段。THX v3/Carol 的 `audioEqualizer` 发布选择模式与预设，组装 `{mode,eqData:{name,current},version}` 进入事件队列；THX v2 使用另一结构与 `ON_SET_EQUALIZER`；安装 `audioEqualizer_fw` 或 THX generic 时跳过旧 EQ restore，最后进入 GenericFeatureManager 的 restore。

`thxSpatialAudio` 先发布本地选择，再核对设备是否为当前默认输出；不是默认输出时跳过 `ON_SET_SPATIAL_ENABLE`。这项跳过是源码的路由语义，不能记录为 DSP setter 成功。音频设备名、endpoint ID、container ID 和产品 PID 分属不同身份，按真实枚举结果传递。

## 1443 本机 APO 的效果枚举与处理模式

本节限定于当前保留的 1443 驱动包中的 `THXUSBAPO/THXRTAPO/THXOutAPO-SSE2-v3.dll`。实际输入、版本资源、完整获取链和 PE 身份见 [效果语义收据](../../../.analysis/results/native/cf68efbd4d5ceb735585bbacc9d1f635d7d2e9f312fc387502ea0cc8d40e6470/effects-semantics.json)。路径含 SSE2，而 PE 描述含 AVX2、固定文件版本与资源产品版本也不同；这些字段分别记录，不能由文件名推断指令集、部署选择或其他变体行为。

### 枚举结果、状态设置与内存所有权

MFX 的 `GetEffectsList`（RVA `0x3a770`）和 SFX 的对应方法（`0x5b590`）都按各自实例保存的模式分支。只有 DEFAULT 模式 `c18e2f7e-933d-4965-b7d1-1eef228d2af3` 分配 32 字节，返回 count=2，顺序为 `6f64adcc-8211-11e2-8c70-2c27d7f001fa`、`6f64adc7-8211-11e2-8c70-2c27d7f001fa`。其他模式写 NULL／count=0 并返回 S_OK；这不证明设备断开、能力缺失或效果已关闭。

分配使用 `CoTaskMemAlloc`。分配失败返回 `0x8007000e`，调用方传入的输出指针和 count 保持原值，不能将失败后的旧输出当作本次列表。传入的事件 HANDLE 在这两个函数中没有保存或注册，列表释放与事件所有权仍须沿实际 COM 调用方核实。MFX `GetControllableSystemEffectsList`（`0x3a9c0`）固定返回空列表和 S_OK；紧随的 `SetAudioSystemEffectState`（`0x3a9d0`）完整函数只有两条指令，固定返回 E_NOTIMPL（`0x80004001`）。这项方法不能用作成功的效果开关，也不能据此推断所有 THX 参数写入链均不存在。

### 初始化模式与音频缓冲

MFX `Initialize`（`0x34ad0`）分别从 88 字节输入的 offset 64、80 字节输入的 offset 56 读取模式，保存在实例 base+`0x14c0`。SFX 对应模式字段为 base+`0x90`；效果枚举子对象及实时处理子对象有各自 this 偏移，不能直接套用同一个对象地址。非 DEFAULT 模式保存模式后进入直通分支；DEFAULT 初始化的完整 DSP、EQ 与空间渲染依赖仍需继续逆向。

MFX `APOProcess`（`0x52eb50`）在输入 buffer flag=1 时，先按输出原有 valid-frame count 清零，再按模式处理：DEFAULT 调 `0x52f7f0`，其他模式跳过 DSP。通道数相等时复制 `frames × channels × 4` 字节；不同通道数进入 `0x52b250` 转换链，该链尚未全部闭合。flag=2 分别按输入、输出各自原有 frame count 清零；flag=0 或未知值只走相应日志路径。正常出口仍传播输入的 flags 和 frame count，不能根据最终元数据反推缓冲已经按另一套长度处理。SFX `APOProcess`（`0x530f30`）另有连接数检查和混音分支，不能沿用 MFX 的完整算法。

### 静态核对与后续测试

本专项核验了 19 个完整函数、10,185 条原始指令、53 条汇编断言和 6 份虚表，并保留 10 个有固定期望的静态向量。输入来源逐层核对 manifest、根包、嵌入 CAB、packageZip 和 driver.zip 的实际字节。唯一 Hex-Rays 失败 `0x4c7140` 仍保留失败状态；其 163 字节、49 条完整指令及调用点 `0x4c7173` 已核实为 UTF-16 surrogate 到 UTF-8 的辅助链。静态默认 mbstate 位于 PE 零填充区，不代表观察过其运行值。

| 必测分支 | 独立预期与实际对照要求 |
| --- | --- |
| DEFAULT／其他模式的两种效果枚举 | 分别核对 GUID 顺序、count、HRESULT；非 DEFAULT 的空列表与错误状态分离 |
| 分配失败与列表释放 | 用不同的初始输出哨兵核对失败不改输出；核实真正调用方的 `CoTaskMemFree`、事件和 COM 生命周期 |
| 可控效果与状态 setter | 空列表 S_OK 与 E_NOTIMPL 分别比较，后者不得呈现写入成功 |
| 两种初始化结构与对象偏移 | 核对结构大小、模式 offset、MFX／SFX this 调整、失败和重复初始化 |
| buffer flag、帧数与通道分支 | 输入／输出使用不同旧帧数，覆盖 0／1／2／未知值、相等／不同通道和边界长度，分别比较缓冲与出口元数据 |
| UTF-16／UTF-8 辅助链 | 覆盖 surrogate、非法序列、容量及 mbstate；纯向量不替代实际 DLL 的相同输入结果 |

当前 [THX V4 能力表](../../../crates/razer-device/src/audio/thx_v4.rs)只准入 1398／1401／2638／2641／4124／4126；[Shell THX 所有者](../../../crates/razer-shell/src/shell/audio/thx_sound.rs)在能力缺失时不建立对应会话，1443 不进入这条生产链。现有 JSON／protobuf 转换、本地 RPC 和 HID／EQ 逻辑不能作为本节 APO 效果枚举、模式和 DSP 的实现证明。V4 的本地绑定、GetVersion readiness 与实际 setter 应答也分别验收，不能合称原版状态确认。

以上向量是静态证据核对，尚未作为 Rust 行为测试或官方实际 DLL 差分测试执行。完整 DSP、COM 创建／服务、取消与释放、其他二进制变体、Rust 实现、UI／后台接线和运行验收分别保持未验证；完成捕获不提升这些状态。

## APO4 本机流属性 RPC 与产品分支

本节的客户端比较限定于两个已保留资源分支：2641 的 `THXAPO2641-0A51` 4.1.28.1056、1415 的 `THXAPO1415-0587` 4.1.28.1060。原件、版本和五层容器来源见 [流属性 RPC 收据](../../../.analysis/results/native/81c7c20b867a45e83eaa6ea00163159dfaf353f8577193632d67d1708d685bde/stream-properties-rpc.json)。每份核对 68 个完整函数、5,823 条已枚举指令及 63 条独立汇编断言；68 个函数的完整 chunks 和 10 个数据区域在两份原件中逐字节相等。每模块的 22,841 字节原始函数块全部保留，其中 95 字节没有对应的分析器指令项，不能写作逐字节完整反汇编。此比较只适用于所列函数和数据区。

### 客户端身份与本地服务地址

这两份 `grpc.dll` 只有 `dllCreateStreamPropertyClient`（RVA `0x124f30`）和 `dllDestroyStreamPropertyClient`（`0x125090`）两个 PE 导出，实现的是客户端。创建函数接收两个借用的 NUL 结尾窄字节串；扫描前没有 NULL 检查。非空销毁通过虚表首槽的 deleting destructor 释放对象，NULL 可以直接返回，但不能把反编译器给出的整数类型当作已确认的有效返回值或完整 C ABI。

构造器 `0x11b510` 获取 ProgramData，再按原路径 join 规则组合 `thx`、首参数和 `thx_stream_properties.ipc`，形成 `unix:` 本地地址。首参数包含绝对路径或盘符时有单独规则；转换仅在当前代码页为 65001 时使用 UTF-8，否则采用 CP_ACP。KnownFolder 失败会记录 HRESULT，随后以空基值继续组合。非空工厂／通道指针和 channel ready 均不能证明真实设备在线。

### 完整 wire schema 与读写差异

从 PE 恢复两份完整 FileDescriptorProto：`thx_audio_properties.proto` 为 5,292 字节、35 个消息、17 个方法；`thx_stream_audio_properties.proto` 为 1,547 字节、11 个消息、7 个方法。流服务包含 CreateStream、DeleteStream、GetStreamProperty、GetStreamProperties、SetStreamProperty、SubscribeToStreamPropertyChanges 和 ApplyStreamPreset。CreateStream 的 stream_id 为字段 3，Delete／Get／Set 使用字段 2，不能共用一套未经分支校验的参数编码。

| PropertyType | wire 值 | 当前客户端写入 | 当前客户端读取 |
| --- | --- | --- | --- |
| Bool=1 | 字段 5，bool | 要求 variant 索引 2，保留显式 false | 从对应成员转换 |
| Int=2 | 字段 4，int64 | 要求 variant 索引 0，负值使用 int64 varint | 保留 64 位值 |
| Float=3 | 字段 3，float32 | 要求 variant 索引 1，原指令为 movss | float32，不能改作 float64 |
| String=4 | 字段 6，string | 要求 variant 索引 3，保留显式空串 | 从对应成员转换 |
| StringArray=5 | 字段 7，repeated string | 要求 variant 索引 4，保留顺序及空元素 | 返回数组 |
| FloatArray=6 | 字段 8，repeated float32 | 仅写 id/type，没有填充数组值，仍发 RPC | 支持数组并放入 variant 索引 5 |
| None／未知类型 | 原始 type | 仅写 id/type，仍发 RPC | 保留 raw type，初始 int64 零 variant 不代表实测值 |

四个标量值字段具有 proto3 optional presence。读取适配器 `0x11af90` 只按 type 取成员，没有检查 has-bit，因此缺字段与显式零值可产生相同的原版转换结果；本项目仍须区分未观测和真实返回值。类型 1–5 与 variant 不匹配会在 RPC 前抛出 `bad_variant_access`。FloatArray 读写差异已由第二产品原件和完整分支复核；自行补齐数组不能称为与当前包装器逐输入等价，服务端是否实际支持该写法须另查。

### 结果消费者、事件与释放

Get 的实际 Property 先进入转换器，再调用成功消费者，最后发布成功判别位；失败不调用消费者，也不回退本地配置。空回调实际抛出 `std::bad_function_call`，其 throw 类型已从原 PE 另行核对，不能采用 IDA 错标的 `Concurrency::cancel_current_task` 名称解释成正常取消。批量 Get 按属性 ID 匹配请求回调，响应中缺失的 ID 不产生默认结果。

Set 返回 `google.protobuf.Empty`，包装器仅消费 gRPC 状态，没有独立 getter、缓存更新或 DSP／设备读回。同步 unary 所有者在 status=0 却没有响应消息时改为 status=12，原文为 `No message returned for unary request`。包装器将非零状态折叠为 error=1／false，只赋值结果前 5 字节；成功清零 12 字节并置 +4 为 true。错误时其余非活动成员或填充字节不属于已赋值业务字段，须在保留原始样本的前提下按真实结果布局比较。

变更订阅只收到 stream_id／property_id，没有新属性值，不能直接发布设置已应用。最后一个订阅注销先移除回调、取消 ClientContext，解锁后 join 读线程，再销毁上下文；join 使用无限等待，完成后关闭线程句柄。连接观察另有 map／线程，只观察 gRPC channel state 是否为字面值 2，并在变化时通知。销毁继续释放 Stub、等待通道变化、释放共享 channel、停止并 join 连接观察线程、清理 map／端点／上下文后释放对象；外部并发使用、完整 unwind 和所有间接回调仍需核对。

### 四产品属性库与调音预设

同一收据另保留 1415、1401、2641、2638 的四份 `_props.dat`，逐项核对当前 manifest 与完整提取链。1415 包有 240 个 ZIP entries／235 个文件，其余各 241 个 entries／236 个文件，均有五个目录。共 963 个成员的顺序、路径、原始压缩范围、CRC 和解压 SHA 已复核；939 个 JSON 完整解析并检查重复键。ZIP 无前缀、尾随数据、加密或重名成员，包内容完整不代表服务已加载或设备已支持相应参数。

四包的 `thx_endpoints.json`、`VERSION=20250924`、404 条主属性定义和 26 条麦克风定义逐字节相同。主定义含 89 bool、238 float、67 string、7 int、2 float[] 和 1 string[]；FloatArray 分别为 `headtracker_ui_bounds`、`headtracker_ui_preview_camera_bounds`，StringArray 为 `headtracker_available_camera_names`。`headtracking_enabled` 单独标记 volatile。麦克风的 `mic_sidetone_level` 与 `mic_high_pass_filter_cutoff_freq` 没有 default，仅凭包声明不能补零，2641 的服务解析分支另见下文；定义存在也不能用于推断当前值、每个 endpoint 的能力或首次发现时必须写默认值。

| 产品 | 与 2641 的解压内容比较 | 必须保留的分支 |
| --- | --- | --- |
| 1401 | 全部 241 个成员字节相等，外层 DAT 哈希不同 | 实际 manifest 身份为 1401，不能因文件名或邻近编号归入 2638 |
| 1415 | 240 个共有成员全部相等 | 没有额外的 BlackShark V3 Harman 调音文件 |
| 2638 | 240 个共有成员全部相等 | 额外调音文件为 Pro 版本；与 V3 的 50 个属性身份相同，其中 25 个值不同 |

例如 Pro／V3 的 band0 gain 为 7／15.8，band1 gain 为 11／-1.8。全部 25 项差异保留实际 ID、type 和原始 value，不能因为主定义相同就跨产品复制调音参数。四包的 `preset_groups.json` 均将 device_presets 的 default_preset 声明为 `default`，对应精确 group/id 文件存在；额外 Harman 文件的存在不证明服务启动会选它。实际加载、选取、恢复、持久化、DSP 消费者和读回仍须沿代码与实际对照分别确认。

### 2641 实际服务注册与读写结果

同一收据的 `actual_service` 保留 2641 服务的基础切片：88 个完整函数、26 个数据区、18,329 条指令和 47 条独立汇编断言。服务全捕获为 36,208 个函数，7 个 Hex-Rays 失败原样保留；所选函数无该失败。79,211 字节完整 chunks 中有 1,379 字节未枚举为指令，228 个范围单列。固定版本和具名 FileVersion／ProductVersion 块已独立核对，通用版本解析器对非标准 LegalCopyright 块的失败仍保留。原件、哈希、容器链和具体函数全部见收据，不把所选闭环泛化到所有产品服务。

真实启动链为 `0x14df20 → 0x16aee0 → 0x16ac60` 构造服务，随后 `0x18cc60 → 0x18b880` 将实际主对象和偏移 40 字节的流服务对象注册，再启动 gRPC server。流虚表 `0x9c9830` 的 slot5 确实指向 `0x7db4b0`：SetStreamProperty 固定返回状态 12（UNIMPLEMENTED），message／details 为空，不读取 request 或写存储。这一结论有实际构造、注册及 generated callable 分派依据，不能只凭 generated base 默认表推断，也不能扩展成所有设置均不支持。

GetStreamProperty 经 `0x182410` 区分三条路径：空 stream_id 调全局 getter；非空但不存在的 stream 返回 domain8；已有 stream 的内存 override 查询失败才调全局 getter。全局 `0x181b30` 仅在保存值类型匹配当前 definition 时返回 JSON 存储值，否则 `0x180b80` 取 definition default；无 definition 为 domain3，definition type0 为 domain4。这些是服务状态及配置默认值，不能作为设备实测，不能将官方服务的默认分支变成设备读取失败回退。

全局 SetProperty 经 `0x12d450 → 0x1920d0`。同 id 的缓存 variant 与新值相等时，在后续 definition／type 校验之前就返回本地成功，不触发存储和通知。变更分支才检查 definition／type，更新缓存、调用存储 setter、调用属性 observers、向现有 stream 内存 storage 复制值，再经 `0x188850` 通知流消费者。存储结果容器没有在这个所有者内检查。通知消费者可能产生外部副作用，不能写成“只改内存”或“没有 DSP／硬件写入”；完整下游仍须另证。

JsonPropertyStorage 的真实虚表只有 3 槽。`0x192610` 更新 map 后排队精确 1000 ms 的延后保存，立即返回服务成功；worker 经 `0x1742f0 → 0x18d9a0` 打开 properties.json、序列化、写入并关闭。打开失败只记录日志，不会改变已返回的 RPC。所选保存函数未见临时文件／rename 原子发布，析构会清 delay、停止并等待任务，没有证明无条件最后一次 flush。RPC 成功、内存状态、落盘和设备应答必须分别判断。

服务与同包客户端的两份描述符逐字节相等，但 request→domain 支持 type1–6，domain→response 与批量 Get 只填 type1–5；type6 响应仅有 id/type，没有 float_array_value。domain3／4 映射 gRPC3，其余包括 domain8 映射 gRPC13。CreateStream 拒绝重复 id，DeleteStream 对不存在的 id 仍成功；ApplyStreamPreset 逐属性修改内存并通知，首个失败返回，未证实回滚。

定义工厂传入主文件名，实际 parser 枚举父目录中名称包含该 stem 的 regular `.json`，所以本包主 404 项与 mic 26 项均为候选。缺失 is_volatile 为 false；`0x178cc0` 只对 volatile 项经真实 setter 写 default，本包唯一为 `headtracking_enabled=false`，随后应用非空的当前预设。两个缺 default 的 mic float 在已核 traits parser 中采用 0.0，属于服务解析规则，包声明仍是缺字段，也不意味着启动写零或硬件值为零。231 个 preset、26,166 条属性、103 个文件名／内部 id 差异、108 个显式 null 选择及 7 个包内未解选择引用原样保留；不能补造默认选择。

流通知只带 stream_id／property_id。ServerContext 取消或停止标志使订阅退出并清除队列／映射；server 析构注销 property／preset／stream tokens、唤醒、shutdown 并释放对象。完整框架竞态、writer 失败、异常展开、预设选择冲突及所有恢复链仍未闭合。

### 四款实际服务的比较范围

同一收据的 `actual_service_consumers` 将所选服务链和麦克风预览消费者扩至下表。四份各自核实官方 manifest、完整资源及逐层容器字节；每份完整捕获均为 36,208 个函数、7 个 Hex-Rays 失败，所选切片各保留 131 个完整函数、38 个数据区及 81 条映射指令断言。四份合计核对 89,518 条已枚举指令，原始函数块内未枚举的字节另列；主收据合并去重后复核 2,246 个输入。

| PID | 实际服务 | 版本 |
| --- | --- | --- |
| 2641 | `thx_razer_blacksharkv3_xbox_svc.exe` | 4.1.28.1056 |
| 1415 | `thx_razer_krakenkittyv3pro_svc.exe` | 4.1.28.1060 |
| 1401 | `thx_razer_blacksharkv3_pc_svc.exe` | 4.1.28.1054 |
| 2638 | `thx_razer_blacksharkv3pro_xbox_svc.exe` | 4.1.28.1059 |

四份真实流服务的 slot5 都指向固定 gRPC12 的 SetStreamProperty handler；上述 Get、全局 Set 的相等 variant 提前返回、通知和 1000 ms 保存分支也逐服务核实。函数地址不能直接照抄：1415／2638 多处相对基线偏移 −0x20，1401 多处偏移 −0x30，2638 的部分数据表另偏移 +0x10。基础 88 函数的指令偏移和长度匹配，但三份兄弟分别有 923／940／932 条指令字节差异，逐项保留 rel32 目标及数据引用依据；指定表槽、switch、描述符和数据区另核原字节。其余数据引用保留原址和 xref，不代表被引用数据已全部语义闭合。新增的产品 identity 返回构造函数确有指令形状差异，不能宣称 131 个函数字节相等、整个服务等价或官方执行结果一致。

### 麦克风预览的真实 Windows 消费者

以下 RVA 以 2641 为基线，其余三份由收据中的逐函数和数据映射定位。实际入口 `0x1310e0 → 0x1416a0 → 0x14df20` 取得产品配置及 microphone property service；六槽配置表的 slot4 返回 true，构造 `0x141fc0` 并运行 `0x14f300`。该布尔属于当前产品配置，不是硬件能力读回。运行时先注册 endpoint monitor、枚举 capture／render 配对，再注册 mic property callback 并将 token 保存在 owner+144，最后读取当前 `mic_preview_enabled`；仅读取成功且 variant 为 bool 时应用。endpoint 注册失败可以返回 false，但此调用方没有检查，不能把后续初始化继续当作监控注册成功。

全局属性 observer 进入 `0x1466c0`，精确匹配 19 字节 `mic_preview_enabled`。variant 的 +72 字节必须为 discriminator 2，布尔值取自 +40；其他类型若到达该回调，进入 `0x117050` 的 `bad_variant_access` 路径，不作 truthy 转换。definition／type 校验可能在通知前拒绝无效请求。此 observer 为 void，不向已完成的 RPC 回传单次 OS 写入 HRESULT。

配对由 `0x14a6f0` 枚举 capture=1、render=0，双方 stateMask=15。产品过滤读取 PROPERTYKEY `{4a82dc8f-2acc-466f-b11a-3fbf40a339a0}, pid0`，要求 VT_LPWSTR=31 和非 NULL 指针，将 UTF-16 字符低字节收窄后与当前服务产品 identity 精确比较。随后沿 topology／connector／part 取得两端身份串，相等才保留真实 endpoint IDs。topology 失败可能返回空串，两边空串仍可能比较相等；不能擅自增加非空规则，也不能把静态配对条件写成硬件匹配已经确认。

OS writer `0x151730` 为每一对执行 CoInitializeEx，再创建 CLSID `{870af99c-171d-4f9e-af0d-e63df40c2bc9}`、IID `{f8679f50-850a-41cf-9c72-430f290290c8}`。实际调用是虚表 +96／slot12 的五参数 `(this, captureEndpointId, 0, PROPERTYKEY*, PROPVARIANT*)`，必须保留第三参数的零。写入顺序如下：

1. 写 fmtid `{24dbb0fc-9311-4b3d-9cf0-18ff155639d4}, pid1`，VT_BOOL=11，false 编码为 0、true 为 0xFFFF。
2. 仅第一 HRESULT 非负且下一 variant 构造成功，才写同 fmtid 的 pid0；true 使用 VT_LPWSTR=31 的配对 render ID，false 使用 VT_EMPTY=0 清除。
3. 正常返回路径清理 PROPVARIANT、Release COM 并 CoUninitialize。第一写失败或字符串分配失败跳过第二写；第二写失败记录日志，没有第一写的补偿回滚或独立读回。

CoInitialize／CoCreate 失败进入 WIL 非返回失败链，不能概括为仅记录日志并正常清理，也不能在未展开全局错误处理器时断言固定 C++ 抛出形式。所选函数没有 OS 属性独立读回或设备读回；服务 Set 成功不能作为预览已被系统或设备确认的依据。

新增端点事件在锁内重新枚举配对，此 owner 未重新应用当前预览 bool；移除事件只删对应配对。释放链已追到 owner、monitor、枚举器 Release 及完整析构；所选析构未见用 property token+144 注销、恢复 preview=false 或调用枚举器 slot7 取消通知。并发回调、COM 注册引用与整体 shutdown 仍需继续核实，不能把可见 Release 当成恢复完成，也不能由局部范围断言必然存在悬空调用。

### 必测范围与当前实现边界

| 必测链路 | 独立期望与对照要求 |
| --- | --- |
| 属性编码 | 本收据的 12 组固定期望覆盖 false／零、负 int64、float32 负零与 NaN 位型、空串／数组、FloatArray 缺值及未知类型；Rust 编码器不能生成自己的期望 |
| Get 与批量 Get | 缺字段、optional presence、未知类型、漏 ID、空回调与消费者顺序分别核对；不得把 protobuf 初值当设备实测 |
| Set 与失败状态 | 分别比较实际请求、原始状态、包装结果和独立读回；Empty 或 channel ready 不算写回确认 |
| 属性事件与连接观察 | 事件身份和实际值分离，覆盖最后订阅取消、迟到结果、连接切换及清理失败 |
| 路径、编码与对象释放 | 覆盖 ProgramData 失败、盘符／绝对路径、当前代码页、NULL 边界、真实取消及 join 后释放 |
| 产品与服务 | 分别核实调用方参数、endpoint 来源、服务 handler、持久化、DSP／驱动消费者和预设差异；同字节客户端不能替代服务验证 |
| 属性库与预设 | 覆盖两项缺省字段、volatile、精确 group/id、缺失或不同的附加调音文件，以及服务实际选择；404 条定义不等于每产品全能力 |
| 服务早返回、保存与取消 | 同 variant 在校验前成功、未知 stream 错误、type6 响应缺数组、1000 ms 保存失败、取消与释放分别核对；不能用相同成功状态掩盖副作用差异 |
| 麦克风预览与 OS 副作用 | 四产品分别覆盖精确属性 ID／variant、初读失败、端点配对和热插拔、五参数 ABI、两次 HRESULT、分配失败、无回滚／读回及注销／恢复边界；实际写入前须证明能够完整恢复原值 |

四款服务的所选注册、Get／Set、属性库、通知、保存及麦克风预览 OS 消费者已有静态依据；其他产品服务、其他属性及完整 DSP／驱动消费者继续分别核对。现有 [THX V4 转换与能力所有者](../../../crates/razer-device/src/audio/thx_v4.rs)和 [Shell 生命周期](../../../crates/razer-shell/src/shell/audio/thx_sound.rs)属于另一条已核边界；函数名或 protobuf 类型相近不能证明本独立流客户端及服务已由 Rust 替代。

本次通过的是原件字节、完整描述符、表槽和固定向量的静态复核。未执行产品 Rust 行为测试、官方 DLL、服务、真实设备读写或 UI 验收；服务端实际写回、其他产品／固件／平台分支及恢复清理继续单独保持未验证。

## 固件预设和板载索引

| 产品分支 | 源码索引/作用域 | 必须保留的差异 |
| --- | --- | --- |
| 1365 | default 0；game/music/movie 7/8/9；esports 250–254；custom 255 | `useEqualizerOBM` 仅 250–255，不包括标准三项 |
| 1383 | 0/7/8/9/255 | OBM 仅 255 |
| 1387 | 0/7/8/9/255 | OBM 为 7/8/9/255 |
| 1396 | 0/7/8/9/255；固件支持，profile global | 不与旧 THX v3 的文件 EQ 混用 |
| 1404、2644、4130 | 0/1/2/3；固件支持，profile global | `skipUpdateaudioEqualizerToLocalStorage`；2644/4130 另有语音提示 feature |
| 1453 | 0/1/2/3/16；非 global | 当前显式 `audioEqualizer_fw` 的 map 包含 custom 16，设备类型允许 `rzDeviceJolie` |
| 1462 | 1/2/3/31；非 global | `avoidReadingEQFromFW:true`；`onlyCustomHasEQBands:true` |
| 1427 | THX v4 与固件同步；0/1/2/3 | band 的负值使用 `128+abs(value)`，读回以 `128-value` 解码；不是二补码 |
| 1465 | `audioEqualizer_fw` | `useEQBandSettingWithPreset:true`、`onlyCustomEQHasBands:true`；另有 profile 2 的 save-effect |
| 2657、2660、2664、2668、2689 | 协议 4.0 耳塞，固件支持，非 global | 操作按耳塞的实际 transport 与 feature，不能调用 2.5 的同名方法 |

显式 `AudioEqualizer_FW` 的完整模块 57283 可以在各 PID 的 `7283.*.js` 核对。1453 的 `7283.d1f11cf349c6803a4c9d.js` 全文件 4998 bytes；2657 的 `7283.d739a1e04541416d4900.js` 全文件 4927 bytes。`setupRenderEQData/getRuntimeData` 使用本地 profile 数据，并不执行固件 EQ 读取。setter 把 `eqData.current` 转为有序 `{index,level}`，再按预设 map 调用协议 helper。

这个外层 `setAudioEqualizer` 等待内部调用，却不消费其布尔返回；内部也可能在类型不匹配或异常时返回 `false`，外层仍返回 `true`。因此需要分别理解“源码外层结果”“实际调用是否发生”“原始协议应答”。restore 随后的 UI 更新从 runtime cache 取得，也不是独立读回。是否实际使用这份 lazy module，由各产品 feature 装配决定，不能把包中存在的类视为全部型号的活动链。

## 本地多槽 EQ 与实际设备数据

1398、1401、2638、2641、4124、4126 的 `cloudBasedEqualizer` 名称沿用源码；其设备 EQ 与本地预设读写属于本机能力，在线目录/账户部分不在产品范围。以下完整所有者来自 1398 MW `main.11fdb3ee2d36c7e8efa4.js` bytes `[950699,959614)`。

读槽先 `getSpeakerEQOrderInfo(index)`，正常槽再 `getSpeakerEQBandByPosition(index)`。顺序信息包含 index、语音提示索引、enabled、active、预设 ID 和 custom；band 另行读取，不能靠 index 推断开关。`operationMutex` 控制初始化，`deviceAccessMutex` 串行设备访问。断开或 reset 清除 `initDone/dataRestoreDone`；PostRestore 在配置要求时重新读取；硬件 EQ 变化通知仅在初始化后进入本地列表、活动标志和 UI。

原版对 `0xff` 无效槽保留旧 band；读取异常时还返回默认 EQ，`checkActiveEq` 会从旧活动槽、enabled 槽或首槽补选活动项。这里是原版数据修补的实际边界，不能将其转为本项目“读取失败后的设备实测状态”。真实观测、文件默认值与草稿必须分开。

Stereo 写入为 batch start → 每槽 band → 每槽 order metadata → batch stop；每项检查 `status=="SUCCESS"`。Spatial 路径只写选中槽 order metadata。改变活动项也使用 batch start → 每槽 order → batch stop。原版 catch 后释放 mutex，但失败分支没有 finally 补发 batch stop；不能声称存在自动回滚或设备恢复。footsteps 的 band 配置同时有固定表和本地预设存储读取，不能依赖在线获取才能执行本地 DSP 写入。

## 参数均衡器与效果顺序

1422/1446 的参数 EQ 属于麦克风设备，不是上表十段播放 EQ。当前完整 feature 为模块 65561，1422 `5561.bee7bca633d9da9fd151.js` bytes `[549,9915)`；profile 映射为模块 23374，`3828.c5e55b0ac9b8ddaac249.js` bytes `[549,1098)`。

| 对象 | UI/领域含义 | 固件编码或顺序 |
| --- | --- | --- |
| preset（MW/API 映射） | podcast、arena、fpscallout、studio、instrument、custom | preset ID 6/7/8/9/10/16；对应 profile ID 1/2/3/4/5/6 |
| frequency | 20–20000 Hz | u16 大端；源码位运算会截断小数 |
| gain | -12.0 至 +12.0 dB，0.1 步长 | 乘 10 后 signed u8 二补码，与 1427 的编码不同 |
| Q | 0.3–15.0，0.1 步长 | 乘 10 存入字节 |
| band | 稳定 band ID、status、filter type | profile、direction、band、status、freq、gain、Q、filter 的完整字段 |
| HPF | enabled 与档位 0–5 | profile 1；禁用写 0 |
| vocal bass/exciter | profile 的独立效果 | 保留 band 8/9，300 Hz low shelf / 6000 Hz high shelf；amount-50 的有符号十分位；Q byte 7 |

单 band 修改先真实 `getEQPreset(0)`，核对成功且有 `jsonData`，将设备 preset ID 映射到实际 profile，再 `setParametricEQSetting(profile,direction=0,...)`。完整切预设是 profile 1 清十个 status → 选择 preset → 顺序写全部 bands → 本地 profile 的 vocal bass → vocal exciter → HPF。初始 `initDataFromDevice` 只补/发布本地 EQ 结构，不读硬件；名称不能替代函数体。

源码检查单 band 的 status，但完整预设流程没有传递清 status、选 preset、band 循环及 HPF 的全部返回；vocal bass/exciter 返回失败会阻断后续阶段。HPF setter 的 `true` 也只表示 await 未抛出，getter 在失败时保留内部初值。`getEQActiveBands` 使用 status 值作为 getter 的 band 实参，而不是循环下标；不能把此方法改写成自行推测的正确读取顺序或用它证明当前页面已读到完整 EQ。

## Seiren 实际 EQ 页面与当前 Rust 接线

本节只覆盖 1422/1446 的实际 mounted EQ 页面。两款均由 UI 模块 6007 挂载，但必须分别读取自己的主入口、产品配置、middleware 与资源，不能把同名组件当作型号兼容性证据。

| PID | 实际 UI 原件 | 完整 EQ factory（UTF-8 bytes） | 产品配置中的页面预设 |
| --- | --- | --- | --- |
| 1422 | [684.f6b88b50.chunk.js](../../../.analysis/inputs/frontend/audio-1422-current/raw/apps.razer.com/synapse/products/1422/ui/static/js/684.f6b88b50.chunk.js) | `[55022,94170)` | [main.61ab07b9.js](../../../.analysis/inputs/frontend/audio-1422-current/raw/apps.razer.com/synapse/products/1422/ui/static/js/main.61ab07b9.js) 模块 8193，五项数组 `[209344,209396)` |
| 1446 | [684.9501aa81.chunk.js](../../../.analysis/inputs/frontend/audio-1446-current/raw/apps.razer.com/synapse/products/1446/ui/static/js/684.9501aa81.chunk.js) | `[67423,106618)` | [main.9f98c3ff.js](../../../.analysis/inputs/frontend/audio-1446-current/raw/apps.razer.com/synapse/products/1446/ui/static/js/main.9f98c3ff.js) 模块 8193，五项数组 `[211862,211914)` |

当前页面的五个预设是 `podcast/arena/fpscallout/studio/custom`；`instrument` 只出现在上述 MW/API 映射，不能凭协议映射给页面增加第六个按钮。[Rust EQ model](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq_model.rs) 的 `PRESETS` 与这两个实际数组一致。各预设的选中 band、band 列表和 HPF 分别保存，不能把“切预设”简化为改变标题或共用第一组 band。

已逐段核对的页面行为及当前所有者如下。这些是静态语义与接线结论，不代表设备效果已经实测一致。

| 行为 | 当前官方 mounted owner | 当前 Rust 所有者与核对结果 |
| --- | --- | --- |
| 麦克风、耳机、sidetone 三组控制 | 1422 monitoring `[58171,62903)`；1446 `[70597,75329)` | [EQ 页面](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq.rs) 为三组值各保留 300 ms 提交任务；静音立即提交，不取消先前排队的音量。AGC 禁用 mic fader/数值编辑；Stream Mixer 禁用 sidetone，并有随鼠标定位的提示；[graph 控件](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq_graph.rs) 持有相应禁用和 meter 呈现。 |
| 曲线、band 与参数 | 1422 graph `[68916,78106)`、band `[78107,79170)`、properties `[80759,83577)`；1446 graph `[81342,90532)` | graph/math/model 分别负责 20–20000 Hz 对数坐标、滤波器响应与稳定 band ID。当前 model 支持五种滤波器、最多八个 band、至少保留一个；pass 类型的 gain 为 0 且禁用编辑，shelf 的 Q 显示 1 且禁用编辑。删除所选 band 时按原数组相邻项选中，不能用排序后的显示下标充当协议 ID。 |
| de-esser、vocal bass、exciter 与 HPF | 1422 effects `[91280,93856)`；1446 `[103709,106282)` | 三个效果共享 20 ms throttle 与 200 ms 最后一次提交任务；HPF 独立立即提交。曲线计算包含实际启用的 bass/exciter/HPF；编辑曲线和数值时的草稿不能冒充设备读回。 |
| 预设、reset、flat、filter tutorial | 两款模块 6007 的完整 wrapper、graph 与 properties | [EQ 页面](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq.rs) 有独立预设、reset、flat 与五类滤波器帮助入口；帮助的选中类型、展开、关闭和焦点属于页面所有者。reset 比较源码默认的有效 band 与 HPF；flat 是另一动作，不能互相代替。 |

页面的第一行是 microphone monitoring 与 visualizer，第二行是 band、properties、effects。CSS 必须按实际 lazy import 和 webpack link 插入顺序计算：1422 为 `main.2715ebd0.css → 38.0df290bd.chunk.css → 684.51447135.chunk.css`，1446 为 `main.2715ebd0.css → 287.cae4ef23.chunk.css → 684.8af27a50.chunk.css`。visualizer 宽 1000 px、graph 840×242 px；三个下排面板宽 200/400/580 px，间距 20 px；预设按钮至少 150×36 px。Rust graph 常量和页面两行布局已能对应这些尺寸，仍需逐状态视觉验收，不能以常量相同宣称整体 1:1。filter tutorial 的 800×420 px 最小尺寸、五组图文、300 px tooltip 和下拉展开/失焦关闭也属于实际页面范围。

真实动作已能沿当前文件追到独立实现：

| 阶段 | 当前生产所有者 | 必须分别保留的结果 |
| --- | --- | --- |
| 页面排队 | [audio_seiren_eq_write.rs](../../../crates/razer-pages/src/features/audio/seiren/audio_seiren_eq_write.rs) | `SeirenEqRequested` 持有 entity/epoch/sequence/guard、设置快照与 `Change`。同类未执行项可合并；预设和 band 的有效先后顺序保留；取消后等待工作结束才释放在途槽。 |
| 事件与持久化 | [source_workspace.rs](../../../crates/razer-pages/src/features/workspace/source_workspace.rs)、[Shell EQ](../../../crates/razer-shell/src/shell/audio/seiren_eq.rs) | 订阅真实 `SeirenEqRequested`，按 NoSave 与需要保存的字段分流；需要保存时成功后才提交设备，并在保存前核对、合并当前 profile/device/global 字段。单独的 `AudioProductChanged` 只捕获设置快照，不能代替设备请求。 |
| 保留会话与传输 | [seiren_audio_operations.rs](../../../crates/razer-shell/src/shell/audio/seiren_audio_operations.rs)、[audio_notifications_worker.rs](../../../crates/razer-shell/src/shell/audio/audio_notifications_worker.rs)、[服务 EQ](../../../crates/razer-service/src/runtime/windows/audio/seiren_eq.rs) | 使用当前唯一物理路由、保留会话与顺序队列；切连接后使旧 guard 失效。采样率动作先 ready=false、停止 routing，执行真实请求，再按源码恢复 routing 并发布实际结果。服务核对 target/container/instance，保留超时与关闭清理。 |
| 独立设备与系统音频逻辑 | [设备 EQ](../../../crates/razer-device/src/audio/seiren_eq.rs)、[会话 owner](../../../crates/razer-service/src/runtime/windows/audio/seiren_session_owner.rs)、[驱动](../../../crates/razer-service/src/runtime/windows/audio/seiren_driver.rs)、[端点](../../../crates/razer-service/src/runtime/windows/audio/seiren_endpoint.rs) | HID 协议由 Rust 编码/执行；另一分支由 Rust 使用精确设备接口、`DeviceIoControl`、系统音频端点及回调完成。`NativeOperation` 是本项目的操作类型，并不表示调用官方业务 DLL；当前会话不加载厂商库，也不伪报厂商 DLL 版本。 |

`source_result` 是按官方外层消费者保留的结果；实际传输阶段、应答、skip、错误和 `confirmed()` 必须另行检查。设备 disconnected/failed/unobserved 与持久化默认值、当前草稿分别处理，不能用初始化的 profile 或页面已显示曲线推断 EQ 实测成功。当前初始化遮罩来自实际 ready 条件，也不是 DLL 初始化或 DSP 效果确认。

### 验证与索引边界

本轮只进行了静态核验：引用的官方原件、CSS、向导资源和获取收据共 80 份身份匹配，保留的 92 个完整 module body、128 个完整 function owner 与原件字节及 body SHA-256 一致，36 份完整 JS 静态解析无错误。其中也包括 [麦克风文档](microphone-and-preview.md#1446-mic-页面与映射) 使用的两个 MW feature；这些数量不表示整个输入闭包、整个音频族或全部设备分支完成。两款 UI 获取收据仍分别有 35/47 项未决闭包，完整设备与主机源码的版本审计仍需继续。

`audio-MapAudio-audit.json` 的原件身份和完整 owner 可定位上述证据；其中 `rust_implementation/connected_local_chains` 仍含已不存在的 `seiren_eq_native.rs` 和旧 ABI adapter 描述，`rust_ui_implementation` 也只是先前文件快照。这些旧 Rust 身份不能作当前接线或完成依据，必须以本节实际文件重新生成后再核验。本文未把旧报告中的“已接线”标签或 query 无失败计数当作验收结论。

每个 PID 都仍需将官方实际 DLL 的对应功能结果与本地 Rust 对照，覆盖五个页面预设及协议有效分支、band/filter/status、bass/exciter/HPF、端点音量/静音、sampling/mode、错误、取消、恢复和释放；DLL 外层返回与真实应答应分栏保留。可定位的纯测试与模拟传输不等于已取得这些实测结果，本轮没有执行官方 DLL 或设备对照。MIC 底栏挂载、向导层级和独立映射链的已知缺口见 [麦克风文档](microphone-and-preview.md#当前-rust-已接线部分与缺口)。

## 前后音箱环绕与时序校准

1392、1442、3942 当前挂载 surround；1392 的完整 UI/MW/native-wrapper 所有者保留于 `audio-surround-audit.json`，另外两款分别保留自身输入与绑定，不能只按 Carol 同名适配器合并。

页面先枚举真实可用 front endpoints。开启 Surround 时，若没有已选 front endpoint，挂载选择弹层；Cancel 关闭并撤回本次启用选择，Confirm 采用实际条目的 ID/name。设备列表空时禁用切换并显示源码警告。前后音量、各自三段 EQ、replicate、delay 与试听是独立动作。

Time Alignment 控件为 -100 至 +100、步长 1，标签是毫秒；native wrapper 的 `SetComboDelay` 实际把值除以 100 后传出，不可根据标签自行改成除以 1000。音量同样除以 100；EQ 各自传 JSON 数组。试听状态变更、关闭 Surround 和组件 unmount 都发送停止测试信号。

任务先检查取消。模式任务先更新 runtime 草稿，再检查 front identity，再调用 `SetComboMode`；该调用成功后按模式写 rear/front EQ，关闭时写零值。front endpoint 选择成功后更新 runtime，再异步触发已启用的模式任务；rear volume 大于 0 时先解除 rear mute。音量、EQ、delay、replicate 和模式任务各自依据实际 `result/reason` 选择完成或失败发布路径；静态核对没有把所有 runner 归并为已完成。不能用 optimistic runtime 修改证明原生操作已接受。

设备事件 `onAudioDeviceStateChanged` 仅匹配保存的 front endpoint ID：state 4 断开时读取并关闭 combo，清 runtime front identity 并发布禁用/移除；state 1 重连时读取并启用 combo，恢复其 ID/name，发布 UI，再排队 front-speaker 事件。读取失败与已确认关闭不同，不能额外补一个默认端点。

### Virtualizer／FIR 的 client 产品与版本分支

当前保留的三款 client 对同名导出的行为不同。[client 分支统一收据](../../../.analysis/results/frontend/audio-surround-audit.json#/native_client_branch_boundaries)核对各自 manifest、原件、包装器 loader、PE 导出、226 处实际 `GetProcAddress` 调用及三份完整 FFI 初始化器，共 455 个完整函数／chunk、20,072 条指令和 88,556 字节 raw chunks，另保留 105 字节未枚举范围。禁用 collector 后的独立复核与 31 个篡改反例均通过；这些是静态收据检查，未执行 DLL、Rust 功能或设备。该片独立于下文路由服务的统计，不直接累加。

按 PE 导出名核对的 102 项为 CTC 35、Virtualizer 25、Spatial 27、Bass 15。1392 的所选 client 不导出这 102 个名字；1442 的 102 项进入活动函数或活动 thunk，3942 的 102 项均为 host-spatial-disabled 日志后返回 11 的正常路径。导出名数量不是不同函数、页面动作或已完成能力数量。下表列出其中三项 Virtualizer／FIR 的具体差异，页面可见性、包装器是否调用该入口和实际 DSP 消费者仍分别核对，不能按 `thxCarol` 名称共用一个“不支持”结果。

| 产品／client | 所选 Virtualizer／FIR 入口 |
| --- | --- |
| 1392，`thx_carol_routing_client_v1.0.80.0.dll` | [实际 PE 入口](../../../.analysis/results/native/aaf21e02a34db9ac603bd7a97721226486af8bf383707e70eef43b95159b140e/entrypoints.json)没有上述 102 项，包括下面三项导出；导出缺失不等于调用后返回 11 |
| 1442，`razer_clio_x_routing_client_v1.2.0.51.dll` | FIR setter [原生函数 `0x1fa50`](../../../.analysis/results/native/b84535e588da1582ef1938b41d58d272baf88f37d9b76db1a89d970aa6c66802/functions/1fa50.json)调用活动实现 `0x3c0e0`，仅内层返回 0 才调用已注册回调；JSON getter `0x1f5f0` 为活动读取，setter `0x1faf0` 跳转到 `0x3c390` 的解析／更新链，同样不能按 3942 的固定返回处理 |
| 3942，`razer_soma_pro_routing_client_1.2.1.59_2.dll` | `thxCarolSetVirtualizerFirs`／`thxCarolSetVirtualizerConfigJson`／`thxCarolGetVirtualizerConfigJson` 分别为 `0x20b40`／`0x20c40`／`0x20240`；完整函数记录 `Not available: SPATIAL_ON_HOST is disabled`，日志链正常返回后固定返回 11 |

三份当前 FFI 配置均为逐字节相同的 67＋4 分组，未绑定这 102 项 CTC／Virtualizer／Spatial／Bass API；所选 wrapper 也没有这些同名导出。1392 wrapper 缺少第二组四个 Monitor 导出，完整 `initElectron` 在配置结果为 false 时会移除该组重试；不能把第二组的缺失直接写作整份初始化必然失败，也不能将存在于 client 的未绑定导出补成页面功能。原件中实际使用的 combo、音量与 EQ 链继续按各自调用方追踪。

1442 的 [完整 JSON getter](../../../.analysis/results/native/b84535e588da1582ef1938b41d58d272baf88f37d9b76db1a89d970aa6c66802/functions/1f5f0.json)先检查目标 buffer、长度输出指针及正容量，无效时返回 4；实例 gate 阻断、无 owner、未知 selector 或空 JSON 走返回 1。取得非空 JSON 后先输出不含终止符的长度；在正 int32 可表示的长度范围内，只有容量严格大于该长度才复制并追加 NUL、返回 0，容量等于长度也返回 5 且不复制。实际指令以低 32 位输出长度并作有符号容量比较，复制仍使用完整 size_t，超大字符串的宽度边界须另测。该结果是所选 client 的读取契约，不能提升为硬件观测。FIR 的内部返回及回调也不能直接充当 DSP／设备 ACK。

3942 的上述 [FIR setter](../../../.analysis/results/native/9779234a43451b4500d4a4fe51a6b9e5750924ffb7facf499ce7a071cf7cc336/functions/20b40.json)、[JSON setter](../../../.analysis/results/native/9779234a43451b4500d4a4fe51a6b9e5750924ffb7facf499ce7a071cf7cc336/functions/20c40.json)与 [JSON getter](../../../.analysis/results/native/9779234a43451b4500d4a4fe51a6b9e5750924ffb7facf499ce7a071cf7cc336/functions/20240.json)没有处理调用参数、填充 getter 输出或调用 FIR 结果回调。这只界定这些导出的正常返回路径，不取消下文六支服务状态、默认 FIR、数据库与通知链，也不证明整个 client、服务或所有 DSP 均无实现。本节未执行官方 DLL；三款实际结果、初始化、错误与完整释放仍须分别对照。

#### 1442 的活动 FIR、IPC 状态与 DSP 配置边界

[1442 活动链收据](../../../.analysis/results/frontend/audio-surround-audit.json#/native_virtualizer_fir_1442)单独绑定该产品当前 manifest、client 和服务包；服务原件经过七层实际提取／嵌入关系追到 manifest 声明的包。复用两份完整静态捕获，在 client 9,731 个、服务 14,088 个函数中选取 79＋26 个完整函数继续分析；本片共 14,427 条指令、61,540 字节 raw chunks、9 个实际虚表槽和 44 条直接调用／跳转绑定，70 字节保留为未枚举范围。禁用 collector 与原生收集助手后的独立复核通过，43 个原件、来源、函数、虚表与语义篡改反例全部被拒绝。另有 33 个维护工具纯测试通过，新增测试仅验证描述符解析和 PE 字节范围；这些数量均不表示 FIR 功能或实际 DLL 对照通过，也不与其他切片直接相加。

FIR setter 的 [内部实现 `0x3c0e0`](../../../.analysis/results/native/b84535e588da1582ef1938b41d58d272baf88f37d9b76db1a89d970aa6c66802/functions/3c0e0.json)接收 selector、NUL 结尾的名称、左右 f32 数组指针及各自的有符号元素数。名称只是复制到配置中的元数据，所选链没有按名称读取 FIR 文件。空名称指针、空数组指针、非正长度或左右长度不同返回 4；实例 shutdown gate、无 owner 或未知 selector 返回 1。selector 0／1／2 分别更新 `GLOBAL` 的 Front／Side／Rear：深拷贝名称和两个数组，设置 changed，再将类型与 ID 的副本入队。不能把元素数当字节数。public setter 的伪代码将第三参误标为 `int`，实际汇编保留 R8 指针并传入内部 64 位参数；该伪原型不能直接用于执行 DLL，对照前仍须核实完整 ABI。

下面各例假定 owner 与对应状态可访问、输入指针有效、回调已注册，列出必须分别比较的静态边界；尚未执行这些 DLL／服务／DSP 用例。`length/2` 只记录实际除法，不能自行解释为 PCM 帧数或滤波阶数。

| 输入／边界 | client 与直接回调 | 服务解析／配置边界 |
| --- | --- | --- |
| 左右各 1 个 f32 | 接受本地复制；直接回调数量为 0 | 奇数长度不构造自定义两行矩阵，记录 `Bad Virtualizer FIR size`，保留配置构造结果 |
| 左右各 3 个 f32 | 接受本地复制；直接回调数量为 1 | 同样拒绝构造该自定义矩阵；不能从 client 返回 0 推断 DSP 生效 |
| 左右各 2／4 个 f32 | 接受本地复制；直接回调数量为 1／2 | 每个数组拆成两半，形成两行矩阵；是否最终配置成功仍单独核对 |
| 长度非正或左右不等 | 返回 4，不进入本地复制或直接成功回调 | 没有由本次 public setter 生成的状态提交 |
| 未知 selector，JSON 合法 | FIR setter 返回 1；JSON setter 解析成功后却可返回 0 且不改状态 | 不能把两个入口的同一 selector 合并为统一返回码 |
| getter 容量等于 JSON 字节数 | 长度输出已写入，返回 5，不复制 JSON | getter 读取 client 内存，未同步询问服务或设备 |

队列的实际 lambda 虚表槽进入 `0x73a50`，遍历注册 observer；构造器 `0x74fd0` 已将 Front／Side／Rear 的实际 IPC observer 绑定到 `0x864b0`／`0x86470`／`0x86430`，再分别进入 typed Any 打包、`0x77a70 → 0x30280` 请求／响应、committed state 解包及 revision／payload 比较。服务回复更旧 revision 时会记录可能重启并接受回复；发布状态的接收链另按更高 revision 更新。空回复、Any 解析失败或 typed Unpack 失败只记录日志，不能修改早已返回的 public setter 结果。请求／响应具有串行 mutex 和 socket 异常重建路径；完整连接参数、所有错误分支及在途取消仍需继续闭合。

直接 setter 回调仅在内部返回 0 后、回调 mutex 内调用，传入 selector、调用者原始名称指针、输入长度除以 2 和注册上下文，**不等待服务确认**。注册函数 `0x1f3b0` 在同一 mutex 内替换函数与上下文，非空时同步读取 0／1／2 并使用共享 1,024 字节名称空间，空函数可注销。通知注册 `0x3c7b0` 先移除旧订阅再挂入三支 observer，并立即触发三次通知；通知函数 `0x1fb00` 使用先清零的 256 字节名称空间，逐项读取 active、六组 biquad 与 FIR 标签。标签 getter 只复制至容量减 1，报告左数组元素数除以 2；owner 存在时，缺状态或未知 selector 仍可能返回 0，不能据此伪造已观测值。队列产生的其他通知可能早于或晚于直接回调，实际线程顺序仍待比较。

服务 dispatcher `0x2853c0` 的三支 Virtualizer 请求进入各自 typed state 修改与回复函数。六个正常／初始化消费者分别读取 Front／Side／Rear 配置，调用 [解析器 `0x2cbb20`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2cbb20.json)，再调用 [DSP 配置器 `0x2d8d90`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2d8d90.json)。解析器最多转换六组 correction biquad，采样率为 48 kHz，仅对非零、等长且偶数的左右 FIR 拆半；缺数组时日志写 `using default filters`，这里只证明保留构造结果。配置器要求两行和正长度，按变化配置四个 FIR processor 与六组 biquad，可能返回 false；所选六个消费者均不使用该 bool。因此 client 本地复制、服务 committed state、DSP 配置接受与最终 PCM／设备效果必须分别保存，服务状态回复不能冒充 DSP 成功。

正常清理已追到字符串／数组的状态所有权、排队 lambda 释放复制 ID、shutdown gate 与共享引用释放，以及队列 stop、signal、join 后销毁 map／mutex。公开 setter 未提供逐操作取消令牌或取消确认通道。下面继续记录同一 1442 服务 build 的 observer、音频 worker 与 PCM 消费者；这些静态结果不扩展上述收据的功能验证范围。

##### 1442 的 observer、PCM 顺序与配置切换

本段仅使用服务原件 SHA-256 `169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b`、ImageBase `0x140000000` 的[完整捕获](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/capture.json)。下列地址均为 RVA；不能套用 3942 的函数或虚表。

[`WasapiManager::init`，`0x2ca750`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2ca750.json)分别注册三种 typed observer，lambda 保存 manager 指针。注册函数持 state manager+80 的独占 SRW 锁，将回调写入按类型索引的 map；另持 +128 的队列 mutex 入队注册通知并 signal +208。注册通知的三张实际虚表 `0x6b6638`／`0x6b6548`／`0x6b6910` 的 invoke 槽均进入 `0x249e10 → 0x246620`，枚举该类型已有 ID 并调用刚注册的 observer。

| 分支 | 注册函数 | observer 虚表／invoke → 消费者 | aggregate 内 Virtualizer 指针 | PCM 声道对 |
| --- | --- | --- | --- | --- |
| Front | `0x288d60` | `0x6bb750`／`0x2ed0e0 → 0x2e70b0` | +8 | 0、1 |
| Side | `0x288f40` | `0x6baac0`／`0x2ecf80 → 0x2e7300` | +40 | 6、7 |
| Rear | `0x289120` | `0x6bad98`／`0x2ece20 → 0x2e7550` | +24 | 4、5 |

后续状态变化由 `0x295f80`／`0x296460`／`0x296940` 在释放 typed state 写锁后，分别经 `0x2a0e80`／`0x2a1760`／`0x2a2040` 入队类型与 ID 副本。三张通知虚表 `0x6b7430`／`0x6b65c8`／`0x6b68d8` 的 invoke 均为 `0x24a210`；它持 +80 独占锁遍历该类型所有 observer。队列线程 [`0x287b60`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/287b60.json)先在 +128 mutex 内取走任务，解锁后才调用 invoke，stop 时处理至队列为空才退出。三种 Wasapi observer 均重新读取 `GLOBAL`，而非直接使用通知的 ID。消费者通过 `0x2999d0`／`0x299b80`／`0x299d30` 获得对应 state 的共享 guard，保留至解析与 `0x2d8d90` 调用结束；这把 state 读锁不能证明音频线程也受同一锁保护。

manager 构造器 `0x2c95e0` 分配 +1720 的 aggregate，按 48 kHz、双声道、最大块长 4,800 的描述符进入 `0x2c8870`，创建并 prepare Front／Rear／Side 三对 Virtualizer 与 CTC。描述符 +8 为 2、+12 为 `0x12c0`，见 `0x2c9d15`／`0x2c9d1c`。`refreshState`（`0x2cc760`）为各 render endpoint 启动 [`streamThread`，`0x2d1790`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2d1790.json)：endpoint signal-flow 查找／准备受 manager+1488 mutex 保护，随后解锁。线程在音频事件等待或轮询后调用 [`fillBuffer`，`0x2d22a0`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2d22a0.json)，从 VAD 读取八声道平面 f32 数据、不足补零，先经过 endpoint signal flow 与全局 gain。只有当前 render endpoint ID 等于所选 Front endpoint ID 时，才调用 `0x2c8bb0(manager+1720)`，随后进入输出声道／格式转换和 WASAPI `ReleaseBuffer`。+1328 mutex 只保护所选 endpoint ID 的复制，VAD route 查询的 mutex 也已在 DSP 前释放。

`0x2c8bb0` 在 aggregate active 为 false 时跳过本层；active 时先应用八声道 mute／gain，再处理 **Front Virtualizer → Front CTC → Center／LFE 的 delay 与 biquad → Rear Virtualizer／CTC → Side Virtualizer／CTC**。后两组通过 `0x2c9160`，各组两个 processor 指针均存在才执行。虚表实际字节绑定如下，槽号从 0 开始；`0x23dfa0` 是本 build 的空 prepare。

| processor | 虚表 RVA | slot 0：释放 | slot 1：prepare | slot 2：PCM |
| --- | --- | --- | --- | --- |
| Virtualizer | `0x6bb5e0` | `0x2e1630` | `0x2d68f0` | `0x2d6250` |
| 单路 Convolver | `0x6bb598` | `0x2d7be0` | `0x2d6010` | `0x2d5700` |
| 双声道六级 BiquadCascade | `0x6babb8` | `0x2e3360` | `0x23dfa0` | `0x2d56d0` |
| Mixer | `0x6baa40` | `0x2e1800` | `0x2d56c0` | `0x2d5580` |

单声道 Biquad 使用另一套槽位：虚表 `0x6b7478` 的 slot 0 为释放 `0x2b7400`、slot 1 为 PCM `0x2b07b0`、slot 2 为系数 hook `0x23dfa0`，不能套用上表的 prepare／PCM 槽义。

[`Virtualizer::process`，`0x2d6250`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2d6250.json)先校验输入／输出 span，对正帧数保存 dry 副本并复制输入至输出，再依次运行四个 convolver。`0x2df340` 实际是逐行复制，不能因部分伪代码遗漏 RDX 而当作清零函数。配置拆分与 PCM 消费的对应关系是：

| Virtualizer 成员偏移 | FIR 数据 | PCM 输入 | 临时输出 |
| --- | --- | --- | --- |
| +528 | `fir_l` 前半 | L | +4144 |
| +808 | `fir_l` 后半 | L | +4176 |
| +1088 | `fir_r` 前半 | R | +4208 |
| +1368 | `fir_r` 后半 | R | +4240 |

随后 wet L 取 `conv(L, fir_l 前半) + conv(R, fir_r 前半)`，wet R 取 `conv(L, fir_l 后半) + conv(R, fir_r 后半)`，两行经过 correction cascade，再进入 Mixer，最后复制回输出。这里不能按惯常的交叉矩阵命名自行交换 FIR。单路 [`0x2d5700`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2d5700.json)保存历史输入，先做时域乘加；存在剩余 partitions 时按块清零工作区、补零、FFT、频域乘法、逆 FFT 与 overlap-add，最后复制输出并移动、清理剩余累加区。FFT 底层各数值／CPU 分支尚未因此获得实际一致性验证。

配置切换有两种不同边界。`0x2d8d90` 在 FIR 变化时，依次调用四次 `0x2d85d0`；四次均执行，再合并 bool，失败即返回，所选链没有整组事务回滚。单个 convolver 会直接重建系数、history、FFT plan、partition 与输出累加区。六组 biquad 则由 `0x2dac70` 为两声道分别计算、排队系数（`0x2b5440 → 0x2b0ba0`），并经 `0x2e0a80` 排队 cascade metadata；`0x2d56d0 → 0x2df3c0` 在处理块开始取走可取得的 metadata，再按两声道、六级顺序调用 enabled 项。每个 `0x2b07b0` 另在块开始取系数，保留其两个历史状态，执行转置二阶递推；处理期间设置 MXCSR 的 DAZ／FTZ 位，结束恢复原值。局部队列／共享快照不等于四个 FIR、active 和十二个 biquad 实例会一起原子切换；所选 observer 与 PCM 链尚未找到覆盖整组配置的共同锁。

Mixer 的 [汇编 `0x2d5580`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2d5580.json)还明确了 active 的实际消费者：配置器将 target 设为 0 或 1，首次处理前 current 直接跟随 target；prepare 将 rate 写为 f32 `0.2`。每次非空处理块推进 `current += copysign(min(abs(target-current), rate), target-current)`，`0x530350` 是拷贝 float 符号位，不能从失真的伪原型推断为 double 算法。current 恰为 0／1 时分别复制完整 dry／wet 矩阵；中间权重分支先逐行清空输出，但随后两次 `0x2b2cd0` 只传首行数据指针和 `nframes`，没有第二行的同样循环。该静态边界仍须用真实 PCM 结果比较，不能直接以常见双声道 dry／wet 公式代替该 build 的实际结果。active=false 通过混合回到 dry，并不在 `0x2d6250` 中跳过四路 FIR 的计算。

正常释放由 [`0x2ca220`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/2ca220.json)设置停止标志、signal、注销 endpoint volume COM 通知，随后在 manager+664 mutex 内置各 stream stop 并 `Thrd_join`；成功等待后才释放 +1720 aggregate。stream 正常退出路径经 `0x2cfb00` 停止 audio client、释放两个 COM 引用并关闭事件 HANDLE，再退出 VAD route 和释放缓冲／共享引用。aggregate deleter `0x2d8250` 进入各 processor 的释放槽；Virtualizer `0x2e1630 → 0x2e1670` 先释放六份 PCM 工作区、两声道各六个 biquad 与其 pending/current 系数所有权，再释放 cascade 队列、四个 convolver 及配置。convolver `0x2d6130` 释放两个 FFT allocation、partitions、history、系数与输出／工作区；这些均是正常调用链的静态结论。

全局正常停止与所选析构／构造异常边界已继续追到下一节；`0x2ca750` 未保存三个注册返回 ID，manager 析构也没有对应注销，仍不能据音频线程 join 宣称回调生命周期安全。全部回调抛异常与重入、shutdown／IPC 交错、并发配置的实际时序、该版本的完整持久化／默认恢复和全部平台／固件／传输分支继续保持未闭合。源码身份与已列调用链的静态语义可核验；独立 Rust 实现、页面／后端接线、功能纯／模拟测试、官方实际 DLL 对照和设备／DSP 验收分别保持未验证。

##### 1442 的全局停止与 observer 所有权

本节复用同一服务 SHA 的完整 capture，续核 57 个完整函数、57 个 chunks、3,853 条指令及 16,592 字节原始块；24 字节未枚举为指令。另核五张虚表的九个实际槽及本 PE 的 32 字节 FH4 整数解码表；与上节及既有收据存在复用，覆盖数不能相加。这些检查静态追踪全局 owner、通知线程和音频线程的停止关系，不是实际功能执行结果。

[全局单例 `0x240c50`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/240c50.json) 在构造时注册 `atexit(0x543840)`；后者经 `0x26aea0` 调用 [`0x26c180`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/26c180.json)。服务控制入口 `0x24a7c0` 对 STOP／SHUTDOWN 设置 STOP_PENDING 并 signal 停止事件；`SvcMain=0x24ac80` 等到该事件后注销电源通知，再调用同一停止函数，返回后才报告 STOPPED。电源通知／后续事件创建失败也有显式停止分支；这些正常调用关系不代表进程被强制结束时同样执行。

停止入口在 `0x26c192` 使用原指令 `xchg al,[rcx+0x58]` 原子设置 gate，只有旧值为 0 的调用继续。全局 base 为 RVA `0x75f6a0`；`0x26aff0` 的构造绑定和实际 shared_ptr 控制块虚表确认以下顺序：

| 全局成员偏移 | owner | 正常停止中的位置 |
| --- | --- | --- |
| +48／+56 | IpcServiceMessageQueue | 首先置空并释放全局引用 |
| +32／+40 | ServiceStateManager | 第二个置空并释放 |
| +64／+72 | DebugWindow | 第三个置空并释放 |
| +0／+8 | WasapiManager | 第四个置空并释放 |
| +16／+24 | 数据库 owner | 第五个置空并释放 |
| +80 | LogManager 原始所有权 | 清空后析构／free，再调用全局库清理入口 `0x32f830` |

五次置空均进入 [`0x272030`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/272030.json)：`0x461f90` 取得全局 SRW 独占锁，在锁内替换 object／control pair，`0x461fa0` 解锁后以 `lock xadd` 减旧 control 的强引用，只有旧计数为 1 才同步调用 destroy。后续 while 检查的是**刚被置空的原全局槽**，不是保留的旧 control；没有并发重新发布时直接跳过，不能将这些 while 写成等待全部借用或在途回调结束的屏障。gate 会拒绝后续入口借用，却不会撤回此前已经取得的局部 shared_ptr。重复停止调用因 gate 已设置而跳过正文；它本身不证明第一次调用已完成。

虚表原字节将全局状态 owner 的最后一次释放接到 `0x6b2580[0] → 0x238e40 → object[0]`，再由 `ServiceStateManager` 的 `0x6b71f0[0] → 0x284610 → 0x284650` 析构。该析构先执行 `api_state` 及 14 个 `GLOBAL` 持久化入口，释放派生成员，最后才进入 [StateManager 析构 `0x287240`](../../../.analysis/results/native/169bef9718b1b9c32ad4bc1072baec51b6dd85b293ea834e292b939b28f3d82b/functions/287240.json)。后者在 `0x28725c` 原子设置 +296 stop、signal +208；存在通知线程时检查 self-join 并 join，成功后清空 +280 thread pair，再释放 +368／+304 map、条件变量／mutex、+88 队列，最后经 `0x2875f0 → 0x287e50 → 0x245bc0` 销毁 +16 observer map 及各 callable。正常 join 的对象是通知队列线程；队列仍按上一节的条件排空，不能把 stop 字节解释为丢弃所有已排任务。此链依靠整个 map 析构释放 observer，未新增逐 ID 注销证据。

IPC 队列的最后一次释放经 `0x6b24b0[0] → 0x238e40 → 0x6b2350[0]=0x26ae00`。派生析构先减其 +264 保存的状态 owner 引用，再释放 Any 成员，随后才由 `0x26a420` 设 stop、向控制 socket 发送 `shutdown`、join 队列线程并关闭三个 socket／context；发送失败且 errno 不为 11、self-join 与 join 失败均有异常分支。因全局正常顺序中 +32 仍未置空，不能仅凭这个成员释放顺序断言状态已经销毁；也不能把派生析构简写成“先 join 再释放状态”。Wasapi 最后一次释放则由 `0x6b2368[0]=0x272ec0 → 0x2ca220` 停止并 join 已列 PCM stream，join 成功后才释放 +1720 aggregate。

三种 Virtualizer observer 的 callable 只保存 raw manager 指针；invoke 各自检查全局 gate，并借用 ServiceStateManager 的 object／control pair。所借状态 owner 保留至 typed state guard 与 `0x2d8d90` 调用结束后才减引用，**没有同时取得 WasapiManager 的强引用**。因此，“状态最终析构时先排空通知、后释放 observer”与“全局必定等完所有回调再销毁音频 owner”是两个不同结论：前者有正常路径证据，后者未由已清空槽的 while 或 gate 证明。若最后一份状态引用在通知线程回调退出处释放，其析构还会触及明确的 self-join 检查；是否出现该交错及 raw 指针在实际调度中的有效期尚未执行验证，不能登记为已发现必现崩溃，也不能登记为并发安全已确认。

异常边界直接依据本 PE 的 FH4 数据与 CRT 消费者核对，未按反编译函数名猜测。`0x26aea0`／`0x284650`／`0x287240` 的 FuncInfo 分别位于 `0x6db330`／`0x6ddb08`／`0x6ddefc`，header 均为 `0x60`，没有 UnwindMap／TryBlockMap；NoExcept 位由本 PE 的 `0x4699e8 → 0x468b28` 实际消费。`0x2ca220` 的 FuncInfo=`0x6e1fe0`、header=`0x68`，唯一 unwind 条目 `0x6d432e` 的 action 为 `0x466888 → 0x479518`；后者调用所装终止处理器并进入 abort。故 self-join、join 失败或其他 C++ 异常越出所核 noexcept 边界时，不能补称会继续执行正文剩余释放、恢复配置或成功停止；本节没有覆盖全部 CRT、SEH 与外来异常。

另核 `StateManager` 构造 `0x287320` 的 14 条 FH4 unwind：`0x536760…0x53682c` 按已完成的构造状态释放 observer／state／lock map、队列、mutex／条件变量；`0x276bc0` 释放尚未交接的线程参数分配。thread 成员 guard `0x22de90` 对仍 joinable 的线程进入终止处理，不额外执行 stop／join。该构造的 FuncInfo=`0x6ddf24`、header=`0x28`、无 TryBlockMap，不能把资源 guard 当作业务重试、默认状态写回或成功恢复。全局并发退出、回调抛异常／重入、持久化实际结果、Rust 独立实现与官方 DLL／服务对照继续保持未验证。

### 3942 的实际路由协议与状态完成点

[Soma 路由服务收据](../../../.analysis/results/native/af4fcfb594afeb7e678d4cbe3cf7f41556252a31f7f839b567f2cea855033b82/routing-semantics.json)对应 PID3942 的 THXVAD3942 1.2.1.59 及其 `razer_soma_pro_routing_service.exe`。逐层官方 manifest／资源／静态提取来源保留在收据；完整捕获为 12,253 个函数、零反编译失败。基础片核对 174 个完整函数、24,414 条指令、10 张实际表、26 条固定断言及 452 个 artifact；107,289 字节 raw chunks 中的 190 字节没有枚举为指令，原始范围完整保留。再加音量、声道／延迟、活动播放线程、试听生命周期和 WASAPI 格式／PCM，共六个基础切片，去重为 265 个函数、655 个 artifact。

另十九片覆盖 endpoint refresh、VAD／driver、前端桥／命令／页面分支／控件 CSS／横向音量／灯光效果、GLOBAL EQ／异常生命周期、重采样／FFT、非 VAD 四段 flow／异常生命周期／外层所有权，以及 CTC／Virtualizer Front 默认、六支持久化、DB 文件边界和消费者分类。当前 25 个正式 section 按原件 SHA＋RVA 和精确 artifact 身份合并为 965 个不同函数、6,091 个去重 artifact：服务 746、驱动 194、包装 DLL 22、client 3 个函数；六份前端片合并为 514 个完整 AST owner。交集的完整记录必须相同；捕获范围、逆向语义、Rust、接线、模拟验证和实际验收分别判断。

该服务使用 ZeroMQ＋protobuf Any。完整 `thx_wasapi_routing.proto` 描述符为 5,634 字节，含 36 个消息和两个枚举，没有 gRPC service 或外部 proto dependency；定义数量不等于请求支持数量。实际队列表 `0x37f858` 的 processRequest 槽指向 `0x43d30 → 0x5e440`。路径生成器 `0x42cd0` 的真实参数是输出、namespace、leaf 三项，不能沿用伪代码误识别的单参数 ABI。三个地址均含完整 ProgramData／THX 命名空间：

| 通道 | 地址 |
| --- | --- |
| 请求／响应 | `ipc://{ProgramData}\THX\soma_pro\thx_reqrep` |
| 进程内停止控制 | `inproc://{ProgramData}\THX\soma_pro\thx_control` |
| 状态发布 | `ipc://{ProgramData}\THX\soma_pro\thx_pubsub` |

外层 Any 解析后，按 type_url 最后 `/` 后的完整消息名匹配，不要求固定 URL 前缀。真实请求只有 16 支：13 组状态（WasapiManager、ChannelMaps、Eq、Front／Side／Rear Ctc、Front／Side／Rear Virtualizer、SpatialGlobalSettings、BassManagement、Delay、EndpointVolume），以及 PlayDelayTestSignal、StopDelayTestSignal、ClientConnect。未知类型或外层解析失败记录日志并返回空字串。所选调用方没有检查 typed UnpackTo 的返回值，畸形 inner payload 不能据此称为已严格拒绝；StreamStates、EndpointsState 等有定义或通知，不代表有对应写请求。

`front_ctc_virtualizer_state_and_defaults` 独立核对 Front CTC／Virtualizer 的 54 个完整函数、8,785 条指令、6 张实际表、12 个数据区域、25 处指令断言、46 条直接绑定和 302 个 artifact；39,333 字节 raw chunks 中保留 38 字节未枚举范围，31 个证据篡改反例全部拒绝。本片只闭合所选 Front 两支的状态与默认数据语义；Side／Rear 在此只保留描述符，另由六支持久化片逐一核对，实际处理与完整异常仍另验。

这两支已追到服务状态 owner、revision/diff、通知队列和 IPC Any 发布，但尚未找到它们挂入实际 WASAPI／DSP 流的证据。Front CTC 与 Front Virtualizer 分别由 observer slot2 `0x71780`／`0x714b0` 进入 typed pack、`0x600e0` 和 `0x5b7f0`；socket 发送失败且 `errno=11` 时仍可返回 1，不能把发布尝试当作送达或 DSP 生效。CTC 更新沿 `0x5e440 → 0x6dd50 → 0x63910 → 0x77da0`，Virtualizer 更新沿 `0x5e440 → 0x6ebf0 → 0x64000 → 0x79840`；旧 revision 不复制，新 revision 仅记录并保留服务状态，equal revision 且 payload 有差异才复制并递增 revision。

默认配置只证明服务／数据库初始化：CTC 的默认 active 均为 false、gain=0、race attenuation=-6、delay=0，内部临时 biquad 参数为 1000 Hz、Q≈0.7071、48 kHz；Virtualizer 默认 active=true、`fir_name='Built In'`，左右各 512 个 f32 的 FIR 是单位脉冲（index 0 和 256 为 1.0）。畸形 inner payload 的 `UnpackTo` 返回值未检查，不能宣称必然拒绝；默认值也不等于设备实测。后续必须分别验证 UI 分支、真实消费者、异常／取消／释放和 DLL／服务／设备对照。

`ctc_virtualizer_six_branch_persistence` 按每支真实函数、RTTI 和虚表核对完整家族：120 个完整函数、16,607 条指令、18 张表、6 份 RTTI、30 个数据区、66 处指令锚点、194 组直接绑定、24 处 RIP 绑定、275 条源码断言、10 个消息定义和 323 个 artifact。71,861 字节 raw chunks 中有 48 字节未枚举；与 Front 基础片按 SHA＋RVA 去重为 123 个不同函数，不能记为 174。禁用 collector 后独立复核通过，46 个家族篡改反例全部拒绝。

| 状态／variant | 持久化 key | typed reader／writer RVA | publication RVA |
| --- | --- | --- | --- |
| Front CTC／4 | `front_ctc_state` | `0x77910`／`0x73d50` | `0x71780` |
| Side CTC／5 | `side_ctc_state` | `0x781f0`／`0x74110` | `0x71690` |
| Rear CTC／6 | `rear_ctc_state` | `0x78ad0`／`0x744d0` | `0x715a0` |
| Front Virtualizer／7 | `front_virtualizer_state` | `0x793b0`／`0x74890` | `0x714b0` |
| Side Virtualizer／8 | `side_virtualizer_state` | `0x79c90`／`0x74c50` | `0x713c0` |
| Rear Virtualizer／9 | `rear_virtualizer_state` | `0x7a570`／`0x75010` | `0x712d0` |

初始化 `0x5dd60` 对 GLOBAL 按上表顺序读取。只有对应 typed reader 返回 false 才构造该支默认，成功读取保留配置；两种正常路径都递增 revision、释放锁后通知。reader 的可证 false 包括有效 owner 下的底层 DB 指针为空、DB status 错误、外层 Any 解析失败和类型不匹配。owner+440 本身为空时只日志、重新读取并继续进入 `0xb4a0`，而后者入口直接解引用 owner；不能把 owner 缺失也归为安全 false／默认初始化。inner `UnpackTo` bool 未检查的边界六支都存在。

析构 `0x5d6d0` 按同样顺序保存六支：先取得 typed readonly state，再经保存 wrapper 调对应 writer；状态不存在只日志并跳过，不构造默认。writer 检查自己的 DB owner、外层序列化和 DB status，但不检查 Any PackFrom；wrapper 丢弃 writer bool，因此普通返回 false 不阻止后续保存，六项不构成事务或回滚。`0xb4a0` 使用底层虚槽 `+32`、options 低两字节 `01 01` 与空指针；`0xb720` 使用 `+8`、options 首字节 `00`。这些字段的底层消费者另由下一片核对，保存返回仍不能证明落盘耐久。

请求 revision 比较、header 排除、锁后通知与 Front 规则一致，均由每支独立绑定证明。初始 task slot2 走 `0x22c00 → 0x1f410`，变更 task slot2 走 `0x23000`；publication callback 执行时重读当前 typed state，再打包发送，不保存请求时快照，不检查序列化返回，也不代表设备读回。派生析构先完成这六次保存并释放 DB shared owner，随后基类 `0x602c0` 停止／join 通知 worker，worker 会排空已有队列；全部并发与异常路径、真实 DSP 挂载、Rust 实现、UI 接线和实际功能结果仍未验证。

`ctc_virtualizer_database_boundary` 继续核对实际 DB owner、文件适配器和 OS 导入：58 个完整函数、7,130 条指令、4 张虚表、57 处指令锚点、33 组直接绑定、4 处 RIP、12 处虚槽绑定、5 项文件 API、11 处导入调用、86 条源码断言及 220 个 artifact。29,996 字节 raw chunks 中有 29 字节未枚举为指令；禁用全部 collector 后独立复核与 48 个篡改反例通过。三份 CTC／Virtualizer 片去重为 178 个函数，本片相对前两片新增 55 个，不与完整专项的 union 混算。

owner 的实际构造沿 `0x440f0 → 0xa820 → 0xadd0 → 0xccd10 → 0xc8bc0` 安装 DB 表 `0x305f50`，服务从 routing `+16/+24` 复制 shared owner 到 `+440/+448`。读槽 `+32 → 0xcaf80` 依次查询 writable memtable、适用的 readonly memtable、磁盘版本；options byte0 只在实际读 block 时控制 checksum，byte1 仅在 block 可缓存时控制加入缓存。空 snapshot 使用当前 sequence；显式 snapshot 的 sequence 在 `+24`，查找 tag 为 `(sequence << 8) | 1`。不能将两字节 `01 01` 解读为每次读取都经过磁盘校验。

写槽 `+8 → 0xcd430 → +24/0xcf730` 的实际 ABI 为 DB、status out、options、key、value 五参；batch encoder `0xc4410` 则为 batch string、key、value 三参，不能沿用遗漏实参的伪代码签名。保存选项首字节为零，日志仍经 `0xd45e0 → file slot3/0xe3190 → 0xe5530 → WriteFile` 写出，仅跳过非零选项要求的 `file slot4/0xe5080 → FlushFileBuffers`。日志写失败或所请求的同步失败均跳过 memtable 更新；同步失败还会在 DB `+544` 为空时保存错误并唤醒等待者，后续 `0xcbd30` 先检查该错误。该分支不证明所有日志失败都会保存为同一状态。`WriteFile` 只检查 BOOL，未比较 NumberOfBytesWritten；不能由成功返回推定完整字节或断电耐久。

DB 最后一个 shared 引用释放后沿 `0x11bb0 → 0xc9300 → 0xc8f80` 设 closing、等待后台工作，再调用 file slot0=`0xe2cb0` 关闭句柄。该 file 析构自身不 flush、不调用 FlushFileBuffers，且丢弃 CloseHandle 结果；另一条显式 close `0xe2f60` 会先 flush 并保留较早错误，但不是这里选择的析构槽。共享引用释放不一定立即关闭 DB，也不能凭这个析构断言全部 shutdown 路径都无同步。完整修复／重建、日志恢复／压缩、所有间接调用与异常并发、真实文件副作用、Rust 实现和实际 DLL／服务／设备结果仍未验证。

`ctc_virtualizer_consumer_inventory` 核对六支所选入口的全部保留交叉引用及实际 callable 分派：164 个完整函数、165 个 chunk、17,100 条指令、102 个入口记录、270 条引用、18 张表、36 处表地址绑定、24 处分派、83 组直接绑定、192 条源码断言及 360 个 artifact。72,388 字节 raw chunks 保留 50 字节未枚举范围；禁用所有 collector 和构建 plan 后独立复核通过，57 个篡改反例全部拒绝。四份 CTC／Virtualizer 片去重为 232 个函数，相对前三片新增 54 个；其中一项已在其他服务片出现，因此全专项只新增 53 个函数。

六支 readonly 的实际直接消费者分别为 publication、析构保存和请求序列化，不能将后两类误标为 DSP 处理：

| 状态 | readonly | publication read／析构保存／请求读取 |
| --- | --- | --- |
| Front CTC | `0x72130` | `0x67180`／`0x68ca0`／`0x6e040` |
| Side CTC | `0x722e0` | `0x67370`／`0x68e90`／`0x6e520` |
| Rear CTC | `0x72490` | `0x67560`／`0x69080`／`0x6ea00` |
| Front Virtualizer | `0x72640` | `0x67750`／`0x69270`／`0x6eee0` |
| Side Virtualizer | `0x727f0` | `0x67940`／`0x69460`／`0x6f3c0` |
| Rear Virtualizer | `0x729a0` | `0x67b30`／`0x69650`／`0x6f8a0` |

六个 registrar 的直接调用者为构造 `0x5d050`，六个保存 wrapper 的调用者为析构 `0x5d6d0`。CopyFrom 的其他调用者 `0x7e050/0x7e330/0x7e610/0x7e8f0/0x7ebd0/0x7eeb0` 是内存 state map 的插入／variant 替换；对象 `+16` 为 header 指针、`+24` 为 config 指针，node `+176` 保存 variant 4..9，旧 variant 先销毁再重建。深拷贝和未知字段保留不代表系数已经进入音频流。

CopyFrom 的另外 12 条数据引用已从原 PE 的 exception directory 追到 `UNWIND_INFO` 的 CHAININFO 三元组：例如 `0x39fbb0` 保存 BeginAddress／EndAddress／UnwindInfo RVA，属于异常元数据，不能当作间接 DSP 函数指针。真正的通知任务在 `0x60be0` 释放队列 mutex 后调 callable slot2：初始任务经 `0x22c00 → 0x1f410`，变更任务经 `0x23000`，再调用六支 publication。所列 18 张表只覆盖本片类型，不证明穷尽全部运行时 callable；仍保留 1,798 条未展开直接边和 35 条间接／导入边。实际 WASAPI／DSP 数据桥、完整异常并发、外部消息送达及 Rust／DLL／设备结果继续未验证。

状态请求先按 StateHeader.id 定位，独占锁内同步执行 revision callable，然后读当前服务状态并打包响应；缺失 type／id 还可能注册内存状态，不能推断设备在线。revision 分支如下：

| 输入 revision／内容 | 服务状态与通知 |
| --- | --- |
| 小于当前值 | 保留当前值，不拷贝请求状态 |
| 等于当前值且有效载荷相同 | 比较排除 state_header，不拷贝、不增加 revision、不由该 callable 产生变更通知 |
| 等于当前值且有效载荷不同 | 锁内拷贝并增加一次 revision，释放状态锁后入通知队列 |
| 大于当前值 | 记录服务可能重启，保留并返回当前服务状态 |

通知 worker `0x60be0` 在队列锁内取出任务，解锁后调用真实 callable slot2；停止时仍排空已有队列。服务 publication observer 另读取当时的当前状态、打包 Any 并发布，不能凭本请求回包推断所有观察者已执行，也不能假定每条通知对应独立的历史状态快照。getter 失败保留空／未观测结果并记录错误，服务状态及配置默认值都不等于硬件实测。

变更 callable `0x23000` 对回调列表使用独占 SRW 锁：`0x23025 → 0x21d414 → KERNEL32!AcquireSRWLockExclusive`，尾部 `0x230a6 → 0x21d424 → ReleaseSRWLockExclusive`。收据的 `observer_lock_bindings` 逐项核对原始指令、导入 thunk、IAT、lookup 和导入名；不能按只读遍历推断共享锁。状态读取自身的 shared lock 属于另一把锁。

ClientConnect 使用文件版本及固定 API 字符串 `6` 构造 ClientConnectAcknowledge；实际分支检查 API 的长度和精确字节相等。相等才遍历并发布已注册状态，API 不等仍记录日志并返回已准备的 ack。所选函数未见独立 file_version 相等拒绝门，ack 不能作为 WASAPI 初始化或设备写回确认。两条试听请求的空响应同样不能独立确认播放／停止。

WASAPI 初始化 `0x97810` 的六组真实 registrar 已分别核对 RTTI 原字节、安装表的引用、注册调用和 invoke→consumer，不能只按回调命名猜测类型：

| 通知注册类型 | 表及消费边界 |
| --- | --- |
| WasapiManagerState | `0x382348 → 0xaba00 → 0xa3f40`，读取 GLOBAL 后到 `0x98710` 系统更新入口 |
| ChannelMapsState | `0x382980 → 0xab8f0 → 0xa4140`，继续到 `0xa8f00` |
| EqState | `0x382bb8 → 0xab8b0 → 0x981d0`，按当前 stream 访问状态 |
| DelayState | `0x382128 → 0xab7a0 → 0xa4340`，继续到 `0xa9120` |
| VadChannelVolumesState | `0x3825d8 → 0xab670 → 0xa4540`，实际读取 EndpointVolumeState 后调用 OS 音量接口；registrar `0xa49b0` 的 RTTI 与 reader 类型须分别判断 |
| EndpointVolumeState | `0x382cb0 → 0xab550 → 0xa47c0`，实际读取 VadChannelVolumesState.multiplier，仅在 `0 < multiplier ≤ 1` 时更新 manager+520 的运行时 float，没有直接写 OS 属性 |

数据库边界包含 open／create、重试与损坏修复、typed Any 读写和失败日志。启动读取 api_state 和 13 组 GLOBAL 持久化状态，API 不匹配存在清空本地数据库分支。derived ServiceStateManager 析构先保存 API6 和状态，随后 base 才设停止、signal、join 并销毁通知队列；不能改述为全部回调停止后才保存，也不能由该顺序证明保存成功。请求队列析构设置停止，发送精确八字节 shutdown，join 后关闭 sockets／context；非 EAGAIN=11 的收发错误仍有异常分支，context 终止可在 EINTR=4 时重试。所选 WasapiManager 析构有音量回调注销、停止／等待 stream 线程和 OS 引用释放，完整恢复与关闭竞态仍需另核。

同一收据的 `endpoint_volume_consumer` 进一步核对 44 个完整函数、5,696 条指令、六张表、37 条固定指令断言、六组 RTTI／variant 绑定和 22 条直接调用绑定，独立重读 173 项证据。23,205 字节 raw chunks 中有 28 字节未枚举为指令，原始范围仍保留；独立复核的六个篡改反例全部拒绝。

通知类型与实际读取类型交叉：`0xa4540 → 0x73060` 的实际 RTTI 和 variant tag15 证明它读取 EndpointVolumeState，使用 muted+24 和 float volume+28；`0xa47c0 → 0x75f10` 则读取 tag12 的 VadChannelVolumesState，取 multiplier+40。后者的零、负数、大于一或 NaN 不更新运行时标量。不能按 registrar 名称改换字段来源，也不能仅凭交叉绑定判定官方缺陷。

端点注册 `0x9c210` 先按 id 获取 IMMDevice，再以实际 IID `5cdf2c82-841e-4546-9722-0cf74078229a`、CLSCTX23 激活 IAudioEndpointVolume。端点接口和 callback 的两张 map 在注册通知前已插入；注册或初读失败会返回 false，但本函数未删除已插入项。成功注册后依次读 GetMasterVolumeLevelScalar、GetMute，两次 getter 均成功才同时写入 volume／muted 状态；失败日志可能保留较早的注册 HRESULT，不能据日志中的旧值推断当前读取结果。

真正 OS 写入由 `0xa4540` 按通知 id 查端点，在持有状态共享锁期间依次调用 slot14／+112 的 SetMute、slot7／+56 的 SetMasterVolumeLevelScalar。两个调用均传 event GUID `137f8c2b-a1ac-4252-9e74-bd4d66991816`，两个 HRESULT 都被丢弃，没有写后读回或独立成功确认。缺状态会日志，缺端点跳过写入；不能把 EndpointVolumeState 的请求回包直接连成同名状态写 OS 的确认。

实际 callback 四槽表 `0x382320` 的 OnNotify 指向 `0x9f3b0`。VAD 标记分支先读取通道并刷新 GLOBAL VadChannelVolumes，再比较 event GUID；自己的 GUID 只跳过 EndpointVolume 反馈，不跳过此前的 GLOBAL 刷新及其通知。外部 GUID 将 muted／volume 写入类型化 EndpointVolumeState，无有效载荷相等门禁地增加 revision，解锁后排队通知。通道读取在静音时置八个运行时标量为零，否则读取八通道并对超出实际通道数的位置补零，getter HRESULT 未检查；GLOBAL repeated vector 至少扩为八项，既有更长 vector 不截断，这次 GLOBAL 刷新同样无相等门禁地增加 revision、解锁后排队。这些服务状态和零值不构成设备成功应答。

`ChannelMapsState` 通知沿 `0x382980 → 0xab8f0 → 0xa4140 → 0xa8f00`，在 manager `+0x210` 的锁内清空并重建 `+0x218` 的内存增益路由表。状态 vector/count 为 `+0x38/+0x30`，endpoint id 为 `+0x28`；`0xaeb10` 选 endpoint，`0xa60d0` 以 output_channel 为外层 key，`0xa5e10` 以 input_channel 为内层 key。所选调用准备 `10.0` 和 `gain_db / 20.0`，调用 `0x23d110` 并将 float 结果存入节点 `+0x14`。该函数没有直接的 stream、OS 或设备写入，不能由内存表更新推定音频设置已应用。

上述收据的 `channel_map_delay_consumers` 核对 24 个完整函数、3,520 条指令、3 张实际表、28 条固定指令断言、15 条直接绑定及 135 个输入。14,015 字节完整 chunks 中有 18 字节未枚举为指令；独立复核另核 12 处指令锚点，五个篡改反例全部被拒绝。这些统计限定于本切片的静态证据。

`DelayState` 通知沿 `0x382128 → 0xab7a0 → 0xa4340 → 0xa9120`，从 state vector/count（`+0x20/+0x18`）读取 endpoint，使用 manager mutex `+1488`、map `+1568` 与 sentinel `+1576`（此处为十进制字节偏移），通过 FNV endpoint map 查找，缺项时创建 `0x78` 字节的 `EndpointSignalFlow`。新建对象按 48000 Hz、8 channels、4800 buffer frames 初始化；4800 属于本机 buffer 准备，不能称为实际 WASAPI period。delay（`+0x18`）大于 100 取 100，小于 0 取正零；NaN 与负零不被这两个有序比较替换。值先扩为 double，乘 `0.001` 和 `48000` 后转回 float，写入 flow `+104`，没有取整，允许小数 sample。该字段由活动播放链中的 `0x9f750` 读取，非零值进入 `0x87df0` 插值读取，具体挂载与消费见下文。析构路径为 `0x972f0 → 0x97280/0xa68f0` 与 `0x97160 → 0xa0310`；完整 stream 重建与异常恢复仍须另验。

普通注销 `0x9cd80` 临时 AddRef 两对象，调用 UnregisterControlChangeNotify 时未检查 HRESULT，随后删除两张 map 的对应项并释放引用。manager 析构另对注销 HRESULT 检查／记录，再在后续 map 析构释放节点 COM 引用；callback 最终 Release 到零才经实际 deleting-destructor 槽释放其字符串、引用及对象。当前只确认所选释放路径，没有证明所有回调已静止、原值已恢复或失败注销后仍安全；锁跨 OS 调用的并发、取消和关闭竞态继续保持未验证。

### 活动播放线程与流对象生命周期

同一收据的 `active_render_chain` 保留 25 个完整函数、7,123 条指令、79 个字节锚点、18 组直接绑定、一张流虚表及三个 COM IID，共 180 个证据文件。30,660 字节完整 chunks 中有 31 字节未枚举为指令。独立复核重新核实原件、捕获身份、表槽、调用字节及精确证据集合，七个篡改反例全部被拒绝。

正常 refresh `0x98710` 建立共享停止标志和线程 payload，将 manager 写入 `+64`、真实 worker `0x9d4a0` 写入 `+72`，以 `0xadfd0` 作为 `_beginthreadex` 入口，再由 thunk 调用 worker。worker 与 Delay consumer 使用同一 endpoint map `manager+0x620`、sentinel `+0x628` 和 mutex `+0x5d0`，取得节点 `+0x30` 的 flow；新建 flow 通过 slot1 初始化，随后把该对象传给 `0x9dfb0`。因此已经找到真实活动流的静态调用方。

`0x9dfb0` 调用 GetCurrentPadding，并读取打开流时 GetBufferSize 保存的实际帧数，获取 render buffer，准备八通道 scratch 并补零不足部分，调用 flow slot2=`0x9f750` 处理延迟，再进入 `0x9ebc0` 声道累加／交错和适用的 PCM 转换，最后调用 IAudioRenderClient ReleaseBuffer。正常声道分支持有 `manager+0x210` 共享锁，读取 setter 重建的同一 `+0x218` map，用节点 gain 乘输入 sample 并累加到输出；有序值限幅到 `[-1,1]`。缺 endpoint 映射时输出清零，test mode 则绕过 map 并复制至多前两个源通道。格式与 PCM 转换另见下述独立切片；完整上游 DSP 仍未闭合。

打开流的 `0x99d50` 在 IDA 中被错标为 RTC／void，但调用方检查 EAX，函数出口将 HRESULT 写入 EAX。原始 IID 和槽位分别核对 IAudioClient、IAudioClient3 与 IAudioRenderClient；实际缓冲帧数来自 GetBufferSize，独立于 flow 的 4800 buffer frames。`fillBuffer` 的 true 表示继续／重试循环：没有可用帧或部分 ReleaseBuffer 失败分支也可返回 true，不能将它作为提交成功或硬件应答。

正常停止 `0x9bd10` 设置共享停止标志、检查 self-join 与 join 失败，等待线程后移除线程所有权节点，但保留 flow map。worker 退出沿 `0x9bab0` 请求 Stop、释放两份 COM 引用、关闭 event，最终移除上游 ring reader。manager 析构 `0x972f0` 先停止并 join 各 worker，再经 `0x97280 → 0xa68f0 → slot0=0xa73a0` 销毁 flow。worker 在 map 锁外缓存原始节点，delay setter 持锁写 `+104`，处理函数读取该字段时未持同一锁；以上静态顺序不能证明不存在竞态。试听分支的停止与恢复边界另见下文；运行结果、完整异常路径、Rust 行为及实际 DLL／设备对照仍未验证。

### 试听请求、实际 WAV 与停止边界

同一收据的 `audition_lifecycle` 保留 42 个完整函数、10,601 条指令、68 处锚点、30 组直接绑定、2 张真实虚表、9 个数据区域及 281 个 artifact。46,431 字节 raw chunks 中有 62 字节未枚举为指令；原始块完整保留。独立核对器重读官方 PE、完整函数、FH4 引用、WAV 及容器成员，并拒绝全部 12 个篡改反例。

请求沿 `0x43d30 → 0x5e440` 识别 PlayDelayTestSignal／StopDelayTestSignal。Play 调 `0x99460`；Stop 在 dispatcher 内调用 `0x9bd10`。两个分支最终都构造空 std::string 返回，manager 不存在时也可无操作；空响应不确认文件准备、线程就绪、实际出声、停止或原值恢复。

所选 endpoint 的来源是 `0xa85e0 → 0xadb20 → 0x9c210`：音量与 mute 查询成功、端点自定义分类为 1 时更新 manager `+1296`。分类来自 render endpoint 的 PROPERTYKEY `{04c1d2d3-90f9-4f65-8e10-b19217440603}, pid2`，值为 1 时写 EndpointProperties `+64=1`；同 GUID 的 pid3 另有 fallback 分类 2。这不是 Windows ERole，也不表示默认端点角色。`0x93910` 设置 key，`0x95060` 读取并分类；该读取支没有先核 PROPVARIANT 的 vt。本节未带 `0x` 的对象偏移均为十进制字节。`+1216` 的 endpoint mutex 解锁后才构造／复制 `+1440` 的 StreamProperties，不能称 endpoint 与格式在同一锁内原子发布。

Play 在 `+664` 的线程所有者锁内使用 `+600` map，先请求旧 selected endpoint 的 worker 停止、join 并 erase，再将 `+1440` 的参数复制到 payload，通过 `0xadfd0 → 0x9d4a0` 启动同一 render worker。`_beginthreadex` 和线程入表之后，才设置 upstream `+246296` 与 manager `+1632` 的两个测试标志；线程可能先运行。该分支没有保存旧普通流或旧 StreamProperties 快照。

`delay_test_signal.wav` 已在同一核验通过的驱动 ZIP 内找到并逐成员对照：12,290,122 字节，IEEE float32、双声道、48 kHz、1,536,000 frames、32 秒，不能再记为缺失输入。`0x816a0` 读取模块相对文件并核对双通道、32 bits 和采样率；没有在此重采样。采样率比较的 COMISD／JBE 同时包含 unordered 路径，不能仅据伪代码写成显式拒绝 NaN。prepared `+88` 为假时，注入函数 `0x81df0` 保持目标缓冲不变；测试标志为真不证明文件已准备。

上游 worker `0x833f0` 在真实 driver frames 和 ring 容量满足条件时注入 WAV，随后仍经过上游 global EQ slot2 和共享 ring 写入。下游继续使用同一 flow 的 `0x9f750 → 0x87df0` 延迟；manager `+1632` 只让 `0x9ebc0` 绕过正常 ChannelMaps、复制至多前两通道，其余清零，不能概括为绕过全部 DSP。WAV cursor 在构造时置零、注入时按文件 frames 取模；Play／Stop 分支没有重置它。

Stop 对当前 selected endpoint 停止并 join render worker，返回后清除两个测试标志。停止／join 异常可阻止后续清标志；该分支没有重新调用普通 refresh `0x98710`、重建旧流、恢复旧参数或删除保留的 endpoint flow，也不停止常驻 upstream worker。独立 refresh 的触发与状态发布见下节。Play 的 catch funclet `0x2f4744` 由 runtime-function／unwind／FH4 原始引用绑定，只记录线程创建失败日志；完整压缩异常状态与所有失败恢复尚未验收。

manager 最后释放 upstream shared reference 的真实虚表 `0x382550` slot0 经 `0xabf80 → 0x824e0`；后者发停止 event 并 join upstream，之后才释放 sample vectors。该次序与先等待 render workers、后销毁 flow 分别核实。端点切换、并发修改、完整驱动／DSP、所有异常和 OS 句柄释放、实际听音／恢复、Rust 接线及官方实际功能对照仍保持未验证。

### 实际试听页面、宿主 FFI 与布尔结果

同一收据的 `frontend_audition_bridge` 核对 86 个完整 JS AST 所有者、61 个角色、23 处词法绑定、10 项导出，以及实际包装 DLL 的 22 个完整函数和 routing client 的 3 个完整函数；4,447 个 artifact 的精确集合由独立检查器重开原件核对，29 个篡改反例均被拒绝。所依赖 MW 核对 131 份原件及 2,299 份输出，UI 核对 526 份原件及 1,344 份输出；原捕获失败记录原样保留，`successful_full_capture=false`。选中所有者已核对不表示整包恢复完成。

当前默认 display mode 的实际挂载为 main lazy chunk70／module1405 → Ic(jc) → READY gate fc(vc) → nav id2 的 Gl → Ol。Ol 按钮修改本地 React bool，effect `[playing,comboEnabled]` 发送当前选择；禁用、依赖变化的 cleanup 和卸载都发送 false。组件不等待 DLL 结果，视觉停止与下表的返回完成点须分别测试。其他 display mode 仍在本片边界之外；BroadcastChannel 准入与任务状态另见下节。

| 边界 | 实际返回与消费者 |
| --- | --- |
| 页面按钮 | 本地 bool；不消费真实播放／停止应答 |
| MW Di | 解构内层 result，truthy 时先更新 runtime combo bool，再尝试写任务状态；结果对象的静态异常见下节，不能称任务已经完成 |
| SDK／宿主 | Play／Stop 声明为 `bool(string)`；`ffiPreloadAsync → FFIPreloadMain.callDLLAsync` 由 FFI callback 原样返回该值，正常包装为 `{result,reason:""}` |
| 3942 wrapper | SDK 存在且 client status=0 时转 true；异步执行器等待三秒，busy／超时可返回 false |
| routing client | Play／Stop 丢弃 sendRequest 的响应字符串，普通返回为整数 0；全局 gate 命中或 client 缺失也可返回 0 |
| 服务 | 所核 Play／Stop handler 返回空 std::string；是否出声、读回与恢复须另验 |

包装器身份由 installedResources 的 Common＋Synapse 集合选取：名称为 THXNativeDLL3942／RzNative_3942 且 usedBy 包含当前 PID，首项 filePath 拼到 `userDataDir/Apps/` 后初始化。实际原件为 `thxvadvirtualroutenative_3942_v1.0.8.0.dll`，其 loader 按同目录准确文件名加载 `razer_soma_pro_routing_client_1.2.1.59_2.dll`。Stop 沿 wrapper `0x795a0 → 0x39b50 → 0x1a8d0` 进入该 client 的 `0x1d0b0`；Play 另沿 `0x74df0 → 0x34d30 → 0x19a20`。资源选择、包装 bool 与 client 整数状态分别保留，不把 SDK 默认文件名当成当前调用目标。

wrapper 的 StreamStates callback `0x34410` 仅记录日志并检查 context／initialized，所核 Carol JS 事件消费者只处理音量。三秒等待超时未证明已排队任务被撤销；response disposer 的完整卸载调用也未闭合。上述返回链和选中 callback 没有普通流恢复动作，系统层独立 refresh 的触发见“停止后的状态发布与独立重建”；完整外部反馈、真实取消、恢复和 DLL／服务对照仍未验证。

### 试听命令准入、FIFO、取消与结果对象

`frontend_command_lifecycle` 另保留 39 个角色、90 个完整 AST 所有者、29 处词法绑定、15 项导出和 169 个 artifact；27 个定向篡改反例均被拒绝。本片只含 JS 静态证据，原生函数数为零。与前端桥按原件 SHA＋源码区间合并为 168 个不同 AST 所有者，artifact 交集身份一致，不重复扩充原生覆盖。

页面的 `mwBroadcastChannel` 包装器发送 `{msgName:USER_COMMAND,msgData:command}`，UI 与 MW 使用相同的 `usb_5426_<productId>_<URL containerId>_mw`。MW module42339 的 Ct 把消息交给 EventEmitter，USER_COMMAND 的 ri 等待 module19148 的 `_x` 准入过滤，再构造 module91217 的 V 事件，以当前 `Date.now()` 写入 timerTick，进入 module67617 的 event FIFO。过滤器仅吞掉此处规定的 tab-status／frequency 事件，试听可以通过；但 scheduler 的 initialized=false 时会丢弃试听，只有两个电池事件豁免，初始化标志不等于设备实测就绪。

状态机按录宏独占状态选择动作：未占用时执行 storage update 与 deviceAccess；占用时只执行 storage update，试听 maker 可仅改缓存而不调 DLL。maker closure 的 SUCCESS／FAIL 表示它自身结束，未等待已排入的 DLL 任务完成。so 把 `Di.bind(null,flag)` 放入 NORMAL 的 `audioFunction` FIFO，忽略返回的 task ID 或 -1；所核页面与 maker 没有保留当前试听的取消句柄。enqueue 失败不能当作已经执行。

module81988 的 E.run 创建 AbortController，并通过 exclusive Web Lock 执行 task closure。pending 取消可移除队列项，已等待锁或执行中的取消只发 abort；Di 仅在调用 DLL 前检查一次 signal，未核出调用中撤销或调用后的 abort／连接代次检查。页面 cleanup 的 false 是另一条普通 Stop 命令，可能被初始化 gate 丢弃、在录宏占用时只进缓存，或排在旧 Play 后面。不能把它解释为撤销先前 Play 或恢复音频的确认。

完整源码还存在结果对象不一致：E.run 传入 `{result:null,value:void 0,error:void 0}`，直接绑定的 Di 没有初始化 value，却在 aborted、false、success 与 catch 路径写 `o.value.status`。按该静态调用链，这些写入会解引用 undefined；truthy 分支在此之前已修改 runtime combo bool，catch 又尝试写同一字段。Ci 只检查 error，没有核对 Promise 拒绝或 value.status，因此绿色日志也不足以确认任务成功。这是静态源码异常，未执行厂商 JS，不能写成已复现的运行故障；全部动态调用方、断连与 shutdown 清理仍未穷尽。

### 页面 display、Profile 分支与直接 cleanup

`frontend_page_branches` 另核 118 个完整 AST owner、58 个角色、38 个绑定、22 个导出和 169 个 artifact，原生函数为零；36 个篡改反例均被拒绝。三份前端切片按原件 SHA＋完整源码区间合并为 249 个不同 AST owner。`isSensaDevice=true` 先把 displayMode 覆盖为 `sensaApp`；最终 `chromaApp`、`sensaApp`、默认分别选择 module2364、4555、1405。外层 To／po／go hooks 相同，但选定根的子树不同：Chroma 根挂 lighting module1481，Sensa 根挂触觉 `je`；这些选定根不代表其全部嵌套分支已穷尽。

导航只渲染当前 tab，共享 body wrapper 在 activeView 变化时滚动，没有在所选完整 owner 中保留隐藏的旧 tab 子树。离开 combo tab 会卸载 `Ol`，Surround 关闭时它仍挂载，非 READY 状态渲染 `bc`。`Ol` effect 只依赖本地 `playing` 和 `combo.isEnabled`，不依赖 profile 或 frontSpeaker；依赖变化前 cleanup 先发 false，禁用 effect 还会再发 false。关闭 Surround 的顺序是本地切换 → `setComboMode(false)` → reducer 广播 `ON_SET_THX_COMBO_MODE`；模式 runner 调用模式／EQ setter，不直接调用 `sJ8`，而 `Ol` 另发普通 Stop。

模式 runner 在 await 原生 mode 前已修改 runtime `isEnabled`，仅在 mode 内层结果为 truthy 后才按禁用状态写零 front／rear EQ；它也在未初始化 `value` 的情况下写 `value.status`，与试听 runner 有相同的静态结果形状异常，不能由 runtime 已变化推断任务完成。这只核所选两个 runner，不扩展为所有音量或 EQ runner 都有同一问题。

Profile 只有 GUID 改变才 dispatch，并广播 `ON_SWITCH_PROFILE`；`un` 恢复 delay、EQ、frontSpeaker、isEnabled、replicate，再发 `SET_COMBO_MODE_PROFILE`，不处理 test signal。MW 的所选 restore case 会修改 activeProfile／version 并调用通用恢复；完整 `Aa` 没有直接的 comboMode、delayTestSignal 或 sJ8 字面量，间接 feature restore 仍未穷尽。由于 `Ol` effect 没有 GUID／frontSpeaker 依赖，保持挂载且两个依赖不变时不能承诺切换 Profile 必然 Stop。

3942 当前产品配置的 `hasRearAudioEndpointDevice=true` 使 Carol-or-rear cleanup 分支适用。两条直接调用链均绕过 NORMAL audioFunction maker 队列，顺序与结果消费如下：

| 所选清理 owner／调用者 | 真实顺序与边界 |
| --- | --- |
| `ea`，更新清理 | await `setDeviceMode(0,0)` → await `sJ8(false)` → await `setDeviceMode(0,0)`；嵌套 catch 只记录 reset 失败 |
| `ta`，设备清理 | await `setDeviceMode(0,0)` → await `sJ8(false)`；同样不检查 Stop 返回的 `{result,reason}` |
| 断开与 exitApp | 调用设备清理但不 await；两处重连分支随后立即 `location.reload()` |
| runUpdate | await 清理后才移除 HID 并进入更新编排；本次没有执行这些动作 |

`sJ8` 会把原生异常装入 reason，所以上表中的 await 本身也不证明 Stop 成功。前置 mode 拒绝会跳过 Stop；同一外层 try 中更早的错误也可跳过后续 cleanup。`ta/ea` 完整 owner 未调用 cancelTask、cancelAllTasks、abort 或等待队列 drain；所选 `Zl` 清 runtime 标志、`z_` 卸 storage listener、`UJ` 卸 shutdown／suspend、`gy` 清隔离 storage、`cr` 注销硬件事件／设备。这里只界定实际选定体，不推断全部间接 helper 都没有清理。直接 Stop 与此前排队 Play 的完整顺序仍未建立，不能写成已复现的竞态。

SDK 的 beforeunload 调 terminate 不 await；terminate 以 container 字符串调用 DeviceTerminate，await 后忽略返回 bool、设置 isInit=false 并 return true。所选两个 owner 没有调用保存的 response disposer，也未直接停止试听；container 字符串与宿主 spread 的 ABI 闭包仍需单独核实。所选 event consumer 没有 isInit／连接代次 guard，Carol 仅消费音量事件；完整 observer、系统关闭、恢复、Rust 接线及实际 DLL／设备结果仍未验证。

### 3942 实际控件、CSS 与当前 Rust 差异

`frontend_controls_css_and_display_subtrees` 保留 87 个完整 AST owner、29 个角色、19 个实际 webpack 导出、10 个词法绑定，以及 4 份 CSS 中的 447 个完整块和 media 祖先，共 52 个 artifact；另核当前 3 份 Rust 文件的 8 个完整方法。28 个证据篡改反例全部被拒绝。这里记录所选 3942 控件及 display 子树，当前十种 Quick 编辑器另见灯光片；1392／1442、其他产品分支和真实设备调用继续逐项追踪。CSS 完整块不等于 computed style 或像素验收。

front speaker 列表先排除名称包含去空格、小写设备名的条目。剩余为空时，本地 Surround 置 false，开关与指针路径禁用并显示警告。开启且没有选择、列表非空时弹出选择层，临时 index 从 0 起；Confirm 使用条目，未选状态下 Cancel 会禁用本地 Surround。列表变化后，选中位置按保存的 frontSpeakerId 重建；不能把位置当作稳定身份或补造默认设备。

Time Alignment 与 EQ 内容层在 Surround／frontSpeakerId 条件不满足时通过 opacity `.3` 和 pointer-events 禁用，所选 `Pl/Rl` 调用点没有额外传 `isReadOnly`。纵向 `Rl` 在 keydown 对所有键执行 preventDefault，onChange 只更新本地整数，mouseup 仅在 baseline 与当前值不同时提交。源码 `setTimeout(l({...}),200)` 先同步求值 `l`，不能称为 200 ms 防抖。此结论只适用于 `Ol → Pl → Rl` 的延迟和 `$l → Vl(kl) → Pl → Rl` 的 EQ；音量 `ll → module7114` 是另一条横向滑条链。

教程 `Hn` 等待动态媒体导入后挂载 autoPlay／muted／loop 的 MP4，等待时有视频占位。导入失败只日志并清 loading，未清除先前 URL，也没有该 owner 内的错误呈现或代次／取消 guard；不能把静态源码路径写成已观察到的媒体竞态。选定弹层有关闭、Back、Next／Done，完整 owner 内没有 Escape、focus trap 或焦点恢复，不据此推断全局／宿主层同样缺失。

选定 CSS 保留 960px combo 重排、311×260 椅子图、520×163 扬声器选择层、300px 旋转纵向滑条及各禁用层。与当前 Rust 的首个明确差异为：教程使用 `img(...tutorial-N.webp)`；Delay／EQ 的 Release 路径没有对应相等值 gate，事件送达时可能重复 dispatch 未变化值；EQ 两列源 CSS 为 gap50px，Rust 为 `surface::css(20.)`。这三项分别是媒体行为、事件条件与声明几何差异，实际硬件流量、框架缩放／样式叠加和视觉结果尚未执行验收，不能把它们合并为整页测试结论。

Sensa 子树也保留实际条件：强度范围 0..6、默认 4；从禁用且零值开启时替换为 4，强度零和 enabled 的切换按所选 owner 分别执行。来源卡片 gain 为 1..6，正文点击排除 slider／input／link／button；指针标志由 pointerup 的零延迟 timer 重置，选定卡片没有 pointercancel。来源初始缺项时写两项 enabled／gain4 属于本地存储初始化；事件及 Redux 更新不能作为设备应答。Intro modal 自身没有焦点 trap，Sensa 根另有 Escape→parent closePopup，两个层次分别保留。

lighting 子树包含左侧 Brightness／Dim、右侧 Quick／Advanced 和 Switch-off；当前配置 `hideLightingIdle=false`，显示 idle checkbox 与 1..15 分钟滑条。Dim 的固定 1／3／5／10／15 选项是另一控件；选定配置无 BLE hardware-effect shortcut，不能套用可选分支。adjustment mode 会阻断部分动作并触发 prompt；Brightness 的零值有两次 setState callback，尚未执行框架批处理结果。Quick 的实际编辑器及颜色弹层见下节，真实设备写回与官方实际 DLL 结果另验。

### 3942 Quick 灯光、颜色草稿与实际消费者

`lighting_effects` 核对 145 个完整 AST owner、81 个角色、21 项导出、27 处词法绑定、8 份选定原件、203 个完整 CSS 块与 333 个 artifact，包含 59 条语义约束；禁用 collector 后独立复核通过，37 个篡改反例全部拒绝。六份前端片的完整 owner 去重为 514 个；layout／volume／lighting 三片自身去重为 308 个 owner、577 个 CSS 块。当前十项效果按实际列表顺序选择八个参数编辑器；另外两项按源码返回空参数内容，并非不支持该效果。

| 列表位置／效果 ID | 当前效果 | 实际参数内容 |
| --- | --- | --- |
| 0／11 | Ambient | screen：full／left／top／right／bottom |
| 1／12 | Audio Meter | colorBoost，当前分支不保留编辑器的旧参数对象 |
| 2／2 | Breathing | color1／color2／isRandom |
| 3／8 | Fire | default 分支返回 null |
| 4／3 | Spectrum | 当前未传 useOptionButtonGroup，返回 null |
| 5／7 | StarlightV2 | color1／color2／isRandom／duration（1／2／3） |
| 6／1 | Static | 只提交 color1，替换参数对象 |
| 7／19 | Tidal | 颜色／随机及 direction（out=1、in=0） |
| 8／4 | WaveWithSpeedV2 | direction（up=3、down=4）及 speed（1／2／3） |
| 9／13 | Wheel | direction（1／2） |

当前配置是 ACCESSORY／CHAIR，使用 StarlightV2 与 WaveWithSpeedV2。Wave 挂载时先匹配已有方向，否则选择首项 up=3，再立即提交 `updateEffect({direction,speed:existing||2})`；打开编辑器因此存在真实写入意图，不能实现为纯读取或标为硬件已确认。它的方向／速度控件未接其他效果所用的 adjustment-mode mouse guard。通用 U2A 默认、MW Profile 默认与设备观测分别保留：MW 默认 Wave 是 direction3／speed3／pattern0，通用默认 direction2 会被当前挂载逻辑正规化。缺选择分支将默认 effect ID4 传给按列表 index 取项的 changeEffect，对应 index4 的 Spectrum；本片未执行该分支，不把静态条件写成已复现故障。

Audio Meter 使用本地 `Be` stepper，范围 `.25..4`、步长 `.25`，不是音量的 module7114。未传 allowLiveUpdate，失焦时解析、限幅并向上对齐；Enter 与 Escape 都触发 blur 后提交，而不是取消。上下键／wheel 逐步修改，spinner 每 300 ms 重复并在 mouseup／leave 停止。点击后添加 window mousewheel，blur 移除；所选 unmount 只清 interval，未清 wheel listener。真实浏览器时序、焦点及父层行为仍未执行。

颜色下拉保留 40 个预设色、no-color 和 16 个自定义槽，`#111` 为自定义空槽标记；Static 隐藏 no-color。RGB／hex／canvas 先编辑 picker draft；Save 先保存自定义色，再 selectColor／onChange，Cancel 关闭 picker 并重新打开下拉。输入框 Enter／Escape 都会 blur、正规化并提交输入草稿，不等于 picker 的 Save／Cancel。删除已选自定义槽会保留 selectedColor，以 index−1 发出，不补替代颜色。picker 的 DOM／window／宿主订阅及 dropdown click listener 有对应清理；这不能替代全局 Escape、焦点恢复和窗口验收。

Quick 挂载读取同步内存、注册 listener 并请求缓存设置，按 Profile GUID 更新；卸载移除 listener，选定 Promise 和 1200 ms 动画 timer 没有保留取消／代次 guard。`SET_SELECTED_EFFECT → J1d` 先乐观更新，再以默认 region0 广播 `ON_SET_EFFECTS`。MW `Bi` 修改活动 Profile、关闭本地 Chroma user、更新 version 并持久化，随后发布 UI completed／runtime；该处 deviceAccess 仅可能排队 OLED，不是灯光帧写入。`52671.Pf` 经 serial 或 force gate 写 `synapse_3942`／可选 Chroma key；hash／clone／localStorage 成功仍只证明本机配置路径。

真实 Quick 消费者是 LE 对 `synapse_` storage 的监听：持 mutex 的 `onStorageChange_device` 解析上电 DeviceInfo、Profile 元数据和实际 rzDevice，再到 `applyRegionQuickEffect`／`applyCustomQuickEffect`。connectedDevices 监听另负责 driver／lighting／power／ports，不能把它当作 Quick 配置消费者。runtime gate 起初为 true，六处真实 Zl 调用在 cleanup／close／断开／reload／exit 将它关闭；没有据此证明全部时序安全。

LE 把当前 Wave direction3／4 转成 up／down，speed1／2／3 转成 15／25／35；Starlight duration1／2／3 转成 500／1700／2000。manifest3942/0 的 `syncHWEffectWithSWEffect=false`、`syncHWEffectWithSWEffectWhenExit=true`、`useUserQuickEffectOnExit=false`：syncHW 更新退出目标与 host configureDevicesDefaultData，随后软件 effect owner 设置 mode／custom frame 并调用 engine 的 add／effect／layer／enable，不能当作即时硬件 ACK 或原值快照。Advanced 只选择已有本地 Chroma Profile 并启动 Studio，有 installed／empty／list 等分支，不包含 Studio 编辑器；远程安装分支不进入本项目离线产品路径。完整 native 计算／帧传输／应答／读回／退出写入及恢复释放、其他产品／BLE 分支、Rust 接线和实际 DLL／设备结果仍未闭合。

### 3942 横向音量控件与真实结果边界

`frontend_horizontal_volume_controls` 另核 103 个完整 AST owner、60 个角色、21 项导出、18 个词法绑定、9 份选定原件、58 个完整 CSS 块和 168 个 artifact；同时保留当前 2 份 Rust 文件的 8 个完整方法。禁用 collector 后复核通过，35 个证据篡改反例全部拒绝。包含灯光效果的六份前端片按原件 SHA＋完整区间去重后共 514 个 AST owner，重叠部分完整记录一致；不能把各片数量相加。

`ll → module7114` 的三条横向滑条均为 0..100、step1，主音量延时 50 ms，front／rear 为 120 ms。拖动 Change 清除并替换真实延时回调 `() => changeValue(...)`；非鼠标 Change 立即调用。window mouseup 在 mouseIsDown 时再次提交当前值，不检查 baseline，也不清除待执行延时器。主音量的 `ll` 调用方会过滤与其当前 Redux 值相同的值，front／rear 调用方没有这一 gate；这只证明回调与排队条件，尚未证明实际硬件发生重复写入。

7114 只阻止 End、Home 和方向键，不阻止全部 keydown；完整类没有自身 Escape 取消、blur 提交或 pointercancel。active 通过 CSS opacity／pointer-events 控制，input 的原生 disabled 只取 disabledInput，而 `ll` 未传该参数。外部 value 变化会覆盖本地 slider value，只有可选 cancelMouseUpOnValueChange 才取消拖动和 timer，`ll` 同样未启用。Sensa `Ee` 另传 disabledInput、范围为 1..6，不能混同这两组调用。

卸载仅移除 window mouseup 和 observer，所选类未清 debounce timer，也未保存／取消几何重试的 requestAnimationFrame；真实迟到回调尚未执行。module1107 的 focus／tab visibility 只驱动可见性和几何更新，不等同 DOM 键盘焦点移动或恢复；浏览器、宿主及 GPUI 的完整焦点行为另验。

页面由 CHANGE_VOLUME／SET_THX_COMBO_VOLUME 乐观更新 Redux，再发送主音量或前后声道事件。主音量 runner 在 setter 前后检查 abort，但不解释 setter 结果就标完成；当前 Carol 分支依次设置 global mute，并仅在 enabled 时设置 global volume，最后更新 runtime，没有独立 getter 确认。front／rear runner 只在 setter 前查 abort，要求内层 result truthy 后把请求值写入 runtime，完成消费者仅日志；不应以这份缓存冒充设备读回。SDK 的百分比参数除以 100，getter 结果乘 100 再 round，bool mute 单列。

初始化和 Ready 的当前 rear-endpoint 分支都先 await rear getter，再 await front getter，随后按 front→rear 发布；UI reducer 只消费 channel 的 value，丢弃 isEnabled。主音量是另一组分支：Ready 优先 runtime.renderVolume，否则 `olx(true)` 读系统 endpoint；已初始化 THX 的 `olx(false)` 才依次读 global mute、global volume。getter catch 可发布初值或部分读取对象，项目必须保持实测、未观测和缓存的区分，不能照搬为读取失败后的默认成功。

Carol 只处理 `Razer-ThxVolume-Changed`，拒绝 endpointIdType／isMuted／level 为 null 的事件。每次事件重新创建 `sg(callback,100)`，其 `K` helper 每次分配新的 timer map，所以只证明逐事件延时 100 ms，不能说成跨事件按 endpoint 合并。延时回调总会读 global volume；只有 vad 分支要求读取值与事件 level 换算后相同，front／rear 无此 gate。所选回调没有本层连接代次或取消 guard，真实顺序与迟到结果未验。

当前 Rust 保留 Master Change50 ms、front／rear120 ms 和 Release 立即提交，但 `submit_surround_slider` 缺少官方主音量调用方的相同值过滤；其停用路径会清 timer／身份，而官方所选 unmount 未清 timer。Shell 的主音量通过 role1 endpoint `AudioVolumeRead/Write` 并检查真实读回，与官方 global／forced-endpoint／runtime 分支是否结果等价仍未实测；源码差异不能直接证明设备错误或正确，也没有由本收据接受 Rust、UI 或实际 DLL 功能。

### 停止后的状态发布与独立重建

`endpoint_refresh` 切片另核 62 个完整函数、12,024 条指令、12 张虚表、19 个数据区、84 处锚点及 20 组直接绑定，共 327 个 artifact。完整 12,253 函数汇编的 header 和三组直接调用者清单也在收据中；计算得到的间接目标不宣称穷尽。18 个证据篡改反例均被独立检查器拒绝。

`0x9bd10` 仅在找到线程项、必要的 join 成功且 erase 完成后，经 `0x9c0b0 → 0xa9b70` 发布 GLOBAL StreamStates 的 STOPPED=2；无线程提前返回或 join 异常不走这条成功发布链。通知沿 `0xae280 → 0x60be0` 入队，由真实任务虚表 `0x380b68` 的 slot2=`0x71b40` 进入 `0x669c0 → 0x600e0 → 0x5b7f0`，最后通过 ZMQ PUB 发布。它不直接调用普通流重建，STOPPED 也不是恢复确认。

普通 refresh `0x98710` 的已核直接调用点为 WasapiManagerState observer 的 `0xa3fb9` 和用户在场处理的 `0x45761`。后续独立 refresh 即使拿到相同 properties，也会在现有状态为 STOPPED=2 或 FAILED=3 时重建；其他状态可移除待重建项。用户在场 GUID 已与留存 SDK 声明和 PE 原字节核对，实际为 `GUID_GLOBAL_USER_PRESENCE`（`786e8a1d-b427-4344-9207-09e70bdcbea9`），不是 SESSION 版本。manager `+1635` 按状态变化去重，重复相同通知不保证再次 refresh。外部通知是否形成完整反馈环、实际调度顺序和最终出声恢复仍未实测。

### VAD 驱动帧与上游缓冲

`vad_upstream_and_driver` 切片分别核对服务 53 个完整函数和驱动全部 194 个捕获函数，合计 16,606 条指令、14 张表、64 处锚点、41 组直接绑定及 368 个 artifact。服务与驱动使用各自原件 SHA＋RVA，不能按裸 RVA 合并。驱动完整捕获与字节检查只证明所选输入完整，194 个函数的全部 PortCls／系统语义尚未逐项还原。45 个证据篡改反例均被拒绝。

服务 `0x833f0` 打开 `\\.\ThxVad_soma_pro`。驱动入口经 `0x4900 → 0x492c → 0x12000` 将 `0x12d0` 注册为 MajorFunction[14]；专用 IOCTL 分支为 `0xcf546006/600b/600c/6017/601b`。服务发送的 flush `0xcf546013` 落入默认 PcDispatchIrp／PortCls 分派，不能补造专用 handler，也不能仅由静态分派判为固定成功或不支持。event 创建为空，以及 enable／disable 的 adapter 不存在或内部失败，均存在仍完成 IRP 成功的分支；该返回不能代替实际流状态读回。

get-frame `0xcf54600c` 接收 4 字节输入，输出 245,784 字节：24 字节小端 u32 头依次为 channels、sample rate、通知帧数、读取前 available、实际 copied、flags，随后是八路 planar i32，每路最多 7,680 帧，stride=30,720 字节。内核每通道 ring 容量为 38,400 帧；读取使用 `min(available,7680)` 并推进 reader，flags 随读取清零。内核 overrun 使用 bit2=`0x4`，服务停止条件检查 bit0=`0x1`，不能把溢出标志直接当成服务停止。PCM producer 的跨字节／跨通道部分输入、wrap-copy 和 format reset 分别保留原实现条件。

服务把 i32 按原始常量 `0x30000000`（`2^-31`）转 float，float／double 包装器按 CPU 能力选择实现；普通 48 kHz ring 初始化请求 4,801 帧、实际容量 4,802 帧，八通道并按 16 帧对齐。interpolated reader 的 available 为 `max(base_fill-3,0)`；其虚表 slot0=`0x6c70` 只返回零，真正 add-reader 是 `0x82b10 → 0x84a30`。不能用虚表槽名替代实际调用关系。

GlobalEQ 的实际 prepare 槽 `0x16d90` 是 no-op，处理槽 `0x856d0 → 0x88cf0` 另行消费动态 filters。重采样为八个独立单通道 SOXR 对象，capacity 按 `target/input × max(block,7680)+32`，recipe=3、flags=`0xA`、phase=50、passband=0.881、stopband=1.089；运行参数为 threads=1、DFT 8／8、coef_size_kbytes=400、flags=0。处理错误仅记录日志并继续其余通道，返回最后通道的 output count，不校验通道间一致，也不返回内部 ring 实际夹限后的数量。环境、分派与预热由下节独立切片继续核对。

正常 upstream 析构先发停止 event、join，再 disable 和销毁重采样／ring／DSP；driver unload 与 stream destructor 的 event、timer、DPC 和 buffer 清理有独立路径。服务 event／frame／flush 失败的提前返回可能绕过 worker 正常 CloseHandle；完整 driver handle、shutdown event、VirtualUnlock 和调度注册恢复尚未闭合，不能从正常析构推断全部失败均释放。

### 重采样分派、环境、预热与 FIFO

`resampler_dispatch_priming_fifo` 独立重开原件核对 59 个完整函数、60 个 chunks、6,907 条指令、63 个数据区、5 张分发表、10 个实际槽调用、55 处指令锚点、12 处环境绑定和 39 组直接调用，共 153 个 artifact。28,382 字节 raw chunks 中 33 字节未枚举为指令；286 条直接边、41 处间接／导入调用和 203 条尚未展开的直接边均原样保留。80 个有条件静态模型及 59 个篡改反例只核实所列控制与计数，未计算音频样本或执行官方 DLL。

当前 VAD 的 recipe3／flags10 经 `0x1f59f0` 得到 precision16 与 effective flags=`0x8000000a`，`0x1f4d20` 默认选择 `0x3d5240` 的 cr32s，32 位默认分支不执行 CPUID。`SOXR_USE_SIMD` 优先于 SIMD32／SIMD64；变量存在但为空或无数字时解析为零，仍覆盖后者。整数解析允许前置空白与符号，遇非数字停止，int32 溢出饱和至 INT_MIN／INT_MAX；非零含负值均作为 true。64 位分支在没有环境覆盖时才检查 CPUID ECX 的 `0x18000000` 和 XGETBV 的 `0x6`；显式环境值可绕过该检查。quality bit `0x20` 另直接选择 vr32，不能以默认路径替代该分支。

| 环境覆盖 | 接受范围；超范围保留已有字段 |
| --- | --- |
| SOXR_MIN_DFT_SIZE／SOXR_LARGE_DFT_SIZE | 8–15／8–20 |
| SOXR_COEFS_SIZE／SOXR_NUM_THREADS | 100–800／0–64 |
| SOXR_COEF_INTERP | 0–3，flags 低两位 |
| SOXR_STRICT_BUF／SOXR_NOSMALLINTOPT | 0–1，flags 的 bit2／bit3 |

构造后的 `0x89b50` 保存 63 项精确采样率对；八乘八组合唯一缺项为 22050→96000，缺项跳过预热，不能补成零项。每条记录 24 字节，只解释 f64 input、f64 output 与 i32 frames 的前 20 字节。收集器从完整伪代码赋值取值，独立检查器从 40 个原字节 literal load 与 154 个 stack store 重建 1,260 字段字节，两者一致。固定输出 48000 Hz 的预热输入帧数如下，其余采样率对完整保留在收据 `priming.entries`。

| 输入 Hz | 22050 | 24000 | 44100 | 48000 | 88200 | 96000 | 176400 | 192000 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 预热输入帧 | 95 | 80 | 94 | 0 | 68 | 64 | 386 | 151 |

表中 count=0 的现有项仍调用 process；输入是每通道非空的零 float32，外部 output=NULL／capacity=0 只跳过外部读，内部 ring 中产生的样本和延迟状态仍保留。因此对应官方初块必须从同样的 create＋预热状态比较，不能用裸 SOXR 的首块作为期望。

实际 planar 路由依次调用对象槽 +176=`0x213730` 输入 FIFO、+256=`0x1f6570` 转换、适用时 +200=`0x212150` 单次 flush，再经 +184=`0x213870` 阶段处理、+192=`0x2137b0` 输出 FIFO、+264=`0x1f70f0` 转换。当前单通道 float32 转换使用 memmove；阶段记录为 208 字节。输出受 requested、available 及 flush balance 限制，推进 cursor 和累计计数；flush 只更新一次 balance，累计输入先转 signed32 再按 ratio 计算目标，不能擅自扩为无限精度计数。负 input count 的补码与全接收后才 flush、input_done 存在时的 ceil 限制分别在静态模型中保留。

正常释放 `0x1f5280 → 0x1f52a0 → slot+208=0x212040` 清阶段、FFT／系数缓冲、各 FIFO 和对象，但不会自动 drain 尾样本。默认 FFT／stage 的已选算法边界见下节；非默认转换、全部分配失败、CRT locale、线程交错与实际清理仍未验收，完整独立 Rust 算法和官方实际音频结果对照继续保持未验证。

### 默认 FFT、滤波器设计与系数布局

`resampler_fft_and_stage` 再核 46 个完整函数、46 个 chunks、10,598 条指令、45 个数据区、20 处 FFT 槽调用、1 处 provider 分配槽、63 处锚点、8 处机器级 ABI 校正和 39 组直接调用，共 81 个 artifact。42,328 字节 raw chunks 中保留 13 字节对齐间隙；121 条直接边、21 处间接／导入调用及 84 条未展开边没有隐藏。49 个条件静态模型与 49 个篡改反例通过，仅证明所选结构、局部算式与证据完整性。

| 当前 cr32s 的真实表 | 布局与用途 |
| --- | --- |
| provider `0x3205f0` | 80 字节；`+72` 指 FFT 表，`+64` 指 polyphase 描述表，`+24/+32` 指 halfband 表及数量；flags=14、标量 4 字节、系数分组宽度 4 |
| FFT `0x3d5a30` | 128 字节、16 槽，15 个有效入口与一个 NULL；不是相邻下一张表的一部分 |
| polyphase `[0x3201c0,0x3205e8)` | 19×56 字节；每行保留 beta、taps、三种插值成本／内核入口及实际 padding |
| halfband `0x320170` | 2×40 字节，实际内核 `0x1fe800/0x1fe950`，中心 0.5 与八／九个对称侧系数 |

FFT 的构造槽经 `0x214df0 → 0x215fb0(N,0)` 创建 96 字节 real plan；析构释放 backing allocation 后释放 plan。real／complex factor 候选顺序分别为 `[4,2,3,5,0]`／`[5,3,4,2,0]`，实际当前 stage 选二次幂长度，执行内核对应 radix2／4，不能据候选表宣称任意长度 FFT 已验证。twiddle 角先按 double 计算 `(-2π)*j*k/N`，再转 float32，随后调用 CRT sin／cos helper；默认 cr32s 创建分支不查 CPUID，不代表这些数学 helper 的 CPU 分派或误差已经闭合。

stage designer `0x213f40` 选择的 FFT 指数为 `min(max(min_dft,trunc(log2(fir_length)+2.77)),max(large_dft,trunc(log2(fir_length)+1.77)))`。默认 min／large 均为 8，实际长度仍随 FIR 长度变化，不是恒定 256。phase=50 时 group delay 为 FIR length／2；其他 phase 依赖未展开的 `0x1fac30`。滤波器缩放按 callback_scale×`1/N`×插值因子×incoming gain，double FIR 转 float32 后写入 `(N-fir_length+1+i)&(N-1)`，并保留清零区域。

processor `0x2139e0` 从 FIFO 取样、适用时插零，经 forward FFT、对应布局的复数乘法、inverse FFT，丢弃 FIR length−1 并 decimate。内部布局乘法会另外覆盖 output[0] 与 output[4] 的实数乘积；interleaved 路径还在 SIMD 循环后计算 `output[1]=output[N]*b[N]-output[N+1]*b[N+1]`。求值次序、原值与别名条件都须保留，不能直接用通用复数乘法替代并宣称等价。scratch flags=3，两个缓冲分别为 `N*4` 与 `2*N*4` 字节。

`0x2144b0` 将 taps 向上补齐到 4 的倍数，分配 `phases*padded_taps*(order+1)*4` 字节清零存储；分配入口是 provider+8=`0x214d10`，不是 FFT 表的 +8。tap／phase 均逆序遍历，`tap_offset=padded_taps-t-1`，base scalar index 为 `(p*padded_taps+(tap_offset&~3))*(order+1)+(tap_offset&3)`，多项式常数至三次项按间隔 4 分组存放。初始 seed 取未乘 gain 的 FIR[0]，后续值才按实际条件乘 gain，padding 也不能按普通前缀填充推断。实际内核 `0x1feae0/0x1fef70/0x200290/0x2012b0` 分别执行直接、线性、二次、三次系数计算；选行和成本分支完整保留在收据。

低通设计 `0x1fa640` 使用对称 Kaiser 窗 sinc。令 `M=max(factor,1)`、`B=min((Fs/|Fn|-Fp/|Fn|)/(2M),Fs/(2M|Fn|))`、`Fc=Fs/(M|Fn|)-B`，则距中心 x 的 sinc 为 `sin(π*x*Fc)/(π*x)`，x=0 取 Fc；窗为 `I0(beta*sqrt(1-(x/(half_length+rho))²))*M/I0(beta)`。rho 在 M=1 时取 0.5，其余按 attenuation≥120 取 0.75，否则 0.63；Fn<0 只估长度，不分配 FIR。beta／长度估算 `0x1fb9a0` 的多项式、分段与对齐保留实际数据和字节，异常输入及 CRT 舍入未计入数值等价。

`0x20e330` 的三个分段表对应 modified Bessel I0，边界为 |x|=8.5／12.5。八个有限点分别用原 PE 系数的 Horner 模型和独立 85 位 Decimal 级数核对，只用于辨认该数学函数；49 个模型中的其余项核 DFT 选择、factorization、地址与局部插值。所有模型均未执行官方函数、未计算整条音频样本。8 项 ABI 校正来自寄存器／栈实参，属于内部调用关系，不是可直接调用的导出签名。

非 50 phase、fallback time interpolation、其他 provider、所有采样率／质量／环境／别名／flush／分配失败组合及 CRT CPU 数学分支尚未穷尽；实际音频数值、独立 Rust 实现、服务接线、取消与运行验收仍未验证。

### GLOBAL EQ 的动态滤波器与快照

`global_eq_filter_and_snapshots` 保留 59 个完整函数、7,571 条指令、11 张真实虚表、15 个 PE 数据区、11 处构造绑定、43 处字节锚点和 36 组直接调用，共 201 个 artifact；24 个定向篡改反例均被独立核对器拒绝。完整 EqType 描述符、24 个 switch 目标及普通有限输入的系数公式分别保存在该 section 的 `eq_type_enum`、`coefficient_cases` 和 `finite_input_equations`，原始函数与未枚举的 121 字节也保留。

通知进入 `0xa4d90 → 0xa9720` 后按 endpoint ID 的长度和 memcmp 精确匹配 current VAD id：匹配项进入 upstream 的八声道、每声道十段 EQ；其他端点另走四段 flow，后者算法不计入本片完成范围。成功注册端点且分类为 1 后，`0x9c210` 在 `0x9c9e1 → 0x981d0` 重读 GLOBAL EqState。current VAD id 为空会跳过初读；更新只遍历实际存在的 bands，没有先清空缺失 bands，也没有证明所有 VAD 重建都会清空旧段。

`0xacea0` 将每个 EqBand 转为下列 64 字节参数，逐声道懒创建 `0xa6390` Biquad。真实构造绑定虚表 `0x382ce8`，slot1=`0xa1d50` 负责处理，slot2=`0x16d90` 是 no-op。64 字节参数随后连同六个 double 系数组成 112 字节快照。

| 参数偏移 | 字段与当前 GLOBAL 来源 |
| --- | --- |
| 0 | i32 filter type，来自 EqBand enum |
| 8／16／24 | f64 frequency／gain_db／q，由三个 f32 字段扩展 |
| 32／40 | frequency2=1000、q2=0.7071067811865476 |
| 48／56 | sample rate=48000、enabled=true |
| 64／72／80 | 输出 a0／a1／a2 |
| 88／96／104 | 输出 b0／b1／b2 |

| EqType 数值 | 完整分支 |
| --- | --- |
| 0–2 | PEAK、LOW_SHELF、HIGH_SHELF |
| 3–7 | HIGH_PASS、LOW_PASS、NOTCH、BAND_PASS、ALL_PASS |
| 8–9 | LOW_PASS_1P、HIGH_PASS_1P |
| 10–15 | PB_SECOND_LOWPASS_FQ、PB_SECOND_HIGHPASS_FQ、PB_FIRST_LOWPASS_F、PB_FIRST_HIGHPASS_F、PB_SECOND_ALLPASS_FQ、PB_FIRST_ALLPASS_F |
| 16–19 | PB_SECOND_PEAK_FQG、PB_SECOND_DIP_FQG、PB_GAIN、PB_INVERT |
| 20–23 | PB_SECOND_BASS_SHELF_FQ_FQ、PB_SECOND_TREBLE_SHELF_FQ_FQ、PB_FIRST_BASS_SHELF_F_F、PB_FIRST_TREBLE_SHELF_F_F |

系数器 `0xa2140` 没有统一验证 frequency／q／rate 的有效范围。frequency2 非正或 unordered 时取 frequency，q2 非正或 unordered 时取其默认值；a0 为正／负零时跳过归一，其他值含 unordered 执行除法并置 a0=1。未知 filter type 不重新初始化系数尾部，结果可能来自已复用的 pool 对象，不能给未知值补造恒等滤波器。band index 的实际 gate 将低 32 位符号扩展后与 10 比较，只拒绝 signed≥10，不能称为完整 u32 范围检查。

PB_GAIN 的 b0 实际取 `P(10,gain_db/20)` 返回值，PB_INVERT 取其负值；P 通过 CRT `0x23c900` 分派。反编译文本里复用的 10 是实参，不能当成固定增益。两个实际数学实现及条件跳转均保留，但完整 CRT 误差、特殊值、异常和 CPU 分支尚未证明数值等价；代数公式也不替代逐指令求值顺序。

实际 kernel 为 DF2T：`y=b0*x+z1`，`z1=b1*x-a1*y+z2`，`z2=b2*x-a2*y`。八路 planar binary64 按 active metadata 的 index 和顺序处理十个槽，kernel 不做限幅。每次处理先排空已到达的系数快照并保留最后成功取出的快照；历史 z1／z2 仅构造时归零，更新系数不清历史。函数保存 MXCSR，设置 `0x8040`（FTZ＋DAZ），处理完恢复，四 sample 展开与尾部处理均保留。

系数快照与 5,760 字节 cascade metadata 分别入队：先逐个发布八路系数，再发布 metadata，并非一个原子事务。两种 setter 都忽略 enqueue 返回，`setBiquad` 普通返回 1 不能确认八路音频已应用。pool 初始五个对象，耗尽时请求扩为当前 vector 的两倍并重试一次；该重试结果也未检查。控制侧更新、处理侧当前快照与已出声音频必须分开比较。

正常清理先停止并 join upstream，再销毁 8×10 动态 filters、metadata owner；filter 释放系数 owner、changed flag 和 mutex。快照 owner 依次清 pending queue、当前 shared reference、pool mutex、free queue 与 pool objects；最后引用释放会回收到 owner 的 free queue。分配／锁异常不在 setter 转成业务失败；已选 C++ 展开和部分发布由下节继续核对，全部队列交错与跨线程静止性不能由正常释放链推定。

### EQ 入队失败、部分发布与异常释放

`global_eq_exception_lifecycle` 独立核对 46 个完整函数、2,586 条指令、30 个数据区、11 个 runtime-function 根、9 个 FH4 根、29 个 unwind 状态、119 个调用点生命周期及 8 个完整指令范围，共 119 个 artifact。其中 20 个函数来自上一片，26 个新增函数的 1,182 字节原始块另有 2 字节未枚举为指令；两片合并为 85 个不同函数。42 个定向篡改反例全部拒绝。异常结构由实际 decoder `0x2211c8` 及 PE 压缩表独立解码，不以未登记的 SDK header 作为协议证据。

`0xae440` 的 false 路径均在搬移输入 shared_ptr pair 之前；成功才在 `0xae5f0..0xae601` 搬移并清空输入，然后在 `0xae60c` 发布 tail。入队失败不消费调用方 pair，但不能推论队列内部所有字段未改。`0xa69d0` 在正常失败后释放本地 shared_ptr；最后强引用经 `0xaef10 → 0xa6e30` 尝试归还 free queue，归还失败也未检查。pool vector 仍拥有 raw 对象，所以不能写成“入队失败立即 free 系数对象”。pool 耗尽后的第二次 dequeue 结果仍未核，不能补造安全回退。

处理侧 Biquad 的 current 位于 `+1040/+1048`，cascade owner 的 current 位于 `+720/+728`，仅在成功 dequeue 且 raw 非空时替换；失败保留 current。发布侧此前可能已裁剪 pending，随后发布失败不会恢复这些待处理项。因此“当前快照仍在”与“全部待处理状态保持原样”须分开验证。

`0xacea0` 先改八声道的 control metadata，再逐声道构造／设系数／置 changed，最后才调用 `0xae620` 发布 metadata。某声道 enqueue=false 不中止后续正常迭代；中途 C++ 异常则跳过后续声道与最后 metadata 发布，没有整组回滚。已 active 的 band 可留下声道间新旧系数并存；未 active 的 band 仍可能被旧 metadata 阻止处理。最后 metadata 发布失败也不回滚此前的八次系数发布。这是静态调用路径允许的状态，不宣称已枚举所有线程交错。

所选七个构造／setter／扩容函数的 FH4 header=`0x28`，没有 TryBlockMap；unwind 负责资源释放，没有对应业务恢复。以 `0xa69d0` 为例，控制块分配期由 `0xaef60` 的 borrowed-object guard 归还对象，系数计算／入队期间由 `0x9f90` 释放 shared_ptr。扩容异常释放未转交的当前临时对象并解锁，之前成功转交给 vector 的扩容项保留。半构造 filter 依次释放 changed flag、mutex、`0x698` allocation；owner 按已初始化状态清 pending／current／mutex／free queue／vector。所列调用点映射不表示每个 callee 必然会抛异常；全部 CRT／队列内部异常、SEH／访问异常、外层服务 catch 和所有 shutdown 恢复仍未闭合，官方实际结果及 Rust 功能验收保持未验证。

### 非 VAD 端点四段 EQ 与默认高通

`endpoint_nonvad_four_band_flow` 核对 67 个完整函数、8,992 条指令、10 张虚表、13 个数据区域、11 个构造绑定、4 处实际间接调用和 50 条直接绑定，精确保留 185 个 artifact。36,817 字节 raw chunks 中的 137 字节没有枚举为指令；包括原样保留的 jump table 与填充，不能补称为已反编译指令。独立复核在禁用 collector 后仍通过，44 个篡改反例全部拒绝；19 个有界静态模型只验证已提取语义的条件，不是 Rust 功能测试或实际 DLL 计算结果。

`0xa9720` 按 endpoint ID 的完整字节长度和 memcmp 分流：匹配 VAD endpoint 的路径使用上游 GLOBAL 八通道十段 cascade；不匹配时在 manager 的 endpoint map 中取得或创建 flow，经 `flow+80 → 0xa13d0` 修改该端点的八通道四段 cascade。初始化仍为 48 kHz、8 通道、4800 buffer frames；只遍历报文实际包含的 bands，遗漏项不清除旧条目，空列表也不会删除构造期高通。不能把“非 VAD”理解为跳过处理，或把十段限制套到这一支。

`0x9fb00` 分配 `0xf78` 字节 cascade，先创建 32 个 metadata 条目及 32 个空 filter 指针。参数结构的 `enabled=true` 与 metadata 的 `active=false` 是不同字段。随后 `0x9febe` 真实调用 `0xa13d0(index=3)`，尝试向八路发布 20 Hz、HIGH_PASS=3、gain=0、Q≈0.7071、配置采样率的高通，返回值未检查；这不是只有常量而没有调用，也不是音频已经处理的确认。后续请求可覆盖 band3。配置虚表 slot1=`0x16d90` 本身为空操作，不能用该槽推断整次初始化没有作用。

setter 把 index 的低 32 位符号扩展，只比较有符号 `<4`；没有独立的负数拒绝，不能因此称负索引可安全使用。每个参数依次复制到八个 metadata 条目，active 取 enabled，按需构造 filter 并分别发布系数，最后发布整份 metadata。队列／producer／enqueue 的结果没有形成八路事务或处理确认；中途失败可能保留先前已修改和已发布的部分，正常返回 1 也不等于消费完成。metadata 初始池为 5 个 `0x900` 字节对象；发布前会裁剪 pending，消费侧排空并保留最后成功取得的快照，没有新快照则保留 current。归还对象的 shared owner 经 `0xaef90 → 0xa6f10` 回到 free queue；全部队列交错仍未实测。

真实处理链为 `0x9dfb0 → flow slot2=0x9f750 → cascade slot2=0xa17b0 → 0xa6730 → Biquad slot1=0xa1d50`。flow 在 ring／delay 后将八路 float32 转 double，按每路四个 metadata 的 active 与 index 选择非空 filter，逐段原位处理，再转回 float32。这里的转换和 cascade 没有显式缩放／限幅；后续 `0x9ebc0` 的混音限幅是另一边界。共享 Biquad 使用既有系数构造与 DF2T 历史，系数更新不自动清历史；其 MXCSR 保存、设置 `0x8040`、恢复与转换器使用的调用方浮点环境分别记录。

正常释放已追到 flow 的 ring／scratch／cascade、四段 filter 向量、metadata pending／current／mutex／free queue／原始对象池。所选四段专用 FH4 与失败释放见下一片；全部分配及队列内部失败、跨线程静止、CPU 探测分支与全部浮点环境的数值等价仍需继续核对；本片没有执行官方程序、DLL、设备或当前 Rust。

### 非 VAD 四段发布的异常与对象所有权

`endpoint_nonvad_exception_lifecycle` 独立核对 84 个完整函数、4,306 条指令、89 个原始数据区域、18 个 runtime-function 根、12 个 FH4 根、41 个 unwind 条目、155 个调用点和 12 个完整指令范围，共 157 个 artifact。16,230 字节 raw chunks 中有 15 字节未枚举为指令；其中 47 个函数相对基础片新增，原始块合计 3,000 字节。基础脚本前缀及基础收据原字节保持不变，禁用 collector 后复核通过，71 个证据篡改反例全部拒绝。

`0x9fb00` 依次把 float scratch、ring 和 cascade 装入 flow，再在 `0x9febe` 发布构造期高通。该调用、随后配置虚槽和 double scratch 配置在本函数的 EH state 均为 `-1`，没有本层业务回滚；资源已在 flow 字段中，不能据此断言泄漏。外层 flow 所有者、全部嵌套分配及 shutdown 的异常消费者仍须逐段核实。

`0xa13d0` 先更新八个控制条目，再逐声道尝试系数发布，八次正常迭代结束后才发布 metadata。中间入队 false 不会终止后续系数尝试；后续异常没有本函数 C++ 回滚，先前已发布系数不会恢复。metadata 失败也不恢复系数或已裁剪的 pending。9 个有界静态模型仅在明确假定先前调用成功的条件下核对这些分支，不模拟音频线程或全部队列调度。

`0xa6520` 的借用 guard `0xaefe0 → 0xa6f10` 和 shared-pair guard `0x9f90` 分别归还借用、释放本地引用；它们不恢复参数或 pending。第二次 pool dequeue 的结果未检查，不能补称一定安全；回收链 `0xaef90 → 0xa6f10 → 0xacaf0` 不检查 enqueue 结果。原始对象仍归池的 vector 所有，归还失败不能说成已立即释放。消费侧 `0xa6730` 没有新 pending 时保留 current，发布失败不自动清空 current。

`0xa79d0` 扩池时由 mutex guard `0xc5b0` 和 raw-object guard `0xa7e10` 管理尚未转移的对象；对象先进入所有权 vector，再尝试加入 free queue。后续异常只释放尚未转移的对象，不删除此前成功扩入的对象；free queue 入队 false 也不立即释放 vector 中的对象。`0xa72a0` 的 FH4 header 实为 `0x68`，含 `0x221eb8 → 0x234b48` 的 terminate 动作；不能套用其他析构的 header 或声称所有析构异常都恢复成功。

所选 metadata 正常释放仍依次为 pending、current shared pair、mutex、free queue、raw vector；flow 与 cascade 的 filter／scratch 释放分别保留。析构链存在不证明并发音频使用已静止；全部 CRT／队列异常、SEH、跨线程交错、Rust 功能测试和官方实际功能结果继续保持未验证。

`endpoint_nonvad_outer_owner_exception` 进一步核对外层所有权与实际 catch 范围：88 个完整函数、7,530 条指令、123 个原字节区域、25 个 runtime 根、20 个 FH4 根、112 个 unwind 条目、5 个 try block／5 个 handler、605 个调用点、14 个指令范围、17 处直接调用绑定及 283 个 artifact。32,588 字节 raw chunks 中有 55 字节未枚举为指令；相对前两片新增 70 个函数、27,304 原始字节。禁用全部 collector 与其 decoder 后独立复核通过，96 个篡改反例全部拒绝；7 个条件静态模型不替代异常注入或官方实际执行。

`0xa58b0` 的所有权依次经过 caller 临时指针、待插 node guard、manager map：`0xa5980` 清 caller 临时指针，`0xa5984` 把 flow 存入 node+48；rehash 失败时已构造 node 的 guard 经 `0xa6ca0` 释放 flow 和节点，构造前的 raw node 只经 `0x4fd20` 释放。map 链接完成后 guard 结束，随后三处首次配置 `0x9d933/0xa936e/0xa9a39` 均通过 slot1 调 `0x9fb00`，caller 只剩 mutex／引用 guard，没有 erase 已转入 map 的 flow。配置异常不能改写为自动回滚，也不能仅凭局部无回滚就断言泄漏。

`0x9d4a0` 的实际 `std::exception` catch 只覆盖 state13 的 render 调用，不覆盖 state4 的首次配置。handler `0x2f4cd1` 记录 what，continuation `0x9ddd0` 释放当前 stream，再按 stop 状态决定重试，不回滚 map。线程 wrapper `0xadfd0` 的 FH4 header 为 `0x68`，但 worker 调用 `0xae056` 的 state 为 −1，不能把表中的 terminate action 当作该点的活动清理链；另核 CRT 的 header `0x40` 检查与 `0x224647 → 0x234b48 → abort`，越出该 noexcept 边界的 C++ 异常不能记作普通恢复。全部 CRT／SEH／外来异常路径仍未穷尽。

正常 shutdown 先 stop／join stream threads，再经 `0x975ce → 0x97280 → 0xa68f0 → 0xa73a0` 销毁 flow map；单个 stream 停止不删除 map flow，join 失败仍可抛异常，不能将正常顺序推广为全部异常清理完成。通知队列先解锁再调用 task，所选 dispatcher／queue／wrapper 没有业务 catch 或 flow 回滚；全部外层通知异常消费者仍未闭合。`0x97810` 的六组 registrar 与独立的 `0x97eb0 → 0xa85e0 → 0xadb20 → 0x9c210 → 0x981d0` endpoint-vector 链分别核实，`0x98087` 的调用属于 `0x97eb0`，不能因地址相邻而混为一个函数。初始化的全部异常消费者、并发静止性、Rust／UI 接线和实际 DLL／设备结果继续保持未验证。

### WASAPI 格式来源、打开失败与 PCM 字节

同一收据的 `wasapi_format_and_pcm` 另核 22 个完整函数、7,864 条指令、139 处指令锚点、26 处 COM 调用、17 组直接绑定和 12 个数据区域，共 154 个 artifact。34,342 字节 raw chunks 中有 17 字节未枚举为指令。此切片同时追踪格式的生产者和真实打开／转换消费者，不根据反编译的函数名或浮点十进制显示推定 ABI 与数值。

GetMixFormat 实际位于端点属性填充 `0x95060`，调用 IAudioClient slot8；不在 `0x99d50` 打开函数内。成功且非空才处理真实格式：标准格式扩成 cbSize=22、valid bits=bits 的 40 字节结构，1／2／6／8 通道分别设 mask=4／3／1551／1599，其余为 0；extensible 原样复制 40 字节。已接受的结构保存在 EndpointProperties `+144`，经 `0x9c210 → 0x96410` 复制到 StreamProperties `+24`，再由 `0xb3d60` 复制给线程。GetMixFormat 失败或空指针可返回构造初值，成功但 encoding=5 则释放并返回空属性；这些分支不能登记为真实格式已观测。

打开函数复制这 40 字节并强制 sample rate=48000、average bytes/sec=`nBlockAlign*48000`；它没有再次按系统混音格式替换，也不采用 IsFormatSupported 给出的 closest format。短于 40 字节的输入在此消费者没有长度检查，生产者与 IPC 边界须另验。

`0x99d50` 要求 IAudioClient3 查询成功，失败不会转为普通 IAudioClient 继续。SetClientProperties 的结果丢弃；IsFormatSupported、GetSharedModeEnginePeriod 和 GetDevicePeriod 用于日志或诊断，不因这些调用失败而中止。实际初始化仍调用 IAudioClient::Initialize slot3，share mode=0、duration=periodicity=0、session GUID=NULL；首轮 flags=`0x88040000`，重试改为 `0x88000000` 并保留同一 48 kHz 候选格式，分别进入 event 与 polling 路径。不能将它写成 InitializeSharedAudioStream 或 closest-format 协商成功。

初始化后依序设置 event（适用时）、GetBufferSize、GetService、GetCurrentPadding、GetBuffer、ReleaseBuffer(SILENT)、Start，最后分配并替换 float scratch。创建 event 失败或实际 buffer frames 为零时，错误通知可携带前一成功调用留下的非负 HRESULT；单看返回码不足以证明上下文可用。padding 减法没有在此核出防下溢门，scratch 的 channels×frames 使用 32 位乘积，分配可在 Start 之后抛异常。fill loop 的 `std::exception` catch 不覆盖此前的 open／Start／分配；不能由正常停止链推定全部异常均清理完成。

| encoding 值 | 当前分类与输出 |
| --- | --- |
| 0 | IEEE float32，交错 float 输出 |
| 1 | PCM16，转换后的低 16 位，小端 |
| 2 | packed PCM24，依次写低三字节，小端 |
| 3 | PCM24 左移 8 位后写 32 位；仅 extensible PCM、wBitsPerSample=24 且**总 nBlockAlign=4**进入此类，不能改成每通道对齐判定 |
| 4 | PCM32，完整 32 位，小端 |
| 5 | 未知；仍可保存到已初始化的上下文，fill 不进行映射／转换却仍可能 ReleaseBuffer，不能显示格式已支持 |

extensible 要求 cbSize 至少 22，并按实际 SubFormat GUID 与 wBitsPerSample 分类；不读取 valid bits。普通 PCM24 即使 nBlockAlign=4 仍为 2；extensible PCM32／valid bits=24 仍为 4。20 个分类静态用例分别保留这些分支，不用常见 WAVEFORMAT 约定覆盖原件的具体谓词。

PCM helper `0x965f0/0x96620/0x96650/0x96680` 先按有序比较限幅，再以 binary32 乘 32767／8388607／**2147483648**，使用 CVTTSS2SI 向零截断。PCM32 的实际常量字节为 `00 00 00 4f`，反编译显示的 `2147483600.0` 不是精确常量；`+1.0` 乘 `2^31` 后溢出，在 invalid 被屏蔽时输出 `0x80000000`，不能改为 `0x7fffffff` 饱和并称与原件一致。NaN 在同一条件下分别写出 PCM16=`00 00`、PCM24=`00 00 00`、移位 PCM24=`00 00 00 00`、PCM32=`00 00 00 80`。Rust 的普通饱和浮点转整数 cast 不足以复现这条指令语义。

14 个数值向量限定于 binary32 乘法采用 round-to-nearest、invalid 被屏蔽的静态模型；原函数没有统一 MXCSR，其他舍入、异常屏蔽、FTZ／DAZ 与 signaling NaN 尚须实际对照。30 个证据篡改反例全部被独立核对器拒绝。这些分类、向量与反例验证静态分析记录，未执行官方代码，也不代表 Rust 行为测试、真实 COM／音频输出、恢复或实际 DLL／服务功能比较已通过。

### 当前 Rust 协议接线与差异

当前项目已有独立的 [surround 协议](../../../crates/razer-device/src/audio/surround/protocol.rs)、[Any 所有者](../../../crates/razer-device/src/audio/surround/wire.rs)、[服务 worker](../../../crates/razer-service/src/runtime/windows/audio/surround/worker.rs)及 [Shell 调用方](../../../crates/razer-shell/src/shell/audio/surround.rs)。静态逐项比较确认 36 个消息的 115 个字段名称、tag、类型和 repeated／optional 标记，以及两个枚举的值与本描述符一致；VirtualRoute 的请求／发布地址也一致，生产传输使用独立 Rust ZeroMQ。提交从已观测状态构造，不自行递增 revision；收到真实响应后核对消息类型、id、所选可编辑字段和旧 revision，保留实际返回状态，再处理取消。ChannelMaps 的提交比较只检查 endpoint_channel_maps，没有比较 schema 中的 vector_clock；官方 `0x63470` 的差异比较只排除 state_header，因此不能把两者称为完整有效载荷一致。上述回应确认的是服务协议状态，不能提升为 WASAPI／硬件已应用。

已经找到两处具体的接收差异。官方 `0xe98e0` 只要求 type_url 以 `/完整消息名` 结尾，Rust `Payload::decode` 要求固定 `type.googleapis.com/thx_wasapi_routing_proto.` 前缀，因此替换前缀的合法同名 Any 接受范围不同；标准编码前缀本身一致。更直接的缺口是 `VadChannelVolumesState`：Rust 已有其协议 struct，但 Payload、decode 和 [Observations](../../../crates/razer-device/src/audio/surround/observed.rs)没有消费分支。官方启动 `0x5d050 → 0x66680` 实际注册此 GLOBAL 状态，API6 匹配的 ClientConnect 随后遍历注册状态并经 `0x7d570` 发布；它不是仅存在于描述符的未使用声明。该消息送达 Rust 时会被当作未知类型：初始化接收会失败，连接后的接收错误会标为断开。尚未实测发布顺序或实际投递，不能声称每次连接必然失败；修复范围应是只读通知与状态消费，不能擅自给它增加官方 16 请求表中没有的写接口。

服务重启后的 revision 还存在条件性恢复缺口。例如客户端保留 revision10，服务只返回 revision1，官方 `0x63220` 明确保留并返回服务当前状态；Rust 将其判为 Stale，不接纳低 revision，调用分支只发布 failure，不清 connected，后续低 revision 通知也不触发重连。如果传输一直可用，这条路径尚无服务重启代次重建入口；传输报错后的正常重连不能证明此条件已覆盖。实际重启场景未执行。已有的页面 Identity／代次拒绝、替换前等待旧 owner、独立取消清理和 Shutdown 后 socket／join 结果应分别保留，不能误报为全无生命周期处理，也不能把外部客户端关闭等同服务进程析构。

本轮没有修改或运行这些 Rust 实现。后续仍须逐项测试 16 个请求、revision 回退与 vector_clock、API 不匹配 ack、畸形 inner、完整初始状态发布、通知顺序、数据库失败、取消／释放和六组真实消费者，并继续追完整 DSP、CTC／Virtualizer、stream 重建及设备应答。纯／模拟测试、官方实际 DLL／服务对照和窗口／设备验收分别保持未验证。

| 待补差异测试 | 独立依据与必须区分的结果 |
| --- | --- |
| Any 前缀与消息名 | 相同完整消息名分别使用标准和其他 URL 前缀，按 `0xe98e0` 比较接受范围；未知消息名仍单列 |
| 完整初始发布 | API6 后包含 GLOBAL VadChannelVolumesState，分别在 ready 前后送达；核接收、观察状态与连接结果，不增加该状态的写请求 |
| 服务 revision 回退 | 传输继续可用时，以已观测 revision10 接收服务 revision1；与传输报错后的新 Client 重连分别验证，保留真实低 revision 和旧连接身份 |
| ChannelMaps 确认 | maps 相同但 vector_clock 不同，分别核官方差异比较、Rust 所选字段确认及实际返回状态，不把 clock 差异直接当成硬件失败 |
| 取消与停止 | 在真实响应到达前后取消，核状态保留、独立恢复、socket 关闭及 join；空试听响应只能证明所核服务协议结果 |
| 试听结果与任务调度 | 分别核页面本地 bool、MW 内层 result、wrapper bool、client status 和服务空响应，覆盖准入、录宏占用、enqueue=-1、FIFO、三秒超时、迟到执行及停止／恢复；结果 value／status／error 与绿色日志分开核对 |
| VAD 帧和驱动状态 | 按独立布局测试 24 字节头、八路 planar i32、部分输入、wrap、available／copied、两个不同 flags mask，以及 flush 通用分派与 IRP 成功后的真实状态 |
| GLOBAL EQ | 逐项覆盖 24 类系数、未知类型、signed band gate、缺 band、系数更新后的历史和 FTZ／DAZ；注入第 N 声道分配／入队及末尾 metadata 失败，区分 active／inactive、current 与 pending 裁剪、borrow guard／recycle 失败及释放；普通代数一致不算数值或实际应用确认 |
| 重采样 | 保留采样率、预热、环境和 CPU 条件，比较各通道实际 consumed／output、错误、flush／drain、重建及清理；包装器返回的最后通道计数单独记录 |
| CTC／Virtualizer | 六支分别覆盖 variant／key／RTTI、typed read false／保留配置、inner malformed、revision/diff、保存失败继续、当前状态 publication 与析构 stop/join；区分 owner 空和底层 DB 空，socket 返回 1／单位 FIR／静态通知不能代替耐久、DSP／设备应答或实际 DLL／服务对照 |
| client Virtualizer／FIR 产品分支 | 1392 的导出缺失、1442 的活动路径与 3942 的正常返回 11 分别测试；1442 getter 覆盖状态 4／1／5／0、容量恰好等于 JSON 长度、NUL 与 32 位长度／size_t 边界，FIR 分别记录内层结果与回调。3942 核返回值、未写输出与未调用结果回调，不能将相同错误码或所选桩函数推广为服务／硬件一致 |
| 数据库文件边界 | 分别覆盖 memtable／磁盘读取、checksum／cache／snapshot、五参 Put 与三参 batch 编码；零选项仍写日志，非零才额外同步。注入日志／同步失败，核 memtable 是否更新、同步错误是否保留及后续写拒绝；区分显式 close 与最后 shared 引用析构，真实文件／耐久及官方结果另验 |
| 非 VAD 四段 EQ | 分别覆盖 endpoint 精确分流、band3 构造期高通、enabled／active、四段索引门、遗漏 bands、八路部分发布、metadata 最后快照和有状态处理；注入第 N 路异常、metadata／pool 入队 false，核 guard、已转 vector 所有权与 pending/current 不回滚；分别验证 node guard／map 转移、state13 render catch 与未覆盖的首次配置、noexcept／join 失败，不能据正常清理推断异常恢复 |
| 3942 控件／教程 | 纵向 Delay／EQ 的未变值 mouseup、key prevention、同步提交与横向音量分别测试；动态 MP4／失败／迟到结果、选择弹层取消、CSS 几何和焦点层次另核，Rust WebP／Release／gap 声明差异不因静态收据通过而视为修复 |
| 3942 横向音量 | 分别测试 50／120 ms、mouseup 保留 timer、主音量／channel 相同值过滤、外来 value／卸载／取消、逐事件 100 ms 与 vad gate；核 Ready 缓存／forced endpoint／global 初读及 rear→front 读取顺序，比较 Rust role1/readback 的实际结果，不能将 runtime 更新当确认 |
| 3942 灯光效果 | 十项 ID／列表位置与八编辑器逐项核，区分 UI／MW 默认和设备观测；覆盖 Wave 挂载提交、Meter blur／Enter／Escape、自定义色 Save／Cancel／删除、Profile／串行 storage／退出。分别比较 region 参数、LE 软件状态、实际 native 帧与 ACK／读回；本机 cache、completed 或退出目标不能替代设备结果 |
| 页面分支与 cleanup | 覆盖 display 根选择、nav 卸载、Surround cleanup、Profile 恢复、mode 拒绝、Stop 排队与 reload；分别记录 UI 本地 bool、服务状态、wrapper/client 返回和实际设备读回 |

## 演示与来源

1352 的本地演示不循环、不带自定义 controls、不浮动；1392/1442/3942 循环、有 controls 且可浮动。四款都使用系统默认 render endpoint、不静音、不归 profile 所有。关闭播放、全屏和浮动状态属于媒体生命周期，不能顺便改设备 EQ。完整原件、CSS 与媒体收据见 `audio-demo-audit.json`。

本页深层证据分别见 `audio-mw-audit.json` 的 `audio-local-profiles`、`audio-MapAudio-audit.json` 的 `seiren_eq/leviathan`、`audio-help-reset-audit.json` 的完整 EQ/reset 所有者，以及 `audio-surround-audit.json`。本文描述当前原件的静态语义；官方包装器 ABI、设备应答和本项目独立实现状态分别核验。
