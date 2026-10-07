# Tenajlo — Desktop Git Client for Forgejo

**Status:** Draft v0.1 · **Owner:** Evan · **Name:** Tenajlo · **Bundle identifier:** `com.kovirlabs.tenajlo`

A cross-platform desktop Git client in the spirit of GitHub Desktop, built for self-hosted Forgejo instances (first target: `TMC-GIT01.tmus.local`). Built with Tauri 2 (Rust backend) and TypeScript/React (frontend).

---

## 1. Goals

1. Let engineers who are not Git experts clone, commit, pull, push, and branch without the CLI.
2. Make authentication painless and secure across all three methods: Git credentials over HTTPS, SSH keys, and Forgejo personal access tokens (PATs).
3. Work reliably on an internal network: private DNS (`*.tmus.local`), internal or self-signed TLS certificates, and Windows domain-joined machines.
4. Stay small, fast, and easy to maintain by one person in limited time.

## 2. Non-goals (v1)

- Pull requests, issues, reviews, or other Forgejo web features beyond repository listing.
- Interactive rebase, cherry-pick UI, reflog browsing, bisect.
- Submodule management (detect and warn only).
- Multiple windows, tabs, or a plugin system.
- GitHub or GitLab specific integrations. Generic Git remotes still work for clone/pull/push.

## 3. Target users and platforms

| Platform | Priority | Notes |
|---|---|---|
| Windows 10/11 x64 | **P0** | Most TMC engineering workstations. Bundles MinGit. |
| Linux x64 (Ubuntu 24.04+) | P1 | Uses system git. Developer machines. |
| macOS (arm64) | P2 | Uses system git. Best effort. |

Primary users are mechanical and controls engineers working with code, PLC exports, scripts, and documentation.

---

## 4. Architecture

### 4.1 Lessons from GitHub Desktop (`desktop/desktop`, MIT)

| GitHub Desktop pattern | Tenajlo equivalent |
|---|---|
| `dugite`: bundled git binary, spawned per command | Rust `git` module spawning the git CLI. MinGit bundled as a Tauri sidecar on Windows. |
| `app/src/lib/git/*`: one file per git operation, typed results | `src-tauri/src/git/*`: one module per operation, typed structs |
| **Trampoline**: small helper set as `GIT_ASKPASS` / credential helper that calls back into the app over a local socket | `tenajlo-askpass` sidecar binary plus a localhost IPC server in the app, with a per-operation token |
| `AppStore` / `GitStore` / `Dispatcher` | Rust `RepoManager` owns git state. Frontend uses a Zustand store and a thin `api/` layer for Tauri commands. |
| Parses `status --porcelain=v2 -z` and custom `log --format` | Same approach |
| Progress parsing of `--progress` stderr | Same approach, emitted as Tauri events |
| SSH host-key and passphrase prompts routed to UI | Same, via `SSH_ASKPASS` and the trampoline |

Code may be ported from GitHub Desktop where useful (MIT). Ported files must keep attribution and be listed in `THIRD_PARTY_NOTICES.md`.

### 4.2 Process model

```
┌─────────────────────────── Tenajlo (Tauri app) ───────────────────────────┐
│  WebView (React + TS)                                                   │
│    UI components ── Zustand store ── api/ (typed invoke + listen)       │
│                       ▲ events (progress, repo-changed, auth-prompt)    │
│ ──────────────────────┼──────────────────── IPC (tauri commands) ────── │
│  Rust core            │                                                 │
│    commands/ ── RepoManager ── git/ (spawn git CLI, parse output)       │
│                 │            └── forgejo/ (REST client, reqwest)        │
│                 └── auth/ (keyring, accounts, trampoline server)        │
│                            ▲ localhost TCP, per-op token                │
└────────────────────────────┼────────────────────────────────────────────┘
          git.exe ── GIT_ASKPASS / credential.helper / SSH_ASKPASS ──► tenajlo-askpass (sidecar)
```

### 4.3 Responsibilities

**Rust core (`src-tauri/`)**
- Spawns all git processes. The frontend never builds git command lines.
- Owns all secrets. Tokens and passwords never cross into the WebView.
- Talks to the Forgejo REST API.
- Watches the working tree (`notify` crate, debounced) and emits `repo-changed`.
- Persists app state (repository list, accounts metadata, settings) as JSON in the app data dir.

**Frontend (`src/`)**
- Renders state and dispatches intents through typed commands.
- Contains no business logic about Git semantics beyond display.

**`tenajlo-askpass` (sidecar, Rust, `crates/askpass/`)**
- A tiny binary. Reads the prompt from argv (askpass mode) or stdin (credential-helper mode).
- Connects to `127.0.0.1:$TENAJLO_TRAMPOLINE_PORT`, sends `$TENAJLO_TRAMPOLINE_TOKEN` and the request, prints the response, then exits.
- Never stores anything.

### 4.4 Key crates and packages

| Area | Choice |
|---|---|
| App shell | Tauri 2.x (`bundle.externalBin` for the askpass sidecar, which git launches itself, so no `tauri-plugin-shell`; `tauri-plugin-dialog`) |
| Async | `tokio` |
| Typed IPC bindings | `specta` + `tauri-specta` (generate `src/bindings.ts`) |
| Secrets | `keyring` (Windows Credential Manager, macOS Keychain, Secret Service) |
| HTTP | `reqwest` 0.13 with `rustls` + `rustls-platform-verifier`, so the OS verifier and trust store apply (internal CA) |
| FS watching | `notify` + `notify-debouncer-full` |
| Errors | `thiserror` (library) / `anyhow` only at command boundary |
| Logging | `tracing` + rolling file appender, secrets redacted |
| Frontend | React 18+, TypeScript strict, Vite, Zustand, TanStack Query optional for API data |
| Diff rendering | Custom unified diff component (parse in Rust, render virtualized list) |
| Tests | `cargo test`, `tempfile`, Vitest, Forgejo in Docker for integration |

---

## 5. Git execution layer

### 5.1 Git binary resolution

1. Windows: bundled MinGit sidecar (pinned version, updated deliberately). Fall back to system git only if the user sets it in Settings.
2. Linux/macOS: system `git` from `PATH`.
3. On startup, run `git --version` and enforce a minimum of **2.40**. Show a blocking, friendly error if not met.

### 5.2 Spawning rules

- Spawn with an argument array. Never use a shell. Never interpolate user input into a string command.
- Always insert `--` before pathspecs.
- Reject user-supplied ref names or remote names that begin with `-`, and validate with `git check-ref-format`.
- Use a base environment for every invocation:
  - `GIT_TERMINAL_PROMPT=0` (never hang waiting on a TTY)
  - `GIT_ASKPASS`, `SSH_ASKPASS` → `tenajlo-askpass` path; `SSH_ASKPASS_REQUIRE=force`
  - `TENAJLO_TRAMPOLINE_PORT`, `TENAJLO_TRAMPOLINE_TOKEN` (per operation)
  - `LC_ALL=C` for parse stability (except commit message input)
  - `GIT_OPTIONAL_LOCKS=0` for read-only background commands (status, log)
- Use base `-c` options per invocation, never written to the user's global config:
  - `core.quotepath=false`
  - `credential.helper=` (clears inherited helpers), then `credential.helper=<tenajlo-askpass> credential` *when a Tenajlo account matches the remote host*. Otherwise the user's own helper chain stays intact (see §6.2).
  - Windows: `http.sslBackend=schannel` so the Windows cert store, including domain-pushed internal CAs, is trusted.
- Every operation has a timeout and is cancellable (kill the child process tree).
- At most one mutating operation per repository at a time, using a per-repo async mutex. Reads may run concurrently.

### 5.3 Operations (v1)

| Operation | Command basis | Output |
|---|---|---|
| Status | `status --porcelain=v2 --branch -z --untracked-files=all` | `WorkingDirectoryStatus` |
| Diff (file) | `diff --no-ext-diff --patience -M -z` / `diff --cached` / `diff --no-index` for untracked | `FileDiff` (hunks, lines, binary flag) |
| Stage / unstage | `add -- <paths>` / `restore --staged -- <paths>` | — |
| Commit | `commit -F -` (message on stdin) | commit SHA |
| Log | `log --format=<NUL-delimited custom> -z --max-count=N --skip=M` | `Commit[]` |
| Commit detail | `show --format=... --numstat -z <sha>` + per-file diff | `CommitDetail` |
| Branches | `for-each-ref --format=... refs/heads refs/remotes` | `Branch[]` |
| Checkout / create / delete | `switch`, `switch -c`, `branch -d/-D` | — |
| Fetch | `fetch --progress --prune <remote>` | progress events |
| Pull | `pull --progress --ff-only` default; setting for merge or rebase | progress events |
| Push | `push --progress <remote> <branch>`; `--set-upstream` on publish | progress events |
| Clone | `clone --progress -- <url> <path>` | progress events |
| Ahead/behind | `rev-list --left-right --count @{u}...HEAD` | `{ahead, behind}` |
| Stash | `stash push -u -m`, `stash pop`, `stash list` | — |
| Discard | `restore --worktree -- <paths>` (tracked), move to OS trash (untracked) | — |
| Ignore | append to `.gitignore` | — |
| Merge conflict detect | status codes `u` entries | conflict list |

### 5.4 Progress

Parse `--progress` stderr lines (`Receiving objects:  45% (450/1000)`, etc.) into `{ phase, percent, detail }` and emit `git-progress` events keyed by operation ID. Throttle to about 10 Hz.

### 5.5 Error mapping

Map git stderr patterns to typed errors. GitHub Desktop's `lib/git/core.ts` error table is a good reference.

| Pattern | `GitErrorKind` | UI |
|---|---|---|
| `Authentication failed` / HTTP 401/403 | `AuthFailed` | Re-auth dialog for that host |
| `Host key verification failed` | `HostKeyUnknown` | Host key confirmation dialog |
| `Permission denied (publickey)` | `SshKeyRejected` | SSH help dialog |
| `non-fast-forward` / `rejected` | `PushRejected` | "Pull first" prompt |
| `Not possible to fast-forward` | `PullDiverged` | Offer merge or rebase |
| `would be overwritten by checkout` | `LocalChangesBlock` | Offer stash and switch |
| `SSL certificate problem` | `TlsUntrusted` | Explain internal CA, link to settings |
| `Could not resolve host` | `HostUnreachable` | Check network or VPN |
| `CONFLICT` | `MergeConflict` | Conflict view |
| anything else | `Unknown` | Show raw stderr (redacted) in expandable details |

---

## 6. Authentication

### 6.1 Account model

```ts
type Account = {
  id: string;                 // uuid
  kind: "forgejo";            // future: "generic-https"
  baseUrl: string;            // https://TMC-GIT01.tmus.local
  login: string;              // from GET /api/v1/user
  displayName: string;
  avatarUrl?: string;
  tokenScopes?: string[];     // if discoverable
  sshHost?: string;           // optional override, e.g. TMC-GIT01.tmus.local:2222
};
// Secret stored separately in OS keychain:
//   service = "tenajlo", user = `${baseUrl}|${login}` → PAT
```

Only metadata goes to disk and to the frontend. The PAT lives only in the keychain.

### 6.2 HTTPS remotes (Git credentials + PAT)

Resolution order when git asks for credentials for `https://host/...`:

1. **Tenajlo account for this host** → return `username=<login>`, `password=<PAT>` from the keychain. Forgejo accepts PATs as the HTTPS password.
2. **User's existing git credential helper** (e.g. Git Credential Manager). When no Tenajlo account matches the host, Tenajlo does not override `credential.helper`, so the user's chain runs as normal.
3. **Prompt** via the trampoline. Show a UI dialog for username and password or token. (A "Save to keychain" option is deferred to v1.1: hosts that need saved credentials should use a Forgejo account, or the user's own helper.)

On `AuthFailed` for an account host, check the PAT with `GET /api/v1/user`. If that returns 401, the token is dead: keep the account metadata, mark it as needing sign-in, and show a re-enter-token dialog once. If the token still works, the failure is a repository permission problem: say so and keep the PAT. Never loop.

When the credential helper protocol sends `erase`, delete the keychain entry. On `store`, save it only if the user opted in.

### 6.3 SSH remotes

- Use the system OpenSSH client (Windows ships `C:\Windows\System32\OpenSSH\ssh.exe`; prefer it over MinGit's ssh so the Windows `ssh-agent` service is used). Set `core.sshCommand` per invocation.
- **Key passphrase:** `SSH_ASKPASS` → trampoline → UI dialog. Optionally remember the passphrase in the keychain per key fingerprint (off by default).
- **Unknown host key:** detect the prompt text, show host, key type, and fingerprint, and let the user accept or reject. On accept, answer `yes` so OpenSSH writes `known_hosts` itself. Tenajlo never edits `known_hosts` directly.
- **Key management (v1.1):** list keys in `~/.ssh`, generate an ed25519 key, and upload the public key to Forgejo (`POST /api/v1/user/keys`, needs `write:user` scope).

### 6.4 Forgejo sign-in flow

1. User enters an instance URL. Normalize it (add `https://`, strip trailing `/`).
2. `GET {base}/api/v1/version` checks reachability and that it is Forgejo/Gitea compatible. On TLS error, show the internal-CA explanation.
3. User pastes a PAT. Show a deep link to `{base}/user/settings/applications` and the required scopes: `read:user`, `read:repository`, `write:repository`.
4. `GET {base}/api/v1/user` with `Authorization: token <PAT>`. On success, save the account and keychain entry.
5. **v2:** OAuth2 authorization-code + PKCE flow against Forgejo's built-in OAuth2 provider, using a loopback redirect. This replaces PAT pasting for most users.

### 6.5 Trampoline protocol

- At app start, bind a TCP listener on `127.0.0.1:0` and record the port.
- For each git operation, mint a random 32-byte token, valid only for that operation's lifetime, and map it to `{ repoId, operationId, remoteUrl }`.
- Each request is one line of JSON: `{ token, mode: "askpass" | "credential", prompt?, op?: "get"|"store"|"erase", fields? }`.
- Requests with unknown or expired tokens are rejected and logged without the token.
- If the user must answer, the core emits an `auth-prompt` event to the UI and awaits the reply, with a 5-minute timeout. Cancelling or timing out kills the git process.

---

## 7. Forgejo API integration (v1 scope)

| Purpose | Endpoint |
|---|---|
| Health/version | `GET /api/v1/version` |
| Current user | `GET /api/v1/user` |
| Repos for clone dialog | `GET /api/v1/user/repos?limit=50&page=N` — includes org repos reachable through teams (verified on Forgejo 11); listing orgs would need the extra `read:organization` scope |
| User's orgs | `GET /api/v1/user/orgs` |
| SSH keys (v1.1) | `GET/POST /api/v1/user/keys` |

The client handles pagination via the `Link` / `X-Total-Count` headers, caches responses briefly with a refresh button, and uses a 15 s timeout.

---

## 8. UI specification

The layout mirrors GitHub Desktop, which users may already know.

```
┌───────────────────────────────────────────────────────────────────┐
│ [Current repo ▾]  [Current branch ▾]  [Fetch/Pull/Push origin ↻]   │  toolbar
├──────────────────┬────────────────────────────────────────────────┤
│ Changes | History│                                                │
│ ☑ file1.py   M   │          Diff viewer (unified)                  │
│ ☑ README.md  M   │                                                │
│ ☐ new.txt    ?   │                                                │
│                  │                                                │
│ ┌──────────────┐ │                                                │
│ │ Summary      │ │                                                │
│ │ Description  │ │                                                │
│ │[Commit to main]│                                                │
└──────────────────┴────────────────────────────────────────────────┘
```

### 8.1 Screens and dialogs
- **Welcome / first run:** sign in to Forgejo, or skip. Set git name and email (writes `user.name` and `user.email` globally only after confirmation, prefilled from the Forgejo profile).
- **Repository list (dropdown):** recent repos, filter box, "Add local…", "Clone…", "Create new…" (`git init`).
- **Clone dialog:** tabs for *Your Forgejo repos* (list from API) and *URL*. Choose HTTPS or SSH URL (SSH from M5; M4 clones HTTPS only), then the local path. Default path is `~/Documents/Tenajlo/<repo>`, configurable.
- **Changes tab:** file list with checkboxes (stage state), status icons, and a right-click menu (discard, ignore, reveal in explorer, open in editor). Commit box at the bottom.
- **History tab:** virtualized commit list (author, relative time, summary). Selecting one shows the changed files and diff.
- **Branch dropdown:** filter, current, recent, other local, remote-only (check out creates a tracking branch), plus "New branch…". When switching with local changes, offer *Bring changes* or *Stash and switch* (GitHub Desktop behavior).
- **Toolbar sync button:** context-aware label: *Publish branch* / *Fetch origin* / *Pull origin (↓3)* / *Push origin (↑2)*, with a progress bar during the operation.
- **Auth prompts:** HTTPS credentials, SSH passphrase, and host-key confirmation dialogs, all driven by `auth-prompt` events.
- **Accounts dialog (M4 interim):** list, sign in, sign out; opened from the repository dropdown and the no-repository screen. One account per server. Folds into Settings in M6.
- **Settings:** accounts, git identity, default clone path, external editor (VS Code / Notepad++ / custom), pull strategy, background fetch interval (default 5 min, 0 = off), theme (system / light / dark), git binary override.
- **Conflict banner:** lists conflicted files with "Open in editor" and "Mark resolved" (`git add`). Commit is blocked until resolved. Abort merge is available.

### 8.2 UX rules
- Every mutating action shows progress and is undoable or confirmable. Discarding changes moves untracked files to the OS trash, never hard-deletes.
- Error dialogs show a plain-language message first and raw git output behind "Details".
- Keyboard shortcuts follow GitHub Desktop where sensible (`Ctrl+Enter` commit, `Ctrl+Shift+P` push, `Ctrl+Shift+N` new branch, `Ctrl+T` repo list).
- Respect the OS dark/light setting. Meet WCAG AA contrast.

---

## 9. Data and persistence

| Data | Location | Format |
|---|---|---|
| Repository list (paths, last opened, alias) | `$DATA/repositories.json` | JSON, versioned schema |
| Accounts (no secrets) | `$DATA/accounts.json` | JSON |
| Settings | `$DATA/settings.json` | JSON |
| Secrets (PATs, optional passphrases) | OS keychain | — |
| Logs | `$DATA/logs/` | rolling, 7 days, redacted |

`$DATA` is Tauri's app data directory for the bundle identifier `com.kovirlabs.tenajlo`: `%APPDATA%\com.kovirlabs.tenajlo` on Windows, `~/.local/share/com.kovirlabs.tenajlo` on Linux, `~/Library/Application Support/com.kovirlabs.tenajlo` on macOS.

Every JSON file has a `"schemaVersion"` field and a migration function. Writes are atomic: write to a temp file, then rename.

---

## 10. Security requirements

1. Secrets never reach the WebView, logs, crash reports, or disk outside the keychain.
2. Redact `https://user:token@host` patterns and `Authorization` headers in all logs and error details.
3. Tauri capabilities are least-privilege. The frontend gets only Tenajlo's own commands plus dialog open/save. No `fs`, `shell`, or `http` plugin access from JS.
4. Use a strict CSP with no remote scripts. All assets are bundled.
5. Never pass a shell. Use argument arrays only, with `--` before paths, and validate refs (§5.2).
6. The trampoline listener binds to loopback only, uses per-operation random tokens, and rejects everything else.
7. Never auto-accept SSH host keys. Never set `http.sslVerify=false`, and offer no UI toggle for it. Untrusted certs are fixed in the OS trust store.
8. Updates (v1.1) use Tauri updater with signature verification, from an internal update endpoint.
9. Repository paths opened by the user are canonicalized. Tenajlo operates only inside the selected repo root.

---

## 11. Testing strategy

| Layer | Approach |
|---|---|
| Git parsers | Rust unit tests with fixture outputs (porcelain v2, log, diff, progress lines), including unicode paths, renames, binaries, and CRLF |
| Git operations | Rust tests creating temp repos via `tempfile` plus real git, plus a bare repo as a fake remote |
| Auth/trampoline | Tests with a mocked prompt responder: token validation, timeout, cancel, erase |
| Forgejo client | Integration tests against `codeberg.org/forgejo/forgejo` in Docker (seeded user + PAT), behind `--features integration` |
| Frontend | Vitest + React Testing Library for components and store logic. `api/` is mocked. |
| End-to-end (v1.1) | Tauri WebDriver smoke test: sign in, clone, commit, push |
| Manual matrix | Windows domain machine with internal CA, Ubuntu, SSH with passphrase key, PAT with HTTPS |

CI (Forgejo Actions on TMC-GIT01) runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm lint`, `pnpm typecheck`, and `pnpm test` on every push, plus Windows build artifacts on tags.

---

## 12. Milestones

| # | Milestone | Done when |
|---|---|---|
| M0 | Skeleton | Tauri 2 + React app builds on Windows and Linux. Specta bindings generated. Git binary detection and version check. |
| M1 | Read-only repo | Add local repo, status, file diff, history list, commit detail, branch list. FS watcher refreshes. |
| M2 | Local writes | Stage/unstage, commit, discard, ignore, create/switch/delete branch, stash-on-switch |
| M3 | Remotes over HTTPS | Fetch, pull (ff-only), push, publish, ahead/behind, progress UI. Trampoline with credential prompts and user's helper passthrough. |
| M4 | Forgejo accounts | Sign-in with PAT, keychain storage, clone dialog listing repos, automatic HTTPS auth for account hosts |
| M5 | SSH | System OpenSSH, passphrase prompt, host-key confirmation, clone via SSH URL |
| M6 | Polish and ship v1.0 | Conflict banner, settings screen, error mapping, Windows installer (MSI/NSIS), signed build, internal docs |
| v1.1 | Nice-to-haves | Hunk/line staging, SSH key generate + upload, Git LFS awareness (CAD/binary files), auto-update, background fetch notifications |
| v2 | Forgejo native | OAuth2 PKCE sign-in, open PR in browser from branch, PR status badge |

---

## 13. Open questions

1. **LFS:** Will TMC repos hold large binaries (CAD, PLC archives, EPLAN projects)? If so, LFS support moves from v1.1 to M6.
2. **SSH port:** Will Forgejo SSH run on 22 or an alternate port (e.g. 2222)? This affects the default clone URL and `sshHost` handling.
3. **Internal CA:** Is TMC-GIT01's certificate issued by a domain CA pushed via GPO? If so, schannel handles Windows. Linux machines need the CA installed manually.
4. **Distribution:** Internal-only (Intune/RMM push of MSI) or public under Kovir Labs? Public release affects naming, code signing, and the update endpoint.
5. **Pull default:** Should the default stay ff-only with a prompt on divergence (safest), or use merge (GitHub Desktop's default)?
