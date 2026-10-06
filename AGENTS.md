# Current source requirement

The user requires the latest stable Razer Synapse UI, not the historical reference snapshot.

- Current Dashboard source: `.ref/applications/synapse/dashboard/`. Its live HTML, manifests and relevant JS/CSS were independently fetched on 2026-10-02 and matched byte for byte. See `docs/re/20-current-source-version.md`.
- Current official host source: `.ref/host-4.0.827/`, statically extracted from the package named by the current production background-manager update manifest on 2026-10-02. Package SHA-256, ASAR extraction and version evidence are recorded in `docs/re/current-host-version-audit.md`. The installed host remains 4.0.821; it was not upgraded or executed.
- Do not recreate, read or use `.ref/frontend/`, `.ref/synapse-asar/`, `.ref/host-4.0.821/` or `.work/latest-source-check/host-4.0.821/` as implementation evidence. They are obsolete. The user manually deleted all four directories on 2026-10-02; their absence and the presence of both current source directories were verified afterward.
- Use maintained static extraction tools in `tools/`. Historical `.ref/tools/` scripts may execute downloaded JavaScript or refer to obsolete sources and must not be used.
- Older audit documents are historical where marked. Renamed links do not prove that old minified symbols or behavior were re-audited.
- Do not run the application, builds, tests, installers, downloaded JavaScript, or DLLs. Permitted verification includes `cargo check --locked --all-targets`, formatting, static source parsing, resource preparation and validation.

# Work sequence confirmed on 2026-10-06

- Follow `docs/re/ui-readonly-first-roadmap.md`. Audit and finish missing UI items individually against current source; routes and descriptors alone do not count as completed UI.
- Include source-verified DLL read/query capabilities and state observation in the current scope. During development, inspect DLL files and wrappers statically only; do not load or execute them for verification.
- UI editing, add/delete/change actions, Apply/Save flows and local draft persistence remain in scope now. Only the DLL-backed mutation/write-back leg is deferred; do not omit UI operations or replace them with fake device success.
- Defer changes, device/service write-back and persistence through DLL APIs to a later coordinated integration phase. Do not mix those write operations into the current read-only interface work.
- Existing local draft persistence is separate from DLL write-back and must be identified as local. Do not fabricate successful device reads, writes or saves.
