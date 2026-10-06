//! Bounded background image loading and ownership of decoded GPUI resources.
use super::*;

const MAX_ENTRIES: usize = 64;
const MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;
const MAX_LOADS: usize = 8;

/// Reuse the preview's atlas retirement: the last owner releases the texture.
pub(in crate::app) struct CachedImage(crate::computer_use::PreviewImage);

impl std::fmt::Debug for CachedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CachedImage")
            .field(&self.0.source_id)
            .finish()
    }
}

impl CachedImage {
    pub(in crate::app) fn new(
        source_id: u64,
        image: Arc<gpui::RenderImage>,
        cx: &App,
    ) -> Arc<Self> {
        Arc::new(Self(crate::computer_use::PreviewImage::new(
            source_id, image, cx,
        )))
    }

    pub(in crate::app) fn image(&self) -> Arc<gpui::RenderImage> {
        self.0.image.clone()
    }

    fn bytes(&self) -> usize {
        (0..self.0.image.frame_count())
            .filter_map(|frame| self.0.image.as_bytes(frame))
            .map(<[u8]>::len)
            .sum()
    }
}

#[derive(Default)]
pub(in crate::app) struct ImageModel {
    cache: RefCell<ImageCache>,
}

#[derive(Default)]
struct ImageCache {
    entries: HashMap<String, Entry>,
    /// Cached panes replay without resolving images again. Preserve their pins
    /// until the pane actually renders or leaves the root's mounted subtree.
    views: HashMap<gpui::EntityId, HashSet<String>>,
    clock: u64,
    bytes: usize,
    loads: usize,
}

struct Entry {
    state: RemoteImageState,
    touched: u64,
    bytes: usize,
}

enum RemoteImageState {
    Loading(u64),
    Ready(Arc<CachedImage>),
    Unavailable,
}

impl ImageCache {
    fn begin_frame(&mut self, root: gpui::EntityId, transcript: Option<gpui::EntityId>) {
        self.views
            .retain(|view, _| *view == root || Some(*view) == transcript);
        self.views.entry(root).or_default().clear();
    }

    fn request(
        &mut self,
        reference: &str,
        view: gpui::EntityId,
        supported: bool,
    ) -> (Option<Arc<CachedImage>>, Option<u64>) {
        self.views
            .entry(view)
            .or_default()
            .insert(reference.to_owned());
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(reference) {
            entry.touched = self.clock;
            return (
                match &entry.state {
                    RemoteImageState::Ready(image) => Some(image.clone()),
                    _ => None,
                },
                None,
            );
        }
        if supported && self.loads >= MAX_LOADS {
            return (None, None);
        }
        let ticket = supported.then_some(self.clock);
        self.entries.insert(
            reference.to_owned(),
            Entry {
                state: ticket.map_or(RemoteImageState::Unavailable, RemoteImageState::Loading),
                touched: self.clock,
                bytes: 0,
            },
        );
        self.loads += usize::from(supported);
        (None, ticket)
    }

    fn insert(&mut self, reference: String, image: Option<Arc<CachedImage>>) {
        self.clock += 1;
        let bytes = image.as_ref().map_or(0, |image| image.bytes());
        let previous = self.entries.insert(
            reference,
            Entry {
                state: image.map_or(RemoteImageState::Unavailable, RemoteImageState::Ready),
                touched: self.clock,
                bytes,
            },
        );
        self.bytes = self.bytes - previous.map_or(0, |entry| entry.bytes) + bytes;
    }

    fn complete(&mut self, reference: String, ticket: u64, image: Option<Arc<CachedImage>>) {
        self.loads -= 1;
        if self.entries.get(&reference).is_some_and(
            |entry| matches!(entry.state, RemoteImageState::Loading(current) if current == ticket),
        ) {
            self.insert(reference, image);
        }
        self.trim();
    }

    fn trim(&mut self) {
        if self.entries.len() <= MAX_ENTRIES && self.bytes <= MAX_CACHE_BYTES {
            return;
        }
        let pinned = self.views.values().flatten().collect::<HashSet<_>>();
        let mut candidates = self
            .entries
            .iter()
            .filter_map(|(reference, entry)| {
                (!pinned.contains(reference)
                    && !matches!(entry.state, RemoteImageState::Loading(_)))
                .then_some((entry.touched, reference.clone()))
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        for (_, reference) in candidates {
            if self.entries.len() <= MAX_ENTRIES && self.bytes <= MAX_CACHE_BYTES {
                break;
            }
            self.bytes -= self.entries.remove(&reference).unwrap().bytes;
        }
    }

    fn clear(&mut self) {
        // Keep the in-flight count and monotonic tickets: old completions may
        // finish, but cannot repopulate a cleared cache or replace newer loads.
        self.entries.clear();
        self.views.clear();
        self.bytes = 0;
    }
}

impl Michelle {
    /// Frames read memory only. Fetching and decoding both run in the background;
    /// rendering a RenderImage bypasses GPUI's unbounded global Image asset cache.
    pub(in crate::app) fn image_for_reference(
        &self,
        reference: &str,
        daemon_path: Option<&Path>,
        name: Option<&str>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<CachedImage>> {
        if !michelle_protocol::blob::is_reference(reference)
            && !reference.starts_with(michelle_protocol::attachments::ATTACHMENT_SCHEME)
        {
            return None;
        }
        let format = name
            .and_then(image_format_for_name)
            .or_else(|| image_format_for_name(reference));
        let (image, ticket) = self.image_model.cache.borrow_mut().request(
            reference,
            window.current_view(),
            format.is_some(),
        );
        let Some(ticket) = ticket else {
            return image;
        };
        let format = format.unwrap();
        let cache_key = reference.to_owned();
        let fetch_reference = cache_key.clone();
        let daemon_path = daemon_path.map(Path::to_path_buf);
        let daemon = self.daemon.clone();
        let renderer = cx.svg_renderer();
        cx.spawn(async move |michelle, cx| {
            let image = cx
                .background_executor()
                .spawn(async move {
                    let bytes = michelle_client::persistence::read_remote_reference(
                        &fetch_reference,
                        daemon_path.as_deref(),
                        &daemon,
                    )?;
                    let image = gpui::Image::from_bytes(format, bytes);
                    image
                        .to_image_data(renderer)
                        .ok()
                        .map(|decoded| (image.id(), decoded))
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                let image = image.map(|(id, image)| CachedImage::new(id, image, cx));
                michelle
                    .image_model
                    .cache
                    .borrow_mut()
                    .complete(cache_key, ticket, image);
                cx.notify();
            });
        })
        .detach();
        None
    }

    pub(in crate::app) fn begin_image_frame(
        &self,
        window: &Window,
        transcript_mounted: bool,
        cx: &mut Context<Self>,
    ) {
        let root = window.current_view();
        let transcript = transcript_mounted.then(|| self.shell_ui.transcript_pane.entity_id());
        self.image_model
            .cache
            .borrow_mut()
            .begin_frame(root, transcript);
        // Row measurement happens after render returns; trim after all visible
        // rows have registered their pins, including a reused pane's old pins.
        let weak = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = weak.update(cx, |this, _| this.image_model.cache.borrow_mut().trim());
        });
    }
}

impl ImageModel {
    pub(in crate::app) fn begin_view(&self, view: gpui::EntityId) {
        self.cache
            .borrow_mut()
            .views
            .entry(view)
            .or_default()
            .clear();
    }

    pub(in crate::app) fn retain_preview(&self, reference: String, image: Arc<CachedImage>) {
        self.cache.borrow_mut().insert(reference, Some(image));
    }

    pub(in crate::app) fn clear(&self) {
        self.cache.borrow_mut().clear();
    }
}

#[cfg(test)]
mod image_cache_tests {
    use super::*;
    use gpui::TestAppContext;

    fn image(cx: &App, width: usize, height: usize) -> Arc<CachedImage> {
        let source = gpui::Image::from_bytes(
            gpui::ImageFormat::Svg,
            format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect width="100%" height="100%" fill="#f00"/></svg>"##).into_bytes(),
        );
        let decoded = source.to_image_data(cx.svg_renderer()).unwrap();
        // The managed path must not populate GPUI's global encoded-image cache.
        assert!(!Arc::new(source.clone()).is_asset_cached(cx));
        CachedImage::new(source.id(), decoded, cx)
    }

    #[test]
    fn image_cache_preserves_cached_pane_pins_until_it_renders_or_unmounts() {
        let model = ImageModel::default();
        let root = gpui::EntityId::from(1);
        let pane = gpui::EntityId::from(2);
        {
            let mut cache = model.cache.borrow_mut();
            cache.request("composer", root, false);
            cache.request("transcript", pane, false);
            cache.begin_frame(root, Some(pane));
            assert!(cache.views[&root].is_empty());
            assert!(cache.views[&pane].contains("transcript"));
        }
        model.begin_view(pane);
        assert!(model.cache.borrow().views[&pane].is_empty());
        let mut cache = model.cache.borrow_mut();
        cache.request("new transcript", pane, false);
        cache.begin_frame(root, None);
        assert!(!cache.views.contains_key(&pane));
    }

    #[gpui::test]
    fn image_cache_bounds_entries_and_preserves_recent_hits(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut cache = ImageCache::default();
            let view = gpui::EntityId::from(1);
            let image = image(cx, 16, 16);
            let size = image.image().size(0);
            assert_eq!(
                image.bytes(),
                size.width.0 as usize * size.height.0 as usize * 4
            );
            for index in 0..MAX_ENTRIES {
                cache.insert(index.to_string(), Some(image.clone()));
            }
            assert!(cache.request("0", view, true).0.is_some());
            cache.views.clear();
            for index in MAX_ENTRIES..1000 {
                cache.insert(index.to_string(), Some(image.clone()));
                cache.trim();
                assert_eq!(cache.entries.len(), MAX_ENTRIES);
                assert_eq!(cache.bytes, MAX_ENTRIES * image.bytes());
                if index == MAX_ENTRIES {
                    assert!(cache.entries.contains_key("0"));
                    assert!(!cache.entries.contains_key("1"));
                }
            }
        });
        cx.run_until_parked();
    }

    #[gpui::test]
    fn image_cache_bounds_decoded_bytes_and_keeps_visible_images(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut cache = ImageCache::default();
            let view = gpui::EntityId::from(1);
            let image = image(cx, 512, 256);
            assert_eq!(image.bytes(), 2 * 1024 * 1024);
            let count = MAX_CACHE_BYTES / image.bytes();
            for index in 0..count {
                cache.insert(index.to_string(), Some(image.clone()));
            }
            cache.request("0", view, true);
            cache.insert("new".into(), Some(image.clone()));
            cache.trim();
            assert_eq!(cache.bytes, MAX_CACHE_BYTES);
            assert!(cache.entries.contains_key("0"));
            assert!(!cache.entries.contains_key("1"));
            // Visible content is a soft-budget exception, then becomes evictable.
            for reference in cache.entries.keys().cloned().collect::<Vec<_>>() {
                cache.request(&reference, view, true);
            }
            cache.insert("visible".into(), Some(image));
            cache.request("visible", view, true);
            cache.trim();
            assert!(cache.bytes > MAX_CACHE_BYTES);
            cache.views.clear();
            cache.trim();
            assert!(cache.bytes <= MAX_CACHE_BYTES);
        });
        cx.run_until_parked();
    }

    #[test]
    fn image_cache_limits_failures_and_deduplicates_bounded_loads() {
        let mut cache = ImageCache::default();
        let view = gpui::EntityId::from(1);
        for index in 0..1000 {
            cache.request(&index.to_string(), view, false);
            cache.views.clear();
            cache.trim();
            assert!(cache.entries.len() <= MAX_ENTRIES);
        }
        cache.clear();
        let first = cache.request("first", view, true).1.unwrap();
        assert!(cache.request("first", view, true).1.is_none());
        for index in 1..MAX_LOADS {
            assert!(cache.request(&index.to_string(), view, true).1.is_some());
        }
        assert_eq!(cache.loads, MAX_LOADS);
        assert!(cache.request("waiting", view, true).1.is_none());
        cache.complete("first".into(), first, None);
        assert!(cache.request("waiting", view, true).1.is_some());
        assert_eq!(cache.loads, MAX_LOADS);
    }

    #[test]
    fn image_cache_ignores_completions_after_clear_and_newer_requests() {
        let mut cache = ImageCache::default();
        let view = gpui::EntityId::from(1);
        let old = cache.request("same", view, true).1.unwrap();
        cache.clear();
        let new = cache.request("same", view, true).1.unwrap();
        cache.complete("same".into(), old, None);
        assert!(
            matches!(cache.entries["same"].state, RemoteImageState::Loading(ticket) if ticket == new)
        );
        assert_eq!(cache.loads, 1);
        cache.clear();
        cache.complete("same".into(), new, None);
        assert!(cache.entries.is_empty());
        assert_eq!(cache.loads, 0);
    }

    #[gpui::test]
    fn image_cache_releases_decoded_data_but_keeps_an_open_preview(cx: &mut TestAppContext) {
        let (weak, decoded, mut cache, preview) = cx.update(|cx| {
            let image = image(cx, 16, 16);
            let weak = Arc::downgrade(&image);
            let decoded = Arc::downgrade(&image.image());
            let mut cache = ImageCache::default();
            cache.insert("preview".into(), Some(image.clone()));
            (weak, decoded, cache, image)
        });
        cache.clear();
        cx.run_until_parked();
        assert!(weak.upgrade().is_some());
        assert!(decoded.upgrade().is_some());
        drop(preview);
        cx.run_until_parked();
        assert!(weak.upgrade().is_none());
        assert!(decoded.upgrade().is_none());
    }

    #[gpui::test]
    fn image_cache_keeps_submitted_preview_when_an_older_fetch_finishes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut cache = ImageCache::default();
            let view = gpui::EntityId::from(1);
            let ticket = cache.request("submitted", view, true).1.unwrap();
            let preview = image(cx, 16, 16);
            cache.insert("submitted".into(), Some(preview.clone()));
            cache.complete("submitted".into(), ticket, None);
            assert!(Arc::ptr_eq(
                &cache.request("submitted", view, true).0.unwrap(),
                &preview
            ));
            assert_eq!(cache.loads, 0);
        });
        cx.run_until_parked();
    }
}
