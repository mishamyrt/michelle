//! Deduplicated background loading of daemon-owned image bytes.
use super::*;

#[derive(Default)]
pub(in crate::app) struct ImageModel {
    /// In-memory GPUI images for daemon-owned bytes. A missing entry schedules
    /// one background fetch only when a visible row asks to render it; the
    /// desktop never creates another attachment file.
    remote_images: RefCell<HashMap<String, RemoteImageState>>,
}

#[derive(Clone, Debug)]
enum RemoteImageState {
    Loading,
    Ready(Arc<gpui::Image>),
    Unavailable,
}

impl Michelle {
    /// Resolve one daemon-owned image for a visible row. Frames consult only
    /// in-memory state; the first miss starts a deduplicated background RPC and
    /// a later notification lets GPUI render the returned bytes from memory.
    pub(in crate::app) fn image_for_reference(
        &self,
        reference: &str,
        daemon_path: Option<&Path>,
        name: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Option<Arc<gpui::Image>> {
        let attachment_reference =
            reference.starts_with(michelle_protocol::attachments::ATTACHMENT_SCHEME);
        if !michelle_protocol::blob::is_reference(reference) && !attachment_reference {
            return None;
        }
        if let Some(state) = self.image_model.remote_images.borrow().get(reference) {
            return match state {
                RemoteImageState::Ready(image) => Some(image.clone()),
                RemoteImageState::Loading | RemoteImageState::Unavailable => None,
            };
        }

        let Some(format) = name
            .and_then(image_format_for_name)
            .or_else(|| image_format_for_name(reference))
        else {
            self.image_model
                .remote_images
                .borrow_mut()
                .insert(reference.to_owned(), RemoteImageState::Unavailable);
            return None;
        };

        self.image_model
            .remote_images
            .borrow_mut()
            .insert(reference.to_owned(), RemoteImageState::Loading);
        let cache_key = reference.to_owned();
        let fetch_reference = cache_key.clone();
        let daemon_path = daemon_path.map(Path::to_path_buf);
        let daemon = self.daemon.clone();
        cx.spawn(async move |michelle, cx| {
            let image = cx
                .background_executor()
                .spawn(async move {
                    michelle_client::persistence::read_remote_reference(
                        &fetch_reference,
                        daemon_path.as_deref(),
                        &daemon,
                    )
                    .map(|bytes| Arc::new(gpui::Image::from_bytes(format, bytes)))
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| {
                michelle.image_model.remote_images.borrow_mut().insert(
                    cache_key,
                    image.map_or(RemoteImageState::Unavailable, RemoteImageState::Ready),
                );
                cx.notify();
            });
        })
        .detach();
        None
    }
}

impl ImageModel {
    pub(in crate::app) fn retain_preview(&self, reference: String, image: Arc<gpui::Image>) {
        self.remote_images
            .borrow_mut()
            .insert(reference, RemoteImageState::Ready(image));
    }
}
