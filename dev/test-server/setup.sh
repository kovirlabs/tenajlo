#!/usr/bin/env bash
# Starts the local test Forgejo (compose.yml) and, the first time, generates its secrets and
# creates two users, an access token and a sample repository. Everything you need to sign in
# is written to credentials.txt. Safe to re-run: existing secrets and users are kept.
# macOS / Linux twin of setup.ps1; keep them doing the same steps.
set -euo pipefail
cd "$(dirname "$0")"

IMAGE=codeberg.org/forgejo/forgejo:11
URL=http://localhost:3001
compose() { docker compose -f compose.yml "$@"; }
# Reads a fixed amount, so nothing in the pipeline is cut off early (which `pipefail` reports).
password() {
  local s
  s=$(head -c 512 /dev/urandom | LC_ALL=C tr -dc 'A-Za-z0-9')
  printf '%s' "${s:0:20}"
}

# 1. Server secrets, generated once with Forgejo's own generator.
if [ ! -f .env ]; then
  echo "Generating server secrets…"
  secret() { docker run --rm "$IMAGE" forgejo generate secret "$1"; }
  umask 077
  {
    echo "FORGEJO__security__SECRET_KEY=$(secret SECRET_KEY)"
    echo "FORGEJO__security__INTERNAL_TOKEN=$(secret INTERNAL_TOKEN)"
    echo "FORGEJO__oauth2__JWT_SECRET=$(secret JWT_SECRET)"
    echo "FORGEJO__server__LFS_JWT_SECRET=$(secret LFS_JWT_SECRET)"
  } >.env
fi

# 2. Start it and wait until it answers.
compose up -d
echo "Waiting for Forgejo…"
for _ in $(seq 1 60); do
  curl -fsS "$URL/api/healthz" >/dev/null 2>&1 && break
  sleep 2
done
curl -fsS "$URL/api/healthz" >/dev/null || { echo "Forgejo didn't start; see: docker compose -f dev/test-server/compose.yml logs" >&2; exit 1; }

if [ -f credentials.txt ]; then
  echo "Already set up."
  cat credentials.txt
  exit 0
fi

# 3. Users: "tester" (you, an admin) and "teammate" (to make changes you then pull).
forgejo() { compose exec -T -u git forgejo forgejo "$@"; }
tester_pw=$(password)
teammate_pw=$(password)
forgejo admin user create --username tester --email tester@example.invalid \
  --password "$tester_pw" --admin --must-change-password=false
forgejo admin user create --username teammate --email teammate@example.invalid \
  --password "$teammate_pw" --must-change-password=false

# 4. An access token with the permissions Tenajlo asks for, plus write:user for SSH keys.
token=$(forgejo admin user generate-access-token --username tester --token-name tenajlo \
  --scopes write:user,write:repository --raw | tr -d '\r\n')

# 5. A private sample repository that teammate can also push to.
curl -fsS -u "tester:$tester_pw" -H 'Content-Type: application/json' \
  -d '{"name":"sample","private":true,"auto_init":true,"description":"Try Tenajlo here"}' \
  "$URL/api/v1/user/repos" >/dev/null
curl -fsS -u "tester:$tester_pw" -H 'Content-Type: application/json' -X PUT \
  -d '{"permission":"write"}' \
  "$URL/api/v1/repos/tester/sample/collaborators/teammate" >/dev/null

umask 077
cat >credentials.txt <<EOF
Tenajlo local test server (dev/test-server). Keep this file private; it isn't committed.

Server address:   $URL
Access token:     $token
                  (Tenajlo: Settings → Accounts → sign in with the address and this token)

tester    (you, admin)   password: $tester_pw
teammate  (2nd person)   password: $teammate_pw

Sample repository, private, both users can push:
  HTTPS: $URL/tester/sample.git
  SSH:   ssh://git@localhost:2223/tester/sample.git
EOF
echo
cat credentials.txt
