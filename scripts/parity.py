#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Parity check: legacy/compress.py vs `imgcompress run --preset NAME` on copies of fixtures/.

For each built-in preset, both tools run on a fresh copy of the legacy-compatible fixtures.
Per original file we compare: resulting file set (stem + format), output size (+-5%), and
whether the original remains. Known/intended differences are reported as EXPECTED, not failures.
Exit status is 1 only for unexplained mismatches (UNEXPECTED).

Safety: IMGCOMPRESS_CONFIG / IMGCOMPRESS_HISTORY point into the scratch dir, --no-history is passed, and
originals are removed with `--originals delete` (legacy --deleteOnConvert equivalent) so nothing goes to ~/.Trash.

Usage: scripts/parity.py [--bin PATH] [--presets a,b] [--keep] [--workdir DIR] [--tolerance 0.05]
"""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEGACY = ROOT / "legacy" / "compress.py"
FIXTURES = ROOT / "fixtures"

# preset -> (legacy flags, target ext normalisation)
PRESETS = {
    "lossless": ([], None),
    "lossless-zopfli": (["--zopfli"], None),
    "lossy": (["--lossy"], None),
    "lossy-zopfli": (["--lossy", "--zopfli"], None),
    "jpg": (["--jpg", "--deleteOnConvert"], "jpg"),
    "png": (["--png", "--deleteOnConvert"], "png"),
    "heif": (["--heif", "--deleteOnConvert"], "heic"),
    "avif": (["--avif", "--deleteOnConvert"], "avif"),
    "jxl": (["--jxl", "--deleteOnConvert"], "jxl"),
}
LEGACY_EXTS = {".jpg", ".jpeg", ".png"}
# Fixtures that legacy cannot handle sanely (it aborts the whole asyncio.gather on one error) or ignores by design.
SKIP_COPY = {"corrupt_truncated.jpg", "zero_byte.jpg", ".hidden_photo.jpg", "notes.txt"}
NORM = {".jpeg": ".jpg", ".heif": ".heic"}


def norm(ext):
    e = ext.lower()
    return NORM.get(e, e)


def copy_fixtures(dst: Path, skip_modern=False):
    # modern/photo.* share one stem, so converting them all collides on photo.<target> (rust refuses to
    # overwrite, legacy ignores them); they are only exercised by the non-conversion presets.
    for src in FIXTURES.rglob("*"):
        if skip_modern and src.relative_to(FIXTURES).parts[0] == "modern":
            continue
        if src.is_file() and src.name not in SKIP_COPY:
            out = dst / src.relative_to(FIXTURES)
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, out)


def snapshot(d: Path):
    """{relative path: size}"""
    return {str(p.relative_to(d)): p.stat().st_size for p in d.rglob("*") if p.is_file()}


def by_stem(files):
    """{(dir, stem): {ext: size}}"""
    out = {}
    for rel, size in files.items():
        p = Path(rel)
        out.setdefault((str(p.parent), p.stem), {})[norm(p.suffix)] = size
    return out


def run(cmd, env, cwd):
    return subprocess.run(cmd, env=env, cwd=cwd, capture_output=True, text=True, timeout=1800)


def compare(preset, target, before, leg, rus, tol):
    """Per original file. Returns [(name, status, note)].

    OK         same file set, size within tolerance
    BETTER     same file set, rust smaller than legacy by more than tolerance (needs a documented reason)
    WORSE      same file set, rust bigger than legacy by more than tolerance (a bug until explained)
    UNEXPECTED file set / formats / surviving originals differ
    EXPECTED   covered by the intended differences (more input types, legacy skips <10KB, ...)
    """
    rows = []
    lstems, rstems = by_stem(leg), by_stem(rus)
    for rel in sorted(before):
        p = Path(rel)
        key, ext = (str(p.parent), p.stem), norm(p.suffix)
        if p.suffix.lower() not in LEGACY_EXTS:
            rows.append((rel, "EXPECTED", f"legacy ignores {p.suffix}; rust -> {sorted(rstems.get(key, {}))}"))
            continue
        lo, ro = lstems.get(key, {}), rstems.get(key, {})
        if before[rel] < 9999:
            rows.append((rel, "EXPECTED", f"legacy skips <10KB inputs (legacy {lo}, rust {ro})"))
            continue
        orig = norm(p.suffix)
        if target and target != orig and set(ro) == {orig} and set(lo) == {"." + target.lstrip(".")}:
            # rust keeps the original when the converted file is not smaller (replace-if smaller);
            # legacy converts unconditionally (e.g. jpg->png is 5x bigger, palette png -> avif/heic is bigger)
            rows.append((rel, "EXPECTED", f"legacy converts without keep-smaller check ({lo}); rust kept original ({ro})"))
            continue
        if set(lo) != set(ro):
            if target and set(ro) == {"." + target.lstrip(".")} and set(lo) == {"." + target.lstrip("."), orig}:
                rows.append((rel, "EXPECTED", f"legacy quirk: cjxl --quiet writes stderr so legacy treats success as failure and keeps the original ({lo}); rust {ro}"))
            else:
                rows.append((rel, "UNEXPECTED", f"file set legacy={lo} rust={ro}"))
            continue
        status, notes = "OK", []
        for e in sorted(lo):
            ratio = ro[e] / lo[e] if lo[e] else 1.0
            notes.append(f"{e}: orig {before[rel]} legacy {lo[e]} rust {ro[e]} ({ratio - 1:+.1%})")
            if target == "heic" and abs(ratio - 1) > tol:
                status = "EXPECTED"  # intended difference: Apple ImageIO HEIC encoder vs heif-enc
            elif target == "avif" and abs(ratio - 1) > tol:
                status = "EXPECTED"  # not like-for-like: cavif defaults vs libaom q60/speed6; rust smaller on large, larger on small files
            elif ratio - 1 > tol:
                status = "WORSE"
            elif 1 - ratio > tol and status == "OK":
                status = "BETTER"
        rows.append((rel, status, "; ".join(notes)))
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=str(ROOT / "target/release/imgcompress"))
    ap.add_argument("--presets", default=",".join(PRESETS))
    ap.add_argument("--keep", action="store_true")
    ap.add_argument("--workdir")
    ap.add_argument("--tolerance", type=float, default=0.05)
    a = ap.parse_args()

    work = Path(a.workdir or tempfile.mkdtemp(prefix="parity-"))
    work.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env["IMGCOMPRESS_CONFIG"] = str(work / "presets.json")
    env["IMGCOMPRESS_HISTORY"] = str(work / "history.db")
    if not Path(a.bin).exists():
        sys.exit(f"missing {a.bin}; run `make cli` first")

    summary, bad = [], 0
    for preset in a.presets.split(","):
        flags, target = PRESETS[preset]
        L, R = work / preset / "legacy", work / preset / "rust"
        for d in (L, R):
            shutil.rmtree(d, ignore_errors=True)
            copy_fixtures(d, skip_modern=bool(target))
        before = snapshot(R)
        lp = run(["uv", "run", str(LEGACY), "-q", *flags, "--parallelism", "4", str(L)], env, ROOT / "legacy")
        rflags = ["--originals", "delete"] if target else []
        rp = run([a.bin, "run", "--preset", preset, "--no-history", *rflags, str(R)], env, ROOT)
        rows = compare(preset, target, before, snapshot(L), snapshot(R), a.tolerance)
        counts = {}
        for _, s, _ in rows:
            counts[s] = counts.get(s, 0) + 1
        summary.append((preset, counts, lp.returncode, rp.returncode))
        print(f"\n=== {preset}  (legacy exit {lp.returncode}, rust exit {rp.returncode})")
        for f, s, n in rows:
            print(f"  {s:10} {f}: {n}")
        if lp.returncode:
            print("  legacy stderr:", lp.stderr.strip()[-400:])
        if rp.returncode:
            print("  rust stderr:", rp.stderr.strip()[-400:])
        bad += counts.get("UNEXPECTED", 0) + counts.get("WORSE", 0)

    print("\n=== summary")
    print(f"{'preset':18} {'OK':>3} {'BETTER':>7} {'WORSE':>6} {'UNEXP':>6} {'EXPECTED':>9}  exit(l/r)")
    for p, c, le, re_ in summary:
        print(f"{p:18} {c.get('OK',0):>3} {c.get('BETTER',0):>7} {c.get('WORSE',0):>6} {c.get('UNEXPECTED',0):>6} {c.get('EXPECTED',0):>9}  {le}/{re_}")
    if not a.keep and not a.workdir:
        shutil.rmtree(work, ignore_errors=True)
    else:
        print("workdir:", work)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
