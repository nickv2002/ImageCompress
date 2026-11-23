---
title: Presets
description: The 21 built-in presets, how favorites and the Finder menu use them, and how to edit, share and script preset files.
permalink: /presets/
---

# Presets

<p class="applies">Applies to {% include pills.html on="both" %}</p>

<p class="lead">A preset is a saved set of options; the app, the Finder menu and the command line all use the same ones.</p>

## Built-in presets

{% include shot.html src="settings-presets" w=820 h=592 alt="Settings, Presets pane: a list of presets with stars, and the selected preset's options" caption="Settings → Presets. Starred presets fill the toolbar; each one can be shown in the Finder menu." %}

There are 21 built-in presets: every output format (Preserve, JPEG XL, AVIF, HEIC, WebP, JPEG, PNG) at every
quality (Lossless, Visually lossless, Smaller). Their names look like "JPEG XL · Smaller" and their ids like
`jxl-smaller`. See [Formats and quality]({{ site.baseurl }}/formats/) for what they do.

None of them touches your originals beyond replacing the file in place.

## Favorites and the toolbar
{: .mac}

The Format and Quality controls in the toolbar run these presets. A preset with a format and quality that is marked
as a favorite (the star in Settings → Presets) fills those controls.

You can also make your own presets. In the main window, change options in the inspector and choose
**Save as Preset…**, or use the Presets menu in the toolbar. Manage them in Settings → Presets.

In the app, presets build on your defaults in Settings → General. An option a preset does not set takes the default value.

## Show in Finder menu
{: .mac}

A preset marked **Show in Finder menu** appears in the Finder right-click submenu
(see [Get started]({{ site.baseurl }}/getting-started/)). The built-in ones marked this way are:

- Preserve · Lossless (`keep-lossless`)
- Preserve · Smaller (`keep-smaller`)
- JPEG XL · Visually lossless (`jxl-visually-lossless`)

Turn the setting on or off per preset in Settings → Presets.

## The Presets folder

Every preset is one JSON file. The file name, without `.json`, is the preset's id.

| System | Folder |
| --- | --- |
| macOS | `~/Library/Group Containers/3D9SMH4RWM.com.nickvance.imagecompress/Presets` |
| Linux | `$XDG_CONFIG_HOME/imgcompress/Presets`, usually `~/.config/imgcompress/Presets` |

Set the `IMGCOMPRESS_CONFIG` environment variable to a folder to use that folder instead. The Presets folder is
then inside it.

The app creates the folder when it starts. Until the folder exists, the built-in presets are used.
In Settings → Presets, **Show Preset File in Finder** takes you to a file. On the command line, `imgcompress presets path`
prints the folder.

## Edit and share preset files

You can edit the files in any text editor and copy them to another Mac. A file only needs the options that
differ from your defaults. Here is a complete preset:

```json
{
  "name": "Web photos",
  "show_in_finder": true,
  "options": {
    "format": "webp",
    "target-quality": 75,
    "max-dimension": 2000,
    "metadata": "none"
  }
}
```

Save it as `web-photos.json` in the Presets folder. Its id is then `web-photos`.

- `name` is what the app shows. If it is missing, the file name is used.
- `show_in_finder` puts the preset in the Finder menu.
- `options` uses the same names as the command line flags (without `--`). Run `imgcompress options schema` to see them all.
  A name that is not an option is an error, and that file is skipped.
- A value of `null` switches an option off, for example `"jpeg-quality": null` means lossless JPEG
  (a repack of JPEG files; other images converted to JPEG at lossless are skipped).

Files that cannot be read are skipped, so one typo never breaks the rest. Two presets cannot share a name.
Presets made by the app also have a `toolbar` entry (favorite, format and quality). The app uses it; you can leave it out of files you write by hand.

Here is a built-in preset as `imgcompress presets export keep-smaller` prints it:

```json
{
  "name": "Preserve · Smaller",
  "show_in_finder": true,
  "options": {
    "format": "keep",
    "png-quantize": [70, 90],
    "target-quality": 70
  },
  "toolbar": {
    "favorite": true,
    "format": "keep",
    "quality": "smaller"
  }
}
```

## Presets on the command line
{: .cli}

```bash
imgcompress presets list                     # ids and names; [finder] marks the Finder menu
imgcompress presets show keep-smaller        # all options, expanded
imgcompress presets export keep-smaller > my.json   # one shareable file
imgcompress presets export > all.json        # every preset as one bundle
imgcompress presets import my.json           # copy a file or bundle into the Presets folder
imgcompress presets delete web-photos
imgcompress presets reset                    # restore the built-ins, keep your own
imgcompress presets reset --all              # also remove your own presets
imgcompress presets path                     # print the Presets folder
```

Use a preset when compressing:

```bash
imgcompress run --preset keep-smaller ~/Pictures
imgcompress run --preset ./my.json photo.png      # a preset file, anywhere
```

`--preset` takes an id, a name, or the path of a preset file. Flags you add after it win over the preset.
To save the options of a run as a preset, add `--save-preset "My preset"`. With no paths it only saves the preset. With paths it saves and then compresses:

```bash
imgcompress run --format avif --quality 60 --save-preset "Small AVIF" photo.png
```

The older preset ids `lossless`, `lossy`, `jpg`, `png`, `heic`, `avif` and `jxl` still work in scripts; they point to the
matching built-in presets.

See the [command line reference]({{ site.baseurl }}/cli/) for every option.
