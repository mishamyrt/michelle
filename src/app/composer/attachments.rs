//! Background attachment staging and upload preparation.
use super::*;
use anyhow::Context as _;
use base64::Engine as _;
const MAX_ATTACHMENT_BYTES: u64 = michelle_client::attachments::MAX_ATTACHMENT_BYTES as u64;

impl Michelle {
    pub(in crate::app) fn stage_attachment_paths(
        &mut self,
        paths: &[PathBuf],
        cx: &mut Context<Self>,
    ) -> bool {
        if paths.is_empty() {
            return false;
        }
        let paths = paths.to_vec();
        let daemon = self.daemon.clone();
        let draft_owner = self.selected_composer_draft_key();
        cx.spawn(async move |michelle, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut stored = Vec::with_capacity(paths.len());
                    for source_path in paths {
                        let (name, upload, image_bytes) =
                            attachment_upload_from_path(&source_path)?;
                        let is_image = image_bytes.is_some();
                        let preview_image = image_bytes.and_then(|bytes| {
                            image_preview::image_format_for_name(&name)
                                .map(|format| Arc::new(gpui::Image::from_bytes(format, bytes)))
                        });
                        let response = daemon.client().request(
                            Uuid::nil(),
                            Uuid::nil(),
                            michelle_client::Command::ImportAttachment { name, upload },
                        )?;
                        let michelle_client::ResponsePayload::AttachmentStored { attachment } =
                            response
                        else {
                            anyhow::bail!("the daemon returned an invalid attachment response");
                        };
                        stored.push((attachment, preview_image, is_image));
                    }
                    Ok::<_, anyhow::Error>(stored)
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| match result {
                Ok(stored) => {
                    if michelle.selected_composer_draft_key() != draft_owner {
                        return;
                    }
                    let mut changed = false;
                    for (attachment, preview_image, is_image) in stored {
                        changed |= michelle.stage_daemon_attachment(
                            attachment.path,
                            attachment.name,
                            attachment.is_dir,
                            is_image,
                            attachment.reference,
                            preview_image,
                        );
                    }
                    if changed {
                        michelle.schedule_composer_draft_save(cx);
                        cx.notify();
                    }
                }
                Err(error) => {
                    michelle.show_toast(error.to_string());
                    cx.notify();
                }
            });
        })
        .detach();
        true
    }

    fn stage_daemon_attachment(
        &mut self,
        path: PathBuf,
        name: String,
        is_dir: bool,
        is_image: bool,
        reference: String,
        client_preview_image: Option<Arc<gpui::Image>>,
    ) -> bool {
        if self.composer_model.attachments.iter().any(|attachment| {
            attachment.path == path
                || attachment.blob_reference.as_deref() == Some(reference.as_str())
        }) {
            return false;
        }
        let mut mention = path.display().to_string();
        if is_dir && !mention.ends_with('/') {
            mention.push('/');
        }
        self.composer_model.attachments.push(ComposerAttachment {
            path,
            client_preview_image,
            mention,
            name: SharedString::from(name),
            is_dir,
            is_image,
            blob_reference: Some(reference),
        });
        true
    }

    /// Stage the clipboard's primary representation, including large text.
    /// On-disk paths reuse drop handling; raw image bytes are copied into Michelle's
    /// durable blob store on the background executor before their chip appears.
    pub(in crate::app) fn stage_pasted_attachments(
        &mut self,
        entries: Vec<ClipboardEntry>,
        cx: &mut Context<Self>,
    ) {
        let mut paths = Vec::new();
        let mut images = Vec::new();
        for entry in entries {
            match entry {
                ClipboardEntry::String(text) => self.stage_pasted_text(text.into_text(), cx),
                ClipboardEntry::Image(image) if !image.bytes.is_empty() => images.push(image),
                ClipboardEntry::ExternalPaths(external) => {
                    paths.extend(external.paths().iter().cloned())
                }
                ClipboardEntry::Image(_) => {}
            }
        }
        self.stage_attachment_paths(&paths, cx);
        if images.is_empty() {
            return;
        }

        let daemon = self.daemon.clone();
        let draft_owner = self.selected_composer_draft_key();
        cx.spawn(async move |michelle, cx| {
            let stored = cx
                .background_executor()
                .spawn(async move {
                    let image_count = images.len();
                    images
                        .into_iter()
                        .enumerate()
                        .map(|(index, image)| {
                            let preview_image = Arc::new(image);
                            let bytes = preview_image.bytes.clone();
                            let response = daemon
                                .client()
                                .request(
                                    Uuid::nil(),
                                    Uuid::nil(),
                                    michelle_client::Command::StoreBlob {
                                        mime_type: preview_image.format.mime_type().to_owned(),
                                        bytes,
                                    },
                                )
                                .map_err(|error| error.to_string())?;
                            let michelle_client::ResponsePayload::BlobStored { reference, path } =
                                response
                            else {
                                return Err("the daemon returned an invalid blob response".into());
                            };
                            let extension = path
                                .extension()
                                .and_then(|extension| extension.to_str())
                                .unwrap_or("png");
                            let name = if image_count == 1 {
                                format!("image.{extension}")
                            } else {
                                format!("image-{}.{extension}", index + 1)
                            };
                            Ok::<_, String>((path, name, reference, preview_image))
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .await;
            let _ = michelle.update(cx, |michelle, cx| match stored {
                Ok(stored) => {
                    if michelle.selected_composer_draft_key() != draft_owner {
                        return;
                    }
                    let mut staged = false;
                    for (path, name, reference, preview_image) in stored {
                        staged |= michelle.stage_daemon_attachment(
                            path,
                            name,
                            false,
                            true,
                            reference,
                            Some(preview_image),
                        );
                    }
                    if staged {
                        michelle.schedule_composer_draft_save(cx);
                        cx.notify();
                    }
                }
                Err(error) => {
                    michelle.show_toast(tr!("errors.store_pasted_image", error = error));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn stage_pasted_text(&mut self, text: String, cx: &mut Context<Self>) {
        let daemon = self.daemon.clone();
        let draft_owner = self.selected_composer_draft_key();
        self.composer_ui
            .input
            .update(cx, |input, cx| input.begin_attachment_paste(cx));
        cx.notify();
        cx.spawn(async move |michelle, cx| {
            let (text, stored) = cx
                .background_executor()
                .spawn(async move {
                    let stored = (|| {
                        let upload = pasted_text_attachment_upload(&text)?;
                        let response = daemon.client().request(
                            Uuid::nil(),
                            Uuid::nil(),
                            michelle_client::Command::ImportAttachment {
                                name: "pasted-text.txt".into(),
                                upload,
                            },
                        )?;
                        let michelle_client::ResponsePayload::AttachmentStored { attachment } =
                            response
                        else {
                            anyhow::bail!("the daemon returned an invalid attachment response");
                        };
                        Ok::<_, anyhow::Error>(attachment)
                    })();
                    (text, stored)
                })
                .await;
            let _ =
                michelle.update(cx, |michelle, cx| {
                    michelle
                        .composer_ui
                        .input
                        .update(cx, |input, cx| input.finish_attachment_paste(cx));
                    cx.notify();
                    if michelle.selected_composer_draft_key() != draft_owner {
                        let Some(owner) = draft_owner else {
                            return;
                        };
                        if let crate::persistence::ComposerDraftKey::Session(session_id) = owner
                            && !michelle
                                .state
                                .sessions
                                .iter()
                                .any(|session| session.id == session_id)
                        {
                            return;
                        }
                        let mut draft = michelle
                            .composer_model
                            .drafts
                            .get(owner)
                            .cloned()
                            .unwrap_or_default();
                        match stored {
                            Ok(attachment) => draft.attachments.push(
                                crate::persistence::ComposerDraftAttachment {
                                    mention: attachment.path.display().to_string(),
                                    path: attachment.path,
                                    name: attachment.name,
                                    is_dir: attachment.is_dir,
                                    is_image: false,
                                    blob_reference: Some(attachment.reference),
                                },
                            ),
                            Err(error) => {
                                draft.text.push_str(&text);
                                michelle.show_toast(tr!("errors.store_pasted_text", error = error));
                            }
                        }
                        michelle.composer_model.drafts.set(owner, draft);
                        michelle.schedule_composer_draft_save(cx);
                        return;
                    }
                    match stored {
                        Ok(attachment) => {
                            michelle.stage_daemon_attachment(
                                attachment.path,
                                attachment.name,
                                attachment.is_dir,
                                false,
                                attachment.reference,
                                None,
                            );
                        }
                        Err(error) => {
                            // Keep the pasted content if storage failed instead of
                            // silently losing it after consuming the paste event.
                            let cursor = michelle.composer_ui.input.read(cx).cursor(cx);
                            michelle.composer_ui.input.update(cx, |input, cx| {
                                input.replace_range(cursor..cursor, &text, cx);
                            });
                            michelle.show_toast(tr!("errors.store_pasted_text", error = error));
                        }
                    }
                    michelle.schedule_composer_draft_save(cx);
                });
        })
        .detach();
    }
}
pub(in crate::app) fn pasted_text_attachment_upload(
    text: &str,
) -> anyhow::Result<michelle_client::attachments::AttachmentUpload> {
    if text.len() as u64 > MAX_ATTACHMENT_BYTES {
        anyhow::bail!("attachment is larger than 32 MB");
    }
    Ok(michelle_client::attachments::AttachmentUpload::File {
        data_base64: base64::engine::general_purpose::STANDARD.encode(text.as_bytes()),
    })
}

/// Reads a client-local drop into an upload payload. This is the explicit
/// client/daemon boundary: none of these source paths are persisted or handed
/// to a provider.
fn attachment_upload_from_path(
    source: &Path,
) -> anyhow::Result<(
    String,
    michelle_client::attachments::AttachmentUpload,
    Option<Vec<u8>>,
)> {
    let metadata = std::fs::symlink_metadata(source)
        .with_context(|| format!("could not read attachment {}", source.display()))?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "symbolic-link attachments are not supported: {}",
            source.display()
        );
    }
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow::anyhow!("attachment has no file name: {}", source.display()))?
        .to_owned();
    if metadata.is_file() {
        if metadata.len() > MAX_ATTACHMENT_BYTES {
            anyhow::bail!("attachment is larger than 32 MB: {}", source.display());
        }
        let bytes = std::fs::read(source)
            .with_context(|| format!("could not read attachment {}", source.display()))?;
        let is_image = is_image_attachment_path(source);
        return Ok((
            name,
            michelle_client::attachments::AttachmentUpload::File {
                data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
            },
            is_image.then_some(bytes),
        ));
    }
    if !metadata.is_dir() {
        anyhow::bail!(
            "attachment is not a file or directory: {}",
            source.display()
        );
    }

    let mut pending = vec![source.to_path_buf()];
    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).with_context(|| {
            format!(
                "could not read attachment directory {}",
                directory.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            if entries.len() >= michelle_client::attachments::MAX_ATTACHMENT_FILES {
                anyhow::bail!(
                    "attachment directory contains more than {} files",
                    michelle_client::attachments::MAX_ATTACHMENT_FILES
                );
            }
            total_bytes = total_bytes.saturating_add(metadata.len());
            if total_bytes > MAX_ATTACHMENT_BYTES {
                anyhow::bail!("attachment directory is larger than 32 MB");
            }
            let relative_path = path
                .strip_prefix(source)
                .context("attachment entry escaped its source directory")?
                .to_path_buf();
            let bytes = std::fs::read(&path)
                .with_context(|| format!("could not read attachment {}", path.display()))?;
            entries.push(michelle_client::attachments::AttachmentUploadEntry {
                relative_path,
                data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            });
        }
    }
    Ok((
        name,
        michelle_client::attachments::AttachmentUpload::Directory { entries },
        None,
    ))
}

pub(in crate::app) fn dropped_file_mention(
    root: Option<&std::path::Path>,
    path: &std::path::Path,
    is_dir: bool,
) -> String {
    let mention = root
        .and_then(|root| path.strip_prefix(root).ok())
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path)
        .display()
        .to_string();
    if is_dir && !mention.ends_with('/') {
        format!("{mention}/")
    } else {
        mention
    }
}

fn is_image_attachment_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "webp"
                    | "bmp"
                    | "svg"
                    | "tif"
                    | "tiff"
                    | "ico"
                    | "pnm"
                    | "pbm"
                    | "pgm"
                    | "ppm"
            )
        })
}
