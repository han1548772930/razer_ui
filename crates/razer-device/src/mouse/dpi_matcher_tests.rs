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
        r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}","timeTick":true}]"#,
        r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}","timeTick":null}]"#,
        r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}","timeTick":[]}]"#,
    ] {
        assert!(InputBatch::from_event(190, "main", text).is_err());
    }
}

#[test]
fn dpi_matcher_callback_string_ticks_follow_source_unary_plus() {
    for (text, expected) in [
        ("", 0.),
        (" \t\r\n\u{feff}\u{3000}", 0.),
        ("\u{feff} +1.5e1 \u{2029}", 15.),
        ("-0", -0.),
        ("0x14", 20.),
        ("0B10100", 20.),
        ("0o24", 20.),
        ("Infinity", f64::INFINITY),
        ("+Infinity", f64::INFINITY),
        ("-Infinity", f64::NEG_INFINITY),
        ("1e400", f64::INFINITY),
        ("-1e-4000", -0.),
        ("not-a-number", f64::NAN),
        ("inf", f64::NAN),
        ("infinity", f64::NAN),
        ("-0x14", f64::NAN),
        ("1_000", f64::NAN),
        ("\u{0085}1", f64::NAN),
    ] {
        let event = json!([{
            "timeTick":text,
            "data":r#"{"type":"mouseMove","x":2}"#,
        }]);
        let input = InputBatch::from_event(190, "main", &event.to_string())
            .unwrap_or_else(|error| panic!("{text:?}: {error:#}"));
        assert_eq!(input.movement.len(), 1);
        assert_source_number(input.movement[0].time_tick, expected);
    }
}

fn string_batch(container: &str, product_id: u32, rows: &[(&str, f64)]) -> InputBatch {
    let events: Vec<_> = rows
        .iter()
        .map(|(tick, x)| {
            json!({
                "timeTick":tick,
                "data":json!({"type":"mouseMove","x":x}).to_string(),
            })
        })
        .collect();
    InputBatch::from_event(product_id, container, &json!(events).to_string()).unwrap()
}

#[test]
fn dpi_matcher_string_tick_conversion_reaches_the_six_source_results_after_stop() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        for (main, source, final_main) in [
            (
                &[("1e1", 1000.), ("0x14", 2000.), ("0o36", 2500.)][..],
                &[("0b10100", 1000.), ("30", 1250.)][..],
                ("4e1", -100.),
            ),
            (
                &[("", 4500.)][..],
                &[(" \u{feff}\u{3000}", 2250.)][..],
                ("1", 0.),
            ),
            (&[("-0", 4500.)][..], &[("0", 2250.)][..], ("1", 0.)),
            (
                &[("not-a-number", 1000.), ("0", 1000.), ("1", 4500.)][..],
                &[("1", 3250.)][..],
                ("2", 0.),
            ),
            (
                &[("Infinity", 4500.)][..],
                &[("+Infinity", 2250.)][..],
                ("1e400", 0.),
            ),
            (
                &[("-Infinity", 4500.)][..],
                &[("-1e400", 2250.)][..],
                ("-Infinity", 0.),
            ),
        ] {
            let mut session = Session::new(Product::new(product_id).unwrap());
            session.begin(800.).unwrap();
            session.acquired_lock().unwrap();
            assert!(
                !session
                    .ingest(string_batch("source", 123, source), false, 0)
                    .unwrap()
            );
            assert!(
                !session
                    .ingest(string_batch("main", product_id, main), true, 1)
                    .unwrap()
            );
            assert_eq!(session.response().process(), 99.);
            assert!(
                session
                    .ingest(string_batch("main", product_id, &[final_main]), true, 2)
                    .unwrap()
            );
            assert_eq!(session.state, State::Stopping);
            assert!(session.new_dpi().is_none());
            session.complete_after_stop().unwrap();
            assert_eq!(session.new_dpi(), Some(400.), "{product_id}: {main:?}");
        }
    }
}

#[test]
fn dpi_matcher_nan_reference_tick_does_not_fabricate_stop_or_completion() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
        session.begin(800.).unwrap();
        session.acquired_lock().unwrap();
        session
            .ingest(string_batch("source", 123, &[("invalid", 2250.)]), false, 0)
            .unwrap();
        for (received, tick, x) in [(1, "1", 4500.), (2, "2", 0.)] {
            assert!(
                !session
                    .ingest(
                        string_batch("main", product_id, &[(tick, x)]),
                        true,
                        received
                    )
                    .unwrap()
            );
        }
        assert_eq!(session.response().process(), 99.);
        assert_eq!(session.state, State::Calibrating);
        assert!(session.new_dpi().is_none());
        assert!(session.complete_after_stop().is_err());
        assert!(session.check_timeout(502));
        assert_eq!(session.state, State::Error);
        assert!(session.new_dpi().is_none());
        session.reset();
        assert_eq!(session.product.product_id(), product_id);
    }
}

#[test]
fn dpi_matcher_callback_keeps_nonfinite_numeric_ticks_as_source_numbers() {
    for (text, expected) in [
        (
            r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}","timeTick":"NaN"}]"#,
            f64::NAN,
        ),
        (
            r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}","timeTick":"Infinity"}]"#,
            f64::INFINITY,
        ),
        (
            r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2}","timeTick":"-Infinity"}]"#,
            f64::NEG_INFINITY,
        ),
    ] {
        let input = InputBatch::from_event(190, "main", text).unwrap();
        assert_eq!(input.movement.len(), 1);
        assert_eq!(input.movement[0].x, 2.);
        assert_source_number(input.movement[0].time_tick, expected);
    }
}

#[test]
fn dpi_matcher_begin_preserves_source_baselines_without_extra_numeric_guards() {
    for current_dpi in [
        0.,
        -0.,
        -800.,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
    ] {
        let mut session = Session::default();
        assert!(
            session.begin(current_dpi).is_ok(),
            "baseline {current_dpi:?} was rejected"
        );
        assert_source_number(session.response().current_dpi().unwrap(), current_dpi);
        assert_eq!(session.state, State::RequestingLock);
        assert!(session.complete_after_stop().is_err());
        session.acquired_lock().unwrap();
        assert!(session.begin(800.).is_err());
    }
}

#[test]
fn dpi_matcher_ingest_preserves_nonfinite_source_samples_and_math_min_nan() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
        session.begin(800.).unwrap();
        session.acquired_lock().unwrap();
        session
            .ingest(batch("source", 123, &[(1., 1.)]), false, 0)
            .unwrap();
        session
            .ingest(batch("main", product_id, &[(1., f64::NAN)]), true, 1)
            .unwrap();
        assert!(session.response().process().is_nan());
        assert_eq!(session.state, State::Calibrating);
        assert!(session.new_dpi().is_none());
        assert!(session.complete_after_stop().is_err());
    }
}

fn assert_source_number(actual: f64, expected: f64) {
    if expected.is_nan() {
        assert!(actual.is_nan(), "expected NaN, observed {actual:?}");
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:?} != {expected:?}"
        );
    }
}

fn stopped_numeric_result(
    product_id: u32,
    current_dpi: f64,
    main: &[(f64, f64)],
    source: &[(f64, f64)],
    final_main: (f64, f64),
) -> f64 {
    let mut session = Session::new(Product::new(product_id).unwrap());
    session.begin(current_dpi).unwrap();
    session.acquired_lock().unwrap();
    assert!(
        !session
            .ingest(batch("source", 123, source), false, 0)
            .unwrap()
    );
    let stopping = session
        .ingest(batch("main", product_id, main), true, 1)
        .unwrap();
    if !stopping {
        assert!(
            session
                .ingest(batch("main", product_id, &[final_main]), true, 2)
                .unwrap()
        );
    }
    assert_eq!(session.state, State::Stopping);
    assert_eq!(session.response().phase(), Phase::Calibrating);
    assert!(session.new_dpi().is_none());
    assert!(!session.check_timeout(10_000));
    session.complete_after_stop().unwrap();
    assert_eq!(session.state, State::Completed);
    assert_eq!(session.response().phase(), Phase::Completed);
    let result = session.new_dpi().unwrap();
    assert_source_number(session.response().new_dpi(), result);
    assert!(session.complete_after_stop().is_err());
    result
}

#[test]
fn dpi_matcher_complete_after_stop_keeps_zero_nan_infinity_and_overflow_results() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        for (source, last_main, expected) in [
            (&[(20., 0.), (30., 0.)][..], (40., -100.), 0.),
            (&[(20., 1000.), (30., 1250.)][..], (10., -100.), f64::NAN),
        ] {
            let result = stopped_numeric_result(
                product_id,
                800.,
                &[(10., 1000.), (20., 2000.), (30., 2500.)],
                source,
                last_main,
            );
            assert_source_number(result, expected);
        }
        for (main, source) in [
            (
                &[(10., 0.), (20., 4500.), (30., 0.)][..],
                &[(10., 1000.), (30., 0.)][..],
            ),
            (
                &[(10., 1e-308), (20., 4500.), (30., 0.)][..],
                &[(10., 1e308), (30., 0.)][..],
            ),
        ] {
            let result = stopped_numeric_result(product_id, 800., main, source, (10., 0.));
            assert_source_number(result, f64::INFINITY);
        }
        let result = stopped_numeric_result(
            product_id,
            f64::MAX,
            &[(1., 5e307), (2., 1.5e308)],
            &[(1., 1e308)],
            (3., 0.),
        );
        assert_source_number(result, f64::INFINITY);
        let result = stopped_numeric_result(
            product_id,
            800.,
            &[(10., 1e308), (20., 1e308), (30., 0.)],
            &[(10., 1e308), (20., 1e308), (30., 0.)],
            (40., 0.),
        );
        assert_source_number(result, f64::NAN);
    }
}

#[test]
fn dpi_matcher_math_round_keeps_negative_results_ties_and_signed_zero_after_stop() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        for (baseline, main, source, expected) in [
            (0., 1., 1., 0.),
            (-0., 1., 1., -0.),
            (-800., 2., 1., -400.),
            (-1., 2., 1., -0.),
            (-3., 2., 1., -1.),
            (-5., 2., 1., -2.),
            (f64::NEG_INFINITY, 2., 1., f64::NEG_INFINITY),
        ] {
            let result = stopped_numeric_result(
                product_id,
                baseline,
                &[(1., main)],
                &[(1., source)],
                (2., 0.),
            );
            assert_source_number(result, expected);
        }
    }
}

#[test]
fn dpi_matcher_nonfinite_event_bounds_keep_the_source_inverse_comparisons() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let result = stopped_numeric_result(
            product_id,
            800.,
            &[(f64::NAN, 1000.), (0., 1000.), (1., 4500.)],
            &[(1., 3250.)],
            (2., 0.),
        );
        assert_source_number(result, 400.);
        let result = stopped_numeric_result(
            product_id,
            800.,
            &[(10., 2000.), (20., 2500.)],
            &[(10., 1000.), (50., 1250.), (20., 0.)],
            (f64::NAN, 0.),
        );
        assert_source_number(result, 400.);
    }
}

#[test]
fn dpi_matcher_math_round_retains_near_half_and_large_finite_values() {
    for (input, expected) in [
        (0.49999999999999994, 0.),
        (0.5, 1.),
        (-0.5, -0.),
        (-1.5, -1.),
        (4503599627370495., 4503599627370495.),
        (f64::MAX, f64::MAX),
        (f64::INFINITY, f64::INFINITY),
        (f64::NEG_INFINITY, f64::NEG_INFINITY),
        (f64::NAN, f64::NAN),
    ] {
        assert_source_number(source_round(input), expected);
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
fn dpi_matcher_cancel_publishes_source_idle_before_clearing_movement() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
        session.begin(800.).unwrap();
        session.acquired_lock().unwrap();
        session
            .ingest(batch("reference", 123, &[(1., 100.)]), false, 0)
            .unwrap();
        session
            .ingest(batch("main", product_id, &[(1., 2000.)]), true, 1)
            .unwrap();
        assert_eq!(
            session.response(),
            Calibration::new(Phase::Calibrating, 50., 800., 0.)
        );
        assert_eq!(
            session.cancel_after_stop(),
            Some(Calibration::new(Phase::Ready, 50., 800., 0.)),
            "product {product_id}",
        );
        assert_eq!(
            session.response(),
            Calibration::new(Phase::Ready, 0., 0., 0.)
        );
        assert!(session.source_container().is_empty());
        assert_eq!(session.source_product_id, None);
        assert!(session.new_dpi().is_none());
        assert!(!session.check_timeout(10_000));
        assert!(session.cancel_after_stop().is_none());
        session.begin(1600.).unwrap();
        session.acquired_lock().unwrap();
        session
            .ingest(
                batch("fresh-main", product_id, &[(5., 4000.)]),
                true,
                20_000,
            )
            .unwrap();
        assert_eq!(session.response().process(), 0.);
        assert!(session.source_container().is_empty());
    }
}

#[test]
fn dpi_matcher_cancel_event_is_ignored_outside_the_source_calibrating_state() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut session = Session::new(Product::new(product_id).unwrap());
        assert!(session.cancel_after_stop().is_none());
        assert_eq!(session.state, State::Ready);
        session.begin(100.).unwrap();
        assert!(session.cancel_after_stop().is_none());
        assert_eq!(session.state, State::RequestingLock);
        session.acquired_lock().unwrap();
        assert_eq!(
            session.response(),
            Calibration::new(Phase::Calibrating, 0., 0., 0.)
        );
        session.reset();
        session.begin(100.).unwrap();
        session.acquired_lock().unwrap();
        session
            .ingest(batch("reference", 123, &[(1., 100.)]), false, 0)
            .unwrap();
        session
            .ingest(batch("main", product_id, &[(1., 1.)]), true, 1)
            .unwrap();
        assert!(session.check_timeout(501));
        assert!(session.cancel_after_stop().is_none());
        assert_eq!(session.state, State::Error);
        session.begin(100.).unwrap();
        session.acquired_lock().unwrap();
        session
            .ingest(batch("reference", 123, &[(1., 1000.)]), false, 1000)
            .unwrap();
        session
            .ingest(batch("main", product_id, &[(1., 600.)]), true, 1001)
            .unwrap();
        assert!(
            session
                .ingest(batch("main", product_id, &[(2., 0.)]), true, 1002)
                .unwrap()
        );
        session.complete_after_stop().unwrap();
        assert_eq!(session.state, State::Completed);
        assert!(session.cancel_after_stop().is_none());
        assert_eq!(session.state, State::Completed);
        assert_eq!(
            session.response(),
            Calibration::new(Phase::Completed, 0., 0., 0.)
        );
        assert_eq!(session.new_dpi(), Some(0.));
    }
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
fn dpi_matcher_retains_container_partition_through_reset_and_timeout() {
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
            // Every current R(pid,container) keeps the known reference stream.
            assert_eq!(session.source_product_id, Some(product_id));
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
        assert!(product.is_dual_link_reference(241));
    }
}
