use super::*;
use razer_model::{macro_library::MacroLibraryFile, shortcuts::Compiler};

static CALLBACK_TESTS: Mutex<()> = Mutex::new(());
struct Binding;
impl Drop for Binding {
    fn drop(&mut self) {
        *EVENTS.lock().unwrap() = None;
        OVERFLOW.store(false, Ordering::Release);
    }
}

#[test]
fn dependency_documents_are_acknowledged_before_referencing_shortcuts() {
    let document = Compiler::new()
        .compile(&[], &MacroLibraryFile::default())
        .unwrap();
    let mut writes = Vec::new();
    publication_steps(&document, |key, value| {
        writes.push((key.to_owned(), value.clone()));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        writes
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        [
            "synapseDefaultTurbos",
            "synapseTurbos",
            "synapseMacros",
            "synapsePhasedMacros",
            "synapseGlobalShortcuts",
        ]
    );
    assert_eq!(writes[0].1.as_array().unwrap().len(), 38);
    assert_eq!(writes[1].1, json!([]));
    assert_eq!(writes[2].1, json!([]));
    assert_eq!(writes[3].1, json!([]));
    assert_eq!(
        writes[4].1,
        json!({"appEngine":{
            "hash":"de2ccb2014607a84df08306567cd96f0", "mappings":[],
        }})
    );
}

#[test]
fn failed_dependency_never_publishes_the_referencing_mapping_document() {
    let document = Compiler::new()
        .compile(&[], &MacroLibraryFile::default())
        .unwrap();
    for failed in [
        "synapseDefaultTurbos",
        "synapseTurbos",
        "synapseMacros",
        "synapsePhasedMacros",
    ] {
        let mut writes = Vec::new();
        let error = publication_steps(&document, |key, _| {
            writes.push(key.to_owned());
            if key == failed {
                anyhow::bail!("simulated {key} rejection");
            }
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains(failed));
        assert_eq!(writes.last().unwrap(), failed);
        assert!(!writes.iter().any(|key| key == "synapseGlobalShortcuts"));
    }
}

#[test]
fn cleanup_uses_the_original_empty_mapping_document_and_hash() {
    assert_eq!(
        empty_mappings().unwrap(),
        json!({"appEngine":{
            "hash":"de2ccb2014607a84df08306567cd96f0", "mappings":[],
        }})
    );
}

#[test]
fn rust_callback_copies_only_type_three_and_reports_queue_overflow() {
    let _tests = CALLBACK_TESTS.lock().unwrap();
    let state = State::new();
    state.activate_events().unwrap();
    let _binding = Binding;
    // SAFETY: this invokes only our Rust callback, with live NUL-terminated
    // input buffers. No vendor library is loaded or called by this test.
    unsafe { unsupported(2, std::ptr::null(), std::ptr::null(), std::ptr::null(), 1) };
    assert!(state.events.try_recv().is_err());
    assert!(!OVERFLOW.load(Ordering::Acquire));
    {
        let device = CString::new("{\"productId\":70}").unwrap();
        let input = CString::new("[1,2]").unwrap();
        let output = CString::new("{\"type\":\"disabled\"}").unwrap();
        // SAFETY: all pointers borrow the live CStrings only for this call.
        unsafe {
            unsupported(
                3,
                device.as_ptr(),
                input.as_ptr(),
                output.as_ptr(),
                u64::MAX,
            )
        };
    }
    let event = state.events.try_recv().unwrap();
    assert_eq!(event.device_info(), "{\"productId\":70}");
    assert_eq!(event.input(), "[1,2]");
    assert_eq!(event.output(), "{\"type\":\"disabled\"}");
    assert_eq!(event.time_tick(), u64::MAX);
    assert!(!OVERFLOW.load(Ordering::Acquire));
    for tick in 0..=512 {
        // SAFETY: null vendor string arguments are explicitly handled by
        // copy_string and carry no borrowed context into the event queue.
        unsafe {
            unsupported(
                3,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                tick,
            )
        };
    }
    assert!(OVERFLOW.load(Ordering::Acquire));
    assert_eq!(state.events.try_iter().count(), 512);
}

#[test]
fn creating_another_unstarted_state_cannot_steal_a_retained_callback_sink() {
    let _tests = CALLBACK_TESTS.lock().unwrap();
    let first = State::new();
    first.activate_events().unwrap();
    let _binding = Binding;
    let second = State::new();
    // SAFETY: invokes only our Rust callback with explicitly handled null
    // strings. A second unstarted state must leave the first sink untouched.
    unsafe { unsupported(3, std::ptr::null(), std::ptr::null(), std::ptr::null(), 19) };
    assert_eq!(first.events.try_recv().unwrap().time_tick(), 19);
    assert!(second.events.try_recv().is_err());
    assert!(second.activate_events().is_err());
}
