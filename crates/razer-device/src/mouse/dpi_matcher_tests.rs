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
fn dpi_matcher_final_buffer_consumption_updates_progress_without_another_callback() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
        session.begin(800.).unwrap();
        session.acquired_lock().unwrap();
        assert!(
            !session
                .ingest(batch("source", 123, &[(1., 100.)]), false, 0)
                .unwrap()
        );
        assert!(
            !session
                .ingest(batch("main", product_id, &[(1., 2000.)]), true, 1)
                .unwrap()
        );
        assert_eq!(session.response().process(), 50.);
        assert!(
            !session
                .ingest(batch("main", product_id, &[(2., -100.)]), true, 2)
                .unwrap()
        );
        assert_eq!(session.response().process(), 50.);
        assert!(
            !session
                .ingest(batch("main", product_id, &[(3., 2500.)]), true, 3)
                .unwrap()
        );
        assert_eq!(session.response().process(), 99.);
        assert_eq!(session.state, State::Calibrating);
        assert!(session.new_dpi().is_none());
        assert!(
            !session
                .ingest(batch("source", 123, &[(4., 200.)]), false, 4)
                .unwrap()
        );
        assert!(
            session
                .ingest(batch("main", product_id, &[(5., 0.)]), true, 5)
                .unwrap()
        );
        assert_eq!(session.state, State::Stopping);
        assert!(session.new_dpi().is_none());
        session.complete_after_stop().unwrap();
        assert_eq!(session.new_dpi(), Some(53.));
    }
}

#[test]
fn dpi_matcher_progress_includes_the_short_final_chunk_after_hundred_row_chunks() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
        session.begin(800.).unwrap();
        session.acquired_lock().unwrap();
        session
            .ingest(batch("source", 123, &[(0., 100.)]), false, 0)
            .unwrap();
        let rows = (1..=205).map(|tick| (tick as f64, 10.)).collect::<Vec<_>>();
        assert!(
            !session
                .ingest(batch("main", product_id, &rows), true, 1)
                .unwrap()
        );
        // Source k/w consumes 100, 100, 5 rows, then schedules the publication
        // pass even with an empty buffer: round(2050 / 4000 * 100) = 51.
        assert_eq!(session.response().process(), 51.);
        assert_eq!(session.state, State::Calibrating);
        assert!(session.new_dpi().is_none());
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
    assert_eq!(session.state, State::Stopping);
    assert_eq!(session.new_dpi(), None);
    assert_eq!(session.response().phase(), Phase::Calibrating);
    session.complete_after_stop().unwrap();
    // Current U: only the overlapping interval [20,30] contributes. The
    // measured source/main ratio is 2250/4500, against an observed 800 DPI.
    assert_eq!(session.new_dpi(), Some(400.));
    assert_eq!(session.source_container(), "source");
    assert_eq!(session.source_product_id, Some(123));
    assert_eq!(session.response().phase(), Phase::Completed);
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
        Calibration::new(Phase::Error, 0., 0., 0.)
    );
    assert!(session.new_dpi().is_none());
    session.reset();
    assert!(!session.check_timeout(20_000));
}

#[test]
fn dpi_matcher_all_mounted_products_wait_for_stop_before_publishing_the_result() {
    // The six reviewed k/w and U owners use the same distance calculation and
    // call U only from the continuation after stopMouseInputMonitoring.
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
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
                    batch(
                        "main",
                        product_id,
                        &[(10., 1000.), (20., 2000.), (30., 2500.)]
                    ),
                    true,
                    1,
                )
                .unwrap()
        );
        assert!(
            !session
                .ingest(batch("source", 123, &[(30., 1250.)]), false, 2)
                .unwrap()
        );
        assert!(
            session
                .ingest(batch("main", product_id, &[(40., -100.)]), true, 3)
                .unwrap()
        );
        assert_eq!(session.state, State::Stopping, "product {product_id}");
        assert_eq!(session.new_dpi(), None);
        assert_eq!(session.response().new_dpi(), 0.);
        assert!(!session.check_timeout(50_000));
        assert!(
            session
                .ingest(batch("late", 123, &[(40., 99999.)]), false, 4)
                .is_err()
        );
        session.complete_after_stop().unwrap();
        assert_eq!(session.new_dpi(), Some(400.));
        assert!(session.complete_after_stop().is_err());
    }
}

#[test]
fn dpi_matcher_pid226_keeps_product_partition_through_reset_and_timeout() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
        for cycle in 0..3 {
            session.begin(800.).unwrap();
            session.acquired_lock().unwrap();
            session
                .ingest(batch("reference", 123, &[(1., 1.)]), false, 0)
                .unwrap();
            session
                .ingest(batch("reference", product_id, &[(2., 1.)]), true, 1)
                .unwrap();
            // R(pid,container) keeps this callback in the known reference
            // stream; PID226 R(pid) moves it to main even with that container.
            let expected = if product_id == 226 { 123 } else { product_id };
            assert_eq!(session.source_product_id, Some(expected));
            if cycle == 0 {
                session.reset();
            } else {
                session
                    .ingest(batch("different", 123, &[]), false, 2)
                    .unwrap();
                assert!(session.check_timeout(502));
                assert_eq!(session.state, State::Error);
            }
        }
    }
}

#[test]
fn dpi_matcher_only_audited_logical_products_select_a_sampling_policy() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        assert_eq!(Product::new(product_id).unwrap().product_id(), product_id);
    }
    for product_id in [0, 70, 179, 190 + 1, 193, 196, 223, 227, 228, 230, 240, 241] {
        assert!(Product::new(product_id).is_err());
    }
}

#[test]
fn dpi_matcher_initial_main_identities_follow_each_logical_factory() {
    for (product_id, wired, dongle, ble) in [
        (190, 190, 191, None),
        (192, 192, 193, None),
        (222, 222, 223, None),
        (226, 226, 227, Some(228)),
        (229, 229, 230, None),
        (239, 239, 240, None),
    ] {
        let product = Product::new(product_id).unwrap();
        assert!(product.accepts_initial_main_product(wired));
        assert!(product.accepts_initial_main_product(dongle));
        if let Some(ble) = ble {
            assert!(product.accepts_initial_main_product(ble));
        }
        for foreign in [70, 164, 241] {
            assert!(!product.accepts_initial_main_product(foreign));
        }
        assert_eq!(product.is_dual_link_reference(241), product_id == 226);
    }
}
