# Version control for PLC projects with Git and Forgejo

"Which version is running on line 3?" is a question every controls team has answered by
searching a shared drive for `Line3_backup_2024-03-12_FINAL.zap17`. Git on a self-hosted
[Forgejo](https://forgejo.org) server answers it properly: every change has an author, a date
and a reason; changes can be reviewed before they reach a machine; and the version on each
machine is recorded.

This page covers how to put PLC and HMI projects in Git, what to commit for the common
platforms, and how controls engineers can do it all from a desktop app instead of the
command line.

## What Git gives a controls team

- **History with reasons.** Every commit says who changed what and why ("Increase conveyor
  jam timeout to 5 s after line 2 stoppages").
- **Review before commissioning.** A colleague can look at a change on Forgejo and approve it
  before it's downloaded to a running machine.
- **A record of what's on each machine.** Tag the commit you downloaded (for example
  `line3-2026-11-07`) on the Forgejo website, and you can always get that exact project back.
- **One copy, not twenty.** Everyone works from the same repository on the server instead of
  copies on laptops and USB sticks.

## Binary projects versus text exports

PLC tools store projects in different ways, and that decides how well Git can help:

- **Text files** (structured text, XML exports) can be compared line by line. Git shows
  exactly which rungs or lines changed, and two people's changes to different blocks can be
  merged.
- **Binary project files and archives** can't be compared or merged. Store them with
  [Git LFS](https://git-lfs.com) and lock them before editing, so two people never change the
  same project at once.

The best setup usually commits both: the native project or archive (so it opens in the
vendor's tool) and a text export (so changes can be reviewed).

### Siemens TIA Portal

Projects are folders of binary files. Commit a project archive (`.zap16`, `.zap17` …) to Git
LFS as a lockable file. In recent versions, TIA Portal's Version Control Interface can export
program blocks as text files (XML or SCL) to a folder: put that folder in the same repository
to make changes reviewable.

### Rockwell Studio 5000 Logix Designer

The `.ACD` project is binary: store it in Git LFS and lock it before editing. Also commit an
`.L5X` export of the project after each change. It's XML, so Forgejo and Tenajlo can show what
changed.

### Beckhoff TwinCAT 3

TwinCAT 3 projects are already made of text (XML) files, one per POU, and work well with Git
directly. Ignore the build output folders (`_Boot/`, `_CompileInfo/`) in `.gitignore`.

### CODESYS

The `.project` file is binary: store it in Git LFS as lockable, and commit a PLCopen XML
export alongside it for review. CODESYS also sells a Git add-on that stores the project as
text.

### HMI and drives

HMI projects, drive parameter files and network configurations are usually binary too. The
same rule applies: Git LFS, lockable, and an export or PDF printout for review if the tool
offers one.

## The everyday workflow in Tenajlo

[Tenajlo](forgejo-desktop-client.md) is a free desktop Git app for Forgejo, made for engineers
who don't use the command line:

1. **Get the latest version** with the toolbar's **Get commits** button.
2. **Lock the project file** in **Manage → Lock a file…** before opening it in the PLC tool.
3. **Make your change** and export the text version if your platform uses one.
4. **Commit** the project and the export together with a summary of the change. Tenajlo shows
   the changed lines for text exports and the size change for binary files.
5. **Send your commits and unlock** the project.

For changes that go to a running machine, protect the main branch on Forgejo so changes arrive
through a pull request. In Tenajlo, create a branch, commit and send it, then click **Create
pull request…** in the branch menu. A colleague reviews and approves it on Forgejo.

## Setting it up

The Forgejo administrator sets this up once:
[Setting up Forgejo for an engineering team](forgejo-team-setup.md) covers the server, Git LFS,
teams, branch protection, and a template repository with a `.gitattributes` for PLC files,
for example:

```gitattributes
*.zap*  filter=lfs diff=lfs merge=lfs -text lockable
*.acd   filter=lfs diff=lfs merge=lfs -text lockable
*.ACD   filter=lfs diff=lfs merge=lfs -text lockable
*.project filter=lfs diff=lfs merge=lfs -text lockable
*.tnzip filter=lfs diff=lfs merge=lfs -text lockable
```

`*.project` matches CODESYS projects; leave it out if the repository also holds Eclipse
`.project` files, which are text.

Leave text exports (`.L5X`, `.xml`, `.scl`, `.st`, TwinCAT's `.TcPOU`) out of LFS so their
changes can be compared.

## Get started

- [Download Tenajlo](https://github.com/kovirlabs/tenajlo/releases/latest) for Windows,
  Linux or macOS
- [Set up Forgejo for your team](forgejo-team-setup.md)
- [Tenajlo user guide](user-guide.md)
