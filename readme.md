# ImageCompress

Compress and convert images (PNG, JPEG, WebP, GIF, AVIF, JPEG XL, HEIC) from the command line on macOS and Linux, or from a native macOS app with drag and drop and a Finder right-click menu.

The engine is a single Rust library (`compress-core`) with every codec embedded, so you need neither Homebrew nor Python:

- `imgcompress`: the CLI (macOS universal, static Linux musl builds for x86_64 and aarch64);
- `ImageCompress.app`: a SwiftUI app (macOS 15+) that drives the same engine, with a Finder Sync submenu, Settings for every encoder option and Sparkle updates.

## Install

**macOS app** (includes the `imgcompress` command; updates itself):

```bash
brew tap nickv2002/tap
brew trust --cask nickv2002/tap/imagecompress
brew install --cask imagecompress
```

**CLI only**, on macOS or Linux: `brew trust --formula nickv2002/tap/imgcompress && brew install nickv2002/tap/imgcompress`.

**Debian, Ubuntu, Fedora and openSUSE**, from the signed package repository at [nickv2002.github.io/packages](https://nickv2002.github.io/packages/)
(updates with `apt upgrade` / `dnf upgrade` / `zypper update`; the binary is static, so there are no dependencies):

```bash
# Debian / Ubuntu
sudo install -d -m 0755 /etc/apt/keyrings
sudo curl -fsSLo /etc/apt/keyrings/nickv2002.asc https://nickv2002.github.io/packages/nickv2002.asc
echo "deb [signed-by=/etc/apt/keyrings/nickv2002.asc] https://nickv2002.github.io/packages/deb stable main" \
  | sudo tee /etc/apt/sources.list.d/nickv2002.list
sudo apt update && sudo apt install imgcompress

# Fedora / RHEL / Rocky / Alma
sudo curl -fsSLo /etc/yum.repos.d/nickv2002.repo https://nickv2002.github.io/packages/rpm/nickv2002.repo
sudo dnf install imgcompress

# openSUSE
sudo zypper addrepo https://nickv2002.github.io/packages/rpm/nickv2002.repo
sudo zypper install imgcompress
```

To build it yourself, download `imgcompress-<version>-sources.tar.gz` from a release; `BUILD_FROM_SOURCE.txt` inside
has the `cargo` steps.

Or download from the [GitHub releases page](https://github.com/nickv2002/ImageCompress/releases):

| Asset | Contents |
|---|---|
| `ImageCompress-<version>.zip` | Notarized macOS app (includes the CLI) |
| `imgcompress-<version>-macos-universal.tar.gz` | Notarized CLI for scripts and Homebrew |
| `imgcompress-<version>-linux-x86_64.tar.gz`, `...-linux-aarch64.tar.gz` | Fully static Linux CLI |
| `imgcompress_<version>_amd64.deb`, `..._arm64.deb`, `imgcompress-<version>-1.x86_64.rpm`, `...aarch64.rpm` | The same static CLI as Linux packages |
| `imgcompress-<version>-sources.tar.gz` | Complete Corresponding Source (GPL) |
| `gifsicle-<version>-src.tar.gz` | Source of the bundled gifsicle |

Each release lists SHA256 sums for every asset. Versions are CalVer (`YYYY.MM.DD.N`).

## Usage

```bash
# Compress images in place, recursing into folders
imgcompress run --recursive ~/Pictures

# Convert to AVIF and move the originals to the Trash
imgcompress run --format avif --quality 60 --originals trash photo.png

# See every option, the linked encoders, presets and history
imgcompress --help
imgcompress run --help
imgcompress tools
imgcompress options schema
```

### Output names

`--destination` picks where results go: `in-place` (default), `alongside` (adds `--suffix`, default `-min`) or a folder. The name is decided before any work starts, from the sorted input list, so it never depends on which worker finishes first; `--dry-run` prints it (`=> path`).

- **Replacing the input**: when the output path is the input itself (same format, `in-place`, or `alongside` with an empty suffix) the file is replaced by an atomic rename, and the original is never trashed or deleted (it was replaced). Nothing else can take that name.
- **Colliding inputs**: several inputs of one run that would write the same name (`a.png` and `a.avif` with `--format webp --destination alongside --suffix -w` both want `a-w.webp`) never overwrite each other. Inputs whose format changes keep their source extension: `a-w.png.webp`, `a-w.avif.webp`. This is symmetric: neither file is privileged by sorting first. An input whose format does not change keeps the plain name (`a.webp` -> `a-w.webp`). If names still clash (`x/IMG.png`, `y/IMG.png` into one destination folder) the later one in path order gets `-2`, `-3`. Names are compared case-insensitively. Inputs that end up skipped (not smaller, errors) still reserve their name, so names do not change from run to run.
- **Existing files**: a file that is not the input itself is never replaced unless you ask. `--overwrite skip` (default) skips the input and reports `output exists: ...`, leaving the original alone, so re-running a command is safe. `--overwrite rename` writes `name-2.ext`, `name-3.ext`, ... instead. `--overwrite replace` replaces it (but never a file that is itself an input of the same run; that one is renamed). A file that appears while the run is in progress makes that job fail instead of being overwritten.

| Situation | `skip` (default) | `rename` | `replace` |
| --- | --- | --- | --- |
| Output is the input (in-place, same format) | atomic replace | atomic replace | atomic replace |
| Several inputs, same output name | distinct names (extension kept, then `-N`) | same | same |
| Output name is an existing file | input skipped, original kept | `-N` | file replaced |
| Output name is another input of the run | input skipped | `-N` | `-N` |

Before this option, an in-place format change onto an existing file failed with an error; it is now a skip (exit code 0), and `alongside` or a folder destination silently overwrote existing files.

`imgcompress licenses` prints the bundled third-party license notices.

## Building from source

Prerequisites (macOS): `brew install rustup cmake nasm meson ninja xcodegen` then `rustup-init`. Linux builds run in Docker.

```bash
make cli            # cargo build --release
make test           # cargo test + Swift tests
make build          # Debug build of the macOS app (xcodegen + xcodebuild)
make licenses       # cargo-deny + third-party notices (needs cargo-deny and cargo-about)
scripts/build_linux.sh <version>   # static musl tarballs via docker/linux-build.Dockerfile
make release-quiet  # full signed + notarized release pipeline (maintainer only)
```

Install the license tools with `cargo install --locked cargo-deny cargo-about --features cli`. Releases are cut with `scripts/release.sh` (see `.claude/skills/release-imagecompress/SKILL.md`): license gate, version bump in `macos/project.yml` and `Cargo.toml`, notarization through 1Password's `op`, Linux tarballs and .deb/.rpm packages (nfpm in Docker), source tarball, Sparkle appcast (`docs/appcast.xml`, served by GitHub Pages), GitHub release, the Homebrew tap (`scripts/update_tap.sh`, templates in `packaging/homebrew/`) and the self-hosted apt/dnf repository (`scripts/sign_linux_packages.sh` and `scripts/publish_linux_packages.sh`, which index, sign and force-push the GitHub Pages site of `nickv2002/packages`; the signing key was made once by `scripts/make_repo_key.sh` and lives in 1Password; local settings such as the 1Password references go in the gitignored `scripts/release.local.env`).

## Legacy Python tool

The original Python implementation (`compress.py`, run through [uv](https://github.com/astral-sh/uv) and Homebrew CLI tools) is kept in `legacy/` until the Rust engine reaches parity, then it will be removed.

## License

The source code in this repository is released under CC0-1.0 (see `LICENSE`). **Binary distributions (the `imgcompress` CLI and the ImageCompress macOS app) are licensed GPL-3.0-or-later; see `COPYING`.** They statically link libimagequant and jpegxl-rs, which are GPL-3.0-or-later. The bundled gifsicle (GPL-2.0-only) runs as a separate process and is not linked.

Third-party notices are in `NOTICE.md`, `THIRD_PARTY_LICENSES.html` and the `Licenses` folder inside the app and in every CLI download. This software is based in part on the work of the Independent JPEG Group.

## Patents

AV1 (libaom, dav1d), WebP and JPEG XL are shipped with their royalty-free patent grants (included with the notices). The optional Linux `heic` build feature (x265, libde265) may require HEVC patent licenses in some jurisdictions; it is off by default and not part of the macOS builds, which use Apple's ImageIO. This project grants no patent license.
