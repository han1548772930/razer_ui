# PID 515 — TAB_CUSTOMIZE — Snap Tap

本文件从当前官方原文件静态提取；未执行官方 JS。仅覆盖此界面的 Snap Tap 区域，不代表整个 Customize 页完成。

分支：`ordinary`；列：`left`；快捷键：`False`。偏移单位为 Unicode 字符，另存 UTF-16 偏移。

## 交互与缺口

普通分支：默认 A/D、标题开关、最多四对、首对不可删除、按键释放录入、重复/禁用键警告、成功消息三秒、窗口失焦及离页清理、调整模式警告。原服务输入捕获/映射恢复、真实配置写回和观察尚未连接，不能将本地草稿视作设备保存。

以下 JavaScript 含原 JSX 编译后的 HTML 元素结构和事件处理；CSS 为原规则完整文本。共享依赖、native 调用与源码未提取分支仍是明确缺口。

## 实际挂载与条件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/js/main.f60ca5aa.js` SHA-256 `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8`

字符 [7347842, 7348592)；UTF-16 [7347842, 7348592)。

```javascript
a.jsx)("div",{className:"text hypershift",children:(0,za.jsx)(ln.A,{text:ye.LUv})})]}),(0,za.jsx)("div",{children:(0,za.jsxs)("div",{style:{position:"relative"},children:[(0,za.jsx)("div",{className:"help"}),(0,za.jsx)("div",{style:{width:362},className:"tip hypershift-mode-tip",children:(0,za.jsx)(ln.A,{text:ye.Dq6})})]})})]}),"macro"!==this.props.displayMode?(0,za.jsxs)(fr,{children:[(0,za.jsxs)(vr,{direction:"left",children:[(0,za.jsx)(DA,{}),(0,za.jsx)(Jl,{})]}),(0,za.jsx)(vr,{direction:"right",children:(0,za.jsx)(mA,{})})]}):null]})}}const eO=(0,g.connect)(e=>({lang:e.languageReducer.lang,buttonList:e.customizeReducer.buttonList,activeButton:e.customizeReducer.activeButton,isPanelOpen:e.customizeReducer.isPanelOpen,isMappingOpen:e.cust
```

## 标题、开关、帮助与快捷键

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/js/main.f60ca5aa.js` SHA-256 `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8`

字符 [7342208, 7343191)；UTF-16 [7342208, 7343191)。

```javascript
Jl=e=>{let t=e.supportsShortcut;const a=(0,g.useDispatch)(),n=(0,g.useSelector)(e=>e.snapTapReducer.keyList),o=(0,g.useSelector)(e=>e.snapTapReducer.isEnabled),i=(0,g.useSelector)(e=>e.customizeReducer.adjustmentModeRunning),E=(0,g.useSelector)(e=>e.customizeReducer.buttonList),s=(0,m.useMemo)(()=>{if(!E||!E.length)return!1;const e=["DKM_F6","DKM_D2"];return E.some(t=>e.includes(t.inputID))},[E]);return(0,za.jsxs)(br,{title:ye.Fqe,tips:s?ye.qI5:ye.f_m,hasSwitch:!0,toggleSwitch:()=>{i?a((0,zi.onShowAdjustmentModePrompt)(!0)):(o&&qc(),"SYSTEM"===ie.DeviceInfo.category&&(0,we.JB)({enabled:!o,snapTapPairs:o?[]:n,label:Kl.xt.ENABLE}),a((0,Jc.vU)(!o)))},active:o,supportsShortcut:t,renderShortcut:(0,za.jsx)(Ql,{keyCombinations:["FN","L SHIFT"],tip:ye.cyD}),extraClass:"snap-tap-widget",children:[(0,za.jsx)("p",{className:"snap-tap-desc",children:(0,za.jsx)(ln.A,{text:ye.PVl})}),(0,za.jsx)("section",{className:"snap-tap ".concat(o?"":"disabled"),children:(0,za.jsx)(ql,{})})]})}
```

## 完整录入编辑组件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/js/main.f60ca5aa.js` SHA-256 `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8`

字符 [7335988, 7341740)；UTF-16 [7335988, 7341740)。

```javascript
ql=()=>{const e=(0,g.useDispatch)(),t=(0,g.useSelector)(e=>e.snapTapReducer),a=t.isEnabled,n=t.keyList,o=(0,g.useSelector)(e=>{var t;return null!==(t=e.keymapReducer)&&void 0!==t?t:[]}).directionMode,i=(0,g.useSelector)(e=>e.profileReducer.selectedProfileGuid),E=(0,g.useSelector)(e=>e.deviceReducer),s=E.isBle,r=E.isDongle,_=(0,m.useState)(!1),T=(0,Wa.A)(_,2),I=T[0],c=T[1],A=(0,m.useState)(EA.READY),l=(0,Wa.A)(A,2),O=l[0],S=l[1],d=(0,m.useState)(bl.INTRODUCTION),u=(0,Wa.A)(d,2),R=u[0],N=u[1],D=(0,m.useState)(-1),C=(0,Wa.A)(D,2),L=C[0],p=C[1],P=(0,m.useState)(n),h=(0,Wa.A)(P,2),U=h[0],H=h[1],G=(0,m.useRef)(null),f=(0,m.useRef)(!1),y=(0,m.useRef)(null),v=(0,m.useRef)(null),F=(0,m.useRef)(null);(0,m.useEffect)(()=>{f.current=!1,N(bl.INTRODUCTION),p(-1),S(EA.READY)},[i]),(0,m.useEffect)(()=>{H(n),B(n)},[JSON.stringify(n)]),(0,m.useEffect)(()=>{var t,a;const o=null!==(t=null===n||void 0===n?void 0:n.filter(e=>{let t=e.key1,a=e.key2;return t&&a}))&&void 0!==t?t:[];o.length!==(null!==(a=null===n||void 0===n?void 0:n.length)&&void 0!==a?a:0)&&e((0,Jc.gr)(o))},[]);const B=e=>{"SYSTEM"===ie.DeviceInfo.category&&(0,we.JB)({enabled:a,snapTapPairs:e,label:Kl.xt.CHANGE})},b=()=>{document.removeEventListener("mousedown",Y),window.removeEventListener("keyup",z,!0),window.removeEventListener("keydown",k,!0),window.removeEventListener("blur",w),zt.A.off("inputredirect",W)};(0,m.useEffect)(()=>(a?(window.addEventListener("blur",w),window.addEventListener("keyup",z,!0),window.addEventListener("keydown",k,!0),document.addEventListener("mousedown",Y),zt.A.on("inputredirect",W)):V(),()=>{b()}),[a,O,JSON.stringify(U)]);const V=()=>{b(),R!==bl.EMPTY_KEY_WARNING&&R!==bl.DUPLICATE_KEY_WARNING||q(L)},Y=t=>{if(O!==EA.READY&&v.current instanceof Node&&t.target instanceof Node&&v.current&&!v.current.contains(t.target)&&F.current instanceof Node&&F.current&&!F.current.contains(t.target)){if(U.some(e=>!e.key1||!e.key2))return void N(bl.DUPLICATE_KEY_WARNING);if(wl.includes(R))return;e((0,Jc.gr)(U)),X()}};(0,m.useEffect)(()=>{const t=()=>{let t=[...n];if(O===EA.KEY2){const e=U.find(e=>e.id===L);e&&(t=t.map(t=>t.id===L?(0,M.A)((0,M.A)({},t),{},{key1:e.key1}):t))}const a=t.filter(e=>e.key1&&e.key2);e((0,Jc.gr)(a)),X()};return window.addEventListener("beforeunload",t),()=>{window.removeEventListener("beforeunload",t)}},[JSON.stringify(U),R]);const K=(0,m.useMemo)(()=>({onInputRedirectEvent:e=>{if(O===EA.READY)return;const t=JSON.parse(e.input);if(!kl.includes(t.type))return;let a=Is.Rk.find(e=>{const a="number"===typeof e.outputFlag?e.outputFlag:e.flag;return t.scancode===e.scancode&&(a===t.flag||a+1===t.flag)});a||"razerKey"!==t.type||(a=(null!==Wl&&void 0!==Wl?Wl:wc).find(e=>e.key===t.key.toString())),a&&(e=>e.flag%2===1)(t)&&Z(a)},onWindowBlur:()=>{let t=U.filter(e=>{let t=e.id,a=e.key1,n=e.key2;return!(wl.includes(R)&&t===L)&&a&&n});if(1===U.length&&0===t.length)return H(n),B(n),N(bl.INTRODUCTION),void X();e((0,Jc.gr)(t)),N(bl.INTRODUCTION),X()}}),[O,JSON.stringify(U),L]),W=K.onInputRedirectEvent,w=K.onWindowBlur,z=e=>{if(e.preventDefault(),!0===e.metaKey||"Meta"===e.key)return;const t=jc(e);t&&Z(t)},k=e=>{!0!==e.metaKey&&"Meta"!==e.key||e.stopImmediatePropagation()},x=()=>{f.current=!1,clearTimeout(G.current),zt.A.disableMapping(),Fa.A.callElectronAction({action:"registerNoBrowserInputHandler"}),zt.A.on("inputredirect",W),Zc(s,r)},X=()=>{f.current=!1,S(EA.READY),Fa.A.callElectronAction({action:"unRegisterNoBrowserInputHandler"}),zt.A.enableMapping(),qc(s,r)},j=(0,m.useRef)(X);j.current=X,(0,m.useEffect)(()=>()=>{j.current()},[]);const Z=t=>{const a=t.inputID;if(((e,t)=>e&&"KEY_NUMPAD_NUM_LOCK"===t)(f.current,a))f.current=!1;else if(f.current="KEY_PAUSE"===a,!Xl.includes(t.inputID)||"KEYPAD"!==ie.DeviceInfo.category||o!==ce.mZI.DIRECTIONAL)switch(O){case EA.KEY1:let t=structuredClone(U);if(t=U.map(e=>{if(e.id===L){return(0,M.A)((0,M.A)({},e),{},{key1:a})}return e}),N(bl.INTRODUCTION),((e,t)=>{const a=t.reduce((t,a)=>t+Object.values(a).filter(t=>t===e).length,0);return a>1})(a,t))return void N(bl.DUPLICATE_KEY_WARNING);if(jl(a,t))return void N(bl.DUPLICATE_KEY_WARNING);H(t),B(t),S(EA.KEY2);break;case EA.KEY2:let n=structuredClone(U);if(n=U.map(e=>{if(e.id===L){return(0,M.A)((0,M.A)({},e),{},{key2:a})}return e}),jl(a,n))return void N(bl.DUPLICATE_KEY_WARNING);H(n),B(n),e((0,Jc.gr)(n)),X(),N(bl.SUCCESS),G.current=setTimeout(()=>{N(bl.INTRODUCTION)},3e3)}},q=t=>{t===L?(p(-1),N(bl.INTRODUCTION),X()):t<L&&p(e=>e-1);let a=U.filter(e=>e.id!==t).map((e,t)=>(0,M.A)((0,M.A)({},e),{},{id:t+1})),o=n.find(e=>e.id===t);U.some(e=>!e.key1||!e.key2)||o.key1===o.key2||R!==bl.EMPTY_KEY_WARNING&&R!==bl.DUPLICATE_KEY_WARNING||(a=n,H(n),B(n)),e((0,Jc.gr)(a))},Q=(e,t)=>{O===EA.READY&&(S(t),p(e),x())};return(0,za.jsxs)("div",{children:[(0,za.jsxs)("div",{className:"snap-tap-wrapper",children:[(0,za.jsx)("div",{className:"snap-tap-key-list",ref:v,children:(0,za.jsx)("div",{className:"snap-tap-group",children:null===U||void 0===U?void 0:U.map(e=>(0,za.jsx)(Vl,{keyGroup:e,recordingState:O,onDelete:()=>q(e.id),onEdit:Q,isEditing:e.id===L,isDuplicateKeyWarning:R===bl.DUPLICATE_KEY_WARNING&&e.id===L,messageStatus:R},e.id))})}),(0,za.jsxs)("div",{className:"snap-tap-add-button ".concat(U.length>=4||O!==EA.READY?"disabled":""),onClick:()=>{const t=[...n,{key1:"",key2:"",id:n.length+1}];S(EA.KEY1),p(t.length),e((0,Jc.xg)(t)),x()},ref:F,children:[(0,za.jsx)("div",{className:I?"showTooltip":"hideTooltip",ref:y,children:(0,za.jsx)("p",{children:(0,za.jsx)(ln.A,{text:ye.Yjp})})}),(0,za.jsx)("div",{className:"snap-tap-add-button-overlay",onMouseEnter:()=>c(!0),onMouseLeave:()=>c(!1),onMouseMove:e=>{y.current.style.top=e.clientY+20+"px",y.current.style.left=e.clientX+10+"px"}}),"+"]})]}),(0,za.jsx)(Zl,{status:R})]})}
```

## 按键对行

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/js/main.f60ca5aa.js` SHA-256 `4a9db2072b3d64035e36d468fcc006b1930ebbf8c0bc620e4869a2e970c432e8`

字符 [7334400, 7335163)；UTF-16 [7334400, 7335163)。

```javascript
Vl=e=>{let t=e.onDelete,a=e.keyGroup,n=e.recordingState,o=e.isDuplicateKeyWarning,i=e.onEdit,E=e.messageStatus,s=e.isEditing;const r=(0,g.useSelector)(e=>e.deviceReducer.layoutId);return(0,za.jsxs)("div",{className:"snap-tap-key-list-wrapper",children:[(0,za.jsx)(tA,{isRecording:s&&n===EA.KEY1,idleState:"READY",name:zc(a.key1)?kc(a.key1):(0,oA.A)(r,a.key1,"KEYBOARD"),isWarning:s&&(n===EA.KEY1&&o||n===EA.KEY1&&E===bl.EMPTY_KEY_WARNING),onEdit:()=>i(a.id,EA.KEY1)}),(0,za.jsx)(tA,{isRecording:s&&n===EA.KEY2,idleState:"READY",name:zc(a.key2)?kc(a.key2):(0,oA.A)(r,a.key2,"KEYBOARD"),isWarning:s&&(n===EA.KEY2&&o||n===EA.KEY2&&E===bl.EMPTY_KEY_WARNING),onEdit:()=>i(a.id,EA.KEY2)}),1!==a.id?(0,za.jsx)("div",{className:"snap-tap-icon-delete",onClick:t}):null]})}
```

## CSS

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [17636, 17741)；UTF-16 [17636, 17741)。

```css
.key-snap-tap{background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg);height:16px;width:16px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [17741, 17824)；UTF-16 [17741, 17824)。

```css
.key-dks,.key-snap-tap{background-size:cover;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [75532, 75822)；UTF-16 [75532, 75822)。

```css
.factory-default .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [75822, 75891)；UTF-16 [75822, 75891)。

```css
.factory-default .warning-alert .title{color:#ccc;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [75891, 76180)；UTF-16 [75891, 76180)。

```css
.mode-switcher .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [76180, 76263)；UTF-16 [76180, 76263)。

```css
.mode-switcher .warning-alert .title{color:#fd8611;display:flex;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [76263, 76322)；UTF-16 [76263, 76322)。

```css
.mode-switcher .warning-alert .title span{margin-right:4px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [76322, 76382)；UTF-16 [76322, 76382)。

```css
.mode-switcher .warning-alert .content{white-space:pre-wrap}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [84029, 84092)；UTF-16 [84029, 84092)。

```css
.disabled .check-box:hover{border-color:#737373;cursor:default}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [114861, 114902)；UTF-16 [114861, 114902)。

```css
.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [120300, 120544)；UTF-16 [120300, 120544)。

```css
.remove-alert,.warning-alert{background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [120544, 120771)；UTF-16 [120544, 120771)。

```css
.remove-alert .title,.warning-alert .title{align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [120771, 120981)；UTF-16 [120771, 120981)。

```css
.remove-alert .title .icon,.warning-alert .title .icon{background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [120981, 121104)；UTF-16 [120981, 121104)。

```css
.remove-alert .body,.warning-alert .body{color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [121104, 121194)；UTF-16 [121104, 121194)。

```css
.remove-alert .action,.warning-alert .action{display:flex;gap:20px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [121194, 121571)；UTF-16 [121194, 121571)。

```css
.remove-alert .action .ok-button,.remove-alert .action .secondary-button,.warning-alert .action .ok-button,.warning-alert .action .secondary-button{align-items:center;background-color:#707070;border-radius:3px;color:#ccc;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [121571, 121878)；UTF-16 [121571, 121878)。

```css
.remove-alert .action .enable-button,.warning-alert .action .enable-button{align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364181, 364233)；UTF-16 [364181, 364233)。

```css
.combined-key-blink-active{padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364233, 364285)；UTF-16 [364233, 364285)。

```css
.body-widgets .widget>div.snap-tap{will-change:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364285, 364311)；UTF-16 [364285, 364311)。

```css
.snap-tap{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364311, 364362)；UTF-16 [364311, 364362)。

```css
.snap-tap .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364362, 364422)；UTF-16 [364362, 364422)。

```css
.snap-tap-group{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364422, 364469)；UTF-16 [364422, 364469)。

```css
.snap-tap-desc{margin-bottom:20px;margin-top:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364469, 364652)；UTF-16 [364469, 364652)。

```css
.snap-tap-add-button{align-items:center;border:2px solid #ccc;border-radius:5px;color:#ccc;display:flex;font-size:20px;height:44px;justify-content:center;position:relative;width:64px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364652, 364715)；UTF-16 [364652, 364715)。

```css
.snap-tap-add-button .disabled{opacity:30%;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364715, 364777)；UTF-16 [364715, 364777)。

```css
.snap-tap-add-button:hover{border-color:#44d62c;color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364777, 364930)；UTF-16 [364777, 364930)。

```css
.snap-tap-add-button .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [364930, 365037)；UTF-16 [364930, 365037)。

```css
.snap-tap-add-button .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365037, 365104)；UTF-16 [365037, 365104)。

```css
.snap-tap-add-button .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365104, 365169)；UTF-16 [365104, 365169)。

```css
.snap-tap-add-button-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365169, 365347)；UTF-16 [365169, 365347)。

```css
.snap-tap-icon-delete{background-image:url(../../static/media/icon_delete.de9b7746.svg);background-repeat:no-repeat;background-size:cover;height:20px;margin-left:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365347, 365446)；UTF-16 [365347, 365446)。

```css
.snap-tap-icon-delete:hover{background-image:url(../../static/media/icon_delete_snap.c1abb283.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365446, 365507)；UTF-16 [365446, 365507)。

```css
.snap-tap-wrapper{display:flex;justify-content:space-between}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365507, 365570)；UTF-16 [365507, 365570)。

```css
.snap-tap-key-list{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365570, 365657)；UTF-16 [365570, 365657)。

```css
.snap-tap-key-list-wrapper{align-items:center;display:flex;gap:10px;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365657, 365717)；UTF-16 [365657, 365717)。

```css
.create-snaptap{display:flex;flex-direction:column;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365717, 365784)；UTF-16 [365717, 365784)。

```css
.create-snaptap-header{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365784, 365924)；UTF-16 [365784, 365924)。

```css
.create-snaptap-header-title{background-color:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:6px 16px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [365924, 366006)；UTF-16 [365924, 366006)。

```css
.create-snaptap-header-title.disable{background-color:#30961f;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [366006, 366061)；UTF-16 [366006, 366061)。

```css
.create-snaptap.disable{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [366061, 366130)；UTF-16 [366061, 366130)。

```css
.create-snaptap-message{display:block;font-size:14px;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [366130, 366177)；UTF-16 [366130, 366177)。

```css
.create-snaptap-message--warning{color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [366177, 366221)；UTF-16 [366177, 366221)。

```css
.create-snaptap-message--success{color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [366221, 366270)；UTF-16 [366221, 366270)。

```css
.create-snaptap-message--introduction{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [366270, 366351)；UTF-16 [366270, 366351)。

```css
.widget .titleRow .shortcuts .shortcutButton-snaptap{padding:5px 10px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367219, 367280)；UTF-16 [367219, 367280)。

```css
.key-record,.key-record-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367280, 367406)；UTF-16 [367280, 367406)。

```css
.key-record-item{border:2px solid #ccc;border-radius:4px;box-sizing:initial;height:40px;justify-content:center;min-width:60px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367406, 367452)；UTF-16 [367406, 367452)。

```css
.key-record-item-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367452, 367505)；UTF-16 [367452, 367505)。

```css
.key-record-item-editing-active{border-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367505, 367559)；UTF-16 [367505, 367559)。

```css
.key-record-item-editing-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367559, 367737)；UTF-16 [367559, 367737)。

```css
.key-record-item-editing div{text-wrap:nowrap;align-items:center;display:flex;font-size:11px;height:25px;justify-content:center;margin:auto 10px;min-width:38px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367737, 367831)；UTF-16 [367737, 367831)。

```css
.key-record-item-assignment{align-items:center;color:#fff;display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367831, 367923)；UTF-16 [367831, 367923)。

```css
.key-record-item-assignment-active{background-color:#44d62c;border-color:#44d62c;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [367923, 368011)；UTF-16 [367923, 368011)。

```css
.key-record-item-assignment-deactive{background-color:#888;border-color:#888;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [368011, 368093)；UTF-16 [368011, 368093)。

```css
.key-record-item-assignment div{font-size:11px;margin:auto 10px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [368093, 368151)；UTF-16 [368093, 368151)。

```css
.blink-active{animation:blinker-active 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [368151, 368211)；UTF-16 [368151, 368211)。

```css
.blink-warning{animation:blinker-warning 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [368498, 368570)；UTF-16 [368498, 368570)。

```css
.blink-active-border{animation:blinker-active-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [368570, 368644)；UTF-16 [368570, 368644)。

```css
.blink-warning-border{animation:blinker-warning-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [368887, 368924)；UTF-16 [368887, 368924)。

```css
.snaptap-shortcuts{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [368924, 369087)；UTF-16 [368924, 369087)。

```css
.snaptap-shortcuts .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369087, 369192)；UTF-16 [369087, 369192)。

```css
.snaptap-shortcuts .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369192, 369257)；UTF-16 [369192, 369257)。

```css
.snaptap-shortcuts .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369257, 369320)；UTF-16 [369257, 369320)。

```css
.snaptap-shortcuts-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369320, 369493)；UTF-16 [369320, 369493)。

```css
.snap-tap-add-button-v3{color:#ccc;margin-top:10px!important;margin:auto;padding:6px;text-align:center;text-decoration:underline;width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369493, 369537)；UTF-16 [369493, 369537)。

```css
.snap-tap-add-button-v3:hover{color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369537, 369609)；UTF-16 [369537, 369609)。

```css
.snap-tap-key-list-v3{background:#1f1f1f;border-radius:5px;padding:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369609, 369793)；UTF-16 [369609, 369793)。

```css
.snap-tap-key-list-v3 .title-snap-tap{align-items:center;display:flex;font-size:10px;justify-content:center;line-height:12px;text-align:center;white-space:normal;word-break:break-word}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369793, 369954)；UTF-16 [369793, 369954)。

```css
.key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:initial;display:flex;height:30px;justify-content:center;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [369954, 370067)；UTF-16 [369954, 370067)。

```css
.key-record-item-v3-text{border-radius:3px;margin:0 2px!important;min-width:0!important;padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [370067, 370122)；UTF-16 [370067, 370122)。

```css
.snap-tap-widget .titleRow{position:relative;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [370122, 370189)；UTF-16 [370122, 370189)。

```css
.snaptap-shortcuts-overlay:hover+.tip{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [370339, 370432)；UTF-16 [370339, 370432)。

```css
.single-key-snap-tap-key-list{display:flex;flex-direction:column;min-height:72px;width:160px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [370432, 370478)；UTF-16 [370432, 370478)。

```css
.single-key-snap-tap-to-left{margin-left:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [370478, 370530)；UTF-16 [370478, 370530)。

```css
.single-key-snap-tap-desc{display:block;width:530px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [370530, 370645)；UTF-16 [370530, 370645)。

```css
.key-record-item-v3-skst-text{border-radius:3px;min-width:0!important;padding:0 10px!important;text-transform:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [370975, 371154)；UTF-16 [370975, 371154)。

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [371154, 371210)；UTF-16 [371154, 371210)。

```css
.skst-key-record-item-v3:hover{border:1px solid #9b9b9b}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [371210, 371381)；UTF-16 [371210, 371381)。

```css
.skst-key-record-item-v3-warning{align-items:center;border:1px solid #fd8611;border-radius:4px;color:#fd8611;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [371381, 371500)；UTF-16 [371381, 371500)。

```css
.snap-tap-pair{grid-gap:20px 30px;align-items:stretch;display:grid;gap:20px 30px;grid-template-columns:repeat(2,160px)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/515/ui/static/css/main.f65da71b.css` SHA-256 `d8038bcd55d84e4f57b6c09057fe78e5c4a2495def92e0e259b2b72f861746e4`

字符 [371500, 371558)；UTF-16 [371500, 371558)。

```css
.single-key-snap-tap-key-list>:last-child{margin-top:auto}
```
