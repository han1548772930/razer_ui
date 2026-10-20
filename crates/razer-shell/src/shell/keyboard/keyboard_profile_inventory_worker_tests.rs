use super::*;
use razer_device::{
    backend::HidNode,
    keyboard_onboard_profiles::{ImportDetails, KeyIdentity, Snapshot},
    keyboard_profile_inventory::{ProfileMetadataObservation, ProfileSlot},
    onboard_macro::{MacroCatalog, MacroInventory, MacroStorageInfo},
};
use std::sync::Mutex;

fn target(pid: u32) -> Target {
    let cap = razer_device::keyboard_metadata::hid_capability(pid, pid).unwrap();
    Target::Collection {
        node: HidNode {
            path: b"physical-keyboard".to_vec(),
            vendor_id: cap.vendor_id,
            product_id: pid as u16,
            interface_number: i32::from(cap.claim_interface_for(pid).unwrap()),
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
        product_id: pid,
        peer_product_id: None,
    }
}

fn complete(options: &ReadOptions) -> CompleteReading {
    let mut keys =
        razer_device::keyboard_onboard_inventory::current_679_inventory(options.layout_id)
            .iter()
            .map(|key| KeyIdentity {
                input_id: key.input_id.to_owned(),
                firmware_key_id: key.firmware_key_id,
                ignore_normal: key.ignore_normal,
                ignore_hypershift: key.ignore_hypershift,
            })
            .collect::<Vec<_>>();
    keys.sort_unstable_by_key(|key| key.firmware_key_id);
    CompleteReading {
        snapshot: Snapshot {
            product_id: 679,
            layout_id: options.layout_id,
            key_inventory: keys,
            macros: MacroCatalog {
                inventory: MacroInventory {
                    storage: MacroStorageInfo {
                        max_macro_id: 65535,
                        max_memory: 4096,
                        available_memory: 4096,
                        recyclable_memory: 0,
                    },
                    count: 0,
                    ids: vec![],
                    pages: vec![],
                },
                macros: vec![],
            },
            inventory: ProfileInventory {
                max_profiles_supported: 7,
                declared_profile_count: 0,
                slots: vec![],
            },
            profiles: vec![],
        },
        import_details: ImportDetails {
            macros: vec![],
            continuous_rapid_trigger: vec![],
        },
    }
}

struct Mock {
    events: Arc<Mutex<Vec<&'static str>>>,
    pid: u32,
    canceled: Arc<AtomicBool>,
    cancel_after_read: bool,
    fail_read: bool,
    panic_read: bool,
    fail_cleanup: bool,
    wrong_layout: bool,
}

impl Mock {
    fn new(events: &Arc<Mutex<Vec<&'static str>>>, canceled: &Arc<AtomicBool>, pid: u32) -> Self {
        Self {
            events: events.clone(),
            pid,
            canceled: canceled.clone(),
            cancel_after_read: false,
            fail_read: false,
            panic_read: false,
            fail_cleanup: false,
            wrong_layout: false,
        }
    }
    fn record(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }
    fn read_status(&self) -> anyhow::Result<()> {
        assert!(!self.panic_read, "mock physical read panic");
        ensure!(!self.fail_read, "actual transport failure");
        if self.cancel_after_read {
            self.canceled.store(true, Ordering::Release);
        }
        Ok(())
    }
}

impl Transport for Mock {
    fn resolve(&mut self) -> anyhow::Result<Target> {
        self.record("resolve");
        Ok(target(self.pid))
    }
    fn inventory(&mut self, _: &Target) -> anyhow::Result<ProfileInventory> {
        self.record("inventory");
        self.read_status()?;
        Ok(ProfileInventory {
            max_profiles_supported: 7,
            declared_profile_count: 1,
            slots: vec![ProfileSlot {
                slot_id: 9,
                metadata: ProfileMetadataObservation::SourceSkippedFirstSlot,
            }],
        })
    }
    fn complete(&mut self, _: &Target, options: &ReadOptions) -> anyhow::Result<CompleteReading> {
        self.record("complete");
        self.read_status()?;
        let mut reading = complete(options);
        if self.wrong_layout {
            reading.snapshot.layout_id = options.layout_id.wrapping_add(1);
        }
        Ok(reading)
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.record("shutdown");
        ensure!(!self.fail_cleanup, "shutdown failed");
        Ok(())
    }
    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.record("terminate");
        ensure!(!self.fail_cleanup, "termination failed");
        Ok(())
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.record("release");
    }
}

#[test]
fn keyboard_inventory_worker_delivers_only_after_service_shutdown_termination_and_release() {
    let events = Arc::new(Mutex::new(vec![]));
    let canceled = Arc::new(AtomicBool::new(false));
    let result = run_with(None, &canceled, || Ok(Mock::new(&events, &canceled, 550)));
    assert_eq!(
        *events.lock().unwrap(),
        ["resolve", "inventory", "shutdown", "terminate", "release"]
    );
    assert!(result.cleanup_error.is_none());
    let Reading::Inventory(observation) = result.result.unwrap_or_else(|error| panic!("{error}"))
    else {
        panic!("wrong read mode")
    };
    assert_eq!(observation.target, target(550));
    assert_eq!(observation.inventory.max_profiles_supported, 7);
    assert_eq!(observation.inventory.slots[0].slot_id, 9);
}

#[test]
fn keyboard_inventory_worker_679_uses_complete_read_once_without_an_extra_inventory_read() {
    let events = Arc::new(Mutex::new(vec![]));
    let canceled = Arc::new(AtomicBool::new(false));
    let options = ReadOptions {
        layout_id: 0,
        locally_known_macro_guids: ["local-macro-guid".to_owned()].into(),
    };
    let result = run_with(Some(options.clone()), &canceled, || {
        Ok(Mock::new(&events, &canceled, 679))
    });
    assert_eq!(
        *events.lock().unwrap(),
        ["resolve", "complete", "shutdown", "terminate", "release"]
    );
    let Reading::Complete(observation) = result.result.unwrap_or_else(|error| panic!("{error}"))
    else {
        panic!("wrong read mode")
    };
    assert_eq!(observation.options, options);
    assert_eq!(
        observation
            .reading
            .snapshot
            .inventory
            .max_profiles_supported,
        7
    );
    assert!(observation.reading.snapshot.inventory.slots.is_empty());
}

#[test]
fn keyboard_inventory_worker_cancel_before_admission_never_creates_a_service() {
    let canceled = AtomicBool::new(true);
    let result = run_with::<Mock>(None, &canceled, || {
        panic!("canceled request created a service")
    });
    assert!(result.result.is_err());
    assert!(result.cleanup_error.is_none());
}

#[test]
fn keyboard_inventory_worker_cancel_failure_and_panic_always_complete_cleanup() {
    for branch in 0..3 {
        let events = Arc::new(Mutex::new(vec![]));
        let canceled = Arc::new(AtomicBool::new(false));
        let result = run_with(None, &canceled, || {
            let mut mock = Mock::new(&events, &canceled, 550);
            mock.cancel_after_read = branch == 0;
            mock.fail_read = branch == 1;
            mock.panic_read = branch == 2;
            Ok(mock)
        });
        assert!(result.result.is_err());
        assert!(result.cleanup_error.is_none());
        assert_eq!(
            *events.lock().unwrap(),
            ["resolve", "inventory", "shutdown", "terminate", "release"]
        );
    }
}

#[test]
fn keyboard_inventory_worker_retains_independent_cleanup_failures_and_rejects_wrong_read_mode_or_layout()
 {
    let events = Arc::new(Mutex::new(vec![]));
    let canceled = Arc::new(AtomicBool::new(false));
    let result = run_with(None, &canceled, || {
        let mut mock = Mock::new(&events, &canceled, 550);
        mock.fail_cleanup = true;
        Ok(mock)
    });
    assert!(result.result.is_ok());
    let error = result.cleanup_error.unwrap();
    assert!(error.contains("shutdown failed") && error.contains("termination failed"));
    assert_eq!(
        *events.lock().unwrap(),
        ["resolve", "inventory", "shutdown", "terminate", "release"]
    );
    events.lock().unwrap().clear();
    let result = run_with(None, &canceled, || Ok(Mock::new(&events, &canceled, 679)));
    assert!(result.result.is_err());
    assert_eq!(
        *events.lock().unwrap(),
        ["resolve", "shutdown", "terminate", "release"]
    );
    events.lock().unwrap().clear();
    let options = ReadOptions {
        layout_id: 0,
        locally_known_macro_guids: Default::default(),
    };
    let result = run_with(Some(options), &canceled, || {
        let mut mock = Mock::new(&events, &canceled, 679);
        mock.wrong_layout = true;
        Ok(mock)
    });
    assert!(result.result.is_err());
    assert_eq!(
        *events.lock().unwrap(),
        ["resolve", "complete", "shutdown", "terminate", "release"]
    );
}
