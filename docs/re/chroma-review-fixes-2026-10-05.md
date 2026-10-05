# Chroma Dashboard 核对后的第一批修正

来源是当前 `.ref/applications/chroma-app/dashboard/` 清单声明的代码。本文对应 [逐项核对 C01–C07](ui-source-review-2026-10-05.md)，不是整页完全一致的验收结论。完整 AST、CSS、独立模块目标和缺失媒体见 [本轮证据](chroma-review-fixes-2026-10-05-evidence.json)。

| 核对项 | 本次实现 | 尚未覆盖 |
| --- | --- | --- |
| C01 导航 | 高48、下边框2黑、胶囊28、字12/行高14、间隔20、圆角14；选中/hover/pressed颜色来自332 CSS；背景300ms、文字100ms CSS ease | 工具栏不是本批改动范围 |
| C02 预设 | 卡120×130、上20/左右10、图48、可见12px大写标签、间隔10；保留7个Quick Effects与末尾Chroma Apps | Advanced Effects依赖真实全局Chroma profile列表，当前未提供；不能由产品本地profile伪造 |
| C03 横幅 | 使用共同 `AppIntroductionBanner`，独立于卡片宽度上限；1220–2500/minH531、42bold/24/14、body max1020、原图和箭头、两个真实tour直接打开 | 共享组件自身证据见 `app-introduction-banner-current-evidence.json`（主任务维护） |
| C04 分组 | 保留子树，max-height与translation为300ms linear，箭头300ms linear；300ms后恢复overflow；预设组mt20/title17与独立help | help沿用当前项目WidgetTip定位；尚未单独复刻Ss的tip top24/left8；拖动重排未实现 |
| C05 预设状态 | 按设备实际effect及相关颜色/方向/random/duration匹配同步选中；激活底111/边5d5d5d；hover/pressed边色与底色；tooltip使用原文“可应用于”与本地可写设备名称，空列表显示原No Device | 原灰色和hover动画SVG缺失，未自行制作；tooltip当前仍是通用组件，未实现光标跟随300px提示框；无真实SDK状态时不伪造spinner/Chroma Apps启动/3000ms警告；backdrop blur30未实现 |
| C06 目录与设备 | Modules不再是恒定空div：按65596/P5四项目录绘制1220宽、80高、40图标、16字、源中文名称特例、27×90操作区域；设备可显示真实edition_name的12/14灰色副标题 | Modules目录不等价于服务available/recentModules；无服务记录则不显示日期、大小、说明、固件、维护/下载进度。四模块真正编辑器均未接入；第三方/Sensa/immersive分组和设备battery/effect色块仍缺 |
| C07 布局 | 设备名min17/max33允许多行；validDevices>4宽上限2460，其余1220；viewport<=1279时910并取消居中；分组高度依据真实卡数和源290×245/20间隔计算 | 此批没有实现源码分组重排、吸附和完整reflow动画 |

`ChromaPreset::matches` 只读取现有产品的真实灯光设置；不把一次点击当成同步成功。Source `62296/us` 的字段为 color1/color2/direction/isRandom/duration，Cs中的绿为 `#00ff00`，Starlight duration2、Wave direction2、Wheel direction1。当前只有能写入既有lighting设置的本地产品参与apply；生成的静态灯光页不会被假装写入。

## Studio 入口身份纠正

当前 `23322/Ks.openChromaStudioTab → 60336/A8` 是：

```json
{"windowName":"chroma-studio","url":"/synapse/chroma-studio/","openParam":"policy=3,tab_visible=1"}
```

确切位置是 `332.e7f2fd76.chunk.js @1075955–1076048`。它不是 `/chroma-app/dashboard/`。本次未把 Dashboard 作为 Studio 的直接打开目标；原 app_picker_host 的错误能力标记由主任务移除。ChromaPage只新增 `OpenTour(TourKind)` 事件；独立窗口向宿主转发同一个实际导览。

同一注册表的 Visualizer 指向 `/synapse/audio-visualizer/`，Sensa 指向 `/chroma-app/sensa-hd/`。当前Studio目录此前没有缓存，代码中也没有独立Studio实体。维护工具仅对源代码登记的Studio路由抓取，HTML/两个manifest均为network_error、JS/CSS为0，不能推测编辑器规模。

## 验证及资源边界

- `node tools/audit-chroma-app.cjs` 成功：31个挂载组件、73条既有CSS规则、89个locale key；补3个原版提示键到10种语言。
- `node tools/audit-chroma-review.cjs` 静态解析本轮当前源及验证既有Chroma资源的源/输出SHA-256，不执行下载脚本。
- 本子任务未运行cargo、应用、构建或测试。曾对修改的Rust文件运行rustfmt，随后遵从主任务统一格式化/检查安排。最终 `cargo check --locked --all-targets` 由主任务执行。
- 没有新增位图/SVG。常规下载工具请求14个gray/animated原SVG均network_error；没有提权重试。更完整的当前CSS还声明Advanced Effects gray资源，亦列入缺失媒体清单。
- `prepare-chroma-assets.py --offline --check` 在系统Python缺Pillow；已有resource-env的Pillow无法加载_imaging，故无法在该环境重新解码AVIF。既有20项原/输出文件哈希校验单独执行，不能把它称为重新生成验证。
