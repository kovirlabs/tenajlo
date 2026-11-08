#!/bin/sh
# Creates the integration-test users in the dev server. Safe to re-run.
#   dev/seed-forgejo.sh          # Forgejo (dev/forgejo.yml)
#   dev/seed-forgejo.sh gitea    # Gitea (dev/gitea.yml)
set -eu
server="${1:-forgejo}"
case "$server" in
  forgejo | gitea) ;;
  *) echo "usage: $0 [forgejo|gitea]" >&2; exit 2 ;;
esac
compose="docker compose -f $(dirname "$0")/$server.yml"
admin="$compose exec -T -u git $server $server admin user"
until $admin list >/dev/null 2>&1; do sleep 1; done
$admin create \
  --username tenajlo --password tenajlo-dev-password \
  --email tenajlo@example.invalid --must-change-password=false 2>/dev/null \
  || echo "user tenajlo already exists"
# A second user for tests where someone else holds a lock (LFS locks, v1.4).
$admin create \
  --username alice --password alice-dev-password \
  --email alice@example.invalid --must-change-password=false 2>/dev/null \
  || echo "user alice already exists"
