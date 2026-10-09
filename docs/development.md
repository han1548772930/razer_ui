# 开发与静态检查

本项目按当前官方源逐项实现 UI，使用 `D:\rust_test\gpui-kit\skills` 中的 [GPUI Kit 约定](../../gpui-kit/skills/gpui-kit/SKILL.md)和[设计约定](../../gpui-kit/skills/gpui-kit-design-guides/SKILL.md)。组件需要稳定身份、状态所有者、焦点与清理路径；原版特有布局和交互以实际挂载源码为准。

允许的 Rust 检查：

```text
cargo check --locked --all-targets
cargo fmt --all -- --check
git diff --check
```

静态检查入口：

```text
python -X utf8 tools/validate-ui-fix-registry.py
python -X utf8 tools/validate-current-docs.py
python -X utf8 tools/audit-feature-capabilities.py --check
python -X utf8 tools/validate-embedded-json.py
```

按改动范围补对应契约列出的源码、生成数据和资源 `--check`。收据发生变化时先核对原因，再更新相应记录；不要将重新计算指纹写成重新验证全部 UI。

滚动容器应让视口高度、实际滚动节点和 ScrollHandle 保持同一所有者；浮层要核对底层 Capture 与指针遮挡，不能仅检查按钮回调是否存在。状态观察与本地编辑分开，纯查询不产生 dirty 或设备保存成功。

禁止运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。新增回归用例只做编译检查。源码工具只能解析数据，不得通过 require/eval/import 执行厂商代码；静态资源准备遵守 [资源契约](re/resources-current.md)。
