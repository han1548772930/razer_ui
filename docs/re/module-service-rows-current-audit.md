# 当前设备与模块行：进度、详情图片与固件外链

2026-10-05。静态追踪当前 Dashboard `44442/w/L`、`75551/o` 和 `55/6505` CSS；未执行下载 JavaScript。

进度条改为源 `#2c5824` 轨道、`#44d62c` 指示条，200×8px、轨道4px圆角且裁切、指示条15px圆角。独立 RenderOnce 保存展示状态：初始从0开始，300ms线性到观测目标；下降时先到100（上次目标小于100时）、1ms回零、再300ms到新目标。仅插值宽度，安装阶段、服务快照和无障碍百分比均保持实际观测。减少动态效果设置直接显示目标。

`w` 的图片来自 `detail.srcImage`，不能从相同 PID 推断为 Dashboard 缩略图。删除此前未经证明的替代，保留288×162px区域；原 `restartRequired` 条件文字已补。没有发起图片请求，所以不假设出现 source onError，也不显示错误占位图。

固件详情复用现有 `external-link.svg`：其 SHA-256 与当前 `icon_external_link.48227e72.svg` 完全相同，20px、right -25px、top -2px。仅固件 warning severity 把品类图标变橙色；普通新设备/已安装设备保持普通图标。固件名称按 `L` 优先读取 `productName`，再读取 `title`。

新快照打断正在执行的展示序列时，以最新观测为目标；不复制浏览器多个 transitionend 回调竞态。1ms重置按时钟采样，未保证占据一个屏幕帧。图片、字体、实际布局和动画尚未运行验证。

`node tools/audit-module-service-rows.cjs --check` 核对完整源码片段、CSS、外链字节哈希和原生展示接入。未运行应用、构建或测试；cargo check 由主任务统一执行。详见 [module-service-rows-current-evidence.json](module-service-rows-current-evidence.json)。
