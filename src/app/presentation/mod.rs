mod activity;
mod project_selector;
mod providers;

pub use activity::{activity_icon, activity_noun, localized_session_title, status_color};
pub use project_selector::ProjectNameSelector;
pub(in crate::app) use project_selector::project_label;
pub use providers::{provider_color, provider_icon, provider_mark};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_referenced_icon_is_available() {
        use crate::assets::Assets;
        use crate::model::{ActivityKind, ProviderKind, RuntimeMode};
        use gpui::AssetSource;

        let assets = Assets::default();
        assets.prepare_system_icons().unwrap();
        let mut paths = assets.list("").unwrap();
        for provider in ProviderKind::ALL {
            paths.push(provider_icon(provider).into());
        }
        for mode in RuntimeMode::ACCESS_OPTIONS {
            paths.push(mode.icon().into());
        }
        for kind in [
            ActivityKind::Reasoning,
            ActivityKind::Command,
            ActivityKind::FileChange,
            ActivityKind::FileRead,
            ActivityKind::FileSearch,
            ActivityKind::FileList,
            ActivityKind::Search,
            ActivityKind::Plan,
            ActivityKind::Tool,
        ] {
            paths.push(activity_icon(kind).into());
        }
        for path in paths {
            assert!(
                assets.load(&path).unwrap().is_some(),
                "missing icon: {path}"
            );
        }
    }
}

/// Collapse provider- or page-supplied text into a label that cannot contain
/// hard line breaks. GPUI's `truncate()` prevents wrapping, but explicit
/// newlines still produce multiple visual lines.
pub(in crate::app) fn single_line_label(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
