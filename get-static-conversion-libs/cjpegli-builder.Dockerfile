FROM debian:bookworm-slim AS build

RUN apt update && apt install -y --no-install-recommends \
    git ca-certificates cmake ninja-build pkg-config clang \
    libbrotli-dev libpng-dev libgif-dev libjpeg-dev zlib1g-dev \
    python3 \
    && rm -rf /var/lib/apt/lists/*

ENV CC=clang CXX=clang++

# Invalidates the git-clone layer cache whenever the target commit changes,
# so `docker build` always fetches fresh source instead of reusing a stale clone.
ARG JPEGLI_COMMIT=unknown
RUN echo "jpegli commit: $JPEGLI_COMMIT"

RUN git clone --recursive --shallow-submodules --depth 1 \
    https://github.com/google/jpegli.git /src/jpegli

WORKDIR /src/jpegli
RUN cmake -B build -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_TESTING=OFF \
    -DJPEGLI_STATIC=ON \
    && cmake --build build -- -j$(nproc)

FROM scratch AS export
COPY --from=build /src/jpegli/build/tools/cjpegli /cjpegli
