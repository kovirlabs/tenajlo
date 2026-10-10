# Local test server

A Forgejo server that runs on your own computer in Docker, for trying Tenajlo by hand: signing
in, cloning over HTTPS and SSH, pushing, pulling a teammate's changes, conflicts, Git LFS, SSH
keys and notifications. It's the server to use for the manual checks in
[`docs/releasing.md`](../../docs/releasing.md).

It's separate from [`dev/forgejo.yml`](../forgejo.yml), the throwaway server the automated
integration tests use. This one keeps its data between restarts, generates its own secrets,
and listens on different ports, so both can run at once.

| What                 | Where                                            |
| -------------------- | ------------------------------------------------ |
| Web and HTTPS remote | `http://localhost:3001`                          |
| SSH remote           | `ssh://git@localhost:2223/<owner>/<repo>.git`    |
| Data                 | Docker volume `tenajlo-test-server_forgejo-data` |

It only listens on `localhost`, so nothing outside your computer can reach it. Tenajlo accepts
plain `http://` only for `localhost` addresses, which is why this works without a certificate.

## Set up

You need [Docker Desktop](https://www.docker.com/products/docker-desktop/) running.

**Windows** (PowerShell, from the repository folder):

```powershell
powershell -ExecutionPolicy Bypass -File dev\test-server\setup.ps1
```

**macOS / Linux:**

```bash
dev/test-server/setup.sh
```

The first run:

1. Generates the server's secret keys with Forgejo's own generator and saves them in
   `dev/test-server/.env`.
2. Starts Forgejo and waits until it answers.
3. Creates two users with random passwords: **tester** (you, an administrator) and
   **teammate** (a second person whose changes you can pull).
4. Creates an access token for tester with the permissions Tenajlo asks for, plus `write:user`
   so you can try adding SSH keys from Tenajlo.
5. Creates a private repository, `tester/sample`, that both users can push to.
6. Writes the address, token, passwords and clone URLs to `dev/test-server/credentials.txt`.

`.env` and `credentials.txt` are only on your computer (git ignores them). Running the script
again starts the server if it's stopped and prints `credentials.txt`; it never changes
existing secrets or users.

## Use it with Tenajlo

- **Sign in:** Settings → Accounts → **Sign in to another server…**, address
  `http://localhost:3001`, then paste the access token from `credentials.txt`.
- **Clone:** `tester/sample` appears in the clone list. Try both **HTTPS** and **SSH**. The first
  SSH connection asks you to trust `[localhost]:2223`.
- **SSH keys:** Settings → SSH keys → **Add to localhost:3001**.
- **Someone else's changes:** open `http://localhost:3001` in a private browser window, sign in as
  **teammate**, and edit a file in `tester/sample`. Then in Tenajlo, **Fetch** or wait for the
  background fetch (and its notification, if Tenajlo isn't the active window), and **Pull**.
- **Conflicts:** change the same line in Tenajlo and as teammate on the website, then pull.
- **Remembered passwords:** the password prompt only appears for servers you haven't signed in
  to. Sign out of the `localhost:3001` account, clone `http://localhost:3001/tester/sample.git`
  from the **URL** tab, enter tester's username and password when asked, and tick **Remember
  this password**. The next fetch shouldn't ask again.
- **Large files:** in the sample repository, run `git lfs track "*.bin"`, commit a large file and
  push.

## Stop, start, start over

```bash
docker compose -f dev/test-server/compose.yml stop      # stop; data is kept
docker compose -f dev/test-server/compose.yml start     # start again (or rerun setup)
docker compose -f dev/test-server/compose.yml down -v   # delete the server and all its data
```

After `down -v`, also delete `dev/test-server/.env` and `dev/test-server/credentials.txt`, then
run setup again for a fresh server with new secrets. Sign out of the account in Tenajlo first
(Settings → Accounts → **Sign out**) so the old token is removed from your password store.

## Troubleshooting

- **"port is already allocated":** something else uses port 3001 or 2223. Stop it, or change
  the left-hand port numbers in `compose.yml` and the matching `localhost:` addresses in the
  setup scripts.
- **SSH says the host key changed** after starting over: the new server has a new key. Remove
  the old one with `ssh-keygen -R "[localhost]:2223"`.
- **Server logs:** `docker compose -f dev/test-server/compose.yml logs forgejo`.
