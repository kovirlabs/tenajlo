# Changelog

All notable changes to Tenajlo are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Everyday problems get a title that says what happened (for example "The server has newer
  commits", with a **Get the server's commits** button) instead of "Something went wrong".
- The commit button says why it's disabled when no files are ticked.
- **Undo** disappears once the commit is pushed, you switch branches, or the commit is a
  merge (which can't be undone), and it says how long ago you committed.
- Long paths in file lists cut off the folder rather than the file name. A renamed file shows
  its old name, and says so when its contents didn't change.
- Right-click menus work with the arrow keys, and right-clicking a file selects it.
- **New branch** says when a branch with that name already exists.
- The chosen repository in the clone list is clearly highlighted, and permission names on the
  sign-in screen no longer break in the middle of a word.
- The repository list no longer has an **Accounts…** button; accounts are in Settings.

### Fixed

- **New branch** refuses a name that differs from an existing branch only in capital letters
  (`Feature` next to `feature`). On Windows and macOS git could create both after a clone, and
  then a commit on one silently moved the other too. Tracking a server branch checks the same.
- When a push is refused because someone else has files locked, the details now say which files
  and who has them. Before, they only said "Cannot update locked files."
- **Pushes always check file locks.** Before, Tenajlo only refused a push that changes someone
  else's locked file if it had already listed the server's locks that session, and a setting
  git-lfs writes after a server error could turn the check off for good. Otherwise git-lfs
  only warned and the push went through.
- People with read-only access see who has files locked. Before, the lock list was empty
  for them.
- A repository on a network share or a folder owned by another Windows account gets the
  "owned by someone else" explanation again, instead of "This lock belongs to someone else".
- History drops the **Not pushed** marker as soon as the commit is pushed.
- A branch whose name on the server differs from yours (e.g. `fix` tracking `evan/fix`)
  shows its pull request, and **Create pull request…** opens the right branch.
- Turning pull requests off for a repository no longer hides commit checks and **View on
  Forgejo**.
- Forgejo links work for SSH remotes written as `host:owner/repo.git` (no `git@`).
- Ctrl+Shift+P, Ctrl+Shift+N, Ctrl+T and Ctrl+, do nothing while a confirmation, error or
  sign-in dialog is open, not just Settings and the other main dialogs.
- "Checked 3 minutes ago" on the sync button keeps up with the clock.
- Publish repository suggests a name the server accepts (a folder called "My Project" becomes
  `My-Project`), can't be cancelled halfway, and says that names can't end in `.git`.
- After a lock or unlock fails, the lock list reloads to match the server.
- Changes you leave on a branch come back with the same files ticked.
- History moves to the newest commit as you work, unless you picked an older one.
- "Sign in to Forgejo" no longer shows on the start screen when you're already signed in.
- Settings → Git no longer shows `\\?\` in front of the bundled Git's path on Windows.
- Settings → Repositories shows the real folder new clones go in (for example under OneDrive).
- Signing in to a server that only uses `http://` without typing it explains that, instead of
  "Couldn't reach the server".
- The discard error says Recycle Bin on Windows.

## [1.4.0] - 2026-11-05

### Added

- **File locks for large files**: in repositories that use Git LFS, lock a CAD model or PLC
  project before editing it so nobody else changes it at the same time. The sidebar shows how
  many files are locked; **Manage** lists every lock with who holds it, and lets you lock a
  file, unlock yours, or force-unlock someone else's. Locked files get a padlock in your
  changes, the commit box warns when you include a file someone else has locked, and Tenajlo
  won't push changes to such a file until it's unlocked.
- **Previews for binary files**: instead of "this binary file has changed", see the size
  before and after, and images side by side. Files stored with Git LFS show their real size
  (not the pointer) and their lock.
- **Pull requests and checks**: when you're signed in to the repository's server, the branch
  menu shows the branch's open pull request (and its number next to the branch name), or
  offers **Create pull request…** once the branch is pushed. In History, pushed commits show
  whether their checks passed, failed or are still running, with **View on Forgejo**.
- **Keyboard-friendly lists**: in Changes, History and a commit's files, the arrow keys (and
  Home, End, Page Up, Page Down) move the selection, Space ticks or unticks a file, and the
  Menu key or Shift+F10 opens the right-click menu.
- **Discard all changes…**: right-click the "changed files" line (or any file) to undo every
  change at once. Discarded files go to the Recycle Bin on Windows (the dialog now says so) or
  the Trash elsewhere.
- **Not pushed** marker in History for commits that are only on your computer, and a way to
  copy a commit's full ID (right-click, or **Copy ID** above its files).
- **Remove from list…**: right-click a repository in the repository list to take it off
  Tenajlo's list. Its files stay where they are.

### Changed

- **Clearer sync button**: it now says what it will do: **Check for changes**, **Get 3
  commits**, **Send 2 commits** or **Publish branch**, with "Checked 3 minutes ago" below.
  Point at it for a full sentence that also names the git command (fetch, pull or push).
- **New look**: warm "solar" colors in both themes (paper and sand in light mode, ember tones
  in dark mode) with a sun-orange accent. The toolbar has a sunrise band, the app icon and
  rounded buttons; Changes/History is a pill switch; the selected file or commit gets an accent
  bar; and file status badges are solid. Checkboxes use the accent color instead of the system
  blue. The website's dark mode matches.
- **Commit messages are kept per repository** while Tenajlo is open, so switching to
  another repository and back no longer loses a half-written summary.
- Tenajlo now also checks the server when you open a repository or come back to the window,
  if it last checked over a minute ago (only when background checks are on).
- If you start a commit while a fetch, pull or push is running, the commit box says it's
  waiting for it to finish. In the repository list, Enter opens the first match, and the
  filter is cleared each time the list opens.

### Fixed

- When the list of changes can't be loaded, there's now a **Try again** button.
- **Cancel now works while a push or pull is waiting.** If a background fetch was still running,
  clicking Cancel on a push or pull did nothing, and the push still ran once the fetch finished.
  Now it stops right away.
- **Ctrl+Shift+P and Ctrl+Shift+N are ignored while a dialog is open**, so they can no longer
  start a push or open New Branch behind Settings or another dialog.

## [1.2.0] - 2026-10-19

### Added

- **Publish repository**: a repository that's only on your computer can now be put on your
  Forgejo server from the toolbar. Choose your own account or an organization, a name, and
  whether it's private; Tenajlo creates the repository on the server and publishes your branch
  to it. This needs extra access token permissions (`write:user` and `write:repository`, plus
  `write:organization` for organizations). If your token doesn't have them, Tenajlo offers to
  replace it.

### Changed

- The app icon has light and shadow, so it looks three-dimensional in the Dock, taskbar and on
  desktop shortcuts. On macOS it is the same size as other icons in the Dock.

## [1.1.3] - 2026-10-17

### Fixed

- Switching to, adding or opening another repository shows only that repository. Before, the
  previous repository's file list and diff stayed on screen and the new ones stacked below.

## [1.1.2] - 2026-10-16

### Fixed

- You can sign in to a Forgejo server that only lets signed-in users see it. Before, Tenajlo
  stopped at "Only signed in user is allowed to call APIs" before you could paste your token.

## [1.1.1] - 2026-10-13

### Changed

- Dark text on the orange buttons in the dark theme is bold, so it's easier to read.

### Fixed

- When the server has commits you don't, a push no longer leaves you stuck with a Push
  button that keeps failing: Tenajlo gets those commits and the button changes to Pull.

## [1.1.0] - 2026-10-10

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

[Unreleased]: https://github.com/kovirlabs/tenajlo/compare/v1.4.0...HEAD
[1.4.0]: https://github.com/kovirlabs/tenajlo/compare/v1.2.0...v1.4.0
[1.2.0]: https://github.com/kovirlabs/tenajlo/compare/v1.1.3...v1.2.0
[1.1.3]: https://github.com/kovirlabs/tenajlo/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/kovirlabs/tenajlo/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/kovirlabs/tenajlo/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/kovirlabs/tenajlo/compare/v1.0.2...v1.1.0
[1.0.2]: https://github.com/kovirlabs/tenajlo/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/kovirlabs/tenajlo/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/kovirlabs/tenajlo/releases/tag/v1.0.0
