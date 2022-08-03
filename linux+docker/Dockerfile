FROM debian:bullseye-slim as mozjpegbuild
WORKDIR /opt
RUN apt update && \
    apt upgrade && \
    apt -y install imagemagick build-essential libboost-all-dev \
    pkg-config autoconf automake libtool nasm make cmake flex libpng-tools libpng-dev \
    zlib1g-dev optipng advancecomp parallel git && \
    git clone https://github.com/mozilla/mozjpeg.git && \
    cd mozjpeg && \
    cmake . && \
    make

FROM debian:bullseye-slim as oxipngbuild
WORKDIR /rust
RUN apt update && \
    apt upgrade && \
    apt -y install curl build-essential libboost-all-dev && \
    curl https://sh.rustup.rs -s >> rustup.sh && \
    chmod 755 /rust/rustup.sh && \
    /rust/rustup.sh -y  && \
    /root/.cargo/bin/cargo install oxipng

FROM debian:bullseye-slim as final
RUN apt update && \
    apt upgrade && \
    apt install -y pngquant gifsicle imagemagick && \
    apt clean && \
    rm -rf /var/lib/apt/lists/*

COPY --from=mozjpegbuild /opt/mozjpeg/cjpeg-static /opt/mozjpeg/jpegtran-static /usr/bin/
COPY --from=oxipngbuild /root/.cargo/bin/oxipng /usr/bin/oxipng
