# 宿主 4.0.827 Key/Memory/WindowStorage 当前闭合链

证据：`.ref/host-4.0.827/electron/keyStorage.js`、`electron/modules/memory_storage/index.js`、`electron/modules/window_storage/index.js` 与 `electron/constants.js` 的 `storageKeyState`。这些模块是 Electron 进程内 Map；它们和 `razer-storage::write_document` 的 `APPDATA/razer_ui/profiles.json` 草稿文件是两个作用域，不能互相冒充。

Rust 实现位于 `crates/razer-storage/src/host.rs`，通过 `razer-ipc::ServiceRequest` 的 `HostStorageView`、`HostStorageCall`、`HostStorageEvents`、`HostStorageClose` 由 service worker 持有单一实例。View 必须由实际宿主消费者提供 URL、webContents 状态和 `urlInfoMap` 跟踪位；worker 不猜 HWND、URL、设备或平台状态。

## 已按源闭合的语义

- KeyStorage 保存 key 的 `value/hasValue/urlEventList`；`getKeys` 过滤无值槽，`getItem` 可返回 JS `undefined`，`removeItem` 的空 key 返回 `{result:false,reason:""}`，最后一个无值且无订阅的槽才删除。
- Key `setItem/removeItem` 向除发送者以外的有效、非 remote、非 crashed/disposed/destroyed、已订阅 URL 发送 `keyStorageEvent`，并保留 `created/changed/removed` 对应边界（首次 set 的事件字段由源 `keyStorage.js` 发送值；wire 的当前事件字段显式可审查）。
- Memory/Window 用 URL 分组并保留插入顺序。set 首次为 `created`，覆盖为 `changed`；remove 发送 `oldValue`，clear 只清发送者 URL；reset 清空全部 Map；register/unregister 是 URL 集合。
- `getMemoryStorageItem/getWindowStorageItem` 返回 JSON 字符串数组，保留源代码对 JS truthy 值的过滤和 `windowName` 字段。Wire 用 `{defined:false}` 表达 JS `undefined`，不会把未定义冒充 `null`。
- Memory 的 `setMemoryStorageItemNoEvent` 写入但不广播。正常 set 可使用 source `targetUrlArray`，对匹配窗口发送事件；未跟踪 URL 的窗口按源 `shouldFireEvent` 规则仍可接收，远程窗口和发送者过滤保持在 worker。

## 明确的边界

Map 是原宿主的运行时状态，worker 退出即丢失；它不是产品配置或本地草稿持久化。`HostStorageView`/`Close` 是宿主窗口的真实生命周期入口，未注册 sender、已销毁 sender 和非字符串 JSON key 会返回错误，不以默认值掩盖缺失证据。前端页面尚未逐 action 接入这些 IPC 命令，不能把 storage 层完成误报成全 UI 完成；下一步必须接 preload/页面调用方，并保留原事件 payload 和错误反馈。

静态验证只允许哈希、源码解析、格式和 `cargo check --locked --all-targets --workspace`；不执行 vendor JS、DLL、应用、EXE 或设备。
