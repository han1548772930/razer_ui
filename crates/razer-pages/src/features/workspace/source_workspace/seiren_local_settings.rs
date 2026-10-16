//! Generic33422 Storage_Global: current Ps/Hs own samplingRate/micSideTone at
//! the device-document root. Nested paths are only the mounted UI projection.
use serde_json::{Value,json};

const FIELDS:[(&str,&str);2]=[("profile","samplingRate"),("device","micSideTone")];
fn enabled(product:u32)->bool {matches!(product,1422|1446)}
fn take(snapshot:&mut Value,section:&str,field:&str)->Option<Value> {
    let direct=snapshot.as_object_mut().and_then(|root|root.remove(field));
    snapshot.get_mut(section).and_then(Value::as_object_mut).and_then(|fields|fields.remove(field)).or(direct)
}
pub(super) fn capture(product:u32,settings:&mut Option<Value>,snapshot:&mut Value) {
    if !enabled(product){return;}
    for (section,field) in FIELDS {
        if let Some(value)=take(snapshot,section,field){settings.get_or_insert_with(||json!({}))[field]=value;}
    }
}
pub(super) fn migrate_profile(product:u32,settings:&mut Option<Value>,snapshot:&mut Value) {
    if !enabled(product){return;}
    for (section,field) in FIELDS {
        if let Some(value)=take(snapshot,section,field) {
            settings.get_or_insert_with(||json!({})).as_object_mut().expect("source device settings object").entry(field).or_insert(value);
        }
    }
}
pub(super) fn project(product:u32,settings:Option<&Value>,snapshot:&mut Option<Value>) {
    if !enabled(product){return;}
    for (section,field) in FIELDS {
        let value=settings.and_then(|settings|settings.get(field)).cloned().unwrap_or_else(|| match field {
            "samplingRate"=>json!(48_000),_=>json!({"isEnabled":true,"value":70}),
        });
        let target=snapshot.get_or_insert_with(||json!({}));
        if target.get(section).is_none(){target[section]=json!({});}
        target[section][field]=value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn global_seiren_settings_survive_switching_and_leave_profile_fields_intact() {
        let mut global=None;
        let mut first=json!({"profile":{"samplingRate":96000,"dialControl":2},"device":{"micSideTone":{"isEnabled":false,"value":38},"eqEffects":{"kept":true}},"unknown":"kept"});
        capture(1422,&mut global,&mut first);
        assert_eq!(global,Some(json!({"samplingRate":96000,"micSideTone":{"isEnabled":false,"value":38}})));
        assert!(first["profile"].get("samplingRate").is_none());assert!(first["device"].get("micSideTone").is_none());
        assert_eq!(first["profile"]["dialControl"],2);assert_eq!(first["unknown"],"kept");
        let mut second=Some(json!({"profile":{"samplingRate":44100},"device":{"micSideTone":{"isEnabled":true,"value":90}}}));
        project(1422,global.as_ref(),&mut second);
        assert_eq!(second.as_ref().unwrap()["profile"]["samplingRate"],96000);
        assert_eq!(second.as_ref().unwrap()["device"]["micSideTone"]["value"],38);
    }
    #[test]
    fn migration_prefers_existing_global_then_active_profile_without_changing_other_products() {
        let mut global=Some(json!({"samplingRate":48000,"other":"retained"}));
        let mut active=json!({"profile":{"samplingRate":96000},"device":{"micSideTone":{"isEnabled":true,"value":25}}});
        let mut inactive=json!({"samplingRate":44100,"micSideTone":{"isEnabled":false,"value":90}});
        migrate_profile(1446,&mut global,&mut active);migrate_profile(1446,&mut global,&mut inactive);
        assert_eq!(global.as_ref().unwrap()["samplingRate"],48000);assert_eq!(global.as_ref().unwrap()["micSideTone"]["value"],25);
        assert!(inactive.get("micSideTone").is_none());assert_eq!(global.as_ref().unwrap()["other"],"retained");
        let mut other=json!({"profile":{"samplingRate":44100}});let before=other.clone();
        capture(1342,&mut global,&mut other);assert_eq!(other,before);
    }
}
