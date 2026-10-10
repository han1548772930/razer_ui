# Current 1342 Audio Mixer page and submission chains

Current implementation evidence is the hashed 1342 frontend JS/CSS and
middleware in [the source receipt](audio-mixer-page-bindings-source-current.json),
plus [CmMixerLib native evidence](audio-mixer-dll-protocol-current.json) and
[the new mapping-engine IDA/Hex-Rays receipt](evidence/mapping-engine-global-shortcuts-ida.json).
No historical frontend/host source is used. Source acquisition, semantic
recovery, Rust implementation, UI/backend connection and runtime acceptance
remain separate; this page/product is not declared fully restored.

The typed page intents reach the retained Shell/service HID collection. Each
setter follows its original reports. `MixerWriteResult.transport_completed`
means the source setter transport completed; it is not device state readback.
The implementation performs no additional before-value getter, after-value
getter, readback comparison or initial all-DSP refresh. Caller-required reads
and setter-required register reads remain. Only explicit original GET replies
enter the separate observed-state map; local drafts and preset storage remain
local. Owner/revision cancellation and collection identity checks do not invent
hardware GET commands or rollback.

Voice `Ti` reads Magic Voice enabled state and changes it only when different.
The changed switch setter reads its register once, copies low24 into the fixed
C0 high template and toggles bit1. Enabled mode submission reads EQ/Magic/Echo
gates in that order, then the original `pe` register query, then the selected
fixed mode template. Modes MONSTER=0/CARTOON=1/LOW=2/HIGH=3 retain the original
codes. Disable retains local mode and sends no mode. It does not query the
result after the setter.

Echo `Ci` reads enabled state and changes it only when different. The switch
setter reads its register once, copies low24 into fixed80 and toggles mask12.
When enabled, room/decay/gain/delay follow in order; each original scalar setter
reads EQ/Magic/Echo gates before sending its fixed selector/template and value.
They do not read that scalar's previous or resulting value. Room uses
floor(room*24/100-43); Gain=0 is skipped by the original truthiness guard.
Library defaults to [40,1.7,0.5,150]; named arrays and retained Custom values are
source-derived. Changes reset the original 500ms debounce. A setter transport
failure stops later fields; local zero is never represented as device zero.

Key Shifter submits zero when disabled and its retained level when enabled.
Its fixed template reads the three gates and retains original negative sign
encoding. Vocal Fading follows its enabled-state caller GET, changed switch
register RMW, then retained level when enabled. Its scalar setter reads the
three gates and truncates the level into the original fixed template. Neither
path performs added getter verification after SET.

`LM.changeTab`, `LM.onDrag` and `LM.reset` submit the full ten-band array. Reset
selects Custom plus Default zeros. The source `setMicEQ(true)` sends fixed
Output13 command0x5FFC0034 payload0x40000091 directly; disabled uses0x40000090.
It has no register GET. Each band sends its original fixed template directly
without any prior or subsequent band getter. Source setter data frequencies
30/60/120 differ from display31/63/125; band register addresses advance by4.
One retained collection executes enable then bands0..9; failure reports the
submitted prefix and stops later fields. All eleven completions are transport
completions. Basic bands expand [0,0,0,0,1,1,1,1,2,2], with independent Basic and
Advanced Custom arrays. Current mode caller's `micBaicEqualizer` typo remains
an explicit source error branch. Kitty's `deviceEqDifferent` import/confirmation
is gated to AudioAWKittyBLE and is not added to AudioMixer.

Noise Gate/Compressor maintain original Basic/Advanced visible control gates;
Show More/Less retains local mode without an invented mode hardware command.
Their source page codecs and remaining UI details require further individual
closure. The five virtual AudioCamy reducer namespaces are separate from
CmMixerLib DSP endpoint properties and must not be equated without evidence.

Effects presets use the current initializer's random GUID and `Preset` name,
separate from provisional frontend `Default`. Add/Duplicate/Rename/Reset/Delete
and selection persist the device-scoped local `_audioMixerPresets` library.
Rename trims and checks32 UTF-16 units; original name duplicate checks remain
case-sensitive and ordering case-insensitive. Reset retains guid/name and
removes mapping; Delete requires another preset, delays confirmation100ms and
selects the first sorted survivor. Selection restores Voice/Echo/Key/Vocal and
submits those original chains. Reset has one confirmation; Delete has Cancel.
Current GPUI global Dialog/generic menus still differ from the original anchored
profile-act/profile-del/popupReset layouts and are not marked complete.

The Effects mount now uses Microphone Voice/Echo on the left, Line In Key/Vocal
on the right, original20px bottom headings, title switches, Voice27px tabs and
Key/Vocal source slider styling. The top uses276px left group,250px selector
plus26px action control and230x27px capture. Echo CU rotary canvas/drag geometry,
room numeric editor, exact anchored popups and final visual details remain gaps.

`GU/yU` keyboard capture preserves sided Ctrl/Shift, default Ctrl+Alt with no
held modifiers, ignored standalone Alt and Delete/Backspace removal. Source
117-key identities are used; duplicate mapping display strings remove the
current mapping and retain attempted warning text. Mouse/Hypershift capture,
the source delayed listeners, general-mapping suppression, warning icon/tooltip
and all native mapping/macro branches remain incomplete.

Actual shortcut registration is direct Rust platform code, without loading
mapping_engine.dll. Registration/removal returns its original OS completion;
there is no extra registered-list query/confirmation gate. Source modifiers
normalize into Windows MOD flags; WM_HOTKEY reads physical sides and selects
the first subset-matching binding. Events reach the current connected workspace
and selected preset's four DSP chains. Capture disables publication and clears
queued events; focus-out enables; revision checks reject stale callbacks.
Disconnect/quit retires workers and releases actual OS registrations. Non-Windows
registration remains explicitly unsupported, while shared source identities
are OS-independent.

The native receipt records21 functions, original SHA-256, RVA boundaries,
pseudocode, instructions and xrefs. `0x141F0/0x143E0 -> 0xC97C0` registers;
`0x14810/0x149E0 -> 0xC9AA0` unregisters. `0xC94C0` consumes WM_HOTKEY and uses
GetKeyState; `0x189A0` serializes the first matching original binding;
`0x11C460` confirms physical modifier bits. IDs0..0xBFFF reuse free IDs and
MOD_NOREPEAT is absent. Thread-bound hWnd=NULL replaces native hidden HWND,
enable/disable is idempotent rather than native disable-token errors, and the
queue is bounded512; no original timeTick/event type is fabricated. These host
implementation differences do not claim complete native semantic equivalence.

Static verification uses `cargo check --locked --all-targets`, formatting,
source audit `python tools/audit-audio-mixer-page-bindings-current.py --check`
and diff checks. Existing static test definitions reflect source report order;
no tests, application, DLL, device command or vendor JS were executed. Runtime
acceptance remains unperformed. See [the structured chain receipt](audio-mixer-page-bindings-current.json).
