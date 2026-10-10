# PID 679 ? ACTUATION ? ????????

## ?????????

?Rust???Customize??Snap Tap??????????PID515???679?CU????DU/jm???????ACTUATION????actuation_page?????/rapid/continuous??DU?Wh reset/help????????Button????????????continuous?tips?????????/?????????caller???

????/???679???keyboard_snap679.rs?ACTUATION???DU??????????Customize????/+?????????????CSS??reset????Ja?????????/??????help?Reset Actuation??740/746?????Calibration?679???????Calibration???????

## ???????

????/????????????????????????ON_SET_SNAP_TAP/ON_SET_KEYMAPPING???typed????native???unsupported?REGISTER/UNREGISTER_SNAP_TAP_EVENT?disable/enableMapping?host registerNoBrowserInputHandler???redirect?????????pressedKeys??????????????native key location/??????????????????????????500ms/?????sensor??????vertical-slider??/???tooltip???????????

## ???? JavaScript ? HTML ???/??

### CU

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7294922,7296460)?UTF-16 [7294923,7296461)?

```javascript
class CU extends g.Component{constructor(){super(...arguments),this.setDisplaySaveAlertRef=e=>{this.props.setDisplaySaveAlertRef(e)}}render(){const e=this.props.buttonList.some(e=>e.isFocused);return(0,un.jsxs)("div",{children:[this.props.isFactoryDefaultProfile?(0,un.jsx)("div",{className:"factory-default",children:(0,un.jsxs)("div",{className:"warning-alert",children:[(0,un.jsx)("div",{className:"title",children:(0,un.jsx)(Sn.A,{text:be.mzS})}),(0,un.jsx)("div",{className:"content",children:(0,un.jsx)("p",{dangerouslySetInnerHTML:{__html:(0,Xt.getTextItem)(be.ebo,{dots:' <svg version="1.1" id="Layer_1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px"\n                         viewBox="0 0 20 20" style="enable-background:new 0 0 20 20;fill:#ccc;width:14px;height:14px;margin:0 -5px -5px" xml:space="preserve">\n                      <path d="M4,8c-1.1,0-2,0.9-2,2s0.9,2,2,2s2-0.9,2-2S5.1,8,4,8z M16,8c-1.1,0-2,0.9-2,2s0.9,2,2,2s2-0.9,2-2S17.1,8,16,8z M10,8\n                        c-1.1,0-2,0.9-2,2s0.9,2,2,2s2-0.9,2-2S11.1,8,10,8z"/>\n                      </svg>\n                      '})}})})]})}):null,(0,un.jsxs)(ZE,{className:this.props.isFactoryDefaultProfile?"disabled":"",children:[(0,un.jsx)(dU,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef}),(0,un.jsxs)(ZE,{children:[(0,un.jsxs)(e_,{direction:"left",children:[(0,un.jsx)(DU,{}),(0,un.jsx)(kh,{isActive:e})]}),(0,un.jsxs)(e_,{direction:"right",children:[(0,un.jsx)($h,{isActive:e}),(0,un.jsx)(RU,{})]})]})]})]})}}
```

### Wh

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7249552,7257678)?UTF-16 [7249553,7257679)?

```javascript
class Wh extends g.Component{constructor(e){super(e),this.onMakeActuationPointChange=e=>{if(this.props.adjustmentModeRunning)return void this.props.onShowAdjustmentModePrompt(!0);const t=He.fH.toMappingVal(e);this.props.isSelectAll&&this.props.setActuationAllKeys(t),this.setState({makeActuationPointValue:t,isDisabled:!1},()=>{this.onActuationPointChange(t,this.state.customReleasePointValue)})},this.onActuationPointChange=(e,t)=>{let a={0:e,1:t};this.props.setActuationToBtnList(a),this.props.setActuation(this.props.buttonList,this.props.mappingList);const n=this.props.buttonList.filter(e=>e.isFocused).map(e=>e.inputID);"KEYBOARD"===_e.DeviceInfo.category&&Array.isArray(n)&&n.length>0&&(0,Fm.gP)({actuationPoint:e/1638/10,selectedKeys:n}),"KEYPAD"===_e.DeviceInfo.category&&Array.isArray(n)&&n.length>0&&(0,kD.Lp)({actuationPoint:e/1638/10,selectedKeys:n})},this.handleClickOutside=e=>{this.resetPopupRef&&this.resetPopupRef.current&&!this.resetPopupRef.current.contains(e.target)&&this.state.isShowResetPopup&&this.setState({isShowResetPopup:!1})},this.syncActuationKeys=e=>{if(this.props.adjustmentModeRunning)return void this.props.onShowAdjustmentModePrompt(!0);let t=[];"all"===e?(t=this.props.buttonList.filter(e=>!!(0,Me.QY)(e.inputID,this.props.mappingList)),this.setState({syncSettingsToAllKeys:!0})):this.setState({syncSettingsToSelectedKeys:!0});const a=this.props,n=a.syncActuationKeys,o=a.mappingList;n(e,o,{0:this.state.makeActuationPointValue,1:this.state.makeActuationPointValue},t),setTimeout(()=>{this.setState({syncSettingsToAllKeys:!1,syncSettingsToSelectedKeys:!1})},2e3),o.length>0&&"all"===e&&setTimeout(()=>{this.setState({isDisabled:!0})},2500)},this.resetActuationToDefault=e=>{this.props.adjustmentModeRunning?this.props.onShowAdjustmentModePrompt(!0):(e.stopPropagation(),this.props.resetActuation(this.props.mappingList),this.props.setSelectedAndDeselectedAllKeys(!1,[]),this.setState({isShowResetPopup:!1}))},this.handleShowResetPopup=e=>{this.props.adjustmentModeRunning?this.props.onShowAdjustmentModePrompt(!0):(e.stopPropagation(),this.setState({isShowResetPopup:!0,isShowResetTip:!1}))},this.handleShowAdjustmentModePrompt=e=>{this.props.onShowAdjustmentModePrompt(e)},this.state={minActuation:e.actuationDefaultConfig.min,maxActuation:e.actuationDefaultConfig.max,makeActuationPointValue:e.actuation[0]||e.actuationDefaultConfig.default[0],customReleasePointValue:e.actuation[1]||e.actuationDefaultConfig.default[1],isEnableCustomRelease:!1,syncSettingsToAllKeys:!1,syncSettingsToSelectedKeys:!1,isShowResetPopup:!1,isShowResetTip:!1,isDisabled:!0},this.resetPopupRef=f().createRef(),this.buttonsStack=[]}findChangedButton(e,t){let a=[],n=[];return a=e.map(e=>e.inputID),n=t.map(e=>e.inputID),a.length>n.length?(a.forEach(e=>{n.includes(e)&&(a=a.filter(t=>t!==e))}),this.buttonsStack.push(a[0])):(n.forEach(e=>{a.includes(e)&&(n=n.filter(t=>t!==e))}),this.buttonsStack=this.buttonsStack.filter(e=>e!==n[0]))}hasActuationChanged(e){if(!this.props.actuation)return!1;const t=He.fH.toSliderVal(this.state.makeActuationPointValue),a=He.fH.toSliderVal(this.props.actuation[0]);return this.props.actuation[0]!==e.actuation[0]&&t!==a}componentDidUpdate(e){if(dn()(this.props.actuationDefaultConfig,e.actuationDefaultConfig)||this.setState({minActuation:this.props.actuationDefaultConfig.min,maxActuation:this.props.actuationDefaultConfig.max,makeActuationPointValue:this.props.actuation[0]||this.props.actuationDefaultConfig.default[0],customReleasePointValue:this.props.actuation[1]||this.props.actuationDefaultConfig.default[1]}),!dn()(this.props.selectedButtonList,e.selectedButtonList)||this.hasActuationChanged(e)){0===this.props.selectedButtonList.length?this.setState({isDisabled:!0}):this.setState({isDisabled:!1}),1===Math.abs(this.props.selectedButtonList.length-e.selectedButtonList.length)||1===this.props.selectedButtonList.length?this.findChangedButton(this.props.selectedButtonList,e.selectedButtonList):this.buttonsStack=[];const t=new Map(this.props.mappingList.filter(e=>!e.isHyperShift&&"AnalogInput"===e.inputType).map(e=>[e.inputID,e.mapping[0].actuationPoint])),a=e=>{if(!e||!e[0])return"disabled";let t="".concat(e[0]);return e[0]&&(t+="-".concat(e[0])),t},n=new Map,o=new Map;this.props.selectedButtonList.forEach(e=>{const i=t.get(e.inputID),s=a(i);let r=n.get(s)||0;n.set(s,r+1),o.set(s,i)});const i=[...n.entries()].reduceRight((e,t)=>{let a=(0,pn.A)(t,2),n=a[0],o=a[1];return e.max<o?{key:n,max:o}:e},{key:null,max:0}),s=i.key;let r;if(n.get(s)>=1&&0!==this.buttonsStack.length){let e=this.props.selectedButtonList.find(e=>e.inputID===this.buttonsStack.slice(-1)[0]);t.has(null===e||void 0===e?void 0:e.inputID)&&(r=t.get(e.inputID))}else r=o.get(s);if(!r)return void this.setState({makeActuationPointValue:this.props.actuationDefaultConfig.default[0]});this.setState({makeActuationPointValue:r[0]||this.props.actuationDefaultConfig.default[0]})}}componentDidMount(){document.addEventListener("click",this.handleClickOutside)}componentWillUnmount(){document.removeEventListener("click",this.handleClickOutside),this.props.setDefaultActuation(this.props.actuationDefaultConfig.default[0],this.props.actuationDefaultConfig.default[1])}render(){const e=this.props,t=e.isActive,a=e.adjustmentModeRunning,n=this.state,o=n.makeActuationPointValue,i=n.minActuation,s=n.maxActuation,r=He.fH.toSliderVal(o),E=He.fH.toSliderVal(i),_=He.fH.toSliderVal(s),T=_e.DeviceInfo.analogSpecs.actuationInfo.offSet,c=tn.r?Number(He.fH.toDisplayVal(this.state.minActuation))+T:He.fH.toDisplayVal(this.state.minActuation),I=tn.r?Number(He.fH.toDisplayVal(this.state.maxActuation))+T:He.fH.toDisplayVal(this.state.maxActuation);return(0,un.jsxs)(JE,{title:be.$so,extraClass:"actuation-widget",children:[(0,un.jsxs)(un.Fragment,{children:[(0,un.jsx)("div",{className:"help"}),(0,un.jsx)("div",{className:"tip",children:(0,un.jsx)("div",{className:"actuation-tip",children:(0,un.jsx)("div",{children:(0,un.jsx)(Sn.A,{text:be.QuW})})})})]}),(0,un.jsxs)("div",{"aria-roledescription":"reload page",role:"button",className:"arrow icon-refresh-actuation",onMouseOver:()=>this.setState({isShowResetTip:!this.state.isShowResetPopup}),onMouseLeave:()=>this.setState({isShowResetTip:!1}),onClick:this.handleShowResetPopup,children:[(0,un.jsx)("div",{className:"actuation-reset-tip ".concat(this.state.isShowResetTip?"show":""," "),children:(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.mJB})})}),(0,un.jsxs)("div",{className:"actuation-reset-popup ".concat(this.state.isShowResetPopup?"show":""),ref:this.resetPopupRef,children:[(0,un.jsx)("h6",{children:(0,un.jsx)(Sn.A,{text:be.hlp})}),(0,un.jsx)("p",{children:(0,un.jsx)(Sn.A,{text:be._HI})}),(0,un.jsx)(Kh.A,{name:be.Uty,onClick:this.resetActuationToDefault})]})]}),(0,un.jsxs)("div",{className:"body-container",children:[(0,un.jsx)("div",{className:"actuation-point-title-desc",children:(0,un.jsx)(Sn.A,{text:be.Xdy})}),(0,un.jsxs)("div",{className:"actuation-container",children:[(0,un.jsxs)("div",{className:"animated-container",children:[(0,un.jsx)("img",{src:yh,alt:"Actuation Animated",draggable:!1}),(0,un.jsx)("div",{className:"actuation-point-slider"}),(0,un.jsx)("div",{className:"vertical-slider-container",children:(0,un.jsx)(Fh,{type:"range",min:E,max:_,active:t,minTag:"".concat(c," mm"),maxTag:"".concat(I," mm"),extraClass:"mb20",value:r,changeValue:LE()(this.onMakeActuationPointChange,500),analogKeyEvents:this.props.analogKeyEvents,mappingList:this.props.mappingList,handleShowAdjustmentModePrompt:this.handleShowAdjustmentModePrompt,adjustmentModeRunning:a,selectedButtonList:this.props.selectedButtonList})})]}),"analogV2"===_e.DeviceInfo.AnalogGenVersion&&r<10?(0,un.jsx)("p",{className:"actuation-warning",children:(0,un.jsx)(Sn.A,{text:be.T0U})}):null,(0,un.jsxs)("div",{className:"btn-sync-container",children:[(0,un.jsx)(Bh,{syncSettings:this.state.syncSettingsToAllKeys,name:be.xfg,onClick:()=>this.syncActuationKeys("all"),disabled:this.state.isDisabled}),(0,un.jsx)(Bh,{syncSettings:this.state.syncSettingsToSelectedKeys,name:be.HCt,onClick:()=>this.syncActuationKeys("selected"),disabled:!t})]})]})]})]})}}
```

### Jh

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7258738,7268344)?UTF-16 [7258739,7268345)?

```javascript
class Jh extends g.Component{constructor(e){super(e),this.findChangedButton=(e,t)=>{let a=[],n=[];return a=e.map(e=>e.inputID),n=t.map(e=>e.inputID),a.length>n.length?(a.forEach(e=>{n.includes(e)&&(a=a.filter(t=>t!==e))}),this.buttonsStack.push(a[0])):(n.forEach(e=>{a.includes(e)&&(n=n.filter(t=>t!==e))}),this.buttonsStack=this.buttonsStack.filter(e=>e!==n[0]))},this.initState=()=>{const e=qh(10*this.props.defaultRapidTrigger.makeSensitivity),t=qh(10*this.props.defaultRapidTrigger.breakSensitivity)||1;this.setState({isEnabled:!1,makeSensitivity:e,breakSensitivity:t,isEnableUpStroke:!1,isToggleContinuous:!1})},this.toggleSwitch=LE()(()=>{this.props.adjustmentModeRunning?this.props.onShowAdjustmentModePrompt(!0):this.setState({isEnabled:!this.state.isEnabled},()=>{this.setValues()})},Xh),this.toggleContinuous=LE()(()=>{this.setState({isToggleContinuous:!this.state.isToggleContinuous},()=>{this.setValues()})},Xh),this.setValues=()=>{var e,t;const a=this.state,n=a.makeSensitivity,o=a.breakSensitivity,i=a.isEnableUpStroke,s=this.props,r=s.setRapidTrigger,E=s.mappingList,_=s.selectedButtonList,T=s.setRapidTriggerAllKeys,c=s.isSelectAll,I=Qh(n);let A=Qh(o);i||(A=I);const l={makeSensitivity:I,breakSensitivity:A,isEnableUpStroke:i},O=this.props.profile.selectedProfileGuid===(null===(e=this.props.obmProfile)||void 0===e||null===(t=e[4])||void 0===t?void 0:t.guid)?ve.YZb:null;if(_&&Array.isArray(_)&&_.length>0){if("KEYBOARD"===Zh.category){const e=_.filter(e=>e&&e.analogInputID).map(e=>e.analogInputID);(0,Fm.Ep)({selectedKeys:e,enabled:this.state.isEnabled,downSensitivity:l.makeSensitivity,upSensitivity:l.breakSensitivity})}if("KEYPAD"===Zh.category){const e=_.filter(e=>e&&e.analogInputID).map(e=>e.analogInputID);(0,kD.$L)({selectedKeys:e,enabled:this.state.isEnabled,downSensitivity:l.makeSensitivity,upSensitivity:l.breakSensitivity})}}r(l,E,_,(0,U.A)({},this.props.defaultActuation),O,this.state.isEnabled),c&&T(l)},this.checkUpstroke=()=>{this.props.adjustmentModeRunning?this.props.onShowAdjustmentModePrompt(!0):this.setState({isEnableUpStroke:!this.state.isEnableUpStroke},()=>{this.setValues()})},this.onMakeRapidTriggerSliderChange=LE()(e=>{this.props.adjustmentModeRunning?this.props.onShowAdjustmentModePrompt(!0):this.setState({makeSensitivity:e},()=>{this.setValues()})},Xh),this.onUpStrokeSliderChange=e=>{this.props.adjustmentModeRunning||this.props.adjustmentModeRunning?this.props.onShowAdjustmentModePrompt(!0):this.setState({breakSensitivity:e},()=>{this.setValues()})},this.syncRapidTriggerKeys=e=>{var t,a;if(this.props.adjustmentModeRunning)return void this.props.onShowAdjustmentModePrompt(!0);let n=[];"all"===e?(n=this.props.buttonList.filter(e=>!!(0,Me.QY)(e.inputID,this.props.mappingList)),this.setState({syncSettingsToAllKeys:!0})):this.setState({syncSettingsToSelectedKeys:!0});const o=this.props.profile.selectedProfileGuid,i=null===ve.UMp||void 0===ve.UMp||null===(t=ve.UMp.find(e=>e.guid===o))||void 0===t?void 0:t.name,s=null!==(a={General:ve.CFc,"High Sensitivity":ve.YZb}[i])&&void 0!==a?a:null,r=this.state,E=r.makeSensitivity,_=r.breakSensitivity,T=r.isEnableUpStroke,c=r.isToggleContinuous,I={makeSensitivity:Qh(E),breakSensitivity:Qh(_),continuous:c,isEnableUpStroke:T},A=this.props,l=A.syncRapidTriggerKeys,O=A.mappingList,d=A.selectedButtonList,S=A.buttonList;l(e,this.state.isEnabled,I,O,d,S,s,n),setTimeout(()=>{this.setState({syncSettingsToAllKeys:!1,syncSettingsToSelectedKeys:!1})},2e3)},this.handleShowAdjustmentModePrompt=e=>{this.props.onShowAdjustmentModePrompt(e)},this.state={isEnabled:!1,makeSensitivity:1,breakSensitivity:1,isEnableUpStroke:!1,syncSettingsToAllKeys:!1,syncSettingsToSelectedKeys:!1,isToggleContinuous:!1,shouldCenterTooltip:!1},this.buttonsStack=[]}componentDidMount(){this.initState()}componentDidUpdate(e){if(this.props.defaultRapidTrigger.makeSensitivity!==e.defaultRapidTrigger.makeSensitivity&&this.initState(),!dn()(e.selectedButtonList,this.props.selectedButtonList)||!dn()(e.mappingList,this.props.mappingList)){var t;1===Math.abs(this.props.selectedButtonList.length-e.selectedButtonList.length)||1===this.props.selectedButtonList.length?this.findChangedButton(this.props.selectedButtonList,e.selectedButtonList):this.buttonsStack=[];const a=function(){let e=arguments.length>1&&void 0!==arguments[1]?arguments[1]:[];return((arguments.length>0&&void 0!==arguments[0]?arguments[0]:[])||[]).every(t=>{const a=t.inputID;return e.some(e=>e.inputID===a)})}(this.props.selectedButtonList,this.props.mappingList);this.setState({isEnabled:this.props.isActive&&!!a,isToggleContinuous:null!==(t=null===a||void 0===a?void 0:a.continuous)&&void 0!==t&&t});const n=new Map(this.props.mappingList.filter(e=>!e.isHyperShift&&"AnalogInput"===e.inputType).map(e=>[e.inputID,e.mapping[0].rapidTrigger])),o=e=>{if(!e||!e.continuous)return"disabled";let t="enable-".concat(e.makeSensitivity);return e.isEnableUpStroke&&(t+="-".concat(e.breakSensitivity)),t},i=new Map,s=new Map;this.props.selectedButtonList.forEach(e=>{const t=n.get(e.inputID),a=o(t);let r=i.get(a)||0;i.set(a,r+1),s.set(a,t)});const r=[...i.entries()].reduce((e,t)=>{let a=(0,pn.A)(t,2),n=a[0],o=a[1];return e.max<o?{key:n,max:o}:e},{key:null,max:0}),E=r.key;let _=null;if(i.get(E)>=1&&0!==this.buttonsStack.length){let e=this.props.selectedButtonList.find(e=>e.inputID===this.buttonsStack.slice(-1)[0]);n.has(null===e||void 0===e?void 0:e.inputID)&&(_=n.get(e.inputID))}else _=s.get(E);if(Yi()(_)){const e=qh(10*this.props.defaultRapidTrigger.makeSensitivity),t=qh(10*this.props.defaultRapidTrigger.breakSensitivity)||1;return void this.setState({makeSensitivity:e,breakSensitivity:t,isEnableUpStroke:!1,isEnabled:!1})}const T=_,c=T.makeSensitivity,I=T.breakSensitivity,A=T.continuous,l=T.isEnableUpStroke,O=qh(c),d=qh(I);this.setState({makeSensitivity:O,breakSensitivity:d,isToggleContinuous:A,isEnableUpStroke:l,isEnabled:!0})}}render(){const e=this.state,t=e.isEnabled,a=e.makeSensitivity,n=e.breakSensitivity,o=this.props,i=o.isActive,s=o.adjustmentModeRunning,r=o.lang;return(0,un.jsx)(un.Fragment,{children:(0,un.jsxs)(JE,{title:be.sS$,hasSwitch:!0,disableSwitchTips:be.mcf,toggleSwitch:this.toggleSwitch,active:t,disable:!i,children:[(0,un.jsxs)(un.Fragment,{children:[(0,un.jsx)("div",{className:"help"}),(0,un.jsx)("div",{className:"tip",children:(0,un.jsxs)("div",{className:"rapid-trigger-tip",children:[(0,un.jsx)("div",{children:(0,un.jsx)(Sn.A,{text:be.v04})}),(0,un.jsx)("div",{children:(0,un.jsx)("img",{src:xh,alt:"Special tip",draggable:!1,className:"rapid-trigger-animation-slider-tip"})}),(0,un.jsxs)("div",{className:"rapid-trigger-tip-slider-text",children:[(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.Deq})}),(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.HH$})})]}),(0,un.jsxs)("div",{className:"rapid-trigger-tip-bottom",children:[(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.IxD})}),(0,un.jsxs)("div",{className:"rapid-trigger-tip-bottom-status rapid-trigger-tip-bottom-status--green",children:[(0,un.jsx)("div",{}),(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.HcD})})]}),(0,un.jsxs)("div",{className:"rapid-trigger-tip-bottom-status rapid-trigger-tip-bottom-status--red",children:[(0,un.jsx)("div",{}),(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.Odj})})]})]})]})})]}),(0,un.jsxs)("div",{className:"rapid-trigger-sensitivity-section",children:[(0,un.jsx)("div",{className:"rapid-trigger-title-desc",children:(0,un.jsx)(Sn.A,{text:be.sgV})}),(0,un.jsx)("div",{className:"sensitivity-title ".concat(t?"":"disabled"),children:(0,un.jsx)(Sn.A,{text:be.pvx})}),(0,un.jsx)("div",{className:"sensitivity-title configure-desc ".concat(t?"":"disabled"),children:(0,un.jsx)(Sn.A,{text:be.LRV})}),(0,un.jsx)("div",{className:"sensitivity-slider-container",children:(0,un.jsx)(zh.A,{downStroke:!0,step:.1,min:.1,max:ve.q3X,active:t,minTag:"".concat(ve.q3X.toFixed(1)," mm (").concat((0,Xt.getTextItem)(be.$LP,{_lang:r}),")"),maxTag:"0.1 mm (".concat((0,Xt.getTextItem)(be.RYm,{_lang:r}),")"),extraClass:"mb20",value:a,changeValue:this.onMakeRapidTriggerSliderChange,callOnChangeOnEveryStep:!0,adjustmentModeRunning:s,handleShowAdjustmentModePrompt:this.handleShowAdjustmentModePrompt})}),"analogV2"===Zh.AnalogGenVersion&&this.state.makeSensitivity>.7?(0,un.jsxs)("div",{className:"actuation-warning-container",children:[(0,un.jsx)("div",{className:"info-icon"}),(0,un.jsx)("div",{children:(0,un.jsx)(Sn.A,{text:be.VUQ})})]}):null]}),(0,un.jsxs)("div",{className:"up-down-stroke-sensitivity-section",children:[this.state.isEnableUpStroke?(0,un.jsxs)(un.Fragment,{children:[(0,un.jsx)("div",{className:"upstroke-text",children:(0,un.jsx)(Sn.A,{text:be.I3R})}),(0,un.jsx)("div",{className:"up-down-stroke-sensitivity-slider-container",children:(0,un.jsx)(zh.A,{downStroke:!1,step:.1,min:.1,max:ve.q3X,active:t,minTag:"".concat(ve.q3X.toFixed(1)," mm (").concat((0,Xt.getTextItem)(be.$LP,{_lang:r}),")"),maxTag:"0.1 mm (".concat((0,Xt.getTextItem)(be.RYm,{_lang:r}),")"),extraClass:"mb20",value:n,changeValue:LE()(this.onUpStrokeSliderChange,Xh)})})]}):"",(0,un.jsx)("div",{className:"upstroke-sensitivity-checkbox",children:(0,un.jsx)(Sr.A,{id:"checkUpstroke",disabled:!t,active:this.state.isEnableUpStroke,onCheck:this.checkUpstroke,name:be.KZJ})}),(0,un.jsxs)("div",{className:"btn-sync-container",children:[(0,un.jsx)(Bh,{syncSettings:this.state.syncSettingsToAllKeys,name:be.xfg,onClick:()=>this.syncRapidTriggerKeys("all"),disabled:!i||!t}),(0,un.jsx)(Bh,{syncSettings:this.state.syncSettingsToSelectedKeys,name:be.HCt,onClick:()=>this.syncRapidTriggerKeys("selected"),disabled:!i||!t})]})]})]})})}}
```

### OU

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7284544,7291954)?UTF-16 [7284545,7291955)?

```javascript
class OU extends g.Component{constructor(e){var t;super(e),t=this,this.handleResize=()=>{this.state.specialTipShowed&&this.multimediaRef&&this.setPositionForMediaKeys(this.multimediaRef)},this.keyDownHandler=e=>{e.keyCode===lU&&(this.props.setSelectedAndDeselectedAllKeys(!1,[]),this.setState({showClearSelection:!1}))},this.hoverBtn=function(e,a){let n,o=arguments.length>2&&void 0!==arguments[2]&&arguments[2],i=t.props.buttonList[e];if("mediaVolume"===i.buttonKey)return console.log("mediavolume"),void t.hoverSpecial(a);switch(i.inputType){case"AnalogInput":if(ve.IOh.includes(i.buttonKey))n=i;else{let e=i.inputID;n=t.props.mappingList.find(t=>t.inputID===e)}break;case"MouseInput":{let e=i.MouseInput;n=t.props.mappingList.find(t=>t.inputID===e)}break;case"DKMInput":{let e=i.DKMInput;n=t.props.mappingList.find(t=>t.inputID===e)}}t.currentKey=a.current,setTimeout(()=>t.setState({hoverIndex:e,tipShowed:!0,activeHoverKey:i,hoverKeyMapping:n,specialChildTipShowed:o}),0)},this.hoverSpecial=e=>{this.multimediaRef=e,this.setState({specialTipShowed:!0})},this.setPositionForMediaKeys=e=>{var t,a,n,o,i,s;let r=e.current.getBoundingClientRect();const E=document.querySelector(".drawer-open"),_=document.querySelector(".main-container"),T=document.querySelector(".body-wrapper"),c=null!==(t=null===(a=document.querySelector(".customize #body-wrapper"))||void 0===a?void 0:a.scrollLeft)&&void 0!==t?t:0,I=null!==(n=null===(o=document.querySelector(".customize #body-wrapper"))||void 0===o?void 0:o.scrollTop)&&void 0!==n?n:0;this.specialTipDom.current.style.left=r.left-(null!==(i=null===E||void 0===E||null===(s=E.getBoundingClientRect())||void 0===s?void 0:s.width)&&void 0!==i?i:0)+c+"px",this.specialTipDom.current.style.top=r.top-(_.getBoundingClientRect().height-T.getBoundingClientRect().height+20)+I+"px"},this.leaveBtn=()=>{setTimeout(()=>this.setState({hoverIndex:-1,tipShowed:!1,specialChildTipShowed:!1}),0)},this.setMultiDial=e=>{this.multiDial=e},this.clickBtn=async(e,t,a)=>{const n=AU;if(ve.IOh.includes(e)||n.includes(e)||"KEY_APPLICATION"===e&&this.props.isHyperShiftOn||"macro"===this.props.displayMode&&["DIAL_LEFT_ROTATION","DIAL_CLICK","DIAL_RIGHT_ROTATION"].includes(e)||(0,Me.QY)(e,this.props.mappingList))return;if(this.props.isMappingChanged&&this.props.activeButton)return this.buttonKey=e,void(this.refToSet=a.current);let o=this.props.buttonList.find(t=>t.buttonKey===e),i=this.props.keyProperty;if(!(o.disableHypershiftMapping&&this.props.isHyperShiftOn&&"multi"!==i||!o.isEnabled)){if(!this.props.isSelectAll){let e,t;if(o.actuation)t=function(){let e=arguments.length>0&&void 0!==arguments[0]?arguments[0]:{};if(!e||0===Object.keys(e).length)return{};const t=e=>{if("number"!==typeof e)return e;const t=e/10;return Math.min(t,ve.q3X)};return(0,U.A)((0,U.A)({},e),{},{makeSensitivity:t(e.makeSensitivity),breakSensitivity:t(e.breakSensitivity)})}(o.actuation.rapidTrigger),e={0:o.actuation.actuation,1:this.props.actuation[1]};else{t={},e={0:this.props.defaultActuation,1:this.props.actuation[1]}}this.props.setDefaultRapidTrigger(t),this.props.setDefaultActuation(e)}this.props.setActiveSelectedKeys(e)}},this.setCtx=e=>{this.ctx=e},this.hideTip=()=>{this.setState({tipShowed:!1})},this.setTipDom=e=>{this.tipDom=e},this.setDom=e=>{this.canvasDom=e,this.rect=this.canvasDom.getBoundingClientRect()},this.setSpecialTipDom=e=>{this.specialTipDom=e},this.handleClick=e=>{if(0===(null===e||void 0===e?void 0:e.button)&&this.state.showClearSelection&&this.setState({showClearSelection:!1}),2!==(null===e||void 0===e?void 0:e.button))return!1;{let t=e.pageX,a=e.pageY;this.x=t,this.y=a,this.configDom&&this.configDom.current&&this.configDom.current.contains(e.target)&&this.setState({showClearSelection:!0})}},this.state={hyperShiftOn:!1,hoverIndex:-1,activeButton:-1,tipShowed:!1,activeHoverKey:{},hoverKeyMapping:{},specialTipShowed:!1,specialChildTipShowed:!1,wrist:!0,showClearSelection:!1},this.configDom=f().createRef(),this.leftCol=f().createRef(),this.rightCol=f().createRef(),this.multimediaRef=f().createRef(),this.leftCol=0,this.rightCol=0,this.mappings=[],this.filteredMappings=[],this.tipDom=null,this.specialTipDom=null,this.currentKey=null,this.refToSet=null}componentDidUpdate(e){this.state.wrist||e.wristrestConnected===this.props.wristrestConnected||this.setState({wrist:!0}),e.selectedProfileGuid!==this.props.selectedProfileGuid&&this.props.setSelectedAndDeselectedAllKeys(!1,[])}componentDidMount(){var e;this.setState({wrist:this.props.wristrestConnected}),window.addEventListener("resize",this.handleResize),document.getElementById("body-wrapper").addEventListener("scroll",this.handleResize),null===(e=document.querySelector(".actuation-config-wrapper"))||void 0===e||e.addEventListener("mousedown",this.handleClick),document.addEventListener("keydown",this.keyDownHandler)}componentWillUnmount(){var e;window.removeEventListener("resize",this.handleResize),null===(e=document.querySelector(".actuation-config-wrapper"))||void 0===e||e.removeEventListener("mousedown",this.handleClick),document.removeEventListener("keydown",this.keyDownHandler)}renderClearSelection(){return(0,un.jsxs)("div",{className:"clear-section ".concat(this.state.showClearSelection?"show":""),style:{top:this.y,left:this.x},onClick:()=>{this.props.setSelectedAndDeselectedAllKeys(!1,[]),this.setState({showClearSelection:!1})},children:[(0,un.jsx)(Sn.A,{text:be.Lwo}),(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.IY})})]})}render(){var e;const t=this.setMultiDial;let a="wrist-rest";return a+=this.props.wristrestConnected?" show":" hide",(0,un.jsxs)(ZE,{children:[this.renderClearSelection(),(0,un.jsxs)("div",{className:"config-wrapper actuation-config-wrapper",style:{position:"relative"},children:[(0,un.jsxs)("div",{className:"config-block actuation-config-block",ref:this.configDom,children:[(0,un.jsx)(cU,{mappingList:this.props.mappingList,hoverKeyMapping:this.state.hoverKeyMapping?this.state.hoverKeyMapping:this.state.activeHoverKey,activeKey:this.state.activeHoverKey,setTipDom:this.setTipDom,show:this.state.tipShowed,currentKey:this.currentKey,isHyperShiftOn:this.props.isHyperShiftOn,category:_e.DeviceInfo.category}),"KEYPAD"===_e.DeviceInfo.category?(0,un.jsx)(aU,{defaultActuation:this.props.defaultActuation,mappingList:this.props.mappingList,buttonList:this.props.buttonList,clickBtn:this.clickBtn,hoverBtn:this.hoverBtn,leaveBtn:this.leaveBtn,setMultiDial:t,isActuationTab:!0,minHeight:this.props.minHeight||450,minWidth:this.props.minWidth||350}):(0,un.jsx)(OM,{defaultActuation:this.props.defaultActuation,mappingList:this.props.mappingList,buttonList:this.props.buttonList,clickBtn:this.clickBtn,hoverBtn:this.hoverBtn,leaveBtn:this.leaveBtn,setMultiDial:t,isActuationTab:!0,keyHoverAndRemappedColor:this.props.keyHoverAndRemappedColor,isSvg:this.props.isSvg,minHeight:this.props.minHeight,minWidth:this.props.useProductSvg&&_e.SVG_CONFIG?null===(e=_e.SVG_CONFIG)||void 0===e?void 0:e.minWidth:730,width:this.props.width}),_e.DeviceInfo.UseSVGForProductImage?null:(0,un.jsx)(bM,{pid:_e.DeviceInfo.productId,editionId:this.props.editionId,layoutId:this.props.layoutId,alt:kn.onLang(this.props.lang).productName,category:_e.DeviceInfo.category,isSvg:this.props.useProductSvg,extraClass:this.props.useProductSvg?"svg-image":""}),this.state.wrist?(0,un.jsx)("div",{className:a}):null]}),(0,un.jsx)("div",{className:"dim-corner"})]}),(0,un.jsx)(nU,{})]})}}
```

### uU

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7292749,7293166)?UTF-16 [7292750,7293167)?

```javascript
class uU extends g.Component{constructor(e){super(e),this.handleToggleContinuous=LE()(()=>{this.props.adjustmentModeRunning?this.props.onShowAdjustmentModePrompt(!0):this.props.setContinousRapidTriggerEnable(!this.props.isEnabled)},500)}render(){return(0,un.jsx)(JE,{title:be.W0q,hasSwitch:!0,toggleSwitch:this.handleToggleContinuous,active:this.props.isEnabled,tips:be.QjA,children:(0,un.jsx)(Sn.A,{text:be.gsQ})})}}
```

### DU

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7293926,7294921)?UTF-16 [7293927,7294922)?

```javascript
DU=()=>{const e=(0,H.useDispatch)(),t=(0,H.useSelector)(e=>e.snapTapReducer.isEnabled),a=(0,H.useSelector)(e=>e.snapTapReducer.keyList),n=(0,H.useSelector)(e=>e.customizeReducer.adjustmentModeRunning);(0,g.useEffect)(()=>{Object.assign(window,{getKeyboardLayout:()=>Ln.A.keyboardLayout}),e((0,RN.getSystemKeyboardLayout)())},[]);const o=LE()(()=>{n?e((0,ss.onShowAdjustmentModePrompt)(!0)):(e((0,Um.vU)(!t)),"KEYBOARD"===_e.DeviceInfo.category&&(0,Fm.z6)({enabled:!t,label:Bm.R.ENABLE,snapTapPairs:!1===!t?[]:a}),"KEYPAD"===_e.DeviceInfo.category&&(0,kD.Wk)({enabled:!t,label:Bm.R.ENABLE,snapTapPairs:!1===!t?[]:a}))},500);return(0,un.jsxs)(JE,{title:be.Fqe,hasSwitch:!0,toggleSwitch:o,active:t,supportsShortcut:_e.DeviceInfo.category!==ve.qjw,renderShortcut:(0,un.jsx)(NU,{keyCombinations:["FN","L SHIFT"],tip:be.cyD}),extraClass:"snap-tap-widget",children:[(0,un.jsx)("span",{children:(0,un.jsx)(Sn.A,{text:be.PVl})}),(0,un.jsx)("section",{className:"snap-tap",children:(0,un.jsx)(jm,{})})]})}
```

### jm

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7096250,7100685)?UTF-16 [7096251,7100686)?

```javascript
jm=()=>{const e=(0,H.useDispatch)(),t=(0,H.useSelector)(e=>e.snapTapReducer),a=t.isEnabled,n=t.keyList,o=(0,H.useSelector)(e=>{var t;return null===(t=e.modTapReducer)||void 0===t?void 0:t.isEnabled}),i=(0,H.useSelector)(e=>e.snapTapReducer.pressedKeys),s=(0,H.useSelector)(e=>e.customizeReducer.adjustmentModeRunning),r=(0,H.useSelector)(e=>e.customizeReducer.selectedTab),E=(0,g.useState)(Vm.READY),_=(0,pn.A)(E,2),T=_[0],c=_[1],I=(0,g.useState)([fm.READY,fm.READY]),A=(0,pn.A)(I,2),l=A[0],O=A[1],d=(0,g.useState)(gm.INTRODUCTION),S=(0,pn.A)(d,2),u=S[0],R=S[1],N=(0,g.useState)(n[0]),D=(0,pn.A)(N,2),C=D[0],p=D[1],L=(0,g.useRef)(null),m=(0,H.useSelector)(e=>e.deviceReducer.layoutId),P=(0,g.useRef)(null),M=(0,H.useSelector)(e=>e.customizeReducer.buttonList),h=(0,g.useMemo)(()=>{if(!M||!M.length)return;const e=["DKM_D2","DKM_F6"];return M.some(t=>e.includes(t.inputID))},[M]);(0,g.useEffect)(()=>((e=>{var t;null===(t=window.mwBroadcastChannel)||void 0===t||t.postMessage({type:de.QXP,payload:e})})(a),()=>{(()=>{var e;null===(e=window.mwBroadcastChannel)||void 0===e||e.postMessage({type:de.Bkx})})()}),[a]),(0,g.useEffect)(()=>{const e=i.find(e=>e.inputID===(null===C||void 0===C?void 0:C.key1)),t=i.find(e=>e.inputID===(null===C||void 0===C?void 0:C.key2)),a=[e?e.snaptap?fm.SNAPPED:fm.ACTIVE:fm.READY,t?t.snaptap?fm.SNAPPED:fm.ACTIVE:fm.READY];O(a)},[i]),(0,g.useEffect)(()=>(r!==ve.wnw.ACTUATION&&i.length>0&&e((0,Um.Lq)()),()=>{F()}),[r,e]),(0,g.useEffect)(()=>{p(n[0])},[JSON.stringify(n)]),(0,g.useEffect)(()=>(window.addEventListener("keyup",B,!0),window.addEventListener("blur",v),Qt.A.on("inputredirect",y),document.addEventListener("mousedown",f),()=>{window.removeEventListener("keyup",B,!0),document.removeEventListener("mousedown",f),window.removeEventListener("blur",v),Qt.A.off("inputredirect",y)}),[JSON.stringify(l),T,JSON.stringify(C),u]);const f=t=>{if(P.current instanceof Node&&t.target instanceof Node&&P.current&&!P.current.contains(t.target)){if(u===gm.WARNING)return;e((0,Um.gr)([C])),T!==Vm.READY&&C&&b([C]),F()}};(0,g.useEffect)(()=>{if(!a&&T!==Vm.READY){F();const t=(0,U.A)((0,U.A)({},n[0]),T===Vm.KEY1?{key2:null===C||void 0===C?void 0:C.key2}:{key1:null===C||void 0===C?void 0:C.key1});p(t),e((0,Um.gr)([t])),R(gm.SUCCESS),b([t]),L.current=setTimeout(()=>{R(gm.INTRODUCTION)},5e3)}},[a,Vm,JSON.stringify(n)]);const G=(0,g.useMemo)(()=>({onInputRedirectEvent:e=>{if(T===Vm.READY)return;const t=JSON.parse(e.input);if(!Km.includes(t.type))return;const a=Mr.Rk.find(e=>{const a="number"===typeof e.outputFlag?e.outputFlag:e.flag;return t.scancode===e.scancode&&(a===t.flag||a+1===t.flag)});o&&wm.includes(a.inputID)||a&&(e=>e.flag%2===1)(t)&&w(a)},onWindowBlur:()=>{T!==Vm.READY&&(p(n[0]),R(gm.INTRODUCTION),F())}}),[T,JSON.stringify(l),JSON.stringify(C)]),y=G.onInputRedirectEvent,v=G.onWindowBlur,F=()=>{c(Vm.READY),Ln.A.callElectronAction({action:"unRegisterNoBrowserInputHandler"}),Qt.A.enableMapping(),Pm()},B=e=>{e.preventDefault();const t=Lm(e);t&&w(t)},b=e=>{"KEYBOARD"===km&&(0,Fm.z6)({enabled:!0,label:Bm.R.CHANGE,snapTapPairs:e}),"KEYPAD"===km&&(0,kD.Wk)({enabled:!0,label:Bm.R.CHANGE,snapTapPairs:e})},w=t=>{const a=t.inputID;switch(T){case Vm.KEY1:{const e=(0,U.A)((0,U.A)({},C),{},{key1:a});if(p(e),zm(t,e))return void R(gm.WARNING);c(Vm.KEY2),R(gm.NO_MESSAGE)}break;case Vm.KEY2:{const n=(0,U.A)((0,U.A)({},C),{},{key2:a});if(R(gm.SUCCESS),p(n),zm(t,n))return void R(gm.WARNING);e((0,Um.gr)([n])),b([n]),F(),L.current=setTimeout(()=>{R(gm.INTRODUCTION)},5e3)}}};return(0,un.jsxs)("div",{children:[(0,un.jsxs)("div",{className:"create-snaptap ".concat(a?"":"disable"),children:[(0,un.jsx)("div",{className:"create-snaptap-header",onClick:()=>{s?e((0,ss.onShowAdjustmentModePrompt)(!0)):a&&T===Vm.READY&&(clearTimeout(L.current),c(Vm.KEY1),R(gm.NO_MESSAGE),O([fm.READY,fm.READY]),Qt.A.disableMapping(),mm())},children:(0,un.jsx)(hm,{tip:"KEYPAD"===km?be.nns:h?be.qI5:be.f_m,children:(0,un.jsx)("div",{className:"create-snaptap-header-title ".concat(T===Vm.READY&&a?"":"disable"),children:(0,un.jsx)(Sn.A,{text:be.EAG})})})}),(0,un.jsxs)("div",{className:"key-record",ref:P,children:[(0,un.jsx)(Gm,{isRecording:a&&T===Vm.KEY1,idleState:l[0],name:(0,vm.A)(m,null===C||void 0===C?void 0:C.key1),isWarning:u===gm.WARNING}),(0,un.jsx)(Gm,{isRecording:a&&T===Vm.KEY2,idleState:l[1],name:(0,vm.A)(m,null===C||void 0===C?void 0:C.key2),isWarning:u===gm.WARNING})]})]}),(0,un.jsx)(xm,{status:u})]})}
```

### Gm

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7094952,7095522)?UTF-16 [7094953,7095523)?

```javascript
Gm=e=>{let t=e.isRecording,a=e.idleState,n=e.name,o=e.isWarning,i=e.onEdit;const s=(0,g.useMemo)(()=>t?o?"blink-warning":"blink-active":"",[t,o]),r=(0,g.useMemo)(()=>t||o?"key-record-item-editing ".concat(o?"key-record-item-editing-warning":"key-record-item-editing-active"):"key-record-item-assignment ".concat(a===fm.ACTIVE?"key-record-item-assignment-active":""," ").concat(a===fm.SNAPPED?"key-record-item-assignment-deactive":""),[t,a,o]);return(0,un.jsx)("div",{className:"key-record-item ".concat(r),onClick:i,children:(0,un.jsx)("div",{className:s,children:n})})}
```

### hm

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7094315,7094599)?UTF-16 [7094316,7094600)?

```javascript
hm=e=>{let t=e.children,a=e.tip;return(0,un.jsxs)("div",{className:"st-wrapper",children:[t,(0,un.jsxs)("div",{className:"st-tooltip",children:[(0,un.jsx)("div",{className:"st-tooltip-help"}),(0,un.jsx)("div",{className:"st-tooltip-content",children:(0,un.jsx)(Sn.A,{text:a})})]})]})}
```

### nU

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7275560,7276626)?UTF-16 [7275561,7276627)?

```javascript
nU=(0,H.connect)(e=>({profiles:e.profileReducer.profiles,lang:e.languageReducer.lang,productName:e.deviceReducer.productName,buttonList:e.actuationComponentReducer.buttonList,mappingList:e.customizeReducer.mappingList}),e=>(0,v.zH)((0,U.A)({},lr),e))(e=>{const t=e.buttonList,a=e.setSelectedAndDeselectedAllKeys,n=e.mappingList,o=(0,g.useState)({selectAll:!0,deselectAll:!1}),i=(0,pn.A)(o,2),s=i[0],r=i[1];(0,g.useEffect)(()=>{let e=(0,U.A)({},s);const a=(0,Me.hF)(t).filter(e=>!(0,Me.QY)(e.buttonKey,n));e=a.every(e=>e.isFocused)?{selectAll:!0,deselectAll:!1}:a.every(e=>!e.isFocused)?{selectAll:!1,deselectAll:!0}:{selectAll:!1,deselectAll:!1},r(e)},t);return(0,un.jsxs)("div",{className:"key-selection-component",children:[(0,un.jsx)("div",{className:"selection-btn ".concat(s.selectAll?"disabled":""),onClick:()=>{let e=[];e=t.filter(e=>!!(0,Me.QY)(e.inputID,n)),a(!0,e)},children:(0,un.jsx)(Sn.A,{text:be.S$g})}),(0,un.jsx)("div",{className:"selection-btn ".concat(s.deselectAll?"disabled":""),onClick:()=>a(!1,[]),children:(0,un.jsx)(Sn.A,{text:be.XcT})})]})})
```

### Ja

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [6586695,6587672)?UTF-16 [6586695,6587672)?

```javascript
Ja=(e,t)=>{var a,n,o;let i=structuredClone(t.payload);return i=i.reduce((e,t)=>{var a;null!==t&&void 0!==t&&t.mapping||e.push(t);const n=null!==t&&void 0!==t&&t.mapping?null===t||void 0===t?void 0:t.mapping[0]:[],o=null!==t&&void 0!==t&&t.mapping?null===t||void 0===t?void 0:t.mapping[1]:[],i=$a(n,o);return n&&(void 0!==n.rapidTrigger||"disableGroup"===(null===n||void 0===n?void 0:n.outputType)||null!==ya&&void 0!==ya&&ya.resetActuationWithHypershift&&(null===(a=n.actuationPoint)||void 0===a?void 0:a[0])!==Va||n[null===n||void 0===n?void 0:n.outputType]&&Object.keys(n[null===n||void 0===n?void 0:n.outputType]).length>0)&&!i&&(n.actuationPoint[0]=Va,e.push(t)),i&&e.push(t),e},[]),null===(a=window.mwBroadcastChannel)||void 0===a||a.postMessage({type:"ON_SET_KEYMAPPING",timerTick:void 0,payload:{mappingList:i}}),(0,U.A)((0,U.A)({},e),{},{actuation:null===ya||void 0===ya||null===(n=ya.analogSpecs)||void 0===n||null===(o=n.actuationInfo)||void 0===o?void 0:o.default})}
```

### actual-route

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7314260,7314800)?UTF-16 [7314261,7314801)?

```javascript
onent()},this.triggerUpdate=()=>{Si(Be.qg.UI_REQUEST_MW,"runUpdate")};var t=[{id:1,name:be.M9m,renderComponent:()=>(0,un.jsx)(Gh,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef})},{id:2,name:be.$so,renderComponent:()=>(0,un.jsx)(pU,{setDisplaySaveAlertRef:this.setDisplaySaveAlertRef,buttonList:this.props.buttonList})},{id:3,name:be.lgc,renderComponent:()=>(0,un.jsx)(em,{})},{id:4,name:be._$r,renderComponent:()=>(0,un.jsx)(uN,{resetObm:this.props.obmResetDevice})}];const a=JSON.parse(sessionStorage.getItem(ve.sJv))||[],n=JSON.parse
```

### snap-default-and-state

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [6593683,6593795)?UTF-16 [6593683,6593795)?

```javascript
En={isEnabled:!1,keyList:[{key1:"KEY_A",key2:"KEY_D",id:1,mode:"LAST_INPUT"}],pressedKeys:[]};const _n={isEnable
```

### snap-reducer

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [6600514,6603730)?UTF-16 [6600514,6603730)?

```javascript
snapTapReducer:function(){let e=arguments.length>0&&void 0!==arguments[0]?arguments[0]:En,t=arguments.length>1?arguments[1]:void 0;switch("SYSTEM"===(null===_e.DeviceInfo||void 0===_e.DeviceInfo?void 0:_e.DeviceInfo.category)&&(0,je.IX)(t,e,"snapTapReducer"),t.type){case de.Cn7:return(0,U.A)((0,U.A)({},e),{},{keyList:t.payload});case de.Gno:{const a=(0,U.A)((0,U.A)({},e),{},{keyList:t.payload});return window.mwBroadcastChannel.postMessage({type:"ON_SET_SNAP_TAP",payload:a}),a}case de.JzR:{const a=(0,U.A)((0,U.A)({},e),{},{keyList:t.payload});return window.mwBroadcastChannel.postMessage({type:"ON_SET_OBM_SNAP_TAP",payload:a}),a}case de.K25:{const a=(0,U.A)((0,U.A)({},e),{},{isEnabled:t.payload});return window.mwBroadcastChannel.postMessage({type:"ON_SET_SNAP_TAP",payload:a}),a}case de.ptM:{const a=(0,U.A)((0,U.A)({},e),{},{isEnabled:t.payload});return window.mwBroadcastChannel.postMessage({type:"ON_SET_SNAP_TAP",payload:a}),a}case de.tv4:return(0,U.A)((0,U.A)({},e),{},{isEnabled:t.payload.isEnabled,keyList:t.payload.keyList});case de.PFu:{const a=t.payload,n=a.inputID,o=a.input,i=a.snaptap,s=e.keyList[0],r=s.key1,E=s.key2;if(n===r||n===E){const t=structuredClone(e.pressedKeys),a=t.findIndex(e=>e.inputID===n);return i?a>=0&&(1===(null===o||void 0===o?void 0:o.flag)||3===(null===o||void 0===o?void 0:o.flag)?t[a].snaptap=!0:t[a].snaptap=!1):1===(null===o||void 0===o?void 0:o.flag)||3===(null===o||void 0===o?void 0:o.flag)?a>=0&&t.splice(a,1):a>=0?t[a]={inputID:n,snaptap:i}:t.push({inputID:n,snaptap:!1}),(0,U.A)((0,U.A)({},e),{},{pressedKeys:t})}return e}case de.iwq:{if(e.isEnabled)return e;const a=t.payload,n=a.flag,o=a.scancode;if("undefined"===typeof o)return e;const i=(0,rn.nt)(o),s=e.keyList[0],r=n%2===0;if(![s.key1,s.key2].includes(i))return e;let E=structuredClone(e.pressedKeys);const _=E.findIndex(e=>e.inputID===i);return r?-1===_&&E.push({inputID:i,snaptap:!1}):_>=0&&E.splice(_,1),(0,U.A)((0,U.A)({},e),{},{pressedKeys:E})}case de.B8M:return(0,U.A)((0,U.A)({},e),{},{pressedKeys:[]});default:return e}},continuousRapidTriggerReducer:function(){let e=arguments.length>0&&void 0!==arguments[0]?arguments[0]:_n,t=arguments.length>1?arguments[1]:void 0;switch(t.type){case de.dGp:return window.mwBroadcastChannel.postMessage({type:"ON_TOGGLE_CONTINUOUS_RAPID_TRIGGER",payload:{value:t.payload}}),(0,U.A)((0,U.A)({},e),{},{isEnabled:t.payload});case de.uW_:return(0,U.A)((0,U.A)({},e),{},{isEnabled:t.payload});default:return e}}}),cn=[b],In=v.Zz,An=(0,v.y$)(Tn,{},In((0,v.Tw)(...cn)));window.reduxStore=An;const ln=An;var On=a(68742),dn=a.n(On),Sn=a(10264),un=a(92722);class Rn extends g.Component{constructor(e){super(e),this.changeView=e=>{this.props.changeView(e)},this.onKeyDown=e=>{const t=this.props,a=t.name,n=t.changeView;"Enter"===e.code&&n(a)},this.state={active:this.props.active}}componentDidUpdate(e){const t=this.props.active;t!==e.active&&this.setState({active:t})}render(){const e=this.props,t=e.name,a=e.ariaAttributes,n=this.state.active;let o="nav ".concat(n?"active":""," ").concat(this.props.extraClass?this.props.extraClass:"");return(0,un.jsx)("div",{onKeyDown:this.onKeyDown,className:o,onClick:this.changeView.bind(this,t),role:"tab","aria-selected":null===a
```

### input-redirect-helpers-and-tables

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [7092450,7096250)?UTF-16 [7092451,7096251)?

```javascript
FT"],ENTER:["KEY_ENTER","KEY_NUMPAD_ENTER"]},Lm=e=>{const t={ScrollLock:"KEY_SCROLL_LOCK",Pause:"KEY_PAUSE",NumLock:"KEY_NUMPAD_NUM_LOCK"};if(t[e.code])return Mr.Rk.find(a=>a.inputID===t[e.code]);const a=Mr.Rk.filter(t=>"".concat(e.keyCode)===t.keyCode);if(0===a.length)return null;if(1===a.length)return a[0];for(const n of Object.entries(pm)){const t=(0,pn.A)(n,1)[0];if(t.includes(a[0].inputID)){const n=[0,1].includes(e.location)?t[0]:t[1];return a.find(e=>e.inputID===n)}}return a[0]},mm=async function(){let e=arguments.length>0&&void 0!==arguments[0]&&arguments[0],t=arguments.length>1&&void 0!==arguments[1]&&arguments[1];const a=e?await(new Rm.yD).getDevices():await Cm.getRazerDevices();if(!Array.isArray(a))return;let n=_e.DeviceInfo.productId;e&&(n=_e.DeviceInfo.bleId),t&&(n=_e.DeviceInfo.dongleId),Mm.isDuallink&&(n=Mm.masterDevicePID);const o=a.find(e=>e.productId===n);if(!o)return;await Qt.A.isInputRedirectEnabled(o)||await Qt.A.enableInputRedirect(o)},Pm=async function(){let e=arguments.length>0&&void 0!==arguments[0]&&arguments[0],t=arguments.length>1&&void 0!==arguments[1]&&arguments[1];const a=e?await(new Rm.yD).getDevices():await Cm.getRazerDevices();if(!Array.isArray(a))return;let n=_e.DeviceInfo.productId;e&&(n=_e.DeviceInfo.bleId),t&&(n=_e.DeviceInfo.dongleId),Mm.isDuallink&&(n=Mm.masterDevicePID);const o=a.find(e=>e.productId===n);if(!o)return;await Qt.A.isInputRedirectEnabled(o)&&await Qt.A.disableInputRedirect(o)},Mm=(()=>{var e;let t=[];try{t=Object.values(JSON.parse(localStorage.getItem("duallink-devices")||"{}"))}catch(o){console.error("Failed to parse duallink-devices from localStorage",o)}const a=t.find(e=>e.dongleId===(null===_e||void 0===_e?void 0:_e.DeviceInfo.dongleId)),n={isDuallink:!1,masterDevicePID:null};return a?(n.isDuallink=!0,n.masterDevicePID=null===(e=a.master)||void 0===e?void 0:e.dongleId,n):n})(),hm=e=>{let t=e.children,a=e.tip;return(0,un.jsxs)("div",{className:"st-wrapper",children:[t,(0,un.jsxs)("div",{className:"st-tooltip",children:[(0,un.jsx)("div",{className:"st-tooltip-help"}),(0,un.jsx)("div",{className:"st-tooltip-content",children:(0,un.jsx)(Sn.A,{text:a})})]})]})};var Um=a(90492);const gm=Object.freeze({NO_MESSAGE:0,INTRODUCTION:"introduction",DUPLICATE_KEY_WARNING:"duplicate_key_warning",ASSIGNED_ANALOG_FEATURE:"assigned_analog_feature",DEFAULT_SHORTCUT_WARNING:"default_shortcut_warning",EMPTY_KEY_WARNING:"empty_key_warning",WARNING:"warning",SUCCESS:"success"}),fm=Object.freeze({READY:0,ACTIVE:1,SNAPPED:2}),Gm=e=>{let t=e.isRecording,a=e.idleState,n=e.name,o=e.isWarning,i=e.onEdit;const s=(0,g.useMemo)(()=>t?o?"blink-warning":"blink-active":"",[t,o]),r=(0,g.useMemo)(()=>t||o?"key-record-item-editing ".concat(o?"key-record-item-editing-warning":"key-record-item-editing-active"):"key-record-item-assignment ".concat(a===fm.ACTIVE?"key-record-item-assignment-active":""," ").concat(a===fm.SNAPPED?"key-record-item-assignment-deactive":""),[t,a,o]);return(0,un.jsx)("div",{className:"key-record-item ".concat(r),onClick:i,children:(0,un.jsx)("div",{className:s,children:n})})};var ym,Hm,vm=a(90857),Fm=a(48368),Bm=a(37773);const bm=(0,U.A)({},_e).MOD_TAP_KEYS,wm=void 0===bm?[]:bm,Vm=Object.freeze({READY:0,KEY1:1,KEY2:2}),Ym=Object.freeze({introduction:"TEST_SNAP_TAP_INTRODUCTION",warning:"CREATE_SNAP_TAP_INFO_MESSAGE",success:"CREATE_SNAP_TAP_SUCCESS_MESSAGE"}),Km=["keyboard","analogKey"],Wm=["KEY_APPLICATION","KEY_LEFT_GUI"],km=null!==(ym=null===_e||void 0===_e||null===(Hm=_e.DeviceInfo)||void 0===Hm?void 0:Hm.category)&&void 0!==ym?ym:"",zm=(e,t)=>{const a=Wm.includes(e.inputID),n=t.key1===t.key2;return a||n},xm=e=>{let t=e.status;return t===gm.NO_MESSAGE?null:(0,un.jsx)("span",{className:"create-snaptap-message create-snaptap-message--".concat(t),children:(0,un.jsx)(Sn.A,{text:Ym[t]})})},
```

### 62905-actions

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [5294790,5296380)?UTF-16 [5294790,5296380)?

```javascript
),t)}}const I=c},62905:(e,t,a)=>{"use strict";a.r(t),a.d(t,{resetActuation:()=>l,resetSelectedButtonList:()=>S,setActiveSelectedKeys:()=>r,setActuation:()=>c,setActuationAllKeys:()=>I,setActuationPreconfig:()=>T,setActuationToBtnList:()=>_,setDefaultActuation:()=>s,setSelectedAndDeselectedAllKeys:()=>E,syncActuationKeys:()=>A,updateActuationButtonList:()=>O,updateAnalogKeyEvent:()=>d});var n=a(13254);let o=null;const i=e=>{null!=o&&(clearTimeout(o),o=null),e({type:n.Zlu,payload:!0}),o=setTimeout(()=>{o=null,e({type:n.Zlu,payload:!1})},2e3)},s=e=>t=>{t({type:n.IsE,payload:e})},r=e=>t=>{t({type:n.T4X,payload:e})},E=(e,t)=>a=>{a({type:n.bAe,payload:{isSelectAll:e,disableActuationMappings:t}})},_=e=>t=>{t({type:n.iBu,payload:e})},T=e=>t=>{t({type:n.q_k,payload:e})},c=(e,t)=>a=>{a({type:n.mi_,payload:{actuationButtonList:e,mappingList:t}}),i(a)},I=e=>t=>{t({type:n.fZZ,payload:e}),i(t)},A=(e,t,a,o)=>s=>{s({type:n.c2U,payload:{option:e,mappingList:t,actuation:a,disableActuationMappings:o}}),i(s)},l=e=>t=>{t({type:n.hlp,payload:e}),i(t)},O=e=>t=>{t({type:n.r9V,payload:e})},d=e=>t=>{t({type:n.NB4,payload:e})},S=function(){let e=arguments.length>0&&void 0!==arguments[0]?arguments[0]:[];return t=>{t({type:n.qXn,payload:e})}}},63107:(e,t,a)=>{var n=a(35764),o=a(116);e.exports=function(e){return o(e)&&n(e)}},63249:(e,t,a)=>{"use strict";a.r(t),a.d(t,{TIME_END:()=>ne,TIME_START:()=>ae,bin2dec:()=>R,bin2hex:()=>N,bytes2HexString:()=>M,bytesToFloat:()=>B,bytesToString:()=>U,compareVersions:()=>J,convertByteArrayToHexString:()=>P,convertHexStringToByteArray:()=>g,convertNumArrayTo
```

### shared-alias-generations

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js` SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

Unicode [5235224,5235537)?UTF-16 [5235224,5235537)?

```javascript
60481:(e,t,a)=>{"use strict";a.d(t,{T:()=>r,r:()=>s});var n,o,i=a(78193);const s="analogV1"===(null===(n=i.DeviceInfo)||void 0===n?void 0:n.AnalogGenVersion),r="analogV2"===(null===(o=i.DeviceInfo)||void 0===o?void 0:o.AnalogGenVersion)},60788:e=>{e.exports=function(){this.__data__=[],this.size=0}},60860:(e,t,a)
```

## CSS ????

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [19898,20128)?UTF-16 [19898,20128)?

```css
.clear-section{align-items:center;background-color:#5d5d5d;border-radius:3px;color:#ccc;display:flex;height:35px;justify-content:space-between;max-width:250px;opacity:0;padding:10px;position:absolute;visibility:hidden;width:250px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [20128,20159)?UTF-16 [20128,20159)?

```css
.clear-section>span{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [20159,20220)?UTF-16 [20159,20220)?

```css
.clear-section.show{opacity:1;visibility:visible;z-index:999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [20220,20266)?UTF-16 [20220,20266)?

```css
.clear-section:hover{background-color:#2d2d2d}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [20528,20561)?UTF-16 [20528,20561)?

```css
div.nav-tabs.disabled{opacity:.5}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [24721,24766)?UTF-16 [24721,24766)?

```css
.nav.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [33770,33823)?UTF-16 [33770,33823)?

```css
.s3-dropdown.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [37792,37932)?UTF-16 [37792,37932)?

```css
.dropdown-razer-2 .dropdown-item-2.disabled,.dropdown-razer-2 .dropdown-item.disabled{background-color:#0000;opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [38306,38374)?UTF-16 [38306,38374)?

```css
.s3-dropdown-with-dual-item.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [53699,53740)?UTF-16 [53699,53740)?

```css
.profile-act .action.disabled{opacity:.3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [71315,71385)?UTF-16 [71315,71385)?

```css
.body-widgets .widget.actuation-widget>div{will-change:auto!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [71520,71556)?UTF-16 [71520,71556)?

```css
.widget-tooltip.disabled{opacity:.5}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [71666,71725)?UTF-16 [71666,71725)?

```css
.idleEffect .widget.disabled{opacity:1;pointer-events:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [73376,73449)?UTF-16 [73376,73449)?

```css
.widget .titleRow .shortcuts{align-items:center;display:flex;height:27px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [73449,73629)?UTF-16 [73449,73629)?

```css
.widget .titleRow .shortcuts .shortcutButton{align-items:center;border:1px solid #5d5d5d;border-radius:3px;display:flex;height:100%;justify-content:center;margin:0 10px;width:47px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [73629,73706)?UTF-16 [73629,73706)?

```css
.widget .titleRow .shortcuts .shortcutButton-filled{background-color:#292929}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [74704,74833)?UTF-16 [74704,74833)?

```css
.widget .exclamation:before,.widget .help{background-repeat:no-repeat;border-radius:7px;height:14px;position:absolute;width:14px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [74833,75028)?UTF-16 [74833,75028)?

```css
.widget .help{background-color:#4a4a4a;background-image:url(../../static/media/tooltip_questionmark.96138d2f.svg);right:10px;top:10px;transition:background-color .3s;will-change:background-color}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [75028,75075)?UTF-16 [75028,75075)?

```css
.widget .help:hover{background-color:#ffffff4d}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [75075,75365)?UTF-16 [75075,75365)?

```css
.factory-default .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:465px;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [75365,75434)?UTF-16 [75365,75434)?

```css
.factory-default .warning-alert .title{color:#ccc;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [75434,75723)?UTF-16 [75434,75723)?

```css
.mode-switcher .warning-alert{align-items:center;background:#111 0 0 no-repeat padding-box;border:1px solid #fd8611;border-radius:3px;box-shadow:0 6px 10px #0003;height:-webkit-fit-content;height:fit-content;left:50%;opacity:1;padding:20px;text-align:center;top:45%;width:360px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [75723,75806)?UTF-16 [75723,75806)?

```css
.mode-switcher .warning-alert .title{color:#fd8611;display:flex;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [75806,75865)?UTF-16 [75806,75865)?

```css
.mode-switcher .warning-alert .title span{margin-right:4px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [75865,75925)?UTF-16 [75865,75925)?

```css
.mode-switcher .warning-alert .content{white-space:pre-wrap}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [76476,76542)?UTF-16 [76476,76542)?

```css
.widget .help:hover+.tip{opacity:1;visibility:visible;z-index:100}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [76542,76619)?UTF-16 [76542,76619)?

```css
.widget .help:hover+.tip.mt-tip{background-color:#0000;border:none;padding:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [77102,77209)?UTF-16 [77102,77209)?

```css
.widget-prod img.audio-left.disabled,.widget-prod img.audio-right.disabled{opacity:50%;pointer-events:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [78346,78414)?UTF-16 [78346,78414)?

```css
.h2-body.disabled,.h2-title.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [83268,83320)?UTF-16 [83268,83320)?

```css
.check-item.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [92685,92742)?UTF-16 [92685,92742)?

```css
.config-wrapper.actuation-config-wrapper{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [92742,92783)?UTF-16 [92742,92783)?

```css
.actuation-config-block{padding-top:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [92981,93090)?UTF-16 [92981,93090)?

```css
.config-wrapper .config-block .keyboard-svg .disabled{fill:#c8323c80!important;opacity:1;pointer-events:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [112582,112667)?UTF-16 [112582,112667)?

```css
.icon.spinner.down.disabled,.icon.spinner.up.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [114404,114445)?UTF-16 [114404,114445)?

```css
.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [119843,120087)?UTF-16 [119843,120087)?

```css
.remove-alert,.warning-alert{background-color:#111;border:1px solid #fd8611;border-radius:5px;color:#ccc;font-size:14px;left:50%;line-height:17px;padding:20px 30px;position:fixed;top:50%;transform:translateX(-50%) translateY(-100%);width:400px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [120087,120314)?UTF-16 [120087,120314)?

```css
.remove-alert .title,.warning-alert .title{align-items:center;color:#fd8611;display:flex;font-family:Roboto;font-size:16px;justify-content:center;line-height:16.8px;margin-bottom:20px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [120314,120524)?UTF-16 [120314,120524)?

```css
.remove-alert .title .icon,.warning-alert .title .icon{background-image:url(../../static/media/warning.ad3f47f8.svg);background-position:50%;background-repeat:no-repeat;height:25px;margin-right:10px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [120524,120647)?UTF-16 [120524,120647)?

```css
.remove-alert .body,.warning-alert .body{color:#ccc;font-family:Roboto;font-size:14px;line-height:16.8px;text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [120647,120737)?UTF-16 [120647,120737)?

```css
.remove-alert .action,.warning-alert .action{display:flex;gap:20px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [120737,121114)?UTF-16 [120737,121114)?

```css
.remove-alert .action .ok-button,.remove-alert .action .secondary-button,.warning-alert .action .ok-button,.warning-alert .action .secondary-button{align-items:center;background-color:#707070;border-radius:3px;color:#ccc;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [121114,121421)?UTF-16 [121114,121421)?

```css
.remove-alert .action .enable-button,.warning-alert .action .enable-button{align-items:center;background-color:#44d62c;border-radius:3px;color:#212121;cursor:pointer;display:flex;font-size:12px;height:27px;justify-content:center;text-transform:uppercase;-webkit-user-select:none;user-select:none;width:90px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [132576,132605)?UTF-16 [132576,132605)?

```css
.toolbar.disabled{opacity:.5}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [133112,133168)?UTF-16 [133112,133168)?

```css
.toolbar .arrow.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [145287,145395)?UTF-16 [145287,145395)?

```css
.device_linked_game .game-art .cover-art .custom-cover-art .refresh.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [148036,148080)?UTF-16 [148036,148080)?

```css
.block-input--container.disabled{opacity:.2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [148936,149047)?UTF-16 [148936,149047)?

```css
.block-input .icon.spinner.down.disabled,.block-input .icon.spinner.up.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [149184,149244)?UTF-16 [149184,149244)?

```css
.widget-col .float-tip.disabled{opacity:0;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [154665,154727)?UTF-16 [154665,154727)?

```css
.dashboard .box-item .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [162576,162686)?UTF-16 [162576,162686)?

```css
.custom-global-shortcuts .global_shortcuts_container.disabled{opacity:1;pointer-events:none;position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [164639,164711)?UTF-16 [164639,164711)?

```css
.shortcut_item.disabled{opacity:1;pointer-events:none;position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [205214,205249)?UTF-16 [205214,205249)?

```css
.stage .switch.disabled{opacity:.3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [206807,206920)?UTF-16 [206807,206920)?

```css
.icon-sensitivity-xy.disabled{background-image:url(../../static/media/icon_sensitivity_xy_disabled.3a6ee34e.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [224156,224217)?UTF-16 [224156,224217)?

```css
.main-setting .general-setting .disabled{pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [241894,241988)?UTF-16 [241894,241988)?

```css
.icon-actual-point{background-image:url(../../static/media/icon_actuation_point.bbadbed4.svg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [242073,242174)?UTF-16 [242073,242174)?

```css
.actuation-text-guide{color:#999;font-size:14px;padding-bottom:10px;width:220px;word-break:break-all}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [254721,254818)?UTF-16 [254721,254818)?

```css
.module-installation-container .module-installation .btn.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [296639,296719)?UTF-16 [296639,296719)?

```css
.widget .help.indicator-led:hover .tip{opacity:1;visibility:visible;z-index:100}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [296719,296850)?UTF-16 [296719,296850)?

```css
.widget .help.indicator-led:before{background-color:#0000;bottom:-5px;content:"";height:10px;left:5px;position:absolute;width:50px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [296850,296901)?UTF-16 [296850,296901)?

```css
.widget .help.indicator-led .tip{left:36%;top:19px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [297848,297933)?UTF-16 [297848,297933)?

```css
.indicator-led-tooltip-body .list-wrapper{display:flex;flex-direction:column;gap:6px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [297933,298018)?UTF-16 [297933,298018)?

```css
.indicator-led-tooltip-body .list-wrapper .list-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [298018,298147)?UTF-16 [298018,298147)?

```css
.indicator-led-tooltip-body .list-wrapper .list-item-dot{border-radius:50%;height:3px;margin-left:8px;margin-right:8px;width:3px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [303907,303961)?UTF-16 [303907,303961)?

```css
.widget .panel-light--description.disabled{opacity:.2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [304090,304149)?UTF-16 [304090,304149)?

```css
.widget-covering.disabled{opacity:.3;transition:opacity 1s}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [325186,325375)?UTF-16 [325186,325375)?

```css
.haptic-content .scroll-slide .scroll-item .stepper .icon.spinner.down.disabled,.haptic-content .scroll-slide .scroll-item .stepper .icon.spinner.up.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [375278,375416)?UTF-16 [375278,375416)?

```css
.dropdown-razer .dropdown-item-2.disabled,.dropdown-razer .dropdown-item.disabled{background-color:initial;opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [390626,390729)?UTF-16 [390626,390729)?

```css
.combined-key-list-wrapper{align-items:center;box-sizing:border-box;display:flex;flex-direction:column}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [391414,391466)?UTF-16 [391414,391466)?

```css
.combined-key-blink-active{padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [405908,405978)?UTF-16 [405908,405978)?

```css
.config-btns .config-btn.disabled{color:#cccccc4d;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [430733,430804)?UTF-16 [430733,430804)?

```css
.stepper.disabled,.turbo-label.disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [443624,443687)?UTF-16 [443624,443687)?

```css
.fan-mode .recommended.disabled{opacity:.2;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [445266,445366)?UTF-16 [445266,445366)?

```css
.dynamic-key-stroke-body .map-header-item-adjustment .rapid-trigger-slider-container{margin-top:5px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [445366,445467)?UTF-16 [445366,445467)?

```css
.dynamic-key-stroke-body .map-header-item-adjustment .rapid-trigger-slider-tip{background-color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [445467,445580)?UTF-16 [445467,445580)?

```css
.dynamic-key-stroke-body .map-header-item-adjustment .rapid-trigger-slider-container .track{background:#44d62c4d}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [445580,445920)?UTF-16 [445580,445920)?

```css
.dynamic-key-stroke-body .map-header-item-adjustment .rapid-trigger-slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background:#44d62c;border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [445920,446241)?UTF-16 [445920,446241)?

```css
.dynamic-key-stroke-body .map-header-item-adjustment .rapid-trigger-slider-container.on .rapid-trigger-slider::-webkit-slider-thumb:active,.dynamic-key-stroke-body .map-header-item-adjustment .rapid-trigger-slider-container.on .rapid-trigger-slider::-webkit-slider-thumb:hover{background:#44d62c;border:2px solid #44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [449344,449393)?UTF-16 [449344,449393)?

```css
.actuation-warning{color:#fd8611;margin-top:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [449393,449500)?UTF-16 [449393,449500)?

```css
.key-selection-component{display:flex;justify-content:center;margin-bottom:10px;margin-top:30px;z-index:10}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [449500,449751)?UTF-16 [449500,449751)?

```css
.key-selection-component .selection-btn{border:1px solid #9c9c9c;border-radius:3px;color:#ccc;font:normal normal normal 12px/14px Roboto;height:27px;letter-spacing:0;opacity:.3;opacity:1;padding:7px 16px 6px;text-align:center;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [449751,449830)?UTF-16 [449751,449830)?

```css
.key-selection-component .selection-btn.disabled{cursor:not-allowed;opacity:.3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [449830,449902)?UTF-16 [449830,449902)?

```css
.key-selection-component .selection-btn+.selection-btn{margin-left:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [449951,450012)?UTF-16 [449951,450012)?

```css
.actuation-point-slider{display:flex;height:204px;width:40px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [450283,450469)?UTF-16 [450283,450469)?

```css
.icon-refresh-actuation{background-image:url(../../static/media/icon_refresh_white.80aa16c3.svg);background-repeat:no-repeat;height:20px;position:absolute;right:36px;top:31px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [450469,450735)?UTF-16 [450469,450735)?

```css
.actuation-key-tip{background-color:#000;border:1px solid #5d5d5d;max-width:300px;min-height:75px;opacity:0;position:absolute;text-align:center;transition:opacity .3s,visibility 0s,left 0s,top 0s;visibility:hidden;will-change:opacity,visibility,left,top;z-index:102}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [450735,450800)?UTF-16 [450735,450800)?

```css
.actuation-key-tip.show{opacity:1;visibility:visible;z-index:999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [450800,450879)?UTF-16 [450800,450879)?

```css
.actuation-key-tip .tip-msg{font-family:Roboto;font-size:12px;padding:2px 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [450879,450945)?UTF-16 [450879,450945)?

```css
.actuation-key-tip .notmapped{background-color:#5d5d5d;color:#ccc}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [450945,451092)?UTF-16 [450945,451092)?

```css
.actuation-key-tip .index{color:#707070;font-size:14px;line-height:14px;margin-bottom:6px;margin-top:6px;padding:8px 10px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451092,451165)?UTF-16 [451092,451165)?

```css
.actuation-key-tip .values-container{display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451165,451252)?UTF-16 [451165,451252)?

```css
.actuation-key-tip .value-container{display:flex;font-size:15px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451252,451311)?UTF-16 [451252,451311)?

```css
.actuation-key-tip .value-container+.index{margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451311,451381)?UTF-16 [451311,451381)?

```css
.actuation-key-tip .value-container+.value-container{margin-left:15px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451381,451497)?UTF-16 [451381,451497)?

```css
.actuation-key-tip .key-down-icon{content:url(../../static/media/icon_key_down.7a538e15.svg);height:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451497,451609)?UTF-16 [451497,451609)?

```css
.actuation-key-tip .key-up-icon{content:url(../../static/media/icon_key_up.39a8f68e.svg);height:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451609,451874)?UTF-16 [451609,451874)?

```css
.actuation-reset-popup{align-items:center;background-color:#111;border:2px solid #ff4500;border-radius:4px;display:none;flex-direction:column;gap:10px;justify-content:center;min-width:300px;padding:20px;position:absolute;right:-55px;top:calc(100% + 5px);z-index:10}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451874,451934)?UTF-16 [451874,451934)?

```css
.actuation-reset-popup h6,.actuation-reset-popup p{margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [451934,452030)?UTF-16 [451934,452030)?

```css
.actuation-reset-popup h6{color:#ff4500;font-size:14px;font-weight:700;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452030,452073)?UTF-16 [452030,452073)?

```css
.actuation-reset-popup p{text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452073,452158)?UTF-16 [452073,452158)?

```css
.actuation-reset-popup .customize-setting-button{background-color:#ff4500;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452158,452199)?UTF-16 [452158,452199)?

```css
.actuation-reset-popup.show{display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452199,452242)?UTF-16 [452199,452242)?

```css
.actuation-reset-popup.show+.tip{opacity:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452242,452343)?UTF-16 [452242,452343)?

```css
.actuation-tip{align-items:center;display:flex;flex-direction:column;gap:16px;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452343,452658)?UTF-16 [452343,452658)?

```css
.actuation-reset-tip{background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:16px;opacity:0;padding:8px 10px;position:absolute;right:100%;text-align:left;top:100%;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:nowrap;will-change:visibility,opacity;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452658,452713)?UTF-16 [452658,452713)?

```css
.actuation-reset-tip.show{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452713,452779)?UTF-16 [452713,452779)?

```css
.body-widgets .widget>div.icon-refresh-actuation{will-change:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452803,452944)?UTF-16 [452803,452944)?

```css
.actuation-vertical-slider-container{height:64px;opacity:.3;pointer-events:none;position:relative;transition:opacity .3s;will-change:opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [452944,453008)?UTF-16 [452944,453008)?

```css
.actuation-vertical-slider-container.system{pointer-events:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453008,453078)?UTF-16 [453008,453078)?

```css
.actuation-vertical-slider-container.on{opacity:1;pointer-events:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453078,453149)?UTF-16 [453078,453149)?

```css
.actuation-vertical-slider-container.no-pointer{pointer-events:inherit}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453149,453352)?UTF-16 [453149,453352)?

```css
.actuation-vertical-slider{-webkit-appearance:none;background:#0000;border-radius:3px;bottom:25px;height:6px;margin:0;opacity:1;outline:none;position:absolute;transition:opacity .2s;width:100%;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453352,453411)?UTF-16 [453352,453411)?

```css
.actuation-vertical-slider.gradient{height:25px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453411,453475)?UTF-16 [453411,453475)?

```css
.actuation-vertical-slider-battery{left:50%;width:30%;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453475,453614)?UTF-16 [453475,453614)?

```css
.actuation-vertical-slider-left{background:#707070;border-radius:3px;bottom:25px;height:6px;left:50%;position:absolute;width:30%;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453614,453932)?UTF-16 [453614,453932)?

```css
.actuation-vertical-slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background:#ccc;background:var(--StateColor);border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [453932,454071)?UTF-16 [453932,454071)?

```css
.actuation-vertical-slider-container.on .actuation-vertical-slider::-webkit-slider-thumb:hover{background:#5d5d5d;border:2px solid #44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [454071,454211)?UTF-16 [454071,454211)?

```css
.actuation-vertical-slider-container.on .actuation-vertical-slider::-webkit-slider-thumb:active{background:#383838;border:2px solid #44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [454211,454343)?UTF-16 [454211,454343)?

```css
.actuation-vertical-thumb-tag{font-weight:700;height:16px;line-height:16px;margin-left:-8px;text-align:center;width:16px;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [454343,454471)?UTF-16 [454343,454471)?

```css
.actuation-vertical-slider-tip,.actuation-vertical-thumb-tag{color:#212121;font-size:12px;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [454471,454867)?UTF-16 [454471,454867)?

```css
.actuation-vertical-slider-tip{background-color:#fff;border-bottom-left-radius:4px;border-bottom-right-radius:5px;border-top-left-radius:4px;border-top-right-radius:5px;bottom:42px;line-height:14px;margin-bottom:-82px;margin-left:-35px;opacity:1;padding:6px 4px;transform:rotate(270deg);transition:opacity .3s;transition:left 0s,opacity 0s;width:70px;will-change:opacity;will-change:left,opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [454867,454930)?UTF-16 [454867,454930)?

```css
.actuation-vertical-slider-tip.gradient{height:22px;width:22px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [454930,455015)?UTF-16 [454930,455015)?

```css
.actuation-vertical-slider-tip .tag-container{display:flex;font-size:12px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [455015,455225)?UTF-16 [455015,455225)?

```css
.actuation-vertical-slider-tip .tag-container .upload-tag-icon{content:url(../../static/media/icon_upload_in_black.4230f4d3.svg);height:14px;margin-left:2px;margin-right:4px;transform:rotate(180deg);width:14px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [455225,455413)?UTF-16 [455225,455413)?

```css
.actuation-vertical-slider-tip:after{border-bottom:11px solid #0000;border-left:10px solid #fff;border-top:11px solid #0000;content:"";height:0;left:69px;position:absolute;top:2px;width:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [455413,455488)?UTF-16 [455413,455488)?

```css
.actuation-vertical-slider-container.actuation-vertical-no-tip{height:36px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [455488,455595)?UTF-16 [455488,455595)?

```css
.actuation-vertical-slider-container.actuation-vertical-no-tip .actuation-vertical-slider-tip{display:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [455595,455779)?UTF-16 [455595,455779)?

```css
.actuation-vertical-slider-container .actuation-vertical-active{background:#44d62c;border-radius:3px;bottom:25px;height:6px;left:0;max-width:100%;position:absolute;width:50%;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [455779,455953)?UTF-16 [455779,455953)?

```css
.actuation-vertical-slider-container .actuation-vertical-right{background:#44d62c;border-radius:3px;bottom:25px;height:6px;left:50%;max-width:30%;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [455953,456117)?UTF-16 [455953,456117)?

```css
.actuation-vertical-slider-container .actuation-vertical-track{background:#44d62c4d;border-radius:3px;bottom:25px;height:6px;position:absolute;width:100%;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456117,456310)?UTF-16 [456117,456310)?

```css
.actuation-vertical-slider-container .actuation-vertical-foot{bottom:0;opacity:1;position:absolute;transition:visibility 0s,opacity .3s linear;visibility:visible;will-change:visibility,opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456310,456411)?UTF-16 [456310,456411)?

```css
.actuation-vertical-slider-container.actuation-vertical-no-tag .actuation-vertical-foot{display:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456411,456505)?UTF-16 [456411,456505)?

```css
.actuation-vertical-slider-container.on .actuation-vertical-foot{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456505,456590)?UTF-16 [456505,456590)?

```css
.actuation-vertical-foot.min{left:-18px;top:10px;transform:rotate(270deg);width:50px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456590,456655)?UTF-16 [456590,456655)?

```css
.actuation-vertical-foot.mid{left:0;text-align:center;width:100%}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456655,456719)?UTF-16 [456655,456719)?

```css
.actuation-vertical-oot.mid1{left:0;text-align:center;width:66%}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456719,456785)?UTF-16 [456719,456785)?

```css
.actuation-vertical-foot.mid2{left:0;text-align:center;width:133%}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456785,456870)?UTF-16 [456785,456870)?

```css
.actuation-vertical-foot.max{left:208px;top:11px;transform:rotate(-90deg);width:50px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456870,456938)?UTF-16 [456870,456938)?

```css
.actuation-vertical-foot.value2{left:0;text-align:center;width:103%}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [456938,457006)?UTF-16 [456938,457006)?

```css
.actuation-vertical-foot.value3{left:0;text-align:center;width:157%}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [457006,457079)?UTF-16 [457006,457079)?

```css
.actuation-vertical-slider-container.indent{margin-left:30px;width:490px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [457079,457273)?UTF-16 [457079,457273)?

```css
.actuation-vertical-hide-slider .actuation-vertical-slider-container{height:0;margin-left:30px;opacity:0;overflow:hidden;transition:height .3s,opacity .3s;width:490px;will-change:height,opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [457273,457420)?UTF-16 [457273,457420)?

```css
.actuation-vertical-hide-slider .actuation-vertical-slider-container.on{height:64px;margin-bottom:20px;opacity:1;overflow:visible;overflow:initial}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [457420,457517)?UTF-16 [457420,457517)?

```css
.actuation-vertical-has-slider .actuation-vertical-slider-container{margin-left:30px;width:490px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458051,458187)?UTF-16 [458051,458187)?

```css
.rapid-trigger-slider-container{height:64px;opacity:.3;pointer-events:none;position:relative;transition:opacity .3s;will-change:opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458187,458246)?UTF-16 [458187,458246)?

```css
.rapid-trigger-slider-container.system{pointer-events:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458246,458311)?UTF-16 [458246,458311)?

```css
.rapid-trigger-slider-container.on{opacity:1;pointer-events:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458311,458377)?UTF-16 [458311,458377)?

```css
.rapid-trigger-slider-container.no-pointer{pointer-events:inherit}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458377,458490)?UTF-16 [458377,458490)?

```css
.body-widgets .widget>div.rapid-trigger-sensitivity-section{transition:none!important;will-change:auto!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458490,458688)?UTF-16 [458490,458688)?

```css
.rapid-trigger-slider{-webkit-appearance:none;background:#0000;border-radius:3px;bottom:25px;height:6px;margin:0;opacity:1;outline:none;position:absolute;transition:opacity .2s;width:100%;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458688,458742)?UTF-16 [458688,458742)?

```css
.rapid-trigger-slider.gradient{height:25px;width:25px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458742,458828)?UTF-16 [458742,458828)?

```css
.rapid-trigger-slider-battery,.rapid-trigger-slider-left{left:50%;width:30%;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458828,458933)?UTF-16 [458828,458933)?

```css
.rapid-trigger-slider-left{background:#707070;border-radius:3px;bottom:25px;height:6px;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [458933,459220)?UTF-16 [458933,459220)?

```css
.rapid-trigger-slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background:#fd8611;border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [459220,459435)?UTF-16 [459220,459435)?

```css
.rapid-trigger-slider-container.on .rapid-trigger-slider::-webkit-slider-thumb:active,.rapid-trigger-slider-container.on .rapid-trigger-slider::-webkit-slider-thumb:hover{background:#fd8611;border:2px solid #fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [459548,459652)?UTF-16 [459548,459652)?

```css
.rapid-trigger-slider-tip,.thumb-tag{color:#212121;font-size:12px;pointer-events:none;position:absolute}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [459652,459885)?UTF-16 [459652,459885)?

```css
.rapid-trigger-slider-tip{background-color:#fd8611;border-radius:3px;bottom:42px;line-height:14px;opacity:1;padding:4px 8px;transition:opacity .3s;transition:left 0s,opacity 0s;width:80px;will-change:opacity;will-change:left,opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [459885,459943)?UTF-16 [459885,459943)?

```css
.rapid-trigger-slider-tip.gradient{height:22px;width:22px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [459943,460008)?UTF-16 [459943,460008)?

```css
.rapid-trigger-slider-tip .tag-container{display:flex;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460008,460172)?UTF-16 [460008,460172)?

```css
.rapid-trigger-slider-tip .tag-container .upload-tag-icon{content:url(../../static/media/icon_upload_in_black.4230f4d3.svg);height:15px;margin-right:5px;width:15px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460172,460267)?UTF-16 [460172,460267)?

```css
.rapid-trigger-slider-tip .tag-container .upload-tag-icon.down-stroke{transform:rotate(180deg)}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460267,460318)?UTF-16 [460267,460318)?

```css
.rapid-trigger-slider-container.no-tip{height:36px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460318,460396)?UTF-16 [460318,460396)?

```css
.rapid-trigger-slider-container.no-tip .rapid-trigger-slider-tip{display:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460396,460554)?UTF-16 [460396,460554)?

```css
.rapid-trigger-slider-container .left{background:#fd8611;border-radius:3px;bottom:25px;height:6px;left:0;max-width:100%;position:absolute;width:50%;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460554,460704)?UTF-16 [460554,460704)?

```css
.rapid-trigger-slider-container .right{background:#44d62c;border-radius:3px;bottom:25px;height:6px;left:50%;max-width:30%;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460704,460856)?UTF-16 [460704,460856)?

```css
.rapid-trigger-slider-container .track{background:rgba(253,134,17,.302);border-radius:3px;bottom:25px;height:6px;position:absolute;width:100%;z-index:1}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [460856,461025)?UTF-16 [460856,461025)?

```css
.rapid-trigger-slider-container .foot{bottom:0;opacity:1;position:absolute;transition:visibility 0s,opacity .3s linear;visibility:visible;will-change:visibility,opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [461025,461083)?UTF-16 [461025,461083)?

```css
.rapid-trigger-slider-container.no-tag .foot{display:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [461083,461153)?UTF-16 [461083,461153)?

```css
.rapid-trigger-slider-container.on .foot{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [461421,461489)?UTF-16 [461421,461489)?

```css
.rapid-trigger-slider-container.indent{margin-left:30px;width:490px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [461489,461673)?UTF-16 [461489,461673)?

```css
.rapid-trigger-hide-slider .rapid-trigger-slider-container{height:0;margin-left:30px;opacity:0;overflow:hidden;transition:height .3s,opacity .3s;width:490px;will-change:height,opacity}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [461673,461810)?UTF-16 [461673,461810)?

```css
.rapid-trigger-hide-slider .rapid-trigger-slider-container.on{height:64px;margin-bottom:20px;opacity:1;overflow:visible;overflow:initial}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [461810,461897)?UTF-16 [461810,461897)?

```css
.rapid-trigger-has-slider .rapid-trigger-slider-container{margin-left:30px;width:490px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [461995,462084)?UTF-16 [461995,462084)?

```css
.rapid-trigger-continuous{display:flex;gap:7px;padding-top:20px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462252,462325)?UTF-16 [462252,462325)?

```css
.rapid-trigger-animation-slider-tip{display:flex;margin:auto;width:200px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462325,462433)?UTF-16 [462325,462433)?

```css
.rapid-trigger-tip{align-items:center;display:flex;flex-direction:column;justify-content:center;padding:5px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462433,462486)?UTF-16 [462433,462486)?

```css
.rapid-trigger-tip-slider-text{display:flex;gap:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462486,462540)?UTF-16 [462486,462540)?

```css
.rapid-trigger-tip-slider-text span{text-align:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462540,462626)?UTF-16 [462540,462626)?

```css
.rapid-trigger-tip-bottom{display:flex;gap:8px;justify-content:center;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462626,462699)?UTF-16 [462626,462699)?

```css
.rapid-trigger-tip-bottom-status{align-items:center;display:flex;gap:8px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462699,462777)?UTF-16 [462699,462777)?

```css
.rapid-trigger-tip-bottom-status div{border-radius:50%;height:14px;width:14px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462777,462846)?UTF-16 [462777,462846)?

```css
.rapid-trigger-tip-bottom-status--green div{background-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462846,462905)?UTF-16 [462846,462905)?

```css
.rapid-trigger-tip-bottom-status--green span{color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462905,462972)?UTF-16 [462905,462972)?

```css
.rapid-trigger-tip-bottom-status--red div{background-color:#fd4949}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [462972,463029)?UTF-16 [462972,463029)?

```css
.rapid-trigger-tip-bottom-status--red span{color:#fd4949}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [463076,463349)?UTF-16 [463076,463349)?

```css
.rapid-trigger-continuous .continuous-help{background-color:#4a4a4a;background-image:url(../../static/media/tooltip_questionmark.96138d2f.svg);background-repeat:no-repeat;border-radius:7px;height:14px;transition:background-color .3s;width:14px;will-change:background-color}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [463349,463759)?UTF-16 [463349,463759)?

```css
.rapid-trigger-continuous .continuous-tip{background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;line-height:16px;max-width:-webkit-max-content;max-width:max-content;opacity:0;padding:8px 10px;position:absolute;text-align:left;text-transform:none;top:135px;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:pre-wrap;width:65%;will-change:visibility,opacity;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [463759,463835)?UTF-16 [463759,463835)?

```css
.rapid-trigger-continuous .continuous-help:hover{background-color:#ffffff4d}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [463835,463941)?UTF-16 [463835,463941)?

```css
.rapid-trigger-continuous .continuous-help:hover .continuous-tip{opacity:1;visibility:visible;z-index:100}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [463941,464047)?UTF-16 [463941,464047)?

```css
.rapid-trigger-continuous .help:hover .continuous-tip.mt-tip{background-color:#0000;border:none;padding:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [464047,464117)?UTF-16 [464047,464117)?

```css
.actuation-warning-container{display:flex;gap:5px;padding-bottom:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [464117,464332)?UTF-16 [464117,464332)?

```css
.actuation-warning-container .info-icon{background-image:url(../../static/media/info-icon.769264c1.svg);background-position:50%;background-repeat:no-repeat;background-size:cover;flex-shrink:0;height:20px;width:20px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [464410,464461)?UTF-16 [464410,464461)?

```css
.snap-tap .disabled{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [464751,464814)?UTF-16 [464751,464814)?

```css
.snap-tap-add-button .disabled{opacity:30%;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [465669,465756)?UTF-16 [465669,465756)?

```css
.snap-tap-key-list-wrapper{align-items:center;display:flex;gap:10px;margin-bottom:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [465756,465816)?UTF-16 [465756,465816)?

```css
.create-snaptap{display:flex;flex-direction:column;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [465816,465883)?UTF-16 [465816,465883)?

```css
.create-snaptap-header{width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [465883,466023)?UTF-16 [465883,466023)?

```css
.create-snaptap-header-title{background-color:#44d62c;border-radius:3px;color:#000;font-size:12px;padding:6px 16px;text-transform:uppercase}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466023,466105)?UTF-16 [466023,466105)?

```css
.create-snaptap-header-title.disable{background-color:#30961f;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466105,466160)?UTF-16 [466105,466160)?

```css
.create-snaptap.disable{opacity:.3;pointer-events:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466160,466229)?UTF-16 [466160,466229)?

```css
.create-snaptap-message{display:block;font-size:14px;margin-top:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466229,466276)?UTF-16 [466229,466276)?

```css
.create-snaptap-message--warning{color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466276,466320)?UTF-16 [466276,466320)?

```css
.create-snaptap-message--success{color:lime}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466320,466369)?UTF-16 [466320,466369)?

```css
.create-snaptap-message--introduction{color:#999}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466369,466450)?UTF-16 [466369,466450)?

```css
.widget .titleRow .shortcuts .shortcutButton-snaptap{padding:5px 10px;width:auto}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466450,466503)?UTF-16 [466450,466503)?

```css
.st-wrapper{align-items:center;display:flex;gap:10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466503,466533)?UTF-16 [466503,466533)?

```css
.st-tooltip{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466533,466780)?UTF-16 [466533,466780)?

```css
.st-tooltip-help{background-color:#4a4a4a;background-image:url(../../static/media/tooltip_questionmark.96138d2f.svg);background-repeat:no-repeat;border-radius:7px;height:14px;transition:background-color .3s;width:14px;will-change:background-color}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466780,466859)?UTF-16 [466780,466859)?

```css
.st-tooltip:hover .st-tooltip-content{opacity:1;visibility:visible;z-index:100}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [466859,467253)?UTF-16 [466859,467253)?

```css
.st-tooltip-content{background-color:#000;border:1px solid #5d5d5d;color:#ccc;font-size:14px;left:5px;line-height:16px;max-width:340px;opacity:0;padding:8px 10px;position:absolute;text-align:left;text-transform:none;top:20px;transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:pre-wrap;width:-webkit-max-content;width:max-content;will-change:visibility,opacity;z-index:99}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467253,467318)?UTF-16 [467253,467318)?

```css
.key-record{gap:10px;width:-webkit-fit-content;width:fit-content}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467318,467379)?UTF-16 [467318,467379)?

```css
.key-record,.key-record-item{align-items:center;display:flex}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467379,467505)?UTF-16 [467379,467505)?

```css
.key-record-item{border:2px solid #ccc;border-radius:4px;box-sizing:initial;height:40px;justify-content:center;min-width:60px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467505,467551)?UTF-16 [467505,467551)?

```css
.key-record-item-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467551,467604)?UTF-16 [467551,467604)?

```css
.key-record-item-editing-active{border-color:#44d62c}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467604,467658)?UTF-16 [467604,467658)?

```css
.key-record-item-editing-warning{border-color:#fd8611}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467658,467836)?UTF-16 [467658,467836)?

```css
.key-record-item-editing div{text-wrap:nowrap;align-items:center;display:flex;font-size:11px;height:25px;justify-content:center;margin:auto 10px;min-width:38px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467836,467930)?UTF-16 [467836,467930)?

```css
.key-record-item-assignment{align-items:center;color:#fff;display:flex;justify-content:center}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [467930,468022)?UTF-16 [467930,468022)?

```css
.key-record-item-assignment-active{background-color:#44d62c;border-color:#44d62c;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [468022,468110)?UTF-16 [468022,468110)?

```css
.key-record-item-assignment-deactive{background-color:#888;border-color:#888;color:#000}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [468110,468192)?UTF-16 [468110,468192)?

```css
.key-record-item-assignment div{font-size:11px;margin:auto 10px;padding:auto 10px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [468192,468250)?UTF-16 [468192,468250)?

```css
.blink-active{animation:blinker-active 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [468250,468310)?UTF-16 [468250,468310)?

```css
.blink-warning{animation:blinker-warning 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [468597,468669)?UTF-16 [468597,468669)?

```css
.blink-active-border{animation:blinker-active-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [468669,468743)?UTF-16 [468669,468743)?

```css
.blink-warning-border{animation:blinker-warning-border 1s linear infinite}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [468986,469023)?UTF-16 [468986,469023)?

```css
.snaptap-shortcuts{position:relative}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [469023,469186)?UTF-16 [469023,469186)?

```css
.snaptap-shortcuts .showTooltip{background-color:#111;border:1px solid #5d5d5d;color:#ccc;padding:8px 10px;position:fixed;visibility:visible;width:300px;z-index:3}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [469186,469291)?UTF-16 [469186,469291)?

```css
.snaptap-shortcuts .showTooltip p{font-family:Roboto,sans-serif;font-size:14px;line-height:17px;margin:0}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [469291,469356)?UTF-16 [469291,469356)?

```css
.snaptap-shortcuts .hideTooltip{position:fixed;visibility:hidden}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [469356,469419)?UTF-16 [469356,469419)?

```css
.snaptap-shortcuts-overlay{inset:0;position:absolute;z-index:2}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [469892,470053)?UTF-16 [469892,470053)?

```css
.key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:initial;display:flex;height:30px;justify-content:center;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [470053,470166)?UTF-16 [470053,470166)?

```css
.key-record-item-v3-text{border-radius:3px;margin:0 2px!important;min-width:0!important;padding:0 10px!important}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [470221,470288)?UTF-16 [470221,470288)?

```css
.snaptap-shortcuts-overlay:hover+.tip{opacity:1;visibility:visible}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [470629,470744)?UTF-16 [470629,470744)?

```css
.key-record-item-v3-skst-text{border-radius:3px;min-width:0!important;padding:0 10px!important;text-transform:none}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [471074,471253)?UTF-16 [471074,471253)?

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [471253,471309)?UTF-16 [471253,471309)?

```css
.skst-key-record-item-v3:hover{border:1px solid #9b9b9b}
```

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`?Unicode [471309,471480)?UTF-16 [471309,471480)?

```css
.skst-key-record-item-v3-warning{align-items:center;border:1px solid #fd8611;border-radius:4px;color:#fd8611;display:flex;height:30px;justify-content:right;min-width:30px}
```
