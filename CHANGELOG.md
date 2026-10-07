# Changelog

All notable changes to Tenajlo are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

First public release (1.0) in preparation:

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
