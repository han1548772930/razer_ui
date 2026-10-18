use super::*;

const SOURCE: &str = r#"<Macro>
<Name>本地宏</Name><Version>4</Version><DelaySetting>0</DelaySetting><MouseMoveType>none</MouseMoveType>
<MacroEvents>
<MacroEvent><Type>actionBar</Type><recordProfile/></MacroEvent>
<MacroEvent><Type>1</Type><KeyEvent><Makecode>30</Makecode><State>0</State></KeyEvent></MacroEvent>
<MacroEvent><Type>4</Type><Text>你好 &amp; &lt;tag&gt; 😀</Text></MacroEvent>
<MacroEvent><Type>1</Type><KeyEvent><Makecode>30</Makecode><State>1</State></KeyEvent></MacroEvent>
</MacroEvents></Macro>"#;

#[test]
fn source_xml_decodes_unicode_and_native_keyboard_edges_without_registration() {
    let entry = decode(SOURCE).unwrap();
    assert_eq!(entry.name, "本地宏");
    assert_eq!(entry.macro_type, MacroType::Standard);
    assert_eq!(entry.actions.len(), 3);
    assert_eq!(entry.actions[1].kind, ActionKind::Text);
    assert_eq!(entry.actions[1].value, "你好 & <tag> 😀");
    for (action, state) in [(&entry.actions[0], 0), (&entry.actions[2], 1)] {
        let key = action.keyboard.as_ref().unwrap();
        assert_eq!(key.makecode, Some(30));
        assert_eq!(key.state, Some(state));
        assert_eq!(key.flag, Some(state));
        assert_eq!(key.pair_id, Some(1));
    }
    assert!(entry.xml_guid.is_none());
    assert!(entry.source_document.is_none());
}

#[test]
fn source_xml_export_preserves_content_and_explicit_event_order() {
    let mut entry = decode(SOURCE).unwrap();
    entry.xml_guid = Some("11111111-1111-4111-8111-111111111111".into());
    let output = encode(&entry, std::slice::from_ref(&entry)).unwrap();
    let document = Document::parse(&output).unwrap();
    let root = document.root_element();
    assert_eq!(value(root, "Guid"), "11111111-1111-4111-8111-111111111111");
    assert_eq!(value(root, "Name"), "本地宏");
    assert_eq!(value(root, "Version"), "4");
    let events = child(root, "MacroEvents").unwrap();
    let events: Vec<_> = events.children().filter(|node| node.is_element()).collect();
    assert_eq!(
        events
            .iter()
            .map(|event| value(*event, "Type"))
            .collect::<Vec<_>>(),
        ["actionBar", "1", "4", "1"]
    );
    assert_eq!(value(events[2], "Text"), "你好 & <tag> 😀");
    assert_eq!(value(child(events[1], "KeyEvent").unwrap(), "State"), "0");
    assert_eq!(value(child(events[3], "KeyEvent").unwrap(), "State"), "1");
}

#[test]
fn source_xml_rejects_lossy_and_invalid_documents_atomically() {
    for invalid in [
        SOURCE.replace("<Type>4</Type>", "<Type>99</Type>"),
        SOURCE.replace("<Version>4</Version>", "<Version>3</Version>"),
        SOURCE.replace("<Text>", "<phase>1</phase><Text>"),
        SOURCE.replace("<Type>actionBar</Type>", "<Type>0</Type>"),
        SOURCE.replace("你好 &amp;", "你好 &amp;amp;"),
        "<!DOCTYPE Macro [<!ENTITY example SYSTEM 'file:///missing'>]><Macro>&example;</Macro>"
            .into(),
    ] {
        assert!(decode(&invalid).is_err());
    }
    assert!(decode(&" ".repeat(MAX_BYTES + 1)).is_err());
}
