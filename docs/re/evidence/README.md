# 全量逆向证据存档

这里保存大体积机器证据，正文与功能说明仍在上级目录。29 份 ZIP 各只有一个 JSON 成员，另 1 份复用上级现有的 gzip，成员保留原文件的完整字节；没有删掉函数正文、原文摘录、参数、返回值、页面细节、产品分支或未闭合链路。

[完整索引](index.json) 记录原路径、成员名称、原文件大小/SHA-256、ZIP 大小/SHA-256。源码收据、UTF-16 区间、二进制身份及证据间引用保持原值；索引中的 `original_path` 是工具使用的原逻辑路径，对应 ZIP 内的文件，不代表丢失证据。

30 份原 JSON 共 527.96 MiB；29 份 ZIP 和 1 份共享 gzip 共约 55.01 MiB。完整页面语义汇总原 ZIP 与 gzip 解压字节相同，现只保留 gzip。原有 `.gz` 中部分包含更完整的内部树或机器码，与同名 JSON 摘要内容不同；它们继续保留，未按名称相同删除。已有 `.gz` Git 忽略规则保留；本次 `.zip` 不受该规则影响。

## 查看与校验

可以用 Windows 文件管理器直接打开 ZIP，读取或解压其中的 JSON。需要原路径时：

```powershell
python tools/evidence-store.py materialize
```

工具不会覆盖已存在的文件。查看完毕后移除内容未变的解压副本：

```powershell
python tools/evidence-store.py dematerialize
```

只删除与存档原 SHA 完全一致的副本；修改过的文件继续保留，ZIP 不变。Windows 文件临时占用时有限重试，并继续处理其他副本。重新整理或校验：

```powershell
python tools/evidence-store.py archive --min-mib 2
python tools/evidence-store.py check
```

## 继续运行现有静态工具

工具通过临时恢复原文件运行现有解析器，结束后删除本次恢复且内容未变的副本。不会修改解析算法、源码 SHA 或生成器身份；如果工具改变了结果，保留该文件供检查，不会自动删除。

```powershell
python tools/evidence-store.py run tools/prepare-discovery-catalog.py --check
python tools/evidence-store.py run tools/audit-native-chains-current.cjs --check
```

`run` 只接受 `tools/` 下的 Python/Node 静态检查脚本；非 `validate-*` 工具需要 `--check`。执行带输出的静态生成器时，先 `materialize`，按原工具命令生成，核实新结果后再 `archive`。它不会运行应用、测试、下载的 JavaScript、DLL 或硬件命令。
