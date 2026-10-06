# 第二批音频独立 Armory 根（2026-10-06）

本批新增五款经过各自当前 bundle 静态核实的独立 `displayMode=armory` 页面。证据为 [逐产品根、DeviceInfo、CSS 与图片导入](armory-audio-next-roots-current-evidence.json)，不以同产品家族替代逐包核对。

| 产品 | 当前根及 ProductImage 调用链 | 当前固定条件 |
| --- | --- | --- |
| 1328 Kraken BT Kitty Edition | `yv → Fv → eM/connect → Zm` | DeviceInfo 模块 7816；无 specialAudio，普通 250 高图框 |
| 1330 Leviathan V2 | `Aw → Ow → JH/connect → XH` | DeviceInfo 模块 2292；subCategory=SPEAKER，无 specialAudio，普通图框 |
| 1331 Kraken V3 HyperSense | `uz → cz → bH/connect → vH` | DeviceInfo 模块 2292；无 specialAudio，普通图框 |
| 1332 Kraken BT Sanrio Limited Edition | `yv → Fv → eM/connect → Zm` | 读取 1332 自己的模块 7816；无 specialAudio，普通图框 |
| 1335 Kraken V3 X | `xf → Kf → yc/connect → fc` | DeviceInfo 模块 2292 明确 specialAudio=true，独立图框高 390 |

所有根都只给 ProductImage 传入空属性；没有选择复杂的声音、映射或灯光编辑根。五个固定 DeviceInfo 都没有 showPairedMouseOnTop；customDotPattern/isSvg 也未由根提供，因此普通位图路径可达。1328 与 1332 的压缩符号相同，但源码 SHA、固定产品 ID、图片请求和入口位置均分别记录。

1335 的差异不能只用白名单解决：`xf` 只将 `.widget-prod.dot-bg` 的高度改为 390，而 `fc` 仍按 `this.props.height || 250` 将图片高设为 250，再由 CSS 居中。`ArmoryProduct::product_image` 现在分别计算图框与图片高度，保留 390 高点阵，图片维持 250 高居中。3893 的 200 高图和其他产品的 250 高图也各自保持原值。

五份当前 CSS 都明确 `.widget-prod` 的 margin 10 auto、max-width 1220，以及图片居中、22 像素点阵和径向渐隐。普通根覆盖 width:auto/min-width:unset；1335 只覆盖 height:390/min-width:0。本批没有引入新的字体、颜色或动画。

## 图像与内联源恢复

本批 31 个精确原始资源 URL 全部取得。新增 10 个产品/edition 身份及其各自 `prd-3x`、Dashboard 图，共 20 张准备后的 PNG。合并前批后，专用资源表覆盖 13 款产品、21 个产品/edition 身份，共 41 张 PNG；图片未替换成其他产品的缩略图。

1330 的 `prd-1x.png` 原始导入模块 3900 在当前 `3900.3c87230a.chunk.js` 中直接导出 PNG data URL，未丢失外部 chunk。原工具只识别外部 media 导出，因而误报缺失模块；本轮为静态审计和获取工具增加了这种明确的字符串导出识别。该项记录源 chunk SHA 与 data URL SHA；不为内联图片请求不存在的网络 URL。实际原生高分辨率产品图仍按原 `prd-3x.png` 导入选择 `prd-3x.ea5c6d9e.avif`。

资源准备沿用 `tools/prepare-armory-product-images.py`，新增本批证据输入；专用查找表和公共 manifest/embedded 已更新。合并后的源/输出 SHA、模块请求、edition 映射见 [资源收据](armory-product-image-resources.json)。

## 静态验证

```text
node tools/audit-armory-product-roots.cjs 1328 1330 1331 1332 1335 --output docs/re/armory-audio-next-roots-current-evidence.json --check
.work/resource-env/Scripts/python.exe tools/prepare-armory-product-images.py --check
```

仅进行了源解析、资源取得/转换/校验和 Rust 格式化；未运行应用、构建、测试、厂商 JavaScript 或 DLL。统一 cargo check 仍交由主任务。运行时像素比对、其他复杂产品独立根与遥测状态并未宣称完成。
