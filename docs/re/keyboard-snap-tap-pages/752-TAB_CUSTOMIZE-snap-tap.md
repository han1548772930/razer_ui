# PID 752 — TAB_CUSTOMIZE — Snap Tap

本文件从当前官方原文件静态提取；未执行官方 JS。仅覆盖此界面的 Snap Tap 区域，不代表整个 Customize 页完成。

分支：`ordinary`；列：`left`；快捷键：`False`。偏移单位为 Unicode 字符，另存 UTF-16 偏移。

## 交互与缺口

普通分支：默认 A/D、标题开关、最多四对、首对不可删除、按键释放录入、重复/禁用键警告、成功消息三秒、窗口失焦及离页清理、调整模式警告。原服务输入捕获/映射恢复、真实配置写回和观察尚未连接，不能将本地草稿视作设备保存。

以下 JavaScript 含原 JSX 编译后的 HTML 元素结构和事件处理；CSS 为原规则完整文本。共享依赖、native 调用与源码未提取分支仍是明确缺口。

## 实际挂载与条件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/js/8141.480d3f0c.chunk.js` SHA-256 `bca8156dab8bb0d015e56d220d87f901bb070e0ae287a14697392eef6f8df66d`

字符 [1747653, 1748403)；UTF-16 [1747653, 1748403)。

```javascript
WJ})}),(0,I.jsx)("div",{className:"text hypershift",children:(0,I.jsx)(O.A,{text:D.LUv})})]}),(0,I.jsx)("div",{children:(0,I.jsxs)("div",{style:{position:"relative"},children:[(0,I.jsx)("div",{className:"help"}),(0,I.jsx)("div",{style:{width:362},className:"tip hypershift-mode-tip",children:(0,I.jsx)(O.A,{text:D.Dq6})})]})})]}),"macro"!==this.props.displayMode?(0,I.jsxs)(d.A,{children:[(0,I.jsxs)(Zt.A,{direction:"left",children:[(0,I.jsx)(Fe,{}),(0,I.jsx)(vn,{})]}),(0,I.jsxs)(Zt.A,{direction:"right",children:[(0,I.jsx)(W,{category:g.DeviceInfo.category}),(0,I.jsx)(dt,{})]})]}):null]})}}const Pn=(0,_.connect)(e=>({lang:e.languageReducer.lang,buttonList:e.customizeReducer.buttonList,activeButton:e.customizeReducer.activeButton,isPanelOpen:e.c
```

## 标题、开关、帮助与快捷键

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/js/8141.480d3f0c.chunk.js` SHA-256 `bca8156dab8bb0d015e56d220d87f901bb070e0ae287a14697392eef6f8df66d`

字符 [1739485, 1740450)；UTF-16 [1739485, 1740450)。

```javascript
vn=e=>{let{supportsShortcut:t}=e;const n=(0,_.useDispatch)(),a=(0,_.useSelector)(e=>e.snapTapReducer.keyList),r=(0,_.useSelector)(e=>e.snapTapReducer.isEnabled),s=(0,_.useSelector)(e=>e.customizeReducer.adjustmentModeRunning),o=(0,_.useSelector)(e=>e.customizeReducer.buttonList),c=(0,i.useMemo)(()=>{if(!o||!o.length)return!1;const e=["DKM_F6","DKM_D2"];return o.some(t=>e.includes(t.inputID))},[o]);return(0,I.jsxs)(u.A,{title:D.Fqe,tips:c?D.qI5:D.f_m,hasSwitch:!0,toggleSwitch:()=>{s?n((0,N.onShowAdjustmentModePrompt)(!0)):(r&&Ee(),"SYSTEM"===g.DeviceInfo.category&&(0,In.JB)({enabled:!r,snapTapPairs:r?[]:a,label:Tn.xt.ENABLE}),n((0,he.vU)(!r)))},active:r,supportsShortcut:t,renderShortcut:(0,I.jsx)(fn,{keyCombinations:["FN","L SHIFT"],tip:D.cyD}),extraClass:"snap-tap-widget",children:[(0,I.jsx)("p",{className:"snap-tap-desc",children:(0,I.jsx)(O.A,{text:D.PVl})}),(0,I.jsx)("section",{className:"snap-tap "+(r?"":"disabled"),children:(0,I.jsx)(Ln,{})})]})}
```

## 完整录入编辑组件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/js/8141.480d3f0c.chunk.js` SHA-256 `bca8156dab8bb0d015e56d220d87f901bb070e0ae287a14697392eef6f8df66d`

字符 [1733550, 1739032)；UTF-16 [1733550, 1739032)。

```javascript
Ln=()=>{const e=(0,_.useDispatch)(),{isEnabled:t,keyList:n}=(0,_.useSelector)(e=>e.snapTapReducer),{directionMode:a}=(0,_.useSelector)(e=>{var t;return null!==(t=e.keymapReducer)&&void 0!==t?t:[]}),r=(0,_.useSelector)(e=>e.profileReducer.selectedProfileGuid),{isBle:s,isDongle:o}=(0,_.useSelector)(e=>e.deviceReducer),[c,l]=(0,i.useState)(!1),[d,E]=(0,i.useState)(ge.READY),[u,h]=(0,i.useState)(Sn.INTRODUCTION),[A,S]=(0,i.useState)(-1),[T,C]=(0,i.useState)(n),N=(0,i.useRef)(null),p=(0,i.useRef)(!1),y=(0,i.useRef)(null),R=(0,i.useRef)(null),L=(0,i.useRef)(null);(0,i.useEffect)(()=>{p.current=!1,h(Sn.INTRODUCTION),S(-1),E(ge.READY)},[r]),(0,i.useEffect)(()=>{C(n),f(n)},[JSON.stringify(n)]),(0,i.useEffect)(()=>{var t,a;const r=null!==(t=null===n||void 0===n?void 0:n.filter(e=>{let{key1:t,key2:n}=e;return t&&n}))&&void 0!==t?t:[];r.length!==(null!==(a=null===n||void 0===n?void 0:n.length)&&void 0!==a?a:0)&&e((0,he.gr)(r))},[]);const f=e=>{"SYSTEM"===g.DeviceInfo.category&&(0,In.JB)({enabled:t,snapTapPairs:e,label:Tn.xt.CHANGE})},v=()=>{document.removeEventListener("mousedown",w),window.removeEventListener("keyup",F,!0),window.removeEventListener("keydown",G,!0),window.removeEventListener("blur",$),te.A.off("inputredirect",P)};(0,i.useEffect)(()=>(t?(window.addEventListener("blur",$),window.addEventListener("keyup",F,!0),window.addEventListener("keydown",G,!0),document.addEventListener("mousedown",w),te.A.on("inputredirect",P)):U(),()=>{v()}),[t,d,JSON.stringify(T)]);const U=()=>{v(),u!==Sn.EMPTY_KEY_WARNING&&u!==Sn.DUPLICATE_KEY_WARNING||Y(A)},w=t=>{if(d!==ge.READY&&R.current instanceof Node&&t.target instanceof Node&&R.current&&!R.current.contains(t.target)&&L.current instanceof Node&&L.current&&!L.current.contains(t.target)){if(T.some(e=>!e.key1||!e.key2))return void h(Sn.DUPLICATE_KEY_WARNING);if(gn.includes(u))return;e((0,he.gr)(T)),H()}};(0,i.useEffect)(()=>{const t=()=>{let t=[...n];if(d===ge.KEY2){const e=T.find(e=>e.id===A);e&&(t=t.map(t=>t.id===A?{...t,key1:e.key1}:t))}const a=t.filter(e=>e.key1&&e.key2);e((0,he.gr)(a)),H()};return window.addEventListener("beforeunload",t),()=>{window.removeEventListener("beforeunload",t)}},[JSON.stringify(T),u]);const{onInputRedirectEvent:P,onWindowBlur:$}=(0,i.useMemo)(()=>({onInputRedirectEvent:e=>{if(d===ge.READY)return;const t=JSON.parse(e.input);if(!Mn.includes(t.type))return;let n=X.Rk.find(e=>{const n="number"===typeof e.outputFlag?e.outputFlag:e.flag;return t.scancode===e.scancode&&(n===t.flag||n+1===t.flag)});n||"razerKey"!==t.type||(n=(null!==Cn&&void 0!==Cn?Cn:ie).find(e=>e.key===t.key.toString())),n&&(e=>e.flag%2===1)(t)&&B(n)},onWindowBlur:()=>{let t=T.filter(e=>{let{id:t,key1:n,key2:a}=e;return!(gn.includes(u)&&t===A)&&n&&a});if(1===T.length&&0===t.length)return C(n),f(n),h(Sn.INTRODUCTION),void H();e((0,he.gr)(t)),h(Sn.INTRODUCTION),H()}}),[d,JSON.stringify(T),A]),F=e=>{if(e.preventDefault(),!0===e.metaKey||"Meta"===e.key)return;const t=de(e);t&&B(t)},G=e=>{!0!==e.metaKey&&"Meta"!==e.key||e.stopImmediatePropagation()},K=()=>{p.current=!1,clearTimeout(N.current),te.A.disableMapping(),M.A.callElectronAction({action:"registerNoBrowserInputHandler"}),te.A.on("inputredirect",P),_e(s,o)},H=()=>{p.current=!1,E(ge.READY),M.A.callElectronAction({action:"unRegisterNoBrowserInputHandler"}),te.A.enableMapping(),Ee(s,o)},b=(0,i.useRef)(H);b.current=H,(0,i.useEffect)(()=>()=>{b.current()},[]);const B=t=>{const{inputID:n}=t;if(((e,t)=>e&&"KEY_NUMPAD_NUM_LOCK"===t)(p.current,n))p.current=!1;else if(p.current="KEY_PAUSE"===n,!yn.includes(t.inputID)||"KEYPAD"!==g.DeviceInfo.category||a!==m.mZI.DIRECTIONAL)switch(d){case ge.KEY1:let t=structuredClone(T);if(t=T.map(e=>{if(e.id===A){return{...e,key1:n}}return e}),h(Sn.INTRODUCTION),((e,t)=>t.reduce((t,n)=>t+Object.values(n).filter(t=>t===e).length,0)>1)(n,t))return void h(Sn.DUPLICATE_KEY_WARNING);if(Rn(n,t))return void h(Sn.DUPLICATE_KEY_WARNING);C(t),f(t),E(ge.KEY2);break;case ge.KEY2:let a=structuredClone(T);if(a=T.map(e=>{if(e.id===A){return{...e,key2:n}}return e}),Rn(n,a))return void h(Sn.DUPLICATE_KEY_WARNING);C(a),f(a),e((0,he.gr)(a)),H(),h(Sn.SUCCESS),N.current=setTimeout(()=>{h(Sn.INTRODUCTION)},3e3)}},Y=t=>{t===A?(S(-1),h(Sn.INTRODUCTION),H()):t<A&&S(e=>e-1);let a=T.filter(e=>e.id!==t).map((e,t)=>({...e,id:t+1})),r=n.find(e=>e.id===t);T.some(e=>!e.key1||!e.key2)||r.key1===r.key2||u!==Sn.EMPTY_KEY_WARNING&&u!==Sn.DUPLICATE_KEY_WARNING||(a=n,C(n),f(n)),e((0,he.gr)(a))},V=(e,t)=>{d===ge.READY&&(E(t),S(e),K())};return(0,I.jsxs)("div",{children:[(0,I.jsxs)("div",{className:"snap-tap-wrapper",children:[(0,I.jsx)("div",{className:"snap-tap-key-list",ref:R,children:(0,I.jsx)("div",{className:"snap-tap-group",children:null===T||void 0===T?void 0:T.map(e=>(0,I.jsx)(Dn,{keyGroup:e,recordingState:d,onDelete:()=>Y(e.id),onEdit:V,isEditing:e.id===A,isDuplicateKeyWarning:u===Sn.DUPLICATE_KEY_WARNING&&e.id===A,messageStatus:u},e.id))})}),(0,I.jsxs)("div",{className:"snap-tap-add-button "+(T.length>=4||d!==ge.READY?"disabled":""),onClick:()=>{const t=[...n,{key1:"",key2:"",id:n.length+1}];E(ge.KEY1),S(t.length),e((0,he.xg)(t)),K()},ref:L,children:[(0,I.jsx)("div",{className:c?"showTooltip":"hideTooltip",ref:y,children:(0,I.jsx)("p",{children:(0,I.jsx)(O.A,{text:D.Yjp})})}),(0,I.jsx)("div",{className:"snap-tap-add-button-overlay",onMouseEnter:()=>l(!0),onMouseLeave:()=>l(!1),onMouseMove:e=>{y.current.style.top=e.clientY+20+"px",y.current.style.left=e.clientX+10+"px"}}),"+"]})]}),(0,I.jsx)(On,{status:u})]})}
```

## 按键对行

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/js/8141.480d3f0c.chunk.js` SHA-256 `bca8156dab8bb0d015e56d220d87f901bb070e0ae287a14697392eef6f8df66d`

字符 [1732002, 1732750)；UTF-16 [1732002, 1732750)。

```javascript
Dn=e=>{let{onDelete:t,keyGroup:n,recordingState:a,isDuplicateKeyWarning:r,onEdit:i,messageStatus:s,isEditing:o}=e;const c=(0,_.useSelector)(e=>e.deviceReducer.layoutId);return(0,I.jsxs)("div",{className:"snap-tap-key-list-wrapper",children:[(0,I.jsx)(Se,{isRecording:o&&a===ge.KEY1,idleState:"READY",name:se(n.key1)?oe(n.key1):(0,Te.A)(c,n.key1,"KEYBOARD"),isWarning:o&&(a===ge.KEY1&&r||a===ge.KEY1&&s===Sn.EMPTY_KEY_WARNING),onEdit:()=>i(n.id,ge.KEY1)}),(0,I.jsx)(Se,{isRecording:o&&a===ge.KEY2,idleState:"READY",name:se(n.key2)?oe(n.key2):(0,Te.A)(c,n.key2,"KEYBOARD"),isWarning:o&&(a===ge.KEY2&&r||a===ge.KEY2&&s===Sn.EMPTY_KEY_WARNING),onEdit:()=>i(n.id,ge.KEY2)}),1!==n.id?(0,I.jsx)("div",{className:"snap-tap-icon-delete",onClick:t}):null]})}
```

## CSS

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [62732, 62784)；UTF-16 [62732, 62784)。

```css
.combined-key-blink-active{padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [62784, 62836)；UTF-16 [62784, 62836)。

```css
.body-widgets .widget>div.snap-tap{will-change:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [62836, 62862)；UTF-16 [62836, 62862)。

```css
.snap-tap{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [62862, 62913)；UTF-16 [62862, 62913)。

```css
.snap-tap .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [62913, 62947)；UTF-16 [62913, 62947)。

```css
.snap-tap-group{width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [62947, 62994)；UTF-16 [62947, 62994)。

```css
.snap-tap-desc{margin-bottom:20px;margin-top:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [62994, 63177)；UTF-16 [62994, 63177)。

```css
.snap-tap-add-button{align-items:center;border:2px solid #ccc;border-radius:5px;color:#ccc;display:flex;font-size:20px;height:44px;justify-content:center;position:relative;width:64px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63177, 63240)；UTF-16 [63177, 63240)。

```css
.snap-tap-add-button .disabled{opacity:30%;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63240, 63302)；UTF-16 [63240, 63302)。

```css
.snap-tap-add-button:hover{border-color:#44d62c;color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63302, 63455)；UTF-16 [63302, 63455)。

```css
.snap-tap-add-button .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63455, 63562)；UTF-16 [63455, 63562)。

```css
.snap-tap-add-button .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63562, 63629)；UTF-16 [63562, 63629)。

```css
.snap-tap-add-button .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63629, 63694)；UTF-16 [63629, 63694)。

```css
.snap-tap-add-button-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63694, 63872)；UTF-16 [63694, 63872)。

```css
.snap-tap-icon-delete{background-image:url(../../static/media/icon_delete.de9b7746.svg);background-repeat:no-repeat;background-size:cover;height:20px;margin-left:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63872, 63971)；UTF-16 [63872, 63971)。

```css
.snap-tap-icon-delete:hover{background-image:url(../../static/media/icon_delete_snap.c1abb283.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [63971, 64032)；UTF-16 [63971, 64032)。

```css
.snap-tap-wrapper{display:flex;justify-content:space-between}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64032, 64069)；UTF-16 [64032, 64069)。

```css
.snap-tap-key-list{width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64069, 64156)；UTF-16 [64069, 64156)。

```css
.snap-tap-key-list-wrapper{align-items:center;display:flex;gap:10px;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64156, 64216)；UTF-16 [64156, 64216)。

```css
.create-snaptap{display:flex;flex-direction:column;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64216, 64257)；UTF-16 [64216, 64257)。

```css
.create-snaptap-header{width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64257, 64397)；UTF-16 [64257, 64397)。

```css
.create-snaptap-header-title{background-color:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:6px 16px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64397, 64479)；UTF-16 [64397, 64479)。

```css
.create-snaptap-header-title.disable{background-color:#30961f;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64479, 64534)；UTF-16 [64479, 64534)。

```css
.create-snaptap.disable{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64534, 64603)；UTF-16 [64534, 64603)。

```css
.create-snaptap-message{display:block;font-size:14px;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64603, 64650)；UTF-16 [64603, 64650)。

```css
.create-snaptap-message--warning{color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64650, 64694)；UTF-16 [64650, 64694)。

```css
.create-snaptap-message--success{color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64694, 64743)；UTF-16 [64694, 64743)。

```css
.create-snaptap-message--introduction{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [64743, 64824)；UTF-16 [64743, 64824)。

```css
.widget .titleRow .shortcuts .shortcutButton-snaptap{padding:5px 10px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [65640, 65701)；UTF-16 [65640, 65701)。

```css
.key-record,.key-record-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [65701, 65827)；UTF-16 [65701, 65827)。

```css
.key-record-item{border:2px solid #ccc;border-radius:4px;box-sizing:initial;height:40px;justify-content:center;min-width:60px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [65827, 65873)；UTF-16 [65827, 65873)。

```css
.key-record-item-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [65873, 65926)；UTF-16 [65873, 65926)。

```css
.key-record-item-editing-active{border-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [65926, 65980)；UTF-16 [65926, 65980)。

```css
.key-record-item-editing-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [65980, 66158)；UTF-16 [65980, 66158)。

```css
.key-record-item-editing div{text-wrap:nowrap;align-items:center;display:flex;font-size:11px;height:25px;justify-content:center;margin:auto 10px;min-width:38px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66158, 66252)；UTF-16 [66158, 66252)。

```css
.key-record-item-assignment{align-items:center;color:#fff;display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66252, 66344)；UTF-16 [66252, 66344)。

```css
.key-record-item-assignment-active{background-color:#44d62c;border-color:#44d62c;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66344, 66432)；UTF-16 [66344, 66432)。

```css
.key-record-item-assignment-deactive{background-color:#888;border-color:#888;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66432, 66514)；UTF-16 [66432, 66514)。

```css
.key-record-item-assignment div{font-size:11px;margin:auto 10px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66514, 66572)；UTF-16 [66514, 66572)。

```css
.blink-active{animation:blinker-active 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66572, 66632)；UTF-16 [66572, 66632)。

```css
.blink-warning{animation:blinker-warning 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66919, 66991)；UTF-16 [66919, 66991)。

```css
.blink-active-border{animation:blinker-active-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [66991, 67065)；UTF-16 [66991, 67065)。

```css
.blink-warning-border{animation:blinker-warning-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67308, 67345)；UTF-16 [67308, 67345)。

```css
.snaptap-shortcuts{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67345, 67508)；UTF-16 [67345, 67508)。

```css
.snaptap-shortcuts .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67508, 67613)；UTF-16 [67508, 67613)。

```css
.snaptap-shortcuts .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67613, 67678)；UTF-16 [67613, 67678)。

```css
.snaptap-shortcuts .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67678, 67741)；UTF-16 [67678, 67741)。

```css
.snaptap-shortcuts-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67741, 67888)；UTF-16 [67741, 67888)。

```css
.snap-tap-add-button-v3{color:#ccc;margin-top:10px!important;margin:auto;padding:6px;text-align:center;text-decoration:underline;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67888, 67932)；UTF-16 [67888, 67932)。

```css
.snap-tap-add-button-v3:hover{color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [67932, 68004)；UTF-16 [67932, 68004)。

```css
.snap-tap-key-list-v3{background:#1f1f1f;border-radius:5px;padding:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68004, 68188)；UTF-16 [68004, 68188)。

```css
.snap-tap-key-list-v3 .title-snap-tap{align-items:center;display:flex;font-size:10px;justify-content:center;line-height:12px;text-align:center;white-space:normal;word-break:break-word}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68188, 68349)；UTF-16 [68188, 68349)。

```css
.key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:initial;display:flex;height:30px;justify-content:center;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68349, 68462)；UTF-16 [68349, 68462)。

```css
.key-record-item-v3-text{border-radius:3px;margin:0 2px!important;min-width:0!important;padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68462, 68517)；UTF-16 [68462, 68517)。

```css
.snap-tap-widget .titleRow{position:relative;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68517, 68584)；UTF-16 [68517, 68584)。

```css
.snaptap-shortcuts-overlay:hover+.tip{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68734, 68827)；UTF-16 [68734, 68827)。

```css
.single-key-snap-tap-key-list{display:flex;flex-direction:column;min-height:72px;width:160px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68827, 68873)；UTF-16 [68827, 68873)。

```css
.single-key-snap-tap-to-left{margin-left:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68873, 68925)；UTF-16 [68873, 68925)。

```css
.single-key-snap-tap-desc{display:block;width:530px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [68925, 69040)；UTF-16 [68925, 69040)。

```css
.key-record-item-v3-skst-text{border-radius:3px;min-width:0!important;padding:0 10px!important;text-transform:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [69370, 69549)；UTF-16 [69370, 69549)。

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [69549, 69605)；UTF-16 [69549, 69605)。

```css
.skst-key-record-item-v3:hover{border:1px solid #9b9b9b}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [69605, 69776)；UTF-16 [69605, 69776)。

```css
.skst-key-record-item-v3-warning{align-items:center;border:1px solid #fd8611;border-radius:4px;color:#fd8611;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [69776, 69895)；UTF-16 [69776, 69895)。

```css
.snap-tap-pair{grid-gap:20px 30px;align-items:stretch;display:grid;gap:20px 30px;grid-template-columns:repeat(2,160px)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/8141.0382ebf6.chunk.css` SHA-256 `f9c4968bd2e89597fbdc560bfc89680fc2787556d2b6fa4e837c9a85b608fe46`

字符 [69895, 69953)；UTF-16 [69895, 69953)。

```css
.single-key-snap-tap-key-list>:last-child{margin-top:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [17532, 17637)；UTF-16 [17532, 17637)。

```css
.key-snap-tap{background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg);height:16px;width:16px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [17637, 17720)；UTF-16 [17637, 17720)。

```css
.key-dks,.key-snap-tap{background-size:cover;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [74988, 75251)；UTF-16 [74988, 75251)。

```css
.factory-default .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [75251, 75320)；UTF-16 [75251, 75320)。

```css
.factory-default .warning-alert .title{color:#ccc;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [75320, 75582)；UTF-16 [75320, 75582)。

```css
.mode-switcher .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [75582, 75665)；UTF-16 [75582, 75665)。

```css
.mode-switcher .warning-alert .title{color:#fd8611;display:flex;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [75665, 75724)；UTF-16 [75665, 75724)。

```css
.mode-switcher .warning-alert .title span{margin-right:4px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [75724, 75797)；UTF-16 [75724, 75797)。

```css
.mode-switcher .warning-alert .content,.widget .tip{white-space:pre-wrap}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [86509, 86572)；UTF-16 [86509, 86572)。

```css
.disabled .check-box:hover{border-color:#737373;cursor:default}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [150457, 150498)；UTF-16 [150457, 150498)。

```css
.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [156546, 156790)；UTF-16 [156546, 156790)。

```css
.remove-alert,.warning-alert{background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [156790, 157017)；UTF-16 [156790, 157017)。

```css
.remove-alert .title,.warning-alert .title{align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [157017, 157227)；UTF-16 [157017, 157227)。

```css
.remove-alert .title .icon,.warning-alert .title .icon{background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [157227, 157350)；UTF-16 [157227, 157350)。

```css
.remove-alert .body,.warning-alert .body{color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [157350, 157440)；UTF-16 [157350, 157440)。

```css
.remove-alert .action,.warning-alert .action{display:flex;gap:20px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [157440, 157817)；UTF-16 [157440, 157817)。

```css
.remove-alert .action .ok-button,.remove-alert .action .secondary-button,.warning-alert .action .ok-button,.warning-alert .action .secondary-button{align-items:center;background-color:#707070;border-radius:3px;color:#ccc;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/752/ui/static/css/main.390c1d51.css` SHA-256 `7475fffbc94627b0737f26a98412805ea45b463eecc7a2cb3da2092afb422be1`

字符 [157817, 158124)；UTF-16 [157817, 158124)。

```css
.remove-alert .action .enable-button,.warning-alert .action .enable-button{align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```
