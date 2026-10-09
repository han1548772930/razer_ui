# 当前动态 DLL 的实际调用方、工厂和初始化参数

2026-10-09，继续上一轮全量清单中 875 个“init 接收调用方 DLL 路径”的作用域。依据只有当前 middleware manifest/HTTP 回执、JS 字节和 Acorn AST；不执行厂商 JS、应用或 DLL。逐作用域、逐工厂、逐参数的 SHA-256/UTF-16 范围和原文保存在 [完整压缩证据](native-factory-current-evidence.json.gz)，可读计数在 [摘要](native-factory-current-summary.json)。

这次没有把清单中的 762 个候选签名扩成“确定加载”，没有修改 `assets/data/native-library-inventory.json` 或 Rust。源码中出现 DLL 默认名，产品 manifest 拥有某资源，PE 导出名字匹配，均不能替代实际 init 实参。

## 全量分母和剩余未知

| 项目 | 范围 |
| --- | --- |
| 起点 | 875 个 caller-DLL 输入作用域，来自上一轮 2332 未归属条目的一个子集 |
| 来源 | 330 个产品环境，28924 份 manifest 声明 JS；逐文件核对 SHA、HTTP 200、URL、字节数 |
| 模块解析 | 2631 个相关模块上下文；函数表达式、箭头与 webpack 简写方法均静态解析，失败 0 |
| 直接实例/初始化链 | 329 个作用域：154 IoT、175 audio capture；实际 new、字段或立即调用参数转交、init 实参均登记 |
| 带条件工厂/存储链 | 545 个作用域：154 ASRock、154 Hue v2、237 THX；构造器、条件返回、runtime 字段和真实 init 实参均登记，激活分支仍需证明 |
| 未闭调用方 | 1 个旧单文件 Hue 作用域；其余 874 已有直接或带条件的实际 caller 链 |

“直接链”说明原代码中存在这条实例和参数传递链；它不表示共享 chunk 在该产品启动时必然执行，也不表示缓存返回了特定文件，更不是硬件读取或初始化成功。所有 875 个作用域都保留 init 正文、导出及分项 unknown。其他 1457 未归属条目和独立应用 FFI 通道不在这个子任务的分母内。

## IoT：构造器到安装资源缓存

原管理器先 `this.rzIotMgr = new <class alias>`，再调用 `loadDll()`。该方法把资源选择函数返回数组的第 0 项传给 `this.rzIotMgr.init(path)`；选择函数的输入字面量是 `"IoTNative"`。本次逐份证明类 alias 指向隔离的真实 class，而非依据函数名认定类。

进一步追到真实 `installedResources` getter：通过源码的导出/局部函数关系读取缓存，解析 JSON，合并 `Common` 和 `Synapse`，缓存列表；列表为空抛出 `Failed to get Installed Resources`。选择器逐个查找 `record.name === requestedName`，且 `usedBy` 包含源码 DeviceInfo 产品 ID 或 `background-manager`，然后把 `userDataDir/Apps/record.filePath` 转换路径分隔符后返回；无匹配返回 `undefined`。枚举常量 `INSTALLED_RESOURCE_KEY` 也追到原字面量 `"installedResources"`，没有自行假设缓存键。

调用者把返回值直接传入 IoT init；无匹配时仍可进入 init 的源码回退分支 `Apps/Common/IoTNative.dll`。缓存整体为空而抛错、缓存存在但无匹配、匹配返回路径是三种不同分支。不能以 manifest 的归属替代缓存内容，也不能把无匹配冒充读取成功。

代表原件：产品 70 的 `.ref/middleware/70/7254.928934d7d583696a2c98.js`，SHA `45013e604a613a20f7b11fde3475e2b9d6d5ec0291bf6844c810ae350873726c`，构造器 `35678..35683`、实参解构 `42649..42681`、init 调用 `42690..42711`。它实际导入的选择器/缓存 getter 在 `.ref/middleware/70/6120.7fff31f083f844383e26.js`，SHA `5212f6a74225ecd0799bc9140acfaabf2ef6e5018ef22b41699c0915d96fb4a3`；其他 153 份各自保留独立原件。

## Audio capture：立即调用参数不能漏掉

原代码构造 capture 类实例后，把它作为立即调用箭头函数的参数传入。函数内取 `[path] = resourceSelector(["audCapNative"])`，再 `path && instance.initialize(path)`。上轮只看 class/init 字面量漏掉了这层参数转交；本次把实参、形参、实例构造、词法解构声明和 resource selector 全部关联。

这个实际 caller 在路径为空时不调用 initialize，因此不能把 class 内的默认 `audCapNative.dll` 回退当作该 caller 必然采用的路径。选择器随后读取的仍是安装资源缓存，而不是本项目枚举 USB 设备，也不是直接把 manifest 文件名当实参。

代表原件：产品 515 的 `.ref/middleware/515/2592.ca84d3908676d84b1779.js`，SHA `f450dd55cd10f1bb6622695b6a9a6c942f58b31ce84d160d969c858020f1b84c`。实参声明 `1776800..1776835`，选择器 `1893522..1893844`，真实缓存 getter `1893256..1893518`，缓存常量 `619897..619940`。175 份 wrapper caller 均逐份登记；页面/feature 是否激活这个共享引擎仍单独未知。

## THX：普通类和 Carol 类是条件分支

实际链为 lazy factory 属性 → webpack `require.bind(moduleId)` → default 导出 getter → 解构的 class → `new` → `runtimeData.thxDevice` → 另一模块的局部别名 → `init(resourcePath)`。证据还保留 switch case、factory 选择表达式和 feature 到类的映射表，未把所有 factory alternatives 归为同时使用。

产品 1306 的当前原件明确把 `AudioAvneraDevice` 映射到普通 THXV3 类，把 `rzDevice25Carol` 映射到 Carol 类。两者的 chunk 同时存在不证明两个分支都执行，也不能用 Carol 表的 40 个独有导出反推该产品一定加载 Carol DLL。原件 `.ref/middleware/1306/3487.591e6c57187c1625f29c.js`，SHA `c99e6afdd55d71341e9b44de0bcb9c1a653839af5c062526fe4a5624fe5a1c3a`：普通 lazy factory `1690540..1690592`，Carol lazy factory `1691064..1691120`，各自构造分支和存储/init 别名在完整证据。

同一当前 DLL loader 从 `installedResources` 合并列表中按状态筛选：`thxv3` 分支找 `THXNativeDLLV3Common`；其他分支找 `THXNativeDLL + runtime productId` 或 `RzNative_ + runtime productId`，并核对 `usedBy`。没有匹配则失败；有匹配时使用第一个记录的 `filePath` 拼接真实 init 参数，之后还有 THX service 状态/重试及可能的子进程分支。因此 Carol class 的默认 `ThxV3Native.dll` 不是实际参数结论，缓存顺序、资源记录和 feature 状态不能省略。

124 份这类普通/Carol 工厂链已追到存储和实参表达式；另外 113 份 THX 原件使用直接导入的 class 构造后写入同一 runtime 字段，实际 init caller 带 `instanceof` 类身份门禁并通过数组第 0 项传入资源选择结果。本次也追回它们的真实类、字段和参数，未借用普通/Carol 的 minified 符号。遇到 THXV4 类检查的相邻 initElectron 分支，不能关联成同一 V3 实例；工具按实际 class 字节身份排除了这种错误关联。

## 其他未知与重现

ASRock 的 generic factory 逐个枚举真实 factory map 的 key，受 `features[key]` 条件控制；只有实际 key 对应的动态 import/default class 可关联。接着通过已检查正文的 async helper/generator 的 return 返回 `new` 实例，保存局部变量，再写入 runtime rzDevice。通用 `hasDll` dispatcher 从同一导出对象/字段取实例，迭代 `getFeatureParam("hasDll", "dll")`，把 `item.name` 传入 resource selector，并在路径有效时 `instance.init(path)`。类选择条件、异步完成顺序和配置值仍未知；这些条件链不能解释为该产品已经执行 ASRock 初始化。

Hue v2 源码写入 `runtime.rzDevice = this.philipsHueMgr = new <real class alias>`，同一通用 dispatcher 读取 runtime 字段。此前只处理 assignment 右侧 Identifier 漏掉了这个嵌套赋值，此次连同 const 字段别名补上了 154 个上下文。ASRock 中还有 55 份旧 webpack 源码通过 `exports.default = alias` 导出 class，此次已按原赋值解析，不能因缺 export getter 而引用另一个版本。

产品 100 的 ASRock 代表链在 `.ref/middleware/100/763.743197fd9c333115a1de.js`，SHA `765d35bd3a7f2e9796aee8e7e6f144ca4d0079e08dcb2c1d8342750ae9f6d7bc`：computed default 解构 `2187409..2187433`、条件 return `2187434..2187501`、runtime 字段写入 `2187561..2187576`、init caller `2198302..2198311`。Hue 的真实 `new`/嵌套字段赋值在 `.ref/middleware/100/5559.3945c4185363d254236f.js`，SHA `450e8ac474d690ce13698836894561dbab9bc765a7814b86b5b0607b4ee1678b`，范围 `16652..16690`。

尚未闭合的唯一 caller 是产品 3886 的旧单文件 PhilipsHueNative：class/init 表在原件中，但它的工厂/receiver/init 调用关系不在已识别的 webpack module 范围，不能套用 Hue v2 的 caller。该项保留完整 init 正文、原件区间和 `legacy monolithic scope lacks resolved webpack caller context` 原因。所有已经追到 caller 的链也仍保留 feature 激活、真实安装缓存返回和运行 DLL 身份未知。

`node --max-old-space-size=4096 tools/audit-native-factories-current.cjs --check` 独立重读当前 manifest 收据和所有文件，重新解析模块、比对压缩完整证据及摘要。`--inspect-product=<id>` 仅输出该产品的静态诊断，不改全量产物。imports/字段对象按实际词法 declaration 解析，局部参数或变量遮蔽时不能借模块顶级同名导入；init receiver alias 的 assignment 也必须对应同一 declaration。门禁涵盖当前识别的词法变量、数组/default 解构、直接 IIFE 转交、明确 runtime 字段、计算 factory 的同一 ForIn key 和限定 async helper 的 return；任意动态属性、其他 helper 转发、执行次序、feature 条件和运行返回仍未穷尽。
