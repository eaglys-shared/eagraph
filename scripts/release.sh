#!/usr/bin/env bash
# Bump the workspace version, commit, tag, and push.
# Pushing the tag triggers .github/workflows/release.yml and attaches
# per-target binaries to the GitHub Release page for that tag.
#
# Only explicit versions are accepted. No patch/minor/major shortcuts.
# Picking the right version is a decision, not an increment — the
# contributor has to name it.
#
# Usage:
#   scripts/release.sh 0.1.1
#   scripts/release.sh 0.2.0
#   scripts/release.sh 1.0.0

set -euo pipefail

NEW="${1:-}"
if [[ -z "$NEW" ]]; then
    echo "usage: $0 X.Y.Z" >&2
    exit 1
fi

# Enforce X.Y.Z form. No pre-release / build metadata; keep it strict.
if [[ ! "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: version must be X.Y.Z with digits only (got '$NEW')" >&2
    exit 1
fi

# Preflight.
if [[ -n "$(git status --porcelain)" ]]; then
    echo "error: working tree has uncommitted changes" >&2
    exit 1
fi
if [[ "$(git rev-parse --abbrev-ref HEAD)" != "main" ]]; then
    echo "error: must be on main branch (currently on $(git rev-parse --abbrev-ref HEAD))" >&2
    exit 1
fi
git fetch --tags origin main

# Read current version from the workspace.
CURRENT=$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)
if [[ -z "$CURRENT" ]]; then
    echo "error: could not read version from Cargo.toml" >&2
    exit 1
fi

# Reject if NEW is not strictly greater than CURRENT. Compares numerically
# component-by-component so "0.10.0" > "0.9.9" (no lexicographic surprises).
IFS='.' read -r CMAJ CMIN CPAT <<< "$CURRENT"
IFS='.' read -r NMAJ NMIN NPAT <<< "$NEW"
if (( NMAJ < CMAJ )) \
   || (( NMAJ == CMAJ && NMIN < CMIN )) \
   || (( NMAJ == CMAJ && NMIN == CMIN && NPAT <= CPAT )); then
    echo "error: new version $NEW is not greater than current $CURRENT" >&2
    exit 1
fi

TAG="v$NEW"
if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "error: tag $TAG already exists" >&2
    exit 1
fi

echo "Bumping $CURRENT -> $NEW"

# Rewrite the workspace version. Portable between BSD and GNU sed by
# writing to a temp file instead of using the differing -i flag.
sed "s/^version = \"$CURRENT\"\$/version = \"$NEW\"/" Cargo.toml > Cargo.toml.tmp
mv Cargo.toml.tmp Cargo.toml

# Regenerate Cargo.lock and confirm the workspace still builds.
cargo check --workspace --quiet

git add Cargo.toml Cargo.lock
git commit -m "Release $TAG"
git tag -a "$TAG" -m "Release $TAG"

echo "Pushing commit and tag to origin..."
git push origin main
git push origin "$TAG"

echo
echo "Released $TAG"
echo "Watch the release build at: https://github.com/$(git config --get remote.origin.url | sed -E 's#.*[:/]([^/]+/[^/]+)\.git$#\1#')/actions"
