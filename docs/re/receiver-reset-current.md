# 当前 164 / 241 Help 重置链

普通 Help 的 `confirmDel → resetDevice → ON_RESET_DEVICE {}` 没有调用通用 `DeviceResetFeature`。当前产品没有注册该 feature，也没有启用 `isOBMDevice / singleProfileDevice`；实际落入 **taskMakerResetOBM**。原代码先发送 UI completed，再修改软件配置，不能把这条 completed 当作设备接受重置。

当前源的完整函数正文、SHA-256、模块、偏移和获取收据见 [重置证据](receiver-reset-current-evidence.json)。[维护审计工具](../../tools/audit-receiver-reset-current.cjs)静态读取当前 middleware / 产品页面，生成 80 条源码收据。

## 实际状态链

1. 以保留的真实接收器 command collection / Container / instance 查询 `getSerialNumber`：共享 Rust 发送 `[22,0,130]`，使用原主 Linker 的 E0 事务号，按原 parser 仅保留可打印 ASCII。没有以 USB 字符串或保存的 UI serial 替代设备结果。
2. 缺本地文档时，按原 first-document producer 创建 schema 13、默认 profile、`isDefault` 和串号 metadata。已有文档缺该串号的有效 `activeProfileGuid` 时，按 `b4` 先追加一个唯一名默认 profile，再选中串号 metadata；这个缓存读修改不增 version。
3. Help task 保留旧 profiles，再追加默认 profile。名称为空计算机名时为 `Default`，其他情况为 `computerName-Default`；唯一名遵循原 100 次检查。UUID、53 位 version 回绕、activeProfile 和 metadata 按当前源处理，普通重置不合并通用 factory-reset globals。
4. 本应用将原文档保存到串号 / Container 所有者独立的本地 JSON 适配文件，提交前校验磁盘原值。该文件不是原 Chromium 存储格式，也不是设备固件 profile。
5. 原 `Fk` 后续要求 brightness、effects、mappings。亮度已接入原pre-read、条件写入与setter响应，不追加原TaskRunner没有的getter或值比较，见 [亮度协议](receiver-brightness-current.md)。真正response/error单独保存，原链完成不等于另一次硬件回读；effects与mapping未提交，整体 `device_refresh_complete` 保持false。
6. UI 接收真实串号和原文档，保留原本地 UI 草稿，刷新实际新增 profiles / active profile。原 Help 的两秒按钮冷却已独立恢复；该计时与内部真实任务 pending/error 分开，原 reset widget 下方不插入虚构的进度/丢弃面板。页面、设备与请求 generation 变更会取消旧任务，退出会保留并 join 工作线程。服务清理失败保留已经保存的文档与部分设备结果。

## 仍需完成

effects 和 mapping 刷新、原 host 发布与 memory cache、Chromium storage 同步、storage overrides、旧 schema migration、linked sub-device scope，以及其他产品的 Help 分支仍是全量范围内的缺口。它们没有被标记完成。

确认弹窗仍使用 gpui-kit 对话框；原 Help 所有确认视觉细节需要继续逐项核对。已连接的重置链不代表所有 Help 页面或整个产品界面完成。

允许的验证为静态审计、格式化和 `cargo check --locked --all-targets`。没有运行应用、构建、测试、安装程序、厂商 JavaScript 或 DLL；设备运行时验收尚未进行。
