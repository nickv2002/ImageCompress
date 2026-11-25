#!/bin/bash
# Publishes the release's .deb and .rpm packages to the self-hosted apt/dnf repository (the GitHub Pages site of
# nickv2002/packages, https://nickv2002.github.io/packages), so `apt upgrade` / `dnf upgrade` pick up new releases.
#
#   deb/dists/stable/{InRelease,Release,Release.gpg}   signed apt index (amd64, arm64)
#   deb/pool/main/<l>/<name>/*.deb
#   rpm/repodata/repomd.xml{,.asc}                      signed dnf/zypper index
#   rpm/packages/*.rpm                                  (signed by sign_linux_packages.sh)
#   rpm/nickv2002.repo  nickv2002.asc  index.html
#
# The site is rebuilt each time from the packages already published plus the new ones, keeping the newest
# KEEP versions of each package, and pushed as a single commit that replaces the last: old packages never pile
# up in git history, and the site stays far below the 1 GB Pages limit. Re-running for the same version is safe.
#
# Usage: scripts/publish_linux_packages.sh [--no-push] [--reset] <version>...
#   --no-push  build the site in build/packages/site and stop (for testing)
#   --reset    drop everything already published (versions are then optional: none publishes an empty repository)
# Env: PACKAGES_REMOTE (default https://github.com/nickv2002/packages.git), SITE_URL (default
#      https://nickv2002.github.io/packages), KEEP (default 3), REPO_KEY_FILE (a local copy of the key, see repo_key.sh).
set -euo pipefail

push=1
reset=0
versions=()
for arg in "$@"; do
  case "$arg" in
    --no-push) push=0 ;;
    --reset) reset=1 ;;
    -*) echo "error: unknown option $arg" >&2; exit 1 ;;
    *) versions+=("$arg") ;;
  esac
done
[[ ${#versions[@]} -gt 0 || $reset -eq 1 ]] || { echo "usage: publish_linux_packages.sh [--no-push] [--reset] <version>..." >&2; exit 1; }

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
. scripts/repo_key.sh
remote="${PACKAGES_REMOTE:-https://github.com/nickv2002/packages.git}"
site_url="${SITE_URL:-https://nickv2002.github.io/packages}"
keep="${KEEP:-3}"

new=()
for version in "${versions[@]+"${versions[@]}"}"; do
  shopt -s nullglob
  debs=(dist/imgcompress_"${version}"_*.deb)
  rpms=(dist/imgcompress-"${version}"-*.rpm)
  shopt -u nullglob
  [[ ${#debs[@]} -gt 0 && ${#rpms[@]} -gt 0 ]] || { echo "error: no packages for ${version} in dist/" >&2; exit 1; }
  new+=("${debs[@]}" "${rpms[@]}")
done

work="build/packages"
rm -rf "$work"
mkdir -p "$work/site/deb/pool/main" "$work/site/rpm/packages"

echo "==> fetching the published repository"
if git ls-remote --exit-code "$remote" main >/dev/null 2>&1; then
  git clone --quiet --depth 1 --branch main "$remote" "$work/clone"
  if [[ $reset -eq 1 ]]; then echo "    (--reset: dropping the published packages)"; fi
  [[ $reset -eq 0 && -d "$work/clone/deb/pool" ]] && cp -R "$work/clone/deb/pool/." "$work/site/deb/pool/"
  [[ $reset -eq 0 && -d "$work/clone/rpm/packages" ]] && cp -R "$work/clone/rpm/packages/." "$work/site/rpm/packages/"
else
  echo "    (empty; starting a new repository)"
  git init --quiet -b main "$work/clone"
  git -C "$work/clone" remote add origin "$remote"
fi
for f in "${new[@]+"${new[@]}"}"; do
  case "$f" in
    *.deb) name=$(basename "$f"); name=${name%%_*}
           mkdir -p "$work/site/deb/pool/main/${name:0:1}/$name"
           cp "$f" "$work/site/deb/pool/main/${name:0:1}/$name/" ;;
    *.rpm) cp "$f" "$work/site/rpm/packages/" ;;
  esac
done

repo_tools
key=$(repo_private_key)
[[ "$key" == *"PRIVATE KEY BLOCK"* ]] || { echo "error: could not read the repository signing key" >&2; exit 1; }

echo "==> indexing and signing (keeping the newest ${keep} versions)"
printf '%s\n' "$key" | docker run --rm -i --tmpfs /root/.gnupg:mode=0700 -v "$repo_root/$work/site:/site" \
  -e FPR="$REPO_KEY_FPR" -e SITE_URL="$site_url" -e KEEP="$keep" "$REPO_TOOLS_IMAGE" bash -euo pipefail -c '
  gpg --batch --quiet --import 2>/dev/null
  gpg --batch --quiet --armor --export "$FPR" > /site/nickv2002.asc
  rpm --import /site/nickv2002.asc
  cd /site
  shopt -s nullglob

  # Prune: the newest $KEEP versions of each package, in both formats.
  for dir in deb/pool/main/*/*/; do
    name=$(basename "$dir")
    mapfile -t all < <(for f in "$dir"*.deb; do f=${f##*/}; f=${f#"${name}_"}; echo "${f%_*}"; done | sort -uV)
    drop=$(( ${#all[@]} - KEEP ))
    for ((i = 0; i < drop; i++)); do
      echo "    dropping $name ${all[i]}"
      rm -f "$dir${name}_${all[i]}_"*.deb rpm/packages/"${name}-${all[i]}-"*.rpm
    done
  done
  for f in rpm/packages/*.rpm; do
    rpm -K "$f" | grep -q "signatures OK" || { echo "error: $f is not signed (run sign_linux_packages.sh)" >&2; exit 1; }
  done

  # apt
  cd /site/deb
  for arch in amd64 arm64; do
    mkdir -p dists/stable/main/binary-$arch
    apt-ftparchive --arch $arch packages pool > dists/stable/main/binary-$arch/Packages
    gzip -9nkf dists/stable/main/binary-$arch/Packages
  done
  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin=nickv2002 -o APT::FTPArchive::Release::Label=nickv2002 \
    -o APT::FTPArchive::Release::Suite=stable -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Architectures="amd64 arm64" -o APT::FTPArchive::Release::Components=main \
    -o APT::FTPArchive::Release::Description="Packages by nickv2002" \
    release dists/stable > /tmp/Release
  mv /tmp/Release dists/stable/Release
  gpg --batch --yes --local-user "$FPR" --digest-algo SHA512 --clearsign -o dists/stable/InRelease dists/stable/Release
  gpg --batch --yes --local-user "$FPR" --digest-algo SHA512 -abs -o dists/stable/Release.gpg dists/stable/Release

  # dnf / zypper
  cd /site/rpm
  createrepo_c --quiet --no-database --general-compress-type=gz .
  gpg --batch --yes --local-user "$FPR" --digest-algo SHA512 --detach-sign --armor repodata/repomd.xml
  cat > nickv2002.repo <<REPO
[nickv2002]
name=nickv2002 packages
baseurl=${SITE_URL}/rpm
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=${SITE_URL}/nickv2002.asc
REPO

  # Listing for the landing page, newest first: "<name> <version> <files...>"
  cd /site
  for dir in deb/pool/main/*/*/; do
    name=$(basename "$dir")
    for v in $(for f in "$dir"*.deb; do f=${f##*/}; f=${f#"${name}_"}; echo "${f%_*}"; done | sort -uV -r); do
      echo "$name $v $(cd "$dir" && ls "${name}_${v}_"*.deb | tr "\n" " ")$(cd rpm/packages && ls "${name}-${v}-"*.rpm | tr "\n" " ")"
    done
  done > /tmp/listing
  cp /tmp/listing /site/.listing
'

fpr_spaced=$(echo "$REPO_KEY_FPR" | sed -E 's/(....)/\1 /g; s/ $//')
rows=""
while read -r name version files; do
  [[ -n "$name" ]] || continue
  links=""
  for f in $files; do
    case "$f" in
      *.deb) links+="<a href=\"deb/pool/main/${name:0:1}/${name}/${f}\">${f}</a><br>" ;;
      *.rpm) links+="<a href=\"rpm/packages/${f}\">${f}</a><br>" ;;
    esac
  done
  src="https://github.com/nickv2002/ImageCompress/releases/tag/v${version}"
  rows+="<tr><td>${name}</td><td>${version}</td><td>${links}</td><td><a href=\"${src}\">Source code</a></td></tr>"
done < "$work/site/.listing"
rm "$work/site/.listing"
[[ -n "$rows" ]] || rows='<tr><td colspan="4">No packages published yet.</td></tr>'

cat > "$work/site/index.html" <<HTML
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>nickv2002 packages</title>
<style>
:root { --bg: #fbfbfd; --text: #1d1d1f; --muted: #6e6e73; --line: #e3e3e8; --accent: #0a66d9; --code: #f2f2f5; color-scheme: light dark; }
@media (prefers-color-scheme: dark) { :root { --bg: #141416; --text: #f2f2f4; --muted: #a1a1a8; --line: #333338; --accent: #4c9bff; --code: #26262a; } }
body { margin: 0; background: var(--bg); color: var(--text); font: 16px/1.55 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }
main { max-width: 820px; margin: 0 auto; padding: 24px 16px 48px; }
a { color: var(--accent); }
pre, code { font: 0.9em/1.5 ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
pre { background: var(--code); padding: 12px 14px; border-radius: 8px; overflow-x: auto; }
table { border-collapse: collapse; width: 100%; font-size: 14px; display: block; overflow-x: auto; }
th, td { border-bottom: 1px solid var(--line); padding: 6px 8px; text-align: left; vertical-align: top; }
.muted { color: var(--muted); }
</style>
</head>
<body>
<main>
<h1>nickv2002 packages</h1>
<p>apt and dnf repositories for <a href="https://nickv2002.github.io/ImageCompress/">imgcompress</a>.
  Packages are signed; once the repository is added, your normal system updates keep them current.</p>

<h2>Debian, Ubuntu</h2>
<pre><code>sudo install -d -m 0755 /etc/apt/keyrings
sudo curl -fsSLo /etc/apt/keyrings/nickv2002.asc ${site_url}/nickv2002.asc
echo "deb [signed-by=/etc/apt/keyrings/nickv2002.asc] ${site_url}/deb stable main" \\
  | sudo tee /etc/apt/sources.list.d/nickv2002.list
sudo apt update && sudo apt install imgcompress</code></pre>

<h2>Fedora, RHEL, Rocky, Alma</h2>
<pre><code>sudo curl -fsSLo /etc/yum.repos.d/nickv2002.repo ${site_url}/rpm/nickv2002.repo
sudo dnf install imgcompress</code></pre>

<h2>openSUSE</h2>
<pre><code>sudo zypper addrepo ${site_url}/rpm/nickv2002.repo
sudo zypper install imgcompress</code></pre>

<h2>Signing key</h2>
<p><a href="nickv2002.asc">nickv2002.asc</a>, fingerprint <code>${fpr_spaced}</code>.</p>

<h2>Packages</h2>
<p class="muted">The newest ${keep} versions. imgcompress is free software under the GNU GPL v3 or later; the
  complete Corresponding Source for each version (and the source of the bundled gifsicle) is attached to its
  GitHub release, linked below.</p>
<table>
<tr><th>Package</th><th>Version</th><th>Files</th><th>Source</th></tr>
${rows}
</table>
</main>
</body>
</html>
HTML
touch "$work/site/.nojekyll"

if [[ $push -eq 0 ]]; then
  echo "==> built $work/site (not pushed)"
  exit 0
fi

echo "==> pushing to $remote"
clone="$work/clone"
find "$clone" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
cp -R "$work/site/." "$clone/"
git -C "$clone" checkout --quiet --orphan publish
git -C "$clone" add -A
git -C "$clone" -c user.name="$(git config user.name)" -c user.email="$(git config user.email)" \
  commit --quiet -m "Publish ${versions[*]:-(empty)}"
git -C "$clone" push --quiet --force origin publish:main
echo "==> published ${versions[*]:-an empty repository} to ${site_url}"
