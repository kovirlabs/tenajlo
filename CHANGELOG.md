# Changelog

All notable changes to Tenajlo are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.1.0] - Unreleased

### Added

- **Commit part of a file.** Tick single lines in the diff, or a whole block, to include only
  those in the next commit. Shift+click ticks a range.
- **SSH keys in Settings.** Create an SSH key and add it to your Forgejo account without the
  command line. Tenajlo checks that your access token is allowed to add keys first, and
  offers to replace it with one that is.
- **Remember passwords.** Sign-in and SSH passphrase dialogs have a "Remember" checkbox; the
  secret goes to this computer's password store. Settings → Passwords lists and forgets them.
  A saved password the server stops accepting is forgotten automatically.
- **Automatic updates.** Tenajlo checks for a new version at startup and installs it when you
  click **Install and restart**, after checking it's signed by Tenajlo's release key. On Linux
  this asks for your administrator password. Turn the check off in Settings → About. Copies
  of 1.0.x can't update themselves: install 1.1.0 by hand once.
- **Notifications** when a background fetch finds new commits on your branch, while you're
  using another window. Turn them off in Settings → Repositories.

### Changed

- Settings lists its sections down the side, and every dialog fits the window.
- Long repository and branch names are shortened with "…"; long diff lines scroll sideways;
  History puts the commit's files above the diff in narrow windows.

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

## [1.0.2] - 2026-10-09

### Fixed

- The **Current repository** dropdown is no longer cut off at the left edge of the window.
- Dropdowns and right-click menus always stay inside the window, even when it's small or the
  menu opens near an edge.

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

[Unreleased]: https://github.com/kovirlabs/tenajlo/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/kovirlabs/tenajlo/compare/v1.0.2...v1.1.0
[1.0.2]: https://github.com/kovirlabs/tenajlo/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/kovirlabs/tenajlo/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/kovirlabs/tenajlo/releases/tag/v1.0.0
