//! Current 1382 mapping-root renderSaveAlert and 913 closeMapping/saveMapping.
use super::*;
use gpui_kit::base::{Button as BaseButton, Dialog};
use std::time::Instant;

pub(super) struct WarningState {
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    opened: Instant,
    departure: Option<Departure>,
}

impl AudioEditor {
    pub(super) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.changed {
            cx.emit(EditorEvent::Cancel);
            return;
        }
        self.open_warning(None, window, cx);
    }

    pub(super) fn request_departure(
        &mut self,
        destination: Departure,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.changed {
            return false;
        }
        self.open_warning(Some(destination), window, cx);
        true
    }

    fn open_warning(
        &mut self,
        departure: Option<Departure>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.warning.is_some() {
            return;
        }
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.warning = Some(WarningState {
            focus,
            return_focus,
            opened: Instant::now(),
            departure,
        });
        cx.notify();
    }

    fn hide_warning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.warning.take() {
            if let Some(focus) = state.return_focus {
                focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    fn dismiss_warning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // dismissSave(true) -> saveMapping(false,true): keep the parent's dirty
        // flag and the child draft, but re-enable the ordinary Save action.
        self.can_save = true;
        self.hide_warning(window, cx);
    }

    fn dont_save_warning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // closeMapping(false) only supplied nextAction=clear, no dontSaveAction.
        // Therefore Don't Save clears dirty but neither clears nor leaves the editor.
        self.can_save = true;
        self.changed = false;
        self.hide_warning(window, cx);
    }

    fn save_warning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The source alert Save has no canSave gate. It calls saveMapRef(true),
        // then the requested close continuation. Only the local draft is submitted.
        let mapping = match self.mapping() {
            Ok(mapping) => mapping,
            Err(error) => {
                self.save_error = Some(error);
                cx.notify();
                return;
            }
        };
        let departure = self
            .warning
            .as_ref()
            .and_then(|state| state.departure.clone());
        self.hide_warning(window, cx);
        if let Some(path) = departure {
            cx.emit(EditorEvent::SaveThen(mapping, path));
        } else {
            cx.emit(EditorEvent::Save(mapping));
        }
    }

    pub(super) fn render_warning(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(state) = &self.warning else {
            return div().into_any_element();
        };
        let opacity = (state.opened.elapsed().as_secs_f32() / 0.1).clamp(0., 1.);
        if opacity < 1. {
            window.request_animation_frame();
        }
        let dont_save = t(&data().labels["set"]);
        let actions = h_flex()
            .justify_center()
            .mt(surface::css(20.))
            .mr(surface::css(-10.))
            .child(action(
                "pod-warning-dont-save",
                dont_save,
                false,
                window,
                cx,
                Self::dont_save_warning,
            ))
            .child(action(
                "pod-warning-save",
                t(&data().labels["W0P"]),
                true,
                window,
                cx,
                Self::save_warning,
            ));
        let panel = v_flex()
            .relative()
            .occlude()
            .w(surface::css(400.))
            .py(surface::css(20.))
            .px(surface::css(30.))
            .border_1()
            .border_color(Colors::selected())
            .rounded(surface::css(5.))
            .bg(Colors::warning_panel())
            .text_color(Colors::text())
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_center()
            .child(
                BaseButton::new("pod-warning-close")
                    .accessibility_label(t("CLOSE"))
                    .absolute()
                    .top(surface::css(8.))
                    .right(surface::css(8.))
                    .size(surface::css(20.))
                    .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                    .on_click(cx.listener(|this, _, window, cx| this.dismiss_warning(window, cx))),
            )
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(Colors::selected())
                    .mb(surface::css(20.))
                    .child(t(&data().labels["Ep9"]).to_uppercase()),
            )
            .child(format!(
                "{}\n\n{}",
                t(&data().labels["Lvx"]),
                t(&data().labels["ENf"])
            ))
            .when_some(self.save_error.as_ref(), |panel, error| {
                panel.child(note(error.clone(), true))
            })
            .child(actions);
        Dialog::new(cx)
            .focus_handle(state.focus.clone())
            .close_on_backdrop_press(false)
            .close_on_escape(false)
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(Colors::warning_backdrop())
                    .opacity(opacity),
            )
            // CSS top:50%; transform:translateY(-100%) places the panel's
            // bottom edge at half of the viewport, not its center there.
            .popup(
                div()
                    .absolute()
                    .left_0()
                    .bottom(relative(0.5))
                    .w(window.viewport_size().width)
                    .flex()
                    .justify_center()
                    .opacity(opacity)
                    .child(panel),
            )
            .into_any_element()
    }
}

fn action(
    id: &'static str,
    label: String,
    primary: bool,
    window: &mut Window,
    cx: &mut Context<AudioEditor>,
    handler: fn(&mut AudioEditor, &mut Window, &mut Context<AudioEditor>),
) -> AnyElement {
    let pointer = surface::pointer_state(id, window, cx);
    let (hovered, pressed) = pointer.read(cx).sample();
    let opacity = surface::fade_opacity(
        id,
        if pressed {
            0.6
        } else if hovered {
            0.8
        } else {
            1.
        },
        300,
        window,
        cx,
    );
    let wide = !primary && label.chars().count() > 9;
    let button = BaseButton::new((ElementId::from(id), "action"))
        .accessibility_label(label.clone())
        .min_w(surface::css(100.))
        .h(surface::css(27.))
        .border_1()
        .border_color(Colors::warning_button_border())
        .rounded(surface::css(3.))
        .pl(surface::css(if wide { 16. } else { 6. }))
        .pr(surface::css(if wide { 16. } else { 10. }))
        .pt(surface::css(6.))
        .pb(surface::css(7.))
        .mr(surface::css(10.))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .bg(if primary {
            Colors::selected()
        } else {
            Colors::warning_secondary()
        })
        .text_color(if primary {
            Colors::warning_button_border()
        } else {
            Colors::warning_secondary_text()
        })
        .child(label.to_uppercase())
        .on_click(cx.listener(move |this, _, window, cx| handler(this, window, cx)));
    surface::track_pointer(
        div().id(id).opacity(opacity).child(button),
        &pointer,
        window,
    )
    .into_any_element()
}
