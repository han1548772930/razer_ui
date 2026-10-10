# Philips Hue 当前界面契约

当前产品 769 源为 `.ref/devices/769/static/js/main.ad1113f8.js`，SHA-256 `3d1660626594af99c54c622b2e1caf3190e172e8703392a65286e81b9a3fff9d`；CSS 为 main.341edb83.css。实际组件、十语言、灯效、archetype 与资源见 [当前证据](hue-current-evidence.json)和 [资源 manifest](../../assets/synapse/hue-manifest.json)。Xo 未收到 renderProfileBar，Hue 不显示普通设备配置下拉。

[HueWorkspace](../../crates/razer-pages/src/features/hue.rs)由 SourceProductWorkspace 实际挂载；onboarding/bridge/brightness/effects 分别拥有对应内容。正式入口初始未配对，缺少 Hue 通信 adapter，扫描等操作不可用；预览为独立实体，样例网桥、灯具和娱乐区不进入实时设备列表或配置。

## 当前内容与状态规则

引导覆盖扫描、手动 IP、发现、等待 Push-Link、配对及失败/重试等待；已配对页为两个 600px 栏，按 1280px 条件换列，包含网桥、灯效、亮度和设备。网桥保留开关、娱乐区、刷新、删除、关闭/占用、接管与移除确认内容。没有观察时灯具计数为 0，不复制 reducer 种子 1/1/1。

手动 IP 从四个空值开始，按源 parseInt 去前导零/取数字前缀，上限 255；HTML pattern 不等于严格 IPv4 校验。空值 Backspace 退一段，三字符前进，首尾不越界。配对按钮只请求 WAIT_USER_CLICK_PAIR；Bi 无对应正文，不能自动制造 PAIRING。源失败页重试/从头开始按钮没有回调，保持该实际行为。

自动扫描入口已有 13 秒 UI 超时，到期仍处于 Scanning 才进入 ScanFailed；离开 Scanning 取消任务。手动 ScanningIp 不启动该计时器。扫描页本来没有进度条；Pairing 页才有 300px 轨道与 80px 绿条，2 秒从-80px 滑到 100%，close 在右侧 20px 范围，reduce_motion 静止。计时器只表示界面等待结束，不伪造网桥回应或成功。

亮度已使用共享 SourceSlider：64px 容器、数值气泡、OFF/BRIGHT 端点、0–100；禁用按源透明度与交互门控。isGlobalBrightness 缺失时显示全局选中，而第一次取反原字段写 true，第二次才切换逐灯。灯具按 containerId 去重，通道数按 name 去重，在线/计数来自 isOn。控件身份使用 containerId+区域 ID，NA 实际 JSX 未使用 handleSelectDevice，不能添加额外勾选框。

快速灯效顺序为环境感知 11、音频计 12、呼吸 2、光谱 3、静态 1；已挂屏幕区域、色彩增强、双色/随机颜色和单色控件，光谱无附加参数。当前 40 色与 no-color 次序已核对，但不宣称完整颜色编辑器视觉验收。

高级灯效按真实 chroma_installed 分支显示安装或启动内容。两入口均发 HueChromaRequested，经 WorkspaceEvent::OpenChroma 打开本地 Chroma 窗口；这仅是导航，不修改安装状态，不等于外部安装、设备灯效或同步成功。真实配置列表、安装/可用性观察和跨设备同步仍缺。

教程点使用源 36×36 动画资源，已配对、非 loading 且本地 isShowTutorialHue 值不为 false 时显示；点击隐藏并将 false 写入应用本地 JSON。源对应 localStorage，这不是 DLL 持久化。当前写入失败未向用户暴露，不能把本地写入尝试称为设备保存；持久化逻辑已存在，不再列为尚未实现。

## 草稿与剩余工作

snapshot/restore 只接纳 brightness、quickEffects、ports；网桥状态、IP、分组、灯具列表、安装状态、全局高级灯效开关和命令结果不进入 profile。预览操作不会保存样例观察。UI 编辑与本地 Apply/Save 仍需随正式状态链逐项完成；DLL 设备/服务写回均须在当前范围实现，真实网桥目标、请求、响应和刷新链仍需接通。

仍需 Hue 网络发现和服务往返、真实网桥/娱乐区/灯具状态 publisher、正式可编辑条件、完整 Chroma 状态及剩余步进器/提示/禁用视觉细节。动态语言、浮层、焦点、滚动和实际灯具输出未经运行验收。extract-hue.cjs --check 与 validate-hue.py 核对当前收据、文案、资源、SourceSlider、教程持久化、配对动画及 13 秒超时；本次均通过。没有运行应用、测试、下载 JavaScript 或 DLL。
