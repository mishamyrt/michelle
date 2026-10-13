use crate::ui::squircle::SquircleExt;
use gpui::{
    AnyElement, App, BoxShadow, Context, Div, ElementId, FocusHandle, Interactivity, IntoElement,
    RenderOnce, Stateful, StyleRefinement, Window, div, prelude::*, px,
};

use super::control::with_defaults;
use crate::ui::primitives::activation::{ActivationHandler, activation_listener, with_activation};
use crate::ui::{ActivationExt, IconSize, StyledTypography, TextStyle, UiColors};
use crate::ui::{
    SfSymbolWeight, sf_icon,
    squircle::{SquircleStyled, squircle},
};

pub const SIDEBAR_ITEM_HEIGHT: f32 = 32.0;
pub const SIDEBAR_SECTION_HEADER_HEIGHT: f32 = 16.0;
const ITEM_PADDING: f32 = 8.0;
const INDENT_STEP: f32 = 9.0;
const DISCLOSURE_WIDTH: f32 = 12.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SidebarDisclosure {
    #[default]
    None,
    Reserved,
    Collapsed,
    Expanded,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SidebarItemProps {
    pub selected: bool,
    pub depth: usize,
    pub disclosure: SidebarDisclosure,
    pub icon_size: IconSize,
    pub icon_weight: SfSymbolWeight,
}

#[derive(IntoElement)]
pub struct SidebarItem {
    base: Stateful<Div>,
    icon: &'static str,
    content: AnyElement,
    props: SidebarItemProps,
    activate: Option<ActivationHandler>,
}

impl SidebarItem {
    pub fn new(id: impl Into<ElementId>, icon: &'static str, content: impl IntoElement) -> Self {
        Self {
            base: div().id(id),
            icon,
            content: content.into_any_element(),
            props: SidebarItemProps::default(),
            activate: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.props.selected = selected;
        self
    }

    pub fn depth(mut self, depth: usize) -> Self {
        self.props.depth = depth;
        self
    }

    pub fn disclosure(mut self, disclosure: SidebarDisclosure) -> Self {
        self.props.disclosure = disclosure;
        self
    }

    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.base = self.base.track_focus(focus);
        self
    }
}

impl Styled for SidebarItem {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for SidebarItem {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ActivationExt for SidebarItem {
    fn on_activation<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        self.activate = Some(activation_listener(cx, handler));
        self
    }
}

impl RenderOnce for SidebarItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = with_defaults(self.base, |base| {
            build_sidebar_item(
                base,
                self.icon,
                self.content,
                self.props,
                &UiColors::current(cx),
            )
        })
        .tab_index(0)
        .tab_stop(true);
        match self.activate {
            Some(handler) => with_activation(base, handler),
            None => base,
        }
        .squircle()
    }
}

/// Presentation only: callers own selection, focus handles, and activation.
/// Content may also be an inline rename field or a session's text block.
pub fn sidebar_item(
    id: impl Into<ElementId>,
    icon: &'static str,
    content: impl IntoElement,
    props: SidebarItemProps,
    theme: &UiColors,
) -> Stateful<crate::ui::squircle::SquircleElement<Div>> {
    build_sidebar_item(div().squircle().id(id), icon, content, props, theme)
}

fn build_sidebar_item<E: Styled + StatefulInteractiveElement + ParentElement + IntoElement>(
    base: E,
    icon: &'static str,
    content: impl IntoElement,
    props: SidebarItemProps,
    theme: &UiColors,
) -> E {
    let leading = div()
        .flex_none()
        .flex()
        .gap(px(2.0))
        .items_center()
        .when(props.disclosure != SidebarDisclosure::None, |leading| {
            leading.child(
                div()
                    .w(px(DISCLOSURE_WIDTH))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(props.disclosure == SidebarDisclosure::Collapsed, |slot| {
                        slot.child(
                            sf_icon("chevron.right", 9.0, theme.text_secondary)
                                .weight(SfSymbolWeight::Semibold),
                        )
                    })
                    .when(props.disclosure == SidebarDisclosure::Expanded, |slot| {
                        slot.child(
                            sf_icon("chevron.down", 9.0, theme.text_secondary)
                                .weight(SfSymbolWeight::Semibold),
                        )
                    }),
            )
        })
        .child(
            div()
                .w(px(props.icon_size.max_width()))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    sf_icon(icon, props.icon_size.font_size(), theme.accent)
                        .weight(props.icon_weight),
                ),
        );

    let left_padding = if props.disclosure != SidebarDisclosure::None {
        props.depth as f32 * INDENT_STEP
    } else {
        ITEM_PADDING + props.depth as f32 * INDENT_STEP + props.icon_size.max_width()
    };

    base.w_full()
        .min_w_0()
        .h(px(SIDEBAR_ITEM_HEIGHT))
        .flex_none()
        .relative()
        .pl(px(left_padding))
        .pr(px(ITEM_PADDING))
        .rounded(px(7.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        .cursor_default()
        .text_color(theme.text)
        .text_style(if props.selected {
            TextStyle::BODY.bold()
        } else {
            TextStyle::BODY
        })
        .when(props.selected, |item| {
            item.child(
                squircle()
                    .rounded(px(7.5))
                    .bg(theme.row_selection)
                    .absolute_expand(),
            )
        })
        .focus_visible(|style| {
            style.shadow(vec![
                BoxShadow::new(px(0.0), px(0.0), theme.accent)
                    .spread_radius(px(1.0))
                    .inset(),
            ])
        })
        .child(leading)
        .child(div().flex_1().min_w_0().truncate().child(content))
}

pub fn sidebar_section_header(
    content: impl IntoElement,
    theme: &UiColors,
) -> crate::ui::squircle::SquircleElement<Div> {
    div()
        .w_full()
        .h(px(SIDEBAR_SECTION_HEADER_HEIGHT))
        .flex_none()
        .pb(px(2.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_style(TextStyle::SUBHEADLINE.bold())
        .text_color(theme.text_secondary)
        .child(div().flex_1().min_w_0().truncate().child(content))
        .squircle()
}
