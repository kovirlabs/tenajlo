// Regenerates the app icons in src-tauri/icons and the site's PNG icons from
// src-tauri/app-icon.svg. Windows and Linux icons fill their canvas. The macOS icon.icns
// follows Apple's icon grid instead: an 824px tile centred on a 1024px canvas with a soft
// shadow below, so it is the same size as other icons in the Dock.
//
// Usage: node scripts/build-icons.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(root, "src-tauri", "app-icon.svg");
const icons = join(root, "src-tauri", "icons");

const tauriIcon = (input, output) =>
  execFileSync("pnpm", ["exec", "tauri", "icon", input, "--output", output], {
    cwd: root,
    // tauri lists every file it writes on stderr; a failure still throws with that output.
    stdio: "pipe",
    shell: process.platform === "win32",
  });

/** The source artwork inside Apple's 824px tile, with the Dock shadow under it. */
function macosSvg(svg) {
  const inner = svg.slice(svg.indexOf(">") + 1, svg.lastIndexOf("</svg>"));
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <filter id="macos-shadow" x="-10%" y="-10%" width="120%" height="125%">
      <feDropShadow dx="0" dy="10" stdDeviation="10" flood-color="#000000" flood-opacity="0.3"/>
    </filter>
  </defs>
  <g filter="url(#macos-shadow)">
    <g transform="translate(100 100) scale(${824 / 1024})">${inner}</g>
  </g>
</svg>
`;
}

const work = mkdtempSync(join(tmpdir(), "tenajlo-icons-"));
try {
  tauriIcon(source, icons);
  // The app ships for desktop only.
  for (const mobile of ["android", "ios"])
    rmSync(join(icons, mobile), { recursive: true, force: true });

  const macSource = join(work, "app-icon-macos.svg");
  writeFileSync(macSource, macosSvg(readFileSync(source, "utf8")));
  tauriIcon(macSource, join(work, "out"));
  copyFileSync(join(work, "out", "icon.icns"), join(icons, "icon.icns"));

  const site = join(root, "site", "assets");
  copyFileSync(source, join(site, "icon.svg"));
  copyFileSync(join(icons, "32x32.png"), join(site, "favicon-32.png"));
  copyFileSync(join(icons, "128x128.png"), join(site, "icon-128.png"));
} finally {
  rmSync(work, { recursive: true, force: true });
}
console.log("icons: src-tauri/icons and site/assets updated");
