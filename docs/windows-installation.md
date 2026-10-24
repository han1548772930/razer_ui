# Windows 安装交付

程序以安装包交付。`packaging/windows/RazerUI.iss` 使用 Inno Setup，默认安装到当前用户的 `%LOCALAPPDATA%\Programs\Razer UI`，无需管理员权限。可以选择其他可写目录；安装路径及缓存路径不能穿过符号链接或目录联接。最低系统设为 Windows 10 1703（15063），对应已存在的 GPUI EXE 静态导入的系统 ICU C API。

安装内容包括 Windows GUI 程序、必要且显式指定的运行库、固定窗口图标及原生图标及安装凭据。目录库是 EXE 旁的独立 SQLite 文件，SQLite 引擎静态链接，无需另装数据库或 SQLite DLL；首次使用可离线发布表情数据与资源索引。安装包不包含整套产品图片／视频／字体、分析文件、`synapse.pack` 或 HF 访问令牌，也不会在安装时下载产品资源。

## 目录和生命周期

| 路径 | 内容及生命周期 |
| --- | --- |
| `{app}/razer_ui.exe` | 程序；升级替换 |
| `{app}/resource-catalog.sqlite3` | 版本固定的独立目录库；升级替换，与 EXE 内容身份一致 |
| `{app}/*.dll` | 仅命令明确指定且通过依赖闭包验证的应用运行库；不复制系统 DLL |
| `{app}/Razer UI.ico` | 从 Git 保留的原生 RGBA 尺寸无损封装，供快捷方式及卸载列表使用 |
| `{app}/razer-ui-installation.json` | 安装方式、版本、体系结构、主程序及相对缓存位置 |
| `{app}/assets/cache/` | 程序按需获取的资源缓存；升级保留，后续启动和离线可复用 |
| `{app}/assets/cache/catalogs/<内容SHA>.sqlite3` | 从 EXE 旁独立目录库原子发布的只读表情与资源目录；缺失或损坏离线恢复，升级使用新内容版本并保留已有缓存 |

`crates/razer-resource-db/installation.json` 是目录库来源、安装文件名及缓存相对目录的单一配置；Cargo、运行时和安装准备工具共用。缓存位置通过 `razer-resource-db::InstallationPaths` 按当前主程序所在目录统一计算，不依赖工作目录、源码仓库的 `assets/` 或机器上的另一份安装。目录库在 UI 构造前发布、核验并打开，权限、文件或数据库错误会阻止初始化，不返回空词表。媒体仍按需读取和验证，安装器不预先取回完整资源集。目录库副本属于同一缓存生命周期；卸载保留缓存时保留它们，选择清理时一起限定在本安装的 `assets/cache/` 内删除，重新安装可再次从独立安装文件恢复目录库。

目录库和媒体共用 `razer-resource-db::StagedResource` 管理暂存与原子替换。发布前同步并关闭暂存文件；取消和失败只清理本次成功独占创建的文件，保留有效目标和其他任务的暂存。媒体的取消／提交顺序由资源运行时决定，目录库版本及媒体内容身份分别由其所有者核验；并发替换失败不能未经核验就视为成功。

开始菜单始终创建入口，桌面快捷方式由用户选择；安装完成不自动运行程序。更新使用 Inno Setup 的文件占用检测，保留缓存，不强制终止进程。安装凭据同时记入 HKCU `Software\Razer UI` 的 `InstallLocation`、`CacheRelative`、`Version`。x64 与 arm64 共用稳定 AppId，同一安装位置升级，不注册成两份互相覆盖的应用。

交互卸载询问是否删除下载的产品资源；选择保留可供重新安装后离线复用。静默卸载默认保留，`/KEEPASSETCACHE=0` 明确清理，`/KEEPASSETCACHE=1` 明确保留。配置、设备档案和用户文件不属于资源缓存清理范围。

清理前校验 HKCU 中记录的安装位置与当前卸载器所属目录一致，只清理其下确切的 `assets/cache/`；再次拒绝链接或目录联接。Inno Setup 的 `DelTree` 不递归进入缓存内部的重解析点。删除失败会记录日志，交互卸载同时展示保留位置，不报告已清理成功。不会递归删除整个安装目录。

## 维护入口

由 `tools/workspace.py` 调用 `tools/windows_installer.py`，准备阶段只读取已经存在的 EXE 和 DLL，不执行构建、应用或安装器：

```powershell
python tools/workspace.py prepare-windows-installer
python tools/workspace.py verify-windows-installer --stage target/installer/windows/0.1.0-x64
```

`--exe` 可以指定已有 Windows GUI EXE；默认 `target/release/razer_ui.exe`。版本默认读取 Cargo 包版本；`--version` 只接受 Windows 版本资源可表示的数字 `x.y.z`。`--architecture` 支持 `x64` 和 `arm64`；`--output` 必须是 `target/installer/windows/` 下的独立目录。不会覆盖不同内容的已有准备结果。

若 EXE 导入 VC 运行库等非系统 DLL，必须通过可重复的 `--runtime-dll <文件>` 显式提供。工具静态解析所有输入 PE 的普通及延迟导入，核对体系结构、GUI 子系统、EXE/DLL 类型和传递依赖；缺少运行库、额外无用 DLL、系统 DLL 复制、载荷更改、链接路径、注入资源文件均导致失败。不会从分析目录或产品资源目录递归收集依赖。

准备产物保存在 `target/installer/windows/<版本>-<体系结构>/`。`installer-input.json` 记录脚本、每个输入文件、原生图标来源的真实 SHA-256；`payload/` 是严格允许列表，`runtime-files.iss` 逐项列出运行库，禁止通配打包。校验会重新计算内容、PE 依赖及图标字节，不能只改清单后加入任意安装文件。

发布环境另行安装 Inno Setup 6.3 或更新版本后，可以编译安装包：

```powershell
python tools/workspace.py compile-windows-installer --stage target/installer/windows/0.1.0-x64 --iscc 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
```

此命令只调用现成 ISCC 编译，不下载编译器、不启动生成的 Setup。输出放在独立的 `<准备目录>-compiled/`，返回文件长度和 SHA-256。签名证书不在仓库中；未配置签名时不得将产物称为已签名。

## 静态验证边界

安装路径归属、当前用户权限、缓存保留和卸载边界属于安装架构；Razer 设备协议仍需当前原始源码与二进制证据。资源仓库和固定提交由 `tools/asset-release.json` 及其生成的 SQLite 索引管理，程序按实际请求取得单个媒体文件，长度与 SHA-256 校验通过后才发布到缓存。有效缓存无需凭据或网络即可复用。

Inno Setup 行为对照官方文档：[最低权限](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm)、[体系结构](https://jrsoftware.org/ishelp/topic_setup_architecturesallowed.htm)、[文件占用](https://jrsoftware.org/ishelp/topic_setup_closeapplications.htm)、[卸载事件](https://jrsoftware.org/ishelp/topic_scriptevents.htm)、[限定删除](https://jrsoftware.org/ishelp/topic_isxfunc_deltree.htm)。系统 ICU 最低版本对照 [Microsoft ICU 说明](https://learn.microsoft.com/en-us/windows/win32/intl/international-components-for-unicode--icu-)。参考源码只用于静态阅读。

纯 Python 合成 PE 输入测试入口为 `python -m unittest -v test_windows_installer`（工作目录 `tools/`）。测试验证依赖闭包、错误输入、文件注入、链接、变更拒绝和 RGBA 像素保真，不执行任何测试二进制。

准备安装包需要已经存在的 release EXE，编译安装包需要已经安装的 ISCC；缺少输入时入口明确失败。当前没有生成安装包：默认 release EXE 缺失，ISCC 未安装；没有运行应用或安装器。工具测试不能代替安装窗口、实际升级、卸载及 Windows 运行时依赖验收。真实安装与应用运行仍遵守项目执行限制，实际验证结果归入统一验证记录。

## 官方安装包装层静态捕获

2026-10-24 用 IDA 9.1、Hex-Rays 9.1.0.250226 和 Python 3.11.9 并行捕获了两个此前没有完整 `capture.json` 的官方 PE。每个模块由独立 IDA 进程处理，原件 SHA、数据库和输出目录隔离；没有执行安装器、子进程或设备服务。

| 原件 | 版本资源 | 原件 SHA-256 | 完整函数 | 导入／字符串 | 捕获收据 |
| --- | --- | --- | ---: | ---: | --- |
| `RzComDriver_v23.0.6.0.exe` | Razer ComDriver Installer 23.0.6.0 | `d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc` | 105 | 165／6,928 | [`capture.json`](../.analysis/results/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/capture.json) |
| `1790573318Wp33aPzORazerChroma-Web-v4.0.827.exe` | Razer Chroma Setup 4.0.827.0 | `1a7eecf362e5de07c9bfd799bcccbdf3327740ffb3d713bc4c3649110bb94586` | 105 | 163／6,904 | [`capture.json`](../.analysis/results/native/1a7eecf362e5de07c9bfd799bcccbdf3327740ffb3d713bc4c3649110bb94586/capture.json) |

两份原件都是 32-bit PE、ImageBase `0x400000`、5 个段，唯一入口分别为 `start` RVA `0x34f7` 与 `0x3557`。共同的捕获函数包含 `DialogFunc`、`start`、`StartAddress`、注册表辅助函数和文件操作辅助函数；没有可作为设备协议 API 的导出表。`SETUPAPI`、`Software\\Microsoft\\Windows\\CurrentVersion`、`verifying installer: %d%%`、完整性失败、临时文件失败和启动失败等字符串与导入表一起确认这是 NSIS 安装包装层，不能按 `RzComDriver` 或 `RazerChroma-Web` 文件名把它当作运行时设备库。

入口 `start` 解析命令行开关，读取自身文件并调用 `sub_40307D`（[`RzComDriver 307d`](../.analysis/results/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/functions/307d.json)）检查 NSIS 尾部标记、压缩数据长度和 CRC32；标记、读取或 CRC 不匹配返回安装器完整性错误。`sub_4032B4`（[`RzComDriver 32b4`](../.analysis/results/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/functions/32b4.json)）按块读取／解压，进度通过窗口更新；`sub_4034C6`（[`RzComDriver 34c6`](../.analysis/results/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/functions/34c6.json)）准备临时目录和临时文件。

解压后的控制路径使用 `CreateFileW`、`ReadFile`、`WriteFile`、`CopyFileW`、`MoveFileExW`、`DeleteFileW`、`RemoveDirectoryW` 和属性恢复；子进程由 `CreateProcessW`（[`RzComDriver 5aea`](../.analysis/results/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/functions/5aea.json)）启动，`WaitForSingleObject` 以 100 ms 轮询并泵送窗口消息，随后读取 `GetExitCodeProcess`（[`RzComDriver 697f`](../.analysis/results/native/d17bbc15e025bcee12948655ed5956bfe855019e626e497dd10ed40cb9a5e0dc/functions/697f.json)）。注册表和 INI 辅助路径会读取／写入／枚举／删除键值，但本批没有把任何设备设置、HID 报文或服务 ACK 暴露到包装层。

`GetSystemDirectoryW` → `LoadLibraryExW` → `GetProcAddress` 是 NSIS 对系统兼容 DLL 的动态解析通道；捕获只证明该通道存在，不证明某个厂商 DLL 被加载或导出被调用。后续若要逆向 ComDriver 或 Chroma 的真实设备能力，必须继续追踪其解压载荷和父进程消费者，并分别捕获对应 DLL／服务；本节不提升 Rust 实现、官方 DLL 对照或真实设备验收状态。

## 新一批工具包装层静态捕获

2026-10-24 另用两个独立 `capture-native` 进程并行执行 IDA 9.1，分别记录 PID `16892` 和 `14260`；两件都是 32-bit PE，ImageBase `0x400000`，5 个段，完整反编译 105 函数且 0 个失败。原件、开始与结束时间、IDA 日志和产物均在各自 capture 目录中；没有运行安装器、子进程、服务或设备。

| 原件 | 版本资源 | 原件 SHA-256 | 导入／字符串 | capture 与 IDA 日志 |
| --- | --- | --- | ---: | --- |
| `RazerXTU-v1.0.8.0.exe` | `RazerXTU` 1.0.8.0 | `61f3c84e765e044feb13d6cd053e113e9a246f138a8e957d8edc84e9d229ba8f` | 163／6907 | [`capture.json`](../.analysis/results/native/61f3c84e765e044feb13d6cd053e113e9a246f138a8e957d8edc84e9d229ba8f/capture.json) · [`ida.log`](../.work/native/61f3c84e765e044feb13d6cd053e113e9a246f138a8e957d8edc84e9d229ba8f/batch/ida.log) |
| `Blade10ToolsSetup-v1.0.0.9.exe` | `Blade10Tools` 1.0.0.9 | `3bd1c893b0aeffff995c886b05f716647291bc5f1279fa8aa3fefe51ec33012d` | 165／6928 | [`capture.json`](../.analysis/results/native/3bd1c893b0aeffff995c886b05f716647291bc5f1279fa8aa3fefe51ec33012d/capture.json) · [`ida.log`](../.work/native/3bd1c893b0aeffff995c886b05f716647291bc5f1279fa8aa3fefe51ec33012d/batch/ida.log) |

`RazerXTU` 的 `start` RVA `0x351c`，`DialogFunc` RVA `0x2fb8`，`StartAddress` RVA `0x5699`；`Blade10ToolsSetup` 对应为 `0x34f7`、`0x2f93`、`0x563c`。两者的包装层都按同一条 NSIS 路径工作：自身文件打开与尾部标记校验，块读取／解压，临时文件回退，然后发起子进程并读取退出码。`RazerXTU` 的核心函数为 [`30a2`](../.analysis/results/native/61f3c84e765e044feb13d6cd053e113e9a246f138a8e957d8edc84e9d229ba8f/functions/30a2.json) 自身校验、[`32d9`](../.analysis/results/native/61f3c84e765e044feb13d6cd053e113e9a246f138a8e957d8edc84e9d229ba8f/functions/32d9.json) 分块处理、[`34eb`](../.analysis/results/native/61f3c84e765e044feb13d6cd053e113e9a246f138a8e957d8edc84e9d229ba8f/functions/34eb.json) 临时文件回退；`Blade10ToolsSetup` 的对应函数为 [`307d`](../.analysis/results/native/3bd1c893b0aeffff995c886b05f716647291bc5f1279fa8aa3fefe51ec33012d/functions/307d.json)、[`32b4`](../.analysis/results/native/3bd1c893b0aeffff995c886b05f716647291bc5f1279fa8aa3fefe51ec33012d/functions/32b4.json)、[`34c6`](../.analysis/results/native/3bd1c893b0aeffff995c886b05f716647291bc5f1279fa8aa3fefe51ec33012d/functions/34c6.json)。

`start` 在这两个包装中都会初始化 OLE，检查临时路径，调用自身校验后创建并执行解压输出；输入标记、CRC 或文件读取失败会返回 `Installer integrity check has failed` 或临时文件错误。`StartAddress` 遍历配置项并在 OLE 清理后返回包装层计数；这不是设备初始化或设备 ACK。

`CreateProcessW` 、`WaitForSingleObject(100 ms)`、`GetExitCodeProcess`、注册表/文件替换和 `LoadLibraryExW`/`GetProcAddress` 仍然只能证明包装层有这些渠道。两个原件都没有在本次捕获中暴露 HID 报文、服务 ACK 或设备读回；真实 RazerXTU/Blade 功能仍需继续追踪解压载荷和父进程消费者。两份 `all-functions.asm` 和 `all-functions.c` 也已在对应 capture 目录保存，其中一份 ASM 与 C 输出 SHA 分别为 `0df4e566f5d3aa613226ef5c3ea519f9fb898c941f01bf1e759a308746f903d8` / `af51fbd410dab48c596a229217c453f085c632b4ba747ba7b7b63d44985c2a59`，另一份为 `569368519c50e4c008bae776c178399a1faa144757f3edb8a9aee2d16d1a95f2` / `61ee72d6d94f5ffe252de2e280ba68107a84a29b24eab8b10465912614443c35`。
