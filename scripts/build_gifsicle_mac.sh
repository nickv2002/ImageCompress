#!/bin/bash
# Build a universal (arm64 + x86_64) gifsicle from source into build/helpers/gifsicle.
# gifsicle is GPL-2.0-only: it is shipped as a separate executable, never linked.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD

VERSION="${GIFSICLE_VERSION:-1.98}"
# No upstream checksum is published; this pins the tarball first fetched over HTTPS from lcdf.org.
SHA256="${GIFSICLE_SHA256:-e9d4cc0816db95c64d32e79f778836db0f1f7eb15868490bd330104e5146b66e}"
URL="https://www.lcdf.org/gifsicle/gifsicle-$VERSION.tar.gz"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-15.0}"

OUT="$ROOT/build/helpers"
WORK="${GIFSICLE_WORK:-$ROOT/build/gifsicle-work}"
TARBALL="$WORK/gifsicle-$VERSION.tar.gz"
mkdir -p "$OUT" "$WORK"

if [[ -x "$OUT/gifsicle" && "$(cat "$OUT/gifsicle.version" 2>/dev/null)" == "$VERSION" && -z "${FORCE:-}" ]]; then
  echo "gifsicle $VERSION already built (FORCE=1 to rebuild)"; exit 0
fi

if [[ ! -f "$TARBALL" ]]; then
  curl --proto '=https' --tlsv1.2 -fsSL -o "$TARBALL" "$URL"
fi
echo "$SHA256  $TARBALL" | shasum -a 256 -c - || { echo "checksum mismatch" >&2; rm -f "$TARBALL"; exit 1; }

for arch in arm64 x86_64; do
  d="$WORK/$arch"
  rm -rf "$d"; mkdir -p "$d"
  tar -xzf "$TARBALL" -C "$d" --strip-components=1
  host=$([[ $arch == arm64 ]] && echo aarch64-apple-darwin || echo x86_64-apple-darwin)
  (
    cd "$d"
    CC="clang -arch $arch" CFLAGS="-O2 -arch $arch" LDFLAGS="-arch $arch" \
      ./configure --host="$host" --disable-gifview --disable-gifdiff --without-x >/dev/null
    make -j"$(sysctl -n hw.ncpu)" src/gifsicle >/dev/null 2>&1 || make -j"$(sysctl -n hw.ncpu)" >/dev/null
  )
done

lipo -create "$WORK/arm64/src/gifsicle" "$WORK/x86_64/src/gifsicle" -output "$OUT/gifsicle"
strip -x "$OUT/gifsicle"
echo "$VERSION" > "$OUT/gifsicle.version"
cp "$WORK/arm64/COPYING" "$OUT/gifsicle.COPYING"
lipo -info "$OUT/gifsicle"
otool -L "$OUT/gifsicle"
