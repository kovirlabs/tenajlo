// Captures screenshots of the real UI with invented demo data (demo/), for the README and
// the landing page. Builds demo/ with Vite, serves it statically (a dev server's live-reload
// socket would keep headless Chrome waiting forever), renders each scene in headless Chrome
// and writes 2x PNGs to site/assets/screenshots/. Set CHROME_PATH if Chrome is elsewhere.
//
// Usage: node scripts/screenshots.mjs
import { spawn } from "node:child_process";
import { createReadStream, existsSync, mkdirSync, mkdtempSync, rmSync, statSync } from "node:fs";
import { setTimeout as sleep } from "node:timers/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { build } from "vite";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "site", "assets", "screenshots");
const dist = join(root, "target", "demo-site");
const chrome =
  process.env.CHROME_PATH ??
  {
    darwin: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    win32: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  }[process.platform] ??
  "google-chrome";

const SCENES = [
  { name: "changes", query: "scene=changes" },
  { name: "changes-dark", query: "scene=changes&theme=dark" },
  { name: "history", query: "scene=history" },
  { name: "clone", query: "scene=clone" },
  { name: "conflict", query: "scene=conflict" },
  { name: "welcome", query: "scene=welcome" },
];

await build({
  root,
  configFile: join(root, "vite.config.ts"),
  base: "./",
  logLevel: "warn",
  build: {
    outDir: dist,
    emptyOutDir: true,
    rollupOptions: { input: join(root, "demo", "index.html") },
  },
});

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
};
const server = createServer((req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname));
  const file = join(dist, path.endsWith("/") ? join(path, "index.html") : path);
  if (!file.startsWith(dist) || !existsSync(file)) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
  createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}/demo/`;

try {
  mkdirSync(out, { recursive: true });
  for (const scene of SCENES) {
    const file = join(out, `${scene.name}.png`);
    await capture(`${base}?${scene.query}`, file);
    console.log(`screenshots: ${scene.name}.png (${Math.round(statSync(file).size / 1024)} KB)`);
  }
  // Social card (link previews, GitHub social preview) from scripts/social-card.html.
  const card = join(root, "site", "assets", "social-card.png");
  await capture(pathToFileURL(join(root, "scripts", "social-card.html")).href, card, {
    size: "1280,640",
    scale: 1,
  });
  console.log(`screenshots: social-card.png (${Math.round(statSync(card).size / 1024)} KB)`);
} finally {
  server.close();
}

/**
 * Headless Chrome writes the screenshot after `--timeout` ms but doesn't always exit (its
 * updater helper keeps it alive), so wait for the file to appear and stop growing, then
 * stop Chrome.
 */
async function capture(url, file, { size = "1200,760", scale = 2 } = {}) {
  rmSync(file, { force: true });
  const profile = mkdtempSync(join(tmpdir(), "tenajlo-shot-"));
  const proc = spawn(
    chrome,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--no-first-run",
      "--no-default-browser-check",
      `--user-data-dir=${profile}`,
      `--window-size=${size}`,
      `--force-device-scale-factor=${scale}`,
      "--allow-file-access-from-files",
      // Time for the mocked IPC, stores and demo scene setup to settle.
      "--timeout=3000",
      `--screenshot=${file}`,
      url,
    ],
    { stdio: "ignore" },
  );
  try {
    let last = -1;
    for (let i = 0; i < 300; i++) {
      await sleep(200);
      const size = existsSync(file) ? statSync(file).size : -1;
      if (size > 0 && size === last) return;
      last = size;
    }
    throw new Error(`no screenshot from Chrome for ${url}`);
  } finally {
    proc.kill("SIGKILL");
    await sleep(300);
    rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
}
