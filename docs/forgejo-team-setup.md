# Setting up Forgejo for an engineering team that won't use the CLI

This guide is for the person who runs the Forgejo server: IT, a controls lead, or whoever
drew the short straw. It covers the server settings, organization layout, repository
defaults and onboarding steps that let mechanical, electrical and controls engineers use Git
every day through [Tenajlo](user-guide.md) without ever opening a terminal.

You'll need the command line and admin access to Forgejo for the setup. Your engineers won't.

The examples use `git.example.com` as the server name. Tenajlo is built and tested against
Forgejo 11.

## What you'll end up with

- Engineers sign in once with their usual company login and a Forgejo access token, then
  clone, commit, push and pull from Tenajlo.
- Every repository stores CAD models, PLC archives and drawings with Git LFS automatically,
  so nobody has to remember which files are "large".
- Files that can't be merged are locked before someone edits them, so two people never lose
  a day's work on the same assembly.
- Each department or project team sees only its own repositories.
- Important branches (for example, the PLC code that goes to the machines) can only change
  through a reviewed pull request.

## Before you start

**Use HTTPS with a certificate your computers already trust.** Tenajlo never turns off
certificate checks. If your company has its own certificate authority (CA), deploy its root
certificate to every Windows computer through Group Policy; Tenajlo uses the Windows
certificate store. On macOS and Linux, install the CA in the system trust store.

**Make revocation checks work.** On Windows, the certificate's revocation list (CRL) or OCSP
address must be reachable from engineers' computers, including over VPN. If it isn't,
Tenajlo shows "Windows couldn't check whether the server's certificate has been revoked" and
won't connect. Check the CRL Distribution Points in your server certificate and make sure
that address answers from a client network.

**Pick a stable server name.** Repositories remember the server address they were cloned
from. Choose a name you'll keep (for example `git.example.com`, not a machine name like
`srv-04`) and set it in DNS before anyone clones.

**Plan for storage.** CAD and PLC repositories grow quickly because every saved version of a
binary file is kept. Put Forgejo's LFS storage on a volume with room to grow, or on S3-compatible
object storage, and include it in your backups.

## Configure the server

These settings go in Forgejo's `app.ini` (or the matching `FORGEJO__section__KEY`
environment variables in Docker). Restart Forgejo after changing them.

```ini
[server]
; The address engineers type into Tenajlo. Must match what the reverse proxy serves,
; or LFS uploads go to the wrong place.
ROOT_URL = https://git.example.com/
; Git LFS: required for CAD models, PLC archives and other binary files.
LFS_START_SERVER = true
; Largest single LFS file in bytes. 0 means no limit; set one that fits your storage.
LFS_MAX_FILE_SIZE = 0

[repository]
DEFAULT_BRANCH = main

[service]
; Accounts come from your directory (see below), not self-registration.
DISABLE_REGISTRATION = true
; Hide the server from anyone who isn't signed in.
REQUIRE_SIGNIN_VIEW = true
```

### Reverse proxy limits

Most "push failed" reports from CAD teams come from the reverse proxy, not Forgejo. A large
assembly can be hundreds of megabytes, and the default upload limit of most proxies is a
few megabytes. For nginx:

```nginx
server {
    server_name git.example.com;
    # Large LFS uploads (CAD models, PLC archives). 0 = no limit.
    client_max_body_size 0;
    # Slow uploads over VPN shouldn't time out halfway.
    proxy_read_timeout 600s;
    proxy_send_timeout 600s;
    proxy_request_buffering off;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

Other proxies have the same kind of settings: a request body size limit and a timeout.

### Sign-in with company accounts

Add your directory as an authentication source so engineers sign in to Forgejo with the
username and password they already use: **Site Administration → Authentication Sources →
Add Authentication Source**, then choose **LDAP (via BindDN)** for Active Directory or
**OAuth2** for an OpenID Connect provider such as Entra ID or Keycloak.

Whichever you choose, Tenajlo itself signs in with a personal access token, not the
password, so it keeps working with single sign-on and multi-factor authentication.

### SSH (optional)

HTTPS with an access token is the simplest choice for engineers, and it's what Tenajlo
suggests by default. If you also offer SSH, publish the server's host key fingerprints
somewhere engineers can find them (an intranet page is fine). The first time someone
connects, Tenajlo shows the fingerprint and asks them to check it before trusting the
server. Print them on the server with:

```bash
for f in /etc/ssh/ssh_host_*_key.pub; do ssh-keygen -lf "$f"; done
```

If Forgejo runs its own SSH server or listens on a port other than 22, set `SSH_DOMAIN` and
`SSH_PORT` in `[server]` so the clone addresses Forgejo shows are correct.

## Organize people and repositories

Forgejo has two levels that matter here:

- **Organizations** own repositories. Create one per department or business unit, for
  example `controls`, `mechanical` and `tooling`.
- **Teams** inside an organization decide who can see and change which repositories.

A layout that works for most engineering groups:

| Team        | Permission                            | Who                                  |
| ----------- | ------------------------------------- | ------------------------------------ |
| `Owners`    | Owner (created with the organization) | You and one backup admin             |
| `engineers` | Write, on all repositories            | Everyone who changes designs or code |
| `reviewers` | Write, on all repositories            | Leads who approve pull requests      |
| `viewers`   | Read, on all repositories             | Production, quality, purchasing      |

Create teams under the organization's **Teams** tab. Make repositories **private**; with
`REQUIRE_SIGNIN_VIEW = true` nothing is public anyway. Tenajlo's clone list shows every
repository a person can reach through their teams, so engineers never need to know a URL.

Prefer a few large teams over one team per repository. When someone joins a project, adding
them to one team is all it takes.

## Set up repositories for binary files

The most important thing you can do for a non-CLI team is decide, once, which file types go
into Git LFS. Engineers then never have to think about it.

### Start from a template repository

1. Create a repository in the organization, for example `controls/template-plc`.
2. Add a `.gitattributes` and a `.gitignore` (examples below) and commit them.
3. In the repository's **Settings**, tick **Template repository**.
4. For each new project, use **New Repository**, choose the template, and tick **Git content**
   so the files are copied.

Engineers who create repositories themselves in Tenajlo (**Current repository → New…**)
won't get the template. For teams that need it, either create project repositories for them
or ask them to start from the template on the Forgejo website and then clone.

### `.gitattributes`

This file tells Git which files go to LFS and which can be locked. Adjust the list to the
tools your team uses:

```gitattributes
# Text files: normalise line endings, so Windows and Linux tools agree.
* text=auto

# CAD (SolidWorks, Inventor, STEP, Creo, AutoCAD …)
*.sldprt filter=lfs diff=lfs merge=lfs -text lockable
*.sldasm filter=lfs diff=lfs merge=lfs -text lockable
*.slddrw filter=lfs diff=lfs merge=lfs -text lockable
*.ipt    filter=lfs diff=lfs merge=lfs -text lockable
*.iam    filter=lfs diff=lfs merge=lfs -text lockable
*.idw    filter=lfs diff=lfs merge=lfs -text lockable
*.dwg    filter=lfs diff=lfs merge=lfs -text lockable
*.step   filter=lfs diff=lfs merge=lfs -text
*.stp    filter=lfs diff=lfs merge=lfs -text

# PLC and HMI projects (TIA Portal, Studio 5000, TwinCAT archives …)
*.zap*   filter=lfs diff=lfs merge=lfs -text lockable
*.acd    filter=lfs diff=lfs merge=lfs -text lockable
*.tnzip  filter=lfs diff=lfs merge=lfs -text lockable

# Documents and images
*.pdf    filter=lfs diff=lfs merge=lfs -text
*.xlsx   filter=lfs diff=lfs merge=lfs -text lockable
*.docx   filter=lfs diff=lfs merge=lfs -text lockable
*.png    filter=lfs diff=lfs merge=lfs -text
*.jpg    filter=lfs diff=lfs merge=lfs -text
```

Things to know:

- **Patterns match the extension's case exactly on Linux and macOS.** If your tools save
  `PART.SLDPRT`, add the upper-case pattern too, or use a pattern like `*.[sS][lL][dD][pP][rR][tT]`.
- **`lockable`** makes Git LFS check out these files read-only until the person locks them.
  That turns "lock before you edit" from a rule into something the CAD tool enforces: it
  opens the file read-only, which is the reminder to lock it in Tenajlo first. Leave
  `lockable` off for file types where that would just get in the way.
- **Set this up before the first commit of binary files.** Files committed before a pattern
  existed stay in regular Git. Converting history later (`git lfs migrate`) rewrites commits
  and means everyone has to clone again.

Text-based PLC exports (structured text, L5X, XML) are better left out of LFS: Git can show
and merge their changes line by line, and Tenajlo shows those diffs.

### `.gitignore`

Keep CAD lock files, backups and build output out of the repository:

```gitignore
# SolidWorks / Office lock and temporary files
~$*
*.bak
*.tmp

# AutoCAD
*.dwl
*.dwl2

# TwinCAT build output (add your PLC tool's build and cache folders)
_Boot/
_CompileInfo/

# Operating system clutter
Thumbs.db
.DS_Store
```

## File locks

Forgejo supports Git LFS file locks as soon as `LFS_START_SERVER = true`; there's nothing
else to enable. In Tenajlo, engineers see who has locked which file, lock files before
editing, and unlock them after pushing. Tenajlo refuses to push a file someone else has
locked.

As an organization owner you can see and remove locks on the Forgejo website under the
repository's **Settings → LFS → Locks**. Agree on a rule for stale locks, for example:
"if the person is away, ask their lead before using **Force unlock**". Anyone with write
access can force-unlock in Tenajlo, so the rule is social, not technical.

## Protect the branches that matter

For a design repository where everyone works on the same files, pushing straight to `main`
with locks is often enough. For PLC code that goes to running machines, require a review.

In the repository, open **Settings → Branches → Add new rule**:

- **Protected branch name pattern:** `main`
- **Push:** **Disable push**, so changes only arrive through pull requests.
- **Required approvals:** `1`, and limit approvals to the `reviewers` team.
- **Block merge on rejected reviews:** on.

When someone tries to push to a protected branch, Tenajlo tells them "The server doesn't
allow pushing directly to this branch" and to create a branch and a pull request. Engineers
create the branch in Tenajlo, push it, and click **Create pull request…** in the branch menu,
which opens Forgejo's page with the branch filled in. Reviewers approve and merge on the
Forgejo website.

If you use Forgejo Actions (for example, to check a PLC export compiles or to render
drawings to PDF), Tenajlo shows each commit's checks in **History**. You can also require
them under **Enable status check** in the same branch rule.

## Add an "Open in Tenajlo" button

Forgejo can show desktop apps in the clone menu on every repository page. Add Tenajlo there,
and engineers can open any repository in Tenajlo's Clone dialog with one click, with the
address already filled in. (Tenajlo 1.5 and later. The link only fills in the dialog; the
engineer still chooses the folder and clicks **Clone**.)

1. Open **Site Administration → Configuration → Settings → Repository**.
2. In the **Open with** editor apps box, enter:

   ```text
   Tenajlo = tenajlo://clone?url={url}
   VS Code = vscode://vscode.git/clone?url={url}
   VSCodium = vscodium://vscode.git/clone?url={url}
   ```

   Saving this list replaces Forgejo's built-in one, so keep any other apps your team uses.

3. Save. Repository pages now offer **Open with Tenajlo** next to the clone address, for
   HTTPS or SSH, whichever the engineer picked.

## Onboard engineers

### 1. Install Tenajlo

The Windows installer installs for the current user only and doesn't need administrator
rights. Git and Git LFS are included. Download it from the
[Releases page](https://github.com/kovirlabs/tenajlo/releases), or deploy it with your
software distribution tool. Tenajlo checks for updates itself; engineers can turn that off
in **Settings → About**.

### 2. Make sure each person has a Forgejo account

With an LDAP source, accounts are created the first time someone signs in to the Forgejo
website. Ask each new engineer to sign in once in a browser, then add them to the right
team.

### 3. Create an access token

Tenajlo walks them through this when they sign in: it opens the token page for them. The
token needs these permissions:

| Permission           | Needed for                                                   |
| -------------------- | ------------------------------------------------------------ |
| `read:user`          | Signing in                                                   |
| `read:repository`    | The clone list, pull requests and checks                     |
| `write:repository`   | Pushing, file locks                                          |
| `write:user`         | Optional: adding SSH keys and creating personal repositories |
| `write:organization` | Optional: publishing new repositories into an organization   |

If you set a maximum token lifetime, Tenajlo tells the engineer when their token stops
working and offers **Sign in again** with a new one.

### 4. Hand them the basics

Give new engineers the [Tenajlo user guide](user-guide.md) and three rules:

1. **Get changes before you start** (the toolbar button shows **Get … commits**).
2. **Lock CAD and PLC files before editing them**, and unlock after pushing.
3. **Commit small and often**, with a summary a colleague would understand.

## Checklist

- [ ] Server reachable over HTTPS at its permanent name, with a certificate from a CA every
      client trusts
- [ ] Certificate revocation address (CRL/OCSP) reachable from client networks and VPN
- [ ] `ROOT_URL` matches the address engineers use
- [ ] `LFS_START_SERVER = true`, LFS storage sized and backed up
- [ ] Reverse proxy allows large uploads and long timeouts
- [ ] Company sign-in configured; self-registration off
- [ ] Organizations and teams created; repositories private
- [ ] Template repository with `.gitattributes` and `.gitignore`
- [ ] Branch protection on production branches
- [ ] SSH host key fingerprints published (if you offer SSH)
- [ ] **Open with Tenajlo** added to the clone menu
- [ ] Tenajlo installed and a test engineer has cloned, locked, committed and pushed a CAD
      file end to end

## Troubleshooting

| What engineers see                                                             | What to check                                                                                           |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| Push of a large file fails partway, or the error mentions `413`                | The reverse proxy's upload limit (`client_max_body_size` in nginx) and timeouts.                        |
| "This computer doesn't trust the server's certificate"                         | The company CA isn't in that computer's trust store. Check Group Policy on Windows.                     |
| "Windows couldn't check whether the server's certificate has been revoked"     | The CRL or OCSP address in the certificate isn't reachable from that network.                           |
| Large files arrive as small text files starting with `version https://git-lfs` | LFS isn't enabled on the server, or the file was committed before its `.gitattributes` pattern existed. |
| LFS uploads fail but normal pushes work                                        | `ROOT_URL` doesn't match the address the proxy serves, or the proxy blocks `/info/lfs/` paths.          |
| An engineer can't see a repository in the clone list                           | They're not in a team with access to it, or they haven't been added to the organization.                |
| "The server doesn't allow pushing directly to this branch"                     | Working as intended: branch protection is on. They push a branch and open a pull request.               |
| Tenajlo says the token is missing a permission                                 | They need a new token with the permissions in [Create an access token](#3-create-an-access-token).      |
