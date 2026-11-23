# Tools for the self-hosted apt/dnf repository (scripts/publish_linux_packages.sh, scripts/sign_linux_packages.sh,
# scripts/make_repo_key.sh): apt-ftparchive, createrepo_c, rpmsign and gpg.
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends apt-utils createrepo-c rpm gnupg jq ca-certificates \
 && rm -rf /var/lib/apt/lists/*
ENV GNUPGHOME=/root/.gnupg
