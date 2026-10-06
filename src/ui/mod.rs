//! Apple-style UI tokens, icons, components and drawing primitives.
//! Application-specific provider and session presentation lives in `app`.

pub mod appearance;
pub mod components;
pub mod icons;
pub mod layout;
pub mod prelude;
pub mod primitives;
pub mod tokens;
mod ui_colors;

pub use ui_colors::UiColors;

pub use appearance::colors;
pub use components::{
    ActionRow, Badge, Button, ButtonRole, ButtonStyle, Checkbox, Dialog, DisclosureGroup, Divider,
    EmptyState, FieldMode, FormRow, IconButton, InfoRow, InlineNotice, InputEvent, KeyHint, Label,
    MediaPaste, MenuChip, Picker, ProgressBar, ProgressRing, SearchField, Section, Segment,
    SegmentedControl, SelectableRow, SidebarItem, Skeleton, StatusTone, Surface, TabBar, TabItem,
    TextArea, TextField, TextInput, Toast, Toggle, ToolbarButton, Tooltip, focus_dialog_input,
    icon_button, toolbar_button_segment, toolbar_split_button,
};
pub use components::{menu, sidebar, text_field, toolbar, tooltip};
pub use icons::{SfIcon, SfSymbol, SfSymbolWeight, SymbolWeight, file_icon, icon, sf_icon};
pub use layout::{h_stack, spacer, v_stack};
pub use primitives::{ActivationExt, contain_scroll, motion, scrollbar, squircle};
pub use tokens::{
    ControlSize, IconSize, Palette, StyledTypography, TOOLBAR_HEIGHT, TextStyle, UI_REM_SIZE, sp,
};
