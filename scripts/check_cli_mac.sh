#!/bin/bash
# Verify build/cli/imgcompress (universal): slices, linkage, tools --check, bundled-gifsicle lookup in a mock app
# bundle with a minimal PATH, and arm64 vs x86_64 (Rosetta) output equality on a few fixtures.
# Env: SKIP_X86=1 to skip the Rosetta comparison.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
CLI="${CLI:-$ROOT/build/cli/imgcompress}"
GIF="${GIF:-$ROOT/build/helpers/gifsicle}"
WORK=$(mktemp -d); trap 'rm -rf "$WORK"' EXIT
fail=0
ok()  { echo "ok:   $*"; }
bad() { echo "FAIL: $*" >&2; fail=1; }

for f in "$CLI" "$GIF"; do
  info=$(lipo -info "$f")
  [[ "$info" == *x86_64* && "$info" == *arm64* ]] && ok "universal: $info" || bad "not universal: $info"
  if [[ $f == "$CLI" ]]; then
    for a in arm64 x86_64; do
      v=$(lipo -thin $a "$f" -output "$WORK/slice" && otool -l "$WORK/slice" | awk '/minos/{print $2; exit}')
      [[ "$v" == 15.0 ]] && ok "$a minos $v" || bad "$a minos $v (want 15.0)"
    done
  fi
  nonsys=$(otool -L "$f" | grep -E '^\s' | awk '{print $1}' | grep -vE '^(/usr/lib/|/System/)' || true)
  [[ -z "$nonsys" ]] && ok "only system libs: $(basename "$f")" || bad "non-system libs in $f: $nonsys"
done

# Mock app bundle: Contents/MacOS/imgcompress + Contents/Helpers/gifsicle, no Homebrew on PATH.
B="$WORK/Mock.app/Contents"; mkdir -p "$B/MacOS" "$B/Helpers"
cp "$CLI" "$B/MacOS/imgcompress"; cp "$GIF" "$B/Helpers/gifsicle"
out=$(env -i PATH=/usr/bin:/bin "$B/MacOS/imgcompress" tools --check) && ok "tools --check (mock bundle, clean env)" || { echo "$out"; bad "tools --check failed"; }
grep -q "Contents/MacOS/../Helpers/gifsicle" <<<"$out" && ok "gifsicle resolved from ../Helpers" || { echo "$out"; bad "gifsicle not resolved from bundle"; }

# CLI-next-to-exe layout (tarball / Linux-style).
mkdir -p "$WORK/flat"; cp "$CLI" "$GIF" "$WORK/flat/"
env -i PATH=/usr/bin:/bin "$WORK/flat/imgcompress" tools --check | grep -q "$WORK/flat/gifsicle" && ok "gifsicle resolved next to exe" || bad "gifsicle not found next to exe"

# arm64 vs x86_64 determinism.
if [[ -z "${SKIP_X86:-}" ]]; then
  if arch -x86_64 /usr/bin/true 2>/dev/null; then
    # Integer/entropy-coded paths must be byte-identical across arches. Float-SIMD encoders (imagequant, aom, libjxl)
    # legitimately differ between NEON and SSE/AVX, so those only need to be within 5% in size.
    CASES=("jpeg/baseline.jpg:jpeg:70:exact" "jpeg/baseline.jpg:jpeg:85:exact" "png/palette.png:keep:80:exact" "gif/animated.gif:keep:80:exact"
           "modern/photo.webp:webp:75:exact" "png/rgba_photo.png:png:80:approx" "png/rgba_photo.png:avif:60:approx" "png/flat_illustration.png:jxl:80:approx")
    for c in "${CASES[@]}"; do
      IFS=: read -r rel fmt q mode <<<"$c"
      for a in arm64 x86_64; do
        d="$WORK/out-$a"; rm -rf "$d"; mkdir -p "$d"
        env -i PATH=/usr/bin:/bin arch -$a "$CLI" run "fixtures/$rel" --format "$fmt" --quality "$q" --destination "$d" --originals keep -q >/dev/null 2>&1 || true
        f=$(ls "$d" | head -1)
        if [[ -n "$f" ]]; then echo "$(shasum -a 256 "$d/$f" | awk '{print $1}') $(wc -c < "$d/$f" | tr -d ' ')" > "$WORK/sum-$a"; else echo "none 0" > "$WORK/sum-$a"; fi
      done
      read -r h1 s1 < "$WORK/sum-arm64"; read -r h2 s2 < "$WORK/sum-x86_64"
      label="$rel -> $fmt q$q ($mode; arm64 ${s1}B, x86_64 ${s2}B)"
      if [[ $s1 == 0 || $s2 == 0 ]]; then bad "no output: $label"
      elif [[ $h1 == "$h2" ]]; then ok "identical: $label"
      elif [[ $mode == approx ]] && (( (s1 - s2) * 100 <= 5 * s1 && (s2 - s1) * 100 <= 5 * s1 )); then ok "within 5%: $label"
      else bad "differs: $label"; fi
    done
  else
    echo "skip: Rosetta not available"
  fi
fi
[[ $fail == 0 ]] && echo "ALL CHECKS PASSED" || { echo "CHECKS FAILED" >&2; exit 1; }
