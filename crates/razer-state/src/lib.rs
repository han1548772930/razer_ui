//! Retained application palette state; persisted preferences belong to the model.
use gpui_kit::{App, BorrowAppContext, Global};
pub use razer_model::preferences::*;

#[derive(Default)]
pub struct CustomColors {
    colors: CustomColorSlots,
    saved: CustomColorSlots,
}
impl Global for CustomColors {}
impl CustomColors {
    pub fn new(colors: CustomColorSlots) -> Self {
        Self {
            colors,
            saved: colors,
        }
    }
    pub fn ensure(cx: &mut App) {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
    }
    pub fn colors(&self) -> CustomColorSlots {
        self.colors
    }
    pub fn dirty(&self) -> bool {
        self.colors != self.saved
    }
    pub fn replace(colors: CustomColorSlots, cx: &mut App) {
        Self::ensure(cx);
        if cx.global::<Self>().colors != colors {
            cx.update_global::<Self, _>(|palette, _| palette.colors = colors);
        }
    }
    pub fn mark_saved(colors: CustomColorSlots, cx: &mut App) {
        // A completed write acknowledges its captured revision, not later edits.
        cx.update_global::<Self, _>(|palette, _| palette.saved = colors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui_kit::test]
    fn palette_save_completion_keeps_later_edits_pending(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            CustomColors::ensure(cx);
            let mut first = [None; 16];
            first[0] = Some([1, 2, 3]);
            CustomColors::replace(first, cx);
            let captured = cx.global::<CustomColors>().colors();
            let mut second = first;
            second[1] = Some([4, 5, 6]);
            CustomColors::replace(second, cx);
            CustomColors::mark_saved(captured, cx);
            assert!(cx.global::<CustomColors>().dirty());
            assert_eq!(cx.global::<CustomColors>().colors(), second);
            CustomColors::mark_saved(second, cx);
            assert!(!cx.global::<CustomColors>().dirty());
        });
    }
}
