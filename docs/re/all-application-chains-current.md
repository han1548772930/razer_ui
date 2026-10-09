# 当前独立应用全量静态链

全部24端点均已列入：22个本地HTML入口、2个目录404端点。入口共755个唯一JS文件，本次全部Acorn静态解析成功；带webpack chunk标记的代码中模块factory候选登记8233项，CSS文件引用88次。webpack4的稀疏数组模块与webpack5数字对象模块分别解析，保留优化后的0/1/2参数factory；background-manager是ESM，不能用0个webpack模块解释为没有代码。

每个应用保存HTML script/link→asset-manifest entrypoints→全部JS/CSS→webpack模块声明/导出/字面依赖/lazy引用或ESM imports→JSX/createElement引用与状态/消息候选；CSS selector、原声明、条件、字体声明偏移和资源URL保存在[完整机器记录](all-application-chains-current.json)指向的gzip中。

范围仅代表当前本地源结构已全量列出。数字模块及命名调用仍含库代码；未按每个分支还原业务含义，动态模块/模板URL尚未全解，启动candidate并不等于已证明某个page实际挂载。22应用没有任何一个被计为全语义完成。实际本地UI和服务边界见[公共应用审查](application-review-current.md)。

| 端点 | manifest入口 | JS / CSS | 模块登记 | ESM import引用 | 状态调用候选 | 消息调用候选 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| /alisha/ | static/css/main.2ba74446.css, static/js/main.83ea24ca.js | 31 / 5 | 950 | 0 | 171 | 2065 |
| /background-manager/ | assets/index-8d39b3d5.js | 53 / 0 | 0 | 513 | 31 | 2815 |
| /chroma-app/dashboard/ | static/js/main.e3feeb1b.js | 83 / 6 | 675 | 0 | 255 | 5030 |
| /chroma-app/settings/ | static/js/main.7c706211.js | 24 / 1 | 199 | 0 | 64 | 1277 |
| /cortex/ | 无当前页源 | 0 / 0 | 0 | 0 | 0 | 0 |
| /feedback/ | static/css/main.5d2f226d.css, static/js/main.28905525.js | 25 / 2 | 261 | 0 | 63 | 1966 |
| /natalie/ | static/js/runtime-main.93ca35ea.js, static/js/2.a33344b0.chunk.js, static/css/main.fb527a4d.chunk.css, static/js/main.35e04e8c.chunk.js | 4 / 1 | 334 | 0 | 29 | 619 |
| /profile-migration/ | static/css/main.e18d10bc.css, static/js/main.512f18b6.js | 66 / 1 | 401 | 0 | 41 | 4731 |
| /rz-app-menu/ | static/css/main.3f2a3e21.css, static/js/main.83ced465.js | 10 / 1 | 26 | 0 | 12 | 708 |
| /rz-user-profile-menu/ | static/css/main.090e2108.css, static/js/main.bc7ef425.js | 9 / 1 | 23 | 0 | 18 | 710 |
| /settings/ | static/css/main.a418d266.css, static/js/main.eda30dd1.js | 39 / 5 | 305 | 0 | 88 | 1450 |
| /sophie-lite/ | static/js/runtime-main.fe71eb8a.js, static/js/2.0c77e20b.chunk.js, static/css/main.fe094892.chunk.css, static/js/main.bb22144c.chunk.js | 5 / 1 | 360 | 0 | 61 | 600 |
| /sophie/ | static/js/runtime-main.008ec9be.js, static/js/2.73c0ac19.chunk.js, static/css/main.6b373df4.chunk.css, static/js/main.0dad7d2f.chunk.js | 5 / 1 | 616 | 0 | 155 | 1487 |
| /synapse/ | 无当前页源 | 0 / 0 | 0 | 0 | 0 | 0 |
| /synapse/alexa/ | static/css/main.bbca16c4.css, static/js/main.05f102d2.js | 9 / 1 | 0 | 0 | 39 | 1614 |
| /synapse/armory/ | static/css/main.c0e644c4.css, static/js/main.3d0e8bd0.js | 28 / 4 | 633 | 0 | 88 | 1840 |
| /synapse/chroma-studio/ | static/css/main.8ec0cda4.css, static/js/main.6b22e9cc.js | 47 / 26 | 407 | 0 | 57 | 1211 |
| /synapse/dashboard/ | static/js/main.01550b17.js | 123 / 12 | 922 | 0 | 635 | 6618 |
| /synapse/introduction-tour/ | static/css/main.adb3bb78.css, static/js/main.bf769e69.js | 26 / 1 | 91 | 0 | 23 | 236 |
| /synapse/macro/ | static/css/main.9ea5d7e7.css, static/js/main.3f4b9604.js | 82 / 3 | 585 | 0 | 104 | 4407 |
| /synapse/profiles/ | static/css/main.ee3cb5b6.css, static/js/main.776df2b1.js | 73 / 3 | 497 | 0 | 151 | 4314 |
| /synapse/settings/ | static/js/main.31a43758.js | 25 / 1 | 230 | 0 | 70 | 1281 |
| /synapse/update-fw/ | static/css/main.3e3077d6.css, static/js/main.69cc5fbd.js | 62 / 1 | 326 | 0 | 64 | 3380 |
| /systray/systrayv2/ | static/css/main.1665f0a2.css, static/js/main.9579c403.js | 33 / 11 | 392 | 0 | 49 | 1452 |

## 仍需逐条还原的语义

- HTML外部框架脚本与各应用厂商模块的完整调度关系；模块引用并不证明真实执行顺序。
- 每个真实route/displayMode及独立模态的挂载、账号/安装/locale/固件/服务/产品能力门控。
- reducer初值、请求/订阅顺序、失败/取消/超时/销毁、消息字段和后端处理器；名称相似不能视为同一个API。
- 所有CSS动态类、布局/字重/字体fallback/hover/pressed/动画与资源实际消费。
- UI编辑、增删、Apply/Save及本地存储语义与DLL读观察/写回分别记录，局部草稿不冒充设备保存。

`node tools/extract-all-application-chains.cjs`只解析文本；`node tools/validate-full-ui-chains.cjs`核实证据；`python -X utf8 tools/report-full-ui-chains.py --check`核实MD表。没有运行应用、厂商JS、DLL、构建或测试。
