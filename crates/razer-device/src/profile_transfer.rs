//! Current756 UI55714/31867 and MW50574.ry. A .synapse4 file contains
//! Base64 UTF-8 JSON and per-entry hashes; parsing never applies a profile.
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use html5ever::{parse_fragment, tendril::TendrilSink, LocalName, QualName, ns, namespace_url};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

#[derive(Clone, Debug)]
pub struct DecodedFile {
    profiles: Vec<Value>,
    macros: Vec<Value>,
    device_name: String,
    warning: bool,
}
impl DecodedFile {
    pub fn profiles(&self) -> &[Value] { &self.profiles }
    pub fn macros(&self) -> &[Value] { &self.macros }
    pub fn device_name(&self) -> &str { &self.device_name }
    pub fn has_warning(&self) -> bool { self.warning }
}

/// UI55714 recursively reads the textContent of a detached div after assigning
/// innerHTML. Parse a fragment with the same HTML context; no script is run.
pub fn decode_xml_entities(value: &Value) -> Value {
    fn collect(node: &Handle, result: &mut String) {
        if let NodeData::Text { contents } = &node.data { result.push_str(&contents.borrow()); }
        for child in node.children.borrow().iter() { collect(child, result); }
    }
    match value {
        Value::String(value) => {
            let dom=parse_fragment(RcDom::default(),Default::default(),
                QualName::new(None,ns!(html),LocalName::from("div")),Vec::new()).one(value.clone());
            let mut result=String::new(); collect(&dom.document,&mut result); Value::String(result)
        }
        Value::Array(rows)=>Value::Array(rows.iter().map(decode_xml_entities).collect()),
        Value::Object(rows)=>Value::Object(rows.iter().map(|(key,value)|(key.clone(),decode_xml_entities(value))).collect()),
        other=>other.clone(),
    }
}

/// MW50574.ay builds a fresh ordinary macro engine from the imported source
/// events. Hashing precedes assigning the selected mouse mode in that body.
pub fn prepare_macro(value:&Value)->Result<Value> {
    let mut result=value.clone();
    let events=value["macroEvents"].as_array().context("Imported macro events are absent")?;
    let mode=events.first().and_then(|event|event["recordProfile"]["mmtSetting"].as_u64()).unwrap_or(0);
    let mode=u8::try_from(mode).context("Imported macro mouse mode is invalid")?;
    let movement=match mode {0=>"none",1=>"absolute",2=>"foreground",3=>"relative",_=>anyhow::bail!("Imported macro mouse mode is unknown")};
    let mut compiled=Vec::new();
    for event in events {
        let geometry=event.get("monitorInforBefore").map(|_|event);
        compiled.extend(crate::macro_engine::compile_event(event,mode,geometry)?);
    }
    let mut engine=json!({"events":compiled,"hash":"","mouseMoveType":"absolute"});
    engine["hash"]=crate::mapping_engine_hash::source_hash(&engine)?.into();
    engine["mouseMoveType"]=movement.into();result["appEngine"]=engine;
    Ok(result)
}

fn truthy(value:&Value)->bool {
    match value {
        Value::Null=>false,Value::Bool(value)=>*value,
        Value::Number(value)=>value.as_f64().is_some_and(|value|value!=0.),
        Value::String(value)=>!value.is_empty(),Value::Array(_)|Value::Object(_)=>true,
    }
}

fn decode_base64(input: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(input.len() * 3 / 4);
    let (mut bits, mut available) = (0u32, 0);
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch-b'A', b'a'..=b'z' => ch-b'a'+26,
            b'0'..=b'9' => ch-b'0'+52, b'+'|b'-' => 62, b'/'|b'_' => 63,
            // UI57207 normalizes URL-safe characters and removes all other
            // characters, including padding, before decoding.
            _ => continue,
        };
        bits=(bits<<6)|u32::from(value); available+=6;
        if available>=8 { available-=8; bytes.push((bits>>available) as u8); }
    }
    ensure!(available != 6, "Truncated Base64 profile payload");
    Ok(bytes)
}
fn encode_base64(input: &[u8]) -> String {
    const ALPHABET: &[u8;64]=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out=String::with_capacity(input.len().div_ceil(3)*4);
    for bytes in input.chunks(3) {
        let value=(u32::from(bytes[0])<<16)|(u32::from(*bytes.get(1).unwrap_or(&0))<<8)|u32::from(*bytes.get(2).unwrap_or(&0));
        out.push(ALPHABET[((value>>18)&63) as usize] as char);
        out.push(ALPHABET[((value>>12)&63) as usize] as char);
        out.push(if bytes.len()>1 {ALPHABET[((value>>6)&63) as usize] as char} else {'='});
        out.push(if bytes.len()>2 {ALPHABET[(value&63) as usize] as char} else {'='});
    }
    out
}

fn decode_entry(encoded: &Value) -> Result<Value> {
    let bytes=decode_base64(encoded["payload"].as_str().context("Profile payload is absent")?)?;
    // js-base64 decodes malformed UTF-8 with replacement characters.
    let mut value:Value=serde_json::from_str(&String::from_utf8_lossy(&bytes))?;
    let mut hashed=value.clone();
    let object=hashed.as_object_mut().context("Profile payload is not an object")?;
    object.remove("hash"); object.remove("gamemode");
    let digest=format!("{:x}",md5::compute(crate::mapping_engine_hash::canonical_json(&hashed)?));
    ensure!(encoded["hash"].as_str()==Some(digest.as_str()),"Profile hash does not match");
    value["name"]=json!(encoded["name"].as_str().context("Profile name is absent")?);
    Ok(value)
}

/// The file reader's category/PID compatibility and corrupt-entry semantics.
/// Payload hashes are verified before applying the outer display name.
pub fn decode(bytes: &[u8], target: &Value) -> Result<DecodedFile> {
    let text=String::from_utf8_lossy(bytes);
    let document:Value=serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text))?;
    let rows=document["profiles"].as_array().context("Profile file has no profiles array")?;
    let same_number=|a:&Value,b:&Value| a.as_f64().zip(b.as_f64()).is_some_and(|(a,b)|a==b);
    let same_category=document["category"].is_string()&&document["category"]==target["category"];
    let compatible=same_category||["productId","dongleId","bleId"].into_iter().any(|field|same_number(&document["productId"],&target[field]));
    let mut warning=same_category&&!same_number(&document["productId"],&target["productId"]);
    let mut decode_rows=|rows:&[Value]| rows.iter().filter_map(|entry|match decode_entry(entry) {
        Ok(value)=>Some(value),Err(_)=>{warning=true;None}
    }).collect::<Vec<_>>();
    let mut profiles=if compatible {decode_rows(rows)} else {Vec::new()};
    let macros=decode_rows(document["macros"].as_array().map(Vec::as_slice).unwrap_or(&[]));
    if !truthy(&target["is8kAnalogDevice"]) {
        for profile in &mut profiles {
            let mut rows=profile["mappings"].as_array().cloned().unwrap_or_default();
            rows.retain(|row|!row["mapping"].as_array().is_some_and(|stages|stages.iter().any(|stage|truthy(&stage["dynamicKeyStrokeGroup"]))));
            profile["mappings"]=json!(rows);
        }
    }
    Ok(DecodedFile{profiles,macros,device_name:document["deviceName"].as_str().unwrap_or("").into(),warning})
}

fn encode_entry(value: &Value, name: &str) -> Result<Value> {
    ensure!(value.is_object(),"Export payload is not an object");
    Ok(json!({"name":name,"payload":encode_base64(serde_json::to_string(value)?.as_bytes()),
        "hash":crate::mapping_engine_hash::source_hash(value)?}))
}
/// MW50574.ry excludes only profile appEngine and exports only the three macro
/// payload fields. Game links and external Chroma effects live in other stores.
pub fn encode(target:&Value, profiles:&[Value], macros:&[Value]) -> Result<Vec<u8>> {
    let profiles=profiles.iter().map(|profile| {
        let mut value=profile.clone();
        value.as_object_mut().context("Export profile is not an object")?.remove("appEngine");
        encode_entry(&value,profile["name"].as_str().context("Export profile name is absent")?)
    }).collect::<Result<Vec<_>>>()?;
    let macros=macros.iter().map(|mac|encode_entry(&json!({"guid":mac["guid"],"macroEvents":mac["macroEvents"],
        "macroPlaybackOption":mac["macroPlaybackOption"]}),mac["name"].as_str().context("Export macro name is absent")?)).collect::<Result<Vec<_>>>()?;
    Ok(serde_json::to_vec_pretty(&json!({"productId":target["productId"],"deviceName":target["deviceName"],
        "category":target["category"],"profiles":profiles,"macros":macros}))?)
}

/// The current middleware helper preserves an existing numeric suffix. The UI
/// duplication helper removes that suffix before dispatching; callers choose
/// the helper belonging to their own stage instead of conflating them.
pub fn imported_name(rows:&[Value], name:&str)->String {
    let used=|candidate:&str| rows.iter().any(|row|row["name"]==candidate||row["content"]==candidate);
    if !used(name) {return if name.trim().is_empty(){format!("{name} (1)")}else{name.into()};}
    (1..).map(|n|format!("{name} ({n})")).find(|candidate|!used(candidate)).expect("finite profile names")
}

/// Current UI21561/3342/35380, followed by MW50574.ay. Each selected row has
/// profile/listItems/hasWarning. The analog root imports the complete macroData
/// array, even when an individual row's macro checkbox is cleared.
pub fn prepare_import_756(
    rows:&[Value], macros:&[Value], existing_profiles:&[Value], existing_macros:&[Value],
    active_guid:&str, layout:u32, mut fresh_guid:impl FnMut()->String,
)->Result<(Vec<Value>,Vec<Value>)> {
    use std::collections::BTreeMap;
    let source=crate::keyboard_latest_mapping::source();
    let mut all_macros=existing_macros.to_vec();
    let mut remapped=BTreeMap::<String,(String,String)>::new();
    let mut imported_macros=Vec::new();
    for value in macros {
        let old=value["guid"].as_str().context("Imported macro GUID is absent")?;
        let mut value=value.clone();
        let name=imported_name(&all_macros,value["name"].as_str().context("Imported macro name is absent")?);
        value["name"]=json!(name);
        if all_macros.iter().any(|mac|mac["guid"]==old) {
            let guid=fresh_guid();remapped.insert(old.to_owned(),(guid.clone(),name));value["guid"]=json!(guid);
        }
        let value=prepare_macro(&value)?;all_macros.push(value.clone());imported_macros.push(value);
    }
    let target=&source["profile_transfer_target"];
    let info=if active_guid==source["preset_guids"]["HIGH_SENSITIVITY"].as_str().unwrap_or("") {
        &target["actuationHighSensitivityInfo"]
    } else {&target["actuationInfo"]};
    let default=info["defaultValue"].as_u64().context("Import actuation default is absent")?;
    let unit=info["unit"].as_u64().context("Import actuation unit is absent")?;
    let minimum=info["minBreak"].as_u64().context("Import actuation break minimum is absent")?;
    let actuation=json!({"0":default,"1":default.saturating_sub(unit).max(minimum)});
    let disabled=source["button_list"].as_array().context("Import button list is absent")?
        .iter().filter(|button|button["disableHypershiftMapping"]==true)
        .filter_map(|button|button["inputID"].as_str()).collect::<Vec<_>>();
    let non_analog=source["import_non_analog_ids"].as_array().context("Import analog exemptions are absent")?;
    let keymap_passthrough=source["import_keymap_passthrough_ids"].as_array().context("Import keymap exemptions are absent")?;
    let restricted_layout=source["import_restricted_layouts"].as_array().context("Import layout rules are absent")?
        .iter().any(|value|value.as_u64()==Some(u64::from(layout)));
    let restricted_ids=source["import_restricted_input_ids"].as_array().context("Import layout input IDs are absent")?;
    let convert_analog=|mapping:&Value|->Value {
        if mapping["inputType"]=="AnalogInput"||non_analog.contains(&mapping["inputID"]) {return mapping.clone();}
        let mut primary=json!({"actuationPoint":actuation,"outputType":mapping["outputType"]});
        if let Some(kind)=mapping["outputType"].as_str() {
            if let Some(value)=mapping.get(kind) {primary[kind]=value.clone();}
        }
        let mut result=json!({"inputType":"AnalogInput","mapping":[primary,
            {"actuationPoint":actuation,"isDefault":false,"outputType":"defaultGroup","isHyperShift":false}]});
        for key in ["inputID","isHyperShift"] {if let Some(value)=mapping.get(key) {result[key]=value.clone();}}
        result
    };
    let rewrite=|mapping:&mut Value| {
        let direct=mapping.get("macroGroup").is_some();
        let group=if direct {mapping.get_mut("macroGroup")} else {
            mapping.get_mut("mapping").and_then(Value::as_array_mut).and_then(|stages|stages.first_mut()).and_then(|stage|stage.get_mut("macroGroup"))
        };
        if let Some(group)=group {
            if let Some(guid)=group["guid"].as_str() {
                if let Some((guid,name))=remapped.get(guid) {group["guid"]=json!(guid);group["name"]=json!(name);}
                else if let Some(mac)=all_macros.iter().find(|mac|mac["guid"]==guid) {group["name"]=mac["name"].clone();}
            }
        }
    };
    let mut all_profiles=existing_profiles.to_vec();let mut imported_profiles=Vec::new();
    for row in rows {
        let mut profile=row["profile"].clone();
        ensure!(profile.is_object(),"Imported profile is not an object");
        let mappings=profile["mappings"].as_array().context("Imported profile mappings are absent")?;
        // UI35380 filters both spellings before Analog conversion.
        let mut mappings=mappings.iter().filter(|mapping|!((mapping["isHyperShift"]==true||mapping["IsHyperShift"]==true)
            &&mapping["inputID"].as_str().is_some_and(|id|disabled.contains(&id))))
            .map(&convert_analog).collect::<Vec<_>>();
        let mut deselected=row["listItems"].as_array().cloned().unwrap_or_default();
        for mac in &all_macros {
            if !deselected.iter().any(|item|item["guid"]==mac["guid"]) {
                deselected.push(json!({"guid":mac["guid"],"text":mac["name"]}));
            }
        }
        mappings.retain(|mapping|!deselected.iter().any(|item|item["isMacroSelected"]!=true&&
            mapping.get("macroGroup").is_some_and(|group|group["guid"]==item["guid"])));
        for mapping in &mut mappings {rewrite(mapping);}
        if row["hasWarning"]==true {
            mappings.retain(|mapping|!matches!(mapping["inputID"].as_str(),Some("DKM_KBMK_01"|"DKM_KBMK_02"|"DKM_KBMK_03")));
        }
        mappings.retain(|mapping|!(restricted_layout&&restricted_ids.contains(&mapping["inputID"])||
            mapping["isHyperShift"]==true&&mapping["inputID"].as_str().is_some_and(|id|disabled.contains(&id))));
        profile["mappings"]=json!(mappings);
        if let Some(keymaps)=profile.get_mut("keymaps").and_then(Value::as_array_mut) {
            for keymap in keymaps {
                if let Some(mappings)=keymap.get_mut("mappings").and_then(Value::as_array_mut) {
                    for mapping in mappings {
                        if !keymap_passthrough.contains(&mapping["inputID"]) {*mapping=convert_analog(mapping);}
                        rewrite(mapping);
                    }
                }
            }
        }
        if let Some(keys)=profile.get_mut("snapTap").and_then(|tap|tap.get_mut("keyList")).and_then(Value::as_array_mut) {
            let mode=&source["default_profile"]["snapTap"]["keyList"][0]["mode"];
            for key in keys {if !truthy(&key["mode"]) {key["mode"]=mode.clone();}}
        }
        if !truthy(&profile["commandDial"])&&source["default_profile"].get("commandDial").is_some() {
            profile["commandDial"]=source["default_profile"]["commandDial"].clone();
        }
        profile["name"]=json!(imported_name(&all_profiles,profile["name"].as_str().context("Imported profile name is absent")?));
        profile["guid"]=json!(fresh_guid());profile.as_object_mut().unwrap().remove("appEngine");
        all_profiles.push(profile.clone());imported_profiles.push(profile);
    }
    Ok((imported_profiles,imported_macros))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_file_roundtrip_excludes_engines_and_checks_before_outer_name() {
        let target=json!({"productId":756,"deviceName":"Reclusa","category":"KEYBOARD","is8kAnalogDevice":true});
        let profile=json!({"guid":"profile","name":"键盘","mappings":[],"appEngine":{"mappings":[1]}});
        let mac=json!({"guid":"macro","name":"宏","macroEvents":[{"Type":1}],"macroPlaybackOption":"standard","appEngine":{"events":[]}});
        let bytes=encode(&target,&[profile],&[mac]).unwrap();
        let mut outer:Value=serde_json::from_slice(&bytes).unwrap();
        outer["profiles"][0]["name"]=json!("Outer name");
        let loaded=decode(&serde_json::to_vec(&outer).unwrap(),&target).unwrap();
        assert_eq!(loaded.profiles()[0]["name"],"Outer name");
        assert!(loaded.profiles()[0].get("appEngine").is_none());
        assert!(loaded.macros()[0].get("appEngine").is_none());
        outer["profiles"][0]["hash"]=json!("invalid");
        let loaded=decode(&serde_json::to_vec(&outer).unwrap(),&target).unwrap();
        assert!(loaded.profiles().is_empty()); assert!(loaded.has_warning());
        assert_eq!(loaded.macros().len(),1);
    }
    #[test]
    fn current_reader_and_writer_keep_their_distinct_less_than_hash_rules() {
        let target=json!({"productId":756,"category":"KEYBOARD"});
        let bytes=encode(&target,&[json!({"name":"<tag>","mappings":[]})],&[]).unwrap();
        let loaded=decode(&bytes,&target).unwrap();
        assert!(loaded.profiles().is_empty()); assert!(loaded.has_warning());
        assert_eq!(decode_base64("e=y!J4I joxfQ==").unwrap(),br#"{"x":1}"#);
    }
}
