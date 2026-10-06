# Armory 音频根、原始图片与本地直达入口（2026-10-06）

> 同日第二批又补齐 1328、1330、1331、1332、1335，其中 1335 实现了真实的 390 高图框 / 250 高图片分支。当前累计 13 款产品、21 个产品/edition 身份和 41 张 PNG；本页下文数量描述第一批，续接详情见 [第二批独立根](armory-audio-next-roots-2026-10-06.md)。

本轮依照仓库 GPUI Kit 两份 skill 和当前源码要求，仅使用当前 `.ref/devices` 与当前应用目录。旧参考目录未读取。用户明确要求本地已实现入口直接打开，因此安装状态与可打开页面是两个独立事实。

## 两个真实缺失的 displayMode 根

| 产品 | 当前根调用链 | 固定 DeviceInfo |
| --- | --- | --- |
| 1319 Kraken Ultimate | `renderView → GK → vK → bH/connect → vH` | 当前模块 9245，category=AUDIO；没有 specialAudio 或 showPairedMouseOnTop 字段 |
| 1325 Kraken V3 Pro | `renderView → GF → vF → Mh/connect → Dh` | 当前模块 2292，category=AUDIO；没有 specialAudio 或 showPairedMouseOnTop 字段 |

两者都由 `displayMode === "armory"` 直接选中独立根，根只传入空属性的 ProductImage；不是普通 Lighting、Sound 或 Customize 页。特殊 390 高分支要求 `specialAudio === true`，这两个产品自己的固定对象不满足；没有擅自让共享 ProductImage 中的配对鼠标、自定义点阵、耳机 SVG 条件变为真。

`src/features/armory_product.rs::supports` 现在包含两款产品。既有 `SourceProductWorkspace::mapping_page_element` 优先选择此独立组件，Armory 既有 `has_armory_device_page` 检查即可打开，不需要制造产品导航标签或另一份设备状态。

布局逐产品来自当前 CSS：`.widget-prod` 高 250、margin 10 auto、max-width 1220；根 wrapper 用 width:auto/min-width:unset 覆盖普通页的 min-width:1024。产品图由 `.widget-prod img` 定位到水平/垂直中心，产品图组件传入 height=250。点阵周期 22、径向渐隐覆盖、背景 #222 沿用已有对应实现。该根没有文字控件或额外动画，未添加推测的转场。

完整源条件、组件、connect 调用、DeviceInfo 字面量、CSS、Webpack 图片请求及 SHA-256 在 [新音频根证据](armory-audio-roots-current-evidence.json)。维护工具现在显式支持独立输出与只读检查，避免新审计覆盖既有六款根收据。

## 原始资源恢复

本次原始 URL 获取成功，2026-10-05 的 network_error 记录是历史状态。已恢复 1303、1304、1313、1319、1325、3893、3894、3907 共八款产品的实际 `img_prods/prd-3x.png` Webpack 导入。共有 10 张独立源图、11 个产品/edition 身份；3894 edition 0 和 255 按各自原请求共用同一源图。PNG 保持原像素尺寸和 RGBA，源 AVIF、源 bundle、导出模块、manifest 和输出图均保留 SHA-256。

同时准备这 11 个身份各自的 Dashboard/应用选择器 `PluginImages` 图，单独登记，产品根仍只选择 `prd` 原图。新的 `armory-product-images.rs` / `armory-dashboard-images.rs` 经 `resources.rs` 接入，防止基础资源表的固定产品集合再次遗漏这些身份。精确图片与身份映射见 [资源收据](armory-product-image-resources.json)。

四个相机 section SVG 的公共资源登记也在本轮完成，来源与所有四款相机 manifest 的比对属于相机并行任务；`prepare-camera-section-assets.py --register` 可重现登记，`--check-registration` 全程只读。

## 本地入口的安装门控

AppPicker 原先只把 bundled_modules 当成本地可见入口，`launchable_modules/apps` 仍先经过安装列表才能出现；host 为显示 Chroma 额外写入虚构的 installed_modules/native_apps。

现在 catalog 的本地 capability 直接参与模块与其他应用的显示和排序。Chroma 原生页通过 `launchable_apps` 进入既有 Open 分派；host 不再写入外部安装事实，未知状态保持未知。推荐安装不会与已经可打开的应用重复出现。实际内容非空时也不再追加“安装状态尚未读取”提示。仅在专门模拟外部未安装应用的推荐/下载/安装预览中，显式清空本地 app capability，以保留隔离预览。

这是用户要求的本地适配策略；没有把它描述成官方安装流程本来就无门控。原菜单尺寸、字体、图标、动画和 Open 目标均由现有当前源实现保留。

## 可复查命令与限制

```text
node tools/audit-armory-product-roots.cjs --check
node tools/audit-armory-product-roots.cjs 1319 1325 --output docs/re/armory-audio-roots-current-evidence.json --check
.work/resource-env/Scripts/python.exe tools/prepare-armory-product-images.py --check
python tools/prepare-camera-section-assets.py --check-registration
```

本子任务仅运行静态解析、原始资源获取/转换/校验和 Rust 格式化。未运行应用、构建、测试、下载 JavaScript 或 DLL；现有 AppPicker 静态契约断言已随行为更新但未执行。统一 `cargo check --locked --all-targets` 由主任务记录。

产品原图的缺口已补齐；动态硬件遥测、其他尚未实现的 Armory/chromaApp 产品根和交互状态仍须继续逐项审计。没有运行窗口，因此不宣称已完成运行时或逐像素验证。
