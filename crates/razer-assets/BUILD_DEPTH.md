# 资源包与编译边界

资源表反复嵌套 `std::iter::Chain` 曾引发类型深度溢出；全量图片内嵌又触发 Windows PE 链接的 2 GiB 上限。当前维护源表在 `assets/synapse/tables/asset-tables-source.rs`，不会作为 Rust 模块编译；[src/lib.rs](src/lib.rs) 保留原加载与列表语义，大资源由 [src/packed.rs](src/packed.rs) 按需从外部包读取并逐项校验。

表顺序决定同名资源的首项匹配；轻量 `asset-list.json` 保留原始顺序和重复项。sprite、厂商本地音频 demo 控件和 GPUI Kit `AllAssets` 的优先级保持不变；它们与已删除的假设备 demo 不是同一功能。

正式 `target/asset-pack/synapse.pack` 是可重建生成物，已从 Git 提交范围排除，也不进入 HF 原文件资源仓库；打包分发时仍须在程序旁携带 `assets/synapse.pack`。克隆源码后按下面的维护命令从本地源资源生成，或取得与该源码版本配套的发布包。构建脚本只复制本地生成包到 Cargo profile 的 `assets/`，不联网、不运行厂商程序，也不读取分析原件。

静态检查和允许的纯 Rust library 测试：

```powershell
python tools/workspace.py audit-asset-pack --prepare
python tools/workspace.py prepare-asset-pack
python tools/workspace.py verify-asset-pack
cargo check --locked --offline -p razer-assets --all-targets
python tools/workspace.py validate-rust --suite assets
```

旧的 7 项 assets 测试记录不适用于本次迁移。全量 codec 解码、真实 list/load 回环及全目标检查须以当前 [验证记录](../../.analysis/validation.json) 的源码身份和结果为准。应用窗口与真实设备验收未执行。完整边界见 [AGENTS.md](../../AGENTS.md)。
