#!/usr/bin/env bash
# Runs every local gate and keeps its full output under target/qa/evidence/<stamp>/, beside the
# sha256 of every source file, so a review can tie each result to one exact tree.
#   scripts/evidence.sh            prints the evidence directory last
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

dir="target/qa/evidence/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$dir"

# Tracked and untracked files that exist; ignored ones (target/, node_modules/) are not source.
# Vite bundles its config into a short-lived `*.timestamp-*.mjs` beside it; that is not source.
sources() {
  # Deleted files are still listed until the deletion is committed; `|| true` skips them.
  # The single-quoted script is sh's own; $f expands there, not here.
  # shellcheck disable=SC2016
  git ls-files -co --exclude-standard -z |
    xargs -0 sh -c 'for f; do case "$f" in *.timestamp-*.mjs) ;; *) [ -f "$f" ] && sha256sum "$f" || true;; esac; done' _ |
    sort -k2
}
sources > "$dir/sources.sha256"
sha256sum "$dir/sources.sha256" | cut -c1-64 > "$dir/tree"
echo "tree $(cat "$dir/tree") ($(wc -l < "$dir/sources.sha256") files)"

failed=0
run() {
  local name=$1
  shift
  { echo "\$ $*"; "$@"; echo "exit=$?"; } > "$dir/$name.log" 2>&1
  local result
  result=$(tail -1 "$dir/$name.log")
  echo "$name $result"
  [[ $result == exit=0 ]] || failed=1
}
run cargo-fmt cargo fmt --all --check
run cargo-clippy cargo clippy --workspace --all-targets -- -D warnings
run cargo-test cargo test --workspace
# On their own, as CI's rust-core job builds them: a feature only the app turns on must not leak.
run cargo-test-core cargo test -p lcu -p winer-core
run cargo-xwin-check cargo xwin check --target x86_64-pc-windows-msvc --workspace --all-targets
run pnpm-format pnpm format:check
run pnpm-lint pnpm lint
run pnpm-typecheck pnpm typecheck
run pnpm-test pnpm test
# The tree must not have moved while the gates ran.
if ! sources | diff "$dir/sources.sha256" - > "$dir/tree-drift.diff"; then
  echo "the tree changed during the run: see $dir/tree-drift.diff"
  failed=1
else
  rm -f "$dir/tree-drift.diff"
fi
echo "$dir"
exit "$failed"
