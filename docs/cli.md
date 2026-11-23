---
title: Command line
description: "Reference for the imgcompress command: running, options, presets, history and other commands."
permalink: /cli/
---

# Command line

<p class="applies">Applies to {% include pills.html on="cli" %}</p>

<p class="lead">The <code>imgcompress</code> command compresses images in Terminal or in scripts, on macOS and Linux.</p>

It uses the same engine, the same [presets]({{ site.baseurl }}/presets/) and the same options as the app. The Finder menu, the Services menu, the Before/After view and the automatic 24-hour undo are only in the Mac app. On the command line you can keep your own backups with `--backup-dir`. See [Undo and safety]({{ site.baseurl }}/undo/#undo-on-the-command-line). For installing it, see
[Get started]({{ site.baseurl }}/getting-started/). Run `imgcompress --help` or `imgcompress <command> --help` any time.

## Examples

```bash
# Compress images in place, recursing into folders
imgcompress run --recursive ~/Pictures

# Convert to AVIF and move the originals to the Trash
imgcompress run --format avif --quality 60 --originals trash photo.png

# Use a preset
imgcompress run --preset jxl-visually-lossless ~/Pictures/Holiday

# Write the results next to the originals with -small in the name
imgcompress run --destination alongside --suffix -small --format webp *.png

# See what would happen, without changing anything
imgcompress run --dry-run --format avif ~/Pictures

# See every option, the linked encoders, presets and history
imgcompress --help
imgcompress run --help
imgcompress tools
imgcompress options schema
```

## Commands

| Command | What it does |
| --- | --- |
| `run` | Compress or convert files and folders. |
| `preview` | Encode one file with the chosen options, write it to `--out` and report the size. It leaves the input alone. |
| `presets` | List, show, export, import, delete and reset presets. See [Presets]({{ site.baseurl }}/presets/). |
| `options schema` | Print every option as a JSON Schema. |
| `tools` | List the encoders and helpers in this build. `--check` exits with 3 if a required one is missing. `--json` prints JSON. |
| `history` | `history stats` shows how many files are remembered. `history clear` forgets them. See [Privacy]({{ site.baseurl }}/privacy/). |
| `undo RECORD` | Revert one finished job from its undo record. See [Undo]({{ site.baseurl }}/undo/). |
| `open` {% include pills.html on="mac" %} | macOS only. Hand files to the app window. It takes the same options as `run`, plus `--start` to begin at once. |
| `install` | Link this program into a folder on your PATH (`--dir`, default `/usr/local/bin`). |
| `uninstall` | Remove the link made by `install`. |
| `licenses` | Print the license notices. See [Acknowledgements]({{ site.baseurl }}/acknowledgements/). |
| `completions SHELL` | Print shell completions for `bash`, `zsh`, `fish`, `elvish` or `powershell`. |
| `man` | Print the manual page. |

## Run options

`imgcompress run [OPTIONS] [PATHS]...` takes files and folders. The options below are grouped by what they do.
Each one overrides the matching setting from the preset you started from.

### Starting point

| Option | Meaning |
| --- | --- |
| `--preset NAME\|FILE` | Start from a preset: its id or name, or the path of a preset file. |
| `--options-file FILE` | A JSON file with some options. Applied after `--preset`. |
| `--save-preset NAME` | Save the options of this run as a preset. With no paths, it only saves. |

Options are applied in this order: built-in defaults, `--preset`, `--options-file`, then the flags you typed.

### Choosing input files

| Option | Meaning |
| --- | --- |
| `--recursive`, `--no-recursive` | Go into subfolders. On by default. |
| `--include-hidden` | Also process files and folders that start with a dot. |
| `--min-size SIZE` | Skip files smaller than this, for example `10KB` or `2MB`. |

A folder gives you its images (JPEG, PNG, GIF, WebP, AVIF, JPEG XL, HEIC, TIFF, SVG). A file you name directly is
always tried.

### Output

| Option | Meaning |
| --- | --- |
| `--format FORMAT` | `keep` (default), `jpeg`, `png`, `webp`, `heic`, `avif` or `jxl`. |
| `--destination in-place\|alongside\|DIR` | Replace the file, write next to it with a suffix, or write into a folder. |
| `--suffix SUFFIX` | The suffix for `alongside`. Default `-min`. |
| `--overwrite skip\|rename\|replace` | What to do when the output name is taken by another file. Default `skip`. |
| `--replace-if smaller\|always` | Keep a result only if it is smaller (default), or always. |
| `--preserve-dates`, `--no-preserve-dates` | Keep the file's dates. On by default. |
| `--anim-to keep\|webp\|avif` | What animated GIF, PNG, WebP and AVIF files become. |

The output rules are described in [Undo and safety]({{ site.baseurl }}/undo/).

### Originals

| Option | Meaning |
| --- | --- |
| `--originals keep\|trash\|delete` | What happens to the original after a format change. Default `keep`. |
| `--backup-dir DIR` | Keep a copy of every file this run replaces or removes in `DIR`, and add an undo record to each finished result. |

### Quality shortcut and quality targets

| Option | Meaning |
| --- | --- |
| `--quality N\|lossless` | A number from 1 to 100, or `lossless`. It is applied to the encoder that `--format` selects. With `--format keep` it applies to JPEG and PNG. |
| `--target-quality N\|off` | Aim for this SSIMULACRA2 score and find the lowest quality that reaches it, for each image (JPEG, WebP, AVIF, JPEG XL). 88 is "Visually lossless", 70 is "Smaller". See [Formats]({{ site.baseurl }}/formats/). |
| `--target-tolerance N` | Accept a score from the target up to the target plus `N`. |
| `--max-encodes N` | The most trial encodes per image. |

### Per-format options

JPEG:

| Option | Meaning |
| --- | --- |
| `--jpeg-encoder jpegli\|mozjpeg` | The encoder. |
| `--jpeg-quality 1-100\|lossless` | Lossless keeps the picture data and only repacks it, so it works only on JPEG files. Converting another format to JPEG at lossless skips that file; choose a quality, or a lossless format (PNG, WebP or JPEG XL). |
| `--jpeg-progressive`, `--no-jpeg-progressive` | Progressive files. |
| `--jpeg-subsampling auto\|444\|422\|420` | Colour detail. |

PNG:

| Option | Meaning |
| --- | --- |
| `--png-quantize MIN-MAX\|off` | Reduce colours within this quality range. `off` is lossless. |
| `--png-quantize-speed N` | 1 to 11. |
| `--png-dither`, `--no-png-dither` | Dithering for reduced colours. |
| `--png-optimize-level 0-6\|max` | How hard to repack. |
| `--png-zopfli ITERATIONS\|off` | Slower, smaller repacking. |

JPEG XL:

| Option | Meaning |
| --- | --- |
| `--jxl-quality 1-100\|lossless` | Quality. |
| `--jxl-effort N` | 1 to 10. Higher is slower and smaller. |
| `--jxl-jpeg-transcode`, `--no-jxl-jpeg-transcode` | Lossless repacking of JPEG files. |
| `--jxl-progressive`, `--no-jxl-progressive` | Progressive decoding. |

AVIF, HEIC and WebP:

| Option | Meaning |
| --- | --- |
| `--avif-quality N` | 1 to 100. |
| `--avif-alpha-quality 1-100\|auto` | Quality of transparency. |
| `--avif-speed N` | 0 to 10. |
| `--avif-depth 8\|10` | Bits per colour. |
| `--heic-quality N` | 1 to 100. |
| `--heic-depth 8\|10` | Bits per colour. macOS only. |
| `--webp-quality 1-100\|lossless` | Quality. |
| `--webp-method N` | 0 to 6. Higher is slower and smaller. |
| `--webp-alpha-quality N` | 1 to 100. |

GIF and SVG:

| Option | Meaning |
| --- | --- |
| `--gif-optimize-level N` | 1 to 3. |
| `--gif-lossy N\|off` | 0 to 200. Allows some loss for a smaller file. |
| `--svg-precision N` | 0 to 5 decimal places kept in numbers and paths. |
| `--svg-aggressive`, `--no-svg-aggressive` | Also rename ids and simplify transforms. The result is still checked by drawing it. |
| `--svg-max-size SIZE` | Skip SVGs larger than this. `0` means no limit. |

### Size and colour

| Option | Meaning |
| --- | --- |
| `--max-dimension PX` | Shrink so the longer side is at most this. It never enlarges. |
| `--scale PERCENT` | Shrink to this percent of the size, 1 to 99. |
| `--convert-to-srgb` | Convert wide-gamut colours (such as Display P3) to sRGB and drop the profile. |

### Metadata

| Option | Meaning |
| --- | --- |
| `--metadata none\|icc\|exif-no-gps\|all` | What to keep. Default `icc`. See [Formats]({{ site.baseurl }}/formats/#metadata). |
| `--compact-icc`, `--no-compact-icc` | Drop colour profiles that only mean sRGB. On by default. |
| `--strip-previews` | Remove embedded thumbnails and previews. |

### Running

| Option | Meaning |
| --- | --- |
| `--history`, `--no-history` | Skip files already processed with the same options. On by default. |
| `--jobs N` | How many files at once. `0` uses every core. |
| `--dry-run` | Print the plan and change nothing. |
| `--json` | Print one JSON event per line, for scripts. |
| `-q`, `--quiet` / `-v`, `--verbose` | Less or more output. |

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Everything went fine. |
| 1 | Some files failed. |
| 2 | Wrong command or option. |
| 3 | An encoder or helper is missing. |

## Shell completions and manual

```bash
imgcompress completions zsh > ~/.zfunc/_imgcompress     # or bash, fish
man imgcompress
```

The Homebrew formula and the Linux packages install the completions and the manual page for you.
If you installed the tool another way, run `imgcompress man` to print the manual page.
