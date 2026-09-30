# Computer Use

Michelle embeds **Cua Driver 0.28.0** through its native SDK ABI (1.1). The
JavaScript REPL exposes every native tool directly, such as `cua.list_apps()`,
`cua.get_window_state(args)`, and `cua.click(args)`. Setup binds the native methods internally. The bundled skill contains the
host-specific method signatures and direct-call examples; agent code does
not discover or dispatch tools through a catalog API.
Use `jsRepl.write(value)` for output and `await jsRepl.emitImage(image)` for
images. Tool schemas, capture, accessibility, input, and authorization come
from Cua.
The previous `sky` API and custom macOS action engine have been removed.
Computer Use is available in development and release builds. Enable it in
Settings to let supported providers register the `cua` bridge and attach the
bundled skill. System permissions and app approvals still apply.

## Processes and lifetime

On macOS, the signed `Michelle Computer Use.app` hosts the SDK library directly.
Its Launch Services bridge preserves the helper's existing independent TCC
identity and Michelle's Screen Recording/Accessibility onboarding. The bundled
library is signed with the same identity as the helper. Permission requests
remain host-owned: direct SDK permission checks do not open macOS prompts.

Each helper connection owns one SDK runtime. Native tool refusals preserve the
connection and the full result, including error codes, snapshot tokens, capture
metadata, and action outcomes. REPL reset and disconnect close the connection;
Stop cancels native work and ends the host. Interrupted actions are never
automatically retried. The `bring_to_front` tool is omitted from the exposed
API. Other tool arguments pass through to Cua unchanged. All SDK and IPC work
occurs outside the GUI process.

Michelle's preview decodes each PNG from the agent's `get_window_state` result on
a background worker. The previous decoded frame stays visible until the latest
replacement is ready; stale or invalid frames are discarded. There is no
second capture or continuous accessibility walk to change the agent's snapshot.

The helper initializes Cua's native cursor facility. On macOS, Cua's renderer
owns the helper's OS main thread while MCP and actions run on workers. Cursor movement, action animations,
themes, and reduced-motion handling use the same implementation as standalone
Cua Driver. Headless hosts still report unavailable graphics facilities.

## OpenCode 2

OpenCode 2 uses the existing shared service. Michelle registers one temporary MCP
connection per workspace through `/api/mcp` and attaches a session instruction
pointing to the bundled skill (OpenCode limits each entry to 8 KB). `js` and `js_reset` remain direct tools, with
OpenCode's additional codemode wrapper disabled for this server.

OpenCode's `_meta.sessionID` selects a Michelle-owned registration, so each task
has independent JavaScript bindings, native helper processes, cancellation,
and PiP frames. Unregistered sessions cannot execute calls through the bridge.
Detaching a task revokes its registration and removes its instructions; the
last task removes the temporary MCP server. Reconnecting checks the live
server before replacing it, preserving kernels across ordinary SSE reconnects.
No OpenCode configuration files or service descriptors are written.

## Platform requirements

- **macOS:** grant the Michelle helper Screen Recording and Accessibility access
  in Settings > Computer Use. Relaunch the permission-owning helper after a
  grant changes; new REPL connections launch a fresh helper.
Native window IDs are preserved as 64-bit values, including in preview events.
Use IDs and element tokens from fresh observations rather than reconstructing
them or assuming discovery order implies focus.

## Packaging and checks

`scripts/cua-driver.ts` packages the macOS SDK and REPL.
`scripts/cua-host.ts` builds the SDK from a pinned source revision with its native host entrypoints
exposed through `resources/computer-use/cua-host.rs`. This small ABI extension
enables Cua's existing cursor facility and main loop; it does not implement
input, capture, or rendering. Authorization still uses Cua's original checks.

The SDK uses its own pinned Rust toolchain and lockfile, isolated from Michelle's
workspace. Sources and builds are cached under `.michelle-cache/cua-host` so normal
dev rebuilds reuse the compiled SDK. The macOS bundle and dev watcher
package the same host-enabled SDK. `scripts/cua-api.ts` reads the native tool
metadata during packaging and writes the complete API reference into the
bundled skill. Bump the source revision, version, and ABI bindings together.
Include `resources/computer-use/CUA-LICENSE`.

The portable host can run `list-tools` as a diagnostic without capturing or
operating the desktop. The protocol smoke test uses only tool discovery,
configuration reads, a request missing required arguments, a synthetic image,
and REPL reset/reconnect:

```sh
cargo build -p michelle --bin michelle_js_repl -p michelle-computer-use --bin michelle_computer_use
bun scripts/cua-driver.ts bundle target/debug target/debug/resources debug
bun scripts/test-computer-use.ts
```

To test the signed macOS host, pass its packaged REPL and helper executable
paths to `scripts/test-computer-use.ts`. Add `--expect-cursor` to check native
cursor availability in a graphical session without moving or clicking anything.
CI runs the SDK smoke test on macOS. UI automation tests are separate
and should only run when requested.

References: [in-process SDK guide](https://cua.ai/docs/how-to-guides/driver/use-sdk-in-process),
[native ABI](https://github.com/trycua/cua/blob/1b50c02e2d34734f64d2d22f54eb76cc97b4a663/libs/cua-driver/rust/include/cua_driver_abi.h),
[pinned release](https://github.com/trycua/cua/releases/tag/cua-driver-rs-v0.28.0).
