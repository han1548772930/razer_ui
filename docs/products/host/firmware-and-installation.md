# 本地固件与安装生命周期

本机固件操作使用已在本地的固件文件、产品配置和真实设备传输；安装生命周期管理本程序的文件、驱动/服务动作、共享依赖、版本记录与清理。文件存在、哈希匹配、准备成功、设备传输完成和安装记录发布各有独立的成功条件。

## 当前源码和原生所有者

安装行为来自 [installer/index.325f8759.js](../../../.analysis/inputs/frontend/installer-current/raw/apps.razer.com/installer/assets/index.325f8759.js)及其[获取收据](../../../.analysis/inputs/frontend/installer-current/acquisition.json)。固件本机动作和事件消费者来自 [update-fw/main.69cc5fbd.js](../../../.analysis/inputs/frontend/synapse-update-fw-current/raw/apps.razer.com/synapse/update-fw/static/js/main.69cc5fbd.js)及其[获取收据](../../../.analysis/inputs/frontend/synapse-update-fw-current/acquisition.json)。下列前端区间均指各自原件的 UTF-8 字节。

| 所有者 | 原件字节区间 | 本地职责 |
| --- | --- | --- |
| 固件包装器 `rzFWUpdate` | `[543895,549620)` | 原版调用声明、事件注册、SDK 初始化与退出 |
| 固件 API `Wr` | `[1867558,1878953)` | 设备/连接分支、本机文件准备、开始传输、失败恢复 |
| 固件事件管理器 `Br` | `[1860436,1866222)` | 当前设备事件关联、进度、完成和错误 |
| 安装文件核对/发布 `Rr/Xa` | `[134646,137251)` | 实际本机文件清单、内容写入、hash 核对 |
| 本机安装动作 `Ka` | `[137252,139690)` | 本地可执行文件/驱动/服务动作与结果 |
| 安装执行 `yl/$l` | `[181450,182795)` / `[183691,184260)` | 优先级、并发、取消、错误、重启与安装记录 |
| 卸载 `Bl/Dr` | `[192589,193737)` / `[194321,195353)` | 产品版本、共享使用者、真实删除与待清理记录 |

对应原生原件为 [RzFWSDKWrapper_v1.0.3.0.dll](../../../.analysis/inputs/host-current/blobs/0c6137afee7b08c1ebf18eaa37cd99db350a0765f3f5abc318aabb18c5ac6e76/RzFWSDKWrapper_v1.0.3.0.dll) 的[静态捕获](../../../.analysis/results/native/0c6137afee7b08c1ebf18eaa37cd99db350a0765f3f5abc318aabb18c5ac6e76/capture.json)，以及 [FirmwareUpdaterSDK_v1.0.1.031300.dll](../../../.analysis/inputs/host-current/blobs/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/FirmwareUpdaterSDK_v1.0.1.031300.dll) 的[静态捕获](../../../.analysis/results/native/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/capture.json)。原版的业务 DLL 加载和导出转发只用来确定独立实现的边界，不能进入本项目产品后端。

## 固件目标和准备

固件 API 读取当前设备的 productId、vendorId、serialNumber、连接方式、supportUsb/supportDongle、现有版本和目标版本。普通分支与 Windows UMA 分支选择不同的原版包装器。USB、dongle 和 BLE 的产品身份不能互换；缺少真实连接身份或必要版本时，本次操作失败，不能从本地记录补成已确认读数。

标准准备 `[1869071,1870963)` 先核对目标版本和 API 初始化状态，按连接方式选真实 PID，再读取已缓存的本机产品配置。提交内容是 `pid/vid/SN/targetVersion/deviceConfig`，deviceConfig 在通信边界序列化。返回必须能解析，且 `code===SUCCESS`；缺配置、异常、无效 JSON 或其他 code 均不算准备成功。此时还没有证明设备已经更新。

### 本地 ZIP、文件哈希与提取消费者

UMA 原版 Er 的联网获取排除在本项目产品之外；其保存后的 `pr [1852455,1854265)` 是本机文件准备链，须独立还原。pr 调 simpleGetUserApps、解析数组后仅取第一个对象的首个 value，按原 JS 的 OS 分隔符精确匹配 `folderName/PID/target.zip`，不做通用路径归一。它取 Common 清单首个 name 严格等于 RzPowerTool 的项，以其 version 拼成 EXE 名，而不使用该项 filePath；存在实际安装文件、版本候选和本轮读取的分析原件分别核实，不能混用。

有 expected SHA 时，前端将两边转大写再比较；缺 expected SHA 仍可启动 helper。实际 `--fwupdate-unzip PID/version.zip` 参数却再次从 lr().device 读取当前 PID／target，可能与此前保存／匹配的 ZIP 不同。pr 仅用 `result && exitCode===0` 判断，不读取 stdout。hash 不匹配或 launch 正常结算后均尝试删 ZIP，忽略删除 false／reason，catch 只日志；缺 helper 提前返回，JSON／launch 抛异常没有 finally，可留下 ZIP。ZIP 被删也不代表提取物回滚。

当前 preload 的 simpleServiceAction 转给 Ze.callDLL，主机 [simple_service/win/index.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/simple_service/win/index.js) 的 Get／Add／Remove／Launch 分别保留完整字段所有者。Get callback 为 `void(bool,string,string)`，生成 result／reason／userApps；Remove 为 `void(bool,string)`，Launch 为 `void(bool,string,int)`。cbMap 持有 callback 到 promise 结算，没有这条链的 timeout／cancel；前端 Get 只取 userApps，Remove 只取 result，pr 连该 result 也未核实。3010 的特定驱动通知匹配不涵盖 RzPowerTool，不能当作本次解包成功。

当前宿主 simple_service.dll 的实际本机链为：

| 行为 | 原生所有者与结果 |
| --- | --- |
| Apps 目录 | `0x114e → PathService key112 → provider 0xe5870 → SHGetFolderPathW(CSIDL28)`，再由 `0x96ed0` 连续追加 Razer／RazerAppEngine／User Data／Apps；四次调用是 append，有原汇编 r8 字符串参数，不能按反编译漏参误写为 DirName |
| 实际清单／SHA | `GetUserApps 0x10290 → main表+0x40 → 0x2a50 → 0x2c30 → 0x21dd4 → 0x21fb0 → 0x26caa` 枚举真实目录；`0x1bd28` 流式读取 1024 B，SHA-256 输出为每字节两位大写十六进制。打开失败或负值读取错误清空 hash；`0x7af70` 将 ERROR_HANDLE_EOF=38 转为零、按正常 EOF 完成，其余读取错误返回 -1，不把已读前缀作为成功 hash |
| 删除 | `RemoveUserAppFile 0x11a50 → main表+0x60 → 0x3490 → 0x3660 → 0x22ca8 → 0x22e70 → 0x27d72` 重新计算当前文件 hash、逐字节精确匹配传入 hash，匹配才经 `0x7b8e0/0x7b8f0` 非递归 DeleteFileW；成功再发 NotifyUserAppFileRemoved，失败有 reason |

目录常规属于当前进程用户 LOCALAPPDATA；PathService 有 cache／override／provider 链，不能用字符串推断另一用户或当前设备状态。真实 getter／删除两槽、SHA hasher 完整四槽、SHA 初值与 provider header 均对原 PE 核字节；provider header 不是虚表函数槽。

已保留的版本化 [RzPowerTool_v1.0.18.0.exe](../../../.analysis/inputs/host-current/blobs/fc826508a70f990df16a1eb4f77aa2f63c35f7d380bca5b256a40609d61cf175/RzPowerTool_v1.0.18.0.exe) 是该版本行为依据，不能与宿主无版本 helper 混用，也不证明当前 Common 查询一定选到此版。其 `WinMain 0x36200 → 0x38c40` 按 `/` 切出非空 PID／ZIP，在当前用户 LOCALAPPDATA 的 `Razer\RazerAppEngine\User Data\Apps\Common\fwupdate\PID` 找 ZIP；找不到才经 `0x3a790` 读取活动用户环境，WTS token 失败再经 `0x3a550` 查活动会话 explorer.exe token，选中链有 token／环境／snapshot 清理。helper 与主服务用户目录分支不能合并为同一固定身份。

提取 `0x30c30 → 0x30400` 直接拼接目标根、反斜杠和原 ZIP entry 名，再建父目录；只提取无扩展／单点或忽略大小写的 `.md/.txt/.config/.haps/.chroma/.hex/.bin`，其他项只日志跳过。ZIP metadata、read<0、entry／archive close 错误没有汇总成失败；open 成功并正常走完循环即 true，含零 entry，但异常仍可能中断。若目标路径已是非目录文件，会删该文件并建目录后返回 false，该分支不继续解包。没有已证的路径范围检查、取消、rollback 或部分输出删除；旧文件和部分结果可保留。entry CRC 与下层 ZIP 全库语义也不能由选中 owner 推定闭合。

WinMain 将 helper bool 转为退出码 0／1，没有补做逐文件确认；pr 看到 0 不能证明全部文件提取成功。独立 `--fwupdate-delete` 没有在本链自动调用，删除 ZIP 和清理提取目录是两件事。后续纯／模拟及实际 DLL／本地文件对照须覆盖缺 SHA、hash 不匹配、查询与运行间设备切换、callback 未结算、删除 false、活动用户身份、零 entry／被过滤 entry／read与close失败／已存同名文件／部分输出；helper 和安装器本轮均未执行。

SDK `initSDK` RVA `0x1c0d0` 解析本地 `localResourceRootPath` 与 log 配置，执行字符串/JSON 校验。`prepareUpdate` RVA `0x1c8d0` 的真实本机链为：

1. `0xf7c0` 解析产品身份、序列号、目标版本和 deviceConfig；后者可以是 JSON 字符串或对象。
2. `0x13280` 从 releaseNote 记录选择最高版本，不按调用方 targetVersion 筛选；读取该记录的 `resourceSha256`，形成本地 ZIP 路径。
3. `0x1a850 → 0x39790 → 0x4e090` 计算 SHA-256 并转大写十六进制，按长度和实际字符精确比较；不匹配返回 code 2001。文件打开失败产生空串的边界见下文。
4. `0x1ae30` 解压本地归档；其返回 false 才映射 2002，多种内部提取错误并未使其返回 false。
5. 打开选中版本目录的 `deviceConfigInfo.json`，由 `0x11320` 解析 VID/PID、reportId、reportLength、deviceType、协议和各 IC 的 mcuIndex、targetVersion、updateType、fwFileName、delay、bcdPid 等字段；文件打不开返回 2003。
6. 准备函数在上述文件读取路径返回 code 1，但未检查 `0x11320` 的 bool 返回；该解析器 catch 后可以返回 0。因此原件的 code 1 连“全部 IC 配置有效”都不能独立证明，更不能代替后续版本读取、IC 选择、传输应答或最终事件。异常路径的返回与错误记录也须单独保留。

原版配置键实际拼写包含 `supportProrocol`，在外部格式边界按原件解析；内部仍应使用具名类型。配置中固件包 hash 属于实际文件完整性约束，与逆向原件的证据 SHA 各有用途。产品不得拿工作站路径或证据文件 hash 判断设备兼容性。

### 固件 ZIP 哈希的实际 SHA-256 生命周期

SDK 的 `0x1a850` 是 `CalculateZipFileHash` 的完整 owner（[原始函数捕获](../../../.analysis/results/native/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/functions/1a850.json)，函数文件 SHA-256=`297019f20a835ea5c52b2be88905a770d4d22af0933c0a41067f3dffcb7031e8`）。它以 `ifstream` 的 binary 输入模式打开传入路径，逐字节 `sgetc/sbumpc` 读到动态缓冲；open 失败直接把输出 string 清空并记录日志，读取错误没有独立的 `ferror` 结果。正常结束后把完整字节范围交给 `0x39790`，而不是按 ZIP entry 或解压后内容计算。

`0x39790` 先调用 `0x4e090`，后者的完整函数（[哈希边界捕获](../../../.analysis/results/native/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/functions/4e090.json)，函数文件 SHA-256=`377d80a0129de539abbc031a2d906204131dc1716755c2311d65735abbe5d083`）按 32-bit word 累积长度，处理 64 B 分块并使用标准 SHA-256 八个初始 word；压缩轮在 `0x4d6d0`（[原始轮函数](../../../.analysis/results/native/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/functions/4d6d0.json)）使用 64 个常量、`ROTR`/`Ch`/`Maj`，末尾 `0x1a6c0` 加 `0x80`、零填充到 56 B 并写入大端 bit length。PE 数据区 `0x7adc0` 的 288 B 初始状态／轮常量原字节 SHA-256=`d736d450a7808c38481216f2dbe3d6caaa0bc79d620d9f4033d7ea0be0e6dd42`；输出按大端 word 拆成最多 32 B。这闭合了当前 SDK 文件 hash 的算法身份，但不等同于固件镜像的设备认证或加密校验。

`0x39790` 将 32 B digest 逐字节用宽度 2、填充字符 `'0'` 的十六进制流格式化，再由 `0x1a850` 对整个结果调用 `toupper`，所以该 owner 的实际比较串是 64 个大写 ASCII 十六进制字符。读失败的空串仍会进入同一比较；expected `resourceSha256` 没有在此路径独立规范化，因而两边均为空的静态边界不能当作文件完整性通过。哈希上下文在 `0xd72d4` 分配、`0xd7350` 清零并释放，异常路径由 `ifstream`／动态缓冲析构处理；未执行 DLL 或文件行为对照。

比较发生在 SDK `prepareUpdate` `0x1c8d0`（[完整函数捕获](../../../.analysis/results/native/e204e2e1099d5948050c3aab23589b70bf8872928ba452bb2d9be36b00fdd572/functions/1c8d0.json)，函数文件 SHA-256=`65ec8e4039db25266cdc128e4b77602759339c8d7069cdb3875cc286febf6768`）：先把两边 string 的 data 指针按 small-string／堆存储布局取出，再比较 `size`；长度相等且为非零时才调用 `memcmp`，任一字节差异返回 `{"code":2001,"message":"File hash value mismatch."}`，零长度不会调用 `memcmp` 而直接继续。这个 owner 只证明文件 hash 字符串的字节相等，不证明 ZIP 可提取、配置可解析、设备身份或下层写入成功；未执行 DLL 或文件行为对照。

### 资源版本、哈希与 ZIP 提取

`0xf7c0` 按存在字段依次写顶层 `pid/vid/SN/targetVersion/deviceConfig`，缺失保留旧全局，后续异常不撤回。deviceConfig 接受 string 或 object→JSON string，其他类型先转空串再解析而失败。其 `productName/vid/mainPid/minSdkVersion/deviceType/configModifyTime` 同样没有必填检查；minSdkVersion 是严格 string，但所读准备链没有据它执行 SDK 版本约束。deviceType、configModifyTime 分别使用下文的 u16／u64 转换。

releaseNote 存在时先销毁旧列表；array 直接遍历，string 仅解析一次，直接 object 经 dump→parse 仍是 object、遍历 values，其他直接类型变为空数组。每条记录为 232 B：version、resourceSha256、minBaseFWVersion 三个 string，releaseDate u64，随后 targetICVersions、engineerNote、en、cn 四个 string。multiLanguage 遍历容器 values，仅从 object value 读取可选严格 string 的 en／cn，各自保留最后一次出现的值；直接 `{"en":"a","cn":"x"}` 反而只遍历两个 string，不填语言字段。解析错误保留已提交全局及前缀记录。

`0x13280` 复制列表并保留严格较大的版本，版本相等保留首项，空列表返回全空默认记录。比较 `0xb870 → 0xb620 → 0x2aa20` 按点分段、以 Windows signed32 `strtol(base10)` 解析并逐段有符号比较，缺段补零；`1` 与 `1.0` 相等，数字后缀未检查，`1x` 可当作 1。无数字／ERANGE 抛异常，不能把此比较描述为严格版本格式验证。

路径中的 base 来自 init 的 localResourceRootPath，PID 来自顶层 pid、以十进制转换：ZIP 为 `base\<pid>\<selected.version>.zip`，提取目标为 `base\<pid>`，配置为 `base\<pid>\<selected.version>\deviceConfigInfo.json`。这里没有使用 deviceConfig.mainPid 或 targetVersion 选路径；`0x1f5a0` 是十进制格式化，`0x37da0` 是字符串追加，不是路径规范化。

哈希计算成功时产生 64 个大写十六进制字符，但 resourceSha256 本身不做大小写／格式规范化，也不独立要求 64 字符。文件打开失败返回空串，两端都空时可越过精确比较；读取也未进一步区分正常 EOF 和 I/O 错误。因此比较通过不能单独证明输入文件实际可读且完整。

ZIP 的常规 false 出口仅见 archive 打开或 global-info 失败。entry-info、open-current、output-open 失败仅记日志并增加 index，且绕过 go-next，可能重复处理当前项；read<0 只日志，写入状态不检查，输出 close 失败仅设 failbit，go-next 失败可提前结束仍返回 true。`0x648e0` 的 entry CRC 错误 `-105` 也在两处消费者被忽略。所读链没有预清目标、失败删除部分文件或回滚全局，prepare 仍可能读取旧配置；提取成功 bool 不能代替逐文件结果。

输出路径直接拼接 `dest+"/"+entryname`，完整选中链未见目录范围检查。文件名 buffer 为 256 B，仅原名长度小于容量才补 NUL，调用方未预先清零却继续扫描 NUL，长度至少 256 的终止保证缺失。这些均为静态发现，未运行异常归档。prepare 的外层 catch `0x6dd2c` 实际返回 null，日志中的 code 666 不是其返回 JSON。

### 配置解析、数值转换与部分更新

`0x11320` 直接读取 JSON 根字段，没有 `device` 包装层，也没有必填字段、枚举范围或字段组合验证。有效的 `{}` 或非对象根值都可以返回 true；后者不修改配置。未知字段忽略，字段缺失与存在但为 null／错类型分别处理。下表列出实际处理顺序，位置均为 SDK 的 RVA：

| 根字段 | 实际类型与全局存储 | 字段缺失时 |
| --- | --- | --- |
| `vid/pid` | u16，`0x94110/0x94112` | 保留此前值 |
| `productName/minBaseFWVersion/targetICVersions` | 严格 JSON string，`0x94118/0x94138/0x94158` | 保留此前字符串；不进行版本格式或 IC 数量校验 |
| `reportId/reportLength/deviceType/supportProrocol` | u16，`0x94178/0x9417a/0x9417c/0x9417e` | 保留此前值；reportId 在配置中不是 u8 |
| `ics` | 先销毁旧列表，再按下述形态转换 | 旧 IC 列表保留，即使本次 VID／PID 已改变 |

所有 u16 字段使用 `0x23a20`：整数取低 16 位，`-1→65535`、`65536→0`；boolean 也接受。浮点先由 `cvttsd2si eax` 向零截为 signed 32 位，再存 AX，不能写成有范围检查的 float→u16。null、字符串等非数值类型抛 `type_error.302`。`delay` 则使用 `0x23ee0` 的完整 64 位转换，接受负整数的补码和数字浮点，不接受 boolean，也没有合理等待范围检查；极端浮点结果还受指令及实际浮点异常控制影响，尚未执行差分。

`ics` 为 array 时按原顺序遍历，为 object 时深拷贝包装为单项 array，为其他非字符串 primitive 时替换为空 array。为 string 时仅再解析一次 JSON，不重复对象包装；因此直接 `{"icName":"A"}` 产生一条名称 A 的 IC，而字符串解出的同一 object 按有序 map 遍历成员值，`"A"` 这个非对象元素产生一条默认 IC。非对象数组项同样会追加默认 IC，不跳过、不拒绝。字符串解出的 null 不产生项，其他 primitive 则产生一个默认项。

每条 IC 为 128 B，按 `icName(+0)`、`mcuIndex(+32)`、`targetVersion(+40)`、`updateType(+72)`、`fwFileName(+80)`、`delay(+112)`、`bcdPid(+120)` 处理；字符串缺失为长度零空串，数值缺失为零。除 delay 为 u64，其余数值均采用上述 u16 转换。没有名称／MCU 去重、支持值检查、文件存在检查或版本比较，所有配置项逐个追加。JSON 对象重复键由 `0x403d0 → 0x53eb0` 复用原 slot，后值覆盖前值；重复 IC 记录全部保留。

首次 DOM 语法解析失败时尚未写全局；解析成功后则逐字段即时提交。后字段类型错误不撤回前字段，`ics` 内部字符串解析失败时旧列表已经清空，第 N 项失败时已追加的前 N−1 项仍保留。catch funclet `0x6c9b0` 仅记录错误并返回 0，没有恢复快照。`prepareUpdate` 在 `0x1d671` 调用它后不检查 AL，继续于 `0x1d718` 返回 code 1。不能将这条“准备成功”当成配置校验、原子提交或设备状态确认。

UMA 分支 `[1871666,1874303)` 从实际本机 Common 文件清单中找到两个芯片文件，按目标版本和产品 ID 选择，传入 `chip1UpdatePath/chip2UpdatePath`、当前/目标版本与设备身份。源码当前使用 `some` 判断两个路径的存在结果，至少一项存在就可越过该判断；不能把它记为“两个固件文件已全部校验”。原件的 UMA 初始化还会跳过标准 setUpSDK，因此不能套用同一套准备状态。

## 实际传输、事件和成功

标准分支 `[1874303,1875528)` 对参数进行校验后调用实际传输，并通过事件 promise 等待结果；`runFWUpdater` 的 void 返回本身没有成功含义。原件 getVersionInfo 在缺接收器版本时会返回主 current／target、isDongleUpgrade=false，导致 performStandardUpgrade 可能选择 supportUsb；prepareUpdate 却按实际 connectType 选择 supportDongle。该错分支须保留为独立验证条件，不能把主版本冒充接收器实测；实际接收器版本保持未观测，不能凭该 fallback 确认身份或成功。

原版 wrapper 的 `runFWUpdater` RVA `0x33c30` 复制输入字符串的所有权，再交给工作队列；callback RVA `0x32ee0` 构造 `callBackId/callBackType/cmdType/arg/event`，上层由事件频道消费。SDK 的运行输入解析器 `0x12ad0` 依次可选读取 `vid/pid/SN/targetVersion`；前两者使用上述 u16 转换，后两者严格要求 string。缺字段保留旧全局，后字段失败不撤回前缀。内部 catch `0x6cb43` 仅记日志并吞掉 `std::exception`，`runFWUpdater 0x1d8e0` 不检查解析返回，清标志后仍进入调度；仅外层 catch `0x6deac` 发布 2003，不能保证无效 JSON／字段必然触发该事件。

调度 `0x1a340` 复制已准备配置的完整 IC 列表，没有按设备版本、SN 或目标版本再次筛选。每轮从剩余项中查找 mcuIndex 等于当前零基轮号的首项，找不到则选剩余首项；执行后不消费其返回值，移除该项，轮号加一并等待 500 ms，再继续处理。因此它不是对任意稀疏／重复 mcuIndex 排序，也没有在首个 IC 返回失败后统一停止。空列表发布 code 2004。运行 targetVersion 在 `0x180f0/0x19020` 参与文件路径选择，长度为零时采用 IC 字段；初始化 `0x1450 → 0x31020` 实际构造的是长度 1、内容为 NUL 的 SN／targetVersion，缺字段保留它，不能据此推定满足长度零的 fallback 条件。这不是比较设备版本后跳过更新。

实际 IC 执行 `0x19b80` 每次都先查找引导设备 `1532:110e`，没有 protocol／updateType 条件；找到时发送复位，再查找目标 VID/PID/SN。找不到目标句柄发布 3003；找到后关闭查询句柄，再按 updateType 分派实际更新器。已读到的分支为 0…4 经 `0x180f0`、10 经 `0x19020`；不能把其他值推断成已支持。两条链分别管理本地固件路径、真实更新调用、进度回调及成功/失败，加载下层更新库失败发布 3004，实际更新失败发布 3002。不同事件的前端消费者须按真实 CommandName 分别判断，下层产品协议仍需独立迁移。

### 设备发现、复位预检与 3003 的实际消费者

[设备预检专项](../../../.analysis/results/host-current/firmware-device-observation.json)保留标准 SDK、wrapper、宿主转发与前端完整所有者。`0x19b80` 先以空串号、配置的 FeatureReportByteLength 查找 `1532:110e`，找到即发送 91 B report（offset 6…8=`00 10 05`），忽略 `0x16c30` 返回、关闭探针句柄并等待 1000 ms。这条链会写设备，不能作为只读查询测试。随后仅执行一次目标 VID/PID/SN 查找，没有轮询重连或版本读取；探针句柄也不能为下层更新器重新打开设备提供同一身份保证。调度器忽略单项结果、移除该 IC 并等待 500 ms 后继续，不因 3003 统一停止。

finder `0x16dc0` 将路径大写后分别匹配四位 VID、PID 子串，没有锚定 `VID_`／`PID_`。以 access=0／share=3 打开，核 HidP_GetCaps 成功与配置的 FeatureReportByteLength 后，空 SN 或请求长度为零直接接受首个接口。运行 SN 先按 C 字符串截到 NUL，比较长度另取原 std::string 长度的低 16 位；初始长度 1 的 NUL 因重建为空而绕过查询。非空 SN 的 wire 长度只取低 8 位，响应却从 offset 9 复制完整 u16 长度并区分大小写比较，没有 82 B 上界检查，也未校验实际逻辑 SN 长度。枚举 detail、GetCaps 短路、串号不匹配与查询失败各有未配对的 buffer／句柄释放路径，不能合并成“查找后均已清理”。

预检 exchange `0x16c30` 与 V25 的 `0x5660` 不同：请求对 offset 3…88 XOR、写 offset 89；最多三次 SetFeature，之后总计最多二十次 GetFeature，只检查响应 offset 1 的 status，不核 transaction、命令回显、响应 checksum 或实际长度。返回 busy=1 要求 busy 恰在第十次读取出现，API 失败也计数，不能改写为连续十次 busy。Set 全失败返回 6，读次数耗尽返回 7；其他 status 2／3／4 分别返回同值，非 busy 未知值返回 5。`0x16b60` 的真实汇编读取完整 64 位 QPC／QPF，不能照抄反编译的 LowPart。

3003 的 CommandName 实际是 `Get Target Device Handle.`；SDK `0x15430`、wrapper `0x32ee0` 与宿主 relay 均不改名。当前 `Br.report → xr` 只注册 `FirmwareUpdate`，因此这条事件在未知命令处返回，不会自行结算升级 promise 或进入 handleUpgradeFailure。标准调用没有 timeout，但剩余 IC／后续设备事件仍可独立影响流程，不能泛称所有运行必然永久挂起。该分发差异、预检写入、串号边界和资源清理均须分别做纯／模拟测试与实际 DLL／设备对照；静态核查不是已执行的结果。

### 底层更新器选择、返回值与进度

[固件分派专项证据](../../../.analysis/results/host-current/firmware-dispatch.json)保留标准 wrapper、SDK、V25、Ry 及当前 PID 179 的 UMA wrapper／SDK 选中完整函数、真实分发表、页面完整 AST 与嵌套 CSS 原始块。标准资源逐项核对 host release index → resource manifest → 原资源声明 → 实际字节，UMA 另核对当前保留的 179/mw manifest 声明与实际字节。以下 RVA 按各自模块解释，不从另一个同名 DLL 或更新器包转移身份；选中函数与来源身份以该收据为准，完整捕获数量不作为语义完成率。

| 当前分支 | 真实下层调用及结果消费者 |
| --- | --- |
| updateType 0…4 | SDK `0x16560` 选择 `Razer_Upgrade_SDK_V25_v1.2.0.dll`，查询 `delay/setdevtyppe/UpgradeFW/AddSdkCallBack/GetSdkVersion`。实际执行注册进度 callback 后调用 UpgradeFW；SDK `0x180f0` 仅将返回 `2` 视为本次成功，其他值发布 3002 |
| updateType 10 | SDK `0x17660` 选择 `Ry_Online_Update_Dll_ouput_v1.0.1.dll`，绑定 `userSetCfgFilePath/chipGoToBootFirmware/appUpdateFirmware`；`0x19020` 仅将返回 `0` 视为成功 |
| 5…9 或大于 10 | 选中的 `0x19b80` 没有相应更新／错误事件分支，继续本机清理；不能把这种静默路径当作支持、成功或本项目应该恒定返回“不支持”的依据 |

两套 loader 都在 LoadLibraryW 成功后直接返回 true，没有逐个核对 GetProcAddress。它们查询的未使用导出与真实执行必需导出须分别处理，不能把加载 bool 当成功能完整性证明。这些厂商加载逻辑仅供静态对照，产品必须以 Rust 独立实现下层协议。

V25 原件的 `UpgradeFW 0x6150` 在没有全局 callback 时返回 `4`；`AddSdkCallBack 0x6320` 只接受非空值，传空不会注销。实际 updateType 0／2 到 `0x7f20`，1 到 `0x8f20`，3 到 `0x6340`，4 到 `0x68d0`；bcdPid 非零才替代原 PID。以下分别记录实际传输与应答，不以同一导出覆盖五条协议链。

HID 公共 helper `0x5660` 始终传输 91 B，发送前把请求 offset 3…88 的 XOR 写到 offset 89；最多发送三次，每次 SetFeature 成功后最多执行一百次“等待 → GetFeature”。响应只比 offset 2／7／8，匹配后 status 1 继续等待，status 2 才复制完整响应并返回 `2`；其他已匹配的非 busy 状态为 `3`，无效句柄或系统 API 失败为 `5`。错误变量跨尝试保留，没有响应 XOR、report ID、长度或完整地址回显验证；等待结束和 API true 都不能替代 status 2。

type 0／2 的完整链 `0x7f20 → 0x8300` 按下列 91 B 帧工作，未列字段初始化为零，offset 0 为 report ID：

| 动作 | 请求固定字段与载荷 |
| --- | --- |
| type 0 切 boot | offset 6…9=`02 00 04 01`，offset 10=`00` |
| boot 身份查询 | offset 6…8=`00 10 80`；响应 offset 16…17 的 BE16 必须等于预期 PID |
| 开始 | offset 6…8=`08 10 01`；offset 9…12 为 base BE32，13…16 为 size BE32 |
| 每块写入 | offset 6=`n+1`，7…8=`10 02`，9=`n`，10…13 为地址 BE32，14 起为最多 64 B 数据 |
| 每块读回 | offset 6…8=`05 10 83`，9=`n`，10…13 为地址 BE32；逐字节比较响应 offset 14 起的 n B |
| 结束／reset | offset 6…8=`00 10 05` |

type 0 在初始 HID 打开成功后发送切 boot，但丢弃其返回；最多五十次、每次先 Sleep(1000) 查找 `1532:110E`，关闭探针后再打开 boot。初始打开失败会直接尝试 boot，type 2 则跳过切换、使用初始句柄。`0x8300` 先写完全部块，再第二遍完整读取与原镜像比较，任一 helper 不是 `2` 或任一字节不同均失败。这条真实读回链不能扩展到下述其他 updateType。

V25 的实际读回与失败释放由独立 owner 进一步限定：[调用方 `0x7f20`](../../../.analysis/results/native/5899b2c58435460f98cba697e3600f00bef3f08cea12b436dd65d24a7ddc170e/functions/7f20.json，函数文件 SHA-256=`c4463ce029eca78967539c0c1f254656eca1a2be589cc09c876d8843fd0ecfd9`) 在写入前用 `0x5660` helper 发开始帧，helper 返回值不是 `2` 就直接以该值结束；[读回 owner `0x8300`](../../../.analysis/results/native/5899b2c58435460f98cba697e3600f00bef3f08cea12b436dd65d24a7ddc170e/functions/8300.json，函数文件 SHA-256=`5ba76d10123d539cdb472f1fb94669e9f487148232f8de4d34e38650ac7f9dab`) 只有全部写块成功才进入第二遍。第二遍每块发送 `05 10 83`，只接受 `0x5660` 返回 `2`，从响应 offset 14 复制本块长度并逐字节比较；首个差异返回 `3`，I/O／状态失败保留 helper 的 `3` 或 `5`，空镜像则不传输直接返回 `2`。成功后的 `00 10 05` reset 结果被调用方丢弃。

`0x5660`（[helper 捕获](../../../.analysis/results/native/5899b2c58435460f98cba697e3600f00bef3f08cea12b436dd65d24a7ddc170e/functions/5660.json)，函数文件 SHA-256=`843e3d936111ad38c5220e55b7ed35f9bfbf99404107dfabe88d3729250820b7`）每次将请求 offset 3…88 的 XOR 写到 offset 89，最多三次 SetFeature；每次成功后等待，再最多一百次 GetFeature，仅比对响应 offset 2、7、8 和 status，status=2 才复制完整 91 B 响应并返回 `2`，非 busy 的其他状态返回 `3`，API／等待耗尽返回 `5`。它不核响应 checksum、长度或完整回显，也不关闭传入 HANDLE；`0x7f20` 仅在重连循环中关闭最终成功的句柄，失败、读回不匹配和 `0x8300` 返回异常时只释放镜像 `Block`，旧 boot／初始 HID 句柄仍有未配对路径。等待 helper `0x55a0`（[捕获](../../../.analysis/results/native/5899b2c58435460f98cba697e3600f00bef3f08cea12b436dd65d24a7ddc170e/functions/55a0.json)，函数文件 SHA-256=`fbf5688173ddd499448e878a1fe2c28f9a6f4c13009216744083527751538888`）也没有取消或句柄释放；因此返回 `2` 只代表这条 helper／本地读回条件，不能当作设备持久化确认或完整资源清理。

type 1 的 `0x8640` 请求 offset 6…7=`08 00`，mode 2 时 offset 8=`93`，其他为 `87`；ACK 成功且响应 offset 13=`B1` 才认 boot，mode 3 还接受 offset 9=`B1`。`0x8f20` 未认 boot 时发送 `00 0A 00`，要求成功后至多十次等待一秒再查询；随后 `00 0A 80` 要求响应 offset 9 非零、11 为 2／3，并取 offset 10 为 selector。

type 1 的逐行 owner `0x8730` 写 offset 3…4=`FF FF`、6=`u8(line_len+7)`、7…8=`0A 02`、9=selector、15=`u8(line_len)`，16 起复制原行；helper 返回 `2` 后还要求响应 offset 14 为零。CRC 从 `FFFF` 开始，每行补零到 92 B 后处理完整 92 B，递推对应非反射多项式 `1021`。最终帧 offset 6…8=`0A 0A 02`、15…16=`03 3C`、17…18=CRC BE16，同样要求 ACK offset 14 为零。这里没有 type 0／2 的逐块固件读回；源码对原行完整长度复制与 wire 长度截为 u8 的边界仍需保留，不能接受任意长行。

HID 两条主链均忽略结束帧结果，分别忙等两／三秒后最多五十次等待原 VID／PID 重新出现；普通 HID 重开没有原 selector，也不调用版本导出核对目标固件。主 HID 和切换前句柄没有随探针一同关闭。`0x55a0` 的实际 QPC／QPF 使用完整 64 位值，按 `(counter_delta/frequency)×10^6 >= int32(delay×1000)` 忙等，频率查询失败跳过等待，没有取消检查；不能照伪代码 LowPart 只取低 32 位。`0x8300` 进度分母为有符号长度除 64 向零截断，1…63 B 首块成功后可达除零，非整倍数可能提前升高，空数据跳过两遍返回 `2`。未执行这些异常输入。

V25 的两条 WinUSB 链并不使用相同的地址和应答契约：

| 分支与完整所有者 | 擦除／发送 | 实际状态条件 |
| --- | --- | --- |
| type 3，`0x6340 → 0x7210` | 按 512 B 区间擦除：`0x40/0x82`，Value 为起址、Index 为起址加 511。随后对解析记录先发送 `0x40/0x81`，再发送 `0x40/0x87`；每条最多 8 B，Value／Index 为记录起止地址 | `0xC0/0x8F`、Value／Index=0、Length=1；首字节只要求不等于 `0xFF` |
| type 4，`0x68d0 → 0x6dd0` | 按 4096 B 区间擦除；Value／Index 分别为 32 位地址的低／高 16 位。数据同样先 `0x81` 再 `0x87`，每块最多 64 B，末块用实际长度 | 相同状态请求，但首字节必须等于 `1` |

type 3 擦除的初始化／OUT 每区间至多三次，状态查询也至多三次；数据每条 OUT 至多两次，随后 ACK 至多三次。type 4 擦除每区间仅一次；数据每块每遍 OUT 和 ACK 各至多十次。两条数据链的 ACK 耗尽都直接失败，不返回 OUT 重发。type 3 的 `0x87` 是再次发送同一数据并查询状态，没有读取固件字节逐项比较。type 3 擦除停止后仍按 `current_start < parsed_end` 判错，保留实际边界，不能统一改写为一个“重试成功”布尔。

WinUSB 的发现 `0x5ca0` 使用 GUID `c9348766-d27c-41ee-b2be-ea9412f72b52`，先匹配路径中的四位 VID／PID，再要求描述符 bcdDevice 等于传入 selector；默认 boot 身份为 `1532:110D`。selector 的实际用途不能直接改名为固件版本。type 4 在 `0x6b15` 检查的是传入 VID 的低字节非零，并非取消标志。传输均未消费 LengthTransferred，type 4 擦除的状态 buffer 仅在循环前初始化为 `0xFF`，后续可能保留上一应答；API true 不能证明这次确实收到完整状态。

两个 WinUSB 分支均忽略切 boot `0x6c90` 和 reset `0x6d40` 的结果；reset 发送 `0x40/0x84`、Value／Index／Length=0，之后等待约两秒，再最多五十次、每次先等待一秒检查设备重新出现。该检查只尝试重新打开，未独立核对更新后版本。原 HID 句柄和真正 boot 文件句柄在选中的成功／普通失败尾部没有配对 CloseHandle；各次 WinUsb_Free 仅释放对应接口，不等于关闭底层文件。额外探测句柄的关闭不能证明会话已清理，更没有刷写失败后的回滚或取消链。

镜像边界也须按当前原件保留：`0x32d0` 仅大小写精确的 `.hex` 后缀进入 `0x24d0`，`.HEX` 走 raw 分支；raw 读取到零即关闭返回 `2`，没有 ferror 检查，空文件也走成功返回。type 3 的解析器 `0x1350` 从原行 offset 2／4 取两个二字符拼地址、offset 8 起取数据，`0x4b90` 是直接 substr，不能套用标准 Intel HEX 偏移。type 4 数据进度以向零截断的 `data_len/64` 为分母；1…63 B 且首块 ACK 成功时存在静态可达除零路径，零长度反而跳过两遍返回 `2`。这些是尚未执行的静态边界，不能作为安全传输或 Rust 接受无效镜像的依据。

文本打开 `0x3600` 使用 `_Fiopen(path,1,64)`；`getline 0x4540` 不跳过前导空白，消耗而不保存 LF，保留冒号，没有额外 trim 或显式去 CR。CRT 文本模式换行转换单独保留。两种 parser 都将普通 getline false 视为正常终止，已打开的空文件返回 `2`，不要求至少一条 data 或 EOF 记录，也不校验记录 checksum。空行、BOM 和前导空格不能越过冒号判断。关闭 `0x4dd0` 失败只改流 failbit，不改 parser 的成功返回。

两种格式的具体差异如下，不能共用一套未经证实的标准 HEX 解码器：

| 所有者 | 实际记录与地址／数据规则 |
| --- | --- |
| type 3 `0x1350` | 仅 `line[7]=='0' && line[8]=='0'` 进入 data，其余记录全部忽略。地址为 `hex(substr(2,2))*256+hex(substr(4,2))`，载荷为 `substr(8,line_len-(末字符为LF)-11)`，不读取声明 count，也不支持扩展地址。输出每组最多 8 B，token 为 count＋起址 BE16＋含末字节的止址 BE16＋切出的 hex＋空格 |
| type 4 `0x24d0` | count 来自 `[1:3]`；04 保存 `[9:13]` 前缀，00 由此前缀＋`[3:7]` 得完整地址且首条设置 start。05 不读自己的 payload，只将上一 data 的完整地址＋上一声明 count 写 end。01 等其他类型忽略，继续读后续行 |

type 4 的 `0xFF` 补洞仅比较前后地址低 16 位，任一为零则跳过，不能当成完整扩展地址补洞；数据从 offset 9 取最多 `2×count` 字符，再先右移一位求实际字节数，奇数尾 nibble 被丢弃。它的 strtol／strtoll 不核对 errno 或 endptr。type 3 数值转换会拒绝无数字前缀／ERANGE，但允许部分数字；奇数尾片还能使后续 substr 越界。直接索引及过短记录也没有完整长度保护。

静态推导的 LF 行 `:04123400112233440C` 在 type 3 产生 start=`0x4123`、含末字节 end=`0x4126`、token=`044123412601122334 `；type 4 则首址=`0x1234`、数据=`11 22 33 44`，end 要等后续 05 才赋值。此例未执行 DLL。两者 EH4 元数据仅注册流／字符串清理，没有本地 TryBlockMap 将格式异常统一转为 `4`；raw FILE 及临时裸 malloc buffer 的分配异常清理也未闭合。实现和测试须保留这些差异，不能把模拟器按标准格式解码所得的“一致”冒充官方实际结果。

Ry 的真实虚表调用到 SDK `0x17b30`，使用 `update_config.ini`。它最多执行四个向前推进的轮次；每轮先设置配置，再请求进入 boot，最后更新 app，任一步返回非零便退出，完整成功一轮后等待 200 ms 才进入下一轮。这是四个依序阶段，不能写成“失败重试四次”。异常捕获返回 `-1`，缺模块返回 `4`。底层三个导出的回调／返回与上层更新完成事件仍是不同边界。

Ry `userSetCfgFilePath 0x9170` 先拼接两个窄字符串并做 mode 0 的文件访问检查，失败清空两个路径全局并返回 `1`；成功只保存路径，不验证 INI schema 或镜像。实际 reader `0x3090` 使用所保存的路径、100 B 的 GetPrivateProfileStringA buffer 和空默认值，其形式上的文件名参数没有决定实际文件路径。`chipGoToBootFirmware 0x8fc0` 的非空 callback 替换全局，空值保留旧 callback；`0x6220/0x6cc0` 读取 boot 身份、checksn、SN 和 timeout，再调用 HID 枚举 `0x38f0`。timeout 仅接受 3…100，其余设为 30 并通过 tag 0 发布；这两个完整 owner 本身没有切 boot 写入或按此值等待的循环。不能由导出名证明设备已经进入 boot。

Ry 的 checksn 先经 `0x19df4c → 0x19df54` 转换大写，再与 `TRUE` 比较，ASCII 的 `true/True` 也可启用；该分支要求 91 B feature report。串号查询 `0x3250` 在请求 offset 6／8 写 `22/0x82`，offset 89 为 offset 3…88 的 XOR；响应标识匹配且 status=2 才复制数据。`0x38f0:0x3db2…0x3dcb` 将查询串号和配置 SN 分别放入 RCX／RDX，再调用字符串比较 `0x19a64c → 0x19a654`，返回零才沿两级父节点及 `0x34b0` 找接口。串号比较函数保持字节／多字节字符的大小写，不做数值解析；反编译调用遗漏第二参数时不能据单参外观推定转换语义。未启用 checksn 时走完整身份子串匹配。实际拓扑、多个同型设备和异常串号的完整适用范围仍需分别核实。

完整 INI 调用集合实际只有以下 8 个 section/key，没有单独的 APPDEVICE_INFO：

| section | key 与实际消费者 |
| --- | --- |
| `BOOTDEVICE_INFO` | `bootdev_info/checksn/SN`，供 app probe 与两个 boot 发现 owner 共用 |
| `BIN_FILE` | `upgrade_file`，文件名／校验和／传输源 |
| `ENCRY_EN` | `encryption_en`，probe 的持久标志与传输的局部标志分别处理 |
| `WAIT_TIME_OUT` | `time_out`，持久 signed32，初值 30；仅 3…100 保留，其余 30，扫描失败保留前值；所读 boot owner 只通过 tag 0 发布，不实际等待 |
| `UPDATE_ADD_WAIT` | `add_time`，每次初值 1000；仅 100…3000 保留，其余 1000，作为 `0x4230` 的毫秒等待参数 |
| `APP_START_ADDR` | `app_start`，仅小写 `0x` 前缀走 `%hhx`，其他走 `%hhd`；读入 8 位后零扩展，须非零且 `u32(payloadLen+(byte<<9))<=0x20000`，否则用 `FF` 继续 |

selector 先按 CRT 转小写、删除全部 ASCII 空格，存在分号才截取首个分号前内容；没有分号保留整串，首字符为分号才变为空。所有 INI 值共用 100 B buffer，没有截断检测。`chipGoToBootFirmware 0x8fc0` 先尝试 `0x6220`、失败后尝试 `0x6cc0`；两者实际都是一次枚举并返回 0／2，导出检查返回 3 的分支不能据此认为可达，更不能将函数名解释为发送了 boot 切换命令。

未启用 SN 时，`0x38f0` 选择枚举顺序首个“路径包含完整 selector 且位置大于零”的接口，没有歧义检查。启用时只先取 selector 的 vid_／pid_ 后各四字符，分别作为路径子串筛选；标签缺失也没有拒绝，可能退化为 Mid(3,4)。候选以 access=0/share=3 打开，要求 FeatureReportByteLength=91。匹配 SN 后沿两级 CM_Get_Parent 进入 `0x34b0` 的 Child／Sibling／Parent 遍历，匹配 hid 前缀及完整 selector；初始节点无 child 仍可访问其 sibling，不能称为严格封闭子树。接口循环不断覆盖路径，实际保留最后取得者，失败枚举后仍可继续取 Detail；返回、分配和 Detail buffer 释放也不完整。

SN 查询复制 offset 9 起 22 B，但候选缓冲只分配 22 B、未补 NUL，且在判断查询返回是否为 2 之前已按 C 字符串扫描，查询失败也可能扫描未初始化内容。正常候选拒绝／成功会关闭候选句柄并释放外层 Detail 与 SN，GetCaps 短路失败仍可能遗留 preparsed data；任意 SN 内容及异常路径不能视为安全、完整的字符串／清理契约。

Ry `appUpdateFirmware 0x9070 → 0x4b30` 发送由三个 uint32 `0x30/4/1987339623` 组成的查询，响应要求 payload[0]=`0x30`、offset 4 的 uint16=`16`。`0x4b30` 从 response[20:22]／[16:18] 分别取 BE16 high／low；镜像版本不大于 high 时先 `high|=0xFFFFF000`，再 `high<<16|low`，标记落在 bits28…31。导出却检查 `result&0x0000F000` 的 bits12…15，等于 F000 或小于 A000 才拒绝，拒绝分支再按 unsigned u32 将大于 15 的结果映射为 6。因此静态推演 `F123A122` 可越过 guard、`A123F122` 被拒绝，不能将其写成完整的旧版／同版保护；这些例子未执行 DLL。

不能按字段含义把 INI section 与 key 反写。`0x78d0` 的 ENCRY_EN 本次局部扫描初值为 0，仅 sscanf 返回 0 时强制设 1；空串的 EOF=-1 留 0，因此本 owner 缺失／空值选 `.bin`、转换不匹配才选 `.enc`，后缀区分大小写。`0x4b30` 则使用持久全局 `0x261508`，缺失／空值会保留前次值，不能共用“每次缺失均选 bin”的规则。该全局处于 PE 装载器清零区，没有对应磁盘原始字节，收据单独核对 section 的 VirtualSize／SizeOfRawData，不伪造 raw hash。配置文件名的名义形参仍不参与 reader 的路径选择。

文件名版本取原 BIN_FILE 的 Right(19)→Left(6)，要求首字符 v／V，再改成 A、移除全部句点，先核 hex 后按 `%04x` 解析；checksum 取 Right(12)→Left(8)，核 hex 后按 `%08x` 解析。没有完整文件名正则、固定总长度或分隔符检查；`0x78d0` 的此文件名处理块没有设备版本比较，`0x4b30` 的版本比较与导出掩码错位见上文。宿主以 `rb` 读取，按 512 B 块累积整个文件的 little-endian u32 XOR 后，将原始头及后续字节直接交给传输；完整已追链没有解密、密钥加载或派生，`.enc` 在此是不透明内容。设备端格式、算法、密钥及认证仍未取证，不能由扩展名推定 AES，也不能将 XOR 当密码学完整性证明。

`0x78d0` 的 command `0x31` 发送头及首 32 B，特定设备身份附带 440／412 B 的原始 loader 数据；`0x32` 发送后续最多 512 B 数据块并在本地累计 XOR，`0x33` 才发送累计 XOR 和本机日期时间。三者实际调用的是 RVA `0x7740`，尽管 IDA 给它标了 `Concurrency::…StealUnrealizedChore` 名称，完整指令证明这里是传输／ACK owner。它将 payload 长度加八后按最多 64 B 分片，分片 ACK 要求同 command 且 offset 8=`0x36`，最终 ACK 要求同 command 且该值=`0x34`。只有最终读取失败触发整包重发，总计最多三次尝试，即首次加最多两次重发；应答身份不符立即返回，不能套通用重试。

Ry `0x40d0/0x4230` 向设备写／读 65 B，写入前补 report ID 0，读取后剥离该字节。两个 owner 都在 I/O 前主动 SetEvent；立即 I/O 失败时仅凭等待返回 WAIT_OBJECT_0 便走成功，没有 GetOverlappedResult、实际长度相等检查或取消 I/O，成功分支也没有关闭 event。上层 `0x78d0` 仅在状态机常规收尾关闭 stream／设备句柄并发布 tag 5；配置错误、文件打开失败和 checksum 失败均绕过这段设备清理／回调，checksum 分支仅先关闭 stream。不能宣称全部失败均清理，也不能由普通收尾证明在途 I/O 和事件已释放。

app probe 的句柄还需单独结算：`0x4b30` 写／读失败会关闭并清空 hObject／hFile，成功或响应 command／length 错误反而保留；`0x9070` 的 mask 拒绝也不关闭，`0x78d0` 后续打开成功会覆盖两全局、遗失 probe 句柄。正常尾部去重关闭当前两个句柄不等于恢复此前丢失的所有权。读取超时、取消、错误映射、多个同型设备及所有这些泄漏／越界路径均未做实际 DLL／设备对照，独立 Rust 必须记录其处理与原件差异。

Ry 进度分母实际为 `u32(ftell−32)`，头成功后分子已增加 51200，每个数据块成功再增加 51200，首个数据块的分子为 102400；不从“没有除零 guard”推断固定 32 B 文件必然执行除法。XOR 循环每次取 32 位，非四字节整倍数的最后一次读取可能消费 buffer 中的旧尾字节，并非有证明的零填充。此处分别保留原始 bool、报文 ACK、进度和最终返回，不能将它们合并为已确认设备状态。

[设备与镜像边界专项](../../../.analysis/results/host-current/firmware-device-observation.json)另保留 `0x78d0` command31 的实际字段：offset 2 是 `u32(ftell−32)>>2` 的低 16 位，offset 4 是 header 实读 h＋8＋可选 loader block 长度，offset 8／12 为数据长度／镜像版本，原文件前 32 B 从 offset 16 开始。loader block 固定从 offset 48 开始，内容为 LE32 长度、440／412 B 原始数据、LE32 XOR。因此 h<32 且启用 loader 时，发送范围会包含未由这次读取覆盖的 `[16+h,48)`，并截掉 loader 尾部 32−h 字节；前面的 workspace 清零不覆盖这个 gap。不能自行把缺失 header 补零后宣称与原版一致。

镜像 0…31 B 没有最低 32 B 拒绝；u32 长度下溢及地址加法回绕可越过范围判断。在配置、checksum、状态和设备 ACK 均满足时，静态可走 command31→command33、返回 0，却没有数据 command32。稳定 32 B 文件同样不自然进入进度除法；需要长度快照为零、后续又实际读到数据且 ACK 通过等条件，才会到无零保护的 div，文件共享与变化条件仍须另核。fread 的完整已读链在 EOF 或错误时均返回已完成前缀，调用方不检查 ferror；32 位 ftell 失败或超 INT_MAX 后返回 −1，仍继续减 32，fseek／fclose 结果也未检查。36 个静态长度分支与独立手算只验证上述条件推演，不算行为模拟、实际 DLL 或设备测试。

V25 进度 callback `0x16480` 以无符号 16 位 step 和 IC 序号／数量计算 `total=(step+100×(ordinal−1))/count`，整数截断；total 与上次相同则不发布，该 owner 没有把输入夹到 0…100。Ry callback `0x17270` 的 tag 0／1 只记录等待／版本，其余 tag 将四阶段进度按 `(progress+100×(stage−1))>>2` 投影到 16 位 step，再按 IC 数计算 total，同样按 total 去重；不把未知 tag 自动解释成独立成功事件。

序列化 `0x15430` 将同一全局字段低 16 位写为 `stepProgress`、高 16 位写为 `totalProgress`。两套外层成功分支都只写 `stepProgress=100` 和 `updateCode="0000"`，随后发布时没有同时补写 total。**成功返回或成功文本本身不能保证前端已收到 totalProgress=100**；下游 promise 的完成仍依赖实际进度 callback 与事件顺序，不得用计时器或 Rust 自行补满进度掩盖差异。

两套 adapter 析构 `0x16b00/0x18080` 释放各自模块与对象，选中函数没有等待 callback 完成或恢复固件的逻辑；SDK `endFWUpdater 0x1da80` 为空。没有由这些析构证明刷写可取消，也不能据此安排实际刷写测试。此次仅静态核对，没有加载 DLL 或执行固件动作。

当前 [Shell 固件入口](../../../crates/razer-shell/src/shell/windows/firmware_update.rs)只创建页面、处理关闭与导航；[Flow](../../../crates/razer-app-pages/src/firmware/firmware_update/state.rs)明示为本地预览模型，`load` 根据预览 key 设置阶段／进度／完成列表。该入口不是上述 SDK 调度、真实版本、原件校验、协议写入与事件确认的生产消费者，不能由阶段名一致或预览完成认作固件功能已经实现。后续纯／模拟测试须覆盖 V25 返回 2 与 Ry 返回 0、缺失导出、稀疏／重复 mcuIndex、前一 IC 失败后的后续处理、四轮中任一步失败、重复 total、成功事件仍带旧 total、迟到 callback 与释放；期望来自各自原件，不执行刷写。

下层纯／模拟用例还须区分配置缺失／null／错类型、重复键与重复 IC、u16 截断与 delay64、字符串解出的对象、部分提交与旧 IC 残留、解析返回被忽略；同时覆盖两个 WinUSB ACK 条件、每一级 OUT／ACK 重试耗尽、短传输与残留 buffer、空文件及 1…63 B 镜像、扩展名大小写、两套 parser 的真实偏移／05／低16位补洞／奇数尾片／checksum 与 EOF 缺失、VID 低字节零、reset 失败但设备重新出现、更新后版本未观测，以及 Ry 的中间／最终 ACK、最终读失败重发、主动置位 event、部分 I/O、零进度分母和关闭后的迟到完成。必须明确记录源码缺陷与 Rust 的处理决定，不能通过放宽对照条件掩盖差异。实际 DLL／设备功能对照和刷后验收仍未执行。

MessageMgr 按 CommandName 路由；仅传入 SerialNumber 为 truthy 时才与当前设备比较，空串或缺失绕过该核对。USB 枚举／打开／关闭事件单独消费，不算 DFU 完成。`xr` 对缺少 CommandData、`updateCode!=="0000"` 或 totalProgress 非零且 falsy 的输入立即返回终止错误；0 本身允许通过。101、−1 或 truthy 非数值只警告，`emitEvent` 拒绝发布并返回 false，listener 仍等待；仅数值 100 结算成功。`report` 内异常与注册的执行函数 reject 有各自失败结算，`handleEvent` 的 JSON 解析异常却只记日志，不结算等待中的 promise。stepProgress／stepName 用于阶段显示，不能用计时器补满进度。

操作登记前，`jr` 对已有在途事件任务最多等待五轮，延迟从 500 ms 倍增、上限 5000 ms；这是等待事件通道空闲，并非重新刷写五次。标准与 UMA 调用均未提供该事件 promise 的 timeout，源码打印的 “no timeout” 不能描述成自动取消或已实现超时恢复。

原版还通过本机 window storage 关联正在更新的 productId/serialNumber；当前代码的序列号键名为 `serialNUmber`，需按实际 producer/consumer 匹配。前置 `zr` 解析各窗口记录，只返回首个 status truthy 的记录，`Xr` 再比较 productId／序列号；没有 active 记录则写本窗口 status=true。同设备多个窗口仍可通过，读写不是原子锁，`Yr` 吞掉写入异常也可使 Xr 返回 true；不能当作严格设备互斥或设备处于 bootloader 的实测结果。false 记录在前置已被过滤，不能据 Xr 单独推断其绕过身份校验。

### 当前 PID 179 的 UMA 双芯片链

当前实际链为 `Wr → RzUMASDKWrapper_v1.0.3.0.dll → Razer_Upgrade_SDK.dll → HID`。两份原件来自同一份 [179/mw manifest](../../../.analysis/inputs/official-native-current/manifests/72671f1738b7d9a37cf93429d457f0bff61cabcb93836ddb2cb3e1f2ba3b4929/manifest.json)，通过[资源获取收据](../../../.analysis/inputs/official-native-current/acquisition.json)和上述专项证据核实。这里复核的是 2026-10-15 保留观测的实际字节，不是重新在线确认部署；也不能推广为全部 UMA 产品。下层 SDK 的 manifest resourceVersion 为 `1.0.0.0`，PE FileVersion／ProductVersion 为 `1.1.2.0`，两种版本身份分别保留。

`findTargetResource` 与实际 upgrade 同时要求 Windows、source 包含 `usb_5426_179`；`Wr.init` 跳过标准 setUpSDK 则仅检查 source，不带相同平台条件。前端从 Common 清单取首个名称包含目标 wrapper 的资源，两个芯片名称按目标版本、keyboard／dongle 和 LPC55S28／nRF52833 前缀选择，匹配多项保留最后者。两个名称均须存在于清单，但文件存在检查只用 `some(Boolean)`，不能认定两文件都实际存在。

UMA wrapper 的 DllMain `0x34a40` 调 `Initialize 0x30f80` 后始终返回 true。loader `0x30540 → 0x2e520` 在 wrapper 目录找下层 SDK，精确文件名优先，否则比较 `_v/_V` 版本段；LoadLibrary 成功后单个导出缺失仍只日志，initialized 可以置真。`initSDK 0x32b80` 不解析参数，注册 `0x32ab0` callback、调用 `setdevtyppe(2)`，缺指针也只日志，返回 malloc 分配的空串。此 wrapper 没有 prepareUpdate，JS 首次 ConfigureFFI 失败后删除该声明重试；宿主失败链没有自动卸载，同路径重试再调用 LoadLibraryW，在没有其他卸载者时复用驻留模块，不重跑 PROCESS_ATTACH，详见下节的实际宿主 FFI 链。

`runFWUpdater 0x33350 → 0x30110 → 0x2fab0/0x36680 → 0x3abb0 → 0x336b0` 通过队列执行。实际任务槽为表 `0x58758+0x10`；专项证据另外保留 `0x58720` 的 6 槽与 `0x587f0` 的析构槽，前两表各 48 B，后续 RTTI 不伪装为函数槽。worker 仅消费两个严格 string 的 chip path，调用方 JSON 的 VID、PID、SN、currentVersion、targetVersion 不参与设备筛选。

SDK `upgrade_firmware_process 0x7010` 的六参是 `(u16 vid,u16 pid,const char* file,int delay_ms,int algorithm_selector,u16 alternate_pid)`；第 4 项的 `2` 流入传输等待，**第 5 项才选择算法**。第 6 项非零时替代 PID 身份比较，本消费者传零。

| 顺序与 wrapper 调用点 | 实际参数与下层确认 |
| --- | --- |
| 第一芯片 `0x33f24` | `(5426,179,chip1,2,0,0)`；`0x8e00` 进入／等待 `1532:110e` boot，核 PID 身份；`0x91b0/0x92b0` 发送地址长度、写完全部最多 64 B 块后，再完整逐块读回比较；差异返回 3 |
| 第二芯片 `0x3408d` | 仅第一返回 2 才调用 `(5426,179,chip2,2,1,0)`；`0x9ef0/0x9640` 查 boot 并按需切换，`0x9720` 逐文本行发送，以补零至 92 B 的每行递推 CRC16，最终 CRC ACK 后 callback 100；没有第一算法的整镜像读回 |
| 两芯片完成后 | `GetDeviceFWVersion(5426,179,out[8],8)` 返回 true，且前后两个 4 B 区域分别至少一 byte 非零；约十秒轮询，间隔 1、2、2…秒。**没有比较 targetVersion** |

两条算法共用 `0x65c0` 的 91 B HID feature report，请求 `[3,89)` XOR 写 offset 89；响应匹配 `[2]/[7]/[8]`、status `[1]==2` 才复制返回 2，没有复核响应 CRC。最多三轮，每轮 SetFeature 至多三次、GetFeature 至多五次，不能套用 V25 的 100 次读取上限。枚举 `0x6830` 匹配 VID／PID 字符串和 FeatureReportByteLength=91，以 access=0/share=3 打开。两算法末尾都发送结束命令、等待原身份再次枚举；重现本身不是目标版本确认。

第一失败发 `0001 / Update First Chip Failed` 并停止；第二失败发 `0002 / Update Second Chip Failed`。最终就绪成功才发 `0000 / UpdateInfo`，超时发 `6789 / Wait for the device ready time out!`。`0x31750` 按 signed int 计算 `((progress+base)*100)/200`，base 先零后 100、没有 clamp；`0x32ab0` 去重，并抑制第二芯片 progress=100，避免在最终就绪之前完成前端 promise。step=0、stepName 为空、stepProgress=-1，SN／UniqueDeviceID 为空；空 SN 不触发 Br 的序列号过滤，回调还没有连接 generation。

缺升级指针、chip 参数缺失或异步 JSON 类型错误没有对应失败事件；与未设置 timeout 的前端等待组合，存在不结算路径。`uninitSDK 0x32ef0` 只日志 true，`upgrade_progress_percent(0)` 不清 callback，`UnRegisterFFIEvent 0x31610` 只清匹配 event 名。`Terminate 0x31370 → 0x2eb60` 直接卸载并清六指针，没有 stop／drain／join；singleton 析构 `0x304a0 → 0x2fa00` 遇非零 thread ID 调 std::terminate。所选版本查询／BackToDefault／芯片算法打开的主 HID 句柄没有 CloseHandle，`0x6ad0` 只关闭自身临时探测句柄。BackToDefault 虽被解析，但 worker、失败、uninit 未调用，不能称为固件回滚或取消恢复。

上述为当前消费者实际选中的 selector 0／1。同一 SDK 的其他算法已按下节单独核对，不能把代码存在推断为某款产品实际准入。这里未加载或执行 DLL、未刷写；独立 Rust 实现、纯／模拟行为测试、实际 DLL 对照与设备验收分别未验证。

### 同一 UMA SDK 的 selector 2／3／4

`0x7010` 的 selector 2 仍进入 `0x8e00`，但 enter_boot=false：直接向原 VID／PID 的 91 B HID 发送 boot 协议，不发送切换命令，也不改为等待 `1532:110e`。身份按 effective_pid 核对，写完后真实逐块读回，再发结束并等正常枚举；不能为该分支补出另一套 boot 切换。

selector 3／4 则进入 `0x71e0/0x7730`，等待 `1532:110d` WinUSB。finder `0x6b70` 的 GUID 为 `c9348766-d27c-41ee-b2be-ea9412f72b52`，读写 access、share=3、OVERLAPPED；读取 18 B Device Descriptor，offset 12 的 bcdDevice 与 effective_pid 比较。`0x7a00` 的切 boot 命令结果被这两分支忽略。`0x6ad0` 每轮先等 1000 ms、最多 50 次，PID=110d 才用 WinUSB finder，其他 PID 用 HID；成功后只关闭探测自身的句柄，不代表主流程句柄已释放。

| 分支 | 擦除与 ACK | 写入与设备校验请求 |
| --- | --- | --- |
| selector 3 | `40/82`，Value=`low16(current)`，Index=`low16(current+511)`，Length=0，步长 512；OUT 至多三次，成功等 2 ms 后查 `C0/8F` 至多三次，状态只须不等于 FF | `0x8290` 先 OUT `40/81`，再 OUT `40/87`，两遍都附同一本地数据；OUT 至多两次、状态至多三次。每块最多 8 B，进度在 OUT 前发布，不核实际传输长度 |
| selector 4 | `40/82`，Value／Index 分别为起始地址低／高 16 位，Length=0，步长 4096；擦除与状态均单次，状态必须为 1 | `0x7ee0 → 0x7ab0/0x7c80` 两遍各读 64 B、发送 OUT 81／87；每个 OUT 和状态各至多十次，间隔 delay_ms，状态必须为 1，不核实际长度或 gcount |

这两种 OUT 87 都把本地字节交给设备，**不是主机读取固件内容后比较**，不能套 selector 0／2 的读回结论。selector 3 的擦除循环条件是 `current<=end+1`，错误后仅 `current<end` 返回 3，相等边界仍可能继续；状态每次先置 FF，只接受“变化了”并不证明设备内部具体校验算法。结束 `0x7e50` 发空 OUT 84、返回被忽略，再等 2000 ms 和原设备重现。

selector 3 文本 parser `0x2520` 采用确实存在的非标准偏移：只处理 `line[7:9]=="00"`，地址取 `substr(2,2)/substr(4,2)`，payload 从 offset 8 截取，长度为 `line_len−(末字符为 LF?12:11)`。每行按最多 8 B 生成 `长度+BE16起始+BE16包含末字节地址+数据+空格`，不跨行拼组；不处理 04 高地址，不核 checksum、声明 byteCount 或地址单调。空输入可返回 2，错误内容可抛 C++ 转换／字符串异常；不能按标准 Intel HEX 猜改其偏移。传输进度的 `end−start−1` 也没有零分母保护。

selector 4 的 `0x1020` 用 `GetModuleFileNameW(NULL)` 获取**宿主 EXE 目录**，经 CP_ACP 转换后固定追加 `\new_fw.bin`，不是 DLL 目录或系统临时目录。initializer 表 `0xc4d0` 指向该函数，FileName 指针 `0x139f0` 位于 PE 零填充区、运行时才赋值，没有磁盘原始指针可伪造 hash。固定文件名无并发隔离。`0x41e0` 仅精确小写 `.hex` 走 `0x1480`，其他扩展按 4096 B 块复制，start=0；读写和 stat 错误没有完整检查。HEX 分支支持 04 前缀、00 数据，以第一条组合地址为 start，低地址缺口填 FF 但排除地址为零，05 取上一组合地址加上一数据长度，不消费自身 entrypoint，也不核 checksum；预处理成功仍保留临时文件。

`0x7730` 的 RVA `0x7926` 原字节 `80 7c 24 30 00` 比较 `[rsp+30h]`，完整函数未写入或传出此字节；LengthTransferred／Overlapped 参数只占 +20／+28，不能解释为已经初始化该 flag。因此擦除后是否进入 `0x7ee0` 取决于未初始化栈值，不能写成稳定的两遍升级流程。

即使进入 `0x7ee0`，文件大小为 64 的整倍数时两遍都会额外发零长度块；空文件或小于 64 B 在首个 OUT 成功后可于 `0x8090/0x81ce` 整数除零。进度分母 `D=floor(size/64)`，第一遍为 `min(100,50*k/D)`，第二遍为 `min(100,50+50*k/D)`；第一遍并不严格限于 0…50，例如 64 B 会出现 50→100。仅两遍成功才删除临时文件，失败 unwind 关闭流但留下文件；局部 WinUSB interface 释放不包含主 HANDLE，也没有取消、原固件恢复或失败 reboot 闭环。

#### WinUSB 探测、ACK 与消费者释放边界（补核）

本轮以 [UMA SDK 原件](../../../.analysis/inputs/official-native-current/blobs/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/Razer_Upgrade_SDK.dll)（SHA-256 `6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b`）及其 [完整捕获](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/capture.json)（SHA-256 `c3e6bfa3b4b74f704b706c7cc7658daa82a7ed28f9cfcc5fa3bfd3bdcf9dba85`）为边界，使用 `ThreadPoolExecutor(max_workers=4)` 并行核对 SDK 的两组函数；另以 [UMA wrapper 原件](../../../.analysis/inputs/official-native-current/blobs/54b8d53c215c7992ab949aa75db1cd92706bdd1fb4a488a1efcdb695141f68c5/RzUMASDKWrapper_v1.0.3.0.dll)（SHA-256 `54b8d53c215c7992ab949aa75db1cd92706bdd1fb4a488a1efcdb695141f68c5`）和其 [完整捕获](../../../.analysis/results/native/54b8d53c215c7992ab949aa75db1cd92706bdd1fb4a488a1efcdb695141f68c5/capture.json)（SHA-256 `e74f7b407b2787d01e150669dbf766f7836b192deea4cc23502d85bf4fbf2fd5`）交叉核对消费者。每个函数的 JSON、完整 chunk 原字节、指令字节和入口 xref 均由原 PE 独立重算；下表只列本次补核的函数，不能把这些局部集合扩展为整个 DLL 已验证。

| 函数（原件内 RVA） | 函数 JSON SHA-256 | 完整 chunk 字节／chunk SHA-256 | 入口代码 xref |
| --- | --- | --- | --- |
| [`0x6b70`](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/functions/6b70.json) | `b0ab94cb12da33e0b4de134872c8363f7f16334eefc0f4633f65902f77cdb767` | 759 B／`5aa2be288b6f9b46cbc51abcbc3281aafa17b1c342133a1c36f90b70dd2e1d77` | `0x6b1d, 0x7268, 0x77ad` |
| [`0x6ad0`](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/functions/6ad0.json) | `11860b5bc6dfc30c00bba6b007e94f8aefb77a1c8a987d14a9ac7909638f56ca` | 151 B／`bd517f42a723ccd75fc4e74b819d29e9e340f9c8499ffb3edf6f04f663fe3590` | `0x724c, 0x7690, 0x7791, 0x797c, 0x8ead, 0x911d, 0xa083` |
| [`0x7730`](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/functions/7730.json) | `dbda1a370282e2fc6b406cc6685d8c709e0636b386767e2ebb7944b2fed589e6` | 710 B／`03d87f5402c92ef65b150d5976ca2f3db4e220824b802af3c4b367d8d4f45a6e` | `0x719d` |
| [`0x7ab0`](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/functions/7ab0.json) | `55d2ae1d02b69e7f06cca50a4604c6898b40f0154c8bbbaa944cbe739c491573` | 451 B／`94127d97337dd0f8f051ca7fe25fc846438e50146411991430294c63ef071cb2` | `0x8058` |
| [`0x7c80`](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/functions/7c80.json) | `3c3cdb93557378e45bf26af0d35bf02470d24a55a90e439e9775aff67a113e9e` | 451 B／`a13aff143e381240bc90ad7ca7cdc91d5d5a99d95808e854661ad32b87fef933` | `0x8197` |
| [`0x7ee0`](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/functions/7ee0.json) | `ca563bff11050242e0277e936f4d6546f397a65ad81fb550374babbd0d33039d` | 933 B／`86134f97e3612fcade242848f7c56809e078f338bc86bcbf20588f16e1dd7310` | `0x7950` |
| [`0x8290`](../../../.analysis/results/native/6150d86066e0e2a127a234ddc9d300ce9ac2b0cfa4c9d778fd026ea87cc1574b/functions/8290.json) | `82b12dfcf3c294a22304957bb3127515c735c877e6c5c3ac754899dc9fc4ad0b` | 2715 B／`4c026841b9e16a24997c9efa46b9cb8777bb2a7a0cbbc0be6e135aa460b2fcbe` | `0x755f, 0x765d` |

`0x6b70` 的失败顺序有两个不能省略的边界：第二次 `SetupDiGetDeviceInterfaceDetailW` 失败时先释放分配的 detail buffer，却仍沿 `v9+4` 继续组成路径；这是原二进制的潜在空指针路径，不能在 Rust 中静默改成“跳过该设备”而不留下差异。路径匹配后以 `CreateFileW(GENERIC_READ|GENERIC_WRITE, share=3, OPEN_EXISTING, FILE_FLAG_OVERLAPPED)` 打开，再 `WinUsb_Initialize` 和 `WinUsb_GetDescriptor(type=1,index=0,lang=0x409,length=0x12)` 读取 bcdDevice。Descriptor 初始化为零，初始化或读取失败只留下零；因此 `effective_pid==0` 时即使没有成功读到 descriptor 也可能被接受，其他值才会关闭该主句柄并继续枚举。找到后 `SetupDiDestroyDeviceInfoList` 与 detail buffer 都释放，但返回的主句柄由调用者拥有；`0x6ad0` 的 50 次等待只关闭它自己用来探测的成功句柄。

`0x7ab0`／`0x7c80` 对 OUT `40/81`／`40/87` 各自最多尝试十次，每次成功都立即 `WinUsb_Free` interface；随后状态查询使用 `C0/8F`、长度 1，并在成功传输且字节等于 1 时返回 2，否则继续，十次耗尽返回 3。状态缓冲在进入查询循环时才置为 `FF`，失败的 `Initialize`／`ControlTransfer` 不会重置它，但失败传输不会进入成功分支，不能把旧字节当成确认；`LengthTransferred` 只作为 out 参数接收，返回值不比较实际长度。`0x7e50` 的空 OUT `40/84` 同样只返回 WinUSB BOOL，调用者丢弃该值。

`0x8290` 的两遍写入在解析每行后把数据长度截为 `unsigned short` 传入 OUT；每次 OUT 成功才进行一次 `C0/8F` 状态轮询，状态查询前置 `FF`，成功且仍为 `FF` 时最多重复三次，随后失败返回 0；普通失败路径会释放临时字符串／数据 block，但没有关闭传入的 WinUSB 主句柄。`strtoul/strtol` 对缺失、非法或越界字段走 C++ 异常路径，不能泛化为“坏行跳过”。`0x7ee0` 的 `remove(FileName)` 返回值被丢弃，文件删除失败仍返回 2；因此“成功”只代表两遍传输和本地流 EOF 条件，不代表临时文件已经清理。

wrapper 的实际消费者是 [异步 worker `0x336b0`](../../../.analysis/results/native/54b8d53c215c7992ab949aa75db1cd92706bdd1fb4a488a1efcdb695141f68c5/functions/336b0.json)（函数 JSON SHA-256 `47359a18a0ad084eaa77433ff8c1a31b5fadd5a8fe5a179231fb31ea32e47661`，入口代码 xref `0x3abb4`）。它通过 `0x2e520` 动态保存的 `mUpgradeFirmwareProcessApi` 指针两次调用下层 ABI，第一芯片返回非 2 即记录 `0001` 并释放两条本地路径；第二芯片返回非 2 即记录 `0002` 并释放路径；两次返回 2 后才调用 `GetDeviceFWVersion` 等待并发布 `0000`，十秒超时发布 `6789`。该 worker 没有 selector 3／4 的分支、取消屏障或失败恢复调用；路径 block 的释放不等于 SDK 内部主 HID／WinUSB 句柄释放，队列销毁也不等待在途动态调用。上述 wrapper 函数和 SDK 入口的 xref／字节已独立核对，尚未执行任何 DLL、服务、设备或固件写入。

当前已证的 UMA wrapper 仅选择 0／1；标准 SDK 的 `0x16560` 显式加载的是 V25，不能接成此 SDK 的消费者。直接名称／导出扫描只界定字面候选，配置化、计算生成名称或外部间接消费者仍需追踪，不能由未找到 literal 宣称不存在。上述全部为原字节、完整函数及常量区静态核对，没有实际设备应答、DLL 功能差分或恢复验收。

### PID 691 固件包的独立 x86 SDK 与接收器更新链

[691 包专项证据](../../../.analysis/results/host-current/firmware-package-691.json)沿保留的 `691/mw` manifest 的 `Skip_FirmwareUpdater` 声明、官方包和静态提取收据，核对包内 `BlackwidowV4Pro75FirmwareUpdater.exe` 与 `Razer_Upgrade_SDK.dll` 的实际字节。前者是 ILSpy 静态恢复的托管调用方，后者是 x86、Image Base `0x10000000`、PE 版本 `1.1.1.0`；它不是当前 179 分支的 x64／`1.1.2.0` 原件。本次重核的是 2026-10-15 的官方获取观测，没有重新在线验证今日部署。包仅静态读取，未运行其窗口、安装器或刷写动作。

| 边界 | 当前包的实际契约 |
| --- | --- |
| 原生 ABI | `upgrade_firmware_process 0x4560` 为 cdecl 五参数：`u16 VID,u16 PID,ANSI path,i32 delayTime,i32 UpdateType`。`UpdateType=0/1` 分别进 `0x3f20/0x4280`，其他值返回 4；不能套用 UMA 的六参数声明或 selector 2…4 |
| 接收器第一芯片 | 托管 `backgroundWorker_DongleUpdate_DoWork` 先关闭已有 MCU 句柄（不消费 bool）、等待 1000ms，`setdevtyppe(2)`，保留并注册进度 delegate，再传 `VID/PID/nxp_path/3/0`，结束后等待 2000ms。完成处理只在返回 2 时启动第二芯片 |
| 第二芯片与重连 | 等待 800ms，重设 type／callback，传 `VID/PID/nor_path/3/1`，再等待 2000ms；之后最多约 12 秒反复 Connect／GetDevFwVersion。超时、没有句柄或 getter 返回 false **不改写之前的刷写结果**，完成处理仍按该结果是否为 2 继续 reset worker |
| 真正版本 getter | 上述托管 Connect／GetDevFwVersion 实际绑定的是另一份 `RazerMCUUpdaterInterface.dll`。本 SDK 自有 `GetDeviceFWVersion 0x44b0` 在无设备时也返回 1、且不写输出；成功则从响应 offset 9 按调用方 Size 复制，没有 payload 长度上限。不能把这两个 getter 或返回 1 的含义合并 |
| 进度与异常 | 第一芯片显示 `percent/2`，第二芯片显示 `50+percent/2`，通过同步 Control.Invoke 交 UI；该 owner 没有 clamp。第一 worker 异常设结果 0，第二 worker 设 `Result.FAIL=3`，它们不能按同一个错误值归并 |

`OpenDevice 0x1750` 选枚举中首个路径大写后同时包含四位 VID、PID 字符串且 FeatureReportByteLength=91 的接口，没有 SN、container、usage 或多设备歧义判定，使用 access=0／share=3 打开。成功和 Detail 失败分支释放 detail／设备列表，其他候选路径的 detail 分配没有在下一次迭代前释放；列表有效性只判非零。`0x1520` 发送 91 B，XOR offset 3…88 写 89；响应核 offset 2／7／8 与 status=2，不核响应 checksum。`0x3f20/0x4280/0x44b0` 内部打开的句柄没有在这些完整 owner 中 CloseHandle，不能用托管层关闭另一份 MCU 句柄证明它们已经释放。

type 0 会进入／重枚举 boot `1532:110e`，另有 `2f9a:0300→0301` 特例，最多十轮各等 1000ms。进入 boot 的应答被忽略，boot 身份检查通过后，HEX 转为主 EXE 目录下的 `new_fw.bin`，由 `0x3040` 进行实际第二遍读回比较；末尾退出命令结果被忽略。type 1 使用同 VID/PID 句柄，type 2 时命令为 `93`、其余为 `87`，必要时进入 boot 并在原句柄轮询，`0x3840` 逐行文本补至 92 B 算 CRC、检查最终 ACK，没有整镜像读回。`0x1a00/0x3040/0x3840` 的原始 Hex-Rays 有 bad-SP 警告；本节具体控制流已核汇编，不把其伪代码参数当成所有文件格式与异常语义已闭合。

`upgrade_progress_percent 0x45b0` 对 null 不写全局，所以传 null 不能注销；writer 直接调用全局 callback，没有空指针 guard。device type、callback 与 stream 全局位于 `.data` 的装载器零填充区，收据按 section 的 VirtualSize／SizeOfRawData 验证，未伪造磁盘字节。不同设备或 worker 不能各自假定拥有独立 callback。

实际 MCU 观察／恢复链另行闭合：同包 `RazerMCUUpdaterInterface.dll` 也是 x86／Image Base `0x10000000`，全局与句柄属于自己的模块。预检查接收器会设 transaction=`e0`，读 8 B，将两组版本三元组与目标作小于比较；普通句柄缺失但找到通用 boot 时，调用 ExitBootloader 后直接返回 true，没有消费其 bool。原生 getter `0x1f40` 按 HANDLE 查询，size 的低 8 位写 request offset 6，正的完整 size 用于复制响应 offset 9，仍无 payload 长度上限。它使用 `87`，只有该 **MCU 模块自己的** dongle option 非零才用 `93`；SDK 的 `setdevtyppe(2)` 不会修改它。

MCU 的 `0x1520` 与 SDK 同 RVA 函数也不是相同契约：最多三次 Set，随后直接向原 request buffer Get；不核响应命令／transaction／checksum。busy 恰在第十次读取时返回 1，最多二十次读取后返回 7，status 2／3／4 分别返回 2／3／4，其他非 busy 状态返回 5，三次 Set 失败返回 6。`BackToDefault 0x2130` 发 `01 00 0b 01`（offset 6…9）并按 exchange==2 返回；`DFUExitEx 0x2080` 却是给现有句柄 WinUsb_Initialize、发送 requestType=`40`／request=`84` 的零长控制传输、WinUsb_Free，再等 100ms，不关闭传入句柄。不能将它补写为 HID reset。部分原生 getter／恢复路径仅保证 AL 返回字节，例如 `0x1f40/0x42f0` 在 memcpy 后只改 AL；不能推广到每条成功路径，`0x4270` 的成功出口会建立完整 EAX=1。托管 P/Invoke 的 bool 没有显式 I1 返回封送声明；完整寄存器与封送的实际结果未执行，不能猜测两边 ABI 行为一致。

实际 UI 的失败、取消和恢复也不能由按钮名推断：`OnUpdatingException` 弹出 RetryCancel 后丢弃选择结果，重新初始化控件，再通过 MCU wrapper 调 ExitBootloader 且忽略 bool；FormClosing handler 为空，Cancel／Close 只改 NextPage 并关闭窗口，没有建立原生刷写取消。reset worker 不消费 ResetUserSetting 的 bool；后续版本函数仅在两芯片版本精确命中时清除更新标志，版本不符或 getter 失败没有转为 worker 失败。测试必须分别比较刷写返回、重连观测、版本匹配、默认恢复和页面完成，不能以其中一个成功替代其余。

`noforcequit` 也不是“必然更新”：`ReceivedAnnesMessage` 对每项参数用 Contains 匹配，先清 `needCloseAnne`，尝试键盘、再尝试接收器；无设备、版本读取失败、第二组三元组至少 `1.2.2`、SN 查询失败／不在指定范围或异常均直接退出 0。启动域按 SN 的 `[2:6]/[6:10]/[10:15]` 分段，允许 `2434/F886` 的 1…3000、`2435/F886` 的 3001…5000、`2434/F888` 的 1…1000，以及 `2433/F887` 的 1…2000 **扣除**完整 IL 保留的稀疏集合。常规打开时的 OLED 修复判定却仅接纳 `2433/F887` 中这个被扣除的集合，两个判定正好不同；不能合成通用“SN 在范围内”。常规 SN 查询 false 不清旧共享标志，转换异常才清 false。

键盘连接检查读到 OLED low-power=true 时先写 false，忽略 setter 结果，再读取版本；low-power 读取失败也不会阻止版本查询。reset worker 仅在上述修复标志为真时查询 scaling，查询成功且不为 100 才写 100；其返回被丢弃，也没有写后读回。原生 SN `0x42f0` 先清输出，命令 `82/92`，request 长度取 u16 的低 8 位、复制却用完整 u16；OLED 命令 class=`17`，low-power GET／SET=`93/13`，scaling=`9b/1b`，值均在 offset 9。low-power getter 仅将原值恰为 1 转 true，scaling getter 保留原 u8，setter 不校验 0…100。这些初始读取、必要写入、确认边界和失败留值均需按具体分支验证，不从本地默认值推定设备状态。

键盘分支使用同包的另一份 x86 `Ry_Online_Update_Dll_ouput_v1.0.1.dll`，不能套标准宿主 x64 同名模块。`OnStartUpdate` 在接收器条件不满足时进入 RyCore worker：等待 500ms，拆分本地 INI 路径，依次调用 userSetCfgFilePath、chipGoToBootFirmware、appUpdateFirmware，前两项非零立即停止，第三项结果原样保存，catch 设 6。只有完成器收到 0 才等待 100ms 后启动名为 NxpCore 的第二 worker。INI 的保留默认值包含 `vid_1532&pid_02B3&mi_05`、add_time=1000、encryption_en=1 和指定 enc 文件；ReadFromConfigFile 会按运行身份改写 bootdev_info，PID=00CA 则用 mi_02，写 INI 的封装却丢弃系统 API 返回值并恒返 true。默认文件或写入函数返回 true 都不能作为真实当前身份确认。

RY 原生 `0x6c40` 查询版本后使用 `(result&0xf000)`，即位 12…15，区分是否调用镜像传输 `0x5580`。拒绝分支只把大于 15 的结果改为 6，0…15 原样返回，因此仅看最终 0 不能证明发生过刷写。回调原生传入两个 DWORD 全局的地址，托管声明为两组 16 项 ushort；高 16 位落在索引 1，但消费者只取索引 0，再右移 24／20／16 的三个版本字段均为零。进度分支则取索引 0 后除以 2，没有 clamp；不应把这段显示当成完整原生版本或设备完成状态。

该 RY 的 `0x2e40/0x2f80` 分别发／读 65 B：先设定手动复位 event 为 signaled，再调用 WriteFile／ReadFile；失败后 WaitForSingleObject 返回 0 也被作为成功，没有 GetOverlappedResult、实际长度确认、取消或等待在途 I/O 结束。成功路径没有关闭 event；读取会复制预清零 buffer 的后 64 B。读取等待配置实际也是 UPDATE_ADD_WAIT/add_time，仅接受 100…3000，其余用 1000。上层 `0x5410` 核中间 ACK=`36`、最终 ACK=`34` 及命令回显，最终读取失败才重发整条命令、最多三次；该业务 ACK 契约不能消除底层完成状态未核实的问题。

键盘第二 worker 先等待 500ms，向既有键盘句柄 SetDevMode(1,0) 但忽略 bool，再等 900ms、仅连接一次 `1532:110e`。仅 `IntPtr.Zero` 设结果 5，`-1` 未被该托管检查拒绝；其他句柄值向 MCU 导出传 `(HANDLE,PID,path,delay=2,normal=true)`，这里以 0 成功，与接收器 SDK 的 2 成功不同。随后等待 1500ms、最多十次重连原 VID/PID，失败间隔 600ms，没有版本读取，也不因重连失败更改已存刷写结果。catch 只日志；若在首次结果赋值前被这个 catch 吞掉异常，e.Result 仍为 null，完成器 Convert.ToInt32(null)=0 可进入 reset。框架另有未捕获 error 时 Result getter 抛错，不属于这个分支，不能混称所有异常均成功。

MCU `upgrade_firmware_process 0x3e80` 的 normal=true 先发 class=`10`／command=`80`，要求 exchange=2，响应 offset 16…17 的 BE16 PID 非零且精确匹配；失败返回 1、PID 不符返回 2。本地 HEX 转为主 EXE 目录固定 new_fw.bin 后，发 class10／command01 擦除；`0x3670` 每块最多 64 B，以 command02 写入，再从本地文件首部开始用 command83 **读取设备数据并比较**，地址位于 offset 10…13、数据从 14 开始。这与 normal=false 的 WinUSB OUT87 携本地数据不同，后者不能写成主机读回。写失败／读失败／比较失败分别为 3／1／4；成功才发 command05，但忽略退出应答，也不查询最终版本。

`0x3670` 以非负文件大小除以 64 作为进度分母，第一遍 `min(100,50*k/D+5)`、第二遍 `min(100,45*k/D+55)`；只在此前块数严格大于 D 时改分母。小于 64 B 的文件在首次传输成功后可到无零保护的 idiv，64 B 整倍数另有零长尾轮。第一遍回调有空指针检查、第二遍直接调用；注册函数忽略 null，不提供注销。成功仅尝试在输入流析构前删除临时文件、不检查 remove；失败路径也析构本地流，但没有尝试删除临时文件。入口不释放两份路径分配或关闭传入设备句柄。可选 WinUSB helper `0x1820` 每次释放 interface，却不释放空 buffer 的 scratch；五次耗尽时可能在最后传输 BOOL=true、status=FF 的情况下仍返回 true。句柄、临时文件、传输返回、真实比较和版本确认须分别核对。

#### HEX 转换、地址与输出失败

MCU `0x2190` 的解析器内联在完整函数内；其反编译存在 bad-SP 警告，实参由汇编确定为 ECX 输入 HEX、EDX 输出 BIN，栈上传 start／end 指针，调用者先把两者清零。输入经 `0x59f0` 以 mode=1／share=64 打开，`0x6090` 按 LF 取行且不保留 LF、不额外 trim；首字符必须为冒号，空行、BOM 或前导空格均返回 2。

type 取 `[7,9)`，只精确处理 `04/00/05`：04 替换高地址文本；00 拼接高地址文本和行内四字符低地址字段，首条写 start，数据顺序追加；05 忽略自己的地址载荷，先发 callback=4，再以最后一条 00 的完整起址加其声明 count 写排他 end，随后继续读。标准 01 EOF、02／03 和未知类型跳过，EOF 后的数据仍可处理。没有记录 checksum、完整数字消费、声明长度一致或必需数据／EOF／05 检查；已打开的空输入也可返回 0，start／end 保持零。

数据截取受实际剩余行长限制；字符数先右移一位再调用 ceil，实际仍向下取整，奇数尾 nibble 丢弃。count 和字节对以无 endptr 的 strtol 解析，完整地址取 strtoll 低 32 位。地址空洞仅按相邻行四字符低地址字段的 signed32 strtol 结果与旧声明 count 计算，没有强制 u16 截断；旧、新地址任一为零就不补洞，高地址变化不参与。正差填 FF，重叠／倒序没有明确拒绝而进入负差分配路径，没有 seek 覆盖。通过首字符冒号检查后，包含冒号的整行长度为 1…6 时抛 out_of_range，整行长度为 7／8 的不完整 type 却可被跳过；00／04 还要求字段起点 9 可取，此后剩余内容可为空。所选 EH3 只有 unwind、无本地 catch，不能统称格式错误都返回 2。

转换先尝试删除输出并忽略结果，再以 mode=0x22 打开固定 new_fw.bin；打开失败只设 output failbit，随后却误查 input FILE。两处写入及关闭错误均未改变正常返回 0，返回 0 不能证明镜像有效或输出成功。坏前缀／解析失败没有删除已产生的部分输出；每条数据记录的裸 buffer 未见配对 free，gap buffer 有释放。流和字符串的 unwind 已保留；后续升级成功才另行尝试删除临时文件，也不核结果。

#### 可选 WinUSB 分支的发现、两遍传输与重试

托管键盘调用固定传 normal=true；normal=false 是另一个可选导出分支，它跳过 HID PID 查询，直接消费调用者 HANDLE，不调用 OpenWinUsbBootloader。完整捕获中 `0x19d0/0x16c0/0x1770` 的 finder／控制传输只有导出或 data 引用，没有内部 code caller；`0x1820` 的内部调用仅来自擦除和两遍数据命令，不能把这些导出排列成已接通的托管更新链。

finder `0x19d0` 使用 GUID `c9348766-d27c-41ee-b2be-ea9412f72b52`，以两组四位大写十六进制子串匹配 VID／PID，用读写／share3／OVERLAPPED 打开。它取 Device Descriptor offset 12 的 u16 bcdDevice，或路径 REV_ 后十六进制的低 16 位；不核描述符实际长度。初始化／描述符失败时 AX=0 仍参与比较，selector=0 可因此接纳；描述符失败没有 Free，成功才释放 interface。不匹配 VID／PID 的 detail 分配未释放，detail 失败或枚举结束有重复 DestroyDeviceInfoList，返回的文件 HANDLE 交给调用者。

normal=false 从转换所得本地文件按每 512 地址发 OUT40／82 擦除，payload 长度零；start≤end 的条件包含恰好终点。两遍分别发 OUT40／81、OUT40／87，均来自本地文件每 64 B／尾长读取，第二遍 clear／seekg(0) 后重新读文件，却绕过主机的逐字节比较。wValue／wIndex 只传地址低 16 位，没有用 payload 补传高位；调用者 delay 不进入这条 WinUSB 等待。因此 OUT87 只能按其命令与 ACK 契约核对，不能称为主机镜像读回。

`0x1820` 每轮 OUT 初始化成功才传输并 Free；仅 OUT 返回真才等待 20ms、将同一 buffer[0] 写成 FF 并进入 IN。IN 初始化成功才发 `c0/8f`、value／index=0、length=1 并 Free；IN 返回真且状态非 FF 才提前成功，不要求状态 2。读失败或仍 FF 时会重发 OUT，**没有恢复已经被改写的 payload 首字节**。五轮耗尽返回末轮保存的布尔状态的归一化低字节，初始化失败会将其清为假；末次 IN 传输真而 status=FF 仍可返回真，两次实际传输长度均未核。OUT81／87 失败分别映射 3／1，normal=false 不进入本地主机比较失败 4；只有传输函数返回 0 才调用 DFUExitEx，而退出结果仍被丢弃。

输入流成功构造后，普通返回及 C++ 异常 unwind 路径设有析构，`0x34f0/0x5960` 可达 fclose；writer 的 EH3 没有本地 catch 将异常转为升级结果。分配链使用会抛 bad_alloc 的 new，不能写成 calloc 或可恢复 null；scratch、两份路径和设备 HANDLE 没有完整释放所有者。stat 返回、实际 gcount 和 seekg 结果未核，零进度分母、零长尾轮及 callback 限制同样适用；本轮未执行该可选分支。

#### 接收器 Nordic 文本、CRC 与进度

同包 SDK 的 `0x4280` 先把 CRC16 全局设为 FFFF，再调用 `0x3840`。其输入是 getline 取得的原行字节，未检查冒号、HEX 记录或 EOF 类型；先统计流中的 LF，以低 32 位的 LF 数加一作为进度分母，再忽略 seekg 结果回到起点。每行复制到预清零区域，恰取前 92 B 累积未反射的 CRC16／1021，初值 FFFF、无最终异或；补零参与 CRC，LF 不参与。它不是 HID 报文本身的 XOR checksum。

原件没有 92 B 或报文 payload 长度上界：CRC 前的 memcpy 使用完整正行长，报文复制却只取低 8 位。报文 offset 6 为低 8 位的 `lineLength+7`，7／8=`0a/02`，9 为调用方 boot 字段，15 为低 8 位行长，数据从 16 开始；行长 74 已将数据放到随后会被 exchange 覆盖的 checksum offset 89，更长行还可能越过本地存储。每行要求 exchange=2 且响应 offset 14=0；随后回调 `min(95,floor(95*k/(LF数+1)))`，没有 callback 空值检查，再等 10ms。

结束包 offset 6…8=`0a 0a 02`、15…16=`03 3c`、17／18 为 CRC 高／低字节；exchange=2 且响应 offset 14=0 才回调 100、返回 2。可读空文件可带 CRC=FFFF 进入结束包；getline 因正常 EOF 或错误返回 false 都走这里，前缀 CRC 的 ACK 不能证明完整文件被读取。上层返回 2 后发送退出，但不消费退出应答，也不确认新版本。

已将汇编中的 CRC 紧凑公式与独立标准库算法及固定 `123456789→29b1` 校验值核对，并保留六个仅含 LF 的静态算例。例如 `abc` 和 `abc\n` 的 CRC 都为 BD12，最后一条行进度却分别是 95 与 47；可读空输入没有行进度、结束前 CRC 为 FFFF。这些只是静态算术复核，没有执行 DLL、文件流、传输或 Rust 功能测试，不能充当真实设备应答夹具。

#### 接收器 SDK 的另一条 HEX 与读回链

同包 SDK 的 `0x1a00` 与 MCU `0x2190` 的 ABI 和结果不同：调用者 `0x3f20` 的汇编确定 ECX=输入 HEX、EDX=start 指针、栈=end 指针，输出路径由转换器内部构造；正常返回 2，输入打开或前缀错误返回 4。转换开始 callback=1，每条 00 数据记录 callback=2，05 先 callback=4 再计算排他 end，读行结束 callback=5，均无 null guard。00／04／05 的字段、signed32 补洞、奇数 nibble、短行异常和 EOF 后继续读取的限制分别核自该原件，不能直接复用 MCU 的返回码或回调。

转换器通过主 EXE 路径与 CP_ACP 构造全局固定 new_fw.bin；路径获取、字符转换、反斜杠与长度没有完整错误检查。remove 不核结果，输出打开失败仅设 failbit，随后却再次检查 input FILE；写入、关闭或读流错误仍可能走 callback=5／返回 2。空输入也可返回 2，不能据此确认镜像有效。裸路径和每条数据分配未见配对释放，gap buffer 有 free；异常保留部分文件。全局 locale 被改变，没有恢复原值。converter／writer 的 EH3 分别为 maxState=18／4，均无本地 try/catch；有 stream／string／locale unwind 不表示异常会变成返回码。

writer `0x3040` 的汇编确定 ECX=HANDLE、EDX=start、栈=delay；打开失败返回 4，stat 的 signed32 size、实际 gcount 与 seekg 结果不核。第一遍 command02 将 BE32 地址写 offset 10…13、数据从 14 开始；第二遍 command83 读取设备 offset 14 起数据并始终比较 64 B，尾块包含双方预清零部分。累计已处理字节数是 DWORD，只有尾长取低 8 位，坏-SP 伪码中的 char 累计量不可信。比较不符返回 3；exchange 失败保留其返回值。整 64 B 文件两遍均有零长尾轮，两处进度 idiv 无零保护；成功先关闭输入流，再尝试 remove，结果仍为 2。这与 MCU 在析构前删除的顺序不同，也没有最终版本、取消或原固件恢复确认。

#### 文件流、异常与 CRT 依赖边界

[691 包专项证据](../../../.analysis/results/host-current/firmware-package-691.json)中的 `package_native_file_streams` 分别保留本包 SDK／MCU 的完整文件流函数、逐指针虚表、异常表和汇编断言。MCU 转换器 `0x2190` 的 FuncInfo `0x8e20` 有 21 个 unwind state；SDK 转换器 `0x1a00` 的 FuncInfo `0x8db8` 有 18 个，两者均无本地 catch。这只能说明转换器本体的字段切片、裸缓冲分配或回调异常没有在该层统一转换成返回码，不能扩展为整条读行链没有 catch。

MCU `getline 0x6090` 的 EH `0x9060` 和 SDK `getline 0x5bd0` 的 EH `0x9020` 各为完整 112 B，由 36 B FuncInfo、40 B unwind map、20 B try map 和 16 B handler 组成。两者均有 `tryLow=2/tryHigh=2/catchHigh=3` 的 catch-all；实际 handler 分别位于 `0x61c3`／`0x5d03`，调用 `setstate(badbit=4,Reraise=true)` 后回到正常状态处理和解锁路径。伪代码没有显示这段 catch body，判断依据为原始 EH 和完整汇编。是否重抛取决于实际 CRT 的异常掩码；若异常被转换为流失败，转换器循环可能结束并继续结束回调和正常返回。此保护范围只覆盖 `getline` 内的取行／追加循环，不能保护其后的 HEX 字段解析和分配。

两份 filebuf 虚表各有 15 槽：MCU 位于 `0x8388`，SDK 位于 `0x8338`，共 60 B。文件锁／解锁、读写、定位及 codecvt 的目标均已逐项对应本包函数；相邻第 16 DWORD 属于下一份 RTTI，流 sentry 清理的 `vtable+8` 也是解锁，不能当作设备回调。FILE 保存于对象 `+76`，codecvt facet 于 `+56`，mbstate 于 `+64/+68`。open 经 `_Fiopen(path,mode,64)` 后按流 locale 选择 codecvt：`always_noconv` 为真时清空 facet；无 facet 的读写走 `fgetc/fwrite`，有 facet 时走标准转换分支，均不构成额外的设备命令。

MCU close `0x5960` 先调用 `0x5890`，SDK close `0x5490` 先调用 `0x53c0`，分别处理 overflow／unshift／尾部写入；helper 失败仍会调用 fclose，随后清空 FILE、facet 与 mbstate。输入流先打开，再以空 locale 名和 category mask=63 构造 locale、调用 `std::locale::global`，最后打开输出流；未见恢复旧 global locale。临时 locale 对象的析构不能代替全局恢复。字符串辅助函数的 15 B SSO／堆路径、析构与裸 data buffer 分开核对；分配失败可抛异常，字符串析构不能证明裸缓冲已有释放。

另一官方部署保留的 x86 MSVCP140 仅作为独立依赖候选：其 `_Fiopen` 表将 mode=1／0x21／0x22 分别映射为 `r/rb/wb`；`codecvt<char,char>` 的 `always_noconv` 槽返回真，`ios_base::_Init` 将异常掩码置零。候选的导出和汇编已独立核对，IDA 的 COMDAT 别名不作为类型依据。但目前存在不同字节的 x86 候选，尚未证明 PID 691 实际加载哪份 CRT；实际 UCRT、加载器选择、进程 locale、窄字符路径和文本模式 CRLF／0x1a 行为仍未执行验证。不得将候选样本的默认状态直接套为本包实际行为，也不得把 HEX 解析器不 trim 等同于文件层不会转换输入。

同份报告的 `package_dependency_binding` 进一步核对完整 ZIP 的 18 个成员，均与提取原件逐字节相同；包中没有 CRT DLL 或外部 `.config/.manifest`，当前 manifest 的 5 个资源声明也没有单列 CRT。SDK／MCU 各普通导入 MSVCP140 的 47 个符号、VCRUNTIME140 的 12 个符号和 7 个 UCRT API-set 契约的 36 个符号；MCU 另有 WINUSB／SHELL32。RY 没有这些普通／延迟动态 CRT 导入，不能将 SDK 的依赖结论直接套给它，也不能据此称其内嵌 CRT 已完整逆向。

SDK、MCU、RY、FWUpdaterDLL 与托管主 EXE 的 64 个普通导入描述符中，TimeDateStamp／ForwarderChain 均为零，磁盘 ILT 与 IAT 相同，Bound Import／Delay Import 目录为空；包内其他 PE 和外壳也单独核对，共 16 个 PE／75 个描述符。内嵌 RT_MANIFEST 经 XML 解析没有有效的 `dependentAssembly/file` 绑定，注释里的样例不算声明。这些实际字段仍不能选定 691 的 CRT 字节。两个 x86 官方部署候选版本分别为 14.0.23918.0／14.0.24212.0，均覆盖全部 47＋12 个所需导出；两者 MSVCP140 又各延迟导入 CONCRT140 的 12 个符号，而各自保留包未附该文件。另三个包中的同名 CRT 为 x64，不能直接满足 x86 导入。实际加载器、API-set 与已执行的延迟依赖须继续在显式对照中核实，符号匹配不构成功能一致。

#### 同包 MCU 的其余导出与 48 B 协议

PE 导出目录现逐项对应 SDK 的 7 个、MCU 的 24 个和 RY 的 3 个完整导出函数体。MCU 新增的 driver、Dock MAC 与 PS5 导出在本份完整托管 IL 中没有同名引用，原生完整捕获也只有导出 data 引用；这仅界定当前包调用方，不表示整个项目没有其他消费者，也不将它们当作 PID 691 已挂载功能。

`InstallBootLoaderDriver 0x1ca0`／`UninstallBootLoaderDriver 0x1cc0` 仅以调用者 ANSI 路径调用 ShellExecuteA，verb=open，参数分别为 ` /s`／` /U isousb.inf /s`，显示参数分别为 0／5。它们原样返回 HINSTANCE，没有检查大于 32、等待退出、确认驱动状态或恢复；函数名不能证明安装完成，本次也未运行安装动作。

另一 helper `0x4490` 使用 48 B Feature Report，保存 command=offset3，再写前缀 `03 5a a5`；最多三次 SET。command09 只要 SET 成功就返回 true，reset_ps5 因而没有 GET 或重连确认。其他命令先等待，再最多二十次 GET，只匹配响应 offset0=command 与 offset1=2；**恰好第三次 GET 为 true 且不匹配时提前失败，第三次为 false 则继续**，不能概括为三次读取上限。没有响应 checksum 或有效载荷长度核对。

`GetDockMACAddress 0x49c0` 用 command1d、offset9=6／11=95，成功后将响应 `[9,15)` 原样复制为六字节；失败保留调用者旧输出，不做文本或端序转换，未核输出指针／容量。成功只设置 AL，不能自行补成完整 EAX 布尔契约。

`upgrade_ps5_process 0x4880` 先以 command1a 查询和准备，再调用 `0x45a0`：ECX=HANDLE、EDX=delay、栈为首次响应 offset13 起的 DWORD／文件路径／零扩展的低 8 位 flag；writer 只消费 token 低字节，flag 保留 0…255，不是布尔归一化。flag=0 时读弃前 32768 B，再以 28 B 读取原文件；command1a 的 offset10…11=`0a 02`，12=query 字节、13…16=BE32 地址、18=低 8 位尾长、19 起为数据。没有镜像格式、CRC、签名或设备镜像读回。小文件在跳过后可出现负尾长取低 8 位，例如 32767 B 文件可产生长度 255 并直接 memcpy，超过 28 B 源区域与 48 B 报文；stat、gcount 和 EOF 边界没有预先验证。

这条 writer 在传输成功后才用 32 位乘法和 signed idiv 更新进度，没有零分母保护；空文件可在第一次除法失败，flag=0、大小 32768 B 时可在进度重映射的分母 `100−trunc(3276800/S)` 遇到零。正大小 32444…32768 的商均为 100，但较小值可能先进入越界复制，不能假定都稳定到达同一异常。剩余长度 `S−skip` 为 28 的倍数时可能多发零长尾轮；skip 为 0 或 32768。每块 callback 有 null guard，EOF 的 callback100 却直接调用。正常读流结束返回 0 后还发结束命令，结束应答 true／false 分别映射完整 EAX 的 0／1；打开和传输失败为 2／3。输入流构造后的正常／异常路径有析构，但没有取消、callback 注销、调用者 HANDLE 关闭或重连／最终版本确认。以上均为静态协议事实，未执行该导出或刷写。

#### 同包 RY 的配置、镜像与实际报文

包内 x86 RY 的配置、编码与镜像链已补到内部 owner。`0x2af0` 将两个全局 CString 的目录和 INI 文件名直接拼接，`GetPrivateProfileStringA` 使用空默认值和 100 字节缓冲，第三个名义 CString 参数只参与释放，既不是 API 默认值也不是实际文件来源；截断、缺键和读取错误没有分别建模。`0x36f0` 对 bootdev_info 转小写、删除所有 ASCII 空格并截去首个分号之后的内容，再交给现有 HID substring 匹配。文件与 INI 全链使用 ANSI API，不能声称支持一个已经验证的 UTF-8 路径契约。

文件名先严格核对末四字符 `.enc`／`.bin`。随后 `Right(19) → Left(6)` 取版本，要求首字符 v 或 V，改成 A 并删除所有点，经过十六进制检验再按 `%04x` 解析；这使 v1.2.2 得到 A122，并非通用语义版本解析。再次读取文件名，以 `Right(12) → Left(8)` 取 `%08x` 校验值。`0x3210` 被 IDA 错标为时区函数且反编译有 bad-SP 警告，逐指令核实其真实行为是完整匹配 `[0-9a-fA-F]+`，再将每两个字符写成 ECX 指向的字节；奇数末尾半字节被忽略。encryption_en 的 `%x` 转换只有返回恰好 0 才强制 1，EOF 不走该分支；它选择扩展名，本机这条写入链没有解密镜像，报文第 6 字节始终为 1。

`0x36f0` 向 HID 发送 12 字节 `30 00 00 00 04 00 00 00 67 65 74 76`，只核回复命令 30h 和 offset4 的 16 位长度 16，以 offset20／16 的大端 16 位分别取 app／boot 版本。目标版本小于等于设备 app 版本时，它将 F000 标记加在返回值高 16 位；但导出 `0x6c40` 判断的是低 16 位 boot 的 bits12…15，不能描述成“已最新自动跳过”或防降级保证。传输失败至多三次，只有写失败额外 Sleep100；语义错回复直接返回 2。查询成功及语义错回复没有关闭已开的设备句柄，进入 `0x5580` 后会被新句柄覆盖，因此当前官方原件并不具备完整的旧查询句柄释放链。

`0x5580` 先按 512 字节读取整个文件（含 32 字节头），按小端 DWORD 异或并核对文件名校验；打开失败返回 7，校验不一致关闭文件后返回 8。它不检查 fseek／ftell／ferror、最小 32 字节长度或四字节对齐；不齐尾块的 DWORD 读取会包含缓冲中的旧字节，不能伪造一个稳定的官方期望值。之后将 ftell 减 32 作为 body 长度，重定位并发送原始头与 body。APP_START_ADDR/app_start 支持精确小写 0x 前缀或十进制，解析失败、零或大小判断失败均发送 FF 哨兵并继续；包内默认 INI 没有该键，不能据此推断实际 flash 地址。

31h 开始报文的 8 字节头依次为 command、低 8 位 sequence、低 16 位 bodylen/4、payload 长度、固定 1、start page；payload 为 bodylen／目标版本两个小端 DWORD、文件头及可选设备代码。boot 低 12 位小于 105h 不附代码；105h…121h 附 1aec00 的 440 字节，122h 及以上附 1aea60 的 412 字节，各增加 4 字节代码长度和 4 字节代码 XOR，实际附加量分别 448／420 字节。这些内置设备代码的原始字节已保留，设备侧的执行与解密语义仍单独未验证。32h 按 512 字节发送剩余 body，33h 发送 body XOR 与本地年月日时分秒；终态只核现有 5410 逻辑应答，没有刷后版本或 flash 字节读回。

RY 的状态 5 同时表示成功与错误，进度计算还给开始报文预记一块工作量，最后 33h 未确认前即可达到 100；33h 失败可返回 9 而保留已到 100 的进度。普通终态路径关闭当前 FILE 与当前设备句柄，但查询旧句柄已可能丢失；C++ EH 表的 34／35 个状态均只释放 CString，没有 FILE／HANDLE unwind owner，也没有 catch 将分配、字符串或 regex 异常转换成普通错误码。不能将回调状态 5、进度 100 或最后返回 0 单独当成全部写入与恢复已经完成。

实际 RY 镜像来自同包 SelenaUpdater.resources 的内置资源，`Info` 构造器按 RY Core FW Size 顺序取 RY Core FW Sector 并写成本地文件，再更新 INI；本次按 .resources v2 原字节静态核对全部名称哈希和数据边界，不实例化 user type。37 个扇区合计 37,408 字节，其小端 XOR32 为 EBB79E76，与资源文件名一致；32 字节头之后的 body 恰为 73 个 512 字节块，body XOR32 为 C9A9E961。该一致仅闭合包内静态输入与校验链；Rust 独立实现、模拟测试、官方 DLL 实际功能和真实设备写入／恢复验收分别保持未验证。

资源写出本身还有独立失败语义：`ParseFWFile` 先以 FileMode.Create 截断目标，再建立 BinaryWriter／ResourceSet；循环只要求累计扇区字节达到或超过声明 Size，没有 exact-size、扇区长度或写后 hash 核对。原包此次恰好相等是独立静态核验结果，不能替该函数补出不存在的校验。异常路径只日志并返回 false，没有 finally 保证关闭已建立的流／writer／ResourceSet 或删除部分文件；Info 构造器自身的 ResourceSet 也未显式 Close，前一 NOR 文件生成失败会直接 return，不继续生成 RY 或更新 INI。

已为该包的 37,408 B 镜像保留静态报文向量：31h 无附加代码／旧代码／新代码的命令长分别为 48／496／468 B，按 64 B HID payload 分为 1／8／8 片；首个 32h 头为 `32018024000201ff`，命令长 520 B，分九片且末片 8 B。73 个 body 块后，33h 头与 body XOR 为 `334a80240c0001ff61e9a9c9`；时间尾部由实际本地时钟决定，没有伪造固定样本。传完第 72 块已可达到 100％，最后一块未夹制的商为 101；这些是结构构造与独立字节字面量的静态核对，没有执行 DLL／传输或 Rust 功能测试。

#### 691 独立窗口的挂载、交互与完成消费者

包内独立窗口仍有自己的完整 UI 调用链，不能由外层固件页或刷写函数结果代替。`Program.Main` 建立 `appContextDevice`，后者预建 Guide、ModeSel、Main、Success 并先显示 Guide。页面关闭先 Dispose 当前实例，再按 `NextPage` 选择或重建下一窗体，并传递原窗口位置；最终 Close 分支调用当前进程 Kill。Guide 的 Next 启动五项本地 StopSvc 调用及退出已有本地宿主进程的 worker，未消费这些 bool 或完成器 Error／Cancelled 就进入模式选择；这只能证明调度，不能证明服务已停或已恢复。模式页 Load 清空有线／无线选择并禁用 Next，点击对应图片后才建立互斥选择、保留高亮并允许进入更新页。

Main 设计基准为 720×600、无系统边框、居中、禁最小化与最大化，按 Font／6×13 自动缩放；主体使用 Microsoft Sans Serif 12，左上标题为 RazerF5 14、位置 `(18,5)`、AutoSize。Load 设置当前语言和大写产品标题，再依次初始化控件、注册设备监听／改写必要 INI、按无线且阶段 1 选择接收器预检，否则预检键盘。无线阶段 1 使用 `PlugHyperPollingWirelessDongle` 并隐藏第二条说明；有线／阶段 2 使用 Wired_1 背景和两条键盘说明。两张当前背景均为原件内 720×600 PNG。主题绿为 `#44d62c`，按钮文字为 `#222222`；按钮源图为 90×27，控件设计尺寸为 90×30 且 AutoSize。Shown 会按实际窗口、实际按钮尺寸重新定位：主按钮右／下各留 40，Back 在左侧再隔 20；标题超过窗口宽减 48 时逐次缩小字号。不能直接把设计器 `(604,540)` 当成所有语言与 DPI 的最终按钮位置。

| Main 控件 | 设计器矩形 `(x,y,w,h)` |
| --- | --- |
| 两条连接说明 | `(300,88,348,88)`／`(300,269,348,46)` |
| 主提示／第二提示 | `(2,381,716,23)`／`(78,423,568,23)` |
| Updating／百分比 | `(34,429,101,20)`／`(616,429,69,18)` |
| 进度条 | `(35,452,651,15)` |
| 当前／目标版本 | `(37,501,299,30)`／`(37,536,299,30)` |
| 右上关闭 | `(671,1,48,32)` |

`InitializeControlDatas` 清空／隐藏状态、进度和版本字段，重设 Back／Update 文案并禁用 Update，却不恢复 Back 的 Visible／Enabled。接收器已经最新会隐藏 Back、把主按钮变为 Next；此时断开设备再初始化仍会保留隐藏的 Back，只有进入阶段 2 的 Next 分支显式设回可见。键盘已经最新或高于目标时只显示无需更新并允许 Back，不显式启用 Update；从初始化进入该分支时主按钮仍禁用。Update／Next 与 Back／Close 的分派比较当前本地化按钮 Text。`MyButton.Enabled` 本身只修改自有布尔值、TabStop、光标和文字颜色，点击／双击／鼠标事件 override 才按该布尔值拦截；真正的 `base.Enabled` 属于另一个 EnabledSet 属性。不能把两者当成同一 Windows 控件状态。Update click 先禁用两个按钮，OnStartUpdate 再设 wait cursor／isUpdating、检查 worker IsBusy；选中 worker 已 busy 时没有另一路启动或恢复。

进度也分两层：窗口回调算术与百分比文本没有钳制，第一接收器芯片用 `percent/2`、其他 MCU 段用 `50+percent/2`，RY 取首个 ushort 后除以 2；但实际依赖 `CustomProgressBar.dll` 的 Value setter **会把内部数值钳到 0…100**，绘图按这个内部值计算宽度，使用 `#333333` 背景和 `#44d62c` 前景。因此超界回调可能使文本与条形宽度不同，不能笼统称整个进度控件没有 clamp。Label 和回调通过同步 Invoke 更新；reset worker 更把 reset、延时、OLED 修复和版本读回的整段闭包同步 Invoke 到窗口线程。

预检接收器的 current 显示返回字节 `[4:7]` 的 NOR 三元组，target 却是 dongle_nxp_version；更新后版本页改显示 `[0:3]` 的 NXP 三元组，两个 current 消费者并不相同。reset 优先取非零键盘句柄且键盘需更新的分支，忽略 ResetUserSetting／OLED 修复 bool，查询版本后仍固定 Result=0；否则对非零接收器句柄且需更新设 transaction=e0、reset、查版本并 Result=1。两个条件均不满足时 Result 保持 null，完成器 ToInt32(null)=0 仍进入成功页；内部 catch 设 5 后完成器没有 else 处理。未捕获 Error／Cancelled 时访问 Result 的框架异常另算，不能与 null 混淆。版本 getter 失败或两芯片未精确匹配目标，也不阻止 CheckUpdatingVersion 显示“已更新最新”、允许 Next、隐藏 Back。键盘 Result=0 直接关闭到 Success，接收器 Result=1 留在 Main 等 Next。最终 Success_Load 不再读取设备，仅按选择显示 Common.Info 的接收器 NXP／键盘 NOR **固定目标**，无线选择还会列出预检已最新而未实际刷写的接收器。必须分别验证写入结果、重连、真实版本、恢复、完成导航和目标文案。

关闭按钮在更新中没有被禁用，FormClosing 的完整 IL 仅 ret；Back／Close 没有建立原生刷写取消。Main Dispose 没有显式取消／等待 worker、退订设备事件、停止监听或关闭设备句柄。监听器 Start 分配过滤器并注册通知后只把注册句柄存局部，用 IntPtr.Equals(null) 判运行标志，既未保存到 hCtrl，也没有在该 owner 释放过滤器；Stop 使用仍为构造零值的 hCtrl。该包 EXE 的完整 IL 中，没有到 DeviceListener.Stop／BackgroundWorker.CancelAsync 的直接 call／callvirt 或 delegate ldftn／ldvirtftn 引用，不外推其他程序集或动态调用。Process 按 WM_DEVICECHANGE 的到达／移除、前 40 个路径字符及 VID／PID 子串发事件，Main 在 isUpdating=true 时全部忽略；boot 到达分支新开局部 boot 句柄，却检查旧 mDevHandle 后才决定是否向局部句柄 ExitBootloader。上述是当前原件的具体边界，不能将关闭窗口、Dispose 或进程退出记成已验证的取消、恢复或释放链。

Guide 是唯一显式设 TopMost=true 的页面，两按钮设计器 TabStop=false，Load 只设文字和颜色，没有调用自有 Enabled=true 恢复 TabStop。ModeSel 的 Cancel.Enabled=true、Next=false 分别令 TabStop 为 true／false，选择图片后 Next 才为 true；Success 的 Close.Enabled=true 令其 TabStop=true，Main 则随预检与阶段变化。MyButton 统一隐藏 ShowFocusCues，不能据此声称按钮没有默认键盘行为。ApplicationContext 自持 currForm、订阅 Closed 并调用无参数 Show；重建时不写回原 deviceForms 列表，只显式传递窗口位置。在本份 EXE 的直接 IL 引用中，未见设置 Form.Owner／ApplicationContext.MainForm、Focus／Activate／ActiveControl、AcceptButton／CancelButton、KeyPreview 或自定义键盘事件绑定；框架默认行为与真实焦点／激活结果仍须另行验收。

失败提示另走 WinForms MessageBox：OnUpdatingException 清 isUpdating，再用不带显式 owner 的五参数 Show，按钮为 RetryCancel、图标 Exclamation、默认第一按钮，并丢弃 DialogResult。它没有自定义焦点保存／恢复，默认模态和键盘行为不能从 MyButton 的实现推断。

语言选择来自 GetSystemDefaultLangID 的低字节匹配；中文再按高字节 04／0c 选择 zh-CHT，其余中文匹配选择 zh-CN，未知语言走空 culture 的 neutral 英文。表中的 Font 字段没有进入这些页面的字体设置。包内九份语言卫星各有 36 个 ResourceStr 词条，原始 PE 中的字符串均与反编译 resx 逐项相同，但都缺 neutral 中的 dayoneprompt；Guide 的该长文走 neutral 回退。资源证据还逐项绑定了 23 个图片序列化 payload 和四个窗体 Icon 的原始 PE 范围；全程只读字节，未反序列化对象，22 个 PNG 的 IHDR、完整 chunk 边界和 CRC 已静态核对。布局、各语言文案、worker 与设备事件竞争、关闭及清理的实际 DLL／设备结果仍未执行对照，保持未验证。

专项完整保留所选原生函数、逐字节核验指令、完整 IL 方法及反编译文件，输入／产物身份与数量见当前收据。IL 的方法 header／code 长度和原 PE 字节另行核对，不冒充独立 IL 指令反汇编；异常表保留于完整 IL。bad-SP 告警与未展开的底层库、CRT／locale／异常及间接分支继续保留，没有新增 Rust 实现、运行纯／模拟功能测试或实际 DLL 对照，不能将导出覆盖推广为整个升级包或设备功能完成。

## 固件页面、状态机与样式

页面不是简单的进度条。专项证据保留 `ao [1885792,1890714)` 状态机、`ro [1890715,1891670)` 生命周期、`Jr [1880881,1883724)` 准备、`Qr [1883724,1884349)` 升级、`Zr [1885189,1885393)` 切换、`eo [1885393,1885609)` 关闭，以及实际挂载树、reducer、App effects、结果消费者的完整 AST。Ei 在 CUSTOM 且 source 包含 `usb_5426_179` 时直接进入 Prepare，否则从 Launch 开始；更新项数量来自连接／能力分支，显式 false 会排除对应项，不是实测升级数量。

| 状态 | 实际转换与完成边界 |
| --- | --- |
| Launch | Next 遇 isExceeded guard 留在 Launch，否则 → Prepare；Close → Close |
| Prepare | Jr 判定相同版本、缺目标版本或已有 dongle 成功结果时设置 skipUpgrade，状态机可直接把该项记 succeed、进入 Partial／Done，不执行写入；其他成功 → Ready，失败／异常／SKip → SkipFail |
| Ready | Next → Upgrade，SKip → SkipFail |
| Upgrade | 调 Qr；失败 → SkipFail，成功且有下一项 → Partial，否则 Done。没有 SKip／Close／Restart 处理器 |
| Partial | invoke revert，Next 增加索引 → Switch；SKip → SkipFail，Close → Close |
| Switch | Zr 保存旧连接、等 500 ms 即返回 success，没有探测和自动转换；Next → Prepare，SKip → SkipFail |
| Done／SkipFail | invoke revert，onDone 只日志，不检查 false、不设 onError，也不等待恢复才开放按钮；底层均接受 Close／Restart，但 Done 页面隐藏 StartOver，SkipFail 页面提供该按钮 |
| Close | eo 排队 200 ms 后调用 closeWindow，返回 status；onDone 不核 status 便 finalExit，不能证明实际关窗或恢复完成 |

ao 的 Upgrade onDone 仅将 `installStatus===fail` 判为失败，缺失／空状态仍可进入 Partial／Done；结果 action 却以 `status||fail` 将其归为 failed。Qr 当前只产 success／fail，但异常响应测试须分别比较状态目标与 previousUpgradeRes，不能将状态机目标当作严格响应验证。

非 skip 准备包含每 200 ms 增长的模拟进度，依次 isolate MW、Er、Kr.init，全 true 后置 100，再等五秒。Er 仅在 source 包含 `usb_5426_179` 时发起原版在线获取、保存与解包；本项目须移除该远程编排，独立实现本地固件文件准备，不能把静态逆向依据当作产品联网授权。准备计时不能作为固件进度、设备 ACK 或对照测试读回期望。Do 的 UPDATE 先 await Xr，再禁用按钮并发送 Next；SKIP 禁用自身并发送 SKip。离开 invoke Promise 不会停止已运行的异步 JS／native，链中未见 AbortSignal；不能把页面跳过或关闭等同于设备任务取消。

Partial 显示前一结果；Switch 的 Next 另要求连接方式改变、按 SN 或限定 related-PID 分支恰好匹配一台设备。Zr 的 500 ms success 不满足这个设备发现条件。Partial 将大写 connectType 与小写 `dongle` 比较，可能走到另一文案分支，此差异须按原件保留和验证。

Fo 的成功列表按连接与能力分类，可包含没有写入的 skip 项；Done entry 的 `updateCurrentFWVersion [1849151,1849875)` **只是把目标版本赋给 Redux 中的 current 字段，没有设备查询**。本项目必须将此页面流程结果、草稿／持久化和设备实测分开，不能用它冒充读取结果，也不能用它证明前述原生升级已与目标版本一致。

warning 的 WAIT 没有动作，UPGRADING 有 Continue，断连／隔离／更新错误有 CANCEL／RETRY。Cancel 清错误并发 Close，在没有该 handler 的状态会被忽略；Retry／Continue 清错误并设 ignoreError=true，没有重新 Prepare／Restart。Restart 由 ao actions 调 `no.resetIndex=assign({currentItem:1})`，只重置机器 context 的索引，items 与 previousUpgradeRes 不清；ro.init 对已有 singleton 忽略新 context，可继续影响 skip。to 的 isExceeded 是 `currentItem>items`，isProceed 是 `currentItem+1<=items`。uiReducer 另在状态转入 Launch 时重置 UI／初始连接方向，并在 SkipFail → Launch 时异步调用 Ei(false)，不能与机器 context 的重置混为一谈。版本 effect 仅在值 truthy 时覆盖，缺失可能留下旧值。断连检查跳过 Prepare／Ready／Upgrade／Switch；只有 Done／Partial／SkipFail 对非唯一匹配最多三次、间隔 500 ms，Launch 零匹配立即报断连，多个匹配则不进入该报错分支。beforeunload 只发起 `_r().then(...)` 与清理，不等待原生取消或设备恢复。

产品图 Nr 按 sidepad → wrist → wristrest → mat 覆盖，再走 PID／edition／layout fallback；图片 effect 没有 generation／cancel。共享 on／sn 工具栏实际挂载，refresh 禁用，前后按钮为空回调；完整挂载后代和静态键盘／焦点语义见下节，各资源分支与实际窗口行为另行验收。

样式来自 [main.3e3077d6.css](../../../.analysis/inputs/frontend/synapse-update-fw-current/raw/apps.razer.com/synapse/update-fw/static/css/main.3e3077d6.css)，包含原生 CSS nesting。专项证据保存完整选中平衡块及其原字节，边界校验不是 CSS AST／声明有效性或浏览器计算样式验收。

| 区域与 CSS 字节范围 | 原始尺寸与状态 |
| --- | --- |
| root／主容器 `[390575,391384)` | 字号 14 px、按钮 12 px，文字 `#ccc`、强调 `#44d62c`、背景 `#222`；主容器纵向满宽高，内层 margin 12 px、顶部 2 px 黑边，内容宽 640 px／顶部 32 px |
| 产品／footer `[24633,25621)` | 产品图 466×250、cover；按钮 gap 20、90×27，最后按钮绿色、其他灰色；hover／focus opacity .7，disabled .5 且不可指向，另有 root disabled .3 分支 |
| 进度 `[25621,26068)` | 上 20／下 50 px，进度轨高 8 px、深绿底与绿色填充、.3 s transition；零值时整块 disabled |
| Switch／结果 `[26791,29171)` | 卡片 640×120、左右 padding 20／上下 0，第二张上距 20；序号 20 px 圆形、偏移 -10／-10；未升级文字橙色 `#fd8611` |
| 弹层 `[29171,30682)` | fixed 全屏遮罩 `#0009`、z-index 1000；modal top 306、橙边；wait 440×74，其他宽 480／最小高 162、padding 22／34；动作上距 24、gap 16，Cancel 130×28、Continue 172×28 |

原件里的 unitless box-shadow、`background:(red,.3)` 等字面声明不擅自修成另一套视觉；实际浏览器是否接受、继承或忽略仍需明确验证。Switch 块中的七个资源 URL 保留在完整原块；当前获取仍有 63 个未决引用，JS／CSS source map 不可用，选中组件与样式闭合不能替代所有媒体和共享组件依赖获取。

纯／模拟验证须逐项覆盖 skip 不写仍入成功列表、计时进度、target→current 本地赋值、状态特定 Close／SKip、恢复 false／reject、重试不重启、索引重置但旧结果保留、切换恰好一台匹配、断连和迟到图片。UMA 还须覆盖硬编码身份、两个文件只存在一个、两级返回、最终版本半区非零与目标版本不一致、progress 抑制、缺事件无 timeout、callback／队列／主句柄释放。页面视觉、键盘／焦点、GPUI 接线和实际 DLL／设备结果分别验收；本轮仅静态证据，未执行这些行为测试或窗口验收。

### 共享工具栏、弹层与键盘／焦点

[共享交互专项](../../../.analysis/results/frontend/synapse-update-fw-current/page-interactions.json)连接固件入口实际挂载的共享后代、JSX 参数及完整 CSS 平衡块。固件错误层 `zo` 直接渲染 overlay／content 两个普通 div，没有使用共享通用 Modal，也没有背景点击关闭、dialog／aria-modal、Esc、自动聚焦、焦点圈定或恢复逻辑。`Zo` 的 Cancel／Continue 与 `io` 页脚使用原生 button.disabled；警告按钮各自禁用自身后同步调用父回调，不等待原生恢复。清除错误通常卸载该实例；若同一实例只更换错误类型，源码没有重置两项禁用状态。原生按钮的浏览器键盘行为与实际窗口焦点仍须独立验收。

工具栏刷新和前后导航是 div role=button，没有自己的 tabIndex／键盘 handler；`.toolbar .arrow.disabled` 以 opacity .3、pointer-events:none 阻止鼠标点击，刷新回调仍存在且不检查 isRefreshDisable。固件 App 固定禁用刷新，前后回调仅返回空函数；设置、固件提示和配置迁移动作没有额外固件状态门禁。实际 Redux 树只有 ui／language／settings／installation／common／device 六项 reducer，没有 connection、profile、linkedGames、fw 或 oledLoading reducer，不能将通用工具栏的全部可选能力当成本页已启用；OLED 分支默认不挂载。

工具栏设置动作按精确客户端名激活 settings-synapse，或以 policy=3／shouldFocus=1 打开 `/synapse/settings/`，不发送固件 Close，也不确认设备恢复。实际 `pe` 来自 webpack `7226.A`，经 `callElectronAction → preload.doElectronAction → main` 保留 subTab／willShow；只转发 windowName 的同名 preload 便捷 API 不属于这条路径。main 执行窗口 focus、页签激活及可选 focusSubTab；`rt` 不返回或等待激活结果，不能由请求发出推定目标页或设备已经就绪。

新页的 policy=3 沿 `Tab.createTab → common.createNewTab → TabUI → doTabAction/tabEvent → TabManager → Tab.init` 找到真实创建者。已有页激活通过 TabUI／TabManager 发 activeTabChanged；关闭先经过允许关闭／force-focus 条件，选中 Tab 的销毁按可用路径等待 beforeunload，再发送带实际 frameName 的 tabClosed。固件 `rn` 消费该事件，清除迁移指示与偏好；窗口隐藏／休眠／关闭属于另一策略。页签通知不确认档案迁移、固件恢复或原生资源释放。

迁移组件 `rn` 被以空 props 挂载，没有获得父工具栏的默认 currentApp，因此当前入口构造 `/profile-migration/?app=undefined`。目标 URL 参数是非空字符串 `"undefined"`，不会被 `||"synapse"` 改写；仅 BodyWrapper 的窗口名映射未命中后回到 synapse，迁移主体仍将该字符串传给 toolbar，init／scan 不按它选择分支。设置与迁移 BodyWrapper 都查询宿主可见性／焦点并消费 focus／changeActiveTab／activeTabChanged，卸载调用对应 disposer；异步焦点查询没有取消或代次保护。两处 accessibility MutationObserver 未保留 disconnect，迁移主体启动 effect 未返回原生工作取消／释放链；语言及设备监听有配对移除，不能代替迁移工作释放。共享未保存档案菜单先从本地列表移除条目，再广播 Save All／Discard All；菜单本身没有设备写回或外部保存完成确认。

App Explorer 的实际 iframe 子页注册 message consumer、发 iframe-ready，接 popUpData 后发布 currentApp／modules／devices，卸载移除该 listener；固件父组件回应这些 props 并处理 close-popup／add-wifi-device，却不消费子页另发的 update-height。三个实际导航 callback 都同步且不返回值，丢弃 module `91._P` 的 Promise；`_P` 等待 windowName 精确匹配，查询／JSON／find 出错也转为 null，因而同样进入新开分支。已有页分支不等待 activation 结果；SDK activation 本身也不返回或等待其通用 IPC Promise。`W` 调用 callback 后随即启动独立 `J`，只有 `J` 自己的安装产品查询决定其 finally／close-popup，关闭可能早于导航完成；该查询不结算时也不能声称弹层必然关闭。`W` 没有 try/finally，同步 callback 抛错则不会启动 `J`。

模块行明确声明 policy=3；设备行只声明 shouldFocus／tab_visible，未声明 policy=3；其他应用使用自己的 openParam。新开 URL 原样传给 window.open，仅在 features 包含 `%` 时执行 encodeURI。Discover 外链和 url=null 的 add-wifi 动作绕过上述导航；设备候选也受应用、ready、错误及电源条件过滤。选中双方接收器未核 event.origin／source，父方发送指定当前 origin、子方发送 wildcard；弹层关闭和跨页通知都不能证明目标加载、迁移或设备操作成功。

共享下拉框在窗口失焦或外部点击时收起。`Me` 捕获阶段 click 监听的移除参数不匹配，卸载未清 timeoutOnShow；通用 Modal `lt` 未提供卸载时计时器和 modal-open 清理，portal 所有者只移除自身元素。用户菜单 `Dt` 的匿名 message listener、迁移 `rn` 的 tab-message 注册没有在选中 owner 中配对释放；图片预加载只设卸载标志，没有终止 Image，Promise settle 后仍先写 imgs 状态。这些属于静态生命周期事实，没有实际执行泄漏或窗口恢复验收。

工具栏默认挂载独立 F1 离线提示：监听 keyup 的键码 112，不 preventDefault；网络状态恰为 0 且两图预加载成功后显示 body portal，仅内容区域的 click handler 关闭它。遮罩和关闭图标没有各自 handler，也没有 Esc／焦点逻辑；它不触发固件 Cancel。两张动态 SVG 已沿保留的当前官方共享 catalog 索引边、HTTP 200 收据和实际字节闭合，共 10,355 B；这是 2026-10-15 的稳定观测重核，本轮没有重新观察在线部署，也没有完成整个资源库获取或像素验收。

宿主另有 F1 keyDown 路径，受 options／noBrowserInputHandler 及 `!global.releaseBuild` 条件约束，运行时 releaseBuild 值未推定。该路径查询 SysUtils 网络状态后打开官方支持页，或发送 keyPress=f1 的 handleBeforeInput；保留的固件完整 main 没有这个消息的消费者。它与上述 DOM keyup 不是同一处理器，也未建立本地帮助文件／页面动作。账户、余额、评分反馈、在线更新及联网离线提示按本地优先范围排除，保留挂载证据以核对原版。设置控件、迁移原生数据分支、其他 iframe 和全部跨页产品分支仍须各自核对；实际键盘／焦点、GPUI 接线和 DLL／设备结果保持独立验证。

共享交互专项保留 241 个新增完整 AST 所有者、复用主报告 10 个所有者，另保留 205 个完整 CSS 平衡块并复用原 36 块；117 个 JSX 交互节点、3 组目标页面来源、9 份宿主 ASAR 源码与两张动态 SVG 均已核原字节。生成后回读与独立复核通过，但这些是选定证据范围和边界的验证，不是相同数量的功能完成，也不是行为模拟、实际 DLL 对照或窗口验收。

## 固件失败、恢复与退出

失败处理 `[1877290,1877861)` 先调用 getVersionInfo 读取传入对象的既有版本字段、丢弃返回，再允许窗口／宿主退出并请求重新打开对应设备 MW；这里没有实际版本刷新。重连检查由 Sr 按 productId 和 deviceContainerId 拼出精确 MW windowName，查询本机窗口服务客户，循环间隔从 3000 ms 降到最小 500 ms，另有 10000 ms 边界；存在设备页不等于设备或设置已读回。`_r` 返回 false 仍可在非异常 then 分支被记作 cleanup success。计时器和异步回调须由真实生命周期所有者承接，不能把日志中的 cleanup completed 当作设备恢复证明。

前端 destroy 会清任务 map、timers 和 onstorage；没有展示对所有 Electron response 注册的同身份注销。原版 JS rzFW 包装器的 terminate 和配置退出动作均只调用 uninitSDK、清 isInit；完整当前 bundle 未见实际调用 remove／unregister／endUpdate。标准／UMA 事件 promise 没有 timeout，清 map 不等于取消原生任务。

标准原生 wrapper 的 `runFWUpdater 0x33c30` 只在提交时核导出指针，将输入复制到 shared-owned string，经 `0x31bd0` 包装 void packaged_task 入 FIFO 并唤醒工作线程。worker `0x369b0 → 0x31570` 在锁内移出首项、解锁后执行；真实 lambda 表 `0x4b6b8+0x10 → 0x37280` 此时重新读取全局 run 指针直接 call，没有再次检空、连接代次或模块 lease。`0x354c0` 将异常存入 future state，所读提交链没有 future.get 反馈；队列接受不能证明 SDK 已执行或事件已结算。

原生 `Terminate 0x32540 → 0x30850` 先 FreeLibrary，再清模块、8 个导出和初始化标志，没有调用 SDK uninit、停止／唤醒／排空 worker、join 或注销 callback。`DeviceTerminate 0x325d0` 仅日志并返回 true，`0x32cf0` 才单独转发 uninitSDK。队列析构 `0x45a70 → 0x314c0` 遇非零 thread ID 调用 std::terminate；完整捕获中的已界定引用未证明另一条 reset／stop／join 路径，不能报告安全卸载或退出。

`SetNodeFFIEvent 0x328a0` 保存 Node 指针并经 `0x33640` 注册 `0x32ee0`，忽略注册返回；`setCallBackId 0x32e70` 仅日志 true，并不更新保存 ID。`0x327e0` 只在 event 名相等时清字符串，不清 Node、不删 SDK callback，对 null 参数也无 guard。callback 仅检查 Node 非空和 event 长度大于零，将 arg 保留为原 payload string、同步投递后销毁临时 JSON；没有 isInit、连接、generation 或取消门禁，晚到结果仍可沿当前 event 名投递。

SDK `addSdkIotCallBack 0x1d7e0` 忽略 ID 并追加非空 callback，不去重；`removeSdkIotCallBack 0x1d830` 只移除空项，不按 ID 注销。选中的实际进度／事件发布者调用首槽，不广播全部列表。`uninitSDK 0x1d770` 仅释放 logger，`endFWUpdater 0x1da80` 为空，均不能证明 worker 或 callback 已排空。后续测试须分别核任务接受、实际执行、future 错误、最终事件、迟到结果和全部释放，不能声称可安全中止刷写。

### 宿主 FFI 加载重试与释放

本节说明原版链，不能作为本项目接入官方业务 DLL 的实现方案。当前 preload 显式 `supportElectron20:true`；update-fw 的 `F=(n(2069),n(8935))`、`F.yL` 经 module8935 的 h 调 doDLLMainAction，实际路由是 `ffiPreload → Ge.callDLL`。m／oE 才进入 doDLLMainActionAsync；不能拿旧 ffiMain 或子进程分支解释这次失败。13 份宿主 JS 已按当前官方包、ASAR 容器切片和实际原件核对，完整所有者和节点绑定保留在专项证据。

活动所有者是 [FFIPreloadMain.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/ffi/FFIPreloadMain.js) 的 ConfigureFFI `[2273,3226)`、callDLL `[8564,11243)` 与 callDLLAsync `[12005,13913)`；[Library](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/node_modules/ffi-napi-rz/lib/library.js) `[744,2177)` 先 new DynamicLibrary，实际 LoadLibraryW，再逐项 GetProcAddress。缺 prepareUpdate 后抛异常，没有 catch-close；成功完成所有导出之前不设置 lib.dllObj，也不插入 ffiLibMap。宿主 catch 返回 result=false 与具体 reason，首次失败发生在 callback 注册前。

同路径、同 channel 的第二次 ConfigureFFI 会再次执行 Library／LoadLibraryW；失败链没有自动 FreeLibrary。在没有外部卸载的条件下，Windows 增加已有模块引用，不再发送 DLL_PROCESS_ATTACH；不能将前端重试说成重新初始化了下层 SDK。配置成功后缓存只按 channel 建立，后续同 channel 直接返回 dll already loaded，不重新核传入路径和声明表，换路径也不意味着切换了模块。

当前 ffi_bindings.node 的 `0xb010/0xb160/0xb1b0` 分别落到 LoadLibraryW／FreeLibrary／GetProcAddress。ref-napi 的 PointerBuffer 构造 `0x29a0` 注册 finalize `0xa7b0`，实际链经空槽 `0x1150`、析构 `0x2ce0`，仅清 N-API 引用并 free 包装对象；两张真实 vtable 与完整函数均已对 PE 核字节。它没有 FreeLibrary，GC 不能补上缺导出后的卸载。

callDLL／callDLLAsync 的 FreeFFI 分支直接 return true，未调用空的 `_handleAction_FreeFFI [7719,7757)`；完整 FFIPreloadMain 没有删除库／callback map 或 close。DLLRegistry 只存 channel／path 元数据。函数代理保留 funcPtr、CIF 与类型，没有独立模块 lease；FreeMalloc 只释放返回字符串。SetNodeFFIEvent `[6351,7719)` 按 channel 保留 callback，重复注册直接 true，保存 map 也不以原生注册结果为真为条件。

后续纯／模拟测试与显式 DLL 对照须区分首次缺导出、再次加载引用、PROCESS_ATTACH 次数、同 channel 换路径、callback false／重复／迟到与 FreeFFI 空操作，分别核真实恢复、模块卸载和在途任务释放。这里是原件静态链和系统加载语义，未加载 DLL 或实测引用计数，不能宣称运行恢复或安全释放通过。

## 本机安装文件与执行结果

安装入口要求 type、id 和 location，申请 `installation_<type>_<id>` 的本机锁。锁通过 `navigator.locks.request(...,{ifAvailable:true})` 获得，保存 resolver 维持租约，统一结束路径释放。拿到锁只证明本轮安装协调权，不证明任何文件或驱动已安装。

`Rr` 从 `simpleGetUserApps` 的真实文件清单同时匹配 filename 和 sha256。`Xa` 要求 action 具有 saveToDisk 和实际 uint8ArrayData，按 app/path/filePath/fileName 发布本地文件；现有同 hash 文件可以跳过重复写入，否则调用 `simpleAddUserAppFile` 并读取真实 bool/result/reason。纯存储清单项、资源名和目标版本都不能代替实际文件存在与内容核对。

`Ka` 的本地执行行为保留以下分支：

| 条件 | 原版行为与结果边界 |
| --- | --- |
| 缺 action 或 saveToDisk | 本次动作失败 |
| 文件后缀为 .dll | 不作为可执行安装器运行；文件替换/写回另由本地文件发布和清理链管理 |
| manifest 设置 NO_INSTALL | 跳过执行；原件把跳过记作成功，不能据此认定驱动/服务安装完成 |
| Windows 且有 stop-service 动作 | 先执行已声明的服务停止，再进入安装动作 |
| 非 ASCII 路径且满足驱动/文件/服务版本分支 | 通过实际 ASCII 复制路径启动；不能泛化为所有平台都需要提权 |
| manifest 明确要求 elevated | 使用对应提升入口；其他分支使用普通本机进程入口 |
| 实际进程返回 | 保留 result、exitCode、reason 并广播 `install-fileExitData`；异常属于失败 |

安装资源按 priority 分桶，桶之间顺序执行，桶内 Promise.all 并行。`yl` 先写 INSTALLING，再检查空列表，仅在进入整个安装循环之前检查一次 isCancelling；桶内和后续桶没有再次检查。文件发布完成和 runOnInstall 的可执行安装动作是两种阶段。`$l` 要求实际调用结果不是 false/installerError 后才维护安装资源记录；manifest 要求重启或 exitCode=3010 时设置 restartRequired。安装后按 keep 标志决定是否删除安装文件，删除使用实际路径和 hash。

最终发布产品版本前检查 installErrors；源码的 ignoreErrors 分支可以在仍有错误时完成原版流程，这个状态不能在本项目中被解释为所有本地能力成功。completedResources、expectedResources、upgradedResources、真实进程结果、资源记录与产品记录分别维护。

### InterHaptics 安装与卸载的旧进程退出链

当前资源清单绑定的 `1.0.0.49` [安装包与完整提取收据](../../../.analysis/results/native-packages/33954697ea3c99d8a7e1ffd99255834c29a01e4d0712b9e8fbc83f5db346d83f/inspection.json)、[安装解释器捕获](../../../.analysis/results/native/33954697ea3c99d8a7e1ffd99255834c29a01e4d0712b9e8fbc83f5db346d83f/capture.json)及[同包卸载解释器捕获](../../../.analysis/results/native/2889e68bb939c46eb568ed1ac386f435b1672af6ecbf04c0e212a451c23824ec/capture.json)共同约束这条本地生命周期。安装包为 2,157,704 B，提取的 `Uninstall.exe` 为 366,544 B，均为 x86、Image Base `0x400000`。两套捕获各 105 个函数、8,320 条指令、25,589 B raw chunks，逐函数文件摘要、chunk 摘要及全部指令字节已与各自 PE 核对；这只是静态证据核验，不是执行安装、卸载或 helper 的收据。

两份文件的 NSIS firstheader 均位于文件 `0x9200`；`+20` 为解压长度，`+28` 的高位表示压缩、低 31 位为含 5 B 属性的压缩块长度。安装包的长度分别为 `44,222/0x127e`，卸载包为 `32,100/0xe0c`；文件 `0x9220` 的属性均为 `5d 00 00 80 00`，据此以 LZMA1、`lc=3/lp=0/pb=2/dict=0x800000` 静态解压后，长度精确匹配且解码器到达流末尾。下文以解压 header 为 `h`、entry 使用从 0 开始的索引：安装 entries 为 `h+0x11dc` 的 1,008 条 28 B 记录，strings 从 `h+0x801c` 开始；卸载对应 `h+0x9c4` 的 751 条记录与 `h+0x5be8`。每条记录按 7 个 little-endian i32 读取，首项为 opcode、后六项为操作数。`0x30a9` 将 header 的块相对偏移转换为基址指针，`0x1389` 以 `28*index` 分发到 `0x1434`；非零的正跳转目标先减 1，再作为下一 entry 索引，零表示顺序继续，不能把原始目标直接当从 0 开始的索引。

字符串由 `0x65b9` 按 UTF-16 code unit 展开：`3` 加后继编码表示 NSIS 变量，变量号为后继两个字节各取低 7 位后组合；`2` 表示 shell／注册表目录，`1` 为语言表引用，`4` 转义下一个 code unit。安装 string `1502` 位于 `h+0x8bd8`，完整内容为变量 21（`$INSTDIR`）加 `\Synesthesia\SynesthesiaStop.exe`；string `1537` 为变量 15（`$R5`）。卸载对应 string `984/1019`，路径位于 `h+0x6398`。目录前缀不能从可打印字符串片段中丢弃。安装 entry `501` 先设置 `$INSTDIR=$PROGRAMFILES\Interhaptics\`，entry `506/518` 将 `Synesthesia.exe` 放入进程查询参数；helper 路径不是本包中待解压的文件声明。

| 分支与 entry | 实际指令、结果消费者及后续路径 |
| --- | --- |
| 安装 section `499..982`，`523..525` | `nsProcess::_FindProcess` 查询 `Synesthesia.exe`，结果 Pop 到变量 10（`$R0`），只将数值 `0` 当作找到；非零跳到 entry `582`，直接进入后续服务检查。此前没有产品版本比较将这条进程处理限定为某个旧版本。 |
| 安装 `528..539` | `528=(25,15,1502,0,0,0,0)` 设置 `$R5`；`529` 分别记录文件存在／缺失。`535=(12,1537,0,539,0,0,0)` 的缺失目标 `539` 实际落到 entry `538`，只跳过 calling 日志，仍尝试 `538=(41,1537,-1,1,0,0,0)`。该指令等待 helper 退出，随后 `539` Sleep `300` ms。不能把文件缺失解释为已经跳过实际启动。 |
| 安装 `544..581` | 再次 `_FindProcess`；仍返回 `0` 时调用 `555` 的 `_KillProcess`，Sleep `500` ms 后在 `561` 再查。`563` 仍为 `0` 时，`568` 跳到 entry `964` 的重启安装；非零则沿 `570/576` 的日志路径进入 entry `587` 的 `Check_HapticService_Installed`。`_KillProcess` 的直接返回值未在此用作成功条件，实际分支读取后一次 `_FindProcess`。 |
| 安装 `964..977` | 创建 `$PROGRAMFILES\Razer\Temp`、复制当前安装包，将 HKLM `Software\Microsoft\Windows\CurrentVersion\RunOnce\RazerAppEngine-InterHaptics-installer` 写为带引号的安装包路径加 `/S /runOnce`，`976` 设置进程退出码 `3010`。这是延后安装登记，没有执行系统重启的指令，也没有证明该登记／复制在真实系统上成功。 |
| 卸载 `.onInit` `337..444`，`388..403` | 独立的卸载 header 在初始化中查询 `Synesthesia.exe`；找到后设置 `$R5`。`399=(12,1019,0,403,0,0,0)` 的缺失目标同样实际落到 `402=(41,1019,-1,1,0,0,0)`，仍尝试 ExecWait；`403` 的等待间隔为 `500` ms。此链属于卸载初始化，并非安装 MainSection 的共享 entry。 |
| 卸载 `410..444` | `_FindProcess → 必要时 419 _KillProcess → 420 Sleep 500 ms → 425 _FindProcess`；`427` 检测仍找到时仅记录失败，随后继续结束初始化，`441` 还会 Sleep `300` ms。这里没有安装 entry `568` 的立即转重启分支，不能将安装控制流套用到卸载。 |
| 卸载 section `445..731`，`657..676` | 后续另行执行旧 Synesthesia 目录删除：`660` 清除错误，`661=(23,2771,3,0,0,0,0)` 对 `$PROGRAMFILES\InterHaptics\Synesthesia` 做递归 RMDir；`662` 检查该操作的错误。失败计数达到 `80` 转 entry `712`，未达到时等待 `500` ms 再试。该目录删除发生在 helper 调用之后，不能说 helper 自身承担删除或迁移。 |
| 卸载 `712..728` | 失败路径登记 `RazerAppEngine-InterHaptics-uninstaller` RunOnce，命令为带引号的 `$PROGRAMFILES\InterHaptics\Uninstall.exe` 加 `/S /runOnce`，设置重试结果；最终依该结果设置退出码 `3010` 及 reboot-required 标志。正常路径清理相应注册表与目录。登记或退出码都不等于真实删除完成。 |

`0x1434` 的 opcode `0x29` 在 `0x1fd0` 展开命令后，经 `0x1fe1 → 0x5b5f` 调用 `CreateProcessW`：`lpApplicationName=NULL`、直接使用展开的命令字符串、创建标志 `0x4000000`，没有由这条指令补入引号。`0x5b5f` 成功后先关闭线程 handle，再返回进程 handle；`0x1ff6 → 0x6a1b` 以 `WaitForSingleObject(handle,100)` 循环等待，超时期间处理消息，没有总等待期限或已核实的取消分支。离开超时循环后调用 `GetExitCodeProcess`，其 bool 未参与校验；因此静态代码中的返回值读取不能冒充真实退出成功。这里操作数为 `exit-variable=-1/wait=1`，启动失败或 helper 返回非零只增加 NSIS 错误计数（`0x2c59`），随后继续执行后续 entry；进程 handle 在 `0x2017` 关闭。helper 的退出结果与后来对 Synesthesia 的进程观察是两个结果，不代表 HapticService 或设备停止。

同包 `nsProcess.dll` 已按实际字节与[完整捕获](../../../.analysis/results/native/30c6c3dd3cc7fcea6e6081ce821adc7b2888542dae30bf00e881c0a105eb4d11/capture.json)核对：x86、Image Base `0x10000000`、11 个函数、425 条指令、1,280 B。`_FindProcess` 导出 `0x1377` 通过 `0x10d0` 查询进程名；找到返回 `0`，未找到为 `603`，OS／加载／查询失败另有 `604..608` 等结果。上述脚本把所有非零结果走到同一个“未运行”分支，因此这些分支不能提升成已确认进程不存在。`_KillProcess` 导出 `0x13b8` 使用 `0x103f` 的 `OpenProcess/TerminateProcess` 路径，关闭成功打开的进程 handle；本脚本未调用带窗口关闭等待的 `_CloseProcess`。NT 查询缓冲在正常枚举完成后 LocalFree，模块查询 handle 与 Toolhelp snapshot 按各自路径释放；输入／输出 NSIS 栈节点由 `0x143a/0x1489` 消费和产生，opcode `0x1f` Pop 实际 GlobalFree 节点。不能只据导出名字推定返回 ABI 或强制停止成功。

HapticService 的停止等待与 helper 是独立阶段。安装 entry `688` 调用 `SimpleSC::StopService` 后等 `1,000` ms，重新取得服务状态；仅状态 `3/4` 进入 entry `757..783` 的进程轮询。该循环每次仍找到 `HapticService.exe` 才等 `1,000` ms 并增加计数，达到 `50` 退出；entry `788/790` 再次查询／判断，仍找到时拒绝继续移除服务并转重启安装，非零查询结果才继续 `RemoveService`。卸载另有 entry `519` 的 StopService、`523` 的 `1,000` ms 延迟、`524..550` 的 50 次轮询，却没有安装的最后一轮 `_FindProcess` 屏障；它继续查询服务状态并在 `612` 请求 RemoveService，后续由目录操作决定是否登记重启卸载。这里的 `SimpleSC` 调用名、参数、返回栈及脚本消费者均为静态事实，完整 SCM 内部路径及实际服务结果仍需独立核对，不能由日志补成成功。

解释器 `0x1434` 的 opcode `0x2c` 在调用前核实模块和实际导出地址，并传递窗口、字符串容量 `1024`、变量表、栈地址及额外参数表；并非通过猜测的普通 C getter 签名调用。`0x68d9` 的文件存在检查会 FindClose；普通 Synesthesia 查询／终止的调用在没有保持加载或注册回调条件时 FreeLibrary。HapticService 轮询的 nsProcess 调用带保持加载操作数，不能称每轮都卸载。进程退出前 `0x3557 → 0x3b74 → 0x3bb9` 处理输入 handle、已登记插件回调／模块与临时插件目录，`0x5c88` 处理目录枚举、删除及失败后的延后删除；这不是删除 `$R5` 指向的 helper。尚未取得 `SynesthesiaStop.exe` 的当前官方文件，也未取得 `Interhaptics.HidProvider.dll`，本包完整提取的 11 个文件没有它们。因此这里只确认安装／卸载调用方对 helper 的执行意图和结果消费，helper 内部的通信、退出、等待及释放仍未确认。

后续测试须独立覆盖进程查询 `0/603/查询错误`、helper 缺失／创建失败／非零退出／等待、两条不同的延迟、终止后仍存活、50 次服务等待、80 次目录重试以及 RunOnce／`3010` 消费；独立 Rust 实现与当前官方 DLL 的实际执行结果仍须按全功能对照规则留收据。当前仅完成上述静态核验，没有执行 installer、uninstaller、helper 或厂商脚本；这些程序的运行验收、实际服务变化和设备停止全部保持未验证。

## 安装原生队列、退出码与服务状态

当前主机 `main.js` 的实际 Windows 发布路径选择宿主版本目录中的无版本 `simple_service.dll`；调试 CommonDLL 分支另行选择本机副本。这里只分析该选择规则，不将厂商路径迁入产品。用于下列主链核对的是[当前主机原件](../../../.analysis/inputs/installed-current/files/app-engine/CommonDLL/simple_service.dll)及其[静态捕获](../../../.analysis/results/native/f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b/capture.json)，Image Base 为 `0x180000000`；另保留版本化原件的独立身份，不能因名字相近便当作同一个二进制。

导出对参数非空和线程有效性的核对先于真实工作。AddUserAppFile 复制输入字节至拥有自身生命周期的容器；普通、提升和 ASCII 启动复制 app、filePath、params，并把完成回调排入队列。实际虚调用已经按构造函数和原始 PE 核对：vtable RVA `0x1f2040` 的 `+0x48/+0x68/+0x70/+0x78` 分别落到 `0x2d10/0x3720/0x3a00/0x3ce0`。未发现可从本次 installer cancel 广播传入的取消参数、任务 ID 或进程句柄。

| 动作 | 导出 → 主队列 → Apps 队列 → 实际工作 |
| --- | --- |
| 发布文件 | `0x10660 → 0x2d10/0x2ee0 → 0x221de/0x223a0 → 0x274f4` |
| 普通进程 | `0x12280 → 0x3720/0x3920 → 0x23052/0x23250 → 0x1d5e8/0x284d0` |
| 提升进程 | `0x13140 → 0x3a00/0x3c00 → 0x236cc/0x238d0 → 0x1d74a/0x29570` |
| ASCII 路径进程 | `0x14000 → 0x3ce0/0x3ec0 → 0x233a6/0x23580 → 0x1d8ac/0x2a040` |

主队列缺失返回 `Invalid thread`；Apps 线程或实例缺失分别有独立失败，进程回调可带 exitCode=-1。Apps 实际工作先确认目录、文件及文件校验，再进入本机启动或 elevation COM 分支。普通进程的等待标志为真，ASCII 路径另包含是否已为 ASCII、复制目录及完成后清理的分支；提升入口进入 COM，不能据这个名字假定每次都由当前进程直接 CreateProcess。下节记录当前客户端及独立保留服务副本的接口、启动、事件与释放；服务副本的最新部署身份和真实运行结果分别保持未验证。

直接进程路径的 `0x79da0` 实际以 `WaitForMultipleObjects` 同时等待进程句柄和 AppsService 的停止 event，无固定超时；只有进程对象先满足时调用 GetExitCodeProcess。停止 event 先满足返回 1，等待／读取失败返回 2。调用方在处理这些返回之前已经排队 bool=true 和当前 exitCode，因此 true 不保证取得了真实退出码；初始 exitCode=-1 也不能自动视为安装器失败码或成功码。必须区分启动、进程实际退出、等待被释放和安装器业务结果。

Electron 的[Windows 适配器](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/modules/simple_service/win/index.js)对普通／提升启动保留 `void(bool,string,int)` 和 `{result,reason,exitCode}`，但 ASCII 启动只声明 `void(bool,string)` 并返回 `{result,reason}`。原生 ASCII 完成包装器 `0x13de0` 实际调用三参数回调，第三项为 exitCode；这条桥接丢失退出码，`$l` 因而不能从该结果识别 3010，除非 manifest 另声明 restartRequired。这里是静态已证实的结果字段丢失，未据此猜测或执行 ABI。适配器用 cbMap 保留回调，收到回调才删除，所读方法没有超时和取消。

### 提升 COM 的客户端、类型库与事件释放

[客户端与安装提权证据](../../../.analysis/results/host-current/installation-elevation-client.json)保留 16 个完整 AST 所有者、99 个客户端原生函数、11 个客户端 PE 数据区域、14 个客户端虚表槽位及逐项字节身份。客户端 DLL 已核对当前官方 `4.0.827` 宿主包；服务端则是另一条来源：[本机保留的 razer_elevation_service.exe](../../../.analysis/inputs/installed-current/files/elevation-service/razer_elevation_service.exe) 由[独立获取收据](../../../.analysis/inputs/installed-current/service-acquisition.json)记录版本 `1.1.0.8`、x64、注册的 `LocalServer32` 来源及签名观察。该 EXE 尚未与最新官方 `4.0.827` 部署物逐字节核对，当前性及部署依赖闭包保持未验证。其 `REGISTRY`、`TYPELIB` 及下述选中代码分别核对了实际 PE 字节；服务副本补充保留 75 个完整 `.pdata` 范围、10 个显式 leaf／thunk 区域、7,409 条指令、10 个数据区域及 4 个真实业务槽位。该报告的服务端切片使用 LLVM 静态反汇编；另已完成同一原件的[完整 IDA／Hex-Rays 捕获](../../../.analysis/results/native/fd7dc9d5de93cfb027eda972967f7b0e6871eeb4f6e1cd337dfd6d186572176c/capture.json)，4,182 个函数文件及 9 份输出的实际摘要均已核对。5 个函数的伪代码失败保留，完整汇编和 raw chunks 另核原 PE；捕获完成不代表原报告的所选语义已扩展为整个服务，也不证明最新官方服务身份或实际对照完成。

客户端 `0x2dc9a/0x2e152/0x2e89a/0x2ed1c` 使用 `CoCreateInstance`、`CLSCTX_LOCAL_SERVER=4`。PE 内实际 CLSID 为 `0edeaf3c-d36e-4e7e-9467-900b977dc4ff`，IID 为 `bfe24d59-6568-4179-8ae5-d9d53869a3e3`；二者与保留服务端副本的类型库及注册收据一致。该类型库 `RzUtilityLib` 的 LIBID 为 `f24a9e29-fc07-4277-9bbe-8315a7735bf0`，其 `IElevator` 方法为：

| 虚表字节偏移／DISPID | 方法 | 类型库参数与客户端使用 |
| --- | --- | --- |
| `+0x38`／1 | `LaunchProcess` | 输入 `BSTR filePath`、`BSTR params`、`UI4 cbId`；输出 `INT* exit_code`。等待版 `0x2e152` 及 ASCII 专用 `0x2ed1c` 的真实反汇编均调用此槽位 |
| `+0x40`／2 | `GetVersionInfo` | 输出 `BSTR* versionInfo`；客户端 `0x2dc9a` 读取后 `SysFreeString` |
| `+0x48`／3 | `CopyRazerFile` | 输入两个 `BSTR` 路径，输出 `BSTR* error`；保留服务副本的真实槽位 `0x3180` 直接返回 `E_NOTIMPL`，不复制、不写输出，不能从声明推定具备文件复制实现 |
| `+0x50`／4 | `LaunchProcessNoWait` | 输入 `BSTR filePath`、`BSTR params`、`UI4 cbId`；`0x2e89a` 实际调用，没有退出码输出参数 |

这些方法的结果类型为 `HRESULT`。等待版导出 `0x13140` 传入 wait=true，沿主队列和 Apps 队列到 `0x29570`，后者分配 job ID、将退出码初始化为 `-1`，再把该变量地址交给 COM。只要 COM 方法返回非负 HRESULT，外层便排队 `result=true` 及该整数；客户端没有另行确认服务端确实写了输出参数、进程已退出或安装器业务成功。不等待版 `0x138a0` 选择 `+0x50`，内部完成结果仍可带初始 `-1`，不能当成真实进程退出码。前端 `simpleLaunchUserAppElevatedJson` 还有 engine `4.0.531` 版本边界；较早宿主走普通进程入口，不能假定所有版本都调用提升接口。

连接器由 AppsService 的 I/O 线程持有；`0x280a0` 复用有效实例或经 `0x2db6c` 构造。`0x760c0` 使用 `CoInitializeEx(NULL,6)`，析构 `0x761e0` 仅在记录的 HRESULT 非负时配对 `CoUninitialize`。连接器销毁先使 weak pointer 失效；线程、COM apartment、服务对象及其事件订阅分别有所有者，不能把一个导出调用当作整个服务的租约。

每次启动方法都在栈上构造 `0x2e5e4` 的 176 字节临时 sink。`0x2e66e/0x2f326 → 0x30956/0x309a1` 读取 COM 类型信息，优先用 `IProvideClassInfo2::GetGUID(1)` 得到默认事件 IID；备用分支经 `IPersist::GetClassID`、coclass 类型信息和 implemented-interface flags=3 找默认 source。`0x30e5c` 再查询 `IConnectionPointContainer`，调用 `FindConnectionPoint`、`Advise`，将 cookie 保存在 sink `+44`。保留副本类型库中的默认 source 确为 `_IElevatorEvents`，IID `c6237515-dd5f-4acc-b6f0-f45569adf965`，与当前客户端 QueryInterface 条件一致。

该事件接口只有 DISPID=1 的 `ElevatorEvent(UI4 eventId,BSTR eventData,UI4 cbId)`；客户端的 `0x30410`、真实 `FUNC_INFO` 和 `DispCallFunc` 对应同一参数顺序。这里 `cbId` 是 job ID，BSTR 是事件文本。`0x30450` 将 eventId=0／1／2 分别交给连接器 `+8/+16/+24`，实际落到 `0x2f3d0/0x2f710/0x2f9f0`，再排队 AppsService 的错误／已启动／已退出事件。未知 eventId 不触发这些事件。退出事件将文本转为整数；`0x79870` 即使解析失败也写出解析器的当前／截断／饱和值，其 bool 被调用方忽略，不能把该消费者描述成严格有效整数校验。

AppsService 随后经 `0x1cc60/0x1ce80/0x1d090` 通知观察者；当前主机虚表的实际目标 `0x9650/0x99d0/0x9cf0` 仅在注册标记及外部 callback 均有效时序列化 `jobID` 和对应字段，再发布原生事件 4／5／6。JS 将其映射为 `onsimplelaunchprocesserror`、`onsimpleprocesslaunched`、`onsimpleprocessexited`，通过 `appsserviceevent` 广播给有效 webContents。这条状态事件链与单次启动的 `{result,reason,exitCode}` 完成回调是不同通道。

COM 方法返回时，无论同步等待还是 NoWait，客户端都调用 `0x2e804`：若 cookie 有效，经 `0x30fcc` 尝试 `Unadvise`，再清 cookie、释放保留的接口和 BSTR。该析构不汇总 Unadvise 的 HRESULT，也没有显式等待任意在途的外部回调；sink 的 AddRef／Release 是恒定计数返回，并未将栈对象转成独立生命周期。**NoWait 的临时订阅并不保留到未来的真实进程退出**；只能记录调用期间实际到达的事件，不能由方法提交成功推断之后仍能收到退出通知。

ASCII 专用 `0x2ed1c` 使用同一等待接口，并让 sink 保存实际选择的 ASCII／复制路径；eventId=0 或 2 时尝试删除该路径，eventId=1 不删除，删除结果未参与成功判断。原有直接启动回退及其清理是另外的分支。当前客户端的等待 COM 方法没有 stop-event 参数、取消 token 或本机超时；`0x79da0` 的直接进程／停止 event 等待机制不能套到 COM RPC 上。设置 AppsService 的停止 event、停止线程或关闭窗口，都没有证明服务端等待被打断、外部进程被终止或其副作用已恢复。

### 保留服务副本的实际启动、事件与停止

以下结论仅适用于上述 `1.1.0.8` 保留副本。构造函数 `0x7ace` 将 `IElevator` 放在对象 `+40`，安装真实虚表 `0x143af0`；业务槽位 `+56/+64/+72/+80` 分别为 `0x6490/0x7510/0x3180/0x7320`。`GetVersionInfo 0x7510` 分配 BSTR 文本 `1.1.0.8` 并交给调用方，分配失败进入已有异常路径；`CopyRazerFile` 的未实现返回与类型库声明必须分别记录。

`LaunchProcess 0x6490` 和 `LaunchProcessNoWait 0x7320` 先做路径／stat 核对；非零结果统一发布 eventId=0 的 `File not found` 并返回 `E_FAIL`，不能把该文本当成所有失败均已准确分类。之后才做签名允许检查，失败发布检查器的错误文本并返回 `E_ACCESSDENIED`，两者均不会启动进程。

签名入口 `0x15464` 按扩展名选择 `.ico` 与普通 PE 两条链，允许名字表的两个字符串为 `Razer USA Ltd.`、`THX LTD.`。普通路径 `0x130cd → 0x12bf8` 使用 `WinVerifyTrust`、通用 Authenticode action、无 UI、VERIFY 状态和 provider flags `0x1000`，取得证书链、复制编码证书后检查 subject commonName；有 provider 数据时先 CLOSE 再比较，provider 为空的错误分支没有这次显式 CLOSE。它遍历返回链中的证书，不能描述成仅比较叶签名证书。`.ico` 路径另检查六字节头、条目数、签名数据、重建内容与签名，缺签名、读取失败、重建失败、名字不允许和签名无效有不同结果。该副本自己的 CN／算法／路径已另对原字节核实，见下节信任专项；它没有因此获得当前官方部署身份。

签名通过后，`0x61e0` 选择枚举到的第一个 `WTSActive` 会话并释放枚举内存。返回 session ID=0 与“没有活动会话／枚举失败”在调用方都会跳过 WTS 启动，这个零值分支不能省略。

| 启动路径 | 实际参数、状态与失败行为 |
| --- | --- |
| `0x6a40` WTS | 查询用户 token，建立继承环境，获取 linked token 并复制为 primary token；desktop 为 `winsta0\default`，当前目录传文件父目录，`CreateProcessAsUserW` 的 applicationName 为空、flags 为 `0x420`。命令行直接从 filePath 复制，只有参数 BSTR 长度至少 2 才拼接空格和参数，单字符参数被省略；任一准备／创建失败返回 `E_FAIL` 后继续 fallback |
| `0x6f8a` fallback | 为 filePath 加双引号并拼非空参数。先调用 `0x41800 → SetCurrentDirectoryW` 改服务进程的当前目录，忽略结果，所读 owner 没有恢复旧目录。`LaunchOptions +0x21=true` 先经 `0x3f2e0 → 0x3efe0` 调 `ShellExecuteExW`，verb 为 `runas`、保留进程句柄；失败后再清该 flag 走普通 `CreateProcessW`。两条分支均未传独立的非空目录参数，不能把服务当前目录修改当成已确认的每任务目录 |

成功创建先发布 eventId=1。等待版 WTS 分支直接 `WaitForSingleObject(INFINITE)`，不检查 wait 返回；退出码先置 0，随后也不检查 `GetExitCodeProcess` 的 BOOL，仍发布 eventId=2、写 out 并返回 `S_OK`。fallback 的 `0x3ee70 → 0x3ee90` 本来只有 wait 返回 `WAIT_OBJECT_0` 且退出码读取成功才写 out，但 `0x6f8a` 忽略这个 helper 的 bool，也可能把初始 0 发布为退出码。因此 **`S_OK + exitCode=0` 不足以证明真实进程已退出并取得退出码**，必须连同 wait／read 原始结果比较。

NoWait 传 `wait=false`，两条服务端路径都跳过等待、退出码写入和 eventId=2，然后关闭所持进程／线程句柄；没有后台退出观察者。该副本自身不会在未来补发 NoWait 的退出事件，加上客户端方法返回时释放临时 sink，不能承诺后续退出通知。关闭句柄也没有终止进程。

事件生产者 `0x667e` 对非空 sink AddRef，按 DISPPARAMS 倒序构造 `UI4 cbId`、`BSTR eventData`、`UI4 eventId`，通过 `IDispatch::Invoke(DISPID=1, LCID=0x400, DISPATCH_METHOD)` 发送，再清 VARIANT、Release。空槽跳过；启动调用方忽略其 Invoke HRESULT，所以 `S_OK` 不证明消费者收到了 started／exited／error。

订阅 `Advise 0x8200` 验证指针、查询 source IID、对 sink QueryInterface，保存取得的接口至首个空槽，将一基槽号交作 cookie；接口不匹配和容量分配失败分别返回对应连接错误，失败时释放取得的接口。`Unadvise 0x83a0` 对有效非空槽先清空再 Release，空槽／无效 cookie 返回 `CONNECT_E_NOCONNECTION`；cookie 可随空槽复用，只在当前订阅生命周期内有意义。对象零引用经 `0x7cc0 → 0x8040 → 0x8dbe` 释放剩余 sink 和向量。这些选中方法没有显式在途回调屏障或向量锁，不能虚构等待全部外部 callback 后才释放的保证。

服务注册与启动也有不同路径：`0x17b2` 依据经过类型与终止 NUL 校验的 AppID `LocalService` 值，选择 SCM dispatch 或本地 COM server；`0x53ea` 的创建参数是 own-process、demand-start、RPCSS 依赖、带引号的模块路径、空 account/password，并配对关闭 SCM／service 句柄。命令解析保留 RegServer／UnregServer／per-user／Service 分支；这些只是静态参数证据，没有执行注册或安装。

`ServiceMain 0x5a90` 建立 COM 后关闭本地 idle-monitor flag；`0x5d32` 在 service 模式初始化 COM security、注册 `CLSCTX_LOCAL_SERVER=4`／`REGCLS=5` 并 resume class objects，临时 class factory 引用由 `0x60a2` 释放。控制处理器 `0x5cc2` 收到 STOP 时设 `STOP_PENDING` 并给服务线程投递 `WM_QUIT`，没有子进程句柄、取消参数或 `TerminateProcess`。消息循环退出后 `0x5f38 → 0x6048` 撤销 class cookies；第一次非 `S_OK` 撤销结果会使循环停止，不能记成无条件全部注销成功。本地 server 的 idle event／monitor 另有独立关闭链。SCM stop、消息退出、RPC 返回、子进程退出和恢复原系统状态必须分别验收。

服务停止由 installer `Ce.stopService` → 主进程 `StopService` → [serviceFunction.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/serviceFunction.js) → simpleLaunchRazerApp → 当前宿主目录的 [RzPowerTool.exe](../../../.analysis/inputs/installed-current/files/app-engine/CommonDLL/RzPowerTool.exe) 完成。文件缺失返回 undefined。helper 的[静态捕获](../../../.analysis/results/native/5d04be921ae8f9f2338fc4b69afbfdb5ac3435dd1d95511bc0c9f82c3f305c72/capture.json)使用 Image Base `0x140000000`；`WinMain 0x36430` 对 `--stop-service` 返回停止 bool 本身作为退出码，即成功为 1、失败为 0，不能套用常规退出码 0 成功规则。前端确实用 `!!exitCode` 转换，但 `Ka` 等待后丢弃这个 bool，仍继续执行安装器。

PowerTool 的 `0x356e0` 只接受固定服务名；`0x36360` 打开 SCM，`0x36290` 打开服务并先停止活动依赖，随后 `0x35eb0` 查询／请求停止并核对 SERVICE_STOPPED。已有 stopped 返回 true；pending 等待按 waitHint 取 1–10 秒间隔，停止请求后的等待直接采用 waitHint，30 秒边界只在等待后检查，不是严格的单次调用时限。递归依赖停止没有逐项合并 bool，父函数也不检查依赖停止结果；部分成功日志先于／独立于真实返回，故日志不能代替状态核对。正常失败返回关闭服务／SCM 句柄，依赖枚举缓冲区也释放；这不恢复停止前的服务状态。

完整 `Ka`、`$l`、取消回调、正常结束及异常关闭消费者均没有对应 `StartService` 或初始服务状态保存，因此不能声称安装失败／取消会恢复被停止的服务及其依赖。产品的可恢复写入对照仍必须先记录真实原值并恢复，当前静态链没有为执行官方安装器提供授权。

### 当前官方升级工具的不同实现

同一[证据报告](../../../.analysis/results/host-current/installation-elevation-client.json)另保留两套 PowerTool 的 21 个完整 IDA 函数及逐指令／chunk 字节校验。当前宿主包选中的无版本 helper 与后台资源清单声明的 `1.0.18.0` helper 是不同输入，升级动作不能按同名工具合并。

当前宿主 `WinMain 0x36430` 的 `--upgrade-elevation-service <filename>` 进入 `0x36dd0`，但忽略其结果，正常继续返回初始退出码 0。升级 owner 在固定 elevation-service 目录组合源文件与 `razer_elevation_service.exe` 目标；源存在时请求停服务，却不检查 stop bool，再进入最多 5 次、失败 false 之间间隔 500 ms 的复制。复制错误异常与 false 重试是不同分支，不能一概声称异常也会重试。无论是否复制 EXE，随后都会调用 `0x37570` 检查客户端 DLL；EXE 复制成功才删源文件，复制成功或目标不存在时再尝试删除升级计划任务。

`0x37570` 读取并校验实际服务与当前 `simple_service.dll` 的四段文件版本，读取失败或当前 DLL 不存在便停止；编译数据和构造函数 `0x1010/0x1040` 证明以下选择：

| 服务版本 | 客户端版本条件与文件来源 |
| --- | --- |
| 恰好 `1.1.0.7` | 必须精确为 `1.2.2.9`；来源 `simple_service_v1.2.2.9.dll`。降级前先将 outgoing DLL 备份为 `simple_service_new.dll` |
| 其他版本 | 四段版本至少为 `1.2.2.10` 时不动；否则从 `simple_service_new.dll` 恢复。复制成功才删除已消费 backup |

这些文件位于 helper 自己的目录。DLL 复制重试仍失败后，原件会枚举 `RazerAppEngine.exe`、打开并终止匹配进程、逐个等待至多 5 秒并关闭句柄，随后再复制；没有完整原始进程／服务状态恢复。任务删除 `0x36b00` 使用固定系统 schtasks 路径，等 30 秒，超时请求终止这个任务删除进程，其他 wait 结果仍尝试读退出码且忽略 BOOL；结果只写日志，不能汇总成可靠升级成功。

版本化 `1.0.18.0` 的 `WinMain 0x36200 → 0x368d0` 则直接做存在检查、复制、删源、system 任务删除；该 owner 没有停服务、复制重试、上述客户端版本选择、备份／恢复或杀宿主进程分支，正常返回 1 也被 WinMain 丢弃。两套 helper 均未执行；外层退出码、日志和文件版本分支都不能证明实际安装、替换或恢复成功，更不能由此证明本机服务副本已匹配当前官方部署。

### 文件准入、PE／ICO 签名与发布清理

[安装信任专项](../../../.analysis/results/host-current/installation-trust.json)复用并重核当前 simple_service 的 218 个完整 IDA 函数、两套 helper 的 4 个完整函数，另对保留服务的 49 个完整 .pdata 范围、4 个 leaf 和全部选中数据区核实际字节。当前客户端与 helper 绑定已有官方获取收据，本次没有重新在线确认发布；保留服务没有当前部署绑定或 IDA／Hex-Rays 完整捕获。其 LLVM 范围与当前客户端完整函数分别保留，不能混称同一来源或能力。

当前客户端普通 PE 的路径／句柄入口为 `0x4991d/0x4997a → 0x49448`。WinVerifyTrust 使用 action GUID `00aac56b-cd44-11d0-8cc2-00c04fc295ee`、UIChoice=2、RevocationChecks=0、ProvFlags=`0x1000`，要求返回 0 且 csSigners 非零。它依 signer／simple-chain／element 顺序复制证书，关闭 provider state 后再解析；允许表 `0x24b090` 顺序为 `Razer USA Ltd.`、`THX LTD.`，任一已解析链证书的 subject CN 精确命中即可，不限叶证书。provider 为空的错误分支没有进入后续显式 CLOSE。

CN 已追至 `0xd5950 → 0xd5340 → 0xd5590 → 0x1252d0/0x125f00`：TBSCertificate subject 位于解析对象 +72，证书对象 +8 是 subject principal，OID `55 04 03` 为 commonName；`55 04 0a` 才是组织名。保留第一个非空 CN，按相同长度和精确字节比较，不忽略大小写、不匹配子串、不改用 issuer 或指纹。UTF8／Printable／Teletex／IA5／Universal／BMP 六种字符串转换及失败分别保留；解析有效期字段本身不证明已经比较当前时间。

ICO 独立进入 `0x4adc3 → 0x4ae43`：读取 PNG iTXt，重建去掉选中 chunk 的 ICO 并核内容摘要，再核 SignedData 的内容和 signedAttrs.messageDigest。选第一个允许 CN 的嵌入证书，用其 SPKI 验签；signedAttrs 首 tag 改为 `0x31`。当前调用方只选择 RSA PKCS#1 v1.5＋SHA-1、RSA PKCS#1 v1.5＋SHA-256、ECDSA＋SHA-256。实际 EVP method+`0x38` 已解析到 RSA `0x127790` 与 EC `0x1516f0`，并追到填充／DigestInfo 精确比较与 ECDSA 的 DER、范围和公钥方程；库中 RSA-PSS 等其他能力不能据此算入该分支。此链没有 WinVerifyTrust、受信根构建、当前时间有效期比较或逐证书 issuer 签名验证，不能套用普通 PE 的 Windows 根信任结论。

Apps 根通过 PathService key112／`0xe5870` 取得 LocalAppData，再 append 各目录；`0x80820` 只是清空 FilePath。UTF-8 转换失败替换为 U+FFFD，但上层丢弃失败 bool；FilePath 截断嵌入 NUL 并处理尾分隔符，append 不证明真实解析后的目录包含关系。open／move 另有 ReferencesParent 的词法组件检查，没有已证的 reparse-point 根范围保证。ASCII 复制走 ProgramData／Razer／Synapse／Temp／BaseName 和 CopyFileW，复制后核属性并清只读位；复制前验签不等于复制后的字节确认。

| AddUserAppFile 分支 | 验证、发布与清理边界 |
| --- | --- |
| `.zip` | 直接写入，未进入上述 PE／ICO 签名 owner |
| 普通文件 | 以 flags=`0xb868` 建 delete-on-close 临时文件，按句柄验签，再另开目标重写 |
| `.msi/.msix` | flags=`0x9868` 无 delete-on-close；关闭初始文件、附扩展名重命名、按路径验签，再 MoveFileExW(flags=3)；重命名／最终 move 失败有删除尝试，信任或建目录失败的选中路径没有显式删除已改名候选 |

这些写入只检查返回值不等于 -1，没有等长确认或最终文件读回 hash。当前 PowerTool 的 `0x36dd0/0x37430/0x37570`、版本化 helper 的 `0x368d0` 则是直接复制／文件版本分支，选中 owner 没有沿用上述签名或摘要检查。保留服务的 `0x120fc8 → 0x121320` stat 入口也没有 Apps 根限制。各结论限于已核调用链；底层密码算法全部 CPU 变体、其他平台／产品分支、真实文件效果及恢复仍未闭合。本次未执行任何 DLL、helper、安装器或设备，也未运行这些路径的纯／模拟行为测试，实际对照保持未验证。

## 共享依赖、卸载和取消

共享资源有 usedBy 所有者。更新或卸载时仍被其他产品引用的文件只移除当前 owner；所选 installer 的共享判断按 resourceName 查找，记录修改按 pathName 查找，不能擅自归一两个条件。不再共享的文件移除使用本轮 simpleGetUserApps 取得的目录和精确 SHA 观察，不是每次删除前重新读磁盘。失败加入 pending-remove 的调用没有 await。

当前 background 经 `jA` 打开 installer 时不传 version，installer 缺省版本为空，先按 latest 选择 manifest；`Bl` 再要求 resourceFetchedVersion 与 installedVersion 精确相等，不等即报 UNINSTALL_PRODUCT_NOT_EXIST。本地独立实现须保留版本判定和真实本机结果；在线 manifest 获取属于排除的编排，不进入离线产品流程。原版入口在此之前仍等待存储／服务注册尝试、查询安装记录和用户目录，并验证 manifests；服务重试耗尽回调只构造错误而没有调用 handleError，不能将该处描述为已阻止后续卸载。

`Bl` 将具有 runOnInstall 字段的 saveToDisk 资源分到驱动组，普通文件串行移除，驱动用 Promise.all。驱动分支先查询 getDriverInfo 的 isInstalled 与版本，再调用 simpleLaunchRazerApp。宿主返回整个 `{result,reason,exitCode}`，installer wrapper 原样传递，`kl` 既不检查 result／exitCode，又在调用后删除资源记录；非空的失败对象仍为 truthy。普通资源返回 false／undefined 时也可能只计数和警告，随后删除当前 installedModules／installedDevices 记录，再执行 pending 清理，最后才发 COMPLETED。实际删除目标来自 storage handler 的 installId，removeInstalledProduct 的实参未被使用。记录变化、警告后继续与 COMPLETED 都不能作为文件、驱动或进程成功证据。

pending 清理还有独立的结果类型差异：simpleRemoveUserAppFile 在 host 将 bool／string 回调包装为对象，installer wrapper 却只返回 result 布尔值。`Dr` 按 bool 使用，`En` 对 truthy 值执行 `"result" in t`，因此在当前 Electron 链的 true 返回上存在静态 TypeError 路径；false 留下 pending。removePendingRemove 的过滤条件由 app、name、sha256 三项不等用 AND 连接，任一项相等即可去除条目，不能描述为只清完全匹配项。该结论来自完整源码和返回形状核对，未执行厂商 JS。

取消频道名为 `installation_<type>_<id>`。`install-cancel` 回调立即写 CANCELED、设置 isCancelling，再调用 `$r`；后者只收集当前 We 中的 fetch AbortController，等待 abort signal 回调计数达到控制器数后删除注册项。这个 Promise 没有等待响应流消费、文件写入、资源记录、进程退出或服务恢复。已经 abort 的控制器计入完成，没有活动控制器时直接 resolve；日志中的 All downloaders cancelled 只对应此信号阶段。

`Xl` 下载完成后检查取消；进入 `yl` 后直到全部 priority 处理结束，`Xl` 才再检查。取消在安装期间到达时，原件可能继续启动后续桶、写 installedResources／completedResources、更新单项记录和删除安装文件，最后抛 USER_CANCELLED；最终产品版本发布可以因此没有执行，但之前的副作用没有回滚。不能描述成“取消会阻止所有尚未开始的安装动作”。`yl` 在入口检查 flag 之前先写 INSTALLING，也会出现 CANCELED 被后续状态覆盖的时间窗口。

正常结束 `gn` 依次清 INSTALL_RESOURCES／瞬态状态、清该 manifest 的浏览器缓存、释放锁、等待 service worker offlineReady，最后延时关闭。缓存的 forEach 调用 cache.delete 没有 await 每个删除，锁释放前也没有加入外部进程确认；offlineReady 等待最多 60 轮各 1 秒，未读取取消标记。正常流程先执行 En 的 pending-remove 和 Tl 的未使用记录清理；En 的对象成功／文件不存在分支受上述 bool／对象差异限制，不能当作实际清理保证。Tl catch 返回错误对象却没有调用 handleError，调用方也未检查该返回值。卸载取消同样先发 CANCELED，但 Bl／Dr／kl 不读取取消标记，未见驱动进程终止或已删记录／文件恢复；UNINSTALL_PROGRESS 也不在 gn 清理的进度名单中。

`unhandledrejection` 的生产关闭分支只在延时后移除五类进度／错误状态、释放锁并关窗，没有调用完整 `gn`、En 或 Tl，未清 INSTALL_RESOURCES，也没有启动服务、恢复文件或结束外部进程。`beforeunload` 仅发起 `$r().then(...)`；既没有阻止卸载，也没有证据证明宿主等待其异步完成。`navigator.locks` 的辅助函数保留 resolver 维持租约，但成功回调调用工作函数时不 await／返回它；请求与实际工作异常不构成同一个 Promise 完成链。

卸载后续的完整 AST 和宿主绑定记录在[自启／卸载收据](../../../.analysis/results/frontend/synapse-settings-current/local-startup.json)的 `application_uninstall`。后台只根据 installedModules storage 事件和闭包当前目标启动子模块清理，不等待 En／COMPLETED，也不核对实际文件；启动时核对入口没有设置该闭包目标，不能合并两条路径。旧 SDK 的 uninstallAppItem 是独立的排队、依赖文件、apps 删除和事件链，尚未证明由后台 installer handoff 调用。宿主保存的 appLaunchedList.log 不是 Run 重建，WssAction.stop 返回 true 也早于 websocket close 回调。

同一收据的 `native_uninstall_producers` 另将当前保留 Synapse／Chroma 4.0.827 包内 Lua 与真实卸载事件连接：342 个完整词法函数、94 个记录、20 个生命周期挂载和 44 条调用边，经独立双层解包及 CRC／原字节复核。单应用 producer 构造 `--uninstall-app`，整引擎停止组合 `--exit-application`；`OnPostUninstall` 包含从 HKCU Run 两份参数名单移除当前应用并请求写回的脚本路径，空 apps 则请求删值。写入原参为尚未闭合解析的 `REG_SW`，无写后读回；唯一卸载 relaunch 调用处于长注释内。完整分支和日志结果边界见[原生卸载脚本](settings.md#原生卸载脚本的-clirun-与退出)。宿主最终退出消费者 Mn 的完整 JS 所有者、3 秒超时、Je/Qe/Ze 分派及尚未核全的原生完成边界见[宿主设置文档](settings.md#宿主最终退出清理消费者-mn)。已生成的卸载器、服务端／OS 真实效果、取消恢复和原值恢复继续保持未验证，不能把当前包的脚本证据扩展成整个安装生命周期已通过。

原生 SimpleService shutdown 与此窗口取消也是两条链。`0xf090` 在停队列之前清全局初始化标记；完成包装器 `0x12060/0x12f20/0x13de0` 只有标记仍真才调用外部回调，因此停机中的迟到结果可能不再回到 JS。shutdown `0x19c0 → 0x1cf0 → 0xb270 → 0x218ac/0x21a40` 释放 AppsService；析构 `0x26a9c` 设置上述停止 event、清 COM 引用、停止 I/O 线程并关闭 event。该释放 event 可使等待退出码提前结束，但所读析构没有 TerminateProcess。不能把线程释放／cbMap 删除或窗口已关当作外部安装器已经退出，更不能由此推断服务状态恢复。

本项目的目录库、缓存、安装文件名和清理边界统一来自 [installation.json](../../../crates/razer-resource-db/installation.json)。普通用户必须能写入安装缓存；卸载只清本程序的安装/缓存目录并保留用户配置语义。原版更广范围的历史文件扫描、驱动/产品目录清理不能直接照搬为本项目删除权限。独立 SQLite 目录库随安装发布为单独文件，必要核心图标离线可用；缺失目录库且无有效缓存时应显式失败。

## 验证边界

安装扩展重新校验 installer 原始脚本实际 bytes／SHA-256 和 tree-sitter 完整解析；主机 main、serviceFunction、SimpleService 适配器对照 ASAR 提取收据核对字节。上述无版本 DLL、版本化 DLL 和 PowerTool 的实际原件分别核对各自捕获身份；所读函数 JSON 按函数清单核对 hash，虚表从原始 PE 读取。无版本与版本化 SimpleService 的 selected owners 虽多数相同，提升调用的函数边界定位仍不同，本文主链 RVA 以当前宿主选中的无版本捕获为准。

| 安装前端所有者 | UTF-8 半开字节区间 |
| --- | --- |
| fetch abort 与取消消费者 | `$r [92243,92609)`；频道初始化／onmessage `[109376,110022)`／`[109645,109906)` |
| 服务调用与进程桥接 | stopService `[65417,65711)`；普通 JSON `[97868,98282)`；提升 JSON `[107581,108346)`；`Ka [137252,139690)` |
| 锁、执行与单项提交 | `wn [142461,143032)`；`yl [181450,182795)`；`$l [183691,184260)` |
| 清理与结束 | `Tl [187465,188075)`；`En [190014,190510)`；`gn [191612,191912)`；`Fl/bl/Ul [191913,192573)` |
| 最终消费者与异常 | `Xl [200311,202973)`；beforeunload `[203065,203199)`；unhandledrejection `[203204,203785)` |

全功能测试要求见[验证规则](../../../.agents/rules/verification.md)。下列为独立 Rust 实现需要覆盖的可测试条件，尚未执行或据其生成通过记录：

| 条件 | 必须比较的输出与副作用 |
| --- | --- |
| 下载前、两桶之间、进程等待中取消 | flag／状态次序、fetch abort、实际启动动作序列、已写资源项、最终产品记录和清理完成分别比较 |
| 文件发布成功后进程失败／记录失败 | 文件实际字节、真实 result/reason/exitCode、记录部分提交、keep 分支及 pending-remove；不以最终窗口状态代替 |
| 标准固件预检与 3003 | 引导复位为写入，不能放进只读入口；分别比较原 CommandName 的忽略、FirmwareUpdate 消费、SN 截断／空值与实际清理，不能仅比错误码 |
| V25 type 0/2 写后读回与失败释放 | `0x7f20` 的启动／重连、`0x8300` 的逐块读回和 byte mismatch=3、helper=3/5、reset 丢弃、失败句柄与 Block 释放分别比较；不能把返回 2 当作设备持久化或句柄清理 |
| DFU 事件解析与等待 | 缺 payload／错误 code／falsy 进度的错误结算，与 truthy 非数值／越界进度继续等待、JSON 解析异常只日志分别核对； incoming SN 为空与非空、数值 0 与 100 单独覆盖 |
| 共享工具栏与跨窗口动作 | CSS 禁用与原生 disabled、SDK 保参、policy=3 实际 Tab 创建、focus／tabClosed 事件、iframe 消费、异步迟到与卸载分别比较；窗口消息不能代替保存、恢复或设备确认 |
| App Explorer 导航和关闭顺序 | 三个同步 callback 丢弃 Promise、查询错误转 null、独立 J 的 finally、查询未结算与同步抛错分别覆盖；模块／设备／其他应用的实际 policy 及 features 编码条件分别核对 |
| Ry 短头／尾字节／文件变化 | h=0…32 的实际发送范围、loader gap／截尾、u32 回绕、XOR 未观测尾、EOF／错误和长度快照分别核对；静态推演不作为实际应答样本 |
| 固件 ZIP SHA-256 生命周期 | binary 原始字节与 ZIP entry／解压内容的区别、open／read／EOF 失败、空串 expected、64 B 分块与 bit length、32 B digest 的大写 hex、`prepareUpdate 0x1c8d0` 的精确长度／`memcmp` 比较、上下文析构分别比较；不能把文件 hash 通过当作设备认证 |
| 691 键盘及接收器 | 三份原生业务 DLL 各自 ABI／0或2成功、RY版本掩码及回调宽度、MCU写入与真实第二遍读回、null结果、重连／版本／恢复各自确认 |
| 691 HEX 转换与文件失败 | 05 的 callback／end 顺序、含冒号整行长度 1…9、空输入／坏前缀／EOF 后数据、signed32 地址及正负 gap、奇数 nibble、输出打开／写入失败与部分文件保留分别比较 |
| 691 可选 WinUSB | normal=false 的调用者 HANDLE、selector=0、OUT81／87 的本地数据、初始化／传输失败、首字节突变后的重发、非 FF ACK／五次耗尽返回及全部释放分别比较；不执行固件刷写作为测试 |
| 691 Nordic 文本与 CRC | 零行／空行／末尾 LF、92 B 补零 CRC、74 B 与长行截断／越界边界、进度分母、最终 CRC ACK、流错误与退出应答忽略分别核对；静态算术结果不冒充实际 DLL／设备结果 |
| 691 文件流与异常 | 转换器无本地 catch、getline catch-all／badbit 与重抛、15 槽锁与 codecvt、输入／全局 locale／输出顺序、flush 失败仍 fclose、文本模式转换和裸缓冲释放分别核对；实际依赖未绑定前，不用候选 CRT 默认值断言本包的实际结果 |
| 691 其余 MCU 导出及 48 B | 03／5a／a5 前缀、最多三次 SET／二十次 GET、第三次 true 错 ACK 与 false 的差别、command09 无 GET、MAC 失败保旧输出、flag 低 8 位／32768 跳读／尾块与零分母分别核对；driver 的 ShellExecute 结果不能当安装完成 |
| 691 RY 资源与报文 | 固定位置版本／校验、global 与 local encryption 差异、query 高位标记与 boot 掩码、37 扇区与 header／body XOR、三种设备代码分支、31h／32h／33h、进度 100 但最终 ACK 失败、CString 与 FILE／HANDLE 清理分别核对 |
| 691 独立窗口与语言 | 四窗导航／重建、Guide TopMost 与各页 TabStop、MyButton 自有 Enabled、current 两种消费者、文本与条形 clamp 差异、null reset 进入成功、固定目标成功页、九种语言 neutral 回退、更新中关闭／监听句柄释放分别核对 |
| PE／ICO 文件准入 | subject CN 的字段／大小写／链证书顺序、ICO 内容与 signedAttrs 摘要、三种签名组合和各自根信任边界分别核对，不把双方拒绝当成功能一致 |
| ZIP／普通文件／MSI／MSIX 发布 | 短写、验签失败、重命名／move／建目录失败、候选遗留、delete-on-close、最终实际内容和完整恢复分别比较 |
| 普通／提升／ASCII 返回 0、3010、其他值或无回调 | 原生返回、桥接字段丢失、restartRequired、超时／取消及 callback 生命周期；3010 的缺失字段保持未观测 |
| 服务不存在、拒绝停止、依赖失败、pending 超时 | helper 实际退出码 0／1、真实服务状态、Ka 是否继续、原值与依赖是否恢复；日志不作为独立期望 |
| 关窗与 shutdown 竞争 | 原生初始化标记、迟到回调、等待 event、句柄关闭和仍存活外部进程各自记录，不把静默回调当成功 |
| 等待／NoWait 的 COM 边界 | 实际槽位、BSTR 所有权、job ID、HRESULT 与 out 退出码分别核对；没有写输出、仍为 `-1`、0、3010 及业务失败分别覆盖 |
| 事件订阅及注册失败 | 类型信息／source IID／FindConnectionPoint／Advise 失败时不得继续启动；cookie、接口释放、Unadvise 失败、晚到回调和 NoWait 提前释放分别比较 |
| COM 事件与 ASCII 清理 | DISPID、eventId、cbId、错误及退出码文本、未知／非法／溢出输入、weak pointer 失效及 eventId=0／2 的复制文件删除副作用分别核对 |
| 保留服务副本的方法结果 | CopyRazerFile 的 `E_NOTIMPL` 与未改 out sentinel、版本 BSTR 所有权、路径核对／签名拒绝与未启动副作用分别验证；当前官方服务尚未绑定时不提升为当前官方实测 |
| 服务的 WTS／fallback／等待 | session 0／非零、token／环境失败、含空格路径、参数长度 0／1／2、服务当前目录修改与未恢复、runas 失败后的普通启动、wait／exit-code 失败仍发 0 各自比较 |
| 服务订阅与停止 | cookie 复用、QI／分配／Invoke／Unadvise 失败、空槽、析构 Release、STOP 期间阻塞 RPC 和继续存活的子进程分别记录；NoWait 的未来退出事件应保持未产生 |
| 两套升级工具 | 版本 1.1.0.7／其他、DLL 精确／最低版本、缺源／缺目标、stop false、复制 false／异常／五次边界、备份／删除、杀进程和任务删除分别用纯／模拟接口验证，不执行 helper 充作可恢复 DLL 对照 |
| 正常收尾、全局异常、cache.delete 延迟／失败 | INSTALL_RESOURCES、五类瞬态键、pending 清理、缓存、锁、service worker 等待及关窗顺序分别验证 |

无设备时可先用纯逻辑和模拟文件／传输／进程／服务接口验证上述状态机与失败注入；回放的期望须来自已核实的原件。纯逻辑、模拟验证、当前官方实际 DLL 对照与真实设备／系统验收各有独立状态，不能以两端都未连接判为一致。安装器执行、主程序运行和固件刷写不在本轮授权边界内，不能为了对照擅自执行。

已静态核对本机安装文件链、标准固件预检／调度／事件消费者、当前 179 与 691 的选中升级链、固件共享交互、当前客户端 COM／直接进程／停止链、PE／ICO 信任与发布清理，以及保留服务副本的选中启动／事件／订阅／释放代码和两套升级工具差异。服务 EXE 的最新官方部署字节与依赖闭包仍未核实；当前产品清单和宿主包 ZIP 已留目录核对，该 ZIP 没有服务 EXE，不能据此认定外层安装或另行下载依赖不存在。完整服务捕获、其余底层密码／文件算法分支、其余固件协议、其他平台及全部产品分支仍需继续。报告保留原始分析快照，并另外核对并行获取结束后的当前输入索引及全部不可变来源。完整捕获、包装器初始化和文件写入都不能证明独立 Rust 实现完成。本文没有运行应用、厂商 JavaScript、安装器、helper、DLL、服务或设备，也没有执行本节纯／模拟测试或实际 DLL 功能对照；这些验证及固件刷写、安装运行验收保持未验证。

## PID0542/PID0543 音频设备驱动安装器的原生包装链（静态捕获）

这两个文件是当前官方设备资源中的音频设备驱动安装器，而不是 Lighting/Power 协议 DLL；PID0542/PID0543 属于 Seiren V2 Pro/X 资源。版本资源分别标为 `Razer PID0542 Driver Installer` 和 `Razer PID0543 Driver Installer`，文件版本均为 `5.30.1.0`。原件收据位于 [corpus.json](../../../.analysis/results/corpus.json)；对应输入是 [PID0542Drv_v5.30.1.0.exe](../../../.analysis/inputs/official-native-current/blobs/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/PID0542Drv_v5.30.1.0.exe)（2,096,928 字节，SHA-256 `6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8`，官方资源 URL `https://app-assets.razer.com/files/synapse/devices/1346/1.0.0.0/PID0542Drv_v5.30.1.0.exe`）和 [PID0543Drv_v5.30.1.0.exe](../../../.analysis/inputs/official-native-current/blobs/f12164552213b106fc5b10745637dfe92ed83f139679b27d2ca7e2554c8f241f/PID0543Drv_v5.30.1.0.exe)（2,097,192 字节，SHA-256 `f12164552213b106fc5b10745637dfe92ed83f139679b27d2ca7e2554c8f241f`，官方资源 URL `https://app-assets.razer.com/files/synapse/devices/1347/1.0.0.0/PID0543Drv_v5.30.1.0.exe`）。两份文件的 `.text`、`.rdata` 和 `.data` 原始区 SHA-256 分别为 `b7bd0795251a2cdc950dc4cc1e0789678829a2531e5f5c449cd1adee82f02fe0`、`e57e06f0a1440e2bc802fdb52f8f75ca76a1ac6925d92603ffaac204917446e9`、`eefc550e3dcbf08f3a45d7bf56cd1f2d97a3c85a8f219c8ce9a398f0c78b2dfe`；差异落在资源和附加数据，不能据此把两个产品的资源收据合并为一个输入。

两份输入分别使用 IDA Pro `9.1`、Hex-Rays `9.1.0.250226`、Python `3.11.9`、Image Base `0x400000` 做独立 `capture-native`。每份都完成 `105/105` 函数、`105` 份伪代码、`0` 个失败、`160` 个导入、`6,515` 个字符串、`5` 个段；[PID0542 捕获收据](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/capture.json) 和 [PID0543 捕获收据](../../../.analysis/results/native/f12164552213b106fc5b10745637dfe92ed83f139679b27d2ca7e2554c8f241f/capture.json) 均报告 `capture_complete=true`、`functions_complete=true`、`all_pseudocode_captured=true`。两份捕获的 `all-functions.asm` SHA-256 都是 `a4407a63493163d6a125a529ad592e6a825b1836d7815b9431ca3465c190f8f9`，`all-functions.c` 都是 `354666918ae24547cf314426e003426fe20948f542bb396b71726153bf60b661`；这只说明包装层代码相同，不代表附加的设备载荷相同。

静态调用链如下（RVA 以 `0x400000` 为基址）：

1. 入口 [start `0x3312`](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/3312.json) 设置 `SetErrorMode(0x8001)`，解析命令行 `/S`，依次尝试 `%TEMP%`、Windows `Temp` 和低权限临时目录并创建目录。临时目录无法创建时不会假定设备可用，而是转入错误／退出路径。
2. 自校验和载荷提取由 [0x2EA1](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/2ea1.json) 完成：以 `CreateFileA` 只读打开自身，按 512 字节（已有载荷时为 `0x8000`）读取附加区域，校验包含 `0xDEADBEEF` 等标记的头和长度，使用 [CRC32 `0x6581`](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/6581.json) 核对块，再由 [0x30D8](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/30d8.json) 解压到 `GlobalAlloc`。头、长度、CRC 或解压长度不符时返回 `Installer integrity check has failed`；进度窗口由 [DialogFunc `0x2DBA`](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/2dba.json) 更新 `verifying installer: %d%%`。
3. NSIS 指令分发在 [0x1434](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/1434.json)；其中包含文件打开/读写/逐行读取、`ShellExecuteExA`、子进程执行、动态 `LoadLibraryExA`/`GetProcAddress`、窗口和注册表操作。注册表包装函数 [0x5F47](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/5f47.json)、[0x5F75](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/5f75.json)、[0x5FA8](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/5fa8.json) 分别执行打开、创建和查询；这些是安装器通用动作，不能解释为键盘灯光寄存器或 HID 报文。
4. 子进程包装 [0x5760](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/5760.json) 调用 `CreateProcessA`，创建标志为 `0x04000000`（`CREATE_NO_WINDOW`），关闭线程句柄并返回进程句柄；[0x653F](../../../.analysis/results/native/6c747ca0871f35dcd6092860890d94387b18b9a2aab0c4efca559222c20462f8/functions/653f.json) 每 100 ms 等待，直到 `WaitForSingleObject` 不再返回 `258`，随后读取 `GetExitCodeProcess`。该循环没有取消令牌、截止时间或退出码重试语义，调用方仅在返回后关闭进程句柄。
5. 收尾在 `start` 的 `sub_4037FA`、`OleUninitialize` 和 `ExitProcess` 路径；完整性／临时文件／子进程失败会显示 `NSIS Error` 或 `Error launching installer`。`0x82B0` 的 `SETUPAPI` 只是字符串，捕获的导入表没有 `SetupAPI.dll`、`DeviceIoControl`、HID 或设备专用 DLL 导入，因此本次捕获不能把包装器当作已分析的设备通信实现。

这条证据把“页面触发驱动安装”与“设备灯光/省电协议”分开：当前页面源码及 Rust worker 仍须通过实际设备适配器完成读取、写入、ACK/读回、取消和释放，安装器只证明本地资源可被自校验、解包并启动子进程。内嵌载荷在捕获中是压缩数据，未从其中得到 `.sys`/`.inf` 或 HID 命令的可核对函数；因此驱动服务安装、设备枚举、灯光帧、Power 读回、失败恢复和释放仍标记为 **Unverified**。本批没有运行安装器、主程序、官方 DLL、服务或真实设备，也没有改变生产 Rust 路径；官方 DLL 与 Rust 的运行结果对照必须在后续获准的只读/可恢复测试中单独记录。
## PID0521/PID0524 音频设备安装器的文件、注册表与取消分支（静态捕获）

为覆盖另一组音频设备驱动资源，又并行捕获了 [PID0521Drv_v0.1.2.0.exe](../../../.analysis/inputs/official-native-current/blobs/05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec/PID0521Drv_v0.1.2.0.exe)（2,778,392 字节，SHA-256 `05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec`，资源 URL `https://app-assets.razer.com/files/synapse/devices/1313/1.0.1.0/PID0521Drv_v0.1.2.0.exe`）和 [PID0524Drv_v0.1.2.0.exe](../../../.analysis/inputs/official-native-current/blobs/8e1e75faadcd7916cc26e7b5deca915f4e9dad1f31e2b1fe5723fe4290d15633/PID0524Drv_v0.1.2.0.exe)（2,778,392 字节，SHA-256 `8e1e75faadcd7916cc26e7b5deca915f4e9dad1f31e2b1fe5723fe4290d15633`，资源 URL `https://app-assets.razer.com/files/synapse/devices/3867/1.0.0.3/PID0524Drv_v0.1.2.0.exe`）。两者使用相同的 `0.1.2.0` NSIS 包装层：`.text`/`.rdata`/`.data` SHA-256 分别为 `23a323d8fc04afc0c453270dcec39c0d705892fe39c33e2bbd81e0b7d3b65b8d`、`c9e3fd2da79cde7a47d4932f469001f7e5f67eacf69ed520523eb2dcd01ab735`、`b5ed404759b38b231232a0fea8e8d06071dbc3ece12c69518f0f2e4cc908bf35`；资源区因产品版本信息不同而不同，压缩附加数据也分别保留。

[PID0521 捕获收据](../../../.analysis/results/native/05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec/capture.json) 与 [PID0524 捕获收据](../../../.analysis/results/native/8e1e75faadcd7916cc26e7b5deca915f4e9dad1f31e2b1fe5723fe4290d15633/capture.json) 均由 IDA Pro `9.1`/Hex-Rays `9.1.0.250226`/Python `3.11.9` 完成 `110/110` 函数和伪代码、`0` 失败、`163` 导入、`7,129` 字符串。两份 `all-functions.asm` 的 SHA-256 都是 `0ba7e4f76fcf822d36f67b1527c6999eabd0e599644145e1cf5740828b1fa4f1`，`all-functions.c` 都是 `f0d3e07ca11e2949921bc91587698da2226791577c171acf2aef495d991b40cd`。

与前一组包装器相同的解包入口现在是 [start `0x37AD`](../../../.analysis/results/native/05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec/functions/37ad.json) 和 [自校验/解包 `0x333C`](../../../.analysis/results/native/05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec/functions/333c.json)。本组的 NSIS 分发 [0x1434](../../../.analysis/results/native/05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec/functions/1434.json) 额外保留了下列可核对语义：

- 文件写入分支调用 `WriteFile` 封装；创建或写入失败后进入实际的 Retry/Abort/Cancel 分支，字符串分别为 `File: error, user retry`、`File: error, user abort` 和 `File: error, user cancel`。Retry 回到写入循环，Abort 返回 `0x7FFFFFFF`，Cancel 增加错误计数并返回，不伪造完成。
- Delete/RMDir 分支调用 [递归清理 `0x5DFC`](../../../.analysis/results/native/05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec/functions/5dfc.json)：遍历目录后逐项 `DeleteFileA`，再 `RemoveDirectoryA`；失败、重启后删除和非法输入分别写入对应日志，不能当作已清理。安装日志路径由 [0x3D76](../../../.analysis/results/native/05b076d2b6e184ec0d9ae3c05ce5c489444374d742e8411c6b9a6f63c5aab0ec/functions/3d76.json) 组合为临时目录下的 `install.log`。
- `ExecShell` 分支调用 `ShellExecuteExA`，成功或失败都写入日志；请求等待时使用返回的进程句柄并在等待后关闭。`Exec` 分支调用 `CreateProcessA`，按脚本参数决定是否等待并读取退出码；创建失败设置错误标志，退出句柄始终走关闭路径。
- 注册表分支包含 `RegCreateKeyExA`、`RegSetValueExA`、`RegQueryValueExA`、删除和值枚举；DLL 注册分支通过 `LoadLibraryExA`/`GetProcAddress` 查找入口，缺少 OLE、DLL 或入口时分别记录错误。`SETUPAPI`、`PROPSYS` 在字符串表中出现，但不是导入表中的 DLL，不能据此推断设备协议调用。

这批仍然只覆盖安装器的本地文件/注册表/子进程包装。压缩载荷中的 `.sys`/`.inf`、服务安装、音频设备 HID 命令、灯光帧、Power 读回、设备确认、取消后的恢复和句柄释放尚未从静态包装层闭合，均保持 **Unverified**。没有运行安装器、官方 DLL、主程序、服务或真实设备；后续若要对照，必须把安装器成功/失败、取消、重启删除和实际设备读回分别记录，不能把 NSIS 包装器返回值当成设备写入成功。
## RazerMCUUpdaterInterface.dll：HID/WinUSB 固件更新与 OLED 命令链（静态捕获）

原件 [RazerMCUUpdaterInterface.dll](../../../.analysis/inputs/official-native-current/extracted/840128a4f3b8d5c9d04fc60967136a635dbf7fe9f1134b2be0e8d140f348b542/RazerMCUUpdaterInterface.dll) 为 32 位 PE，42,496 字节，SHA-256 05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28，Image Base 0x10000000。IDA 捕获 [capture.json](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/capture.json) SHA-256 bc5e006873c56cab498f56fd8b88ba60520755c4ca186a3dd2e830d653864cd0，160/160 函数、0 个 Hex-Rays 失败；导出表保留 24 个入口。

### 设备枚举、句柄和控制传输

- [OpenDevice 0x1ce0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1ce0.json) 通过 HID 类 GUID 枚举接口，把 VID/PID 以四位大写十六进制匹配设备路径；以共享读写、零访问打开句柄后要求 HidP_GetCaps 成功且 FeatureReportByteLength == 91，不满足就关闭句柄继续枚举。它会释放 preparsed data、设备信息列表和临时路径缓冲。
- [OpenWinUsbBootloader 0x19d0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/19d0.json) 枚举另一 GUID，以 GENERIC_READ|GENERIC_WRITE 打开 WinUSB 句柄，初始化接口并读取 USB descriptor；descriptor 的值或路径中的 REV_ 十六进制值命中请求版本时返回句柄，否则 WinUsb_Free、关闭句柄并继续。两个枚举入口失败都返回空句柄。
- [usbControlIn 0x1770](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1770.json) 和 [usbControlOut 0x16c0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/16c0.json) 每次临时 WinUsb_Initialize，执行一次 control transfer 后 WinUsb_Free；它们只返回 WinUSB BOOL，不等于设备业务应答。
- [WinUSB_SendCommand 0x1820](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1820.json) 用 request type 0x40 发送控制包，成功后等待约 20 ms、再以固定读回包 0x8FC0/1 byte 检查首字节不是 0xFF；初始化或传输失败分别等待约 100 ms，最多重试五轮。成功条件是第二次读回控制传输成功且值不为 0xFF，不是只看第一次写入 BOOL。

### HID report、状态码和设置

- [sub_10001520 0x1520](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1520.json) 对 91 字节 Feature report 计算 XOR 校验写入第 90 字节，以 HidD_SetFeature 最多尝试三次，再 HidD_GetFeature 轮询状态；返回值区分写失败、轮询超时和 report 状态 1/2/3/4，调用者将 2 作为成功门禁。
- [GetDeviceFWVersion 0x1f40](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1f40.json) 使用命令 0x87（或 dongle 选项打开时 0x93），把返回 report 的第 9 字节起复制到调用者缓冲；[SetDeviceMode 0x1ff0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1ff0.json)、[SetOledScreenLowPowerMode 0x4210](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4210.json) 和 [SetOledBrightnessScaling 0x43a0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/43a0.json) 组装固定 report，均以 sub_10001520(...) == 2 才报告成功。
- [BackToDefault 0x2130](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/2130.json) 和 [reset_ps5 0x4a50](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/4a50.json) 是独立的固定 report 路径；[DFUExitEx 0x2080](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/2080.json) 以 WinUSB vendor transfer 退出 DFU 后等待约 100 ms。

### 固件文件、分块和释放边界

- [upgrade_firmware_process 0x3e80](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/3e80.json) 从当前进程目录固定打开 new_fw.bin；非 PS5 分支按最多 512 字节块发送 request 0x82，PS5 分支先写启动 report，再把镜像交给内部分块函数。非 PS5 完成后调用 DFUExitEx，另一分支发送结束 report；进度回调由 [upgrade_progress_percent 0x41d0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/41d0.json) 设置，结果 0 才代表内部流程完成，文件缺失、句柄无效、分块传输失败和校验不符仍返回非零（2 表示版本门禁不符）。
- 所有 WinUSB helper 在本次捕获中都显式释放 interface；枚举失败路径关闭文件句柄和设备信息列表。导出 [InstallBootLoaderDriver 0x1ca0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1ca0.json) 与 [UninstallBootLoaderDriver 0x1cc0](../../../.analysis/results/native/05c454f02c2d7c8f5cfd33519d2770fe70180005f35ed8e7ae9992b728289f28/functions/1cc0.json) 只是 ShellExecuteA 调用，不能把其返回句柄当作驱动安装成功。
- 该 DLL 的静态链没有闭合固件镜像校验、真实设备确认、取消/恢复和进度消费者；本项目 Rust 实现必须保留读回状态、失败和释放三态。没有运行 DLL、WinUSB/HID、安装动作或真实设备，以上结果仍为 Unverified。
