# Releasing Tenajlo

Releases are built by GitHub Actions (`.github/workflows/release.yml`) from a version tag and
land in a **draft** GitHub Release, so a person checks them before anyone can download them.

## 1. Prepare

1. Make sure `main` is green in CI (Windows, Linux, macOS, and the Forgejo integration job).
2. Set the new version in all three places (they must match):
   - `src-tauri/tauri.conf.json` → `version`
   - `src-tauri/Cargo.toml` → `[package] version`
   - `package.json` → `version`
3. In `CHANGELOG.md`, give the version's section today's date (it says `Unreleased` until
   then) and add an empty `## [Unreleased]` above it if there isn't one.
4. Commit: `Release v1.2.3`.

The dependency license list is automatic: `scripts/third-party-licenses.mjs` (cargo-about for
Rust crates, `pnpm licenses` for npm packages) runs in CI on every push and fails if a
dependency's license isn't accepted in `about.toml`. The release build bundles the result as
`THIRD_PARTY_LICENSES.html` and attaches it to the release. If CI rejects a new dependency,
check its license before adding it to the allow-list.

## 2. Build

```bash
git tag v1.2.3
git push origin main v1.2.3
```

The release workflow checks the tag matches `tauri.conf.json`, builds `tenajlo-askpass`,
downloads the pinned MinGit and git-lfs (`scripts/fetch-mingit.mjs` verifies their SHA-256),
generates `THIRD_PARTY_LICENSES.html`, and builds the NSIS installer. Linux (`ubuntu-22.04`, so
the `.deb` runs on older glibc too) and macOS (Apple Silicon) build a `.deb` and a `.dmg` in
parallel; they use the system's git, so they skip the MinGit download. Each platform's build
settings are in `src-tauri/tauri.<platform>.conf.json`. A first job creates the draft release
and the three builds upload into it, along with the license list.

## 3. Check before publishing

### Windows

Install the draft's installer on a clean Windows 10 or 11 machine (no Git installed) and run
through the manual matrix from spec §11. [`dev/test-server/`](../dev/test-server/README.md)
sets up a local Forgejo with users, a token and a sample repository for this:

- [ ] The installer runs without administrator rights; SmartScreen's "unknown publisher"
      warning is expected while builds are unsigned.
- [ ] Tenajlo starts and Settings → Git shows the bundled Git and Git LFS versions.
- [ ] Sign in with a PAT; clone a private repository over HTTPS with no password prompt.
- [ ] Commit and push; pull a teammate's change; merge a divergence; resolve a conflict.
- [ ] Clone over SSH (port 2222) with a passphrase-protected key: the host-key dialog and
      passphrase dialog appear in Tenajlo, not in a console window.
- [ ] A repository with LFS files clones with real content, and pushes new large files.
- [ ] On a domain-joined machine: the internal CA is trusted (schannel), and a path longer
      than 260 characters clones.
- [ ] Uninstall removes the app; reinstalling keeps accounts and settings.

### Linux (Ubuntu 24.04 or Debian 13)

- [ ] `sudo apt install ./Tenajlo_<version>_amd64.deb` installs it, pulling in git and
      git-lfs; Tenajlo appears in the app menu.
- [ ] Sign in (the token is saved in the desktop keyring); clone, commit, push, pull over HTTPS
      and SSH; an LFS repository clones with real content.
- [ ] `sudo apt remove tenajlo` removes it.

### macOS (Apple Silicon)

- [ ] The `.dmg` opens and Tenajlo drags to Applications. The first launch is blocked as
      unverified; System Settings → Privacy & Security → **Open Anyway** starts it.
- [ ] Sign in (the token is saved in the Keychain); clone, commit, push, pull over HTTPS and
      SSH; the SSH passphrase prompt appears in Tenajlo.

### Updates

- [ ] The draft has `latest.json`, plus a `.sig` file next to the NSIS installer, the macOS
      `.app.tar.gz` and the `.deb`. (They're missing if the `TAURI_SIGNING_PRIVATE_KEY` secret
      isn't set.) `latest.json` lists `windows-x86_64`, `darwin-aarch64` and `linux-x86_64-deb`.
- [ ] After publishing: a copy of the previous version shows "Tenajlo <version> is available"
      at startup, and **Install and restart** installs it and reopens Tenajlo. On Linux, the
      system asks for an administrator password first (`pkexec`).

Then publish the draft release. Installed copies only see a release once it's published:
`latest.json` is served from the latest _published_ release.

## 4. After publishing

Publishing triggers `.github/workflows/package-managers.yml`, which opens the winget pull
request and pushes the Chocolatey package (each only if its secret is set; see
[Package managers](#package-managers)). Then point the Scoop manifest and the Chocolatey files
in this repository at the new installer and commit:

```bash
node scripts/update-packaging.mjs 1.2.3
git commit -am "Packaging: point Scoop and Chocolatey at v1.2.3"
```

The script takes the installer's SHA-256 from the digest GitHub publishes for the asset. Scoop
users get the new version on their next `scoop update` once this commit is on `main`.

## Package managers

Windows users can install Tenajlo with winget, Scoop or Chocolatey. All three install the same
NSIS installer from the GitHub release, per-user and without administrator rights.

| Manager    | Where the package lives                                      | Updated by                                              |
| ---------- | ------------------------------------------------------------ | ------------------------------------------------------- |
| winget     | `microsoft/winget-pkgs` (`KovirLabs.Tenajlo`)                | `package-managers.yml` → `wingetcreate update --submit` |
| Scoop      | `bucket/tenajlo.json` in this repository                     | `scripts/update-packaging.mjs` after each release       |
| Chocolatey | community.chocolatey.org (`tenajlo`), `packaging/chocolatey` | `package-managers.yml` → `choco push`                   |

`packaging/winget/manifests/` holds the manifests for the first winget submission (v1.4.0).
After that, winget-pkgs is the source of truth and wingetcreate copies the previous version's
manifest forward, so the files here don't need to change.

### One-time setup (in this order)

1. **Merge the packaging files** (this repository): `bucket/`, `packaging/`,
   `scripts/update-packaging.mjs` and `package-managers.yml`. Scoop works as soon as they're
   on `main`: `scoop bucket add tenajlo https://github.com/kovirlabs/tenajlo`. Don't merge the
   winget and Chocolatey commands in the user guide until steps 2 and 4 are accepted.
2. **First winget submission** (on a Windows machine): winget-pkgs only accepts updates
   through wingetcreate once the package exists, so the first version is a normal PR.
   - `winget install wingetcreate`, then from this repository:
     `wingetcreate submit packaging/winget/manifests/k/KovirLabs/Tenajlo/1.4.0` (it signs you
     in to GitHub, forks `microsoft/winget-pkgs` and opens the PR), or copy that folder into a
     fork of winget-pkgs at the same path and open the PR by hand.
   - Before submitting, `winget validate --manifest <folder>` and
     `winget install --manifest <folder>` (needs `winget settings --enable LocalManifestFiles`
     as administrator) check it locally.
   - The PR runs Microsoft's validation pipeline (installs the app in a VM, scans it). A
     moderator merges it, usually within a few days. Unsigned installers are accepted, but
     the scan can flag them for manual review; answer any bot or moderator comments on the PR.
3. **`WINGET_TOKEN` secret** (for automatic updates): create a GitHub personal access token
   **(classic)** with only the `public_repo` scope, on the account that will own the
   winget-pkgs fork (wingetcreate doesn't support fine-grained tokens). Give it an expiry and
   a calendar reminder. Then `gh secret set WINGET_TOKEN`. From the next published release,
   the workflow opens the winget-pkgs PR itself; it still needs a moderator to merge it.
4. **Chocolatey** (optional): create an account on community.chocolatey.org, then on a
   Windows machine `cd packaging/chocolatey`, `choco pack`, and
   `choco push tenajlo.1.4.0.nupkg --source https://push.chocolatey.org/ --api-key <key>`.
   The first version of a package goes through automated checks (validation, verification —
   it installs and uninstalls the package — and a VirusTotal scan) and then **human
   moderation**, which can take days to weeks. Unsigned installers are allowed but the
   VirusTotal step tends to flag them, which needs a reply on the package page. Once the
   package is approved, add the API key (Account → API Keys) as the `CHOCO_API_KEY` secret;
   later versions are pushed by the workflow and go through the same automated checks.

If a submission fails or is rejected, rerun it from Actions → **Package managers** → Run
workflow with the version.

### Notes

- **Scoop** extracts the NSIS installer with 7-Zip (`#/dl.7z`) instead of running it, the
  usual way Scoop packages Tauri apps. That keeps it per-user and out of Apps & Features, but
  Tenajlo still thinks it's an NSIS install, so its own update banner would run the installer
  and put a second copy in `%LocalAppData%\Tenajlo`. The manifest's notes tell Scoop users to
  update with `scoop update tenajlo` and turn update checks off in Settings → About.
  `checkver` and `autoupdate` let Scoop's own tooling (`checkver.ps1 tenajlo -u`) bump it too.
- **Chocolatey** usually runs elevated. The installer is per-user, so Tenajlo lands in the
  `%LocalAppData%` of the account that ran `choco install` (normally the signed-in user, via
  UAC). An elevated _different_ account would install it for that account instead.
- **winget** runs the installer with `/S` (silent) or `/P` (progress bar only); the
  manifest's `ProductCode: Tenajlo` matches the uninstall key Tauri writes, so `winget upgrade`
  and `winget uninstall` find it, including after the in-app updater installed a new version.
- **SmartScreen**: package managers don't avoid the code-signing question. winget and
  Chocolatey check the SHA-256, but Windows may still show "unknown publisher" for interactive
  installs, as with a direct download.

### No Homebrew cask

Homebrew 5.0 deprecated casks that fail macOS Gatekeeper (not code-signed with a Developer ID
and notarized), and announced that such casks would be removed from the official
`homebrew/cask` tap from September 2026, along with the `--no-quarantine` escape hatch. Tenajlo's
macOS app is ad hoc signed and not notarized (see below), so it can't go into the official
tap. If a Homebrew install is wanted anyway, the route is a personal tap: a separate
`kovirlabs/homebrew-tenajlo` repository with a `Casks/tenajlo.rb`, installed with
`brew install kovirlabs/tenajlo/tenajlo`. Users would still need to allow the app once in
System Settings.

## Updating bundled Git or Git LFS

Edit the `MINGIT` / `GIT_LFS` constants in `scripts/fetch-mingit.mjs`: version, URL and the
SHA-256 from the release's published digest (GitHub shows it on the release asset; the
GitHub API returns it as `digest`). Update the versions in `THIRD_PARTY_NOTICES.md`. Run
`node scripts/fetch-mingit.mjs --force` to check the download and layout.

## Code signing (not planned)

Decided on 2026-10-10: Windows installers aren't code-signed and the macOS app isn't
notarized. Users see SmartScreen's "unknown publisher" warning on Windows and allow the app
once in System Settings on macOS (the user guide explains both). Update packages are still
signed with Tenajlo's update key (below), so auto-update is unaffected.

If that changes: for Windows, set
`bundle.windows.signCommand` in `src-tauri/tauri.windows.conf.json` (or
`certificateThumbprint` for a local certificate) and pass its secrets to the release job.
See Tauri's Windows code-signing guide.

The macOS app is signed ad hoc (`"signingIdentity": "-"` in `src-tauri/tauri.macos.conf.json`),
which Apple Silicon needs to run it at all, but it isn't notarized, so Gatekeeper asks users to
allow it once. Notarizing needs an Apple Developer ID certificate: set `signingIdentity` to it
and pass `APPLE_CERTIFICATE`, `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID` to the macOS
build. See Tauri's macOS code-signing guide.

## Update signing

Installed copies of Tenajlo download updates only if they're signed with Tenajlo's update key
(a minisign key made by Tauri, separate from code signing and from any GPG key). The public
half is in `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`; the private half and its
password are GitHub secrets that only the release workflow sees.

### One-time setup

1. Create the key pair on your own computer (not in CI), and choose a strong password when asked:

   ```bash
   pnpm tauri signer generate -w ~/.tauri/tenajlo-updater.key
   ```

   This writes `~/.tauri/tenajlo-updater.key` (private, password-protected) and
   `~/.tauri/tenajlo-updater.key.pub` (public).

2. **Back up the private key file and its password** somewhere safe and separate, such as a
   password manager. If they're lost, installed copies can never verify a new update: everyone
   would have to download and reinstall Tenajlo by hand.
3. Add both as repository secrets (Settings → Secrets and variables → Actions, or with `gh`):

   ```bash
   gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/tenajlo-updater.key
   gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD   # paste the password when asked
   ```

4. Put the contents of `~/.tauri/tenajlo-updater.key.pub` into `plugins.updater.pubkey` in
   `src-tauri/tauri.conf.json` and commit it. The public key isn't secret.

From the next release on, the Windows, macOS and Linux builds sign their update packages and the
release gets a `latest.json`. Copies installed _before_ the public key was added can't update
themselves; they need one manual install of a version that has it.

If the secret is set but `pubkey` is empty, the release workflow stops with an error rather
than publishing updates nobody can verify.

### Replacing the key

Only if the private key leaks: generate a new pair, update both secrets and `pubkey`, and
release. Copies with the old public key will reject updates signed with the new one, so tell
users to download the new version by hand.

## Codeberg mirror

`.github/workflows/mirror.yml` pushes `main` and the `v*` tags to Codeberg after every push, so
the Forgejo community can find Tenajlo there. Installers stay on GitHub Releases. One-time
setup:

1. On Codeberg, create the `kovirlabs` organization (or use a personal account) and an empty
   `tenajlo` repository. In its description, say it's a mirror and link to the GitHub
   releases; turn off issues and pull requests there so reports land in one place.
2. Create a Codeberg access token with the `write:repository` permission.
3. In the GitHub repository's **Settings → Secrets and variables → Actions**, add the
   variables `CODEBERG_REPO` (`kovirlabs/tenajlo`) and `CODEBERG_USER` (the account that
   owns the token), and the secret `CODEBERG_TOKEN`.
4. Run the **Mirror to Codeberg** workflow once by hand (**Actions → Run workflow**).

Until `CODEBERG_REPO` is set, the workflow is skipped.
