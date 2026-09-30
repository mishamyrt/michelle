// Package the macOS Computer Use SDK and REPL.
import { cp, mkdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { writeCuaSkill } from "./cua-api";
import { prepareCuaHost } from "./cua-host";

const root = resolve(import.meta.dir, "..");

export async function bundleComputerUse(
  binDirectory: string,
  resourcesDirectory: string,
  profile: "debug" | "release",
): Promise<void> {
  const hostSdk = await prepareCuaHost();
  const target = resolve(
    root,
    process.env.CARGO_TARGET_DIR || "target",
    profile,
  );
  await mkdir(binDirectory, { recursive: true });
  await mkdir(resourcesDirectory, { recursive: true });
  for (const file of ["michelle_js_repl", "michelle_computer_use"]) {
    const destination = join(binDirectory, file);
    if (resolve(target, file) !== resolve(destination))
      await cp(join(target, file), destination);
  }
  await cp(
    join(hostSdk, "libcua_driver_sdk.dylib"),
    join(binDirectory, "libcua_driver_sdk.dylib"),
  );
  for (const [source, relative] of [
    ["resources/computer-use/pi-extension.ts", "computer-use/pi-extension.ts"],
    ["resources/computer-use/CUA-LICENSE", "computer-use/CUA-LICENSE"],
  ]) {
    const destination = join(resourcesDirectory, relative!);
    await mkdir(dirname(destination), { recursive: true });
    await cp(join(root, source!), destination);
  }
  await writeCuaSkill(
    join(binDirectory, "michelle_computer_use"),
    join(resourcesDirectory, "skills/michelle-computer-use/SKILL.md"),
  );
}

if (import.meta.main) {
  const [command, ...args] = process.argv.slice(2);
  if (
    command === "bundle" &&
    args.length === 3 &&
    ["debug", "release"].includes(args[2]!)
  ) {
    await bundleComputerUse(
      resolve(args[0]!),
      resolve(args[1]!),
      args[2] as "debug" | "release",
    );
  } else
    throw new Error(
      "Usage: bun scripts/cua-driver.ts bundle <bin-dir> <resources-dir> <debug|release>",
    );
}
