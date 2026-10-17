//! Text templates requested by a mounted product page. Views only read this
//! retained state; resource I/O is queued from lifecycle methods and completion
//! subscriptions, never from an SVG renderer.
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

type LoadResult = anyhow::Result<Option<Cow<'static, [u8]>>>;

/// The original product loaders retry edition0 after an edition failure.
/// An asynchronous cache lookup is still in progress and must not fetch it.
pub(crate) fn load_edition_resource<T>(
    edition: u32,
    mut loader: impl FnMut(u32) -> anyhow::Result<Option<T>>,
) -> anyhow::Result<Option<T>> {
    match loader(edition) {
        Ok(Some(value)) => Ok(Some(value)),
        Err(error) if edition == 0 || razer_assets::is_resource_pending(&error) => Err(error),
        Ok(None) | Err(_) if edition != 0 => loader(0),
        result => result,
    }
}

#[derive(Default)]
struct TextResource {
    text: Option<Arc<str>>,
    error: Option<String>,
}

#[derive(Default)]
pub(crate) struct TextResources {
    requested: BTreeMap<&'static str, TextResource>,
}

impl TextResources {
    pub(crate) fn request(&mut self, path: &'static str, cx: &gpui_kit::App) {
        self.request_with(path, &mut |path| razer_assets::load_path(path, cx));
    }

    fn request_with(&mut self, path: &'static str, loader: &mut impl FnMut(&str) -> LoadResult) {
        if self
            .requested
            .get(path)
            .is_some_and(|state| state.text.is_some())
        {
            return;
        }
        self.requested.entry(path).or_default();
        self.refresh_path(path, loader);
    }

    pub(crate) fn refresh(&mut self, cx: &gpui_kit::App) -> bool {
        self.refresh_with(&mut |path| razer_assets::load_path(path, cx))
    }

    fn refresh_with(&mut self, loader: &mut impl FnMut(&str) -> LoadResult) -> bool {
        let pending = self
            .requested
            .iter()
            .filter_map(|(&path, state)| state.text.is_none().then_some(path))
            .collect::<Vec<_>>();
        pending.into_iter().fold(false, |changed, path| {
            self.refresh_path(path, loader) || changed
        })
    }

    pub(crate) fn text(&self, path: &str) -> Option<&str> {
        self.requested.get(path)?.text.as_deref()
    }

    fn refresh_path(
        &mut self,
        path: &'static str,
        loader: &mut impl FnMut(&str) -> LoadResult,
    ) -> bool {
        let result = loader(path).and_then(|bytes| {
            bytes
                .map(|bytes| String::from_utf8(bytes.to_vec()).map(Arc::<str>::from))
                .transpose()
                .map_err(Into::into)
        });
        let state = self.requested.get_mut(path).expect("requested resource");
        match result {
            Ok(Some(text)) => {
                state.text = Some(text);
                state.error = None;
                true
            }
            Err(error) if razer_assets::is_resource_pending(&error) => {
                state.error = None;
                false
            }
            result => {
                let error = match result {
                    Ok(None) => format!("Resource is absent from the asset manifest: {path}"),
                    Err(error) => format!("{error:#}"),
                    Ok(Some(_)) => unreachable!(),
                };
                if state.error.as_ref() != Some(&error) {
                    eprintln!("Product resource {path}: {error}");
                    state.error = Some(error);
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_missing_template_recovers_without_loading_another_product() {
        let mut resources = TextResources::default();
        let mut paths = Vec::new();
        resources.request_with("assets/product-a.svg", &mut |path| {
            paths.push(path.to_owned());
            Err(razer_assets::ResourcePending.into())
        });
        assert!(resources.text("assets/product-a.svg").is_none());
        assert!(resources.requested["assets/product-a.svg"].error.is_none());
        assert!(resources.text("assets/product-b.svg").is_none());
        assert!(resources.refresh_with(&mut |path| {
            paths.push(path.to_owned());
            Ok(Some(Cow::Borrowed(b"<svg/>")))
        }));
        assert_eq!(resources.text("assets/product-a.svg"), Some("<svg/>"));
        assert_eq!(paths, ["assets/product-a.svg", "assets/product-a.svg"]);
        assert!(!resources.refresh_with(&mut |_| panic!("loaded templates do not request again")));
        resources.request_with("assets/product-a.svg", &mut |_| {
            panic!("existing template reused")
        });
    }

    #[test]
    fn invalid_template_is_not_retained_as_a_successful_image() {
        let mut resources = TextResources::default();
        resources.request_with("assets/template.svg", &mut |_| {
            Ok(Some(Cow::Borrowed(&[0xff])))
        });
        assert!(resources.text("assets/template.svg").is_none());
        assert!(resources.refresh_with(&mut |_| Ok(Some(Cow::Borrowed(b"<svg/>")))));
        assert_eq!(resources.text("assets/template.svg"), Some("<svg/>"));
    }

    #[test]
    fn pending_edition_does_not_request_the_base_edition() {
        let mut requested = Vec::new();
        let result = load_edition_resource::<()>(3, |edition| {
            requested.push(edition);
            Err(razer_assets::ResourcePending.into())
        });
        assert!(razer_assets::is_resource_pending(&result.unwrap_err()));
        assert_eq!(requested, [3]);
    }

    #[test]
    fn missing_or_failed_edition_requests_only_its_base_edition() {
        for failed in [false, true] {
            let mut requested = Vec::new();
            let result = load_edition_resource(3, |edition| {
                requested.push(edition);
                if edition == 0 {
                    Ok(Some("base artwork"))
                } else if failed {
                    anyhow::bail!("source edition absent")
                } else {
                    Ok(None)
                }
            });
            assert_eq!(result.unwrap(), Some("base artwork"));
            assert_eq!(requested, [3, 0]);
        }
    }

    #[test]
    fn available_edition_and_failed_base_do_not_request_more_resources() {
        let mut requested = Vec::new();
        assert_eq!(
            load_edition_resource(3, |edition| {
                requested.push(edition);
                Ok(Some("edition artwork"))
            })
            .unwrap(),
            Some("edition artwork")
        );
        assert_eq!(requested, [3]);
        requested.clear();
        let result = load_edition_resource::<()>(0, |edition| {
            requested.push(edition);
            anyhow::bail!("base absent")
        });
        assert_eq!(result.unwrap_err().to_string(), "base absent");
        assert_eq!(requested, [0]);
    }
}
