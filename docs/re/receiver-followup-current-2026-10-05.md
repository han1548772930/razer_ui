# 179 接收器与服务产品模式图标继续复核

2026-10-05。本轮继续当前源码的一致性任务，补查上一轮未追到的 Widget 包装层、配对加载图与产品模式根。只使用当前产品 manifest 声明的资源及静态 AST；没有运行应用、构建、测试、下载的 JavaScript 或 DLL。

## 179 的实际修复

179 的 Customize 仍由 `9473/mE → Te + OE` 挂载，左侧 HyperPolling 的右 padding 为 **30px**，不是默认 Widget 的 40px；已有代码这一项正确。本轮没有重做此前的配对入口、三种指示模式或无 ProfileBar 的头部。

继续追入 `g.A → 7693/l` 后，发现帮助提示不能套用普通 CSS 兄弟 `.tip` 的淡入。实际组件以 `showTip` 条件创建 `document.body` portal，并在 `positionTip` 中直接写入 opacity 1。CSS `.body-widget-tip-portal` 还覆盖为 `position:fixed; visibility:visible; z-index:10001`。新增的 `ReceiverWidgetPortal` 仅由 179 调用：

- 悬停立即显示、离开立即隐藏；帮助圆点自己的背景仍按源保留 300ms 过渡。
- 提示文字实际绘制恢复 14px / 18px 行高、300px 最大内容宽度，而非共用分支的 16px 行高。
- 默认位置为 Widget 右边缘减 14px、顶部加 34px；左右越界按源移动，底部越界改放帮助按钮旁边，必要时移到其左侧或向上挪 10px。
- 独立使用 10001 层级，不把当前 179 的规则扩展到尚未核验的其他产品。

配对 modal 补回 `.modal_modal__UO3fZ` 的纵向滚动/内容裁剪以及标题栏两个 5px 上圆角。加载图改用圆弧绘制并补齐前景 `stroke-linecap="square"` 的半线宽端部；继续保留源 2 秒旋转/弧长关键帧和 20px CSS 显示大小。

本轮通过维护工具取得 179 自己的 `spinner.svg`、`icon_close.svg`、`tooltip_questionmark.svg`，与已使用的三个共享资源逐字节相同；没有新增替代图。机器证据增加了 Widget/Spinner/Radio/Close 模块、两条 portal CSS、原始 SVG 和共享资源匹配，见 [receiver-current-evidence.json](receiver-current-evidence.json)。

## 服务产品的运行时 favicon

[product-mode-tab-icons-current-evidence.json](product-mode-tab-icons-current-evidence.json) 独立覆盖 164、179、241、653、740、746、769、777、778、784、3871、3884、3886、3946 的 manifest JS 与字面 DOM favicon 选择器。

164 的 `_D` 配对类与 Dashboard 包里未挂载的同类不同：其 `connect` 返回值 `tD` 被 `mL.render` 在 URL `displayMode=multiDevicePairing` 分支真实挂载。这时 favicon 变成 `static/media/hyperpolling_icon.b7c3d035.svg`；普通 164 产品根仍是 HTML 的 ACCESSORY。179 没有该字面选择器，241 的 hook 只把已有图片字节转成 data URL。不能把 164 的配对模式图标误换到普通产品页签，也不能用 Dashboard 的“未挂载”结论代替产品自己的挂载核验。

本地普通产品页签的 HTML 图标映射保持现状。现有独立配对窗口没有 TabUI 图标槽，其独立 OS 窗口策略仍是已记录的本地例外；本轮没有创建一个虚构的产品模式页签来消费该图标。

## 静态验证和剩余边界

专项重查命令为 `node tools/audit-receiver-current.cjs --check`、`node tools/audit-product-mode-tab-icons.cjs --check`，并对改动 Rust 文件执行 rustfmt。Cargo 检查由主任务合并其他子任务后统一完成。

未验证实际窗口栅格化、DPI、字体、滚轮和悬停命中，不能将静态几何核对称为像素验收。配对界面继续等待真实 `DUALLINK_BIND_INFO`；本轮没有编造设备、扫描结果、成功响应或固件状态。产品 favicon 收据仅覆盖列出的 14 个服务产品及字面选择器，不代表全部 331 产品的所有运行模式均已验收。
