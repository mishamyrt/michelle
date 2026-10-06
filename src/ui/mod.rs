//! Apple-style UI tokens, icons, components and drawing primitives.
//! Application-specific provider and session presentation lives in `app`.

pub mod appearance;
pub mod components;
pub mod icons;
pub mod prelude;
pub mod primitives;
pub mod tokens;
mod ui_colors;

pub use ui_colors::UiColors;

pub use appearance::colors;
pub use components::{
    Button, ButtonStyle, FieldMode, InputEvent, Label, MediaPaste, MenuChip, TextField, TextInput,
    Tooltip, icon_button, toggle_switch, toolbar_button, toolbar_button_segment,
    toolbar_split_button,
};
pub use components::{menu, sidebar, text_field, toolbar, tooltip};
pub use icons::{SfIcon, SfSymbol, SfSymbolWeight, SymbolWeight, file_icon, icon, sf_icon};
pub use primitives::{ActivationExt, contain_scroll, motion, scrollbar, squircle};
pub use tokens::{ControlSize, IconSize, Palette, StyledTypography, TOOLBAR_HEIGHT, TextStyle, sp};
