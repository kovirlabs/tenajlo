#!/bin/sh
# Creates the integration-test user in the dev Forgejo (dev/forgejo.yml). Safe to re-run.
set -eu
compose="docker compose -f $(dirname "$0")/forgejo.yml"
until $compose exec -T -u git forgejo forgejo admin user list >/dev/null 2>&1; do sleep 1; done
$compose exec -T -u git forgejo forgejo admin user create \
  --username tenajlo --password tenajlo-dev-password \
  --email tenajlo@example.invalid --must-change-password=false 2>/dev/null \
  || echo "user tenajlo already exists"
