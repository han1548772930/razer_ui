//! Actual 2676 np/Qm/qm and 2684 Ic/yc/Tc calibration selection widget.
//! Current JSX/CSS and resource receipts: gamepad-trigger-selection-resources-current.json.
use super::*;
use razer_widgets::theme::GamepadDialogColors as Colors;

pub(super) fn source_svg(pid: u32, edition: u32, kind: &str) -> Option<&'static str> {
    const SOURCES: &[(u32, u32, &str, &str)] =
        include!("../../../../assets/synapse/gamepad-calibration-svg-sources.rs");
    SOURCES
        .iter()
        .find(|&&(product, variant, source_kind, _)| {
            product == pid && variant == edition && source_kind == kind
        })
        .or_else(|| {
            SOURCES.iter().find(|&&(product, variant, source_kind, _)| {
                product == pid && variant == 0 && source_kind == kind
            })
        })
        .map(|&(_, _, _, asset)| asset)
}

pub(super) fn render(
    pid: u32,
    edition: u32,
    hovered_part: Option<u8>,
    cx: &Context<GamepadProductWorkspace>,
) -> AnyElement {
    // The later equally specific base panel rule overrides the earlier
    // --trigger-calibration height:422.42px in both current stylesheets.
    let mut panel = div()
        .relative()
        .w(surface::css(600.))
        .h(surface::css(480.))
        .flex_shrink_0()
        .my(surface::css(10.))
        .pt(surface::css(30.))
        .px(surface::css(40.))
        .pb(surface::css(20.))
        .overflow_hidden()
        .rounded(surface::css(5.))
        .bg(Colors::panel())
        .text_color(Colors::text())
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .child(
            div()
                .font_family("RazerF5")
                .text_size(surface::css(16.))
                .text_color(Colors::primary())
                .mb(surface::css(17.))
                .child(t("TAB_CALIBRATION")),
        )
        .child(
            div()
                .w(surface::css(520.))
                .line_height(surface::css(17.))
                .text_color(description_color())
                .child(t("CONTROLLER_CALIBRATION_DESCRIPTION")),
        );
    if let Some(image) = source_svg(pid, edition, "prd") {
        panel = panel.child(
            div()
                .absolute()
                .left(surface::css(114.))
                .top(surface::css(160.))
                .w(surface::css(372.))
                .h(surface::css(253.))
                .child(img(image).w_full().h_full().object_fit(ObjectFit::Contain)),
        );
    }
    for (part, label, width, left, top) in [
        (3_u8, "LEFT_TRIGGER", 111., 40., 120.),
        (4, "RIGHT_TRIGGER", 118., 442., 120.),
        (1, "CALIBBRTION_LEFT_THUMBSITCK", 137., 40., 0.),
        (2, "CALIBBRTION_RIGHT_THUMBSITCK", 144., 416., 0.),
    ] {
        panel = panel.child(
            BaseButton::new(("gamepad-calibration-select", part as u32))
                .accessibility_label(t(label))
                .absolute()
                .left(surface::css(left))
                .when(part <= 2, |b| b.bottom(surface::css(32.)))
                .when(part >= 3, |b| b.top(surface::css(top)))
                .w(surface::css(width))
                .min_h(surface::css(27.))
                .px(surface::css(15.))
                .pt(surface::css(7.))
                .pb(surface::css(6.))
                .border_1()
                .border_color(Colors::text())
                .rounded(surface::css(3.))
                .bg(Colors::panel())
                .text_color(Colors::text())
                .font_family("Roboto")
                .text_size(surface::css(12.))
                .line_height(surface::css(14.))
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .hover(|style| style.bg(Colors::primary_text()))
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.set_calibration_selection_hover(
                        if *hovered { Some(part) } else { None },
                        cx,
                    )
                }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if part <= 2 {
                        this.open_calibration_popup(part, window, cx)
                    } else {
                        this.open_trigger_calibration_popup(part, window, cx)
                    }
                }))
                .child(t(label).to_uppercase()),
        );
    }
    panel = panel.child(
        div()
            .absolute()
            .left(surface::css(151.))
            .top(surface::css(133.5))
            .w(surface::css(292.))
            .h(surface::css(304.))
            .child(
                img(SharedString::from(format!(
                    "synapse/gamepad-{pid}-calibration-connectors-{}.svg",
                    hovered_part.unwrap_or(0)
                )))
                .w_full()
                .h_full(),
            ),
    );
    h_flex()
        .w_full()
        .justify_center()
        .child(panel)
        .into_any_element()
}

// Source .thumbstick-calibration-widget__description has its own #bbb role.
fn description_color() -> Hsla {
    rgb(0xbbbbbb).into()
}
