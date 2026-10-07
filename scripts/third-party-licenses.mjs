// Generates THIRD_PARTY_LICENSES.html: the license of every Rust crate and npm package
// shipped in Tenajlo, with full texts. Fails if any license isn't on the allow-list
// (about.toml for crates, NPM_ACCEPTED below for npm), so a new dependency with an
// unexpected license is caught in CI, not after release.
//
// Usage: node scripts/third-party-licenses.mjs [--out <file>]
// Default output: src-tauri/resources/THIRD_PARTY_LICENSES.html (bundled by the Windows
// installer). Needs cargo-about (`cargo install cargo-about --features cli --locked`).
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** Same policy as about.toml. */
const NPM_ACCEPTED = new Set([
  "0BSD",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "CC0-1.0",
  "ISC",
  "MIT",
  "MIT-0",
  "MPL-2.0",
  "Unlicense",
  "Zlib",
]);

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outArg = process.argv.indexOf("--out");
const out =
  outArg > 0
    ? process.argv[outArg + 1]
    : join(root, "src-tauri", "resources", "THIRD_PARTY_LICENSES.html");

const run = (cmd, args, shell = false) =>
  execFileSync(cmd, args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024, shell });

// --- Rust crates (cargo-about) -------------------------------------------------------------
// `cargo metadata` resolves every platform's dependencies, so fetch them all first; then run
// cargo-about offline and locked, so the result depends only on Cargo.lock.
run("cargo", ["fetch", "--locked"]);
const aboutJson = join(root, "target", "about.json");
mkdirSync(dirname(aboutJson), { recursive: true });
run("cargo", [
  "about",
  "generate",
  "--frozen",
  "--fail",
  "--format",
  "json",
  "-c",
  "about.toml",
  "-o",
  aboutJson,
]);
const about = JSON.parse(readFileSync(aboutJson, "utf8"));
rmSync(aboutJson);
const rustGroups = about.licenses
  .map((l) => ({
    id: l.id,
    name: l.name,
    text: l.text,
    users: l.used_by.map((u) => `${u.crate.name} ${u.crate.version}`),
  }))
  .sort((a, b) => a.id.localeCompare(b.id) || a.users[0].localeCompare(b.users[0]));

// --- npm packages (bundled into the app's frontend) ------------------------------------------
// On Windows pnpm is pnpm.cmd, which Node only starts through a shell. The arguments are fixed
// strings, so nothing needs quoting.
const npm = JSON.parse(
  run("pnpm", ["licenses", "list", "--prod", "--json"], process.platform === "win32"),
);
const npmByText = new Map();
const rejected = [];
for (const [license, packages] of Object.entries(npm)) {
  if (!npmLicenseAccepted(license)) {
    rejected.push(...packages.map((p) => `${p.name} (${license})`));
    continue;
  }
  for (const pkg of packages) {
    const text = licenseText(pkg.paths[0]) ?? `${license} (no license file in the package)`;
    const key = `${license}\n${text}`;
    const group = npmByText.get(key) ?? { id: license, text, users: [] };
    group.users.push(`${pkg.name} ${pkg.versions.join(", ")}`);
    npmByText.set(key, group);
  }
}
if (rejected.length) {
  console.error(`third-party-licenses: licenses not accepted:\n  ${rejected.join("\n  ")}`);
  process.exit(1);
}
const npmGroups = [...npmByText.values()].sort((a, b) => a.id.localeCompare(b.id));

/** SPDX expression check: all identifiers accepted, or an OR-only expression with one accepted. */
function npmLicenseAccepted(expr) {
  const ids = expr
    .replace(/[()]/g, " ")
    .split(/\s+(?:OR|AND)\s+|\s+/)
    .filter(Boolean);
  if (ids.every((id) => NPM_ACCEPTED.has(id))) return true;
  return !/\bAND\b/.test(expr) && ids.some((id) => NPM_ACCEPTED.has(id));
}

function licenseText(dir) {
  const file = readdirSync(dir).find((f) => /^(licen[cs]e|copying)(\.|$)/i.test(f));
  return file ? readFileSync(join(dir, file), "utf8").trim() : null;
}

// --- HTML ----------------------------------------------------------------------------------
const esc = (s) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

const section = (title, groups) =>
  `<h2>${esc(title)}</h2>\n` +
  groups
    .map(
      (g) =>
        `<section>\n<h3>${esc(g.id)}</h3>\n<p class="used-by">Used by: ${g.users
          .map(esc)
          .join(", ")}</p>\n<pre>${esc(g.text)}</pre>\n</section>`,
    )
    .join("\n");

const crateCount = about.crates.length;
const npmCount = Object.values(npm).reduce((n, p) => n + p.length, 0);
const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Tenajlo third-party licenses</title>
<style>
body { font: 14px/1.5 system-ui, sans-serif; max-width: 60rem; margin: 2rem auto; padding: 0 1rem; }
pre { white-space: pre-wrap; background: #f4f6f8; padding: 0.75rem; border-radius: 6px; font-size: 12px; }
.used-by { color: #57606a; }
</style>
</head>
<body>
<h1>Third-party licenses</h1>
<p>Tenajlo includes ${crateCount} Rust crates and ${npmCount} npm packages, listed below with
their licenses. Bundled programs (Git for Windows, Git LFS) are described in
THIRD_PARTY_NOTICES.md.</p>
${section("Rust crates", rustGroups)}
${section("JavaScript packages", npmGroups)}
</body>
</html>
`;

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, html);
console.log(`third-party-licenses: ${crateCount} crates, ${npmCount} npm packages → ${out}`);
