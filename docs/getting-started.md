---
title: Get started
description: Install ImageCompress on macOS or Linux, compress your first images and turn on the Finder menu.
permalink: /getting-started/
---

# Get started

<p class="lead">Install ImageCompress, drop some images on the window and press Compress.</p>

## Install on macOS

ImageCompress needs macOS 15 or later.

With Homebrew:

```bash
brew tap nickv2002/tap
brew trust --cask nickv2002/tap/imagecompress
brew install --cask imagecompress
```

Or without Homebrew: download
[ImageCompress.zip](https://github.com/nickv2002/ImageCompress/releases/latest/download/ImageCompress.zip),
unzip it and drag ImageCompress to your Applications folder.

The app updates itself. See [Updates](#updates).

## Your first compression

1. Open ImageCompress.
2. Drag images or folders onto the window. You can also use File → Add Files… (⌘O).
3. In the toolbar, choose a **Format** and a **Quality**. The [formats page]({{ site.baseurl }}/formats/) explains the choices.
4. Click **Compress**, or press ⌘↩.

By default the app replaces each file with the smaller version. It keeps a copy of every original so you can
[undo]({{ site.baseurl }}/undo/). A result is only kept if it is smaller than the original.

Select a finished image and press Space or ⌘Y to compare Before and After.

Each Format and Quality pair is a [preset]({{ site.baseurl }}/presets/). Settings (⌘,) lets you change the defaults
and make your own presets.

## Turn on the Finder menu

The Finder menu is a Finder extension. macOS keeps it off until you turn it on.

1. Open ImageCompress → Settings → Finder and click **Open Extensions Settings…**.
   Or open System Settings → General → Login Items & Extensions yourself and look under Extensions for Finder.
2. Enable **ImageCompress Finder Menu**.

Now right-click images or folders in Finder and choose the **ImageCompress** submenu. Click a preset to compress
with it right away.

The submenu lists the presets marked "Show in Finder menu". Out of the box that is Preserve · Lossless,
Preserve · Smaller and JPEG XL · Visually lossless. See [Presets]({{ site.baseurl }}/presets/) to change the list.
In Settings → Finder you can rename the menu, or make it open the files in ImageCompress instead of compressing at once.

The extension also provides a toolbar button. To add it to Finder's toolbar, choose View → Customize Toolbar… in Finder.

### If the Finder menu does not appear

- Quit Finder so it reloads its extensions: run `killall Finder` in Terminal. Finder reopens by itself.
- Keep only one copy of ImageCompress.app on your Mac. Old copies in Downloads or elsewhere can make Finder ignore the extension. Delete the extra copies, then run `killall Finder` again.
- Check that the extension is switched on in System Settings. The Settings → Finder pane in ImageCompress shows its status.
- On a Mac managed by your employer or school, extensions can be blocked without any message.

## Services menu

Without the Finder extension, the same actions are in the Services menu (Finder → Services, or right-click →
Services). They are for images and folders:

- **Compress with ImageCompress** compresses with the settings currently in the window.
- **Compress Smaller with ImageCompress** compresses with the Preserve · Smaller preset.
- **Open in ImageCompress…** adds the files to the window without starting.

## Command line tool

The app includes the `imgcompress` command. To use it in Terminal, choose ImageCompress → **Install Command Line Tool…**.
This links `/usr/local/bin/imgcompress` to the copy inside the app. The same item is in Settings → Tools.
If you installed the app with Homebrew, the cask has already put `imgcompress` on your PATH.

If you prefer Homebrew, the command is also a formula. It works on macOS and Linux:

```bash
brew trust --formula nickv2002/tap/imgcompress && brew install nickv2002/tap/imgcompress
```

See the [command line reference]({{ site.baseurl }}/cli/). The app and the command share presets (not the app's General defaults), so the same preset gives the same result in both.

## Install on Linux

Only the command line tool is available on Linux. Packages are hosted by [Cloudsmith](https://cloudsmith.com)
and update with your normal package manager.

```bash
# Debian / Ubuntu
curl -1sLf 'https://dl.cloudsmith.io/public/nickv2002/imagecompress/setup.deb.sh' | sudo -E bash
sudo apt install imgcompress
# Fedora / RHEL / openSUSE
curl -1sLf 'https://dl.cloudsmith.io/public/nickv2002/imagecompress/setup.rpm.sh' | sudo -E bash
sudo dnf install imgcompress
```

You can also download fully static tarballs (`imgcompress-<version>-linux-x86_64.tar.gz` and
`imgcompress-<version>-linux-aarch64.tar.gz`) from the
[releases page](https://github.com/nickv2002/ImageCompress/releases).
Homebrew on Linux works too, with the formula above.

## Updates

On macOS the app checks for updates itself (it uses Sparkle). In Settings → Tools → Updates you can turn
automatic checks off, or click **Check Now**. You can also use ImageCompress → Check for Updates….

Homebrew and the Linux packages update with `brew upgrade`, `apt upgrade` or `dnf upgrade`.

## Next

- [Formats and quality]({{ site.baseurl }}/formats/)
- [Presets]({{ site.baseurl }}/presets/)
- [Undo and safety]({{ site.baseurl }}/undo/)
