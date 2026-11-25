# Changelog

## 2026.11.25.1 — 2026-11-25

- Docs site: contents sidebar, real screenshots, macOS/CLI pills, star nudge and a format comparison page
- Format comparison: a zoomable split viewer, a Quality 25 breakdown strip, and PNG for every photo
- Format comparison: true lossless results, a size-against-quality chart, wider text and an easier divider
- Docs: JPEG XL now shows in Chrome, Firefox and Opera; link caniuse for current support
- Skip JPEG conversions at lossless quality instead of writing a lossy JPEG
- Docs: say what Safari's partial JPEG XL support leaves out
- JPEG: skip only when lossless was asked for; a plain --format jpeg converts at quality 85 again
- Plan: name the re-encode when a lossless JPEG has to be resized or converted to sRGB
- Docs: say JPEG is encoded with jpegli by default, what it improves on mozjpeg, and which encoder the comparison used
- Docs: tighten the jpegli wording (images not photos, compare at a score, the measured gap)
- Docs home: stop the compare viewer's .tag style floating the tagline over the header; add a jpegli-vs-mozjpeg card
- Docs: Open Graph and Twitter card tags with a 1200x630 social card, so links unfurl with an image in Slack, Mastodon and the like
- Docs home: a Free and open source badge in the hero and a short section on what that means
- Docs home: the requirements line names both the Mac app and the macOS/Linux command line
- Docs home: drop the hero badge; say Free and open source in the requirements line instead, linking to that section
- Docs: say plainly what built-in presets do to originals; a home-page section on making your own presets
- Finder menu: offer to turn it on at first launch, a Turn On button and pluginkit-based status in Settings → Finder, a warning listing other app copies, and docs for each way to enable it
- Install Command Line Tool: say where imgcompress is already installed (Homebrew, /usr/local/bin, ~/.local/bin) instead of failing; ask for an administrator password when /usr/local/bin is not writable
- Install Command Line Tool: show each install's path in monospace with its version and source in plain words, and the get-started command on its own line
- Before / After: open at once from the Undo copy and the result on disk, zoom to 1×–8× with a navigator map, Slider / Side by Side / Difference views, and a SSIMULACRA2 similarity score in plain words; new `imgcompress score`; row Undo and Reveal buttons no longer take keyboard focus, so Space can never press them
- Compress again from the original: adding a result again (drop, Add Files, Open With, Finder menu) or choosing Compress Again from Original puts the original back from its Undo copy first and compresses that with the current settings, including renamed results (photo.avif maps back to photo.png); a quiet, dismissible bar offers it when settings change; a refused restore compresses nothing and says why; docs for the app and the CLI
- Compress again: a refused restore is shown as a notice that survives the next run; the bar sits below the list content (not over the last row), slides in, and comes back after the next run; a Finder hand-off with nothing left to run gives the window its settings back
- Screenshot thumbnail drops: drag the floating screenshot thumbnail onto the window or Dock icon and it is moved out of macOS's temporary folder into the screenshot location (com.apple.screencapture location, else Desktop; " 2" if the name is taken), then compressed; window drops now read file URLs straight off the pasteboard, because SwiftUI's drop handling failed on drags that also carry a file promise
- Release: use local copies of the App Store Connect key and the repository signing key when ASC_KEY_PATH and REPO_KEY_FILE are set (from the gitignored release.local.env), so a release can run without a 1Password prompt
- Docs: retake the Before / After screenshot with the new comparison sheet (Slider at 2× with the navigator and the similarity verdict) and add a screenshot of the settings-changed Compress Again bar to the Undo page

## 2026.11.23.1 — 2026-11-23

- Show the release version in imgcompress --version, and fix build warnings
- Give the app its real version number, so Sparkle stops offering the installed release
- Draft the release summary with claude -p directly instead of the cldp shell alias
- Ask for 1Password approval at the start of a release, and retry timed-out prompts

## 2026.11.22.1 — 2026-11-22

- First release: the native Mac app with its Finder extension, the `imgcompress` command line tool for macOS and Linux, and Homebrew, apt and dnf packages.


