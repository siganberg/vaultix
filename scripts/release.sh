#!/usr/bin/env bash
# Cut a release: bump every version, commit, tag (the tag message = release notes), push.
# CI (.github/workflows/release.yml) then builds the Docker image and all installers and
# publishes the GitHub Release with these notes.
#
#   scripts/release.sh 0.4.0 notes.md          # new release; notes.md is Markdown
#   scripts/release.sh --notes v0.4.0 notes.md # replace the notes of an existing release
set -euo pipefail
cd "$(dirname "$0")/.."

die() { echo "error: $*" >&2; exit 1; }

if [ "${1:-}" = "--notes" ]; then
  [ $# -eq 3 ] || die "usage: $0 --notes vX.Y.Z notes.md"
  [ -s "$3" ] || die "notes file '$3' is empty or missing"
  command -v gh >/dev/null || die "the GitHub CLI (gh) is required"
  gh release edit "$2" --notes-file "$3"
  echo "Updated notes for $2"
  exit 0
fi

[ $# -eq 2 ] || die "usage: $0 X.Y.Z notes.md   |   $0 --notes vX.Y.Z notes.md"
VERSION="${1#v}"
NOTES="$2"
TAG="v$VERSION"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || die "version must look like 1.2.3"
[ -s "$NOTES" ] || die "notes file '$NOTES' is empty or missing"
NOTES_ABS="$(cd "$(dirname "$NOTES")" && pwd)/$(basename "$NOTES")"

[ "$(git rev-parse --abbrev-ref HEAD)" = "main" ] || die "releases are cut from main"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "commit or stash your changes first"
git fetch -q origin main --tags
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "main is not in sync with origin/main (pull/push first)"
git rev-parse -q --verify "refs/tags/$TAG" >/dev/null && die "tag $TAG already exists"

echo "Releasing $TAG"

# One version for everything: server, sync engine, desktop app, macOS app.
for f in server/Cargo.toml client/Cargo.toml desktop/Cargo.toml server-app/Cargo.toml; do
  perl -0pi -e "s/^version = \"[^\"]+\"/version = \"$VERSION\"/m" "$f"
done
perl -pi -e "s/\"version\": \"[^\"]+\"/\"version\": \"$VERSION\"/" desktop/tauri.conf.json server-app/tauri.conf.json

# Refresh the lockfiles' own entries (no dependency upgrades).
cargo update -q --workspace --offline 2>/dev/null || cargo update -q --workspace
(cd desktop && { cargo update -q -p vaultix-desktop -p vaultix-sync --offline 2>/dev/null || cargo update -q -p vaultix-desktop -p vaultix-sync; })
(cd server-app && { cargo update -q -p vaultix-server-app -p vaultix-server --offline 2>/dev/null || cargo update -q -p vaultix-server-app -p vaultix-server; })

git add server/Cargo.toml client/Cargo.toml desktop/Cargo.toml desktop/tauri.conf.json server-app/Cargo.toml server-app/tauri.conf.json Cargo.lock desktop/Cargo.lock server-app/Cargo.lock
git commit -q -m "Release $TAG"
git tag -a "$TAG" --cleanup=verbatim -F "$NOTES_ABS"
git push -q origin main
git push -q origin "$TAG"

echo "Pushed $TAG. CI is building the release:"
echo "  $(gh repo view --json url -q .url 2>/dev/null || echo 'https://github.com/<you>/vaultix')/actions"
