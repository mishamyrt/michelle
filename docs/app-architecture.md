# Desktop application ownership

`src/app.rs` is the desktop facade. `Michelle` owns the component models and UI
state, the daemon supervisor, one `PersistedState`, and its `StateStore`.
`app/startup.rs` assembles components, installs subscriptions, and starts their
initial requests. The daemon remains responsible for task execution, persisted
provider state, workspace operations, and attachment bytes.

A component's main file composes GPUI controls and forwards user intentions.
Its `model.rs` owns data, preparation, guards, asynchronous operations, and their
results. Additional modules follow responsibility rather than splitting layout
by line count. These are ordinary owned structs; an independent GPUI entity is
used where its lifetime or render identity matters, including editors, terminals,
the navigation rail, and cached panes.

| Component | Data and operations | UI and composition |
| --- | --- | --- |
| Toolbar | Calls the relevant component operations | `toolbar.rs`: three sections and attached menus |
| Usage | `usage_page/model.rs`: history, request generations and freshness | `usage_page.rs`: mode, period, filters, lists and chart interaction |
| Commit | `commit_dialog/model.rs`: preparation, generation and commit/push | `commit_dialog.rs`: draft, options, focus and modal |
| Settings | `settings/model.rs`, `providers.rs`, `permissions.rs`, `plan_usage.rs` | `settings.rs`: navigation, inputs, expansion and scrolling |
| Skills | `skills_page/model.rs`: catalog and optimistic changes | `skills_page.rs`: filters, selection and document viewer |
| Pickers | Each picker's `model.rs`: projections, selection and branch operations | `model_picker.rs`, `project_picker.rs`, `branch_picker.rs`: inputs, cursor, menus and lists |
| Sessions | `sessions/model.rs`, `activation.rs`, `queue.rs`, `checkpoints.rs`, `interactions.rs` | `SessionUi` in `sessions/interactions.rs`: provider answer input, Escape confirmation and preview position |
| Runtime | `runtime.rs` and `runtime/model.rs`: transport, preparation, event pump and background bookkeeping | Invoked by session scenarios; it has no independent screen |
| Composer | `composer/model.rs`, `drafts.rs`, `attachments.rs`, `sources.rs` | `composer.rs` and `autocomplete.rs`: input, completion and controls |
| Transcript | `transcript_view/model.rs`: rows, navigation, footer, Markdown and activity caches | `transcript_view.rs`, `list.rs`, `transcript_search.rs`: composition, virtualization, anchors, selection and search |
| Sidebar | `sidebar/model.rs`: rows, ordering, rename, collections and daemon moves | `sidebar.rs`: groups, virtual list, focus, inline editor, drag preview and animation |
| Right panel | `right_panel/model.rs`, `workspace.rs`, `review.rs`, `files.rs`, `terminals.rs`, `session_state.rs` | `right_panel.rs`, `file_search.rs`: tabs, tree, filters, selection and scrolling |
| Goal | `goal_dialog/model.rs`: queued operations and pursuit | `goal_dialog.rs`: objective editor and modal |
| Image preview | `image_preview/model.rs`: deduplicated image loading and cache | `image_preview.rs`: overlay and focus generation |
| Notifications | `notifications/model.rs`: timers, hover pause and control-copy feedback | `notifications.rs`: overlay and selection |
| Shell | `shell/model.rs`: geometry helpers, cached environment data and label clock | `shell.rs`, `render.rs`, `window_chrome.rs`: pane entities, panels and window composition |

Provider labels and icons belong to `app/presentation`. Reusable controls,
navigation primitives, palette tokens and motion belong to `src/ui`. Feature
operations stay internal to `app`; module-local details use private visibility.
There is no additional public component API or generic reducer.

## Lifetimes and asynchronous results

Each operation retains its original guards. QueryCache tokens reject obsolete
workspace results; explicit generations guard Usage, Skills, discovery,
checkpoints and review requests. Editor read epochs prevent an older read from
replacing newer file contents. Session/runtime identities gate provider work.
Commit generation and Git mutation belong to the model and survive modal close.
The image cache retains loading and unavailable states to deduplicate misses.

The right panel stores inactive-session snapshots and moves the active data and
UI through `take`/`replace`. It does not clone another active session model.
Terminal entities are retained while referenced by active or stored surfaces;
restoring a session preserves its tabs and requests the active terminal's focus.
Composer drafts use their existing persisted keys and debounce.

## Rendering constraints

Render and measurement read in-memory state. A miss can queue background work,
but RPC, subprocesses and filesystem work execute on the background executor.
Lists remain virtualized. Transcript and Sidebar fingerprints retain their
existing input granularity; row builders do not rebuild the whole session.
Cached panes observe the root and preserve the panel-slide invalidation rules.

The event pump's stream-commit cadence and the shared pulse clock are unchanged.
See [performance.md](performance.md) for their limits and measurement procedure.
System reduce-motion handling, stable keyboard focus identities, key bindings,
and focus restoration stay with the UI that owns the interaction.

## Verification

Run `cargo fmt --all -- --check`, `git diff --check`, and
`cargo test --locked --workspace`. Git fixture tests may need signing disabled
only in the test process, without changing the user's Git configuration.
The existing `bun ./scripts/dev.ts` watcher rebuilds, signs, and relaunches the
debug app after source edits. Do not start another watcher or manually relaunch
its app. Visual validation is performed only when requested.
