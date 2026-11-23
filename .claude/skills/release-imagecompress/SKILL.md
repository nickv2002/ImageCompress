---
name: release-imagecompress
description: Cut an ImageCompress release (check in and merge pending work to main, license gate, build the Mac app + CLI, sign, notarize, Linux tarballs, GPL source tarball, changelog, Sparkle appcast, tag, push, GitHub release, Homebrew tap, Linux packages, install). Use when the user says /release-imagecompress or asks to ship a release.
disable-model-invocation: true
model: haiku
---

Cut a release with the quiet target. Never run `make release` or `scripts/release.sh` directly; their output is huge.

1. **Get pending work onto `main`.** The changelog is built from commit subjects since the last tag, so every subject must read as a user-facing changelog line (e.g. `Add AVIF quality slider to Settings`), never `WIP`/`fix`/`Merge branch …`.
   - **Look before committing.** Review `git status --porcelain` and `git diff --stat`. Commit only files that belong to the work. Stray or unexplained files (scratch output, logs, secrets, `dist/`, `build/`, anything outside `crates/ macos/ scripts/ docker/ licenses/ fixtures/ docs/ vendor/ Cargo.* deny.toml about.toml Makefile readme.md NOTICE.md COPYING LICENSE ExportOptions.plist packaging/ CHANGELOG.md .claude/`): stop and ask.
   - **Verify the tree first.** If there are uncommitted source changes, run `make test` and `make licenses` (timeout 600000 ms each). The Rust build must be warning-free, `cargo deny check licenses bans sources` must pass, and the tests must pass. On failure, stop and report; don't commit.
   - **Dirty tree on a feature branch:** commit there with one changelog-ready subject. Then `git checkout main && git pull --ff-only`, `git rebase main <branch>`, `git checkout main && git merge --ff-only <branch>`, `git branch -d <branch>`. Fast-forward only, so no merge commit lands in the changelog.
   - **Dirty tree on `main`:** commit directly.
   - **Clean feature branch with commits not on `main`:** same rebase + fast-forward merge as above.
   - **Stop and report** on rebase conflicts, a non-fast-forward `main`, or a branch already pushed with an open PR (ask whether to merge the PR instead).
   - **Never force-push.** Don't push here; the release pushes `main`.
2. **Preflight.** You must now be on `main` with a clean tree (`git status --porcelain` empty, untracked files count) and not behind `origin/main` (`git fetch github main`, then `git rev-list --count HEAD..github/main` is `0`). Local commits ahead are fine. Docker must be running (`docker info`) for the Linux builds and the offline source-tarball check. If any check fails, stop and report.
3. **Release.** Run `make release-quiet` (foreground, timeout 600000 ms; the Linux builds make this slow, so if it times out rerun in the background and wait for the exit notification). It prints only `==>` progress lines. Full output is in `build/release.log`. 1Password (`op`) will prompt the owner to approve the App Store Connect key during notarization, and the Linux package signing key (when signing the .rpm packages and when publishing the repository).
4. **On failure,** report the printed tail and the failing step. Don't retry blindly: the pipeline bumps versions in `macos/project.yml` and `Cargo.toml`, commits, tags and pushes partway through. Check `git status`, `git log -3` and `git tag --points-at HEAD` before any retry, and ask the user.
5. **On success,** report the commits that were merged, the tag, the assets attached (Mac zip and its unversioned `ImageCompress.zip` copy, macOS CLI tarball, Linux x86_64/aarch64 tarballs, .deb and .rpm packages, `imgcompress-<ver>-sources.tar.gz`, `gifsicle-*-src.tar.gz`), whether the Homebrew tap was pushed and the Linux packages were published to https://nickv2002.github.io/packages (or the `WARNING` line that skipped it), and the GitHub release URL (`gh release view <tag> --repo nickv2002/ImageCompress --json url --jq .url`; without `--repo` gh may answer "release not found").
