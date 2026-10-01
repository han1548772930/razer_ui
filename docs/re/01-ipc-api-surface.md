# Electron 宿主 IPC / API 清单与 GPUI Kit 接入边界


审计对象是 [.ref/synapse-asar](../../.ref/synapse-asar) 的 Electron 宿主（razerappengine 4.0.563）。本文件保留静态 IPC / wrapper 清单，用于查找窗口、系统、存储和设备适配边界。页面规格另见 [总规格](../RAZER-SYNAPSE-UI-SPEC.md) 和 [逐页文档](../screens/README.md)。

2026-09-30 修订：更正“必须重写全部 Electron API”“只有 HID 能通信”“行为全在几个 DLL”及偏移单位等结论。已有函数清单是静态调用面，不等于每一项都被当前三个产品调用，更不代表本轮验证过 DLL ABI 或设备成功执行。

下列旧定位数值来自 .NET ReadAllText(...).IndexOf(...)，单位是 **UTF-16 字符串索引，不是字节偏移**。以文件和搜索锚点为准；表中的 Bytes 仅表示文件字节大小，不能用于解释函数 offset。

| File | Bytes | Role |
|---|---|---|
| `electron/preload.js` | 26 992 | `contextBridge` surface (`window.apiElectron`) — the entire renderer-visible API |
| `electron/main.js` | 50 726 | All `ipcMain.handle` registrations + window/tab/app lifecycle |
| `electron/constants.js` | 9 530 | `actionEnum`, `razerAppPathEnum`, `razerAppInfo`, `windowEvent` |
| `electron/UsbRzDeviceAction.js` | 29 868 | `hid.*` / `usb.*` (node-rz-hid, rz-usb-detect) |
| `electron/WssAction.js` | 3 948 | Embedded **WebSocket server** for plugin clients |
| `electron/serviceFunction.js` | 1 563 | Windows service start/stop/status via `RzPowerTool.exe` |
| `electron/keyStorage.js` | 3 946 | Cross-window key/value store + change events |
| `electron/mainSubFunction.js` | 12 715 | Logging, process info, endpoint discovery, Loupedeck |
| `electron/nativeNotificationHandler.js` | 2 391 | Toast via `rzNotification.exe` |
| `electron/components/Tab/TabManager.js` | 8 514 | `tabEvent` channel |
| `electron/modules/*` | — | serial / noble(BLE) / wifi / IoT / LampArray / lighting / mapping_engine / simple_service / sysutil / storage / FFI |
| `electron/Protocol/*` | — | Protocol-25 HID command encoding + protocol logger |

> 宿主包主要提供 native bridge 和窗口管理；UI 由应用 URL 加载。当前工作区已经另行取得 `.ref/frontend` 与三个 `.ref/devices` 产品包，必须同时使用这些资料。替换为 GPUI Kit 需要实现实际页面及其所需行为；本 IPC 清单不能独立充当 UI 规格，也不要求原样复刻所有宿主服务。

---

## 0. Transport model / 传输边界

There are **four** distinct transports:

### 0.1 Renderer → main: Electron IPC (`ipcRenderer.invoke`)
`preload.js` builds one object and exposes it as `window.apiElectron`
(`preload.js` offset **3060** = `exposeInMainWorld("apiElectron"`). It carries:

- static metadata: `electronVersion`, `chromeVersion`, `nodeVersion`, `appData`, `localAppData`,
  `userDataDir`, `rendererProcessId`, `programFiles64Dir`, `programFiles32Dir`,
  `engineVersion:"4.0.563"`, `windowName` (parsed from `--razer-page-name=` argv),
  `supportElectron20`, `setDevicesToDefaultNoAsync`, `osPlatform`, `isBeta`
  (`preload.js` offsets 3060–4200)
- 17 device/DLL bridge functions (see §1, dispatcher mapping)
- window convenience wrappers (`setBounds`, `Minimize`, `Close`, `getAllDisplays`, …)

All 17 `ipcMain.handle` channels are registered in `main.js` (offsets **31 754 – 50 621**) and one in
`components/Tab/TabManager.js` (offset **1 631**).

### 0.2 Main → renderer: `webContents.send` push events
No `ipcRenderer.on` is exposed through the bridge for most of them; `preload.js` listens directly.
Full inventory in §19.

### 0.3 The **plugin WebSocket server** (`WssAction.js`) — a second, non-Electron API
This is *not* the UI's path, but it is a first-class external API and the UI controls it.

- `basePort = 5426` (`WssAction.js` offset **197**) — note 5426 = `0x1532`, the Razer USB vendor ID.
- `new e.Server({ port: this.port, path: "/synapse" })` (`WssAction.js` offset **383**).
- On a port clash it retries `basePort+1`, then `basePort+2` (max 3 attempts).
- Only four `clientName` values are accepted; anything else is rejected with close code `1008`:
  `SynapseControlPlugin`, `AudioMixerPlugin`, `KeylightChromaControlPlugin`, `KiyoProUltraPlugin`
  (`WssAction.js` offset **26**).
- Server → main-process event: `handleClientMessage` does
  `sender.send("webSocketMessage", e)` → the renderer receives a `webSocketMessage` IPC event
  carrying `{eventType: ws_connect|ws_message|ws_close|ws_error, clientName, requestId, data}`.
- Server pushes `{"data":"pong"}` every 30 s per client.

### 0.4 Local HTTP dev server + `application-host`
- `--devport=<n>` makes the shell open `http://localhost:<n>` (`main.js` offset **15 449**,
  `createDefaultWindow()` = `function Vo()` at offset **14 295**; URL built as
  `"http://localhost:" + So + Lo`; `--devport` parsed at offset **5 951**).
- `--application-host=<host>` (`main.js` offset **5 372**, re-parsed on `second-instance` at
  ≈ **48 500**) rewrites `Ao` (the base origin):
  - contains `localhost` → forced `http://…`
  - ends with `.razer.com` → forced `https://…`
  - anything else → falls back to the constant `https://apps.razer.com` (`main.js` offset **4 218**,
    `ho="https://apps.razer.com"`).
- HTTP headers to `https://*.razer.com/*` get an `x-rzr-endpoint` header injected by
  `appendRazerEndpointToHttpHeader` (`mainSubFunction.js`, `onBeforeSendHeaders`); the value comes
  from `<commonDLL>/endpoint.txt` via `getRazerEndpoint()`.
- `getLoupedeckWebSocketPort` reads `SynapsePort.txt` from
  `%LOCALAPPDATA%\Loupedeck\Temp\` (`mainSubFunction.js`) — a *third-party* integration surface.

Other argv flags that alter behaviour: `--url-params=apps=a,b,c`, `--launch-force-hidden=`,
`--force-close-window`, `--proxy-server=`, `--ignore-certificate-errors`, `--host-rules=`,
`--disable-logging`, `--main-log-level=`, `--hibernate-timeout=`, `--autoStart=1`,
`--exit-application`, `--exit-application-uninstall-relaunch`, `--exit-application-relaunch`,
`--uninstall-app=`, `--migrateRzAppEngine=`, `--extension=`, `--open-debug-window`,
`--local-crash-report`, `--devporturl=`, `--logProtocol`, `--debugCommonDLL`
(`main.js` offsets ≈ 5 300 – 9 200).

---

## 1. IPC channel inventory (`ipcMain.handle`)

| # | Channel | Registered at (`main.js` UTF-16 offset) | Backing module | Purpose |
|---|---|---|---|---|
| 1 | `electronAction` | 31 754 | `main.js` inline + `memory_storage` + `window_storage` + `sysutil` | **The catch-all.** Window/app/tab lifecycle, displays, dialogs, storage, login state, sysutil pass-through |
| 2 | `mappingEngineAction` | 48 672 | `modules/mapping_engine/win/index.js` via FFI | Key/macro/input-hook engine (`mapping_engine.dll`) |
| 3 | `simpleServiceAction` | 48 732 | `modules/simple_service/win/index.js` via FFI | Process launching, audio devices, user apps (`simple_service.dll`) |
| 4 | `lightingDriver` | 48 792 | `modules/lighting/ffiLightingDriver.js` | Chroma lighting driver DLL lifecycle |
| 5 | `rzDeviceAction` | 48 847 | `UsbRzDeviceAction.js` | USB enumerate + HID read/write + mutex + OLED |
| 6 | `rzSerialDeviceAction` | 48 907 | `modules/serial/index.js` | Serial/COM ports (`serialport-rz`) |
| 7 | `ffiPreload` | 49 247 | `modules/ffi/FFIPreloadMain.js` | Generic synchronous FFI DLL call into main process |
| 8 | `ffiPreloadAsync` | — | `modules/ffi/FFIPreloadMain.js` | Async variant |
| 9 | `ffiSubPreloadAsync` | 49 362 | `modules/ffi_subprocess/FFIPreloadSubProcess.js` | Same, executed in a `utilityProcess` child |
| 10 | `rzBleDeviceAction` | 49 430 | `modules/noble/index.js` | BLE via Noble |
| 11 | `nativeNotification` | 50 141 | `nativeNotificationHandler.js` | Windows toast via `rzNotification.exe` |
| 12 | `rzWssAction` | 50 195 | `WssAction.js` | Start/stop/send on the plugin WebSocket server |
| 13 | `rzIoTAction` | 50 252 | `modules/IoT/IoTNativeAction.js` | IoT lighting devices (`IoTSDKNative_Action` FFI channel) |
| 14 | `rzLampArrayAction` | 50 485 | `modules/LampArray/LampArrayAction.js` | Windows LampArray (`rzLampArrayChannel` FFI channel) |
| 15 | `rzWifiAction` | 50 556 | `modules/wifi/index.js` | Wi-Fi scan/connect (`node-rz-wifi`) |
| 16 | `keyStorageAction` | 50 621 | `keyStorage.js` | App-wide key/value store with URL-scoped events |
| 17 | `tabEvent` | `TabManager.js` 1 631 | `components/Tab/TabManager.js` | Tab create/close/focus/tooltip |

Renderer dispatcher mapping (`preload.js`, offsets 4191–5321):

```
doTabAction              -> invoke("tabEvent")
doElectronAction         -> invoke("electronAction")
doRzDeviceAction         -> invoke("rzDeviceAction")        // 4191
doSerialDeviceAction     -> invoke("rzSerialDeviceAction")
doBleDeviceAction        -> invoke("rzBleDeviceAction")     // 4311
doIoTDeviceAction        -> action.startsWith("wifi.") ? invoke("rzWifiAction")
                          : action.startsWith("IoT.")    ? invoke("rzIoTAction")
                          : console.error("unknown action:")   // 4370
doNativeNotificationAction -> invoke("nativeNotification")
doRzWssAction            -> invoke("rzWssAction")           // 4623
doLampArrayDeviceAction  -> invoke("rzLampArrayAction")
doDLLAction / doDLLActionAsync -> DEPRECATED, logs an error only
doDLLMainAction          -> invoke("ffiPreload")            // preload offset 4918
doDLLMainActionAsync     -> invoke("ffiPreloadAsync")
doDLLSubProcessActionAsync -> invoke("ffiSubPreloadAsync")
doMappingEngineAction    -> invoke("mappingEngineAction")   // 5130
doSimpleServiceAction    -> invoke("simpleServiceAction")
doLightingDriverAction   -> invoke("lightingDriver")        // 5260
doKeyStorageAction       -> invoke("keyStorageAction")      // 5321
ipcRendererSend(channel,data) -> raw ipcRenderer.send (used for "close-iframe")
response(channel,cb)     -> raw ipcRenderer.on with auto-remove
```

`preload.js` also opens a `BroadcastChannel("electron-broadcast-channel")`; a `loggedIn`/`loggedOut`
message from *any* window is re-injected as `electronAction{action:"loggedIn"|"loggedOut", payload}`
(this path additionally computes `payload = currentGuestUserId === currentUserId`).

**Debug hooks** (only present when the `enable-rz-protocol` argv flag is set): `rzIpcInvokeHook(cb)`
and `rzIpcResponseHook(cb)` monkey-patch `ipcRenderer.invoke` / `.on` so a caller can observe all IPC
traffic (`preload.js` offsets ≈ 2 600–3 000).

---

## 2. `electronAction` — 静态 action 清单

### 2.1 Login / session / environment (main.js 31 838 – 32 235)
| action | Behavior |
|---|---|
| `loggedIn` | Sets `isLogin=true`, stores `isGuest`, rebuilds the tray menu |
| `loggedOut` | Clears login state, rebuilds the tray menu |
| `updateLang` | Sets `globalNodeVar.lang` (drives tray labels), rebuilds tray |
| `process.versions` | Returns Node/Electron/Chrome/V8 versions |
| `process.env.APPDATA` | Returns `%APPDATA%` |
| `process.env.LocalAppData` | Returns `%LOCALAPPDATA%` |

### 2.2 `actionEnum` values actually dispatched (main.js 32 300 – 39 700)
| action | Behavior |
|---|---|
| `minimize` | Minimize the sender's window |
| `maximize` | Toggle maximize/unmaximize |
| `isMaximized` | Returns boolean |
| `restore` | Restore window |
| `close` | Mark will-close then close sender's window |
| `closeWindow` | `BrowserUtil.closeWindow(windowName)` — policy-aware close for a named window |
| `setBounds` | `setBounds({x,y,width,height})` on sender |
| `getBounds` / `setSize` / `getSize` / `center` | Geometry, optionally targeting `windowName` |
| `getWindowStatus` | `{isMinimized,isNormal,isMaximized,isVisible,isResizable,size,maximumSize,minimumSize,bounds}` |
| `setBrowserVisible` | Show/hide sender window (`BrowserUtil.setBrowserVisible`) |
| `setBrowserVisibleWithName` | Same, for an explicit `windowName` |
| `setBrowserAlwaysOnTop` | `setAlwaysOnTop(true,"screen-saver",1)` or off |
| `setBoundWithName` | Direct `browserWindow.setBounds` for a named window |
| `isVisible` | Returns visibility of a named window |
| `setBrowserButtonVisible` | macOS only — traffic-light buttons |
| `getAllDisplays` | `screen.getAllDisplays()` |
| `getDisplayNearestPoint` | `screen.getDisplayNearestPoint({x,y})`, defaults to sender bounds |
| `getMonitorInfo` | See quirk §20.1 — returns mapped display list |
| `openInDefaultBrowser` / `openExternalWindow` | `shell.openExternal(url)` |
| `activateWindowServiceClient` / `focus` | Focus window by name or by tab; also emits `focusSubTab` via `tab-message` |
| `getWindowServiceClients` | Lists every tab (id, url, `backgroundThrottling`, frameName, pos, hibernation time) and every top-level window (`type:"tab"`/`"window"`, `hibernateStatus`) |
| `getFocusTab` | Returns the focused tab's `frameName` for a window |
| `setBrowserTabVisible` | `TabManager.sendToggleShowTabAction(frameName,isShow,isActive)` |
| `getProcessInfo` | Per-window `{url,windowName,processId,processInfo{Name,WorkingSet,PrivateMemorySize,Id}}` — process info shelled out to `powershell Get-Process -Id …` |
| `getUserApps` | Enumerates installed user apps |
| `getAllProcessInfo` | All process/window info |
| `getProcessesByName` | Filter by process name |
| `getCursorScreenPoint` | Mouse position |
| `getWindowVersion` | `RzWindowVersion` (renderer build string) |
| `exitApplication` | `quitApp(appName, isSimpleQuit)`; body read from `payload.appName`/`payload.isSimpleQuit` |
| `setCanExitAppEngine` | Veto list: `payload.actionArgs=[allow:boolean, appName]` maintains `cannotExitAppNameList` |
| `launchAppSimple` | Focus/restore an already-running app window, else create a bare `BrowserWindow` from `payload.url` |
| `loadURLWithQuery` | Loads `url` into `winMap[windowName].childViews[1]` (the "second layer" webview) |
| `relaunch` | `app.relaunch()` then `quitApp()` |
| `relaunchWithLauncher` | Writes `Logs/relaunch.log` `{apps,host}` then relaunches via `RazerAppEngine.exe` so the launcher re-reads it |
| `updateControlPanelVersion` | Delegates to `simple_service.updateControlPanelVersion` |
| `getRazerEndpoint` | Returns the cached endpoint from `endpoint.txt` |
| `getLogFiles` | Collects `userData` + `system` log files (incl. `setupapi.dev.log`) |
| `fileExists` | `fs.existsSync(payload.actionArgs)` |
| `setRestoreFileWhenCrash` | Persists `{vendorId,productId,content}` for crash restore |
| `getOsUserName` / `getSystemInfo` / `getOsVersion` | Host info (system info via `sysutil.GetSystemLanguage` etc.) |

### 2.3 Tab / window-manager actions (main.js 39 700 – 42 400)
| action | Behavior |
|---|---|
| `storeData` | Persist tab layout (`TabManager.storeData`) |
| `setManualClose` / `removeManualClose` / `getManualCloseList` | Manual-close tab set |
| `setForceFocus` / `getForceFocus` | Force-focus tabs per window |
| `addTabTooltipStatus` | `{text,icon,htmlText}` into the tab tooltip |
| `setTabTitle` | Rename a tab |
| `handleCloseBtn` | Show/hide a window's close button |
| `setTabPos` / `setTabPosArr` | Push tab strip positions back to the page |
| `openDevTool` / `openWindowDevTool` / `toggleDevTools` | DevTools (undocked when always-on-top) |
| `checkRazerIdAvailable` | Retries the `razer-id` login page up to 5 s, then opens it |
| `getFallbackPageInfo` | Sends `razer-fallback-channel {initFallbackPage|show-no-internet}` |
| `retryWhenPageDown` | Re-fetch a failed page and relaunch its window |
| `hibernate` / `hibernateWindow` / `cancelHibernateWindow` / `restoreFromHibernate` | Tab & whole-window hibernation (`--hibernate-timeout`) |
| `registerWindowListChange` | Subscribe sender to `window-list-change` push events |
| `registerNoBrowserInputHandler` / `unRegisterNoBrowserInputHandler` | Suppress keyboard input handling in a window |
| `showFileOpenDialog` | `dialog.showOpenDialog`, extension filter from `payload.types` (dots stripped), `multiSelections` |
| `showFileSaveDialog` | `dialog.showSaveDialog`, remembers last dir, writes `payload.data` |
| `getNotificationURI` | Returns the custom-protocol URI from argv (`razerappengine://…`) |
| `logIn` / `logOut` | Broadcast `electron-action{action:logIn|logOut}` to all renderers + tray state + `LeftSystray.login()/logout()` |

### 2.4 Storage actions routed through `electronAction` (main.js 44 786 – 45 460)
Memory storage (module `modules/memory_storage/index.js`):
`registerMemoryStorageEvent`, `unRegisterMemoryStorageEvent`, `setMemoryStorageItem`,
`setMemoryStorageItemNoEvent`, `getMemoryStorageKeys`, `getMemoryStorageItem`,
`removeMemoryStorageItem`, `clearMemoryStorage`, `getMemoryStorage`, `resetMemoryStorage`

Window storage (module `modules/window_storage/index.js`):
`registerWindowStorageEvent`, `unRegisterWindowStorageEvent`, `setWindowStorageItem`,
`getWindowStorageKeys`, `getWindowStorageItem`, `removeWindowStorageItem`, `clearWindowStorage`,
`getWindowStorage`, `resetWindowStorage`

Both are keyed by **sender URL**, emit `memoryStorageEvent` / `windowStorageEvent`
(`{key,keyState:created|changed|removed|cleared,newValue,oldValue,srcURL}`) to sibling windows, and
can be targeted with `targetUrlArray`.

### 2.5 Lighting-engine lifecycle (main.js 45 458 – 45 800)
| action | Behavior |
|---|---|
| `lightingEngineRunning` | No-op `break` |
| `lightingEngineCanShutdown` | Sets `gIsLightingEngineCanShutdown = true` |
| `lightingEngineExitCompleted` | Sets `gIsLightingEngineExitCompleted = true` |
| `lightingEngineSetupDriver` | Installs `ffiLightingDriver.onWriteCallback`: routes driver writes with `function_name` starting `hid.` → `UsbRzDeviceAction`, `IoT.` → `IoTNativeAction`, `LampArray.` → `LampArrayAction`; then `enableFFICallback()` |
| `isOnBatteryPower` | `powerMonitor.isOnBatteryPower()` |

### 2.6 Misc (main.js 46 400 – 48 000)
`setRzWindowVersion` (→ `RzWindowVersion.callFunction`: `setRzWindowVersion`, `futureImplementation`)

### 2.7 sysutil pass-through (main.js 42 907 – 44 650)
These are **forwarded verbatim** to `SysUtilsNative.dll` via `globalNodeVar.ffiSysUtils.callDLL`,
or `callDLLAsync` for the four marked async:

| # | action | Function |
|---|---|---|
| 1 | `initialize` | Init the sysutils DLL |
| 2 | `terminate` | Tear it down |
| 3 | `setApplicationAutoStart` | Register/unregister run-at-login |
| 4 | `applicationAutoStart` | Query run-at-login |
| 5 | `getApplicationAutoStartApps` | List auto-start apps |
| 6 | `launchMicrosoftApp` | Launch an MS Store app |
| 7 | `launchTaskManager` | Open Task Manager |
| 8 | `launchFileExplorer` | Open Explorer |
| 9 | `msSettings` | Open a Windows Settings page |
| 10 | `getFileVersionInfo` | PE version resource |
| 11 | `keyboardLayout` | Current keyboard layout (Win) |
| 12 | `hibernateWorkstation` | Hibernate |
| 13 | `lockWorkstation` | Lock |
| 14 | `restartWorkstation` | Restart |
| 15 | `shutdownWorkstation` | Shutdown |
| 16 | `sleepWorkstation` | Sleep |
| 17 | `OpenAudioProperties` | Sound control panel |
| 18 | `OpenKeyboardProperties` | Keyboard control panel |
| 19 | `OpenMouseProperties` | Mouse control panel |
| 20 | `OpenSoundVolume` | Volume mixer |
| 21 | `systemSKU` | SMBIOS SKU |
| 22 | `windowsSystemDirectory` | `System32` path |
| 23 | `computerName` | Hostname |
| 24 | `displayPowerState` | Monitor power state |
| 25 | `getMonitorInfo` | Monitor info from sysutils (see quirk §20.1) |
| 26 | `getVirtualScreenRect` | Virtual desktop rect |
| 27 | `GetRazerMonitorList` | Razer-specific monitor list |
| 28 | `getCurrentMonitor` | Monitor under a point |
| 29 | `SetWindowAppId` | Set AppUserModelID (taskbar grouping) |
| 30 | `ShowColorPicker` | Native colour picker |
| 31 | `CloseColorPicker` | Close it |
| 32 | `getNetworkStatus` | Online/offline |
| 33 | `GetGMS3Info` | GMS3 info blob |
| 34 | `StartMonitorForegroundWindow` | Start foreground-window hook |
| 35 | `StopMonitorForegroundWindow` | Stop it |
| 36 | `IsScreenLocked` | Screen-lock state |
| 37 | `GetDriverInfo` | Installed driver info |
| 38 | `GetSystemLanguage` | System language |
| 39 | `systemFamily` | OS family |
| 40 | `GetSystemSerialNumber` | Machine serial |
| 41 | `StartMonitorKeyboardLayout` | Start layout-change hook |
| 42 | `StopMonitorKeyboardLayout` | Stop it |
| 43 | `getTouchPadEnableStatus` | Touchpad on/off |
| 44 | `toggleTouchPadEnableStatus` | Toggle touchpad |
| 45 | `getWheelScrollLines` | Scroll lines |
| 46 | `setWheelScrollLines` | Set scroll lines |
| 47 | `registerDiscord` | Discord rich presence registration |
| 48 | `GetURLFromInternetShortcut` | Resolve `.url` |
| 49 | `OpenGameController` | Game Controllers cpl |
| 50 | `GetModuleInfo` | Loaded module info |
| 51 | `SetProcessPowerThrottling` | EcoQoS / power throttling |
| 52 | `GetInstalledRazerAppEngineProduct` | Installed Razer products |
| 53 | `GetPathFromShortcutLnk` | Resolve `.lnk` |
| 54 | `getWindowsLogonTime` | Logon timestamp |
| 55 | `isDarkMode` | Dark mode flag (also used by notifications, §2.9) |
| 56 | `isFullScreenMode` | Fullscreen detection |
| 57 | `getASRockSystemEdition` | ASRock motherboard info |
| 58 | `BringProcessWindowToFront` | Focus another process's window |
| 59 | `isMediaFoundationInstalled` | Media Foundation check |
| 60 | `launchActivityMonitor` | macOS Activity Monitor |
| 61 | `launchMacOsApp` | macOS app launch |
| 62 | `launchFinder` | macOS Finder |
| 63 | `macSetting` | macOS Settings pane |
| 64 | `keyboardLayoutMac` | Current layout (mac) |
| 65 | `IsAppLaunched` | Registered-app query |
| 66 | `RegisterAppLaunched` | Register a launched app |
| 67 | `UnRegisterAppLaunched` | Unregister |
| 68 | `GetRegisteredApplaunchedList` | List |
| 69 | `getInstalledApplicationList` *(async)* | Installed apps (for the app launcher UI) |
| 70 | `GetMachineId` *(async)* | Stable machine identifier |
| 71 | `getApplicationIcons` *(async)* | App icons (base64) |
| 72 | `getApplicationIconsAlt` *(async)* | Alternate icon extraction |

Additional sysutil actions called internally: `CreateExitEvent`, `SetExitEvent`,
`MigrateRzAppEngineRegistry`, `GetWindowsUBRVersion`, `GetSystemLanguage`
(`mainSubFunction.js`), and the sysutil module's own event names
`foregroundWindow`, `keyboardlayoutchange`, plus the `StartMonitorForegroundWindow` /
`StopMonitorForegroundWindow` / `StartMonitorKeyboardLayout` / `StopMonitorKeyboardLayout` cases it
implements (`modules/sysutil/win/index.js`; mac variant in `modules/sysutil/mac/index.js`).

---

## 3. `tabEvent` (multi-tab window manager)

`main.js` never registers this channel; it lives in `TabManager.js` (offset **1 631**).

| action | Behavior |
|---|---|
| `tab-create` | Creates a `WebviewTag`/`BrowserView` tab from `data.{url,features,windowId}`; shows close button if needed |
| `tab-changeActive` | Activates a tab, restores it from hibernation if needed, re-lays out bounds, notifies the window |
| `tab-close` | Destroys the tab and activates `tabNameToBeActive` |
| `tab-request-close` | Close request (respects force-focus and "manual close") |
| `tab-request-changeActive` | Activate request (respects force-focus) |
| `tab-showTooltip` | Shows the hover tooltip for a tab |
| `tab-hideTooltip` | Hides it |
| `tab-updated` | Updates tab-strip positions from `data.tabPosInfo` JSON |
| `tab-userFocusTab` | User clicked a tab → `userFocusTab` down to the preload tooltip |

TabManager also emits `window-list-change` to registered windows with
`{action: winMapChanged|tabListChanged|activeTabChanged|close|create}`.

---

## 4. `rzDeviceAction` — USB / HID subsystem (`UsbRzDeviceAction.js`)

Backed by `node-rz-hid` + `rz-usb-detect`. A `deviceMap` keyed by
`"<productId>-<deviceContainerId>-<claimInterface>"` holds `{hidDevice, mutex, mutexRelease, mutexUrl}`
(`async-mutex` with a 1 000 ms timeout).

| action | Behavior |
|---|---|
| `usb.version` | `rz-usb-detect` version |
| `usb.getDevices` | `usbDetect.find()` — full device list with `deviceContainerId`, `locationId`, `deviceAddress` |
| `hid.version` | `node-rz-hid` version |
| `hid.getDevices` | `HID.devices()` |
| `hid.removeDevice` | Close the cached HID handle and cancel its mutex |
| `hid.openDevice` | Open by `path`; optionally send `queryData` via `write` or `sendFeatureReport`; auto-starts the read loop on write |
| `hid.sendFeatureReport` / `hid.sendFeatureReportMutex` | `sendFeatureReport`, length-90 payloads get `[reportId??claimInterface, ...data]` prefixing |
| `hid.getFeatureReport` | `getFeatureReport(reportId, reportLength)`; strips byte 0 unless `noSlice` |
| `hid.sendFeatureReportInBatch` | Batch write (lighting frames), gated by `lightingManagerFrames` |
| `hid.setDevicesToDefault` | Replays per-device `dataSend` arrays, protocol-25 aware, honours `delay` |
| `hid.configureDevicesDefaultData` | Caches the default command set and sets `useDefaultCmdNoAsync` |
| `hid.setDevicesToDefaultNoAsync` | Interleaved sync replay across all devices (index-major) |
| `hid.setTimerResolution` / `hid.clearTimerResolution` | `timeBeginPeriod`-style timer precision |
| `hid.isBatchLightingSupported` | Capability probe |
| `hid.write` / `hid.writeMutex` | `write([...dataSend])` |
| `hid.read` | 5× `readTimeout(10)` retry loop |
| `hid.getInputReport` | `getInputReport(reportId, reportLength)` |
| `hid.acquireMutex` / `hid.releaseMutex` | Manual inter-renderer arbitration; records `mutexUrl` (sender URL) |
| `hid.startLightingFramesInElectronMain` / `hid.stopLightingFramesInElectronMain` | Toggle the frame gate |
| `hid.writeOledBatch` / `hid.writeOledBatchMutex` | Paged OLED/matrix upload with `packetSize`(65), `maxRetryOut`(3), `maxRetryIn`(20), `sleepTimeBetweenOut/In` |
| `hid.writeAsync` / `hid.readAsync` | Non-blocking variants |
| `hid.registerHidDeviceEvent` / `hid.unregisterHidDeviceEvent` | Subscribe to raw HID input events (`modules/hidHardwareEvents`) |
| `hid.addDeviceCmdDataSuspend` / `hid.removeDeviceCmdDataSuspend` | Maintain the "apply on suspend" command set (keyed by `transactionIdCmdSteer`) |
| `hid.addDeviceCmdDataSuspend_send_test` | Immediately replay the suspend set |

**Power/state hooks (main-process, no renderer involvement):**
`globalNodeVar.events` `nodeUSBEvent-remove` (vendorId 5426 only) → `hid.removeDevice`;
`appEvent` `tabDestroyed` → release the mutex held by a destroyed tab's URL;
`commonDestroyed`; on `suspend` → `hid.setDevicesToDefaultNoAsync` + suspend set + disable all
mapping hooks; on `lock-screen` → set devices to default; on `resume` / `unlock-screen` → re-enable
mapping hooks (skipped while `IsScreenLocked`); on `exit-app-lighting-engine` → set devices to default.

`initUSB()` polls `rz-usb-detect` and broadcasts `nodeUSBEvent-add` / `nodeUSBEvent-remove` to every
webContents and to the internal event bus.

---

## 5. `rzSerialDeviceAction` (`modules/serial/index.js`)

| action | Behavior |
|---|---|
| `serial.devices` | `SerialPort.list()`; caches by lowercased `containerId` |
| `serial.connect` | Opens at **baudRate 115200**, `autoOpen:false`, 3 s timeout; pipes `data` to `webContents.send(notifyName, Buffer)`; on `Access denied` sends a Sentry warning `serial-comport-error-bdf54fa18318` with the list of Razer processes holding the port |
| `serial.write` | Writes `dataSend` to an open port (payload is deliberately not logged) |
| `serial.close` | Closes and forgets the port |
| `serial.remove` | Forgets the port without closing |

---

## 6. `rzBleDeviceAction` — BLE via Noble (`modules/noble/index.js`)

| action | Behavior |
|---|---|
| `ble.devices` | Known/paired BLE devices |
| `ble.startScan` | Start advertisement scan |
| `ble.stopScan` | Stop scan |
| `ble.connect` | Connect (per-device strategy: `protocol30`, `protocol30Layla`, `protocol40`, `protocolAudiowise`, `standard`) |
| `ble.disconnect` | Disconnect |
| `ble.send` | Write to a GATT characteristic; when `--logProtocol` is on, logs `(mac, serviceUUID, "s"/"r", data, length, senderURL)` to `protocol.log` |
| `ble.read` | Read a characteristic |
| `ble.notify` | Subscribe to notifications (`rz-ble-notify-<…>` per strategy) |
| `ble.updateStatus` | Push device status |
| `ble.configureAudioDeviceSupport` | Enable audio-device-oriented handling |

Main-process push events from this module: `rz-ble-system-notify`,
`rz-ble-notify-*` (per strategy). The module also exposes `startBLEEndpointMonitoring` /
`stopBLEEndpointMonitoring`, wired in `main.js` `whenReady`/`quitAppFinal`.

---

## 7. `rzWifiAction` (`modules/wifi/index.js`, `node-rz-wifi`)

| action | Behavior |
|---|---|
| `wifi.getWifiList` | `scan()` → `{result, error, data:[…]}` |
| `wifi.connect` | `connect({ssid, password})` |
| `wifi.disconnect` | `disconnect()` |
| `wifi.getCurrentConnections` | Current connection list |
| `wifi.deleteConnection` | `deleteConnection({ssid, password})` |

Routing is done in `preload.js` by the `wifi.` prefix (offset 4370).

---

## 8. IoT (`modules/IoT/IoTNativeAction.js`)

Channel `rzIoTAction`. FFI channel name `IoTSDKNative_Action` (offset 114). Commands go through
`IOT_RunCmdFw25(host, "<cmd>", jsonData, len, bool)`.

| action | Behavior |
|---|---|
| `IoT.SetChromaFrameV2` | Requires `dataSend.length >= 6`; forwards verbatim |
| `IoT.SetChromaFrame` | Rebuilds the frame as `[0, dataSend[0], dataSend[3], dataSend[4], dataSend[5], ...dataSend.slice(6), whiteLedBrightness]`; brightness defaults to **125** and is stored per `host` |
| `IoT.UpdateWhiteLEDBrightness` | Sets `whiteLEDBrightnessMap[host] = nWhiteLedBrightness` |
| `IoT.IsLightingDriverSupport` | Always returns `true` |

Payload is `{host, dataSend}`; `host` is mandatory.

---

## 9. LampArray (`modules/LampArray/LampArrayAction.js`)

Channel `rzLampArrayAction`, FFI channel `rzLampArrayChannel` (offset 114). Payload requires
`deviceID`; `indices` required for the effect call.

| action | Behavior |
|---|---|
| `LampArray.effect_set_colors_for_indices` | Calls `libFFI.JsonApiNoReturn(JSON.stringify({action:"effect_set_colors_for_indices", device_id, indices, colors:dataSend}))` |
| `LampArray.IsLightingDriverSupport` | Always `true` |

This is the bridge to the native Windows **LampArray** API (keyboard/mouse lighting for
LampArray-aware apps).

---

## 10. Lighting driver (`modules/lighting/ffiLightingDriver.js`)

Channel `lightingDriver` → `callDLL(event, action)`. Actions:

`InitDLL`, `GetDllVersion`, `AddDevice`, `RemoveDevice`, `Configure`, `Pause`, `Resume`,
`HookLightingCallback`

`HookLightingCallback` installs `onWriteCallback(fn)`, which is what
`electronAction{action:"lightingEngineSetupDriver"}` wires up to the HID / IoT / LampArray routers
(§2.5). This is the Chroma "lighting engine" ↔ device-write path.

---

## 11. Mapping engine (`modules/mapping_engine/win/index.js`, via `mappingEngineAction`)

Windows DLL: `mapping_engine.dll` (`mapping_engine.dylib` on macOS). This is the **key/macro/input**
subsystem. 130 distinct actions:

**Lifecycle / state**
`mappingEngineInitialize`, `mappingEngineShutdown`, `ConfigureFFI`, `isMappingEngineInitialize`

**Device registry**
`addDevice`, `removeDevice`, `addUsbDevice`, `removeUsbDevice`, `addUsbDeviceWithoutFilterDriver`,
`new-ble-device`, `update-ble-device`, `disconnect-ble-device`, `update-system-bluetooth-status`,
`notifyInputForExternalDevice`

**Device mode / profiles**
`getDeviceMode`, `getGlobalMode`, `isHypershiftActive`, `isOtfsActive`, `queryRunningMacrosAndKeysPressed`

**Hook enable/disable (global)**
`enableAllDevicesHooks`, `disableAllDevicesHooks`, `isAllDevicesHooksEnabled`,
`enableMapping`, `disableMapping`

**Input redirection**
`enableInputRedirect`, `disableInputRedirect`, `isInputRedirectEnabled`,
`enableKeyboardInputRedirect`, `disableKeyboardInputRedirect`, `isKeyboardInputRedirectEnabled`,
`enableMouseInputRedirect`, `disableMouseInputRedirect`, `isMouseInputRedirectEnabled`,
`enableMouseMoveRedirect`, `disableMouseMoveRedirect`, `isMouseMoveRedirectEnabled`,
`enableRazerKeyInputRedirect`, `disableRazerKeyInputRedirect`, `isRazerKeyInputRedirectEnabled`,
`setInputRedirectCallback`, `setMouseMoveRedirectCallback`

**Input monitoring**
`startKeyboardInputMonitoring`, `stopKeyboardInputMonitoring`, `hasKeyboardInputMonitoringStarted`,
`startMouseInputMonitoring`, `stopMouseInputMonitoring`, `hasMouseInputMonitoringStarted`,
`setInputMonitoringCallback`, `registerInputNotification`, `unregisterInputNotification`,
`setInputNotificationCallback`, `isInputNotificationRegistered`

**Event registration (register + `is*Registered` + `set*Callback` triples)**
`registerDeviceEvent`/`unregisterDeviceEvent`/`is…` + `deviceEvent`;
`registerHardwareEvent`/`unregisterHardwareEvent` + `registerIntervaledHardwareEvent`;
`registerDeviceModeChangedEvent` + `setDeviceModeChangedEventCallback` + `isDeviceModeChangedEventRegistered`;
`registerJoystickEvent`/`unregisterJoystickEvent` + `registerIntervaledJoystickEvent` + `setJoystickEventCallback`;
`registerXboxGamepadEvent`/`unregisterXboxGamepadEvent` + `registerIntervaledXboxGamepadEvent` + `setXboxGamepadEventCallback`;
`registerAnalogKeyEvent`/`unregisterAnalogKeyEvent` + `registerAllAnalogKeysEvent`/`unregisterAllAnalogKeysEvent` +
`registerIntervaledAnalogKeysEvent` + `setAnalogKeyEventCallback` + `isAnalogKeyEventRegistered`/`isAllAnalogKeysEventRegistered`;
`registerSnaptapKeyEvent`/`unregisterSnaptapKeyEvent` + `setSnaptapKeyEventCallback` + `isSnaptapKeyEventRegistered`;
`registerUnsupportedMapping`/`unregisterUnsupportedMapping` + `setUnsupportedMappingCallback` +
`setGlobalShortcutUnsupportedMappingCallback` + `isUnsupportedMappingRegistered`
*(razer `Snaptap` = Razer's "Snap Tap" SOCD feature)*

**Global shortcuts**
`registerGlobalShortcut`, `unregisterGlobalShortcut`, `unregisterGlobalShortcuts`,
`getGlobalShortcuts`, `isGlobalShortcutRegistered`, `enableGlobalShortcut`, `disableGlobalShortcut`,
`registerGlobalShortcutEnableEvent`, `unregisterGlobalShortcutEnableEvent`,
`setGlobalShortcutEnableEventCallback`, `setGlobalShortcutEventCallback`,
`isGlobalShortcutEnableEventRegistered`

**Macro recording**
`startMacroRecording`, `stopMacroRecording`, `hasMacroRecordingStarted`, `getMacroRecordingMode`,
`registerMacroRecorderEvent`, `setMacroRecorderEventCallback`, `stopRunningTurbosAndMacros`

**Audio (default devices + volume)**
`getDefaultMicrophoneDevice`, `getDefaultSpeakerDevice`, `setDefaultMicrophoneDeviceChangedCallback`,
`setDefaultSpeakerDeviceChangedCallback`,
`setMicrophoneVolume`, `setMicrophoneMute`, `microphoneVolumeStepUp`, `microphoneVolumeStepDown`,
`setSpeakerVolume`, `setSpeakerMute`, `speakerVolumeStepUp`, `speakerVolumeStepDown`,
`setMicrophoneVolumeChangedCallback`, `setSpeakerVolumeChangedCallback`,
`registerAudioServiceEvent`, `unregisterAudioServiceEvent`

**Misc**
`setMouseSensorRotationAngle`, `localStorageSetItem`, `localStorageDeleteItem`,
`localStorageDeleteAllItems`

**Mapping-engine → renderer push events** (`webContents.send`, `mapping_engine/win/index.js`):
`analogKeyEvent`, `defaultmicrophonedevicechanged`, `defaultspeakerdevicechanged`, `deviceEvent`,
`devicemodechanged`, `globalshortcutenableevent`, `globalshortcutevent`,
`globalshortcutunsupportedmappingevent`, `hardwareevent`, `inputMonitoringEvent`, `inputnotified`,
`inputredirect`, `joystickEvent`, `macroitem`, `macrorecordingstarted`, `macrorecordingstopped`,
`microphonevolumechanged`, `mousemoveredirect`, `snaptapKeyEvent` (Windows only),
`speakervolumechanged`, `unsupportedmapping`, `xboxGamepadEvent`.

---

## 12. `simpleServiceAction` (`modules/simple_service/win/index.js`)

Windows DLL `simple_service.dll`. 45 actions:

- **Lifecycle**: `simpleServiceInitialize`, `simpleServiceShutdown`, `ConfigureFFI`,
  `isSimpleServiceInitialize`
- **Process launching**: `simpleLaunchRazerApp`, `simpleLaunchRazerAppNoWait`,
  `simpleLaunchRazerAppElevated`, `simpleLaunchRazerAppElevatedNoWait`,
  `simpleLaunchUserAppProcess`, `simpleLaunchUserAppProcessNoWait`,
  `simpleLaunchUserAppElevated`, `simpleLaunchUserAppElevatedNoWait`,
  `simpleLaunchUserAppInAsciiFolder`
- **User apps**: `simpleGetUserApps`, `simpleAddUserAppFile`, `simpleRemoveUserApp`,
  `simpleRemoveUserAppFile`, `simpleRemoveUserAppDirectory`
- **Audio devices**: `simpleEnumerateAudioDevices`, `simpleGetDefaultMicrophone`,
  `simpleGetDefaultSpeaker`, `simpleSetDefaultAudioDevice`,
  `simpleGetMicrophoneVolume`, `simpleSetMicrophoneVolume`,
  `simpleGetSpeakerVolume`, `simpleSetSpeakerVolume`,
  `simpleGetSidetoneVolume`, `simpleSetSidetoneVolume`, `simpleSetSidetoneConfig`,
  `simpleRegisterAudioDeviceStateChangedEvent`, `simpleUnregisterAudioDeviceStateChangedEvent`
- **Audio events**: `simpleRegisterMicrophoneEvent`, `simpleUnregisterMicrophoneEvent`,
  `simpleRegisterSpeakerEvent`, `simpleUnregisterSpeakerEvent`
- **Apps-service events**: `registerAppsServiceEvent`, `unregisterAppsServiceEvent`,
  `setAppsServiceEventCallback`, `isAppsServiceEventRegistered`
- **Misc**: `simpleGetVersionInfo`
- **Push events**: `appsserviceevent`, `audioServiceEvent`

`electronAction{action:"updateControlPanelVersion"}` and `GetServiceStatus`/`StartService`/
`StopService` ultimately funnel into `simpleLaunchRazerApp` (`serviceFunction.js`).

---

## 13. `ffiPreload`, `ffiPreloadAsync`, `ffiSubPreloadAsync`

Generic dynamic-library bridge (`modules/ffi/FFIPreloadMain.js` and
`modules/ffi_subprocess/FFIPreloadSubProcess.js`). The renderer passes `(channel, action)` and the
main process dispatches into the DLL registered for that `channel` with `callDLL` / `callDLLAsync`.

| action | Behavior |
|---|---|
| `SetNodeFFIEvent` | Wire DLL callbacks back into Node |
| `ConfigureFFI` | Load/attach a DLL for a channel (`{channel, libPath, …}`) |
| `ConfigureFFI_APIToCallWhenExit` | Register an API to run at app exit |
| `SetAPIToCallWhenExitDevice` / `RemoveAPIToCallWhenExitDevice` | Per-device exit hooks |
| `SetAPIToCallWhenShutdown` / `RemoveAPIToCallWhenShutdown` | Shutdown hooks |
| `SetAPIToCallWhenSuspend` / `RemoveAPIToCallWhenSuspend` | Suspend hooks |
| `SkipFFILoggingAPI`, `SkipFFILoggingResultAPI`, `SkipFFILoggingEvent`, `SkipFFILoggingChannelEvent` | Log-noise suppression lists |
| `FreeFFI` | Unload |
| `future-implememtion` | Placeholder (typo is in the original) |

Direct evidence: `main.js` 49 247 (`ffiPreload`), 49 362 (`ffiSubPreloadAsync`);
`UsbRzDeviceAction.js` offsets 4 700 / 15 854 show the callers.
The sub-process variant runs inside an Electron `utilityProcess`
(`main.js` requires `./modules/ffi_subprocess` at start-up).

**Known FFI channels seen in the bundle:** `IoTSDKNative_Action` (IoT),
`rzLampArrayChannel` (LampArray). `SysUtilsNative.dll`, `mapping_engine.dll` and
`simple_service.dll` are attached through their own module classes rather than the generic channel API.

---

## 14. `keyStorageAction` (`keyStorage.js`)

App-wide, main-process-resident key/value store keyed by string; each key tracks a list of
subscribing **sender URLs** (`urlEventList`).

| action | Behavior |
|---|---|
| `setItem` | `{key,value}`; notifies every other window whose URL is in `urlEventList` with `keyStorageEvent{key,value,srcURL,keyState:"changed"}` |
| `getKeys` | All keys |
| `getItem` | `{key}` → value |
| `removeItem` | Delete + emit `keyState:"removed"` |
| `registerEvent` | `{key}` → add sender URL to the key's notify list (errors `key does not exists`) |
| `unregisterEvent` | Remove sender URL from the notify list |

Error strings live in `enumErrorMsg`: `missing arg.payload value`, `missing arg.payload key`,
`missing sender URL`, `key does not exists`.

---

## 15. `rzWssAction` (`WssAction.js`)

| action | Behavior |
|---|---|
| `start` | Start the plugin WebSocket server on `ws://localhost:5426/synapse` (port+1/+2 fallback); rejects unknown `clientName` with code `1008` |
| `stop` | Close every client, clear 30 s ping timers, close the server |
| `sendMessage` | `payload` is a JSON string `{pluginName, ...data}`; if `pluginName` is set the message goes to that one client, otherwise broadcast |

Main → renderer: every client event is re-emitted as `webSocketMessage` on all webContents with
`{eventType: "ws_connect"|"ws_message"|"ws_close"|"ws_error", clientName, requestId, data}`.

---

## 16. `nativeNotification`

`nativeNotification` handler (`main.js` 50 141) calls
`handleNativeNotification(ffiSysUtils, arg)`, which:
1. queries `isDarkMode` from sysutils and writes `payload.darkMode = 0|1`,
2. spawns `rzNotification.exe --toast=<JSON>` detached with `stdio:"ignore"`
   (`nativeNotificationHandler.js`).

Windows-only. It is **payload-driven, not action-driven** — there is no `action` switch.
Companion pieces:
- `getNotificationURI` → parses an `RazerAppEngine.://` / `razerappengine.://` URI out of argv.
- `fireNotificationURI` → pushes it to renderers as `notification-uri`;
  `app.on("open-url")` pushes `handle-notification-uri` instead.

---

## 17. `actionEnum`（constants.js 静态清单，offset 1 442）

| Constant | Value |
|---|---|
| `MINIMIZE` | `minimize` |
| `GET_WINDOW_SERVICE_CLIENTS` | `getWindowServiceClients` |
| `SET_BOUNDS` | `setBounds` |
| `SET_BROWSER_VISIBLE` | `setBrowserVisible` |
| `SET_BROWSER_ON_TOP` | `setBrowserAlwaysOnTop` |
| `CLOSE_WINDOW` | `closeWindow` |
| `GET_USER_APPS` | `getUserApps` |
| `EXIT_APPLICATION` | `exitApplication` |
| `CLOSE` | `close` |
| `RESTORE` | `restore` |
| `MAXIMIZE` | `maximize` |
| `OPEN_IN_DEFAULT_BROWSER` | `openInDefaultBrowser` |
| `GET_ALL_DISPLAYS` | `getAllDisplays` |
| `GET_DISPLAY_NEAREST_POINT` | `getDisplayNearestPoint` |
| `ACTIVE_WINDOW_SERVICE_CLIENT` | `activateWindowServiceClient` |
| `USER_DATA_DIR` | `userDataDir` |
| `GET_CURSOR_SCREEN_POINT` | `getCursorScreenPoint` |

**17 values.** Note `W.GET_MONITOR_INFO` is *used* in `main.js` (offset 39 810) but **is not defined**
in this enum — see §20.1.

---

## 18. Razer sub-applications and routes

### 18.1 Route table — `razerAppPathEnum` (`constants.js` offset 15)

| Constant | Route | Sub-app |
|---|---|---|
| `VIRTUAL_RING_LIGHT_PATH` | `/natalie` | Razer Virtual Ring Light |
| `SYNAPSE_PATH` | `/synapse/dashboard` | Razer Synapse |
| `CHROMA_PATH` | `/chroma-app/dashboard` | Razer Chroma |
| `STREAMER_COMPANION_PATH` | `/alisha` | Razer Streamer Companion App |
| `SPATIAL_AUDIO_PATH` | `/sophie` | Razer THX Spatial Audio |
| `CORTEX_PATH` | `/cortex` | Razer Cortex |
| `SURROUND_SOUND_PATH` | `/sophie-lite` | Razer 7.1 Surround Sound |
| `SYSTRAY_LEFT` | `/systray/systrayv2` | Left systray flyout |
| `SETTINGS` | `/settings` | Razer Settings |

All routes are appended to the base origin (`application-host`, default `https://apps.razer.com`)
with a trailing `/`.

### 18.2 `razerAppInfo` (`constants.js` offset 329)

| appName | folderName | commercialName (installer token) | Launched URL | Window options |
|---|---|---|---|---|
| `streamer-companion-app` | `StreamerCompanion` | Razer Streamer Companion App / `Razer_Streamer_Companion_App` | `<host>/alisha/` | 1280×720, min 600×500, policy 5, browser hidden |
| `virtual-ring-light` | `VirtualRingLight` | Razer Virtual Ring Light / `Razer_Virtual_Ring_Light` | `<host>/natalie/` | 255×255 fixed, policy 6, browser visible |
| `synapse` | `Synapse` | Razer Synapse / `Razer_Synapse` | `<host>/synapse/dashboard/` | 1280×720, min 600×500, policy 5, settings button hidden |
| `cortex` | `Cortex` | Razer Cortex / `Razer_Cortex` | `<host>/cortex/` | 1024×640, policy 6 |
| `sophie-lite` | `SurroundSound` | Razer 7.1 Surround Sound / `7.1_Surround_Sound` | `<host>/sophie-lite/` | 604×605 fixed, policy 5, tabs hidden |
| `spatial-audio` | `SpatialAudio` | Razer THX Spatial Audio / `THX_Spatial_Audio` | `<host>/sophie/` | 1004×725 fixed, policy 5 |
| `chroma-app` | `ChromaApp` | Razer Chroma / `chroma-app` | `<host>/chroma-app/dashboard/` | 1280×720, policy 5 |
| `razer-settings` | *(empty)* | Razer Settings / `Razer_Settings` | `<host>/settings/` | 1280×720, min 600×500, policy 5, browser visible |

**Non-product windows also created by `Ho()` (`main.js` offset **9 025**, body ≈ 9 025 – 11 300), useful for RE/debugging:**

| name | URL | Notes |
|---|---|---|
| `jstestrzdevice` | `<host>/jstestrzdevice/` | 1240×840, policy 0, browser visible — HID test harness |
| `jstestrzdevice2` | `<host>/jstestrzdevice2/` | 1024×640 |
| `jstestrzdevice-tab` | `http://localhost:3000/` | dev server |
| `jstestnatalie` | `<host>/jstestnatalie/` | Virtual Ring Light test page |
| `jstestgms` | `<host>/jstestgms/` | GMS test page |
| `cache` | `<host>/synapse/assets/html/cache.html?devices=<list>` | per-device cache priming |
| `razer-id` | `https://id.razer.com/` (or `https://id-staging.razer.com/` for staging hosts) | login window, 800×600, policy 6 — created by `LeftSystray.createLoginPage()` |
| `systray-left` | `<host>/systray/systrayv2/` | tray flyout, 300×200, always-on-top, skipped from taskbar |
| `lighting-engine` | *(internal)* | Chroma lighting engine window, closed last on quit |
| `default` | `http://localhost:<devport>` | only with `--devport` |

`razerAppInfo` is also the source of the tray menu: one item per app that is currently running
(icon from `<userData>/Apps/<folderName>/icon.ico`), plus Settings, Log In/Out, and
Exit / Exit All Apps. Labels are localised (`constants.js` `string_translation`, 12 locales; used by
`electronAction{action:"updateLang"}`).

---

## 19. Main → renderer push events（静态清单）

| Channel | Payload | Source |
|---|---|---|
| `window-list-change` | `{action: winMapChanged\|tabListChanged\|activeTabChanged\|close\|create, windowName}` | `TabManager.js` (offset ≈ 1 300) |
| `tab-message` | `{action: show-tooltip\|hide-tooltip\|add-tooltipStatus\|setShowTab\|request-close-tab\|manualClose\|userFocusTab\|changeTitle\|focusSubTab\|setTabPos\|setTabPosArr\|tabClosed\|activeTabChanged\|windowHibernate…}` | `components/Tab/common.js` |
| `async-message` | `{action:"appFocus", appName}` | `components/Tab/common.js` |
| `electron-action` | `{action:"logIn"\|"logOut"\|"click"\|"double-click"\|"align", payload}` | `main.js` (offset ≈ 30 500), `LeftSystray.js` |
| `highContrastEvent` | nativeTheme flags | `TabManager.initHighContrastEvent` |
| `razer-fallback-channel` | `{type:"initFallbackPage"\|"show-no-internet", data}` | `TabManager.getFallbackPageInfo` |
| `exitApp` / `exitAppRejected` | `{appName, runningApps}` / `{cannotExitAppNameList}` | `main.js` `quitApp` |
| `uninstallApp` | `{appName}` | `main.js` `second-instance` |
| `handle-notification-uri` / `notification-uri` | URI string | `main.js` `open-url`, `nativeNotificationHandler.js` |
| `suspend`, `resume`, `on-ac`, `on-battery`, `shutdown`, `lock-screen`, `unlock-screen` | *(none)* | `powerMonitor` wiring, `main.js` ≈ 30 000 |
| `memoryStorageEvent` | `{key,keyState,newValue,oldValue,srcURL}` | `modules/memory_storage` |
| `windowStorageEvent` | `{key,keyState,newValue,oldValue,srcURL}` | `modules/window_storage` |
| `keyStorageEvent` | `{key,value,srcURL,keyState}` | `keyStorage.js` |
| `nodeUSBEvent-add` / `nodeUSBEvent-remove` | USB descriptor | `UsbRzDeviceAction.initUSB` |
| `webSocketMessage` | `{eventType, clientName, requestId, data}` | `WssAction.handleClientMessage` |
| `hidhardwareevent` | raw HID event | `modules/hidHardwareEvents/index.js` |
| `events` | FFI event payload | `modules/ffi/FFIPreloadMain.js`, `modules/ffi_subprocess/index.js` |
| *mapping-engine events (22–23 names)* | see §11 | `modules/mapping_engine/win/index.js` |
| `appsserviceevent` / `audioServiceEvent` | see §12 | `modules/simple_service/win/index.js` |

Renderer → main one-way: `ipcRenderer.send("close-iframe")` from `preload.js` `pagehide`
(inside an iframe only).

---

## 20. Quirks and notable findings

**20.1 `getMonitorInfo` is a latent bug that works by accident.**
`preload.js` sends `{action: actionEnum.GET_MONITOR_INFO}`, but `actionEnum` has no
`GET_MONITOR_INFO` key, so `action === undefined`. In `main.js` the `switch` contains
`case W.GET_MONITOR_INFO:` (offset 39 810) which is also `undefined`, so it matches and returns
`screen.getAllDisplays().map(… → {deviceName,dpiScaleX,dpiScaleY,isPrimaryMonitor,monitorRect,workRect})`.
Any *new* caller that omits `action` entirely will silently hit the same branch.
Also note `getMonitorInfo` appears **twice** as a case label in the same switch (offset 39 810 via
the enum alias, 43 487 in the sysutil pass-through group); the first match wins, and the aliased
one is reachable only because both sides evaluate to `undefined`.

**20.2 `USER_DATA_DIR` (`userDataDir`) is declared in `actionEnum` but has no handler** in the
`electronAction` switch. The value is exposed to the renderer as the static property
`window.apiElectron.userDataDir` instead.

**20.3 `doDLLAction` / `doDLLActionAsync` are dead** — they only `console.error("… deprecated")`.
Anything calling them must be migrated to `doDLLMainAction` / `doDLLSubProcessActionAsync`.

**20.4 Single-instance + relaunch handshake go through log files**, not IPC:
`--exit-application` writes `Logs/exit-app-relaunch.log`, `--uninstall-app=` writes
`Logs/uninstall-relaunch.log`, `relaunchWithLauncher` writes `Logs/relaunch.log` — all `{apps,host}`,
re-read at start-up so the new process re-opens the same windows against the same origin.

**20.5 Quit is cooperative and multi-stage.** `quitApp()` sends `exitApp` to every renderer, waits
200 ms, asks the lighting engine to shut down with a 2 s poll (needs *both*
`lightingEngineCanShutdown` and `lightingEngineExitCompleted` for a clean exit), stops the WSS
server and BLE monitoring, then `exitApp()` closes windows with 10 s → 5 s → 5 s escalation to
`app.exit()`.

**20.6 Hang / veto mechanism:** `electronAction{action:"setCanExitAppEngine"}` maintains
`cannotExitAppNameList`; a quit is retried up to `cannotExitAppEngine < 2` times, after which the
veto is force-overridden. This is how unsaved profile/macro edits block exit.

**20.7 Hibernation** is a real subsystem: `HIBERNATE_STATUS = {NONE:1, ON_COUNTDOWN:2, HIBERNATED:3, CANNOT_HIBERNATE:4}`
(`constants.js` offset **8 750**), driven by `--hibernate-timeout`, with `WINDOW_STORAGE_KEYS.STOP_HIBERNATE="stop-hibernate"`
(`constants.js` offset **8 643**). `functionWarningTime = 10` (`constants.js` offset **8 712**).

**20.8 `AI_SERVICES`** (`constants.js` offset **8 838**): `CHATGPT:"chatgpt"`, `COPILOT:"copilot.microsoft"`,
`BAIDU:"chat.baidu"`; rendered in the window named `ai-services`, with a launcher component at
`electron/components/RzAiLauncher/`. This is the "Razer AI" entry point.

**20.9 Vendor ID `5426`** is hardcoded for both the WebSocket base port and the Razer USB filter
(`UsbRzDeviceAction.js` offset ≈ 1 200: `5426===e.vendorId`). Ports 5426–5428 on localhost should be
treated as Razer-owned when re-implementing.

**20.10 `windowEvent` constants** (`constants.js` offset **7 833**):
`winMapChanged`, `windowResized`, `tabListChanged`, `activeTabChanged`, `tabClosed`, `tabDestroyed`,
`commonDestroyed` (spelt `commonDestroyted` in the source — a typo that is part of the ABI),
`hibernated` (value `"hibernate"`).

---

## 21. 当前项目应实现的契约

### 21.1 按实际页面选择适配范围

本项目不需要为了“UI 替换”自动重写这里的每个 IPC channel。GPUI Kit 也不要求保留 Electron 的 channel 名；可以由 Rust 领域接口适配原服务或自行实现相同业务行为。是否兼容第三方 WebSocket、其它 Razer 应用或原宿主命令行，是独立范围，不能从三个产品页面推导。

| 页面需求 | 已确认原版边界 | Rust 实现要求 |
|---|---|---|
| 应用/产品导航 | app/window/tab 服务与实际根路由 | 应用身份、产品 route、历史和关闭分开 |
| Profile / 映射 | 前端 profile/mapping 数据、映射引擎、状态事件 | 草稿、保存/丢弃、profile 隔离与映射加载 |
| DPI / polling / Smart Tracking / Power | 产品 bundle 的具体 setter 与设备配置 | 保留父组件参数、连接/固件门槛、回读/失败 |
| Pairing | DUALLINK_BIND_INFO / SCAN_DEVICE / BIND_DEVICE / UNBIND_DEVICE / CANCEL | 以请求响应更新状态，保留单/双设备通道 |
| Sound / Mic | 独立 audioEq / micEq action | 预设、频率、自定义、设备差异确认彼此隔离 |
| Lighting | 产品 quick/advanced 数据、Chroma 安装/接管、engine 与 driver | 分清 effect ID、engine action、driver Configure JSON |
| Windows 属性/混合器 | 对应宿主系统操作 | 调用明确系统入口，报告失败 |
| 安装/固件入口 | 清单、安装进度、连接要求、外部更新器 | 不由按钮点击伪造完成 |

产品行为契约见逐页文档；硬件协议不能只按这一层 IPC 名称猜测。

### 21.2 传输与 DLL

USB/HID、BLE、serial、Wi-Fi、IoT、LampArray 等均在本包存在。**HID 不是本清单唯一的设备路径**。是否需要其中某条路径，由目标设备、连接和实际调用链决定；例如当前 777 配置已有 BLE 相关分支，不能一概将 BLE 排除。

mapping_engine、simple_service、sysutil、lighting driver 等是不同边界。JS 绑定能证明函数名/声明的参数形状，不能单凭此证明 Rust 可无条件复用：还要核对实际 DLL 版本、ABI、字符串分配/释放、回调生命周期、线程和初始化依赖。本轮没有执行这些验证。

当前 [backend/lighting.rs](../../src/backend/lighting.rs) 有绑定，普通 [lighting.rs](../../src/features/lighting.rs) 仍主要改变本地状态；命令行演示路径不等于 UI 端到端完成。[灯光协议](02-lighting-actions.md) 分开记录三个 ID/调用层级。

### 21.3 存储与确认

memory/window/keyStorage 的实现不能代表原版所有持久化：前端还使用 localStorage、profile 数据与其它服务。不能从这些宿主 Map 得出“原版没有保存功能”或“必须保存到唯一指定 JSON 格式”。

当前 [store.rs](../../src/store.rs) 将 Vec<Device> 写入 `%APPDATA%/razer_ui/profiles.json`，是本项目自有存储。它不自动保存 AppShell 的全部全局字段，不自动与原版 profile 兼容，也不证明设备已应用。

每项应分开报告：草稿更新、写本地文件、发出设备请求、收到确认/回读。失败、取消、断连、目标 profile 改变和旧响应都需要处理。

### 21.4 下一步验证

优先按 [差异表](03-implementation-gap.md) 修正实际路由和领域语义，再建立页面事件 → 领域操作 → 后端请求 → 响应/回读 → 页面反馈的可追踪链。协议枚举/静态字符串只能定位实现；真实设备成功必须另行验证。
