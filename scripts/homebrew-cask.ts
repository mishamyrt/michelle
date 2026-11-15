export function renderHomebrewCask(tag: string, sha256: string, downloadUrl: string): string {
  const version = tag.match(/^v(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.+-]+)?)$/)?.[1];
  if (!version) throw new Error("Expected a release tag such as v0.1.19");
  if (!/^[0-9a-f]{64}$/.test(sha256)) throw new Error("Expected a SHA-256 checksum");
  const url = new URL(downloadUrl);
  if (url.protocol !== "https:" || url.username || url.password) {
    throw new Error("Expected a public HTTPS download URL without credentials");
  }
  // Ruby double-quoted strings interpolate #{...}, unlike JSON strings.
  const rubyUrl = JSON.stringify(url.href).replaceAll("#{", "\\#{");
  return `cask "michelle" do
  version "${version}"
  sha256 "${sha256}"

  url ${rubyUrl}
  name "Michelle"
  desc "Native macOS client for local coding agents"
  homepage "https://github.com/mishamyrt/michelle"

  depends_on arch: :arm64
  depends_on macos: :ventura

  app "Michelle.app"

  postflight_steps do
    run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "{{appdir}}/Michelle.app"]
  end
end
`;
}

if (import.meta.main) {
  const [tag, sha256, downloadUrl] = Bun.argv.slice(2);
  if (!tag || !sha256 || !downloadUrl || Bun.argv.length !== 5) {
    throw new Error("Usage: bun scripts/homebrew-cask.ts <tag> <sha256> <download-url>");
  }
  process.stdout.write(renderHomebrewCask(tag, sha256, downloadUrl));
}
