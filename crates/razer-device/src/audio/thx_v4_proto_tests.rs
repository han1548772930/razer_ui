use super::*;
use serde_json::Value;

fn property() -> Property {
    Property {
        id: "SpatialEnabled".into(),
        property_type: PropertyType::Bool,
        float_value: Some(0.),
        int_value: Some(-55),
        bool_value: Some(false),
        string_value: Some(String::new()),
        string_array_value: vec![String::new(), "headphones".into()],
        float_array_value: vec![0., 1.25, -0., -2.5],
    }
}
fn definition() -> PropertyDefinition {
    PropertyDefinition {
        id: "SpatialEnabled".into(),
        property_type: PropertyType::Bool,
        bool_traits: Some(BoolPropertyTraits {
            default_value: false,
        }),
        float_traits: Some(FloatPropertyTraits {
            min_value: -2.5,
            max_value: 2.5,
            step: 0.5,
            default_value: 1.,
        }),
        int_traits: Some(IntPropertyTraits {
            min_value: -55,
            max_value: 55,
            default_value: -1,
        }),
        string_traits: Some(StringPropertyTraits {
            default_value: "headphones".into(),
            allowed_values: vec![String::new(), "headphones".into()],
        }),
        string_array_traits: Some(StringArrayPropertyTraits {
            size: 2,
            default_value: vec![String::new(), "x".into()],
            allowed_values: vec!["x".into(), "y".into()],
        }),
        float_array_traits: Some(FloatArrayPropertyTraits {
            min_value: -2.5,
            max_value: 2.5,
            step: 0.5,
            size: 4,
            default_value: vec![0., 1.25, -0., -2.5],
        }),
    }
}
fn requests() -> Vec<Request> {
    let endpoint_id = Some(String::new());
    vec![
        Request::GetEndpointList {
            endpoint_type: "game".into(),
        },
        Request::GetPropertyList {
            endpoint_id: endpoint_id.clone(),
        },
        Request::GetPropertyDefinition {
            id: "SpatialEnabled".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::GetProperty {
            id: "SpatialEnabled".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::SetProperty {
            property: Some(property()),
            endpoint_id: endpoint_id.clone(),
        },
        Request::SubscribeToPropertyChanges {
            endpoint_id: endpoint_id.clone(),
        },
        Request::GetPresetList {
            preset_group_id: "spatial".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::SelectPreset {
            preset_group_id: "spatial".into(),
            preset_id: "p1".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::GetSelectedPreset {
            preset_group_id: "spatial".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::SavePreset {
            preset_group_id: "spatial".into(),
            preset_id: "p1".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::ResetPreset {
            preset_group_id: "spatial".into(),
            preset_id: "p1".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::AddPreset {
            preset_group_id: "spatial".into(),
            source_preset_id: "original".into(),
            preset_id: "p1".into(),
            endpoint_id: endpoint_id.clone(),
            description: Some(String::new()),
        },
        Request::RemovePreset {
            preset_group_id: "spatial".into(),
            preset_id: "p1".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::GetPresetDescription {
            preset_group_id: "spatial".into(),
            preset_id: "p1".into(),
            endpoint_id: endpoint_id.clone(),
        },
        Request::SubscribeToPresetChanges { endpoint_id },
        Request::GetVersion,
    ]
}
fn responses() -> Vec<Response> {
    vec![
        Response::GetEndpointList {
            endpoint_ids: vec!["headphones_game".into(), String::new(), "speakers".into()],
        },
        Response::GetPropertyList {
            properties: vec!["SpatialEnabled".into(), "Eq".into()],
        },
        Response::GetPropertyDefinition(definition()),
        Response::GetProperty(property()),
        Response::SetProperty,
        Response::SubscribeToPropertyChanges(property()),
        Response::GetPresetList {
            preset_ids: vec!["p1".into(), String::new()],
        },
        Response::SelectPreset,
        Response::GetSelectedPreset {
            preset_group_id: "spatial".into(),
            preset_id: "p1".into(),
        },
        Response::SavePreset,
        Response::ResetPreset,
        Response::AddPreset {
            preset_ids: vec!["p1".into(), String::new()],
        },
        Response::RemovePreset {
            preset_ids: vec!["p1".into(), String::new()],
        },
        Response::GetPresetDescription {
            description: String::new(),
        },
        Response::SubscribeToPresetChanges(PresetChange {
            event: PresetEvent::Selected,
            preset_group_id: "spatial".into(),
            preset_id: "p1".into(),
        }),
        Response::GetVersion {
            version: "4.0".into(),
        },
    ]
}
fn golden(method: Method, direction: &str) -> Vec<u8> {
    let data: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/thx-v4-protobuf-goldens-current.json"
    ))
    .unwrap();
    let descriptor: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/thx-v4-protobuf-current.json"
    ))
    .unwrap();
    assert_eq!(
        data["descriptor_sha256"],
        descriptor["descriptor"]["span"]["sha256"]
    );
    let name = method.path().rsplit('/').next().unwrap();
    let record = data["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["method"] == name && r["direction"] == direction)
        .unwrap();
    let text = record["hex"].as_str().unwrap();
    text.as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn ok_trailers() -> GrpcTrailers {
    GrpcTrailers {
        status: "0".into(),
        message: None,
    }
}

#[test]
fn all_sixteen_method_requests_match_independent_descriptor_protobuf_goldens() {
    for (method, request) in Method::ALL.into_iter().zip(requests()) {
        assert_eq!(request.method(), method);
        let expected = golden(method, "request");
        assert_eq!(request.encode().unwrap(), expected, "{}", method.path());
        assert_eq!(Request::decode(method, &expected).unwrap(), request);
        assert_eq!(
            request.encode_framed().unwrap(),
            encode_grpc_frame(&expected).unwrap()
        );
    }
}
#[test]
fn all_sixteen_responses_and_notifications_match_independent_descriptor_goldens() {
    for (method, response) in Method::ALL.into_iter().zip(responses()) {
        let expected = golden(method, "response");
        assert_eq!(response.encode().unwrap(), expected, "{}", method.path());
        assert_eq!(Response::decode(method, &expected).unwrap(), response);
    }
}
#[test]
fn recovered_path_literals_and_streaming_flags_match_the_saved_descriptor() {
    let evidence: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/thx-v4-protobuf-current.json"
    ))
    .unwrap();
    let methods = evidence["methods"].as_array().unwrap();
    assert_eq!(methods.len(), Method::ALL.len());
    for method in Method::ALL {
        let original = methods
            .iter()
            .find(|m| m["full_path"] == method.path())
            .unwrap();
        assert_eq!(original["server_streaming"], method.server_streaming());
        assert_eq!(original["client_streaming"], false);
        assert_eq!(Method::from_path(method.path()), Some(method));
    }
    assert!(Method::from_path("/thx.proto.ThxAudioProperties/Missing").is_none());
    assert_eq!(
        Method::ALL
            .into_iter()
            .filter(|m| !m.server_streaming())
            .count(),
        14
    );
}
#[test]
fn optional_false_empty_string_zero_and_missing_remain_distinct() {
    let present = Property {
        bool_value: Some(false),
        string_value: Some(String::new()),
        int_value: Some(0),
        float_value: Some(0.),
        ..Default::default()
    };
    assert_eq!(
        present.encode().unwrap(),
        [0x1d, 0, 0, 0, 0, 0x20, 0, 0x28, 0, 0x32, 0]
    );
    assert_eq!(
        Property::decode(&present.encode().unwrap()).unwrap(),
        present
    );
    assert_ne!(present, Property::decode(&[]).unwrap());
    assert_eq!(
        Request::GetPropertyList {
            endpoint_id: Some(String::new())
        }
        .encode()
        .unwrap(),
        [0x0a, 0]
    );
    assert!(
        Request::GetPropertyList { endpoint_id: None }
            .encode()
            .unwrap()
            .is_empty()
    );
    let empty_message = Request::SetProperty {
        property: Some(Property::default()),
        endpoint_id: Some(String::new()),
    };
    assert_eq!(empty_message.encode().unwrap(), [0x0a, 0, 0x12, 0]);
    assert_eq!(
        Request::decode(Method::SetProperty, &[0x0a, 0, 0x12, 0]).unwrap(),
        empty_message
    );
    assert!(
        Request::SetProperty {
            property: None,
            endpoint_id: None
        }
        .encode()
        .unwrap()
        .is_empty()
    );
}
#[test]
fn signed_int64_values_use_twos_complement_varints_with_no_unsigned_normalization() {
    for value in [i64::MIN, -55, -1, 0, 55, i64::MAX] {
        let p = Property {
            int_value: Some(value),
            ..Default::default()
        };
        assert_eq!(
            Property::decode(&p.encode().unwrap()).unwrap().int_value,
            Some(value)
        );
    }
    assert_eq!(
        Property {
            int_value: Some(-1),
            ..Default::default()
        }
        .encode()
        .unwrap(),
        [
            0x20, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1
        ]
    );
}
#[test]
fn full_property_and_nested_event_enums_keep_unknown_numeric_values() {
    for n in [0, 1, 2, 3, 4, 5, 6, 7, 99, -55] {
        let kind = PropertyType::from_number(n);
        assert_eq!(kind.number(), n);
        let p = Property {
            property_type: kind,
            ..Default::default()
        };
        assert_eq!(
            Property::decode(&p.encode().unwrap())
                .unwrap()
                .property_type,
            kind
        );
    }
    for n in [0, 1, 2, 99, -1] {
        let event = PresetEvent::from_number(n);
        assert_eq!(
            PresetChange::decode(
                &PresetChange {
                    event,
                    ..Default::default()
                }
                .encode()
                .unwrap()
            )
            .unwrap()
            .event,
            event
        );
    }
}
#[test]
fn packed_and_unpacked_float_arrays_merge_and_preserve_negative_zero_and_nan_bits() {
    let p = Property {
        float_value: Some(f32::from_bits(0x7fc0_0042)),
        float_array_value: vec![-0., 1.25],
        ..Default::default()
    };
    let mut data = p.encode().unwrap();
    data.extend_from_slice(&[0x45, 0, 0, 0x20, 0xc0]); // unpacked field 8 = -2.5
    let result = Property::decode(&data).unwrap();
    assert_eq!(result.float_value.unwrap().to_bits(), 0x7fc0_0042);
    assert_eq!(
        result
            .float_array_value
            .iter()
            .map(|n| n.to_bits())
            .collect::<Vec<_>>(),
        [-0f32, 1.25, -2.5].map(f32::to_bits)
    );
}
#[test]
fn repeated_nested_messages_merge_and_singular_scalars_use_the_last_occurrence() {
    // Two Property occurrences: first owns id, second owns Bool/false fields.
    let data = [0x0a, 3, 0x0a, 1, b'x', 0x0a, 4, 0x10, 1, 0x28, 0];
    let Request::SetProperty {
        property: Some(p), ..
    } = Request::decode(Method::SetProperty, &data).unwrap()
    else {
        panic!()
    };
    assert_eq!(p.observed_bool("x").unwrap(), false);
    let p = Property::decode(&[0x0a, 1, b'x', 0x0a, 1, b'y', 0x28, 1, 0x28, 0]).unwrap();
    assert_eq!(p.id, "y");
    assert_eq!(p.bool_value, Some(false));
}
#[test]
fn all_unknown_wire_kinds_are_skipped_without_becoming_property_fields() {
    let mut data = property().encode().unwrap();
    data.extend_from_slice(&[
        0xa0, 6, 1, // unknown 100 varint
        0xa1, 6, 1, 2, 3, 4, 5, 6, 7, 8, // fixed64
        0xa2, 6, 3, b'x', b'y', b'z', // bytes
        0xa3, 6, 0x08, 1, 0xa4, 6, // group
        0xa5, 6, 1, 2, 3, 4, // fixed32
    ]);
    assert_eq!(Property::decode(&data).unwrap(), property());
    assert_eq!(
        Response::decode(Method::SetProperty, &[0xa0, 6, 1]).unwrap(),
        Response::SetProperty
    );
}
#[test]
fn malformed_varints_tags_lengths_utf8_groups_and_packed_arrays_are_errors() {
    for data in [
        vec![0],
        vec![0x0e],
        vec![0x0a, 1, 0xff],
        vec![0x1d, 1, 2, 3],
        vec![0x42, 3, 1, 2, 3],
        vec![0xa3, 6, 0xac, 6],
        vec![0xa4, 6],
        vec![0xa2, 6, 0x80],
        vec![
            0x20, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 2,
        ],
        vec![0xff; 10],
    ] {
        assert!(Property::decode(&data).is_err(), "{data:?}");
    }
    assert!(matches!(
        Property::decode(&[0x0a, 1, 0xff]),
        Err(CodecError::InvalidUtf8)
    ));
    assert!(matches!(
        Property::decode(&[0x42, 3, 1, 2, 3]),
        Err(CodecError::InvalidPackedFloat)
    ));
    assert!(matches!(
        Property::decode(&[0x18, 1]),
        Err(CodecError::WrongWireType { field: 3, .. })
    ));
}
#[test]
fn nested_unknown_groups_and_repeated_fields_have_explicit_bounds() {
    let data = [
        vec![0xa3, 6].repeat(MAX_GROUP_DEPTH + 1),
        vec![0xa4, 6].repeat(MAX_GROUP_DEPTH + 1),
    ]
    .concat();
    assert_eq!(Property::decode(&data), Err(CodecError::GroupDepthLimit));
    let data = vec![0xa0, 6, 1].repeat(MAX_FIELDS + 1);
    assert_eq!(Property::decode(&data), Err(CodecError::FieldLimit));
    let values = vec![0.; MAX_FIELDS + 1];
    assert_eq!(
        Property {
            float_array_value: values,
            ..Default::default()
        }
        .encode(),
        Err(CodecError::FieldLimit)
    );
    let mut writer = Writer::default();
    writer.blob(8, &vec![0; (MAX_FIELDS + 1) * 4]).unwrap();
    assert_eq!(Property::decode(&writer.bytes), Err(CodecError::FieldLimit));
}
#[test]
fn message_size_limit_is_checked_before_parsing_or_encoding_excess() {
    let bytes = vec![0; MAX_MESSAGE_BYTES + 1];
    assert!(matches!(
        Property::decode(&bytes),
        Err(CodecError::SizeLimit { .. })
    ));
    assert!(matches!(
        encode_grpc_frame(&bytes),
        Err(CodecError::SizeLimit { .. })
    ));
    assert!(matches!(
        Property {
            id: "x".repeat(MAX_MESSAGE_BYTES + 1),
            ..Default::default()
        }
        .encode(),
        Err(CodecError::SizeLimit { .. })
    ));
}
#[test]
fn property_observation_checks_id_type_and_presence_before_accepting_false() {
    assert_eq!(property().observed_bool("SpatialEnabled").unwrap(), false);
    assert_eq!(
        property().observed_bool("different"),
        Err(CodecError::PropertyIdMismatch)
    );
    let mut p = property();
    p.property_type = PropertyType::Int;
    assert_eq!(
        p.observed_bool("SpatialEnabled"),
        Err(CodecError::PropertyTypeMismatch)
    );
    p.property_type = PropertyType::Bool;
    p.bool_value = None;
    assert_eq!(
        p.observed_bool("SpatialEnabled"),
        Err(CodecError::MissingPropertyValue)
    );
    assert_eq!(
        Property::default().observed_bool("SpatialEnabled"),
        Err(CodecError::PropertyIdMismatch)
    );
}
#[test]
fn grpc_five_byte_header_and_every_fragment_boundary_reassemble_one_body() {
    let body = golden(Method::GetProperty, "response");
    let frame = encode_grpc_frame(&body).unwrap();
    assert_eq!(frame[0], 0);
    assert_eq!(
        u32::from_be_bytes(frame[1..5].try_into().unwrap()) as usize,
        body.len()
    );
    for boundary in 0..=frame.len() {
        let mut decoder = GrpcFrameDecoder::new();
        let mut bodies = decoder.push(&frame[..boundary]).unwrap();
        bodies.extend(decoder.push(&frame[boundary..]).unwrap());
        decoder.finish().unwrap();
        assert_eq!(bodies, [body.clone()]);
    }
    let mut unary = UnaryGrpcResponse::new();
    for byte in frame {
        unary.push(&[byte]).unwrap();
    }
    assert_eq!(
        unary
            .finish(
                Method::GetProperty,
                200,
                Some("application/grpc"),
                Some(&ok_trailers())
            )
            .unwrap(),
        Response::GetProperty(property())
    );
}
#[test]
fn streaming_frames_decode_notifications_but_unary_rejects_multiple_frames() {
    let body = golden(Method::SubscribeToPropertyChanges, "response");
    let frame = encode_grpc_frame(&body).unwrap();
    let both = [frame.clone(), frame].concat();
    let mut decoder = GrpcFrameDecoder::new();
    let bodies = decoder.push(&both).unwrap();
    decoder.finish().unwrap();
    assert_eq!(bodies.len(), 2);
    for body in bodies {
        assert_eq!(
            Response::decode(Method::SubscribeToPropertyChanges, &body).unwrap(),
            Response::SubscribeToPropertyChanges(property())
        );
    }
    let mut unary = UnaryGrpcResponse::new();
    assert_eq!(unary.push(&both), Err(CodecError::UnaryFrameCount(2)));
    assert!(matches!(
        unary.finish(
            Method::GetProperty,
            200,
            Some("application/grpc"),
            Some(&ok_trailers())
        ),
        Err(CodecError::UnaryFrameCount(2))
    ));
    assert!(matches!(
        complete_unary(
            Method::SubscribeToPropertyChanges,
            200,
            Some("application/grpc"),
            &[body],
            Some(&ok_trailers())
        ),
        Err(CodecError::NotUnary)
    ));
}
#[test]
fn empty_success_requires_one_zero_length_message_and_final_success_trailers() {
    let mut unary = UnaryGrpcResponse::new();
    unary.push(&[0, 0, 0, 0, 0]).unwrap();
    assert_eq!(
        unary
            .finish(
                Method::SetProperty,
                200,
                Some("application/grpc+proto; charset=utf-8"),
                Some(&ok_trailers())
            )
            .unwrap(),
        Response::SetProperty
    );
    assert_eq!(
        UnaryGrpcResponse::new().finish(
            Method::SetProperty,
            200,
            Some("application/grpc"),
            Some(&ok_trailers())
        ),
        Err(CodecError::UnaryFrameCount(0))
    );
    let mut unary = UnaryGrpcResponse::new();
    unary.push(&[0, 0, 0, 0, 0]).unwrap();
    assert_eq!(
        unary.finish(Method::SetProperty, 200, Some("application/grpc"), None),
        Err(CodecError::MissingTrailers)
    );
}
#[test]
fn partial_header_body_or_extra_partial_frame_never_complete_unary_success() {
    for bytes in [
        vec![0],
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0, 2, 0],
        vec![0, 0, 0, 0, 0, 0],
    ] {
        let mut unary = UnaryGrpcResponse::new();
        unary.push(&bytes).unwrap();
        assert_eq!(
            unary.finish(
                Method::SetProperty,
                200,
                Some("application/grpc"),
                Some(&ok_trailers())
            ),
            Err(CodecError::Truncated)
        );
    }
}
#[test]
fn compressed_invalid_flag_and_oversize_headers_are_explicit_failure() {
    for (bytes, expected) in [
        ([1, 0, 0, 0, 0], CodecError::CompressionUnsupported),
        ([2, 0, 0, 0, 0], CodecError::InvalidGrpcFlag(2)),
    ] {
        let mut decoder = GrpcFrameDecoder::new();
        assert_eq!(decoder.push(&bytes), Err(expected));
        assert_eq!(decoder.push(&[0, 0, 0, 0, 0]), Err(CodecError::Truncated));
        assert_eq!(decoder.finish(), Err(CodecError::Truncated));
    }
    let mut decoder = GrpcFrameDecoder::new();
    assert!(matches!(
        decoder.push(&[0, 255, 255, 255, 255]),
        Err(CodecError::SizeLimit { .. })
    ));
}
#[test]
fn http_grpc_status_and_content_type_failures_never_acknowledge_a_write() {
    let body = [Vec::new()];
    let failed = GrpcTrailers {
        status: "14".into(),
        message: Some("service unavailable".into()),
    };
    assert_eq!(
        complete_unary(
            Method::SetProperty,
            200,
            Some("application/grpc"),
            &body,
            Some(&failed)
        ),
        Err(CodecError::GrpcStatus {
            code: 14,
            message: failed.message.clone()
        })
    );
    assert_eq!(
        complete_unary(
            Method::SetProperty,
            500,
            Some("application/grpc"),
            &body,
            Some(&ok_trailers())
        ),
        Err(CodecError::HttpStatus(500))
    );
    for content_type in [
        None,
        Some("application/json"),
        Some("application/grpc+json"),
    ] {
        assert_eq!(
            complete_unary(
                Method::SetProperty,
                200,
                content_type,
                &body,
                Some(&ok_trailers())
            ),
            Err(CodecError::InvalidContentType)
        );
    }
    for code in ["", "+0", "-0", " 0", "0x0", "4294967296"] {
        assert_eq!(
            GrpcTrailers {
                status: code.into(),
                message: None
            }
            .verify_success(),
            Err(CodecError::InvalidGrpcStatus)
        );
    }
}
#[test]
fn bounded_arbitrary_protobuf_and_grpc_bytes_do_not_panic() {
    let mut seed = 0x12345678u32;
    for length in 0..64 {
        for _ in 0..16 {
            let data = (0..length)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    (seed >> 24) as u8
                })
                .collect::<Vec<_>>();
            let outcome = std::panic::catch_unwind(|| {
                for method in Method::ALL {
                    let _ = Request::decode(method, &data);
                    let _ = Response::decode(method, &data);
                }
                let mut decoder = GrpcFrameDecoder::new();
                let _ = decoder.push(&data);
                let _ = decoder.finish();
            });
            assert!(outcome.is_ok(), "{data:?}");
        }
    }
}
