# CLAUDE.md — Tenajlo

Tenajlo is a desktop Git client for self-hosted Forgejo, modeled on GitHub Desktop. It uses Tauri 2 with a Rust backend and a React + TypeScript frontend.

**`spec.md` is the source of truth** for scope, architecture, and milestones. Read the relevant section before starting a task. If a request conflicts with the spec, say so and ask before deviating. Do not silently expand scope beyond the current milestone.

## Commands

```bash
pnpm install                 # frontend deps
pnpm tauri dev               # run app (hot reload frontend, rebuild Rust on change)
pnpm tauri build             # release build + installer
pnpm typecheck               # tsc --noEmit
pnpm lint                    # eslint + prettier --check
pnpm test                    # vitest
pnpm bindings                # regenerate src/bindings.ts from Rust (tauri-specta)
pnpm site                    # build the website (site/ + guide/ from docs/user-guide.md) into target/site
node scripts/build-askpass.mjs [--release]  # build the tenajlo-askpass sidecar (tauri dev/build run this for you)
node scripts/third-party-licenses.mjs --out target/THIRD_PARTY_LICENSES.html  # dependency license check (needs cargo-about; CI runs it)

cd src-tauri
cargo fmt --all
cargo clippy --all-targets -- -D warnings   # also with --target x86_64-pc-windows-msvc when touching cfg(windows) code (needs llvm-rc, e.g. `brew install llvm`)
cargo clippy --all-targets --features integration -- -D warnings   # also compiles the Forgejo integration tests (no Docker needed)
cargo test --workspace       # unit + temp-repo tests + askpass crate (needs git >= 2.40 and git-lfs on PATH; on a fresh checkout run `node scripts/build-askpass.mjs` first, or tauri-build copies its empty placeholder over the askpass test binary)
cargo test --features integration   # needs Forgejo in Docker: docker compose -f dev/forgejo.yml up -d && dev/seed-forgejo.sh
```

Before calling a task done, run `cargo fmt`, `cargo clippy -D warnings` (with and without `--features integration`), `cargo test --workspace`, `pnpm typecheck`, `pnpm lint`, and `pnpm test`. All must pass.

## Layout

```
src-tauri/
  src/
    main.rs, lib.rs        # Tauri builder, plugin + command registration
    commands/              # #[tauri::command] fns — thin: validate, call core, map errors
    git/                   # one module per git operation (status.rs, commit.rs, push.rs …)
      exec.rs              # THE ONLY place that spawns git (env, -c flags, timeout, cancel)
      parse/               # pure parsers for porcelain v2, log, diff, progress
      error.rs             # GitErrorKind + stderr pattern table
    auth/                  # accounts, keyring access, trampoline server
    forgejo/               # REST client (reqwest)
    repo_manager.rs        # per-repo state, per-repo mutation lock, fs watcher
    store/                 # JSON persistence with schemaVersion + migrations
  capabilities/            # Tauri permission files — keep minimal
crates/askpass/            # tenajlo-askpass sidecar binary
src/                       # React frontend
  api/                     # wrappers around generated bindings + event listeners
  bindings.ts              # GENERATED — never edit by hand
  stores/                  # Zustand stores
  components/              # UI
  views/                   # Changes, History, Clone, Settings …
dev/forgejo.yml            # docker compose for integration tests
```

## Architecture rules (non-negotiable)

1. **Only `git/exec.rs` spawns git.** Use argument arrays, never a shell. Put `--` before pathspecs. Validate ref and remote names (reject a leading `-`; use `git check-ref-format`). New operations go through `exec.rs`, never through `std::process::Command` elsewhere.
2. **Secrets never reach the frontend.** PATs, passwords, and passphrases stay in Rust and the OS keychain (`keyring` crate). Commands return account *metadata* only. Auth prompts flow UI → Rust as answers, never Rust → UI as stored values.
3. **Never log secrets.** Pass anything that might contain a URL or header through `redact()` before `tracing`. Add a test whenever you add a new log site that touches remote URLs.
4. **Never weaken TLS or SSH trust.** Do not use `http.sslVerify=false`, do not auto-accept host keys, and do not edit `known_hosts` directly.
5. **Never write to the user's global git config** except `user.name` and `user.email`, after explicit confirmation. Per-invocation settings go in `-c` flags.
6. **One mutating git op per repo at a time.** Take the per-repo lock in `RepoManager`. Read-only ops use `GIT_OPTIONAL_LOCKS=0`.
7. **Commands are thin.** `commands/*` validates input, calls into `git/`, `auth/`, or `forgejo/`, and maps errors to the serializable `AppError`. Logic lives below that layer.
8. **Frontend holds no Git semantics.** It renders typed data and calls `api/`. If you find yourself parsing git output or building refs in TS, move that work to Rust.
9. **Tauri capabilities stay least-privilege.** Do not add `fs`, `shell`, or `http` plugin permissions for the WebView. If a feature seems to need one, add a Rust command instead.
10. **Discard is recoverable.** Untracked files go to the OS trash (`trash` crate), never `remove_file`.

## Adding a git operation (checklist)

1. Read the matching file in GitHub Desktop (`app/src/lib/git/<op>.ts`) for flags, edge cases, and error handling. It is MIT-licensed. If you port non-trivial logic, add a header comment crediting `desktop/desktop` and update `THIRD_PARTY_NOTICES.md`.
2. Add `src-tauri/src/git/<op>.rs` with a typed result struct (`#[derive(Serialize, specta::Type)]`).
3. Put parsing in `git/parse/` as a pure function and add fixture-based unit tests, including unicode paths, renames, and empty output.
4. Map new stderr patterns in `git/error.rs`.
5. Add an integration test using a temp repo (`tempfile`) and, for remote ops, a local bare repo as origin.
6. Expose it via `commands/`, run `pnpm bindings`, and wrap it in `src/api/`.
7. For long-running ops, emit `git-progress` events (throttled) and support cancellation.

## Auth model (summary — details in spec §6)

- **HTTPS with a Tenajlo account for the host:** the credential helper returns `login` + PAT from the keychain.
- **HTTPS without an account:** leave the user's `credential.helper` chain alone (e.g. Git Credential Manager), and fall back to a trampoline UI prompt.
- **SSH:** system OpenSSH (on Windows, use `System32\OpenSSH` so the Windows ssh-agent works). Passphrase and host-key prompts go through `SSH_ASKPASS` and the trampoline to a UI dialog.
- **Trampoline:** `tenajlo-askpass` connects to a loopback port with a per-operation random token. Treat an unknown or expired token as an attack: reject it and log without the token.
- **Forgejo API:** `Authorization: token <PAT>`. The base URL comes from the account.

## Environment notes

- Users are often on company networks with private DNS names and an internal CA. On Windows, pass `-c http.sslBackend=schannel`. `reqwest` uses rustls with `rustls-platform-verifier` (the OS verifier). If you hit TLS errors in dev, fix the trust store and never the verification flag.
- Windows is the P0 platform. Test path handling with backslashes, spaces, long paths, and CRLF. MinGit ships as a Tauri sidecar on Windows only.
- Minimum git version is 2.40. Use modern porcelain (`switch`, `restore`, `status --porcelain=v2`).

## Code style

- **Rust:** edition 2021, `thiserror` for module errors, `anyhow` only in `main`/tests. No `unwrap()`/`expect()` outside tests and startup invariants. Prefer `tokio::process`. Public functions get doc comments.
- **TypeScript:** `strict: true`, no `any`, no default exports, function components and hooks only. Zustand stores are small and per-domain (`repoStore`, `accountStore`, `uiStore`).
- Keep files under about 400 lines. Split by responsibility.
- UI copy is plain language for non-Git-experts: "Your branch is 3 commits behind origin. Pull to get them." rather than "behind by 3".

## Working with me (Evan)

- I usually work in short evening sessions. Keep tasks scoped so each one ends in a buildable, passing state. Commit at the end with a clear message, and tell me what's next.
- When something is ambiguous, ask one focused question instead of guessing on anything hard to undo (schema, auth flow, persistence format).
- Summaries at the end of a task should be short: what changed, what was tested, and anything I need to decide.
- Don't add dependencies without saying why. Prefer well-maintained crates and packages with small footprints.

## Do not

- Edit `src/bindings.ts` by hand.
- Add telemetry, analytics, or any network call other than to the user's configured Forgejo host(s) and Tenajlo's own update check. The update check talks only to the project's GitHub Releases (the `latest.json` feed and the signed installers it lists; see `src-tauri/src/updates.rs`), sends nothing about the user or their repositories, and can be turned off in Settings → About.
- Implement features outside the current milestone without asking (see spec §12).
- Copy GitHub Desktop branding, icons, or name.
