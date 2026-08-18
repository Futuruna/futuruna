#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_dir="$(CDPATH= cd -- "$script_dir/../.." && pwd)"
output_dir="$repo_dir/website/target/dx/futuruna-web/release/web/public"
current_branch="$(git -C "$repo_dir" branch --show-current)"
deploy_branch="${1:-$current_branch}"

test -n "$deploy_branch" || { echo "deployment branch is required" >&2; exit 1; }
test -z "$(git -C "$repo_dir" status --porcelain)" || {
  echo "refusing to deploy a dirty worktree" >&2
  exit 1
}

if [[ "$deploy_branch" == "main" ]]; then
  test "$current_branch" = "main" || {
    echo "production deploys must run from the main branch" >&2
    exit 1
  }
  git -C "$repo_dir" fetch --quiet origin main
  test "$(git -C "$repo_dir" rev-parse HEAD)" = "$(git -C "$repo_dir" rev-parse origin/main)" || {
    echo "production deploys require local main to match origin/main" >&2
    exit 1
  }
fi

"$script_dir/build-ssg.sh"

commit_hash="$(git -C "$repo_dir" rev-parse HEAD)"
commit_message="$(git -C "$repo_dir" log -1 --pretty=%s)"

cd "$repo_dir"
npx --yes wrangler@4.123.0 pages deploy "$output_dir" \
  --project-name=futuruna \
  --branch="$deploy_branch" \
  --commit-hash="$commit_hash" \
  --commit-message="$commit_message" \
  --commit-dirty=false
