FROM debian:bullseye-slim AS base
RUN apt update && apt upgrade -y
RUN apt -y install git curl build-essential libboost-all-dev nasm

FROM base AS mozjpegbuild
WORKDIR /opt
RUN apt -y install imagemagick pkg-config autoconf automake libtool make cmake flex libpng-tools libpng-dev \
    zlib1g-dev optipng advancecomp parallel
RUN git clone https://github.com/mozilla/mozjpeg.git
WORKDIR /opt/mozjpeg
RUN cmake .
RUN make

FROM base AS oxipngbuild
WORKDIR /rust
RUN curl https://sh.rustup.rs -s >> rustup.sh && \
    chmod 755 /rust/rustup.sh && \
    /rust/rustup.sh -y
RUN /root/.cargo/bin/cargo install oxipng cavif

FROM base AS final
# RUN apt install -y pngquant gifsicle imagemagick && \
#     apt clean && \
#     rm -rf /var/lib/apt/lists/*
COPY --from=mozjpegbuild /opt/mozjpeg/cjpeg-static /opt/mozjpeg/jpegtran-static /usr/bin/
COPY --from=oxipngbuild /root/.cargo/bin/oxipng /usr/bin/oxipng
COPY --from=oxipngbuild /root/.cargo/bin/cavif /usr/bin/cavif
