FROM debian:bullseye-slim AS base
RUN apt update && apt upgrade -y
RUN apt -y install curl wget

FROM base AS final
# Install packages available via apt
RUN apt update && apt install -y pngquant gifsicle && \
    apt clean && \
    rm -rf /var/lib/apt/lists/*

# Download and install oxipng from GitHub releases
RUN curl -s -L https://api.github.com/repos/shssoichiro/oxipng/releases/latest | \
    grep "browser_download_url.*oxipng.*amd64.deb" | \
    cut -d '"' -f 4 | \
    xargs wget -O oxipng.deb && \
    dpkg -i oxipng.deb && \
    rm oxipng.deb

# Download and install cavif from GitHub releases
RUN curl -s -L https://api.github.com/repos/kornelski/cavif-rs/releases/latest | \
    grep "browser_download_url.*cavif.*amd64.deb" | \
    cut -d '"' -f 4 | \
    xargs wget -O cavif.deb && \
    dpkg -i cavif.deb && \
    rm cavif.deb
