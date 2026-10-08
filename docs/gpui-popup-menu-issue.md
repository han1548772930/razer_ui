# Feature request: expose PopupMenu presentation parts without replacing its behavior

## Problem

I am implementing a Windows tray context menu using `gpui-kit`. The application's reference UI uses a dark Chromium Views menu, with its own typography, icon column, item padding, separators and surface shape.

I want to retain `PopupMenu`'s keyboard navigation, selection, focus, submenu handling and dismissal, while configuring its presentation in the application. Currently, the public API does not expose all the parts needed to do this. This is a presentation API request, not a report that menu commands or keyboard navigation are broken.

## Version and source inspected

- Published dependency: `gpui-kit 0.7.1` / `gpui-component 0.7.1`.
- Also checked the local `longbridge/gpui-kit` checkout at commit `1e41f17e27ea46589af9c6c428d18b5bf23ef4f6`; the relevant menu source has no local modifications.
- This report is based on static source inspection. The reproduction below has not been run.

## Minimal example using the current API

```rust
use gpui_kit::*;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};

fn tray_menu(window: &mut Window, cx: &mut App) -> Entity<PopupMenu> {
    PopupMenu::build(window, cx, |menu, _, _| {
        menu.item(PopupMenuItem::element(|_, _| {
            div().p_0().h(px(32.)).child("Application")
        }))
        .separator()
        .item(PopupMenuItem::new("Settings"))
    })
}
```

Styling the custom element changes its content, but does not provide access to the surrounding item wrapper, items container or separator presentation.

## Current limitations, with source references

| Part | Current implementation | Missing application control |
| --- | --- | --- |
| Surface | [`PopupMenu::render`](https://github.com/longbridge/gpui-kit/blob/1e41f17e27ea46589af9c6c428d18b5bf23ef4f6/crates/component/src/menu/popup_menu.rs#L1435) applies `popover_style(cx)` | Per-menu background, border, corner radius, shadow and typography refinements |
| Items container | The same render method applies `.p_1().gap_y_0p5().min_w(rems(8.))` | Padding and item gap; removing the built-in width floor when needed |
| Item wrapper | [`render_item`](https://github.com/longbridge/gpui-kit/blob/1e41f17e27ea46589af9c6c428d18b5bf23ef4f6/crates/component/src/menu/popup_menu.rs#L1210) applies its own horizontal padding, typography, radius and minimum content height | Item geometry and typography independently of custom content |
| Separator | [`Separator` rendering](https://github.com/longbridge/gpui-kit/blob/1e41f17e27ea46589af9c6c428d18b5bf23ef4f6/crates/component/src/menu/popup_menu.rs#L1260) uses fixed margins and a `2px` bottom border | Separator space, line thickness, line position, inset and color |
| Selected/hovered item | [`MenuItemElement`](https://github.com/longbridge/gpui-kit/blob/1e41f17e27ea46589af9c6c428d18b5bf23ef4f6/crates/component/src/menu/menu_item.rs#L94) applies theme accent colors | Per-menu semantic selection styles that work for both mouse and keyboard selection |
| Icon/check column | [`render_icon`](https://github.com/longbridge/gpui-kit/blob/1e41f17e27ea46589af9c6c428d18b5bf23ef4f6/crates/component/src/menu/popup_menu.rs#L1163) controls the reserved column | An explicit icon slot/column contract for differently sized raster icons and rows without icons |

`MenuItemElement` implements `Styled`, but is `pub(crate)` and constructed inside `PopupMenu`. `PopupMenuItem::element` supplies the inner content, rather than the complete styled item. The inspected `PopupMenu` does not implement `Styled` or offer presentation-part setters.

Changing the global theme affects other surfaces and does not expose the missing geometry. Reimplementing the menu would duplicate behavior that the framework already handles.

## Requested capability

Please expose an opt-in presentation API for the surface, items container, item wrapper, separator and icon/check column. It could use `Styled`, explicit parts, typed semantic state styles, or item renderers that receive the framework's actual selected/disabled state. The API design is up to the maintainers.

The application should be able to:

- Configure menu appearance locally, without changing the global theme.
- Configure item padding, gaps and typography, including system menu fonts.
- Render a thin separator within independently configured vertical space.
- Render differently sized raster icons in an aligned column, reserving that column for rows without icons.
- Style keyboard and mouse selection consistently, without maintaining a second selection index.

These controls should keep the existing default appearance and preserve `PopupMenu`'s navigation, disabled-item skipping, focus, command dispatch, dismissal and submenu behavior.

## Acceptance criteria

An application can create two menus with different local presentation using the public API, while both retain the standard menu behavior. It can do so without vendoring the crate, accessing private fields, inspecting arbitrary descendants or duplicating keyboard navigation.
