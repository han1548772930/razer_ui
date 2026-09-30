# 重建工作单

## 状态：**已恢复可编译** ✅

```
cargo check --bin razer_ui   →  0 error, 19 warning
cargo test  --bin razer_ui   →  19 passed; 0 failed
```

源码规模：**36 个文件 / 10,441 行**（起始 10,380 行，内容已全部重建）。

## 错误数变化

| 阶段 | 错误数 |
|---|---|
| 起始（语法错清除后） | **184** |
| `model.rs` 重建 | 181 |
| `impl AppShell`（86 个方法）重建 | 29 |
| 各设备页分区重建 | 11 |
| 后端 `EngineLibrary` / `probe_engine` / `handshake` | 3 |
| 视图（顶栏 + 标签栏 + 路由）重建 | **0** |

---

## 重建了哪些东西

### 领域层

- `src/model.rs` —— `Device`（26 字段）+ `SetupStatus` / `Profile` / `DpiStages` /
  `DkmKey` / `FirmwareInfo` / `PowerStatus` + `measured_devices()` + 7 个单元测试
- `src/nav.rs` —— `Tab`（26 变体）+ `DeviceKind`（6 类）+ `tabs()` + 4 个单元测试

### 应用层

- `src/app.rs`
  - `impl AppShell`：**86 个方法**。靠 3 个通用辅助（`edit_features` / `flip` /
    `bump_u8`）把大多数方法压成一行，避免 86 份重复的「改 + 刷新」
  - `AppShell::new()`：自读 `--demo-keyboard` / `--tab`
  - `impl Render`：`top_bar`（三区）+ `tab_strip`（`.nav` 药丸）+ `content`（路由）

### 页面层（全部按 `docs/screens/` 的规格重写）

| 文件 | 重建内容 |
|---|---|
| `performance.rs` | 7 个分区 + `.stage-circle-{1..5}` 五色 + 轮询率按钮组 |
| `customize.rs` | 4 个渲染件 + Hypershift 改成下拉 |
| `lighting.rs` | `zone_card`（效果决定是否显示主色/速度）+ `dim_on_battery_card` |
| `dashboard.rs` | `box_group`（按 §2.2 的 CSS 逐条）+ `device_card` + `demo_banner` |
| `macros.rs` | `macro_card`（事件下拉 + 延迟滑块） |
| `engines.rs` | `engine_row`（标注「会阻塞」的引擎） |
| `widgets.rs` | 共享的 `not_wired_hint()` |

### 后端（方案 2）

- `src/backend/dll.rs` —— `EnginePaths::discover()`（找两处安装位置）、
  `resolve()`（精确名优先、兼容 `_v` 版本后缀）、`EngineLibrary`
  （`load` / `discover` / `path` / `func`）
- `src/backend/mod.rs` —— `probe_engine()` + `ProbeStatus`（区分「没装」/
  「加载失败」/「导出不全」三种情况）
- `src/backend/lighting.rs` —— `handshake()`：注册 → 恢复 → 注销，
  **不下发任何灯效**，字段名逐字取自实测 JS 的 `device_handle` / `device_identifier`

---

## 剩余工作（非阻塞）

### 1. 19 个 warning

全是重建留下的边角料：7 个未用常量、6 个未用导入、2 个未用函数、1 个未用变量。
**不含任何逻辑问题。**

### 2. 一处样式缺口

`content()` 的滚动：真实 `.body-wrapper` 是可以滚动的，但 gpui 的
`overflow_y_scroll` / `overflow_y_scrollbar` 都没能在 `gpui_kit::Div` 上解析到
（两个 trait 的路径都试过）。**已去掉该调用以免卡住编译**，其余布局完整。

### 3. 未接通的写入路径

界面上已用 `not_wired_hint()` 显式说明：改动只写本地配置，不下发设备。
接通点是 `LightingDriver::startup` / `configure` / `shutdown`（都已实现）。

### 4. 缩略项

- 提示框 `.tip` 仍是库默认（真实：方角、`#000` 底、`padding:8px 10px`）
- `.switch.disabled` / `.widget.disabled` 的 `opacity:.3`
- `.dots3` 标签溢出菜单
- `.widget { transition:height .2s }`

---

## 备份

`.ref/backup/` 下每次改动前都有快照；**`src-GREEN-*` 是编译通过的那份**。

## 纪律（血泪教训）

1. **改源码前先 `git commit`；至少要留快照。** 本仓库至今没有任何提交。
   第 5 轮的损坏不可回滚，就是因为没有它。
2. **不用 PowerShell 的 `-replace` / 反引号内联脚本改源码。**
   本轮又踩了一次内联 `node -e` 的反引号问题。**一律写成 `.ref/tools/*.js` 再 `node`。**
3. **`.rs` 是 CRLF**：按行处理的脚本必须 CRLF 感知。
4. **`Tee-Object` 写的是 UTF-16**：读日志前先判 BOM，否则正则静默失效。
5. **先让编译器说话**：语法错会让 rustc 提前停止，错误数毫无意义。
6. **改大文件用具名锚点**，不要靠行号或模糊匹配。
7. **编译器给的「相似名字」建议要验证**：`overflow_y_scrollbar` 的建议是错的，
   它属于另一个没引入的 trait。
