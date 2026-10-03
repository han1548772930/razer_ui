# 无线接收器左上角控件复核

核验日期：2026-10-03。用户指出无线接收器页签左上角下拉框不符合原版。

实际产品为 179 / HyperPolling Wireless Dongle。当前 `main.4849f7ca.js` 的普通根在 UTF-16 偏移 4572383 明确传入 `renderProfileBar:!1`；Header 在偏移 4410076 使用 `this.props.renderProfileBar&&this.props.renderProfileBar()` 条件挂载。因此该页应保留头部布局区域，但不显示配置文件下拉框。不能因为 bundle 内还包含宏模式使用的 ProfileBar 类，就把它挂到普通设备页面。

`SourceProductWorkspace` 之前给所有源产品自动放置 180px Kit Select，是错误添加的内容。本轮按真实根条件隐藏 179 的配置文件控件。没有根据已有控件猜测一个新的下拉设计，也没有删除其他产品确实挂载的 ProfileBar。

原始输入哈希、Acorn 节点偏移及代码片段见 [可复核证据](receiver-profile-header-current-evidence.json)。重查：`node tools/audit-receiver-header.cjs --check`。本记录只证明 179 的这一处挂载条件，不代表所有产品头部、下拉菜单或页面已完成像素验收；没有运行应用或测试。
