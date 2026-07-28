#!/usr/bin/env bash
set -euo pipefail

repo_url="${FUTURUNA_REPO_URL:-https://github.com/Futuruna/futuruna.git}"
ssh_repo_url="${FUTURUNA_SSH_REPO_URL:-git@github.com:Futuruna/futuruna.git}"
branch="${FUTURUNA_BRANCH:-main}"
install_dir="${FUTURUNA_HOME:-$HOME/.local/share/futuruna}"
bin_dir="${FUTURUNA_BIN_DIR:-$HOME/.local/bin}"
link_runa=0
run_smoke=1

usage() {
    cat <<'USAGE'
Futuruna AI bootstrap

Installs or updates Futuruna into ~/.local/share/futuruna, builds runa, and
runs a smoke test. No sudo and no shell profile edits are performed.

Usage:
  bash ai-bootstrap.sh [options]

Options:
  --dir PATH       Install or update Futuruna at PATH
  --branch NAME    Git branch to fetch and build (default: main)
  --link           Link runa into ~/.local/bin/runa
  --no-smoke       Skip the weather demo smoke test
  -h, --help       Show this help

Environment:
  FUTURUNA_HOME      Install directory override
  FUTURUNA_BRANCH    Branch override
  FUTURUNA_BIN_DIR   Link directory override for --link
  FUTURUNA_REPO_URL  Repository URL override
  FUTURUNA_SSH_REPO_URL  SSH fallback URL override
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dir)
            [ "$#" -ge 2 ] || { echo "Missing value for --dir" >&2; exit 2; }
            install_dir="$2"
            shift 2
            ;;
        --branch)
            [ "$#" -ge 2 ] || { echo "Missing value for --branch" >&2; exit 2; }
            branch="$2"
            shift 2
            ;;
        --link)
            link_runa=1
            shift
            ;;
        --no-smoke)
            run_smoke=0
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1" >&2
        return 1
    fi
}

if [ "${EUID:-$(id -u)}" = "0" ]; then
    echo "Run this as a normal user, not with sudo." >&2
    exit 1
fi

need git || exit 1
if ! need cargo; then
    cat >&2 <<'RUSTUP'

Rust/Cargo is required.
Install it first:
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Then rerun:
  curl -fsSL https://futuruna.com/ai-bootstrap.sh | bash
RUSTUP
    exit 1
fi

auth_error() {
    cat >&2 <<EOF

Could not clone or update Futuruna non-interactively.

This works immediately if the GitHub repository is public. If the repository is
private, run this from an environment with GitHub access, or configure SSH and
rerun:

  FUTURUNA_REPO_URL=$ssh_repo_url bash ai-bootstrap.sh
  curl -fsSL https://futuruna.com/ai-bootstrap.sh | FUTURUNA_REPO_URL=$ssh_repo_url bash

EOF
    exit 1
}

fetch_origin() {
    case "$remote" in
        git@*|ssh://*)
            GIT_SSH_COMMAND="ssh -o BatchMode=yes" \
                git -C "$install_dir" fetch --depth 1 origin "$branch"
            ;;
        *)
            GIT_TERMINAL_PROMPT=0 \
                git -C "$install_dir" fetch --depth 1 origin "$branch"
            ;;
    esac
}

clone_futuruna() {
    if GIT_TERMINAL_PROMPT=0 \
        git clone --depth 1 --branch "$branch" "$repo_url" "$install_dir"; then
        return 0
    fi

    if [ "$repo_url" != "$ssh_repo_url" ]; then
        echo
        echo "HTTPS clone failed; trying SSH fallback..."
        GIT_SSH_COMMAND="ssh -o BatchMode=yes" \
            git clone --depth 1 --branch "$branch" "$ssh_repo_url" "$install_dir"
    fi
}

echo "Futuruna bootstrap"
echo "  repository: $repo_url"
echo "  branch:     $branch"
echo "  directory:  $install_dir"
echo

if [ -d "$install_dir/.git" ]; then
    remote="$(git -C "$install_dir" config --get remote.origin.url || true)"
    case "$remote" in
        https://github.com/Futuruna/futuruna|https://github.com/Futuruna/futuruna.git|git@github.com:Futuruna/futuruna.git)
            ;;
        *)
            echo "Refusing to update existing checkout with unexpected origin:" >&2
            echo "  $install_dir" >&2
            echo "  origin: ${remote:-<none>}" >&2
            exit 1
            ;;
    esac

    if git -C "$install_dir" diff --quiet && git -C "$install_dir" diff --cached --quiet; then
        echo "Updating existing Futuruna checkout..."
        if fetch_origin; then
            git -C "$install_dir" checkout -q -B "$branch" "origin/$branch"
        elif [ "$remote" != "$ssh_repo_url" ]; then
            echo
            echo "Fetch from origin failed; trying SSH fallback..."
            GIT_SSH_COMMAND="ssh -o BatchMode=yes" \
                git -C "$install_dir" fetch --depth 1 "$ssh_repo_url" "$branch" || auth_error
            git -C "$install_dir" checkout -q -B "$branch" FETCH_HEAD
        else
            auth_error
        fi
    else
        echo "Existing checkout has local changes; building it as-is."
    fi
elif [ -e "$install_dir" ]; then
    echo "Install path exists but is not a git checkout:" >&2
    echo "  $install_dir" >&2
    echo "Choose another path with --dir or FUTURUNA_HOME." >&2
    exit 1
else
    echo "Cloning Futuruna..."
    mkdir -p "$(dirname "$install_dir")"
    clone_futuruna || auth_error
fi

echo
echo "Building runa..."
cargo build --release --manifest-path "$install_dir/Cargo.toml" --bin runa

runa="$install_dir/target/release/runa"
echo
"$runa" --version

if [ "$run_smoke" = "1" ]; then
    echo
    echo "Running smoke test: examples/weather_demo.runa"
    "$runa" run "$install_dir/examples/weather_demo.runa"
fi

if [ "$link_runa" = "1" ]; then
    mkdir -p "$bin_dir"
    if [ -e "$bin_dir/runa" ] && [ ! -L "$bin_dir/runa" ]; then
        echo "Refusing to replace non-symlink: $bin_dir/runa" >&2
        exit 1
    fi
    ln -sfn "$runa" "$bin_dir/runa"
    echo
    echo "Linked: $bin_dir/runa -> $runa"
fi

cat <<EOF

Futuruna is ready.

Compiler:
  $runa

Try:
  "$runa" init hello
  cd hello
  "$runa" check src/main.runa
  "$runa" run src/main.runa

For AI agents:
  1. Read $install_dir/README.md
  2. Inspect $install_dir/examples/weather_demo.runa
  3. Translate a law, policy, or contract into .runa
  4. Audit it for paradoxes, tensions, loopholes, missing definitions, and enforcement gaps

EOF
