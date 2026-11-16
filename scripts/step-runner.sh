# shellcheck shell=bash
# Step runner shared by scripts/mint.sh and scripts/tax-audit-safeguards.sh.
# Sourced, not executed; keep it compatible with macOS's /bin/bash 3.2.
#
# A caller runs `steps_init PREFIX "GROUP..." "$@"`, then declares its steps in
# execution order and ends with `steps_finish`:
#
#   prerequisite CMD...   runs whenever any step runs; a failure stops at once
#   step GROUP CMD...     runs when GROUP is selected (every group by default)
#
# Each script is the single definition of its steps; CI selects groups of it
# instead of repeating commands.

STEPS_PREFIX=
STEPS_GROUPS=()
STEPS_SELECTED=()
STEPS_KEEP_GOING=0
STEPS_LIST=0
STEPS_TOTAL=0
STEPS_FAILED=()

steps_usage() {
    cat <<EOF
Usage: $0 [--group NAME]... [--keep-going] [--list]

  --group NAME   run only the steps of group NAME (repeatable); default: all
  --keep-going   run every selected step, then list each failure and exit 1
                 (default: stop at the first failure with its exit status)
  --list         print the selected steps and their groups without running

Groups: ${STEPS_GROUPS[*]}
EOF
}

steps_init() {
    STEPS_PREFIX="$1"
    read -r -a STEPS_GROUPS <<<"$2"
    shift 2
    local name group known
    while (($#)); do
        case "$1" in
            --group)
                if (($# < 2)); then
                    steps_usage >&2
                    exit 2
                fi
                STEPS_SELECTED+=("$2")
                shift 2
                ;;
            --group=*)
                STEPS_SELECTED+=("${1#--group=}")
                shift
                ;;
            --keep-going)
                STEPS_KEEP_GOING=1
                shift
                ;;
            --list)
                STEPS_LIST=1
                shift
                ;;
            -h | --help)
                steps_usage
                exit 0
                ;;
            *)
                echo "$STEPS_PREFIX unknown argument: $1" >&2
                steps_usage >&2
                exit 2
                ;;
        esac
    done
    for name in ${STEPS_SELECTED[@]+"${STEPS_SELECTED[@]}"}; do
        known=0
        for group in "${STEPS_GROUPS[@]}"; do
            if [[ "$group" == "$name" ]]; then
                known=1
            fi
        done
        if ((!known)); then
            echo "$STEPS_PREFIX unknown group: $name (groups: ${STEPS_GROUPS[*]})" >&2
            exit 2
        fi
    done
}

# True when no --group narrowed the run, so every step of the script runs.
steps_running_all() {
    ((${#STEPS_SELECTED[@]} == 0))
}

steps_group_selected() {
    steps_running_all && return 0
    local name
    for name in "${STEPS_SELECTED[@]}"; do
        if [[ "$name" == "$1" ]]; then
            return 0
        fi
    done
    return 1
}

# Runs one command; prints its outcome and duration. Returns its exit status.
steps_execute() {
    local started=$SECONDS status=0
    echo
    echo "$STEPS_PREFIX $*"
    "$@" || status=$?
    if ((status == 0)); then
        echo "$STEPS_PREFIX ok ($((SECONDS - started))s): $*"
    else
        echo "$STEPS_PREFIX FAILED (exit $status, $((SECONDS - started))s): $*"
    fi
    return "$status"
}

prerequisite() {
    if ((STEPS_LIST)); then
        printf '%-12s %s\n' "(always)" "$*"
        return 0
    fi
    local status=0
    steps_execute "$@" || status=$?
    ((status == 0)) || exit "$status"
}

step() {
    local group="$1"
    shift
    steps_group_selected "$group" || return 0
    if ((STEPS_LIST)); then
        printf '%-12s %s\n' "$group" "$*"
        return 0
    fi
    STEPS_TOTAL=$((STEPS_TOTAL + 1))
    local status=0
    steps_execute "$@" || status=$?
    ((status == 0)) && return 0
    ((STEPS_KEEP_GOING)) || exit "$status"
    STEPS_FAILED+=("exit $status [$group] $*")
}

# Exits 0 after --list; otherwise lists every failed step and exits 1 when
# any failed, or returns so the caller can report success.
steps_finish() {
    if ((STEPS_LIST)); then
        exit 0
    fi
    if ((${#STEPS_FAILED[@]} == 0)); then
        return 0
    fi
    local failure
    echo
    echo "$STEPS_PREFIX ${#STEPS_FAILED[@]} of $STEPS_TOTAL steps failed:"
    for failure in "${STEPS_FAILED[@]}"; do
        echo "$STEPS_PREFIX   $failure"
    done
    if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
        {
            echo "### $STEPS_PREFIX ${#STEPS_FAILED[@]} of $STEPS_TOTAL steps failed"
            echo
            for failure in "${STEPS_FAILED[@]}"; do
                echo "- \`$failure\`"
            done
        } >>"$GITHUB_STEP_SUMMARY"
    fi
    exit 1
}
