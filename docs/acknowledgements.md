---
title: Acknowledgements
description: Licenses, the third-party software ImageCompress is built on, where to get the source code, and thanks.
permalink: /acknowledgements/
---

# Acknowledgements

<p class="lead">ImageCompress is free software, built on the work of many open source projects.</p>

## License

The ImageCompress app and the `imgcompress` command are licensed under the
**GNU General Public License, version 3 or (at your option) any later version** (GPL-3.0-or-later).
They link GPL-licensed libraries (libimagequant and jpegxl-rs), so the whole program is GPL.
The bundled gifsicle (GPL-2.0-only) runs as a separate program and is not linked.

The source code in the [repository](https://github.com/nickv2002/ImageCompress) is also released under
CC0-1.0, which is a public domain dedication.

## Get the source

The complete Corresponding Source of every release is attached to that release on GitHub as
`imgcompress-<version>-sources.tar.gz`. It includes the vendored Rust crates, the vendored jpegli source and the build
scripts. The source of the bundled gifsicle is attached as `gifsicle-<version>-src.tar.gz`.

Download them from the [releases page](https://github.com/nickv2002/ImageCompress/releases).

## Read the license notices

- In the app: Settings → Tools → **Licenses…**
- On the command line: `imgcompress licenses`
- In the folder named `Licenses` inside the app and in every command line download. It also has the full list of
  software the program uses (`THIRD_PARTY_LICENSES.html`).

## Required credits

This software is based in part on the work of the Independent JPEG Group.

ImageCompress is built on these projects, among others:

- **Independent JPEG Group, mozjpeg**: JPEG encoding (IJG, BSD-3-Clause and zlib licenses).
- **libaom, libavif, dav1d, libyuv**: AVIF (BSD-2-Clause or BSD-3-Clause, with the Alliance for Open Media Patent License 1.0).
- **libjxl, jpegli, libwebp**: JPEG XL, better JPEG encoding and WebP (BSD-3-Clause, with Google's Additional IP Rights Grant for patents).
- **libimagequant** (GPL-3.0-or-later, by Kornel Lesiński and others) and **jpegxl-rs** (GPL-3.0-or-later).
- **zopfli, libdeflater, oxipng** and other Rust crates: see `THIRD_PARTY_LICENSES.html`.
- **oxvg**, **resvg**, **usvg** and **tiny-skia**: SVG optimising and checking (MIT, Apache-2.0 or BSD-3-Clause).
  oxvg uses lightningcss, cssparser and selectors, which are MPL-2.0.
- **gifsicle**: GIF optimising (GPL-2.0-only), bundled as a separate program.
- **Sparkle**: the Mac app updater (MIT, with BSD-2-Clause, MIT and zlib parts).
- **musl libc, LLVM libunwind and the Rust standard library**: in the static Linux builds.
- On macOS, HEIC uses Apple's ImageIO.

## Patents

AV1 (libaom, dav1d), WebP and JPEG XL come with their royalty-free patent grants, which are included with the notices.
An optional Linux `heic` build feature (x265, libde265) may require HEVC patent licenses in some countries. It is off by default
and not part of the macOS builds, which use Apple's ImageIO. This project grants no patent license.

## Thanks

Linux package hosting is generously provided by [Cloudsmith](https://cloudsmith.com).
