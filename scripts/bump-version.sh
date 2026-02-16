#!/bin/bash
# Usage: ./scripts/bump-version.sh 0.3.0
# Updates version in all package files from a single source.

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.3.0"
  exit 1
fi

NEW_VERSION="$1"

# Validate semver format
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
  echo "Error: invalid semver format: $NEW_VERSION"
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"

# 1. Cargo.toml (source of truth)
sed -i '' "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" "$ROOT/Cargo.toml"
echo "  Cargo.toml → ${NEW_VERSION}"

# 2. npm/mdxport/package.json (CI also overwrites this, but keep in sync for local dev)
node -e "
  const fs = require('fs');
  const path = '$ROOT/npm/mdxport/package.json';
  const pkg = JSON.parse(fs.readFileSync(path, 'utf8'));
  pkg.version = '${NEW_VERSION}';
  fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
"
echo "  npm/mdxport/package.json → ${NEW_VERSION}"

# 3. Update Cargo.lock
cd "$ROOT"
cargo check --quiet 2>/dev/null || true
echo "  Cargo.lock updated"

echo ""
echo "Version bumped to ${NEW_VERSION}"
echo ""
echo "Next steps:"
echo "  git add -A && git commit -m \"chore: bump version to ${NEW_VERSION}\""
echo "  git tag v${NEW_VERSION}"
echo "  git push origin main --tags"
