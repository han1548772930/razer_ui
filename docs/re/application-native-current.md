# 当前独立应用：原生与外部进程调用侧

本轮将全部 24 个目录端点、22 个有 HTML 的应用、755 个当前 JS 文件都纳入静态解析及原生边界检索，补充此前产品 middleware/host 库表之外的旧 FFILibrary、音频独立应用、更新器、区域选择及外部进程调用者。**全量扫描完成不等于 22 个应用的全部业务分支完成**。本节逐条说明已追到的实际 wrapper、初始化来源、返回消费及清理，余下动态目标和业务挂载边界单独标明。

证据来自 [专用 JSON](evidence/application-native-current-evidence.json.zip)，维护工具为 `node tools/audit-application-native-current.cjs --check`；原 [全应用入口/模块图](all-application-chains-current.md) 保留原范围，本工具没有改动其生成器。每个来源匹配当前应用目录与模块索引的 SHA-256，收据为实际完整 AST 源码切片，offset/end 使用 UTF-16 代码单元、end 不包含尾端。没有运行厂商 JS、应用、DLL、helper、构建或测试。

## 全量范围与计数的准确含义

扫描找到 33 个含显式边界的 JS、4789 个调用位置，其中包括863个 getProc、437个已按同模块 receiver 归属关联的旧 call、2504个 callDLLApi、410个 callDLLApiAsync、366个 _sendDLLAction。这些位置包含重复共享包装代码及不同 ABI 分支，不能当作4789个独立功能或4789个已证明可从应用根执行的行为。

旧桥有5个字面 `new window.FFILibrary(...)`，另39个原生 receiver 的别名构造位置；39中包含同一包装器的传入路径/默认路径重试和在多个 bundle 中的副本，不是39个新库。322个 `[returnType,args]` 形式的现代 FFI API 对象按原 AST 保存，不能仅依据这些声明认为 PE/调用约定/所有权正确。已人工追踪的语义锚点按 id 独立保存，完整 wrapper 保留 JSON decoder、catch、event assignment、unload listener。

| 应用端点 | JS | 旧构造位置（含别名） | getProc 声明 | doDLL* 桥调用 | 进程 API 调用 | 全部边界位置 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| /alisha/ | 31 | 1 | 17 | 0 | 5 | 41 |
| /background-manager/ | 53 | 0 | 0 | 4 | 9 | 36 |
| /chroma-app/dashboard/ | 83 | 6 | 123 | 4 | 8 | 708 |
| /chroma-app/settings/ | 24 | 0 | 0 | 0 | 5 | 5 |
| /cortex/ | 0 | 0 | 0 | 0 | 0 | 0 |
| /feedback/ | 25 | 0 | 0 | 0 | 5 | 5 |
| /natalie/ | 4 | 3 | 38 | 0 | 3 | 83 |
| /profile-migration/ | 66 | 6 | 123 | 4 | 3 | 738 |
| /rz-app-menu/ | 10 | 0 | 0 | 0 | 0 | 0 |
| /rz-user-profile-menu/ | 9 | 0 | 0 | 0 | 0 | 0 |
| /settings/ | 39 | 0 | 0 | 0 | 5 | 5 |
| /sophie-lite/ | 5 | 2 | 31 | 0 | 3 | 74 |
| /sophie/ | 5 | 2 | 39 | 0 | 3 | 239 |
| /synapse/ | 0 | 0 | 0 | 0 | 0 | 0 |
| /synapse/alexa/ | 9 | 0 | 0 | 0 | 1 | 1 |
| /synapse/armory/ | 28 | 0 | 0 | 0 | 0 | 0 |
| /synapse/chroma-studio/ | 47 | 0 | 0 | 2 | 2 | 16 |
| /synapse/dashboard/ | 123 | 6 | 123 | 4 | 10 | 712 |
| /synapse/introduction-tour/ | 26 | 0 | 0 | 0 | 0 | 0 |
| /synapse/macro/ | 82 | 6 | 123 | 4 | 4 | 706 |
| /synapse/profiles/ | 73 | 6 | 123 | 4 | 3 | 705 |
| /synapse/settings/ | 25 | 0 | 0 | 0 | 5 | 5 |
| /synapse/update-fw/ | 62 | 6 | 123 | 4 | 5 | 705 |
| /systray/systrayv2/ | 33 | 0 | 0 | 0 | 5 | 5 |

cortex、synapse 两个0 JS端点没有当前 HTML 页面源；其他0命中应用只是没有命中本工具明确的 FFI/进程 API 集。5个进程 API位置经常来自共享 RzWindow 包装器，不能解释为该页面实际启动5个进程。HTML外部框架脚本、USB/BLE、窗口服务、动态字符串与跨应用广播属于另外的边界。

全部755 JS中没有发现静态 child_process import/require；3个 `.spawn` 属于 XState actor 调用，已排除，不能说这些位置启动 OS 进程。Promise.spawn 的成员赋值同样不是进程启动。真实应用进程链以 simpleLaunchUserAppProcess/NoWait/Json、host simple service 与 job事件为依据。

## 旧 FFILibrary 与现代 Electron 桥必须分别记录

旧分支的实际形态是 `window.FFILibrary` 或解构别名 → 构造库 → setAllocator/setEventInterface → getProc(name,signature,"X64") → instance.call(name,...args)；回调多为 onffievent/onffiexception。getProc的 `b/c/v/d/i/...` 字符串是旧包装器的声明，不能直接替代现代 ffi-napi 的 bool/pointer/string/int 定义。

现代应用共享 helper 从 `window.top.apiElectron` 取桥，分别调用 doDLLAction、doDLLActionAsync、doDLLMainAction、doDLLMainActionAsync；helper 的 catch 通常仅 warn 后返回未赋值的局部变量。包装器再按 supportElectron20、DLL basename、channelName 和配置 payload 选择分支。**捕获异常后的 undefined、本地 false/空对象/默认值，不等于真实设备查询成功**。函数名包含 get/set 不作为本项目读取/写回许可，必须查看真实调用正文、初始化副作用、返回语义和宿主分派器。

当前宿主分派、main/subprocess、指针解码与 FreeMalloc 边界维护在 [当前宿主 FFI 语义](host-ffi-current.md)。现有当前 host preload 的现代 apiElectron不能自动证明页面旧 `window.FFILibrary` 已提供或适配；旧调用者的存在不代表当前宿主会执行那一支。本文只沿真实应用语法到边界，不执行兼容层。

## Alisha：Streamer Companion 更新器

`alisha/static/js/main.83ea24ca.js` 的模块50777保存 RzSparkle 实例包装，模块82544是真实动作调用者。锚点 `alisha-updater-path-input` 保存 `u = isWindowsOS() ? userDataDir + constants.W4 : constants.TL`，随后 `alisha-updater-config-and-actions` 将 u 传 init，依次配置 appcast、app details/当前版本、automatic check=false、canShutdown=false、start、relaunch path。constants引用保留在模块依赖中，不以相同函数名跨应用套用 DLL路径。

原 wrapper 的 init 在 try 内构造 FFILibrary、设置 allocator/event、声明17个RzSparkle入口并调用 Initialize；异常只记录message，init没有返回“已初始化成功”布尔值。check/perform/cancel/installer直接消费旧call返回值；terminate仅在 isCallAvailable（实例存在且 call 是函数）时调用 Terminate，没有在该方法中销毁实例或置空。事件将原 e 或 e.ffi 原样 emit，不统一 JSON.parse。

原 getter `isActive` 先检查 rzUpdater 的 call 可用，却实际调用 `this.thx.call("RzSparkle_IsActive")`；该类实例字段为 rzUpdater。这个错用对象必须保留为原代码异常，不得悄悄修正后宣称原代码一致。相同问题也存在下面 Lite 更新器副本。

## Sophie Lite：7.1 Surround Sound 与迁移更新器

`sophie-lite/static/js/main.bb22144c.chunk.js` 的 `ie` 是真实 SurroundSound 实例；入口在 render(root)前执行 `ba = isWindowsOS() ? userDataDir + L : U; ie.init(ba); et()`。常量正文给出 ThxNativeLite.dll 与 Apps/SurroundSound 路径，init参数锚点保存原表达式。不能用共享 THX v2 类的路径替代该独立应用。

`lite-audio-wrapper` 声明13个 THX 入口，Initialize 先于 allocator/event设置；isActive getter每次会调用 THX_IsActive。getDeviceListEx默认 `{Devices:[]}`，活跃时调用THX_GetDeviceList并JSON.parse，解析失败恢复同一默认对象；getOutputDevice默认-1，其他方法各有自己的fallback。setOutputDevice(deviceIndex,enabled)、processing与surround直接旧call，没有统一 ConfigureFFI。terminate仅活跃时调Terminate；upgradeDLL先Terminate，再调用库对象的upgrade（存在才调用），最后Initialize。它会改变库/处理状态，不能用来作只读验证。

`lite-updater-native-path` 将新版 updater定位 userDataDir/Apps/SurroundSound/RzSparkle.dll 或非Windows常量；`lite-updater-init-migration` 以 **init返回0** 判断失败/旧版本，再init另一fallback库。该 wrapper init成功返回1，失败返回0；GetVersions为可选getProc，缺失仅置isGetVersionCallAvailable=false。版本JSON辅助decoder失败走默认client/service版本，不是读取真实成功。迁移分支配置不同appcast及app版本，后续可terminate/reinit切换THX或RzSparkle的appcast。init成功与“升级过程已经完成”是不同状态。

## Sophie：Spatial Audio

`sophie/static/js/main.0dad7d2f.chunk.js` 中 `bo=new Mr`，Lo明确传 `window.userDataDir + Apps/SpatialAudio/ThxNativeFull.dll`，随后 `window.surroundSound=bo`。源 Mr 是 Babel函数类，构造器与prototype方法都已保留，不能因无ClassDeclaration而漏掉。其 init直接构造解构别名的FFILibrary，Initialize、allocator/event、21个核心/可选proc声明后设置 isInit=true，返回0。这里 **0表示结束值，而Lite更新器的0表示失败**；不可把不同wrapper统一为布尔结果。

THX回调将didChangeAudioStreams/didUpdateDevices/didChangeDefaultDevice映射为本地eventId再emit；入口消费getDeviceListEx的Devices并派发Redux，订阅对应事件刷新UI。Mr的getDeviceListEx只在call可用且THX_IsActive时调用GetDeviceList，非null结果直接JSON.parse；该方法没有Lite式catch恢复 `{Devices:[]}`。异常由上层调用者的try处理。terminate检查isInit/IsActive后调用Terminate并清isInit，不据此证明底层FFI对象已经释放。

更新器Kn与Lite的同名家族独立保存源码；其参数Jn根据宿主版本比较选择userDataDir/SpatialAudio路径或fallback，init返回0才进入旧版逻辑。appcast、版本、canShutdown、automatic-check、relaunch按独立源配置。不能把THX init的0语义迁移给Kn。

## Natalie：Virtual Ring Light 与 updater

核心 `Pa → ha.natalie = new Ca → ha.natalie.init()`，init缺省路径是 `window.userDataDir + Apps/VirtualRingLight/VirtualRingLight.dll`；传入路径和默认路径是两个别名FFILibrary构造位置。库存在后设置allocator/event、声明20个proc位置，再调用GetDLLVersion和Initialize；GetCameraList实际重复声明为b和c两种签名，不能只保留其中一条假装没有冲突。异常令isInit=false，正常令isInit=true。此core init没有本文此前其他设备链中的EnumDevices；显示器/设备枚举必须按本页实际GetMonitorList及窗口服务消费另追。Initialize属于生命周期入口，不能因为随后GetScreenBrightness等名称就认为整条init已证明无副作用。beforeunload调用terminate并取消自身监听，native实例状态也有清理。

本文重新保存核心init与factory的准确范围；页面挂载、monitor mismatch、UI timer与设置层细节复用 [Ring Light 当前审查](ring-light-ui-current.md) 和 [原收据](ring-light-current-evidence.json)。宿主getMonitorInfo/action enum及旧FFILibrary兼容断点仍按该证据保留，不伪造显示器编号。

另一个 updater实例Wa仍是RzSparkle，不是VirtualRingLight.dll；init参数Ya由宿主版本比较选择新路径，0返回触发fallbackCe，GetVersions可选，之后配置Razer appcast、appdetails、自动检查/关机旗标及relaunch。核心Ring Light读取与更新器进程不是同一个DLL会话。

## Chroma Studio：区域选择和取色的真实 native wrapper

`synapse/chroma-studio/static/js/main.6b22e9cc.js` 的 `studio-native-wrapper-configure-return-events-free` 是该应用专用包装，缺省 userDataDir/Apps/Synapse/chromaStudioNative.dll；channelName以basename附后缀。现代支持分支调用doDLLMainAction，旧支持分支调用doDLLAction，输入为 `{action:"ConfigureFFI",payload:{dllPath,apiObj}}`，apiObj声明版本、FreeMalloc、SetNodeFFIEvent、屏幕分辨率、区域窗口和取色入口。

只有返回result===true或直接boolean true才置isInit，并继续读取版本和注册FFI事件；异常仅warn、保留未初始化。现代SetNodeFFIEvent没有传JavaScript callback参数，旧桥payload.actionArgs为nodeFFICallback；不可将两套回调方式合并。

onRegionSelected/onRegionCancelled会先延迟0调用stopSelectRegion再emit；onColorPickerDone先closeColorPicker再emit，普通picker event只转发。native JSON callback直接JSON.parse，现代host response由isOnColorPicker/isOnRegionSelect限制。构造器现代response实际注册onColorPicker、onColorPickerDone、onRegionSelected，未见同处注册onRegionCancelled；它与handleEvent支持的事件集合不同。

terminate若创建过区域窗口先closeSelectedRegion；仅supportElectron20时调用FreeFFI，然后清isInit/lib/dllName。没有在此方法看到构造器response的取消订阅，也没有据此证明不同host版本的卸载效果一致。Show/Start/Stop与Close都会影响窗口或native状态，即使没有设备设置写回，也不能以get/set前缀替代权限判断。

## 跨应用共享产品包装：只能证明代码在场

Chroma Dashboard、Synapse Dashboard/Macro、Profiles、Profile Migration、Update FW都带大量THX/Nanoleaf/摄像头/Stream Mixer/Scarlett等共享module。证据保存了每个文件的全部getProc/call、现代API对象、完整class与同文件/跨文件incoming module引用；target_candidates保留同ID多来源歧义。**模块被import或被lazy登记，仍不能证明当前应用页面实例化所有产品类**。下面以当前Dashboard `2973.acc7b128.chunk.js` 的专用人工锚点说明包装本身的实际语义，其余副本保留各自source/hash，不复用旧symbol当重新审查。

- AudioEffectsTHXV2有initChromium/旧FFILibrary和initElectron/ConfigureFFI两支；userDataDir/Apps/Synapse默认目录、basename channel、ApiObj、callback与beforeunload均在wrapper内。AudioEffectsTHX的stream float-setting读取有JSON.parse；部分设置函数先SelectStream/SetFloatSetting再SendChanges，不能将这些包装器整体当读取库。各catch/fallback不同。
- Nanoleaf默认NanoleafNative.dll、InitNanoleaf后读取版本；事件JSON.parse(e.ffi)再转onEvent。Discover/Pair/UnPair、EnableSynapseControl、亮度与frame发送都在同一类；destroy先关闭BC再terminate，不能仅调用“GetPanelListInfo”就跳过初始化、副作用与生命周期说明。
- Stream Mixer/Chelsea/Camy类根据设备PID、container、native文件及supportElectron20建立现代channel。ConfigureFFI成功后有getVersion、SetNodeFFIEvent、DeviceInitialize等动作；Terminate与FreeFFI不是同义。本地connectDevice可直接返回 `{result:true,error:""}`，部分getSerialNumber可直接构造空字符串SUCCESS；这类源stub不能成为真实读取成功的依据。
- Loupedeck包装使用外部app/server、WebSocket与serial等路径；getUsbSerialFirmwareInfo有Loupedeck状态门控，openSerialPort成功才读取version/serial，并在finally closeSerialPort。其init传给chelseaDllDevice，不能解释成只用一个DLL。

摄像头/RzNative/Scarlett等其余完整wrapper已静态保存，但每个参数与所有页面调用者仍需对应产品逐支复核；没有据共享class数量宣称整应用的native语义完成。

## 外部进程：任务ID、退出码与清理

RzWindow共享simpleLaunchUserAppProcess(folder,file,params)在Electron中构造simpleServiceAction payload，返回其result；非Electron才调window.simpleLaunchUserAppProcess。NoWait与Json是另外的方法，不能用helper名猜返回结果。background-manager的Loupedeck服务也保留同样边界。

Alexa的 `alexa-process-launch-job-match-and-exit-consumption` 用Promise执行window.simpleLaunchUserAppProcess(folder,name,launchArgs)，先设置全局window.onappsserviceevent。onsimpleprocesslaunched按app/folder+filePath/name保存jobID；onsimpleprocessexited须匹配jobID，再根据其接受的exitCode集合resolve/reject。该调用侧没有在同一closure看到恢复旧global handler，不能把它说成取消订阅完整的event listener。主机权限、句柄、实际进程等待属于宿主dispatcher边界。

Update FW的 `firmware-unzip-helper-arguments-hash-result-and-cleanup` 从simpleGetUserApps的JSON列表定位 `folderName/pid/version.zip`，从远程Common资源索引取wrapper版本，参数来自当前device.productId与firmwareUpdateInfo.targetFWVersion，形如 `--fwupdate-unzip <pid>/<target>.zip`，helper文件名为动态 `<wrapper>_v<version>.exe`。给定expected SHA时先比较大小写归一后的hash；源另有不提供SHA仍调用helper的分支，不能在记录里删去。返回仅 `result && exitCode===0` 才令本地i为真；随后simpleRemoveUserAppFile清zip，移除错误单独catch。本文没有执行解压、删除、安装或固件写回。

## PE证据与当前未解边界

现有二进制静态结果见 [产品native PE](native-library-pe-current-evidence.json)、[当前host native PE](host-native-library-pe-current-evidence.json) 和 [DLL函数总表](dll-function-inventory.md)。本工具记录所有字面dll/dylib/exe/node候选与**精确basename** PE匹配；文件名匹配只是导航，不能证明加载的是同字节文件、更不能证明ABI、内部分配或硬件执行结果。动态版本命名保留未解表达式。

这两份PE表没有直接列出本节独立应用的RzSparkle.dll、ThxNativeLite.dll、ThxNativeFull.dll、VirtualRingLight.dll、chromaStudioNative.dll、NanoleafNative.dll同名资源。ThxV3/ThxV4Native、其他产品RzNative不能仅据“THX”名称代替它们；尚需对应当前应用资源包/版本与实际PE指纹。没有将未取得的binary标为验证成功，也没有声称从JS包装恢复C/C++原始工程。

本轮完整完成的是755文件的显式边界覆盖、source/hash验证、调用/声明/包装源码收据及上面列明的人工语义锚点。仍未完成全部现代共享类在每个应用route的真实实例化/初始化顺序、所有外部HTML框架入口、动态FFI库/资源包解析、所有callback取消/重连代际、每个DLL/helper内部语义和本地Rust逐项差异。现代host的subprocess/dispatcher另见宿主文档；设备/服务DLL写回继续后置，本地UI操作不能省略，也不能用原源stub冒充真实保存。

## 独立应用 DLL 的来源链：2026-10-09 元数据补查

这部分新增 [独立资源来源证据](application-resource-metadata-current-evidence.json) 和 [HTTP 原始正文及收据目录](application-resource-metadata-2026-10-09/)，不改变上面的755文件native边界证据。源码仍是2026-10-02冻结语料；2026-10-09只查询原代码明确引用的官方JSON/XML。线上清单的当前响应不代表其所指UI已经复核到新版本，也不代表它就是当前稳定DLL版本。本次未下载清单指向的EXE、DLL、ZIP或其他二进制包，没有执行安装器、源码或DLL。

### Background Manager 的资源清单不等于独立应用的 DLL 清单

当前 `background-manager/assets/index-8d39b3d5.js` 的 `qD` 是带 `cache-control:no-cache` 的GET/JSON helper。`aC` 读取 `/background-manager/installer-manifest.json`，按 `baseURL + latest.url` 获取实际清单；保存键为 `moduleManifest`，安装状态键为 `installedModules`，owner名称为 `background-manager`。它的 `systemOS` 原表达式是 `(navigator.userAgent.includes("Macintosh"),0)`，本审查保留始终0的语义，没有替换成另一个engine updater类的OS判断。

2026-10-09响应指定 `latest.version=1.0.136`、`baseURL=background-manager/manifests`、`latest.url=manifest-1.0.136.json`。第二级清单含24个resources；冻结的Background Manager HTML manifest中内嵌的 `resourceManifest.version` 同为1.0.136。两层JSON均保留原字节及HTTP收据。资源落盘检查正文使用 `userDataDir/Apps/{overwriteApp || "common"}/{path}/{action.saveToDisk.filePath}/{url basename}`；`runOnInstall` 项另按 `NormalDriver_`、`ExtraInstaller_`、`Skip_` 分别检查驱动、模块或直接视为已安装，不能统一成“DLL存在就是安装完成”。

这24个资源没有本节六个精确basename。原源还有独立engine升级清单（正式站点走源码规定的 `preprod/engine` 路径，见 [当前宿主来源](current-host-version-audit.md)），固件 `device.resourceDownloadAddress` 链和Haptic升级资源链；不能把这些不同owner清单当作ThxNativeLite/Full、VirtualRingLight等独立应用DLL的来源。

原源出现的 `/synapse/assets/index.json?filter=synapseAssets.json` 本次响应含114项，都是Web资源索引，没有 `.dll/.exe/.node/.zip` 条目。它能说明字体、图标与浏览器脚本的来源，不能替代native二进制资源清单。

### Appcast 常量必须追到实际切换调用

以下收据均保留完整AST节点和准确source/hash；offset单位是JavaScript UTF-16 code units。详细路径、原始参数和分支见本节JSON的 `receipts`，不能仅凭“常量定义过”认定调用发生。

| 当前应用与native owner | 源路径与真实appcast使用 | 2026-10-09只读元数据结果 | 尚未证明的二进制边界 |
| --- | --- | --- | --- |
| Alisha / RzSparkle | `Apps/StreamerCompanion/RzSparkle.dll`；constants导出 `W4→v`、`TL→g`、`WL→y`，入口把 `WL` 传给 `setAppCastURL` | `https://appcasts.razer.com/alisha/appcast.xml` 返回403，保留失败正文；无可解析appcast版本 | 未取得可核对的当前DLL包、PE版本或hash |
| Sophie Lite / RzSparkle | `Apps/SurroundSound/RzSparkle.dll`；`et`的init返回0走旧版fallback并选择 `k`（appcast-rzsparkle），正常分支选择主appcast | 主appcast item版本1.8、shortVersion1.8.0.0；sparkle appcast item版本1.0.13.0 | 这是升级feed条目，不能当成当前DLL的PE版本；未提取所指安装包 |
| Sophie Lite / ThxNativeLite | `Apps/SurroundSound/ThxNativeLite.dll`；`nt`在THX更新分支terminate/reinit更新器后传 `z`（appcast-thx），版本来自 `ie.getVersion()` | appcast-thx item版本1.0.1.0，enclosure为 `SurroundSoundSetupUpgrade_v1.0.1.8.exe` | feed版本与安装包文件名版本不同；没有实际DLL字节证据 |
| Sophie / RzSparkle、ThxNativeFull | updater路径经host版本判断；audio入口明确 `Apps/SpatialAudio/ThxNativeFull.dll`；`qn`选择主appcast，`$n`用 `Y` 切到appcast-thx，版本来自deviceReducer.dllVersion | 主item版本1.14.0.0，THX item版本1.0.7.0；实际XML和全部enclosure属性独立保存 | updater、UI、THX DLL、安装包四种版本不能合并 |
| Natalie / VirtualRingLight、RzSparkle | core默认 `Apps/VirtualRingLight/VirtualRingLight.dll`；updater另用该目录RzSparkle；`Ja`用 `we`主appcast，`Za`用 `Le` appcast-dll | 主、dll、rzsparkle三个端点均403；`Ue`保存为定义过的rzsparkle常量，本次未把它标成已追到切换调用 | 无可解析feed版本或包身份；不能由目录名称推断DLL版本 |
| Chroma Studio / chromaStudioNative | wrapper默认 `Apps/Synapse/chromaStudioNative.dll` | 当前Chroma Studio HTML manifest的resourceManifest为1.0.0且resources为空 | native路径已追到wrapper；尚未定位此basename当前可核对资源包 |
| Dashboard共享Nanoleaf / NanoleafNative | wrapper默认 `Apps/Synapse/NanoleafNative.dll`；wrapper在场与页面实际挂载分开记账 | 当前Dashboard HTML manifest的resourceManifest为1.0.2，只列WinLauncher；未列该DLL | 尚未定位此basename的当前包/PE指纹及全部根挂载分支 |

五份可解析XML各只有一个item，含2019/2020年的发布日期或releasedate。特别是Sophie主appcast的enclosure文件名为 `THXSpatialAudioSetupUpgrade_v1.0.0.1111Test.exe`，`pubDate`还是 `DDD, DD MMM YYYY HH:MM:SS TZ`；Sophie THX feed的channel.link指向Sophie Lite，而其实际获取URL是Sophie。这里原样记录这些不一致，不据此认定源链正确、包可安全安装或它就是最新稳定版本。没有请求这些enclosure URL。

本次检查7个冻结应用HTML manifests、刚取回的24资源Background Manager清单，以及已有product/host两份PE证据：六个basename均无匹配；批准当前资源目录内的文件路径检索也无同名binary。这个结论限定于这些已审资源，不表示整个磁盘不存在文件。ThxV3/ThxV4Native、ThxVADCarolNative与routingclient不能因THX名称相近而替代ThxNativeLite/Full。

复核工具：[静态资源来源审查](../../tools/audit-application-resource-metadata-current.cjs) 的 `--check` 验证21段源码收据、5个源文件hash、7个HTML manifest、12个HTTP响应正文及appcast摘要来源hash；[官方元数据读取工具](../../tools/fetch-application-resource-metadata-current.py) 限定官方HTTPS的JSON/XML端点，`--summarize`仅解析已保存XML。当前新增的完整结论是“真实源代码的资源选择与appcast切换边界已留证”；独立应用DLL内部仍需当前资源归属与真实PE字节后才能继续静态逆向，不能从上述JS/API声明重建并宣称已获得C/C++原工程。
