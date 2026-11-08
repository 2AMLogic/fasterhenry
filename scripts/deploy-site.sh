#!/usr/bin/env bash
#
# deploy-site.sh — build the fasterhenry.org site and deploy it to Cloudflare
# Pages. Mirrors klayout-tools/scripts/deploy-site.sh.
#
# Pipeline:
#   1. build  -- `python3 site/build.py`, producing site/dist/ (stdlib only;
#                no Node, no network).
#   2. deploy -- `wrangler pages deploy site/dist` (skipped under --no-deploy).
#
# Auth is a scoped API token in the environment, NOT wrangler OAuth:
#   set -a; source <your scoped token env file>; set +a
# The file must export CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID. Point
# FASTERHENRY_CF_ENV_FILE at it and this script sources it for you. No
# credential value is ever printed or stored in this repository.
#
# Usage:
#   scripts/deploy-site.sh [--no-deploy] [--help]
#
#   --no-deploy   Build only (site/dist/); skip wrangler. Needs no credentials.
#   --help        Show this help and exit.
#
# Environment:
#   FASTERHENRY_CF_ENV_FILE   optional env file to source for the token
#   CF_PAGES_PROJECT          Pages project name (default: fasterhenry)
#
# Out of scope: regenerating the hero figure (site/gen_hero.py needs the Rust
# CLI); this script builds whatever is committed under site/.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

info() { printf '[deploy] %s\n' "$*"; }
err()  { printf '[deploy] ERROR: %s\n' "$*" >&2; }

usage() {
  sed -n '2,/^set -euo pipefail/p' "${BASH_SOURCE[0]}" \
    | sed -e 's/^# \{0,1\}//' -e '/^set -euo pipefail/d'
}

NO_DEPLOY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --no-deploy) NO_DEPLOY=1 ;;
    -h|--help)   usage; exit 0 ;;
    *) err "Unknown argument: $1"; echo "Run '$0 --help' for usage." >&2; exit 2 ;;
  esac
  shift
done

cd "${REPO_ROOT}"

command -v python3 >/dev/null 2>&1 || { err "'python3' not found — required to build the site."; exit 1; }

info "Step 1/2: building the site (python3 site/build.py)"
python3 site/build.py
info "Site built to site/dist/."

if [ "${NO_DEPLOY}" -eq 1 ]; then
  info "Step 2/2: --no-deploy set; skipping Cloudflare Pages deploy."
  info "Done. Build is in site/dist/ (not deployed)."
  exit 0
fi

if [ -n "${FASTERHENRY_CF_ENV_FILE:-}" ]; then
  [ -r "${FASTERHENRY_CF_ENV_FILE}" ] || { err "FASTERHENRY_CF_ENV_FILE is not readable."; exit 1; }
  set -a; . "${FASTERHENRY_CF_ENV_FILE}"; set +a
fi
: "${CLOUDFLARE_API_TOKEN:?set CLOUDFLARE_API_TOKEN (or FASTERHENRY_CF_ENV_FILE)}"
: "${CLOUDFLARE_ACCOUNT_ID:?set CLOUDFLARE_ACCOUNT_ID (or FASTERHENRY_CF_ENV_FILE)}"
export CLOUDFLARE_API_TOKEN CLOUDFLARE_ACCOUNT_ID

info "Step 2/2: deploying site/dist/ to Cloudflare Pages (branch main)."
exec npx --yes wrangler pages deploy site/dist --project-name "${CF_PAGES_PROJECT:-fasterhenry}" --branch main --commit-dirty=true
