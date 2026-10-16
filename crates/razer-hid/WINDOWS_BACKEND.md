# Windows HID backend and MSVC compilation

The workspace pins `hidapi` 2.6.7. Windows uses its bundled C backend, Linux uses
`linux-native-basic-udev`, and macOS uses the bundled system backend. These
adapters do not establish Razer protocol evidence.

The workspace [Cargo.toml](../../Cargo.toml) sets `debug = 0` for package `hidapi`
in dev and test profiles. This omits the C compiler's `-Z7` debug records associated
with the reported MSVC 14.42 compilation failure, while preserving the ordinary
Rust/application debug profile. The exact cause of a new `cc-rs` exit code 2 must
be determined from its own C compiler diagnostic.

The upstream [`windows-native` feature](https://docs.rs/crate/hidapi/2.6.7/source/Cargo.toml)
avoids the C backend but does not provide the control Input Report operation used
by this project's [get_input implementation](src/transport/hidapi.rs).
The upstream [src/lib.rs](https://docs.rs/crate/hidapi/2.6.7/source/src/lib.rs)
gates `HidDevice::get_input_report` on `cfg(any(hidapi, target_os = "linux"))`.
Replacing this operation with an interrupt read or a silent unsupported result
would change local device behavior. A backend replacement must preserve control
Input support, collection identity, report lengths and cleanup.

Verify with the checked-in profile and without `CARGO_PROFILE_*_DEBUG` overrides:

```powershell
cargo check --locked --offline -p razer-hid --all-targets
```

The [verification record](../../.analysis/validation.json) includes a passed
workspace all-targets check. It does not record a fresh C compiler invocation or
its flags. Device communication and application runtime remain unverified.
See [AGENTS.md](../../AGENTS.md).
