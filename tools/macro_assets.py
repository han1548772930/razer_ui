"""Current Macro SVG preparation; standard library only, no vendor execution."""
import hashlib
import shutil

ASSETS = {
    "delay.svg": "icon_delay_g.5050a3f7.svg",
    "keyboard.svg": "icon_config_keyboard_a.7051c99b.svg",
    "key-down.svg": "icon_key_down.4d5fb90e.svg",
    "key-up.svg": "icon_key_up.5fe01d07.svg",
    "mouse.svg": "icon_config_mouse_o.8a44fbe7.svg",
    "macro.svg": "icon_macro_a.7e1bc94f.svg",
    "launch.svg": "icon_config_launch_p.482fbff5.svg",
    "launch-folder.svg": "icon_folder.0ac33709.svg",
    "command.svg": "icon_runcmd_b.10c00024.svg",
    "text.svg": "icon_config_text_b.bc93ac89.svg",
    "loop.svg": "icon_refresh-1_r.ff48f955.svg",
    "close.svg": "close.1d7eff2a.svg",
    "binding-more.svg": "icon_more_g.32e1e984.svg",
    "binding-close.svg": "icon_close.4f578909.svg",
    "drag.svg": "icon_draggable_g.695879f5.svg",
    "delete.svg": "icon_delete.d1a6a9ad.svg",
    "delete-hover.svg": "icon_delete_r.033c20fb.svg",
    "duplicate.svg": "icon_duplicate.f2ce29fb.svg",
    "phase-held.svg": "icon_keyhold.89fa438f.svg",
    "phase-chevron.svg": "icon_chevron.6a725e60.svg",
    "phase-chevron-hover.svg": "icon_chevron_r.0ba840e4.svg",
    "drag-delay.svg": "icon_delay_d.224b6fe2.svg",
    "drag-keyboard.svg": "icon_config_keyboard_d.2a40b559.svg",
    "drag-mouse.svg": "icon_config_mouse_d.7f54db58.svg",
    "drag-macro.svg": "icon_config_macro_d.7b17228b.svg",
    "drag-launch.svg": "icon_config_launch_d.483e2609.svg",
    "drag-command.svg": "icon_runcmd_d.9aeb6de2.svg",
    "drag-text.svg": "icon_config_text_d.b54bb44d.svg",
    "drag-loop.svg": "icon_refresh-1_d.b0292515.svg",
    "drag-layers.svg": "icon_layers_d.2816d5ba.svg",
    "new.svg": "icon_new_marco.6edec51b.svg",
    "new-hover.svg": "icon_new_marco-hover.47953e67.svg",
    "new-active.svg": "icon_new_marco-pressed.0faeaad4.svg",
    "folder-add.svg": "icon_addfolder-1.3c65591c.svg",
    "folder-add-hover.svg": "icon_addfolder-hover.a804c680.svg",
    "folder-add-active.svg": "icon_addfolder-pressed.aa5ec3ce.svg",
    "folder.svg": "icon_folder_close-2.523e657a.svg",
    "folder-open.svg": "icon_folder_open.076142a3.svg",
    "file.svg": "icon_macro-file.2f24e5a0.svg",
    "file-active.svg": "icon_marco-file-hover.0e086ddd.svg",
    "onboarding-record.svg": "Onboarding_Step2.8d4382a1.svg",
    "onboarding-add.svg": "Onboarding_Step3.ad2907e8.svg",
    "indicator.svg": "indicator_animated.b7ce7af4.svg",
    "warning.svg": "icon_warning.6c0cd78b.svg",
    "add.svg": "icon_add.a38ffd57.svg",
    "undo.svg": "icon_undo.638cc83d.svg",
    "undo-enable.svg": "icon_undo_enable.dc068f5b.svg",
    "undo-hover.svg": "icon_undo_hover.e7a454de.svg",
    "redo.svg": "icon_redo.6df14b75.svg",
    "redo-enable.svg": "icon_redo_enable.9fb6a098.svg",
    "redo-hover.svg": "icon_redo_hover.d651f41e.svg",
    "record.svg": "icon_record.9183ffe6.svg",
    "record-expand.svg": "icon_expand_d.0cbe4fcf.svg",
    "record-expand-hover.svg": "icon_expand_d.3.2a1d50df.svg",
    "tree-more-hover.svg": "icon_more-pressed.f7f8588b.svg",
    "tree-more-active.svg": "icon_more-hover.aa4fe640.svg",
    "drag-folder.svg": "icon_folder_g.220b24b0.svg",
    "drag-file.svg": "icon_layer.0c20e889.svg",
}

def prepare(root, output):
    source = root / ".ref/applications/synapse/macro/static/media"
    target = output / "macro"
    target.mkdir(exist_ok=True)
    records = []
    for name, original in ASSETS.items():
        src, dst = source / original, target / name
        shutil.copyfile(src, dst)
        digest = hashlib.sha256(src.read_bytes()).hexdigest()
        records.append(dict(source=src.relative_to(root).as_posix(),
            output=dst.relative_to(root).as_posix(), source_sha256=digest, sha256=digest,
            source_url="https://apps.razer.com/synapse/macro/static/media/" + original))
    return records
