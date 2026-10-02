# 官方目录、连接别名与产品界面清单

更新日期：2026-10-02。由保存的 HTTP 响应、资源清单和入口 AST 生成；没有执行下载的 JS、驱动或安装器。

六份官方目录合计 449 个主产品 ID，与旧记录合并为 450 个主候选；沿连接别名及 manifest 中的明确声明共检查 596 个 ID。其中 331 个有 UI 入口。别名仍保留父产品关系，不计为独立型号。

已取得 30658 份清单所列 JS/CSS，331 个产品清单齐备；331 个产品找到明确写入 state.navs 或作为 JSX navs 传入的数组，包括入口、displayMode 和 setupStatus 延迟根组件。记录保留每条解析依据、条件表达式、调用偏移、根入口和 chunk/module ID；Babel 类及 useMemo 分支也按实际绑定追踪。未解析的名称和条件界面继续保留，不能按产品类别猜页面。

机器可读记录：[product-catalog.json](product-catalog.json)。原始响应及错误体在 `.ref/discovery`，代码在 `.ref/devices`。资源哈希是本次下载指纹，不是发布方数字签名。

## 来源

| 目录 | 原始条目数 | HTTP | SHA-256 |
| --- | ---: | ---: | --- |
| [AvailableDevices.json](https://apps.razer.com/synapse/dashboard/AvailableDevices.json) | 330 | 200 | `428b43ac965095a26d3033e7e4043966eebe05138a3fcd7087e7fe00281be511` |
| [Synapse2Devices.json](https://apps.razer.com/synapse/dashboard/Synapse2Devices.json) | 164 | 200 | `89eeea48a23b9bfd9b8df9dc41fa7514e21171d848dc1fa3138dfee636ad2a36` |
| [Windows10Devices.json](https://apps.razer.com/synapse/dashboard/Windows10Devices.json) | 2 | 200 | `ec5364cf7245af813be3589e2435084d0a215b1db67cffdc505079f561d62086` |
| [XboxDevices.json](https://apps.razer.com/synapse/dashboard/XboxDevices.json) | 6 | 200 | `5eb5b767af17780bc1c7c95a55b7eef79ab5309b897fb3886dd3306807da3833` |
| [XboxHeadsets.json](https://apps.razer.com/synapse/dashboard/XboxHeadsets.json) | 8 | 200 | `ddf9a219639c74fbebef0bcfcccc2d3597e719de3c6880f34d3d948c65dc3cc5` |
| [inDevelopmentDevices.json](https://apps.razer.com/synapse/dashboard/inDevelopmentDevices.json) | 0 | 200 | `4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945` |

## 全部候选及实际入口

同一导航组合不表示控件、弹窗、服务、配色或布局相同。下表只列明确挂载的导航；附属界面和业务完成度仍以逐页审计为准。404 表示记录时此路径不可用，不表示产品不存在。

| ID | 原名称 | 来源目录／别名 | UI | JS/CSS | 实际导航 | Rust |
| ---: | --- | --- | --- | ---: | --- | --- |
| 7 | Razer DeathAdder | Synapse2 | not_found | 0/0 | — | 未适配 |
| 19 | Razer Orochi | Synapse2 | not_found | 0/0 | — | 未适配 |
| 20 | Razer Orochi Wireless | Synapse2 | not_found | 0/0 | — | 未适配 |
| 21 | Razer Naga | Synapse2 | not_found | 0/0 | — | 未适配 |
| 22 | Razer DeathAdder 3.5G | Synapse2 | not_found | 0/0 | — | 未适配 |
| 31 | Razer Naga Epic | Synapse2 | not_found | 0/0 | — | 未适配 |
| 32 | Razer Abyssus 1800 (Cyclosa Bundle) | Synapse2 | not_found | 0/0 | — | 未适配 |
| 33 | Razer Naga Epic Gaming Dock | Synapse2 | not_found | 0/0 | — | 未适配 |
| 36 | Razer Mamba 2012 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 37 | Razer Mamba 2012 Gaming Dock | Synapse2 | not_found | 0/0 | — | 未适配 |
| 41 | Razer DeathAdder 3.5G Black | Synapse2 | not_found | 0/0 | — | 未适配 |
| 42 | Star Wars™: The Old Republic™ Gaming Mouse by Razer | Synapse2 | not_found | 0/0 | — | 未适配 |
| 43 | Star Wars™: The Old Republic™ Gaming Keyboard by Razer | Synapse2 | not_found | 0/0 | — | 未适配 |
| 44 | Star Wars™: The Old Republic™ Gaming Headset by Razer | Synapse2 | not_found | 0/0 | — | 未适配 |
| 46 | Razer Naga 2012 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 47 | Razer Imperator 2012 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 49 | Star Wars™: The Old Republic™ Gaming Mouse by Razer | Synapse2 | not_found | 0/0 | — | 未适配 |
| 50 | Razer Ouroboros | Synapse2 | not_found | 0/0 | — | 未适配 |
| 51 | Razer Ouroboros Gaming Dock | Synapse2 | not_found | 0/0 | — | 未适配 |
| 52 | Battlefield 4 Razer Taipan | Synapse2 | not_found | 0/0 | — | 未适配 |
| 53 | Razer Krait 2013 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 54 | Razer Naga Hex | Synapse2 | not_found | 0/0 | — | 未适配 |
| 55 | Razer DeathAdder 2013 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 56 | Razer DeathAdder 1800 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 57 | Razer Orochi 2013 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 60 | Razer Abyssus 1800 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 62 | Razer Naga Epic Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 63 | Razer Naga Epic Chroma Dock | Synapse2 | not_found | 0/0 | — | 未适配 |
| 64 | Razer Naga (Best Buy) | Synapse2 | not_found | 0/0 | — | 未适配 |
| 65 | Razer Naga Hex V2 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 66 | Razer Abyssus | Synapse2 | not_found | 0/0 | — | 未适配 |
| 67 | Deus Ex Razer DeathAdder Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 68 | Razer Mamba RGB | Synapse2 | not_found | 0/0 | — | 未适配 |
| 69 | Razer Mamba RGB Dock | Synapse2 | not_found | 0/0 | — | 未适配 |
| 70 | Razer Mamba TE | Available | 有入口 | 67/67 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 72 | Razer Orochi RGB | Synapse2 | not_found | 0/0 | — | 未适配 |
| 73 | Razer Turret Mouse | Synapse2 | not_found | 0/0 | — | 未适配 |
| 74 | Razer Turret Dock | Synapse2 | not_found | 0/0 | — | 未适配 |
| 76 | Razer Diamondback RGB | Synapse2 | not_found | 0/0 | — | 未适配 |
| 77 | Razer DeathAdder Pro | Synapse2 | not_found | 0/0 | — | 未适配 |
| 78 | Razer Taipan 3500 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 79 | Razer DeathAdder SC | Synapse2 | not_found | 0/0 | — | 未适配 |
| 80 | Razer Naga Hex V2 | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 83 | Razer Naga Chroma | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 84 | CrossFire Razer DeathAdder 3500 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 86 | Razer Orochi RGB Dock | Synapse2 | not_found | 0/0 | — | 未适配 |
| 89 | Razer Lancehead | Available | 有入口 | 67/67 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 90 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 91 | Razer Abyssus V2 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 92 | Razer Deathadder Elite | Available | 有入口 | 69/69 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 94 | Razer Abyssus 2000 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 95 | Razer DeathAdder 2000 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 96 | Razer Lancehead TE | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 97 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 98 | Razer Atheris | Available | 有入口 | 126/126 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 99 | Razer Jugan | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 100 | Razer Basilisk | Available | 有入口 | 98/98 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 101 | Razer Basilisk | Available | 有入口 | 130/130 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 103 | RAZER NAGA TRINITY | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 104 | Razer Mamba Hyperflux | Available | 有入口 | 83/83 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 105 | Razer Mamba Hyperflux | Available | 有入口 | 83/83 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 106 | D.VA Razer Abyssus Elite | Available | 有入口 | 67/67 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 107 | Razer Abyssus Essential | Available | 有入口 | 55/55 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 108 | Razer Mamba Elite | Available | 有入口 | 56/56 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 110 | Razer DeathAdder Essential | Available | 有入口 | 56/56 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 111 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 112 | Razer Lancehead Wireless | Available | 有入口 | 80/80 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 113 | Razer DeathAdder Essential | Available | 有入口 | 70/70 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 114 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 115 | Razer Mamaba Wireless | Available | 有入口 | 93/93 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 116 | Razer Abyssus Lite | Available | 有入口 | 67/67 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 117 | Razer Turret Mouse Xbox One Edition | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 118 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 119 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 120 | Razer Viper | Available | 有入口 | 103/103 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP / HELP | 未适配 |
| 122 | Razer Viper Ultimate | Available | 有入口 | 82/82 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 123 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 124 | Razer DeathAdder V2 Pro | Available | 有入口 | 60/60 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 125 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 126 | Razer Mouse Dock | Available | 有入口 | 98/98 | TAB_LIGHTING / HELP | 未适配 |
| 128 | Razer Pro Click | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / HELP | 未适配 |
| 130 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 131 | Razer Basilisk X Hyperspeed | Available | 有入口 | 110/110 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 132 | RAZER DEATHADDER V2 | Available | 有入口 | 87/87 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 133 | Razer Basilisk V2 | Available | 有入口 | 69/69 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 134 | Razer Basilisk Ultimate | Available | 有入口 | 70/70 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 136 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 138 | Razer Viper Mini | Available | 有入口 | 79/79 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 140 | Razer Deathadder V2 Lite | Available | 有入口 | 64/64 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 141 | Razer Naga Left Handed Edition | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 142 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 143 | RAZER NAGA PRO | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 144 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 145 | Razer Viper 8Khz | Available | 有入口 | 68/68 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 146 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 147 | RAZER NAGA CLASSIC EDITION | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 148 | Razer Orochi V2 | Available | 有入口 | 230/230 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 149 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 150 | Razer Naga X | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 152 | Razer DeathAdder Essential | Available | 有入口 | 84/84 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 153 | Razer Basilisk V3 | Available | 有入口 | 71/71 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 154 | Razer Pro Click Mini | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / HELP | 未适配 |
| 155 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 156 | Razer DeathAdder V2 X Hyperspeed | Available | 有入口 | 97/97 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 157 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 158 | Razer Viper Mini Signature Edition | Available | 有入口 | 60/60 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / HELP；TAB_PAIRING | 未适配 |
| 159 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 161 | Razer Deathadder V2 Lite | Available | 有入口 | 64/64 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 162 | cobra | 历史探测 | 有入口 | 64/64 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 163 | Razer Cobra | Available | 有入口 | 74/74 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 164 | Razer Mouse Dock Pro | Available | 有入口 | 9/9 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP；TAB_PAIRING | 未适配 |
| 165 | RAZER VIPER V2 PRO | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 166 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 167 | RAZER NAGA V2 PRO | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_SCROLLING / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 168 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 169 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 170 | Razer Basilisk V3 Pro | Available | 有入口 | 62/62 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP；TAB_PAIRING | 未适配 |
| 171 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 172 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 175 | Razer Cobra Pro | Available | 有入口 | 58/58 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 176 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 177 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 178 | Razer DeathAdder V3 | Available | 有入口 | 67/67 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_CALIBRATION / HELP | 未适配 |
| 179 | HyperPolling Wireless Dongle | Available | 有入口 | 7/7 | TAB_CUSTOMIZE / HELP | 未适配 |
| 180 | Razer Naga V2 Hyperspeed | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 181 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 182 | Razer DeathAdder V3 Pro | Available | 有入口 | 118/118 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / HELP；TAB_PAIRING | 已有适配 |
| 183 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 184 | Razer Viper V3 HyperSpeed | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / HELP；TAB_PAIRING | 未适配 |
| 185 | Razer Basilisk V3 X Hyperspeed | Available | 有入口 | 101/101 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP；TAB_PAIRING | 未适配 |
| 186 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 190 | Razer DeathAdder V4 Pro | Available | 有入口 | 53/53 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / ADVANCED / HELP；TAB_PAIRING | 未适配 |
| 191 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 192 | Razer Viper V3 Pro | Available | 有入口 | 72/72 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / ADVANCED / HELP；TAB_PAIRING | 未适配 |
| 193 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 194 | Razer DeathAdder V3 Pro Hyperpolling Technology | Available | 有入口 | 82/82 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 195 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 196 | Razer DeathAdder V3 HyperSpeed | Available | 有入口 | 51/51 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / ADVANCED / HELP；TAB_PAIRING | 未适配 |
| 197 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 199 | Razer Pro Click V2 Vertical Edition | Available | 有入口 | 79/79 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 200 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 201 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 203 | Razer Basilisk V3 35K | Available | 有入口 | 67/67 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 204 | Razer Basilisk V3 Pro 35K | Available | 有入口 | 58/58 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP；TAB_PAIRING | 未适配 |
| 205 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 206 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 207 | HyperFlux V2 Wireless Charging System | Available | 有入口 | 10/10 | TAB_CUSTOMIZE / HELP；TAB_PAIRING | 未适配 |
| 208 | Razer Pro Click V2 | Available | 有入口 | 76/76 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 209 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 210 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 211 | Razer Basilisk Mobile | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 212 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 213 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 214 | Razer Basilisk V3 Pro 35K Phantom Green Edition | Available | 有入口 | 58/58 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP；TAB_PAIRING | 未适配 |
| 215 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 216 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 218 | Razer Cobra HyperSpeed | Available | 有入口 | 56/56 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 219 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 220 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 221 | Razer Boomslang 20th Anniversary Edition | Available | 有入口 | 91/91 | TAB_PAIRING；TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 222 | Razer Viper V3 Pro SE | Available | 有入口 | 61/61 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / ADVANCED / HELP；TAB_PAIRING | 未适配 |
| 223 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 224 | Razer Orochi V2 | Available | 有入口 | 97/97 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / HELP | 未适配 |
| 225 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 226 | Razer Basilisk V4 Pro | Available | 有入口 | 82/82 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / ADVANCED / HELP | 未适配 |
| 227 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 228 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 229 | Razer Viper V4 Pro | Available | 有入口 | 59/59 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / ADVANCED / HELP | 未适配 |
| 230 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 231 | RAZER NAGA V3 PRO | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 232 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 233 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 234 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 235 | Razer Basilisk V4 HyperSpeed | Available | 有入口 | 92/92 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 236 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 239 | Razer DeathAdder V4 Pro Carbon Fiber Edition | Available | 有入口 | 57/57 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_POWER / TAB_CALIBRATION / ADVANCED / HELP | 未适配 |
| 240 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 241 | Razer Mouse Dock V2 Pro | Available | 有入口 | 28/28 | TAB_PAIRING / TAB_LIGHTING / HELP | 未适配 |
| 265 | Razer Lycosa | Synapse2 | not_found | 0/0 | — | 未适配 |
| 267 | Razer Arctosa | Synapse2 | not_found | 0/0 | — | 未适配 |
| 269 | Razer BlackWidow Ultimate | Synapse2 | not_found | 0/0 | — | 未适配 |
| 270 | Razer BlackWidow | Synapse2 | not_found | 0/0 | — | 未适配 |
| 271 | Razer Anansi | Synapse2 | not_found | 0/0 | — | 未适配 |
| 272 | Razer Cyclosa | Synapse2 | not_found | 0/0 | — | 未适配 |
| 273 | Razer Nostromo | Synapse2 | not_found | 0/0 | — | 未适配 |
| 275 | Razer Orbweaver | Synapse2 | not_found | 0/0 | — | 未适配 |
| 276 | Razer DeathStalker Ultimate | Synapse2 | not_found | 0/0 | — | 未适配 |
| 278 | Razer Blade | Synapse2 | not_found | 0/0 | — | 未适配 |
| 280 | Razer DeathStalker | Synapse2 | not_found | 0/0 | — | 未适配 |
| 281 | Razer DeathStalker Essential | Synapse2 | not_found | 0/0 | — | 未适配 |
| 282 | Battlefield 4 Razer BlackWidow Ultimate | Synapse2 | not_found | 0/0 | — | 未适配 |
| 283 | Razer BlackWidow 2013 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 284 | Razer BlackWidow TE 2014 Edition | Synapse2 | not_found | 0/0 | — | 未适配 |
| 285 | Razer Blade 14 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 286 | Razer Edge KB | Synapse2 | not_found | 0/0 | — | 未适配 |
| 287 | Razer DeathStalker Essential Lite | Synapse2 | not_found | 0/0 | — | 未适配 |
| 513 | Razer Tartarus | Synapse2 | not_found | 0/0 | — | 未适配 |
| 514 | Razer DeathStalker LC | Synapse2 | not_found | 0/0 | — | 未适配 |
| 515 | Razer Blackwidow Chroma | Available | 有入口 | 210/210 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 516 | Razer DeathStalker Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 517 | Razer Blade Stealth | Synapse2 | not_found | 0/0 | — | 未适配 |
| 518 | Razer Turret Keyboard | Synapse2 | not_found | 0/0 | — | 未适配 |
| 519 | Razer Orbweaver Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 520 | Razer Tartarus Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 521 | Razer Blackwidow TE Chroma V2 | Available | 有入口 | 181/181 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 525 | Razer Cynosa Pro | Synapse2 | not_found | 0/0 | — | 未适配 |
| 526 | Razer Cynosa | Synapse2 | not_found | 0/0 | — | 未适配 |
| 527 | Razer Blade 4 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 528 | Razer Blade Pro Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 529 | Razer Blackwidow Chroma | Available | 有入口 | 157/157 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 532 | Lenovo BlackWidow Ultimate by Razer | Synapse2 | not_found | 0/0 | — | 未适配 |
| 533 | Razer Core | Synapse2 | not_found | 0/0 | — | 未适配 |
| 534 | Razer Blackwidow X Chroma | Available | 有入口 | 207/207 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 535 | Razer BlackWidow Ultimate X | Synapse2 | not_found | 0/0 | — | 未适配 |
| 536 | Razer BlackWidow X Ultimate SE | Synapse2 | not_found | 0/0 | — | 未适配 |
| 537 | Razer BlackWidow X | Synapse2 | not_found | 0/0 | — | 未适配 |
| 538 | Razer BlackWidow X TE Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 539 | Razer BlackWidow TE X | Synapse2 | not_found | 0/0 | — | 未适配 |
| 542 | Razer Ornata Chroma | Available | 有入口 | 173/173 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 543 | Razer Ornata | Synapse2 | not_found | 0/0 | — | 未适配 |
| 544 | Razer Blade Stealth 2 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 545 | Razer BlackWidow Chroma V2 | Available | 有入口 | 191/191 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 548 | Razer Blade 6 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 549 | Razer Blade Pro 2 Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 550 | Razer Huntsman Elite | Available | 有入口 | 255/255 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 551 | Razer Huntsman | Available | 有入口 | 220/220 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 552 | Razer Blackwidow Elite | Available | 有入口 | 238/238 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 554 | Razer Cynosa Chroma | Available | 有入口 | 152/152 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 555 | RAZER TARTARUS V2 | Available | 有入口 | 109/109 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 556 | Razer Cynosa Chroma Pro | Available | 有入口 | 112/112 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 557 | Razer Blade Stealth 3 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 559 | Razer Blade Pro 2.5 Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 562 | Razer Blade Stealth 4 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 563 | Razer Blade 15 | Available | 有入口 | 177/177 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 564 | Razer Blade Pro 17 | Available | 有入口 | 123/123 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 565 | Razer Blackwidow Lite | Available | 有入口 | 158/158 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 567 | Razer Blackwidow Essential | Available | 有入口 | 161/161 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 569 | Razer Blade Stealth 13 | Available | 有入口 | 124/124 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 570 | Razer Blade 15 | Available | 有入口 | 178/178 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 571 | Razer Blade 15 | Available | 有入口 | 137/137 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 574 | Turret Keyboard Xbox One Edition | Available | 有入口 | 102/102 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 575 | Razer Cynosa Lite | Available | 有入口 | 155/155 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 576 | Razer Blade 15 | Available | 有入口 | 136/136 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 577 | Razer BlackWidow | Available | 有入口 | 172/172 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 579 | Razer Huntsman Tournament Edition | Available | 有入口 | 148/148 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 580 | Tartarus Pro | Available | 有入口 | 124/124 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 581 | Razer Blade 15 | Available | 有入口 | 177/177 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 582 | Razer Blade 15 | Available | 有入口 | 172/172 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 584 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 585 | Razer Pro Type | Available | 有入口 | 103/103 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 586 | Razer Blade Stealth 13 | Available | 有入口 | 147/147 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 587 | Razer Blade 15 Advanced Model | Available | 有入口 | 136/136 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 588 | Razer Blade Pro 17 | Available | 有入口 | 134/134 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_PERFORMANCE / HELP | 未适配 |
| 589 | Razer Blade 15 Studio Edition | Available | 有入口 | 173/173 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 590 | Razer BlackWidow V3 | Available | 有入口 | 273/273 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 591 | Razer BlackWidow X: Tenkeyless | Available | 有入口 | 119/119 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 592 | Razer Huntsman Essential | Available | 有入口 | 111/111 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 593 | Razer Pokémon | Available | 有入口 | 128/128 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 594 | Razer Blade Stealth 13 Base Model | Available | 有入口 | 123/123 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 595 | Razer Blade 15 Advanced Model | Available | 有入口 | 155/155 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 597 | Razer Blade 15 Base Model | Available | 有入口 | 218/218 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 598 | Razer Blade Pro 17 | Available | 有入口 | 134/134 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 599 | Razer Huntsman Mini | Available | 有入口 | 218/218 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 600 | Razer BlackWidow V3 Mini HyperSpeed | Available | 有入口 | 153/153 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 601 | Razer Blade Stealth 13 | Available | 有入口 | 137/137 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 602 | Blackwidow V3 Pro | Available | 有入口 | 168/168 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 603 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 604 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 605 | Razer Ornata V2 | Available | 有入口 | 189/189 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 606 | RAZER CYNOSA V2 | Available | 有入口 | 149/149 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 614 | Razer Huntsman V2 Analog | Available | 有入口 | 149/149 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 616 | Razer Blade 15 Base Model | Available | 有入口 | 126/126 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 617 | Razer Huntsman Mini | Available | 有入口 | 182/182 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 618 | Razer Book 13 | Available | 有入口 | 155/155 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 619 | Razer Huntsman V2 Tenkeyless | Available | 有入口 | 246/246 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 620 | Razer Huntsman V2 | Available | 有入口 | 186/186 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 621 | Razer Blade 15 Advanced Model | Available | 有入口 | 126/126 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 622 | Razer Blade Pro 17 | Available | 有入口 | 128/128 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 623 | Razer Blade 15 Base Model | Available | 有入口 | 126/126 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 624 | Razer Blade 14 | Available | 有入口 | 124/124 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 625 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 626 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 630 | Razer Blade 15 Advanced Model | Available | 有入口 | 161/161 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 631 | Razer Pro Type Ultra | Available | 有入口 | 116/116 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 632 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 633 | Razer Blade 17 | Available | 有入口 | 124/124 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 634 | Razer Blade 15 Base Model | Available | 有入口 | 128/128 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 635 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 642 | Razer Huntsman Mini Analog | Available | 有入口 | 127/127 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 647 | Razer BlackWidow V4 | Available | 有入口 | 198/198 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 650 | Razer Blade 15 Advanced Model | Available | 有入口 | 157/157 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 651 | Razer Blade 17 | Available | 有入口 | 157/157 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 652 | Razer Blade 14  | Available | 有入口 | 160/160 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 653 | Blackwidow V4 Pro | Available | 有入口 | 212/212 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 已有适配 |
| 654 | Razer Cynosa Pro | Available | 有入口 | 95/95 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 655 | Razer Ornata V3 | Available | 有入口 | 159/159 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 656 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 657 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 658 | Razer DeathStalker V2 Pro | Available | 有入口 | 129/129 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 659 | Razer BlackWidow V4 X | Available | 有入口 | 374/374 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 660 | Razer Ornata V3 | Available | 有入口 | 147/147 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 661 | Razer DeathStalker V2 Pro | Available | 有入口 | 202/202 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 662 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 663 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 664 | Razer DeathStalker V2 Pro Tenkeyless | Available | 有入口 | 196/196 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 669 | Razer Blade 14  | Available | 有入口 | 150/150 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 670 | Razer Blade 15 | Available | 有入口 | 126/126 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 671 | Razer Blade 16 | Available | 有入口 | 158/158 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 672 | Razer Blade 18 | Available | 有入口 | 156/156 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 673 | Razer Ornata V3 | Available | 有入口 | 171/171 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 674 | Razer Ornata V3 | Available | 有入口 | 150/150 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 675 | Razer Ornata V3 Tenkeyless | Available | 有入口 | 180/180 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 677 | Razer BlackWidow V4 75% | Available | 有入口 | 130/130 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 678 | Razer Huntsman V3 Pro | Available | 有入口 | 160/160 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 679 | Razer Huntsman V3 Pro Tenkeyless | Available | 有入口 | 149/149 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 680 | 未提供名称 | Available | not_found | 0/0 | — | 未适配 |
| 688 | Razer Huntsman V3 Pro Mini | Available | 有入口 | 153/153 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 689 | RAZER HUNTSMAN V3 X TENKEYLESS | Available | 有入口 | 117/117 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 691 | Blackwidow V4 Pro 75% | Available | 有入口 | 369/369 | TAB_CUSTOMIZE / TAB_LIGHTING / OLED / TAB_POWER / HELP | 未适配 |
| 692 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 693 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 694 | Razer Blade 14 | Available | 有入口 | 300/300 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 695 | Razer Blade 16 | Available | 有入口 | 301/301 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 696 | Razer Blade 18 | Available | 有入口 | 300/300 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 697 | Razer BlackWidow V3 Mini HyperSpeed | Available | 有入口 | 99/99 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 698 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 699 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 706 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 707 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 708 | Razer Pro Type Ergo | Available | 有入口 | 127/127 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 709 | Razer Blade 14 | Available | 有入口 | 318/318 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 710 | Razer Blade 16 | Available | 有入口 | 333/333 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 711 | Razer Blade 18 | Available | 有入口 | 294/294 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 713 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 714 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 716 | Razer BlackWidow V4 Low-profile HyperSpeed | Available | 有入口 | 118/118 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 717 | Razer Joro | Available | 有入口 | 127/127 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 718 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 719 | Razer Huntsman V3 Pro 8KHz | Available | 有入口 | 166/166 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 720 | Razer Huntsman V3 Pro Tenkeyless 8KHZ | Available | 有入口 | 197/197 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 721 | Razer Huntsman V3 Pro Mini 8KHz | Available | 有入口 | 148/148 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 722 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 723 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 724 | BlackWidow V4 Low-profile Tenkeyless HyperSpeed | Available | 有入口 | 130/130 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 725 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 726 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 727 | Razer BlackWidow V4 Tenkeyless HyperSpeed | Available | 有入口 | 99/99 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP；TAB_PAIRING | 未适配 |
| 728 | Razer Huntsman Signature Editon | Available | 有入口 | 136/136 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 730 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 736 | Razer Blade 16 | Available | 有入口 | 336/336 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 737 | Razer Blade 18 | Available | 有入口 | 143/143 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_DISPLAY / TAB_SOUND / TAB_BATTERY / TAB_LIGHTING / HELP | 未适配 |
| 739 | Razer Reclusa X Mini 65% | Available | 有入口 | 111/111 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 740 | Razer Huntsman V3 HE Magnetic Mini 65% 8KHz | Available | 有入口 | 140/140 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 741 | Razer Huntsman V3 Tenkeyless 8KHZ | Available | 有入口 | 181/181 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 742 | RAZER HUNTSMAN V3 PRO LOW-PROFILE TENKEYLESS 8KHZ | Available | 有入口 | 156/156 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 746 | Razer Huntsman V3 HE Magnetic Tenkeyless 8KHz | Available | 有入口 | 135/135 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / TAB_CALIBRATION / HELP | 未适配 |
| 747 | Tartarus Pro | Available | 有入口 | 125/125 | TAB_CUSTOMIZE / ACTUATION / TAB_LIGHTING / HELP | 未适配 |
| 752 | Razer BlackWidow V4 75% XBOX 25 Anniversary Edition  | Available | 有入口 | 120/120 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 769 | RAZER PHILIPS HUE | Available | 有入口 | 7/7 | HOME / HELP | 未适配 |
| 777 | RAZER KRAKEN BT SANRIO LIMITED EDITION | Available | 有入口 | 46/46 | TAB_SOUND / TAB_MIC / TAB_LIGHTING / TAB_POWER / HELP | 已有适配 |
| 778 | ASRock B550 Taichi Razer Edition | Available | 有入口 | 73/73 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 780 | Razer Key Light Chroma | Available | 有入口 | 6/6 | TAB_LIGHTING / HELP | 未适配 |
| 781 | Razer Aether Lamp Pro | Available | 有入口 | 8/8 | TAB_LIGHTING / HELP | 未适配 |
| 782 | Razer Aether Lamp | Available | 有入口 | 8/8 | TAB_LIGHTING / HELP | 未适配 |
| 783 | Razer Aether Light Bulb | Available | 有入口 | 8/8 | TAB_LIGHTING / HELP | 未适配 |
| 784 | Razer Aether Light Strip | Available | 有入口 | 8/8 | CUSTOMIZED / TAB_LIGHTING / HELP | 未适配 |
| 790 | RAZER AETHER MONITOR LIGHT BAR | Available | 有入口 | 8/8 | TAB_LIGHTING / HELP | 未适配 |
| 791 | Razer Aether Standing Light Bars | Available | 有入口 | 8/8 | TAB_LIGHTING / HELP | 未适配 |
| 1281 | Razer Kraken 7.1 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1282 | Razer Kraken USB | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1283 | Razer Seiren | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1284 | Razer Kraken 7.1 Chroma | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1286 | Razer Kraken 7.1 Blue | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1287 | Razer Kraken USB Crossfire | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1288 | Razer Seiren Pro | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1293 | Razer Kraken 7.1 Special Edition | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1295 | Razer ManOWar 7.1 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1296 | Razer Kraken 7.1 V2 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1300 | Razer Electra V2 USB | Synapse2 | not_found | 0/0 | — | 未适配 |
| 1303 | Razer Nommo Chroma | Available | 有入口 | 12/12 | TAB_SOUND / TAB_LIGHTING / HELP | 未适配 |
| 1304 | Razer Nommo Pro | Available | 有入口 | 23/23 | TAB_SOUND / TAB_LIGHTING / HELP | 未适配 |
| 1306 | RAZER NARI ULTIMATE | Available | 有入口 | 34/34 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1308 | RAZER NARI | Available | 有入口 | 12/12 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1310 | RAZER NARI ESSENTIAL | Available | 有入口 | 10/10 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1312 | RAZER USB AUDIO CONTROLLER | Available | 有入口 | 7/7 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_DEMO / HELP | 未适配 |
| 1313 | Razer Kraken Kitty Edition | Available | 有入口 | 16/16 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_DEMO / HELP | 未适配 |
| 1318 | Razer Kraken X USB | Available | 有入口 | 9/9 | TAB_SOUND / TAB_LIGHTING / HELP | 未适配 |
| 1319 | RAZER KRAKEN ULTIMATE | Available | 有入口 | 11/11 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_DEMO / HELP | 未适配 |
| 1320 | RAZER BLACKSHARK V2 PRO | Available | 有入口 | 14/14 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1321 | Razer USB Sound Card | Available, Windows10 | 有入口 | 11/11 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_DEMO / HELP | 未适配 |
| 1322 | Razer Odessa T1 | Windows10 | not_found | 0/0 | — | 未适配 |
| 1324 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1325 | RAZER KRAKEN V3 PRO | Available | 有入口 | 12/12 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_POWER / DEMO / HELP | 未适配 |
| 1328 | Razer Kraken BT Kitty Edition | Available | 有入口 | 46/46 | TAB_SOUND / TAB_MIC / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1330 | RAZER LEVIATHAN V2 | Available | 有入口 | 194/194 | TAB_SOUND / TAB_EQ / TAB_LIGHTING / HELP | 未适配 |
| 1331 | RAZER KRAKEN V3 HYPERSENSE | Available | 有入口 | 11/11 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_DEMO / HELP | 未适配 |
| 1332 | RAZER KRAKEN BT SANRIO LIMITED EDITION | Available | 有入口 | 46/46 | TAB_SOUND / TAB_MIC / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1335 | Razer Kraken V3 X | Available | 有入口 | 23/23 | TAB_SOUND / TAB_LIGHTING / HELP | 未适配 |
| 1337 | RAZER BARRACUDA PRO 2.4 | Available | 有入口 | 55/55 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1338 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1339 | Razer Barracuda 2.4 | Available | 有入口 | 14/14 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1340 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1342 | Razer Audio Mixer | Available | 有入口 | 87/87 | TAB_CUSTOMIZE / TAB_MIXER / TAB_MIC / EFFECTS / TAB_LIGHTING / HELP | 未适配 |
| 1346 | Razer Seiren V2 Pro | Available | 有入口 | 28/28 | MIC / STREAM_MIXER_HEADER / HELP | 未适配 |
| 1347 | Razer Seiren V2 X | Available | 有入口 | 28/28 | MIC / STREAM_MIXER_HEADER / HELP | 未适配 |
| 1352 | Razer Leviathan V2 Pro | Available | 有入口 | 29/29 | TAB_SOUND / TAB_EQ / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1353 | RAZER KRAKEN V3 | Available | 有入口 | 11/11 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_DEMO / HELP | 未适配 |
| 1354 | Razer Leviathan V2 X | Available | 有入口 | 22/22 | TAB_SOUND / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1356 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1364 | Razer Kitty V2 Pro | Available | 有入口 | 13/13 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_DEMO / HELP | 未适配 |
| 1365 | RAZER BLACKSHARK V2 PRO | Available | 有入口 | 9/9 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1368 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1369 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1370 | RAZER NOMMO V2 PRO | Available | 有入口 | 8/8 | TAB_SOUND / TAB_EQ / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1371 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1372 | RAZER NOMMO V2 | Available | 有入口 | 8/8 | TAB_SOUND / TAB_EQ / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1373 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1374 | RAZER NOMMO V2 X | Available | 有入口 | 7/7 | TAB_SOUND / TAB_EQ / TAB_POWER / HELP | 未适配 |
| 1376 | Razer Kraken Kitty V2 | Available | 有入口 | 48/48 | TAB_SOUND / TAB_MIC / TAB_LIGHTING / HELP | 未适配 |
| 1378 | Razer Kraken Kitty V2 BT Quartz Edition | Available | 有入口 | 28/28 | TAB_SOUND / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1381 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1382 | Razer WIRELESS CONTROL POD | Available | 有入口 | 81/81 | TAB_CUSTOMIZE / HELP | 未适配 |
| 1383 | Razer Kraken V4 Pro | Available | 有入口 | 293/293 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_HAPTICS / TAB_OLED / TAB_LIGHTING / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1384 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1386 | Razer Seiren V3 Mini | Available | 有入口 | 83/83 | MIC / STREAM_MIXER_HEADER / HELP | 未适配 |
| 1387 | Razer Kraken V4 | Available | 有入口 | 53/53 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1388 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1389 | Razer Kraken V4 X | Available | 有入口 | 67/67 | TAB_SOUND / TAB_MIC / TAB_LIGHTING / HELP | 未适配 |
| 1390 | RAZER BLACKSHARK V2 HYPERSPEED | Available | 有入口 | 9/9 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1391 | Razer Seiren V3 Chroma | Available | 有入口 | 92/92 | MIC / STREAM_MIXER_HEADER / TAB_LIGHTING / HELP | 未适配 |
| 1392 | RAZER CLIO | Available | 有入口 | 181/181 | TAB_SOUND / SURROUND / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1394 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1395 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1396 | Razer Barracuda X Chroma | Available | 有入口 | 23/23 | TAB_SOUND / TAB_EQ / TAB_MIC / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1397 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1398 | Razer BlackShark V3 Pro | Available | 有入口 | 79/79 | TAB_SOUND / TAB_CALIBRATION / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1399 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1400 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1401 | Razer BlackShark V3 | Available | 有入口 | 74/74 | TAB_SOUND / TAB_CALIBRATION / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1402 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1403 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1404 | Razer BlackShark V3 X Hyperspeed | Available | 有入口 | 21/21 | TAB_SOUND / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 1405 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1406 | Razer Kraken V2 BT Hello Kitty and Friends Edition | Available | 有入口 | 60/60 | TAB_SOUND / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1407 | Razer \| Sanrio Characters Limited Edition Wireless Headset | Available | 有入口 | 60/60 | TAB_SOUND / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1411 | Razer Barracuda Pro | Available | 有入口 | 72/72 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1412 | Razer Barracuda 2.4 | Available | 有入口 | 31/31 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1415 | Razer Kraken Kitty V3 Pro | Available | 有入口 | 78/78 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_LIGHTING / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1416 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1417 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1420 | Razer Hammerhead V3 | Available | 有入口 | 15/15 | TAB_SOUND / TAB_MIC / HELP | 未适配 |
| 1422 | Razer Seiren V3 Pro | Available | 有入口 | 28/28 | TAB_EQ / TAB_EFFECTS / STREAM_MIXER_HEADER / TAB_LIGHTING / HELP | 未适配 |
| 1426 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 1427 | Razer Madeline T1 | Available | 有入口 | 75/75 | TAB_SOUND / TAB_CALIBRATION / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1439 | Razer Kraken V4 X Sensa HD Haptics | Available | 有入口 | 9/9 | TAB_SOUND / TAB_MIC / TAB_HAPTICS / TAB_LIGHTING / HELP | 未适配 |
| 1442 | RAZER CLIO X | Available | 有入口 | 192/192 | TAB_SOUND / SURROUND / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 1443 | RAZER LEVIATHAN V2 | Available | 有入口 | 8/8 | TAB_SOUND / TAB_EQ / TAB_LIGHTING / HELP | 未适配 |
| 1446 | Razer Seiren V3 Pro | Available | 有入口 | 40/40 | MIC / TAB_EQ / TAB_EFFECTS / STREAM_MIXER_HEADER / TAB_LIGHTING / HELP | 未适配 |
| 1453 | RAZER MAKO X | Available | 有入口 | 24/24 | TAB_SOUND / TAB_EQ / TAB_MIC / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 1462 | Razer Kraken Kitty V3 | Available | 有入口 | 12/12 | TAB_SOUND / TAB_EQ / TAB_MIC / TAB_LIGHTING / HELP | 未适配 |
| 1465 | Razer Hammerhead V3 Chroma | Available | 有入口 | 56/56 | TAB_SOUND / TAB_EQ / TAB_ENHANCEMENT / TAB_MIC / LIGHTING / HELP | 未适配 |
| 1537 | Razer Surround | Synapse2 | not_found | 0/0 | — | 未适配 |
| 2304 | Razer Serval | Synapse2 | not_found | 0/0 | — | 未适配 |
| 2308 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2562 | Razer ManOWar | Synapse2 | not_found | 0/0 | — | 未适配 |
| 2580 | Razer Wolverine Ultimate | Xbox | not_found | 0/0 | — | 未适配 |
| 2581 | Razer Wolverine Tournament Edition | Xbox | not_found | 0/0 | — | 未适配 |
| 2593 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2594 | Razer Turret Mouse Gears Of War 5 Edition | Available | 有入口 | 52/52 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 2595 | Turret Keyboard Gears Of War 5 Edition | Available | 有入口 | 113/113 | TAB_CUSTOMIZE / TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 2596 | RAZER BLACKWIDOW V3 TENKEYLESS | Available | 有入口 | 205/205 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 2597 | Razer Kaira Pro for Xbox | XboxHeadsets | not_found | 0/0 | — | 未适配 |
| 2598 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2599 | Razer Kaira for Xbox | XboxHeadsets | not_found | 0/0 | — | 未适配 |
| 2601 | Razer Wolverine V2 White | Xbox | not_found | 0/0 | — | 未适配 |
| 2606 | Razer Wolverine V2 Chroma | Xbox | not_found | 0/0 | — | 未适配 |
| 2610 | Razer Hammerhead True Wireless V2 | XboxHeadsets | not_found | 0/0 | — | 未适配 |
| 2612 | Razer Kaira Pro HyperSpeed | XboxHeadsets | not_found | 0/0 | — | 未适配 |
| 2623 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2627 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2629 | Razer Wolverine V3 Tournament Edition | Available | 有入口 | 129/129 | TAB_CUSTOMIZE / TRIGGERS / THUMBSTICKS / TAB_CALIBRATION / HELP | 未适配 |
| 2636 | Razer Wolverine V3 Pro | Available | 有入口 | 161/161 | TAB_CUSTOMIZE / TRIGGERS / THUMBSTICKS / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 2637 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2638 | Razer BlackShark V3 Pro XBOX | Available | 有入口 | 74/74 | TAB_SOUND / TAB_CALIBRATION / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 2639 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2640 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2641 | Razer BlackShark V3 XBOX | Available | 有入口 | 64/64 | TAB_SOUND / TAB_CALIBRATION / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 2642 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2643 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2644 | Razer BlackShark V3 X Hyperspeed | Available | 有入口 | 9/9 | TAB_SOUND / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 2645 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2646 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2647 | Razer Wolverine V3 Pro PC | Available | 有入口 | 130/130 | TAB_CUSTOMIZE / TRIGGERS / THUMBSTICKS / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 2648 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2649 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2650 | Razer Wolverine V3 TE PC | Available | 有入口 | 118/118 | TAB_CUSTOMIZE / THUMBSTICKS / TAB_CALIBRATION / HELP | 未适配 |
| 2651 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2657 | Razer Hammerhead V3 HyperSpeed | Available | 有入口 | 10/10 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 2660 | Razer Hammerhead V3 X HyperSpeed | Available | 有入口 | 17/17 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 2664 | Razer Hammerhead V3 X Hyperspeed for Xbox | Available | 有入口 | 11/11 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 2665 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2668 | Razer Hammerhead V3 X HyperSpeed For PlayStation | Available | 有入口 | 17/17 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 2674 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2676 | Razer Wolverine V4 Pro | Available | 有入口 | 125/125 | TAB_CUSTOMIZE / TRIGGERS / THUMBSTICKS / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 2677 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2678 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2680 | Razer Kaira HyperSpeed | XboxHeadsets | not_found | 0/0 | — | 未适配 |
| 2681 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2682 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2684 | Razer Silver T2 X | Available | 有入口 | 93/93 | TAB_CUSTOMIZE / TRIGGERS / THUMBSTICKS / TAB_CALIBRATION / HELP | 未适配 |
| 2685 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 2689 | Razer Hammerhead V3 X HyperSpeed Kuromi Edition | Available | 有入口 | 17/17 | TAB_SOUND / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 2690 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 3072 | Razer Firefly Hard Edition | Available | 有入口 | 14/14 | TAB_LIGHTING / HELP | 已有适配 |
| 3073 | Razer Goliathus Chroma | Available | 有入口 | 21/21 | TAB_LIGHTING / HELP | 已有适配 |
| 3074 | Razer Goliathus Extended Chroma | Available | 有入口 | 30/30 | TAB_LIGHTING / HELP | 已有适配 |
| 3076 | Razer Firefly V2 | Available | 有入口 | 10/10 | TAB_LIGHTING / HELP | 已有适配 |
| 3077 | Razer Strider Chroma | Available | 有入口 | 21/21 | TAB_LIGHTING / HELP | 已有适配 |
| 3078 | Razer Goliathus Chroma 3XL | Available | 有入口 | 10/10 | TAB_LIGHTING / HELP | 已有适配 |
| 3080 | Razer Firefly V2 Pro | Available | 有入口 | 29/29 | TAB_LIGHTING / HELP | 已有适配 |
| 3328 | Razer Ripsaw Game Capture Card | Synapse2 | not_found | 0/0 | — | 未适配 |
| 3331 | Razer RipSaw HD | Available | 有入口 | 6/6 | TAB_SETTING / HELP | 未适配 |
| 3334 | Razer Stream Controller | Available | 有入口 | 32/32 | TAB_HOME / STREAM_MIXER_HEADER / HELP | 未适配 |
| 3337 | Razer Stream Controller X | Available | 有入口 | 31/31 | TAB_HOME / STREAM_MIXER_HEADER / HELP | 未适配 |
| 3585 | Razer Stargazer | Synapse2 | not_found | 0/0 | — | 未适配 |
| 3587 | Razer Kiyo | Available | 有入口 | 5/5 | TAB_CUSTOMIZE / HELP | 未适配 |
| 3589 | Razer Kiyo Pro | Available | 有入口 | 5/5 | TAB_CUSTOMIZE / HELP | 未适配 |
| 3590 | Razer Kiyo X | Available | 有入口 | 5/5 | TAB_CUSTOMIZE / HELP | 未适配 |
| 3592 | Razer Kiyo Pro Ultra | Available | 有入口 | 5/5 | CAMERA / PROCESSING / IMAGE / HELP | 未适配 |
| 3594 | Razer Kiyo V2 Pro | Available | 有入口 | 7/7 | CAMERA / PROCESSING / IMAGE / HELP | 未适配 |
| 3595 | Razer Kiyo V2 | Available | 有入口 | 11/11 | CAMERA / PROCESSING / IMAGE / MIC / HELP | 未适配 |
| 3596 | Razer Kiyo V2 X | Available | 有入口 | 11/11 | CAMERA / PROCESSING / IMAGE / HELP | 未适配 |
| 3840 | Lenovo Y900 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 3841 | Lenovo Y27 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 3843 | Razer Tiamat 7.1 V2 | Synapse2 | not_found | 0/0 | — | 未适配 |
| 3847 | Razer Chroma Mug Holder | Synapse2 | not_found | 0/0 | — | 未适配 |
| 3848 | RAZER BASE STATION CHROMA | Available | 有入口 | 53/53 | TAB_LIGHTING / HELP | 未适配 |
| 3849 | Razer Chroma HDK | Available | 有入口 | 23/23 | TAB_LIGHTING / HELP | 未适配 |
| 3853 | Razer Laptop Stand Chroma | Available | 有入口 | 21/21 | TAB_LIGHTING / HELP | 未适配 |
| 3858 | RAZER RAPTOR 27 | Available | 有入口 | 5/5 | TAB_GAMING / TAB_COLOR / TAB_DISPLAY / TAB_LIGHTING / HELP | 未适配 |
| 3859 | Lian Li 011 Dynamic | Available | 有入口 | 21/21 | TAB_LIGHTING / HELP | 未适配 |
| 3863 | Razer Tomahawk ATX | Available | 有入口 | 51/51 | TAB_LIGHTING / HELP | 未适配 |
| 3865 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 3866 | Razer Core X Chroma | Available | 有入口 | 21/21 | TAB_LIGHTING / HELP | 未适配 |
| 3867 | Razer Seiren Emote | Available | 有入口 | 27/27 | TAB_SOUND / TAB_LIGHTING / HELP | 未适配 |
| 3869 | Razer Bungee V3 Chroma | Available | 有入口 | 24/24 | TAB_LIGHTING / HELP | 未适配 |
| 3871 | Razer Chroma Addressable RGB Controller | Available | 有入口 | 6/6 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 3872 | Razer Base Station V2 Chroma | Available | 有入口 | 51/51 | TAB_AUDIO / TAB_LIGHTING / HELP | 未适配 |
| 3873 | Razer Thunderbolt 4 Dock Chroma | Available | 有入口 | 38/38 | TAB_AUDIO / TAB_LIGHTING / HELP | 未适配 |
| 3878 | Razer Chroma Charging Pad 10W Fast Wireless Charger | Available | 有入口 | 9/9 | TAB_LIGHTING / HELP | 未适配 |
| 3879 | Tomahawk Gaming Desktop | Available | 有入口 | 20/20 | TAB_LIGHTING / HELP | 未适配 |
| 3880 | RAZER RAPTOR 27 (165Hz) | Available | 有入口 | 6/6 | TAB_GAMING / TAB_COLOR / TAB_DISPLAY / TAB_LIGHTING / HELP | 未适配 |
| 3883 | Razer Laptop Stand V2 Chroma | Available | 有入口 | 21/21 | TAB_LIGHTING / HELP | 未适配 |
| 3884 | Razer Chroma Wireless ARGB Controller | Available | 有入口 | 8/8 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 3885 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 3886 | Razer Chroma Wireless ARGB Controller | 连接／声明引用 | 有入口 | 3/3 | TAB_CUSTOMIZE / TAB_LIGHTING；TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 3893 | Razer Hanbo AIO 240MM ARGB | Available | 有入口 | 44/44 | TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 3894 | Razer Head Cushion Chroma | Available | 有入口 | 38/38 | TAB_LIGHTING / HELP | 未适配 |
| 3900 | RAZER PWM PC FAN CONTROLLER | Available | 有入口 | 22/22 | TAB_PERFORMANCE / HELP | 未适配 |
| 3907 | Razer Laptop Cooling Pad | Available | 有入口 | 67/67 | TAB_PERFORMANCE / TAB_LIGHTING / HELP | 未适配 |
| 3909 | Razer Freyja | Available | 有入口 | 8/8 | TAB_CUSTOMIZE / HELP | 未适配 |
| 3910 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 3921 | Razer Core X V2 | Available | 有入口 | 31/31 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 3922 | Razer Thunderbolt 5 Dock Chroma | Available | 有入口 | 183/183 | TAB_SOUND / TAB_LIGHTING / HELP | 未适配 |
| 3929 | Razer Monitor Stand Chroma | Available | 有入口 | 68/68 | TAB_CUSTOMIZE / HELP | 未适配 |
| 3932 | Handheld Gaming Dock | Available | 有入口 | 66/66 | TAB_LIGHTING / HELP | 未适配 |
| 3940 | Razer Soma Chroma | Available | 有入口 | 184/184 | TAB_LIGHTING / TAB_POWER / HELP | 未适配 |
| 3942 | Razer Marci | Available | 有入口 | 37/37 | TAB_SOUND / SURROUND / TAB_HAPTICS / TAB_LIGHTING / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 3946 | August T2 | Available | 有入口 | 126/126 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 3949 | MarciT2H | Available | 有入口 | 36/36 | TAB_HAPTICS / TAB_POWER / HELP | 未适配 |
| 4114 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4115 | Razer Kitsune | Available | 有入口 | 243/243 | TAB_CUSTOMIZE / TAB_LIGHTING / HELP | 未适配 |
| 4123 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4124 | Razer BlackShark V3 Pro PS | Available | 有入口 | 69/69 | TAB_SOUND / TAB_CALIBRATION / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 4125 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4126 | Razer BlackShark V3 PS | Available | 有入口 | 64/64 | TAB_SOUND / TAB_CALIBRATION / TAB_ENHANCEMENT / TAB_MIC / TAB_POWER / TAB_DEMO / HELP | 未适配 |
| 4129 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4130 | Razer BlackShark V3 X Hyperspeed | Available | 有入口 | 17/17 | TAB_SOUND / TAB_MIC / TAB_POWER / HELP | 未适配 |
| 4131 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4132 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4133 | Razer Raiju V3 Pro | Available | 有入口 | 173/173 | TAB_CUSTOMIZE / TRIGGERS / THUMBSTICKS / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 4134 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4135 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4136 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4143 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4144 | Razer Raiju V3 Pro Signature Edition | Available | 有入口 | 113/113 | TAB_CUSTOMIZE / TRIGGERS / THUMBSTICKS / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 4145 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4146 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 4147 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 5122 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 5125 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 5126 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 5127 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 45066 | Razer Basilisk V3 Pro 35K - XBOX 25 Anniversary Edition | Available | 有入口 | 77/77 | TAB_CUSTOMIZE / TAB_PERFORMANCE / TAB_LIGHTING / TAB_POWER / TAB_CALIBRATION / HELP | 未适配 |
| 45067 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |
| 45068 | 未提供名称 | 连接／声明引用 | not_found | 0/0 | — | 未适配 |

## 尚未完成

- 导航中仍有 0 个名称表达式未化简；保留原表达式与源码偏移。条件弹窗、子应用和各显示模式继续逐项追踪。
- 此表不是完整功能验收清单；当前 Rust 产品入口为 182/653/777 及 3072/3073/3074/3076/3077/3078/3080。新增鼠标垫的本地灯光、帮助和资源见[逐页规格](../screens/17-mouse-mat-lighting.md)，设备与 Chroma 服务仍未接通。不能把下载或编译完成计作实际界面验收。
- 图像、视频、字体、source map 和原生服务不在 JS/CSS 齐备统计内。
- 目录是已记录版本的官方来源，不能证明未来或未公开产品的完整性。
