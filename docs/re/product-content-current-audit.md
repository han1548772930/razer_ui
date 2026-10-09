# 已有产品页内容核验（2026-10-05，未完成）

当前页面内容审查，不把 tab、图标、导航注册、JSON 描述符覆盖率当作页面一致性证据。只读取当前 `.ref/devices/<pid>/` 中 manifest 声明的 JS/CSS；没有启动应用、构建、测试或执行供应商 JS/DLL。

可复查收据：`product-content-current-evidence.json`。维护工具：`tools/audit-product-content-current.cjs --check`，需要现有 `.work/audit-js/node_modules` 提供 Acorn。它分别报告公共外层、Nommo 已复核组件和相机外层；没有“所有页面已一致”的判定。

## 公共外层的范围

下表是现有六个 family 数据表的清点。页面条目包括描述符里的 HELP、补充页、空页和被其他适配器接管的页；数量不是已实现或已核验页面数。

| Renderer | 产品数据条目 | 页面条目 | 本轮核验范围 |
| --- | ---: | ---: | --- |
| mouse_products | 76 | 384 | 公共 body CSS、当前共享 body 的 AST props |
| keyboard_products | 71 | 255 | 70 项公共 body；691 的 lazy CSS 未核 |
| audio_products | 76 | 309 | 公共 body；1303/1304 灯光左列另行核验 |
| gamepad_products | 9 | 39 | 公共 body |
| accessory_system_products | 6 | 10 | 公共 body |
| system_products | 41 | 199 | 公共 body |

278 项当前 main.css 的相应声明逐项一致：`body,html` 为 Roboto、16px、`#ccc` 文本、`#222` 背景；`.body-wrapper` 为 `padding:10px 20px 20px;min-width:600px;width:100%`；widget 为 14px。每项收据保留自己的 manifest、CSS 文件与哈希、规则位置、当前 JS body props。共享 body 定义的存在不证明每个条件分支和所有祖先级联已经复核。

修复前 mouse/keyboard/gamepad/audio/accessory renderer 的通用 `p_5` 将顶部做成 20px，system 缺少这层左右 20px。audio 还在正文前插入了产品名，gamepad/system/accessory 插入了本地实现说明。新增 `features/product_surface.rs` 按上面的已核验合同提供外层，去掉这些额外内容。产品名仍由已有宿主导航表达；没有改变设备写入能力或伪造服务确认。

边界：691 当前 manifest 没有 main.css，保留原 renderer 待分析 lazy cascade。182 虽出现在 mouse 数据表，实际由 `DeviceWorkspace` 接管，本轮 family body 修复不能算修复了 182。653、777、鼠标垫等原有适配器亦不计入本轮逐页内容核验。

## Nommo 1303 / 1304 的灯光左列

本轮从两个当前完整 bundle 重新解析 AST，定位并读取：

- 1303 `wm → tP/_P、Ym/Wm`；1304 `kM → op/ip、rp/sp`：左列按亮度、关闭灯光排列，右列为灯光效果。
- `_P/ip`：标题旁开关、帮助、0–100 亮度滑条；设置 0 自动关闭亮度，非零开启。`Yl/AR` 在拖动中维护自己的显示值，松手提交；关闭状态的 brightness 滑条仍允许拖动。
- `Wm/sp.render` 只挂载 `checkDisplay`，没有空闲选项或空闲滑条。`isIdleEnabled/idleMinutes` 虽存在于事件 payload 和配置，不能据此画出控件。

修复内容：

1. 用源形状的标题旁开关与帮助按钮替代普通 checkbox，移除重复亮度标题。
2. 实现 64px 滑条容器、6px 轨道、16px 圆柄、`8 + p × (width − 16)` 填充公式、底部 42px 的 12px/14px 数值气泡、底部 −2px 的 14px `#ccc` 两端刻度；亮度关态透明度 `.3`，变化 300ms。
3. 滑条草稿随拖动更新，松手才提交亮度对象；非零重新开启、零关闭；标题开关不改保存的数值。
4. 删除错误挂载的空闲选项；显示器关闭选项随亮度开关禁用。原配置里的 idle 字段保留。
5. 同步修复 `prepare-audio-products.cjs`，重新生成描述符与 coverage，确认再生成没有其他产品数据变化。

仍未完成：右列 quick/advanced effects 的完整布局、效果专用编辑器和服务状态分支；nanoLeaf/adjustment 服务状态传入；滑柄 hover 背景的 300ms 动画与精确原生命中区域。本轮没有把这些声明为一致。

## 相机 3592 / 3594 / 3595 / 3596 的外层

逐款读取当前 main root 的 `extraClass:"advanced-camera-container"` 分支及 children，确认先 `renderView()` 再挂载 video sibling；逐款 CSS 一致：横向 flex、padding 0，400px 设置列、`padding:27px 20px`、`#111`；视频容器 `flex:1 1`。

本地原先额外使用 20px 外边距和 440px 最小宽度，并只在 CAMERA tab 把 202px 黑色“预览不可用”说明框插入设置列。现在恢复独立横向区域，设置列自行垂直滚动；视频区域随主内容展开，保留无视频帧的黑色表面，不制造已连接、故障、重连成功或正在加载状态。四款现有 IMAGE/CAMERA/PROCESSING/MIC 路由使用同一外层。

仍未完成：每个 tab 的控件顺序、全部标题/副标题级联、native select/slider 与源组件的样式和行为、折叠/预览开关、tutorial、视频 transport 和错误分支。3587/3589/3590 legacy-camera 本轮未改。

## 后续必须逐页核验的内容

- Mouse：Customize 的产品图、输入热点/抽屉及映射类别；Performance 的 DPI/stages、轮询条件分支；Power、Calibration、Scrolling、Advanced 的控件与说明。不能从同 family 推定特殊产品一致。
- Keyboard：Customize/Gaming、Lighting/Effects、Power、Actuation、Calibration/OLED/Pairing 的挂载链、正文级联、控件类型、disabled/hover/focus、弹层与保存行为；691 lazy root 优先单独核验。
- Audio：除上面两个左列外，309 个描述符页面条目仍不能视为已核。现有通用 renderer 使用“一 section 一列”和 `gap_5`，源码可能把多个 section 叠在同列；许多 toggle、select、EQ、mixer、haptic、OLED 仍是通用形状。需按当前根和真实组件逐页替换。
- Gamepad：自定义、扳机、摇杆、灯光、电源、校准的专有布局、手柄图、范围控件、条件分支和硬件观察态。
- Accessory/System：显示器、Hanbo/PWM/Cooling/Core X 的内部布局；笔记本性能/风扇/电池/显示/灯光/音频/键盘内部布局；SourceControls 的 accessory 补充内容不在六 family 的公共外层修复内。
- 原有 `DeviceWorkspace` 适配器、专用 Hue/ARGB/Strip/Dock 等页面仍需各自审计；其他代理负责的 receiver/host tab 不由本收据声明完成。
- 字体：本轮明确字体家族、weight、字号和有显式 CSS 的行高；CSS `normal` 与 GPUI 默认 `phi()` 行高不同，不能把任意 1.2 或 17px 宣称为浏览器 normal 的精确对应。字形度量、fallback、栅格化和实际换行尚未通过运行画面验证（任务禁止运行）。

格式化与静态收据验证通过。最终 `cargo check --locked --all-targets` 由父线程汇总执行；本子任务未启动应用或测试。
