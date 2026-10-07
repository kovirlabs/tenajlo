// Fetches the git that Windows builds bundle (spec §5.1): MinGit plus git-lfs (MinGit doesn't
// include it; spec §13.1). Pinned by version and SHA-256, downloaded at build time only, and
// extracted to src-tauri/resources/mingit (git-lfs.exe next to the real git.exe in ucrt64/bin —
// mingw64/bin before Git for Windows 2.5x — which MinGit's cmd/git.exe puts on PATH).
//
// Usage: node scripts/fetch-mingit.mjs [--force]
// Runs only for Windows targets (TAURI_ENV_PLATFORM=windows, or on a Windows host) unless
// --force is given. Downloads are cached in target/downloads.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Update deliberately: bump the version, then the hash from the release's published digest.
const MINGIT = {
  version: "2.56.0.2",
  url: "https://github.com/git-for-windows/git/releases/download/v2.56.0.windows.2/MinGit-2.56.0.2-64-bit.zip",
  sha256: "da35e72aa21c005a5a0d298cfbae110bc1609a815730ea0dde84b01a1b3cd3be",
};
const GIT_LFS = {
  version: "3.8.0",
  url: "https://github.com/git-lfs/git-lfs/releases/download/v3.8.0/git-lfs-windows-amd64-v3.8.0.zip",
  sha256: "b62e7b8ceddee635f691233d77de8eaa4b213e9209e0173811d8cfa77f7882c1",
};

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dest = join(root, "src-tauri", "resources", "mingit");
const stamp = join(dest, ".tenajlo-bundle");
const wanted = `MinGit ${MINGIT.version} + git-lfs ${GIT_LFS.version}\n`;

const forWindows =
  process.argv.includes("--force") ||
  process.env.TAURI_ENV_PLATFORM === "windows" ||
  (!process.env.TAURI_ENV_PLATFORM && process.platform === "win32");
if (!forWindows) {
  console.log("fetch-mingit: not a Windows build; skipping");
  process.exit(0);
}
if (existsSync(stamp) && readFileSync(stamp, "utf8") === wanted) {
  console.log(`fetch-mingit: ${wanted.trim()} already in place`);
  process.exit(0);
}

const downloads = join(root, "target", "downloads");
mkdirSync(downloads, { recursive: true });

/** Downloads `pkg` (or reuses the cache) and verifies its SHA-256. */
async function fetchVerified(pkg) {
  const file = join(downloads, pkg.url.split("/").pop());
  if (!existsSync(file) || sha256(file) !== pkg.sha256) {
    console.log(`fetch-mingit: downloading ${pkg.url}`);
    const res = await fetch(pkg.url);
    if (!res.ok) throw new Error(`download failed: ${res.status} ${pkg.url}`);
    writeFileSync(`${file}.part`, Buffer.from(await res.arrayBuffer()));
    renameSync(`${file}.part`, file);
  }
  const actual = sha256(file);
  if (actual !== pkg.sha256) {
    rmSync(file);
    throw new Error(`checksum mismatch for ${pkg.url}: expected ${pkg.sha256}, got ${actual}`);
  }
  return file;
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

/** bsdtar reads zip files (built into Windows 10+ and macOS). */
function unzip(zip, into) {
  mkdirSync(into, { recursive: true });
  execFileSync("tar", ["-xf", zip, "-C", into], { stdio: "inherit" });
}

const mingitZip = await fetchVerified(MINGIT);
const lfsZip = await fetchVerified(GIT_LFS);

rmSync(dest, { recursive: true, force: true });
unzip(mingitZip, dest);
const lfsTmp = join(downloads, "git-lfs-extract");
rmSync(lfsTmp, { recursive: true, force: true });
unzip(lfsZip, lfsTmp);
const lfsExe = join(lfsTmp, `git-lfs-${GIT_LFS.version}`, "git-lfs.exe");
if (!existsSync(lfsExe)) throw new Error(`git-lfs.exe not found in ${GIT_LFS.url}`);
const binDir = ["ucrt64", "mingw64"]
  .map((d) => join(dest, d, "bin"))
  .find((d) => existsSync(join(d, "git.exe")));
if (!binDir) throw new Error("MinGit layout not recognized: no ucrt64/bin or mingw64/bin");
renameSync(lfsExe, join(binDir, "git-lfs.exe"));
rmSync(lfsTmp, { recursive: true, force: true });

for (const required of [join(dest, "cmd", "git.exe"), join(binDir, "git-lfs.exe")]) {
  if (!existsSync(required)) throw new Error(`bundle is missing ${required}`);
}
writeFileSync(stamp, wanted);
console.log(`fetch-mingit: ${wanted.trim()} → ${dest}`);
