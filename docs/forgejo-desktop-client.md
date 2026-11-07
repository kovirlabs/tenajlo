# A desktop Git client for Forgejo

[Forgejo](https://forgejo.org) gives your team a fast, self-hosted home for its repositories.
Its web interface covers reviews, issues and settings. For the daily work of getting a copy of
a project, saving changes and sharing them, most people want a desktop app, especially
people who don't use the command line.

Any Git client can push to a Forgejo server if you paste the repository's address. The
difference is in everything around that: signing in, finding repositories, getting through
a company network, and handling the large binary files engineering teams work with. Most
desktop clients are built around a hosted service's accounts. Tenajlo is built around
Forgejo.

## What to look for

### Signing in to your own server

A good client lets you sign in to _your_ server, by its own address, once. Tenajlo asks for
the server address and a Forgejo access token, opens the token page for you, and keeps the
token in Windows Credential Manager, the macOS Keychain or the Linux Secret Service. From then
on, every clone, pull and push to that server just works. If the token expires, Tenajlo says
so and asks for a new one.

### Finding repositories without copying URLs

Tenajlo lists every repository you can reach on the server, including the ones you have
through organization teams. Pick one, choose HTTPS or SSH, and click **Clone**. No URLs, no
guessing which protocol IT allows.

### Working on a company network

Self-hosted servers usually sit behind a company certificate authority, a VPN and sometimes
an unusual SSH port. Tenajlo:

- trusts what your operating system trusts, including company certificates deployed by IT,
  and never turns certificate checks off;
- uses your computer's own OpenSSH and ssh-agent, and asks you in the app before trusting a
  new server's key;
- explains failures in plain language ("Connect to the company network or VPN and try
  again") instead of showing raw Git errors, with the exact output under **Details**.

### Large files and file locks

Engineering repositories hold CAD models, PLC archives, drawings and spreadsheets. These
belong in [Git LFS](https://git-lfs.com), and they can't be merged, so two people editing the
same file is a lost day of work. Tenajlo stores large files with Git LFS automatically, shows
who has locked which file, lets you lock a file before you edit it, and won't push a file
someone else has locked. It also shows image previews and real file sizes for binary files.
See [Version control for CAD files](git-for-cad.md).

### Plain language

Tenajlo's toolbar shows the next useful step in words: **Get 3 commits**, **Send 2
commits**, **Publish branch**. Conflicts come with a banner that lists the files and the
three steps to resolve them. The goal is that an engineer who has never heard of `rebase` can
work safely all day.

### Forgejo, not a generic remote

Because Tenajlo talks to the Forgejo API, it can do things a generic client can't:

- create a repository on the server from a folder on your computer (**Publish repository**),
  in your account or an organization;
- add your SSH key to your Forgejo account in one click;
- show your branch's open pull request and each commit's checks (such as Forgejo Actions
  builds), with links to view them on Forgejo.

### Nothing else

Tenajlo connects only to your Forgejo and Git servers, plus its own update check (which you
can turn off). There's no account to create, no analytics and no telemetry. It's free and
open source under the Apache License 2.0.

## Coming from GitHub Desktop

If your team used GitHub Desktop before moving to a self-hosted server, Tenajlo will feel
familiar: a changes list with checkboxes, a commit box, a history view, a branch menu and one
sync button. Tenajlo isn't affiliated with GitHub, and it adds what self-hosted engineering
teams need: sign-in to your own server, company-network support, and Git LFS file locks.

## Does it work with Gitea?

Forgejo started as a fork of Gitea, and the two share most of their API. Tenajlo is built and
tested against Forgejo, and Gitea servers may work too. Plain Git operations (clone, pull,
push, branches) work with any Git server, as long as you use the **URL** tab to clone.

## What Tenajlo doesn't do

Tenajlo keeps the everyday workflow simple and leaves the rest to Forgejo's web interface
and specialist tools:

- Reviews, issues and merging pull requests happen on the Forgejo website. Tenajlo links to
  them.
- There's no interactive rebase, cherry-pick or bisect.
- It signs in to one account per server.

## Try it

Tenajlo runs on Windows 10/11, Debian and Ubuntu, and macOS on Apple Silicon. The Windows
installer includes Git and Git LFS and doesn't need administrator rights.

- [Download Tenajlo](https://github.com/kovirlabs/tenajlo/releases/latest)
- [User guide](user-guide.md)
- [Set up Forgejo for an engineering team](forgejo-team-setup.md)
