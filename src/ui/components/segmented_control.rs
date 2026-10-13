use super::control::with_defaults;
use crate::ui::primitives::activation::with_activation;
use crate::ui::primitives::navigation::horizontal_navigation_target;
use crate::ui::squircle::SquircleExt;
use crate::ui::{ControlSize, StyledTypography, TextStyle, UiColors};
use gpui::{
    App, Context, Div, ElementId, FocusHandle, KeyDownEvent, RenderOnce, SharedString, Stateful,
    StyleRefinement, Window, div, prelude::*, px,
};
use std::rc::Rc;

pub struct Segment<T> {
    id: ElementId,
    value: T,
    label: SharedString,
    focus: FocusHandle,
}

impl<T> Segment<T> {
    pub fn new(
        id: impl Into<ElementId>,
        value: T,
        label: impl Into<SharedString>,
        focus: &FocusHandle,
    ) -> Self {
        Self {
            id: id.into(),
            value,
            label: label.into(),
            focus: focus.clone(),
        }
    }
}

type ChangeHandler<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SegmentedControl<T: Clone + PartialEq + 'static> {
    base: Stateful<Div>,
    selected: T,
    options: Vec<Segment<T>>,
    size: ControlSize,
    change: Option<ChangeHandler<T>>,
}

impl<T: Clone + PartialEq + 'static> SegmentedControl<T> {
    pub fn new(
        id: impl Into<ElementId>,
        selected: T,
        options: impl IntoIterator<Item = Segment<T>>,
    ) -> Self {
        Self {
            base: div().id(id),
            selected,
            options: options.into_iter().collect(),
            size: ControlSize::Small,
            change: None,
        }
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_change<E: 'static>(
        mut self,
        cx: &mut Context<E>,
        handler: impl Fn(&mut E, T, &mut Window, &mut Context<E>) + 'static,
    ) -> Self {
        let listener = cx.listener(move |owner, value: &T, window, cx| {
            handler(owner, value.clone(), window, cx)
        });
        self.change = Some(Rc::new(move |value, window, cx| {
            listener(&value, window, cx)
        }));
        self
    }
}

impl<T: Clone + PartialEq + 'static> Styled for SegmentedControl<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl<T: Clone + PartialEq + 'static> RenderOnce for SegmentedControl<T> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = UiColors::current(cx);
        let targets = Rc::new(
            self.options
                .iter()
                .map(|option| (option.value.clone(), option.focus.clone()))
                .collect::<Vec<_>>(),
        );
        let len = self.options.len();
        let base = with_defaults(self.base, |base| {
            base.rounded(px(7.0))
                .border_1()
                .border_color(colors.border_strong)
                .flex()
                .overflow_hidden()
        })
        .tab_group();
        base.children(self.options.into_iter().enumerate().map(|(index, option)| {
            let selected = option.value == self.selected;
            let targets = targets.clone();
            let change = self.change.clone();
            let mut segment = div()
                .id(option.id)
                .track_focus(&option.focus)
                .tab_index(0)
                .tab_stop(true)
                .h(self.size.height())
                .px(px(if self.size == ControlSize::Mini {
                    9.0
                } else {
                    11.0
                }))
                .flex()
                .items_center()
                .cursor_default()
                .text_style(TextStyle::BODY)
                .text_color(if selected {
                    colors.text
                } else {
                    colors.text_secondary
                })
                .when(index == 0, |segment| segment.rounded_l(px(6.0)))
                .when(index + 1 == len, |segment| segment.rounded_r(px(6.0)))
                .when(selected, |segment| segment.bg(colors.overlay))
                .when(!selected, |segment| {
                    segment.hover(|style| style.text_color(colors.text))
                })
                .focus_visible(|style| {
                    style.shadow(vec![
                        gpui::BoxShadow::new(px(0.0), px(0.0), colors.accent)
                            .spread_radius(px(1.0))
                            .inset(),
                    ])
                })
                .child(option.label)
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let Some(next) =
                        horizontal_navigation_target(index, targets.len(), &event.keystroke.key)
                    else {
                        return;
                    };
                    if let Some(change) = &change {
                        window.focus(&targets[next].1, cx);
                        change(targets[next].0.clone(), window, cx);
                        cx.stop_propagation();
                    }
                });
            if let Some(change) = self.change.clone() {
                segment = with_activation(
                    segment,
                    Rc::new(move |window, cx| change(option.value.clone(), window, cx)),
                );
            }
            segment.squircle()
        }))
        .squircle()
    }
}
