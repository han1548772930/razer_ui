use super::*;
use razer_model::{macro_library::MacroLibraryFile,shortcuts::Compiler};
use mapping_control::{Operation,Observation,Target};

#[test]
fn shortcut_dependencies_precede_the_referencing_engine() {
    let document=Compiler::new().compile(&[],&MacroLibraryFile::default()).unwrap();
    let mut writes=Vec::new();
    publish_dependencies(&document,|key,value|{writes.push((key.to_owned(),value.clone()));Ok(())}).unwrap();
    assert_eq!(writes.iter().map(|(key,_)|key.as_str()).collect::<Vec<_>>(),["synapseDefaultTurbos","synapseTurbos","synapseMacros","synapsePhasedMacros"]);
    assert_eq!(writes[0].1.as_array().unwrap().len(),38);
    assert_eq!(writes[1].1,json!([]));assert_eq!(writes[2].1,json!([]));assert_eq!(writes[3].1,json!([]));
    assert_eq!(document.app_engine(),&json!({"hash":"de2ccb2014607a84df08306567cd96f0","mappings":[]}));
}
#[test]
fn failed_dependency_never_runs_later_publication_steps() {
    let document=Compiler::new().compile(&[],&MacroLibraryFile::default()).unwrap();
    for failed in ["synapseDefaultTurbos","synapseTurbos","synapseMacros","synapsePhasedMacros"] {
        let mut writes=Vec::new();
        let error=publish_dependencies(&document,|key,_|{writes.push(key.to_owned());ensure!(key!=failed,"rejected {key}");Ok(())}).unwrap_err();
        assert!(error.to_string().contains(failed));assert_eq!(writes.last().unwrap(),failed);
    }
}
fn target(mapping:bool,shortcuts:Option<bool>)->Target {serde_json::from_value(json!({"mapping_enabled":mapping,"global_shortcuts_enabled":shortcuts})).unwrap()}
#[test]
fn execution_gate_scope_preserves_each_required_operation() {
    use Operation::*;
    for (mapping,shortcuts,expected) in [(false,None,vec![DisableMapping]),(true,None,vec![EnableMapping]),(false,Some(false),vec![DisableMapping,DisableGlobalShortcuts]),(true,Some(true),vec![EnableMapping,EnableGlobalShortcuts]),(false,Some(true),vec![DisableMapping,EnableGlobalShortcuts])] {
        let mut calls=Vec::new();
        apply_execution(target(mapping,shortcuts),|operation|{calls.push(operation.clone());Ok(Observation::new(operation,true,String::new()))}).unwrap();
        assert_eq!(calls,expected);
    }
}
#[test]
fn failed_mapping_restore_still_restores_shortcuts() {
    let mut calls=Vec::new();
    let error=apply_execution(target(true,Some(true)),|operation|{calls.push(operation.clone());anyhow::bail!("rejected {operation:?}")}).unwrap_err();
    assert_eq!(calls,[Operation::EnableMapping,Operation::EnableGlobalShortcuts]);
    assert!(error.to_string().contains("EnableMapping"));assert!(error.to_string().contains("EnableGlobalShortcuts"));
}
#[test]
fn already_satisfied_source_observations_remain_false() {
    let observations=apply_execution(target(true,Some(true)),|operation|{
        let reason=if operation==Operation::EnableMapping{"already enabled"}else{"hotkey was not disabled"};
        Ok(Observation::new(operation,false,reason.into()))
    }).unwrap();
    assert_eq!(observations.len(),2);assert!(observations.iter().all(|observation|!observation.result()));
}
#[test]
fn unrelated_false_results_never_confirm_a_gate() {
    use Operation::*;
    for operation in [EnableMapping,DisableMapping,EnableGlobalShortcuts,DisableGlobalShortcuts] {
        for reason in ["","access denied","already enabled","already disabled","hotkey was not disabled"] {
            let expected=matches!((&operation,reason),(EnableMapping,"already enabled")|(DisableMapping,"already disabled")|(EnableGlobalShortcuts,"hotkey was not disabled"));
            assert_eq!(execution_confirmed(&Observation::new(operation.clone(),false,reason.into())),expected);
        }
    }
}

#[test]
fn device_event_queues_preserve_owner_generation_and_independent_drains() {
    let a=Uuid::new_v4();let b=Uuid::new_v4();
    let mut first=Inbox::new(a);let mut second=Inbox::new(b);
    first.push(a,"first");second.push(b,"second");
    first.push(b,"foreign");second.push(a,"foreign");
    assert_eq!(first.drain().unwrap(),["first"]);
    assert_eq!(second.drain().unwrap(),["second"]);
    let mut replacement=Inbox::new(Uuid::new_v4());
    replacement.push(a,"late previous connection");
    assert!(replacement.drain().unwrap().is_empty());
}
#[test]
fn typed_event_queue_preserves_payload_and_rejects_partial_overflow() {
    let generation=Uuid::new_v4();let mut queue=Inbox::new(generation);
    let event=razer_ipc::shortcuts::UnsupportedMapping::new("device".into(),"[1,2]".into(),"{}".into(),u64::MAX);
    queue.push(generation,event);
    let events=queue.drain().unwrap();assert_eq!(events[0].device_info(),"device");assert_eq!(events[0].input(),"[1,2]");assert_eq!(events[0].time_tick(),u64::MAX);
    for tick in 0..=512 {queue.push(generation,razer_ipc::shortcuts::UnsupportedMapping::new(String::new(),String::new(),String::new(),tick));}
    assert!(queue.drain().is_err());assert_eq!(queue.pending.len(),512);
}
