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

## PID182：DPI 与活动配置档、板载槽位

统一入口 `audit-family-function --family mouse --function mw --product 182 --query mouse-dpi-active-slot-chain`；正式证据归 `.analysis/results/frontend/mouse-mw-audit.json#/mouse_dpi_active_slot_chain`，当前阶段与后续路线仍归[统一路线图](../../re/ui-readonly-first-roadmap.md)。该查询直接核对当前保留部署，不复用旧部署的完成状态。

- 活动配置档来自当前 serial 的 `deviceMetadatas.activeProfileGuid`。普通 DPI 任务按 GUID 匹配 `obmData.profiles` 后取得实际 `slotId`；没有匹配项时不写板载槽，不能由 `OBMSlots:1` 推定槽号或配置档身份。
- 板载库存按设备返回的 max／count／ID list 读取。原始加载器跳过列表 index 0 的身份读取，投影另排除实际 ID 1。空名称槽会按本地 `obmSlotId`、再按首个本地配置档补 GUID／名称；这属于本地关联，不能记作设备实读身份。
- 初始化先经过 serial、本地缓存、活动配置档、供电及宏录制门槛。`ON_STAGES_INIT` 只有 deviceAccess 权限；普通启动的 DPI 恢复早于板载引擎加载，后续板载初始化另走库存读取与同步链。缺少本地文件不能作为设备需要默认值覆盖的依据。
- 普通任务先 SET／GET 固定 profile 1，再同步 GUID 对应槽。槽同步逐槽 pre-GET、必要时 SET，没有 SET 后读回；profile 1 的确认与任务 completed 均不能证明后续板载写入已持久化。任务内存保存、UI 乐观值、ACK／读回及板载身份来源必须分开。
- 本段仅闭合当前 PID182 的上述静态语义。独立 Rust、UI／后端接线、官方 DLL／真实设备对照与运行验收仍为未验证；其他产品及板载宏、映射、配置档增删改链继续由各自缺项接续。

## PID182：实际配置重命名与板载槽条件

统一入口 `python tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query mouse-182-profile-slot-chain`；正式结果见 [mouse-mw-audit.json](../../../.analysis/results/frontend/mouse-mw-audit.json) 的 `mouse_182_profile_slot_chain`。当前 UI 为 `main.529a6075.js`，MW 为 `6259.dedaea65043c32c54f0d.js`；原件身份、完整所有者和获取来源归该收据。本批从上一节剩余配置链接续，只核实际菜单的重命名。

- 当前 root 挂载普通 Profile 菜单，明确传 `hasOBM:false`。菜单包含 Rename；产品声明 `OBMSlots:1`。公共保存／移除槽 reducer 的存在不能证明该型号挂载手动槽面板。
- 重命名持有已选配置 GUID；编辑器最多 32 字符，提交时 trim，拒绝空白和精确同名，包括未改变的原名。Blur／Enter 提交；Escape 传回原名，通常由同名检查阻止发送。UI 先按选中 GUID 乐观改名、排序，再发送 `ON_RENAME_PROFILE`，不以列表下标或槽号作为配置身份。
- MW 先选择目标 runtime／serial，读取必需本机文档，按参数 GUID 改名并推进版本，依次保存文档、发布 runtime、更新关联游戏中的名称，再刷新配置列表。关联游戏名称按 `profileID` 匹配，没有额外 PID 条件。存储或后续步骤失败不自动撤回此前修改。
- 板载改名派生只在 `deviceAccess && isOBMDevice && OBMSlots>1` 时入队，另需匹配 GUID、非槽 1 且未锁定。182 的声明为 1，当前动作不进入该分支；实际库存数量不能替换这条产品条件。该结论不推广至其他型号，也不表示本产品的 DPI／映射没有板载写入。
- 文档存储钩子未指定 `useLockableStorage`，因此此处使用普通 `localStorage` 路径；外层事件队列的锁与文档读写不能混为一谈。去重哈希排除 version，缓存先于写入更新，失败后不能假定同内容重试一定再写。`PostSetLocalStorage` 另外启动未等待的 MappingEngine 文档同步：只比较 GUID、数量、active、hash、gamemode 等，不比较名称；热缓存纯改名可跳过，冷缓存或其他变化仍可能发布。
- 服务文档发布复用当前 Host／MappingEngine 的精确静态证据和 7 个原生函数，不重复捕获。原生布尔结果未被该钩子核对；本机存储完成、服务观察者发布、映射实际生效及设备读回分别判断，不能把这里的 completed 当作设备确认。
- 结果顺序为完整列表 → completed → postprocess → 任务内存保存。内存保存若最后失败，可能在完成通知之后再发 error；UI 按 timerTick 匹配并移除待办，未匹配结果会忽略，没有乐观值回滚。页面卸载取消存储订阅和计时器，但已派发重命名没有连接代次、AbortSignal 或补偿链，不能据此认定原生观察者已排空。

`Mouse182ProfileSlotChainTests` 静态反例 11/11 通过，覆盖实际挂载、GUID／serial、板载条件、编辑取消、持久化顺序、缓存／失败和消费者边界。该批没有执行厂商 JS、官方 DLL、应用或设备操作，没有新增临时目录、数据库或捕获。其他配置动作须先核各自实际挂载和消费者；Rust 独立实现、UI／后端接线、实际 DLL／设备对照与运行验收仍为未完成／未验证。

## PID182：实际配置档导出

统一入口 `python tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query mouse-182-profile-action-chain`；正式结果归 [mouse-mw-audit.json](../../../.analysis/results/frontend/mouse-mw-audit.json) 的 `mouse_182_profile_action_chain`。本批沿重命名收据的剩余配置链，只核实际挂载的 Export；复用同一当前 UI／MW 和已核验的本机存储证据，Host 保存桥另与当前官方包中的 main／preload 原件重新核对。

- 普通 Profile 菜单实际挂载导出弹层，进入前须有选中 GUID。列表排除产品工厂预设 GUID；同时存在 General 与 Factory Default 时再排除后者。初始选择当前配置档及其宏，重新选中配置档会选中全部宏，全选只改变配置档选择。确认按钮使用 CSS disabled 类，没有原生 disabled 属性或处理函数守卫；确认派发后立即关闭弹层，不等待保存。
- MW 先读本机宏库，再按目标 runtime／serial 读取必需配置文档，按所选 GUID 重新取实际配置，不导出 UI 携带的旧 profile 对象。缺少配置 GUID 会在解析时进入错误分支；找不到的宏直接跳过。共享存储读取在缺少 serial 元数据时可能在返回文档中补配置关联，导出动作自身不调用配置写入或设备／板载写入。
- 配置只删除 `appEngine`，其余字段通过 JSON、UTF-8／base64 编码；宏 payload 只含 GUID、事件和播放选项，名称位于外层。哈希另排除顶层 hash／gamemode，按稳定排序的 JSON 转义 `<` 后计算 MD5，不能把哈希输入和导出 payload 混为一谈。原版跨配置档宏去重检查外层 guid，但实际只保存 name／payload／hash，因此同一真实宏可能重复导出；这里保留该事实。
- 文件是带产品、分类和当前语言名称的 `.synapse4` JSON。实际路径为服务 → preload 的 Electron IPC → Host 保存对话框 → Node 文件写入；默认目录取上次未取消的保存目录或下载目录。取消不更新目录；对话框和写盘错误只记录日志，没有文件读回或原子发布确认。
- 保存调用没有 await，completed／postprocess 先于用户选择和文件写入结果。任务内存保存若随后失败，还可能补发 error；UI 对已移除的 timerTick 不再匹配。关闭弹层及页面订阅清理不取消已派发保存，也不能证明文件回调或共享宏库通道已释放。当前 transition 不经过同名的公共导出权限 helper，不能从该 helper 推定本动作持有设备／存储锁。

`Mouse182ProfileActionChainTests` 静态反例 10/10 通过，覆盖挂载、选择、GUID 重读、编码／哈希、原版去重边界、保存与完成顺序、取消、当前证据依赖和失败收据。没有执行厂商 JS、官方 DLL、应用、安装器或设备操作，没有新增临时目录、数据库或捕获。本段只覆盖导出；其他动作见下列专题，实施与验收边界以统一路线图为准。

## PID182：实际配置档复制

统一入口 `python tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query mouse-182-profile-duplicate-chain`；正式结果归 [mouse-mw-audit.json](../../../.analysis/results/frontend/mouse-mw-audit.json) 的 `mouse_182_profile_duplicate_chain`。本批从 Export 收据的剩余配置链接续，复用同一当前 UI／MW、活动配置恢复及本机存储证据。

- 普通 Profile 菜单包含 Duplicate，必须先选中配置 GUID。名称算法去掉末尾“空格＋数字括号”后缀，使用未占用的基础名或首个可用正整数后缀；GUID 使用原版 `crypto.getRandomValues` 模板生成。UI 只先加入 name／guid、排序并选择新 GUID，再将新配置的 HyperShift 及缓存设为 false；不会在此克隆完整设置。
- 实际 transition 只取得 exclusive 存储权限，没有 deviceAccess。后台按来源 GUID 从必需本机文档深克隆完整配置，覆盖新 name／guid、追加、切换文档与 serial 的活动 GUID，再推进版本。设置、映射、宏 GUID 引用、hash 和板载关联字段原样保留；没有另行复制宏实体或关联游戏。来源不存在会报错，不能生成默认配置。
- 目标 runtime 可按 container 选择，但此函数的文档及旧活动配置读取使用当前 runtime 默认参数，保存再使用已选目标，不能把两者改写成一致参数。后台还会异步读改写产品的 `profiles_stack`；该操作没有 await，失败不进入复制函数的 catch。随后依次保存文档、发布 runtime，再以新旧活动配置的差异交给已有恢复链。文档的 MappingEngine 钩子同样未等待原生结果，不能当成设备确认。
- 复制来源就是此前活动配置且设置未变时，差异仅为 name／guid；当前 PID182 没有这两个初始化 handler，也没有 PROFILE_DATA_PATCH 或 genericFeature 配置，因此不会额外排队设置／HID 写入或板载改名。恢复函数仍可能读取宏录制模式并刷新 runtime／配置列表；该可选读取复用当前 Host `macro_native_recorder` 的 4 个原生所有者，存储发布另复用 7 个，均未重新捕获或实际执行。
- 若来源与原活动配置不同，恢复链只处理实际变化字段：DPI → `ON_STAGES_INIT`，映射 → `ON_SET_KEYMAPPING`，有线 polling → `ON_POLLING_RATE`，无线 polling 走已有重读，lowPowerMode → `ON_SET_LOW_POWER_MODE`。这些动作仍受各自消费者、队列和确认约束，复制 completed 不代表排队动作已完成。复制结果顺序为恢复调用返回 → completed → postprocess → 任务内存保存；后续内存错误可能晚于完成通知，没有回滚或连接代次取消保证。

`Mouse182ProfileDuplicateChainTests` 静态反例 12/12 通过，覆盖名称／GUID、乐观状态、真实权限、来源克隆、目标差异、持久化顺序、恢复条件、晚期失败及精确依赖。未创建临时目录、数据库或捕获，未执行厂商 JS、DLL、应用或设备。本段只覆盖复制；实际导入见下一专题，剩余动作与实施／验收边界以统一路线图为准。

## PID182：实际本地配置档导入

统一入口 `python tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query mouse-182-profile-import-chain`；正式结果归 [mouse-mw-audit.json](../../../.analysis/results/frontend/mouse-mw-audit.json) 的 `mouse_182_profile_import_chain`。本批接续 Duplicate 的精确剩余项，复用同一当前 UI／MW、共享存储和 7 个原生发布函数，不重捕获已确认的链。

- 实际入口为普通 Profile 菜单的 Import → `IEMode:"import"` → `Fc` 连接的 `Bc` 弹层；父组件同名空回调不是执行入口。弹层使用 Redux `importProfile`，选中配置档时选中其全部宏，派发仅保留所选配置档及其宏 GUID 并集。当前 PID182 没有禁用 HyperShift 按键、额外导入 sanitizer 或 analogSpecs 配置，不应用相应可选预处理。
- 本地文件选择器声明 `.synapse4`，先按文本 JSON 的 profiles 字段识别当前格式，再尝试按 ZIP 的 DeviceInfo 前缀识别旧格式。当前格式按 URL-safe base64、UTF-8、JSON 解码；哈希排除顶层 hash／gamemode，使用稳定排序 JSON 的 MD5。配置按分类相同或匹配实际产品／dongle／BLE 身份接受，单项解码失败告警并继续；读取或外层 JSON 解析拒绝不在该局部 catch 内。宏通过哈希后，弹层还会递归解码字符串中的 XML 实体。
- 宏名称按当前库去重；只有 GUID 碰撞才生成新 GUID，并修复选中映射的对应引用。宏事件先生成 appEngine 并计算哈希，再设置实际 mouseMoveType。每个宏先写 MACRO_CONFIG，之后才整体提交 IndexedDB；IDB 使用独占锁、等待事务完成再发变更通知。原生宏投影删除外层维护字段，但会修改共享 appEngine 的虚拟延迟与哈希，因此 IDB 提交和后续 runtime 发布属于不同阶段。
- 配置浅合并当前 defaultProfile 和导入对象，生成新名称／GUID并追加。当前处理器明确调用 `qe(o.mappings,o,!1,g.concat(h))`：普通映射编译，第四参数保留其他配置档的 turbo 引用；不能套用旧 Xt 快照的第三参数语义。普通配置 handler 不修改 activeProfile，本动作不切换活动配置、不调用设备恢复，也没有对应的设备确认阶段。
- 提交顺序是逐宏配置 → 宏 IDB → 原生宏投影 → 设备配置文档／钩子 → runtime → 配置列表／宏列表 → completed → postprocess → 可选任务内存保存。跨存储没有事务或回滚；原生布尔返回未被消费者核对，晚期内存写入失败可能在完成通知后才补发 error。弹层确认立即关闭，Cancel／关闭只关闭弹层，没有 FileReader abort、连接代次取消或已派发工作的排空保证。
- 旧格式实际请求 `${origin}/profile-migration/worker.js`。后续[旧版配置与宏坐标专题](customize.md#pid182本地旧版配置导入与宏坐标)已核当前两轮 HTTP 部署、ZIP／XML 分组、源 PID 分派、九项共享转换、宏事件、坐标计算及当前 Host 4.0.896 的 monitor 回复／释放边界。每次 onmessage 内才创建 2 秒计时器，ready 等忽略消息留下的计时器也可能使请求拒绝，且没有 port.close。其余共享／源产品转换、原生 monitor 枚举和实际执行仍未闭合；入口清单与静态测试不代表完整迁移、Rust 或设备对照完成。

`Mouse182ProfileImportChainTests` 静态反例 12/12 通过，包含入口、文件身份／哈希、选择与预处理、宏冲突、当前普通映射、活动配置、提交／结果顺序、取消和失效输入拒绝。未执行厂商 JS、DLL、应用或设备，新增捕获、分析会话和临时资源均为 0。后续从 `mouse_182_profile_import_chain/remaining_chains/1` 接续其余实际动作；legacy 独立缺项保留在 remaining_chains/0。既有 `PreparedMouseImport` 只是本地候选结果，其当前源码一致性、真实发布、UI／后端接线、实际 DLL／设备对照和运行验收仍分别待完成／未验证。

## 旧版文件：共享映射转换

统一入口 `python -B -X utf8 tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query legacy-mapping-conversion-chain`；正式结果归[鼠标中间件收据](../../../.analysis/results/frontend/mouse-mw-audit.json)的 `legacy_mapping_conversion_chain`。本段接续 `mouse_182_legacy_worker_chain` 的 Mappings 缺项，复用同一当前 worker、两轮 HTTP 部署收据及实际 Import 消费者；29 个完整模块覆盖 `8061 → input62/output3405`、21 个实际输出分组、19 个输出 helper 及键表／别名／解析依赖。源产品专属转换调用方仍另行判断，不把目标 PID182 当作源文件范围。

- 共享注册器只在 all／源产品专属转换未接管该特征时调用 Mappings。键盘判断为 `deviceInfo.Type === "2"`；第一代 analog 仅按源 PID614／580／642 选择。源 PID677 在过滤之后，将禁用 HyperShift 列表内输入改成 `IsDefault:"true"`，不是先删除整条映射。
- analog 过滤函数 `u` 返回逗号表达式：先读取可选第二 actuation，再判断“第一点不等于默认且至少为 unit”，或 Level1 非默认，或“第二点 truthy、严格大于第一点且 Level2 非默认”。缺少／异常 analog 数据被 catch 后返回 undefined，因而不入选；不能把这条表达式误读为无 return。选中项继续双段与 RapidTrigger 转换。Broadcaster 输入绕过默认过滤；普通输入要求精确字符串 `IsDefault === "false"`，键盘滚轮音量默认映射另排除。
- `Boolean(IsHyperShift)` 将非空字符串 `"false"` 也当 true。Key／analog 输入优先扫描码，否则 HID，并取键表中第一个匹配项；找不到键时省略映射。未知输入类型或未匹配 Dial 可留下空 inputID，随后仍能合并有效输出。当前键表顺序、扩展键、数字键盘转换和别名必须保留，不能自行按名称修正。
- 直接 analog 转换以 gen1 unit12／default0 或 gen2 unit1638／default19656 形成按下值与 `max(value-unit,0)`；第一段输出必需，缺第二段时补 defaultGroup。RapidTrigger 的上下行灵敏度分别除 unit 并 Math.round，continuous 恒 true。Controller 曲线 x 为 gen1 的 `round2(210*x/25500)+1.5` 或其他代次的 `round2(390*x/25500)+0.1`，y 解析整数，无范围截断；输入数组与未知值仍遵循实际失败路径。
- 输出 dispatcher 实际有 21 个 case。键盘默认或同键且无 modifier／turbo 时得到空 keyboardGroup；Chroma_Effect、AdvanceGainSettings 虽在常量表声明，dispatcher 没有分支。鼠标、DPI clutch、文本、媒体／Windows／XBOX、配置跳转、Joystick、Keymap、灯光／全局亮度、Lightpac、音频与 Interdevice 均按原表和字段转换；表 helper 使用 `in` 与 truthiness，不是新增的严格 own-key 校验。
- HyperShift 输出按 inputID 计数，恰好一次时复制**列表中第一个相同 inputID 的映射**并设 isHyperShift=true；没有验证被复制项的输出类型、原层或重复目标层。共享转换仅向输出 dispatcher 传三个参数，宏 helper 的第四参数列表缺失，合法宏引用因此保留原 GUID，名称可成为非空 `undefined - S3`，不会落入写在后面的 `Macro 1` 回退。
- Launch 只要求 Path 或 Url 之一；Url 不含 `https://` 子串便加前缀，因此仅 Path 输入也能生成 `https://undefined`。Interdevice 按 PID 别名表第一个匹配项规范身份，Profile_Lightpac 另要求 LightpackGuid。音频切换 payload 按 PrimaryEndpointID 字段存在判定，不要求非空；这些都是当前文件转换行为，不代表已执行启动、音频或设备动作。
- 单项 `convertMapping` 内异常被吞并省略该项，但过滤和预转换 input-map 在 catch 外，畸形输入仍可能拒绝整个共享 worker。同步转换不增加取消、回滚、端口释放或设备确认；结果继续沿已确认的导入选择、普通映射编译和本机／原生发布链，不能把转换完成当成硬件写回。

首次定向验证发现 analog 返回逗号表达式被误读及测试缺少 argparse 导入，已按原件修正且保留失败事实。静态验证包含完整原件／范围／哈希、29 个模块缺失与正文变化拒绝、AST 返回逗号表达式、输入身份、源 PID、全部输出 case、键盘／analog／宏／HyperShift／失败边界。已有 `PreparedMouseImport` 和 `mouse_legacy_mapping` 保留原实现事实，本批未改 Rust 或重新验收它们；三类源产品 Mappings 调用由下节接续，其他共享转换、原生 monitor 枚举、UI／后端和真实 DLL／设备对照仍按各自缺项继续。未执行厂商 JS、DLL 或应用，未新增捕获、分析会话及临时资源。

## 旧版文件：源产品映射转换

统一入口 `python -B -X utf8 tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query legacy-product-mapping-chain`；正式结果归[鼠标中间件收据](../../../.analysis/results/frontend/mouse-mw-audit.json)的 `legacy_product_mapping_chain`。本段复用当前 worker 的两轮部署证明、共享映射及导入消费者，保留 11 个完整模块；对整份 worker 的 AST 核对确认，直接导入 shared8061 的模块只有共享注册器4104与侧板转换6892。目标 PID182 不筛掉旧文件的其他实际源产品。

| 实际源转换 | 原始 source Product_ID | 当前行为 |
| --- | --- | --- |
| 6892 侧板 | 167、143、103、150、83、80、141、147、180 | 每次仅以 profile 一个参数调用 shared8061，再按 `profile.deviceInfo.Product_ID` 的 swapPID 结果查侧板表。只建立该产品声明的侧板数组，按 2键 → 12键且非DKMInput → 6键 → 7键 → 主映射的先后顺序分流；6／7键分支改 inputType 为 KeyInput，其他分支保留。没有按当前连接侧板选择，也不跨侧板复制。 |
| 8667 广播器 | 1391、1386 | 直接对 Mapping 调用 forEach，不归一化单例；没有映射列表时返回 dialControl=0 和空映射，单例对象会抛错。FrontDialClick 按 Headphone 设 dialControl=1，否则0，后项覆盖前项；该输入不在五项输入映射表中，不形成映射。实际只处理 Chroma_Effect、AdvanceGainSettings、Multimedia，保留原效果ID、向下循环和麦克风音量转换。 |
| 8311 音频 | 1387、1388、1383、1393 | 单例与数组均归一化，跳过 falsy 条目；支持的 Audio 项固定使用 KEY_SWITCH_AUDIO_EQ／AudioInput／非HyperShift。SwitchAudioEQ 转成 SwitchAudioEq，ToggleMicMonitoring 保留；重复项保留，未知分组／assignment省略，缺 AudioGroup 会抛错。 |

源 registry 在 swapPID **之前**按原始 PID 选择，因此不能自动将143的144／146、167的168／169、180的181或1383的1384别名补入这些专属范围。专属 Mappings 会抑制共享同名特征；dispatcher 仅在真正执行某个共享转换时才通过 `Object.assign(profile,{deviceInfo})` 注入上下文。完全由专属转换处理的 profile 可能没有 deviceInfo，6892因此查不到侧板定义，shared8061也不进入键盘／第一代analog分支；这种输入仍按原链转换，不能自行补设备上下文。

all及源产品转换分别位于独立 try/catch，实际表达式为 `converter(profile) && Object.assign(result, converter(profile))`：成功返回对象时同一转换执行两次。转换内部异常丢失本次局部结果，之前已合入的字段和其他转换仍保留；三种专属转换都没有单条映射的 catch。同步转换返回的聚合对象继续交给既有 worker／Import 发布链，不增加设备确认、回滚、取消或端口排空语义。

静态验证核对实际参数、源PID与别名、侧板优先级、广播器与音频输入差异、双调用／catch边界、完整原件及变更拒绝、正式CLI分派和失败发布。其他源产品功能转换与共享功能仍按原清单独立接续；现有 Rust 导入候选、sidePanelMappings 宏引用修复及存储实现保留原验证范围。本段没有修改产品 Rust、执行厂商代码或新增临时资源，UI／后端和实际 DLL／设备验收仍未验证。

## 旧版文件：跨产品共享标量与开关

统一入口 `python -B -X utf8 tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query legacy-shared-scalar-chain`；正式结果归[鼠标中间件收据](../../../.analysis/results/frontend/mouse-mw-audit.json)的 `legacy_shared_scalar_chain`。本段一次覆盖共享注册器中的25个实际转换及6个依赖模块，复用相同 worker、源特征分派、导入消费者与部署证明；这些是旧文件的键盘／鼠标／音频／系统／充电设置转换，不代表只适用目标PID182，也不增加新产品能力。

| 源功能组 | 本次核对的转换 |
| --- | --- |
| 输入与系统 | GamingMode、BossKey、FunctionSwitch、InGamePollingRate、ModTap、PowerSavingsRefreshRate、MouseRotation、LedState |
| 音频与触觉 | MicrophoneVolumeNormalization、SpeakerBassBoost、MicrophoneAmbientNoiseReduction、MicrophoneSidetone、MicrophoneVoiceGate、SpeakerSoundNormalization、SpeakerVoiceClarity、MicrophoneVocalClarity、VirtualSurround、HapticIntensity、MonitorTHXMode、GameChatBalance |
| 灯光开关与充电标量 | PreventChromaSDKControl、SmartHome、EnableChromaEffects、ChargingLedBrightness、ChargingForceChromaControl |

各转换保留原字段与判定：多数开关要求精确字符串 `"1"`；轮询状态、刷新率节电和鼠标旋转使用精确 `"true"`；MonitorTHXMode要求 `BrightRoom`。FunctionSwitch直接判断FNKeyState的truthiness，因此非空 `"0"` 也得到multi。GamingMode另设isGameOn=false，并按Off／On／GameLaunched表取state；BossKey以“结果不等于undefined”接受合法值0，表查询仍使用原来的 `in` 语义。

数值使用 `value ? parseInt(value) : default`，默认0；truthy但不可解析的字符串保留NaN，不换成默认或做范围截断。MicrophoneVoiceGate显式传默认50后计算 `Math.round(n/20) || 1`，GameChatBalance计算 `Math.floor((100-n)/5)`。SpeakerVoiceClarity读取Volume、ChargingLedBrightness读取Brightness，其余相应值读取Value；InGamePollingRate分别保留WiredValue和DongleValue。LedState只生成logoBrightness开关和固定值0。这些默认值仅属于文件转换，不能作为设备初始化或读失败回退依据。

PreventChromaSDKControl与SmartHome实际都写入preventChromaConnectLighting；后者声明的syn4FeatureKey虽为SmartHome，dispatcher不读取该声明来改写输出。真正的覆盖顺序由 `Object.keys(profile)` 和每次 `Object.assign` 决定，不能用注册器顺序重排。专属all／源产品特征仍优先阻止共享转换，并沿已核专属链处理。

只有三个选定转换自带catch：MicrophoneVolumeNormalization失败返回空对象，两个充电标量失败返回undefined，均不会向聚合对象增加字段；其他畸形必需字段可使共享转换向外抛错。同步转换不提供逐特征取消、回滚、设备应答或清理保证，结果继续沿既有worker和Import发布链。静态反例覆盖全部25个转换的原件身份与真实分支，并核对真值、解析／取整、重复输出覆盖、catch差异及失败发布。

共享8061和三类源Mappings由前两节独立记录；其余17个共享转换由下节接续，其他专属源产品转换继续独立核对。没有新增捕获、分析会话、临时资源或产品Rust修改；独立实现、UI／后端与实际DLL／设备对照仍按各自收据判断。

## 旧版文件：共享结构化转换与完整注册表覆盖

统一入口 `python -B -X utf8 tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query legacy-shared-structured-chain`；正式结果归[鼠标中间件收据](../../../.analysis/results/frontend/mouse-mw-audit.json)的 `legacy_shared_structured_chain`。本段复用worker、已发布标量和共享映射三个正式段，核对最后17个导入共享转换及11个直接依赖，共28个完整模块。当前导入共享注册表的52项，恰由既有9项、共享映射1项、标量25项和本组17项覆盖；内置name／guid仍由原注册器链记录。这个集合闭合只证明当前共享文件转换的分段静态语义覆盖，整个worker、产品及独立实现仍未完成。

| 功能组 | 当前原件中的分支与结果 |
| --- | --- |
| 基础灯光、显示器颜色、系统灯光电源 | BasicLightingEffects只专门处理breathing／static，其他效果生成ID3；其static不做已有4982 helper的颜色数组归一化。MonitorColorPreset仅接受truthy模式查表结果，无VideoGain时RGB保留50。SystemLedPowerSettings无论是否联动均读取三套嵌套状态，idle值除60不取整；SystemLighting的off=0经 `||1` 变成ID1，twoMode的logo沿用电池LogoEffect。 |
| 扬声器校准、三类EQ、应用预设 | CalibrationV3直接map数组，随后 `filter(e=>e)` 保留id未定义的对象。EqualizerV3可用SystemEQs替换standard列表，但仍必须从StandardEQs读取headphones；选中预设未匹配时currentFrequencyData保持undefined。MicrophoneEqualizer固定currentBands为空，普通Equalizer在未匹配模式时不输出。SpatialApplicationsV3只归一化外层单例，保留ExeList.string和重复项，按实际输出名称／后缀选择预设。 |
| Camera | 先构造全部Presets.Preset图像条目，最后一个custom预设可覆盖基础focus。仅原始源PID3592进入扩展分支；扩展camera替换基础camera，不继承cameraEnabled／previewActive／FOV／hdrActive字段。MJPEG自动值要求字符串5，resolution默认3840×2160／30fps；各HDR、对焦、曝光、降噪、水印和ViewPreset字段按完整原件及shortcut helper转换，缺必需嵌套对象会抛错。 |
| Keymappings与commandDial | Keymappings归一化Keymapping列表，但内部Mapping必须是数组；按源PID选择analog代次，向输出helper传四个参数，第四个来自 `profile.macro?.items`。它返回顶层mappings空数组，可按profile键序覆盖普通Mappings。commandDial自定义项只传两个参数，不调用input helper；八种内置功能各保留两方向映射。槽位按parseInt(Slot)-1直接写数组，保留空洞和覆盖，内置UID每次重新生成。 |
| 充电灯效 | Object.entries保留区域顺序；已知区域的未知效果仍可得到只有regionId的对象。wave helper只收一个参数，因此不应用源PID方向分支；fastwave使用字符串FAST_WAVE。未知区域或任一效果异常被唯一的整组catch吞并，整组返回undefined，没有部分hardwareEffects发布。 |
| PWM与HapticsSettings | PWM跳过数组index0，其余端口从26编号；Off仍读取曲线，未知模式可留下undefined。触觉区域必须在实际四区域表中，否则向外抛错；各数组顺序、重复项和整数值保留，没有排序、范围截断或逐项catch。 |
| KillSwitch与PerformanceMode | KillSwitch归一化按键对，只有两个HID都解析成功才追加并递增ID；结构有效却无可用按键对时补A／D，缺嵌套数据仍会先抛错。PerformanceMode缺失返回空对象，未知模式抛错；已知模式仍读取全部三套插电风扇设置，电池固定balanced／auto／4500，custom模式不补造custom payload。 |

所有源数组的直接map／forEach与显式concat归一化均分别记录，不能统一补单例、过滤未知值或忽略缺字段。该组仅充电灯效6415带转换级catch；其他异常沿共享转换向外传播。BasicLightingEffects／LightingEffects同写quickEffects，Keymappings／Mappings同写mappings，SystemLighting同写brightness／logoLighting；最终覆盖由profile自身键顺序和Object.assign决定，不能按注册器顺序改写。

16项静态反例核对了全部原件、实际依赖、52项共享注册表分段覆盖、数组／单例、真值／默认、相机源PID和替换边界、analog／dial调用参数、未知效果／区域及失败发布。测试读取源码AST，不执行厂商JavaScript。独立Rust、UI／后端与实际DLL／设备对照仍另行验收；源产品专属转换由下节接续，原生monitor枚举及worker完整取消／释放继续保留各自缺项。没有新增捕获、会话或临时文件。

## 旧版文件：其余源产品转换与完整源注册表覆盖

统一入口 `python -B -X utf8 tools/workspace.py audit-family-function --family mouse --function mw --product 182 --query legacy-source-product-chain`；正式结果归[鼠标中间件收据](../../../.analysis/results/frontend/mouse-mw-audit.json)的 `legacy_source_product_chain`。本段核对剩余22个源产品转换及8个共享依赖，共30个完整模块。实际source registry共26项，由已核all6283、三类Mappings和本组22项形成无遗漏、无重复的分段覆盖。原始source Product_ID、注册顺序与特征选择器逐项保留；不能由swapPID结果补入原registry没有的别名，也不能按目标PID182删去其他源分支。

| 实际转换组 | 已核输入与结果差异 |
| --- | --- |
| 4714／PID1342混音器 | 完整初值对象的17个输出字段、数值范围／步长及默认绑定保留原始区间／SHA，随后按源码18步读取设置。声明的LedPowerSettings／LedBrightness被专属接管但不读取。路由音量按原min/max线性换算，输入通道百分比保留各自一位／两位取整。未知Fader Mode保留indexOf的-1并选meterEffects；EQ使用固定10／3个频率按下标取值，字符串CustomBands保留零默认，缺必需对象仍抛错。 |
| 4349显示器、8011面板 | 显示器按固定顺序读取五组必需设置，ColorGamut可选，未知模式保留undefined。面板亮度可缺省为关闭／10，但ColorTemperature始终读取并覆盖初始3000，缺对象不会成功返回默认暖度。 |
| 3803滚轮阶段 | 源167／168／169／180／181按Order-1写阶段数组，保留空洞与覆盖。自定义曲线按520×330尺度转换，y反向、保留两位小数，tension／steps默认11／15；原始三点及控制点必须存在，没有裁剪或排序。 |
| 1978低音、7337指示灯、5929语音门限、338 EQ | 低音字段缺失时本地转换生成enabled／level4，存在时复用1423并改字段名。指示灯四组设置均必需。特定音频源的语音门限用 `floor(value/50)+1`，不沿用共享 `round(value/20)||1`；PID1303只输出preset，不生成共享EQ频带。 |
| 6016主板、219水冷、7385面板灯、7526多端口 | 主板源Edition_ID精确字符串129选择第三ARGB，其余分支选择第二RGB；其brightness按PortID匹配。水冷的PortID精确字符串4指向泵，其余指向风扇，曲线只覆盖已有5／7项。7526的brightness和灯效按数组位置处理，忽略PortID；GlobalEffect helper在后续truthiness判断前执行，缺失仍可抛错。四类外层catch返回空对象，6016／7526另有brightness局部catch，可能保留部分结果继续转换。 |
| 2984与1182混音输出 | PID1346按registry顺序同时运行两者，分别产生streamMixerSettings与streamMixerSettingsAsPreset，互不替代。2984声明ListenMonitoringMix却不读取，两个总开关保持false；1182读取并反转Listen状态。1182的isDefault只按两个有序输入、字符串音量50及输出音量／部分状态判定，不额外核对link、route、playback state或listen状态。 |
| 5784系统灯光电源 | 只以 `{SystemLighting}` 调共享1627，另生成switchOffLighting的固定plugged／battery和真实twoMode转换。LedPowerSettings仅空字符串有特殊处理，null／缺失不能冒充空字符串；返回还包含brightness／logoLighting，不能按syn4FeatureKey声明丢掉它们。 |

七个性能转换保留实际差异；均不执行设备初始化，源字段缺失、空对象、未匹配值和抛错分别记录：

| 模块／源PID | 实际分支 |
| --- | --- |
| 5077：601、594 | 电池与插电balanced／batterySaver／gaming默认4000；缺PluggedInPerformanceMode抛错，模式查表结果可为undefined。 |
| 5093：587 | PerformanceMode与Wattage形成balanced／custom，默认4500；不执行其他严格分支的CPU／GPU数字类型和模式truthiness校验。 |
| 7717：616、652 | 电池4900，插电balanced／creator／gaming默认4300；缺插电分组抛错。 |
| 1723：621；8777：670、630、650、671 | 两者converter正文SHA相同，源PID组仍独立。默认4100；模式须truthy，CPU／GPU须number，合法0保留；silent风扇值固定4100。 |
| 9871：669、694 | 同样严格校验，默认3800，并增加turbo的Auto／FanRPM读取。 |
| 6628：672 | 同样严格校验，默认3000，无turbo字段。 |

每个profile先执行共享转换，再执行all6283，最后按registry顺序执行所有匹配源转换。专属转换只接收profile一个参数，不能保证共享阶段已经注入deviceInfo；成功的truthy结果会被调用两次。源码使用Object.assign替换整个输出字段：成功源性能结果会替换all的整个performance，源转换抛错则保留此前all结果。所有初值、局部失败和结果覆盖均为本地文件转换语义，不能当成设备实测、写回确认或通用恢复行为。

14项静态反例首次运行全部通过，覆盖30个完整模块、26项源registry集合、实际PID、mixer完整初值与18步顺序、原共享helper参数、数组位置、类型／默认／失败差异及正式CLI失败发布。当前共享52项与源26项转换分别具有可复核覆盖；原生monitor枚举、实际Import发布依赖、完整取消／释放及独立Rust、UI／后端、实际DLL／设备验收仍另行推进。没有新增捕获、会话、临时文件或产品Rust改动。

这里记录当前官方源码语义。源码获取、逆向语义、Rust 独立实现、UI／后端接线、静态／模拟验证和真实运行验收分别判断，不能由此索引推断整族功能已完成。
