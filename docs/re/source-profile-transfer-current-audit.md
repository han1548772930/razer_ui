# Current source product profile transfer

2026-10-04. Audited products: 164, 241, 778, 784, 3871, 3884, 3886 and 3946.

Each current mounted ImportExportModal and its row, cloud and local-browse children is bound to its own manifest-declared source. All eight use a 602×481 modal at top 104 and left calc(50% − 300px), a 36px header, 49px Local/Cloud area and 47px browse area.

The profile menu now mounts a retained native dialog. Export shows actual local profile names; seven products initially select the active profile, while 3886 initially selects all. The source’s initially Deselect all link toggles independently from individual selections. No preset is inferred from a user-editable profile name.

Import opens a real single-file OS picker, preserves the previous path on cancellation, and guards late results after switching Local/Cloud or closing. Seven products request .synapse4 and open from the whole filename row. Current 3886 has no accept filter and opens only from the folder image; these differences are retained.

Current 3886 also enables its compatible-device cloud dropdown. Without connected compatible cloud devices the native dropdown remains disabled and states that cloud profiles are unavailable. The other seven retain the actual source-disabled cloud notice and cone resource.

The final Import/Export buttons remain disabled because vendor .synapse4 encoding/decoding and device services are unavailable. Selected files are not read or executed. The unrelated razer-ui-profile JSON format is never substituted. No imported profile, remote data, macro row or transfer success is fabricated.

The close, folder and cone SVGs match actual current 3946 resources byte for byte. The folder and cone were fetched from their exact production manifest paths using the maintained snapshot helper; HTTP receipts remain next to those source resources.

The source has no Escape, Enter or backdrop-close handlers. Base Dialog/DialogPopup consume those actions, trap focus and occlude underlying content; explicit close/cancel emits one retained close event and the owner restores profile focus.

Run `node tools/audit-source-profile-transfer.cjs --check` for a read-only source/asset/native-route audit. Formatting and parent-owned cargo check are permitted; applications, builds, tests, downloaded JavaScript and DLLs were not executed. Pixel, animation and interactive behavior have not been run.

See [source-profile-transfer-current-evidence.json](source-profile-transfer-current-evidence.json) for complete current receipts and explicit native limitations.
