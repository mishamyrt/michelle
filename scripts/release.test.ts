import { expect, test } from "bun:test";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "..");

test("bundle signs every nested executable and invalidates the helper cache when signing mode changes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "michelle-signing-test-"));
  try {
    await cp(join(root, "resources"), join(directory, "resources"), { recursive: true });
    for (const path of ["bin", "sdk", "target/release"]) {
      await mkdir(join(directory, path), { recursive: true });
    }
    for (const path of [
      "sdk/libcua_driver_sdk.dylib", "sdk/cua_driver_abi.h", "sdk/cua-host.h",
      "target/release/michelle", "target/release/michelle_js_repl", "target/release/michelle-daemon",
    ]) {
      await writeFile(join(directory, path), "fixture");
    }
    const mocks: Record<string, string> = {
      bun: `
        case "$1" in
          scripts/cua-host.ts) printf '%s/sdk\\n' "$PWD" ;;
          scripts/cua-api.ts) : > "$3" ;;
          *) exit 1 ;;
        esac`,
      xcrun: `
        if [ "$2" = "-version" ]; then
          echo 'Swift fixture'
        else
          while [ "$#" -gt 0 ]; do
            if [ "$1" = "-o" ]; then : > "$2"; break; fi
            shift
          done
        fi`,
      codesign: `
        printf '<%s>' "$@" >> "$SIGNING_LOG"
        printf '\\n' >> "$SIGNING_LOG"`,
      plutil: "exit 0",
      xattr: "exit 0",
    };
    for (const [name, body] of Object.entries(mocks)) {
      await writeFile(join(directory, "bin", name), `#!/bin/sh\nset -eu\n${body}\n`, { mode: 0o755 });
    }
    const log = join(directory, "codesign.log");
    // Keep the identity the same to prove the helper cache includes signing mode.
    for (const mode of ["self-signed", "developer-id", "adhoc"]) {
      await writeFile(log, "");
      const identity = mode === "adhoc" ? "-" : "Fixture Signing";
      const result = Bun.spawnSync(["sh", join(root, "scripts/bundle.sh"), "release"], {
        cwd: directory,
        env: {
          ...process.env,
          PATH: `${join(directory, "bin")}:${process.env.PATH}`,
          CARGO_TARGET_DIR: "target",
          MICHELLE_SKIP_CARGO_BUILD: "1",
          MICHELLE_CODESIGN_IDENTITY: identity,
          MICHELLE_CODESIGN_SELF_SIGNED: mode === "self-signed" ? "1" : "0",
          SIGNING_LOG: log,
        },
      });
      expect(result.stderr.toString()).not.toContain("error");
      expect(result.exitCode).toBe(0);
      const calls = (await readFile(log, "utf8")).trim().split("\n");
      const signatures = calls.filter((line) => line.includes("<--sign>"));
      expect(signatures).toHaveLength(5);
      for (const [index, target] of [
        "/Frameworks/libcua_driver_sdk.dylib",
        "/Michelle Computer Use.app",
        "/Resources/michelle_js_repl",
        "/MacOS/michelle-daemon",
        "/Michelle.app",
      ].entries()) {
        expect(signatures[index]).toEndWith(`${target}>`);
      }
      for (const signature of signatures) {
        expect(signature).toContain(`<--sign><${identity}>`);
        if (mode === "adhoc") {
          expect(signature).not.toContain("<--options>");
          expect(signature).not.toContain("<--timestamp");
        } else {
          expect(signature).toContain(mode === "self-signed" ? "<--options><0>" : "<--options><runtime>");
          expect(signature).toContain(mode === "self-signed" ? "<--timestamp=none>" : "<--timestamp>");
        }
      }
      expect(calls.filter((line) => line.includes("<--verify>"))).toHaveLength(3);
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}, 30_000);

test("release rejects conflicting signing modes before building", () => {
  const result = Bun.spawnSync([process.execPath, join(root, "scripts/release.ts"), "--adhoc", "--self-signed"]);
  expect(result.exitCode).not.toBe(0);
  expect(result.stderr.toString()).toContain("Use either --adhoc or --self-signed");
});

test("workflows install Metal when xcrun finds only Xcode's compiler stub", async () => {
  const directory = await mkdtemp(join(tmpdir(), "michelle-metal-test-"));
  try {
    await writeFile(join(directory, "xcrun"), `#!/bin/sh
for argument in "$@"; do
  if [ "$argument" = "--find" ]; then exit 0; fi
done
test -f "$METAL_STATE"
`, { mode: 0o755 });
    await writeFile(join(directory, "xcodebuild"), `#!/bin/sh
set -eu
test "$1" = "-downloadComponent"
test "$2" = "MetalToolchain"
touch "$METAL_STATE"
echo download >> "$METAL_DOWNLOADS"
`, { mode: 0o755 });
    for (const name of ["test", "release"]) {
      const workflow = Bun.YAML.parse(await Bun.file(join(root, `.github/workflows/${name}.yml`)).text());
      const job = workflow.jobs[name === "test" ? "test" : "macos"];
      const step = job.steps.find((step: { name?: string }) => step.name === "Install Metal toolchain");
      const state = join(directory, name);
      const downloads = `${state}-downloads`;
      // The second run must reuse the working compiler.
      for (let attempt = 0; attempt < 2; attempt++) {
        const result = Bun.spawnSync(["bash", "-e", "-o", "pipefail", "-c", step.run], {
          env: {
            ...process.env,
            PATH: `${directory}:${process.env.PATH}`,
            METAL_STATE: state,
            METAL_DOWNLOADS: downloads,
          },
        });
        expect(result.exitCode).toBe(0);
        expect(await readFile(downloads, "utf8")).toBe("download\n");
      }
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
