# 帮助、设备信息与恢复操作

帮助页同时承载设备身份、固件与软件版本、操作教程、支持链接和恢复操作。它不是一个统一的“恢复默认”接口：不同产品传入的 `resetObm`、连接方式、OLED 能力、音频重启和固件恢复参数会选择不同路径。

各产品实际挂载的 Help 组件、调用参数与完整组件字节位置见各族 `products/<PID>.md`。下列跨产品行为只适用于列出的原件；组件中存在某个条件分支，不表示该型号启用了该分支。

## 设备身份与版本从哪里来

PID 70、679、563、3594 和 2684 的当前通用 Help 组件在挂载时分别读取 UI、MW 与应用版本。UI 版本来自 `noscript#version` 的 JSON，并组合 `version.buildVersion`；保留 `VERSION_PLACEHOLDER` 的内容不作为有效版本。MW 版本来自宿主窗口存储；2684 按 URL 中的 `containerId` 及产品／dongle ID 选择记录，3594 的当前代码直接取返回数组第一项，二者不能混写为同一种关联规则。

应用版本来自本机 `apps` 记录中 `name === "synapse"` 的项，显示时把第一段替换为 `4`。固件、BIOS、EC 与 VBIOS 值则由组件 props 提供，不来自上述软件版本。更多信息按钮仅改变组件 `viewMore`，不发起固件查询。

通用组件将传入序列号保存为组件初值；复制调用 `navigator.clipboard.writeText`，立即显示已复制状态，2 秒后清除。原件没有等待剪贴板 Promise 的成功应答。带 Camo 分支时，序列号校验、重试请求和授权码属于另一条带账号／联网依赖的业务链；不能把它当作所有产品的序列号读取过程。

## 恢复确认弹窗实际触发什么

通用组件先打开确认框；取消关闭弹窗并恢复按钮，确认关闭弹窗、禁用按钮，再执行 `resetDevice`。PID 70／679／563／3594 使用 `resetObm && !isBle` 选择调用方传入的恢复函数，否则向 MW 发送 `ON_RESET_DEVICE`。PID 2684 额外判断产品类别：鼠标允许在相应 BLE 条件下使用 `resetObm`。这属于保留组件版本的真实差异。

`oledfactoryReset` 成立时，组件另外发送 `ON_OLED_FACTORY_RESET_DEVICE`。部分 OLED 分支的 `confirmResetOled` 再把调用方的 `oledResetInitialState` 转为 `ON_RESET_OLED` payload：

| UI 字段 | 提交字段与转换 |
| --- | --- |
| emote 图片 | `imageData = emote.src` |
| banner 图片开关／位置 | `imageState = enabled ? 1 : 0`；`imagePos = position === "top" ? 0 : 1` |
| banner 图片 | 保留 `imageData`，`imageIndex = 0` |
| media 信息／频谱开关 | 转成 `mediaState`／`visualizerState` 的 0、1 |
| system 幻灯、温度、时间 | `timeBetweenSlides → interval`、`temperatureUnit → tempFormat`，同时携带日期、时间格式和 slides |

这里的初始化对象由该产品调用方提供；不能从组件的字段转换推断所有产品使用同一套内容。`isNonSupportFactoryReset`、`isSupportResetOLED`、`isBle`、`firmwareResetProps` 与产品类别继续决定显示普通恢复面板、固件说明面板还是 OLED 确认内容。

通用 Help 在发出恢复动作后用 2 秒定时器恢复按钮。PID 1342 的音频 Help 使用独立 `resetAudio`：先设置 `isRestart`，1 秒后调用 props 的音频恢复函数，并清除该标志；它的设备恢复只调用传入的 `resetObm`，按钮也用 1 秒定时器恢复。**这些 UI 定时器表示界面等待时长，不能解释成设备应答或读回确认。** 实际恢复范围继续由对应产品 MW、任务和原生消费者决定。

## 教程与链接

教程按钮分别调用 `setShowInstructions(true)` 或 `changeAnalogTutorialStatus(true)`；不是设备恢复。固件更新提示向共享窗口频道发 `navigateDeviceAndModuleView`，由 Dashboard 接续导航；点击提示本身不执行固件刷写。

支持页、语言对应 PDF 和注册页属于原版在线入口。源组件通常使用 `masterGuide + lang + ".pdf"`，部分产品根据 edition 另选 PDF。Camo 分支还包括本机应用启动和在线授权码。它们在源码中实际存在，但账号、在线支持与远程更新编排位于本项目本地优先范围之外；本机设备信息、教程、恢复及本地文件动作要分别理解。

## 系统信息的两个布局

当前完整读取链覆盖 PID 694、695、696、709、710、711、736、737。694／695／696 的调用方选择 basic；709／710／711／736／737 选择 image。包内保留两个组件不表示两个都挂载。

basic 展开后列出设备名、CPU、RAM、序列号、型号、OS、前两个 Display 名称及软件信息。image 上部展示设备图与 CPU／GPU／RAM，展开区遍历全部 Display，显示 OS 和软件信息；NVIDIA 项在两个候选软件字段都存在时优先取 `NVIDIANvApp`。各布局的空字符串及“未安装”展示文本是具体字段的原版呈现规则，不是整个系统查询失败后的替代结果。

MW 的能力分支调用 `getBladeDisplayData()`，解析返回 JSON、保存 `systemDetails` 并发布对应 UI action；reducer 把 payload 留在 `systemInfo`。读取异常的 catch 只记录错误。页面的展开动作仅切换显示状态。

原生报告返回 JSON 中的 `Display` 键可以是 `null`；每项的 `Name` 和原版拼写 `Manufactuer` 需按 producer 理解。显示查询、驱动查询、固件查询与全量保存是不同导出，不应拿显示页已取得的数据冒充完整报告。

## 保存完整系统报告

点击下载发送 `ON_STORE_SYSTEM_INFO_DATA`，进入状态机和 TaskMaker。当前 TaskMaker 使用 color-profile 命名的队列／版本结构；这是原件真实复用关系，不能按日志名字把它归为灯光设置。

执行器再次调用 `getAllBladeInfo()`，解析 JSON 后以两空格缩进保存。建议文件名为 `systemDetails.DeviceName + " - " + Date.now() + ".synapse4"`，对话框标题为 `Save as`，按钮为 `Save file`，过滤器为 `Synapse 4 (*.synapse4)`。

宿主按消息发送者定位 BrowserWindow，默认目录使用已记住的目录或系统 Downloads；取消不更新目录。取得路径后调用 `fs.writeFile`，写入错误只在回调中输出。这个宿主 case 没有把对话框及写盘回调完整 await 为设备任务结果，因此原版执行器的 SUCCESS 日志不能证明文件已写完。文档保留这条实际链，不补出源码没有的事务确认。

官方本地 wrapper 把原生字符串复制为 JS 字符串后通过 `FreeMalloc` 释放。`Terminate`、`TerminateWhenExit` 及 DllMain 是另一条原生生命周期链；已有捕获保存其函数与交叉引用，仍有间接和下层 helper 边未闭合，不能据此宣称整个 DLL 已理解完成。

## 证据入口与阅读边界

| 已直接核对的所有者 | 当前原件与 UTF-8 字节区间 |
| --- | --- |
| 通用 Help，PID 70 | [main.8f24b6a1.js](../../../.analysis/inputs/frontend/mouse-70-current/raw/apps.razer.com/synapse/products/70/ui/static/js/main.8f24b6a1.js)，`[5667140,5686347)` |
| 通用 Help，PID 679 | [main.194d8a7b.js](../../../.analysis/inputs/frontend/keyboard-679-current/raw/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js)，`[7728572,7747842)` |
| 音频 Help，PID 1342 | [main.2623357c.js](../../../.analysis/inputs/frontend/audio-1342-current/raw/apps.razer.com/synapse/products/1342/ui/static/js/main.2623357c.js)，`[7311062,7315670)` |
| 系统 basic／image 组件 | [main.f0afa1aa.js](../../../.analysis/inputs/frontend/system-694-current/raw/apps.razer.com/synapse/products/694/ui/static/js/main.f0afa1aa.js)，`[8459274,8461357)`／`[8470784,8473047)` |
| 系统 UI reducer | 同一原件，`[7832194,7832902)` |
| 完整报告 TaskMaker／执行器 | [6737.43877d4945b570a8510a.js](../../../.analysis/inputs/frontend/system-mw-694-current/raw/apps.razer.com/synapse/products/694/mw/6737.43877d4945b570a8510a.js)，`[939263,939979)`／`[1506934,1508211)` |
| 宿主保存 case | [electron/main.js](../../../.analysis/inputs/host-current/asar/b2ce8c54dc5c/electron/main.js)，`[51081,51562)` |

系统信息全部 8 个产品的 UI、MW、native wrapper、原生函数及版本对应关系保存在 [local-audit.json](../../../.analysis/results/host-current/local-audit.json) 的 `help_system_info`。该报告同时含本项目实现说明，不能把那些说明当作官方行为。本文仅采用经原件核对的源码部分。

全产品恢复的具体字段、设备命令和恢复后事件仍需逐产品接续；本文件不把通用 Help 按钮闭合等同于全产品恢复协议闭合。本次仅静态阅读，未执行页面、DLL、文件对话框或恢复写入。
