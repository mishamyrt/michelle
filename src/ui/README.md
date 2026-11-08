# Michelle UI

Reusable GPUI components and design tokens for Michelle's macOS interface.
Start a new screen with `use crate::ui::prelude::*;`.

```rust,ignore
use crate::ui::prelude::*;

Label::new("Settings").text_style(TextStyle::Headline);

Button::new("save", "Save")
    .button_style(ButtonStyle::Primary)
    .control_size(ControlSize::Regular)
    .track_focus(&self.save_focus)
    .on_activation(cx, |this, window, cx| this.save(window, cx));

IconButton::new("refresh", SfSymbol::new("arrow.clockwise"))
    .track_focus(&self.refresh_focus)
    .tooltip("Refresh")
    .loading(self.pending)
    .disabled(self.pending)
    .on_activation(cx, |this, window, cx| this.refresh(window, cx));

Toggle::new("render-math", self.render_math)
    .track_focus(&self.math_focus)
    .on_change(cx, |this, enabled, _, cx| this.set_render_math(enabled, cx));

ToolbarButton::new("info", "info.circle")
    .track_focus(&self.info_focus)
    .tooltip("Information")
    .selected(self.info_open)
    .on_activation(cx, |this, window, cx| this.toggle_info(window, cx));

SfSymbol::new("gearshape")
    .size(IconSize::Regular)
    .weight(SymbolWeight::Semibold)
    .color(colors(cx).text_secondary);

div().text_style(TextStyle::Caption).child("Last updated today");
```

## Exported components and consumers

Control builders are exported through `ui::components`, `ui` and `ui::prelude`.
Menu types use the module paths noted below. The prelude also includes
`ActivationExt`, tokens, colors, icons, layout helpers and the GPUI traits needed
for local refinements.

| API | Current consumers |
| --- | --- |
| `Button`, `IconButton`, `ToolbarButton` | [Settings](../app/settings.rs), [toolbar](../app/toolbar.rs), [FindBar](../app/transcript_search.rs), [composer](../app/composer.rs), [transcript](../app/components.rs) |
| `Label`, `Divider`, `FormRow`, `Section`, `Surface`, `InfoRow`, `EmptyState` | Settings, [Skills](../app/skills_page.rs), [right panel](../app/right_panel.rs) |
| `TextField`, `SearchField`, `TextArea` | Settings, Skills, FindBar, [commit](../app/commit_dialog.rs) and [goal](../app/goal_dialog.rs) dialogs |
| `Toggle`, `Checkbox`, `ActionRow`, `KeyHint`, `Dialog` | Settings, Skills and commit/goal dialogs |
| `MenuChip`, `Picker`, `MenuItem`, `ContextMenuHandle` | Settings and [model](../app/model_picker.rs), [project](../app/project_picker.rs), [branch](../app/branch_picker.rs) pickers |
| `SidebarItem`, `SelectableRow`, `Segment`, `SegmentedControl`, `TabItem`, `TabBar` | Settings navigation, pickers, [Usage](../app/usage_page.rs), right-panel tabs |
| `DisclosureGroup` | [Transcript activity](../app/transcript_view.rs) |
| `Badge`, `InlineNotice`, `ProgressBar`, `ProgressRing`, `Skeleton`, `Toast` | [Daemon settings](../app/settings.rs), Usage, [usage meter](../app/usage_meter.rs), [notifications](../app/notifications.rs) |
| `Tooltip` | Shared controls and application-owned actions |

`TextInput` and its events remain the existing input backend, re-exported for
field consumers. `MenuItem`, `ContextMenuHandle` and menu helpers live under
`ui::components::menu` and `ui::menu`; the prelude imports them directly.

## Ownership

| Module | Owns |
| --- | --- |
| `tokens/colors` | Abstract `Palette`: label shades, neutral fills, separators and color families |
| `tokens/typography` | Text size, line height and weight by role |
| `tokens/metrics` | Fixed UI rem base and shared control sizes, spacing and radii |
| `UiColors`, `appearance` | Resolved control colors, their in-memory snapshot and cache generation |
| `icons` | SF Symbol catalog, weights, preparation, proportions and SVG wrappers |
| `components` | Labels, buttons, fields, toggles, menus, sidebar rows and tooltips |
| `layout`, form components | Ordinary GPUI stacks, cards, dividers, section headings and form rows |
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
- Apply fixed macOS `TextStyle` roles through `Label` or `StyledTypography`.
  New components and their shared helpers use `px()`/metric tokens for geometry
  from the start. Neither direct nor transitive `sp()` dependencies belong in
  new UI. `sp()` is excluded from `ui::prelude` and remains available only for
  legacy callers. Content and code settings never resize interface labels,
  icons or controls.
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
re-exports of the canonical modules used by current screens.
The vendored squircle geometry/style implementation and license are retained.
Rounded component surfaces use that geometry at their existing corner radii;
their native GPUI layout and control states remain in place. Circles retain
native painting. See [the primitive notes](primitives/squircle/README.md).

## Typography and content sizing

UI inherits `.SystemUIFont`. Role metrics match AppKit's preferred macOS fonts
in logical points; GPUI handles the display scale. Emphasized variants use
semibold, except the headline, which already uses the system's bold weight.

| Role | Size | Line height | Weight |
| --- | --- | --- | --- |
| Caption (`caption1`) | 10 | 13 | Regular |
| Body | 13 | 16 | Regular |
| Headline | 13 | 16 | Bold |
| Title3 | 15 | 20 | Regular |
| Title2 | 17 | 22 | Regular |
| Title1 | 22 | 26 | Regular |

`UI_REM_SIZE` is always 14, preserving authored `sp()` dimensions. It is a
compatibility unit, not the body font size or a user preference. Migrate old
`sp()` callers gradually without changing the helper's return type. This
compatibility path is not an intermediate implementation for new components.
SF Symbols, SVG/file icons and spinners already use fixed pixel dimensions.

The application owns `content_font_size` for messages, reasoning, documents
and prompt text; `code_font_size` independently controls code surfaces.
It passes resolved pixel metrics into Markdown and input containers. All
multi-line fields, including auto-height composer input, inherit those metrics.
Search and settings fields use fixed UI roles. Toasts use fixed UI metrics even
when Markdown provides text selection. Image zoom and terminal metrics remain
independent. Changes to content size remeasure transcript rows without changing
the window rem base or resetting fixed sidebar/control geometry.

The serialized `ui_font_size` key is retained for `content_font_size` in both
app settings and legacy state. Existing values survive save/load and a code
rollback; the old application will again apply them to its interface.

## Control contract

Use `Button::button_style`, `control_size` and `role`. `ButtonStyle` has Primary,
Secondary and Ghost; `ButtonRole` has Normal and Destructive. The role expresses
the action's meaning separately from its visual style. `Secondary + Destructive`
uses the danger foreground and soft danger fill. A destructive primary button
uses the danger fill, and a destructive ghost button uses the danger foreground.
Calling `role` before or
after `button_style` has the same result.

```rust,ignore
Button::new("delete", "Delete")
    .button_style(ButtonStyle::Secondary)
    .role(ButtonRole::Destructive)
    .control_size(ControlSize::Small)
    .track_focus(&self.delete_focus)
    .on_activation(cx, |this, _, cx| this.confirm_delete(cx));
```

The removed Button aliases no longer shadow GPUI `Styled::style` or `.size(px(...))`.
Use `ToolbarButton::new` and `Toggle::new`; constructor and switch compatibility
wrappers were removed after migrating their remaining callers and tests.

The style-resolution contract is: defaults,
then size/style/role, then explicit builder and GPUI refinements, then state
styles. Preserve caller hover/active refinements. State styling must keep the
focus indicator visible and cannot enable a disabled action. Layout/style
merging moves only `StyleRefinement`; identities, children and event handlers
stay on the original element. Button, IconButton, Toggle, ToolbarButton,
MenuChip and TextField preserve explicit geometry, padding, typography and
base colors. Button/ToolbarButton hover fills blend the resolved solid fill.

Use the controls' inherent `hover`, `active` and `focus_visible` methods for
state styles: they merge caller refinements before installing each GPUI state
once. Fully qualified native state-style trait calls bypass this merge and are
unsupported on these controls: pinned GPUI permits only one hover style.
Keep the focus ring when customizing hover/active shadows; GPUI applies those
states after focus. `focus_visible` appends the accent ring to custom shadows.
TextField keeps an accent border while its input is focused, even when its
base border color is overridden; editing and read-only state belong to TextInput.

Owners retain stable focus handles and interaction state; controls propose
changes through handlers. Disabled activation is resolved at render time, so
builder order cannot re-enable it. `loading` alone will not disable an action.
Button, IconButton and ToolbarButton render a shared reduce-motion-aware
spinner while loading. Disabled controls retain focus and show 55% opacity;
their semantic handler is omitted. Tooltip builders use the shared show delay.
Toggle's `on_change` proposes the inverse of the rendered value; the owner
keeps storage, pending-operation guards and notifications. `on_activation`
remains available when the owner must read state at event time.

`icon_button`, `toolbar_button_segment` and `toolbar_split_button` remain used
for raw menu triggers and the split toolbar's prepared halves. They keep their
GPUI return types and caller-owned handlers. Use the builders for ordinary
controls; raw menu attachments continue to use `ContextMenuHandle` for focus
and keyboard opening. ToolbarButton paints its surface and action on one layout node,
so caller width/height apply to the complete target. The split toolbar surface
keeps its existing menu composition.

Raw mouse handlers and menu attachments retain their caller-owned guards:
`MenuChip::disabled` gates `on_activation`, not handlers added by `dropdown_menu`
or `popover`. Use `Picker` for a disabled choice menu: it passes availability
to the shared menu assembly, gating both mouse and keyboard opening. A TextField
shell does not pretend to disable input by changing opacity.

Keep keyboard conventions, focus visibility and reduce-motion when extending
controls. Theme changes publish the complete in-memory `UiColors` snapshot
before refreshing and increment its generation. Add status colors together
with their first consumer; the existing TOML schema remains unchanged.

Remove a compatibility API only after searching source, tests and documentation.
Keep helpers used for distinct native compositions, including `sidebar_item`,
`sidebar_section_header`, menu/popover attachment and split toolbar surfaces.
Keep application-specific layouts in their screens; a plain `div()` does not
need a component wrapper. Do not add new compatibility aliases for builders.

## Forms and small choice menus

General and Appearance settings use these compositions. `h_stack`, `v_stack`
and `spacer` return ordinary `Div`s. `Surface` is a raised, rounded, unpadded
card; set padding, border or background with GPUI refinements. Grouped
`FormRow`s own their padding. `Section` supplies a heading, optional description
and children without adding a card. `FormRow` supplies the wrapping label column,
optional leading content and an independent control; it has no row-wide action.
Keep controls from shrinking with `flex_none` (already set by Picker and Toggle).
The parent places `Divider::horizontal().inset(px(20.0))` between rows.

```rust,ignore
Surface::new().child(
    FormRow::new("Render math expressions")
        .description("Show formatted formulas in responses")
        .control(
            Toggle::new("render-math", self.render_math)
                .on_change(cx, |this, enabled, _, cx| this.set_render_math(enabled, cx)),
        ),
);

Picker::new(
    "scheme", selected_label, "scheme-menu", &self.scheme_menu,
    move |_| items.clone(), // Selected MenuItems and handlers belong to the owner.
)
.w(px(116.0))
.disabled(self.pending);

SearchField::new("search", self.search.clone());
```

Picker composes an outlined `MenuChip` and the existing dropdown without an
extra layout node. It retains the supplied trigger/menu IDs and handle; menu
items are built only while open. `align` defaults to `BelowRight` and
`control_size` sets the trigger height. Selected MenuItems and their event-time
guards remain caller-owned. Disabled preserves trigger focus and blocks both
opening handlers; modified Enter/Space chords do not open anchored menus.
When disabling an already open picker, close its handle in the owner's
state-change handler, then notify. Render never closes or changes menu state.
Legacy dropdown/popover helpers keep their signatures and remain enabled.

SearchField adds the search icon to TextField using the same input and layout
nodes. It keeps the owner's selection, focus, undo, clipboard and input mode.
Search behavior and placeholder updates remain with the owner. Form labels use
fixed Headline/Body metrics; content size and theme do not change their geometry.
Surface does not clip child paint to its rounded corners; edge children with
backgrounds need matching radii.

FormRow's `label_content` and `description_content` accept existing elements
for provider versions, detection details or a status beside the label. Plain
descriptions and custom descriptions replace each other in builder order.
InfoRow supplies a baseline-aligned label/value pair and optional trailing
children. Its `label_width` sets the column shared by the parent's rows (Skills
uses 84px, daemon credentials 80px). EmptyState accepts a title, description,
icon and action, with GPUI refinements for compact placeholders. These
compositions read prepared values and never fetch their own data.

## Actions and dialogs

Commit and goal dialogs share ActionRow, KeyHint, TextArea and Dialog.
ActionRow has a leading icon or loading indicator, a truncating label and
optional trailing content. `role(ButtonRole::Destructive)` supplies the danger
foreground. Loading does not disable activation. Disabled rows retain their
owner's focus identity, omit activation and leave the Tab order, preserving
the dialogs' previous policy and loading/disabled colors. Geometry and colors
use the existing rows' metrics; text uses fixed Body metrics. KeyHint displays
an already resolved platform shortcut and registers no binding.

Checkbox is controlled: `on_change` proposes the inverse of the rendered value,
and `on_activation` lets an owner read the current model. The label shares the
hit target; trailing counts remain separate content. Disabled omits activation
and the Tab stop while retaining focus. `checked_background` can match the
surrounding surface. Owners keep operation guards and storage.

TextArea wraps an existing multi-line TextInput in the dialog's 112px viewport.
Pass content metrics through `text_size` and `line_height`; padding and height
can be refined with GPUI. Editing, read-only, selection, scrolling, IME,
clipboard and undo belong to the input. The shell neither changes input mode
nor updates content during render.

Dialog owns the deferred layer, scrim, card, header, body children and actions.
It shares Surface's in-memory defaults on the same card node, preserving the
existing 420px width, 18px radius and deferred priority 4. Set the card background
and scrim color explicitly for application surfaces. Mouse down inside the card
stops propagation; `on_dismiss` handles the scrim. Attach native action handlers
and `key_context` to the card for Escape and confirm shortcuts; keep those
bindings and event-time availability checks with the owner. Header, body and
actions add no wrapper entities or extra layout nodes.

Call `focus_dialog_input` when opening, with the input's focus and a predicate
checking the owner's current dialog identity. It focuses after two frames,
only while the owner and that dialog still exist. A replacement request should
invalidate the previous predicate. Owners restore composer focus on close and
decide how to dismiss drafts; closing the shell never cancels their operation.

Daemon, provider and computer-use forms use Surface/Section/FormRow with their
existing models, validations and visibility. Skills uses SearchField, InfoRow,
EmptyState and keyboard-operable Button actions, retaining delete confirmation,
virtualization, selection and Markdown caches. Plain form headings/descriptions
use Headline/Body; content and code sizing remain independent. Subject status
badges and unusual local geometry stay in their screens.

## Navigation and prepared rows

Settings navigation uses `SidebarItem`, a builder over the existing sidebar
geometry, disclosure and selection. The legacy `sidebar_item` signature remains
available. Supply the owner's stable focus and `on_activation`; arrow navigation,
page filtering and persistence stay in Settings.

`SelectableRow` accepts prepared content plus optional leading/trailing elements.
`selected` draws a checkmark; `highlighted` paints the keyboard cursor independently.
`checkmark_at_start` reserves the model and project menus' leading checkmark column.
Branch rows use the trailing mark. Local geometry and state refinements remain
explicit; model rows retain their squircle highlight and inherited foreground.
Disabled rows keep the ghost foreground and omit semantic activation and pointer
state styles. Raw handlers keep their owner's guards. Picker search fields and
cursor actions own keyboard navigation; rows do not acquire new focus entities.
A supplied `track_focus` registers a Tab stop for independently focused rows.

Use stable data IDs, including the provider and favorites/catalog section for
models which appear twice. Selection, favorites, filtering, ranking, row snapshots,
`ListState` and scrollbars stay with the owner. A row never prepares a collection.

```rust,ignore
SelectableRow::new(SharedString::from(format!("project-{project_id}")), project_name)
    .selected(is_current_project)
    .highlighted(is_keyboard_cursor)
    .on_activation(cx, |this, _, cx| this.choose_project(cx));

SegmentedControl::new("usage-views", selected_view, [
    Segment::new("daily", daily_view, "Daily", &self.daily_focus),
    Segment::new("monthly", monthly_view, "Monthly", &self.monthly_focus),
])
.on_change(cx, |this, value, _, cx| this.set_view(value, cx));
```

`Segment` stores the owner's stable ID, value, label and focus. SegmentedControl
proposes a value on click/Enter/Space, arrows and Home/End. Arrows stop at the ends;
modified chords propagate. Usage uses Small (26px) for views and Mini (22px) for
metrics/breakdowns. Fixed Body typography and matching inset corner radii keep
segment backgrounds inside the rounded border. Loading/data operations remain
in Usage's model.

`TabBar::new(id, &scroll_handle).tab(TabItem::new(id, label, &focus))` renders the
existing horizontal scroller on one node. The owner supplies selected tabs,
leading icons, optional `dirty_indicator` and `close_button` elements, and separate
activation handlers. Left/Right/Home/End select and focus tabs, including when a
nested close control has focus. Nested actions use their own stable focus and
ActivationExt, which stops propagation for click and Enter/Space.

Right-panel tab IDs use surface identity; callbacks resolve the current index at
the event. Existing reveal, scroll fades, unsaved status and terminal operations
stay in the application. Keyboard tab selection keeps focus in the bar; closing
by keyboard focuses the active remaining tab or composer. Close controls retain
the 10px icon with a 24px hit target. The bar, tabs and controls add no entities,
I/O, timers or recurring animation. Their labels use fixed Body metrics; long
labels still truncate on one line.

`DisclosureGroup::new(expanded, header, body_factory)` composes a prepared header
and a lazy body on the same layout node as the previous activity cluster. The
factory runs during construction only when expanded, so it may borrow the
owner's render context without a stored closure or another entity. Construct
heavy Markdown, output and diff elements inside the factory, not before it.
Local styles refine the defaults directly.

```rust,ignore
let header = div()
    .id("activity-toggle")
    .track_focus(&self.activity_focus)
    .tab_index(0)
    .on_activation(cx, |this, _, cx| this.toggle_activity(cx));
DisclosureGroup::new(expanded, header, || {
    self.render_activity_body(window, cx)
});
```

The screen owns header activation, arrow keys, focus treatment and expanded
state. Transcript activity toggles retain scroll pinning and row remeasurement;
the group owns no scroll handle, cache, animation or notification. Expanded
activity items still use the existing reasoning, output, diff and selection paths.

Transcript FindBar uses `SearchField` around its existing query entity. Copy,
fork and revert footers use stable owner focus handles and `IconButton`; actions
remain visible without hover. Removing the old cross-row hover tracking also
removes its root notifications. Composer send/stop/queue and right-panel preview
and diff refresh use the same activation contract, retaining submission guards,
loading spinners and local hit geometry. Onboarding uses `Button`; right-panel
empty states use `EmptyState`. The empty state adds one layout wrapper and uses
the shared Headline/Body labels and 420px description limit.

## Status, progress and loading

Badge and InlineNotice accept ready labels/content, optional icons and a
StatusTone (Neutral, Success, Warning or Danger). InlineNotice also accepts an
independent action element. Tone resolves the foreground from the in-memory
UiColors snapshot; explicit GPUI styles override defaults. `success` and
`warning` project the theme's existing roles without changing its TOML schema.
Daemon's status is the first Badge consumer; Usage notices retain their wording,
visibility and local foreground refinements. Labels use fixed Body metrics.

```rust,ignore
Badge::new("Online").tone(StatusTone::Success);
InlineNotice::new("Some records could not be loaded").tone(StatusTone::Warning);
ProgressBar::new(Some(0.5)).fill_color(colors(cx).accent);
ProgressRing::new(None).size(px(13.0));
```

ProgressBar and ProgressRing receive an optional fraction, never a percent or
provider metric. None remains unknown; finite zero remains known zero. NaN and
infinities render only the track. Finite fractions clamp to 0–1 before any width
or arc calculation. Labels, thresholds, formatting and provider colors stay with
the owner. `minimum_fill` is an explicit visual policy, normalized separately;
it changes the cue without changing the source metric. Usage keeps its 1.5%
positive quota sliver and the context ring's 5% minimum cue, including measured
zero, while an unknown ring has no arc. The ring defaults to a 13px canvas and
2.5px stroke; GPUI size refinements change the drawing bounds.

`ProgressBar::segments` accepts ready `(fraction, color)` pairs. The application
computes totals and shares. Invalid segment fractions suppress the fill; zero
segments are omitted. First and last visible segments have their own matching
corner radii, since GPUI's overflow clip is rectangular. Bars use two layout
nodes plus visible segments; a ring uses one canvas and the existing two-arc
stroke construction. Neither progress component animates unknown values.

Skeleton is a static block with ordinary GPUI dimensions, radius and background
refinements. Its default height is 9px. The owner composes the silhouette, then
`Skeleton::pulse(group)` animates the prepared Styled group on the same layout
node. It shares the existing 1400ms pulse, 0.45–0.9 opacity and 15Hz cadence;
blocks have no timers or individual animation leases. Spinner retains the
existing primitive. Under reduce motion, the shared clock supplies the first
frame and creates no lease; after unmount, leases expire and the clock parks.

```rust,ignore
Skeleton::pulse(
    v_stack().gap(px(12.0))
        .child(Skeleton::new().w(px(132.0)))
        .child(Skeleton::new().h(px(3.0)).rounded_full()),
);
```

Toast shares Surface's defaults on its existing card node and accepts ready
content, an icon and a `dismiss_button`. A supplied child paints before content;
`overlay` paints last. Notifications uses those slots for the selection registry
reset and input installation, preserving the plain-text renderer, selection and
copying. The 560px maximum width, 10px radius, padding, shadow and top-centered
150ms entrance remain with the same card/layer structure.

The owner supplies a stable focus for the IconButton dismiss action. That focus
lives only as long as the active toast and is released on replacement/close.
Mouse/Enter/Space use ActivationExt; Notifications handles unmodified Escape.
The card and dismiss action stop click propagation. Hover, countdown, pauses,
generations, message storage and selection cleanup stay in Notifications; Toast
creates no entity, timer or recurring animation and leaves entrance/reduce-motion
behavior to the existing application layer.
