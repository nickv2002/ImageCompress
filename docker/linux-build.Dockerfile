# syntax=docker/dockerfile:1.7
#
# Static musl builds of imgcompress (+ gifsicle) on Alpine. Built natively per
# architecture (--platform linux/amd64,linux/arm64; non-native under QEMU) so the
# C/C++ dependencies (libaom, dav1d, libjxl, jpegli, mozjpeg, libwebp) link
# against Alpine's static libstdc++/musl with no cross toolchains.
#
# Used by scripts/build_linux.sh (stage `artifacts`) and by
# scripts/make_source_tarball.sh --verify (stage `toolchain`, offline build).
#
# STATUS: see scripts/build_linux.sh header for what has been verified.

ARG RUST_IMAGE=rust:1-alpine

# ---- toolchain: compilers + build tools, no sources ------------------------
FROM ${RUST_IMAGE} AS toolchain
RUN apk add --no-cache \
      build-base musl-dev linux-headers \
      cmake ninja meson nasm perl python3 pkgconf git curl \
      binutils file rsync bash
# Fully static: force crt-static + a static C++ runtime (libstdc++, libgcc) for
# jpegli/libjxl/libaom. The build passes an explicit --target so RUSTFLAGS apply
# to the target only (crt-static would break host proc-macro dylibs).
ENV RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-static-libstdc++ -C link-arg=-static-libgcc"
ENV CARGO_TERM_COLOR=never

# ---- gifsicle: static GPL-2.0-only helper, built from the pinned tarball ----
FROM toolchain AS gifsicle
ARG GIFSICLE_VERSION
ARG GIFSICLE_URL
ARG GIFSICLE_SHA256
RUN curl -fsSL -o /tmp/gifsicle.tar.gz "${GIFSICLE_URL}" \
 && echo "${GIFSICLE_SHA256}  /tmp/gifsicle.tar.gz" | sha256sum -c - \
 && mkdir /src && tar -xzf /tmp/gifsicle.tar.gz -C /src --strip-components=1
WORKDIR /src
RUN ./configure --disable-gifview --disable-gifdiff LDFLAGS=-static \
 && make -j"$(nproc)" \
 && strip src/gifsicle \
 && ! readelf -l src/gifsicle | grep -q INTERP

# ---- build: imgcompress ------------------------------------------------------
FROM toolchain AS build
ARG TARGETARCH
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,id=imgcompress-target-${TARGETARCH} \
    triple=$(rustc -vV | sed -n 's/^host: //p') \
 && cargo build --release --locked -p imgcompress --target "$triple" \
 && mkdir -p /out/completions /out/man \
 && cp "target/$triple/release/imgcompress" /out/imgcompress \
 && strip /out/imgcompress
# Must be fully static: no NEEDED entries, no dynamic interpreter.
RUN ! readelf -d /out/imgcompress | grep -q NEEDED \
 && ! readelf -l /out/imgcompress | grep -q INTERP \
 && (ldd /out/imgcompress 2>&1 | grep -qiE 'not a (valid )?dynamic|statically linked' || true) \
 && echo "static OK: $(file /out/imgcompress)" \
 && /out/imgcompress --version
# Shell completions / man page if the CLI provides them (skipped otherwise).
RUN for sh in bash zsh fish; do \
      /out/imgcompress completions "$sh" > "/out/completions/imgcompress.$sh" 2>/dev/null \
        || rm -f "/out/completions/imgcompress.$sh"; \
    done; \
    /out/imgcompress man > /out/man/imgcompress.1 2>/dev/null || rm -f /out/man/imgcompress.1; \
    true

# ---- artifacts: extract with --output type=local ------------------------------
FROM scratch AS artifacts
COPY --from=build /out/ /
COPY --from=gifsicle /src/src/gifsicle /gifsicle
