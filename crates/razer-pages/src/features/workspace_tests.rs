use super::{Continue, DeviceWorkspace};
use crate::features::{controls::Control, settings::EqKind};
use crate::nav::Tab;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, ScrollDelta, SharedString, TestAppContext, point, px, size};
use razer_model::model::DeviceCategory;

#[gpui_kit::test]
fn source_widget_breakpoint_reflows_real_lighting_without_resetting_controls(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    for scale in [1., 1.25] {
        cx.update(|cx| {
            gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16. * scale))
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(1280. * scale), px(1100. * scale)), |window, cx| {
            let view = cx.new(|cx| {
                DeviceWorkspace::new(razer_model::demo::demo_keyboard(), true, window, cx)
            });
            view.update(cx, |view, cx| view.set_page(Tab::Lighting, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("brightness-enabled").checked(), Some(true));
            window.click("brightness-enabled", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let (settings, effect_entity, profile_entity, brightness_entity) = cx.update(|cx| {
            let state = view.read(cx);
            assert!(!state.settings().lighting.enabled);
            (
                state.settings().clone(),
                state.controls.effect.entity_id(),
                state.controls.profile.entity_id(),
                state.controls.sliders[&Control::Brightness].entity_id(),
            )
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.within("profile-select").click("input", cx);
            assert_eq!(window.find("profile-select").expanded(), Some(true));
        })
        .unwrap();
        cx.run_until_parked();

        // Source main CSS: @media (max-width:1279px) adds 30px column
        // margins; fixed 600px widgets first share a row at 1280px.
        for (width, stacked) in [(1280., false), (1279., true), (1280., false)] {
            cx.simulate_window_resize(handle.into(), size(px(width * scale), px(1100. * scale)));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let brightness = window
                    .within("control-Brightness")
                    .find("slider-bar-container")
                    .bounds();
                let effect = window
                    .within("page-columns")
                    .find("source-dropdown")
                    .bounds();
                let quick = window.find("lighting-quick").bounds();
                assert_eq!(brightness.size.width, px(520. * scale));
                assert_eq!(effect.size, size(px(150. * scale), px(27. * scale)));
                if stacked {
                    assert!((effect.left() - brightness.left()).abs() <= px(1.));
                    assert!(quick.top() > brightness.bottom());
                } else {
                    assert!(effect.left() > brightness.right());
                    assert!(quick.top() < brightness.bottom());
                }
                let sync = window.find("lighting-sync").bounds();
                assert!((sync.left() - effect.right() - px(20. * scale)).abs() <= px(1.));
                assert!((sync.center().y - effect.center().y).abs() <= px(1.));

                let nav = window.find("device-navigation").bounds();
                let profile = window
                    .within("device-navigation")
                    .find("source-dropdown")
                    .bounds();
                assert_eq!(profile.size.height, px(27. * scale));
                assert!((profile.center().y - nav.center().y).abs() <= px(1.));
                assert_eq!(window.find("profile-select").expanded(), Some(true));
                assert_eq!(window.find("brightness-enabled").checked(), Some(false));
            })
            .unwrap();
            cx.update(|cx| {
                let state = view.read(cx);
                assert_eq!(state.settings(), &settings);
                assert_eq!(state.controls.effect.entity_id(), effect_entity);
                assert_eq!(state.controls.profile.entity_id(), profile_entity);
                assert_eq!(
                    state.controls.sliders[&Control::Brightness].entity_id(),
                    brightness_entity
                );
            });
        }
    }
}

#[gpui_kit::test]
fn narrow_short_workspace_keeps_mapping_overlay_fixed_while_product_scrolls(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1080.), px(800.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(
                razer_model::model::measured_devices().remove(0),
                true,
                window,
                cx,
            )
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mouse-input-CycleUpSensitivityStages", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let (body_id, settings, mapping_entity) = cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(
            state.mapping.as_ref().unwrap().input,
            "CycleUpSensitivityStages"
        );
        (
            SharedString::from(format!("device-body-{}", state.identity())),
            state.settings().clone(),
            state.controls.mapping.entity_id(),
        )
    });
    cx.simulate_window_resize(handle.into(), size(px(700.), px(400.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let nav = window.find("device-navigation").bounds();
        let body = window.find(body_id.clone()).bounds();
        let front = window.find("mouse-front-image").bounds();
        assert_eq!(nav.size, size(px(700.), px(48.)));
        assert!(body.top() >= nav.bottom());
        assert!(body.right() <= px(700.));
        assert!(body.bottom() <= px(400.));
        assert_eq!(front.size, size(px(300.), px(340.)));
        let original_close = window.find("mapping-close");
        assert!(original_close.visible());
        assert!(original_close.bounds().top() >= body.top());
        assert!(original_close.bounds().bottom() <= body.bottom());

        // The source's 770px canvas scrolls independently of the mapping
        // overlay and navigation, including in a short viewport.
        window.scroll(
            body_id.clone(),
            ScrollDelta::Pixels(point(px(-10_000.), px(-10_000.))),
            cx,
        );
        let scrolled = window.find("mouse-front-image").bounds();
        let close = window.find("mapping-close");
        assert!(scrolled.left() < front.left());
        assert!(scrolled.top() < front.top());
        assert_eq!(scrolled.size, front.size);
        assert!(close.visible());
        assert_eq!(close.bounds(), original_close.bounds());
        assert!(close.bounds().top() >= body.top());
        assert!(close.bounds().bottom() <= body.bottom());
        assert_eq!(window.find("device-navigation").bounds(), nav);
        assert_eq!(window.find(body_id.clone()).bounds(), body);
        window.scroll(
            body_id.clone(),
            ScrollDelta::Pixels(point(px(-10_000.), px(-10_000.))),
            cx,
        );
        assert_eq!(window.find("mapping-close").bounds(), close.bounds());
        assert_eq!(window.find("mouse-front-image").bounds(), scrolled);

        window.scroll(
            body_id.clone(),
            ScrollDelta::Pixels(point(px(10_000.), px(10_000.))),
            cx,
        );
        assert_eq!(window.find("mouse-front-image").bounds(), front);
        let overlay = window.find("mouse-cycle-overlay").bounds();
        assert_eq!(overlay.size, front.size);
        assert!((overlay.left() - front.left() - px(37.)).abs() <= px(1.));
        assert!((overlay.top() - front.top() + px(77.)).abs() <= px(1.));
        assert_eq!(
            window
                .find("mouse-input-CycleUpSensitivityStages")
                .selected(),
            Some(true)
        );
    })
    .unwrap();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings(), &settings);
        assert_eq!(state.controls.mapping.entity_id(), mapping_entity);
        assert_eq!(
            state.mapping.as_ref().unwrap().input,
            "CycleUpSensitivityStages"
        );
    });
}

#[gpui_kit::test]
fn narrow_mic_workspace_keeps_eq_width_and_reaches_retained_last_band(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1100.), px(800.)), |window, cx| {
        let mut device = razer_model::demo::demo_keyboard();
        device.product_id = 777;
        device.category = DeviceCategory::Headset;
        let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        view.update(cx, |view, cx| view.set_page(Tab::Mic, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mic-preset-conference", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let (body_id, settings, band_entities) = cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings().mic.preset, "conference");
        (
            SharedString::from(format!("device-body-{}", state.identity())),
            state.settings().clone(),
            state
                .settings()
                .mic
                .bands
                .iter()
                .map(|band| {
                    (
                        band.frequency,
                        state.controls.sliders[&Control::Mic(band.frequency)].entity_id(),
                    )
                })
                .collect::<Vec<_>>(),
        )
    });
    cx.simulate_window_resize(handle.into(), size(px(700.), px(800.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let panel = window.find("mic-eq-panel").bounds();
        let body = window.find(body_id.clone()).bounds();
        let nav = window.find("device-navigation").bounds();
        // 777 CSS fixes #eqBox at 940px, including below the widget breakpoint.
        assert_eq!(panel.size.width, px(940.));
        assert_eq!(body.size.width, px(700.));
        assert!(!window.within("mic-band-16000").find("eq-slider").visible());
        window.scroll(
            body_id.clone(),
            ScrollDelta::Pixels(point(px(-10_000.), px(0.))),
            cx,
        );
        let shifted = window.find("mic-eq-panel").bounds();
        let last = window.within("mic-band-16000").find("eq-slider");
        let reset = window.find("mic-eq-reset");
        assert_eq!(shifted.size, panel.size);
        assert_eq!(shifted.top(), panel.top());
        assert!(shifted.left() < panel.left());
        assert!(last.visible());
        assert!(last.bounds().left() >= body.left());
        assert!(last.bounds().right() <= body.right());
        assert_eq!(last.bounds().size.height, px(300.));
        assert!(reset.visible());
        assert!(reset.bounds().right() <= body.right());
        assert!(!window.within("mic-band-31").find("eq-slider").visible());
        assert_eq!(window.find("device-navigation").bounds(), nav);
    })
    .unwrap();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings(), &settings);
        for (frequency, entity) in &band_entities {
            assert_eq!(
                state.controls.sliders[&Control::Mic(*frequency)].entity_id(),
                *entity
            );
        }
    });

    // Exercise hit testing at the formerly clipped edge, then resize back.
    cx.update_window(handle.into(), |_, window, cx| {
        let bar = window
            .within("mic-band-16000")
            .find("slider-bar-container")
            .bounds();
        window.drag(bar.center(), point(bar.center().x, bar.top() + px(16.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let edited = cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings().mic.preset, "custom");
        assert!(state.settings().mic.bands.last().unwrap().decibel > 0);
        assert_eq!(&state.settings().mic.bands[..9], &settings.mic.bands[..9]);
        assert_eq!(state.settings().audio, settings.audio);
        state.settings().clone()
    });
    cx.simulate_window_resize(handle.into(), size(px(1100.), px(800.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("mic-eq-panel").bounds().size.width, px(940.));
        assert!(window.within("mic-band-31").find("eq-slider").visible());
        assert!(window.within("mic-band-16000").find("eq-slider").visible());
    })
    .unwrap();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings(), &edited);
        for (frequency, entity) in &band_entities {
            assert_eq!(
                state.controls.sliders[&Control::Mic(*frequency)].entity_id(),
                *entity
            );
        }
    });
}

#[gpui_kit::test]
fn source_navigation_and_mouse_overlay_keep_geometry_when_scaled(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    for (width, scale) in [(1080., 1.), (1280., 1.), (1350., 1.25)] {
        cx.update(|cx| {
            gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16. * scale))
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(width), px(900.)), |window, cx| {
            let view = cx.new(|cx| {
                DeviceWorkspace::new(
                    razer_model::model::measured_devices().remove(0),
                    true,
                    window,
                    cx,
                )
            });
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let nav = w.find("device-navigation").bounds();
            assert_eq!(nav.size.height, px(48. * scale));
            let first = w.find("device-tab-customize").bounds();
            let last = w.find("device-tab-calibration").bounds();
            assert!(
                ((first.left() + last.right()) / 2. - nav.center().x).abs() <= px(1.),
                "width={width} scale={scale} nav={nav:?} first={first:?} last={last:?}"
            );
            assert_eq!(first.size.height, px(28. * scale));
            let front = w.find("mouse-front-image").bounds();
            assert_eq!(front.size, size(px(300. * scale), px(340. * scale)));
            assert!(w.try_find("mouse-cycle-overlay").is_none());
            w.hover("mouse-input-CycleUpSensitivityStages", cx);
            let overlay = w.find("mouse-cycle-overlay").bounds();
            assert_eq!(w.find("mouse-front-image").bounds(), front);
            assert_eq!(overlay.size, front.size);
            assert!((overlay.left() - front.left() - px(37. * scale)).abs() <= px(1.));
            assert!((overlay.top() - front.top() + px(77. * scale)).abs() <= px(1.));
            w.click("device-tab-performance", cx);
        })
        .unwrap();
        cx.update(|cx| assert_eq!(view.read(cx).page, Tab::Performance));
    }
}

#[gpui_kit::test]
fn source_eq_spacing_reset_position_and_switch_keyboard_activation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1100.)), |window, cx| {
        let mut device = razer_model::demo::demo_keyboard();
        device.product_id = 777;
        device.category = DeviceCategory::Headset;
        let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        view.update(cx, |view, cx| view.set_page(Tab::Sound, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        let first = w.within("audio-band-31").find("eq-slider").bounds();
        let next = w.within("audio-band-63").find("eq-slider").bounds();
        let last = w.within("audio-band-16000").find("eq-slider").bounds();
        let reset = w.find("audio-eq-reset").bounds();
        assert_eq!(next.left() - first.left(), px(47.));
        assert_eq!(first.size.height, px(300.));
        assert_eq!(
            w.within("audio-band-31")
                .find("slider-bar-container")
                .bounds()
                .size
                .height,
            px(300.)
        );
        assert!(reset.left() > last.right());
        assert!((reset.center().y - first.center().y).abs() <= px(1.));
        assert_eq!(
            w.within("playback-enabled")
                .find("switch-track")
                .bounds()
                .size,
            size(px(32.), px(18.))
        );
        // Headless windows start without an OS keyboard-focus target.
        w.focus_next(cx);
        w.render_frame(cx);
        for _ in 0..24 {
            if w.find("playback-enabled").focused() == Some(true) {
                break;
            }
            w.press("tab", cx);
        }
        assert_eq!(w.find("playback-enabled").focused(), Some(true));
        w.press("space", cx);
    })
    .unwrap();
    cx.update(|cx| assert!(!view.read(cx).settings().playback_enabled));
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("device-tab-mic", cx);
        let first = w.within("mic-band-31").find("eq-slider").bounds();
        let next = w.within("mic-band-63").find("eq-slider").bounds();
        assert_eq!(next.left() - first.left(), px(78.));
        assert_eq!(w.find("mic-eq-panel").bounds().size.width, px(940.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn mic_drag_presets_reset_and_profile_switch_are_isolated(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1240.), px(860.)), |window, cx| {
        let mut device = razer_model::demo::demo_keyboard();
        device.product_id = 777;
        device.category = DeviceCategory::Headset;
        let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        view.update(cx, |view, cx| view.set_page(Tab::Mic, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    let audio = cx.update(|cx| view.read(cx).settings().audio.clone());
    let slider_id = cx.update(|cx| view.read(cx).controls.sliders[&Control::Mic(31)].entity_id());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mic-preset-boost", cx);
        let bar = window
            .within("mic-band-31")
            .find("slider-bar-container")
            .bounds();
        window.drag(bar.center(), point(bar.center().x, bar.top() + px(12.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings().mic.preset, "custom");
        assert!(state.settings().mic.bands[0].decibel > 0);
        assert_eq!(state.settings().audio, audio);
        assert_eq!(
            state.controls.sliders[&Control::Mic(31)].entity_id(),
            slider_id
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mic-eq-reset", cx);
        window.click("profile-more", cx);
        window.click("profile-duplicate", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings().mic.preset, "custom");
        assert!(state.settings().mic.bands.iter().all(|b| b.decibel == 0));
        assert_ne!(state.device.active_profile, "demo-profile-kb");
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mic-preset-conference", cx);
        // Select the first profile through the real menu and keyboard path.
        window.within("profile-select").click("input", cx);
        assert_eq!(window.find("profile-select").expanded(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("up", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.device.active_profile, "demo-profile-kb");
        assert_eq!(state.settings().mic.preset, "custom");
        assert_eq!(state.settings().audio, audio);
    });
}
#[gpui_kit::test]
fn calibration_checkbox_and_drag_keep_landing_below_lift(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1240.), px(860.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(
                razer_model::model::measured_devices().remove(0),
                true,
                window,
                cx,
            )
        });
        view.update(cx, |view, cx| view.set_page(Tab::Calibration, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("asymmetric-cutoff", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        let bar = w
            .within("control-Landing")
            .find("slider-bar-container")
            .bounds();
        w.drag(
            bar.center(),
            point(bar.right() - px(1.), bar.center().y),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let tracking = &view.read(cx).settings().tracking;
        assert!(tracking.asymmetric);
        assert!(tracking.landing > 1);
        assert!(tracking.landing < tracking.lift);
    });
}
#[gpui_kit::test]
fn keyboard_hit_target_and_mapping_cancel_preserve_layer(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(960.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(razer_model::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("keyboard-input-KEY_A", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).mapping.as_ref().unwrap().input, "KEY_A"));
    // Editing via Select exercises the mapping subscription and pending continuation.
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("mapping-category-keyboard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(view.read(cx).mapping_dirty()));
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("profile-more", cx);
        w.click("profile-duplicate", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("mapping-keep-editing", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).device.profiles.len(),
            1,
            "cancelling must not create a profile"
        )
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("mapping-hypershift", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("mapping-keep-editing", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert!(!view.read(cx).hypershift);
        assert!(view.read(cx).mapping.is_some());
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("mapping-hypershift", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("mapping-discard", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert!(view.read(cx).hypershift);
        assert!(view.read(cx).mapping.is_none());
        assert!(!view.read(cx).settings().bindings.contains_key("KEY_A"));
    });
}
#[gpui_kit::test]
fn same_pid_different_instances_keep_distinct_identity_and_saved_revisions(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.open_window(size(px(1000.), px(720.)), |window, cx| {
        let original = razer_model::model::measured_devices().remove(0);
        let mut another = original.clone();
        another.device_container_id = "other-container".into();
        let first = cx.new(|cx| DeviceWorkspace::new(original, true, window, cx));
        let second = cx.new(|cx| DeviceWorkspace::new(another, true, window, cx));
        assert_ne!(first.read(cx).identity(), second.read(cx).identity());
        first.update(cx, |state, cx| {
            state.edit(window, cx, |s| s.mic.select(EqKind::Mic, "boost"));
            let captured = state.snapshot();
            state.edit(window, cx, |s| s.mic.select(EqKind::Mic, "conference"));
            state.mark_saved(captured, cx);
            assert!(state.dirty());
            state.discard(window, cx);
            assert_eq!(state.settings().mic.preset, "boost");
            state.apply_continue(Continue::Layer(true), window, cx);
        });
        assert_eq!(second.read(cx).settings().mic.preset, "default");
        Root::new(first, window, cx)
    });
}

#[gpui_kit::test]
fn lighting_parameters_and_disabled_brightness_follow_real_controls(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(860.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(razer_model::demo::demo_keyboard(), true, window, cx));
        view.update(cx, |view, cx| view.set_page(Tab::Lighting, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("lighting-effect").click("input", cx);
        window.press("up", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("effect-direction-0", cx);
        window.click("brightness-enabled", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let lighting = &view.read(cx).settings().lighting;
        assert_eq!(u8::from(lighting.effect), 19);
        assert_eq!(lighting.params().direction, 0);
        assert!(!lighting.enabled);
    });
    let before = cx.update(|cx| view.read(cx).settings().lighting.brightness);
    cx.update_window(handle.into(), |_, window, cx| {
        let bar = window
            .within("control-Brightness")
            .find("slider-bar-container")
            .bounds();
        window.drag(bar.center(), point(bar.left() + px(5.), bar.center().y), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().lighting.brightness, before));
}
