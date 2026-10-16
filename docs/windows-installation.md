# Windows 安装交付

程序以安装包交付。`packaging/windows/RazerUI.iss` 使用 Inno Setup，默认安装到当前用户的 `%LOCALAPPDATA%\Programs\Razer UI`，无需管理员权限。可以选择其他可写目录；安装路径及缓存路径不能穿过符号链接或目录联接。最低系统设为 Windows 10 1703（15063），对应已存在的 GPUI EXE 静态导入的系统 ICU C API。

安装内容只包括 Windows GUI 程序、必要且显式指定的运行库、原生图标及安装凭据。安装包不会包含产品资源目录、分析文件、`synapse.pack` 或 HF 访问令牌，也不会在安装时下载产品资源。

## 目录和生命周期

| 路径 | 内容及生命周期 |
| --- | --- |
| `{app}/razer_ui.exe` | 程序；升级替换 |
| `{app}/*.dll` | 仅命令明确指定且通过依赖闭包验证的应用运行库；不复制系统 DLL |
| `{app}/Razer UI.ico` | 从 Git 保留的原生 RGBA 尺寸无损封装，供快捷方式及卸载列表使用 |
| `{app}/razer-ui-installation.json` | 安装方式、版本、体系结构、主程序及相对缓存位置 |
| `{app}/assets/cache/` | 程序按需获取的资源缓存；升级保留，后续启动和离线可复用 |

缓存位置按当前主程序所在目录计算，不依赖工作目录、源码仓库的 `assets/` 或机器上的另一份安装。具体按需读取和失败处理由资源运行时负责，安装器不预先取回完整资源集。

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

安装路径归属、当前用户权限、缓存保留和卸载边界属于安装架构；Razer 设备协议仍需当前原始源码与二进制证据。资源仓库和固定提交由 `tools/asset-release.json` 及其生成的 Rust 索引管理，程序按实际请求取得单个文件，长度与 SHA-256 校验通过后才发布到缓存。有效缓存无需凭据或网络即可复用。

Inno Setup 行为对照官方文档：[最低权限](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm)、[体系结构](https://jrsoftware.org/ishelp/topic_setup_architecturesallowed.htm)、[文件占用](https://jrsoftware.org/ishelp/topic_setup_closeapplications.htm)、[卸载事件](https://jrsoftware.org/ishelp/topic_scriptevents.htm)、[限定删除](https://jrsoftware.org/ishelp/topic_isxfunc_deltree.htm)。系统 ICU 最低版本对照 [Microsoft ICU 说明](https://learn.microsoft.com/en-us/windows/win32/intl/international-components-for-unicode--icu-)。参考源码只用于静态阅读。

纯 Python 合成 PE 输入测试入口为 `python -m unittest -v test_windows_installer`（工作目录 `tools/`）。测试验证依赖闭包、错误输入、文件注入、链接、变更拒绝和 RGBA 像素保真，不执行任何测试二进制。

准备安装包需要已经存在的 release EXE，编译安装包需要已经安装的 ISCC；缺少输入时入口明确失败。当前没有生成安装包：默认 release EXE 缺失，ISCC 未安装；没有运行应用或安装器。工具测试不能代替安装窗口、实际升级、卸载及 Windows 运行时依赖验收。真实安装与应用运行仍遵守项目执行限制，实际验证结果归入统一验证记录。
