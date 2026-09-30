# 顶栏高度与字阶：实测结论

本文件记录本轮**已核实**的真实规格，与实现对照。核实方法见每节的复现命令，
数据全部来自 `.ref/` 下雷云自己的 CSS，不是推测。

---

## 1. 顶栏高度 —— 之前是错的

```css
div.nav-tabs {
  align-items:center; min-height:48px; position:relative; width:100%; z-index:106;
  background-color:#222; border-bottom:2px solid #000; color:#5d5d5d
}
```

| 项 | 真实值 | 修复前 | 修复后 |
|---|---|---|---|
| 高度 | `min-height:48px` | **34px** | `min_h(48px)` |
| 下边框 | `2px solid #000` | `2px` + `theme.group_box`（**#111**） | `NAV_TABS_BORDER = #000` |
| 底色 | `#222` | `theme.background`（#222）✅ | 不变 |

**34px 的来源**：`gpui_kit::component::TitleBar` 内部写死了

```rust
pub const TITLE_BAR_HEIGHT: Pixels = px(34.);        // title_bar.rs:15
...
.h(TITLE_BAR_HEIGHT)                                  // title_bar.rs:335
.refine_style(&self.style)                            // title_bar.rs:343
```

`refine_style` 排在第 335 行**之后**，所以外部传入的高度能覆盖它——但**不传**时
根元素就是 34px，而里面的内容按 `min-height:48px` 排，于是内容溢出。
这正是「顶栏高度不对」的原因。

> 之前 `app.rs` 只写了 `TitleBar::new().child(...)`，漏了这一条。

---

## 2. 字阶 —— 雷云是固定档位，不是比例派生

`.ref/tools/font-size-audit.js` 统计 `.ref/` 下全部 CSS：

```powershell
node .ref/tools/font-size-audit.js 10
```

**1821 条 `font-size` 声明，去重后 28 个取值**，主档位：

| 字号 | 条数 | 代表选择器 |
|---|---|---|
| **14px** | 1124 | `.desc-text`、`.body-text`、`.thx-btn` 默认（主字阶） |
| **12px** | 371 | `.thx-btn.sm` / `.fit` / `.inline`、`.navs-wrapper`、`.slider-container .title-more` |
| **16px** | 121 | **`body,html`**、**`.widget .titleRow .title`**、`.alert-title` |
| 18px | 25 | `.main-setting .widget .title` |
| 20px | 27 | `.main-title`、`.snap-tap-add-button` |
| 10px | 42 | `.badge`、`.header-unsaved>.box>.badge` |
| 13px | 23 | 次级说明 |

### 2.1 之前错在哪

| 项 | 真实值 | 修复前 | 说明 |
|---|---|---|---|
| `body,html` 基字号 | **16px** | **14px** | `main.rs` 里注释写的是 16px、代码写的 14px，**注释和代码自相矛盾** |
| `.widget` 正文 | 14px | 无（继承基字号） | 基字号改 16px 后必须显式压回 14px |
| 卡片标题 `.widget .titleRow .title` | **16px** | 14px（继承 `.widget`） | 92 处 `div().font_bold()` |
| `.thx-btn.sm` | **12px** | 由库派生 | 见下 |

### 2.2 为什么必须显式指定，而不能靠主题

gpui-kit 的字号是从 `theme.font_size` **按比例派生**的：

```rust
fn button_text_size(self, size: Size) -> Self {
    match size {
        Size::XSmall => self.text_xs(),
        Size::Small => self.text_sm(),     // sizing.rs:330
        _ => self.text_base(),
    }
}
// Size::Size(v) => element.text_size(v * 0.875)      sizing.rs:239
// _ => rems(0.875)                                   accordion.rs:284
```

要同时得到「16px 正文 + 12px 小按钮」，需要 `font_size ≈ 13.71px`，而正文又必须是
16px —— **两者不可能同时满足**。所以基字号取真实的 16px，
需要固定档位处在调用点显式指定（`widgets::geometry::TEXT_*`）。

### 2.3 落地的常量

`src/pages/widgets.rs::geometry`：

```rust
pub const TEXT_10: f32 = 10.0;
pub const TEXT_12: f32 = 12.0;   // .thx-btn.sm / .navs-wrapper / slider 数值文字
pub const TEXT_13: f32 = 13.0;
pub const TEXT_14: f32 = 14.0;   // .widget 正文（1124 条，主字阶）
pub const TEXT_16: f32 = 16.0;   // body,html / .widget .titleRow .title
pub const TEXT_18: f32 = 18.0;   // .main-setting .widget .title
pub const TEXT_20: f32 = 20.0;   // .main-title
```

组件侧：`card()` 设 `TEXT_14`、`btn()` 设 `TEXT_12`、新增 `card_title()` 设 `TEXT_16`。

---

## 3. `.nav` 标签胶囊 —— 逐条核对后是对的

```css
.nav-tabs .nav {
  background-repeat:no-repeat; border-radius:14px; color:#999; line-height:14px;
  margin-right:20px; padding:7px 10px; text-align:center; text-transform:uppercase;
  transition:background-color .3s,color,.1s; white-space:nowrap
}
.nav-tabs .nav:hover          { background-color:#2d2d2d; color:#ccc }
.nav-tabs .nav:active         { background-color:#3cbf27; color:#111 }
.nav.active                   { background-color:#44d62c; color:#111 }
.nav-tabs .nav.active:hover   { background-color:#44d62c; color:#111 }
.nav.disabled                 { opacity:.3; pointer-events:none }
```

| 属性 | 真实值 | 实现 | 状态 |
|---|---|---|---|
| 圆角 | `14px` | `rounded(14px)` | ✅ |
| 内距 | `7px 10px` | `px(10).py(7)` | ✅ |
| 行高 | `14px` | `line_height(14px)` | ✅ |
| 常态字色 | `#999` | `muted_foreground` | ✅ |
| hover | `#2d2d2d` / `#ccc` | `secondary_hover` / `foreground` | ✅ |
| active 底 | `#3cbf27` | `NAV_ACTIVE_BG` | ✅ |
| 选中底 | `#44d62c` / `#111` | `primary` / `primary_foreground` | ✅ |
| 选中 hover | **不变** | hover 只作用于未选中项 | ✅ |
| `margin-right` | `20px`（末项 0） | `mr(20px)` + last 例外 | ✅ |
| 字号 | 继承 `.navs-wrapper` 的 `12px` | `text_size(12px)` | ✅ |

`.nav` 上**没有** `font-size`，字号来自父级 `.navs-wrapper { font-size:12px }`。

---

## 4. 复现命令

```powershell
# 顶栏与 .nav 规则
node .ref/tools/audit-css.js ".ref/frontend/static/css/55.a5b041a2.chunk.css" nav-tabs
node .ref/tools/grep-css.js ".ref/frontend/static/css/55.a5b041a2.chunk.css" "nav:hover" 8

# .nav 的基础几何（在各设备模块的 main.css 里）
node .ref/tools/find-decl.js "border-radius:14px" 12

# 全站字阶
node .ref/tools/font-size-audit.js 10

# TitleBar 的写死高度
#   gpui-component-0.7.0/src/title_bar.rs:15  TITLE_BAR_HEIGHT = px(34.)
#   gpui-component-0.7.0/src/sizing.rs:330    Size::Small => text_sm()
```
