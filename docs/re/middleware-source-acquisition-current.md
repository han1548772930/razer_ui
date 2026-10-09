# 当前 middleware 全部声明 JavaScript 取得链

2026-10-09。本次处理已有当前源码库中的 `.ref/middleware/<productId>/webpackManifest.json`，补取每份固定清单已声明而本地缺失的 JS。此前已取得 HTML、manifest、入口脚本和部分 chunk 并不等于 middleware 代码已全量取得：初始 **331 份清单共有 27,544 个缺失产品/文件路径，按文件名去重仍有 26,603 个**。

维护工具：[acquire-all-middleware-sources.py](../../tools/acquire-all-middleware-sources.py)。聚合状态、全部 manifest SHA-256、原缺项列表定位、实际成功/未成功项及字节数见 [middleware-source-acquisition-current.json](middleware-source-acquisition-current.json)。**以其中 `all_original_missing_acquired`、`verified_paths`、`unresolved_paths` 为实际完成状态；文档存在或任务启动不表示下载完成。**

本次最终结果：**27,544 个初始缺项全部取得，共 876,645,965 字节，未解决项 0，当前固定 middleware 清单中的 JS 缺项 0**。第一轮的 12 个临时读取超时/网关错误在恢复轮全部成功；随后 `--verify-only` 再次校验全部取得字节、对应 HTTP 收据以及 331 份固定 manifest SHA，通过。这里只声明清单 JS 的来源取得完成；下文的语义逆向、source map 与二进制边界仍存在。

## 来源与取得顺序

1. 只读当前 331 个已有 webpackManifest，记录各原始文件 SHA-256，取所有以 `.js` 结尾的 values，按 `(productId,file)` 去重。
2. 固化初始缺项到 `.ref/middleware/ALL-MANIFEST-SOURCE-REQUESTS.json`。后续恢复任务必须与原 manifest 集合及 SHA 完全一致；不刷新 manifest、HTML，也不覆盖已有源码。
3. 对缺项直接 GET `https://apps.razer.com/synapse/products/<productId>/mw/<file>`，标准库 HTTPS 校验证书、每线程复用连接，最多 24 个并行 worker。拒绝 redirect、非 200、HTML/空响应和超过单文件 16MiB 的响应。
4. 文件只以 exclusive create（`xb`）方式落盘，不覆盖共享目录已有源码。每份成功响应旁保留 `.http.json`：source_url、final_url、http_status、取得时间、SHA-256、bytes、ETag、Last-Modified、content_type、body_file、所属 manifest 路径。
5. 失败保留 `.fetch-error.json`；批次保存 `.ref/middleware/ALL-MANIFEST-SOURCE-PROGRESS.json`。HTTP 403/404/410 记录具体断点，不重复盲目请求；其他网络错误最多两次尝试。再次启动会继续未取得文件，最终成功与旧 failure receipt 的历史记录需按 `.http.json`/聚合最终核实判定。
6. 完成或指定样本结束时，重新验证全部 original missing 的本地 bytes/receipt 以及所有 pinned manifest SHA，生成聚合 MD 引用的 JSON。文件名、路径、hash 相同与内容从对应产品 URL 确实取得是不同证据；没有仅按相同文件名复制到不同产品并伪装 HTTP 收据。

## 可重复命令

```powershell
python tools/acquire-all-middleware-sources.py --plan-only
python tools/acquire-all-middleware-sources.py --limit 64 --batch-size 64
python tools/acquire-all-middleware-sources.py --batch-size 512
python tools/acquire-all-middleware-sources.py --verify-only
```

`--plan-only` 仅建立固定请求集合，不联网；`--limit` 是明确不完整的样本；正常命令尝试全部尚缺文件；`--verify-only` 不联网，只验证已有 bytes/receipt 并更新聚合报告。验证不加载 JavaScript，不调用 DLL，不运行应用/构建/测试。

## 完整性边界

webpack hashed filename 不等于 manifest 声明了独立的完整文件 SHA-256。HTTP 200、source URL 与本次取得 SHA-256 只能证明**当前这次响应的来源和本地字节一致性**；不能据此声称取得历史编译时的精确源码，也不能因为 HTTP 成功便认定行为兼容。未带 hash 的 `service-worker.js` 尤其需要保留取得时间，后续全量 AST/依赖图应标记来源日期边界。

补齐清单中的所有 JS 不等于恢复 source-map 注释指向但未取得的 map，不等于将 minified 模块还原为作者原项目目录，更不等于 DLL/.node/EXE 内部逆向完成。接下来应按全量源码地图对每个 middleware action、device identity、FFI/transport、callback/state publication 和页面消费继续静态追踪，而不能把 acquisition 状态当作接入或运行验证状态。

当前宿主入口与这些 middleware 的 IPC/FFI 对应关系见 [host-architecture-current.md](host-architecture-current.md)；产品声明到 ABI/PE 及本地接入边界另按总逆向地图整理。

## 全量语法与模块索引复核

补采后使用 [audit-all-middleware-code-current.cjs](../../tools/audit-all-middleware-code-current.cjs)，对上述 **331 份固定清单中的全部 28,931 个 JS 路径**（含此前已存在的 1,387 个）重新计算源码 SHA-256，按 **28,304 个独立内容 hash** 去重后用 Acorn 8.15.0 静态解析，每个独立内容实际解析一次。所有 **28,931 个文件语法解析通过，失败 0**。源码按产品路径计共 1,687,891,178 字节；它包含原有文件与本次补采，不能与仅 missing 的 876,645,965 字节混用。

轻量汇总：[middleware-code-current-summary.json](middleware-code-current-summary.json)。完整压缩索引：[middleware-code-current-evidence.json.gz](middleware-code-current-evidence.json.gz)，含每文件 product/path/hash/bytes/parse_status，以及按独立 hash 保存的 AST 统计、webpack module id/函数范围、导入候选、lazy chunk 候选、action field/switch 候选。没有保存完整 AST 或每个原函数全文，没有加载/运行厂商 JS。

| 全部声明路径累计项 | 实际静态统计 |
| --- | ---: |
| 函数 AST 节点 | 12,210,251 |
| webpack module 形式候选 | 290,732 |
| module require 参数调用候选边（每模块目标去重） | 923,812 |
| lazy chunk 调用候选 | 38,310 |
| action 字段/switch 候选 | 256,568 |

这些是**按所有声明产品路径累计、包含共同库重复内容**的语法节点，不是独立产品功能数。当前文件经过 webpack 打包，ES import / 字面量 `require` 顶层语法计数为 0，模块依赖必须读上表及详细索引的 webpack require candidates，不能据此说没有依赖。numeric-key/function object 作为 module 形式候选，require 参数名字匹配尚有词法遮蔽歧义；候选 action 不证明可达性、IPC 路由、原生 ABI 或设备操作成功。

可重复校验为 `node tools/audit-all-middleware-code-current.cjs --check`。它重新读取固定 manifest/源码字节和 hash，按维护工具版本及 Acorn 版本匹配内容缓存，重建并对比 gzip/summary；`--check` 不写源、证据或缓存。缓存只是解析成本优化，不免除当前源字节核验。

**全量语法通过和模块索引完成仍不等于全部 1,221 万个函数节点逐一语义逆向完成**。后续要沿实际页面 action、模块引用与产品/DLL wrapper 完成去重和跨层调用链；source map、作者原工程目录、二进制内部与运行时服务结果仍保持前述边界。
