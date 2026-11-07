use super::{Operation, Report, finish_initial_restores, finish_pending_firmware, mount, reading};
use gpui_kit::{AppContext, TestAppContext, test::TestWindowExt};
use razer_device::{
    earbuds_anc::{Observation, State},
    earbuds_events::Notification,
    earbuds_protocol40::{AncLevel, AncMode, AncRuntimeState, AncWrite},
};

fn anc_result(operation: Operation, write: AncWrite) -> Report {
    let Operation::Anc { setting } = operation else {
        panic!("ANC operation")
    };
    let confirmed = write == AncWrite::Acknowledged;
    let mut report = reading(2657, false);
    report.operation = operation;
    report.connected = None;
    report.anc = Some(razer_device::earbuds_anc::Report {
        runtime: if confirmed {
            AncRuntimeState(setting)
        } else {
            AncRuntimeState::default()
        },
        write,
        confirmation: confirmed.then_some(setting),
    });
    report
}

#[gpui_kit::test]
fn anc_write_failure_and_status_only_notification_keep_their_own_state(cx: &mut TestAppContext) {
    let (window, owner, _) = mount(2657, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&request, Ok(reading(2657, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, view.spec.product_id, cx);
            assert_eq!(view.earbuds.anc.device, Observation::Unobserved);
        });
        window.render_frame(cx);
        window.click("earbuds-anc", cx);
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            assert!(request.needs_persistence());
            view.mark_earbuds_persisted(&request);
            view.finish_earbuds_request(
                &request,
                Ok(anc_result(
                    request.operation(),
                    AncWrite::Failed("SET rejected".into()),
                )),
                cx,
            );
            assert_eq!(view.earbuds.anc.device, Observation::Unobserved);
            assert!(
                !view
                    .earbuds
                    .controls
                    .as_ref()
                    .unwrap()
                    .draft
                    .anc
                    .unwrap()
                    .enabled
            );
            let monitor = view.earbuds.monitor.clone().unwrap();
            view.observe_earbuds_notification(
                &monitor,
                Notification::Anc {
                    state: State::Ambient,
                },
                cx,
            );
            assert_eq!(
                view.earbuds.anc.device,
                Observation::Observed {
                    state: State::Ambient,
                    level: None
                }
            );
            let notification = view.earbuds.pending.clone().unwrap();
            assert!(notification.is_hardware_notification());
            assert!(notification.needs_persistence());
            let controls = view.earbuds.controls.as_ref().unwrap();
            assert_eq!(controls.anc_runtime, Some(AncRuntimeState::default()));
            assert_eq!(controls.draft.anc.unwrap().mode, AncMode::Ambient);
            assert_eq!(controls.draft.anc.unwrap().level, AncLevel::Full);
            assert!(!controls.anc_persisted.unwrap().enabled);
            assert!(view.mark_earbuds_persisted(&notification));
            view.finish_earbuds_notification_save(&notification, None, cx);
            assert!(view.earbuds.pending.is_none());
            assert_eq!(
                view.earbuds.controls.as_ref().unwrap().anc_runtime,
                Some(AncRuntimeState::default())
            );
            let draft = view.earbuds.controls.as_ref().unwrap().draft.anc;
            view.observe_earbuds_notification(
                &monitor,
                Notification::Anc {
                    state: State::Unknown(42),
                },
                cx,
            );
            assert_eq!(
                view.earbuds.anc.device,
                Observation::Observed {
                    state: State::Unknown(42),
                    level: None
                }
            );
            assert_eq!(view.earbuds.controls.as_ref().unwrap().draft.anc, draft);
            assert!(view.earbuds.pending.is_none());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn anc_selector_runs_child_and_bubbled_toggle_in_source_order(cx: &mut TestAppContext) {
    let (window, owner, _) = mount(2657, cx);
    cx.update_window(window,|_,window,cx|{
        owner.update(cx,|view,cx|{
            let request=view.earbuds.pending.clone().unwrap();view.finish_earbuds_request(&request,Ok(reading(2657,false)),cx);
            finish_pending_firmware(view,cx);
            finish_initial_restores(view,view.spec.product_id,cx);
        });
        window.render_frame(cx);window.click(("earbuds-anc-mode",10usize),cx);
        owner.update(cx,|view,cx|{
            let first=view.earbuds.pending.clone().unwrap();
            assert!(matches!(first.operation(),Operation::Anc{setting,..} if setting.mode==AncMode::Anc));
            assert_eq!(view.earbuds.controls.as_ref().unwrap().draft.anc.unwrap().mode,AncMode::Ambient);
            view.mark_earbuds_persisted(&first);
            view.finish_earbuds_request(&first,Ok(anc_result(first.operation(),AncWrite::Skipped)),cx);
            assert_eq!(view.earbuds.anc.device,Observation::Unobserved);
            let second=view.earbuds.pending.clone().unwrap();
            assert!(matches!(second.operation(),Operation::Anc{setting,..} if setting.mode==AncMode::Ambient));
            view.mark_earbuds_persisted(&second);
            view.finish_earbuds_request(&second,Ok(anc_result(second.operation(),AncWrite::Acknowledged)),cx);
            assert_eq!(view.earbuds.anc.device,Observation::Observed{state:State::Ambient,level:Some(AncLevel::Full)});
        });
        window.render_frame(cx);
        assert_eq!(window.find("earbuds-reduce-anc").checked(),Some(false));
        window.click("earbuds-reduce-anc",cx);
        assert!(owner.read(cx).earbuds.pending.is_none());
    }).unwrap();
}

#[gpui_kit::test]
fn saved_gesture_tap_and_anc_reach_the_current_workspace_profile(cx: &mut TestAppContext) {
    use crate::features::source_workspace::SourceProductWorkspace;
    use razer_device::{
        earbuds_gesture::TapSpeed,
        earbuds_protocol40::{ANC_PROFILE_DEFAULT, AncSetting},
    };
    use razer_storage::earbuds_profiles as profiles;
    cx.update(|cx| gpui_kit::init(cx));
    let mut device = razer_model::product_preview::registered_preview(2657).unwrap();
    device.profiles.clear();
    device.active_profile.clear();
    profiles::initialize_auto_pause_profile(&mut device, "saved-fields", "Desktop-Default")
        .unwrap();
    let mut owner = None;
    let window = cx.open_window(
        gpui_kit::size(gpui_kit::px(1600.), gpui_kit::px(1000.)),
        |window, cx| {
            let view = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
            owner = Some(view.clone());
            gpui_kit::component::Root::new(view, window, cx)
        },
    );
    let owner = owner.unwrap();
    cx.update_window(window.into(), |_, _, cx| {
        owner.update(cx, |view, cx| {
            let mut saved = view.saved_snapshot();
            profiles::merge_gesture(&mut saved, "saved-fields", false).unwrap();
            profiles::merge_tap_speed(&mut saved, "saved-fields", TapSpeed::Fast).unwrap();
            let anc = AncSetting {
                mode: AncMode::Ambient,
                level: AncLevel::Reduced,
                ..ANC_PROFILE_DEFAULT
            };
            profiles::merge_anc(&mut saved, "saved-fields", anc).unwrap();
            view.mark_saved(saved, cx);
            assert_eq!(
                profiles::read_gesture(view.device(), "saved-fields").unwrap(),
                Some(false)
            );
            assert_eq!(
                profiles::read_tap_speed(view.device(), "saved-fields").unwrap(),
                Some(TapSpeed::Fast)
            );
            assert_eq!(
                profiles::read_anc(view.device(), "saved-fields").unwrap(),
                Some(anc)
            );
        })
    })
    .unwrap();
}

#[gpui_kit::test]
fn focused_reduce_checkbox_keeps_the_source_keyboard_handler_after_ambient_notification(
    cx: &mut TestAppContext,
) {
    let (window, owner, _) = mount(2657, cx);
    cx.update_window(window,|_,window,cx|{
        owner.update(cx,|view,cx|{
            let gate=view.earbuds.pending.clone().unwrap();view.finish_earbuds_request(&gate,Ok(reading(2657,false)),cx);
            finish_pending_firmware(view,cx);finish_initial_restores(view,2657,cx);
        });
        window.render_frame(cx);window.click("earbuds-reduce-anc",cx);
        owner.update(cx,|view,cx|{
            let edit=view.earbuds.pending.clone().unwrap();
            assert!(matches!(edit.operation(),Operation::Anc{setting,..} if setting.level==AncLevel::Reduced));
            view.mark_earbuds_persisted(&edit);
            view.finish_earbuds_request(&edit,Ok(anc_result(edit.operation(),AncWrite::Acknowledged)),cx);
            let monitor=view.earbuds.monitor.clone().unwrap();
            view.observe_earbuds_notification(&monitor,Notification::Anc{state:State::Ambient},cx);
            let notification=view.earbuds.pending.clone().unwrap();view.mark_earbuds_persisted(&notification);
            view.finish_earbuds_notification_save(&notification,None,cx);
        });
        window.render_frame(cx);window.press("enter",cx);
        owner.update(cx,|view,_|{
            let request=view.earbuds.pending.as_ref().unwrap();
            assert!(!request.is_hardware_notification());
            assert!(matches!(request.operation(),Operation::Anc{setting,..} if setting.mode==AncMode::Ambient&&setting.level==AncLevel::Full));
            assert_eq!(view.earbuds.anc.device,Observation::Observed{state:State::Ambient,level:None});
        });
    }).unwrap();
}

#[gpui_kit::test]
fn acknowledged_anc_write_before_profile_switch_cannot_make_new_restore_skip_real_io(
    cx: &mut TestAppContext,
) {
    use razer_device::{
        backend::FeatureTransport,
        earbuds_protocol40::{Protocol40Session, Sleeper},
    };
    use std::cell::{Cell, RefCell};
    struct NoSleep;
    impl Sleeper for NoSleep {
        fn sleep(&self, _: std::time::Duration) {}
    }
    struct Device {
        state: Cell<u8>,
        last: RefCell<Vec<u8>>,
        sent: Cell<usize>,
    }
    impl FeatureTransport for Device {
        fn send_feature(&self, bytes: &[u8]) -> anyhow::Result<()> {
            assert_eq!(&bytes[7..9], &[24, 6]);
            self.state.set(bytes[10]);
            self.sent.set(self.sent.get() + 1);
            self.last.replace(bytes.to_vec());
            Ok(())
        }
        fn get_feature(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
            let request = self.last.borrow();
            bytes.fill(0);
            bytes[0] = 2;
            bytes[1] = 2;
            bytes[2] = request[2];
            bytes[7] = 24;
            bytes[8] = 6;
            Ok(64)
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::Value::Null
        }
    }
    let cache = razer_device::earbuds_anc::Cache::default();
    let hardware = Device {
        state: Cell::new(1),
        last: RefCell::new(vec![]),
        sent: Cell::new(0),
    };
    let (window, owner, mut model) = mount(2657, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let gate = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&gate, Ok(reading(2657, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2657, cx);
        });
        window.render_frame(cx);
        window.click("earbuds-anc", cx);
        owner.update(cx, |view, cx| {
            let old = view.earbuds.pending.clone().unwrap();
            view.mark_earbuds_persisted(&old);
            let confirmed = razer_device::earbuds_auto_pause::execute(
                &mut Protocol40Session::with_sleeper(&hardware, NoSleep),
                2657,
                old.operation(),
                &cache,
                &|| Ok(()),
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(hardware.state.get(), 0);
            let mut next = model.profiles[0].clone();
            next.id = "next-anc".into();
            next.guid = "next-anc".into();
            model.profiles.push(next);
            model.active_profile = "next-anc".into();
            view.set_earbuds_context(&model, true, cx);
            assert!(!view.earbuds_request_current(&old));
            view.finish_earbuds_request(&old, Ok(confirmed), cx);
            let current = view.earbuds.pending.clone().unwrap();
            assert!(current.is_profile_restore());
            assert!(
                view.earbuds
                    .controls
                    .as_ref()
                    .unwrap()
                    .draft
                    .anc
                    .unwrap()
                    .enabled
            );
            let restored = razer_device::earbuds_auto_pause::execute(
                &mut Protocol40Session::with_sleeper(&hardware, NoSleep),
                2657,
                current.operation(),
                &cache,
                &|| Ok(()),
                &|| Ok(()),
            )
            .unwrap();
            view.finish_earbuds_request(&current, Ok(restored), cx);
            assert_eq!(
                hardware.state.get(),
                1,
                "new profile must reach hardware after the previous confirmed SET"
            );
            assert_eq!(hardware.sent.get(), 2);
        });
    })
    .unwrap();
}
