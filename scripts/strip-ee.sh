#!/usr/bin/env bash
# strip-ee.sh — remove BSL 1.1 code before syncing to the public mirror.
#
# Usage:
#   bash scripts/strip-ee.sh [--dry-run]
#
# What it does:
#   1. Deletes every directory named "ee/" anywhere in the tree.
#   2. Removes the Next.js (ee) route group (apps/web/app/(ee)/).
#   3. Removes bin/kaya-cloud/ (BSL binary).
#   4. Removes the BSL license file.
#   5. Strips BSL workspace members from apps/backend/Cargo.toml.
#   6. Verifies that no remaining Cargo.toml references a deleted crate.
#
# Run this script in a clean checkout of the tag you are releasing.
# Do NOT run it in your working tree.
#
# NOTE: This script is intentionally left unrun during development.
# The release workflow (.github/workflows/release.yml) calls it on tag push.

set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=true
  echo "[dry-run] No files will be deleted."
fi

delete() {
  local target="$1"
  if [[ -e "$target" ]]; then
    echo "Deleting: $target"
    if [[ "$DRY_RUN" == "false" ]]; then
      rm -rf "$target"
    fi
  fi
}

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
echo "Repository root: $REPO_ROOT"

# 1. Remove every ee/ directory (BSL crates, BSL routes, BSL components, etc.)
while IFS= read -r -d '' ee_dir; do
  delete "$ee_dir"
done < <(find "$REPO_ROOT" -type d -name "ee" -print0)

# 2. Remove the Next.js (ee) route group — parentheses prevent -name "ee" from matching
delete "$REPO_ROOT/apps/web/app/(ee)"

# 3. Remove the cloud binary (BSL)
delete "$REPO_ROOT/apps/backend/bin/kaya-cloud"

# 4. Remove BSL license
delete "$REPO_ROOT/LICENSE-BSL"

# 5. Strip BSL workspace members from the root Cargo.toml
WORKSPACE_TOML="$REPO_ROOT/apps/backend/Cargo.toml"
if [[ -f "$WORKSPACE_TOML" ]]; then
  echo "Removing BSL members from $WORKSPACE_TOML"
  if [[ "$DRY_RUN" == "false" ]]; then
    sed -i.bak \
      -e '/[[:space:]]*"crates\/ee\//d' \
      -e '/[[:space:]]*"bin\/kaya-cloud"/d' \
      "$WORKSPACE_TOML"
    rm -f "$WORKSPACE_TOML.bak"
  fi
fi

# 6. Sanity check: no remaining Cargo.toml should reference a deleted crate
echo "Checking for dangling BSL references in Cargo.toml files..."
if grep -r "kaya-billing\|kaya-tenant\|kaya-metering\|kaya-postgres-storage\|kaya-cloud" \
     "$REPO_ROOT/apps/backend" --include="Cargo.toml" \
     --exclude-dir=target 2>/dev/null; then
  if [[ "$DRY_RUN" == "true" ]]; then
    echo "[dry-run] WARNING: BSL references remain (expected — sed did not run)."
  else
    echo "ERROR: Remaining Cargo.toml files still reference BSL crates." >&2
    exit 1
  fi
fi

echo "Strip complete. Verify with: cargo build --workspace (from apps/backend/)"
