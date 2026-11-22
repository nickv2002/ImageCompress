---
title: Privacy
description: ImageCompress works on your computer only. What it stores, and the one network request the Mac app makes.
permalink: /privacy/
---

# Privacy

<p class="lead">ImageCompress processes your images entirely on your computer and never uploads them.</p>

## What stays on your computer

- Your images are read, compressed and written locally. They are never sent anywhere.
- There are no accounts, no sign-in and no analytics.
- The command line tool makes no network requests.

## The one network request

The Mac app checks for new versions. It does this with Sparkle, a common updater for Mac apps. It fetches one file,
the update list at `https://nickv2002.github.io/ImageCompress/appcast.xml`, which is a public page hosted by GitHub Pages.

The app does not turn on Sparkle's optional system profile, so it does not send information about your Mac with the request.
As with any web request, the server hosting the file can see your IP address.

You can turn this off in Settings → Tools → Updates by switching off **Check for updates automatically**.
Then the app only checks when you click **Check Now**.

If you install with Homebrew or a Linux package, updates come from Homebrew, apt or dnf instead.

## Files ImageCompress keeps

- **Presets and settings.** Your presets are plain JSON files in the [Presets folder]({{ site.baseurl }}/presets/#the-presets-folder).
- **Undo copies.** Originals are kept for up to 24 hours in a temporary folder. See [Undo and safety]({{ site.baseurl }}/undo/).
- **Processing history.** See below.

## Processing history

So that a second run does not redo work, ImageCompress remembers which files it has already processed. It keeps this
in a small database on your computer:

| System | File |
| --- | --- |
| macOS | `~/Library/Application Support/imgcompress/history.sqlite` |
| Linux | `$XDG_DATA_HOME/imgcompress/history.sqlite`, usually `~/.local/share/imgcompress/history.sqlite` |

Set `IMGCOMPRESS_HISTORY` to a file path to keep it somewhere else.

For each processed file it stores:

- the full path of the file;
- the file's modification time and size;
- a hash of the file's contents (a short fingerprint, not the content);
- a fingerprint of the compression options used;
- the time it was recorded.

It does not store images or their contents. A file counts as done only if its path, date, size, hash and options all
still match. Change the file or the options and it is processed again.

To see how much is remembered, run `imgcompress history stats`. To erase it, run `imgcompress history clear`,
or use Settings → General → **Forget history…** in the app. To stop remembering, switch off
**Remember processed files** in the app's options, or pass `--no-history` on the command line.
