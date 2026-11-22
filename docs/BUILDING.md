# Building

## Prerequisites (macOS)

```sh
brew install rustup cmake nasm meson ninja && rustup-init
rustup target add aarch64-apple-darwin x86_64-apple-darwin
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
```

Xcode command line tools are required. `nasm` is needed for the x86_64 slice (libaom, dav1d, mozjpeg SIMD); `meson` + `ninja` build dav1d; `cmake` + `ninja` build libaom, libavif, libjxl and jpegli.

## Universal CLI

```sh
scripts/build_cli_mac.sh      # build/cli/imgcompress + build/helpers/gifsicle
scripts/check_cli_mac.sh      # verification
```

- Builds `imgcompress` with `cargo build --release --locked` for `aarch64-apple-darwin` and `x86_64-apple-darwin` (`MACOSX_DEPLOYMENT_TARGET=15.0`) and joins them with `lipo -create`. Set `CARGO_TARGET_DIR` to keep intermediates elsewhere.
- Cmake-based `-sys` crates get `CMAKE_OSX_ARCHITECTURES` from the environment (cmake reads it as the initial cache value), which is how jpegli-sys, libaom and libavif follow the target.
- `libdav1d-sys` runs `meson setup` natively and has no macOS cross file, so the script puts a `meson` shim on `PATH` that injects a cross file (`clang -arch <arch>`, `cpu_family`) for the target. Without it the x86_64 link fails with undefined `_dav1d_*` symbols (arm64 objects).
- `scripts/build_gifsicle_mac.sh` downloads `gifsicle-$GIFSICLE_VERSION.tar.gz` from lcdf.org over HTTPS, verifies a pinned SHA-256 (upstream publishes none, so the pin is trust-on-first-fetch), runs `configure && make` once per arch with `-arch` flags, and `lipo`s the results. Output: `build/helpers/gifsicle`, `gifsicle.version`, `gifsicle.COPYING` (GPL-2.0-only). gifsicle is always a separate executable, never linked. `FORCE=1` rebuilds.
- Both scripts fail if anything but `/usr/lib` or `/System` is linked.

## Verification (`check_cli_mac.sh`)

1. Both binaries are universal, the CLI has `minos 15.0` in both slices, and only system libraries are linked.
2. A mock bundle (`Mock.app/Contents/MacOS/imgcompress` + `Contents/Helpers/gifsicle`) runs `imgcompress tools --check` under `env -i PATH=/usr/bin:/bin` and must resolve gifsicle via `../Helpers`; a flat layout (gifsicle next to the exe) is checked too.
3. Fixtures run through the arm64 slice and the x86_64 slice (Rosetta, `arch -x86_64`; `SKIP_X86=1` to skip). JPEG (mozjpeg), oxipng, WebP and gifsicle output must be byte-identical. Float-SIMD encoders (imagequant, libaom, libjxl) produce slightly different bytes between NEON and SSE/AVX, so they only have to land within 5% in size.

## gifsicle lookup

`compress_core::gif_codec::find_gifsicle` searches, in order: `<exe dir>/gifsicle`, `<exe dir>/../Helpers/gifsicle`, `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`, then `PATH`. The app bundle layout `Contents/MacOS/imgcompress` + `Contents/Helpers/gifsicle` needs no change. Note the Homebrew fallbacks mean a developer machine can satisfy the lookup without the bundled copy; `tools --check` prints the resolved path, which is what the check script asserts on.

## Other targets

Linux (`scripts/build_linux.sh`) and releases (`scripts/release.sh`) are described in the implementation plan.
