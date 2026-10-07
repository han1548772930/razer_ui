# Current native HID transport and source-selected receiver queries

The runtime transport uses the original `node-rz-hid` 0.0.31 native module from
the current official host package, through its plain C HID exports. Win32
SetupAPI supplies current interface identity and capabilities; it is not a
replacement implementation of the Razer feature-report transport.

## Original module and static evidence

- Official host: 4.0.827, acquired through the production package documented in
  [the current host audit](current-host-version-audit.md).
- Inner archive SHA-256:
  `9d4765d46c5c5e1ff9c11d16452bd12a9eb43f14cd70be893fa843df3882cd90`.
- ASAR SHA-256:
  `b2ce8c54dc5c991feef3a24e1880ced57ba88a0f40a687a5b082187ac0a1cca6`.
- Original unpacked entry:
  `win-unpacked/resources/app.asar.unpacked/node_modules/node-rz-hid/build/Release/HID.node`.
- Signed AMD64 file: 405,704 bytes, SHA-256
  `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`.
- Packaged asset: `assets/native/razer-hid-0.0.31.node`, byte-identical to that entry.

`tools/extract-current-hid-native.py` statically reads ASAR metadata and archive
entries. It verifies that the difference from the unsigned ASAR size is exactly
the PE certificate tail. `tools/audit-receiver-native-hid.py` inspects the PE and
uses static `dumpbin /disasm` output to record the actual C ABI. No vendor module,
installer, DLL, application, build or test is executed during this validation.

The [native evidence](receiver-native-hid-current-evidence.json) includes 23 PE
exports and five disassembly records. The relevant ABI is:

| Export | Original implementation | Rust boundary |
| --- | --- | --- |
| `hid_open_path` | RVA `0x17250`, RCX NUL-terminated path, RAX opaque handle/null | `extern "C" fn(*const c_char) -> *mut c_void` |
| `hid_send_feature_report` | RVA `0x17a10`, RCX handle, RDX bytes, R8 length; resolves `HidD_SetFeature` | `extern "C" fn(*mut c_void, *const u8, usize) -> c_int` |
| `hid_get_feature_report` | RVA `0x17ac0`, same arguments with writable buffer; `DeviceIoControl(0xB0192)` and completion count | `extern "C" fn(*mut c_void, *mut u8, usize) -> c_int` |
| `hid_close` | RVA `0x17de0`, opaque handle cleanup | `extern "C" fn(*mut c_void)` |

The original get-feature implementation returns the actual byte count plus the
ReportID byte. The worker validates that result instead of assuming that an
allocated buffer was filled. Native structure fields are never read by Rust.
`node.exe` is a delay import; these plain C paths do not call N-API registration.
Loading the file still executes its DLL entry point, so native loading remains
inside the isolated worker and has not been exercised during development.

## Responsibilities

`runtime_hid_transport.rs` owns only native materialization, exact-byte checking,
library lifetime, opaque handle lifetime, and feature-report calls. It has no
product IDs or receiver protocol commands. The prepared module is loaded from
`%LOCALAPPDATA%/RazerUi/native/<SHA-256>/HID.node` with DLL-load-directory and
system-directory search flags; an existing different file fails explicitly.

`receiver_protocol.rs` owns the current shared rzDevice25 V2 envelope, outgoing
checksum and response parsing. `runtime_receiver.rs` selects a source capability
from the real current interface, revalidates identity and performs bounded
queries. The UI/worker request carries the path and ContainerId rather than a
claimed product type. `receiver_capabilities.rs` reads the generated source
capability asset on every platform.

Capabilities require a traced product DeviceInfo, actual main feature setup,
factory branch, class inheritance, transport parameters and query method. A
product's presence in a compatibility JSON or a shared class's presence in its
bundle does not establish support. The ongoing catalog-wide source audit must
cover all current products and explicitly retain unresolved or inapplicable
bindings; the initial two traced bindings are not a claim of complete coverage.

`tools/audit-receiver-capabilities.cjs` currently records the independently
traced starting bindings and exact host routing in
[the capability evidence](receiver-capabilities-current-evidence.json). It also
checks send-command transaction exceptions: commands `0x46` and `0x41` alter
transaction bits, whereas V2 `0xBF` uses the selected class's normal namespace.
The real factory multiplies each base 5 ms interval by five; the runtime uses
the resulting 25 ms, with the source OUT=20 and IN=10 limits.

## Observation boundaries

The worker requires a complete current enumeration, a nonzero ContainerId, a
single current path and instance, and exact VID/PID/interface/feature-length
matches to the source binding. It validates those facts before opening, after
opening, and after a successful protocol response. Missing `MI_XX` identity
remains unknown. The middleware's additional interface `-1` fallback is not
silently represented as interface zero.

The local named mutex coordinates this application's workers. It does not
claim to acquire the proprietary host's private mutex. Transaction and command
checks reject interleaved replies. Transaction counters belong to the concrete
path, container and protocol namespace, and normalize against that capability's
range. A ten-second application observation budget discards late results; it is
not a vendor protocol timeout. The current complete retry schedule has at most
5.5 seconds of intentional sleeps. The separate 15-second parent worker timeout
bounds a stuck native call.

Only the source-verified `[80, 0, 191]` query is emitted by this protocol. No mode
initialization, pairing, unpairing, mapping, device persistence or source
publishing chain is invoked. A valid count of zero is an empty observation;
timeouts, wrong lengths, unsupported commands and truncated records are errors.
Firmware product IDs remain raw, and unknown status bytes remain unknown.
Product mapping and Dashboard readiness require their separate source rules.
The ability to issue a real query is not evidence that attached hardware was
successfully read during development; runtime acceptance remains outstanding.
