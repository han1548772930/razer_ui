# 当前 164 / 241 Lighting 亮度页面与提交

[页面证据](receiver-brightness-page-current-evidence.json)记录当前前端 lexical AST、middleware task 和带条件的 CSS 收据；[审计工具](../../tools/audit-receiver-brightness-page-current.cjs)只把原 JS/CSS 当作数据解析。

原 `vr / Mn` 将 brightness 和 switch-off 放在左列，effects 放在右列。已修正通用控件把三块竖排、重复标题和内容行 switch 的布局。专属页面使用原 600px 列宽、widget 的 `30px 40px` padding / `10px` 外边距、RazerF5 16px 绿色标题、Roboto 内容，switch 紧随标题，help 位于右上。亮度滑条恢复 64px 容器、6px 轨道、8px inset、绿色提示、0/100 标签和原关闭时 0.3 opacity；关灯时依然可以拖动。idle 保留原两个复选框、1–15 滑条及 brightness / idle 禁用依赖。

原共享 slider 在鼠标拖动时只改自身 state，mouseup 才调用 changeValue；brightness 的 changeValue 同步 isEnabled，零值关闭。当前原生事件已按 Change 预览、Release 提交，开关保存记忆亮度并立即提交。

`taskMakerSetBrightness` 的实际链为 serial metadata active profile → 53 位 version → 修改该 profile brightness → `mZ` 保存 → NORMAL_SKIPPABLE setting task。原生消费者以真实串号查询和原默认文档 producer / `b4` 缓存处理保存本地源文档，随后调用共享 `ReceiverBrightnessWrite`。本地存储与硬件结果分别保存，见 [协议证据](receiver-brightness-current.md)。页面读取消费者也调用真实 `ReceiverBrightnessRead`；设备观察不会伪造为本地配置。

请求槽在页面或 profile 失效时保留到真实工作线程结束，最新 brightness 仅排队一次。旧读取的 edit revision 不能抹除新拖动提交。通信失败保留本地已保存文档与真实部分 response；退出会取消并 join 工作线程。

右侧 Quick / Advanced tabs、每种 effect 参数与同步、effects / mapping engine 提交尚未完整。display / idle 当前仍是本地草稿；其原 service lifecycle 注册、source 文档写回与 host/memory/cache 链仍需实现。旧 schema 和 linked / override 存储分支也未完成。此次修正不能视为整页或整个产品完成。

静态 CSS 收据与 cargo check 不等于像素或设备运行时验收。没有执行应用、构建、测试、厂商 JS 或 DLL。
