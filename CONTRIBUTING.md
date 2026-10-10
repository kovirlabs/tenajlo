# Contributing to Tenajlo

Thanks for helping! Bug reports, ideas, documentation fixes and code are all welcome.

## Before you start

- **Questions and ideas:** start a [discussion](https://github.com/kovirlabs/tenajlo/discussions).
- **Bugs:** open an [issue](https://github.com/kovirlabs/tenajlo/issues/new/choose) with the
  steps to reproduce, and the error's **Details** text if there is one (Tenajlo removes
  passwords and tokens from it). Log files are in **Settings → About → Show log files**.
- **Security problems:** please report them privately; see [SECURITY.md](SECURITY.md).
- **Bigger changes:** open an issue or discussion first, so we can agree on the approach.
  [`spec.md`](spec.md) describes what's in scope; features outside it need a conversation.

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

You need Rust (stable), Node.js 24 with pnpm, Git 2.40+ and Git LFS, plus
[Tauri's system dependencies](https://tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm tauri dev
```

Integration tests run against a local Forgejo in Docker:

```bash
docker compose -f dev/forgejo.yml up -d && dev/seed-forgejo.sh
cd src-tauri && cargo test --features integration
```

To try the app by hand against a server of your own (with users, a token and a sample
repository generated for you), see [`dev/test-server/`](dev/test-server/README.md).

## Making a change

1. Read the relevant section of [`spec.md`](spec.md). The architecture rules in
   [`CLAUDE.md`](CLAUDE.md) apply to all contributions (it doubles as the project's
   engineering guide). The most important ones:
   - Only `src-tauri/src/git/exec.rs` starts git, always with an argument list.
   - Secrets never reach the frontend or logs; log anything URL-like through `redact()`.
   - Never weaken TLS or SSH host-key checks.
   - The frontend renders data and calls `src/api/`; Git logic lives in Rust.
2. Add tests: pure parsers get fixture tests, Git operations get temp-repo tests, and
   components get Vitest tests.
3. If you change a Rust command, run `pnpm bindings` (never edit `src/bindings.ts` by hand).
4. Run everything CI runs:

   ```bash
   pnpm typecheck && pnpm lint && pnpm test
   cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace
   ```

5. Write UI text in plain language for people who aren't Git experts.
6. New dependencies need a reason in the PR, and a license that `about.toml` accepts (CI
   checks this).

## Pull requests

Keep each PR focused on one change, describe what it does and how you tested it, and add a
line to [CHANGELOG.md](CHANGELOG.md) under "Unreleased" for anything users will notice. By
contributing, you agree that your contributions are licensed under the
[Apache License 2.0](LICENSE).
