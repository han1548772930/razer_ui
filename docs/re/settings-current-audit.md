# 当前 Settings 源码审计

日期：2026-10-03。实现依据只取当前 [.ref/applications/synapse/settings/](../../.ref/applications/synapse/settings/)。[独立 GET 记录](settings-current-source-check.json) 确认当前入口、manifest、asset-manifest、main、720 JS/CSS 与本地字节一致。本轮不读取 `.ref/settings/` 作为实现依据。

核心文件：

- [720.1e5d1c8f.chunk.js](../../.ref/applications/synapse/settings/static/js/720.1e5d1c8f.chunk.js)，SHA-256 `09885484dcda19f25b9b216d575da5d2cbed2abe5678e6280c0011387be4ce64`。
- [720.dbc9cca5.chunk.css](../../.ref/applications/synapse/settings/static/css/720.dbc9cca5.chunk.css)，SHA-256 `986d72b2d006545b4805e158a6f20d10328b7328970ad8955b932afae6f3aafd`。
- [当前 Dashboard manifest](../../.ref/applications/synapse/dashboard/manifest.json)，SHA-256 `edf8fe27b5278326d15d91595ead50c8ecef9a757f713d5983f4046d9846fffe`；其线上版本来源核验见 [20-current-source-version.md](20-current-source-version.md)。

以下偏移是把当前720 JS以UTF-8读取后，定位表中字符串得到的零起始字符索引；用于定位原文，不替代具体render/调用分析。

| 定位字符串与字符偏移 | 本轮实际读取的行为 |
| --- | --- |
| `ho=` 215186 / `uo=` 215413 | Synapse 的 `Ks,bn,Ia` 左列，`Un,Ta,ia` 右列；General 的 `Te` 左列、`Fs` 右列 |
| `Te=` 26204 | 语言立即 `updateLanguage` 和 storage 写入；发行说明为 `RELEASE_PATCH_NOTE_WHATS_NEW` 中的 `Release Notes` 可点击片段，没有附加支持链接 |
| `Fs=` 77038 | 从 Dashboard manifest 拼接前端版本；当地当前年份版权、原商标译文、政策和社交入口 |
| `Ks=` 80201 | `RazerApp.getAppAutoStart/getMinimizedOnStartUp` 读值，`setAppAutoStart/setMinimizedOnStartUp` 写值；最小化禁用条件为 `!autoStart` |
| `bn=` 48463 | 更新通知时同时调用 `RazerApp.updateAppData` 与 `localStorage.setItem`，没有保存/丢弃按钮 |
| `Un=` 50168 | 重置后禁用重置按钮；恢复多个教程存储键；没有额外完成说明 |
| `Ia=` 88266 | 推荐开关和子项各自立即调用 `Oa` 写storage；RESET清空产品忽略/拥有记录，不清分类；子项外层随总开关disabled |
| `Ta=` 90994 | 查找并激活 `syn3-profile-migration`；不存在时 `window.open('/profile-migration/', name, 'policy=3,tab_visible=1,shouldFocus=1')` |
| `ia=` 84051 | OS build≥22631或build22621且revision≥2506显示；读取 `isDynamicLighting`，监听storage与 `wdl-devices.isSwitching`，调用Windows灯光设置 |

## 本轮修正

1. 删除正式设置页无原render依据的附加内容：底部保存/丢弃栏、教程重置后说明、General支持链接、About本项目版本说明。连接限制和本项目标识集中在用户保留的服务连接页。
2. 依据 `.style_checkGroup__f2JLx{display:flex;flex-direction:column;gap:10px;justify-content:center}` 将推荐设备从两列恢复单列。`style_title`为大写；`style_groupDesc`上5px下10px；`mg6/mg10/mg20`分别为下间距6/10/20px。重置使用 `.style_ignoreDevice__rodIQ button` 的灰底、27px高、水平27px内边距及fit-content宽度。
3. `Fs` 的版本公式是 `["4", ...manifest.version.split(".").slice(1), manifest.buildVersion].join(".")`。当前manifest的 `version=0.0.86`、`buildVersion=2609221012`，因此静态复原目标为 `4.0.86.2609221012`。这与宿主4.0.827、已安装宿主4.0.821和本crate版本不同。未实现运行时联网刷新该前端版本。
4. 版权按原 `new Date().getFullYear()` 语义在Windows读取当地年份。`windows-sys 0.61.2` 的 `GetLocalTime(*mut SYSTEMTIME)` 签名与 `SYSTEMTIME.wYear` 已核对，仅增加既有依赖的SystemInformation功能。非Windows返回空年份，未声称已实现其他平台当地日期。
5. 教程及迁移命令继续沿用原`.setting-block .thx-btn.test`尺寸与交互透明度；迁移正式入口发事件给Shell，样例从服务连接进入。
6. `ia` 的 `tips:le.j4s` 由共享widget的右上角`.help`渲染，不是额外正文。恢复14×14px帮助按钮、right/top各10px、原说明及tooltip样式；正式未知模式不预填Chroma或WDL。通知帮助也复用来源样式。CSS `.widget .help`常态背景`#4a4a4a`，`.widget .help:hover`为`#ffffff4d`；`.widget .tip`为黑底、`#5d5d5d`边框、14px字号/18px行高、8px/10px内边距、最大300px。`assets/synapse/onboard-help.svg` 与当前Dashboard资源 `tooltip_questionmark.96138d2f.svg` 字节一致，SHA-256均为 `efe866785ea43c2766e4af7e4136a3f0f140f92414f66c5de7edc19b0ae6c92e`。

## 仍未完成的原生或条件行为

这些项不能从本地偏好或预览推断为已实现：

| 当前源的具体行为 | 当前边界 |
| --- | --- |
| `Ks`读取和写入宿主自动启动状态，随后延迟发送 `storeData` | 正式控件禁用，未绑定原生宿主 |
| `bn`向 `RazerApp.updateAppData`提交桌面通知偏好；`Ia/Oa`监听/修改原版storage | 仅本项目本地偏好自动保存；尚无原版跨窗口同步和桌面通知提交 |
| `Un`还重置 `isShowTutorialMacro`、`isShowArmoryIntroductionBanner`、`isShowSensitivityTutorial` 等键，并遍历 `synapse_\d+`恢复 `showAIPromptMasterTutorial` / `showAppProfileAITutorial` | 当前Shell只联动已实现的设备介绍、Dashboard、Gamer Room，未建立整个原版教程集合 |
| `ia`读 `isDynamicLighting`、听storage及`wdl-devices`切换消息 | 本地状态预览具备Chroma/WDL/切换中分支；正式控制权未读取，切换禁用 |
| `Te`只有引擎≥4.0.633才广播 `releaseNotePatch`，等待 `settings`通道`turnOffLoading`复位 | 当前打开本地发行说明视图；引擎版本门槛、广播及真实内容回执尚未接入 |
| `Fs`联网刷新Dashboard manifest，失败后读取 `RazerApp.getAppData('synapse','version')` | 本轮按已核对清单静态显示目标版本；尚无运行时刷新/回退通道 |
| `Ta`通过宿主window-service激活迁移页；迁移应用再读取原生扫描/导入结果 | 本地Shell页签与预览可复原界面；原生扫描和导入不由设置页假造 |

原CSS帮助提示有相对卡片及body边界的重定位逻辑；GPUI提示使用组件定位，未运行窗口比对，不能据编译通过宣称浮层像素位置与浏览器完全一致。其他布局本轮也只做静态核对，不报告截图或交互测试结果。
