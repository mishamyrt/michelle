import { $ } from "bun";
import { resolve } from "node:path";

if (process.platform !== "darwin") {
  throw new Error("Local signed builds require macOS.");
}

process.chdir(resolve(import.meta.dir, ".."));
const identity = "Michelle Signing";
// Self-signed identities can sign local code without being trusted roots.
const identities = await $`security find-identity -p codesigning`.text();
const certificate = identities.match(/^\s*\d+\) ([A-F0-9]{40}) "Michelle Signing"/m)?.[1];
if (!certificate) {
  throw new Error(
    `Install the "${identity}" certificate and private key in Keychain Access.`,
  );
}

// Reuse upstream packaging unchanged, then replace its ad-hoc signatures.
await $`env WAKU_CODESIGN_IDENTITY=- sh scripts/bundle.sh release`;
const app = resolve(process.env.CARGO_TARGET_DIR ?? "target", "release/Michelle.app");
const contents = `${app}/Contents`;
const helper = `${contents}/Helpers/Michelle Computer Use.app`;
const sparkle = `${contents}/Frameworks/Sparkle.framework`;
const repl = `${contents}/Resources/waku_js_repl`;
const daemon = `${contents}/MacOS/waku-daemon`;
const { package: { version } } = Bun.TOML.parse(await Bun.file("Cargo.toml").text());
await $`plutil -replace CFBundleShortVersionString -string ${version} ${contents}/Info.plist`;
await $`plutil -replace CFBundleVersion -string ${version} ${contents}/Info.plist`;

// Refresh the installed helper when switching from ad-hoc signing or changing certificates.
const fingerprint = Bun.file(`${helper}/Contents/Resources/.waku-helper-fingerprint`);
await Bun.write(fingerprint, `${(await fingerprint.text()).trim()}\n${certificate}\n`);

// Sign inside out. A self-signed certificate has no Team ID, so hardened
// runtime library validation would reject the embedded Sparkle/Cua libraries.
for (const code of [
  `${helper}/Contents/Frameworks/libcua_driver_sdk.dylib`,
  helper,
  `${sparkle}/Versions/B/Autoupdate`,
  `${sparkle}/Versions/B/Updater.app`,
  sparkle,
  repl,
  daemon,
  app,
]) {
  await $`codesign --force --options 0 --timestamp=none --preserve-metadata=identifier --sign ${certificate} ${code}`;
  await $`codesign --verify --deep --strict ${code}`;
}

console.log(`\nBuilt ${app}\nSigned with ${identity}.`);
