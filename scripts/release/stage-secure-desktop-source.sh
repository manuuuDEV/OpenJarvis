#!/usr/bin/env bash
# Stage the reviewed source tree that the cloud-only desktop bundle uses at runtime.
# The resulting directory intentionally has no .git history, node_modules, or build target.
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
destination="$repo_root/frontend/src-tauri/resources/openjarvis-source"

rm -rf "$destination"
mkdir -p "$destination"

# Archive exactly the commit being built. The desktop app must never clone a
# mutable remote repository on first launch.
git -C "$repo_root" archive --format=tar HEAD | tar -xf - -C "$destination"

# The desktop package contains its own rendered frontend. Runtime needs the
# Python backend source and Rust extension source, not development artefacts.
rm -rf \
  "$destination/.github" \
  "$destination/docs" \
  "$destination/examples" \
  "$destination/tests" \
  "$destination/frontend/node_modules" \
  "$destination/frontend/src-tauri/target"

test -f "$destination/pyproject.toml"
test -f "$destination/uv.lock"
test -d "$destination/src/openjarvis"
test -d "$destination/rust"

printf 'Bundled secure source staged at %s\n' "$destination"
