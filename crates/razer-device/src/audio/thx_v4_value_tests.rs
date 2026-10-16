use super::*;
use crate::thx_v4::infer_property_type;
use serde_json::json;

fn request(kind: &str, value: Value) -> ThxPropertyRequest {
    ThxPropertyRequest::new("property", value, Some(kind)).unwrap()
}
fn convert(kind: &str, value: Value) -> Property {
    property_from_request(&request(kind, value)).unwrap()
}

#[test]
fn bool_request_retains_false_presence_and_exact_wire_bytes() {
    let property =
        property_from_request(&ThxPropertyRequest::boolean("spatial_enabled", false)).unwrap();
    assert_eq!(property.property_type, PropertyType::Bool);
    assert_eq!(property.bool_value, Some(false));
    assert_eq!(property.observed_bool("spatial_enabled").unwrap(), false);
    let mut expected = vec![0x0a, 15];
    expected.extend_from_slice(b"spatial_enabled");
    expected.extend_from_slice(&[0x10, 1, 0x28, 0]);
    assert_eq!(property.encode().unwrap(), expected);
    assert_eq!(Property::decode(&expected).unwrap(), property);
}

#[test]
fn native_bool_and_string_converters_reject_other_json_kinds() {
    for value in [
        Value::Null,
        json!(0),
        json!(1.0),
        json!("true"),
        json!([]),
        json!({}),
    ] {
        assert_eq!(
            property_from_request(&request("Bool", value)),
            Err(expected("boolean", None))
        );
    }
    for value in [Value::Null, json!(true), json!(0), json!([]), json!({})] {
        assert_eq!(
            property_from_request(&request("String", value)),
            Err(expected("string", None))
        );
    }
    assert_eq!(
        convert("String", json!("héllo\u{0000}世界"))
            .string_value
            .as_deref(),
        Some("héllo\u{0000}世界")
    );
    assert_eq!(
        convert("String", json!("")).string_value,
        Some(String::new())
    );
}

#[test]
fn int_signed_and_unsigned_take_low_32_bits_then_sign_extend() {
    // Native MOV EAX,[RAX] at 0x1A5355 / 0x1A5373 then CDQE at 0x19E435.
    for (value, observed) in [
        (json!(-1), -1),
        (json!(-2147483649i64), 2147483647),
        (json!(2147483647u64), 2147483647),
        (json!(2147483648u64), -2147483648),
        (json!(4294967295u64), -1),
        (json!(4294967296u64), 0),
        (json!(u64::MAX), -1),
        (json!(i64::MIN), 0),
        (json!(i64::MAX), -1),
    ] {
        assert_eq!(convert("Int", value).int_value, Some(observed));
    }
}

#[test]
fn int_double_truncates_and_returns_sse_indefinite_on_overflow() {
    // Masked CVTTSD2SI EAX consumes a double, not Rust's saturating cast.
    for (value, observed) in [
        (3.9, 3),
        (-3.9, -3),
        (-0.0, 0),
        (2147483647.99, 2147483647),
        (2147483648.0, i32::MIN),
        (-2147483648.9, i32::MIN),
        (-2147483649.0, i32::MIN),
        (1e300, i32::MIN),
        (-1e300, i32::MIN),
    ] {
        assert_eq!(
            convert("Int", json!(value)).int_value,
            Some(i64::from(observed))
        );
    }
}

#[test]
fn numeric_converters_accept_native_boolean_number_branch() {
    assert_eq!(convert("Int", json!(true)).int_value, Some(1));
    assert_eq!(convert("Int", json!(false)).int_value, Some(0));
    assert_eq!(convert("Float", json!(true)).float_value, Some(1.0));
    assert_eq!(convert("Float", json!(false)).float_value, Some(0.0));
    for kind in ["Int", "Float"] {
        for value in [Value::Null, json!("5"), json!([]), json!({})] {
            assert_eq!(
                property_from_request(&request(kind, value)),
                Err(expected("number", None))
            );
        }
    }
}

#[test]
fn float_integer_conversion_uses_all_64_bits_despite_decompiler_cast_artifact() {
    // The native instruction is CVTSI2SS qword, not a low32 integer load.
    for (value, bits) in [
        (json!(-4294967296i64), 0xcf80_0000),
        (json!(4294967296u64), 0x4f80_0000),
        (json!(i64::MIN), 0xdf00_0000),
        (json!(9223372036854775808u64), 0x5f00_0000),
        (json!(u64::MAX), 0x5f80_0000),
        (json!(16777217u64), 0x4b80_0000),
    ] {
        assert_eq!(convert("Float", value).float_value.unwrap().to_bits(), bits);
    }
}

#[test]
fn double_to_float_rounding_zero_and_overflow_are_not_normalized() {
    let negative_zero = convert("Float", json!(-0.0));
    assert_eq!(negative_zero.float_value.unwrap().to_bits(), 0x8000_0000);
    assert_eq!(
        Property::decode(&negative_zero.encode().unwrap())
            .unwrap()
            .float_value
            .unwrap()
            .to_bits(),
        0x8000_0000
    );
    assert_eq!(
        convert("Float", json!(0.1)).float_value.unwrap().to_bits(),
        0x3dcc_cccd
    );
    assert_eq!(
        convert("Float", json!(1e300)).float_value.unwrap(),
        f32::INFINITY
    );
    assert_eq!(
        convert("Float", json!(-1e300)).float_value.unwrap(),
        f32::NEG_INFINITY
    );
    assert_eq!(
        convert("Float", json!(1e-300))
            .float_value
            .unwrap()
            .to_bits(),
        0
    );
}

#[test]
fn none_ignores_any_value_while_scalar_missing_value_is_an_explicit_error() {
    for value in [
        Value::Null,
        json!(true),
        json!(1.0),
        json!("x"),
        json!([1, "x"]),
        json!({"a": []}),
    ] {
        assert_eq!(
            convert("None", value),
            Property {
                id: "property".into(),
                ..Property::default()
            }
        );
    }
    let none = property_from_json(&json!({"id": "", "type": "None"})).unwrap();
    assert_eq!(none, Property::default());
    assert_eq!(
        json_from_property(&none).unwrap(),
        json!({"id": "", "type": "None"})
    );
    assert_eq!(value_from_property(&none), Err(ValueError::NoValue));
    for kind in [
        "Bool",
        "Int",
        "Float",
        "String",
        "StringArray",
        "FloatArray",
    ] {
        assert_eq!(
            property_from_json(&json!({"id": "x", "type": kind})),
            Err(ValueError::MissingField("value"))
        );
    }
}

#[test]
fn array_handlers_iterate_arrays_null_and_single_primitive_values() {
    assert_eq!(
        convert("StringArray", json!(["", "héllo"])).string_array_value,
        ["", "héllo"]
    );
    assert_eq!(
        convert("StringArray", json!("whole string")).string_array_value,
        ["whole string"]
    );
    assert!(
        convert("StringArray", Value::Null)
            .string_array_value
            .is_empty()
    );
    assert!(
        convert("StringArray", json!([]))
            .string_array_value
            .is_empty()
    );
    assert_eq!(
        convert("FloatArray", json!([true, -5, 4294967296u64, 1.25])).float_array_value,
        [1.0, -5.0, 4294967296.0, 1.25]
    );
    assert_eq!(convert("FloatArray", json!(true)).float_array_value, [1.0]);
    assert_eq!(convert("FloatArray", json!(1.25)).float_array_value, [1.25]);
    assert!(
        convert("FloatArray", Value::Null)
            .float_array_value
            .is_empty()
    );
}

#[test]
fn array_object_values_follow_native_string_map_order() {
    let strings =
        serde_json::from_str(r#"{"z":"last","A":"first","é":"unicode","a":"second"}"#).unwrap();
    assert_eq!(
        convert("StringArray", strings).string_array_value,
        ["first", "second", "last", "unicode"]
    );
    let floats = serde_json::from_str(r#"{"z":3,"a":1,"b":2.5}"#).unwrap();
    assert_eq!(
        convert("FloatArray", floats).float_array_value,
        [1.0, 2.5, 3.0]
    );
    assert!(
        convert("FloatArray", json!({}))
            .float_array_value
            .is_empty()
    );
}

#[test]
fn array_handlers_convert_each_element_and_fail_at_invalid_value() {
    assert_eq!(
        property_from_request(&request("StringArray", json!(["valid", 3]))),
        Err(expected("string", Some(1)))
    );
    assert_eq!(
        property_from_request(&request("FloatArray", json!([1.25, "3"]))),
        Err(expected("number", Some(1)))
    );
    assert_eq!(
        property_from_request(&request("FloatArray", json!([[1.25]]))),
        Err(expected("number", Some(0)))
    );
    assert_eq!(
        property_from_request(&request("StringArray", json!(true))),
        Err(expected("string", Some(0)))
    );
    assert_eq!(
        property_from_request(&request("FloatArray", json!({"a": 1, "b": null}))),
        Err(expected("number", Some(1)))
    );
}

#[test]
fn source_type_inference_is_preserved_before_native_converter_dispatch() {
    assert_eq!(infer_property_type(&json!(5)), ThxPropertyType::None);
    let inferred = ThxPropertyRequest::parse("x", "5", None).unwrap();
    assert_eq!(
        property_from_request(&inferred).unwrap().property_type,
        PropertyType::None
    );
    let explicit = ThxPropertyRequest::parse("x", "5", Some("Float")).unwrap();
    assert_eq!(
        property_from_request(&explicit).unwrap().float_value,
        Some(5.0)
    );
    assert_eq!(
        infer_property_type(&json!([true, false])),
        ThxPropertyType::Bool
    );
    let boolean_array = ThxPropertyRequest::new("x", json!([true, false]), None).unwrap();
    assert_eq!(
        property_from_request(&boolean_array),
        Err(expected("boolean", None))
    );
}

#[test]
fn property_json_validates_id_type_and_exact_registered_type_names() {
    assert_eq!(
        property_from_json(&json!([])),
        Err(ValueError::ExpectedObject)
    );
    assert_eq!(
        property_from_json(&json!({"type": "Bool", "value": true})),
        Err(ValueError::MissingField("id"))
    );
    assert_eq!(
        property_from_json(&json!({"id": 5, "type": "Bool", "value": true})),
        Err(ValueError::InvalidField("id"))
    );
    assert_eq!(
        property_from_json(&json!({"id": "x", "value": true})),
        Err(ValueError::MissingField("type"))
    );
    assert_eq!(
        property_from_json(&json!({"id": "x", "type": 1, "value": true})),
        Err(ValueError::InvalidField("type"))
    );
    for name in [
        "",
        "bool",
        "BOOL",
        "Bool ",
        "PropertyType_Bool",
        "Count",
        "Missing",
    ] {
        assert_eq!(
            property_from_json(&json!({"id": "x", "type": name, "value": true})),
            Err(ValueError::UnknownType(name.into()))
        );
    }
    assert!(property_from_json(&json!({"id": "", "type": "String", "value": ""})).is_ok());
    assert_eq!(
        property_from_request(&request("Bool", json!(false))).unwrap(),
        property_from_json(
            &json!({"id": "property", "type": "Bool", "value": false, "ignored": 3})
        )
        .unwrap()
    );
}

#[test]
fn native_projection_defaults_are_separate_from_strict_device_observation() {
    let defaults = [
        (PropertyType::Bool, json!(false)),
        (PropertyType::Int, json!(0)),
        (PropertyType::Float, json!(0.0)),
        (PropertyType::String, json!("")),
        (PropertyType::StringArray, json!([])),
        (PropertyType::FloatArray, json!([])),
    ];
    for (property_type, value) in defaults {
        let property = Property {
            id: "x".into(),
            property_type,
            ..Property::default()
        };
        assert_eq!(value_from_property(&property).unwrap(), value);
    }
    let missing = Property {
        id: "spatial_enabled".into(),
        property_type: PropertyType::Bool,
        ..Property::default()
    };
    assert_eq!(value_from_property(&missing).unwrap(), json!(false));
    assert_eq!(
        missing.observed_bool("spatial_enabled"),
        Err(CodecError::MissingPropertyValue)
    );
    assert_eq!(
        missing.observed_bool("different"),
        Err(CodecError::PropertyIdMismatch)
    );
}

#[test]
fn native_projection_selects_by_type_and_preserves_full_reply_int64() {
    let mut property = Property {
        id: "x".into(),
        property_type: PropertyType::Int,
        int_value: Some(i64::MAX),
        bool_value: Some(true),
        string_value: Some("extra".into()),
        float_value: Some(2.5),
        string_array_value: vec!["one".into()],
        float_array_value: vec![1.25],
    };
    assert_eq!(value_from_property(&property).unwrap(), json!(i64::MAX));
    assert_eq!(
        json_from_property(&property).unwrap(),
        json!({"id": "x", "type": "Int", "value": i64::MAX})
    );
    property.property_type = PropertyType::Bool;
    assert_eq!(value_from_property(&property).unwrap(), json!(true));
    property.property_type = PropertyType::StringArray;
    assert_eq!(value_from_property(&property).unwrap(), json!(["one"]));
    for property_type in [
        PropertyType::Count,
        PropertyType::Unknown(99),
        PropertyType::Unknown(-1),
    ] {
        property.property_type = property_type;
        assert_eq!(
            value_from_property(&property),
            Err(ValueError::UnsupportedType(property_type.number()))
        );
        assert_eq!(
            json_from_property(&property),
            Err(ValueError::UnsupportedType(property_type.number()))
        );
    }
}

#[test]
fn float_reply_dump_widens_finite_and_emits_null_for_nonfinite_values() {
    let mut property = Property {
        property_type: PropertyType::Float,
        float_value: Some(-0.0),
        ..Property::default()
    };
    assert_eq!(
        value_from_property(&property)
            .unwrap()
            .as_f64()
            .unwrap()
            .to_bits(),
        (-0.0f64).to_bits()
    );
    property.float_value = Some(0.1);
    assert_eq!(
        value_from_property(&property).unwrap().as_f64(),
        Some(0.10000000149011612)
    );
    for value in [
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(0x7fc0_0042),
    ] {
        property.float_value = Some(value);
        assert_eq!(value_from_property(&property).unwrap(), Value::Null);
    }
    property.property_type = PropertyType::FloatArray;
    property.float_array_value = vec![1.25, f32::NAN, f32::INFINITY, -0.0];
    let value = value_from_property(&property).unwrap();
    assert_eq!(value[0], json!(1.25));
    assert!(value[1].is_null());
    assert!(value[2].is_null());
    assert_eq!(value[3].as_f64().unwrap().to_bits(), (-0.0f64).to_bits());
}

#[test]
fn rust_boundaries_reject_excess_arrays_and_messages_before_transport() {
    let request = request(
        "FloatArray",
        Value::Array(vec![json!(0); MAX_ARRAY_ELEMENTS + 1]),
    );
    assert_eq!(property_from_request(&request), Err(ValueError::ArrayLimit));
    let oversized = ThxPropertyRequest {
        id: "x".repeat(crate::thx_v4_proto::MAX_MESSAGE_BYTES + 1),
        value: Value::Null,
        property_type: ThxPropertyType::None,
    };
    assert!(matches!(
        property_from_request(&oversized),
        Err(ValueError::Codec(CodecError::SizeLimit { .. }))
    ));
}
