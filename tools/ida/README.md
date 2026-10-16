# Static IDA analysis

Supported installed paths are `E:/IDA` (IDA 9.1) and
`D:/ida/IDA Pro 8.3` (IDA 8.3). Select the available installation with
`--ida-root`; verify the actual analyzer, Hex-Rays and Python versions in each
capture receipt. Static capture does not establish semantic or runtime acceptance.

Use the unified entry point:

```powershell
python tools/workspace.py capture-native --input PROJECT_RELATIVE_INPUT --ida-root E:/IDA --no-refresh-gaps
python tools/workspace.py capture-native --input PROJECT_RELATIVE_INPUT --ida-root "D:/ida/IDA Pro 8.3" --no-refresh-gaps
python tools/workspace.py inspect-corpus --no-refresh-gaps
python tools/workspace.py audit-native-chain --query all
```

`capture-native` verifies the indexed original, copies it to private scratch
and rechecks its bytes before starting hidden IDA batch analysis. Results retain
input SHA-256, image base, function boundaries/RVAs, complete disassembly and
pseudocode, raw function chunks, types, names, strings, imports, exports,
references and failures. Each output has its own digest. `capture_complete`
does not mean every decompilation, ABI, local feature or UI chain is complete.

`--resume` verifies previous input and output identities. Function bodies are
reused only when IDA version, Hex-Rays version and capture script digest agree,
and start/end addresses still match. Otherwise bodies are generated with the
actual analyzer. Databases live in `.work/native/`; captures live in
`.analysis/results/native/`. Concurrent captures defer gap reconciliation.

The 9.1 workstation uses the pinned official Python 3.11.9 embeddable runtime under
`tools/analyzers/python-3.11.9/`. Its 35 files were restored from the existing
receipt without changing expected hashes. This is an analyzer dependency,
separate from the product. Installed IDAPython supports `idautils.Structs` and
`StructMembers` through `ida_typeinf`; actual results and failures are captured.

The optional MCP plugin is the pinned 2.0.0 snapshot at commit
`c133c3853faa111a9b00ee615c013b720d0c4acd`; see
[UPSTREAM-PROVENANCE.json](ida-pro-mcp-2.0/UPSTREAM-PROVENANCE.json).
`configure-ida` regenerates the project plugin and loopback registration.
Batch and `--mcp-only` use separate databases. Registration is not a connection
receipt. `probe-ida --input PROJECT_RELATIVE_INPUT --function NAME` verifies
metadata, original/private hashes and complete paginated static bodies before
claiming connection acceptance. This session uses batch IDAPython and does
not inherit an old MCP connection result. The `idalib` backend is not used.

Only static analysis is permitted. Never invoke target exports, helper or
installer entry points, vendor JavaScript, debugger execution, device access or
vendor-service operations. Unresolved branches and decompiler failures remain
explicit and drive further analysis. Real application, window and device
acceptance remains unverified.
