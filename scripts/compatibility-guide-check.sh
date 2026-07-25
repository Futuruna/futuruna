#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/compatibility-guide-check.sh [--base <rev>] [--head <rev>]

Checks whether a change that touches Futuruna stable-surface files also updates
docs/compatibility-guides/ or explicitly explains why no guide entry is needed.

PR CI reads the "Compatibility guide entry (or reason none was needed):" field
from the GitHub pull_request event. Local runs may set:

  FUTURUNA_COMPATIBILITY_GUIDE_REASON="internal refactor only"

EOF
}

BASE="${FUTURUNA_COMPAT_BASE:-}"
HEAD="${FUTURUNA_COMPAT_HEAD:-HEAD}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --base)
            BASE="${2:?--base requires a revision}"
            shift 2
            ;;
        --head)
            HEAD="${2:?--head requires a revision}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "compatibility-guide-check: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -z "$BASE" ]]; then
    if git rev-parse --verify origin/main >/dev/null 2>&1; then
        BASE="origin/main"
    else
        BASE="HEAD~1"
    fi
fi

changed_files() {
    if ! git diff --name-only "$BASE...$HEAD" 2>/dev/null; then
        git diff --name-only "$BASE" "$HEAD"
    fi
}

is_compatibility_guide() {
    case "$1" in
        docs/compatibility-guides/[0-9]*.md) return 0 ;;
        *) return 1 ;;
    esac
}

is_stable_surface() {
    case "$1" in
        src/bin/runa.rs|src/lib.rs|src/proof_kernel.rs) return 0 ;;
        docs/reference/basics.md|docs/reference/runes.md|docs/reference/stdlib.md) return 0 ;;
        docs/tutorial/01-hello.md|docs/tutorial/02-types.md|docs/tutorial/03-functions.md) return 0 ;;
        docs/feature-stages.md|docs/feature-stages.json|docs/compatibility-policy.md) return 0 ;;
        docs/artifact-codegen-contracts.md) return 0 ;;
        tests/expect/artifact/*|tests/expect/golden/artifact/*) return 0 ;;
        *) return 1 ;;
    esac
}

extract_reason() {
    if [[ -n "${FUTURUNA_COMPATIBILITY_GUIDE_REASON:-}" ]]; then
        printf '%s\n' "$FUTURUNA_COMPATIBILITY_GUIDE_REASON"
        return 0
    fi

    if [[ -n "${FUTURUNA_PR_BODY_FILE:-}" && -f "$FUTURUNA_PR_BODY_FILE" ]]; then
        python3 - "$FUTURUNA_PR_BODY_FILE" <<'PY'
import sys

body = open(sys.argv[1], encoding="utf-8").read()
marker = "Compatibility guide entry (or reason none was needed):"
lower = body.lower()
idx = lower.find(marker.lower())
if idx < 0:
    print("")
    raise SystemExit

tail = body[idx + len(marker):]
stops = [
    "\nPermanent coverage added:",
    "\nParked follow-ups",
    "\n## Verification",
    "\n## Review Notes",
    "\nCommands run:",
    "\nSkipped lanes and reason:",
]
positions = [tail.find(stop) for stop in stops if tail.find(stop) >= 0]
end = min(positions) if positions else len(tail)
print(tail[:end].strip())
PY
        return 0
    fi

    if [[ -n "${GITHUB_EVENT_PATH:-}" && -f "$GITHUB_EVENT_PATH" ]]; then
        python3 - "$GITHUB_EVENT_PATH" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    event = json.load(handle)

body = ((event.get("pull_request") or {}).get("body") or "")
marker = "Compatibility guide entry (or reason none was needed):"
lower = body.lower()
idx = lower.find(marker.lower())
if idx < 0:
    print("")
    raise SystemExit

tail = body[idx + len(marker):]
stops = [
    "\nPermanent coverage added:",
    "\nParked follow-ups",
    "\n## Verification",
    "\n## Review Notes",
    "\nCommands run:",
    "\nSkipped lanes and reason:",
]
positions = [tail.find(stop) for stop in stops if tail.find(stop) >= 0]
end = min(positions) if positions else len(tail)
print(tail[:end].strip())
PY
        return 0
    fi

    printf '\n'
}

reason_is_explicit() {
    python3 - "$1" <<'PY'
import re
import sys

text = sys.argv[1].strip()
normalized = re.sub(r"[\s`*_#:\-\[\]().]+", "", text.lower())
placeholders = {
    "",
    "na",
    "n/a",
    "none",
    "noneneeded",
    "notneeded",
    "notapplicable",
    "todo",
    "tbd",
}
raise SystemExit(0 if normalized not in placeholders else 1)
PY
}

stable_changes=()
guide_changed=0

while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    if is_compatibility_guide "$file"; then
        guide_changed=1
    fi
    if is_stable_surface "$file"; then
        stable_changes+=("$file")
    fi
done < <(changed_files)

if [[ ${#stable_changes[@]} -eq 0 ]]; then
    echo "compatibility-guide-check: no tracked stable-surface files changed."
    exit 0
fi

if [[ "$guide_changed" -eq 1 ]]; then
    echo "compatibility-guide-check: compatibility guide updated for stable-surface changes."
    exit 0
fi

reason="$(extract_reason)"
if reason_is_explicit "$reason"; then
    echo "compatibility-guide-check: stable-surface change has explicit no-guide reason."
    exit 0
fi

cat >&2 <<EOF
compatibility-guide-check: stable-surface files changed without a guide update.

Tracked stable-surface changes:
$(printf '  - %s\n' "${stable_changes[@]}")

Update docs/compatibility-guides/0.1.x.md, or fill the PR template field:

  Compatibility guide entry (or reason none was needed):

with a concrete reason. For local runs, set FUTURUNA_COMPATIBILITY_GUIDE_REASON.
EOF
exit 1
