# ImageCompress notices

ImageCompress binaries (the `imgcompress` command-line tool and the ImageCompress macOS app) are licensed under the GNU General Public License, version 3 or (at your option) any later version (GPL-3.0-or-later). The full text is in `COPYING`. The repository's own source code is additionally dedicated to the public domain under CC0-1.0 (`LICENSE`); the combined binaries are GPL-3.0-or-later because they statically link GPL-licensed libraries (libimagequant and jpegxl-rs).

The complete Corresponding Source of every release is attached to that GitHub release as `imgcompress-<version>-sources.tar.gz`. It includes the vendored Rust crates (`cargo vendor`), the vendored jpegli source and the build scripts. Run `imgcompress licenses` to print these notices from the CLI.

## Required attributions

- **Independent JPEG Group**: This software is based in part on the work of the Independent JPEG Group. (mozjpeg, IJG / BSD-3-Clause / zlib.)
- **libaom / libavif / dav1d / libyuv**: BSD-2-Clause or BSD-3-Clause, plus the Alliance for Open Media Patent License 1.0 (`manual/AOM-Patent-License-1.0.txt`).
- **libjxl, jpegli, libwebp**: BSD-3-Clause, with Google's "Additional IP Rights Grant (Patents)" (`PATENTS`, in `vendored/`).
- **libimagequant**: GPL-3.0-or-later, Copyright (c) Kornel Lesiński and others (pngquant `COPYRIGHT` notice in `vendored/`).
- **jpegxl-rs**: GPL-3.0-or-later.
- **zopfli, libdeflater, oxipng and other Rust crates**: see `THIRD_PARTY_LICENSES.html` for every crate, its license and its copyright notices.
- **oxvg (SVG optimiser), resvg/usvg/tiny-skia (SVG render check)**: MIT, Apache-2.0 or BSD-3-Clause. oxvg pulls in lightningcss, cssparser and selectors, which are MPL-2.0 (file-level copyleft, compatible with GPL-3.0-or-later; their source is part of the release source tarball).
- **musl libc, LLVM libunwind, Rust standard library** (Linux static builds): MIT, Apache-2.0 WITH LLVM-exception and MIT OR Apache-2.0 respectively, in `manual/`.
- **gifsicle** (bundled as a separate executable, GPL-2.0-only): not linked; see `manual/gifsicle.txt` and `GPL-2.0.txt`. Its source is published with every release.
- **Sparkle** (macOS app updater): MIT with BSD-2-Clause, MIT and zlib components, see `manual/Sparkle-LICENSE.txt`.
- **Optional Linux `heic` feature** (libheif, libde265: LGPL-3.0-only; x265: GPL-2.0-or-later): see `manual/heic-feature.txt` and `LGPL-3.0.txt`.

## Patents

AV1, WebP and JPEG XL are distributed with their royalty-free patent grants (see `manual/patents-and-notices.txt`). The optional Linux `heic` feature may require HEVC patent licenses in some jurisdictions; it is off by default and not part of the macOS builds, which use Apple's ImageIO. This project grants no patent license.

## Folder contents

| Path | Contents |
|---|---|
| `COPYING` | GPL-3.0 |
| `GPL-2.0.txt` | GPL-2.0 (gifsicle) |
| `LGPL-3.0.txt` | LGPL-3.0 (optional heic feature) |
| `THIRD_PARTY_LICENSES.html` | Licenses of all Rust crates and the C/C++ code bundled in them |
| `manual/` | Hand-maintained notices (above) |
| `vendored/` | NOTICE, PATENTS, COPYING and COPYRIGHT files swept from dependency sources |
