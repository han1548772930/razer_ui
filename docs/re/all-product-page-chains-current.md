# 当前产品全量页面引用链

证据范围固定为2026-10-02取得的当前源；本次只解析本地源，没有重新联网确认发布日期。当前host仅使用4.0.827。

这是全部已注册产品的逐页索引，页内组件引用、状态调用、条件、候选命令与CSS类的原文位置保存在[机器索引](all-product-page-chains-current.json)和其gzip完整记录中。引用链不是完整执行链；本表没有把共享组件、路由存在或AST解析计为完成界面。

共331款，主导航1419页，独立模式33页；1452页取得非空引用子图，0页根未解，0页触及每页250组件边界。完整语义/视觉完成页数仍为0。

静态链按导航原文→根组件词法绑定→包装器/JSX组件→webpack导出与可定位lazy模块展开；保留每个组件的路径、SHA-256及UTF-16源码offset/end。useState/useSelector/setState/dispatch等是状态观察候选；get/set/read/write等名称只提供定位，不自动推定DLL读写。导航父级displayMode/lazy入口仍由原注册收据保存；导航没有component字段时必须追到实际render挂载，不能凭页面name补根。

各页的CSS规则候选见[全量样式链](all-product-layout-chains-current.md)。页内弹窗/控制器/生命周期和服务消息的精确语义仍需按原文逐分支审查；现有已人工核对的细节见[产品分组审查](product-review-current.md)及各产品族current契约。

部分新产品的导航表只有id/name，真实页在同一owner的AsyncRouter computed-key children对象中；已按导航name原表达式与computed key严格唯一匹配恢复JSX，并在recovered_component中保存原表达式/SHA/range。另保留函数包装根解析和父displayMode/lazy chain原证据。691等产品manifest使用/synapse/products/{id}/ui/绝对前缀，本地文件对应关系按该前缀规范化，原manifest保持不变。

独立multiDevicePairing另按owner.render→children:this.renderView()→this.state.navs.find(name===active_view)→唯一navigation name guard→实际返回JSX恢复；保留整个render/renderView/guard/return原文，静态validator逐个校验。TAB_LIGHTING的成员根通过外层webpack runtime的实际module调用AST确认require，再定位default导出；769 HOME的conditional根保留真实条件与两个分支，未任意选一个。

前一轮只重新展开缺根产品，其他原图显式保留旧解析器。本轮进一步按全部331个产品的真实导航根重新解析内层引用；不会把旧子图直接换stamp。先前computed-router、renderView、conditional双分支根原文仍保留并逐区间验证。断点只有scanner与registration指纹同时一致、所有源码摘要复核通过才复用。

通用解引用现在包括优化后的0/1/2/3参数webpack factory、ESM getter、CommonJS named assignment/module.exports转发、Object.defineProperty getter、有源码__esModule/default分支证明的Babel互操作、外层webpack runtime实际factory调用证明及可唯一定位的对象字面量成员。React/ReactRedux只有在产品当前HTML的实际script src指向现存UMD、UMD浏览器分支赋值到同名global且唯一导出可定位时展开；Fragment继续追到实际Symbol.for("react.fragment")，作为框架符号终点，并保存原文，未随意将未解标为外部。

函数、block/for/switch/catch及解构声明都保留词法遮蔽。require必须实际lookup到工厂第三形参；exports/module出口在整个词法表建成后确认绑定，内层同名参数/变量不会被算作工厂出口。UMD全局只在没有本地binding时采用，UMD依赖只使用对应factory形参；其独立数字loader不混入产品模块表。全部manifest JS先建立模块空间；同id而正文不同的factory保持module_conflicting_factories，附双方当前原文range，不凭先读/后读顺序选一个。部分冲突只是不同chunk的局部变量改名，但未经完整等价证明仍保留未知。

共保存98727条通用解引用证明（见gzip各页resolutions），包含原调用、导出/转发、runtime或UMD声明的路径/SHA/range。它们证明结构关联，不证明HOC副作用、真实state更新、分支可达性或任一设备读写成功。

| 通用解析证明类型 | 证明条数 |
| --- | ---: |
| babel_esm_default_interop | 280 |
| commonjs_global_export | 544 |
| commonjs_named_assignment | 10747 |
| commonjs_require_reexport | 11723 |
| defineProperty_export_getter | 49 |
| react_symbol_token | 1385 |
| umd_global_named_assignment | 544 |
| webpack_export_getter | 30576 |
| webpack_require | 42879 |

| 未解引用原因 | 引用次数 |
| --- | ---: |
| export_not_resolved | 1277 |
| lexical_binding_or_parameter_not_resolved | 727 |
| member_component_not_resolved | 5390 |
| module_conflicting_factories | 766 |

仍未解的动态props/PortComponent、React state保存的lazy组件、闭包参数、计算属性、非唯一factory/导出及尚未完全传递的Babel default namespace保持原表达式与owner。它们不是缺失界面，也不能因为部分共享Fragment已闭合就宣布业务语义完成；实际消息字段、读写API、清理和Apply/Save仍须逐分支审查。

本轮export_not_resolved全部指向4202/54202的Fragment：当前某些产品JSX-runtime先写Fragment=60107，再在Symbol.for可用分支改写为i("react.fragment")。两条原文确实存在，扫描器目前要求唯一出口，因此没有任选一个环境分支后宣称已解；后续应保留条件与赋值顺序展开两个可能值。它们不是模块缺失或新缺失的业务页面。

条件多出口原文实例：`.ref/devices/92/static/js/main.ca936871.js`，SHA-256 `77261e574636cc9ca13db973b531bc9b88cb345ce7965af4d95c684d61e3d09a`，UTF-16 `[210754,210883)`；test为`[210757,210812)`。

```javascript
if(a.Fragment=60107,"function"===typeof Symbol&&Symbol.for){var i=Symbol.for;_=i("react.element"),a.Fragment=i("react.fragment")}
```

test的逗号表达式先执行Fragment=60107；后半条件成立才执行Symbol.for别名及第二次Fragment赋值。上例从4202 factory的实际IfStatement提取；不能把第一次写入误描述为另一个独立else分支。

以下每种未解原因列出最常见的实际表达式及首个源owner，便于继续逐条查；统计含多个产品/页面中的共享重复引用，不是独立缺失功能数。

| 未解原因 / 原表达式 | 引用次数 | 首个产品 / 页面 | 源owner位置 |
| --- | ---: | --- | --- |
| export_not_resolved / `4202:Fragment` | 647 | 92 / TAB_CUSTOMIZE | .ref/devices/92/static/js/main.ca936871.js@11577 (s) |
| export_not_resolved / `54202:Fragment` | 630 | 709 / TAB_CUSTOMIZE | .ref/devices/709/static/js/main.f07ec2e5.js@642790 (o) |
| lexical_binding_or_parameter_not_resolved / `e` | 492 | 70 / TAB_LIGHTING | .ref/devices/70/static/js/main.8f24b6a1.js@4670117 (bl) |
| lexical_binding_or_parameter_not_resolved / `t` | 97 | 226 / TAB_CUSTOMIZE | .ref/devices/226/static/js/8123.dc0a3c84.chunk.js@33454 (Fe) |
| lexical_binding_or_parameter_not_resolved / `E` | 21 | 164 / TAB_LIGHTING | .ref/devices/164/static/js/main.458d4103.js@4400764 (ArrowFunctionExpression) |
| lexical_binding_or_parameter_not_resolved / `g` | 19 | 190 / TAB_CUSTOMIZE | .ref/devices/190/static/js/316.f2f64e83.chunk.js@29970 (Vt) |
| lexical_binding_or_parameter_not_resolved / `o` | 17 | 555 / TAB_CUSTOMIZE | .ref/devices/555/static/js/main.110aac54.js@6219370 (Ki) |
| lexical_binding_or_parameter_not_resolved / `l` | 16 | 1306 / TAB_DEMO | .ref/devices/1306/static/js/975.60eae682.chunk.js@394241 (E) |
| lexical_binding_or_parameter_not_resolved / `a` | 10 | 720 / TAB_CUSTOMIZE | .ref/devices/720/static/js/main.2b3c6f06.js@43293 (a) |
| lexical_binding_or_parameter_not_resolved / `i` | 10 | 170 / TAB_CUSTOMIZE | .ref/devices/170/static/js/main.7a4583ef.js@4409816 (gi) |
| member_component_not_resolved / `e.PortComponent` | 372 | 70 / TAB_LIGHTING | .ref/devices/70/static/js/main.8f24b6a1.js@4669857 (ArrowFunctionExpression) |
| member_component_not_resolved / `ve.A` | 219 | 1321 / TAB_SOUND | .ref/devices/1321/static/js/main.7ed29db3.js@4987741 (pG) |
| member_component_not_resolved / `VI.A` | 198 | 1398 / TAB_SOUND | .ref/devices/1398/static/js/main.cab68a9a.js@5195556 (Am) |
| member_component_not_resolved / `bI.A` | 198 | 1401 / TAB_SOUND | .ref/devices/1401/static/js/main.f418a62d.js@5182008 (Om) |
| member_component_not_resolved / `We.A` | 170 | 1313 / TAB_SOUND | .ref/devices/1313/static/js/main.c9234892.js@5052225 (sy) |
| member_component_not_resolved / `i.Comp` | 160 | 70 / TAB_CUSTOMIZE | .ref/devices/70/static/js/5107.895d674e.chunk.js@31332 (qt) |
| member_component_not_resolved / `Pe.A` | 132 | 1404 / TAB_SOUND | .ref/devices/1404/static/js/main.d350fd68.js@4585635 (wC) |
| member_component_not_resolved / `ye.A` | 127 | 1310 / TAB_SOUND | .ref/devices/1310/static/js/main.b8876f9f.js@5047038 (RH) |
| module_conflicting_factories / `24355:A` | 250 | 131 / TAB_CUSTOMIZE | .ref/devices/131/static/js/KeypadButtonPanel.1357b3ad.chunk.js@6497 (v) |
| module_conflicting_factories / `4355:A` | 146 | 70 / TAB_CUSTOMIZE | .ref/devices/70/static/js/KeypadButtonPanel.de25d09a.chunk.js@737 (v) |
| module_conflicting_factories / `70130:A` | 130 | 199 / TAB_PERFORMANCE | .ref/devices/199/static/js/683.8755a8fc.chunk.js@204259 (pi) |
| module_conflicting_factories / `44230:A` | 33 | 199 / TAB_PERFORMANCE | .ref/devices/199/static/js/683.8755a8fc.chunk.js@193819 (Ha) |
| module_conflicting_factories / `64625:A` | 30 | 691 / TAB_CUSTOMIZE | .ref/devices/691/static/js/8721.a0d61210.chunk.js@36815 (_t) |
| module_conflicting_factories / `58837:A` | 25 | 221 / TAB_CUSTOMIZE | .ref/devices/221/static/js/5107.180d922d.chunk.js@1093 (C) |
| module_conflicting_factories / `50151:A` | 24 | 691 / TAB_CUSTOMIZE | .ref/devices/691/static/js/8721.a0d61210.chunk.js@33084 (Mt) |
| module_conflicting_factories / `76299:A` | 24 | 691 / TAB_CUSTOMIZE | .ref/devices/691/static/js/6375.a5fed9ed.chunk.js@678254 (de) |

## 70 · Razer Mamba TE

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/70/static/js/main.8f24b6a1.js@4931938 | `(0,St.jsx)(MT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/70/static/js/main.8f24b6a1.js@4932035 | `(0,St.jsx)(kI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/70/static/js/main.8f24b6a1.js@4932082 | `(0,St.jsx)(oS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/70/static/js/main.8f24b6a1.js@4932129 | `(0,St.jsx)(TR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/70/static/js/main.8f24b6a1.js@4932176 | `(0,St.jsx)(Xm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 80 · Razer Naga Hex V2

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/80/static/js/main.16a5772c.js@4970230 | `(0,Et.jsx)(vT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/80/static/js/main.16a5772c.js@4970327 | `(0,Et.jsx)(JI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/80/static/js/main.16a5772c.js@4970374 | `(0,Et.jsx)(AS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/80/static/js/main.16a5772c.js@4970421 | `(0,Et.jsx)(cR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/80/static/js/main.16a5772c.js@4970468 | `(0,Et.jsx)(aM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 83 · Razer Naga Chroma

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/83/static/js/main.e187f033.js@4984903 | `(0,Et.jsx)(vT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/83/static/js/main.e187f033.js@4985000 | `(0,Et.jsx)(JI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/83/static/js/main.e187f033.js@4985047 | `(0,Et.jsx)(OS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/83/static/js/main.e187f033.js@4985094 | `(0,Et.jsx)(cR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/83/static/js/main.e187f033.js@4985141 | `(0,Et.jsx)(aM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 89 · Razer Lancehead

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [90]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/89/static/js/main.714ba9f0.js@4969964 | `(0,vt.jsx)(xT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/89/static/js/main.714ba9f0.js@4970061 | `(0,vt.jsx)(TA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/89/static/js/main.714ba9f0.js@4970108 | `(0,vt.jsx)(PS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/89/static/js/main.714ba9f0.js@4970155 | `(0,vt.jsx)(KR,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/89/static/js/main.714ba9f0.js@4970202 | `(0,vt.jsx)(UR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/89/static/js/main.714ba9f0.js@4970249 | `(0,vt.jsx)(LM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 92 · Razer Deathadder Elite

类别：MOUSE；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/92/static/js/main.ca936871.js@4923655 | `(0,Et.jsx)(eP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 62 | 23 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/92/static/js/main.ca936871.js@4923752 | `(0,Et.jsx)(eh,{})` | 25 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/92/static/js/main.ca936871.js@4923799 | `(0,Et.jsx)(wm,{})` | 33 | 17 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/92/static/js/main.ca936871.js@4923846 | `(0,Et.jsx)(tL,{})` | 19 | 2 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/92/static/js/main.ca936871.js@4923893 | `(0,Et.jsx)(Ac,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 96 · Razer Lancehead TE

类别：MOUSE；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/96/static/js/main.532590c5.js@4963953 | `(0,yE.jsx)(IO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 61 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/96/static/js/main.532590c5.js@4964050 | `(0,yE.jsx)(Pl,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/96/static/js/main.532590c5.js@4964097 | `(0,yE.jsx)(lA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/96/static/js/main.532590c5.js@4964144 | `(0,yE.jsx)(hS,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/96/static/js/main.532590c5.js@4964191 | `(0,yE.jsx)(Sm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 98 · Razer Atheris

类别：MOUSE；edition：[0, 128, 129, 130, 131]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [98]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [97]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/98/static/js/main.91167ba0.js@4881945 | `(0,Nt.jsx)(gT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/98/static/js/main.91167ba0.js@4882042 | `(0,Nt.jsx)(ZI,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/98/static/js/main.91167ba0.js@4882089 | `(0,Nt.jsx)(OA,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/98/static/js/main.91167ba0.js@4882136 | `(0,Nt.jsx)($O,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/98/static/js/main.91167ba0.js@4882183 | `(0,Nt.jsx)(LP,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 99 · Razer Jugan

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/99/static/js/main.0f9c94fa.js@4912090 | `(0,JE.jsx)(xL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 61 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/99/static/js/main.0f9c94fa.js@4912187 | `(0,JE.jsx)(xM,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/99/static/js/main.0f9c94fa.js@4912234 | `(0,JE.jsx)(Bm,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/99/static/js/main.0f9c94fa.js@4912281 | `(0,JE.jsx)(JD,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/99/static/js/main.0f9c94fa.js@4912328 | `(0,JE.jsx)(rc,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 100 · Razer Basilisk

类别：MOUSE；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/100/static/js/main.1734869e.js@4985787 | `(0,_t.jsx)(NT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/100/static/js/main.1734869e.js@4985884 | `(0,_t.jsx)(FI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/100/static/js/main.1734869e.js@4985931 | `(0,_t.jsx)(Ql,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/100/static/js/main.1734869e.js@4985978 | `(0,_t.jsx)(tR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/100/static/js/main.1734869e.js@4986025 | `(0,_t.jsx)(Ym,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 101 · Razer Basilisk

类别：MOUSE；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/101/static/js/main.f1e7055d.js@4985004 | `(0,it.jsx)(dT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/101/static/js/main.f1e7055d.js@4985101 | `(0,it.jsx)(BI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/101/static/js/main.f1e7055d.js@4985148 | `(0,it.jsx)(ql,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/101/static/js/main.f1e7055d.js@4985195 | `(0,it.jsx)(ER,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/101/static/js/main.f1e7055d.js@4985242 | `(0,it.jsx)(bm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 103 · RAZER NAGA TRINITY

类别：MOUSE；edition：[0, 128, 129, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/103/static/js/main.b753ffe5.js@5047345 | `(0,Jt.jsx)(Sl,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 78 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/103/static/js/main.b753ffe5.js@5047442 | `(0,Jt.jsx)(hS,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/103/static/js/main.b753ffe5.js@5047489 | `(0,Jt.jsx)(kA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/103/static/js/main.b753ffe5.js@5047536 | `(0,Jt.jsx)(vR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/103/static/js/main.b753ffe5.js@5047583 | `(0,Jt.jsx)(RM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 104 · Razer Mamba Hyperflux

类别：MOUSEPLUSMAT；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/104/static/js/main.9b94769e.js@5015635 | `(0,vt.jsx)(nI,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 65 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/104/static/js/main.9b94769e.js@5015776 | `(0,vt.jsx)(pA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/104/static/js/main.9b94769e.js@5015867 | `(0,vt.jsx)(wS,{})` | 42 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/104/static/js/main.9b94769e.js@5015914 | `(0,vt.jsx)(HR,{pId:this.props.pId})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/104/static/js/main.9b94769e.js@5016023 | `(0,vt.jsx)(lM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 105 · Razer Mamba Hyperflux

类别：MOUSEPLUSMAT；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/105/static/js/main.ef834389.js@5015635 | `(0,vt.jsx)(nI,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 65 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/105/static/js/main.ef834389.js@5015776 | `(0,vt.jsx)(pA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/105/static/js/main.ef834389.js@5015867 | `(0,vt.jsx)(wS,{})` | 42 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/105/static/js/main.ef834389.js@5015914 | `(0,vt.jsx)(HR,{pId:this.props.pId})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/105/static/js/main.ef834389.js@5016023 | `(0,vt.jsx)(lM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 106 · D.VA Razer Abyssus Elite

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/106/static/js/main.4d2daa0b.js@4964090 | `(0,it.jsx)(EP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 62 | 23 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/106/static/js/main.4d2daa0b.js@4964187 | `(0,it.jsx)(Eh,{})` | 25 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/106/static/js/main.4d2daa0b.js@4964234 | `(0,it.jsx)(km,{})` | 33 | 17 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/106/static/js/main.4d2daa0b.js@4964281 | `(0,it.jsx)(iL,{})` | 19 | 2 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/106/static/js/main.4d2daa0b.js@4964328 | `(0,it.jsx)(Sc,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 107 · Razer Abyssus Essential

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/107/static/js/main.eca0fb8e.js@4914941 | `(0,tt.jsx)(eP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/107/static/js/main.eca0fb8e.js@4915038 | `(0,tt.jsx)(eh,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/107/static/js/main.eca0fb8e.js@4915085 | `(0,tt.jsx)(wm,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/107/static/js/main.eca0fb8e.js@4915132 | `(0,tt.jsx)(tL,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/107/static/js/main.eca0fb8e.js@4915179 | `(0,tt.jsx)(Ac,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 108 · Razer Mamba Elite

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/108/static/js/main.e453e8a5.js@4948901 | `(0,it.jsx)(NT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/108/static/js/main.e453e8a5.js@4948998 | `(0,it.jsx)(FI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/108/static/js/main.e453e8a5.js@4949045 | `(0,it.jsx)(Ql,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/108/static/js/main.e453e8a5.js@4949092 | `(0,it.jsx)(tR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/108/static/js/main.e453e8a5.js@4949139 | `(0,it.jsx)(Ym,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 110 · Razer DeathAdder Essential

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/110/static/js/main.6e992750.js@4715839 | `(0,Ka.jsx)(rD,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 62 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/110/static/js/main.6e992750.js@4715936 | `(0,Ka.jsx)(eP,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/110/static/js/main.6e992750.js@4715983 | `(0,Ka.jsx)(bD,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/110/static/js/main.6e992750.js@4716030 | `(0,Ka.jsx)(Yd,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 112 · Razer Lancehead Wireless

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [111]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/112/static/js/main.47216244.js@4992606 | `(0,Ht.jsx)(kT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/112/static/js/main.47216244.js@4992703 | `(0,Ht.jsx)(sA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/112/static/js/main.47216244.js@4992750 | `(0,Ht.jsx)(PS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/112/static/js/main.47216244.js@4992797 | `(0,Ht.jsx)(WR,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/112/static/js/main.47216244.js@4992844 | `(0,Ht.jsx)(hR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/112/static/js/main.47216244.js@4992891 | `(0,Ht.jsx)(DM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 113 · Razer DeathAdder Essential

类别：MOUSE；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/113/static/js/main.c8b01b6c.js@4720370 | `(0,Ka.jsx)(ID,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/113/static/js/main.c8b01b6c.js@4720467 | `(0,Ka.jsx)(aP,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/113/static/js/main.c8b01b6c.js@4720514 | `(0,Ka.jsx)(KD,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/113/static/js/main.c8b01b6c.js@4720561 | `(0,Ka.jsx)(Yd,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 115 · Razer Mamaba Wireless

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [114]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/115/static/js/main.414c4a81.js@4980277 | `(0,Pt.jsx)(yT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/115/static/js/main.414c4a81.js@4980374 | `(0,Pt.jsx)(RS,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/115/static/js/main.414c4a81.js@4980421 | `(0,Pt.jsx)(_l,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/115/static/js/main.414c4a81.js@4980468 | `(0,Pt.jsx)(HR,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/115/static/js/main.414c4a81.js@4980515 | `(0,Pt.jsx)(uR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/115/static/js/main.414c4a81.js@4980562 | `(0,Pt.jsx)(RM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 116 · Razer Abyssus Lite

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/116/static/js/main.2e83bad2.js@4938552 | `(0,Et.jsx)($L,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/116/static/js/main.2e83bad2.js@4938649 | `(0,Et.jsx)($M,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/116/static/js/main.2e83bad2.js@4938696 | `(0,Et.jsx)(Km,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/116/static/js/main.2e83bad2.js@4938743 | `(0,Et.jsx)(EL,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/116/static/js/main.2e83bad2.js@4938790 | `(0,Et.jsx)(Oc,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 117 · Razer Turret Mouse Xbox One Edition

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2308]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/117/static/js/main.479a8db6.js@4956032 | `(0,nt.jsx)(RT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 61 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/117/static/js/main.479a8db6.js@4956129 | `(0,nt.jsx)(BI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/117/static/js/main.479a8db6.js@4956176 | `(0,nt.jsx)(LS,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/117/static/js/main.479a8db6.js@4956223 | `(0,nt.jsx)(wR,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/117/static/js/main.479a8db6.js@4956270 | `(0,nt.jsx)(MR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/117/static/js/main.479a8db6.js@4956317 | `(0,nt.jsx)(mM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 120 · Razer Viper

类别：MOUSE；edition：[0, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/120/static/js/main.026ff14a.js@4980156 | `(0,ct.jsx)(GT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/120/static/js/main.026ff14a.js@4980253 | `(0,ct.jsx)(jI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/120/static/js/main.026ff14a.js@4980300 | `(0,ct.jsx)(sS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/120/static/js/main.026ff14a.js@4980347 | `(0,ct.jsx)(OR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/120/static/js/main.026ff14a.js@4980394 | `(0,ct.jsx)(qm,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/120/static/js/main.026ff14a.js@4980441 | `(0,ct.jsx)(qm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 122 · Razer Viper Ultimate

类别：MOUSE；edition：[0, 128, 129, 130, 131, 132]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [123]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/122/static/js/main.4197e469.js@4982878 | `(0,Bt.jsx)(xT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 64 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/122/static/js/main.4197e469.js@4982975 | `(0,Bt.jsx)(TA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/122/static/js/main.4197e469.js@4983022 | `(0,Bt.jsx)(PS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/122/static/js/main.4197e469.js@4983069 | `(0,Bt.jsx)(xR,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/122/static/js/main.4197e469.js@4983116 | `(0,Bt.jsx)(HR,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/122/static/js/main.4197e469.js@4983163 | `(0,Bt.jsx)(MM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 124 · Razer DeathAdder V2 Pro

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [125]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [142]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/124/static/js/main.564e2add.js@5022368 | `(0,Gt.jsx)(QP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 76 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/124/static/js/main.564e2add.js@5022465 | `(0,Gt.jsx)(JU,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/124/static/js/main.564e2add.js@5022512 | `(0,Gt.jsx)(Wh,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/124/static/js/main.564e2add.js@5022559 | `(0,Gt.jsx)(IG,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/124/static/js/main.564e2add.js@5022606 | `(0,Gt.jsx)(WN,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 126 · Razer Mouse Dock

类别：ACCESSORY；edition：[0, 128, 129, 130, 131, 132]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/126/static/js/main.766604be.js@4559753 | `(0,WE.jsx)(SO.default,{})` | 50 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/126/static/js/main.766604be.js@4559808 | `(0,WE.jsx)(tL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 128 · Razer Pro Click

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [119]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [118]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/128/static/js/main.1c01e761.js@4714626 | `(0,$a.jsx)(KD,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 61 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/128/static/js/main.1c01e761.js@4714722 | `(0,$a.jsx)(AP,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/128/static/js/main.1c01e761.js@4714768 | `(0,$a.jsx)($D,{})` | 13 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/128/static/js/main.1c01e761.js@4714814 | `(0,$a.jsx)(rc,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 131 · Razer Basilisk X Hyperspeed

类别：MOUSE；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [131]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [130]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/131/static/js/main.b7514675.js@4891099 | `(0,Dt.jsx)(BI,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 64 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/131/static/js/main.b7514675.js@4891196 | `(0,Dt.jsx)($A,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/131/static/js/main.b7514675.js@4891243 | `(0,Dt.jsx)(NS,{})` | 13 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/131/static/js/main.b7514675.js@4891290 | `(0,Dt.jsx)(hp,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/131/static/js/main.b7514675.js@4927553 | `(0,Dt.jsx)(lM,{deviceInfo:se.DeviceInfo,deviceName:se.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 132 · RAZER DEATHADDER V2

类别：MOUSE；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/132/static/js/main.6b45cd12.js@4956167 | `(0,_t.jsx)(vO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/132/static/js/main.6b45cd12.js@4956264 | `(0,_t.jsx)(Xl,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/132/static/js/main.6b45cd12.js@4956311 | `(0,_t.jsx)(vA,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/132/static/js/main.6b45cd12.js@4956358 | `(0,_t.jsx)(tR,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/132/static/js/main.6b45cd12.js@4956405 | `(0,_t.jsx)(wm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 133 · Razer Basilisk V2

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/133/static/js/main.84fbd82a.js@4946000 | `(0,ot.jsx)(ET,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 64 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/133/static/js/main.84fbd82a.js@4946097 | `(0,ot.jsx)(cI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/133/static/js/main.84fbd82a.js@4946144 | `(0,ot.jsx)(jl,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/133/static/js/main.84fbd82a.js@4946191 | `(0,ot.jsx)(_R,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/133/static/js/main.84fbd82a.js@4946238 | `(0,ot.jsx)(Vm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 134 · Razer Basilisk Ultimate

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [136]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/134/static/js/main.88c1e1a3.js@4950876 | `(0,dt.jsx)(Ln,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 64 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/134/static/js/main.88c1e1a3.js@4950973 | `(0,dt.jsx)(fp,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/134/static/js/main.88c1e1a3.js@4951020 | `(0,dt.jsx)(HC,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/134/static/js/main.88c1e1a3.js@4951067 | `(0,dt.jsx)(kp,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/134/static/js/main.88c1e1a3.js@4951114 | `(0,dt.jsx)(No,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/134/static/js/main.88c1e1a3.js@4951161 | `(0,dt.jsx)(bN,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 138 · Razer Viper Mini

类别：MOUSE；edition：[0, 128, 129, 131, 132]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/138/static/js/main.358f6b29.js@4947122 | `(0,Nt.jsx)(iP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/138/static/js/main.358f6b29.js@4947219 | `(0,Nt.jsx)(ih,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/138/static/js/main.358f6b29.js@4947266 | `(0,Nt.jsx)(jm,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/138/static/js/main.358f6b29.js@4947313 | `(0,Nt.jsx)(rL,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/138/static/js/main.358f6b29.js@4947360 | `(0,Nt.jsx)(gc,{resetObm:()=>{var e;null===(e=Be.mwBroadcastChannel)&#124;&#124;void 0===e&#124;&#124;e.postMessage({type:"ON_RESET_OBM",timerTick:void 0,payload:{}})}})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 140 · Razer Deathadder V2 Lite

类别：MOUSE；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/140/static/js/main.db53a005.js@4937720 | `(0,at.jsx)(GO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/140/static/js/main.db53a005.js@4937817 | `(0,at.jsx)(wl,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/140/static/js/main.db53a005.js@4937864 | `(0,at.jsx)(GA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/140/static/js/main.db53a005.js@4937911 | `(0,at.jsx)(ZS,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/140/static/js/main.db53a005.js@4937958 | `(0,at.jsx)(Bm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 141 · Razer Naga Left Handed Edition

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/141/static/js/main.aa4788cc.js@4997750 | `(0,Et.jsx)(Al,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/141/static/js/main.aa4788cc.js@4997847 | `(0,Et.jsx)(pS,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/141/static/js/main.aa4788cc.js@4997894 | `(0,Et.jsx)(zA,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/141/static/js/main.aa4788cc.js@4997941 | `(0,Et.jsx)(vR,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/141/static/js/main.aa4788cc.js@4997988 | `(0,Et.jsx)(dM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 143 · RAZER NAGA PRO

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [144]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [146]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/143/static/js/main.3eb704f1.js@5112712 | `(0,TE.jsx)(CA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 91 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/143/static/js/main.3eb704f1.js@5112809 | `(0,TE.jsx)(VO,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/143/static/js/main.3eb704f1.js@5112856 | `(0,TE.jsx)(OR,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/143/static/js/main.3eb704f1.js@5112903 | `(0,TE.jsx)(Bc,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/143/static/js/main.3eb704f1.js@5112950 | `(0,TE.jsx)(cU,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 145 · Razer Viper 8Khz

类别：MOUSE；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/145/static/js/main.24a35fa5.js@4968171 | `(0,st.jsx)(DT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 64 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/145/static/js/main.24a35fa5.js@4968268 | `(0,st.jsx)(VI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/145/static/js/main.24a35fa5.js@4968315 | `(0,st.jsx)(ES,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/145/static/js/main.24a35fa5.js@4968362 | `(0,st.jsx)(IR,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/145/static/js/main.24a35fa5.js@4968409 | `(0,st.jsx)(Xm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 147 · RAZER NAGA CLASSIC EDITION

类别：MOUSE；edition：[0, 128, 129, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/147/static/js/main.885c5195.js@5027753 | `(0,JE.jsx)(Sl,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 78 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/147/static/js/main.885c5195.js@5027850 | `(0,JE.jsx)(hS,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/147/static/js/main.885c5195.js@5027897 | `(0,JE.jsx)(kO,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/147/static/js/main.885c5195.js@5027944 | `(0,JE.jsx)(vR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/147/static/js/main.885c5195.js@5027991 | `(0,JE.jsx)(RM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 148 · Razer Orochi V2

类别：MOUSE；edition：[0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 138]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [148]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [149]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/148/static/js/main.7e209fb0.js@4942391 | `(0,mt.jsx)(dO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 76 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/148/static/js/main.7e209fb0.js@4942488 | `(0,mt.jsx)(Bl,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/148/static/js/main.7e209fb0.js@4942535 | `(0,mt.jsx)(aR,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/148/static/js/main.7e209fb0.js@4942582 | `(0,mt.jsx)(OM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/148/static/js/main.7e209fb0.js@4954747 | `(0,mt.jsx)(vU,{deviceInfo:Te.DeviceInfo,deviceName:Te.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 150 · Razer Naga X

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/150/static/js/main.54e6989b.js@4989246 | `(0,Et.jsx)(sl,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/150/static/js/main.54e6989b.js@4989343 | `(0,Et.jsx)(uS,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/150/static/js/main.54e6989b.js@4989390 | `(0,Et.jsx)(KA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/150/static/js/main.54e6989b.js@4989437 | `(0,Et.jsx)(MR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/150/static/js/main.54e6989b.js@4989484 | `(0,Et.jsx)(TM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 152 · Razer DeathAdder Essential

类别：MOUSE；edition：[0, 128, 130, 131, 132, 133, 134, 135, 136, 137, 144, 145]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/152/static/js/main.265c9698.js@4726955 | `(0,ja.jsx)(ND,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/152/static/js/main.265c9698.js@4727052 | `(0,ja.jsx)(sP,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/152/static/js/main.265c9698.js@4727099 | `(0,ja.jsx)(jD,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/152/static/js/main.265c9698.js@4727146 | `(0,ja.jsx)(Kd,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 153 · Razer Basilisk V3

类别：MOUSE；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/153/static/js/main.9095b82c.js@4954844 | `(0,Nt.jsx)(CT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 66 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/153/static/js/main.9095b82c.js@4954941 | `(0,Nt.jsx)(yI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/153/static/js/main.9095b82c.js@4954988 | `(0,Nt.jsx)(AS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/153/static/js/main.9095b82c.js@4955035 | `(0,Nt.jsx)(uR,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/153/static/js/main.9095b82c.js@4955082 | `(0,Nt.jsx)(tM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 154 · Razer Pro Click Mini

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [154]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [155]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/154/static/js/main.35e3c57f.js@4783977 | `(0,e_.jsx)(dP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/154/static/js/main.35e3c57f.js@4784073 | `(0,e_.jsx)(wp,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/154/static/js/main.35e3c57f.js@4784119 | `(0,e_.jsx)(hP,{})` | 13 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/154/static/js/main.35e3c57f.js@4784165 | `(0,e_.jsx)(Tc,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 156 · Razer DeathAdder V2 X Hyperspeed

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [156]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [157]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/156/static/js/main.9543954c.js@4918623 | `(0,Pt.jsx)(lO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 76 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/156/static/js/main.9543954c.js@4918720 | `(0,Pt.jsx)(HS,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/156/static/js/main.9543954c.js@4918767 | `(0,Pt.jsx)(Xl,{})` | 13 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/156/static/js/main.9543954c.js@4918814 | `(0,Pt.jsx)(_M,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/156/static/js/main.9543954c.js@4930886 | `(0,Pt.jsx)(pU,{deviceInfo:se.DeviceInfo,deviceName:se.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 158 · Razer Viper Mini Signature Edition

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [159]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/158/static/js/main.dc9fe529.js@4930562 | `(0,Gt.jsx)(HP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 75 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/158/static/js/main.dc9fe529.js@4930659 | `(0,Gt.jsx)(Xp,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/158/static/js/main.dc9fe529.js@4930706 | `(0,Gt.jsx)(oM,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/158/static/js/main.dc9fe529.js@4930753 | `(0,Gt.jsx)(pD,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/158/static/js/main.dc9fe529.js@4930800 | `(0,Gt.jsx)(Wc,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/158/static/js/main.dc9fe529.js@4943590 | `(0,Gt.jsx)(fM,{deviceInfo:re.DeviceInfo,deviceName:re.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 161 · Razer Deathadder V2 Lite

类别：MOUSE；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/161/static/js/main.ba9a72b0.js@4937626 | `(0,at.jsx)(GO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/161/static/js/main.ba9a72b0.js@4937723 | `(0,at.jsx)(wl,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/161/static/js/main.ba9a72b0.js@4937770 | `(0,at.jsx)(GA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/161/static/js/main.ba9a72b0.js@4937817 | `(0,at.jsx)(ZS,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/161/static/js/main.ba9a72b0.js@4937864 | `(0,at.jsx)(Bm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 162 · cobra

类别：productCategoryIcon；edition：[]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/162/static/js/main.4dce6fd2.js@3718980 | `(0,on.jsx)(Jc,{setDisplaySaveAlertRef:r.setDisplaySaveAlertRef})` | 1 | 2 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/162/static/js/main.4dce6fd2.js@3719084 | `(0,on.jsx)(i$,{})` | 10 | 21 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/162/static/js/main.4dce6fd2.js@3719143 | `(0,on.jsx)(CJ,{})` | 12 | 24 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/162/static/js/main.4dce6fd2.js@3719199 | `(0,on.jsx)(Xc,{})` | 14 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/162/static/js/main.4dce6fd2.js@3719258 | `(0,on.jsx)(Ca,{resetObm:r.props.obmResetDevice})` | 4 | 9 | partial_static_reference_graph | partial_native |

## 163 · Razer Cobra

类别：MOUSE；edition：[0, 128, 129, 130, 131, 132, 133, 134]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/163/static/js/main.efd0b513.js@4968295 | `(0,Nt.jsx)(iP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/163/static/js/main.efd0b513.js@4968392 | `(0,Nt.jsx)(ih,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/163/static/js/main.efd0b513.js@4968439 | `(0,Nt.jsx)(jm,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/163/static/js/main.efd0b513.js@4968486 | `(0,Nt.jsx)(rL,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/163/static/js/main.efd0b513.js@4968533 | `(0,Nt.jsx)(gc,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 164 · Razer Mouse Dock Pro

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/164/static/js/main.458d4103.js@4575203 | `(0,w.jsx)(Fr.default,{})` | 37 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/164/static/js/main.458d4103.js@4575256 | `(0,w.jsx)(Br,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/164/static/js/main.458d4103.js@4575301 | `(0,w.jsx)(iu,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/164/static/js/main.458d4103.js@4587916 | `(0,w.jsx)(ED,{deviceInfo:Ce.DeviceInfo,deviceName:Ce.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 165 · RAZER VIPER V2 PRO

类别：MOUSE；edition：[0, 128, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [166]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/165/static/js/main.571c242f.js@4771940 | `(0,Za.jsx)(cI,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/165/static/js/main.571c242f.js@4772037 | `(0,Za.jsx)(yO,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/165/static/js/main.571c242f.js@4772084 | `(0,Za.jsx)(oA,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/165/static/js/main.571c242f.js@4772131 | `(0,Za.jsx)(XO,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/165/static/js/main.571c242f.js@4772178 | `(0,Za.jsx)(yp,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 167 · RAZER NAGA V2 PRO

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [168]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [169]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/167/static/js/main.508445ac.js@5064497 | `(0,RE.jsx)(UO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 92 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/167/static/js/main.508445ac.js@5064594 | `(0,RE.jsx)(jl,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_SCROLLING / default | .ref/devices/167/static/js/main.508445ac.js@5064641 | `(0,RE.jsx)(CS,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/167/static/js/main.508445ac.js@5064688 | `(0,RE.jsx)(Fc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/167/static/js/main.508445ac.js@5064735 | `(0,RE.jsx)(oN,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/167/static/js/main.508445ac.js@5064782 | `(0,RE.jsx)(KU,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/167/static/js/main.508445ac.js@5103692 | `(0,RE.jsx)(MG,{deviceInfo:le.DeviceInfo,deviceName:le.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 170 · Razer Basilisk V3 Pro

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [171]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [172]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/170/static/js/main.7a4583ef.js@4917850 | `(0,St.jsx)(YO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 81 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/170/static/js/main.7a4583ef.js@4917947 | `(0,St.jsx)($A,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/170/static/js/main.7a4583ef.js@4917994 | `(0,St.jsx)(Ld,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/170/static/js/main.7a4583ef.js@4918041 | `(0,St.jsx)(Bd,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/170/static/js/main.7a4583ef.js@4918088 | `(0,St.jsx)(kd,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/170/static/js/main.7a4583ef.js@4918135 | `(0,St.jsx)(Ih,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/170/static/js/main.7a4583ef.js@4932899 | `(0,St.jsx)(bU,{deviceInfo:Te.DeviceInfo,deviceName:Te.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 175 · Razer Cobra Pro

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [176]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [177]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/175/static/js/main.f6dfd3ef.js@4931209 | `(0,Pt.jsx)(TP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 77 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/175/static/js/main.f6dfd3ef.js@4931306 | `(0,Pt.jsx)(rU,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/175/static/js/main.f6dfd3ef.js@4931353 | `(0,Pt.jsx)($M,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/175/static/js/main.f6dfd3ef.js@4931400 | `(0,Pt.jsx)(CU,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/175/static/js/main.f6dfd3ef.js@4931447 | `(0,Pt.jsx)(au,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/175/static/js/main.f6dfd3ef.js@4956821 | `(0,Pt.jsx)(xU,{deviceInfo:Te.DeviceInfo,deviceName:Te.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 178 · Razer DeathAdder V3

类别：MOUSE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/178/static/js/main.963979c0.js@4788531 | `(0,ht.jsx)(LL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/178/static/js/main.963979c0.js@4788628 | `(0,ht.jsx)(yp,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/178/static/js/main.963979c0.js@4788675 | `(0,ht.jsx)(LD,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/178/static/js/main.963979c0.js@4788722 | `(0,ht.jsx)(Yc,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 179 · HyperPolling Wireless Dongle

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/179/static/js/main.4849f7ca.js@4570389 | `(0,y.jsx)(kl.default,{})` | 35 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/179/static/js/main.4849f7ca.js@4570442 | `(0,y.jsx)(zl,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 180 · Razer Naga V2 Hyperspeed

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [181]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/180/static/js/main.ca4353ee.js@4868793 | `(0,at.jsx)(XP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 88 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/180/static/js/main.ca4353ee.js@4868890 | `(0,at.jsx)(uM,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/180/static/js/main.ca4353ee.js@4868937 | `(0,at.jsx)(vM,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/180/static/js/main.ca4353ee.js@4868984 | `(0,at.jsx)(ZC,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/180/static/js/main.ca4353ee.js@4998780 | `(0,at.jsx)(FG,{deviceInfo:Ae.DeviceInfo,deviceName:Ae.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 182 · Razer DeathAdder V3 Pro

类别：MOUSE；edition：[0, 128, 129, 130, 131, 132]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [183]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/182/static/js/main.db20a7c4.js@4856796 | `(0,gt.jsx)(em,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 77 | 7 | partial_static_reference_graph | partial_native_reaudited |
| TAB_PERFORMANCE / default | .ref/devices/182/static/js/main.db20a7c4.js@4856893 | `(0,gt.jsx)(lM,{})` | 27 | 0 | partial_static_reference_graph | partial_native_reaudited |
| TAB_POWER / default | .ref/devices/182/static/js/main.db20a7c4.js@4856940 | `(0,gt.jsx)(MM,{})` | 15 | 0 | partial_static_reference_graph | partial_native_reaudited |
| TAB_CALIBRATION / default | .ref/devices/182/static/js/main.db20a7c4.js@4856987 | `(0,gt.jsx)(mD,{})` | 18 | 0 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/182/static/js/main.db20a7c4.js@4857034 | `(0,gt.jsx)(KN,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |
| TAB_PAIRING / multiDevicePairing | .ref/devices/182/static/js/main.db20a7c4.js@4963412 | `(0,gt.jsx)(mG,{deviceInfo:Ae.DeviceInfo,deviceName:Ae.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 184 · Razer Viper V3 HyperSpeed

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [184]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/184/static/js/main.8f62d76a.js@4752156 | `(0,xa.jsx)(NO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 75 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/184/static/js/main.8f62d76a.js@4752253 | `(0,xa.jsx)(BA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/184/static/js/main.8f62d76a.js@4752300 | `(0,xa.jsx)(nS,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/184/static/js/main.8f62d76a.js@4752347 | `(0,xa.jsx)(kA,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/184/static/js/main.8f62d76a.js@4752394 | `(0,xa.jsx)(bM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/184/static/js/main.8f62d76a.js@4786922 | `(0,xa.jsx)($M,{deviceInfo:se.DeviceInfo,deviceName:se.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 185 · Razer Basilisk V3 X Hyperspeed

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [185]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [186]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/185/static/js/main.31b0a4d2.js@5049030 | `(0,fE.jsx)(HO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 80 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/185/static/js/main.31b0a4d2.js@5049127 | `(0,fE.jsx)(ql,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/185/static/js/main.31b0a4d2.js@5049174 | `(0,fE.jsx)(OG,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/185/static/js/main.31b0a4d2.js@5049221 | `(0,fE.jsx)(mG,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/185/static/js/main.31b0a4d2.js@5049268 | `(0,fE.jsx)(od,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/185/static/js/main.31b0a4d2.js@5049315 | `(0,fE.jsx)(dM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/185/static/js/main.31b0a4d2.js@5087870 | `(0,fE.jsx)(KG,{deviceInfo:Se.DeviceInfo,deviceName:Se.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 190 · Razer DeathAdder V4 Pro

类别：MOUSE；edition：[0, 128, 130, 131, 132, 133]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [191]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/190/static/js/main.81b09779.js@4794403 | `(0,Ga.jsx)(vO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 78 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/190/static/js/main.81b09779.js@4794500 | `(0,Ga.jsx)(cS,{})` | 37 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/190/static/js/main.81b09779.js@4794547 | `(0,Ga.jsx)(cl,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/190/static/js/main.81b09779.js@4794594 | `(0,Ga.jsx)(GS,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| ADVANCED / default | .ref/devices/190/static/js/main.81b09779.js@4794641 | `(0,Ga.jsx)(sl,{})` | 29 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/190/static/js/main.81b09779.js@4794688 | `(0,Ga.jsx)(fm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/190/static/js/main.81b09779.js@4828393 | `(0,Ga.jsx)(Xm,{deviceInfo:oe.DeviceInfo,deviceName:oe.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 192 · Razer Viper V3 Pro

类别：MOUSE；edition：[0, 128, 129, 130, 131, 132, 133, 134, 135]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [193]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/192/static/js/main.a839b83d.js@4998719 | `(0,Wt.jsx)(nO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 75 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/192/static/js/main.a839b83d.js@4998816 | `(0,Wt.jsx)(Zl,{})` | 37 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/192/static/js/main.a839b83d.js@4998863 | `(0,Wt.jsx)(qS,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/192/static/js/main.a839b83d.js@4998910 | `(0,Wt.jsx)(iS,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| ADVANCED / default | .ref/devices/192/static/js/main.a839b83d.js@4998957 | `(0,Wt.jsx)(yS,{})` | 29 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/192/static/js/main.a839b83d.js@4999004 | `(0,Wt.jsx)(UU,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/192/static/js/main.a839b83d.js@5033685 | `(0,Wt.jsx)(KU,{deviceInfo:Ae.DeviceInfo,deviceName:Ae.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 194 · Razer DeathAdder V3 Pro Hyperpolling Technology

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [195]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/194/static/js/main.6e5e136c.js@4908628 | `(0,vt.jsx)(YL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/194/static/js/main.6e5e136c.js@4908725 | `(0,vt.jsx)($p,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/194/static/js/main.6e5e136c.js@4908772 | `(0,vt.jsx)(OP,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/194/static/js/main.6e5e136c.js@4908819 | `(0,vt.jsx)(UD,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/194/static/js/main.6e5e136c.js@4908866 | `(0,vt.jsx)(kN,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 196 · Razer DeathAdder V3 HyperSpeed

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [197]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/196/static/js/main.bbc961f9.js@4797555 | `(0,ht.jsx)(eA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 75 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/196/static/js/main.bbc961f9.js@4797652 | `(0,ht.jsx)(NS,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/196/static/js/main.bbc961f9.js@4797699 | `(0,ht.jsx)(cl,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/196/static/js/main.bbc961f9.js@4797746 | `(0,ht.jsx)(US,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| ADVANCED / default | .ref/devices/196/static/js/main.bbc961f9.js@4797793 | `(0,ht.jsx)(nl,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/196/static/js/main.bbc961f9.js@4797840 | `(0,ht.jsx)(qm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/196/static/js/main.bbc961f9.js@4832692 | `(0,ht.jsx)(TU,{deviceInfo:se.DeviceInfo,deviceName:se.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 199 · Razer Pro Click V2 Vertical Edition

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [200]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [201]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/199/static/js/683.8755a8fc.chunk.js@228196 | `(0,V.jsx)(Ea,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 70 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/199/static/js/683.8755a8fc.chunk.js@228291 | `(0,V.jsx)(Bi,{})` | 23 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/199/static/js/683.8755a8fc.chunk.js@228336 | `(0,V.jsx)(_a.A,{})` | 36 | 7 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/199/static/js/683.8755a8fc.chunk.js@228383 | `(0,V.jsx)(pa,{resetObm:this.props.obmResetDevice})` | 25 | 1 | partial_static_reference_graph | partial_native |

## 203 · Razer Basilisk V3 35K

类别：MOUSE；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/203/static/js/main.fe8e2b48.js@4906404 | `(0,vt.jsx)(yT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 68 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/203/static/js/main.fe8e2b48.js@4906501 | `(0,vt.jsx)(LO,{})` | 37 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/203/static/js/main.fe8e2b48.js@4906548 | `(0,vt.jsx)(JS,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/203/static/js/main.fe8e2b48.js@4906595 | `(0,vt.jsx)(sR,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/203/static/js/main.fe8e2b48.js@4906642 | `(0,vt.jsx)(km,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 204 · Razer Basilisk V3 Pro 35K

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [205]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [206]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/204/static/js/main.97f302da.js@4947894 | `(0,Ht.jsx)(_A,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 83 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/204/static/js/main.97f302da.js@4947991 | `(0,Ht.jsx)(wl,{isBle:this.props.isBle})` | 37 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/204/static/js/main.97f302da.js@4948060 | `(0,Ht.jsx)(dN,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/204/static/js/main.97f302da.js@4948107 | `(0,Ht.jsx)(UN,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/204/static/js/main.97f302da.js@4948154 | `(0,Ht.jsx)(YN,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/204/static/js/main.97f302da.js@4948201 | `(0,Ht.jsx)(_U,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/204/static/js/main.97f302da.js@4985364 | `(0,Ht.jsx)(FG,{deviceInfo:Ae.DeviceInfo,deviceName:Ae.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 207 · HyperFlux V2 Wireless Charging System

类别：MOUSEMAT；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/207/static/js/main.9b300921.js@4471112 | `(0,We.jsx)(ah,{})` | 48 | 3 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/207/static/js/main.9b300921.js@4471159 | `(0,We.jsx)(Jp,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/207/static/js/main.9b300921.js@4589774 | `(0,We.jsx)(Zg,{deviceInfo:$.DeviceInfo,deviceName:$.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 208 · Razer Pro Click V2

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [209]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [210]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/208/static/js/2833.ab2c9712.chunk.js@224119 | `(0,w.jsx)(Ni,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 71 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/208/static/js/2833.ab2c9712.chunk.js@224214 | `(0,w.jsx)(Gn,{})` | 24 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/208/static/js/2833.ab2c9712.chunk.js@224259 | `(0,w.jsx)(Ai.A,{})` | 37 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/208/static/js/2833.ab2c9712.chunk.js@224306 | `(0,w.jsx)(ui,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 211 · Razer Basilisk Mobile

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [212]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [213]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/211/static/js/main.8895a352.js@4872985 | `(0,lt.jsx)(WA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 77 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/211/static/js/main.8895a352.js@4873082 | `(0,lt.jsx)(tS,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/211/static/js/main.8895a352.js@4873129 | `(0,lt.jsx)(Bd,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/211/static/js/main.8895a352.js@4873176 | `(0,lt.jsx)(jd,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/211/static/js/main.8895a352.js@4873223 | `(0,lt.jsx)(jU,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/211/static/js/main.8895a352.js@4899197 | `(0,lt.jsx)(LG,{deviceInfo:Ae.DeviceInfo,deviceName:Ae.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 214 · Razer Basilisk V3 Pro 35K Phantom Green Edition

类别：MOUSE；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [215]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [216]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/214/static/js/main.9287a47f.js@4957352 | `(0,re.jsx)(fA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 85 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/214/static/js/main.9287a47f.js@4957449 | `(0,re.jsx)(RS,{isBle:this.props.isBle})` | 37 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/214/static/js/main.9287a47f.js@4957518 | `(0,re.jsx)(zN,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/214/static/js/main.9287a47f.js@4957565 | `(0,re.jsx)(ac,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/214/static/js/main.9287a47f.js@4957612 | `(0,re.jsx)(Oc,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/214/static/js/main.9287a47f.js@4957659 | `(0,re.jsx)(gU,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/214/static/js/main.9287a47f.js@4972321 | `(0,re.jsx)(ig,{deviceInfo:Ye.DeviceInfo,deviceName:Ye.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 218 · Razer Cobra HyperSpeed

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [219]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [220]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/218/static/js/main.f14e7b89.js@4928938 | `(0,Mt.jsx)(IP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 77 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/218/static/js/main.f14e7b89.js@4929035 | `(0,Mt.jsx)(IU,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/218/static/js/main.f14e7b89.js@4929082 | `(0,Mt.jsx)(Eh,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/218/static/js/main.f14e7b89.js@4929129 | `(0,Mt.jsx)(LU,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/218/static/js/main.f14e7b89.js@4929176 | `(0,Mt.jsx)(_u,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/218/static/js/main.f14e7b89.js@4976939 | `(0,Mt.jsx)(JU,{deviceInfo:Te.DeviceInfo,deviceName:Te.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 221 · Razer Boomslang 20th Anniversary Edition

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [221]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_PAIRING / multiDevicePairing | .ref/devices/221/static/js/1390.f96908c7.chunk.js@7410 | `(0,g.jsx)(f,{deviceInfo:y.DeviceInfo,deviceName:y.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |
| TAB_CUSTOMIZE / default | .ref/devices/221/static/js/7793.37ba37ac.chunk.js@101742 | `(0,X.jsx)(_t,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 58 | 9 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/221/static/js/7793.37ba37ac.chunk.js@101837 | `(0,X.jsx)(Xs,{})` | 21 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/221/static/js/7793.37ba37ac.chunk.js@101882 | `(0,X.jsx)(Pt.A,{})` | 31 | 11 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/221/static/js/7793.37ba37ac.chunk.js@101929 | `(0,X.jsx)(oi,{})` | 13 | 3 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/221/static/js/7793.37ba37ac.chunk.js@101974 | `(0,X.jsx)(Rt,{})` | 12 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/221/static/js/7793.37ba37ac.chunk.js@102019 | `(0,X.jsx)(ut,{resetObm:this.props.obmResetDevice})` | 24 | 4 | partial_static_reference_graph | partial_native |

## 222 · Razer Viper V3 Pro SE

类别：MOUSE；edition：[0, 128, 129, 130, 131]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [223]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/222/static/js/main.b8c354ff.js@4942360 | `(0,Wt.jsx)(MA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 77 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/222/static/js/main.b8c354ff.js@4942457 | `(0,Wt.jsx)(ll,{isBle:this.props.isBle})` | 37 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/222/static/js/main.b8c354ff.js@4942526 | `(0,Wt.jsx)(RR,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/222/static/js/main.b8c354ff.js@4942573 | `(0,Wt.jsx)(pl,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| ADVANCED / default | .ref/devices/222/static/js/main.b8c354ff.js@4942620 | `(0,Wt.jsx)(tR,{})` | 29 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/222/static/js/main.b8c354ff.js@4942667 | `(0,Wt.jsx)(jU,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/222/static/js/main.b8c354ff.js@4977376 | `(0,Wt.jsx)(sh,{deviceInfo:Ie.DeviceInfo,deviceName:Ie.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 224 · Razer Orochi V2

类别：MOUSE；edition：[0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 138]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [224]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [225]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/224/static/js/main.06abb0ec.js@4880589 | `(0,R_.jsx)(DI,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 63 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/224/static/js/main.06abb0ec.js@4880686 | `(0,R_.jsx)(yO,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/224/static/js/main.06abb0ec.js@4880733 | `(0,R_.jsx)(iS,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/224/static/js/main.06abb0ec.js@4880780 | `(0,R_.jsx)($P,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 226 · Razer Basilisk V4 Pro

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [227]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [228]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/226/static/js/8355.3d5e573e.chunk.js@275354 | `(0,w.jsx)(qe.A,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 83 | 8 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/226/static/js/8355.3d5e573e.chunk.js@275451 | `(0,w.jsx)(Ri,{isBle:this.props.isBle})` | 34 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/226/static/js/8355.3d5e573e.chunk.js@275518 | `(0,w.jsx)(Ii.A,{})` | 38 | 5 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/226/static/js/8355.3d5e573e.chunk.js@275565 | `(0,w.jsx)(Zi,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/226/static/js/8355.3d5e573e.chunk.js@275610 | `(0,w.jsx)(Qi,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| ADVANCED / default | .ref/devices/226/static/js/8355.3d5e573e.chunk.js@275655 | `(0,w.jsx)(Lt,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/226/static/js/8355.3d5e573e.chunk.js@275700 | `(0,w.jsx)(za,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 229 · Razer Viper V4 Pro

类别：MOUSE；edition：[0, 128, 129, 130, 131, 132, 133, 134]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [230]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/229/static/js/main.3e19820b.js@4559712 | `(0,Aa.jsx)(aT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 72 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/229/static/js/main.3e19820b.js@4559809 | `(0,Aa.jsx)(WO,{})` | 36 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/229/static/js/main.3e19820b.js@4559856 | `(0,Aa.jsx)(WI,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/229/static/js/main.3e19820b.js@4559903 | `(0,Aa.jsx)(QO,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| ADVANCED / default | .ref/devices/229/static/js/main.3e19820b.js@4559950 | `(0,Aa.jsx)(hI,{})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/229/static/js/main.3e19820b.js@4559997 | `(0,Aa.jsx)(qP,{resetObm:this.resetDevice})` | 25 | 0 | partial_static_reference_graph | partial_native |

## 231 · RAZER NAGA V3 PRO

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [232]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [233]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/231/static/js/main.741f0de8.js@5078920 | `(0,ot.jsx)(rO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 92 | 7 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/231/static/js/main.741f0de8.js@5079027 | `(0,ot.jsx)(QA,{})` | 38 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/231/static/js/main.741f0de8.js@5079086 | `(0,ot.jsx)(aN,{})` | 37 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/231/static/js/main.741f0de8.js@5079142 | `(0,ot.jsx)(MN,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/231/static/js/main.741f0de8.js@5079195 | `(0,ot.jsx)(xm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 235 · Razer Basilisk V4 HyperSpeed

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [234]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [236]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/235/static/js/7184.4ca1d2e9.chunk.js@263390 | `(0,R.jsx)(ot,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 2 | 1 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/235/static/js/7184.4ca1d2e9.chunk.js@263485 | `(0,R.jsx)(ci,{isBle:this.props.isBle})` | 32 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/235/static/js/7184.4ca1d2e9.chunk.js@263552 | `(0,R.jsx)(di.A,{})` | 36 | 7 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/235/static/js/7184.4ca1d2e9.chunk.js@263599 | `(0,R.jsx)(Ai,{})` | 11 | 1 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/235/static/js/7184.4ca1d2e9.chunk.js@263644 | `(0,R.jsx)(Ii,{})` | 16 | 2 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/235/static/js/7184.4ca1d2e9.chunk.js@263689 | `(0,R.jsx)(Ra,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 239 · Razer DeathAdder V4 Pro Carbon Fiber Edition

类别：MOUSE；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [240]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/239/static/js/main.6bc74a2d.js@4790634 | `(0,Fa.jsx)(qT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 76 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/239/static/js/main.6bc74a2d.js@4790731 | `(0,Fa.jsx)(yO,{})` | 37 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/239/static/js/main.6bc74a2d.js@4790778 | `(0,Fa.jsx)(YA,{})` | 16 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/239/static/js/main.6bc74a2d.js@4790825 | `(0,Fa.jsx)(XO,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| ADVANCED / default | .ref/devices/239/static/js/main.6bc74a2d.js@4790872 | `(0,Fa.jsx)(pA,{})` | 29 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/239/static/js/main.6bc74a2d.js@4790919 | `(0,Fa.jsx)(dM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 241 · Razer Mouse Dock V2 Pro

类别：ACCESSORY；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [241]}, {'source': 'AvailableDevices.json', 'kind': 'wiredId', 'ids': [241]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_PAIRING / default | .ref/devices/241/static/js/11.219fb515.chunk.js@311151 | `(0,C.jsx)(Oi.default,{})` | 23 | 2 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/241/static/js/11.219fb515.chunk.js@311204 | `(0,C.jsx)(Ii.A,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/241/static/js/11.219fb515.chunk.js@311251 | `(0,C.jsx)(Oa,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 515 · Razer Blackwidow Chroma

类别：KEYBOARD；edition：[0, 1, 2, 3]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/515/static/js/main.f60ca5aa.js@7532005 | `(0,za.jsx)(IO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 95 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/515/static/js/main.f60ca5aa.js@7532102 | `(0,za.jsx)(Nc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/515/static/js/main.f60ca5aa.js@7532149 | `(0,za.jsx)(jM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 521 · Razer Blackwidow TE Chroma V2

类别：KEYBOARD；edition：[0, 1, 128, 129, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/521/static/js/main.bbf02831.js@7177945 | `(0,ka.jsx)(SO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/521/static/js/main.bbf02831.js@7178042 | `(0,ka.jsx)(Nc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/521/static/js/main.bbf02831.js@7178089 | `(0,ka.jsx)($m,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 529 · Razer Blackwidow Chroma

类别：KEYBOARD；edition：[0, 1]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/529/static/js/main.c4a3f376.js@7063993 | `(0,Wa.jsx)(_O,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 95 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/529/static/js/main.c4a3f376.js@7064090 | `(0,Wa.jsx)(uc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/529/static/js/main.c4a3f376.js@7064137 | `(0,Wa.jsx)(xm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 534 · Razer Blackwidow X Chroma

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/534/static/js/main.0cc999dc.js@7261281 | `(0,ka.jsx)(SO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/534/static/js/main.0cc999dc.js@7261378 | `(0,ka.jsx)(Nc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/534/static/js/main.0cc999dc.js@7261425 | `(0,ka.jsx)($m,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 542 · Razer Ornata Chroma

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/542/static/js/main.1e52c858.js@7580076 | `(0,Wa.jsx)(Al,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/542/static/js/main.1e52c858.js@7580173 | `(0,Wa.jsx)(dc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/542/static/js/main.1e52c858.js@7580220 | `(0,Wa.jsx)(qm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 545 · Razer BlackWidow Chroma V2

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/545/static/js/main.c8080dd3.js@7697885 | `(0,Oa.jsx)(mO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 94 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/545/static/js/main.c8080dd3.js@7697982 | `(0,Oa.jsx)(kT,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/545/static/js/main.c8080dd3.js@7698029 | `(0,Oa.jsx)(dM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 550 · Razer Huntsman Elite

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/550/static/js/2485.d642b62c.chunk.js@84369 | `(0,h.jsx)(ue.kg,{lazy:()=>Promise.all([s.e(3679),s.e(4802),s.e(9784),s.e(8697)]).then(s.bind(s,78697)),setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 1 | 1 | partial_static_reference_graph / unique_owner_computed_router_key_matches_navigation_name | partial_native |
| TAB_LIGHTING / default | .ref/devices/550/static/js/2485.d642b62c.chunk.js@84387 | `(0,h.jsx)(ue.kg,{lazy:()=>Promise.all([s.e(712),s.e(3487)]).then(s.bind(s,712))})` | 1 | 1 | partial_static_reference_graph / unique_owner_computed_router_key_matches_navigation_name | partial_native |
| HELP / default | .ref/devices/550/static/js/2485.d642b62c.chunk.js@84405 | `(0,h.jsx)(ue.kg,{lazy:()=>Promise.all([s.e(1599),s.e(5722),s.e(8934)]).then(s.bind(s,88934)),resetObm:this.props.obmResetDevice})` | 1 | 1 | partial_static_reference_graph / unique_owner_computed_router_key_matches_navigation_name | partial_native |

## 551 · Razer Huntsman

类别：KEYBOARD；edition：[0, 128, 129, 130, 131]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [551]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/551/static/js/main.5f9f6113.js@7242440 | `(0,v.jsx)(oM,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/551/static/js/main.5f9f6113.js@7242536 | `(0,v.jsx)(RL,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/551/static/js/main.5f9f6113.js@7242582 | `(0,v.jsx)(WR,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 552 · Razer Blackwidow Elite

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/552/static/js/main.bdf1be1e.js@7382986 | `(0,za.jsx)(uO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 97 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/552/static/js/main.bdf1be1e.js@7383083 | `(0,za.jsx)(mc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/552/static/js/main.bdf1be1e.js@7383130 | `(0,za.jsx)(sM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 554 · Razer Cynosa Chroma

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/554/static/js/main.faed76d3.js@7555803 | `(0,wa.jsx)(ll,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/554/static/js/main.faed76d3.js@7555900 | `(0,wa.jsx)(Uc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/554/static/js/main.faed76d3.js@7555947 | `(0,wa.jsx)(Jm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 555 · RAZER TARTARUS V2

类别：KEYPAD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/555/static/js/main.110aac54.js@6702053 | `(0,Ya.jsx)(Ym,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 85 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/555/static/js/main.110aac54.js@6702150 | `(0,Ya.jsx)(vL,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/555/static/js/main.110aac54.js@6702197 | `(0,Ya.jsx)(hN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 556 · Razer Cynosa Chroma Pro

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/556/static/js/main.3c239c1f.js@6760034 | `(0,Ka.jsx)(dd,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/556/static/js/main.3c239c1f.js@6760131 | `(0,Ka.jsx)(UI,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/556/static/js/main.3c239c1f.js@6760178 | `(0,Ka.jsx)(Jm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 563 · Razer Blade 15

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/563/static/js/main.7084ce8a.js@7020070 | `(0,Vn.jsx)(SO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/563/static/js/main.7084ce8a.js@7020167 | `(0,Vn.jsx)(Pd,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/563/static/js/main.7084ce8a.js@7020214 | `(0,Vn.jsx)(dI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/563/static/js/main.7084ce8a.js@7020261 | `(0,Vn.jsx)(OM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 564 · Razer Blade Pro 17

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/564/static/js/main.1f11c347.js@7074402 | `(0,Eo.jsx)(qO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/564/static/js/main.1f11c347.js@7074499 | `(0,Eo.jsx)(TS,{})` | 43 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/564/static/js/main.1f11c347.js@7074546 | `(0,Eo.jsx)(ZI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/564/static/js/main.1f11c347.js@7074593 | `(0,Eo.jsx)(XM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 565 · Razer Blackwidow Lite

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/565/static/js/main.3280575a.js@6858001 | `(0,Fa.jsx)(JI,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 90 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/565/static/js/main.3280575a.js@6858098 | `(0,Fa.jsx)(s_,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/565/static/js/main.3280575a.js@6858145 | `(0,Fa.jsx)(TL,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 567 · Razer Blackwidow Essential

类别：KEYBOARD；edition：[0, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/567/static/js/main.33124f31.js@7584086 | `(0,wa.jsx)(iA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 95 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/567/static/js/main.33124f31.js@7584183 | `(0,wa.jsx)(Lr,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/567/static/js/main.33124f31.js@7584230 | `(0,wa.jsx)(dL,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 569 · Razer Blade Stealth 13

类别：SYSTEM；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/569/static/js/main.11459c95.js@7019823 | `(0,Zn.jsx)(BO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/569/static/js/main.11459c95.js@7019920 | `(0,Zn.jsx)(xd,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/569/static/js/main.11459c95.js@7019967 | `(0,Zn.jsx)(FI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/569/static/js/main.11459c95.js@7020014 | `(0,Zn.jsx)(UM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 570 · Razer Blade 15

类别：KEYBOARD, SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/570/static/js/main.ec14218a.js@6969252 | `(0,Zn.jsx)(BO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/570/static/js/main.ec14218a.js@6969349 | `(0,Zn.jsx)(xd,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/570/static/js/main.ec14218a.js@6969396 | `(0,Zn.jsx)(FI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/570/static/js/main.ec14218a.js@6969443 | `(0,Zn.jsx)(UM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 571 · Razer Blade 15

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/571/static/js/main.cf9862f1.js@6952801 | `(0,Yn.jsx)(uO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/571/static/js/main.cf9862f1.js@6952898 | `(0,Yn.jsx)(Pd,{})` | 36 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/571/static/js/main.cf9862f1.js@6952945 | `(0,Yn.jsx)(SI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/571/static/js/main.cf9862f1.js@6952992 | `(0,Yn.jsx)(OM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 574 · Turret Keyboard Xbox One Edition

类别：KEYBOARD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2308]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/574/static/js/main.4e1cb347.js@6806748 | `(0,Yn.jsx)(cp,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 95 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/574/static/js/main.4e1cb347.js@6806845 | `(0,Yn.jsx)(uM,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/574/static/js/main.4e1cb347.js@6806892 | `(0,Yn.jsx)(fM,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/574/static/js/main.4e1cb347.js@6806939 | `(0,Yn.jsx)(Zu,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 575 · Razer Cynosa Lite

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/575/static/js/main.e6809e11.js@7492060 | `(0,wa.jsx)(TO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 95 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/575/static/js/main.e6809e11.js@7492157 | `(0,wa.jsx)(Pc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/575/static/js/main.e6809e11.js@7492204 | `(0,wa.jsx)(lP,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 576 · Razer Blade 15

类别：KEYBOARD, SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/576/static/js/main.0fe2c485.js@7041713 | `(0,Vn.jsx)(SO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/576/static/js/main.0fe2c485.js@7041810 | `(0,Vn.jsx)(Pd,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/576/static/js/main.0fe2c485.js@7041857 | `(0,Vn.jsx)(dI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/576/static/js/main.0fe2c485.js@7041904 | `(0,Vn.jsx)(OM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 577 · Razer BlackWidow

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/577/static/js/main.7053d506.js@7655080 | `(0,ya.jsx)(jO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 97 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/577/static/js/main.7053d506.js@7655177 | `(0,ya.jsx)(rc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/577/static/js/main.7053d506.js@7655224 | `(0,ya.jsx)(BM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 579 · Razer Huntsman Tournament Edition

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/579/static/js/main.3355e99f.js@7153359 | `(0,Fa.jsx)(cM,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/579/static/js/main.3355e99f.js@7153456 | `(0,Fa.jsx)(RL,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/579/static/js/main.3355e99f.js@7153503 | `(0,Fa.jsx)(BR,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 580 · Tartarus Pro

类别：KEYPAD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/580/static/js/main.1648cf28.js@6924808 | `()=>(0,Ln.jsx)(Fc,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 79 | 4 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/580/static/js/main.1648cf28.js@6924915 | `()=>(0,Ln.jsx)(kd,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 57 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/580/static/js/main.1648cf28.js@6925069 | `()=>(0,Ln.jsx)(UI,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/580/static/js/main.1648cf28.js@6925126 | `()=>(0,Ln.jsx)(oP,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 581 · Razer Blade 15

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/581/static/js/main.1347344b.js@6956663 | `(0,Yn.jsx)(uO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/581/static/js/main.1347344b.js@6956760 | `(0,Yn.jsx)(Md,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/581/static/js/main.1347344b.js@6956807 | `(0,Yn.jsx)(SI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/581/static/js/main.1347344b.js@6956854 | `(0,Yn.jsx)(dM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 582 · Razer Blade 15

类别：SYSTEM；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/582/static/js/main.746928a2.js@6968107 | `(0,Zn.jsx)(BO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/582/static/js/main.746928a2.js@6968204 | `(0,Zn.jsx)(xd,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/582/static/js/main.746928a2.js@6968251 | `(0,Zn.jsx)(FI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/582/static/js/main.746928a2.js@6968298 | `(0,Zn.jsx)(UM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 585 · Razer Pro Type

类别：KEYBOARD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [585]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [584]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/585/static/js/main.04a0e3c0.js@6798291 | `(0,Io.jsx)(lI,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 92 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/585/static/js/main.04a0e3c0.js@6798388 | `(0,Io.jsx)(M_,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/585/static/js/main.04a0e3c0.js@6798435 | `(0,Io.jsx)(LI,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/585/static/js/main.04a0e3c0.js@6798482 | `(0,Io.jsx)(VL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 586 · Razer Blade Stealth 13

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/586/static/js/main.c88d4a36.js@7108215 | `(0,jn.jsx)(FO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/586/static/js/main.c88d4a36.js@7108312 | `(0,jn.jsx)(zd,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/586/static/js/main.c88d4a36.js@7108359 | `(0,jn.jsx)(vI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/586/static/js/main.c88d4a36.js@7108406 | `(0,jn.jsx)(hM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 587 · Razer Blade 15 Advanced Model

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/587/static/js/main.4b36ee81.js@7154893 | `(0,so.jsx)(jO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/587/static/js/main.4b36ee81.js@7154990 | `(0,so.jsx)(rS,{})` | 43 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/587/static/js/main.4b36ee81.js@7155037 | `(0,so.jsx)(XI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/587/static/js/main.4b36ee81.js@7155084 | `(0,so.jsx)(zM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 588 · Razer Blade Pro 17

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/588/static/js/main.6ce2be5d.js@7006015 | `(0,_o.jsx)(QO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/588/static/js/main.6ce2be5d.js@7006112 | `(0,_o.jsx)(qI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/588/static/js/main.6ce2be5d.js@7006159 | `(0,_o.jsx)(cS,{})` | 43 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/588/static/js/main.6ce2be5d.js@7006206 | `(0,_o.jsx)(XM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 589 · Razer Blade 15 Studio Edition

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/589/static/js/main.782d646c.js@6919884 | `(0,io.jsx)(jd,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/589/static/js/main.782d646c.js@6919981 | `(0,io.jsx)(aS,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/589/static/js/main.782d646c.js@6920028 | `(0,io.jsx)(xI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/589/static/js/main.782d646c.js@6920075 | `(0,io.jsx)(bM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 590 · Razer BlackWidow V3

类别：KEYBOARD；edition：[0, 128, 129, 130, 131, 132, 133, 134]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/590/static/js/main.5c506335.js@7803601 | `(0,Ga.jsx)(Xs,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 99 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/590/static/js/main.5c506335.js@7803698 | `(0,Ga.jsx)(OC,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/590/static/js/main.5c506335.js@7803745 | `(0,Ga.jsx)(sR,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 591 · Razer BlackWidow X: Tenkeyless

类别：KEYBOARD；edition：[0, 128, 129, 130, 131, 132]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [591]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/591/static/js/main.60b1f83e.js@6751622 | `(0,va.jsx)(zc,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/591/static/js/main.60b1f83e.js@6751719 | `(0,va.jsx)(n_,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/591/static/js/main.60b1f83e.js@6751766 | `(0,va.jsx)(sL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 592 · Razer Huntsman Essential

类别：KEYBOARD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [592]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/592/static/js/main.319b06ba.js@6793550 | `(0,H.jsx)(mp,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/592/static/js/main.319b06ba.js@6793646 | `(0,H.jsx)(VD,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/592/static/js/main.319b06ba.js@6793692 | `(0,H.jsx)(bR,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 593 · Razer Pokémon

类别：KEYBOARD；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/593/static/js/main.5ba3a935.js@7498030 | `(0,wa.jsx)(TA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/593/static/js/main.5ba3a935.js@7498127 | `(0,wa.jsx)(Or,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/593/static/js/main.5ba3a935.js@7498174 | `(0,wa.jsx)(CL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 594 · Razer Blade Stealth 13 Base Model

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/594/static/js/main.b17f7701.js@7113024 | `(0,qn.jsx)(bO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/594/static/js/main.b17f7701.js@7113121 | `(0,qn.jsx)(Jd,{})` | 44 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/594/static/js/main.b17f7701.js@7113168 | `(0,qn.jsx)(BI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/594/static/js/main.b17f7701.js@7113215 | `(0,qn.jsx)(vM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 595 · Razer Blade 15 Advanced Model

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/595/static/js/main.e6e68be9.js@7057219 | `(0,$n.jsx)(gO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/595/static/js/main.e6e68be9.js@7057316 | `(0,$n.jsx)(qd,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/595/static/js/main.e6e68be9.js@7057363 | `(0,$n.jsx)(UI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/595/static/js/main.e6e68be9.js@7057410 | `(0,$n.jsx)(bM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 597 · Razer Blade 15 Base Model

类别：SYSTEM；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/597/static/js/main.ef1c3cde.js@7157009 | `(0,qn.jsx)(bO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/597/static/js/main.ef1c3cde.js@7157106 | `(0,qn.jsx)(Jd,{})` | 44 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/597/static/js/main.ef1c3cde.js@7157153 | `(0,qn.jsx)(BI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/597/static/js/main.ef1c3cde.js@7157200 | `(0,qn.jsx)(vM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 598 · Razer Blade Pro 17

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/598/static/js/main.375cd241.js@7050520 | `(0,Eo.jsx)(ZO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/598/static/js/main.375cd241.js@7050617 | `(0,Eo.jsx)(lS,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/598/static/js/main.375cd241.js@7050664 | `(0,Eo.jsx)(XI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/598/static/js/main.375cd241.js@7050711 | `(0,Eo.jsx)(QM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 599 · Razer Huntsman Mini

类别：KEYBOARD；edition：[0, 128, 129, 130, 131, 132]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/599/static/js/main.7e39cfe6.js@7181304 | `(0,Ka.jsx)(MO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/599/static/js/main.7e39cfe6.js@7181401 | `(0,Ka.jsx)(Vc,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/599/static/js/main.7e39cfe6.js@7181448 | `(0,Ka.jsx)(TP,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 600 · Razer BlackWidow V3 Mini HyperSpeed

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [625]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [626]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/600/static/js/main.d94b34ab.js@7120281 | `(0,en.jsx)(_P,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 109 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/600/static/js/main.d94b34ab.js@7120378 | `(0,en.jsx)(_U,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/600/static/js/main.d94b34ab.js@7120425 | `(0,en.jsx)(uU,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/600/static/js/main.d94b34ab.js@7120472 | `(0,en.jsx)(mu,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/600/static/js/main.d94b34ab.js@7135578 | `(0,en.jsx)(HU,{deviceInfo:oe.DeviceInfo,deviceName:oe.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 601 · Razer Blade Stealth 13

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/601/static/js/main.4aefdaa9.js@7076332 | `(0,Zn.jsx)(BO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/601/static/js/main.4aefdaa9.js@7076429 | `(0,Zn.jsx)(Qd,{})` | 44 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/601/static/js/main.4aefdaa9.js@7076476 | `(0,Zn.jsx)(FI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/601/static/js/main.4aefdaa9.js@7076523 | `(0,Zn.jsx)(yM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 602 · Blackwidow V3 Pro

类别：KEYBOARD；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [604]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [603]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/602/static/js/main.79e7fc3d.js@7605904 | `(0,Hn.jsx)(AM,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 112 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/602/static/js/main.79e7fc3d.js@7606001 | `(0,Hn.jsx)(lU,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/602/static/js/main.79e7fc3d.js@7606048 | `(0,Hn.jsx)(CU,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/602/static/js/main.79e7fc3d.js@7606095 | `(0,Hn.jsx)(du,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/602/static/js/main.79e7fc3d.js@7653794 | `(0,Hn.jsx)(QU,{deviceInfo:Te.DeviceInfo,deviceName:Te.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 605 · Razer Ornata V2

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/605/static/js/main.fcf07de2.js@7759170 | `(0,wa.jsx)(yl,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/605/static/js/main.fcf07de2.js@7759267 | `(0,wa.jsx)(HA,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/605/static/js/main.fcf07de2.js@7759314 | `(0,wa.jsx)(dm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 606 · RAZER CYNOSA V2

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/606/static/js/main.08f3ff4a.js@7590109 | `(0,ma.jsx)(jO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/606/static/js/main.08f3ff4a.js@7590206 | `(0,ma.jsx)(ec,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/606/static/js/main.08f3ff4a.js@7590253 | `(0,ma.jsx)(BM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 614 · Razer Huntsman V2 Analog

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/614/static/js/main.73d0babc.js@7421680 | `()=>(0,Sn.jsx)(Ih,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 106 | 4 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/614/static/js/main.73d0babc.js@7421787 | `()=>(0,Sn.jsx)($h,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 57 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/614/static/js/main.73d0babc.js@7421927 | `()=>(0,Sn.jsx)(JL,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/614/static/js/main.73d0babc.js@7421984 | `()=>(0,Sn.jsx)(dN,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 616 · Razer Blade 15 Base Model

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/616/static/js/main.17a0cc39.js@7104606 | `(0,qn.jsx)(bO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/616/static/js/main.17a0cc39.js@7104703 | `(0,qn.jsx)(Jd,{})` | 44 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/616/static/js/main.17a0cc39.js@7104750 | `(0,qn.jsx)(BI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/616/static/js/main.17a0cc39.js@7104797 | `(0,qn.jsx)(vM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 617 · Razer Huntsman Mini

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/617/static/js/main.bcdaed34.js@6844193 | `(0,ya.jsx)(Ad,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/617/static/js/main.bcdaed34.js@6844290 | `(0,ya.jsx)(mI,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/617/static/js/main.bcdaed34.js@6844337 | `(0,ya.jsx)(oP,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 618 · Razer Book 13

类别：SYSTEM；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/618/static/js/main.67e42cfd.js@7122622 | `(0,_o.jsx)(QO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/618/static/js/main.67e42cfd.js@7122719 | `(0,_o.jsx)(IS,{})` | 44 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/618/static/js/main.67e42cfd.js@7122766 | `(0,_o.jsx)(qI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/618/static/js/main.67e42cfd.js@7122813 | `(0,_o.jsx)(ZM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 619 · Razer Huntsman V2 Tenkeyless

类别：KEYBOARD；edition：[0, 128, 129, 130, 132]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/619/static/js/main.f420bb87.js@7126253 | `(0,Ka.jsx)(vM,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 103 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/619/static/js/main.f420bb87.js@7126350 | `(0,Ka.jsx)(mL,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/619/static/js/main.f420bb87.js@7126397 | `(0,Ka.jsx)(WR,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 620 · Razer Huntsman V2

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/620/static/js/main.05ff3291.js@7374911 | `(0,wa.jsx)(zP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 103 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/620/static/js/main.05ff3291.js@7375008 | `(0,wa.jsx)(hL,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/620/static/js/main.05ff3291.js@7375055 | `(0,wa.jsx)(zR,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 621 · Razer Blade 15 Advanced Model

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/621/static/js/main.3dde7861.js@7202154 | `(0,_o.jsx)(QO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/621/static/js/main.3dde7861.js@7202251 | `(0,_o.jsx)(uS,{})` | 49 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/621/static/js/main.3dde7861.js@7202298 | `(0,_o.jsx)(qI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/621/static/js/main.3dde7861.js@7202345 | `(0,_o.jsx)(th,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 622 · Razer Blade Pro 17

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/622/static/js/main.eb3237df.js@7019854 | `(0,ro.jsx)(Zd,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/622/static/js/main.eb3237df.js@7019951 | `(0,ro.jsx)(lS,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/622/static/js/main.eb3237df.js@7019998 | `(0,ro.jsx)(XI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/622/static/js/main.eb3237df.js@7020045 | `(0,ro.jsx)(QM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 623 · Razer Blade 15 Base Model

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/623/static/js/main.548e4fea.js@7104905 | `(0,qn.jsx)(bO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/623/static/js/main.548e4fea.js@7105002 | `(0,qn.jsx)(Jd,{})` | 44 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/623/static/js/main.548e4fea.js@7105049 | `(0,qn.jsx)(BI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/623/static/js/main.548e4fea.js@7105096 | `(0,qn.jsx)(vM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 624 · Razer Blade 14

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/624/static/js/main.08d9f86c.js@7032268 | `(0,_o.jsx)(QO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/624/static/js/main.08d9f86c.js@7032365 | `(0,_o.jsx)(cS,{})` | 44 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/624/static/js/main.08d9f86c.js@7032412 | `(0,_o.jsx)(qI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/624/static/js/main.08d9f86c.js@7032459 | `(0,_o.jsx)(jM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 630 · Razer Blade 15 Advanced Model

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/630/static/js/main.5be75a67.js@7193691 | `(0,to.jsx)(GO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/630/static/js/main.5be75a67.js@7193788 | `(0,to.jsx)(eS,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/630/static/js/main.5be75a67.js@7193835 | `(0,to.jsx)(fI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/630/static/js/main.5be75a67.js@7193882 | `(0,to.jsx)(WM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 631 · Razer Pro Type Ultra

类别：KEYBOARD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [635]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [632]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/631/static/js/main.9463ff90.js@7185246 | `(0,so.jsx)(HA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 109 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/631/static/js/main.9463ff90.js@7185343 | `(0,so.jsx)(U_,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/631/static/js/main.9463ff90.js@7185390 | `(0,so.jsx)(YA,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/631/static/js/main.9463ff90.js@7185437 | `(0,so.jsx)(nm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 633 · Razer Blade 17

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/633/static/js/main.a7b90165.js@7053267 | `(0,To.jsx)(JO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/633/static/js/main.a7b90165.js@7053364 | `(0,To.jsx)(SS,{})` | 47 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/633/static/js/main.a7b90165.js@7053411 | `(0,To.jsx)(QI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/633/static/js/main.a7b90165.js@7053458 | `(0,To.jsx)(eh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 634 · Razer Blade 15 Base Model

类别：SYSTEM；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/634/static/js/main.f755b164.js@7047349 | `(0,To.jsx)(JO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/634/static/js/main.f755b164.js@7047446 | `(0,To.jsx)(SS,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/634/static/js/main.f755b164.js@7047493 | `(0,To.jsx)(QI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/634/static/js/main.f755b164.js@7047540 | `(0,To.jsx)(eh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 642 · Razer Huntsman Mini Analog

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/642/static/js/main.c81e7939.js@7206271 | `()=>(0,Cn.jsx)(Nd,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 111 | 4 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/642/static/js/main.c81e7939.js@7206378 | `()=>(0,Cn.jsx)(oU,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 57 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/642/static/js/main.c81e7939.js@7206518 | `()=>(0,Cn.jsx)(dA,{})` | 37 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/642/static/js/main.c81e7939.js@7206575 | `()=>(0,Cn.jsx)(nM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 647 · Razer BlackWidow V4

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/647/static/js/main.b14e4864.js@7825191 | `(0,va.jsx)(r_,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/647/static/js/main.b14e4864.js@7825288 | `(0,va.jsx)(hC,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/647/static/js/main.b14e4864.js@7825335 | `(0,va.jsx)(CR,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 650 · Razer Blade 15 Advanced Model

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/650/static/js/main.a44b3201.js@7209620 | `(0,To.jsx)(JO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/650/static/js/main.a44b3201.js@7209717 | `(0,To.jsx)(RS,{})` | 49 | 0 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/650/static/js/main.a44b3201.js@7209764 | `(0,To.jsx)(MS,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/650/static/js/main.a44b3201.js@7209810 | `(0,To.jsx)(QI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/650/static/js/main.a44b3201.js@7209857 | `(0,To.jsx)(Th,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 651 · Razer Blade 17

类别：SYSTEM；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/651/static/js/main.14910255.js@7208169 | `(0,To.jsx)(JO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/651/static/js/main.14910255.js@7208266 | `(0,To.jsx)(SS,{})` | 47 | 0 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/651/static/js/main.14910255.js@7208313 | `(0,To.jsx)(NU,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/651/static/js/main.14910255.js@7208359 | `(0,To.jsx)(QI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/651/static/js/main.14910255.js@7208406 | `(0,To.jsx)(eh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 652 · Razer Blade 14 

类别：SYSTEM；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/652/static/js/main.aacbcd3d.js@7138544 | `(0,To.jsx)(JO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/652/static/js/main.aacbcd3d.js@7138641 | `(0,To.jsx)(SS,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/652/static/js/main.aacbcd3d.js@7138688 | `(0,To.jsx)(mS,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/652/static/js/main.aacbcd3d.js@7138734 | `(0,To.jsx)(QI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/652/static/js/main.aacbcd3d.js@7138781 | `(0,To.jsx)(Eh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 653 · Blackwidow V4 Pro

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/653/static/js/main.7b71cce5.js@7990798 | `(0,ka.jsx)(nP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 129 | 8 | partial_static_reference_graph | partial_native_reaudited |
| TAB_LIGHTING / default | .ref/devices/653/static/js/main.7b71cce5.js@7990895 | `(0,ka.jsx)(Fh,{})` | 36 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/653/static/js/main.7b71cce5.js@7990942 | `(0,ka.jsx)(Jd,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 654 · Razer Cynosa Pro

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/654/static/js/main.e100f1a2.js@6662052 | `(0,ka.jsx)(_I,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 96 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/654/static/js/main.e100f1a2.js@6662149 | `(0,ka.jsx)(P_,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/654/static/js/main.e100f1a2.js@6662196 | `(0,ka.jsx)(DL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 655 · Razer Ornata V3

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/655/static/js/main.f6ad51bc.js@7859188 | `(0,zn.jsx)(Gl,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/655/static/js/main.f6ad51bc.js@7859285 | `(0,zn.jsx)(gA,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/655/static/js/main.f6ad51bc.js@7859332 | `(0,zn.jsx)(lm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 658 · Razer DeathStalker V2 Pro

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [656]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [657]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/658/static/js/main.62faa864.js@7591293 | `(0,gn.jsx)(OM,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 113 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/658/static/js/main.62faa864.js@7591390 | `(0,gn.jsx)(SU,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/658/static/js/main.62faa864.js@7591437 | `(0,gn.jsx)(LU,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/658/static/js/main.62faa864.js@7591484 | `(0,gn.jsx)(lu,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/658/static/js/main.62faa864.js@7616821 | `(0,gn.jsx)(ZU,{deviceInfo:Te.DeviceInfo,deviceName:Te.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 659 · Razer BlackWidow V4 X

类别：KEYBOARD；edition：[0, 128, 129, 130, 131, 132, 133]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/659/static/js/main.82fcdd62.js@7833356 | `(0,Pa.jsx)(Hs,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/659/static/js/main.82fcdd62.js@7833453 | `(0,Pa.jsx)(eC,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/659/static/js/main.82fcdd62.js@7833500 | `(0,Pa.jsx)(Xu,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 660 · Razer Ornata V3

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/660/static/js/main.85645113.js@7684403 | `(0,wa.jsx)(ll,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/660/static/js/main.85645113.js@7684500 | `(0,wa.jsx)(uc,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/660/static/js/main.85645113.js@7684547 | `(0,wa.jsx)(JM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 661 · Razer DeathStalker V2 Pro

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/661/static/js/main.db42d2c2.js@7766290 | `(0,gn.jsx)(up,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 99 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/661/static/js/main.db42d2c2.js@7766387 | `(0,gn.jsx)(MP,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/661/static/js/main.db42d2c2.js@7766434 | `(0,gn.jsx)(Vu,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 664 · Razer DeathStalker V2 Pro Tenkeyless

类别：KEYBOARD；edition：[0, 128, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [662]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [663]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/664/static/js/main.33d8ba2a.js@7474890 | `(0,Hn.jsx)(DM,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 113 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/664/static/js/main.33d8ba2a.js@7474987 | `(0,Hn.jsx)(CU,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/664/static/js/main.33d8ba2a.js@7475034 | `(0,Hn.jsx)(HU,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/664/static/js/main.33d8ba2a.js@7475081 | `(0,Hn.jsx)(Su,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/664/static/js/main.33d8ba2a.js@7522757 | `(0,Hn.jsx)(sg,{deviceInfo:Ie.DeviceInfo,deviceName:Ie.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 669 · Razer Blade 14 

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/669/static/js/main.b7566b48.js@7554987 | `(0,uo.jsx)(Ed,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/669/static/js/main.b7566b48.js@7555084 | `(0,uo.jsx)(NS,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/669/static/js/main.b7566b48.js@7555131 | `(0,uo.jsx)(zS,{})` | 32 | 0 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/669/static/js/main.b7566b48.js@7555178 | `(0,uo.jsx)(hS,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/669/static/js/main.b7566b48.js@7555224 | `(0,uo.jsx)(iA,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/669/static/js/main.b7566b48.js@7555271 | `(0,uo.jsx)(hh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 670 · Razer Blade 15

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/670/static/js/main.edf99560.js@7212177 | `(0,ao.jsx)(HO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/670/static/js/main.edf99560.js@7212274 | `(0,ao.jsx)(Jd,{})` | 36 | 0 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/670/static/js/main.edf99560.js@7212321 | `(0,ao.jsx)(TS,{})` | 32 | 0 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/670/static/js/main.edf99560.js@7212368 | `(0,ao.jsx)(uS,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/670/static/js/main.edf99560.js@7212414 | `(0,ao.jsx)(GI,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/670/static/js/main.edf99560.js@7212461 | `(0,ao.jsx)(sh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 671 · Razer Blade 16

类别：SYSTEM；edition：[0, 129, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/671/static/js/main.a4a1b2d9.js@7494596 | `(0,uo.jsx)(Ed,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/671/static/js/main.a4a1b2d9.js@7494693 | `(0,uo.jsx)(AS,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/671/static/js/main.a4a1b2d9.js@7494740 | `(0,uo.jsx)(FU,{})` | 35 | 0 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/671/static/js/main.a4a1b2d9.js@7494787 | `(0,uo.jsx)(kU,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/671/static/js/main.a4a1b2d9.js@7494833 | `(0,uo.jsx)(iA,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/671/static/js/main.a4a1b2d9.js@7494880 | `(0,uo.jsx)(qP,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 672 · Razer Blade 18

类别：SYSTEM；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/672/static/js/main.2283fcdf.js@7660050 | `(0,_o.jsx)(VO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 5 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/672/static/js/main.2283fcdf.js@7660147 | `(0,_o.jsx)(jd,{})` | 48 | 0 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/672/static/js/main.2283fcdf.js@7660194 | `(0,_o.jsx)(LS,{})` | 35 | 0 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/672/static/js/main.2283fcdf.js@7660241 | `(0,_o.jsx)(HS,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/672/static/js/main.2283fcdf.js@7660287 | `(0,_o.jsx)(Kc,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/672/static/js/main.2283fcdf.js@7660334 | `(0,_o.jsx)(Rh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 673 · Razer Ornata V3

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/673/static/js/main.7c592b5e.js@7923372 | `(0,zn.jsx)(Gl,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/673/static/js/main.7c592b5e.js@7923469 | `(0,zn.jsx)(gA,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/673/static/js/main.7c592b5e.js@7923516 | `(0,zn.jsx)(lm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 674 · Razer Ornata V3

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/674/static/js/main.b8c0b92d.js@7807378 | `(0,zn.jsx)(ll,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/674/static/js/main.b8c0b92d.js@7807475 | `(0,zn.jsx)(uA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/674/static/js/main.b8c0b92d.js@7807522 | `(0,zn.jsx)(JM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 675 · Razer Ornata V3 Tenkeyless

类别：KEYBOARD；edition：[0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/675/static/js/main.b4dbb43b.js@7577296 | `(0,wa.jsx)(HO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 96 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/675/static/js/main.b4dbb43b.js@7577393 | `(0,wa.jsx)(Hc,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/675/static/js/main.b4dbb43b.js@7577440 | `(0,wa.jsx)(lM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 677 · Razer BlackWidow V4 75%

类别：KEYBOARD；edition：[0, 128, 129, 130, 131, 132, 133, 134]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/677/static/js/main.b1c9555f.js@7000277 | `(0,Va.jsx)(KO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 101 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/677/static/js/main.b1c9555f.js@7000374 | `(0,Va.jsx)(PI,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/677/static/js/main.b1c9555f.js@7000421 | `(0,Va.jsx)(gP,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 678 · Razer Huntsman V3 Pro

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/678/static/js/main.f78f0dcf.js@7432224 | `()=>(0,un.jsx)(vh,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 115 | 4 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/678/static/js/main.f78f0dcf.js@7432331 | `()=>(0,un.jsx)(PU,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 59 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/678/static/js/main.f78f0dcf.js@7432471 | `()=>(0,un.jsx)(em,{})` | 37 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/678/static/js/main.f78f0dcf.js@7432528 | `()=>(0,un.jsx)(uN,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 679 · Razer Huntsman V3 Pro Tenkeyless

类别：KEYBOARD；edition：[0, 128, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/679/static/js/main.194d8a7b.js@7314338 | `()=>(0,un.jsx)(Gh,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 115 | 4 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/679/static/js/main.194d8a7b.js@7314445 | `()=>(0,un.jsx)(pU,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 59 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/679/static/js/main.194d8a7b.js@7314585 | `()=>(0,un.jsx)(em,{})` | 37 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/679/static/js/main.194d8a7b.js@7314642 | `()=>(0,un.jsx)(uN,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 688 · Razer Huntsman V3 Pro Mini

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/688/static/js/main.e09fd702.js@7207425 | `()=>(0,Mn.jsx)(bd,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 115 | 4 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/688/static/js/main.e09fd702.js@7207532 | `()=>(0,Mn.jsx)(mU,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 59 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/688/static/js/main.e09fd702.js@7207672 | `()=>(0,Mn.jsx)(DA,{})` | 37 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/688/static/js/main.e09fd702.js@7207729 | `()=>(0,Mn.jsx)(DM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 689 · RAZER HUNTSMAN V3 X TENKEYLESS

类别：KEYBOARD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/689/static/js/main.9d55a71f.js@7208154 | `(0,Ka.jsx)(PO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/689/static/js/main.9d55a71f.js@7208251 | `(0,Ka.jsx)(bc,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/689/static/js/main.9d55a71f.js@7208298 | `(0,Ka.jsx)(_P,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 691 · Blackwidow V4 Pro 75%

类别：KEYBOARD；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [692]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [693]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/691/static/js/7668.a3c13eca.chunk.js@159074 | `(0,M.jsx)(Oe.kg,{lazy:()=>Promise.all([a.e(8865),a.e(6375),a.e(2535),a.e(3544)]).then(a.bind(a,78697)),setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 101 | 38 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/691/static/js/7668.a3c13eca.chunk.js@159258 | `(0,M.jsx)(Oe.kg,{lazy:()=>Promise.all([a.e(8865),a.e(6664),a.e(1259)]).then(a.bind(a,76664))})` | 30 | 13 | partial_static_reference_graph | partial_native |
| OLED / default | .ref/devices/691/static/js/7668.a3c13eca.chunk.js@159381 | `(0,M.jsx)(Oe.kg,{lazy:()=>Promise.all([a.e(3725),a.e(7077)]).then(a.bind(a,42553))})` | 105 | 30 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/691/static/js/7668.a3c13eca.chunk.js@159495 | `(0,M.jsx)(Oe.kg,{lazy:()=>a.e(3730).then(a.bind(a,35665)),isDongle:this.props.isDongle})` | 21 | 9 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/691/static/js/7668.a3c13eca.chunk.js@159612 | `(0,M.jsx)(Oe.kg,{lazy:()=>Promise.all([a.e(5722),a.e(2043)]).then(a.bind(a,88934)),resetObm:this.resetDevice})` | 20 | 14 | partial_static_reference_graph | partial_native |

## 694 · Razer Blade 14

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/694/static/js/main.79278d0f.js@7915629 | `(0,ul.jsx)(xL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 58 | 29 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/694/static/js/main.79278d0f.js@7915726 | `(0,ul.jsx)(sM,{})` | 32 | 59 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/694/static/js/main.79278d0f.js@7915773 | `(0,ul.jsx)(BM,{})` | 22 | 19 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/694/static/js/main.79278d0f.js@7915820 | `(0,ul.jsx)(HP,{})` | 38 | 32 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/694/static/js/main.79278d0f.js@7915867 | `(0,ul.jsx)(AM,{})` | 8 | 11 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/694/static/js/main.79278d0f.js@7915913 | `(0,ul.jsx)(qD,{})` | 28 | 46 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/694/static/js/main.79278d0f.js@7915960 | `(0,ul.jsx)(Zf,{resetObm:this.resetDevice,hasSystemInfo:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 695 · Razer Blade 16

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/695/static/js/main.de25a2a9.js@7853994 | `(0,Sl.jsx)(jL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 58 | 29 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/695/static/js/main.de25a2a9.js@7854091 | `(0,Sl.jsx)(eh,{})` | 32 | 59 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/695/static/js/main.de25a2a9.js@7854138 | `(0,Sl.jsx)(Kh,{})` | 25 | 22 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/695/static/js/main.de25a2a9.js@7854185 | `(0,Sl.jsx)(BP,{})` | 38 | 32 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/695/static/js/main.de25a2a9.js@7854232 | `(0,Sl.jsx)(Eh,{})` | 8 | 11 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/695/static/js/main.de25a2a9.js@7854278 | `(0,Sl.jsx)(QD,{})` | 28 | 46 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/695/static/js/main.de25a2a9.js@7854325 | `(0,Sl.jsx)(eH,{resetObm:this.resetDevice,hasSystemInfo:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 696 · Razer Blade 18

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/696/static/js/main.e247cb61.js@8020293 | `(0,il.jsx)(hL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 61 | 30 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/696/static/js/main.e247cb61.js@8020390 | `(0,il.jsx)(gh,{})` | 32 | 59 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/696/static/js/main.e247cb61.js@8020437 | `(0,il.jsx)(_P,{})` | 22 | 19 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/696/static/js/main.e247cb61.js@8020484 | `(0,il.jsx)(pM,{})` | 38 | 32 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/696/static/js/main.e247cb61.js@8020531 | `(0,il.jsx)(uP,{})` | 8 | 11 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/696/static/js/main.e247cb61.js@8020577 | `(0,il.jsx)(MD,{})` | 28 | 46 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/696/static/js/main.e247cb61.js@8020624 | `(0,il.jsx)(Yf,{resetObm:this.resetDevice,hasSystemInfo:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 697 · Razer BlackWidow V3 Mini HyperSpeed

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [698]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [699]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/697/static/js/main.9db50ae0.js@7115406 | `(0,en.jsx)(_P,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 109 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/697/static/js/main.9db50ae0.js@7115503 | `(0,en.jsx)(_U,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/697/static/js/main.9db50ae0.js@7115550 | `(0,en.jsx)(uU,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/697/static/js/main.9db50ae0.js@7115597 | `(0,en.jsx)(mu,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/697/static/js/main.9db50ae0.js@7153477 | `(0,en.jsx)(BU,{deviceInfo:oe.DeviceInfo,deviceName:oe.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 708 · Razer Pro Type Ergo

类别：KEYBOARD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [706]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [707]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/708/static/js/8231.8f6e8a7c.chunk.js@197578 | `(0,C.jsx)(Ys,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 119 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/708/static/js/8231.8f6e8a7c.chunk.js@197673 | `(0,C.jsx)(Ks.A,{})` | 34 | 6 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/708/static/js/8231.8f6e8a7c.chunk.js@197720 | `(0,C.jsx)(ga,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/708/static/js/8231.8f6e8a7c.chunk.js@197765 | `(0,C.jsx)(Gs,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 709 · Razer Blade 14

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/709/static/js/2915.e5ea330d.chunk.js@414213 | `(0,N.jsx)(rt,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 94 | 28 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/709/static/js/2915.e5ea330d.chunk.js@414308 | `(0,N.jsx)(ea,{})` | 61 | 29 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/709/static/js/2915.e5ea330d.chunk.js@414353 | `(0,N.jsx)(Xa,{})` | 36 | 10 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/709/static/js/2915.e5ea330d.chunk.js@414398 | `(0,N.jsx)(Xn,{})` | 44 | 15 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/709/static/js/2915.e5ea330d.chunk.js@414443 | `(0,N.jsx)(la,{})` | 16 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/709/static/js/2915.e5ea330d.chunk.js@414487 | `(0,N.jsx)(ot.A,{})` | 39 | 21 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/709/static/js/2915.e5ea330d.chunk.js@414534 | `(0,N.jsx)(Yr,{resetObm:this.resetDevice,hasSystemInfo:!0,hasSystemInfoImage:!0})` | 24 | 8 | partial_static_reference_graph | partial_native |

## 710 · Razer Blade 16

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/710/static/js/4812.d689e7ff.chunk.js@415427 | `(0,A.jsx)(rt,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 92 | 28 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/710/static/js/4812.d689e7ff.chunk.js@415522 | `(0,A.jsx)(ea,{})` | 61 | 29 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/710/static/js/4812.d689e7ff.chunk.js@415567 | `(0,A.jsx)(Ja,{})` | 36 | 10 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/710/static/js/4812.d689e7ff.chunk.js@415612 | `(0,A.jsx)(Jn,{})` | 44 | 15 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/710/static/js/4812.d689e7ff.chunk.js@415657 | `(0,A.jsx)(la,{})` | 16 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/710/static/js/4812.d689e7ff.chunk.js@415701 | `(0,A.jsx)(ot.A,{})` | 39 | 21 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/710/static/js/4812.d689e7ff.chunk.js@415748 | `(0,A.jsx)(Yr,{resetObm:this.resetDevice,hasSystemInfo:!0,hasSystemInfoImage:!0})` | 24 | 8 | partial_static_reference_graph | partial_native |

## 711 · Razer Blade 18

类别：SYSTEM；edition：[0, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/711/static/js/4812.3dd4cf02.chunk.js@415254 | `(0,A.jsx)(rt,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 95 | 28 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/711/static/js/4812.3dd4cf02.chunk.js@415349 | `(0,A.jsx)(ea,{})` | 61 | 29 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/711/static/js/4812.3dd4cf02.chunk.js@415394 | `(0,A.jsx)(Ja,{})` | 36 | 10 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/711/static/js/4812.3dd4cf02.chunk.js@415439 | `(0,A.jsx)(Jn,{})` | 44 | 15 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/711/static/js/4812.3dd4cf02.chunk.js@415484 | `(0,A.jsx)(la,{})` | 16 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/711/static/js/4812.3dd4cf02.chunk.js@415528 | `(0,A.jsx)(ot.A,{})` | 39 | 21 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/711/static/js/4812.3dd4cf02.chunk.js@415575 | `(0,A.jsx)(Yr,{resetObm:this.resetDevice,hasSystemInfo:!0,hasSystemInfoImage:!0})` | 24 | 8 | partial_static_reference_graph | partial_native |

## 716 · Razer BlackWidow V4 Low-profile HyperSpeed

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [713]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [714]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/716/static/js/main.a149c89a.js@7231886 | `(0,hn.jsx)(nP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 120 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/716/static/js/main.a149c89a.js@7231983 | `(0,hn.jsx)(tg,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/716/static/js/main.a149c89a.js@7232030 | `(0,hn.jsx)(pg,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/716/static/js/main.a149c89a.js@7232077 | `(0,hn.jsx)(Gu,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/716/static/js/main.a149c89a.js@7281227 | `(0,hn.jsx)(Wg,{deviceInfo:oe.DeviceInfo,deviceName:oe.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 717 · Razer Joro

类别：KEYBOARD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [730]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [718]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/717/static/js/main.9af2005e.js@6961193 | `(0,qn.jsx)(iP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 103 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/717/static/js/main.9af2005e.js@6961290 | `(0,qn.jsx)(oU,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/717/static/js/main.9af2005e.js@6961337 | `(0,qn.jsx)(DU,{})` | 13 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/717/static/js/main.9af2005e.js@6961384 | `(0,qn.jsx)(Ou,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/717/static/js/main.9af2005e.js@7009167 | `(0,qn.jsx)(eg,{deviceInfo:Te.DeviceInfo,deviceName:Te.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 719 · Razer Huntsman V3 Pro 8KHz

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/719/static/js/7928.8ab59a50.chunk.js@283078 | `()=>(0,h.jsx)(ci,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 123 | 8 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/719/static/js/7928.8ab59a50.chunk.js@283183 | `()=>(0,h.jsx)(Xn,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 66 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/719/static/js/7928.8ab59a50.chunk.js@283321 | `()=>(0,h.jsx)(ai.A,{})` | 35 | 7 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/719/static/js/7928.8ab59a50.chunk.js@283378 | `()=>(0,h.jsx)(It,{resetObm:this.props.obmResetDevice})` | 25 | 1 | partial_static_reference_graph | partial_native |

## 720 · Razer Huntsman V3 Pro Tenkeyless 8KHZ

类别：KEYBOARD；edition：[0, 128, 129, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/720/static/js/7293.9acc3165.chunk.js@300474 | `()=>(0,h.jsx)(ci,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 125 | 8 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/720/static/js/7293.9acc3165.chunk.js@300579 | `()=>(0,h.jsx)(Xn,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 66 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/720/static/js/7293.9acc3165.chunk.js@300717 | `()=>(0,h.jsx)(ai.A,{})` | 35 | 7 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/720/static/js/7293.9acc3165.chunk.js@300774 | `()=>(0,h.jsx)(It,{resetObm:this.resetDevice})` | 25 | 1 | partial_static_reference_graph | partial_native |

## 721 · Razer Huntsman V3 Pro Mini 8KHz

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/721/static/js/557.44f061f6.chunk.js@282465 | `()=>(0,m.jsx)(_i,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 120 | 6 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/721/static/js/557.44f061f6.chunk.js@282570 | `()=>(0,m.jsx)(va,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 67 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/721/static/js/557.44f061f6.chunk.js@282708 | `()=>(0,m.jsx)(gi.A,{})` | 35 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/721/static/js/557.44f061f6.chunk.js@282765 | `()=>(0,m.jsx)(Vt,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 724 · BlackWidow V4 Low-profile Tenkeyless HyperSpeed

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [722]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [723]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/724/static/js/main.4b1b7b70.js@7357736 | `(0,co.jsx)(FM,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 120 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/724/static/js/main.4b1b7b70.js@7357833 | `(0,co.jsx)(Yg,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/724/static/js/main.4b1b7b70.js@7357880 | `(0,co.jsx)(cf,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/724/static/js/main.4b1b7b70.js@7357927 | `(0,co.jsx)(aR,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/724/static/js/main.4b1b7b70.js@7407341 | `(0,co.jsx)(Wf,{deviceInfo:_e.DeviceInfo,deviceName:_e.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 727 · Razer BlackWidow V4 Tenkeyless HyperSpeed

类别：KEYBOARD；edition：[0, 129, 130, 131, 132, 133]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [725]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [726]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/727/static/js/main.07c5d653.js@6983908 | `(0,ti.jsx)(zP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 118 | 8 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/727/static/js/main.07c5d653.js@6984005 | `(0,ti.jsx)(kU,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/727/static/js/main.07c5d653.js@6984052 | `(0,ti.jsx)(lg,{isDongle:this.props.isDongle})` | 14 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/727/static/js/main.07c5d653.js@6984127 | `(0,ti.jsx)(Cu,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_PAIRING / multiDevicePairing | .ref/devices/727/static/js/main.07c5d653.js@7042589 | `(0,ti.jsx)($g,{deviceInfo:Ee.DeviceInfo,deviceName:Ee.DEVICE_NAME,allMasters:this._allMastersCached})` | 3 | 0 | partial_static_reference_graph / owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX | independent_not_primary |

## 728 · Razer Huntsman Signature Editon

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/728/static/js/3470.5c149f4b.chunk.js@300792 | `()=>(0,h.jsx)(Si,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 122 | 8 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/728/static/js/3470.5c149f4b.chunk.js@300897 | `()=>(0,h.jsx)(ia,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 66 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/728/static/js/3470.5c149f4b.chunk.js@301035 | `()=>(0,h.jsx)(mi.A,{})` | 34 | 8 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/728/static/js/3470.5c149f4b.chunk.js@301092 | `()=>(0,h.jsx)(Tt,{resetObm:this.props.obmResetDevice})` | 25 | 1 | partial_static_reference_graph | partial_native |

## 736 · Razer Blade 16

类别：SYSTEM；edition：[0, 129, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/736/static/js/8557.8b857d45.chunk.js@421589 | `(0,A.jsx)(rt,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 93 | 28 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/736/static/js/8557.8b857d45.chunk.js@421684 | `(0,A.jsx)(ea,{})` | 61 | 29 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/736/static/js/8557.8b857d45.chunk.js@421729 | `(0,A.jsx)(Ja,{})` | 36 | 10 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/736/static/js/8557.8b857d45.chunk.js@421774 | `(0,A.jsx)(er,{})` | 46 | 15 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/736/static/js/8557.8b857d45.chunk.js@421819 | `(0,A.jsx)(la,{})` | 16 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/736/static/js/8557.8b857d45.chunk.js@421863 | `(0,A.jsx)(ot.A,{})` | 39 | 21 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/736/static/js/8557.8b857d45.chunk.js@421910 | `(0,A.jsx)(Qr,{resetObm:this.resetDevice,hasSystemInfo:!0,hasSystemInfoImage:!0})` | 24 | 8 | partial_static_reference_graph | partial_native |

## 737 · Razer Blade 18

类别：SYSTEM；edition：[0, 129, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/737/static/js/8923.bdb8bcb2.chunk.js@404363 | `(0,N.jsx)(st,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 93 | 28 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/737/static/js/8923.bdb8bcb2.chunk.js@404458 | `(0,N.jsx)(qi,{})` | 61 | 29 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/737/static/js/8923.bdb8bcb2.chunk.js@404503 | `(0,N.jsx)(Ga,{})` | 36 | 10 | partial_static_reference_graph | partial_native |
| TAB_SOUND / default | .ref/devices/737/static/js/8923.bdb8bcb2.chunk.js@404548 | `(0,N.jsx)(Xn,{})` | 46 | 15 | partial_static_reference_graph | partial_native |
| TAB_BATTERY / default | .ref/devices/737/static/js/8923.bdb8bcb2.chunk.js@404593 | `(0,N.jsx)(aa,{})` | 16 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/737/static/js/8923.bdb8bcb2.chunk.js@404637 | `(0,N.jsx)(et.A,{})` | 39 | 21 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/737/static/js/8923.bdb8bcb2.chunk.js@404684 | `(0,N.jsx)(zr,{resetObm:this.resetDevice,hasSystemInfo:!0,hasSystemInfoImage:!0})` | 24 | 8 | partial_static_reference_graph | partial_native |

## 739 · Razer Reclusa X Mini 65%

类别：KEYBOARD；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/739/static/js/2278.2e3e2ec7.chunk.js@198436 | `(0,_.jsx)(Sa,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 109 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/739/static/js/2278.2e3e2ec7.chunk.js@198531 | `(0,_.jsx)(Ca.A,{})` | 33 | 7 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/739/static/js/2278.2e3e2ec7.chunk.js@198578 | `(0,_.jsx)(pa,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 740 · Razer Huntsman V3 HE Magnetic Mini 65% 8KHz

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/740/static/js/9908.1f02bab3.chunk.js@299735 | `()=>(0,h.jsx)(Ci,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 120 | 7 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/740/static/js/9908.1f02bab3.chunk.js@299840 | `()=>(0,h.jsx)(aa,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 67 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/740/static/js/9908.1f02bab3.chunk.js@299978 | `()=>(0,h.jsx)(mi.A,{})` | 36 | 7 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/740/static/js/9908.1f02bab3.chunk.js@300035 | `()=>(0,h.jsx)(Aa,{buttonList:this.props.buttonList})` | 39 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/740/static/js/9908.1f02bab3.chunk.js@300122 | `()=>(0,h.jsx)(Tt,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 741 · Razer Huntsman V3 Tenkeyless 8KHZ

类别：KEYBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/741/static/js/8427.2d5ccb8c.chunk.js@279241 | `()=>(0,h.jsx)(ci,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 123 | 7 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/741/static/js/8427.2d5ccb8c.chunk.js@279346 | `()=>(0,h.jsx)(Xn,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 66 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/741/static/js/8427.2d5ccb8c.chunk.js@279484 | `()=>(0,h.jsx)(ai.A,{})` | 35 | 7 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/741/static/js/8427.2d5ccb8c.chunk.js@279541 | `()=>(0,h.jsx)(It,{resetObm:this.props.obmResetDevice})` | 25 | 1 | partial_static_reference_graph | partial_native |

## 742 · RAZER HUNTSMAN V3 PRO LOW-PROFILE TENKEYLESS 8KHZ

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/742/static/js/3666.9e3f3856.chunk.js@277502 | `()=>(0,h.jsx)(Si,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 128 | 8 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/742/static/js/3666.9e3f3856.chunk.js@277607 | `()=>(0,h.jsx)(aa,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 66 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/742/static/js/3666.9e3f3856.chunk.js@277745 | `()=>(0,h.jsx)(hi.A,{})` | 34 | 8 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/742/static/js/3666.9e3f3856.chunk.js@277802 | `()=>(0,h.jsx)(Tt,{resetObm:this.props.obmResetDevice})` | 25 | 1 | partial_static_reference_graph | partial_native |

## 746 · Razer Huntsman V3 HE Magnetic Tenkeyless 8KHz

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/746/static/js/6498.86d76d1a.chunk.js@307978 | `()=>(0,h.jsx)(Ci,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 120 | 8 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/746/static/js/6498.86d76d1a.chunk.js@308083 | `()=>(0,h.jsx)(aa,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 66 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/746/static/js/6498.86d76d1a.chunk.js@308221 | `()=>(0,h.jsx)(mi.A,{})` | 34 | 8 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/746/static/js/6498.86d76d1a.chunk.js@308278 | `()=>(0,h.jsx)(Aa,{buttonList:this.props.buttonList})` | 38 | 2 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/746/static/js/6498.86d76d1a.chunk.js@308365 | `()=>(0,h.jsx)(It,{resetObm:this.props.obmResetDevice})` | 25 | 1 | partial_static_reference_graph | partial_native |

## 747 · Tartarus Pro

类别：KEYPAD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/747/static/js/7727.590e023e.chunk.js@277859 | `()=>(0,m.jsx)(li,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 100 | 7 | partial_static_reference_graph | partial_native |
| ACTUATION / default | .ref/devices/747/static/js/7727.590e023e.chunk.js@277974 | `()=>(0,m.jsx)(Ya,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})` | 65 | 2 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/747/static/js/7727.590e023e.chunk.js@278132 | `()=>(0,m.jsx)(ni.A,{})` | 31 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/747/static/js/7727.590e023e.chunk.js@278198 | `()=>(0,m.jsx)(wt,{resetObm:this.props.obmResetDevice})` | 25 | 0 | partial_static_reference_graph | partial_native |

## 752 · Razer BlackWidow V4 75% XBOX 25 Anniversary Edition 

类别：KEYBOARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/752/static/js/3717.9326e687.chunk.js@182639 | `(0,l.jsx)(Ie,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 102 | 6 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/752/static/js/3717.9326e687.chunk.js@182734 | `(0,l.jsx)(Re.A,{})` | 34 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/752/static/js/3717.9326e687.chunk.js@182781 | `(0,l.jsx)(bt,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 769 · RAZER PHILIPS HUE

类别：HUE, HUE_HUE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| HOME / default | .ref/devices/769/static/js/main.ad1113f8.js@6693001 | `t?(0,q.jsx)(Fi,{}):(0,q.jsx)(pA,{})` | 55 | 4 | partial_static_reference_graph / conditional_JSX_root_both_branches_preserved | partial_native |
| HELP / default | .ref/devices/769/static/js/main.ad1113f8.js@6693062 | `(0,q.jsx)(hA,{})` | 9 | 0 | partial_static_reference_graph | partial_native |

## 777 · RAZER KRAKEN BT SANRIO LIMITED EDITION

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [777]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/777/static/js/main.eb70ce38.js@4673350 | `(0,_E.jsx)(WM,{})` | 37 | 7 | partial_static_reference_graph | partial_native_reaudited |
| TAB_MIC / default | .ref/devices/777/static/js/main.eb70ce38.js@4673397 | `(0,_E.jsx)(KM,{})` | 14 | 4 | partial_static_reference_graph | partial_native_reaudited |
| TAB_LIGHTING / default | .ref/devices/777/static/js/main.eb70ce38.js@4673444 | `(0,_E.jsx)(YG,{})` | 34 | 17 | partial_static_reference_graph | partial_native_reaudited |
| TAB_POWER / default | .ref/devices/777/static/js/main.eb70ce38.js@4673491 | `(0,_E.jsx)(KG,{})` | 11 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/777/static/js/main.eb70ce38.js@4673538 | `(0,_E.jsx)(Tv,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native_reaudited |

## 778 · ASRock B550 Taichi Razer Edition

类别：ACCESSORY_MAINBOARD；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/778/static/js/main.032287fa.js@4545793 | `(0,GI.jsx)(iR,{})` | 32 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/778/static/js/main.032287fa.js@4545840 | `(0,GI.jsx)(xd,{})` | 36 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/778/static/js/main.032287fa.js@4545887 | `(0,GI.jsx)(Hm,{})` | 13 | 0 | partial_static_reference_graph | partial_native |

## 780 · Razer Key Light Chroma

类别：IOT_KEY_LIGHT；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/780/static/js/main.157991a2.js@4582159 | `(0,Ke.jsx)(QN,{})` | 49 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/780/static/js/main.157991a2.js@4582206 | `(0,Ke.jsx)(uh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 781 · Razer Aether Lamp Pro

类别：IOT_LAMP；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/781/static/js/main.8720d82b.js@4568768 | `(0,Ke.jsx)(td,{})` | 51 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/781/static/js/main.8720d82b.js@4568815 | `(0,Ke.jsx)(mh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 782 · Razer Aether Lamp

类别：IOT_LAMP；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/782/static/js/main.32e42b80.js@4577372 | `(0,Ke.jsx)(td,{})` | 51 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/782/static/js/main.32e42b80.js@4577419 | `(0,Ke.jsx)(mh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 783 · Razer Aether Light Bulb

类别：IOT_BULB；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/783/static/js/main.5b59b09c.js@4576601 | `(0,Ke.jsx)(td,{})` | 51 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/783/static/js/main.5b59b09c.js@4576648 | `(0,Ke.jsx)(mh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 784 · Razer Aether Light Strip

类别：IOT_STRIP；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| CUSTOMIZED / default | .ref/devices/784/static/js/main.3094caa4.js@4601150 | `(0,Ye.jsx)(Ag,{})` | 17 | 2 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/784/static/js/main.3094caa4.js@4601197 | `(0,Ye.jsx)(aN,{})` | 44 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/784/static/js/main.3094caa4.js@4601244 | `(0,Ye.jsx)(pU,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 790 · RAZER AETHER MONITOR LIGHT BAR

类别：IOT_STRIP；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/790/static/js/main.768d7d9e.js@4554570 | `(0,we.jsx)(Td,{})` | 53 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/790/static/js/main.768d7d9e.js@4554617 | `(0,we.jsx)(HU,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 791 · Razer Aether Standing Light Bars

类别：IOT_STRIP；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/791/static/js/main.973498e0.js@4569523 | `(0,Ke.jsx)(_d,{})` | 44 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/791/static/js/main.973498e0.js@4569570 | `(0,Ke.jsx)(Mh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1303 · Razer Nommo Chroma

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1303/static/js/main.4de5d448.js@4557722 | `(0,aE.jsx)(JL,{})` | 36 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1303/static/js/main.4de5d448.js@4557769 | `(0,aE.jsx)(zm,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1303/static/js/main.4de5d448.js@4557816 | `(0,aE.jsx)(rG,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1304 · Razer Nommo Pro

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1304/static/js/main.3b6d76c2.js@4573907 | `(0,oE.jsx)(ep,{})` | 50 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1304/static/js/main.3b6d76c2.js@4573954 | `(0,oE.jsx)(KM,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1304/static/js/main.3b6d76c2.js@4574001 | `(0,oE.jsx)(Ig,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1306 · RAZER NARI ULTIMATE

类别：AUDIO；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1306/static/js/64.22d5d6dd.chunk.js@509525 | `(0,k.jsx)(Lc,{})` | 49 | 13 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1306/static/js/64.22d5d6dd.chunk.js@509570 | `(0,k.jsx)(_d,{})` | 19 | 5 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1306/static/js/64.22d5d6dd.chunk.js@509615 | `(0,k.jsx)(lm,{})` | 36 | 9 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1306/static/js/64.22d5d6dd.chunk.js@509660 | `(0,k.jsx)(Cp.A,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1306/static/js/64.22d5d6dd.chunk.js@509707 | `(0,k.jsx)(ym,{})` | 12 | 5 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1306/static/js/64.22d5d6dd.chunk.js@509752 | `(0,k.jsx)(_p,{})` | 33 | 26 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1306/static/js/64.22d5d6dd.chunk.js@509797 | `(0,k.jsx)(pp,{resetObm:this.factoryReset})` | 24 | 6 | partial_static_reference_graph | partial_native |

## 1308 · RAZER NARI

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1308/static/js/main.82fa5e39.js@5411114 | `(0,We.jsx)(PB,{})` | 46 | 24 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1308/static/js/main.82fa5e39.js@5411161 | `(0,We.jsx)(Tb,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1308/static/js/main.82fa5e39.js@5411208 | `(0,We.jsx)(Jb,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1308/static/js/main.82fa5e39.js@5411255 | `(0,We.jsx)(tz,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1308/static/js/main.82fa5e39.js@5411302 | `(0,We.jsx)(IF,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1308/static/js/main.82fa5e39.js@5411349 | `(0,We.jsx)(nW,{})` | 4 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1308/static/js/main.82fa5e39.js@5411396 | `(0,We.jsx)(eW,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1310 · RAZER NARI ESSENTIAL

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1310/static/js/main.b8876f9f.js@5347845 | `(0,be.jsx)(OF,{})` | 46 | 24 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1310/static/js/main.b8876f9f.js@5347892 | `(0,be.jsx)(qF,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1310/static/js/main.b8876f9f.js@5347939 | `(0,be.jsx)(FV,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1310/static/js/main.b8876f9f.js@5347986 | `(0,be.jsx)(JV,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1310/static/js/main.b8876f9f.js@5348033 | `(0,be.jsx)(jY,{})` | 5 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1310/static/js/main.b8876f9f.js@5348080 | `(0,be.jsx)(wY,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1312 · RAZER USB AUDIO CONTROLLER

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1312/static/js/main.a0d767c7.js@5182122 | `(0,Ge.jsx)(bH,{})` | 43 | 24 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1312/static/js/main.a0d767c7.js@5182169 | `(0,Ge.jsx)($H,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1312/static/js/main.a0d767c7.js@5182216 | `(0,Ge.jsx)(yB,{})` | 24 | 11 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1312/static/js/main.a0d767c7.js@5182263 | `(0,Ge.jsx)(hF,{})` | 3 | 10 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1312/static/js/main.a0d767c7.js@5182310 | `(0,Ge.jsx)(pF,{})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1313 · Razer Kraken Kitty Edition

类别：AUDIO；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1313/static/js/main.c9234892.js@5441530 | `(0,xe.jsx)(aw,{})` | 50 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1313/static/js/main.c9234892.js@5441577 | `(0,xe.jsx)(lw,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1313/static/js/main.c9234892.js@5441624 | `(0,xe.jsx)(bw,{})` | 25 | 11 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1313/static/js/main.c9234892.js@5441671 | `(0,xe.jsx)(DY,{})` | 34 | 33 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1313/static/js/main.c9234892.js@5441718 | `(0,xe.jsx)(mz,{})` | 4 | 9 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1313/static/js/main.c9234892.js@5441765 | `(0,xe.jsx)(Tz,{resetObm:this.resetDevice})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1318 · Razer Kraken X USB

类别：AUDIO, productCategoryIcon；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1318/static/js/main.db06a21e.js@4541243 | `(0,Fe.jsx)(bN,{})` | 30 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1318/static/js/main.db06a21e.js@4541290 | `(0,Fe.jsx)(ld,{})` | 21 | 5 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1318/static/js/main.db06a21e.js@4541337 | `(0,Fe.jsx)(Uh,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1319 · RAZER KRAKEN ULTIMATE

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1319/static/js/main.18b7442e.js@5453965 | `(0,xe.jsx)(HF,{})` | 51 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1319/static/js/main.18b7442e.js@5454012 | `(0,xe.jsx)(Iw,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1319/static/js/main.18b7442e.js@5454059 | `(0,xe.jsx)(eV,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1319/static/js/main.18b7442e.js@5454106 | `(0,xe.jsx)(Xz,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1319/static/js/main.18b7442e.js@5454153 | `(0,xe.jsx)(qY,{})` | 5 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1319/static/js/main.18b7442e.js@5454200 | `(0,xe.jsx)(YY,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1320 · RAZER BLACKSHARK V2 PRO

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1320]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1320/static/js/main.ac594f45.js@5203962 | `(0,He.jsx)(tB,{})` | 50 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1320/static/js/main.ac594f45.js@5204009 | `(0,He.jsx)(OB,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1320/static/js/main.ac594f45.js@5204056 | `(0,He.jsx)(Iy,{})` | 32 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1320/static/js/main.ac594f45.js@5204103 | `(0,He.jsx)(my,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1320/static/js/main.ac594f45.js@5204150 | `(0,He.jsx)(RV,{})` | 4 | 10 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1320/static/js/main.ac594f45.js@5204197 | `(0,He.jsx)(AV,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1321 · Razer USB Sound Card

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1321/static/js/main.7ed29db3.js@5264681 | `(0,ye.jsx)(fH,{})` | 50 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1321/static/js/main.7ed29db3.js@5264728 | `(0,ye.jsx)(wH,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1321/static/js/main.7ed29db3.js@5264775 | `(0,ye.jsx)(wy,{})` | 32 | 18 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1321/static/js/main.7ed29db3.js@5264822 | `(0,ye.jsx)(GF,{})` | 3 | 10 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1321/static/js/main.7ed29db3.js@5264869 | `(0,ye.jsx)(UF,{})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1325 · RAZER KRAKEN V3 PRO

类别：AUDIO；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1324]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1325/static/js/main.6d8e67f5.js@5627233 | `(0,je.jsx)(DU,{})` | 56 | 31 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1325/static/js/main.6d8e67f5.js@5627280 | `(0,je.jsx)(Qv,{})` | 18 | 9 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1325/static/js/main.6d8e67f5.js@5627327 | `(0,je.jsx)(wG,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1325/static/js/main.6d8e67f5.js@5627374 | `(0,je.jsx)(jb,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1325/static/js/main.6d8e67f5.js@5627421 | `(0,je.jsx)($G,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| DEMO / default | .ref/devices/1325/static/js/main.6d8e67f5.js@5627468 | `(0,je.jsx)(qH,{})` | 5 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1325/static/js/main.6d8e67f5.js@5627515 | `(0,je.jsx)(xH,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1328 · Razer Kraken BT Kitty Edition

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1328]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1328/static/js/main.8a79cd9c.js@4673338 | `(0,_E.jsx)(WM,{})` | 37 | 7 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1328/static/js/main.8a79cd9c.js@4673385 | `(0,_E.jsx)(KM,{})` | 14 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1328/static/js/main.8a79cd9c.js@4673432 | `(0,_E.jsx)(YG,{})` | 34 | 17 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1328/static/js/main.8a79cd9c.js@4673479 | `(0,_E.jsx)(KG,{})` | 11 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1328/static/js/main.8a79cd9c.js@4673526 | `(0,_E.jsx)(Tv,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1330 · RAZER LEVIATHAN V2

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1356]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1330/static/js/main.f0797abf.js@5456167 | `(0,We.jsx)(wB,{})` | 48 | 27 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1330/static/js/main.f0797abf.js@5456220 | `(0,We.jsx)(ry,{})` | 11 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1330/static/js/main.f0797abf.js@5456270 | `(0,We.jsx)(IV,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1330/static/js/main.f0797abf.js@5456326 | `(0,We.jsx)(FF,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1331 · RAZER KRAKEN V3 HYPERSENSE

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1331/static/js/main.cdddaa98.js@5412035 | `(0,We.jsx)(kB,{})` | 51 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1331/static/js/main.cdddaa98.js@5412082 | `(0,We.jsx)(Eb,{})` | 18 | 9 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1331/static/js/main.cdddaa98.js@5412129 | `(0,We.jsx)(aF,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1331/static/js/main.cdddaa98.js@5412176 | `(0,We.jsx)(Uk,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1331/static/js/main.cdddaa98.js@5412223 | `(0,We.jsx)(XV,{})` | 4 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1331/static/js/main.cdddaa98.js@5412270 | `(0,We.jsx)(zV,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1332 · RAZER KRAKEN BT SANRIO LIMITED EDITION

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1332]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1332/static/js/main.18a00d58.js@4673394 | `(0,_E.jsx)(WM,{})` | 37 | 7 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1332/static/js/main.18a00d58.js@4673441 | `(0,_E.jsx)(KM,{})` | 14 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1332/static/js/main.18a00d58.js@4673488 | `(0,_E.jsx)(YG,{})` | 34 | 17 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1332/static/js/main.18a00d58.js@4673535 | `(0,_E.jsx)(KG,{})` | 11 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1332/static/js/main.18a00d58.js@4673582 | `(0,_E.jsx)(Tv,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1335 · Razer Kraken V3 X

类别：AUDIO；edition：[0, 128, 129, 130]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1335/static/js/main.8345750d.js@4623175 | `(0,be.jsx)(Vc,{})` | 32 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1335/static/js/main.8345750d.js@4623222 | `(0,be.jsx)(iL,{})` | 33 | 17 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1335/static/js/main.8345750d.js@4623269 | `(0,be.jsx)(cf,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1337 · RAZER BARRACUDA PRO 2.4

类别：AUDIO；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1338]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1337/static/js/main.df767002.js@7023342 | `(0,He.jsx)(SH,{})` | 51 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1337/static/js/main.df767002.js@7023389 | `(0,He.jsx)($H,{})` | 20 | 9 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1337/static/js/main.df767002.js@7023436 | `(0,He.jsx)(ww,{})` | 24 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1337/static/js/main.df767002.js@7023483 | `(0,He.jsx)(Xw,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1337/static/js/main.df767002.js@7023530 | `(0,He.jsx)(VB,{})` | 5 | 10 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1337/static/js/main.df767002.js@7023577 | `(0,He.jsx)(wB,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1339 · Razer Barracuda 2.4

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1340]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1339/static/js/main.8817fdae.js@5236907 | `(0,He.jsx)(Dy,{})` | 51 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1339/static/js/main.8817fdae.js@5236954 | `(0,He.jsx)(Hy,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1339/static/js/main.8817fdae.js@5237001 | `(0,He.jsx)(PB,{})` | 24 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1339/static/js/main.8817fdae.js@5237048 | `(0,He.jsx)(FB,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1339/static/js/main.8817fdae.js@5237095 | `(0,He.jsx)(Ub,{})` | 5 | 10 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1339/static/js/main.8817fdae.js@5237142 | `(0,He.jsx)(Pb,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1342 · Razer Audio Mixer

类别：AUDIO_MIXER_AUDIO；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/1342/static/js/main.2623357c.js@6525346 | `(0,Qe.jsx)(WP,{})` | 26 | 6 | partial_static_reference_graph | partial_native |
| TAB_MIXER / default | .ref/devices/1342/static/js/main.2623357c.js@6525393 | `(0,Qe.jsx)(Ag,{changeView:e=>this.changeView(e)})` | 29 | 5 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1342/static/js/main.2623357c.js@6525472 | `(0,Qe.jsx)(MU,{})` | 31 | 8 | partial_static_reference_graph | partial_native |
| EFFECTS / default | .ref/devices/1342/static/js/main.2623357c.js@6525519 | `(0,Qe.jsx)(kU,{})` | 28 | 10 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1342/static/js/main.2623357c.js@6525566 | `(0,Qe.jsx)(OP,{})` | 32 | 9 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1342/static/js/main.2623357c.js@6525613 | `(0,Qe.jsx)(Rg,{resetObm:this.resetDevice,resetAudio:this.resetAudio})` | 12 | 2 | partial_static_reference_graph | partial_native |

## 1346 · Razer Seiren V2 Pro

类别：BROADCASTER_MICROPHONE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| MIC / default | .ref/devices/1346/static/js/main.4d98fb93.js@4508563 | `(0,We.jsx)(zL,{})` | 34 | 0 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/1346/static/js/main.4d98fb93.js@4508610 | `(0,We.jsx)(gP,{})` | 32 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1346/static/js/main.4d98fb93.js@4508657 | `(0,We.jsx)(jm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1347 · Razer Seiren V2 X

类别：BROADCASTER_MICROPHONE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| MIC / default | .ref/devices/1347/static/js/main.be19f1cb.js@4492106 | `(0,We.jsx)(zL,{})` | 34 | 0 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/1347/static/js/main.be19f1cb.js@4492153 | `(0,We.jsx)(gP,{})` | 32 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1347/static/js/main.be19f1cb.js@4492200 | `(0,We.jsx)(jm,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1352 · Razer Leviathan V2 Pro

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1368]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1352/static/js/main.95a4f703.js@4712603 | `(0,Y_.jsx)(LG,{})` | 72 | 23 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1352/static/js/main.95a4f703.js@4712650 | `(0,Y_.jsx)(KG,{})` | 20 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1352/static/js/main.95a4f703.js@4712697 | `(0,Y_.jsx)(Hv,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1352/static/js/main.95a4f703.js@4712744 | `(0,Y_.jsx)(wv,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1352/static/js/main.95a4f703.js@4712791 | `(0,Y_.jsx)(QM,{resetObm:!0})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1353 · RAZER KRAKEN V3

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1353/static/js/main.dfef4e81.js@5453890 | `(0,Ye.jsx)(ZF,{})` | 51 | 27 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1353/static/js/main.dfef4e81.js@5453937 | `(0,Ye.jsx)(sw,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1353/static/js/main.dfef4e81.js@5453984 | `(0,Ye.jsx)(tV,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1353/static/js/main.dfef4e81.js@5454031 | `(0,Ye.jsx)(Hz,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1353/static/js/main.dfef4e81.js@5454078 | `(0,Ye.jsx)(QY,{})` | 4 | 9 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1353/static/js/main.dfef4e81.js@5454125 | `(0,Ye.jsx)(xY,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1354 · Razer Leviathan V2 X

类别：AUDIO_SPEAKER；edition：[0, 128, 129, 131, 132]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1369]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1354/static/js/main.e16a6fa2.js@4600764 | `(0,W_.jsx)(bG,{})` | 48 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1354/static/js/main.e16a6fa2.js@4600811 | `(0,W_.jsx)(hH,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1354/static/js/main.e16a6fa2.js@4600858 | `(0,W_.jsx)(FH,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1354/static/js/main.e16a6fa2.js@4600905 | `(0,W_.jsx)(nh,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1364 · Razer Kitty V2 Pro

类别：AUDIO；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1364/static/js/main.6cf04746.js@5448876 | `(0,xe.jsx)(nF,{})` | 51 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1364/static/js/main.6cf04746.js@5448923 | `(0,xe.jsx)(HF,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1364/static/js/main.6cf04746.js@5448970 | `(0,xe.jsx)(Dw,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1364/static/js/main.6cf04746.js@5449017 | `(0,xe.jsx)(Dz,{})` | 35 | 33 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1364/static/js/main.6cf04746.js@5449064 | `(0,xe.jsx)(uY,{})` | 4 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1364/static/js/main.6cf04746.js@5449111 | `(0,xe.jsx)(TY,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1365 · RAZER BLACKSHARK V2 PRO

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1365]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1365/static/js/main.4949c2ce.js@5024459 | `(0,ye.jsx)(Lh,{})` | 51 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1365/static/js/main.4949c2ce.js@5024506 | `(0,ye.jsx)(tU,{})` | 17 | 9 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1365/static/js/main.4949c2ce.js@5024553 | `(0,ye.jsx)(kU,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1365/static/js/main.4949c2ce.js@5024600 | `(0,ye.jsx)(nf,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1365/static/js/main.4949c2ce.js@5024647 | `(0,ye.jsx)(Sv,{})` | 4 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1365/static/js/main.4949c2ce.js@5024694 | `(0,ye.jsx)(iv,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1370 · RAZER NOMMO V2 PRO

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1371]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1370/static/js/main.55a82042.js@5237815 | `(0,Be.jsx)(yH,{})` | 54 | 33 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1370/static/js/main.55a82042.js@5237862 | `(0,Be.jsx)(RF,{})` | 11 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1370/static/js/main.55a82042.js@5237909 | `(0,Be.jsx)(SW,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1370/static/js/main.55a82042.js@5237956 | `(0,Be.jsx)($H,{})` | 10 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1370/static/js/main.55a82042.js@5238003 | `(0,Be.jsx)(Wb,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1372 · RAZER NOMMO V2

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1373]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1372/static/js/main.0948803f.js@5288129 | `(0,Be.jsx)(WB,{})` | 54 | 33 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1372/static/js/main.0948803f.js@5288176 | `(0,Be.jsx)(Cw,{})` | 13 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1372/static/js/main.0948803f.js@5288223 | `(0,Be.jsx)(uY,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1372/static/js/main.0948803f.js@5288270 | `(0,Be.jsx)(ny,{})` | 10 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1372/static/js/main.0948803f.js@5288317 | `(0,Be.jsx)(KF,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1374 · RAZER NOMMO V2 X

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1374/static/js/main.e6cd4110.js@4881352 | `(0,Ue.jsx)(gM,{})` | 50 | 32 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1374/static/js/main.e6cd4110.js@4881399 | `(0,Ue.jsx)(RG,{})` | 9 | 3 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1374/static/js/main.e6cd4110.js@4881446 | `(0,Ue.jsx)(XM,{})` | 10 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1374/static/js/main.e6cd4110.js@4881493 | `(0,Ue.jsx)(xg,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1376 · Razer Kraken Kitty V2

类别：AUDIO；edition：[0, 128, 129, 130, 131, 132, 133, 134]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1376/static/js/main.03a3e8b2.js@4679545 | `(0,sE.jsx)(fM,{})` | 31 | 6 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1376/static/js/main.03a3e8b2.js@4679592 | `(0,sE.jsx)(KM,{})` | 14 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1376/static/js/main.03a3e8b2.js@4679639 | `(0,sE.jsx)(VG,{})` | 36 | 17 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1376/static/js/main.03a3e8b2.js@4679686 | `(0,sE.jsx)(_v,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1378 · Razer Kraken Kitty V2 BT Quartz Edition

类别：AUDIO；edition：[0, 128, 129, 130, 131, 132, 133, 134, 137, 138]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1378]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1378/static/js/main.2397f4bd.js@4671770 | `(0,eE.jsx)(dM,{})` | 32 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1378/static/js/main.2397f4bd.js@4671817 | `(0,eE.jsx)(CG,{})` | 36 | 17 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1378/static/js/main.2397f4bd.js@4671864 | `(0,eE.jsx)(pG,{})` | 11 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1378/static/js/main.2397f4bd.js@4671911 | `(0,eE.jsx)(MH,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1382 · Razer WIRELESS CONTROL POD

类别：ACCESSORY；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/1382/static/js/main.275d3b94.js@4698814 | `(0,Ja.jsx)(sL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 68 | 26 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1382/static/js/main.275d3b94.js@4698911 | `(0,Ja.jsx)(rc,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1383 · Razer Kraken V4 Pro

类别：AUDIO；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1384]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1394]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674301 | `(0,P.jsx)(Lr,{})` | 89 | 35 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674346 | `(0,P.jsx)(Dl,{})` | 13 | 5 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674391 | `(0,P.jsx)(gd,{})` | 39 | 10 | partial_static_reference_graph | partial_native |
| TAB_HAPTICS / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674436 | `(0,P.jsx)(dp,{})` | 74 | 27 | partial_static_reference_graph | partial_native |
| TAB_OLED / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674481 | `(0,P.jsx)(xx,{})` | 122 | 18 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674526 | `(0,P.jsx)(Hc.A,{})` | 35 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674573 | `(0,P.jsx)(Td,{})` | 11 | 4 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674618 | `(0,P.jsx)(Ec,{})` | 33 | 26 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1383/static/js/6141.5d00192e.chunk.js@674663 | `(0,P.jsx)(yc,{resetObm:this.factoryReset,oledfactoryReset:!0,oledResetInitialState:Wu.u,oledLoading:this.props.oledLoading})` | 24 | 6 | partial_static_reference_graph | partial_native |

## 1386 · Razer Seiren V3 Mini

类别：BROADCASTER_MICROPHONE；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| MIC / default | .ref/devices/1386/static/js/1117.cf94275c.chunk.js@319110 | `(0,N.jsx)(wa.default,{})` | 32 | 1 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/1386/static/js/1117.cf94275c.chunk.js@319163 | `(0,N.jsx)(Da.default,{})` | 32 | 2 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1386/static/js/1117.cf94275c.chunk.js@319216 | `(0,N.jsx)(Ia.default,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1387 · Razer Kraken V4

类别：AUDIO；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1388]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1387/static/js/main.c3a5df87.js@5369985 | `(0,tI.jsx)(XD,{})` | 52 | 39 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1387/static/js/main.c3a5df87.js@5370032 | `(0,tI.jsx)(gL,{})` | 12 | 9 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1387/static/js/main.c3a5df87.js@5370079 | `(0,tI.jsx)(lp,{})` | 26 | 21 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1387/static/js/main.c3a5df87.js@5370126 | `(0,tI.jsx)(_f,{})` | 23 | 33 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1387/static/js/main.c3a5df87.js@5370173 | `(0,tI.jsx)(Cf,{})` | 8 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1387/static/js/main.c3a5df87.js@5370220 | `(0,tI.jsx)(Dh,{})` | 4 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1387/static/js/main.c3a5df87.js@5370267 | `(0,tI.jsx)(Th,{resetObm:this.factoryReset})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 1389 · Razer Kraken V4 X

类别：AUDIO；edition：[0, 128, 129, 130, 131, 132, 133, 134, 135, 136]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1389/static/js/main.7ceeec13.js@4660989 | `(0,w_.jsx)(wU,{})` | 34 | 0 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1389/static/js/main.7ceeec13.js@4661036 | `(0,w_.jsx)(NH,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1389/static/js/main.7ceeec13.js@4661083 | `(0,w_.jsx)(Jf,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1389/static/js/main.7ceeec13.js@4661130 | `(0,w_.jsx)(vM,{resetObm:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1390 · RAZER BLACKSHARK V2 HYPERSPEED

类别：AUDIO；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1381]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1390/static/js/main.dace2c06.js@5443512 | `(0,Ve.jsx)(aB,{})` | 51 | 28 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1390/static/js/main.dace2c06.js@5443559 | `(0,Ve.jsx)(mb,{})` | 17 | 9 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1390/static/js/main.dace2c06.js@5443606 | `(0,Ve.jsx)(AF,{})` | 33 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1390/static/js/main.dace2c06.js@5443653 | `(0,Ve.jsx)(VF,{})` | 24 | 8 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1390/static/js/main.dace2c06.js@5443700 | `(0,Ve.jsx)(VW,{})` | 4 | 11 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1390/static/js/main.dace2c06.js@5443747 | `(0,Ve.jsx)(hW,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1391 · Razer Seiren V3 Chroma

类别：BROADCASTER_MICROPHONE；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| MIC / default | .ref/devices/1391/static/js/1915.aaeeae2a.chunk.js@203811 | `(0,_.jsx)(bi.default,{})` | 32 | 1 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/1391/static/js/1915.aaeeae2a.chunk.js@203864 | `(0,_.jsx)(Ci.default,{})` | 31 | 2 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1391/static/js/1915.aaeeae2a.chunk.js@203917 | `(0,_.jsx)(Pi.default,{})` | 48 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1391/static/js/1915.aaeeae2a.chunk.js@203970 | `(0,_.jsx)(Oi.default,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1392 · RAZER CLIO

类别：AUDIO_SPEAKER_HEAD_CUSHION；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1392]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1392/static/js/main.6fccfd1e.js@5757943 | `(0,Be.jsx)(Og,{})` | 43 | 17 | partial_static_reference_graph | partial_native |
| SURROUND / default | .ref/devices/1392/static/js/main.6fccfd1e.js@5757996 | `(0,Be.jsx)(Cv,{})` | 21 | 15 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1392/static/js/main.6fccfd1e.js@5758048 | `(0,Be.jsx)(Dg,{})` | 11 | 6 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1392/static/js/main.6fccfd1e.js@5758101 | `(0,Be.jsx)(uG,{})` | 4 | 12 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1392/static/js/main.6fccfd1e.js@5758153 | `(0,Be.jsx)(AG,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1396 · Razer Barracuda X Chroma

类别：AUDIO；edition：[0, 128, 129, 130, 131]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1396]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1396/static/js/main.807d655b.js@4918737 | `(0,we.jsx)(sM,{})` | 29 | 10 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1396/static/js/main.807d655b.js@4918784 | `(0,we.jsx)(KM,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1396/static/js/main.807d655b.js@4918831 | `(0,we.jsx)(Ah,{})` | 12 | 7 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1396/static/js/main.807d655b.js@4918878 | `(0,we.jsx)(ig,{})` | 32 | 32 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1396/static/js/main.807d655b.js@4918925 | `(0,we.jsx)(Tg,{})` | 8 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1396/static/js/main.807d655b.js@4918972 | `(0,we.jsx)(gH,{resetObm:this.resetDevice})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1398 · Razer BlackShark V3 Pro

类别：AUDIO；edition：[0, 128, 129, 130, 131, 132]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1399]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1397]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1398/static/js/main.cab68a9a.js@5685439 | `(0,uI.jsx)(Rm,{})` | 69 | 50 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/1398/static/js/main.cab68a9a.js@5685486 | `(0,uI.jsx)(WF,{})` | 19 | 15 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1398/static/js/main.cab68a9a.js@5685533 | `(0,uI.jsx)(Lh,{})` | 19 | 13 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1398/static/js/main.cab68a9a.js@5685580 | `(0,uI.jsx)(GM,{})` | 36 | 27 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1398/static/js/main.cab68a9a.js@5685627 | `(0,uI.jsx)(wb,{})` | 21 | 7 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1398/static/js/main.cab68a9a.js@5685674 | `(0,uI.jsx)(GH,{})` | 36 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1398/static/js/main.cab68a9a.js@5685721 | `(0,uI.jsx)(Gf,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:il.MVq},resetTitle:il.mY5,hasTutorial:!0,hasTHXPartialAudio:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 1401 · Razer BlackShark V3

类别：AUDIO；edition：[0, 128, 129, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1402]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1400]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1401/static/js/main.f418a62d.js@5669999 | `(0,cI.jsx)(um,{})` | 69 | 50 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/1401/static/js/main.f418a62d.js@5670046 | `(0,cI.jsx)(Lb,{})` | 19 | 15 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1401/static/js/main.f418a62d.js@5670093 | `(0,cI.jsx)(sh,{})` | 15 | 12 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1401/static/js/main.f418a62d.js@5670140 | `(0,cI.jsx)(cM,{})` | 36 | 27 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1401/static/js/main.f418a62d.js@5670187 | `(0,cI.jsx)(mF,{})` | 21 | 7 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1401/static/js/main.f418a62d.js@5670234 | `(0,cI.jsx)(cH,{})` | 36 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1401/static/js/main.f418a62d.js@5670281 | `(0,cI.jsx)(cf,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:al.MVq},resetTitle:al.mY5,hasTutorial:!0,hasTHXPartialAudio:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 1404 · Razer BlackShark V3 X Hyperspeed

类别：AUDIO；edition：[0, 128, 129, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1405]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1403]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1404/static/js/main.d350fd68.js@4771394 | `(0,ae.jsx)(rL,{})` | 41 | 17 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1404/static/js/main.d350fd68.js@4771441 | `(0,ae.jsx)(xL,{})` | 18 | 12 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1404/static/js/main.d350fd68.js@4771488 | `(0,ae.jsx)(rP,{})` | 10 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1404/static/js/main.d350fd68.js@4771535 | `(0,ae.jsx)(oM,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:"You are about to reset this device back to its factory settings. All custom settings stored on the device will be erased."},resetTitle:"Performing a factory reset will erase all custom settings stored on this device."})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1406 · Razer Kraken V2 BT Hello Kitty and Friends Edition

类别：AUDIO；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1406]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1406/static/js/main.f2aa3af5.js@4705715 | `(0,oE.jsx)(bM,{})` | 32 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1406/static/js/main.f2aa3af5.js@4705762 | `(0,oE.jsx)(jG,{})` | 36 | 17 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1406/static/js/main.f2aa3af5.js@4705809 | `(0,oE.jsx)(JG,{})` | 11 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1406/static/js/main.f2aa3af5.js@4705856 | `(0,oE.jsx)(dv,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1407 · Razer &#124; Sanrio Characters Limited Edition Wireless Headset

类别：AUDIO；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1407]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1407/static/js/main.cbabe455.js@4705715 | `(0,oE.jsx)(bM,{})` | 32 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1407/static/js/main.cbabe455.js@4705762 | `(0,oE.jsx)(jG,{})` | 36 | 17 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1407/static/js/main.cbabe455.js@4705809 | `(0,oE.jsx)(JG,{})` | 11 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1407/static/js/main.cbabe455.js@4705856 | `(0,oE.jsx)(dv,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1411 · Razer Barracuda Pro

类别：AUDIO；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1411]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1411/static/js/3816.a2c45b2e.chunk.js@475514 | `(0,P.jsx)(hl,{})` | 56 | 3 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1411/static/js/3816.a2c45b2e.chunk.js@475559 | `(0,P.jsx)(Ol,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1411/static/js/3816.a2c45b2e.chunk.js@475604 | `(0,P.jsx)(Dd,{})` | 28 | 1 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1411/static/js/3816.a2c45b2e.chunk.js@475649 | `(0,P.jsx)(Vd,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1411/static/js/3816.a2c45b2e.chunk.js@475694 | `(0,P.jsx)(Rc,{})` | 35 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1411/static/js/3816.a2c45b2e.chunk.js@475739 | `(0,P.jsx)(Nc,{resetObm:this.factoryReset})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1412 · Razer Barracuda 2.4

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1412]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1412/static/js/697.3421df89.chunk.js@491860 | `(0,O.jsx)(El,{})` | 56 | 3 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1412/static/js/697.3421df89.chunk.js@491905 | `(0,O.jsx)(Ol,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1412/static/js/697.3421df89.chunk.js@491950 | `(0,O.jsx)(Dd,{})` | 28 | 1 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1412/static/js/697.3421df89.chunk.js@491995 | `(0,O.jsx)(Bd,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1412/static/js/697.3421df89.chunk.js@492040 | `(0,O.jsx)(Tc,{})` | 35 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1412/static/js/697.3421df89.chunk.js@492085 | `(0,O.jsx)(bc,{resetObm:this.factoryReset})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1415 · Razer Kraken Kitty V3 Pro

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1416]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [1417]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1415/static/js/281.844e6a1f.chunk.js@662578 | `(0,L.jsx)(_l,{})` | 92 | 36 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1415/static/js/281.844e6a1f.chunk.js@662623 | `(0,L.jsx)(id,{})` | 14 | 5 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1415/static/js/281.844e6a1f.chunk.js@662668 | `(0,L.jsx)(bd,{})` | 30 | 10 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1415/static/js/281.844e6a1f.chunk.js@662713 | `(0,L.jsx)(Bc.A,{})` | 35 | 18 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1415/static/js/281.844e6a1f.chunk.js@662760 | `(0,L.jsx)(ap,{})` | 9 | 3 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1415/static/js/281.844e6a1f.chunk.js@662805 | `(0,L.jsx)(kc,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1415/static/js/281.844e6a1f.chunk.js@662850 | `(0,L.jsx)(xc,{resetObm:this.factoryReset})` | 22 | 9 | partial_static_reference_graph | partial_native |

## 1420 · Razer Hammerhead V3

类别：AUDIO；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1420/static/js/main.4f81d888.js@4409190 | `(0,Fe.jsx)(QL,{})` | 32 | 5 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1420/static/js/main.4f81d888.js@4409237 | `(0,Fe.jsx)(AP,{})` | 12 | 3 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1420/static/js/main.4f81d888.js@4409284 | `(0,Fe.jsx)(bm,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1422 · Razer Seiren V3 Pro

类别：BROADCASTER_MICROPHONE；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_EQ / default | .ref/devices/1422/static/js/684.f6b88b50.chunk.js@426838 | `(0,E.jsx)(wi.default,{})` | 31 | 2 | partial_static_reference_graph | partial_native |
| TAB_EFFECTS / default | .ref/devices/1422/static/js/684.f6b88b50.chunk.js@426891 | `(0,E.jsx)(Mi.default,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/1422/static/js/684.f6b88b50.chunk.js@426944 | `(0,E.jsx)(Si.default,{})` | 27 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1422/static/js/684.f6b88b50.chunk.js@426997 | `(0,E.jsx)(Ei.default,{})` | 42 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1422/static/js/684.f6b88b50.chunk.js@427050 | `(0,E.jsx)(yi.default,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1427 · Razer Madeline T1

类别：AUDIO；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1426]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1427/static/js/789.bffae9a6.chunk.js@752675 | `(0,F.jsx)(Wl,{})` | 97 | 34 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/1427/static/js/789.bffae9a6.chunk.js@752720 | `(0,F.jsx)(Y_,{})` | 25 | 5 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1427/static/js/789.bffae9a6.chunk.js@752765 | `(0,F.jsx)(Yc,{})` | 25 | 4 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1427/static/js/789.bffae9a6.chunk.js@752810 | `(0,F.jsx)(ep,{})` | 44 | 9 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1427/static/js/789.bffae9a6.chunk.js@752855 | `(0,F.jsx)(Lm,{})` | 10 | 3 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1427/static/js/789.bffae9a6.chunk.js@752900 | `(0,F.jsx)(Cm,{})` | 39 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1427/static/js/789.bffae9a6.chunk.js@752945 | `(0,F.jsx)(Hp,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:j.MVq},resetTitle:j.mY5,hasTutorial:!1,hasTHXPartialAudio:!0})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1439 · Razer Kraken V4 X Sensa HD Haptics

类别：AUDIO；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1439/static/js/main.8adbaf05.js@4804328 | `(0,Wi.jsx)(WG,{})` | 34 | 0 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1439/static/js/main.8adbaf05.js@4804375 | `(0,Wi.jsx)(TH,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_HAPTICS / default | .ref/devices/1439/static/js/main.8adbaf05.js@4804422 | `(0,Wi.jsx)(VW,{})` | 73 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1439/static/js/main.8adbaf05.js@4804469 | `(0,Wi.jsx)(kv,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1439/static/js/main.8adbaf05.js@4804516 | `(0,Wi.jsx)(bh,{resetObm:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1442 · RAZER CLIO X

类别：AUDIO_SPEAKER_HEAD_CUSHION_CLIO_X；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [1442]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1442/static/js/3737.a871f924.chunk.js@559030 | `(0,R.jsx)(Yc,{})` | 43 | 7 | partial_static_reference_graph | partial_native |
| SURROUND / default | .ref/devices/1442/static/js/3737.a871f924.chunk.js@559081 | `(0,R.jsx)(hp,{})` | 23 | 6 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1442/static/js/3737.a871f924.chunk.js@559131 | `(0,R.jsx)(hd,{})` | 12 | 3 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/1442/static/js/3737.a871f924.chunk.js@559182 | `(0,R.jsx)(lm,{})` | 34 | 26 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1442/static/js/3737.a871f924.chunk.js@559232 | `(0,R.jsx)(sm,{resetObm:this.factoryReset})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1443 · RAZER LEVIATHAN V2

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1443/static/js/main.a3f7f9d1.js@5578779 | `(0,Be.jsx)(FM,{})` | 48 | 27 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1443/static/js/main.a3f7f9d1.js@5578832 | `(0,Be.jsx)(EU,{})` | 11 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1443/static/js/main.a3f7f9d1.js@5578882 | `(0,Be.jsx)(tg,{})` | 30 | 30 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1443/static/js/main.a3f7f9d1.js@5578938 | `(0,Be.jsx)(Gh,{resetObm:this.factoryReset})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1446 · Razer Seiren V3 Pro

类别：BROADCASTER_MICROPHONE；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| MIC / default | .ref/devices/1446/static/js/684.9501aa81.chunk.js@439348 | `(0,y.jsx)(Si.default,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1446/static/js/684.9501aa81.chunk.js@439401 | `(0,y.jsx)(Mi.default,{})` | 31 | 2 | partial_static_reference_graph | partial_native |
| TAB_EFFECTS / default | .ref/devices/1446/static/js/684.9501aa81.chunk.js@439454 | `(0,y.jsx)(Ni.default,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/1446/static/js/684.9501aa81.chunk.js@439507 | `(0,y.jsx)(yi.default,{})` | 27 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1446/static/js/684.9501aa81.chunk.js@439560 | `(0,y.jsx)(Ei.default,{})` | 48 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1446/static/js/684.9501aa81.chunk.js@439613 | `(0,y.jsx)(wi.default,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 1453 · RAZER MAKO X

类别：AUDIO_SPEAKER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1453/static/js/993.82aa2be7.chunk.js@408116 | `(0,w.jsx)(vr,{})` | 46 | 6 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1453/static/js/993.82aa2be7.chunk.js@408161 | `(0,w.jsx)(dc,{})` | 19 | 5 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1453/static/js/993.82aa2be7.chunk.js@408206 | `(0,w.jsx)(Rc,{})` | 13 | 3 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1453/static/js/993.82aa2be7.chunk.js@408251 | `(0,w.jsx)(Oc.A,{})` | 37 | 17 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/1453/static/js/993.82aa2be7.chunk.js@408298 | `(0,w.jsx)(Rr,{})` | 12 | 3 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1453/static/js/993.82aa2be7.chunk.js@408343 | `(0,w.jsx)(El,{hasTutorial:!0,resetObm:this.factoryReset})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 1462 · Razer Kraken Kitty V3

类别：AUDIO；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1462/static/js/main.77653bee.js@4893165 | `(0,ye.jsx)(uM,{})` | 29 | 13 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1462/static/js/main.77653bee.js@4893212 | `(0,ye.jsx)(JM,{})` | 15 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1462/static/js/main.77653bee.js@4893259 | `(0,ye.jsx)(Ph,{})` | 16 | 13 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/1462/static/js/main.77653bee.js@4893306 | `(0,ye.jsx)(cg,{})` | 37 | 35 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1462/static/js/main.77653bee.js@4893353 | `(0,ye.jsx)(LH,{resetObm:this.resetDevice})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 1465 · Razer Hammerhead V3 Chroma

类别：AUDIO_EARPHONES；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'wiredId', 'ids': [1465]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/1465/static/js/main.73ac4806.js@5202023 | `(0,bO.jsx)(NN,{})` | 27 | 20 | partial_static_reference_graph | partial_native |
| TAB_EQ / default | .ref/devices/1465/static/js/main.73ac4806.js@5202070 | `(0,bO.jsx)(xC,{})` | 9 | 9 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/1465/static/js/main.73ac4806.js@5202117 | `(0,bO.jsx)(rC,{})` | 7 | 8 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/1465/static/js/main.73ac4806.js@5202164 | `(0,bO.jsx)(tC,{})` | 17 | 10 | partial_static_reference_graph | partial_native |
| LIGHTING / default | .ref/devices/1465/static/js/main.73ac4806.js@5202211 | `(0,bO.jsx)(mP,{})` | 18 | 31 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/1465/static/js/main.73ac4806.js@5202258 | `(0,bO.jsx)(HM,{resetObm:this.resetDevice})` | 18 | 22 | partial_static_reference_graph | partial_native |

## 2594 · Razer Turret Mouse Gears Of War 5 Edition

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2593]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2594/static/js/main.877e745a.js@4954846 | `(0,Y.jsx)(cT,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 61 | 4 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/2594/static/js/main.877e745a.js@4954942 | `(0,Y.jsx)(bI,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/2594/static/js/main.877e745a.js@4954988 | `(0,Y.jsx)(mS,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2594/static/js/main.877e745a.js@4955034 | `(0,Y.jsx)(xR,{})` | 18 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2594/static/js/main.877e745a.js@4955080 | `(0,Y.jsx)(GR,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2594/static/js/main.877e745a.js@4955126 | `(0,Y.jsx)(UM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2595 · Turret Keyboard Gears Of War 5 Edition

类别：KEYBOARD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2593]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2595/static/js/main.01d8ae5b.js@6807817 | `(0,Yn.jsx)(cp,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 95 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/2595/static/js/main.01d8ae5b.js@6807914 | `(0,Yn.jsx)(uM,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2595/static/js/main.01d8ae5b.js@6807961 | `(0,Yn.jsx)(fM,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2595/static/js/main.01d8ae5b.js@6808008 | `(0,Yn.jsx)(Zu,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2596 · RAZER BLACKWIDOW V3 TENKEYLESS

类别：KEYBOARD；edition：[0, 128, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2596/static/js/main.eff25ed8.js@7428603 | `(0,wa.jsx)(MO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 98 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/2596/static/js/main.eff25ed8.js@7428700 | `(0,wa.jsx)(bc,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2596/static/js/main.eff25ed8.js@7428747 | `(0,wa.jsx)(_M,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2629 · Razer Wolverine V3 Tournament Edition

类别：GAMEPAD；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2627]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2629/static/js/main.000315e2.js@6763284 | `()=>(0,vn.jsx)(XP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 80 | 4 | partial_static_reference_graph | partial_native |
| TRIGGERS / default | .ref/devices/2629/static/js/main.000315e2.js@6763391 | `()=>(0,vn.jsx)(eh,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/2629/static/js/main.000315e2.js@6763448 | `()=>(0,vn.jsx)(Jh,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2629/static/js/main.000315e2.js@6763505 | `()=>(0,vn.jsx)(iM,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2629/static/js/main.000315e2.js@6763562 | `()=>(0,vn.jsx)(GN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2636 · Razer Wolverine V3 Pro

类别：GAMEPAD；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2623]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2636/static/js/main.82d8a835.js@6704172 | `()=>(0,li.jsx)(Kh,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 94 | 8 | partial_static_reference_graph | partial_native |
| TRIGGERS / default | .ref/devices/2636/static/js/main.82d8a835.js@6704279 | `()=>(0,li.jsx)(Xh,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/2636/static/js/main.82d8a835.js@6704336 | `()=>(0,li.jsx)(kM,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/2636/static/js/main.82d8a835.js@6704393 | `()=>(0,li.jsx)(sm,{})` | 22 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2636/static/js/main.82d8a835.js@6704450 | `()=>(0,li.jsx)(cm,{})` | 9 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2636/static/js/main.82d8a835.js@6704507 | `()=>(0,li.jsx)(qM,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2636/static/js/main.82d8a835.js@6704564 | `()=>(0,li.jsx)(VN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2638 · Razer BlackShark V3 Pro XBOX

类别：AUDIO；edition：[0, 128, 129, 130]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2645]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [2639]}, {'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2637]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2638/static/js/main.43474509.js@5672537 | `(0,uI.jsx)(Rm,{})` | 69 | 50 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2638/static/js/main.43474509.js@5672584 | `(0,uI.jsx)(Wb,{})` | 19 | 15 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/2638/static/js/main.43474509.js@5672631 | `(0,uI.jsx)(Lh,{})` | 19 | 13 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2638/static/js/main.43474509.js@5672678 | `(0,uI.jsx)(GM,{})` | 36 | 27 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2638/static/js/main.43474509.js@5672725 | `(0,uI.jsx)(wF,{})` | 21 | 7 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/2638/static/js/main.43474509.js@5672772 | `(0,uI.jsx)(GH,{})` | 36 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2638/static/js/main.43474509.js@5672819 | `(0,uI.jsx)(Gf,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:il.MVq},resetTitle:il.mY5,hasTutorial:!0,hasTHXPartialAudio:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 2641 · Razer BlackShark V3 XBOX

类别：AUDIO；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2648]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [2642]}, {'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2640]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2641/static/js/main.b410e3b4.js@5669536 | `(0,cI.jsx)(um,{})` | 69 | 50 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2641/static/js/main.b410e3b4.js@5669583 | `(0,cI.jsx)(Lb,{})` | 19 | 15 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/2641/static/js/main.b410e3b4.js@5669630 | `(0,cI.jsx)(sh,{})` | 15 | 12 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2641/static/js/main.b410e3b4.js@5669677 | `(0,cI.jsx)(cM,{})` | 36 | 27 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2641/static/js/main.b410e3b4.js@5669724 | `(0,cI.jsx)(mF,{})` | 21 | 7 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/2641/static/js/main.b410e3b4.js@5669771 | `(0,cI.jsx)(cH,{})` | 36 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2641/static/js/main.b410e3b4.js@5669818 | `(0,cI.jsx)(cf,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:al.MVq},resetTitle:al.mY5,hasTutorial:!0,hasTHXPartialAudio:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 2644 · Razer BlackShark V3 X Hyperspeed

类别：AUDIO；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2651]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [2646]}, {'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2643]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2644/static/js/main.74ecb9b2.js@4779416 | `(0,ae.jsx)(WD,{})` | 41 | 17 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2644/static/js/main.74ecb9b2.js@4779463 | `(0,ae.jsx)(BL,{})` | 26 | 14 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2644/static/js/main.74ecb9b2.js@4779510 | `(0,ae.jsx)(QL,{})` | 10 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2644/static/js/main.74ecb9b2.js@4779557 | `(0,ae.jsx)(qm,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:Te.kTe},resetTitle:(0,B.JN)(Te.ARF)})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 2647 · Razer Wolverine V3 Pro PC

类别：GAMEPAD；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2649]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2647/static/js/main.7aa41341.js@6672176 | `()=>(0,mn.jsx)(pL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 4 | partial_static_reference_graph | partial_native |
| TRIGGERS / default | .ref/devices/2647/static/js/main.7aa41341.js@6672283 | `()=>(0,mn.jsx)(ap,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/2647/static/js/main.7aa41341.js@6672340 | `()=>(0,mn.jsx)(mm,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2647/static/js/main.7aa41341.js@6672397 | `()=>(0,mn.jsx)(XD,{})` | 9 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2647/static/js/main.7aa41341.js@6672454 | `()=>(0,mn.jsx)(Gm,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2647/static/js/main.7aa41341.js@6672511 | `()=>(0,mn.jsx)(LN,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2650 · Razer Wolverine V3 TE PC

类别：GAMEPAD；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2650/static/js/main.257d35dd.js@6627251 | `()=>(0,tn.jsx)($C,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 74 | 4 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/2650/static/js/main.257d35dd.js@6627358 | `()=>(0,tn.jsx)(nm,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2650/static/js/main.257d35dd.js@6627415 | `()=>(0,tn.jsx)(Tm,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2650/static/js/main.257d35dd.js@6627472 | `()=>(0,tn.jsx)(nN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2657 · Razer Hammerhead V3 HyperSpeed

类别：AUDIO_EARBUDS；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2657]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2657/static/js/main.3bb092b2.js@4821925 | `(0,Ve.jsx)(mM,{})` | 46 | 20 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/2657/static/js/main.3bb092b2.js@4821972 | `(0,Ve.jsx)(Ch,{})` | 31 | 20 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2657/static/js/main.3bb092b2.js@4822019 | `(0,Ve.jsx)(YM,{})` | 15 | 7 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2657/static/js/main.3bb092b2.js@4822066 | `(0,Ve.jsx)(Vf,{})` | 13 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2657/static/js/main.3bb092b2.js@4822113 | `(0,Ve.jsx)(mf,{resetObm:this.resetDevice})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 2660 · Razer Hammerhead V3 X HyperSpeed

类别：AUDIO_EARBUDS；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2660]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2660/static/js/288.3c430057.chunk.js@333085 | `(0,x.jsx)(qo,{})` | 44 | 20 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/2660/static/js/288.3c430057.chunk.js@333130 | `(0,x.jsx)(Vn,{})` | 26 | 14 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2660/static/js/288.3c430057.chunk.js@333175 | `(0,x.jsx)(pn,{})` | 16 | 6 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2660/static/js/288.3c430057.chunk.js@333220 | `(0,x.jsx)(Wn,{})` | 14 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2660/static/js/288.3c430057.chunk.js@333265 | `(0,x.jsx)(wr,{resetObm:this.resetDevice})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 2664 · Razer Hammerhead V3 X Hyperspeed for Xbox

类别：AUDIO_EARBUDS；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2664]}, {'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2665]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2664/static/js/main.ecec90b9.js@4048192 | `(0,me.jsx)(uL,{})` | 47 | 20 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/2664/static/js/main.ecec90b9.js@4048239 | `(0,me.jsx)(EP,{})` | 24 | 14 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2664/static/js/main.ecec90b9.js@4048286 | `(0,me.jsx)(GL,{})` | 14 | 7 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2664/static/js/main.ecec90b9.js@4048333 | `(0,me.jsx)(sP,{})` | 12 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2664/static/js/main.ecec90b9.js@4048380 | `(0,me.jsx)(VP,{resetObm:this.resetDevice})` | 21 | 13 | partial_static_reference_graph | partial_native |

## 2668 · Razer Hammerhead V3 X HyperSpeed For PlayStation

类别：AUDIO_EARBUDS；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2668]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2668/static/js/606.0edd3284.chunk.js@335314 | `(0,E.jsx)(nn,{})` | 51 | 20 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/2668/static/js/606.0edd3284.chunk.js@335359 | `(0,E.jsx)(Fn,{})` | 26 | 14 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2668/static/js/606.0edd3284.chunk.js@335404 | `(0,E.jsx)(Sn,{})` | 16 | 6 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2668/static/js/606.0edd3284.chunk.js@335449 | `(0,E.jsx)(qn,{})` | 14 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2668/static/js/606.0edd3284.chunk.js@335494 | `(0,E.jsx)(Ir,{resetObm:this.resetDevice})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 2676 · Razer Wolverine V4 Pro

类别：GAMEPAD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2680, 2681]}, {'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2674, 2678]}, {'source': 'AvailableDevices.json', 'kind': 'wiredId', 'ids': [2676, 2677]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2676/static/js/main.5ee115d4.js@6819863 | `()=>(0,yn.jsx)(fL,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 83 | 5 | partial_static_reference_graph | partial_native |
| TRIGGERS / default | .ref/devices/2676/static/js/main.5ee115d4.js@6819970 | `()=>(0,yn.jsx)(zL,{})` | 21 | 0 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/2676/static/js/main.5ee115d4.js@6820027 | `()=>(0,yn.jsx)(Hm,{})` | 40 | 1 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2676/static/js/main.5ee115d4.js@6820084 | `()=>(0,yn.jsx)(Vm,{})` | 9 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2676/static/js/main.5ee115d4.js@6820141 | `()=>(0,yn.jsx)(np,{})` | 28 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2676/static/js/main.5ee115d4.js@6820198 | `()=>(0,yn.jsx)(rN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2684 · Razer Silver T2 X

类别：GAMEPAD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'xBoxId', 'ids': [2682]}, {'source': 'AvailableDevices.json', 'kind': 'wiredId', 'ids': [2684, 2685]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/2684/static/js/8169.3cf63c70.chunk.js@547321 | `()=>(0,A.jsx)(ul,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 83 | 5 | partial_static_reference_graph | partial_native |
| TRIGGERS / default | .ref/devices/2684/static/js/8169.3cf63c70.chunk.js@547426 | `()=>(0,A.jsx)(Dl,{})` | 21 | 0 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/2684/static/js/8169.3cf63c70.chunk.js@547481 | `()=>(0,A.jsx)(mc,{})` | 40 | 1 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/2684/static/js/8169.3cf63c70.chunk.js@547536 | `()=>(0,A.jsx)(Ic,{})` | 28 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2684/static/js/8169.3cf63c70.chunk.js@547591 | `()=>(0,A.jsx)(Fn,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 2689 · Razer Hammerhead V3 X HyperSpeed Kuromi Edition

类别：AUDIO_EARBUDS；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [2689]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [2690]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/2689/static/js/288.05254377.chunk.js@333085 | `(0,x.jsx)(qo,{})` | 44 | 20 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/2689/static/js/288.05254377.chunk.js@333130 | `(0,x.jsx)(Vn,{})` | 26 | 14 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/2689/static/js/288.05254377.chunk.js@333175 | `(0,x.jsx)(pn,{})` | 16 | 6 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/2689/static/js/288.05254377.chunk.js@333220 | `(0,x.jsx)(Wn,{})` | 14 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/2689/static/js/288.05254377.chunk.js@333265 | `(0,x.jsx)(wr,{resetObm:this.resetDevice})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 3072 · Razer Firefly Hard Edition

类别：MOUSEMAT；edition：[0, 1]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3072/static/js/main.22fdc9c0.js@4555278 | `(0,YE.jsx)(pO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/3072/static/js/main.22fdc9c0.js@4555333 | `(0,YE.jsx)(lL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 3073 · Razer Goliathus Chroma

类别：MOUSEMAT；edition：[0, 128, 129, 130, 132]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3073/static/js/main.c88f5161.js@4554629 | `(0,ze.jsx)(xd,{})` | 47 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/3073/static/js/main.c88f5161.js@4554676 | `(0,ze.jsx)(Yh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 3074 · Razer Goliathus Extended Chroma

类别：MOUSEMAT；edition：[0, 128, 129, 130, 132, 133]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3074/static/js/main.85c88c49.js@4548568 | `(0,WE.jsx)(eO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/3074/static/js/main.85c88c49.js@4548623 | `(0,WE.jsx)($D,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 3076 · Razer Firefly V2

类别：MOUSEMAT；edition：[0, 128, 129, 130, 132]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3076/static/js/main.5acaa9f5.js@4544684 | `(0,qE.jsx)(nO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/3076/static/js/main.5acaa9f5.js@4544739 | `(0,qE.jsx)(oL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 3077 · Razer Strider Chroma

类别：MOUSEMAT；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3077/static/js/main.fd1964d4.js@4565063 | `(0,qE.jsx)(nO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/3077/static/js/main.fd1964d4.js@4565118 | `(0,qE.jsx)(iL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 3078 · Razer Goliathus Chroma 3XL

类别：MOUSEMAT；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3078/static/js/main.7f259c76.js@4543815 | `(0,YE.jsx)(sO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/3078/static/js/main.7f259c76.js@4543870 | `(0,YE.jsx)(nL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 3080 · Razer Firefly V2 Pro

类别：MOUSEMAT；edition：[0, 128, 129, 130, 131]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3080/static/js/main.961e0950.js@4574735 | `(0,Pa.jsx)(BO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native_reaudited |
| HELP / default | .ref/devices/3080/static/js/main.961e0950.js@4574790 | `(0,Pa.jsx)(vL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native_reaudited |

## 3331 · Razer RipSaw HD

类别：BROADCASTER_GAME_CAPTURE_CARD；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SETTING / default | .ref/devices/3331/static/js/main.37e94222.js@3756835 | `(0,t.jsx)(a.A,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3331/static/js/main.37e94222.js@4240864 | `(0,ho.jsx)(dL,{resetObm:!0})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3334 · Razer Stream Controller

类别：BROADCASTER_STREAM_CONTROLLER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_HOME / default | .ref/devices/3334/static/js/main.496bb585.js@4520699 | `(0,ge.jsx)(wm,{})` | 21 | 0 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/3334/static/js/main.496bb585.js@4520746 | `(0,ge.jsx)(VU,{})` | 32 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3334/static/js/main.496bb585.js@4520793 | `(0,ge.jsx)(Xp,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3337 · Razer Stream Controller X

类别：BROADCASTER_STREAM_CONTROLLER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_HOME / default | .ref/devices/3337/static/js/main.a048e901.js@4520659 | `(0,ge.jsx)(wm,{})` | 21 | 0 | partial_static_reference_graph | partial_native |
| STREAM_MIXER_HEADER / default | .ref/devices/3337/static/js/main.a048e901.js@4520706 | `(0,ge.jsx)(VU,{})` | 32 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3337/static/js/main.a048e901.js@4520753 | `(0,ge.jsx)(Xp,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3587 · Razer Kiyo

类别：BROADCASTER_CAMERA；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3587/static/js/main.459c3161.js@4455598 | `(0,We.jsx)($S,{supportsFocus:!0})` | 19 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3587/static/js/main.459c3161.js@4455661 | `(0,We.jsx)(TU,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3589 · Razer Kiyo Pro

类别：BROADCASTER_CAMERA；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3589/static/js/main.b9251f30.js@4436891 | `(0,We.jsx)($S,{supportsFocus:!0,supportsFieldOfView:!0,supportsHDR:!0})` | 19 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3589/static/js/main.b9251f30.js@4436992 | `(0,We.jsx)(mM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3590 · Razer Kiyo X

类别：BROADCASTER_CAMERA, CAMERA_CAMERA；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3590/static/js/main.28ecadf2.js@4439428 | `(0,We.jsx)($S,{supportsFocus:!0})` | 19 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3590/static/js/main.28ecadf2.js@4439491 | `(0,We.jsx)(mM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3592 · Razer Kiyo Pro Ultra

类别：BROADCASTER_CAMERA；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| CAMERA / default | .ref/devices/3592/static/js/main.323f0a8a.js@4586394 | `(0,Ye.jsx)(eG,{})` | 23 | 0 | partial_static_reference_graph | partial_native |
| PROCESSING / default | .ref/devices/3592/static/js/main.323f0a8a.js@4586441 | `(0,Ye.jsx)(SG,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| IMAGE / default | .ref/devices/3592/static/js/main.323f0a8a.js@4586488 | `(0,Ye.jsx)(DG,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3592/static/js/main.323f0a8a.js@4586535 | `(0,Ye.jsx)(rm,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3594 · Razer Kiyo V2 Pro

类别：BROADCASTER_CAMERA；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| CAMERA / default | .ref/devices/3594/static/js/main.bb246bb0.js@7036548 | `(0,P_.jsx)(xm,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| PROCESSING / default | .ref/devices/3594/static/js/main.bb246bb0.js@7036595 | `(0,P_.jsx)(iU,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| IMAGE / default | .ref/devices/3594/static/js/main.bb246bb0.js@7036642 | `(0,P_.jsx)(SU,{supportSharpness:!1,supportGain:!1,isDisabledWhenAutoFrameOn:!0,supportWatermark:!1,isDisableWhenThirdPartySelected:!0})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3594/static/js/main.bb246bb0.js@7036807 | `(0,P_.jsx)(xP,{resetObm:this.resetDevice,hasCamoStudio:!0,isGuest:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3595 · Razer Kiyo V2

类别：BROADCASTER_CAMERA；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| CAMERA / default | .ref/devices/3595/static/js/main.19c1ceba.js@4617946 | `(0,M_.jsx)(eh,{})` | 27 | 0 | partial_static_reference_graph | partial_native |
| PROCESSING / default | .ref/devices/3595/static/js/main.19c1ceba.js@4617993 | `(0,M_.jsx)(Sh,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| IMAGE / default | .ref/devices/3595/static/js/main.19c1ceba.js@4618040 | `(0,M_.jsx)(Dh,{supportSharpness:!1,supportGain:!1,isDisabledWhenAutoFrameOn:!0,supportWatermark:!1,isDisableWhenThirdPartySelected:!0})` | 15 | 0 | partial_static_reference_graph | partial_native |
| MIC / default | .ref/devices/3595/static/js/main.19c1ceba.js@4618205 | `(0,M_.jsx)(Ph,{})` | 6 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3595/static/js/main.19c1ceba.js@4618252 | `(0,M_.jsx)(oM,{resetObm:this.resetDevice,hasCamoStudio:!0,isGuest:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3596 · Razer Kiyo V2 X

类别：BROADCASTER_CAMERA；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| CAMERA / default | .ref/devices/3596/static/js/main.0e57873f.js@4599337 | `(0,M_.jsx)(UU,{})` | 26 | 0 | partial_static_reference_graph | partial_native |
| PROCESSING / default | .ref/devices/3596/static/js/main.0e57873f.js@4599384 | `(0,M_.jsx)(YU,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| IMAGE / default | .ref/devices/3596/static/js/main.0e57873f.js@4599431 | `(0,M_.jsx)(XU,{supportSharpness:!1,supportGain:!1,isDisabledWhenAutoFrameOn:!0,supportWatermark:!1,isDisableWhenThirdPartySelected:!0})` | 15 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3596/static/js/main.0e57873f.js@4599596 | `(0,M_.jsx)(Wp,{resetObm:this.resetDevice,isGuest:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3848 · RAZER BASE STATION CHROMA

类别：ACCESSORY；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3848/static/js/main.a5ed8890.js@4557461 | `(0,qE.jsx)(IO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3848/static/js/main.a5ed8890.js@4557516 | `(0,qE.jsx)(TL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3849 · Razer Chroma HDK

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3849/static/js/main.bb884c00.js@4560206 | `(0,YE.jsx)(TO.default,{})` | 38 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3849/static/js/main.bb884c00.js@4560261 | `(0,YE.jsx)(rL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3853 · Razer Laptop Stand Chroma

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3853/static/js/main.8f044274.js@4541373 | `(0,qE.jsx)(nO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3853/static/js/main.8f044274.js@4541428 | `(0,qE.jsx)(oL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3858 · RAZER RAPTOR 27

类别：MONITOR；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'monitorPortIds', 'ids': [5122]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_GAMING / default | .ref/devices/3858/static/js/main.608e1599.js@4573695 | `(0,t6O.jsx)(NTA,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_COLOR / default | .ref/devices/3858/static/js/main.608e1599.js@4573745 | `(0,t6O.jsx)(xAA,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/3858/static/js/main.608e1599.js@4573795 | `(0,t6O.jsx)(cSA,{})` | 29 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3858/static/js/main.608e1599.js@4573845 | `(0,t6O.jsx)(WAA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3858/static/js/main.608e1599.js@4573895 | `(0,t6O.jsx)(opA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3859 · Lian Li 011 Dynamic

类别：CASE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3859/static/js/main.ffac912b.js@4567918 | `(0,WE.jsx)(eO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3859/static/js/main.ffac912b.js@4567973 | `(0,WE.jsx)($D,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3863 · Razer Tomahawk ATX

类别：CASE；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3863/static/js/main.0ae565e3.js@4552703 | `(0,Ea.jsx)(sO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3863/static/js/main.0ae565e3.js@4552758 | `(0,Ea.jsx)(oL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3866 · Razer Core X Chroma

类别：EGPU；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3866/static/js/main.28d3f4af.js@4551426 | `(0,we.jsx)(qd,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3866/static/js/main.28d3f4af.js@4551473 | `(0,we.jsx)(zh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3867 · Razer Seiren Emote

类别：BROADCASTER_MICROPHONE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/3867/static/js/main.7afd910b.js@4430716 | `(0,be.jsx)(Qc,{})` | 29 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3867/static/js/main.7afd910b.js@4430763 | `(0,be.jsx)(tL,{})` | 5 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3867/static/js/main.7afd910b.js@4430810 | `(0,be.jsx)(uM,{resetObm:this.props.obmResetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3869 · Razer Bungee V3 Chroma

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3869/static/js/main.82c8869b.js@4552810 | `(0,ve.jsx)($d,{})` | 38 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3869/static/js/main.82c8869b.js@4552857 | `(0,ve.jsx)(Xh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3871 · Razer Chroma Addressable RGB Controller

类别：ACCESSORY_ARGB_CONTROLLER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3871/static/js/main.7e64d4d0.js@4614642 | `(0,fI.jsx)(ol,{})` | 35 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3871/static/js/main.7e64d4d0.js@4614689 | `(0,fI.jsx)(Zd,{})` | 37 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3871/static/js/main.7e64d4d0.js@4614776 | `(0,fI.jsx)(rU,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3872 · Razer Base Station V2 Chroma

类别：ACCESSORY；edition：[0, 128, 129]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_AUDIO / default | .ref/devices/3872/static/js/main.e84a97bd.js@4580417 | `(0,QE.jsx)(_u,{})` | 35 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3872/static/js/main.e84a97bd.js@4580464 | `(0,QE.jsx)(ou.default,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3872/static/js/main.e84a97bd.js@4580519 | `(0,QE.jsx)(fL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3873 · Razer Thunderbolt 4 Dock Chroma

类别：ACCESSORY；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_AUDIO / default | .ref/devices/3873/static/js/main.08e5eb5a.js@4554263 | `(0,QE.jsx)(qc,{})` | 32 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3873/static/js/main.08e5eb5a.js@4554310 | `(0,QE.jsx)(Qc.default,{})` | 32 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3873/static/js/main.08e5eb5a.js@4554365 | `(0,QE.jsx)(UL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3878 · Razer Chroma Charging Pad 10W Fast Wireless Charger

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3878/static/js/main.1c5334c6.js@4537257 | `(0,ka.jsx)(CL.default,{})` | 52 | 1 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3878/static/js/main.1c5334c6.js@4537312 | `(0,ka.jsx)(cC,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3879 · Tomahawk Gaming Desktop

类别：CASE；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3879/static/js/main.2f2244ff.js@4572502 | `(0,ve.jsx)(Yd,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3879/static/js/main.2f2244ff.js@4572549 | `(0,ve.jsx)(vh,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3880 · RAZER RAPTOR 27 (165Hz)

类别：MONITOR；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'monitorPortIds', 'ids': [5125, 5126, 5127]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_GAMING / default | .ref/devices/3880/static/js/main.0ff487d9.js@4585223 | `(0,U6O.jsx)(FTA,{})` | 15 | 0 | partial_static_reference_graph | partial_native |
| TAB_COLOR / default | .ref/devices/3880/static/js/main.0ff487d9.js@4585273 | `(0,U6O.jsx)(NSA,{})` | 23 | 0 | partial_static_reference_graph | partial_native |
| TAB_DISPLAY / default | .ref/devices/3880/static/js/main.0ff487d9.js@4585323 | `(0,U6O.jsx)(qSA,{})` | 31 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3880/static/js/main.0ff487d9.js@4585373 | `(0,U6O.jsx)(iSA,{})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3880/static/js/main.0ff487d9.js@4585423 | `(0,U6O.jsx)(ypA,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3883 · Razer Laptop Stand V2 Chroma

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3883/static/js/main.85b90298.js@4541000 | `(0,QE.jsx)(sO.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3883/static/js/main.85b90298.js@4541055 | `(0,QE.jsx)(nL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3884 · Razer Chroma Wireless ARGB Controller

类别：ACCESSORY_ARGB_CONTROLLER；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [3884]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [3885]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3884/static/js/main.607cfa0c.js@4531900 | `(0,_T.jsx)(xU,{})` | 39 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3884/static/js/main.607cfa0c.js@4531947 | `(0,_T.jsx)(Vl,{})` | 37 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3884/static/js/main.607cfa0c.js@4532034 | `(0,_T.jsx)(mp,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3886 · Razer Chroma Wireless ARGB Controller

类别：productCategoryIcon；edition：[]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / chromaApp | .ref/devices/3886/static/js/main.44393ea2.js@4017814 | `(0,Do.jsx)(u3,{})` | 38 | 0 | partial_static_reference_graph | independent_not_primary |
| TAB_LIGHTING / chromaApp | .ref/devices/3886/static/js/main.44393ea2.js@4017861 | `(0,Do.jsx)(c2,{})` | 34 | 6 | partial_static_reference_graph | independent_not_primary |
| TAB_CUSTOMIZE / default | .ref/devices/3886/static/js/main.44393ea2.js@4178500 | `(0,Do.jsx)(u3,{})` | 38 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3886/static/js/main.44393ea2.js@4178547 | `(0,Do.jsx)(c2,{})` | 34 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3886/static/js/main.44393ea2.js@4178634 | `(0,Do.jsx)(A8,{})` | 13 | 0 | partial_static_reference_graph | partial_native |

## 3893 · Razer Hanbo AIO 240MM ARGB

类别：ACCESSORY_LIQUID_CONTROLLER；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_PERFORMANCE / default | .ref/devices/3893/static/js/main.a7630c14.js@4602155 | `(0,b.jsx)(xg,{})` | 32 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3893/static/js/main.a7630c14.js@4602204 | `(0,b.jsx)(Er,{})` | 36 | 6 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3893/static/js/main.a7630c14.js@4602253 | `(0,b.jsx)(KG,{resetObm:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3894 · Razer Head Cushion Chroma

类别：ACCESSORY；edition：[0, 255]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [3894]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3894/static/js/main.be5b5e43.js@4538170 | `(0,qE.jsx)(qI.default,{})` | 47 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3894/static/js/main.be5b5e43.js@4538225 | `(0,qE.jsx)(ZD,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3900 · RAZER PWM PC FAN CONTROLLER

类别：ACCESSORY_FAN_CONTROLLER；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_PERFORMANCE / default | .ref/devices/3900/static/js/main.1f8b3065.js@4468409 | `(0,He.jsx)(VP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 28 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3900/static/js/main.1f8b3065.js@4468506 | `(0,He.jsx)(oU,{resetObm:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3907 · Razer Laptop Cooling Pad

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_PERFORMANCE / default | .ref/devices/3907/static/js/main.49bf0db6.js@4852920 | `(0,B.jsx)(gm,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 96 | 4 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3907/static/js/main.49bf0db6.js@4853019 | `(0,B.jsx)(F_,{})` | 36 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3907/static/js/main.49bf0db6.js@4853068 | `(0,B.jsx)(mL,{resetObm:!1})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3909 · Razer Freyja

类别：ACCESSORY；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [3910]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3909/static/js/main.5ea04b2c.js@4562003 | `(0,ke.jsx)(Ef,{})` | 50 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3909/static/js/main.5ea04b2c.js@4562050 | `(0,ke.jsx)(_m,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3921 · Razer Core X V2

类别：EGPU；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3921/static/js/430.ffb57133.chunk.js@352426 | `(0,f.jsx)(Cn.A,{})` | 32 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3921/static/js/430.ffb57133.chunk.js@352473 | `(0,f.jsx)(Pi.A,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3921/static/js/430.ffb57133.chunk.js@352520 | `(0,f.jsx)(Ga,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3922 · Razer Thunderbolt 5 Dock Chroma

类别：ACCESSORY；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/3922/static/js/2608.894c2893.chunk.js@408881 | `(0,E.jsx)(Zo,{})` | 32 | 5 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3922/static/js/2608.894c2893.chunk.js@408926 | `(0,E.jsx)(zo.A,{})` | 33 | 17 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3922/static/js/2608.894c2893.chunk.js@408973 | `(0,E.jsx)(Zn,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 3929 · Razer Monitor Stand Chroma

类别：ACCESSORY；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3929/static/js/main.2aea7d59.js@4744162 | `(0,Wa.jsx)(IA,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 86 | 8 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3929/static/js/main.2aea7d59.js@4744259 | `(0,Wa.jsx)(eP,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3932 · Handheld Gaming Dock

类别：ACCESSORY；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3932/static/js/main.a61a864b.js@4725185 | `(0,Pa.jsx)(BO,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 86 | 8 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3932/static/js/main.a61a864b.js@4725282 | `(0,Pa.jsx)(PL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3940 · Razer Soma Chroma

类别：ACCESSORY_CHAIR；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_LIGHTING / default | .ref/devices/3940/static/js/9775.989c29ca.chunk.js@427004 | `(0,S.jsx)(Di.A,{})` | 45 | 19 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/3940/static/js/9775.989c29ca.chunk.js@427051 | `(0,S.jsx)(Ji,{})` | 9 | 2 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3940/static/js/9775.989c29ca.chunk.js@427096 | `(0,S.jsx)(eo,{resetObm:this.resetDevice})` | 25 | 5 | partial_static_reference_graph | partial_native |

## 3942 · Razer Marci

类别：ACCESSORY_CHAIR, CHAIR_CHAIR；edition：[0, 128]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'wiredId', 'ids': [3942]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/3942/static/js/70.f296e7d2.chunk.js@536972 | `(0,x.jsx)($r,{})` | 47 | 19 | partial_static_reference_graph | partial_native |
| SURROUND / default | .ref/devices/3942/static/js/70.f296e7d2.chunk.js@537017 | `(0,x.jsx)(Gl,{})` | 36 | 25 | partial_static_reference_graph | partial_native |
| TAB_HAPTICS / default | .ref/devices/3942/static/js/70.f296e7d2.chunk.js@537062 | `(0,x.jsx)(In,{})` | 54 | 21 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3942/static/js/70.f296e7d2.chunk.js@537107 | `(0,x.jsx)(Rn.A,{})` | 37 | 22 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/3942/static/js/70.f296e7d2.chunk.js@537154 | `(0,x.jsx)(cc,{})` | 13 | 5 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/3942/static/js/70.f296e7d2.chunk.js@537199 | `(0,x.jsx)(kn,{})` | 33 | 27 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3942/static/js/70.f296e7d2.chunk.js@537244 | `(0,x.jsx)(Aa,{resetObm:this.resetDevice})` | 28 | 14 | partial_static_reference_graph | partial_native |

## 3946 · August T2

类别：ACCESSORY；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/3946/static/js/main.46ad416c.js@6990848 | `(0,zi.jsx)(WH,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 156 | 10 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/3946/static/js/main.46ad416c.js@6990945 | `(0,zi.jsx)(sd,{changeView:this.changeView})` | 34 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3946/static/js/main.46ad416c.js@6991018 | `(0,zi.jsx)(PL,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 3949 · MarciT2H

类别：ACCESSORY_CHAIR；edition：[0]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_HAPTICS / default | .ref/devices/3949/static/js/main.5f0af02d.js@4718455 | `(0,$e.jsx)(TH,{})` | 50 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/3949/static/js/main.5f0af02d.js@4718502 | `(0,$e.jsx)(hH,{})` | 10 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/3949/static/js/main.5f0af02d.js@4718549 | `(0,$e.jsx)(bM,{resetObm:this.resetDevice})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 4115 · Razer Kitsune

类别：GAMEPAD, GAMEPAD_ARCADE_CONTROLLER；edition：[0, 128, 129, 130, 131, 132, 133]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'psModeIds', 'ids': [4114]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/4115/static/js/main.1a52fc92.js@6717445 | `()=>(0,wn.jsx)(MP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 23 | 0 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/4115/static/js/main.1a52fc92.js@6717552 | `()=>(0,wn.jsx)(cm,{})` | 37 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/4115/static/js/main.1a52fc92.js@6717609 | `()=>(0,wn.jsx)(BN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 4124 · Razer BlackShark V3 Pro PS

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [4123]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [4125]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/4124/static/js/main.7ea8f906.js@5671888 | `(0,uI.jsx)(Rm,{})` | 69 | 50 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/4124/static/js/main.7ea8f906.js@5671935 | `(0,uI.jsx)(Wb,{})` | 19 | 15 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/4124/static/js/main.7ea8f906.js@5671982 | `(0,uI.jsx)(Lh,{})` | 19 | 13 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/4124/static/js/main.7ea8f906.js@5672029 | `(0,uI.jsx)(GM,{})` | 36 | 27 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/4124/static/js/main.7ea8f906.js@5672076 | `(0,uI.jsx)(wF,{})` | 21 | 7 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/4124/static/js/main.7ea8f906.js@5672123 | `(0,uI.jsx)(GH,{})` | 36 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/4124/static/js/main.7ea8f906.js@5672170 | `(0,uI.jsx)(Gf,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:il.MVq},resetTitle:il.mY5,hasTutorial:!0,hasTHXPartialAudio:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 4126 · Razer BlackShark V3 PS

类别：AUDIO；edition：[0, 128]；连接别名：[]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/4126/static/js/main.61243a04.js@5669254 | `(0,cI.jsx)(um,{})` | 69 | 50 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/4126/static/js/main.61243a04.js@5669301 | `(0,cI.jsx)(Lb,{})` | 19 | 15 | partial_static_reference_graph | partial_native |
| TAB_ENHANCEMENT / default | .ref/devices/4126/static/js/main.61243a04.js@5669348 | `(0,cI.jsx)(sh,{})` | 15 | 12 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/4126/static/js/main.61243a04.js@5669395 | `(0,cI.jsx)(cM,{})` | 36 | 27 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/4126/static/js/main.61243a04.js@5669442 | `(0,cI.jsx)(mF,{})` | 21 | 7 | partial_static_reference_graph | partial_native |
| TAB_DEMO / default | .ref/devices/4126/static/js/main.61243a04.js@5669489 | `(0,cI.jsx)(cH,{})` | 36 | 23 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/4126/static/js/main.61243a04.js@5669536 | `(0,cI.jsx)(cf,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:al.MVq},resetTitle:al.mY5,hasTutorial:!0,hasTHXPartialAudio:!0})` | 20 | 20 | partial_static_reference_graph | partial_native |

## 4130 · Razer BlackShark V3 X Hyperspeed

类别：AUDIO；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [4129]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [4131]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_SOUND / default | .ref/devices/4130/static/js/main.6a9cd75a.js@4784194 | `(0,ae.jsx)(rL,{})` | 41 | 17 | partial_static_reference_graph | partial_native |
| TAB_MIC / default | .ref/devices/4130/static/js/main.6a9cd75a.js@4784241 | `(0,ae.jsx)(tP,{})` | 26 | 14 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/4130/static/js/main.6a9cd75a.js@4784288 | `(0,ae.jsx)(uP,{})` | 10 | 4 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/4130/static/js/main.6a9cd75a.js@4784335 | `(0,ae.jsx)(NM,{firmwareResetProps:{shouldPowerOff:!0,modalMsg:Te.kTe},resetTitle:(0,B.JN)(Te.ARF)})` | 23 | 13 | partial_static_reference_graph | partial_native |

## 4133 · Razer Raiju V3 Pro

类别：GAMEPAD；edition：[0, 128, 129]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [4135]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [4136]}, {'source': 'AvailableDevices.json', 'kind': 'psModeIds', 'ids': [4132, 4134]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/4133/static/js/main.d34fa647.js@6794796 | `()=>(0,yn.jsx)(ZP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 75 | 4 | partial_static_reference_graph | partial_native |
| TRIGGERS / default | .ref/devices/4133/static/js/main.d34fa647.js@6794903 | `()=>(0,yn.jsx)(th,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/4133/static/js/main.d34fa647.js@6794960 | `()=>(0,yn.jsx)(eM,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/4133/static/js/main.d34fa647.js@6795017 | `()=>(0,yn.jsx)(lm,{})` | 9 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/4133/static/js/main.d34fa647.js@6795074 | `()=>(0,yn.jsx)(sM,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/4133/static/js/main.d34fa647.js@6795131 | `()=>(0,yn.jsx)(GN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 4144 · Razer Raiju V3 Pro Signature Edition

类别：GAMEPAD；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [4146]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [4147]}, {'source': 'AvailableDevices.json', 'kind': 'psModeIds', 'ids': [4143, 4145]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/4144/static/js/main.a5c50629.js@6754138 | `()=>(0,gn.jsx)(bP,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 75 | 4 | partial_static_reference_graph | partial_native |
| TRIGGERS / default | .ref/devices/4144/static/js/main.a5c50629.js@6754245 | `()=>(0,gn.jsx)(xP,{})` | 20 | 0 | partial_static_reference_graph | partial_native |
| THUMBSTICKS / default | .ref/devices/4144/static/js/main.a5c50629.js@6754302 | `()=>(0,gn.jsx)(zh,{})` | 24 | 0 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/4144/static/js/main.a5c50629.js@6754359 | `()=>(0,gn.jsx)(nm,{})` | 9 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/4144/static/js/main.a5c50629.js@6754416 | `()=>(0,gn.jsx)(Jh,{})` | 11 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/4144/static/js/main.a5c50629.js@6754473 | `()=>(0,gn.jsx)(DN,{})` | 26 | 0 | partial_static_reference_graph | partial_native |

## 45066 · Razer Basilisk V3 Pro 35K - XBOX 25 Anniversary Edition

类别：MOUSE；edition：[0]；连接别名：[{'source': 'AvailableDevices.json', 'kind': 'dongleId', 'ids': [45067]}, {'source': 'AvailableDevices.json', 'kind': 'bleId', 'ids': [45068]}]。

| 页面 / 模式 | 当前导航源及偏移 | 根表达式 | 组件 | 未解 | 源子图 | 本地路由状态 |
| --- | --- | --- | ---: | ---: | --- | --- |
| TAB_CUSTOMIZE / default | .ref/devices/45066/static/js/286.1978470e.chunk.js@249605 | `(0,D.jsx)(qe,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})` | 2 | 1 | partial_static_reference_graph | partial_native |
| TAB_PERFORMANCE / default | .ref/devices/45066/static/js/286.1978470e.chunk.js@249700 | `(0,D.jsx)($s,{isBle:this.props.isBle})` | 35 | 1 | partial_static_reference_graph | partial_native |
| TAB_LIGHTING / default | .ref/devices/45066/static/js/286.1978470e.chunk.js@249767 | `(0,D.jsx)(Ys.A,{})` | 38 | 4 | partial_static_reference_graph | partial_native |
| TAB_POWER / default | .ref/devices/45066/static/js/286.1978470e.chunk.js@249814 | `(0,D.jsx)(ni,{})` | 14 | 0 | partial_static_reference_graph | partial_native |
| TAB_CALIBRATION / default | .ref/devices/45066/static/js/286.1978470e.chunk.js@249859 | `(0,D.jsx)(gi,{})` | 17 | 0 | partial_static_reference_graph | partial_native |
| HELP / default | .ref/devices/45066/static/js/286.1978470e.chunk.js@249904 | `(0,D.jsx)(da,{resetObm:this.props.obmResetDevice})` | 25 | 0 | partial_static_reference_graph | partial_native |

## 复生成与验证

`node tools/extract-all-product-page-chains.cjs`解析全部产品；`node tools/extract-all-product-layout-chains.cjs`连接CSS；`python -X utf8 tools/report-full-ui-chains.py --check`检查报告一致性；`node tools/validate-full-ui-chains.cjs`核对完整压缩记录、工具指纹和当前源原文。

未运行厂商JS、应用、DLL、构建或测试。界面运行、视觉、真实服务读取均未验收；DLL写回仍后置。
