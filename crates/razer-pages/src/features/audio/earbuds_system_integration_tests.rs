use super::{AudioProductWorkspace, State};
use crate::features::source_help_owner::{HelpOwnerCommand, HelpOwnerRequested, SourceHelpOwner};
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size, test::TestWindowExt};
use std::{cell::RefCell, rc::Rc};

#[test]
fn windows_icon_keeps_source_initial_state_and_rejects_unmounted_reads() {
    let mut state = State::default();
    assert!(state.windows_11);
    assert!(!state.observed);
    let old = state.generation;
    state.cancel();
    assert!(!state.observe(old, Some("10")));
    assert!(state.observe(state.generation, Some("10")));
    assert!(!state.windows_11);
    assert!(state.observe(state.generation, Some("11")));
    assert!(state.windows_11);
    assert!(state.observe(state.generation, None));
    assert!(!state.windows_11);
}

#[gpui_kit::test]
fn earbuds_sound_properties_dispatches_the_real_owner_without_saving(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let mut handles = None;
        let window = cx.open_window(size(px(1600.), px(1600.)), |window, cx| {
            let help = cx.new(|cx| SourceHelpOwner::new(pid, cx));
            let view = cx.new(|cx| {
                let mut view = AudioProductWorkspace::new(pid, window, cx);
                view.set_source_owner(help.clone(), cx);
                view.set_page("TAB_SOUND", window, cx);
                view
            });
            handles = Some((view.clone(), help));
            Root::new(view, window, cx)
        });
        let (view, help) = handles.unwrap();
        let requests = Rc::new(RefCell::new(Vec::new()));
        let received = requests.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&help, move |_, event: &HelpOwnerRequested, _| {
                received.borrow_mut().push(event.clone());
            })
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.render_frame(cx);
            let before = view.read(cx).snapshot();
            assert!(window.find("earbuds-sound-properties").visible());
            window.click("earbuds-open-sound-properties", cx);
            window.press("space", cx);
            window.click("earbuds-open-volume-mixer", cx);
            window.press("space", cx);
            assert_eq!(view.read(cx).snapshot(), before);
            view.update(cx, |view, cx| view.set_page("TAB_ENHANCEMENT", window, cx));
            window.render_frame(cx);
            assert!(window.try_find("earbuds-open-sound-properties").is_none());
        })
        .unwrap();
        cx.update(|cx| {
            let requests = requests.borrow();
            assert_eq!(requests.len(), 4);
            assert_eq!(
                requests
                    .iter()
                    .map(HelpOwnerRequested::command)
                    .collect::<Vec<_>>(),
                [
                    HelpOwnerCommand::OpenAudioProperties,
                    HelpOwnerCommand::OpenAudioProperties,
                    HelpOwnerCommand::OpenSoundVolume,
                    HelpOwnerCommand::OpenSoundVolume
                ]
            );
            for request in requests.iter() {
                assert!(help.read(cx).accepts(request));
            }
            help.update(cx, |owner, cx| owner.set_active(false, cx));
            assert!(
                requests
                    .iter()
                    .all(|request| !help.read(cx).accepts(request))
            );
            help.update(cx, |owner, cx| owner.set_active(true, cx));
            assert!(
                requests
                    .iter()
                    .all(|request| !help.read(cx).accepts(request))
            );
            let replacement = cx.new(|cx| SourceHelpOwner::new(pid, cx));
            assert!(
                requests
                    .iter()
                    .all(|request| !replacement.read(cx).accepts(request))
            );
        });
    }
}
