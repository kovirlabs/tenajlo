// Writes the GitHub Release notes for a version: a short intro, a download table and that
// version's CHANGELOG.md section. The release workflow uses it for the draft release; edit the
// draft on GitHub if a release needs more.
//
//   node scripts/release-notes.mjs 1.5.0 > notes.md
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "https://github.com/kovirlabs/tenajlo";
const SITE = "https://kovirlabs.github.io/tenajlo";

/** The body of `## [version]` in a Keep a Changelog file, without its heading. */
export function changelogSection(changelog, version) {
  const lines = changelog.split("\n");
  const start = lines.findIndex((l) => l.startsWith(`## [${version}]`));
  if (start === -1) throw new Error(`CHANGELOG.md has no "## [${version}]" section`);
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => l.startsWith("## ["));
  return (end === -1 ? rest : rest.slice(0, end)).join("\n").trim();
}

/** The full release notes for `version` (no leading "v"). */
export function releaseNotes(changelog, version) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`not a release version: ${version}`);
  const file = (name) => `[\`${name}\`](${REPO}/releases/download/v${version}/${name})`;
  return `Tenajlo is a free desktop Git client for self-hosted Forgejo and Gitea servers, made for engineers who'd rather not use the command line.

## Download

| Platform | File | Notes |
| --- | --- | --- |
| Windows 10/11 (x64) | ${file(`Tenajlo_${version}_x64-setup.exe`)} | Installs for your user, no admin rights. Git and Git LFS included. |
| Debian / Ubuntu (x64) | ${file(`Tenajlo_${version}_amd64.deb`)} | \`sudo apt install ./Tenajlo_${version}_amd64.deb\` |
| macOS (Apple Silicon) | ${file(`Tenajlo_${version}_aarch64.dmg`)} | Needs Git 2.40+ and Git LFS. |

The installers aren't code-signed yet, so Windows and macOS show a warning the first time; the [user guide](${SITE}/guide/#install) explains what to click. Already have Tenajlo 1.2 or later? It offers this update itself.

The other files (\`.sig\`, \`.app.tar.gz\`, \`latest.json\`) are for the built-in updater. \`THIRD_PARTY_LICENSES.html\` lists the licenses of bundled components.

## What's new in ${version}

${changelogSection(changelog, version)}

---

[User guide](${SITE}/guide/) · [Set up Forgejo for your team](${SITE}/forgejo-setup/) · [Full changelog](${REPO}/blob/main/CHANGELOG.md) · Questions: [Discussions](${REPO}/discussions)
`;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const version = (process.argv[2] ?? "").replace(/^v/, "");
  process.stdout.write(releaseNotes(readFileSync(join(root, "CHANGELOG.md"), "utf8"), version));
}
