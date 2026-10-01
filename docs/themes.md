# Themes

Michelle includes a built-in theme named **Michelle**. **Color Scheme** chooses
System, Light, or Dark independently of the theme. System follows macOS appearance.

Put user themes in `~/Library/Application Support/Michelle/themes/`. Michelle
creates this directory and loads its TOML files once at startup, on a background
worker. Restart after adding or editing a file. The Theme selector appears in
Appearance settings only when at least one valid user theme is available.

Start with [the complete example](themes/example.toml), change `name`, and edit
the colors in `[light]` and `[dark]`. Both sections and every color field are
required. Colors use quoted `#RRGGBB` or `#RRGGBBAA` strings; the final byte is alpha.
`is_dark` is determined by the section and must not be supplied.

`sidebar` colors the native macOS sidebar tint, above its vibrancy material.
`sidebar_drag_background` is the solid surface used while resizing the sidebar.

Invalid files are skipped and their errors are logged. Michelle remains available
even if the directory cannot be read. If a selected theme disappears or becomes
invalid, the app uses Michelle with the selected color scheme. Selection is stored
by filename, so changing a theme's display name preserves it.
