# PID 716 — TAB_CUSTOMIZE — Snap Tap

本文件从当前官方原文件静态提取；未执行官方 JS。仅覆盖此界面的 Snap Tap 区域，不代表整个 Customize 页完成。

分支：`ordinary`；列：`left`；快捷键：`True`。偏移单位为 Unicode 字符，另存 UTF-16 偏移。

## 交互与缺口

普通分支：默认 A/D、标题开关、最多四对、首对不可删除、按键释放录入、重复/禁用键警告、成功消息三秒、窗口失焦及离页清理、调整模式警告。原服务输入捕获/映射恢复、真实配置写回和观察尚未连接，不能将本地草稿视作设备保存。

以下 JavaScript 含原 JSX 编译后的 HTML 元素结构和事件处理；CSS 为原规则完整文本。共享依赖、native 调用与源码未提取分支仍是明确缺口。

## 实际挂载与条件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/js/main.a149c89a.js` SHA-256 `c4d563c8ed9d05de9268905fad8e600a7bd8710c3340fe774d78f980fc7d3de7`

字符 [7097147, 7097897)；UTF-16 [7097148, 7097898)。

```javascript
children:(0,hn.jsx)(jn.A,{text:ye.LUv})})]}),(0,hn.jsxs)("div",{style:{position:"relative"},children:[(0,hn.jsx)("div",{className:"help"}),(0,hn.jsx)("div",{style:{width:362},className:"tip hypershift-mode-tip",children:(0,hn.jsx)(jn.A,{text:ye.Dq6})})]})]}),"macro"!==this.props.displayMode?(0,hn.jsxs)(Zs,{children:[(0,hn.jsxs)(tr,{direction:"left",children:[(0,hn.jsx)(vL,{}),this.props.isBle?null:(0,hn.jsx)(xm,{category:oe.DeviceInfo.category}),(0,hn.jsx)(pm,{supportsShortcut:!0})]}),(0,hn.jsxs)(tr,{direction:"right",children:[oe.DeviceInfo.isMultiPairingSupported&&this.state.showMultiPairingFeature?(0,hn.jsx)(WM,{deviceInfo:oe.DeviceInfo}):(0,hn.jsx)(zC,{deviceInfo:oe.DeviceInfo}),(0,hn.jsx)(gm,{}),(0,hn.jsx)(Tp,{})]})]}):null]})}}const x
```

## 标题、开关、帮助与快捷键

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/js/main.a149c89a.js` SHA-256 `c4d563c8ed9d05de9268905fad8e600a7bd8710c3340fe774d78f980fc7d3de7`

字符 [7068766, 7069744)；UTF-16 [7068767, 7069745)。

```javascript
pm=e=>{let{supportsShortcut:t}=e;const a=(0,G.useDispatch)(),n=(0,G.useSelector)(e=>e.snapTapReducer.keyList),i=(0,G.useSelector)(e=>e.snapTapReducer.isEnabled),o=(0,G.useSelector)(e=>e.customizeReducer.adjustmentModeRunning),E=(0,G.useSelector)(e=>e.customizeReducer.buttonList),s=(0,h.useMemo)(()=>{if(!E||!E.length)return!1;const e=["DKM_F6","DKM_D2"];return E.some(t=>e.includes(t.inputID))},[E]);return(0,hn.jsxs)(Js,{title:ye.Fqe,tips:s?ye.qI5:ye.f_m,hasSwitch:!0,toggleSwitch:()=>{o?a((0,Ai.onShowAdjustmentModePrompt)(!0)):(i&&_L(),"SYSTEM"===oe.DeviceInfo.category&&(0,We.JB)({enabled:!i,snapTapPairs:i?[]:n,label:lm.xt.ENABLE}),a((0,IL.vU)(!i)))},active:i,supportsShortcut:t,renderShortcut:(0,hn.jsx)(Lm,{keyCombinations:["FN","L SHIFT"],tip:ye.cyD}),extraClass:"snap-tap-widget",children:[(0,hn.jsx)("p",{className:"snap-tap-desc",children:(0,hn.jsx)(jn.A,{text:ye.PVl})}),(0,hn.jsx)("section",{className:"snap-tap "+(i?"":"disabled"),children:(0,hn.jsx)(Cm,{})})]})}
```

## 完整录入编辑组件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/js/main.a149c89a.js` SHA-256 `c4d563c8ed9d05de9268905fad8e600a7bd8710c3340fe774d78f980fc7d3de7`

字符 [7062785, 7068305)；UTF-16 [7062786, 7068306)。

```javascript
Cm=()=>{const e=(0,G.useDispatch)(),{isEnabled:t,keyList:a}=(0,G.useSelector)(e=>e.snapTapReducer),{directionMode:n}=(0,G.useSelector)(e=>{var t;return null!==(t=e.keymapReducer)&&void 0!==t?t:[]}),i=(0,G.useSelector)(e=>e.profileReducer.selectedProfileGuid),{isBle:o,isDongle:E}=(0,G.useSelector)(e=>e.deviceReducer),[s,r]=(0,h.useState)(!1),[_,T]=(0,h.useState)(DL.READY),[I,A]=(0,h.useState)(Im.INTRODUCTION),[l,O]=(0,h.useState)(-1),[d,c]=(0,h.useState)(a),S=(0,h.useRef)(null),u=(0,h.useRef)(!1),R=(0,h.useRef)(null),N=(0,h.useRef)(null),D=(0,h.useRef)(null);(0,h.useEffect)(()=>{u.current=!1,A(Im.INTRODUCTION),O(-1),T(DL.READY)},[i]),(0,h.useEffect)(()=>{c(a),C(a)},[JSON.stringify(a)]),(0,h.useEffect)(()=>{var t,n;const i=null!==(t=null===a||void 0===a?void 0:a.filter(e=>{let{key1:t,key2:a}=e;return t&&a}))&&void 0!==t?t:[];i.length!==(null!==(n=null===a||void 0===a?void 0:a.length)&&void 0!==n?n:0)&&e((0,IL.gr)(i))},[]);const C=e=>{"SYSTEM"===oe.DeviceInfo.category&&(0,We.JB)({enabled:t,snapTapPairs:e,label:lm.xt.CHANGE})},L=()=>{document.removeEventListener("mousedown",m),window.removeEventListener("keyup",U,!0),window.removeEventListener("keydown",g,!0),window.removeEventListener("blur",P),wt.A.off("inputredirect",M)};(0,h.useEffect)(()=>(t?(window.addEventListener("blur",P),window.addEventListener("keyup",U,!0),window.addEventListener("keydown",g,!0),document.addEventListener("mousedown",m),wt.A.on("inputredirect",M)):p(),()=>{L()}),[t,_,JSON.stringify(d)]);const p=()=>{L(),I!==Im.EMPTY_KEY_WARNING&&I!==Im.DUPLICATE_KEY_WARNING||F(l)},m=t=>{if(_!==DL.READY&&N.current instanceof Node&&t.target instanceof Node&&N.current&&!N.current.contains(t.target)&&D.current instanceof Node&&D.current&&!D.current.contains(t.target)){if(d.some(e=>!e.key1||!e.key2))return void A(Im.DUPLICATE_KEY_WARNING);if(dm.includes(I))return;e((0,IL.gr)(d)),H()}};(0,h.useEffect)(()=>{const t=()=>{let t=[...a];if(_===DL.KEY2){const e=d.find(e=>e.id===l);e&&(t=t.map(t=>t.id===l?{...t,key1:e.key1}:t))}const n=t.filter(e=>e.key1&&e.key2);e((0,IL.gr)(n)),H()};return window.addEventListener("beforeunload",t),()=>{window.removeEventListener("beforeunload",t)}},[JSON.stringify(d),I]);const{onInputRedirectEvent:M,onWindowBlur:P}=(0,h.useMemo)(()=>({onInputRedirectEvent:e=>{if(_===DL.READY)return;const t=JSON.parse(e.input);if(!Sm.includes(t.type))return;let a=$R.Rk.find(e=>{const a="number"===typeof e.outputFlag?e.outputFlag:e.flag;return t.scancode===e.scancode&&(a===t.flag||a+1===t.flag)});a||"razerKey"!==t.type||(a=(null!==Om&&void 0!==Om?Om:eL).find(e=>e.key===t.key.toString())),a&&(e=>e.flag%2===1)(t)&&v(a)},onWindowBlur:()=>{let t=d.filter(e=>{let{id:t,key1:a,key2:n}=e;return!(dm.includes(I)&&t===l)&&a&&n});if(1===d.length&&0===t.length)return c(a),C(a),A(Im.INTRODUCTION),void H();e((0,IL.gr)(t)),A(Im.INTRODUCTION),H()}}),[_,JSON.stringify(d),l]),U=e=>{if(e.preventDefault(),!0===e.metaKey||"Meta"===e.key)return;const t=oL(e);t&&v(t)},g=e=>{!0!==e.metaKey&&"Meta"!==e.key||e.stopImmediatePropagation()},f=()=>{u.current=!1,clearTimeout(S.current),wt.A.disableMapping(),Dn.A.callElectronAction({action:"registerNoBrowserInputHandler"}),wt.A.on("inputredirect",M),rL(o,E)},H=()=>{u.current=!1,T(DL.READY),Dn.A.callElectronAction({action:"unRegisterNoBrowserInputHandler"}),wt.A.enableMapping(),_L(o,E)},y=(0,h.useRef)(H);y.current=H,(0,h.useEffect)(()=>()=>{y.current()},[]);const v=t=>{const{inputID:a}=t;if(((e,t)=>e&&"KEY_NUMPAD_NUM_LOCK"===t)(u.current,a))u.current=!1;else if(u.current="KEY_PAUSE"===a,!Rm.includes(t.inputID)||"KEYPAD"!==oe.DeviceInfo.category||n!==Ae.mZI.DIRECTIONAL)switch(_){case DL.KEY1:let t=structuredClone(d);if(t=d.map(e=>{if(e.id===l){return{...e,key1:a}}return e}),A(Im.INTRODUCTION),((e,t)=>{const a=t.reduce((t,a)=>t+Object.values(a).filter(t=>t===e).length,0);return a>1})(a,t))return void A(Im.DUPLICATE_KEY_WARNING);if(Nm(a,t))return void A(Im.DUPLICATE_KEY_WARNING);c(t),C(t),T(DL.KEY2);break;case DL.KEY2:let n=structuredClone(d);if(n=d.map(e=>{if(e.id===l){return{...e,key2:a}}return e}),Nm(a,n))return void A(Im.DUPLICATE_KEY_WARNING);c(n),C(n),e((0,IL.gr)(n)),H(),A(Im.SUCCESS),S.current=setTimeout(()=>{A(Im.INTRODUCTION)},3e3)}},F=t=>{t===l?(O(-1),A(Im.INTRODUCTION),H()):t<l&&O(e=>e-1);let n=d.filter(e=>e.id!==t).map((e,t)=>({...e,id:t+1})),i=a.find(e=>e.id===t);d.some(e=>!e.key1||!e.key2)||i.key1===i.key2||I!==Im.EMPTY_KEY_WARNING&&I!==Im.DUPLICATE_KEY_WARNING||(n=a,c(a),C(a)),e((0,IL.gr)(n))},B=(e,t)=>{_===DL.READY&&(T(t),O(e),f())};return(0,hn.jsxs)("div",{children:[(0,hn.jsxs)("div",{className:"snap-tap-wrapper",children:[(0,hn.jsx)("div",{className:"snap-tap-key-list",ref:N,children:(0,hn.jsx)("div",{className:"snap-tap-group",children:null===d||void 0===d?void 0:d.map(e=>(0,hn.jsx)(Am,{keyGroup:e,recordingState:_,onDelete:()=>F(e.id),onEdit:B,isEditing:e.id===l,isDuplicateKeyWarning:I===Im.DUPLICATE_KEY_WARNING&&e.id===l,messageStatus:I},e.id))})}),(0,hn.jsxs)("div",{className:"snap-tap-add-button "+(d.length>=4||_!==DL.READY?"disabled":""),onClick:()=>{const t=[...a,{key1:"",key2:"",id:a.length+1}];T(DL.KEY1),O(t.length),e((0,IL.xg)(t)),f()},ref:D,children:[(0,hn.jsx)("div",{className:s?"showTooltip":"hideTooltip",ref:R,children:(0,hn.jsx)("p",{children:(0,hn.jsx)(jn.A,{text:ye.Yjp})})}),(0,hn.jsx)("div",{className:"snap-tap-add-button-overlay",onMouseEnter:()=>r(!0),onMouseLeave:()=>r(!1),onMouseMove:e=>{R.current.style.top=e.clientY+20+"px",R.current.style.left=e.clientX+10+"px"}}),"+"]})]}),(0,hn.jsx)(Dm,{status:I})]})}
```

## 按键对行

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/js/main.a149c89a.js` SHA-256 `c4d563c8ed9d05de9268905fad8e600a7bd8710c3340fe774d78f980fc7d3de7`

字符 [7061237, 7061989)；UTF-16 [7061238, 7061990)。

```javascript
Am=e=>{let{onDelete:t,keyGroup:a,recordingState:n,isDuplicateKeyWarning:i,onEdit:o,messageStatus:E,isEditing:s}=e;const r=(0,G.useSelector)(e=>e.deviceReducer.layoutId);return(0,hn.jsxs)("div",{className:"snap-tap-key-list-wrapper",children:[(0,hn.jsx)(OL,{isRecording:s&&n===DL.KEY1,idleState:"READY",name:tL(a.key1)?aL(a.key1):(0,SL.A)(r,a.key1,"KEYBOARD"),isWarning:s&&(n===DL.KEY1&&i||n===DL.KEY1&&E===Im.EMPTY_KEY_WARNING),onEdit:()=>o(a.id,DL.KEY1)}),(0,hn.jsx)(OL,{isRecording:s&&n===DL.KEY2,idleState:"READY",name:tL(a.key2)?aL(a.key2):(0,SL.A)(r,a.key2,"KEYBOARD"),isWarning:s&&(n===DL.KEY2&&i||n===DL.KEY2&&E===Im.EMPTY_KEY_WARNING),onEdit:()=>o(a.id,DL.KEY2)}),1!==a.id?(0,hn.jsx)("div",{className:"snap-tap-icon-delete",onClick:t}):null]})}
```

## CSS

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [17545, 17650)；UTF-16 [17545, 17650)。

```css
.key-snap-tap{background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg);height:16px;width:16px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [17650, 17733)；UTF-16 [17650, 17733)。

```css
.key-dks,.key-snap-tap{background-size:cover;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [75131, 75394)；UTF-16 [75131, 75394)。

```css
.factory-default .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [75394, 75463)；UTF-16 [75394, 75463)。

```css
.factory-default .warning-alert .title{color:#ccc;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [75463, 75725)；UTF-16 [75463, 75725)。

```css
.mode-switcher .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [75725, 75808)；UTF-16 [75725, 75808)。

```css
.mode-switcher .warning-alert .title{color:#fd8611;display:flex;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [75808, 75867)；UTF-16 [75808, 75867)。

```css
.mode-switcher .warning-alert .title span{margin-right:4px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [75867, 75927)；UTF-16 [75867, 75927)。

```css
.mode-switcher .warning-alert .content{white-space:pre-wrap}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [83522, 83585)；UTF-16 [83522, 83585)。

```css
.disabled .check-box:hover{border-color:#737373;cursor:default}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [114175, 114216)；UTF-16 [114175, 114216)。

```css
.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [119562, 119806)；UTF-16 [119562, 119806)。

```css
.remove-alert,.warning-alert{background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [119806, 120033)；UTF-16 [119806, 120033)。

```css
.remove-alert .title,.warning-alert .title{align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [120033, 120243)；UTF-16 [120033, 120243)。

```css
.remove-alert .title .icon,.warning-alert .title .icon{background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [120243, 120366)；UTF-16 [120243, 120366)。

```css
.remove-alert .body,.warning-alert .body{color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [120366, 120456)；UTF-16 [120366, 120456)。

```css
.remove-alert .action,.warning-alert .action{display:flex;gap:20px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [120456, 120833)；UTF-16 [120456, 120833)。

```css
.remove-alert .action .ok-button,.remove-alert .action .secondary-button,.warning-alert .action .ok-button,.warning-alert .action .secondary-button{align-items:center;background-color:#707070;border-radius:3px;color:#ccc;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [120833, 121140)；UTF-16 [120833, 121140)。

```css
.remove-alert .action .enable-button,.warning-alert .action .enable-button{align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395495, 395547)；UTF-16 [395495, 395547)。

```css
.combined-key-blink-active{padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395547, 395599)；UTF-16 [395547, 395599)。

```css
.body-widgets .widget>div.snap-tap{will-change:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395599, 395625)；UTF-16 [395599, 395625)。

```css
.snap-tap{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395625, 395676)；UTF-16 [395625, 395676)。

```css
.snap-tap .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395676, 395710)；UTF-16 [395676, 395710)。

```css
.snap-tap-group{width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395710, 395757)；UTF-16 [395710, 395757)。

```css
.snap-tap-desc{margin-bottom:20px;margin-top:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395757, 395940)；UTF-16 [395757, 395940)。

```css
.snap-tap-add-button{align-items:center;border:2px solid #ccc;border-radius:5px;color:#ccc;display:flex;font-size:20px;height:44px;justify-content:center;position:relative;width:64px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [395940, 396003)；UTF-16 [395940, 396003)。

```css
.snap-tap-add-button .disabled{opacity:30%;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396003, 396065)；UTF-16 [396003, 396065)。

```css
.snap-tap-add-button:hover{border-color:#44d62c;color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396065, 396218)；UTF-16 [396065, 396218)。

```css
.snap-tap-add-button .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396218, 396325)；UTF-16 [396218, 396325)。

```css
.snap-tap-add-button .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396325, 396392)；UTF-16 [396325, 396392)。

```css
.snap-tap-add-button .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396392, 396457)；UTF-16 [396392, 396457)。

```css
.snap-tap-add-button-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396457, 396635)；UTF-16 [396457, 396635)。

```css
.snap-tap-icon-delete{background-image:url(../../static/media/icon_delete.de9b7746.svg);background-repeat:no-repeat;background-size:cover;height:20px;margin-left:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396635, 396734)；UTF-16 [396635, 396734)。

```css
.snap-tap-icon-delete:hover{background-image:url(../../static/media/icon_delete_snap.c1abb283.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396734, 396795)；UTF-16 [396734, 396795)。

```css
.snap-tap-wrapper{display:flex;justify-content:space-between}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396795, 396832)；UTF-16 [396795, 396832)。

```css
.snap-tap-key-list{width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396832, 396919)；UTF-16 [396832, 396919)。

```css
.snap-tap-key-list-wrapper{align-items:center;display:flex;gap:10px;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396919, 396979)；UTF-16 [396919, 396979)。

```css
.create-snaptap{display:flex;flex-direction:column;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [396979, 397020)；UTF-16 [396979, 397020)。

```css
.create-snaptap-header{width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397020, 397160)；UTF-16 [397020, 397160)。

```css
.create-snaptap-header-title{background-color:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:6px 16px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397160, 397242)；UTF-16 [397160, 397242)。

```css
.create-snaptap-header-title.disable{background-color:#30961f;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397242, 397297)；UTF-16 [397242, 397297)。

```css
.create-snaptap.disable{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397297, 397366)；UTF-16 [397297, 397366)。

```css
.create-snaptap-message{display:block;font-size:14px;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397366, 397413)；UTF-16 [397366, 397413)。

```css
.create-snaptap-message--warning{color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397413, 397457)；UTF-16 [397413, 397457)。

```css
.create-snaptap-message--success{color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397457, 397506)；UTF-16 [397457, 397506)。

```css
.create-snaptap-message--introduction{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [397506, 397587)；UTF-16 [397506, 397587)。

```css
.widget .titleRow .shortcuts .shortcutButton-snaptap{padding:5px 10px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [398403, 398464)；UTF-16 [398403, 398464)。

```css
.key-record,.key-record-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [398464, 398590)；UTF-16 [398464, 398590)。

```css
.key-record-item{border:2px solid #ccc;border-radius:4px;box-sizing:initial;height:40px;justify-content:center;min-width:60px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [398590, 398636)；UTF-16 [398590, 398636)。

```css
.key-record-item-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [398636, 398689)；UTF-16 [398636, 398689)。

```css
.key-record-item-editing-active{border-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [398689, 398743)；UTF-16 [398689, 398743)。

```css
.key-record-item-editing-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [398743, 398921)；UTF-16 [398743, 398921)。

```css
.key-record-item-editing div{text-wrap:nowrap;align-items:center;display:flex;font-size:11px;height:25px;justify-content:center;margin:auto 10px;min-width:38px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [398921, 399015)；UTF-16 [398921, 399015)。

```css
.key-record-item-assignment{align-items:center;color:#fff;display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [399015, 399107)；UTF-16 [399015, 399107)。

```css
.key-record-item-assignment-active{background-color:#44d62c;border-color:#44d62c;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [399107, 399195)；UTF-16 [399107, 399195)。

```css
.key-record-item-assignment-deactive{background-color:#888;border-color:#888;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [399195, 399277)；UTF-16 [399195, 399277)。

```css
.key-record-item-assignment div{font-size:11px;margin:auto 10px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [399277, 399335)；UTF-16 [399277, 399335)。

```css
.blink-active{animation:blinker-active 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [399335, 399395)；UTF-16 [399335, 399395)。

```css
.blink-warning{animation:blinker-warning 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [399682, 399754)；UTF-16 [399682, 399754)。

```css
.blink-active-border{animation:blinker-active-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [399754, 399828)；UTF-16 [399754, 399828)。

```css
.blink-warning-border{animation:blinker-warning-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400071, 400108)；UTF-16 [400071, 400108)。

```css
.snaptap-shortcuts{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400108, 400271)；UTF-16 [400108, 400271)。

```css
.snaptap-shortcuts .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400271, 400376)；UTF-16 [400271, 400376)。

```css
.snaptap-shortcuts .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400376, 400441)；UTF-16 [400376, 400441)。

```css
.snaptap-shortcuts .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400441, 400504)；UTF-16 [400441, 400504)。

```css
.snaptap-shortcuts-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400504, 400651)；UTF-16 [400504, 400651)。

```css
.snap-tap-add-button-v3{color:#ccc;margin-top:10px!important;margin:auto;padding:6px;text-align:center;text-decoration:underline;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400651, 400695)；UTF-16 [400651, 400695)。

```css
.snap-tap-add-button-v3:hover{color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400695, 400767)；UTF-16 [400695, 400767)。

```css
.snap-tap-key-list-v3{background:#1f1f1f;border-radius:5px;padding:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400767, 400951)；UTF-16 [400767, 400951)。

```css
.snap-tap-key-list-v3 .title-snap-tap{align-items:center;display:flex;font-size:10px;justify-content:center;line-height:12px;text-align:center;white-space:normal;word-break:break-word}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [400951, 401112)；UTF-16 [400951, 401112)。

```css
.key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:initial;display:flex;height:30px;justify-content:center;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [401112, 401225)；UTF-16 [401112, 401225)。

```css
.key-record-item-v3-text{border-radius:3px;margin:0 2px!important;min-width:0!important;padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [401225, 401280)；UTF-16 [401225, 401280)。

```css
.snap-tap-widget .titleRow{position:relative;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [401280, 401347)；UTF-16 [401280, 401347)。

```css
.snaptap-shortcuts-overlay:hover+.tip{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [401497, 401590)；UTF-16 [401497, 401590)。

```css
.single-key-snap-tap-key-list{display:flex;flex-direction:column;min-height:72px;width:160px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [401590, 401636)；UTF-16 [401590, 401636)。

```css
.single-key-snap-tap-to-left{margin-left:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [401636, 401688)；UTF-16 [401636, 401688)。

```css
.single-key-snap-tap-desc{display:block;width:530px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [401688, 401803)；UTF-16 [401688, 401803)。

```css
.key-record-item-v3-skst-text{border-radius:3px;min-width:0!important;padding:0 10px!important;text-transform:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [402133, 402312)；UTF-16 [402133, 402312)。

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [402312, 402368)；UTF-16 [402312, 402368)。

```css
.skst-key-record-item-v3:hover{border:1px solid #9b9b9b}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [402368, 402539)；UTF-16 [402368, 402539)。

```css
.skst-key-record-item-v3-warning{align-items:center;border:1px solid #fd8611;border-radius:4px;color:#fd8611;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [402539, 402658)；UTF-16 [402539, 402658)。

```css
.snap-tap-pair{grid-gap:20px 30px;align-items:stretch;display:grid;gap:20px 30px;grid-template-columns:repeat(2,160px)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/716/ui/static/css/main.40fff12f.css` SHA-256 `d9660443d2d7810f4fa7dc9ef1a81342312b04702cca505c0f0e68b225e2bed0`

字符 [402658, 402716)；UTF-16 [402658, 402716)。

```css
.single-key-snap-tap-key-list>:last-child{margin-top:auto}
```
