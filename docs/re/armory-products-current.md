# Armory 产品独立根

`ArmoryProduct` 挂载产品自己的 `displayMode=armory` 内容，产品图与复杂系统根分别核实；本地已有内容的入口可直接打开。安装状态与本地内容是否存在分别记录。

已核实的产品图范围由 [根收据](armory-product-roots-current-evidence.json)、[音频收据](armory-audio-roots-current-evidence.json)、[扩展音频收据](armory-audio-next-roots-current-evidence.json)及[资源表](armory-product-image-resources.json)定义。当前资源表覆盖 13 产品、21 个产品/edition 身份和 41 张 PNG；资源已准备，不再作为待下载缺口。

1335 的图框高 390，图片仍高 250 并居中；3893 的图片高 200，其余已登记普通图根高 250。每个产品使用自己的 DeviceInfo、CSS 和原图导入，不能用 Dashboard 缩略图代替。1330 的 1x 原图来自当前 chunk 的内联 PNG；准备器静态读取字符串，不执行代码。

3894 固定类别为 ACCESSORY，实际进入产品图根；同包 DEFAULT 灯光编辑分支不可据共享导出认定可达。[剩余根证据](armory-remaining-roots-current-evidence.json)单独记录它与 3907 的挂载链。

3893 显示 CPU、冷却、GPU 读数及 C/F 切换；源初值与实际遥测区分。3907 独立显示 CPU/GPU、曲线与本地 Smart 草稿；未观察到系统时显示 Not detected/破折号，不能制造硬件零读数。曲线分别保留 CPU/GPU 与模式状态，新增/删除/复位遵守源参数与端点约束。

仍缺实时遥测、固件过旧警告、连接后的性能/Hyperboost/Fixed RPM 状态与远程映射。字体回退、表格自动列宽、tooltip 位置及完整窗口像素未运行验收。

维护入口为 `audit-armory-product-roots.cjs --check`、`audit-armory-remaining-roots.cjs --check`、`prepare-armory-product-images.py --check`。具体参数与 source SHA 在对应 JSON；静态通过不表示整个产品完成。
