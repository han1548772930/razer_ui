//! Display ownership follows GPUI element state, including deferred surfaces.
use super::{FilteredIdentity, ResourceImageCache};
use gpui_kit::{App, Context, ElementId, Resource, SharedString, WeakEntity, Window};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum CacheIdentity {
    Image(Resource),
    Filtered(FilteredIdentity),
}

#[derive(Clone, Default)]
pub(super) struct DisplayOwnership(Rc<Cell<usize>>);

impl DisplayOwnership {
    fn acquire(&self) -> DisplayLease {
        self.0.set(self.0.get() + 1);
        DisplayLease(self.clone())
    }
    pub(super) fn unused(&self) -> bool {
        self.0.get() == 0
    }
    pub(super) fn is_same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

struct DisplayLease(DisplayOwnership);
impl Drop for DisplayLease {
    fn drop(&mut self) {
        self.0.0.set(self.0.0.get() - 1);
    }
}

struct DisplayedImage {
    cache: WeakEntity<ResourceImageCache>,
    identity: CacheIdentity,
    lease: Option<DisplayLease>,
}

impl DisplayedImage {
    fn release(&mut self, cx: &mut App) {
        let Some(lease) = self.lease.take() else { return };
        let ownership = lease.0.clone();
        drop(lease);
        if !ownership.unused() {
            return;
        }
        let cache = self.cache.clone();
        let identity = self.identity.clone();
        // The current window is temporarily absent from App during drawing.
        // Deferring also avoids re-entering its borrowed image-cache entity.
        cx.defer(move |cx| {
            let _ = cache.update(cx, |cache, cx| {
                cache.release_displayed(&identity, &ownership, cx);
            });
        });
    }
}

pub(super) fn retain(
    cache: WeakEntity<ResourceImageCache>,
    identity: CacheIdentity,
    ownership: DisplayOwnership,
    key: SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    let state = window.use_keyed_state(ElementId::Name(key), cx, |_, cx| {
        cx.on_release(|state: &mut DisplayedImage, cx| state.release(cx))
            .detach();
        DisplayedImage {
            cache: cache.clone(),
            identity: identity.clone(),
            lease: Some(ownership.acquire()),
        }
    });
    state.update(cx, |state, cx: &mut Context<DisplayedImage>| {
        if state
            .lease
            .as_ref()
            .is_some_and(|lease| lease.0.is_same(&ownership))
        {
            return;
        }
        state.release(cx);
        state.cache = cache;
        state.identity = identity;
        state.lease = Some(ownership.acquire());
    });
}

#[cfg(test)]
mod tests {
    use super::DisplayOwnership;

    #[test]
    fn display_ownership_keeps_shared_images_until_the_last_surface_leaves() {
        let owner = DisplayOwnership::default();
        let main = owner.acquire();
        let overlay = owner.acquire();
        assert!(!owner.unused());
        drop(main);
        assert!(!owner.unused());
        drop(overlay);
        assert!(owner.unused());
        let reentered = owner.acquire();
        assert!(
            !owner.unused(),
            "queued cleanup must recheck a returning consumer"
        );
        drop(reentered);
        assert!(owner.unused());
    }

    #[test]
    fn display_ownership_does_not_release_a_replacement_decode() {
        let old = DisplayOwnership::default();
        let current = DisplayOwnership::default();
        let previous_surface = old.acquire();
        let new_surface = current.acquire();
        drop(previous_surface);
        assert!(old.unused());
        assert!(!old.is_same(&current));
        assert!(!current.unused());
        drop(new_surface);
        assert!(current.unused());
    }
}
