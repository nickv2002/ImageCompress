#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Generate the deterministic synthetic fixture corpus into ./fixtures.

Idempotent: wipes and rebuilds fixtures/ each run. Needs magick, exiftool,
cjxl, avifenc, heif-enc, cwebp, gifsicle, pngquant, ffmpeg, img2webp (cjpegli optional).
Real photos can go in fixtures-local/ (gitignored); this script never touches it.
"""
import os, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "fixtures"
P3 = Path("/System/Library/ColorSync/Profiles/Display P3.icc")
CMYK = Path("/System/Library/ColorSync/Profiles/Generic CMYK Profile.icc")
SEED = "1234"
FONT = next((f for f in ("/System/Library/Fonts/Supplemental/Arial.ttf", "/System/Library/Fonts/Helvetica.ttc", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") if Path(f).exists()), "")
missing = []


def have(t):
    ok = shutil.which(t) is not None
    if not ok and t not in missing:
        missing.append(t)
    return ok


def run(*a, **kw):
    r = subprocess.run([str(x) for x in a], capture_output=True, text=True, **kw)
    if r.returncode != 0:
        sys.exit(f"FAILED: {' '.join(map(str, a))}\n{r.stderr}")
    return r


def magick(*a):
    return run("magick", "-seed", SEED, *a,
               # strip timestamps for determinism
               ) if False else run("magick", "-seed", SEED, *a)


PNG_DET = ["-define", "png:exclude-chunks=date,time", "+set", "date:create", "+set", "date:modify", "+set", "date:timestamp"]


def photo(path, w, h, seed):
    """Plasma + gaussian noise + shapes + text: photo-ish, incompressible-ish."""
    run("magick", "-seed", seed, "-size", f"{w}x{h}", "plasma:fractal", "-blur", "0x1.2",
        "-attenuate", "0.35", "+noise", "Gaussian",
        "-fill", "rgba(255,200,40,0.55)", "-draw", f"circle {w*0.3},{h*0.4} {w*0.3},{h*0.4+h*0.2}",
        "-fill", "rgba(20,40,120,0.5)", "-draw", f"rectangle {w*0.55},{h*0.15} {w*0.9},{h*0.5}",
        "-font", FONT, "-fill", "white", "-stroke", "black", "-strokewidth", "1", "-pointsize", str(h // 10),
        "-gravity", "south", "-annotate", "+0+20", "ImageCompress fixture", "+repage",
        "-depth", "8", str(path))


def flat(path, w, h):
    run("magick", "-size", f"{w}x{h}", "xc:#f4efe6", "-fill", "#e4572e", "-draw", f"circle {w*0.3},{h*0.5} {w*0.3},{h*0.85}",
        "-fill", "#29335c", "-draw", f"rectangle {w*0.5},{h*0.2} {w*0.9},{h*0.6}",
        "-fill", "#f3a712", "-draw", f"polygon {w*0.1},{h*0.9} {w*0.4},{h*0.55} {w*0.7},{h*0.9}",
        "-font", FONT, "-fill", "#29335c", "-pointsize", str(h // 8), "-gravity", "north", "-annotate", "+0+20", "Flat Illustration",
        "-depth", "8", *PNG_DET, str(path))


def main():
    for t in ("magick", "exiftool", "cjxl", "avifenc", "heif-enc", "cwebp", "gifsicle", "pngquant", "ffmpeg", "img2webp"):
        have(t)
    if missing:
        sys.exit(f"missing required tools: {missing}")
    have("cjpegli")

    if OUT.exists():
        shutil.rmtree(OUT)
    d = {k: OUT / k for k in ("jpeg", "png", "gif", "anim", "modern", "other", "edge")}
    for p in d.values():
        p.mkdir(parents=True)
    work = Path(tempfile.mkdtemp(prefix="imgfix-"))
    try:
        base = work / "base.png"
        photo(base, 1200, 800, SEED)
        small = work / "small.png"
        run("magick", base, "-resize", "800x", *PNG_DET, small)

        # ---- JPEG
        jq = ["-sampling-factor", "4:2:0", "-quality", "92"]
        run("magick", base, *jq, "-interlace", "None", d["jpeg"] / "baseline.jpg")
        run("magick", base, *jq, "-interlace", "Plane", d["jpeg"] / "progressive.jpg")
        run("magick", base, "-profile", CMYK, "-colorspace", "CMYK", "-quality", "90", d["jpeg"] / "cmyk.jpg")
        run("magick", base, "-resize", "1000x", "-quality", "90", d["jpeg"] / "exif_gps_orient6.jpg")
        run("exiftool", "-q", "-overwrite_original", "-n", "-Orientation=6",
            "-GPSLatitude=37.7749", "-GPSLatitudeRef=N", "-GPSLongitude=122.4194", "-GPSLongitudeRef=W",
            "-Make=FixtureCam", "-Model=FX-1", "-DateTimeOriginal=2020:01:02 03:04:05",
            "-ThumbnailImage<=" + str(write_thumb(work, base)), d["jpeg"] / "exif_gps_orient6.jpg")
        run("magick", base, "-profile", P3, "-quality", "90", d["jpeg"] / "display_p3.jpg")

        # ---- PNG
        alpha = work / "alpha_mask.png"
        run("magick", "-size", "800x600", "radial-gradient:white-black", "-evaluate", "multiply", "1.0", alpha)
        run("magick", small, alpha, "-alpha", "off", "-compose", "CopyOpacity", "-composite", *PNG_DET, d["png"] / "rgba_photo.png")
        run("magick", small, "-colors", "256", "-dither", "FloydSteinberg", *PNG_DET, "PNG8:" + str(d["png"] / "palette.png"))
        run("magick", base, "-resize", "600x", "-depth", "16", "-define", "png:color-type=2", *PNG_DET, d["png"] / "rgb16.png")
        flat(d["png"] / "flat_illustration.png", 1000, 700)

        # ---- GIF
        frames = []
        for i in range(8):
            f = work / f"f{i}.png"
            run("magick", "-size", "320x240", "gradient:#204080-#80c0ff", "-rotate", str(i * 12), "-gravity", "center",
                "-crop", "320x240+0+0", "+repage", "-fill", "#ffcc00", "-draw", f"circle {40+i*30},120 {40+i*30},150",
                "-font", FONT, "-fill", "white", "-pointsize", "24", "-annotate", "+0+90", f"frame {i}", f)
            frames.append(f)
        run("magick", "-delay", "10", "-loop", "0", *frames, d["gif"] / "animated.gif")
        run("magick", small, "-resize", "640x", "-colors", "128", d["gif"] / "static.gif")

        # ---- animations (8 frames, 160x120, opaque, loop forever): the same content in every animated format
        af = []
        for i in range(8):
            f = work / f"a{i}.png"
            run("magick", "-size", "160x120", "gradient:#204080-#80c0ff", "-rotate", str(i * 12), "-gravity", "center",
                "-extent", "160x120", "+repage", "-fill", "#ffcc00", "-draw", f"circle {20+i*15},60 {20+i*15},75", "-alpha", "off", f)
            af.append(f)
        run("ffmpeg", "-loglevel", "error", "-y", "-framerate", "10", "-i", work / "a%d.png", "-plays", "0", "-pix_fmt", "rgb24", "-f", "apng", d["anim"] / "animated.png")
        run("img2webp", "-loop", "0", "-lossy", "-q", "75", "-d", "100", *af, "-o", d["anim"] / "animated.webp")
        run("avifenc", "--fps", "10", "-q", "60", "-s", "8", "-j", "1", *af, d["anim"] / "animated.avif")
        run("cjxl", d["anim"] / "animated.png", d["anim"] / "animated.jxl", "--quiet")
        run("heif-enc", "-S", "-q", "50", *af, "-o", d["anim"] / "sequence.heic")

        # ---- modern formats
        run("heif-enc", "-q", "60", base, "-o", d["modern"] / "photo.heic")
        run("avifenc", "-q", "60", "-s", "8", "-j", "1", small, d["modern"] / "photo.avif")
        run("cjxl", small, d["modern"] / "photo.jxl", "-q", "85", "-e", "4", "--num_threads=0", "--quiet")
        run("cwebp", "-quiet", "-q", "80", "-mt" if False else "-m", "4", small, "-o", d["modern"] / "photo.webp")
        run("magick", small, "-compress", "LZW", "-resize", "600x", d["modern"] / "photo.tiff")

        (d["other"] / "logo.svg").write_text(SVG)
        (d["other"] / "icon.svg").write_text(SVG_ICON)
        (d["other"] / "transform_trap.svg").write_text(SVG_TRAP)
        import gzip
        (d["other"] / "icon.svgz").write_bytes(gzip.compress(SVG_ICON.encode(), 9, mtime=0))

        # ---- edge cases
        run("magick", "-size", "64x64", "gradient:red-blue", *PNG_DET, d["edge"] / "tiny_under_10kb.png")
        run("magick", base, "-resize", "500x", "-quality", "85", d["edge"] / ".hidden_photo.jpg")
        for sub, sd in (("a", "11"), ("b", "22")):
            (d["edge"] / "dupes" / sub).mkdir(parents=True)
            photo(work / f"dup{sub}.png", 640, 427, sd)
            run("magick", work / f"dup{sub}.png", "-quality", "88", d["edge"] / "dupes" / sub / "IMG_0001.jpg")
        (d["edge"] / "with spaces").mkdir()
        run("magick", base, "-resize", "700x", "-quality", "88", d["edge"] / "with spaces" / "my holiday photo (1).jpg")
        run("magick", small, "-resize", "400x", *PNG_DET, d["edge"] / "with spaces" / "café 日本語 \U0001F600.png")
        good = d["edge"] / "corrupt_truncated.jpg"
        run("magick", base, "-resize", "800x", "-quality", "90", good)
        data = good.read_bytes()
        good.write_bytes(data[: len(data) // 2])
        (d["edge"] / "zero_byte.jpg").write_bytes(b"")
        (d["edge"] / "notes.txt").write_text("not an image\n")
    finally:
        shutil.rmtree(work, ignore_errors=True)

    # deterministic mtimes
    for p in OUT.rglob("*"):
        os.utime(p, (1577934245, 1577934245))
    total = 0
    for p in sorted(OUT.rglob("*")):
        if p.is_file():
            s = p.stat().st_size
            total += s
            print(f"{s:>10}  {p.relative_to(OUT)}")
    print(f"{total:>10}  TOTAL ({total/1e6:.1f} MB)")
    if "cjpegli" in missing:
        print("note: cjpegli missing (only needed by bench.py)")


def write_thumb(work, base):
    t = work / "thumb.jpg"
    run("magick", base, "-resize", "160x", "-quality", "70", t)
    return t


SVG = """<?xml version="1.0" encoding="UTF-8"?>
<!-- Generated by make_fixtures.py: deliberately verbose so SVG optimizers have work to do -->
<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="400.000000" height="300.000000" viewBox="0 0 400 300">
  <metadata><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description>fixture</rdf:Description></rdf:RDF></metadata>
  <defs>
    <linearGradient id="g1" x1="0.000000" y1="0.000000" x2="1.000000" y2="1.000000">
      <stop offset="0.000000" stop-color="#e4572e"/>
      <stop offset="1.000000" stop-color="#29335c"/>
    </linearGradient>
  </defs>
  <rect x="0.000000" y="0.000000" width="400.000000" height="300.000000" fill="url(#g1)"/>
  <circle cx="120.123456" cy="150.654321" r="60.000000" fill="#f3a712" stroke="#000000" stroke-width="2.000000"/>
  <path d="M 220.000000 80.000000 L 340.000000 80.000000 L 280.000000 220.000000 Z" fill="#f4efe6" opacity="0.900000"/>
  <text x="20.000000" y="280.000000" font-family="sans-serif" font-size="24.000000" fill="#ffffff">ImageCompress</text>
</svg>
"""

# No <text>, so the render guard is active; ids are referenced from outside the file.
SVG_ICON = """<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<!-- Created with Inkscape (http://www.inkscape.org/) -->
<svg xmlns="http://www.w3.org/2000/svg" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="48.000000" height="48.000000" viewBox="0 0 48 48" id="icon-root">
  <sodipodi:namedview id="base" pagecolor="#ffffff" inkscape:zoom="1.000000"/>
  <defs id="defs1"><linearGradient id="sheen" x1="0.000000" y1="0.000000" x2="0.000000" y2="1.000000"><stop offset="0.000000" stop-color="#ffffff" stop-opacity="0.800000"/><stop offset="1.000000" stop-color="#ffffff" stop-opacity="0.000000"/></linearGradient></defs>
  <g id="layer1" transform="translate(0.000000,0.000000)">
    <circle id="disc" cx="24.000000" cy="24.000000" r="20.000000" fill="#3465a4" stroke="#204a87" stroke-width="2.000000"/>
    <path id="arrow" d="M 14.000000 24.000000 L 34.000000 24.000000 L 28.000000 18.000000 M 34.000000 24.000000 L 28.000000 30.000000" fill="none" stroke="#ffffff" stroke-width="3.000000" stroke-linecap="round" stroke-linejoin="round"/>
    <ellipse id="shine" cx="24.000000" cy="14.000000" rx="14.000000" ry="8.000000" fill="url(#sheen)"/>
  </g>
</svg>
"""

# A rotation about a huge centre (cancelled by the inner translate so the art stays on canvas): SVGO's convertTransform/applyTransforms round it badly (the render moves).
SVG_TRAP = """<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200">
  <g transform="rotate(0.048 -3525290.8 -2390056)"><g transform="translate(2001.135046 -2954.262896)">
    <rect x="20.000000" y="20.000000" width="160.000000" height="160.000000" fill="#c0392b"/>
    <path d="M 40.000000 160.000000 L 100.000000 40.000000 L 160.000000 160.000000 Z" fill="#f1c40f" stroke="#000000" stroke-width="4.000000"/>
  </g></g>
</svg>
"""

if __name__ == "__main__":
    main()
