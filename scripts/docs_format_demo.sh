#!/bin/bash
# Builds the data and files behind the docs "Format comparison" page (docs/compare.md): for each sample photo it cuts a
# 480x320 detail crop (the busiest spot, taken from the full-size original when build/samples-orig has it), saves it as a
# lossless PNG, encodes it with imgcompress in JPEG, WebP, AVIF, JPEG XL and HEIC at four levels (Visually lossless =
# SSIMULACRA2 88, Smaller = 70, fixed quality 50 and 25), scores each result against the crop, and also encodes the whole
# 2400px sample at the same settings for the "whole photo" sizes. An optional docs/assets/samples/graphic.png is added the
# same way. Every input also gets a lossless PNG result, which is the same at every level.
#
# Usage: scripts/docs_format_demo.sh      (rewrites docs/assets/demo/ and docs/_data/format_demo.json; sizes in KB = 1000 bytes)
# Env:   IMGCOMPRESS=<path to the binary>  (default: imgcompress on PATH, else the installed app's)
# Needs: python3, swift (crop tool), cargo (builds the SSIMULACRA2 scorer example once), sips.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
bin="${IMGCOMPRESS:-$(command -v imgcompress || echo /Applications/ImageCompress.app/Contents/MacOS/imgcompress)}"
[[ -x "$bin" ]] || { echo "error: imgcompress not found (set IMGCOMPRESS)" >&2; exit 1; }
samples=docs/assets/samples
out=docs/assets/demo
data=docs/_data/format_demo.json
version=$("$bin" --version | awk '{print $NF}')

cargo build --release -q -p compress-core --example score
scorer=target/release/examples/score

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Crop tool: picks the 480x320 window (on a 120x80 grid) with the most local contrast and saves it as PNG.
cat > "$work/crop.swift" <<'SWIFT'
import Foundation
import ImageIO
import UniformTypeIdentifiers
let a = CommandLine.arguments
guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: a[1]) as CFURL, nil),
      let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { fatalError("cannot read \(a[1])") }
let cw = 480, ch = 320, W = img.width, H = img.height
var best = (x: 0, y: 0)
if W > cw && H > ch {
    let g = 4  // sample every 4th pixel when measuring
    let gw = W / g, gh = H / g
    let cs = CGColorSpaceCreateDeviceGray()
    var bytes = [UInt8](repeating: 0, count: gw * gh)
    bytes.withUnsafeMutableBytes { p in
        let c = CGContext(data: p.baseAddress, width: gw, height: gh, bitsPerComponent: 8, bytesPerRow: gw, space: cs, bitmapInfo: CGImageAlphaInfo.none.rawValue)!
        c.interpolationQuality = .none
        c.draw(img, in: CGRect(x: 0, y: 0, width: gw, height: gh))
    }
    // Energy of each pixel against its neighbours, measured on a 1:1 crop-sized area is too noisy at 1/4 scale,
    // so use the full-resolution image only through this coarse map plus a fine pass below.
    var energy = [Double](repeating: 0, count: gw * gh)
    for y in 0..<(gh - 1) { for x in 0..<(gw - 1) {
        let v = Int(bytes[y * gw + x])
        energy[y * gw + x] = Double(abs(v - Int(bytes[y * gw + x + 1])) + abs(v - Int(bytes[(y + 1) * gw + x])))
    } }
    // Fine pass: full-resolution gradient on a candidate grid (every 120x80 px), restricted to the central 90%.
    let fw = W, fh = H
    var full = [UInt8](repeating: 0, count: fw * fh)
    full.withUnsafeMutableBytes { p in
        let c = CGContext(data: p.baseAddress, width: fw, height: fh, bitsPerComponent: 8, bytesPerRow: fw, space: cs, bitmapInfo: CGImageAlphaInfo.none.rawValue)!
        c.draw(img, in: CGRect(x: 0, y: 0, width: fw, height: fh))
    }
    // Integral image of gradient magnitude.
    var integ = [Double](repeating: 0, count: (fw + 1) * (fh + 1))
    for y in 0..<(fh - 1) {
        var row = 0.0
        for x in 0..<(fw - 1) {
            let v = Int(full[y * fw + x])
            row += Double(abs(v - Int(full[y * fw + x + 1])) + abs(v - Int(full[(y + 1) * fw + x])))
            integ[(y + 1) * (fw + 1) + x + 1] = integ[y * (fw + 1) + x + 1] + row
        }
    }
    var bestE = -1.0
    let x0 = Int(Double(W) * 0.05), x1 = Int(Double(W) * 0.95) - cw
    let y0 = Int(Double(H) * 0.05), y1 = Int(Double(H) * 0.95) - ch
    var y = y0
    while y <= y1 {
        var x = x0
        while x <= x1 {
            let e = integ[(y + ch) * (fw + 1) + x + cw] - integ[y * (fw + 1) + x + cw] - integ[(y + ch) * (fw + 1) + x] + integ[y * (fw + 1) + x]
            if e > bestE { bestE = e; best = (x, y) }
            x += 120
        }
        y += 80
    }
}
let rect = CGRect(x: best.x, y: best.y, width: min(cw, W), height: min(ch, H))
guard let piece = img.cropping(to: rect),
      let ctx = CGContext(data: nil, width: piece.width, height: piece.height, bitsPerComponent: 8, bytesPerRow: 0,
                          space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue),
      // Convert wide-gamut sources (e.g. ProPhoto) to sRGB, so every format decodes to the same colours.
      let _ = Optional(ctx.draw(piece, in: CGRect(x: 0, y: 0, width: piece.width, height: piece.height))),
      let crop = ctx.makeImage(),
      let dst = CGImageDestinationCreateWithURL(URL(fileURLWithPath: a[2]) as CFURL, UTType.png.identifier as CFString, 1, nil) else { fatalError("crop failed") }
CGImageDestinationAddImage(dst, crop, nil)
guard CGImageDestinationFinalize(dst) else { fatalError("write failed") }
print("\(best.x),\(best.y)")
SWIFT
swiftc -O -o "$work/crop" "$work/crop.swift" 2>/dev/null

rm -rf "$out"
mkdir -p "$out"
: > "$work/results.tsv"
: > "$work/photos.tsv"
failed=()

# encode <input> <outfile-without-ext> <format> <level>  ->  writes the file, prints its path (or nothing on failure)
encode() {
  local input=$1 format=$2 level=$3 dest=$4
  local opts=() ext=$format
  case $level in
    vl) opts=(--target-quality 88 --target-tolerance 0 --max-encodes 16) ;;
    smaller) opts=(--target-quality 70 --target-tolerance 0 --max-encodes 16) ;;
    q50) opts=(--quality 50) ;;
    q25) opts=(--quality 25) ;;
    lossless) opts=(--quality lossless) ;;
  esac
  [[ $format == jpeg ]] && ext=jpg
  # HEIC has no built-in quality search: bisect the quality on the crop for the target, reuse that quality for the whole photo.
  if [[ $format == heic && ( $level == vl || $level == smaller ) ]]; then
    local qvar=HEIC_Q_$level
    if [[ -z ${!qvar} ]]; then
      local target=88 lo=1 hi=100 mid s; [[ $level == smaller ]] && target=70
      while (( lo < hi )); do
        mid=$(( (lo + hi) / 2 ))
        rm -rf "$work/enc"; mkdir -p "$work/enc"
        "$bin" run --format heic --quality "$mid" --destination "$work/enc" --overwrite replace --replace-if always "$input" >/dev/null 2>&1 || true
        s=$(score "$input" "$(ls "$work/enc"/*.heic | head -1)")
        if python3 -I -c 'import sys; sys.exit(0 if float(sys.argv[1])>=float(sys.argv[2]) else 1)' "$s" "$target"; then hi=$mid; else lo=$((mid + 1)); fi
      done
      printf -v "$qvar" %s "$lo"
    fi
    opts=(--quality "${!qvar}")
  fi
  rm -rf "$work/enc"; mkdir -p "$work/enc"
  "$bin" run --format "$format" "${opts[@]}" --destination "$work/enc" --overwrite replace --replace-if always --json "$input" > "$work/enc.log" 2>&1 || true
  local f
  f=$(ls "$work/enc"/*."$ext" 2>/dev/null | head -1 || true)
  [[ -n "$f" ]] || return 1
  mv "$f" "$dest.$ext"
  ENC_STAGE=$(grep -o 'ssim2=[0-9.]*' "$work/enc.log" | tail -1 | cut -d= -f2 || true)
  ENC_PATH="$dest.$ext"
}

score() {  # score <crop.png> <result>
  local s
  s=$("$scorer" "$1" "$2" 2>/dev/null || true)
  if ! [[ $s =~ ^-?[0-9.]+$ ]]; then
    sips -s format png "$2" --out "$work/dec.png" >/dev/null 2>&1 || true
    s=$("$scorer" "$1" "$work/dec.png" 2>/dev/null || true)
    rm -f "$work/dec.png"
  fi
  [[ $s =~ ^-?[0-9.]+$ ]] && echo "$s" || echo "n/a"
}

process() {  # process <id> <title> <credit> <full-image> <crop-source> <formats...>
  local id=$1 title=$2 credit=$3 full=$4 cropsrc=$5; shift 5
  local dir="$out/$id"; mkdir -p "$dir"
  HEIC_Q_vl=""; HEIC_Q_smaller=""
  local pos
  pos=$("$work/crop" "$cropsrc" "$dir/original.png")
  local fw fh
  fw=$(sips -g pixelWidth "$full" | awk '/pixelWidth/{print $2}')
  fh=$(sips -g pixelHeight "$full" | awk '/pixelHeight/{print $2}')
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$title" "$credit" "$fw" "$fh" "$(stat -f %z "$full")" "$(stat -f %z "$dir/original.png")" >> "$work/photos.tsv"
  echo "== $id: crop at $pos from $(basename "$cropsrc")"
  local spec format level
  for spec in "$@"; do
    format=${spec%:*}; level=${spec#*:}
    if ! encode "$dir/original.png" "$format" "$level" "$dir/$format-$level"; then
      echo "FAILED: $id $format $level" >&2; failed+=("$id $format $level"); continue
    fi
    local res=$ENC_PATH stage=$ENC_STAGE s fullkb=""
    s=$(score "$dir/original.png" "$res")
    if [[ -n $stage && $s != n/a ]] && python3 -I -c 'import sys; sys.exit(0 if abs(float(sys.argv[1])-float(sys.argv[2]))<=0.15 else 1)' "$stage" "$s"; then :; elif [[ -n $stage ]]; then echo "note: $id $format $level scorer $s vs search $stage"; fi
    if encode "$full" "$format" "$level" "$work/full"; then
      fullkb=$(stat -f %z "$work/full".*); rm -f "$work/full".*
    else
      echo "FAILED (whole photo): $id $format $level" >&2; failed+=("$id $format $level whole")
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$format" "$level" "${res#"$out/"}" "$(stat -f %z "$res")" "$s" "$fullkb" >> "$work/results.tsv"
  done
}

lossy=()
for f in jpeg webp avif jxl heic; do for l in vl smaller q50 q25; do lossy+=("$f:$l"); done; done

# Photos
for name in landscape portrait city; do
  info=$(python3 -I - "$samples/credits.json" "$name.jpg" <<'PY'
import json, sys, urllib.parse, os
c = [x for x in json.load(open(sys.argv[1])) if x["file"] == sys.argv[2]][0]
orig = os.path.join("build/samples-orig", urllib.parse.unquote(os.path.basename(c["original_url"])))
print("\t".join([c["title"], f'Photo: {c["author"]}, {c["license"].split()[0]}', orig if os.path.exists(orig) else ""]))
PY
  )
  IFS=$'\t' read -r title credit orig <<< "$info"
  process "$name" "$title" "$credit" "$samples/$name.jpg" "${orig:-$samples/$name.jpg}" "${lossy[@]}" png:lossless
done

# Optional graphic
if [[ -f $samples/graphic.png ]]; then
  process graphic "App screenshot (PNG)" "Screenshot: ImageCompress" "$samples/graphic.png" "$samples/graphic.png" "${lossy[@]}" png:lossless
else
  echo "(no $samples/graphic.png, skipping the graphic)"
fi

mkdir -p "$(dirname "$data")"
python3 -I - "$version" "$work/photos.tsv" "$work/results.tsv" "$data" <<'PY'
import json, sys
version, photos_tsv, results_tsv, dest = sys.argv[1:5]

def kb(n):
    v = n / 1000
    return round(v, 1) if v < 10 else round(v)

names = {"jpeg": "JPEG", "webp": "WebP", "avif": "AVIF", "jxl": "JPEG XL", "heic": "HEIC", "png": "PNG"}
formats = [
    ("jpeg", "Photos that need to open anywhere, with fast encoding and decoding.", "Everywhere."),
    ("webp", "Smaller files than JPEG for photos and graphics, with transparency and animation.", "All current browsers."),
    ("avif", "Small files at good quality, especially for photos at low and medium quality.", "All current browsers."),
    ("jxl", "High quality at small sizes, and lossless recompression of existing JPEGs.", "Safari. Other browsers are adding it."),
    ("heic", "Small photos on Apple devices, the format the iPhone camera saves.", "Safari on Apple devices, and Apple Photos."),
    ("png", "Screenshots, logos and anything that must stay pixel-exact, with transparency.", "Everywhere."),
]
# How each format fails at Quality 25 (written from the portrait strip on the page; check after changing encoders).
breaks = {"jpeg": "Square 8×8 blocks and flat patches of colour.", "webp": "Fine detail smears into streaks and soft blocks.", "avif": "Texture is smoothed away into soft blobs, but the file is tiny.", "jxl": "Softer, with faint block edges; keeps more texture than AVIF.", "heic": "Keeps the most texture here, for the largest file.", "png": "Lossless: nothing breaks, but the file is far larger."}
levels = [("vl", "Visually lossless"), ("smaller", "Smaller"), ("q50", "Quality 50"), ("q25", "Quality 25")]

results = {}
for line in open(results_tsv):
    pid, fmt, lvl, src, b, score, fullb = line.rstrip("\n").split("\t")
    results.setdefault(pid, []).append((fmt, lvl, src, int(b), score, fullb))

photos = []
for line in open(photos_tsv):
    pid, title, credit, w, h, fullbytes, origbytes = line.rstrip("\n").split("\t")
    origbytes = int(origbytes)
    rs = []
    for fmt, lvl, src, b, score, fullb in results.get(pid, []):
        r = {"format": fmt, "format_name": names[fmt], "level": "vl" if lvl == "lossless" else lvl, "src": src,
             "bytes": b, "kb": kb(b), "saved": round((1 - b / origbytes) * 100)}
        r["score"] = float(score) if score != "n/a" else None
        r["full_kb"] = kb(int(fullb)) if fullb else None
        if lvl == "lossless":
            r["label"] = "Lossless"
        rs.append(r)
    photos.append({"id": pid, "title": title, "credit": credit,
                   "full": {"w": int(w), "h": int(h), "kb": kb(int(fullbytes))},
                   "original": {"src": f"{pid}/original.png", "bytes": origbytes, "kb": kb(origbytes)},
                   "results": rs})

doc = {"version": version, "crop": {"w": 480, "h": 320},
       "formats": [{"id": i, "name": names[i], "good_at": g, "shows_in": s, "breaks": breaks[i]} for i, g, s in formats],
       "levels": [{"id": i, "name": n} for i, n in levels],
       "photos": photos}
with open(dest, "w") as f:
    json.dump(doc, f, indent=1, ensure_ascii=False)
    f.write("\n")
PY

echo
printf '%-10s %-6s %-8s %8s %7s %7s\n' photo format level kb saved score
python3 -I -c '
import json, sys
d = json.load(open(sys.argv[1]))
for p in d["photos"]:
    for r in p["results"]:
        print("%-10s %-6s %-8s %8s %6s%% %7s" % (p["id"], r["format"], r["level"], r["kb"], r["saved"], r["score"]))
' "$data"
echo "total size of $out: $(du -sh "$out" | cut -f1)"
if [[ ${#failed[@]} -gt 0 ]]; then printf 'FAILED: %s\n' "${failed[@]}" >&2; exit 1; fi
