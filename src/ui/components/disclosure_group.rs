use gpui::{AnyElement, App, Div, RenderOnce, StyleRefinement, Window, div, prelude::*, px};

/// The owner supplies the header's focus and activation and remeasures its list.
#[derive(IntoElement)]
pub struct DisclosureGroup {
    base: Div,
    header: AnyElement,
    body: Option<AnyElement>,
}

impl DisclosureGroup {
    /// Build the body only when expanded, while the owner's render context is available.
    pub fn new(
        expanded: bool,
        header: impl IntoElement,
        body: impl FnOnce() -> AnyElement,
    ) -> Self {
        Self {
            base: div().w_full().min_w_0().flex().flex_col().gap(px(4.0)),
            header: header.into_any_element(),
            body: expanded.then(body),
        }
    }
}

impl Styled for DisclosureGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for DisclosureGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.base.child(self.header).children(self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{ActivationExt, IconButton};
    use gpui::{
        Context, FocusHandle, ListAlignment, ListState, Modifiers, Render, TestAppContext, list,
        point,
    };
    use std::{cell::Cell, rc::Rc};

    struct Harness {
        expanded: bool,
        focus: FocusHandle,
        child_focus: FocusHandle,
        bodies: Rc<Cell<usize>>,
        child_activations: usize,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let bodies = self.bodies.clone();
            let header = div()
                .id("header")
                .track_focus(&self.focus)
                .tab_index(0)
                .w_full()
                .h(px(26.0))
                .on_activation(cx, |this, _, cx| {
                    this.expanded = !this.expanded;
                    cx.notify();
                });
            DisclosureGroup::new(self.expanded, header, || {
                bodies.set(bodies.get() + 1);
                IconButton::new("nested", "Open file")
                    .track_focus(&self.child_focus)
                    .w(px(100.0))
                    .h(px(26.0))
                    .on_activation(cx, |this, _, _| this.child_activations += 1)
                    .into_any_element()
            })
        }
    }

    #[gpui::test]
    fn collapsed_body_is_lazy_and_owner_activation_preserves_focus(cx: &mut TestAppContext) {
        let bodies = Rc::new(Cell::new(0));
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            expanded: false,
            focus: cx.focus_handle(),
            child_focus: cx.focus_handle(),
            bodies: bodies.clone(),
            child_activations: 0,
        });
        let focus = view.read_with(cx, |view, _| view.focus.clone());
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("cmd-enter alt-space");
        assert_eq!(bodies.get(), 0);
        cx.simulate_keystrokes("enter");
        assert!(view.read_with(cx, |view, _| view.expanded));
        assert!(bodies.get() > 0);
        assert!(cx.update(|window, _| focus.is_focused(window)));
        cx.simulate_click(point(px(20.0), px(40.0)), Modifiers::none());
        view.read_with(cx, |view, _| {
            assert_eq!(view.child_activations, 1);
            assert!(view.expanded);
        });
        let child_focus = view.read_with(cx, |view, _| view.child_focus.clone());
        cx.update(|window, cx| window.focus(&child_focus, cx));
        cx.simulate_keystrokes("space");
        view.read_with(cx, |view, _| {
            assert_eq!(view.child_activations, 2);
            assert!(view.expanded);
        });
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_keystrokes("space");
        let before = bodies.get();
        assert!(!view.read_with(cx, |view, _| view.expanded));
        cx.update(|window, _| window.refresh());
        assert_eq!(bodies.get(), before);
    }

    struct ListHarness {
        state: ListState,
        expanded: bool,
        builds: Rc<Cell<usize>>,
        bodies: Rc<Cell<usize>>,
    }

    impl Render for ListHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let expanded = self.expanded;
            let builds = self.builds.clone();
            let bodies = self.bodies.clone();
            div().w(px(300.0)).h(px(286.0)).child(
                list(self.state.clone(), move |index, _, _| {
                    builds.set(builds.get() + 1);
                    DisclosureGroup::new(expanded, div().id(("header", index)).h(px(26.0)), || {
                        bodies.set(bodies.get() + 1);
                        div()
                            .h(px(48.0))
                            .child("Prepared output")
                            .into_any_element()
                    })
                    .into_any_element()
                })
                .size_full(),
            )
        }
    }

    #[gpui::test]
    fn long_disclosure_lists_build_only_visible_rows_and_bodies(cx: &mut TestAppContext) {
        for expanded in [false, true] {
            for count in [100, 10_000] {
                let state = ListState::new(count, ListAlignment::Top, px(0.0));
                state.reset_with_uniform_height(count, px(if expanded { 78.0 } else { 26.0 }));
                let builds = Rc::new(Cell::new(0));
                let bodies = Rc::new(Cell::new(0));
                let (_, cx) = cx.add_window_view(|_, _| ListHarness {
                    state: state.clone(),
                    expanded,
                    builds: builds.clone(),
                    bodies: bodies.clone(),
                });
                for distant in [false, true] {
                    builds.set(0);
                    bodies.set(0);
                    cx.update(|window, _| {
                        if distant {
                            state.scroll_to_reveal_item(count - 1);
                        }
                        window.refresh();
                    });
                    assert!(builds.get() > 0 && builds.get() <= 22);
                    assert_eq!(bodies.get(), if expanded { builds.get() } else { 0 });
                }
            }
        }
    }
}
