use super::*;

fn batch(container: &str, product_id: u32, rows: &[(f64, f64)]) -> InputBatch {
    InputBatch {
        product_id,
        container_id: container.into(),
        movement: rows
            .iter()
            .map(|&(time_tick, x)| Movement { x, time_tick })
            .collect(),
    }
}

#[test]
fn dpi_matcher_nested_callback_requires_movement_fields_and_ignores_other_input() {
    let callback = json!({
        "productId":190,"containerId":"main",
        "jsonEvent":r#"[{"data":"{\"type\":\"mouseButton\"}"},{"data":"{\"type\":\"mouseMove\",\"x\":12}","timeTick":"41"}]"#
    });
    let parsed = InputBatch::from_callback(&callback).unwrap();
    assert_eq!(parsed.movement.len(), 1);
    assert_eq!(parsed.movement[0].x, 12.);
    assert_eq!(parsed.movement[0].time_tick, 41.);
    for text in [
        r#"[{"data":"{\"type\":\"mouseMove\"}","timeTick":1}]"#,
        r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}"}]"#,
        r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}","timeTick":"NaN"}]"#,
    ] {
        assert!(InputBatch::from_event(190, "main", text).is_err());
    }
}

#[test]
fn dpi_matcher_distance_uses_positive_overlapping_ticks_and_retains_container_classification() {
    let mut session = Session::default();
    session.begin(800.).unwrap();
    session.acquired_lock().unwrap();
    assert!(
        !session
            .ingest(batch("source", 123, &[(20., 1000.)]), false, 0)
            .unwrap()
    );
    assert!(
        !session
            .ingest(
                batch("main", 190, &[(10., 1000.), (20., 2000.), (30., 2500.)]),
                false,
                1
            )
            .unwrap()
    );
    assert!(
        !session
            .ingest(batch("source", 123, &[(30., 1250.)]), true, 2)
            .unwrap()
    );
    assert!(
        session
            .ingest(batch("main", 190, &[(40., -100.)]), false, 3)
            .unwrap()
    );
    // Current U: only the overlapping interval [20,30] contributes. The
    // measured source/main ratio is 2250/4500, against an observed 800 DPI.
    assert_eq!(session.new_dpi(), Some(400.));
    assert_eq!(session.source_container(), "source");
    assert_eq!(session.source_product_id, Some(123));
    assert_eq!(session.response()["payload"]["state"], "completed");
}

#[test]
fn dpi_matcher_timeout_arms_each_stream_on_its_callback_after_both_streams_appear() {
    let mut session = Session::default();
    session.begin(800.).unwrap();
    session.acquired_lock().unwrap();
    session
        .ingest(batch("main", 190, &[(1., 1.)]), true, 0)
        .unwrap();
    assert!(!session.check_timeout(10_000));
    session
        .ingest(batch("source", 123, &[]), false, 10_000)
        .unwrap();
    assert!(!session.check_timeout(10_499));
    session
        .ingest(batch("main", 190, &[]), true, 10_400)
        .unwrap();
    assert!(session.check_timeout(10_500));
    assert_eq!(
        session.response(),
        json!({"type":"MW_SET_DPI_MATCHER_STATE","payload":{
            "state":"error","process":0.,"newDpi":0.,"currentDpi":0.
        }})
    );
    assert!(session.new_dpi().is_none());
    session.reset();
    assert!(!session.check_timeout(20_000));
}
