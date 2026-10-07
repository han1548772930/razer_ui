# Macro Phased editor — current source audit

The current Macro bundle defines three explicit sections: `pressEvents`, `holdEvents`, and `releaseEvents`. The selector remains gated by `global.macro.phasedMacro`; the native editor renders explicit saved Phased documents without inventing that capability response.

Palette insertion uses the active phase. Dropping on a phase header assigns that phase to the moved rows, while an ordinary row drop adopts the preceding defined phase or the next defined phase and then applies the source stable phase ordering. Phase headers preserve the current source geometry: a 1px section border, 50px header, 42px rows, zero-height per-section drop space, and a 100px trailing drop space.

Launch overlays use the phase-aware row offset so the six-parent Phased layout accounts for visible section headers and expanded rows. As re-audited on 2026-10-07, the phase record control now selects its phase and invokes the shared recorder/countdown entry, matching current Ka. See [the follow-up review](macro-recording-review-2026-10-07.md) for the source receipt, cancellation fixes and still-unverified runtime boundary.

Static verification is recorded in [macro-phased-current-evidence.json](macro-phased-current-evidence.json) and can be regenerated with `node tools/audit-macro-phased.cjs`. No application, vendor JavaScript, build, or runtime test was used.
