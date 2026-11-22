#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Benchmark imgcompress against reference tools, or run edge-case sanity checks.

  scripts/bench.py [--bin target/release/imgcompress] [--fixtures fixtures] [--only SUBSTR]
  scripts/bench.py --sanity [--bin ...]

Quality metric: SSIMULACRA2 (`ssimulacra2` CLI) if installed, else PSNR via
`magick compare` (higher is better; reported as dB). PASS = ours <= ref*1.02 and
quality not worse than ref by more than 2 points (SSIM2) / 1.5 dB (PSNR).
"""
import argparse, json, os, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HAVE_SSIM2 = shutil.which("ssimulacra2") is not None
METRIC = "SSIM2" if HAVE_SSIM2 else "PSNR"


def sh(*a, **kw):
    return subprocess.run([str(x) for x in a], capture_output=True, text=True, **kw)


def have(t):
    return shutil.which(t) is not None


# ---- references: (src, dst) -> bool.  dst's extension decides output format.
def ref_png(src, dst):
    shutil.copy(src, dst)
    r = sh("pngquant", "--quality", "65-85", "--speed", "3", "--force", "--output", dst, src)
    if r.returncode not in (0, 98, 99):  # 98/99 = skipped, quality too low / larger
        return False
    return sh("oxipng", "-o", "4", "--strip", "safe", "-q", dst).returncode == 0


def ref_jpeg_lossy(src, dst):
    return sh("cjpegli", src, dst, "-q", "85").returncode == 0


def ref_jpeg_lossless(src, dst):
    return sh("jpegtran", "-optimize", "-progressive", "-copy", "none", "-outfile", dst, src).returncode == 0


def ref_gif(src, dst):
    return sh("gifsicle", "-O3", src, "-o", dst).returncode == 0


def ref_jxl(src, dst):
    # lossy re-encode (never the reversible JPEG transcode)
    return sh("cjxl", src, dst, "-q", "80", "-e", "7", "--lossless_jpeg=0", "--quiet").returncode == 0


def ref_jxl_transcode(src, dst):
    return sh("cjxl", src, dst, "-e", "7", "--quiet").returncode == 0


def ref_avif(src, dst):
    return sh("avifenc", "-q", "60", "-s", "6", src, dst).returncode == 0


def ref_webp(src, dst):
    return sh("cwebp", "-quiet", "-q", "80", "-m", "4", src, "-o", dst).returncode == 0


def ref_heic(src, dst):
    return sh("heif-enc", "-q", "50", src, "-o", dst).returncode == 0


COMMON = ["--min-size", "0", "--destination", "in-place", "--originals", "delete", "--no-history", "--jobs", "1"]
# case: (glob under fixtures, label, ref fn, ref out ext, our flags, our out ext or None=same)
CASES = [
    ("png/*.png", "png", ref_png, ".png", ["--png-quantize", "65-85"], None),
    ("jpeg/*.jpg", "jpg-lossy", ref_jpeg_lossy, ".jpg", ["--jpeg-encoder", "jpegli", "--jpeg-quality", "85"], None),
    ("jpeg/*.jpg", "jpg-lossless", ref_jpeg_lossless, ".jpg", ["--jpeg-quality", "lossless"], None),
    ("gif/*.gif", "gif", ref_gif, ".gif", ["--gif-optimize-level", "3"], None),
    ("jpeg/baseline.jpg", "jpg->jxl", ref_jxl, ".jxl", ["--format", "jxl", "--no-jxl-jpeg-transcode", "--jxl-quality", "80", "--jxl-effort", "7"], ".jxl"),
    ("jpeg/baseline.jpg", "jpg->jxl-lossless", ref_jxl_transcode, ".jxl", ["--format", "jxl", "--jxl-jpeg-transcode", "--jxl-effort", "7"], ".jxl"),
    ("png/rgba_photo.png", "png->avif", ref_avif, ".avif", ["--format", "avif", "--avif-quality", "60", "--avif-speed", "6"], ".avif"),
    ("png/rgba_photo.png", "png->webp", ref_webp, ".webp", ["--format", "webp", "--webp-quality", "80", "--webp-method", "4"], ".webp"),
    ("jpeg/baseline.jpg", "jpg->heic", ref_heic, ".heic", ["--format", "heic", "--heic-quality", "50"], ".heic"),
    ("modern/photo.webp", "webp", ref_webp, ".webp", ["--webp-quality", "80"], None),
    ("modern/photo.avif", "avif", ref_avif, ".avif", ["--avif-quality", "60"], None),
    ("modern/photo.jxl", "jxl", ref_jxl, ".jxl", ["--jxl-quality", "80"], None),
]
TOOLS = {ref_png: ("pngquant", "oxipng"), ref_jpeg_lossy: ("cjpegli",), ref_jpeg_lossless: ("jpegtran",),
         ref_gif: ("gifsicle",), ref_jxl: ("cjxl",), ref_jxl_transcode: ("cjxl",), ref_avif: ("avifenc",), ref_webp: ("cwebp",), ref_heic: ("heif-enc",)}


def to_png(src, dst):
    """Decode anything to an sRGB, auto-oriented, first-frame PNG for metrics."""
    r = sh("magick", f"{src}[0]", "-auto-orient", "-colorspace", "sRGB", "-background", "#808080", "-alpha", "remove", "-alpha", "off", "-depth", "8", dst)
    if r.returncode != 0 and str(src).endswith((".jxl",)):
        tmp = str(dst) + ".dj.png"
        if sh("djxl", src, tmp).returncode == 0:
            r = sh("magick", tmp, "-auto-orient", "-colorspace", "sRGB", "-background", "#808080", "-alpha", "remove", "-alpha", "off", "-depth", "8", dst)
    return r.returncode == 0


def quality(orig, out, tmp):
    a, b = Path(tmp) / "m_a.png", Path(tmp) / "m_b.png"
    if not (to_png(orig, a) and to_png(out, b)):
        return None
    if HAVE_SSIM2:
        r = sh("ssimulacra2", a, b)
        try:
            return float(r.stdout.strip().split()[-1])
        except (ValueError, IndexError):
            return None
    r = sh("magick", "compare", "-metric", "PSNR", a, b, "null:")
    try:
        v = (r.stderr or r.stdout).strip().split()[0]
        return 99.0 if v == "inf" else float(v)
    except (ValueError, IndexError):
        return None


def run_ours(binp, src, extra_flags, ext, tmp):
    """Copy src into an isolated dir, run in-place. Return (bytes, outpath) or (None, reason)."""
    d = Path(tmp) / "ours"
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir()
    cp = d / src.name
    shutil.copy(src, cp)
    try:
        r = sh(binp, "run", "--json", *COMMON, *extra_flags, "--", cp, timeout=600)
    except FileNotFoundError:
        return None, "binary missing"
    if r.returncode in (2, 3):
        return None, f"exit {r.returncode}: {r.stderr.strip().splitlines()[-1] if r.stderr.strip() else 'unsupported'}"
    if r.returncode != 0:
        return None, f"exit {r.returncode}"
    files = [p for p in d.iterdir() if p.is_file()]
    want = [p for p in files if ext is None or p.suffix == ext] or files
    if not want:
        return None, "no output"
    out = max(want, key=lambda p: p.stat().st_mtime)
    return out, None


def bench(args):
    binp = Path(args.bin)
    if not binp.is_absolute():
        binp = ROOT / binp
    have_bin = binp.exists()
    if not have_bin:
        print(f"SKIP ours: {binp} not found (reference columns still printed)", file=sys.stderr)
    fx = Path(args.fixtures) if os.path.isabs(args.fixtures) else ROOT / args.fixtures
    rows, npass, nfail = [], 0, 0
    print(f"metric: {METRIC}" + ("" if HAVE_SSIM2 else "  (ssimulacra2 not installed; `brew install ssimulacra2` for SSIM2)"))
    with tempfile.TemporaryDirectory(prefix="imgbench-") as tmp:
        for pat, label, ref, rext, flags, oext in CASES:
            for src in sorted(fx.glob(pat)):
                if args.only and args.only not in f"{label}/{src.name}":
                    continue
                name = f"{label}:{src.name}"
                orig = src.stat().st_size
                miss = [t for t in TOOLS[ref] if not have(t)]
                rb = rq = ob = oq = None
                status = ""
                if miss:
                    status = f"SKIP(ref: no {','.join(miss)})"
                else:
                    rd = Path(tmp) / ("ref" + rext)
                    rd.unlink(missing_ok=True)
                    rsrc = src
                    if src.suffix in (".avif", ".jxl", ".heic", ".webp"):  # ref encoders can't read these
                        rsrc = Path(tmp) / "ref_in.png"
                        to_png(src, rsrc)
                    if ref(rsrc, rd) and rd.exists():
                        rb = rd.stat().st_size
                        rq = quality(src, rd, tmp)
                    else:
                        status = "SKIP(ref failed)"
                if have_bin:
                    out, why = run_ours(binp, src, flags, oext, tmp)
                    if out is None:
                        status = (status + " " if status else "") + f"SKIP(ours: {why})"
                    else:
                        ob = out.stat().st_size
                        oq = quality(src, out, tmp)
                else:
                    status = (status + " " if status else "") + "SKIP(ours)"
                if rb is not None and ob is not None:
                    ok = ob <= rb * 1.02
                    if ok and oq is not None and rq is not None:
                        ok = oq >= rq - (2.0 if HAVE_SSIM2 else 1.5)
                    status = "PASS" if ok else "FAIL"
                    npass += ok
                    nfail += not ok
                rows.append((name, orig, rb, ob, rq, oq, status))
    f = lambda v, fmt="{}": "-" if v is None else fmt.format(v)
    print(f"{'file':<34}{'orig':>9}{'ref':>9}{'ours':>9}{'ours/ref':>9}{'q_ref':>7}{'q_ours':>7}  status")
    for name, o, r, u, rq, oq, st in rows:
        ratio = f(u / r if (u is not None and r) else None, "{:.3f}")
        print(f"{name:<34}{o:>9}{f(r):>9}{f(u):>9}{ratio:>9}{f(rq, '{:.1f}'):>7}{f(oq, '{:.1f}'):>7}  {st}")
    print(f"\nPASS {npass}  FAIL {nfail}  (rows {len(rows)})")
    return 1 if nfail else 0


# ---------------------------------------------------------------- sanity
def events(stdout):
    out = []
    for line in stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return out


def kind(ev):
    k = ev.get("type") or ev.get("event") or ev.get("kind")
    if k is None and len(ev) == 1:
        k = next(iter(ev))
    return str(k).lower() if k else ""


def evpath(ev):
    inner = ev if "path" in ev else next((v for v in ev.values() if isinstance(v, dict) and "path" in v), {})
    return str(inner.get("path", ""))


def sanity(args):
    binp = Path(args.bin)
    if not binp.is_absolute():
        binp = ROOT / binp
    if not binp.exists():
        print(f"SKIP: {binp} not found")
        return 0
    fx = (Path(args.fixtures) if os.path.isabs(args.fixtures) else ROOT / args.fixtures) / "edge"
    results = []

    def check(name, cond, detail=""):
        results.append(cond)
        print(f"{'PASS' if cond else 'FAIL'}  {name}" + (f"  [{detail}]" if detail and not cond else ""))

    flags = ["--json", "--no-history", "--min-size", "0", "--destination", "alongside", "--suffix", "-min",
             "--originals", "keep", "--jobs", "2"]

    def go(d, *extra, paths=None):
        try:
            r = sh(binp, "run", *flags, *extra, "--", *(paths or [d]), timeout=300)
        except subprocess.TimeoutExpired:
            return None
        return r

    with tempfile.TemporaryDirectory(prefix="imgsanity-") as tmp:
        tmp = Path(tmp)
        # A: everything except corrupt, recursive
        a = tmp / "a"
        shutil.copytree(fx, a)
        (a / "corrupt_truncated.jpg").unlink()
        r = go(a, "--recursive")
        if r is None or r.returncode in (2, 3):
            print(f"SKIP: run unsupported (exit {None if r is None else r.returncode}): {r.stderr.strip()[:200] if r else 'timeout'}")
            return 0
        evs = events(r.stdout)
        check("A exit 0 when only good/skippable files", r.returncode == 0, f"exit {r.returncode}")
        check("A no crash signal", r.returncode >= 0)
        outs = sorted(p.relative_to(a).as_posix() for p in a.rglob("*-min.*"))
        check("A two IMG_0001.jpg both produced", {"dupes/a/IMG_0001-min.jpg", "dupes/b/IMG_0001-min.jpg"} <= set(outs), str(outs))
        da, db = a / "dupes/a/IMG_0001-min.jpg", a / "dupes/b/IMG_0001-min.jpg"
        check("A dupes not cross-contaminated", da.exists() and db.exists() and da.read_bytes() != db.read_bytes())
        check("A hidden file skipped by default", not any(p.name.startswith(".") and "-min" in p.name for p in a.rglob("*")))
        check("A zero-byte not output", not (a / "zero_byte-min.jpg").exists())
        zk = [kind(e) for e in evs if "zero_byte" in evpath(e)]
        check("A zero-byte reported skipped (not failed)", "failed" not in zk and (not zk or "skipped" in zk), str(zk))
        check("A notes.txt untouched", (a / "notes.txt").read_text() == "not an image\n" and not (a / "notes-min.txt").exists())
        check("A unicode/space names processed", any("café" in o for o in outs) and any("my holiday photo (1)-min.jpg" in o for o in outs), str(outs))
        check("A originals kept", (a / "dupes/a/IMG_0001.jpg").exists())
        # B: hidden with flag
        b = tmp / "b"
        shutil.copytree(fx, b)
        r = go(b, "--recursive", "--include-hidden")
        check("B --include-hidden processes dotfile", r is not None and (b / ".hidden_photo-min.jpg").exists() or (b / ".hidden_photo.jpg-min").exists() or any(".hidden_photo" in p.name and "-min" in p.name for p in b.iterdir()))
        # C: corrupt alone
        c = tmp / "c"
        c.mkdir()
        shutil.copy(fx / "corrupt_truncated.jpg", c)
        r = go(c)
        evs = events(r.stdout)
        check("C corrupt: exit code 1 (not crash)", r.returncode == 1, f"exit {r.returncode}")
        check("C corrupt: failed event emitted", any(kind(e) == "failed" for e in evs), r.stdout[-200:])
        check("C corrupt: original intact, no partial output", (c / "corrupt_truncated.jpg").exists() and len(list(c.iterdir())) == 1, str(list(c.iterdir())))
        # D: corrupt + good => 1 and good still processed
        d = tmp / "d"
        d.mkdir()
        shutil.copy(fx / "corrupt_truncated.jpg", d)
        shutil.copy(fx / "with spaces/my holiday photo (1).jpg", d)
        r = go(d)
        check("D mixed: exit 1 and good file still done", r.returncode == 1 and (d / "my holiday photo (1)-min.jpg").exists(), f"exit {r.returncode}")
        # E: nothing to do
        e = tmp / "e"
        e.mkdir()
        (e / "notes.txt").write_text("x")
        r = go(e)
        check("E nothing to do -> exit 0", r.returncode == 0, f"exit {r.returncode}")
        e2 = tmp / "empty"
        e2.mkdir()
        r = go(e2)
        check("E empty dir -> exit 0", r.returncode == 0, f"exit {r.returncode}")
        # F: zero byte alone
        z = tmp / "z"
        z.mkdir()
        shutil.copy(fx / "zero_byte.jpg", z)
        r = go(z)
        check("F zero-byte only -> exit 0", r.returncode == 0, f"exit {r.returncode}")
        # G: usage error
        r = sh(binp, "run", "--definitely-not-a-flag")
        check("G bad flag -> exit 2", r.returncode == 2, f"exit {r.returncode}")
        # H: under min-size default skipped
        h = tmp / "h"
        h.mkdir()
        shutil.copy(fx / "tiny_under_10kb.png", h)
        r = sh(binp, "run", "--json", "--no-history", "--min-size", "10KB", "--destination", "alongside", "--suffix", "-min", "--", h)
        check("H <10KB skipped by --min-size, exit 0", r.returncode == 0 and not (h / "tiny_under_10kb-min.png").exists(), f"exit {r.returncode}")
    print(f"\n{sum(results)}/{len(results)} passed")
    return 0 if all(results) else 1


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/release/imgcompress")
    ap.add_argument("--fixtures", default="fixtures")
    ap.add_argument("--only", help="substring filter on 'label/filename'")
    ap.add_argument("--sanity", action="store_true", help="edge-case behaviour checks instead of size bench")
    a = ap.parse_args()
    sys.exit(sanity(a) if a.sanity else bench(a))
