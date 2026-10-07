# Chroma Dashboard 当前界面契约

当前来源为 `.ref/applications/chroma-app/dashboard/`，主导航23322/Ks、Dashboard62296/Wn、Apps77778/Se、Modules40554/K及65596/P5。`ChromaPage` 持有本地视图偏好；真实SDK应用、安装状态、固件与immersive能力均不可从本地目录推造。

## 根、导航与本地保存

`ChromaWindow` 是 `chroma-app` policy5独立宿主；Dashboard、Studio、Chroma Settings在其子页管理中使用各自实际根。Studio身份为 `chroma-studio`、`/synapse/chroma-studio/`、policy3，已有retained StudioSession及本地草稿保存；不能再把Dashboard当作Studio。Settings身份为 `settings-chroma`、`/chroma-app/settings/`，已有独立Chroma/General内容，不进入Synapse Settings。

导航高48/黑底边2、28px胶囊、12/14px文字、间隔20、圆角14；背景300ms和文字100ms按当前CSS。页面历史、介绍横幅、预设分组、Devices/Modules/Apps的实际内容入口均已接入，尚不表示各服务分支完整。

本地Preferences保存错误通过统一save_preferences保留，实际根显示chroma-preferences-error及重试；失败不回退会话编辑，成功重试清除错误。该路径仍普通文件写入，未原子替换；损坏偏好载入仍回默认，是尚存可靠性边界，不是官方服务同步。

## Dashboard预设与分组

- 七个Quick Effects及末尾Chroma Apps使用120×130卡、48图、12px大写标签、10间隔。Advanced Effects需要真实全局Chroma profile目录，不用产品本地profile冒充。
- `ChromaPreset::matches` 按实际effect及color1/color2/direction/isRandom/duration比较；源Cs绿色为#00ff00、Starlight时长2、Wave方向2、Wheel方向1。Apply只修改已支持产品的本地lighting设置，点击本身不代表SDK/设备同步成功。
- 预设激活底#111/边#5d5d5d；可应用提示显示实际支持的本地设备名或No Device。没有SDK状态时不生成spinner、Chroma Apps启动或3000ms警告回执。
- 分组保留子树，max-height/translation/箭头为300ms linear，结束恢复overflow。设备卡名允许多行、edition_name显示12/14灰字；validDevices>4时最大2460，否则1220，≤1279宽910并取消居中；卡290×245及20间距参与高度。
- AppIntroductionBanner独立于卡片宽上限，按1220–2500/min-height531、42/24/14文字和body max1020使用原图、箭头和真实tour入口。

## Modules与Apps

Modules按当前四项目录绘制Chroma Connect、Chroma Studio、Visualizer、Sensa，1220组/80行/40图/16字/27×90操作区及当前中文特例名称。Studio按钮打开实际编辑器；其余模块的完整编辑器/安装链仍未实现。目录不等于服务available/recentModules，无记录时不显示虚构日期、大小、固件或进度。

Apps已有onboard、SDK总开关及自动排序/显示禁用等偏好位置；总开关仍禁用，真实应用列表/优先级拖排/启停UI未接。偏好保存和SDK应用状态分开。

## 剩余项与资源

分组重排/吸附/reflow、第三方/Sensa/immersive分组、设备battery/effect色块、SDK非空/错误状态、Advanced Effects目录仍缺。Ss帮助top24/left8、随光标300px提示、backdrop blur30等精确UI未完成。灰色/hover动画SVG仍有15项当前原图缺口，离线指纹未找到匹配，不能用新绘图宣称一致。

工具栏的完整服务状态、所有Chroma Settings教程消费者、服务查询和设备写回分别保留缺口。具体Studio画布/图层/属性与保存边界见[Studio契约](studio-ui-current.md)；Settings见[当前Settings](chroma-settings-current-audit.md)。独立功能实现的存在不等于Dashboard全部状态或完整产品已验收。

## 保留证据

- [应用组件/语言](chroma-app-current-evidence.json)：`audit-chroma-app.cjs`。
- [导航/预设/分组/模块源](chroma-review-fixes-2026-10-05-evidence.json)：`audit-chroma-review.cjs`。
- [资源摘要](chroma-assets-current-evidence.json)、[缺失原图离线指纹](current-media-offline-recovery-chroma.json)：`prepare-chroma-assets.py`、`recover-current-media.cjs`。
- [介绍横幅](app-introduction-banner-current-evidence.json)、[Studio根](chroma-studio-source.json)、[Studio宿主](chroma-studio-window-source.json)、[Settings](chroma-settings-current-evidence.json)。

保留原JSON名称与静态事实，不改名伪装重新审计。取证时native哈希和编译通过不是视觉/输入/设备验收。未运行应用、测试、安装器、下载JavaScript或DLL。
