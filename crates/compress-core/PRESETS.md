# Presets

One JSON file per preset in the `Presets` folder, shared by the CLI (`imgcompress presets ...`, `--preset NAME|FILE`), the Mac app and the Finder extension. A preset file can be shared as it is: drop it into the folder, or pass its path to `--preset`. Source: `src/presets.rs`, paths: `src/config.rs`.

## Location

| Platform | Config folder |
|---|---|
| macOS | `~/Library/Group Containers/3D9SMH4RWM.com.nickvance.imagecompress/` |
| Linux | `$XDG_CONFIG_HOME/imgcompress/` (default `~/.config/imgcompress/`) |
| override | env `IMGCOMPRESS_CONFIG` = the config folder (all platforms) |

Inside it: `Presets/<id>.json` (one per preset), `preset-order.json` (optional list order), `jobs/` (app hand-off). `imgcompress presets path` prints the Presets folder.

If the Presets folder does not exist, readers use the 21 built-in presets (`imgcompress presets export` prints them). The first change (`--save-preset`, `presets import|delete|reset`, or the Mac app at launch) writes the built-ins into it first. Writes are atomic (a hidden temp file in the same folder, then rename); readers skip files whose names start with `.` or do not end in `.json`.

The history database is NOT in the App Group container: `~/Library/Application Support/imgcompress/history.sqlite` (Linux `$XDG_DATA_HOME/imgcompress/history.sqlite`, override `IMGCOMPRESS_HISTORY`). Only the CLI touches it (`imgcompress history stats|clear`).

## Preset file

`Presets/jxl-visually-lossless.json`:

```json
{
  "name": "JPEG XL · Visually lossless",
  "show_in_finder": true,
  "options": { "format": "jxl", "png-quantize": [80, 95], "target-quality": 88 },
  "toolbar": { "format": "jxl", "quality": "visually-lossless", "favorite": true }
}
```

- The file name (without `.json`) is the preset's id. `--preset` matches id first, then name (case-insensitive) or slug. An `id` key inside the file is ignored.
- `name`: display name; the id when missing. `show_in_finder`: include in the Finder right-click menu.
- `toolbar` (Mac app, optional): `format` and `quality` tags place the preset in the app's Format and Quality toolbar controls when `favorite` is true. Tags are free strings; the known ones are `keep jxl avif heic webp jpeg png` and `lossless visually-lossless smaller`. A preset without both tags is listed in the app's Presets menu instead. Other unknown top-level keys are preserved too.
- `options`: a PARTIAL `Options` object with the kebab-case keys of `imgcompress options schema`. Layering is key-by-key on top of `Options::default()`. An explicit `null` is meaningful and overrides to "none" (e.g. `"jpeg-quality": null` = lossless, `"png-quantize": null` = off). Unknown keys make the file invalid (typo guard). Writers may store full options objects as well.
- `png-quantize` is `[min, max]` or `null`.

A file that is not valid JSON, has an unknown option, or has the same name as another preset is skipped: the CLI says so on stderr (`presets list`), and fails only when that preset is asked for. The Mac app shows the reason and never writes over such a file.

List order: the ids in `preset-order.json` (a JSON array, written by the Mac app when presets are reordered or added), then the built-ins in their order, then the rest by name.

`presets export NAME` prints a shareable preset file; `presets export` prints all of them as `{"version": 1, "presets": [...]}` with ids. `presets import FILE...` accepts either form; an imported preset gets a file named after its name, and a name that already exists is refused unless `--force`.

## Layering at run time

defaults -> `--preset` -> `--options-file` -> individual flags. `--quality N|lossless` expands relative to the effective `format` before the other flags apply. `--save-preset NAME` stores the effective options as the difference from defaults.

## Built-in presets

Every output format (or `keep`) × three qualities: 21 presets with id `<format>-<quality>`, for example `keep-lossless`, `avif-smaller`, `jxl-visually-lossless`. None of them touches originals. Names read "JPEG XL · Visually lossless". `keep-lossless`, `keep-smaller` and `jxl-visually-lossless` are in the Finder menu.

| quality | options |
|---|---|
| `lossless` | `target-quality`, `png-quantize`, `jpeg-quality`, `jxl-quality`, `webp-quality` all `null`; AVIF adds `avif-quality: 100` |
| `visually-lossless` | `target-quality: 88`, `png-quantize: [80,95]` |
| `smaller` | `target-quality: 70`, `png-quantize: [70,90]` |

HEIC ignores the quality search and gets `heic-quality` 100 / 90 / 50.

The ids of the old built-ins still work with `--preset` when no preset has that name: `lossless` → `keep-lossless`, `lossy` → `keep-smaller`, `jpg` `png` `heif` `avif` `jxl` → `<format>-visually-lossless`.

`imgcompress presets reset` rewrites these files (keeps custom presets; `--all` removes every other file and the saved order).

## Job files (`imgcompress open`)

macOS only. `<config dir>/jobs/<uuid>.json`:
`{"preset": "keep-smaller"|null, "overrides": { partial Options }, "paths": ["/abs/..."], "start": false}`, followed by `open -b com.nickvance.imagecompress imagecompress://job/<uuid>`. `overrides` = `--options-file` contents plus explicit flags (not the preset). The app reads and deletes the file.
