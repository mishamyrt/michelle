//! Shared keyboard cursor movement for filtered pickers.
/// Where the picker's keyboard cursor lands, wrapping at both ends.
///
/// `None` for `current` means the cursor has not moved yet, so `down` opens on
/// the first row and `up` on the last. `None` in the result means the key does
/// not navigate.
pub(crate) fn next_picker_highlight(
    current: Option<usize>,
    len: usize,
    key: &str,
) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match key {
        "down" => Some(current.map_or(0, |index| (index + 1) % len)),
        "up" => Some(current.map_or(len - 1, |index| (index + len - 1) % len)),
        _ => None,
    }
}

pub(crate) fn horizontal_navigation_target(current: usize, len: usize, key: &str) -> Option<usize> {
    if len == 0 || current >= len {
        return None;
    }
    match key {
        "left" => Some(current.saturating_sub(1)),
        "right" => Some((current + 1).min(len - 1)),
        "home" => Some(0),
        "end" => Some(len - 1),
        _ => None,
    }
}
