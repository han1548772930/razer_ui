// Document current, byte-verified AST receipts. Never evaluate vendor scripts.
const fs=require('fs'),crypto=require('crypto');
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const notes=`# Current Wolverine V4 calibration: middleware, device and transport

This audit parses both independently fetched official middleware bundles and their exact webpack-declared device chunks. It does not execute vendor JavaScript, native libraries, an application, or real device operations. Reverse-engineered semantics, Rust implementation, UI integration and runtime acceptance are separate statuses. The device/service adapter was unimplemented when this receipt was produced; a task acknowledgement is not evidence of a successful device save.

## Dispatch and acknowledgement

Module 1418's ON_SET_CONTROLLER_CALIBRATION custom handler reads TaskMaker's current event: timerTick, from, payload.action and payload.partId. It invokes taskMakerControllerCalibration(timerTick,from,action,partId). Module 22167 selects the singleton handler from ControllerCalibrationParams.Version; both current products specify Version 3, selecting module 36840's V3 class, which extends V2 (28927), which extends V1 (22130).

The task maker calls start(partId) for action start WITHOUT awaiting the returned promise. For rotate-complete it awaits rotateComplete only if the handler is instanceof V3. All other action strings call stop. The try/catch catches synchronous invocation failures, not an unawaited start promise rejection. It then broadcasts MW_SET_CONTROLLER_CALIBRATION_TO_UI with {timertick,actionType:"taskMakerControllerCalibration",status:"completed",value:{action}}. This ack does not claim that raw reads, calibration sampling, a write or readback succeeded.

## Actual parameters and model differences

Both products: Range.min=0, Range.max=8192, IdealCenterPoint={x:4096,y:4096}, DeadzoneRatio=3, AllowedDeviationPercent=9, BoundaryTolerance=12, ValidityThreshold=.35, CenterPointThreshold=.1. TriggerMax=2048, ThresholdPercent={min:5,max:2}, ValidationRange=300, PressValidityPercentUI=18, ReleaseValidityPercentUI=11. The constructor's fallback values 5/2 for UI validity are overridden by the actual per-product configuration 18/11.

| Product | Final left PressThreshold / ReleaseThreshold | Final right PressThreshold / ReleaseThreshold |
| --- | --- | --- |
| 2676 Wolverine V4 Pro | 500 / 1490 | 600 / 1600 |
| 2684 Wolverine V4 Tournament Edition | 400 / 1400 | 500 / 1500 |

These are strict final-range acceptance thresholds, distinct from the UI step validity thresholds. The V3 method bodies other than the constructor are byte-identical between these independently parsed products. Constructor temporary variable ordering differs; product configuration values also differ, so the configuration must remain keyed by product.

## State, timers and exact step ordering

Parts: None=0, LeftJoystick=1, RightJoystick=2, LeftTrigger=3, RightTrigger=4. MW steps: None=0, joystick center steps1..4, Rotate=5, Complete=6, TriggerPress=10, TriggerRelease=11, TriggerComplete=12. UI error=-1. The trigger selector passed to device commands is NOT the UI part ID: part3 maps to analog mask4, part4 maps to mask8.

Inherited state includes state, step, partId, CenterPoints, BoundaryPoints, UserMovementPoints, isValidUserMovement, calibrationParams, boundaryTimerId and userMovementTimerId. V2 adds isStepValid, THROTTLE_TIME=33, lastMovementUpdateTime, movementUpdateTimer, actualCenterPoint. V3 adds hasTravelled=false, hasReturned=false, restingTriggerValue=8192, TriggerMovement={min:8192,max:8192,value:8192}, triggerSample={centerPoint:{x:0,y:0},boundary:{xMin:8192,xMax:8192,yMin:0,yMax:0}}, lastTriggerUpdateTime=0, triggerUpdateTimer=null, triggerHoldTimer=null, previousTriggerValue=undefined, stablePollCount=0 and triggerParams.

1. start(part3/4) calls reset; sets state=inProgress and partId; awaits getAnalogInputRawDataAll().jsonData; selects Lt or Rt. A finite inclusive [0,8192] reading becomes restingTriggerValue; absent/invalid reading falls back to8192. resetTriggerMovement sets min=max=value=resting and resets previousTriggerValue/count. It then awaits prepareNextStep.
2. prepareNextStep first exits if idle. For triggers it clears hold timer, records old step and advances 0→10→11→12. Step12 immediately calls completeTriggerCalibration. Steps10/11 set isStepValid=false, hasTravelled=false, hasReturned=false; resetTriggerMovement; await another raw read and, when valid, set min=max=value to that reading. It broadcasts UI progress and starts a one-shot polling timeout for10ms. Initial invalid reads are not silently converted to successful observations.
3. Polling getUserMovementPoints is restricted to steps10/11. It records current step, awaits a raw read, and exits if the step changed while awaiting. Only valid finite inclusive-range readings update stability, min/max/value. It then validates the existing movement, awaits the33ms window-storage throttle handler, and schedules another10ms one-shot poll only if the recorded step still matches. There is no part/generation comparison in this original method.
4. travelHeadroom=restingTriggerValue−Range.min. If a previous reading exists and abs(new−previous)≤.01*travelHeadroom, stablePollCount increments; otherwise it resets to0. previous becomes new. Stable means count≥5 (five consecutive stable differences, normally six readings).
5. pressValidityThreshold=(18/100)*2048=368.64. releaseValidityThreshold=resting−min(.01*travelHeadroom,(11/100)*2048), where225.28 is the second limit. Press sets hasTravelled |= value≤pressThreshold and valid=hasTravelled && value≤pressThreshold && stable. Release sets hasReturned |= value≥releaseThreshold and valid=hasReturned && value≥releaseThreshold && stable. A validity change to true clears any old hold timer and starts a2,000ms press /3,000ms release timeout; a change to false clears it. Each validity change also broadcasts UI progress. No UI animation advances SDK state.
6. Hold timeout first sets triggerHoldTimer=null then calls onTriggerHoldComplete. This exits if idle, unsupported step, or invalid. Otherwise it stops polling and pending trigger movement-storage timeout; saves the current step; gets five raw readings, each finite inclusive [0,8192], with10ms between reads1..4. Any invalid/absent sample fails the whole sample. A valid sample is Math.round(sum/5). Captured step10 stores triggerSample.boundary.xMin; step11 stores xMax; it awaits prepareNextStep. The original sampling function does not recheck state/step/generation after each await; this limitation must not be misrepresented as an original cancellation guard.
7. Step12 stops user polling, trigger storage timer and hold timer. Final xMin=abs(round(sampleMin−.05*2048)); xMax=abs(round(sampleMax+.02*2048)); center={0,0}, yMin=yMax=0. Thus margin values before rounding are102.4 and40.96.
8. writeAndVerify first validates both finite bounds, strict min>0/max<8192, strict min<the product/side PressThreshold, max>the product/side ReleaseThreshold, and max−min>300. Invalid yields "Trigger calculated value not within range". It awaits the save helper; helper false yields "Command Failed". It then awaits readback; absent yields "Trigger calibration read back failed"; unequal xMin/xMax yields "Trigger calibration read back mismatch"; readback then must independently pass the same validation. Only successful readback yields no error.
9. Success awaits removal of CalibrationUserMovement window storage, broadcasts UI step12 using inherited updateUI, then calls stop/reset. Failure clears hold timer, awaits removal of that key, sets valid=false, broadcasts UI error−1 with its exact error string, then stop/reset. Inherited UI progress shape is {step,stepDesc,isStepValid,error}, with stepDesc taken from the current UI step enum and error empty unless step−1.

## Window storage and cancellation cleanup

The UI movement key is exactly CalibrationUserMovement. Trigger data is {x:0,y:0,t:100−n}, n=clamp(round((releaseThreshold−value)/(releaseThreshold−pressThreshold)*100),0,100). Trigger writes are at most every33ms; otherwise a pending timeout is replaced with a new33ms timeout (the original uses a full33ms delay, not remaining time). Pending timeout callback updates lastTriggerUpdateTime and writes current movement; it does not null its own handle. The next throttle invocation or stop clears/nulls the stored handle. Window storage is a host/UI observation channel, separate from physical calibration persistence.

stop calls the V3 virtual reset. V1 reset sets idle, step0, part0; clears center points; resets boundary points to4096; clears boundary timeout; resets UserMovementPoints bounds to4096 and x=y=0; clears user movement timeout; sets isValidUserMovement=true. V2 reset then sets valid=false, lastMovementUpdateTime=0 and movementUpdateTimer=null; calls removeMovementWindowStorage without awaiting; sets isValidUserMovement=false. V3 reset then sets travelled/returned false, resting8192, default sample8192/8192; resets TriggerMovement and stability; clears trigger storage and hold timers; resets lastTriggerUpdateTime=0. The V2 reset nulls the movement timer without clearTimeout. Neither reset nor stop aborts a raw-read promise already in flight; neither increments a generation. Rust cleanup protection must be documented as an implementation choice rather than attributed to the vendor source.

## Device methods, packet fields and host bridge

Entry useFeature calls prove both use rzDevice25SagePC with reportId10 and sleepTimeBetweenOut/OutIn/In=30ms. Module87887 resolves this to webpack chunks2620/2661 and module78786. SagePC extends controller module98773, which extends base rzDevice25 module87969. SagePC sets reportLength91 and uses the configured reportId rather than its constructor default4. DeviceInfo.claimInterface=1, vendorId5426. PID2676 supports wired2676/2677 and wireless2680/2681; PID2684 supports wired2684/2685. Actual selected transport productId/container remain the observed device's values.

| Call | Header [payloadSize,class,id] | Payload / response |
| --- | --- | --- |
| getAnalogInputRawDataAll | [80,12,152] | The actual source passes the same three-byte header array as payload; the90-byte frame therefore begins payload [80,12,152,0…]. Success response only checks packetSizeReturned>0 before reading six BEu16 pairs: Lx0..1,Ly2..3,Rx4..5,Ry6..7,Lt8..9,Rt10..11. |
| getAnalogInputCalibrationDataAll(mask) | [51,12,153] | Zero-filled51-byte payload, byte0 mask. Response parser requires statusSUCCESS and packetSizeReturned===33. |
| setAnalogInputCalibrationDataAll(mask,fields) | [51,12,25] | Same51-byte payload; byte0 mask; selected joystick/trigger fields below, all others zero. |

Calibration reply/payload fields: analogId0; LxMid1..2,LxMin3..4,LxMax5..6,LyMid7..8,LyMin9..10,LyMax11..12,RxMid13..14,RxMin15..16,RxMax17..18,RyMid19..20,RyMin21..22,RyMax23..24,LtMin25..26,LtMax27..28,RtMin29..30,RtMax31..32. Each pair is big endian, proven by76912.Jz=(a&255)<<8|(b&255), E4=(n>>8)&255, l7=n&255. Get requests still reserve51bytes. Set supports exactly one enum selector per call (1,2,4,8), not arbitrary combined masks. Falsey supplied fields become zero in original packing. save helper builds all16 keys with undefined for other parts; returns false only when the device method throws and does not separately check returned status. Final V3 readback is the actual success guard.

Base _createDataSend constructs90 zero bytes: status0, transactionId at byte1 (constructor starts0; the getter resets counter31 to0, returns the old counter via post-increment, therefore emits0..30 repeatedly), bytes2..4 zero, payloadSize5,class6,id7,payload8 onward, checksum88,reserved89. Checksum is XOR bytes2..87 inclusive. HID reportId is separate from these90bytes; reportLength is91. sendCommand's Electron branch sends doRzDeviceAction with action hid.sendFeatureReportMutex when host mutex is supported, otherwise hid.sendFeatureReport. Payload includes productId,vendorId,deviceContainerId,claimInterface,dataSend,reportId,reportLength,protocol:"25" and the three sleep times. It requires the returned write count equal91, waits30ms then calls hid.getFeatureReport with the same identity/report parameters. It checks returned transaction/class/id against the transmitted header; mismatch requests OUT retransmit. Status2 succeeds;0(new),1(busy),3(failed),4(timeout) fail with retry semantics;5(unsupported) and unknown fail without retransmit. Finally it releases the host HID mutex if enabled. The source does not validate a returned checksum here. Browser fallback uses WebUSB class/interface requests9 OUT and1 IN, value768, index claimInterface,90-byte transfers. These are wrapper semantics proven by current source, not a claim that the native host bridge has been reimplemented.

## Selection product background cascade

2676 Km and2684 vc create widget-prod dot-bg earbuds-svg plus dim-corner; however both current calibration-specific CSS override widget-prod background:none, width372,height253,margin0,min-width0,max-width:none and explicitly dim-corner display:none. Rendering no dot pattern or radial dim layer in this selection widget is correct. Both wrappers return null whenever URL searchParams.get("displayMode") is nonempty; the enclosing selection controls/connectors remain. That URL mode behavior must be carried explicitly if the native product view supports it.

## Remaining full-scope gaps

The shared Rust state machine, command encoder/decoder, native HID/service adapter, real polling/observation bridge, physical device persistence, failure/cancellation integration and runtime acceptance are separate implementation items. These receipts and prepared resources establish source coverage only. No application/DLL/device verification was executed. Native host internals beyond doRzDeviceAction require current-host source and, where native binaries are involved, installed IDA Pro/Hex-Rays static receipts. Any unsupported platform adapter must return an explicit capability/error rather than fabricate reads, writes or successful saves.
`;
let md=notes;
for(const pid of [2676,2684]){
 const mw=JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-middleware-live-source.json`,'utf8'));
 const dev=JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-device-live-source.json`,'utf8'));
 if(hash(fs.readFileSync(mw.source))!==mw.sha256)throw Error('Changed middleware '+pid);
 md+=`\n## PID ${pid}: complete current source evidence\n\nMiddleware: \`${mw.source}\`, SHA-256 \`${mw.sha256}\`. Offsets below are UTF-16 code-unit positions, not native RVAs.\n`;
 for(const mid of [26091,83207,22167,69427,85654,36840]){
  const m=mw.modules.find(m=>m.module===mid);
  md+=`\n### Module ${mid}, ${m.offset}..${m.end}, SHA-256 ${m.sha256}\n\n\`\`\`javascript\n${m.source}\n\`\`\`\n`;
 }
 const custom=mw.modules.find(m=>m.module===1418).methods.find(m=>m.source.includes('taskMakerControllerCalibration'));
 md+=`\n### Complete custom calibration dispatcher, ${custom.offset}..${custom.end}\n\n\`\`\`javascript\n${custom.source}\n\`\`\`\n`;
 for(const mid of [22130,28927]){
  const m=mw.modules.find(m=>m.module===mid);
  for(const method of m.methods.filter(m=>['reset','stop','updateUI','resetBoundaryPoints','stopBoundaryPointsTimer','resetUserMovementPoints','startUserMovementTimer','stopUserMovementTimer','removeMovementWindowStorage','constructor'].includes(m.name)))md+=`\n### Inherited module${mid} ${method.name}, ${method.offset}..${method.end}\n\n\`\`\`javascript\n${method.source}\n\`\`\`\n`;
 }
 md+='\n### Current product entry feature declarations\n\n```javascript\n'+mw.entry_features.map(e=>e.source).join('\n')+'\n```\n';
 for(const mid of [98773,78786,76912,88382]){
  const m=dev.modules.find(m=>m.module===mid);
  if(hash(fs.readFileSync(m.source))!==m.source_sha256)throw Error('Changed device '+pid+'/'+mid);
  md+=`\n### Device module${mid}, ${m.offset}..${m.end}, source ${m.source}, SHA-256 ${m.source_sha256}\n\n\`\`\`javascript\n${m.module_source}\n\`\`\`\n`;
 }
 const base=dev.modules.find(m=>m.module===87969);
 for(const method of base.methods.filter(m=>['_createDataSend','_getTransactionId','_calculateChecksum','sendCommand','_getUSBTransferInResult'].includes(m.name)))md+=`\n### Complete base device ${method.name}, ${method.offset}..${method.end}, source ${base.source}, SHA-256 ${base.source_sha256}\n\n\`\`\`javascript\n${method.source}\n\`\`\`\n`;
}
const host=JSON.parse(fs.readFileSync('docs/re/gamepad-calibration-host-live-source.json','utf8'));
md+=`\n## Host4.0.827 bridge and native boundary\n\nCurrent source root is local-ui-reverse/source/installed/app-4.0.827; package.json identity was independently parsed as version4.0.827. Preload doRzDeviceAction invokes IPC rzDeviceAction; main handler delegates to UsbRzDeviceAction.handleAction. For90-byte protocol data and91-byte reportLength, host prepends payload.reportId (10 here) to make91 bytes before node-rz-hid sendFeatureReport. Get invokes getFeatureReport(reportId,reportLength) and removes its first byte unless noSlice is true. Mutex send waits the configured OUT30ms since last command timestamp, using2ms sleeps; get also updates that timestamp. Protocol mutex acquire timeout500ms, hold timeout30,000ms; general acquire timeout120,000ms; send timeout500ms; get timeout10,000ms. A tabDestroyed event releases that URL's held mutex; removeDevice closes the HID handle and cancels/removes device mutex state. releaseMutex rejects calls from a URL other than the holder.\n\nnode-rz-hid's JavaScript wrapper statically selects HID.node outside Linux; Linux selects HID_hidraw.node unless an alternate driver is requested. It forwards native prototype methods to binding.HID. This audit records the JavaScript boundary without loading a binding. New analysis of native node implementation requires IDA Pro/Hex-Rays, byte hashes, RVAs, function boundaries, pseudocode and xrefs; none of those are claimed by this JavaScript receipt.\n`;
for(const file of host.files){
 if(hash(fs.readFileSync(file.source))!==file.sha256)throw Error('Changed host '+file.source);
 const names=['preload:doRzDeviceAction','ipc:rzDeviceAction','case:hid.sendFeatureReportMutex','case:hid.getFeatureReport','case:hid.acquireMutex','case:hid.releaseMutex','constructor','buildDeviceKey','getDeviceFromDeviceMap'];
 if(file.source.endsWith('nodehid.js'))md+=`\n### Complete native-wrapper source ${file.source}, SHA-256 ${file.sha256}\n\n\`\`\`javascript\n${file.complete_source}\n\`\`\`\n`;
 for(const receipt of file.receipts.filter(r=>names.includes(r.name)&&!file.source.endsWith('preload.js')||r.name==='preload:doRzDeviceAction'))md+=`\n### Host ${receipt.name}, ${receipt.offset}..${receipt.end}, source ${file.source}, SHA-256 ${file.sha256}\n\n\`\`\`javascript\n${receipt.source}\n\`\`\`\n`;
}
fs.writeFileSync('docs/re/gamepad-calibration-middleware-current.md',md);
console.log(JSON.stringify({document:'docs/re/gamepad-calibration-middleware-current.md',bytes:Buffer.byteLength(md)}));
