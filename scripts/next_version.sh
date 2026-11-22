#!/bin/bash
# Prints the next release version as YYYY.MM.DD.N, where N is the build
# counter for today (1 for the first release cut today, 2 for the second...).
# Determined by scanning existing `vYYYY.MM.DD.*` git tags.
#
#   scripts/next_version.sh                 -> 2026.11.15.1
#   scripts/next_version.sh --cargo 2026.11.15.1  -> 2026.11.15-1
#
# --cargo converts a CalVer string into a valid Cargo/SemVer version (SemVer has
# three numeric parts and forbids leading zeros): YYYY.M.D-N.
set -euo pipefail

if [[ "${1:-}" == "--cargo" ]]; then
  calver="${2:?usage: next_version.sh --cargo <YYYY.MM.DD.N>}"
  IFS=. read -r y m d n <<<"$calver"
  [[ -n "${n:-}" ]] || { echo "error: not a CalVer string: $calver" >&2; exit 1; }
  echo "${y}.$((10#$m)).$((10#$d))-${n}"
  exit 0
fi

today=$(date +%Y.%m.%d)

last_n=$(git tag -l "v${today}.*" | sed "s/^v${today}\.//" | sort -n | tail -1)
next_n=$(( ${last_n:-0} + 1 ))

echo "${today}.${next_n}"
