# 资源索引与编译边界

产品只编译有类型的资源索引和必需的 9 个原生 RGBA 图标，避免把整套图片放入 PE。`assets/synapse/tables/asset-tables-source.rs` 供维护工具解析，不作为携带图片的 Rust 模块编译；`remote-assets.rs` 由固定资源清单生成。[src/lib.rs](src/lib.rs) 保留原资源 alias、加载优先级和列表语义，载荷由资源运行时按单个文件获取。

索引保留源表顺序、重复 alias、sprite fragment 和本地音频 demo 控件的优先级，未注册的键仍交给 GPUI Kit `AllAssets`。已声明而加载失败的文件保持真实错误，不能伪装为缺失键或空图。

运行时从私有资源仓库的固定提交按当前界面所需文件获取，缓存到当前 EXE 所在目录的 `assets/cache/`。有效缓存离线复用；不会启动时预取其他产品、恢复源码 `assets/` 或解压整套资源。Cargo 不执行网络获取，也没有 staging pack 的构建脚本；编译及安装均无需 `synapse.pack`。独立的 `razer-asset-pack` 容器核心仍可用于格式验证，详见 [资源交付与容器格式](../../docs/asset-pack-format.md)。

静态检查和允许的纯 Rust library 测试：

```powershell
python tools/workspace.py verify-runtime-assets
python tools/workspace.py prepare-typed-data --check --crate razer-assets
cargo check --locked --offline -p razer-assets --all-targets
```

纯 Rust 测试检查索引身份、路径与产品分支、缓存校验和模拟请求；不要求整套本地图片，也不下载资源。完整原件解码属于维护工具验证，不能替代应用窗口或设备验收。各项结论以当前 [验证记录](../../.analysis/validation.json) 的源码身份和结果为准。完整边界见 [AGENTS.md](../../AGENTS.md)。
