# Security policy

Tenajlo handles Git credentials, Forgejo access tokens and SSH prompts, so security reports
are very welcome.

## Reporting a vulnerability

Please **don't open a public issue**. Report it privately through GitHub:
<https://github.com/kovirlabs/tenajlo/security/advisories/new>

Include what you found, how to reproduce it, and which version and operating system you used.
You'll get a reply within a week. Once a fix is released, we'll credit you in the release
notes unless you'd rather not be named.

## Supported versions

Security fixes go into the latest release.

## What Tenajlo promises

These are the rules the code is built and tested around (see `spec.md` §10). A way to break
any of them is a vulnerability:

- Access tokens and passwords stay in the operating system's password store and in memory
  while in use. They never reach the app's web view, its log files or any other file.
- Log files and error details remove credentials from URLs and `Authorization` headers.
- Git is always started with an argument list, never through a shell, and user input can't
  become a Git or SSH option.
- The credential helper answers only for the server and user an account belongs to, and only
  for a Git process Tenajlo started (per-operation random token on a loopback-only listener).
- TLS certificate checks and SSH host-key checks are never weakened or skipped. Unknown SSH
  host keys need the user's explicit confirmation.
- Tenajlo never writes to your global Git configuration except your name and email, and
  only after you confirm.
- No telemetry: Tenajlo connects only to the Git and Forgejo servers you use.
