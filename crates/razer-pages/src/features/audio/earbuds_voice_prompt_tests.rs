use super::{
    AutoPauseObservation, DndObservation, Operation, Report, failed_dnd, finish_gesture_restores,
    finish_initial_restores, finish_pending_firmware, mount, reading, voice_restored,
};
use gpui_kit::{AppContext, TestAppContext, test::TestWindowExt};
use razer_device::earbuds_protocol40::ToggleObservation;
use serde_json::json;

#[gpui_kit::test]
fn voice_user_toggle_requires_save_and_keeps_device_observation_when_set_fails(
    cx: &mut TestAppContext,
) {
    let (window, owner, _) = mount(2660, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let read = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&read, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, 2660, cx);
        });
        window.render_frame(cx);
        window.click("earbuds-voice-prompt", cx);
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            assert_eq!(
                request.operation(),
                Operation::VoicePrompt { enabled: false }
            );
            assert!(request.needs_persistence());
            assert!(!request.is_profile_restore());
            assert_eq!(view.earbuds.voice_persisted, None);
            assert_eq!(view.earbuds.voice_device, ToggleObservation::Observed(true));
            assert!(!view.earbuds.voice_draft);
            assert!(view.mark_earbuds_persisted(&request));
            view.finish_earbuds_request(&request, Err("SET failed".into()), cx);
            assert_eq!(view.earbuds.voice_persisted, Some(false));
            assert_eq!(view.earbuds.voice_device, ToggleObservation::Observed(true));
            assert!(!view.earbuds.voice_draft);
            assert_eq!(view.earbuds.voice_error.as_deref(), Some("SET failed"));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn ordinary_restores_do_not_persist_missing_fields_or_rollback_target_drafts(
    cx: &mut TestAppContext,
) {
    for failed_read in [false, true] {
        let (window, owner, mut device) = mount(2660, cx);
        device.active_profile = "old-profile".into();
        device.profiles[0].id = "old-profile".into();
        device.profiles[0].source_settings =
            Some(json!({"profile":{"notDisturb":{"isEnabled":true}}}));
        cx.update_window(window,|_,_,cx| {
            owner.update(cx,|view,cx|{
                let old=view.earbuds.pending.clone().unwrap();
                view.set_earbuds_context(&device,true,cx);
                view.finish_earbuds_request(&old,Err("scope replaced".into()),cx);
                let read=view.earbuds.pending.clone().unwrap();
                let mut observed=reading(2660,false);observed.voice_prompt=Some(ToggleObservation::Observed(false));
                view.finish_earbuds_request(&read,Ok(observed),cx);
                finish_pending_firmware(view,cx);
                let dnd=view.earbuds.pending.clone().unwrap();
                assert_eq!(dnd.operation(),Operation::DoNotDisturb{enabled:true});
                assert!(!dnd.needs_persistence());assert!(!view.mark_earbuds_persisted(&dnd));
                view.finish_earbuds_request(&dnd,Ok(failed_dnd(2660,true,false)),cx);
                assert!(view.earbuds.dnd_draft);assert_eq!(view.earbuds.dnd_device,DndObservation::Observed(false));
                finish_gesture_restores(view, 2660, cx);
                let voice=view.earbuds.pending.clone().unwrap();
                assert_eq!(voice.operation(),Operation::RestoreVoicePrompt{enabled:None});
                assert!(!voice.needs_persistence());assert!(!view.mark_earbuds_persisted(&voice));
                let mut reply=voice_restored(2660,None,false,true);
                if failed_read {reply.voice_prompt=Some(ToggleObservation::ReadFailed("GET failed".into()));}
                view.finish_earbuds_request(&voice,Ok(reply),cx);
                assert!(view.earbuds.voice_draft);assert_eq!(view.earbuds.voice_persisted,None);
                let auto=view.earbuds.pending.clone().unwrap();
                assert_eq!(auto.operation(),Operation::Restore{enabled:None});
                assert!(!auto.needs_persistence());assert!(!view.mark_earbuds_persisted(&auto));
                let mut reply:Report=serde_json::from_value(json!({"product_id":2660,"operation":{"Restore":{"enabled":null}},
                    "connected":null,"left_battery":null,"right_battery":null,"observation":{"Observed":false},"write":{"Failed":"SET failed"}})).unwrap();
                if failed_read {reply.observation=AutoPauseObservation::ReadFailed("GET failed".into());}
                view.finish_earbuds_request(&auto,Ok(reply),cx);
                let controls=view.earbuds.controls.as_ref().unwrap();
                assert!(controls.draft.auto_pause);assert_eq!(controls.auto_pause_persisted,None);
                let snapshot=view.snapshot();
                assert!(snapshot.pointer("/profile/audioVoicePromptControl").is_none());
                assert!(snapshot.pointer("/profile/audioAutoPauseControl").is_none());
                assert_eq!(snapshot.pointer("/profile/notDisturb/isEnabled"),Some(&json!(true)));
            });
        }).unwrap();
    }
}
