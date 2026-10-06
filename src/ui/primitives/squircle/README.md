# Squircle

Local copy of [brendon-felix/gpui_squircle](https://github.com/brendon-felix/gpui_squircle),
revision [`b7fb4d4`](https://github.com/brendon-felix/gpui_squircle/tree/b7fb4d4b02f003c73378e6ca12a826d9b9c6b640).
The component and styling API retain the upstream MIT license in [LICENSE](LICENSE).

```rust
use gpui::{ParentElement, Styled, div, px};
use crate::ui::squircle::{SquircleStyled, squircle};

div()
    .relative()
    .size(px(200.0))
    .child(
        squircle()
            .rounded(px(25.0))
            .rounded_smoothing(px(0.6))
            .bg(gpui::red())
            .border(px(2.0))
            .border_color(gpui::blue())
            .border_inside()
            .absolute_expand(),
    )
```

`SquircleStyled` supplies layout, text, corner, fill, and border setters for the
squircle; GPUI's `Styled` continues to style ordinary elements. Set the squircle
styles before calling `.id()`. Smoothing ranges from 0 to 1; the default is 0.6.
Each corner can have its own radius. Like GPUI rounded rectangles, this component
does not clip children to its curved outline.

Local adaptations:

- Use the GPUI revision pinned by Michelle, including its exported style macros
  and `GridTemplateMinSize` name.
- Use standard `Vec` storage instead of a direct `smallvec` dependency.
- Delegate layout and painting to GPUI's `Interactivity`, as its `Div` does,
  so mouse and keyboard listeners, tab stops, opacity, and inherited text styles
  are registered through the native element lifecycle.
- Include the corner calculations from `figma_squircle` 0.1.0, its upstream
  dependency, in `geometry.rs`. Preserve the licenses for Cameron P Campbell and
  Tien Pham. The geometry builds `PathBuilder` commands directly; it creates no
  SVG strings, parses no SVG, and uses fixed arrays instead of maps.

No Cargo dependencies are added. Geometry work is bounded by four corners and
performs no I/O or animation scheduling.
