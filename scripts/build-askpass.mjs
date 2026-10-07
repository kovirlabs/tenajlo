// Builds tenajlo-askpass and puts it where Tauri expects sidecars:
// src-tauri/binaries/tenajlo-askpass-<target-triple>[.exe]
// Usage: node scripts/build-askpass.mjs [--release]
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const release = process.argv.includes("--release");
const triple =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  /host: (\S+)/.exec(execFileSync("rustc", ["-vV"]).toString())?.[1];
if (!triple) throw new Error("could not determine the Rust target triple");

execFileSync(
  "cargo",
  ["build", "-p", "tenajlo-askpass", "--target", triple, ...(release ? ["--release"] : [])],
  { cwd: root, stdio: "inherit" },
);

const ext = triple.includes("windows") ? ".exe" : "";
const built = join(root, "target", triple, release ? "release" : "debug", `tenajlo-askpass${ext}`);
const dest = join(root, "src-tauri", "binaries", `tenajlo-askpass-${triple}${ext}`);
mkdirSync(dirname(dest), { recursive: true });
copyFileSync(built, dest);
console.log(`tenajlo-askpass → ${dest}`);
