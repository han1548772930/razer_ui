# 679 mounted Joystick mapping editor

## Current source acquisition

This checkpoint directly reads the current official product JS/CSS corpus. Maintained tools/audit_keyboard_mapping_joystick.cjs parses it statically with Acorn; no vendor JavaScript executes. keyboard-joystick-current-source.json preserves UTF-8 byte offsets, function/class boundaries, raw mounted59184 module, helpers, dropdown/radio/option widgets, caller, CSS and original SHA-256.

| Current source | SHA-256 |
| --- | --- |
| local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js | 3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20 |
| local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/MapJoyStick.4500df41.chunk.js | 0eb942f711694d116ab669fe4e758e2de53f4da9fa9364d8f41d7de2ecbe9f03 |
| local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/2383.d3b2762a.chunk.js | c037f08e5bd2fc3c62d3db4a831f1fd83949017fc33f88b14a4df62634aa71b6 |

Shared12383 category loader maps JOYSTICK (54693.lZK) to chunk545/module59184 MapJoyStick. This is the actual mounted popup editor, independent from Quick Remapping. Source render mounts only Button/Direction radios and dropdowns; the joystick graph/simulator seen elsewhere is not mounted here.

## Source semantics

Button dropdown has source NONE placeholder and24 assignments as two-digit strings01..24. Direction has positive/negative X/Y/Z, four X/Y diagonals, six rotating axes rX/rY/rZ and the original separators. Zero/NONE is the SELECTAKEY display placeholder and is omitted from the open list by shared27734; explicit picks require index>0. Labels remain source BUTTON, DIRECTIONNOSTROMO, localized directional keys and raw r-axis labels.

Button saveMapping calls29267.rq(activeButton,joystickGroup,isHyperShift,true), then attaches {joystickMode:button,joystickButtonAssignment:string,isJoystickMovement:false}. Direction uses joystickMode:axes or raxes, isJoystickMovement:true and joystickAxisAssignment values?128. X+ is128; Y- is-128; Z+ is128; diagonals preserve both source signs. No generic HID joystick protocol is inferred from these profile payloads.

Analog dropdown guards A/b inspect both primary and secondary mapping slots in the active Hypershift layer; nonanalog R/x inspect primary only. Button duplicate requires source joystickButtonAssignment, and analog button duplicate also requires mode button. Direction uses29267.uw to reconstruct the source assignedValue name. Placeholder/divider entries remain exempt. Existing raw assignments load their radio/index without fake selection. Once explicit selection exists,12383 isFunctionConfigured uses that selection; an empty index disables Save.

Secondary Direction is omitted when primary staged Controller selection is analog or primary Joystick is direction. Changing primary selection resets an existing secondary Direction to its remembered Button selection with source enableSave based on that index. Current678/679/688 do not offer creating secondary Joystick through their category list because isShowDoubleMappingOption is false; existing source secondary mappings are retained, and the guard is still implemented. Primary direction hides actuation; Button retains mounted TwoTap actuation. No graph or extra deadzone/sensitivity controls are added.

Radio switching preserves each dropdown index while mounted. Shared27734 closes on outside click/window blur and removes listeners on unmount, flips above when selector bottom+182px exceeds the window, uses180px maximum list height,25px rows,27px selector and source z-index101/100. CSS makes Joystick dropdown width180px and left margin30px. Selected text is#44d62c; disabled options use30% opacity and preserve the original restriction.

## Resource preparation

Current icon_expand.55a47b0c.svg SHA-256 is eb1db88587b95be8ff747cb778ccad15c5ec608dd87d44bd400b6533b649df88. Existing embedded assets/synapse/expand.svg SHA-256 is 238cd792ac3ceebc43269b801b482480281b453f3ef783aecb27620c53bd72d5; the audit verifies equality after line-ending normalization and records that the original bytes differ. The difference is LF versus CRLF only. The current SVG was inspected directly and the renderer reuses the re-audited embedded asset. This is not evidence inferred from an older audit document.

## Rust implementation and connection

keyboard_mapping_joystick.rs implements source tables, payload conversions, assignment reconstruction, both radio/dropdown states, duplicate guards, secondary hide/reset, source placeholders, popup bounds/priority and cleanup. Drawer slots retain remembered selection as explicitly local snapshot state; open menu/bounds are skipped during serialization. Source Save/Cancel/header dirty-confirm remains in keyboard_mapping_drawer.rs. Selection creates only a staged payload; Save flows through TwoTap getter/full-list merge and existing ON_SET_KEYMAPPING intent. Cancel removes the popup without submitting. Category changes preserve actuation/rapid metadata; joystick primary removes rapid in the audited existing-list merge branch.

No editor-specific success or error state is invented. Current MapJoyStick logs an assignment comparison exception and exposes no own error dialog. Device submission uses the existing generation/error chain; the current shell still reports Keyboard actuation device adapter is not implemented. This checkpoint does not claim successful service/device write-back.

## Verification and remaining full-scope work

Static extraction, raw corpus parsing, resource equality validation and rustfmt completed. Three pure Rust unit tests cover all16 directional sign/mode goldens across the three products, active-layer/both-slot duplicate filtering and source selection validity/retention. Parent owns cargo check/test execution and reports its result separately. No application, downloaded JS, native binary, helper, installer or device operation ran.

Runtime visual/device acceptance remains absent. Shared dropdown transition timing, exact radio skin, selected-item scroll positioning and original hover-tip portal need visual verification and any remaining parity edits; broader mapping warning/error alerts, true analog event callback acquisition/cleanup, service/native persistence and readback remain required full-program gaps. These do not hide vendor categories or fabricate device results.
