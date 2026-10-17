# 资源索引与编译边界

产品仅内嵌必需的 9 个原生 RGBA 图标与 4 个固定窗口控制 SVG，资源字节入口集中在 `native_icons.rs` 和 `window_icons.rs`。SQLite 目录库以独立文件随安装交付，不编入 PE。`tools/resource-mappings.json` 是资源 alias 与图片选择条件的统一维护入口，替代旧 Rust 表目录；`assets/synapse/resource-catalog.sqlite3` 由统一入口生成，保存完整表情数据及固定资源索引。`razer-resource-db` 统一管理独立目录库的离线原子发布、安装缓存位置与只读连接；[razer-resources::catalog](../razer-resources/src/catalog.rs) 解码并验证有类型索引，保留原 alias、加载优先级和列表语义，render 不执行 SQL，媒体由运行时按单个文件获取。

索引保留源表顺序、重复 alias、sprite fragment 和本地音频 demo 控件的优先级，未注册的键仍交给 GPUI Kit `AllAssets`。已声明而加载失败的文件保持真实错误，不能伪装为缺失键或空图。

运行时从私有资源仓库的固定提交按所有当前界面（包括设置、宏、帮助、独立窗口与产品页）所需文件获取，缓存到当前 EXE 所在目录的 `assets/cache/`。有效缓存离线复用；不会启动时预取其他产品、恢复源码 `assets/` 或解压整套资源。Cargo 不执行网络获取，也没有 staging pack 的构建脚本；编译及安装均无需 `synapse.pack`。`installation.json` 统一 Cargo、运行时和安装器路径。无 GPUI 依赖的 `razer-resources::ResourceRuntime` 拥有队列、Tokio 工作线程及实际 HTTP 客户端；应用注入 client，本 crate 只保留 GPUI 适配、窗口解码缓存、资源作用域和通知。应用退出等待资源 owner 取消并关闭工作线程。同一界面的实际请求按 8 ms 收集、每批至多 64 文件、最多 4 路并发；客户端初始化、凭据查找、磁盘读取、校验及发布在资源线程执行，界面不等待这些 I/O。教程媒体同样使用等待就绪的缓存，避免首次下载尚未完成就永久记成解码失败。独立的 `razer-asset-pack` 容器核心仍可用于格式验证，详见 [资源交付与容器格式](../../docs/asset-pack-format.md)。

静态检查和允许的纯 Rust library 测试：

```powershell
python tools/workspace.py verify-runtime-assets
python tools/workspace.py prepare-resource-database --check
python tools/workspace.py prepare-typed-data --check --crate razer-assets
cargo check --locked --offline -p razer-assets --all-targets
cargo test --locked --offline -p razer-resources --lib
```

`razer-resources` 的纯 Rust／模拟传输测试检查索引身份、路径与产品分支、缓存校验、请求合批、去重、失败重试和关闭；不要求整套本地图片，也不下载资源。`cache-inspection` 仅供显式的已安装缓存检查测试，不能成为生产同步读取入口。完整原件解码属于维护工具验证，不能替代应用窗口或设备验收；GPUI 窗口测试不在允许的执行范围内。各项结论以当前 [验证记录](../../.analysis/validation.json) 的源码身份和结果为准。完整边界见 [AGENTS.md](../../AGENTS.md)。
