#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
website_dir="$(CDPATH= cd -- "$script_dir/.." && pwd)"
output_dir="$website_dir/target/dx/futuruna-web/release/web/public"

case "$output_dir" in
  "$website_dir"/target/dx/futuruna-web/release/web/public) ;;
  *) echo "refusing to clear unexpected output directory: $output_dir" >&2; exit 1 ;;
esac

rm -rf -- "$output_dir"

cd "$website_dir"
dx build --web --ssg --release --locked --force-sequential --debug-symbols false

routes=(
  "why"
  "playground"
  "docs"
  "docs/basics"
  "docs/stdlib"
  "docs/streams"
  "docs/rust"
  "docs/tutorial"
  "research"
  "research/philosophy"
  "research/danish-constitution"
  "research/danish-constitution-audit"
  "research/personskatteloven"
  "research/income-cliffs"
  "research/us-constitution"
  "research/ownership"
)

for route in "${routes[@]}"; do
  source_file="$output_dir/$route/index.html"
  destination_file="$output_dir/$route.html"
  test -f "$source_file" || { echo "missing Dioxus SSG output: $source_file" >&2; exit 1; }
  mv -- "$source_file" "$destination_file"
done

"$script_dir/check-ssg.sh" "$output_dir"
echo "$output_dir"
