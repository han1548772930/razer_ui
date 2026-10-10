# 2676 calibration — current source, 2026-10-10

Independent per-thumbstick popup. Source acquisition, static semantics, Rust presentation, state/event seam and runtime acceptance are separate. This page is not marked fully complete.

- JavaScript: `local-ui-reverse/source/official/apps.razer.com/synapse/products/2676/ui/static/js/main.5ee115d4.js`
- SHA-256: `624631c29708f83d15d2b8f754a965439eab38ba3ae8be951d1337154513f377`
- Mounted initializer: `hm`, UTF-16 offsets 6783178..6790506.
- Rust: `crates/razer-pages/src/features/gamepad_calibration_popup.rs`; state: `gamepad_calibration_state.rs`.
- Complete AST source and CSS rule offsets: [receipt](gamepad-2676-calibration-live-source.json).
- Complete mounted callers, parent widget and dispatchers: [caller receipt](gamepad-2676-calibration-callers-live-source.json).

## Mounted HTML / JavaScript logic

The JSX below is the original complete component initializer, not an invented HTML template. Element tags, props, classes, conditional branches and source action handlers are retained verbatim.

```javascript
e=>{let{selectedThumbstick:t,open:n,onClose:i,isSvg:o=!1}=e;const r=(0,G.useDispatch)(),s=(0,tL.A)("synapse",window.name),E=(0,G.useSelector)(e=>e.deviceReducer.productId),_=(0,G.useSelector)(e=>e.deviceReducer.editionId),T=(0,G.useSelector)(e=>e.customizeReducer.gamepadTester),l=(0,G.useSelector)(e=>e.controllerCalibrationReducer.step),I=(0,G.useSelector)(e=>e.controllerCalibrationReducer.partId),A=(0,G.useSelector)(e=>e.controllerCalibrationReducer.isStepValid),d=re.DeviceInfo.isPlaystationController,[c,O]=(0,M.useState)(null),[S,u]=(0,M.useState)(""),[R,N]=(0,M.useState)(!1),[D,C]=(0,M.useState)(0),[L,m]=(0,M.useState)({x:0,y:0}),p=(0,M.useRef)(!1),P=(0,M.useRef)(!1),h=(0,M.useRef)(!1),g=I||t,U=g===Dt.CONTROLLER_THUMBSTICKS_ENUM.THUMBSTICK_RIGHT?"right":"left",f=(0,M.useCallback)(()=>{p.current||(p.current=!0,i())},[i]);(0,M.useEffect)(()=>{(async()=>{const e=o?"svg_prods":"img_prods",t=o?"Calibration.svg":"0-3x.png";O(null),u(""),N(!1);try{const n=await a(21353)(`./${E}_${_}/${e}/${t}`);if(o){const e=await fetch(n.default);if(!e.ok)throw new Error("Failed to get SVG");u(await e.text())}else O(n.default)}catch{try{const n=await a(46675)(`./${E}_0/${e}/${t}`);if(o){const e=await fetch(n.default);if(!e.ok)throw new Error("Failed to get SVG");u(await e.text())}else O(n.default)}catch(n){console.log(n),N(!0)}}})()},[_,o,E]),(0,M.useEffect)(()=>{n&&(p.current=!1,P.current=!1,h.current=!1)},[n,t]),(0,M.useEffect)(()=>{n&&t&&(C(0),m({x:0,y:0}),h.current=!1,r(Pm(Dt.CALIBRATION_EVENT.START,t)))},[r,n,t]),(0,M.useEffect)(()=>{n&&(s?P.current=!0:P.current&&f())},[s,n,f,t]),(0,M.useEffect)(()=>{if(!n)return;const e=e=>{if(e.key===Dt.CALIBRATION_USER_MOVEMENT&&e.newValue)try{const{x:t,y:a}=JSON.parse(e.newValue);if(!Number.isFinite(t)||!Number.isFinite(a))return;m({x:Math.min(100,Math.max(-100,t)),y:Math.min(100,Math.max(-100,a))})}catch{}};return ti.on("windowstorageevent",e),()=>{ti.off("windowstorageevent",e)}},[n]),(0,M.useEffect)(()=>{l<=1&&(C(0),h.current=!1)},[l,I]),(0,M.useEffect)(()=>{h.current||l===Dt.CALIBRATION_PROGRESS_STEP.Step_5_Rotate&&A&&3===D&&(h.current=!0,r(Pm(Dt.CALIBRATION_EVENT.ROTATE_COMPLETE,g)))},[g,l,r,A,D]);const v=(0,M.useCallback)(()=>{r(Pm(Dt.CALIBRATION_EVENT.START,g)),C(0),h.current=!1},[g,r]),y=(0,M.useCallback)(()=>{switch(l){case 0:return(0,yn.jsx)(vn.A,{text:He.Kr7});case 1:return(0,yn.jsx)(vn.A,{text:He.dqO});case 2:return(0,yn.jsx)(vn.A,{text:He.YHw});case 3:return(0,yn.jsx)(vn.A,{text:He.R5i});case 4:return(0,yn.jsx)(vn.A,{text:He.Kx$});case 5:return(0,yn.jsx)(vn.A,{text:He.gEn});default:return null}},[l]),H=(0,M.useCallback)(()=>{switch(l){case 1:case 2:case 3:case 4:case 5:return(0,yn.jsx)("button",{className:"btnCancel uppercase",onClick:f,children:(0,yn.jsx)(vn.A,{text:He.bOp})});case 6:return(0,yn.jsx)("button",{className:"btnPrimary",onClick:f,children:(0,yn.jsx)(vn.A,{text:He.DHM})});default:return null}},[l,f]),B=[6===l&&g===Dt.CONTROLLER_THUMBSTICKS_ENUM.THUMBSTICK_LEFT?"show-left-thumbstick":"",6===l&&g===Dt.CONTROLLER_THUMBSTICKS_ENUM.THUMBSTICK_RIGHT?"show-right-thumbstick":"",l>=1&&l<=4&&A?"show-left-bumper":""].filter(Boolean).join(" "),F=l<1?He.YmC:6===l?He.Jvl:null,w=y(),b=H();return-1===l?(0,yn.jsxs)("div",{className:`thumbstick-calibration-popup thumbstick-calibration-popup--${U} thumbstick-calibration-popup--error`,children:[(0,yn.jsx)("div",{className:"thumbstick-calibration-popup__error-icon",children:(0,yn.jsx)(pm,{})}),(0,yn.jsx)("div",{className:"thumbstick-calibration-popup__error-title",children:(0,yn.jsx)(vn.A,{text:He.IyE})}),(0,yn.jsxs)("div",{className:"thumbstick-calibration-popup__error-actions",children:[(0,yn.jsx)("button",{className:"btnCancel uppercase",onClick:f,children:(0,yn.jsx)(vn.A,{text:He.bOp})}),(0,yn.jsx)("button",{className:"btnPrimary",onClick:v,children:(0,yn.jsx)(vn.A,{text:He.AGU})})]})]}):(0,yn.jsx)("div",{className:`thumbstick-calibration-popup thumbstick-calibration-popup--${U}`,children:(0,yn.jsxs)("div",{className:"calibration-wrapper-v2",children:[(0,yn.jsx)("div",{className:"thumbstick-calibration-popup__steps step-indicator",style:{visibility:l>=1?"visible":"hidden"},children:(0,yn.jsx)(cm,{steps:l+1})}),(0,yn.jsxs)("div",{className:"thumbstick-calibration-popup__copy",children:[F?(0,yn.jsx)("div",{className:"thumbstick-calibration-popup__title calibration-title",children:(0,yn.jsx)(vn.A,{text:F})}):null,w?(0,yn.jsx)("div",{className:"thumbstick-calibration-popup__description calibration-description",children:w}):null]}),(0,yn.jsxs)("div",{className:"thumbstick-calibration-popup__visualization calibration-area",children:[(0,yn.jsx)("div",{className:"thumbstick-simulator",style:{visibility:0===l?"visible":"hidden"},children:(0,yn.jsx)(qL,{thumbStickXData:T.leftThumbX,thumbStickYData:T.leftThumbY})}),(0,yn.jsxs)("div",{className:"prodImage-container",children:[o&&S?(0,yn.jsx)("div",{className:`prodImage-calibration img-nodrag svg-image ${B}`,dangerouslySetInnerHTML:{__html:S}}):!o&&c?(0,yn.jsx)("img",{className:"prodImage-calibration img-nodrag",src:c}):R?null:(0,yn.jsx)("div",{className:"prodImage-calibration img-nodrag placeholder "+(o?"svg-image":"")}),(()=>{const e=10*L.x,t=-10*L.y;switch(l){case 0:return o?null:g===Dt.CONTROLLER_THUMBSTICKS_ENUM.THUMBSTICK_LEFT?(0,yn.jsx)("div",{className:"left-thumbstick-overlay "+(d?"ps-ctrl":""),children:(0,yn.jsx)("svg",{width:38,height:38,viewBox:"-1 -1 38 38",fill:"none",xmlns:"http://www.w3.org/2000/svg",children:(0,yn.jsxs)("g",{children:[(0,yn.jsx)("circle",{cx:"19",cy:"15",r:"15",fill:"#44D62C",fillOpacity:"0.2",shapeRendering:"crispEdges"}),(0,yn.jsx)("circle",{cx:"19",cy:"15",r:"14.5",stroke:"#2EB717",shapeRendering:"crispEdges"})]})})}):(0,yn.jsx)("div",{className:"right-thumbstick-overlay "+(d?"ps-ctrl":""),children:(0,yn.jsx)("svg",{width:38,height:38,viewBox:"-1 -1 38 38",fill:"none",xmlns:"http://www.w3.org/2000/svg",children:(0,yn.jsxs)("g",{children:[(0,yn.jsx)("circle",{cx:"19",cy:"15",r:"15",fill:"#44D62C",fillOpacity:"0.2",shapeRendering:"crispEdges"}),(0,yn.jsx)("circle",{cx:"19",cy:"15",r:"14.5",stroke:"#2EB717",shapeRendering:"crispEdges"})]})})});case 1:case 2:case 3:case 4:case 5:return A&&5!==l?o?null:(0,yn.jsx)("div",{className:"lb-overlay "+(d?"ps-ctrl":""),children:(0,yn.jsx)("svg",{width:"64",height:"29",viewBox:"0 0 64 29",fill:"none",xmlns:"http://www.w3.org/2000/svg",children:(0,yn.jsx)("path",{d:"M18.5709 7.17398C30.422 0.973981 44.4011 0.669753 52.0409 0C56.7366 0 61.6373 4.78395 63.5006 7.17593C63.9665 7.55864 62.0006 8.41699 59.5008 8.41699C54.752 8.41699 42.2503 10.2711 30.0007 14.417C19.793 17.8718 5.71369 24.9703 0.123589 28.4147C0.0304184 27.0752 -0.100014 24.1092 0.123589 22.961C0.347191 21.8129 1.3348 19.995 1.80064 19.2295C2.45282 17.7944 6.71993 13.374 18.5709 7.17398Z",fill:"#44D62C",fillOpacity:"0.3"})})}):(0,yn.jsx)("div",{className:`joystick-direction-overlay-${g} ${d?"ps-ctrl":""}`,children:(0,yn.jsx)(Lm,{thumbStickXData:e,thumbStickYData:t,step:l,rotationCount:D,handleRotationCount:e=>C(e)})});default:return null}})()]}),(0,yn.jsx)("div",{className:"thumbstick-simulator",style:{visibility:0===l?"visible":"hidden"},children:(0,yn.jsx)(qL,{thumbStickXData:T.rightThumbX,thumbStickYData:T.rightThumbY})})]}),b?(0,yn.jsx)("div",{className:"thumbstick-calibration-popup__actions calibration-footer",children:b}):null]})})}
```

## Source CSS

Source `local-ui-reverse/source/official/apps.razer.com/synapse/products/2676/ui/static/css/main.5a8c40cd.css`; SHA-256 `e43575cc38eea25b6b8d2fbfd902cd71a63c0f5beea8969d0721bbeb02c0bdbb`.

```css
.backdrop.show .choose-a-mat { top:100px }
.device_linked_game .popup-widget .game-detail .choose-a-mat { max-height:100vh!important }
.device_linked_game .popup-widget .game-detail .choose-a-mat .scrollable { overflow-y:visible!important }
.device_linked_game .popup-widget .game-detail .choose-a-mat .custom-cover-art .refresh-cover-art { padding-top:60px }
.device_linked_game .popup-widget .game-detail .choose-a-mat .custom-cover-art .tooltip { background-color:#000;border:1px solid #5d5d5d;font-size:14px;margin-left:30px;padding:8px 10px;width:max-content }
@media only screen and (max-width:1284px) { .device_linked_game .popup-widget .game-detail .choose-a-mat { max-width:800px } }
@media only screen and (max-width:1284px) { .device_linked_game .popup-widget .game-detail .choose-a-mat .scrollable { overflow-y:visible!important } }
.calibration-body { margin:auto;max-width:600px }
.iot-device-add .popup-widget { height:100% }
.haptic-content .backdrop .choose-a-mat { width:850px }
.welcome .calibration-welcome { background-color:#2d2d2d;border:1px solid #2d2d2d;border-radius:5px;padding:20px 30px;position:relative }
.welcome .calibration-welcome .close { background-image:url(../../static/media/icon_close_white.8ab462b8.svg);background-position:50%;background-repeat:no-repeat;background-size:20px;height:36px;position:absolute;right:0;top:0;transition:background-color .2s;width:36px;will-change:background-color }
.welcome .calibration-welcome .close:active,.welcome .calibration-welcome .close:hover { background-image:url(../../static/media/icon_close_green.45f61360.svg) }
.welcome .calibration-welcome .close:active { opacity:.7 }
.choose-a-mat { background-color:#222;border-radius:5px 5px 0 0;bottom:0;left:50%;max-height:95vh;max-width:1050px;min-width:800px;position:absolute;top:100%;transform:translate(-50%);transition:top .3s;will-change:top }
.popup-calibration { max-height:calc(100vh - 105px);top:105px!important }
.choose-a-mat .head .close { background-image:url(../../static/media/icon_close.55fe41f1.svg);right:0 }
.choose-a-mat .head .back,.choose-a-mat .head .close { background-color:#0000;background-position:50%;background-repeat:no-repeat;background-size:20px;height:36px;position:absolute;top:0;transition:background-color .2s;width:36px;will-change:background-color }
.choose-a-mat .head .back { background-image:url(../../static/media/icon_back_arrow.b39e4841.svg);left:0 }
.choose-a-mat .head { box-shadow:0 1px 0 0 #5d5d5d;color:#999;font-family:RazerF5;font-size:16px;height:36px;line-height:19px;padding:9px 0 8px;position:relative;text-align:center;text-transform:uppercase;z-index:1 }
.choose-a-mat .head .back:hover,.choose-a-mat .head .close:hover { background-color:#ffffff1a }
.choose-a-mat .head .back:active,.choose-a-mat .head .close:active { background-color:#0000001a }
.choose-a-mat .body.scrollable { max-height:calc(100% - 36px);overflow-y:scroll;padding:20px 0 30px 25px }
.choose-a-mat .body .desc { color:#ccc;font-size:14px;line-height:17px;margin-bottom:10px;text-align:center }
@media screen and (min-width:1400px) { .choose-a-mat { width:1050px } }
@media screen and (max-width:1398px)and (min-width:800px) { .choose-a-mat.popup-calibration { width:800px } }
@media screen and (max-width:799px) { .choose-a-mat.popup-calibration { min-width:546px } }
.choose-a-mat .body .illust,.illust-inverted { height:340px;margin:20px auto;width:500px }
.choose-a-mat .body .step-1 .illust { background-image:url(../../static/media/mouse_calibration_1.f0c7bbe8.svg) }
.choose-a-mat .body .step-1 .illust-inverted { background-image:url(../../static/media/mouse_calibration_1_inverted.defc1125.svg) }
.choose-a-mat .body .step-2 .illust { background-image:url(../../static/media/mouse_calibration_2.518758cc.svg) }
.choose-a-mat .body .step-2 .illust-inverted { background-image:url(../../static/media/mouse_calibration_2_inverted.8ce2cf1f.svg) }
.choose-a-mat .body .complete .illust { background-image:url(../../static/media/mouse_calibration_complete.0fcb4d53.svg) }
.choose-a-mat .body .error .illust { background-image:url(../../static/media/mouse_calibration_error.a227b74c.svg) }
.custom-calibration .custom-hyper-wrapper { margin-left:auto;margin-right:auto;width:fit-content }
.custom-calibration .hyper-wrapper.hyper-on .text.hypershift { background-color:#44d62c }
.tooltip_checkbox_wrapper.calibration_tool_tip { display:flex }
.tooltip_checkbox_wrapper.calibration_tool_tip .checkbox-text { position:relative;top:1px }
.tooltip_checkbox_wrapper.calibration_tool_tip .help { left:5px;position:relative }
.tooltip_checkbox_wrapper.calibration_tool_tip .tip { left:0;right:auto;top:25px }
.choose-a-mat .body .span { animation:blink 2s linear infinite;color:#fff;font-family:cursive;font-size:26px;text-align:center }
.popup-widget .backdrop { height:100%;overflow:hidden;position:fixed }
.popup-widget .backdrop .choose-a-mat { border-radius:5px 5px 0 0;margin-top:110px;max-height:calc(100vh - 110px)!important }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar::-webkit-scrollbar-track { background:#0000 }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar { height:100%;overflow-y:hidden;padding:20px 0 42px 25px }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar .drag-area { padding-bottom:25px }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar .content { height:calc(100% - 10px);overflow-y:scroll }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar .content-addApp { grid-gap:8px;display:grid;gap:8px;grid-template-columns:repeat(3,33%);height:calc(100% - 10px);overflow-x:hidden;overflow-y:scroll;padding:0 32px 20px 0 }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar .content-addApp.no-available-app { align-items:center;color:#ccc;display:flex;flex-direction:column;font-size:14px;justify-content:center;line-height:17px }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar .content-addApp.no-available-app .no-available { margin-bottom:10px;text-align:center }
.popup-widget .backdrop .choose-a-mat #transparent-scrollbar .content-addApp.no-available-app .add-app-description>span { color:#44d62c;cursor:pointer;margin-right:3px;text-decoration:underline }
.popup-widget .backdrop .choose-a-mat .head { padding:20px 0 10px }
.popup-widget .backdrop.no-popup,.popup-widget .backdrop.no-popup .choose-a-mat { transition:none 0s ease 0s;transition:initial;-webkit-transition:unset;will-change:auto }
.popup-widget .app_row { margin-right:15px }
.popup-widget .main-nav { font-size:14px;line-height:17px;margin-bottom:10px;padding:0;width:100% }
.popup-widget .main-nav .action-block { align-items:flex-end;display:flex;justify-content:flex-end;margin-right:15px;text-align:right;width:100% }
.popup-widget .main-nav .action-block input[type=file] { opacity:0;position:absolute }
.popup-widget .main-nav .action-block.browser { align-items:center }
.popup-widget .main-nav .dropdown-block { margin-right:18px }
.popup-widget .main-nav a { color:#fff }
.popup-widget .main-nav a:hover { color:#44d62c }
.popup-widget .main-nav a:active { opacity:.7 }
.popup-widget .main-nav label { margin-right:10px }
.popup-widget .main-nav .filter-nav { display:flex;justify-content:space-between;margin:12px 0 0 10px;width:245px }
.popup-widget .main-nav .filter-nav .added { margin-left:130px;position:relative }
.popup-widget .main-nav .filter-nav .razer-presets { margin-left:15px }
.popup-widget .gradient-fillter:after { background-image:linear-gradient(180deg,#e6e6e661,#222 40%);border-radius:5px 5px 0 0;content:"";height:100%;left:0;position:fixed;top:0;width:100%;z-index:-1 }
.popup-widget .game-profile { display:flex;height:auto;margin-bottom:25px;margin-right:5px;width:100% }
.popup-widget .game-profile .game-info { display:flex;flex-direction:column;font-size:14px;line-height:17px;max-width:calc(100% - 310px);position:relative;text-align:left }
.popup-widget .game-profile .game-info .location { margin-bottom:10px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap }
.popup-widget .game-profile .game-info .missing-game { color:#c8323c;flex:1 1;opacity:0;visibility:hidden }
.popup-widget .game-profile .game-info .missing-game p { margin:0 }
.popup-widget .game-profile .game-info .missing-game.show { opacity:1;visibility:visible }
.popup-widget .game-profile .game-info .remove-button { text-decoration:underline;text-transform:capitalize;width:max-content }
.popup-widget .game-profile .game-info .remove-button:hover { color:#44d62c }
.popup-widget .game-profile .game-info .remove-button:active { opacity:.7 }
.popup-widget .linked-device-list { line-height:17px;text-align:center;width:calc(100% + 8px) }
.popup-widget .linked-device-list .title { font-size:14px;line-height:17px;margin-bottom:10px;text-transform:uppercase }
.popup-widget .list-box { display:flex;flex-wrap:wrap;margin-right:-5px }
.device_to_linked_game .choose-a-mat { min-width:0;width:calc(100vw - 40px) }
@media(min-width:1600px) { .device_to_linked_game .choose-a-mat.linked-games-popup { max-width:none;width:1300px!important } }
.calibration-title { color:#44d62c;font-family:RazerF5,sans-serif;font-size:16px;min-height:22px;text-transform:uppercase }
.calibration-description { font-size:14px;margin:auto;min-height:44px;width:460px }
.calibration-description.ps-ctrl { width:600px }
.calibration-instructions-wrapper { background:#2b2b2b;border-radius:5px;font-size:14px;margin:auto;padding:20px 40px;text-align:left }
.calibration-instructions-wrapper.calibration-title { color:#44d62c }
.calibration-instructions-wrapper .calibration-instructions ol { line-height:1.5;padding-left:20px }
.calibration-wrapper { text-align:center }
.calibration-test-area { display:flex;font-family:Roboto;font-size:14px;font-weight:400;line-height:17px;margin:11px 30px 30px 23px;min-width:850px;text-align:center;-webkit-text-decoration-skip-ink:none;text-decoration-skip-ink:none;text-underline-position:from-font }
.prodImage-calibration { grid-area:1/1/2/2;max-width:372px!important }
@media(max-width:850px) { .calibration-test-area { transform:scale(.8) } }
@media(max-width:650px) { .calibration-test-area { transform:scale(.6) } }
@media(max-width:400px) { .calibration-test-area { transform:scale(.5) } }
.joystick-direction-overlay-1,.joystick-direction-overlay-2,.joystick-rotate-overlay-1,.joystick-rotate-overlay-2,.left-bumper-overlay,.left-thumbstick-overlay,.right-bumper-overlay,.right-thumbstick-overlay { grid-area:1/1/2/2;z-index:2 }
.joystick-direction-overlay-2 { padding-left:110px;padding-top:93px }
.joystick-direction-overlay-2.ps-ctrl { padding-top:75px }
.joystick-direction-overlay-1 { padding-right:205.5px;padding-top:36px }
.joystick-direction-overlay-1.ps-ctrl { padding-right:110px;padding-top:75px }
.calibration-footer .button { background:#0000;border:1px solid #ccc;border-radius:3px;color:#ccc;font-size:12px;height:27px;margin:10px 5px 0;padding:0 14px }
.calibration-footer .button:hover { background:rgba(0,0,0,.929);cursor:pointer }
.calibration-footer .btn { margin:10px 5px }
.calibration-warning-icon { display:inline;margin:0 10px;vertical-align:top }
.thumbstick-simulator { margin:auto auto -38.5px }
.calibration-footer { display:flex;justify-content:center }
.thumbstick-calibration-simulator { height:157px;position:relative }
.thumbstick-calibration-simulator #bg-chart { position:absolute }
.thumbstick-calibration-simulator #chart { position:relative }
.thumbstick-calibration-simulator .rotate-count { color:#fff;font-size:14px;font-weight:400;position:absolute;right:50%;top:100%;transform:translateX(50%) }
.thumbstick-calibration-popup { margin:0 auto;max-width:850px;width:100% }
.thumbstick-calibration-popup .calibration-wrapper-v2 { align-items:center;display:flex;flex-direction:column;margin:0 auto;max-width:601px;min-height:352px;width:601px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__steps { display:flex;height:21px;justify-content:center;margin:0;max-width:100%;width:601px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__steps svg { display:block;height:21px;overflow:visible;width:601px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__copy { align-items:center;display:flex;flex-direction:column;margin-top:50px;max-width:100%;min-height:34px;width:600px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__title { min-height:16px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__description { color:#ccc;line-height:17px;margin:0;max-width:100%;min-height:34px;width:600px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__title+.thumbstick-calibration-popup__description { margin-top:12px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__visualization { justify-content:center;margin:20px 0 0;max-width:100%;min-width:0;transform:none;width:601px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-calibration-popup__actions { margin-top:30px }
.thumbstick-calibration-popup .calibration-wrapper-v2 .prodImage-container { margin:0 auto }
.thumbstick-calibration-popup .calibration-wrapper-v2 .prodImage-container .prodImage-calibration.svg-image.show-left-thumbstick .leftJoystick,.thumbstick-calibration-popup .calibration-wrapper-v2 .prodImage-container .prodImage-calibration.svg-image.show-right-thumbstick .rightJoystick { display:inline }
.thumbstick-calibration-popup .calibration-wrapper-v2 .thumbstick-simulator { display:none }
.thumbstick-calibration-popup--error { padding:24px 0 8px;text-align:center }
.thumbstick-calibration-popup__error-icon { margin-bottom:10px }
.thumbstick-calibration-popup__error-title { color:#fd8611;font-family:RazerF5,sans-serif;font-size:16px;line-height:16px;text-transform:uppercase }
.thumbstick-calibration-popup__error-description { color:#ccc;font-size:14px;line-height:17px;margin:10px auto 0;max-width:420px }
.thumbstick-calibration-popup__error-actions { display:flex;justify-content:center;margin-top:10px }
.thumbstick-calibration-widget { color:#ccc;width:600px }
.thumbstick-calibration-widget--trigger-calibration .thumbstick-calibration-panel { height:422.42px }
.thumbstick-calibration-widget .thumbstick-calibration-panel { box-sizing:border-box;height:480px;min-width:600px;overflow:hidden;padding:30px 40px 20px;width:600px }
.thumbstick-calibration-widget .thumbstick-calibration-panel .titleRow .title { margin-bottom:17px }
.thumbstick-calibration-widget__description { color:#bbb;font-family:Roboto,sans-serif;font-size:14px;line-height:17px;width:520px }
.thumbstick-calibration-widget__product { height:253px;left:114px;position:absolute;top:160px;width:372px }
.thumbstick-calibration-widget__product .widget-prod { background:none;height:253px;margin:0;max-width:none;min-width:0;width:372px }
.thumbstick-calibration-widget__product .widget-prod img { height:253px;object-fit:contain;width:372px }
.thumbstick-calibration-widget__product .dim-corner { display:none }
.thumbstick-calibration-widget__button { align-items:center;background:#111;border:1px solid #ccc;border-radius:3px;box-sizing:border-box;color:#ccc;cursor:pointer;display:flex;font-family:Roboto,sans-serif;font-size:12px;height:auto;justify-content:center;line-height:14px;margin:0;min-height:27px;padding:7px 15px 6px;pointer-events:auto;position:absolute;transition:background-color .2s ease;z-index:1 }
.thumbstick-calibration-widget__button.customize-setting-button { overflow-wrap:anywhere;text-align:center;text-transform:uppercase;white-space:normal }
.thumbstick-calibration-widget__button--left-trigger { left:40px;top:120px;width:111px }
.thumbstick-calibration-widget__button--right-trigger { right:40px;top:120px;width:118px }
.thumbstick-calibration-widget__button--left-thumbstick { bottom:32px;left:40px;width:137px }
.thumbstick-calibration-widget__button--right-thumbstick { bottom:32px;right:40px;width:144px }
.thumbstick-calibration-widget__button--disabled { cursor:default;opacity:.3;pointer-events:none }
.thumbstick-calibration-widget__button:hover { background:#000 }
.thumbstick-calibration-widget__button:active { opacity:1 }
.thumbstick-calibration-widget__connector-overlay { display:block;height:304px;left:151px;overflow:visible;pointer-events:none;position:absolute;top:133.5px;width:292px;z-index:2 }
.thumbstick-calibration-widget__connector-overlay .connector { stroke:#5d5d5d;transition:stroke .2s ease }
.thumbstick-calibration-widget .thumbstick-calibration-widget__button--left-thumbstick:hover~.thumbstick-calibration-widget__connector-overlay .connector--left-thumbstick,.thumbstick-calibration-widget .thumbstick-calibration-widget__button--left-trigger:hover~.thumbstick-calibration-widget__connector-overlay .connector--left-trigger,.thumbstick-calibration-widget .thumbstick-calibration-widget__button--right-thumbstick:hover~.thumbstick-calibration-widget__connector-overlay .connector--right-thumbstick,.thumbstick-calibration-widget .thumbstick-calibration-widget__button--right-trigger:hover~.thumbstick-calibration-widget__connector-overlay .connector--right-trigger { stroke:#fff }
.popup-widget .backdrop.thumbstick-calibration-popup-widget .choose-a-mat.linked-games-popup { height:100vh;margin-top:0;max-height:100vh!important;width:850px }
.popup-widget .backdrop.thumbstick-calibration-popup-widget .choose-a-mat .head { align-items:center;display:flex;justify-content:center;padding:0 36px }
.popup-widget .backdrop.thumbstick-calibration-popup-widget .choose-a-mat .body.scrollable { overflow:visible;padding:20px 25px 30px }
.calibration-wrapper-v2 { text-align:center }
.calibration-wrapper-v2 .calibration-title { color:#44d62c;font-family:RazerF5,sans-serif;font-size:16px;min-height:22px;text-transform:uppercase }
.calibration-wrapper-v2 .step-indicator { margin:10px }
.calibration-wrapper-v2 .step-indicator .pendingStep { fill:#707070;stroke:#707070 }
.calibration-wrapper-v2 .step-indicator .activeStep { fill:#44d62c;stroke:#44d62c }
.calibration-wrapper-v2 .calibration-area { display:flex;flex-direction:row;margin:11px 30px 30px 23px;min-width:850px }
.calibration-wrapper-v2 .calibration-area .prodImage-container { display:grid;margin:auto;width:fit-content }
.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration { grid-area:1/1/2/2;max-width:372px!important }
.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image { height:250px;width:371px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image svg { display:block;height:auto;width:100% }
.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image .leftBumper,.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image .leftJoystick,.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image .rightJoystick { display:none }
.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image.show-left-bumper .leftBumper,.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image.show-thumbsticks .leftJoystick,.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.svg-image.show-thumbsticks .rightJoystick { display:inline }
.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration.placeholder { min-height:250px;width:371px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-1,.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-2,.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-rotate-overlay-1,.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-rotate-overlay-2,.calibration-wrapper-v2 .calibration-area .prodImage-container .left-thumbstick-overlay,.calibration-wrapper-v2 .calibration-area .prodImage-container .right-thumbstick-overlay { grid-area:1/1/2/2;z-index:2 }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-1 { padding-right:204.5px;padding-right:var(--calibration-direction-left-right,204.5px);padding-top:11px;padding-top:var(--calibration-direction-left-top,11px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-1.ps-ctrl { padding-right:110px;padding-right:var(--calibration-direction-left-right-ps,110px);padding-top:50px;padding-top:var(--calibration-direction-left-top-ps,50px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-2 { padding-left:110px;padding-left:var(--calibration-direction-right-left,110px);padding-top:68px;padding-top:var(--calibration-direction-right-top,68px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-2.ps-ctrl { padding-right:0;padding-right:var(--calibration-direction-right-right-ps,0);padding-top:50px;padding-top:var(--calibration-direction-right-top-ps,50px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .left-thumbstick-overlay { padding-right:208.5px;padding-right:var(--calibration-thumbstick-left-right,208.5px);padding-top:69px;padding-top:var(--calibration-thumbstick-left-top,69px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .left-thumbstick-overlay.ps-ctrl { padding-right:112.5px;padding-right:var(--calibration-thumbstick-left-right-ps,112.5px);padding-top:108px;padding-top:var(--calibration-thumbstick-left-top-ps,108px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .right-thumbstick-overlay { padding-left:108px;padding-left:var(--calibration-thumbstick-right-left,108px);padding-top:126px;padding-top:var(--calibration-thumbstick-right-top,126px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .right-thumbstick-overlay.ps-ctrl { padding-right:0;padding-right:var(--calibration-thumbstick-right-right-ps,0);padding-top:108px;padding-top:var(--calibration-thumbstick-right-top-ps,108px) }
.calibration-wrapper-v2 .calibration-area .prodImage-container .lb-overlay { grid-area:1/1/2/2;padding-left:56px;padding-left:var(--calibration-lb-left,56px);padding-top:5px;padding-top:var(--calibration-lb-top,5px);text-align:left;z-index:2 }
.calibration-wrapper-v2 .calibration-area .prodImage-container .lb-overlay.ps-ctrl { padding-left:49px;padding-left:var(--calibration-lb-left-ps,49px);padding-top:2px;padding-top:var(--calibration-lb-top-ps,2px) }
.calibration-wrapper-v2 .calibration-description { font-size:14px;margin:auto;min-height:44px;width:460px }
.calibration-wrapper-v2 .thumbstick-simulator { margin:auto auto -38.5px }
.calibration-wrapper-v2 .calibration-footer { display:flex;justify-content:center }
.calibration-wrapper-v2 .calibration-footer .button { background:#0000;border:1px solid #ccc;border-radius:3px;color:#ccc;font-size:12px;height:27px;margin:10px 5px 0;padding:0 14px }
.calibration-wrapper-v2 .calibration-footer .button:hover { background:rgba(0,0,0,.929);cursor:pointer }
@media(max-width:850px) { .calibration-area { transform:scale(.8) } }
@media(max-width:650px) { .calibration-area { transform:scale(.6) } }
@media(max-width:400px) { .calibration-area { transform:scale(.5) } }
.popup-widget .backdrop.trigger-calibration-popup-widget .choose-a-mat.linked-games-popup { height:100vh;margin-top:0;max-height:100vh!important;width:850px }
.popup-widget .backdrop.trigger-calibration-popup-widget .choose-a-mat .head { align-items:center;display:flex;justify-content:center;padding:0 36px }
.popup-widget .backdrop.trigger-calibration-popup-widget .choose-a-mat .body.scrollable { overflow:visible;padding:20px 25px 30px }
.trigger-calibration-popup { align-items:center;box-sizing:border-box;color:#ccc;display:flex;flex-direction:column;margin:0 auto;max-width:100%;min-height:352px;text-align:center;width:601px }
.trigger-calibration-popup__steps { display:flex;height:21px;justify-content:center;width:601px }
.trigger-calibration-popup__steps svg { display:block;height:21px;overflow:visible;width:90.25px }
.trigger-calibration-popup__description { color:#ccc;font-family:Roboto,sans-serif;font-size:14px;font-weight:400;line-height:17px;margin-top:50px;min-height:34px;width:400px }
.trigger-calibration-popup__description--wide { width:450px }
.trigger-calibration-popup__description--success { color:#44d62c;font-family:RazerF5,sans-serif;font-size:16px;font-weight:400;line-height:16px;text-transform:uppercase }
.trigger-calibration-popup__visualization { height:158px;margin-top:20px;position:relative;transform:none;width:601px }
.trigger-calibration-popup__meter { align-items:center;display:flex;height:138.91px;justify-content:center;overflow:visible;position:absolute;top:3px;width:74.94px;z-index:2 }
.trigger-calibration-popup__meter--left { left:39.56px }
.trigger-calibration-popup__meter--right { right:39.56px }
.trigger-calibration-popup__meter-svg { display:block;flex-shrink:0;overflow:visible;position:relative;z-index:2 }
.trigger-calibration-popup__meter-svg .semi-circular-widget__prompt { animation:trigger-calibration-prompt-sweep 2s linear infinite,trigger-calibration-prompt-fade 2s linear infinite;transform-box:view-box;transform-origin:70.4707px 70.4707px;transform-origin:var(--calibration-arc-center,70.4707px) var(--calibration-arc-center,70.4707px) }
.trigger-calibration-popup__timer { height:100px;pointer-events:none;position:absolute;top:19.455px;width:100px;z-index:1 }
.trigger-calibration-popup__timer svg { display:block;height:100px;overflow:visible;width:100px }
.trigger-calibration-popup__meter--left .trigger-calibration-popup__timer { left:19.455px }
.trigger-calibration-popup__meter--right .trigger-calibration-popup__timer { right:19.455px }
.trigger-calibration-popup__artwork { align-items:center;display:flex;height:158px;justify-content:center;left:114.5px;position:absolute;top:0;width:372px }
.trigger-calibration-popup__artwork .actuation-device-artwork { align-self:center }
.trigger-calibration-popup__artwork--success-left .leftTrigger,.trigger-calibration-popup__artwork--success-right .rightTrigger { fill-opacity:.2;stroke:#44d62c;stroke-width:1;opacity:1 }
.trigger-calibration-popup__actions { display:flex;justify-content:center;margin-top:30px }
.trigger-calibration-popup__state { align-items:center;display:flex;flex:1 1;flex-direction:column;margin-top:50px;width:601px }
.trigger-calibration-popup__state>.trigger-calibration-popup__visualization { margin-top:20px }
.trigger-calibration-popup__error-heading { align-items:flex-start;color:#fff;display:flex;font-family:RazerF5,sans-serif;font-size:16px;font-weight:400;gap:10px;height:34px;justify-content:center;line-height:16px;text-transform:uppercase }
.trigger-calibration-popup__error-heading span { margin-top:4px }
.trigger-calibration-popup__error-icon { align-items:center;display:flex;flex:0 0 24px;height:24px;justify-content:center;width:24px }
.thumbstick-calibration-popup { --calibration-direction-left-top:11px;--calibration-direction-left-right:204.5px;--calibration-direction-right-top:68px;--calibration-direction-right-left:110px;--calibration-thumbstick-left-top:69px;--calibration-thumbstick-left-right:208.5px;--calibration-thumbstick-right-top:126px;--calibration-thumbstick-right-left:108px;--calibration-lb-top:5px;--calibration-lb-left:56px }
```

## Assets and edition fallback

- `local-ui-reverse/source/official/apps.razer.com/synapse/products/2676/ui/static/media/Calibration.21d34a0f.svg` → `assets/synapse/gamepad-2676-calibration-source.svg`; original SHA-256 `66ac4c2ab938abe37db2679af70577bc955e9909ec687fb867c7dc290b3ae7cc`.
- `local-ui-reverse/source/official/apps.razer.com/synapse/products/2676/ui/static/media/icon_close.55fe41f1.svg` → `assets/synapse/gamepad-2676-calibration-close.svg`; original SHA-256 `53b67d2c7d30aa84869391d651541e981b2f9ac508a2c0c04d37f5f4a8faef11`.

Raster callers request the selected PID and edition, then the same PID edition zero. 2650 has only edition-zero calibration artwork. 4144 passes `isSvg: true` from Qh. 2676/2684 callers also pass `isSvg: true`, and the popup displays only its selected thumbstick overlay at step 6. The right PS overlays retain inherited padding-left; the PS rules alter padding-top/padding-right.

## State, actions and acceptance

Popup mount requests start for the selected thumbstick. Steps 1–5 keep Cancel; step 5 with validity plus three genuine rotations sends rotate_complete once and waits for actual step 6 before rendering Done. Explicit close, Cancel and Done request stop and restore focus. The error content remains inside this same popup with Cancel/Recalibrate. CALIBRATION_USER_MOVEMENT percentages are finite-checked, clamped, and converted to x*10, -y*10.

TAB_CALIBRATION selection widget and the trigger calibration popup remain unimplemented. The existing generic THUMBSTICKS layout is still wrong: its source mounts product artwork, two deadzone rows in one widget, independent clutch and circularity widgets. This batch adds its confirmed per-row calibration action, but does not claim that surrounding layout is complete. The popup focus/initialization observation bridge is also still missing.

Canvas fade/interpolation and all runtime screenshot/interaction acceptance remain unverified. No application, vendor JS, DLL or real device operation was executed. Formatting and static AST/resource/hash validation were performed; parent owns the consolidated cargo check.
