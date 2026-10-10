# 宿主 4.0.827 Key/Memory/WindowStorage 当前闭合链

证据：`.ref/host-4.0.827/electron/keyStorage.js`、`electron/modules/memory_storage/index.js`、`electron/modules/window_storage/index.js` 与 `electron/constants.js` 的 `storageKeyState`。这些模块是 Electron 进程内 Map；它们和 `razer-storage::write_document` 的 `APPDATA/razer_ui/profiles.json` 草稿文件是两个作用域，不能互相冒充。

Rust 实现位于 `crates/razer-storage/src/host.rs`，通过 `razer-ipc::ServiceRequest` 的 `HostStorageView`、`HostStorageCall`、`HostStorageEvents`、`HostStorageClose` 由 service worker 持有单一实例。View 必须由实际宿主消费者提供 URL、webContents 状态和 `urlInfoMap` 跟踪位；worker 不猜 HWND、URL、设备或平台状态。

## 已按源闭合的语义

- KeyStorage 保存 key 的 `value/hasValue/urlEventList`；`getKeys` 过滤无值槽，`getItem` 可返回 JS `undefined`，`removeItem` 的无值 key 返回 `{result:false,reason:""}`，最后一个无值且无订阅的槽才删除。
- Key `setItem/removeItem` 向除发送者以外的有效、非 remote、非 crashed/disposed/destroyed、已订阅 URL 发送 `keyStorageEvent`，并保留 `created/changed/removed` 对应边界（首次 set 的事件字段由源 `keyStorage.js` 发送值；wire 的当前事件字段显式可审查）。
- Memory/Window 用 URL 分组并保留插入顺序。set 首次为 `created`，覆盖为 `changed`；remove 发送 `oldValue`，clear 只清发送者 URL；reset 清空全部 Map；register/unregister 是 URL 集合。
- `getMemoryStorageItem/getWindowStorageItem` 返回 JSON 字符串数组，保留源代码对 JS truthy 值的过滤和 `windowName` 字段。Wire 用 `{defined:false}` 表达 JS `undefined`，不会把未定义冒充 `null`。
- Memory 的 `setMemoryStorageItemNoEvent` 写入但不广播。正常 set 可使用 source `targetUrlArray`，对匹配窗口发送事件；未跟踪 URL 的窗口按源 `shouldFireEvent` 规则仍可接收，远程窗口和发送者过滤保持在 worker。

## 明确的边界

三个原模块均直接以 `payload.key` 操作 JS Map，没有字符串转换。Rust 现在保留 JSON 原始类型的字符串、数字、布尔和 null key，数字按 JS Number 的 SameValueZero 比较（`1` 与 `1.0` 相同，数字 `1` 与字符串 `"1"` 不同）。KeyStorage 源的 `null==key/value` 检查仍拒绝 null；Memory/Window 源只拒绝 undefined，因此允许 null key/value。对象、数组的 JS 引用身份无法由现有 JSON wire 表达，显式拒绝而不采用结构相等冒充。

Map 是原宿主的运行时状态，worker 退出即丢失；它不是产品配置或本地草稿持久化。`HostStorageView`/`Close` 是宿主窗口的真实生命周期入口，未注册 sender、已销毁 sender 和对象身份 key 会返回错误，不以默认值掩盖缺失证据。前端页面尚未逐 action 接入这些 IPC 命令，不能把 storage 层完成误报成全 UI 完成；下一步必须接 preload/页面调用方，并保留原事件 payload 和错误反馈。JSON 不可表达的 undefined 值及其他 JS 非 JSON 类型仍是协议缺口。

源动作字节范围及 Rust 消费者见 [当前存储证据](host-storage-current-evidence.json)，由 `python tools/audit-host-storage-current.py --check` 静态复核。`cargo test --locked -p razer-storage --lib host::tests` 的 4 项纯 Rust 测试已通过，覆盖 primitive key 类型及数字相等、订阅墓碑、旧值/空值/JS truthiness、目标重复匹配、无事件写入和关闭窗口过滤。测试不运行 IPC、文件操作、vendor JS、DLL、应用或设备；实际宿主/页面及设备运行验收仍未执行。
