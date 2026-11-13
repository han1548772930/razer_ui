use super::admit_earbuds_request;
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size, test::TestWindowExt};
use razer_device::earbuds_auto_pause::{Operation, Report};
use razer_pages::features::{AudioProductWorkspace, EarbudsAutoPauseRequested};
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};

#[gpui_kit::test]
fn earbuds_cancelled_admission_finishes_old_request_and_releases_next_connection(
    cx: &mut TestAppContext,
) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        cx.update(gpui_kit::init);
        let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
        device.device_container_id = format!("sound-admission-{pid}");
        device.active_profile = "absent".into();
        device.profiles.clear();
        let radio: Report = serde_json::from_value(serde_json::json!({
            "product_id": pid, "operation": "Read", "connected": true,
            "left_battery": 71, "right_battery": 82,
            "observation": "Unobserved", "write": "Skipped"
        }))
        .unwrap();
        let mut page = None;
        let handle = cx.open_window(size(px(1600.), px(1400.)), |window, cx| {
            let view = cx.new(|cx| {
                let mut view = AudioProductWorkspace::new(pid, window, cx);
                view.set_page("TAB_SOUND", window, cx);
                view.set_earbuds_context(&device, true, cx);
                view.set_earbuds_power_context(&device, true, 1, cx);
                view.set_earbuds_sound_context(&device, true, 1, cx);
                view.observe_earbuds_shared_connection(&radio, cx);
                view
            });
            page = Some(view.clone());
            Root::new(view, window, cx)
        });
        let page = page.unwrap();
        let requests = Rc::new(RefCell::new(Vec::new()));
        let received = requests.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&page, move |_, request: &EarbudsAutoPauseRequested, _| {
                if request.is_device_action() {
                    received.borrow_mut().push(request.clone());
                }
            })
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            window.click("earbuds-input-confirm", cx);
        })
        .unwrap();
        let old = requests.borrow()[0].clone();
        cx.update_window(handle.into(), |_, window, cx| {
            page.update(cx, |page, cx| {
                page.set_earbuds_power_context(&device, true, 2, cx);
                page.set_earbuds_sound_context(&device, true, 2, cx);
                page.observe_earbuds_shared_connection(&radio, cx);
            });
            window.render_frame(cx);
            window.click("earbuds-input-open", cx);
            window.click("earbuds-input-confirm", cx);
        })
        .unwrap();
        assert_eq!(requests.borrow().len(), 1);
        assert!(!old.guard().load(Ordering::Acquire));
        cx.update(|cx| assert!(!admit_earbuds_request(&page, &old, cx)));
        assert_eq!(requests.borrow().len(), 2);
        let next = requests.borrow()[1].clone();
        assert_eq!(next.operation(), Operation::SwitchInputSource);
        assert!(!next.needs_persistence() && !next.initializes_profile());
        cx.update(|cx| {
            assert!(admit_earbuds_request(&page, &next, cx));
            assert!(!admit_earbuds_request(&page, &old, cx));
            assert!(page.read(cx).earbuds_request_current(&next));
            next.guard().store(false, Ordering::Release);
            assert!(!admit_earbuds_request(&page, &next, cx));
        });
        assert_eq!(requests.borrow().len(), 2);
    }
}
