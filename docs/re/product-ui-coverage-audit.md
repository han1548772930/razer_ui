# 当前产品界面覆盖审计

核验日期：2026-10-02。当前 Rust 有 **10 个官方产品 ID 的工作区路由**；已取得 UI 入口的 331 个 ID 中，**321 个尚无产品工作区路由**。这不是全部界面已经完成的结论：已有路由仍缺少部分条件界面和服务接入。

完整待实现列表：[unimplemented-products.json](unimplemented-products.json)。每项保留产品名称、原始分类、互斥分组、目录及别名关系、入口 HTTP 收据、JS/CSS 齐备记录，以及实际导航的源文件、SHA-256、组件表达式、所属组件、显示模式和偏移。

## 统计口径与来源

| 项目 | ID 数 | 含义 |
| --- | ---: | --- |
| 本次目录及明确引用共处理 | 596 | 包含官方主目录、历史候选和连接／manifest 引用 |
| 有 UI 入口 | 331 | HTML、清单及导航已有来源证据 |
| 清单所列 JS/CSS 齐备 | 331 | 沿用目录的逐资源收据校验；不含图像、视频、字体或原生服务 |
| Rust 已有产品工作区路由 | 10 | 由当前编译模块与 `Tab::for_product` 推导 |
| 有 UI 入口而无产品工作区路由 | 321 | 本次待实现清单的精确集合 |
| 未取得 UI 入口 | 265 | 单列，不算作 265 个已确认缺失的界面 |

这里按 `product_id` 计数。连接别名、关联 ID 及共享导航仍保留，不将 331 个 ID 宣称为 331 种产品外观或独立界面。相同导航也不能证明内部控件、条件分支、布局或能力相同。HTTP 404 只说明记录时该地址不可用。

目录来源为 [product-catalog.json](product-catalog.json)，本次 SHA-256：`caef6c7c4349ce3300a102df38500c36c98e0e45664428b4d97af8496df15b84`。其上游 `.ref/discovery/products.json` 的 SHA-256 为 `678d072ec5ce92d11d048baa47d8d3b32f8b8013f1d782620f7a37ce0e5cd1ed`，本次重算一致。

本次重新检查目录所有导航引用的 **332 份不同 JS 文件**，实际文件 SHA-256 全部与导航记录相符。导航内容沿用目录的 AST 挂载证据，不把本轮的哈希校验写成重新追踪了所有子界面。产品源码均来自 `.ref/devices/<ID>/`；没有使用已停用目录。Dashboard 与宿主的当前版本证据另见 [20-current-source-version.md](20-current-source-version.md)。

## Rust 实际入口

[src/nav.rs](../../src/nav.rs) 的 `Tab::for_product` 只显式挂载 182、653、777，以及 [src/product.rs](../../src/product.rs) 中的七个 `audited_mouse_mat` 条目。`src/demo.rs::DEMO_PRODUCT_ID = 9001` 是本地演示产品，排除在官方统计外。

| 官方 ID | 当前工作区页签 | 另有工具栏动作 |
| --- | --- | --- |
| 182 | Customize、Performance、Power、Calibration | Help |
| 653 | Customize、Lighting | Help |
| 777 | Sound、Mic、Lighting、Power | Help |
| 3072、3073、3074、3076、3077、3078、3080 | Lighting | Help |

[src/shell.rs](../../src/shell.rs) 以 `Tab::for_product` 非空作为打开产品工作区的条件。[workspace.rs](../../src/features/workspace.rs) 的 `new`、`request_page` 及 `Render` 分别选择初始页、限制页切换并挂载实际页内容；未匹配的页面显示“此产品的页面尚未接入”。因此不能把枚举中的页面名称或磁盘上的 Rust 文件直接计为已接入界面。

Help 是已有工作区的工具栏动作，不额外增加产品 ID。182 的源码还包含单独配对根；主机共享配对流程也不能代替 **179 HyperPolling Wireless Dongle** 自身的 Customize／Help 工作区。179 当前没有产品路由，仍在 321 项内。

计数不使用目录里的 `implementation` 文本，也不使用 `report-razer-discovery.py` 的静态已适配列表作为唯一依据。JSON 记录了本次 `main.rs`、`nav.rs`、`product.rs`、`demo.rs`、`features/mod.rs` 的源文件指纹。

## 未挂载的历史文件

[src/main.rs](../../src/main.rs) 挂载 `features`；[features/mod.rs](../../src/features/mod.rs) 当前生产模块包括 `audio_page`、`controls`、`customize_drawer`、`customize_page`、`device_pages`、`help_page`、`keyboard_controls`、`lighting_color`、`lighting_input`、`sensitivity`、`settings`、`shortcut_engine`、`shortcuts`、`workspace`。`workspace` 又明确引用 `mapping_editor.rs`、`profile.rs`；`profile` 引用 `linked_games.rs`、`onboard_memory.rs`、`profile_transfer.rs`。

以下同目录文件没有进入当前生产模块链，不能据文件名宣称这些界面已完成：`audio.rs`、`calibration.rs`、`customize.rs`、`dashboard.rs`、`display.rs`、`engines.rs`、`enhancement.rs`、`eq.rs`、`haptics.rs`、`keyboard.rs`、`lighting.rs`、`macros.rs`、`mic.rs`、`mixer.rs`、`oled.rs`、`pairing.rs`、`performance.rs`、`power.rs`、`scrolling.rs`、`setting.rs`、`sound.rs`。测试模块也不增加生产界面覆盖。

## 尚无产品工作区的分布

以下分组互斥，总和为 321。分组按原始 `categories` 依次判断：鼠标；非 SYSTEM 的键盘／小键盘；SYSTEM；AUDIO 前缀或麦克风；GAMEPAD 前缀；摄像头；IOT 前缀或 Hue；其余。具体规则及每个导航键的完整计数均在 JSON 内。页面列为实际导航中的主要项目，每个产品对同一导航键只计一次；它们不是独立实现工作的数量。

| 分组 | 待实现 ID | 源码实际导航中的主要项目（括号为 ID 数） |
| --- | ---: | --- |
| 鼠标 | 74 | Customize／Performance 各74，Lighting／Calibration 各55，Power40，Pairing19，ADVANCED7，Scrolling1 |
| 键盘／小键盘 | 71 | Customize／Lighting 各71，Power／ACTUATION 各15，Pairing9，Calibration2，OLED1 |
| 音频／麦克风 | 70 | Sound63，Mic47，Power43，Lighting37，Enhancement34，Demo29，EQ12，Calibration7，Stream Mixer6，另有 MIC5、Haptics2、Surround2 等 |
| 笔记本／SYSTEM | 41 | Customize／Performance／Lighting 各41，Battery15，Display12，Sound8 |
| 手柄 | 9 | Customize9，Thumbsticks／Calibration 各8，Triggers7，Power5，Lighting2 |
| 摄像头 | 7 | CAMERA／PROCESSING／IMAGE 各4，Customize3，MIC1 |
| IoT／Hue | 8 | Lighting7，HOME1，CUSTOMIZED1 |
| 其他 | 41 | Lighting31，Customize13；另有 Display、Color、Gaming、Audio、Haptics、Stream Mixer、Pairing 等 |

原始 `TAB_MIC` 与 `MIC`、`TAB_LIGHTING` 与 `LIGHTING` 等键在机器清单里分别保留，没有仅凭名称合并为同一种页面。每组产品也都有源 `HELP` 导航。

“其他”包含扩展坞、鼠标坞、207 鼠标垫、灯光控制器、机箱、显示器、散热设备、Stream Controller、采集卡和座椅等。162 的来源分类是 `productCategoryIcon`，没有凭 `cobra` 名称将其擅自计入鼠标；同样保留其它不明确的原始分类。

## 已接入产品仍有的界面缺口

**Chroma 高级效果的已安装分支尚未实现。** 653 的当前 `.ref/devices/653/static/js/main.7b71cce5.js`，SHA-256 `5854d3cb89f5e01eeb258f3218301988e3509e27f38ed23ddd77040e39dcb6ab`，`Dh` 从字符偏移7968548开始。其渲染按 `allChromaResourcesInstalled` 区分安装介绍与已安装界面；后者包括说明、无配置空态、`chroma-studio-dropdown`、当前配置选择以及打开 Studio 的按钮。`changeEffect` 将所选配置的 `id` 传给 `setSelectedChromaProfiles`，`Ch` 将配置列表和已安装状态接入组件。

当前 [device_pages.rs](../../src/features/device_pages.rs) 的 `lighting_page` 在 `lighting.advanced` 分支只呈现安装介绍图、详情链接和未连接提示，没有已安装配置选择界面。接通服务本身不会自动补出这套条件 UI，后续需一并实现状态、空态、选择与按钮行为。

**灯光映射编辑器也仍为不可用说明。** [mapping_editor.rs](../../src/features/mapping_editor.rs) 的 `Category::Lighting` 初始值落入 `Assignment::Unavailable`，渲染只显示需要 Chroma 资源的说明。当前653源码的 `assignLightpacGroup`（字符7881020附近）会处理 `chromaEffectAssignment`，也会根据配置列表解析具体 `ID`／`Name`；当前 Rust 尚无对应的配置编辑分支。本次只确认该消费逻辑和 Rust 缺口，不把尚未逐项追踪的原映射选择器选项记为已审计。Macro、Interdevice 的不可用说明同样不能计作完整编辑器。

**182 Calibration 不是整页缺失。** 当前工作区已有介绍框和追踪控制。当前 `.ref/devices/182/static/js/main.db20a7c4.js`，SHA-256 `7626cc9c0a20cf491a481c701829429ac8f23c9c8c5b907736505f70d3b9bcc7`，`DD → LD` 的真实挂载是 `.welcome > .calibration-welcome.mouse-mat-calibration`，内部只有 close、title、body-text（字符4713211附近）。该介绍组件没有插图子项，不应再沿用历史文档里“缺介绍插图”的判断。布局与状态细节仍需依当前源码单独核对。

本轮没有运行界面，因而未验收实际像素、字体回退、硬件往返、Chroma 服务及完整交互。10 个已有路由表示已有可到达的实现入口，不表示这些项目已全部验收。

## 下一批候选：3853，然后3869

这两个产品均只有 Lighting／Help 入口，真实布局是产品图在上、左列亮度及熄灯、右列效果，适合在已审计灯光控件上增加明确的产品配置。优先3853：它有独立的 Lighting 模块3288，产品图引用也更简单。两项本轮仍是待实现，不因提出方案而改计数。

| 项目 | 3853 Razer Laptop Stand Chroma | 3869 Razer Bungee V3 Chroma |
| --- | --- | --- |
| 当前主 JS | `.ref/devices/3853/static/js/main.8f044274.js` | `.ref/devices/3869/static/js/main.82c8869b.js` |
| 主 JS SHA-256 | `a53d67ad1e95e35f3178d97b388cd25871f24a29e69095a46208983f612bdddc` | `29ad190a6df3ff9c0ab2465659065e9356ef222a6d6e6b0365b7eb897e5d7060` |
| 主 JS 字节数 | 5370983 | 5475443 |
| 导航所属组件／数组字符偏移 | `vP`／4541372 | `gg`／4552809 |
| 实际 Lighting 入口 | `nO.default`，其中 `nO=a(3288)` | `$d=Jd`，`Jd` 从4392642开始 |
| Lighting 根结构 | 模块3288：`Fa → I.default + left(H,Ba) + right(fa)` | `Jd → Qd + left(Hl,xd) + right(wd)` |
| 产品配置模块8193字符偏移 | 3944400 | 3801039 |
| Wave 方向 | `LeftRight` | `CWCCW` |
| 已取得／清单所列 JS/CSS | 21／21 | 24／24 |
| 帮助支持地址 | `https://mysupport.razer.com/app/answers/detail/a_id/4066` | `https://mysupport.razer.com/app/answers/detail/a_id/3849` |
| 指南前缀 | `http://dl.razer.com/master-guides/RazerSynapse3/LaptopStandChroma-00003853-` | `http://dl.razer.com/master-guides/RazerSynapse3/MouseBungeeV3Chroma-00003869-` |

两者当前 CSS 均为各自目录下的 `static/css/main.93c574c8.css`，424665字节，SHA-256 均为 `349092a4067831a2d4418148000071949ecc564bd76b461511f16a6d0b4a6616`。上述偏移为0起始的源码字符定位；导航记录保留原 AST 偏移，不作为字节偏移使用。

两者模块8193的实际效果顺序均为 **Audio Meter、Breathing、Spectrum、Starlight、Static、Tidal、Wave**。默认亮度启用、值100；随显示器关闭与闲置熄灯均关闭，闲置分钟为1；默认效果ID3，即Spectrum；`backlightStep=5`，类别为ACCESSORY。

这与当前 [settings.rs](../../src/features/settings.rs) 的七效果鼠标垫配置不同：3072／3076／3077／3080 使用 **Reactive**，3853／3869 使用 **Starlight**，且位置顺序不同；两款候选的 Wave 方向也不同。因此不能只将新 ID 加进 `audited_mouse_mat` 或现有效果列表。后续应增加经源配置核对的附件产品条目、路由、默认值、效果能力、产品图及帮助资源，再复核各自真实渲染条件。

3853 的产品图来自模块7855，资源上下文8388含 `./3853_0/img_prods/prd-1x.png`、`prd-3x.png` 和 chroma SVG。3869 同一资源上下文还区分两个 chroma SVG／产品图变体；新增前应按各自 manifest 准备映射，不能复用鼠标垫图片。

## 本次验证范围

完成 JSON 解析、321个唯一待实现 ID 的集合核对、10个官方路由和演示ID排除、分类计数及导航保留校验、目录／上游库存哈希校验、332份导航源文件哈希校验，以及文档差异空白检查。只生成覆盖审计资料，没有实施新品、运行应用、构建或测试；`cargo check --locked --all-targets` 由主任务统一执行。
