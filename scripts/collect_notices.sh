#!/bin/bash
# Assembles the legal-notices bundle shipped in every app and CLI download
# (GPLv3 §4-§5, LGPLv3 §4, BSD/Apache notice reproduction, AOM/Google patent
# grants, IJG acknowledgement) into build/licenses/:
#
#   COPYING                    GPL-3.0 text (the license of the binaries)
#   GPL-2.0.txt, LGPL-3.0.txt  gifsicle / optional heic-feature libraries
#   THIRD_PARTY_LICENSES.html  cargo-about output for every linked Rust crate
#   NOTICE.md                  overview + attribution requirements
#   manual/                    hand-maintained notices (licenses/manual/)
#   vendored/<crate>/...       NOTICE*, PATENTS*, COPYING*, COPYRIGHT*, README.ijg
#                              and C-library LICENSE files swept from the
#                              dependency source trees
#
# Fails if cargo-about cannot determine a license for any linked crate (that
# means about.toml needs a [crate.clarify] entry).
#
# Usage: scripts/collect_notices.sh [--check]   (--check: only run the gates)
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

check_only=0
[[ "${1:-}" == "--check" ]] && check_only=1

out="build/licenses"

for tool in cargo cargo-deny cargo-about python3; do
  command -v "$tool" >/dev/null 2>&1 || { echo "error: required tool not found: $tool (cargo install --locked cargo-deny cargo-about --features cli)" >&2; exit 1; }
done

echo "==> cargo deny check licenses bans sources"
cargo deny check licenses bans sources

echo "==> cargo about generate"
mkdir -p build
about_log=$(mktemp)
trap 'rm -f "$about_log"' EXIT
tmp_html="build/THIRD_PARTY_LICENSES.html.tmp"
cargo about generate --locked -c about.toml licenses/about.hbs -o "$tmp_html" 2>"$about_log" || { cat "$about_log" >&2; exit 1; }
if grep -qE "unable to synthesize|doesn't have a license field|failed to satisfy|no license" "$about_log"; then
  echo "error: cargo-about could not resolve a license for a linked crate; add a clarify entry to about.toml:" >&2
  cat "$about_log" >&2
  exit 1
fi

if [[ $check_only -eq 1 ]]; then
  rm -f "$tmp_html"
  echo "==> license gates passed"
  exit 0
fi

rm -rf "$out"
mkdir -p "$out/manual" "$out/vendored"
mv "$tmp_html" "$out/THIRD_PARTY_LICENSES.html"
cp COPYING "$out/COPYING"
cp licenses/GPL-2.0.txt licenses/LGPL-3.0.txt "$out/"
cp NOTICE.md "$out/NOTICE.md"
cp licenses/manual/* "$out/manual/"

echo "==> sweeping dependency sources for NOTICE/PATENTS/COPYING files"
cargo metadata --format-version 1 --locked --filter-platform "$(rustc -vV | sed -n 's/^host: //p')" 2>/dev/null \
| python3 -I -c '
import json, os, re, shutil, sys

out = sys.argv[1]
root = os.getcwd()
meta = json.load(sys.stdin)
NAME = re.compile(r"^(NOTICE|PATENTS|COPYING|COPYRIGHT|LICENSE|LICENCE|README\.ijg)", re.I)
# Plain LICENSE files are covered by cargo-about for ordinary crates; only
# sweep them inside bundled C/C++ source trees (vendor/, third_party/, libjxl/ ...).
ALWAYS = re.compile(r"^(NOTICE|PATENTS|COPYING|COPYRIGHT|README\.ijg)", re.I)
SKIP_DIR = {".git", "test", "tests", "testdata", "doc", "docs", "examples", "fuzz", "benches", "bench", "oss-fuzz", "target", "build", "debian"}
count = 0
for pkg in meta["packages"]:
    d = os.path.dirname(pkg["manifest_path"])
    label = "%s-%s" % (pkg["name"], pkg["version"])
    for cur, dirs, files in os.walk(d):
        rel = os.path.relpath(cur, d)
        depth = 0 if rel == "." else rel.count(os.sep) + 1
        dirs[:] = [x for x in dirs if x not in SKIP_DIR] if depth < 6 else []
        in_c_tree = any(p in ("vendor", "third_party", "libjxl", "libavif", "libaom", "dav1d", "libwebp", "jpegli", "mozjpeg", "highway", "lcms2", "libheif") for p in rel.split(os.sep))
        for f in files:
            if not NAME.match(f):
                continue
            if not (ALWAYS.match(f) or in_c_tree):
                continue
            src = os.path.join(cur, f)
            dest_dir = os.path.join(out, label)
            os.makedirs(dest_dir, exist_ok=True)
            flat = (rel + "/" + f).replace("./", "").replace("/", "__").lstrip("_")
            shutil.copyfile(src, os.path.join(dest_dir, flat))
            count += 1
print("    %d files from dependency sources" % count)
' "$out/vendored"

if [[ -z "$(ls -A "$out/vendored")" ]]; then
  echo "error: no vendored notice files found (cargo metadata sweep failed?)" >&2
  exit 1
fi
for must in libaom-sys libwebp-sys jpegxl-src imagequant; do
  ls -d "$out"/vendored/${must}-* >/dev/null 2>&1 || { echo "error: expected notices for $must missing from sweep" >&2; exit 1; }
done

echo "==> notices bundle: $out ($(du -sh "$out" | cut -f1))"
