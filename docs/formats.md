---
title: Formats and quality
description: Which image formats ImageCompress reads and writes, and what Lossless, Visually lossless and Smaller mean.
permalink: /formats/
---

# Formats and quality

<p class="applies">Applies to {% include pills.html on="both" %}</p>

<p class="lead">Pick a format and how much loss you accept; ImageCompress works out the rest for each image.</p>

## Formats it can handle

| Format | Read | Write | Notes |
| --- | --- | --- | --- |
| JPEG | yes | yes | |
| PNG | yes | yes | Animated PNG is optimised when the pixels are not changed. |
| WebP | yes | yes | Animated WebP too. |
| GIF | yes | no | Animated GIFs are optimised as GIFs. They can also become animated WebP or AVIF. |
| AVIF | yes | yes | Animated AVIF too. |
| JPEG XL | yes | yes | |
| HEIC / HEIF | yes | yes | On macOS. On Linux it needs a build option that is off in the packages. |
| TIFF | yes | no | Can be converted to any of the formats above. |
| SVG / SVGZ | yes | yes | Optimised as SVG. An SVG is never turned into a raster format. |

Folders are searched for files with these extensions: jpg, jpeg, png, gif, heic, heif, webp, tif, tiff, avif, jxl,
svg and svgz.

GIF optimisation uses gifsicle, which is bundled with the app and with the Linux tarballs and packages.

### SVG

SVG files are made smaller by removing what is not needed. ImageCompress draws the result next to the original
and keeps the original if they look different. Files with text are handled more carefully, because the check
cannot see text. Files over 1 MB are skipped by default. Both limits and the number precision can be changed.

## Format and Quality

{% include shot.html src="format-menu" w=560 h=215 alt="The Format menu open in the toolbar: Preserve format, or convert to JPEG XL, AVIF, HEIC, WebP, JPEG or PNG" caption="The Format menu and the Quality control in the toolbar." %}

The toolbar has two controls.

**Format**: Preserve, JPEG XL, AVIF, HEIC, WebP, JPEG or PNG.

- **Preserve** keeps each image in its own format and just makes it smaller.
- The others convert every image to that format. The new file gets the new extension.

**Quality**:

| Quality | What it means |
| --- | --- |
| Lossless | Every pixel stays exactly the same. Smallest safe savings. |
| Visually lossless | Technically lossy, but you would not see a difference. Modest savings for JPEG, much more with a modern format. |
| Smaller | A little loss that is hard to spot, for a big size reduction. |

Seven formats and three qualities make the 21 built-in [presets]({{ site.baseurl }}/presets/).

## How "Visually lossless" and "Smaller" work

Fixed quality numbers are a poor guide. The same number looks fine on one photo and bad on another. So for
JPEG, WebP, AVIF and JPEG XL, ImageCompress tests each image on its own.

1. It encodes the image at a trial quality.
2. It decodes the result and scores it against the original with SSIMULACRA2, a measure of how different two images look to a person.
3. It adjusts the quality and repeats, up to 6 encodes per image, until it finds the lowest quality whose score still reaches the target.

The targets are 88 for Visually lossless and 70 for Smaller. A higher score means closer to the original. The result lands within about 1.5 points above the target.
For very large images the search runs on sample tiles taken at full resolution, then the final quality is checked on the whole image.

Other formats work a little differently:

- **PNG** has no quality slider. Visually lossless and Smaller shrink the colour palette within a range (80 to 95 and 70 to 90). Lossless only repacks the data.
- **HEIC** does not use the search. It uses fixed qualities: 100 for Lossless, 90 for Visually lossless and 50 for Smaller.
- **AVIF** has no exact lossless mode here. Lossless uses the highest quality setting, 100.
- **JPEG** is lossless only for JPEG files, which are repacked without decoding. Other images converted to JPEG at Lossless are skipped, because a JPEG made from them would lose detail.

## JPEG encoders

ImageCompress writes JPEG with **jpegli** by default. jpegli is Google's JPEG encoder, from the JPEG XL project
(libjxl). It makes ordinary JPEG files that every JPEG decoder opens. Google's [announcement](https://opensource.googleblog.com/2024/04/introducing-jpegli-new-jpeg-coding-library.html)
says it uses adaptive quantization to reduce noise and improve image quality, and claims about 35% better
compression at high quality settings in its own tests. It can also work with 10 or more bits per component to avoid
banding in slow gradients, but Google says that needs application support.

**mozjpeg** is the other encoder, and the long-standing reference. Choose it with `--jpeg-encoder mozjpeg` in
the command line, or with Encoder in the JPEG section of the options panel.

We measured both on the four sample photos from the [format comparison]({{ site.baseurl }}/compare/), at Visually
lossless (score 88). jpegli gave the smaller file every time:

| Photo | jpegli | mozjpeg | Smaller by |
| --- | --- | --- | --- |
| Landscape | 92.3 KB | 100.5 KB | 8% |
| Portrait | 65.6 KB | 75.6 KB | 13% |
| City | 68.7 KB | 77.7 KB | 12% |
| Graphic | 31.6 KB | 33.0 KB | 4% |

These are single crops, so treat the figures as a guide, not a promise. At the same fixed quality number (85) the
two encoders do not give the same picture: jpegli's files were larger but scored higher, so compare at a score, as
Visually lossless does.

Lossless JPEG repacks are a separate path. They do not use either encoder to re-encode the picture: the
image data is kept and only repacked.

## Lossless JPEG to JPEG XL

When you convert a JPEG to JPEG XL at Lossless quality, ImageCompress does not decode the picture. It repacks
the JPEG data. The original JPEG can be rebuilt byte for byte.
ImageCompress checks that the rebuilt file matches before it trusts the result. If it does not, it falls back to a
pixel-exact encode. This repacking is skipped if you also resize the image or change its colours.

## Only smaller results are kept

By default a result replaces the original only if it is smaller. Otherwise the file is skipped and left alone,
and the app says "not smaller". In the command line this is `--replace-if smaller` (the default).
Use `--replace-if always` to keep the result anyway.

## Metadata

Photos carry extra data: the colour profile, camera details (EXIF), location and more. You choose what to keep:

| Setting | Keeps |
| --- | --- |
| `none` | Nothing. All metadata is removed. |
| `icc` (default) | Only the colour profile (ICC). |
| `exif-no-gps` | Colour profile and EXIF, but removes the GPS location, embedded thumbnails and previews. |
| `all` | Colour profile, EXIF and XMP, also across format conversions. |

When pixels are rewritten upright, the orientation tag is reset so the picture still shows the right way up.

By default ImageCompress also drops a colour profile that only says "sRGB", since untagged images are sRGB anyway.
Wide-gamut profiles such as Display P3 are always kept, unless you ask to convert to sRGB. File dates are kept.

In the app these options are in the options panel (⌥⌘I) and in Settings. In the command line they are
[`--metadata`]({{ site.baseurl }}/cli/).

## Which apps open which format

Most apps open JPEG, PNG and GIF. WebP and AVIF are supported by current browsers and by many apps. HEIC opens
in Apple apps such as Preview and Photos, but support elsewhere is patchy. JPEG XL now shows in Chrome, Firefox
and Opera, and in Safari without animation or progressive loading, but not yet in Edge; [Can I use](https://caniuse.com/jpegxl) has the current list. If you publish images on the web, check the formats you need in the browsers you care about, or keep
JPEG, PNG or WebP.

If you are unsure, use Preserve with Smaller. Your files keep their format and every app still opens them.
