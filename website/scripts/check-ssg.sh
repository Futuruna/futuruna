#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
website_dir="$(CDPATH= cd -- "$script_dir/.." && pwd)"
output_dir="${1:-$website_dir/target/dx/futuruna-web/release/web/public}"

count_pattern() {
  local pattern="$1"
  local file="$2"
  { grep -Eo "$pattern" "$file" || true; } | wc -l | tr -d '[:space:]'
}

pages=(
  "index.html^https://futuruna.com/^Futuruna - Law Programming^A programming language for law."
  "why.html^https://futuruna.com/why^Why Futuruna — A Programming Language for Law^<h1>Why Futuruna</h1>"
  "playground.html^https://futuruna.com/playground^Playground — Futuruna Programming Language^>Futuruna Playground</h1>"
  "docs.html^https://futuruna.com/docs^Seven Runes — Futuruna Language Reference^<h1>The Seven Runes</h1>"
  "docs/basics.html^https://futuruna.com/docs/basics^Language Basics — Futuruna Documentation^<h1>Futuruna Basics</h1>"
  "docs/stdlib.html^https://futuruna.com/docs/stdlib^Standard Library — Futuruna Documentation^<h1>Standard Library</h1>"
  "docs/streams.html^https://futuruna.com/docs/streams^Reactive Streams — Futuruna Documentation^<h1>Reactive Streams</h1>"
  "docs/rust.html^https://futuruna.com/docs/rust^Rust Compatibility — Futuruna Documentation^<h1>Rust Compatibility</h1>"
  "docs/tutorial.html^https://futuruna.com/docs/tutorial^Tutorial — Futuruna Programming Language^<h1>Build a Rule-Driven Tax Program</h1>"
  "research.html^https://futuruna.com/research^Research — Futuruna Programming Language^<h1 class=\"research-title\">Research</h1>"
  "research/philosophy.html^https://futuruna.com/research/philosophy^Philosophy of Futuruna — Partitioned Syntax^<h1>Philosophy of Futuruna</h1>"
  "research/danish-constitution.html^https://futuruna.com/research/danish-constitution^Danmarks Riges Grundlov | Futuruna^<h1>Danmarks Riges Grundlov</h1>"
  "research/danish-constitution-audit.html^https://futuruna.com/research/danish-constitution-audit^Prøvning af grundlovsmodellen | Futuruna^<h1>Prøvning af grundlovsmodellen</h1>"
  "research/personskatteloven.html^https://futuruna.com/research/personskatteloven^Personskatteloven - Futuruna-forskning^<h1>Personskatteloven i Futuruna</h1>"
  "research/income-cliffs.html^https://futuruna.com/research/income-cliffs^Can earning one more krone leave you with less? — Futuruna^<h1>Can earning one more krone leave you with less?</h1>"
  "research/us-constitution.html^https://futuruna.com/research/us-constitution^US Constitution in Futuruna — Research^<h1>The United States Constitution</h1>"
  "research/ownership.html^https://futuruna.com/research/ownership^Invisible Ownership — Futuruna Research^<h1>Invisible Ownership</h1>"
)

test -s "$output_dir/sitemap.xml" || { echo "missing sitemap.xml" >&2; exit 1; }

generated_pages=()
for entry in "${pages[@]}"; do
  IFS='^' read -r relative_file canonical title content_marker <<< "$entry"
  file="$output_dir/$relative_file"
  test -s "$file" || { echo "missing generated page: $relative_file" >&2; exit 1; }
  generated_pages+=("$file")

  test "$(count_pattern '<title([ >])' "$file")" -eq 1 || {
    echo "$relative_file must contain exactly one title" >&2
    exit 1
  }
  test "$(count_pattern '<meta[^>]+name="description"' "$file")" -eq 1 || {
    echo "$relative_file must contain exactly one meta description" >&2
    exit 1
  }
  test "$(count_pattern '<link[^>]+rel="canonical"' "$file")" -eq 1 || {
    echo "$relative_file must contain exactly one canonical link" >&2
    exit 1
  }
  canonical_tag="$({ grep -Eo '<link[^>]+rel="canonical"[^>]*>' "$file" || true; })"
  [[ "$canonical_tag" == *"href=\"$canonical\""* ]] || {
    echo "$relative_file canonical tag does not point to $canonical" >&2
    exit 1
  }
  grep -Fq "<title>$title</title>" "$file" || {
    echo "$relative_file does not contain its exact expected title" >&2
    exit 1
  }
  grep -Fq "<loc>$canonical</loc>" "$output_dir/sitemap.xml" || {
    echo "$canonical is missing from sitemap.xml" >&2
    exit 1
  }
  grep -Eq '<h1([ >])' "$file" || {
    echo "$relative_file does not contain an h1" >&2
    exit 1
  }
  grep -Fq "$content_marker" "$file" || {
    echo "$relative_file does not contain its route-specific content marker" >&2
    exit 1
  }
  if grep -Fq '<div id="main"></div>' "$file"; then
    echo "$relative_file still contains an empty client-only app shell" >&2
    exit 1
  fi

  for property in og:type og:title og:description og:url og:image og:image:width og:image:height og:image:alt; do
    test "$(count_pattern "<meta[^>]+property=\"$property\"" "$file")" -eq 1 || {
      echo "$relative_file must contain exactly one $property meta tag" >&2
      exit 1
    }
  done
  for name in twitter:title twitter:description twitter:image twitter:image:alt; do
    test "$(count_pattern "<meta[^>]+name=\"$name\"" "$file")" -eq 1 || {
      echo "$relative_file must contain exactly one $name meta tag" >&2
      exit 1
    }
  done
  og_url_tag="$({ grep -Eo '<meta[^>]+property="og:url"[^>]*>' "$file" || true; })"
  [[ "$og_url_tag" == *"content=\"$canonical\""* ]] || {
    echo "$relative_file og:url does not point to $canonical" >&2
    exit 1
  }
done

income_cliffs_file="$output_dir/research/income-cliffs.html"
income_og_type_tag="$({ grep -Eo '<meta[^>]+property="og:type"[^>]*>' "$income_cliffs_file" || true; })"
[[ "$income_og_type_tag" == *'content="article"'* ]] || {
  echo "income-cliffs article is missing og:type=article" >&2
  exit 1
}
for property in article:published_time article:modified_time; do
  test "$(count_pattern "<meta[^>]+property=\"$property\"" "$income_cliffs_file")" -eq 1 || {
    echo "income-cliffs article must contain exactly one $property meta tag" >&2
    exit 1
  }
done

for public_asset in robots.txt llms.txt ai-setup.md codemeta.json _headers; do
  test -s "$output_dir/$public_asset" || {
    echo "missing public discovery asset: $public_asset" >&2
    exit 1
  }
done

social_image_tag="$({ grep -Eo '<meta[^>]+property="og:image"[^>]*>' "$output_dir/index.html" || true; })"
social_image_url="$(sed -n 's/.*content="\([^"]*\)".*/\1/p' <<< "$social_image_tag")"
social_image_path="${social_image_url#https://futuruna.com}"
[[ "$social_image_url" == https://futuruna.com/* ]] && test -s "$output_dir$social_image_path" || {
  echo "generated Open Graph image does not resolve inside the static artifact" >&2
  exit 1
}

python3 - "$output_dir/codemeta.json" "${generated_pages[@]}" <<'PY'
import json
import re
import sys

with open(sys.argv[1], encoding="utf-8") as codemeta:
    json.load(codemeta)

for page_path in sys.argv[2:]:
    html = open(page_path, encoding="utf-8").read()
    documents = re.findall(
        r'<script[^>]+type="application/ld\+json"[^>]*>(.*?)</script>',
        html,
        flags=re.DOTALL,
    )
    if not documents:
        raise SystemExit(f"{page_path} has no JSON-LD document")
    parsed = [json.loads(document) for document in documents]
    if page_path.endswith("research/income-cliffs.html"):
        if not any(
            document.get("@type") == "TechArticle"
            and document.get("@id")
            == "https://futuruna.com/research/income-cliffs#article"
            for document in parsed
            if isinstance(document, dict)
        ):
            raise SystemExit("income-cliffs article has no matching TechArticle JSON-LD")
PY

grep -Fq 'id="nav-menu-toggle"' "$output_dir/index.html" || {
  echo "generated navigation is missing its mobile menu control" >&2
  exit 1
}
grep -Fq 'href="https://github.com/Futuruna/futuruna"' "$output_dir/index.html" || {
  echo "generated navigation is missing the GitHub destination" >&2
  exit 1
}
grep -Fq 'Cache-Control: public, max-age=31536000, immutable' "$output_dir/_headers" || {
  echo "fingerprinted assets are missing their immutable cache policy" >&2
  exit 1
}

test -s "$output_dir/404.html" || { echo "missing 404.html" >&2; exit 1; }
grep -Fq 'noindex' "$output_dir/404.html" || { echo "404.html must be noindex" >&2; exit 1; }
test -s "$output_dir/_redirects" || { echo "missing _redirects" >&2; exit 1; }
grep -Fq '/research/optimization /research/philosophy 301' "$output_dir/_redirects" || {
  echo "missing optimization redirect" >&2
  exit 1
}
test ! -e "$output_dir/research/optimization.html" || {
  echo "legacy optimization route must redirect instead of being generated" >&2
  exit 1
}

echo "Verified ${#pages[@]} statically generated Futuruna pages."
