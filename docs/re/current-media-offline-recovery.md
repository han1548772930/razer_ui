# 当前媒体的离线内容指纹恢复

2026-10-05，网络补取仍失败时，维护工具 `tools/recover-current-media.cjs` 直接读取当前生产 manifest、当前源码的资源引用及本地原始媒体候选。它不执行参考应用代码，也不修改历史 HTTP 失败收据。

当前 Dashboard 已缓存的 166 个 SVG 均满足：**文件名中的 8 位内容指纹 = 文件原始字节 MD4 的前 8 位**。本地 `assets/synapse/gr-hotspot-source.svg` 的 MD4 为 `53dd5566d963d7c28e53733c006c064e`，与当前 manifest 的 `gamer_room_hotspot_animation.53dd5566.svg` 相符；SHA-256 为 `16ea857d1571db9eb72fbe03d580373c1c780618fdf74574aa89bd8a7e6b9e88`。在允许的当前缓存及已有打包资源范围内，没有其他内容不同的候选匹配该指纹。

据此将候选原始字节恢复到当前 `.ref/applications/synapse/dashboard/static/media/`，供静态解析原动画使用。`current-media-offline-recovery.json` 保存当前 manifest 哈希、源码引用、全部校准文件及候选完整 MD4/SHA-256。**这是当前内容指纹匹配的离线恢复，不是重新联网取得文件后的独立逐字节比对。** 没有伪造下载成功、重新创建被删除的旧源码目录或为动画自行补时间曲线。

同一方式检查了 Chroma 的 14 个已缓存原 SVG，15 个缺失灰色/动画 SVG 均没有找到匹配候选，仍保留缺口。Dashboard 的独立待机关机、Xbox、PlayStation 图标也没有找到匹配候选。负面结果分别记录在两个 JSON 中，未制造替代文件。

验证命令：

```powershell
node --openssl-legacy-provider tools/recover-current-media.cjs --check
node --openssl-legacy-provider tools/recover-current-media.cjs --chroma --check
```

`--openssl-legacy-provider` 仅启用 Node 内置 MD4 摘要算法，数据输入是 SVG 原始字节；没有运行 SVG、下载的 JavaScript 或 Razer DLL。
