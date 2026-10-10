# Changelog

All notable changes to Tenajlo are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- History no longer retries loading older commits forever after a failed load.
- Right-click menus keep keyboard focus where you put it while the app refreshes.
- Pressing Enter right after typing a branch name creates the name you typed.
- If switching branches fails, Tenajlo shows why, even when it can't put your changes back
  (they stay saved and can be restored from the banner).
- Signing in to one server while another account's token is being rejected no longer loses
  either change.
- The Changes list loads faster for folders with many untracked files, and very large new
  files are reported as too large to show without being read first.

## [1.0.1] - 2026-10-07

- Linux: a `.deb` package for Debian and Ubuntu (x64). It uses the system's Git.
- macOS: a `.dmg` for Apple Silicon Macs. It isn't notarized yet, so macOS asks you to allow
  it once in System Settings → Privacy & Security.

## [1.0.0] - 2026-10-07

First public release:

- Sign in to Forgejo with an access token, kept in the OS password store; HTTPS remotes on
  that server then work without passwords.
- Clone from a list of your Forgejo repositories or a URL, over HTTPS or SSH; create new
  repositories; add existing ones.
- Changes, diffs, commit (with undo), history, branches (create, switch with your changes,
  delete).
- Fetch, pull and push with progress; merge the server's changes when both sides changed;
  conflict banner with mark-resolved and abort.
- SSH via the system's OpenSSH with in-app passphrase and host-key prompts.
- Git LFS for large files, bundled on Windows with Git.
- Settings: accounts, name and email, Git program, pull behaviour, background fetch, editor,
  theme.
- First-run welcome, plain-language error messages, daily log files.

[Unreleased]: https://github.com/kovirlabs/tenajlo/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/kovirlabs/tenajlo/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/kovirlabs/tenajlo/releases/tag/v1.0.0
