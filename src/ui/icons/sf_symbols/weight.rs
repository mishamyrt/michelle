use gpui::SharedString;

const FONT_ICON_PREFIX: &str = "sf-symbols/";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SfSymbolWeight {
    #[default]
    Regular,
    Medium,
    Semibold,
    Bold,
}

impl SfSymbolWeight {
    pub(crate) const ALL: [Self; 4] = [Self::Regular, Self::Medium, Self::Semibold, Self::Bold];

    pub(crate) fn path(self, name: &str) -> SharedString {
        match self {
            Self::Regular => format!("{FONT_ICON_PREFIX}{name}").into(),
            Self::Medium => format!("{FONT_ICON_PREFIX}medium/{name}").into(),
            Self::Semibold => format!("{FONT_ICON_PREFIX}semibold/{name}").into(),
            Self::Bold => format!("{FONT_ICON_PREFIX}bold/{name}").into(),
        }
    }
}

pub(crate) fn parse_font_icon_path(path: &str) -> Option<(&str, SfSymbolWeight)> {
    let name = path.strip_prefix(FONT_ICON_PREFIX)?;
    let Some((weight, name)) = name.split_once('/') else {
        return Some((name, SfSymbolWeight::Regular));
    };
    let weight = match weight {
        "medium" => SfSymbolWeight::Medium,
        "semibold" => SfSymbolWeight::Semibold,
        "bold" => SfSymbolWeight::Bold,
        _ => return None,
    };
    Some((name, weight))
}

pub use SfSymbolWeight as SymbolWeight;
