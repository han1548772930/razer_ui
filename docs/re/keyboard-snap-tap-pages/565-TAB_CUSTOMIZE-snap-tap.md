# PID 565 — TAB_CUSTOMIZE — Snap Tap

本文件从当前官方原文件静态提取；未执行官方 JS。仅覆盖此界面的 Snap Tap 区域，不代表整个 Customize 页完成。

分支：`ordinary`；列：`right`；快捷键：`False`。偏移单位为 Unicode 字符，另存 UTF-16 偏移。

## 交互与缺口

普通分支：默认 A/D、标题开关、最多四对、首对不可删除、按键释放录入、重复/禁用键警告、成功消息三秒、窗口失焦及离页清理、调整模式警告。原服务输入捕获/映射恢复、真实配置写回和观察尚未连接，不能将本地草稿视作设备保存。

以下 JavaScript 含原 JSX 编译后的 HTML 元素结构和事件处理；CSS 为原规则完整文本。共享依赖、native 调用与源码未提取分支仍是明确缺口。

## 实际挂载与条件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/js/main.3280575a.js` SHA-256 `02384c503b5b673215daf6434c0d6bcd2392f8bd38d7ebc84c0250ec99786391`

字符 [6671681, 6672431)；UTF-16 [6671681, 6672431)。

```javascript
ren:(0,Fa.jsx)(En.A,{text:He.LUv})})]}),(0,Fa.jsx)("div",{children:(0,Fa.jsxs)("div",{style:{position:"relative"},children:[(0,Fa.jsx)("div",{className:"help"}),(0,Fa.jsx)("div",{style:{width:362},className:"tip hypershift-mode-tip",children:(0,Fa.jsx)(En.A,{text:He.Dq6})})]})})]}),"macro"!==this.props.displayMode?(0,Fa.jsx)("div",{style:{width:"100%"},children:(0,Fa.jsx)(ws,{children:(0,Fa.jsxs)(Ys,{direction:"right",children:[(0,Fa.jsx)(cT,{}),(0,Fa.jsx)(bI,{})]})})}):null]})}}const VI=(0,U.connect)(e=>({lang:e.languageReducer.lang,buttonList:e.customizeReducer.buttonList,activeButton:e.customizeReducer.activeButton,isPanelOpen:e.customizeReducer.isPanelOpen,isMappingOpen:e.customizeReducer.isMappingOpen,isMappingChanged:e.customizeReduce
```

## 标题、开关、帮助与快捷键

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/js/main.3280575a.js` SHA-256 `02384c503b5b673215daf6434c0d6bcd2392f8bd38d7ebc84c0250ec99786391`

字符 [6665768, 6666751)；UTF-16 [6665768, 6666751)。

```javascript
bI=e=>{let t=e.supportsShortcut;const a=(0,U.useDispatch)(),n=(0,U.useSelector)(e=>e.snapTapReducer.keyList),o=(0,U.useSelector)(e=>e.snapTapReducer.isEnabled),i=(0,U.useSelector)(e=>e.customizeReducer.adjustmentModeRunning),E=(0,U.useSelector)(e=>e.customizeReducer.buttonList),r=(0,m.useMemo)(()=>{if(!E||!E.length)return!1;const e=["DKM_F6","DKM_D2"];return E.some(t=>e.includes(t.inputID))},[E]);return(0,Fa.jsxs)(ks,{title:He.Fqe,tips:r?He.qI5:He.f_m,hasSwitch:!0,toggleSwitch:()=>{i?a((0,Fi.onShowAdjustmentModePrompt)(!0)):(o&&B_(),"SYSTEM"===oe.DeviceInfo.category&&(0,We.JB)({enabled:!o,snapTapPairs:o?[]:n,label:MI.xt.ENABLE}),a((0,w_.vU)(!o)))},active:o,supportsShortcut:t,renderShortcut:(0,Fa.jsx)(BI,{keyCombinations:["FN","L SHIFT"],tip:He.cyD}),extraClass:"snap-tap-widget",children:[(0,Fa.jsx)("p",{className:"snap-tap-desc",children:(0,Fa.jsx)(En.A,{text:He.PVl})}),(0,Fa.jsx)("section",{className:"snap-tap ".concat(o?"":"disabled"),children:(0,Fa.jsx)(FI,{})})]})}
```

## 完整录入编辑组件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/js/main.3280575a.js` SHA-256 `02384c503b5b673215daf6434c0d6bcd2392f8bd38d7ebc84c0250ec99786391`

字符 [6659548, 6665300)；UTF-16 [6659548, 6665300)。

```javascript
FI=()=>{const e=(0,U.useDispatch)(),t=(0,U.useSelector)(e=>e.snapTapReducer),a=t.isEnabled,n=t.keyList,o=(0,U.useSelector)(e=>{var t;return null!==(t=e.keymapReducer)&&void 0!==t?t:[]}).directionMode,i=(0,U.useSelector)(e=>e.profileReducer.selectedProfileGuid),E=(0,U.useSelector)(e=>e.deviceReducer),r=E.isBle,s=E.isDongle,_=(0,m.useState)(!1),T=(0,ya.A)(_,2),I=T[0],c=T[1],A=(0,m.useState)(X_.READY),l=(0,ya.A)(A,2),O=l[0],S=l[1],d=(0,m.useState)(pI.INTRODUCTION),u=(0,ya.A)(d,2),R=u[0],N=u[1],D=(0,m.useState)(-1),C=(0,ya.A)(D,2),L=C[0],P=C[1],M=(0,m.useState)(n),h=(0,ya.A)(M,2),g=h[0],G=h[1],f=(0,m.useRef)(null),H=(0,m.useRef)(!1),y=(0,m.useRef)(null),v=(0,m.useRef)(null),F=(0,m.useRef)(null);(0,m.useEffect)(()=>{H.current=!1,N(pI.INTRODUCTION),P(-1),S(X_.READY)},[i]),(0,m.useEffect)(()=>{G(n),B(n)},[JSON.stringify(n)]),(0,m.useEffect)(()=>{var t,a;const o=null!==(t=null===n||void 0===n?void 0:n.filter(e=>{let t=e.key1,a=e.key2;return t&&a}))&&void 0!==t?t:[];o.length!==(null!==(a=null===n||void 0===n?void 0:n.length)&&void 0!==a?a:0)&&e((0,w_.gr)(o))},[]);const B=e=>{"SYSTEM"===oe.DeviceInfo.category&&(0,We.JB)({enabled:a,snapTapPairs:e,label:MI.xt.CHANGE})},b=()=>{document.removeEventListener("mousedown",V),window.removeEventListener("keyup",k,!0),window.removeEventListener("keydown",z,!0),window.removeEventListener("blur",K),Vt.A.off("inputredirect",W)};(0,m.useEffect)(()=>(a?(window.addEventListener("blur",K),window.addEventListener("keyup",k,!0),window.addEventListener("keydown",z,!0),document.addEventListener("mousedown",V),Vt.A.on("inputredirect",W)):w(),()=>{b()}),[a,O,JSON.stringify(g)]);const w=()=>{b(),R!==pI.EMPTY_KEY_WARNING&&R!==pI.DUPLICATE_KEY_WARNING||q(L)},V=t=>{if(O!==X_.READY&&v.current instanceof Node&&t.target instanceof Node&&v.current&&!v.current.contains(t.target)&&F.current instanceof Node&&F.current&&!F.current.contains(t.target)){if(g.some(e=>!e.key1||!e.key2))return void N(pI.DUPLICATE_KEY_WARNING);if(UI.includes(R))return;e((0,w_.gr)(g)),X()}};(0,m.useEffect)(()=>{const t=()=>{let t=[...n];if(O===X_.KEY2){const e=g.find(e=>e.id===L);e&&(t=t.map(t=>t.id===L?(0,p.A)((0,p.A)({},t),{},{key1:e.key1}):t))}const a=t.filter(e=>e.key1&&e.key2);e((0,w_.gr)(a)),X()};return window.addEventListener("beforeunload",t),()=>{window.removeEventListener("beforeunload",t)}},[JSON.stringify(g),R]);const Y=(0,m.useMemo)(()=>({onInputRedirectEvent:e=>{if(O===X_.READY)return;const t=JSON.parse(e.input);if(!GI.includes(t.type))return;let a=nr.Rk.find(e=>{const a="number"===typeof e.outputFlag?e.outputFlag:e.flag;return t.scancode===e.scancode&&(a===t.flag||a+1===t.flag)});a||"razerKey"!==t.type||(a=(null!==hI&&void 0!==hI?hI:g_).find(e=>e.key===t.key.toString())),a&&(e=>e.flag%2===1)(t)&&Z(a)},onWindowBlur:()=>{let t=g.filter(e=>{let t=e.id,a=e.key1,n=e.key2;return!(UI.includes(R)&&t===L)&&a&&n});if(1===g.length&&0===t.length)return G(n),B(n),N(pI.INTRODUCTION),void X();e((0,w_.gr)(t)),N(pI.INTRODUCTION),X()}}),[O,JSON.stringify(g),L]),W=Y.onInputRedirectEvent,K=Y.onWindowBlur,k=e=>{if(e.preventDefault(),!0===e.metaKey||"Meta"===e.key)return;const t=v_(e);t&&Z(t)},z=e=>{!0!==e.metaKey&&"Meta"!==e.key||e.stopImmediatePropagation()},x=()=>{H.current=!1,clearTimeout(f.current),Vt.A.disableMapping(),ha.A.callElectronAction({action:"registerNoBrowserInputHandler"}),Vt.A.on("inputredirect",W),F_(r,s)},X=()=>{H.current=!1,S(X_.READY),ha.A.callElectronAction({action:"unRegisterNoBrowserInputHandler"}),Vt.A.enableMapping(),B_(r,s)},j=(0,m.useRef)(X);j.current=X,(0,m.useEffect)(()=>()=>{j.current()},[]);const Z=t=>{const a=t.inputID;if(((e,t)=>e&&"KEY_NUMPAD_NUM_LOCK"===t)(H.current,a))H.current=!1;else if(H.current="KEY_PAUSE"===a,!HI.includes(t.inputID)||"KEYPAD"!==oe.DeviceInfo.category||o!==Ie.mZI.DIRECTIONAL)switch(O){case X_.KEY1:let t=structuredClone(g);if(t=g.map(e=>{if(e.id===L){return(0,p.A)((0,p.A)({},e),{},{key1:a})}return e}),N(pI.INTRODUCTION),((e,t)=>{const a=t.reduce((t,a)=>t+Object.values(a).filter(t=>t===e).length,0);return a>1})(a,t))return void N(pI.DUPLICATE_KEY_WARNING);if(yI(a,t))return void N(pI.DUPLICATE_KEY_WARNING);G(t),B(t),S(X_.KEY2);break;case X_.KEY2:let n=structuredClone(g);if(n=g.map(e=>{if(e.id===L){return(0,p.A)((0,p.A)({},e),{},{key2:a})}return e}),yI(a,n))return void N(pI.DUPLICATE_KEY_WARNING);G(n),B(n),e((0,w_.gr)(n)),X(),N(pI.SUCCESS),f.current=setTimeout(()=>{N(pI.INTRODUCTION)},3e3)}},q=t=>{t===L?(P(-1),N(pI.INTRODUCTION),X()):t<L&&P(e=>e-1);let a=g.filter(e=>e.id!==t).map((e,t)=>(0,p.A)((0,p.A)({},e),{},{id:t+1})),o=n.find(e=>e.id===t);g.some(e=>!e.key1||!e.key2)||o.key1===o.key2||R!==pI.EMPTY_KEY_WARNING&&R!==pI.DUPLICATE_KEY_WARNING||(a=n,G(n),B(n)),e((0,w_.gr)(a))},Q=(e,t)=>{O===X_.READY&&(S(t),P(e),x())};return(0,Fa.jsxs)("div",{children:[(0,Fa.jsxs)("div",{className:"snap-tap-wrapper",children:[(0,Fa.jsx)("div",{className:"snap-tap-key-list",ref:v,children:(0,Fa.jsx)("div",{className:"snap-tap-group",children:null===g||void 0===g?void 0:g.map(e=>(0,Fa.jsx)(mI,{keyGroup:e,recordingState:O,onDelete:()=>q(e.id),onEdit:Q,isEditing:e.id===L,isDuplicateKeyWarning:R===pI.DUPLICATE_KEY_WARNING&&e.id===L,messageStatus:R},e.id))})}),(0,Fa.jsxs)("div",{className:"snap-tap-add-button ".concat(g.length>=4||O!==X_.READY?"disabled":""),onClick:()=>{const t=[...n,{key1:"",key2:"",id:n.length+1}];S(X_.KEY1),P(t.length),e((0,w_.xg)(t)),x()},ref:F,children:[(0,Fa.jsx)("div",{className:I?"showTooltip":"hideTooltip",ref:y,children:(0,Fa.jsx)("p",{children:(0,Fa.jsx)(En.A,{text:He.Yjp})})}),(0,Fa.jsx)("div",{className:"snap-tap-add-button-overlay",onMouseEnter:()=>c(!0),onMouseLeave:()=>c(!1),onMouseMove:e=>{y.current.style.top=e.clientY+20+"px",y.current.style.left=e.clientX+10+"px"}}),"+"]})]}),(0,Fa.jsx)(vI,{status:R})]})}
```

## 按键对行

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/js/main.3280575a.js` SHA-256 `02384c503b5b673215daf6434c0d6bcd2392f8bd38d7ebc84c0250ec99786391`

字符 [6657960, 6658723)；UTF-16 [6657960, 6658723)。

```javascript
mI=e=>{let t=e.onDelete,a=e.keyGroup,n=e.recordingState,o=e.isDuplicateKeyWarning,i=e.onEdit,E=e.messageStatus,r=e.isEditing;const s=(0,U.useSelector)(e=>e.deviceReducer.layoutId);return(0,Fa.jsxs)("div",{className:"snap-tap-key-list-wrapper",children:[(0,Fa.jsx)(W_,{isRecording:r&&n===X_.KEY1,idleState:"READY",name:G_(a.key1)?f_(a.key1):(0,z_.A)(s,a.key1,"KEYBOARD"),isWarning:r&&(n===X_.KEY1&&o||n===X_.KEY1&&E===pI.EMPTY_KEY_WARNING),onEdit:()=>i(a.id,X_.KEY1)}),(0,Fa.jsx)(W_,{isRecording:r&&n===X_.KEY2,idleState:"READY",name:G_(a.key2)?f_(a.key2):(0,z_.A)(s,a.key2,"KEYBOARD"),isWarning:r&&(n===X_.KEY2&&o||n===X_.KEY2&&E===pI.EMPTY_KEY_WARNING),onEdit:()=>i(a.id,X_.KEY2)}),1!==a.id?(0,Fa.jsx)("div",{className:"snap-tap-icon-delete",onClick:t}):null]})}
```

## CSS

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [17636, 17741)；UTF-16 [17636, 17741)。

```css
.key-snap-tap{background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg);height:16px;width:16px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [17741, 17824)；UTF-16 [17741, 17824)。

```css
.key-dks,.key-snap-tap{background-size:cover;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [75532, 75822)；UTF-16 [75532, 75822)。

```css
.factory-default .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [75822, 75891)；UTF-16 [75822, 75891)。

```css
.factory-default .warning-alert .title{color:#ccc;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [75891, 76180)；UTF-16 [75891, 76180)。

```css
.mode-switcher .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [76180, 76263)；UTF-16 [76180, 76263)。

```css
.mode-switcher .warning-alert .title{color:#fd8611;display:flex;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [76263, 76322)；UTF-16 [76263, 76322)。

```css
.mode-switcher .warning-alert .title span{margin-right:4px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [76322, 76382)；UTF-16 [76322, 76382)。

```css
.mode-switcher .warning-alert .content{white-space:pre-wrap}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [87272, 87335)；UTF-16 [87272, 87335)。

```css
.disabled .check-box:hover{border-color:#737373;cursor:default}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [118104, 118145)；UTF-16 [118104, 118145)。

```css
.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [123543, 123787)；UTF-16 [123543, 123787)。

```css
.remove-alert,.warning-alert{background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [123787, 124014)；UTF-16 [123787, 124014)。

```css
.remove-alert .title,.warning-alert .title{align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124014, 124224)；UTF-16 [124014, 124224)。

```css
.remove-alert .title .icon,.warning-alert .title .icon{background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124224, 124347)；UTF-16 [124224, 124347)。

```css
.remove-alert .body,.warning-alert .body{color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124347, 124437)；UTF-16 [124347, 124437)。

```css
.remove-alert .action,.warning-alert .action{display:flex;gap:20px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124437, 124814)；UTF-16 [124437, 124814)。

```css
.remove-alert .action .ok-button,.remove-alert .action .secondary-button,.warning-alert .action .ok-button,.warning-alert .action .secondary-button{align-items:center;background-color:#707070;border-radius:3px;color:#ccc;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124814, 125121)；UTF-16 [124814, 125121)。

```css
.remove-alert .action .enable-button,.warning-alert .action .enable-button{align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360056, 360108)；UTF-16 [360056, 360108)。

```css
.combined-key-blink-active{padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360108, 360160)；UTF-16 [360108, 360160)。

```css
.body-widgets .widget>div.snap-tap{will-change:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360160, 360186)；UTF-16 [360160, 360186)。

```css
.snap-tap{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360186, 360237)；UTF-16 [360186, 360237)。

```css
.snap-tap .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360237, 360297)；UTF-16 [360237, 360297)。

```css
.snap-tap-group{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360297, 360344)；UTF-16 [360297, 360344)。

```css
.snap-tap-desc{margin-bottom:20px;margin-top:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360344, 360527)；UTF-16 [360344, 360527)。

```css
.snap-tap-add-button{align-items:center;border:2px solid #ccc;border-radius:5px;color:#ccc;display:flex;font-size:20px;height:44px;justify-content:center;position:relative;width:64px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360527, 360590)；UTF-16 [360527, 360590)。

```css
.snap-tap-add-button .disabled{opacity:30%;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360590, 360652)；UTF-16 [360590, 360652)。

```css
.snap-tap-add-button:hover{border-color:#44d62c;color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360652, 360805)；UTF-16 [360652, 360805)。

```css
.snap-tap-add-button .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360805, 360912)；UTF-16 [360805, 360912)。

```css
.snap-tap-add-button .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360912, 360979)；UTF-16 [360912, 360979)。

```css
.snap-tap-add-button .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360979, 361044)；UTF-16 [360979, 361044)。

```css
.snap-tap-add-button-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361044, 361222)；UTF-16 [361044, 361222)。

```css
.snap-tap-icon-delete{background-image:url(../../static/media/icon_delete.de9b7746.svg);background-repeat:no-repeat;background-size:cover;height:20px;margin-left:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361222, 361321)；UTF-16 [361222, 361321)。

```css
.snap-tap-icon-delete:hover{background-image:url(../../static/media/icon_delete_snap.c1abb283.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361321, 361382)；UTF-16 [361321, 361382)。

```css
.snap-tap-wrapper{display:flex;justify-content:space-between}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361382, 361445)；UTF-16 [361382, 361445)。

```css
.snap-tap-key-list{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361445, 361532)；UTF-16 [361445, 361532)。

```css
.snap-tap-key-list-wrapper{align-items:center;display:flex;gap:10px;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361532, 361592)；UTF-16 [361532, 361592)。

```css
.create-snaptap{display:flex;flex-direction:column;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361592, 361659)；UTF-16 [361592, 361659)。

```css
.create-snaptap-header{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361659, 361799)；UTF-16 [361659, 361799)。

```css
.create-snaptap-header-title{background-color:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:6px 16px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361799, 361881)；UTF-16 [361799, 361881)。

```css
.create-snaptap-header-title.disable{background-color:#30961f;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361881, 361936)；UTF-16 [361881, 361936)。

```css
.create-snaptap.disable{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361936, 362005)；UTF-16 [361936, 362005)。

```css
.create-snaptap-message{display:block;font-size:14px;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362005, 362052)；UTF-16 [362005, 362052)。

```css
.create-snaptap-message--warning{color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362052, 362096)；UTF-16 [362052, 362096)。

```css
.create-snaptap-message--success{color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362096, 362145)；UTF-16 [362096, 362145)。

```css
.create-snaptap-message--introduction{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362145, 362226)；UTF-16 [362145, 362226)。

```css
.widget .titleRow .shortcuts .shortcutButton-snaptap{padding:5px 10px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363094, 363155)；UTF-16 [363094, 363155)。

```css
.key-record,.key-record-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363155, 363281)；UTF-16 [363155, 363281)。

```css
.key-record-item{border:2px solid #ccc;border-radius:4px;box-sizing:initial;height:40px;justify-content:center;min-width:60px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363281, 363327)；UTF-16 [363281, 363327)。

```css
.key-record-item-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363327, 363380)；UTF-16 [363327, 363380)。

```css
.key-record-item-editing-active{border-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363380, 363434)；UTF-16 [363380, 363434)。

```css
.key-record-item-editing-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363434, 363612)；UTF-16 [363434, 363612)。

```css
.key-record-item-editing div{text-wrap:nowrap;align-items:center;display:flex;font-size:11px;height:25px;justify-content:center;margin:auto 10px;min-width:38px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363612, 363706)；UTF-16 [363612, 363706)。

```css
.key-record-item-assignment{align-items:center;color:#fff;display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363706, 363798)；UTF-16 [363706, 363798)。

```css
.key-record-item-assignment-active{background-color:#44d62c;border-color:#44d62c;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363798, 363886)；UTF-16 [363798, 363886)。

```css
.key-record-item-assignment-deactive{background-color:#888;border-color:#888;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363886, 363968)；UTF-16 [363886, 363968)。

```css
.key-record-item-assignment div{font-size:11px;margin:auto 10px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363968, 364026)；UTF-16 [363968, 364026)。

```css
.blink-active{animation:blinker-active 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364026, 364086)；UTF-16 [364026, 364086)。

```css
.blink-warning{animation:blinker-warning 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364373, 364445)；UTF-16 [364373, 364445)。

```css
.blink-active-border{animation:blinker-active-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364445, 364519)；UTF-16 [364445, 364519)。

```css
.blink-warning-border{animation:blinker-warning-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364762, 364799)；UTF-16 [364762, 364799)。

```css
.snaptap-shortcuts{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364799, 364962)；UTF-16 [364799, 364962)。

```css
.snaptap-shortcuts .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364962, 365067)；UTF-16 [364962, 365067)。

```css
.snaptap-shortcuts .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365067, 365132)；UTF-16 [365067, 365132)。

```css
.snaptap-shortcuts .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365132, 365195)；UTF-16 [365132, 365195)。

```css
.snaptap-shortcuts-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365195, 365368)；UTF-16 [365195, 365368)。

```css
.snap-tap-add-button-v3{color:#ccc;margin-top:10px!important;margin:auto;padding:6px;text-align:center;text-decoration:underline;width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365368, 365412)；UTF-16 [365368, 365412)。

```css
.snap-tap-add-button-v3:hover{color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365412, 365484)；UTF-16 [365412, 365484)。

```css
.snap-tap-key-list-v3{background:#1f1f1f;border-radius:5px;padding:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365484, 365668)；UTF-16 [365484, 365668)。

```css
.snap-tap-key-list-v3 .title-snap-tap{align-items:center;display:flex;font-size:10px;justify-content:center;line-height:12px;text-align:center;white-space:normal;word-break:break-word}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365668, 365829)；UTF-16 [365668, 365829)。

```css
.key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:initial;display:flex;height:30px;justify-content:center;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365829, 365942)；UTF-16 [365829, 365942)。

```css
.key-record-item-v3-text{border-radius:3px;margin:0 2px!important;min-width:0!important;padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365942, 365997)；UTF-16 [365942, 365997)。

```css
.snap-tap-widget .titleRow{position:relative;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365997, 366064)；UTF-16 [365997, 366064)。

```css
.snaptap-shortcuts-overlay:hover+.tip{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366214, 366307)；UTF-16 [366214, 366307)。

```css
.single-key-snap-tap-key-list{display:flex;flex-direction:column;min-height:72px;width:160px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366307, 366353)；UTF-16 [366307, 366353)。

```css
.single-key-snap-tap-to-left{margin-left:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366353, 366405)；UTF-16 [366353, 366405)。

```css
.single-key-snap-tap-desc{display:block;width:530px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366405, 366520)；UTF-16 [366405, 366520)。

```css
.key-record-item-v3-skst-text{border-radius:3px;min-width:0!important;padding:0 10px!important;text-transform:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366850, 367029)；UTF-16 [366850, 367029)。

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367029, 367085)；UTF-16 [367029, 367085)。

```css
.skst-key-record-item-v3:hover{border:1px solid #9b9b9b}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367085, 367256)；UTF-16 [367085, 367256)。

```css
.skst-key-record-item-v3-warning{align-items:center;border:1px solid #fd8611;border-radius:4px;color:#fd8611;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367256, 367375)；UTF-16 [367256, 367375)。

```css
.snap-tap-pair{grid-gap:20px 30px;align-items:stretch;display:grid;gap:20px 30px;grid-template-columns:repeat(2,160px)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/565/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367375, 367433)；UTF-16 [367375, 367433)。

```css
.single-key-snap-tap-key-list>:last-child{margin-top:auto}
```
