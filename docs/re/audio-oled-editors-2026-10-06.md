# 1383 OLED 编辑器续接

2026-10-06。范围为 Kraken V4 Pro 当前 manifest 的 OLED Home Screen Display，
不使用其他产品的 UI、旧前端或旧宿主作为实现依据。源码解析工具只处理 AST/CSS 数据；
不执行参考脚本。入口安装门控沿用 [本地入口记录](local-device-entrypoints-2026-10-06.md)。

## 接入范围

原七卡中的六个可编辑模式现在都从 EDIT 直接打开本地编辑器；Headset Info 按源没有 EDIT。
所有编辑器使用独立草稿，取消/关闭不会修改 profile；Apply 才写入对应 `oledHome` 分支。
切换 profile 时先恢复源初值，再恢复受控保存值并归一化。

| 分支 | 本地行为 | 当前源码证据 |
| --- | --- | --- |
| Animation / Image | 固定6/10个槽、选择、启停且至少保留一个启用槽；关闭选中槽后选择首个启用槽；本地文件选择、裁剪、移动、缩放、局部恢复默认 | [Artwork审计](audio-oled-artwork-current-audit.md)及 `audio-oled-artwork-current-evidence.json` |
| Emote | 104个源表情、去空白搜索、选中与不匹配占位分支 | 同上 |
| Banner | 8张原图、64种字体、14种字号、粗体/斜体/下划线、图像启停与位置、四种滚动方向、暂停/重播、Reset | [Banner审计](audio-oled-banner-native.md)及 `audio-oled-banner-current-evidence.json` |
| System Info | 三张固定幻灯片、每张左右两槽、九个标签、拖到空槽/菜单添加/删除、3/5/10秒、温度/日期/时间设置；六槽全空时Apply禁用 | [System审计](audio-oled-system-native.md)及 `audio-oled-system-source.json` |
| Media | 原有两个区域开关、位置单选、三种可视化、Reset/Cancel/Apply，迁入同源共享弹层 | [Home审计](audio-oled-home-native.md) |

System Reset 只复位幻灯片、当前页、间隔和温度单位，保留日期/时间格式；这是当前 `_v`
实际回调。格式设置暂存时不替换源样例数值。Home 跳过全空幻灯片并按间隔轮播。
System 的空槽及 Banner 的字体样式会使用 null，通用 `merge_known` 无法恢复这类状态，
因此只对这两个分支执行保存对象恢复，再交由各自规范化函数限制字段/值。

Banner 的横向周期为 `3400 + 28 × scrollWidth` 毫秒，纵向为5000毫秒，包含原不连续
位移/透明度关键帧。主页与编辑器各有自己的测量生命周期：编辑器打开时首测，之后文字或
方向变化才重测；仅改字体样式不重测。Apply 时若主页文字/方向不变，保留主页原周期。
恢复 profile 重建本地预览时按当前字体测量。对当前官方主机包进行静态归档提取，
恢复 Electron41.2.0 / Chromium146.0.7680.179 版本字符串，并读取对应版本的上游UA CSS，
见 [浏览器来源收据](current-browser-ua-evidence.json)。textarea 的默认padding为2px，
当前产品只有div的全局border-box规则，Banner textarea仍是content-box；385×98内容加边框/
内边距得到391×104外框。该尺寸由提取数据驱动。normal行距与最终原生字体度量仍需运行时
像素验收；上游CSS不被宣称为已逐字节验证的宿主编译样式。此次未运行窗口。

## 共享弹层与提示

`audio_oled_dialog.rs` 从1383的 `Mv → Tn` 提取公共表现。动画/图片/表情最大宽800px，
其余编辑器最大850px；保留100px top加110px margin、36px标题、35px底部按钮位置。
标题下方是源码1px阴影；背景100ms linear、top300ms ease、关闭背景200ms ease。
Base Dialog 负责焦点捕获/恢复和键盘模态范围；关闭按源直接卸载。

`audio_oled_tooltip.rs` 对“需要 Synapse”使用窗口 portal，保留300px main、15px目标、
源定位偏移、1060层级及100ms淡入。`58837:h` 先处理右边界、再处理左边界，各保留8px；
不添加纵向翻转。几何适配直接使用当帧 prepaint 的目标位置，避免缓存上一帧坐标。
收据和只读校验由 `tools/audit-audio-oled-dialog.cjs` 维护。

动画/图片的替换与恢复按钮提示也复用当前58837 portal。它们以28×27px按钮为目标、
源margin-left为270px，正常位置为按钮left−22px/bottom+5px；没有需要Synapse图标的−3px偏移。
系统信息的添加菜单则按 `hv` 留在幻灯片内top61、left0/right0，宽126/129px；保留祖先滚动
裁剪，不应用portal的视口吸附。绘制和指针命中都使用同一裁剪矩形。

## 资源与完成边界

四套独立嵌入表由同一 AssetSource 的 load/list 注册：Home27、Artwork111、Banner19、
System19，共176项。统一资源校验检查产品 manifest、原始源码切片、source/output哈希、
内嵌位图字节、PNG尺寸、WebP帧时长/循环及嵌入键唯一性；各准备工具另比较实际解码像素。
静态WebP与动画WebP分别解析，不把单帧图当作丢失动画元数据。

本地文件裁剪保留原数据和裁剪几何，不伪造原 worker 的编码结果、帧数、上传进度或设备
确认。BLE/dongle条件、加载/错误/重试、语言下载、原worker编码、遥测和设备服务写入仍有缺口。
这些编辑器不代表所有产品或整个 OLED 工作流已经完成。

只进行格式化、`cargo check --locked --all-targets`、静态源码解析及资源校验。
没有运行应用、构建、测试、安装器、参考 JavaScript 或 DLL，没有运行时视觉验收结论。

## 本批最终静态结果

- `cargo check --locked --all-targets` 通过，仅保留原有 `layer_button`、`select_profile`、
  `set_monitor_runtime` 三项未使用方法警告；之后只对父模块进行了Rustfmt格式化。
- `cargo fmt --all -- --check` 通过；`git diff --check` 通过。
- 四类OLED源码生成/审计、共享弹层与提示审计、当前浏览器UA审计均通过只读 `--check`。
- 四套资源准备工具的只读验证通过，包括实际解码像素、帧时长和循环；统一资源校验确认176项
  OLED资源及1257项共享资源、35项服务SVG。
- 45份嵌入JSON静态校验0失败、4项原有结构推断跳过；Artwork绑定和原生产品描述符校验通过。
- 旧1383五控件审计已更新本地文件指纹和已完成编辑器范围；共享滑条只读审计仍通过。
