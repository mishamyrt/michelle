use gpui::{Div, div, prelude::*};

pub fn h_stack() -> Div {
    div().flex().items_center()
}

pub fn v_stack() -> Div {
    div().flex().flex_col()
}

pub fn spacer() -> Div {
    div().flex_1()
}
