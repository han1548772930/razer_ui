# Current application end-to-end reverse map

This document records the static chain that can be proven from the current source snapshot: UI renderer → preload bridge → Electron main IPC → host service/native wrapper/DLL boundary → returned state and rendering. It is an index of source evidence, not a runtime trace. The source freeze is 2026-10-02; the audit was generated on 2026-10-09.

The machine-readable record is [application-end-to-end-current.json](application-end-to-end-current.json); the identical compressed copy has been removed. The generator is [audit-application-end-to-end-current.cjs](../../tools/audit-application-end-to-end-current.cjs). It reads the current UI semantic-anchor evidence and the current host 4.0.827 architecture/FFI evidence, then records exact source slices from `.ref/host-4.0.827/electron/preload.js` and `main.js`.

## What is closed and what is not

The preload bridge has three independently evidenced contracts. `doElectronAction`/`doRzDeviceAction` invoke the `electronAction` IPC channel; the FFI, mapping-engine, simple-service, lighting-driver and RzDevice helpers use their own IPC channels; and `response(channel, fn)` returns a listener-removal function. The main process contains the matching `electronAction`, native-action and FFI IPC handlers. These slices prove the bridge shape, but they do not prove that a particular screen dispatched a particular action at runtime.

The host service, middleware, remote API, native wrapper and DLL ABI are separate boundaries. A function named `get`, `set`, `true`, or `success` is not treated as hardware success. Local drafts, host settings storage, app/simple-service state, legacy `FFILibrary`, modern `ffiPreload`, and eventual device/DLL write-back are recorded as different layers. UI Apply, Save, Delete, Import, Export and Cancel branches remain in scope, while native/service write-back is intentionally deferred by the current roadmap.

No endpoint is marked semantically complete. The summary is 24 endpoints, 26 UI anchor references, 6 exact host bridge slices, and zero closed end-to-end claims.

## Endpoint coverage

| Route | Owner | Current status | Proven state boundary |
|---|---|---|---|
| `/alisha/` | Streamer Companion | `partial` | reducer/app-service session and connection state |
| `/background-manager/` | Background Manager | `infrastructure_only` | metadata, local storage and simple-service responses |
| `/chroma-app/dashboard/` | Chroma Dashboard | `partial` | app/service reducers and host broadcasts |
| `/chroma-app/settings/` | Chroma Settings | `partial` | reducer, app data and host queries |
| `/cortex/` | Cortex | `no_current_html` | no current HTML source |
| `/feedback/` | Feedback | `partial` | local form draft/validation and network submission |
| `/natalie/` | Virtual Ring Light | `partial_native` | legacy `FFILibrary` observations plus local config |
| `/profile-migration/` | Profile Migration | `partial` | reducer selection/progress/result; scanner chain incomplete |
| `/rz-app-menu/` | Application Menu | `partial` | parent `postMessage` data and host window actions |
| `/rz-user-profile-menu/` | Account Menu | `partial` | parent user/guest/balance messages |
| `/settings/` | Standalone Settings | `partial_host_storage` | host settings queries/setters plus renderer state |
| `/sophie-lite/` | 7.1 Surround Sound | `partial_legacy_ffi` | legacy FFI device selection/surround observation |
| `/sophie/` | THX Spatial Audio | `partial_legacy_ffi` | legacy FFI audio state and network license state |
| `/synapse/` | Synapse shell | `no_current_html` | no current top-level HTML source |
| `/synapse/alexa/` | Alexa | `partial` | login/device/install reducers and app-service events |
| `/synapse/armory/` | Armory | `partial` | browse/share/profile reducers and host tab state |
| `/synapse/chroma-studio/` | Chroma Studio | `partial_modern_ffi` | editor drafts and modern FFI response state |
| `/synapse/dashboard/` | Synapse Dashboard | `partial_device_observation` | middleware/window-storage broadcasts and reducer state |
| `/synapse/introduction-tour/` | Introduction Tour | `partial` | tour index and local language state |
| `/synapse/macro/` | Macro | `partial_local_service` | local drafts/history and asynchronous actor/service results |
| `/synapse/profiles/` | Profiles | `partial_local_storage` | sessionStorage navigation and local profile state |
| `/synapse/settings/` | Synapse Settings | `partial_host_storage` | startup/minimized host settings query/setter state |
| `/synapse/update-fw/` | Firmware Update | `partial_service_process` | firmware actor progress from service-process events |
| `/systray/systrayv2/` | Tray | `partial_host_bridge` | iframe parent messages and host LeftSystray actions |

The detailed per-route record in the JSON contains five explicit fields: `read_observation`, `write_persistence`, `error_cancel`, `cleanup`, and `closure`. These distinguish what the UI observes, where an edit is retained, which failure/cancel branches exist, which listeners/timers are removed, and which link is still open.

## Product-specific boundaries

Dashboard device cards are not proven to be USB reads from one DLL. The source shows middleware, window-storage/BroadcastChannel and reducer inputs, with `rzDeviceAction` only one host branch. Tray is an upstream presentation surface: it receives parent messages and host actions and is not a direct DLL reader. Macro/Profile Save and mapping actions are local/file/host-dialog operations unless a later source audit proves a device write. Firmware helper exit code is not hardware confirmation. Audio and Ring Light native calls are statically identified, but their PE bytes, ABI and return semantics remain unresolved.

## Errors, cancellation and cleanup

The map preserves source-visible offline, retry, license, setup, validation, warning, cancel, partial/complete, process-exit and ErrorBoundary branches. It also records unresolved ownership: response listeners are only closed where the returned remover is retained; anonymous listeners, actor late results, timers, media promises and beforeunload paths differ by endpoint. These are source observations, not tested behavior.

## Reproducibility and limits

Run only the static audit command required by the repository instructions:

```powershell
node tools/audit-application-end-to-end-current.cjs --check
```

The audit validates the 24 endpoint records, exact source ranges and hashes, generated JSON, and gzip output. It does not execute the application, vendor JavaScript, host, service, DLL, installer or downloaded code. Current source receipts and the exact fragments are retained in the JSON so later changes can be reviewed against the same files. No claim here closes the deferred device/service DLL write-back phase.
