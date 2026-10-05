# Macro Phased editor — current source audit

The current Macro bundle defines three explicit sections: `pressEvents`, `holdEvents`, and `releaseEvents`. The selector remains gated by `global.macro.phasedMacro`; the native editor renders explicit saved Phased documents without inventing that capability response.

Palette insertion uses the active phase. Dropping on a phase header assigns that phase to the moved rows, while an ordinary row drop adopts the preceding defined phase or the next defined phase and then applies the source stable phase ordering. Phase headers preserve the current source geometry: a 1px section border, 50px header, 42px rows, zero-height per-section drop space, and a 100px trailing drop space.

Launch overlays use the phase-aware row offset so the six-parent Phased layout accounts for visible section headers and expanded rows. The phase record control selects a phase and reports the native recorder/service boundary; it does not claim a recording implementation that is not present.

Static verification is recorded in [macro-phased-current-evidence.json](macro-phased-current-evidence.json) and can be regenerated with `node tools/audit-macro-phased.cjs`. No application, vendor JavaScript, build, or runtime test was used.
