#!/usr/bin/env bash
# Usage: measure-ram.sh <pid|process-name>
# Prints RSS and PSS (proportional, shared pages split between processes) for the process and all its children.
set -euo pipefail

target="${1:?usage: measure-ram.sh <pid|process-name>}"

if [[ "$target" =~ ^[0-9]+$ ]]; then
    root="$target"
else
    root="$(pgrep -o -x "$target" || true)"
    [[ -n "$root" ]] || { echo "process not found: $target" >&2; exit 1; }
fi

tree() {
    echo "$1"
    for child in $(pgrep -P "$1" || true); do tree "$child"; done
}

rss=0
pss=0
for pid in $(tree "$root"); do
    [[ -r "/proc/$pid/smaps_rollup" ]] || continue
    read -r r p < <(awk '/^Rss:/ {r=$2} /^Pss:/ {p=$2} END {print r, p}' "/proc/$pid/smaps_rollup")
    printf '%8s  rss %6d MB  pss %6d MB  %s\n' "$pid" $((r / 1024)) $((p / 1024)) "$(tr '\0' ' ' < "/proc/$pid/cmdline" | cut -c1-60)"
    rss=$((rss + r))
    pss=$((pss + p))
done

printf 'TOTAL     rss %6d MB  pss %6d MB\n' $((rss / 1024)) $((pss / 1024))
