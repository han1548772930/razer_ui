# PID 659 — TAB_CUSTOMIZE — Snap Tap

本文件从当前官方原文件静态提取；未执行官方 JS。仅覆盖此界面的 Snap Tap 区域，不代表整个 Customize 页完成。

分支：`ordinary`；列：`right`；快捷键：`False`。偏移单位为 Unicode 字符，另存 UTF-16 偏移。

## 交互与缺口

普通分支：默认 A/D、标题开关、最多四对、首对不可删除、按键释放录入、重复/禁用键警告、成功消息三秒、窗口失焦及离页清理、调整模式警告。原服务输入捕获/映射恢复、真实配置写回和观察尚未连接，不能将本地草稿视作设备保存。

以下 JavaScript 含原 JSX 编译后的 HTML 元素结构和事件处理；CSS 为原规则完整文本。共享依赖、native 调用与源码未提取分支仍是明确缺口。

## 实际挂载与条件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/js/main.82fcdd62.js` SHA-256 `f71ec3026586ab2968bd719dcca140db55edb86a82c9adc0f1a00a6495e1ae8c`

字符 [7388386, 7389136)；UTF-16 [7388386, 7389136)。

```javascript
ildren:(0,Pa.jsx)(Qa.A,{text:Va.LUv})})]}),(0,Pa.jsx)("div",{children:(0,Pa.jsxs)("div",{style:{position:"relative"},children:[(0,Pa.jsx)("div",{className:"help"}),(0,Pa.jsx)("div",{style:{width:362},className:"tip hypershift-mode-tip",children:(0,Pa.jsx)(Qa.A,{text:Va.Dq6})})]})})]}),"macro"!==this.props.displayMode?(0,Pa.jsxs)(uE,{children:[(0,Pa.jsx)(bi,{direction:"left",children:(0,Pa.jsx)(Ro,{})}),(0,Pa.jsxs)(bi,{direction:"right",children:[(0,Pa.jsx)(Os,{}),(0,Pa.jsx)(qo,{})]})]}):null]})}}const Ss=(0,H.connect)(e=>({buttonList:e.customizeReducer.buttonList,activeButton:e.customizeReducer.activeButton,isPanelOpen:e.customizeReducer.isPanelOpen,isMappingOpen:e.customizeReducer.isMappingOpen,isMappingChanged:e.customizeReducer.isMapping
```

## 标题、开关、帮助与快捷键

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/js/main.82fcdd62.js` SHA-256 `f71ec3026586ab2968bd719dcca140db55edb86a82c9adc0f1a00a6495e1ae8c`

字符 [7380327, 7381309)；UTF-16 [7380327, 7381309)。

```javascript
Os=e=>{let t=e.supportsShortcut;const a=(0,H.useDispatch)(),n=(0,H.useSelector)(e=>e.snapTapReducer.keyList),E=(0,H.useSelector)(e=>e.snapTapReducer.isEnabled),o=(0,H.useSelector)(e=>e.customizeReducer.adjustmentModeRunning),i=(0,H.useSelector)(e=>e.customizeReducer.buttonList),s=(0,P.useMemo)(()=>{if(!i||!i.length)return!1;const e=["DKM_F6","DKM_D2"];return i.some(t=>e.includes(t.inputID))},[i]);return(0,Pa.jsxs)(DE,{title:Va.Fqe,tips:s?Va.qI5:Va.f_m,hasSwitch:!0,toggleSwitch:()=>{o?a((0,un.onShowAdjustmentModePrompt)(!0)):(E&&xE(),"SYSTEM"===ee.DeviceInfo.category&&(0,k.JB)({enabled:!E,snapTapPairs:E?[]:n,label:ns.xt.ENABLE}),a((0,jE.vU)(!E)))},active:E,supportsShortcut:t,renderShortcut:(0,Pa.jsx)(cs,{keyCombinations:["FN","L SHIFT"],tip:Va.cyD}),extraClass:"snap-tap-widget",children:[(0,Pa.jsx)("p",{className:"snap-tap-desc",children:(0,Pa.jsx)(Qa.A,{text:Va.PVl})}),(0,Pa.jsx)("section",{className:"snap-tap ".concat(E?"":"disabled"),children:(0,Pa.jsx)(As,{})})]})}
```

## 完整录入编辑组件

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/js/main.82fcdd62.js` SHA-256 `f71ec3026586ab2968bd719dcca140db55edb86a82c9adc0f1a00a6495e1ae8c`

字符 [7374109, 7379859)；UTF-16 [7374109, 7379859)。

```javascript
As=()=>{const e=(0,H.useDispatch)(),t=(0,H.useSelector)(e=>e.snapTapReducer),a=t.isEnabled,n=t.keyList,E=(0,H.useSelector)(e=>{var t;return null!==(t=e.keymapReducer)&&void 0!==t?t:[]}).directionMode,o=(0,H.useSelector)(e=>e.profileReducer.selectedProfileGuid),i=(0,H.useSelector)(e=>e.deviceReducer),s=i.isBle,_=i.isDongle,r=(0,P.useState)(!1),I=(0,Ma.A)(r,2),T=I[0],A=I[1],c=(0,P.useState)(no.READY),O=(0,Ma.A)(c,2),l=O[0],S=O[1],d=(0,P.useState)(es.INTRODUCTION),u=(0,Ma.A)(d,2),R=u[0],N=u[1],D=(0,P.useState)(-1),C=(0,Ma.A)(D,2),L=C[0],p=C[1],M=(0,P.useState)(n),U=(0,Ma.A)(M,2),h=U[0],g=U[1],G=(0,P.useRef)(null),y=(0,P.useRef)(!1),F=(0,P.useRef)(null),v=(0,P.useRef)(null),B=(0,P.useRef)(null);(0,P.useEffect)(()=>{y.current=!1,N(es.INTRODUCTION),p(-1),S(no.READY)},[o]),(0,P.useEffect)(()=>{g(n),V(n)},[JSON.stringify(n)]),(0,P.useEffect)(()=>{var t,a;const E=null!==(t=null===n||void 0===n?void 0:n.filter(e=>{let t=e.key1,a=e.key2;return t&&a}))&&void 0!==t?t:[];E.length!==(null!==(a=null===n||void 0===n?void 0:n.length)&&void 0!==a?a:0)&&e((0,jE.gr)(E))},[]);const V=e=>{"SYSTEM"===ee.DeviceInfo.category&&(0,k.JB)({enabled:a,snapTapPairs:e,label:ns.xt.CHANGE})},b=()=>{document.removeEventListener("mousedown",Y),window.removeEventListener("keyup",x,!0),window.removeEventListener("keydown",X,!0),window.removeEventListener("blur",z),lt.A.off("inputredirect",w)};(0,P.useEffect)(()=>(a?(window.addEventListener("blur",z),window.addEventListener("keyup",x,!0),window.addEventListener("keydown",X,!0),document.addEventListener("mousedown",Y),lt.A.on("inputredirect",w)):K(),()=>{b()}),[a,l,JSON.stringify(h)]);const K=()=>{b(),R!==es.EMPTY_KEY_WARNING&&R!==es.DUPLICATE_KEY_WARNING||J(L)},Y=t=>{if(l!==no.READY&&v.current instanceof Node&&t.target instanceof Node&&v.current&&!v.current.contains(t.target)&&B.current instanceof Node&&B.current&&!B.current.contains(t.target)){if(h.some(e=>!e.key1||!e.key2))return void N(es.DUPLICATE_KEY_WARNING);if(os.includes(R))return;e((0,jE.gr)(h)),Z()}};(0,P.useEffect)(()=>{const t=()=>{let t=[...n];if(l===no.KEY2){const e=h.find(e=>e.id===L);e&&(t=t.map(t=>t.id===L?(0,m.A)((0,m.A)({},t),{},{key1:e.key1}):t))}const a=t.filter(e=>e.key1&&e.key2);e((0,jE.gr)(a)),Z()};return window.addEventListener("beforeunload",t),()=>{window.removeEventListener("beforeunload",t)}},[JSON.stringify(h),R]);const W=(0,P.useMemo)(()=>({onInputRedirectEvent:e=>{if(l===no.READY)return;const t=JSON.parse(e.input);if(!ss.includes(t.type))return;let a=UE.Rk.find(e=>{const a="number"===typeof e.outputFlag?e.outputFlag:e.flag;return t.scancode===e.scancode&&(a===t.flag||a+1===t.flag)});a||"razerKey"!==t.type||(a=(null!==Es&&void 0!==Es?Es:bE).find(e=>e.key===t.key.toString())),a&&(e=>e.flag%2===1)(t)&&Q(a)},onWindowBlur:()=>{let t=h.filter(e=>{let t=e.id,a=e.key1,n=e.key2;return!(os.includes(R)&&t===L)&&a&&n});if(1===h.length&&0===t.length)return g(n),V(n),N(es.INTRODUCTION),void Z();e((0,jE.gr)(t)),N(es.INTRODUCTION),Z()}}),[l,JSON.stringify(h),L]),w=W.onInputRedirectEvent,z=W.onWindowBlur,x=e=>{if(e.preventDefault(),!0===e.metaKey||"Meta"===e.key)return;const t=kE(e);t&&Q(t)},X=e=>{!0!==e.metaKey&&"Meta"!==e.key||e.stopImmediatePropagation()},j=()=>{y.current=!1,clearTimeout(G.current),lt.A.disableMapping(),da.A.callElectronAction({action:"registerNoBrowserInputHandler"}),lt.A.on("inputredirect",w),zE(s,_)},Z=()=>{y.current=!1,S(no.READY),da.A.callElectronAction({action:"unRegisterNoBrowserInputHandler"}),lt.A.enableMapping(),xE(s,_)},q=(0,P.useRef)(Z);q.current=Z,(0,P.useEffect)(()=>()=>{q.current()},[]);const Q=t=>{const a=t.inputID;if(((e,t)=>e&&"KEY_NUMPAD_NUM_LOCK"===t)(y.current,a))y.current=!1;else if(y.current="KEY_PAUSE"===a,!rs.includes(t.inputID)||"KEYPAD"!==ee.DeviceInfo.category||E!==f.mZI.DIRECTIONAL)switch(l){case no.KEY1:let t=structuredClone(h);if(t=h.map(e=>{if(e.id===L){return(0,m.A)((0,m.A)({},e),{},{key1:a})}return e}),N(es.INTRODUCTION),((e,t)=>{const a=t.reduce((t,a)=>t+Object.values(a).filter(t=>t===e).length,0);return a>1})(a,t))return void N(es.DUPLICATE_KEY_WARNING);if(Is(a,t))return void N(es.DUPLICATE_KEY_WARNING);g(t),V(t),S(no.KEY2);break;case no.KEY2:let n=structuredClone(h);if(n=h.map(e=>{if(e.id===L){return(0,m.A)((0,m.A)({},e),{},{key2:a})}return e}),Is(a,n))return void N(es.DUPLICATE_KEY_WARNING);g(n),V(n),e((0,jE.gr)(n)),Z(),N(es.SUCCESS),G.current=setTimeout(()=>{N(es.INTRODUCTION)},3e3)}},J=t=>{t===L?(p(-1),N(es.INTRODUCTION),Z()):t<L&&p(e=>e-1);let a=h.filter(e=>e.id!==t).map((e,t)=>(0,m.A)((0,m.A)({},e),{},{id:t+1})),E=n.find(e=>e.id===t);h.some(e=>!e.key1||!e.key2)||E.key1===E.key2||R!==es.EMPTY_KEY_WARNING&&R!==es.DUPLICATE_KEY_WARNING||(a=n,g(n),V(n)),e((0,jE.gr)(a))},$=(e,t)=>{l===no.READY&&(S(t),p(e),j())};return(0,Pa.jsxs)("div",{children:[(0,Pa.jsxs)("div",{className:"snap-tap-wrapper",children:[(0,Pa.jsx)("div",{className:"snap-tap-key-list",ref:v,children:(0,Pa.jsx)("div",{className:"snap-tap-group",children:null===h||void 0===h?void 0:h.map(e=>(0,Pa.jsx)(ts,{keyGroup:e,recordingState:l,onDelete:()=>J(e.id),onEdit:$,isEditing:e.id===L,isDuplicateKeyWarning:R===es.DUPLICATE_KEY_WARNING&&e.id===L,messageStatus:R},e.id))})}),(0,Pa.jsxs)("div",{className:"snap-tap-add-button ".concat(h.length>=4||l!==no.READY?"disabled":""),onClick:()=>{const t=[...n,{key1:"",key2:"",id:n.length+1}];S(no.KEY1),p(t.length),e((0,jE.xg)(t)),j()},ref:B,children:[(0,Pa.jsx)("div",{className:T?"showTooltip":"hideTooltip",ref:F,children:(0,Pa.jsx)("p",{children:(0,Pa.jsx)(Qa.A,{text:Va.Yjp})})}),(0,Pa.jsx)("div",{className:"snap-tap-add-button-overlay",onMouseEnter:()=>A(!0),onMouseLeave:()=>A(!1),onMouseMove:e=>{F.current.style.top=e.clientY+20+"px",F.current.style.left=e.clientX+10+"px"}}),"+"]})]}),(0,Pa.jsx)(Ts,{status:R})]})}
```

## 按键对行

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/js/main.82fcdd62.js` SHA-256 `f71ec3026586ab2968bd719dcca140db55edb86a82c9adc0f1a00a6495e1ae8c`

字符 [7372521, 7373284)；UTF-16 [7372521, 7373284)。

```javascript
ts=e=>{let t=e.onDelete,a=e.keyGroup,n=e.recordingState,E=e.isDuplicateKeyWarning,o=e.onEdit,i=e.messageStatus,s=e.isEditing;const _=(0,H.useSelector)(e=>e.deviceReducer.layoutId);return(0,Pa.jsxs)("div",{className:"snap-tap-key-list-wrapper",children:[(0,Pa.jsx)(QE,{isRecording:s&&n===no.KEY1,idleState:"READY",name:KE(a.key1)?YE(a.key1):(0,eo.A)(_,a.key1,"KEYBOARD"),isWarning:s&&(n===no.KEY1&&E||n===no.KEY1&&i===es.EMPTY_KEY_WARNING),onEdit:()=>o(a.id,no.KEY1)}),(0,Pa.jsx)(QE,{isRecording:s&&n===no.KEY2,idleState:"READY",name:KE(a.key2)?YE(a.key2):(0,eo.A)(_,a.key2,"KEYBOARD"),isWarning:s&&(n===no.KEY2&&E||n===no.KEY2&&i===es.EMPTY_KEY_WARNING),onEdit:()=>o(a.id,no.KEY2)}),1!==a.id?(0,Pa.jsx)("div",{className:"snap-tap-icon-delete",onClick:t}):null]})}
```

## CSS

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [17636, 17741)；UTF-16 [17636, 17741)。

```css
.key-snap-tap{background-image:url(../../static/media/snap_tap_icon.fdbdd616.svg);height:16px;width:16px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [17741, 17824)；UTF-16 [17741, 17824)。

```css
.key-dks,.key-snap-tap{background-size:cover;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [75532, 75822)；UTF-16 [75532, 75822)。

```css
.factory-default .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [75822, 75891)；UTF-16 [75822, 75891)。

```css
.factory-default .warning-alert .title{color:#ccc;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [75891, 76180)；UTF-16 [75891, 76180)。

```css
.mode-switcher .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [76180, 76263)；UTF-16 [76180, 76263)。

```css
.mode-switcher .warning-alert .title{color:#fd8611;display:flex;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [76263, 76322)；UTF-16 [76263, 76322)。

```css
.mode-switcher .warning-alert .title span{margin-right:4px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [76322, 76382)；UTF-16 [76322, 76382)。

```css
.mode-switcher .warning-alert .content{white-space:pre-wrap}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [84029, 84092)；UTF-16 [84029, 84092)。

```css
.disabled .check-box:hover{border-color:#737373;cursor:default}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [114861, 114902)；UTF-16 [114861, 114902)。

```css
.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [120300, 120544)；UTF-16 [120300, 120544)。

```css
.remove-alert,.warning-alert{background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [120544, 120771)；UTF-16 [120544, 120771)。

```css
.remove-alert .title,.warning-alert .title{align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [120771, 120981)；UTF-16 [120771, 120981)。

```css
.remove-alert .title .icon,.warning-alert .title .icon{background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [120981, 121104)；UTF-16 [120981, 121104)。

```css
.remove-alert .body,.warning-alert .body{color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [121104, 121194)；UTF-16 [121104, 121194)。

```css
.remove-alert .action,.warning-alert .action{display:flex;gap:20px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [121194, 121571)；UTF-16 [121194, 121571)。

```css
.remove-alert .action .ok-button,.remove-alert .action .secondary-button,.warning-alert .action .ok-button,.warning-alert .action .secondary-button{align-items:center;background-color:#707070;border-radius:3px;color:#ccc;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [121571, 121878)；UTF-16 [121571, 121878)。

```css
.remove-alert .action .enable-button,.warning-alert .action .enable-button{align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [328790, 328842)；UTF-16 [328790, 328842)。

```css
.combined-key-blink-active{padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [328842, 328894)；UTF-16 [328842, 328894)。

```css
.body-widgets .widget>div.snap-tap{will-change:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [328894, 328920)；UTF-16 [328894, 328920)。

```css
.snap-tap{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [328920, 328971)；UTF-16 [328920, 328971)。

```css
.snap-tap .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [328971, 329031)；UTF-16 [328971, 329031)。

```css
.snap-tap-group{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329031, 329078)；UTF-16 [329031, 329078)。

```css
.snap-tap-desc{margin-bottom:20px;margin-top:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329078, 329261)；UTF-16 [329078, 329261)。

```css
.snap-tap-add-button{align-items:center;border:2px solid #ccc;border-radius:5px;color:#ccc;display:flex;font-size:20px;height:44px;justify-content:center;position:relative;width:64px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329261, 329324)；UTF-16 [329261, 329324)。

```css
.snap-tap-add-button .disabled{opacity:30%;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329324, 329386)；UTF-16 [329324, 329386)。

```css
.snap-tap-add-button:hover{border-color:#44d62c;color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329386, 329539)；UTF-16 [329386, 329539)。

```css
.snap-tap-add-button .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329539, 329646)；UTF-16 [329539, 329646)。

```css
.snap-tap-add-button .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329646, 329713)；UTF-16 [329646, 329713)。

```css
.snap-tap-add-button .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329713, 329778)；UTF-16 [329713, 329778)。

```css
.snap-tap-add-button-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329778, 329956)；UTF-16 [329778, 329956)。

```css
.snap-tap-icon-delete{background-image:url(../../static/media/icon_delete.de9b7746.svg);background-repeat:no-repeat;background-size:cover;height:20px;margin-left:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [329956, 330055)；UTF-16 [329956, 330055)。

```css
.snap-tap-icon-delete:hover{background-image:url(../../static/media/icon_delete_snap.c1abb283.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330055, 330116)；UTF-16 [330055, 330116)。

```css
.snap-tap-wrapper{display:flex;justify-content:space-between}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330116, 330179)；UTF-16 [330116, 330179)。

```css
.snap-tap-key-list{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330179, 330266)；UTF-16 [330179, 330266)。

```css
.snap-tap-key-list-wrapper{align-items:center;display:flex;gap:10px;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330266, 330326)；UTF-16 [330266, 330326)。

```css
.create-snaptap{display:flex;flex-direction:column;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330326, 330393)；UTF-16 [330326, 330393)。

```css
.create-snaptap-header{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330393, 330533)；UTF-16 [330393, 330533)。

```css
.create-snaptap-header-title{background-color:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:6px 16px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330533, 330615)；UTF-16 [330533, 330615)。

```css
.create-snaptap-header-title.disable{background-color:#30961f;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330615, 330670)；UTF-16 [330615, 330670)。

```css
.create-snaptap.disable{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330670, 330739)；UTF-16 [330670, 330739)。

```css
.create-snaptap-message{display:block;font-size:14px;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330739, 330786)；UTF-16 [330739, 330786)。

```css
.create-snaptap-message--warning{color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330786, 330830)；UTF-16 [330786, 330830)。

```css
.create-snaptap-message--success{color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330830, 330879)；UTF-16 [330830, 330879)。

```css
.create-snaptap-message--introduction{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [330879, 330960)；UTF-16 [330879, 330960)。

```css
.widget .titleRow .shortcuts .shortcutButton-snaptap{padding:5px 10px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [331828, 331889)；UTF-16 [331828, 331889)。

```css
.key-record,.key-record-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [331889, 332015)；UTF-16 [331889, 332015)。

```css
.key-record-item{border:2px solid #ccc;border-radius:4px;box-sizing:initial;height:40px;justify-content:center;min-width:60px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332015, 332061)；UTF-16 [332015, 332061)。

```css
.key-record-item-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332061, 332114)；UTF-16 [332061, 332114)。

```css
.key-record-item-editing-active{border-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332114, 332168)；UTF-16 [332114, 332168)。

```css
.key-record-item-editing-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332168, 332346)；UTF-16 [332168, 332346)。

```css
.key-record-item-editing div{text-wrap:nowrap;align-items:center;display:flex;font-size:11px;height:25px;justify-content:center;margin:auto 10px;min-width:38px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332346, 332440)；UTF-16 [332346, 332440)。

```css
.key-record-item-assignment{align-items:center;color:#fff;display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332440, 332532)；UTF-16 [332440, 332532)。

```css
.key-record-item-assignment-active{background-color:#44d62c;border-color:#44d62c;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332532, 332620)；UTF-16 [332532, 332620)。

```css
.key-record-item-assignment-deactive{background-color:#888;border-color:#888;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332620, 332702)；UTF-16 [332620, 332702)。

```css
.key-record-item-assignment div{font-size:11px;margin:auto 10px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332702, 332760)；UTF-16 [332702, 332760)。

```css
.blink-active{animation:blinker-active 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [332760, 332820)；UTF-16 [332760, 332820)。

```css
.blink-warning{animation:blinker-warning 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333107, 333179)；UTF-16 [333107, 333179)。

```css
.blink-active-border{animation:blinker-active-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333179, 333253)；UTF-16 [333179, 333253)。

```css
.blink-warning-border{animation:blinker-warning-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333496, 333533)；UTF-16 [333496, 333533)。

```css
.snaptap-shortcuts{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333533, 333696)；UTF-16 [333533, 333696)。

```css
.snaptap-shortcuts .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333696, 333801)；UTF-16 [333696, 333801)。

```css
.snaptap-shortcuts .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333801, 333866)；UTF-16 [333801, 333866)。

```css
.snaptap-shortcuts .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333866, 333929)；UTF-16 [333866, 333929)。

```css
.snaptap-shortcuts-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [333929, 334102)；UTF-16 [333929, 334102)。

```css
.snap-tap-add-button-v3{color:#ccc;margin-top:10px!important;margin:auto;padding:6px;text-align:center;text-decoration:underline;width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334102, 334146)；UTF-16 [334102, 334146)。

```css
.snap-tap-add-button-v3:hover{color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334146, 334218)；UTF-16 [334146, 334218)。

```css
.snap-tap-key-list-v3{background:#1f1f1f;border-radius:5px;padding:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334218, 334402)；UTF-16 [334218, 334402)。

```css
.snap-tap-key-list-v3 .title-snap-tap{align-items:center;display:flex;font-size:10px;justify-content:center;line-height:12px;text-align:center;white-space:normal;word-break:break-word}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334402, 334563)；UTF-16 [334402, 334563)。

```css
.key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:initial;display:flex;height:30px;justify-content:center;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334563, 334676)；UTF-16 [334563, 334676)。

```css
.key-record-item-v3-text{border-radius:3px;margin:0 2px!important;min-width:0!important;padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334676, 334731)；UTF-16 [334676, 334731)。

```css
.snap-tap-widget .titleRow{position:relative;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334731, 334798)；UTF-16 [334731, 334798)。

```css
.snaptap-shortcuts-overlay:hover+.tip{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [334948, 335041)；UTF-16 [334948, 335041)。

```css
.single-key-snap-tap-key-list{display:flex;flex-direction:column;min-height:72px;width:160px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [335041, 335087)；UTF-16 [335041, 335087)。

```css
.single-key-snap-tap-to-left{margin-left:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [335087, 335139)；UTF-16 [335087, 335139)。

```css
.single-key-snap-tap-desc{display:block;width:530px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [335139, 335254)；UTF-16 [335139, 335254)。

```css
.key-record-item-v3-skst-text{border-radius:3px;min-width:0!important;padding:0 10px!important;text-transform:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [335584, 335763)；UTF-16 [335584, 335763)。

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [335763, 335819)；UTF-16 [335763, 335819)。

```css
.skst-key-record-item-v3:hover{border:1px solid #9b9b9b}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [335819, 335990)；UTF-16 [335819, 335990)。

```css
.skst-key-record-item-v3-warning{align-items:center;border:1px solid #fd8611;border-radius:4px;color:#fd8611;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [335990, 336109)；UTF-16 [335990, 336109)。

```css
.snap-tap-pair{grid-gap:20px 30px;align-items:stretch;display:grid;gap:20px 30px;grid-template-columns:repeat(2,160px)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/659/ui/static/css/main.09025276.css` SHA-256 `5b36468ce09bb44460277fa18bad52f9bda05ca2821f0a19366ad89f9b581a14`

字符 [336109, 336167)；UTF-16 [336109, 336167)。

```css
.single-key-snap-tap-key-list>:last-child{margin-top:auto}
```
