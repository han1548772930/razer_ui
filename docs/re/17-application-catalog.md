# 独立应用与模块入口

根据宿主、主前端和 Settings 的路径字面量逐项取得；未执行下载的 JS。路由不等于独立产品或已完成的界面。

原始响应及哈希：[application-catalog.json](application-catalog.json)。源文件、偏移及 SHA-256 保留在各项 evidence 中。

| 路由 | HTML | 清单中的 JS/CSS | JS/CSS 取得 |
| --- | --- | --- | ---: |
| [/alisha/](https://apps.razer.com/alisha/) | ok | 齐备 | 36/36 |
| [/background-manager/](https://apps.razer.com/background-manager/) | ok | 齐备 | 53/53 |
| [/chroma-app/dashboard/](https://apps.razer.com/chroma-app/dashboard/) | ok | 齐备 | 89/89 |
| [/chroma-app/settings/](https://apps.razer.com/chroma-app/settings/) | ok | 齐备 | 25/25 |
| [/cortex/](https://apps.razer.com/cortex/) | not_found | 待追踪 | 0/0 |
| [/feedback/](https://apps.razer.com/feedback/) | ok | 齐备 | 27/27 |
| [/natalie/](https://apps.razer.com/natalie/) | ok | 齐备 | 5/5 |
| [/profile-migration/](https://apps.razer.com/profile-migration/) | ok | 齐备 | 67/67 |
| [/rz-app-menu/](https://apps.razer.com/rz-app-menu/) | ok | 齐备 | 11/11 |
| [/rz-user-profile-menu/](https://apps.razer.com/rz-user-profile-menu/) | ok | 齐备 | 10/10 |
| [/settings/](https://apps.razer.com/settings/) | ok | 齐备 | 44/44 |
| [/sophie-lite/](https://apps.razer.com/sophie-lite/) | ok | 齐备 | 6/6 |
| [/sophie/](https://apps.razer.com/sophie/) | ok | 齐备 | 6/6 |
| [/synapse/](https://apps.razer.com/synapse/) | not_found | 待追踪 | 0/0 |
| [/synapse/alexa/](https://apps.razer.com/synapse/alexa/) | ok | 齐备 | 10/10 |
| [/synapse/armory/](https://apps.razer.com/synapse/armory/) | ok | 齐备 | 32/32 |
| [/synapse/dashboard/](https://apps.razer.com/synapse/dashboard/) | ok | 齐备 | 135/135 |
| [/synapse/introduction-tour/](https://apps.razer.com/synapse/introduction-tour/) | ok | 齐备 | 27/27 |
| [/synapse/macro/](https://apps.razer.com/synapse/macro/) | ok | 齐备 | 85/85 |
| [/synapse/profiles/](https://apps.razer.com/synapse/profiles/) | ok | 齐备 | 76/76 |
| [/synapse/settings/](https://apps.razer.com/synapse/settings/) | ok | 齐备 | 26/26 |
| [/synapse/update-fw/](https://apps.razer.com/synapse/update-fw/) | ok | 齐备 | 63/63 |
| [/systray/systrayv2/](https://apps.razer.com/systray/systrayv2/) | ok | 齐备 | 44/44 |

没有 asset-manifest 的入口仅能确认 HTML 声明的脚本；其动态 import、条件路由和原生服务仍须追踪。404 仅代表记录时该端点不可用。

`/rz-app-menu/` 来自主前端 `App.72827d47.chunk.js` 的 `${window.location.origin}/rz-app-menu/` 模板。发现脚本只提取静态路径，不执行模板或下载的代码。弹层结构、安装条件和 Alexa 启动路径见[更多应用规格](../screens/19-app-picker.md)。
