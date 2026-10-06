use gpui::{BoxShadow, Div, ElementId, FontWeight, IntoElement, Stateful, div, prelude::*, px};

use crate::ui::{IconSize, TextStyle, UiColors};
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

/// Presentation only: callers own selection, focus handles, and activation.
/// Content may also be an inline rename field or a session's text block.
pub fn sidebar_item(
    id: impl Into<ElementId>,
    icon: &'static str,
    content: impl IntoElement,
    props: SidebarItemProps,
    theme: &UiColors,
) -> Stateful<Div> {
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

    div()
        .id(id)
        .w_full()
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
        .text_size(TextStyle::Body.size())
        .text_color(theme.text)
        .font_weight(if props.selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
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

pub fn sidebar_section_header(content: impl IntoElement, theme: &UiColors) -> Div {
    div()
        .w_full()
        .h(px(SIDEBAR_SECTION_HEADER_HEIGHT))
        .flex_none()
        .pb(px(2.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(TextStyle::Caption.size())
        .line_height(px(SIDEBAR_SECTION_HEADER_HEIGHT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_secondary)
        .child(div().flex_1().min_w_0().truncate().child(content))
}
