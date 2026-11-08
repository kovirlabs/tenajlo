# Tenajlo

[![CI](https://github.com/kovirlabs/tenajlo/actions/workflows/ci.yml/badge.svg)](https://github.com/kovirlabs/tenajlo/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Platform: Windows | Linux | macOS](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](#build-from-source)

A free, open-source desktop Git client for self-hosted [Forgejo](https://forgejo.org) and
Gitea servers, in the spirit of GitHub Desktop. It's made for engineers who work with code, PLC
projects, CAD files and documentation and would rather not learn the Git command line. Git LFS
and file locking for binary files like SolidWorks models and TIA Portal archives are built in.

**Website:** <https://kovirlabs.github.io/tenajlo/> ·
**[Download](https://github.com/kovirlabs/tenajlo/releases/latest)** ·
**[User guide](https://kovirlabs.github.io/tenajlo/guide/)**

> [!NOTE]
> Designed, reviewed, and maintained by me, [@evangress](https://github.com/evangress). I use AI coding tools heavily for implementation,
> but I own the architecture, read the changes, and use this app daily against my own Forgejo server.
> Bug reports go to a human.

![Tenajlo's Changes view: changed files including a PLC program and a CAD part, the highlighted changes, and a commit box](site/assets/screenshots/changes.png)

- **Clone** your Forgejo or Gitea repositories from a list, over HTTPS or SSH.
- **Commit, push, pull and branch** with one sync button that says what it will do ("Get 3
  commits", "Send 2 commits"). Commit part of a file, undo a commit, discard to the Recycle Bin.
- **Sign in once** with an access token, kept in the OS password store. HTTPS remotes on that
  server then just work, and your existing Git credential helpers are left alone for other
  servers.
- **Large files and file locks** with Git LFS (bundled on Windows): lock a CAD model or PLC
  project before you edit it, see who has which file locked, and never push over someone
  else's lock.
- **Binary file previews:** image previews side by side, and real sizes for files stored with
  Git LFS.
- **Forgejo links:** your branch's pull request, each commit's checks (Forgejo Actions), and
  **View on Forgejo**. Publish a local folder as a new repository on the server, and add your
  SSH key to your account in one click.
- **Open in Tenajlo** (next release): server admins can add Tenajlo to every repository
  page's clone menu.
- **SSH** through your system's OpenSSH, with passphrase and host-key prompts shown in the
  app (never accepted automatically).
- **Conflicts** made approachable: merge the server's changes, open conflicted files, mark
  them resolved, or abort.
- **Works on company networks:** uses the OS certificate store (internal CAs), private DNS,
  and Windows long paths. No telemetry.

Windows 10/11 is the main platform; Linux and macOS work too. Tested against Forgejo 11 and
Gitea 1.27 and 28 on every change.

| History                                                                                             | Clone                                                                           | Conflicts                                                                                         |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| ![History view with commits and the selected commit's changes](site/assets/screenshots/history.png) | ![Clone dialog listing Forgejo repositories](site/assets/screenshots/clone.png) | ![Conflict banner with Open, Mark resolved and Abort merge](site/assets/screenshots/conflict.png) |

<sub>Screenshots show invented demo data; regenerate them with `node scripts/screenshots.mjs`.</sub>

## Install

Download the latest version from the
[Releases page](https://github.com/kovirlabs/tenajlo/releases/latest):

| Platform              | File                              | Notes                                                              |
| --------------------- | --------------------------------- | ------------------------------------------------------------------ |
| Windows 10/11 (x64)   | `Tenajlo_<version>_x64-setup.exe` | Installs for your user, no admin rights. Includes Git and Git LFS. |
| Debian / Ubuntu (x64) | `Tenajlo_<version>_amd64.deb`     | `sudo apt install ./Tenajlo_<version>_amd64.deb`                   |
| macOS (Apple Silicon) | `Tenajlo_<version>_aarch64.dmg`   | Needs Git 2.40+ (and Git LFS for large files).                     |

The installers aren't code-signed yet; the [user guide](docs/user-guide.md#install) explains
the Windows and macOS warnings. Tenajlo updates itself (you can turn that off in Settings).
Other Linux distributions and Intel Macs: [build from source](#build-from-source).

## Documentation

- [User guide](docs/user-guide.md) (also [on the website](https://kovirlabs.github.io/tenajlo/guide/)):
  install, sign in, clone, commit, share, conflicts, troubleshooting
- [Setting up Forgejo for an engineering team](docs/forgejo-team-setup.md) (also
  [on the website](https://kovirlabs.github.io/tenajlo/forgejo-setup/)): server settings, Git
  LFS and file locks for CAD and PLC files, teams, branch protection, onboarding
- Guides on the website:
  [a desktop client for Forgejo and Gitea](https://kovirlabs.github.io/tenajlo/forgejo-desktop-client/),
  [version control for CAD files](https://kovirlabs.github.io/tenajlo/git-for-cad/),
  [version control for PLC projects](https://kovirlabs.github.io/tenajlo/git-for-plc/)
- [Changelog](CHANGELOG.md)
- [Releasing](docs/releasing.md): how installers are built and checked
- [Specification](spec.md): scope, architecture and security model

## Build from source

You need Rust (stable), Node.js 24 with pnpm, Git 2.40+ and Git LFS, plus
[Tauri's system dependencies](https://tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm tauri dev      # run with hot reload
pnpm tauri build    # build an installer for this platform
```

Checks that CI runs:

```bash
pnpm typecheck && pnpm lint && pnpm test
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

Integration tests run against Forgejo in Docker (and against Gitea with `dev/gitea.yml`; see
CLAUDE.md):

```bash
docker compose -f dev/forgejo.yml up -d && dev/seed-forgejo.sh
cd src-tauri && cargo test --features integration
```

## Contributing

Questions and ideas go to [Discussions](https://github.com/kovirlabs/tenajlo/discussions), bugs
to [Issues](https://github.com/kovirlabs/tenajlo/issues). See [CONTRIBUTING.md](CONTRIBUTING.md)
and the [Code of Conduct](CODE_OF_CONDUCT.md). Please report security problems privately
([SECURITY.md](SECURITY.md)).

## Privacy

Tenajlo has no telemetry or analytics. It only connects to the Git and Forgejo servers you
use.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). Bundled third-party software
is listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Tenajlo is an independent project by Kovir Labs. It is not affiliated with, endorsed by or
sponsored by GitHub, Inc. "GitHub" and "GitHub Desktop" are trademarks of GitHub, Inc.
