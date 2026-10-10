# 2636 calibration — current source, 2026-10-10

Full-page calibration wizard. Source acquisition, static semantics, Rust presentation, state/event seam and runtime acceptance are separate. This page is not marked fully complete.

- JavaScript: `local-ui-reverse/source/official/apps.razer.com/synapse/products/2636/ui/static/js/main.d624c214.js`
- SHA-256: `e2974c1a031ac2af18d79a0dd412ebc757368be9e11ed4a996c09e6fa17add48`
- Mounted initializer: `XM`, UTF-16 offsets 6687021..6692361.
- Rust: `crates/razer-pages/src/features/gamepad_calibration.rs`; state: `gamepad_calibration_state.rs`.
- Complete AST source and CSS rule offsets: [receipt](gamepad-2636-calibration-live-source.json).

## Mounted HTML / JavaScript logic

The JSX below is the original complete component initializer, not an invented HTML template. Element tags, props, classes, conditional branches and source action handlers are retained verbatim.

```javascript
()=>{const e=(0,v.useSelector)(e=>e.deviceReducer.productId),t=(0,v.useSelector)(e=>e.deviceReducer.editionId),n=(0,v.useSelector)(e=>e.customizeReducer.gamepadTester),i=(0,v.useSelector)(e=>e.controllerCalibrationReducer.step),o=(0,v.useSelector)(e=>e.controllerCalibrationReducer.partId),[r,s]=(0,g.useState)(null),E=(0,v.useDispatch)(),_=re.DeviceInfo.isPlaystationController,T=(0,v.useSelector)(e=>e.controllerCalibrationReducer.isStepValid),[l,I]=(0,g.useState)(0),A=async()=>{try{const t=await a(52927)(`./${e}_0/img_prods/0-3x.png`);s(t.default)}catch(t){console.log(t)}};(0,g.useEffect)(()=>{(async()=>{try{const n=await a(71461)(`./${e}_${t}/img_prods/0-3x.png`);s(n.default)}catch(n){A()}})()},[]);const d=e=>{O("start",e)},c=()=>{O("stop",o)},O=(e,t)=>{E(((e,t)=>async a=>{a({type:St.ON_SET_CONTROLLER_CALIBRATION,payload:{action:e,partId:t}})})(e,t))},S=(0,g.useCallback)(()=>(0,li.jsx)("svg",{width:38,height:38,viewBox:"-1 -1 38 38",fill:"none",xmlns:"http://www.w3.org/2000/svg",children:(0,li.jsxs)("g",{children:[(0,li.jsx)("circle",{cx:"19",cy:"15",r:"15",fill:"#44D62C",fillOpacity:"0.2",shapeRendering:"crispEdges"}),(0,li.jsx)("circle",{cx:"19",cy:"15",r:"14.5",stroke:"#2EB717",shapeRendering:"crispEdges"})]})}),[]),u=(0,g.useCallback)(()=>(0,li.jsx)("svg",{width:"64",height:"29",viewBox:"0 0 64 29",fill:"none",xmlns:"http://www.w3.org/2000/svg",children:(0,li.jsx)("path",{d:"M18.5709 7.17398C30.422 0.973981 44.4011 0.669753 52.0409 0C56.7366 0 61.6373 4.78395 63.5006 7.17593C63.9665 7.55864 62.0006 8.41699 59.5008 8.41699C54.752 8.41699 42.2503 10.2711 30.0007 14.417C19.793 17.8718 5.71369 24.9703 0.123589 28.4147C0.0304184 27.0752 -0.100014 24.1092 0.123589 22.961C0.347191 21.8129 1.3348 19.995 1.80064 19.2295C2.45282 17.7944 6.71993 13.374 18.5709 7.17398Z",fill:"#44D62C",fillOpacity:"0.3"})}),[]),R=(0,g.useCallback)(()=>{switch(i){case 0:return(0,li.jsx)(Ti.A,{text:He.Kr7});case 1:return(0,li.jsx)(Ti.A,{text:He.dqO});case 2:return(0,li.jsx)(Ti.A,{text:He.YHw});case 3:return(0,li.jsx)(Ti.A,{text:He.R5i});case 4:return(0,li.jsx)(Ti.A,{text:He.Kx$});case 5:return(0,li.jsx)(Ti.A,{text:He.gEn});case 6:return}},[i]),N=(0,g.useCallback)(()=>{switch(i){case 0:return(0,li.jsxs)(li.Fragment,{children:[(0,li.jsx)("button",{className:"button",onClick:()=>d(1),children:(0,li.jsx)(Ti.A,{text:He.$JN})}),(0,li.jsx)("button",{className:"button",onClick:()=>d(2),children:(0,li.jsx)(Ti.A,{text:He.MKQ})})]});case 1:case 2:case 3:case 4:return(0,li.jsx)("button",{className:"btnCancel uppercase",onClick:()=>c(),children:(0,li.jsx)(Ti.A,{text:He.bOp})});case 5:return T&&3===l?(0,li.jsx)("button",{className:"btnPrimary",onClick:()=>c(),children:(0,li.jsx)(Ti.A,{text:He.DHM})}):(0,li.jsx)("button",{className:"btnCancel uppercase",onClick:()=>c(),children:(0,li.jsx)(Ti.A,{text:He.bOp})});case 6:return(0,li.jsx)("button",{className:"btnPrimary",onClick:()=>c(),children:(0,li.jsx)(Ti.A,{text:He.DHM})})}},[i,l,T]);return(0,li.jsxs)(li.Fragment,{children:[-1===i&&(0,li.jsx)("div",{className:"popup-overlay",children:(0,li.jsxs)("div",{className:"popup-content",children:[(0,li.jsxs)("div",{className:"header-warning",children:[(0,li.jsx)("div",{className:"calibration-warning-icon",children:(0,li.jsx)(jM,{})}),(0,li.jsx)("span",{className:"popupTitle",children:(0,li.jsx)(Ti.A,{text:He.w2s})})]}),(0,li.jsx)("div",{className:"popup-description",children:(0,li.jsx)(Ti.A,{text:He.KPZ})}),(0,li.jsx)("button",{className:"btnCancel uppercase",onClick:c,children:(0,li.jsx)(Ti.A,{text:He.bOp})}),(0,li.jsx)("button",{className:"btnPrimary",onClick:()=>{O("start",o)},children:(0,li.jsx)(Ti.A,{text:He.AGU})})]})}),(0,li.jsxs)("div",{className:"calibration-wrapper-v2",children:[(0,li.jsx)("div",{className:"step-indicator",style:{visibility:i>=1?"visible":"hidden"},children:(0,li.jsx)(zM,{steps:i+1})}),(0,li.jsx)("div",{className:"calibration-title",children:(0,li.jsx)(Ti.A,{text:i<1?He.YmC:6===i||5===i&&T&&3===l?He.Jvl:""})}),(0,li.jsx)("br",{}),(0,li.jsxs)("div",{className:"calibration-area",children:[(0,li.jsx)("div",{className:"thumbstick-simulator",style:{visibility:0==i?"visible":"hidden"},children:(0,li.jsx)(aM,{thumbStickXData:n.leftThumbX,thumbStickYData:n.leftThumbY})}),(0,li.jsxs)("div",{className:"prodImage-container",children:[(0,li.jsx)("img",{className:"prodImage-calibration img-nodrag",src:r}),(()=>{const e=1===o?n.leftThumbX:n.rightThumbX,t=1===o?n.leftThumbY:n.rightThumbY;switch(i){case 0:return(0,li.jsxs)(li.Fragment,{children:[(0,li.jsx)("div",{className:"left-thumbstick-overlay "+(_?"ps-ctrl":""),children:S()}),(0,li.jsx)("div",{className:"right-thumbstick-overlay "+(_?"ps-ctrl":""),children:S()})]});case 1:case 2:case 3:case 4:case 5:return T&&5!==i?(0,li.jsx)("div",{className:"lb-overlay "+(_?"ps-ctrl":""),children:u()}):(0,li.jsx)("div",{className:`joystick-direction-overlay-${o} ${_?"ps-ctrl":""}`,children:(0,li.jsx)(xM,{thumbStickXData:e,thumbStickYData:t,step:i,rotationCount:l,handleRotationCount:e=>I(e)})});default:return null}})()]}),(0,li.jsx)("div",{className:"thumbstick-simulator",style:{visibility:0==i?"visible":"hidden"},children:(0,li.jsx)(aM,{thumbStickXData:n.rightThumbX,thumbStickYData:n.rightThumbY})})]}),(0,li.jsx)("div",{className:"calibration-description",children:R()}),(0,li.jsx)("div",{className:"calibration-footer",children:N()})]})]})}
```

## Source CSS

Source `local-ui-reverse/source/official/apps.razer.com/synapse/products/2636/ui/static/css/main.3d9eb3d1.css`; SHA-256 `726c9570155ac2adaf8a39a012181e805f77c919e8e86b1ea1128d791a13d1d2`.

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
.calibration-wrapper-v2 { text-align:center }
.calibration-wrapper-v2 .calibration-title { color:#44d62c;font-family:RazerF5,sans-serif;font-size:16px;min-height:22px;text-transform:uppercase }
.calibration-wrapper-v2 .step-indicator { margin:10px }
.calibration-wrapper-v2 .step-indicator .pendingStep { fill:#707070;stroke:#707070 }
.calibration-wrapper-v2 .step-indicator .activeStep { fill:#44d62c;stroke:#44d62c }
.calibration-wrapper-v2 .calibration-area { display:flex;flex-direction:row;margin:11px 30px 30px 23px;min-width:850px }
.calibration-wrapper-v2 .calibration-area .prodImage-container { display:grid;margin:auto;width:fit-content }
.calibration-wrapper-v2 .calibration-area .prodImage-container .prodImage-calibration { grid-area:1/1/2/2;max-width:372px!important }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-1,.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-2,.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-rotate-overlay-1,.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-rotate-overlay-2,.calibration-wrapper-v2 .calibration-area .prodImage-container .left-thumbstick-overlay,.calibration-wrapper-v2 .calibration-area .prodImage-container .right-thumbstick-overlay { grid-area:1/1/2/2;z-index:2 }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-1 { padding-right:204.5px;padding-top:11px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-1.ps-ctrl { padding-right:110px;padding-top:50px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-2 { padding-left:110px;padding-top:68px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .joystick-direction-overlay-2.ps-ctrl { padding-right:0;padding-top:50px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .left-thumbstick-overlay { padding-right:208.5px;padding-top:69px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .left-thumbstick-overlay.ps-ctrl { padding-right:112.5px;padding-top:108px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .right-thumbstick-overlay { padding-left:108px;padding-top:126px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .right-thumbstick-overlay.ps-ctrl { padding-right:0;padding-top:108px }
.calibration-wrapper-v2 .calibration-area .prodImage-container .lb-overlay { grid-area:1/1/2/2;padding-left:56px;padding-top:5px;text-align:left;z-index:2 }
.calibration-wrapper-v2 .calibration-area .prodImage-container .lb-overlay.ps-ctrl { padding-left:49px;padding-top:2px }
.calibration-wrapper-v2 .calibration-description { font-size:14px;margin:auto;min-height:44px;width:460px }
.calibration-wrapper-v2 .thumbstick-simulator { margin:auto auto -38.5px }
.calibration-wrapper-v2 .calibration-footer { display:flex;justify-content:center }
.calibration-wrapper-v2 .calibration-footer .button { background:#0000;border:1px solid #ccc;border-radius:3px;color:#ccc;font-size:12px;height:27px;margin:10px 5px 0;padding:0 14px }
.calibration-wrapper-v2 .calibration-footer .button:hover { background:rgba(0,0,0,.929);cursor:pointer }
@media(max-width:850px) { .calibration-area { transform:scale(.8) } }
@media(max-width:650px) { .calibration-area { transform:scale(.6) } }
@media(max-width:400px) { .calibration-area { transform:scale(.5) } }
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
```

## Assets and edition fallback

- `local-ui-reverse/source/official/apps.razer.com/synapse/products/2636/ui/static/media/0-3x.12d19936.avif` → `assets/synapse/gamepad-2636-calibration-edition-0.png`; original SHA-256 `a3e9de876d596c66f980497383742bbaf69625d2af05640199481a15f73d5b35`; edition 0, webpack module 22726.
- `local-ui-reverse/source/official/apps.razer.com/synapse/products/2636/ui/static/media/0-3x.05e41989.avif` → `assets/synapse/gamepad-2636-calibration-edition-128.png`; original SHA-256 `e35dbd31f5dfec80376e5e7e051aab18d85dbb54158cfdd692e939af8f495bfb`; edition 128, webpack module 69171.
- `local-ui-reverse/source/official/apps.razer.com/synapse/products/2636/ui/static/media/0-3x.de66435c.avif` → `assets/synapse/gamepad-2636-calibration-edition-129.png`; original SHA-256 `1313518a905be0d25d2f5af790faa29e9f41c04064a7d37c5ac267e411a9de56`; edition 129, webpack module 13932.

Raster callers request the selected PID and edition, then the same PID edition zero. 2650 has only edition-zero calibration artwork. 4144 passes `isSvg: true` from Qh. 2676/2684 callers also pass `isSvg: true`, and the popup displays only its selected thumbstick overlay at step 6. The right PS overlays retain inherited padding-left; the PS rules alter padding-top/padding-right.

## State, actions and acceptance

Step zero exposes both thumbstick start actions. Steps 1–4 expose Cancel. Step 5 requires source validity and three observed rotations for Done; step 6 exposes Done. Cancel/Done emit stop. Error uses the original warning title/description and explicit Cancel/Recalibrate controls; no added Escape/backdrop dismissal.

Native service/device submission and real observations are not accepted by this static UI batch. Existing event seam is retained; issuing a local intention never proves a successful device write.

Canvas fade/interpolation and all runtime screenshot/interaction acceptance remain unverified. No application, vendor JS, DLL or real device operation was executed. Formatting and static AST/resource/hash validation were performed; parent owns the consolidated cargo check.
