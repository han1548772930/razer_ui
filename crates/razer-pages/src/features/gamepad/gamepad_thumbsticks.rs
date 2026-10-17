//! Current mounted Thumbsticks branches: legacy progress controls and the
//! 2676/2684 controller widgets. The complete AST/CSS/resource producers are
//! retained by `gamepad_thumbsticks` in gamepad-main-audit.json.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use razer_widgets::theme::GamepadThumbstickColors as Colors;
use std::{sync::Arc, time::Duration};

struct Control {
    slider: Entity<SliderState>,
    focus: FocusHandle,
    preview: Option<i64>,
    commit: Option<Task<()>>,
}

#[derive(Default)]
pub(super) struct ThumbstickUi {
    controls: BTreeMap<String, Control>,
    tutorial: bool,
    artwork: Option<Arc<Image>>,
}

impl ThumbstickUi {
    pub(super) fn artwork_pending(&self) -> bool {
        self.artwork.is_none()
    }
}

fn nearest_step(value: f32, steps: &[i64]) -> i64 {
    // Source reduce replaces only for a strictly smaller distance. A tie
    // therefore preserves the first source dataset member.
    steps
        .iter()
        .copied()
        .reduce(|best, step| {
            if (step as f32 - value).abs() < (best as f32 - value).abs() {
                step
            } else {
                best
            }
        })
        .expect("current thumbstick step dataset")
}

fn next_step(value: i64, steps: &[i64], increase: bool) -> i64 {
    let index = steps.iter().position(|step| *step == value).unwrap_or(0);
    let next = if increase {
        index.saturating_add(1)
    } else {
        index.saturating_sub(1)
    };
    steps.get(next).copied().unwrap_or(value)
}

fn circle_radius(svg: &mut String, class: &str, radius: i64) -> anyhow::Result<()> {
    let class = format!("class=\"{class}\"");
    let position = svg
        .find(&class)
        .ok_or_else(|| anyhow::anyhow!("Missing source sensitivity circle"))?;
    let begin = svg[..position]
        .rfind("<circle")
        .ok_or_else(|| anyhow::anyhow!("Sensitivity class is not a circle"))?;
    let end = position
        + svg[position..]
            .find('>')
            .ok_or_else(|| anyhow::anyhow!("Incomplete sensitivity circle"))?;
    let radius_begin = begin
        + svg[begin..end]
            .find(" r=\"")
            .ok_or_else(|| anyhow::anyhow!("Missing source circle radius"))?
        + 4;
    let radius_end = radius_begin
        + svg[radius_begin..end]
            .find('"')
            .ok_or_else(|| anyhow::anyhow!("Incomplete circle radius"))?;
    svg.replace_range(radius_begin..radius_end, &radius.to_string());
    Ok(())
}

impl GamepadProductWorkspace {
    fn modern_thumbsticks(&self) -> bool {
        matches!(self.spec.product_id, 2676 | 2684)
    }

    pub(super) fn initialize_thumbstick_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.spec.pages.iter().any(|page| page == "THUMBSTICKS") {
            return;
        }
        self.thumbstick_ui.tutorial =
            !self.clutch_assigned("leftStick") || !self.clutch_assigned("rightStick");
        for side in ["leftStick", "rightStick"] {
            for sensitivity in [false, true] {
                let field = if sensitivity {
                    "sensitivityValue"
                } else {
                    "deadzoneValue"
                };
                let path = format!("/controller/{side}/{field}");
                let steps = if sensitivity {
                    &self.spec.sensitivity_steps
                } else {
                    &self.spec.deadzone_steps
                };
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(*steps.first().expect("current slider minimum") as f32)
                        .max(*steps.last().expect("current slider maximum") as f32)
                        .step(if self.modern_thumbsticks() {
                            if sensitivity { 10. } else { 1. }
                        } else {
                            0.001
                        })
                        .default_value(self.number(&path) as f32)
                });
                self.thumbstick_ui.controls.insert(
                    path.clone(),
                    Control {
                        slider: slider.clone(),
                        focus: cx.focus_handle().tab_stop(true),
                        preview: None,
                        commit: None,
                    },
                );
                self.subscriptions.push(cx.subscribe_in(
                    &slider,
                    window,
                    move |this, _, event, window, cx| {
                        if this.syncing || this.page != "THUMBSTICKS" {
                            return;
                        }
                        if sensitivity && !this.clutch_assigned(side) {
                            return;
                        }
                        let steps = if sensitivity {
                            &this.spec.sensitivity_steps
                        } else {
                            &this.spec.deadzone_steps
                        };
                        match event {
                            SliderEvent::Change(value) => {
                                let value = nearest_step(value.start(), steps);
                                if this.modern_thumbsticks() {
                                    this.commit_thumbstick_value(&path, value, window, cx);
                                    this.sync_thumbstick_controls(window, cx);
                                } else if let Some(control) =
                                    this.thumbstick_ui.controls.get_mut(&path)
                                {
                                    control.preview = Some(value);
                                    control.commit = None;
                                    cx.notify();
                                }
                            }
                            SliderEvent::Release(value) if !this.modern_thumbsticks() => {
                                let value = nearest_step(value.start(), steps);
                                this.delay_thumbstick_commit(path.clone(), value, window, cx);
                            }
                            SliderEvent::Release(_) => {}
                        }
                    },
                ));
            }
        }
        self.refresh_thumbstick_artwork(cx);
    }

    fn delay_thumbstick_commit(
        &mut self,
        path: String,
        value: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = path.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(control) = this.thumbstick_ui.controls.get_mut(&key) {
                    control.preview = None;
                }
                this.commit_thumbstick_value(&key, value, window, cx);
                this.sync_thumbstick_controls(window, cx);
            });
        });
        if let Some(control) = self.thumbstick_ui.controls.get_mut(&path) {
            control.commit = Some(task);
        }
    }

    fn commit_thumbstick_value(
        &mut self,
        path: &str,
        value: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if path.ends_with("/deadzoneValue") {
            if value < 7 {
                let previous = self
                    .previous_deadzones
                    .get(path)
                    .or_else(|| self.draft.pointer(path))
                    .cloned()
                    .expect("source previous deadzone");
                self.low_deadzone = Some((path.to_owned(), previous));
                self.deadzone_dialog = Some(deadzone_dialog::DialogState::new(window, cx));
            } else {
                self.previous_deadzones
                    .insert(path.to_owned(), json!(value));
            }
        }
        let unchanged = self.draft.pointer(path) == Some(&json!(value));
        self.write(path, json!(value), cx);
        // Source progress release invokes its setter even when the nearest
        // step is unchanged; in particular it still opens the low alert.
        if unchanged {
            self.thumbstick_changed(path, cx);
            cx.emit(GamepadProductChanged);
            cx.notify();
        }
    }

    pub(super) fn cancel_thumbstick_edits(&mut self, cx: &mut Context<Self>) {
        for control in self.thumbstick_ui.controls.values_mut() {
            control.preview = None;
            control.commit = None;
        }
        cx.notify();
    }

    pub(super) fn sync_thumbstick_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (path, control) in &self.thumbstick_ui.controls {
            if control.preview.is_none() {
                if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                    control
                        .slider
                        .update(cx, |state, cx| state.set_value(value as f32, window, cx));
                }
            }
        }
    }

    pub(super) fn refresh_thumbstick_artwork(&mut self, cx: &App) {
        if !self.modern_thumbsticks() || self.page != "THUMBSTICKS" {
            return;
        }
        let loaded = (|| -> anyhow::Result<Arc<Image>> {
            let path = |edition| {
                format!(
                    "synapse/gamepad-thumbstick-device/{}/{edition}.svg",
                    self.spec.product_id
                )
            };
            let bytes =
                crate::runtime_resources::load_edition_resource(self.edition_id, |edition| {
                    cx.asset_source().load(&path(edition))
                })?
                .ok_or_else(|| anyhow::anyhow!("Current sensitivity SVG is missing"))?;
            let mut svg = String::from_utf8(bytes.to_vec())?;
            circle_radius(
                &mut svg,
                "left-thumbstick",
                2 + self.number("/controller/leftStick/deadzoneValue"),
            )?;
            circle_radius(
                &mut svg,
                "right-thumbstick",
                2 + self.number("/controller/rightStick/deadzoneValue"),
            )?;
            Ok(Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                svg.into_bytes(),
            )))
        })();
        match loaded {
            Ok(image) => self.thumbstick_ui.artwork = Some(image),
            Err(error) => {
                self.thumbstick_ui.artwork = None;
                eprintln!("Controller sensitivity artwork: {error:#}");
            }
        }
    }

    fn thumbstick_buttons(&self, side: &str) -> String {
        let mut names: Vec<_> = self.draft["profile"]["mappings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|mapping| {
                let assignment = mapping["controllerGroup"]["controllerAssignment"].as_str();
                assignment == Some("GLOBAL_SENSITIVITY_CLUTCH")
                    || assignment
                        == Some(if side == "leftStick" {
                            "LEFT_SENSITIVITY_CLUTCH"
                        } else {
                            "RIGHT_SENSITIVITY_CLUTCH"
                        })
            })
            .filter_map(|mapping| mapping["inputID"].as_str())
            .filter_map(|id| id.split('_').nth(1))
            .map(|name| {
                if name == "BUTTON" {
                    t("SHARE")
                } else {
                    name.to_owned()
                }
            })
            .collect();
        names.sort();
        names.join(", ")
    }

    pub(super) fn thumbsticks(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.modern_thumbsticks() {
            self.thumbstick_widgets(window, cx)
        } else {
            self.thumbstick_legacy(window, cx)
        }
    }

    fn progress_control(
        &self,
        side: &'static str,
        sensitivity: bool,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let field = if sensitivity {
            "sensitivityValue"
        } else {
            "deadzoneValue"
        };
        let path = format!("/controller/{side}/{field}");
        let control = self
            .thumbstick_ui
            .controls
            .get(&path)
            .expect("retained source thumbstick control");
        let value = control.preview.unwrap_or_else(|| self.number(&path));
        let steps = if sensitivity {
            &self.spec.sensitivity_steps
        } else {
            &self.spec.deadzone_steps
        };
        let modern = self.modern_thumbsticks();
        let highlight = !sensitivity && value < 7;
        let color = if highlight {
            Colors::warning()
        } else {
            Colors::accent()
        };
        let maximum = *steps.last().expect("source maximum") as f32;
        let fraction = value as f32 / maximum;
        let tip = if sensitivity {
            if value == 0 {
                t("DISABLED")
            } else {
                (value / 10).to_string()
            }
        } else {
            format!("{value}%")
        };
        let slider = &control.slider;
        let focus = control.focus.clone();
        let key_path = path.clone();
        let input = div()
            .id(SharedString::from(format!(
                "gamepad-thumbstick-input-{path}"
            )))
            .when(enabled, |input| {
                input
                    .track_focus(&focus)
                    .capture_any_mouse_down(move |event, window, cx| {
                        if event.button == MouseButton::Left {
                            focus.focus(window, cx);
                        }
                    })
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        let increase = match event.keystroke.key.as_str() {
                            "left" | "down" => false,
                            "right" | "up" => true,
                            _ => return,
                        };
                        cx.stop_propagation();
                        let steps = if sensitivity {
                            &this.spec.sensitivity_steps
                        } else {
                            &this.spec.deadzone_steps
                        };
                        let next = next_step(this.number(&key_path), steps, increase);
                        this.commit_thumbstick_value(&key_path, next, window, cx);
                        this.sync_thumbstick_controls(window, cx);
                    }))
            });
        let mut root = BaseSlider::new(slider)
            .disabled(!enabled)
            .relative()
            .w(surface::css(if modern { 360. } else { 300. }))
            .h(surface::css(if modern { 43. } else { 26. }))
            .opacity(if enabled { 1. } else { 0.3 });
        if modern {
            root = root
                .child(
                    div()
                        .absolute()
                        .bottom(surface::css(9.))
                        .w_full()
                        .h(surface::css(5.))
                        .rounded(surface::css(6.))
                        .bg(if highlight {
                            Colors::warning_track()
                        } else {
                            Colors::accent_track()
                        })
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .rounded(surface::css(6.))
                                .bg(color),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(surface::css(25.))
                        .left(relative(fraction))
                        .ml(surface::css(8. - 16. * fraction))
                        .w_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .flex_shrink_0()
                                .min_w(surface::css(24.))
                                .px(surface::css(8.))
                                .py(surface::css(4.))
                                .text_size(surface::css(12.))
                                .line_height(surface::css(14.))
                                .text_color(Colors::selected_text())
                                .bg(color)
                                .rounded(surface::css(3.))
                                .child(tip),
                        ),
                )
                .children(
                    [
                        (false, if sensitivity { "0" } else { "0%" }),
                        (true, if sensitivity { "10" } else { "15%" }),
                    ]
                    .map(|(right, label)| {
                        div()
                            .absolute()
                            .bottom(surface::css(-18.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(14.))
                            .when(right, |d| d.right_0())
                            .when(!right, |d| d.left_0())
                            .child(label)
                    }),
                )
                .child(
                    SliderTrack::new(slider)
                        .disabled(!enabled)
                        .absolute()
                        .bottom(surface::css(4.))
                        .w_full()
                        .h(surface::css(16.))
                        .child(
                            SliderIndicator::new(slider)
                                .absolute()
                                .left(surface::css(8.))
                                .right(surface::css(8.))
                                .h_full()
                                .child(
                                    SliderThumb::new(slider)
                                        .disabled(!enabled)
                                        .absolute()
                                        .left(relative(fraction))
                                        .ml(surface::css(-8.))
                                        .size(surface::css(16.))
                                        .rounded_full()
                                        .bg(color)
                                        .hover(move |s| {
                                            s.bg(Colors::thumb_hover())
                                                .border_2()
                                                .border_color(color)
                                        })
                                        .active(move |s| {
                                            s.bg(Colors::thumb_active())
                                                .border_2()
                                                .border_color(color)
                                        }),
                                ),
                        ),
                );
        } else {
            let line = div()
                .absolute()
                .left(surface::css(8.))
                .top(surface::css(12.))
                .w(surface::css(274.))
                .h(surface::css(2.))
                .bg(Colors::stage_text())
                .children(steps.iter().copied().map(|step| {
                    let selected = enabled && step == value;
                    div()
                        .absolute()
                        .left(relative(step as f32 / maximum - 0.03))
                        .top(relative(0.5))
                        .mt(surface::css(-10.))
                        .size(surface::css(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .size(surface::css(if selected { 20. } else { 8. }))
                                .rounded_full()
                                .bg(if selected { color } else { Colors::stage_text() }),
                        )
                        .when(selected, |step| {
                            step.child(
                                div()
                                    .absolute()
                                    .top(surface::css(-40.))
                                    .left(relative(0.5))
                                    .ml(surface::css(-25.))
                                    .w(surface::css(50.))
                                    .h(surface::css(30.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(Colors::bubble())
                                    .border_1()
                                    .border_color(Colors::bubble_border())
                                    .rounded(surface::css(3.))
                                    .text_size(surface::css(14.))
                                    .text_color(Colors::white())
                                    .child(tip.clone())
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    }),
                            )
                        })
                }));
            root = root.child(
                SliderTrack::new(slider)
                    .disabled(!enabled)
                    .relative()
                    .w_full()
                    .h_full()
                    .child(SliderIndicator::new(slider).size_full().child(line)),
            );
        }
        let _ = window;
        input.child(root).into_any_element()
    }

    fn thumbstick_legacy(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let sensitivity = self.sensitivity;
        let compact = window.viewport_size().width < window.rem_size() * (1280. / 16.);
        let bounds = self.thumbstick_bounds.clone();
        let panels = ["leftStick", "rightStick"].map(|side| self.legacy_stick(side, window, cx));
        let pid = self.spec.product_id;
        let ps = matches!(pid, 4133 | 4144);
        let mut center = v_flex()
            .items_center()
            .w(surface::css(460.))
            .relative()
            .gap(surface::css(20.))
            .mb(surface::css(10.))
            .child(self.thumbstick_product_image(window));
        for (left, x, y) in if ps {
            [(true, 155., 110.), (false, 266., 110.)]
        } else {
            [(true, 107., 69.), (false, 265., 126.)]
        } {
            center = center.child(
                img(SharedString::from(format!(
                    "synapse/gamepad-thumbsticks/{pid}/highlight.svg"
                )))
                .absolute()
                .left(surface::css(x))
                .top(surface::css(y))
                .when(ps, |i| i.w(surface::css(38.)))
                .id(if left {
                    "gamepad-left-stick-highlight"
                } else {
                    "gamepad-right-stick-highlight"
                }),
            );
        }
        center = center.child(div().text_size(surface::css(14.)).text_center().child(
            if sensitivity {
                t("SENSITIVITY_DESCRIPTION")
            } else {
                format!(
                    "{}\n\n{}",
                    t("DEADZONE_DESCRIPTION_1"),
                    t("DEADZONE_DESCRIPTION_2")
                )
            },
        ));
        let tab = h_flex()
            .mx_auto()
            .mt(surface::css(30.))
            .mb(surface::css(20.));
        let tab = tab.child(segment(
            "gamepad-thumbstick-tabs",
            [
                ("DEADZONE", !sensitivity),
                ("SENSITIVITY_CLUTCH", sensitivity),
            ],
            cx,
            |this, _, _, cx| {
                this.cancel_thumbstick_edits(cx);
                this.sensitivity = !this.sensitivity;
                this.thumbstick_ui.tutorial = this.sensitivity
                    && !this.clutch_assigned("leftStick")
                    && !this.clutch_assigned("rightStick");
                // The original parent's click handler sees React's previous
                // tab state. Apply that result here before GPUI bubbles the
                // event, so entering Sensitivity does not dismiss its tutorial.
                cx.stop_propagation();
                cx.notify();
            },
        ));
        let mut content = v_flex()
            .w_full()
            .max_w(surface::css(1280.))
            .gap(surface::css(10.))
            .mb(surface::css(20.))
            .child(
                h_flex()
                    .justify_between()
                    .w_full()
                    .when(compact, |d| {
                        d.w(surface::css(800.)).mx_auto().mt(surface::css(40.))
                    })
                    .children(panels),
            )
            .child(
                v_flex()
                    .mx_auto()
                    .when(!compact, |d| d.mt(surface::css(-362.)))
                    .child(center),
            );
        if !sensitivity {
            content = content.child(self.thumbstick_bottom_widgets(cx));
        }
        let mut page = v_flex()
            .relative()
            .font_family("Roboto")
            .text_color(Colors::text())
            .child(tab)
            .child(content)
            .id("gamepad-thumbsticks-source")
            .on_click(cx.listener(|this, _, _, cx| {
                if this.sensitivity {
                    this.thumbstick_ui.tutorial = false;
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |value, window, _| {
                        if bounds.replace(value) != value {
                            window.refresh();
                        }
                    },
                    |_, _, _, _| (),
                )
                .absolute()
                .size_full(),
            );
        if sensitivity
            && self.thumbstick_ui.tutorial
            && !self.clutch_assigned("leftStick")
            && !self.clutch_assigned("rightStick")
        {
            let tutorial = v_flex()
                .occlude()
                .w(surface::css(290.))
                .p(surface::css(20.))
                .gap(surface::css(10.))
                .bg(Colors::panel())
                .border_1()
                .border_color(Colors::warning())
                .rounded(surface::css(3.))
                .child(razer_widgets::tutorial_media::clip(
                    tutorial_asset(pid),
                    t("SENSITIVITY_CLUTCH"),
                    750. / 570.,
                ))
                .child(
                    div()
                        .text_color(Colors::warning())
                        .text_size(surface::css(16.))
                        .child(
                            format!("{} {}", t("ASSIGN"), t("SENSITIVITY_CLUTCH")).to_uppercase(),
                        ),
                )
                .child(
                    div()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .child(t("SENSITIVITY_MAPPING_INFO")),
                )
                .child(
                    source_action("gamepad-thumbstick-tutorial-assign", "ASSIGN_NOW")
                        .w(surface::css(105.))
                        .mx_auto()
                        .mt(surface::css(10.))
                        .bg(Colors::warning())
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(GamepadNavigationRequested {
                                page_key: "TAB_CUSTOMIZE",
                            })
                        })),
                );
            page = page.child(
                h_flex()
                    .absolute()
                    .top(surface::css(200.))
                    .w_full()
                    .justify_center()
                    .child(tutorial),
            );
        }
        page.into_any_element()
    }

    fn legacy_stick(
        &self,
        side: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sensitivity = self.sensitivity;
        let enabled = !sensitivity || self.clutch_assigned(side);
        let left = side == "leftStick";
        let deadzone = self.number(&format!("/controller/{side}/deadzoneValue"));
        let highlight = !sensitivity && deadzone < 7;
        let key = if sensitivity {
            if left {
                "LEFT_THUMBSTICK_TIP_2"
            } else {
                "RIGHT_THUMBSTICK_TIP_2"
            }
        } else if deadzone == 0 {
            "ZERO_DEADZONE_HIGHLIGHT_DESC"
        } else if highlight {
            "LOW_DEADZONE_HIGHLIGHT_DESC"
        } else if left {
            "LEFT_THUMBSTICK_TIP_1"
        } else {
            "RIGHT_THUMBSTICK_TIP_1"
        };
        let text = if !enabled {
            t("SENSITIVITY_ASSIGN_INFO")
        } else {
            t(key).replace("{0}", &self.thumbstick_buttons(side))
        };
        let pid = self.spec.product_id;
        let mode = if sensitivity { "sensitivity" } else { "deadzone" };
        let mut panel = v_flex()
            .relative()
            .items_center()
            .w(surface::css(380.))
            .min_h(surface::css(362.))
            .when(highlight, |d| d.justify_between())
            .child(
                h_flex()
                    .items_center()
                    .gap(surface::css(10.))
                    .child(
                        img(SharedString::from(format!(
                            "synapse/gamepad-thumbsticks/{pid}/{mode}-min.svg"
                        )))
                        .size(surface::css(16.))
                        .when(!enabled, |i| i.opacity(0.3)),
                    )
                    .child(self.progress_control(side, sensitivity, enabled, window, cx))
                    .child(
                        img(SharedString::from(format!(
                            "synapse/gamepad-thumbsticks/{pid}/{mode}-max.svg"
                        )))
                        .size(surface::css(30.))
                        .when(!enabled, |i| i.opacity(0.3)),
                    ),
            );
        if !enabled {
            panel = panel.child(
                source_action(
                    if left {
                        "gamepad-left-stick-assign"
                    } else {
                        "gamepad-right-stick-assign"
                    },
                    "ASSIGN",
                )
                .w(surface::css(90.))
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(GamepadNavigationRequested {
                        page_key: "TAB_CUSTOMIZE",
                    })
                })),
            );
        }
        panel = panel.child(
            h_flex()
                .relative()
                .items_start()
                .gap(surface::css(5.))
                .w(surface::css(380.))
                .mt(surface::css(if enabled { 20. } else { 10. }))
                .mb(surface::css(20.))
                .min_h(surface::css(if enabled { 34. } else { 17. }))
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .when(highlight, |d| d.text_color(Colors::warning()))
                .when(!sensitivity && deadzone == 0, |d| {
                    d.child(surface::help_control(
                        if left {
                            "gamepad-left-zero-deadzone-tip"
                        } else {
                            "gamepad-right-zero-deadzone-tip"
                        },
                        t("ZERO_DEADZONE_HIGHLIGHT_TIP"),
                    ))
                })
                .child(mapping_highlight(
                    text,
                    &self.thumbstick_buttons(side),
                    false,
                )),
        );
        let mut bottom = v_flex().items_center();
        if highlight {
            bottom = bottom.child(
                source_action(
                    if left {
                        "gamepad-left-stick-recalibrate"
                    } else {
                        "gamepad-right-stick-recalibrate"
                    },
                    "RECALIBRATE",
                )
                .mb(surface::css(20.))
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(GamepadNavigationRequested {
                        page_key: "TAB_CALIBRATION",
                    })
                })),
            );
        } else if !sensitivity
            && ["leftStick", "rightStick"]
                .iter()
                .any(|side| self.number(&format!("/controller/{side}/deadzoneValue")) < 7)
        {
            bottom = bottom.child(div().h(surface::css(47.)));
        }
        bottom = bottom.child(self.thumbstick_simulator(side, 162., true, enabled));
        panel.child(bottom).into_any_element()
    }

    fn thumbstick_bottom_widgets(&self, cx: &Context<Self>) -> AnyElement {
        let active = self.checked("/controller/leftStick/preventDoubleDeadzone")
            && self.checked("/controller/rightStick/preventDoubleDeadzone");
        surface::page_columns()
            .child(surface::page_column(
                source_panel(
                    "CIRCULARITY_MODE_HEADER",
                    "gamepad-circularity-help",
                    "CIRCULARITY_MODE_TOOLTIPS",
                    cx,
                )
                .child(self.circularity_widget(cx)),
            ))
            .child(surface::page_column(
                surface::panel_with_title_switch(
                    t("PREVENT_DOUBLE_DEADZONES"),
                    surface::SynapseSwitch::new("gamepad-prevent-double-deadzone")
                        .accessibility_label(t("PREVENT_DOUBLE_DEADZONES"))
                        .checked(active)
                        .on_change(cx.listener(|this, _, _, cx| this.toggle_double_deadzone(cx))),
                    div(),
                    cx,
                )
                .child(surface::note(t("PREVENT_DOUBLE_DEADZONES_DESCRIPTION"), cx)),
            ))
            .into_any_element()
    }

    fn toggle_double_deadzone(&mut self, cx: &mut Context<Self>) {
        let modern = self.modern_thumbsticks();
        let active = self.checked("/controller/leftStick/preventDoubleDeadzone")
            && self.checked("/controller/rightStick/preventDoubleDeadzone");
        let states = ["leftStick", "rightStick"].map(|side| {
            (
                side,
                if modern {
                    !active
                } else {
                    !self.checked(&format!("/controller/{side}/preventDoubleDeadzone"))
                },
            )
        });
        for (side, next) in states {
            self.write(
                &format!("/controller/{side}/preventDoubleDeadzone"),
                json!(next),
                cx,
            );
        }
    }

    fn circularity_widget(&self, cx: &Context<Self>) -> AnyElement {
        let standard = self.number("/controller/circularity/mode") == self.spec.standard_mode;
        v_flex()
            .items_center()
            .gap(surface::css(20.))
            .child(segment(
                "gamepad-circularity",
                [("STANDARD", standard), ("CIRCULAR", !standard)],
                cx,
                |this, _, _, cx| {
                    let next =
                        if this.number("/controller/circularity/mode") == this.spec.standard_mode {
                            this.spec.circular_mode
                        } else {
                            this.spec.standard_mode
                        };
                    this.write("/controller/circularity/mode", json!(next), cx);
                },
            ))
            .child(
                div()
                    .w_full()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(t(if standard {
                        "CIRCULARITY_MODE_STANDARD_DESC"
                    } else {
                        "CIRCULARITY_MODE_CIRCULAR_DESC"
                    })),
            )
            .into_any_element()
    }

    fn thumbstick_widgets(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut device = div()
            .relative()
            .w(surface::css(500.))
            .h(surface::css(251.))
            .flex()
            .items_start()
            .justify_center();
        if let Some(image) = &self.thumbstick_ui.artwork {
            device = device.child(
                img(image.clone())
                    .w(surface::css(372.))
                    .h(surface::css(251.)),
            );
        } else {
            device = device.child(self.thumbstick_product_image(window));
            for (x, y) in [(140., 79.), (298., 137.)] {
                device = device.child(
                    div()
                        .absolute()
                        .left(surface::css(x))
                        .top(surface::css(y))
                        .size(surface::css(14.))
                        .rounded_full()
                        .bg(Colors::highlight())
                        .border_1()
                        .border_color(Colors::highlight_border()),
                );
            }
        }
        let stage = h_flex()
            .w(surface::css(780.))
            .h(surface::css(251.))
            .mx_auto()
            .justify_between()
            .items_start()
            .child(div().mt(surface::css(39.)).child(self.thumbstick_simulator(
                "leftStick",
                140.,
                false,
                true,
            )))
            .child(device)
            .child(div().mt(surface::css(39.)).child(self.thumbstick_simulator(
                "rightStick",
                140.,
                false,
                true,
            )));
        let mut instructions = v_flex()
            .w(surface::css(780.))
            .mx_auto()
            .mt(surface::css(20.))
            .mb(surface::css(17.))
            .items_center()
            .child(
                h_flex()
                    .gap(surface::css(4.))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(t("THUMBSTICKS_TEST_DEADZONE_LABEL")),
                    )
                    .child(t("THUMBSTICKS_TEST_DEADZONE_BODY")),
            );
        let mut mapped = v_flex().mt(surface::css(17.));
        for side in ["leftStick", "rightStick"] {
            if self.clutch_assigned(side) {
                let buttons = self.thumbstick_buttons(side);
                let text = t(if side == "leftStick" {
                    "THUMBSTICKS_TEST_SENSITIVITY_CLUTCH_LEFT_BODY"
                } else {
                    "THUMBSTICKS_TEST_SENSITIVITY_CLUTCH_RIGHT_BODY"
                });
                let spacer = side == "rightStick" && self.clutch_assigned("leftStick");
                mapped = mapped.child(
                    h_flex()
                        .gap(surface::css(4.))
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .when(spacer, |label| label.invisible())
                                .child(t("THUMBSTICKS_TEST_SENSITIVITY_CLUTCH_LABEL")),
                        )
                        .child(mapping_highlight(
                            text.replace("{{buttonName}}", &buttons),
                            &buttons,
                            true,
                        )),
                );
            }
        }
        if self.clutch_assigned("leftStick") || self.clutch_assigned("rightStick") {
            instructions = instructions.child(mapped);
        }
        let active = self.checked("/controller/leftStick/preventDoubleDeadzone")
            && self.checked("/controller/rightStick/preventDoubleDeadzone");
        v_flex()
            .mt(surface::css(30.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::stage_text())
            .child(stage)
            .child(instructions)
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        v_flex().child(self.deadzone_widget(window, cx)).child(
                            surface::panel_with_title_switch(
                                t("PREVENT_DOUBLE_DEADZONES"),
                                surface::SynapseSwitch::new("gamepad-prevent-double-deadzone")
                                    .accessibility_label(t("PREVENT_DOUBLE_DEADZONES"))
                                    .checked(active)
                                    .on_change(cx.listener(|this, _, _, cx| {
                                        this.toggle_double_deadzone(cx)
                                    })),
                                div(),
                                cx,
                            )
                            .child(surface::note(t("PREVENT_DOUBLE_DEADZONES_DESCRIPTION"), cx)),
                        ),
                    ))
                    .child(surface::page_column(
                        v_flex().child(self.sensitivity_widget(window, cx)).child(
                            source_panel(
                                "CIRCULARITY_MODE_HEADER",
                                "gamepad-circularity-help",
                                "CIRCULARITY_MODE_TOOLTIPS",
                                cx,
                            )
                            .child(self.circularity_widget(cx)),
                        ),
                    )),
            )
            .into_any_element()
    }

    fn deadzone_widget(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut panel = source_panel(
            "DEADZONE",
            "gamepad-deadzone-help",
            "LOW_DEADZONE_INFO_DETAIL",
            cx,
        )
        .child(div().line_height(surface::css(17.)).child(format!(
            "{}\n\n{}",
            t("DEADZONE_DESCRIPTION_1"),
            t("DEADZONE_DESCRIPTION_2")
        )));
        for side in ["leftStick", "rightStick"] {
            let value = self.number(&format!("/controller/{side}/deadzoneValue"));
            let part = if side == "leftStick" { 1 } else { 2 };
            let mut row = v_flex()
                .mt(surface::css(20.))
                .child(self.widget_slider_row(side, false, true, window, cx));
            if value < 7 {
                row = row.child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap(surface::css(10.))
                        .mt(surface::css(20.))
                        .p(surface::css(20.))
                        .bg(Colors::panel())
                        .border_1()
                        .border_color(Colors::warning())
                        .rounded(surface::css(3.))
                        .child(
                            h_flex()
                                .items_center()
                                .gap(surface::css(10.))
                                .flex_1()
                                .when(value == 0, |d| {
                                    d.child(surface::help_control(
                                        if part == 1 {
                                            "gamepad-left-zero-help"
                                        } else {
                                            "gamepad-right-zero-help"
                                        },
                                        t("ZERO_DEADZONE_HIGHLIGHT_TIP"),
                                    ))
                                })
                                .child(t(if value == 0 {
                                    "ZERO_DEADZONE_HIGHLIGHT_DESC"
                                } else {
                                    "LOW_DEADZONE_HIGHLIGHT_DESC"
                                })),
                        )
                        .child(
                            source_action(
                                if part == 1 {
                                    "gamepad-left-stick-calibration"
                                } else {
                                    "gamepad-right-stick-calibration"
                                },
                                "RECALIBRATE",
                            )
                            .w(surface::css(94.))
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.open_calibration_popup(part, window, cx)
                                },
                            )),
                        ),
                );
            }
            panel = panel.child(row);
        }
        panel.into_any_element()
    }

    fn sensitivity_widget(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let neither = !self.clutch_assigned("leftStick") && !self.clutch_assigned("rightStick");
        let mut rows = v_flex().relative();
        for side in ["leftStick", "rightStick"] {
            rows = rows.child(
                div()
                    .when(side == "rightStick", |d| d.mt(surface::css(20.)))
                    .child(if neither || self.clutch_assigned(side) {
                        self.widget_slider_row(side, true, !neither, window, cx)
                    } else {
                        h_flex()
                            .gap(surface::css(20.))
                            .min_h(surface::css(61.))
                            .child(widget_side_label(side))
                            .child(self.assignment_message(cx))
                            .into_any_element()
                    }),
            );
        }
        if neither {
            rows = rows.child(
                div()
                    .absolute()
                    .left(surface::css(160.))
                    .top(relative(0.5))
                    .mt(surface::css(-29.5))
                    .occlude()
                    .child(self.assignment_message(cx)),
            );
        }
        source_panel(
            "SENSITIVITY_CLUTCH",
            "gamepad-sensitivity-help",
            "THUMBSTICKS_SENSITIVITY_DESCRIPTION",
            cx,
        )
        .child(rows)
        .into_any_element()
    }

    fn assignment_message(&self, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .items_center()
            .justify_between()
            .gap(surface::css(8.))
            .w(surface::css(376.))
            .min_h(surface::css(59.))
            .p(surface::css(16.))
            .bg(Colors::black())
            .border_1()
            .border_color(Colors::bubble_border())
            .rounded(surface::css(4.))
            .text_color(Colors::white())
            .child(
                div()
                    .flex_1()
                    .font_family("Segoe UI")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(20.))
                    .child(t("THUMBSTICKS_SENSITIVITY_ASSIGN_INFO")),
            )
            .child(
                source_action("gamepad-stick-mapping-assign", "ASSIGN").on_click(cx.listener(
                    |_, _, _, cx| {
                        cx.emit(GamepadNavigationRequested {
                            page_key: "TAB_CUSTOMIZE",
                        })
                    },
                )),
            )
            .into_any_element()
    }

    fn widget_slider_row(
        &self,
        side: &'static str,
        sensitivity: bool,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .items_start()
            .gap(surface::css(20.))
            .w(surface::css(520.))
            .min_h(surface::css(61.))
            .child(widget_side_label(side))
            .child(self.progress_control(side, sensitivity, enabled, window, cx))
            .into_any_element()
    }

    fn thumbstick_simulator(
        &self,
        side: &str,
        extent: f32,
        clamp: bool,
        enabled: bool,
    ) -> AnyElement {
        let left = side == "leftStick";
        let sample = self
            .controller_input_sample
            .as_ref()
            .map(|sample| sample.thumbsticks())
            .map(|values| {
                if left {
                    (values[0], values[1])
                } else {
                    (values[2], values[3])
                }
            });
        let sample = sample.filter(|_| enabled);
        let position = sample.map(|(x, y)| {
            let span = if extent == 162. {
                153.
            } else {
                f64::from(extent) - 12.
            };
            let (mut x, mut y) = (x * span / 2000., y * span / 2000.);
            let radius = f64::from(extent) / 2. - 3.;
            if clamp && x.hypot(y) > radius {
                let scale = radius / x.hypot(y);
                x *= scale;
                y *= scale;
            }
            (x, y)
        });
        let values = sample.map_or_else(
            || ("X: —".to_owned(), "Y: —".to_owned()),
            |(x, y)| {
                let value = |value: f64| {
                    if value == 0. {
                        "0".to_owned()
                    } else {
                        format!("{value:.1}")
                    }
                };
                (
                    format!("X: {}%", value(x / 10.)),
                    format!("Y: {}%", value(-y / 10.)),
                )
            },
        );
        v_flex()
            .w(surface::css(extent))
            .when(!enabled, |d| d.opacity(0.3))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let scale = bounds.size.width / extent;
                        let middle =
                            bounds.origin + point(bounds.size.width / 2., bounds.size.height / 2.);
                        let radius = extent / 2. - 3.;
                        let circle = |center: Point<Pixels>, radius: f32, stroke: bool| {
                            let mut path = if stroke {
                                PathBuilder::stroke(scale * 2.)
                            } else {
                                PathBuilder::fill()
                            };
                            for step in 0..=64 {
                                let angle = step as f32 * std::f32::consts::TAU / 64.;
                                let p = center
                                    + point(
                                        scale * radius * angle.cos(),
                                        scale * radius * angle.sin(),
                                    );
                                if step == 0 {
                                    path.move_to(p);
                                } else {
                                    path.line_to(p);
                                }
                            }
                            path.close();
                            path.build().ok()
                        };
                        if let Some(path) = circle(middle, radius, true) {
                            window.paint_path(path, Colors::circle());
                        }
                        if let Some(path) = circle(middle, 5., false) {
                            window.paint_path(path, Colors::accent());
                        }
                        if let Some((x, y)) = position {
                            let current = middle + point(scale * x as f32, scale * y as f32);
                            let mut line = PathBuilder::stroke(scale * 2.);
                            line.move_to(current);
                            line.line_to(middle);
                            if let Ok(path) = line.build() {
                                window.paint_path(path, Colors::accent());
                            }
                            if let Some(path) = circle(current, 5., false) {
                                window.paint_path(path, Colors::accent());
                            }
                        }
                    },
                )
                .w(surface::css(extent))
                .h(surface::css(extent)),
            )
            .child(
                h_flex()
                    .w_full()
                    .mt(surface::css(if extent == 140. { 15. } else { 20. }))
                    .when(extent == 140., |d| d.justify_between())
                    .when(extent != 140., |d| {
                        d.justify_center().gap(surface::css(20.))
                    })
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(Colors::stage_text())
                    .child(values.0)
                    .child(values.1),
            )
            .into_any_element()
    }
}

fn widget_side_label(side: &str) -> Div {
    div()
        .w(surface::css(140.))
        .flex_shrink_0()
        .pt(surface::css(26.))
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .text_color(Colors::white())
        .child(
            t(if side == "leftStick" {
                "CALIBBRTION_LEFT_THUMBSITCK"
            } else {
                "CALIBBRTION_RIGHT_THUMBSITCK"
            })
            .to_uppercase(),
        )
}

fn source_action(id: &'static str, key: &'static str) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(t(key))
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .px(surface::css(16.))
        .pt(surface::css(7.))
        .pb(surface::css(6.))
        .rounded(surface::css(3.))
        .bg(Colors::accent())
        .border_1()
        .border_color(Colors::button_border())
        .text_color(Colors::selected_text())
        .font_family("Roboto")
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .hover(|s| s.opacity(0.8))
        .active(|s| s.opacity(0.6))
        .child(t(key).to_uppercase())
}

fn source_panel(key: &str, id: &'static str, help: &str, cx: &App) -> Div {
    surface::panel_with_help(t(key), surface::help_control(id, t(help)), cx)
}

fn mapping_highlight(text: String, buttons: &str, bold: bool) -> StyledText {
    let range = if buttons.is_empty() {
        None
    } else {
        text.find(buttons).map(|start| start..start + buttons.len())
    };
    StyledText::new(text).with_highlights(range.into_iter().map(|range| {
        (
            range,
            HighlightStyle {
                color: Some(Colors::accent()),
                font_weight: bold.then_some(FontWeight::BOLD),
                ..Default::default()
            },
        )
    }))
}

fn segment<F>(
    id: &'static str,
    choices: [(&'static str, bool); 2],
    cx: &Context<GamepadProductWorkspace>,
    listener: F,
) -> AnyElement
where
    F: Fn(
            &mut GamepadProductWorkspace,
            &ClickEvent,
            &mut Window,
            &mut Context<GamepadProductWorkspace>,
        ) + Copy
        + 'static,
{
    h_flex()
        .id(id)
        .h(surface::css(36.))
        .p(surface::css(5.))
        .bg(Colors::panel())
        .border_1()
        .border_color(Colors::border())
        .rounded(surface::css(18.))
        .on_click(cx.listener(listener))
        .children(choices.map(|(key, selected)| {
            BaseButton::new(SharedString::from(format!("{id}-{key}")))
                .accessibility_label(t(key))
                .selected(selected)
                .h(surface::css(24.))
                .px(surface::css(10.))
                .pt(surface::css(6.))
                .pb(surface::css(5.))
                .rounded(surface::css(12.))
                .text_size(surface::css(14.))
                .line_height(surface::css(14.))
                .when(selected, |button| {
                    button
                        .bg(Colors::accent())
                        .text_color(Colors::selected_text())
                })
                .when(!selected, |button| {
                    button
                        .text_color(Colors::text())
                        .hover(|s| s.text_color(Colors::accent()))
                        .active(|s| s.bg(Colors::hover()))
                })
                .child(t(key).to_uppercase())
                .on_click(cx.listener(move |this, event, window, cx| {
                    listener(this, event, window, cx);
                    cx.stop_propagation();
                }))
        }))
        .into_any_element()
}

fn tutorial_asset(pid: u32) -> &'static str {
    match pid {
        2629 => "synapse/gamepad-thumbsticks/2629/tutorial.webp",
        2636 => "synapse/gamepad-thumbsticks/2636/tutorial.webp",
        2647 => "synapse/gamepad-thumbsticks/2647/tutorial.webp",
        2650 => "synapse/gamepad-thumbsticks/2650/tutorial.webp",
        4133 => "synapse/gamepad-thumbsticks/4133/tutorial.webp",
        4144 => "synapse/gamepad-thumbsticks/4144/tutorial.webp",
        _ => unreachable!("current legacy thumbstick products"),
    }
}

#[cfg(test)]
mod tests {
    use super::{circle_radius, nearest_step, next_step};
    use std::prelude::v1::test;
    #[test]
    fn source_numeric_steps_keep_lower_tie_and_clamp_outside_track() {
        let steps = [0, 3, 5, 7, 9, 11, 13, 15];
        assert_eq!(nearest_step(1.5, &steps), 0);
        assert_eq!(nearest_step(1.6, &steps), 3);
        assert_eq!(nearest_step(-10., &steps), 0);
        assert_eq!(nearest_step(50., &steps), 15);
        assert_eq!(next_step(3, &steps, false), 0);
        assert_eq!(next_step(15, &steps, true), 15);
    }
    #[test]
    fn source_sensitivity_svg_changes_only_the_named_circle_radii() {
        let mut svg = r#"<svg><circle class="left-thumbstick" cx="2" r="9"/><circle class="right-thumbstick" r="9"/></svg>"#.to_owned();
        circle_radius(&mut svg, "left-thumbstick", 2).unwrap();
        circle_radius(&mut svg, "right-thumbstick", 17).unwrap();
        assert_eq!(
            svg,
            r#"<svg><circle class="left-thumbstick" cx="2" r="2"/><circle class="right-thumbstick" r="17"/></svg>"#
        );
        assert!(circle_radius(&mut svg, "absent", 7).is_err());
    }
}
