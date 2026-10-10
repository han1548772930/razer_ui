# PID 567 — TAB_CUSTOMIZE — Snap Tap

本文件从当前官方原文件静态提取；未执行官方 JS。仅覆盖此界面的 Snap Tap 区域，不代表整个 Customize 页完成。

分支：`ordinary`；列：`right`；快捷键：`False`。偏移单位为 Unicode 字符，另存 UTF-16 偏移。

## 交互与缺口

普通分支：默认 A/D、标题开关、最多四对、首对不可删除、按键释放录入、重复/禁用键警告、成功消息三秒、窗口失焦及离页清理、调整模式警告。原服务输入捕获/映射恢复、真实配置写回和观察尚未连接，不能将本地草稿视作设备保存。

以下 JavaScript 含原 JSX 编译后的 HTML 元素结构和事件处理；CSS 为原规则完整文本。共享依赖、native 调用与源码未提取分支仍是明确缺口。

## 实际挂载与条件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/js/main.33124f31.js` SHA-256 `8bfe6cc4ee10ce0b01c86063a46bfa25710664fcab22eabdc405396890b39b3a`

字符 [7395491, 7396241)；UTF-16 [7395491, 7396241)。

```javascript
cn.A,{text:ye.LUv})})]}),(0,wa.jsx)("div",{children:(0,wa.jsxs)("div",{style:{position:"relative"},children:[(0,wa.jsx)("div",{className:"help"}),(0,wa.jsx)("div",{style:{width:362},className:"tip hypershift-mode-tip",children:(0,wa.jsx)(cn.A,{text:ye.Dq6})})]})})]}),"macro"!==this.props.displayMode?(0,wa.jsxs)(er,{children:[(0,wa.jsx)(ar,{direction:"left",children:(0,wa.jsx)(mI,{})}),(0,wa.jsxs)(ar,{direction:"right",children:[(0,wa.jsx)(HI,{}),(0,wa.jsx)(xI,{})]})]}):null]})}}const jI=(0,g.connect)(e=>({lang:e.languageReducer.lang,buttonList:e.customizeReducer.buttonList,activeButton:e.customizeReducer.activeButton,isPanelOpen:e.customizeReducer.isPanelOpen,isMappingOpen:e.customizeReducer.isMappingOpen,isMappingChanged:e.customizeReducer
```

## 标题、开关、帮助与快捷键

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/js/main.33124f31.js` SHA-256 `8bfe6cc4ee10ce0b01c86063a46bfa25710664fcab22eabdc405396890b39b3a`

字符 [7389563, 7390546)；UTF-16 [7389563, 7390546)。

```javascript
xI=e=>{let t=e.supportsShortcut;const a=(0,g.useDispatch)(),n=(0,g.useSelector)(e=>e.snapTapReducer.keyList),E=(0,g.useSelector)(e=>e.snapTapReducer.isEnabled),o=(0,g.useSelector)(e=>e.customizeReducer.adjustmentModeRunning),i=(0,g.useSelector)(e=>e.customizeReducer.buttonList),s=(0,m.useMemo)(()=>{if(!i||!i.length)return!1;const e=["DKM_F6","DKM_D2"];return i.some(t=>e.includes(t.inputID))},[i]);return(0,wa.jsxs)(or,{title:ye.Fqe,tips:s?ye.qI5:ye.f_m,hasSwitch:!0,toggleSwitch:()=>{o?a((0,wo.onShowAdjustmentModePrompt)(!0)):(E&&aI(),"SYSTEM"===oe.DeviceInfo.category&&(0,we.JB)({enabled:!E,snapTapPairs:E?[]:n,label:FI.xt.ENABLE}),a((0,EI.vU)(!E)))},active:E,supportsShortcut:t,renderShortcut:(0,wa.jsx)(kI,{keyCombinations:["FN","L SHIFT"],tip:ye.cyD}),extraClass:"snap-tap-widget",children:[(0,wa.jsx)("p",{className:"snap-tap-desc",children:(0,wa.jsx)(cn.A,{text:ye.PVl})}),(0,wa.jsx)("section",{className:"snap-tap ".concat(E?"":"disabled"),children:(0,wa.jsx)(zI,{})})]})}
```

## 完整录入编辑组件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/js/main.33124f31.js` SHA-256 `8bfe6cc4ee10ce0b01c86063a46bfa25710664fcab22eabdc405396890b39b3a`

字符 [7383343, 7389095)；UTF-16 [7383343, 7389095)。

```javascript
zI=()=>{const e=(0,g.useDispatch)(),t=(0,g.useSelector)(e=>e.snapTapReducer),a=t.isEnabled,n=t.keyList,E=(0,g.useSelector)(e=>{var t;return null!==(t=e.keymapReducer)&&void 0!==t?t:[]}).directionMode,o=(0,g.useSelector)(e=>e.profileReducer.selectedProfileGuid),i=(0,g.useSelector)(e=>e.deviceReducer),s=i.isBle,_=i.isDongle,r=(0,m.useState)(!1),T=(0,ba.A)(r,2),I=T[0],A=T[1],c=(0,m.useState)(II.READY),O=(0,ba.A)(c,2),l=O[0],S=O[1],R=(0,m.useState)(GI.INTRODUCTION),d=(0,ba.A)(R,2),u=d[0],N=d[1],D=(0,m.useState)(-1),C=(0,ba.A)(D,2),L=C[0],p=C[1],P=(0,m.useState)(n),U=(0,ba.A)(P,2),h=U[0],H=U[1],G=(0,m.useRef)(null),f=(0,m.useRef)(!1),y=(0,m.useRef)(null),F=(0,m.useRef)(null),v=(0,m.useRef)(null);(0,m.useEffect)(()=>{f.current=!1,N(GI.INTRODUCTION),p(-1),S(II.READY)},[o]),(0,m.useEffect)(()=>{H(n),B(n)},[JSON.stringify(n)]),(0,m.useEffect)(()=>{var t,a;const E=null!==(t=null===n||void 0===n?void 0:n.filter(e=>{let t=e.key1,a=e.key2;return t&&a}))&&void 0!==t?t:[];E.length!==(null!==(a=null===n||void 0===n?void 0:n.length)&&void 0!==a?a:0)&&e((0,EI.gr)(E))},[]);const B=e=>{"SYSTEM"===oe.DeviceInfo.category&&(0,we.JB)({enabled:a,snapTapPairs:e,label:FI.xt.CHANGE})},Y=()=>{document.removeEventListener("mousedown",K),window.removeEventListener("keyup",z,!0),window.removeEventListener("keydown",k,!0),window.removeEventListener("blur",w),Xt.A.off("inputredirect",W)};(0,m.useEffect)(()=>(a?(window.addEventListener("blur",w),window.addEventListener("keyup",z,!0),window.addEventListener("keydown",k,!0),document.addEventListener("mousedown",K),Xt.A.on("inputredirect",W)):V(),()=>{Y()}),[a,l,JSON.stringify(h)]);const V=()=>{Y(),u!==GI.EMPTY_KEY_WARNING&&u!==GI.DUPLICATE_KEY_WARNING||q(L)},K=t=>{if(l!==II.READY&&F.current instanceof Node&&t.target instanceof Node&&F.current&&!F.current.contains(t.target)&&v.current instanceof Node&&v.current&&!v.current.contains(t.target)){if(h.some(e=>!e.key1||!e.key2))return void N(GI.DUPLICATE_KEY_WARNING);if(BI.includes(u))return;e((0,EI.gr)(h)),X()}};(0,m.useEffect)(()=>{const t=()=>{let t=[...n];if(l===II.KEY2){const e=h.find(e=>e.id===L);e&&(t=t.map(t=>t.id===L?(0,M.A)((0,M.A)({},t),{},{key1:e.key1}):t))}const a=t.filter(e=>e.key1&&e.key2);e((0,EI.gr)(a)),X()};return window.addEventListener("beforeunload",t),()=>{window.removeEventListener("beforeunload",t)}},[JSON.stringify(h),u]);const b=(0,m.useMemo)(()=>({onInputRedirectEvent:e=>{if(l===II.READY)return;const t=JSON.parse(e.input);if(!VI.includes(t.type))return;let a=Ts.Rk.find(e=>{const a="number"===typeof e.outputFlag?e.outputFlag:e.flag;return t.scancode===e.scancode&&(a===t.flag||a+1===t.flag)});a||"razerKey"!==t.type||(a=(null!==vI&&void 0!==vI?vI:ZT).find(e=>e.key===t.key.toString())),a&&(e=>e.flag%2===1)(t)&&Z(a)},onWindowBlur:()=>{let t=h.filter(e=>{let t=e.id,a=e.key1,n=e.key2;return!(BI.includes(u)&&t===L)&&a&&n});if(1===h.length&&0===t.length)return H(n),B(n),N(GI.INTRODUCTION),void X();e((0,EI.gr)(t)),N(GI.INTRODUCTION),X()}}),[l,JSON.stringify(h),L]),W=b.onInputRedirectEvent,w=b.onWindowBlur,z=e=>{if(e.preventDefault(),!0===e.metaKey||"Meta"===e.key)return;const t=eI(e);t&&Z(t)},k=e=>{!0!==e.metaKey&&"Meta"!==e.key||e.stopImmediatePropagation()},x=()=>{f.current=!1,clearTimeout(G.current),Xt.A.disableMapping(),Fa.A.callElectronAction({action:"registerNoBrowserInputHandler"}),Xt.A.on("inputredirect",W),tI(s,_)},X=()=>{f.current=!1,S(II.READY),Fa.A.callElectronAction({action:"unRegisterNoBrowserInputHandler"}),Xt.A.enableMapping(),aI(s,_)},j=(0,m.useRef)(X);j.current=X,(0,m.useEffect)(()=>()=>{j.current()},[]);const Z=t=>{const a=t.inputID;if(((e,t)=>e&&"KEY_NUMPAD_NUM_LOCK"===t)(f.current,a))f.current=!1;else if(f.current="KEY_PAUSE"===a,!bI.includes(t.inputID)||"KEYPAD"!==oe.DeviceInfo.category||E!==Ae.mZI.DIRECTIONAL)switch(l){case II.KEY1:let t=structuredClone(h);if(t=h.map(e=>{if(e.id===L){return(0,M.A)((0,M.A)({},e),{},{key1:a})}return e}),N(GI.INTRODUCTION),((e,t)=>{const a=t.reduce((t,a)=>t+Object.values(a).filter(t=>t===e).length,0);return a>1})(a,t))return void N(GI.DUPLICATE_KEY_WARNING);if(WI(a,t))return void N(GI.DUPLICATE_KEY_WARNING);H(t),B(t),S(II.KEY2);break;case II.KEY2:let n=structuredClone(h);if(n=h.map(e=>{if(e.id===L){return(0,M.A)((0,M.A)({},e),{},{key2:a})}return e}),WI(a,n))return void N(GI.DUPLICATE_KEY_WARNING);H(n),B(n),e((0,EI.gr)(n)),X(),N(GI.SUCCESS),G.current=setTimeout(()=>{N(GI.INTRODUCTION)},3e3)}},q=t=>{t===L?(p(-1),N(GI.INTRODUCTION),X()):t<L&&p(e=>e-1);let a=h.filter(e=>e.id!==t).map((e,t)=>(0,M.A)((0,M.A)({},e),{},{id:t+1})),E=n.find(e=>e.id===t);h.some(e=>!e.key1||!e.key2)||E.key1===E.key2||u!==GI.EMPTY_KEY_WARNING&&u!==GI.DUPLICATE_KEY_WARNING||(a=n,H(n),B(n)),e((0,EI.gr)(a))},Q=(e,t)=>{l===II.READY&&(S(t),p(e),x())};return(0,wa.jsxs)("div",{children:[(0,wa.jsxs)("div",{className:"snap-tap-wrapper",children:[(0,wa.jsx)("div",{className:"snap-tap-key-list",ref:F,children:(0,wa.jsx)("div",{className:"snap-tap-group",children:null===h||void 0===h?void 0:h.map(e=>(0,wa.jsx)(fI,{keyGroup:e,recordingState:l,onDelete:()=>q(e.id),onEdit:Q,isEditing:e.id===L,isDuplicateKeyWarning:u===GI.DUPLICATE_KEY_WARNING&&e.id===L,messageStatus:u},e.id))})}),(0,wa.jsxs)("div",{className:"snap-tap-add-button ".concat(h.length>=4||l!==II.READY?"disabled":""),onClick:()=>{const t=[...n,{key1:"",key2:"",id:n.length+1}];S(II.KEY1),p(t.length),e((0,EI.xg)(t)),x()},ref:v,children:[(0,wa.jsx)("div",{className:I?"showTooltip":"hideTooltip",ref:y,children:(0,wa.jsx)("p",{children:(0,wa.jsx)(cn.A,{text:ye.Yjp})})}),(0,wa.jsx)("div",{className:"snap-tap-add-button-overlay",onMouseEnter:()=>A(!0),onMouseLeave:()=>A(!1),onMouseMove:e=>{y.current.style.top=e.clientY+20+"px",y.current.style.left=e.clientX+10+"px"}}),"+"]})]}),(0,wa.jsx)(wI,{status:u})]})}
```

## 按键对行

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/js/main.33124f31.js` SHA-256 `8bfe6cc4ee10ce0b01c86063a46bfa25710664fcab22eabdc405396890b39b3a`

字符 [7381755, 7382518)；UTF-16 [7381755, 7382518)。

```javascript
fI=e=>{let t=e.onDelete,a=e.keyGroup,n=e.recordingState,E=e.isDuplicateKeyWarning,o=e.onEdit,i=e.messageStatus,s=e.isEditing;const _=(0,g.useSelector)(e=>e.deviceReducer.layoutId);return(0,wa.jsxs)("div",{className:"snap-tap-key-list-wrapper",children:[(0,wa.jsx)(sI,{isRecording:s&&n===II.KEY1,idleState:"READY",name:qT(a.key1)?QT(a.key1):(0,Qr.A)(_,a.key1,"KEYBOARD"),isWarning:s&&(n===II.KEY1&&E||n===II.KEY1&&i===GI.EMPTY_KEY_WARNING),onEdit:()=>o(a.id,II.KEY1)}),(0,wa.jsx)(sI,{isRecording:s&&n===II.KEY2,idleState:"READY",name:qT(a.key2)?QT(a.key2):(0,Qr.A)(_,a.key2,"KEYBOARD"),isWarning:s&&(n===II.KEY2&&E||n===II.KEY2&&i===GI.EMPTY_KEY_WARNING),onEdit:()=>o(a.id,II.KEY2)}),1!==a.id?(0,wa.jsx)("div",{className:"snap-tap-icon-delete",onClick:t}):null]})}
```

## CSS

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [17636, 17741)；UTF-16 [17636, 17741)。

```css
.key-snap-tap{background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg);height:16px;width:16px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [17741, 17824)；UTF-16 [17741, 17824)。

```css
.key-dks,.key-snap-tap{background-size:cover;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [75532, 75822)；UTF-16 [75532, 75822)。

```css
.factory-default .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [75822, 75891)；UTF-16 [75822, 75891)。

```css
.factory-default .warning-alert .title{color:#ccc;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [75891, 76180)；UTF-16 [75891, 76180)。

```css
.mode-switcher .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [76180, 76263)；UTF-16 [76180, 76263)。

```css
.mode-switcher .warning-alert .title{color:#fd8611;display:flex;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [76263, 76322)；UTF-16 [76263, 76322)。

```css
.mode-switcher .warning-alert .title span{margin-right:4px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [76322, 76382)；UTF-16 [76322, 76382)。

```css
.mode-switcher .warning-alert .content{white-space:pre-wrap}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [87272, 87335)；UTF-16 [87272, 87335)。

```css
.disabled .check-box:hover{border-color:#737373;cursor:default}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [118104, 118145)；UTF-16 [118104, 118145)。

```css
.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [123543, 123787)；UTF-16 [123543, 123787)。

```css
.remove-alert,.warning-alert{background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [123787, 124014)；UTF-16 [123787, 124014)。

```css
.remove-alert .title,.warning-alert .title{align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124014, 124224)；UTF-16 [124014, 124224)。

```css
.remove-alert .title .icon,.warning-alert .title .icon{background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124224, 124347)；UTF-16 [124224, 124347)。

```css
.remove-alert .body,.warning-alert .body{color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124347, 124437)；UTF-16 [124347, 124437)。

```css
.remove-alert .action,.warning-alert .action{display:flex;gap:20px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124437, 124814)；UTF-16 [124437, 124814)。

```css
.remove-alert .action .ok-button,.remove-alert .action .secondary-button,.warning-alert .action .ok-button,.warning-alert .action .secondary-button{align-items:center;background-color:#707070;border-radius:3px;color:#ccc;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [124814, 125121)；UTF-16 [124814, 125121)。

```css
.remove-alert .action .enable-button,.warning-alert .action .enable-button{align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360056, 360108)；UTF-16 [360056, 360108)。

```css
.combined-key-blink-active{padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360108, 360160)；UTF-16 [360108, 360160)。

```css
.body-widgets .widget>div.snap-tap{will-change:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360160, 360186)；UTF-16 [360160, 360186)。

```css
.snap-tap{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360186, 360237)；UTF-16 [360186, 360237)。

```css
.snap-tap .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360237, 360297)；UTF-16 [360237, 360297)。

```css
.snap-tap-group{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360297, 360344)；UTF-16 [360297, 360344)。

```css
.snap-tap-desc{margin-bottom:20px;margin-top:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360344, 360527)；UTF-16 [360344, 360527)。

```css
.snap-tap-add-button{align-items:center;border:2px solid #ccc;border-radius:5px;color:#ccc;display:flex;font-size:20px;height:44px;justify-content:center;position:relative;width:64px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360527, 360590)；UTF-16 [360527, 360590)。

```css
.snap-tap-add-button .disabled{opacity:30%;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360590, 360652)；UTF-16 [360590, 360652)。

```css
.snap-tap-add-button:hover{border-color:#44d62c;color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360652, 360805)；UTF-16 [360652, 360805)。

```css
.snap-tap-add-button .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360805, 360912)；UTF-16 [360805, 360912)。

```css
.snap-tap-add-button .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360912, 360979)；UTF-16 [360912, 360979)。

```css
.snap-tap-add-button .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [360979, 361044)；UTF-16 [360979, 361044)。

```css
.snap-tap-add-button-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361044, 361222)；UTF-16 [361044, 361222)。

```css
.snap-tap-icon-delete{background-image:url(../../static/media/icon_delete.de9b7746.svg);background-repeat:no-repeat;background-size:cover;height:20px;margin-left:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361222, 361321)；UTF-16 [361222, 361321)。

```css
.snap-tap-icon-delete:hover{background-image:url(../../static/media/icon_delete_snap.c1abb283.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361321, 361382)；UTF-16 [361321, 361382)。

```css
.snap-tap-wrapper{display:flex;justify-content:space-between}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361382, 361445)；UTF-16 [361382, 361445)。

```css
.snap-tap-key-list{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361445, 361532)；UTF-16 [361445, 361532)。

```css
.snap-tap-key-list-wrapper{align-items:center;display:flex;gap:10px;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361532, 361592)；UTF-16 [361532, 361592)。

```css
.create-snaptap{display:flex;flex-direction:column;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361592, 361659)；UTF-16 [361592, 361659)。

```css
.create-snaptap-header{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361659, 361799)；UTF-16 [361659, 361799)。

```css
.create-snaptap-header-title{background-color:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:6px 16px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361799, 361881)；UTF-16 [361799, 361881)。

```css
.create-snaptap-header-title.disable{background-color:#30961f;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361881, 361936)；UTF-16 [361881, 361936)。

```css
.create-snaptap.disable{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [361936, 362005)；UTF-16 [361936, 362005)。

```css
.create-snaptap-message{display:block;font-size:14px;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362005, 362052)；UTF-16 [362005, 362052)。

```css
.create-snaptap-message--warning{color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362052, 362096)；UTF-16 [362052, 362096)。

```css
.create-snaptap-message--success{color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362096, 362145)；UTF-16 [362096, 362145)。

```css
.create-snaptap-message--introduction{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [362145, 362226)；UTF-16 [362145, 362226)。

```css
.widget .titleRow .shortcuts .shortcutButton-snaptap{padding:5px 10px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363094, 363155)；UTF-16 [363094, 363155)。

```css
.key-record,.key-record-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363155, 363281)；UTF-16 [363155, 363281)。

```css
.key-record-item{border:2px solid #ccc;border-radius:4px;box-sizing:initial;height:40px;justify-content:center;min-width:60px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363281, 363327)；UTF-16 [363281, 363327)。

```css
.key-record-item-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363327, 363380)；UTF-16 [363327, 363380)。

```css
.key-record-item-editing-active{border-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363380, 363434)；UTF-16 [363380, 363434)。

```css
.key-record-item-editing-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363434, 363612)；UTF-16 [363434, 363612)。

```css
.key-record-item-editing div{text-wrap:nowrap;align-items:center;display:flex;font-size:11px;height:25px;justify-content:center;margin:auto 10px;min-width:38px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363612, 363706)；UTF-16 [363612, 363706)。

```css
.key-record-item-assignment{align-items:center;color:#fff;display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363706, 363798)；UTF-16 [363706, 363798)。

```css
.key-record-item-assignment-active{background-color:#44d62c;border-color:#44d62c;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363798, 363886)；UTF-16 [363798, 363886)。

```css
.key-record-item-assignment-deactive{background-color:#888;border-color:#888;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363886, 363968)；UTF-16 [363886, 363968)。

```css
.key-record-item-assignment div{font-size:11px;margin:auto 10px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [363968, 364026)；UTF-16 [363968, 364026)。

```css
.blink-active{animation:blinker-active 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364026, 364086)；UTF-16 [364026, 364086)。

```css
.blink-warning{animation:blinker-warning 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364373, 364445)；UTF-16 [364373, 364445)。

```css
.blink-active-border{animation:blinker-active-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364445, 364519)；UTF-16 [364445, 364519)。

```css
.blink-warning-border{animation:blinker-warning-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364762, 364799)；UTF-16 [364762, 364799)。

```css
.snaptap-shortcuts{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364799, 364962)；UTF-16 [364799, 364962)。

```css
.snaptap-shortcuts .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [364962, 365067)；UTF-16 [364962, 365067)。

```css
.snaptap-shortcuts .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365067, 365132)；UTF-16 [365067, 365132)。

```css
.snaptap-shortcuts .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365132, 365195)；UTF-16 [365132, 365195)。

```css
.snaptap-shortcuts-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365195, 365368)；UTF-16 [365195, 365368)。

```css
.snap-tap-add-button-v3{color:#ccc;margin-top:10px!important;margin:auto;padding:6px;text-align:center;text-decoration:underline;width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365368, 365412)；UTF-16 [365368, 365412)。

```css
.snap-tap-add-button-v3:hover{color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365412, 365484)；UTF-16 [365412, 365484)。

```css
.snap-tap-key-list-v3{background:#1f1f1f;border-radius:5px;padding:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365484, 365668)；UTF-16 [365484, 365668)。

```css
.snap-tap-key-list-v3 .title-snap-tap{align-items:center;display:flex;font-size:10px;justify-content:center;line-height:12px;text-align:center;white-space:normal;word-break:break-word}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365668, 365829)；UTF-16 [365668, 365829)。

```css
.key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:initial;display:flex;height:30px;justify-content:center;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365829, 365942)；UTF-16 [365829, 365942)。

```css
.key-record-item-v3-text{border-radius:3px;margin:0 2px!important;min-width:0!important;padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365942, 365997)；UTF-16 [365942, 365997)。

```css
.snap-tap-widget .titleRow{position:relative;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [365997, 366064)；UTF-16 [365997, 366064)。

```css
.snaptap-shortcuts-overlay:hover+.tip{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366214, 366307)；UTF-16 [366214, 366307)。

```css
.single-key-snap-tap-key-list{display:flex;flex-direction:column;min-height:72px;width:160px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366307, 366353)；UTF-16 [366307, 366353)。

```css
.single-key-snap-tap-to-left{margin-left:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366353, 366405)；UTF-16 [366353, 366405)。

```css
.single-key-snap-tap-desc{display:block;width:530px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366405, 366520)；UTF-16 [366405, 366520)。

```css
.key-record-item-v3-skst-text{border-radius:3px;min-width:0!important;padding:0 10px!important;text-transform:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [366850, 367029)；UTF-16 [366850, 367029)。

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367029, 367085)；UTF-16 [367029, 367085)。

```css
.skst-key-record-item-v3:hover{border:1px solid #9b9b9b}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367085, 367256)；UTF-16 [367085, 367256)。

```css
.skst-key-record-item-v3-warning{align-items:center;border:1px solid #fd8611;border-radius:4px;color:#fd8611;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367256, 367375)；UTF-16 [367256, 367375)。

```css
.snap-tap-pair{grid-gap:20px 30px;align-items:stretch;display:grid;gap:20px 30px;grid-template-columns:repeat(2,160px)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/567/ui/static/css/main.d44c89be.css` SHA-256 `25bf7acc339317b94ec55865619017b32e2c615ea9d07513308b0c07d72ec600`

字符 [367375, 367433)；UTF-16 [367375, 367433)。

```css
.single-key-snap-tap-key-list>:last-child{margin-top:auto}
```
