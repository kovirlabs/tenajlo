# Version control for CAD files with Git and Forgejo

Many engineering teams keep CAD files on a shared drive. It works until two people open the
same assembly, someone saves over a colleague's changes, or nobody can say which
`bracket_final_v3.sldprt` went to the supplier. A full product data management (PDM) system
fixes that, but it's expensive and heavy for a small team.

Git with Git LFS on a self-hosted [Forgejo](https://forgejo.org) server sits in between: every
saved version is kept with who changed it and why, files are locked while someone edits them,
and it costs nothing but a server. This page explains how it works and how to set it up so
designers never need the command line.

## Why CAD files need special handling in Git

Git was designed for source code: text files that it can compare line by line and merge
when two people change different parts. CAD files are different:

- **They're binary.** Git can't show what changed inside a part file, only that it changed.
- **They're large.** An assembly can be hundreds of megabytes, and Git keeps every version.
  Stored directly in Git, a repository quickly becomes too slow to clone.
- **They can't be merged.** If two people change the same part, one of them has to redo
  their work.

Two Git features solve this.

### Git LFS keeps large files out of the way

[Git LFS](https://git-lfs.com) (Large File Storage) stores the actual CAD files on the
Forgejo server and keeps small pointers in the repository. Cloning stays fast, and you only
download the versions you need. Which files go to LFS is decided once, per repository, by a
list of extensions in a `.gitattributes` file. After that it's automatic.

### File locks prevent lost work

Git LFS file locking lets a designer lock a file on the server before editing it. Everyone
else sees who holds the lock, and their changes to that file are refused until it's
unlocked. With the `lockable` setting, Git even keeps the file read-only on everyone else's
computer, so the CAD tool opens it read-only: a built-in reminder to lock it first.

## The everyday workflow in Tenajlo

[Tenajlo](forgejo-desktop-client.md) is a free desktop app for Forgejo that handles Git LFS
and file locks without the command line:

1. **Get the latest changes.** The toolbar shows **Get 3 commits** when colleagues have
   shared new work.
2. **Lock the files you'll edit.** In the sidebar, click **Manage → Lock a file…**. Files
   locked by others show a red padlock with their name.
3. **Work in your CAD tool as usual.** Save as often as you like.
4. **Commit.** In **Changes**, tick the files, write a short summary ("Widen mounting slot to
   8 mm"), and click **Commit**. Tenajlo shows each file's size before and after, and image
   previews for PNG and JPEG renders.
5. **Share and unlock.** Click **Send commits**, then unlock the files so others can edit
   them.

If you include a file someone else has locked, Tenajlo warns you in the commit box and
won't push it until they unlock it.

## Setting it up

The Forgejo administrator does this once. The full walkthrough is in
[Setting up Forgejo for an engineering team](forgejo-team-setup.md); the CAD-specific parts
are:

- **Turn on Git LFS** on the server (`LFS_START_SERVER = true`) and raise the reverse proxy's
  upload limit, so large assemblies don't fail halfway.
- **Add a `.gitattributes`** listing your CAD extensions before anyone commits CAD files, for
  example:

  ```gitattributes
  *.sldprt filter=lfs diff=lfs merge=lfs -text lockable
  *.sldasm filter=lfs diff=lfs merge=lfs -text lockable
  *.slddrw filter=lfs diff=lfs merge=lfs -text lockable
  *.ipt    filter=lfs diff=lfs merge=lfs -text lockable
  *.iam    filter=lfs diff=lfs merge=lfs -text lockable
  *.dwg    filter=lfs diff=lfs merge=lfs -text lockable
  *.step   filter=lfs diff=lfs merge=lfs -text
  ```

- **Ignore temporary files** such as `~$*` lock files and `*.bak` backups in `.gitignore`.
- **Use a template repository** so every new project starts with these files.

## Good habits

- **Keep an assembly and its parts in the same repository.** CAD tools find referenced parts
  by file name and folder, so moving them between repositories breaks references.
- **Rename and move files inside the CAD tool,** so it updates references, then commit the
  result as one change.
- **Commit exported formats you send out** (STEP, PDF drawings) alongside the native files,
  so you can always see exactly what went to a supplier.
- **Commit small and often,** with a summary a colleague would understand in a year.
- **Plan for storage.** Every version of every file is kept on the server. Size the LFS
  storage and include it in backups.

## When you need a PDM system instead

Git doesn't understand what's inside your models. It won't manage part numbers, bills of
materials, where-used searches, release workflows or approvals. If you need those, use a PDM
or PLM system. If what you need is history, locking and a single source of truth for a
team, Git and Forgejo cover it at a fraction of the cost.

## Get started

- [Download Tenajlo](https://github.com/kovirlabs/tenajlo/releases/latest) for Windows,
  Linux or macOS
- [Set up Forgejo for your team](forgejo-team-setup.md)
- [Tenajlo user guide](user-guide.md#large-files-git-lfs)
