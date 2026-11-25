---
title: Get started
description: Install ImageCompress on macOS or Linux, compress your first images and turn on the Finder menu.
permalink: /getting-started/
---

# Get started

<p class="applies">Applies to {% include pills.html on="both" %}</p>

<p class="lead">Install ImageCompress, drop some images on the window and press Compress.</p>

## Install on macOS
{: .mac}

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
{: .mac}

{% include shot.html src="drop-zone" w=1040 h=560 alt="The empty ImageCompress window, asking you to drop images or folders" caption="The window before you add anything. Drop files or folders on it, or press ⌘O." %}

1. Open ImageCompress.
2. Drag images or folders onto the window. You can also use File → Add Files… (⌘O).
3. In the toolbar, choose a **Format** and a **Quality**. The [formats page]({{ site.baseurl }}/formats/) explains the choices.
4. Click **Compress**, or press ⌘↩.

By default the app replaces each file with the smaller version. It keeps a copy of every original so you can
[undo]({{ site.baseurl }}/undo/). A result is only kept if it is smaller than the original.

Select a finished image and press Space or ⌘Y to compare Before and After. The comparison opens at once on the
original and the result, and rates how different they look, from *Visually lossless* to *Large differences*.
Use the slider, put the two side by side, or show only the difference. Zoom up to 8× (pinch, ⌘-scroll, or ⌘= and ⌘-)
to see single pixels, and drag the small map at the top left to move around. Space closes it again.

Each Format and Quality pair is a [preset]({{ site.baseurl }}/presets/). Settings (⌘,) lets you change the defaults
and make your own presets.

## Turn on the Finder menu
{: .mac}

{% include shot.html src="finder-menu" w=980 h=710 alt="Finder context menu with an ImageCompress submenu listing presets" caption="Right-click a selection in Finder and choose a preset from the ImageCompress submenu." %}

{% include shot.html src="settings-finder" w=820 h=592 alt="Settings, Finder pane: extension status Enabled, menu title and what choosing a preset does" caption="Settings → Finder shows whether the extension is on and what a preset in the menu does." %}

The Finder menu is a Finder extension. macOS keeps it off until you turn it on.

- **When you first open ImageCompress**, it offers to turn the menu on. Click **Turn On**. Finder restarts
  for a moment to load it.
- **Later**, open ImageCompress → Settings → Finder. The pane shows whether the menu is on, and has a
  **Turn On** button while it is off.
- **In System Settings**, the switch is under General → Login Items & Extensions, in the Extensions list.
  Some versions of macOS 15 Sequoia do not show a switch for Finder menus at all; use one of the other ways.
- **In Terminal**, this does the same and restarts Finder:

  ```bash
  pluginkit -e use -i com.nickvance.imagecompress.FinderMenu && killall Finder
  ```

Now right-click images or folders in Finder and choose the **ImageCompress** submenu. Click a preset to compress
with it right away.

The submenu lists the presets marked "Show in Finder menu". Out of the box that is Preserve · Lossless,
Preserve · Smaller and JPEG XL · Visually lossless. See [Presets]({{ site.baseurl }}/presets/) to change the list.
In Settings → Finder you can rename the menu, or make it open the files in ImageCompress instead of compressing at once.

The extension also provides a toolbar button. To add it to Finder's toolbar, choose View → Customize Toolbar… in Finder.

### If the Finder menu does not appear

Open ImageCompress → Settings → Finder first. It checks the two usual causes:

- **The extension is off.** The status says "Not enabled". Click **Turn On**.
- **There is more than one copy of ImageCompress.app.** Old copies in Downloads, on another disk, or left by a
  build can make Finder ignore the menu without any message. The pane lists each one; click a path to show it in
  Finder. Delete the extra copies, then click **Restart Finder**.

If the status says "Enabled" and no other copies are listed, click **Restart Finder**, or run `killall Finder` in
Terminal. Finder reopens by itself. On a Mac managed by your employer or school, extensions can be blocked without
any message.

To check from Terminal, `pluginkit -m -A -D -v -i com.nickvance.imagecompress.FinderMenu` lists every copy of the
extension macOS knows about. A `+` at the start of a line means it is on.

## Services menu
{: .mac}

Without the Finder extension, the same actions are in the Services menu (Finder → Services, or right-click →
Services). They are for images and folders:

- **Compress with ImageCompress** compresses with the settings currently in the window.
- **Compress Smaller with ImageCompress** compresses with the Preserve · Smaller preset.
- **Open in ImageCompress…** adds the files to the window without starting.

## Command line tool
{: .cli}

The app includes the `imgcompress` command. If you installed the app with Homebrew, the cask has already put
`imgcompress` on your PATH and there is nothing to do.

Otherwise choose ImageCompress → **Install Command Line Tool…** (also in Settings → Tools). It first checks whether
`imgcompress` is already installed, in the Homebrew folders, `/usr/local/bin` or `~/.local/bin`, and if so tells you
where and which version. If not, it links `/usr/local/bin/imgcompress` to the copy inside the app, asking for your
administrator password when that folder needs it.

If you prefer Homebrew, the command is also a formula. It works on macOS and Linux:

```bash
brew trust --formula nickv2002/tap/imgcompress && brew install nickv2002/tap/imgcompress
```

See the [command line reference]({{ site.baseurl }}/cli/). The app and the command share presets (not the app's General defaults), so the same preset gives the same result in both.

## Install on Linux
{: .cli}

Only the command line tool is available on Linux. Add the signed package repository once, and your normal
system updates keep `imgcompress` current. The binary is static, so there are no dependencies.

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

The signing key's fingerprint is `01F9 3355 749B 96C1 1D12  B14D 63E5 419C 7998 DC58`
([repository page](https://nickv2002.github.io/packages/)).

You can also download fully static tarballs (`imgcompress-<version>-linux-x86_64.tar.gz` and
`imgcompress-<version>-linux-aarch64.tar.gz`) from the
[releases page](https://github.com/nickv2002/ImageCompress/releases).
Homebrew on Linux works too, with the formula above. To build it yourself, download
`imgcompress-<version>-sources.tar.gz` from a release; `BUILD_FROM_SOURCE.txt` inside has the `cargo` steps.

## Updates
{: .both}

On macOS the app checks for updates itself (it uses Sparkle). In Settings → Tools → Updates you can turn
automatic checks off, or click **Check Now**. You can also use ImageCompress → Check for Updates….

Homebrew and the Linux packages update with `brew upgrade`, `apt upgrade` or `dnf upgrade`.

## Next

- [Formats and quality]({{ site.baseurl }}/formats/)
- [Presets]({{ site.baseurl }}/presets/)
- [Undo and safety]({{ site.baseurl }}/undo/)
