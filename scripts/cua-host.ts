// Build the pinned Cua SDK with its native host/cursor entrypoints exposed.
// Keep its dependency graph and lockfile isolated from Waku's GPUI workspace.
import { $ } from "bun";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const revision = "1b50c02e2d34734f64d2d22f54eb76cc97b4a663";
const version = "0.28.0";

export async function prepareCuaHost(): Promise<string> {
  if (process.platform !== "darwin") {
    throw new Error("Computer Use builds require macOS.");
  }
  const extension = await readFile(
    join(root, "resources/computer-use/cua-host.rs"),
    "utf8",
  );
  const header = await readFile(
    join(root, "resources/computer-use/cua-host.h"),
    "utf8",
  );
  // Keep the cache key and build on Waku's toolchain, not Cua's older pin.
  const toolchain = (
    await $`rustup show active-toolchain`.cwd(root).quiet().text()
  ).trim().split(/\s+/)[0]!;
  const compiler = await $`rustc +${toolchain} -vV`.quiet().text();
  const key = createHash("sha256")
    .update(revision + extension + header + compiler)
    .digest("hex");
  const cache = join(root, ".waku-cache/cua-host");
  const destination = join(cache, key);
  const library = "libcua_driver_sdk.dylib";
  if (existsSync(join(destination, library))) return destination;
  await mkdir(cache, { recursive: true });
  const source = join(cache, revision);
  if (!existsSync(join(source, "libs/cua-driver/rust/Cargo.lock"))) {
    const staging = await mkdtemp(join(cache, ".source-"));
    try {
      await $`git clone --depth 1 --branch ${`cua-driver-rs-v${version}`} --filter=blob:none --sparse https://github.com/trycua/cua.git ${staging}`.quiet();
      if (
        (await $`git -C ${staging} rev-parse HEAD`.quiet().text()).trim() !==
        revision
      )
        throw new Error("Cua source revision mismatch");
      await $`git -C ${staging} sparse-checkout set libs/cua-driver/rust libs/cua-driver/contract`.quiet();
      await rename(staging, source);
    } finally {
      await rm(staging, { recursive: true, force: true });
    }
  }
  const abi = join(
    source,
    "libs/cua-driver/rust/crates/cua-driver-sdk/src/abi.rs",
  );
  const upstream =
    await $`git -C ${source} show ${`${revision}:libs/cua-driver/rust/crates/cua-driver-sdk/src/abi.rs`}`
      .quiet()
      .text();
  const extended = `${upstream}\n${extension}`;
  if ((await readFile(abi, "utf8")) !== extended)
    await writeFile(abi, extended);
  const target = join(cache, "target");
  console.error("Building Cua SDK native cursor host...");
  await $`cargo +${toolchain} build --locked --release --manifest-path ${join(source, "libs/cua-driver/rust/Cargo.toml")} --target-dir ${target} --package cua-driver-sdk`.cwd(
    join(source, "libs/cua-driver/rust"),
  );
  if (existsSync(join(destination, library))) return destination;
  const staging = await mkdtemp(join(cache, ".host-"));
  try {
    await cp(join(target, "release", library), join(staging, library));
    await cp(
      join(source, "libs/cua-driver/rust/include/cua_driver_abi.h"),
      join(staging, "cua_driver_abi.h"),
    );
    await writeFile(join(staging, "cua-host.h"), header);
    try {
      await rename(staging, destination);
    } catch (error) {
      if (!existsSync(join(destination, library))) throw error;
    }
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
  return destination;
}

if (import.meta.main) console.log(await prepareCuaHost());
