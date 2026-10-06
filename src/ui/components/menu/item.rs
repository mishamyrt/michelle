use super::*;

/// One row of a menu.
#[derive(Clone)]
pub enum MenuItem {
    Entry {
        label: SharedString,
        tooltip: Option<SharedString>,
        icon: Option<&'static str>,
        /// A full-color raster icon — a real app icon — where `icon` would
        /// draw a tinted glyph.
        image: Option<std::sync::Arc<gpui::Image>>,
        /// Draws a leading check, for menus that present a current choice.
        selected: bool,
        /// Shown greyed and inert. Preferred over omitting the row when the
        /// action is temporarily unavailable, so the menu keeps a stable shape.
        disabled: bool,
        #[allow(clippy::type_complexity)]
        on_click: Rc<dyn Fn(&mut Window, &mut App)>,
    },
    /// Opens a one-level flyout beside the parent card. `value` keeps the
    /// current choice visible in the parent row, matching native inspector
    /// menus whose submenu is a preference rather than an action.
    Submenu {
        label: SharedString,
        value: Option<SharedString>,
        #[allow(clippy::type_complexity)]
        items: Rc<dyn Fn(&mut App) -> Vec<MenuItem>>,
    },
    /// A caller-drawn row, for choices that need more than a label — a badge, a
    /// secondary line, an inline swatch. Clickable when `on_click` is set.
    Custom {
        #[allow(clippy::type_complexity)]
        render: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
        selected: bool,
        #[allow(clippy::type_complexity)]
        on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    },
    /// A non-interactive caption grouping the rows beneath it.
    Header(SharedString),
    Separator,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self::Entry {
            label: label.into(),
            tooltip: None,
            icon: None,
            image: None,
            selected: false,
            disabled: false,
            on_click: Rc::new(on_click),
        }
    }

    /// A caller-drawn row. `render` runs on every frame the menu is open.
    pub fn custom(render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        Self::Custom {
            render: Rc::new(render),
            selected: false,
            on_click: None,
        }
    }

    pub fn submenu_with_value(
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        items: impl Fn(&mut App) -> Vec<MenuItem> + 'static,
    ) -> Self {
        Self::Submenu {
            label: label.into(),
            value: Some(value.into()),
            items: Rc::new(items),
        }
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        if let Self::Custom { on_click, .. } = &mut self {
            *on_click = Some(Rc::new(handler));
        }
        self
    }

    pub fn selected(mut self, value: bool) -> Self {
        match &mut self {
            Self::Entry { selected, .. } | Self::Custom { selected, .. } => *selected = value,
            _ => {}
        }
        self
    }

    pub fn disabled(mut self, value: bool) -> Self {
        if let Self::Entry { disabled, .. } = &mut self {
            *disabled = value;
        }
        self
    }

    pub fn tooltip(mut self, label: impl Into<SharedString>) -> Self {
        if let Self::Entry { tooltip, .. } = &mut self {
            *tooltip = Some(label.into());
        }
        self
    }

    pub fn icon(mut self, path: &'static str) -> Self {
        if let Self::Entry { icon, .. } = &mut self {
            *icon = Some(path);
        }
        self
    }

    pub fn image(mut self, value: std::sync::Arc<gpui::Image>) -> Self {
        if let Self::Entry { image, .. } = &mut self {
            *image = Some(value);
        }
        self
    }

    pub(super) fn is_focusable(&self) -> bool {
        match self {
            Self::Entry { disabled, .. } => !disabled,
            Self::Submenu { .. } => true,
            Self::Custom { on_click, .. } => on_click.is_some(),
            Self::Header(_) | Self::Separator => false,
        }
    }

    pub(super) fn click_handler(self) -> Option<Rc<dyn Fn(&mut Window, &mut App)>> {
        match self {
            Self::Entry {
                disabled: false,
                on_click,
                ..
            } => Some(on_click),
            Self::Entry { disabled: true, .. } => None,
            Self::Custom { on_click, .. } => on_click,
            Self::Submenu { .. } | Self::Header(_) | Self::Separator => None,
        }
    }
}
