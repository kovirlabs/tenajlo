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

## Share and get changes (push, pull, fetch)

The button in the toolbar always shows the next useful step:

- **Publish branch**: your branch isn't on the server yet.
- **Push origin ↑2**: you have 2 commits the server doesn't have.
- **Pull origin ↓3**: the server has 3 commits you don't have.
- **Fetch origin**: check the server for changes.

Tenajlo also checks the server every few minutes in the background (change or turn this off
in **Settings → Repositories**).

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

## Large files (Git LFS)

Repositories with CAD models, PLC archives and other large files often use Git LFS. Tenajlo
handles it automatically: large files are uploaded and downloaded with your other changes.

On Linux and macOS, install Git LFS first. If a repository needs it and it's missing, Tenajlo
shows a warning and won't commit or push, so large files aren't stored the wrong way.

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
- **Repositories:** where new clones go, what Pull does, how often to check the server, and
  which editor opens files.
- **Appearance:** light, dark, or the same as your computer.

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
