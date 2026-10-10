# Alexa 输入设备与本地状态链

依据为当前 `.ref/applications/synapse/alexa/static/js/main.05f102d2.js`。`tools/audit-alexa-content.cjs` 静态解析 Acorn 词法范围，保留每个绑定的字节位置、SHA-256 和原文；不执行下载的 JS。最新精确位置见 [证据](alexa-content-current-evidence.json) 的 `Ya`、`ta`、`le`、`ja`、`Fa`、`za`、`Ep`、`yE` 和 `Yp`。

## 原始链路

- `Ep` 应用初始化调用 `Ya.init()`。`Ya` 从 `alexaSettings.alexaAudioInputDevice` 取得已选择的对象，监听 `navigator.mediaDevices` 的 `devicechange`。
- `yE` 设置页挂载后订阅 `devicesChange`、`deviceChange`、`languageChange`，再调用 `Ya.getDevices()`。这条输入列表来自浏览器媒体 API，不是 Razer HID 或 DLL 库存。
- `_getDeviceList` 仅保留 `kind == "audioinput"`、`deviceId` 既不是 `default` 也不是 `communications`、`label` 非空的记录。保持返回顺序及重复项，未排序。先发布新列表；所选 ID 消失后调用 `setDevice(null)`。
- 菜单始终有本地化的 Default 项；触发器在未选择设备时使用原版字面量 `"Default"`。选择默认项提交 `null`，选择设备提交整个媒体记录。`setDevice` 比较记录，变化时先更新当前对象，再通过 `ta.update(le,{[ja]:e})` 合并持久化，然后发布 `deviceChange`。
- ID 仍存在时，原版保留已选择的旧对象。触发器显示 `P.label`，菜单显示最新记录的 `label`，两者可能暂时不同；不能把触发器旧标签写回菜单或宣称重新观察过设备。
- `Yp` 的 `synapseSkills` 开关初值从 `alexaSettings` 读取，缺省为 `true`。切换先更改可见状态，再直接 `ta.update(le,{synapseSkills:t})`，没有广播或设备提交。

## Rust 实现和适配边界

`alexa_page/input.rs` 实现源过滤、选择、消失清空以及已存记录和新枚举记录的分离。正式页与测试预览均不注入示例麦克风。`observe_media_inputs` 仅接收真正成功的媒体枚举响应；错误响应不得作为空列表传入，否则会错误清除选择。当前还没有调用此方法的跨平台媒体枚举适配器，不能把 `simple_service`、`RzAudioUtil` 的 endpoint ID 直接视为浏览器媒体 ID。

`alexa_page.rs` 的选择事件提交整个记录或 `null`，更新 `SelectState`；`sections.rs` 使用 `SynapseSelect.selected_text` 独立保留触发器显示文本，列表选择和焦点仍由框架负责。刷新记录后保留源 `P` 的对象语义；异步本地读取若晚到，也必须对已经观察的列表进行消失检查。

正式页通过 `alexa_page/local_settings.rs` 的单工作线程顺序读写 `razer_storage::store_path()` 同目录下 `alexa-settings.json`。该文件是本项目的本地存储替代：保留 `synapseSkills` 和 `alexaAudioInputDevice` 原键名以及其他 JSON 字段；不是 Chromium 数据库，也不是 DLL 写回。测试预览没有存储工作线程，场景切换不写正式文件。读写错误保留可见失败状态，不显示虚构保存成功。

## 验证与缺口

纯 Rust 测试覆盖别名/空标签过滤、顺序和重复项、ID 存在时保留旧选中对象、设备消失清空、Default 对应 null、存储晚到对已观察列表的协调，以及合并本地键不丢失其他字段。GUI 测试只编译，不执行。

2026-10-10 验证：`cargo check --locked --all-targets -p razer-app-pages` 通过；`cargo test --locked -p razer-app-pages alexa_page::input::tests:: --lib` 4 项通过；`cargo test --locked -p razer-app-pages alexa_page::local_settings::tests:: --lib` 1 项通过。两个测试命令仅运行这些纯状态/JSON 用例，其余 GUI 和其他用例被过滤；没有执行应用、媒体/设备适配、DLL 或下载的 JS。

媒体身份获取、设备变化订阅的 OS 适配以及其取消/清理仍缺失；Razer/Amazon 账户链、设置广播接收者、语音语言回退、快捷键注册、跨进程 storage 事件仍未接通。页面和本地状态修复不代表这些链路完成，也不代表运行视觉验收通过。设备与模块、迁移正式页仍各自保留未观察服务状态，不能把预览记录当真实模块、扫描结果或成功迁移。
