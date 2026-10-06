use std::rc::Rc;

use gpui::{App, Context, Element, Hitbox, KeyDownEvent, Stateful, Window, prelude::*};

pub(crate) type ActivationHandler = Rc<dyn Fn(&mut Window, &mut App)>;

pub(crate) fn activation_listener<E: 'static>(
    cx: &mut Context<E>,
    activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
) -> ActivationHandler {
    let listener = cx.listener(move |this, _: &(), window, cx| activate(this, window, cx));
    Rc::new(move |window, cx| listener(&(), window, cx))
}

pub(crate) fn with_activation<E: StatefulInteractiveElement>(
    element: E,
    activate: ActivationHandler,
) -> E {
    let click_activate = activate.clone();
    element
        .on_click(move |_, window, cx| {
            click_activate(window, cx);
            cx.stop_propagation();
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            // Modified chords belong to the command that owns them.
            if !event.keystroke.modifiers.modified()
                && matches!(event.keystroke.key.as_str(), "enter" | "space")
            {
                activate(window, cx);
                cx.stop_propagation();
            }
        })
}

/// Add conventional mouse and keyboard activation to a focusable element.
pub trait ActivationExt: Sized {
    fn on_activation<E>(
        self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self
    where
        E: 'static;
}

impl<T: InteractiveElement + Element> ActivationExt for Stateful<T> {
    fn on_activation<E>(
        self,
        cx: &mut Context<E>,
        activate: impl Fn(&mut E, &mut Window, &mut Context<E>) + 'static,
    ) -> Self
    where
        E: 'static,
    {
        with_activation(self, activation_listener(cx, activate))
    }
}

impl<E: Element<PrepaintState = Option<Hitbox>> + Styled + StatefulInteractiveElement> ActivationExt
    for crate::ui::squircle::SquircleElement<E>
{
    fn on_activation<T: 'static>(
        self,
        cx: &mut Context<T>,
        activate: impl Fn(&mut T, &mut Window, &mut Context<T>) + 'static,
    ) -> Self {
        with_activation(self, activation_listener(cx, activate))
    }
}
