#!/bin/bash
# Build the universal macOS imgcompress CLI (+ gifsicle helper) into build/cli and build/helpers.
# Prereqs: brew install rustup cmake nasm meson ninja; rustup target add aarch64-apple-darwin x86_64-apple-darwin
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
export PATH="/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-15.0}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
OUT="$ROOT/build/cli"
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

for t in cmake ninja nasm meson cargo rustup; do command -v "$t" >/dev/null || { echo "missing tool: $t (see docs/BUILDING.md)" >&2; exit 1; }; done
installed=$(rustup target list --installed)
for t in "${TARGETS[@]}"; do grep -qx "$t" <<<"$installed" || rustup target add "$t"; done

# libdav1d-sys runs `meson setup` natively and only knows cross files for Linux/Windows/wasm, so an x86_64 build on
# an arm64 host would compile arm64 objects. A `meson` shim on PATH injects a macOS cross file for the target arch.
SHIM_DIR=$(mktemp -d); trap 'rm -rf "$SHIM_DIR"' EXIT
REAL_MESON=$(command -v meson)
make_meson_shim() {  # $1 = clang arch (arm64|x86_64), $2 = meson cpu_family
  cat > "$SHIM_DIR/cross.meson" <<CROSS
[binaries]
c = ['clang', '-arch', '$1']
cpp = ['clang++', '-arch', '$1']
ar = 'ar'
strip = 'strip'
nasm = 'nasm'
[host_machine]
system = 'darwin'
subsystem = 'macos'
cpu_family = '$2'
cpu = '$2'
endian = 'little'
[built-in options]
c_args = ['-arch', '$1', '-mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET']
c_link_args = ['-arch', '$1', '-mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET']
CROSS
  cat > "$SHIM_DIR/meson" <<SHIM
#!/bin/bash
if [[ "\$1" == setup ]]; then shift; exec "$REAL_MESON" setup --cross-file "$SHIM_DIR/cross.meson" "\$@"; fi
exec "$REAL_MESON" "\$@"
SHIM
  chmod +x "$SHIM_DIR/meson"
}

for t in "${TARGETS[@]}"; do
  arch=${t%%-*}; [[ $arch == aarch64 ]] && cmake_arch=arm64 || cmake_arch=$arch
  echo "==> cargo build --release --target $t"
  make_meson_shim "$cmake_arch" "$arch"
  # CMAKE_OSX_ARCHITECTURES: cmake-based -sys crates (libaom, libavif, jpegli, ...) otherwise build for the host arch.
  PATH="$SHIM_DIR:$PATH" \
  CMAKE_OSX_ARCHITECTURES="$cmake_arch" \
  CMAKE_OSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
    cargo build --release --locked -p imgcompress --target "$t"
done

mkdir -p "$OUT"
lipo -create -output "$OUT/imgcompress" \
  "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/imgcompress" \
  "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/imgcompress"
lipo -info "$OUT/imgcompress"

"$ROOT/scripts/build_gifsicle_mac.sh"

# Only system libraries may be linked.
fail=0
for bin in "$OUT/imgcompress" "$ROOT/build/helpers/gifsicle"; do
  bad=$(otool -L "$bin" | grep -E '^\s' | awk '{print $1}' | grep -vE '^(/usr/lib/|/System/)' || true)
  if [[ -n "$bad" ]]; then echo "non-system libraries linked in $bin:" >&2; echo "$bad" >&2; fail=1; fi
done
[[ $fail == 0 ]] || exit 1
echo "OK: $(du -h "$OUT/imgcompress" | cut -f1) $OUT/imgcompress"
