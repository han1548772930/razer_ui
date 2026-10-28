# 鼠标产品功能索引

本目录覆盖当前产品注册中的全部 75 个鼠标 PID。设备名称保留官方原文；产品定义、页面挂载和实际消费者分别核实，不能因为 bundle 含有某个共享组件或词条就判定产品拥有该功能。每份原件读取均核对 acquisition 的实际字节与 SHA-256，没有执行厂商 JavaScript。

功能入口：[自定义映射（含 Perfect180 入口）](customize.md)、[DPI 与性能](performance.md)、[动态灵敏度与旋转](advanced-sensitivity.md)、[校准](calibration.md)、[滚轮](scroll-wheel.md)、[灯光与电源](lighting-power.md)。详细挂载见各 PID 页面。

另有 [当前 162 独立入口](products/162.md)：该 URL 的 DeviceInfo.productId 为 163，设备名为 Razer Cobra。它是当前官方入口别名，保留其独立原件与参数；不把它计为注册表之外的另一款 162 硬件。

## 全部产品

下表名称来自 DeviceInfo 或其同模块名称声明；拼写与大小写保留原件，包括 115 的 Mamaba 和 199 的 Eden。116 缺少名称字段，保持未声明。DPI 的范围单位为 counts/inch，分号后为步长；缺少字段不能补成默认 1。“板载”分别列出 isOBMDevice 与 OBMSlots；false 与字段缺失不是同一状态。

导航按静态页面定义列出；Calibration、Advanced 等仍受下文的 BLE／固件条件控制，不代表每种连接状态均同时显示。

| PID | 官方设备名称 | DPI 范围；步长 | 板载；槽位 | 页面定义顺序 |
| --- | --- | --- | --- | --- |
| [70](products/70.md) | Razer Mamba TE | 100–16000；1 | 否；0 | Customize → Performance → Lighting → Calibration → Help |
| [80](products/80.md) | Razer Naga Hex V2 | 100–16000；1 | 未声明；未声明 | Customize → Performance → Lighting → Calibration → Help |
| [83](products/83.md) | Razer Naga Chroma | 100–16000；1 | 未声明；未声明 | Customize → Performance → Lighting → Calibration → Help |
| [89](products/89.md) | Razer Lancehead | 100–16000；100 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [92](products/92.md) | Razer Deathadder Elite | 100–16000；50 | 未声明；未声明 | Customize → Performance → Lighting → Calibration → Help |
| [96](products/96.md) | Razer Lancehead TE | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [98](products/98.md) | Razer Atheris | 200–7200；100 | 是；1 | Customize → Performance → Power → Calibration → Help |
| [99](products/99.md) | Razer Jugan | 200–7200；100 | 未声明；未声明 | Customize → Performance → Lighting → Calibration → Help |
| [100](products/100.md) | Razer Basilisk | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [101](products/101.md) | Razer Basilisk | 200–6400；100 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [103](products/103.md) | RAZER NAGA TRINITY | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [104](products/104.md) | Razer Mamba Hyperflux | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [105](products/105.md) | Razer Mamba Hyperflux | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [106](products/106.md) | D.VA Razer Abyssus Elite | 200–7200；100 | 未声明；未声明 | Customize → Performance → Lighting → Calibration → Help |
| [107](products/107.md) | Razer Abyssus Essential | 200–7200；100 | 未声明；未声明 | Customize → Performance → Lighting → Calibration → Help |
| [108](products/108.md) | Razer Mamba Elite | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [110](products/110.md) | Razer DeathAdder Essential | 200–6400；100 | 未声明；未声明 | Customize → Performance → Lighting → Help |
| [112](products/112.md) | Razer Lancehead Wireless | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [113](products/113.md) | Razer DeathAdder Essential | 200–6400；100 | 未声明；未声明 | Customize → Performance → Lighting → Help |
| [115](products/115.md) | Razer Mamaba Wireless | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [116](products/116.md) | 未声明；指南标识 ABYSSUSLITE | 200–6400；100 | 未声明；未声明 | Customize → Performance → Lighting → Calibration → Help |
| [117](products/117.md) | Razer Turret Mouse Xbox One Edition | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [120](products/120.md) | Razer Viper | 100–16000；50 | 是；1 | Customize → Performance → Lighting → Calibration → Help |
| [122](products/122.md) | Razer Viper Ultimate | 100–20000；50 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [124](products/124.md) | Razer DeathAdder V2 Pro | 100–20000；50 | 是；4 | Customize → Performance → Lighting → Power → Help → Calibration |
| [128](products/128.md) | Razer Pro Click | 100–16000；50 | 是；1 | Customize → Performance → Power → Help |
| [131](products/131.md) | Razer Basilisk X Hyperspeed | 100–16000；50 | 是；1 | Customize → Performance → Power → Help → Calibration |
| [132](products/132.md) | RAZER DEATHADDER V2 | 100–20000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [133](products/133.md) | Razer Basilisk V2 | 100–20000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [134](products/134.md) | Razer Basilisk Ultimate | 100–20000；50 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [138](products/138.md) | Razer Viper Mini | 200–8500；100 | 是；1 | Customize → Performance → Lighting → Calibration → Help |
| [140](products/140.md) | Razer Deathadder V2 Lite | 200–8500；100 | 是；1 | Customize → Performance → Lighting → Calibration → Help |
| [141](products/141.md) | Razer Naga Left Handed Edition | 100–20000；100 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [143](products/143.md) | Razer Naga Pro | 100–20000；50 | 是；4 | Customize → Performance → Lighting → Power → Help → Calibration |
| [145](products/145.md) | Razer Viper 8Khz | 100–20000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [147](products/147.md) | RAZER NAGA CLASSIC EDITION | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [148](products/148.md) | Razer Orochi V2 | 100–18000；50 | 是；1 | Customize → Performance → Power → Help → Calibration |
| [150](products/150.md) | Razer Naga X | 100–18000；100 | 是；1 | Customize → Performance → Lighting → Calibration → Help |
| [152](products/152.md) | Razer DeathAdder Essential | 200–6400；100 | 是；1 | Customize → Performance → Lighting → Help |
| [153](products/153.md) | Razer Basilisk V3 | 100–26000；50 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [154](products/154.md) | Razer Pro Click Mini | 200–12000；50 | 是；1 | Customize → Performance → Power → Help |
| [156](products/156.md) | Razer DeathAdder V2 X Hyperspeed | 200–14000；100 | 是；1 | Customize → Performance → Power → Help → Calibration |
| [158](products/158.md) | Razer Viper Mini Signature Edition | 100–30000；50 | 是；1 | Customize → Performance → Power → Calibration → Help |
| [161](products/161.md) | Razer Deathadder V2 Lite | 200–8500；100 | 未声明；1 | Customize → Performance → Lighting → Calibration → Help |
| [163](products/163.md) | Razer Cobra | 200–8500；50 | 是；1 | Customize → Performance → Lighting → Calibration → Help |
| [165](products/165.md) | Razer VIPER V2 PRO | 100–30000；50 | 是；1 | Customize → Performance → Power → Calibration → Help |
| [167](products/167.md) | Razer Naga V2 Pro | 100–30000；50 | 是；4 | Customize → Performance → Scrolling → Lighting → Power → Help → Calibration |
| [170](products/170.md) | Razer Basilisk V3 Pro | 100–30000；50 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [175](products/175.md) | Razer Cobra Pro | 100–30000；50 | 是；4 | Customize → Performance → Lighting → Power → Help → Calibration |
| [178](products/178.md) | Razer DeathAdder V3 | 100–30000；50 | 是；4 | Customize → Performance → Calibration → Help |
| [180](products/180.md) | Razer Naga V2 Hyperspeed | 100–30000；50 | 是；1 | Customize → Performance → Power → Help → Calibration |
| [182](products/182.md) | Razer DeathAdder V3 Pro | 100–30000；50 | 是；1 | Customize → Performance → Power → Calibration → Help |
| [184](products/184.md) | Razer Viper V3 HyperSpeed | 100–30000；未声明 | 是；1 | Customize → Performance → Power → Calibration → Help |
| [185](products/185.md) | Razer Basilisk V3 X Hyperspeed | 100–18000；50 | 是；1 | Customize → Performance → Lighting → Power → Calibration → Help |
| [190](products/190.md) | Razer DeathAdder V4 Pro | 100–45000；1 | 是；1 | Customize → Performance → Power → Calibration → Advanced → Help |
| [192](products/192.md) | Razer Viper V3 Pro | 100–35000；1 | 是；1 | Customize → Performance → Power → Calibration → Advanced → Help |
| [194](products/194.md) | Razer DeathAdder V3 Pro Hyperpolling Technology | 100–30000；50 | 是；1 | Customize → Performance → Power → Calibration → Help |
| [196](products/196.md) | Razer DeathAdder V3 HyperSpeed | 100–26000；50 | 是；1 | Customize → Performance → Power → Calibration → Advanced → Help |
| [199](products/199.md) | Eden | 100–30000；50 | 是；1 | Customize → Performance → Lighting → Help |
| [203](products/203.md) | Razer Basilisk V3 35K | 100–35000；1 | 是；4 | Customize → Performance → Lighting → Calibration → Help |
| [204](products/204.md) | Razer Basilisk V3 Pro 35K | 100–35000；1 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [208](products/208.md) | Razer Pro Click V2 | 100–30000；50 | 是；1 | Customize → Performance → Lighting → Help |
| [211](products/211.md) | Razer Basilisk Mobile | 100–18000；50 | 是；4 | Customize → Performance → Lighting → Power → Help |
| [214](products/214.md) | Razer Basilisk V3 Pro 35K Phantom Green Edition | 100–35000；1 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [218](products/218.md) | Razer Cobra HyperSpeed | 100–26000；50 | 是；4 | Customize → Performance → Lighting → Power → Help → Calibration |
| [221](products/221.md) | Razer Boomslang 20th Anniversary Edition | 100–45000；1 | 是；1 | Customize → Performance → Lighting → Power → Calibration → Help |
| [222](products/222.md) | Razer Viper V3 Pro SE | 100–35000；1 | 是；1 | Customize → Performance → Power → Calibration → Advanced → Help |
| [224](products/224.md) | Razer Orochi V2 | 100–18000；50 | 是；1 | Customize → Performance → Power → Help |
| [226](products/226.md) | Razer Basilisk V4 Pro | 100–50000；1 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Advanced → Help |
| [229](products/229.md) | Razer Viper V4 Pro | 100–50000；1 | 是；1 | Customize → Performance → Power → Calibration → Advanced → Help |
| [231](products/231.md) | Razer Naga V3 Pro | 100–50000；1 | 是；4 | Customize → Performance → Lighting → Power → Help → Calibration |
| [235](products/235.md) | Razer Basilisk V4 HyperSpeed | 100–33000；1 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [239](products/239.md) | Razer DeathAdder V4 Pro Carbon Fiber Edition | 100–50000；1 | 是；1 | Customize → Performance → Power → Calibration → Advanced → Help |
| [2594](products/2594.md) | Razer Turret Mouse Gears Of War 5 Edition | 100–16000；50 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |
| [45066](products/45066.md) | Razer Basilisk V3 Pro 35K - XBOX 25 Anniversary Edition | 100–35000；1 | 是；4 | Customize → Performance → Lighting → Power → Calibration → Help |

## 页面挂载条件

PID124、131、143、148、156、167、180、231 的 Calibration 采用 `!isBle || DeviceInfo.ble?.calibration`；PID175、218 使用 `isBle || navs.push(calibration)`。PID98、170、185 的导航实现没有使用相同 BLE 条件，不能直接套用；PID224 不挂载 Calibration，不能由同名设备推断。

PID190、192、196、222、226、229、239 声明实际 Advanced 页面；具体固件门槛见 [动态灵敏度](advanced-sensitivity.md)。PID221、231、235 没有 Advanced 导航，即使共享包包含相关组件。PID226、235 的 Perfect180 是 Customize 映射入口。

PID167 有独立 Scrolling；PID231 的滚轮区域嵌入 Customize。PID104、105 是鼠标与垫的复合灯光产品，见 [灯光与电源](lighting-power.md)。页面路由、设备能力声明、实际读写任务和成功消费者必须分别判断。

BLE 能力分支由各产品定义：PID98、131、143、148、156、167、180、185、224、231 具有不同的 BLE calibration／pollingRate 等声明。PID226 明确 `supportBluetoothPollingRate=false`；不能从协议中存在某 getter 或某处出现 1000 Hz 就推断 BLE 页面支持设置。

## 配件与映射分支

PID80、83、103、141、143、147、150、167、180、231 声明 `attachmentInfo.hasAttachement=true`。各型号分别使用 2／6／7／12 键侧板，真实配件、用户浏览视图与持久化侧板映射分别建模；完整映射见 [自定义](customize.md)。

PID199、208 声明 `isHidingKeyMapping=true`、`defaultVisibleFunctionsCount=5`；折叠列表实际为过滤后的前五项加最后一项，不能把该数字理解为总共五项。

## 产品定义原件

原始路径前缀为 `.analysis/inputs/frontend/mouse-{PID}-current/raw/apps.razer.com/synapse/products/{PID}/ui/static/js/`。DeviceInfo 列为 object 本身，导航列为对应页面所有者的半开 UTF-8 字节区间；具体条件和 lazy module 另在逐产品文档及功能文档中说明。全部原件均已核对当前 acquisition 收据，源文件字节不以工作站路径作为运行时条件。

| PID | 原始文件 | DeviceInfo | 导航 owner |
| --- | --- | --- | --- |
| 70 | `main.8f24b6a1.js` | `[4687223,4687712)` | `[5726122,5735069)` |
| 80 | `main.16a5772c.js` | `[4862753,4863211)` | `[5763955,5773352)` |
| 83 | `main.e187f033.js` | `[4711065,4711531)` | `[5778666,5788056)` |
| 89 | `main.714ba9f0.js` | `[4688646,4689479)` | `[5764438,5773496)` |
| 92 | `main.ca936871.js` | `[42284,42681)` | `[5717925,5726678)` |
| 96 | `main.532590c5.js` | `[46323,47176)` | `[5758817,5767173)` |
| 98 | `main.91167ba0.js` | `[4781763,4782648)` | `[5677514,5685256)` |
| 99 | `main.0f9c94fa.js` | `[4735482,4735923)` | `[5707104,5715095)` |
| 100 | `main.1734869e.js` | `[43981,44816)` | `[5780157,5788918)` |
| 101 | `main.f1e7055d.js` | `[43953,44724)` | `[5779374,5788135)` |
| 103 | `main.b753ffe5.js` | `[45438,46312)` | `[5841713,5850569)` |
| 104 | `main.9b94769e.js` | `[44088,44893)` | `[5809917,5819641)` |
| 105 | `main.ef834389.js` | `[44088,44893)` | `[5809917,5819641)` |
| 106 | `main.4d2daa0b.js` | `[4778095,4778516)` | `[5759054,5767378)` |
| 107 | `main.eca0fb8e.js` | `[4735495,4735945)` | `[5709720,5717940)` |
| 108 | `main.e453e8a5.js` | `[25263,26101)` | `[5743271,5752032)` |
| 110 | `main.6e992750.js` | `[4830711,4831122)` | `[5511363,5519233)` |
| 112 | `main.47216244.js` | `[25577,26396)` | `[5787096,5796138)` |
| 113 | `main.c8b01b6c.js` | `[4830859,4831286)` | `[5515798,5523764)` |
| 115 | `main.414c4a81.js` | `[24817,25713)` | `[5774851,5783635)` |
| 116 | `main.2e83bad2.js` | `[4758709,4759036)` | `[5733331,5741551)` |
| 117 | `main.479a8db6.js` | `[43642,44533)` | `[5750064,5759525)` |
| 120 | `main.026ff14a.js` | `[24714,25441)` | `[5775076,5783663)` |
| 122 | `main.4197e469.js` | `[97778,98660)` | `[5776859,5786412)` |
| 124 | `main.564e2add.js` | `[97689,98548)` | `[5816612,5825915)` |
| 128 | `main.1c01e761.js` | `[244527,245298)` | `[5509579,5517570)` |
| 131 | `main.b42b8138.js` | `[257761,258642)` | `[5655679,5663562)` |
| 132 | `main.6b45cd12.js` | `[97170,98066)` | `[5750822,5759302)` |
| 133 | `main.84fbd82a.js` | `[99703,100508)` | `[5741431,5749426)` |
| 134 | `main.88c1e1a3.js` | `[98411,99348)` | `[5745592,5754118)` |
| 138 | `main.358f6b29.js` | `[4761497,4762243)` | `[5741792,5750557)` |
| 140 | `main.db53a005.js` | `[4761804,4762533)` | `[5732387,5740831)` |
| 141 | `main.aa4788cc.js` | `[143972,144871)` | `[5792252,5801088)` |
| 143 | `main.3eb704f1.js` | `[4770701,4771800)` | `[5906357,5916325)` |
| 145 | `main.24a35fa5.js` | `[97543,98394)` | `[5762879,5771921)` |
| 147 | `main.885c5195.js` | `[45407,46298)` | `[5822563,5830977)` |
| 148 | `main.1037d6bc.js` | `[165436,166309)` | `[5705402,5713291)` |
| 150 | `main.54e6989b.js` | `[32876,33678)` | `[5783793,5792470)` |
| 152 | `main.265c9698.js` | `[4831405,4832082)` | `[5522479,5530349)` |
| 153 | `main.9095b82c.js` | `[98747,99651)` | `[5749943,5758270)` |
| 154 | `main.35e3c57f.js` | `[245478,246282)` | `[5578937,5586925)` |
| 156 | `main.f791bc3f.js` | `[258666,259555)` | `[5683545,5691222)` |
| 158 | `main.d9d086de.js` | `[102010,102789)` | `[5694921,5703575)` |
| 161 | `main.ba9a72b0.js` | `[4761804,4762410)` | `[5732293,5740737)` |
| 163 | `main.efd0b513.js` | `[4778461,4779203)` | `[5762791,5771864)` |
| 165 | `main.571c242f.js` | `[4837008,4837888)` | `[5567825,5574919)` |
| 167 | `main.05d42904.js` | `[169124,171557)` | `[5823218,5833720)` |
| 170 | `main.74c5d12e.js` | `[105800,106772)` | `[5713554,5722153)` |
| 175 | `main.28575452.js` | `[105071,105968)` | `[5726775,5735757)` |
| 178 | `main.963979c0.js` | `[97244,98069)` | `[5582733,5591972)` |
| 180 | `main.dd14f44a.js` | `[4783499,4784564)` | `[5623120,5631878)` |
| 182 | `main.998a1822.js` | `[109343,110111)` | `[5621706,5630987)` |
| 184 | `main.059dbbc3.js` | `[4811953,4812749)` | `[5519739,5526977)` |
| 185 | `main.4b7c3d69.js` | `[268174,269106)` | `[5808097,5817118)` |
| 190 | `main.81b09779.js` | `[4825340,4826176)` | `[5589878,5598203)` |
| 192 | `main.e3a31907.js` | `[4878763,4879592)` | `[5762785,5771439)` |
| 194 | `main.6e5e136c.js` | `[99225,99993)` | `[5703189,5712047)` |
| 196 | `main.e5afc608.js` | `[4821451,4822300)` | `[5563117,5571218)` |
| 199 | `main.0b830f5b.js` | `[172018,172984)` | `[223633,232528)` |
| 203 | `main.fe8e2b48.js` | `[101980,102917)` | `[5701055,5709830)` |
| 204 | `main.20516787.js` | `[106374,107384)` | `[5742692,5752249)` |
| 208 | `main.1754b8f8.js` | `[196440,197482)` | `[219548,228451)` |
| 211 | `main.edf5c019.js` | `[103970,104862)` | `[5637055,5646124)` |
| 214 | `main.675ad770.js` | `[106554,107491)` | `[5748706,5758245)` |
| 218 | `main.5518adbf.js` | `[103976,104880)` | `[5724504,5733486)` |
| 221 | `main.28155bf9.js` | `[26608,27694)` | `[91772,100649)` |
| 222 | `main.38e03b8d.js` | `[4828301,4829135)` | `[5707696,5715921)` |
| 224 | `main.06abb0ec.js` | `[249068,249943)` | `[5676061,5683931)` |
| 226 | `main.6201645c.js` | `[60582,61739)` | `[131243,141873)` |
| 229 | `main.551458e6.js` | `[4840592,4841416)` | `[5583086,5591414)` |
| 231 | `main.eacf1afc.js` | `[115919,118442)` | `[5859808,5871816)` |
| 235 | `main.7496e955.js` | `[395023,396099)` | `[109390,119117)` |
| 239 | `main.6bc74a2d.js` | `[4830208,4830991)` | `[5585449,5593822)` |
| 2594 | `main.877e745a.js` | `[43698,44585)` | `[5749106,5758320)` |
| 45066 | `main.afdaaac9.js` | `[464435,465504)` | `[108731,118931)` |

这里记录当前官方源码语义。源码获取、逆向语义、Rust 独立实现、UI／后端接线、静态／模拟验证和真实运行验收分别判断，不能由此索引推断整族功能已完成。
