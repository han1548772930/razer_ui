# 本地启动

本仓库当前要求代理不运行应用、构建、测试、安装程序或厂商代码；代理可执行 `cargo check --locked --all-targets`、格式化和静态验证。下面是开发者自行启动程序的说明。

如果工作区由 Windows 沙箱配置过，其目录可能带有可继承的 Low Mandatory Level 标签，目录内生成的 exe 也会继承。即使从普通终端 `cargo run`，程序仍可能以 Low 完整性运行，无法向 Medium 完整性的 Explorer 注册托盘。

在普通 PowerShell 中进入项目目录，使用项目外的构建目录：

```powershell
cargo run --target-dir "$env:LOCALAPPDATA\razer-ui-build"
```

此操作不要求管理员权限，不更改仓库 ACL。调试输出目录由 Cargo 命令行参数指定，不写入绝对路径配置；release 构建也可以使用同一 `--target-dir` 参数。

若仍失败，终端 `[tray]` 行会报告注册结果、窗口归属和进程安全状态。4096 为 Low，8192 为 Medium。`elevated=false` 只说明没有管理员提权，不能单独证明进程为 Medium。注册失败时主窗口关闭仍退出，避免隐藏后无法恢复。
