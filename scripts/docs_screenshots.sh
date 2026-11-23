#!/bin/bash
# Turns raw PNG screenshots into the AVIF files the docs site shows, using imgcompress itself at the
# "Visually lossless" target (SSIMULACRA2 88), and prints each result's size, score and the width/height
# to put in {% include shot.html %} (half the pixel size, since captures are Retina 2x).
#
# Usage: scripts/docs_screenshots.sh [raw-dir]     (default build/shots-raw; output docs/assets/shots)
# Env:   IMGCOMPRESS=<path to the binary>          (default: imgcompress on PATH, else the installed app's)
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
raw="${1:-build/shots-raw}"
out=docs/assets/shots
bin="${IMGCOMPRESS:-$(command -v imgcompress || echo /Applications/ImageCompress.app/Contents/MacOS/imgcompress)}"
[[ -x "$bin" ]] || { echo "error: imgcompress not found (set IMGCOMPRESS)" >&2; exit 1; }

shopt -s nullglob
shots=("$raw"/*.png)
[[ ${#shots[@]} -gt 0 ]] || { echo "error: no .png screenshots in $raw" >&2; exit 1; }
mkdir -p "$out"

printf '%-26s %11s %9s %9s %7s  %s\n' shot pixels png avif ssim2 "include size"
for png in "${shots[@]}"; do
  name=$(basename "$png" .png)
  rm -f "$out/$name.avif"
  "$bin" run --format avif --target-quality 88 --target-tolerance 0 --max-encodes 16 --destination "$out" --overwrite replace --replace-if always --json "$png" \
    | python3 -I -c '
import json, re, subprocess, sys
name, png = sys.argv[1], sys.argv[2]
score, fin = "-", None
for line in sys.stdin:
    e = json.loads(line)
    if e.get("event") == "stage":
        m = re.search(r"ssim2=([0-9.]+)", e.get("stage", ""))
        if m: score = m.group(1)
    elif e.get("event") == "finished":
        fin = e
    elif e.get("event") in ("failed", "skipped"):
        sys.exit(f"error: {name}: {e}")
if not fin: sys.exit(f"error: {name}: no result")
w, h = (int(v) for v in subprocess.check_output(["sips", "-g", "pixelWidth", "-g", "pixelHeight", png], text=True).split()[-3::2])
ib, ob = (f"{n/1024:.0f} KB" for n in (fin["in_bytes"], fin["out_bytes"]))
print(f"{name:<26} {w:>5}x{h:<5} {ib:>9} {ob:>9} {score:>7}  w={w//2} h={h//2}")
' "$name" "$png"
done
