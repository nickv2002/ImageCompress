---
title: Undo and safety
description: How ImageCompress keeps your originals, how long Undo works, and how output names and originals are handled.
permalink: /undo/
---

# Undo and safety

<p class="applies">Applies to {% include pills.html on="both" %}</p>

<p class="lead">ImageCompress can put back every original it replaces: automatically for a day in the app, and with a backup folder you choose on the command line.</p>

## In the app and on the command line

| | macOS app | Command line |
| --- | --- | --- |
| How copies are kept | Automatically, on by default | Only when you add `--backup-dir DIR` |
| Where | A temporary folder, `/private/tmp/ImageCompressUndo-<your user id>` | The folder `DIR` you chose |
| How long | 24 hours | No expiry. You delete the copies yourself. |
| How much space | Up to 1 GB, oldest first | Whatever the originals take in `DIR` |
| How to undo | **Process → Recent Compressions & Undo…** (⌥⌘Z) | `imgcompress undo RECORD` |
| If the file changed | Undo refuses and changes nothing | Undo refuses and changes nothing |

Both check the compressed file against a fingerprint (a blake3 hash) taken when it was written.
The rest of this page covers the app first, then the command line.

## Undo in the app
{: .mac}

{% include shot.html src="undo-history" w=560 h=360 alt="The Recent Compressions window listing compressed photos, each with an Undo button" caption="Process → Recent Compressions & Undo… (⌥⌘Z) lists everything that can still be put back." %}

Before the app replaces or removes a file, it keeps a copy of the original.

- Copies are kept for **24 hours**, up to **1 GB** in total. When there is more, the oldest go first.
- They are stored in a temporary folder, `/private/tmp/ImageCompressUndo-<your user id>`. Restarting your Mac clears
  that folder. macOS also deletes files there after about three days, even if you never open ImageCompress again.
- Undo copies are for a quick change of mind. They are not a backup. Keep your own backups of anything important.

To undo, choose **Process → Recent Compressions & Undo…** (⌥⌘Z). The list shows recent files with their sizes.
Click Undo next to a file, or right-click a finished file in the main window and choose **Undo Compression**.

Undo puts the original back. If the run made a new file next to it, that new file goes to the Trash. Undo refuses, and changes nothing, when:

- the compressed file was edited, moved or deleted since ImageCompress wrote it;
- the saved original is gone (cleared by a restart, or older than a day);
- the original's place is taken by another file.

If you do not want undo copies, turn off **Keep undo copies of replaced originals** in Settings → General.

## Changing settings and compressing again
{: .mac}

Say you compressed `photo.png` to AVIF and now want WebP, or a different quality. While the undo copy exists
(24 hours), compressing again starts from the original, not from the earlier result. That way quality is not lost twice.

There are three ways to do it:

- **Add the file again.** Drop it on the window, use Add Files…, Open With or the Finder menu. You can add either
  the result (`photo.avif`) or the original, if it is still there (`photo.png`).
- **Right-click finished images** and choose **Compress Again from Original**.
- **Click Compress Again** in the small bar at the bottom of the list. It appears when you change the format, quality,
  preset or options while finished images can still be redone, and counts only those images. Click × to hide it. It
  comes back when you change the settings again or the next images finish. Nothing is compressed again unless you ask.

{% include shot.html src="compress-again" w=960 h=512 alt="The main window with six finished images and a bar at the bottom: Settings changed. Compress these 6 images again from their originals? with a Compress Again button and a close button" caption="After a settings change, the bar offers to compress the finished images again from their originals." %}

In each case ImageCompress first does what Undo does: it puts the original back, and an earlier result with a new name
goes to the Trash. Then it compresses the original with the current settings. The row shows the new result. Its
savings are measured against the original. To keep the earlier result as well, rename or move it first. After that
it counts as a separate file.

If the original can't be put back, ImageCompress leaves everything as it is and says why in the window. For example,
the result may have been edited since. Nothing is compressed in that case.

Once the undo copy is gone, the file you add is compressed as it is now. The copy goes after 24 hours, after a restart,
or when you edit or move the result. The same happens with undo copies turned off. When that file is an earlier result,
the window says so. Compressing a lossy result again loses a little more quality each time. So for a different format
or quality, start from your own copy of the original when you can.

Adding a folder again compresses the files in it as they are now. To redo images from a folder, select their rows
and choose **Compress Again from Original**.

## Undo on the command line
{: .cli}

Undo is opt-in on the command line. Add `--backup-dir DIR` and each original that the run replaces or removes is
copied into `DIR` first. Nothing is removed from `DIR` later, so delete the copies when you no longer need them.

The undo record is not saved as a file for you. With `--json`, each `finished` event carries it in an `undo` field.
Save the event output, pick out the record of the file you want back, and give that file to `imgcompress undo`:

```bash
imgcompress run --json --backup-dir ~/ImageCompress-backups photos/holiday.png > run.jsonl
jq -c 'select(.event == "finished") | .undo' run.jsonl > holiday-undo.json
imgcompress undo holiday-undo.json
```

A record covers one file, so a run on many files gives you one record per file. `imgcompress undo` puts the original back
from the backup folder. It refuses, changes nothing and exits with 1 when the compressed file was edited, moved or deleted
since the run, or when the saved original is gone. See [Command line]({{ site.baseurl }}/cli/) for the other options.

`imgcompress run` always works on the file as it is. It does not look up earlier records, so running it again on a
result compresses the result, not the original. To redo a file with other settings, run `imgcompress undo RECORD`
first. Then run the new command on the original.

## Files are replaced safely

ImageCompress writes the new file next to the old one first. It checks that the new file opens. Only then does it
swap it in with a single rename. If something goes wrong halfway, the original is still there.

## Output names

Where results go is set by the destination:

- **In place** (default): the result replaces the file.
- **Alongside**: the result is written next to the input with a suffix added to its name (default `-min`).
- **A folder**: the results are written into that folder.

A format change gives the file a new extension. A file is replaced only when the output name is the input itself
(the same format, in place). In that case the original is not moved to the Trash or deleted, because it was replaced.

Two inputs that would get the same name never overwrite each other. `a.png` and `a.avif` converted to WebP
alongside with the suffix `-w` become `a-w.png.webp` and `a-w.avif.webp`. If names still clash (`x/IMG.png` and
`y/IMG.png` into one folder), the later one gets `-2`, then `-3`. Names are compared without regard to upper and lower case.

### When the name is already taken

When the output name belongs to a file that is not the input itself, you decide what happens with `--overwrite`
on the command line:

| Situation | `skip` (default) | `rename` | `replace` |
| --- | --- | --- | --- |
| Output is the input (in place, same format) | atomic replace | atomic replace | atomic replace |
| Several inputs, same output name | distinct names (extension kept, then `-N`) | same | same |
| Output name is an existing file | input skipped, original kept | `-N` | file replaced |
| Output name is another input of the run | input skipped | `-N` | `-N` |

With `skip` you can safely run the same command again. An input skipped this way is reported as
`output exists: ...` and its original is left alone.

You can find the same choice for the app in the Output and originals section of Settings → General.

## What happens to originals

{% include shot.html src="settings-general" w=820 h=592 alt="Settings, General pane: default preset, start compressing on add, keep undo copies, originals and destination" caption="Settings → General holds the app's choices for undo copies, originals and where results go." %}

A preset or run chooses one of three:

- **Keep**: the original stays. This is the default. When the format changes, you end up with both files.
- **Trash**: the original moves to the Trash after a successful conversion.
- **Delete**: the original is deleted permanently.

The app asks before it permanently deletes originals. It can also ask before moving them to the Trash:
Settings → General → **Ask before moving originals to the Trash**.

Undo works for Trash and Delete runs too, as long as the undo copy is still there.

### Volumes without a Trash
{: .mac}

Some volumes have no Trash, usually network shares. If a run would move originals to the Trash on such a volume,
the app says so before it starts. It offers to keep the originals next to the new files for that run instead, because
otherwise those files would fail.

## Other safeguards

- The result is only kept if it is smaller than the original (the `--replace-if smaller` default).
- A result is decoded again to check it before it replaces anything.
- File dates are kept by default.
- Use `--dry-run` on the command line to see what would happen. Nothing is changed. It also prints the output name of each file. {% include pills.html on="cli" %}
- `--backup-dir DIR` keeps copies of the originals and gives each finished result an undo record.
  `imgcompress undo RECORD` reverts one record. See [Undo on the command line](#undo-on-the-command-line). {% include pills.html on="cli" %}
