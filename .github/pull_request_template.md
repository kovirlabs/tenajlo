## What does this change?

<!-- What and why. Link the issue or discussion, e.g. "Fixes #12". -->

## How was it tested?

<!-- Tests added or run, and any manual testing (OS, HTTPS/SSH, LFS…). -->

## Checklist

- [ ] `pnpm typecheck && pnpm lint && pnpm test` pass
- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace` pass
- [ ] `pnpm bindings` was run if a Rust command changed
- [ ] User-facing text is plain language
- [ ] CHANGELOG.md updated under "Unreleased" (if users will notice)
