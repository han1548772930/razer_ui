//! Current Advanced body/column mounts, including source Fragment siblings.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Column {
    Left,
    Right,
}

/// The seven current root components have four distinct mount trees. Consume
/// each fragment once so its tooltip remains adjacent to its own widget.
fn compose<T>(
    product_id: u32,
    mut dynamic: Vec<T>,
    rotation: Vec<T>,
    mut column: impl FnMut(Column, Vec<T>) -> T,
) -> Option<Vec<T>> {
    match product_id {
        190 | 192 | 226 => {
            dynamic.push(column(Column::Left, rotation));
            dynamic.push(column(Column::Right, Vec::new()));
            Some(dynamic)
        }
        196 => {
            dynamic.extend(rotation);
            Some(dynamic)
        }
        222 => {
            dynamic.extend(rotation);
            Some(vec![column(Column::Left, dynamic)])
        }
        229 | 239 => {
            dynamic.extend(rotation);
            Some(vec![
                column(Column::Left, dynamic),
                column(Column::Right, Vec::new()),
            ])
        }
        _ => None,
    }
}

impl MouseProductWorkspace {
    pub(super) fn advanced(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let compact = surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        let elements = compose(
            self.spec.product_id,
            self.dynamic_panel(cx),
            vec![self.rotation_panel(cx)],
            |side, children| {
                // `.widget-col` is a block with width:600px; its direct flex
                // parent supplies flex:auto and the viewport media rule adds
                // 30px side margins. Child widgets retain their own widths.
                div()
                    .id(match side {
                        Column::Left => "mouse-advanced-left",
                        Column::Right => "mouse-advanced-right",
                    })
                    .w(surface::css(surface::WIDGET_WIDTH))
                    .min_w(surface::css(surface::WIDGET_WIDTH))
                    .flex_auto()
                    .when(compact, |column| {
                        column.mx(surface::css(surface::COMPACT_COLUMN_MARGIN))
                    })
                    .children(children)
                    .into_any_element()
            },
        )
        .expect("Advanced page is missing its audited product layout");
        surface::page_columns()
            .children(elements)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Column, compose};

    #[derive(Debug, PartialEq, Eq)]
    enum Node {
        Dynamic,
        FirmwareTip,
        Rotation,
        Column(Column, Vec<Node>),
    }

    #[test]
    fn mouse_advanced_mounts_match_all_seven_current_product_roots() {
        use super::Column::{Left, Right};
        use Node::{Column, Dynamic, Rotation};

        // Complete current JSX root declarators are byte-bound in the unified
        // mouse-advanced-product-mounted-layout source review.
        let cases = [
            (
                190,
                vec![Dynamic, Column(Left, vec![Rotation]), Column(Right, vec![])],
            ),
            (
                192,
                vec![Dynamic, Column(Left, vec![Rotation]), Column(Right, vec![])],
            ),
            (196, vec![Dynamic, Rotation]),
            (222, vec![Column(Left, vec![Dynamic, Rotation])]),
            (
                226,
                vec![Dynamic, Column(Left, vec![Rotation]), Column(Right, vec![])],
            ),
            (
                229,
                vec![Column(Left, vec![Dynamic, Rotation]), Column(Right, vec![])],
            ),
            (
                239,
                vec![Column(Left, vec![Dynamic, Rotation]), Column(Right, vec![])],
            ),
        ];
        for (product_id, expected) in cases {
            assert_eq!(
                compose(product_id, vec![Dynamic], vec![Rotation], Column),
                Some(expected),
                "product {product_id}",
            );
        }
    }

    #[test]
    fn mouse_advanced_firmware_fragment_keeps_widget_order_and_empty_columns() {
        use super::Column::{Left, Right};
        use Node::{Column, Dynamic, FirmwareTip, Rotation};

        for (product_id, expected) in [
            (
                190,
                vec![
                    Dynamic,
                    FirmwareTip,
                    Column(Left, vec![Rotation]),
                    Column(Right, vec![]),
                ],
            ),
            (196, vec![Dynamic, FirmwareTip, Rotation]),
            (
                222,
                vec![Column(Left, vec![Dynamic, FirmwareTip, Rotation])],
            ),
            (
                229,
                vec![
                    Column(Left, vec![Dynamic, FirmwareTip, Rotation]),
                    Column(Right, vec![]),
                ],
            ),
        ] {
            assert_eq!(
                compose(
                    product_id,
                    vec![Dynamic, FirmwareTip],
                    vec![Rotation],
                    Column
                ),
                Some(expected),
                "product {product_id}",
            );
        }
    }

    #[test]
    fn mouse_advanced_physical_or_unknown_products_do_not_borrow_a_layout() {
        for product_id in [0, 70, 182, 191, 193, 223, 227, 228, 230, 240, 999] {
            assert!(
                compose(
                    product_id,
                    vec![Node::Dynamic],
                    vec![Node::Rotation],
                    Node::Column
                )
                .is_none()
            );
        }
    }
}
