# Contributing to Michelle

Thanks for helping improve Michelle. Bug reports, focused fixes, tests, and
well-scoped features are welcome.

## Development setup

The debug app requires:

- macOS with Xcode and the Metal toolchain
- Rust 1.98.1 or newer
- Bun
- A supported agent CLI when testing a provider integration

Install dependencies and start the development watcher from the repository
root:

```sh
bun install
bun run dev
```

The watcher builds and signs `target/debug/Michelle Debug.app`. The provider
daemon remains an external `target/debug/michelle-debug-daemon`: provider-only
edits rebuild and hot-swap that process without relaunching the app, while
desktop edits rebuild and relaunch the app normally. Keep that watcher running
while you work. Do not start a second watcher or manually relaunch the debug
app. Press `Ctrl-C`, or quit the app, to stop it.

## Making changes

- Before starting work on anything larger than a bug fix, open an issue and
  discuss the proposal first.
- Keep changes focused and follow the existing Rust and GPUI conventions.
- Keep filesystem, process, network, and other blocking work off the UI thread.
  Rendering and row-building paths must read data already held in memory.
- Keep long collections virtualized and per-frame work proportional to visible
  content.
- Make every mouse control keyboard-operable, preserve visible focus, honor
  reduce-motion settings, and do not communicate state with color alone.
- Prefer provider-neutral behavior when a change applies to every agent, while
  preserving provider-native event order and session semantics.
- Add or update tests for behavior that can be verified without the UI.

## Checks

Run the focused checks relevant to your change, then run the full baseline
before opening a pull request:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
```

For user-visible changes, wait for the watcher to report a successful rebuild
and validate the freshly relaunched app. Include screenshots or a short
recording in the pull request when they make the result easier to review.

## Pull requests

Use the [pull request template](.github/pull_request_template.md) and write
your responses in your own words.

- Explain the problem and the chosen solution.
- List the checks you ran.
- Call out known limitations or follow-up work.
- Link the related issue, if one exists.

### AI policy

- **Disclose all AI usage.** In your pull request description or issue, name
  each tool you used and explain the extent of its involvement.
- **Understand the entire change.** You are responsible for the code you
  submit and all actions taken, including those performed by AI tools. Take
  time to review and verify the work before submitting it.
- **PR descriptions and comments must not be LLM generated.** Write them
  yourself, in your own words. PRs with obviously LLM-generated descriptions
  or comments will be closed immediately without review. This is especially
  important when submitting multiple PRs in a short period. A PR may be
  reopened once the text is corrected.
- These requirements also apply to issues, though enforcement is less strict.
- There is no vouching system at this time, though one may be added in the
  future.

AI-assisted contributions are welcome. This policy aims to prevent low-effort
submissions and preserve Michelle's standards for code quality, performance, and
maintainability.

## License

By contributing, you agree that your contribution will be licensed under the
[GNU General Public License v3.0 only](LICENSE).
