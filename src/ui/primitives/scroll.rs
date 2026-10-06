use gpui::{App, Pixels, ScrollHandle, px};

/// Keeps a wheel gesture in a nested scrollable while it can consume the
/// delta, then lets it chain to the ancestor at either boundary. Call from an
/// `on_scroll_wheel` listener. GPUI's own scroll handler runs first during the
/// bubble phase, so an offset outside the clamped range means this event tried
/// to move past the top or bottom and must keep bubbling. A viewport whose
/// content fits also keeps chaining so short blocks do not dead-zone the page.
/// Stopping propagation skips wheel listeners pushed earlier on the same
/// element, so fold any sibling wheel logic into the listener that calls this.
pub fn contain_scroll(handle: &ScrollHandle, cx: &mut App) {
    if nested_scroll_consumed_delta(handle.offset().y, handle.max_offset().y) {
        cx.stop_propagation();
    }
}

fn nested_scroll_consumed_delta(offset: Pixels, max_offset: Pixels) -> bool {
    max_offset > px(0.5) && offset >= -max_offset && offset <= px(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_scroll_chains_only_after_reaching_a_boundary() {
        let max_offset = px(100.0);

        assert!(nested_scroll_consumed_delta(px(0.0), max_offset));
        assert!(nested_scroll_consumed_delta(px(-50.0), max_offset));
        assert!(nested_scroll_consumed_delta(px(-100.0), max_offset));
        assert!(!nested_scroll_consumed_delta(px(1.0), max_offset));
        assert!(!nested_scroll_consumed_delta(px(-101.0), max_offset));
        assert!(!nested_scroll_consumed_delta(px(0.0), px(0.0)));
    }
}
