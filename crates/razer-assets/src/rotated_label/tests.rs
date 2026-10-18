use super::*;

// Current 3893 acquisition receipt: Roboto-Regular.woff SHA-256
// 1570f866bf6eae82041e407280894a86ad2b8b275e01908ae156914dc693a4ed.
// The fixture is its losslessly decoded sfnt, including its original tables;
// SHA-256 9939ae1ec75415c4b6e8889c95704bfbcbfb9d4245203605c1044ba21845ca36.
const FONT: &[u8] = include_bytes!("../../testdata/rotated_label/Roboto-Regular.ttf");

fn request(text: &str) -> LabelIdentity {
    LabelIdentity {
        request: LabelRequest::new(
            text,
            "en",
            LabelGeometry::new(14., size(20., 410.), point(0., 164.5)),
        ),
        font: Arc::from(FONT),
    }
}

fn decode(identity: &LabelIdentity) -> Result<Arc<[u8]>, String> {
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    outline(
        identity,
        Arc::new(usvg::fontdb::Database::new()),
        &operation,
    )
}

#[::core::prelude::v1::test]
fn rotated_label_outlines_current_roboto_without_a_runtime_font_dependency() {
    let output = decode(&request("Fan Speed")).unwrap();
    let svg = std::str::from_utf8(&output).unwrap();
    assert!(!svg.contains("<text"));
    let parsed = usvg::Tree::from_data(&output, &usvg::Options::default()).unwrap();
    assert_eq!(parsed.size().width(), 20.);
    assert_eq!(parsed.size().height(), 410.);
    let bounds = parsed.root().abs_bounding_box();
    assert!(bounds.width() > 5. && bounds.width() < 14.);
    assert!(bounds.height() > 50.);
    assert!((bounds.top() + bounds.bottom() - 329.).abs() < 3.);
}

#[::core::prelude::v1::test]
fn rotated_label_xml_text_is_escaped_before_shaping() {
    let output = decode(&request("A < B & C > D")).unwrap();
    let parsed = usvg::Tree::from_data(&output, &usvg::Options::default()).unwrap();
    assert!(!parsed.root().children().is_empty());
    assert!(parsed.root().abs_bounding_box().height() > 65.);
}

#[::core::prelude::v1::test]
fn rotated_label_missing_glyph_is_an_error_instead_of_successful_empty_pixels() {
    let error = decode(&request("风扇")).unwrap_err();
    assert!(
        error.contains("No local font for label character"),
        "{error}"
    );
}

#[::core::prelude::v1::test]
fn rotated_label_rejects_invalid_font_and_geometry() {
    let mut invalid_font = request("Fan Speed");
    invalid_font.font = Arc::from(&b"invalid-font"[..]);
    assert_eq!(
        decode(&invalid_font).unwrap_err(),
        "Invalid source Roboto font"
    );
    let mut invalid_geometry = request("Fan Speed");
    invalid_geometry.request.geometry.font_size = f32::NAN;
    assert_eq!(
        decode(&invalid_geometry).unwrap_err(),
        "Invalid rotated-label geometry"
    );
}

#[::core::prelude::v1::test]
fn rotated_label_cancelled_request_does_not_produce_an_outline() {
    let transfer = Transfer::new();
    let operation = transfer.accept().unwrap();
    transfer.cancel();
    assert!(
        outline(
            &request("Fan Speed"),
            Arc::new(usvg::fontdb::Database::new()),
            &operation
        )
        .is_err()
    );
}
