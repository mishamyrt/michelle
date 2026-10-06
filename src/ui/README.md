# Michelle UI

Reusable GPUI components and design tokens for Michelle's macOS interface.
Start a new screen with `use crate::ui::prelude::*;`.

```rust,ignore
use crate::ui::prelude::*;

Label::new("Settings").text_style(TextStyle::Headline);

Button::new("save", "Save")
    .style(ButtonStyle::Primary)
    .size(ControlSize::Regular)
    .track_focus(&self.save_focus)
    .on_activation(cx, |this, window, cx| this.save(window, cx));

SfSymbol::new("gearshape")
    .size(IconSize::Regular)
    .weight(SymbolWeight::Semibold)
    .color(colors(cx).text_secondary);

div().text_style(TextStyle::Caption).child("Last updated today");
```

## Ownership

| Module | Owns |
| --- | --- |
| `tokens/colors` | Abstract `Palette`: label shades, neutral fills, separators and color families |
| `tokens/typography` | Text size, line height and weight by role |
| `tokens/metrics` | UI text scaling and shared control sizes, spacing and radii |
| `UiColors`, `appearance` | Resolved control colors, their in-memory snapshot and cache generation |
| `icons` | SF Symbol catalog, weights, preparation, proportions and SVG wrappers |
| `components` | Labels, buttons, fields, toggles, menus, sidebar rows and tooltips |
| `primitives` | Squircle painting, scrolling and the shared motion clock |

`Palette` is source material for themes: `Palette::light().labels.secondary`,
`Palette::dark().colors.blue`, `palette.fills.tertiary`. It has no active state,
application surfaces or theme-file schema. Each color group is independently
replaceable when composing a palette.

`src/theme/{light,dark}.rs` compose these tokens into Michelle's `Theme`.
Application roles such as canvas, sidebar, composer, terminal and usage meters
belong there. `src/theme.rs` loads and selects themes; `Theme::ui_colors()` maps
the selected theme into `UiColors` and publishes it through `ui::appearance`.
Components read `ui::colors(cx)` or `UiColors::current(cx)` and never depend on
the application theme. Explicit helper arguments accept `theme.ui_colors()`.
The complete TOML theme schema and `theme::{Theme, sp}` imports are preserved.

```rust,ignore
let palette = Palette::light();
let mut theme = crate::theme::Theme::light();
theme.text = palette.labels.primary;
theme.accent = palette.colors.blue;
theme.success = palette.colors.green;
```

Provider branding, session statuses, activity labels and the project selector
belong to `src/app/presentation`. Pass ready-to-render labels, icon names and
handlers into the reusable UI. For example, `MenuChip::icon` accepts a brand
mark without knowing which provider it represents.

## Extending the library

- Keep `mod.rs` for declarations and exports. Add implementation in a named file.
- Split a growing component by responsibility, as in `components/menu`: items,
  state/navigation, placement, attachment and rendering. Keep helpers private.
- Use shared tokens when multiple controls use the same value. Keep special
  geometry beside its component. Use GPUI layout directly for ordinary rows.
- Apply `TextStyle` through `Label` or `StyledTypography`. UI text uses `sp()`;
  editor and markdown surfaces retain their independent font settings.
- Labels and new SF Symbol builders inherit the container's foreground color
  and system font. Existing `icon`/`sf_icon` helpers retain explicit sizing/color.
- Add only used SF Symbol names to `icons/sf_symbols/catalog.rs`. The same list
  drives startup preparation and the asset listing; preserve optical exceptions
  in `macos.rs`. A frame reads prepared data and never calls AppKit.
- Stateless controls use `RenderOnce`; keep focus identities and interaction
  state with the owning entity. `Button` supports click and unmodified Enter/
  Space through `ActivationExt`, has a visible focus ring and ignores activation
  while disabled. Use `track_focus` when a form needs an explicit focus identity.
- Preserve keyboard navigation and system reduce-motion behavior. Repeating
  animation uses the shared pulse clock; see `docs/performance.md`.

Existing `ui::menu`, `ui::sidebar`, `ui::motion` and other short paths are
re-exports of the canonical modules, so current screens can migrate gradually.
The vendored squircle geometry/style implementation and license are retained.
