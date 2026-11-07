# Tenajlo user guide

Tenajlo is a desktop app for working with Git repositories on a Forgejo server: get a copy of
a project, save your changes, and share them with your team, without the command line.

## Install

**Windows 10/11:** download `Tenajlo_<version>_x64-setup.exe` from the Releases page of
Tenajlo's GitHub repository and run it. It installs for
your user account only, so you don't need administrator rights. Git and Git LFS come with
Tenajlo; you don't need to install them.

> Tenajlo's installer isn't code-signed yet, so Windows may say "Windows protected your PC".
> Choose **More info → Run anyway** if you downloaded it from Tenajlo's Releases page.

**Debian and Ubuntu (x64):** download `Tenajlo_<version>_amd64.deb` from the Releases page
and install it with:

```bash
sudo apt install ./Tenajlo_<version>_amd64.deb
```

`apt` also installs Git and Git LFS if you don't have them. Tenajlo needs Git 2.40 or newer:
Ubuntu 24.04 and Debian 13 have it; on Debian 12, install `git` from bookworm-backports first.

**macOS (Apple Silicon, M1 or later):** download `Tenajlo_<version>_aarch64.dmg`, open it and
drag Tenajlo to Applications. Tenajlo needs Git 2.40 or newer: recent Apple command line tools
include it (`xcode-select --install`), or install it with `brew install git`. For large files,
also install [Git LFS](https://git-lfs.com) (`brew install git-lfs`).

> Tenajlo isn't notarized by Apple yet, so the first time you open it macOS says it can't
> check the app. Click **Done**, then open **System Settings → Privacy & Security**, scroll
> down and click **Open Anyway** next to Tenajlo. You only need to do this once.

**Other Linux distributions and Intel Macs:** build Tenajlo from source (see the
[README](../README.md)).

## Sign in to your Forgejo server

The first time you open Tenajlo, a welcome screen walks you through signing in and setting
the name and email Git records on your commits. You can skip either step and do it later in
**Settings**.

1. Open **Settings → Accounts** (or click **Sign in to Forgejo…** on the start screen).
2. Enter your server's address, for example `git.example.com`, and click **Continue**.
3. Click **Open token settings in your browser**. On that page, create an access token with
   these permissions:
   - `read:user`
   - `read:repository`
   - `write:repository`
4. Copy the token, paste it into Tenajlo, and click **Sign in**.

Tenajlo keeps the token in your computer's password store (Windows Credential Manager,
macOS Keychain, or the Linux Secret Service), never in a file. From then on, Tenajlo uses it
automatically whenever you work with repositories on that server.

**If your sign-in stops working** (for example, the token expired), Tenajlo tells you and
offers **Sign in again**. Paste a new token; nothing else changes.

## Get a copy of a repository (clone)

1. Click **Current repository → Clone…**.
2. On **Your Forgejo repositories**, pick a repository. Use **Filter** to find it.
3. Choose how to connect:
   - **HTTPS** uses your Forgejo account. Pick this if you're not sure.
   - **SSH** uses your SSH key. Your key must be added to your Forgejo account first (see
     [SSH keys](#ssh-keys)).
4. Check the **Local folder** (by default `Documents\Tenajlo\<name>`) and click **Clone**.

You can also paste an address on the **URL** tab, for example
`https://git.example.com/team/project.git` or `git@git.example.com:team/project.git`.

### Open in Tenajlo from a Forgejo page

If your Forgejo administrator has added Tenajlo to the server, a repository's page has
**Open with Tenajlo** in its clone menu (next to **HTTPS** / **SSH**). Click it, and Tenajlo
starts (or comes to the front) with the Clone dialog open on the **URL** tab and the address
filled in. Your browser may ask whether to open Tenajlo first. Nothing is copied until you
check the **Local folder** and click **Clone**, so a link on a web page can't clone anything
on its own. Links with an address Tenajlo wouldn't accept on the **URL** tab are ignored.

For an HTTPS address on a server you haven't signed in to, Tenajlo asks for a username and
password (or access token) when it's needed. Tick **Remember this password** to keep it in
your computer's password store. Tenajlo only saves it once the server accepts it, and forgets
it if the server later rejects it.

To open a repository that's already on your computer, use **Current repository → Add local…**. To start a brand-new one, use **Current repository → New…**. Press **Ctrl+T** (⌘T on a Mac) to open the repository list quickly.

## Save your changes (commit)

1. Edit files as usual in your own tools.
2. In Tenajlo's **Changes** tab, tick the files you want to include. Click a file to see what
   changed.
3. Write a short **Summary** of what you changed (for example, "Adjust pump timer to 5 s").
4. Click **Commit to <branch>** (or press Ctrl+Enter).

To commit only part of a file, click the file, then use the boxes beside the changed lines
in the diff. Tick a line to include it, or tick the box on a block's header to include the
whole block. Shift+click includes or leaves out every line since the last one you clicked.
The rest stays in your files for a later commit. Renamed files, links, and files stored with
Git LFS can only be included whole.

Made a mistake? Right after committing, click **Undo** in the commit box. Your changes stay
there, ready to commit again.

Right-click a file for more: **Open in editor**, **Show in folder**, **Discard changes…**
(discarded new files go to the Recycle Bin, so you can get them back), and **Ignore**.
Right-click the "changed files" line above the list for **Discard all changes…**.

Your summary and description are kept if you switch to another repository and come back.

You can also use the keyboard: the arrow keys move through the list, Space ticks or unticks
the selected file, and the Menu key (or Shift+F10) opens its right-click menu.

### Images, CAD models and other binary files

Git can't show line-by-line changes for binary files. Instead, Tenajlo shows the file's size
before and after, and for images (PNG, JPEG, GIF, WebP and BMP up to 10 MB) the old and new
picture side by side. Files stored with Git LFS are marked **Stored with Git LFS**, with their
real size and, if the file is locked, who holds the lock.

## Share and get changes (push, pull, fetch)

The button in the toolbar always shows the next useful step:

- **Publish repository**: this repository isn't on a server yet (see below).
- **Publish branch**: your branch isn't on the server yet.
- **Send 2 commits ↑2**: you have 2 commits the server doesn't have (git calls this _push_).
- **Get 3 commits ↓3**: the server has 3 commits you don't have (_pull_).
- **Check for changes**: ask the server whether anyone has sent new commits (_fetch_). Your
  files don't change.

Point at the button to see exactly what it will do. If you and the server both have new
commits, it shows **Get** first: get theirs, then send yours.

Tenajlo also checks the server every few minutes in the background, and when you open a
repository or come back to the window (change or turn this off in **Settings → Repositories**).
While a check, push or pull is running, a commit or other change you start waits for it to
finish; the commit box says so.

In **History**, commits marked **Not pushed** are only on your computer so far. Right-click a
commit (or use the **Copy ID** link above its files) to copy its full commit ID.

To take a repository off Tenajlo's list, open the repository list, right-click it, and choose
**Remove from list…**. Its folder and files stay on your computer.

### Put a new repository on the server (publish)

A repository you created with **New…**, or added from your computer, starts out only on your
computer. To share it:

1. Make at least one commit.
2. Click **Publish repository** in the toolbar.
3. Choose the **Owner**: your own account or an organization you can create repositories in.
4. Check the **Name** (it starts as the folder name), add a description if you like, and
   choose whether to **Keep this repository private**.
5. Choose **HTTPS** or **SSH**, the same as when cloning, and click **Publish repository**.

Tenajlo creates the repository on Forgejo, then publishes your current branch to it. If the
upload stops partway, click **Publish branch** to try again; the repository on the server stays.

Creating repositories needs more permissions than signing in: `write:user` and
`write:repository` for your own account, and `write:organization` for organizations. If your
access token doesn't have them, Tenajlo says so and offers **Update the token**. Create a new
token with the permissions it lists and paste it.

### When you and someone else both changed the branch

Pull then says your branch and the server's have both changed, and offers **Merge the
server's changes**. That combines both sets of changes with a merge commit; then push.

### Conflicts

If you and a teammate changed the same lines, Tenajlo shows a **conflicts** banner listing
the files:

1. Click **Open** to edit a file. Look for the blocks between `<<<<<<<`, `=======` and
   `>>>>>>>`, keep the right content, and delete the marker lines.
2. Click **Mark resolved**.
3. When every file is resolved, click **Commit merge**, then push.

Changed your mind? **Abort merge** puts your branch back exactly as it was.

## Branches

Use the **Current branch** menu to switch branches or create a new one (Ctrl+Shift+N). If you
have unfinished changes when switching, Tenajlo asks whether to **Bring my changes** to the
other branch or **Leave my changes** on the branch you're leaving (you get them back when you
return).

### Pull requests and checks on Forgejo

When you're signed in to the repository's Forgejo server, Tenajlo links to it:

- If your branch has an open pull request, its number appears next to the branch name (for
  example **#42**). Open the **Current branch** menu and click it to see it on Forgejo.
- If your branch is on the server but has no pull request yet, the menu offers **Create pull
  request…**, which opens Forgejo's page for it.
- In **History**, a commit that's on the server shows its checks (such as Forgejo Actions
  builds): **Checks passed**, **Checks failed** or **Checks running**. Point at it to see each
  check. **View on Forgejo** (also in the right-click menu) opens the commit in your browser.

## Large files (Git LFS)

Repositories with CAD models, PLC archives and other large files often use Git LFS. Tenajlo
handles it automatically: large files are uploaded and downloaded with your other changes.

On Linux and macOS, install Git LFS first. If a repository needs it and it's missing, Tenajlo
shows a warning and won't commit or push, so large files aren't stored the wrong way.

### Lock files before you edit them

CAD models and PLC projects can't be merged: if two people change the same file, one of them
loses their work. In repositories that use Git LFS, Tenajlo shows a line at the top of the
sidebar, such as "2 files locked (1 by you)", when the server supports file locks.

- **Lock a file before editing it.** Click **Manage**, then **Lock a file…** and choose it. A
  file you've already changed can also be locked from its right-click menu (**Lock file**).
- Locked files in your changes show a padlock: grey for yours, red with the person's name for
  someone else's.
- If you include a file someone else has locked, the commit box warns you, and Tenajlo won't
  push it until they unlock it.
- When you're done, push your changes, then **Unlock** the file (in **Manage**, or **Unlock
  file** in the right-click menu).
- **Force unlock…** removes someone else's lock. Only use it when they've finished or left the
  lock by mistake.

The list refreshes when you open the repository and whenever Tenajlo checks the server.

## SSH keys

Your SSH key needs to be added to your Forgejo account. Tenajlo can do this for you in
**Settings → SSH keys**:

- **Create an SSH key…** makes a new key named `id_ed25519` in the `.ssh` folder in your home
  folder. It only appears if you don't have one yet; Tenajlo never replaces a key. A
  passphrase is optional but protects the key if someone copies it.
- **Add to <server>** puts a key on your Forgejo account. Keys already on the account say
  **Added to <server>**.
- If your access token can't add keys, Tenajlo says so and offers **Update the token**. Create a
  new token with the `write:user`, `read:repository` and `write:repository` permissions and
  paste it. You can also add keys yourself on the Forgejo website under
  **Settings → SSH / GPG Keys**.

Tenajlo uses your computer's own SSH (on Windows, the built-in OpenSSH and its ssh-agent).

- **Passphrase:** Tenajlo asks for it when needed. Tick **Remember this passphrase** to keep
  it in your computer's password store so Tenajlo doesn't ask again. If you change the
  passphrase later, Tenajlo notices the saved one no longer works and asks you again.
- **"Connect to <server>?"** appears the first time you connect to a server. Check the
  fingerprint with your IT team before choosing **Trust and connect**. If it doesn't match,
  cancel.
- **"The server's identity has changed"** means the server's key is different from last time.
  Don't continue; contact your IT team.

## Settings

- **Accounts:** sign in and out of Forgejo servers.
- **SSH keys:** create an SSH key and add it to your Forgejo account.
- **Passwords:** passwords and SSH passphrases you chose to remember. **Forget** removes one;
  Tenajlo asks for it next time.
- **Git:** your name and email for commits, and which Git program to use.
- **Repositories:** where new clones go, what Pull does, how often to check the server,
  whether to show a notification when the server has new commits on your branch (only while
  you're using another window), and which editor opens files.
- **Appearance:** light, dark, or the same as your computer.
- **About:** Tenajlo's version, its log files, and **Updates**. Tenajlo checks for a new
  version when it starts (turn this off with **Check for updates when Tenajlo starts**) and
  shows a banner when one is available. **Install and restart** downloads it, checks it's
  signed by Tenajlo's release key, installs it and restarts. On Linux, installing asks for
  your administrator password, because the `.deb` package is installed for the whole computer.

## Troubleshooting

| Message                                                                    | What to do                                                                                                                                 |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| "This computer doesn't trust the server's certificate"                     | Your company's certificate isn't installed on this computer. Ask IT to install it. On Windows, Tenajlo uses the Windows certificate store. |
| "Windows couldn't check whether the server's certificate has been revoked" | Connect to the company network or VPN and try again.                                                                                       |
| "Couldn't reach the server"                                                | Check your network or VPN connection and the server address.                                                                               |
| "The server doesn't allow pushing directly to this branch"                 | Create a new branch, push it, and open a pull request on Forgejo.                                                                          |
| "A file path is too long for Windows"                                      | Clone to a shorter folder, such as `C:\src`.                                                                                               |
| "Another program seems to be using this repository"                        | Close other Git tools. If none are running, delete the `.lock` file named in **Details**.                                                  |

Every error has a **Details** section with Git's exact output. Include it when asking for
help. Tenajlo removes passwords and tokens from it.
