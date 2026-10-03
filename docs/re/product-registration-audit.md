# 全量产品源码导航注册

2026-10-03 静态核验。注册 331 个已有官方 UI 入口的 ID，其中 321 个在上一轮清单中尚未注册。此数字是产品 ID 数，不是独立型号数、已完成界面数或可操作硬件数。

维护工具 [generate-product-registry.cjs](../../tools/generate-product-registry.cjs) 从 [product-catalog.json](product-catalog.json) 读取导航，通过当前维护中 `inventory-razer-interfaces.cjs` 的扫描器 SHA-256 核对原始 AST 清单，并重新核对每份导航源码的 SHA-256。工具用 Acorn 静态解析 363 个导航数组及 1452 个页面对象，核对每页实际 `component` 或 `renderComponent` 源表达式；从未执行下载的代码。

生成的 [逐 ID 记录](product-registration-audit.json) 和 [Rust 注册表](../../src/product/registry_data.rs) 保留产品 ID、官方名称、原分类、edition、原导航顺序、原始页面 ID、翻译键/字面量、组件表达式、条件 class、源码路径和偏移。连接别名和父产品关系完整保存在逐 ID 记录，不合并成同一型号。没有 UI 入口的另外 265 个候选 ID 未被伪造成可用界面。

## 主导航与独立模式

331 个产品均得到有源码根挂载依据的 `primary_navigation()`，没有未解析的导航组。主导航不是“第一个数组”：

- 普通类组件追踪准确位置的导航 owner、Redux/HOC 包装、实际 JSX 挂载和最终根 `displayMode` 分支。setupStatus READY 外层组件保留在链中，不能跳过源门控推断当前硬件已 READY。
- 采用延迟入口的产品逐层核对默认 displayMode、条件 setupStatus 分支、chunk/module ID 和父源码 SHA-256。
- 没有 displayMode 分流的产品追踪至实际 `createRoot(...).render(...)`。Babel 类、具名函数、匿名 useMemo 根、简单赋值别名、包裹函数的连接调用均按实际源表达式处理。
- 根选择表达式中的负条件单独保留，不把条件文本误当作 URL 参数。例如 791 的 `chromaApp !== mode` 路径仍包含默认产品页面。
- 131/182 等配对数组由根三元分支确定为 `multiDevicePairing`，不会接到产品主导航。3886 的两份数组分别对应 `chromaApp` 和默认根，保留其不同 Help 挂载。
- 主导航中的真正 `TAB_PAIRING` 页面仍是普通产品页面，例如 241。不会仅因为页面名包含 Pairing 就把它移出主导航。
- Help 保留在数据原顺序中并标记独立 role，供工作区渲染工具栏入口；120 的两项 Help 具有不同源码偏移和组件参数，注册层不按名称去重。

`ProductPageId` 使用产品 ID、源文件名和原页面对象偏移，页面身份独立于翻译标题。`ProductPageKind` 保留源名称，仅描述导航语义；它不是跨产品共用控件的授权。独立家族适配器须再核对其产品配置、范围、实际组件及条件。

## 注册与适配进度

逐 ID 记录给原有十个已挂载原生工作区标记 `existing_partial_native_adapter`；其余标记 `source_audited_no_native_adapter_claim`。这是注册生成时的基础状态，新家族适配器的控件与限制由各自审计另行记录。注册表本身不声称完成界面，也不提供硬件写入能力。

`Tab::for_product` 继续仅服务原十个适配器；新增工作区使用 `RegisteredProduct` 和独立 `ProductPageId`，不会按类别或同名导航套用 182、653、777 的现有页面。

## 验证方法

- `node tools/generate-product-registry.cjs`：核验当前源并生成注册数据、逐 ID 审计。
- `node tools/generate-product-registry.cjs --check`：重复静态核验，要求生成文件逐字节一致。
- `--inspect=<逗号分隔的ID>`：只打印选定 ID 的根导航证据，不改生成物。

未运行应用、构建、测试、下载 JavaScript 或 DLL。Rust 检查由主任务统一执行 `cargo check --locked --all-targets`。
