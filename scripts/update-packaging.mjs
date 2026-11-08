// Points the Scoop manifest (bucket/tenajlo.json) and the Chocolatey package
// (packaging/chocolatey) at a published release's Windows installer. The SHA-256 comes from
// the digest GitHub publishes for the release asset, so nothing is downloaded.
//
// Usage: node scripts/update-packaging.mjs <version>     e.g. 1.4.1 (no "v")
// Run it after publishing a release, then commit the result. The release workflow runs it too,
// before packing the Chocolatey package (see .github/workflows/package-managers.yml).
// GITHUB_TOKEN is used if set (higher API rate limit); the repository is public, so it's optional.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "kovirlabs/tenajlo";
const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error("Usage: node scripts/update-packaging.mjs <version>   (e.g. 1.4.1)");
  process.exit(1);
}
const tag = `v${version}`;
const asset = `Tenajlo_${version}_x64-setup.exe`;
const url = `https://github.com/${REPO}/releases/download/${tag}/${asset}`;

const headers = { Accept: "application/vnd.github+json", "User-Agent": "tenajlo-packaging" };
if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
const res = await fetch(`https://api.github.com/repos/${REPO}/releases/tags/${tag}`, { headers });
if (!res.ok) {
  console.error(`Release ${tag} not found (HTTP ${res.status}). Is it published?`);
  process.exit(1);
}
const release = await res.json();
const digest = release.assets?.find((a) => a.name === asset)?.digest ?? "";
const sha256 = digest.startsWith("sha256:") ? digest.slice("sha256:".length) : "";
if (!/^[0-9a-f]{64}$/.test(sha256)) {
  console.error(`Release ${tag} has no SHA-256 digest for ${asset}`);
  process.exit(1);
}

/** Replaces exactly one match of `pattern` in a file, or fails. */
function replaceOnce(rel, pattern, replacement) {
  const path = join(root, rel);
  const text = readFileSync(path, "utf8");
  const matches = text.match(new RegExp(pattern.source, pattern.flags + "g")) ?? [];
  if (matches.length !== 1) {
    throw new Error(`${rel}: expected one match for ${pattern}, found ${matches.length}`);
  }
  writeFileSync(path, text.replace(pattern, replacement));
}

// Scoop (edited as text so the file keeps its Prettier formatting; the autoupdate block's
// "$version" URL isn't touched because it doesn't match a released URL)
const scoopFile = "bucket/tenajlo.json";
replaceOnce(scoopFile, /^ {2}"version": "[^"]*"/m, `  "version": "${version}"`);
replaceOnce(scoopFile, /"url": "https:[^"$]*#\/dl\.7z"/, `"url": "${url}#/dl.7z"`);
replaceOnce(scoopFile, /"hash": "[0-9a-f]{64}"/, `"hash": "${sha256}"`);
const scoop = JSON.parse(readFileSync(join(root, scoopFile), "utf8"));
if (scoop.version !== version || scoop.architecture["64bit"].hash !== sha256) {
  throw new Error(`${scoopFile} didn't update as expected`);
}

// Chocolatey
replaceOnce(
  "packaging/chocolatey/tenajlo.nuspec",
  /<version>[^<]*<\/version>/,
  `<version>${version}</version>`,
);
replaceOnce(
  "packaging/chocolatey/tools/chocolateyinstall.ps1",
  /\$url64 = '[^']*'/,
  `$url64 = '${url}'`,
);
replaceOnce(
  "packaging/chocolatey/tools/chocolateyinstall.ps1",
  /\$checksum64 = '[^']*'/,
  `$checksum64 = '${sha256}'`,
);

console.log(`Scoop and Chocolatey now point at ${tag} (${sha256}).`);
