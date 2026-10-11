# Current analog keyboard resource identity

The 2026-10-11 static comparison independently resolves the current asset manifests for 678, 679 and 688. All ten resources below are byte-identical across these products and to their prepared Rust assets. The resource aliases no longer imply a 679-only UI condition.

| Source resource key | Rust resource path |
| --- | --- |
| `static/media/chroma_sync_v3_static.svg` | `synapse/chroma_sync_v3_static.svg` |
| `static/media/controller-fill-green-icon.svg` | `synapse/controller-fill-green-icon.svg` |
| `static/media/controller-fill-grey-icon.svg` | `synapse/controller-fill-grey-icon.svg` |
| `static/media/icon_expand.svg` | `synapse/icon_expand.svg` |
| `static/media/controller_icon_qe.svg` | `synapse/controller_icon_qe.svg` |
| `static/media/controller_icon_wasd.svg` | `synapse/controller_icon_wasd.svg` |
| `static/media/icon_sidepanel_a.svg` | `synapse/icon_sidepanel_a.svg` |
| `static/media/icon_sidepanel.svg` | `synapse/icon_sidepanel.svg` |
| `static/media/xbox-trigger-left.svg` | `synapse/xbox-trigger-left.svg` |
| `static/media/xbox-trigger-right.svg` | `synapse/xbox-trigger-right.svg` |

Acquisition and resource semantics: [exact receipt](keyboard-analog-shared-assets-current-source.json) records each product's manifest hash, original hashed filename, byte hash and inert JS/CSS reference snippets with UTF-8 and UTF-16 ranges. [Static comparison tool](../../tools/audit-keyboard-analog-shared-assets-current.py) checks the original bytes; it never evaluates vendor code.

Rust implementation: the embedded resource table is `assets/synapse/keyboard-analog-embedded.rs`; the current keyboard/analog renderers use source resource names. The original bytes remain unchanged. Matching resources alone do not prove equal mounted layouts, state predicates or click handlers; those still require their individual current-source audits.

UI/backend connection and runtime acceptance: this change only corrects resource naming and provenance. No device read/write or native save acknowledgement is inferred. The application, vendor JavaScript and DLLs were not executed.
