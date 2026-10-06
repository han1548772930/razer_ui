# Local Studio window integration

2026-10-06. The current `/synapse/chroma-studio/` feature is the named
`chroma-studio` subtab inside the `chroma-app` host window. Chroma Dashboard
remains its own root in that host. The app picker, its inherited shortcut route and Chroma Dashboard's
Studio module row open the Studio session directly, based on local page
capability. This does not set an installed-module or online-account flag.

The current host `initChromaAppSubTabManager` registers Studio under the Chroma
app subtab manager. The implementation opens/focuses `chroma-app` before
selecting the separate Studio root; it never opens a policy-5 Studio window.
The shared host TabUI frame/tab/control geometry is reused. Closing the Studio
tab selects Dashboard and does not discard its retained draft session.

`AppShell` owns `StudioSession`, so closing and reopening the native window
retains in-memory edits. The session owns the feature entity, source Save event
subscription and a serialized background save queue. Repeated Open focuses the
existing Chroma host window. Its established 1280 × 720 Chroma bounds remain in
effect. The route/root receipts are in `chroma-studio-source.json`.
Host registration, TabUI geometry and current favicon receipts are in
`chroma-studio-window-source.json`. Host recreation refreshes its handle and
replaces the retained page subscription. The subscription uses `cx.subscribe`,
not `subscribe_in` (GPUI's `ensure_window` keeps its first entity association).
Settings/tour commands explicitly activate and navigate the owning shell's main
window. Studio activation is deferred out of the Chroma host event callback,
avoiding a recursive update of the already borrowed host entity. Selecting
Studio focuses its feature handle; returning to Dashboard focuses its own handle.

Save writes `%APPDATA%/razer_ui/chroma-studio-draft.json` (or the same fallback
directory as the workspace store). This is a local schema-versioned draft, not
a vendor device profile. Deserialization rejects unknown fields and unsupported
versions, and validates source effect values and unique layer IDs. Invalid files
are reported and never overwritten as an empty document. Writes check the
observed file bytes, stage and sync a temporary file, back up the prior bytes,
then rename. An external file change blocks the write rather than losing it.
The installed Rust standard-library Windows implementation was read statically:
`std/src/sys/fs/windows.rs::rename` calls `MoveFileExW` with
`MOVEFILE_REPLACE_EXISTING`, so an existing destination is supported on Windows.
The committed snapshot alone is marked saved; edits made while a write runs
remain dirty. Failures stay visible in the Studio window. A successful write
does not report that any device or SDK received a command.
While a save is pending, Save remains available even when Undo has returned to
the previously saved document. That explicit new snapshot queues behind the
in-flight snapshot, preventing an earlier save from winning over a later Save.
The session sets the feature's pending flag before starting/queuing each request
and reconciles it after completion, failure and follow-up queue startup.

No application, test or build was executed. Rust formatting was performed;
the root agent records the unified permitted cargo check result. Shell Exit
drains already requested Studio saves alongside its workspace writer; a failed
Studio save cancels the exit intent. Unsubmitted layer edits do not become an
implicit Save. Closing only the Studio window keeps the session and task alive.
