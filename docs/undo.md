---
title: Undo and safety
description: How ImageCompress keeps your originals, how long Undo works, and how output names and originals are handled.
permalink: /undo/
---

# Undo and safety

<p class="lead">ImageCompress keeps a copy of every original it replaces for a day, so you can put it back.</p>

## Undo

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

A preset or run chooses one of three:

- **Keep**: the original stays. This is the default. When the format changes, you end up with both files.
- **Trash**: the original moves to the Trash after a successful conversion.
- **Delete**: the original is deleted permanently.

The app asks before it permanently deletes originals. It can also ask before moving them to the Trash:
Settings → General → **Ask before moving originals to the Trash**.

Undo works for Trash and Delete runs too, as long as the undo copy is still there.

### Volumes without a Trash

Some volumes have no Trash, usually network shares. If a run would move originals to the Trash on such a volume,
the app says so before it starts. It offers to keep the originals next to the new files for that run instead, because
otherwise those files would fail.

## Other safeguards

- The result is only kept if it is smaller than the original (the `--replace-if smaller` default).
- A result is decoded again to check it before it replaces anything.
- File dates are kept by default.
- Use `--dry-run` on the command line to see what would happen. Nothing is changed. It also prints the output name of each file.
- On the command line, `--backup-dir DIR` keeps copies of the originals and writes an undo record for each file.
  `imgcompress undo RECORD` reverts one record.
