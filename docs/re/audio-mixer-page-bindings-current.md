# 1342 Audio Mixer page bindings

The current 1342 middleware names the Noise Gate, Compressor, Vocal Fading,
Key Shifter, Voice Changer and microphone EQ controls with `RazerT2*` properties in
`AudioMixer.cde922aae2f0fea23404.js`. Those controls now produce typed
`HidNodeMixerRead/Write` intents through the shared page workspace. The HID
target is still checked by the service against the source-derived 1342
identity and report recipes in `audio-mixer-protocol.json`.

The five reducer families (`outputMixerReducer`, `playbackMixReducer`,
`streamMixReducer`, `lineOutReducer`, and `voiceChatReducer`) remain separate.
The source calls `RzNativeAudioCamy` with virtual `DeviceId_*` values for those
paths. They are not mapped to `MixerControl` endpoint properties because the
current source contains no equivalence between those namespaces. Routing them
to an HID target would fabricate a device write.

The Shell consumer accepts a unique current wired HID observation and checks
the reply's node, product, target and source property before delivering a
completion. Requests are serialized, cancellation is checked before submitting
a new setting, and epoch/profile/discovery revisions reject old responses. EQ
writes use the fixed current JS templates for all ten bands. Device observations
remain separate from locally saved drafts.

The current `ki`/`wi` callers accept `basic.value` only in `useMode=0` and
advanced threshold/target/timing fields only in `useMode=1`. A mode change
itself does not submit settings. Basic forwards the numeric threshold without
introducing a gain percentage conversion or additional setters. Key Shifter
disabling submits zero while retaining its local level; Vocal Fading enabling
submits the switch before the retained level. Microphone EQ editing enables EQ
before writing the complete ten-band array. The pure Rust planner preserves
these gates and order.

The original Noise Gate and Compressor wrappers show only the Basic slider in
mode 0 and the detailed fields in mode 1, with Show More/Less outside the
disabled settings body. The Rust mounted panels now preserve that condition
and disable settings while their source switch is off. Show More/Less changes
the locally retained mode without an invented device command; the next visible
field uses its corresponding real DSP submission.

Current `LM.changeTab` sends the named preset array, or retained Custom bands
with Default fallback. `LM.onDrag` clones the full frequency array, replaces
the changed gain and selects Custom. `LM.reset` always selects Custom with
the Default zero array; it does not restore the selected named preset.
These three UI paths now issue `HidNodeMixerEqWrite`, retaining one HID handle
and collection lock for enable and bands 0..9. Each step requires real readback;
an error stops later bands and reports the confirmed prefix without rollback.
Requests queued during an active operation coalesce to the newest full array;
revision changes/cancellation prevent stale completion publication. An already
submitted original operation is not falsely described as canceled device state.
The cancellation token is checked only before service submission. The worker
does not receive that token during its eleven synchronous operations; only
identity/deadline/error gates can stop later bands. A partial failure never
copies attempted or confirmed prefix values into the visible local profile.

`setMicEQLevel_MultiBand` requires exactly ten entries and calls the ten setters
in order. Each setter uses fixed JS Output `0x13` templates, register
`0x5ffc0070 + 4*index`, data fields 30,60,120,250,500,1000,2000,4000,8000,16000,
gain in bits 16..21 and bit0 set only for nonzero gain. No getter precedes that
source write. The native getter has an overlapping data/gain field; preserving
its returned data would therefore differ from the current source templates.
The statically generated [EQ recipe](../../assets/data/audio-mixer-mic-eq-current.json)
keeps setter data separate from current preset display labels 31/63/125.

Basic and Advanced have separate retained Custom arrays. The current three
Basic bands BASS/MID/TREBLE expand into indices 0..3/4..7/8..9 respectively,
then use the same ten-band write. Original Basic preset arrays and the Show
More/Less mode selection are now present. The current mode caller persists
mode, then tries the misspelled `micBaicEqualizer` in its Basic branch; no current
source creates that alias. That automatic branch reports failure rather than
inventing a successful write. Selecting a Basic preset, Reset or releasing a
Basic band issues its actual `ON_SET_MIC_BASIC_EQ` equivalent. Advanced mode
selection uses its retained complete array. The shared `deviceEqDifferent` confirmation/import flow is gated by
`rzDevice.name === "AudioAWKittyBLE"` at the sole current `Qs()` call;
AudioMixer does not execute that device comparison branch. It must be audited
and implemented for that actual product rather than adding a new AudioMixer
modal. Exact mounted EQ visual details still require completion; this does
not claim the entire microphone page finished.

Current CSS separates the mounted Basic and Advanced widgets. Basic uses
the custom 140px vertical track, BASS/MID/TREBLE title and separate numeric/dB
rows, and hides the ordinary bubble/frequency/y-axis/reset elements. Advanced
uses the 300px track and retains Reset. These branches are now reflected in
the page; remaining exact widget styles, spacing and assets remain explicit
visual gaps.

Remaining AudioMixer page consumers include the AudioCamy virtual reducers
and product mapping/lighting. The shared gated Kitty comparison
flow remains an independent product implementation gap. This receipt does not
claim the complete 1342 page or product has been restored.

The mounted Voice Changer switch and four mode options now follow
`ON_SET_VOICE_CHANGER -> taskMakerSetVoiceChanger -> f2B/Ti`. The source first
reads the Magic Voice switch, changes it only when needed, and when enabled
submits MONSTER=0, CARTOON=1, LOW_PITCH=2 or HIGH_PITCH=3. Disabling retains the
local mode and submits no mode. Every real write still requires device readback.
The JS he switch template copies the three low query bytes into a fixed C0
high byte before changing bit 1; `PageMagicVoiceEnabled` preserves that source
operation separately from the native switch. The ge/ye/fe/Se mode templates
differ from the native setter's reserved-bit mask: `PageMagicVoice` reconstructs the current fixed high template and
observed EQ/Magic/Echo gates, while native `MagicVoice` retains its original DLL
codec. The static auditor compares all four JS template codes against the
recovered native code table. New local objects replace unsent same-family fields;
failure stops the queued mode and never overwrites the visible local draft.

The connected Shell path supports a retained portable `HidNode` and a Windows
`ContainerId` observation. The isolated Windows adapter joins the actual
`HidDevices` path/container/VID/PID/usage/release/instance metadata to the actual
hidapi `HidNode`; it never creates path bytes or guesses an interface number.
Descriptor observations are checked with `validate_report_lengths`, shared
with the device session. Only one same-container collection supporting the
requested source recipe is accepted. Structural enumeration failures, missing
nodes and multiple candidates are errors before a device report is sent.
Windows metadata is observed again after descriptor access, before a setting
and after a response; the service retains its exact-node checks before and
after every report. This is an application identity policy: original
`CmMixerOpenHID` selects the first VID/PID collection with no discovered usage
gate. The bridge does not claim original ContainerId filtering. Its binary
evidence remains in [the DLL receipt](audio-mixer-dll-protocol-current.json).

The current profile restoration caller dispatches local `noiseGate` and
`compressor` fields and enqueues their `ON_SET_*` actions with
`MW_ACTION_FROM_LOCALSTORAGE`. The current caller does not seed these controls
from an initial DSP threshold read. The visible controls therefore retain the
local profile draft; real observed DSP values are stored separately, and failed
reads/writes use existing window notifications. No debugging receipt rows are
added to the original page layout. Full original profile application and all
remaining callers are still required; this chain does not claim those complete.

Static verification: `cargo check --locked -p razer-device -p razer-pages -p razer-service -p razer-shell --all-targets` and
`git diff --check` passed. No DLL, application, device, or runtime JavaScript
was executed. Runtime acceptance remains unperformed. See
[audio-mixer-page-bindings-current.json](audio-mixer-page-bindings-current.json).

Current JS/CSS hashes, UTF-16 ranges and original snippets are recorded in
[audio-mixer-page-bindings-source-current.json](audio-mixer-page-bindings-source-current.json).
Validate the receipt with `python tools/audit-audio-mixer-page-bindings-current.py --check`.

The source-derived planner test definitions remain available for future
authorized verification. No test execution is performed under the current
static-only development requirement.
