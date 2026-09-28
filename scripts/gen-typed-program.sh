#!/usr/bin/env bash
# Seeded generator of small typed Futuruna programs for the differential gate.
# Usage: scripts/gen-typed-program.sh <seed> <output.runa>
# Each program combines a record, a sum type, a rule, collections (lists, maps,
# sets, sorting) and strings; the choice of values and operations follows the
# seed, so a seed always produces the same program.
set -euo pipefail

seed="$1"
out="$2"
state=$(( seed % 2147483647 ))
if (( state <= 0 )); then state=$(( state + 2147483646 )); fi

# Draws happen in arithmetic expansion (no subshell) so every draw advances K.
RN=()
for (( k = 0; k < 256; k++ )); do
    state=$(( (state * 48271) % 2147483647 ))
    RN+=("$state")
done
K=0

names=("Ane" "Bo" "Åse" "Zeta" "anna" "v1.0" "a { b }" "say \\\"hi\\\"" "tab\\tx" "1.0")
tags=("urgent" "legal" "tax" "2.0" "x(y)" "x { y }" "")
count=$(( $(( RN[K++] % 4 )) + 2 ))

{
    echo "-- Generated typed differential program, seed ${seed}."
    echo "# Claim(id: Int, name: String, amount: Float, tags: List(String))"
    echo "# Status = Approved | Rejected(String) | Pending(Int)"
    echo
    echo "> status_of(c: Claim) -> Status {"
    echo "    if c.amount > $(( RN[K++] % 900 )).5 { Rejected(\"limit \" + show(c.id)) } else if length(c.tags) > $(( RN[K++] % 3 )) { Pending(c.id) } else { Approved }"
    echo "}"
    echo
    echo "| fee(c) -> $(( RN[K++] % 50 )) under c.amount < $(( RN[K++] % 500 )).25"
    echo "| fee(c) -> $(( RN[K++] % 200 ))"
    echo
    echo "> label(s: Status) -> String {"
    echo "    match s {"
    echo "        | Approved -> \"ok\""
    echo "        | Rejected(why) -> \"no: \" + why"
    echo "        | Pending(n) -> \"wait \" + show(n)"
    echo "    }"
    echo "}"
    echo
    printf '= claims = ['
    for (( i = 0; i < count; i++ )); do
        (( i > 0 )) && printf ', '
        printf 'Claim(%d, "%s", %d.%d, ["%s", "%s"])' \
            $(( $(( RN[K++] % 30 )) - 10 )) "${names[$(( RN[K++] % ${#names[@]} ))]}" \
            $(( RN[K++] % 1200 )) $(( RN[K++] % 100 )) "${tags[$(( RN[K++] % ${#tags[@]} ))]}" "${tags[$(( RN[K++] % ${#tags[@]} ))]}"
    done
    echo ']'
    echo '= statuses = map(claims, |c| status_of(c))'
    echo '@ print(show(claims))'
    echo '@ print(show(statuses))'
    echo '@ print(join(map(statuses, |s| label(s)), "; "))'
    echo '@ print(show(map(claims, |c| fee(c))))'
    echo '= ids = map(claims, |c| c.id)'
    echo '@ print(show(sort(ids)))'
    echo '@ print(show(sort(map(claims, |c| c.name))))'
    echo '@ print(show(sort_by(claims, |c| c.amount)))'
    echo '= by_id = map_from(map(claims, |c| (c.id, c.name)))'
    echo '@ print(show(by_id))'
    echo '@ print(show(map_keys(by_id)))'
    echo '= tag_set = set_from_list(flat_map(claims, |c| c.tags))'
    echo '@ print(show(tag_set))'
    echo '@ print(show(set_len(tag_set)))'
    echo "@ print(show(set_contains(tag_set, \"${tags[$(( RN[K++] % ${#tags[@]} ))]}\")))"
    echo '= total = foldl(map(claims, |c| c.amount), 0.0, |acc, a| acc + a)'
    echo '@ print(show(total))'
    echo "@ print(show(total / $(( $(( RN[K++] % 9 )) + 1 )).0))"
    echo "@ print(show(chunked(ids, $(( $(( RN[K++] % 3 )) + 1 )))))"
    echo '@ print(show(filter(claims, |c| c.amount > 100.0)))'
    echo '@ print(show(distinct(map(claims, |c| c.name))))'
    echo '@ print(show(Pair(length(claims), head(map(claims, |c| c.name)))))'
} >"$out"
