#!/bin/bash
# Sourced by release.sh, notarize.sh and repo_key.sh.
#
# op_retry <op args...>   runs `op` and prints its output. If the 1Password approval prompt times out (the owner
#                         was away from the keyboard), it beeps and asks again, up to 3 attempts, instead of failing
#                         a release whose long build has already finished. It also beeps if one approval has waited
#                         more than a minute.

op_beep() { command -v afplay >/dev/null 2>&1 && afplay /System/Library/Sounds/Glass.aiff >/dev/null 2>&1 & }

op_retry() {
  local attempt out rc err nag
  err=$(mktemp)
  for attempt in 1 2 3; do
    ( sleep 60; op_beep ) >/dev/null 2>&1 &
    nag=$!
    rc=0
    out=$(op "$@" 2>"$err") || rc=$?
    kill "$nag" 2>/dev/null || true
    if [[ $rc -eq 0 ]]; then
      rm -f "$err"
      [[ -n "$out" ]] && printf '%s\n' "$out"
      return 0
    fi
    if [[ $attempt -lt 3 ]] && grep -qiE 'authorization (timeout|prompt dismissed)|not currently signed in|account is not signed in' "$err"; then
      echo "==> 1Password approval timed out; approve the new prompt (attempt $((attempt + 1)) of 3)" >&2
      op_beep
      continue
    fi
    cat "$err" >&2
    rm -f "$err"
    return "$rc"
  done
}
