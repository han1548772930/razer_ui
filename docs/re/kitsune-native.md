# 4115 Kitsune Customize

更新：2026-10-06，状态 **partial**。

## 当前来源

主包 `.ref/devices/4115/static/js/main.1a52fc92.js`，SHA-256 `03ad7f2d6eafc52a1766a7b850c0d03912344a5bfcd00695b62c5baff76254aa`。实际挂载为 Customize → MP → hP；hP 挂 pP/mP 设备图、SP/OP 轮询率、PP Mode Switcher、CP SOCD，并给 CP 传 `isArcadeController:true`。

[当前证据](kitsune-current-evidence.json)包含 42 项 AST 收据、8 份 manifest 声明的 CSS、两个 SVG 的原始字节和动态请求链。`tools/prepare-kitsune.cjs --check` 静态复核来源并验证生成数据、嵌入登记和资源，不执行厂商脚本。

## 当前实现

- `GamepadProductWorkspace` 对 4115 使用专门的 `kitsune.rs`，不改变其他手柄布局。
- 顶部 340px 预览区、770px 内框、原始 SVG 按 251px 高显示，复用已存在的 22px 点阵与径向淡出背景。
- 当前动态上下文 58388 只有 `./4115_0/svg_prods/0.svg`，模块 19105 指向 `0.ff39cdc0.svg`。源 pP 对缺少 edition 资源回退 edition 0，保留原 layout；本地使用传入的 `device.layout_id`，仅 layout 0 显示这张图，不把其他 layout 假定为 0。
- 左列先显示轮询率，再显示模式切换；右列显示 SOCD。轮询率使用源 OP/oP 的 72×27 按钮及纯数值标签，保留 250/1000 的本地值。
- 模式 safe/standard 与五个 SOCD 值使用原语言导出及各自说明。4115 标题为 `SOCD_SETTINGS`，不是通用 `DPAD_SOCD_SETTINGS`；右侧采用原 `kitsune_dpad_socd.svg`，228×271。
- Base Radio 管理选择、焦点和语义行为；应用按原 CSS 绘制 20px 外圆、10px 内圆及 200ms Ease 缩放/透明度过渡。
- 继续通过原 `write()` → GamepadProductChanged → SourceProductWorkspace.capture 修改本地草稿，没有加入 DLL 写入或伪造设备状态。

## 仍未完成

- 三个面板帮助图标与提示已接入：即时挂载/移除 portal，图标底色 300ms Ease；原始 questionmark SVG 与共享资源一致。仍需用真实 body-wrapper 矩形替换共享 helper 的 viewport 边界近似，以及核对自适应/行高/精确像素和容器过渡。
- 源 hP 的 `displayMode=macro` 仅显示顶部预览；目前专门核对的是普通 Customize 分支。独立复核确认：GAMEPAD 不在 Macro 类别默认白名单，也不命中 3907 特例；只有运行时 supportMacro=true 才可能通过筛选。当前 4115 主包与静态设备目录没有该能力证据；宿主仅转发运行时字段。不能仅凭 hP 分支就添加入口，外部运行时能力发布者仍待核对。
- 真实设备连接、固件、无线模式与运行时读取观察，及窗口变更时的设备元数据同步仍需追链。静态配置与本地草稿不代表读取成功。
- SVG 布局、动画、焦点与运行交互未验收；没有运行应用、测试、构建、下载的 JavaScript 或 DLL。

格式化和 `cargo check --locked --all-targets` 通过，保留 3 项既有 Rust 警告。资源校验覆盖新增的两个 SVG；嵌入 JSON 校验为 50 份、0 失败、4 项既有跳过。独立回读见 [报告 G](ui-readonly-first-roadmap.md)。
